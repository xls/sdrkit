//! GK-2A UHRIT (`gk2a_uhrit`): 2048-byte CADUs in the data fields of a DVB-S2 generic stream, AOS frames with a CRC-16,
//! space packets whose 8-byte secondary header the length field does not count, a CRC-32 on each packet, and xRIT files that
//! go to the GK-2A file layer (pictures, keys and texts as for `gk2a_hrit`).
//!
//! No public document was read for this link: the symbol rate, the frame CRC, the length rule of the secondary header, the
//! packet CRC and the file framing are not confirmed by a public document. The DVB-S2, AOS and space packet layers follow
//! their standards, and the xRIT headers the KMA dialect.
//!
//! Standard: ETSI EN 302 307-1 (DVB-S2); CCSDS 131.0-B (marker), 732.0-B (AOS), 133.0-B (space packets); KMA LRIT/HRIT
//! mission specification (xRIT headers).

use super::s2::{
    self, attach, cadu_state, crc_packet, frame_fields, link_of, routes,
    CaduStream, Dvbs2Parts, FieldFrames, FieldFramesComponent, FieldFramesReport, LinkState,
    CARRIER_OFFSET, FRAME_LENGTH, MODCOD, MODCODS, MODCOD_RULE, PILOTS_RULE, PL_SCRAMBLING,
    ZONE_BYTES,
};
use crate::components::last;
use crate::components::xrit::{
    ProductComponent, StoreReport, FILTER, FOLLOW_NEWEST,
};
use crate::gen::ZoneStream;
use crate::items::{WholePacket, XritFile};
use crate::typed::{
    CaduFields, UhritApidRow, UhritFields, UhritFrameRow, UHRIT_FRAMES, UHRIT_PACKETS_BY_APID,
};
use sdrkit_core::interface::Fields;
use crate::xrit::store::StoreConfig;
use crate::REPORT_S;
use sdrkit_core::publish::{field, Published};
use sdrkit_core::{
    AutoGain, ConfigError, GenConfig, Generator, LockProfile, Receiver, Results, ResultsDecl,
    Setting, SettingError, SettingId, SettingSet, StackConfig, StackDescriptor, Tuning, Value,
    Values,
};
use sdrkit_graph::{
    no_values, values_of, Applied, Catalog, Component, ComponentInfo, Format, GraphBuilder,
    GraphError, Io, Item, Kind, Port, RateFrom, StackGraph, Timed,
};
use sdrkit_proto_ccsds::xrit::headers::{parse, Builder, Dialect};
use sdrkit_proto_dvbs2::settings::RollOffChoice;
use sdrkit_util_bits::crc::catalog::CRC32_ISO_HDLC;
use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::sync::Arc;

/// The stack id.
pub const ID: &str = "gk2a_uhrit";
/// The symbol rate of the link (not confirmed by a public document).
pub const LINK_SYMBOL_RATE: f64 = 15_622_440.0;
/// The input rate the stack asks for.
pub const ASKED_RATE: f64 = 30e6;
/// Bytes of the packet secondary header; by default the length field does not count them (not confirmed by a public document).
pub const SECONDARY: u16 = 8;
/// The secondary header bytes outside the packet length field, as a setting.
pub const UNCOUNTED: Setting<field::I64> =
    Setting::new("uncounted_secondary", "Secondary header outside the length", "bytes", SECONDARY as i64)
        .range(0.0, 64.0, 1.0)
        .help("A packet with its secondary header flag set is this much longer than its length field says.")
        .advanced();
/// Bytes of the transport header before each xRIT file.
pub const TRANSPORT: usize = 10;
/// The shortest packet read: headers, CRC and one byte.
pub const MIN_PACKET: usize = 6 + SECONDARY as usize + 4 + 1;

// ---------------------------------------------------------------------------------------------------------------------------------
// Files.

/// One file of [`UhritDecoder`].
#[derive(Clone, Debug, PartialEq)]
pub struct UhritFile {
    /// Virtual channel.
    pub vcid: u8,
    /// APID.
    pub apid: u16,
    /// The xRIT file (primary header first).
    pub bytes: Vec<u8>,
    /// Where each packet's user data starts in `bytes`.
    pub packet_starts: Vec<u32>,
    /// The name: the annotation record, cleaned, else "vc<V>_apid<A>_<n>.lrit".
    pub name: String,
    /// A new first packet ended it before its last packet.
    pub truncated: bool,
}

/// Counters of [`UhritDecoder`].
#[derive(Clone, Debug, Default, PartialEq)]
pub struct UhritStats {
    /// Packets taken.
    pub packets: u64,
    /// Packets with a bad CRC-32.
    pub crc_bad: u64,
    /// Files given out.
    pub files: u64,
    /// Files cut short by a new first packet.
    pub truncated: u64,
    /// Packets of each APID.
    pub by_apid: BTreeMap<u16, u64>,
    /// The name of the last file.
    pub last_name: String,
}

#[derive(Clone, Debug, Default)]
struct Building {
    data: Vec<u8>,
    starts: Vec<u32>,
}

/// The packet and file layer of UHRIT, without the graph. See the [module documentation](self).
#[derive(Clone, Debug, Default)]
pub struct UhritDecoder {
    files: BTreeMap<(u8, u16), Building>,
    /// The counters.
    pub stats: UhritStats,
}

/// A name from an annotation: / \ : and control characters become "_".
pub fn clean_annotation(a: &str) -> String {
    a.chars()
        .map(|c| {
            if c.is_control() || matches!(c, '/' | '\\' | ':') {
                '_'
            } else {
                c
            }
        })
        .collect()
}

impl UhritDecoder {
    /// Takes one whole packet of virtual channel `vc` and calls `out` with each file it finishes.
    pub fn push(&mut self, vc: u8, p: &[u8], out: &mut impl FnMut(UhritFile)) {
        if p.len() < MIN_PACKET {
            return;
        }
        let apid = u16::from_be_bytes([p[0], p[1]]) & 0x7FF;
        let flags = p[2] >> 6;
        self.stats.packets += 1;
        *self.stats.by_apid.entry(apid).or_default() += 1;
        let n = p.len();
        let key = (vc, apid);
        let crc = u32::from_be_bytes([p[n - 4], p[n - 3], p[n - 2], p[n - 1]]);
        if CRC32_ISO_HDLC.checksum(&p[..n - 4]) != crc {
            self.stats.crc_bad += 1;
            self.files.remove(&key);
            return;
        }
        let user = &p[6 + usize::from(SECONDARY)..n - 4];
        if flags == 0b01 || flags == 0b11 {
            if let Some(b) = self.files.remove(&key) {
                self.stats.truncated += 1;
                self.finish(key, b, true, out);
            }
            self.files.insert(key, Building::default());
        }
        let Some(b) = self.files.get_mut(&key) else {
            return;
        };
        b.starts.push(b.data.len() as u32);
        b.data.extend_from_slice(user);
        if flags == 0b10 || flags == 0b11 {
            if let Some(b) = self.files.remove(&key) {
                self.finish(key, b, false, out);
            }
        }
    }

    fn finish(
        &mut self,
        (vcid, apid): (u8, u16),
        b: Building,
        truncated: bool,
        out: &mut impl FnMut(UhritFile),
    ) {
        if b.data.len() <= TRANSPORT {
            return;
        }
        let bytes = b.data[TRANSPORT..].to_vec();
        let packet_starts = b
            .starts
            .iter()
            .map(|&s| s.saturating_sub(TRANSPORT as u32))
            .collect();
        self.stats.files += 1;
        let name = parse(&bytes, Dialect::Gk2a)
            .ok()
            .and_then(|h| h.annotation)
            .map(|a| clean_annotation(a.trim_end_matches('\0')))
            .filter(|a| !a.is_empty())
            .unwrap_or_else(|| format!("vc{vcid}_apid{apid}_{}.lrit", self.stats.files));
        self.stats.last_name.clone_from(&name);
        out(UhritFile {
            vcid,
            apid,
            bytes,
            packet_starts,
            name,
            truncated,
        });
    }
}

// ---------------------------------------------------------------------------------------------------------------------------------
// The components.

/// The report of [`UhritComponent`].
#[derive(Clone, Debug, Default, PartialEq)]
pub struct UhritReport {
    /// The counters.
    pub stats: UhritStats,
    /// Files whose headers did not read.
    pub damaged: u64,
}

impl Item for UhritReport {
    const KIND: &'static str = "satimg.uhrit_report";
}

/// Whole packets in; xRIT files for the GK-2A file layer and the report out.
#[derive(Default)]
pub struct UhritComponent {
    dec: UhritDecoder,
    damaged: u64,
}

impl Component for UhritComponent {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "satimg.uhrit",
            vec![Port::typed::<WholePacket>("in")],
            vec![
                Port::typed::<XritFile>("files").rate_from(RateFrom::Fixed(0.0)),
                Port::typed::<UhritReport>("report").rate_from(RateFrom::Fixed(0.0)),
            ],
        )
    }
    /// The files in work and the counts are of the carrier of the frequency before.
    fn forget(&mut self, _: &sdrkit_graph::Kept) {
        *self = UhritComponent::default();
    }
    fn configure(&mut self, _: &[Format]) -> Result<Vec<Format>, ConfigError> {
        Ok(vec![
            Format::new(Kind::typed::<XritFile>(), 0.0),
            Format::new(Kind::typed::<UhritReport>(), 0.0),
        ])
    }
    fn process(&mut self, io: &mut Io) {
        let mut files = Vec::new();
        let mut damaged = 0;
        for p in io.items::<WholePacket>(0) {
            let time = p.value.time;
            self.dec.push(p.value.vcid, &p.value.bytes, &mut |f| {
                let Ok(headers) = parse(&f.bytes, Dialect::Gk2a) else {
                    damaged += 1;
                    return;
                };
                let n = f.packet_starts.len() as u32;
                files.push(Timed {
                    time,
                    value: XritFile {
                        vcid: f.vcid,
                        apid: f.apid,
                        bytes: Arc::from(f.bytes),
                        packet_starts: f.packet_starts,
                        packet_index: (0..n).collect(),
                        headers,
                        errors: f.truncated,
                        truncated: f.truncated,
                        time,
                    },
                });
            });
        }
        self.damaged += damaged;
        io.items_out::<XritFile>(0).extend(files);
        if io.tick() {
            io.items_out::<UhritReport>(1).push(Timed {
                time: 0.0,
                value: UhritReport {
                    stats: self.dec.stats.clone(),
                    damaged: self.damaged,
                },
            });
        }
    }
    fn apply(&mut self, id: SettingId, _: &Value) -> Result<Applied, SettingError> {
        Err(SettingError::NoSuchId { id })
    }
    fn values(&self) -> Values {
        no_values()
    }
}

// ---------------------------------------------------------------------------------------------------------------------------------
// Settings.

/// The symbol rate (the default is the UHRIT rate, not confirmed by a public document).
pub const SYMBOL_RATE: Setting<field::Hz> = s2::symbol_rate(LINK_SYMBOL_RATE);
/// The roll-off.
pub const ROLL_OFF: Setting<field::Choice<RollOffChoice>> = s2::roll_off(RollOffChoice::R25);

sdrkit_core::settings! {
    /// The settings of `gk2a_uhrit`.
    pub enum UhritSetting {
        /// The symbol rate.
        SymbolRate = SYMBOL_RATE,
        /// The roll-off.
        RollOff = ROLL_OFF,
        /// The carrier.
        CarrierOffset = CARRIER_OFFSET,
        /// The secondary header bytes outside the packet length field.
        UncountedSecondary = UNCOUNTED,
        /// Which products the table lists.
        Filter = FILTER,
        /// Show the newest product.
        FollowNewest = FOLLOW_NEWEST,
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
    }
}

sdrkit_core::settings! {
    /// The settings of [`UhritDisplay`].
    pub enum UhritDisplaySetting {
        /// The symbol rate.
        SymbolRate = SYMBOL_RATE,
    }
}

// ---------------------------------------------------------------------------------------------------------------------------------
// The display.

fn display_declaration() -> ResultsDecl {
    ResultsDecl::new()
        .with_fields(CaduFields::SHAPE.fields)
        .with_fields(UhritFields::SHAPE.fields)
        .with_table(UHRIT_PACKETS_BY_APID.def())
        .with_table(UHRIT_FRAMES.def())
}

/// What `gk2a_uhrit` publishes.
pub fn declaration() -> ResultsDecl {
    crate::typed::shape(ID).results_decl()
}

const NOTE_TEXT: &str = "Experimental: not confirmed on a real signal. No public document was read for this link. Not \
    confirmed by a public document: the symbol rate 15 622 440 sym/s and roll-off 0.25, the CADUs of 2048 bytes in the data \
    field, the CRC-16 of each frame, the 8-byte secondary header that the packet length field does not count, the CRC-32 \
    of each packet and the 10-byte header before each file.";

/// The display of `gk2a_uhrit`.
pub struct UhritDisplay {
    published: Published,
    symbol_rate: f64,
    summary: String,
    hint: String,
    text: String,
    key: String,
    frames: FieldFramesReport,
}

impl Default for UhritDisplay {
    fn default() -> Self {
        UhritDisplay {
            published: super::display::published(display_declaration()),
            symbol_rate: LINK_SYMBOL_RATE,
            summary: String::new(),
            hint: String::new(),
            text: String::new(),
            key: String::new(),
            frames: FieldFramesReport::default(),
        }
    }
}

impl Component for UhritDisplay {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::sink(
            "satimg.uhrit_display",
            vec![
                Port::typed::<LinkState>("link"),
                Port::typed::<FieldFramesReport>("frames"),
                Port::typed::<UhritReport>("uhrit"),
                Port::typed::<StoreReport>("store"),
            ],
        )
        .with_settings(UhritDisplaySetting::schema())
    }
    fn configure(&mut self, _: &[Format]) -> Result<Vec<Format>, ConfigError> {
        Ok(vec![])
    }
    fn process(&mut self, io: &mut Io) {
        if !io.tick() {
            return;
        }
        let link = link_of(io, 0).unwrap_or_default();
        let fr = last::<FieldFramesReport>(io, 1);
        let u = last::<UhritReport>(io, 2).cloned().unwrap_or_default();
        let st = last::<StoreReport>(io, 3);
        let state = cadu_state(&link, fr);
        let p = &mut self.published;
        if let Some(st) = st {
            super::display::log_lines(p.results_mut(), &st.log);
        }
        if let Some(fr) = fr {
            self.frames.clone_from(fr);
            for (vc, &n) in fr.per_vc.iter().enumerate().filter(|(_, n)| **n > 0) {
                self.key.clear();
                let _ = write!(self.key, "{vc}");
                let row = UhritFrameRow {
                    vcid: vc as i64,
                    frames: n as i64,
                };
                UHRIT_FRAMES.put(p, &self.key, &row);
            }
        }
        p.put(&frame_fields::set(&self.frames, &mut self.text));
        let mut top: Vec<(&u16, &u64)> = u.stats.by_apid.iter().collect();
        top.sort_by(|a, b| b.1.cmp(a.1));
        for (apid, n) in top.into_iter().take(32) {
            self.key.clear();
            let _ = write!(self.key, "{apid}");
            let row = UhritApidRow {
                apid: i64::from(*apid),
                packets: *n as i64,
            };
            UHRIT_PACKETS_BY_APID.put(p, &self.key, &row);
        }
        self.text.clear();
        if let Some(m) = link.modcod.filter(|_| link.state >= 2) {
            let _ = write!(self.text, "MODCOD {m}");
        }
        s2::hint(&link, self.symbol_rate, &self.text, &mut self.hint);
        p.put(&UhritFields {
            frames_crc_bad: self.frames.crc_bad as i64,
            packets_crc_bad: u.stats.crc_bad as i64,
            files: u.stats.files as i64,
            last_file: u.stats.last_name.as_str(),
            products: st.map(|s| s.products as i64),
            hint: &self.hint,
            note: NOTE_TEXT,
        });
        self.summary.clear();
        let frames = fr.map_or(0, |f| f.cadus);
        let _ = match state {
            3 => write!(
                self.summary,
                "GK-2A UHRIT, {frames} frames, {} packets, {} files",
                u.stats.packets, u.stats.files
            ),
            1 => write!(self.summary, "GK-2A UHRIT, carrier found, no frames yet"),
            _ => write!(self.summary, "GK-2A UHRIT, searching"),
        };
        s2::write_status(p, &link, state, &self.summary);
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        match UhritDisplaySetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            UhritDisplaySetting::SymbolRate => self.symbol_rate = SYMBOL_RATE.read(id, v)?,
        }
        Ok(Applied::Live)
    }
    fn values(&self) -> Values {
        values_of([(UhritDisplaySetting::SymbolRate, Value::Hz(self.symbol_rate))])
    }
    /// The frames and the packets counted are of the carrier of the frequency before.
    fn forget(&mut self, _: &sdrkit_graph::Kept) {
        self.frames = FieldFramesReport::default();
        self.published.clear_rows(UHRIT_FRAMES.def());
        self.published.clear_rows(UHRIT_PACKETS_BY_APID.def());
    }
    fn results(&self) -> Option<&Results> {
        Some(self.published.results())
    }
}

// ---------------------------------------------------------------------------------------------------------------------------------
// The graph and the descriptor.

const PARTS: Dvbs2Parts = Dvbs2Parts {
    ts: false,
    field: true,
};

/// The UHRIT graph.
pub fn builder() -> GraphBuilder {
    attach(GraphBuilder::new(), PARTS)
        .add(
            "frames",
            FieldFramesComponent::new(FieldFrames {
                frame_crc: true,
                channels: !(1u64 << 63),
                uncounted_secondary: SECONDARY,
            }),
        )
        .add("uhrit", UhritComponent::default())
        .add("products", ProductComponent::new(StoreConfig::gk2a("UHRIT")))
        .add("display", UhritDisplay::default())
        .connect("s2_fec.field", "frames.in")
        .connect("frames.packets", "uhrit.in")
        .connect("uhrit.files", "products.in")
        .connect("s2_report.link", "display.link")
        .connect("frames.report", "display.frames")
        .connect("uhrit.report", "display.uhrit")
        .connect("products.report", "display.store")
        .output("products.events")
        .output("products.text")
        .output("products.pictures")
        .status_from("display")
        .clock(REPORT_S)
}

/// The UHRIT receiver with the settings of `cfg`.
///
/// # Errors
///
/// The errors of [`GraphBuilder::build`] and of [`sdrkit_graph::StackGraphBuilder::build`], among them a refused setting.
pub fn receiver(cfg: &StackConfig) -> Result<StackGraph, GraphError> {
    let mut g = builder()
        .catalog(&Catalog::of(cfg)?)
        .declare(&declaration())
        .build()?;
    g.set_max_items(1 << 20);
    let mut b = StackGraph::of(crate::typed::shape(ID), g)
        .min_rate(2e6, "GK-2A UHRIT needs more samples")
        .command(
            "import_keys",
            sdrkit_graph::CommandTarget::of("products", "import_keys"),
        )
        .command(
            "add_keys",
            sdrkit_graph::CommandTarget::of("products", "add_keys"),
        )
        .command("clear", sdrkit_graph::CommandTarget::every("clear"));
    for (key, targets) in routes(PARTS) {
        let t: Vec<&str> = match key {
            "symbol_rate" => targets
                .iter()
                .copied()
                .chain(["display.symbol_rate"])
                .collect(),
            _ => targets.to_vec(),
        };
        b = b.route(key, &t)?;
    }
    b.route("filter", &["products.filter"])?
        .route("uncounted_secondary", &["frames.uncounted_secondary"])?
        .route("follow_newest", &["products.follow_newest"])?
        .with_settings(&cfg.settings)
        .build()
}

fn make_receiver(cfg: &StackConfig) -> Result<Box<dyn Receiver>, ConfigError> {
    super::stack_receiver(receiver(cfg))
}

fn make_generator(cfg: &GenConfig) -> Result<Box<dyn Generator>, ConfigError> {
    Ok(Box::new(generator(cfg)?))
}

fn schema() -> Arc<sdrkit_core::Schema> {
    UhritSetting::schema()
}

/// GK-2A UHRIT.
pub static DESCRIPTOR: StackDescriptor = StackDescriptor {
    id: ID,
    name: "GK-2A UHRIT",
    group: crate::GROUP,
    blurb: "GEO-KOMPSAT-2A ultra high rate xRIT on X band (DVB-S2, 15.6 Msym/s): full-disk pictures, keys and texts.",
    tuning: Tuning {
        min_hz: 950e6,
        max_hz: 9000e6,
        ..Tuning::anywhere(8070e6, ASKED_RATE, 26.9e6, 1.5 * LINK_SYMBOL_RATE + 20e3)
    },
    submodes: &[],
    settings: schema,
    make_receiver,
    make_generator: Some(make_generator),
    lock_profile: LockProfile::DEFAULT,
    signal_types: &[],
    auto_gain: AutoGain::Radio,
    verify: Some(&crate::verify::UHRIT_HOOKS),
    benchmarks: None,
};

// ---------------------------------------------------------------------------------------------------------------------------------
// The test signal.

/// The spacecraft id of the test signal.
pub const TEST_SPACECRAFT: u8 = 0x12;
/// The virtual channel of the test signal.
pub const TEST_VC: u8 = 7;
/// The APID of the test files.
pub const TEST_APID: u16 = 0x020;
/// The data bytes of a test file.
pub const TEST_DATA: usize = 6000;
/// The largest packet data of the test signal.
pub const TEST_MAX_DATA: usize = 3000;

/// The xRIT test file of `step`: a primary header and the annotation `name`, then `data`.
pub fn test_file(name: &str, data: &[u8]) -> Vec<u8> {
    let mut f = Builder::new(0).annotation(name).build(data.len());
    f.extend_from_slice(data);
    f
}

/// The packets of one xRIT file on `apid`: a 10-byte transport header of zeros, then the file, cut at `max` bytes, with the
/// 8-byte secondary header outside the length field and a CRC-32 on each packet.
pub fn file_packets(apid: u16, file: &[u8], max: usize, seq: &mut u16) -> Vec<Vec<u8>> {
    let mut all = vec![0u8; TRANSPORT];
    all.extend_from_slice(file);
    let chunks: Vec<&[u8]> = all.chunks(max.max(1)).collect();
    let k = chunks.len();
    chunks
        .iter()
        .enumerate()
        .map(|(i, c)| {
            let flags = match (i == 0, i + 1 == k) {
                (true, true) => 3,
                (true, false) => 1,
                (false, true) => 2,
                _ => 0,
            };
            let p = crc_packet(apid, flags, *seq, &[0; 8], c, false);
            *seq = (*seq + 1) & 0x3FFF;
            p
        })
        .collect()
}

/// The CADUs of the test signal, one by one: spacecraft 12 hex, virtual channel 7, one file "IMG_TEST" of 6000 data bytes a
/// step (byte i = (37 i + step) mod 256).
#[derive(Default)]
pub struct TestCadus {
    zs: ZoneStream,
    step: u32,
    seq: u16,
    counter: u32,
}

impl TestCadus {
    /// Appends the next CADU to `out`.
    pub fn next_cadu(&mut self, out: &mut Vec<u8>) {
        while self.zs.len() < ZONE_BYTES {
            let step = self.step;
            let data: Vec<u8> = (0..TEST_DATA)
                .map(|i| ((37 * i as u32 + step) % 256) as u8)
                .collect();
            let f = test_file("IMG_TEST", &data);
            for p in file_packets(TEST_APID, &f, TEST_MAX_DATA, &mut self.seq) {
                self.zs.push(&p);
            }
            self.step += 1;
        }
        let (fh, zone) = self.zs.take(ZONE_BYTES);
        out.extend(s2::cadu_2048(TEST_SPACECRAFT, TEST_VC, self.counter, fh, &zone));
        self.counter = (self.counter + 1) & 0xFF_FFFF;
    }
}

/// The UHRIT test signal: 8PSK 2/3 (MODCOD 13), normal frames, pilots, roll-off 0.25, a generic continuous stream of
/// [`TestCadus`] at the symbol rate of the setting `symbol_rate`.
///
/// # Errors
///
/// [`ConfigError`] if the symbol rate does not fit the sample rate.
pub fn generator(cfg: &GenConfig) -> Result<s2::S2Transmitter, ConfigError> {
    let rs = match cfg.settings.get(&SYMBOL_RATE) {
        r if r > 0.0 => r,
        _ => LINK_SYMBOL_RATE,
    };
    let mut cadus = TestCadus::default();
    s2::transmitter(
        cfg.rate_hz,
        rs,
        0.25,
        13,
        true,
        Box::new(sdrkit_proto_mpegts::source::NullSource),
        Some(Box::new(CaduStream::new(move |out: &mut Vec<u8>| {
            cadus.next_cadu(out)
        }))),
    )
}
