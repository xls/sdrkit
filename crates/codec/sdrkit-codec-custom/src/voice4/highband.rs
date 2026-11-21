//! The voice codec 4 high band at 16 kHz: band levels from the indices, noise synthesis by MDCT, and the encoder analysis.

use super::frame::Params;
use super::layout::Layout;
use super::math::{db, lin, Lcg, Mdct};

/// MDCT bin edges of the six bands (bins of 100 Hz).
pub(crate) const EDGES: [usize; 7] = [36, 41, 47, 54, 61, 69, 80];
/// Band widths in bins.
pub(crate) const WIDTH: [f64; 6] = [5.0, 6.0, 7.0, 7.0, 8.0, 11.0];
/// Time-envelope levels in dB.
pub(crate) const TIME_LEVELS: [f64; 4] = [0.0, -4.0, -9.0, -16.0];

/// The band position t(j) = (j - 2.5) / 2.5.
#[inline]
pub(crate) fn t(j: usize) -> f64 {
    (j as f64 - 2.5) / 2.5
}

/// The shape of a half from its indices, normalised to 0 dB mean power (section 3.6.2 step 1).
pub(crate) fn shape(mode: u32, idx: &[u32; 6]) -> [f64; 6] {
    let mut s = [0f64; 6];
    match mode {
        0 => {
            for (j, v) in s.iter_mut().enumerate() {
                *v = (-21.0 + 6.0 * idx[0] as f64) * t(j);
            }
        }
        1 => {
            let mut m = 0.0;
            for j in 0..6 {
                m += t(j) * t(j);
            }
            m /= 6.0;
            for (j, v) in s.iter_mut().enumerate() {
                *v = (-21.0 + 6.0 * idx[0] as f64) * t(j)
                    + (-9.0 + 6.0 * idx[1] as f64) * (t(j) * t(j) - m);
            }
        }
        2 => {
            for (j, v) in s.iter_mut().enumerate() {
                *v = -13.5 + 6.0 * idx[j] as f64;
            }
        }
        _ => {
            for (j, v) in s.iter_mut().enumerate() {
                *v = -22.0 + 4.0 * idx[j] as f64;
            }
        }
    }
    let mut num = 0.0;
    let mut den = 0.0;
    for j in 0..6 {
        num += WIDTH[j] * lin(s[j]);
        den += WIDTH[j];
    }
    let off = db(num / den);
    for v in s.iter_mut() {
        *v -= off;
    }
    s
}

/// The high-band decoder state.
pub(crate) struct HbDecoder {
    pub last: [f64; 6],
    pub have: bool,
    pub rng: Lcg,
    ola: [f32; 80],
}

/// Band levels for every block of a frame: up to 8 blocks of 6 bands.
pub(crate) type Levels = [[f64; 6]; 8];

impl HbDecoder {
    pub(crate) fn new() -> Self {
        Self {
            last: [-100.0; 6],
            have: false,
            rng: Lcg(0x4842_4E5A),
            ola: [0.0; 80],
        }
    }

    /// The band levels of one half (section 3.6.2). `good` holds the indices of a good frame, `None` for a lost frame.
    pub(crate) fn half_levels(
        &mut self,
        lay: &Layout,
        p: Option<(&Params, usize)>,
        out: &mut [[f64; 6]],
    ) {
        let mut cur = [0f64; 6];
        let mut tl = [0f64; 4];
        let mut use_time = false;
        match p {
            Some((p, h)) => {
                let s = shape(lay.hb_mode, &p.hb_shape[h]);
                let li = p.hb_level[h];
                let g = if li == 0 {
                    -100.0
                } else {
                    li as f64 * 96.0 / (1u32 << lay.hb_level_bits) as f64
                };
                for j in 0..6 {
                    cur[j] = g + s[j];
                }
                if lay.hb_time_bits > 0 && !p.time_bad {
                    use_time = true;
                    let mut m = 0.0;
                    for b in 0..4 {
                        tl[b] = TIME_LEVELS[(p.hb_time[h][b] & 3) as usize];
                        m += lin(tl[b]);
                    }
                    let off = db(m / 4.0);
                    for v in tl.iter_mut() {
                        *v -= off;
                    }
                }
            }
            None => {
                for j in 0..6 {
                    cur[j] = self.last[j] - 3.0;
                }
            }
        }
        if !self.have {
            self.last = cur;
            self.have = true;
        }
        for (b, lv) in out.iter_mut().enumerate().take(4) {
            for j in 0..6 {
                lv[j] = if use_time {
                    cur[j] + tl[b]
                } else {
                    self.last[j] + (b + 1) as f64 * 0.25 * (cur[j] - self.last[j])
                };
                if cur[j] < -60.0 && self.last[j] < -60.0 {
                    lv[j] = -100.0;
                }
            }
        }
        self.last = cur;
    }

    /// Plain noise synthesis of `nb` blocks of levels into `out` (80 samples per block, section 3.6.3).
    pub(crate) fn synth(&mut self, mdct: &Mdct, levels: &[[f64; 6]], out: &mut [f32]) {
        for (b, lv) in levels.iter().enumerate() {
            let mut x = [0f32; 80];
            for j in 0..6 {
                if lv[j] < -60.0 {
                    continue;
                }
                let a = (lin(lv[j]) * 3.0).sqrt() as f32;
                for xk in x.iter_mut().take(EDGES[j + 1]).skip(EDGES[j]) {
                    *xk = a * self.rng.uni();
                }
            }
            mdct.inverse_ola(&x, &mut self.ola, &mut out[b * 80..(b + 1) * 80]);
        }
    }
}
