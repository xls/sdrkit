//! CIS-36 (MFSK-36), identification only: the strongest tone of short frames, a histogram of the tones heard, their spacing
//! (the largest spacing on which the tones line up), their span and the tone rate.

use super::{Out, Sub};
use sdrkit_algo_dsp::fft::WindowedFft;
use sdrkit_algo_dsp::window::Window;
use sdrkit_core::Complex32;
use std::f64::consts::TAU;

const RATE: f64 = super::RATE;
/// Samples a frame (25 ms).
const FRAME: usize = 300;
/// Samples between frames (12.5 ms).
const HOP: usize = 150;
/// The transform size.
const NFFT: usize = 1024;
/// Frames between analyses.
const EVERY: u64 = 80;
/// Frames analysed.
const SPAN: usize = 960;

/// What an analysis found.
#[derive(Clone, Debug, Default)]
struct Analysis {
    tones: Vec<f64>,
    spacing: Option<f64>,
    rate: Option<f64>,
    double_gaps: i64,
    identified: bool,
    reason: &'static str,
}

/// The `cis36` receiver.
pub struct Cis36 {
    fft: WindowedFft,
    ring: Vec<Complex32>,
    n: u64,
    frames: u64,
    /// The tone of each kept frame (`None` when no tone stood out).
    kept: Vec<Option<f64>>,
    scratch: Vec<f32>,
    result: Analysis,
}

impl Default for Cis36 {
    fn default() -> Self {
        Self::new()
    }
}

impl Cis36 {
    /// The receiver.
    pub fn new() -> Self {
        Cis36 {
            fft: WindowedFft::new(NFFT, FRAME, Window::Hann)
                .expect("a window that fits the transform"),
            ring: Vec::with_capacity(2 * FRAME),
            n: 0,
            frames: 0,
            kept: Vec::new(),
            scratch: Vec::new(),
            result: Analysis {
                reason: "searching",
                ..Analysis::default()
            },
        }
    }

    /// The strongest tone of the newest frame in 150 to 4500 Hz, if it stands 25 times above the median bin.
    fn frame(&mut self) -> Option<f64> {
        let start = self.ring.len() - FRAME;
        let mut buf = self.ring[start..].to_vec();
        buf.resize(NFFT, Complex32::default());
        let p = self.fft.run_power(&buf[..FRAME]).to_vec();
        let bin = RATE / NFFT as f64;
        let lo = (150.0 / bin).ceil() as usize;
        let hi = (4500.0 / bin).floor() as usize;
        let med = sdrkit_algo_dsp::spectrum::percentile(&p[..NFFT / 2], 0.5, &mut self.scratch);
        let (k, &v) = p[lo..=hi]
            .iter()
            .enumerate()
            .max_by(|a, b| a.1.total_cmp(b.1))?;
        let k = k + lo;
        if v < 25.0 * med {
            return None;
        }
        let (l, r) = (p[k - 1], p[k + 1]);
        let den = l - 2.0 * v + r;
        let d = if den.abs() > 1e-30 {
            (0.5 * (l - r) / den).clamp(-0.5, 0.5)
        } else {
            0.0
        };
        Some((k as f64 + f64::from(d)) * bin)
    }

    fn analyse(&mut self) {
        let kept: Vec<f64> = self.kept.iter().flatten().copied().collect();
        let mut a = Analysis::default();
        // A histogram in 2 Hz bins, smoothed.
        let nb = 2300;
        let mut h = vec![0.0f64; nb];
        for &f in &kept {
            let b = (f / 2.0).round() as usize;
            if b < nb {
                h[b] += 1.0;
            }
        }
        let k = [0.25, 0.75, 1.0, 0.75, 0.25];
        let hs: Vec<f64> = (0..nb)
            .map(|i| {
                (0..5)
                    .map(|j| k[j] * h.get((i + j).wrapping_sub(2)).copied().unwrap_or(0.0))
                    .sum()
            })
            .collect();
        let need = (0.004 * self.kept.len() as f64).max(3.0);
        let mut tones: Vec<(f64, f64)> = Vec::new();
        for i in 7..nb - 7 {
            let v = hs[i];
            if v < need || (i - 7..=i + 7).any(|j| hs[j] > v || (hs[j] == v && j < i)) {
                continue;
            }
            let (mut s, mut w) = (0.0, 0.0);
            for (j, &h) in hs.iter().enumerate().take(i + 5).skip(i - 4) {
                s += h * 2.0 * j as f64;
                w += h;
            }
            let f = s / w;
            if tones.last().is_none_or(|t| f - t.0 >= 14.0) {
                tones.push((f, w));
            }
        }
        a.tones = tones.iter().map(|t| t.0).collect();
        if tones.len() < 4 {
            a.reason = "not an MFSK signal";
            self.result = a;
            return;
        }
        // The spacing: the largest c for which the tones line up.
        let tw: f64 = tones.iter().map(|t| t.1).sum();
        let mut c = 100.0;
        while c >= 15.0 {
            let (mut re, mut im) = (0.0, 0.0);
            for &(f, w) in &tones {
                re += w * (TAU * f / c).cos();
                im += w * (TAU * f / c).sin();
            }
            if (re * re + im * im).sqrt() / tw > 0.85 {
                a.spacing = Some(c);
                break;
            }
            c -= 0.05;
        }
        // The rate: tone changes between consecutive kept frames.
        if let Some(sp) = a.spacing {
            let th = (sp / 2.0).max(8.0);
            let (mut changes, mut pairs) = (0.0f64, 0.0f64);
            for w in self.kept.windows(2) {
                if let (Some(x), Some(y)) = (w[0], w[1]) {
                    pairs += 1.0;
                    if (x - y).abs() > th {
                        changes += 1.0;
                    }
                }
            }
            if pairs > 0.0 {
                let r =
                    changes / pairs * RATE / HOP as f64 / (1.0 - 1.0 / tones.len() as f64).max(0.5);
                a.rate = Some(
                    [10.0, 20.0, 40.0]
                        .into_iter()
                        .find(|&s| (r - s).abs() <= 0.3 * s)
                        .unwrap_or(r),
                );
            }
            a.double_gaps = a
                .tones
                .windows(2)
                .filter(|t| ((t[1] - t[0]) / sp - 2.0).abs() < 0.3)
                .count() as i64;
        }
        let span = a.tones.last().unwrap_or(&0.0) - a.tones.first().unwrap_or(&0.0);
        a.identified = a.spacing.is_some_and(|s| (37.0..=43.0).contains(&s))
            && a.tones.len() >= 20
            && (1000.0..=1500.0).contains(&span);
        a.reason = if a.identified {
            "CIS-36"
        } else {
            "an MFSK signal, not CIS-36"
        };
        self.result = a;
    }
}

impl Sub for Cis36 {
    fn push(&mut self, x: &[Complex32], _out: &mut Out) {
        for &v in x {
            self.ring.push(v);
            self.n += 1;
            if self.ring.len() >= FRAME && self.n.is_multiple_of(HOP as u64) {
                let t = self.frame();
                self.kept.push(t);
                if self.kept.len() > SPAN {
                    self.kept.remove(0);
                }
                self.frames += 1;
                if self.frames.is_multiple_of(EVERY) {
                    self.analyse();
                }
            }
            if self.ring.len() >= 4 * FRAME {
                self.ring.drain(..2 * FRAME);
            }
        }
    }
    fn state(&self) -> u8 {
        if self.result.identified {
            2
        } else if self.result.tones.len() >= 4 {
            1
        } else {
            0
        }
    }
    fn summary(&self) -> String {
        let a = &self.result;
        match (a.spacing, a.rate) {
            (Some(s), Some(r)) if a.tones.len() >= 4 => format!(
                "{}: {} tones {s:.1} Hz apart, {r:.1} Bd",
                a.reason,
                a.tones.len()
            ),
            _ => a.reason.to_string(),
        }
    }
    fn measure<'a>(&'a self, m: &mut super::Measures<'a>) {
        let a = &self.result;
        m.tones = Some(a.tones.len() as i64);
        m.spacing_hz = a.spacing.map(|v| (v * 100.0).round() / 100.0);
        let lo = a.tones.first().copied();
        let hi = a.tones.last().copied();
        m.low_hz = lo.map(|v| v.round());
        m.high_hz = hi.map(|v| v.round());
        m.span_hz = lo.zip(hi).map(|(l, h)| (h - l).round());
        m.symbol_rate = a.rate.map(|v| (v * 10.0).round() / 10.0);
        m.double_gaps = Some(a.double_gaps);
    }
    fn identified(&self) -> Option<String> {
        self.result.identified.then(|| "CIS-36".to_string())
    }
}
