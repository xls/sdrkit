//! The carrier layout of each DVB-T symbol: which carriers hold data, scattered pilots, continual pilots and TPS, and the pilot
//! values. The tables and sequences come from [`crate::coding`].
//!
//! Standard: ETSI EN 300 744 clauses 4.5 and 4.6.

use crate::coding::{continual, pilot_sequence, symbol_permutation, tps_carriers};
use crate::params::Mode;

/// What a carrier of a symbol carries.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CarrierKind {
    /// A data cell.
    Data,
    /// A scattered pilot.
    Scattered,
    /// A continual pilot.
    Continual,
    /// A TPS cell.
    Tps,
}

/// The carrier layout of a mode, for each of the four scattered-pilot phases.
#[derive(Clone, Debug)]
pub struct Layout {
    /// The mode.
    pub mode: Mode,
    /// Carriers K.
    pub k: usize,
    /// w_k for every carrier.
    pub wk: Vec<u8>,
    /// Pilot values ±4/3 per carrier (sign from w_k).
    pub pilot: Vec<f32>,
    /// Continual pilot carriers.
    pub continual: Vec<usize>,
    /// TPS carriers.
    pub tps: Vec<usize>,
    /// Data carriers of a symbol with l mod 4 = p.
    pub data: [Vec<u16>; 4],
    /// Scattered pilots of a symbol with l mod 4 = p.
    pub scattered: [Vec<u16>; 4],
    /// The kind of every carrier for each phase.
    pub kind: [Vec<CarrierKind>; 4],
    /// The symbol interleaver permutation.
    pub perm: Vec<u16>,
}

impl Layout {
    /// The layout of a mode.
    pub fn new(mode: Mode) -> Layout {
        let k = mode.carriers();
        let wk = pilot_sequence(k);
        let pilot: Vec<f32> = wk
            .iter()
            .map(|&w| 4.0 / 3.0 * (1.0 - 2.0 * w as f32))
            .collect();
        let continual = continual(mode);
        let tps = tps_carriers(mode);
        let mut is_cont = vec![false; k];
        for &c in &continual {
            is_cont[c] = true;
        }
        let mut is_tps = vec![false; k];
        for &c in &tps {
            is_tps[c] = true;
        }
        let mut data: [Vec<u16>; 4] = Default::default();
        let mut scattered: [Vec<u16>; 4] = Default::default();
        let mut kind: [Vec<CarrierKind>; 4] = Default::default();
        for p in 0..4 {
            let mut kk = vec![CarrierKind::Data; k];
            for (c, slot) in kk.iter_mut().enumerate() {
                *slot = if c % 12 == 3 * p {
                    CarrierKind::Scattered
                } else if is_cont[c] {
                    CarrierKind::Continual
                } else if is_tps[c] {
                    CarrierKind::Tps
                } else {
                    CarrierKind::Data
                };
                match *slot {
                    CarrierKind::Data => data[p].push(c as u16),
                    CarrierKind::Scattered => scattered[p].push(c as u16),
                    _ => {}
                }
            }
            kind[p] = kk;
        }
        Layout {
            mode,
            k,
            wk,
            pilot,
            continual,
            tps,
            data,
            scattered,
            kind,
            perm: symbol_permutation(mode),
        }
    }
    /// The FFT bin of carrier `c` (unshifted, before any carrier offset): c - (K - 1)/2 mod N.
    pub fn bin(&self, c: usize) -> usize {
        let n = self.mode.fft() as isize;
        ((c as isize - (self.k as isize - 1) / 2).rem_euclid(n)) as usize
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn counts_match_the_standard() {
        for (m, cont, tps, nd) in [
            (Mode::K1, 25, 7, 756),
            (Mode::K2, 45, 17, 1512),
            (Mode::K4, 89, 34, 3024),
            (Mode::K8, 177, 68, 6048),
        ] {
            let l = Layout::new(m);
            assert_eq!(l.continual.len(), cont);
            assert_eq!(l.tps.len(), tps);
            for p in 0..4 {
                assert_eq!(l.data[p].len(), nd, "{m:?} phase {p}");
            }
            let mut h = l.perm.clone();
            assert_eq!(h.len(), nd);
            h.sort_unstable();
            assert!(
                h.iter().enumerate().all(|(i, &v)| v as usize == i),
                "H is a permutation"
            );
        }
        // The register rule gives the DVB-T permutations of the shared interleaver for 2K and 8K.
        for m in [Mode::K2, Mode::K8] {
            assert_eq!(crate::coding::symbol_interleaver(m), symbol_permutation(m));
        }
        let w = pilot_sequence(32);
        assert_eq!(&w[..11], &[1; 11]);
        assert_eq!(&w[11..20], &[0; 9]);
        assert_eq!(w[20], 1);
    }
}
