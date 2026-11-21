//! The audio signal finder of the keyboard MFSK stacks and Hell: the centre follows the signal while nothing is decoding.

use crate::util::median;
use sdrkit_algo_dsp::fft::WindowedFft;
use sdrkit_algo_dsp::window::Window;
use sdrkit_core::Complex32;

const N: usize = 4096;
const RATE: f32 = 8000.0;

/// Finds the position of a signal of a known bandwidth in the 8 kHz sideband.
#[derive(Debug)]
pub struct CentreFinder {
    spec: WindowedFft,
    buf: Vec<Complex32>,
    avg: Vec<f32>,
    count: u32,
    /// The signal's bandwidth, Hz.
    pub bandwidth: f32,
    cand: Option<f32>,
    stable: u32,
    sums: Vec<f32>,
    scratch: Vec<f32>,
}

impl CentreFinder {
    /// A finder for a signal `bandwidth` Hz wide.
    pub fn new(bandwidth: f32) -> Self {
        CentreFinder {
            spec: WindowedFft::new(N, N, Window::Hann).expect("a window that fits the transform"),
            buf: Vec::with_capacity(N),
            avg: vec![0.0; N / 2],
            count: 0,
            bandwidth,
            cand: None,
            stable: 0,
            sums: Vec::with_capacity(N / 2),
            scratch: Vec::with_capacity(N / 2),
        }
    }

    /// Samples until the next spectrum: feed at most this much at a time so decisions fall at the same sample however the
    /// input is cut.
    pub fn until_frame(&self) -> usize {
        N - self.buf.len()
    }

    /// Forgets the candidate (not the spectrum).
    pub fn reset(&mut self) {
        self.cand = None;
        self.stable = 0;
    }

    /// Feeds audio. `free`: the decoder is not locked to plausible text (the finder can then move the centre). Returns a new centre.
    pub fn feed(&mut self, x: &[Complex32], centre: f32, free: bool) -> Option<f32> {
        let mut moved = None;
        for &s in x {
            self.buf.push(s);
            if self.buf.len() < N {
                continue;
            }
            // Each spectrum as a share of its power, so every frame weighs the same and a loud one (the AGC overshoot at the
            // start of a burst after silence, a static crash) does not decide the centre. A frame of silence adds zeros.
            let p = self.spec.run_power(&self.buf);
            let total: f32 = p[..N / 2].iter().sum();
            let k = if total > 0.0 { 1.0 / total } else { 0.0 };
            for (a, &v) in self.avg.iter_mut().zip(&p[..N / 2]) {
                *a += 0.15 * (v * k - *a);
            }
            self.buf.clear();
            self.count += 1;
            if self.count.is_multiple_of(2) && free {
                if let Some(c) = self.evaluate(centre) {
                    moved = Some(c);
                }
            }
        }
        moved
    }

    fn evaluate(&mut self, centre: f32) -> Option<f32> {
        let bin = RATE / N as f32;
        let w = (self.bandwidth.max(60.0) / bin).round() as usize;
        let lo = (250.0 / bin) as usize;
        let hi = (3450.0 / bin) as usize;
        if hi <= lo + w {
            return None;
        }
        self.sums.clear();
        let mut s: f32 = self.avg[lo..lo + w].iter().sum();
        self.sums.push(s);
        for i in lo + 1..=hi - w {
            s += self.avg[i + w - 1] - self.avg[i - 1];
            self.sums.push(s);
        }
        let (bi, &best) = self
            .sums
            .iter()
            .enumerate()
            .max_by(|a, b| a.1.total_cmp(b.1))?;
        let med = median(&self.sums, &mut self.scratch);
        if best <= 1.15 * med || best <= 0.0 {
            self.stable = 0;
            self.cand = None;
            return None;
        }
        // Of the windows about as strong as the best (a single tone fits many), the one nearest the current centre.
        let mut bi = bi;
        let mut near = f32::MAX;
        for (i, &v) in self.sums.iter().enumerate() {
            let m = (lo + i) as f32 * bin + w as f32 * bin / 2.0 - bin / 2.0;
            if v >= 0.98 * best && (m - centre).abs() < near {
                near = (m - centre).abs();
                bi = i;
            }
        }
        let mid = (lo + bi) as f32 * bin + w as f32 * bin / 2.0 - bin / 2.0;
        let tol = (self.bandwidth / 16.0).max(8.0);
        match self.cand {
            Some(c) if (c - mid).abs() <= tol => self.stable += 1,
            _ => {
                self.cand = Some(mid);
                self.stable = 1;
            }
        }
        if self.stable >= 3 && (mid - centre).abs() > (self.bandwidth / 12.0).max(10.0) {
            self.stable = 0;
            self.cand = None;
            return Some(mid.clamp(300.0, 3500.0));
        }
        None
    }
}
