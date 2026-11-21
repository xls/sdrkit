//! The span search of `horus` (section 3.9 of the specification): an averaged spectrum of the whole input, and every second
//! the narrow signals in it: runs of bins 8 dB over the median of the span, joined across gaps of up to 600 Hz and at most
//! 2500 Hz wide, each at its power-weighted centre. A signal counts when two searches in a row see it within 400 Hz.

use sdrkit_algo_dsp::fft::WindowedFft;
use sdrkit_algo_dsp::spectrum::percentile;
use sdrkit_algo_dsp::window::Window;
use sdrkit_core::Complex32;

/// A run of bins counts when it is this far over the floor (8 dB).
const OVER_FLOOR: f32 = 6.3;
/// Runs closer than this join, Hz.
const GAP_HZ: f64 = 600.0;
/// A signal is at most this wide, Hz.
const WIDTH_HZ: f64 = 2500.0;
/// Two searches see one signal when they find it this close, Hz.
const SAME_HZ: f64 = 400.0;
/// The band around 0 Hz (the DC of the radio) that the search leaves out when the dial is not at the centre, Hz.
const DC_HZ: f64 = 1500.0;

/// The averaged spectrum and the signals it holds.
pub struct SpanSearch {
    rate: f64,
    fft: WindowedFft,
    ring: Vec<Complex32>,
    pos: usize,
    filled: usize,
    hop: usize,
    since: usize,
    since_search: usize,
    avg: Vec<f32>,
    spectra: u64,
    half_span_hz: f64,
    skip_dc: bool,
    last: Vec<f64>,
    work: Vec<Complex32>,
    scratch: Vec<f32>,
}

impl SpanSearch {
    /// The search of an input at `rate`. `decimated`: the input comes from the shared decimator, whose band edges alias, so
    /// only +-0.30 of the rate is searched (else +-0.45). `skip_dc`: the dial is not at the centre, so the band around 0 Hz
    /// is left out.
    pub fn new(rate: f64, decimated: bool, skip_dc: bool) -> Option<SpanSearch> {
        let n = ((rate / 60.0).ceil() as usize)
            .next_power_of_two()
            .clamp(64, 65_536);
        Some(SpanSearch {
            rate,
            fft: WindowedFft::new(n, n, Window::Hann).ok()?,
            ring: vec![Complex32::default(); n],
            pos: 0,
            filled: 0,
            hop: (0.25 * rate).round().max(1.0) as usize,
            since: 0,
            since_search: 0,
            avg: vec![0.0; n],
            spectra: 0,
            half_span_hz: if decimated { 0.30 } else { 0.45 } * rate,
            skip_dc,
            last: Vec::new(),
            work: Vec::with_capacity(n),
            scratch: Vec::new(),
        })
    }

    /// The signals of the last search, Hz from the centre.
    pub fn last(&self) -> &[f64] {
        &self.last
    }

    /// The bin width, Hz.
    pub fn bin_hz(&self) -> f64 {
        self.rate / self.avg.len() as f64
    }

    /// Takes samples. At each search (every second once 6 spectra are in) appends to `out` the signals that this search and
    /// the one before both found, Hz from the centre, and returns true.
    pub fn push(&mut self, x: &[Complex32], out: &mut Vec<f64>) -> bool {
        let n = self.ring.len();
        let mut searched = false;
        for &v in x {
            self.ring[self.pos] = v;
            self.pos = (self.pos + 1) % n;
            self.filled = (self.filled + 1).min(n);
            self.since += 1;
            self.since_search += 1;
            if self.since >= self.hop && self.filled == n {
                self.since = 0;
                self.spectrum();
            }
            if self.since_search as f64 >= self.rate && self.spectra >= 6 {
                self.since_search = 0;
                let now = self.signals();
                for &f in &now {
                    if self.last.iter().any(|g| (f - g).abs() <= SAME_HZ) {
                        out.push(f);
                    }
                }
                self.last = now;
                searched = true;
            }
        }
        searched
    }

    fn spectrum(&mut self) {
        let n = self.ring.len();
        self.work.clear();
        self.work
            .extend(self.ring[self.pos..].iter().chain(&self.ring[..self.pos]));
        let bins = self.fft.run(&self.work);
        self.spectra += 1;
        let a = if self.spectra <= 8 {
            1.0 / self.spectra as f32
        } else {
            1.0 / 8.0
        };
        for (v, b) in self.avg.iter_mut().zip(bins).take(n) {
            *v += a * (b.norm_sqr() - *v);
        }
    }

    /// The signals of the averaged spectrum now, Hz from the centre.
    fn signals(&mut self) -> Vec<f64> {
        let n = self.avg.len() as i64;
        let bin = self.bin_hz();
        let half = (self.half_span_hz / bin).floor() as i64;
        let skip_dc = self.skip_dc;
        let dc = if skip_dc {
            (DC_HZ / bin).ceil() as i64
        } else {
            -1
        };
        let at = |k: i64| self.avg[k.rem_euclid(n) as usize];
        let ks: Vec<i64> = (-half..=half).filter(|k| k.abs() > dc).collect();
        let values: Vec<f32> = ks.iter().map(|&k| at(k)).collect();
        let floor = percentile(&values, 0.5, &mut self.scratch);
        if floor.is_nan() || floor <= 0.0 {
            return Vec::new();
        }
        let th = OVER_FLOOR * floor;
        let gap = (GAP_HZ / bin).round() as i64;
        let mut out = Vec::new();
        let mut run: Option<(i64, i64)> = None;
        let close = |run: (i64, i64), out: &mut Vec<f64>| {
            let (a, b) = run;
            if (b - a) as f64 * bin > WIDTH_HZ {
                return;
            }
            let (mut w, mut s) = (0f64, 0f64);
            for k in a..=b {
                let p = f64::from((at(k) - floor).max(0.0));
                w += p;
                s += p * k as f64 * bin;
            }
            if w > 0.0 {
                out.push(s / w);
            }
        };
        for &k in &ks {
            if at(k) <= th {
                continue;
            }
            run = match run {
                // A run does not join across the band left out around 0 Hz.
                Some((a, b)) if k - b <= gap + 1 && !(skip_dc && b < 0 && k > 0) => Some((a, k)),
                Some(r) => {
                    close(r, &mut out);
                    Some((k, k))
                }
                None => Some((k, k)),
            };
        }
        if let Some(r) = run {
            close(r, &mut out);
        }
        out
    }
}
