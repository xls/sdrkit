//! DTMF (dual-tone multi-frequency, the 16 telephone keys): the decoder and the generator.
//!
//! Standard: ITU-T Q.23, Q.24

use crate::analyse::{db_power, power_db, Analyser};
use crate::gen::{Step, ToneSequence};
use crate::{Param, TonesError};
use sdrkit_algo_dsp::window::Window;
use sdrkit_util_ids::tones::dtmf;
use std::f64::consts::{PI, TAU};

/// The settings of [`DtmfDecoder`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DtmfConfig {
    /// The smallest level of each tone, dB relative to a full-scale sine.
    pub min_level_db: f32,
    /// The smallest share of the window's power the two tones hold together (0 to 1).
    pub min_share: f64,
    /// How much weaker every other tone of a group must be, dB (0 or more).
    pub group_margin_db: f32,
    /// The largest amount the column tone can be above the row tone, dB (0 or more).
    pub max_twist_db: f32,
    /// The largest amount the column tone can be below the row tone, dB (0 or more).
    pub max_reverse_twist_db: f32,
    /// The largest frequency deviation, as a fraction (0.005 to 0.035).
    pub max_deviation: f64,
    /// Refuse a window whose tones have strong second harmonics.
    pub harmonics: bool,
    /// Windows in a row that hold a key before it goes down (1 or more).
    pub min_on: u32,
    /// Windows in a row without its tones before a key goes up (1 or more).
    pub min_off: u32,
    /// Windows in a row that do not hold the key before it goes up anyway (1 or more).
    pub max_invalid: u32,
}

impl Default for DtmfConfig {
    /// The values of the module documentation.
    fn default() -> Self {
        DtmfConfig {
            min_level_db: -45.0,
            min_share: 0.5,
            group_margin_db: 10.0,
            max_twist_db: 4.0,
            max_reverse_twist_db: 8.0,
            max_deviation: 0.025,
            harmonics: true,
            min_on: 2,
            min_off: 2,
            max_invalid: 8,
        }
    }
}

/// A key going down or up.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DtmfEvent {
    /// The key: `0` to `9`, `*`, `#`, `A` to `D`.
    pub key: char,
    /// The key went down (`true`) or up.
    pub down: bool,
    /// The input sample (counted from the decoder's start) at the end of the window that decided.
    pub at: u64,
    /// The row tone level when the key went down, dB relative to a full-scale sine.
    pub row_db: f32,
    /// The column tone level when the key went down, dB relative to a full-scale sine.
    pub column_db: f32,
}

/// What one window holds.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Seen {
    /// A valid key, with its levels.
    Key(char, f32, f32),
    /// A signal that is not a valid key. The tones of the key down can be part of it.
    Other,
    /// Silence or noise only.
    Nothing,
}

/// The DTMF decoder for real audio at 4000 Hz or more. It measures the eight tones and their second harmonics over windows of
/// 25.6 ms, every quarter window. A window holds a key when the strongest row and column tones are each at least
/// `min_level_db`, hold `min_share` of the power, stand `group_margin_db` above the other tones of their group, have a twist
/// within the limits, lie within `max_deviation` of their frequencies, and are present in both halves of the window. With
/// `harmonics`, a tone with a strong second harmonic (speech) is refused. A key goes down after two windows that hold it, and
/// up after two windows without its tones. The events do not depend on how the input is split across calls.
///
/// ```
/// use sdrkit_proto_tones::dtmf::{dtmf_sequence, DtmfConfig, DtmfDecoder};
/// let mut gen = dtmf_sequence(8000.0, "159#", 0.2, 0.2, 0.05, 0.08).unwrap();
/// let mut audio = vec![0f32; gen.len() as usize];
/// gen.fill(&mut audio);
/// let mut d = DtmfDecoder::new(8000.0, DtmfConfig::default()).unwrap();
/// let mut keys = String::new();
/// d.process(&audio, |e| if e.down { keys.push(e.key) });
/// assert_eq!(keys, "159#");
/// ```
#[derive(Clone, Debug)]
pub struct DtmfDecoder {
    cfg: DtmfConfig,
    an: Analyser,
    rate: f64,
    /// The index in the analyser's list of each tone's second harmonic.
    harm: [Option<usize>; 8],
    key: Option<(char, f32, f32)>,
    cand: Option<char>,
    cand_n: u32,
    off_n: u32,
    invalid_n: u32,
    events: [Option<DtmfEvent>; 2],
}

impl DtmfDecoder {
    /// A decoder for audio at `rate` Hz (4000 or more). Errors for a bad rate or setting.
    pub fn new(rate: f64, cfg: DtmfConfig) -> Result<Self, TonesError> {
        let ok = rate.is_finite()
            && rate >= 4000.0
            && cfg.min_level_db.is_finite()
            && (0.0..=1.0).contains(&cfg.min_share)
            && cfg.group_margin_db >= 0.0
            && cfg.max_twist_db >= 0.0
            && cfg.max_reverse_twist_db >= 0.0
            && (0.005..=0.035).contains(&cfg.max_deviation)
            && cfg.min_on >= 1
            && cfg.min_off >= 1
            && cfg.max_invalid >= 1;
        if !ok {
            return Err(TonesError::BadParameter(Param::Dtmf));
        }
        let mut freqs: Vec<f64> = dtmf::ROWS
            .iter()
            .chain(dtmf::COLUMNS.iter())
            .map(|&f| f as f64 / rate)
            .collect();
        let mut harm = [None; 8];
        for (k, h) in harm.iter_mut().enumerate() {
            let f2 = 2.0 * freqs[k];
            if f2 < 0.45 {
                *h = Some(freqs.len());
                freqs.push(f2);
            }
        }
        let len = ((0.0256 * rate / 4.0).round() as usize * 4).max(8);
        let an = Analyser::new(&freqs, len, len / 4, Window::Rectangular, true)?;
        Ok(DtmfDecoder {
            cfg,
            an,
            rate,
            harm,
            key: None,
            cand: None,
            cand_n: 0,
            off_n: 0,
            invalid_n: 0,
            events: [None; 2],
        })
    }

    /// The key down now.
    pub fn key(&self) -> Option<char> {
        self.key.map(|k| k.0)
    }

    /// The window length, samples.
    pub fn window(&self) -> usize {
        self.an.len()
    }

    /// Forgets everything (no event for a key that was down).
    pub fn reset(&mut self) {
        self.an.reset();
        self.key = None;
        self.cand = None;
        self.cand_n = 0;
        self.off_n = 0;
        self.invalid_n = 0;
    }

    /// Takes the next audio samples and calls `on_event` for every key going down or up.
    pub fn process(&mut self, x: &[f32], mut on_event: impl FnMut(DtmfEvent)) {
        for &v in x {
            if self.an.push(v) {
                let seen = self.look();
                self.step(seen);
                for e in self.events.iter_mut() {
                    if let Some(e) = e.take() {
                        on_event(e);
                    }
                }
            }
        }
    }

    /// The tone `k` (0 to 7) frequency error as a fraction, from the phase change between the halves of the window.
    fn deviation(&self, k: usize) -> Option<f64> {
        let f = self.an.freqs()[k];
        let m = self.an.len() / 2;
        let a = self.an.phasor_part(f, 0, m);
        let b = self.an.phasor_part(f, m, m);
        let (na, nb) = (a.norm(), b.norm());
        if na <= 1e-12 || nb <= 1e-12 {
            return None;
        }
        // Both halves within 3 dB of each other.
        if (20.0 * (na / nb).log10()).abs() > 3.0 {
            return None;
        }
        let expect = TAU * f * m as f64;
        let d = ((b * a.conj()).arg() - expect + PI).rem_euclid(TAU) - PI;
        let df = d / (TAU * m as f64);
        Some(df / f)
    }

    fn look(&self) -> Seen {
        let hop = self.an.last();
        let floor = db_power(self.cfg.min_level_db);
        let p = hop.power;
        let pick = |r: std::ops::Range<usize>| {
            r.clone()
                .max_by(|&a, &b| p[a].total_cmp(&p[b]))
                .unwrap_or(r.start)
        };
        let (r, c) = (pick(0..4), pick(4..8));
        if p[r] < floor && p[c] < floor {
            return Seen::Nothing;
        }
        if p[r] < floor || p[c] < floor || hop.share(r) + hop.share(c) < self.cfg.min_share {
            return Seen::Other;
        }
        let margin = 10f64.powf(self.cfg.group_margin_db as f64 / 10.0);
        let others_low = |best: usize, range: std::ops::Range<usize>| {
            range
                .filter(|&k| k != best)
                .all(|k| p[k] * margin <= p[best])
        };
        if !others_low(r, 0..4) || !others_low(c, 4..8) {
            return Seen::Other;
        }
        let (rdb, cdb) = (power_db(p[r]), power_db(p[c]));
        let twist = cdb - rdb;
        if twist > self.cfg.max_twist_db || -twist > self.cfg.max_reverse_twist_db {
            return Seen::Other;
        }
        if self.cfg.harmonics {
            for k in [r, c] {
                if let Some(h) = self.harm[k] {
                    if p[h] * 10.0 > p[k] {
                        return Seen::Other;
                    }
                }
            }
        }
        for k in [r, c] {
            match self.deviation(k) {
                Some(d) if d.abs() <= self.cfg.max_deviation => {}
                _ => return Seen::Other,
            }
        }
        Seen::Key(dtmf::KEYS[r][c - 4], rdb, cdb)
    }

    /// Whether the window holds neither tone of `key`.
    fn absent(&self, key: char) -> bool {
        let Some((r, c)) = dtmf::position(key) else {
            return true;
        };
        let hop = self.an.last();
        let floor = db_power(self.cfg.min_level_db);
        let gone = |k: usize| hop.power[k] < floor || hop.share(k) < 0.05;
        gone(r) && gone(c + 4)
    }

    fn step(&mut self, seen: Seen) {
        let at = self.an.position();
        let key_now = match seen {
            Seen::Key(k, _, _) => Some(k),
            _ => None,
        };
        // The candidate for the next key.
        if key_now.is_some() && key_now == self.cand {
            self.cand_n += 1;
        } else {
            self.cand = key_now;
            self.cand_n = key_now.is_some() as u32;
        }
        if let Some((k, rdb, cdb)) = self.key {
            if key_now == Some(k) {
                self.off_n = 0;
                self.invalid_n = 0;
                return;
            }
            self.invalid_n += 1;
            if seen == Seen::Nothing || self.absent(k) {
                self.off_n += 1;
            } else {
                self.off_n = 0;
            }
            let other_key = key_now.is_some() && self.cand_n >= self.cfg.min_on;
            if self.off_n >= self.cfg.min_off || self.invalid_n >= self.cfg.max_invalid || other_key
            {
                self.events[0] = Some(DtmfEvent {
                    key: k,
                    down: false,
                    at,
                    row_db: rdb,
                    column_db: cdb,
                });
                self.key = None;
                self.off_n = 0;
                self.invalid_n = 0;
            } else {
                return;
            }
        }
        if let (Seen::Key(k, rdb, cdb), true) = (seen, self.cand_n >= self.cfg.min_on) {
            self.key = Some((k, rdb, cdb));
            self.off_n = 0;
            self.invalid_n = 0;
            self.events[1] = Some(DtmfEvent {
                key: k,
                down: true,
                at,
                row_db: rdb,
                column_db: cdb,
            });
        }
    }

    /// The rate, Hz.
    pub fn rate(&self) -> f64 {
        self.rate
    }
}

/// The audio of `keys` (`0` to `9`, `*`, `#`, `A` to `D`, either case). A space is a pause of one key time. Each key lasts
/// `tone_s`, then `gap_s` of silence follows. The row tone has amplitude `row_amp` and the column tone `column_amp`, with 2 ms
/// ramps. Errors for another character or a bad rate.
pub fn dtmf_sequence(
    rate: f64,
    keys: &str,
    row_amp: f32,
    column_amp: f32,
    tone_s: f64,
    gap_s: f64,
) -> Result<ToneSequence, TonesError> {
    let mut steps = Vec::with_capacity(2 * keys.len());
    for k in keys.chars() {
        if k == ' ' {
            steps.push(Step::silence(tone_s + gap_s));
            continue;
        }
        let (r, c) = dtmf::tones(k).ok_or(TonesError::NotADtmfKey { key: k })?;
        steps.push(Step::two(
            [r as f64, c as f64],
            [row_amp, column_amp],
            tone_s,
        ));
        steps.push(Step::silence(gap_s));
    }
    ToneSequence::new(rate, &steps, 0.002)
}
