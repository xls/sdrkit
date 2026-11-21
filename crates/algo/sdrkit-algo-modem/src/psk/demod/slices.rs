//! The sample path of [`PskDemodulator`]: the mixer over the samples that the next run of the timing loop waits for, the
//! timing loop, the carrier loop over each run, and the symbols to the caller run by run.

use super::{PskDemodulator, PskOrder, PIECE, RUN};
use crate::Complex32;
use sdrkit_algo_dsp::nco::Nco;
use std::ops::ControlFlow;

impl PskDemodulator {
    /// Takes the samples of `x` in order and appends the symbols, carrier corrected, to `out`.
    pub fn process(&mut self, x: &[Complex32], out: &mut Vec<Complex32>) {
        self.process_with(x, |_, z| {
            out.extend_from_slice(z);
            ControlFlow::Continue(())
        });
    }

    /// Takes the samples of `x` in order and calls `on_symbols` with the demodulator and each run of symbols (1 or more,
    /// carrier corrected). `on_symbols` may change the mixer ([`set_mixer_offset`], [`hand_over`], [`retune_after`]), the
    /// carrier loop, or [`reset`] the demodulator; the samples after those of the run then see the change, however the
    /// input is split across calls. After [`ControlFlow::Break`] no further sample is taken.
    ///
    /// Returns the samples taken: `x.len()`, or those up to the last sample that the run that broke needed. Symbols that
    /// the timing loop made with that run stay for the next call.
    ///
    /// [`set_mixer_offset`]: PskDemodulator::set_mixer_offset
    /// [`hand_over`]: PskDemodulator::hand_over
    /// [`retune_after`]: PskDemodulator::retune_after
    /// [`reset`]: PskDemodulator::reset
    pub fn process_with(
        &mut self,
        x: &[Complex32],
        mut on_symbols: impl FnMut(&mut Self, &[Complex32]) -> ControlFlow<()>,
    ) -> usize {
        let mut mixed = [Complex32::new(0.0, 0.0); PIECE];
        let mut run = [Complex32::new(0.0, 0.0); RUN];
        let mut i = 0;
        'pieces: loop {
            // Symbols that a call before left.
            while self.at < self.syms.len() {
                let m = self.carrier_run(&mut run);
                if on_symbols(self, &run[..m]).is_break() {
                    return i;
                }
            }
            let mut n = (x.len() - i).min(PIECE);
            if let Some((k, c)) = self.pending {
                if k == 0 {
                    self.nco.set_freq(self.nco.freq() + c);
                    self.pending = None;
                } else {
                    n = n.min(k);
                }
            }
            if n == 0 && self.timing.need() > 0 {
                return i;
            }
            // The piece is mixed at once; the timing loop takes from it what its next run waits for, and no more. When
            // a run changes the mixer, the samples behind it are mixed again.
            let start = self.nco.clone();
            let (freq, pending) = (self.nco.freq(), self.pending);
            mixed[..n].copy_from_slice(&x[i..i + n]);
            self.nco.mix_in_place(&mut mixed[..n]);
            let offset = self.cfg.order == PskOrder::Oqpsk;
            let mut j = 0;
            loop {
                self.syms.clear();
                self.at = 0;
                if offset {
                    self.timing.set_carrier(self.carrier.phase(), self.carrier.freq());
                }
                let (m, made) = self.timing.process_run(&mut mixed[j..n], &mut self.syms);
                j += m;
                let mut stop = false;
                while !stop && self.at < self.syms.len() {
                    let k = self.carrier_run(&mut run);
                    stop = on_symbols(self, &run[..k]).is_break();
                }
                let changed = self.nco.freq() != freq || self.pending != pending;
                if stop || changed {
                    if j < n {
                        self.rewind(&start, j);
                    }
                    if self.pending == pending {
                        if let Some((k, _)) = self.pending.as_mut() {
                            *k -= j;
                        }
                    }
                    i += j;
                    if stop {
                        return i;
                    }
                    continue 'pieces;
                }
                if j == n && !made {
                    break;
                }
            }
            if let Some((k, _)) = self.pending.as_mut() {
                *k -= n;
            }
            i += n;
        }
    }

    /// Puts the mixer where it is `taken` samples after `start`, with the frequency it has now: the samples of a mixed
    /// piece after those are given back.
    fn rewind(&mut self, start: &Nco, taken: usize) {
        let freq = self.nco.freq();
        let mut skip = [Complex32::new(0.0, 0.0); PIECE];
        self.nco = start.clone();
        self.nco.fill(&mut skip[..taken]);
        if self.nco.freq() != freq {
            self.nco.set_freq(freq);
        }
    }

    /// The carrier loop, the hand-over and the counts for the next run of the symbols that wait. The symbols go to `run`;
    /// returns their count.
    #[inline]
    fn carrier_run(&mut self, run: &mut [Complex32; RUN]) -> usize {
        let at = self.at;
        let free = self.symbols < self.cfg.hold_symbols;
        // The frequency loop acts on single symbols, so a loop that pulls in with one takes single steps until it is locked.
        let single = free || self.cfg.fll_bn_t.is_some() && !self.carrier.locked();
        let m = if single { 1 } else { self.run }.min(self.syms.len() - at);
        let z = &mut run[..m];
        z.copy_from_slice(&self.syms[at..at + m]);
        self.at += m;
        self.carrier.run(z, free);
        if let Some(h) = self.cfg.handover {
            if self.carrier.freq().abs() > h {
                self.hand_over();
            }
        }
        // The EVM: the mean over the symbols so far, then a mean that forgets over about 100 symbols, a run at a time.
        let o = self.cfg.order;
        if self.symbols < 100 {
            for &v in z.iter() {
                let err = (v - o.nearest(v)).norm_sqr() as f64;
                self.evm_acc += (err - self.evm_acc) / (self.symbols + 1).min(100) as f64;
                self.symbols += 1;
            }
        } else {
            let err: f32 = z.iter().map(|&v| (v - o.nearest(v)).norm_sqr()).sum();
            self.evm_acc += 0.01 * (err as f64 - m as f64 * self.evm_acc);
            self.symbols += m as u64;
        }
        m
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::psk::{PskDemodConfig, PskOrder};
    use sdrkit_algo_sim::modulate::Linear;

    /// QPSK at 4 samples a symbol through a root raised cosine of 0.35, the carrier at 0.003 cycles a sample.
    fn signal(n: usize) -> Vec<Complex32> {
        let mut m = Linear::new(sdrkit_algo_dsp::pulse::Shape::Rrc { beta: 0.35 }, 16.0, 4.0).unwrap();
        let mut s = 11u32;
        let syms: Vec<Complex32> = (0..n)
            .map(|_| {
                s = s.wrapping_mul(1_103_515_245).wrapping_add(12_345);
                PskOrder::Qpsk.point((s >> 16) as usize)
            })
            .collect();
        let mut x = Vec::new();
        m.push(&syms, &mut x);
        for (i, v) in x.iter_mut().enumerate() {
            *v *= Complex32::from_polar(1.0, (std::f64::consts::TAU * 0.003 * i as f64) as f32);
        }
        x
    }

    fn demod() -> PskDemodulator {
        let mut c = PskDemodConfig::new(PskOrder::Qpsk, 4.0);
        c.handover = Some(0.0005);
        PskDemodulator::new(c).unwrap()
    }

    /// What the caller does at symbol `k`: a pending move, a mixer change and a reset, at fixed symbols.
    fn act(d: &mut PskDemodulator, k: usize) {
        match k {
            300 => d.retune_after(5, 0.0004),
            700 => d.set_mixer_offset(-0.0031),
            1500 => d.reset().unwrap(),
            _ => {}
        }
    }

    /// The symbols of `x` through [`PskDemodulator::process_with`] in pieces of `cut`, with the actions of [`act`] at the
    /// end of the run that holds their symbol, and a stop at the run that holds symbol 1000.
    fn by_slices(x: &[Complex32], cut: usize) -> Vec<Complex32> {
        let mut d = demod();
        let mut out = Vec::new();
        for piece in x.chunks(cut) {
            let mut at = 0;
            while at < piece.len() {
                at += d.process_with(&piece[at..], |d, z| {
                    let mut stop = false;
                    for &v in z {
                        out.push(v);
                        act(d, out.len());
                        stop |= out.len() == 1000;
                    }
                    if stop {
                        ControlFlow::Break(())
                    } else {
                        ControlFlow::Continue(())
                    }
                });
            }
        }
        out
    }

    #[test]
    fn the_cuts_of_the_input_do_not_change_the_symbols() {
        let x = signal(4000);
        let want = by_slices(&x, x.len());
        assert!(want.len() > 3900, "{}", want.len());
        for cut in [1, 37, 256, 1000] {
            let got = by_slices(&x, cut);
            assert_eq!(got, want, "cut {cut}");
        }
    }
}
