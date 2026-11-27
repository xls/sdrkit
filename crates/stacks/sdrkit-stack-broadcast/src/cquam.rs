//! AM stereo, C-QUAM: the receiver's components, its graph, its declared results and the generator of the clean test signal.
//!
//! Standard: Motorola C-QUAM AM stereo system, as adopted in ITU-R BS.1062.

use crate::component::ClearCommand;
use crate::typed::{CquamFields, CquamFrontFields, PILOT_SPECTRUM};
use sdrkit_algo_dsp::fft::Fft;
use sdrkit_algo_dsp::goertzel::goertzel_windowed;
use sdrkit_algo_dsp::iir::{Biquad, BiquadCoeffs};
use sdrkit_algo_dsp::loopfilter::{LoopFilter, PhaseAcc};
use sdrkit_core::interface::{Fields, Hertz, Seconds};
use sdrkit_core::publish::{field, Published};
use sdrkit_core::{
    Choice, CommandError, CommandId, CommandSet, Commands, Complex32, ConfigError, GenConfig,
    Generator, LockState, Receiver, Record, Results, ResultsDecl, SeriesId, Setting, SettingError,
    SettingId, SettingSet, SignalInfo, StackConfig, Status, Value, Values,
};
use sdrkit_graph::components::{Channel, FirFilter};
use sdrkit_graph::{
    no_values, values_of, Applied, Catalog, ChannelsFrom, Component, ComponentInfo, Format, Graph,
    GraphBuilder, GraphError, Io, Item, Kept, Kind, Port, RateFrom, RateRule, StackGraph, Timed,
};
use std::collections::VecDeque;
use std::f64::consts::TAU;
use std::sync::Arc;

/// The stack id.
pub const ID: &str = "cquam";
/// The demodulation rate.
pub const RATE: f64 = 48_000.0;
/// The tune offset (the radio is tuned this far above the dial).
pub const TUNE_OFFSET_HZ: f64 = 250e3;
/// The minimum input rate.
pub const MIN_RATE: f64 = 250e3;
/// The block of the carrier decisions and of the report rate: 0.1 s.
const BLOCK: u64 = 4800;
/// The loop noise bandwidth while it pulls in: a natural frequency of 30 Hz with damping 0.707.
const BN_PULL_HZ: f64 = 100.0;
/// The loop noise bandwidth once locked: a natural frequency of 4 Hz, well under the 25 Hz pilot.
const BN_LOCKED_HZ: f64 = 13.3;
/// The loop damping.
const ZETA: f64 = std::f64::consts::FRAC_1_SQRT_2;
/// The rate of the pilot analysis.
const PILOT_RATE: f64 = 1000.0;
/// Samples of the pilot analysis at 1 kHz (1 s).
const PILOT_WINDOW: usize = 1000;

sdrkit_core::choice_enum! {
    /// How the receiver chooses between mono and stereo.
    #[derive(Default)]
    pub enum StereoMode {
        /// The pilot decides.
        #[default]
        Auto = ("auto", "Auto"),
        /// Always mono.
        Mono = ("mono", "Mono"),
        /// L - R used whatever the pilot says.
        Stereo = ("stereo", "Stereo"),
    }
}

/// What the carrier loop reports at the end of each 0.1 s block.
#[derive(Clone, Debug, PartialEq)]
pub struct CarrierInfo {
    /// The 48 kHz sample index this applies from.
    pub at: u64,
    /// Locked.
    pub locked: bool,
    /// The loop frequency, Hz.
    pub offset_hz: f64,
    /// The carrier amplitude.
    pub amplitude: f32,
    /// Mean power in the channel (after the filter).
    pub channel_power: f32,
    /// Mean cos φ of the stereo phase.
    pub cos_phi: f32,
}
impl Item for CarrierInfo {
    const KIND: &'static str = "cquam.carrier";
}

/// The noise density beside the channel.
#[derive(Clone, Debug, PartialEq)]
pub struct NoiseInfo {
    /// Sample index.
    pub at: u64,
    /// Noise power per Hz.
    pub density: f64,
}
impl Item for NoiseInfo {
    const KIND: &'static str = "cquam.noise";
}

/// One pilot check.
#[derive(Clone, Debug, PartialEq)]
pub struct PilotInfo {
    /// Sample index.
    pub at: u64,
    /// The check found the pilot.
    pub hit: bool,
    /// Stereo after this check (two hits in a row switch on, three misses off).
    pub stereo: bool,
    /// The 25 Hz bin over the mean of its four neighbours, dB.
    pub level_db: f32,
    /// Pilot amplitude as a percentage of the carrier.
    pub percent: f32,
    /// The quadrature spectrum 0 to 60 Hz, dB relative to the carrier.
    pub spectrum: [f32; 61],
}
impl Item for PilotInfo {
    const KIND: &'static str = "cquam.pilot";
}

/// The audio width either side of the carrier.
pub const WIDTH: Setting<field::Hz> = Setting::new("width", "Width", "Hz", 5000.0)
    .range(3000.0, 10000.0, 500.0)
    .help("Audio width either side of the carrier.");
/// The stereo mode.
pub const STEREO_MODE_SETTING: Setting<field::Choice<StereoMode>> =
    Setting::choice("stereo", "Stereo", StereoMode::Auto).help(
        "Auto follows the 25 Hz pilot. Mono plays L + R only. Stereo uses L - R whatever the pilot says.",
    );
/// The squelch.
pub const SQUELCH: Setting<field::Bool> = Setting::new("squelch", "Squelch (4 dB)", "", true)
    .help("Mutes below 2.5 dB C/N, opens above 4 dB.");
/// The station's distance from the dial.
pub const CHANNEL_OFFSET: Setting<field::Hz> =
    Setting::new("channel_offset", "Channel offset", "Hz", 0.0)
        .range(-10000.0, 10000.0, 1.0)
        .help("The station's distance from the dial.");

/// The transition band of the channel filter: flat to the width, then falling over 1.5 kHz.
const CHANNEL_TRANSITION_HZ: f64 = 1500.0;

/// The carrier loop and I/Q demodulation (`broadcast.cquam_demod`, iq at 48 kHz -> real `sum` (L + R), real `diff` (L - R plus the
/// pilot), typed `carrier`).
pub struct CarrierDemod {
    lf: LoopFilter,
    acc: PhaseAcc,
    locked: bool,
    idx: u64,
    amp: f32,
    b_step: Complex32,
    b_prev: Complex32,
    b_mean: Complex32,
    prev_mean: Complex32,
    b_abs: f32,
    b_pow: f32,
    b_cos: f32,
    b_n: u64,
    weak_blocks: u32,
    good_blocks: u32,
    infos: Vec<CarrierInfo>,
}

impl Default for CarrierDemod {
    fn default() -> Self {
        CarrierDemod {
            lf: Self::filter(BN_PULL_HZ),
            acc: PhaseAcc::new(0.0),
            locked: false,
            idx: 0,
            amp: 0.0,
            b_step: Complex32::new(0.0, 0.0),
            b_prev: Complex32::new(0.0, 0.0),
            b_mean: Complex32::new(0.0, 0.0),
            prev_mean: Complex32::new(0.0, 0.0),
            b_abs: 0.0,
            b_pow: 0.0,
            b_cos: 0.0,
            b_n: 0,
            weak_blocks: 0,
            good_blocks: 0,
            infos: Vec::with_capacity(16),
        }
    }
}

impl CarrierDemod {
    fn filter(bn_hz: f64) -> LoopFilter {
        // Valid by construction: both bandwidths are far inside (0, 0.5) per sample.
        LoopFilter::new(bn_hz / RATE, ZETA).unwrap_or_else(|_| LoopFilter::from_gains(0.0, 0.0))
    }
    fn reset(&mut self) {
        let idx = self.idx;
        *self = CarrierDemod::default();
        self.idx = idx;
    }
    fn set_locked(&mut self, locked: bool) {
        self.locked = locked;
        let bn = if locked { BN_LOCKED_HZ } else { BN_PULL_HZ };
        let _ = self.lf.set_bandwidth(bn / RATE, ZETA, 1.0);
    }
    /// The end of a 0.1 s block: pull in, the lock test, and the block's report.
    fn end_block(&mut self) {
        let nb = self.b_n.max(1) as f32;
        let coh = self.b_mean.norm() / self.b_abs.max(1e-20);
        let resid = self.b_step.arg() as f64 / TAU; // cycles per sample left
        if !self.locked {
            // Pull: the loop frequency moves 70 % of the way to the mean phase step.
            if coh < 0.5 {
                let f = self.lf.integrator() + 0.7 * resid;
                self.lf.set_integrator(f);
            }
            // The lock test: the block mean stays put (its drift from the last block), which no programme biases.
            let drift =
                (self.b_mean * self.prev_mean.conj()).arg() as f64 * RATE / (TAU * BLOCK as f64);
            if coh > 0.6 && drift.abs() < 3.0 {
                self.good_blocks += 1;
                if self.good_blocks >= 2 {
                    self.set_locked(true);
                    self.weak_blocks = 0;
                }
            } else {
                self.good_blocks = 0;
            }
        } else if coh < 0.3 {
            self.weak_blocks += 1;
            if self.weak_blocks >= 3 {
                self.set_locked(false);
                self.good_blocks = 0;
            }
        } else {
            self.weak_blocks = 0;
        }
        self.infos.push(CarrierInfo {
            at: self.idx,
            locked: self.locked,
            offset_hz: self.lf.integrator() * RATE,
            amplitude: self.amp.abs(),
            channel_power: self.b_pow / nb,
            cos_phi: self.b_cos / nb,
        });
        self.b_step = Complex32::new(0.0, 0.0);
        self.prev_mean = self.b_mean;
        self.b_mean = Complex32::new(0.0, 0.0);
        self.b_abs = 0.0;
        self.b_pow = 0.0;
        self.b_cos = 0.0;
        self.b_n = 0;
    }
}

impl Component for CarrierDemod {
    fn info(&self) -> ComponentInfo {
        let carrier = Port::typed::<CarrierInfo>("carrier").rate_from(RateFrom::Fixed(0.0));
        ComponentInfo::processor(
            "broadcast.cquam_demod",
            vec![Port::iq("in").rate(RateRule::Exact(RATE))],
            vec![
                Port::real("sum").rate_from(RateFrom::Fixed(RATE)),
                Port::real("diff").rate_from(RateFrom::Fixed(RATE)),
                carrier,
            ],
        )
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        let f = &inputs[0];
        self.reset();
        let mut r = Format::new(Kind::Real, RATE);
        r.bandwidth_hz = f.bandwidth_hz / 2.0;
        Ok(vec![
            r.clone(),
            r,
            Format::new(Kind::typed::<CarrierInfo>(), 0.0),
        ])
    }
    fn process(&mut self, io: &mut Io) {
        let x = io.iq(0);
        let mut sums = std::mem::take(io.real_out(0));
        let mut diffs = std::mem::take(io.real_out(1));
        sums.reserve(x.len());
        diffs.reserve(x.len());
        for &s in x {
            let r = self.acc.derotate(s);
            let mag = r.norm();
            // Carrier amplitude: the mean of the in-phase part.
            self.amp += 2e-4 * (r.re - self.amp);
            let a = self.amp.max(1e-9);
            let re_c = r.re.max(0.05 * mag).max(1e-12);
            let env = mag / a;
            let d_raw = r.im / re_c * env;
            // Phase detector: while pulling in, the angle of the derotated sample; once locked, the decoded L - R (zero mean for
            // any programme, where the mean angle is not: a one-sided stereo signal pulls the carrier phase off).
            let e = if self.locked {
                d_raw.clamp(-1.0, 1.0) as f64
            } else {
                sdrkit_algo_dsp::approx::atan2(r.im, r.re) as f64
            };
            let v = self.lf.update(e / TAU);
            self.acc.steer(v);
            sums.push(if self.locked { env - 1.0 } else { 0.0 });
            diffs.push(if self.locked { d_raw } else { 0.0 });
            self.b_step += r * self.b_prev.conj();
            self.b_prev = r;
            self.b_mean += r;
            self.b_abs += mag;
            self.b_pow += mag * mag;
            self.b_cos += if mag > 0.0 { r.re / mag } else { 0.0 };
            self.b_n += 1;
            self.idx += 1;
            if self.idx.is_multiple_of(BLOCK) {
                self.end_block();
            }
        }
        *io.real_out(0) = sums;
        *io.real_out(1) = diffs;
        let out = io.items_out::<CarrierInfo>(2);
        for i in self.infos.drain(..) {
            out.push(Timed {
                time: i.at as f64 / RATE,
                value: i,
            });
        }
    }
    fn values(&self) -> Values {
        no_values()
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

/// The noise meter (`broadcast.cquam_noise`, iq at 48 kHz -> typed `noise`): the noise power density beside the channel filter (from the
/// width plus 800 Hz to 19 kHz, both sides), from 512-point spectra averaged 8 times, using the median.
pub struct NoiseMeter {
    width: f64,
    fft: Fft,
    win: Vec<f32>,
    buf: Vec<Complex32>,
    acc: Vec<f32>,
    frames: u32,
    idx: u64,
    scratch: Vec<f32>,
    w2: f32,
    infos: Vec<NoiseInfo>,
}

impl Default for NoiseMeter {
    fn default() -> Self {
        let win: Vec<f32> = sdrkit_algo_dsp::window::Window::Hann.coefficients(512);
        let w2 = win.iter().map(|w| w * w).sum();
        NoiseMeter {
            width: 5000.0,
            fft: Fft::forward(512),
            win,
            buf: Vec::with_capacity(512),
            acc: vec![0.0; 512],
            frames: 0,
            idx: 0,
            scratch: Vec::with_capacity(512),
            w2,
            infos: Vec::with_capacity(4),
        }
    }
}

impl NoiseMeter {
    fn analyse(&mut self) {
        self.scratch.clear();
        let bw = RATE / 512.0;
        let lo = self.width + 800.0;
        for (i, &a) in self.acc.iter().enumerate() {
            let f = if i < 256 { i as f64 } else { i as f64 - 512.0 } * bw;
            if f.abs() >= lo && f.abs() <= 19_000.0 {
                self.scratch.push(a / 8.0);
            }
        }
        if !self.scratch.is_empty() {
            let mid = self.scratch.len() / 2;
            let (_, med, _) = self.scratch.select_nth_unstable_by(mid, f32::total_cmp);
            // The median of an average of 8 exponential bins is 0.959 of their mean.
            let mean = *med as f64 / 0.959;
            let density = mean / (self.w2 as f64 * RATE);
            self.infos.push(NoiseInfo {
                at: self.idx,
                density,
            });
        }
        self.acc.iter_mut().for_each(|a| *a = 0.0);
        self.frames = 0;
    }
}

sdrkit_core::settings! {
    /// The settings of [`NoiseMeter`].
    pub enum NoiseMeterSetting {
        /// The audio width either side of the carrier.
        Width = WIDTH,
    }
}

impl Component for NoiseMeter {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "broadcast.cquam_noise",
            vec![Port::iq("in").rate(RateRule::Exact(RATE))],
            vec![Port::typed::<NoiseInfo>("noise").rate_from(RateFrom::Fixed(0.0))],
        )
        .with_settings(NoiseMeterSetting::schema())
    }
    fn configure(&mut self, _inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        self.buf.clear();
        self.frames = 0;
        self.acc.iter_mut().for_each(|a| *a = 0.0);
        Ok(vec![Format::new(Kind::typed::<NoiseInfo>(), 0.0)])
    }
    fn process(&mut self, io: &mut Io) {
        let x = io.iq(0);
        for &s in x {
            self.buf.push(s);
            self.idx += 1;
            if self.buf.len() == 512 {
                for (b, w) in self.buf.iter_mut().zip(&self.win) {
                    *b *= *w;
                }
                self.fft.process(&mut self.buf);
                for (a, b) in self.acc.iter_mut().zip(&self.buf) {
                    *a += b.norm_sqr();
                }
                self.buf.clear();
                self.frames += 1;
                if self.frames == 8 {
                    self.analyse();
                }
            }
        }
        let out = io.items_out::<NoiseInfo>(0);
        for i in self.infos.drain(..) {
            out.push(Timed {
                time: i.at as f64 / RATE,
                value: i,
            });
        }
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        match NoiseMeterSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            NoiseMeterSetting::Width => self.width = WIDTH.read(id, v)?,
        }
        Ok(Applied::Live)
    }
    fn values(&self) -> Values {
        values_of([(NoiseMeterSetting::Width, Value::Hz(self.width))])
    }
}

/// The pilot detector (`broadcast.cquam_pilot`, real diff at 1 kHz -> typed `pilot`): every 0.5 s a Goertzel analysis at 0 to
/// 60 Hz over the last second (Hann window). The pilot is present when the 25 Hz bin is 12 dB above the mean of the four bins
/// beside it and at least 1 % of the carrier.
pub struct PilotDetector {
    /// The last second at 1 kHz, a ring written at `head`.
    ring: Vec<f32>,
    head: usize,
    filled: usize,
    line: Vec<f32>,
    n1k: u64,
    hits: u32,
    misses: u32,
    stereo: bool,
    win: Vec<f32>,
    wsum: f32,
    infos: Vec<PilotInfo>,
}

impl Default for PilotDetector {
    fn default() -> Self {
        let win = sdrkit_algo_dsp::window::Window::Hann.coefficients(PILOT_WINDOW);
        let wsum = win.iter().sum();
        PilotDetector {
            ring: vec![0.0; PILOT_WINDOW],
            head: 0,
            filled: 0,
            line: vec![0.0; PILOT_WINDOW],
            n1k: 0,
            hits: 0,
            misses: 0,
            stereo: false,
            win,
            wsum,
            infos: Vec::with_capacity(4),
        }
    }
}

impl PilotDetector {
    /// The amplitude of each 1 Hz bin from 0 to 60 Hz of the last second (relative to full scale 1 = the carrier).
    fn analyse(&mut self) -> [f32; 61] {
        // The ring in time order.
        let (old, new) = self.ring.split_at(self.head);
        self.line[..new.len()].copy_from_slice(new);
        self.line[new.len()..].copy_from_slice(old);
        let mut amps = [0f32; 61];
        for (f, a) in amps.iter_mut().enumerate() {
            let x = goertzel_windowed(&self.line, &self.win, f as f64 / 1000.0);
            *a = (x.norm() * 2.0 / self.wsum as f64) as f32;
        }
        amps
    }
    fn check(&mut self) {
        let amps = self.analyse();
        let p = |f: usize| (amps[f] as f64).powi(2);
        // The four bins beside the window's main lobe (±1 Hz still holds the pilot through the Hann window).
        let neigh = (p(22) + p(23) + p(27) + p(28)) / 4.0;
        let ratio_db = (10.0 * (p(25) / neigh.max(1e-30)).log10()) as f32;
        let corrected = (p(25) - neigh).max(0.0).sqrt();
        let hit = ratio_db >= 12.0 && corrected >= 0.01;
        if hit {
            self.hits += 1;
            self.misses = 0;
            if self.hits >= 2 {
                self.stereo = true;
            }
        } else {
            self.misses += 1;
            self.hits = 0;
            if self.misses >= 3 {
                self.stereo = false;
            }
        }
        let mut spectrum = [0f32; 61];
        for (s, a) in spectrum.iter_mut().zip(amps) {
            *s = 20.0 * a.max(1e-10).log10();
        }
        self.infos.push(PilotInfo {
            at: self.n1k * 48,
            hit,
            stereo: self.stereo,
            level_db: ratio_db,
            percent: (corrected * 100.0) as f32,
            spectrum,
        });
    }
}

impl Component for PilotDetector {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "broadcast.cquam_pilot",
            vec![Port::real("diff").rate(RateRule::Exact(PILOT_RATE))],
            vec![Port::typed::<PilotInfo>("pilot").rate_from(RateFrom::Fixed(0.0))],
        )
    }
    fn configure(&mut self, _inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        self.head = 0;
        self.filled = 0;
        self.n1k = 0;
        self.hits = 0;
        self.misses = 0;
        self.stereo = false;
        Ok(vec![Format::new(Kind::typed::<PilotInfo>(), 0.0)])
    }
    fn process(&mut self, io: &mut Io) {
        for &x in io.real(0) {
            self.ring[self.head] = x;
            self.head = (self.head + 1) % PILOT_WINDOW;
            self.filled = (self.filled + 1).min(PILOT_WINDOW);
            self.n1k += 1;
            if self.n1k.is_multiple_of(500) && self.filled == PILOT_WINDOW {
                self.check();
            }
        }
        let out = io.items_out::<PilotInfo>(0);
        for i in self.infos.drain(..) {
            out.push(Timed {
                time: i.at as f64 / RATE,
                value: i,
            });
        }
    }
    fn values(&self) -> Values {
        no_values()
    }
}

/// The matrix, high-pass and squelch (`broadcast.cquam_matrix`, real sum + real diff + typed carrier, noise and pilot -> audio at
/// 48 kHz), and the stack's results.
pub struct StereoMatrix {
    mode: StereoMode,
    squelch: bool,
    width: f64,
    idx: u64,
    gain: f32,
    hp_l: Biquad,
    hp_r: Biquad,
    carrier: Option<CarrierInfo>,
    density: Option<f64>,
    pilot: Option<PilotInfo>,
    open: bool,
    cn_db: f32,
    pend_c: VecDeque<CarrierInfo>,
    pend_n: VecDeque<NoiseInfo>,
    pend_p: VecDeque<PilotInfo>,
    sl: f64,
    sr: f64,
    sd: f64,
    ss: f64,
    meter_n: u64,
    peak: f32,
    peak_hold: f32,
    peak_n: u64,
    checks_good: u64,
    checks_bad: u64,
    published: Published,
    spectrum: SeriesId,
    status: Status,
}

impl Default for StereoMatrix {
    fn default() -> Self {
        StereoMatrix {
            mode: StereoMode::Auto,
            squelch: true,
            width: 5000.0,
            idx: 0,
            gain: 0.0,
            hp_l: Self::highpass(),
            hp_r: Self::highpass(),
            carrier: None,
            density: None,
            pilot: None,
            open: false,
            cn_db: -100.0,
            pend_c: VecDeque::with_capacity(8),
            pend_n: VecDeque::with_capacity(8),
            pend_p: VecDeque::with_capacity(8),
            sl: 0.0,
            sr: 0.0,
            sd: 0.0,
            ss: 0.0,
            meter_n: 0,
            peak: 0.0,
            peak_hold: 0.0,
            peak_n: 0,
            checks_good: 0,
            checks_bad: 0,
            published: Published::from_decl(Arc::new(
                ResultsDecl::new()
                    .with_fields(CquamFields::SHAPE.fields)
                    .with_series(&PILOT_SPECTRUM),
            )),
            spectrum: SeriesId(0),
            status: Status::new(LockState::Off),
        }
    }
}

impl StereoMatrix {
    fn highpass() -> Biquad {
        Biquad::new(BiquadCoeffs::highpass(
            RATE,
            50.0,
            std::f64::consts::FRAC_1_SQRT_2,
        ))
    }
    fn locked(&self) -> bool {
        self.carrier.as_ref().is_some_and(|c| c.locked)
    }
    fn stereo_now(&self) -> bool {
        match self.mode {
            StereoMode::Mono => false,
            StereoMode::Stereo => self.locked(),
            StereoMode::Auto => self.locked() && self.pilot.as_ref().is_some_and(|p| p.stereo),
        }
    }
    fn update_cn(&mut self) {
        if let (Some(c), Some(d)) = (&self.carrier, self.density) {
            let n = d * 2.0 * self.width;
            self.cn_db = (10.0 * (c.channel_power as f64 / n.max(1e-30)).log10()) as f32;
            if self.cn_db < 2.5 {
                self.open = false;
            } else if self.cn_db > 4.0 {
                self.open = true;
            }
        }
    }
    fn publish(&mut self) {
        let locked = self.locked();
        let stereo = self.stereo_now();
        let carrier = self.carrier.as_ref();
        let (offset, amplitude) = carrier.map_or((0.0, 0.0), |c| (c.offset_hz, c.amplitude));
        let carrier_dbfs = 20.0 * amplitude.max(1e-10).log10();
        let (pseen, ppct) = self
            .pilot
            .as_ref()
            .map_or((false, 0.0), |p| (p.stereo, p.percent));
        let pilot_seen = pseen && locked;
        let measured = self.meter_n > 0;
        let n = self.meter_n.max(1) as f64;
        let level = |sum: f64| measured.then(|| 10.0 * (sum / n).max(1e-20).log10());
        let fields = CquamFields {
            offset_hz: carrier.map(|c| Hertz(c.offset_hz)),
            cn_db: (carrier.is_some() && self.density.is_some()).then_some(f64::from(self.cn_db)),
            carrier_dbfs: carrier.map(|_| f64::from(carrier_dbfs)),
            stereo,
            pilot_seen,
            pilot_db: self.pilot.as_ref().map(|p| f64::from(p.level_db)),
            pilot_pct: self.pilot.as_ref().map(|p| f64::from(p.percent)),
            lock_quality: carrier.map(|c| f64::from(c.cos_phi)),
            left_dbfs: level(self.sl),
            right_dbfs: level(self.sr),
            width_db: (stereo && measured)
                .then(|| 10.0 * ((self.sd / n).max(1e-20) / (self.ss / n).max(1e-20)).log10()),
            mod_peak_pct: f64::from(self.peak_hold.max(self.peak) * 100.0),
            stereo_mode: self.mode,
            filter_width: Hertz(self.width),
            noise_dbfs: self
                .density
                .map(|x| 10.0 * (x * 2.0 * self.width).max(1e-30).log10()),
            pilot_checks_ok: self.checks_good as i64,
            pilot_checks_bad: self.checks_bad as i64,
            signal_time: Seconds(self.idx as f64 / RATE),
        };
        self.published.put(&fields);
        self.sl = 0.0;
        self.sr = 0.0;
        self.sd = 0.0;
        self.ss = 0.0;
        self.meter_n = 0;
        let cn_db = self.cn_db;
        let st = &mut self.status;
        st.lock_state = if !locked {
            LockState::Searching
        } else if stereo {
            LockState::Locked
        } else {
            LockState::Tracking
        };
        st.snr_db = Some(cn_db);
        st.level_dbfs = Some(carrier_dbfs);
        st.offset_hz = Some(offset);
        st.summary.clear();
        use std::fmt::Write;
        // Writing into a String cannot fail.
        let _ = if !locked {
            write!(st.summary, "AM stereo: no carrier yet")
        } else if stereo {
            write!(
                st.summary,
                "C-QUAM stereo, carrier {offset:+.0} Hz, C/N {cn_db:.0} dB, pilot {ppct:.1} %"
            )
        } else {
            write!(
                st.summary,
                "AM mono, carrier {offset:+.0} Hz, C/N {cn_db:.0} dB{}",
                if pilot_seen { "" } else { ", no 25 Hz pilot" }
            )
        };
        let r = self.published.results_mut();
        r.set_status(&self.status);
        if self.pilot.is_none() {
            r.update_series(self.spectrum, |s| {
                let changed = !s.points.is_empty();
                s.points.clear();
                changed
            });
        }
        if let Some(p) = &self.pilot {
            r.update_series(self.spectrum, |s| {
                let changed = !s.line
                    || s.points.len() != p.spectrum.len()
                    || s.points.iter().zip(&p.spectrum).any(|(a, &b)| a[1] != b);
                if changed {
                    s.line = true;
                    s.points.clear();
                    s.points
                        .extend(p.spectrum.iter().enumerate().map(|(i, &v)| [i as f32, v]));
                }
                changed
            });
        }
    }
    fn clear_signal(&mut self) {
        self.carrier = None;
        self.pilot = None;
        self.density = None;
        self.pend_c.clear();
        self.pend_n.clear();
        self.pend_p.clear();
        self.open = false;
        self.gain = 0.0;
    }
}

sdrkit_core::settings! {
    /// The settings of [`StereoMatrix`].
    pub enum MatrixSetting {
        /// The stereo mode.
        Stereo = STEREO_MODE_SETTING,
        /// The squelch.
        Squelch = SQUELCH,
        /// The audio width either side of the carrier.
        Width = WIDTH,
    }
}

impl Component for StereoMatrix {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "broadcast.cquam_matrix",
            vec![
                Port::real("sum").rate(RateRule::Exact(RATE)),
                Port::real("diff").rate(RateRule::Exact(RATE)),
                Port::typed::<CarrierInfo>("carrier"),
                Port::typed::<NoiseInfo>("noise"),
                Port::typed::<PilotInfo>("pilot"),
            ],
            vec![Port::audio("audio")
                .rate_from(RateFrom::Fixed(RATE))
                .channels_from(ChannelsFrom::Fixed(2))],
        )
        .with_settings(MatrixSetting::schema())
    }
    fn configure(&mut self, _inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        let mut f = Format::audio(RATE, 2);
        f.bandwidth_hz = self.width;
        Ok(vec![f])
    }
    fn process(&mut self, io: &mut Io) {
        let sum = io.real(0);
        let diff = io.real(1);
        self.pend_c
            .extend(io.items::<CarrierInfo>(2).iter().map(|t| t.value.clone()));
        self.pend_n
            .extend(io.items::<NoiseInfo>(3).iter().map(|t| t.value.clone()));
        for p in io.items::<PilotInfo>(4) {
            if p.value.hit {
                self.checks_good += 1;
            } else {
                self.checks_bad += 1;
            }
            self.pend_p.push_back(p.value.clone());
        }
        let n = sum.len().min(diff.len());
        let out = io.audio_out(0);
        out.samples.reserve(2 * n);
        let step = 1.0 / (0.040 * RATE as f32);
        for i in 0..n {
            let idx = self.idx;
            // Apply the decisions due at this sample.
            while self.pend_c.front().is_some_and(|c| c.at <= idx) {
                self.carrier = self.pend_c.pop_front();
                self.update_cn();
            }
            while self.pend_n.front().is_some_and(|c| c.at <= idx) {
                self.density = self.pend_n.pop_front().map(|n| n.density);
                self.update_cn();
            }
            while self.pend_p.front().is_some_and(|c| c.at <= idx) {
                self.pilot = self.pend_p.pop_front();
            }
            // The quadrature gain fades over 40 ms at a switch.
            let target = if self.stereo_now() { 1.0 } else { 0.0 };
            if self.gain < target {
                self.gain = (self.gain + step).min(target);
            } else if self.gain > target {
                self.gain = (self.gain - step).max(target);
            }
            let s = sum[i];
            let d = diff[i];
            let g = self.gain;
            let l = s * (1.0 - 0.5 * g) + 0.5 * g * d;
            let r = s * (1.0 - 0.5 * g) - 0.5 * g * d;
            let l = self.hp_l.process_sample(if g == 0.0 { s } else { l });
            let r = if g == 0.0 {
                // Identical channels in mono.
                self.hp_r.process_sample(s);
                l
            } else {
                self.hp_r.process_sample(r)
            };
            let audible = self.locked() && (!self.squelch || self.open);
            let (lo, ro) = if audible { (l, r) } else { (0.0, 0.0) };
            out.push_stereo(lo, ro);
            self.sl += (lo * lo) as f64;
            self.sr += (ro * ro) as f64;
            self.sd += ((lo - ro) * (lo - ro)) as f64;
            self.ss += ((lo + ro) * (lo + ro)) as f64;
            self.meter_n += 1;
            if self.locked() {
                self.peak = self.peak.max(s.abs());
            }
            self.peak_n += 1;
            if self.peak_n.is_multiple_of(RATE as u64) {
                self.peak_hold = self.peak;
                self.peak = 0.0;
            }
            self.idx += 1;
            if self.idx.is_multiple_of(BLOCK) {
                self.publish();
            }
        }
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        match MatrixSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            MatrixSetting::Stereo => self.mode = STEREO_MODE_SETTING.read(id, v)?,
            MatrixSetting::Squelch => self.squelch = SQUELCH.read(id, v)?,
            MatrixSetting::Width => self.width = WIDTH.read(id, v)?,
        }
        Ok(Applied::Live)
    }
    fn values(&self) -> Values {
        values_of([
            (MatrixSetting::Stereo, Value::Choice(self.mode.index())),
            (MatrixSetting::Squelch, Value::Bool(self.squelch)),
            (MatrixSetting::Width, Value::Hz(self.width)),
        ])
    }
    fn results(&self) -> Option<&Results> {
        Some(self.published.results())
    }
    fn commands(&self) -> &Commands {
        ClearCommand::commands()
    }
    fn invoke(&mut self, id: CommandId, _args: &Values) -> Result<Option<Record>, CommandError> {
        match ClearCommand::from_id(id).ok_or(CommandError::NoSuchId { id })? {
            ClearCommand::Clear => self.clear_signal(),
        }
        Ok(None)
    }
    /// Every result is a measurement of the signal: the carrier, the pilot, the levels and the pilot checks start
    /// again, and the results say so at once.
    fn forget(&mut self, _kept: &Kept) {
        self.clear_signal();
        self.checks_good = 0;
        self.checks_bad = 0;
        self.peak = 0.0;
        self.peak_hold = 0.0;
        self.sl = 0.0;
        self.sr = 0.0;
        self.sd = 0.0;
        self.ss = 0.0;
        self.meter_n = 0;
        self.publish();
    }
}

/// The front end (`broadcast.cquam_front`): the standard channel component to exactly 48 kHz, 14 kHz either side of the
/// carrier. It publishes the input rate.
pub struct Front {
    channel: Channel,
    published: Published,
}

impl Front {
    /// The front end, or the refusal of its channel, named by its label.
    fn new() -> Result<Front, GraphError> {
        Ok(Front {
            channel: GraphError::checked("front", crate::component::channel(14_000.0, RATE))?,
            published: Published::new(CquamFrontFields::SHAPE.fields),
        })
    }
}

impl Component for Front {
    fn info(&self) -> ComponentInfo {
        let mut i = self.channel.info();
        i.name = "broadcast.cquam_front".into();
        i
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        let o = self.channel.configure(inputs)?;
        self.published.put(&CquamFrontFields {
            input_rate: Hertz(inputs[0].rate),
        });
        Ok(o)
    }
    fn process(&mut self, io: &mut Io) {
        self.channel.process(io)
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        self.channel.apply(id, v)
    }
    fn values(&self) -> Values {
        self.channel.values()
    }
    fn results(&self) -> Option<&Results> {
        Some(self.published.results())
    }
}

/// The C-QUAM graph.
///
/// # Errors
///
/// [`GraphError`] if the graph does not build: a fault of this crate, never of the input.
pub fn graph(catalog: &Arc<Catalog>) -> Result<Graph, GraphError> {
    GraphBuilder::new()
        .add("front", Front::new()?)
        .add(
            "channel",
            GraphError::checked(
                "channel",
                FirFilter::lowpass(WIDTH.default, CHANNEL_TRANSITION_HZ),
            )?,
        )
        .add("demod", CarrierDemod::default())
        .add("noise", NoiseMeter::default())
        .add("pilot", PilotDetector::default())
        .add("matrix", StereoMatrix::default())
        .input("front")
        .connect("front", "channel")
        .connect("front", "noise")
        .connect("channel", "demod")
        .connect("demod.sum", "matrix.sum")
        .connect("demod.diff", "matrix.diff")
        .connect("demod.diff", "pilot")
        .connect("demod.carrier", "matrix.carrier")
        .connect("noise", "matrix.noise")
        .connect("pilot", "matrix.pilot")
        .output("matrix")
        .status_from("matrix")
        .reconfigure_on_retune()
        .catalog(catalog)
        .build()
}

/// The C-QUAM receiver: its graph in a [`StackGraph`]. A retune starts the loop, the pilot detector and the noise estimate again.
///
/// # Errors
///
/// [`GraphError::NamedSetting`] if `cfg` holds a value that a component refuses, or another [`GraphError`] if the graph does not
/// build.
pub fn receiver(cfg: &StackConfig) -> Result<StackGraph, GraphError> {
    StackGraph::of(crate::typed::shape(ID), graph(&Catalog::of(cfg)?)?)
        .min_rate(
            MIN_RATE,
            "AM stereo (C-QUAM) needs a sample rate of at least 250 ksps",
        )
        .route("stereo", &["matrix.stereo"])?
        .route("squelch", &["matrix.squelch"])?
        .route("width", &["channel.high", "noise.width", "matrix.width"])?
        .route("channel_offset", &["front.offset"])?
        .command("clear", sdrkit_graph::CommandTarget::every("clear"))
        .on_retune(sdrkit_graph::CommandTarget::of("matrix", "clear"))?
        .with_settings(&cfg.settings)
        .build()
}

sdrkit_core::choice_enum! {
    /// What the test signal sends.
    #[derive(Default)]
    pub enum GenKind {
        /// C-QUAM stereo.
        #[default]
        Stereo = ("stereo", "C-QUAM stereo"),
        /// Plain AM, mono, no pilot.
        PlainAm = ("am", "Plain AM (mono, no pilot)"),
        /// No signal (the noise of the framework alone).
        NoiseOnly = ("noise", "Noise only"),
    }
}

/// The generator's options. Noise, offsets, fading and the level come from the framework's `gen_*` settings.
#[derive(Clone, Debug)]
pub struct GenOptions {
    /// Signal kind.
    pub kind: GenKind,
    /// Left silent (separation check).
    pub left_silent: bool,
    /// Left tone, a whole number of Hz (the right is 1.5 times it).
    pub left_hz: f64,
    /// The position of the dial in the samples (the tune offset below the centre when the rate allows it).
    pub centre_offset_hz: f64,
}

impl Default for GenOptions {
    fn default() -> Self {
        GenOptions {
            kind: GenKind::Stereo,
            left_silent: false,
            left_hz: 440.0,
            centre_offset_hz: 0.0,
        }
    }
}

/// The clean C-QUAM test signal: a carrier of amplitude 1, left and right tones of 0.35 each, the 25 Hz pilot at 5 %.
pub struct CquamGenerator {
    rate: f64,
    o: GenOptions,
    n: u64,
}

impl CquamGenerator {
    /// A generator at `rate`.
    pub fn new(rate: f64, o: GenOptions) -> CquamGenerator {
        CquamGenerator { rate, o, n: 0 }
    }
    /// From a generator configuration (the stack's `gen_*` test signal settings).
    pub fn from_config(cfg: &GenConfig) -> CquamGenerator {
        let s = &cfg.settings;
        let o = GenOptions {
            kind: s.get(&GEN_KIND),
            left_silent: s.get(&GEN_LEFT_SILENT),
            left_hz: s.get(&GEN_LEFT_HZ),
            centre_offset_hz: default_centre_offset(cfg.rate_hz),
        };
        CquamGenerator::new(cfg.rate_hz, o)
    }
}

/// Where the generator puts the dial in its samples: the tune offset below the centre when the rate allows it, else the centre.
pub fn default_centre_offset(rate: f64) -> f64 {
    if rate >= 4.0 * TUNE_OFFSET_HZ {
        -TUNE_OFFSET_HZ
    } else {
        0.0
    }
}

impl Generator for CquamGenerator {
    fn rate_hz(&self) -> f64 {
        self.rate
    }
    fn generate(&mut self, out: &mut [Complex32]) -> usize {
        let o = &self.o;
        let lf = o.left_hz.round();
        let rf = (1.5 * lf).round();
        // The generator's time wraps every 1000 s: every tone is a whole number of Hz, so the wrap leaves no step.
        let wrap = (1000.0 * self.rate) as u64;
        for s in out.iter_mut() {
            let t = (self.n % wrap) as f64 / self.rate;
            let lv = if o.left_silent {
                0.0
            } else {
                0.35 * (TAU * lf * t).sin()
            };
            let rv = 0.35 * (TAU * rf * t).sin();
            let pilot = 0.05 * (TAU * 25.0 * t).sin();
            let carrier_ph = TAU * ((o.centre_offset_hz * t) % 1.0);
            *s = match o.kind {
                GenKind::Stereo => {
                    // The QUAM vector I + jQ with its envelope corrected to I: (1 + L + R) e^(j atan2(L - R + p, 1 + L + R)).
                    let i = 1.0 + lv + rv;
                    let q = lv - rv + pilot;
                    Complex32::from_polar(i as f32, (q.atan2(i) + carrier_ph) as f32)
                }
                GenKind::PlainAm => {
                    Complex32::from_polar((1.0 + 0.5 * (lv + rv)) as f32, carrier_ph as f32)
                }
                GenKind::NoiseOnly => Complex32::new(0.0, 0.0),
            };
            self.n += 1;
        }
        out.len()
    }
    fn signal_info(&self) -> Option<SignalInfo> {
        // Mean power: the carrier (1) and the two tones (0.35 rms / sqrt 2 each); plain AM sends half of each tone. The C/N of the
        // specification is stated in 10 kHz.
        let tone = 0.35f64 * 0.35 / 2.0;
        let power = match self.o.kind {
            GenKind::Stereo => 1.0 + if self.o.left_silent { 1.0 } else { 2.0 } * tone,
            GenKind::PlainAm => 1.0 + 0.25 * if self.o.left_silent { 1.0 } else { 2.0 } * tone,
            GenKind::NoiseOnly => 1.0,
        };
        Some(SignalInfo::new(10e3).with_power(power))
    }
}

/// Test signal: what it sends.
pub const GEN_KIND: Setting<field::Choice<GenKind>> =
    Setting::choice("gen_kind", "Test signal", GenKind::Stereo)
        .section("Test signal")
        .advanced();
/// Test signal: the left channel is silent.
pub const GEN_LEFT_SILENT: Setting<field::Bool> =
    Setting::new("gen_left_silent", "Left silent", "", false)
        .section("Test signal")
        .advanced();
/// Test signal: the left tone.
pub const GEN_LEFT_HZ: Setting<field::Hz> = Setting::new("gen_left_hz", "Left tone", "Hz", 440.0)
    .range(100.0, 4000.0, 1.0)
    .section("Test signal")
    .advanced();

sdrkit_core::settings! {
    /// The settings of the C-QUAM stack: the receiver's, then the test signal's.
    pub enum CquamSetting {
        /// The stereo mode.
        Stereo = STEREO_MODE_SETTING,
        /// The audio width.
        Width = WIDTH,
        /// The squelch.
        Squelch = SQUELCH,
        /// The station's distance from the dial.
        ChannelOffset = CHANNEL_OFFSET,
        /// Test signal: what it sends.
        GenKind = GEN_KIND,
        /// Test signal: the left channel is silent.
        GenLeftSilent = GEN_LEFT_SILENT,
        /// Test signal: the left tone.
        GenLeftHz = GEN_LEFT_HZ,
    }
}

/// Makes the receiver.
pub fn make_receiver(cfg: &StackConfig) -> Result<Box<dyn Receiver>, ConfigError> {
    Ok(Box::new(receiver(cfg)?))
}
/// Makes the generator.
pub fn make_generator(cfg: &GenConfig) -> Result<Box<dyn Generator>, ConfigError> {
    Ok(Box::new(CquamGenerator::from_config(cfg)))
}
