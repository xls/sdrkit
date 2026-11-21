//! The pulsar test signals.
//!
//! [`PulsarGenerator`] makes IQ: complex noise of power 1 and, in each channel of the filter bank, a tone at the centre of the
//! channel with a random phase and the amplitude `sqrt(S/N / C)`, keyed by raised-cosine pulses of duty 3 % that arrive at
//! `n P + delay_c` (`delay_c` the dispersion delay of the channel to the top channel). The radio centre is the stack's default
//! dial, 408 MHz, since a generator does not know where the radio is tuned. The output is that signal times
//! [`OUTPUT_SCALE`].
//!
//! [`rows`] makes filter-bank rows directly, for long tests: noise of mean 1 and deviation 0.05, a channel gain ripple
//! `1 + 0.02 sin(2 pi t / 37 s + c)`, and raised-cosine pulses whose top is the given S/N times the deviation.

use super::bank::Layout;
use super::delay_s;
use sdrkit_algo_dsp::fft::Fft;
use sdrkit_algo_sim::rng::Rng;
use sdrkit_core::publish::field;
use sdrkit_core::{Complex32, Generator, Hz, Setting, SignalInfo};
use std::f64::consts::TAU;

/// The S/N of the pulses of the test signal.
pub const GEN_SNR: Setting<field::F64> = Setting::new("gen_snr", "Test signal S/N", "", 0.1)
    .range(0.0, 10.0, 0.0)
    .help("The power of the tones at the top of a pulse over the power of the noise in the whole band.")
    .section("Test signal")
    .advanced();
/// The period of the test signal.
pub const GEN_PERIOD_S: Setting<field::F64> =
    Setting::new("gen_period_s", "Test signal period", "s", 0.5)
        .range(0.0015, 10.0, 0.0)
        .help("The period of the pulses of the test signal. 0.5 s is a test value, not a known pulsar.")
        .section("Test signal")
        .advanced();
/// The dispersion measure of the test signal.
pub const GEN_DM: Setting<field::F64> = Setting::new("gen_dm", "Test signal DM", "pc cm⁻³", 20.0)
    .range(0.0, 1000.0, 0.0)
    .help("The dispersion measure of the pulses of the test signal, for a radio centred at 408 MHz. 20 is a test value.")
    .section("Test signal")
    .advanced();

/// The output is the signal of the module docs times this. The stated power is 1, so the 8-bit model of the test source
/// (full scale 4) keeps the noise peaks and the noise is many steps of the converter.
pub const OUTPUT_SCALE: f64 = 0.2;
/// The duty cycle of the pulses of the test signal.
pub const DUTY: f64 = 0.03;
/// The radio centre the test signal is dispersed for, Hz: the stack's default dial.
pub const CENTRE_HZ: f64 = 408e6;

/// The raised-cosine pulse of width `width_s` at `t` seconds after its start: 0 outside `[0, width_s)`.
pub fn raised_cosine(t: f64, width_s: f64) -> f64 {
    if t < 0.0 || t >= width_s || width_s <= 0.0 {
        0.0
    } else {
        0.5 - 0.5 * (TAU * t / width_s).cos()
    }
}

/// The level of a pulse train of period `period_s` and pulse width `width_s` at time `t` for pulses that start at
/// `n period_s + delay`.
fn train(t: f64, delay: f64, period_s: f64, width_s: f64) -> f64 {
    raised_cosine((t - delay).rem_euclid(period_s), width_s)
}

/// The delay of each channel of `freqs_hz` to the top channel at `dm`, s.
pub fn channel_delays(freqs_hz: &[f64], dm: f64) -> Vec<f64> {
    let top = freqs_hz.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    freqs_hz.iter().map(|&f| delay_s(dm, f, top)).collect()
}

/// The IQ test signal (module docs).
pub struct PulsarGenerator {
    rate: f64,
    channels: usize,
    amplitude: f64,
    period_s: f64,
    width_s: f64,
    delays: Vec<f64>,
    phases: Vec<Complex32>,
    rng: Rng,
    ifft: Fft,
    frame: Vec<Complex32>,
    frames: u64,
    used: usize,
}

impl PulsarGenerator {
    /// A generator at `rate` with `channels` asked (as the setting: the next power of two, 8 to 1024), pulses of S/N `snr`,
    /// period `period_s` and dispersion measure `dm`.
    pub fn new(rate: f64, channels: usize, snr: f64, period_s: f64, dm: f64, seed: u64) -> Self {
        let layout = Layout::new(rate, channels, 0.5);
        let c = layout.channels;
        let mut rng = Rng::new(seed ^ 0x5055_4C53);
        let phases = (0..c)
            .map(|_| {
                let p = rng.range(0.0, TAU);
                Complex32::new(p.cos() as f32, p.sin() as f32)
            })
            .collect();
        let period_s = period_s.max(1e-6);
        PulsarGenerator {
            rate,
            channels: c,
            amplitude: (snr.max(0.0) / c as f64).sqrt(),
            period_s,
            width_s: DUTY * period_s,
            delays: channel_delays(&layout.freqs(CENTRE_HZ, rate), dm),
            phases,
            rng,
            ifft: Fft::inverse(c),
            frame: vec![Complex32::default(); c],
            frames: 0,
            used: c,
        }
    }

    /// The generator of the settings of `cfg`.
    pub fn from_config(cfg: &sdrkit_core::GenConfig) -> Self {
        let channels = usize::try_from(cfg.settings.get(&super::CHANNELS)).unwrap_or(64);
        PulsarGenerator::new(
            cfg.rate_hz,
            channels,
            cfg.settings.get(&GEN_SNR),
            cfg.settings.get(&GEN_PERIOD_S),
            cfg.settings.get(&GEN_DM),
            cfg.seed,
        )
    }

    /// Makes the next frame of `C` samples: the tones of the channels whose pulse is on at the middle of the frame, through
    /// one inverse transform, and the noise.
    fn next_frame(&mut self) {
        let c = self.channels;
        let t = (self.frames as f64 * c as f64 + c as f64 / 2.0) / self.rate;
        let mut any = false;
        self.frame.fill(Complex32::default());
        // Channel `ch` is the bin (ch + C / 2) mod C; a frame starts at a multiple of C samples, so each bin is a whole number
        // of cycles from the start of every frame.
        for ch in 0..c {
            let e = train(t, self.delays[ch], self.period_s, self.width_s);
            if e > 0.0 {
                any = true;
                self.frame[(ch + c / 2) % c] = self.phases[ch] * (self.amplitude * e) as f32;
            }
        }
        if any {
            self.ifft.process(&mut self.frame);
        }
        let scale = OUTPUT_SCALE as f32;
        for z in self.frame.iter_mut() {
            *z = (*z + self.rng.complex_normal(1.0)) * scale;
        }
        self.frames += 1;
        self.used = 0;
    }
}

impl Generator for PulsarGenerator {
    fn rate_hz(&self) -> Hz {
        self.rate
    }
    fn generate(&mut self, out: &mut [Complex32]) -> usize {
        let mut i = 0;
        while i < out.len() {
            if self.used == self.channels {
                self.next_frame();
            }
            let n = (self.channels - self.used).min(out.len() - i);
            out[i..i + n].copy_from_slice(&self.frame[self.used..self.used + n]);
            self.used += n;
            i += n;
        }
        out.len()
    }
    fn signal_info(&self) -> Option<SignalInfo> {
        // The nominal level, not the mean power: see OUTPUT_SCALE.
        Some(SignalInfo::new(self.rate).with_power(1.0))
    }
    fn station_offset_hz(&self, _tune_offset_hz: Hz) -> Hz {
        0.0
    }
}

/// What [`rows`] makes.
#[derive(Clone, Debug, PartialEq)]
pub struct RowsParams {
    /// The radio centre, Hz.
    pub centre_hz: f64,
    /// The radio rate, Hz.
    pub rate_hz: f64,
    /// The layout of the rows.
    pub layout: Layout,
    /// The period, s.
    pub period_s: f64,
    /// The dispersion measure, pc cm⁻³.
    pub dm: f64,
    /// The top of a pulse over the deviation of the noise, per channel and row.
    pub snr: f64,
    /// The duty cycle of the pulses.
    pub duty: f64,
    /// The length, s.
    pub seconds: f64,
    /// The seed of the noise.
    pub seed: u64,
}

/// The deviation of the noise of [`rows`].
pub const ROWS_SIGMA: f64 = 0.05;
/// The depth of the gain ripple of [`rows`].
pub const ROWS_RIPPLE: f64 = 0.02;
/// The period of the gain ripple of [`rows`], s.
pub const ROWS_RIPPLE_S: f64 = 37.0;

/// Filter-bank rows (module docs), row-major. A pulse is the mean of the raised cosine over the row (8 points).
pub fn rows(p: &RowsParams) -> Vec<f32> {
    let l = p.layout;
    let c = l.channels;
    let n = (p.seconds / l.row_s).round() as usize;
    let delays = channel_delays(&l.freqs(p.centre_hz, p.rate_hz), p.dm);
    let width = p.duty * p.period_s;
    let mut rng = Rng::new(p.seed ^ 0x524F_5753);
    let mut out = Vec::with_capacity(n * c);
    for r in 0..n {
        let t0 = r as f64 * l.row_s;
        for (ch, &d) in delays.iter().enumerate() {
            let pulse = (0..8)
                .map(|k| train(t0 + (k as f64 + 0.5) * l.row_s / 8.0, d, p.period_s, width))
                .sum::<f64>()
                / 8.0;
            let gain = 1.0 + ROWS_RIPPLE * (TAU * t0 / ROWS_RIPPLE_S + ch as f64).sin();
            let v = gain * (1.0 + ROWS_SIGMA * (rng.normal() + p.snr * pulse));
            out.push(v as f32);
        }
    }
    out
}
