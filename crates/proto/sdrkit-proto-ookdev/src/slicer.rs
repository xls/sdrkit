//! Line-code slicers: a [`Train`] to [`BitRows`] by a device's [`Coding`] and [`Timing`].
//!
//! Every device protocol names one coding and its timing in microseconds: short width `short`, long width `long`, reset limit
//! `reset` (a space at least this long ends the transmission), gap limit `gap` (a space this long starts a new row, 0 for none),
//! sync width `sync` (0 for none) and tolerance `tolerance` (0: windows split at the midpoints between the widths). The slicer
//! converts the train into rows of bits and outputs them each time a reset space ends a transmission and at the end of the
//! train, so one train can produce several sets of rows.
//!
//! | Coding | Each bit is | Rule |
//! |---|---|---|
//! | [`Coding::Pwm`] | a mark | A short mark is 1 and a long mark is 0. A mark near `sync` starts a new row. A mark shorter than every window is ignored. Any other mark starts a new row. A space over `gap` starts a new row. |
//! | [`Coding::Ppm`] | a space | A short space is 0 and a long space is 1. With no tolerance, a space below `(short + long) / 2 + 1 µs` is 0, and from there to `gap` (or `reset`) it is 1. A space near `sync`, or any other space below `reset`, starts a new row. |
//! | [`Coding::Nrz`] | a bit period | A mark is `round(mark / T)` ones and a space is `round(space / T)` zeros, at most `gap / T` (or `reset / T`). A space over `gap` starts a new row. `T` starts at `short`, and the slicer measures it from the train (below). |
//! | [`Coding::ManchesterZero`] | two half-bit periods of `short` | The rows start with a 0 bit. The slicer adds each run to an accumulator. If a mark takes it past 1.5 `short`, the slicer adds a 1 and clears it. If a space does, it adds a 0 and clears it. This reads "1 = high then low, 0 = low then high" when the first bit is a known 0. |
//!
//! The NRZ bit width. `T` becomes the mean of the longest run of at least 12 consecutive single-width runs (each rounding to
//! one `short`), a preamble. Without a preamble, it is the mean over every run within `tolerance` (default `long / 4`) of `short` or
//! `2 short` (halved), when there are more than 20 of them. The slicer stores the measured width in [`BitRows::bit_us`]. A protocol
//! with [`Timing::width_check`] refuses rows whose width is more than 20 % from its own (two protocols with different bit rates
//! then never both decode one train).
//!
//! The last space of an OOK train always ends the transmission ([`Train::space_at`]). The last space of an FSK train is data.
//!
//! Manchester and FM0 codes inside an NRZ row are read by [`BitRows::decode_manchester`] and [`BitRows::decode_fm0`] after NRZ
//! slicing at the chip rate.
//!
//! ```
//! use sdrkit_proto_ookdev::{slicer::{Coding, Slicer, Timing}, Modulation, Train};
//! // PPM: 500 µs marks, a 1000 µs space is 0 and 2000 µs is 1.
//! let t = Timing::new(Coding::Ppm, 1000.0, 2000.0, 5000.0).with_gap(3000.0);
//! let train = Train::from_us(Modulation::Ook, 1e6, &[(500.0, 1000.0), (500.0, 2000.0), (500.0, 2000.0), (500.0, 0.0)]);
//! let mut s = Slicer::new();
//! let mut got = Vec::new();
//! s.slice(&t, &train, |rows| got.push(rows.row(0).bytes.to_vec())).unwrap();
//! assert_eq!(got, vec![vec![0b0110_0000]]);
//! ```

use crate::rows::BitRows;
use crate::train::Train;
use crate::Error;
use serde::{Deserialize, Serialize};

/// How a device codes its bits as marks and spaces (module documentation).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Coding {
    /// Pulse width: each mark is a bit.
    Pwm,
    /// Pulse position (pulse distance): each space is a bit.
    Ppm,
    /// Non-return-to-zero (PCM): each bit period is a level.
    Nrz,
    /// Manchester with a known 0 as the first bit.
    ManchesterZero,
}

impl Coding {
    /// "PWM", "PPM", "NRZ" or "Manchester".
    pub fn name(self) -> &'static str {
        match self {
            Coding::Pwm => "PWM",
            Coding::Ppm => "PPM",
            Coding::Nrz => "NRZ",
            Coding::ManchesterZero => "Manchester",
        }
    }
}

/// A coding and its timing in microseconds. A 0 means "not given" (module documentation).
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Timing {
    /// The coding.
    pub coding: Coding,
    /// Short width (the bit width for NRZ, the half-bit for Manchester), µs.
    pub short: f32,
    /// Long width, µs.
    pub long: f32,
    /// A space at least this long ends the transmission, µs.
    pub reset: f32,
    /// A space this long starts a new row, µs (0: none).
    pub gap: f32,
    /// Sync width, µs (0: none).
    pub sync: f32,
    /// Window half-width around each width, µs (0: split at the midpoints).
    pub tolerance: f32,
    /// NRZ only: refuse rows whose measured bit width is more than 20 % from `short`.
    pub width_check: bool,
}

impl Timing {
    /// A coding with its short and long widths and reset limit, with no gap, sync or tolerance.
    pub const fn new(coding: Coding, short: f32, long: f32, reset: f32) -> Self {
        Timing {
            coding,
            short,
            long,
            reset,
            gap: 0.0,
            sync: 0.0,
            tolerance: 0.0,
            width_check: false,
        }
    }
    /// With a gap limit.
    pub const fn with_gap(mut self, gap: f32) -> Self {
        self.gap = gap;
        self
    }
    /// With a sync width.
    pub const fn with_sync(mut self, sync: f32) -> Self {
        self.sync = sync;
        self
    }
    /// With a tolerance.
    pub const fn with_tolerance(mut self, tolerance: f32) -> Self {
        self.tolerance = tolerance;
        self
    }
    /// With the NRZ width test.
    pub const fn with_width_check(mut self) -> Self {
        self.width_check = true;
        self
    }

    /// Whether the widths make sense: finite and not negative, `short` and `reset` positive, and `long` positive for PWM and PPM.
    pub fn is_valid(&self) -> bool {
        let all = [
            self.short,
            self.long,
            self.reset,
            self.gap,
            self.sync,
            self.tolerance,
        ];
        all.iter().all(|v| v.is_finite() && *v >= 0.0)
            && self.short > 0.0
            && self.reset > 0.0
            && (!matches!(self.coding, Coding::Pwm | Coding::Ppm) || self.long > 0.0)
    }
}

/// Timing converted to samples at one rate.
#[derive(Clone, Copy, Debug)]
struct Widths {
    s: f32,
    l: f32,
    r: f32,
    g: f32,
    y: f32,
    t: f32,
    one_us: f32,
}

impl Widths {
    fn new(t: &Timing, rate_hz: f64) -> Widths {
        let k = (rate_hz * 1e-6) as f32;
        Widths {
            s: t.short * k,
            l: t.long * k,
            r: t.reset * k,
            g: t.gap * k,
            y: t.sync * k,
            t: t.tolerance * k,
            one_us: k,
        }
    }
}

/// The slicer: holds the rows it fills and its scratch, so slicing allocates nothing once built.
#[derive(Clone, Debug, Default)]
pub struct Slicer {
    rows: BitRows,
}

impl Slicer {
    /// A slicer with rows of the default capacity.
    pub fn new() -> Self {
        Self::default()
    }

    /// A slicer whose rows hold `max_rows` rows of `max_bits` bits.
    pub fn with_capacity(max_rows: usize, max_bits: usize) -> Self {
        Slicer {
            rows: BitRows::new(max_rows, max_bits),
        }
    }

    /// Slices `train` by `timing`, calling `f` with each set of rows that is not empty (empty rows at the end removed). Errors
    /// for a train with lengths that are not finite or are negative, or a timing that is not valid.
    pub fn slice(
        &mut self,
        timing: &Timing,
        train: &Train,
        mut f: impl FnMut(&BitRows),
    ) -> Result<(), Error> {
        if !train.is_valid() {
            return Err(Error::BadTrain);
        }
        if !timing.is_valid() {
            return Err(Error::BadTiming);
        }
        let w = Widths::new(timing, train.rate_hz);
        self.rows.clear();
        match timing.coding {
            Coding::Pwm => self.pwm(&w, train, &mut f),
            Coding::Ppm => self.ppm(&w, train, &mut f),
            Coding::Nrz => self.nrz(&w, timing, train, &mut f),
            Coding::ManchesterZero => self.manchester_zero(&w, train, &mut f),
        }
        Ok(())
    }

    /// Hands the rows over (when not empty) and starts again.
    fn finish(&mut self, f: &mut impl FnMut(&BitRows)) {
        self.rows.trim();
        if !self.rows.is_empty() {
            f(&self.rows);
        }
        let us = self.rows.bit_us;
        self.rows.clear();
        self.rows.bit_us = us;
    }

    fn pwm(&mut self, w: &Widths, train: &Train, f: &mut impl FnMut(&BitRows)) {
        // Windows (low, high, symbol): 1 short, 0 long, 2 sync.
        let mut wins = [(0f32, 0f32, 0u8); 3];
        let nw = if w.t > 0.0 {
            wins[0] = (w.s - w.t, w.s + w.t, 1);
            wins[1] = (w.l - w.t, w.l + w.t, 0);
            if w.y > 0.0 {
                wins[2] = (w.y - w.t, w.y + w.t, 2);
                3
            } else {
                2
            }
        } else {
            let mut c = [(w.s, 1u8), (w.l, 0u8), (w.y, 2u8)];
            let k = if w.y > 0.0 { 3 } else { 2 };
            c[..k].sort_by(|a, b| a.0.total_cmp(&b.0));
            for i in 0..k {
                let lo = if i == 0 {
                    c[0].0 - (c[1].0 - c[0].0) / 2.0
                } else {
                    (c[i - 1].0 + c[i].0) / 2.0
                };
                let hi = if i + 1 == k {
                    c[i].0 + (c[i].0 - c[i - 1].0) / 2.0
                } else {
                    (c[i].0 + c[i + 1].0) / 2.0
                };
                wins[i] = (lo, hi, c[i].1);
            }
            k
        };
        let wins = &wins[..nw];
        let lowest = wins.iter().map(|x| x.0).fold(f32::MAX, f32::min);
        for i in 0..train.pulses.len() {
            let m = train.pulses[i].mark;
            let sp = train.space_at(i);
            if m >= lowest {
                match wins.iter().find(|x| m >= x.0 && m < x.1) {
                    Some(x) if x.2 == 2 => {
                        if !self.rows.row_empty() {
                            self.rows.new_row();
                        }
                    }
                    Some(x) => self.rows.push(x.2),
                    None => {
                        if !self.rows.row_empty() {
                            self.rows.new_row();
                        }
                    }
                }
            }
            if sp > w.r {
                self.finish(f);
            } else if w.g > 0.0 && sp > w.g && !self.rows.row_empty() {
                self.rows.new_row();
            }
        }
        self.finish(f);
    }

    fn ppm(&mut self, w: &Widths, train: &Train, f: &mut impl FnMut(&BitRows)) {
        let upper = if w.g > 0.0 { w.g } else { w.r };
        for i in 0..train.pulses.len() {
            let sp = train.space_at(i);
            if sp >= w.r {
                self.finish(f);
                continue;
            }
            if w.t > 0.0 {
                if (sp - w.s).abs() <= w.t {
                    self.rows.push(0);
                } else if (sp - w.l).abs() <= w.t {
                    self.rows.push(1);
                } else {
                    self.rows.new_row();
                }
            } else {
                let mid = (w.s + w.l) / 2.0 + w.one_us;
                if w.y > 0.0 && (sp - w.y).abs() < (w.y - w.l).abs() / 2.0 {
                    self.rows.new_row();
                } else if sp > 0.0 && sp < mid {
                    self.rows.push(0);
                } else if sp >= mid && sp < upper {
                    self.rows.push(1);
                } else {
                    self.rows.new_row();
                }
            }
        }
        self.finish(f);
    }

    /// The NRZ bit width in samples (module documentation).
    fn nrz_width(w: &Widths, timing: &Timing, train: &Train) -> f32 {
        let n = train.pulses.len();
        // Runs in order, the last space left out.
        let runs = (0..2 * n - 1).map(|i| {
            let p = &train.pulses[i / 2];
            if i % 2 == 0 {
                p.mark
            } else {
                p.space
            }
        });
        let (mut best_len, mut best_mean) = (0usize, 0f32);
        let (mut run, mut sum) = (0usize, 0f32);
        for e in runs.clone() {
            if (e / w.s).round() == 1.0 {
                run += 1;
                sum += e;
                if run > best_len {
                    best_len = run;
                    best_mean = sum / run as f32;
                }
            } else {
                run = 0;
                sum = 0.0;
            }
        }
        if best_len >= 12 {
            return best_mean;
        }
        let tol = if timing.tolerance > 0.0 {
            w.t
        } else {
            w.l / 4.0
        };
        let (mut k, mut acc) = (0usize, 0f32);
        for e in runs {
            if (e - w.s).abs() <= tol {
                k += 1;
                acc += e;
            } else if (e - 2.0 * w.s).abs() <= tol {
                k += 1;
                acc += e / 2.0;
            }
        }
        if k > 20 {
            acc / k as f32
        } else {
            w.s
        }
    }

    fn nrz(&mut self, w: &Widths, timing: &Timing, train: &Train, f: &mut impl FnMut(&BitRows)) {
        if train.pulses.is_empty() {
            return;
        }
        let tb = Self::nrz_width(w, timing, train);
        let bit_us = tb / w.one_us;
        if timing.width_check && (bit_us - timing.short).abs() > 0.2 * timing.short {
            return;
        }
        self.rows.bit_us = bit_us;
        let cap = if w.g > 0.0 { w.g } else { w.r };
        let max_zeros = (cap / tb) as usize;
        let max_bits = self.rows.max_bits();
        for i in 0..train.pulses.len() {
            let m = train.pulses[i].mark;
            let sp = train.space_at(i);
            let ones = ((m / tb).round() as usize).min(max_bits);
            for _ in 0..ones {
                self.rows.push(1);
            }
            if sp > w.r {
                self.finish(f);
                continue;
            }
            let zeros = ((sp / tb).round() as usize).min(max_zeros);
            for _ in 0..zeros {
                self.rows.push(0);
            }
            if w.g > 0.0 && sp > w.g {
                self.rows.new_row();
            }
        }
        self.finish(f);
    }

    fn manchester_zero(&mut self, w: &Widths, train: &Train, f: &mut impl FnMut(&BitRows)) {
        let lim = 1.5 * w.s;
        let mut acc = 0f32;
        self.rows.push(0);
        for i in 0..train.pulses.len() {
            let m = train.pulses[i].mark;
            let sp = train.space_at(i);
            if acc + m > lim {
                self.rows.push(1);
                acc = 0.0;
            } else {
                acc += m;
            }
            if sp > w.r {
                // A row holding only the leading 0 is not a transmission.
                if self.rows.len(0) > 1 || self.rows.num_rows() > 1 {
                    self.finish(f);
                } else {
                    self.rows.clear();
                }
                self.rows.push(0);
                acc = 0.0;
                continue;
            }
            if acc + sp > lim {
                self.rows.push(0);
                acc = 0.0;
            } else {
                acc += sp;
            }
        }
        if self.rows.len(0) > 1 || self.rows.num_rows() > 1 {
            self.finish(f);
        }
    }
}
