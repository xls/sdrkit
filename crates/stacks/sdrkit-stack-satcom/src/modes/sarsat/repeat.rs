//! Repeats of a first-generation beacon: a beacon sends the same message every 47.5 to 52.5 s on the same carrier. A decode
//! that repaired bits waits for a later burst to confirm it, and the soft bits of bursts that did not decode are summed with
//! those of its earlier bursts.
//!
//! Standard: C/S T.001 section 2.2 (the repetition period of 50 s +-5 %).

use super::coherent::{Decider, Decoded, Rule, BITS};
use super::t001::Fgb;

/// Bursts on carriers this close (Hz) belong to one beacon.
const SAME_CARRIER_HZ: f64 = 150.0;
/// The most decodes held for confirmation.
const MAX_HELD: usize = 16;
/// The most bursts kept for combining.
const MAX_KEPT: usize = 16;
/// The newest kept bursts a new burst is matched to.
const MATCH_NEWEST: usize = 3;
/// Repeat intervals a burst is matched across.
const MAX_REPEATS: usize = 4;
/// The tolerance of an interval, seconds.
const INTERVAL_SLACK_S: f64 = 0.5;
/// The least correlation of two timings that counts as the same message.
const MIN_CORRELATION: f32 = 0.3;
/// The correlation is taken over bits 25 to 112, which every message has.
const CORRELATED: std::ops::Range<usize> = 24..112;

/// The settings of [`Repeats`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RepeatConfig {
    /// Sum the soft bits of bursts that do not decode.
    pub combining: bool,
    /// The shortest repeat interval, seconds.
    pub min_s: f64,
    /// The longest repeat interval, seconds.
    pub max_s: f64,
}

impl Default for RepeatConfig {
    fn default() -> Self {
        RepeatConfig {
            combining: true,
            min_s: 47.5,
            max_s: 52.5,
        }
    }
}

impl RepeatConfig {
    /// How long a held decode, a kept burst and an expected carrier live: four of the longest intervals and 2 s.
    pub fn lifetime_s(&self) -> f64 {
        MAX_REPEATS as f64 * self.max_s + 2.0
    }
}

/// A message to report, with how it was reached.
#[derive(Clone, Debug, PartialEq)]
pub struct Report {
    /// The message.
    pub message: Fgb,
    /// Bursts whose soft bits were summed (1 for a burst decoded alone).
    pub combined: u32,
    /// A later burst decoded to the same identification.
    pub confirmed: bool,
    /// The repair margin.
    pub margin: f32,
    /// The repair cost.
    pub cost: f32,
}

/// What became of a burst.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Outcome {
    /// The message to report now.
    pub report: Option<Report>,
    /// The burst was decoded, held or kept: its carrier is expected again.
    pub expect: bool,
}

struct Held {
    at: f64,
    carrier: f64,
    decoded: Decoded,
}

struct Kept {
    at: f64,
    carrier: f64,
    timings: Vec<Vec<f32>>,
}

/// The repeat state of a receiver: decodes held for confirmation and bursts kept for combining.
#[derive(Default)]
pub struct Repeats {
    cfg: RepeatConfig,
    held: Vec<Held>,
    kept: Vec<Kept>,
    decider: Decider,
    sum: Vec<f32>,
}

/// The normalised dot product of two soft-bit vectors over bits 25 to 112.
fn correlation(a: &[f32], b: &[f32]) -> f32 {
    let (mut ab, mut aa, mut bb) = (0f64, 0f64, 0f64);
    for (x, y) in a[CORRELATED].iter().zip(&b[CORRELATED]) {
        ab += f64::from(*x) * f64::from(*y);
        aa += f64::from(*x) * f64::from(*x);
        bb += f64::from(*y) * f64::from(*y);
    }
    if aa <= 0.0 || bb <= 0.0 {
        return 0.0;
    }
    (ab / (aa * bb).sqrt()) as f32
}

impl Repeats {
    /// An empty state with these settings.
    pub fn new(cfg: RepeatConfig) -> Self {
        Repeats {
            cfg,
            ..Default::default()
        }
    }

    /// The settings.
    pub fn config(&self) -> RepeatConfig {
        self.cfg
    }

    /// Changes the settings; what is held stays.
    pub fn set_config(&mut self, cfg: RepeatConfig) {
        self.cfg = cfg;
    }

    /// Forgets everything held.
    pub fn clear(&mut self) {
        self.held.clear();
        self.kept.clear();
    }

    /// Takes a burst heard at `at` s on `carrier` Hz: its decode, and the soft bits of its best timings when no sure decode came.
    ///
    /// A sure decode is reported and removes the decodes held on its carrier. Another decode is reported when a decode held on
    /// a carrier within 150 Hz has the same identification ("confirmed by repeat"), otherwise held. A burst without a sure
    /// decode is matched with the newest three kept bursts on its carrier whose distance is a whole number (1 to 4) of repeat
    /// intervals: its timing with the largest summed correlation is taken, each kept burst joins with its best timing when that
    /// correlates by 0.3 or more, and the sum is decided after each burst joins. A burst that is neither decoded nor combined
    /// keeps its soft bits. A reported decode removes the bursts kept on its carrier.
    pub fn burst(
        &mut self,
        at: f64,
        carrier: f64,
        decoded: Option<Decoded>,
        soft: Vec<Vec<f32>>,
    ) -> Outcome {
        let life = self.cfg.lifetime_s();
        self.held.retain(|h| at - h.at <= life);
        self.kept.retain(|k| at - k.at <= life);
        let near = |c: f64| (c - carrier).abs() <= SAME_CARRIER_HZ;
        if let Some(d) = decoded {
            if d.sure {
                self.held.retain(|h| !near(h.carrier));
                self.kept.retain(|k| !near(k.carrier));
                return Outcome {
                    report: Some(Self::report(d, 1, false)),
                    expect: true,
                };
            }
            if let Some(i) = self
                .held
                .iter()
                .position(|h| near(h.carrier) && h.decoded.message.hex_id == d.message.hex_id)
            {
                self.held.remove(i);
                self.kept.retain(|k| !near(k.carrier));
                return Outcome {
                    report: Some(Self::report(d, 1, true)),
                    expect: true,
                };
            }
            if self.cfg.combining && !soft.is_empty() {
                if let Some(r) = self.combine(at, carrier, &soft) {
                    return Outcome {
                        report: Some(r),
                        expect: true,
                    };
                }
            }
            if self.held.len() >= MAX_HELD {
                self.held.remove(0);
            }
            self.held.push(Held {
                at,
                carrier,
                decoded: d,
            });
        } else if !self.cfg.combining || soft.is_empty() {
            return Outcome::default();
        } else if let Some(r) = self.combine(at, carrier, &soft) {
            return Outcome {
                report: Some(r),
                expect: true,
            };
        }
        if !self.cfg.combining || soft.is_empty() {
            return Outcome {
                report: None,
                expect: true,
            };
        }
        if self.kept.len() >= MAX_KEPT {
            self.kept.remove(0);
        }
        self.kept.push(Kept {
            at,
            carrier,
            timings: soft,
        });
        Outcome {
            report: None,
            expect: true,
        }
    }

    /// Sums the soft bits of a burst that did not decode with those of its earlier bursts (see [`Repeats::burst`]).
    fn combine(&mut self, at: f64, carrier: f64, soft: &[Vec<f32>]) -> Option<Report> {
        let (lo, hi) = (self.cfg.min_s, self.cfg.max_s);
        let repeat_of = |dt: f64| {
            (1..=MAX_REPEATS).any(|m| {
                let m = m as f64;
                dt >= m * lo - INTERVAL_SLACK_S && dt <= m * hi + INTERVAL_SLACK_S
            })
        };
        let mut matched: Vec<usize> = (0..self.kept.len())
            .rev()
            .filter(|&i| {
                let k = &self.kept[i];
                (k.carrier - carrier).abs() <= SAME_CARRIER_HZ && repeat_of(at - k.at)
            })
            .take(MATCH_NEWEST)
            .collect();
        if matched.is_empty() {
            return None;
        }
        // The best correlation of a timing with a kept burst, and the timing of the kept burst that gives it.
        let best_with = |s: &[f32], k: &Kept| -> (f32, usize) {
            k.timings
                .iter()
                .enumerate()
                .map(|(j, t)| (correlation(s, t), j))
                .fold((f32::MIN, 0), |a, b| if b.0 > a.0 { b } else { a })
        };
        let (mut pick, mut total) = (0usize, 0f32);
        for (i, s) in soft.iter().enumerate() {
            let sum: f32 = matched
                .iter()
                .map(|&m| best_with(s, &self.kept[m]).0)
                .filter(|&c| c >= MIN_CORRELATION)
                .sum();
            if sum > total {
                (pick, total) = (i, sum);
            }
        }
        if total <= 0.0 {
            return None;
        }
        let new = &soft[pick];
        let mut joins: Vec<(f32, usize, usize)> = matched
            .iter()
            .map(|&m| {
                let (c, j) = best_with(new, &self.kept[m]);
                (c, m, j)
            })
            .filter(|j| j.0 >= MIN_CORRELATION)
            .collect();
        joins.sort_by(|a, b| b.0.total_cmp(&a.0));
        self.sum.clear();
        self.sum.extend_from_slice(new);
        self.sum.resize(BITS, 0.0);
        matched.clear();
        for (_, m, j) in joins {
            for (s, v) in self.sum.iter_mut().zip(&self.kept[m].timings[j]) {
                *s += v;
            }
            matched.push(m);
            if let Some(d) = self.decider.decide(&self.sum, Rule::Combined) {
                let n = matched.len() as u32 + 1;
                let mut i = 0;
                self.kept.retain(|k| {
                    let drop =
                        matched.contains(&i) || (k.carrier - carrier).abs() <= SAME_CARRIER_HZ;
                    i += 1;
                    !drop
                });
                return Some(Self::report(d, n, false));
            }
        }
        None
    }

    fn report(d: Decoded, combined: u32, confirmed: bool) -> Report {
        Report {
            message: d.message,
            combined,
            confirmed,
            margin: d.margin,
            cost: d.cost,
        }
    }
}
