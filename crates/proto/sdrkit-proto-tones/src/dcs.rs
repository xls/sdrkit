//! DCS (digital coded squelch, a 23-bit word sent again and again under the voice at 134.4 bit/s): the decoder and the
//! generator. The codes and their readings are in `sdrkit_util_ids::tones::dcs`.
//!
//! Standard: TIA-603

use crate::analyse::power_db;
use crate::subaudio::SubAudio;
use crate::{Param, TonesError};
use sdrkit_algo_dsp::goertzel::goertzel_windowed;
use sdrkit_algo_dsp::iir::OnePole;
use sdrkit_algo_dsp::nco::Nco;
use sdrkit_util_ids::tones::dcs::{self, Dcs, Reading};

/// Bits in the three words an analysis reads.
const BITS: usize = 69;
/// Timing phases tried per bit.
const PHASES: usize = 12;

/// The settings of [`DcsDecoder`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DcsConfig {
    /// The time between analyses, s (0.02 to 0.5).
    pub hop_s: f64,
    /// The smallest distance between the high and the low level, as a fraction of full scale (more than 0).
    pub min_swing: f32,
    /// The time without a reading before a release, s (0.5 to 10).
    pub release_s: f64,
    /// Look for the 134 Hz turn-off code.
    pub turn_off: bool,
}

impl Default for DcsConfig {
    /// An analysis every 64 ms, a swing of 0.01, a release after 1.5 s, the turn-off on.
    fn default() -> Self {
        DcsConfig {
            hop_s: 0.064,
            min_swing: 0.01,
            release_s: 1.5,
            turn_off: true,
        }
    }
}

/// What happened to a code.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum DcsEventKind {
    /// The code locked.
    Locked,
    /// The code stopped (no turn-off code seen).
    Released,
    /// The turn-off code came. The code is released.
    TurnOff,
}

/// A change of the locked code.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DcsEvent {
    /// What happened.
    pub kind: DcsEventKind,
    /// The reading: the code, its polarity and its alias.
    pub reading: Reading,
    /// The input sample (counted from the decoder's start) at which the decoder made the decision.
    pub at: u64,
    /// The level of the bit stream, dB relative to a full-scale sine (from half the distance between the levels).
    pub level_db: f32,
}

/// The DCS decoder for real audio at 1000 Hz or more. Every `hop_s` it slices the last three words of the sub-audio band at
/// twelve timing phases per bit, takes the majority of the three words, and reads it at all rotations and both polarities. A
/// code locks when two analyses in a row read it, and releases when no analysis reads it for 1.5 s. A 134 Hz tone during a
/// lock is the turn-off code. The decoder does not correct the Golay word. The events do not depend on how the input is split
/// across calls.
///
/// ```
/// use sdrkit_util_ids::tones::dcs::Dcs;
/// use sdrkit_proto_tones::dcs::{DcsConfig, DcsDecoder, DcsEncoder, DcsEventKind};
/// let rate = 8000.0;
/// let mut enc = DcsEncoder::new(rate, Dcs::from_octal("023").unwrap(), false, 0.2, 2.0, true).unwrap();
/// let mut audio = vec![0f32; enc.len() as usize + 16000];
/// enc.fill(&mut audio);
/// let mut d = DcsDecoder::new(rate, DcsConfig::default()).unwrap();
/// let mut got = Vec::new();
/// d.process(&audio, |e| got.push((e.kind, e.reading.name())));
/// assert_eq!(got, [(DcsEventKind::Locked, "D023N".to_string()), (DcsEventKind::TurnOff, "D023N".to_string())]);
/// ```
#[derive(Clone, Debug)]
pub struct DcsDecoder {
    cfg: DcsConfig,
    sub: SubAudio,
    ring: Vec<f32>,
    head: usize,
    filled: usize,
    work: Vec<f32>,
    sorted: Vec<f32>,
    window: Vec<f32>,
    hop: usize,
    since: usize,
    spb: f64,
    /// Sub-audio samples since the start.
    n: u64,
    input_pos: u64,
    lock: Option<(Reading, u64, f32)>,
    pending: Option<(Reading, u64)>,
    event: Option<DcsEvent>,
}

impl DcsDecoder {
    /// A decoder for audio at `rate` Hz (1000 or more). Errors for a bad rate or setting.
    pub fn new(rate: f64, cfg: DcsConfig) -> Result<Self, TonesError> {
        if !((0.02..=0.5).contains(&cfg.hop_s)
            && cfg.min_swing > 0.0
            && cfg.min_swing.is_finite()
            && (0.5..=10.0).contains(&cfg.release_s))
        {
            return Err(TonesError::BadParameter(Param::Dcs));
        }
        let sub = SubAudio::new(rate)?;
        let srate = sub.rate();
        let spb = srate / dcs::BIT_RATE as f64;
        let len = (BITS as f64 * spb).ceil() as usize + 2;
        let tlen = (0.15 * srate).round() as usize;
        let mut window = vec![0f32; tlen];
        sdrkit_algo_dsp::window::Window::Hann.fill_periodic(&mut window);
        Ok(DcsDecoder {
            cfg,
            ring: vec![0.0; len],
            head: 0,
            filled: 0,
            work: vec![0.0; len],
            sorted: vec![0.0; len],
            window,
            hop: ((cfg.hop_s * srate).round() as usize).max(1),
            since: 0,
            spb,
            n: 0,
            input_pos: 0,
            lock: None,
            pending: None,
            event: None,
            sub,
        })
    }

    /// The locked code's reading and its level (dB).
    pub fn locked(&self) -> Option<(Reading, f32)> {
        self.lock.map(|l| (l.0, l.2))
    }

    /// Forgets everything (no event for a code that was locked).
    pub fn reset(&mut self) {
        self.sub.reset();
        self.ring.fill(0.0);
        self.head = 0;
        self.filled = 0;
        self.since = 0;
        self.lock = None;
        self.pending = None;
    }

    /// The position (input samples since the start) of the next sample.
    pub fn position(&self) -> u64 {
        self.input_pos
    }

    /// Takes the next audio samples and calls `on_event` for every change.
    pub fn process(&mut self, x: &[f32], mut on_event: impl FnMut(DcsEvent)) {
        for &v in x {
            self.input_pos += 1;
            let Some(s) = self.sub.push(v) else { continue };
            self.ring[self.head] = s;
            self.head = (self.head + 1) % self.ring.len();
            self.filled = (self.filled + 1).min(self.ring.len());
            self.n += 1;
            self.since += 1;
            if self.since >= self.hop && self.filled == self.ring.len() {
                self.since = 0;
                self.analyse();
            }
            if let Some(e) = self.event.take() {
                on_event(e);
            }
        }
    }

    fn release(&mut self, kind: DcsEventKind) {
        if let Some((reading, _, level_db)) = self.lock.take() {
            self.event = Some(DcsEvent {
                kind,
                reading,
                at: self.input_pos,
                level_db,
            });
        }
    }

    fn analyse(&mut self) {
        let srate = self.sub.rate();
        let now = self.n;
        let secs = |a: u64, b: u64| (a - b) as f64 / srate;
        if self
            .lock
            .is_some_and(|l| secs(now, l.1) > self.cfg.release_s)
        {
            self.release(DcsEventKind::Released);
        }
        if self.pending.is_some_and(|p| secs(now, p.1) > 1.0) {
            self.pending = None;
        }
        let n = self.ring.len();
        let (a, b) = self.ring.split_at(self.head);
        self.work[..b.len()].copy_from_slice(b);
        self.work[b.len()..].copy_from_slice(a);
        if self.cfg.turn_off && self.lock.is_some() && self.turn_off() {
            self.release(DcsEventKind::TurnOff);
            self.pending = None;
            return;
        }
        self.sorted.copy_from_slice(&self.work);
        self.sorted.sort_unstable_by(|a, b| a.total_cmp(b));
        let tenth = (n / 10).max(1);
        let low = self.sorted[..tenth].iter().sum::<f32>() / tenth as f32;
        let high = self.sorted[n - tenth..].iter().sum::<f32>() / tenth as f32;
        if high - low < self.cfg.min_swing {
            return;
        }
        let mid = 0.5 * (high + low);
        let level_db = power_db((0.5 * (high - low) as f64).powi(2) / 2.0);
        let mut best: Option<(u32, Reading)> = None;
        for p in 0..PHASES {
            let ph = p as f64 * self.spb / PHASES as f64;
            let mut w = [0u32; 3];
            for k in 0..BITS {
                // The middle of bit k at this phase.
                let t = ph + (k as f64 + 0.5) * self.spb;
                let i = t.floor() as usize;
                let f = (t - i as f64) as f32;
                let v = self.work[i.min(n - 1)] * (1.0 - f) + self.work[(i + 1).min(n - 1)] * f;
                if v > mid {
                    w[k / 23] |= 1 << (k % 23);
                }
            }
            let vote = (w[0] & w[1]) | (w[0] & w[2]) | (w[1] & w[2]);
            let d = w.map(|x| (x ^ vote).count_ones());
            let total: u32 = d.iter().sum();
            if d.iter().filter(|&&x| x <= 1).count() >= 2 && total <= 3 {
                if let Some(r) = dcs::decode(vote) {
                    if best.is_none_or(|b| total < b.0) {
                        best = Some((total, r));
                    }
                }
            }
        }
        let Some((_, r)) = best else { return };
        match self.lock {
            Some((l, _, _)) if same(l, r) => self.lock = Some((l, now, level_db)),
            _ => {
                if self.pending.is_some_and(|p| same(p.0, r)) {
                    self.pending = None;
                    if self.lock.is_some() {
                        self.release(DcsEventKind::Released);
                        // Both events cannot leave in one sample: the new lock waits for the next analysis.
                        self.pending = Some((r, now));
                        return;
                    }
                    self.lock = Some((r, now, level_db));
                    self.event = Some(DcsEvent {
                        kind: DcsEventKind::Locked,
                        reading: r,
                        at: self.input_pos,
                        level_db,
                    });
                } else {
                    self.pending = Some((r, now));
                }
            }
        }
    }

    /// Whether the end of the work buffer holds the turn-off tone.
    fn turn_off(&self) -> bool {
        let m = self.window.len();
        let x = &self.work[self.work.len() - m..];
        let mean = x.iter().sum::<f32>() / m as f32;
        let (mut e, mut wsq, mut wsum) = (0.0f64, 0.0f64, 0.0f64);
        for (&v, &w) in x.iter().zip(&self.window) {
            let y = ((v - mean) * w) as f64;
            e += y * y;
            wsq += (w as f64) * (w as f64);
            wsum += w as f64;
        }
        if e <= 1e-20 {
            return false;
        }
        // Remove the mean on the fly: the tone lies far from DC, so the mean barely touches it.
        let z = goertzel_windowed(x, &self.window, dcs::TURN_OFF_HZ as f64 / self.sub.rate())
            * (2.0 / wsum);
        let p = z.norm_sqr() / 2.0;
        let total = e / wsq;
        p >= 0.5 * total
    }
}

/// Two readings of one code: the same code and polarity.
fn same(a: Reading, b: Reading) -> bool {
    a.code == b.code && a.inverted == b.inverted
}

/// The audio of a DCS code (module documentation). Streaming. It allocates nothing.
#[derive(Clone, Debug)]
pub struct DcsEncoder {
    word: u32,
    rate: f64,
    amp: f32,
    data_len: u64,
    total: u64,
    pos: u64,
    lp: [OnePole; 2],
    tone: Nco,
}

impl DcsEncoder {
    /// The code `code` (with `inverted`, the complement of its word) at amplitude `amp`, for `seconds`, then with `turn_off` 0.2
    /// s of the 134 Hz turn-off tone. The bits are low-passed by two one-pole filters at 300 Hz. Errors for a rate below 1000 Hz
    /// or a bad length.
    pub fn new(
        rate: f64,
        code: Dcs,
        inverted: bool,
        amp: f32,
        seconds: f64,
        turn_off: bool,
    ) -> Result<Self, TonesError> {
        if !(rate.is_finite() && rate >= 1000.0 && seconds.is_finite() && seconds >= 0.0)
            || !amp.is_finite()
        {
            return Err(TonesError::BadParameter(Param::DcsSignal));
        }
        let w = code.word();
        let data_len = (seconds * rate).round() as u64;
        let tail = if turn_off {
            (dcs::TURN_OFF_S as f64 * rate).round() as u64
        } else {
            0
        };
        Ok(DcsEncoder {
            word: if inverted { !w & 0x7F_FFFF } else { w },
            rate,
            amp,
            data_len,
            total: data_len + tail,
            pos: 0,
            lp: [OnePole::new(rate, 300.0), OnePole::new(rate, 300.0)],
            tone: Nco::new(rate, dcs::TURN_OFF_HZ as f64),
        })
    }

    /// The length, samples.
    pub fn len(&self) -> u64 {
        self.total
    }

    /// Whether the signal has no samples.
    pub fn is_empty(&self) -> bool {
        self.total == 0
    }

    /// Writes the next samples into `out` (zeros past the end). Returns how many were inside the signal.
    pub fn fill(&mut self, out: &mut [f32]) -> usize {
        let mut given = 0;
        for o in out.iter_mut() {
            if self.pos >= self.total {
                *o = 0.0;
                continue;
            }
            let v = if self.pos < self.data_len {
                let bit = ((self.pos as f64 * dcs::BIT_RATE as f64 / self.rate) as u64 % 23) as u32;
                let x = if (self.word >> bit) & 1 == 1 {
                    self.amp
                } else {
                    -self.amp
                };
                let y = self.lp[0].process_sample(x);
                self.lp[1].process_sample(y)
            } else {
                self.amp * self.tone.next_sample().re
            };
            *o = v;
            self.pos += 1;
            given += 1;
        }
        given
    }
}
