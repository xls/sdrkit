//! Globalstar terminal uplink activity monitor (`globalstar`). The CDMA content is encrypted and is never demodulated. The stack
//! measures the power, occupancy and Doppler of the 13 channels of 1.23 MHz.

pub mod gen;

use sdrkit_algo_dsp::meter::power_to_db as db;
use crate::display::{set_points, Clock};
use crate::typed::{
    GlobalstarActivityEvent, GlobalstarActivityRow, GlobalstarChannelRow, GlobalstarFields,
    GLOBALSTAR_ACTIVITY, GLOBALSTAR_ACTIVITY_EVENT, GLOBALSTAR_CHANNELS, GLOBALSTAR_DOPPLER,
};
use sdrkit_algo_dsp::floor::{BinMedian, DEFAULT_MIN_FLOOR};
use sdrkit_algo_dsp::spectrum::{Scaling, SpectrumAnalyzer, SpectrumConfig};
use sdrkit_algo_dsp::window::Window;
use sdrkit_core::interface::{Fields as _, Hertz, Seconds, Time};
use sdrkit_core::publish::keys::Family;
use sdrkit_core::publish::{field, Published};
use sdrkit_core::verify::{CaseSetting, CheckLine, VerifyCase, VerifyHooks, VerifyOptions};
use sdrkit_core::SignalType;
use sdrkit_core::{
    AutoGain, Complex32, ConfigError, GenConfig, Generator, LockProfile, LockState, Receiver,
    ResultsDecl, Schema, SeriesId, Setting, SettingError, SettingId, SettingSet, StackConfig,
    StackDescriptor, StateBlob, StateError, Tuning, Value, Values,
};
use sdrkit_graph::{
    no_values, values_of, Catalog, Decoder, DecoderComponent, Format, GraphBuilder, GraphError,
    Item, Kind, Outbox, Port, RateFrom, Registry, RunContext, StackGraph, Timed,
};
use sdrkit_util_objects::commands::Tables;
use sdrkit_util_objects::shaped::{self, RowOut, Shaped};
use sdrkit_util_objects::{LogRing, Seen, Shift};
use serde::{Deserialize, Serialize};
use std::fmt::Write;
use std::sync::{Arc, OnceLock};

/// The stack id.
pub const ID: &str = "globalstar";
/// The stack name.
pub const NAME: &str = "Globalstar";
/// The lowest sample rate, Hz.
pub const MIN_RATE: f64 = 3e6;
/// Seconds between reports.
pub const REPORT_S: f64 = 0.25;
/// The slot over which FFT frames are averaged.
pub const SLOT_S: f64 = 0.01;
/// Channel 1 centre.
pub const CH1_HZ: f64 = 1611.25e6;
pub const CH_STEP_HZ: f64 = 1.23e6;
pub const CHANNELS: usize = 13;
/// The band centre, also assumed when the input has no frequency.
pub const BAND_CENTRE_HZ: f64 = 1618.25e6;
pub const POWER_HALF_HZ: f64 = 615e3;
/// The power is summed over the channel less a Doppler margin, so a strong neighbour moved by its Doppler does not spill into it.
pub const POWER_SUM_HALF_HZ: f64 = 570e3;
pub const EVENTS_CAP: usize = 200;
pub const TRACK_CAP: usize = 2400;
/// FFT frames averaged in each slot.
pub const FRAMES_PER_SLOT: usize = 16;
/// How far around a channel's centre its edges are looked for.
pub const EDGE_SPAN_HZ: f64 = 700e3;
/// The smoothing of the excess profile per slot.
pub const PROFILE_ALPHA: f64 = 0.3;
/// Slots that set the noise floor at the start.
pub const WARMUP_SLOTS: u64 = 20;
/// A channel turns on after 3 slots in a row whose power stands this far over its floor, dB.
pub const GATE_ON_DB: f64 = 0.8;
/// A channel turns off after 10 slots in a row whose power stands less than this over its floor, dB.
pub const GATE_OFF_DB: f64 = 0.4;

/// The centre of channel `n` (1 to 13).
pub fn channel_hz(n: usize) -> f64 {
    CH1_HZ + (n as f64 - 1.0) * CH_STEP_HZ
}

/// The channel nearest a frequency.
pub fn nearest_channel(f: f64) -> usize {
    (((f - CH1_HZ) / CH_STEP_HZ).round() as i64 + 1).clamp(1, CHANNELS as i64) as usize
}

/// The FFT size for a rate: about 10 kHz bins, 256 to 4096 points.
pub fn fft_size(rate: f64) -> usize {
    ((rate / 10e3).round() as usize)
        .next_power_of_two()
        .clamp(256, 4096)
}

/// One channel's measurement in a slot.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ChannelSlot {
    pub channel: usize,
    pub in_view: bool,
    /// Power in the channel, dBFS.
    pub power_dbfs: f64,
    /// Noise floor in the channel, dBFS.
    pub floor_dbfs: f64,
    /// Power above the floor, dB (0 when off).
    pub snr_db: f64,
    pub active: bool,
    pub doppler_hz: f64,
}

/// All channels in one slot.
#[derive(Clone, Debug, Default)]
pub struct ChannelsSlot {
    pub channels: [ChannelSlot; CHANNELS],
    /// The noise floor is set (the warm-up is over).
    pub warm: bool,
}
impl Item for ChannelsSlot {
    const KIND: &'static str = "satcom.globalstar_channels";
}

#[derive(Clone, Debug, Default)]
struct Track {
    floor: f64,
    on: bool,
    above: u32,
    below: u32,
    profile: Vec<f64>,
    fresh: bool,
    doppler: f64,
}

/// The Doppler of a 1.23 MHz wide signal from its excess power profile (bin 0 at `f0` Hz from the channel centre): the two edges
/// where the profile crosses half its plateau, each 615 kHz from the signal's centre. An edge that a neighbouring signal hides (no
/// fall) is left out. `None` when no edge is found.
pub fn edge_doppler(profile: &[f64], f0: f64, bin_hz: f64) -> Option<f64> {
    let n = profile.len();
    if n == 0 {
        return None;
    }
    let f = |i: usize| f0 + i as f64 * bin_hz;
    let sm = |i: usize| {
        let a = profile[i.saturating_sub(1)];
        let c = profile[(i + 1).min(n - 1)];
        (a + 2.0 * profile[i] + c) / 4.0
    };
    // The plateau: the mean over the middle +-450 kHz.
    let (mut sum, mut count) = (0.0, 0usize);
    for (i, v) in profile.iter().enumerate() {
        if f(i).abs() <= 450e3 {
            sum += v;
            count += 1;
        }
    }
    if count == 0 {
        return None;
    }
    let plateau = sum / count as f64;
    if plateau <= 0.0 {
        return None;
    }
    let h = plateau / 2.0;
    let centre = (0..n).min_by(|&a, &b| f(a).abs().total_cmp(&f(b).abs()))?;
    let mut left = None;
    let mut i = centre;
    while i > 0 {
        let (hi, lo) = (sm(i), sm(i - 1));
        if f(i - 1) < -EDGE_SPAN_HZ + bin_hz {
            break;
        }
        if hi >= h && lo < h {
            left = Some(f(i) - (hi - h) / (hi - lo) * bin_hz);
            break;
        }
        i -= 1;
    }
    let mut right = None;
    let mut i = centre;
    while i + 1 < n {
        let (lo, hi) = (sm(i), sm(i + 1));
        if f(i + 1) > EDGE_SPAN_HZ - bin_hz {
            break;
        }
        if lo >= h && hi < h {
            right = Some(f(i) + (lo - h) / (lo - hi) * bin_hz);
            break;
        }
        i += 1;
    }
    // An edge further than 100 kHz from its place belongs to something else.
    let left = left.filter(|l: &f64| (l + POWER_HALF_HZ).abs() < 100e3);
    let right = right.filter(|r: &f64| (r - POWER_HALF_HZ).abs() < 100e3);
    match (left, right) {
        (Some(l), Some(r)) => Some((l + r) / 2.0),
        (Some(l), None) => Some(l + POWER_HALF_HZ),
        (None, Some(r)) => Some(r - POWER_HALF_HZ),
        _ => None,
    }
}

/// The channel measurer: windowed FFT frames of the whole band averaged into 10 ms slots, then for each channel the power in
/// +-570 kHz, the noise floor tracker, on and off with hysteresis, and the Doppler from the signal's edges.
pub struct ChannelMeter {
    rate: f64,
    centre: f64,
    spectrum: Option<SpectrumAnalyzer>,
    floor: Option<BinMedian>,
    acc: Vec<f32>,
    acc_frames: usize,
    frames: u64,
    tracks: Vec<Track>,
    slots: u64,
    warm_sum: f64,
    warm_n: u64,
}

impl Default for ChannelMeter {
    fn default() -> Self {
        ChannelMeter {
            rate: 0.0,
            centre: BAND_CENTRE_HZ,
            spectrum: None,
            floor: None,
            acc: Vec::new(),
            acc_frames: 0,
            frames: 0,
            tracks: vec![Track::default(); CHANNELS],
            slots: 0,
            warm_sum: 0.0,
            warm_n: 0,
        }
    }
}

impl ChannelMeter {
    fn centre_of(f: &Format) -> f64 {
        if f.centre_hz > 0.0 && f.centre_hz.is_finite() {
            f.centre_hz
        } else {
            BAND_CENTRE_HZ
        }
    }

    /// The averaged slot is complete: measure every channel.
    fn measure(&mut self, time: f64, out: &mut Vec<Timed<ChannelsSlot>>) {
        if self.acc_frames == 0 {
            return;
        }
        let k = 1.0 / self.acc_frames as f32;
        self.acc.iter_mut().for_each(|v| *v *= k);
        let n = self.acc.len();
        let bin_hz = self.rate / n as f64;
        let half = self.rate / 2.0;
        // The mean noise per bin of the band, from the median of the usable bins.
        let lo = ((0.05 * n as f64) as usize).max(1);
        let floor = self.floor.as_mut().expect("configured");
        let med = floor
            .estimate(self.acc[lo..n - lo].iter().copied())
            .unwrap_or(0.0) as f64;
        self.slots += 1;
        let warm = self.slots > WARMUP_SLOTS;
        if !warm {
            self.warm_sum += med;
            self.warm_n += 1;
        }
        let mut slot = ChannelsSlot {
            warm,
            ..Default::default()
        };
        let bins = &self.acc;
        for ch in 1..=CHANNELS {
            let off = channel_hz(ch) - self.centre;
            let in_view = off.abs() + POWER_HALF_HZ <= half;
            let cs = &mut slot.channels[ch - 1];
            cs.channel = ch;
            cs.in_view = in_view;
            let tr = &mut self.tracks[ch - 1];
            if !in_view {
                continue;
            }
            let bin_of = |f: f64| ((f + half) / bin_hz).round() as i64;
            let (p0, p1) = (
                bin_of(off - POWER_SUM_HALF_HZ),
                bin_of(off + POWER_SUM_HALF_HZ),
            );
            let mut p = 0.0f64;
            let mut count = 0usize;
            for b in p0.max(0)..=p1.min(n as i64 - 1) {
                p += bins[b as usize] as f64;
                count += 1;
            }
            let p = p / count.max(1) as f64;
            if !warm {
                tr.floor = self.warm_sum / self.warm_n as f64;
            } else if p < tr.floor {
                tr.floor += 0.3 * (p - tr.floor);
            } else if !tr.on && p < 1.6 * tr.floor {
                tr.floor += 0.01 * (p - tr.floor);
            }
            tr.floor = tr.floor.max(f64::from(DEFAULT_MIN_FLOOR));
            let ratio = p / tr.floor - 1.0;
            let snr = if ratio > 0.0 { db(ratio) } else { -30.0 };
            // The gate reads the total power over the floor.
            let over = db(p / tr.floor);
            if warm {
                if over > GATE_ON_DB {
                    tr.above += 1;
                    tr.below = 0;
                } else if over < GATE_OFF_DB {
                    tr.below += 1;
                    tr.above = 0;
                } else {
                    tr.above = 0;
                    tr.below = 0;
                }
                if !tr.on && tr.above >= 3 {
                    tr.on = true;
                }
                if tr.on && tr.below >= 10 {
                    tr.on = false;
                }
            }
            cs.power_dbfs = db(p * count as f64);
            cs.floor_dbfs = db(tr.floor * count as f64);
            cs.snr_db = if tr.on { snr } else { 0.0 };
            cs.active = tr.on;
            if tr.on {
                // The channel's excess profile over +-EDGE_SPAN, smoothed over time while the channel is on.
                let nb = (2.0 * EDGE_SPAN_HZ / bin_hz).ceil() as usize | 1;
                if tr.profile.len() != nb {
                    tr.profile = vec![0.0; nb];
                    tr.fresh = true;
                }
                let b0 = bin_of(off) - (nb as i64) / 2;
                let a = if tr.fresh { 1.0 } else { PROFILE_ALPHA };
                tr.fresh = false;
                for (j, pv) in tr.profile.iter_mut().enumerate() {
                    let b = b0 + j as i64;
                    let w = if (0..n as i64).contains(&b) {
                        bins[b as usize] as f64 - tr.floor
                    } else {
                        0.0
                    };
                    *pv += a * (w - *pv);
                }
                let f0 = b0 as f64 * bin_hz - half - off;
                if let Some(d) = edge_doppler(&tr.profile, f0, bin_hz) {
                    tr.doppler = d;
                }
                cs.doppler_hz = tr.doppler;
            } else {
                tr.fresh = true;
            }
        }
        out.push(Timed { time, value: slot });
        self.acc.iter_mut().for_each(|v| *v = 0.0);
        self.acc_frames = 0;
    }
}

impl Decoder for ChannelMeter {
    type Input = Complex32;
    fn name(&self) -> &'static str {
        "satcom.globalstar_channels"
    }
    fn outputs(&self) -> Vec<Port> {
        vec![Port::typed::<ChannelsSlot>("out").rate_from(RateFrom::Fixed(1.0 / SLOT_S))]
    }
    fn values(&self) -> Values {
        no_values()
    }
    fn configure(&mut self, f: &Format) -> Result<Vec<Format>, ConfigError> {
        if f.rate.is_nan() || f.rate < MIN_RATE {
            return Err(ConfigError::RateTooLow {
                need: MIN_RATE,
                got: f.rate,
            });
        }
        let n = fft_size(f.rate);
        let mut cfg = SpectrumConfig::new(n, f.rate);
        cfg.window = Window::Hann;
        cfg.scaling = Scaling::Power;
        cfg.frames_per_second = FRAMES_PER_SLOT as f64 / SLOT_S;
        let spectrum = SpectrumAnalyzer::new(cfg)?;
        let mut floor = BinMedian::new(FRAMES_PER_SLOT as u32)?;
        floor.reserve(n);
        *self = ChannelMeter {
            rate: f.rate,
            centre: Self::centre_of(f),
            spectrum: Some(spectrum),
            floor: Some(floor),
            acc: vec![0.0; n],
            ..Default::default()
        };
        // One item a slot.
        Ok(vec![Format::new(
            Kind::typed::<ChannelsSlot>(),
            1.0 / SLOT_S,
        )])
    }
    fn retune(&mut self, f: &Format) {
        self.centre = Self::centre_of(f);
        self.acc.iter_mut().for_each(|v| *v = 0.0);
        self.acc_frames = 0;
        if let Some(s) = &mut self.spectrum {
            s.drop_partial();
        }
    }
    fn process(&mut self, x: &[Complex32], _first: u64, out: &mut Outbox) {
        let Some(hop) = self.spectrum.as_ref().map(|s| s.hop()) else {
            return;
        };
        let piece = hop.floor().max(1.0) as usize;
        let out = out.items::<ChannelsSlot>(0);
        let mut i = 0usize;
        while i < x.len() {
            // A piece of at most one hop completes at most one frame.
            let take = piece.min(x.len() - i);
            let s = self.spectrum.as_mut().expect("configured");
            let made = s.push(&x[i..i + take]);
            i += take;
            if made > 0 {
                for (a, p) in self.acc.iter_mut().zip(s.power()) {
                    *a += *p;
                }
                self.acc_frames += 1;
                self.frames += 1;
                if self.acc_frames == FRAMES_PER_SLOT {
                    // The end of the slot's last frame, counted in frames so that any cut of the input produces the same time.
                    let t = (self.frames as f64 * hop + self.acc.len() as f64) / self.rate;
                    self.measure(t, out);
                }
            }
        }
    }
}

// The results.

/// What the display publishes. The Doppler series come first, channel 1 first.
fn display_decl() -> Arc<ResultsDecl> {
    static DECL: OnceLock<Arc<ResultsDecl>> = OnceLock::new();
    DECL.get_or_init(|| {
        let mut d = ResultsDecl::new();
        for s in GLOBALSTAR_DOPPLER {
            d = d.series(s.key, s.label, s.x_unit, s.y_unit);
        }
        Arc::new(
            d.with_fields(GlobalstarFields::SHAPE.fields)
                .with_table(&GLOBALSTAR_CHANNELS)
                .with_table(&GLOBALSTAR_ACTIVITY),
        )
    })
    .clone()
}

/// What the stack publishes.
pub fn declaration() -> ResultsDecl {
    crate::typed::shape(ID).results_decl()
}

/// One stretch of activity.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ActivityEvent {
    /// Signal time of the start.
    pub start: f64,
    pub duration: f64,
    /// Wall-clock time of the start, when live.
    pub wall: Option<f64>,
    pub channel: usize,
    pub peak_snr_db: f64,
    pub doppler_start_hz: f64,
    pub doppler_end_hz: f64,
}

impl Shift for ActivityEvent {
    fn shift_times(&mut self, by_s: f64) {
        self.start += by_s;
    }
}

impl Shaped for ActivityEvent {
    type Row = GlobalstarActivityRow;
    fn row(&self, _seen: Option<&Seen>, _now_s: f64, out: RowOut<'_, '_, Self::Row>) {
        out.put(&GlobalstarActivityRow {
            start: Seconds(self.start),
            duration: Seconds(self.duration),
            channel: self.channel as i64,
            peak_snr_db: self.peak_snr_db,
            doppler_start: Hertz(self.doppler_start_hz),
            doppler_end: Hertz(self.doppler_end_hz),
            wall_start: self.wall.map(Time),
        });
    }
}

/// One point of the Doppler track.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct TrackPoint {
    pub time: f64,
    pub channel: usize,
    pub doppler_hz: f64,
    pub snr_db: f64,
}

impl Shift for TrackPoint {
    fn shift_times(&mut self, by_s: f64) {
        self.time += by_s;
    }
}

/// A channel row.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ChannelRow {
    /// The channel number, 1 to 13.
    pub index: usize,
    /// The channel lies inside the input.
    pub in_view: bool,
    /// The channel is active now.
    pub active: bool,
    /// Its power, dBFS.
    pub power_dbfs: f64,
    /// Its noise floor, dBFS.
    pub floor_dbfs: f64,
    /// Its SNR, dB.
    pub snr_db: f64,
    /// The share of time the channel was on, 0 to 1 (60 s average).
    pub occupancy: f64,
    /// Its Doppler shift, Hz.
    pub doppler_hz: f64,
    /// Signal time it was last active.
    pub last_active: Option<f64>,
    /// Stretches of activity on it.
    pub events: u64,
}

impl Shift for ChannelRow {
    fn shift_times(&mut self, by_s: f64) {
        if let Some(t) = &mut self.last_active {
            *t += by_s;
        }
    }
}

impl ChannelRow {
    /// The row of the channel table.
    fn shaped(&self) -> GlobalstarChannelRow {
        GlobalstarChannelRow {
            channel: self.index as i64,
            centre: Hertz(channel_hz(self.index)),
            in_view: self.in_view,
            active: self.active,
            power_dbfs: self.power_dbfs,
            floor_dbfs: self.floor_dbfs,
            snr_db: self.snr_db,
            occupancy_pct: 100.0 * self.occupancy,
            doppler: Hertz(self.doppler_hz),
            last_active: self.last_active.map(Seconds),
            events: self.events as i64,
        }
    }
}

/// What the display keeps (saved state).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GsTables {
    /// The channel rows.
    pub rows: Vec<ChannelRow>,
    /// The stretches of activity.
    pub events: LogRing<ActivityEvent>,
    /// The Doppler track.
    pub track: LogRing<TrackPoint>,
    /// Stretches of activity completed.
    pub completed: u64,
}

impl Default for GsTables {
    fn default() -> Self {
        GsTables {
            rows: (1..=CHANNELS)
                .map(|i| ChannelRow {
                    index: i,
                    ..Default::default()
                })
                .collect(),
            events: LogRing::new(EVENTS_CAP),
            track: LogRing::new(TRACK_CAP),
            completed: 0,
        }
    }
}

impl Shift for GsTables {
    fn shift_times(&mut self, by_s: f64) {
        self.rows.iter_mut().for_each(|r| r.shift_times(by_s));
        self.events.shift_times(by_s);
        self.track.shift_times(by_s);
    }
}

impl Tables for GsTables {
    const FORMAT: sdrkit_util_objects::Format =
        sdrkit_util_objects::Format::new("satcom.globalstar", 1);
    fn clear(&mut self) {
        self.events.clear();
        self.track.clear();
        self.completed = 0;
        for r in &mut self.rows {
            r.events = 0;
            r.occupancy = 0.0;
            r.last_active = None;
        }
    }
    fn adopt(&mut self, saved: Self) {
        if saved.rows.len() == CHANNELS {
            self.rows = saved.rows;
        }
        self.events.adopt(saved.events);
        self.track.adopt(saved.track);
        self.completed = saved.completed;
    }
}

#[derive(Clone, Copy, Debug, Default)]
struct Run {
    start: f64,
    peak: f64,
    d0: f64,
    d1: f64,
    // Averages since the last report.
    sum_d: f64,
    sum_snr: f64,
    n: u64,
}

/// The display: channel rows, stretches of activity, the Doppler track and the status.
pub struct GsDisplay {
    clock: Clock,
    live: bool,
    host_clock: sdrkit_core::Clock,
    tables: GsTables,
    runs: [Option<Run>; CHANNELS],
    last_any: Option<f64>,
    window_s: f64,
    pass_overlay: bool,
    /// The activity log changed since the last report.
    dirty: bool,
}

impl Default for GsDisplay {
    fn default() -> Self {
        GsDisplay {
            clock: Clock::default(),
            live: false,
            host_clock: sdrkit_core::Clock::fixed(0.0),
            tables: GsTables::default(),
            runs: [None; CHANNELS],
            last_any: None,
            window_s: 300.0,
            pass_overlay: false,
            dirty: true,
        }
    }
}

/// The span of the Doppler plot.
pub const WINDOW_S: Setting<field::Duration> =
    Setting::new("window_s", "Doppler plot span", "s", 300.0).range(60.0, 600.0, 10.0);
/// Draw the pass planner's predicted Doppler over the track.
pub const PASS_OVERLAY: Setting<field::Bool> =
    Setting::new("pass_overlay", "Pass overlay", "", false)
        .help("Draw the pass planner's predicted Doppler over the track.");

sdrkit_core::settings! {
    /// The settings of [`GsDisplay`].
    pub enum GsDisplaySetting {
        /// Doppler plot span.
        WindowS = WINDOW_S,
        /// Pass overlay.
        PassOverlay = PASS_OVERLAY,
    }
}

sdrkit_core::settings! {
    /// The settings of the Globalstar stack. Noise and other impairments of the test signal come from the standard `gen_*`
    /// settings.
    pub enum GlobalstarSetting {
        /// Doppler plot span.
        WindowS = WINDOW_S,
        /// Pass overlay.
        PassOverlay = PASS_OVERLAY,
        /// Scenario of the test signal.
        GenScenario = gen::GEN_SCENARIO,
        /// Channel of the test signal.
        GenChannel = gen::GEN_CHANNEL,
        /// Time scale of the test signal.
        GenTimeScale = gen::GEN_TIME_SCALE,
        /// Radio centre of the test signal.
        GenCentreHz = gen::GEN_CENTRE_HZ,
    }
}

impl GsDisplay {
    /// A stretch of activity on `ch` ended at signal time `t`: keep it if it lasted 50 ms, and return it.
    fn end_run(&mut self, ch: usize, t: f64) -> Option<ActivityEvent> {
        let r = self.runs[ch - 1].take()?;
        let duration = t - r.start;
        if duration < 0.05 {
            return None;
        }
        let e = ActivityEvent {
            start: r.start,
            duration,
            wall: self.clock.wall(r.start),
            channel: ch,
            peak_snr_db: r.peak,
            doppler_start_hz: r.d0,
            doppler_end_hz: r.d1,
        };
        self.dirty = true;
        self.tables.events.push(e.clone());
        self.tables.rows[ch - 1].events += 1;
        self.tables.completed += 1;
        Some(e)
    }

    fn item(&mut self, it: &Timed<ChannelsSlot>, events: &mut Vec<sdrkit_core::Event>) {
        let t = self.clock.at(it.time);
        let k = SLOT_S / 60.0;
        for cs in &it.value.channels {
            if cs.channel == 0 {
                continue;
            }
            let i = cs.channel - 1;
            let row = &mut self.tables.rows[i];
            row.in_view = cs.in_view;
            row.power_dbfs = cs.power_dbfs;
            row.floor_dbfs = cs.floor_dbfs;
            row.snr_db = cs.snr_db;
            if cs.in_view {
                row.occupancy += ((cs.active as u8 as f64) - row.occupancy) * k;
            }
            let was = row.active;
            row.active = cs.active;
            if cs.active {
                row.doppler_hz = cs.doppler_hz;
                row.last_active = Some(t);
                self.last_any = Some(t);
                let r = self.runs[i].get_or_insert(Run {
                    start: t - SLOT_S * 3.0,
                    peak: cs.snr_db,
                    d0: cs.doppler_hz,
                    ..Default::default()
                });
                r.peak = r.peak.max(cs.snr_db);
                r.d1 = cs.doppler_hz;
                r.sum_d += cs.doppler_hz;
                r.sum_snr += cs.snr_db;
                r.n += 1;
            } else if was {
                // The off decision came 10 slots after the activity ended.
                if let Some(e) = self.end_run(cs.channel, t - SLOT_S * 10.0) {
                    events.push(GLOBALSTAR_ACTIVITY_EVENT.event(
                        e.start,
                        channel_hz(e.channel) + e.doppler_start_hz,
                        &GlobalstarActivityEvent {
                            source: &format!("channel {}", e.channel),
                            text: &format!(
                                "{:.2} s of activity, Doppler {:+.0} Hz",
                                e.duration, e.doppler_start_hz
                            ),
                            snr_db: Some(e.peak_snr_db).filter(|s| s.is_finite()),
                            family: Family::Satellite,
                            channel: e.channel as i64,
                            duration: Seconds(e.duration),
                        },
                    ));
                }
            }
        }
    }

    /// Adds a track point for each active channel (the average since the last report).
    fn add_track_points(&mut self) {
        let now = self.clock.now();
        for ch in 1..=CHANNELS {
            if let Some(r) = &mut self.runs[ch - 1] {
                if r.n > 0 {
                    self.tables.track.push(TrackPoint {
                        time: now,
                        channel: ch,
                        doppler_hz: r.sum_d / r.n as f64,
                        snr_db: r.sum_snr / r.n as f64,
                    });
                    r.sum_d = 0.0;
                    r.sum_snr = 0.0;
                    r.n = 0;
                }
            }
        }
    }

    fn publish(&mut self, p: &mut Published) {
        let now = self.clock.now();
        let rows = &self.tables.rows;
        let active = rows.iter().filter(|r| r.active).count();
        let state = if active > 0 {
            2
        } else if self.last_any.is_some_and(|t| now - t < 30.0) {
            1
        } else {
            0
        };
        let doppler = (active > 0).then(|| {
            rows.iter()
                .filter(|r| r.active)
                .map(|r| r.doppler_hz)
                .sum::<f64>()
                / active as f64
        });
        let snr = rows
            .iter()
            .filter(|r| r.active)
            .map(|r| r.snr_db)
            .fold(None, |a: Option<f64>, b| Some(a.map_or(b, |a| a.max(b))));
        let lock_state = LockState::from_level(state);
        let snr32 = snr.map(|v| v as f32);
        p.results_mut().update_status(|st| {
            let mut summary = String::new();
            let _ = match (doppler, snr) {
                (Some(d), Some(s)) => write!(
                    summary,
                    "{} channel{} active, Doppler {:+.1} kHz, SNR {:.0} dB",
                    active,
                    if active == 1 { "" } else { "s" },
                    d / 1e3,
                    s
                ),
                _ if state == 1 => write!(summary, "activity seen lately, idle now"),
                _ => write!(summary, "listening: no Globalstar terminal uplink"),
            };
            let same = st.lock_state == lock_state
                && st.offset_hz == doppler
                && st.snr_db == snr32
                && st.summary == summary;
            if !same {
                st.lock_state = lock_state;
                st.offset_hz = doppler;
                st.snr_db = snr32;
                st.summary = summary;
            }
            !same
        });
        p.put(&GlobalstarFields {
            active_channels: active as i64,
            activities: self.tables.completed as i64,
            doppler: doppler.map(Hertz),
        });
        let mut key = String::new();
        for row in rows {
            key.clear();
            let _ = write!(key, "{:02}", row.index);
            GLOBALSTAR_CHANNELS.put(p, &key, &row.shaped());
        }
        if std::mem::take(&mut self.dirty) {
            shaped::log_rows(&self.tables.events, &GLOBALSTAR_ACTIVITY, now, p);
        }
        let lo = now - self.window_s;
        let r = p.results_mut();
        for ch in 1..=CHANNELS {
            let points = || {
                self.tables
                    .track
                    .iter()
                    .filter(move |t| t.channel == ch && t.time >= lo)
                    .map(|t| [t.time as f32, t.doppler_hz as f32])
            };
            set_points(r, SeriesId((ch - 1) as u16), false, points);
        }
    }
}

impl Decoder for GsDisplay {
    type Input = Timed<ChannelsSlot>;
    fn name(&self) -> &'static str {
        "satcom.globalstar_display"
    }
    fn outputs(&self) -> Vec<Port> {
        vec![Port::new("events", Kind::Messages).rate_from(RateFrom::Fixed(0.0))]
    }
    fn settings(&self) -> Arc<Schema> {
        GsDisplaySetting::schema()
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<(), SettingError> {
        match GsDisplaySetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            GsDisplaySetting::WindowS => self.window_s = WINDOW_S.read(id, v)?,
            GsDisplaySetting::PassOverlay => self.pass_overlay = PASS_OVERLAY.read(id, v)?,
        }
        Ok(())
    }
    fn values(&self) -> Values {
        values_of([
            (GsDisplaySetting::WindowS, Value::Duration(self.window_s)),
            (
                GsDisplaySetting::PassOverlay,
                Value::Bool(self.pass_overlay),
            ),
        ])
    }
    fn results(&self) -> Published {
        Published::from_decl(display_decl())
    }
    fn attach(&mut self, ctx: &RunContext) {
        self.live = ctx.live;
        self.host_clock = ctx.clock.clone();
    }
    fn configure(&mut self, _f: &Format) -> Result<Vec<Format>, ConfigError> {
        // A restart drops the stretches in progress. The tables stay.
        self.clock.restart(self.live, self.host_clock.now());
        self.runs = [None; CHANNELS];
        Ok(vec![Format::new(Kind::Messages, 0.0)])
    }
    fn retune(&mut self, _f: &Format) {
        self.runs = [None; CHANNELS];
    }
    fn process(&mut self, x: &[Timed<ChannelsSlot>], _first: u64, out: &mut Outbox) {
        let events = out.messages(0);
        for it in x {
            self.item(it, events);
        }
    }
    fn tick(&mut self, _out: &mut Outbox) {
        self.add_track_points();
    }
    fn report(&mut self, results: &mut Published) {
        self.publish(results);
    }
    fn report_interval_s(&self) -> f64 {
        REPORT_S
    }
    fn clear(&mut self) {
        self.tables.clear();
        self.runs = [None; CHANNELS];
        self.last_any = None;
        self.dirty = true;
    }
    fn save_state(&self) -> Option<StateBlob> {
        self.tables.save_state(self.clock.now()).ok()
    }
    fn restore_state(&mut self, state: &StateBlob) -> Result<(), StateError> {
        self.tables.restore_state(state, self.clock.now())?;
        self.dirty = true;
        Ok(())
    }
}

/// Adds the stack's components to `r`.
pub fn register(r: &mut Registry) {
    r.add("satcom.globalstar_channels", || {
        Box::new(DecoderComponent::new(ChannelMeter::default()))
    });
    r.add("satcom.globalstar_display", || {
        Box::new(DecoderComponent::new(GsDisplay::default()))
    });
}

/// The receiver.
///
/// # Errors
///
/// A [`GraphError`] if a setting of `cfg` is refused.
pub fn receiver(cfg: &StackConfig) -> Result<StackGraph, GraphError> {
    let g = GraphBuilder::new()
        .add("channels", DecoderComponent::new(ChannelMeter::default()))
        .add("display", DecoderComponent::new(GsDisplay::default()))
        .input("channels.in")
        .connect("channels.out", "display.in")
        .output("display.events")
        .status_from("display")
        .clock(REPORT_S)
        .catalog(&Catalog::of(cfg)?)
        .build()?;
    StackGraph::of(crate::typed::shape(ID), g)
        .min_rate(
            MIN_RATE,
            "Globalstar needs a sample rate of at least 3000 ksps",
        )
        .default_centre(BAND_CENTRE_HZ)
        .route("window_s", &["display.window_s"])?
        .route("pass_overlay", &["display.pass_overlay"])?
        .command("clear", sdrkit_graph::CommandTarget::every("clear"))
        .with_settings(&cfg.settings)
        .build()
}

fn make_receiver(cfg: &StackConfig) -> Result<Box<dyn Receiver>, ConfigError> {
    crate::stack_receiver(receiver(cfg))
}

fn make_generator(cfg: &GenConfig) -> Result<Box<dyn Generator>, ConfigError> {
    Ok(Box::new(gen::GsGenerator::from_config(cfg)))
}

/// The verification of the stack: a steady terminal at 10 dB must be found active.
pub static HOOKS: VerifyHooks = VerifyHooks {
    cases: verify_cases,
    vector_settings: None,
    check_vector: None,
};

/// The level of the test signal ahead of an 8-bit converter, dBFS: the CDMA signal has a high crest factor, so it needs
/// this headroom below the full scale. At 0 dBFS the converter clips it, and the clipped spectrum fills the next channels.
/// The verify tool feeds the signal unquantised, so its case leaves the level at the default.
pub const CONVERTER_LEVEL_DBFS: f64 = -12.0;

/// The case of the verify tool. Its settings are settings of the stack: the level and the other standard `gen_*`
/// impairments belong to the test radio, which the tool sets itself.
fn verify_cases(o: &VerifyOptions) -> Vec<VerifyCase> {
    let mut c = VerifyCase::new("test signal, a steady terminal", o, 0.0);
    c.settings
        .push(CaseSetting::new(&gen::GEN_SCENARIO, gen::Scenario::Steady));
    c.snr_db = Some(if o.clean { 30.0 } else { o.snr_db.max(6.0) });
    c.seconds = o.seconds.max(1.5);
    c.expect_audio = Some(false);
    c.check = Some(check_active);
    vec![c]
}

fn check_active(r: &sdrkit_core::Results) -> Vec<CheckLine> {
    let n = crate::field(r, "active_channels") as i64;
    vec![CheckLine::new(
        "active_channels",
        n == 1,
        format!("{n} channels active (want 1)"),
    )]
}

/// The descriptor of the stack.
pub static DESCRIPTOR: StackDescriptor = StackDescriptor {
    id: ID,
    name: NAME,
    group: "Satellite",
    blurb: "Activity of the Globalstar terminal uplink: occupancy, power and Doppler per CDMA channel. Nothing is decoded. Receive only.",
    tuning: Tuning {
        min_hz: 1600e6,
        max_hz: 1640e6,
        default_hz: BAND_CENTRE_HZ,
        rate_hz: 10e6,
        min_rate_hz: MIN_RATE,
        bandwidth_hz: 10e6,
        tune_offset_hz: 0.0,
        carrier_at_centre: false,
        not_circular: false,
        radio_filter_hz: None,
        no_offset_tuning: true,
    },
    submodes: &[],
    settings: GlobalstarSetting::schema,
    make_receiver,
    make_generator: Some(make_generator),
    lock_profile: LockProfile::DEFAULT,
    signal_types: &[SignalType::Globalstar],
    auto_gain: AutoGain::Radio,
    verify: Some(&HOOKS),
    benchmarks: None,
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn channel_plan() {
        assert_eq!(channel_hz(1), 1611.25e6);
        assert!((channel_hz(13) - (1611.25e6 + 12.0 * 1.23e6)).abs() < 1.0);
        assert_eq!(nearest_channel(BAND_CENTRE_HZ), 7);
        assert_eq!(fft_size(10e6), 1024);
    }

    #[test]
    fn edge_doppler_of_a_shifted_plateau() {
        let bin = 10e3;
        let f0 = -EDGE_SPAN_HZ;
        let n = (2.0 * EDGE_SPAN_HZ / bin) as usize + 1;
        let profile: Vec<f64> = (0..n)
            .map(|i| {
                let f = f0 + i as f64 * bin;
                if (f - 20e3).abs() < POWER_HALF_HZ {
                    1.0
                } else {
                    0.0
                }
            })
            .collect();
        let d = edge_doppler(&profile, f0, bin).unwrap();
        assert!((d - 20e3).abs() < bin, "{d}");
        assert_eq!(edge_doppler(&[], 0.0, bin), None);
    }
}
