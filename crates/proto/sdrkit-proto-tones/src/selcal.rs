//! Aviation SELCAL (selective calling): the decoder and the generator of calls of two pulses of two tones, code `AB-CD`.
//! The tones and codes are in `sdrkit_util_ids::tones::selcal`.
//!
//! Standard: ARINC 714, ASRI SELCAL User's Guide Table 2-1

use crate::analyse::{db_power, power_db, Analyser};
use crate::gen::{Step, ToneSequence};
use crate::{Param, TonesError};
use sdrkit_algo_dsp::window::Window;
use sdrkit_util_ids::tones::selcal::{Code, Tone, TONES};

/// The stack id of aviation SELCAL. One stack registers it with this decoder.
pub const STACK_ID: &str = "selcal";

/// The tone shifts tried with `ssb_search`, Hz.
const SHIFTS: [f32; 11] = [
    -25.0, -20.0, -15.0, -10.0, -5.0, 0.0, 5.0, 10.0, 15.0, 20.0, 25.0,
];

/// The settings of [`SelcalDecoder`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SelcalConfig {
    /// Listen for the 16 classic tones only (otherwise all 32).
    pub classic_only: bool,
    /// The analysis window, s (0.05 to 0.5).
    pub window_s: f64,
    /// The time between analyses, s (0.01 up to the window).
    pub hop_s: f64,
    /// The smallest level of each tone of a pair, dB relative to a full-scale sine.
    pub min_level_db: f32,
    /// The shortest pulse, s (0.2 to 1.5).
    pub min_pulse_s: f64,
    /// The longest gap between the two pulses, s (0.05 to 2).
    pub max_gap_s: f64,
    /// Try the tones moved by -25 to +25 Hz (a sideband receiver off tune).
    pub ssb_search: bool,
}

impl Default for SelcalConfig {
    /// All 32 tones, a 0.2 s window every 0.05 s, -45 dB, pulses of 0.5 s or more, gaps up to 0.8 s, no shift search.
    fn default() -> Self {
        SelcalConfig {
            classic_only: false,
            window_s: 0.2,
            hop_s: 0.05,
            min_level_db: -45.0,
            min_pulse_s: 0.5,
            max_gap_s: 0.8,
            ssb_search: false,
        }
    }
}

/// A call.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SelcalCall {
    /// The code, first pulse first.
    pub code: Code,
    /// The input sample (counted from the decoder's start) at the start of the first pulse.
    pub start: u64,
    /// The input sample at the end of the second pulse.
    pub end: u64,
    /// The mean level of the four tones, dB relative to a full-scale sine.
    pub level_db: f32,
    /// The shift that fitted best, Hz (0 without `ssb_search`).
    pub shift_hz: f32,
}

/// A pair that ended: the two tone indexes, its start and its end (input samples).
type Pulse = ((usize, usize), u64, u64);

/// The SELCAL decoder for real audio at 4000 Hz or more. Every `hop_s` it measures the tones over the last `window_s` (Hann
/// window). The two strongest tones are a pair when they hold 0.65 of the power, the weaker holds 0.15 and a fifth of the
/// stronger, the third tone is at most a quarter of the weaker, and the weaker is at least `min_level_db`. A pair that lasts
/// `min_pulse_s` is a pulse. Two different pulses less than `max_gap_s` apart are a call. With `ssb_search`, the decoder also
/// tries the tones moved by -25 to +25 Hz in 5 Hz steps. The decoded calls do not depend on how the input is split across
/// `process` calls.
///
/// ```
/// use sdrkit_util_ids::tones::selcal::Code;
/// use sdrkit_proto_tones::selcal::{selcal_sequence, SelcalConfig, SelcalDecoder};
/// let code = Code::parse("AM-CP").unwrap();
/// let mut gen = selcal_sequence(8000.0, code, 0.25, 1.0, 0.2, 0.0).unwrap();
/// let mut audio = vec![0f32; gen.len() as usize + 8000];
/// gen.fill(&mut audio);
/// let mut d = SelcalDecoder::new(8000.0, SelcalConfig::default()).unwrap();
/// let mut calls = Vec::new();
/// d.process(&audio, |c| calls.push(c.code.to_string()));
/// assert_eq!(calls, ["AM-CP"]);
/// ```
#[derive(Clone, Debug)]
pub struct SelcalDecoder {
    cfg: SelcalConfig,
    an: Analyser,
    tones: Vec<Tone>,
    shifts: &'static [f32],
    rate: f64,
    cur: Option<(usize, usize)>,
    cur_start: u64,
    cur_shift: f32,
    level: (f64, u32),
    cand: Option<(usize, usize)>,
    cand_n: u32,
    first: Option<(Pulse, f64)>,
}

impl SelcalDecoder {
    /// A decoder for audio at `rate` Hz (4000 or more). Errors for a bad rate or setting.
    pub fn new(rate: f64, cfg: SelcalConfig) -> Result<Self, TonesError> {
        let ok = rate.is_finite()
            && rate >= 4000.0
            && (0.05..=0.5).contains(&cfg.window_s)
            && cfg.hop_s >= 0.01
            && cfg.hop_s <= cfg.window_s
            && cfg.min_level_db.is_finite()
            && (0.2..=1.5).contains(&cfg.min_pulse_s)
            && (0.05..=2.0).contains(&cfg.max_gap_s);
        if !ok {
            return Err(TonesError::BadParameter(Param::Selcal));
        }
        let tones: Vec<Tone> = TONES
            .iter()
            .filter(|t| t.classic || !cfg.classic_only)
            .copied()
            .collect();
        let shifts: &'static [f32] = if cfg.ssb_search { &SHIFTS } else { &[0.0] };
        let freqs: Vec<f64> = shifts
            .iter()
            .flat_map(|&s| tones.iter().map(move |t| (t.hz + s) as f64 / rate))
            .collect();
        let len = (cfg.window_s * rate).round() as usize;
        let hop = ((cfg.hop_s * rate).round() as usize).clamp(1, len);
        let an = Analyser::new(&freqs, len, hop, Window::Hann, true)?;
        Ok(SelcalDecoder {
            cfg,
            an,
            tones,
            shifts,
            rate,
            cur: None,
            cur_start: 0,
            cur_shift: 0.0,
            level: (0.0, 0),
            cand: None,
            cand_n: 0,
            first: None,
        })
    }

    /// The tones listened for.
    pub fn tones(&self) -> &[Tone] {
        &self.tones
    }

    /// Forgets everything.
    pub fn reset(&mut self) {
        self.an.reset();
        self.cur = None;
        self.cand = None;
        self.cand_n = 0;
        self.first = None;
    }

    /// Takes the next audio samples and calls `on_call` for every call.
    pub fn process(&mut self, x: &[f32], mut on_call: impl FnMut(SelcalCall)) {
        for &v in x {
            if self.an.push(v) {
                if let Some(c) = self.step() {
                    on_call(c);
                }
            }
        }
    }

    /// The pair in the last window, with its shift and the mean power of its two tones.
    fn pair(&self) -> Option<((usize, usize), f32, f64)> {
        let hop = self.an.last();
        let m = self.tones.len();
        let mut best: Option<(f64, usize)> = None;
        for (si, _) in self.shifts.iter().enumerate() {
            let p = &hop.power[si * m..(si + 1) * m];
            let (mut a, mut b) = (0.0f64, 0.0f64);
            for &v in p {
                if v > a {
                    b = a;
                    a = v;
                } else if v > b {
                    b = v;
                }
            }
            if best.is_none_or(|x| a + b > x.0) {
                best = Some((a + b, si));
            }
        }
        let (_, si) = best?;
        let p = &hop.power[si * m..(si + 1) * m];
        let (mut i0, mut i1, mut i2) = (usize::MAX, usize::MAX, usize::MAX);
        for k in 0..m {
            let v = p[k];
            let get = |i: usize| if i == usize::MAX { -1.0 } else { p[i] };
            if v > get(i0) {
                (i2, i1, i0) = (i1, i0, k);
            } else if v > get(i1) {
                (i2, i1) = (i1, k);
            } else if v > get(i2) {
                i2 = k;
            }
        }
        let share = |i: usize| {
            if i == usize::MAX {
                0.0
            } else {
                hop.share(si * m + i)
            }
        };
        let (a, b, c) = (share(i0), share(i1), share(i2));
        let ok = a + b >= 0.65
            && b >= 0.15
            && b >= 0.2 * a
            && c <= 0.25 * b
            && p[i1] >= db_power(self.cfg.min_level_db);
        ok.then(|| {
            (
                (i0.min(i1), i0.max(i1)),
                self.shifts[si],
                0.5 * (p[i0] + p[i1]),
            )
        })
    }

    fn step(&mut self) -> Option<SelcalCall> {
        let now = self.an.position();
        let hop = self.an.hop() as u64;
        // The window's middle is about half a window behind the newest sample.
        let centre = now.saturating_sub(self.an.len() as u64 / 2);
        let seen = self.pair();
        let pair = seen.map(|s| s.0);
        if let Some((p, _, f)) = seen {
            if Some(p) == self.cur {
                self.level.0 += f;
                self.level.1 += 1;
            }
        }
        if let Some((pulse, _)) = self.first {
            if (centre.saturating_sub(pulse.2)) as f64 / self.rate > 3.0 {
                self.first = None;
            }
        }
        if pair == self.cur {
            self.cand = None;
            self.cand_n = 0;
            return None;
        }
        if pair == self.cand {
            self.cand_n += 1;
        } else {
            self.cand = pair;
            self.cand_n = 1;
        }
        if self.cand_n < 2 {
            return None;
        }
        // The change happened two hops ago.
        let at = centre.saturating_sub(hop);
        let call = self.end_pulse(at);
        self.cur = pair;
        self.cur_start = at;
        self.cur_shift = seen.map_or(0.0, |s| s.1);
        self.level = seen.map_or((0.0, 0), |s| (s.2, 1));
        self.cand = None;
        self.cand_n = 0;
        call
    }

    fn end_pulse(&mut self, end: u64) -> Option<SelcalCall> {
        let p = self.cur?;
        if ((end - self.cur_start) as f64 / self.rate) < self.cfg.min_pulse_s {
            return None;
        }
        let level = if self.level.1 > 0 {
            self.level.0 / self.level.1 as f64
        } else {
            0.0
        };
        let this: Pulse = (p, self.cur_start, end);
        match self.first.take() {
            Some((f, flevel))
                if f.0 != p
                    && ((this.1.saturating_sub(f.2)) as f64 / self.rate) < self.cfg.max_gap_s =>
            {
                let d = |i: usize| self.tones[i].designator as u8;
                let text = [d(f.0 .0), d(f.0 .1), d(p.0), d(p.1)];
                let code = Code::parse(std::str::from_utf8(&text).ok()?)?;
                Some(SelcalCall {
                    code,
                    start: f.1,
                    end,
                    level_db: power_db(0.5 * (flevel + level)),
                    shift_hz: self.cur_shift,
                })
            }
            _ => {
                self.first = Some((this, level));
                None
            }
        }
    }
}

/// The audio of a SELCAL call: the first pair for `pulse_s`, `gap_s` of silence, the second pair for `pulse_s`. Each tone has
/// amplitude `amp` and is moved by `shift_hz` (a sideband receiver off tune). A 5 ms ramp shapes each pulse. Errors for a bad
/// rate or tone.
pub fn selcal_sequence(
    rate: f64,
    code: Code,
    amp: f32,
    pulse_s: f64,
    gap_s: f64,
    shift_hz: f64,
) -> Result<ToneSequence, TonesError> {
    let t = code.tones().map(|f| f as f64 + shift_hz);
    ToneSequence::new(
        rate,
        &[
            Step::two([t[0], t[1]], [amp; 2], pulse_s),
            Step::silence(gap_s),
            Step::two([t[2], t[3]], [amp; 2], pulse_s),
        ],
        0.005,
    )
}
