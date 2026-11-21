//! A bank of sliding non-coherent tone correlators with an energy-peak symbol clock, for M-FSK of a few tones (2 or 4 in
//! telemetry and teleprinter links). Each tone is correlated over the last `L` samples (the shared [`ToneCorrelator`]). For
//! each of the `L` sample phases of a symbol the bank keeps an average of the contrast, the strongest energy less the mean of
//! the others over their total; a symbol is taken one window after the last, moved to the phase with the highest average. A
//! retune keeps the averages of the clock and the samples of the window ([`RetunableCorrelator`]), so it loses no symbol.

use crate::error::{ModemError, Param};
use crate::fsk::ToneCorrelator;
use sdrkit_algo_dsp::Complex32;

/// Sliding tone correlators ([`ToneCorrelator`]) that keep the samples of their window: a retune mixes the window again with
/// the new tones, so the energies stay ready and no sample is lost.
#[derive(Clone, Debug)]
pub struct RetunableCorrelator {
    corr: ToneCorrelator,
    tones: Vec<f64>,
    raw: Vec<Complex32>,
    pos: usize,
    filled: usize,
}

impl RetunableCorrelator {
    /// Tones at `tones` (cycles per sample, as [`ToneCorrelator::new`] takes them) over windows of `window` samples.
    pub fn new(tones: &[f64], window: usize) -> Result<Self, ModemError> {
        Ok(RetunableCorrelator {
            corr: ToneCorrelator::new(tones, window)?,
            tones: tones.to_vec(),
            raw: vec![Complex32::default(); window],
            pos: 0,
            filled: 0,
        })
    }

    /// The number of tones.
    pub fn tones(&self) -> usize {
        self.corr.tones()
    }

    /// Whether a whole window has been seen.
    pub fn ready(&self) -> bool {
        self.corr.ready()
    }

    /// Takes one sample.
    #[inline]
    pub fn push(&mut self, x: Complex32) {
        self.raw[self.pos] = x;
        self.pos = (self.pos + 1) % self.raw.len();
        self.filled = (self.filled + 1).min(self.raw.len());
        self.corr.push(x);
    }

    /// The energies of the last window into `out[0..tones]`.
    pub fn energies(&self, out: &mut [f32]) -> Result<(), ModemError> {
        self.corr.energies(out)
    }

    /// Follows new tones (as many as before): the samples of the window are mixed again with them.
    pub fn retune(&mut self, tones: &[f64]) -> Result<(), ModemError> {
        Param::Tones.check_count(tones.len(), self.corr.tones(), self.corr.tones())?;
        let n = self.raw.len();
        let mut corr = ToneCorrelator::new(tones, n)?;
        for k in 0..self.filled {
            corr.push(self.raw[(self.pos + n - self.filled + k) % n]);
        }
        self.corr = corr;
        self.tones = tones.to_vec();
        Ok(())
    }

    /// The tones, cycles per sample.
    pub fn frequencies(&self) -> &[f64] {
        &self.tones
    }

    /// The turn of tone `k` from the older half of the window to the newer half: the sum of the newer half mixed down by the
    /// tone, times the conjugate of that of the older half. Its angle over `2 pi` and half the window is how far the signal in
    /// the window lies above the tone, in cycles per sample. `None` before a whole window or for a tone the bank lacks.
    pub fn half_turn(&self, k: usize) -> Option<Complex32> {
        let n = self.raw.len();
        let f = *self.tones.get(k)?;
        if self.filled < n || n < 2 {
            return None;
        }
        let h = n / 2;
        let (mut a, mut b) = (Complex32::default(), Complex32::default());
        for j in 0..2 * h {
            let x = self.raw[(self.pos + n - 2 * h + j) % n];
            let ph = -std::f64::consts::TAU * f * j as f64;
            let y = x * Complex32::new(ph.cos() as f32, ph.sin() as f32);
            if j < h {
                a += y;
            } else {
                b += y;
            }
        }
        Some(b * a.conj())
    }
}

/// The bank.
#[derive(Clone, Debug)]
pub struct ToneBank {
    corr: RetunableCorrelator,
    window: usize,
    contrast: Vec<f32>,
    alpha: f32,
    phase: usize,
    since: usize,
    last: usize,
    energy: Vec<f32>,
    symbols: u64,
    /// Samples taken.
    n: u64,
    /// The energies of every sample of the last `history` samples, the tones of a sample together, and the first sample
    /// whose energies are of a whole window.
    history: Vec<f32>,
    ready_from: Option<u64>,
    /// The sample of each of the last `times.len()` symbols.
    times: Vec<u64>,
}

/// The timing of a run of symbols that [`ToneBank::refit`] found: symbol `k` of the run ends at sample `start + k period`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Timing {
    /// The sample at which the first symbol of the run ends.
    pub start: f64,
    /// Samples a symbol.
    pub period: f64,
}

impl ToneBank {
    /// Tones at `tones` (cycles per sample) over windows of `window` samples (one symbol, at least 4); the contrast is
    /// averaged over about `average_symbols` symbols.
    pub fn new(tones: &[f64], window: usize, average_symbols: f32) -> Result<Self, ModemError> {
        if window < 4 {
            return Param::Window.at_least(4.0, window as f64);
        }
        if average_symbols.is_nan() || average_symbols < 1.0 {
            return Param::Averaging.at_least(1.0, f64::from(average_symbols));
        }
        Ok(ToneBank {
            corr: RetunableCorrelator::new(tones, window)?,
            window,
            contrast: vec![0.0; window],
            alpha: 1.0 / average_symbols,
            phase: 0,
            since: 0,
            last: 0,
            energy: vec![0.0; tones.len()],
            symbols: 0,
            n: 0,
            history: Vec::new(),
            ready_from: None,
            times: Vec::new(),
        })
    }

    /// The same bank keeping the energies of every sample of the last `symbols` symbols, for [`ToneBank::refit`] and
    /// [`ToneBank::energies_at`].
    pub fn with_history(mut self, symbols: usize) -> Self {
        self.history = vec![0.0; symbols * self.window * self.energy.len()];
        self.times = vec![0; symbols];
        self
    }

    /// The sample at which symbol `k` (counted from 0) was taken, while the history holds it.
    pub fn symbol_sample(&self, k: u64) -> Option<u64> {
        let cap = self.times.len() as u64;
        (cap > 0 && k < self.symbols && k + cap >= self.symbols)
            .then(|| self.times[(k % cap) as usize])
    }

    /// The energies of the window that ends at sample `at`, while the history holds it.
    fn stored(&self, at: u64) -> Option<&[f32]> {
        let m = self.energy.len();
        let cap = (self.history.len() / m) as u64;
        let from = self.ready_from?;
        if cap == 0 || at < from || at >= self.n || at + cap < self.n {
            return None;
        }
        let i = (at % cap) as usize * m;
        Some(&self.history[i..i + m])
    }

    /// The timing of symbols `first` to `first + count - 1` that gives the largest sum of the strongest energy less the mean
    /// of the others over the run: the period within `max_ppm` of the straight line through the samples at which the bank took
    /// them, and the start within half a window of that line. `None` when the history does not hold the run.
    pub fn refit(&self, first: u64, count: usize, max_ppm: f64) -> Option<Timing> {
        if count < 2 {
            return None;
        }
        let at: Vec<f64> = (0..count as u64)
            .map(|k| self.symbol_sample(first + k).map(|s| s as f64))
            .collect::<Option<_>>()?;
        // The least-squares line through the samples of the symbols.
        let c = count as f64;
        let mk = (c - 1.0) / 2.0;
        let ms = at.iter().sum::<f64>() / c;
        let (mut sxy, mut sxx) = (0.0, 0.0);
        for (k, &s) in at.iter().enumerate() {
            let dk = k as f64 - mk;
            sxy += dk * (s - ms);
            sxx += dk * dk;
        }
        let period = sxy / sxx;
        let start = ms - period * mk;
        let w = self.window as f64;
        let score = |t: Timing| -> Option<f64> {
            let mut sum = 0.0;
            for k in 0..count {
                let e = self.stored((t.start + k as f64 * t.period).round().max(0.0) as u64)?;
                let total: f32 = e.iter().sum();
                let best = e.iter().copied().fold(0f32, f32::max);
                sum += f64::from(best - (total - best) / (e.len() - 1).max(1) as f32);
            }
            Some(sum)
        };
        let steps = (max_ppm / 100.0).ceil() as i64;
        let half = (w / 2.0).floor() as i64;
        let mut best: Option<(Timing, f64)> = None;
        for p in -steps..=steps {
            let period = period * (1.0 + p as f64 * max_ppm / steps.max(1) as f64 * 1e-6);
            for d in -half..=half {
                let t = Timing {
                    start: start + d as f64,
                    period,
                };
                if let Some(v) = score(t) {
                    if best.is_none_or(|(_, b)| v > b) {
                        best = Some((t, v));
                    }
                }
            }
        }
        best.map(|(t, _)| t)
    }

    /// Appends the energies of each tone of `count` symbols at timing `t`; false (and nothing appended) when the history does
    /// not hold them.
    pub fn energies_at(&self, t: &Timing, count: usize, out: &mut Vec<f32>) -> bool {
        let len = out.len();
        for k in 0..count {
            match self.stored((t.start + k as f64 * t.period).round().max(0.0) as u64) {
                Some(e) => out.extend_from_slice(e),
                None => {
                    out.truncate(len);
                    return false;
                }
            }
        }
        true
    }

    /// The number of tones.
    pub fn tones(&self) -> usize {
        self.energy.len()
    }

    /// Symbols produced so far.
    pub fn symbols(&self) -> u64 {
        self.symbols
    }

    /// Follows new tones (as many as before). The clock keeps its averages, and the window is mixed again with the new tones.
    pub fn retune(&mut self, tones: &[f64]) -> Result<(), ModemError> {
        self.corr.retune(tones)
    }

    /// Takes samples and appends the energies of each tone for each symbol taken.
    pub fn process(&mut self, x: &[Complex32], out: &mut Vec<f32>) {
        for &v in x {
            self.corr.push(v);
            self.n += 1;
            let p = self.phase;
            self.phase = (self.phase + 1) % self.window;
            self.since += 1;
            if !self.corr.ready() {
                continue;
            }
            if self.corr.energies(&mut self.energy).is_err() {
                continue;
            }
            let m = self.energy.len();
            if !self.history.is_empty() {
                let cap = self.history.len() / m;
                let i = ((self.n - 1) as usize % cap) * m;
                self.history[i..i + m].copy_from_slice(&self.energy);
                self.ready_from.get_or_insert(self.n - 1);
            }
            let total: f32 = self.energy.iter().sum();
            let best = self.energy.iter().copied().fold(0f32, f32::max);
            let others = (total - best) / (self.energy.len() - 1).max(1) as f32;
            let q = if total > 0.0 {
                (best - others) / total
            } else {
                0.0
            };
            self.contrast[p] += self.alpha * (q - self.contrast[p]);
            let top = self
                .contrast
                .iter()
                .enumerate()
                .max_by(|a, b| a.1.total_cmp(b.1))
                .map_or(0, |(i, _)| i);
            // The next symbol is due one window after the last, moved by the signed distance from the phase of the last to the
            // best phase now, so a best phase that moves either way never drops or doubles a symbol.
            let l = self.window as isize;
            let delta = (top as isize - self.last as isize + l + l / 2).rem_euclid(l) - l / 2;
            if self.since as isize >= l + delta {
                self.since = 0;
                self.last = p;
                if !self.times.is_empty() {
                    let cap = self.times.len() as u64;
                    self.times[(self.symbols % cap) as usize] = self.n - 1;
                }
                self.symbols += 1;
                out.extend_from_slice(&self.energy);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn four_tones_with_a_clock_error_come_out_symbol_by_symbol() {
        let (l, tones) = (48usize, [0.05, 0.08, 0.11, 0.14]);
        let mut rng = 12345u32;
        let sent: Vec<usize> = (0..400)
            .map(|_| {
                rng = rng.wrapping_mul(1_103_515_245).wrapping_add(12345);
                (rng >> 16) as usize % 4
            })
            .collect();
        // The symbols 0.2 % longer than the window, phase continuous.
        let per = l as f64 * 1.002;
        let n = (sent.len() as f64 * per) as usize;
        let mut ph = 0.0f64;
        let x: Vec<Complex32> = (0..n)
            .map(|i| {
                ph += std::f64::consts::TAU * tones[sent[(i as f64 / per) as usize]];
                Complex32::from_polar(1.0, ph as f32)
            })
            .collect();
        let mut b = ToneBank::new(&tones, l, 24.0).unwrap();
        let mut e = Vec::new();
        b.process(&x, &mut e);
        let got: Vec<usize> = e
            .chunks(4)
            .map(|s| {
                s.iter()
                    .enumerate()
                    .max_by(|a, b| a.1.total_cmp(b.1))
                    .unwrap()
                    .0
            })
            .collect();
        // After the clock settles, the symbols follow the sent ones in order.
        let tail = &got[100..got.len() - 10];
        let at = (90..=110)
            .find(|&k| sent[k..k + 50] == tail[..50])
            .expect("in step");
        let wrong = tail.iter().zip(&sent[at..]).filter(|(a, b)| a != b).count();
        assert!(wrong <= 2, "{wrong} wrong");
        assert!(ToneBank::new(&tones, 3, 24.0).is_err());
    }

    /// The symbol error rate of non-coherent orthogonal 4-FSK at `es_n0` (a ratio).
    fn theory_4fsk(es_n0: f64) -> f64 {
        // P = sum over k = 1..3 of (-1)^(k+1) C(3, k) / (k + 1) exp(-k / (k + 1) Es/N0).
        1.5 * (-es_n0 / 2.0).exp() - (-2.0 * es_n0 / 3.0).exp() + 0.25 * (-0.75 * es_n0).exp()
    }

    #[test]
    fn the_symbol_errors_in_noise_follow_theory() {
        // 4-FSK of 96 samples a symbol, 270/100 symbol rates apart, symbols 0.1 % long, at Es/N0 8.8 dB.
        let l = 96usize;
        let tones: Vec<f64> = (0..4).map(|k| 0.1 + k as f64 * 2.7 / l as f64).collect();
        let es_n0 = 10f64.powf(0.88);
        let sigma = (l as f64 / es_n0 / 2.0).sqrt();
        let mut rng = 99u64;
        let mut uniform = || {
            rng = rng
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            ((rng >> 11) as f64 + 0.5) / (1u64 << 53) as f64
        };
        let n_sym = 20_000;
        let sent: Vec<usize> = (0..n_sym).map(|_| (uniform() * 4.0) as usize % 4).collect();
        let per = l as f64 * 1.001;
        let n = (n_sym as f64 * per) as usize;
        let mut ph = 0.0f64;
        let x: Vec<Complex32> = (0..n)
            .map(|i| {
                ph += std::f64::consts::TAU * tones[sent[(i as f64 / per) as usize]];
                let r = (-2.0 * uniform().ln()).sqrt() * sigma;
                let a = std::f64::consts::TAU * uniform();
                Complex32::new(
                    (ph.cos() + r * a.cos()) as f32,
                    (ph.sin() + r * a.sin()) as f32,
                )
            })
            .collect();
        let mut b = ToneBank::new(&tones, l, 24.0).unwrap();
        let mut e = Vec::new();
        b.process(&x, &mut e);
        let got: Vec<usize> = e
            .chunks(4)
            .map(|s| {
                s.iter()
                    .enumerate()
                    .max_by(|a, b| a.1.total_cmp(b.1))
                    .unwrap()
                    .0
            })
            .collect();
        let at = (190..=210)
            .min_by_key(|&k| {
                got[200..400]
                    .iter()
                    .zip(&sent[k..])
                    .filter(|(a, b)| a != b)
                    .count()
            })
            .unwrap();
        let errors = got[200..n_sym - 100]
            .iter()
            .zip(&sent[at..])
            .filter(|(a, b)| a != b)
            .count();
        let ser = errors as f64 / (n_sym - 300) as f64;
        let theory = theory_4fsk(es_n0);
        println!("symbol errors {ser} against {theory}");
        // Over 19700 symbols the count has a spread of about 4 %; the clock may cost at most 0.2 dB.
        assert!(
            ser < theory_4fsk(es_n0 / 10f64.powf(0.02)) * 1.1,
            "{ser} against {theory}"
        );
    }

    #[test]
    fn a_retune_mixes_the_window_again() {
        let x: Vec<Complex32> = (0..300)
            .map(|i| Complex32::from_polar(1.0, 0.37 * i as f32 + 0.001 * (i * i) as f32))
            .collect();
        let (a, b) = ([0.05, 0.11], [0.06, 0.13]);
        let mut kept = RetunableCorrelator::new(&a, 64).unwrap();
        for &v in &x[..250] {
            kept.push(v);
        }
        kept.retune(&b).unwrap();
        assert!(kept.ready());
        let mut fresh = ToneCorrelator::new(&b, 64).unwrap();
        for &v in &x[250 - 64..250] {
            fresh.push(v);
        }
        for &v in &x[250..] {
            kept.push(v);
            fresh.push(v);
        }
        let (mut e1, mut e2) = ([0f32; 2], [0f32; 2]);
        kept.energies(&mut e1).unwrap();
        fresh.energies(&mut e2).unwrap();
        for (p, q) in e1.iter().zip(&e2) {
            assert!((p - q).abs() <= 1e-3 * q.max(1.0), "{e1:?} {e2:?}");
        }
        assert!(kept.retune(&[0.1]).is_err());
    }

    #[test]
    fn the_half_turn_measures_how_far_a_tone_lies_from_its_correlator() {
        let mut c = RetunableCorrelator::new(&[0.05, 0.1], 64).unwrap();
        assert!(c.half_turn(0).is_none());
        for i in 0..100 {
            c.push(Complex32::from_polar(
                1.0,
                (std::f64::consts::TAU * 0.1012 * i as f64) as f32,
            ));
        }
        let off = f64::from(c.half_turn(1).unwrap().arg()) / (std::f64::consts::TAU * 32.0);
        assert!((off - 0.0012).abs() < 1e-5, "{off}");
        assert!(c.half_turn(2).is_none());
    }
}
