//! `tones.events`: the list of what the tone decoders heard, the counters, the fields and the state. The events of the decoders
//! leave the graph from their own outputs.

use super::front::{Demod, FrontReport};
use super::listen::GateState;
use super::spectrum::{Spectrum, POINTS, TOP_HZ};
use crate::typed::{TonesFields, TonesHeardRow, TONES_HEARD, TONES_SPECTRUM};
use sdrkit_core::interface::{Fields as _, Hertz, Seconds};
use sdrkit_core::publish::Published;
use sdrkit_core::{
    Choice, CommandError, CommandId, CommandSet, Commands, ConfigError, LockState, Record, Results,
    ResultsDecl, SeriesId, Values,
};
use sdrkit_graph::{no_values, Component, ComponentInfo, Format, Io, Port};
use sdrkit_proto_tones::components::SubAudioEvent;
use sdrkit_proto_tones::ctcss::CtcssEventKind;
use sdrkit_proto_tones::dcs::DcsEventKind;
use sdrkit_proto_tones::dtmf::DtmfEvent;
use sdrkit_proto_tones::selcal::SelcalCall;
use sdrkit_proto_tones::selcall::SelcallCall;
use sdrkit_proto_tones::twotone::TwoTonePage;
use sdrkit_util_ids::tones::selcall::TONE_SETS;
use sdrkit_util_objects::commands::CLEAR;
use sdrkit_util_objects::shaped::{self, RowOut, Shaped};
use sdrkit_util_objects::{LogRing, Seen};
use std::fmt::Write as _;
use std::sync::Arc;

/// The rows the list keeps.
pub const ROWS: usize = 300;
/// A DTMF key less than this long after the last one joins its row, s.
const DTMF_JOIN_S: f64 = 2.5;
/// A row counts as recent for the state this long, s.
const RECENT_S: f64 = 60.0;

/// What `tones.events` publishes: the fields, the list and the spectrum.
fn own_declaration() -> ResultsDecl {
    ResultsDecl::new()
        .with_fields(TonesFields::SHAPE.fields)
        .with_table(&TONES_HEARD)
        .series(
            TONES_SPECTRUM.key,
            TONES_SPECTRUM.label,
            TONES_SPECTRUM.x_unit,
            TONES_SPECTRUM.y_unit,
        )
}

/// The declaration of what the tone stack publishes: the fields, the list, the spectrum and the events of the decoders.
pub fn declaration() -> ResultsDecl {
    crate::typed::shape(super::ID).results_decl()
}

sdrkit_core::choice_enum! {
    /// What kind of thing a row holds.
    pub enum ToneKind {
        /// A CTCSS tone.
        Ctcss = ("ctcss", "CTCSS"),
        /// A DCS code.
        Dcs = ("dcs", "DCS"),
        /// A string of DTMF keys.
        Dtmf = ("dtmf", "DTMF"),
        /// A sequential call.
        Selcall = ("selcall", "Selcall"),
        /// A SELCAL call.
        Selcal = ("selcal", "SELCAL"),
        /// A two-tone page.
        TwoTone = ("two_tone", "2-tone"),
    }
}

impl ToneKind {
    /// The label of the kind.
    pub fn name(self) -> &'static str {
        Self::CHOICES[usize::from(self.index())].label.as_ref()
    }
}

/// One row of the list.
#[derive(Clone, Debug, PartialEq)]
pub struct ToneRow {
    /// The kind.
    pub kind: ToneKind,
    /// What was heard ("123.0 Hz", "D023N", "159#", "12345", "AM-CP", "682.5 + 1000.0 Hz").
    pub text: String,
    /// The details.
    pub detail: String,
    /// Signal time of the start, s.
    pub start_s: f64,
    /// Length so far, s.
    pub length_s: f64,
    /// Level, dB relative to a full-scale sine.
    pub level_db: f32,
    /// Still going on.
    pub active: bool,
}

impl Shaped for ToneRow {
    type Row = TonesHeardRow<'static>;
    fn row(&self, _seen: Option<&Seen>, _now_s: f64, out: RowOut<'_, '_, Self::Row>) {
        out.put(&TonesHeardRow {
            start_s: self.start_s,
            kind: self.kind,
            text: &self.text,
            length: Seconds(self.length_s),
            detail: &self.detail,
            level_db: f64::from(self.level_db),
            active: self.active,
        });
    }
}

sdrkit_core::commands! {
    /// The commands of `tones.events`.
    pub enum ToneCommand {
        /// Empties the list and the counters.
        Clear = CLEAR,
    }
}

/// The detail of a sequential call: the set, the tones, the form sent and the other sets that fit.
pub fn selcall_detail(c: &SelcallCall) -> String {
    let mut d = format!(
        "{}, {} tones of about {:.0} ms",
        c.set.name,
        c.digits().len(),
        c.tone_s * 1000.0
    );
    if c.sent() != c.digits() {
        d.push_str(&format!(", sent as {}", c.sent()));
    }
    let others = also_fits(c);
    if !others.is_empty() {
        d.push_str(&format!("; the tones also fit {others}"));
    }
    d
}

/// The other sets a sequential call fits, as a list ("PZVEI, ZVEI2").
pub fn also_fits(c: &SelcallCall) -> String {
    TONE_SETS
        .iter()
        .enumerate()
        .filter(|(i, _)| c.also_fits & (1 << i) != 0)
        .map(|(_, s)| s.name)
        .collect::<Vec<_>>()
        .join(", ")
}

/// `tones.events` (module documentation).
pub struct EventsComponent {
    p: Published,
    rows: LogRing<ToneRow>,
    counts: [u64; 6],
    front: FrontReport,
    spectrum: Option<Spectrum>,
    gate_open: bool,
    ctcss: Option<(f32, usize)>,
    dcs: Option<(String, Option<String>)>,
    dtmf: String,
    dtmf_last: f64,
    now: f64,
    last_decode: Option<f64>,
    logged_state: Option<u8>,
    spectrum_id: Option<SeriesId>,
    summary: String,
}

impl Default for EventsComponent {
    fn default() -> Self {
        EventsComponent {
            p: Published::from_decl(Arc::new(own_declaration())),
            rows: LogRing::new(ROWS),
            counts: [0; 6],
            front: FrontReport {
                audio_db: -120.0,
                snr_db: 0.0,
                offset_hz: 0.0,
                carrier_recent: false,
                audio_input: false,
                demod: Demod::Fm,
                rate: 0.0,
                dial_hz: 0.0,
            },
            spectrum: None,
            gate_open: false,
            ctcss: None,
            dcs: None,
            dtmf: String::new(),
            dtmf_last: f64::NEG_INFINITY,
            now: 0.0,
            last_decode: None,
            logged_state: None,
            spectrum_id: own_declaration().series_id(TONES_SPECTRUM.key),
            summary: String::new(),
        }
    }
}

impl EventsComponent {
    fn add(&mut self, row: ToneRow) {
        self.counts[usize::from(row.kind.index())] += 1;
        self.p
            .results_mut()
            .log(format!("{} {} {}", row.kind.name(), row.text, row.detail));
        self.rows.push(row);
    }
    /// The newest row of `kind` that is still active.
    fn active(&mut self, kind: ToneKind) -> Option<&mut ToneRow> {
        self.rows
            .find_recent_mut(ROWS, |r| r.kind == kind && r.active)
    }
    fn sub(&mut self, t: f64, e: &SubAudioEvent) {
        match e {
            SubAudioEvent::Ctcss(c) => {
                self.decoded(t);
                match c.kind {
                    CtcssEventKind::Locked => {
                        self.ctcss = Some((c.tone.hz, 0));
                        let detail = match (c.tone.eia, c.tone.motorola) {
                            (true, Some(m)) => format!("EIA tone, Motorola {m}"),
                            (true, None) => "EIA tone".to_string(),
                            (false, Some(m)) => format!("further tone, Motorola {m}"),
                            (false, None) => "further tone".to_string(),
                        };
                        self.add(ToneRow {
                            kind: ToneKind::Ctcss,
                            text: format!("{:.1} Hz", c.tone.hz),
                            detail,
                            start_s: t,
                            length_s: 0.0,
                            level_db: c.level_db,
                            active: true,
                        });
                        self.p
                            .results_mut()
                            .log(format!("Tones: CTCSS {:.1} Hz", c.tone.hz));
                    }
                    CtcssEventKind::Released | CtcssEventKind::TurnOff => {
                        self.ctcss = None;
                        if let Some(r) = self.active(ToneKind::Ctcss) {
                            r.active = false;
                            r.length_s = t - r.start_s;
                        }
                    }
                }
            }
            SubAudioEvent::Dcs(d) => {
                let name = d.reading.name();
                let alias = d
                    .reading
                    .alias
                    .map(|(c, i)| format!("{c}{}", if i { 'I' } else { 'N' }));
                self.decoded(t);
                match d.kind {
                    DcsEventKind::Locked => {
                        let mut detail = if d.reading.code.is_standard() {
                            "standard code".to_string()
                        } else {
                            "not in the usual list of 83 codes".to_string()
                        };
                        if let Some(a) = &alias {
                            detail.push_str(&format!("; the same waveform reads as {a}"));
                        }
                        self.dcs = Some((name.clone(), alias));
                        self.add(ToneRow {
                            kind: ToneKind::Dcs,
                            text: name.clone(),
                            detail,
                            start_s: t,
                            length_s: 0.0,
                            level_db: d.level_db,
                            active: true,
                        });
                        self.p.results_mut().log(format!("Tones: DCS {name}"));
                    }
                    DcsEventKind::Released | DcsEventKind::TurnOff => {
                        self.dcs = None;
                        if let Some(r) = self.active(ToneKind::Dcs) {
                            r.active = false;
                            r.length_s = t - r.start_s;
                        }
                    }
                }
            }
        }
    }
    /// A decoder decided something at `t`.
    fn decoded(&mut self, t: f64) {
        self.now = self.now.max(t);
        self.last_decode = Some(self.now);
    }
    fn dtmf(&mut self, t: f64, e: &DtmfEvent) {
        self.decoded(t);
        if !e.down {
            self.dtmf_last = t;
            return;
        }
        let join = t - self.dtmf_last < DTMF_JOIN_S;
        match self.active(ToneKind::Dtmf) {
            Some(r) if join => {
                r.text.push(e.key);
                r.length_s = t - r.start_s;
            }
            _ => {
                if let Some(r) = self.active(ToneKind::Dtmf) {
                    r.active = false;
                }
                let (row, col) = key_tones(e.key);
                self.add(ToneRow {
                    kind: ToneKind::Dtmf,
                    text: e.key.to_string(),
                    detail: format!("{row:.0} + {col:.0} Hz"),
                    start_s: t,
                    length_s: 0.0,
                    level_db: e.row_db.max(e.column_db),
                    active: true,
                });
                self.dtmf.clear();
            }
        }
        self.dtmf.push(e.key);
        self.dtmf_last = t;
    }
    /// Ends the DTMF string when no key came for 2.5 s.
    fn dtmf_timeout(&mut self) {
        let now = self.now;
        let last = self.dtmf_last;
        if now - last >= DTMF_JOIN_S {
            if let Some(r) = self.active(ToneKind::Dtmf) {
                r.active = false;
                r.length_s = last + DTMF_JOIN_S - r.start_s;
            }
            self.dtmf.clear();
        }
    }
    fn selcall(&mut self, t: f64, c: &SelcallCall) {
        self.decoded(t);
        self.add(ToneRow {
            kind: ToneKind::Selcall,
            text: c.digits().to_string(),
            detail: selcall_detail(c),
            start_s: t,
            length_s: c.length_s,
            level_db: 0.0,
            active: false,
        });
    }
    fn selcal(&mut self, t: f64, c: &SelcalCall, rate: f64) {
        let tones = c.code.tones();
        let length = (c.end.saturating_sub(c.start)) as f64 / rate;
        self.decoded(t);
        self.add(ToneRow {
            kind: ToneKind::Selcal,
            text: c.code.to_string(),
            detail: format!(
                "{:.1} / {:.1} / {:.1} / {:.1} Hz",
                tones[0], tones[1], tones[2], tones[3]
            ),
            start_s: t - length,
            length_s: length,
            level_db: c.level_db,
            active: false,
        });
    }
    fn two_tone(&mut self, t: f64, p: &TwoTonePage) {
        self.decoded(t);
        let text = format!("{:.1} + {:.1} Hz", p.a_hz, p.b_hz);
        let detail = format!("tone A {:.1} s, tone B {:.1} s", p.a_s, p.b_s);
        let same =
            |r: &ToneRow| r.kind == ToneKind::TwoTone && r.active && (r.start_s - t).abs() < 0.01;
        match self.rows.find_recent_mut(ROWS, same) {
            Some(r) => {
                r.detail.clone_from(&detail);
                r.length_s = p.a_s + p.b_s;
                r.active = !p.ended;
            }
            None => self.add(ToneRow {
                kind: ToneKind::TwoTone,
                text,
                detail,
                start_s: t,
                length_s: p.a_s + p.b_s,
                level_db: 0.0,
                active: !p.ended,
            }),
        }
    }
    fn state(&self) -> u8 {
        if self.last_decode.is_some_and(|t| self.now - t <= RECENT_S) {
            2
        } else if self.front.audio_db > -45.0 || self.front.carrier_recent {
            1
        } else {
            0
        }
    }
    fn publish(&mut self) {
        let state = self.state();
        if self.logged_state != Some(state) {
            self.logged_state = Some(state);
            self.p.results_mut().log(match state {
                2 => "Tones: tone or code decoded",
                1 => "Tones: audio, no tone",
                _ => "Tones: no signal",
            });
        }
        let f = self.front;
        let total: u64 = self.counts.iter().sum();
        let lock_state = match state {
            2 => LockState::Locked,
            1 => LockState::Tracking,
            _ => LockState::Searching,
        };
        self.summary.clear();
        // Writing into a `String` cannot fail.
        let _ = match (&self.ctcss, &self.dcs, self.rows.iter().next_back()) {
            (Some((hz, _)), _, _) => write!(self.summary, "CTCSS {hz:.1} Hz"),
            (None, Some((code, _)), _) => write!(self.summary, "DCS {code}"),
            (None, None, Some(r)) if state == 2 => {
                write!(self.summary, "{} {}", r.kind.name(), r.text)
            }
            _ => write!(self.summary, "listening for tones, {total} heard"),
        };
        let summary = &self.summary;
        let p = &mut self.p;
        p.results_mut().update_status(|s| {
            let snr = (!f.audio_input).then_some(f.snr_db);
            let offset = (!f.audio_input).then_some(f64::from(f.offset_hz));
            let changed = s.lock_state != lock_state
                || s.snr_db != snr
                || s.offset_hz != offset
                || s.summary != *summary;
            if changed {
                s.lock_state = lock_state;
                s.snr_db = snr;
                s.offset_hz = offset;
                s.summary.clone_from(summary);
            }
            changed
        });
        let count = |k: ToneKind| self.counts[usize::from(k.index())] as i64;
        let radio = !f.audio_input;
        p.put(&TonesFields {
            audio_db: f64::from(f.audio_db),
            snr_db: radio.then_some(f64::from(f.snr_db)),
            offset_hz: radio.then_some(Hertz(f64::from(f.offset_hz))),
            carrier_recent: radio.then_some(f.carrier_recent),
            ctcss_hz: self.ctcss.map(|(hz, _)| Hertz(f64::from(hz))),
            dcs: self.dcs.as_ref().map(|(c, _)| c.as_str()),
            dcs_alias: self.dcs.as_ref().and_then(|(_, a)| a.as_deref()),
            strongest_hz: self
                .spectrum
                .and_then(|s| s.strongest)
                .map(|(hz, _)| Hertz(hz)),
            dtmf: &self.dtmf,
            gate_open: self.gate_open,
            input_rate_hz: Hertz(f.rate),
            audio_input: f.audio_input,
            demod: f.demod,
            detections: total as i64,
            ctcss_count: count(ToneKind::Ctcss),
            dcs_count: count(ToneKind::Dcs),
            dtmf_count: count(ToneKind::Dtmf),
            selcall_count: count(ToneKind::Selcall),
            selcal_count: count(ToneKind::Selcal),
            two_tone_count: count(ToneKind::TwoTone),
        });
        shaped::log_rows(&self.rows, &TONES_HEARD, self.now, p);
        if let (Some(s), Some(id)) = (&self.spectrum, self.spectrum_id) {
            p.results_mut().update_series(id, |sp| {
                let point = |(i, &v): (usize, &f32)| [i as f32 * TOP_HZ / POINTS as f32, v];
                let same = sp.line
                    && sp.points.len() == s.db.len()
                    && sp
                        .points
                        .iter()
                        .zip(s.db.iter().enumerate().map(point))
                        .all(|(a, b)| *a == b);
                if !same {
                    sp.line = true;
                    sp.points.clear();
                    sp.points.extend(s.db.iter().enumerate().map(point));
                }
                !same
            });
        }
    }
}

/// The row and column tones of a DTMF key (ITU-T Q.23 table 1).
fn key_tones(key: char) -> (f32, f32) {
    use sdrkit_util_ids::tones::dtmf::{COLUMNS, KEYS, ROWS};
    for (r, row) in KEYS.iter().enumerate() {
        if let Some(c) = row.iter().position(|&k| k == key) {
            return (ROWS[r], COLUMNS[c]);
        }
    }
    (0.0, 0.0)
}

impl Component for EventsComponent {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::sink(
            "tones.events",
            vec![
                Port::typed::<FrontReport>("front"),
                Port::typed::<Spectrum>("spectrum"),
                Port::typed::<GateState>("gate"),
                Port::typed::<SubAudioEvent>("sub"),
                Port::typed::<DtmfEvent>("dtmf"),
                Port::typed::<SelcallCall>("selcall"),
                Port::typed::<SelcalCall>("selcal"),
                Port::typed::<TwoTonePage>("two_tone"),
            ],
        )
    }
    fn configure(&mut self, _inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        // A retune keeps the list. The signal state starts again.
        self.ctcss = None;
        self.dcs = None;
        self.dtmf.clear();
        self.dtmf_last = f64::NEG_INFINITY;
        self.now = 0.0;
        self.last_decode = None;
        self.spectrum = None;
        self.publish();
        Ok(vec![])
    }
    fn process(&mut self, io: &mut Io) {
        if let Some(r) = io.items::<FrontReport>(0).last() {
            self.front = r.value;
            self.now = self.now.max(r.time);
        }
        if let Some(s) = io.items::<Spectrum>(1).last() {
            self.spectrum = Some(s.value);
            self.now = self.now.max(s.time);
        }
        if let Some(g) = io.items::<GateState>(2).last() {
            self.gate_open = g.value.open;
            self.now = self.now.max(g.time);
        }
        for e in io.items::<SubAudioEvent>(3) {
            self.sub(e.time, &e.value);
        }
        for e in io.items::<DtmfEvent>(4) {
            self.dtmf(e.time, &e.value);
        }
        for e in io.items::<SelcallCall>(5) {
            self.selcall(e.time, &e.value);
        }
        for e in io.items::<SelcalCall>(6) {
            self.selcal(e.time, &e.value, super::gen::AUDIO_RATE);
        }
        for e in io.items::<TwoTonePage>(7) {
            self.two_tone(e.time, &e.value);
        }
        self.dtmf_timeout();
        if io.tick() {
            self.publish();
        }
    }
    fn values(&self) -> Values {
        no_values()
    }
    fn commands(&self) -> &Commands {
        ToneCommand::commands()
    }
    fn invoke(&mut self, id: CommandId, _args: &Values) -> Result<Option<Record>, CommandError> {
        match ToneCommand::from_id(id).ok_or(CommandError::NoSuchId { id })? {
            ToneCommand::Clear => {
                self.rows.clear();
                self.counts = [0; 6];
                self.p.clear_rows(&TONES_HEARD);
            }
        }
        Ok(None)
    }
    fn results(&self) -> Option<&Results> {
        Some(self.p.results())
    }
}
