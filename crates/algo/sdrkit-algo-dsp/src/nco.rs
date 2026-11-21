//! Numerically controlled oscillator and frequency shifter.
//!
//! The phase is an `f64` accumulator in cycles. Every 64 samples, counted from the oscillator's start, the oscillator restarts from
//! the exact phase, so rounding does not accumulate and the output is bit-exact however the stream is split into calls. A frequency
//! or phase change takes effect at the sample it is made at.

use crate::Complex32;
use std::f64::consts::TAU;

const CHUNK: usize = 64;

#[inline]
fn exact(cycles: f64) -> Complex32 {
    let (s, c) = (TAU * cycles).sin_cos();
    Complex32::new(c as f32, s as f32)
}

/// An oscillator `e^{+j 2 pi f t}`. To move a signal at `+f` Hz down to DC, mix it with an `Nco` at `-f`.
#[derive(Clone, Debug)]
pub struct Nco {
    rate: f64,
    freq: f64,
    /// Cycles per sample.
    inc: f64,
    /// The exact phase (cycles) of the sample at `base_pos` in the current chunk.
    base: f64,
    base_pos: usize,
    /// The position in the current chunk of the next sample (`CHUNK` when the chunk is used up).
    pos: usize,
    /// `lanes[j]`: the phasor of the next sample whose position in the chunk is `j` modulo 4.
    lanes: [Complex32; 4],
    /// One step, and four.
    rot: Complex32,
    rot4: Complex32,
}

impl Nco {
    /// An oscillator at `freq_hz` for samples at `rate_hz`, starting at phase 0.
    pub fn new(rate_hz: f64, freq_hz: f64) -> Self {
        assert!(rate_hz > 0.0, "rate must be positive");
        let mut n = Nco {
            rate: rate_hz,
            freq: 0.0,
            inc: 0.0,
            base: 0.0,
            base_pos: 0,
            pos: 0,
            lanes: [Complex32::new(1.0, 0.0); 4],
            rot: Complex32::new(1.0, 0.0),
            rot4: Complex32::new(1.0, 0.0),
        };
        n.set_freq(freq_hz);
        n
    }

    /// The exact phase (cycles, not reduced) of the sample at position `p` of the current chunk.
    #[inline]
    fn phase_at(&self, p: usize) -> f64 {
        self.base + (p - self.base_pos) as f64 * self.inc
    }

    /// Sets the lanes for the samples from `pos` on: the exact phasor at `pos`, then one rotation per sample.
    fn set_lanes(&mut self) {
        let mut z = exact(self.phase_at(self.pos));
        for k in 0..4 {
            self.lanes[(self.pos + k) % 4] = z;
            z *= self.rot;
        }
    }

    /// Starts the next chunk from the exact phase.
    #[inline]
    fn next_chunk(&mut self) {
        self.base = self.phase_at(CHUNK).rem_euclid(1.0);
        self.base_pos = 0;
        self.pos = 0;
        self.set_lanes();
    }

    /// Changes the frequency from the next sample on. The phase carries on from where it is: no jump.
    pub fn set_freq(&mut self, freq_hz: f64) {
        self.base = self.phase_at(self.pos).rem_euclid(1.0);
        self.base_pos = self.pos;
        self.freq = freq_hz;
        self.inc = freq_hz / self.rate;
        self.rot = exact(self.inc);
        self.rot4 = exact(4.0 * self.inc);
        self.set_lanes();
    }

    /// The frequency in Hz.
    pub fn freq(&self) -> f64 {
        self.freq
    }

    /// The sample rate in Hz.
    pub fn rate(&self) -> f64 {
        self.rate
    }

    /// The phase of the next sample, in cycles [0, 1).
    pub fn phase_cycles(&self) -> f64 {
        self.phase_at(self.pos).rem_euclid(1.0)
    }

    /// Sets the phase of the next sample, in cycles.
    pub fn set_phase_cycles(&mut self, cycles: f64) {
        self.base = cycles.rem_euclid(1.0);
        self.base_pos = self.pos;
        self.set_lanes();
    }

    /// Writes the next `out.len()` oscillator samples.
    #[inline]
    fn generate(&mut self, out: &mut [Complex32]) {
        let rot4 = self.rot4;
        let n = out.len();
        let mut i = 0;
        while i < n {
            if self.pos == CHUNK {
                self.next_chunk();
            }
            let k = (CHUNK - self.pos).min(n - i);
            let seg = &mut out[i..i + k];
            i += k;
            let mut lanes = self.lanes;
            let mut p = self.pos;
            // Up to the next multiple of four, one by one; then four at a time; then the rest.
            let head = ((4 - p % 4) % 4).min(k);
            let (first, rest) = seg.split_at_mut(head);
            for y in first {
                *y = lanes[p % 4];
                lanes[p % 4] *= rot4;
                p += 1;
            }
            let body = rest.len() / 4 * 4;
            let (groups, tail) = rest.as_chunks_mut::<4>();
            for g in groups {
                g.copy_from_slice(&lanes);
                for l in lanes.iter_mut() {
                    *l *= rot4;
                }
            }
            p += body;
            for y in tail {
                *y = lanes[p % 4];
                lanes[p % 4] *= rot4;
                p += 1;
            }
            self.lanes = lanes;
            self.pos = p;
        }
    }

    /// The next oscillator sample.
    pub fn next_sample(&mut self) -> Complex32 {
        let mut y = [Complex32::default()];
        self.generate(&mut y);
        y[0]
    }

    /// Fills `out` with oscillator samples.
    pub fn fill(&mut self, out: &mut [Complex32]) {
        self.generate(out);
    }

    /// Runs `f` on pieces of `n` samples with the oscillator for each (made on the stack, 256 at a time).
    #[inline]
    fn blocks(&mut self, n: usize, mut f: impl FnMut(usize, &[Complex32])) {
        let mut osc = [Complex32::default(); 256];
        let mut i = 0;
        while i < n {
            let k = (n - i).min(osc.len());
            self.generate(&mut osc[..k]);
            f(i, &osc[..k]);
            i += k;
        }
    }

    /// `out[i] = input[i] * osc[i]`: shifts the spectrum of `input` by `freq()`.
    pub fn mix(&mut self, input: &[Complex32], out: &mut [Complex32]) {
        assert_eq!(input.len(), out.len());
        self.blocks(input.len(), |i, o| {
            for ((y, x), o) in out[i..i + o.len()].iter_mut().zip(&input[i..]).zip(o) {
                *y = *x * *o;
            }
        });
    }

    /// [`mix`](Self::mix) in place.
    pub fn mix_in_place(&mut self, buf: &mut [Complex32]) {
        self.blocks(buf.len(), |i, o| {
            for (y, o) in buf[i..i + o.len()].iter_mut().zip(o) {
                *y *= *o;
            }
        });
    }

    /// A real signal times the complex oscillator: `out[i] = input[i] * osc[i]` (a complex result).
    pub fn mix_real(&mut self, input: &[f32], out: &mut [Complex32]) {
        assert_eq!(input.len(), out.len());
        self.blocks(input.len(), |i, o| {
            for ((y, x), o) in out[i..i + o.len()].iter_mut().zip(&input[i..]).zip(o) {
                *y = *o * *x;
            }
        });
    }

    /// A real mixer: `out[i] = input[i] * cos(phase[i])`, a real result (sum and difference products, each at half amplitude).
    pub fn mix_real_cos(&mut self, input: &[f32], out: &mut [f32]) {
        assert_eq!(input.len(), out.len());
        self.blocks(input.len(), |i, o| {
            for ((y, x), o) in out[i..i + o.len()].iter_mut().zip(&input[i..]).zip(o) {
                *y = *x * o.re;
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::*;

    #[test]
    fn frequency_and_phase_are_exact_after_a_second() {
        let rate = 2.4e6;
        let f = 123_456.789;
        let mut nco = Nco::new(rate, f);
        let mut buf = vec![Complex32::default(); 2_400_000];
        nco.fill(&mut buf[..1000]);
        nco.fill(&mut buf[1000..1001]);
        nco.fill(&mut buf[1001..]);
        let n = buf.len() - 1;
        let want = (f * n as f64 / rate).rem_euclid(1.0) * TAU;
        let got = (buf[n].im as f64).atan2(buf[n].re as f64);
        let mut err = (got - want).rem_euclid(TAU);
        if err > std::f64::consts::PI {
            err -= TAU;
        }
        assert!(err.abs() < 1e-4, "phase error {err} rad after 1 s");
        let worst = buf
            .iter()
            .map(|v| (v.norm() - 1.0).abs())
            .fold(0.0f32, f32::max);
        assert!(worst < 1e-5, "magnitude error {worst}");
    }

    #[test]
    fn mixing_moves_a_tone() {
        let x = ctone(4096, 0.1, 1.0);
        let mut y = vec![Complex32::default(); 4096];
        Nco::new(1.0, -0.1).mix(&x, &mut y);
        assert!((camp(&y, 0.0) - 1.0).abs() < 1e-5);
        let r = rtone(4096, 0.05, 1.0);
        Nco::new(1.0, -0.05).mix_real(&r, &mut y);
        // cos = (e^{+} + e^{-}) / 2: half lands at DC, half at -0.1.
        assert!((camp(&y, 0.0) - 0.5).abs() < 1e-5 && (camp(&y, -0.1) - 0.5).abs() < 1e-5);
    }

    #[test]
    fn the_output_does_not_depend_on_the_cuts() {
        let x = ctone(10_000, 0.0123, 0.7);
        let mut whole = vec![Complex32::default(); x.len()];
        let mut a = Nco::new(2.4e6, 345_678.9);
        a.mix(&x, &mut whole);
        for piece in [1usize, 3, 63, 64, 65, 100, 4097] {
            let mut b = Nco::new(2.4e6, 345_678.9);
            let mut y = x.clone();
            for c in y.chunks_mut(piece) {
                b.mix_in_place(c);
            }
            assert_eq!(y, whole, "pieces of {piece}");
        }
        // A change of frequency at the same sample produces the same output, however the input is split around it.
        let run = |cuts: &[usize]| {
            let mut n = Nco::new(1e6, 1000.0);
            let mut y = vec![Complex32::default(); 1000];
            let mut at = 0;
            for &c in cuts {
                n.fill(&mut y[at..c]);
                if c == 333 {
                    n.set_freq(-25_000.0);
                }
                at = c;
            }
            n.fill(&mut y[at..]);
            y
        };
        assert_eq!(run(&[333]), run(&[5, 70, 333, 334, 900]));
    }

    #[test]
    fn retune_keeps_the_phase_continuous() {
        let mut nco = Nco::new(48_000.0, 1000.0);
        let mut a = vec![Complex32::default(); 100];
        nco.fill(&mut a);
        nco.set_freq(5000.0);
        let mut b = vec![Complex32::default(); 2];
        nco.fill(&mut b);
        // The first sample after the retune continues one old step after the last one; the next one uses the new step.
        let step = |p: Complex32, q: Complex32| (q * p.conj()).arg() as f64 / TAU * 48_000.0;
        assert!((step(a[99], b[0]) - 1000.0).abs() < 0.1);
        assert!((step(b[0], b[1]) - 5000.0).abs() < 0.1);
    }
}
