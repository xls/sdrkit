//! The riometer power estimate with interference rejected: chunks, seconds and minutes of the mean noise power per sample.
//!
//! 1. Chunks of max(1024, round(rate × 20 ms)) samples; in a chunk, the power spectra of its whole 1024-sample segments (Hann)
//!    are summed. The chunk value is the median of the 1024 bins turned into a mean by the median-to-mean ratio of a sum of that
//!    many spectra (`floor::BinMedian`), divided by the segments and the window energy: white noise of power 1 per sample
//!    reads 1. Narrow carriers move the median little.
//! 2. Seconds follow the sample count: when a chunk starts in a new second, the values of the second that ended are sorted;
//!    the second's value is the lower quartile (index n / 4); the second is interfered when its median exceeds 1.35 × that.
//! 3. Minutes: every 60 second values, sorted; the minute's power is the element at index 20 (the lower third). The minute is
//!    interfered when more than 20 of its seconds were.

use sdrkit_algo_dsp::fft::WindowedFft;
use sdrkit_algo_dsp::floor::BinMedian;
use sdrkit_algo_dsp::window::Window;
use sdrkit_core::Complex32;

/// Points of a segment.
pub const SEGMENT: usize = 1024;
/// The length of a chunk, s.
pub const CHUNK_S: f64 = 0.02;
/// A second is interfered when its median exceeds this times its lower quartile.
pub const SECOND_RATIO: f64 = 1.35;
/// The index of a minute's power among its 60 sorted second values.
pub const MINUTE_INDEX: usize = 20;
/// A minute is interfered when more than this many of its seconds were.
pub const MINUTE_INTERFERED: usize = 20;

/// One minute of power.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Minute {
    /// The minute's index from the first sample (minute `i` holds seconds `60 i` to `60 i + 59`).
    pub index: u64,
    /// The power, mean per sample (full scale 1).
    pub power: f64,
    /// Interfered.
    pub interfered: bool,
    /// Interfered seconds.
    pub interfered_seconds: usize,
}

/// The meter (module docs).
pub struct PowerMeter {
    rate: f64,
    chunk: usize,
    segments: usize,
    fft: WindowedFft,
    energy: f64,
    median: BinMedian,
    frame: Vec<Complex32>,
    acc: Vec<f32>,
    in_chunk: usize,
    segs: usize,
    chunk_start: u64,
    samples: u64,
    second: Option<u64>,
    values: Vec<f64>,
    seconds: Vec<(f64, bool)>,
    minute_index: u64,
    minutes: Vec<Minute>,
    last_chunk: Option<f64>,
    last_second: Option<f64>,
}

impl PowerMeter {
    /// A meter for input at `rate` (at least 1024 samples a second).
    pub fn new(rate: f64) -> Self {
        let chunk = ((rate * CHUNK_S).round() as usize).max(SEGMENT);
        let segments = chunk / SEGMENT;
        let fft = WindowedFft::new(SEGMENT, SEGMENT, Window::Hann).expect("1024 is a valid size");
        let energy = fft.window().iter().map(|w| f64::from(w * w)).sum::<f64>();
        PowerMeter {
            rate,
            chunk,
            segments,
            fft,
            energy,
            median: BinMedian::new(segments as u32).expect("at least one frame"),
            frame: Vec::with_capacity(SEGMENT),
            acc: vec![0.0; SEGMENT],
            in_chunk: 0,
            segs: 0,
            chunk_start: 0,
            samples: 0,
            second: None,
            values: Vec::with_capacity(64),
            seconds: Vec::with_capacity(60),
            minute_index: 0,
            minutes: Vec::new(),
            last_chunk: None,
            last_second: None,
        }
    }

    /// Samples of a chunk.
    pub fn chunk(&self) -> usize {
        self.chunk
    }

    /// Segments of a chunk.
    pub fn segments(&self) -> usize {
        self.segments
    }

    /// The value of the last chunk.
    pub fn last_chunk(&self) -> Option<f64> {
        self.last_chunk
    }

    /// The value of the last second.
    pub fn last_second(&self) -> Option<f64> {
        self.last_second
    }

    /// Feeds samples. Samples that are not finite count as 0.
    pub fn process(&mut self, x: &[Complex32]) {
        for &z in x {
            let z = if z.re.is_finite() && z.im.is_finite() {
                z
            } else {
                Complex32::default()
            };
            self.samples += 1;
            self.in_chunk += 1;
            if self.segs < self.segments {
                self.frame.push(z);
                if self.frame.len() == SEGMENT {
                    let p = self.fft.run_power(&self.frame);
                    for (a, v) in self.acc.iter_mut().zip(p) {
                        *a += v;
                    }
                    self.frame.clear();
                    self.segs += 1;
                }
            }
            if self.in_chunk == self.chunk {
                self.end_chunk();
            }
        }
    }

    fn end_chunk(&mut self) {
        let mean = self.median.estimate(self.acc.iter().copied()).unwrap_or(0.0);
        let v = f64::from(mean) / (self.segments as f64 * self.energy);
        self.acc.iter_mut().for_each(|a| *a = 0.0);
        self.in_chunk = 0;
        self.segs = 0;
        let second = (self.chunk_start as f64 / self.rate).floor() as u64;
        self.chunk_start = self.samples;
        self.last_chunk = Some(v);
        if self.second.is_some_and(|s| s != second) {
            self.end_second();
        }
        self.second = Some(second);
        self.values.push(v);
    }

    fn end_second(&mut self) {
        if self.values.is_empty() {
            return;
        }
        self.values.sort_by(f64::total_cmp);
        let n = self.values.len();
        let q = self.values[n / 4];
        let interfered = self.values[n / 2] > SECOND_RATIO * q;
        self.values.clear();
        self.last_second = Some(q);
        self.seconds.push((q, interfered));
        if self.seconds.len() == 60 {
            let bad = self.seconds.iter().filter(|s| s.1).count();
            self.seconds.sort_by(|a, b| a.0.total_cmp(&b.0));
            self.minutes.push(Minute {
                index: self.minute_index,
                power: self.seconds[MINUTE_INDEX].0,
                interfered: bad > MINUTE_INTERFERED,
                interfered_seconds: bad,
            });
            self.minute_index += 1;
            self.seconds.clear();
        }
    }

    /// The minutes finished since the last call.
    pub fn take_minutes(&mut self) -> Vec<Minute> {
        std::mem::take(&mut self.minutes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn white_noise_of_power_1_reads_1_at_any_rate() {
        for rate in [48e3, 250e3] {
            let mut rng = sdrkit_algo_sim::rng::Rng::new(5);
            let s = std::f64::consts::FRAC_1_SQRT_2;
            let x: Vec<Complex32> = (0..(rate as usize))
                .map(|_| Complex32::new((s * rng.normal()) as f32, (s * rng.normal()) as f32))
                .collect();
            let mut m = PowerMeter::new(rate);
            m.process(&x);
            let b = m.last_chunk().unwrap();
            assert!((b - 1.0).abs() < 0.12, "{rate}: {b}");
        }
    }
}
