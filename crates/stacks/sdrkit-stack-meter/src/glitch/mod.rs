//! SDR stream glitch detector (`glitch`): what a flaky link, a driver or a gain loop does to the samples. Dropped or
//! repeated samples from a jump of the phase of a steady tone, runs of exactly zero samples, blocks equal to the one before,
//! and level steps that stay. It measures and decodes nothing. A drop is seen only on a steady tone 20 dB over the noise
//! per sample, and its count only to whole turns of the tone (the smallest count that fits is shown); a signal that jumps
//! in phase reads as a drop. No public document states these rules. The log and the totals describe the stream, not the
//! tuned frequency, and stay over a change of frequency.

pub mod gen;

use crate::instrument::Log;
use crate::txtest::{tx_stack, Ctx, TxTest};
use crate::typed::{
    GlitchEvent, GlitchEventRow, GlitchFields, GlitchKind, GLITCH_EVENT, GLITCH_EVENTS,
};
use sdrkit_algo_dsp::fft::Fft;
use sdrkit_algo_dsp::peak::log_parabolic;
use sdrkit_algo_dsp::window::Window;
use sdrkit_core::interface::{Hertz, Seconds};
use sdrkit_core::publish::{field, Published};
use sdrkit_core::verify::{CheckLine, VerifyCase, VerifyOptions};
use sdrkit_core::{Complex32, Schema, Setting, SettingSet, Values};
use sdrkit_graph::Kept;
use std::collections::VecDeque;
use std::f64::consts::{PI, TAU};
use std::sync::Arc;

/// The stack id.
pub const ID: &str = "glitch";
/// The least input rate, Hz.
pub const MIN_RATE_HZ: f64 = 100e3;
/// The points of the tone search.
pub const SEARCH_N: usize = 8192;
/// The weight of the one-pole low-pass of the tracker.
pub const LOOP_ALPHA: f64 = 0.03;
/// Samples before the slow phase starts.
pub const SETTLE: u64 = 1500;
/// The proportional gain of the slow phase.
pub const LOOP_P: f64 = 0.01;
/// The integral gain of the slow phase.
pub const LOOP_I: f64 = 2.5e-5;
/// A residual over this starts a step, rad.
pub const STEP_RAD: f64 = 0.06;
/// Samples between steps.
pub const STEP_GAP: u64 = 1000;
/// Samples from the start of a step to its reading.
pub const STEP_WAIT: u64 = 200;
/// A jump over this counts whatever its sample count, rad.
pub const JUMP_RAD: f64 = 0.5;
/// Level blocks kept.
pub const LEVEL_HISTORY: usize = 50;
/// Level blocks before a check.
pub const LEVEL_MIN: usize = 30;
/// Level blocks of a check.
pub const LEVEL_CHECK: usize = 5;

/// What the stack measures.
pub const NOTE_TEXT: &str = "Measures stream faults: dropped and repeated samples from the phase of a steady tone, zero \
runs, repeated blocks and level steps; decodes nothing. A drop is seen only on a steady tone 20 dB over the noise, and its \
count only to whole turns of the tone. The rules are confirmed by no public document. The log and the totals stay over a \
change of frequency.";

/// The shortest zero run.
pub const ZERO_RUN: Setting<field::I64> =
    Setting::new("zero_run", "Shortest zero run", "samples", 16).range(2.0, 10_000.0, 1.0);
/// The repeated block length.
pub const BLOCK: Setting<field::I64> =
    Setting::new("block", "Repeated block length", "samples", 256).range(16.0, 65_536.0, 1.0);
/// The level step.
pub const STEP_DB: Setting<field::F64> =
    Setting::new("step_db", "Level step", "dB", 1.0).range(0.2, 20.0, 0.0);
/// The tone over the noise per sample.
pub const TONE_DB: Setting<field::F64> =
    Setting::new("tone_db", "Tone over the noise per sample", "dB", 20.0).range(10.0, 60.0, 0.0);

sdrkit_core::settings! {
    /// The settings of the component.
    pub enum DecoderSetting {
        /// The zero run.
        ZeroRun = ZERO_RUN,
        /// The block.
        Block = BLOCK,
        /// The level step.
        StepDb = STEP_DB,
        /// The tone test.
        ToneDb = TONE_DB,
    }
}

sdrkit_core::settings! {
    /// The settings of the `glitch` stack.
    pub enum GlitchSetting {
        /// The zero run.
        ZeroRun = ZERO_RUN,
        /// The block.
        Block = BLOCK,
        /// The level step.
        StepDb = STEP_DB,
        /// The tone test.
        ToneDb = TONE_DB,
        /// The tone of the test signal.
        GenToneHz = gen::GEN_TONE_HZ,
        /// Its amplitude.
        GenToneAmp = gen::GEN_TONE_AMP,
        /// Its drops and repeats.
        GenDrops = gen::GEN_DROPS,
        /// Its zero runs.
        GenZeroRuns = gen::GEN_ZERO_RUNS,
        /// Its repeated blocks.
        GenRepeatBlocks = gen::GEN_REPEAT_BLOCKS,
        /// Its gain steps.
        GenGainSteps = gen::GEN_GAIN_STEPS,
    }
}

/// `x` folded into `-pi .. pi`.
fn wrap(x: f64) -> f64 {
    (x + PI).rem_euclid(TAU) - PI
}

/// The phase tracker of the watched tone.
#[derive(Clone, Debug)]
struct Tracker {
    omega: f64,
    lo: Complex32,
    step: Complex32,
    y: Complex32,
    n: u64,
    slow: Option<(f64, f64)>,
    pending: Option<(u64, f64, f64)>,
    last_step: Option<u64>,
}

impl Tracker {
    fn new(f: f64, rate: f64) -> Tracker {
        let omega = TAU * f / rate;
        Tracker {
            omega,
            lo: Complex32::new(1.0, 0.0),
            step: Complex32::from_polar(1.0, -omega as f32),
            y: Complex32::default(),
            n: 0,
            slow: None,
            pending: None,
            last_step: None,
        }
    }

    /// Takes one sample; returns a jump `(start sample, jump rad, samples)` when a step is read.
    fn push(&mut self, z: Complex32) -> Option<(u64, f64, i64)> {
        let m = z * self.lo;
        self.lo *= self.step;
        if self.n.is_multiple_of(1024) {
            self.lo /= self.lo.norm();
        }
        self.y += (m - self.y) * LOOP_ALPHA as f32;
        let fast = f64::from(self.y.arg());
        let k = self.n;
        self.n += 1;
        if k < SETTLE {
            return None;
        }
        let Some((slow, slope)) = self.slow else {
            self.slow = Some((fast, 0.0));
            return None;
        };
        if let Some((start, hs, hp)) = self.pending {
            if k - start < STEP_WAIT {
                return None;
            }
            self.pending = None;
            self.slow = Some((fast, hp));
            let jump = wrap(fast - (hs + (STEP_WAIT + 1) as f64 * hp));
            let count = (jump / self.omega).round() as i64;
            return (count.abs() >= 1 || jump.abs() > JUMP_RAD).then_some((start, jump, count));
        }
        let r = wrap(fast - slow);
        if r.abs() > STEP_RAD && self.last_step.is_none_or(|l| k - l > STEP_GAP) {
            self.pending = Some((k, slow, slope));
            self.last_step = Some(k);
            return None;
        }
        self.slow = Some((wrap(slow + slope + LOOP_P * r), slope + LOOP_I * r));
        None
    }
}

/// The measurement of `glitch`.
#[derive(Default)]
pub struct Glitch {
    rate: f64,
    zero_min: u64,
    block: usize,
    step_db: f64,
    tone_ratio: f64,
    n: u64,
    zeros: u64,
    zero_start: u64,
    prev: Vec<Complex32>,
    cur: Vec<Complex32>,
    level_len: usize,
    level_acc: f64,
    level_n: usize,
    history: VecDeque<f64>,
    check: Option<(u64, f64, Vec<f64>)>,
    search: Option<Fft>,
    search_buf: Vec<Complex32>,
    tone: Option<(f64, Tracker)>,
    dropped: i64,
    repeated: i64,
    zero_runs: i64,
    repeated_blocks: i64,
    level_steps: i64,
    start_n: u64,
    since: u64,
    log: Log<GlitchEventRow>,
}

impl Glitch {
    fn row(
        &mut self,
        c: &mut Ctx,
        at: u64,
        kind: GlitchKind,
        samples: Option<i64>,
        phase: Option<f64>,
        step: Option<(f64, f64)>,
    ) {
        let t = at as f64 / self.rate;
        let e = GlitchEvent {
            time_s: Seconds(t),
            kind,
            samples,
            phase_step_rad: phase,
            step_db: step.map(|s| s.1 - s.0),
            from_db: step.map(|s| s.0),
            to_db: step.map(|s| s.1),
        };
        c.events.push(GLITCH_EVENT.event(t, c.dial(), &e));
        c.tally.bad(self.n as f64 / self.rate);
        self.log.push(GlitchEventRow {
            time_s: e.time_s,
            kind,
            samples,
            phase_step_rad: phase,
            step_db: e.step_db,
            from_db: e.from_db,
            to_db: e.to_db,
        });
    }

    fn level_block(&mut self, db: f64, c: &mut Ctx) {
        let at = self.n + 1 - self.level_len as u64;
        if let Some((t, base, mut seen)) = self.check.take() {
            seen.push(db);
            if seen.len() < LEVEL_CHECK {
                self.check = Some((t, base, seen));
            } else {
                let m = seen.iter().sum::<f64>() / seen.len() as f64;
                if (m - base).abs() >= self.step_db && (db - base).abs() >= self.step_db {
                    self.level_steps += 1;
                    self.row(c, t, GlitchKind::LevelStep, None, None, Some((base, m)));
                    self.history.clear();
                    return;
                }
            }
        } else if self.history.len() >= LEVEL_MIN {
            let mut h: Vec<f64> = self.history.iter().copied().collect();
            let mid = h.len() / 2;
            let (_, med, _) = h.select_nth_unstable_by(mid, f64::total_cmp);
            let med = *med;
            if (db - med).abs() >= self.step_db {
                self.check = Some((at, med, Vec::new()));
            }
        }
        self.history.push_back(db);
        if self.history.len() > LEVEL_HISTORY {
            self.history.pop_front();
        }
    }

    fn search(&mut self) -> Option<f64> {
        let fft = self.search.as_mut()?;
        let n = SEARCH_N;
        let w = Window::Hann.coefficients(n);
        let mut b: Vec<Complex32> = self.search_buf.iter().zip(&w).map(|(z, g)| z * g).collect();
        fft.process(&mut b);
        let p: Vec<f64> = b.iter().map(|z| f64::from(z.norm_sqr())).collect();
        let k = (0..n).max_by(|&a, &b| p[a].total_cmp(&p[b]))?;
        let mut s = p.clone();
        let (_, med, _) = s.select_nth_unstable_by(n / 2, f64::total_cmp);
        let noise = *med / 0.693;
        if p[k] <= 0.0 || p[k] < self.tone_ratio * n as f64 * 0.667 * noise {
            return None;
        }
        let r = log_parabolic(p[(k + n - 1) % n], p[k], p[(k + 1) % n]);
        let bin = if k >= n / 2 {
            k as f64 - n as f64
        } else {
            k as f64
        } + r.offset;
        Some(bin * self.rate / n as f64)
    }

    fn sample(&mut self, z: Complex32, c: &mut Ctx) {
        if z.re == 0.0 && z.im == 0.0 {
            if self.zeros == 0 {
                self.zero_start = self.n;
            }
            self.zeros += 1;
            if self.zeros == self.zero_min {
                self.zero_runs += 1;
                self.row(c, self.zero_start, GlitchKind::ZeroRun, None, None, None);
            }
        } else {
            self.zeros = 0;
        }
        self.cur.push(z);
        if self.cur.len() == self.block {
            let all_zero = self.cur.iter().all(|z| z.re == 0.0 && z.im == 0.0);
            if !all_zero && self.cur == self.prev {
                self.repeated_blocks += 1;
                self.row(
                    c,
                    self.n + 1 - self.block as u64,
                    GlitchKind::RepeatedBlock,
                    None,
                    None,
                    None,
                );
            }
            std::mem::swap(&mut self.prev, &mut self.cur);
            self.cur.clear();
        }
        self.level_acc += f64::from(z.norm_sqr());
        self.level_n += 1;
        if self.level_n == self.level_len {
            let db = 10.0 * (self.level_acc / self.level_n as f64).max(1e-30).log10();
            self.level_acc = 0.0;
            self.level_n = 0;
            self.level_block(db, c);
        }
        self.n += 1;
    }

    fn track(&mut self, z: Complex32, c: &mut Ctx) {
        let Some((_, tr)) = self.tone.as_mut() else {
            return;
        };
        let Some((start, jump, count)) = tr.push(z) else {
            return;
        };
        let at = self.start_n + start;
        let kind = if count > 0 {
            self.dropped += 1;
            GlitchKind::DroppedSamples
        } else {
            self.repeated += 1;
            GlitchKind::RepeatedSamples
        };
        self.row(c, at, kind, Some(count.abs()), Some(jump), None);
    }
}

impl TxTest for Glitch {
    const ID: &'static str = ID;
    const NAME: &'static str = "glitch.meter";
    const MIN_RATE_HZ: f64 = MIN_RATE_HZ;
    const NOTE: &'static str = NOTE_TEXT;
    fn schema() -> Arc<Schema> {
        DecoderSetting::schema()
    }
    fn start(&mut self, rate_hz: f64, v: &Values) {
        self.rate = rate_hz;
        self.zero_min = v.get(&ZERO_RUN).max(2) as u64;
        self.block = v.get(&BLOCK).max(16) as usize;
        self.step_db = v.get(&STEP_DB);
        self.tone_ratio = 10f64.powf(v.get(&TONE_DB) / 10.0);
        self.since = self.n;
        self.zeros = 0;
        self.prev.clear();
        self.cur.clear();
        self.level_len = ((0.001 * rate_hz).floor() as usize).max(8);
        self.level_acc = 0.0;
        self.level_n = 0;
        self.history.clear();
        self.check = None;
        self.search = Some(Fft::forward(SEARCH_N));
        self.search_buf.clear();
        self.tone = None;
    }
    fn feed(&mut self, x: &[Complex32], c: &mut Ctx) {
        for &z in x {
            self.sample(z, c);
            if self.tone.is_some() {
                self.track(z, c);
                continue;
            }
            self.search_buf.push(z);
            if self.search_buf.len() < SEARCH_N {
                continue;
            }
            if let Some(f) = self.search() {
                self.start_n = self.n - SEARCH_N as u64;
                self.tone = Some((f, Tracker::new(f, self.rate)));
                let replay = std::mem::take(&mut self.search_buf);
                for &y in &replay {
                    self.track(y, c);
                }
                c.tally.good(self.n as f64 / self.rate);
            }
            self.search_buf.clear();
        }
        if self.tone.is_some() {
            c.tally.good(self.n as f64 / self.rate);
        }
    }
    fn clear(&mut self) {
        self.dropped = 0;
        self.repeated = 0;
        self.zero_runs = 0;
        self.repeated_blocks = 0;
        self.level_steps = 0;
        self.log.clear();
    }
    fn forget(&mut self, kept: &Kept) {
        if !kept.table(GLITCH_EVENTS.key) {
            self.log.clear();
        }
        for (key, v) in [
            (GlitchFields::SAMPLE_DROPS.key, &mut self.dropped),
            (GlitchFields::SAMPLE_REPEATS.key, &mut self.repeated),
            (GlitchFields::ZERO_RUNS.key, &mut self.zero_runs),
            (GlitchFields::REPEATED_BLOCKS.key, &mut self.repeated_blocks),
            (GlitchFields::LEVEL_STEPS.key, &mut self.level_steps),
        ] {
            if !kept.field(key) {
                *v = 0;
            }
        }
    }
    fn now(&self) -> f64 {
        self.n as f64 / self.rate.max(1.0)
    }
    fn publish(&mut self, p: &mut Published, _c: &Ctx) -> String {
        self.log.publish(&GLITCH_EVENTS, p);
        let tone = self.tone.as_ref().map(|t| t.0);
        p.put(&GlitchFields {
            tone_hz: tone.map(Hertz),
            sample_drops: self.dropped,
            sample_repeats: self.repeated,
            zero_runs: self.zero_runs,
            repeated_blocks: self.repeated_blocks,
            level_steps: self.level_steps,
            elapsed_s: Seconds((self.n - self.since) as f64 / self.rate.max(1.0)),
        });
        let faults =
            self.dropped + self.repeated + self.zero_runs + self.repeated_blocks + self.level_steps;
        match tone {
            Some(f) => format!("watching a tone at {f:.1} Hz; {faults} faults"),
            None => format!("no steady tone; {faults} faults"),
        }
    }
}

const KEYS: [&str; 4] = ["zero_run", "block", "step_db", "tone_db"];

fn verify_cases(o: &VerifyOptions) -> Vec<VerifyCase> {
    let mut c = VerifyCase::new("a tone with four drops and two repeats", o, 0.0);
    c.seconds = o.seconds.max(4.0);
    c.snr_db = Some(40.0);
    c.expect_audio = Some(false);
    c.check = Some(|r| {
        let d = crate::instrument::number(r, GlitchFields::SAMPLE_DROPS.key);
        let k = crate::instrument::number(r, GlitchFields::SAMPLE_REPEATS.key);
        vec![CheckLine::new(
            GlitchFields::SAMPLE_DROPS.key,
            d == 4.0 && k == 2.0,
            format!("{d} drops and {k} repeats, 4 and 2 sent"),
        )]
    });
    vec![c]
}

tx_stack! {
    meter: Glitch,
    settings: GlitchSetting,
    keys: KEYS,
    name: "SDR stream glitch detector",
    blurb: "Dropped and repeated samples on a steady tone, zero runs, repeated blocks and level steps of the raw stream.",
    tuning: (0.1e6, 6e9, 2e6, 2e6, false),
    rate_text: "The stream glitch detector needs at least 100 ksps",
    generator: |cfg| gen::GlitchGenerator::new(cfg.rate_hz, &cfg.settings),
    verify: verify_cases
}
