//! The no-signal gate of the high-rate links. One piece of samples (optionally through the matched filter) is raised to the
//! power of the modulation, scaled to unit size and transformed once. When the strongest line within the search span stands
//! less than a set number of times over the mean of the bins near it, the piece is noise: the next pieces are let go by
//! unread. Otherwise the line sets the carrier and the demodulator starts. Above 4 samples a symbol only every k-th filtered
//! sample is kept (k a power of two, about half the samples a symbol), so that the noise stays white over the transform. Offset QPSK also tries the doublet of the squared
//! signal at plus and minus half the symbol rate, and raises a piece with fewer than 3 samples a symbol to the fourth power
//! only after an interpolation by 2 through the transform.

use sdrkit_algo_dsp::fft::Fft;
use sdrkit_algo_dsp::window::Window;
use sdrkit_algo_modem::psk::PskPulse;
use sdrkit_core::Complex32;

/// The rules of a gate.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GateConfig {
    /// Samples a second at the gate.
    pub rate: f64,
    /// Symbols a second.
    pub symbol_rate: f64,
    /// Samples of a piece: a power of two.
    pub piece: usize,
    /// 2 (BPSK) or 4 (QPSK, offset QPSK).
    pub power: u32,
    /// Offset QPSK: also the doublet of the squared signal.
    pub oqpsk: bool,
    /// The roll-off of a matched filter applied to the piece first, if any.
    pub matched: Option<f64>,
    /// The least height of the line over the mean of the bins near it that opens the gate.
    pub open_ratio: f64,
    /// Pieces let go by after a piece of noise.
    pub skip: u32,
    /// The largest carrier offset searched, Hz.
    pub span_hz: f64,
}

/// What a look at one piece found.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum GateOutcome {
    /// Noise: the gate stays closed.
    Closed {
        /// The height of the strongest line over the mean.
        ratio: f64,
    },
    /// A carrier.
    Open {
        /// Its offset, Hz.
        carrier_hz: f64,
        /// The height of its line over the mean.
        ratio: f64,
    },
}

/// The gate. See the [module documentation](self).
pub struct Gate {
    cfg: GateConfig,
    piece: Vec<Complex32>,
    work: Vec<Complex32>,
    wide: Vec<Complex32>,
    pow: Vec<f32>,
    window: Vec<f32>,
    taps: Vec<f32>,
    step: usize,
    n: usize,
    rate: f64,
    fft: Fft,
    fft2: Option<(Fft, Fft)>,
    skip_left: u64,
    /// Samples let go by unread.
    pub skipped: u64,
    /// Pieces looked at.
    pub looks: u64,
    /// The height of the line of the last look.
    pub last_ratio: f64,
}

impl Gate {
    /// A gate with these rules. The piece size is rounded up to a power of two of at least 1024.
    pub fn new(mut cfg: GateConfig) -> Self {
        cfg.piece = cfg.piece.max(1024).next_power_of_two();
        let sps = cfg.rate / cfg.symbol_rate.max(1.0);
        let taps = cfg
            .matched
            .and_then(|a| PskPulse::Rrc(a).matched_filter(sps.max(1.01), 8.0).ok())
            .unwrap_or_default();
        let half = (sps / 2.0).floor().max(1.0) as usize;
        let step = if half.is_power_of_two() { half } else { half.next_power_of_two() / 2 };
        let step = step.clamp(1, cfg.piece / 1024);
        let n = cfg.piece / step;
        let fft2 = (cfg.oqpsk && sps / (step as f64) < 3.0).then(|| (Fft::inverse(2 * n), Fft::forward(2 * n)));
        Gate {
            piece: Vec::with_capacity(cfg.piece),
            step,
            n,
            rate: cfg.rate / step as f64,
            cfg,
            work: vec![Complex32::default(); n],
            wide: Vec::new(),
            pow: vec![0.0; 2 * n],
            window: Window::Hann.coefficients(n),
            taps,
            fft: Fft::forward(n),
            fft2,
            skip_left: 0,
            skipped: 0,
            looks: 0,
            last_ratio: 0.0,
        }
    }

    /// The rules.
    pub fn config(&self) -> &GateConfig {
        &self.cfg
    }

    /// Forgets the piece in progress and the pieces to let go by.
    pub fn reset(&mut self) {
        self.piece.clear();
        self.skip_left = 0;
    }

    /// The piece of the last look: after an open gate, the samples the demodulator should take first.
    pub fn piece(&self) -> &[Complex32] {
        &self.piece
    }

    /// Takes samples and returns how many it used, and the outcome of a look when one ended inside them (the samples after it
    /// are not used).
    pub fn push(&mut self, x: &[Complex32]) -> (usize, Option<GateOutcome>) {
        let mut i = 0;
        if self.skip_left > 0 {
            let n = (self.skip_left as usize).min(x.len());
            self.skip_left -= n as u64;
            self.skipped += n as u64;
            i = n;
        }
        if i == x.len() {
            return (i, None);
        }
        if self.piece.len() == self.cfg.piece {
            self.piece.clear();
        }
        let n = (self.cfg.piece - self.piece.len()).min(x.len() - i);
        self.piece.extend_from_slice(&x[i..i + n]);
        i += n;
        if self.piece.len() < self.cfg.piece {
            return (i, None);
        }
        let out = self.look();
        if let GateOutcome::Closed { .. } = out {
            self.skip_left = u64::from(self.cfg.skip) * self.cfg.piece as u64;
        }
        (i, Some(out))
    }

    /// One look at the full piece.
    fn look(&mut self) -> GateOutcome {
        self.looks += 1;
        let n = self.cfg.piece;
        self.filter();
        let p = self.cfg.power.max(1) as i32;
        let (mut best_ratio, mut best_hz) = self.line(p, false);
        if self.cfg.oqpsk {
            if self.fft2.is_some() {
                let (r, f) = self.line(4, true);
                if r > best_ratio {
                    (best_ratio, best_hz) = (r, f);
                }
            }
            let (r, f) = self.doublet();
            if r > best_ratio {
                (best_ratio, best_hz) = (r, f);
            }
        }
        debug_assert_eq!(self.piece.len(), n);
        self.last_ratio = best_ratio;
        if best_ratio >= self.cfg.open_ratio {
            GateOutcome::Open {
                carrier_hz: best_hz,
                ratio: best_ratio,
            }
        } else {
            GateOutcome::Closed { ratio: best_ratio }
        }
    }

    /// Every `step`-th sample of the piece through the matched filter (when set) into `work`.
    fn filter(&mut self) {
        let len = self.cfg.piece;
        let t = &self.taps;
        let h = t.len() / 2;
        for (i, w) in self.work.iter_mut().enumerate() {
            let k = i * self.step;
            if t.is_empty() {
                *w = self.piece[k];
                continue;
            }
            let mut acc = Complex32::default();
            let lo = k.saturating_sub(h);
            let hi = (k + t.len() - h).min(len);
            for (j, x) in self.piece[lo..hi].iter().enumerate() {
                acc += x * t[lo + j + h - k];
            }
            *w = acc;
        }
    }

    /// The strongest line of the filtered piece raised to power `p` (`wide`: interpolated by 2 first), with its height over
    /// the mean of the searched bins and the carrier it gives.
    fn line(&mut self, p: i32, wide: bool) -> (f64, f64) {
        let n = self.n;
        let (len, rate) = if wide {
            self.interpolate();
            (2 * n, 2.0 * self.rate)
        } else {
            (n, self.rate)
        };
        let src = if wide { &self.wide } else { &self.work };
        let win = &self.window;
        let mut spec: Vec<Complex32> = src
            .iter()
            .enumerate()
            .map(|(k, z)| {
                let m = z.norm();
                let u = if m > 0.0 { *z / m } else { Complex32::default() };
                let w = if wide {
                    0.5 - 0.5 * (std::f32::consts::TAU * k as f32 / len as f32).cos()
                } else {
                    win[k]
                };
                u.powi(p) * w
            })
            .collect();
        if wide {
            if let Some((_, f)) = &mut self.fft2 {
                f.process(&mut spec);
            }
        } else {
            self.fft.process(&mut spec);
        }
        for (o, z) in self.pow.iter_mut().zip(&spec) {
            *o = z.norm_sqr();
        }
        let span = (self.cfg.span_hz * f64::from(p)).min(0.45 * rate);
        let bins = ((span / rate) * len as f64) as isize;
        let (mut peak, mut at, mut sum, mut count) = (0.0f32, 0isize, 0.0f64, 0usize);
        for b in -bins..=bins {
            let v = self.pow[b.rem_euclid(len as isize) as usize];
            sum += f64::from(v);
            count += 1;
            if v > peak {
                (peak, at) = (v, b);
            }
        }
        let mean = local_mean(&self.pow[..len], at, sum / count.max(1) as f64);
        let ratio = if mean > 0.0 { f64::from(peak) / mean } else { 0.0 };
        // A parabola through the peak and its neighbours refines the line.
        let get = |b: isize| f64::from(self.pow[b.rem_euclid(len as isize) as usize]);
        let (a, c) = (get(at - 1), get(at + 1));
        let d = a - 2.0 * f64::from(peak) + c;
        let frac = if d < 0.0 { 0.5 * (a - c) / d } else { 0.0 };
        let hz = (at as f64 + frac.clamp(-0.5, 0.5)) * rate / len as f64 / f64::from(p);
        (ratio, hz)
    }

    /// The filtered piece interpolated by 2 through the transform, into `wide`.
    fn interpolate(&mut self) {
        let n = self.n;
        let Some((inv, _)) = &mut self.fft2 else {
            return;
        };
        let mut s = self.work.clone();
        self.fft.process(&mut s);
        self.wide.clear();
        self.wide.resize(2 * n, Complex32::default());
        let h = n / 2;
        self.wide[..h].copy_from_slice(&s[..h]);
        self.wide[2 * n - h..].copy_from_slice(&s[h..]);
        inv.process(&mut self.wide);
    }

    /// The doublet of the squared piece at plus and minus half the symbol rate around twice the carrier: the mean of the two
    /// bins over the mean of the bins near them, and the carrier at its centre.
    fn doublet(&mut self) -> (f64, f64) {
        let n = self.n;
        let rate = self.rate;
        let mut spec = self.work.clone();
        for (k, z) in spec.iter_mut().enumerate() {
            let m = z.norm();
            let u = if m > 0.0 { *z / m } else { Complex32::default() };
            *z = u * u * self.window[k];
        }
        self.fft.process(&mut spec);
        for (o, z) in self.pow.iter_mut().zip(&spec) {
            *o = z.norm_sqr();
        }
        let get = |b: isize| f64::from(self.pow[b.rem_euclid(n as isize) as usize]);
        let d = ((0.5 * self.cfg.symbol_rate / rate) * n as f64).round() as isize;
        let span = (2.0 * self.cfg.span_hz).min(0.45 * rate);
        let bins = ((span / rate) * n as f64) as isize;
        let (mut best, mut at, mut sum, mut count) = (0.0f64, 0isize, 0.0f64, 0usize);
        for b in -bins..=bins {
            sum += get(b);
            count += 1;
            let s = 0.5 * (get(b - d) + get(b + d));
            if s > best {
                (best, at) = (s, b);
            }
        }
        let all = sum / count.max(1) as f64;
        let mean = 0.5 * (local_mean(&self.pow[..n], at - d, all) + local_mean(&self.pow[..n], at + d, all));
        let ratio = if mean > 0.0 { best / mean } else { 0.0 };
        (ratio, at as f64 * rate / n as f64 / 2.0)
    }
}

/// The mean of the bins of `pow` near bin `at` (within 1/64 of the transform, at least 64 bins, the 4 next to it left
/// out): the floor that a line is measured against, so that a floor that is not flat (noise through the matched filter)
/// does not count as a line. `fallback` when there are no such bins.
fn local_mean(pow: &[f32], at: isize, fallback: f64) -> f64 {
    let len = pow.len() as isize;
    let k = (len / 64).max(64).min(len / 2);
    let (mut sum, mut count) = (0.0f64, 0usize);
    for b in at - k..=at + k {
        if (b - at).abs() <= 4 {
            continue;
        }
        sum += f64::from(pow[b.rem_euclid(len) as usize]);
        count += 1;
    }
    if count == 0 {
        fallback
    } else {
        sum / count as f64
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cfg(power: u32, oqpsk: bool) -> GateConfig {
        GateConfig {
            rate: 4e6,
            symbol_rate: 1e6,
            piece: 32_768,
            power,
            oqpsk,
            matched: None,
            open_ratio: 16.0,
            skip: 7,
            span_hz: 200e3,
        }
    }

    fn noise(n: usize, seed: u64) -> Vec<Complex32> {
        let mut r = sdrkit_util_bits::xorshift::Xorshift64::new(seed);
        let mut u = || (r.step() >> 11) as f32 / (1u64 << 53) as f32 - 0.5;
        (0..n).map(|_| Complex32::new(u(), u())).collect()
    }

    #[test]
    fn noise_closes_the_gate_and_skips_seven_pieces() {
        let mut g = Gate::new(cfg(2, false));
        let x = noise(8 * 32_768, 3);
        let (used, out) = g.push(&x);
        assert_eq!(used, 32_768);
        assert!(matches!(out, Some(GateOutcome::Closed { ratio }) if ratio < 16.0));
        let (used, out) = g.push(&x[used..]);
        assert_eq!((used, out), (7 * 32_768, None));
        assert_eq!(g.skipped, 7 * 32_768);
    }

    #[test]
    fn a_bpsk_carrier_opens_the_gate_at_its_offset() {
        let mut g = Gate::new(cfg(2, false));
        let mut r = sdrkit_util_bits::xorshift::Xorshift64::new(9);
        let x: Vec<Complex32> = (0..32_768)
            .map(|k| {
                let s = if (k / 4) % 2 == 0 || r.step() & 1 == 0 { 1.0 } else { -1.0 };
                let ph = std::f64::consts::TAU * 51_000.0 * k as f64 / 4e6;
                Complex32::from_polar(s, ph as f32)
            })
            .collect();
        match g.push(&x).1 {
            Some(GateOutcome::Open { carrier_hz, ratio }) => {
                assert!((carrier_hz - 51_000.0).abs() < 100.0, "{carrier_hz}");
                assert!(ratio > 100.0);
            }
            o => panic!("{o:?}"),
        }
    }
}
