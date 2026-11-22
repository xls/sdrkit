//! The ARDOP sender: frames at the stream rate (the leader, the sync symbol, the type symbols, and the 4FSK body or the PSK
//! and QAM body with its trailer),
//! the FEC broadcast of a text, the ID frame, the Ping, and the ARQ link of [`super::link`] as the calling station and, when
//! answering is on, as the called station. The stack's graph joins the receiver's frames to the sender.

use super::body::{self, Body};
use super::link::{Heard, Link, LinkError, LinkOut};
use super::{frame_type, type_symbols, ArdopDecoder, ArdopFrame, RsCodes, TxFrame};
use crate::typed::{
    ArdopCallArgs, ArdopLinkEvent, ArdopLinkState, ArdopTxFields, KeyboardSendArgs,
    KeyboardTxEvent, ARDOP_CONNECT, ARDOP_DISCONNECT, ARDOP_LINK, ARDOP_PING, ARDOP_SEND,
    ARDOP_SEND_ID, ARDOP_STOP, KEYBOARD_TX,
};
use sdrkit_algo_dsp::nco::Nco;
use sdrkit_core::command::{CommandError, CommandId, CommandSet, Commands};
use sdrkit_core::interface::Fields as _;
use sdrkit_core::publish::keys::Family as SourceFamily;
use sdrkit_core::publish::{field, Published};
use sdrkit_core::settings::DeclareSetting;
use sdrkit_core::{
    Choice as _, Clock, Complex32, ConfigError, Record, Results, Setting, SettingError, SettingId,
    SettingSet as _, StackConfig, StageError, Value, Values,
};
use sdrkit_graph::components::{HostStreams, TxSink};
use sdrkit_graph::{
    values_of, Applied, Catalog, CommandTarget, Component, ComponentInfo, DecoderComponent, Format,
    GraphBuilder, GraphError, Io, Kind, Port, RateFrom, RunContext, StackGraph, TagKind,
};
use sdrkit_util_bits::crc::cyclic::remainder;
use std::collections::VecDeque;
use std::sync::Arc;

/// The peak of the leader and the frame-type tones sent, of full scale. Each leader tone is at half of it. A PSK or QAM body
/// peaks at up to 1.96 times this (eight 16QAM carriers in phase), below full scale.
pub const TX_LEVEL: f32 = 0.5;
/// The silence at the start of a burst, seconds: time for the transmitter to switch.
pub const LEAD_S: f64 = 0.1;
/// The silence between two frames of one burst, seconds.
pub const GAP_S: f64 = 0.15;
/// The raised-cosine fade at each end of a frame, seconds.
pub const FADE_S: f64 = 0.001;
/// The most bytes of one FEC broadcast.
pub const MAX_FEC: usize = 1024;

/// What a symbol sends.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SymKind {
    /// Nothing.
    Quiet,
    /// The two leader tones, 1475 and 1525 Hz, each at half the level.
    Leader,
    /// One tone, Hz above the dial.
    Tone(f64),
    /// One 10 ms symbol on the carriers of [`body::carrier_hz`]: carrier c has the digit (`digits` >> 4c) & 15 of
    /// [`body::digits`], each at `scale` of the carrier peak.
    Carriers {
        /// The carriers, 1, 2, 4 or 8.
        carriers: u8,
        /// Four bits a carrier, carrier 0 lowest.
        digits: u32,
        /// The carrier scaling factor.
        scale: f32,
    },
}

/// One symbol of a frame: what it sends, its length, its sign, and whether the frame starts or ends in it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Sym {
    /// What it sends.
    pub kind: SymKind,
    /// The length, seconds.
    pub s: f64,
    /// +1 or -1.
    pub sign: f32,
    /// The first symbol of a frame: it rises.
    pub rise: bool,
    /// The last symbol of a frame: it falls.
    pub fall: bool,
}

/// The symbols of frame `f`. The leader is `leader_ms` / 20 symbols of 20 ms (5 to 50), the last of them the sync symbol;
/// symbol i before the sync has the sign (-1)^(N + i) and the sync symbol +1, the sign of the symbol before it. Then the ten
/// type symbols at 50 baud on 1425 + 50 s Hz with the signs +1, -1, ...; then each body byte as four symbols, most significant
/// 2-bit group first, signs +1, -1, +1, -1: at 100 baud on 1350 + 100 s Hz for 4FSK.500.100 and 4FSK.500.100S, else at 50
/// baud on 1425 + 50 s Hz. A PSK or QAM frame with a whole body has instead the phase start symbol (phase 0 on every carrier),
/// the data symbols of [`body::digits`] on all carriers at once, and the trailer: [`body::TRAILER_SYMBOLS`] symbols at
/// 1500 Hz, phase 0, with no carrier scaling. A PSK or QAM frame without a body ends after its type symbols.
pub fn frame_symbols(f: &TxFrame, out: &mut Vec<Sym>) {
    let first = out.len();
    let n = (f.leader_ms / 20).clamp(5, 50) as i32;
    for i in 0..n - 1 {
        out.push(sym(SymKind::Leader, 0.02, alt(n + i)));
    }
    out.push(sym(SymKind::Leader, 0.02, 1.0));
    for (i, s) in type_symbols(f.code, f.session).iter().enumerate() {
        out.push(sym(
            SymKind::Tone(1425.0 + 50.0 * f64::from(*s)),
            0.02,
            alt(i as i32),
        ));
    }
    if let Some(ft) = f.psk_body() {
        body_symbols(f, &ft, out);
    } else {
        fsk_symbols(f, out);
    }
    out[first].rise = true;
    if let Some(last) = out.last_mut() {
        last.fall = true;
    }
}

/// The phase start symbol, the data symbols and the trailer of a PSK or QAM body.
fn body_symbols(f: &TxFrame, ft: &super::FrameType, out: &mut Vec<Sym>) {
    let Some(b) = Body::of(ft) else { return };
    let nc = ft.carriers as usize;
    let scale = b.scale(ft.carriers) as f32;
    let carriers = ft.carriers as u8;
    let digits: Vec<Vec<u8>> = f
        .body
        .chunks(ft.block_len())
        .take(nc)
        .map(|blk| body::digits(b, blk))
        .collect();
    let cs = |digits: u32| {
        sym(
            SymKind::Carriers {
                carriers,
                digits,
                scale,
            },
            body::SYMBOL_S,
            1.0,
        )
    };
    out.push(cs(0));
    for k in 0..body::symbols(ft) {
        let d = digits
            .iter()
            .enumerate()
            .fold(0u32, |a, (c, v)| a | u32::from(v[k]) << (4 * c));
        out.push(cs(d));
    }
    for _ in 0..body::TRAILER_SYMBOLS {
        out.push(sym(
            SymKind::Carriers {
                carriers: 1,
                digits: 0,
                scale: 1.0,
            },
            body::SYMBOL_S,
            1.0,
        ));
    }
}

/// The 4FSK body.
fn fsk_symbols(f: &TxFrame, out: &mut Vec<Sym>) {
    let d = super::body_symbol_s(f.code);
    let (base, sp) = if d < 0.015 {
        (1350.0, 100.0)
    } else {
        (1425.0, 50.0)
    };
    for &b in &f.body {
        for k in 0..4 {
            let s = (b >> (6 - 2 * k)) & 3;
            out.push(sym(SymKind::Tone(base + sp * f64::from(s)), d, alt(k)));
        }
    }
}

/// The sign of symbol `i` of an alternating run: +1 for even `i`, else −1.
fn alt(i: i32) -> f32 {
    if i % 2 == 0 {
        1.0
    } else {
        -1.0
    }
}

fn sym(kind: SymKind, s: f64, sign: f32) -> Sym {
    Sym {
        kind,
        s,
        sign,
        rise: false,
        fall: false,
    }
}

/// Writes symbols as complex samples with the dial at 0 Hz: a symbol is a sine of phase zero at its start, as its analytic
/// signal (-j times the complex tone), times its sign; the leader is the sum of its two tones. Each tone has a whole number
/// and a half of cycles in its symbol, so alternating signs give a continuous phase and a repeated sign a reversal. Symbol k
/// starts at the sample nearest to the sum of the lengths before it. A carrier symbol is the sum of [`body::carrier_wave`] over
/// its carriers, each counted from the exact start time of the symbol, times its scale and 26 000 / 28 600 of the level. A
/// frame rises and falls by a raised cosine of [`FADE_S`] inside its first and last symbol. On the lower sideband the samples
/// are mirrored.
pub struct FrameWriter {
    rate: f64,
    level: f32,
    lower: bool,
    syms: VecDeque<Sym>,
    queued_s: f64,
    taken_s: f64,
    /// The symbol being written: its first sample, the sample after its last, and its exact start time, seconds.
    cur: Option<(Sym, u64, u64, f64)>,
    n: u64,
    osc: [Nco; 2],
    fade: u64,
    buf: Vec<Complex32>,
}

impl FrameWriter {
    /// A writer at `rate` with the peak `level`, mirrored when `lower`.
    pub fn new(rate: f64, level: f32, lower: bool) -> Self {
        FrameWriter {
            rate,
            level,
            lower,
            syms: VecDeque::new(),
            queued_s: 0.0,
            taken_s: 0.0,
            cur: None,
            n: 0,
            osc: [Nco::new(rate, 1475.0), Nco::new(rate, 1525.0)],
            fade: ((FADE_S * rate).round() as u64).max(1),
            buf: Vec::new(),
        }
    }

    /// Queues frame `f`.
    pub fn push_frame(&mut self, f: &TxFrame) {
        let mut v = Vec::new();
        frame_symbols(f, &mut v);
        for s in v {
            self.queued_s += s.s;
            self.syms.push_back(s);
        }
    }

    /// Queues `s` seconds of silence.
    pub fn push_quiet(&mut self, s: f64) {
        self.queued_s += s;
        self.syms.push_back(sym(SymKind::Quiet, s, 1.0));
    }

    /// The sample after the last one queued.
    pub fn end_sample(&self) -> u64 {
        (self.queued_s * self.rate).round() as u64
    }

    /// The samples written.
    pub fn written(&self) -> u64 {
        self.n
    }

    /// Whether everything queued is written.
    pub fn finished(&self) -> bool {
        self.n >= self.end_sample()
    }

    /// Writes up to `out.len()` samples of what is queued and returns how many.
    pub fn fill(&mut self, out: &mut [Complex32]) -> usize {
        let mut k = 0;
        while k < out.len() {
            let (s, start, end, t_start) = match self.cur {
                Some(c) if self.n < c.2 => c,
                _ => {
                    let Some(s) = self.syms.pop_front() else {
                        break;
                    };
                    let start = (self.taken_s * self.rate).round() as u64;
                    let t_start = self.taken_s;
                    self.taken_s += s.s;
                    let end = (self.taken_s * self.rate).round() as u64;
                    match s.kind {
                        SymKind::Quiet | SymKind::Carriers { .. } => {}
                        SymKind::Leader => {
                            self.osc[0].set_freq(1475.0);
                            self.osc[1].set_freq(1525.0);
                            self.osc[0].set_phase_cycles(0.75);
                            self.osc[1].set_phase_cycles(0.75);
                        }
                        SymKind::Tone(f) => {
                            self.osc[0].set_freq(f);
                            self.osc[0].set_phase_cycles(0.75);
                        }
                    }
                    self.cur = Some((s, start, end, t_start));
                    continue;
                }
            };
            let m = ((end - self.n) as usize).min(out.len() - k);
            let o = &mut out[k..k + m];
            match s.kind {
                SymKind::Quiet => o.fill(Complex32::default()),
                SymKind::Leader => {
                    self.osc[0].fill(o);
                    self.buf.resize(m, Complex32::default());
                    self.osc[1].fill(&mut self.buf);
                    for (z, b) in o.iter_mut().zip(&self.buf) {
                        *z = (*z + *b) * 0.5;
                    }
                }
                SymKind::Tone(_) => self.osc[0].fill(o),
                SymKind::Carriers {
                    carriers,
                    digits,
                    scale,
                } => {
                    let hz = body::carrier_hz(u32::from(carriers));
                    let g = f64::from(scale) * body::CARRIER_PEAK / body::LEADER_PEAK;
                    for (j, z) in o.iter_mut().enumerate() {
                        let t = (self.n + j as u64) as f64 / self.rate;
                        let tau = (t - t_start) * body::FORMULA_RATE;
                        let (mut re, mut im) = (0.0, 0.0);
                        for (c, &f) in hz.iter().enumerate() {
                            let (a, b) = body::carrier_wave(f, (digits >> (4 * c)) as u8 & 15, tau);
                            re += a;
                            im += b;
                        }
                        *z = Complex32::new((re * g) as f32, (im * g) as f32);
                    }
                }
            }
            if s.kind != SymKind::Quiet {
                let len = end - start;
                for (j, z) in o.iter_mut().enumerate() {
                    let i = self.n + j as u64 - start;
                    let mut g = s.sign * self.level;
                    if s.rise && i < self.fade {
                        g *= ramp(i, self.fade);
                    }
                    if s.fall && len - 1 - i < self.fade {
                        g *= ramp(len - 1 - i, self.fade);
                    }
                    *z = if self.lower { (*z * g).conj() } else { *z * g };
                }
            }
            self.n += m as u64;
            k += m;
        }
        k
    }
}

/// The gain of sample `i` of a fade of `f` samples: 0.5 - 0.5 cos(pi (i + 1) / (f + 1)).
fn ramp(i: u64, f: u64) -> f32 {
    (0.5 - 0.5 * (std::f64::consts::PI * (i + 1) as f64 / (f + 1) as f64).cos()) as f32
}

/// The session id of a link: the remainder of eight one bits followed by the text of the caller's station id and then the
/// called station's (bit 7 of each character first), divided by x^8 + x^7 + x^6 + x^2 + x. A station id is the callsign in
/// upper case, spaces trimmed, with an SSID of 0 ("-0") left out. 0xFF, the id of no link, becomes 0x00.
pub fn session_id(caller: &str, called: &str) -> u8 {
    let mut bits = vec![1u8; 8];
    for id in [caller, called] {
        let id = id.trim().to_ascii_uppercase();
        let id = id.strip_suffix("-0").unwrap_or(&id);
        for b in id.bytes() {
            bits.extend((0..8).rev().map(|i| (b >> i) & 1));
        }
    }
    match remainder(&bits, 0, 0x1C6, 8) as u8 {
        0xFF => 0x00,
        id => id,
    }
}

/// Whether `call` is a callsign a frame can carry: 1 to 8 characters of letters, digits, `-` and `/`.
pub fn valid_call(call: &str) -> bool {
    let c = call.trim();
    (1..=8).contains(&c.len())
        && c.bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'/')
}

sdrkit_core::choice_enum! {
    /// The data frame type of an FEC broadcast.
    pub enum FecType {
        /// 16 data bytes a frame, 50 baud, 200 Hz.
        Fsk200_50s = ("4fsk_200_50s", "4FSK.200.50S"),
        /// 32 data bytes a frame, 100 baud, 500 Hz.
        Fsk500_100s = ("4fsk_500_100s", "4FSK.500.100S"),
        /// 64 data bytes a frame, 100 baud, 500 Hz.
        Fsk500_100 = ("4fsk_500_100", "4FSK.500.100"),
        /// 64 data bytes a frame, 4PSK, 1 carrier, 200 Hz.
        Psk4_200_100 = ("4psk_200_100", "4PSK.200.100"),
        /// 16 data bytes a frame, 4PSK, 1 carrier, 200 Hz.
        Psk4_200_100s = ("4psk_200_100s", "4PSK.200.100S"),
        /// 108 data bytes a frame, 8PSK, 1 carrier, 200 Hz.
        Psk8_200_100 = ("8psk_200_100", "8PSK.200.100"),
        /// 128 data bytes a frame, 16QAM, 1 carrier, 200 Hz.
        Qam16_200_100 = ("16qam_200_100", "16QAM.200.100"),
        /// 128 data bytes a frame, 4PSK, 2 carriers, 500 Hz.
        Psk4_500_100 = ("4psk_500_100", "4PSK.500.100"),
        /// 216 data bytes a frame, 8PSK, 2 carriers, 500 Hz.
        Psk8_500_100 = ("8psk_500_100", "8PSK.500.100"),
        /// 256 data bytes a frame, 16QAM, 2 carriers, 500 Hz.
        Qam16_500_100 = ("16qam_500_100", "16QAM.500.100"),
        /// 256 data bytes a frame, 4PSK, 4 carriers, 1000 Hz.
        Psk4_1000_100 = ("4psk_1000_100", "4PSK.1000.100"),
        /// 432 data bytes a frame, 8PSK, 4 carriers, 1000 Hz.
        Psk8_1000_100 = ("8psk_1000_100", "8PSK.1000.100"),
        /// 512 data bytes a frame, 16QAM, 4 carriers, 1000 Hz.
        Qam16_1000_100 = ("16qam_1000_100", "16QAM.1000.100"),
        /// 512 data bytes a frame, 4PSK, 8 carriers, 2000 Hz.
        Psk4_2000_100 = ("4psk_2000_100", "4PSK.2000.100"),
        /// 864 data bytes a frame, 8PSK, 8 carriers, 2000 Hz.
        Psk8_2000_100 = ("8psk_2000_100", "8PSK.2000.100"),
        /// 1024 data bytes a frame, 16QAM, 8 carriers, 2000 Hz.
        Qam16_2000_100 = ("16qam_2000_100", "16QAM.2000.100"),
    }
}

impl FecType {
    /// The even type byte.
    pub fn code(self) -> u8 {
        super::data_code(self.label()).expect("every FEC type is a data frame type")
    }
    /// The lowest and highest audio frequency of the tones or carriers and half the baud rate each side, Hz.
    pub fn band(self) -> (f64, f64) {
        match self {
            FecType::Fsk200_50s => (1400.0, 1600.0),
            FecType::Fsk500_100s | FecType::Fsk500_100 => (1300.0, 1700.0),
            _ => {
                let n = frame_type(self.code()).map_or(1, |t| t.carriers);
                let hz = body::carrier_hz(n);
                (hz[0] - 50.0, hz[hz.len() - 1] + 50.0)
            }
        }
    }
}

/// The leader of every frame sent.
pub const LEADER_MS: Setting<field::I64> = Setting::new("leader_ms", "Leader", "ms", 240)
    .range(100.0, 1000.0, 20.0)
    .help("The two-tone leader before each frame, 100 to 1000 ms in steps of 20. A receiver needs about 100 ms to find it.")
    .section(crate::sender::TX_SECTION);
/// The frame type of an FEC broadcast.
pub const FEC_TYPE: Setting<field::Choice<FecType>> =
    Setting::choice("fec_type", "FEC frame type", FecType::Fsk500_100)
        .help("The data frames of a text sent with no link: 4FSK.200.50S (16 bytes a frame, 200 Hz), 4FSK.500.100S (32 bytes), 4FSK.500.100 (64 bytes, 500 Hz), or 4PSK, 8PSK or 16QAM at 100 baud on 1, 2, 4 or 8 carriers (200, 500, 1000 or 2000 Hz, 16 to 1024 bytes a frame).")
        .section(crate::sender::TX_SECTION);
/// Whether a connect request to the station callsign is answered.
pub const AUTO_ANSWER: Setting<field::Bool> = Setting::new("auto_answer", "Answer calls", "", false)
    .help("Answers a connect request to the station callsign with ConAck500 and runs the link as the called station, with no key press. Off at the start, so that nothing is sent unattended: a request is then shown and not answered. The engine's transmit switch must be on.")
    .section(crate::sender::TX_SECTION);

sdrkit_core::settings! {
    /// The settings of [`ArdopSender`].
    pub enum ArdopTxSetting {
        /// USB or LSB, shared with the front end.
        Demod = crate::front::DEMOD,
        /// The leader.
        LeaderMs = LEADER_MS,
        /// The FEC frame type.
        FecType = FEC_TYPE,
        /// Answer calls.
        AutoAnswer = AUTO_ANSWER,
    }
}

/// The settings of the sender that the receiver does not have.
pub const OWN_SETTINGS: &[&'static dyn DeclareSetting] = &[&LEADER_MS, &FEC_TYPE, &AUTO_ANSWER];

sdrkit_core::commands! {
    /// The commands of [`ArdopSender`].
    pub enum ArdopTxCommand {
        /// Send a text.
        Send = ARDOP_SEND.command,
        /// Send the ID frame.
        SendId = ARDOP_SEND_ID,
        /// Ping.
        Ping = ARDOP_PING.command,
        /// Connect.
        Connect = ARDOP_CONNECT.command,
        /// Disconnect.
        Disconnect = ARDOP_DISCONNECT,
        /// Stop.
        Stop = ARDOP_STOP,
    }
}

/// Why a request of the sender is refused.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TxError {
    /// The engine's transmit setting is off.
    Off,
    /// The stream rate is below the family's minimum.
    Rate,
    /// The demodulator is NFM or AM.
    Sideband,
    /// The station callsign is not set.
    NoCall,
    /// The station callsign cannot be sent.
    BadCall,
    /// The called station is not a callsign a frame can carry.
    BadTarget,
    /// The text is empty.
    Empty,
    /// The text is longer than [`MAX_FEC`] bytes.
    Long,
    /// The link refuses it.
    Link(LinkError),
}

impl std::fmt::Display for TxError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TxError::Off => f.write_str("the transmitter is off"),
            TxError::Rate => f.write_str("the transmit rate must be at least 12 kHz"),
            TxError::Sideband => f.write_str("the signal is sent on USB or LSB only"),
            TxError::NoCall => f.write_str("the station callsign is not set"),
            TxError::BadCall => {
                f.write_str("the station callsign must be 1 to 8 letters, digits, - or /")
            }
            TxError::BadTarget => {
                f.write_str("the called station must be 1 to 8 letters, digits, - or /")
            }
            TxError::Empty => f.write_str("the text is empty"),
            TxError::Long => write!(f, "an FEC text is at most {MAX_FEC} bytes"),
            TxError::Link(e) => e.fmt(f),
        }
    }
}

impl std::error::Error for TxError {}

/// A burst waiting or on air.
struct Burst {
    w: FrameWriter,
    /// What it sends, for the events.
    what: String,
    /// The name of its first frame.
    frame: String,
    /// The stream time of its first sample, seconds.
    start_s: f64,
    /// Samples written.
    out: u64,
}

/// The time around an own burst in which a frame heard is the stack's own, seconds: the delay of the receiver's filters and
/// more. A frame of the other station starts at least 0.2 s after an own burst ends (its silence and its shortest leader).
const OWN_MARGIN_S: f64 = 0.05;

/// The sender (`hfkbd.ardoptx`). Input `in` is the session's samples (their rate, pace and dial), input `frames` the frames
/// of the receiver; output `out` is complex baseband for `tx.sink`, centred on the dial, output `events` the `hfkbd.tx` and
/// `ardop.link` events. Each burst is 0.1 s of silence and its frames, 0.15 s apart, and ends at the last sample of its last
/// frame with the end mark. A frame heard while the stack itself was sending (within 0.05 s of an own burst) is its own echo
/// and is not given to the link. Times are counted in input samples. The engine's transmit setting is the only switch: off, every
/// request is refused, the burst on air ends and the link closes.
pub struct ArdopSender {
    rate: f64,
    dial: f64,
    transmit: bool,
    live: bool,
    wall: Clock,
    host: Option<Arc<dyn HostStreams>>,
    call: String,
    locator: String,
    demod: crate::front::Demod,
    leader_ms: u32,
    fec: FecType,
    auto_answer: bool,
    link: Link,
    rs: RsCodes,
    pending: VecDeque<Burst>,
    burst: Option<Burst>,
    /// Own bursts: their start and end, stream seconds; the end is infinite while on air.
    own: VecDeque<(f64, f64)>,
    samples: u64,
    end_owed: bool,
    fec_odd: bool,
    request: Option<String>,
    rx_bytes: Vec<u8>,
    rx_text: String,
    outs: Vec<LinkOut>,
    events: Vec<sdrkit_core::Event>,
    results: Published,
}

impl Default for ArdopSender {
    fn default() -> Self {
        let mut s = ArdopSender {
            rate: 0.0,
            dial: 0.0,
            transmit: false,
            live: false,
            wall: Clock::fixed(0.0),
            host: None,
            call: String::new(),
            locator: String::new(),
            demod: crate::front::DEMOD.default,
            leader_ms: LEADER_MS.default as u32,
            fec: FEC_TYPE.default,
            auto_answer: AUTO_ANSWER.default,
            link: Link::new(),
            rs: RsCodes::new(),
            pending: VecDeque::new(),
            burst: None,
            own: VecDeque::new(),
            samples: 0,
            end_owed: false,
            fec_odd: false,
            request: None,
            rx_bytes: Vec::new(),
            rx_text: String::new(),
            outs: Vec::new(),
            events: Vec::new(),
            results: Published::new(ArdopTxFields::SHAPE.fields),
        };
        s.publish();
        s
    }
}

impl ArdopSender {
    /// The stream time now, seconds.
    fn now(&self) -> f64 {
        if self.rate > 0.0 {
            self.samples as f64 / self.rate
        } else {
            0.0
        }
    }

    fn tx_event(&mut self, state: &str, reason: &str, text: &str) {
        self.events.push(KEYBOARD_TX.event(
            0.0,
            self.dial + 1500.0,
            &KeyboardTxEvent {
                state,
                reason,
                text,
                family: SourceFamily::Message,
            },
        ));
    }

    fn link_event(&mut self, state: ArdopLinkState, source: &str, reason: &str, text: &str) {
        self.events.push(ARDOP_LINK.event(
            0.0,
            self.dial + 1500.0,
            &ArdopLinkEvent {
                state,
                source,
                reason,
                text,
                family: SourceFamily::Message,
            },
        ));
    }

    /// The time on air of a burst whose first sample is written in a pass of `pass` seconds: the clock, the lead of the host
    /// and two passes. `None` when the radio is not live or the host gives no lead.
    fn start_time(&self, pass: f64) -> Option<f64> {
        let lead = self.host.as_ref()?.transmit_lead().as_secs_f64();
        (self.live && lead > 0.0).then(|| self.wall.now() + lead + 2.0 * pass)
    }

    fn lower(&self) -> bool {
        self.demod == crate::front::Demod::Lsb
    }

    /// The checks of every request that sends: transmit on, the rate, USB or LSB, a station callsign a frame can carry.
    fn check(&self) -> Result<(), TxError> {
        if !self.transmit {
            return Err(TxError::Off);
        }
        if self.rate < crate::front::MIN_RATE {
            return Err(TxError::Rate);
        }
        crate::sender::lower_of(self.demod).map_err(|_| TxError::Sideband)?;
        if self.call.is_empty() {
            return Err(TxError::NoCall);
        }
        if !valid_call(&self.call) {
            return Err(TxError::BadCall);
        }
        Ok(())
    }

    /// Queues a burst of `frames`.
    fn queue(&mut self, frames: &[TxFrame], what: &str) {
        let mut w = FrameWriter::new(self.rate, TX_LEVEL, self.lower());
        w.push_quiet(LEAD_S);
        for (i, f) in frames.iter().enumerate() {
            if i > 0 {
                w.push_quiet(GAP_S);
            }
            w.push_frame(f);
        }
        let frame = frames
            .first()
            .and_then(|f| frame_type(f.code))
            .map(|t| t.name)
            .unwrap_or_default();
        self.pending.push_back(Burst {
            w,
            what: what.to_owned(),
            frame,
            start_s: 0.0,
            out: 0,
        });
    }

    /// The FEC frames of `data`: frames of the FEC type, even and odd in turn, session 0xFF.
    fn fec_frames(&mut self, data: &[u8]) -> Vec<TxFrame> {
        let code = self.fec.code();
        let size = frame_type(code).map_or(16, |t| t.capacity());
        data.chunks(size)
            .map(|c| {
                let f = TxFrame::data_bytes(&mut self.rs, code | u8::from(self.fec_odd), 0xFF, c)
                    .with_leader(self.leader_ms);
                self.fec_odd = !self.fec_odd;
                f
            })
            .collect()
    }

    fn send(&mut self, text: &str) -> Result<&'static str, TxError> {
        self.check()?;
        if text.is_empty() {
            return Err(TxError::Empty);
        }
        if self.link.active() {
            let now = self.now();
            let mut outs = std::mem::take(&mut self.outs);
            let r = self.link.queue(text.as_bytes(), now, &mut outs);
            self.outs = outs;
            r.map_err(TxError::Link)?;
            return Ok("queued");
        }
        if text.len() > MAX_FEC {
            return Err(TxError::Long);
        }
        let frames = self.fec_frames(text.as_bytes());
        self.queue(&frames, text);
        Ok("started")
    }

    fn send_id(&mut self) -> Result<(), TxError> {
        self.check()?;
        if self.link.active() {
            return Err(TxError::Link(LinkError::Busy));
        }
        let f = TxFrame::two_fields(
            &mut self.rs,
            0x30,
            &self.call.clone(),
            &self.locator.clone(),
        )
        .with_leader(self.leader_ms);
        self.queue(&[f], "IDFrame");
        Ok(())
    }

    fn ping(&mut self, target: &str) -> Result<(), TxError> {
        self.check()?;
        if !valid_call(target) {
            return Err(TxError::BadTarget);
        }
        if self.link.active() {
            return Err(TxError::Link(LinkError::Busy));
        }
        let f = TxFrame::two_fields(&mut self.rs, 0x3E, &self.call.clone(), target.trim())
            .with_leader(self.leader_ms);
        self.queue(&[f], "Ping");
        Ok(())
    }

    fn connect(&mut self, target: &str) -> Result<(), TxError> {
        self.check()?;
        if !valid_call(target) {
            return Err(TxError::BadTarget);
        }
        self.sync_link();
        let now = self.now();
        let mut outs = std::mem::take(&mut self.outs);
        let r = self.link.connect(target, now, &mut outs);
        self.outs = outs;
        r.map_err(TxError::Link)
    }

    /// Ends the burst on air at once; its end mark goes out on the next pass when samples of it were written.
    fn cut(&mut self, reason: &str) {
        self.pending.clear();
        if let Some(b) = self.burst.take() {
            self.end_owed |= b.out > 0;
            self.close_own();
            self.tx_event("finished", reason, &b.what);
        }
    }

    /// The end of the own burst on air is now.
    fn close_own(&mut self) {
        let now = self.now();
        if let Some(o) = self.own.back_mut() {
            if o.1.is_infinite() {
                o.1 = now;
            }
        }
    }

    /// Whether a frame that started at `t` (stream seconds) was sent by the stack itself.
    fn is_own(&self, t: f64) -> bool {
        self.own
            .iter()
            .any(|&(a, b)| t >= a - OWN_MARGIN_S && t <= b + OWN_MARGIN_S)
    }

    fn sync_link(&mut self) {
        let answer = self.auto_answer && self.check().is_ok();
        self.link.set(&self.call, self.leader_ms, answer);
    }

    /// Takes what the link asked for.
    fn take_outs(&mut self) {
        let outs = std::mem::take(&mut self.outs);
        for o in &outs {
            match o {
                LinkOut::Send(f) => {
                    let name = frame_type(f.code).map(|t| t.name).unwrap_or_default();
                    self.queue(std::slice::from_ref(f), &name);
                }
                LinkOut::Deliver(d) => {
                    self.rx_bytes.extend_from_slice(d);
                    if self.rx_bytes.len() > 4000 {
                        let cut = self.rx_bytes.len() - 4000;
                        self.rx_bytes.drain(..cut);
                    }
                    self.rx_text = String::from_utf8_lossy(&self.rx_bytes).into_owned();
                    let text = String::from_utf8_lossy(d).into_owned();
                    let peer = self.link.peer().unwrap_or_default().to_owned();
                    self.link_event(self.link.state(), &peer, "data received", &text);
                }
                LinkOut::State(s, reason) => {
                    let peer = self.link.peer().unwrap_or_default().to_owned();
                    self.link_event(*s, &peer, reason, "");
                }
                LinkOut::Request(from) => {
                    self.request = Some(from.clone());
                    let reason = if !self.auto_answer {
                        "a connect request, not answered: answering is off"
                    } else {
                        "a connect request, not answered: transmit is off"
                    };
                    self.link_event(self.link.state(), from, reason, "");
                }
            }
        }
        self.outs = outs;
        self.outs.clear();
    }

    /// Writes the result fields.
    fn publish(&mut self) {
        let l = &self.link;
        self.results.put(&ArdopTxFields {
            tx_sending: self.burst.is_some(),
            tx_frame: self.burst.as_ref().map(|b| b.frame.as_str()),
            tx_waiting: self.pending.len() as i64,
            link_state: l.state(),
            link_peer: l.peer(),
            link_session: l.session().map(i64::from),
            link_sent: l.sent() as i64,
            link_received: l.received() as i64,
            link_waiting: l.waiting() as i64,
            link_retries: i64::from(l.retries()),
            link_reason: l.reason(),
            link_text: (!self.rx_text.is_empty()).then_some(self.rx_text.as_str()),
            link_request: self.request.as_deref(),
        });
    }

    /// Writes the end mark on the last sample of output 0, or on one silent sample when the pass wrote none.
    fn end_mark(io: &mut Io) {
        let out = io.iq_out(0);
        if out.is_empty() {
            out.push(Complex32::default());
        }
        let last = out.len() - 1;
        io.output(0).tag_here(last, TagKind::BurstEnd);
    }

    fn refuse<T>(&mut self, r: Result<T, TxError>, what: &str) -> Result<T, CommandError> {
        r.map_err(|e| {
            self.tx_event("refused", &e.to_string(), what);
            CommandError::Failed {
                source: StageError::new(e),
            }
        })
    }
}

impl Component for ArdopSender {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "hfkbd.ardoptx",
            vec![Port::iq("in"), Port::typed::<ArdopFrame>("frames")],
            vec![
                Port::iq("out"),
                Port::new("events", Kind::Messages).rate_from(RateFrom::Fixed(0.0)),
            ],
        )
        .with_settings(ArdopTxSetting::schema())
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        let f = &inputs[0];
        if self.rate > 0.0 && f.rate != self.rate {
            // Keep the stream time across a change of rate.
            self.samples = (self.now() * f.rate).round() as u64;
        }
        self.rate = f.rate;
        self.dial = if f.tuned_hz > 0.0 {
            f.tuned_hz
        } else {
            f.centre_hz
        };
        if self.burst.is_some() || !self.pending.is_empty() {
            self.cut("the format changed");
        }
        Ok(vec![
            Format::iq(self.rate, self.dial, self.dial),
            Format::new(Kind::Messages, 0.0),
        ])
    }
    fn attach(&mut self, ctx: &RunContext) {
        self.transmit = ctx.transmit;
        self.live = ctx.live;
        self.wall = ctx.clock.clone();
        self.host.clone_from(&ctx.streams);
        ctx.station
            .callsign
            .trim()
            .to_ascii_uppercase()
            .clone_into(&mut self.call);
        self.locator = ctx.station.locator().unwrap_or_default();
        if !ctx.transmit {
            if self.burst.is_some() || !self.pending.is_empty() {
                self.cut("the transmitter is off");
            }
            let mut outs = std::mem::take(&mut self.outs);
            self.link.abort("the transmitter is off", &mut outs);
            self.outs = outs;
            self.take_outs();
        }
        self.sync_link();
        self.publish();
    }
    fn process(&mut self, io: &mut Io) {
        if std::mem::take(&mut self.end_owed) {
            Self::end_mark(io);
        }
        let room = io.input_len(0);
        let now = self.now();
        self.sync_link();
        let mut outs = std::mem::take(&mut self.outs);
        for f in io.items::<ArdopFrame>(1) {
            let f = &f.value;
            if self.is_own(f.time) {
                continue;
            }
            self.link.heard(&Heard::from(f), now, &mut outs);
        }
        self.link.tick(now, &mut outs);
        self.outs = outs;
        self.take_outs();
        if !self.transmit {
            self.pending.clear();
        }
        if self.burst.is_none() {
            if let Some(mut b) = self.pending.pop_front() {
                b.start_s = now;
                self.own.push_back((now, f64::INFINITY));
                if self.own.len() > 8 {
                    self.own.pop_front();
                }
                self.tx_event("started", "", &b.what);
                self.burst = Some(b);
            }
        }
        let stated = self
            .burst
            .as_ref()
            .filter(|b| b.out == 0)
            .and_then(|_| self.start_time(room as f64 / self.rate.max(1.0)));
        if let Some(b) = &mut self.burst {
            let out = io.iq_out(0);
            let first = out.len();
            out.resize(first + room, Complex32::default());
            let n = b.w.fill(&mut out[first..]);
            out.truncate(first + n);
            if b.out == 0 && n > 0 {
                if let Some(unix_s) = stated {
                    io.output(0).tag_here(first, TagKind::BurstStart { unix_s });
                }
            }
            b.out += n as u64;
            if b.w.finished() {
                Self::end_mark(io);
                let end = b.start_s + b.out as f64 / self.rate;
                if let Some(o) = self.own.back_mut() {
                    o.1 = end;
                }
                if let Some(b) = self.burst.take() {
                    self.tx_event("finished", "", &b.what);
                }
            }
        }
        self.samples += room as u64;
        self.publish();
        io.messages_out(1).append(&mut self.events);
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        match ArdopTxSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            ArdopTxSetting::Demod => self.demod = crate::front::DEMOD.read(id, v)?,
            ArdopTxSetting::LeaderMs => {
                let ms = LEADER_MS.read(id, v)?;
                self.leader_ms = (ms.clamp(100, 1000) / 20 * 20) as u32;
            }
            ArdopTxSetting::FecType => self.fec = FEC_TYPE.read(id, v)?,
            ArdopTxSetting::AutoAnswer => self.auto_answer = AUTO_ANSWER.read(id, v)?,
        }
        self.sync_link();
        Ok(Applied::Live)
    }
    fn values(&self) -> Values {
        values_of([
            (ArdopTxSetting::Demod, Value::Choice(self.demod.index())),
            (
                ArdopTxSetting::LeaderMs,
                Value::I64(i64::from(self.leader_ms)),
            ),
            (ArdopTxSetting::FecType, Value::Choice(self.fec.index())),
            (ArdopTxSetting::AutoAnswer, Value::Bool(self.auto_answer)),
        ])
    }
    fn results(&self) -> Option<&Results> {
        Some(self.results.results())
    }
    fn commands(&self) -> &Commands {
        ArdopTxCommand::commands()
    }
    fn invoke(&mut self, id: CommandId, args: &Values) -> Result<Option<Record>, CommandError> {
        match ArdopTxCommand::from_id(id).ok_or(CommandError::NoSuchId { id })? {
            ArdopTxCommand::Send => {
                let a: KeyboardSendArgs = ARDOP_SEND.args(args)?;
                let text = a.text.to_owned();
                let r = self.send(&text);
                let state = self.refuse(r, &text)?;
                if state == "queued" {
                    self.tx_event("queued", "", &text);
                }
            }
            ArdopTxCommand::SendId => {
                let r = self.send_id();
                self.refuse(r, "IDFrame")?;
            }
            ArdopTxCommand::Ping => {
                let a: ArdopCallArgs = ARDOP_PING.args(args)?;
                let target = a.target.to_owned();
                let r = self.ping(&target);
                self.refuse(r, "Ping")?;
            }
            ArdopTxCommand::Connect => {
                let a: ArdopCallArgs = ARDOP_CONNECT.args(args)?;
                let target = a.target.to_owned();
                let r = self.connect(&target);
                self.refuse(r, "ConReq500M")?;
            }
            ArdopTxCommand::Disconnect => {
                let now = self.now();
                let mut outs = std::mem::take(&mut self.outs);
                self.link.disconnect(now, &mut outs);
                self.outs = outs;
            }
            ArdopTxCommand::Stop => {
                self.cut("stopped");
                let mut outs = std::mem::take(&mut self.outs);
                self.link.abort("stopped", &mut outs);
                self.outs = outs;
            }
        }
        self.take_outs();
        self.publish();
        Ok(None)
    }
}

/// The receiver and sender of the stack: the family front end, the receiver on its 8 kHz sideband, its frames into the
/// sender, and the sender on the session's samples into the transmit sink. `listen` goes to the front end, `demod` to the front
/// end and the sender, the sender's own settings to the sender; `remove` goes to the receiver, `clear` to every component, and
/// `send`, `send_id`, `ping`, `connect`, `disconnect` and `stop` to the sender.
///
/// # Errors
///
/// The errors of the graph and of a refused setting.
pub fn graph(cfg: &StackConfig) -> Result<StackGraph, GraphError> {
    let dec = ArdopDecoder::new();
    let own: Vec<&'static str> = sdrkit_graph::Decoder::commands(&dec)
        .iter()
        .map(|c| c.key)
        .collect();
    let b = GraphBuilder::new()
        .catalog(&Catalog::of(cfg)?)
        .add("front", crate::front::HfFront::new())
        .add("dec", DecoderComponent::new(dec))
        .add("sender", ArdopSender::default())
        .add("tx", TxSink::default())
        .input("front")
        .input("sender.in")
        .connect("front.caudio", "dec.in")
        .connect("dec.frames", "sender.frames")
        .connect("sender.out", "tx.in")
        .output("dec.events")
        .output("sender.events")
        .output("front.listen")
        .status_from("dec")
        .clock(0.1);
    let mut g = StackGraph::of(crate::typed::shape("ardop"), b.build()?)
        .min_rate(
            crate::front::MIN_RATE,
            "HF keyboard stacks need a sample rate of at least 12 ksps",
        )
        .route("listen", &["front.listen"])?
        .route("demod", &["front.demod", "sender.demod"])?;
    for s in OWN_SETTINGS {
        let k = s.key();
        g = g.route(k, &[&format!("sender.{k}")])?;
    }
    for key in own {
        g = g.command(key, CommandTarget::of("dec", key));
    }
    g = g.command("clear", CommandTarget::every("clear"));
    for key in ["send", "send_id", "ping", "connect", "disconnect", "stop"] {
        g = g.command(key, CommandTarget::of("sender", key));
    }
    g.with_settings(&cfg.settings).build()
}
