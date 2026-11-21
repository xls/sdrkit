//! Sequential tone calls (selcall: one tone per digit): the decoder and the generator. The tone sets are in
//! `sdrkit_util_ids::tones::selcall`.

use crate::analyse::{db_power, Analyser};
use crate::gen::{Step, ToneSequence};
use crate::{Param, TonesError};
use sdrkit_algo_dsp::window::Window;
use sdrkit_util_ids::tones::selcall::{self, ToneSet, TONE_SETS};

/// The most segments a call can hold.
pub const MAX_TONES: usize = 16;

/// The settings of [`SelcallDecoder`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SelcallConfig {
    /// The sets to read calls as: bit `i` is `TONE_SETS[i]` (all sets by default).
    pub sets: u32,
    /// The smallest share of the power for the strongest tone (0 to 1).
    pub min_share: f64,
    /// The smallest tone level, dB relative to a full-scale sine.
    pub min_level_db: f32,
    /// The longest segment, s (0.05 to 1).
    pub max_tone_s: f64,
    /// The silence that ends a call, s (0.05 to 2).
    pub end_s: f64,
    /// The fewest segments in a call (1 to 16).
    pub min_tones: usize,
}

impl Default for SelcallConfig {
    /// All sets, share 0.55, level -45 dB, segments up to 0.32 s, 0.16 s of silence to end a call, three tones or more.
    fn default() -> Self {
        SelcallConfig {
            sets: (1 << TONE_SETS.len()) - 1,
            min_share: 0.55,
            min_level_db: -45.0,
            max_tone_s: 0.32,
            end_s: 0.16,
            min_tones: 3,
        }
    }
}

impl SelcallConfig {
    /// Only the set called `name` (as `sdrkit_util_ids::tones::selcall::by_name` reads it).
    pub fn only(name: &str) -> Option<Self> {
        let set = selcall::by_name(name)?;
        let i = TONE_SETS.iter().position(|s| std::ptr::eq(s, set))?;
        Some(SelcallConfig {
            sets: 1 << i,
            ..Self::default()
        })
    }
}

/// A call read from the segments.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SelcallCall {
    digits: [u8; MAX_TONES],
    sent: [u8; MAX_TONES],
    len: usize,
    /// The set the call was read as.
    pub set: &'static ToneSet,
    /// The other sets the tones also fit: bit `i` is `TONE_SETS[i]`.
    pub also_fits: u32,
    /// The input sample (counted from the decoder's start) at the start of the first tone.
    pub start: u64,
    /// The length of the call, s.
    pub length_s: f64,
    /// The median tone length, s.
    pub tone_s: f64,
}

impl SelcallCall {
    /// The digits, with the repeat tone read as the digit before it ("55123").
    pub fn digits(&self) -> &str {
        std::str::from_utf8(&self.digits[..self.len]).unwrap_or("")
    }

    /// The digits as sent, with `E` for the repeat tone ("5E123").
    pub fn sent(&self) -> &str {
        std::str::from_utf8(&self.sent[..self.len]).unwrap_or("")
    }
}

#[derive(Clone, Copy, Debug, Default)]
struct Segment {
    tone: usize,
    hops: u32,
    start: u64,
}

/// The selcall decoder for real audio at a rate that holds every tone of the chosen sets. Every 8 ms it measures the tones over
/// the last 16 ms. The strongest tone counts when its share is at least `min_share` and its level at least `min_level_db`. A
/// tone of 16 ms to `max_tone_s` is a segment. After `end_s` of silence, `min_tones` segments or more are a call. Of the sets
/// that fit, the call takes the one whose tone length is nearest the median segment length. The repeat tone stands for the
/// digit before it. The decoded calls do not depend on how the input is split across `process` calls.
///
/// ```
/// use sdrkit_util_ids::tones::selcall;
/// use sdrkit_proto_tones::selcall::{selcall_sequence, SelcallConfig, SelcallDecoder};
/// let ccir = selcall::by_name("CCIR1").unwrap();
/// let mut gen = selcall_sequence(8000.0, ccir, "55123", 0.3, None).unwrap();
/// let mut audio = vec![0f32; gen.len() as usize + 4000];
/// gen.fill(&mut audio);
/// let mut d = SelcallDecoder::new(8000.0, SelcallConfig::default()).unwrap();
/// let mut calls = Vec::new();
/// d.process(&audio, |c| calls.push((c.digits().to_string(), c.set.name)));
/// assert_eq!(calls, [("55123".to_string(), "CCIR1")]);
/// ```
#[derive(Clone, Debug)]
pub struct SelcallDecoder {
    cfg: SelcallConfig,
    an: Analyser,
    /// The distinct tones of the chosen sets, Hz, ascending.
    pool: Vec<f32>,
    cur: Option<usize>,
    count: u32,
    cur_start: u64,
    cand: Option<Option<usize>>,
    cand_n: u32,
    segs: [Segment; MAX_TONES],
    nsegs: usize,
    last_end: Option<u64>,
    bad: u64,
    rate: f64,
}

impl SelcallDecoder {
    /// A decoder for audio at `rate` Hz. Errors for a bad setting, no set chosen, or a tone above 0.45 of the rate.
    pub fn new(rate: f64, cfg: SelcallConfig) -> Result<Self, TonesError> {
        let ok = rate.is_finite()
            && rate > 0.0
            && cfg.sets & ((1 << TONE_SETS.len()) - 1) != 0
            && (0.0..=1.0).contains(&cfg.min_share)
            && cfg.min_level_db.is_finite()
            && (0.05..=1.0).contains(&cfg.max_tone_s)
            && (0.05..=2.0).contains(&cfg.end_s)
            && (1..=MAX_TONES).contains(&cfg.min_tones);
        if !ok {
            return Err(TonesError::BadParameter(Param::Selcall));
        }
        let mut pool: Vec<f32> = Vec::with_capacity(16 * TONE_SETS.len());
        for (i, s) in TONE_SETS.iter().enumerate() {
            if cfg.sets & (1 << i) != 0 {
                pool.extend(
                    s.digits
                        .iter()
                        .copied()
                        .chain([s.repeat_hz])
                        .chain(s.group_hz),
                );
            }
        }
        pool.sort_by(f32::total_cmp);
        pool.dedup_by(|a, b| (*a - *b).abs() < 0.01);
        if pool.iter().any(|&f| f as f64 >= 0.45 * rate) {
            return Err(TonesError::BadParameter(Param::SelcallRate));
        }
        let freqs: Vec<f64> = pool.iter().map(|&f| f as f64 / rate).collect();
        let len = ((0.016 * rate) as usize / 2 * 2).max(8);
        let an = Analyser::new(&freqs, len, len / 2, Window::Hann, true)?;
        Ok(SelcallDecoder {
            cfg,
            an,
            pool,
            cur: None,
            count: 0,
            cur_start: 0,
            cand: None,
            cand_n: 0,
            segs: [Segment::default(); MAX_TONES],
            nsegs: 0,
            last_end: None,
            bad: 0,
            rate,
        })
    }

    /// Groups of tones that fit no chosen set.
    pub fn bad(&self) -> u64 {
        self.bad
    }

    /// Forgets everything.
    pub fn reset(&mut self) {
        self.an.reset();
        self.cur = None;
        self.count = 0;
        self.cand = None;
        self.cand_n = 0;
        self.nsegs = 0;
        self.last_end = None;
    }

    /// Takes the next audio samples and calls `on_call` for every call.
    pub fn process(&mut self, x: &[f32], mut on_call: impl FnMut(SelcallCall)) {
        for &v in x {
            if self.an.push(v) {
                if let Some(c) = self.step() {
                    on_call(c);
                }
            }
        }
    }

    fn observe(&self) -> Option<usize> {
        let hop = self.an.last();
        let best = (0..hop.power.len()).max_by(|&a, &b| hop.power[a].total_cmp(&hop.power[b]))?;
        (hop.share(best) >= self.cfg.min_share
            && hop.power[best] >= db_power(self.cfg.min_level_db))
        .then_some(best)
    }

    fn step(&mut self) -> Option<SelcallCall> {
        let obs = self.observe();
        let hop = self.an.hop() as u64;
        let now = self.an.position();
        let max_hops = (self.cfg.max_tone_s * self.rate / hop as f64).round() as u32;
        if obs == self.cur {
            self.count += 1;
            self.cand = None;
            if self.cur.is_some() && self.count > max_hops {
                self.nsegs = 0;
            }
        } else {
            if self.cand == Some(obs) {
                self.cand_n += 1;
            } else {
                self.cand = Some(obs);
                self.cand_n = 1;
            }
            if self.cand_n >= 2 {
                if let Some(t) = self.cur {
                    if (2..=max_hops).contains(&self.count) {
                        if self.nsegs == MAX_TONES {
                            self.nsegs = 0;
                        }
                        self.segs[self.nsegs] = Segment {
                            tone: t,
                            hops: self.count,
                            start: self.cur_start,
                        };
                        self.nsegs += 1;
                        self.last_end = Some(now - 2 * hop);
                    } else if self.count > max_hops {
                        self.nsegs = 0;
                    }
                }
                self.cur = obs;
                self.count = self.cand_n;
                self.cur_start = now.saturating_sub(self.an.len() as u64 + hop);
                self.cand = None;
                self.cand_n = 0;
            }
        }
        if self.cur.is_some() || self.nsegs == 0 {
            return None;
        }
        let end = self.last_end?;
        if ((now - end) as f64 / self.rate) <= self.cfg.end_s {
            return None;
        }
        let n = self.nsegs;
        self.nsegs = 0;
        if n < self.cfg.min_tones {
            return None;
        }
        let call = self.read(n);
        if call.is_none() {
            self.bad += 1;
        }
        call
    }

    /// Reads the first `n` segments as a call.
    fn read(&self, n: usize) -> Option<SelcallCall> {
        let segs = &self.segs[..n];
        let hop_s = self.an.hop() as f64 / self.rate;
        // The tone length of a segment: its hops plus one hop (the window is two hops long).
        let len_s = |s: &Segment| (s.hops as f64 + 1.0) * hop_s;
        let mut lens = [0f64; MAX_TONES];
        for (l, s) in lens.iter_mut().zip(segs) {
            *l = len_s(s);
        }
        let lens = &mut lens[..n];
        lens.sort_by(f64::total_cmp);
        let median = lens[n / 2];
        let mut best: Option<SelcallCall> = None;
        let mut fits = 0u32;
        for (i, set) in TONE_SETS.iter().enumerate() {
            if self.cfg.sets & (1 << i) == 0 {
                continue;
            }
            let Some((digits, sent)) = self.fit(set, segs) else {
                continue;
            };
            fits |= 1 << i;
            let better = best.is_none_or(|b| {
                (set.tone_ms as f64 / 1000.0 - median).abs()
                    < (b.set.tone_ms as f64 / 1000.0 - median).abs()
            });
            if better {
                best = Some(SelcallCall {
                    digits,
                    sent,
                    len: n,
                    set,
                    also_fits: 0,
                    start: segs[0].start,
                    length_s: segs.iter().map(len_s).sum(),
                    tone_s: median,
                });
            }
        }
        let mut c = best?;
        let me = TONE_SETS
            .iter()
            .position(|s| std::ptr::eq(s, c.set))
            .unwrap_or(0);
        c.also_fits = fits & !(1 << me);
        Some(c)
    }

    /// The digits of the segments in `set`, when every segment is one of its tones.
    fn fit(&self, set: &ToneSet, segs: &[Segment]) -> Option<([u8; MAX_TONES], [u8; MAX_TONES])> {
        let mut digits = [0u8; MAX_TONES];
        let mut sent = [0u8; MAX_TONES];
        for (j, s) in segs.iter().enumerate() {
            let f = self.pool[s.tone];
            let d = set.digit(f, 0.01)?;
            sent[j] = d as u8;
            digits[j] = if d == 'E' {
                *digits[..j].last()?
            } else {
                d as u8
            };
        }
        Some((digits, sent))
    }
}

/// The audio of a call of `digits` (`0` to `9`, and `A` for the group tone) in `set`, each tone at amplitude `amp` for the
/// set's usual time (or `tone_s`), with the repeat tone in place of a digit equal to the one before. A 1 ms ramp shapes the
/// start and the end. Errors for another character, a missing group tone, or a call of more than 16 tones.
pub fn selcall_sequence(
    rate: f64,
    set: &ToneSet,
    digits: &str,
    amp: f32,
    tone_s: Option<f64>,
) -> Result<ToneSequence, TonesError> {
    let t = tone_s.unwrap_or(set.tone_ms as f64 / 1000.0);
    let mut steps = [Step::silence(0.0); MAX_TONES];
    let mut n = 0;
    let mut prev: Option<char> = None;
    for d in digits.chars() {
        let d = d.to_ascii_uppercase();
        let hz = if prev == Some(d) {
            prev = None;
            Some(set.repeat_hz)
        } else {
            prev = Some(d);
            match d {
                '0'..='9' | 'A' => set.tone(d),
                _ => None,
            }
        };
        let hz = hz.ok_or(TonesError::NoTone {
            digit: d,
            set: set.name,
        })?;
        *steps
            .get_mut(n)
            .ok_or(TonesError::TooManyTones { max: MAX_TONES })? = Step::one(hz as f64, amp, t);
        n += 1;
    }
    ToneSequence::new(rate, &steps[..n], 0.001)
}
