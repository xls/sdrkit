//! The identifier: it scores the signatures against a measurement, verifies the best candidates with detectors, turns the scores
//! into calibrated probabilities and returns a verdict in plain words.

use crate::features::{fmt_width, measure, Channel, Features, Why};
use crate::psd::Psd;
use crate::regions::{
    click_region, find_regions, Region, CLICK_WIDTH_HZ, MAX_REGIONS, MIN_OVER_FLOOR_DB,
};
use crate::signature::{Keying, ModClass, Signature};
use crate::table::builtin;
use crate::verify::{builtin_detectors, Check, Detector, Verification};
use sdrkit_core::Complex32;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// A hint from a band plan or the frequency database: signals in this range are often of this signal type. It moves a score a
/// little, and only for a candidate that already fits the measurement.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Prior {
    /// The low edge (Hz).
    pub lo_hz: f64,
    /// The high edge (Hz).
    pub hi_hz: f64,
    /// The key of the signal type that it suggests.
    pub signal: String,
    /// Points, held to the range -8 to +8.
    pub points: f64,
    /// Where it came from ("band plan", "frequency database").
    pub source: String,
}

/// One signal type that the signal can be.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Candidate {
    /// The signature's name.
    pub name: String,
    /// The key of the signal type.
    pub signal: String,
    /// The group.
    pub family: String,
    /// The score, 0 to 100, after verification and priors.
    pub score: f64,
    /// The same before it was clamped to 100 (it ranks candidates that both reach 100).
    pub raw_score: f64,
    /// The calibrated probability that this, and only this, is right.
    pub probability: f64,
    /// One line per rule ("symbol rate 4800 Bd within 0.4 % (+25)").
    pub evidence: Vec<String>,
    /// The priors applied.
    pub priors: Vec<String>,
    /// What its own detectors found, when it was verified.
    pub verification: Option<Verification>,
    /// The measurement fits it (no rule says it cannot be).
    pub fits: bool,
    /// Where to tune to open it, Hz: the measured centre, rounded to 100 Hz.
    pub freq_hz: f64,
    /// The measured 99 % width, Hz.
    pub width_hz: f64,
}

sdrkit_core::choice_enum! {
    /// The verdict.
    #[derive(Serialize, Deserialize)]
    #[serde(rename_all = "snake_case")]
    pub enum Verdict {
        /// "I do not know."
        Unknown = ("unknown", "Unknown"),
        /// Several about the same.
        Ambiguous = ("ambiguous", "Ambiguous"),
        /// One stands out.
        Likely = ("likely", "Likely"),
    }
}

sdrkit_core::choice_enum! {
    /// How sure a proposal is, in words for people new to the radio.
    #[derive(PartialOrd, Ord, Serialize, Deserialize)]
    #[serde(rename_all = "snake_case")]
    pub enum Sureness {
        /// No proposal.
        None = ("none", "No proposal"),
        /// A guess.
        Guess = ("guess", "Guess"),
        /// Likely.
        Likely = ("likely", "Likely"),
        /// Its own detector found it.
        Sure = ("sure", "Sure"),
    }
}

/// Which signals of a capture to identify.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Look {
    /// The strongest signal.
    Strongest,
    /// The signal that a click at this offset (Hz from the centre) lands on.
    Click(f64),
    /// The signal in a region (a drag).
    Region(Region),
    /// Every signal, strongest first.
    All,
}

/// Everything said about one signal.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Identification {
    /// The measurements.
    pub features: Features,
    /// The candidates, best first.
    pub candidates: Vec<Candidate>,
    /// The verdict.
    pub verdict: Verdict,
    /// How sure.
    pub sureness: Sureness,
    /// The beginner line: "Sure: DMR", "Probably: NXDN96", "Could be: P25 Phase 1 or NXDN96", "Unknown signal, 12.5 kHz wide".
    pub line: String,
    /// The measurement in one line: "4FSK, 4800 Bd, ±648 / ±1944 Hz, 9.6 kHz wide, continuous".
    pub summary: String,
    /// The summary and the verdict: "…. Most likely P25 Phase 1 or NXDN96, 71 % sure."
    pub headline: String,
    /// The "why?" rows of the features.
    pub why: Vec<Why>,
}

impl Identification {
    /// The best candidate when the verdict names one (likely), else `None`.
    pub fn best(&self) -> Option<&Candidate> {
        if self.verdict == Verdict::Likely {
            self.candidates.first()
        } else {
            None
        }
    }
    /// The text export: the headline, the candidates with their evidence and the "why?" rows.
    pub fn text(&self) -> String {
        let mut s = format!("{}\n{}\n", self.line, self.headline);
        for c in &self.candidates {
            s.push_str(&format!(
                "\n{} ({}): score {:.0}, {:.0} %\n",
                c.name,
                c.signal,
                c.score,
                c.probability * 100.0
            ));
            for e in c.evidence.iter().chain(&c.priors) {
                s.push_str(&format!("  {e}\n"));
            }
            if let Some(v) = &c.verification {
                for n in &v.notes {
                    s.push_str(&format!("  verify: {n}\n"));
                }
            }
        }
        s.push_str("\nWhy:\n");
        for w in &self.why {
            s.push_str(&format!("  {}: {} ({})\n", w.feature, w.value, w.why));
        }
        s
    }
}

/// Turns scores into calibrated probabilities: p = σ((s − mid) / slope) × softmax(s / temperature). The defaults were measured on
/// the generators and the synthetic evaluation set; [`Calibration::fit`] measures them again.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Calibration {
    /// The score at which a lone candidate is right half the time.
    pub mid: f64,
    /// How fast that rises.
    pub slope: f64,
    /// How much a close runner-up takes away.
    pub temperature: f64,
}

impl Default for Calibration {
    fn default() -> Self {
        Calibration {
            mid: 58.0,
            slope: 7.0,
            temperature: 5.0,
        }
    }
}

impl Calibration {
    /// The probabilities of candidates with these scores (best first or not).
    pub fn probabilities(&self, scores: &[f64]) -> Vec<f64> {
        if scores.is_empty() {
            return Vec::new();
        }
        let max = scores.iter().copied().fold(f64::MIN, f64::max);
        let e: Vec<f64> = scores
            .iter()
            .map(|s| ((s - max) / self.temperature).exp())
            .collect();
        let sum: f64 = e.iter().sum();
        scores
            .iter()
            .zip(&e)
            .map(|(s, w)| {
                let lone = 1.0 / (1.0 + (-(s - self.mid) / self.slope).exp());
                lone * w / sum
            })
            .collect()
    }
    /// Fits the three parameters to examples (the candidates' scores, best first, and the index of the right one if it is among
    /// them) by the least log loss of the top probability, over a grid.
    pub fn fit(examples: &[(Vec<f64>, Option<usize>)]) -> Calibration {
        let mut best = (f64::MAX, Calibration::default());
        for mid in (30..=85).step_by(2) {
            for slope in [3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 10.0, 12.0, 15.0] {
                for temperature in [2.0, 3.0, 4.0, 5.0, 6.0, 8.0, 10.0] {
                    let c = Calibration {
                        mid: mid as f64,
                        slope,
                        temperature,
                    };
                    let mut loss = 0.0;
                    for (s, truth) in examples {
                        if s.is_empty() {
                            continue;
                        }
                        let p = c.probabilities(s)[0].clamp(1e-6, 1.0 - 1e-6);
                        loss -= if *truth == Some(0) {
                            p.ln()
                        } else {
                            (1.0 - p).ln()
                        };
                    }
                    if loss < best.0 {
                        best = (loss, c);
                    }
                }
            }
        }
        best.1
    }
}

/// Scores one signature against the measurement: (raw score, evidence lines, fits). The raw score is about 0 to 120; candidates
/// show it clamped to 100.
pub fn score(sig: &Signature, f: &Features) -> (f64, Vec<String>, bool) {
    let mut s = 10.0;
    let mut ev = Vec::new();
    let mut fits = true;
    let mut add = |ev: &mut Vec<String>, pts: f64, text: String| {
        s += pts;
        ev.push(format!("{text} ({pts:+.0})"));
    };
    // Class.
    if f.class == sig.main_class {
        add(
            &mut ev,
            30.0,
            format!("class {} as expected", f.class.word()),
        );
    } else if sig.classes.contains(&f.class) {
        add(
            &mut ev,
            20.0,
            format!("class {} is one it shows as", f.class.word()),
        );
    } else if f.class == ModClass::Unknown {
        add(&mut ev, 0.0, "class not known".into());
    } else if f.class.family() == sig.main_class.family() {
        add(
            &mut ev,
            5.0,
            format!("class {} near {}", f.class.word(), sig.main_class.word()),
        );
    } else {
        fits = false;
        add(
            &mut ev,
            -30.0,
            format!("class {}, not {}", f.class.word(), sig.main_class.word()),
        );
    }
    // Symbol rate.
    if !sig.symbol_rates.is_empty() && sig.ofdm_tu_s.is_empty() {
        match f.symbol_rate {
            Some(r) => {
                let (e, want) = sig
                    .symbol_rates
                    .iter()
                    .map(|w| ((r / w - 1.0).abs(), *w))
                    .fold((f64::MAX, 0.0), |a, b| if b.0 < a.0 { b } else { a });
                let pct = e * 100.0;
                if e <= 0.02 {
                    let pts = 25.0 - 7.0 * ((e - 0.002).max(0.0) / 0.018);
                    add(
                        &mut ev,
                        pts,
                        format!("symbol rate {r:.0} Bd within {pct:.1} % of {want:.0}"),
                    );
                } else if e <= 0.05 {
                    add(
                        &mut ev,
                        6.0,
                        format!("symbol rate {r:.0} Bd {pct:.1} % from {want:.0}"),
                    );
                } else {
                    if e > 0.1 {
                        fits = false;
                    }
                    add(
                        &mut ev,
                        -20.0,
                        format!("symbol rate {r:.0} Bd, not {want:.0}"),
                    );
                }
            }
            None => {
                if matches!(sig.main_class.family(), 2 | 3) && sig.main_class != ModClass::Fm {
                    add(&mut ev, -5.0, "no symbol rate measured".into());
                }
            }
        }
    }
    // OFDM.
    if !sig.ofdm_tu_s.is_empty() {
        match &f.ofdm {
            Some(o) => {
                let (e, want) = sig
                    .ofdm_tu_s
                    .iter()
                    .map(|w| ((o.tu_s / w - 1.0).abs(), *w))
                    .fold((f64::MAX, 0.0), |a, b| if b.0 < a.0 { b } else { a });
                if e <= 0.01 {
                    add(
                        &mut ev,
                        30.0,
                        format!(
                            "useful symbol {:.1} µs within {:.1} % of {:.1}",
                            o.tu_s * 1e6,
                            e * 100.0,
                            want * 1e6
                        ),
                    );
                } else if e <= 0.03 {
                    add(
                        &mut ev,
                        20.0,
                        format!(
                            "useful symbol {:.1} µs within {:.1} % of {:.1}",
                            o.tu_s * 1e6,
                            e * 100.0,
                            want * 1e6
                        ),
                    );
                } else {
                    fits = false;
                    add(
                        &mut ev,
                        -25.0,
                        format!("useful symbol {:.1} µs fits none of its own", o.tu_s * 1e6),
                    );
                }
                if let (false, Some(g)) = (sig.ofdm_tg_s.is_empty(), o.tg_s) {
                    let eg = sig
                        .ofdm_tg_s
                        .iter()
                        .map(|w| (g / w - 1.0).abs())
                        .fold(f64::MAX, f64::min);
                    if eg < 0.15 {
                        add(&mut ev, 8.0, format!("guard {:.1} µs as expected", g * 1e6));
                    }
                }
            }
            None => add(&mut ev, -15.0, "no cyclic prefix found".into()),
        }
    }
    // Width.
    let w = f.width_99_hz;
    let (lo, hi) = sig.width_hz;
    if hi > 0.0 {
        if sig.width_is_limit {
            if w <= hi * 1.15 {
                add(
                    &mut ev,
                    6.0,
                    format!("{} wide, within its {}", fmt_width(w), fmt_width(hi)),
                );
            } else {
                if w > hi * 2.0 {
                    fits = false;
                }
                add(
                    &mut ev,
                    -20.0,
                    format!("{} wide, more than its {}", fmt_width(w), fmt_width(hi)),
                );
            }
        } else if w >= lo * 0.85 && w <= hi * 1.15 {
            add(&mut ev, 12.0, format!("{} wide, as expected", fmt_width(w)));
        } else if w >= lo * 0.6 && w <= hi * 1.5 {
            add(
                &mut ev,
                3.0,
                format!(
                    "{} wide, near its {}–{}",
                    fmt_width(w),
                    fmt_width(lo),
                    fmt_width(hi)
                ),
            );
        } else {
            if w < lo * 0.4 || w > hi * 2.5 {
                fits = false;
            }
            add(
                &mut ev,
                -20.0,
                format!(
                    "{} wide, not {}–{}",
                    fmt_width(w),
                    fmt_width(lo),
                    fmt_width(hi)
                ),
            );
        }
    }
    // FSK levels and deviation.
    if let (Some(levels), Some(dev)) = (sig.fsk_levels, sig.fsk_outer_dev_hz) {
        if let Some(k) = &f.fsk {
            if k.levels == levels {
                add(&mut ev, 6.0, format!("{levels} levels"));
            } else {
                add(&mut ev, -8.0, format!("{} levels, not {levels}", k.levels));
            }
            let e = (k.outer_dev_hz / dev - 1.0).abs();
            if e <= 0.3 {
                let pts = 14.0 * (1.0 - e / 0.3);
                add(
                    &mut ev,
                    pts,
                    format!(
                        "outer deviation ±{:.0} Hz within {:.0} % of ±{dev:.0}",
                        k.outer_dev_hz,
                        e * 100.0
                    ),
                );
            } else {
                add(
                    &mut ev,
                    -10.0,
                    format!("outer deviation ±{:.0} Hz, not ±{dev:.0}", k.outer_dev_hz),
                );
            }
        }
    }
    // Keying and bursts.
    match sig.keying {
        Keying::Continuous if f.duty >= 0.97 => add(&mut ev, 4.0, "continuous".into()),
        Keying::Continuous => add(&mut ev, -6.0, "bursty, expected continuous".into()),
        Keying::Bursty if f.duty < 0.9 => add(&mut ev, 4.0, "bursty".into()),
        Keying::Bursty => {}
        Keying::Either => {}
    }
    if let (Some(b), Some(m)) = (sig.burst_s, f.burst_s) {
        let e = (m / b - 1.0).abs();
        if e < 0.15 {
            add(
                &mut ev,
                8.0,
                format!("bursts of {:.2} ms as expected", m * 1e3),
            );
        } else if e > 0.5 {
            add(
                &mut ev,
                -8.0,
                format!("bursts of {:.2} ms, not {:.2}", m * 1e3, b * 1e3),
            );
        }
    }
    if let (Some(p), Some(m)) = (sig.period_s, f.burst_period_s) {
        if (m / p - 1.0).abs() < 0.1 || (m / (p / 2.0) - 1.0).abs() < 0.1 {
            add(
                &mut ev,
                8.0,
                format!("bursts every {:.2} ms as expected", m * 1e3),
            );
        }
    }
    // Pilot.
    if sig.fm_pilot {
        if f.fm_pilot {
            add(&mut ev, 15.0, "19 kHz stereo pilot".into());
        } else {
            add(&mut ev, -8.0, "no stereo pilot".into());
        }
    } else if f.fm_pilot && sig.main_class == ModClass::Fm {
        add(&mut ev, -5.0, "a stereo pilot it does not have".into());
    }
    // Chirps.
    if !sig.chirp_sf.is_empty() {
        if let Some(c) = &f.chirp {
            if c.sf.is_some_and(|sf| sig.chirp_sf.contains(&sf)) {
                add(
                    &mut ev,
                    15.0,
                    format!("spreading factor {}", c.sf.unwrap_or(0)),
                );
            }
            if sig
                .chirp_bw_hz
                .iter()
                .any(|b| (c.bw_hz / b - 1.0).abs() < 0.1)
            {
                add(
                    &mut ev,
                    8.0,
                    format!("chirp bandwidth {}", fmt_width(c.bw_hz)),
                );
            }
        }
    }
    // Tones.
    if let (Some((n, sp)), Some(t)) = (sig.tones, &f.tones) {
        if (t.spacing_hz / sp - 1.0).abs() < 0.1 {
            add(
                &mut ev,
                10.0,
                format!(
                    "{} tones {:.0} Hz apart (it has {n})",
                    t.count, t.spacing_hz
                ),
            );
        }
    }
    // Band: at most ±5 points, so a wrong frequency costs little.
    match sig.in_band(f.rf_hz) {
        Some(true) => add(&mut ev, 5.0, "in its band".into()),
        Some(false) => add(&mut ev, -5.0, "outside its bands".into()),
        None => {}
    }
    // A signature with few rules cannot reach a high score: blend the score with its share of what this signature could reach,
    // so an AM broadcast that fits everything it declares is as sure as a DMR signal that does.
    let reach = reachable(sig).max(60.0);
    let s = s.max(0.0);
    (0.5 * s + 0.5 * 100.0 * s / reach, ev, fits)
}

/// The most a signature can score (before verification).
fn reachable(sig: &Signature) -> f64 {
    let mut r = 10.0 + 30.0;
    if !sig.symbol_rates.is_empty() && sig.ofdm_tu_s.is_empty() && sig.main_class != ModClass::Pulse
    {
        r += 25.0;
    }
    if !sig.ofdm_tu_s.is_empty() {
        r += 30.0 + if sig.ofdm_tg_s.is_empty() { 0.0 } else { 8.0 };
    }
    if sig.width_hz.1 > 0.0 {
        r += if sig.width_is_limit { 6.0 } else { 12.0 };
    }
    if sig.fsk_levels.is_some() {
        r += 20.0;
    }
    if sig.keying != Keying::Either {
        r += 4.0;
    }
    if sig.burst_s.is_some() {
        r += 8.0;
    }
    if sig.period_s.is_some() {
        r += 8.0;
    }
    if sig.fm_pilot {
        r += 15.0;
    }
    if !sig.chirp_sf.is_empty() {
        r += 23.0;
    }
    if sig.tones.is_some() {
        r += 10.0;
    }
    if !sig.bands.is_empty() {
        r += 5.0;
    }
    r
}

/// The identifier: a signature table, detectors, priors and a calibration. It is `Send + Sync`, and a clone shares the detectors.
#[derive(Clone)]
pub struct Identifier {
    /// The signatures.
    pub signatures: Vec<Signature>,
    detectors: Vec<Arc<dyn Detector>>,
    /// Priors from band plans and the frequency database.
    pub priors: Vec<Prior>,
    /// The calibration.
    pub calibration: Calibration,
    /// How many of the best candidates are verified (default 3).
    pub verify_top: usize,
    /// How many candidates are kept (default 6).
    pub keep: usize,
}

impl Default for Identifier {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Debug for Identifier {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Identifier")
            .field("signatures", &self.signatures.len())
            .field("detectors", &self.detectors.len())
            .finish()
    }
}

impl Identifier {
    /// The built-in signatures and detectors.
    pub fn new() -> Self {
        Self::with_signatures(builtin())
    }
    /// These signatures and the built-in detectors.
    pub fn with_signatures(signatures: Vec<Signature>) -> Self {
        Identifier {
            signatures,
            detectors: builtin_detectors(),
            priors: Vec::new(),
            calibration: Calibration::default(),
            verify_top: 3,
            keep: 6,
        }
    }
    /// Adds a signature that a stack declares.
    pub fn add_signature(&mut self, s: Signature) {
        self.signatures.push(s);
    }
    /// Adds a detector that a stack declares (its own frame sync search, for example).
    pub fn add_detector(&mut self, d: Arc<dyn Detector>) {
        self.detectors.push(d);
    }

    /// The length of capture the identifier requires at `rate`: 2 s when that is at most 3 M samples, else 3 M samples,
    /// at least 0.6 s, at most 4 M samples.
    pub fn capture_len(rate: f64) -> usize {
        let s = if rate * 2.0 <= 3e6 { 2.0 } else { 3e6 / rate };
        ((s.max(0.6) * rate) as usize).min(4_000_000)
    }

    /// The capture's spectrum, as every entry point computes it.
    pub fn spectrum(x: &[Complex32], rate: f64) -> Psd {
        Psd::welch(x, rate, 4096, 256)
    }

    /// Identifies the signals of the capture `x` (at `rate`, centred on `rf_hz`) that `look` names. `step` gets the share done,
    /// from 0 to 1, after each part of the work. If `step` returns false, the work stops and the result is `None`.
    pub fn look(
        &self,
        x: &[Complex32],
        rate: f64,
        rf_hz: f64,
        look: Look,
        mut step: impl FnMut(f32) -> bool,
    ) -> Option<Vec<Identification>> {
        let psd = Self::spectrum(x, rate);
        if !step(0.2) {
            return None;
        }
        let regions = match look {
            Look::Strongest => vec![find_regions(&psd, MIN_OVER_FLOOR_DB, 1)
                .first()
                .copied()
                .unwrap_or_else(|| Region::around(0.0, CLICK_WIDTH_HZ.min(rate * 0.9)))],
            Look::Click(hz) => vec![click_region(&psd, hz)],
            Look::Region(r) => vec![r],
            Look::All => find_regions(&psd, MIN_OVER_FLOOR_DB, MAX_REGIONS),
        };
        let n = regions.len().max(1) as f32;
        let mut out = Vec::with_capacity(regions.len());
        for (i, r) in regions.iter().enumerate() {
            let id = self.identify_region(x, rate, rf_hz, r, &psd);
            // A region that measures as noise (a skirt or a bump of the floor) is not a signal of its own.
            if look != Look::All || id.features.class != ModClass::Noise {
                out.push(id);
            }
            if !step(0.2 + 0.8 * (i + 1) as f32 / n) {
                return None;
            }
        }
        Some(out)
    }

    /// Identifies the signal in `region` (Hz from the centre) of the capture `x` (at `rate`, centred on `rf_hz`). `None` takes
    /// the strongest signal.
    pub fn identify(
        &self,
        x: &[Complex32],
        rate: f64,
        rf_hz: f64,
        region: Option<Region>,
    ) -> Identification {
        let look = region.map_or(Look::Strongest, Look::Region);
        self.one(x, rate, rf_hz, look)
    }

    /// Identifies what a click at `click_hz` (from the centre) lands on.
    pub fn identify_click(
        &self,
        x: &[Complex32],
        rate: f64,
        rf_hz: f64,
        click_hz: f64,
    ) -> Identification {
        self.one(x, rate, rf_hz, Look::Click(click_hz))
    }

    /// Identifies every signal at least 8 dB over the floor, strongest first (at most 16). A region that measures as noise is left
    /// out.
    pub fn identify_all(&self, x: &[Complex32], rate: f64, rf_hz: f64) -> Vec<Identification> {
        self.look(x, rate, rf_hz, Look::All, |_| true)
            .unwrap_or_default()
    }

    fn one(&self, x: &[Complex32], rate: f64, rf_hz: f64, look: Look) -> Identification {
        let mut v = self
            .look(x, rate, rf_hz, look, |_| true)
            .unwrap_or_default();
        // A look at one region returns exactly one identification.
        v.swap_remove(0)
    }

    /// Identifies the signal in a region with the capture's spectrum already made.
    pub fn identify_region(
        &self,
        x: &[Complex32],
        rate: f64,
        rf_hz: f64,
        region: &Region,
        psd: &Psd,
    ) -> Identification {
        let (f, ch) = measure(x, rate, rf_hz, region, psd);
        self.decide(f, &ch)
    }

    /// The pure decision from measured features and the signal at 0 Hz: candidates, verification, calibration, words.
    pub fn decide(&self, f: Features, ch: &Channel) -> Identification {
        let summary = if f.class == ModClass::Noise {
            "no signal over the noise".to_string()
        } else {
            f.summary()
        };
        let mut cands: Vec<Candidate> = Vec::new();
        if f.class != ModClass::Noise {
            for sig in &self.signatures {
                let (s, ev, fits) = score(sig, &f);
                if s <= 0.0 {
                    continue;
                }
                cands.push(Candidate {
                    name: sig.name.clone(),
                    signal: sig.signal.clone(),
                    family: sig.family.clone(),
                    score: s.min(100.0),
                    raw_score: s,
                    probability: 0.0,
                    evidence: ev,
                    priors: Vec::new(),
                    verification: None,
                    fits,
                    freq_hz: (f.rf_hz / 100.0).round() * 100.0,
                    width_hz: f.width_99_hz,
                });
            }
        }
        // Priors: only for a candidate that fits.
        for c in &mut cands {
            if !c.fits {
                continue;
            }
            for p in &self.priors {
                if p.signal == c.signal && f.rf_hz >= p.lo_hz && f.rf_hz <= p.hi_hz {
                    let pts = p.points.clamp(-8.0, 8.0);
                    c.raw_score = (c.raw_score + pts).max(0.0);
                    c.score = c.raw_score.min(100.0);
                    c.priors
                        .push(format!("{} says {} ({pts:+.0})", p.source, c.name));
                }
            }
        }
        sort(&mut cands);
        // Verify the best few, and any other within 10 points of the best.
        let top = cands.first().map_or(0.0, |c| c.raw_score);
        for (i, c) in cands.iter_mut().enumerate().take(8) {
            if i >= self.verify_top && c.raw_score < top - 10.0 {
                break;
            }
            let Some(sig) = self.signatures.iter().find(|s| s.signal == c.signal) else {
                continue;
            };
            let parts: Vec<Verification> = self
                .detectors
                .iter()
                .filter_map(|d| d.verify(sig, &f, ch))
                .collect();
            if let Some(v) = Verification::merge(parts) {
                match v.check {
                    Check::Confirmed => {
                        let pts = 20.0 * v.weight;
                        c.raw_score += pts;
                        c.evidence
                            .push(format!("its own detector confirmed it (+{pts:.0})"));
                    }
                    Check::Failed => {
                        c.raw_score = (c.raw_score - 25.0).max(0.0);
                        c.evidence
                            .push("its own detector did not find it (−25)".into());
                    }
                    Check::Inconclusive => {}
                }
                c.score = c.raw_score.min(100.0);
                c.verification = Some(v);
            }
        }
        sort(&mut cands);
        cands.truncate(self.keep);
        let scores: Vec<f64> = cands.iter().map(|c| c.raw_score).collect();
        for (c, p) in cands
            .iter_mut()
            .zip(self.calibration.probabilities(&scores))
        {
            c.probability = if c.fits { p } else { p * 0.25 };
        }
        // Verdict and words.
        let width = fmt_width(f.width_99_hz);
        let p0 = cands.first().map_or(0.0, |c| c.probability);
        let p1 = cands.get(1).map_or(0.0, |c| c.probability);
        let verified = cands
            .first()
            .and_then(|c| c.verification.as_ref())
            .is_some_and(|v| v.check == Check::Confirmed);
        let (verdict, sureness, line) =
            if f.class == ModClass::Noise || cands.is_empty() || p0 < 0.25 || !cands[0].fits {
                let line = if f.class == ModClass::Noise {
                    "No signal here".to_string()
                } else {
                    format!("Unknown signal, {width} wide")
                };
                (Verdict::Unknown, Sureness::None, line)
            } else if p1 >= 0.7 * p0 && cands[1].fits {
                (
                    Verdict::Ambiguous,
                    Sureness::Guess,
                    format!("Could be: {} or {}", cands[0].name, cands[1].name),
                )
            } else if verified && p0 >= 0.8 {
                (
                    Verdict::Likely,
                    Sureness::Sure,
                    format!("Sure: {}", cands[0].name),
                )
            } else if p0 >= 0.5 {
                (
                    Verdict::Likely,
                    Sureness::Likely,
                    format!("Probably: {}", cands[0].name),
                )
            } else {
                (
                    Verdict::Likely,
                    Sureness::Guess,
                    format!("Could be: {}", cands[0].name),
                )
            };
        let headline = match verdict {
            Verdict::Unknown => format!("{summary}. Not one the identifier knows."),
            Verdict::Ambiguous => format!(
                "{summary}. Most likely {} or {}, {:.0} % sure.",
                cands[0].name,
                cands[1].name,
                (p0 + p1).min(1.0) * 100.0
            ),
            Verdict::Likely => format!(
                "{summary}. Most likely {}, {:.0} % sure.",
                cands[0].name,
                p0 * 100.0
            ),
        };
        Identification {
            why: f.why.clone(),
            features: f,
            candidates: cands,
            verdict,
            sureness,
            line,
            summary,
            headline,
        }
    }
}

fn sort(c: &mut [Candidate]) {
    c.sort_by(|a, b| {
        b.fits
            .cmp(&a.fits)
            .then(b.raw_score.total_cmp(&a.raw_score))
            .then(a.name.cmp(&b.name))
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn calibration_prefers_a_lone_high_score() {
        let c = Calibration::default();
        let p = c.probabilities(&[90.0, 40.0]);
        assert!(p[0] > 0.9 && p[1] < 0.05);
        let p = c.probabilities(&[80.0, 79.0]);
        assert!(p[0] < 0.6 && (p[0] - p[1]).abs() < 0.15);
        let p = c.probabilities(&[30.0]);
        assert!(p[0] < 0.05);
        // Fitting on examples where 70 is right and 50 is not moves the middle between them.
        let ex: Vec<(Vec<f64>, Option<usize>)> = (0..20)
            .map(|i| {
                if i % 2 == 0 {
                    (vec![70.0], Some(0))
                } else {
                    (vec![50.0], None)
                }
            })
            .collect();
        let f = Calibration::fit(&ex);
        assert!(f.mid > 50.0 && f.mid < 70.0, "{f:?}");
    }

    #[test]
    fn capture_lengths() {
        assert_eq!(Identifier::capture_len(1e6), 2_000_000);
        assert_eq!(Identifier::capture_len(3e6), 3_000_000);
        assert_eq!(Identifier::capture_len(10e6), 4_000_000);
        assert_eq!(Identifier::capture_len(20e6), 4_000_000);
    }
}
