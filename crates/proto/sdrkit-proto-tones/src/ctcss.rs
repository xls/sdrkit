//! CTCSS (continuous tone-coded squelch system): the decoder of the 50 sub-audio tones of `sdrkit_util_ids::tones::ctcss`, and
//! the generator of a tone with its reverse burst.
//!
//! Standard: EIA RS-220-A, TIA-603

use crate::analyse::{db_power, power_db, Analyser};
use crate::gen::{Step, ToneSequence};
use crate::subaudio::SubAudio;
use crate::{Param, TonesError};
use num_complex::Complex32;
use sdrkit_algo_dsp::goertzel::Goertzel;
use sdrkit_algo_dsp::peak::log_parabolic;
use sdrkit_algo_dsp::window::Window;
use sdrkit_util_ids::tones::ctcss::{self, Tone};
use std::f64::consts::{PI, TAU};

/// Tones closer than this to the best tone do not count against it, Hz.
const NEIGHBOUR_HZ: f32 = 4.5;

/// The settings of [`CtcssDecoder`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CtcssConfig {
    /// Listen for the 38 EIA tones only (otherwise all 50).
    pub eia_only: bool,
    /// The analysis window, s (0.3 to 4).
    pub window_s: f64,
    /// The time between analyses, s (0.02 up to the window).
    pub hop_s: f64,
    /// The smallest share of the sub-audio power for a candidate (0 to 1).
    pub min_share: f64,
    /// How many times stronger than every tone more than 4.5 Hz away a candidate must be (1 or more).
    pub dominance: f64,
    /// The smallest tone level, dB relative to a full-scale sine.
    pub min_level_db: f32,
    /// Candidates in a row before a lock (1 or more).
    pub lock_hops: u32,
    /// Hops in a row without the tone before a release (1 or more).
    pub release_hops: u32,
    /// Look for the reverse burst (phase step) turn-off.
    pub turn_off: bool,
}

impl Default for CtcssConfig {
    /// A 1 s window every 0.1 s, share 0.12, dominance 3, level -50 dB, lock after 4 hops, release after 8, turn-off on. These
    /// are the values two independent decoders used on air.
    fn default() -> Self {
        CtcssConfig {
            eia_only: false,
            window_s: 1.0,
            hop_s: 0.1,
            min_share: 0.12,
            dominance: 3.0,
            min_level_db: -50.0,
            lock_hops: 4,
            release_hops: 8,
            turn_off: true,
        }
    }
}

/// What happened to a tone.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum CtcssEventKind {
    /// The tone locked.
    Locked,
    /// The tone stopped (no turn-off seen).
    Released,
    /// The tone ended with a phase step (reverse burst). The tone is released.
    TurnOff,
}

/// A change of the locked tone.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CtcssEvent {
    /// What happened.
    pub kind: CtcssEventKind,
    /// The tone (from `sdrkit_util_ids::tones::ctcss::TONES`).
    pub tone: Tone,
    /// The input sample (counted from the decoder's start) at which the decoder made the decision.
    pub at: u64,
    /// The measured frequency, Hz.
    pub measured_hz: f32,
    /// The tone level, dB relative to a full-scale sine.
    pub level_db: f32,
}

#[derive(Clone, Copy, Debug)]
struct Lock {
    k: usize,
    misses: u32,
    hz: f32,
    level_db: f32,
    /// The tone amplitude when it locked.
    amp: f32,
}

/// The CTCSS decoder for real audio at 1000 Hz or more. Every `hop_s` it measures each tone over the last `window_s` (Hann
/// window). A tone is a candidate when its share of the sub-audio power is at least `min_share`, its power is `dominance`
/// times that of every tone more than 4.5 Hz away, and its level is at least `min_level_db`. It locks after `lock_hops`
/// candidates and releases after `release_hops` hops without it. A phase step of 90 degrees or more of a locked tone is a
/// turn-off (the reverse burst). The events do not depend on how the input is split across calls.
///
/// ```
/// use sdrkit_proto_tones::ctcss::{ctcss_sequence, CtcssConfig, CtcssDecoder, CtcssEventKind};
/// let rate = 8000.0;
/// let mut gen = ctcss_sequence(rate, 100.0, 0.15, 3.0, Some((0.5, 0.2))).unwrap();
/// let mut audio = vec![0f32; gen.len() as usize + 8000];
/// gen.fill(&mut audio);
/// let mut d = CtcssDecoder::new(rate, CtcssConfig::default()).unwrap();
/// let mut kinds = Vec::new();
/// d.process(&audio, |e| kinds.push((e.kind, e.tone.hz)));
/// assert_eq!(kinds, [(CtcssEventKind::Locked, 100.0), (CtcssEventKind::TurnOff, 100.0)]);
/// ```
#[derive(Clone, Debug)]
pub struct CtcssDecoder {
    cfg: CtcssConfig,
    sub: SubAudio,
    an: Analyser,
    tones: Vec<Tone>,
    /// The power of every tone in the last hop, dB relative to a full-scale sine.
    levels: Vec<f32>,
    cand: Option<usize>,
    count: u32,
    lock: Option<Lock>,
    holdoff: u32,
    input_pos: u64,
    // The phase follower.
    phase: Option<Goertzel>,
    last: Option<Complex32>,
    dev_prev: f64,
    drift: f64,
    windows: u32,
    pending: Option<CtcssEvent>,
}

impl CtcssDecoder {
    /// A decoder for audio at `rate` Hz (1000 or more). Errors for a bad rate or setting.
    pub fn new(rate: f64, cfg: CtcssConfig) -> Result<Self, TonesError> {
        let ok = (0.3..=4.0).contains(&cfg.window_s)
            && cfg.hop_s >= 0.02
            && cfg.hop_s <= cfg.window_s
            && (0.0..=1.0).contains(&cfg.min_share)
            && cfg.dominance >= 1.0
            && cfg.min_level_db.is_finite()
            && cfg.lock_hops >= 1
            && cfg.release_hops >= 1;
        if !ok {
            return Err(TonesError::BadParameter(Param::Ctcss));
        }
        let sub = SubAudio::new(rate)?;
        let srate = sub.rate();
        let tones: Vec<Tone> = ctcss::TONES
            .iter()
            .filter(|t| t.eia || !cfg.eia_only)
            .copied()
            .collect();
        let freqs: Vec<f64> = tones.iter().map(|t| t.hz as f64 / srate).collect();
        let len = (cfg.window_s * srate).round() as usize;
        let hop = ((cfg.hop_s * srate).round() as usize).clamp(1, len);
        let an = Analyser::new(&freqs, len, hop, Window::Hann, true)?;
        Ok(CtcssDecoder {
            cfg,
            sub,
            an,
            levels: vec![-200.0; tones.len()],
            tones,
            cand: None,
            count: 0,
            lock: None,
            holdoff: 0,
            input_pos: 0,
            phase: None,
            last: None,
            dev_prev: 0.0,
            drift: 0.0,
            windows: 0,
            pending: None,
        })
    }

    /// The tones listened for.
    pub fn tones(&self) -> &[Tone] {
        &self.tones
    }

    /// The level of every tone in the last analysis, dB relative to a full-scale sine (in the order of [`Self::tones`]).
    pub fn levels(&self) -> &[f32] {
        &self.levels
    }

    /// The locked tone, its measured frequency (Hz) and its level (dB).
    pub fn locked(&self) -> Option<(Tone, f32, f32)> {
        self.lock.map(|l| (self.tones[l.k], l.hz, l.level_db))
    }

    /// Forgets everything (no event for a tone that was locked).
    pub fn reset(&mut self) {
        self.sub.reset();
        self.an.reset();
        self.cand = None;
        self.count = 0;
        self.lock = None;
        self.holdoff = 0;
        self.phase = None;
        self.last = None;
    }

    /// Counts `n` samples without reading them (another signal holds the channel). The position stays right for later events.
    pub fn skip(&mut self, n: usize) {
        self.input_pos += n as u64;
    }

    /// The position (input samples since the start) of the next sample.
    pub fn position(&self) -> u64 {
        self.input_pos
    }

    /// Takes the next audio samples and calls `on_event` for every change.
    pub fn process(&mut self, x: &[f32], mut on_event: impl FnMut(CtcssEvent)) {
        for &v in x {
            self.input_pos += 1;
            let Some(s) = self.sub.push(v) else { continue };
            self.follow_phase(s);
            if self.an.push(s) {
                self.decide();
            }
            if let Some(e) = self.pending.take() {
                on_event(e);
            }
        }
    }

    /// The phase follower: one Goertzel window per hop at the locked tone.
    fn follow_phase(&mut self, s: f32) {
        if !self.cfg.turn_off {
            return;
        }
        let (Some(g), Some(l)) = (self.phase.as_mut(), self.lock) else {
            return;
        };
        let Some(z) = g.push(s) else { return };
        let n = g.block() as f64;
        let expect = TAU * self.tones[l.k].hz as f64 / self.sub.rate() * n;
        self.windows += 1;
        if let Some(prev) = self.last {
            let d = wrap(((z * prev.conj()).arg() as f64) - expect);
            let dev = wrap(d - self.drift);
            let strong = z.norm() >= 0.5 * l.amp && prev.norm() >= 0.25 * l.amp;
            if strong && self.windows > 3 && wrap(dev + self.dev_prev).abs() >= 0.5 * PI {
                self.release(CtcssEventKind::TurnOff);
                self.holdoff = (self.an.len() / self.an.hop()) as u32 + 1;
                return;
            }
            // A small deviation is a frequency offset: follow it slowly.
            if dev.abs() < 0.25 * PI {
                self.drift = wrap(self.drift + 0.3 * dev);
            }
            self.dev_prev = dev;
        }
        self.last = Some(z);
    }

    fn release(&mut self, kind: CtcssEventKind) {
        if let Some(l) = self.lock.take() {
            self.pending = Some(CtcssEvent {
                kind,
                tone: self.tones[l.k],
                at: self.input_pos,
                measured_hz: l.hz,
                level_db: l.level_db,
            });
        }
        self.phase = None;
        self.last = None;
        self.cand = None;
        self.count = 0;
    }

    fn decide(&mut self) {
        let srate = self.sub.rate();
        let hop = self.an.last();
        for (l, &p) in self.levels.iter_mut().zip(hop.power) {
            *l = power_db(p);
        }
        if self.holdoff > 0 {
            self.holdoff -= 1;
            return;
        }
        let best = (0..hop.power.len()).max_by(|&a, &b| hop.power[a].total_cmp(&hop.power[b]));
        let cand = best.filter(|&b| {
            let f = self.tones[b].hz;
            let other = self
                .tones
                .iter()
                .zip(hop.power)
                .filter(|(t, _)| (t.hz - f).abs() > NEIGHBOUR_HZ)
                .map(|(_, &p)| p)
                .fold(0.0, f64::max);
            hop.share(b) >= self.cfg.min_share
                && hop.power[b] >= self.cfg.dominance * other
                && hop.power[b] >= db_power(self.cfg.min_level_db)
        });
        // The measured frequency and level of the candidate: the strongest of 11 points 0.5 Hz apart around the tone, then a
        // parabola on the log powers of that point and its neighbours.
        let measured = cand.map(|k| {
            let f = self.tones[k].hz as f64;
            let d = 0.5;
            let mut p = [0f64; 13];
            for (i, v) in p.iter_mut().enumerate() {
                *v = self.an.power_at((f + (i as f64 - 6.0) * d) / srate);
            }
            // The best inner point, so it has two neighbours.
            let i = (1..12).max_by(|&a, &b| p[a].total_cmp(&p[b])).unwrap_or(6);
            let pk = log_parabolic(p[i - 1], p[i], p[i + 1]);
            let amp = (2.0 * hop.power[k]).sqrt() as f32;
            (
                (f + (i as f64 - 6.0 + pk.offset) * d) as f32,
                power_db(pk.value),
                amp,
            )
        });
        let hop_len = self.an.hop();
        if cand.is_some() && cand == self.cand {
            self.count += 1;
        } else {
            self.cand = cand;
            self.count = cand.is_some() as u32;
        }
        match self.lock {
            Some(mut l) => {
                if let (Some(k), Some((hz, level_db, _))) = (cand, measured) {
                    if k == l.k {
                        l.misses = 0;
                        l.hz = hz;
                        l.level_db = level_db;
                        self.lock = Some(l);
                        return;
                    }
                }
                l.misses += 1;
                self.lock = Some(l);
                if l.misses >= self.cfg.release_hops {
                    self.release(CtcssEventKind::Released);
                }
            }
            None => {
                let Some(k) = cand.filter(|_| self.count >= self.cfg.lock_hops) else {
                    return;
                };
                let Some((hz, level_db, amp)) = measured else {
                    return;
                };
                self.lock = Some(Lock {
                    k,
                    misses: 0,
                    hz,
                    level_db,
                    amp,
                });
                self.phase = Goertzel::new(self.tones[k].hz as f64 / srate, hop_len).ok();
                self.last = None;
                self.dev_prev = 0.0;
                self.drift = 0.0;
                self.windows = 0;
                self.pending = Some(CtcssEvent {
                    kind: CtcssEventKind::Locked,
                    tone: self.tones[k],
                    at: self.input_pos,
                    measured_hz: hz,
                    level_db,
                });
            }
        }
    }
}

/// An angle wrapped to `[-pi, pi)`.
fn wrap(a: f64) -> f64 {
    (a + PI).rem_euclid(TAU) - PI
}

/// The audio of a CTCSS tone: `hz` at amplitude `amp` for `seconds`, then (with `burst = Some((cycles, seconds))`) the same tone
/// with its phase shifted by `cycles` (0.5 is the 180 degree reverse burst) for that time. A 5 ms ramp shapes the start and the
/// end. Errors for a tone outside the audio band of the rate.
pub fn ctcss_sequence(
    rate: f64,
    hz: f64,
    amp: f32,
    seconds: f64,
    burst: Option<(f64, f64)>,
) -> Result<ToneSequence, TonesError> {
    let mut steps = [Step::one(hz, amp, seconds), Step::silence(0.0)];
    if let Some((cycles, s)) = burst {
        steps[1] = Step {
            phase_step: [cycles, 0.0],
            ..Step::one(hz, amp, s)
        };
    }
    ToneSequence::new(rate, &steps, 0.005)
}
