//! SITOR: the CCIR 476 seven-unit code at 100 baud with 170 Hz shift. The receiver finds the tone pair, slices both polarities,
//! finds the word phase and returns the words to the shared mode B receiver (SITOR-B, FEC). On the same unit decisions, an ARQ
//! monitor of each polarity reads SITOR-A / AMTOR links ([`arq`]).
//!
//! Standard: ITU-R M.476-5 (mode A, Annex 1 section 3) and M.625-4 section 4 (mode B).

pub mod arq;

use crate::decoder::{
    self, last_call, put_text, Heard, HfGenerator, Modulator, GEN_MESSAGE, GEN_SECTION,
};
use crate::decoder::{Emit as _, HeardCommand};
use crate::rtty::FskDemod;
use crate::typed::{SitorArqEvent, SitorFields, SITOR_ARQ};
use crate::util::{db, median, Tone};
use arq::ArqMonitor;
use sdrkit_algo_dsp::fft::WindowedFft;
use sdrkit_algo_dsp::window::Window;
use sdrkit_core::command::{Command, CommandError, CommandId, CommandSet};
use sdrkit_core::interface::Hertz;
use sdrkit_core::publish::{field, Published};
use sdrkit_core::Record;
use sdrkit_core::{
    Complex32, ConfigError, GenConfig, Generator, LockState, Setting, SignalInfo, Status, Values,
};
use sdrkit_graph::{no_values, Format, Kept, Outbox, Port};
use sdrkit_util_text::ccir476::{Fec, FecBDecoder, FecBEncoder, Word};
use sdrkit_util_text::store::TextStore;

const NFFT: usize = 4096;
const BAUD: f64 = 100.0;
const SHIFT: f64 = 170.0;
const WORDS: usize = 64;

/// The lower tone of the test signal.
pub const GEN_LOWER_HZ: Setting<field::Hz> =
    Setting::new("gen_lower_hz", "Lower tone", "Hz", 1615.0)
        .range(100.0, 3500.0, 0.0)
        .section(GEN_SECTION)
        .advanced();

sdrkit_core::settings! {
    /// The test-signal settings of the SITOR stack.
    pub enum SitorGenSetting {
        /// The text sent.
        Message = GEN_MESSAGE,
        /// The lower tone.
        LowerHz = GEN_LOWER_HZ,
    }
}

/// The SITOR-B receiver.
pub struct SitorDecoder {
    rate: f64,
    dial: f64,
    spec: WindowedFft,
    buf: Vec<Complex32>,
    avg: Vec<f32>,
    scratch: Vec<f32>,
    scratch2: Vec<f32>,
    frames: u64,
    found: bool,
    cand: Option<f64>,
    cand_n: u32,
    weak: u32,
    /// Spectrum frames in a row in which a pair away from the tones was much stronger.
    better: u32,
    snr_db: f32,
    demod: FskDemod,
    // bit clock
    phase: f64,
    prev_d: f32,
    /// Peak of |d|, decaying over about 50 units.
    peak: f32,
    /// Largest |d| since the last zero crossing.
    run_max: f32,
    /// A crossing waiting for a strong unit after it: the clock error at the crossing and the samples since.
    pending: Option<(f64, f64)>,
    t: u64,
    // words
    /// Soft units, positive for B (the higher tone).
    soft: Vec<f32>,
    nbits: u64,
    word_phase: usize,
    inverted: bool,
    locked: bool,
    fec: FecBDecoder,
    store: TextStore,
    good: u64,
    bad: u64,
    last_good: f64,
    heard: Heard,
    status: Status,
    level: f64,
    level_n: u64,
    /// The ARQ monitors: B on the higher tone, and inverted.
    arq: [ArqMonitor; 2],
    /// The last second of input, read again when the tones are found.
    history: Vec<Complex32>,
    hist_at: usize,
    replayed: bool,
}

impl SitorDecoder {
    /// A receiver.
    pub fn new() -> Self {
        SitorDecoder {
            rate: 24000.0,
            dial: 0.0,
            spec: WindowedFft::new(NFFT, NFFT, Window::Hann)
                .expect("a window that fits the transform"),
            buf: Vec::with_capacity(NFFT),
            avg: vec![0.0; NFFT],
            scratch: Vec::with_capacity(NFFT),
            scratch2: Vec::with_capacity(NFFT),
            frames: 0,
            found: false,
            cand: None,
            cand_n: 0,
            weak: 0,
            better: 0,
            snr_db: 0.0,
            demod: FskDemod::new(24000.0, BAUD).balanced(),
            phase: 0.0,
            prev_d: 0.0,
            peak: 0.0,
            run_max: 0.0,
            pending: None,
            t: 0,
            soft: vec![0.0; 7 * WORDS * 2],
            nbits: 0,
            word_phase: 0,
            inverted: false,
            locked: false,
            fec: {
                let mut f = FecBDecoder::new();
                f.repair = true;
                f
            },
            store: decoder::store(8000, 0),
            good: 0,
            bad: 0,
            last_good: f64::NEG_INFINITY,
            heard: Heard::new("SITOR"),
            status: Status::default(),
            level: 0.0,
            level_n: 0,
            arq: [ArqMonitor::new(), ArqMonitor::new()],
            history: Vec::new(),
            hist_at: 0,
            replayed: false,
        }
    }

    /// The ARQ monitor shown: the one that has read more blocks.
    fn arq_shown(&self) -> &ArqMonitor {
        if self.arq[1].blocks > self.arq[0].blocks {
            &self.arq[1]
        } else {
            &self.arq[0]
        }
    }

    fn drop_sync(&mut self) {
        self.found = false;
        self.locked = false;
        self.cand = None;
        self.cand_n = 0;
        self.avg.iter_mut().for_each(|a| *a = 0.0);
        self.buf.clear();
        self.frames = 0;
        self.demod = FskDemod::new(self.rate, BAUD).balanced();
        self.nbits = 0;
        self.fec.reset();
        self.arq.iter_mut().for_each(ArqMonitor::reset);
        self.hist_at = 0;
        self.replayed = false;
    }

    fn search(&mut self) {
        let p = self.spec.run_power(&self.buf);
        let a = if self.frames == 0 { 1.0 } else { 0.35 };
        for (s, &v) in self.avg.iter_mut().zip(p) {
            *s += a * (v - *s);
        }
        self.frames += 1;
        let n = NFFT as i64;
        let bin = self.rate / NFFT as f64;
        let half = ((5000.0f64).min(0.45 * self.rate) / bin) as i64;
        let at = |k: i64, avg: &[f32]| avg[k.rem_euclid(n) as usize];
        // Floor over the window.
        self.scratch.clear();
        for k in -half..half {
            self.scratch.push(at(k, &self.avg));
        }
        let floor = {
            let v = std::mem::take(&mut self.scratch);
            let m = median(&v, &mut self.scratch2);
            self.scratch = v;
            m.max(1e-20)
        };
        let s = (SHIFT / bin).round() as i64;
        let tone = |k: i64, avg: &[f32]| at(k - 1, avg) + at(k, avg) + at(k + 1, avg);
        let mut best = (0.0f32, 0i64);
        for k in -half + 2..half - s - 2 {
            let (l, u) = (tone(k, &self.avg), tone(k + s, &self.avg));
            if l < 6.0 * floor || u < 6.0 * floor {
                continue;
            }
            let sc = (l + u) / (6.0 * floor);
            if sc > best.0 {
                best = (sc, k);
            }
        }
        let score = db(best.0 as f64) as f32;
        let centroid = |c: i64, avg: &[f32]| -> f64 {
            let (mut num, mut den) = (0.0, 0.0);
            for k in c - 2..=c + 2 {
                let w = (at(k, avg) - floor).max(0.0) as f64;
                num += w * k as f64;
                den += w;
            }
            if den > 0.0 {
                num / den * bin
            } else {
                c as f64 * bin
            }
        };
        if best.0 > 0.0 {
            self.snr_db = score;
        }
        if self.found {
            // A pair away from the tones that is much stronger for several frames (the first pair was a sideband of keyed
            // bursts): move to it.
            let k_now = (self.demod.tones[0] / bin).round() as i64;
            let now = (tone(k_now, &self.avg) + tone(k_now + s, &self.avg)) / (6.0 * floor);
            if (best.1 - k_now).abs() > 3 && best.0 > 1.5 * now {
                self.better += 1;
                if self.better >= 3 {
                    self.better = 0;
                    self.demod
                        .set_tones([centroid(best.1, &self.avg), centroid(best.1 + s, &self.avg)]);
                    self.heard.logs.push(format!(
                        "FSK moved to {:.0} Hz",
                        0.5 * (self.demod.tones[0] + self.demod.tones[1])
                    ));
                    return;
                }
            } else {
                self.better = 0;
            }
            if score >= 10.0 {
                self.weak = 0;
                let f0 = centroid(best.1, &self.avg);
                let f1 = centroid(best.1 + s, &self.avg);
                let t = self.demod.tones;
                if (f0 - t[0]).abs() < 3.0 * bin {
                    self.demod
                        .set_tones([t[0] + 0.3 * (f0 - t[0]), t[1] + 0.3 * (f1 - t[1])]);
                }
            } else {
                self.weak += 1;
                if self.weak > 15 {
                    self.found = false;
                    self.locked = false;
                }
            }
            return;
        }
        if best.0 <= 0.0 || score < 10.0 {
            self.cand = None;
            return;
        }
        let f = best.1 as f64 * bin;
        match self.cand {
            Some(c) if (c - f).abs() <= 2.0 * bin => self.cand_n += 1,
            _ => {
                self.cand = Some(f);
                self.cand_n = 1;
            }
        }
        // A strong pair in 3 frames, a weaker one (the bursts of an ARQ link in noise) in 6, a very strong one at once.
        if (self.cand_n >= 3 && score >= 14.0) || self.cand_n >= 6 || score >= 24.0 {
            self.found = true;
            self.weak = 0;
            self.demod
                .set_tones([centroid(best.1, &self.avg), centroid(best.1 + s, &self.avg)]);
            self.heard.logs.push(format!(
                "FSK found at {:.0} Hz, shift {:.0} Hz",
                0.5 * (self.demod.tones[0] + self.demod.tones[1]),
                self.demod.tones[1] - self.demod.tones[0]
            ));
        }
    }

    /// One unit decision `d` (positive when the lower tone is stronger).
    fn bit(&mut self, d: f32, out: &mut Outbox) {
        let n = self.soft.len();
        // B is the higher emitted frequency (M.625-4 Table 1 note 3).
        self.soft[(self.nbits % n as u64) as usize] = -d;
        self.nbits += 1;
        let y = u8::from(d >= 0.0);
        self.arq[0].push(y);
        self.arq[1].push(y ^ 1);
        let shown = usize::from(self.arq[1].blocks > self.arq[0].blocks);
        if self.arq.iter().any(|m| !m.events.is_empty()) {
            let time = self.t as f64 / self.rate;
            let f = self.dial + 0.5 * (self.demod.tones[0] + self.demod.tones[1]);
            for (i, m) in self.arq.iter_mut().enumerate() {
                for e in m.events.drain(..) {
                    if i == shown {
                        let t = e.text();
                        self.heard.logs.push(format!("{time:.1} s: SITOR-A {t}"));
                        out.event(SITOR_ARQ.event(time, f, &SitorArqEvent { text: t.as_str() }));
                    }
                }
            }
        }
        // Every word time, judge the word phase and polarity over the last WORDS words: a valid word has four B units.
        if self.nbits.is_multiple_of(7) && self.nbits >= (7 * WORDS + 7) as u64 {
            let mut best = (0usize, 0usize, false);
            let mut current = 0usize;
            for ph in 0..7 {
                let (mut v, mut inv) = (0, 0);
                for w in 0..WORDS {
                    let start = self.nbits - (7 * (WORDS - w)) as u64 - ph as u64;
                    let mut b_units = 0;
                    for b in 0..7 {
                        if self.soft[((start + b) % n as u64) as usize] > 0.0 {
                            b_units += 1;
                        }
                    }
                    if b_units == 4 {
                        v += 1;
                    }
                    if b_units == 3 {
                        inv += 1;
                    }
                }
                if self.locked && ph == self.word_phase {
                    current = if self.inverted { inv } else { v };
                }
                if v > best.0 {
                    best = (v, ph, false);
                }
                if inv > best.0 {
                    best = (inv, ph, true);
                }
            }
            let ok = best.0 * 10 >= WORDS * 7;
            let better = !self.locked || best.0 > current + 4;
            if ok
                && better
                && (!self.locked || best.1 != self.word_phase || best.2 != self.inverted)
            {
                if !self.locked {
                    self.heard.logs.push("locked on a SITOR-B stream".into());
                }
                self.locked = true;
                self.word_phase = best.1;
                self.inverted = best.2;
                self.fec.reset();
            } else if !ok && best.0 * 10 < WORDS * 4 {
                self.locked = false;
            }
        }
        if !self.locked
            || !(self.nbits + self.word_phase as u64).is_multiple_of(7)
            || self.nbits < 7
        {
            return;
        }
        // A word ends here.
        let mut llr = [0.0f32; 7];
        for (b, l) in llr.iter_mut().enumerate() {
            let v = self.soft[((self.nbits - 7 + b as u64) % n as u64) as usize];
            *l = if self.inverted { -v } else { v };
        }
        if llr.iter().filter(|&&l| l > 0.0).count() == 4 {
            self.good += 1;
            self.last_good = self.t as f64 / self.rate;
        } else {
            self.bad += 1;
        }
        if let Some(Fec::Char(c)) = self.fec.push_soft(&llr) {
            if c.is_ascii() {
                let time = self.t as f64 / self.rate;
                let f = self.dial + 0.5 * (self.demod.tones[0] + self.demod.tones[1]);
                self.heard
                    .char(&mut self.store, out, c as u8, time, 100, 0, f, self.snr_db);
            }
        }
    }

    /// One sample through the demodulator and the unit clock.
    fn step(&mut self, s: Complex32, out: &mut Outbox) {
        // Samples a unit at the input rate: the clock runs at the true rate, not the rounded integration length.
        let n = self.rate / BAUD;
        let d = self.demod.step(s);
        // The clock follows the zero crossings of a signal: a crossing between two strong units (the decision reaches 0.3 of
        // its peak on both sides within a unit, with a tone keyed near the peak level) moves it. In the pauses of an ARQ link
        // it runs free, and a weaker second station keying on its own timing does not move it.
        let a = d.abs();
        self.peak = if a > self.peak {
            a
        } else {
            self.peak * (1.0 - 1.0 / (50.0 * n as f32))
        };
        let keyed = self.demod.presence() > 0.7;
        if (d > 0.0) != (self.prev_d > 0.0) {
            self.pending = (self.run_max > 0.3 * self.peak).then_some((0.5 - self.phase, 0.0));
            self.run_max = 0.0;
        }
        if let Some((err, age)) = self.pending {
            if a > 0.3 * self.peak && keyed {
                self.phase += 0.08 * err;
                self.pending = None;
            } else if age > n {
                self.pending = None;
            } else {
                self.pending = Some((err, age + 1.0));
            }
        }
        self.run_max = self.run_max.max(a);
        self.prev_d = d;
        self.phase += 1.0 / n;
        if self.phase >= 1.0 {
            self.phase -= 1.0;
            self.bit(d, out);
        }
    }

    /// When the tones are found: the kept history (the last second, which ends with the current sample) through the
    /// demodulator, so the units before the search found the tones are read too (the first blocks of an ARQ link).
    fn replay(&mut self, out: &mut Outbox) {
        let len = self.history.len();
        let kept = self.hist_at.min(len);
        let now = self.t;
        for i in 0..kept {
            let k = (self.hist_at - kept + i) % len;
            let s = self.history[k];
            self.t = now - (kept - 1 - i) as u64;
            self.step(s, out);
        }
        self.t = now;
    }

    fn state(&self) -> u8 {
        if self.locked && self.t as f64 / self.rate - self.last_good < 2.0 {
            2
        } else if self.found {
            1
        } else {
            0
        }
    }
}

impl Default for SitorDecoder {
    fn default() -> Self {
        Self::new()
    }
}

impl sdrkit_graph::Decoder for SitorDecoder {
    type Input = Complex32;
    fn name(&self) -> &'static str {
        "hfkbd.sitor"
    }
    fn outputs(&self) -> Vec<Port> {
        decoder::outputs()
    }
    fn values(&self) -> Values {
        no_values()
    }
    fn results(&self) -> Published {
        crate::typed::results("sitor")
    }
    fn configure(&mut self, input: &Format) -> Result<Vec<Format>, ConfigError> {
        self.rate = input.rate;
        self.dial = input.tuned_hz;
        self.history = vec![Complex32::default(); input.rate.round().max(1.0) as usize];
        self.drop_sync();
        Ok(decoder::formats())
    }
    fn retune(&mut self, input: &Format) {
        self.dial = input.tuned_hz;
        self.drop_sync();
    }
    fn process(&mut self, x: &[Complex32], _first: u64, out: &mut Outbox) {
        for &s in x {
            self.level += s.norm_sqr() as f64;
            self.level_n += 1;
            self.buf.push(s);
            if self.buf.len() == NFFT {
                self.search();
                self.buf.drain(..NFFT / 2);
            }
            self.t += 1;
            if !self.history.is_empty() {
                let k = self.hist_at % self.history.len();
                self.history[k] = s;
                self.hist_at += 1;
            }
            if !self.found {
                self.replayed = false;
                continue;
            }
            if !self.replayed {
                self.replayed = true;
                self.replay(out);
                continue;
            }
            self.step(s, out);
        }
    }
    fn report(&mut self, res: &mut Published) {
        let state = self.state();
        let st = &mut self.status;
        st.lock_state =
            [LockState::Searching, LockState::Tracking, LockState::Locked][state as usize];
        let centre = if self.found {
            0.5 * (self.demod.tones[0] + self.demod.tones[1])
        } else {
            0.0
        };
        st.offset_hz = Some(centre);
        st.snr_db = self.found.then_some(self.snr_db);
        if self.level_n > 0 {
            st.level_dbfs = Some(db(self.level / self.level_n as f64) as f32);
            self.level = 0.0;
            self.level_n = 0;
        }
        let shift = if self.found {
            self.demod.tones[1] - self.demod.tones[0]
        } else {
            0.0
        };
        let arq = &self.arq[usize::from(self.arq[1].blocks > self.arq[0].blocks)];
        if arq.locked && state < 2 {
            st.lock_state = LockState::Locked;
        }
        match state {
            _ if arq.locked && state < 2 => put_text(
                &mut st.summary,
                format_args!(
                    "SITOR-A (ARQ) locked, {} blocks, {} repeats",
                    arq.new_blocks, arq.repeats
                ),
            ),
            0 => put_text(&mut st.summary, format_args!("SITOR: no FSK signal")),
            1 => put_text(
                &mut st.summary,
                format_args!("SITOR: FSK {shift:.0} Hz shift, not locked"),
            ),
            _ => put_text(
                &mut st.summary,
                format_args!("SITOR-B locked, {} characters", self.fec.chars),
            ),
        }
        res.results_mut().set_status(st);
        self.heard.update(&self.store);
        let a = self.arq_shown();
        res.put(&SitorFields {
            text: self.heard.text(),
            callsign: last_call(&self.store),
            offset_hz: Hertz(centre),
            shift_hz: self.found.then_some(Hertz(shift)),
            symbol_rate_hz: self.found.then_some(Hertz(BAUD)),
            inverted: self.inverted,
            good_words: self.good as i64,
            bad_words: self.bad as i64,
            mutilated: self.fec.mutilated as i64,
            characters: self.fec.chars as i64,
            arq_locked: a.locked,
            arq_blocks: a.blocks as i64,
            arq_new_blocks: a.new_blocks as i64,
            arq_repeats: a.repeats as i64,
            arq_bad: a.bad as i64,
            arq_control_signals: a.control_signals as i64,
            arq_cs_offset_bits: a.cs_offset.map(|o| o as i64),
            arq_chars: a.chars as i64,
            arq_text: a.text.as_str(),
            selcall: a.selcall(),
            arq_changes: a.changes as i64,
        });
        self.heard.report(res);
    }
    fn commands(&self) -> &'static [&'static Command] {
        HeardCommand::COMMANDS
    }
    fn invoke(&mut self, id: CommandId, args: &Values) -> Result<Option<Record>, CommandError> {
        let p = self.results();
        self.heard.invoke(id, args, &p)
    }
    fn clear(&mut self) {
        self.store.clear();
        self.heard.clear();
    }
    fn forget(&mut self, kept: &Kept) {
        let shown = usize::from(self.arq[1].blocks > self.arq[0].blocks);
        let arq = kept
            .field("arq_text")
            .then(|| std::mem::take(&mut self.arq[shown].text));
        self.drop_sync();
        if let Some(t) = arq {
            self.arq[0].text = t;
        }
        self.good = 0;
        self.bad = 0;
        if !kept.field("text") {
            self.store.clear();
        }
        self.heard.forget(kept);
    }
}

/// The words on the air for one transmission of `text` in mode B: phasing (phasing signal 2 in DX positions, 1 in RX
/// positions), the text in DX positions and again in RX positions, then idle alpha. Characters that ITA2 does not have are left
/// out.
pub fn air_words(text: &str) -> Vec<Word> {
    let mut clean = String::new();
    let table = sdrkit_util_text::ita2::ITU;
    for c in text.chars() {
        if c == '\n' || table.code_of(c).is_ok() {
            clean.push(c);
        }
    }
    let mut out = Vec::new();
    FecBEncoder::new()
        .message(&clean, &mut out)
        .expect("only characters of the table are left");
    out
}

/// The SITOR-B test signal: 100 Bd, 170 Hz shift, B on the higher tone, at unit power.
pub struct SitorModulator {
    rate: f64,
    lower: f64,
    tone: Tone,
    words: Vec<Word>,
    bit: usize,
    left: f64,
}

impl SitorModulator {
    /// Sends `text` again and again, the Y unit on the lower tone at `lower_hz` and the B unit 170 Hz higher.
    pub fn new(rate: f64, text: &str, lower_hz: f64) -> Self {
        SitorModulator {
            rate,
            lower: lower_hz,
            tone: Tone::new(rate),
            words: air_words(text),
            bit: 0,
            left: 0.0,
        }
    }
}

impl Modulator for SitorModulator {
    fn fill(&mut self, out: &mut [Complex32]) {
        let total = self.words.len() * 7;
        for o in out {
            while self.left <= 0.0 {
                self.bit = (self.bit + 1) % total;
                self.left += self.rate / BAUD;
            }
            let w = self.words[self.bit / 7];
            let y = (w.0 >> (self.bit % 7)) & 1 == 1;
            let f = if y { self.lower } else { self.lower + SHIFT };
            *o = self.tone.next(f, 1.0);
            self.left -= 1.0;
        }
    }
}

/// The SITOR-B test text.
pub const TEST_TEXT: &str = "TEST MESSAGE FROM K0TST 0123456789";

/// The generator. It reads the settings `gen_message` and `gen_lower_hz`.
pub fn generator(cfg: &GenConfig) -> Result<Box<dyn Generator>, ConfigError> {
    let lower = cfg.settings.get(&GEN_LOWER_HZ);
    let m = SitorModulator::new(cfg.rate_hz, &decoder::message(cfg, TEST_TEXT), lower);
    Ok(HfGenerator::boxed(
        cfg.rate_hz,
        m,
        SignalInfo::new(SHIFT + BAUD)
            .with_symbol_rate(BAUD)
            .with_bit_rate(BAUD * 5.0 / 14.0),
    ))
}
