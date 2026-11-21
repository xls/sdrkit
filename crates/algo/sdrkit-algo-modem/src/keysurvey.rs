//! A survey for keyed carriers: narrow carriers switched on and off in the rhythm of Morse code (ITU-R M.1677-1), found in
//! successive power spectra of a wide block without decoding them.
//!
//! The input is one power spectrum per frame, lowest frequency first, bin `bins / 2` at the middle of the block, bins
//! `bin_hz` apart and frames `frame_s` apart: in dB ([`KeyedSurvey::push_db`]), as linear power
//! ([`KeyedSurvey::push_power`]) or in a unit of the caller ([`KeyedSurvey::push_with`]). [`SampleSurvey`] makes the frames from complex samples with back-to-back Blackman-Harris
//! transforms.
//!
//! | Step | What it does |
//! |---|---|
//! | Burst | the level of a frame is the median of about 512 of its bins; a frame at twice the highest level of the block so far is a burst over the whole block and is brought down to that level, for at most 2 frames in a row (a longer rise is a new level, and the noise is found again); a fall to half within the first 3 frames starts the block again; a block without noise is not judged |
//! | Pair | with `pair`, the states come from the mean of each frame and the one before; a strong bin under 0.3 of its highest mean counts as noise |
//! | Noise | the mean noise per bin of each segment of [`SEGMENT`] bins from its median ([`BinMedian`]), on the first frame and every [`FLOOR_EVERY`] frames after |
//! | State | a bin is on when its power is `on_db` over the noise, or `hold_db` over it and on in the frame before; the states of the last `frames` frames are kept as bit planes |
//! | Counts | for each bin since [`KeyedSurvey::reset`]: the frames on, the marks (starts of an on run), the sum of the power over the noise while on |
//! | Selection | a bin with an on share from `min_duty` to `max_duty` and at least `min_marks` marks, not within `guard_bins` of a bin on for more than `max_duty` (the skirt of a steady carrier); of such bins within `merge_bins` of each other the one with the most level over the noise, and not one that keys with a stronger one within `guard_bins + 1` |
//! | Width | the share of on bins in the ring from `guard_bins` to `ring_hz` on each side, over the frames in which the bin is on and over those in which it is off; a wide signal (voice, data) lights a ring while the bin is on (over `max_ring`) or moves its power into a ring while the bin is off (a rise over `max_ring_shift`; for a weak bin over `weak_side_shift`, or over `weak_ring_shift` in both rings together) |
//! | Rhythm | the runs of the bin from its first to its last whole mark: a mark of one frame dropped; refused when marks of one length start at regular times (`periodic_share` of the intervals within `max_periodic` of their median); then gaps and then marks of at most 0.3 dots merged into the runs around them, with the dot of a first fit or, where that fit has an error over half of `max_error`, a third of the longest mark is 1.4 times its dot, and the fit after it has a lower error and a dot as long and no longer than 0.4 of that mark, that third (dropouts break long marks, and a fit follows the pieces); refused with 6 marks or more when the states repeat at one period (over `max_repeat`; over `pulse_repeat` with 8 marks or more and under `min_inner` of the gaps as short as a dot); pauses over [`LONG_GAP_DOTS`] dots left out; fitted to dots, dashes and gaps of one dot length with [`slowmorse::refine`] and [`slowmorse::error`], with marks read long by a part of a frame; refused when the error exceeds `max_error` (with more than `few_runs` runs, up to `error_rise` more), when there are not both dots and dashes (or marks of one length with gaps of one and of three dots), or when the dot is outside `min_wpm` to `max_wpm` or shorter than `min_dot_frames` |
//!
//! With `occupancy` set, the survey also sums the power of every bin over the frames (a burst brought down as above), and
//! [`KeyedSurvey::occupied`] gives the occupied channels of that sum ([`crate::occupancy`]): signals of any modulation a few
//! kHz wide, which the steps above refuse.
//!
//! A steady carrier is on in nearly every frame and noise in very few, so the on share refuses both. Pulses of one length
//! (telemetry, TDMA) start at regular times and have no dashes; in noise, where some are missed and some run together,
//! their states still repeat at the period over the whole look, which text does not. [`SurveyConfig::new`] has the
//! tests of `pair`, the weak bins and the repeat off; [`SurveyConfig::paired`] has them on, for carriers near 10 dB.
//! Marks of one length pass only with gaps of one dot and of
//! three or more, as in the letters O and S. A dot shorter than two frames is not measured: at 13 ms
//! frames the top speed is about 45 WPM.
//!
//! ```
//! use sdrkit_algo_modem::keysurvey::{KeyedSurvey, SurveyConfig};
//! let cfg = SurveyConfig::new(1024, 76.3, 0.0131, 2.0);
//! let mut s = KeyedSurvey::new(cfg).unwrap();
//! s.push_db(&vec![-100.0; 1024]).unwrap();
//! let mut found = Vec::new();
//! s.candidates(&mut found);
//! assert!(found.is_empty());
//! ```

use crate::occupancy::{OccupancyConfig, OccupancyFinder, Occupied};
use crate::slowmorse::{self, Element, Key, Keying, Run};
use crate::{Complex32, ModemError, Param};
use sdrkit_algo_dsp::approx::power_of_db;
use sdrkit_algo_dsp::fft::WindowedFft;
use sdrkit_algo_dsp::floor::BinMedian;
use sdrkit_algo_dsp::window::Window;

/// The bins of one segment of the noise estimate.
pub const SEGMENT: usize = 1024;
/// The noise is found again every this many frames.
pub const FLOOR_EVERY: u64 = 8;
/// A gap longer than this many dots is a pause and is left out of the fit.
pub const LONG_GAP_DOTS: f64 = 10.0;
/// The level of a frame is the median of about this many of its bins.
const LEVEL_BINS: usize = 512;
/// A frame whose level is this many times the highest level of the block before it is a burst.
const JUMP: f32 = 2.0;
/// The most frames in a row that are taken as a burst; a longer rise is a new level.
const MOST_HELD: u32 = 2;
/// With `pair`, the noise of the means is found from every this many bins of a segment.
const PAIR_FLOOR_STEP: usize = 4;
/// A bin this many times the noise is strong: with `pair` by its highest mean in the look, for `weak_ring_shift` by its
/// mean power while on.
const STRONG: f32 = 30.0;
/// With `pair`, a strong bin under this share of its highest mean is off.
const EDGE: f32 = 0.3;
/// The most multiples of a period over which [`KeyedSurvey::repeats`] looks.
const REPEAT_LAGS: usize = 6;
/// The test of `max_repeat` needs this many marks.
const REPEAT_MARKS: u32 = 6;
/// The test of `min_inner` needs this many marks.
const GAP_MARKS: u32 = 8;
/// The parts of a frame by which the fit tries marks read long and gaps short.
const BIAS_FRAMES: [f64; 4] = [0.0, 0.5, 1.0, 1.5];
/// Gaps and marks shorter than this many dots are a dropout and a spike.
const REPAIR: f64 = 0.3;
/// A third of the longest mark this many times the fitted dot, with a fit error over [`PIECES_ERROR`] of the largest,
/// says that the fit followed the pieces of broken marks.
const PIECES: f64 = 1.4;
/// The share of `max_error` over which a fit may have followed pieces.
const PIECES_ERROR: f64 = 0.5;

/// The settings of a [`KeyedSurvey`].
#[derive(Clone, Debug, PartialEq)]
pub struct SurveyConfig {
    /// The bins per frame.
    pub bins: usize,
    /// The distance between bins, Hz.
    pub bin_hz: f64,
    /// The time between frames, s.
    pub frame_s: f64,
    /// The frames whose states are kept for the rhythm test.
    pub frames: usize,
    /// A bin turns on this far over the noise, dB.
    pub on_db: f32,
    /// A bin that is on stays on this far over the noise, dB.
    pub hold_db: f32,
    /// The lowest speed, words a minute.
    pub min_wpm: f64,
    /// The highest speed, words a minute.
    pub max_wpm: f64,
    /// The shortest dot, in frames: the frames resolve no shorter one.
    pub min_dot_frames: f64,
    /// The fewest marks.
    pub min_marks: u32,
    /// The smallest share of frames on.
    pub min_duty: f64,
    /// The largest share of frames on.
    pub max_duty: f64,
    /// Bins on each side of a chosen bin that are not chosen again.
    pub merge_bins: usize,
    /// Bins on each side that belong to the carrier itself (the main lobe of the window).
    pub guard_bins: usize,
    /// The outer edge of the rings on each side, Hz from the bin.
    pub ring_hz: f64,
    /// The largest share of on bins in a ring while the bin is on.
    pub max_ring: f64,
    /// The largest rise of that share while the bin is off: the power of a wide signal moves to the rings, that of a keyed
    /// carrier stops.
    pub max_ring_shift: f64,
    /// The limit as `max_ring_shift` for a bin whose mean power while on is under 30 times the noise: a weak wide signal
    /// moves little power, and a weak carrier has no sidebands of its keying over the noise.
    pub weak_side_shift: f64,
    /// For such a bin, the largest rise of the share of on bins in both rings together while the bin is off.
    pub weak_ring_shift: f64,
    /// The distance from the median interval between the starts of marks, as a share of it, within which an interval is
    /// regular.
    pub max_periodic: f64,
    /// Marks with at least this share of regular intervals are a pulse train.
    pub periodic_share: f64,
    /// The largest fit error of the runs ([`slowmorse::error`]) for up to `few_runs` runs.
    pub max_error: f64,
    /// The runs up to which `max_error` holds as it is.
    pub few_runs: usize,
    /// How far the largest fit error rises with more runs: by this times `1 - few_runs / runs`. A fit of few runs reads
    /// low, as the dot length and the bias follow them; the error of the same keying grows with the runs fitted.
    pub error_rise: f64,
    /// The most that the states of a bin with 6 marks or more may repeat at one period: the correlation of the states
    /// with themselves a period later, 1 for states that repeat exactly. A pulse train repeats, text does not.
    pub max_repeat: f64,
    /// The least share of gaps of one dot among the gaps that are no pause, for 8 marks or more: the gaps of a pulse
    /// train are of one length, and with its marks read as dots most are longer than a dot. A bin under this share is
    /// refused when its states repeat over `pulse_repeat`.
    pub min_inner: f64,
    /// The limit as `max_repeat` for a bin under `min_inner`.
    pub pulse_repeat: f64,
    /// The most candidates given.
    pub most: usize,
    /// The states come from the mean of each frame and the one before it; the first frame of a block gives none. A strong
    /// bin that falls under 0.3 of its highest mean counts as noise, so its marks are not read long.
    pub pair: bool,
    /// With settings here, the power of the bins is summed over the frames for [`KeyedSurvey::occupied`].
    pub occupancy: Option<OccupancyConfig>,
}

impl SurveyConfig {
    /// The defaults for frames of `bins` bins `bin_hz` apart that come `frame_s` apart, over a dwell of `dwell_s`.
    pub fn new(bins: usize, bin_hz: f64, frame_s: f64, dwell_s: f64) -> SurveyConfig {
        let frames = if frame_s > 0.0 && dwell_s.is_finite() {
            (dwell_s / frame_s).ceil().clamp(1.0, 65_535.0) as usize
        } else {
            1
        };
        SurveyConfig {
            bins,
            bin_hz,
            frame_s,
            frames,
            on_db: 6.0,
            hold_db: 3.0,
            min_wpm: 5.0,
            max_wpm: 60.0,
            min_dot_frames: 2.0,
            min_marks: 3,
            min_duty: 0.12,
            max_duty: 0.8,
            merge_bins: 2,
            guard_bins: 3,
            ring_hz: 1500.0,
            max_ring: 0.2,
            max_ring_shift: 0.05,
            weak_side_shift: 1.0,
            weak_ring_shift: 1.0,
            max_periodic: 0.15,
            periodic_share: 0.75,
            max_error: 0.15,
            few_runs: 12,
            error_rise: 0.0,
            max_repeat: 1.0,
            min_inner: 0.0,
            pulse_repeat: 1.0,
            most: 64,
            pair: false,
            occupancy: None,
        }
    }

    /// The settings for weak carriers, with `pair`. The mean halves the variance of the noise, so a bin turns on and
    /// stays on at lower levels than a single frame allows; the on share of noise that passes those levels stays under
    /// `min_duty`. The noise is taken from a median as of single frames, which reads 0.8 dB high on the means: the
    /// levels are over that.
    pub fn paired(bins: usize, bin_hz: f64, frame_s: f64, dwell_s: f64) -> SurveyConfig {
        SurveyConfig {
            on_db: 3.7,
            hold_db: 1.5,
            min_duty: 0.25,
            max_ring: 0.12,
            weak_side_shift: 0.03,
            weak_ring_shift: 0.025,
            max_error: 0.3,
            error_rise: 0.05,
            max_repeat: 0.45,
            min_inner: 0.3,
            pulse_repeat: 0.25,
            pair: true,
            ..SurveyConfig::new(bins, bin_hz, frame_s, dwell_s)
        }
    }
}

/// A keyed carrier found in the block.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Candidate {
    /// The frequency from the middle of the block (bin `bins / 2`), Hz.
    pub offset_hz: f64,
    /// The bin.
    pub bin: usize,
    /// How well the runs fit Morse timing, 0 to 1: one less the fit error.
    pub score: f32,
    /// The speed, words a minute (PARIS timing).
    pub wpm: f32,
    /// The mean power over the noise while on, dB.
    pub snr_db: f32,
    /// The whole marks fitted.
    pub marks: u32,
    /// The share of frames on.
    pub duty: f32,
}

/// The survey of one block (module docs). Nothing is allocated after [`KeyedSurvey::new`] except by
/// [`KeyedSurvey::candidates`] and [`KeyedSurvey::occupied`].
#[derive(Clone, Debug)]
pub struct KeyedSurvey {
    cfg: SurveyConfig,
    words: usize,
    planes: Vec<u64>,
    head: usize,
    kept: usize,
    judged: u64,
    frames: u64,
    level: f32,
    held: u32,
    has_last: bool,
    floor_at: u64,
    pair_floor_at: u64,
    last: Vec<f32>,
    mean: Vec<f32>,
    peak: Vec<f32>,
    pair_floor: Vec<f32>,
    pair_median: BinMedian,
    lin: Vec<f32>,
    floor: Vec<f32>,
    on: Vec<u16>,
    marks: Vec<u16>,
    excess: Vec<f32>,
    median: BinMedian,
    picks: Vec<usize>,
    taken: Vec<bool>,
    frame_runs: Vec<(bool, u32)>,
    col: Vec<bool>,
    merged: Vec<(bool, u32)>,
    spiked: Vec<(bool, u32)>,
    starts: Vec<f64>,
    runs: Vec<Run>,
    used: Vec<Run>,
    total: Vec<f32>,
    totalled: u32,
    finder: Option<OccupancyFinder>,
}

/// What [`KeyedSurvey::burst`] says of a frame.
#[derive(Clone, Copy, PartialEq)]
enum Burst {
    /// The frame is as the ones before.
    No,
    /// The frame is a burst: times this it has the level of the frames before.
    Now(f32),
    /// The level is a new one: the noise is found again.
    Step,
    /// The frames before, the first of the block, were a burst.
    Before,
}

fn check(cfg: &SurveyConfig) -> Result<(), ModemError> {
    if !(64..=1 << 22).contains(&cfg.bins) {
        return Param::Bins.range(64.0, (1u32 << 22) as f64, cfg.bins as f64);
    }
    if !(cfg.bin_hz.is_finite() && cfg.bin_hz > 0.0) {
        return Err(ModemError::NotAllowed {
            param: Param::BinWidth,
            got: cfg.bin_hz,
        });
    }
    if !(cfg.frame_s.is_finite() && cfg.frame_s > 0.0) {
        return Err(ModemError::NotAllowed {
            param: Param::Period,
            got: cfg.frame_s,
        });
    }
    if !(1..=65_535).contains(&cfg.frames) {
        return Param::Capacity.range(1.0, 65_535.0, cfg.frames as f64);
    }
    if !(cfg.hold_db > 0.0 && cfg.hold_db <= cfg.on_db) {
        return Err(ModemError::Conflict {
            param: Param::Hysteresis,
            with: Param::Threshold,
        });
    }
    if !(cfg.min_wpm > 0.0 && cfg.min_wpm < cfg.max_wpm) {
        return Err(ModemError::Conflict {
            param: Param::Rate,
            with: Param::Rate,
        });
    }
    Ok(())
}

impl KeyedSurvey {
    /// A survey with `cfg`. Errors for bins outside 64 to 4 194 304, a bin width or frame time that is not positive, kept
    /// frames outside 1 to 65 535, a hold level not in (0, on level], or a speed range that is empty.
    pub fn new(cfg: SurveyConfig) -> Result<KeyedSurvey, ModemError> {
        check(&cfg)?;
        let words = cfg.bins.div_ceil(64);
        let segments = cfg.bins.div_ceil(SEGMENT);
        let mut median = BinMedian::new(1).map_err(ModemError::Dsp)?;
        median.reserve(SEGMENT);
        let mut pair_median = BinMedian::new(2).map_err(ModemError::Dsp)?;
        pair_median.reserve(SEGMENT);
        let paired = if cfg.pair { cfg.bins } else { 0 };
        let finder = match &cfg.occupancy {
            Some(o) => Some(OccupancyFinder::new(cfg.bins, cfg.bin_hz, o.clone())?),
            None => None,
        };
        Ok(KeyedSurvey {
            planes: vec![0; words * cfg.frames],
            words,
            head: 0,
            kept: 0,
            judged: 0,
            frames: 0,
            level: 0.0,
            held: 0,
            has_last: false,
            floor_at: 0,
            pair_floor_at: 0,
            last: vec![0.0; paired],
            mean: vec![0.0; paired],
            peak: vec![0.0; paired],
            pair_floor: vec![f32::MAX; if cfg.pair { segments } else { 0 }],
            pair_median,
            lin: Vec::with_capacity(cfg.bins),
            floor: vec![0.0; segments],
            on: vec![0; cfg.bins],
            marks: vec![0; cfg.bins],
            excess: vec![0.0; cfg.bins],
            median,
            picks: Vec::new(),
            taken: vec![false; cfg.bins],
            frame_runs: Vec::new(),
            col: Vec::new(),
            merged: Vec::new(),
            spiked: Vec::new(),
            starts: Vec::new(),
            runs: Vec::new(),
            used: Vec::new(),
            total: vec![0.0; if finder.is_some() { cfg.bins } else { 0 }],
            totalled: 0,
            finder,
            cfg,
        })
    }

    /// The settings.
    pub fn config(&self) -> &SurveyConfig {
        &self.cfg
    }

    /// The frames pushed since the last reset.
    pub fn frames(&self) -> u64 {
        self.frames
    }

    /// Forgets every frame, for a new block.
    pub fn reset(&mut self) {
        self.frames = 0;
        self.level = 0.0;
        self.held = 0;
        self.forget();
    }

    /// Forgets the states, the counts and the noise; the count of frames pushed and the level stay.
    fn forget(&mut self) {
        self.head = 0;
        self.kept = 0;
        self.judged = 0;
        self.has_last = false;
        self.floor_at = 0;
        self.pair_floor_at = 0;
        self.peak.fill(0.0);
        self.pair_floor.fill(f32::MAX);
        self.planes.fill(0);
        self.on.fill(0);
        self.marks.fill(0);
        self.excess.fill(0.0);
        self.total.fill(0.0);
        self.totalled = 0;
    }

    /// Adds a frame of power in dB. Errors when it does not have `bins` bins.
    pub fn push_db(&mut self, bins_db: &[f32]) -> Result<(), ModemError> {
        let mut lin = std::mem::take(&mut self.lin);
        lin.clear();
        lin.extend(bins_db.iter().map(|&x| power_of_db(x)));
        let r = self.push_power(&lin);
        self.lin = lin;
        r
    }

    /// Adds a frame of linear power. Errors when it does not have `bins` bins.
    pub fn push_power(&mut self, bins: &[f32]) -> Result<(), ModemError> {
        self.push_with(bins, |x| x)
    }

    /// Adds a frame in the unit of the caller: `power` gives the linear power of a bin, and is called once or twice for
    /// each. Errors when the frame does not have `bins` bins.
    pub fn push_with(
        &mut self,
        bins: &[f32],
        power: impl Fn(f32) -> f32 + Copy,
    ) -> Result<(), ModemError> {
        let n = self.cfg.bins;
        if bins.len() != n {
            return Err(ModemError::Length {
                param: Param::Bins,
                expected: n,
                got: bins.len(),
            });
        }
        self.frames += 1;
        let gain = match self.burst(bins, power) {
            Burst::Now(gain) => gain,
            // The block began in a burst: what its frames left is dropped.
            Burst::Before => {
                self.forget();
                1.0
            }
            // With `pair` the mean of this frame still has the level before in it: the noise is found a frame later.
            Burst::Step => {
                let at = self.judged + u64::from(self.cfg.pair);
                self.floor_at = at;
                self.pair_floor_at = at;
                1.0
            }
            Burst::No => 1.0,
        };
        if !self.cfg.pair {
            for (t, &x) in self.total.iter_mut().zip(bins) {
                *t += power(x) * gain;
            }
            self.totalled += u32::from(self.finder.is_some());
            self.states(bins, gain, power);
            return Ok(());
        }
        if !self.has_last {
            self.has_last = true;
            for (l, &x) in self.last.iter_mut().zip(bins) {
                *l = power(x);
            }
            self.add_last();
            return Ok(());
        }
        self.pair_mean(bins, gain, power);
        self.add_last();
        let mean = std::mem::take(&mut self.mean);
        self.states(&mean, 1.0, |x| x);
        self.mean = mean;
        Ok(())
    }

    /// Adds the power of the frame kept in `last` to the sum of the occupancy.
    fn add_last(&mut self) {
        for (t, &l) in self.total.iter_mut().zip(&self.last) {
            *t += l;
        }
        self.totalled += u32::from(self.finder.is_some());
    }

    /// The occupied channels of the frames since the last reset, the strongest first, into `out` (cleared first): runs of
    /// bins over the noise from the mean power of each bin ([`OccupancyFinder::find`]). Nothing without `occupancy` in
    /// the settings.
    pub fn occupied(&mut self, out: &mut Vec<Occupied>) {
        match &mut self.finder {
            Some(f) => f.find(&self.total, self.totalled, out),
            None => out.clear(),
        }
    }

    /// Whether the frame `bins` (made power by `power`) is a burst over the whole block, from the median of some of its bins against the highest
    /// such level of the block so far. A rise by [`JUMP`] is a burst for at most [`MOST_HELD`] frames and a new level
    /// after them; a fall by it within the first frames of a block says that those were a burst. A level at the least
    /// value of the estimate is a block without noise, which gives nothing to compare with.
    fn burst(&mut self, bins: &[f32], power: impl Fn(f32) -> f32) -> Burst {
        let step = (bins.len() / LEVEL_BINS).max(1);
        let level = self
            .median
            .estimate(bins.iter().step_by(step).map(|&x| power(x)))
            .unwrap_or(self.median.min());
        let was = self.level;
        let none = self.median.min();
        let up = was > none && level > JUMP * was;
        if up && self.held < MOST_HELD {
            self.held += 1;
            return Burst::Now(was / level);
        }
        self.held = 0;
        if up {
            self.level = level;
            Burst::Step
        } else if level > none && level * JUMP < was && self.frames <= u64::from(MOST_HELD) + 1 {
            self.level = level;
            Burst::Before
        } else {
            self.level = was.max(level);
            Burst::No
        }
    }

    /// The mean of the power of `bins` times `gain` and the frame before into `mean`, with the strong bins under their edge put down to
    /// the noise. A frame with a gain is a burst brought down to the level before it: its bins are under their edges
    /// for that reason, and stay.
    fn pair_mean(&mut self, bins: &[f32], gain: f32, power: impl Fn(f32) -> f32) {
        let each = self
            .mean
            .iter_mut()
            .zip(&mut self.last)
            .zip(&mut self.peak)
            .zip(bins);
        for (((m, l), p), &now) in each {
            let now = power(now) * gain;
            *m = 0.5 * (now + *l);
            *p = p.max(*m);
            *l = now;
        }
        let segments = self
            .mean
            .chunks_mut(SEGMENT)
            .zip(self.peak.chunks(SEGMENT))
            .zip(&self.pair_floor);
        for ((ms, ps), &f) in segments {
            // Until the noise of the block is known no bin is strong.
            let strong = if f == f32::MAX || gain != 1.0 {
                f32::MAX
            } else {
                STRONG * f
            };
            for (m, &p) in ms.iter_mut().zip(ps) {
                if p > strong && *m < EDGE * p {
                    *m = m.min(f);
                }
            }
        }
        if self.judged >= self.pair_floor_at {
            self.pair_floor_at = self.judged + FLOOR_EVERY;
            for (f, chunk) in self.pair_floor.iter_mut().zip(self.mean.chunks(SEGMENT)) {
                *f = self
                    .pair_median
                    .estimate(chunk.iter().step_by(PAIR_FLOOR_STEP).copied())
                    .unwrap_or(self.pair_median.min());
            }
        }
    }

    /// The on and off states of the frame `bins` of `cfg.bins` bins, as power times `gain`, into the planes and the
    /// counts.
    fn states(&mut self, bins: &[f32], gain: f32, power: impl Fn(f32) -> f32 + Copy) {
        let n = self.cfg.bins;
        if self.judged >= self.floor_at {
            self.floor_at = self.judged + FLOOR_EVERY;
            self.find_floor(bins, gain, power);
        }
        let cap = self.cfg.frames;
        let words = self.words;
        let prev_at = (self.head + cap - 1) % cap;
        let has_prev = self.kept > 0;
        let (on_lin, hold_lin) = (power_of_db(self.cfg.on_db), power_of_db(self.cfg.hold_db));
        for w in 0..words {
            let prev = if has_prev {
                self.planes[prev_at * words + w]
            } else {
                0
            };
            let mut word = 0u64;
            let lo = w * 64;
            let hi = (lo + 64).min(n);
            for (j, b) in (lo..hi).enumerate() {
                let sum = power(bins[b]) * gain;
                let noise = self.floor[b / SEGMENT];
                let was = prev >> j & 1 == 1;
                if sum > noise * on_lin || (was && sum > noise * hold_lin) {
                    word |= 1 << j;
                    self.on[b] = self.on[b].saturating_add(1);
                    if !was {
                        self.marks[b] = self.marks[b].saturating_add(1);
                    }
                    self.excess[b] += sum / noise;
                }
            }
            self.planes[self.head * words + w] = word;
        }
        self.head = (self.head + 1) % cap;
        self.kept = (self.kept + 1).min(cap);
        self.judged += 1;
    }

    /// The mean noise per bin of each segment of `bins`, as power times `gain`, from the median of its bins.
    fn find_floor(&mut self, bins: &[f32], gain: f32, power: impl Fn(f32) -> f32) {
        for (s, chunk) in bins.chunks(SEGMENT).enumerate() {
            let mean = self
                .median
                .estimate(chunk.iter().map(|&x| power(x)))
                .unwrap_or(self.median.min());
            self.floor[s] = mean * gain;
        }
    }

    /// True if bin `b` is on in kept plane `p` (0 the oldest).
    fn bit(&self, p: usize, b: usize) -> bool {
        let cap = self.cfg.frames;
        let at = (self.head + cap - self.kept + p) % cap;
        self.planes[at * self.words + b / 64] >> (b % 64) & 1 == 1
    }

    /// The on bins from `lo` to `hi` (excluded) in kept plane `p`.
    fn count(&self, p: usize, lo: usize, hi: usize) -> u32 {
        let cap = self.cfg.frames;
        let at = (self.head + cap - self.kept + p) % cap;
        let plane = &self.planes[at * self.words..(at + 1) * self.words];
        let mut n = 0;
        let mut b = lo;
        while b < hi {
            let w = b / 64;
            let first = b % 64;
            let last = (hi - w * 64).min(64);
            let mask = if last - first == 64 {
                u64::MAX
            } else {
                ((1u64 << (last - first)) - 1) << first
            };
            n += (plane[w] & mask).count_ones();
            b = w * 64 + last;
        }
        n
    }

    /// The share of on bins in the ring on each side of `b`, the larger of the two, over the kept frames in which `b` is on
    /// and over those in which it is off; and the rise of the share in both rings together from the on to the off frames.
    fn ring_share(&self, b: usize) -> (f64, f64, f64) {
        let n = self.cfg.bins;
        let g = self.cfg.guard_bins;
        let r = ((self.cfg.ring_hz / self.cfg.bin_hz).ceil() as usize).max(g + 1);
        let sides = [
            (b.saturating_sub(r), b.saturating_sub(g)),
            ((b + g + 1).min(n), (b + r + 1).min(n)),
        ];
        let mut lit = [[0u64; 2]; 2];
        let mut frames = [0u64; 2];
        for p in 0..self.kept {
            let i = usize::from(self.bit(p, b));
            frames[i] += 1;
            for (s, &(lo, hi)) in sides.iter().enumerate() {
                lit[i][s] += u64::from(self.count(p, lo, hi));
            }
        }
        let share = |i: usize| {
            sides
                .iter()
                .enumerate()
                .map(|(s, &(lo, hi))| {
                    let cells = (hi - lo) as u64 * frames[i];
                    if cells == 0 {
                        0.0
                    } else {
                        lit[i][s] as f64 / cells as f64
                    }
                })
                .fold(0.0, f64::max)
        };
        let both = |i: usize| {
            let cells = sides.iter().map(|&(lo, hi)| (hi - lo) as u64).sum::<u64>() * frames[i];
            if cells == 0 {
                0.0
            } else {
                (lit[i][0] + lit[i][1]) as f64 / cells as f64
            }
        };
        (share(1), share(0), both(0) - both(1))
    }

    /// The share of the on frames of `b` in which `of` is on too.
    fn overlap(&self, b: usize, of: usize) -> f64 {
        let (mut both, mut on) = (0u32, 0u32);
        for p in 0..self.kept {
            if self.bit(p, b) {
                on += 1;
                both += u32::from(self.bit(p, of));
            }
        }
        if on == 0 {
            0.0
        } else {
            f64::from(both) / f64::from(on)
        }
    }

    /// The runs of bin `b` in the kept frames, in frames, without the first and the last run (they touch the ends of the
    /// kept frames and are cut), into `frame_runs`.
    fn frame_column(&mut self, b: usize) {
        let mut frames = std::mem::take(&mut self.frame_runs);
        frames.clear();
        self.col.clear();
        let mut state = self.kept > 0 && self.bit(0, b);
        let mut len = 0u32;
        for p in 0..self.kept {
            let on = self.bit(p, b);
            self.col.push(on);
            if on == state {
                len += 1;
            } else {
                frames.push((state, len));
                state = on;
                len = 1;
            }
        }
        if len > 0 {
            frames.push((state, len));
        }
        if !frames.is_empty() {
            frames.remove(0);
        }
        frames.pop();
        self.frame_runs = frames;
    }

    /// How far the states in `col` repeat at one period, at most 1: the mean of the correlation of the states with
    /// themselves at up to [`REPEAT_LAGS`] multiples of a period, each a frame more or less where the correlation is
    /// higher. The period is the shortest lag at which the correlation is within 0.8 of its highest, of the lags after
    /// the first without correlation (the states of long marks agree at short lags) up to a tenth of the column. Text
    /// repeats in parts only and is low over the multiples; 0 if no period is found.
    fn repeats(&self) -> f64 {
        let c = &self.col;
        let n = c.len();
        let on = c.iter().filter(|&&x| x).count() as f64 / n.max(1) as f64;
        let chance = on * on + (1.0 - on) * (1.0 - on);
        let last = n / 10;
        if chance >= 1.0 {
            return 0.0;
        }
        let at = |lag: usize| {
            let same = c[lag..].iter().zip(c).filter(|(a, b)| a == b).count();
            (same as f64 / (n - lag) as f64 - chance) / (1.0 - chance)
        };
        let Some(first) = (1..last).find(|&l| at(l) <= 0.0) else {
            return 0.0;
        };
        let best = (first + 1..=last).map(at).fold(0.0, f64::max);
        let Some(period) = (first + 1..=last).find(|&l| best > 0.0 && at(l) >= 0.8 * best) else {
            return 0.0;
        };
        let lags = REPEAT_LAGS.min(n / 2 / period);
        let (mut lag, mut sum) = (period, at(period));
        for _ in 1..lags {
            let near = lag + period - 1..=lag + period + 1;
            let (x, l) = near
                .map(|l| (at(l), l))
                .fold(
                    (f64::MIN, lag + period),
                    |a, b| if b.0 > a.0 { b } else { a },
                );
            lag = l;
            sum += x;
        }
        sum / lags as f64
    }

    /// True if the marks of `runs` are a pulse train: marks of one length (each within 40 % of their median) that start at
    /// regular times, at least `periodic_share` of the intervals between the starts within `max_periodic` of their median
    /// (a spike of noise in a gap spoils a few of them).
    fn periodic(&mut self) -> bool {
        let mut starts = std::mem::take(&mut self.starts);
        starts.clear();
        starts.extend(
            self.runs
                .iter()
                .filter(|r| r.key == Key::Down)
                .map(|r| r.seconds),
        );
        starts.sort_by(f64::total_cmp);
        let one_length = starts
            .get(starts.len() / 2)
            .is_some_and(|&m| starts.iter().all(|&s| (s - m).abs() <= 0.4 * m));
        starts.clear();
        if !one_length {
            self.starts = starts;
            return false;
        }
        let (mut at, mut last) = (0.0, None);
        for r in &self.runs {
            if r.key == Key::Down {
                if let Some(l) = last {
                    starts.push(at - l);
                }
                last = Some(at);
            }
            at += r.seconds;
        }
        let n = starts.len();
        let regular = n >= 3 && {
            starts.sort_by(f64::total_cmp);
            let m: f64 = starts[n / 2];
            let near = starts
                .iter()
                .filter(|d| (*d - m).abs() <= self.cfg.max_periodic * m)
                .count();
            near as f64 >= self.cfg.periodic_share * n as f64
        };
        self.starts = starts;
        regular
    }

    /// The runs of `frame_runs` from the first to the last whole mark, in seconds, into `runs`. Gaps of at most `short`
    /// frames between marks are dropouts and are closed; then marks of at most `short` frames, and of one frame (the
    /// frames resolve no dot that short), are spikes and go into the gaps around them.
    fn column(&mut self, short: u32) {
        let h = self.cfg.frame_s;
        let frames = std::mem::take(&mut self.frame_runs);
        let mut merged = std::mem::take(&mut self.merged);
        merged.clear();
        let push = |merged: &mut Vec<(bool, u32)>, on: bool, n: u32| match merged.last_mut() {
            Some((was, len)) if *was == on => *len += n,
            _ => merged.push((on, n)),
        };
        let last = frames.len();
        for (i, &(on, n)) in frames.iter().enumerate() {
            let inside = i > 0 && i + 1 < last;
            push(&mut merged, on || (inside && n <= short), n);
        }
        let mut spiked = std::mem::take(&mut self.spiked);
        spiked.clear();
        spiked.extend_from_slice(&merged);
        merged.clear();
        for &(on, n) in &spiked {
            push(&mut merged, on && n > short.max(1), n);
        }
        self.spiked = spiked;
        let start = merged.iter().position(|r| r.0).unwrap_or(merged.len());
        let end = merged.iter().rposition(|r| r.0).map_or(start, |e| e + 1);
        self.runs.clear();
        self.runs
            .extend(merged[start..end.max(start)].iter().map(|&(on, n)| Run {
                key: if on { Key::Down } else { Key::Up },
                seconds: f64::from(n) * h,
            }));
        self.merged = merged;
        self.frame_runs = frames;
    }

    /// The best fit of `runs` over the starting dot lengths and the parts of a frame by which marks read long: the dot
    /// length, s, the fit error and the bias in dots.
    fn fit(&mut self, dot_lo: f64, dot_hi: f64) -> Option<(f64, f64, f64)> {
        let (mut shortest, mut longest) = (f64::MAX, 0.0f64);
        for r in self.runs.iter().filter(|r| r.key == Key::Down) {
            shortest = shortest.min(r.seconds);
            longest = longest.max(r.seconds);
        }
        if longest == 0.0 {
            return None;
        }
        let h = self.cfg.frame_s;
        let mut best: Option<(f64, f64, f64)> = None;
        for part in BIAS_FRAMES {
            for t0 in [shortest, longest / 3.0] {
                let t0 = t0.clamp(dot_lo, dot_hi);
                let t = slowmorse::refine(&self.runs, Keying::Qrss, t0, part * h / t0);
                self.without_pauses(t);
                let bias = part * h / t;
                let t = slowmorse::refine(&self.used, Keying::Qrss, t, bias);
                let bias = part * h / t;
                let e = slowmorse::error(&self.used, Keying::Qrss, t, bias);
                if best.is_none_or(|(_, be, _)| e < be) {
                    best = Some((t, e, bias));
                }
            }
        }
        best
    }

    /// The runs without the pauses longer than [`LONG_GAP_DOTS`] dots of `t` seconds, into `used`.
    fn without_pauses(&mut self, t: f64) {
        self.used.clear();
        self.used.extend(
            self.runs
                .iter()
                .filter(|r| !(r.key == Key::Up && r.seconds > LONG_GAP_DOTS * t)),
        );
    }

    /// The fit of the runs of bin `b`: the dot length, s, the fit error and the whole marks. `None` when the runs are not
    /// Morse (module docs).
    fn rhythm(&mut self, b: usize) -> Option<(f64, f64, u32)> {
        self.frame_column(b);
        let h = self.cfg.frame_s;
        let dot_lo = (1.2 / self.cfg.max_wpm).max(self.cfg.min_dot_frames * h);
        let dot_hi = 1.2 / self.cfg.min_wpm;
        if dot_lo >= dot_hi {
            return None;
        }
        // A first fit on the runs as read gives the dot that the repair of dropouts and spikes is measured in.
        self.column(0);
        if self.periodic() {
            return None;
        }
        let (t, _, _) = self.fit(dot_lo, dot_hi)?;
        let frames = |t: f64| (REPAIR * t / h).floor() as u32;
        let mut short = frames(t);
        self.column(short);
        // Dropouts break long marks, and a fit follows the pieces: its error stays high, and a third of the longest
        // mark, a dash read up to two frames long, is well over its dot. That third then gives a longer repair, kept
        // when the dot fitted after it is long enough for it, the mark is still a dash, and the error is lower (a
        // repair that closes true gaps of fast keying makes the dot longer and the fit worse).
        let dash = self.frame_runs.iter().filter(|r| r.0).map(|r| r.1).max();
        let dash = f64::from(dash.unwrap_or(0)) * h;
        let third = (dash - 2.0 * h).max(0.0) / 3.0;
        let base = if third >= PIECES * t {
            self.fit(dot_lo, dot_hi).map_or(f64::MAX, |(_, e, _)| e)
        } else {
            0.0
        };
        if base > PIECES_ERROR * self.cfg.max_error {
            let fewer = short;
            for longer in (fewer + 1..=frames(third).min(frames(dot_hi))).rev() {
                self.column(longer);
                let fits =
                    |(t, e, _): (f64, f64, f64)| frames(t) >= longer && 2.5 * t <= dash && e < base;
                if self.fit(dot_lo, dot_hi).is_some_and(fits) {
                    short = longer;
                    break;
                }
            }
            if short == fewer {
                self.column(short);
            }
        }
        let marks = self.runs.iter().filter(|r| r.key == Key::Down).count() as u32;
        if marks < self.cfg.min_marks {
            return None;
        }
        let repeat = if marks >= REPEAT_MARKS {
            self.repeats()
        } else {
            0.0
        };
        if repeat > self.cfg.max_repeat {
            return None;
        }
        let best = self.fit(dot_lo, dot_hi);
        let (t, e, bias) = best?;
        if !(dot_lo * 0.9..=dot_hi * 1.1).contains(&t) {
            return None;
        }
        self.without_pauses(t);
        let el = slowmorse::elements(&self.used, Keying::Qrss, t, bias, false);
        // Dots and dashes; or, in a short look at a letter such as O or S, marks of one length with the gaps inside a
        // letter and between letters.
        let has = |x: Element| el.contains(&x);
        let inner = self
            .used
            .iter()
            .any(|r| r.key == Key::Up && (r.seconds + bias * t) < 2.0 * t);
        let letters = has(Element::LetterEnd) || has(Element::WordEnd);
        let gaps = self.used.iter().filter(|r| r.key == Key::Up);
        let (gaps, short) = gaps.fold((0u32, 0u32), |(n, short), r| {
            (n + 1, short + u32::from(r.seconds + bias * t < 2.0 * t))
        });
        let long_gaps =
            marks >= GAP_MARKS && f64::from(short) < self.cfg.min_inner * f64::from(gaps);
        if long_gaps && repeat > self.cfg.pulse_repeat {
            return None;
        }
        if !(has(Element::Dot) && has(Element::Dash) || inner && letters) {
            return None;
        }
        Some((t, e, marks))
    }

    /// The keyed carriers of the frames since the last reset, the best first, into `out` (cleared first).
    pub fn candidates(&mut self, out: &mut Vec<Candidate>) {
        out.clear();
        if self.judged == 0 {
            return;
        }
        let total = self.judged as f64;
        let cfg = self.cfg.clone();
        let duty = |on: u16| f64::from(on) / total;
        self.taken.fill(false);
        // The skirts of a steady carrier flicker with the noise.
        for b in 0..cfg.bins {
            if duty(self.on[b]) > cfg.max_duty {
                let lo = b.saturating_sub(cfg.guard_bins);
                let hi = (b + cfg.guard_bins + 1).min(cfg.bins);
                self.taken[lo..hi].fill(true);
            }
        }
        let mut picks = std::mem::take(&mut self.picks);
        picks.clear();
        for b in 0..cfg.bins {
            if !self.taken[b]
                && u32::from(self.marks[b]) >= cfg.min_marks
                && (cfg.min_duty..=cfg.max_duty).contains(&duty(self.on[b]))
            {
                picks.push(b);
            }
        }
        picks.sort_by(|&a, &b| self.excess[b].total_cmp(&self.excess[a]));
        let mut accepted: Vec<usize> = Vec::new();
        let mid = (cfg.bins / 2) as f64;
        for &b in &picks {
            if self.taken[b] {
                continue;
            }
            let lo = b.saturating_sub(cfg.merge_bins);
            let hi = (b + cfg.merge_bins + 1).min(cfg.bins);
            self.taken[lo..hi].fill(true);
            // The side of a stronger carrier keys with it.
            if accepted
                .iter()
                .any(|&a| a.abs_diff(b) <= cfg.guard_bins + 1 && self.overlap(b, a) > 0.8)
            {
                continue;
            }
            let (ring_on, ring_off, shift) = self.ring_share(b);
            let weak = self.excess[b] < STRONG * f32::from(self.on[b]);
            if ring_on > cfg.max_ring
                || ring_off - ring_on > cfg.max_ring_shift
                || weak && (ring_off - ring_on > cfg.weak_side_shift || shift > cfg.weak_ring_shift)
            {
                continue;
            }
            let Some((t, e, marks)) = self.rhythm(b) else {
                continue;
            };
            let more = 1.0 - cfg.few_runs as f64 / self.used.len().max(1) as f64;
            let limit = cfg.max_error + cfg.error_rise * more.max(0.0);
            if e > limit {
                continue;
            }
            accepted.push(b);
            // The centre of the carrier from the on counts of the bin and its neighbours.
            let w = |i: usize| f64::from(self.on[i]);
            let (l, r) = (b.saturating_sub(1), (b + 1).min(cfg.bins - 1));
            let sum = w(l) + w(b) + w(r);
            let centre = if sum > 0.0 {
                (l as f64 * w(l) + b as f64 * w(b) + r as f64 * w(r)) / sum
            } else {
                b as f64
            };
            let on = f64::from(self.on[b]).max(1.0);
            out.push(Candidate {
                offset_hz: (centre - mid) * cfg.bin_hz,
                bin: b,
                score: (1.0 - e).clamp(0.0, 1.0) as f32,
                wpm: (1.2 / t) as f32,
                snr_db: (10.0 * (f64::from(self.excess[b]) / on).log10()) as f32,
                marks,
                duty: duty(self.on[b]) as f32,
            });
        }
        self.picks = picks;
        out.sort_by(|a, b| b.score.total_cmp(&a.score));
        out.truncate(cfg.most);
    }
}

/// A [`KeyedSurvey`] fed with complex samples: back-to-back Blackman-Harris transforms of `bins` samples, so the bins are
/// `rate / bins` apart and the frames `bins / rate` apart.
pub struct SampleSurvey {
    fft: WindowedFft,
    buf: Vec<Complex32>,
    fill: usize,
    shifted: Vec<f32>,
    survey: KeyedSurvey,
}

impl std::fmt::Debug for SampleSurvey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SampleSurvey")
            .field("bins", &self.buf.len())
            .field("frames", &self.survey.frames())
            .finish()
    }
}

impl SampleSurvey {
    /// A survey of samples at `rate_hz` in frames of `bins` samples (a power of two), over a dwell of `dwell_s`. Errors as
    /// [`KeyedSurvey::new`], and for `bins` that is not a power of two or a rate that is not positive.
    pub fn new(rate_hz: f64, bins: usize, dwell_s: f64) -> Result<SampleSurvey, ModemError> {
        if !bins.is_power_of_two() {
            return Err(ModemError::NotAllowed {
                param: Param::FftSize,
                got: bins as f64,
            });
        }
        if !(rate_hz.is_finite() && rate_hz > 0.0) {
            return Err(ModemError::NotAllowed {
                param: Param::Rate,
                got: rate_hz,
            });
        }
        let cfg = SurveyConfig::new(bins, rate_hz / bins as f64, bins as f64 / rate_hz, dwell_s);
        Ok(SampleSurvey {
            fft: WindowedFft::new(bins, bins, Window::BlackmanHarris4).map_err(ModemError::Dsp)?,
            buf: vec![Complex32::default(); bins],
            fill: 0,
            shifted: vec![0.0; bins],
            survey: KeyedSurvey::new(cfg)?,
        })
    }

    /// Adds samples. Each whole frame becomes a spectrum of the survey.
    pub fn process(&mut self, mut x: &[Complex32]) {
        let n = self.buf.len();
        while !x.is_empty() {
            let take = (n - self.fill).min(x.len());
            self.buf[self.fill..self.fill + take].copy_from_slice(&x[..take]);
            self.fill += take;
            x = &x[take..];
            if self.fill == n {
                self.fill = 0;
                let p = self.fft.run_power(&self.buf);
                let half = n / 2;
                self.shifted[..half].copy_from_slice(&p[half..]);
                self.shifted[half..].copy_from_slice(&p[..half]);
                // The length matches by construction.
                let _ = self.survey.push_power(&self.shifted);
            }
        }
    }

    /// Forgets every sample and frame, for a new block.
    pub fn reset(&mut self) {
        self.fill = 0;
        self.survey.reset();
    }

    /// The survey.
    pub fn survey(&mut self) -> &mut KeyedSurvey {
        &mut self.survey
    }
}

/// The benchmarks of the survey: a block of 10 MHz in frames of 131 072 bins (76 Hz, 13.1 ms), from dB frames and from
/// samples.
pub fn benchmarks() -> Vec<sdrkit_core::Benchmark> {
    vec![
        sdrkit_core::Benchmark {
            name: "modem.keyed_survey_frames_10m",
            run: bench_frames,
        },
        sdrkit_core::Benchmark {
            name: "modem.keyed_survey_samples_10m",
            run: bench_samples,
        },
    ]
}

const BENCH_RATE: f64 = 10e6;
const BENCH_BINS: usize = 131_072;

/// Noise-like values from a fixed generator, cheap and repeatable.
fn bench_noise(n: usize, seed: u32) -> impl Iterator<Item = f32> {
    let mut s = seed | 1;
    (0..n).map(move |_| {
        s = s.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        (s >> 8) as f32 / (1u32 << 24) as f32 - 0.5
    })
}

fn bench_frames(seconds: f64) -> sdrkit_core::BenchResult {
    let frame_s = BENCH_BINS as f64 / BENCH_RATE;
    let cfg = SurveyConfig::new(BENCH_BINS, BENCH_RATE / BENCH_BINS as f64, frame_s, 2.0);
    let Ok(mut s) = KeyedSurvey::new(cfg) else {
        unreachable!("benchmark settings are valid")
    };
    let frames: Vec<Vec<f32>> = (0..4)
        .map(|k| {
            bench_noise(BENCH_BINS, k + 7)
                .map(|u| -100.0 - 10.0 * (0.5 - u).log10())
                .collect()
        })
        .collect();
    let mut k = 0;
    sdrkit_core::bench::measure(seconds, BENCH_BINS, frame_s, || {
        let _ = s.push_db(&frames[k % frames.len()]);
        k += 1;
    })
}

fn bench_samples(seconds: f64) -> sdrkit_core::BenchResult {
    let Ok(mut s) = SampleSurvey::new(BENCH_RATE, BENCH_BINS, 2.0) else {
        unreachable!("benchmark settings are valid")
    };
    let mut u = bench_noise(2 * BENCH_BINS, 3);
    let x: Vec<Complex32> = (0..BENCH_BINS)
        .map(|_| Complex32::new(u.next().unwrap_or(0.0), u.next().unwrap_or(0.0)))
        .collect();
    sdrkit_core::bench::measure(seconds, BENCH_BINS, BENCH_BINS as f64 / BENCH_RATE, || {
        s.process(&x);
    })
}
