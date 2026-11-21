//! What the stacks on a DVB-S2 carrier share: the settings of the DVB-S2 receiver of `sdrkit-proto-dvbs2`, the hint below
//! its least input rate, the CADUs in the data fields of a generic stream (frames, virtual channels and packets), and the
//! test-signal transmitter around the DVB-S2 generator.

use super::notes::set_status;
use crate::components::last;
use crate::items::WholePacket;
use sdrkit_core::publish::{field, Published};
use sdrkit_core::{
    Complex32, ConfigError, Generator, Setting, SettingError, SettingId, SettingSet, SignalInfo,
    StageError, Value, Values,
};
use sdrkit_graph::{
    values_of, Applied, Component, ComponentInfo, Format, Io, Item, Kind, Port, RateFrom, Timed,
};
use sdrkit_proto_ccsds::aos::Layout;
use sdrkit_proto_ccsds::cadufind::CaduFinder;
use sdrkit_proto_ccsds::packet::{Assembler, AssemblerConfig};
use sdrkit_proto_ipcast::field::DataField;
use sdrkit_proto_mpegts::source::PacketSource;
use sdrkit_proto_dvbs2::gen::{DataFieldSource, DvbsGenParams, DvbsGenerator, GenStandard};
use sdrkit_proto_dvbs2::modcod::ModCod;
use sdrkit_proto_dvbs2::settings::RollOffChoice;
pub use sdrkit_proto_dvbs2::settings::{
    FRAME_LENGTH, MODCOD, MODCODS, MODCOD_RULE, PILOTS_RULE, PL_SCRAMBLING,
};
pub use sdrkit_proto_dvbs2::{
    attach, highest_symbol_rate, least_input_rate, routes, Dvbs2Parts, LinkState,
};
use std::collections::VecDeque;
use std::fmt::Write as _;
use std::sync::Arc;

/// The section of the DVB-S2 settings.
pub const DVBS2_SECTION: &str = "DVB-S2";
/// The section of the test-signal settings.
pub const TEST_SIGNAL: &str = "Test signal";

/// The carrier to decode as an offset from the centre in Hz; empty for the carrier nearest the centre.
pub const CARRIER_OFFSET: Setting<field::Text> =
    Setting::new("carrier_offset", "Carrier", "Hz", "")
        .help("Offset in Hz from the centre. Empty: the carrier nearest the centre.")
        .section(DVBS2_SECTION);

/// A symbol-rate setting with `default` (0: found from the spectrum).
pub const fn symbol_rate(default: f64) -> Setting<field::Hz> {
    Setting::new("symbol_rate", "Symbol rate", "Hz", default)
        .range(0.0, 60e6, 0.0)
        .zero_means("Found from the spectrum")
        .section(DVBS2_SECTION)
}

/// A roll-off setting with `default`.
pub const fn roll_off(default: RollOffChoice) -> Setting<field::Choice<RollOffChoice>> {
    Setting::choice("roll_off", "Roll-off", default).section(DVBS2_SECTION)
}

/// The hint below the least input rate of `symbol_rate` at `input_rate`, else `None`.
pub fn rate_hint(input_rate: f64, symbol_rate: f64) -> Option<String> {
    let need = least_input_rate(symbol_rate);
    (input_rate > 0.0 && symbol_rate > 0.0 && input_rate < need).then(|| {
        format!(
            "This link needs {:.1} Msps or more (a recording works).",
            need / 1e6
        )
    })
}

/// The DVB-S2 test signal of a stack: the DVB-S2 transmitter of `sdrkit-proto-dvbs2` with roll-off `roll_off`, MODCOD
/// `modcod`, normal frames, pilots when `pilots`, carrying either transport packets from `ts` or, when `data` is given, a
/// generic continuous stream.
///
/// # Errors
///
/// [`ConfigError`] when the MODCOD has no code or the symbol rate does not fit the sample rate.
pub fn transmitter(
    rate: f64,
    symbol_rate: f64,
    roll_off: f64,
    modcod: u8,
    pilots: bool,
    ts: Box<dyn PacketSource>,
    data: Option<Box<dyn DataFieldSource>>,
) -> Result<S2Transmitter, ConfigError> {
    let m = ModCod::new(modcod).ok_or_else(|| stage_error("no such MODCOD"))?;
    if symbol_rate <= 0.0 || symbol_rate > rate / 1.1 {
        return Err(stage_error("the symbol rate does not fit the sample rate"));
    }
    let mut p = DvbsGenParams::new(rate);
    p.standard = GenStandard::S2 {
        modcod: m,
        short: false,
        pilots,
    };
    p.symbol_rate = symbol_rate;
    p.rolloff = roll_off;
    p.source = ts;
    p.data = data;
    Ok(S2Transmitter(DvbsGenerator::new(p)))
}

fn stage_error(text: &'static str) -> ConfigError {
    ConfigError::Stage(StageError::new(Message(text)))
}

#[derive(Debug)]
struct Message(&'static str);
impl std::fmt::Display for Message {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.0)
    }
}
impl std::error::Error for Message {}

/// The DVB-S2 test signal made by [`transmitter`].
pub struct S2Transmitter(DvbsGenerator);

impl Generator for S2Transmitter {
    fn rate_hz(&self) -> f64 {
        self.0.rate_hz()
    }
    fn generate(&mut self, out: &mut [Complex32]) -> usize {
        self.0.generate(out)
    }
    fn signal_info(&self) -> Option<SignalInfo> {
        self.0.signal_info()
    }
}

/// A generic continuous stream of CADUs for [`transmitter`]: `next` appends the next CADUs to its argument when the bytes
/// run short.
pub struct CaduStream<F: FnMut(&mut Vec<u8>) + Send> {
    queue: VecDeque<u8>,
    tmp: Vec<u8>,
    next: F,
}

impl<F: FnMut(&mut Vec<u8>) + Send> CaduStream<F> {
    /// A stream made by `next`.
    pub fn new(next: F) -> Self {
        CaduStream {
            queue: VecDeque::new(),
            tmp: Vec::new(),
            next,
        }
    }
}

impl<F: FnMut(&mut Vec<u8>) + Send> DataFieldSource for CaduStream<F> {
    fn fill(&mut self, df: &mut [u8]) {
        while self.queue.len() < df.len() {
            self.tmp.clear();
            (self.next)(&mut self.tmp);
            if self.tmp.is_empty() {
                break;
            }
            self.queue.extend(self.tmp.iter());
        }
        for b in df.iter_mut() {
            *b = self.queue.pop_front().unwrap_or(0);
        }
    }
}

// ---------------------------------------------------------------------------------------------------------------------------------
// CADUs in data fields.

/// Bytes of the CADUs in a data field, marker included.
pub const CADU_BYTES: usize = 2048;

/// The rules of [`FieldFramesComponent`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FieldFrames {
    /// Each frame ends with a CRC-16/IBM-3740 over the rest of the frame; a frame that fails it is dropped.
    pub frame_crc: bool,
    /// The virtual channels whose packets are reassembled (bit k: channel k). Others are counted only.
    pub channels: u64,
    /// Bytes of a secondary header that the packet length field does not count.
    pub uncounted_secondary: u16,
}

/// Counters of [`FieldFramesComponent`].
#[derive(Clone, Debug, Default, PartialEq)]
pub struct FieldFramesReport {
    /// Data fields taken.
    pub fields: u64,
    /// Data fields after a loss.
    pub breaks: u64,
    /// CADUs found.
    pub cadus: u64,
    /// Frames with a bad CRC (when checked).
    pub crc_bad: u64,
    /// Frames of each virtual channel.
    pub per_vc: Vec<u64>,
    /// Spacecraft id of the last frame.
    pub spacecraft: Option<u8>,
    /// Virtual channel of the last frame.
    pub vcid: Option<u8>,
    /// Packets given out.
    pub packets: u64,
    /// Packets lost: a frame count gap or a damaged header.
    pub packets_lost: u64,
    /// Signal time of the last CADU.
    pub last_cadu_s: Option<f64>,
}

impl Item for FieldFramesReport {
    const KIND: &'static str = "satimg.field_frames_report";
}

/// The virtual channels whose packets are read: numbers with commas between them; empty for every channel but 63 (fill).
pub const CHANNELS: Setting<field::Text> = Setting::new("channels", "Virtual channels", "", "")
    .help("Virtual channel numbers with commas between them. Empty: every channel but 63 (fill).")
    .advanced();
/// Bytes of a packet secondary header that the length field does not count.
pub const UNCOUNTED_SECONDARY: Setting<field::I64> =
    Setting::new("uncounted_secondary", "Secondary header outside the length", "bytes", 0)
        .range(0.0, 64.0, 1.0)
        .help("A packet with its secondary header flag set is this much longer than its length field says. 0 follows CCSDS 133.0.")
        .advanced();

sdrkit_core::settings! {
    /// The settings of [`FieldFramesComponent`].
    pub enum FieldFramesSetting {
        /// The virtual channels read.
        Channels = CHANNELS,
        /// The secondary header bytes outside the length field.
        UncountedSecondary = UNCOUNTED_SECONDARY,
    }
}

/// The channel mask of the text of setting `id` (`channels`): empty text is every channel but 63.
///
/// # Errors
///
/// [`SettingError::Malformed`] if a part is not a number from 0 to 63.
pub fn parse_channels(id: SettingId, text: &str) -> Result<u64, SettingError> {
    let mut m = 0u64;
    for p in text
        .split(|c: char| c == ',' || c.is_whitespace())
        .filter(|p| !p.is_empty())
    {
        let v: u8 = p
            .parse()
            .ok()
            .filter(|&v| v < 64)
            .ok_or(SettingError::Malformed { id, at: Some(0) })?;
        m |= 1 << v;
    }
    Ok(if m == 0 { !(1u64 << 63) } else { m })
}

/// Data fields in, whole space packets out: the byte-aligned CADUs of the joined data fields (2048 bytes), their AOS frames, and
/// the packets of the chosen virtual channels. A data field after a loss empties the stream.
pub struct FieldFramesComponent {
    channels_text: String,
    rules: FieldFrames,
    finder: CaduFinder,
    asm: Assembler,
    report: FieldFramesReport,
    out: Vec<Timed<WholePacket>>,
    cadus: Vec<Vec<u8>>,
}

impl FieldFramesComponent {
    /// A component with `rules`.
    pub fn new(rules: FieldFrames) -> Self {
        FieldFramesComponent {
            channels_text: String::new(),
            rules,
            finder: CaduFinder::new(CADU_BYTES),
            asm: Assembler::new(AssemblerConfig {
                uncounted_secondary: rules.uncounted_secondary,
                ..AssemblerConfig::default()
            }),
            report: FieldFramesReport {
                per_vc: vec![0; 64],
                ..Default::default()
            },
            out: Vec::new(),
            cadus: Vec::new(),
        }
    }

    /// Takes one CADU at signal time `time`.
    fn cadu(&mut self, c: &[u8], time: f64) {
        self.report.cadus += 1;
        self.report.last_cadu_s = Some(time);
        let layout = Layout::CADU_2048;
        let Ok(f) = layout.parse(&c[4..]) else {
            return;
        };
        if self.rules.frame_crc && f.fecf_ok != Some(true) {
            self.report.crc_bad += 1;
            return;
        }
        let vc = f.header.vcid.min(63);
        self.report.per_vc[usize::from(vc)] += 1;
        self.report.spacecraft = Some(f.header.spacecraft);
        self.report.vcid = Some(vc);
        if self.rules.channels >> vc & 1 == 0 {
            return;
        }
        let spacecraft = f.header.spacecraft;
        let out = &mut self.out;
        self.asm
            .push_mpdu(vc, Some(f.header.counter), f.first_header, f.zone, |p| {
                let Ok(h) = p.header.bytes() else {
                    return;
                };
                let mut bytes = Vec::with_capacity(6 + p.data.len());
                bytes.extend_from_slice(&h);
                bytes.extend_from_slice(p.data);
                out.push(Timed {
                    time,
                    value: WholePacket {
                        vcid: p.vcid,
                        spacecraft,
                        bytes: Arc::from(bytes),
                        gap_before: p.gap_before,
                        time,
                    },
                });
            });
    }
}

impl Component for FieldFramesComponent {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "satimg.field_frames",
            vec![Port::typed::<DataField>("in")],
            vec![
                Port::typed::<WholePacket>("packets").rate_from(RateFrom::Fixed(0.0)),
                Port::typed::<FieldFramesReport>("report").rate_from(RateFrom::Fixed(0.0)),
            ],
        )
        .with_settings(FieldFramesSetting::schema())
    }
    /// The frames and the packets counted are of the carrier of the frequency before.
    fn forget(&mut self, _: &sdrkit_graph::Kept) {
        let channels_text = std::mem::take(&mut self.channels_text);
        *self = FieldFramesComponent::new(self.rules);
        self.channels_text = channels_text;
    }
    fn configure(&mut self, _: &[Format]) -> Result<Vec<Format>, ConfigError> {
        self.finder.reset();
        self.asm.reset();
        Ok(vec![
            Format::new(Kind::typed::<WholePacket>(), 0.0),
            Format::new(Kind::typed::<FieldFramesReport>(), 0.0),
        ])
    }
    fn process(&mut self, io: &mut Io) {
        let mut cadus = std::mem::take(&mut self.cadus);
        for d in io.items::<DataField>(0) {
            let d = &d.value;
            self.report.fields += 1;
            if d.lost_before > 0 {
                self.report.breaks += 1;
                self.finder.reset();
            }
            let mut n = 0;
            self.finder.push(d.bytes(), |c| {
                if let Some(v) = cadus.get_mut(n) {
                    v.clear();
                    v.extend_from_slice(c);
                } else {
                    cadus.push(c.to_vec());
                }
                n += 1;
            });
            for c in &cadus[..n] {
                self.cadu(c, d.time);
            }
        }
        self.cadus = cadus;
        let st = self.asm.stats();
        self.report.packets = st.packets;
        self.report.packets_lost = st.lost;
        io.items_out::<WholePacket>(0).append(&mut self.out);
        if io.tick() {
            io.items_out::<FieldFramesReport>(1).push(Timed {
                time: 0.0,
                value: self.report.clone(),
            });
        }
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        match FieldFramesSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            FieldFramesSetting::Channels => {
                let t = CHANNELS.read(id, v)?;
                self.rules.channels = parse_channels(id, t)?;
                self.channels_text = t.to_owned();
            }
            FieldFramesSetting::UncountedSecondary => {
                self.rules.uncounted_secondary = UNCOUNTED_SECONDARY.read(id, v)? as u16;
                self.asm = Assembler::new(AssemblerConfig {
                    uncounted_secondary: self.rules.uncounted_secondary,
                    ..AssemblerConfig::default()
                });
            }
        }
        Ok(Applied::Live)
    }
    fn values(&self) -> Values {
        values_of([
            (
                FieldFramesSetting::Channels,
                Value::Text(self.channels_text.clone()),
            ),
            (
                FieldFramesSetting::UncountedSecondary,
                Value::I64(i64::from(self.rules.uncounted_secondary)),
            ),
        ])
    }
}

// ---------------------------------------------------------------------------------------------------------------------------------
// The common part of the displays.

/// The state of a CADU stack for its status: 3 when a CADU came in the last 3 s of signal, 1 when the DVB-S2 receiver is at
/// state 1 or more, else 0.
pub fn cadu_state(link: &LinkState, frames: Option<&FieldFramesReport>) -> u8 {
    let recent = frames
        .and_then(|f| f.last_cadu_s)
        .is_some_and(|t| link.time_s - t <= 3.0);
    if recent {
        3
    } else if link.state >= 1 {
        1
    } else {
        0
    }
}

/// Writes the status of a stack from its DVB-S2 link and `state`.
pub fn write_status(p: &mut Published, link: &LinkState, state: u8, summary: &str) {
    set_status(
        p.results_mut(),
        state,
        link.esn0_db.map(|v| v as f32),
        Some(link.level_dbfs as f32),
        Some(link.offset_hz),
        summary,
    );
}

/// The hint of a stack: the rate hint, else `other`.
pub fn hint(link: &LinkState, symbol_rate: f64, other: &str, out: &mut String) {
    out.clear();
    match rate_hint(link.input_rate, symbol_rate) {
        Some(h) => out.push_str(&h),
        None => out.push_str(other),
    }
}

/// The newest [`LinkState`] on input `i`.
pub fn link_of(io: &Io, i: usize) -> Option<LinkState> {
    last::<LinkState>(io, i).cloned()
}

/// The frames of each virtual channel as text: "5: 120, 6: 3".
pub fn per_vc_text(per_vc: &[u64], out: &mut String) {
    out.clear();
    for (vc, n) in per_vc.iter().enumerate().filter(|(_, n)| **n > 0) {
        if !out.is_empty() {
            out.push_str(", ");
        }
        let _ = write!(out, "{vc}: {n}");
    }
}

/// The fields every CADU stack publishes about its frames.
pub mod frame_fields {
    use super::*;

    /// The frame fields of `f` as a result set, with the text buffer `text`.
    pub fn set<'a>(f: &FieldFramesReport, text: &'a mut String) -> crate::typed::CaduFields<'a> {
        per_vc_text(&f.per_vc, text);
        crate::typed::CaduFields {
            cadus: f.cadus as i64,
            spacecraft: f.spacecraft.map(i64::from),
            vcid: f.vcid.map(i64::from),
            frames_per_vc: text.as_str(),
            space_packets: f.packets as i64,
            packets_lost: f.packets_lost as i64,
            field_breaks: f.breaks as i64,
        }
    }
}

// ---------------------------------------------------------------------------------------------------------------------------------
// The test content of the CADU links.

/// A space packet with an 8-byte secondary header and a CRC-32/ISO-HDLC (big-endian) over all the bytes before it. With
/// `counted` the length field counts the secondary header (CCSDS 133.0); without it the packet is 8 bytes longer than its
/// length field says.
pub fn crc_packet(
    apid: u16,
    flags: u8,
    seq: u16,
    secondary: &[u8; 8],
    user: &[u8],
    counted: bool,
) -> Vec<u8> {
    let after = user.len() + 4 + if counted { 8 } else { 0 };
    let mut p = Vec::with_capacity(18 + user.len());
    p.extend_from_slice(&(0x0800 | (apid & 0x7FF)).to_be_bytes());
    p.extend_from_slice(&((u16::from(flags & 3) << 14) | (seq & 0x3FFF)).to_be_bytes());
    p.extend_from_slice(&((after.max(1) - 1) as u16).to_be_bytes());
    p.extend_from_slice(secondary);
    p.extend_from_slice(user);
    let c = sdrkit_util_bits::crc::catalog::CRC32_ISO_HDLC.checksum(&p);
    p.extend_from_slice(&c.to_be_bytes());
    p
}

/// A 2048-byte CADU: the marker, then an AOS frame of [`Layout::CADU_2048`] with its FECF.
pub fn cadu_2048(
    spacecraft: u8,
    vcid: u8,
    counter: u32,
    first_header: sdrkit_proto_ccsds::tm::FirstHeader,
    zone: &[u8],
) -> Vec<u8> {
    let mut c = Vec::with_capacity(CADU_BYTES);
    c.extend_from_slice(sdrkit_proto_ccsds::marker::TM.bytes());
    c.extend(crate::gen::aos_frame(
        &Layout::CADU_2048,
        spacecraft,
        vcid,
        counter,
        first_header,
        zone,
    ));
    c
}

/// The packet zone bytes of a 2048-byte CADU.
pub const ZONE_BYTES: usize = 2034;
