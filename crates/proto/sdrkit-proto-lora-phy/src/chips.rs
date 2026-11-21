//! The chip stream of one channel, as two rings: the channel at twice the chip rate, read at fractional positions through a
//! windowed-sinc interpolator, and the same filter output at the chip rate for the preamble search.

use crate::error::{LoraError, Result};
use sdrkit_algo_dsp::interp::{Interpolator, Polyphase};
use sdrkit_algo_dsp::Sample;
use sdrkit_core::Complex32;

/// The taps of the interpolator.
pub const TAPS: usize = 32;
const PHASES: usize = 64;

/// The interpolator: a 32-tap Kaiser-windowed sinc for about 50 dB, cut-off at half the Nyquist rate of the twice-rate ring.
type Interp = Polyphase<TAPS, PHASES>;

/// The two rings of a channel.
#[derive(Clone, Debug)]
pub struct ChipRings {
    /// The samples at twice the chip rate.
    pub x2: Vec<Complex32>,
    /// The absolute index of `x2[0]`.
    pub x2_first: u64,
    /// The chips: the filter output at the even positions of `x2`.
    pub x1: Vec<Complex32>,
    /// The absolute index of `x1[0]`. Chip `m` is at `x2` position `2 m`.
    pub x1_first: u64,
    x1_next: u64,
    interp: Interp,
}

impl ChipRings {
    /// Empty rings.
    pub fn new() -> Result<Self> {
        let interp =
            Interp::new(0.5, sdrkit_algo_dsp::fir::design_beta(50.0)).map_err(LoraError::Dsp)?;
        Ok(ChipRings {
            x2: Vec::new(),
            x2_first: 0,
            x1: Vec::new(),
            x1_first: 0,
            x1_next: 0,
            interp,
        })
    }

    /// Forgets all samples. The next sample appended has the index `at` (at twice the chip rate).
    pub fn reset(&mut self, at: u64) {
        self.x2.clear();
        self.x1.clear();
        self.x2_first = at;
        self.x1_first = at.div_ceil(2) + TAPS as u64 / 2;
        self.x1_next = self.x1_first;
    }

    /// Reserves room for `x2` samples at twice the chip rate, so the rings do not grow while they hold no more.
    pub fn reserve(&mut self, x2: usize) {
        self.x2.reserve(x2.saturating_sub(self.x2.len()));
        self.x1.reserve((x2 / 2).saturating_sub(self.x1.len()));
    }

    /// The end (exclusive) of the twice-rate samples.
    pub fn x2_end(&self) -> u64 {
        self.x2_first + self.x2.len() as u64
    }

    /// The end (exclusive) of the chips.
    pub fn x1_end(&self) -> u64 {
        self.x1_first + self.x1.len() as u64
    }

    /// Appends samples at twice the chip rate, and makes the chips they complete.
    pub fn push(&mut self, x: &[Complex32]) {
        self.x2.extend_from_slice(x);
        let row = self.interp.row(0);
        let end = self.x2_end();
        // Chip m needs x2 up to 2 m + TAPS / 2.
        while 2 * self.x1_next + (TAPS as u64) / 2 < end {
            let start = 2 * self.x1_next + 1 - TAPS as u64 / 2;
            if start < self.x2_first {
                self.x1.push(Complex32::default());
            } else {
                let i = (start - self.x2_first) as usize;
                self.x1
                    .push(<Complex32 as Sample>::dot(row, &self.x2[i..i + TAPS]));
            }
            self.x1_next += 1;
        }
    }

    /// Drops what lies before `x2` position `keep`, and the chips before `keep / 2`.
    pub fn trim(&mut self, keep: u64) {
        if keep > self.x2_first + 4096 {
            let n = ((keep - self.x2_first) as usize).min(self.x2.len());
            self.x2.drain(..n);
            self.x2_first += n as u64;
        }
        let k1 = keep / 2;
        if k1 > self.x1_first + 2048 {
            let n = ((k1 - self.x1_first) as usize).min(self.x1.len());
            self.x1.drain(..n);
            self.x1_first += n as u64;
        }
    }

    /// Copies `n` chips from chip `start` into `out`, conjugated if `conj`. If they are not all there, this returns false.
    pub fn chips(&self, start: u64, n: usize, conj: bool, out: &mut [Complex32]) -> bool {
        if out.len() < n || start < self.x1_first || start + n as u64 > self.x1_end() {
            return false;
        }
        let i = (start - self.x1_first) as usize;
        if conj {
            for (o, v) in out[..n].iter_mut().zip(&self.x1[i..i + n]) {
                *o = v.conj();
            }
        } else {
            out[..n].copy_from_slice(&self.x1[i..i + n]);
        }
        true
    }

    /// True if the chips at `start + k step` (chip units), `k < n`, can be made now.
    pub fn has(&self, start: f64, step: f64, n: usize) -> bool {
        let first = 2.0 * start - TAPS as f64;
        let last = 2.0 * (start + step * n as f64) + TAPS as f64;
        first.is_finite()
            && last.is_finite()
            && first >= self.x2_first as f64
            && last < self.x2_end() as f64
    }

    /// Writes the `n` chips at `start + k step` (chip units) into `out`, interpolated from the twice-rate ring and conjugated if
    /// `conj`. If they are not all there, this returns false.
    pub fn interpolate(
        &self,
        start: f64,
        step: f64,
        n: usize,
        conj: bool,
        out: &mut [Complex32],
    ) -> bool {
        if out.len() < n || !self.has(start, step, n) {
            return false;
        }
        let half = TAPS as i64 / 2;
        let base = self.x2_first as i64;
        for (k, o) in out[..n].iter_mut().enumerate() {
            let t = 2.0 * (start + step * k as f64);
            let fl = t.floor();
            let i = (fl as i64 - half + 1 - base) as usize;
            let v = self.interp.at(&self.x2[i..i + TAPS], (t - fl) as f32);
            *o = if conj { v.conj() } else { v };
        }
        true
    }
}
