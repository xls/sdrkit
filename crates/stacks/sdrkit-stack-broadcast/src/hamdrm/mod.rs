//! HamDRM: amateur digital picture transfer on a DRM signal in an SSB channel. The parts of the format that no document fixes
//! come from a JSON file at run time (setting `hamdrm_format`). Without it the stack decodes nothing.
//!
//! Standard: HB9TLK, DRM mode "HAM" specification, with ETSI ES 201 980; MOT of ETSI EN 301 234.

pub mod format;
pub mod frame;

pub use format::{map_key, FacBit, FacCode, FormatError, HamFormat, PhaseList};
pub use frame::{
    capacity, occupancy_text, rates, CellKind, CellMap, Content, FacValues, FrameInfo, HamMode,
    MapFault, DC_HZ, RATE,
};

use crate::component::ClearCommand;
use crate::datafile::{FileError, Loaded};
use crate::drm::data::{content_type_text, PacketService};
use crate::drm::disperse;
use crate::drm::interleave::{CellDeinterleaver, CellInterleaver};
use crate::drm::mlc::Mlc;
use crate::drm::params::{gain_phase, pilot_value, Qam};
use frame::{FRAMES_PER_SUPERFRAME, LABEL_CHARS, LONG_DEPTH};
use sdrkit_algo_dsp::nco::Nco;
use sdrkit_algo_modem::ofdm::{CellFrame, FrameDemodulator, FrameLayout, FrameModulator};
use sdrkit_core::interface::{Fields, Hertz, Seconds, Table};
use sdrkit_core::publish::{field, keys, Published};
use sdrkit_core::{
    Codec, CommandError, CommandId, CommandSet, Commands, Complex32, ConfigError, GenConfig,
    Generator, LockState, Receiver, Record, Results, Setting, SettingError, SettingId, SettingSet,
    SignalInfo, StackConfig, Status, Value, Values,
};
use sdrkit_graph::components::FirFilter;
use sdrkit_graph::{
    values_of, Applied, Catalog, Component, ComponentInfo, Format, Graph, GraphBuilder, GraphError,
    Io, Item, Kept, Kind, Picture, Port, RateFrom, RateRule, StackGraph, Timed,
};
use sdrkit_proto_bcdata::datagroup::{self, Header};
use sdrkit_proto_bcdata::mot::{self, HeaderWriter};
use sdrkit_proto_bcdata::packet::DrmPacketWriter;
use sdrkit_util_bits::crc::catalog::CRC32_ISO_HDLC;
use sdrkit_util_bits::pack::{pack, unpack};
use sdrkit_util_bits::{BitOrder, Pad};
use std::collections::{BTreeMap, VecDeque};
use std::sync::Arc;

/// How long a demodulator refuses the carrier offset of a lock that produced no FAC, s.
const REFUSE_S: f64 = 2.0;

/// The stack id.
pub const ID: &str = "hamdrm";
/// The lowest input rate: the rate of the OFDM demodulator.
pub const MIN_RATE: f64 = RATE;

/// The text of the `format` field without the format file.
pub const NO_FORMAT: &str =
    "not loaded: set hamdrm_format to a file with the HamDRM maps, pilot phases, FAC code and packet layer";

/// A format as a component holds it.
pub type LoadedFormat = Loaded<HamFormat, FormatError>;

impl FileError for FormatError {
    fn read_kind(&self) -> Option<std::io::ErrorKind> {
        match self {
            FormatError::Read { kind } => Some(*kind),
            _ => None,
        }
    }
}

/// Loads the format at `path`: `Ok(None)` for an empty path.
///
/// # Errors
///
/// [`FormatError`] if the file cannot be read or is refused.
pub fn load_format(path: &str) -> Result<Option<Arc<HamFormat>>, FormatError> {
    Loaded::load(path, HamFormat::load).value().clone()
}

/// The text of the `format` field.
fn format_text(l: &LoadedFormat) -> String {
    match l.value() {
        Ok(Some(f)) => format!("loaded: {}", f.note()),
        Ok(None) => NO_FORMAT.into(),
        Err(e) => format!("refused: {e}"),
    }
}

/// The format file of a component.
const COMPONENT_FORMAT: Setting<field::Text> = Setting::new("format", "HamDRM format", "", "")
    .help("A JSON file with the parts of the HamDRM format that no document fixes.");

sdrkit_core::settings! {
    /// The settings of [`Frames`] and [`Files`].
    pub enum FormatSetting {
        /// The format file.
        Format = COMPONENT_FORMAT,
    }
}

/// The OFDM layout of the map the document draws for mode `m`, its known cells the pilots whose values the documents fix:
/// the scattered pilots of modes A and B (ES 201 980 clause 8.4.4). Mode E has none.
pub fn layout(m: HamMode) -> FrameLayout {
    let map = CellMap::drawn(m);
    format::layout_with(&map, |s, c| {
        let a = match map.kind(s, c) {
            CellKind::Scattered => std::f32::consts::SQRT_2,
            CellKind::Boosted => 2.0,
            _ => return None,
        };
        let t = gain_phase(m.drm()?, s, map.kmin() + c as i32)?;
        Some(pilot_value(a, t))
    })
}

/// How the MSC of a frame decoded.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MscState {
    /// Every packet has a good CRC.
    Good,
    /// At least one packet has a bad CRC.
    Bad,
    /// The long interleaver is still filling.
    Waiting,
    /// The frame carries audio, which sdrkit does not decode.
    Audio,
}

/// One frame: its FAC and the data groups that it completed.
#[derive(Clone, Debug)]
pub struct HamFrame {
    /// The mode.
    pub mode: HamMode,
    /// The FAC.
    pub info: FrameInfo,
    /// The label of the super frame so far (the station's call sign).
    pub label: String,
    /// How the MSC decoded.
    pub msc: MscState,
    /// The data groups (MSC data groups of EN 300 401 clause 5.3.3) that the frame completed.
    pub groups: Vec<Vec<u8>>,
    /// SNR, dB.
    pub snr_db: f32,
    /// Tuning error, Hz.
    pub offset_hz: f64,
}
impl Item for HamFrame {
    const KIND: &'static str = "hamdrm.frame";
}

/// Receiver state events.
#[derive(Clone, Debug)]
pub struct HamState {
    /// 0 searching, 1 receiving, 2 ended.
    pub state: u8,
    /// Frames with a good MSC.
    pub good: u64,
    /// Frames with a bad MSC.
    pub bad: u64,
}
impl Item for HamState {
    const KIND: &'static str = "hamdrm.state";
}

/// The MSC decoder of one FAC configuration.
struct Msc {
    key: (Qam, u8, bool, u8),
    deint: CellDeinterleaver,
    mlc: Mlc,
    packets: PacketService,
    bits: Vec<u8>,
    bytes: Vec<u8>,
}

/// The decoder of the frames of one lock.
struct Lock {
    mode: HamMode,
    wide: bool,
    fac: FacCode,
    fac_cells: Vec<usize>,
    msc_cells: Vec<usize>,
    msc: Option<Msc>,
    label: [u8; FRAMES_PER_SUPERFRAME * LABEL_CHARS],
    fac_bits: Vec<u8>,
    z: Vec<Complex32>,
    w: Vec<f32>,
}

impl Lock {
    fn new(f: &HamFormat, mode: HamMode, wide: bool) -> Lock {
        let map = f.map(mode, wide);
        Lock {
            mode,
            wide,
            fac: f.fac_code(),
            fac_cells: map.cells_of(CellKind::Fac),
            msc_cells: map.cells_of(CellKind::Msc),
            msc: None,
            label: [b' '; FRAMES_PER_SUPERFRAME * LABEL_CHARS],
            fac_bits: Vec::with_capacity(frame::FAC_BITS),
            z: Vec::with_capacity(map.count(CellKind::Msc)),
            w: Vec::with_capacity(map.count(CellKind::Msc)),
        }
    }

    /// The FAC of a frame, `None` when its CRC fails or it names another occupancy.
    fn fac(&mut self, f: &HamFormat, cf: &CellFrame) -> Option<FrameInfo> {
        self.z.clear();
        self.z.extend(self.fac_cells.iter().map(|&i| cf.z[i]));
        self.w.clear();
        self.w.extend(self.fac_cells.iter().map(|&i| cf.w[i]));
        self.fac.decode(&self.z, &self.w, &mut self.fac_bits);
        FrameInfo::from_bits(&self.fac_bits, f.values()).filter(|i| i.wide == self.wide)
    }

    /// Decodes the MSC of a frame with FAC `info` into data groups.
    fn msc(
        &mut self,
        f: &HamFormat,
        cf: &CellFrame,
        info: &FrameInfo,
        groups: &mut Vec<Vec<u8>>,
    ) -> MscState {
        let Content::Data { packet_id } = info.content else {
            self.msc = None;
            return MscState::Audio;
        };
        let key = (info.qam, info.protection, info.long_interleave, packet_id);
        if self.msc.as_ref().is_none_or(|m| m.key != key) {
            let n = self.msc_cells.len();
            let depth = if info.long_interleave { LONG_DEPTH } else { 1 };
            let built = (|| {
                Some(Msc {
                    key,
                    deint: CellDeinterleaver::new(n, depth),
                    mlc: frame::msc_code(info.qam, info.protection, n)?,
                    packets: PacketService::new(packet_id, f.packet_bytes())?,
                    bits: Vec::new(),
                    bytes: Vec::new(),
                })
            })();
            self.msc = built;
        }
        let Some(m) = &mut self.msc else {
            return MscState::Bad;
        };
        self.z.clear();
        self.z.extend(self.msc_cells.iter().map(|&i| cf.z[i]));
        self.w.clear();
        self.w.extend(self.msc_cells.iter().map(|&i| cf.w[i]));
        let Some((z, w)) = m.deint.push(cf.index, &self.z, &self.w) else {
            return MscState::Waiting;
        };
        m.mlc.decode(&z, &w, 2, &mut m.bits);
        disperse(&mut m.bits);
        m.bytes.clear();
        pack(&m.bits, BitOrder::Msb, Pad::Drop, &mut m.bytes);
        let whole = m.bytes.len() / f.packet_bytes() * f.packet_bytes();
        let bad = m.packets.stats.bad_crc;
        m.packets
            .push(&m.bytes[..whole], &mut |u| groups.push(u.to_vec()));
        if m.packets.stats.bad_crc == bad {
            MscState::Good
        } else {
            MscState::Bad
        }
    }

    fn label_text(&self) -> String {
        let t: String = self
            .label
            .iter()
            .map(|&c| {
                if (0x20..0x7F).contains(&c) {
                    c as char
                } else {
                    '_'
                }
            })
            .collect();
        t.trim().to_string()
    }
}

/// The OFDM demodulator and frame decoder (`broadcast.hamdrm_frames`, iq @ 12 kHz -> typed `frames`, `state`): one
/// demodulator per mode and occupancy of the format. Without the format it decodes nothing.
pub struct Frames {
    loaded: LoadedFormat,
    format: Option<Arc<HamFormat>>,
    demods: Vec<Slot>,
    active: Option<usize>,
    lock: Option<Lock>,
    /// Seconds of input so far.
    now: f64,
    had_frames: bool,
    good: u64,
    bad: u64,
    state: u8,
}

/// One demodulator of the search, with the input time of its first sample.
struct Slot {
    mode: HamMode,
    wide: bool,
    demod: FrameDemodulator,
    t0: f64,
    misses: u32,
    heard: bool,
}

impl Default for Frames {
    fn default() -> Self {
        Frames::new(LoadedFormat::none())
    }
}

impl Frames {
    /// The stage with this format.
    pub fn new(loaded: LoadedFormat) -> Frames {
        let mut f = Frames {
            format: loaded.get(),
            loaded,
            demods: Vec::new(),
            active: None,
            lock: None,
            now: 0.0,
            had_frames: false,
            good: 0,
            bad: 0,
            state: 0,
        };
        f.reset();
        f
    }

    /// New demodulators from the next input sample on, and no lock.
    fn search(&mut self) {
        self.demods.clear();
        if let Some(f) = &self.format {
            for m in HamMode::ALL {
                for wide in [true, false] {
                    self.demods.push(Slot {
                        mode: m,
                        wide,
                        demod: FrameDemodulator::new(f.layout(m, wide)),
                        t0: self.now,
                        misses: 0,
                        heard: false,
                    });
                }
            }
        }
        self.active = None;
        self.lock = None;
    }

    fn reset(&mut self) {
        self.search();
        self.state = 0;
    }
}

impl Component for Frames {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "broadcast.hamdrm_frames",
            vec![Port::iq("in").rate(RateRule::Exact(RATE))],
            vec![
                Port::typed::<HamFrame>("frames").rate_from(RateFrom::Fixed(0.0)),
                Port::typed::<HamState>("state").rate_from(RateFrom::Fixed(0.0)),
            ],
        )
        .with_settings(FormatSetting::schema())
    }
    fn configure(&mut self, _inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        self.reset();
        Ok(vec![
            Format::new(Kind::typed::<HamFrame>(), 0.0),
            Format::new(Kind::typed::<HamState>(), 0.0),
        ])
    }
    fn process(&mut self, io: &mut Io) {
        let Some(fmt) = self.format.clone() else {
            return;
        };
        let x = io.iq(0);
        let mut frames = Vec::new();
        let mut states = Vec::new();
        let mut lost = false;
        for i in 0..self.demods.len() {
            if self.active.is_some_and(|a| a != i) || lost {
                continue;
            }
            self.demods[i].demod.push(x);
            while let Some(cf) = self.demods[i].demod.next_frame() {
                let (m, wide) = (self.demods[i].mode, self.demods[i].wide);
                let time = self.demods[i].t0 + cf.time;
                let mut lock = match self.lock.take() {
                    Some(l) if l.mode == m && l.wide == wide => l,
                    _ => Lock::new(&fmt, m, wide),
                };
                match lock.fac(&fmt, &cf) {
                    Some(info) => {
                        // A lock counts once its FAC decodes: the right mode and occupancy win at once.
                        self.active = Some(i);
                        self.demods[i].misses = 0;
                        self.demods[i].heard = true;
                        self.had_frames = true;
                        let at = info.frame_id as usize * LABEL_CHARS;
                        lock.label[at..at + LABEL_CHARS].copy_from_slice(&info.label);
                        let mut groups = Vec::new();
                        let msc = lock.msc(&fmt, &cf, &info, &mut groups);
                        match msc {
                            MscState::Good => self.good += 1,
                            MscState::Bad => self.bad += 1,
                            _ => {}
                        }
                        self.state = 1;
                        frames.push((
                            time,
                            HamFrame {
                                mode: m,
                                info,
                                label: lock.label_text(),
                                msc,
                                groups,
                                snr_db: cf.snr_db,
                                offset_hz: cf.cfo_hz,
                            },
                        ));
                        self.lock = Some(lock);
                    }
                    None => {
                        if self.active == Some(i) {
                            self.lock = Some(lock);
                        }
                        let d = &mut self.demods[i];
                        d.misses += 1;
                        if d.misses >= 3 {
                            d.misses = 0;
                            if d.heard {
                                // The signal is gone: every mode searches again from here.
                                lost = true;
                                if self.had_frames {
                                    self.state = 2;
                                }
                            } else {
                                // A lock that never produced a FAC is a false one (a pilot pattern moved by whole carriers):
                                // the next search looks at other carrier offsets.
                                d.demod.block_current(REFUSE_S);
                            }
                        }
                    }
                }
                states.push((
                    time,
                    HamState {
                        state: self.state,
                        good: self.good,
                        bad: self.bad,
                    },
                ));
                if lost || self.active.is_some_and(|a| a != i) {
                    break;
                }
            }
        }
        self.now += x.len() as f64 / RATE;
        if lost {
            self.search();
        }
        for (t, f) in frames {
            io.items_out::<HamFrame>(0)
                .push(Timed { time: t, value: f });
        }
        for (t, s) in states {
            io.items_out::<HamState>(1)
                .push(Timed { time: t, value: s });
        }
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        match FormatSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            FormatSetting::Format => {
                if self
                    .loaded
                    .apply(id, COMPONENT_FORMAT.read(id, v)?, HamFormat::load)?
                {
                    self.format = self.loaded.get();
                    self.reset();
                }
            }
        }
        Ok(Applied::Live)
    }
    /// The search starts again, and so do the frame counts.
    fn forget(&mut self, _kept: &Kept) {
        self.reset();
        self.good = 0;
        self.bad = 0;
    }
    fn values(&self) -> Values {
        values_of([(
            FormatSetting::Format,
            Value::Text(self.loaded.path().into()),
        )])
    }
    fn commands(&self) -> &Commands {
        ClearCommand::commands()
    }
    fn invoke(&mut self, id: CommandId, _args: &Values) -> Result<Option<Record>, CommandError> {
        match ClearCommand::from_id(id).ok_or(CommandError::NoSuchId { id })? {
            ClearCommand::Clear => self.reset(),
        }
        Ok(None)
    }
}

/// The header of a MOT object, once all its header segments are in.
#[derive(Clone, Debug, Default)]
struct HeaderInfo {
    size: u32,
    content_type: u8,
    subtype: u16,
    name: String,
}

#[derive(Clone, Debug, Default)]
struct Partial {
    header: Option<HeaderInfo>,
    header_segs: BTreeMap<u16, Vec<u8>>,
    header_last: Option<u16>,
    body: BTreeMap<u16, Vec<u8>>,
    last: Option<u16>,
    transmissions: u32,
    order: u64,
    reported: bool,
    prev_seg: Option<u16>,
    call: String,
}

/// A finished file.
#[derive(Clone, Debug)]
pub struct FinishedFile {
    /// Transport id.
    pub id: u16,
    /// Name.
    pub name: String,
    /// Call sign: the label of the FAC.
    pub call: String,
    /// MIME type.
    pub mime: String,
    /// Completed at, signal seconds.
    pub completed_s: f64,
    /// Transmissions received.
    pub transmissions: u32,
    /// The bytes.
    pub bytes: Arc<[u8]>,
    /// Size.
    pub size: usize,
    /// CRC-32 of the bytes.
    pub crc32: u32,
}

/// The file being received.
#[derive(Clone, Debug, Default)]
pub struct Current {
    /// Transport id.
    pub id: u16,
    /// Name.
    pub name: String,
    /// Call sign.
    pub call: String,
    /// Body size.
    pub size: u32,
    /// Segments in total (when known).
    pub segments_total: Option<u32>,
    /// Segments received.
    pub segments_received: u32,
    /// Missing segment numbers (at most 64).
    pub missing: Vec<u16>,
    /// Transmissions heard.
    pub transmissions: u32,
    /// Done.
    pub done: bool,
}

sdrkit_core::choice_enum! {
    /// The interleaver of the MSC.
    #[derive(Default)]
    pub enum Interleave {
        /// 400 ms.
        #[default]
        Short = ("short", "400 ms"),
        /// 2 s.
        Long = ("long", "2 s"),
    }
}

/// The results of the HamDRM stack. No mode, occupancy, constellation, protection, interleaver, label, SNR or offset
/// before the first frame, and no file before its first segment.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct HamdrmFields<'a> {
    /// Whether the format is loaded, and from what (or why not).
    #[field(label = "HamDRM format")]
    pub format: &'a str,
    /// The robustness mode, A, B or E.
    #[field(label = "Mode")]
    pub mode: Option<HamMode>,
    /// The occupancy.
    #[field(label = "Occupancy", unit = "Hz")]
    pub occupancy: Option<Hertz>,
    /// The constellation.
    #[field(label = "QAM")]
    pub qam: Option<Qam>,
    /// The protection level, 0 or 1.
    #[field(label = "Protection")]
    pub protection: Option<i64>,
    /// The interleaver.
    #[field(label = "Interleaver")]
    pub interleave: Option<Interleave>,
    /// The label of the FAC (the station's call sign).
    #[field(label = "Label")]
    pub label: Option<&'a str>,
    /// The SNR of the last frame.
    #[field(shared = keys::SNR_DB)]
    pub snr_db: Option<f64>,
    /// The carrier offset of the last frame.
    #[field(shared = keys::OFFSET_HZ)]
    pub offset_hz: Option<Hertz>,
    /// Frames good.
    #[field(shared = keys::FRAMES_OK)]
    pub frames_ok: i64,
    /// Frames bad.
    #[field(shared = keys::FRAMES_BAD)]
    pub frames_bad: i64,
    /// Files completed.
    #[field(label = "Files completed")]
    pub files_completed: i64,
    /// The transport id of the file being received.
    #[field(label = "File id")]
    pub file_id: Option<i64>,
    /// Its name.
    #[field(label = "File name")]
    pub file_name: Option<&'a str>,
    /// Its call sign.
    #[field(label = "Call sign")]
    pub file_call: Option<&'a str>,
    /// Its body size.
    #[field(label = "File size")]
    pub file_size: Option<i64>,
    /// Its segments in total, once the last segment number is known.
    #[field(label = "Segments")]
    pub segments_total: Option<i64>,
    /// Its segments received.
    #[field(label = "Segments received")]
    pub segments_received: Option<i64>,
    /// The transmissions heard.
    #[field(label = "Transmissions")]
    pub transmissions: Option<i64>,
    /// Whether it is complete.
    #[field(label = "Done")]
    pub done: Option<bool>,
}

/// A finished file. The row key is its transport id.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct HamdrmFileRow<'a> {
    /// The transport id.
    #[field(label = "Id")]
    pub id: i64,
    /// The name.
    #[field(label = "Name")]
    pub name: &'a str,
    /// The call sign of the sender.
    #[field(label = "Call sign")]
    pub call: &'a str,
    /// The content type.
    #[field(label = "Type")]
    pub mime: &'a str,
    /// The body size.
    #[field(label = "Size")]
    pub size: i64,
    /// The transmissions heard.
    #[field(label = "Transmissions")]
    pub transmissions: i64,
    /// When it was complete, in signal time.
    #[field(label = "Completed", unit = "s")]
    pub completed: Seconds,
    /// The CRC-32 of the body.
    #[field(label = "CRC-32")]
    pub crc32: i64,
}

/// A missing body segment of the file being received. The row key is the segment number.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct HamdrmMissingRow {
    /// The number of the segment.
    #[field(label = "Segment")]
    pub segment: i64,
}

/// The missing body segments of the file being received (at most 64).
pub static MISSING: Table<HamdrmMissingRow> = Table::new("missing", "Missing segments", 64);

/// The history of finished files, newest first.
pub static HISTORY: Table<HamdrmFileRow<'static>> = Table::new("history", "Files", 8);

/// The state held by the file assembler.
#[derive(Clone, Debug, Default)]
struct HamDetail {
    state: u8,
    mode: Option<HamMode>,
    occupancy_khz: f64,
    qam: Option<Qam>,
    protection: u8,
    interleave: Interleave,
    label: String,
    snr_db: f32,
    offset_hz: f64,
    frames_good: u64,
    frames_bad: u64,
    current: Option<Current>,
    files_completed: u64,
}

/// The file assembler (`broadcast.hamdrm_files`, typed `frames` + `state` -> typed `pictures`), with the mode's results. It
/// collects the MOT header and body segments of each transport id (EN 301 234 clause 5).
pub struct Files {
    loaded: LoadedFormat,
    format_text: String,
    partial: BTreeMap<u16, Partial>,
    history: VecDeque<FinishedFile>,
    history_changed: bool,
    missing_written: Vec<u16>,
    d: HamDetail,
    published: Published,
    status: Status,
    order: u64,
    last_kind: u8,
    run_counted: bool,
    last_report: f64,
    time: f64,
}

impl Default for Files {
    fn default() -> Self {
        Files::new(LoadedFormat::none())
    }
}

impl Files {
    /// The assembler, with the format that the `format` field shows.
    pub fn new(loaded: LoadedFormat) -> Files {
        let mut f = Files {
            format_text: format_text(&loaded),
            loaded,
            partial: BTreeMap::new(),
            history: VecDeque::new(),
            history_changed: false,
            missing_written: Vec::with_capacity(64),
            d: HamDetail::default(),
            published: Published::from_decl(Arc::new(crate::typed::shape(ID).results_decl())),
            status: Status::new(LockState::Searching),
            order: 0,
            last_kind: 0,
            run_counted: false,
            last_report: -1.0,
            time: 0.0,
        };
        f.publish();
        f
    }

    fn missing(p: &Partial) -> Vec<u16> {
        let top = p.last.or_else(|| p.body.keys().next_back().copied());
        match top {
            Some(l) => (0..=l)
                .filter(|i| !p.body.contains_key(i))
                .take(64)
                .collect(),
            None => Vec::new(),
        }
    }

    fn take(&mut self, f: &HamFrame, out: &mut Vec<Picture>) {
        let d = &mut self.d;
        d.mode = Some(f.mode);
        d.occupancy_khz = if f.info.wide { 2.5 } else { 2.3 };
        d.qam = Some(f.info.qam);
        d.protection = f.info.protection;
        d.interleave = if f.info.long_interleave {
            Interleave::Long
        } else {
            Interleave::Short
        };
        d.label.clone_from(&f.label);
        d.snr_db = f.snr_db;
        d.offset_hz = f.offset_hz;
        for g in &f.groups {
            self.group(g, &f.label, out);
        }
    }

    fn group(&mut self, bytes: &[u8], call: &str, out: &mut Vec<Picture>) {
        let Ok(g) = datagroup::parse(bytes) else {
            return;
        };
        let kind = g.header.group_type;
        if kind != datagroup::MOT_HEADER && kind != datagroup::MOT_BODY {
            return;
        }
        let (Some(id), Some((is_last, seg))) = (g.header.transport_id, g.header.segment) else {
            return;
        };
        let Ok((_, data)) = mot::parse_segment(g.data) else {
            return;
        };
        self.order += 1;
        let order = self.order;
        let p = self.partial.entry(id).or_insert_with(|| Partial {
            order,
            ..Default::default()
        });
        p.call = call.to_string();
        // The headers that open a transmission count it (the one closing it, after the body, does not).
        if kind == datagroup::MOT_HEADER
            && self.last_kind != datagroup::MOT_BODY
            && !self.run_counted
        {
            p.transmissions += 1;
            self.run_counted = true;
        }
        if kind == datagroup::MOT_BODY {
            // Body segments starting over without the opening headers (missed while locking) are a new transmission too.
            if p.prev_seg.is_some_and(|prev| seg <= prev) && !self.run_counted {
                p.transmissions += 1;
            }
            if p.transmissions == 0 {
                p.transmissions = 1;
            }
            p.prev_seg = Some(seg);
            self.run_counted = false;
        }
        self.last_kind = kind;
        if kind == datagroup::MOT_HEADER {
            p.header_segs.insert(seg, data.to_vec());
            if is_last {
                p.header_last = Some(seg);
            }
            if let Some(hl) = p.header_last {
                if (0..=hl).all(|i| p.header_segs.contains_key(&i)) {
                    let mut hb = Vec::new();
                    for i in 0..=hl {
                        hb.extend_from_slice(&p.header_segs[&i]);
                    }
                    p.header = mot::Header::parse(&hb).ok().map(|h| HeaderInfo {
                        size: h.body_size,
                        content_type: h.content_type,
                        subtype: h.content_subtype,
                        name: h.content_name_text(),
                    });
                }
            }
            // A file finished from its body alone gets its name when the header comes.
            if let (true, Some(h)) = (p.reported, &p.header) {
                for f in self
                    .history
                    .iter_mut()
                    .filter(|f| f.id == id && f.name.is_empty())
                {
                    f.name.clone_from(&h.name);
                    f.mime = content_type_text(h.content_type, h.subtype);
                    self.history_changed = true;
                }
            }
        } else {
            p.body.insert(seg, data.to_vec());
            if is_last {
                p.last = Some(seg);
            }
        }
        let complete = p
            .last
            .is_some_and(|l| (0..=l).all(|i| p.body.contains_key(&i)));
        if complete && !p.reported {
            let mut bytes = Vec::new();
            for v in p.body.values() {
                bytes.extend_from_slice(v);
            }
            let h = p.header.clone().unwrap_or_default();
            if h.size as usize == bytes.len() || p.header.is_none() {
                p.reported = true;
                let file = FinishedFile {
                    id,
                    name: h.name.clone(),
                    call: p.call.clone(),
                    mime: if p.header.is_some() {
                        content_type_text(h.content_type, h.subtype)
                    } else {
                        String::new()
                    },
                    completed_s: self.time,
                    transmissions: p.transmissions,
                    size: bytes.len(),
                    crc32: CRC32_ISO_HDLC.checksum(&bytes),
                    bytes: bytes.into(),
                };
                self.published
                    .results_mut()
                    .log(format!("received {} from {}", file.name, file.call));
                // EN 301 234 clause 6.2.3: image subtype 1 is JFIF, 3 is PNG, 0 is GIF.
                let codec = match (h.content_type, h.subtype) {
                    (2, 1) => Codec::Jpeg,
                    (2, 3) => Codec::Png,
                    _ => Codec::Gif,
                };
                out.push(Picture {
                    codec,
                    layout: None,
                    name: Some(file.name.clone()),
                    data: file.bytes.clone(),
                });
                self.history.push_front(file);
                while self.history.len() > 8 {
                    self.history.pop_back();
                }
                self.history_changed = true;
                self.d.files_completed += 1;
            }
        }
        let p = &self.partial[&id];
        self.d.current = Some(Current {
            id,
            name: p
                .header
                .as_ref()
                .map(|h| h.name.clone())
                .unwrap_or_default(),
            call: p.call.clone(),
            size: p.header.as_ref().map(|h| h.size).unwrap_or(0),
            segments_total: p.last.map(|l| l as u32 + 1),
            segments_received: p.body.len() as u32,
            missing: Self::missing(p),
            transmissions: p.transmissions,
            done: p.reported,
        });
        while self.partial.len() > 8 {
            let oldest = self
                .partial
                .iter()
                .min_by_key(|(_, p)| p.order)
                .map(|(k, _)| *k);
            if let Some(k) = oldest {
                self.partial.remove(&k);
            }
        }
    }

    fn publish(&mut self) {
        let d = &self.d;
        let p = &mut self.published;
        let framed = d.mode.is_some();
        let current = d.current.as_ref();
        p.put(&HamdrmFields {
            format: &self.format_text,
            mode: d.mode,
            occupancy: framed.then_some(Hertz(d.occupancy_khz * 1e3)),
            qam: d.qam,
            protection: framed.then_some(i64::from(d.protection)),
            interleave: framed.then_some(d.interleave),
            label: (framed && !d.label.is_empty()).then_some(d.label.as_str()),
            snr_db: framed.then_some(f64::from(d.snr_db)),
            offset_hz: framed.then_some(Hertz(d.offset_hz)),
            frames_ok: d.frames_good as i64,
            frames_bad: d.frames_bad as i64,
            files_completed: d.files_completed as i64,
            file_id: current.map(|c| i64::from(c.id)),
            file_name: current.map(|c| c.name.as_str()),
            file_call: current.map(|c| c.call.as_str()),
            file_size: current.map(|c| c.size as i64),
            segments_total: current.and_then(|c| c.segments_total).map(i64::from),
            segments_received: current.map(|c| i64::from(c.segments_received)),
            transmissions: current.map(|c| c.transmissions as i64),
            done: current.map(|c| c.done),
        });
        let missing: &[u16] = current.map_or(&[], |c| &c.missing);
        if self.missing_written != missing {
            p.clear_rows(&MISSING);
            for &m in missing {
                let row = HamdrmMissingRow {
                    segment: i64::from(m),
                };
                MISSING.put(p, &m.to_string(), &row);
            }
            self.missing_written.clear();
            self.missing_written.extend_from_slice(missing);
        }
        if self.history_changed {
            self.history_changed = false;
            p.clear_rows(&HISTORY);
            for f in self.history.iter().rev() {
                let row = HamdrmFileRow {
                    id: f.id as i64,
                    name: &f.name,
                    call: &f.call,
                    mime: &f.mime,
                    size: f.size as i64,
                    transmissions: f.transmissions as i64,
                    completed: Seconds(f.completed_s),
                    crc32: i64::from(f.crc32),
                };
                HISTORY.put(p, &f.id.to_string(), &row);
            }
        }
        let st = &mut self.status;
        st.lock_state = match d.state {
            0 => LockState::Searching,
            1 => LockState::Locked,
            _ => LockState::Tracking,
        };
        st.snr_db = Some(d.snr_db);
        st.offset_hz = Some(d.offset_hz);
        st.summary.clear();
        use std::fmt::Write;
        // Writing into a String cannot fail.
        let _ = match (self.loaded.value(), d.state) {
            (Ok(None), _) => write!(st.summary, "HamDRM format {NO_FORMAT}"),
            (Err(e), _) => write!(st.summary, "HamDRM format refused: {e}"),
            (Ok(Some(_)), 0) => write!(st.summary, "searching"),
            (Ok(Some(_)), 1) => write!(
                st.summary,
                "receiving frames, mode {}, {}",
                d.mode.map_or("", HamMode::letter),
                d.qam.map_or("", Qam::name)
            ),
            (Ok(Some(_)), _) => write!(st.summary, "the transmission ended"),
        };
        p.results_mut().set_status(&self.status);
    }

    /// The finished files, newest first.
    pub fn history(&self) -> impl Iterator<Item = &FinishedFile> {
        self.history.iter()
    }
}

impl Component for Files {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "broadcast.hamdrm_files",
            vec![
                Port::typed::<HamFrame>("frames"),
                Port::typed::<HamState>("state"),
            ],
            vec![Port::typed::<Picture>("pictures").rate_from(RateFrom::Fixed(0.0))],
        )
        .with_settings(FormatSetting::schema())
    }
    fn configure(&mut self, _inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        Ok(vec![Format::new(Kind::typed::<Picture>(), 0.0)])
    }
    fn process(&mut self, io: &mut Io) {
        let mut pics = Vec::new();
        for f in io.items::<HamFrame>(0) {
            self.time = f.time;
            self.take(&f.value, &mut pics);
        }
        for s in io.items::<HamState>(1) {
            if s.value.state == 2 && self.d.state == 1 {
                let miss = self
                    .d
                    .current
                    .as_ref()
                    .map(|c| format!("{:?}", c.missing))
                    .unwrap_or_default();
                self.published
                    .results_mut()
                    .log(format!("transmission ended, missing {miss}"));
            }
            self.d.state = s.value.state;
            self.d.frames_good = s.value.good;
            self.d.frames_bad = s.value.bad;
            self.time = s.time;
        }
        if self.time - self.last_report >= 0.1 {
            self.last_report = self.time;
            self.publish();
        }
        let t = self.time;
        for p in pics {
            io.items_out::<Picture>(0).push(Timed { time: t, value: p });
        }
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        match FormatSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            FormatSetting::Format => {
                if self
                    .loaded
                    .apply(id, COMPONENT_FORMAT.read(id, v)?, HamFormat::load)?
                {
                    self.format_text = format_text(&self.loaded);
                    self.publish();
                }
            }
        }
        Ok(Applied::Live)
    }
    fn values(&self) -> Values {
        values_of([(
            FormatSetting::Format,
            Value::Text(self.loaded.path().into()),
        )])
    }
    fn results(&self) -> Option<&Results> {
        Some(self.published.results())
    }
    fn commands(&self) -> &Commands {
        ClearCommand::commands()
    }
    fn invoke(&mut self, id: CommandId, _args: &Values) -> Result<Option<Record>, CommandError> {
        match ClearCommand::from_id(id).ok_or(CommandError::NoSuchId { id })? {
            ClearCommand::Clear => {
                self.partial.clear();
                self.history.clear();
                self.history_changed = true;
                self.d.current = None;
                self.d.files_completed = 0;
                self.publish();
            }
        }
        Ok(None)
    }
    /// The files of the frequency before that are not complete go, with the frame values and counts. The finished
    /// files and their count stay if `kept` holds them.
    fn forget(&mut self, kept: &Kept) {
        let completed = self.d.files_completed;
        self.partial.clear();
        self.d = HamDetail::default();
        if kept.table(HISTORY.key) {
            if kept.field(HamdrmFields::FILES_COMPLETED.key) {
                self.d.files_completed = completed;
            }
        } else {
            self.history.clear();
            self.history_changed = true;
        }
        self.publish();
    }
}

/// The HamDRM graph with the format that the components start with (or why it did not load).
///
/// # Errors
///
/// [`GraphError`] if the graph does not build: a fault of this crate, never of the input.
pub fn graph(loaded: &LoadedFormat, catalog: &Arc<Catalog>) -> Result<Graph, GraphError> {
    let setting = |key: &str, error| GraphError::Setting {
        key: key.into(),
        error,
    };
    // The upper sideband: the carriers from 150 Hz below carrier 0 (at DC_HZ above the dial) to 2700 Hz above it.
    let front = crate::component::channel(3000.0, RATE)
        .and_then(|c| c.with_offset(DC_HZ))
        .map_err(|e| setting("front", e))?;
    let band = FirFilter::bandpass(-150.0, 2700.0, 250.0).map_err(|e| setting("band", e))?;
    GraphBuilder::new()
        .add("front", front)
        .add("band", band)
        .add("frames", Frames::new(loaded.clone()))
        .add("files", Files::new(loaded.clone()))
        .input("front")
        .connect("front", "band")
        .connect("band", "frames")
        .connect("frames.frames", "files.frames")
        .connect("frames.state", "files.state")
        .output("files")
        .status_from("files")
        .catalog(catalog)
        .build()
}

/// The HamDRM receiver: its graph in a [`StackGraph`]. The format of the `hamdrm_format` setting is loaded once and shared by
/// the components. A retune forgets the signal. Received segments and files stay.
///
/// # Errors
///
/// [`GraphError::Setting`] if `cfg` holds a value that a component refuses, or another [`GraphError`] if the graph does not
/// build. A format file that does not load is not an error: the `format` field says why.
pub fn receiver(cfg: &StackConfig) -> Result<StackGraph, GraphError> {
    let loaded = LoadedFormat::load(cfg.settings.get(&FORMAT_FILE), HamFormat::load);
    StackGraph::of(crate::typed::shape(ID), graph(&loaded, &Catalog::of(cfg)?)?)
        .min_rate(MIN_RATE, "HamDRM needs a sample rate of at least 12 ksps")
        .route("hamdrm_format", &["frames.format", "files.format"])?
        .command("clear", sdrkit_graph::CommandTarget::of("files", "clear"))
        .on_retune(sdrkit_graph::CommandTarget::of("frames", "clear"))?
        .with_settings(&cfg.settings)
        .build()
}

/// Makes the receiver.
pub fn make_receiver(cfg: &StackConfig) -> Result<Box<dyn Receiver>, ConfigError> {
    Ok(Box::new(receiver(cfg)?))
}

/// The format file.
pub const FORMAT_FILE: Setting<field::Text> = Setting::new("hamdrm_format", "HamDRM format", "", "")
    .help("A JSON file with the parts of the HamDRM format that no document fixes. Without it nothing is decoded.")
    .restart();

sdrkit_core::choice_enum! {
    /// The occupancy.
    #[derive(Default)]
    pub enum Occupancy {
        /// 2.3 kHz.
        Narrow = ("2.3", "2.3 kHz"),
        /// 2.5 kHz.
        #[default]
        Wide = ("2.5", "2.5 kHz"),
    }
}

sdrkit_core::choice_enum! {
    /// The protection level.
    #[derive(Default)]
    pub enum ProtectionLevel {
        /// Level 0.
        #[default]
        P0 = ("0", "0"),
        /// Level 1.
        P1 = ("1", "1"),
    }
}

/// Test signal: the mode.
pub const GEN_MODE: Setting<field::Choice<HamMode>> =
    Setting::choice("gen_mode", "Mode", HamMode::B)
        .section("Test signal")
        .advanced();
/// Test signal: the occupancy.
pub const GEN_OCCUPANCY: Setting<field::Choice<Occupancy>> =
    Setting::choice("gen_occupancy", "Occupancy", Occupancy::Wide)
        .section("Test signal")
        .advanced();
/// Test signal: the constellation.
pub const GEN_QAM: Setting<field::Choice<Qam>> = Setting::choice("gen_qam", "QAM", Qam::Q16)
    .section("Test signal")
    .advanced();
/// Test signal: the protection level.
pub const GEN_PROTECTION: Setting<field::Choice<ProtectionLevel>> =
    Setting::choice("gen_protection", "Protection", ProtectionLevel::P0)
        .help("4-QAM has protection 1 only.")
        .section("Test signal")
        .advanced();
/// Test signal: the interleaver.
pub const GEN_INTERLEAVE: Setting<field::Choice<Interleave>> =
    Setting::choice("gen_interleave", "Interleaver", Interleave::Short)
        .section("Test signal")
        .advanced();
/// Test signal: leave out every n-th body segment.
pub const GEN_SKIP_EVERY: Setting<field::I64> =
    Setting::new("gen_skip_every", "Leave out every n-th body segment", "", 0)
        .range(0.0, 100.0, 1.0)
        .section("Test signal")
        .advanced();
/// Test signal: transmissions per round.
pub const GEN_TRANSMISSIONS: Setting<field::I64> =
    Setting::new("gen_transmissions", "Transmissions per round", "", 2)
        .range(1.0, 10.0, 1.0)
        .section("Test signal")
        .advanced();

sdrkit_core::settings! {
    /// The settings of the HamDRM stack: the format file, then the test signal's.
    pub enum HamSetting {
        /// The format file.
        Format = FORMAT_FILE,
        /// Test signal: the mode.
        GenMode = GEN_MODE,
        /// Test signal: the occupancy.
        GenOccupancy = GEN_OCCUPANCY,
        /// Test signal: the constellation.
        GenQam = GEN_QAM,
        /// Test signal: the protection level.
        GenProtection = GEN_PROTECTION,
        /// Test signal: the interleaver.
        GenInterleave = GEN_INTERLEAVE,
        /// Test signal: leave out every n-th body segment.
        GenSkipEvery = GEN_SKIP_EVERY,
        /// Test signal: transmissions per round.
        GenTransmissions = GEN_TRANSMISSIONS,
    }
}

/// The test picture: a 64 x 48 JPEG of a colour gradient, a chequer pattern and a white bar.
pub fn test_picture() -> Vec<u8> {
    let mut px = Vec::with_capacity(64 * 48 * 3);
    for y in 0..48usize {
        for x in 0..64usize {
            if (20..24).contains(&y) {
                px.extend_from_slice(&[255, 255, 255]);
            } else if y > 30 {
                let c = if (x / 8 + y / 8) % 2 == 0 { 0 } else { 200 };
                px.extend_from_slice(&[c, c, c]);
            } else {
                px.extend_from_slice(&[(x * 4) as u8, (y * 5) as u8, 128]);
            }
        }
    }
    sdrkit_codec_image::Raster::from_rgb(64, 48, &px)
        .and_then(|r| {
            sdrkit_codec_image::jpeg::encode(&r, 90, sdrkit_codec_image::jpeg::Subsampling::S444)
        })
        .unwrap_or_default()
}

/// The test station's call sign, sent in the FAC label.
pub const TEST_CALL: &str = "N0CALL";
/// The test file's name.
pub const TEST_NAME: &str = "test.jpg";
/// Bytes of a MOT body segment of the generator.
pub const SEGMENT_BYTES: usize = 200;
/// The packet id of the generator.
const PACKET_ID: u8 = 0;
/// The transport id of the generator's file.
const TRANSPORT_ID: u16 = 1;

/// The generator's options.
#[derive(Clone, Debug)]
pub struct GenOptions {
    /// Mode.
    pub mode: HamMode,
    /// 2.5 kHz.
    pub wide: bool,
    /// QAM.
    pub qam: Qam,
    /// Protection level (0 or 1; 4-QAM has 1 only).
    pub protection: u8,
    /// The long interleaver (2 s).
    pub long_interleave: bool,
    /// Leave out segments n-1, 2n-1, ... in the first transmission of a round.
    pub skip_every: Option<u16>,
    /// Transmissions per round.
    pub transmissions: u32,
    /// The format. Without it there is no signal.
    pub format: Option<Arc<HamFormat>>,
}

impl Default for GenOptions {
    fn default() -> Self {
        GenOptions {
            mode: HamMode::B,
            wide: true,
            qam: Qam::Q16,
            protection: 0,
            long_interleave: false,
            skip_every: None,
            transmissions: 2,
            format: None,
        }
    }
}

/// One step of the generator's plan.
enum Step {
    /// A frame with these MSC packet bytes.
    Frame(Vec<u8>),
    /// Silence, in samples at 12 kHz.
    Silence(usize),
}

/// The HamDRM test signal: USB audio as complex baseband at the output rate (the audio at positive frequencies above the dial).
pub struct HamGenerator {
    o: GenOptions,
    fmt: Arc<HamFormat>,
    out_rate: f64,
    modu: FrameModulator,
    pilots: Vec<Complex32>,
    fac: FacCode,
    fac_cells: Vec<usize>,
    msc_cells: Vec<usize>,
    mlc: Mlc,
    il: CellInterleaver,
    writer: DrmPacketWriter,
    per_frame: usize,
    queue: VecDeque<Step>,
    frame_id: u8,
    pending: VecDeque<Complex32>,
    nco: Nco,
    rs: Option<sdrkit_algo_dsp::resample::Resampler<Complex32>>,
    file: Vec<u8>,
    tmp: Vec<Complex32>,
}

impl HamGenerator {
    /// A generator at `out_rate`. `None` without a format, for a rate it cannot reach, for a constellation and protection
    /// that the mode does not have, or when a packet of the format does not fit a frame.
    pub fn new(o: GenOptions, out_rate: f64) -> Option<HamGenerator> {
        let fmt = o.format.clone()?;
        let map = fmt.map(o.mode, o.wide);
        let mut pilots = vec![Complex32::default(); map.nsym() * map.carriers()];
        for s in 0..map.nsym() {
            for c in 0..map.carriers() {
                if let Some(v) = fmt.pilot(map, s, c) {
                    pilots[s * map.carriers() + c] = v;
                }
            }
        }
        let msc_cells = map.cells_of(CellKind::Msc);
        let mlc = frame::msc_code(o.qam, o.protection, msc_cells.len())?;
        let per_frame = mlc.info_bits() / 8 / fmt.packet_bytes();
        if per_frame == 0 {
            return None;
        }
        let rs = if out_rate != RATE {
            Some(
                sdrkit_algo_dsp::resample::Resampler::with_quality(RATE, out_rate, 0.3, 70.0)
                    .ok()?,
            )
        } else {
            None
        };
        let depth = if o.long_interleave { LONG_DEPTH } else { 1 };
        Some(HamGenerator {
            out_rate,
            modu: FrameModulator::new(fmt.layout(o.mode, o.wide)),
            pilots,
            fac: fmt.fac_code(),
            fac_cells: map.cells_of(CellKind::Fac),
            il: CellInterleaver::new(msc_cells.len(), depth),
            msc_cells,
            mlc,
            writer: DrmPacketWriter::new(PACKET_ID, fmt.packet_bytes()).ok()?,
            per_frame,
            queue: VecDeque::new(),
            frame_id: 0,
            pending: VecDeque::new(),
            nco: Nco::new(RATE, DC_HZ),
            rs,
            file: test_picture(),
            tmp: Vec::new(),
            fmt,
            o,
        })
    }

    /// The options of a generator configuration (the stack's `gen_*` test signal settings and `hamdrm_format`).
    ///
    /// # Errors
    ///
    /// [`FormatError`] if the format file does not load.
    pub fn options(cfg: &GenConfig) -> Result<GenOptions, FormatError> {
        let s = &cfg.settings;
        let qam = s.get(&GEN_QAM);
        Ok(GenOptions {
            mode: s.get(&GEN_MODE),
            wide: s.get(&GEN_OCCUPANCY) == Occupancy::Wide,
            qam,
            protection: if qam == Qam::Q4 {
                1
            } else {
                u8::from(s.get(&GEN_PROTECTION) == ProtectionLevel::P1)
            },
            long_interleave: s.get(&GEN_INTERLEAVE) == Interleave::Long,
            skip_every: u16::try_from(s.get(&GEN_SKIP_EVERY))
                .ok()
                .filter(|&n| n > 0),
            transmissions: u32::try_from(s.get(&GEN_TRANSMISSIONS).max(1)).unwrap_or(1),
            format: load_format(s.get(&FORMAT_FILE))?,
        })
    }

    /// The file it sends.
    pub fn file(&self) -> &[u8] {
        &self.file
    }
    /// MSC packets per frame.
    pub fn packets_per_frame(&self) -> usize {
        self.per_frame
    }
    /// Body segments of the file.
    pub fn segments(&self) -> usize {
        self.file.len().div_ceil(SEGMENT_BYTES)
    }
    /// Frames of transmission `t` of a round (0 is the first), the flush of the long interleaver included.
    pub fn transmission_frames(&self, t: u32) -> usize {
        self.transmission(t, &mut self.writer.clone()).len()
    }

    /// The data groups of one MOT object part: the header or the body of the test file (EN 301 234 clause 5).
    fn groups(&self, kind: u8, data: &[u8], seg_bytes: usize, skip: Option<u16>) -> Vec<Vec<u8>> {
        let segs: Vec<&[u8]> = data.chunks(seg_bytes).collect();
        let n = segs.len();
        let mut out = Vec::new();
        for (i, c) in segs.into_iter().enumerate() {
            if skip.is_some_and(|k| (i as u16 + 1).is_multiple_of(k)) {
                continue;
            }
            let mut seg = Vec::with_capacity(c.len() + 2);
            let mut g = Vec::new();
            let h = Header {
                group_type: kind,
                segment: Some((i + 1 == n, i as u16)),
                transport_id: Some(TRANSPORT_ID),
                ..Header::default()
            };
            if mot::write_segment(0, c, &mut seg).is_ok() && h.write(&seg, &mut g).is_ok() {
                out.push(g);
            }
        }
        out
    }

    /// The frames (their packet bytes) of transmission `t`: the header twice, the body, the header again.
    fn transmission(&self, t: u32, w: &mut DrmPacketWriter) -> Vec<Vec<u8>> {
        let mut hb = Vec::new();
        let _ = HeaderWriter::new(self.file.len() as u32, 2, 1)
            .content_name(TEST_NAME)
            .write(&mut hb);
        let header = self.groups(datagroup::MOT_HEADER, &hb, hb.len().max(1), None);
        let skip = if t == 0 { self.o.skip_every } else { None };
        let body = self.groups(datagroup::MOT_BODY, &self.file, SEGMENT_BYTES, skip);
        let mut stream = Vec::new();
        for g in header.iter().chain(&header).chain(&body).chain(&header) {
            let _ = w.write_unit(g, &mut stream);
        }
        let pb = self.fmt.packet_bytes();
        let fb = self.per_frame * pb;
        let mut frames: Vec<Vec<u8>> = stream.chunks(fb).map(|c| c.to_vec()).collect();
        if let Some(last) = frames.last_mut() {
            while last.len() < fb {
                w.write_padding(last);
            }
        }
        // The long interleaver spreads the last frames over the next four: send them.
        if self.o.long_interleave {
            for _ in 1..LONG_DEPTH {
                let mut f = Vec::with_capacity(fb);
                while f.len() < fb {
                    w.write_padding(&mut f);
                }
                frames.push(f);
            }
        }
        frames
    }

    fn plan_round(&mut self) {
        self.queue.push_back(Step::Silence((0.3 * RATE) as usize));
        let mut w = self.writer.clone();
        for t in 0..self.o.transmissions {
            for f in self.transmission(t, &mut w) {
                self.queue.push_back(Step::Frame(f));
            }
            self.queue.push_back(Step::Silence((2.0 * RATE) as usize));
        }
        self.writer = w;
    }

    fn label(&self, frame_id: usize) -> [u8; LABEL_CHARS] {
        let mut l = [b' '; LABEL_CHARS];
        for (i, c) in l.iter_mut().enumerate() {
            if let Some(&b) = TEST_CALL.as_bytes().get(frame_id * LABEL_CHARS + i) {
                *c = b;
            }
        }
        l
    }

    fn frame(&mut self, packets: &[u8]) {
        let mut grid = self.pilots.clone();
        let info = FrameInfo {
            frame_id: self.frame_id,
            wide: self.o.wide,
            long_interleave: self.o.long_interleave,
            qam: self.o.qam,
            protection: self.o.protection,
            content: Content::Data {
                packet_id: PACKET_ID,
            },
            label: self.label(self.frame_id as usize),
        };
        self.frame_id = (self.frame_id + 1) % FRAMES_PER_SUPERFRAME as u8;
        let mut fc = vec![Complex32::default(); frame::FAC_CELLS];
        self.fac.encode(&info.bits(self.fmt.values()), &mut fc);
        for (&i, &v) in self.fac_cells.iter().zip(&fc) {
            grid[i] = v;
        }
        let mut bits = Vec::new();
        unpack(packets, BitOrder::Msb, &mut bits);
        bits.resize(self.mlc.info_bits(), 0);
        disperse(&mut bits);
        let n = self.msc_cells.len();
        let mut dc = vec![Complex32::default(); n];
        self.mlc.encode(&bits, &mut dc);
        let mut x = Vec::with_capacity(n);
        self.il.push(&dc, &mut x, |_| Qam::Q4.map(0, 0));
        for (&i, &v) in self.msc_cells.iter().zip(&x) {
            grid[i] = v;
        }
        self.tmp.clear();
        self.modu.frame(&grid, &mut self.tmp);
        // A mean power of 1 while it transmits.
        let p = self.tmp.iter().map(|s| s.norm_sqr()).sum::<f32>() / self.tmp.len().max(1) as f32;
        let gain = 1.0 / p.max(1e-12).sqrt();
        for s in &mut self.tmp {
            *s *= gain;
        }
    }
}

impl Generator for HamGenerator {
    fn rate_hz(&self) -> f64 {
        self.out_rate
    }
    fn generate(&mut self, out: &mut [Complex32]) -> usize {
        while self.pending.len() < out.len() {
            if self.queue.is_empty() {
                self.plan_round();
            }
            match self.queue.pop_front() {
                Some(Step::Frame(p)) => self.frame(&p),
                Some(Step::Silence(n)) => {
                    // A gap: the next transmission starts a new super frame and a new interleaver.
                    self.frame_id = 0;
                    let depth = if self.o.long_interleave {
                        LONG_DEPTH
                    } else {
                        1
                    };
                    self.il = CellInterleaver::new(self.msc_cells.len(), depth);
                    self.tmp.clear();
                    self.tmp.resize(n, Complex32::default());
                }
                None => break,
            }
            // Move to the audio position (USB: positive frequencies above the dial).
            let mut x = std::mem::take(&mut self.tmp);
            self.nco.mix_in_place(&mut x);
            match &mut self.rs {
                Some(r) => {
                    let mut y = Vec::new();
                    r.process(&x, &mut y);
                    self.pending.extend(y);
                }
                None => self.pending.extend(x.iter()),
            }
            self.tmp = x;
        }
        for o in out.iter_mut() {
            *o = self.pending.pop_front().unwrap_or_default();
        }
        out.len()
    }
    fn signal_info(&self) -> Option<SignalInfo> {
        Some(SignalInfo::new(if self.o.wide { 2500.0 } else { 2300.0 }))
    }
}

/// Makes the generator.
///
/// # Errors
///
/// [`ConfigError::Stage`] without the format or for a rate it cannot reach.
pub fn make_generator(cfg: &GenConfig) -> Result<Box<dyn Generator>, ConfigError> {
    match HamGenerator::options(cfg)
        .ok()
        .and_then(|o| HamGenerator::new(o, cfg.rate_hz))
    {
        Some(g) => Ok(Box::new(g)),
        None => Err(crate::silence::NoSignal {
            rate_hz: cfg.rate_hz,
        }
        .refused()),
    }
}
