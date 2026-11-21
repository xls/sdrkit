//! The line code of a pulse train that no protocol decoded, and the bits and timings of one frame of it.
//!
//! The guess helps a person add a new device. Lengths are in microseconds. The widths are the marks. The gaps are the spaces
//! between two marks; a gap of at least six times the median gap is a sync gap and is not data. The widths and the data gaps
//! are each sorted into clusters: a new cluster starts where a value is at least the split ratio (1.5 by default) times the
//! value before it, and a cluster is the mean of its values. Clusters with less than a tenth of the values are dropped.
//!
//! A train is noise, and has no line code, when the largest value of a cluster of widths or data gaps is more than 1.4 times
//! the smallest. Otherwise the first rule that holds names the code:
//!
//! | Rule | Code |
//! |---|---|
//! | two width clusters, long at least 1.8 times short, the periods (width plus data gap) within 1.25 times of each other | PWM: the widths are the short and long values |
//! | one width cluster, two gap clusters, long at least 1.8 times short | PPM: the gaps are the short and long values |
//! | at most two clusters each of widths and gaps, one of them with two; each second cluster 2 +- 0.35 times its first; the first width 1 +- 0.35 times the first gap | Manchester, half-bit `T` the mean of the first width and the first gap |
//! | the unit `T`, the 10th percentile of the widths and the data gaps, is at least 20 µs, and the widths and gaps are on average within 0.22 `T` of a whole number of `T` | NRZ-like, with unit `T` |
//! | none | unknown |
//!
//! [`Guesser::guess`] names the code of a whole train. [`Guesser::frame`] names the code of one frame by the same rules and
//! reads its bits: PWM one bit per pulse (a long width is 1; a pulse before a sync gap is skipped, and a last pulse that is
//! short and followed by a gap of at least 12 times its width is a sync pulse and is dropped), PPM one bit per data gap (a long
//! gap is 1), Manchester the half-bits of [`ManchesterHalves`] read in pairs (10 is 1, 01 is 0, to the first equal pair), and
//! NRZ-like `max(1, round(w / T))` ones per pulse and `max(1, round(g / T))` zeros per gap, to the first sync gap. A frame of
//! fewer than 6 bits has no code. [`frame_ranges`] cuts a train into frames at long gaps.
//!
//! ```
//! use sdrkit_proto_ookdev::{guess::{Guesser, LineCode}, Modulation, Train};
//! // A PWM remote: 464 µs and 1404 µs marks, complementary spaces.
//! let mut pairs = Vec::new();
//! for k in 0..24 {
//!     pairs.push(if k % 3 == 0 { (1404.0, 464.0) } else { (464.0, 1404.0) });
//! }
//! let g = Guesser::default().guess(&Train::from_us(Modulation::Ook, 1e6, &pairs));
//! assert_eq!(g.code, LineCode::Pwm);
//! assert!((g.short_us - 464.0).abs() < 1.0 && (g.long_us - 1404.0).abs() < 1.0);
//! ```

use crate::train::{quantile, Train};
use sdrkit_algo_modem::halfbit::{IntervalWindows, ManchesterHalves};
use serde::{Deserialize, Serialize};
use std::ops::Range;

/// The guessed line code.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LineCode {
    /// Pulse position.
    Ppm,
    /// Pulse width.
    Pwm,
    /// Manchester.
    Manchester,
    /// Non-return-to-zero (PCM): runs of a whole number of units.
    Nrz,
    /// No rule fits.
    Unknown,
}

impl LineCode {
    /// "PPM", "PWM", "Manchester", "NRZ" or "?".
    pub fn name(self) -> &'static str {
        match self {
            LineCode::Ppm => "PPM",
            LineCode::Pwm => "PWM",
            LineCode::Manchester => "Manchester",
            LineCode::Nrz => "NRZ",
            LineCode::Unknown => "?",
        }
    }
}

/// A line-code guess with the widths it found, in microseconds.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Guess {
    /// The line code.
    pub code: LineCode,
    /// Short width (the bit width for NRZ, the half-bit for Manchester), µs.
    pub short_us: f32,
    /// Long width, µs.
    pub long_us: f32,
    /// The number of bits the train holds by this code.
    pub bits: usize,
    /// The longest space left out as a sync gap, µs (0 when none).
    pub gap_us: f32,
}

/// The line code, the timings and the bit count of one frame ([`Guesser::frame`]). Times are in µs.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FrameGuess {
    /// The line code; never [`LineCode::Unknown`].
    pub code: LineCode,
    /// PWM: the short width. PPM: the short gap.
    pub short_us: Option<f32>,
    /// PWM: the long width. PPM: the long gap.
    pub long_us: Option<f32>,
    /// PPM: the pulse width.
    pub pulse_us: Option<f32>,
    /// PWM: the mean bit period (width plus data gap).
    pub period_us: Option<f32>,
    /// Manchester: the half-bit. NRZ-like: the unit.
    pub unit_us: Option<f32>,
    /// The bits read.
    pub bits: usize,
    /// The pulses of the frame.
    pub pulses: usize,
    /// From the start of the first pulse to the end of the last.
    pub frame_us: f32,
}

impl FrameGuess {
    /// The bit rate, bit/s: `1 / (2 T)` for Manchester, `1 / T` for NRZ-like, else `None`.
    pub fn bit_rate(&self) -> Option<f64> {
        let t = f64::from(self.unit_us?);
        match self.code {
            LineCode::Manchester => Some(1e6 / (2.0 * t)),
            LineCode::Nrz => Some(1e6 / t),
            _ => None,
        }
    }
}

/// The default split ratio of the clusters.
pub const CLUSTER_RATIO: f32 = 1.5;
/// A cluster whose largest value is more than this times its smallest makes the train noise.
const TIGHT: f32 = 1.4;
/// The periods of PWM are within this ratio.
const PERIOD_TIGHT: f32 = 1.25;
/// The least ratio of the long to the short value of PWM and PPM.
const LONG_SHORT: f32 = 1.8;
/// A gap of this many times the median gap is a sync gap.
const SYNC_GAPS: f32 = 6.0;
/// The fewest bits of a frame.
const MIN_BITS: usize = 6;

/// One cluster: the mean, the smallest and the largest value.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct Cluster {
    mean: f32,
    min: f32,
    max: f32,
}

/// The classification of the filled widths and data gaps.
#[derive(Clone, Copy, Debug)]
struct Class {
    code: LineCode,
    /// PWM: the widths; PPM: the gaps; Manchester: `T` and `2 T`; NRZ: `T` twice.
    short: f32,
    long: f32,
}

/// Makes guesses. It keeps its scratch buffers, so a guess allocates nothing once they have grown.
#[derive(Clone, Debug)]
pub struct Guesser {
    ratio: f32,
    marks: Vec<f32>,
    spaces: Vec<f32>,
    periods: Vec<f32>,
    tmp: Vec<f32>,
    mark_cl: Vec<Cluster>,
    space_cl: Vec<Cluster>,
    halves: Vec<u8>,
    gap_us: f32,
    sync_us: f32,
}

impl Default for Guesser {
    fn default() -> Self {
        Guesser::with_cluster_ratio(CLUSTER_RATIO)
    }
}

/// The clusters of `v` (sorted in place) with the split `ratio` (module docs); clusters with less than a tenth of the values
/// are dropped.
fn clusters(v: &mut [f32], ratio: f32, out: &mut Vec<Cluster>) {
    out.clear();
    v.sort_unstable_by(|a, b| a.total_cmp(b));
    let n = v.len();
    let flush = |sum: f64, c: &Cluster, count: usize, out: &mut Vec<Cluster>| {
        if count > 0 && count as f64 >= 0.1 * n as f64 {
            out.push(Cluster {
                mean: (sum / count as f64) as f32,
                ..*c
            });
        }
    };
    let (mut sum, mut count) = (0f64, 0usize);
    let mut c = Cluster::default();
    for &x in v.iter() {
        if count > 0 && x < ratio * c.max {
            sum += f64::from(x);
            count += 1;
            c.max = x;
        } else {
            flush(sum, &c, count, out);
            c = Cluster {
                mean: 0.0,
                min: x,
                max: x,
            };
            sum = f64::from(x);
            count = 1;
        }
    }
    flush(sum, &c, count, out);
}

impl Guesser {
    /// A guesser whose clusters split at `ratio` (1.2 to 2.5 are sensible; see the module docs).
    pub fn with_cluster_ratio(ratio: f32) -> Guesser {
        Guesser {
            ratio,
            marks: Vec::new(),
            spaces: Vec::new(),
            periods: Vec::new(),
            tmp: Vec::new(),
            mark_cl: Vec::new(),
            space_cl: Vec::new(),
            halves: Vec::new(),
            gap_us: 0.0,
            sync_us: 0.0,
        }
    }

    /// Sets the split ratio of the clusters.
    pub fn set_cluster_ratio(&mut self, ratio: f32) {
        self.ratio = ratio;
    }

    /// The split ratio of the clusters.
    pub fn cluster_ratio(&self) -> f32 {
        self.ratio
    }

    /// Fills the widths, the data gaps and the periods of `n` pulses whose width and gap after are `at(i)` (the gap after the
    /// last pulse is not read). Sets the sync limit and the longest sync gap.
    fn fill(&mut self, n: usize, at: impl Fn(usize) -> (f32, f32)) {
        self.marks.clear();
        self.spaces.clear();
        self.periods.clear();
        self.tmp.clear();
        self.tmp.extend((0..n.saturating_sub(1)).map(|i| at(i).1));
        self.sync_us = SYNC_GAPS * quantile(&mut self.tmp, 0.5);
        self.gap_us = 0.0;
        for i in 0..n {
            let (w, g) = at(i);
            self.marks.push(w);
            if i + 1 == n {
                break;
            }
            if self.is_sync(g) {
                self.gap_us = self.gap_us.max(g);
            } else {
                self.spaces.push(g);
                self.periods.push(w + g);
            }
        }
    }

    /// Whether the gap `g` of the filled pulses is a sync gap.
    fn is_sync(&self, g: f32) -> bool {
        self.sync_us > 0.0 && g >= self.sync_us
    }

    /// The line code of the filled widths and data gaps (module docs).
    fn classify(&mut self) -> Class {
        let unknown = Class {
            code: LineCode::Unknown,
            short: 0.0,
            long: 0.0,
        };
        self.tmp.clear();
        self.tmp.extend_from_slice(&self.marks);
        clusters(&mut self.tmp, self.ratio, &mut self.mark_cl);
        self.tmp.clear();
        self.tmp.extend_from_slice(&self.spaces);
        clusters(&mut self.tmp, self.ratio, &mut self.space_cl);
        let (pc, gc) = (&self.mark_cl, &self.space_cl);
        if pc.is_empty() || pc.iter().chain(gc.iter()).any(|c| c.max > TIGHT * c.min) {
            return unknown;
        }
        let class = |code, short, long| Class { code, short, long };
        let (pmin, pmax) = self
            .periods
            .iter()
            .fold((f32::INFINITY, 0f32), |(a, b), &p| (a.min(p), b.max(p)));
        if pc.len() == 2
            && pc[1].mean >= LONG_SHORT * pc[0].mean
            && !self.periods.is_empty()
            && pmax < PERIOD_TIGHT * pmin
        {
            return class(LineCode::Pwm, pc[0].mean, pc[1].mean);
        }
        if pc.len() == 1 && gc.len() == 2 && gc[1].mean >= LONG_SHORT * gc[0].mean {
            return class(LineCode::Ppm, gc[0].mean, gc[1].mean);
        }
        let double = |c: &[Cluster]| c.len() == 1 || (c[1].mean / c[0].mean - 2.0).abs() <= 0.35;
        if !gc.is_empty()
            && pc.len() <= 2
            && gc.len() <= 2
            && (pc.len() == 2 || gc.len() == 2)
            && double(pc)
            && double(gc)
            && (pc[0].mean / gc[0].mean - 1.0).abs() <= 0.35
        {
            let t = 0.5 * (pc[0].mean + gc[0].mean);
            return class(LineCode::Manchester, t, 2.0 * t);
        }
        self.tmp.clear();
        self.tmp.extend_from_slice(&self.marks);
        self.tmp.extend_from_slice(&self.spaces);
        let t = quantile(&mut self.tmp, 0.1);
        if t >= 20.0 {
            let off: f32 = self
                .tmp
                .iter()
                .map(|&x| (x / t - (x / t).round()).abs())
                .sum::<f32>()
                / self.tmp.len() as f32;
            if off <= 0.22 {
                return class(LineCode::Nrz, t, t);
            }
        }
        unknown
    }

    /// Guesses the line code of the whole of `train` (module docs).
    pub fn guess(&mut self, train: &Train) -> Guess {
        let k = if train.rate_hz > 0.0 {
            (1e6 / train.rate_hz) as f32
        } else {
            1.0
        };
        let p = &train.pulses;
        self.fill(p.len(), |i| (p[i].mark * k, p[i].space * k));
        let c = self.classify();
        let total: f32 = self.marks.iter().sum::<f32>() + self.spaces.iter().sum::<f32>();
        let bits = match c.code {
            LineCode::Ppm => self.spaces.len(),
            LineCode::Pwm => self.marks.len(),
            LineCode::Manchester => (total / c.long) as usize,
            LineCode::Nrz => (total / c.short) as usize,
            LineCode::Unknown => 0,
        };
        Guess {
            code: c.code,
            short_us: c.short,
            long_us: c.long,
            bits,
            gap_us: self.gap_us,
        }
    }

    /// The line code, the timings and the bits of one frame (module docs). `pulses` are (width, gap after) in µs; the gap
    /// after the last pulse is the gap that ended the frame (`f32::INFINITY` at the end of a transmission). `bits` is
    /// cleared and receives one value 0 or 1 per bit. `None` for noise, for no rule, and for fewer than 6 bits.
    pub fn frame(&mut self, pulses: &[(f32, f32)], bits: &mut Vec<u8>) -> Option<FrameGuess> {
        bits.clear();
        let n = pulses.len();
        if n < 2 {
            return None;
        }
        self.fill(n, |i| pulses[i]);
        let c = self.classify();
        let mut g = FrameGuess {
            code: c.code,
            short_us: None,
            long_us: None,
            pulse_us: None,
            period_us: None,
            unit_us: None,
            bits: 0,
            pulses: n,
            frame_us: pulses.iter().map(|p| p.0).sum::<f32>()
                + pulses[..n - 1].iter().map(|p| p.1).sum::<f32>(),
        };
        let last = n - 1;
        match c.code {
            LineCode::Pwm => {
                let mid = 0.5 * (c.short + c.long);
                for (i, &(w, gap)) in pulses.iter().enumerate() {
                    let sync_pulse = i == last && gap >= 12.0 * w && w < mid;
                    if (i < last && self.is_sync(gap)) || sync_pulse {
                        continue;
                    }
                    bits.push(u8::from(w > mid));
                }
                g.short_us = Some(c.short);
                g.long_us = Some(c.long);
                g.period_us = Some(self.periods.iter().sum::<f32>() / self.periods.len() as f32);
            }
            LineCode::Ppm => {
                let mid = 0.5 * (c.short + c.long);
                bits.extend(self.spaces.iter().map(|&gap| u8::from(gap > mid)));
                g.short_us = Some(c.short);
                g.long_us = Some(c.long);
                g.pulse_us = Some(self.marks.iter().sum::<f32>() / n as f32);
            }
            LineCode::Manchester => {
                self.halves.clear();
                let mut h = ManchesterHalves::new(IntervalWindows::manchester(f64::from(c.short)));
                for (i, &(w, gap)) in pulses.iter().enumerate() {
                    let Some(k) = h.edge(f64::from(w)) else { break };
                    self.halves.extend(std::iter::repeat_n(1, k));
                    if i == last || self.is_sync(gap) {
                        break;
                    }
                    let Some(k) = h.edge(f64::from(gap)) else {
                        break;
                    };
                    self.halves.extend(std::iter::repeat_n(0, k));
                }
                for pair in self.halves.as_chunks::<2>().0 {
                    match pair {
                        [1, 0] => bits.push(1),
                        [0, 1] => bits.push(0),
                        _ => break,
                    }
                }
                g.unit_us = Some(c.short);
            }
            LineCode::Nrz => {
                let t = c.short;
                let units = |x: f32| ((x / t).round() as usize).max(1);
                for (i, &(w, gap)) in pulses.iter().enumerate() {
                    bits.extend(std::iter::repeat_n(1, units(w)));
                    if i == last || self.is_sync(gap) {
                        break;
                    }
                    bits.extend(std::iter::repeat_n(0, units(gap)));
                }
                g.unit_us = Some(t);
            }
            LineCode::Unknown => return None,
        }
        g.bits = bits.len();
        (g.bits >= MIN_BITS).then_some(g)
    }
}

/// The frame timeout, µs, of a frame whose widths so far are `widths` (sorted in place): `max_gap_us` while fewer than 4
/// pulses are in, then `min(max_gap_us, max(1200, 12 x the median width))`.
fn timeout_us(widths: &mut [f32], max_gap_us: f32) -> f32 {
    if widths.len() < 4 {
        return max_gap_us;
    }
    max_gap_us.min((12.0 * quantile(widths, 0.5)).max(1200.0))
}

/// Cuts `train` into frames: a frame ends at a space longer than the frame timeout (see [`timeout_us`]: at most `max_gap_us`)
/// and at the end of the train. `out` receives the pulse ranges; `scratch` is reused.
pub fn frame_ranges(
    train: &Train,
    max_gap_us: f32,
    scratch: &mut Vec<f32>,
    out: &mut Vec<Range<usize>>,
) {
    out.clear();
    scratch.clear();
    let n = train.pulses.len();
    let mut start = 0;
    for (i, p) in train.pulses.iter().enumerate() {
        scratch.push(train.us(p.mark));
        let space = if i + 1 == n {
            f32::INFINITY
        } else {
            train.us(p.space)
        };
        if space > timeout_us(scratch, max_gap_us) {
            out.push(start..i + 1);
            start = i + 1;
            scratch.clear();
        }
    }
}

/// The first `max` bits of `bits` as `0` and `1`, with "..." when there are more.
pub fn bits_bin(bits: &[u8], max: usize) -> String {
    let mut s: String = bits
        .iter()
        .take(max)
        .map(|&b| if b & 1 == 1 { '1' } else { '0' })
        .collect();
    if bits.len() > max {
        s.push_str("...");
    }
    s
}

/// The first `max` bits of `bits` as upper-case hex, most significant first; the last digit is padded with zero bits.
pub fn bits_hex(bits: &[u8], max: usize) -> String {
    let mut s = String::new();
    sdrkit_util_bits::pack::bits_to_hex(&bits[..bits.len().min(max)], &mut s);
    s
}
