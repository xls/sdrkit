//! The FM front end: the channel filter and decimation, the constant-modulus equaliser and the multipath detector.

use sdrkit_algo_dsp::decim::FirDecimator;
use sdrkit_core::Complex32;

/// The standard channel width.
pub const STANDARD_WIDTH_HZ: f64 = 220e3;

/// The channel decimation for an input rate: the nearest integer to `rate / 500 kHz`, at least 1.
pub fn decimation(rate: f64) -> usize {
    ((rate / 500e3).round() as usize).max(1)
}

/// The channel width actually used for a setting (0 = standard), held to at least 10 kHz and at most what the rate carries.
pub fn effective_width(setting_hz: f64, rate: f64) -> f64 {
    let w = if setting_hz <= 0.0 {
        STANDARD_WIDTH_HZ
    } else {
        setting_hz.max(10e3)
    };
    w.min(0.9 * rate)
}

/// The transition band of the last stage for a channel width.
///
/// The standard channel has 80 kHz (pass band to 110 kHz, stop band from 190 kHz). A width chosen to keep out a neighbour gets a
/// transition of at most 15 % of the width, so the neighbour's carrier and deviation fall in the stop band.
pub fn last_transition(width_hz: f64) -> f64 {
    if width_hz >= STANDARD_WIDTH_HZ {
        80e3
    } else {
        (0.15 * width_hz).clamp(5e3, 80e3)
    }
}

/// The channel filter: low-pass Kaiser stages, 62 dB, unity gain at DC, decimating to the channel rate near 500 kHz.
#[derive(Clone, Debug)]
pub struct ChannelFilter {
    stages: Vec<FirDecimator<Complex32>>,
    bufs: Vec<Vec<Complex32>>,
    out_rate: f64,
    width: f64,
}

impl ChannelFilter {
    /// A filter for input at `rate` with a channel `width_hz` wide (0 = standard).
    pub fn new(rate: f64, width_hz: f64) -> Self {
        let d = decimation(rate);
        let mut factors = sdrkit_algo_dsp::decim::stage_factors(d);
        if factors.is_empty() {
            factors.push(1);
        }
        let width = effective_width(width_hz, rate);
        let pass = width / 2.0;
        let out_rate = rate / d as f64;
        let mut stages = Vec::new();
        let mut r = rate;
        let n = factors.len();
        for (i, &f) in factors.iter().enumerate() {
            let o = r / f as f64;
            let stop = if i + 1 == n {
                (pass + last_transition(width))
                    .min((o - pass).max(pass + 1e3))
                    .min(r / 2.0)
            } else {
                (o - pass).min(r / 2.0)
            };
            let taps = sdrkit_algo_dsp::fir::lowpass(r, pass, stop, 62.0)
                .expect("the filter settings are valid");
            stages.push(FirDecimator::new(&taps, f));
            r = o;
        }
        let bufs = vec![Vec::new(); stages.len()];
        ChannelFilter {
            stages,
            bufs,
            out_rate,
            width,
        }
    }

    /// The channel rate.
    pub fn output_rate(&self) -> f64 {
        self.out_rate
    }

    /// The width in use.
    pub fn width(&self) -> f64 {
        self.width
    }

    /// Appends the channel samples for `input` to `out`.
    pub fn process(&mut self, input: &[Complex32], out: &mut Vec<Complex32>) {
        let n = self.stages.len();
        let mut bufs = std::mem::take(&mut self.bufs);
        for i in 0..n {
            let (before, after) = bufs.split_at_mut(i);
            let src: &[Complex32] = if i == 0 { input } else { &before[i - 1] };
            if i + 1 == n {
                self.stages[i].process(src, out);
            } else {
                after[0].clear();
                self.stages[i].process(src, &mut after[0]);
            }
        }
        self.bufs = bufs;
    }
}

/// Taps of the constant-modulus equaliser.
pub const CMA_TAPS: usize = 16;
/// The main tap (the equaliser delays the signal by this many samples).
pub const CMA_MAIN: usize = 3;

/// A 16-tap constant-modulus (CMA) equaliser. Its output has unit modulus when it has converged.
///
/// Taps and history are kept as separate real and imaginary arrays (the history twice, newest first), so the filter and the tap
/// update are plain 16-wide loops that vectorise.
#[derive(Clone, Debug)]
pub struct Cma {
    wr: [f32; CMA_TAPS],
    wi: [f32; CMA_TAPS],
    hr: [f32; 2 * CMA_TAPS],
    hi: [f32; 2 * CMA_TAPS],
    idx: usize,
    power: f32,
    mu: f32,
}

impl Default for Cma {
    fn default() -> Self {
        Self::new()
    }
}

impl Cma {
    /// An equaliser at the identity.
    pub fn new() -> Self {
        let mut c = Cma {
            wr: [0.0; CMA_TAPS],
            wi: [0.0; CMA_TAPS],
            hr: [0.0; 2 * CMA_TAPS],
            hi: [0.0; 2 * CMA_TAPS],
            idx: 0,
            power: 0.0,
            mu: 2e-4,
        };
        c.identity();
        c
    }

    /// Back to the identity (the output is then the input, normalised and delayed by [`CMA_MAIN`]).
    pub fn identity(&mut self) {
        self.wr = [0.0; CMA_TAPS];
        self.wi = [0.0; CMA_TAPS];
        self.wr[CMA_MAIN] = 1.0;
    }

    /// Takes one sample into the power average and the history without filtering or adapting (while no one uses the output).
    #[inline]
    pub fn observe(&mut self, x: Complex32) {
        self.power += 0.002 * (x.norm_sqr() - self.power);
        let g = 1.0 / self.power.max(1e-20).sqrt();
        // Newest first: h[idx..idx + N] runs from the newest sample back.
        self.idx = if self.idx == 0 {
            CMA_TAPS - 1
        } else {
            self.idx - 1
        };
        let i = self.idx;
        (self.hr[i], self.hi[i]) = (x.re * g, x.im * g);
        (self.hr[i + CMA_TAPS], self.hi[i + CMA_TAPS]) = (x.re * g, x.im * g);
    }

    /// One sample.
    #[inline]
    pub fn process(&mut self, x: Complex32) -> Complex32 {
        self.observe(x);
        let i = self.idx;
        let hr: &[f32; CMA_TAPS] = self.hr[i..i + CMA_TAPS].try_into().expect("16 taps");
        let hi: &[f32; CMA_TAPS] = self.hi[i..i + CMA_TAPS].try_into().expect("16 taps");
        // y = sum w h: the products element by element, then a fixed tree of sums.
        let (mut pr, mut pi) = ([0.0f32; CMA_TAPS], [0.0f32; CMA_TAPS]);
        for k in 0..CMA_TAPS {
            pr[k] = self.wr[k] * hr[k] - self.wi[k] * hi[k];
            pi[k] = self.wr[k] * hi[k] + self.wi[k] * hr[k];
        }
        let y = Complex32::new(sum16(&pr), sum16(&pi));
        let m = y.norm_sqr();
        if m.is_nan() || m >= 16.0 {
            let out = Complex32::new(hr[CMA_MAIN], hi[CMA_MAIN]);
            self.identity();
            return out;
        }
        // w -= e conj(h), e = y (|y|^2 - 1) mu.
        let (er, ei) = (y.re * (m - 1.0) * self.mu, y.im * (m - 1.0) * self.mu);
        for k in 0..CMA_TAPS {
            self.wr[k] -= er * hr[k] + ei * hi[k];
            self.wi[k] -= ei * hr[k] - er * hi[k];
        }
        y
    }
}

#[inline]
fn sum16(v: &[f32; CMA_TAPS]) -> f32 {
    let mut a = [0.0f32; 8];
    for k in 0..8 {
        a[k] = v[k] + v[k + 8];
    }
    let b = [a[0] + a[4], a[1] + a[5], a[2] + a[6], a[3] + a[7]];
    (b[0] + b[2]) + (b[1] + b[3])
}

/// The multipath detector: an echo makes the FM envelope depend on the instantaneous frequency.
#[derive(Clone, Debug)]
pub struct MultipathDetector {
    sums: [f64; 16],
    counts: [u32; 16],
    acc_f: f32,
    acc_p: f32,
    k: u8,
    n: u64,
    period: u64,
    /// The last ripple measure in percent.
    pub ripple_pct: f32,
    /// Multipath detected (the equaliser is then in use). Starts on.
    pub detected: bool,
}

impl MultipathDetector {
    /// A detector for channel samples at `rate`, judging every 0.25 s.
    pub fn new(rate: f64) -> Self {
        MultipathDetector {
            sums: [0.0; 16],
            counts: [0; 16],
            acc_f: 0.0,
            acc_p: 0.0,
            k: 0,
            n: 0,
            period: (0.25 * rate).round() as u64,
            ripple_pct: 0.0,
            detected: true,
        }
    }

    /// One sample: the instantaneous frequency in Hz and the raw channel power. `snr_db` is the latest SNR measure.
    #[inline]
    pub fn push(&mut self, freq_hz: f32, power: f32, snr_db: f32) {
        self.acc_f += freq_hz;
        self.acc_p += power;
        self.k += 1;
        if self.k == 8 {
            let f = self.acc_f / 8.0;
            let bin = ((f + 80e3) / 10e3).floor();
            if (0.0..16.0).contains(&bin) {
                let b = bin as usize;
                self.sums[b] += self.acc_p as f64;
                self.counts[b] += 8;
            }
            self.acc_f = 0.0;
            self.acc_p = 0.0;
            self.k = 0;
        }
        self.n += 1;
        if self.n >= self.period {
            self.n = 0;
            self.judge(snr_db);
        }
    }

    fn judge(&mut self, snr_db: f32) {
        // The mean power of each bin that holds at least 2000 samples (on the stack: this runs four times a second).
        let mut means = [0.0f64; 16];
        let mut k = 0;
        for (&s, &c) in self.sums.iter().zip(&self.counts) {
            if c >= 2000 {
                means[k] = s / c as f64;
                k += 1;
            }
        }
        let means = &means[..k];
        self.ripple_pct = if means.len() >= 3 {
            let m = means.iter().sum::<f64>() / means.len() as f64;
            let var = means.iter().map(|v| (v - m) * (v - m)).sum::<f64>() / means.len() as f64;
            if m > 0.0 {
                (100.0 * var.sqrt() / m) as f32
            } else {
                0.0
            }
        } else {
            0.0
        };
        let rise = 0.6 * (30.0 - snr_db).max(0.0);
        if self.ripple_pct > 6.0 + rise {
            self.detected = true;
        } else if self.ripple_pct < 3.5 + rise {
            self.detected = false;
        }
        self.sums = [0.0; 16];
        self.counts = [0; 16];
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rates_and_stages() {
        assert_eq!(decimation(2e6), 4);
        assert_eq!(decimation(2.048e6), 4);
        assert_eq!(decimation(500e3), 1);
        assert_eq!(decimation(6e6), 12);
        assert_eq!(ChannelFilter::new(2.048e6, 0.0).output_rate(), 512e3);
        assert_eq!(effective_width(0.0, 2e6), 220e3);
        assert_eq!(effective_width(5e3, 2e6), 10e3);
        assert_eq!(effective_width(2e6, 500e3), 450e3);
    }

    #[test]
    fn the_channel_filter_passes_the_channel_and_stops_neighbours() {
        let rate = 6e6;
        let tone = |f: f64| -> Vec<Complex32> {
            (0..120_000)
                .map(|i| {
                    let p = std::f64::consts::TAU * f * i as f64 / rate;
                    Complex32::new(p.cos() as f32, p.sin() as f32)
                })
                .collect()
        };
        for (f, lo, hi) in [
            (100e3, -0.1, 0.1),
            (-100e3, -0.1, 0.1),
            (400e3, -200.0, -55.0),
            (2.2e6, -200.0, -55.0),
        ] {
            let mut cf = ChannelFilter::new(rate, 0.0);
            let mut out = Vec::new();
            cf.process(&tone(f), &mut out);
            let p: f64 = out[2000..].iter().map(|v| v.norm_sqr() as f64).sum::<f64>()
                / (out.len() - 2000) as f64;
            let db = 10.0 * p.log10();
            assert!(db > lo && db < hi, "{f} Hz: {db} dB");
        }
    }

    #[test]
    fn the_equaliser_is_transparent_on_a_clean_carrier() {
        let mut c = Cma::new();
        let mut last = Complex32::default();
        for i in 0..10_000 {
            let p = 0.01 * i as f32;
            last = c.process(Complex32::new(0.3 * p.cos(), 0.3 * p.sin()));
        }
        assert!((last.norm() - 1.0).abs() < 0.01);
    }
}
