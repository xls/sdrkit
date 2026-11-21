//! The square QAM of HD Radio and CDR.

use sdrkit_algo_dsp::constellation::{PointSet, QamMap};
use sdrkit_algo_modem::constellation::{Constellation, Demap};
use sdrkit_core::Complex32;
use std::sync::OnceLock;

sdrkit_core::choice_enum! {
    /// A square QAM with Gray coding on each axis and the bit map of [`QamMap::IEEE_802_11`] (the in-phase bits first, most
    /// significant first, all zeros on the negative side), as HD Radio and CDR map their cells.
    #[derive(serde::Serialize)]
    pub enum Qam {
        /// QPSK.
        Q4 = ("4", "QPSK"),
        /// 16-QAM.
        Q16 = ("16", "16-QAM"),
        /// 64-QAM.
        Q64 = ("64", "64-QAM"),
    }
}

impl Qam {
    /// Bits per cell.
    pub fn bits(self) -> usize {
        match self {
            Qam::Q4 => 2,
            Qam::Q16 => 4,
            Qam::Q64 => 6,
        }
    }
    /// The number of points.
    pub fn points(self) -> u32 {
        1 << self.bits()
    }
    /// From the number of points.
    pub fn from_points(n: u32) -> Option<Qam> {
        Some(match n {
            4 => Qam::Q4,
            16 => Qam::Q16,
            64 => Qam::Q64,
            _ => return None,
        })
    }
    /// A name ("QPSK", "16-QAM").
    pub fn name(self) -> &'static str {
        match self {
            Qam::Q4 => "QPSK",
            Qam::Q16 => "16-QAM",
            Qam::Q64 => "64-QAM",
        }
    }
    /// The constellation, made once.
    pub fn constellation(self) -> &'static Constellation {
        static C: [OnceLock<Constellation>; 3] =
            [OnceLock::new(), OnceLock::new(), OnceLock::new()];
        let i = self as usize;
        C[i].get_or_init(|| {
            PointSet::qam(self.points() as usize, QamMap::IEEE_802_11)
                .ok()
                .and_then(|p| Constellation::new(p).ok())
                .unwrap_or_else(|| unreachable!("4, 16 and 64-QAM are square"))
        })
    }
    /// The cell of `bits()` bits (the in-phase bits, then the quadrature bits, each most significant first).
    pub fn map(self, bits: &[u8]) -> Complex32 {
        let label = bits
            .iter()
            .take(self.bits())
            .fold(0usize, |a, &b| (a << 1) | (b & 1) as usize);
        self.constellation()
            .points()
            .get(label)
            .copied()
            .unwrap_or_default()
    }
    /// Appends the LLRs (positive means 0) of the bits of the equalised cell `z` with weight `w` (`|H|^2 / sigma^2`), in the
    /// order of [`Qam::map`]. A cell that is not finite produces erasures.
    pub fn demap(self, z: Complex32, w: f32, out: &mut Vec<f32>) {
        let n = self.bits();
        let at = out.len();
        out.resize(at + n, 0.0);
        if self
            .constellation()
            .demap_weighted(z, w.max(0.0), Demap::MaxLog, &mut out[at..])
            .is_err()
        {
            out[at..].fill(0.0);
        }
    }
    /// The nearest point (for error measurements).
    pub fn nearest(self, z: Complex32) -> Complex32 {
        self.constellation().nearest(z)
    }
}
