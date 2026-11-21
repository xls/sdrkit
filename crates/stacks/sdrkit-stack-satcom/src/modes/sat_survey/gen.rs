//! The test signal of the survey, by method, in complex noise of standard deviation [`NOISE_SIGMA`] per part:
//!
//! - carrier: one carrier at the carrier offset + 3000 tanh((300 − t) / 120) Hz, its S/N given in 1 kHz;
//! - channel: three keyed channels at +3, −7 and +12 grid steps, on for 1.5, 2.0 and 1.0 s every 4, 7 and 3 s, each frequency
//!   modulated by ±800 Hz at 600 Hz, their S/N given in 1 kHz (a channel beyond 0.45 of the rate is left out);
//! - frame gap: random QPSK on 32 subcarriers in symbols of 1056 samples at 240 Msps, rescaled to the rate, 302 symbols a
//!   frame and then a gap, frames at the submode's frame rate; S/N over the whole rate;
//! - repeated sequence: random QPSK, one symbol a sample, whose first 40 µs of every period is the same sequence; S/N over
//!   the whole rate.
//!
//! The station is at the centre of the samples ([`Generator::station_offset_hz`] is 0). The stated power is the mean power
//! of the output, so a source that scales the signal to its stated power keeps the noise many 8-bit steps wide.

use super::{params_of, Method, Params};
use sdrkit_algo_sim::rng::Rng;
use sdrkit_core::publish::field;
use sdrkit_core::{Complex32, ConfigError, GenConfig, Generator, Hz, Setting, SignalInfo};
use std::f64::consts::TAU;

/// The test signal is on (off: noise only).
pub const GEN_SIGNAL: Setting<field::Bool> = Setting::new("gen_signal", "Test signal", "", true)
    .help("The test signal holds the signal of the submode's method; off, noise only.")
    .section("Test signal")
    .advanced();
/// The S/N of the test signal.
pub const GEN_LEVEL_DB: Setting<field::F64> =
    Setting::new("gen_level_db", "Test signal S/N", "dB", 0.0)
        .range(-30.0, 60.0, 0.0)
        .zero_means("the method's own")
        .help(
            "Carrier and channel methods: S/N in 1 kHz (0 gives 18 and 30 dB). Frame methods: S/N over the whole rate \
             (0 gives 0 dB).",
        )
        .section("Test signal")
        .advanced();
/// The carrier offset of the test signal.
pub const GEN_CARRIER_HZ: Setting<field::Hz> = Setting::new(
    "gen_carrier_hz",
    "Test signal carrier offset",
    "Hz",
    20e3,
)
.range(-1e6, 1e6, 0.0)
.help(
    "Where the carrier of the carrier method sits, from the centre, before its slow Doppler curve.",
)
.section("Test signal")
.advanced();

/// The noise, standard deviation per part.
pub const NOISE_SIGMA: f64 = 0.05;
/// The Doppler curve of the carrier: amplitude Hz, centre s, time constant s.
pub const CURVE: (f64, f64, f64) = (3000.0, 300.0, 120.0);
/// The channels: grid steps, on time s, period s.
pub const CHANNELS: [(i32, f64, f64); 3] = [(3, 1.5, 4.0), (-7, 2.0, 7.0), (12, 1.0, 3.0)];
/// The frequency modulation of a channel: deviation Hz, tone Hz.
pub const CHANNEL_FM: (f64, f64) = (800.0, 600.0);
/// Channels lie within this share of the rate.
pub const BAND_SHARE: f64 = 0.45;
/// The frame signal: subcarriers, samples of a symbol at [`FRAME_SYMBOL_RATE`], symbols of a frame.
pub const FRAME_OFDM: (usize, f64, u32) = (32, 1056.0, 302);
/// The rate the frame symbol length refers to, Hz.
pub const FRAME_SYMBOL_RATE: f64 = 240e6;
/// The subcarriers fill at most this share of the rate.
pub const FRAME_FILL: f64 = 0.8;
/// The repeated sequence, s.
pub const SEQUENCE_S: f64 = 40e-6;
/// The noise bandwidth of the carrier and channel S/N, Hz.
pub const SNR_BAND_HZ: f64 = 1000.0;
/// The S/N when [`GEN_LEVEL_DB`] is 0, by method: carrier, channel, frame gap, repeated sequence.
pub const DEFAULT_LEVEL_DB: [f64; 4] = [18.0, 30.0, 0.0, 0.0];

/// What the test signal holds.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GenParams {
    /// The signal is on (off: noise only).
    pub signal: bool,
    /// The S/N, dB (see [`GEN_LEVEL_DB`]; here 0 is 0 dB).
    pub level_db: f64,
    /// The carrier offset, Hz.
    pub carrier_hz: f64,
}

impl GenParams {
    /// The parameters of `method` at its own S/N.
    pub fn of(method: Method) -> Self {
        GenParams {
            signal: true,
            level_db: DEFAULT_LEVEL_DB[method as usize],
            carrier_hz: GEN_CARRIER_HZ.default,
        }
    }
}

/// QPSK of unit power from two random bits.
fn qpsk(rng: &mut Rng) -> Complex32 {
    let a = std::f32::consts::FRAC_1_SQRT_2;
    let re = if rng.bit() == 1 { a } else { -a };
    let im = if rng.bit() == 1 { a } else { -a };
    Complex32::new(re, im)
}

/// A unit phasor that turns by `step` radians a sample.
#[derive(Clone, Copy)]
struct Phasor {
    z: Complex32,
    step: Complex32,
    count: u32,
}

impl Phasor {
    fn new(step: f64) -> Self {
        Phasor {
            z: Complex32::new(1.0, 0.0),
            step: Complex32::new(step.cos() as f32, step.sin() as f32),
            count: 0,
        }
    }
    fn next(&mut self) -> Complex32 {
        let v = self.z;
        self.z *= self.step;
        self.count += 1;
        if self.count == 1024 {
            self.z /= self.z.norm();
            self.count = 0;
        }
        v
    }
}

/// The test signal (module documentation).
pub struct SatSurveyGenerator {
    rate: f64,
    p: &'static Params,
    g: GenParams,
    rng: Rng,
    data: Rng,
    n: u64,
    amp: f64,
    power: f64,
    // Carrier and channels.
    phase: f64,
    channels: Vec<(f64, f64, f64, f64)>,
    // Frame gap.
    coef: Vec<Complex32>,
    symbol: Option<(u64, u32)>,
    sub: Phasor,
    base: Phasor,
    // Repeated sequence.
    period: usize,
    pos: usize,
    sequence: Vec<Complex32>,
}

impl SatSurveyGenerator {
    /// The signal of submode `p` at `rate` with `g`, repeatable by `seed`.
    pub fn new(rate: f64, p: &'static Params, g: GenParams, seed: u64) -> Self {
        let noise = 2.0 * NOISE_SIGMA * NOISE_SIGMA;
        let snr = 10f64.powf(g.level_db / 10.0);
        let mut data = Rng::stream(seed, 2);
        let mut mean = 0.0;
        let mut channels = Vec::new();
        let amp = match p.method {
            Method::Carrier | Method::Channel => (snr * noise * SNR_BAND_HZ / rate).sqrt(),
            Method::FrameGap | Method::Repeat => (snr * noise).sqrt(),
        };
        match p.method {
            Method::Carrier | Method::FrameGap | Method::Repeat => mean = amp * amp,
            Method::Channel => {
                for &(steps, on, every) in &CHANNELS {
                    let f = f64::from(steps) * p.grid_hz;
                    if f.abs() + CHANNEL_FM.0 <= BAND_SHARE * rate {
                        channels.push((f, on, every, 0.0));
                        mean += amp * amp * on / every;
                    }
                }
            }
        }
        let (subs, sym_len, _) = FRAME_OFDM;
        let ts = sym_len / FRAME_SYMBOL_RATE;
        let spacing = (1.0 / ts).min(FRAME_FILL * rate / subs as f64);
        let period = (rate / p.frame_hz.max(1.0)).round().max(1.0) as usize;
        let seq_len = ((SEQUENCE_S * rate).round() as usize).clamp(1, period);
        let sequence = (0..seq_len).map(|_| qpsk(&mut data)).collect();
        SatSurveyGenerator {
            rate,
            p,
            g,
            rng: Rng::stream(seed, 1),
            data,
            n: 0,
            amp,
            power: noise + if g.signal { mean } else { 0.0 },
            phase: 0.0,
            channels,
            coef: vec![Complex32::default(); subs],
            symbol: None,
            sub: Phasor::new(TAU * spacing / rate),
            base: Phasor::new(-TAU * (subs as f64 - 1.0) / 2.0 * spacing / rate),
            period,
            pos: 0,
            sequence,
        }
    }

    /// The generator of `cfg`: its submode (the stack's default when none or unknown) and its `gen_*` settings.
    ///
    /// # Errors
    ///
    /// None at present; the result keeps the form of the other generators.
    pub fn from_config(cfg: &GenConfig) -> Result<Self, ConfigError> {
        let p = params_of(cfg.submode.as_deref());
        let v = &cfg.settings;
        let set = v.get(&GEN_LEVEL_DB);
        let g = GenParams {
            signal: v.get(&GEN_SIGNAL),
            level_db: if set == 0.0 {
                DEFAULT_LEVEL_DB[p.method as usize]
            } else {
                set
            },
            carrier_hz: v.get(&GEN_CARRIER_HZ),
        };
        Ok(Self::new(cfg.rate_hz, p, g, cfg.seed))
    }

    /// The carrier offset at time `t`, Hz.
    pub fn carrier_at(&self, t: f64) -> f64 {
        self.g.carrier_hz + CURVE.0 * ((CURVE.1 - t) / CURVE.2).tanh()
    }

    /// The channel offsets of the signal, Hz.
    pub fn channel_offsets(&self) -> Vec<f64> {
        self.channels.iter().map(|c| c.0).collect()
    }

    fn signal(&mut self, t: f64) -> Complex32 {
        let a = self.amp;
        match self.p.method {
            Method::Carrier => {
                self.phase = (self.phase + TAU * self.carrier_at(t) / self.rate).rem_euclid(TAU);
                Complex32::new((a * self.phase.cos()) as f32, (a * self.phase.sin()) as f32)
            }
            Method::Channel => {
                let mut y = Complex32::default();
                let fm = CHANNEL_FM.0 / CHANNEL_FM.1 * (TAU * CHANNEL_FM.1 * t).sin();
                for c in &mut self.channels {
                    c.3 = (c.3 + TAU * c.0 / self.rate).rem_euclid(TAU);
                    if t.rem_euclid(c.2) < c.1 {
                        let ph = c.3 + fm;
                        y += Complex32::new((a * ph.cos()) as f32, (a * ph.sin()) as f32);
                    }
                }
                y
            }
            Method::FrameGap => {
                let (subs, sym_len, symbols) = FRAME_OFDM;
                let frame_s = 1.0 / self.p.frame_hz;
                let ts = sym_len / FRAME_SYMBOL_RATE;
                let frame = (t / frame_s).floor();
                let s = ((t - frame * frame_s) / ts).floor() as u32;
                let z = self.sub.next();
                let base = self.base.next();
                if s >= symbols {
                    return Complex32::default();
                }
                let key = (frame as u64, s);
                if self.symbol != Some(key) {
                    self.symbol = Some(key);
                    for c in &mut self.coef {
                        *c = qpsk(&mut self.data);
                    }
                }
                let mut v = self.coef[subs - 1];
                for k in (0..subs - 1).rev() {
                    v = v * z + self.coef[k];
                }
                v * base * (a / (subs as f64).sqrt()) as f32
            }
            Method::Repeat => {
                let v = if self.pos < self.sequence.len() {
                    self.sequence[self.pos]
                } else {
                    qpsk(&mut self.data)
                };
                self.pos += 1;
                if self.pos == self.period {
                    self.pos = 0;
                }
                v * a as f32
            }
        }
    }
}

impl Generator for SatSurveyGenerator {
    fn rate_hz(&self) -> Hz {
        self.rate
    }
    fn generate(&mut self, out: &mut [Complex32]) -> usize {
        let s = NOISE_SIGMA as f32;
        for z in out.iter_mut() {
            let t = self.n as f64 / self.rate;
            self.n += 1;
            let noise = Complex32::new(s * self.rng.normal_f32(), s * self.rng.normal_f32());
            *z = if self.g.signal {
                noise + self.signal(t)
            } else {
                noise
            };
        }
        out.len()
    }
    fn signal_info(&self) -> Option<SignalInfo> {
        let band = match self.p.method {
            Method::Carrier | Method::Channel => SNR_BAND_HZ,
            Method::FrameGap | Method::Repeat => self.rate,
        };
        Some(SignalInfo::new(band).with_power(self.power))
    }
    fn station_offset_hz(&self, _tune_offset_hz: Hz) -> Hz {
        0.0
    }
}
