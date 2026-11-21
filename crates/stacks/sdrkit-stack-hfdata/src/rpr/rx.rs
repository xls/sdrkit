//! The Robust Packet components: the audio buffer that runs the analyser, and the results.

use super::analyse::{analyse, hex, Analysis, Burst, Scratch, FS};
use super::{AUDIO_CENTRE, CARRIERS, CENTRE_HZ};
use crate::pactorhf::rx::INTERVAL;
use crate::typed::{
    RprBurstEvent, RprBurstRow, RprCarrierRow, RprEventRow, RprFields, RPR_BURST, RPR_BURSTS,
    RPR_CARRIERS, RPR_EVENTS,
};
use crate::util::{level_points, update_points, Origin};
use sdrkit_core::command::{CommandError, CommandId, CommandSet, Commands};
use sdrkit_core::interface::{Fields as _, Hertz, Seconds, Time};
use sdrkit_core::publish::keys::Family;
use sdrkit_core::publish::Published;
use sdrkit_core::results::SeriesId;
use sdrkit_core::{
    Choice, ConfigError, Event, LockState, Record, Results, ResultsDecl, SettingError, SettingId,
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

sdrkit_core::choice_enum! {
    /// The differential modulation of a burst.
    pub enum RprModulation {
        /// Two phase steps: speed level 1.
        DBpsk = ("d_bpsk", "D-BPSK"),
        /// Four phase steps: speed level 2.
        DQpsk = ("d_qpsk", "D-QPSK"),
    }
}

sdrkit_core::choice_enum! {
    /// What the last analysis found.
    pub enum RprState {
        /// No comb.
        Nothing = ("nothing", "No signal"),
        /// The comb, no burst within the buffer.
        Comb = ("comb", "Comb found"),
        /// Bursts demodulated within the buffer.
        Bursts = ("bursts", "Bursts demodulated"),
    }
}

impl RprModulation {
    /// The modulation of a step order (2 or 4).
    pub fn of_order(order: u32) -> Self {
        if order >= 4 {
            RprModulation::DQpsk
        } else {
            RprModulation::DBpsk
        }
    }
    /// The speed level: 1 or 2.
    pub fn level(self) -> i64 {
        match self {
            RprModulation::DBpsk => 1,
            RprModulation::DQpsk => 2,
        }
    }
    /// The net rate of the speed level before the AX.25 overhead, bit/s: 8 carriers x 50 Bd x bits x code rate (1/2 or 3/4).
    pub fn net_rate(self) -> i64 {
        match self {
            RprModulation::DBpsk => 200,
            RprModulation::DQpsk => 600,
        }
    }
    /// The bits of a step index.
    pub fn bits(self) -> u32 {
        match self {
            RprModulation::DBpsk => 1,
            RprModulation::DQpsk => 2,
        }
    }
}

/// What the buffer sends.
#[derive(Clone, Debug, PartialEq)]
pub enum BufferEvent {
    /// An analysis at signal time `at` of audio that starts at signal time `start`, with the comb searched around `nominal_hz`.
    Analysis {
        /// Signal time of the end of the audio, seconds.
        at: f64,
        /// Signal time of the first sample analysed, seconds.
        start: f64,
        /// The nominal comb centre, Hz.
        nominal_hz: f64,
        /// What the analyser found.
        analysis: Box<Analysis>,
    },
    /// The level of the last second, dB.
    Level(f32),
    /// The buffer was emptied (a retune).
    Reset,
}

impl Item for BufferEvent {
    const KIND: &'static str = "rpr.analysis";
}

/// The audio kept: 30 s.
const BUF: usize = 30 * 8000;
/// The least audio analysed: 3 s.
const LEAST: usize = 3 * 8000;
/// A burst that starts within this time of the last one listed is the same burst, seconds.
const SAME_S: f64 = 0.3;

sdrkit_core::settings! {
    /// The settings of [`RprBuffer`].
    pub enum RprBufferSetting {
        /// Analysis interval.
        Interval = INTERVAL,
        /// Audio centre.
        AudioCentre = AUDIO_CENTRE,
    }
}

/// `hfdata.rpr_buffer`: the newest 30 s of 8 kHz audio. Every interval (4 s) it runs the analyser on what it holds (at least 3
/// s), and sends the bursts that start after the last one sent.
pub struct RprBuffer {
    ring: Vec<f32>,
    n: u64,
    have: usize,
    /// The sample count at the last reset: the buffer holds the audio from there while `n - have` equals it.
    since: u64,
    interval: f64,
    centre_hz: f64,
    next: u64,
    lvl: (f64, u64),
    /// The signal time of the start of the last burst sent.
    last_burst: Option<f64>,
    copy: Vec<f32>,
    scratch: Scratch,
    out: Vec<BufferEvent>,
}

impl Default for RprBuffer {
    fn default() -> Self {
        RprBuffer {
            ring: vec![0.0; BUF],
            n: 0,
            have: 0,
            since: 0,
            interval: 4.0,
            centre_hz: CENTRE_HZ,
            next: 4 * 8000,
            lvl: (0.0, 0),
            last_burst: None,
            copy: Vec::new(),
            scratch: Scratch::default(),
            out: Vec::new(),
        }
    }
}

impl RprBuffer {
    fn run(&mut self) {
        if self.have < LEAST {
            return;
        }
        self.copy.clear();
        let first = self.n - self.have as u64;
        for i in first..self.n {
            self.copy.push(self.ring[(i % BUF as u64) as usize]);
        }
        let start = first as f64 / FS;
        let after = self.last_burst.map_or(0.0, |t| t + SAME_S - start);
        let a = analyse(
            &self.copy,
            self.centre_hz,
            after,
            first == self.since,
            &mut self.scratch,
        );
        if let Some(b) = a.bursts.last() {
            self.last_burst = Some(start + b.start_s);
        }
        self.out.push(BufferEvent::Analysis {
            at: self.n as f64 / FS,
            start,
            nominal_hz: self.centre_hz,
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
    fn reset(&mut self) {
        self.have = 0;
        self.since = self.n;
        self.out.push(BufferEvent::Reset);
    }
}

impl Component for RprBuffer {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "hfdata.rpr_buffer",
            vec![Port::real("in")],
            vec![Port::typed::<BufferEvent>("out")],
        )
        .with_settings(RprBufferSetting::schema())
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        self.have = 0;
        self.since = self.n;
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
                self.reset();
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
        match RprBufferSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            RprBufferSetting::Interval => {
                self.interval = INTERVAL.read(id, v)?;
                self.next = self.n + (self.interval * FS) as u64;
            }
            RprBufferSetting::AudioCentre => self.centre_hz = AUDIO_CENTRE.read(id, v)?,
        }
        Ok(Applied::Live)
    }
    fn values(&self) -> Values {
        values_of([
            (RprBufferSetting::Interval, Value::Duration(self.interval)),
            (RprBufferSetting::AudioCentre, Value::Hz(self.centre_hz)),
        ])
    }
}

/// The series of the audio level, one point per second.
pub const LEVEL_SERIES: &str = "level";

const LIST_CAP: usize = 60;
const LEVEL_CAP: usize = 120;
/// The step indices shown per carrier row.
const ROW_STEPS: usize = 200;
/// The symbol times of the hexadecimal column of the bursts table.
const HEX_TIMES: usize = 24;

/// What the Robust Packet stack publishes.
pub fn declaration() -> ResultsDecl {
    ResultsDecl::new()
        .with_fields(RprFields::SHAPE.fields)
        .with_table(&RPR_CARRIERS)
        .with_table(&RPR_BURSTS)
        .with_table(&RPR_EVENTS)
        .series(LEVEL_SERIES, "Level", "s", "dB")
        .event(&RPR_BURST)
}

/// A burst listed.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct BurstRow {
    /// Signal time of the start, seconds.
    pub t_s: f64,
    /// UTC of the start, when the clock is known.
    pub utc: Option<f64>,
    pub length_s: f64,
    /// The order of the phase steps: 2 or 4.
    pub order: u32,
    pub carriers: usize,
    pub symbols: usize,
    pub snr_db: f64,
    pub offset_hz: f64,
    /// The first step indices as hexadecimal.
    pub first_steps: String,
}

impl Shift for BurstRow {
    fn shift_times(&mut self, by_s: f64) {
        self.t_s += by_s;
    }
}

impl Shaped for BurstRow {
    type Row = RprBurstRow<'static>;
    fn row(&self, _: Option<&Seen>, _: f64, out: RowOut<'_, '_, Self::Row>) {
        let modulation = RprModulation::of_order(self.order);
        out.put(&RprBurstRow {
            start: Seconds(self.t_s),
            time: self.utc.map(Time),
            length: Seconds(self.length_s),
            speed_level: modulation.level(),
            modulation,
            net_rate: modulation.net_rate(),
            carriers: self.carriers as i64,
            symbols: self.symbols as i64,
            snr_db: self.snr_db,
            offset: Hertz(self.offset_hz),
            first_steps: &self.first_steps,
        });
    }
}

/// One event of the log.
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
    type Row = RprEventRow<'static>;
    fn row(&self, _: Option<&Seen>, _: f64, out: RowOut<'_, '_, Self::Row>) {
        out.put(&RprEventRow {
            start: Seconds(self.t_s),
            time: self.utc.map(Time),
            text: &self.text,
        });
    }
}

/// What the stack keeps over a restart: the bursts and the events.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RprLists {
    /// The bursts, oldest first.
    pub bursts: LogRing<BurstRow>,
    /// The events, oldest first.
    pub events: LogRing<EventRow>,
}

impl Default for RprLists {
    fn default() -> Self {
        RprLists {
            bursts: LogRing::new(LIST_CAP),
            events: LogRing::new(LIST_CAP),
        }
    }
}

impl Shift for RprLists {
    fn shift_times(&mut self, by_s: f64) {
        self.bursts.shift_times(by_s);
        self.events.shift_times(by_s);
    }
}

impl Tables for RprLists {
    const FORMAT: StateFormat = StateFormat::new("hfdata.rpr_lists", 1);
    fn clear(&mut self) {
        self.bursts.clear();
        self.events.clear();
    }
    fn adopt(&mut self, saved: Self) {
        self.bursts.adopt(saved.bursts);
        self.events.adopt(saved.events);
    }
}

sdrkit_core::commands! {
    /// The commands of [`RprResults`].
    pub enum RprCommand {
        /// Forget the bursts and the events.
        Clear = objcmd::CLEAR,
    }
}

/// The last burst as the fields show it.
#[derive(Clone, Debug, Default)]
struct Last {
    burst: Burst,
    offset_hz: f64,
    end_s: f64,
}

/// The newest step indices of carrier `c` of `steps`, one digit each.
fn row_digits(steps: &[u8], c: usize) -> String {
    let n = steps.len() / CARRIERS;
    let k = n.saturating_sub(ROW_STEPS);
    (k..n)
        .map(|m| char::from_digit(u32::from(steps[m * CARRIERS + c]), 10).unwrap_or('0'))
        .collect()
}

/// `hfdata.rpr_results`: the results. `rpr.analysis` items in, `rpr.burst` events out.
pub struct RprResults {
    p: Published,
    level_series: SeriesId,
    origin: Origin,
    live: bool,
    tuned_hz: f64,
    /// The comb of the last analysis: centre and offset from the nominal centre, Hz, and the contrast.
    comb: Option<(f64, f64, f64)>,
    comb_carriers: usize,
    last: Option<Last>,
    best_snr: Option<f64>,
    analyses: u64,
    detections: u64,
    bursts: u64,
    lists: RprLists,
    level: VecDeque<f32>,
    had_comb: bool,
    now: f64,
    configured: bool,
}

impl Default for RprResults {
    fn default() -> Self {
        let decl = Arc::new(declaration());
        let level_series = decl
            .series_id(LEVEL_SERIES)
            .expect("the declaration holds the level series");
        RprResults {
            p: Published::from_decl(decl),
            level_series,
            origin: Origin::default(),
            live: false,
            tuned_hz: 0.0,
            comb: None,
            comb_carriers: 0,
            last: None,
            best_snr: None,
            analyses: 0,
            detections: 0,
            bursts: 0,
            lists: RprLists::default(),
            level: VecDeque::with_capacity(LEVEL_CAP),
            had_comb: false,
            now: 0.0,
            configured: false,
        }
    }
}

impl RprResults {
    fn take(&mut self, at: f64, start: f64, nominal_hz: f64, a: &Analysis, out: &mut Vec<Event>) {
        self.analyses += 1;
        let Some(comb) = &a.comb else {
            if self.had_comb {
                self.had_comb = false;
                self.push_event(at, "no signal any more".into());
            }
            self.comb = None;
            self.comb_carriers = 0;
            return;
        };
        self.detections += 1;
        let offset = comb.centre_hz - nominal_hz;
        if !self.had_comb {
            self.had_comb = true;
            self.push_event(
                at,
                format!("comb found at {:.0} Hz ({offset:+.0} Hz)", comb.centre_hz),
            );
        }
        self.comb = Some((comb.centre_hz, offset, comb.contrast));
        self.comb_carriers = comb.carriers_seen;
        for b in &a.bursts {
            let t = start + b.start_s;
            let m = RprModulation::of_order(b.order);
            let text = format!(
                "burst of {:.2} s, level {} ({}), {} symbols a carrier, SNR {:.0} dB",
                b.length_s,
                m.level(),
                m.label(),
                b.symbols,
                b.snr_db
            );
            out.push(RPR_BURST.event(
                t,
                self.tuned_hz,
                &RprBurstEvent {
                    speed_level: m.level(),
                    modulation: m,
                    carriers: b.carriers_seen as i64,
                    symbols: b.symbols as i64,
                    length: Seconds(b.length_s),
                    snr_db: b.snr_db,
                    offset_hz: Hertz(offset),
                    steps: Arc::from(b.steps.as_slice()),
                    text: &text,
                    family: Family::Message,
                },
            ));
            self.lists.bursts.push(BurstRow {
                t_s: t,
                utc: self.origin.wall(t),
                length_s: b.length_s,
                order: b.order,
                carriers: b.carriers_seen,
                symbols: b.symbols,
                snr_db: b.snr_db,
                offset_hz: offset,
                first_steps: hex(&b.steps, m.bits(), HEX_TIMES),
            });
            self.push_event(t, text);
            self.bursts += 1;
            self.best_snr = Some(self.best_snr.map_or(b.snr_db, |s| s.max(b.snr_db)));
            self.last = Some(Last {
                burst: b.clone(),
                offset_hz: offset,
                end_s: t + b.length_s,
            });
        }
    }

    fn push_event(&mut self, t: f64, text: String) {
        self.lists.events.push(EventRow {
            t_s: t,
            utc: self.origin.wall(t),
            text,
        });
    }

    fn state(&self) -> RprState {
        match (&self.comb, &self.last) {
            (None, _) => RprState::Nothing,
            (Some(_), Some(l)) if self.now - l.end_s <= BUF as f64 / FS => RprState::Bursts,
            _ => RprState::Comb,
        }
    }

    fn publish(&mut self) {
        let state = self.state();
        let comb = self.comb;
        let last = self.last.as_ref();
        let lb = last.map(|l| &l.burst);
        let modulation = lb.map(|b| RprModulation::of_order(b.order));
        let summary = match (state, comb, modulation) {
            (RprState::Nothing, _, _) | (_, None, _) => "Robust Packet: no signal".to_string(),
            (RprState::Bursts, Some(_), Some(m)) => format!(
                "Robust Packet: speed level {} ({}) on {} carriers, {} bursts; frames not decoded (the channel coding is not public)",
                m.level(),
                m.label(),
                lb.map_or(0, |b| b.carriers_seen),
                self.bursts
            ),
            (_, Some((c, _, _)), _) => {
                format!("Robust Packet: comb of 8 carriers at {c:.0} Hz, no burst yet")
            }
        };
        let rows: [String; 3] = [0, 1, 2].map(|c| lb.map(|b| row_digits(&b.steps, c)).unwrap_or_default());
        let carriers = match (state, lb) {
            (RprState::Bursts, Some(b)) => b.carriers_seen,
            _ => self.comb_carriers,
        };
        let p = &mut self.p;
        p.put(&RprFields {
            state,
            comb_centre: comb.map(|c| Hertz(c.0)),
            offset_hz: comb.map(|c| Hertz(c.1)),
            comb_db: comb.map(|c| 10.0 * c.2.max(1e-12).log10()),
            carrier_count: carriers as i64,
            spacing: lb.map(|b| Hertz(b.spacing_hz)),
            symbol_rate_hz: lb.map(|b| Hertz(b.symbol_rate_hz)),
            clock_ppm: lb.and_then(|b| b.clock_ppm),
            speed_level: modulation.map_or(0, |m| m.level()),
            modulation,
            net_rate: modulation.map(|m| m.net_rate()),
            snr_db: lb.map(|b| b.snr_db),
            best_snr_db: self.best_snr,
            burst_length: lb.map(|b| Seconds(b.length_s)),
            symbols_1: &rows[0],
            symbols_2: &rows[1],
            symbols_3: &rows[2],
            note: "physical layer only: frames not decoded, the channel coding is not public",
            analyses: self.analyses as i64,
            detections: self.detections as i64,
            bursts: self.bursts as i64,
            level_dbfs: self.level.back().map(|&v| v as f64),
            clock: self.origin.source(),
        });
        if let Some(b) = lb {
            for c in 0..CARRIERS {
                RPR_CARRIERS.put(
                    p,
                    &c.to_string(),
                    &RprCarrierRow {
                        frequency: Hertz(b.carrier_hz[c]),
                        concentration_pct: b.concentration[c] * 100.0,
                    },
                );
            }
        } else {
            for c in 0..CARRIERS {
                p.remove_row(&RPR_CARRIERS, &c.to_string());
            }
        }
        shaped::log_rows(&self.lists.bursts, &RPR_BURSTS, self.now, p);
        shaped::log_rows(&self.lists.events, &RPR_EVENTS, self.now, p);
        let lock_state = match state {
            RprState::Nothing => LockState::Searching,
            RprState::Comb => LockState::Tracking,
            RprState::Bursts => LockState::Locked,
        };
        let offset = comb.map(|c| c.1);
        let snr = (state == RprState::Bursts)
            .then(|| last.map(|l| l.burst.snr_db as f32))
            .flatten();
        let r = p.results_mut();
        r.set_status(&Status {
            lock_state,
            snr_db: snr,
            level_dbfs: self.level.back().copied(),
            offset_hz: offset.or(last.map(|l| l.offset_hz)).filter(|_| comb.is_some()),
            summary,
        });
        update_points(r, self.level_series, true, level_points(&self.level));
    }
}

impl Component for RprResults {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "hfdata.rpr_results",
            vec![Port::typed::<BufferEvent>("in")],
            vec![Port::new("events", Kind::Messages)],
        )
    }
    fn attach(&mut self, ctx: &RunContext) {
        self.live = ctx.live;
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        self.origin = Origin::new(self.live);
        self.tuned_hz = inputs[0].tuned_hz;
        if !self.configured {
            self.p.results_mut().log(
                "Robust Packet: eight D-PSK carriers 60 Hz apart, 50 Bd, upper sideband (physical layer only: the channel coding is not public)",
            );
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
                BufferEvent::Analysis {
                    at,
                    start,
                    nominal_hz,
                    analysis,
                } => {
                    self.now = self.now.max(*at);
                    self.take(*at, *start, *nominal_hz, analysis, &mut evs);
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
                    self.comb = None;
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
        RprCommand::commands()
    }
    fn invoke(&mut self, id: CommandId, _: &Values) -> Result<Option<Record>, CommandError> {
        match RprCommand::from_id(id).ok_or(CommandError::NoSuchId { id })? {
            RprCommand::Clear => {
                self.lists.clear();
                self.best_snr = None;
                self.bursts = 0;
            }
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
