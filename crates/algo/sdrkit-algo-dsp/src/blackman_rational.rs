//! A rational resampler by `up / down` with a Blackman-windowed sinc and zero delay, for complex streams.
//!
//! With `up` and `down` reduced by their greatest common divisor, `mx = max(up, down)` and `H = 6 mx`, tap `d + H` for `d` in
//! `-H ..= H` is `(up / mx) sinc(d / mx) (0.42 + 0.5 cos(pi d / H) + 0.08 cos(2 pi d / H))`. Output `m` is at input time
//! `m down / up`: with `pos = m down`, it is the sum of `tap[pos - k up + H] x[k]` over the inputs `k` whose tap index lies in
//! `0 ..= 2H`, in ascending `k`; inputs before 0 and after the end count as zero. `N` inputs give `floor(N up / down)` outputs.
//! When `up = down = 1` the output is the input.
//!
//! ```
//! use sdrkit_algo_dsp::blackman_rational::BlackmanRational;
//! use sdrkit_algo_dsp::Complex32;
//! let x = vec![Complex32::new(1.0, 0.0); 3000];
//! let y = BlackmanRational::convert(&x, 5, 1).unwrap();
//! assert_eq!(y.len(), 15_000);
//! assert!((y[7500].re - 1.0).abs() < 1e-3);
//! ```

use crate::{Complex32, DspError, Param};

fn gcd(a: u64, b: u64) -> u64 {
    if b == 0 {
        a
    } else {
        gcd(b, a % b)
    }
}

/// The full tap set of the rule for the reduced ratio `up / down`: `2 H + 1` taps, index `d + H`.
pub fn taps(up: usize, down: usize) -> Vec<f64> {
    let mx = up.max(down) as f64;
    let h = 6 * up.max(down);
    let hf = h as f64;
    let pi = std::f64::consts::PI;
    (0..=2 * h)
        .map(|i| {
            let d = i as f64 - hf;
            let u = d / mx;
            let sinc = if d == 0.0 { 1.0 } else { (pi * u).sin() / (pi * u) };
            let w = 0.42 + 0.5 * (pi * d / hf).cos() + 0.08 * (2.0 * pi * d / hf).cos();
            up as f64 / mx * sinc * w
        })
        .collect()
}

/// The streaming resampler.
#[derive(Clone, Debug)]
pub struct BlackmanRational {
    up: u64,
    down: u64,
    /// Taps per phase (the widest phase; narrower phases end in zeros).
    width: usize,
    /// For phase `p`, the taps of inputs `q + lo[p] ..` in ascending order, `width` each.
    bank: Vec<f32>,
    lo: Vec<i64>,
    /// The largest `lo[p] + width - 1`: how far past `q` an output reads.
    reach: i64,
    /// Kept input; `buf[0]` is input `base` (negative while the leading zeros are kept).
    buf: Vec<Complex32>,
    base: i64,
    /// Real inputs received.
    received: u64,
    /// The next output.
    m: u64,
    /// The stream has ended: no output at or after `floor(received up / down)`.
    ended: bool,
}

impl BlackmanRational {
    /// A resampler up by `up` and down by `down` (both at least 1; reduced by their greatest common divisor).
    ///
    /// # Errors
    ///
    /// [`DspError`] when `up` or `down` is 0.
    pub fn new(up: u64, down: u64) -> Result<BlackmanRational, DspError> {
        if up == 0 {
            return Param::Interpolation.at_least(1.0, 0.0);
        }
        if down == 0 {
            return Param::Decimation.at_least(1.0, 0.0);
        }
        let g = gcd(up, down);
        let (up, down) = (up / g, down / g);
        let h = 6 * up.max(down) as i64;
        let t = if up == 1 && down == 1 { vec![1.0] } else { taps(up as usize, down as usize) };
        let h = if t.len() == 1 { 0 } else { h };
        let upi = up as i64;
        // Phase p: k = q + i with i from ceil((p - H) / up) to floor((p + H) / up); tap index p - i up + H.
        let lo: Vec<i64> = (0..upi).map(|p| (p - h).div_euclid(upi) + i64::from((p - h).rem_euclid(upi) != 0)).collect();
        let hi: Vec<i64> = (0..upi).map(|p| (p + h).div_euclid(upi)).collect();
        let width = (0..up as usize).map(|p| (hi[p] - lo[p] + 1) as usize).max().unwrap_or(1);
        let mut bank = vec![0f32; up as usize * width];
        for p in 0..upi {
            for i in lo[p as usize]..=hi[p as usize] {
                let idx = p - i * upi + h;
                bank[p as usize * width + (i - lo[p as usize]) as usize] = t[idx as usize] as f32;
            }
        }
        let reach = (0..up as usize).map(|p| lo[p] + width as i64 - 1).max().unwrap_or(0);
        let lead = (-lo.iter().copied().min().unwrap_or(0)).max(0);
        Ok(BlackmanRational {
            up,
            down,
            width,
            bank,
            lo,
            reach,
            buf: vec![Complex32::new(0.0, 0.0); lead as usize],
            base: -lead,
            received: 0,
            m: 0,
            ended: false,
        })
    }

    /// The reduced `(up, down)`.
    pub fn ratio(&self) -> (u64, u64) {
        (self.up, self.down)
    }

    /// Taps a single output reads.
    pub fn width(&self) -> usize {
        self.width
    }

    /// Back to the start of a stream.
    pub fn reset(&mut self) {
        let lead = (-self.lo.iter().copied().min().unwrap_or(0)).max(0);
        self.buf.clear();
        self.buf.resize(lead as usize, Complex32::new(0.0, 0.0));
        self.base = -lead;
        self.received = 0;
        self.m = 0;
        self.ended = false;
    }

    /// Takes `x` and appends every output whose inputs have all arrived.
    pub fn process(&mut self, x: &[Complex32], out: &mut Vec<Complex32>) {
        if self.ended {
            self.reset();
        }
        self.buf.extend_from_slice(x);
        self.received += x.len() as u64;
        self.run(self.received as i64, u64::MAX, out);
    }

    /// Ends the stream: appends the outputs up to `floor(N up / down)` for the `N` inputs received, reading zeros after the end.
    pub fn flush(&mut self, out: &mut Vec<Complex32>) {
        let limit = (u128::from(self.received) * u128::from(self.up) / u128::from(self.down)) as u64;
        let pad = (self.reach + 2).max(0) as usize;
        self.buf.extend(std::iter::repeat_n(Complex32::new(0.0, 0.0), pad));
        let avail = self.received as i64 + pad as i64;
        self.run(avail, limit, out);
        self.ended = true;
    }

    /// Makes outputs while input `q + reach` is below `avail` and the output is below `limit`.
    fn run(&mut self, avail: i64, limit: u64, out: &mut Vec<Complex32>) {
        let w = self.width;
        while self.m < limit {
            let pos = u128::from(self.m) * u128::from(self.down);
            let q = (pos / u128::from(self.up)) as i64;
            let p = (pos % u128::from(self.up)) as usize;
            if q + self.reach >= avail {
                break;
            }
            let k0 = (q + self.lo[p] - self.base) as usize;
            let xs = &self.buf[k0..k0 + w];
            let ts = &self.bank[p * w..(p + 1) * w];
            let (mut re, mut im) = (0f32, 0f32);
            for (v, &t) in xs.iter().zip(ts) {
                re += v.re * t;
                im += v.im * t;
            }
            out.push(Complex32::new(re, im));
            self.m += 1;
        }
        // Drop the input that no later output reads.
        let pos = u128::from(self.m) * u128::from(self.down);
        let q = (pos / u128::from(self.up)) as i64;
        let first = q + self.lo.iter().copied().min().unwrap_or(0);
        let drop = (first - self.base).clamp(0, self.buf.len() as i64) as usize;
        if drop > 4096 || drop == self.buf.len() {
            self.buf.drain(..drop);
            self.base += drop as i64;
        }
    }

    /// Converts a whole signal: `floor(x.len() up / down)` outputs.
    ///
    /// # Errors
    ///
    /// [`DspError`] when `up` or `down` is 0.
    pub fn convert(x: &[Complex32], up: u64, down: u64) -> Result<Vec<Complex32>, DspError> {
        let mut r = BlackmanRational::new(up, down)?;
        let mut out = Vec::with_capacity((x.len() as u128 * u128::from(r.up) / u128::from(r.down)) as usize + 1);
        r.process(x, &mut out);
        r.flush(&mut out);
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The rule of the module header, summed directly.
    fn direct(x: &[Complex32], up: usize, down: usize) -> Vec<Complex32> {
        let g = gcd(up as u64, down as u64) as usize;
        let (up, down) = (up / g, down / g);
        if up == 1 && down == 1 {
            return x.to_vec();
        }
        let t = taps(up, down);
        let h = (t.len() - 1) as i64 / 2;
        let n = x.len() * up / down;
        (0..n)
            .map(|m| {
                let pos = (m * down) as i64;
                let lo = ((pos - h) as f64 / up as f64).ceil().max(0.0) as i64;
                let hi = (((pos + h) as f64 / up as f64).floor() as i64).min(x.len() as i64 - 1);
                let mut acc = Complex32::new(0.0, 0.0);
                for k in lo..=hi {
                    acc += x[k as usize] * t[(pos - k * up as i64 + h) as usize] as f32;
                }
                acc
            })
            .collect()
    }

    fn signal(n: usize) -> Vec<Complex32> {
        (0..n).map(|i| Complex32::new((0.013 * i as f32).sin(), (0.021 * i as f32 + 0.3).cos() * 0.5)).collect()
    }

    #[test]
    fn identity_and_lengths() {
        let x = signal(1000);
        assert_eq!(BlackmanRational::convert(&x, 3, 3).unwrap(), x);
        for (up, down) in [(5, 1), (2, 1), (1, 5), (4, 3), (2, 3), (4, 15), (64, 25), (3200, 9143)] {
            let y = BlackmanRational::convert(&x, up, down).unwrap();
            assert_eq!(y.len() as u64, 1000 * up / down, "{up}/{down}");
        }
        assert!(BlackmanRational::new(0, 1).is_err() && BlackmanRational::new(1, 0).is_err());
    }

    #[test]
    fn the_rule_streamed_in_pieces() {
        let x = signal(3000);
        for (up, down) in [(5, 1), (1, 2), (4, 3), (4, 15), (64, 25)] {
            let want = direct(&x, up, down);
            for piece in [1, 37, 1000, 3000] {
                let mut r = BlackmanRational::new(up as u64, down as u64).unwrap();
                let mut y = Vec::new();
                for c in x.chunks(piece) {
                    r.process(c, &mut y);
                }
                r.flush(&mut y);
                assert_eq!(y.len(), want.len(), "{up}/{down} {piece}");
                let err = y.iter().zip(&want).map(|(a, b)| (a - b).norm()).fold(0f32, f32::max);
                assert!(err < 1e-5, "{up}/{down} in pieces of {piece}: {err}");
            }
        }
    }

    #[test]
    fn zero_delay_and_unit_gain() {
        // A slow tone keeps its phase at the output time m down / up of the input.
        let w = 0.02f64;
        let x: Vec<Complex32> =
            (0..4000).map(|i| Complex32::new((w * i as f64).cos() as f32, (w * i as f64).sin() as f32)).collect();
        for (up, down) in [(5u64, 1u64), (4, 3), (1, 2), (2, 3)] {
            let y = BlackmanRational::convert(&x, up, down).unwrap();
            for m in [y.len() / 3, y.len() / 2] {
                let t = m as f64 * down as f64 / up as f64;
                let want = Complex32::new((w * t).cos() as f32, (w * t).sin() as f32);
                assert!((y[m] - want).norm() < 2e-3, "{up}/{down} at {m}: {:?}", y[m]);
            }
        }
        // The taps of each phase of an interpolator sum to one.
        let t = taps(5, 1);
        for p in 0..5 {
            let s: f64 = t.iter().skip(p).step_by(5).sum();
            assert!((s - 1.0).abs() < 1e-3, "{p}: {s}");
        }
    }
}
