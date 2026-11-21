//! FY-4 LRIT and FY-4A HRIT-II/III (`fy4_lrit`, `fy4a_hrit`): DVB-S2 carrying an MPEG transport stream whose payloads
//! carry 1024-byte CADUs, frames of 1020 bytes without Reed-Solomon, files in the FY-4 dialect and segments joined into pictures.
//! The DVB-S2 receiver is the component of `sdrkit-proto-dvbs2` ([`s2::attach`]); CADU files that the host imports join the
//! same chain.

use super::display::log_lines;
use super::s2::{
    self, Dvbs2Parts, LinkState, CARRIER_OFFSET, FRAME_LENGTH, MODCOD, MODCODS, MODCOD_RULE,
    PILOTS_RULE, PL_SCRAMBLING,
};
use crate::components::last;
use crate::components::link::{item_format, item_port, PacketReport, VcduComponent};
use crate::components::xrit::{
    CaduImportComponent, FilesComponent, FilesReport, ImportReport, ProductComponent, StoreReport,
    FILTER, FOLLOW_NEWEST,
};
use crate::gen::{aos_frame, space_packet, ZoneStream};
use crate::items::{Cadu, CADU_LEN};
use crate::typed::Fy4Fields;
use crate::xrit::store::StoreConfig;
use crate::REPORT_S;
use sdrkit_core::interface::{Fields as _, Seconds};
use sdrkit_core::media::Packet;
use sdrkit_core::publish::{field, Published};
use sdrkit_core::SignalType;
use sdrkit_core::{
    AutoGain, Choice, ConfigError, GenConfig, Generator, LockProfile, Receiver, Results,
    ResultsDecl, Schema, Setting, SettingError, SettingId, SettingSet, StackConfig,
    StackDescriptor, Tuning, Value, Values,
};
use sdrkit_graph::{
    no_values, values_of, Applied, Catalog, Component, ComponentInfo, Format, GraphBuilder,
    GraphError, Io, Item, Port, StackGraph, Timed,
};
use sdrkit_proto_ccsds::aos::Layout;
use sdrkit_proto_ccsds::marker;
use sdrkit_proto_ccsds::tm::FirstHeader;
use sdrkit_proto_ccsds::xrit::headers::{Builder, Dialect};
use sdrkit_proto_ccsds::xrit::transport::split_file;
use sdrkit_proto_mpegts::packet::{parse, write_header, Header, NULL_PID, TS_SIZE};
use sdrkit_proto_mpegts::source::PacketSource;
use sdrkit_proto_mpegts::stream::TsPacket;
use sdrkit_proto_dvbs2::settings::{RollOffChoice, ISI};
use std::collections::{BTreeMap, VecDeque};
use std::fmt::Write as _;
use std::sync::Arc;

/// A link: (setting value, name, frequency, symbol rate, MODCOD name, MODCOD, ideal Es/N0).
pub type Fy4Link = (&'static str, &'static str, f64, f64, &'static str, u8, f64);

/// The links of FY-4.
pub const LINKS: [Fy4Link; 3] = [
    (
        "fy4a",
        "FY-4A LRIT",
        1697.0e6,
        90_000.0,
        "QPSK 2/5",
        3,
        -0.30,
    ),
    (
        "fy4b",
        "FY-4B LRIT",
        1697.0e6,
        120_000.0,
        "QPSK 8/9",
        10,
        6.20,
    ),
    (
        "hrit",
        "FY-4A HRIT",
        1679.0e6,
        1_000_000.0,
        "QPSK 5/6",
        9,
        5.18,
    ),
];

sdrkit_core::choice_enum! {
    /// The FY-4 satellite of the LRIT stack.
    pub enum Fy4Satellite {
        /// FY-4A, 90 ksym/s.
        Fy4a = ("fy4a", "FY-4A (90 ksym/s)"),
        /// FY-4B, 120 ksym/s.
        Fy4b = ("fy4b", "FY-4B (120 ksym/s)"),
    }
}

/// The satellite of the LRIT stack.
pub const SATELLITE_SETTING: Setting<field::Choice<Fy4Satellite>> =
    Setting::choice("satellite", "Satellite", Fy4Satellite::Fy4a)
        .help("A running LRIT stack switches at once.");
/// The symbol rate of `fy4_lrit` (0: found from the spectrum; the satellites send 90 000 and 120 000 sym/s).
pub const LRIT_SYMBOL_RATE: Setting<field::Hz> = s2::symbol_rate(0.0);
/// The symbol rate of `fy4a_hrit`.
pub const HRIT_SYMBOL_RATE: Setting<field::Hz> = s2::symbol_rate(1_000_000.0);
/// The roll-off.
pub const ROLL_OFF: Setting<field::Choice<RollOffChoice>> = s2::roll_off(RollOffChoice::Auto);

sdrkit_core::settings! {
    /// The settings of `fy4_lrit`.
    pub enum Fy4LritSetting {
        /// The satellite.
        Satellite = SATELLITE_SETTING,
        /// The symbol rate.
        SymbolRate = LRIT_SYMBOL_RATE,
        /// The roll-off.
        RollOff = ROLL_OFF,
        /// The carrier.
        CarrierOffset = CARRIER_OFFSET,
        /// The input stream.
        Isi = ISI,
        /// Which MODCODs are decoded.
        ModcodRule = MODCOD_RULE,
        /// The MODCOD of the rule "fixed".
        Modcod = MODCOD,
        /// The MODCODs of the rule "set".
        Modcods = MODCODS,
        /// Which frame length is decoded.
        FrameLength = FRAME_LENGTH,
        /// The expected pilots.
        Pilots = PILOTS_RULE,
        /// The PL scrambling code.
        PlScrambling = PL_SCRAMBLING,
        /// Which products the table lists.
        Filter = FILTER,
        /// Show the newest product.
        FollowNewest = FOLLOW_NEWEST,
    }
}

sdrkit_core::settings! {
    /// The settings of `fy4a_hrit`.
    pub enum Fy4HritSetting {
        /// The symbol rate.
        SymbolRate = HRIT_SYMBOL_RATE,
        /// The roll-off.
        RollOff = ROLL_OFF,
        /// The carrier.
        CarrierOffset = CARRIER_OFFSET,
        /// The input stream.
        Isi = ISI,
        /// Which MODCODs are decoded.
        ModcodRule = MODCOD_RULE,
        /// The MODCOD of the rule "fixed".
        Modcod = MODCOD,
        /// The MODCODs of the rule "set".
        Modcods = MODCODS,
        /// Which frame length is decoded.
        FrameLength = FRAME_LENGTH,
        /// The expected pilots.
        Pilots = PILOTS_RULE,
        /// The PL scrambling code.
        PlScrambling = PL_SCRAMBLING,
        /// Which products the table lists.
        Filter = FILTER,
        /// Show the newest product.
        FollowNewest = FOLLOW_NEWEST,
    }
}

sdrkit_core::settings! {
    /// The settings of [`Fy4Display`].
    pub enum Fy4DisplaySetting {
        /// The satellite (LRIT).
        Satellite = SATELLITE_SETTING,
        /// The symbol rate.
        SymbolRate = LRIT_SYMBOL_RATE,
    }
}

/// The settings of a stack.
pub fn schema_for(hrit: bool) -> Arc<Schema> {
    if hrit {
        Fy4HritSetting::schema()
    } else {
        Fy4LritSetting::schema()
    }
}

fn lrit_schema() -> Arc<Schema> {
    Fy4LritSetting::schema()
}
fn hrit_schema() -> Arc<Schema> {
    Fy4HritSetting::schema()
}

// ---------------------------------------------------------------------------------------------------------------------------------
// CADUs in the transport stream.

/// Counters of the CADU extractor.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct TsStats {
    /// Transport packets.
    pub packets: u64,
    /// Transport errors (sync byte, transport_error_indicator).
    pub errors: u64,
    /// CADUs found.
    pub cadus: u64,
    /// Packets of each PID.
    pub pids: BTreeMap<u16, u64>,
}

/// Finds 1024-byte CADUs in transport stream payloads (any PID, a 40-byte header first).
#[derive(Clone, Debug, Default)]
pub struct TsCaduExtractor {
    building: BTreeMap<u16, Vec<u8>>,
    /// Counters.
    pub stats: TsStats,
}

impl TsCaduExtractor {
    /// Takes one 188-byte transport packet and appends the CADUs it completes.
    pub fn push(&mut self, p: &[u8], out: &mut Vec<Cadu>) {
        self.stats.packets += 1;
        let Some(t) = parse(p) else {
            self.stats.errors += 1;
            return;
        };
        if t.header.tei {
            self.stats.errors += 1;
            return;
        }
        let pid = t.header.pid;
        if pid == NULL_PID {
            return;
        }
        *self.stats.pids.entry(pid).or_default() += 1;
        if t.payload.is_empty() {
            return;
        }
        let mut payload = t.payload;
        if t.header.pusi {
            let ptr = payload[0] as usize;
            let rest = &payload[1..];
            if let Some(mut b) = self.building.remove(&pid) {
                b.extend_from_slice(&rest[..ptr.min(rest.len())]);
                self.finish(&b, out);
            }
            payload = &rest[ptr.min(rest.len())..];
            self.building.insert(pid, payload.to_vec());
        } else if let Some(b) = self.building.get_mut(&pid) {
            b.extend_from_slice(payload);
        } else {
            return;
        }
        // A payload is complete once it holds the header and a whole CADU.
        let done = self
            .building
            .get(&pid)
            .is_some_and(|b| find_marker(b).is_some_and(|at| b.len() >= at + CADU_LEN));
        if done {
            if let Some(b) = self.building.remove(&pid) {
                self.finish(&b, out);
            }
        }
    }

    fn finish(&mut self, b: &[u8], out: &mut Vec<Cadu>) {
        if let Some(at) = find_marker(b).filter(|&at| b.len() >= at + CADU_LEN) {
            let mut c = Cadu {
                bytes: [0; CADU_LEN],
                corrected: Some(0),
                checked: false,
                basis: sdrkit_algo_fec::rs::Basis::Dual,
                time: 0.0,
            };
            c.bytes.copy_from_slice(&b[at..at + CADU_LEN]);
            self.stats.cadus += 1;
            out.push(c);
        }
    }
}

fn find_marker(b: &[u8]) -> Option<usize> {
    let m = marker::TM.bytes();
    (0..b.len().min(128).saturating_sub(3)).find(|&i| &b[i..i + 4] == m)
}

/// Packs a CADU into transport packets on `pid` (the generator's side): the payload is a 40-byte header then the CADU, its
/// first packet with PUSI and a pointer field of 0.
pub fn ts_packets(cadu: &[u8], pid: u16, cc: &mut u8) -> Vec<[u8; TS_SIZE]> {
    let mut payload = vec![0u8; 40];
    payload.extend_from_slice(cadu);
    let mut out = Vec::new();
    let mut first = true;
    let mut pos = 0;
    while pos < payload.len() {
        let mut p = [0xFFu8; TS_SIZE];
        write_header(
            &mut p,
            &Header {
                pusi: first,
                pid,
                afc: 1,
                cc: *cc & 15,
                ..Header::default()
            },
        );
        *cc = cc.wrapping_add(1);
        let mut o = 4;
        if first {
            p[4] = 0;
            o = 5;
            first = false;
        }
        let n = (TS_SIZE - o).min(payload.len() - pos);
        p[o..o + n].copy_from_slice(&payload[pos..pos + n]);
        pos += n;
        out.push(p);
    }
    out
}

/// The report of the extractor.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct TsReport {
    /// Counters.
    pub stats: TsStats,
}
impl Item for TsReport {
    const KIND: &'static str = "satimg.ts_report";
}

/// Transport packets in, CADUs out.
#[derive(Default)]
pub struct TsComponent {
    x: TsCaduExtractor,
    cadus: Vec<Cadu>,
}

impl Component for TsComponent {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "satimg.ts_cadu",
            vec![sdrkit_proto_mpegts::stream::port("in")],
            vec![item_port::<Cadu>("out"), item_port::<TsReport>("report")],
        )
    }
    fn configure(&mut self, _: &[Format]) -> Result<Vec<Format>, ConfigError> {
        Ok(vec![item_format::<Cadu>(), item_format::<TsReport>()])
    }
    fn apply(&mut self, id: SettingId, _: &Value) -> Result<Applied, SettingError> {
        Err(SettingError::NoSuchId { id })
    }
    fn values(&self) -> Values {
        no_values()
    }
    /// The transport stream counted is of the carrier of the frequency before.
    fn forget(&mut self, _: &sdrkit_graph::Kept) {
        *self = TsComponent::default();
    }
    fn process(&mut self, io: &mut Io) {
        let mut cadus = std::mem::take(&mut self.cadus);
        for p in io.items::<TsPacket>(0) {
            self.x.push(p.value.bytes(), &mut cadus);
        }
        io.items_out::<Cadu>(0)
            .extend(cadus.drain(..).map(|c| Timed {
                time: 0.0,
                value: c,
            }));
        self.cadus = cadus;
        if io.tick() {
            let r = TsReport {
                stats: self.x.stats.clone(),
            };
            io.items_out::<TsReport>(1).push(Timed {
                time: 0.0,
                value: r,
            });
        }
    }
}

// ---------------------------------------------------------------------------------------------------------------------------------
// The display.

fn display_declaration() -> ResultsDecl {
    ResultsDecl::new().with_fields(Fy4Fields::SHAPE.fields)
}

/// What the FY-4 stack `id` publishes.
pub fn declaration_of(id: &str) -> ResultsDecl {
    crate::typed::shape(id).results_decl()
}

/// What the FY-4 stacks publish.
pub fn declaration() -> ResultsDecl {
    declaration_of(LRIT.id)
}

/// The FY-4 display.
pub struct Fy4Display {
    hrit: bool,
    satellite: Fy4Satellite,
    user_rs: f64,
    published: Published,
    summary: String,
    hint: String,
    signal: String,
    store: Option<StoreReport>,
}

impl Fy4Display {
    /// The display of the LRIT or the HRIT stack.
    pub fn new(hrit: bool) -> Self {
        Fy4Display {
            hrit,
            satellite: Fy4Satellite::Fy4a,
            user_rs: 0.0,
            published: super::display::published(display_declaration()),
            summary: String::new(),
            hint: String::new(),
            signal: String::new(),
            store: None,
        }
    }
    fn link(&self) -> Fy4Link {
        link_of(self.hrit, self.satellite)
    }
    /// Publishes the fields, the status and the log of the reports given.
    fn show(
        &mut self,
        dv: &LinkState,
        ts: Option<&TsStats>,
        pk: Option<&PacketReport>,
        fl: Option<&FilesReport>,
        st: Option<&StoreReport>,
        im: Option<&ImportReport>,
    ) {
        let (_, name, _, rs0, modcod_name, _, _) = self.link();
        let rs = if self.user_rs > 0.0 {
            self.user_rs
        } else {
            rs0
        };
        let imported = im.map_or(0, |i| i.imported);
        let cadus = ts.map_or(0, |t| t.cadus) + imported;
        let files = fl.map_or(0, |f| f.stats.files);
        let products = st.map_or(0, |s| s.products);
        let state = if cadus > 0 && files > 0 {
            3
        } else if dv.state >= 2 || cadus > 0 {
            2
        } else {
            dv.state.min(1)
        };
        let es = dv.esn0_db.unwrap_or(0.0);
        self.summary.clear();
        // A write to a String cannot fail.
        let _ = match state {
            3 => write!(
                self.summary,
                "{name}: {} LDPC frames ({} bad), {cadus} CADUs, {products} products, Es/N0 {es:.1} dB",
                dv.bch_ok, dv.bch_bad
            ),
            2 => write!(self.summary, "{name}: frames found, waiting for data"),
            1 => write!(
                self.summary,
                "{name}: carrier found ({:.1} kHz), looking for frames",
                dv.offset_hz / 1e3
            ),
            _ => write!(
                self.summary,
                "{name}: searching for the {:.0} ksym/s carrier",
                rs / 1e3
            ),
        };
        self.hint.clear();
        // A write to a String cannot fail.
        let _ = if let Some(h) = s2::rate_hint(dv.input_rate, rs) {
            write!(self.hint, "{h}")
        } else if state == 3 && products == 0 {
            write!(
                self.hint,
                "Data units arrive. The first complete file (a picture segment) takes a while."
            )
        } else {
            Ok(())
        };
        self.signal.clear();
        // A write to a String cannot fail.
        let _ = write!(self.signal, "DVB-S2 {modcod_name}, {:.1} ksym/s", rs / 1e3);
        let res = self.published.results_mut();
        if let Some(im) = im.filter(|i| !i.note.is_empty()) {
            res.log(im.note.as_str());
        }
        if let Some(fl) = fl {
            log_lines(res, &fl.log);
        }
        if let Some(st) = st {
            log_lines(res, &st.log);
        }
        let p = &mut self.published;
        s2::write_status(p, dv, state, &self.summary);
        p.put(&Fy4Fields {
            link: name,
            signal: &self.signal,
            ts_packets: ts.map_or(0, |t| t.packets) as i64,
            ts_errors: ts.map_or(0, |t| t.errors) as i64,
            cadus: cadus as i64,
            cadus_imported: imported as i64,
            transfer_frames: fl.map(|f| f.stats.frames as i64),
            fill_frames: pk.map(|k| k.stats.fill_frames as i64),
            frame_gaps: fl.map(|f| f.stats.gaps as i64),
            packets: pk.map(|k| k.stats.packets as i64),
            files: fl.map(|f| f.stats.files as i64),
            file_errors: st.map(|s| s.stats.file_errors as i64),
            encrypted_files: st.map(|s| s.stats.encrypted as i64),
            image_segments: st.map(|s| s.stats.segments as i64),
            pictures_complete: st.map(|s| s.stats.complete_pictures as i64),
            products: st.map(|s| s.products as i64),
            hint: &self.hint,
            signal_time: Seconds(dv.time_s),
        });
    }
}

/// The link of a stack: HRIT, or the LRIT of `satellite`.
pub fn link_of(hrit: bool, satellite: Fy4Satellite) -> Fy4Link {
    if hrit {
        LINKS[2]
    } else if satellite == Fy4Satellite::Fy4b {
        LINKS[1]
    } else {
        LINKS[0]
    }
}

impl Component for Fy4Display {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::sink(
            "satimg.fy4_display",
            vec![
                Port::typed::<LinkState>("link"),
                Port::typed::<TsReport>("ts"),
                Port::typed::<PacketReport>("packets"),
                Port::typed::<FilesReport>("files"),
                Port::typed::<StoreReport>("store"),
                Port::typed::<ImportReport>("import"),
            ],
        )
        .with_settings(Fy4DisplaySetting::schema())
    }
    fn configure(&mut self, _: &[Format]) -> Result<Vec<Format>, ConfigError> {
        Ok(vec![])
    }
    fn process(&mut self, io: &mut Io) {
        if !io.tick() {
            return;
        }
        let dv = s2::link_of(io, 0).unwrap_or_default();
        let ts = last::<TsReport>(io, 1).map(|t| &t.stats);
        let pk = last::<PacketReport>(io, 2);
        let fl = last::<FilesReport>(io, 3);
        let st = last::<StoreReport>(io, 4);
        let im = last::<ImportReport>(io, 5);
        self.store = st.cloned();
        self.show(&dv, ts, pk, fl, st, im);
    }
    /// The link, the transport stream, the packets, the files and the import are of the carrier of the frequency before;
    /// the store is kept, so its last report is shown again.
    fn forget(&mut self, _: &sdrkit_graph::Kept) {
        let store = self.store.take();
        self.show(&LinkState::default(), None, None, None, store.as_ref(), None);
        self.store = store;
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        match Fy4DisplaySetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            Fy4DisplaySetting::Satellite => self.satellite = SATELLITE_SETTING.read(id, v)?,
            Fy4DisplaySetting::SymbolRate => self.user_rs = LRIT_SYMBOL_RATE.read(id, v)?,
        }
        Ok(Applied::Live)
    }
    fn values(&self) -> Values {
        values_of([
            (
                Fy4DisplaySetting::Satellite,
                Value::Choice(self.satellite.index()),
            ),
            (Fy4DisplaySetting::SymbolRate, Value::Hz(self.user_rs)),
        ])
    }
    fn results(&self) -> Option<&Results> {
        Some(self.published.results())
    }
}

/// The parts of the DVB-S2 receiver that FY-4 takes.
const PARTS: Dvbs2Parts = Dvbs2Parts {
    ts: true,
    field: false,
};

/// The FY-4 graph: the DVB-S2 receiver, the CADUs in its transport stream, frames, packets, files and products.
pub fn builder(hrit: bool) -> GraphBuilder {
    s2::attach(GraphBuilder::new(), PARTS)
        .add("ts", TsComponent::default())
        .add("import", CaduImportComponent::new(None))
        .add("vcdu", VcduComponent::new(Layout::FY4, false))
        .add("files", FilesComponent::new(Dialect::Fy4))
        .add("products", ProductComponent::new(StoreConfig::fy4()))
        .add("display", Fy4Display::new(hrit))
        .connect("s2_bbframe.ts", "ts.in")
        .connect("ts.out", "import.in")
        .connect("import.out", "vcdu.in")
        .connect("vcdu.packets", "files.in")
        .connect("vcdu.vcdu", "files.vcdu")
        .connect("files.out", "products.in")
        .connect("s2_report.link", "display.link")
        .connect("ts.report", "display.ts")
        .connect("vcdu.report", "display.packets")
        .connect("files.report", "display.files")
        .connect("products.report", "display.store")
        .connect("import.report", "display.import")
        .output("products.events")
        .output("products.text")
        .output("products.pictures")
        .status_from("display")
        .clock(REPORT_S)
}

/// The FY-4 receiver: the graph with the settings of the stack.
///
/// # Errors
///
/// The errors of [`GraphBuilder::build`] and of [`sdrkit_graph::StackGraphBuilder::build`], among them a refused setting.
pub fn receiver(hrit: bool, cfg: &StackConfig) -> Result<StackGraph, GraphError> {
    let (id, min) = if hrit {
        (HRIT.id, 1.5e6)
    } else {
        (LRIT.id, 250e3)
    };
    let mut g = builder(hrit)
        .catalog(&Catalog::of(cfg)?)
        .declare(&declaration_of(id))
        .build()?;
    g.set_max_items(1 << 20);
    let mut r = StackGraph::of(crate::typed::shape(id), g)
        .min_rate(min, "")
        .route("filter", &["products.filter"])?
        .route("follow_newest", &["products.follow_newest"])?
        .command(
            "open_cadu",
            sdrkit_graph::CommandTarget::of("import", "open_cadu"),
        )
        .command("clear", sdrkit_graph::CommandTarget::every("clear"));
    for (key, targets) in s2::routes(PARTS) {
        let t: Vec<&str> = match key {
            "symbol_rate" => targets
                .iter()
                .copied()
                .chain(["display.symbol_rate"])
                .collect(),
            _ => targets.to_vec(),
        };
        r = r.route(key, &t)?;
    }
    if !hrit {
        r = r.route("satellite", &["display.satellite"])?;
    }
    r.with_settings(&cfg.settings).build()
}

fn make(hrit: bool, cfg: &StackConfig) -> Result<Box<dyn Receiver>, ConfigError> {
    super::stack_receiver(receiver(hrit, cfg))
}
fn lrit_receiver(cfg: &StackConfig) -> Result<Box<dyn Receiver>, ConfigError> {
    make(false, cfg)
}
fn hrit_receiver(cfg: &StackConfig) -> Result<Box<dyn Receiver>, ConfigError> {
    make(true, cfg)
}

/// FY-4 LRIT.
pub static LRIT: StackDescriptor = StackDescriptor {
    id: "fy4_lrit",
    name: "FY-4 LRIT",
    group: crate::GROUP,
    blurb: "FengYun-4A and 4B LRIT on 1697 MHz (DVB-S2): AGRI channel pictures and messages.",
    tuning: Tuning {
        min_hz: 1670e6,
        max_hz: 1710e6,
        ..Tuning::anywhere(1697.0e6, 2e6, 250e3, 0.3e6)
    },
    submodes: &[],
    settings: lrit_schema,
    make_receiver: lrit_receiver,
    make_generator: Some(lrit_generator),
    lock_profile: LockProfile::DEFAULT,
    signal_types: &[SignalType::Lrit],
    auto_gain: AutoGain::Radio,
    verify: Some(&LRIT_HOOKS),
    benchmarks: None,
};

/// FY-4A HRIT-II/III.
pub static HRIT: StackDescriptor = StackDescriptor {
    id: "fy4a_hrit",
    name: "FY-4A HRIT-II/III",
    group: crate::GROUP,
    blurb: "FengYun-4A HRIT on 1679 MHz (DVB-S2, 1 Msym/s): AGRI channel pictures.",
    tuning: Tuning {
        min_hz: 1670e6,
        max_hz: 1710e6,
        ..Tuning::anywhere(1679.0e6, 4e6, 1.5e6, 1.5e6)
    },
    submodes: &[],
    settings: hrit_schema,
    make_receiver: hrit_receiver,
    make_generator: Some(hrit_generator),
    lock_profile: LockProfile::DEFAULT,
    signal_types: &[SignalType::Hrit],
    auto_gain: AutoGain::Radio,
    verify: Some(&HRIT_HOOKS),
    benchmarks: None,
};

// ---------------------------------------------------------------------------------------------------------------------------------
// The test content: files, frames, CADUs and transport packets, and the test signal.

/// The FY-4 test picture of channel `ch` (96 x 96, 12 bits).
pub fn test_picture(ch: u8) -> Vec<u16> {
    (0..96 * 96)
        .map(|i| {
            ((crate::gen::test_pixel(ch as u16, (i % 96) as u32, (i / 96) as u32, 96) as u16) << 4)
                | (i as u16 & 15)
        })
        .collect()
}

/// The FY-4 test file name of a channel and round.
pub fn test_name(ch: u8, round: u32) -> String {
    let m = round;
    format!("FY4A-_AGRI--_N_DISK_1047E_L1-_FDI-_C{ch:03}_NOM_2026110103{m:02}00_2026110103{m:02}59_4000M_V0001.HDF")
}

/// The image information record (FY-4 dialect, 32 bytes): 12 bits, 96 x 48, no compression.
pub fn image_record(ch: u8, seg: u16, total: u16, first_line: u16, key_bits: u8) -> Vec<u8> {
    let mut b = vec![0u8; 32];
    b[..5].copy_from_slice(b"FY-4A");
    b[9..13].copy_from_slice(b"AGRI");
    b[16] = 12;
    b[17..19].copy_from_slice(&96u16.to_be_bytes());
    b[19..21].copy_from_slice(&48u16.to_be_bytes());
    b[21] = 0;
    b[22] = ch;
    b[23..25].copy_from_slice(&total.to_be_bytes());
    b[25..27].copy_from_slice(&seg.to_be_bytes());
    b[27..29].copy_from_slice(&seg.to_be_bytes());
    b[29..31].copy_from_slice(&first_line.to_be_bytes());
    b[31] = 0x40 | key_bits;
    b
}

/// The test files of one round, as (APID, bytes): C02 and C12 in two segments each (12-bit samples in 2 bytes), and a message.
pub fn test_files(round: u32) -> Vec<(u16, Vec<u8>)> {
    let mut out = Vec::new();
    for ch in [2u8, 12] {
        let pic = test_picture(ch);
        for seg in 0..2u16 {
            let rows = &pic[seg as usize * 48 * 96..(seg as usize + 1) * 48 * 96];
            let data: Vec<u8> = rows.iter().flat_map(|v| v.to_be_bytes()).collect();
            let name = test_name(ch, round);
            let mut nav = vec![b' '; 32];
            nav[..11].copy_from_slice(b"GEOS(104.7)");
            for v in [1.0f32, 1.0, 48.0, 48.0] {
                nav.extend_from_slice(&v.to_bits().to_be_bytes());
            }
            let b = Builder::new(0)
                .record(1, &image_record(ch, seg + 1, 2, seg * 48, 0))
                .record(2, &nav)
                .annotation(&name)
                .time_unix(crate::modes::xrit_gen::TEST_TIME + round as f64 * 60.0);
            let mut f = b.build(data.len());
            f.extend_from_slice(&data);
            out.push((100 + ch as u16, f));
        }
    }
    let text = format!("FY-4 TEST MESSAGE ROUND {round}\r\n");
    let b = Builder::new(1)
        .annotation(&format!("FY4A_MESSAGE_{round}.TXT"))
        .time_unix(crate::modes::xrit_gen::TEST_TIME);
    let mut f = b.build(text.len());
    f.extend_from_slice(text.as_bytes());
    out.push((200, f));
    out
}

/// The CADUs of `rounds` rounds of test files (frames of 1020 bytes on VC 0, spacecraft 0x64, `fill` fill frames between
/// rounds).
pub fn test_cadus(rounds: u32, fill: usize) -> Vec<[u8; CADU_LEN]> {
    pack_cadus((0..rounds).map(test_files).collect(), fill)
}

/// Packs rounds of files, as (APID, bytes), into CADUs.
pub fn pack_cadus(rounds: Vec<Vec<(u16, Vec<u8>)>>, fill: usize) -> Vec<[u8; CADU_LEN]> {
    let layout = Layout::FY4;
    let zl = layout.data_zone_len();
    let mut out = Vec::new();
    let mut counter = 0u32;
    let mut fill_counter = 0u32;
    let mut seq: BTreeMap<u16, u16> = BTreeMap::new();
    let cadu = |v: &[u8]| {
        let mut c = [0u8; CADU_LEN];
        c[..4].copy_from_slice(marker::TM.bytes());
        c[4..].copy_from_slice(&v[..CADU_LEN - 4]);
        c
    };
    for files in rounds {
        for (n, (apid, f)) in files.into_iter().enumerate() {
            let pieces = split_file(&f, n as u16, 4000, &[]);
            let mut zs = ZoneStream::default();
            let k = pieces.len();
            for (i, user) in pieces.iter().enumerate() {
                let flags = match (i == 0, i + 1 == k) {
                    (true, true) => 3,
                    (true, false) => 1,
                    (false, true) => 2,
                    _ => 0,
                };
                let s = seq.entry(apid).or_insert(0);
                zs.push(&space_packet(apid, flags, *s, user, false));
                *s = (*s + 1) & 0x3FFF;
            }
            let r = zs.len() % zl;
            if r != 0 {
                let mut fill = zl - r;
                if fill < 7 {
                    fill += zl;
                }
                zs.push(&space_packet(2047, 3, 0, &vec![0u8; fill - 6], false));
            }
            while !zs.is_empty() {
                let (fhp, zone) = zs.take(zl);
                let v = aos_frame(&layout, 0x64, 0, counter, fhp, &zone);
                counter = (counter + 1) & 0xFF_FFFF;
                out.push(cadu(&v));
            }
        }
        for _ in 0..fill {
            let v = aos_frame(&layout, 0x64, 63, fill_counter, FirstHeader::None, &[]);
            fill_counter = (fill_counter + 1) & 0xFF_FFFF;
            out.push(cadu(&v));
        }
    }
    out
}

/// The transport packets that carry CADUs on `pid`.
pub fn test_ts(cadus: &[[u8; CADU_LEN]], pid: u16) -> Vec<TsPacket> {
    let mut cc = 0u8;
    let mut out = Vec::new();
    for c in cadus {
        for p in ts_packets(c, pid, &mut cc) {
            out.extend(TsPacket::new(Packet::new(Arc::from(&p[..]), 0.0)));
        }
    }
    out
}

/// The PID of the test signal.
pub const TEST_PID: u16 = 0x100;

/// The transport stream of the test signal: rounds of [`test_files`] as CADUs on [`TEST_PID`], 4 fill frames between rounds.
#[derive(Default)]
pub struct Fy4Source {
    round: u32,
    cc: u8,
    queue: VecDeque<[u8; TS_SIZE]>,
}

impl PacketSource for Fy4Source {
    fn next_packet(&mut self, out: &mut [u8; TS_SIZE]) {
        if self.queue.is_empty() {
            for c in pack_cadus(vec![test_files(self.round)], 4) {
                self.queue.extend(ts_packets(&c, TEST_PID, &mut self.cc));
            }
            self.round += 1;
        }
        *out = self
            .queue
            .pop_front()
            .unwrap_or_else(sdrkit_proto_mpegts::source::null_packet);
    }
}

/// The FY-4 test signal: DVB-S2 at the MODCOD of the link, normal frames, no pilots, roll-off 0.35, at the symbol rate of the
/// setting `symbol_rate` (else of the link), carrying [`Fy4Source`].
///
/// # Errors
///
/// [`ConfigError`] if the symbol rate does not fit the sample rate.
pub fn generator(hrit: bool, cfg: &GenConfig) -> Result<s2::S2Transmitter, ConfigError> {
    let s = &cfg.settings;
    let (rate_set, satellite) = if hrit {
        (s.get(&HRIT_SYMBOL_RATE), Fy4Satellite::Fy4a)
    } else {
        (s.get(&LRIT_SYMBOL_RATE), s.get(&SATELLITE_SETTING))
    };
    let (_, _, _, link_rate, _, modcod, _) = link_of(hrit, satellite);
    let rs = if rate_set > 0.0 { rate_set } else { link_rate };
    s2::transmitter(
        cfg.rate_hz,
        rs,
        0.35,
        modcod,
        false,
        Box::new(Fy4Source::default()),
        None,
    )
}

fn lrit_generator(cfg: &GenConfig) -> Result<Box<dyn Generator>, ConfigError> {
    Ok(Box::new(generator(false, cfg)?))
}
fn hrit_generator(cfg: &GenConfig) -> Result<Box<dyn Generator>, ConfigError> {
    Ok(Box::new(generator(true, cfg)?))
}

// ---------------------------------------------------------------------------------------------------------------------------------
// Declarations and verification.

static LRIT_HOOKS: sdrkit_core::VerifyHooks = sdrkit_core::VerifyHooks {
    cases: |o| {
        crate::verify::case(o, "test signal, FY-4A", &[], 12.0, |r| {
            vec![crate::verify::count_at_least(r, &Fy4Fields::CADUS, 5)]
        })
    },
    vector_settings: None,
    check_vector: None,
};

static HRIT_HOOKS: sdrkit_core::VerifyHooks = sdrkit_core::VerifyHooks {
    cases: |o| {
        crate::verify::case(o, "test signal", &[], 3.0, |r| {
            vec![
                crate::verify::count_at_least(r, &Fy4Fields::CADUS, 20),
                crate::verify::count_at_least(r, &Fy4Fields::FILES, 1),
            ]
        })
    },
    vector_settings: None,
    check_vector: None,
};
