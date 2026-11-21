//! The synthesis filterbank of AAC: the inverse MDCT of long and short windows, the sine and Kaiser-Bessel derived window
//! shapes, the four window sequences and the overlap-add.
//!
//! Standard: ISO/IEC 14496-3 clause 4.6.11.

use crate::ics::{IcsInfo, Sequence};
use crate::mdct::Mdct;

/// The rising half of a window of `n` samples (`n / 2` values): sine, or Kaiser-Bessel derived with `alpha`
/// (clause 4.6.11.3.2).
fn half_window(n: usize, kbd: Option<f64>) -> Vec<f32> {
    let half = n / 2;
    match kbd {
        None => (0..half)
            .map(|i| (std::f64::consts::PI / n as f64 * (i as f64 + 0.5)).sin() as f32)
            .collect(),
        Some(alpha) => {
            let q = n as f64 / 4.0;
            let kaiser: Vec<f64> = (0..=half)
                .map(|j| {
                    let t = (j as f64 - q) / q;
                    bessel_i0(std::f64::consts::PI * alpha * (1.0 - t * t).max(0.0).sqrt())
                })
                .collect();
            let total: f64 = kaiser.iter().sum();
            let mut acc = 0.0;
            (0..half)
                .map(|i| {
                    acc += kaiser[i];
                    (acc / total).sqrt() as f32
                })
                .collect()
        }
    }
}

/// The modified Bessel function of the first kind of order 0, by its power series.
fn bessel_i0(x: f64) -> f64 {
    let mut sum = 1.0;
    let mut term = 1.0;
    for k in 1..50 {
        term *= (x / (2.0 * k as f64)) * (x / (2.0 * k as f64));
        sum += term;
        if term < sum * 1e-17 {
            break;
        }
    }
    sum
}

/// The window halves of one frame length, indexed by shape (0 sine, 1 KBD).
#[derive(Clone, Debug)]
struct Windows {
    long: [Vec<f32>; 2],
    short: [Vec<f32>; 2],
}

/// The filterbank of one frame length, shared by the channels of a stream.
#[derive(Clone, Debug)]
pub(crate) struct Filterbank {
    frame: usize,
    win: Windows,
    long: Mdct,
    short: Mdct,
    buf: Vec<f32>,
    tmp: Vec<f32>,
}

/// The overlap of one channel between frames.
#[derive(Clone, Debug)]
pub(crate) struct Overlap {
    prev: Vec<f32>,
    kbd: bool,
}

impl Overlap {
    /// Silence before the first frame of `frame` samples.
    pub(crate) fn new(frame: usize) -> Overlap {
        Overlap {
            prev: vec![0.0; frame],
            kbd: false,
        }
    }
}

impl Filterbank {
    /// The filterbank of frames of `frame` samples (1024 or 960).
    pub(crate) fn new(frame: usize) -> Filterbank {
        let s = frame / 8;
        Filterbank {
            frame,
            win: Windows {
                long: [
                    half_window(2 * frame, None),
                    half_window(2 * frame, Some(4.0)),
                ],
                short: [half_window(2 * s, None), half_window(2 * s, Some(6.0))],
            },
            long: Mdct::new(frame),
            short: Mdct::new(s),
            buf: vec![0.0; 2 * frame],
            tmp: vec![0.0; 2 * s],
        }
    }

    /// Transforms the spectrum `spec` of one channel with the windows of `info`, adds the overlap of the last frame, and
    /// writes `frame` samples to `out` at the scale of the spectrum.
    pub(crate) fn synthesize(
        &mut self,
        info: &IcsInfo,
        spec: &[f32],
        ov: &mut Overlap,
        out: &mut [f32],
    ) {
        let l = self.frame;
        let s = l / 8;
        let m = (l - s) / 2;
        let cur = usize::from(info.kbd);
        let prev = usize::from(ov.kbd);
        let buf = &mut self.buf;
        if info.seq == Sequence::EightShort {
            buf.fill(0.0);
            let scale = 1.0 / s as f32;
            for w in 0..8 {
                // Spectra of the right length always pass the length check.
                let _ = self.short.inverse(&spec[w * s..(w + 1) * s], &mut self.tmp);
                let rise = &self.win.short[if w == 0 { prev } else { cur }];
                let fall = &self.win.short[cur];
                let at = m + w * s;
                for i in 0..s {
                    buf[at + i] += self.tmp[i] * rise[i] * scale;
                    buf[at + s + i] += self.tmp[s + i] * fall[s - 1 - i] * scale;
                }
            }
        } else {
            let _ = self.long.inverse(&spec[..l], buf);
            let scale = 1.0 / l as f32;
            for v in buf.iter_mut() {
                *v *= scale;
            }
            let (left, right) = buf.split_at_mut(l);
            match info.seq {
                Sequence::LongStop => {
                    left[..m].fill(0.0);
                    let rise = &self.win.short[prev];
                    for i in 0..s {
                        left[m + i] *= rise[i];
                    }
                }
                _ => {
                    let rise = &self.win.long[prev];
                    for (v, w) in left.iter_mut().zip(rise) {
                        *v *= w;
                    }
                }
            }
            match info.seq {
                Sequence::LongStart => {
                    let fall = &self.win.short[cur];
                    for i in 0..s {
                        right[m + i] *= fall[s - 1 - i];
                    }
                    right[m + s..].fill(0.0);
                }
                _ => {
                    let fall = &self.win.long[cur];
                    for (i, v) in right.iter_mut().enumerate() {
                        *v *= fall[l - 1 - i];
                    }
                }
            }
        }
        for i in 0..l {
            out[i] = buf[i] + ov.prev[i];
        }
        ov.prev.copy_from_slice(&buf[l..]);
        ov.kbd = info.kbd;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn window_halves_meet_the_princen_bradley_condition() {
        for n in [256, 240, 2048, 1920] {
            for kbd in [None, Some(4.0)] {
                let w = half_window(n, kbd);
                for i in 0..n / 2 {
                    let p = w[i] * w[i] + w[n / 2 - 1 - i] * w[n / 2 - 1 - i];
                    assert!((p - 1.0).abs() < 1e-5, "{n} {kbd:?} {i}: {p}");
                }
            }
        }
    }
}
