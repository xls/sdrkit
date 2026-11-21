//! Two-tone sequential pages (a tone A, then at once a tone B): the decoder and the generator.

use crate::analyse::{db_power, power_db};
use crate::gen::{Step, ToneSequence};
use crate::{Param, TonesError};
use num_complex::Complex32;
use sdrkit_algo_dsp::fft::Fft;
use sdrkit_algo_dsp::peak::log_parabolic;
use sdrkit_algo_dsp::window::Window;

/// The settings of [`TwoToneDecoder`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TwoToneConfig {
    /// The smallest tone level, dB relative to a full-scale sine.
    pub min_level_db: f32,
    /// The shortest tone A, s (0.1 or more).
    pub a_min_s: f64,
    /// The longest tone A, s (`a_min_s` up to 10).
    pub a_max_s: f64,
    /// The shortest tone B, s (0.1 or more).
    pub b_min_s: f64,
    /// The longest gap between A and B, s (0 to 2).
    pub max_gap_s: f64,
}

impl Default for TwoToneConfig {
    /// -45 dB, A of 0.25 to 2 s, B of 0.8 s or more, a gap of 0.3 s at most.
    fn default() -> Self {
        TwoToneConfig {
            min_level_db: -45.0,
            a_min_s: 0.25,
            a_max_s: 2.0,
            b_min_s: 0.8,
            max_gap_s: 0.3,
        }
    }
}

/// A two-tone page.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TwoTonePage {
    /// Tone A, Hz.
    pub a_hz: f64,
    /// Tone B, Hz.
    pub b_hz: f64,
    /// The input sample (counted from the decoder's start) at the start of tone A.
    pub start: u64,
    /// The length of tone A, s.
    pub a_s: f64,
    /// The length of tone B so far, s.
    pub b_s: f64,
    /// Tone B has ended (the second report of the page).
    pub ended: bool,
}

#[derive(Clone, Copy, Debug)]
struct Track {
    /// The mean frequency, Hz, and the number of measures in it.
    hz: f64,
    n: u32,
    start: u64,
    misses: u32,
}

/// The two-tone page decoder for real audio at 4000 Hz or more. Every half window (about 64 ms) it finds the strongest spectrum
/// line between 250 and 3000 Hz. The line is a pure tone when its three bins hold 60 % of the power above 70 Hz. A page is a
/// tone A of `a_min_s` to `a_max_s`, then within `max_gap_s` a tone B of `b_min_s` or more that differs from A by more than 4 %.
/// The page is reported when B reaches `b_min_s`, and again when B ends. The pages do not depend on how the input is split
/// across calls.
///
/// ```
/// use sdrkit_proto_tones::twotone::{two_tone_sequence, TwoToneConfig, TwoToneDecoder};
/// let mut gen = two_tone_sequence(8000.0, 682.5, 1000.0, 1.0, 3.0, 0.3).unwrap();
/// let mut audio = vec![0f32; gen.len() as usize + 4000];
/// gen.fill(&mut audio);
/// let mut d = TwoToneDecoder::new(8000.0, TwoToneConfig::default()).unwrap();
/// let mut pages = Vec::new();
/// d.process(&audio, |p| pages.push(p));
/// assert_eq!(pages.len(), 2);
/// assert!((pages[0].a_hz - 682.5).abs() < 1.5 && (pages[0].b_hz - 1000.0).abs() < 1.5);
/// assert!(!pages[0].ended && pages[1].ended && (pages[1].b_s - 3.0).abs() < 0.1);
/// ```
#[derive(Clone, Debug)]
pub struct TwoToneDecoder {
    cfg: TwoToneConfig,
    rate: f64,
    ring: Vec<f32>,
    head: usize,
    filled: usize,
    since: usize,
    hop: usize,
    pos: u64,
    window: Vec<f32>,
    wsum: f64,
    buf: Vec<Complex32>,
    pw: Vec<f64>,
    fft: Fft,
    cur: Option<Track>,
    cand: Option<(f64, u32, u64)>,
    last: Option<(f64, u64, u64)>,
    page: Option<TwoTonePage>,
    strongest: Option<(f64, f32)>,
}

impl TwoToneDecoder {
    /// A decoder for audio at `rate` Hz (4000 or more). Errors for a bad rate or setting.
    pub fn new(rate: f64, cfg: TwoToneConfig) -> Result<Self, TonesError> {
        let ok = rate.is_finite()
            && rate >= 4000.0
            && cfg.min_level_db.is_finite()
            && cfg.a_min_s >= 0.1
            && cfg.a_max_s >= cfg.a_min_s
            && cfg.a_max_s <= 10.0
            && cfg.b_min_s >= 0.1
            && (0.0..=2.0).contains(&cfg.max_gap_s);
        if !ok {
            return Err(TonesError::BadParameter(Param::TwoTone));
        }
        let n = ((0.064 * rate) as usize).next_power_of_two();
        let mut window = vec![0f32; n];
        Window::Hann.fill_periodic(&mut window);
        let wsum = window.iter().map(|&w| w as f64).sum();
        Ok(TwoToneDecoder {
            cfg,
            rate,
            ring: vec![0.0; n],
            head: 0,
            filled: 0,
            since: 0,
            hop: n / 2,
            pos: 0,
            window,
            wsum,
            buf: vec![Complex32::default(); n],
            pw: vec![0.0; n / 2 + 1],
            fft: Fft::forward(n),
            cur: None,
            cand: None,
            last: None,
            page: None,
            strongest: None,
        })
    }

    /// The strongest pure tone in the last analysis: its frequency (Hz) and level (dB relative to a full-scale sine).
    pub fn strongest(&self) -> Option<(f64, f32)> {
        self.strongest
    }

    /// Forgets everything.
    pub fn reset(&mut self) {
        self.ring.fill(0.0);
        self.head = 0;
        self.filled = 0;
        self.since = 0;
        self.cur = None;
        self.cand = None;
        self.last = None;
        self.page = None;
        self.strongest = None;
    }

    /// Takes the next audio samples and calls `on_page` for every page report.
    pub fn process(&mut self, x: &[f32], mut on_page: impl FnMut(TwoTonePage)) {
        for &v in x {
            self.ring[self.head] = if v.is_finite() { v } else { 0.0 };
            self.head = (self.head + 1) % self.ring.len();
            self.filled = (self.filled + 1).min(self.ring.len());
            self.pos += 1;
            self.since += 1;
            if self.since >= self.hop && self.filled == self.ring.len() {
                self.since = 0;
                self.strongest = self.analyse();
                if let Some(p) = self.track() {
                    on_page(p);
                }
            }
        }
    }

    fn analyse(&mut self) -> Option<(f64, f32)> {
        let n = self.ring.len();
        let mean = self.ring.iter().map(|&v| v as f64).sum::<f64>() / n as f64;
        for i in 0..n {
            let v = self.ring[(self.head + i) % n] - mean as f32;
            self.buf[i] = Complex32::new(v * self.window[i], 0.0);
        }
        self.fft.process(&mut self.buf);
        // A bin's value as the mean power of a sine on it.
        let scale = 2.0 / self.wsum;
        for (p, z) in self.pw.iter_mut().zip(&self.buf) {
            *p = (z.norm_sqr() as f64) * scale * scale / 2.0;
        }
        let bin = |hz: f64| (hz * n as f64 / self.rate).round() as usize;
        let lo = bin(250.0).max(2);
        let hi = bin(3000.0).min(self.pw.len() - 2);
        let k = (lo..=hi).max_by(|&a, &b| self.pw[a].total_cmp(&self.pw[b]))?;
        let above: f64 = self.pw[bin(70.0).max(1)..].iter().sum();
        let three = self.pw[k - 1] + self.pw[k] + self.pw[k + 1];
        // A Hann window spreads a tone over its three bins: their sum is about 1.5 times the tone's power.
        if above <= 0.0 || three < 0.6 * above || three / 1.5 < db_power(self.cfg.min_level_db) {
            return None;
        }
        let pk = log_parabolic(self.pw[k - 1], self.pw[k], self.pw[k + 1]);
        Some((
            (k as f64 + pk.offset) * self.rate / n as f64,
            power_db(pk.value),
        ))
    }

    fn near(a: f64, b: f64) -> bool {
        (a - b).abs() <= 0.015 * a + 6.0
    }

    fn track(&mut self) -> Option<TwoTonePage> {
        let obs = self.strongest.map(|s| s.0);
        let hop = self.hop as u64;
        let now = self.pos;
        let rate = self.rate;
        let secs = move |a: u64, b: u64| a.saturating_sub(b) as f64 / rate;
        let mut report = None;
        match self.cur {
            Some(mut t) if obs.is_some_and(|o| Self::near(t.hz, o)) => {
                let o = obs.unwrap_or(t.hz);
                t.n += 1;
                t.hz += (o - t.hz) / t.n.min(8) as f64;
                t.misses = 0;
                self.cur = Some(t);
                self.cand = None;
            }
            Some(mut t) => {
                t.misses += 1;
                self.cur = Some(t);
                self.note_candidate(obs, now);
                if t.misses >= 2 {
                    let end = now - 2 * hop;
                    if t.n >= 3 {
                        self.last = Some((t.hz, t.start, end));
                    }
                    self.cur = None;
                    if let Some(mut p) = self.page.take() {
                        p.ended = true;
                        p.b_s = secs(end, p.start) - p.a_s;
                        report = Some(p);
                    }
                    if let Some((c, cn, cs)) = self.cand.take().filter(|c| c.1 >= 2) {
                        self.cur = Some(Track {
                            hz: c,
                            n: cn,
                            start: cs,
                            misses: 0,
                        });
                    }
                }
            }
            None => {
                self.note_candidate(obs, now);
                if let Some((c, cn, cs)) = self.cand.filter(|c| c.1 >= 2) {
                    self.cur = Some(Track {
                        hz: c,
                        n: cn,
                        start: cs,
                        misses: 0,
                    });
                    self.cand = None;
                }
            }
        }
        if report.is_some() {
            return report;
        }
        if let (Some(b), Some((fa, astart, aend)), None) = (self.cur, self.last, self.page) {
            let b_s = secs(now, b.start);
            let a_s = secs(aend, astart);
            if b_s >= self.cfg.b_min_s
                && secs(b.start, aend) < self.cfg.max_gap_s
                && (self.cfg.a_min_s..=self.cfg.a_max_s).contains(&a_s)
                && (b.hz - fa).abs() > 0.04 * fa
            {
                let p = TwoTonePage {
                    a_hz: fa,
                    b_hz: b.hz,
                    start: astart,
                    a_s,
                    b_s,
                    ended: false,
                };
                self.page = Some(p);
                return Some(p);
            }
        }
        None
    }

    fn note_candidate(&mut self, obs: Option<f64>, now: u64) {
        let Some(o) = obs else {
            self.cand = None;
            return;
        };
        let start = now.saturating_sub(self.ring.len() as u64 / 2 + self.hop as u64 / 2);
        self.cand = match self.cand {
            Some((c, n, s)) if Self::near(c, o) => Some((c + (o - c) / (n + 1) as f64, n + 1, s)),
            _ => Some((o, 1, start)),
        };
    }
}

/// The audio of a two-tone page: tone A for `a_s`, then tone B for `b_s`, each at amplitude `amp`, the phase running on, with a
/// 5 ms ramp at the start and the end. Errors for a bad rate or tone.
pub fn two_tone_sequence(
    rate: f64,
    a_hz: f64,
    b_hz: f64,
    a_s: f64,
    b_s: f64,
    amp: f32,
) -> Result<ToneSequence, TonesError> {
    ToneSequence::new(
        rate,
        &[Step::one(a_hz, amp, a_s), Step::one(b_hz, amp, b_s)],
        0.005,
    )
}
