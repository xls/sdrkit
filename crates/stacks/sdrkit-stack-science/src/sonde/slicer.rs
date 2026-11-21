//! The FSK symbol slicer: a boxcar over one symbol, a zero-crossing timing loop and a hard decision at mid-symbol.

/// One decision of a [`Slicer`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Decision {
    /// The boxcar at the decision instant.
    pub soft: f32,
    /// 0 when the instant lies nearer the sample just pushed, 1 when nearer the one before. A window of one symbol that ends this
    /// many samples back covers the symbol decided.
    pub back: usize,
}

/// One slicer.
#[derive(Clone, Debug)]
pub struct Slicer {
    sps_nom: f64,
    step: f64,
    step_nom: f64,
    boxbuf: Vec<f32>,
    bpos: usize,
    sum: f64,
    phase: f64,
    prev_y: f32,
    transitions: u32,
    filled: usize,
}

impl Slicer {
    /// For `symbol_rate` at a sample rate of `rate`.
    pub fn new(rate: f64, symbol_rate: f64) -> Slicer {
        let sps = rate / symbol_rate;
        let l = (sps.round() as usize).max(1);
        Slicer {
            sps_nom: sps,
            step: 1.0 / sps,
            step_nom: 1.0 / sps,
            boxbuf: vec![0.0; l],
            bpos: 0,
            sum: 0.0,
            phase: 0.0,
            prev_y: 0.0,
            transitions: 0,
            filled: 0,
        }
    }

    /// Samples per symbol (nominal).
    pub fn sps(&self) -> f64 {
        self.sps_nom
    }

    /// The symbol clock now relative to nominal (1.0 = nominal).
    pub fn clock(&self) -> f64 {
        self.step / self.step_nom
    }

    /// Restarts timing.
    pub fn reset(&mut self) {
        self.boxbuf.iter_mut().for_each(|v| *v = 0.0);
        self.bpos = 0;
        self.sum = 0.0;
        self.phase = 0.0;
        self.prev_y = 0.0;
        self.transitions = 0;
        self.filled = 0;
        self.step = self.step_nom;
    }

    /// Pushes one sample (the discriminator output minus its threshold). Returns a decision when one falls in this sample.
    #[inline]
    pub fn push(&mut self, x: f32) -> Option<Decision> {
        let l = self.boxbuf.len();
        self.sum += (x - self.boxbuf[self.bpos]) as f64;
        self.boxbuf[self.bpos] = x;
        self.bpos += 1;
        if self.bpos == l {
            self.bpos = 0;
            // Refresh the running sum now and then so rounding does not build up.
            self.sum = self.boxbuf.iter().map(|&v| v as f64).sum();
        }
        if self.filled < l {
            self.filled += 1;
        }
        let y = (self.sum / l as f64) as f32;
        let p0 = self.phase;
        let mut p1 = p0 + self.step;
        let mut out = None;
        // A zero crossing: the edge belongs at phase 0.
        if self.filled >= l && (self.prev_y < 0.0) != (y < 0.0) && self.prev_y != y {
            let frac = (self.prev_y / (self.prev_y - y)) as f64;
            let mut pc = p0 + frac * self.step;
            pc -= pc.round();
            let g = 0.04 + 0.4 / (1.0 + 0.25 * self.transitions as f64);
            self.transitions = self.transitions.saturating_add(1);
            p1 -= g * pc;
            self.step -= 0.002 * g * pc * self.step_nom;
            self.step = self.step.clamp(self.step_nom * 0.98, self.step_nom * 1.02);
        }
        // The decision at phase 0.5.
        if p0 < 0.5 && p1 >= 0.5 {
            let frac = ((0.5 - p0) / (p1 - p0)).clamp(0.0, 1.0) as f32;
            out = Some(Decision {
                soft: self.prev_y + (y - self.prev_y) * frac,
                back: usize::from(frac < 0.5),
            });
        }
        if p1 >= 1.0 {
            p1 -= 1.0;
        } else if p1 < 0.0 {
            p1 += 1.0;
        }
        self.phase = p1;
        self.prev_y = y;
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recovers_nrz_symbols_with_a_clock_error() {
        let rate = 100_000.0;
        let sr = 4800.0 * 1.01;
        let mut s = Slicer::new(rate, 4800.0);
        let mut rng = sdrkit_algo_sim::rng::Rng::new(3);
        let syms: Vec<bool> = (0..2000).map(|_| rng.below(2) == 1).collect();
        let mut out = Vec::new();
        let n = (syms.len() as f64 * rate / sr) as usize;
        for i in 0..n {
            let k = (i as f64 * sr / rate) as usize;
            let v = if syms[k.min(syms.len() - 1)] {
                1.0
            } else {
                -1.0
            };
            if let Some(d) = s.push(v) {
                out.push(d.soft > 0.0);
            }
        }
        // Find the alignment and count errors after the loop settles.
        let tail = &out[200..1800];
        let best = (0..40)
            .map(|off| {
                tail.iter()
                    .zip(&syms[200 - 20 + off..])
                    .filter(|(a, b)| a != b)
                    .count()
            })
            .min()
            .unwrap();
        assert!(best < 5, "{best}");
    }
}
