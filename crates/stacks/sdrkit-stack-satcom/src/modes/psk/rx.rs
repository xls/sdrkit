//! The PSK receiver: the channel finder with its zoom, the channeliser, the analyser and the demodulator.

use super::bits::Resolution;
use super::{ANALYSIS_LEN, MIN_RATE, SPS};
use sdrkit_algo_dsp::channel::{ChannelConfig, Channelizer};
use sdrkit_algo_modem::psk::{
    analyse, find_region, Analysis, Found, PskDemodConfig, PskDemodulator, PskOrder, PskPulse,
    SpectrumSum,
};
use sdrkit_core::publish::field;
use sdrkit_core::{
    Choice, Complex32, ConfigError, Schema, Setting, SettingError, SettingId, SettingSet, Value,
    Values,
};
use sdrkit_graph::{values_of, Decoder, Format, Item, Kind, Outbox, Port, SlicePool, Timed};
use std::collections::VecDeque;
use std::sync::Arc;

/// What the receiver passes on.
#[derive(Clone, Debug)]
pub enum PskItem {
    /// The analyser's result for a block.
    Analysis(Arc<Analysis>),
    /// Demodulated symbols (unit power).
    Symbols {
        order: PskOrder,
        syms: Arc<[Complex32]>,
    },
    /// Bits from the bit chain: hard bits, soft bits (positive means 0), the index of the first, and the bit index after each
    /// sync word found.
    Bits {
        bits: Arc<[u8]>,
        soft: Arc<[f32]>,
        first: u64,
        syncs: Arc<[u64]>,
        resolution: Resolution,
        total_syncs: u64,
    },
    /// The receiver's state at each report.
    Status(Arc<RxStatus>),
}

impl Item for PskItem {
    const KIND: &'static str = "satcom.psk_demod_item";
}

/// The receiver's state at each report.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct RxStatus {
    /// 0 searching, 1 a signal (analysing, no lock), 2 locked.
    pub state: u8,
    pub found: Option<Found>,
    /// The carrier, Hz from the dial.
    pub carrier_hz: f64,
    /// The symbol rate the timing loop runs at.
    pub baud: f64,
    pub order: Option<PskOrder>,
    /// The carrier loop's lock value.
    pub lock: f64,
    pub evm: f64,
    pub esn0_db: f64,
    pub ebn0_db: f64,
    /// The slope of the carrier over the last 5 s.
    pub doppler_hz_s: f64,
    pub symbols: u64,
    /// Traces of the channel's I rail, 16 samples each.
    pub eye: Vec<[f32; 16]>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum State {
    Search,
    Analyse,
    Locked,
}

/// The receiver works in pieces aligned to this many input samples.
const PIECE: u64 = 256;
/// Traces of the eye kept for each report.
const EYE_TRACES: usize = 48;

sdrkit_core::choice_enum! {
    /// The modulation order the demodulator uses.
    #[derive(Default)]
    pub enum OrderChoice {
        /// The analyser chooses.
        #[default]
        Auto = ("auto", "Found by the analyser"),
        /// BPSK.
        Bpsk = ("bpsk", "BPSK"),
        /// QPSK.
        Qpsk = ("qpsk", "QPSK"),
        /// Offset QPSK.
        Oqpsk = ("oqpsk", "OQPSK"),
        /// 8PSK.
        Psk8 = ("8psk", "8PSK"),
    }
}

impl OrderChoice {
    /// The order, or `None` when the analyser chooses.
    pub fn order(self) -> Option<PskOrder> {
        match self {
            OrderChoice::Auto => None,
            OrderChoice::Bpsk => Some(PskOrder::Bpsk),
            OrderChoice::Qpsk => Some(PskOrder::Qpsk),
            OrderChoice::Oqpsk => Some(PskOrder::Oqpsk),
            OrderChoice::Psk8 => Some(PskOrder::Psk8),
        }
    }
}

sdrkit_core::choice_enum! {
    /// The pulse shape the demodulator uses.
    #[derive(Default)]
    pub enum ShapeChoice {
        /// The analyser chooses.
        #[default]
        Auto = ("auto", "Found by the analyser"),
        /// Rectangular pulses.
        Rect = ("rect", "Rectangular"),
        /// Root raised cosine, roll-off 0.2.
        Rrc020 = ("0.2", "RRC 0.2"),
        /// Root raised cosine, roll-off 0.25.
        Rrc025 = ("0.25", "RRC 0.25"),
        /// Root raised cosine, roll-off 0.35.
        Rrc035 = ("0.35", "RRC 0.35"),
        /// Root raised cosine, roll-off 0.5.
        Rrc050 = ("0.5", "RRC 0.5"),
        /// Root raised cosine, roll-off 0.8.
        Rrc080 = ("0.8", "RRC 0.8"),
        /// Root raised cosine, roll-off 1.
        Rrc100 = ("1.0", "RRC 1.0"),
    }
}

impl ShapeChoice {
    /// The pulse, or `None` when the analyser chooses.
    pub fn pulse(self) -> Option<PskPulse> {
        match self {
            ShapeChoice::Auto => None,
            ShapeChoice::Rect => Some(PskPulse::Rect),
            ShapeChoice::Rrc020 => Some(PskPulse::Rrc(0.2)),
            ShapeChoice::Rrc025 => Some(PskPulse::Rrc(0.25)),
            ShapeChoice::Rrc035 => Some(PskPulse::Rrc(0.35)),
            ShapeChoice::Rrc050 => Some(PskPulse::Rrc(0.5)),
            ShapeChoice::Rrc080 => Some(PskPulse::Rrc(0.8)),
            ShapeChoice::Rrc100 => Some(PskPulse::Rrc(1.0)),
        }
    }
}

/// The modulation order.
pub const ORDER: Setting<field::Choice<OrderChoice>> =
    Setting::choice("order", "Order", OrderChoice::Auto);
/// The symbol rate (0: found by the analyser).
pub const BAUD: Setting<field::Hz> = Setting::new("baud", "Symbol rate", "Bd", 0.0)
    .range(0.0, 1e6, 1.0)
    .help("0: found by the analyser.");
/// The pulse shape.
pub const SHAPE: Setting<field::Choice<ShapeChoice>> =
    Setting::choice("shape", "Pulse shape", ShapeChoice::Auto);
/// The span searched around the dial.
pub const SPAN: Setting<field::Text> = Setting::new("span", "Search span", "", "auto").help(
    "auto (200 kHz or what the radio gives), a number of Hz (2500 to 200000), 500k, 2M or whole.",
);
/// The symbol rate snaps to a standard rate.
pub const SNAP_STANDARD: Setting<field::Bool> =
    Setting::new("snap_standard", "Snap to standard rates", "", true);
/// The channel follows a moving carrier.
pub const FOLLOW_DOPPLER: Setting<field::Bool> =
    Setting::new("follow_doppler", "Follow Doppler", "", true);
/// The clicked frequency, Hz from the dial.
pub const CLICK_HZ: Setting<field::Text> = Setting::new("click_hz", "Clicked frequency", "Hz", "")
    .help("Hz from the dial; empty: the strongest signal.");

sdrkit_core::settings! {
    /// The settings of [`PskRx`].
    pub enum PskRxSetting {
        /// Order.
        Order = ORDER,
        /// Symbol rate.
        Baud = BAUD,
        /// Pulse shape.
        Shape = SHAPE,
        /// Search span.
        Span = SPAN,
        /// Snap to standard rates.
        SnapStandard = SNAP_STANDARD,
        /// Follow Doppler.
        FollowDoppler = FOLLOW_DOPPLER,
        /// Clicked frequency.
        ClickHz = CLICK_HZ,
    }
}

/// The span setting in Hz at input rate `rate`: "auto" (200 kHz or the rate of the radio), a number of Hz from 2500 to 200000,
/// "500k", "2M" or "whole". `None` for any other text.
pub fn span_hz(s: &str, rate: f64) -> Option<f64> {
    let auto = rate.min(200e3);
    match s.trim() {
        "" | "auto" => Some(auto),
        "500k" => Some(500e3f64.min(rate)),
        "2M" => Some(2e6f64.min(rate)),
        "whole" => Some(rate),
        x => x
            .parse::<f64>()
            .ok()
            .filter(|v| (2.5e3..=200e3).contains(v))
            .map(|v| v.min(rate)),
    }
}

/// A channel at `offset_hz` from the input's centre, at exactly `out_rate`.
fn channel(rate: f64, offset_hz: f64, out_rate: f64) -> Option<Channelizer> {
    let out = out_rate.min(rate);
    let mut cfg = ChannelConfig::new(rate, offset_hz, 0.8 * out, out);
    cfg.exact_rate = true;
    Channelizer::new(cfg).ok()
}

/// The delay of a channel's filters in output samples: where the output of an impulse at its input peaks.
fn delay_of(chan: &Channelizer, rate: f64) -> usize {
    let mut c = chan.clone();
    c.reset();
    let ratio = (rate / c.output_rate()).ceil().max(1.0) as usize;
    let mut x = vec![Complex32::default(); ratio * 512];
    x[0] = Complex32::new(1.0, 0.0);
    let mut y = Vec::with_capacity(600);
    c.process(&x, &mut y);
    y.iter()
        .enumerate()
        .max_by(|a, b| a.1.norm_sqr().total_cmp(&b.1.norm_sqr()))
        .map_or(0, |(i, _)| i)
}

/// The PSK receiver.
pub struct PskRx {
    rate: f64,
    dial: f64,
    force_order: OrderChoice,
    force_baud: f64,
    force_shape: ShapeChoice,
    span_text: String,
    click_text: String,
    span: f64,
    snap: bool,
    follow: bool,
    click: Option<f64>,
    state: State,
    level: Option<Channelizer>,
    level_rate: f64,
    level_centre: f64,
    spec: Option<SpectrumSum>,
    zooms: usize,
    found: Option<Found>,
    chan: Option<Channelizer>,
    chan_rate: f64,
    /// The delay of the channel's filters, output samples.
    chan_delay: usize,
    /// Channel samples the analyser still skips while the filters settle.
    warm_up: usize,
    centre: f64,
    block: Vec<Complex32>,
    fails: u32,
    demod: Option<PskDemodulator>,
    baud: f64,
    order: PskOrder,
    unlocked_syms: u64,
    track: VecDeque<(f64, f64)>,
    eye: Vec<[f32; 16]>,
    eye_cur: [f32; 16],
    eye_n: usize,
    tmp: Vec<Complex32>,
    syms: Vec<Complex32>,
    pool: SlicePool<Complex32>,
    samples: u64,
}

impl Default for PskRx {
    fn default() -> Self {
        PskRx {
            rate: 0.0,
            dial: 0.0,
            force_order: OrderChoice::Auto,
            force_baud: 0.0,
            force_shape: ShapeChoice::Auto,
            span_text: "auto".into(),
            click_text: String::new(),
            span: 0.0,
            snap: true,
            follow: true,
            click: None,
            state: State::Search,
            level: None,
            level_rate: 0.0,
            level_centre: 0.0,
            spec: None,
            zooms: 0,
            found: None,
            chan: None,
            chan_rate: 0.0,
            chan_delay: 0,
            warm_up: 0,
            centre: 0.0,
            block: Vec::new(),
            fails: 0,
            demod: None,
            baud: 0.0,
            order: PskOrder::Bpsk,
            unlocked_syms: 0,
            track: VecDeque::new(),
            eye: Vec::with_capacity(EYE_TRACES),
            eye_cur: [0.0; 16],
            eye_n: 0,
            tmp: Vec::new(),
            syms: Vec::new(),
            pool: SlicePool::default(),
            samples: 0,
        }
    }
}

impl PskRx {
    fn start_search(&mut self) {
        self.state = State::Search;
        self.zooms = 0;
        self.found = None;
        self.demod = None;
        self.chan = None;
        self.block.clear();
        self.track.clear();
        let span = self.span.min(self.rate);
        // The first level: the span around the dial, at about the span's rate.
        let r = if span >= self.rate * 0.9 {
            self.rate
        } else {
            (span * 1.25).min(self.rate)
        };
        self.level_rate = r;
        self.level_centre = self.dial;
        self.level = channel(self.rate, self.dial, r);
        self.spec = SpectrumSum::new(2048).ok();
    }

    fn zoom(&mut self, centre: f64, rate: f64) {
        self.level_rate = rate;
        self.level_centre = centre;
        self.level = channel(self.rate, centre, rate);
        self.spec = SpectrumSum::new(1024).ok();
        self.zooms += 1;
    }

    fn evaluate_search(&mut self) {
        let Some(spec) = self.spec.as_ref() else {
            return;
        };
        let n = spec.len();
        let bin = self.level_rate / n as f64;
        let f0 = self.level_centre - self.dial - self.level_rate / 2.0;
        let half = (self.span / 2.0).min(0.45 * self.level_rate);
        let (lo, hi) = if self.zooms == 0 {
            (-half, half)
        } else {
            let c = self.level_centre - self.dial;
            (c - 0.45 * self.level_rate, c + 0.45 * self.level_rate)
        };
        // The radio's DC lies at -dial from the dial.
        let dc = Some(-self.dial);
        let want = if self.zooms == 0 {
            self.click.map(|c| (c, 5e3))
        } else {
            Some((self.level_centre - self.dial, 1e3))
        };
        let frames = spec.frames().max(1) as f32;
        let acc: Vec<f32> = spec.sum().iter().map(|v| v / frames).collect();
        match find_region(&acc, f0, bin, lo, hi, dc, want) {
            Some(f) => {
                let bins = f.width_3db_hz / bin;
                if bins < 40.0 && self.zooms < 4 && f.width_3db_hz * 25.6 < self.level_rate * 0.8 {
                    let r = (f.width_3db_hz.max(bin) * 25.6).max(MIN_RATE / 4.0);
                    self.found = Some(f.clone());
                    self.zoom(self.dial + f.centre_hz, r);
                } else {
                    self.found = Some(f.clone());
                    self.centre = self.dial + f.centre_hz;
                    let w = f.width_3db_hz.max(bin * 2.0);
                    self.chan_rate = (8.0 * w).clamp(MIN_RATE / 8.0, self.rate);
                    self.chan = channel(self.rate, self.centre, self.chan_rate);
                    // The analyser reads the channel after its filters have settled.
                    self.warm_up = self.chan.as_ref().map_or(0, |c| 2 * delay_of(c, self.rate));
                    self.block.clear();
                    self.state = State::Analyse;
                }
            }
            None => {
                if self.zooms > 0 {
                    // Lost while zooming: start again.
                    self.start_search();
                } else if let Some(s) = self.spec.as_mut() {
                    s.reset();
                }
            }
        }
    }

    fn run_analysis(&mut self, out: &mut Vec<Timed<PskItem>>, t: f64) {
        let width = self
            .found
            .as_ref()
            .map_or(self.chan_rate / 10.0, |f| f.width_3db_hz);
        let mut a = analyse(
            &self.block,
            self.chan_rate,
            width,
            self.force_order.order(),
            self.force_baud,
            self.force_shape.pulse(),
            self.snap,
        );
        a.carrier_hz += self.centre - self.dial;
        self.block.clear();
        let demod = match (a.signal, a.order) {
            (true, Some(order)) => {
                let mut cfg = PskDemodConfig::new(order, SPS);
                cfg.pulse = match a.shape {
                    Some(PskPulse::Rrc(b)) => PskPulse::Rrc(b.max(0.1)),
                    _ => PskPulse::Rect,
                };
                PskDemodulator::new(cfg).ok().map(|d| (order, d))
            }
            _ => None,
        };
        match demod {
            Some((order, d)) => {
                self.order = order;
                self.baud = a.baud;
                self.centre = self.dial + a.carrier_hz;
                self.chan_rate = SPS * a.baud;
                self.chan = channel(self.rate, self.centre, self.chan_rate);
                self.chan_delay = self.chan.as_ref().map_or(0, |c| delay_of(c, self.rate));
                self.demod = Some(d);
                self.state = State::Locked;
                self.unlocked_syms = 0;
                self.fails = 0;
            }
            None => {
                self.fails += 1;
                if self.fails >= 3 {
                    self.fails = 0;
                    self.start_search();
                }
            }
        }
        out.push(Timed {
            time: t,
            value: PskItem::Analysis(Arc::new(a)),
        });
    }

    fn step(&mut self, iq: &[Complex32], first: u64, out: &mut Vec<Timed<PskItem>>) {
        self.samples = first + iq.len() as u64;
        let t = self.samples as f64 / self.rate;
        // A change of state (a new channel) happens only at the end of a whole piece, so it lands on the same sample however the
        // input is cut.
        let aligned = self.samples.is_multiple_of(PIECE);
        self.tmp.clear();
        match self.state {
            State::Search => {
                let Some(level) = self.level.as_mut() else {
                    return;
                };
                level.process(iq, &mut self.tmp);
                let Some(spec) = self.spec.as_mut() else {
                    return;
                };
                spec.push(&self.tmp);
                if spec.frames() >= 8 && aligned {
                    self.evaluate_search();
                }
            }
            State::Analyse => {
                let Some(chan) = self.chan.as_mut() else {
                    self.start_search();
                    return;
                };
                chan.process(iq, &mut self.tmp);
                let skip = self.warm_up.min(self.tmp.len());
                self.warm_up -= skip;
                let rest = &self.tmp[skip..];
                let take = (ANALYSIS_LEN - self.block.len()).min(rest.len());
                self.block.extend_from_slice(&rest[..take]);
                if self.block.len() >= ANALYSIS_LEN && aligned {
                    self.run_analysis(out, t);
                }
            }
            State::Locked => self.demodulate(iq, t, aligned, out),
        }
    }

    fn demodulate(
        &mut self,
        iq: &[Complex32],
        t: f64,
        aligned: bool,
        out: &mut Vec<Timed<PskItem>>,
    ) {
        let (Some(chan), Some(d)) = (self.chan.as_mut(), self.demod.as_mut()) else {
            self.start_search();
            return;
        };
        chan.process(iq, &mut self.tmp);
        self.syms.clear();
        let (syms, unlocked) = (&mut self.syms, &mut self.unlocked_syms);
        d.process_with(&self.tmp, |d, run| {
            syms.extend_from_slice(run);
            if d.lock_metric() < 0.2 {
                *unlocked += run.len() as u64;
            } else {
                *unlocked = 0;
            }
            std::ops::ControlFlow::Continue(())
        });
        for &x in &self.tmp {
            if self.eye.len() < EYE_TRACES {
                self.eye_cur[self.eye_n] = x.re;
                self.eye_n += 1;
                if self.eye_n == 16 {
                    self.eye.push(self.eye_cur);
                    self.eye_n = 0;
                }
            }
        }
        if !self.syms.is_empty() {
            let syms = &self.syms;
            let s = self.pool.exact(syms.len(), |o| o.copy_from_slice(syms));
            out.push(Timed {
                time: t,
                value: PskItem::Symbols {
                    order: self.order,
                    syms: s,
                },
            });
        }
        // Keep the channel filter centred on a moving carrier. The demodulator follows the move when it reaches it through the
        // channel's filters.
        if self.follow && aligned && !d.retune_pending() {
            d.hand_over();
            let fh = d.carrier_offset() * self.chan_rate;
            if fh.abs() > 0.02 * self.baud {
                self.centre += fh;
                chan.set_offset(self.centre);
                d.retune_after(self.chan_delay, fh / self.chan_rate);
            }
        }
        // Lost for 3 s: search again.
        if self.unlocked_syms as f64 > 3.0 * self.baud && aligned {
            self.start_search();
        }
    }
}

impl Decoder for PskRx {
    type Input = Complex32;
    fn name(&self) -> &'static str {
        "satcom.psk_demod_receiver"
    }
    fn outputs(&self) -> Vec<Port> {
        vec![Port::typed::<PskItem>("out").same_format_as(0)]
    }
    fn settings(&self) -> Arc<Schema> {
        PskRxSetting::schema()
    }
    fn values(&self) -> Values {
        values_of([
            (PskRxSetting::Order, Value::Choice(self.force_order.index())),
            (PskRxSetting::Baud, Value::Hz(self.force_baud)),
            (PskRxSetting::Shape, Value::Choice(self.force_shape.index())),
            (
                PskRxSetting::Span,
                Value::Text(self.span_text.as_str().into()),
            ),
            (PskRxSetting::SnapStandard, Value::Bool(self.snap)),
            (PskRxSetting::FollowDoppler, Value::Bool(self.follow)),
            (
                PskRxSetting::ClickHz,
                Value::Text(self.click_text.as_str().into()),
            ),
        ])
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<(), SettingError> {
        let restart = match PskRxSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            PskRxSetting::Order => {
                self.force_order = ORDER.read(id, v)?;
                true
            }
            PskRxSetting::Baud => {
                self.force_baud = BAUD.read(id, v)?;
                true
            }
            PskRxSetting::Shape => {
                self.force_shape = SHAPE.read(id, v)?;
                true
            }
            PskRxSetting::Span => {
                let t = SPAN.read(id, v)?;
                if span_hz(t, 1e12).is_none() {
                    return Err(SettingError::Malformed { id, at: Some(0) });
                }
                self.span_text = t.to_string();
                if self.rate > 0.0 {
                    self.span = span_hz(&self.span_text, self.rate).unwrap_or(self.rate);
                }
                true
            }
            PskRxSetting::SnapStandard => {
                self.snap = SNAP_STANDARD.read(id, v)?;
                false
            }
            PskRxSetting::FollowDoppler => {
                self.follow = FOLLOW_DOPPLER.read(id, v)?;
                false
            }
            PskRxSetting::ClickHz => {
                let t = CLICK_HZ.read(id, v)?;
                self.click = match t.trim() {
                    "" => None,
                    x => Some(
                        x.parse::<f64>()
                            .ok()
                            .filter(|f| f.is_finite())
                            .ok_or(SettingError::Malformed { id, at: Some(0) })?,
                    ),
                };
                self.click_text = t.to_string();
                true
            }
        };
        if restart && self.rate > 0.0 {
            self.start_search();
        }
        Ok(())
    }
    fn configure(&mut self, f: &Format) -> Result<Vec<Format>, ConfigError> {
        if f.rate < MIN_RATE {
            return Err(ConfigError::RateTooLow {
                need: MIN_RATE,
                got: f.rate,
            });
        }
        self.rate = f.rate;
        self.dial = f.signal_offset_hz();
        self.span = span_hz(&self.span_text, f.rate).unwrap_or(f.rate);
        self.samples = 0;
        self.start_search();
        Ok(vec![f.with_kind(Kind::typed::<PskItem>())])
    }
    fn retune(&mut self, f: &Format) {
        self.dial = f.signal_offset_hz();
        self.start_search();
    }
    fn process(&mut self, iq: &[Complex32], first: u64, out: &mut Outbox) {
        let out = out.items::<PskItem>(0);
        // Pieces end at multiples of PIECE in absolute samples, so a change of state lands on the same sample however the input is
        // cut.
        let mut done = 0usize;
        while done < iq.len() {
            let at = first + done as u64;
            let n = ((PIECE - at % PIECE) as usize).min(iq.len() - done);
            self.step(&iq[done..done + n], at, out);
            done += n;
        }
    }
    fn tick(&mut self, out: &mut Outbox) {
        let out = out.items::<PskItem>(0);
        let t = self.samples as f64 / self.rate.max(1.0);
        let mut s = RxStatus {
            state: match self.state {
                State::Search => (self.zooms > 0) as u8,
                State::Analyse | State::Locked => 1,
            },
            found: self.found.clone(),
            ..Default::default()
        };
        if let (State::Locked, Some(d)) = (self.state, &self.demod) {
            let carrier = self.centre - self.dial + d.carrier_offset() * self.chan_rate;
            self.track.push_back((t, carrier));
            while self.track.front().is_some_and(|p| t - p.0 > 5.0) {
                self.track.pop_front();
            }
            s.state = if d.lock_metric() > 0.4 { 2 } else { 1 };
            s.carrier_hz = carrier;
            s.baud = self.chan_rate / d.sps_now();
            s.order = Some(self.order);
            s.lock = d.lock_metric() as f64;
            s.evm = d.evm();
            s.esn0_db = -10.0 * d.evm().powi(2).max(1e-6).log10();
            s.ebn0_db = s.esn0_db - 10.0 * (self.order.bits() as f64).log10();
            s.doppler_hz_s = crate::display::slope_of(self.track.make_contiguous());
            s.symbols = d.symbols();
            s.eye = std::mem::take(&mut self.eye);
        }
        out.push(Timed {
            time: t,
            value: PskItem::Status(Arc::new(s)),
        });
    }
}
