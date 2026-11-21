//! The linear modulator: complex symbols shaped by any pulse at any (fractional) number of samples per symbol, with an optional
//! half-symbol offset of the Q rail (OQPSK, and MSK as OQPSK with half-sine pulses).

use crate::{Param, SimError};
use sdrkit_algo_dsp::pulse::Shape;
use sdrkit_algo_dsp::Sample;
use sdrkit_core::Complex32;

/// Pulse table resolution, points per symbol.
const RES: usize = 512;

/// A linear modulator: `y(t) = sum_k Re(s_k) p(t - k) + j sum_k Im(s_k) p(t - k - d)`, with `t` in symbols, `p` the pulse, and `d`
/// 0 (or 1/2 with [`with_offset_q`](Linear::with_offset_q)).
///
/// The pulse is scaled to unit energy (`integral p^2 dt = 1`, `t` in symbols), so a stream of independent symbols of mean power 1
/// produces a signal of mean power 1, whatever the pulse. Output sample `n` is at `t = n / sps - h`, where `h` is half the pulse's
/// span: the first output is where symbol 0's pulse begins, and symbol `k`'s centre is output sample `(k + h) sps`
/// ([`symbol_centre`](Linear::symbol_centre)). Positions come from the sample number, so a fractional `sps` never drifts.
///
/// [`push`](Linear::push) appends every sample that the symbols so far determine; [`flush`](Linear::flush) appends the tail of the
/// last pulse and starts again.
///
/// ```
/// use sdrkit_algo_sim::modulate::Linear;
/// use sdrkit_algo_dsp::pulse::Shape;
/// use sdrkit_core::Complex32;
/// let mut m = Linear::new(Shape::Rrc { beta: 0.35 }, 12.0, 4.0).unwrap();
/// let mut out = Vec::new();
/// m.push(&[Complex32::new(1.0, 0.0), Complex32::new(-1.0, 0.0)], &mut out);
/// m.flush(&mut out);
/// assert_eq!(out.len(), (2 - 1 + 12) * 4);
/// ```
#[derive(Clone, Debug)]
pub struct Linear {
    half: f64,
    sps: f64,
    offset_q: f64,
    /// One row per table phase (`RES` rows). A row holds `taps` pulse values one symbol apart, then the `taps` differences to
    /// the values of the next phase, all scaled to unit pulse energy.
    rows: Vec<f32>,
    taps: usize,
    /// The pulse values of one output sample, between two rows.
    weights: Vec<f32>,
    /// The last `mask + 1` symbols, newest first. Each symbol is stored twice, `mask + 1` apart, so that a run of symbols
    /// never wraps.
    history: Vec<Complex32>,
    mask: usize,
    syms: u64,
    n: u64,
}

/// The factor that scales `shape` over `span_symbols` to unit energy. Errors when the energy is zero or not finite.
fn unit_energy_scale(shape: Shape, span_symbols: f64) -> Result<f32, SimError> {
    // Energy by the midpoint rule on a grid four times finer than the table.
    let half = span_symbols / 2.0;
    let m = 4 * RES;
    let steps = (span_symbols * m as f64).ceil() as usize;
    let dt = 1.0 / m as f64;
    let e: f64 = (0..steps)
        .map(|i| {
            let v = shape.value(-half + (i as f64 + 0.5) * dt);
            v * v * dt
        })
        .sum();
    if !(e > 0.0 && e.is_finite()) {
        return Param::Pulse.degenerate();
    }
    Ok((1.0 / e.sqrt()) as f32)
}

impl Linear {
    /// A modulator with pulse `shape` truncated to `span_symbols` (in all, centred), at `sps` output samples per symbol. Errors for
    /// a bad pulse parameter, a span not in `(0, 256]`, or `sps` not positive.
    pub fn new(shape: Shape, span_symbols: f64, sps: f64) -> Result<Self, SimError> {
        if !(span_symbols > 0.0 && span_symbols <= 256.0) {
            return Param::Span.range(0.0, 256.0, span_symbols);
        }
        if !(sps > 0.0 && sps.is_finite() && sps <= 1e6) {
            return Param::Sps.range(0.0, 1e6, sps);
        }
        shape.check()?;
        let half = span_symbols / 2.0;
        let scale = unit_energy_scale(shape, span_symbols)?;
        // The pulse at `RES` points per symbol from its beginning, with one point after its end. Between two points the value
        // is interpolated; at and after the last point the pulse is 0.
        let res = RES as f64;
        let points = (span_symbols * res).ceil() as usize + 2;
        let pulse: Vec<f32> = (0..points)
            .map(|i| shape.value(i as f64 / res - half) as f32)
            .collect();
        // One output sample takes one value per symbol under the pulse: the span, and half a symbol more for a delayed Q rail.
        let taps = (span_symbols + 0.5).floor() as usize + 2;
        let mut rows = vec![0f32; RES * 2 * taps];
        for (phase, row) in rows.chunks_exact_mut(2 * taps).enumerate() {
            let (value, step) = row.split_at_mut(taps);
            for (k, (v, s)) in value.iter_mut().zip(step).enumerate() {
                let i = phase + k * RES;
                if i + 1 < points {
                    *v = pulse[i] * scale;
                    *s = (pulse[i + 1] - pulse[i]) * scale;
                }
            }
        }
        let size = taps.next_power_of_two();
        Ok(Linear {
            half,
            sps,
            offset_q: 0.0,
            rows,
            taps,
            weights: vec![0.0; taps],
            history: vec![Complex32::default(); 2 * size],
            mask: size - 1,
            syms: 0,
            n: 0,
        })
    }

    /// A root-raised-cosine modulator, roll-off `beta`, `span` symbols.
    pub fn rrc(beta: f64, span: f64, sps: f64) -> Result<Self, SimError> {
        Self::new(Shape::Rrc { beta }, span, sps)
    }

    /// Delays the Q rail by half a symbol (offset QPSK).
    pub fn with_offset_q(mut self) -> Self {
        self.offset_q = 0.5;
        self
    }

    /// Output samples per symbol.
    pub fn sps(&self) -> f64 {
        self.sps
    }

    /// The output sample (fractional) at the centre of symbol `k`'s pulse (its I rail).
    pub fn symbol_centre(&self, k: u64) -> f64 {
        (k as f64 + self.half) * self.sps
    }

    /// The table phase of a pulse argument `d` symbols from the centre: the row, the first column and the fraction to the next
    /// row. `None` before the pulse begins.
    #[inline]
    fn phase(&self, d: f64) -> Option<(usize, usize, f32)> {
        let u = (d + self.half) * RES as f64;
        if u.is_nan() || u < 0.0 {
            return None;
        }
        let i = u as usize;
        Some((i % RES, i / RES, (u - i as f64) as f32))
    }

    /// The sum of `count` symbols from `start` in the history (newest first), each times its pulse value, when the newest
    /// symbol's centre is `d` symbols before the output sample. Each earlier symbol is one symbol further into the pulse, so
    /// all take their values from the same row.
    #[inline]
    fn shaped(&mut self, d: f64, start: usize, count: usize) -> Complex32 {
        // The newest symbol's pulse may not have begun (a rounding of `d`, or the delayed Q rail): the sum then starts at the
        // symbol before it.
        let (skip, (row, col, f)) = match self.phase(d) {
            Some(p) => (0, p),
            None => match self.phase(d + 1.0) {
                Some(p) => (1, p),
                None => return Complex32::default(),
            },
        };
        // The columns after the last are after the end of the pulse.
        let m = count
            .saturating_sub(skip)
            .min(self.taps.saturating_sub(col));
        let base = row * 2 * self.taps + col;
        let value = &self.rows[base..base + m];
        let step = &self.rows[base + self.taps..base + self.taps + m];
        let weights = &mut self.weights[..m];
        for (w, (&v, &s)) in weights.iter_mut().zip(value.iter().zip(step)) {
            *w = v + s * f;
        }
        Complex32::dot(weights, &self.history[start + skip..start + skip + m])
    }

    /// The output sample at `t` symbols, from the first `limit` symbols.
    #[inline]
    fn sample(&mut self, t: f64, limit: u64) -> Complex32 {
        let lo = (t - self.half - self.offset_q).ceil().max(0.0) as u64;
        let hi = ((t + self.half).floor() as i64).min(limit as i64 - 1);
        if hi < lo as i64 {
            return Complex32::default();
        }
        let hi = hi as u64;
        let count = ((hi - lo) as usize + 1).min(self.mask + 1);
        let start = self.mask - (hi as usize & self.mask);
        let d = t - hi as f64;
        let y = self.shaped(d, start, count);
        if self.offset_q == 0.0 {
            y
        } else {
            Complex32::new(y.re, self.shaped(d - self.offset_q, start, count).im)
        }
    }

    /// Appends every output sample the symbols so far determine.
    pub fn push(&mut self, symbols: &[Complex32], out: &mut Vec<Complex32>) {
        for &s in symbols {
            let at = self.mask - (self.syms as usize & self.mask);
            self.history[at] = s;
            self.history[at + self.mask + 1] = s;
            self.syms += 1;
            // Output n needs every symbol up to t + h = n / sps.
            loop {
                let u = self.n as f64 / self.sps;
                if u >= self.syms as f64 {
                    break;
                }
                let y = self.sample(u - self.half, self.syms);
                out.push(y);
                self.n += 1;
            }
        }
    }

    /// Appends the rest of the last pulse (as if zeros followed) and starts again from symbol 0.
    pub fn flush(&mut self, out: &mut Vec<Complex32>) {
        if self.syms > 0 {
            let end = (self.syms - 1) as f64 + 2.0 * self.half + self.offset_q;
            loop {
                let u = self.n as f64 / self.sps;
                if u >= end {
                    break;
                }
                let y = self.sample(u - self.half, self.syms);
                out.push(y);
                self.n += 1;
            }
        }
        self.reset();
    }

    /// Starts again from symbol 0, forgetting everything pushed.
    pub fn reset(&mut self) {
        self.syms = 0;
        self.n = 0;
        self.history.fill(Complex32::default());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rng::Rng;
    use sdrkit_algo_dsp::pulse::{taps, Norm, PulseTable};

    fn qpsk(rng: &mut Rng, n: usize) -> Vec<Complex32> {
        let a = std::f32::consts::FRAC_1_SQRT_2;
        (0..n)
            .map(|_| {
                Complex32::new(
                    if rng.bit() == 1 { a } else { -a },
                    if rng.bit() == 1 { a } else { -a },
                )
            })
            .collect()
    }

    #[test]
    fn rrc_symbols_come_back_through_a_matched_filter() {
        // Independent check: an RRC matched filter (taps from sdrkit-algo-dsp, unit energy at 8 sps) sampled at the symbol centres
        // recovers the symbols (RRC * RRC is Nyquist), with the power the energy scaling promises.
        let sps = 8.0;
        let mut m = Linear::rrc(0.35, 16.0, sps).unwrap();
        let syms = qpsk(&mut Rng::new(1), 2000);
        let mut y = Vec::new();
        m.push(&syms, &mut y);
        m.flush(&mut y);
        let p = y[200..y.len() - 200]
            .iter()
            .map(|v| v.norm_sqr() as f64)
            .sum::<f64>()
            / (y.len() - 400) as f64;
        assert!((p - 1.0).abs() < 0.02, "mean power {p}");
        let h = taps(Shape::Rrc { beta: 0.35 }, sps, 16.0, Norm::Energy).unwrap();
        let c = h.len() / 2;
        let g = 1.0 / sps.sqrt() as f32; // matched-filter gain: unit-energy taps on a pulse of energy sps samples
        let mut worst = 0.0f32;
        for (k, s) in syms.iter().enumerate().skip(20).take(1900) {
            let centre = m.symbol_centre(k as u64).round() as usize;
            let mut acc = Complex32::default();
            for (j, t) in h.iter().enumerate() {
                acc += y[centre + j - c] * *t;
            }
            worst = worst.max((acc * g - s).norm());
        }
        assert!(worst < 0.02, "worst symbol error {worst}");
    }

    #[test]
    fn fractional_samples_per_symbol_and_oqpsk_rails() {
        // At 3.7 sps with a rectangular pulse, sample n carries symbol floor(n / 3.7) (the half-open rectangle), and the Q rail of
        // OQPSK is half a symbol later.
        let sps = 3.7;
        let mut m = Linear::new(Shape::Rect, 1.0, sps).unwrap().with_offset_q();
        let syms = qpsk(&mut Rng::new(5), 300);
        let mut y = Vec::new();
        for c in syms.chunks(7) {
            m.push(c, &mut y);
        }
        m.flush(&mut y);
        for (n, v) in y.iter().enumerate().take(1000) {
            let t = n as f64 / sps;
            let i = syms[t.floor() as usize].re;
            assert!((v.re - i).abs() < 1e-5, "sample {n}");
            if t >= 0.5 {
                let q = syms[(t - 0.5).floor() as usize].im;
                assert!((v.im - q).abs() < 1e-5, "sample {n} Q");
            }
        }
        assert!(Linear::new(Shape::Rect, 0.0, 4.0).is_err());
        assert!(Linear::new(Shape::Rrc { beta: 2.0 }, 8.0, 4.0).is_err());
        assert!(Linear::new(Shape::Rect, 1.0, f64::NAN).is_err());
    }

    #[test]
    fn any_cut_returns_the_same_samples() {
        let syms = qpsk(&mut Rng::new(9), 500);
        let mut a = Linear::rrc(0.2, 10.0, 5.3).unwrap();
        let mut b = Linear::rrc(0.2, 10.0, 5.3).unwrap();
        let (mut ya, mut yb) = (Vec::new(), Vec::new());
        a.push(&syms, &mut ya);
        for c in syms.chunks(13) {
            b.push(c, &mut yb);
        }
        assert_eq!(ya, yb);
    }

    /// The modulator's definition, one pulse value per symbol and sample: `(samples of push, samples of flush)`.
    fn direct(
        shape: Shape,
        span: f64,
        sps: f64,
        offset: f64,
        syms: &[Complex32],
    ) -> (usize, Vec<Complex32>) {
        let half = span / 2.0;
        let table = PulseTable::new(shape, half, RES).unwrap();
        let scale = unit_energy_scale(shape, span).unwrap();
        let end = (syms.len() - 1) as f64 + span + offset;
        let mut y = Vec::new();
        let mut pushed = 0;
        loop {
            let u = y.len() as f64 / sps;
            if u >= end {
                break;
            }
            if u < syms.len() as f64 {
                pushed += 1;
            }
            let t = u - half;
            let (mut re, mut im) = (0.0f32, 0.0f32);
            for (k, s) in syms.iter().enumerate() {
                let d = t - k as f64;
                if d <= half + offset && d >= -half {
                    re += s.re * table.value(d);
                    im += s.im * table.value(d - offset);
                }
            }
            y.push(Complex32::new(re * scale, im * scale));
        }
        (pushed, y)
    }

    #[test]
    fn samples_equal_the_definition() {
        // Every sample is within 1e-5 of the RMS of the signal from the direct sum, for whole and fractional samples per symbol
        // (below 1 too), whole and fractional spans, the delayed Q rail, pushes cut anywhere, the tail of flush, and a second
        // run after flush. The symbols are 64-QAM-like levels with zeros among them.
        let cases = [
            (Shape::Rrc { beta: 0.15 }, 24.0, 8e6 / 6.952e6, false),
            (Shape::Rrc { beta: 0.35 }, 12.0, 4.0, false),
            (Shape::Rrc { beta: 0.35 }, 12.0, 8.0, true),
            (Shape::Rrc { beta: 0.2 }, 10.1, 5.3, false),
            (Shape::Rrc { beta: 0.5 }, 7.3, 0.7, true),
            (Shape::Rect, 1.0, 3.7, true),
            (Shape::Rect, 1.0, 4.0, false),
            (Shape::Rect, 2.5, 1.0, false),
        ];
        for (case, &(shape, span, sps, offset)) in cases.iter().enumerate() {
            let mut rng = Rng::new(20 + case as u64);
            let syms: Vec<Complex32> = (0..400)
                .map(|_| {
                    let mut level = || (0..3).fold(0, |a, _| 2 * a + rng.bit() as i32) as f32 - 3.5;
                    Complex32::new(level(), level()) * 0.2
                })
                .collect();
            let (pushed, want) = direct(shape, span, sps, if offset { 0.5 } else { 0.0 }, &syms);
            let rms =
                (want.iter().map(|v| v.norm_sqr() as f64).sum::<f64>() / want.len() as f64).sqrt();
            let mut m = Linear::new(shape, span, sps).unwrap();
            if offset {
                m = m.with_offset_q();
            }
            for run in 0..2 {
                let mut y = Vec::new();
                for c in syms.chunks(1 + 16 * run + case) {
                    m.push(c, &mut y);
                }
                assert_eq!(y.len(), pushed, "case {case} run {run}: samples of push");
                m.flush(&mut y);
                assert_eq!(y.len(), want.len(), "case {case} run {run}: samples in all");
                let worst = y
                    .iter()
                    .zip(&want)
                    .map(|(a, b)| (a - b).norm() as f64)
                    .fold(0.0, f64::max);
                assert!(
                    worst <= 1e-5 * rms,
                    "case {case} run {run}: worst error {worst}, RMS {rms}"
                );
            }
        }
    }
}
