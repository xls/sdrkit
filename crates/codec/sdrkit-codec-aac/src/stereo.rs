//! The spectral tools between the channels of an element: M/S stereo, intensity stereo and perceptual noise substitution.
//!
//! Standard: ISO/IEC 14496-3 clauses 4.6.8.1 (M/S), 4.6.8.2 (intensity) and 4.6.13 (PNS).

use crate::ics::{Bands, Channel, IcsInfo, INTENSITY_HCB, INTENSITY_HCB2, MAX_SFB, NOISE_HCB};

/// The M/S mask of a channel pair: `ms_mask_present` and `ms_used` by group and band.
#[derive(Clone, Debug)]
pub(crate) struct MsMask {
    pub present: u8,
    pub used: [[bool; MAX_SFB]; 8],
}

impl Default for MsMask {
    fn default() -> Self {
        MsMask {
            present: 0,
            used: [[false; MAX_SFB]; 8],
        }
    }
}

/// Calls `f` with the group, the band and the sample range of each band of each window.
fn for_each_band(
    info: &IcsInfo,
    bands: &Bands,
    mut f: impl FnMut(usize, usize, std::ops::Range<usize>),
) {
    let short = info.short();
    let offsets = bands.offsets(short);
    let win_len = if short {
        bands.short_len()
    } else {
        bands.frame
    };
    let mut w0 = 0;
    for g in 0..info.n_groups {
        let glen = usize::from(info.group_len[g]);
        for b in 0..info.max_sfb {
            for w in w0..w0 + glen {
                let base = w * win_len;
                f(
                    g,
                    b,
                    base + usize::from(offsets[b])..base + usize::from(offsets[b + 1]),
                );
            }
        }
        w0 += glen;
    }
}

/// A linear congruential generator for the noise of PNS (the standard leaves the generator open).
#[derive(Clone, Copy, Debug)]
pub(crate) struct Noise(u32);

impl Default for Noise {
    fn default() -> Self {
        Noise(0x1F2E_3D4C)
    }
}

impl Noise {
    fn next(&mut self) -> f32 {
        self.0 = self.0.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        (self.0 as i32) as f32
    }

    /// Fills the noise bands of `ch` with noise of the band energy `2^(nrg / 2)` (clause 4.6.13.3). If `copy` holds a
    /// channel and its band is a noise band with M/S used, the band takes that channel's noise shape.
    pub(crate) fn fill(
        &mut self,
        ch: &mut Channel,
        bands: &Bands,
        copy: Option<(&Channel, &MsMask)>,
    ) {
        let info = ch.info;
        let cb = ch.cb;
        let sf = ch.sf;
        let spec = &mut ch.spec;
        for_each_band(&info, bands, |g, b, r| {
            if cb[g][b] != NOISE_HCB {
                return;
            }
            let shared = copy.and_then(|(left, ms)| {
                let used = ms.present == 2 || (ms.present == 1 && ms.used[g][b]);
                (used && left.cb[g][b] == NOISE_HCB).then_some(left)
            });
            match shared {
                Some(left) => spec[r.clone()].copy_from_slice(&left.spec[r.clone()]),
                None => {
                    for v in &mut spec[r.clone()] {
                        *v = self.next();
                    }
                }
            }
            let energy: f32 = spec[r.clone()].iter().map(|v| v * v).sum();
            if energy > 0.0 {
                let gain = 2f32.powf(0.25 * sf[g][b] as f32) / energy.sqrt();
                for v in &mut spec[r] {
                    *v *= gain;
                }
            }
        });
    }
}

/// Applies M/S stereo and then intensity stereo to a channel pair (clauses 4.6.8.1 and 4.6.8.2).
pub(crate) fn apply(l: &mut Channel, r: &mut Channel, ms: &MsMask, bands: &Bands) {
    let rcb = r.cb;
    let rsf = r.sf;
    let info = r.info;
    let (ls, rs) = (&mut l.spec, &mut r.spec);
    let lcb = l.cb;
    for_each_band(&info, bands, |g, b, k| {
        let c = rcb[g][b];
        let used = ms.present == 2 || (ms.present == 1 && ms.used[g][b]);
        if c == INTENSITY_HCB || c == INTENSITY_HCB2 {
            let mut sign = if c == INTENSITY_HCB { 1.0 } else { -1.0 };
            if ms.present == 1 && ms.used[g][b] {
                sign = -sign;
            }
            let scale = sign * 0.5f32.powf(0.25 * rsf[g][b] as f32);
            for i in k {
                rs[i] = ls[i] * scale;
            }
        } else if used && c != NOISE_HCB && lcb[g][b] != NOISE_HCB {
            for i in k {
                let (m, s) = (ls[i], rs[i]);
                ls[i] = m + s;
                rs[i] = m - s;
            }
        }
    });
}
