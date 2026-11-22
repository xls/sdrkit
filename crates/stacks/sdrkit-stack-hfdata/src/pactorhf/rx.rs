//! The PACTOR-II/III/IV components: the audio buffer that runs the analyser, and the results.

use super::analyse::{analyse, Analysis, CarrierInfo, FS};
use crate::typed::{
    PactorhfAnalysisEvent, PactorhfCarrierRow, PactorhfEventRow, PactorhfFields, PACTORHF_ANALYSIS,
    PACTORHF_CARRIERS, PACTORHF_EVENTS,
};
use crate::util::{level_points, update_points, Origin};
use sdrkit_core::command::{CommandError, CommandId, CommandSet, Commands};
use sdrkit_core::interface::{Fields as _, Hertz, Seconds, Time};
use sdrkit_core::publish::keys::Family;
use sdrkit_core::publish::{field, Published};
use sdrkit_core::results::SeriesId;
use sdrkit_core::{
    ConfigError, Event, LockState, Record, Results, ResultsDecl, Setting, SettingError, SettingId,
    SettingSet, StateBlob, StateError, Status, Value, Values,
};
use sdrkit_graph::{
    values_of, Applied, Component, ComponentInfo, Format, Io, Item, Kind, Port, RunContext,
    TagKind, Timed,
};
use sdrkit_util_objects::commands::{self as objcmd, Tables};
use sdrkit_util_objects::shaped::{self, RowOut, Shaped};
use sdrkit_util_objects::{Format as StateFormat, LogRing, Seen, Shift};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::sync::Arc;

impl Item for Analysis {
    const KIND: &'static str = "pactorhf.analysis";
}

/// What the buffer sends: an analysis, the level of the last second, a reset.
#[derive(Clone, Debug, PartialEq)]
pub enum BufferEvent {
    /// An analysis of the buffer at signal time `at`.
    Analysis {
        /// Signal time, seconds.
        at: f64,
        /// What the analyser found.
        analysis: Box<Analysis>,
    },
    /// The level of the last second, dB.
    Level(f32),
    /// The buffer was emptied (a retune).
    Reset,
}

impl Item for BufferEvent {
    const KIND: &'static str = "pactorhf.analysis";
}

const BUF: usize = 40 * 8000;

/// The analysis interval.
pub const INTERVAL: Setting<field::Duration> =
    Setting::new("analysis_interval_s", "Analysis interval", "s", 4.0)
        .range(0.5, 60.0, 0.5)
        .section("Advanced");

sdrkit_core::settings! {
    /// The settings of [`AudioBuffer`].
    pub enum AudioBufferSetting {
        /// Analysis interval.
        Interval = INTERVAL,
    }
}

/// `hfdata.pactorhf_buffer`: the newest 40 s of 8 kHz audio. Every interval (4 s) it runs the analyser on what it holds (at
/// least 3 s).
pub struct AudioBuffer {
    asked: u8,
    ring: Vec<f32>,
    n: u64,
    have: usize,
    interval: f64,
    next: u64,
    lvl: (f64, u64),
    copy: Vec<f32>,
    out: Vec<BufferEvent>,
}

impl AudioBuffer {
    /// A buffer that analyses for the variant `asked` (2, 3 or 4).
    pub fn new(asked: u8) -> Self {
        AudioBuffer {
            asked,
            ring: vec![0.0; BUF],
            n: 0,
            have: 0,
            interval: 4.0,
            next: 4 * 8000,
            lvl: (0.0, 0),
            copy: Vec::new(),
            out: Vec::new(),
        }
    }
    fn run(&mut self) {
        if self.have < 3 * 8000 {
            return;
        }
        self.copy.clear();
        let start = self.n - self.have as u64;
        for i in start..self.n {
            self.copy.push(self.ring[(i % BUF as u64) as usize]);
        }
        let a = analyse(&self.copy, self.asked);
        self.out.push(BufferEvent::Analysis {
            at: self.n as f64 / FS,
            analysis: Box::new(a),
        });
    }
    fn push(&mut self, x: &[f32]) {
        for &v in x {
            self.ring[(self.n % BUF as u64) as usize] = v;
            self.n += 1;
            self.have = (self.have + 1).min(BUF);
            self.lvl.0 += (v * v) as f64;
            self.lvl.1 += 1;
            if self.lvl.1 >= 8000 {
                let db = 10.0 * (self.lvl.0 / self.lvl.1 as f64).max(1e-15).log10();
                self.out.push(BufferEvent::Level(db as f32));
                self.lvl = (0.0, 0);
            }
            if self.n >= self.next {
                self.next = self.n + (self.interval * FS) as u64;
                self.run();
            }
        }
    }
}

impl Component for AudioBuffer {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "hfdata.pactorhf_buffer",
            vec![Port::real("in")],
            vec![Port::typed::<BufferEvent>("out")],
        )
        .with_settings(AudioBufferSetting::schema())
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        self.have = 0;
        // The results need the tuned frequency of the input.
        let mut f = Format::new(Kind::typed::<BufferEvent>(), 0.0);
        f.tuned_hz = inputs[0].tuned_hz;
        Ok(vec![f])
    }
    fn process(&mut self, io: &mut Io) {
        let x = io.real(0);
        let first = io.input(0).first();
        let mut start = 0;
        for t in io.input(0).tags() {
            if let TagKind::Retuned { .. } = t.kind {
                let at = (t.at.saturating_sub(first) as usize).min(x.len());
                self.push(&x[start..at]);
                start = at;
                self.have = 0;
                self.out.push(BufferEvent::Reset);
            }
        }
        self.push(&x[start..]);
        let o = io.items_out::<BufferEvent>(0);
        for e in self.out.drain(..) {
            o.push(Timed {
                time: 0.0,
                value: e,
            });
        }
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        match AudioBufferSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            AudioBufferSetting::Interval => self.interval = INTERVAL.read(id, v)?,
        }
        self.next = self.n + (self.interval * FS) as u64;
        Ok(Applied::Live)
    }
    fn values(&self) -> Values {
        values_of([(AudioBufferSetting::Interval, Value::Duration(self.interval))])
    }
}

sdrkit_core::choice_enum! {
    /// The differential phase modulation of a carrier.
    pub enum Modulation {
        /// Two phase steps.
        DBpsk = ("d_bpsk", "D-BPSK"),
        /// Four phase steps.
        DQpsk = ("d_qpsk", "D-QPSK"),
        /// Eight phase steps.
        D8psk = ("d_8psk", "D-8PSK"),
        /// Sixteen phase steps.
        D16psk = ("d_16psk", "D-16PSK"),
    }
}

/// The series of the audio level, one point per second.
pub const LEVEL_SERIES: &str = "level";

/// The events kept.
const EVENTS_CAP: usize = 60;
const LEVEL_CAP: usize = 120;

/// What the PACTOR-II, -III and -IV stacks publish.
pub fn declaration() -> ResultsDecl {
    ResultsDecl::new()
        .with_fields(PactorhfFields::SHAPE.fields)
        .with_table(&PACTORHF_CARRIERS)
        .with_table(&PACTORHF_EVENTS)
        .series(LEVEL_SERIES, "Level", "s", "dB")
        .event(&PACTORHF_ANALYSIS)
}

/// One event.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct EventRow {
    /// Signal time, seconds.
    pub t_s: f64,
    /// UTC, when the clock is known.
    pub utc: Option<f64>,
    /// The text.
    pub text: String,
}

impl Shift for EventRow {
    fn shift_times(&mut self, by_s: f64) {
        self.t_s += by_s;
    }
}

impl Shaped for EventRow {
    type Row = PactorhfEventRow<'static>;
    fn row(&self, _: Option<&Seen>, _: f64, out: RowOut<'_, '_, Self::Row>) {
        out.put(&PactorhfEventRow {
            start: Seconds(self.t_s),
            time: self.utc.map(Time),
            text: &self.text,
        });
    }
}

/// Writes the cells of a carrier row.
fn carrier_row(c: &CarrierInfo) -> PactorhfCarrierRow {
    PactorhfCarrierRow {
        frequency: Hertz(c.hz),
        line_db: c.line_db,
        steps: i64::from(c.order),
        modulation: modulation(c.order),
        concentration_pct: c.concentration * 100.0,
    }
}

/// What the stack keeps over a restart: the events.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PactorHfLists {
    /// The events, oldest first.
    pub events: LogRing<EventRow>,
}

impl Default for PactorHfLists {
    fn default() -> Self {
        PactorHfLists {
            events: LogRing::new(EVENTS_CAP),
        }
    }
}

impl Shift for PactorHfLists {
    fn shift_times(&mut self, by_s: f64) {
        self.events.shift_times(by_s);
    }
}

impl Tables for PactorHfLists {
    const FORMAT: StateFormat = StateFormat::new("hfdata.pactorhf_lists", 1);
    fn clear(&mut self) {
        self.events.clear();
    }
    fn adopt(&mut self, saved: Self) {
        self.events.adopt(saved.events);
    }
}

fn name(v: u8) -> &'static str {
    match v {
        2 => "PACTOR-II",
        3 => "PACTOR-III",
        4 => "PACTOR-IV",
        _ => "PACTOR",
    }
}

fn modulation(order: u32) -> Option<Modulation> {
    match order {
        2 => Some(Modulation::DBpsk),
        4 => Some(Modulation::DQpsk),
        8 => Some(Modulation::D8psk),
        16 => Some(Modulation::D16psk),
        _ => None,
    }
}

/// The row key of carrier `i`.
fn carrier_key(i: usize) -> String {
    format!("{i:02}")
}

sdrkit_core::commands! {
    /// The commands of [`PactorHfResults`].
    pub enum PactorHfCommand {
        /// Forget the events.
        Clear = objcmd::CLEAR,
    }
}

/// The digits of the newest 200 phase steps of `symbols`.
fn digits(symbols: &[u8]) -> String {
    let k = symbols.len().saturating_sub(200);
    symbols[k..]
        .iter()
        .map(|&s| char::from_digit(s as u32 % 16, 16).unwrap_or('0'))
        .collect()
}

/// `hfdata.pactorhf_results`: the results. `pactorhf.analysis` items in, `pactorhf.analysis` events out.
pub struct PactorHfResults {
    asked: u8,
    p: Published,
    level_series: SeriesId,
    carriers_shown: usize,
    origin: Origin,
    live: bool,
    tuned_hz: f64,
    analysis: Option<Analysis>,
    analyses: u64,
    with_signal: u64,
    lists: PactorHfLists,
    level: VecDeque<f32>,
    last_sig: Option<(u8, usize)>,
    last_found_t: Option<f64>,
    gone: bool,
    now: f64,
    configured: bool,
}

impl PactorHfResults {
    /// The results of the stack for the variant `asked` (2, 3 or 4).
    pub fn new(asked: u8) -> Self {
        let decl = Arc::new(declaration());
        let level_series = decl
            .series_id(LEVEL_SERIES)
            .expect("the declaration holds the level series");
        PactorHfResults {
            asked,
            p: Published::from_decl(decl),
            level_series,
            carriers_shown: 0,
            origin: Origin::default(),
            live: false,
            tuned_hz: 0.0,
            analysis: None,
            analyses: 0,
            with_signal: 0,
            lists: PactorHfLists::default(),
            level: VecDeque::with_capacity(LEVEL_CAP),
            last_sig: None,
            last_found_t: None,
            gone: false,
            now: 0.0,
            configured: false,
        }
    }

    fn take(&mut self, at: f64, a: &Analysis, out: &mut Vec<Event>) {
        self.analyses += 1;
        if a.found {
            self.with_signal += 1;
            self.last_found_t = Some(at);
            self.gone = false;
            let sig = (a.variant, a.carrier_count);
            if self.last_sig != Some(sig) {
                self.last_sig = Some(sig);
                let text = if a.variant == 4 {
                    let c = match a.constellation {
                        2 => "BPSK (estimate)",
                        4 => "4-PSK (estimate)",
                        8 => "8-PSK (estimate)",
                        16 => "QAM (estimate)",
                        _ => "unknown",
                    };
                    format!(
                        "PACTOR-IV signal: 1800 baud, centre {:.0} Hz, clock line {:.0} dB, constellation {c}",
                        a.centre_hz, a.clock_line_db
                    )
                } else {
                    let mut t = format!(
                        "{} signal: {} carriers, centre {:.0} Hz, spacing {:.0} Hz, {:.2} baud",
                        name(a.variant),
                        a.carrier_count,
                        a.centre_hz,
                        a.spacing_hz,
                        a.baud
                    );
                    if a.variant == 3 {
                        t.push_str(&format!(", speed level {}", a.level));
                    }
                    t
                };
                out.push(PACTORHF_ANALYSIS.event(
                    at,
                    self.tuned_hz,
                    &PactorhfAnalysisEvent {
                        variant: i64::from(a.variant),
                        text: &text,
                        family: Family::Other,
                    },
                ));
                self.push_event(at, text);
            }
        } else if let Some(t) = self.last_found_t {
            if at - t >= 12.0 && !self.gone {
                self.gone = true;
                self.last_sig = None;
                self.push_event(at, "signal gone".into());
            }
        }
        self.analysis = Some(a.clone());
    }

    fn push_event(&mut self, t: f64, text: String) {
        self.lists.events.push(EventRow {
            t_s: t,
            utc: self.origin.wall(t),
            text,
        });
    }

    fn publish(&mut self) {
        let a = self.analysis.clone().unwrap_or_default();
        let asked = self.asked;
        let own = a.found && a.variant == asked;
        let summary = if !a.found {
            format!("{}: no signal", name(asked))
        } else if !own {
            format!("{}: this looks like {}", name(asked), name(a.variant))
        } else if a.variant == 4 {
            format!(
                "PACTOR-IV: 1800 baud single carrier, constellation {}",
                if a.constellation != 0 {
                    "estimated"
                } else {
                    "unknown"
                }
            )
        } else if a.frame.found {
            format!(
                "{}: {} carriers, frame {:.2} s",
                name(asked),
                a.carrier_count,
                a.frame.period_s
            )
        } else {
            format!(
                "{}: {} carriers at {:.0} baud",
                name(asked),
                a.carrier_count,
                a.baud
            )
        };
        let offset = a
            .found
            .then_some(a.centre_hz - if a.variant == 2 { 885.0 } else { 1500.0 });
        let found = |v: f64| a.found.then_some(v);
        let fr = &a.frame;
        let row = |i: usize| a.carriers.get(i).map(|c| digits(&c.symbols));
        let p = &mut self.p;
        let rows = [0, 1, 2].map(|i| row(i).unwrap_or_default());
        p.put(&PactorhfFields {
            signal: a.found,
            variant: a.variant as i64,
            other_variant: a.found && !own,
            symbol_rate_hz: found(a.baud).map(Hertz),
            clock_ppm: found(a.clock_ppm),
            centre: found(a.centre_hz).map(Hertz),
            offset_hz: offset.map(Hertz),
            spacing: found(a.spacing_hz).map(Hertz),
            bandwidth: found(a.bandwidth_hz).map(Hertz),
            clock_line_db: self.analysis.as_ref().map(|a| a.clock_line_db),
            carrier_count: a.carrier_count as i64,
            speed_level: a.level as i64,
            constellation: a.constellation as i64,
            symbol_count: a.symbol_count as i64,
            stretch_start: found(a.start_s).map(Seconds),
            stretch_length: found(a.length_s).map(Seconds),
            frame_found: fr.found,
            frame_symbols: fr.found.then_some(fr.period_symbols as i64),
            frame_period: fr.found.then_some(Seconds(fr.period_s)),
            frame_copies: fr.found.then_some(fr.copies as i64),
            frame_agreement_pct: fr.found.then_some(fr.agreement * 100.0),
            constant_start: fr.found.then_some(fr.const_start as i64),
            constant_symbols: fr.found.then_some(fr.const_len as i64),
            constant_hex: &fr.const_hex,
            note: &a.note,
            symbols_1: &rows[0],
            symbols_2: &rows[1],
            symbols_3: &rows[2],
            analyses: self.analyses as i64,
            analyses_with_signal: self.with_signal as i64,
            level_dbfs: self.level.back().map(|&v| v as f64),
            clock: self.origin.source(),
        });
        for (i, c) in a.carriers.iter().enumerate() {
            PACTORHF_CARRIERS.put(p, &carrier_key(i), &carrier_row(c));
        }
        for i in a.carriers.len()..self.carriers_shown {
            p.remove_row(&PACTORHF_CARRIERS, &carrier_key(i));
        }
        self.carriers_shown = a.carriers.len();
        shaped::log_rows(&self.lists.events, &PACTORHF_EVENTS, self.now, p);
        let lock_state = if !a.found {
            LockState::Searching
        } else if a.frame.found {
            LockState::Locked
        } else {
            LockState::Tracking
        };
        let r = p.results_mut();
        r.set_status(&Status {
            lock_state,
            snr_db: a.found.then_some(a.clock_line_db as f32),
            level_dbfs: self.level.back().copied(),
            offset_hz: offset,
            summary,
        });
        update_points(r, self.level_series, true, level_points(&self.level));
    }
}

impl Component for PactorHfResults {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "hfdata.pactorhf_results",
            vec![Port::typed::<BufferEvent>("in")],
            vec![Port::new("events", Kind::Messages)],
        )
    }
    fn attach(&mut self, ctx: &RunContext) {
        self.live = ctx.wall_clock();
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        self.origin = Origin::new(self.live);
        self.tuned_hz = inputs[0].tuned_hz;
        if !self.configured {
            self.p.results_mut().log(match self.asked {
                2 => "PACTOR-II: two D-PSK carriers 200 Hz apart, 100 baud, upper sideband (physical layer only: the channel coding is proprietary)",
                3 => "PACTOR-III: 2 .. 18 D-PSK carriers on a 120 Hz grid, 100 baud, upper sideband (physical layer only: the channel coding is proprietary)",
                _ => "PACTOR-IV: one PSK carrier at 1500 Hz, 1800 baud, upper sideband (physical layer only: the channel coding is proprietary)",
            });
        }
        self.configured = true;
        self.publish();
        Ok(vec![Format::new(Kind::Messages, 0.0)])
    }
    fn process(&mut self, io: &mut Io) {
        let mut evs: Vec<Event> = Vec::new();
        let mut changed = false;
        for e in io.items::<BufferEvent>(0) {
            match &e.value {
                BufferEvent::Analysis { at, analysis } => {
                    self.now = self.now.max(*at);
                    self.take(*at, analysis, &mut evs);
                    changed = true;
                }
                BufferEvent::Level(l) => {
                    if self.level.len() >= LEVEL_CAP {
                        self.level.pop_front();
                    }
                    self.level.push_back(*l);
                    self.now += 1.0;
                }
                BufferEvent::Reset => {
                    self.analysis = None;
                    changed = true;
                }
            }
        }
        io.messages_out(0).extend(evs);
        if changed || io.tick() {
            self.publish();
        }
    }
    fn values(&self) -> Values {
        sdrkit_graph::no_values()
    }
    fn commands(&self) -> &Commands {
        PactorHfCommand::commands()
    }
    fn invoke(&mut self, id: CommandId, _: &Values) -> Result<Option<Record>, CommandError> {
        match PactorHfCommand::from_id(id).ok_or(CommandError::NoSuchId { id })? {
            PactorHfCommand::Clear => self.lists.clear(),
        }
        self.publish();
        Ok(None)
    }
    fn results(&self) -> Option<&Results> {
        Some(self.p.results())
    }
    fn save_state(&self) -> Option<StateBlob> {
        self.lists.save_state(self.now).ok()
    }
    fn restore_state(&mut self, state: &StateBlob) -> Result<(), StateError> {
        self.lists.restore_state(state, self.now)?;
        self.publish();
        Ok(())
    }
}
