//! The chirp detector: Hann FFTs of about 3 ms, the strong bins turned into the start time of a sweep for every candidate rate, hits
//! counted per 10 ms of start time, and a sweep declared when one start time keeps getting hits for long enough.

use sdrkit_algo_dsp::fft::Fft;
use sdrkit_algo_dsp::floor::BinMedian;
use sdrkit_algo_dsp::window::Window;
use sdrkit_core::Complex32;
use std::collections::HashMap;

/// The sweep rates searched, Hz per second.
pub const RATES_HZ_S: [f64; 5] = [100e3, 125e3, 150e3, 200e3, 500e3];
/// The width of a start-time cell, s.
pub const START_CELL_S: f64 = 0.01;
/// A bin is a chirp peak when it stands this far over the mean noise per bin (power ratio, 12 dB).
const PEAK_OVER_NOISE: f32 = 15.85;
/// A peak is at most this far under the strongest peak of its FFT (power ratio, 40 dB).
const BELOW_STRONGEST: f32 = 1e-4;
/// Peaks kept per FFT.
const MAX_PEAKS: usize = 64;
/// A sweep needs this many hits in three neighbouring start cells.
const MIN_HITS: u32 = 30;
/// ... spread over at least this much signal time, s. A wrong rate drifts out of three cells sooner.
const MIN_SPAN_S: f64 = 0.3;
/// The time slots of the hits, s. A sweep has hits in every slot of its span; two signals that cross only briefly do not.
const SLOT_S: f64 = 0.05;
/// Cells not hit for this long are forgotten, s.
const FORGET_S: f64 = 0.5;
/// Hits within this of an active sweep's start belong to it, s.
pub const TIE_S: f64 = 0.03;

/// The FFT size of the detector at `rate_hz`: the power of two nearest 3 ms of samples (at least 256).
pub fn fft_size(rate_hz: f64) -> usize {
    let n = (0.003 * rate_hz).max(256.0);
    1usize << n.log2().round() as u32
}

/// A sweep the detector found.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Found {
    /// The index of the rate in [`RATES_HZ_S`].
    pub rate: usize,
    /// The estimated time the sweep passed 0 Hz, s of signal time.
    pub start_s: f64,
    /// Hits counted.
    pub hits: u32,
    /// The strongest peak over the noise, dB.
    pub snr_db: f64,
    /// The lowest and highest radio frequencies of the hits, Hz.
    pub range_hz: (f64, f64),
}

/// A sweep being received: its hits are tied to it, not counted again.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Active {
    /// The index of the rate.
    pub rate: usize,
    /// The start used to tie hits, s.
    pub start_s: f64,
}

#[derive(Clone, Copy, Debug)]
struct Cell {
    hits: u32,
    first_s: f64,
    last_s: f64,
    sum_start: f64,
    snr: f32,
    lo: f64,
    hi: f64,
    /// Bit k: a hit in slot `base_slot + k`.
    slots: u64,
    base_slot: i64,
}

/// The detector.
pub struct Detector {
    n: usize,
    rate: f64,
    centre: f64,
    t0_s: f64,
    fft: Fft,
    win: Vec<f32>,
    buf: Vec<Complex32>,
    power: Vec<f32>,
    fill: usize,
    frame_start: u64,
    floor: BinMedian,
    peaks: Vec<(f32, usize)>,
    cells: Vec<HashMap<i64, Cell>>,
    ffts: u64,
    span: (f64, f64),
    noise: f64,
    keys: Vec<i64>,
}

const fn group_default() -> Cell {
    Cell {
        hits: 0,
        first_s: 0.0,
        last_s: 0.0,
        sum_start: 0.0,
        snr: 0.0,
        lo: 0.0,
        hi: 0.0,
        slots: 0,
        base_slot: 0,
    }
}

impl Detector {
    /// A detector for samples at `rate_hz` around `centre_hz`; sample 0 is at signal time `t0_s`. Bins beyond ±0.45 × rate and the
    /// two bins either side of the centre are skipped.
    pub fn new(rate_hz: f64, centre_hz: f64, t0_s: f64) -> Detector {
        let n = fft_size(rate_hz);
        Detector {
            n,
            rate: rate_hz,
            centre: centre_hz,
            t0_s,
            fft: Fft::forward(n),
            win: Window::Hann.coefficients(n),
            buf: vec![Complex32::new(0.0, 0.0); n],
            power: vec![0.0; n],
            fill: 0,
            frame_start: 0,
            floor: BinMedian::new(1).expect("one frame is a valid floor"),
            peaks: Vec::with_capacity(MAX_PEAKS * 4),
            cells: vec![HashMap::new(); RATES_HZ_S.len()],
            ffts: 0,
            span: (centre_hz - 0.45 * rate_hz, centre_hz + 0.45 * rate_hz),
            noise: 0.0,
            keys: Vec::with_capacity(64),
        }
    }
    /// The FFT size.
    pub fn fft_len(&self) -> usize {
        self.n
    }
    /// The bin width, Hz.
    pub fn bin_hz(&self) -> f64 {
        self.rate / self.n as f64
    }
    /// FFTs done.
    pub fn ffts(&self) -> u64 {
        self.ffts
    }
    /// The radio frequencies the detector sees, Hz.
    pub fn span_hz(&self) -> (f64, f64) {
        self.span
    }
    /// The mean noise power per bin of the last FFT (window gain included).
    pub fn noise(&self) -> f64 {
        self.noise
    }
    /// Chirp peaks counted that are not yet tied to a sweep.
    pub fn loose_peaks(&self) -> u64 {
        self.cells
            .iter()
            .flat_map(|c| c.values())
            .map(|c| c.hits as u64)
            .sum()
    }
    /// A new centre frequency: the sample count goes on; the cells are kept (they hold signal times).
    pub fn retune(&mut self, centre_hz: f64) {
        self.centre = centre_hz;
        self.span = (centre_hz - 0.45 * self.rate, centre_hz + 0.45 * self.rate);
        self.fill = 0;
    }
    /// Forgets every cell.
    pub fn clear(&mut self) {
        self.cells.iter_mut().for_each(|c| c.clear());
    }

    /// Takes samples starting at sample index `first`. Hits near one of `active` are counted in `tied` (one count per hit, by the
    /// index into `active`). Returns the sweeps found.
    pub fn push(
        &mut self,
        x: &[Complex32],
        first: u64,
        active: &[Active],
        tied: &mut [u32],
        found: &mut Vec<Found>,
    ) {
        for (i, &s) in x.iter().enumerate() {
            if self.fill == 0 {
                self.frame_start = first + i as u64;
            }
            self.buf[self.fill] = s * self.win[self.fill];
            self.fill += 1;
            if self.fill == self.n {
                self.fill = 0;
                self.frame(active, tied, found);
            }
        }
    }

    fn frame(&mut self, active: &[Active], tied: &mut [u32], found: &mut Vec<Found>) {
        let n = self.n;
        self.fft.process(&mut self.buf);
        self.ffts += 1;
        for (k, p) in self.power.iter_mut().enumerate() {
            *p = self.buf[(k + n / 2) % n].norm_sqr();
        }
        let noise = self
            .floor
            .estimate(self.power.iter().step_by(4).copied())
            .unwrap_or(0.0)
            .max(1e-30);
        self.noise = noise as f64;
        let edge = ((0.05 * n as f64) as usize).max(1);
        // Peaks far under the strongest one are its spurs, not chirps.
        let top = self.power[edge..n - edge]
            .iter()
            .copied()
            .fold(0.0f32, f32::max);
        let thr = (noise * PEAK_OVER_NOISE).max(top * BELOW_STRONGEST);
        self.peaks.clear();
        for k in edge..n - edge {
            let p = self.power[k];
            if p > thr && k.abs_diff(n / 2) > 2 && p >= self.power[k - 1] && p > self.power[k + 1] {
                self.peaks.push((p, k));
            }
        }
        if self.peaks.len() > MAX_PEAKS {
            self.peaks
                .select_nth_unstable_by(MAX_PEAKS, |a, b| b.0.total_cmp(&a.0));
            self.peaks.truncate(MAX_PEAKS);
        }
        let t = self.t0_s + (self.frame_start as f64 + 0.5 * n as f64) / self.rate;
        let df = self.rate / n as f64;
        for &(p, k) in &self.peaks {
            let off = sdrkit_algo_dsp::peak::log_parabolic(
                self.power[k - 1] as f64,
                p as f64,
                self.power[k + 1] as f64,
            )
            .offset;
            let f = self.centre + (k as f64 - (n / 2) as f64 + off) * df;
            let snr = p / noise;
            for (ri, &r) in RATES_HZ_S.iter().enumerate() {
                let start = t - f / r;
                if let Some(a) = active
                    .iter()
                    .position(|a| a.rate == ri && (a.start_s - start).abs() <= TIE_S)
                {
                    tied[a] += 1;
                    continue;
                }
                let key = (start / START_CELL_S).floor() as i64;
                let c = self.cells[ri].entry(key).or_insert(Cell {
                    hits: 0,
                    first_s: t,
                    last_s: t,
                    sum_start: 0.0,
                    snr: 0.0,
                    lo: f,
                    hi: f,
                    slots: 0,
                    base_slot: (t / SLOT_S).floor() as i64,
                });
                let k = (t / SLOT_S).floor() as i64 - c.base_slot;
                if (0..64).contains(&k) {
                    c.slots |= 1 << k;
                }
                c.hits += 1;
                c.last_s = t;
                c.sum_start += start;
                c.snr = c.snr.max(snr);
                c.lo = c.lo.min(f);
                c.hi = c.hi.max(f);
            }
        }
        for (ri, cells) in self.cells.iter_mut().enumerate() {
            cells.retain(|_, c| t - c.last_s <= FORGET_S);
            self.keys.clear();
            self.keys.extend(cells.keys().copied());
            for &key in &self.keys {
                let mut arr = [group_default(); 3];
                let mut n = 0;
                for k in key - 1..=key + 1 {
                    if let Some(c) = cells.get(&k) {
                        arr[n] = *c;
                        n += 1;
                    }
                }
                let group = &arr[..n];
                let hits: u32 = group.iter().map(|c| c.hits).sum();
                if hits < MIN_HITS {
                    continue;
                }
                let first = group
                    .iter()
                    .map(|c| c.first_s)
                    .fold(f64::INFINITY, f64::min);
                let last = group.iter().map(|c| c.last_s).fold(0.0, f64::max);
                if last - first < MIN_SPAN_S {
                    continue;
                }
                let mut slots = 0u128;
                let base = (first / SLOT_S).floor() as i64;
                for c in group {
                    let shift = c.base_slot - base;
                    if (0..64).contains(&shift) {
                        slots |= (c.slots as u128) << shift;
                    }
                }
                // Every slot of the span but one holds hits.
                let want = (last / SLOT_S).floor() as i64 - base + 1;
                if (slots.count_ones() as i64) < want - 1 {
                    continue;
                }
                let sum: f64 = group.iter().map(|c| c.sum_start).sum();
                found.push(Found {
                    rate: ri,
                    start_s: sum / hits as f64,
                    hits,
                    snr_db: 10.0
                        * (group.iter().map(|c| c.snr).fold(0.0, f32::max) as f64)
                            .max(1e-30)
                            .log10(),
                    range_hz: (
                        group.iter().map(|c| c.lo).fold(f64::INFINITY, f64::min),
                        group.iter().map(|c| c.hi).fold(0.0, f64::max),
                    ),
                });
                for k in key - 1..=key + 1 {
                    cells.remove(&k);
                }
            }
        }
    }
}
