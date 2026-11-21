//! The file layer components of the xRIT stacks: the CADU import, the file assembler and the product store.
//! Standard: CGMS 03 (LRIT/HRIT Global Specification) section 5

use super::link::{item_format, item_port};
use crate::files;
use crate::items::{
    Cadu, ProductItem, ProductKind, SatText, SpacePacket, Vcdu, XritFile, CADU_LEN,
};
use crate::typed::{
    FileArgs, ImageRow, KeysArgs, ProductRow, ADD_KEYS, IMAGES, IMPORT_CADU, IMPORT_KEYS, PRODUCTS,
};
use crate::xrit::crypto::{parse_key_file, parse_key_text};
use crate::xrit::store::{
    gk2a_channel_name, parse_fy4_name, Changes, Product, Store, StoreConfig, StoreStats,
};
use sdrkit_codec_image::{PixelFormat, Raster};
use sdrkit_core::command::{Command, CommandError, CommandId, CommandSet, Commands};
use sdrkit_core::interface::Seconds;
use sdrkit_core::publish::{field, Published};
use sdrkit_core::{Choice, SettingSet};
use sdrkit_core::{
    ConfigError, Record, Results, Setting, SettingError, SettingId, StateBlob, StateError, Storage,
    Value, Values,
};
use sdrkit_graph::{
    no_values, values_of, Applied, Component, ComponentInfo, Format, Io, Item, Picture, Pool, Port,
    RunContext, Timed,
};
use sdrkit_proto_ccsds::channel::{ChannelConfig, ChannelDecoder};
use sdrkit_proto_ccsds::demux::{Continuity, VcCounters};
use sdrkit_proto_ccsds::packet::{Packet, PrimaryHeader, SeqFlags};
use sdrkit_proto_ccsds::xrit::headers::{parse, Dialect};
use sdrkit_proto_ccsds::xrit::transport::{AssemblerConfig, FileAssembler};
use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::sync::Arc;

// ---------------------------------------------------------------------------------------------------------------------------------

/// Counters of the CADU import.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ImportReport {
    /// CADUs imported.
    pub imported: u64,
    /// The message of the last import.
    pub note: String,
}
impl Item for ImportReport {
    const KIND: &'static str = "satimg.import_report";
}

/// The CADUs found before it pass through. The command `open_cadu` reads a file of CADUs and adds them. With a channel
/// configuration, the randomiser and Reed-Solomon are removed. The import tries the file as sent and as derandomised on its first
/// 16 frames and keeps the choice under which more frame headers say AOS, because the randomiser sequence is itself a code word.
pub struct CaduImportComponent {
    channel: Option<ChannelConfig>,
    pending: Vec<u8>,
    imported: u64,
    note: String,
    cadus: Vec<Cadu>,
    storage: Arc<dyn Storage>,
}

impl CaduImportComponent {
    /// An import for a link with the channel coding `channel` (`None`: frames without Reed-Solomon, as FY-4 sends them).
    pub fn new(channel: Option<ChannelConfig>) -> Self {
        CaduImportComponent {
            channel,
            pending: Vec::new(),
            imported: 0,
            note: String::new(),
            cadus: Vec::new(),
            storage: files::in_memory(),
        }
    }

    /// Returns the bytes of a CADU file to import at the next input.
    pub fn queue(&mut self, bytes: Vec<u8>) {
        self.pending = bytes;
    }

    /// The CADUs of the bytes of a CADU file. A file shorter than one CADU produces none and a note.
    pub fn import(&mut self, data: &[u8], out: &mut Vec<Cadu>) {
        if data.len() < CADU_LEN {
            self.note = "not a CADU file".into();
            return;
        }
        let frames: &[[u8; CADU_LEN]] = data.as_chunks::<CADU_LEN>().0;
        let blank = |f: &[u8; CADU_LEN], checked: bool| Cadu {
            bytes: *f,
            corrected: if checked { None } else { Some(0) },
            checked,
            basis: sdrkit_algo_fec::rs::Basis::Dual,
            time: 0.0,
        };
        match self.channel {
            None => out.extend(frames.iter().map(|f| blank(f, false))),
            Some(channel) => {
                let score = |cfg: ChannelConfig| -> usize {
                    let Ok(mut d) = ChannelDecoder::new(cfg) else {
                        return 0;
                    };
                    frames
                        .iter()
                        .take(16)
                        .filter(|f| {
                            let mut c = **f;
                            d.decode(&mut c[4..]).is_ok_and(|r| r.ok()) && c[4] >> 6 == 1
                        })
                        .count()
                };
                let plain = channel.without_randomiser();
                let cfg = if score(channel) > score(plain) {
                    channel
                } else {
                    plain
                };
                let Ok(mut dec) = ChannelDecoder::new(cfg) else {
                    self.note = "not a CADU file".into();
                    return;
                };
                for f in frames {
                    let mut c = blank(f, cfg.rs.is_some());
                    if let Ok(d) = dec.decode(&mut c.bytes[4..]) {
                        c.basis = d.basis;
                        if d.ok() {
                            c.corrected = Some(d.corrected.min(u16::MAX as u32) as u16);
                        }
                    }
                    out.push(c);
                }
            }
        }
        self.imported += frames.len() as u64;
        self.note = format!("{} CADUs imported", self.imported);
    }
}


sdrkit_core::commands! {
    /// The commands of [`CaduImportComponent`].
    pub enum CaduImportCommand {
        /// Imports a file of CADUs.
        OpenCadu = IMPORT_CADU.command,
    }
}

impl Component for CaduImportComponent {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "satimg.cadu_import",
            vec![Port::typed::<Cadu>("in")],
            vec![
                item_port::<Cadu>("out"),
                item_port::<ImportReport>("report"),
            ],
        )
    }
    fn configure(&mut self, _: &[Format]) -> Result<Vec<Format>, ConfigError> {
        Ok(vec![item_format::<Cadu>(), item_format::<ImportReport>()])
    }
    fn process(&mut self, io: &mut Io) {
        let cadus = io.items::<Cadu>(0);
        io.items_out::<Cadu>(0).extend_from_slice(cadus);
        if !self.pending.is_empty() {
            let data = std::mem::take(&mut self.pending);
            let mut out = std::mem::take(&mut self.cadus);
            self.import(&data, &mut out);
            io.items_out::<Cadu>(0).extend(out.drain(..).map(|c| Timed {
                time: 0.0,
                value: c,
            }));
            self.cadus = out;
        }
        if io.tick() {
            let r = ImportReport {
                imported: self.imported,
                note: std::mem::take(&mut self.note),
            };
            io.items_out::<ImportReport>(1).push(Timed {
                time: 0.0,
                value: r,
            });
        }
    }
    fn apply(&mut self, id: SettingId, _: &Value) -> Result<Applied, SettingError> {
        Err(SettingError::NoSuchId { id })
    }
    fn values(&self) -> Values {
        no_values()
    }
    fn any_ref(&self) -> Option<&dyn std::any::Any> {
        Some(self)
    }
    fn any_mut(&mut self) -> Option<&mut dyn std::any::Any> {
        Some(self)
    }
    fn commands(&self) -> &Commands {
        CaduImportCommand::commands()
    }
    fn invoke(&mut self, id: CommandId, args: &Values) -> Result<Option<Record>, CommandError> {
        match CaduImportCommand::from_id(id).ok_or(CommandError::NoSuchId { id })? {
            CaduImportCommand::OpenCadu => {
                let a: FileArgs = IMPORT_CADU.args(args)?;
                let bytes = files::read(self.storage.as_ref(), args, a.path)?;
                if bytes.len() >= CADU_LEN {
                    self.queue(bytes);
                } else {
                    self.note = "not a CADU file".into();
                }
            }
        }
        Ok(None)
    }
}

// ---------------------------------------------------------------------------------------------------------------------------------

/// Counters of the file layer.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct FilesStats {
    /// Files completed.
    pub files: u64,
    /// Files with errors.
    pub with_errors: u64,
    /// Files whose length differs from the length in their transport header.
    pub length_errors: u64,
    /// Files with damaged headers (dropped).
    pub damaged: u64,
    /// Files cut short by a new first packet.
    pub truncated: u64,
    /// Transfer frames, fill frames not counted.
    pub frames: u64,
    /// Frame count gaps.
    pub gaps: u64,
    /// Transfer frames by virtual channel.
    pub per_vc: BTreeMap<u8, u64>,
}

/// The report of the file layer.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct FilesReport {
    /// Counters.
    pub stats: FilesStats,
    /// New log lines.
    pub log: Vec<String>,
}
impl Item for FilesReport {
    const KIND: &'static str = "satimg.files_report";
}

/// Packets to LRIT and HRIT files, on the transport layer of `sdrkit-proto-ccsds`. A file whose length differs from the
/// length in its transport header is marked with errors (CGMS 03 section 5.3). The `vcdu` input counts the frames of each
/// virtual channel and the gaps of their frame counts.
pub struct FilesComponent {
    dialect: Dialect,
    fa: FileAssembler,
    counters: VcCounters,
    stats: FilesStats,
    log: Vec<String>,
    files: Vec<XritFile>,
}

impl FilesComponent {
    /// For files of `dialect`.
    pub fn new(dialect: Dialect) -> Self {
        FilesComponent {
            dialect,
            fa: FileAssembler::new(AssemblerConfig::default()),
            counters: VcCounters::new(24).unwrap_or_else(|_| unreachable!("24 bits")),
            stats: FilesStats::default(),
            log: Vec::new(),
            files: Vec::new(),
        }
    }

    /// Takes one packet and appends the files it completes.
    pub fn push(&mut self, p: &SpacePacket, out: &mut Vec<XritFile>) {
        let packet = Packet {
            vcid: p.vcid,
            header: PrimaryHeader::telemetry(p.apid, SeqFlags::from_bits(p.flags), p.seq),
            data: &p.data,
            crc_ok: p.crc_ok,
            gap_before: p.gap_before,
        };
        let dialect = self.dialect;
        let stats = &mut self.stats;
        let log = &mut self.log;
        let time = p.time;
        self.fa.push(&packet, |f| {
            let headers = match parse(f.bytes, dialect) {
                Ok(h) => h,
                Err(_) => {
                    stats.damaged += 1;
                    log.push("a file with damaged headers".into());
                    return;
                }
            };
            let mut errors = f.errors;
            if !f.length_ok {
                stats.length_errors += 1;
                errors = true;
                log.push(format!(
                    "a file of {} bytes says {} in its transport header (VC {}, APID {})",
                    f.bytes.len(),
                    f.declared_bits / 8,
                    f.vcid,
                    f.apid
                ));
            }
            stats.files += 1;
            if errors {
                stats.with_errors += 1;
            }
            if f.truncated {
                stats.truncated += 1;
            }
            out.push(XritFile {
                vcid: f.vcid,
                apid: f.apid,
                bytes: Arc::from(f.bytes),
                packet_starts: f.packet_starts.to_vec(),
                packet_index: f.packet_index.to_vec(),
                headers,
                errors,
                truncated: f.truncated,
                time,
            });
        });
    }

    /// The counters.
    pub fn stats(&self) -> &FilesStats {
        &self.stats
    }
}

impl Component for FilesComponent {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "satimg.xrit_files",
            vec![
                Port::typed::<SpacePacket>("in"),
                Port::typed::<Vcdu>("vcdu").optional("the frames are not counted"),
            ],
            vec![
                item_port::<XritFile>("out"),
                item_port::<FilesReport>("report"),
            ],
        )
    }
    fn configure(&mut self, _: &[Format]) -> Result<Vec<Format>, ConfigError> {
        self.fa.reset();
        self.counters.restart();
        Ok(vec![
            item_format::<XritFile>(),
            item_format::<FilesReport>(),
        ])
    }
    fn apply(&mut self, id: SettingId, _: &Value) -> Result<Applied, SettingError> {
        Err(SettingError::NoSuchId { id })
    }
    fn values(&self) -> Values {
        no_values()
    }
    fn process(&mut self, io: &mut Io) {
        if io.is_connected(1) {
            for v in io.items::<Vcdu>(1) {
                self.stats.frames += 1;
                *self.stats.per_vc.entry(v.value.vcid).or_insert(0) += 1;
                if let Continuity::Gap { .. } = self.counters.push(v.value.vcid, v.value.counter) {
                    self.stats.gaps += 1;
                }
            }
        }
        let mut files = std::mem::take(&mut self.files);
        for p in io.items::<SpacePacket>(0) {
            self.push(&p.value, &mut files);
        }
        io.items_out::<XritFile>(0)
            .extend(files.drain(..).map(|f| Timed {
                time: f.time,
                value: f,
            }));
        self.files = files;
        if io.tick() {
            let r = FilesReport {
                stats: self.stats.clone(),
                log: std::mem::take(&mut self.log),
            };
            io.items_out::<FilesReport>(1).push(Timed {
                time: 0.0,
                value: r,
            });
        }
    }
}

// ---------------------------------------------------------------------------------------------------------------------------------

// ---------------------------------------------------------------------------------------------------------------------------------

/// The report of the product store.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct StoreReport {
    /// Counters.
    pub stats: StoreStats,
    /// Products kept.
    pub products: usize,
    /// Keys loaded.
    pub keys: usize,
    /// New log lines.
    pub log: Vec<String>,
    /// JPEG 2000 segments still decoding.
    pub decoding: usize,
}
impl Item for StoreReport {
    const KIND: &'static str = "satimg.store_report";
}

sdrkit_core::choice_enum! {
    /// Which products the `products` table lists.
    pub enum ProductFilter {
        /// Every product.
        All = ("all", "All products"),
        /// Image products.
        Images = ("images", "Images"),
        /// Text and data products.
        Text = ("text", "Text"),
    }
}

impl ProductFilter {
    fn shows(self, k: ProductKind) -> bool {
        match self {
            ProductFilter::All => true,
            ProductFilter::Images => k.is_image(),
            ProductFilter::Text => !k.is_image(),
        }
    }
}

/// Which products the `products` table lists.
pub const FILTER: Setting<field::Choice<ProductFilter>> =
    Setting::choice("filter", "Show", ProductFilter::All);
/// The host shows the newest product.
pub const FOLLOW_NEWEST: Setting<field::Bool> = Setting::new("follow_newest", "Newest", "", true);

sdrkit_core::settings! {
    /// The settings of [`ProductComponent`].
    pub enum ProductSetting {
        /// Which products the table lists.
        Filter = FILTER,
        /// Show the newest product.
        FollowNewest = FOLLOW_NEWEST,
    }
}

/// Forgets every product.
pub const CLEAR: Command = Command::new("clear", "Clear").help("Forgets every product.");

sdrkit_core::commands! {
    /// The commands of [`ProductComponent`].
    pub enum ProductCommand {
        /// Reads a KMA key file.
        ImportKeys = IMPORT_KEYS.command,
        /// Adds keys typed as text.
        AddKeys = ADD_KEYS.command,
        /// Forgets every product.
        Clear = CLEAR,
    }
}

/// The product store as a component: files in. Out: product events, texts, pictures (grey pictures of image products and
/// compressed pictures as they came) and the report. It writes the tables `products` and `images`. The products leave through
/// [`Component::save_state`] and come back through [`Component::restore_state`].
pub struct ProductComponent {
    store: Store,
    changes: Changes,
    filter: ProductFilter,
    newest: bool,
    frame_due: std::collections::BTreeSet<u32>,
    pool: Option<Pool>,
    live: bool,
    dirty: bool,
    key: String,
    gone: Vec<String>,
    published: Published,
    storage: Arc<dyn Storage>,
}

impl ProductComponent {
    /// A store for a stack.
    pub fn new(cfg: StoreConfig) -> Self {
        ProductComponent {
            store: Store::new(cfg),
            changes: Changes::default(),
            filter: ProductFilter::All,
            newest: true,
            frame_due: Default::default(),
            pool: None,
            live: false,
            dirty: false,
            key: String::new(),
            gone: Vec::new(),
            published: Published::new(&[]).with_tables(&[&PRODUCTS, &IMAGES]),
            storage: files::in_memory(),
        }
    }
    /// The store.
    pub fn store(&self) -> &Store {
        &self.store
    }

    fn add_keys(&mut self, keys: BTreeMap<u16, [u8; 8]>) {
        let mut ch = std::mem::take(&mut self.changes);
        let n = self.store.import_keys(keys, &mut ch);
        self.changes = ch;
        if n > 0 {
            self.store.log.push(format!("{n} keys read"));
        }
        self.dirty = true;
    }

    /// Writes the rows of the products and the finished images that changed, and removes the rows of products that went.
    fn write_tables(&mut self) {
        let p = &mut self.published;
        for t in [PRODUCTS.def(), IMAGES.def()] {
            self.gone.clear();
            if let Some(id) = p.results().decl().table_id(t.key) {
                if let Some(table) = p.results().table(id) {
                    for (k, _) in table.rows_newest_first() {
                        let kept = k.parse::<u32>().ok().and_then(|id| self.store.product(id));
                        let keep = kept.is_some_and(|x| {
                            if t.key == PRODUCTS.key {
                                self.filter.shows(x.kind)
                            } else {
                                finished_image(x)
                            }
                        });
                        if !keep {
                            self.gone.push(k.to_owned());
                        }
                    }
                }
            }
            for k in &self.gone {
                p.remove_row(t, k);
            }
        }
        for x in &self.store.products {
            self.key.clear();
            // A write to a String cannot fail.
            let _ = write!(self.key, "{}", x.id);
            if self.filter.shows(x.kind) {
                PRODUCTS.put(
                    p,
                    &self.key,
                    &ProductRow {
                        product: &x.name,
                        kind: x.kind,
                        done: &x.done(),
                    },
                );
            }
            if finished_image(x) {
                let channel = image_channel(x);
                IMAGES.put(
                    p,
                    &self.key,
                    &ImageRow {
                        product: &x.name,
                        channel: channel.as_deref(),
                        time: Some(Seconds(x.start)),
                        width: i64::from(x.width),
                        height: Some(i64::from(x.height)),
                    },
                );
            }
        }
    }
}

/// A product that is a whole picture.
fn finished_image(p: &Product) -> bool {
    p.kind.is_image() && p.complete && (p.picture.is_some() || p.compressed.is_some())
}

/// The channel that the name of a product indicates (GK-2A and FY-4).
fn image_channel(p: &Product) -> Option<String> {
    if let Some(c) = gk2a_channel_name(p.channel) {
        return Some(c.to_owned());
    }
    parse_fy4_name(&p.name).map(|n| n.channel)
}

impl Component for ProductComponent {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "satimg.products",
            vec![Port::typed::<XritFile>("in")],
            vec![
                item_port::<ProductItem>("events"),
                item_port::<SatText>("text"),
                item_port::<Picture>("pictures"),
                item_port::<StoreReport>("report"),
            ],
        )
        .with_settings(ProductSetting::schema())
    }
    fn configure(&mut self, _: &[Format]) -> Result<Vec<Format>, ConfigError> {
        Ok(vec![
            item_format::<ProductItem>(),
            item_format::<SatText>(),
            item_format::<Picture>(),
            item_format::<StoreReport>(),
        ])
    }
    fn attach(&mut self, ctx: &RunContext) {
        // Live input: JPEG 2000 on the helper pool. A recording or a generator decodes in line (the same products at any
        // speed).
        self.live = ctx.live;
        self.pool = Some(ctx.pool.clone());
        self.store.pool = if self.live { self.pool.clone() } else { None };
    }
    fn process(&mut self, io: &mut Io) {
        let mut ch = std::mem::take(&mut self.changes);
        for f in io.items::<XritFile>(0) {
            self.store.push(f.value.clone(), &mut ch);
        }
        self.store.poll(&mut ch);
        self.dirty |= !ch.items.is_empty();
        io.items_out::<ProductItem>(0)
            .extend(ch.items.drain(..).map(|it| Timed {
                time: 0.0,
                value: it,
            }));
        for (_, codec, bytes, name) in ch.compressed.drain(..) {
            io.items_out::<Picture>(2).push(Timed {
                time: 0.0,
                value: Picture {
                    codec,
                    layout: None,
                    name: Some(name),
                    data: bytes,
                },
            });
        }
        for (id, name, text, inner) in ch.texts.drain(..) {
            io.items_out::<SatText>(1).push(Timed {
                time: 0.0,
                value: SatText {
                    name,
                    text,
                    inner_name: inner,
                    product: id,
                },
            });
        }
        self.frame_due.extend(ch.pictures.drain(..));
        self.changes = ch;
        if !io.tick() {
            return;
        }
        // The pictures of products that changed, at most one each per report.
        for id in std::mem::take(&mut self.frame_due) {
            if let Some(p) = self.store.product(id) {
                if let Some(pic) = &p.picture {
                    if let Ok(r) =
                        Raster::from_vec(PixelFormat::Grey8, p.width, p.height, pic.clone())
                    {
                        io.items_out::<Picture>(2).push(Timed {
                            time: 0.0,
                            value: r.to_picture(&p.name),
                        });
                    }
                }
            }
        }
        if std::mem::take(&mut self.dirty) {
            self.write_tables();
        }
        let r = StoreReport {
            stats: self.store.stats.clone(),
            products: self.store.products.len(),
            keys: self.store.keys(),
            log: std::mem::take(&mut self.store.log),
            decoding: self.store.decoding(),
        };
        io.items_out::<StoreReport>(3).push(Timed {
            time: 0.0,
            value: r,
        });
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        match ProductSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            ProductSetting::Filter => {
                self.filter = FILTER.read(id, v)?;
                self.dirty = true;
            }
            ProductSetting::FollowNewest => self.newest = FOLLOW_NEWEST.read(id, v)?,
        }
        Ok(Applied::Live)
    }
    fn values(&self) -> Values {
        values_of([
            (ProductSetting::Filter, Value::Choice(self.filter.index())),
            (ProductSetting::FollowNewest, Value::Bool(self.newest)),
        ])
    }
    fn results(&self) -> Option<&Results> {
        Some(self.published.results())
    }
    fn any_ref(&self) -> Option<&dyn std::any::Any> {
        Some(self)
    }
    fn any_mut(&mut self) -> Option<&mut dyn std::any::Any> {
        Some(self)
    }
    fn commands(&self) -> &Commands {
        ProductCommand::commands()
    }
    fn invoke(&mut self, id: CommandId, args: &Values) -> Result<Option<Record>, CommandError> {
        match ProductCommand::from_id(id).ok_or(CommandError::NoSuchId { id })? {
            ProductCommand::ImportKeys => {
                let a: FileArgs = IMPORT_KEYS.args(args)?;
                let bytes = files::read(self.storage.as_ref(), args, a.path)?;
                self.add_keys(parse_key_file(&bytes));
            }
            ProductCommand::AddKeys => {
                let a: KeysArgs = ADD_KEYS.args(args)?;
                let keys = parse_key_text(a.keys).map_err(|e| {
                    match args.schema().id("keys") {
                        Ok(arg) => CommandError::BadArgument {
                            arg,
                            error: SettingError::Malformed {
                                id: arg,
                                at: Some(e.at),
                            },
                        },
                        Err(_) => CommandError::NotAnArgument { key: "keys" },
                    }
                })?;
                self.add_keys(keys);
            }
            ProductCommand::Clear => {
                self.store.clear();
                self.newest = true;
                self.dirty = true;
            }
        }
        Ok(None)
    }
    fn save_state(&self) -> Option<StateBlob> {
        if self.store.products.is_empty() {
            return None;
        }
        Some(self.store.save_state())
    }
    fn restore_state(&mut self, state: &StateBlob) -> Result<(), StateError> {
        self.store.restore_state(state)?;
        self.dirty = true;
        Ok(())
    }
}
