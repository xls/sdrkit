//! The AM processor: synchronous detection and "Auto quality". [`AmProcessor`] is the algorithm on the 48 kHz channel before the
//! channel filter, and [`AmProcessorComponent`] (`general.amproc`) is the component. It outputs exactly as many samples as it
//! receives, delayed by about 30 ms.

pub mod analysis;
pub mod dynamics;
pub mod nr;
pub mod pll;

use crate::blanker::ImpulseBlanker;
use crate::results::am_declaration;
use crate::smoothing::one_pole;
use crate::typed::{AmFields, WHISTLES};
use crate::types::RestartCommand;
use analysis::{Analysis, Side};
use dynamics::{FirBank, LookAheadLimiter};
use nr::LsaNoiseReduction;
use pll::{CarrierLoop, CarrierSearch};
use sdrkit_algo_dsp::dynamics::Leveller;
use sdrkit_algo_dsp::fir::{design_by_taps, hilbert, Fir};
use sdrkit_algo_dsp::iir::{Biquad, BiquadCoeffs};
use sdrkit_algo_dsp::ring::DelayLine;
use sdrkit_algo_dsp::window::Window;
use sdrkit_core::interface::Hertz;
use sdrkit_core::publish::field;
use sdrkit_core::{
    Choice, CommandError, CommandId, CommandSet, Commands, Complex32, ConfigError, Published,
    Record, Results, Setting, SettingError, SettingId, SettingSet, Value, Values,
};
use sdrkit_graph::{
    values_of, Applied, Component, ComponentInfo, Format, Io, Item, Kept, Kind, Port, RateFrom,
    Timed,
};
use std::sync::Arc;

/// The processor's rate.
pub const RATE: f64 = 48_000.0;
const HILBERT_TAPS: usize = 255;
const LPF_TAPS: usize = 191;

sdrkit_core::choice_enum! {
    /// The detector that a user picks.
    #[derive(Default)]
    pub enum AmDetector {
        /// Auto quality chooses (both sidebands, or one when the other carries interference).
        #[default]
        Auto = ("auto", "Auto"),
        /// The envelope (no carrier needed).
        Envelope = ("envelope", "Envelope"),
        /// Synchronous, both sidebands.
        Synchronous = ("synchronous", "Synchronous"),
        /// Synchronous, upper sideband only.
        Upper = ("upper", "Upper only"),
        /// Synchronous, lower sideband only.
        Lower = ("lower", "Lower only"),
    }
}

impl AmDetector {
    /// A synchronous kind.
    pub fn is_synchronous(self) -> bool {
        matches!(
            self,
            AmDetector::Synchronous | AmDetector::Upper | AmDetector::Lower
        )
    }
}

sdrkit_core::choice_enum! {
    /// The noise reduction that a user picks.
    #[derive(Default)]
    pub enum AmNoise {
        /// From the programme SNR.
        #[default]
        Auto = ("auto", "Auto"),
        /// None.
        Off = ("off", "Off"),
        /// Strength 30 of 100.
        Gentle = ("gentle", "Gentle"),
        /// Strength 90 of 100.
        Strong = ("strong", "Strong"),
    }
}

/// The noise reduction that the processor runs.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum AmNr {
    /// From the programme SNR.
    Auto,
    /// None.
    Off,
    /// A strength from 1 to 100.
    Strength(f32),
}

impl From<AmNoise> for AmNr {
    fn from(n: AmNoise) -> AmNr {
        match n {
            AmNoise::Auto => AmNr::Auto,
            AmNoise::Off => AmNr::Off,
            AmNoise::Gentle => AmNr::Strength(30.0),
            AmNoise::Strong => AmNr::Strength(90.0),
        }
    }
}

/// The settings of the processor, as the receiver supplies them.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AmConfig {
    /// Auto quality.
    pub auto: bool,
    /// The detector chosen.
    pub detector: AmDetector,
    /// A fixed audio bandwidth (Hz). 0 = automatic.
    pub bandwidth: f64,
    /// Noise reduction.
    pub nr: AmNr,
    /// The user switched the blanker on.
    pub nb: bool,
    /// The user's blanker threshold (dB).
    pub nb_threshold: f64,
    /// Synthetic treble.
    pub enhance: bool,
    /// AGC.
    pub agc: bool,
    /// Manual gain (dB).
    pub gain_db: f64,
    /// The channel width (Hz), for the bandwidth when Auto quality is off.
    pub channel_width: f64,
}

impl Default for AmConfig {
    fn default() -> Self {
        AmConfig {
            auto: false,
            detector: AmDetector::Envelope,
            bandwidth: 0.0,
            nr: AmNr::Auto,
            nb: false,
            nb_threshold: 12.0,
            enhance: false,
            agc: true,
            gain_db: 20.0,
            channel_width: 9000.0,
        }
    }
}

impl AmConfig {
    /// Whether the processor runs: Auto quality, or a synchronous detector chosen.
    pub fn active(&self) -> bool {
        self.auto || self.detector.is_synchronous()
    }
}

sdrkit_core::choice_enum! {
    /// The detector in use.
    #[derive(Default)]
    pub enum InUse {
        /// The envelope.
        #[default]
        Envelope = ("envelope", "Envelope"),
        /// Synchronous, both sidebands.
        Synchronous = ("synchronous", "Synchronous"),
        /// Upper only.
        Upper = ("upper", "Upper only"),
        /// Lower only.
        Lower = ("lower", "Lower only"),
    }
}

/// The status of the processor, published every 0.1 s.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct AmStatus {
    /// The processor runs.
    pub active: bool,
    /// A carrier was found.
    pub carrier_found: bool,
    /// The loop is locked.
    pub locked: bool,
    /// Hz from the tuned frequency.
    pub carrier_offset_hz: f64,
    /// dBFS, from the 1 s level.
    pub carrier_dbfs: f32,
    /// Programme SNR (dB).
    pub programme_snr_db: f32,
    /// The detector in use.
    pub detector: InUse,
    /// One sideband chosen because of interference.
    pub one_side: bool,
    /// Interference measure of the upper and lower sides.
    pub interference_upper: f32,
    /// See `interference_upper`.
    pub interference_lower: f32,
    /// Audio bandwidth in use (Hz).
    pub bandwidth_hz: f64,
    /// Noise reduction floor (dB, 0 off).
    pub nr_floor_db: f32,
    /// Share blanked (percent).
    pub blanked_pct: f32,
    /// The blanker's threshold (dB).
    pub blanker_db: f32,
    /// The whistles notched (Hz).
    pub whistles_hz: Vec<f64>,
    /// 50 or 60 when hum is notched.
    pub hum_hz: u32,
    /// Fading: the loop holds.
    pub fading: bool,
    /// Synthetic treble on.
    pub enhanced: bool,
    /// Leveller gain (dB).
    pub leveller_db: f32,
    /// The line in plain words.
    pub line: String,
}

/// What the report needs of the processor every 0.1 s.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct AmReport {
    /// The processor runs.
    pub active: bool,
    /// Share blanked (percent).
    pub blanked_pct: f32,
}

impl Item for AmReport {
    const KIND: &'static str = "general.am_report";
}

/// The AM processor.
pub struct AmProcessor {
    cfg: AmConfig,
    nb: ImpulseBlanker,
    nb_offset: f64,
    nb_count: u64,
    search: CarrierSearch,
    pll: CarrierLoop,
    analysis: Analysis,
    env_lpf: FirBank<Complex32>,
    env_delay: DelayLine<f32>,
    hilbert: Fir<f32>,
    re_delay: DelayLine<f32>,
    detector: InUse,
    prev_detector: InUse,
    fade: u32,
    fade_len: u32,
    dc: f32,
    dc_a: f32,
    audio_lpf: FirBank<f32>,
    bw: f64,
    notches: Vec<Biquad>,
    notch_hz: Vec<f64>,
    hum: Option<u32>,
    nr: LsaNoiseReduction,
    presence: Option<Biquad>,
    enh_bp: Biquad,
    enh_hp: Biquad,
    enh_env: f32,
    enh_a: f32,
    leveller: Leveller,
    limiter: LookAheadLimiter,
    status: AmStatus,
}

/// The 16 audio low-passes, 2500 to 10 000 Hz in steps of 500 Hz (Blackman windowed sinc, unity gain at DC).
fn lpf_bank() -> Vec<Vec<f32>> {
    (0..16)
        .map(|i| {
            let fc = (2500.0 + 500.0 * i as f64) / RATE;
            design_by_taps(LPF_TAPS, fc, Window::Blackman)
                .expect("the cut-offs are inside 0 to 0.5")
        })
        .collect()
}

/// The Hilbert transformer's Kaiser beta: 8.6 has the side lobes of a Blackman window.
const HILBERT_BETA: f64 = 8.6;

fn bank_index(bw: f64) -> usize {
    (((bw.clamp(2500.0, 10_000.0) - 2500.0) / 500.0).round() as usize).min(15)
}

impl AmProcessor {
    /// A processor with these settings.
    pub fn new(cfg: AmConfig) -> Self {
        let bank = lpf_bank();
        let mut p = AmProcessor {
            cfg,
            nb: ImpulseBlanker::new(RATE, 0.05, 15.0, 8),
            nb_offset: 0.0,
            nb_count: 0,
            search: CarrierSearch::new(RATE),
            pll: CarrierLoop::new(RATE),
            analysis: Analysis::new(RATE),
            env_lpf: FirBank::new(bank.clone(), bank_index(4500.0), 2048),
            env_delay: DelayLine::new((HILBERT_TAPS - 1) / 2 - (LPF_TAPS - 1) / 2),
            hilbert: Fir::new(
                &hilbert(HILBERT_TAPS, HILBERT_BETA).expect("the filter settings are valid"),
            ),
            re_delay: DelayLine::new((HILBERT_TAPS - 1) / 2),
            detector: InUse::Envelope,
            prev_detector: InUse::Envelope,
            fade: 0,
            fade_len: (0.02 * RATE) as u32,
            dc: 0.0,
            dc_a: one_pole(0.05, RATE),
            audio_lpf: FirBank::new(bank, bank_index(4500.0), 2048),
            bw: 4500.0,
            notches: Vec::with_capacity(10),
            notch_hz: Vec::with_capacity(10),
            hum: None,
            nr: LsaNoiseReduction::new(RATE),
            presence: None,
            enh_bp: Biquad::new(BiquadCoeffs::bandpass(RATE, 3150.0, 1.5)),
            enh_hp: Biquad::new(BiquadCoeffs::highpass(RATE, 4500.0, 0.707)),
            enh_env: 0.0,
            enh_a: one_pole(0.01, RATE),
            leveller: Leveller::new(RATE, 0.12),
            limiter: LookAheadLimiter::new(RATE, 0.003, 0.9, 0.05),
            status: AmStatus::default(),
        };
        p.apply_config();
        p
    }
    /// The total delay (samples).
    pub fn delay(&self) -> usize {
        self.nb.delay()
            + (HILBERT_TAPS - 1) / 2
            + self.audio_lpf.delay()
            + self.nr.delay()
            + self.limiter.delay()
    }
    /// The settings in use.
    pub fn config(&self) -> &AmConfig {
        &self.cfg
    }
    /// New settings (applied from the next sample, nothing restarts).
    pub fn set_config(&mut self, cfg: AmConfig) {
        self.cfg = cfg;
        self.apply_config();
    }
    fn fixed_bandwidth(&self) -> Option<f64> {
        if self.cfg.bandwidth > 0.0 {
            Some(self.cfg.bandwidth.clamp(2500.0, 10_000.0))
        } else if !self.cfg.auto {
            Some((self.cfg.channel_width / 2.0).clamp(2500.0, 10_000.0))
        } else {
            None
        }
    }
    fn apply_config(&mut self) {
        let bw = self
            .fixed_bandwidth()
            .unwrap_or(if self.analysis.findings.valid {
                self.analysis.findings.auto_bandwidth
            } else {
                4500.0
            });
        self.set_bandwidth(bw);
        self.limiter.active = self.cfg.agc;
        if !self.cfg.auto {
            self.notches.clear();
            self.notch_hz.clear();
            self.hum = None;
        }
        self.nb.set_threshold_db(self.nb_threshold());
        self.update_nr_floor();
    }
    fn nb_enabled(&self) -> bool {
        self.cfg.auto || self.cfg.nb
    }
    fn nb_threshold(&self) -> f64 {
        let base = if self.cfg.nb {
            self.cfg.nb_threshold
        } else {
            15.0
        };
        base + self.nb_offset
    }
    fn set_bandwidth(&mut self, bw: f64) {
        let bw = ((bw.clamp(2500.0, 10_000.0)) / 500.0).round() * 500.0;
        if bw == self.bw && self.presence.is_some() == (self.cfg.auto && bw <= 6000.0) {
            return;
        }
        self.bw = bw;
        let i = bank_index(bw);
        self.audio_lpf.select(i);
        self.env_lpf.select(i);
        self.presence = (self.cfg.auto && bw <= 6000.0)
            .then(|| Biquad::new(BiquadCoeffs::high_shelf(RATE, 0.55 * bw, 0.707, 3.0)));
        self.enh_bp
            .set_coeffs(BiquadCoeffs::bandpass(RATE, 0.7 * bw, 1.5));
        self.enh_hp
            .set_coeffs(BiquadCoeffs::highpass(RATE, bw, 0.707));
        self.nr.bandwidth = bw;
    }
    fn update_nr_floor(&mut self) {
        let floor = if !self.cfg.auto {
            0.0
        } else {
            match self.cfg.nr {
                AmNr::Off => 0.0,
                AmNr::Strength(s) => 3.0 + 0.15 * s,
                AmNr::Auto => {
                    let snr = self.analysis.findings.snr_db;
                    if !self.analysis.findings.valid || snr > 35.0 {
                        0.0
                    } else {
                        (6.0 + 0.4 * (35.0 - snr)).clamp(6.0, 16.0)
                    }
                }
            }
        };
        self.nr.floor_db = floor;
    }
    fn after_analysis(&mut self) {
        if self.fixed_bandwidth().is_none() {
            let bw = self.analysis.findings.auto_bandwidth;
            self.set_bandwidth(bw);
        }
        self.update_nr_floor();
        if self.cfg.auto {
            let f = &self.analysis.findings;
            let mut want: Vec<f64> = f
                .whistles
                .iter()
                .copied()
                .filter(|&w| w < self.bw)
                .collect();
            want.truncate(6);
            let hum = f.hum;
            let changed = want.len()
                != self
                    .notch_hz
                    .len()
                    .saturating_sub(if self.hum.is_some() { 4 } else { 0 })
                || hum != self.hum
                || want
                    .iter()
                    .zip(&self.notch_hz)
                    .any(|(a, b)| (a - b).abs() > 3.0);
            if changed {
                self.notches.clear();
                self.notch_hz.clear();
                for &w in &want {
                    if w > 20.0 && w < RATE / 2.0 - 100.0 {
                        self.notches
                            .push(Biquad::new(BiquadCoeffs::notch(RATE, w, 12.0)));
                        self.notch_hz.push(w);
                    }
                }
                if let Some(h) = hum {
                    for k in 1..=4 {
                        let f0 = h as f64 * k as f64;
                        self.notches
                            .push(Biquad::new(BiquadCoeffs::notch(RATE, f0, 8.0)));
                        self.notch_hz.push(f0);
                    }
                }
                self.hum = hum;
            }
        }
    }
    fn wanted_detector(&self) -> InUse {
        if !self.pll.locked {
            return InUse::Envelope;
        }
        match self.cfg.detector {
            AmDetector::Synchronous => InUse::Synchronous,
            AmDetector::Upper => InUse::Upper,
            AmDetector::Lower => InUse::Lower,
            AmDetector::Envelope if !self.cfg.auto => InUse::Envelope,
            _ => match self.analysis.side {
                Side::Both => InUse::Synchronous,
                Side::Upper => InUse::Upper,
                Side::Lower => InUse::Lower,
            },
        }
    }
    /// Processes `x` (48 kHz, the channel before the filter), appending as many samples to `out`.
    pub fn process(&mut self, x: &[Complex32], out: &mut Vec<f32>) {
        let nb_on = self.nb_enabled();
        let gain = 10f64.powf(self.cfg.gain_db / 20.0) as f32;
        let second = RATE as u64;
        for &v in x {
            let xb = self.nb.push(v, nb_on);
            // Adaptive blanker threshold, each second.
            self.nb_count += 1;
            if self.nb_count.is_multiple_of(second) {
                let share = self.nb.take_share();
                self.status.blanked_pct = share;
                if share > 5.0 {
                    self.nb_offset = (self.nb_offset + 3.0).min(12.0);
                } else if share < 0.5 {
                    self.nb_offset = (self.nb_offset - 1.0).max(0.0);
                }
                self.nb.set_threshold_db(self.nb_threshold());
            }
            if !self.pll.running {
                if let Some(Some(hz)) = self.search.push(xb) {
                    self.pll.acquire(hz);
                }
            }
            let (r, restart) = self.pll.push(xb);
            if restart {
                self.pll.drop_lock();
                self.search.restart();
            }
            if self.analysis.push(r, self.bw) {
                self.after_analysis();
            }
            // Detectors, aligned on the Hilbert transformer's centre.
            let env = self.env_delay.push(self.env_lpf.push(xb).norm());
            let h = self.hilbert.process_sample(r.im);
            let rd = self.re_delay.push(r.re);
            let det = [env, rd, rd - h, rd + h];
            let want = self.wanted_detector();
            if want != self.detector {
                self.prev_detector = self.detector;
                self.detector = want;
                self.fade = self.fade_len;
            }
            let mut d = det[self.detector as usize];
            if self.fade > 0 {
                let t = self.fade as f32 / self.fade_len as f32;
                d = det[self.prev_detector as usize] * t + d * (1.0 - t);
                self.fade -= 1;
            }
            self.dc += self.dc_a * (d - self.dc);
            let a = d - self.dc;
            let mut y = if self.cfg.agc {
                a / self.pll.level().max(1e-6) * 0.5
            } else {
                a * gain
            };
            // Audio stages.
            y = self.audio_lpf.push(y);
            for n in &mut self.notches {
                y = n.process_sample(y);
            }
            y = self.nr.push(y);
            if let Some(p) = &mut self.presence {
                y = p.process_sample(y);
            }
            if self.cfg.auto && self.cfg.enhance {
                let b = self.enh_bp.process_sample(y).abs();
                self.enh_env += self.enh_a * (b - self.enh_env);
                let t = self.enh_hp.process_sample(b - 0.5 * self.enh_env);
                y += 0.18 * t;
            }
            if self.cfg.agc && self.cfg.auto {
                y = self.leveller.push(y);
            }
            y = self.limiter.push(y);
            out.push(y.clamp(-1.0, 1.0));
        }
    }
    /// Fills and returns the status.
    pub fn status(&mut self) -> &AmStatus {
        let nb_thr = self.nb_threshold() as f32;
        let s = &mut self.status;
        let f = &self.analysis.findings;
        s.active = self.cfg.active();
        s.carrier_found = self.pll.running;
        s.locked = self.pll.locked;
        s.carrier_offset_hz = if self.pll.running {
            self.pll.offset_hz()
        } else {
            0.0
        };
        s.carrier_dbfs = 20.0 * self.pll.level().max(1e-10).log10();
        s.programme_snr_db = f.snr_db;
        s.detector = self.detector;
        s.one_side = self.analysis.side != Side::Both && self.detector != InUse::Envelope;
        s.interference_upper = f.interference.0;
        s.interference_lower = f.interference.1;
        s.bandwidth_hz = self.bw;
        s.nr_floor_db = self.nr.floor_db;
        s.blanker_db = nb_thr;
        s.whistles_hz.clear();
        let nw = self.notch_hz.len() - if self.hum.is_some() { 4 } else { 0 };
        s.whistles_hz.extend_from_slice(&self.notch_hz[..nw]);
        s.hum_hz = self.hum.unwrap_or(0);
        s.fading = self.pll.holding;
        s.enhanced = self.cfg.auto && self.cfg.enhance;
        s.leveller_db = if self.cfg.agc && self.cfg.auto {
            self.leveller.gain_db()
        } else {
            0.0
        };
        let line = &mut s.line;
        line.clear();
        use std::fmt::Write;
        if !self.pll.running {
            line.push_str("Looking for the carrier");
        } else if !self.pll.locked {
            line.push_str("Envelope detector (no steady carrier to lock to)");
        } else {
            let _ = write!(
                line,
                "Locked to the carrier ({:+.0} Hz)",
                self.pll.offset_hz()
            );
        }
        if s.one_side {
            line.push_str(match self.analysis.side {
                Side::Lower => ", interference on the upper sideband: lower one only",
                _ => ", interference on the lower sideband: upper one only",
            });
        }
        if s.fading {
            line.push_str(", fading: holding");
        }
        if self.cfg.auto || self.cfg.bandwidth > 0.0 {
            let _ = write!(line, "; audio to {:.1} kHz", self.bw / 1000.0);
        }
        if s.nr_floor_db > 0.0 {
            line.push_str(if f.valid && f.snr_db < 15.0 {
                "; weak signal: strong noise reduction"
            } else {
                "; noise reduction"
            });
        }
        if s.blanked_pct > 0.05 {
            line.push_str("; clicks blanked");
        }
        for w in &s.whistles_hz {
            let _ = write!(line, "; whistle at {:.2} kHz removed", w / 1000.0);
        }
        if s.hum_hz > 0 {
            let _ = write!(line, "; {} Hz hum removed", s.hum_hz);
        }
        if s.enhanced {
            line.push_str("; enhanced (synthetic treble)");
        }
        &self.status
    }
}

const ACTIVE: Setting<field::Bool> = Setting::new("active", "Active", "", false);
const AUTO: Setting<field::Bool> = Setting::new("auto", "Auto quality", "", false);
const DETECTOR: Setting<field::Choice<AmDetector>> =
    Setting::choice("detector", "Detector", AmDetector::Envelope);
const BANDWIDTH: Setting<field::Hz> =
    Setting::new("bandwidth", "Audio bandwidth", "Hz", 0.0).range(0.0, 10_000.0, 0.0);
const NR: Setting<field::Choice<AmNoise>> = Setting::choice("nr", "Noise", AmNoise::Auto);
const NB: Setting<field::Bool> = Setting::new("nb", "Blanker", "", false);
const NB_THRESHOLD: Setting<field::F64> =
    Setting::new("nb_threshold", "Blanker threshold", "dB", 12.0).range(6.0, 30.0, 0.0);
const ENHANCE: Setting<field::Bool> = Setting::new("enhance", "Enhance", "", false);
const AGC: Setting<field::Bool> = Setting::new("agc", "AGC", "", true);
const GAIN: Setting<field::F64> = Setting::new("gain", "Gain", "dB", 20.0).range(-20.0, 80.0, 0.0);
const WIDTH: Setting<field::Hz> =
    Setting::new("width", "Channel width", "Hz", 9000.0).range(1000.0, 20_000.0, 0.0);

sdrkit_core::settings! {
    /// The settings of [`AmProcessorComponent`].
    pub enum AmProcessorSetting {
        /// The processor runs. Off, it outputs silence.
        Active = ACTIVE,
        /// Auto quality.
        Auto = AUTO,
        /// The detector.
        Detector = DETECTOR,
        /// A fixed audio bandwidth (Hz, 0 for automatic).
        Bandwidth = BANDWIDTH,
        /// The noise reduction.
        Nr = NR,
        /// The blanker.
        Nb = NB,
        /// The threshold of the blanker (dB).
        NbThreshold = NB_THRESHOLD,
        /// Synthetic treble.
        Enhance = ENHANCE,
        /// The AGC.
        Agc = AGC,
        /// The gain when the AGC is off (dB).
        Gain = GAIN,
        /// The channel width (Hz), for the bandwidth when Auto quality is off.
        Width = WIDTH,
    }
}

/// The AM processor component (`general.amproc`): the 48 kHz channel before the channel filter in, `Real` audio (as many
/// samples) and an [`AmReport`] every 0.1 s of signal out. It publishes the AM fields and the whistle series. While not active it
/// outputs silence and costs nothing. Its command `restart` starts the processor again.
pub struct AmProcessorComponent {
    cfg: AmConfig,
    nr: AmNoise,
    active: bool,
    p: Option<Box<AmProcessor>>,
    count: u64,
    report: u64,
    results: Published,
    whistles: sdrkit_core::SeriesId,
    written: Option<bool>,
}

impl Default for AmProcessorComponent {
    fn default() -> Self {
        let decl = Arc::new(am_declaration());
        let whistles = decl
            .series_id(WHISTLES.key)
            .expect("the AM declaration has the whistle series");
        AmProcessorComponent {
            cfg: AmConfig::default(),
            nr: AmNoise::Auto,
            active: false,
            p: None,
            count: 0,
            report: 4800,
            results: Published::from_decl(decl),
            whistles,
            written: None,
        }
    }
}

/// Writes the AM fields and the whistle series of `s`. The measurements have no value while the processor does not run.
fn publish(p: &mut Published, whistles: sdrkit_core::SeriesId, s: &AmStatus) {
    let on = |v: f32| s.active.then_some(f64::from(v));
    p.put(&AmFields {
        am_active: s.active,
        am_carrier_found: s.carrier_found,
        am_locked: s.locked,
        am_carrier_offset_hz: (s.active && s.carrier_found).then_some(Hertz(s.carrier_offset_hz)),
        am_carrier_dbfs: on(s.carrier_dbfs),
        am_programme_snr_db: on(s.programme_snr_db),
        am_detector: s.active.then_some(s.detector),
        am_one_side: s.one_side,
        am_interference_upper: on(s.interference_upper),
        am_interference_lower: on(s.interference_lower),
        am_bandwidth_hz: s.active.then_some(Hertz(s.bandwidth_hz)),
        am_nr_floor_db: on(s.nr_floor_db).filter(|_| s.nr_floor_db != 0.0),
        am_blanked_pct: on(s.blanked_pct),
        am_blanker_db: on(s.blanker_db),
        am_hum_hz: (s.hum_hz > 0).then_some(Hertz(f64::from(s.hum_hz))),
        am_fading: s.fading,
        am_enhanced: s.enhanced,
        am_leveller_db: on(s.leveller_db),
        am_status: (!s.line.is_empty()).then_some(s.line.as_str()),
    });
    p.results_mut().update_series(whistles, |series| {
        let same = series.points.len() == s.whistles_hz.len()
            && series
                .points
                .iter()
                .zip(&s.whistles_hz)
                .all(|(p, hz)| p[1] == *hz as f32);
        if same {
            return false;
        }
        series.line = false;
        series.points.clear();
        series.points.extend(
            s.whistles_hz
                .iter()
                .enumerate()
                .map(|(i, hz)| [i as f32, *hz as f32]),
        );
        true
    });
}

impl AmProcessorComponent {
    /// Writes the empty status once after the processor stops.
    fn publish_idle(&mut self) {
        if self.written != Some(false) {
            publish(&mut self.results, self.whistles, &AmStatus::default());
            self.written = Some(false);
        }
    }
}

impl Component for AmProcessorComponent {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "general.amproc",
            vec![Port::iq("in")],
            vec![
                Port::real("out").same_format_as(0),
                Port::typed::<AmReport>("status").rate_from(RateFrom::Fixed(0.0)),
            ],
        )
        .with_settings(AmProcessorSetting::schema())
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        let f = &inputs[0];
        if self.active && (f.rate - RATE).abs() > 1e-6 {
            self.active = false;
        }
        self.p = None;
        self.count = 0;
        self.report = (f.rate / 10.0).round().max(1.0) as u64;
        self.publish_idle();
        Ok(vec![
            Format {
                kind: Kind::Real,
                bandwidth_hz: 10e3,
                ..f.clone()
            },
            Format::new(Kind::typed::<AmReport>(), 0.0),
        ])
    }
    fn process(&mut self, io: &mut Io) {
        let x = io.iq(0);
        let start = io.output(0).end();
        if !self.active || (io.input_format(0).rate - RATE).abs() > 1e-6 {
            let out = io.real_out(0);
            out.resize(out.len() + x.len(), 0.0);
            self.publish_idle();
            return;
        }
        let cfg = self.cfg;
        let p = self
            .p
            .get_or_insert_with(|| Box::new(AmProcessor::new(cfg)));
        p.process(x, io.real_out(0));
        let before = self.count;
        self.count += x.len() as u64;
        if self.count / self.report != before / self.report {
            let st = p.status();
            publish(&mut self.results, self.whistles, st);
            self.written = Some(true);
            let t = (start + x.len() as u64) as f64 / RATE;
            let value = AmReport {
                active: st.active,
                blanked_pct: st.blanked_pct,
            };
            io.items_out::<AmReport>(1).push(Timed { time: t, value });
        }
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        let c = &mut self.cfg;
        match AmProcessorSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            AmProcessorSetting::Active => {
                let a = ACTIVE.read(id, v)?;
                if a != self.active {
                    self.p = None;
                }
                self.active = a;
            }
            AmProcessorSetting::Auto => c.auto = AUTO.read(id, v)?,
            AmProcessorSetting::Detector => c.detector = DETECTOR.read(id, v)?,
            AmProcessorSetting::Bandwidth => c.bandwidth = BANDWIDTH.read(id, v)?,
            AmProcessorSetting::Nr => {
                self.nr = NR.read(id, v)?;
                c.nr = AmNr::from(self.nr);
            }
            AmProcessorSetting::Nb => c.nb = NB.read(id, v)?,
            AmProcessorSetting::NbThreshold => c.nb_threshold = NB_THRESHOLD.read(id, v)?,
            AmProcessorSetting::Enhance => c.enhance = ENHANCE.read(id, v)?,
            AmProcessorSetting::Agc => c.agc = AGC.read(id, v)?,
            AmProcessorSetting::Gain => c.gain_db = GAIN.read(id, v)?,
            AmProcessorSetting::Width => c.channel_width = WIDTH.read(id, v)?,
        }
        let cfg = self.cfg;
        if let Some(p) = &mut self.p {
            p.set_config(cfg);
        }
        Ok(Applied::Live)
    }
    fn values(&self) -> Values {
        let c = &self.cfg;
        values_of([
            (AmProcessorSetting::Active, Value::Bool(self.active)),
            (AmProcessorSetting::Auto, Value::Bool(c.auto)),
            (
                AmProcessorSetting::Detector,
                Value::Choice(c.detector.index()),
            ),
            (AmProcessorSetting::Bandwidth, Value::Hz(c.bandwidth)),
            (AmProcessorSetting::Nr, Value::Choice(self.nr.index())),
            (AmProcessorSetting::Nb, Value::Bool(c.nb)),
            (AmProcessorSetting::NbThreshold, Value::F64(c.nb_threshold)),
            (AmProcessorSetting::Enhance, Value::Bool(c.enhance)),
            (AmProcessorSetting::Agc, Value::Bool(c.agc)),
            (AmProcessorSetting::Gain, Value::F64(c.gain_db)),
            (AmProcessorSetting::Width, Value::Hz(c.channel_width)),
        ])
    }
    fn results(&self) -> Option<&Results> {
        Some(self.results.results())
    }
    /// Starts the processor again and writes the empty status.
    fn forget(&mut self, _kept: &Kept) {
        self.p = None;
        self.count = 0;
        self.written = None;
        self.publish_idle();
    }
    fn commands(&self) -> &Commands {
        RestartCommand::commands()
    }
    fn invoke(&mut self, id: CommandId, _args: &Values) -> Result<Option<Record>, CommandError> {
        match RestartCommand::from_id(id).ok_or(CommandError::NoSuchId { id })? {
            RestartCommand::Restart => self.p = None,
        }
        Ok(None)
    }
}
