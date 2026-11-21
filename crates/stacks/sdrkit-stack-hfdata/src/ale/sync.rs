//! The ALE tone meter and word synchronisation: eight tone powers every 2 audio samples, a candidate word at
//! every instant, acquisition, tracking, the AFC and the word SNR.

use super::word::{self, Decoded};
use sdrkit_core::ConfigError;
use sdrkit_graph::{
    no_values, Component, ComponentInfo, Format, Io, Item, Kind, Port, TagKind, Timed,
};
use serde::{Deserialize, Serialize};
use std::f64::consts::TAU;

/// Audio samples per symbol (8 ms at 8 kHz).
const SYM: usize = 64;
/// Instants per symbol (one every 2 audio samples).
const IPS: u64 = 32;
/// Instants per word (49 symbols).
const TW: u64 = 49 * IPS;
const AUDIO_RING: usize = 8192;
const INST_RING: usize = 4096;

/// One instant of the tone meter (`ale.tones`).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct AleTone {
    /// The strongest tone (0 = 750 Hz .. 7 = 2500 Hz).
    pub tone: u8,
    /// Its tribit.
    pub tribit: u8,
    /// The strongest power over the sum of the eight.
    pub quality: f32,
    /// The strongest power.
    pub max_power: f32,
    /// The mean of the other seven.
    pub mean_other: f32,
}

impl Item for AleTone {
    const KIND: &'static str = "ale.tones";
}

/// A decoded word (`ale.word`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AleWord {
    /// Signal time of its first symbol, seconds.
    pub time: f64,
    /// Word type 0 to 7.
    pub wtype: u8,
    /// The 24 bits.
    pub bits: u32,
    /// The three characters.
    pub chars: [u8; 3],
    /// Vote errors (0 to 49).
    pub votes: u32,
    /// Golay corrections.
    pub golay: u32,
    /// The stuff bit was 0.
    pub stuff0: bool,
    /// SNR in 3 kHz, dB.
    pub snr_db: f64,
}

/// The word synchroniser's state, sent with every report.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct SyncStatus {
    /// In word sync (else searching).
    pub in_sync: bool,
    /// The AFC tone offset, Hz.
    pub afc_hz: f64,
    /// Words decoded, words missed in sync.
    pub words: u64,
    pub missed: u64,
    /// Mean in-band power since the last report, dBFS.
    pub level_dbfs: f64,
    /// Signal time, seconds.
    pub time: f64,
}

/// What the word synchroniser sends (`ale.word`): words, transmission marks and its status.
#[derive(Clone, Debug, PartialEq)]
pub enum AleSync {
    /// A transmission started (its first word follows).
    Start(f64),
    /// A word.
    Word(AleWord),
    /// The transmission ended (two words missed); the AFC at the end.
    End { time: f64, afc_hz: f64 },
    /// A retune: drop the transmission in progress; the new dial (Hz, 0 when unknown).
    Reset(f64),
    /// The periodic status.
    Status(SyncStatus),
}

impl Item for AleSync {
    const KIND: &'static str = "ale.word";
}

/// The tone meter: the power of the eight tones over the newest 64 samples, at the tone frequencies plus the AFC offset.
pub struct ToneMeter {
    coef: Vec<(f32, f32)>,
    coef_afc: f64,
}

impl Default for ToneMeter {
    fn default() -> Self {
        let mut m = ToneMeter {
            coef: vec![(0.0, 0.0); 8 * SYM],
            coef_afc: f64::NAN,
        };
        m.set_afc(0.0);
        m
    }
}

impl ToneMeter {
    /// Recomputes the DFT coefficients when the AFC moved by more than 1 Hz.
    pub fn set_afc(&mut self, afc: f64) {
        if (afc - self.coef_afc).abs() <= 1.0 {
            return;
        }
        self.coef_afc = afc;
        for k in 0..8 {
            let f = 750.0 + 250.0 * k as f64 + afc;
            for m in 0..SYM {
                let p = TAU * f * m as f64 / 8000.0;
                self.coef[k * SYM + m] = (p.cos() as f32, -p.sin() as f32);
            }
        }
    }
    /// The eight powers of a 64-sample window.
    pub fn powers(&self, w: &[f32]) -> [f32; 8] {
        let mut p = [0f32; 8];
        for (k, pk) in p.iter_mut().enumerate() {
            let c = &self.coef[k * SYM..(k + 1) * SYM];
            let (mut re, mut im) = (0f32, 0f32);
            for (x, (a, b)) in w.iter().zip(c) {
                re += x * a;
                im += x * b;
            }
            *pk = re * re + im * im;
        }
        p
    }
    /// One instant from a window.
    pub fn measure(&self, w: &[f32]) -> AleTone {
        let p = self.powers(w);
        let (mut best, mut bp, mut sum) = (0usize, -1f32, 0f32);
        for (k, &v) in p.iter().enumerate() {
            sum += v;
            if v > bp {
                bp = v;
                best = k;
            }
        }
        AleTone {
            tone: best as u8,
            tribit: word::tribit(best as u8),
            quality: if sum > 0.0 { bp / sum } else { 0.0 },
            max_power: bp,
            mean_other: (sum - bp) / 7.0,
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct Cand {
    t: u64,
    d: Decoded,
    metric: f64,
}

enum State {
    Search {
        window: Option<(u64, Cand)>,
    },
    Track {
        expected: u64,
        best: Option<Cand>,
        misses: u32,
    },
}

/// The word synchroniser as a plain object.
pub struct WordSync {
    meter: ToneMeter,
    audio: Vec<f32>,
    n: u64,
    inst: Vec<AleTone>,
    ni: u64,
    state: State,
    afc: f64,
    /// Counters.
    pub words: u64,
    pub missed: u64,
    level_acc: f64,
    level_n: u64,
    last_level: f64,
}

impl Default for WordSync {
    fn default() -> Self {
        WordSync {
            meter: ToneMeter::default(),
            audio: vec![0.0; 2 * AUDIO_RING],
            n: 0,
            inst: vec![AleTone::default(); INST_RING],
            ni: 0,
            state: State::Search { window: None },
            afc: 0.0,
            words: 0,
            missed: 0,
            level_acc: 0.0,
            level_n: 0,
            last_level: -120.0,
        }
    }
}

impl WordSync {
    /// Drops sync and the AFC (a retune); counters stay.
    pub fn reset(&mut self) {
        self.state = State::Search { window: None };
        self.afc = 0.0;
        self.meter.set_afc(0.0);
    }
    /// In word sync.
    pub fn in_sync(&self) -> bool {
        matches!(self.state, State::Track { .. })
    }
    /// The AFC offset.
    pub fn afc(&self) -> f64 {
        self.afc
    }
    fn window_ending(&self, sample: u64) -> &[f32] {
        // The 64 samples ending at absolute sample index `sample` (inclusive).
        let end = (sample % AUDIO_RING as u64) as usize + 1;
        let start = end + AUDIO_RING - SYM;
        &self.audio[start..start + SYM]
    }
    fn candidate(&self, t: u64) -> Option<Cand> {
        if t < 48 * IPS || !(self.ni - t <= INST_RING as u64 - TW) {
            return None;
        }
        let mut tb = [0u8; 49];
        let mut q = 0f64;
        for (s, v) in tb.iter_mut().enumerate() {
            let it = self.inst[((t - 48 * IPS + s as u64 * IPS) % INST_RING as u64) as usize];
            *v = it.tribit;
            q += it.quality as f64;
        }
        let d = word::decode_tribits(&tb);
        let metric = q - 1.5 * (d.votes + d.golay) as f64 + if d.stuff0 { 0.5 } else { 0.0 };
        Some(Cand { t, d, metric })
    }
    fn word_snr(&self, t: u64) -> f64 {
        let (mut s, mut n) = (0f64, 0f64);
        for k in 0..49u64 {
            let it = self.inst[((t - 48 * IPS + k * IPS) % INST_RING as u64) as usize];
            s += it.max_power as f64;
            n += it.mean_other as f64;
        }
        let r = if n > 0.0 {
            (s - n) / n * 125.0 / 3000.0
        } else {
            1e3
        };
        10.0 * r.max(1e-3).log10()
    }
    /// The AFC estimate from the word ending at instant `t` (its own tones, +-60 Hz in 10 Hz steps).
    fn afc_estimate(&self, t: u64) -> Option<f64> {
        // The oldest sample needed must still be in the ring.
        let first_sample = 2 * (t - 48 * IPS) + 1;
        if self.n.saturating_sub(first_sample) + SYM as u64 >= AUDIO_RING as u64 {
            return None;
        }
        let mut p = [0f64; 13];
        for (di, pd) in p.iter_mut().enumerate() {
            let d = -60.0 + 10.0 * di as f64;
            for s in 0..49u64 {
                let ti = t - 48 * IPS + s * IPS;
                let it = self.inst[(ti % INST_RING as u64) as usize];
                let f = 750.0 + 250.0 * it.tone as f64 + self.afc + d;
                let w = self.window_ending(2 * ti + 1);
                let step = TAU * f / 8000.0;
                let (sr, cr) = step.sin_cos();
                let (mut c, mut sn) = (1f64, 0f64);
                let (mut re, mut im) = (0f64, 0f64);
                for &x in w {
                    re += x as f64 * c;
                    im -= x as f64 * sn;
                    let nc = c * cr - sn * sr;
                    sn = sn * cr + c * sr;
                    c = nc;
                }
                *pd += re * re + im * im;
            }
        }
        let (mut bi, mut bv) = (0usize, -1f64);
        for (i, &v) in p.iter().enumerate() {
            if v > bv {
                bv = v;
                bi = i;
            }
        }
        let frac = if bi > 0 && bi < 12 {
            sdrkit_algo_dsp::peak::parabolic(p[bi - 1], p[bi], p[bi + 1]).offset
        } else {
            0.0
        };
        Some(-60.0 + 10.0 * (bi as f64 + frac))
    }

    fn accept(&mut self, c: Cand, out: &mut Vec<AleSync>, starting: bool) {
        let snr = self.word_snr(c.t);
        if let Some(est) = self.afc_estimate(c.t) {
            self.afc = (self.afc + est / 2.0).clamp(-150.0, 150.0);
            self.meter.set_afc(self.afc);
        }
        let end_time = (2 * c.t + 2) as f64 / 8000.0;
        let time = end_time - 0.392;
        if starting {
            out.push(AleSync::Start(time));
        }
        self.words += 1;
        out.push(AleSync::Word(AleWord {
            time,
            wtype: word::wtype(c.d.word),
            bits: c.d.word,
            chars: word::chars(c.d.word),
            votes: c.d.votes,
            golay: c.d.golay,
            stuff0: c.d.stuff0,
            snr_db: snr,
        }));
    }

    fn step_instant(&mut self, out: &mut Vec<AleSync>) {
        let t = self.ni - 1;
        let Some(c) = self.candidate(t) else {
            return;
        };
        let plausible = c.d.good && word::plausible(c.d.word);
        match &mut self.state {
            State::Search { window } => {
                let acq = plausible && c.d.votes <= 10 && c.d.golay <= 2 && c.d.stuff0;
                match window {
                    None => {
                        if acq {
                            *window = Some((t, c));
                        }
                    }
                    Some((start, best)) => {
                        if acq && c.metric > best.metric {
                            *best = c;
                        }
                        // The window spans two symbol times: a candidate one symbol early (one noise tone in front, the
                        // rest of the word shifted) can pass the checks by chance, and the true word must still win.
                        if t >= *start + 2 * IPS {
                            let b = *best;
                            self.state = State::Track {
                                expected: b.t + TW,
                                best: None,
                                misses: 0,
                            };
                            self.accept(b, out, true);
                        }
                    }
                }
            }
            State::Track {
                expected,
                best,
                misses,
            } => {
                if t + 4 >= *expected
                    && t <= *expected + 4
                    && plausible
                    && c.d.votes <= 24
                    && best.is_none_or(|b| c.metric > b.metric)
                {
                    *best = Some(c);
                }
                if t >= *expected + 4 {
                    if let Some(b) = best.take() {
                        *expected = b.t + TW;
                        *misses = 0;
                        self.accept(b, out, false);
                    } else {
                        *misses += 1;
                        self.missed += 1;
                        *expected += TW;
                        if *misses >= 2 {
                            let afc = self.afc;
                            self.state = State::Search { window: None };
                            out.push(AleSync::End {
                                time: (2 * t + 2) as f64 / 8000.0,
                                afc_hz: afc,
                            });
                        }
                    }
                }
            }
        }
    }

    /// Feeds audio (and its in-band power), appending what happened.
    pub fn process(
        &mut self,
        audio: &[f32],
        power: &[f32],
        tones: Option<&mut Vec<AleTone>>,
        out: &mut Vec<AleSync>,
    ) {
        let mut tones = tones;
        for (i, &x) in audio.iter().enumerate() {
            let k = (self.n % AUDIO_RING as u64) as usize;
            self.audio[k] = x;
            self.audio[k + AUDIO_RING] = x;
            self.n += 1;
            if let Some(&p) = power.get(i) {
                if p.is_finite() {
                    self.level_acc += p as f64;
                    self.level_n += 1;
                }
            }
            if self.n.is_multiple_of(2) {
                let it = self.meter.measure(self.window_ending(self.n - 1));
                self.inst[(self.ni % INST_RING as u64) as usize] = it;
                self.ni += 1;
                if let Some(t) = tones.as_deref_mut() {
                    t.push(it);
                }
                self.step_instant(out);
            }
        }
    }

    /// The status for a report (the level is the mean since the last report).
    pub fn status(&mut self) -> SyncStatus {
        if self.level_n > 0 {
            self.last_level = 10.0 * (self.level_acc / self.level_n as f64).max(1e-15).log10();
        }
        self.level_acc = 0.0;
        self.level_n = 0;
        SyncStatus {
            in_sync: self.in_sync(),
            afc_hz: self.afc,
            words: self.words,
            missed: self.missed,
            level_dbfs: self.last_level,
            time: self.n as f64 / 8000.0,
        }
    }
}

/// The word synchroniser as a component: `Real` 8 kHz audio and its in-band power in; `ale.word` and `ale.tones` out.
#[derive(Default)]
pub struct AleWordSync {
    sync: WordSync,
    scratch: Vec<AleSync>,
    tones: Vec<AleTone>,
}

impl Component for AleWordSync {
    fn values(&self) -> sdrkit_core::Values {
        no_values()
    }
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "hfdata.ale_word_sync",
            vec![Port::real("audio"), Port::real("power")],
            vec![
                Port::typed::<AleSync>("words"),
                Port::typed::<AleTone>("tones"),
            ],
        )
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        self.sync.reset();
        let mut a = Format::new(Kind::typed::<AleSync>(), 0.0);
        a.tuned_hz = inputs[0].tuned_hz;
        a.centre_hz = inputs[0].centre_hz;
        let mut b = a.clone();
        b.kind = Kind::typed::<AleTone>();
        Ok(vec![a, b])
    }
    fn process(&mut self, io: &mut Io) {
        let audio = io.real(0);
        let power = io.real(1);
        let first = io.input(0).first();
        let mut out = std::mem::take(&mut self.scratch);
        out.clear();
        self.tones.clear();
        let mut start = 0usize;
        for t in io.input(0).tags() {
            if let TagKind::Retuned { tuned_hz, .. } = t.kind {
                let at = (t.at.saturating_sub(first) as usize).min(audio.len());
                self.sync.process(
                    &audio[start..at],
                    power.get(start..at).unwrap_or(&[]),
                    Some(&mut self.tones),
                    &mut out,
                );
                start = at;
                self.sync.reset();
                out.push(AleSync::Reset(tuned_hz));
            }
        }
        self.sync.process(
            &audio[start..],
            power.get(start..).unwrap_or(&[]),
            Some(&mut self.tones),
            &mut out,
        );
        if io.tick() {
            let st = self.sync.status();
            out.push(AleSync::Status(st));
        }
        let now = self.sync.n as f64 / 8000.0;
        let o = io.items_out::<AleSync>(0);
        for e in out.drain(..) {
            let time = match &e {
                AleSync::Word(w) => w.time,
                AleSync::Start(t) => *t,
                AleSync::End { time, .. } => *time,
                _ => now,
            };
            o.push(Timed { time, value: e });
        }
        self.scratch = out;
        let o = io.items_out::<AleTone>(1);
        for (i, t) in self.tones.iter().enumerate() {
            o.push(Timed {
                time: now - (self.tones.len() - i) as f64 / 4000.0,
                value: *t,
            });
        }
    }
}
