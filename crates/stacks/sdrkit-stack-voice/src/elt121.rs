//! ELT 121.5 MHz (`elt121`): detects the swept tone of an analogue emergency locator transmitter. Detection only: the beacon
//! sends no data and no identity.
//! Standard: ICAO Annex 10 Volume III (an AM carrier, a tone that sweeps down over 700 Hz or more between 1600 and 300 Hz,
//! 2 to 4 sweeps a second).
//!
//! ```text
//! iq -> [guard] -> [front, 12 kHz, dial at 0 Hz] -> [elt121: envelope, tone search, sweep runs, period check] -> events
//! ```

use crate::common::{self, Placer, Report, Stage, MIN_RATE_HZ, RATE};
use crate::typed::{Elt121EndedEvent, Elt121Fields, Elt121FoundEvent, ELT121_ENDED, ELT121_FOUND};
use sdrkit_algo_dsp::peak::log_parabolic;
use sdrkit_algo_dsp::window::Window;
use sdrkit_core::interface::{Fields as _, Hertz};
use sdrkit_core::publish::field;
use sdrkit_core::{
    CommandError, CommandId, CommandSet, Commands, Complex32, ConfigError, GenConfig, Generator,
    Record, Results, ResultsDecl, Setting, SignalInfo, StackConfig, Values,
};
use sdrkit_graph::{
    Catalog, Component, ComponentInfo, DecoderCommand, Format, Graph, GraphBuilder, GraphError, Io,
    Kept, Kind, Port, RunContext, StackGraph,
};
use std::collections::VecDeque;
use std::f64::consts::TAU;
use std::sync::Arc;

/// The stack id.
pub const ID: &str = "elt121";
/// The name in a status line. It says that the stack only detects.
pub const NAME: &str = "ELT 121.5 MHz (detection only)";
/// The samples of a frame (20 ms).
const FRAME: usize = 240;
/// The samples between two frames (10 ms).
const HOP: usize = 120;
/// The tone search: 65 frequencies from 250 Hz in steps of 25 Hz.
const TONES: usize = 65;
const TONE_LOW_HZ: f64 = 250.0;
const TONE_STEP_HZ: f64 = 25.0;
const TONE_HIGH_HZ: f64 = 1850.0;

/// A sweep that the detector counted.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Sweep {
    /// When it started, s.
    pub start_s: f64,
    /// Its first frequency, Hz.
    pub top_hz: f64,
    /// Its lowest frequency, Hz.
    pub bottom_hz: f64,
}

/// A change that [`SweepDetector::process`] reports.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Change {
    /// The periods of the sweeps confirmed the signal.
    Found,
    /// A confirmed signal had no sweep for 2 s. The value is the count of its sweeps.
    Ended(u64),
}

#[derive(Clone, Copy, Debug)]
struct Run {
    start_s: f64,
    start_hz: f64,
    low_hz: f64,
    last_hz: f64,
    last_s: f64,
    quiet: u32,
}

/// The detector: the 12 kHz channel (the carrier anywhere near 0 Hz) to sweeps and their rate.
pub struct SweepDetector {
    carrier: f64,
    ring: Vec<f32>,
    fill: usize,
    frames: u64,
    /// `TONES` rows of `FRAME` windowed cosines and sines.
    cos: Vec<f32>,
    sin: Vec<f32>,
    powers: [f64; TONES],
    run: Option<Run>,
    starts: VecDeque<f64>,
    sweeps: u64,
    sweeps_since_found: u64,
    last_sweep_s: f64,
    top_hz: f64,
    bottom_hz: f64,
    depth: f64,
    confirmed: bool,
    seen: bool,
    rate_hz: f64,
    quality_db: Option<f64>,
}

impl Default for SweepDetector {
    fn default() -> Self {
        let w = Window::Hann.coefficients(FRAME);
        let mut cos = Vec::with_capacity(TONES * FRAME);
        let mut sin = Vec::with_capacity(TONES * FRAME);
        for t in 0..TONES {
            let f = TONE_LOW_HZ + TONE_STEP_HZ * t as f64;
            for (n, &wn) in w.iter().enumerate() {
                let (s, c) = (TAU * f * n as f64 / RATE).sin_cos();
                cos.push(wn * c as f32);
                sin.push(wn * s as f32);
            }
        }
        SweepDetector {
            carrier: 0.0,
            ring: vec![0.0; FRAME],
            fill: 0,
            frames: 0,
            cos,
            sin,
            powers: [0.0; TONES],
            run: None,
            starts: VecDeque::new(),
            sweeps: 0,
            sweeps_since_found: 0,
            last_sweep_s: 0.0,
            top_hz: 0.0,
            bottom_hz: 0.0,
            depth: 0.0,
            confirmed: false,
            seen: false,
            rate_hz: 0.0,
            quality_db: None,
        }
    }
}

impl SweepDetector {
    /// The periods of the sweeps confirm the signal.
    pub fn confirmed(&self) -> bool {
        self.confirmed
    }
    /// A sweep was counted and the signal has not ended.
    pub fn seen(&self) -> bool {
        self.seen
    }
    /// Sweeps a second. 0 when not confirmed.
    pub fn rate_hz(&self) -> f64 {
        self.rate_hz
    }
    /// Sweeps counted.
    pub fn sweeps(&self) -> u64 {
        self.sweeps
    }
    /// The smoothed first frequency of the sweeps, Hz.
    pub fn top_hz(&self) -> f64 {
        self.top_hz
    }
    /// The smoothed lowest frequency of the sweeps, Hz.
    pub fn bottom_hz(&self) -> f64 {
        self.bottom_hz
    }
    /// The modulation of the tone, 0 to 1.
    pub fn depth(&self) -> f64 {
        self.depth.min(1.0)
    }
    /// The peak of the tone search over its mean in the last frame, dB.
    pub fn quality_db(&self) -> Option<f64> {
        self.quality_db
    }

    /// Forgets everything.
    pub fn reset(&mut self) {
        let SweepDetector { cos, sin, .. } = std::mem::take(self);
        self.cos = cos;
        self.sin = sin;
    }

    /// Ends the run: counts it when it is a sweep, and checks the periods.
    fn end_run(&mut self, run: Run, changes: &mut Vec<Change>) {
        let is_sweep = run.start_hz >= 900.0
            && run.start_hz - run.low_hz >= 600.0
            && run.last_s - run.start_s >= 0.08;
        if !is_sweep {
            return;
        }
        if self.starts.back().is_some_and(|&b| run.start_s - b > 1.5) {
            self.starts.clear();
        }
        self.starts.push_back(run.start_s);
        if self.starts.len() > 9 {
            self.starts.pop_front();
        }
        if self.sweeps == 0 || !self.seen {
            self.top_hz = run.start_hz;
            self.bottom_hz = run.low_hz;
        } else {
            self.top_hz = 0.7 * self.top_hz + 0.3 * run.start_hz;
            self.bottom_hz = 0.7 * self.bottom_hz + 0.3 * run.low_hz;
        }
        self.sweeps += 1;
        self.sweeps_since_found += 1;
        self.last_sweep_s = run.start_s;
        self.seen = true;
        if self.starts.len() < 4 {
            return;
        }
        let mut periods: Vec<f64> = self
            .starts
            .iter()
            .zip(self.starts.iter().skip(1))
            .map(|(a, b)| b - a)
            .collect();
        periods.sort_by(f64::total_cmp);
        let median = periods[periods.len() / 2];
        let near = periods
            .iter()
            .filter(|&&p| (p - median).abs() <= 0.2 * median)
            .count();
        if (0.2..=0.6).contains(&median) && near >= 3 {
            self.rate_hz = 1.0 / median;
            if !self.confirmed {
                self.confirmed = true;
                changes.push(Change::Found);
            }
        }
    }

    /// Takes the frame that ends now.
    fn frame(&mut self, x: &[f32; FRAME], changes: &mut Vec<Change>) {
        let now = self.frames as f64 * HOP as f64 / RATE;
        self.frames += 1;
        let mut best = 0;
        let mut sum = 0.0;
        for t in 0..TONES {
            let row = t * FRAME..(t + 1) * FRAME;
            let (mut re, mut im) = (0.0f32, 0.0f32);
            for ((&v, &c), &s) in x.iter().zip(&self.cos[row.clone()]).zip(&self.sin[row]) {
                re += v * c;
                im += v * s;
            }
            let p = f64::from(re * re + im * im);
            self.powers[t] = p;
            sum += p;
            if p > self.powers[best] {
                best = t;
            }
        }
        let peak = self.powers[best];
        let mean = sum / TONES as f64;
        let mut hz = TONE_LOW_HZ + TONE_STEP_HZ * best as f64;
        if best > 0 && best + 1 < TONES {
            hz += TONE_STEP_HZ
                * log_parabolic(self.powers[best - 1], peak, self.powers[best + 1]).offset;
        }
        // The Hann window sums to half the frame.
        let amplitude = 2.0 * peak.sqrt() / (FRAME as f64 / 2.0);
        let modulation = amplitude / self.carrier.max(1e-12);
        self.quality_db = (mean > 0.0).then(|| 10.0 * (peak / mean).log10());
        let strong =
            peak > 12.0 * mean && modulation > 0.25 && (TONE_LOW_HZ..=TONE_HIGH_HZ).contains(&hz);
        if strong {
            self.depth += 0.1 * (modulation - self.depth);
            match self.run {
                None => self.run = Some(Run::new(now, hz)),
                Some(run) if hz > run.last_hz + 250.0 => {
                    self.end_run(run, changes);
                    self.run = Some(Run::new(now, hz));
                }
                Some(ref mut run) if hz <= run.last_hz + 120.0 => {
                    run.last_hz = run.last_hz.min(hz);
                    run.low_hz = run.low_hz.min(hz);
                    run.last_s = now;
                    run.quiet = 0;
                }
                Some(_) => {}
            }
        } else if let Some(mut run) = self.run.take() {
            run.quiet += 1;
            if run.quiet > 4 {
                self.end_run(run, changes);
            } else {
                self.run = Some(run);
            }
        }
        if self.seen && now - self.last_sweep_s > 2.0 {
            if self.confirmed {
                changes.push(Change::Ended(self.sweeps_since_found));
            }
            self.confirmed = false;
            self.seen = false;
            self.rate_hz = 0.0;
            self.sweeps_since_found = 0;
            self.starts.clear();
        }
    }

    /// Takes `x` and appends each change to `changes`.
    pub fn process(&mut self, x: &[Complex32], changes: &mut Vec<Change>) {
        let mut frame = [0.0f32; FRAME];
        for s in x {
            let env = f64::from(s.norm());
            self.carrier += 0.0005 * (env - self.carrier);
            self.ring[self.fill % FRAME] = (env - self.carrier) as f32;
            self.fill += 1;
            if self.fill >= FRAME && self.fill.is_multiple_of(HOP) {
                let at = self.fill % FRAME;
                frame[..FRAME - at].copy_from_slice(&self.ring[at..]);
                frame[FRAME - at..].copy_from_slice(&self.ring[..at]);
                self.frame(&frame, changes);
            }
        }
    }
}

impl Run {
    fn new(now: f64, hz: f64) -> Run {
        Run {
            start_s: now,
            start_hz: hz,
            low_hz: hz,
            last_hz: hz,
            last_s: now,
            quiet: 0,
        }
    }
}

/// What the stack publishes.
pub fn declaration() -> ResultsDecl {
    crate::typed::shape(ID).results_decl()
}

/// The results of the detector component: the shared fields, its own fields.
fn detector_declaration() -> ResultsDecl {
    common::declaration(Elt121Fields::SHAPE.fields)
}

/// The detector as a component (`voice.elt121`).
pub struct Elt121 {
    det: SweepDetector,
    report: Report,
    changes: Vec<Change>,
}

impl Default for Elt121 {
    fn default() -> Self {
        Elt121 {
            det: SweepDetector::default(),
            report: Report::new(NAME, detector_declaration()),
            changes: Vec::new(),
        }
    }
}

impl Elt121 {
    fn publish(&mut self) {
        let d = &self.det;
        let (stage, text) = if d.confirmed() {
            (
                Stage::Confirmed,
                format!(
                    "ELT tone: {:.1} sweeps/s, {:.0} to {:.0} Hz, AM {:.0} %",
                    d.rate_hz(),
                    d.top_hz(),
                    d.bottom_hz(),
                    100.0 * d.depth()
                ),
            )
        } else if d.seen() {
            (
                Stage::Signal,
                format!("swept tone seen ({}), not confirmed", d.sweeps()),
            )
        } else {
            (Stage::Searching, "searching".to_string())
        };
        let p = &mut self.report.p;
        p.put(&Elt121Fields {
            confirmed: d.confirmed(),
            sweep_rate_hz: d.confirmed().then(|| d.rate_hz()),
            sweep_top_hz: d.seen().then(|| Hertz(d.top_hz())),
            sweep_bottom_hz: d.seen().then(|| Hertz(d.bottom_hz())),
            depth_pct: d.seen().then(|| 100.0 * d.depth()),
        });
        let (quality, sweeps) = (d.quality_db(), d.sweeps());
        self.report.publish(stage, &text, quality, sweeps, 0);
    }
}

impl Component for Elt121 {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "voice.elt121",
            vec![Port::iq("in"), Port::iq("raw")],
            vec![Port::new("events", Kind::Messages)],
        )
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        if (inputs[0].rate - RATE).abs() > 1e-6 {
            return Err(ConfigError::WrongRate {
                need: RATE,
                got: inputs[0].rate,
            });
        }
        self.det.reset();
        self.report.configure(inputs[1].rate);
        Ok(vec![Format::new(Kind::Messages, 0.0)])
    }
    fn attach(&mut self, ctx: &RunContext) {
        self.report.attach(ctx);
    }
    fn process(&mut self, io: &mut Io) {
        let x = io.iq(0);
        self.changes.clear();
        self.det.process(x, &mut self.changes);
        self.report.take(x);
        for i in 0..self.changes.len() {
            let e = match self.changes[i] {
                Change::Found => {
                    let d = &self.det;
                    let text = format!(
                        "ELT swept tone found: {:.1} sweeps/s, {:.0} to {:.0} Hz",
                        d.rate_hz(),
                        d.top_hz(),
                        d.bottom_hz()
                    );
                    let (rate, top, bottom) = (d.rate_hz(), d.top_hz(), d.bottom_hz());
                    let t = self.report.note(&text);
                    ELT121_FOUND.event(
                        t,
                        0.0,
                        &Elt121FoundEvent {
                            text: &text,
                            sweep_rate_hz: rate,
                            sweep_top_hz: Hertz(top),
                            sweep_bottom_hz: Hertz(bottom),
                        },
                    )
                }
                Change::Ended(n) => {
                    let text = format!("ELT tone ended after {n} sweeps");
                    let t = self.report.note(&text);
                    ELT121_ENDED.event(t, 0.0, &Elt121EndedEvent { text: &text })
                }
            };
            self.report.send(e);
        }
        self.report.drain(io.messages_out(0));
        if io.tick() || !self.changes.is_empty() {
            self.publish();
        }
    }
    fn values(&self) -> Values {
        sdrkit_graph::no_values()
    }
    fn results(&self) -> Option<&Results> {
        Some(self.report.results())
    }
    fn commands(&self) -> &Commands {
        DecoderCommand::commands()
    }
    fn invoke(&mut self, id: CommandId, _: &Values) -> Result<Option<Record>, CommandError> {
        match DecoderCommand::from_id(id).ok_or(CommandError::NoSuchId { id })? {
            DecoderCommand::Clear => {
                self.det.reset();
                self.report.clear();
                self.publish();
            }
        }
        Ok(None)
    }
    fn forget(&mut self, kept: &Kept) {
        self.det.reset();
        self.report.forget(kept);
        self.publish();
    }
}

/// The graph of the stack.
///
/// # Errors
///
/// [`GraphError`] if the graph does not build.
pub fn graph(catalog: &Arc<Catalog>) -> Result<Graph, GraphError> {
    common::add_front(GraphBuilder::new())
        .add("detector", Elt121::default())
        .connect("front", "detector.in")
        .input("detector.raw")
        .output("detector")
        .clock(0.25)
        .status_from("detector")
        .catalog(catalog)
        .build()
}

/// The stack graph.
///
/// # Errors
///
/// [`GraphError`] if the graph does not build or a setting of `cfg` is refused.
pub fn stack_graph(cfg: &StackConfig) -> Result<StackGraph, GraphError> {
    StackGraph::of(crate::typed::shape(ID), graph(&Catalog::of(cfg)?)?)
        .min_rate(
            MIN_RATE_HZ,
            "ELT 121.5 MHz: the sample rate is too low (250 kHz at least)",
        )
        .command(
            "clear",
            sdrkit_graph::CommandTarget::of("detector", "clear"),
        )
        .on_retune(sdrkit_graph::CommandTarget::of("detector", "clear"))?
        .with_settings(&cfg.settings)
        .build()
}

const GEN: &str = "Test signal";
/// Sweeps a second of the test signal.
pub const GEN_SWEEP_HZ: Setting<field::F64> =
    Setting::new("gen_sweep_hz", "Test signal: sweeps a second", "Hz", 3.0)
        .range(0.5, 10.0, 0.0)
        .section(GEN)
        .advanced();
/// The part of a sweep period with the tone on.
pub const GEN_DUTY_PCT: Setting<field::F64> =
    Setting::new("gen_duty_pct", "Test signal: duty", "%", 60.0)
        .range(10.0, 100.0, 0.0)
        .section(GEN)
        .advanced();
/// The first frequency of a sweep of the test signal.
pub const GEN_TOP_HZ: Setting<field::Hz> =
    Setting::new("gen_top_hz", "Test signal: from", "Hz", 1600.0)
        .range(100.0, 4000.0, 0.0)
        .section(GEN)
        .help("A value below the end frequency makes an upward sweep.")
        .advanced();
/// The last frequency of a sweep of the test signal.
pub const GEN_BOTTOM_HZ: Setting<field::Hz> =
    Setting::new("gen_bottom_hz", "Test signal: to", "Hz", 300.0)
        .range(100.0, 4000.0, 0.0)
        .section(GEN)
        .advanced();
/// The modulation depth of the test signal.
pub const GEN_DEPTH: Setting<field::F64> = Setting::new("gen_depth", "Test signal: depth", "", 0.9)
    .range(0.0, 1.0, 0.0)
    .section(GEN)
    .advanced();
/// The length of the test signal before it starts again.
pub const GEN_SECONDS: Setting<field::Duration> =
    Setting::new("gen_seconds", "Test signal: length", "s", 6.0)
        .range(1.0, 600.0, 0.0)
        .section(GEN)
        .advanced();

sdrkit_core::settings! {
    /// The settings of the stack: only those of its test signal.
    pub enum EltSetting {
        /// Sweeps a second.
        GenSweep = GEN_SWEEP_HZ,
        /// Duty.
        GenDuty = GEN_DUTY_PCT,
        /// First frequency.
        GenTop = GEN_TOP_HZ,
        /// Last frequency.
        GenBottom = GEN_BOTTOM_HZ,
        /// Depth.
        GenDepth = GEN_DEPTH,
        /// Length.
        GenSeconds = GEN_SECONDS,
    }
}

/// The test signal: an AM carrier of amplitude 0.5 with a tone that sweeps in each period, after a lead of 0.3 s.
/// [`EltGenerator::new`] puts the carrier at 0 Hz, [`EltGenerator::from_config`] 20 kHz below the centre, where the radio of
/// the stack sees it.
#[derive(Clone, Debug)]
pub struct EltGenerator {
    /// The sample rate, Hz.
    pub rate: f64,
    /// Sweeps a second.
    pub sweep_hz: f64,
    /// The part of a period with the tone on, 0.1 to 1.
    pub duty: f64,
    /// The first frequency of a sweep, Hz.
    pub top_hz: f64,
    /// The last frequency of a sweep, Hz.
    pub bottom_hz: f64,
    /// The modulation depth.
    pub depth: f64,
    /// The length before the signal starts again, s.
    pub seconds: f64,
    n: u64,
    phase: f64,
    placer: Placer,
}

/// The lead of the test signal: the carrier alone.
const GEN_LEAD_S: f64 = 0.3;

impl EltGenerator {
    /// The default test signal at `rate`.
    pub fn new(rate: f64) -> Self {
        EltGenerator {
            rate,
            sweep_hz: 3.0,
            duty: 0.6,
            top_hz: 1600.0,
            bottom_hz: 300.0,
            depth: 0.9,
            seconds: 6.0,
            n: 0,
            phase: 0.0,
            placer: Placer::new(rate, 0.0),
        }
    }
    /// The generator of the stack's settings.
    pub fn from_config(cfg: &GenConfig) -> Self {
        let s = &cfg.settings;
        EltGenerator {
            sweep_hz: s.get(&GEN_SWEEP_HZ),
            duty: s.get(&GEN_DUTY_PCT) / 100.0,
            top_hz: s.get(&GEN_TOP_HZ),
            bottom_hz: s.get(&GEN_BOTTOM_HZ),
            depth: s.get(&GEN_DEPTH),
            seconds: s.get(&GEN_SECONDS),
            placer: Placer::below_centre(cfg.rate_hz),
            ..EltGenerator::new(cfg.rate_hz)
        }
    }
}

impl Generator for EltGenerator {
    fn rate_hz(&self) -> f64 {
        self.rate
    }
    fn signal_info(&self) -> Option<SignalInfo> {
        let power = 0.25 * (1.0 + 0.5 * self.depth * self.depth * self.duty);
        Some(SignalInfo::new(3000.0).with_power(power))
    }
    fn generate(&mut self, out: &mut [Complex32]) -> usize {
        let period = 1.0 / self.sweep_hz;
        let total = (self.seconds * self.rate).round().max(1.0) as u64;
        for o in out.iter_mut() {
            let t = (self.n % total) as f64 / self.rate - GEN_LEAD_S;
            self.n += 1;
            let mut m = 0.0;
            if t >= 0.0 {
                let into = t % period;
                let on = self.duty * period;
                if into < on {
                    let hz = self.top_hz + (self.bottom_hz - self.top_hz) * into / on;
                    self.phase = (self.phase + hz / self.rate).fract();
                    m = self.depth * (TAU * self.phase).sin();
                }
            }
            *o = Complex32::new((0.5 * (1.0 + m)) as f32, 0.0);
        }
        self.placer.apply(out);
        out.len()
    }
}
