//! The carrier line test: the share of a block's power that lies in its strongest run of a few adjacent bins of one
//! Hann-windowed FFT. A steady carrier puts most of its power into a few bins. White noise spreads it, so the best run of
//! `w` of `n` bins holds only a few times `w / n` of it (about `17 / n` for `w = 5`). A monitor that must not report a
//! carrier in noise asks for a share over a threshold (0.012 for an AM station: a carrier 15 dB under the rest of the
//! channel gives about 0.03).

use crate::fft::Fft;
use crate::window::Window;
use crate::Complex32;

/// The least and the largest block length.
pub const LEN_RANGE: (usize, usize) = (16, 1 << 20);

/// The share of the power of each block in its strongest run of adjacent bins.
#[derive(Clone, Debug)]
pub struct CarrierLine {
    width: usize,
    fft: Fft,
    window: Vec<f32>,
    buf: Vec<Complex32>,
    fill: usize,
    last: Option<f64>,
    blocks: u64,
}

impl CarrierLine {
    /// A test over consecutive blocks of `n` samples (a power of two; another value is rounded down to one, within
    /// [`LEN_RANGE`]) and runs of `width` adjacent bins (1 to `n`). The run wraps around the ends of the spectrum, so a
    /// carrier at 0 Hz counts whole.
    pub fn new(n: usize, width: usize) -> Self {
        let n = pow2_below(n.clamp(LEN_RANGE.0, LEN_RANGE.1));
        CarrierLine {
            width: width.clamp(1, n),
            fft: Fft::forward(n),
            window: Window::Hann.coefficients(n),
            buf: vec![Complex32::default(); n],
            fill: 0,
            last: None,
            blocks: 0,
        }
    }

    /// The block length for `seconds` of samples at `rate_hz`: the largest power of two not above `rate_hz * seconds`, kept to
    /// `min..=max` (both powers of two).
    pub fn block_len(rate_hz: f64, seconds: f64, min: usize, max: usize) -> usize {
        let n = (rate_hz * seconds).max(1.0) as usize;
        pow2_below(n).clamp(min, max)
    }

    /// The block length.
    pub fn len(&self) -> usize {
        self.buf.len()
    }

    /// Always false.
    pub fn is_empty(&self) -> bool {
        false
    }

    /// Takes samples. Each completed block replaces [`CarrierLine::share`].
    pub fn push(&mut self, x: &[Complex32]) {
        let n = self.buf.len();
        let mut rest = x;
        while !rest.is_empty() {
            let take = (n - self.fill).min(rest.len());
            self.buf[self.fill..self.fill + take].copy_from_slice(&rest[..take]);
            self.fill += take;
            rest = &rest[take..];
            if self.fill == n {
                self.fill = 0;
                self.last = Some(self.measure());
                self.blocks += 1;
            }
        }
    }

    /// The share of the last completed block, 0 to 1. `None` before the first block.
    pub fn share(&self) -> Option<f64> {
        self.last
    }

    /// Whether the share of the last block is over `threshold`.
    pub fn present(&self, threshold: f64) -> bool {
        self.last.is_some_and(|s| s > threshold)
    }

    /// Blocks measured.
    pub fn blocks(&self) -> u64 {
        self.blocks
    }

    /// Forgets the partial block and the last share.
    pub fn reset(&mut self) {
        self.fill = 0;
        self.last = None;
        self.blocks = 0;
    }

    fn measure(&mut self) -> f64 {
        for (z, w) in self.buf.iter_mut().zip(&self.window) {
            *z *= *w;
        }
        self.fft.process(&mut self.buf);
        let n = self.buf.len();
        let total: f64 = self.buf.iter().map(|z| f64::from(z.norm_sqr())).sum();
        if total <= 0.0 || !total.is_finite() {
            return 0.0;
        }
        let p = |i: usize| f64::from(self.buf[i % n].norm_sqr());
        let mut run: f64 = (0..self.width).map(p).sum();
        let mut best = run;
        for s in 1..n {
            run += p(s + self.width - 1) - p(s - 1);
            best = best.max(run);
        }
        (best / total).clamp(0.0, 1.0)
    }
}

/// The largest power of two not above `n` (at least 1).
fn pow2_below(n: usize) -> usize {
    if n <= 1 {
        1
    } else {
        1 << (usize::BITS - 1 - n.leading_zeros())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn noise(n: usize, seed: u64) -> Vec<Complex32> {
        let mut s = seed;
        let mut u = || {
            s ^= s << 13;
            s ^= s >> 7;
            s ^= s << 17;
            (s >> 11) as f64 / (1u64 << 53) as f64
        };
        (0..n)
            .map(|_| {
                let r = (-2.0 * u().max(1e-300).ln()).sqrt();
                let a = std::f64::consts::TAU * u();
                Complex32::new((r * a.cos()) as f32, (r * a.sin()) as f32)
            })
            .collect()
    }

    #[test]
    fn noise_reads_about_17_over_n_and_a_carrier_reads_most_of_the_block() {
        let n = 8192;
        let mut t = CarrierLine::new(n, 5);
        t.push(&noise(n, 3));
        let s = t.share().unwrap();
        assert!(s < 0.004 && s > 0.0007, "{s}");
        let mut c = CarrierLine::new(n, 5);
        let x: Vec<Complex32> = (0..n).map(|_| Complex32::new(1.0, 0.0)).collect();
        c.push(&x);
        assert!(c.share().unwrap() > 0.99);
        // A carrier 15 dB under the noise of the block.
        let a = 10f32.powf(-15.0 / 20.0);
        let y: Vec<Complex32> = noise(n, 5)
            .iter()
            .enumerate()
            .map(|(i, z)| z + Complex32::from_polar(a, 0.3 * i as f32))
            .collect();
        let mut d = CarrierLine::new(n, 5);
        d.push(&y);
        assert!(d.present(0.012), "{:?}", d.share());
    }

    #[test]
    fn the_length_is_a_power_of_two_and_pieces_give_the_same_share() {
        assert_eq!(CarrierLine::block_len(48e3, 0.4, 256, 32768), 16384);
        assert_eq!(CarrierLine::block_len(1e3, 0.4, 256, 32768), 256);
        assert_eq!(CarrierLine::new(1000, 5).len(), 512);
        let x = noise(4096, 9);
        let mut a = CarrierLine::new(1024, 5);
        let mut b = CarrierLine::new(1024, 5);
        a.push(&x);
        for c in x.chunks(37) {
            b.push(c);
        }
        assert_eq!(a.blocks(), 4);
        assert_eq!(a.share(), b.share());
    }
}
