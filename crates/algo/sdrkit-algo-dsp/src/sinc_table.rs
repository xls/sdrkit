//! A sample-rate converter for complex streams with a tabulated Blackman-windowed sinc, 256 table points per input sample.
//!
//! `step = in / out`, `cut = min(0.95, 0.95 out / in)`, `half = ceil(16 / cut)`. Table entry `i` of `2 half 256 + 1` is
//! `cut sinc(cut x) w(x / half)` with `x = i / 256 - half` and the Blackman window `w(v) = 0.42 + 0.5 cos(pi v) + 0.08 cos(2 pi v)`.
//! Output `j` is the input at time `t_j` (`t` starts at 0 and grows by `step`): the sum over `i` from `floor(t_j) - half + 1` to
//! `floor(t_j) + half` of `table[round((i - t_j + half) 256)] x[i]`, with no delay compensation. Inputs before 0 count as zero.
//! Output `j` is made once input `floor(t_j) + half` has arrived; [`SincConverter::flush`] feeds `2 half + 2` zeros to end a stream.
//!
//! ```
//! use sdrkit_algo_dsp::sinc_table::SincConverter;
//! use sdrkit_algo_dsp::Complex32;
//! let mut x = vec![Complex32::new(0.0, 0.0); 1000];
//! x[500] = Complex32::new(1.0, 0.0);
//! let y = SincConverter::convert(&x, 640e3, 320e3).unwrap();
//! assert_eq!(y.len(), 518);
//! assert!((y[250].re - 0.475).abs() < 1e-6);
//! ```

use crate::{Complex32, DspError, Param};

/// Table points per input sample.
const PER_SAMPLE: usize = 256;

/// The streaming converter.
#[derive(Clone, Debug)]
pub struct SincConverter {
    step: f64,
    half: usize,
    table: Vec<f32>,
    /// Kept input; `buf[0]` is input `base`.
    buf: Vec<Complex32>,
    base: u64,
    /// Inputs received.
    received: u64,
    /// The time of the next output, input samples.
    t: f64,
}

impl SincConverter {
    /// A converter from `in_rate` to `out_rate` (both positive and finite).
    pub fn new(in_rate: f64, out_rate: f64) -> Result<SincConverter, DspError> {
        if !(in_rate.is_finite() && in_rate > 0.0) {
            return Param::InputRate.positive(in_rate);
        }
        if !(out_rate.is_finite() && out_rate > 0.0) {
            return Param::OutputRate.positive(out_rate);
        }
        let step = in_rate / out_rate;
        let cut = (0.95 * out_rate / in_rate).min(0.95);
        let half = (16.0 / cut).ceil() as usize;
        let table = (0..=2 * half * PER_SAMPLE)
            .map(|i| {
                let x = i as f64 / PER_SAMPLE as f64 - half as f64;
                let u = cut * x;
                let sinc = if u.abs() < 1e-12 {
                    1.0
                } else {
                    (std::f64::consts::PI * u).sin() / (std::f64::consts::PI * u)
                };
                let w = x / half as f64;
                let win = 0.42 + 0.5 * (std::f64::consts::PI * w).cos() + 0.08 * (std::f64::consts::TAU * w).cos();
                (cut * sinc * win) as f32
            })
            .collect();
        Ok(SincConverter {
            step,
            half,
            table,
            buf: Vec::new(),
            base: 0,
            received: 0,
            t: 0.0,
        })
    }

    /// The kernel half length, input samples.
    pub fn half(&self) -> usize {
        self.half
    }

    /// Input samples per output sample.
    pub fn step(&self) -> f64 {
        self.step
    }

    /// Back to the start of a stream.
    pub fn reset(&mut self) {
        self.buf.clear();
        self.base = 0;
        self.received = 0;
        self.t = 0.0;
    }

    /// Takes the input `x` and appends every output that it completes to `out`.
    pub fn process(&mut self, x: &[Complex32], out: &mut Vec<Complex32>) {
        self.buf.extend_from_slice(x);
        self.received += x.len() as u64;
        let half = self.half as i64;
        let last = self.table.len() as i64;
        loop {
            let i0 = self.t.floor() as i64;
            if i0 + half >= self.received as i64 {
                break;
            }
            let mut acc = Complex32::new(0.0, 0.0);
            for i in (i0 - half + 1).max(0)..=i0 + half {
                let k = ((i as f64 - self.t + half as f64) * PER_SAMPLE as f64).round() as i64;
                if (0..last).contains(&k) {
                    acc += self.buf[(i as u64 - self.base) as usize] * self.table[k as usize];
                }
            }
            out.push(acc);
            self.t += self.step;
        }
        // Drop the input that no later output reads.
        let keep_from = (self.t.floor() as i64 - half + 1).max(0) as u64;
        if keep_from > self.base {
            let drop = ((keep_from - self.base) as usize).min(self.buf.len());
            self.buf.drain(..drop);
            self.base += drop as u64;
        }
    }

    /// Ends the stream: feeds `2 half + 2` zero samples and appends the outputs.
    pub fn flush(&mut self, out: &mut Vec<Complex32>) {
        let z = vec![Complex32::new(0.0, 0.0); 2 * self.half + 2];
        self.process(&z, out);
    }

    /// Converts a whole signal: all of `x`, then the flush.
    pub fn convert(x: &[Complex32], in_rate: f64, out_rate: f64) -> Result<Vec<Complex32>, DspError> {
        let mut c = SincConverter::new(in_rate, out_rate)?;
        let mut out = Vec::with_capacity((x.len() as f64 / c.step) as usize + 2 * c.half + 4);
        c.process(x, &mut out);
        c.flush(&mut out);
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kernel_lengths_and_an_impulse() {
        assert_eq!(SincConverter::new(640e3, 320e3).unwrap().half(), 34);
        assert_eq!(SincConverter::new(640e3, 1e6).unwrap().half(), 17);
        let mut x = vec![Complex32::new(0.0, 0.0); 1000];
        x[500] = Complex32::new(1.0, 0.0);
        let y = SincConverter::convert(&x, 640e3, 320e3).unwrap();
        assert_eq!(y.len(), 518);
        assert!((y[250].re - 0.475).abs() < 1e-6);
        assert!((y[249].re - 0.024551).abs() < 1e-6 && (y[251].re - 0.024551).abs() < 1e-6);
    }

    #[test]
    fn counts_and_pieces() {
        // 36 289 inputs at 640 ksps: outputs before and with the flush.
        let x: Vec<Complex32> = (0..36_289).map(|i| Complex32::new((i as f32 * 0.01).sin(), 0.0)).collect();
        for (rate, before, with) in [(320e3, 18_128, 18_163), (250e3, 14_159, 14_194), (1e6, 56_676, 56_732)] {
            let mut c = SincConverter::new(640e3, rate).unwrap();
            let mut y = Vec::new();
            c.process(&x, &mut y);
            assert_eq!(y.len(), before, "{rate}");
            c.flush(&mut y);
            assert_eq!(y.len(), with, "{rate}");
            // The same output in pieces of 777.
            let mut c = SincConverter::new(640e3, rate).unwrap();
            let mut z = Vec::new();
            for p in x.chunks(777) {
                c.process(p, &mut z);
            }
            c.flush(&mut z);
            assert_eq!(y, z);
        }
    }
}
