//! A stand-in for the engine: it applies each step of the sweep service to the fake radio, turns the samples into the
//! spectrum frames that the service declares, and reads the results through the owner by reflection.

#![allow(dead_code)]

pub mod fake;

use fake::FakeRadio;
use sdrkit_algo_dsp::spectrum::{SpectrumAnalyzer, SpectrumConfig};
use sdrkit_algo_dsp::Complex32;
use sdrkit_core::command::CommandError;
use sdrkit_core::owner::{self, Owner};
use sdrkit_core::{Clock, Event, HostRequest, MemStorage, Record, Value};
use sdrkit_svc_sweep::{Input, RadioInfo, SpectrumFrame, SpectrumNeed, Step, SweepService};
use std::sync::Arc;

pub const RATE: f64 = 2.4e6;

/// A service on a fresh memory storage, with the wall clock at 0.
pub fn service() -> (SweepService, Arc<MemStorage>) {
    let st = Arc::new(MemStorage::new());
    (SweepService::new(st.clone(), Clock::fixed(0.0)), st)
}

/// Sets the setting `key` through the checked path a host uses.
pub fn set(s: &mut SweepService, key: &str, v: Value) {
    let id = s.schema().id(key).unwrap();
    owner::apply(s, id, &v).unwrap();
}

/// Runs command `key` with the arguments `args` given by key, through the checked path a host uses.
pub fn call(
    s: &mut SweepService,
    key: &str,
    args: &[(&str, Value)],
) -> Result<Option<Record>, CommandError> {
    let id = s.commands().id(key)?;
    let def = s.commands().def(id).unwrap();
    let mut given = def.new_args();
    for (k, v) in args {
        let fid = given.decl().id(k).unwrap();
        given.set_value(fid, v).unwrap();
    }
    owner::invoke(s, id, &given)
}

pub fn text(v: &str) -> Value {
    Value::Text(v.into())
}

/// The rows of table `key` as (row key, record), found by reflection only.
pub fn rows<'a>(s: &'a SweepService, key: &str) -> Vec<(&'a str, &'a Record)> {
    let id = s.results_decl().table_id(key).unwrap();
    s.results().table(id).unwrap().rows_newest_first().collect()
}

/// The value of column `key` of a row or a reply.
pub fn cell<'a>(r: &'a Record, key: &str) -> &'a Value {
    r.value(r.decl().id(key).unwrap()).unwrap()
}

/// A number cell of any number kind.
pub fn num(r: &Record, key: &str) -> f64 {
    match cell(r, key) {
        Value::F64(v) | Value::Hz(v) | Value::Time(v) | Value::Duration(v) => *v,
        Value::I64(v) => *v as f64,
        other => panic!("{key}: {other:?}"),
    }
}

pub fn flag(r: &Record, key: &str) -> bool {
    match cell(r, key) {
        Value::Bool(b) => *b,
        other => panic!("{key}: {other:?}"),
    }
}

/// The key of the choice in column `key` of `r`.
pub fn choice<'a>(r: &'a Record, key: &str) -> &'a str {
    let id = r.decl().id(key).unwrap();
    match r.value(id) {
        Some(Value::Choice(i)) => &r.decl().field(id).unwrap().choices[usize::from(*i)].key,
        other => panic!("{key}: {other:?}"),
    }
}

/// The value of the result field `key`.
pub fn field<'a>(s: &'a SweepService, key: &str) -> &'a Value {
    let f = s.results().fields();
    f.value(s.results_decl().field_id(key).unwrap()).unwrap()
}

pub fn field_num(s: &SweepService, key: &str) -> f64 {
    num(s.results().fields(), key)
}

pub fn field_choice<'a>(s: &'a SweepService, key: &str) -> &'a str {
    choice(s.results().fields(), key)
}

/// The points of the series `key`.
pub fn series<'a>(s: &'a SweepService, key: &str) -> &'a [[f32; 2]] {
    let id = s.results_decl().series_id(key).unwrap();
    &s.results().series(id).unwrap().points
}

/// The confirmed channels without a flag, from the findings table.
pub fn visible(s: &SweepService) -> Vec<&Record> {
    rows(s, "findings")
        .into_iter()
        .map(|r| r.1)
        .filter(|r| !flag(r, "image") && !flag(r, "spur") && !flag(r, "dc"))
        .collect()
}

/// Turns samples into frames back to back, as the engine does for the spectrum that the service declares.
pub struct Frames {
    analyzer: Option<SpectrumAnalyzer>,
}

impl Frames {
    pub fn new() -> Self {
        Frames { analyzer: None }
    }

    /// Starts again after a retune: no frame holds samples of two centres.
    pub fn retune(&mut self) {
        if let Some(a) = &mut self.analyzer {
            a.reset();
            a.drop_partial();
        }
    }

    /// Feeds `iq` and calls `f` with each frame made for `need`, until `f` returns false.
    pub fn push(
        &mut self,
        need: SpectrumNeed,
        iq: &[Complex32],
        mut f: impl FnMut(&[f32]) -> bool,
    ) {
        let fits = self
            .analyzer
            .as_ref()
            .is_some_and(|a| a.len() == need.bins && a.config().sample_rate == need.rate_hz);
        if !fits {
            let mut cfg = SpectrumConfig::new(need.bins, need.rate_hz);
            cfg.window = need.window;
            cfg.frames_per_second = need.rate_hz / need.bins as f64 * (1.0 - 1e-12);
            cfg.average = need.average as usize;
            self.analyzer = Some(SpectrumAnalyzer::new(cfg).unwrap());
        }
        let a = self.analyzer.as_mut().unwrap();
        let mut i = 0;
        while i < iq.len() {
            let want = (a.len() - a.partial()).min(iq.len() - i);
            if a.push(&iq[i..i + want]) > 0 && !f(a.spectrum_db()) {
                return;
            }
            i += want;
        }
    }
}

/// The engine stand-in.
pub struct Driver {
    pub radio: FakeRadio,
    pub radio_sweeps: bool,
    pub can_sweep: bool,
    pub min_hz: f64,
    pub max_hz: f64,
    pub tunes: Vec<f64>,
    pub waits: u64,
    pub events: Vec<Event>,
    pub requests: Vec<HostRequest>,
    frames: Frames,
    input: Input,
    step: Option<Step>,
}

impl Driver {
    pub fn new(radio: FakeRadio) -> Self {
        Driver {
            radio,
            radio_sweeps: false,
            can_sweep: false,
            min_hz: 24e6,
            max_hz: 1766e6,
            tunes: Vec::new(),
            waits: 0,
            events: Vec::new(),
            requests: Vec::new(),
            frames: Frames::new(),
            input: Input::Spectrum,
            step: None,
        }
    }

    pub fn info(&self) -> RadioInfo {
        RadioInfo {
            min_hz: self.min_hz,
            max_hz: self.max_hz,
            rate_hz: RATE,
            can_sweep: self.can_sweep,
            tuned_hz: self.radio.centre_hz(),
        }
    }

    /// Runs the command start with `args` and the session "1", then the first step.
    pub fn start(&mut self, svc: &mut SweepService, args: &[(&str, Value)]) {
        let mut all = vec![("session", text("1"))];
        all.extend(args.iter().cloned());
        call(svc, "start", &all).unwrap();
        self.step = svc.step(self.radio.time_s(), &self.info()).unwrap();
        assert!(self.step.is_some(), "the first step");
    }

    /// The step that the engine must apply next, taken.
    pub fn step_now(&mut self) -> Option<Step> {
        self.step.take()
    }

    /// Drives the sweep until it is done or the radio reaches `max_s`. Returns where the sweep tunes back to.
    pub fn run(&mut self, svc: &mut SweepService, max_s: f64) -> Option<f64> {
        self.run_with(svc, max_s, |_, _| {})
    }

    /// As [`Driver::run`], with `each` called after every feed.
    pub fn run_with(
        &mut self,
        svc: &mut SweepService,
        max_s: f64,
        mut each: impl FnMut(&mut SweepService, &FakeRadio),
    ) -> Option<f64> {
        let mut buf = vec![Complex32::default(); 2048];
        let mut block = vec![Complex32::default(); 4096];
        let mut sweep: Option<(f64, f64, f64, f64)> = None;
        while self.radio.time_s() < max_s {
            if let Some(s) = self.step.take() {
                match s {
                    Step::Tune {
                        centre_hz,
                        rate_hz,
                        input,
                    } => {
                        if input == Input::Spectrum {
                            self.tunes.push(centre_hz);
                        }
                        self.input = input;
                        sweep = None;
                        self.radio.tune(centre_hz, rate_hz);
                        self.frames.retune();
                    }
                    Step::RadioSweep {
                        low_hz,
                        high_hz,
                        rate_hz,
                    } => {
                        if self.radio_sweeps {
                            let first = low_hz + 0.125 * rate_hz;
                            self.radio.tune(first, rate_hz);
                            self.frames.retune();
                            self.input = Input::Spectrum;
                            sweep = Some((low_hz, high_hz, rate_hz, first));
                        } else {
                            self.step = svc.radio_sweep_failed();
                            continue;
                        }
                    }
                    Step::Wait { until_s } => {
                        self.waits += 1;
                        let now = self.radio.time_s();
                        if until_s > now {
                            self.radio.skip(until_s - now);
                        }
                        self.step = svc.step(self.radio.time_s(), &self.info()).unwrap();
                        continue;
                    }
                    Step::Done { return_hz } => {
                        svc.take_events(&mut self.events);
                        self.requests.extend(svc.take_requests());
                        return Some(return_hz);
                    }
                }
            }
            let iq: &[Complex32] = if sweep.is_some() {
                self.radio.read(&mut block);
                &block
            } else {
                self.radio.read(&mut buf);
                &buf
            };
            let centre = self.radio.centre_hz();
            let now = self.radio.time_s();
            if self.input == Input::Samples {
                self.step = svc.feed_samples(now, centre, iq);
            } else if let Some(need) = svc.spectrum() {
                let step = &mut self.step;
                self.frames.push(need, iq, |bins| {
                    let frame = SpectrumFrame {
                        time_s: now,
                        centre_hz: centre,
                        span_hz: need.rate_hz,
                        bins_db: bins,
                    };
                    *step = svc.feed(&frame).unwrap();
                    step.is_none()
                });
            }
            if let Some((low, high, rate, c)) = sweep {
                let mut next = c + 0.25 * rate;
                if next - 0.125 * rate > high {
                    next = low + 0.125 * rate;
                }
                if self.step.is_none() {
                    self.radio.tune(next, rate);
                    self.frames.retune();
                    sweep = Some((low, high, rate, next));
                }
            }
            each(svc, &self.radio);
            if self.step.is_none() {
                self.step = svc.step(self.radio.time_s(), &self.info()).unwrap();
            }
        }
        svc.take_events(&mut self.events);
        None
    }
}
