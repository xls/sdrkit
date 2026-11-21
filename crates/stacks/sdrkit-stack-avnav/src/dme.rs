//! DME: the pulse pairs of a beacon or of interrogating aircraft on 962 to 1213 MHz. A receiver that does not transmit cannot
//! measure a distance, so this stack listens: it finds the pulses and the pairs, distinguishes X from Y and a beacon from
//! aircraft, counts the pairs, reads the identification, and names the channel and its paired VHF frequency.
//! Standard: ICAO Annex 10 Volume I, 3.5.

use crate::fields::{put_idents, put_status};
use crate::ident::IdentEnvelope;
use crate::keyer::{key_state, push_key, KeyState, Keying};
use crate::morse::{IdentTracker, MorseDecoder, MorseWord};
use crate::pairing::{DmeChannel, DmeMode};
use crate::typed::{DmeFields, IdentFields, TunedFields, IDENTS};
use sdrkit_algo_dsp::pulse::{taps, Norm, Shape};
use sdrkit_algo_dsp::DspError;
use sdrkit_core::interface::{Fields as _, Hertz};
use sdrkit_core::publish::{field, Published};
use sdrkit_core::{
    Choice, CommandError, CommandId, CommandSet, Commands, Complex32, ConfigError, GenConfig,
    Generator, LockState, Receiver, Record, Results, Setting, SettingError, SettingId, SettingSet,
    SignalInfo, StackConfig, StageError, Value, Values,
};
use sdrkit_graph::{
    values_of, Applied, Catalog, Component, ComponentInfo, DecoderCommand, Format, Graph,
    GraphBuilder, GraphError, Io, Item, Kind, Port, RateFrom, StackGraph, Timed,
};
use std::collections::VecDeque;
use std::f64::consts::TAU;
use std::fmt::Write as _;
use std::sync::Arc;

/// The stack id.
pub const ID: &str = "dme";
/// The least radio rate (a 3.5 µs pulse needs at least about 3 samples).
pub const MIN_RATE_HZ: f64 = 1e6;
/// The rate the stack works best at.
pub const RATE_HZ: f64 = 2e6;
/// Identification pairs a second.
pub const IDENT_PPS: f64 = 1350.0;

/// One pulse.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Pulse {
    /// Time of the half-amplitude point of the leading edge, seconds.
    pub time: f64,
    /// Peak amplitude.
    pub amplitude: f32,
    /// Width between the half-amplitude points, µs.
    pub width_us: f32,
}

impl Item for Pulse {
    const KIND: &'static str = "dme.pulse";
}

pub const THRESHOLD: Setting<field::F64> = Setting::new("threshold", "Pulse threshold", "dB", 12.0)
    .range(3.0, 40.0, 0.0)
    .help("How far above the noise a pulse must rise.");
pub const MIN_WIDTH: Setting<field::F64> =
    Setting::new("min_width", "Narrowest pulse", "µs", 1.5).range(0.5, 10.0, 0.0);
pub const MAX_WIDTH: Setting<field::F64> =
    Setting::new("max_width", "Widest pulse", "µs", 7.0).range(1.0, 30.0, 0.0);

sdrkit_core::settings! {
    /// The settings of [`PulseDetect`].
    pub enum PulseDetectSetting {
        /// How far above the noise a pulse must rise.
        Threshold = THRESHOLD,
        /// The narrowest pulse kept.
        MinWidth = MIN_WIDTH,
        /// The widest pulse kept.
        MaxWidth = MAX_WIDTH,
    }
}

/// The pulse detector (`avnav.pulse_detect`, iq -> [`Pulse`]).
///
/// The envelope is compared with the noise floor (the mean envelope of the quiet samples) times `threshold` dB. A pulse ends a
/// little after the envelope falls below the threshold or to 0.3 of its peak (two pulses 12 µs apart barely part at the
/// threshold). Each pulse is timed at the half-amplitude point of its leading edge, interpolated between samples, because the
/// standard measures the spacing there (Annex 10 Volume I, 3.5.4.1.4). A pulse is kept when its width is between `min_width` and
/// `max_width` µs.
pub struct PulseDetect {
    threshold_db: f64,
    min_width: f64,
    max_width: f64,
    rate: f64,
    floor: f64,
    started: bool,
    alpha: f64,
    pre: [f32; 16],
    pre_pos: usize,
    buf: Vec<f32>,
    in_pulse: bool,
    /// The sample index of `buf[0]`. It is below 0 when a pulse starts in the first samples, before the pre-trigger is full.
    start_index: i64,
    below: u32,
    peak: f32,
    n: u64,
}

impl Default for PulseDetect {
    fn default() -> Self {
        PulseDetect {
            threshold_db: THRESHOLD.default,
            min_width: MIN_WIDTH.default,
            max_width: MAX_WIDTH.default,
            rate: 0.0,
            floor: 0.0,
            started: false,
            alpha: 0.0,
            pre: [0.0; 16],
            pre_pos: 0,
            buf: Vec::with_capacity(256),
            in_pulse: false,
            start_index: 0,
            below: 0,
            peak: 0.0,
            n: 0,
        }
    }
}

impl PulseDetect {
    fn finish(&mut self, out: &mut Vec<Timed<Pulse>>) {
        let b = &self.buf;
        let Some((ipk, &pk)) = b.iter().enumerate().max_by(|a, b| a.1.total_cmp(b.1)) else {
            return;
        };
        let half = pk * 0.5;
        // Leading edge: the last sample below half before the peak.
        let mut lead = None;
        for i in (0..ipk).rev() {
            if b[i] < half {
                let f = (half - b[i]) / (b[i + 1] - b[i]).max(1e-12);
                lead = Some(i as f64 + f as f64);
                break;
            }
        }
        let mut trail = None;
        for i in ipk + 1..b.len() {
            if b[i] < half {
                let f = (b[i - 1] - half) / (b[i - 1] - b[i]).max(1e-12);
                trail = Some((i - 1) as f64 + f as f64);
                break;
            }
        }
        if let (Some(l), Some(t)) = (lead, trail) {
            let width = (t - l) / self.rate * 1e6;
            if (self.min_width..=self.max_width).contains(&width) {
                out.push(Timed {
                    time: (self.start_index as f64 + l) / self.rate,
                    value: Pulse {
                        time: (self.start_index as f64 + l) / self.rate,
                        amplitude: pk,
                        width_us: width as f32,
                    },
                });
            }
        }
    }
}

impl Component for PulseDetect {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "avnav.pulse_detect",
            vec![Port::iq("in")],
            vec![Port::typed::<Pulse>("out").rate_from(RateFrom::Fixed(0.0))],
        )
        .with_settings(PulseDetectSetting::schema())
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        let f = &inputs[0];
        if f.rate < MIN_RATE_HZ {
            return Err(ConfigError::RateTooLow {
                need: MIN_RATE_HZ,
                got: f.rate,
            });
        }
        self.rate = f.rate;
        self.alpha = 1.0 - (-1.0 / (0.01 * f.rate)).exp();
        self.started = false;
        self.in_pulse = false;
        self.buf.clear();
        self.pre = [0.0; 16];
        self.n = 0;
        Ok(vec![Format {
            rate: 0.0,
            ..f.with_kind(Kind::typed::<Pulse>())
        }])
    }
    fn process(&mut self, io: &mut Io) {
        let x = io.iq(0);
        let out = io.items_out::<Pulse>(0);
        let k = 10f64.powf(self.threshold_db / 20.0);
        let max_len = (self.max_width * 4.0 * 1e-6 * self.rate) as usize + 16;
        for s in x {
            let m = s.norm();
            if !self.started {
                self.floor = m as f64;
                self.started = true;
            }
            let thr = (self.floor.max(1e-5) * k) as f32;
            if self.in_pulse {
                self.buf.push(m);
                self.peak = self.peak.max(m);
                // Below the threshold, or well down from the peak (two pulses 12 µs apart barely part at the threshold).
                if m < thr || m < 0.3 * self.peak {
                    self.below += 1;
                } else {
                    self.below = 0;
                }
                // The pulse ends a little after the envelope falls below the threshold (the half point can lie further down).
                if self.below >= 3 || self.buf.len() >= max_len {
                    if self.buf.len() < max_len {
                        self.finish(out);
                    }
                    self.in_pulse = false;
                    self.buf.clear();
                }
            } else if m >= thr && m > self.pre[(self.pre_pos + self.pre.len() - 1) % self.pre.len()]
            {
                self.in_pulse = true;
                self.below = 0;
                self.peak = m;
                self.buf.clear();
                // Samples before the crossing, oldest first, so the leading half point is inside.
                let np = self.pre.len();
                for j in 0..np {
                    self.buf.push(self.pre[(self.pre_pos + j) % np]);
                }
                self.buf.push(m);
                self.start_index = self.n as i64 - np as i64;
            } else {
                // The floor learns from quiet samples only.
                self.floor += self.alpha * (m as f64 - self.floor);
            }
            self.pre[self.pre_pos] = m;
            self.pre_pos = (self.pre_pos + 1) % self.pre.len();
            self.n += 1;
        }
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        match PulseDetectSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            PulseDetectSetting::Threshold => self.threshold_db = THRESHOLD.read(id, v)?,
            PulseDetectSetting::MinWidth => self.min_width = MIN_WIDTH.read(id, v)?,
            PulseDetectSetting::MaxWidth => self.max_width = MAX_WIDTH.read(id, v)?,
        }
        Ok(Applied::Live)
    }
    fn values(&self) -> Values {
        values_of([
            (PulseDetectSetting::Threshold, Value::F64(self.threshold_db)),
            (PulseDetectSetting::MinWidth, Value::F64(self.min_width)),
            (PulseDetectSetting::MaxWidth, Value::F64(self.max_width)),
        ])
    }
}

/// What a pulse pair is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PairKind {
    /// 12 µs: X channel, either way.
    X12,
    /// 30 µs: a Y channel reply.
    YReply,
    /// 36 µs: a Y channel interrogation.
    YInterrogation,
}

/// One pulse pair.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PulsePair {
    /// Time of the first pulse, seconds.
    pub time: f64,
    /// The spacing, µs.
    pub spacing_us: f32,
    /// Mean amplitude.
    pub amplitude: f32,
    /// Mean width, µs.
    pub width_us: f32,
    /// Which spacing.
    pub kind: PairKind,
}

impl Item for PulsePair {
    const KIND: &'static str = "dme.pair";
}

sdrkit_core::choice_enum! {
    /// The channel modes whose spacings [`PulsePairs`] looks for.
    pub enum ChannelModeChoice {
        /// X and Y.
        Auto = ("auto", "Automatic"),
        /// X only.
        X = ("x", "X"),
        /// Y only.
        Y = ("y", "Y"),
    }
}

pub const TOLERANCE: Setting<field::F64> =
    Setting::new("tolerance", "Spacing tolerance", "µs", 1.0).range(0.1, 5.0, 0.0);
pub const MODE: Setting<field::Choice<ChannelModeChoice>> =
    Setting::choice("mode", "Channel mode", ChannelModeChoice::Auto);

sdrkit_core::settings! {
    /// The settings of [`PulsePairs`].
    pub enum PulsePairsSetting {
        /// How far a spacing may be from its nominal value.
        Tolerance = TOLERANCE,
        /// The channel modes looked for.
        Mode = MODE,
    }
}

/// Pairs pulses (`avnav.pulse_pairs`, [`Pulse`] -> [`PulsePair`]): a pulse 12, 30 or 36 µs (± `tolerance` µs) after another of
/// similar amplitude (within 6 dB) makes a pair. `mode` limits the spacings looked for.
pub struct PulsePairs {
    tolerance: f64,
    mode: ChannelModeChoice,
    recent: VecDeque<Pulse>,
}

impl Default for PulsePairs {
    fn default() -> Self {
        PulsePairs {
            tolerance: TOLERANCE.default,
            mode: MODE.default,
            recent: VecDeque::with_capacity(16),
        }
    }
}

impl Component for PulsePairs {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "avnav.pulse_pairs",
            vec![Port::typed::<Pulse>("in")],
            vec![Port::typed::<PulsePair>("out").rate_from(RateFrom::Fixed(0.0))],
        )
        .with_settings(PulsePairsSetting::schema())
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        self.recent.clear();
        Ok(vec![inputs[0].with_kind(Kind::typed::<PulsePair>())])
    }
    fn process(&mut self, io: &mut Io) {
        let input = io.items::<Pulse>(0);
        let out = io.items_out::<PulsePair>(0);
        let spacings: &[(f64, PairKind)] = match self.mode {
            ChannelModeChoice::X => &[(12.0, PairKind::X12)],
            ChannelModeChoice::Y => &[(30.0, PairKind::YReply), (36.0, PairKind::YInterrogation)],
            ChannelModeChoice::Auto => &[
                (12.0, PairKind::X12),
                (30.0, PairKind::YReply),
                (36.0, PairKind::YInterrogation),
            ],
        };
        for p in input {
            let p = p.value;
            while self.recent.front().is_some_and(|q| p.time - q.time > 40e-6) {
                self.recent.pop_front();
            }
            let mut paired = None;
            for (i, q) in self.recent.iter().enumerate() {
                let d = (p.time - q.time) * 1e6;
                let ratio = p.amplitude / q.amplitude.max(1e-12);
                if !(0.5..=2.0).contains(&ratio) {
                    continue;
                }
                if let Some(&(_, kind)) = spacings
                    .iter()
                    .find(|(s, _)| (d - s).abs() <= self.tolerance)
                {
                    paired = Some((i, d, kind));
                    break;
                }
            }
            if let Some((i, d, kind)) = paired {
                let q = self.recent[i];
                out.push(Timed {
                    time: q.time,
                    value: PulsePair {
                        time: q.time,
                        spacing_us: d as f32,
                        amplitude: 0.5 * (p.amplitude + q.amplitude),
                        width_us: 0.5 * (p.width_us + q.width_us),
                        kind,
                    },
                });
                // Both pulses are used.
                self.recent.drain(..=i);
            } else {
                if self.recent.len() >= 16 {
                    self.recent.pop_front();
                }
                self.recent.push_back(p);
            }
        }
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        match PulsePairsSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            PulsePairsSetting::Tolerance => self.tolerance = TOLERANCE.read(id, v)?,
            PulsePairsSetting::Mode => self.mode = MODE.read(id, v)?,
        }
        Ok(Applied::Live)
    }
    fn values(&self) -> Values {
        values_of([
            (PulsePairsSetting::Tolerance, Value::F64(self.tolerance)),
            (PulsePairsSetting::Mode, Value::Choice(self.mode.index())),
        ])
    }
}

pub const IDENT_TOLERANCE: Setting<field::F64> =
    Setting::new("tolerance", "Ident spacing tolerance", "µs", 1.5).range(0.2, 10.0, 0.0);
pub const MIN_REGULAR: Setting<field::I64> =
    Setting::new("min_regular", "Ident pairs per 10 ms", "", 5).range(1.0, 13.0, 1.0);

sdrkit_core::settings! {
    /// The settings of [`DmeIdent`].
    pub enum DmeIdentSetting {
        /// How far the spacing of identification pairs may be from 1/1350 s.
        Tolerance = IDENT_TOLERANCE,
        /// The regular pairs in 10 ms that make key down.
        MinRegular = MIN_REGULAR,
    }
}

/// The identification keyer (`avnav.dme_ident`, [`PulsePair`] -> [`Keying`]): in each 10 ms, the pairs that follow another pair by
/// 1/1350 s (± `tolerance` µs) are counted. Enough of them (`min_regular`, out of about 13 when keyed) is key down.
pub struct DmeIdent {
    tolerance: f64,
    min_regular: u32,
    history: VecDeque<f64>,
    period_end: Option<f64>,
    regular: u32,
    state: KeyState,
}

impl Default for DmeIdent {
    fn default() -> Self {
        DmeIdent {
            tolerance: IDENT_TOLERANCE.default,
            min_regular: MIN_REGULAR.default as u32,
            history: VecDeque::with_capacity(64),
            period_end: None,
            regular: 0,
            state: key_state(1e6),
        }
    }
}

impl DmeIdent {
    const PERIOD: f64 = 0.01;
    fn close_until(&mut self, t: f64, out: &mut Vec<Timed<Keying>>) {
        while let Some(end) = self.period_end {
            if t < end {
                break;
            }
            let want = self.regular >= self.min_regular;
            let n = (Self::PERIOD * 1e6) as u64;
            push_key(out, self.state.decide(want, self.regular as f32, n, end));
            self.regular = 0;
            self.period_end = Some(end + Self::PERIOD);
        }
    }
}

impl Component for DmeIdent {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "avnav.dme_ident",
            vec![Port::typed::<PulsePair>("in")],
            vec![Port::typed::<Keying>("out").rate_from(RateFrom::Fixed(0.0))],
        )
        .with_settings(DmeIdentSetting::schema())
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        self.history.clear();
        self.period_end = None;
        self.regular = 0;
        // Decisions are counted in microseconds.
        self.state = key_state(1e6);
        Ok(vec![inputs[0].with_kind(Kind::typed::<Keying>())])
    }
    fn process(&mut self, io: &mut Io) {
        let input = io.items::<PulsePair>(0);
        let out = io.items_out::<Keying>(0);
        let period = 1.0 / IDENT_PPS;
        let tol = self.tolerance * 1e-6;
        for p in input {
            let t = p.value.time;
            if self.period_end.is_none() {
                self.period_end = Some((t / Self::PERIOD).floor() * Self::PERIOD + Self::PERIOD);
            }
            self.close_until(t, out);
            while self
                .history
                .front()
                .is_some_and(|&h| t - h > period + 10e-6)
            {
                self.history.pop_front();
            }
            if self
                .history
                .iter()
                .any(|&h| ((t - h) - period).abs() <= tol)
            {
                self.regular += 1;
            }
            if self.history.len() < 64 {
                self.history.push_back(t);
            }
        }
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        match DmeIdentSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            DmeIdentSetting::Tolerance => self.tolerance = IDENT_TOLERANCE.read(id, v)?,
            DmeIdentSetting::MinRegular => self.min_regular = MIN_REGULAR.read(id, v)? as u32,
        }
        Ok(Applied::Live)
    }
    fn values(&self) -> Values {
        values_of([
            (DmeIdentSetting::Tolerance, Value::F64(self.tolerance)),
            (
                DmeIdentSetting::MinRegular,
                Value::I64(i64::from(self.min_regular)),
            ),
        ])
    }
}

sdrkit_core::choice_enum! {
    /// What the DME receiver hears.
    pub enum Heard {
        /// The replies of a ground beacon.
        Beacon = ("beacon", "beacon"),
        /// The interrogations of aircraft.
        Interrogations = ("interrogations", "interrogations"),
        /// Too few pairs to tell.
        Nothing = ("none", "none"),
    }
}

fn new_published() -> Published {
    crate::typed::store(
        &[DmeFields::SHAPE, TunedFields::SHAPE, IdentFields::SHAPE],
        &[&IDENTS],
    )
}

pub const CONFIRM: Setting<field::I64> = Setting::new("confirm", "Hearings to confirm", "", 2)
    .range(1.0, 10.0, 1.0)
    .help("How many times an identification must be heard before it is shown.");

sdrkit_core::settings! {
    /// The settings of [`DmeReport`].
    pub enum DmeReportSetting {
        /// Hearings that confirm an identification.
        Confirm = CONFIRM,
    }
}

/// The DME report (`avnav.dme_report`): pairs and identification in, results out. Its command `clear` forgets the station.
pub struct DmeReport {
    tuned: f64,
    tracker: IdentTracker,
    wpm: Option<f32>,
    window: VecDeque<(f64, PairKind, f32, f32, f32)>,
    pairs: u64,
    published: Published,
    last_time: f64,
    channel: Option<DmeChannel>,
    channel_name: Option<String>,
    summary: String,
}

impl Default for DmeReport {
    fn default() -> Self {
        DmeReport {
            tuned: 0.0,
            tracker: IdentTracker::new(CONFIRM.default as u32),
            wpm: None,
            window: VecDeque::with_capacity(8192),
            pairs: 0,
            published: new_published(),
            last_time: 0.0,
            channel: None,
            channel_name: None,
            summary: String::with_capacity(64),
        }
    }
}

impl DmeReport {
    fn publish(&mut self) {
        let p = &mut self.published;
        p.put(&TunedFields {
            tuned_hz: Hertz(self.tuned),
        });
        put_idents(p, &self.tracker, self.tracker.confirmed(), self.wpm);
        // The window holds the last second of pairs.
        let n = self.window.len();
        let pps = n as f64;
        let (mut x, mut yr, mut yi) = (0, 0, 0);
        let (mut sp, mut w, mut a) = (0.0f64, 0.0f64, 0.0f64);
        for e in &self.window {
            match e.1 {
                PairKind::X12 => x += 1,
                PairKind::YReply => yr += 1,
                PairKind::YInterrogation => yi += 1,
            }
            sp += e.2 as f64;
            w += e.3 as f64;
            a += e.4 as f64;
        }
        let enough = n >= 20;
        let level = 20.0 * (a / n.max(1) as f64).max(1e-9).log10();
        let f = self.tuned / 1e6;
        let (mode, reply) = if !enough {
            (None, true)
        } else if yr >= x && yr >= yi {
            (Some(DmeMode::Y), true)
        } else if yi >= x {
            (Some(DmeMode::Y), false)
        } else {
            // 12 µs both ways: the frequency says which (replies below 1025 or above 1150 MHz).
            (Some(DmeMode::X), !(1024.5..=1150.5).contains(&f))
        };
        let heard = match (mode, reply) {
            (None, _) => Heard::Nothing,
            (Some(_), true) => Heard::Beacon,
            (Some(_), false) => Heard::Interrogations,
        };
        let chan = mode.and_then(|m| DmeChannel::for_frequency(f, m, reply));
        if chan != self.channel {
            self.channel = chan;
            self.channel_name = chan.map(|c| c.name());
        }
        p.put(&DmeFields {
            heard,
            channel_mode: mode,
            channel: self.channel_name.as_deref(),
            paired_vhf_hz: chan
                .and_then(|c| c.vhf_mhz())
                .map(|m| Hertz((m * 1e6).round())),
            pairs_per_s: pps,
            spacing_us: enough.then(|| sp / n as f64),
            width_us: enough.then(|| w / n as f64),
            pairs: self.pairs as i64,
            level_dbfs: enough.then_some(level),
        });
        let ident = self.tracker.confirmed();
        self.summary.clear();
        if enough {
            let words = &Heard::CHOICES[usize::from(heard.index())].label;
            // Writing into a String cannot fail.
            let _ = write!(
                self.summary,
                "{words} {}, {pps:.0} pairs/s",
                self.channel_name.as_deref().unwrap_or("")
            );
            if let Some(id) = ident {
                let _ = write!(self.summary, ", {id}");
            }
        } else {
            self.summary.push_str("Searching");
        }
        let lock_state = match (enough, ident.is_some()) {
            (true, true) => LockState::Locked,
            (true, false) => LockState::Tracking,
            _ => LockState::Searching,
        };
        put_status(
            p,
            lock_state,
            None,
            enough.then_some(level as f32),
            None,
            &self.summary,
        );
    }
}

impl Component for DmeReport {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::sink(
            "avnav.dme_report",
            vec![
                Port::typed::<PulsePair>("pairs"),
                Port::typed::<MorseWord>("ident"),
            ],
        )
        .with_settings(DmeReportSetting::schema())
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        self.tuned = inputs[0].tuned_hz;
        self.window.clear();
        self.publish();
        Ok(vec![])
    }
    fn process(&mut self, io: &mut Io) {
        for p in io.items::<PulsePair>(0) {
            let v = p.value;
            if self.window.len() < 8192 {
                self.window
                    .push_back((v.time, v.kind, v.spacing_us, v.width_us, v.amplitude));
            }
            self.pairs += 1;
            self.last_time = v.time;
        }
        while self
            .window
            .front()
            .is_some_and(|e| self.last_time - e.0 > 1.0)
        {
            self.window.pop_front();
        }
        for w in io.items::<MorseWord>(1) {
            self.tracker.heard(&w.value.text, w.time);
            self.wpm = Some(w.value.wpm);
            self.published
                .results_mut()
                .log(format!("{:.1} s: ident {}", w.time, w.value.text));
        }
        if io.tick() {
            self.publish();
        }
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        match DmeReportSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            DmeReportSetting::Confirm => self.tracker.set_confirm(CONFIRM.read(id, v)? as u32),
        }
        Ok(Applied::Live)
    }
    fn values(&self) -> Values {
        values_of([(
            DmeReportSetting::Confirm,
            Value::I64(i64::from(self.tracker.confirm())),
        )])
    }
    fn results(&self) -> Option<&Results> {
        Some(self.published.results())
    }
    fn commands(&self) -> &Commands {
        DecoderCommand::commands()
    }
    fn invoke(&mut self, id: CommandId, _: &Values) -> Result<Option<Record>, CommandError> {
        match DecoderCommand::from_id(id).ok_or(CommandError::NoSuchId { id })? {
            DecoderCommand::Clear => {
                self.tracker.clear();
                self.wpm = None;
                self.window.clear();
                self.pairs = 0;
                self.published = new_published();
                self.publish();
            }
        }
        Ok(None)
    }
}

/// The DME graph: pulses to pairs, pairs to the identification keyer and the Morse decoder, and pairs and words to the report.
///
/// # Errors
///
/// A [`GraphError`] if the graph cannot be built.
pub fn graph(catalog: &Arc<Catalog>) -> Result<Graph, GraphError> {
    GraphBuilder::new()
        .add("pulses", PulseDetect::default())
        .add("pairs", PulsePairs::default())
        .add("ident", DmeIdent::default())
        .add("morse", MorseDecoder::default())
        .add("report", DmeReport::default())
        .input("pulses")
        .connect("pulses", "pairs")
        .connect("pairs", "ident")
        .connect("ident", "morse")
        .connect("pairs", "report.pairs")
        .connect("morse", "report.ident")
        .output("morse")
        .clock(0.25)
        .status_from("report")
        .reconfigure_on_retune()
        .catalog(catalog)
        .build()
}

pub const STACK_MODE: Setting<field::Choice<ChannelModeChoice>> =
    Setting::choice("mode", "Channel mode", ChannelModeChoice::Auto);
pub const STACK_THRESHOLD: Setting<field::F64> =
    Setting::new("threshold", "Pulse threshold", "dB", 12.0).range(3.0, 40.0, 0.0);

sdrkit_core::choice_enum! {
    /// The transmitter of the test signal.
    pub enum Transmitter {
        /// A ground beacon: replies and identification.
        Beacon = ("beacon", "Ground beacon"),
        /// An aircraft: interrogations.
        Aircraft = ("aircraft", "Interrogating aircraft"),
    }
}

pub const GEN_MODE: Setting<field::Choice<DmeMode>> =
    Setting::choice("gen_mode", "Test signal: mode", DmeMode::X)
        .section("Test signal")
        .advanced();
pub const GEN_KIND: Setting<field::Choice<Transmitter>> =
    Setting::choice("gen_kind", "Test signal: transmitter", Transmitter::Beacon)
        .section("Test signal")
        .advanced();
pub const GEN_PPPS: Setting<field::F64> =
    Setting::new("gen_ppps", "Test signal: pairs a second", "", 2700.0)
        .range(100.0, 5000.0, 0.0)
        .section("Test signal")
        .advanced();
pub const GEN_IDENT: Setting<field::Text> =
    Setting::new("gen_ident", "Test signal: identification", "", "SY")
        .section("Test signal")
        .advanced();
pub const GEN_WPM: Setting<field::F64> =
    Setting::new("gen_wpm", "Test signal: Morse speed", "wpm", 15.0)
        .range(3.0, 25.0, 0.0)
        .section("Test signal")
        .advanced();
pub const GEN_PAUSE: Setting<field::Duration> =
    Setting::new("gen_pause", "Test signal: pause", "s", 1.5)
        .range(0.5, 30.0, 0.0)
        .section("Test signal")
        .advanced();

sdrkit_core::settings! {
    /// The settings of the DME stack.
    pub enum DmeSetting {
        /// The channel modes looked for.
        Mode = STACK_MODE,
        /// How far above the noise a pulse must rise.
        Threshold = STACK_THRESHOLD,
        /// Hearings that confirm an identification.
        Confirm = CONFIRM,
        /// The channel mode of the test signal.
        GenMode = GEN_MODE,
        /// The transmitter of the test signal.
        GenKind = GEN_KIND,
        /// The random pairs a second of the test signal.
        GenPpps = GEN_PPPS,
        /// The identification of the test signal.
        GenIdent = GEN_IDENT,
        /// The Morse speed of the test signal.
        GenWpm = GEN_WPM,
        /// The pause between identifications of the test signal.
        GenPause = GEN_PAUSE,
    }
}

fn stack_graph(cfg: &StackConfig) -> Result<StackGraph, GraphError> {
    StackGraph::of(crate::typed::shape(ID), graph(&Catalog::of(cfg)?)?)
        .min_rate(MIN_RATE_HZ, "DME needs a sample rate of at least 1 MHz")
        .route("mode", &["pairs.mode"])?
        .route("threshold", &["pulses.threshold"])?
        .route("confirm", &["report.confirm"])?
        .command("clear", sdrkit_graph::CommandTarget::every("clear"))
        .on_retune(sdrkit_graph::CommandTarget::of("report", "clear"))?
        .with_settings(&cfg.settings)
        .build()
}

/// The DME receiver.
pub fn receiver(cfg: &StackConfig) -> Result<Box<dyn Receiver>, ConfigError> {
    Ok(Box::new(stack_graph(cfg)?))
}

/// The DME test signal: a ground beacon's squitter and identification, or an aircraft's interrogations, at baseband. Each pulse
/// has a peak amplitude of 1.
pub struct DmeGenerator {
    rate: f64,
    spacing: f64,
    mean_gap: f64,
    rng: sdrkit_algo_sim::rng::Rng,
    key: IdentEnvelope,
    env: Vec<f32>,
    pulse: Vec<f32>,
    /// Sample positions of the pulses still to come (first pulses and second pulses).
    queue: VecDeque<u64>,
    next_random: u64,
    next_ident: u64,
    n: u64,
    phase: Complex32,
}

impl DmeGenerator {
    /// `mode` X or Y, a `beacon` (replies at the reply spacing, with identification) or an aircraft (interrogations), and
    /// `ppps` random pairs a second.
    ///
    /// # Errors
    ///
    /// The [`DspError`] of the pulse shape for a rate that cannot hold it.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        rate: f64,
        mode: DmeMode,
        beacon: bool,
        ppps: f64,
        ident: &str,
        wpm: f64,
        pause_s: f64,
        seed: u64,
    ) -> Result<Self, DspError> {
        let mut rng = sdrkit_algo_sim::rng::Rng::new(seed);
        let spacing = if beacon {
            mode.reply_spacing_us()
        } else {
            mode.interrogation_spacing_us()
        };
        // A Gaussian pulse 3.5 µs wide at half amplitude, in symbols of 1 µs: its standard deviation is sqrt(ln 2) / (2 pi bt).
        let sigma_us = 3.5 / (2.0 * (2.0 * std::f64::consts::LN_2).sqrt());
        let bt = std::f64::consts::LN_2.sqrt() / (TAU * sigma_us);
        let pulse = taps(
            Shape::Gaussian { bt },
            rate * 1e-6,
            8.0 * sigma_us,
            Norm::Peak,
        )?;
        let ph = rng.uniform() * TAU;
        Ok(DmeGenerator {
            rate,
            spacing,
            mean_gap: 1.0 / ppps.max(1.0),
            key: IdentEnvelope::new(
                if beacon { ident } else { "" },
                wpm,
                rate,
                0.3,
                pause_s,
                0.0,
            ),
            env: Vec::new(),
            pulse,
            queue: VecDeque::with_capacity(64),
            next_random: (rate * 1e-4) as u64,
            next_ident: 0,
            n: 0,
            phase: Complex32::new(ph.cos() as f32, ph.sin() as f32),
            rng,
        })
    }
    /// From the stack's settings.
    ///
    /// # Errors
    ///
    /// What [`DmeGenerator::new`] returns.
    pub fn from_config(cfg: &GenConfig) -> Result<Self, DspError> {
        let s = &cfg.settings;
        DmeGenerator::new(
            cfg.rate_hz,
            s.get(&GEN_MODE),
            s.get(&GEN_KIND) == Transmitter::Beacon,
            s.get(&GEN_PPPS),
            s.get(&GEN_IDENT),
            s.get(&GEN_WPM),
            s.get(&GEN_PAUSE),
            cfg.seed,
        )
    }
    fn add_pair(&mut self, at: u64) {
        let half = (self.pulse.len() / 2) as u64;
        let at = at.max(half);
        let second = at + (self.spacing * 1e-6 * self.rate).round() as u64;
        if self.queue.len() < 62 {
            self.queue.push_back(at - half);
            self.queue.push_back(second - half);
        }
    }
}

impl Generator for DmeGenerator {
    fn rate_hz(&self) -> f64 {
        self.rate
    }
    fn signal_info(&self) -> Option<SignalInfo> {
        // Two pulses for each pair: the mean power is the pair rate times the energy of two pulses. A random pair comes 60 µs
        // after the last one and an exponential gap later.
        let pulse: f64 = self.pulse.iter().map(|&v| (v as f64).powi(2)).sum::<f64>() / self.rate;
        let pairs_per_s = 1.0 / (self.mean_gap + 60e-6);
        Some(SignalInfo::new(1e6).with_power(2.0 * pulse * pairs_per_s))
    }
    fn station_offset_hz(&self, _tune_offset_hz: f64) -> f64 {
        0.0
    }
    fn generate(&mut self, out: &mut [Complex32]) -> usize {
        let ident_period = (self.rate / IDENT_PPS).round() as u64;
        let dead = (60e-6 * self.rate) as u64;
        let mut env = std::mem::take(&mut self.env);
        env.clear();
        env.extend(self.key.next(out.len()));
        for (o, &key) in out.iter_mut().zip(&env) {
            let n = self.n;
            let keyed = key > 0.5;
            if keyed {
                if n >= self.next_ident {
                    self.add_pair(n + dead);
                    self.next_ident = n + ident_period;
                    self.next_random = n + dead;
                }
            } else if n >= self.next_random {
                self.add_pair(n + dead);
                let gap = self.mean_gap * self.rng.exponential();
                self.next_random = n + dead + (gap * self.rate) as u64;
            }
            let mut v = 0.0f32;
            for &start in &self.queue {
                if n >= start && n < start + self.pulse.len() as u64 {
                    v += self.pulse[(n - start) as usize];
                }
            }
            while self
                .queue
                .front()
                .is_some_and(|&s| n >= s + self.pulse.len() as u64)
            {
                self.queue.pop_front();
            }
            *o = self.phase * v;
            self.n += 1;
        }
        self.env = env;
        out.len()
    }
}

/// Makes the DME generator.
///
/// # Errors
///
/// [`ConfigError::Stage`] if the rate cannot hold the pulse.
pub fn generator(cfg: &GenConfig) -> Result<Box<dyn Generator>, ConfigError> {
    Ok(Box::new(
        DmeGenerator::from_config(cfg).map_err(|e| ConfigError::Stage(StageError::new(e)))?,
    ))
}
