//! RC model PPM (the submode `ppm72` of `rc_links`): the pulse train of an AM-keyed radio-control transmitter of 26 to 75 MHz.
//! Each channel is the time from the start of one pulse to the start of the next; a long gap (the sync) ends the frame. The
//! pulse is the carrier state that lasts the shorter time, so a notch in a steady carrier and a short burst of carrier both
//! read. No standard or maker publishes this frame; the stack measures what it sees.
//!
//! The reader mixes out the dial offset, decimates to about 160 kHz and slices the envelope with the shared tracking slicer.

use crate::typed::{PpmPulseState, RcLinksPpmFrameRow, RcPpmFrameEvent, RC_PPM_FRAME};
use sdrkit_algo_dsp::decim::FirDecimator;
use sdrkit_algo_dsp::nco::Nco;
use sdrkit_algo_modem::envtrack::{EdgeTime, Gate, TrackConfig, TrackingSlicer};
use sdrkit_core::interface::Seconds;
use sdrkit_core::publish::field;
use sdrkit_core::{Complex32, Event, Generator, Hz, Setting, SignalInfo, Values};
use std::f64::consts::TAU;

const SECTION: &str = "Test signal";

/// The working rate is this to twice this, Hz.
pub const WORK_HZ: f64 = 160e3;
/// The pass edge before the envelope, Hz.
pub const PASS_HZ: f64 = 40e3;
/// The band that the noise of the test signal is stated in, Hz.
pub const NOISE_BAND_HZ: f64 = 40e3;
/// The longest gap that is a sync, s.
const SYNC_MAX_S: f64 = 0.040;
/// The most channels in a row before the list is dropped.
const MAX_IN_ROW: usize = 16;
/// The channels of a frame.
const FRAME_CHANNELS: (usize, usize) = (2, 12);
/// The time constant of the time in each state, s.
const STATE_TAU_S: f64 = 0.5;
/// A frame in progress is dropped after the gate has been shut this long in the low state, s.
const SHUT_DROP_S: f64 = 0.1;

/// The shortest sync gap.
pub const PPM_SYNC_MS: Setting<field::F64> =
    Setting::new("ppm_sync_ms", "Shortest sync gap (PPM)", "ms", 3.5).range(2.0, 10.0, 0.0);
/// The shortest channel.
pub const PPM_CHANNEL_MIN_MS: Setting<field::F64> =
    Setting::new("ppm_channel_min_ms", "Shortest channel (PPM)", "ms", 0.55).range(0.3, 5.0, 0.0);
/// The longest channel.
pub const PPM_CHANNEL_MAX_MS: Setting<field::F64> =
    Setting::new("ppm_channel_max_ms", "Longest channel (PPM)", "ms", 2.7).range(0.3, 5.0, 0.0);
/// The centre of travel.
pub const PPM_CENTRE_MS: Setting<field::F64> =
    Setting::new("ppm_centre_ms", "Centre of travel (PPM)", "ms", 1.5)
        .range(1.0, 2.0, 0.0)
        .help("A channel this long is 0 %; 0.5 ms less is -100 % and 0.5 ms more +100 %.");
/// The row interval.
pub const PPM_ROW_S: Setting<field::F64> =
    Setting::new("ppm_row_s", "Row interval (PPM)", "s", 0.5).range(0.1, 10.0, 0.0);

/// The channels of the test signal.
pub const GEN_CHANNELS_MS: Setting<field::Text> = Setting::new(
    "gen_channels_ms",
    "PPM channels",
    "ms",
    "1.5, 1.5, 1.5, 1.5",
)
.section(SECTION)
.help("The channel times of the PPM test signal, separated by commas; empty for a steady carrier.");
/// The frame.
pub const GEN_FRAME_MS: Setting<field::F64> = Setting::new("gen_frame_ms", "PPM frame", "ms", 20.0)
    .range(5.0, 50.0, 0.0)
    .section(SECTION);
/// The pulse.
pub const GEN_PULSE_US: Setting<field::F64> =
    Setting::new("gen_pulse_us", "PPM pulse", "us", 350.0)
        .range(50.0, 1000.0, 0.0)
        .section(SECTION);
/// The jitter of each channel.
pub const GEN_JITTER_US: Setting<field::F64> =
    Setting::new("gen_jitter_us", "PPM timing jitter", "us", 0.0)
        .range(0.0, 100.0, 0.0)
        .section(SECTION);
/// The clock of the transmitter.
pub const GEN_PPM_CLOCK_PPM: Setting<field::F64> = Setting::new(
    "gen_ppm_clock_ppm",
    "PPM transmitter clock error",
    "ppm",
    0.0,
)
.range(-200_000.0, 200_000.0, 0.0)
.section(SECTION);
/// The depth.
pub const GEN_PPM_DEPTH: Setting<field::F64> =
    Setting::new("gen_ppm_depth", "PPM keying depth", "", 1.0)
        .range(0.1, 1.0, 0.0)
        .section(SECTION);
/// The polarity.
pub const GEN_PPM_INVERT: Setting<field::Bool> = Setting::new(
    "gen_ppm_invert",
    "PPM carrier only in the pulse",
    "",
    false,
)
.section(SECTION)
.help(
    "Off: the pulse is a notch in a steady carrier. On: the carrier is on only during the pulse.",
);

/// The rules of a [`PpmReader`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PpmRules {
    /// The shortest sync, s.
    pub sync_s: f64,
    /// The channel window, s.
    pub channel_s: (f64, f64),
    /// The centre of travel, s.
    pub centre_s: f64,
    /// The row interval, s.
    pub row_s: f64,
}

impl Default for PpmRules {
    fn default() -> Self {
        PpmRules {
            sync_s: PPM_SYNC_MS.default * 1e-3,
            channel_s: (
                PPM_CHANNEL_MIN_MS.default * 1e-3,
                PPM_CHANNEL_MAX_MS.default * 1e-3,
            ),
            centre_s: PPM_CENTRE_MS.default * 1e-3,
            row_s: PPM_ROW_S.default,
        }
    }
}

/// A frame that the reader took.
#[derive(Clone, Debug, PartialEq)]
pub struct PpmFrame {
    /// The signal time of its sync.
    pub time_s: f64,
    /// The channels, s.
    pub channels: Vec<f64>,
    /// The frame, s.
    pub frame_s: f64,
    /// 20 log10 of peak over floor.
    pub level_db: f64,
}

/// The PPM reader (module docs).
#[derive(Debug)]
pub struct PpmReader {
    /// The rules.
    pub rules: PpmRules,
    nco: Option<Nco>,
    dec: Option<FirDecimator<Complex32>>,
    out: Vec<Complex32>,
    rate: f64,
    slicer: TrackingSlicer,
    in_state: [f64; 2],
    onset: Option<f64>,
    list: Vec<f64>,
    shut_since: Option<f64>,
    prev_count: Option<usize>,
    last_row: f64,
    /// Good frames.
    pub good: i64,
    /// Bad frames.
    pub bad: i64,
    /// The last good frame.
    pub last: Option<PpmFrame>,
    /// The last pulse onset, signal time.
    pub last_onset: Option<f64>,
    /// The frames that make a row, waiting.
    pub rows: Vec<PpmFrame>,
    input: (f64, f64),
}

impl PpmReader {
    /// A reader for input at `rate_hz` with the carrier `offset_hz` from the centre.
    pub fn new(rate_hz: f64, offset_hz: f64, rules: PpmRules) -> PpmReader {
        let d = ((rate_hz / WORK_HZ).floor() as usize).max(1);
        let dec = (d > 1)
            .then(|| FirDecimator::design(rate_hz, d, PASS_HZ, PASS_HZ, 60.0).ok())
            .flatten();
        let d = if dec.is_some() { d } else { 1 };
        let rate = rate_hz / d as f64;
        let cfg = TrackConfig {
            smoothing: 0.12,
            attack: 0.06,
            peak_decay_s: 0.05,
            floor_decay_s: 0.05,
            gate: Gate::Share(0.25),
            shut_level: None,
            hysteresis: 0.12,
            edge: EdgeTime::Interpolated { within: 6.0 },
            start_s: 0.02,
        };
        PpmReader {
            rules,
            nco: (offset_hz != 0.0).then(|| Nco::new(rate_hz, -offset_hz)),
            dec,
            out: Vec::new(),
            rate,
            slicer: TrackingSlicer::new(cfg, rate),
            in_state: [0.0; 2],
            onset: None,
            list: Vec::new(),
            shut_since: None,
            prev_count: None,
            last_row: f64::NEG_INFINITY,
            good: 0,
            bad: 0,
            last: None,
            last_onset: None,
            rows: Vec::new(),
            input: (rate_hz, offset_hz),
        }
    }

    /// The reader was made for input at `rate_hz` with the carrier at `offset_hz`.
    pub fn is_for(&self, rate_hz: f64, offset_hz: f64) -> bool {
        self.input == (rate_hz, offset_hz)
    }

    /// The time of the reader now: the time of the samples it took, s.
    pub fn now(&self) -> f64 {
        self.slicer.position() as f64 / self.rate
    }

    /// The state that is the pulse: the one with less time.
    pub fn pulse_state(&self) -> PpmPulseState {
        if self.in_state[0] <= self.in_state[1] {
            PpmPulseState::CarrierLow
        } else {
            PpmPulseState::CarrierHigh
        }
    }

    /// Forgets the frames and the counts.
    pub fn clear(&mut self) {
        self.list.clear();
        self.prev_count = None;
        self.good = 0;
        self.bad = 0;
        self.last = None;
        self.rows.clear();
    }

    fn pulse_level(&self) -> u8 {
        u8::from(self.pulse_state() == PpmPulseState::CarrierHigh)
    }

    fn onset_at(&mut self, t: f64) {
        self.last_onset = Some(t);
        let Some(prev) = self.onset.replace(t) else {
            return;
        };
        let d = t - prev;
        if d >= self.rules.sync_s && d <= SYNC_MAX_S {
            self.frame_end(t, d);
        } else if d >= self.rules.channel_s.0 && d <= self.rules.channel_s.1 {
            self.list.push(d);
            if self.list.len() > MAX_IN_ROW {
                self.list.clear();
            }
        } else {
            if d < SYNC_MAX_S {
                self.bad += 1;
            }
            self.list.clear();
        }
    }

    fn frame_end(&mut self, t: f64, sync: f64) {
        let n = self.list.len();
        if (FRAME_CHANNELS.0..=FRAME_CHANNELS.1).contains(&n) {
            let channels = std::mem::take(&mut self.list);
            let frame = PpmFrame {
                time_s: t,
                frame_s: sync + channels.iter().sum::<f64>(),
                channels,
                level_db: 20.0
                    * (self.slicer.peak() / self.slicer.floor().max(1e-12))
                        .max(1e-30)
                        .log10(),
            };
            if self.prev_count == Some(n) {
                self.good += 1;
                if t - self.last_row >= self.rules.row_s {
                    self.last_row = t;
                    self.rows.push(frame.clone());
                }
                self.last = Some(frame);
            }
            self.prev_count = Some(n);
        } else if n > 0 {
            self.bad += 1;
            self.prev_count = None;
        }
        self.list.clear();
    }

    /// Takes input samples; times count from the start of the reader.
    pub fn process(&mut self, x: &[Complex32]) {
        self.out.clear();
        let mut mixed;
        let src: &[Complex32] = match self.nco.as_mut() {
            Some(n) => {
                mixed = x.to_vec();
                n.mix_in_place(&mut mixed);
                &mixed
            }
            None => x,
        };
        match self.dec.as_mut() {
            Some(d) => d.process(src, &mut self.out),
            None => self.out.extend_from_slice(src),
        }
        let dt = 1.0 / self.rate;
        let k = (-dt / STATE_TAU_S).exp();
        let out = std::mem::take(&mut self.out);
        for &z in &out {
            let c = self.slicer.push_iq(z);
            let now = self.slicer.position() as f64 * dt;
            let s = usize::from(self.slicer.state());
            self.in_state[0] *= k;
            self.in_state[1] *= k;
            self.in_state[s] += dt;
            if !self.slicer.is_open() && self.slicer.state() != self.pulse_level() {
                let since = *self.shut_since.get_or_insert(now);
                if now - since > SHUT_DROP_S {
                    self.list.clear();
                    self.onset = None;
                }
            } else {
                self.shut_since = None;
            }
            if let Some(c) = c {
                // A change into the pulse state is an onset.
                if c.before != self.pulse_level() {
                    self.onset_at(c.at * dt);
                }
            }
        }
        self.out = out;
    }
}

impl PpmFrame {
    /// The row of the frame, with the text `values` of [`values_text`].
    pub fn row<'a>(&self, values: &'a str) -> RcLinksPpmFrameRow<'a> {
        RcLinksPpmFrameRow {
            time_s: Seconds(self.time_s),
            channels: self.channels.len() as i64,
            values_ms: values,
            frame_ms: self.frame_s * 1e3,
            level_db: self.level_db,
        }
    }

    /// The event of the frame, on the dial `dial_hz`.
    pub fn event(&self, dial_hz: f64) -> Event {
        let values = values_text(&self.channels);
        let r = self.row(&values);
        RC_PPM_FRAME.event(
            self.time_s,
            dial_hz,
            &RcPpmFrameEvent {
                time_s: r.time_s,
                channels: r.channels,
                values_ms: r.values_ms,
                frame_ms: r.frame_ms,
                level_db: r.level_db,
            },
        )
    }
}

/// The channel times in ms, two decimals, separated by commas.
pub fn values_text(ch: &[f64]) -> String {
    ch.iter()
        .map(|c| format!("{:.3}", c * 1e3))
        .collect::<Vec<_>>()
        .join(", ")
}

/// The channels in percent of travel about the centre `centre_s`, separated by commas.
pub fn percent_text(ch: &[f64], centre_s: f64) -> String {
    ch.iter()
        .map(|c| format!("{:.1}", (c - centre_s) * 1e3 * 200.0))
        .collect::<Vec<_>>()
        .join(", ")
}

/// The PPM test signal: a carrier of amplitude 1 at 0 Hz, keyed for a pulse at the start of each channel and once more to
/// close the last; the first pulse at 1.3 ms; each channel moved by its own Gaussian jitter. The edges pass a one-pole
/// low-pass at 100 kHz. The noise is stated against the carrier in 40 kHz.
#[derive(Clone, Debug)]
pub struct PpmGenerator {
    rate: f64,
    channels: Vec<f64>,
    frame_s: f64,
    pulse_s: f64,
    jitter_s: f64,
    scale: f64,
    depth: f64,
    invert: bool,
    alpha: f64,
    y: f64,
    n: u64,
    onsets: Vec<f64>,
    frame_start: f64,
    rng: u64,
    steady: bool,
}

impl PpmGenerator {
    /// The signal of the settings `v` at `rate_hz` with the seed `seed`.
    pub fn new(rate_hz: f64, v: &Values, seed: u64) -> PpmGenerator {
        let text = v.get(&GEN_CHANNELS_MS);
        let channels: Vec<f64> = text
            .split(',')
            .filter_map(|c| c.trim().parse::<f64>().ok())
            .map(|c| c * 1e-3)
            .collect();
        // An empty list is a steady carrier without pulses.
        let steady = text.trim().is_empty();
        let mut g = PpmGenerator {
            rate: rate_hz,
            channels: if channels.is_empty() && !steady {
                vec![1.5e-3; 4]
            } else {
                channels
            },
            steady,
            frame_s: v.get(&GEN_FRAME_MS) * 1e-3,
            pulse_s: v.get(&GEN_PULSE_US) * 1e-6,
            jitter_s: v.get(&GEN_JITTER_US) * 1e-6,
            scale: 1.0 / (1.0 + v.get(&GEN_PPM_CLOCK_PPM) * 1e-6),
            depth: v.get(&GEN_PPM_DEPTH),
            invert: v.get(&GEN_PPM_INVERT),
            alpha: 1.0 - (-TAU * 100e3 / rate_hz).exp(),
            y: 0.0,
            n: 0,
            onsets: Vec::new(),
            frame_start: 1.3e-3,
            rng: seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1,
        };
        g.y = if g.invert { 1.0 - g.depth } else { 1.0 };
        g.plan();
        g
    }

    fn gauss(&mut self) -> f64 {
        let mut u = || {
            self.rng ^= self.rng << 13;
            self.rng ^= self.rng >> 7;
            self.rng ^= self.rng << 17;
            (self.rng >> 11) as f64 / (1u64 << 53) as f64
        };
        let (a, b) = (u().max(1e-300), u());
        (-2.0 * a.ln()).sqrt() * (TAU * b).cos()
    }

    /// The onsets of the frame that starts at `frame_start`.
    fn plan(&mut self) {
        let mut t = self.frame_start;
        self.onsets.clear();
        if self.steady {
            return;
        }
        self.onsets.push(t);
        for k in 0..self.channels.len() {
            let j = self.jitter_s * self.gauss();
            t += (self.channels[k] + j) * self.scale;
            self.onsets.push(t);
        }
    }
}

impl Generator for PpmGenerator {
    fn rate_hz(&self) -> Hz {
        self.rate
    }
    fn generate(&mut self, out: &mut [Complex32]) -> usize {
        for z in out.iter_mut() {
            let t = self.n as f64 / self.rate;
            self.n += 1;
            if t >= self.frame_start + self.frame_s * self.scale {
                self.frame_start += self.frame_s * self.scale;
                self.plan();
            }
            let pulse = self
                .onsets
                .iter()
                .any(|&o| t >= o && t < o + self.pulse_s * self.scale);
            let on = pulse == self.invert;
            let a = if on { 1.0 } else { 1.0 - self.depth };
            self.y += self.alpha * (a - self.y);
            *z = Complex32::new(self.y as f32, 0.0);
        }
        out.len()
    }
    fn signal_info(&self) -> Option<SignalInfo> {
        Some(SignalInfo::new(NOISE_BAND_HZ).with_power(1.0))
    }
    fn station_offset_hz(&self, _tune_offset_hz: Hz) -> Hz {
        0.0
    }
}
