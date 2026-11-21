//! Pulse trains: the output of a detector and the input of the slicers.

use serde::{Deserialize, Serialize};

/// How the carrier was keyed.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Modulation {
    /// On-off keying (amplitude-shift keying with the carrier off for a space).
    #[default]
    Ook,
    /// Two-level frequency-shift keying: the upper tone is the mark.
    Fsk,
}

impl Modulation {
    /// "OOK" or "FSK".
    pub fn name(self) -> &'static str {
        match self {
            Modulation::Ook => "OOK",
            Modulation::Fsk => "FSK",
        }
    }
}

/// One mark and the space after it, in samples.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Pulse {
    /// Length of the mark, samples.
    pub mark: f32,
    /// Length of the space after it, samples.
    pub space: f32,
}

impl Pulse {
    /// A pulse of `mark` then `space` samples.
    pub const fn new(mark: f32, space: f32) -> Self {
        Pulse { mark, space }
    }
}

/// One transmission as marks and spaces. A mark is the carrier on (OOK) or the upper tone (FSK). Lengths are in samples at
/// `rate_hz`, and fractional.
///
/// For OOK the space after the last mark carries no data. For FSK it is data: the time from the last mark to the end of the
/// carrier.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Train {
    /// OOK or FSK.
    pub modulation: Modulation,
    /// The rate the lengths are counted at, Hz.
    pub rate_hz: f64,
    /// The pulses, oldest first.
    pub pulses: Vec<Pulse>,
    /// The stream index of the first mark's start (fractional samples, at `rate_hz`).
    pub start: f64,
    /// The frequency of the signal, in Hz from the centre of the stream the detector read (the tone midpoint for FSK, 0 for OOK
    /// unless the caller sets it).
    pub offset_hz: f64,
    /// FSK deviation (half the distance between the tones), Hz. It is 0 for OOK.
    pub deviation_hz: f64,
    /// Mean power of the marks, dB relative to full scale (a power of 1).
    pub level_dbfs: f32,
    /// Mark power over the noise floor, dB.
    pub snr_db: f32,
}

impl sdrkit_graph::Item for Train {
    const KIND: &'static str = "ookdev.train";
}

impl Train {
    /// An empty train at `rate_hz`.
    pub fn new(modulation: Modulation, rate_hz: f64) -> Self {
        Train {
            modulation,
            rate_hz,
            ..Default::default()
        }
    }

    /// A train from (mark, space) pairs given in microseconds (device documentation and tests).
    pub fn from_us(modulation: Modulation, rate_hz: f64, pairs: &[(f32, f32)]) -> Self {
        let mut t = Train::new(modulation, rate_hz);
        let k = (rate_hz * 1e-6) as f32;
        t.pulses
            .extend(pairs.iter().map(|&(m, s)| Pulse::new(m * k, s * k)));
        t
    }

    /// `samples` at the train's rate in microseconds.
    pub fn us(&self, samples: f32) -> f32 {
        (samples as f64 * 1e6 / self.rate_hz) as f32
    }

    /// `us` microseconds in samples at the train's rate.
    pub fn samples(&self, us: f32) -> f32 {
        (us as f64 * self.rate_hz * 1e-6) as f32
    }

    /// The start of the first mark in seconds of the stream.
    pub fn time_s(&self) -> f64 {
        if self.rate_hz > 0.0 {
            self.start / self.rate_hz
        } else {
            0.0
        }
    }

    /// The whole train (marks and spaces, the last space included) in samples.
    pub fn len_samples(&self) -> f64 {
        self.pulses
            .iter()
            .map(|p| p.mark as f64 + p.space as f64)
            .sum()
    }

    /// The space after pulse `i` as a slicer reads it: for OOK the last space is the end of the transmission (`f32::INFINITY`).
    /// otherwise its length in samples.
    pub fn space_at(&self, i: usize) -> f32 {
        if self.modulation == Modulation::Ook && i + 1 == self.pulses.len() {
            f32::INFINITY
        } else {
            self.pulses[i].space
        }
    }

    /// Whether every length is finite and not negative, and the rate is positive. Slicers refuse other trains.
    pub fn is_valid(&self) -> bool {
        self.rate_hz > 0.0
            && self.rate_hz.is_finite()
            && self.pulses.iter().all(|p| {
                p.mark.is_finite() && p.space.is_finite() && p.mark >= 0.0 && p.space >= 0.0
            })
    }

    /// Merges every inner run (mark or space) shorter than `width` samples into the run before it, together with the run after
    /// it. Such runs are noise that splits a pulse or a gap. The first mark and the last space are never merged away. It works in place and allocates
    /// nothing.
    pub fn despike(&mut self, width: f32) {
        merge_short(&mut self.pulses, width);
    }

    /// Folds slivers with a limit learnt from the train itself: from every run except the last space, those at least a tenth of
    /// their median. Their lower quartile divided by 3 (at least `min_samples`) is the limit. A first mark under the limit is
    /// dropped with its space. Then every inner run under it is merged as [`despike`](Self::despike) does. Trains of fewer than
    /// 4 runs are left as they are. `scratch` is reused (it grows to twice the number of pulses once).
    pub fn fold_slivers(&mut self, min_samples: f32, scratch: &mut Vec<f32>) {
        let n = self.pulses.len();
        // Runs counted without the last space: 2n - 1.
        if n <= 2 {
            return;
        }
        scratch.clear();
        for (i, p) in self.pulses.iter().enumerate() {
            scratch.push(p.mark);
            if i + 1 < n {
                scratch.push(p.space);
            }
        }
        let med = quantile(scratch, 0.5);
        scratch.retain(|&l| l >= med / 10.0);
        let lim = (quantile(scratch, 0.25) / 3.0).max(min_samples);
        let drop = self.pulses.iter().take_while(|p| p.mark < lim).count();
        if drop > 0 {
            self.pulses.drain(..drop.min(n));
        }
        merge_short(&mut self.pulses, lim);
    }
}

/// The `q` quantile (0 to 1) of `v` by the nearest-rank rule (`v` is sorted in place). It is 0 for an empty slice.
pub(crate) fn quantile(v: &mut [f32], q: f64) -> f32 {
    if v.is_empty() {
        return 0.0;
    }
    v.sort_unstable_by(|a, b| a.total_cmp(b));
    let i = ((v.len() - 1) as f64 * q).round() as usize;
    v[i.min(v.len() - 1)]
}

/// Run `i` of the flattened train (even: the mark of pulse i/2, odd: its space).
#[inline]
fn run(p: &[Pulse], i: usize) -> f32 {
    if i.is_multiple_of(2) {
        p[i / 2].mark
    } else {
        p[i / 2].space
    }
}

#[inline]
fn set_run(p: &mut [Pulse], i: usize, v: f32) {
    if i.is_multiple_of(2) {
        p[i / 2].mark = v;
    } else {
        p[i / 2].space = v;
    }
}

/// Merges every inner run shorter than `lim` with its two neighbours into the run before it (the first run and the last two are
/// never the short one). In place: the write position never passes the read position.
fn merge_short(p: &mut Vec<Pulse>, lim: f32) {
    let n = 2 * p.len();
    if n < 4 {
        return;
    }
    let mut acc = run(p, 0);
    let mut j = 0usize;
    let mut i = 1usize;
    while i < n {
        let v = run(p, i);
        if i + 2 < n && v < lim {
            acc += v + run(p, i + 1);
            i += 2;
            continue;
        }
        set_run(p, j, acc);
        j += 1;
        acc = v;
        i += 1;
    }
    set_run(p, j, acc);
    j += 1;
    p.truncate(j / 2);
}
