//! The station finder, for recordings and the generator only: it finds an FM station that is off the centre of the input. It
//! scores each frequency by the median power of 16 averaged spectra over +-75 kHz around it, and moves only to a clear winner.

use sdrkit_algo_dsp::fft::Fft;
use sdrkit_algo_dsp::window::Window;
use sdrkit_core::Complex32;

const N: usize = 4096;
const FRAMES: usize = 16;

/// The station finder.
#[derive(Clone, Debug)]
pub struct StationFinder {
    rate: f64,
    fft: Fft,
    win: Vec<f32>,
    frame: Vec<Complex32>,
    fill: usize,
    frames: usize,
    acc: Vec<f64>,
    active: bool,
}

impl StationFinder {
    /// A finder for input at `rate`. It starts idle.
    pub fn new(rate: f64) -> Self {
        StationFinder {
            rate,
            fft: Fft::forward(N),
            win: Window::Hann.coefficients(N),
            frame: vec![Complex32::default(); N],
            fill: 0,
            frames: 0,
            acc: vec![0.0; N],
            active: false,
        }
    }

    /// Starts collecting spectra.
    pub fn start(&mut self) {
        self.active = true;
        self.fill = 0;
        self.frames = 0;
        self.acc.iter_mut().for_each(|v| *v = 0.0);
    }

    /// Stops without a decision.
    pub fn stop(&mut self) {
        self.active = false;
    }

    /// Whether it is collecting.
    pub fn active(&self) -> bool {
        self.active
    }

    /// Input samples still needed for a decision (0 when idle).
    pub fn needed(&self) -> usize {
        if !self.active {
            return 0;
        }
        (FRAMES - self.frames) * N - self.fill
    }

    /// Takes input samples (at most [`needed`](Self::needed)). Returns true when the spectra are complete; then call
    /// [`decide`](Self::decide).
    pub fn push(&mut self, x: &[Complex32]) -> bool {
        if !self.active {
            return false;
        }
        for &v in x {
            self.frame[self.fill] = v * self.win[self.fill];
            self.fill += 1;
            if self.fill == N {
                self.fill = 0;
                self.fft.process(&mut self.frame);
                for (i, a) in self.acc.iter_mut().enumerate() {
                    // Frequency order: index N/2 is 0 Hz.
                    *a += self.frame[(i + N / 2) % N].norm_sqr() as f64;
                }
                self.frames += 1;
                if self.frames == FRAMES {
                    self.active = false;
                    return true;
                }
            }
        }
        false
    }

    fn hz(&self, i: usize) -> f64 {
        (i as f64 - (N / 2) as f64) * self.rate / N as f64
    }

    fn bin(&self, hz: f64) -> isize {
        (hz / self.rate * N as f64).round() as isize + (N / 2) as isize
    }

    fn median(v: &mut [f64]) -> f64 {
        if v.is_empty() {
            return 0.0;
        }
        let m = v.len() / 2;
        *v.select_nth_unstable_by(m, f64::total_cmp).1
    }

    fn score(&self, centre_hz: f64, scratch: &mut Vec<f64>) -> f64 {
        let half = (75e3 / self.rate * N as f64).round() as isize;
        let c = self.bin(centre_hz);
        scratch.clear();
        for b in (c - half).max(0)..=(c + half).min(N as isize - 1) {
            scratch.push(self.acc[b as usize]);
        }
        Self::median(scratch)
    }

    /// The new tuning point (Hz from the input centre), or `None` to stay at `current_hz`.
    pub fn decide(&self, current_hz: f64) -> Option<f64> {
        let mut all = self.acc.clone();
        let floor = Self::median(&mut all);
        let reach = self.rate / 2.0 - 110e3;
        if reach <= 0.0 || floor <= 0.0 {
            return None;
        }
        let mut scratch = Vec::new();
        let step = ((2e3 / self.rate * N as f64).round() as usize).max(1);
        let mut best = (f64::MIN, 0.0);
        for i in (0..N).step_by(step) {
            let f = self.hz(i);
            if f.abs() > reach {
                continue;
            }
            let s = self.score(f, &mut scratch);
            if s > best.0 {
                best = (s, f);
            }
        }
        let centre = self.score(current_hz, &mut scratch);
        if !(best.0 > 10.0 * floor && centre < 10.0 * floor) {
            return None;
        }
        // The power centroid over +-100 kHz around the best window, above the floor.
        let half = (100e3 / self.rate * N as f64).round() as isize;
        let c = self.bin(best.1);
        let (mut num, mut den) = (0.0, 0.0);
        for b in (c - half).max(0)..=(c + half).min(N as isize - 1) {
            let p = (self.acc[b as usize] - floor).max(0.0);
            num += p * self.hz(b as usize);
            den += p;
        }
        let point = if den > 0.0 { num / den } else { best.1 };
        ((point - current_hz).abs() > 20e3).then_some(point)
    }
}
