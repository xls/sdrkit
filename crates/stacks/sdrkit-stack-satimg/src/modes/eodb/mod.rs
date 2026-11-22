//! Direct-broadcast and dump downlinks of polar weather and earth-observation satellites (`eodb`, 20 submodes, one a
//! profile): one CCSDS-style link receiver driven by the profile's constants (modulation, symbol rate, K = 7 code, NRZ-M,
//! Reed-Solomon code, randomiser, frame, header, packet zone), the packets of one virtual channel to an instrument decoder,
//! 16-bit channel pictures and colour products. Three profiles without CADUs (Oceansat-2, Kanopus-V, DMSP) have their own
//! frame stages on the symbols. A gate lets most of the input go by while no carrier is found. Frame files of CADUs are an
//! input too: the LDPC (Landsat X band) and turbo (STEREO) profiles take frames from frame files only.
//!
//! No public document confirms the profile constants, the instrument packet layouts or the frame formats of Oceansat-2,
//! Kanopus-V and DMSP; the satellites, their downlinks and spacecraft ids, and the AVHRR calibration coefficients are data
//! files.
//!
//! Standard: CCSDS 131.0-B-5 (marker, K = 7 code, randomiser, Reed-Solomon); CCSDS 732.0-B-4 and 132.0-B-3 (frame headers);
//! CCSDS 133.0-B-2 (space packets); CCSDS 121.0-B-3 (Rice, VIIRS).

pub mod gen;
pub mod profiles;

use self::profiles::{profile, Conv, Diff, Header, Instrument, Profile, Rand};
use super::display::{log_lines, published, status, with_series, Histories};
use super::tables::{frame_table_fields, publish_tables};
use crate::typed::{
    EodbFields, EodbProductArgs, FileArgs, FrameTableFields, FRAMES, MAKE_PRODUCT, OPEN_FRAMES,
    VIRTUAL_CHANNELS,
};
use crate::components::gated::{GateReport, GateRules, GatedDemodComponent};
use crate::components::link::{item_format, item_port, DemodReport, FrontEndComponent, FrontReport};
use crate::components::table::{header_ids, FrameBytes, FrameRow, FrameTableComponent, Line, TableLink, TableReport};
use crate::components::{last, Control};
use crate::instruments::avhrr_dump::{AvhrrDump, CalTable};
use crate::instruments::direct::{PacketIn, PacketInstrument};
use crate::instruments::modis::Modis;
use crate::instruments::planes::Planes;
use crate::instruments::products::{self, ProductDef, Thermal};
use crate::instruments::small::{Amsr2, Metimage, Tirs, Vegetation};
use crate::instruments::symbols::{Kanopus, Ocm, Ols, SymbolInstrument, SymbolStats};
use crate::instruments::viirs::Viirs;
use crate::psk::{LinkClass, LinkParams, Modulation, Search, SnrEstimator};
use sdrkit_codec_image::component::{Layout, NamedView, Pictures};
use sdrkit_codec_image::raster::Raster;
use sdrkit_codec_image::strip::{Strip, View};
use sdrkit_core::command::{Command, CommandError, CommandId, CommandSet, Commands};
use sdrkit_core::media::Picture;
use sdrkit_core::interface::{Fields as _, Hertz};
use sdrkit_core::publish::{field, Published};
use sdrkit_core::{
    AutoGain, ConfigError, GenConfig, Generator, LockProfile, Receiver, Record, Results, ResultsDecl, Setting, SettingError,
    SettingId, SettingSet, StackConfig, StackDescriptor, Storage, SubMode, Tuning, Value, Values,
};
use sdrkit_graph::{
    Applied, Catalog, Component, ComponentInfo, Format, GraphBuilder, GraphError, Io, Item, Port, RunContext, StackGraph,
    Timed,
};
use sdrkit_proto_ccsds::codesync::CodeConfig;
use sdrkit_proto_ccsds::packet::{Assembler, AssemblerConfig};
use sdrkit_proto_ccsds::tm::FirstHeader;
use sdrkit_proto_ccsds::wordsync::WordSyncConfig;
use sdrkit_util_objects::datafile::DataFile;
use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::sync::Arc;

/// The stack id.
pub const ID: &str = "eodb";
/// Seconds of signal between reports.
pub const REPORT_S: f64 = 0.1;
/// Seconds without a good frame after which a running demodulator searches the carrier again.
pub const RESEARCH_S: f64 = 8.0;
/// The most rows a channel picture keeps.
pub const MAX_ROWS: usize = 20_000;

// ---------------------------------------------------------------------------------------------------------------------------------
// Settings.

/// A symbol rate in place of the profile's (0: the profile's).
pub const SYMBOL_RATE: Setting<field::Hz> = Setting::new("symbol_rate", "Symbol rate", "Hz", 0.0)
    .range(0.0, 400e6, 1.0)
    .help("0 takes the symbol rate of the profile, which no public document confirms.")
    .restart();
/// A roll-off in place of the profile's (0: the profile's).
pub const ROLL_OFF: Setting<field::F64> = Setting::new("roll_off", "Matched filter roll-off", "", 0.0)
    .range(0.0, 1.0, 0.01)
    .help("0 takes the roll-off of the profile, which no public document confirms.")
    .restart();
/// The satellite list: a file in the host's storage.
pub const SATELLITES: Setting<field::Text> =
    Setting::new("satellites", "Satellite list", "", "links/xband_downlinks.csv").help(
        "A file in the host's storage with the satellites of each link and their spacecraft ids \
         (data/links/xband_downlinks.csv). Empty: no satellite names.",
    );
/// The AVHRR calibration coefficients: a file in the host's storage.
pub const AVHRR_CALIBRATION: Setting<field::Text> =
    Setting::new("avhrr_calibration", "AVHRR calibration", "", "satimg/avhrr_calibration.csv").help(
        "A file in the host's storage with the AVHRR calibration coefficients of each satellite \
         (data/satimg/avhrr_calibration.csv). Empty: no temperature products.",
    );
/// The phase ambiguity of the test signal.
pub const GEN_AMBIGUITY: Setting<field::I64> = Setting::new("gen_ambiguity", "Test signal ambiguity", "", 0)
    .range(0.0, 7.0, 1.0)
    .help("Turns the test signal by 90 degrees this many times (modulo 4), mirrored from 4 on.");

sdrkit_core::settings! {
    /// The settings of `eodb`.
    pub enum EodbSetting {
        /// The symbol rate.
        SymbolRate = SYMBOL_RATE,
        /// The roll-off.
        RollOff = ROLL_OFF,
        /// The satellite list.
        Satellites = SATELLITES,
        /// The AVHRR calibration.
        AvhrrCalibration = AVHRR_CALIBRATION,
        /// The ambiguity of the test signal.
        GenAmbiguity = GEN_AMBIGUITY,
    }
}

/// The symbol rate and roll-off of `p` with the overrides of `v`.
pub fn rates(p: &Profile, v: &Values) -> (f64, f64) {
    let rs = v.get(&SYMBOL_RATE);
    let a = v.get(&ROLL_OFF);
    (
        if rs > 0.0 { rs } else { p.symbol_rate },
        if a > 0.0 { a } else { p.roll_off },
    )
}

/// The demodulator's link of `p`.
pub fn link(p: &Profile, v: &Values) -> LinkParams {
    let (symbol_rate, roll_off) = rates(p, v);
    let modulation = if p.instrument == Instrument::Ols {
        Modulation::Bpsk
    } else {
        p.modulation
    };
    LinkParams {
        modulation,
        symbol_rate,
        roll_off,
        class: LinkClass::Leo,
        search: Search::PowerLine {
            power: if modulation == Modulation::Bpsk { 2 } else { 4 },
            min_ratio: 6.0,
        },
        snr: SnrEstimator::Decisions,
        bpsk_plus_is_zero: false,
        constellation: 256,
        interleaved: false,
    }
}

/// The gate: pieces of 131 072 samples through the matched filter, open at a line 16 times over the mean within 1.5 MHz,
/// 7 pieces let go by after noise.
pub const GATE: GateRules = GateRules {
    piece: 131_072,
    open_ratio: 16.0,
    matched: true,
    skip: 7,
    span_hz: 1.5e6,
};

/// The frame rules of a CADU profile that comes from samples: the line of the profile, the marker with 3 wrong bits to
/// acquire and 10 to track (2 to acquire on uncoded QPSK), either polarity, and the frame stage. `None` for the other
/// profiles.
pub fn table(p: &Profile, symbol_rate: f64) -> Option<TableLink> {
    if !p.has_cadus() || !p.from_samples() {
        return None;
    }
    let frame_bits = (8 * p.frame) as u32;
    let mut acquire = 3;
    let line = match (p.conv, p.modulation, p.diff) {
        (Conv::Ccsds | Conv::Swapped, ..) => Line::Coded(CodeConfig {
            frame_bits,
            search_errors: 10,
            track_errors: 16,
            max_misses: 4,
            nrzm: p.diff == Diff::NrzM,
        }),
        (_, Modulation::Qpsk, Diff::None) => {
            acquire = 2;
            Line::Quad
        }
        (_, Modulation::Qpsk, _) => Line::QuadDiff,
        (_, m, Diff::PerRail) => Line::Rails {
            per_rail: true,
            quad: m != Modulation::Bpsk,
        },
        (_, m, _) => Line::Rails {
            per_rail: false,
            quad: m != Modulation::Bpsk,
        },
    };
    Some(TableLink {
        sync: WordSyncConfig {
            word: 0x1ACF_FC1D,
            word_bits: 32,
            frame_bits: 8 * p.frame,
            acquire_errors: acquire,
            track_errors: 10,
            complement: true,
            max_misses: 3,
        },
        line,
        code: Some(gen::channel(p)),
        data_randomised: p.rand == Rand::After,
        invert: p.invert,
        retry_plain: p.retry_plain,
        llr_rate: symbol_rate * p.bits_per_symbol() as f64,
    })
}

// ---------------------------------------------------------------------------------------------------------------------------------
// The satellite list.

/// One satellite of the list.
#[derive(Clone, Debug, PartialEq)]
pub struct Satellite {
    /// The submode of its link.
    pub submode: String,
    /// The name.
    pub name: String,
    /// The spacecraft id, if the list gives one.
    pub spacecraft: Option<u16>,
    /// The downlink, MHz.
    pub centre_mhz: f64,
}

/// Reads the satellite list: `#` lines, the header `band,submode,name,centre_mhz,width_mhz,symbol_rate,spacecraft,note`,
/// then a row for each satellite; rows of other stacks are skipped.
///
/// # Errors
///
/// A header with other columns.
pub fn parse_satellites(text: &str) -> Result<Vec<Satellite>, String> {
    let mut lines = text.lines().filter(|l| !l.starts_with('#') && !l.trim().is_empty());
    let head = lines.next().ok_or("no header")?;
    if !head.starts_with("band,submode,name,centre_mhz,width_mhz,symbol_rate,spacecraft") {
        return Err(head.to_string());
    }
    Ok(lines
        .filter_map(|l| {
            let f: Vec<&str> = l.split(',').collect();
            (f.len() >= 7 && f[0] == ID).then(|| Satellite {
                submode: f[1].to_string(),
                name: f[2].to_string(),
                spacecraft: f[6].trim().parse().ok(),
                centre_mhz: f[3].parse().unwrap_or(0.0),
            })
        })
        .collect())
}

// ---------------------------------------------------------------------------------------------------------------------------------
// The component of the frames, packets and instruments.

enum Inst {
    None,
    Packets(Box<dyn PacketInstrument>),
    Avhrr(Box<AvhrrDump>),
    Symbols(Box<dyn SymbolInstrument>),
}

fn instrument(p: &Profile) -> Inst {
    match p.instrument {
        Instrument::None => Inst::None,
        Instrument::Viirs => Inst::Packets(Box::new(Viirs::new(MAX_ROWS))),
        Instrument::Modis => Inst::Packets(Box::new(Modis::new(MAX_ROWS / 4))),
        Instrument::Avhrr => Inst::Avhrr(Box::new(AvhrrDump::new(MAX_ROWS))),
        Instrument::Metimage => Inst::Packets(Box::new(Metimage::new(MAX_ROWS))),
        Instrument::Tirs => Inst::Packets(Box::new(Tirs::new(MAX_ROWS))),
        Instrument::Vegetation => Inst::Packets(Box::new(Vegetation::new(MAX_ROWS))),
        Instrument::Amsr2 => Inst::Packets(Box::new(Amsr2::new(MAX_ROWS))),
        Instrument::Ocm => Inst::Symbols(Box::new(Ocm::new(MAX_ROWS))),
        Instrument::Kanopus => Inst::Symbols(Box::new(Kanopus::new(MAX_ROWS))),
        Instrument::Ols => Inst::Symbols(Box::new(Ols::new(MAX_ROWS))),
    }
}

impl Inst {
    fn packets(&mut self) -> Option<&mut dyn PacketInstrument> {
        match self {
            Inst::Packets(i) => Some(i.as_mut()),
            Inst::Avhrr(i) => Some(i.as_mut()),
            _ => None,
        }
    }
    fn planes(&self) -> Option<&Planes> {
        match self {
            Inst::None => None,
            Inst::Packets(i) => Some(i.planes()),
            Inst::Avhrr(i) => Some(i.planes()),
            Inst::Symbols(i) => Some(i.planes()),
        }
    }
    fn planes_mut(&mut self) -> Option<&mut Planes> {
        match self {
            Inst::None => None,
            Inst::Packets(i) => Some(i.planes_mut()),
            Inst::Avhrr(i) => Some(i.planes_mut()),
            Inst::Symbols(i) => Some(i.planes_mut()),
        }
    }
    fn used(&self) -> u64 {
        match self {
            Inst::None => 0,
            Inst::Packets(i) => i.used(),
            Inst::Avhrr(i) => i.used(),
            Inst::Symbols(i) => i.stats().used,
        }
    }
    fn finish(&mut self) {
        match self {
            Inst::None => {}
            Inst::Packets(i) => i.finish(),
            Inst::Avhrr(i) => i.finish(),
            Inst::Symbols(i) => i.finish(),
        }
    }
    fn clear(&mut self) {
        match self {
            Inst::None => {}
            Inst::Packets(i) => i.clear(),
            Inst::Avhrr(i) => i.clear(),
            Inst::Symbols(i) => i.clear(),
        }
    }
}

/// The products of a profile's instrument.
pub fn product_list(p: &Profile) -> &'static [ProductDef] {
    match p.instrument {
        Instrument::Viirs => &products::VIIRS,
        Instrument::Modis => &products::MODIS,
        Instrument::Avhrr => &products::AVHRR,
        _ => &[],
    }
}

/// The report of [`EodbComponent`].
#[derive(Clone, Debug, Default, PartialEq)]
pub struct EodbReport {
    /// Good frames taken (from the frame stage and from frame files).
    pub frames: u64,
    /// Frames of each virtual channel.
    pub per_vc: BTreeMap<u8, u64>,
    /// Spacecraft and virtual channel of the last good frame.
    pub ids: Option<(u16, u8)>,
    /// The satellite of the last good frame.
    pub satellite: String,
    /// Space packets of the profile's virtual channel.
    pub packets: u64,
    /// Packets lost.
    pub packets_lost: u64,
    /// Packets or frames that went into a picture.
    pub used: u64,
    /// Lines of the tallest channel picture.
    pub lines: u64,
    /// The counters of a profile without CADUs.
    pub symbols: Option<SymbolStats>,
    /// CADUs of frame files taken.
    pub imported: u64,
    /// CADUs of frame files whose check failed.
    pub imported_bad: u64,
    /// New log lines.
    pub log: Vec<String>,
    /// Rows of the imported frames since the last report.
    pub rows: Vec<FrameRow>,
}

impl Item for EodbReport {
    const KIND: &'static str = "satimg.eodb_report";
}

/// The `clear` command of the stack with or without a frame table: one text for every profile.
const CLEAR: Command = crate::components::table::CLEAR;

sdrkit_core::commands! {
    /// The commands of [`EodbComponent`].
    pub enum EodbCommand {
        /// Starts new pictures.
        Clear = CLEAR,
        /// Decodes a frame file.
        OpenFrames = OPEN_FRAMES.command,
        /// Makes a product.
        Product = MAKE_PRODUCT.command,
    }
}

/// Good frames (and the soft symbols for the profiles without CADUs) in; the strips of the channel pictures, the good frames
/// as `satimg.frame` items, products and the report out.
pub struct EodbComponent {
    p: &'static Profile,
    inst: Inst,
    asm: Assembler,
    counts: [Option<(u32, u32)>; 64],
    importer: Option<FrameTableComponent>,
    import: Vec<u8>,
    sats: DataFile<Vec<Satellite>>,
    cal: DataFile<CalTable>,
    storage: Arc<dyn Storage>,
    report: EodbReport,
    requests: Vec<(String, usize)>,
    strips: Vec<Strip>,
    out_frames: Vec<Timed<FrameBytes>>,
}

impl EodbComponent {
    /// The component of profile `p`.
    pub fn new(p: &'static Profile) -> Self {
        let importer_link = table(p, p.symbol_rate).or_else(|| {
            p.has_cadus().then(|| TableLink {
                sync: WordSyncConfig {
                    word: 0x1ACF_FC1D,
                    word_bits: 32,
                    frame_bits: 8 * p.frame,
                    acquire_errors: 3,
                    track_errors: 10,
                    complement: true,
                    max_misses: 3,
                },
                line: Line::Nrz,
                code: Some(gen::channel(p)),
                data_randomised: p.rand == Rand::After,
                invert: p.invert,
                retry_plain: p.retry_plain,
                llr_rate: 1.0,
            })
        });
        let importer = importer_link.map(|l| {
            let mut c = FrameTableComponent::new(l);
            let _ = c.configure(&[Format::llr(1.0, None), item_format::<Control>()]);
            c
        });
        EodbComponent {
            p,
            inst: instrument(p),
            asm: Assembler::new(AssemblerConfig::default()),
            counts: [None; 64],
            importer,
            import: Vec::new(),
            sats: DataFile::new(parse_satellites),
            cal: DataFile::new(CalTable::parse),
            storage: crate::files::in_memory(),
            report: EodbReport::default(),
            requests: Vec::new(),
            strips: Vec::new(),
            out_frames: Vec::new(),
        }
    }

    /// The channel pictures, if the profile has an instrument.
    pub fn planes(&self) -> Option<&Planes> {
        self.inst.planes()
    }

    /// Good frames taken, and CADUs of frame files whose check failed.
    pub fn counts(&self) -> (u64, u64) {
        (self.report.frames, self.report.imported_bad)
    }

    /// The satellite of the last good frame, empty when the list does not name it.
    pub fn satellite_name(&self) -> &str {
        &self.report.satellite
    }

    /// The calibration views of AVHRR line `row`.
    pub fn avhrr_line(&self, row: usize) -> Option<crate::instruments::avhrr_dump::LineCal> {
        match &self.inst {
            Inst::Avhrr(a) => a.lines.get(row).cloned().flatten(),
            _ => None,
        }
    }

    /// The name of spacecraft `scid` in the satellite list.
    fn satellite(&self, scid: u16) -> Option<String> {
        let list = self.sats.get()?;
        list.iter()
            .find(|s| s.submode == self.p.id && s.spacecraft == Some(scid))
            .map(|s| s.name.clone())
    }

    /// Takes one good frame (marker, then the bytes after decoding).
    pub fn frame(&mut self, b: &[u8], time: f64) {
        let p = self.p;
        if b.len() < 10 {
            return;
        }
        self.report.frames += 1;
        self.out_frames.push(Timed {
            time,
            value: FrameBytes {
                time_s: time,
                bytes: Arc::from(b),
            },
        });
        let Some(ids) = header_ids(b[4], b[5]) else { return };
        let vc = ids.vcid;
        *self.report.per_vc.entry(vc).or_insert(0) += 1;
        if self.report.ids.map(|i| i.0) != Some(ids.spacecraft) {
            self.report.satellite = self.satellite(ids.spacecraft).unwrap_or_default();
        }
        self.report.ids = Some((ids.spacecraft, vc));
        if p.zone == 0 || p.vc != Some(vc) {
            return;
        }
        let Some(inst) = self.inst.packets() else { return };
        let at = 4 + p.zone_at();
        let (count, pointer) = match p.header {
            Header::Aos(insert) => {
                let c = u32::from_be_bytes([0, b[6], b[7], b[8]]);
                // Continuity: the last count plus 1 modulo 2^24, or a step from xxFF to xx00 (8-bit counters).
                let slot = &mut self.counts[usize::from(vc & 63)];
                let n = match *slot {
                    Some((last, n)) => {
                        let next = (last + 1) & 0xFF_FFFF;
                        let wrap8 = last & 0xFF == 0xFF && c & 0xFF == 0 && c >> 8 == last >> 8;
                        if c == next || wrap8 {
                            n + 1
                        } else {
                            n + 2
                        }
                    }
                    None => 0,
                };
                *slot = Some((c, n));
                let m = 4 + 6 + insert;
                (Some(n & 0xFF_FFFF), u16::from(b[m] & 7) << 8 | u16::from(b[m + 1]))
            }
            Header::Tm => (None, u16::from(b[8] & 7) << 8 | u16::from(b[9])),
            Header::None => return,
        };
        let end = (at + p.zone).min(b.len());
        if at >= end {
            return;
        }
        let zone = &b[at..end];
        let fh = match pointer {
            0x7FE | 0x7FF => FirstHeader::None,
            q if usize::from(q) >= zone.len() => FirstHeader::None,
            q => FirstHeader::At(q),
        };
        let rep = &mut self.report;
        self.asm.push_mpdu(vc, count, fh, zone, |pk| {
            if pk.header.apid == 0x7FF {
                return;
            }
            rep.packets += 1;
            inst.push(&PacketIn {
                apid: pk.header.apid,
                flags: pk.header.flags.bits(),
                seq: pk.header.count,
                data: pk.data,
            });
        });
        self.report.packets_lost = self.asm.stats().lost;
    }

    /// Decodes the CADUs of a frame file of the profile's length.
    pub fn import_frames(&mut self, data: &[u8]) {
        let n = self.p.frame;
        if n == 0 || self.importer.is_none() {
            self.report.log.push("this link has no CADUs".into());
            return;
        }
        let mut good = 0;
        for f in data.chunks_exact(n) {
            if f[..4] != [0x1A, 0xCF, 0xFC, 0x1D] {
                self.report.imported_bad += 1;
                continue;
            }
            self.report.imported += 1;
            let Some(imp) = self.importer.as_mut() else { return };
            let (row, ok) = imp.push_frame(f, 0.0);
            if self.report.rows.len() < crate::components::table::TABLE_ROWS {
                self.report.rows.push(row);
            }
            match ok {
                Some(b) => {
                    good += 1;
                    self.frame(&b, 0.0);
                }
                None => self.report.imported_bad += 1,
            }
        }
        self.inst.finish();
        self.report.log.push(format!("{good} frames of {} CADUs imported", data.len() / n));
    }

    /// Makes product `key` at most `width` columns wide (0: full size).
    pub fn product(&self, key: &str, width: usize) -> Option<Picture> {
        let d = product_list(self.p).iter().find(|d| d.key == key)?;
        let planes = self.inst.planes()?;
        let th = match &self.inst {
            Inst::Avhrr(a) => {
                let cal = self.cal.get();
                let sat = cal.and_then(|c| c.sats.get(&self.report.satellite));
                sat.map(|cal| Thermal { lines: &a.lines, cal })
            }
            _ => None,
        };
        let r = products::render(d, planes, th.as_ref(), width)?;
        let raster = Raster::from_rgb(r.width as u32, r.height as u32, &r.data).ok()?;
        Some(raster.to_picture(d.name, Picture::WHOLE, sdrkit_core::PictureState::Finished))
    }
}

impl Component for EodbComponent {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "satimg.eodb",
            vec![
                Port::typed::<FrameBytes>("frames").optional("the link has no CADUs from samples"),
                Port::llr("symbols"),
            ],
            vec![
                item_port::<Strip>("out"),
                item_port::<FrameBytes>("cadus"),
                item_port::<Picture>("products"),
                item_port::<EodbReport>("report"),
            ],
        )
        .with_settings(EodbFileSetting::schema())
    }
    fn configure(&mut self, _: &[Format]) -> Result<Vec<Format>, ConfigError> {
        Ok(vec![
            item_format::<Strip>(),
            item_format::<FrameBytes>(),
            item_format::<Picture>(),
            item_format::<EodbReport>(),
        ])
    }
    fn attach(&mut self, ctx: &RunContext) {
        self.storage = Arc::clone(&ctx.storage);
        self.sats.attach(Arc::clone(&ctx.storage));
        self.cal.attach(Arc::clone(&ctx.storage));
    }
    fn process(&mut self, io: &mut Io) {
        let frames: Vec<(Arc<[u8]>, f64)> =
            io.items::<FrameBytes>(0).iter().map(|f| (Arc::clone(&f.value.bytes), f.value.time_s)).collect();
        for (b, t) in &frames {
            self.frame(b, *t);
        }
        if let Inst::Symbols(s) = &mut self.inst {
            s.push(io.llrs(1));
        }
        if !self.import.is_empty() {
            let data = std::mem::take(&mut self.import);
            self.import_frames(&data);
        }
        for (key, w) in std::mem::take(&mut self.requests) {
            match self.product(&key, w) {
                Some(pic) => io.items_out::<Picture>(2).push(Timed { time: 0.0, value: pic }),
                None => self.report.log.push(format!("product {key}: no channels or no calibration yet")),
            }
        }
        if let Some(pl) = self.inst.planes_mut() {
            pl.take_strips(&mut self.strips);
        }
        io.items_out::<Strip>(0).extend(self.strips.drain(..).map(|s| Timed { time: 0.0, value: s }));
        io.items_out::<FrameBytes>(1).append(&mut self.out_frames);
        if io.tick() {
            self.report.used = self.inst.used();
            self.report.lines = self.inst.planes().map_or(0, |p| p.lines() as u64);
            self.report.symbols = match &self.inst {
                Inst::Symbols(s) => Some(s.stats()),
                _ => None,
            };
            let r = EodbReport {
                log: std::mem::take(&mut self.report.log),
                rows: std::mem::take(&mut self.report.rows),
                ..self.report.clone()
            };
            io.items_out::<EodbReport>(3).push(Timed { time: 0.0, value: r });
        }
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        match EodbFileSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            EodbFileSetting::Satellites => {
                self.sats.set_path(id, SATELLITES.read(id, v)?)?;
            }
            EodbFileSetting::AvhrrCalibration => {
                self.cal.set_path(id, AVHRR_CALIBRATION.read(id, v)?)?;
            }
        }
        Ok(Applied::Live)
    }
    fn values(&self) -> Values {
        sdrkit_graph::values_of([
            (EodbFileSetting::Satellites, Value::Text(self.sats.path().into())),
            (EodbFileSetting::AvhrrCalibration, Value::Text(self.cal.path().into())),
        ])
    }
    fn commands(&self) -> &Commands {
        EodbCommand::commands()
    }
    fn invoke(&mut self, id: CommandId, args: &Values) -> Result<Option<Record>, CommandError> {
        match EodbCommand::from_id(id).ok_or(CommandError::NoSuchId { id })? {
            EodbCommand::Clear => {
                self.inst.clear();
                self.report = EodbReport::default();
            }
            EodbCommand::OpenFrames => {
                let a: FileArgs = OPEN_FRAMES.args(args)?;
                self.import = crate::files::read(self.storage.as_ref(), args, a.path)?;
            }
            EodbCommand::Product => {
                let a: EodbProductArgs = MAKE_PRODUCT.args(args)?;
                let w = a.width.unwrap_or(0).max(0) as usize;
                self.requests.push((a.product.to_string(), w));
            }
        }
        Ok(None)
    }
    fn any_ref(&self) -> Option<&dyn std::any::Any> {
        Some(self)
    }
    fn any_mut(&mut self) -> Option<&mut dyn std::any::Any> {
        Some(self)
    }
}

sdrkit_core::settings! {
    /// The settings of [`EodbComponent`]: the data files.
    pub enum EodbFileSetting {
        /// The satellite list.
        Satellites = SATELLITES,
        /// The AVHRR calibration.
        AvhrrCalibration = AVHRR_CALIBRATION,
    }
}

// ---------------------------------------------------------------------------------------------------------------------------------
// The display.

fn display_declaration() -> ResultsDecl {
    with_series(
        ResultsDecl::new()
            .with_fields(FrameTableFields::SHAPE.fields)
            .with_fields(EodbFields::SHAPE.fields)
            .with_table(&FRAMES)
            .with_table(&VIRTUAL_CHANNELS),
    )
}

/// The note of profile `p`: what no public document confirms, and what the profile does not decode.
pub fn note(p: &Profile) -> String {
    let mut n = String::from(
        "Experimental: not confirmed on a real signal; no public recording of this link was found. Not confirmed by a \
         public document: every constant of the link profile (modulation, symbol rate, roll-off, differential coding, \
         code, Reed-Solomon code and interleave, randomiser, frame length, header, packet zone and virtual channel)",
    );
    n.push_str(match p.instrument {
        Instrument::Viirs => ", the VIIRS packet layout, zone rules and difference bands",
        Instrument::Modis => ", the MODIS packet layout",
        Instrument::Avhrr => ", the AVHRR line placement; the calibration follows the NOAA KLM User's Guide method (not checked against the text) with coefficients from a data file",
        Instrument::Metimage => ", the METimage packet layout and channel markers",
        Instrument::Tirs => ", the TIRS packet layout",
        Instrument::Vegetation => ", the VEGETATION APIDs and packet layout",
        Instrument::Amsr2 => ", the AMSR2 packet layout and the rule that a first packet ends a line",
        Instrument::Ocm => ", the OCM sync word, randomiser and frame layout",
        Instrument::Kanopus => ", the Kanopus-V interleaving, sync bytes, randomiser, PMSS frame layout and line counter rules",
        Instrument::Ols => ", the OLS frame, line start and pixel rules",
        Instrument::None => "",
    });
    n.push('.');
    match p.conv {
        Conv::Ldpc => n.push_str(" The LDPC code is not decoded: frames come from frame files only."),
        Conv::Turbo => n.push_str(" The turbo code is not decoded, nor the ICER pictures: frames come from frame files only."),
        _ => {}
    }
    if p.instrument == Instrument::None {
        n.push_str(" Frame contents are not decoded: the stack counts and lists the frames.");
    }
    if matches!(p.instrument, Instrument::Viirs | Instrument::Modis) {
        n.push_str(" Products are uncalibrated.");
    }
    n
}

/// The display of `eodb`.
pub struct EodbDisplay {
    p: &'static Profile,
    values: Values,
    published: Published,
    hist: Histories,
    row: u64,
    summary: String,
    hint: String,
    key: String,
    note: String,
    last_good: u64,
    since_good_s: f64,
    /// The newest counts of the gate: samples let go by, their share in percent and the last line height.
    gate: Option<(i64, f64, f64)>,
}

impl EodbDisplay {
    /// The display of profile `p`.
    pub fn new(p: &'static Profile) -> Self {
        EodbDisplay {
            p,
            values: Values::defaults(&EodbDisplaySetting::schema()),
            published: published(display_declaration()),
            hist: Histories::default(),
            row: 0,
            summary: String::new(),
            hint: String::new(),
            key: String::new(),
            note: note(p),
            last_good: 0,
            since_good_s: 0.0,
            gate: None,
        }
    }
}

sdrkit_core::settings! {
    /// The settings that [`EodbDisplay`] holds: those the graph is made from.
    pub enum EodbDisplaySetting {
        /// The symbol rate.
        SymbolRate = SYMBOL_RATE,
        /// The roll-off.
        RollOff = ROLL_OFF,
        /// The ambiguity of the test signal.
        GenAmbiguity = GEN_AMBIGUITY,
    }
}

impl Component for EodbDisplay {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "satimg.eodb_display",
            vec![
                Port::typed::<FrontReport>("front"),
                Port::typed::<DemodReport>("demod"),
                Port::typed::<GateReport>("gate"),
                Port::typed::<TableReport>("table").optional("the link has no CADUs from samples"),
                Port::typed::<EodbReport>("eodb"),
            ],
            vec![item_port::<Control>("control")],
        )
        .with_settings(EodbDisplaySetting::schema())
    }
    fn configure(&mut self, _: &[Format]) -> Result<Vec<Format>, ConfigError> {
        Ok(vec![item_format::<Control>()])
    }
    fn process(&mut self, io: &mut Io) {
        if !io.tick() {
            return;
        }
        let f = last::<FrontReport>(io, 0);
        let d = last::<DemodReport>(io, 1);
        let g = last::<GateReport>(io, 2);
        let none = TableReport::default();
        let r = last::<TableReport>(io, 3).unwrap_or(&none);
        let e = last::<EodbReport>(io, 4).cloned().unwrap_or_default();
        let t = f.map_or(0.0, |f| f.time_s);
        let low = f.is_some_and(|f| f.rate_too_low);
        let open = g.is_some_and(|g| g.open);
        let carrier = open && d.is_some_and(|d| d.state.tracking);
        let sym = e.symbols.unwrap_or_default();
        let good = r.stats.good + sym.frames + (e.imported - e.imported_bad.min(e.imported));
        let locked = r.locked || sym.locked;
        if good > self.last_good {
            self.last_good = good;
            self.since_good_s = 0.0;
        } else if open {
            self.since_good_s += REPORT_S;
        }
        if open && self.since_good_s >= RESEARCH_S {
            self.since_good_s = 0.0;
            io.items_out::<Control>(0).push(Timed {
                time: t,
                value: Control::RestartCarrier,
            });
        }
        let recent = self.since_good_s < (2.0 * self.p.frame_period_s()).max(1.0);
        let state = if good > 0 && recent && (locked || e.imported > 0) {
            3
        } else if locked {
            2
        } else if carrier && !low {
            1
        } else {
            0
        };
        log_lines(self.published.results_mut(), &r.log);
        log_lines(self.published.results_mut(), &e.log);
        let name = self.p.name;
        self.summary.clear();
        // A write to a String cannot fail.
        let _ = match state {
            3 => write!(
                self.summary,
                "{name}, {}, {good} frames, {} lines",
                if e.satellite.is_empty() { "unknown satellite" } else { &e.satellite },
                e.lines
            ),
            2 => write!(self.summary, "{name}, frame sync, frames failing"),
            1 => write!(self.summary, "{name}, symbols, no frame sync"),
            _ => write!(self.summary, "{name}, searching"),
        };
        if !self.p.from_samples() {
            let _ = write!(
                self.summary,
                " ({} not decoded: frames from frame files only)",
                if self.p.conv == Conv::Ldpc { "LDPC" } else { "turbo code" }
            );
        }
        self.hint.clear();
        if low {
            // A write to a String cannot fail.
            let _ = write!(self.hint, "This link needs {:.1} Msps or more (a recording works).", self.p.least / 1e6);
        }
        let p = &mut self.published;
        status(p.results_mut(), state, f, d.filter(|_| open), &self.summary);
        if let Some(g) = g {
            self.gate = Some((
                g.skipped as i64,
                100.0 * g.skipped as f64 / g.samples.max(1) as f64,
                g.last_ratio,
            ));
        }
        p.put(&FrameTableFields {
            snr_db: d.filter(|_| open).map(|d| f64::from(d.state.es_n0_db)),
            symbol_rate_hz: d.map(|d| Hertz(d.state.symbol_rate)),
            frames_ok: good as i64,
            frames_bad: (r.stats.bad + e.imported_bad) as i64,
            frame_lock: locked,
            sync_hits: (r.stats.sync_hits + sym.sync_hits) as i64,
            spacecraft: e.ids.map(|i| i64::from(i.0)),
            vcid: e.ids.map(|i| i64::from(i.1)),
            ..frame_table_fields(t, f, d, r, self.note.as_str())
        });
        self.key.clear();
        if let Some(c) = r.code.filter(|c| c.locked) {
            let _ = write!(self.key, "sync word found, ambiguity {}, pairing {}", c.transform, u8::from(c.shifted));
        }
        p.put(&EodbFields {
            carrier_locked: carrier && d.is_some_and(|d| d.state.locked) || state == 3,
            packets: e.packets as i64,
            packets_lost: e.packets_lost as i64,
            instrument_packets: e.used as i64,
            lines: e.lines as i64,
            satellite: e.satellite.as_str(),
            hypothesis: self.key.as_str(),
            rate_too_low: low,
            imported: e.imported as i64,
            gate_skipped: self.gate.map(|g| g.0),
            gate_skipped_pct: self.gate.map(|g| g.1),
            gate_line: self.gate.map(|g| g.2),
            hint: &self.hint,
        });
        let mut rr = r.clone();
        rr.rows.extend(e.rows.iter().cloned());
        rr.stats.per_vc.clone_from(&e.per_vc);
        publish_tables(p, &rr, &mut self.row, &mut self.key);
        self.hist.publish(self.published.results_mut(), d, None);
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        EodbDisplaySetting::from_id(id).ok_or(SettingError::NoSuchId { id })?;
        self.values.set(id, v)?;
        Ok(Applied::Live)
    }
    fn values(&self) -> Values {
        self.values.clone()
    }
    fn results(&self) -> Option<&Results> {
        Some(self.published.results())
    }
}

// ---------------------------------------------------------------------------------------------------------------------------------
// The graph and the descriptor.

/// The picture layouts of profile `p`: one for each width of its instrument's channels.
pub fn layouts(p: &Profile) -> Vec<Layout> {
    let Some(planes) = instrument(p).planes().cloned() else {
        return Vec::new();
    };
    let mut out: Vec<Layout> = Vec::new();
    for pl in &planes.planes {
        if out.iter().any(|l| l.name == pl.group) {
            continue;
        }
        out.push(Layout {
            name: pl.group,
            width: pl.width as u32,
            max_rows: planes.max_rows as u32,
            full_mask: 1,
            views: vec![
                NamedView::new("auto", "Automatic", View::FirstChannel),
                NamedView::new("rgb123", "Channels 1, 2, 3", View::Rgb([(1, false), (2, false), (3, false)])),
            ],
        });
    }
    out
}

/// The graph of profile `p` with the settings `v`.
///
/// # Errors
///
/// [`GraphError::Schema`] if the picture layouts are refused.
pub fn builder(p: &'static Profile, v: &Values) -> Result<GraphBuilder, GraphError> {
    let l = link(p, v);
    let pictures = Pictures::new(layouts(p)).map_err(GraphError::Schema)?;
    let mut g = GraphBuilder::new()
        .add("frontend", FrontEndComponent::new(l.clone(), p.least, false))
        .add("demod", GatedDemodComponent::new(l.clone(), GATE))
        .add("eodb", EodbComponent::new(p))
        .add("pictures", pictures)
        .add("display", EodbDisplay::new(p))
        .input("frontend.in")
        .connect("frontend.out", "demod.in")
        .connect("demod.soft", "eodb.symbols")
        .connect("eodb.out", "pictures.in")
        .connect("frontend.report", "display.front")
        .connect("demod.report", "display.demod")
        .connect("demod.gate", "display.gate")
        .connect("eodb.report", "display.eodb")
        .feedback("display.control", "demod.control");
    if let Some(t) = table(p, l.symbol_rate) {
        g = g
            .add("frames", FrameTableComponent::new(t).on_helper())
            .connect("demod.soft", "frames.in")
            .connect("frames.frames", "eodb.frames")
            .connect("frames.report", "display.table");
    }
    Ok(g.output("pictures.picture")
        .output("eodb.cadus")
        .output("eodb.products")
        .status_from("display")
        .clock(REPORT_S))
}

/// The receiver with the settings and the submode of `cfg`.
///
/// # Errors
///
/// The errors of [`GraphBuilder::build`] and of [`sdrkit_graph::StackGraphBuilder::build`], among them a refused setting.
pub fn receiver(cfg: &StackConfig) -> Result<StackGraph, GraphError> {
    let p = profile(cfg.submode.as_deref());
    let mut g = builder(p, &cfg.settings)?.catalog(&Catalog::of(cfg)?).build()?;
    g.set_max_items(1 << 20);
    let mut hint = String::new();
    let _ = write!(hint, "this link needs {:.1} Msps or more (a recording works)", p.least / 1e6);
    StackGraph::of(crate::typed::shape(ID), g)
        .min_rate(p.least, &hint)
        .command("clear", sdrkit_graph::CommandTarget::every("clear"))
        .command("open_frames", sdrkit_graph::CommandTarget::of("eodb", "open_frames"))
        .command("product", sdrkit_graph::CommandTarget::of("eodb", "product"))
        .route("symbol_rate", &["display.symbol_rate"])?
        .route("roll_off", &["display.roll_off"])?
        .route("gen_ambiguity", &["display.gen_ambiguity"])?
        .route("satellites", &["eodb.satellites"])?
        .route("avhrr_calibration", &["eodb.avhrr_calibration"])?
        .with_settings(&cfg.settings)
        .build()
}

fn make_receiver(cfg: &StackConfig) -> Result<Box<dyn Receiver>, ConfigError> {
    super::stack_receiver(receiver(cfg))
}

fn make_generator(cfg: &GenConfig) -> Result<Box<dyn Generator>, ConfigError> {
    let p = profile(cfg.submode.as_deref());
    let amb = cfg.settings.get(&GEN_AMBIGUITY).clamp(0, 7) as u8;
    Ok(Box::new(gen::EodbGenerator::new(cfg.rate_hz, p, amb).map_err(super::gen_error)?))
}

fn schema() -> Arc<sdrkit_core::Schema> {
    EodbSetting::schema()
}

/// A profile's submode. It holds no frequency: the host sets the downlink.
const fn sub(id: &'static str, name: &'static str) -> SubMode {
    SubMode {
        id,
        name,
        default_hz: 0.0,
    }
}

/// Direct-broadcast and dump downlinks.
pub static DESCRIPTOR: StackDescriptor = StackDescriptor {
    id: ID,
    name: "X-band direct broadcast",
    group: crate::GROUP,
    blurb: "Direct-broadcast and dump downlinks of polar weather and earth-observation satellites (JPSS, EOS, MetOp, Landsat, Proba, GCOM, Oceansat, SARAL, Kanopus-V, DMSP): frames, instrument pictures and colour products. Most links need a tracking dish and tens of Msps; a recording or a frame file is the usual input.",
    // The stack holds no frequency: the host sets it from the downlinks of data/links/xband_downlinks.csv (the
    // down-converted frequency with a down-converter). Without one the receiver follows the centre of the input.
    tuning: Tuning {
        min_hz: 950e6,
        max_hz: 9000e6,
        ..Tuning::anywhere(0.0, 25e6, 0.6e6, 21e6)
    },
    submodes: &[
        sub("jpss_hrd", "JPSS HRD"),
        sub("jpss_hrd_n21", "JPSS HRD (NOAA-21)"),
        sub("eos_aqua_aura", "EOS Aqua and Aura"),
        sub("eos_terra", "EOS Terra"),
        sub("metop_dump", "MetOp X-band dump"),
        sub("metopsg_ddb", "MetOp-SG DDB"),
        sub("landsat_x", "Landsat X band"),
        sub("landsat_s", "Landsat S band"),
        sub("proba_s", "Proba S band"),
        sub("proba_v_x", "Proba-V X band"),
        sub("gcom_w1", "GCOM-W1"),
        sub("gcom_c1", "GCOM-C1"),
        sub("stereo", "STEREO beacon"),
        sub("oceansat2", "Oceansat-2"),
        sub("oceansat3_l", "Oceansat-3 L band"),
        sub("saral_l", "SARAL L band"),
        sub("oceansat3_x", "Oceansat-3 X band"),
        sub("saral_x", "SARAL X band"),
        sub("kanopus_v", "Kanopus-V"),
        sub("dmsp_rtd", "DMSP RTD"),
    ],
    settings: schema,
    make_receiver,
    make_generator: Some(make_generator),
    lock_profile: LockProfile::DEFAULT,
    signal_types: &[],
    auto_gain: AutoGain::Radio,
    verify: Some(&crate::verify::EODB_HOOKS),
    benchmarks: None,
};

#[cfg(test)]
mod tests {
    use super::profiles::PROFILES;
    use super::*;

    #[test]
    fn every_profile_has_a_submode() {
        assert_eq!(DESCRIPTOR.submodes.len(), PROFILES.len());
        for (s, p) in DESCRIPTOR.submodes.iter().zip(PROFILES.iter()) {
            assert_eq!(s.id, p.id);
        }
    }
}
