//! The test signal of `cw_keying`: a carrier keyed with text in Morse code (ITU-R M.1677-1 element and gap ratios, through
//! `sdrkit_util_text::morse`) by a keyer of a set speed, dah ratio, weight and gaps, with random mark lengths, raised-cosine
//! edges centred on the nominal edges, a chirp that decays from the key-down and a slow level swing. With `T = 1.2 / wpm` s a
//! mark lasts `(1 or the dah ratio) T weight (1 + jitter u)`, `u` uniform in -1 to 1; the element gap is `T (2 - weight)`;
//! a character adds `(character gap - 1) T` and a word `(word gap - character gap) T`. The text starts at 1 s. The offset,
//! the drift and the noise are the standard impairments; an SNR is the CNR in 500 Hz.

use sdrkit_algo_sim::rng::Rng;
use sdrkit_core::publish::field;
use sdrkit_core::{Complex32, Generator, Hz, Setting, SignalInfo, Values};
use sdrkit_util_text::morse;
use std::f64::consts::{PI, TAU};

const SECTION: &str = "Test signal";

/// The default text.
pub const DEFAULT_TEXT: &str = "CQ CQ DE WIAW WIAW K PARIS PARIS QRZ TEST";
/// The text starts here, s.
pub const LEAD_S: f64 = 1.0;

/// The text sent.
pub const GEN_TEXT: Setting<field::Text> =
    Setting::new("gen_text", "Text", "", DEFAULT_TEXT).section(SECTION);
/// The speed.
pub const GEN_WPM: Setting<field::F64> = Setting::new("gen_wpm", "Speed", "wpm", 20.0)
    .range(1.0, 100.0, 0.0)
    .section(SECTION);
/// The dah over the dit.
pub const GEN_DAH_RATIO: Setting<field::F64> = Setting::new("gen_dah_ratio", "Dah ratio", "", 3.0)
    .range(1.5, 6.0, 0.0)
    .section(SECTION);
/// The weight.
pub const GEN_WEIGHT: Setting<field::F64> = Setting::new("gen_weight", "Weight", "", 1.0)
    .range(0.5, 1.5, 0.0)
    .section(SECTION);
/// The character gap in units.
pub const GEN_CHAR_GAP: Setting<field::F64> =
    Setting::new("gen_char_gap", "Character gap", "units", 3.0)
        .range(2.0, 10.0, 0.0)
        .section(SECTION);
/// The word gap in units.
pub const GEN_WORD_GAP: Setting<field::F64> =
    Setting::new("gen_word_gap", "Word gap", "units", 7.0)
        .range(4.0, 20.0, 0.0)
        .section(SECTION);
/// The mark length jitter.
pub const GEN_JITTER: Setting<field::F64> = Setting::new("gen_jitter", "Mark jitter", "", 0.0)
    .range(0.0, 0.5, 0.0)
    .section(SECTION);
/// The length of an edge.
pub const GEN_RAMP_S: Setting<field::F64> = Setting::new("gen_ramp_s", "Edge length", "s", 0.006)
    .range(0.0, 0.05, 0.0)
    .section(SECTION);
/// The chirp at key-down.
pub const GEN_CHIRP_HZ: Setting<field::F64> = Setting::new("gen_chirp_hz", "Chirp", "Hz", 0.0)
    .range(-1000.0, 1000.0, 0.0)
    .section(SECTION);
/// The time constant of the chirp.
pub const GEN_CHIRP_TAU_S: Setting<field::F64> =
    Setting::new("gen_chirp_tau_s", "Chirp time constant", "s", 0.02)
        .range(0.001, 1.0, 0.0)
        .section(SECTION);
/// The level swing.
pub const GEN_FADE_DB: Setting<field::F64> =
    Setting::new("gen_fade_db", "Level swing at 0.2 Hz", "dB", 0.0)
        .range(0.0, 40.0, 0.0)
        .section(SECTION);

/// The band of a noise stated as an SNR, Hz.
pub const NOISE_BAND_HZ: f64 = 500.0;

/// The nominal marks of the settings `v`, `(start s, end s)`, with the random lengths of `rng`.
pub fn marks(v: &Values, rng: &mut Rng) -> Vec<(f64, f64)> {
    let t = 1.2 / v.get(&GEN_WPM);
    let (dah, w, jit) = (
        v.get(&GEN_DAH_RATIO),
        v.get(&GEN_WEIGHT),
        v.get(&GEN_JITTER),
    );
    let (cg, wg) = (v.get(&GEN_CHAR_GAP), v.get(&GEN_WORD_GAP));
    let mut out = Vec::new();
    let mut at = LEAD_S;
    for (wi, word) in v.get(&GEN_TEXT).split_whitespace().enumerate() {
        if wi > 0 {
            at += (wg - cg) * t;
        }
        for (ci, sym) in morse::symbols(word).enumerate() {
            let Ok(code) = morse::encode(sym) else {
                continue;
            };
            if ci > 0 {
                at += (cg - 1.0) * t;
            }
            for d in code.elements() {
                let len = if d { dah } else { 1.0 } * t * w * (1.0 + jit * rng.range(-1.0, 1.0));
                out.push((at, at + len));
                at += len + t * (2.0 - w);
            }
        }
    }
    out
}

/// The test signal generator.
#[derive(Clone, Debug)]
pub struct CwGenerator {
    rate: f64,
    marks: Vec<(f64, f64)>,
    ramp: f64,
    chirp: f64,
    chirp_tau: f64,
    fade_db: f64,
    next: usize,
    phase: f64,
    n: u64,
}

impl CwGenerator {
    /// The signal of the settings `v` at `rate_hz` with the random lengths of `seed`.
    pub fn new(rate_hz: f64, v: &Values, seed: u64) -> CwGenerator {
        let mut rng = Rng::stream(seed, 47);
        CwGenerator {
            rate: rate_hz,
            marks: marks(v, &mut rng),
            ramp: v.get(&GEN_RAMP_S),
            chirp: v.get(&GEN_CHIRP_HZ),
            chirp_tau: v.get(&GEN_CHIRP_TAU_S),
            fade_db: v.get(&GEN_FADE_DB),
            next: 0,
            phase: 0.0,
            n: 0,
        }
    }

    /// The gain of the mark `(s, e)` at `t`: raised-cosine edges of the ramp length centred on `s` and `e`.
    fn gain(&self, (s, e): (f64, f64), t: f64) -> f64 {
        let h = self.ramp / 2.0;
        let edge = |u: f64| {
            if self.ramp > 0.0 {
                0.5 - 0.5 * (PI * ((u + h) / self.ramp).clamp(0.0, 1.0)).cos()
            } else {
                f64::from(u >= 0.0)
            }
        };
        edge(t - s).min(edge(e - t))
    }
}

impl Generator for CwGenerator {
    fn rate_hz(&self) -> Hz {
        self.rate
    }
    fn generate(&mut self, out: &mut [Complex32]) -> usize {
        let h = self.ramp / 2.0;
        for z in out.iter_mut() {
            let t = self.n as f64 / self.rate;
            self.n += 1;
            while self.next < self.marks.len() && self.marks[self.next].1 + h <= t {
                self.next += 1;
            }
            let Some(&m) = self.marks.get(self.next).filter(|m| t >= m.0 - h) else {
                *z = Complex32::default();
                continue;
            };
            let f = self.chirp * (-(t - m.0).max(0.0) / self.chirp_tau).exp();
            self.phase = (self.phase + TAU * f / self.rate).rem_euclid(TAU);
            let level = 10f64.powf(self.fade_db / 2.0 * (TAU * 0.2 * t).sin() / 20.0);
            *z = Complex32::from_polar((self.gain(m, t) * level) as f32, self.phase as f32);
        }
        out.len()
    }
    /// The noise is stated against a carrier of power 1; an SNR is stated in 500 Hz.
    fn signal_info(&self) -> Option<SignalInfo> {
        Some(SignalInfo::new(NOISE_BAND_HZ).with_power(1.0))
    }
    fn station_offset_hz(&self, _tune_offset_hz: Hz) -> Hz {
        0.0
    }
}
