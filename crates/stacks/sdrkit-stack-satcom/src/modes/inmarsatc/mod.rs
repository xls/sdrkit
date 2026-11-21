//! Inmarsat-C (`inmarsatc`): the NCS common channel and LES TDM channels, with up to 3 more channels found in the capture.
//! Standard: Inmarsat-C System Definition Manual; IMO SafetyNET Manual.

pub mod coding;
pub mod egc;
pub mod gen;
pub mod rx;

use crate::display::Clock;
use crate::typed::{
    InmarsatcChannelRow, InmarsatcEgcEvent, InmarsatcFields, InmarsatcMessageRow,
    InmarsatcPacketCountRow, INMARSATC_CHANNELS, INMARSATC_EGC, INMARSATC_MESSAGES,
    INMARSATC_PACKET_COUNTS,
};
use egc::{Bulletin, EgcAssembler, EgcMessage, EgcPacket};
use rx::{CChain, CFrame, ChainStatus};
use sdrkit_algo_dsp::fft::Fft;
use sdrkit_algo_dsp::window::Window;
use sdrkit_core::interface::{Fields as _, Hertz, Seconds, Time};
use sdrkit_core::publish::keys::{Family, Urgency};
use sdrkit_core::publish::{field, Published};
use sdrkit_core::verify::{CheckLine, VerifyCase, VerifyHooks, VerifyOptions};
use sdrkit_core::SignalType;
use sdrkit_core::{
    AutoGain, Choice, Complex32, ConfigError, GenConfig, Generator, HostRequest, LockProfile,
    LockState, Receiver, ResultsDecl, Schema, Setting, SettingError, SettingId, SettingSet,
    StackConfig, StackDescriptor, StateBlob, StateError, Tuning, Value, Values,
};
use sdrkit_graph::{
    values_of, Catalog, Decoder, DecoderComponent, Format, GraphBuilder, GraphError, Item, Kind,
    Outbox, Port, RateFrom, Registry, RunContext, StackGraph, Timed,
};
use sdrkit_util_objects::commands::Tables;
use sdrkit_util_objects::publish::columns;
use sdrkit_util_objects::shaped::{self, RowOut, Shaped};
use sdrkit_util_objects::Seen;
use std::fmt::Write;
use std::sync::{Arc, OnceLock};

/// The stack id.
pub const ID: &str = "inmarsatc";
/// The stack name.
pub const NAME: &str = "Inmarsat-C";
/// The lowest sample rate, Hz.
pub const MIN_RATE: f64 = 250_000.0;
/// Seconds between reports.
pub const REPORT_S: f64 = 0.25;
/// The most EGC messages kept.
pub const MESSAGE_CAP: usize = 150;
/// The most channels decoded at once: the main one and 3 found in the capture.
pub const MAX_CHANNELS: usize = 4;

/// What the demodulator passes on.
#[derive(Clone, Debug)]
pub enum InmcItem {
    Frame {
        channel: usize,
        frame: Box<CFrame>,
    },
    Status {
        channels: Vec<ChannelInfo>,
    },
    /// The signal state was dropped (a retune).
    Reset,
}
impl Item for InmcItem {
    const KIND: &'static str = "inmarsatc.frame";
}

/// A channel's state as the demodulator reports it.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ChannelInfo {
    pub index: usize,
    /// Offset from where the user tuned, Hz.
    pub offset_hz: f64,
    pub status: ChainStatus,
}

struct Extra {
    chain: CChain,
    born_s: f64,
    last_frame_s: f64,
}

/// The averaged spectrum of the whole capture, for more channels and for a wide carrier error.
struct BandSearch {
    n: usize,
    fft: Fft,
    win: Vec<f32>,
    buf: Vec<Complex32>,
    spec: Vec<f32>,
    acc: Vec<f32>,
    blocks: usize,
    block: Vec<Complex32>,
    every: usize,
    skip: usize,
    rate: f64,
    /// Candidates of the previous evaluation (offsets from the input centre).
    prev: Vec<f64>,
}

impl BandSearch {
    fn new(rate: f64) -> Self {
        let n = 16_384;
        BandSearch {
            n,
            fft: Fft::forward(n),
            win: Window::Hann.coefficients(n),
            buf: Vec::with_capacity(n),
            spec: vec![0.0; n],
            acc: vec![0.0; n],
            blocks: 0,
            block: Vec::with_capacity(n),
            // About 20 blocks a second.
            every: ((rate / 20.0) as usize).max(n),
            skip: 0,
            rate,
            prev: Vec::new(),
        }
    }
    /// Feeds samples; returns the steady narrow lumps (offsets from the centre, Hz) after each second.
    fn push(&mut self, x: &[Complex32], span: f64) -> Option<Vec<f64>> {
        let mut res = None;
        for &s in x {
            if self.skip > 0 {
                self.skip -= 1;
                continue;
            }
            self.block.push(s);
            if self.block.len() == self.n {
                self.buf.clear();
                self.buf
                    .extend(self.block.iter().zip(&self.win).map(|(x, w)| x * *w));
                self.fft.process(&mut self.buf);
                for (o, v) in self.spec.iter_mut().zip(&self.buf) {
                    *o = v.norm_sqr();
                }
                for (a, v) in self.acc.iter_mut().zip(self.spec.iter()) {
                    *a += v;
                }
                self.block.clear();
                self.blocks += 1;
                self.skip = self.every - self.n;
                if self.blocks >= 20 {
                    res = Some(self.evaluate(span));
                    self.acc.iter_mut().for_each(|v| *v = 0.0);
                    self.blocks = 0;
                }
            }
        }
        res
    }
    fn evaluate(&mut self, span: f64) -> Vec<f64> {
        let n = self.n;
        let bin = self.rate / n as f64;
        // Smooth over a channel's width (2.4 kHz) and compare with the floor around it (+-8 kHz, circular).
        let w = ((2400.0 / bin) as usize).max(1);
        let f = ((8000.0 / bin) as usize).max(3 * w);
        // The spectrum is in FFT order: bin k is k * bin Hz, the negative frequencies at the top.
        let at = |k: i64| self.acc[k.rem_euclid(n as i64) as usize] as f64;
        let mut cs = vec![0f64; n + 1];
        for i in 0..n {
            cs[i + 1] = cs[i] + at(i as i64);
        }
        let win_sum = |a: i64, b: i64| -> f64 {
            // Sum of bins a..b (b exclusive), circular.
            let mut s = 0.0;
            let mut k = a;
            while k < b {
                let ka = k.rem_euclid(n as i64) as usize;
                let len = ((b - k) as usize).min(n - ka);
                s += cs[ka + len] - cs[ka];
                k += len as i64;
            }
            s
        };
        let mut lumps = Vec::new();
        let lim = if span > 0.0 { span } else { self.rate * 0.45 };
        let kmax = ((lim / bin) as i64).min((0.45 * n as f64) as i64);
        let mut k = -kmax;
        let mut cands: Vec<(f64, f64)> = Vec::new();
        while k <= kmax {
            let c = win_sum(k - w as i64 / 2, k + w as i64 / 2 + 1) / (w + 1) as f64;
            let fl = (win_sum(k - f as i64, k - w as i64)
                + win_sum(k + w as i64 + 1, k + f as i64 + 1))
                / (2 * (f - w)) as f64;
            let off = k as f64 * bin;
            if off.abs() > 2000.0 && c > fl * 10f64.powf(0.2) {
                cands.push((off, c / fl));
            }
            k += 1;
        }
        // Local maxima, at least 3 kHz apart.
        cands.sort_by(|a, b| b.1.total_cmp(&a.1));
        for (o, _) in cands {
            if lumps.iter().all(|&l: &f64| (l - o).abs() > 3000.0) {
                lumps.push(o);
            }
            if lumps.len() >= 12 {
                break;
            }
        }
        // Steady: seen in two evaluations in a row.
        let steady: Vec<f64> = lumps
            .iter()
            .cloned()
            .filter(|o| self.prev.iter().any(|p| (p - o).abs() < 500.0))
            .collect();
        self.prev = lumps;
        steady
    }
}

/// The demodulator: the main channel, the band search and the extra channels.
pub struct InmcDemod {
    rate: f64,
    dial: f64,
    main: Option<CChain>,
    more: Vec<Extra>,
    band: Option<BandSearch>,
    channel_search: bool,
    span_hz: f64,
    frames: Vec<CFrame>,
    no_carrier_s: f64,
    time_s: f64,
    reset_pending: bool,
}

impl Default for InmcDemod {
    fn default() -> Self {
        InmcDemod {
            rate: 0.0,
            dial: 0.0,
            main: None,
            more: Vec::new(),
            band: None,
            channel_search: true,
            span_hz: 0.0,
            frames: Vec::new(),
            no_carrier_s: 0.0,
            time_s: 0.0,
            reset_pending: false,
        }
    }
}

impl InmcDemod {
    /// A demodulator of the main channel only (no search for more channels).
    pub fn without_search() -> Self {
        InmcDemod {
            channel_search: false,
            ..Default::default()
        }
    }

    /// Starts on the input `f` (the main channel at its tuned frequency).
    pub fn start(&mut self, f: &Format) -> Result<(), ConfigError> {
        if f.rate.is_nan() || f.rate < MIN_RATE {
            return Err(ConfigError::RateTooLow {
                need: MIN_RATE,
                got: f.rate,
            });
        }
        self.rate = f.rate;
        self.dial = f.signal_offset_hz();
        self.main = Some(CChain::new(f.rate, self.dial));
        self.more.clear();
        self.band = Some(BandSearch::new(f.rate));
        self.no_carrier_s = 0.0;
        self.time_s = 0.0;
        Ok(())
    }
}

/// Look for up to 3 more channels in the capture.
pub const CHANNEL_SEARCH: Setting<field::Bool> =
    Setting::new("channel_search", "Channel search", "", true)
        .help("Look for up to 3 more channels in the capture.");
/// The limit of the channel search around 0 Hz (0: the whole input).
pub const SEARCH_SPAN_HZ: Setting<field::Hz> =
    Setting::new("search_span_hz", "Search span", "Hz", 0.0)
        .range(0.0, 1e8, 0.0)
        .help("Limit of the channel search around 0 Hz (0: the whole input).");

sdrkit_core::settings! {
    /// The settings of [`InmcDemod`].
    pub enum InmcDemodSetting {
        /// Channel search.
        ChannelSearch = CHANNEL_SEARCH,
        /// Search span.
        SearchSpanHz = SEARCH_SPAN_HZ,
    }
}

impl Decoder for InmcDemod {
    type Input = Complex32;
    fn name(&self) -> &'static str {
        "satcom.inmarsatc_demod"
    }
    fn outputs(&self) -> Vec<Port> {
        vec![Port::typed::<InmcItem>("out").same_format_as(0)]
    }
    fn settings(&self) -> Arc<Schema> {
        InmcDemodSetting::schema()
    }
    fn values(&self) -> Values {
        values_of([
            (
                InmcDemodSetting::ChannelSearch,
                Value::Bool(self.channel_search),
            ),
            (InmcDemodSetting::SearchSpanHz, Value::Hz(self.span_hz)),
        ])
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<(), SettingError> {
        match InmcDemodSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            InmcDemodSetting::ChannelSearch => {
                self.channel_search = CHANNEL_SEARCH.read(id, v)?;
                if !self.channel_search {
                    self.more.clear();
                }
            }
            InmcDemodSetting::SearchSpanHz => self.span_hz = SEARCH_SPAN_HZ.read(id, v)?,
        }
        Ok(())
    }
    fn configure(&mut self, f: &Format) -> Result<Vec<Format>, ConfigError> {
        self.start(f)?;
        Ok(vec![f.with_kind(Kind::typed::<InmcItem>())])
    }
    fn retune(&mut self, f: &Format) {
        if self.start(f).is_ok() {
            self.reset_pending = true;
        }
    }
    fn process(&mut self, iq: &[Complex32], _first: u64, out: &mut Outbox) {
        self.feed(iq, out.items::<InmcItem>(0));
    }
    fn tick(&mut self, out: &mut Outbox) {
        self.status(out.items::<InmcItem>(0));
    }
}

impl InmcDemod {
    /// Demodulates `iq`, appending frames (and a reset after a retune) to `out`.
    pub fn feed(&mut self, iq: &[Complex32], out: &mut Vec<Timed<InmcItem>>) {
        if self.main.is_none() {
            return;
        }
        if self.reset_pending {
            self.reset_pending = false;
            out.push(Timed {
                time: self.time_s,
                value: InmcItem::Reset,
            });
        }
        // Bounded pieces keep the working buffers small.
        for piece in iq.chunks(32_768) {
            self.time_s += piece.len() as f64 / self.rate;
            if let Some(m) = &mut self.main {
                self.frames.clear();
                m.process(piece, &mut self.frames);
                for f in self.frames.drain(..) {
                    out.push(Timed {
                        time: f.time,
                        value: InmcItem::Frame {
                            channel: 0,
                            frame: Box::new(f),
                        },
                    });
                }
                if m.carrier.is_none() {
                    self.no_carrier_s += piece.len() as f64 / self.rate;
                } else {
                    self.no_carrier_s = 0.0;
                }
            }
            for (i, e) in self.more.iter_mut().enumerate() {
                self.frames.clear();
                e.chain.process(piece, &mut self.frames);
                for f in self.frames.drain(..) {
                    e.last_frame_s = self.time_s;
                    out.push(Timed {
                        time: f.time,
                        value: InmcItem::Frame {
                            channel: i + 1,
                            frame: Box::new(f),
                        },
                    });
                }
            }
            let lumps = match &mut self.band {
                Some(b) => b.push(piece, self.span_hz),
                None => None,
            };
            if let Some(l) = lumps {
                self.manage(&l);
            }
        }
    }
    /// Appends the channels' state to `out`.
    pub fn status(&mut self, out: &mut Vec<Timed<InmcItem>>) {
        let mut ch = Vec::with_capacity(MAX_CHANNELS);
        if let Some(m) = &self.main {
            ch.push(ChannelInfo {
                index: 0,
                offset_hz: m.offset - self.dial,
                status: m.status,
            });
        }
        for (i, e) in self.more.iter().enumerate() {
            ch.push(ChannelInfo {
                index: i + 1,
                offset_hz: e.chain.offset - self.dial,
                status: e.chain.status,
            });
        }
        out.push(Timed {
            time: self.time_s,
            value: InmcItem::Status { channels: ch },
        });
    }

    fn manage(&mut self, lumps: &[f64]) {
        let t = self.time_s;
        // A wide carrier error: move the main channel to the nearest steady narrow signal.
        if self.no_carrier_s > 4.0 {
            if let Some(m) = &mut self.main {
                let near = lumps
                    .iter()
                    .cloned()
                    .filter(|o| (o - self.dial).abs() < 120_000.0)
                    .min_by(|a, b| (a - self.dial).abs().total_cmp(&(b - self.dial).abs()));
                if let Some(o) = near {
                    if (o - m.offset).abs() > 1500.0 {
                        m.retune(o);
                        self.no_carrier_s = 0.0;
                    }
                }
            }
        }
        if !self.channel_search {
            return;
        }
        // Drop channels that never produced a frame, went quiet, or double another.
        let main_off = self.main.as_ref().map_or(self.dial, |m| m.offset);
        let mut keep = Vec::new();
        for e in self.more.drain(..) {
            let quiet = if e.last_frame_s > 0.0 {
                t - e.last_frame_s > 30.0
            } else {
                t - e.born_s > 30.0
            };
            let double = (e.chain.offset - main_off).abs() < 1500.0
                || keep
                    .iter()
                    .any(|k: &Extra| (k.chain.offset - e.chain.offset).abs() < 1500.0);
            if !quiet && !double {
                keep.push(e);
            }
        }
        self.more = keep;
        for &o in lumps {
            if self.more.len() >= 3 {
                break;
            }
            let taken = (o - main_off).abs() < 6000.0
                || self
                    .more
                    .iter()
                    .any(|e| (e.chain.offset - o).abs() < 6000.0);
            if !taken {
                self.more.push(Extra {
                    chain: CChain::new(self.rate, o),
                    born_s: t,
                    last_frame_s: 0.0,
                });
            }
        }
    }
}

// ------------------------------------------------------------------------------------------------------------------------------
// Results
// ------------------------------------------------------------------------------------------------------------------------------

sdrkit_core::choice_enum! {
    /// The state of a channel.
    pub enum ChannelState {
        /// Searching for the carrier.
        Searching = ("searching", "Searching"),
        /// The carrier is locked.
        Carrier = ("carrier", "Carrier"),
        /// Frames decode.
        Frames = ("frames", "Frames"),
    }
}

/// The urgency of an EGC priority: distress is an emergency, urgency is urgent, safety is a notice.
pub fn urgency_of(priority: u8) -> Urgency {
    match priority {
        3 => Urgency::Emergency,
        2 => Urgency::Urgent,
        1 => Urgency::Notice,
        _ => Urgency::None,
    }
}

/// The event of a complete EGC message `m`, heard at `t` on `freq_hz`.
pub fn egc_event(m: &EgcMessage, t: f64, freq_hz: f64) -> sdrkit_core::Event {
    let mut src = String::new();
    let _ = write!(src, "{} {}", m.message_number, m.service_text);
    INMARSATC_EGC.event(
        t,
        freq_hz,
        &InmarsatcEgcEvent {
            source: &src,
            text: &m.text,
            family: Family::Message,
            urgency: urgency_of(m.priority),
            message_number: i64::from(m.message_number),
            service_code: i64::from(m.service_code),
            priority_code: i64::from(m.priority),
            subject: &m.subject,
            area: &m.area,
        },
    )
}

/// What the display publishes.
fn display_decl() -> Arc<ResultsDecl> {
    static DECL: OnceLock<Arc<ResultsDecl>> = OnceLock::new();
    DECL.get_or_init(|| {
        Arc::new(
            ResultsDecl::new()
                .with_fields(InmarsatcFields::SHAPE.fields)
                .with_table(&INMARSATC_MESSAGES)
                .with_table(&INMARSATC_CHANNELS)
                .with_table(&INMARSATC_PACKET_COUNTS),
        )
    })
    .clone()
}

/// What the stack publishes.
pub fn declaration() -> ResultsDecl {
    crate::typed::shape(ID).results_decl()
}

impl Shaped for EgcMessage {
    type Row = InmarsatcMessageRow<'static>;
    fn row(&self, seen: Option<&Seen>, _now_s: f64, out: RowOut<'_, '_, Self::Row>) {
        out.put(&InmarsatcMessageRow {
            first_heard: columns::first_heard(seen).map(Time),
            last_heard: columns::last_heard(seen).map(Time),
            channel: self.channel as i64,
            message_number: i64::from(self.message_number),
            service_code: i64::from(self.service_code),
            service: &self.service_text,
            kind: &self.kind,
            priority: &self.priority_name,
            presentation: &self.presentation,
            packets: self.packets as i64,
            complete: self.complete,
            times_heard: self.times_heard_complete as i64,
            area: &self.area,
            subject: &self.subject,
            warning_number: &self.warning_number,
            text: &self.text,
            truncated: self.truncated,
        });
    }
}

/// Per-channel totals the display keeps.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ChannelRow {
    pub index: usize,
    pub offset_hz: f64,
    pub carrier_found: bool,
    pub state: u8,
    pub ebn0_db: f64,
    pub uw_errors_avg: f64,
    pub frame_number: u32,
    pub frames_good: u64,
    pub frames_bad: u64,
    pub messages: u64,
    pub region: String,
    pub station: String,
    pub channel_type: String,
    uw_sum: f64,
    uw_n: f64,
}

/// The NCS information of the main channel's bulletin board.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct NcsInfo {
    pub bulletin: Bulletin,
    pub region: String,
    pub station_name: String,
    pub signalling_uplink_hz: Option<f64>,
}

sdrkit_core::choice_enum! {
    /// The satellite whose NCS common channel the host tunes to.
    pub enum Satellite {
        /// None: tune by hand.
        None = ("none", "None (tune by hand)"),
        /// Alphasat 25E.
        Alphasat = ("alphasat", "Alphasat 25E (Europe, Africa, Indian Ocean) 1537.10 MHz IOR"),
        /// Inmarsat-4 F3 98W.
        I4f3 = ("i4f3", "Inmarsat-4 F3 98W (Americas) 1537.70 MHz AOR-W"),
        /// Inmarsat-3 F5 54W.
        I3f5 = ("i3f5", "Inmarsat-3 F5 54W (Atlantic East) 1541.45 MHz AOR-E"),
        /// Inmarsat-4 F1 143.5E.
        I4f1 = ("i4f1", "Inmarsat-4 F1 143.5E (Pacific, Asia) 1541.45 MHz POR (community value)"),
    }
}

/// The NCS common channel of a satellite (dial frequency, Hz).
pub fn satellite_hz(s: Satellite) -> Option<f64> {
    match s {
        Satellite::Alphasat => Some(1537.10e6),
        Satellite::I4f3 => Some(1537.70e6),
        Satellite::I3f5 | Satellite::I4f1 => Some(1541.45e6),
        Satellite::None => None,
    }
}

sdrkit_core::choice_enum! {
    /// The lowest priority of the messages shown.
    pub enum MinPriority {
        /// All.
        All = ("0", "All"),
        /// Safety and higher.
        Safety = ("1", "Safety and higher"),
        /// Urgency and higher.
        Urgency = ("2", "Urgency and higher"),
        /// Distress.
        Distress = ("3", "Distress"),
    }
}

sdrkit_core::choice_enum! {
    /// The service of the messages shown.
    pub enum ServiceFilter {
        /// All.
        All = ("all", "All"),
        /// SafetyNET.
        SafetyNet = ("safetynet", "SafetyNET"),
        /// FleetNET.
        FleetNet = ("fleetnet", "FleetNET"),
    }
}

/// The satellite whose NCS common channel the host tunes to.
pub const SATELLITE: Setting<field::Choice<Satellite>> =
    Setting::choice("satellite", "Satellite", Satellite::None)
        .help("Asks the host to tune the dial to the NCS common channel of this satellite.");
/// The lowest priority of the messages shown.
pub const MIN_PRIORITY: Setting<field::Choice<MinPriority>> =
    Setting::choice("min_priority", "Priority", MinPriority::All)
        .help("Shows the messages of this priority and higher.");
/// The service of the messages shown.
pub const SERVICE_FILTER: Setting<field::Choice<ServiceFilter>> =
    Setting::choice("service_filter", "Service", ServiceFilter::All)
        .help("Shows the messages of this service only.");

sdrkit_core::settings! {
    /// The settings of [`InmcDisplay`].
    pub enum InmcDisplaySetting {
        /// Satellite.
        Satellite = SATELLITE,
        /// Priority.
        MinPriority = MIN_PRIORITY,
        /// Service.
        ServiceFilter = SERVICE_FILTER,
    }
}

/// The display: packets, system information, EGC messages.
pub struct InmcDisplay {
    clock: Clock,
    live: bool,
    host_clock: sdrkit_core::Clock,
    tuned_hz: f64,
    egc: EgcAssembler,
    channels: Vec<ChannelRow>,
    status: Vec<ChannelInfo>,
    ncs: Option<NcsInfo>,
    uplink_hz: Option<f64>,
    frames_ok: u64,
    frames_bad: u64,
    packets_ok: u64,
    packets_bad: u64,
    counts: std::collections::BTreeMap<u8, (u64, u64)>,
    multi: Vec<(usize, Vec<u8>, usize)>,
    min_priority: MinPriority,
    service_filter: ServiceFilter,
    satellite: Satellite,
    request: Option<HostRequest>,
    last_esn0: f64,
    last_uw: usize,
    last_ser: f64,
    /// The message tables changed since the last report.
    dirty: bool,
}

impl Default for InmcDisplay {
    fn default() -> Self {
        InmcDisplay {
            clock: Clock::default(),
            live: false,
            host_clock: sdrkit_core::Clock::fixed(0.0),
            tuned_hz: 0.0,
            egc: EgcAssembler::new(MESSAGE_CAP),
            channels: Vec::new(),
            status: Vec::new(),
            ncs: None,
            uplink_hz: None,
            frames_ok: 0,
            frames_bad: 0,
            packets_ok: 0,
            packets_bad: 0,
            counts: Default::default(),
            multi: Vec::new(),
            min_priority: MinPriority::All,
            service_filter: ServiceFilter::All,
            satellite: Satellite::None,
            request: None,
            last_esn0: 0.0,
            last_uw: 0,
            last_ser: 0.0,
            dirty: true,
        }
    }
}

impl InmcDisplay {
    fn row(&mut self, ch: usize) -> &mut ChannelRow {
        while self.channels.len() <= ch {
            let i = self.channels.len();
            self.channels.push(ChannelRow {
                index: i,
                ..Default::default()
            });
        }
        &mut self.channels[ch]
    }

    /// The absolute frequency of a channel (its place from the dial plus its carrier error).
    fn channel_hz(&self, ch: usize) -> f64 {
        self.status
            .iter()
            .find(|c| c.index == ch)
            .map_or(self.tuned_hz, |c| {
                self.tuned_hz + c.offset_hz + c.status.carrier_hz
            })
    }

    fn frame(&mut self, ch: usize, f: &CFrame, t: f64, events: &mut Vec<sdrkit_core::Event>) {
        let packets = coding::split_packets(&f.bytes);
        let bb_ok = packets.first().is_some_and(|p| p.0 == 0x7D && p.2);
        let mut frame_number = 0u32;
        {
            let r = self.row(ch);
            r.uw_sum += f.uw_errors as f64;
            r.uw_n += 1.0;
            r.uw_errors_avg = r.uw_sum / r.uw_n;
            if bb_ok {
                r.frames_good += 1;
            } else {
                r.frames_bad += 1;
            }
        }
        if bb_ok {
            self.frames_ok += 1;
        } else {
            self.frames_bad += 1;
        }
        if ch == 0 {
            self.last_esn0 = f.esn0_db;
            self.last_uw = f.uw_errors;
            self.last_ser = f.symbol_errors as f64 / coding::CODED as f64;
        }
        for (d, p, ok) in &packets {
            let e = self.counts.entry(*d).or_default();
            if *ok {
                e.0 += 1;
                self.packets_ok += 1;
            } else {
                e.1 += 1;
                self.packets_bad += 1;
            }
            if !*ok {
                continue;
            }
            match *d {
                0x7D => {
                    if let Some(b) = Bulletin::parse(p) {
                        frame_number = b.frame_number as u32;
                        let reg = egc::region_name(b.ocean_region).to_string();
                        let st = egc::station_name(b.ocean_region, b.station_id).to_string();
                        {
                            let r = self.row(ch);
                            r.frame_number = frame_number;
                            r.region = reg.clone();
                            r.station = st.clone();
                            r.channel_type = b.channel_type_name().into();
                        }
                        if ch == 0 {
                            self.ncs = Some(NcsInfo {
                                bulletin: b,
                                region: reg,
                                station_name: st,
                                signalling_uplink_hz: self.uplink_hz,
                            });
                        }
                    }
                }
                0x6C => {
                    if let Some(u) = egc::signalling_uplink_mhz(p) {
                        if ch == 0 {
                            self.uplink_hz = Some(u * 1e6);
                            if let Some(n) = &mut self.ncs {
                                n.signalling_uplink_hz = self.uplink_hz;
                            }
                        }
                    }
                }
                0xB1 | 0xB2 => {
                    if let Some(e) = EgcPacket::parse(p, true) {
                        self.egc_packet(&e, ch, frame_number, t, events);
                    }
                }
                0xBD => {
                    // The start of a packet split over frames: the inner packet's own descriptor holds its length.
                    if p.len() >= 4 {
                        let inner = p[2..p.len() - 2].to_vec();
                        if inner.len() >= 2 {
                            let want = coding::packet_len(inner[0], inner[1]).unwrap_or(0);
                            self.multi.retain(|m| m.0 != ch);
                            self.multi.push((ch, inner, want));
                        }
                    }
                }
                0xBE => {
                    if p.len() < 4 {
                        continue;
                    }
                    let part = &p[2..p.len() - 2];
                    if let Some(i) = self.multi.iter().position(|m| m.0 == ch) {
                        let (_, mut v, want) = self.multi.remove(i);
                        v.extend_from_slice(part);
                        if v.len() >= want {
                            v.truncate(want);
                            if let Some(e) = EgcPacket::parse(&v, true) {
                                self.egc_packet(&e, ch, frame_number, t, events);
                            }
                        } else {
                            self.multi.push((ch, v, want));
                        }
                    }
                }
                _ => {}
            }
        }
    }

    fn egc_packet(
        &mut self,
        e: &EgcPacket,
        ch: usize,
        frame: u32,
        t: f64,
        events: &mut Vec<sdrkit_core::Event>,
    ) {
        let at = self.clock.stamp(t);
        let Some(key) = self.egc.add(e, at, ch, frame) else {
            return;
        };
        self.row(ch).messages += 1;
        self.dirty = true;
        let freq = self.channel_hz(ch);
        let Some(m) = self.egc.messages.get(&key) else {
            return;
        };
        events.push(egc_event(m, t, freq));
    }

    fn publish(&mut self, p: &mut Published) {
        let main = self.status.first().map(|c| c.status).unwrap_or_default();
        let decoding = self.channels.first().is_some_and(|r| r.frames_good > 0) && main.state == 2;
        let state = if decoding {
            2
        } else if main.carrier_found {
            1
        } else {
            0
        };
        let ebn0 = self.last_esn0 + 3.0;
        let mut summary = String::new();
        let _ = match state {
            2 => write!(
                summary,
                "Inmarsat-C: frame {}, Eb/N0 {:.1} dB, {} messages",
                self.channels.first().map_or(0, |r| r.frame_number),
                ebn0,
                self.egc.seen
            ),
            1 => write!(
                summary,
                "Inmarsat-C: carrier locked, Es/N0 {:.1} dB",
                main.esn0_db
            ),
            _ => write!(summary, "Inmarsat-C: searching"),
        };
        for ci in self.status.clone() {
            let r = self.row(ci.index);
            r.offset_hz = ci.offset_hz + ci.status.carrier_hz;
            r.carrier_found = ci.status.carrier_found;
            r.state = ci.status.state;
            r.ebn0_db = ci.status.esn0_db + 3.0;
        }
        self.channels.truncate(self.status.len().max(1));
        let now = self.clock.now();
        let lock_state = LockState::from_level(state);
        let offset = Some(main.carrier_hz);
        let snr = (state > 0).then_some(main.esn0_db as f32);
        p.results_mut().update_status(|st| {
            let same = st.lock_state == lock_state
                && st.offset_hz == offset
                && st.snr_db == snr
                && st.summary == summary;
            if !same {
                st.lock_state = lock_state;
                st.offset_hz = offset;
                st.snr_db = snr;
                st.summary = summary;
            }
            !same
        });
        let measured = self.frames_ok + self.frames_bad > 0;
        let ncs = self.ncs.as_ref();
        let b = ncs.map(|n| &n.bulletin);
        let status_text = b.map(|b| b.status_text());
        let services_text = b.map(|b| b.services_text());
        p.put(&InmarsatcFields {
            frames_ok: self.frames_ok as i64,
            frames_bad: self.frames_bad as i64,
            offset_hz: Hertz(main.carrier_hz),
            drift_hz_s: main.drift_hz_s,
            frames_found: main.frames_found as i64,
            packets_ok: self.packets_ok as i64,
            packets_bad: self.packets_bad as i64,
            message_count: self.egc.seen as i64,
            esn0_db: measured.then_some(self.last_esn0),
            ebn0_db: measured.then_some(ebn0),
            uw_errors: measured.then_some(self.last_uw as i64),
            uw_errors_avg: self.channels.first().map(|r| r.uw_errors_avg),
            symbol_error_rate: measured.then_some(self.last_ser),
            polarity_slips: main.polarity_slips as i64,
            ocean_region_code: b.map(|b| b.ocean_region as i64),
            region: ncs.map(|n| n.region.as_str()),
            station_id: b.map(|b| b.station_id as i64),
            station_name: ncs.map(|n| n.station_name.as_str()),
            channel_type_code: b.map(|b| b.channel_type as i64),
            channel_type: b.map(|b| b.channel_type_name()),
            network_version: b.map(|b| b.network_version as i64),
            signalling_channel: b.map(|b| b.signalling_channel as i64),
            random_interval: b.map(|b| b.random_interval as i64),
            status_byte: b.map(|b| i64::from(b.status)),
            status_text: status_text.as_deref(),
            services_mask: b.map(|b| i64::from(b.services)),
            services_text: services_text.as_deref(),
            signalling_uplink_hz: ncs.and_then(|n| n.signalling_uplink_hz).map(Hertz),
            frame_number: b.map(|b| b.frame_number as i64),
            frame_time: b.map(|b| Seconds(f64::from(b.frame_number as u32) * 8.64)),
        });
        if std::mem::take(&mut self.dirty) {
            let (min_p, filter) = (self.min_priority.index() as u8, self.service_filter);
            shaped::rows_where(&self.egc.messages, &INMARSATC_MESSAGES, now, p, |r| {
                r.record.priority >= min_p
                    && match filter {
                        ServiceFilter::SafetyNet => r.record.kind == "SafetyNET",
                        ServiceFilter::FleetNet => r.record.kind == "FleetNET",
                        ServiceFilter::All => true,
                    }
            });
        }
        let mut key = String::new();
        for c in &self.channels {
            let state = match c.state {
                2 => ChannelState::Frames,
                1 => ChannelState::Carrier,
                _ => ChannelState::Searching,
            };
            key.clear();
            let _ = write!(key, "{}", c.index);
            INMARSATC_CHANNELS.put(
                p,
                &key,
                &InmarsatcChannelRow {
                    index: c.index as i64,
                    offset_hz: Hertz(c.offset_hz),
                    carrier: c.carrier_found,
                    state,
                    ebn0_db: c.ebn0_db,
                    uw_errors_avg: c.uw_errors_avg,
                    frame_number: i64::from(c.frame_number),
                    frames_ok: c.frames_good as i64,
                    frames_bad: c.frames_bad as i64,
                    messages: c.messages as i64,
                    region: &c.region,
                    station_name: &c.station,
                    channel_type: &c.channel_type,
                },
            );
        }
        for i in self.channels.len()..MAX_CHANNELS {
            key.clear();
            let _ = write!(key, "{i}");
            p.remove_row(&INMARSATC_CHANNELS, &key);
        }
        for (d, (g, b)) in &self.counts {
            key.clear();
            let _ = write!(key, "{d:02X}");
            INMARSATC_PACKET_COUNTS.put(
                p,
                &key,
                &InmarsatcPacketCountRow {
                    descriptor: i64::from(*d),
                    descriptor_name: egc::descriptor_name(*d),
                    packets_ok: *g as i64,
                    packets_bad: *b as i64,
                },
            );
        }
    }

    fn drop_signal(&mut self) {
        self.channels.clear();
        self.status.clear();
        self.ncs = None;
        self.multi.clear();
        self.frames_ok = 0;
        self.frames_bad = 0;
    }
}

impl Decoder for InmcDisplay {
    type Input = Timed<InmcItem>;
    fn name(&self) -> &'static str {
        "satcom.inmarsatc_display"
    }
    fn outputs(&self) -> Vec<Port> {
        vec![
            Port::new("events", Kind::Messages).rate_from(RateFrom::Fixed(0.0)),
            Port::typed::<HostRequest>("requests").rate_from(RateFrom::Fixed(0.0)),
        ]
    }
    fn settings(&self) -> Arc<Schema> {
        InmcDisplaySetting::schema()
    }
    fn values(&self) -> Values {
        values_of([
            (
                InmcDisplaySetting::Satellite,
                Value::Choice(self.satellite.index()),
            ),
            (
                InmcDisplaySetting::MinPriority,
                Value::Choice(self.min_priority.index()),
            ),
            (
                InmcDisplaySetting::ServiceFilter,
                Value::Choice(self.service_filter.index()),
            ),
        ])
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<(), SettingError> {
        match InmcDisplaySetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            InmcDisplaySetting::Satellite => {
                let c = SATELLITE.read(id, v)?;
                if c != self.satellite {
                    if let Some(hz) = satellite_hz(c) {
                        self.request =
                            Some(HostRequest::retune(hz, "Inmarsat-C NCS common channel"));
                    }
                }
                self.satellite = c;
            }
            InmcDisplaySetting::MinPriority => {
                self.min_priority = MIN_PRIORITY.read(id, v)?;
                self.dirty = true;
            }
            InmcDisplaySetting::ServiceFilter => {
                self.service_filter = SERVICE_FILTER.read(id, v)?;
                self.dirty = true;
            }
        }
        Ok(())
    }
    fn results(&self) -> Published {
        Published::from_decl(display_decl())
    }
    fn configure(&mut self, f: &Format) -> Result<Vec<Format>, ConfigError> {
        self.clock.restart(self.live, self.host_clock.now());
        self.tuned_hz = f.tuned_hz;
        self.drop_signal();
        Ok(vec![
            Format::new(Kind::Messages, 0.0),
            Format::new(Kind::typed::<HostRequest>(), 0.0),
        ])
    }
    fn retune(&mut self, f: &Format) {
        self.tuned_hz = f.tuned_hz;
    }
    fn attach(&mut self, ctx: &RunContext) {
        self.live = ctx.live;
        self.host_clock = ctx.clock.clone();
    }
    fn process(&mut self, items: &[Timed<InmcItem>], _first: u64, out: &mut Outbox) {
        let mut events = std::mem::take(out.messages(0));
        for it in items {
            let t = self.clock.at(it.time);
            match &it.value {
                InmcItem::Frame { channel, frame } => self.frame(*channel, frame, t, &mut events),
                InmcItem::Status { channels } => self.status.clone_from(channels),
                // A retune: signal state, channels and the NCS information go; the messages stay.
                InmcItem::Reset => self.drop_signal(),
            }
        }
        *out.messages(0) = events;
        if let Some(r) = self.request.take() {
            out.items::<HostRequest>(1).push(Timed {
                time: self.clock.now(),
                value: r,
            });
        }
    }
    fn report(&mut self, results: &mut Published) {
        self.publish(results);
    }
    fn clear(&mut self) {
        Tables::clear(&mut self.egc);
        for r in &mut self.channels {
            r.messages = 0;
        }
        self.dirty = true;
    }
    fn forget(&mut self, kept: &sdrkit_graph::Kept) {
        if !kept.table(INMARSATC_MESSAGES.key) {
            Tables::clear(&mut self.egc);
        }
        for r in &mut self.channels {
            r.messages = 0;
        }
        self.dirty = true;
    }
    fn save_state(&self) -> Option<StateBlob> {
        self.egc.save_state(self.clock.now()).ok()
    }
    fn restore_state(&mut self, state: &StateBlob) -> Result<(), StateError> {
        self.egc.restore_state(state, self.clock.now())?;
        self.dirty = true;
        Ok(())
    }
}

sdrkit_core::settings! {
    /// The settings of the Inmarsat-C stack. Noise and other impairments of the test signal come from the standard `gen_*`
    /// settings.
    pub enum InmcSetting {
        /// Channel search.
        ChannelSearch = CHANNEL_SEARCH,
        /// Search span.
        SearchSpanHz = SEARCH_SPAN_HZ,
        /// Satellite.
        Satellite = SATELLITE,
        /// Priority.
        MinPriority = MIN_PRIORITY,
        /// Service.
        ServiceFilter = SERVICE_FILTER,
        /// Polarity of the test signal.
        GenInvert = gen::GEN_INVERT,
        /// First frame of the test signal.
        GenFirstFrame = gen::GEN_FIRST_FRAME,
        /// Extra channels of the test signal.
        GenExtra = gen::GEN_EXTRA,
        /// Start of the test signal.
        GenStartSymbols = gen::GEN_START_SYMBOLS,
    }
}

/// The receiver: the demodulator and the display as a graph.
///
/// # Errors
///
/// A [`GraphError`] if a setting of `cfg` is refused.
pub fn receiver(cfg: &StackConfig) -> Result<StackGraph, GraphError> {
    let graph = GraphBuilder::new()
        .add("demod", DecoderComponent::new(InmcDemod::default()))
        .add("display", DecoderComponent::new(InmcDisplay::default()))
        .input("demod.in")
        .connect("demod.out", "display.in")
        .output("display.events")
        .output("display.requests")
        .status_from("display")
        .clock(REPORT_S)
        .catalog(&Catalog::of(cfg)?)
        .build()?;
    StackGraph::of(crate::typed::shape(ID), graph)
        .min_rate(
            MIN_RATE,
            "Inmarsat-C needs a sample rate of at least 250 ksps",
        )
        .default_centre(DESCRIPTOR.tuning.default_hz + DESCRIPTOR.tuning.tune_offset_hz)
        .route("channel_search", &["demod.channel_search"])?
        .route("search_span_hz", &["demod.search_span_hz"])?
        .route("satellite", &["display.satellite"])?
        .route("min_priority", &["display.min_priority"])?
        .route("service_filter", &["display.service_filter"])?
        .command("clear", sdrkit_graph::CommandTarget::every("clear"))
        .with_settings(&cfg.settings)
        .build()
}

/// Adds the stack's components to a registry.
pub fn register(r: &mut Registry) {
    r.add("satcom.inmarsatc_demod", || {
        Box::new(DecoderComponent::new(InmcDemod::default()))
    });
    r.add("satcom.inmarsatc_display", || {
        Box::new(DecoderComponent::new(InmcDisplay::default()))
    });
}

fn make_receiver(cfg: &StackConfig) -> Result<Box<dyn Receiver>, ConfigError> {
    crate::stack_receiver(receiver(cfg))
}

fn make_generator(cfg: &GenConfig) -> Result<Box<dyn Generator>, ConfigError> {
    Ok(Box::new(gen::InmcGenerator::from_config(cfg)))
}

/// The engine's checks of the test signal: frames decode and a message completes.
pub static HOOKS: VerifyHooks = VerifyHooks {
    cases: verify_cases,
    vector_settings: None,
    check_vector: None,
};

fn verify_cases(o: &VerifyOptions) -> Vec<VerifyCase> {
    let mut c = VerifyCase::new("test signal, NCS common channel", o, 0.0);
    // A frame lasts 8.64 s: three frames give a bulletin board and a complete message.
    c.seconds = o.seconds.max(30.0);
    c.expect_audio = Some(false);
    c.check = Some(check_frames);
    vec![c]
}

fn check_frames(r: &sdrkit_core::Results) -> Vec<CheckLine> {
    let num = |k: &str| crate::field(r, k);
    vec![
        CheckLine::new(
            "frames_ok",
            num("frames_ok") >= 1.0,
            format!("{} frames good", num("frames_ok")),
        ),
        CheckLine::new(
            "message_count",
            num("message_count") >= 1.0,
            format!("{} messages", num("message_count")),
        ),
    ]
}

/// The descriptor of the stack.
pub static DESCRIPTOR: StackDescriptor = StackDescriptor {
    id: ID,
    name: NAME,
    group: "Satellite",
    blurb: "Inmarsat-C NCS and LES channels: the bulletin board and SafetyNET / FleetNET messages. Receive only.",
    tuning: Tuning {
        min_hz: 1525e6,
        max_hz: 1559e6,
        default_hz: 1537.100e6,
        rate_hz: 2e6,
        min_rate_hz: MIN_RATE,
        bandwidth_hz: 5e3,
        tune_offset_hz: 50e3,
        carrier_at_centre: false,
        not_circular: false,
        radio_filter_hz: Some(1.75e6),
        no_offset_tuning: false,
    },
    submodes: &[],
    settings: InmcSetting::schema,
    make_receiver,
    make_generator: Some(make_generator),
    lock_profile: LockProfile::DEFAULT,
    signal_types: &[SignalType::InmarsatC],
    auto_gain: AutoGain::Radio,
    verify: Some(&HOOKS),
    benchmarks: Some(crate::benchmarks),
};
