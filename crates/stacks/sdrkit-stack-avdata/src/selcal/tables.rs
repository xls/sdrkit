//! The SELCAL results: the call log, the codes heard with the aircraft each code belongs to, the audio spectrum and the status
//! line. The log and the codes are kept through a restart and in the saved state. The calls leave the graph as the
//! `tones.selcal` events of sdrkit-proto-tones.

use super::{Measures, AIRCRAFT_TABLE};
use crate::report::{self, Heard};
use crate::typed::{
    SelcalCallRow, SelcalCodeRow, SelcalFields, SELCAL_CALLS, SELCAL_CODES, SELCAL_CODE_ROW,
    SELCAL_REMOVE, SELCAL_SPECTRUM,
};
use sdrkit_core::interface::{Hertz, Time};
use sdrkit_core::publish::Published;
use sdrkit_core::{
    Clock, CommandError, CommandId, CommandSet, Commands, ConfigError, Record, Results, SeriesId,
    SettingError, SettingId, SettingSet, StateBlob, StateError, Value, Values,
};
use sdrkit_graph::{
    values_of, Applied, Component, ComponentInfo, Format, Io, Kind, Port, RateFrom, RunContext,
};
use sdrkit_proto_tones::selcal::SelcalCall;
use sdrkit_util_ids::names::{IdKind, IdNames};
use sdrkit_util_objects::commands as objcmd;
use sdrkit_util_objects::publish::columns;
use sdrkit_util_objects::shaped::{self, RowOut, Shaped};
use sdrkit_util_objects::{
    Format as StateFormat, LogRing, ObjectTable, Seen, Shift, Stamp, Tables as _,
};
use serde::{Deserialize, Serialize};
use std::fmt::Write;

/// Calls kept.
pub const CALL_CAP: usize = 300;
/// Codes kept.
pub const CODE_CAP: usize = 300;
/// The width of a bin of the audio spectrum, Hz.
const BIN_HZ: f32 = 31.25;

/// A call as logged.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Logged {
    /// The code, "AM-CP".
    pub code: String,
    /// The four tones, Hz.
    pub tones_hz: [f32; 4],
    /// The four tones as text, "a b / c d".
    #[serde(default)]
    pub tones: String,
    /// The dial (0 when not known).
    pub dial_hz: f64,
    /// The level of the tones, dB relative to a full-scale sine.
    pub level_db: f32,
    /// The aircraft the code belongs to.
    pub aircraft: String,
}

impl Shaped for Heard<Logged> {
    type Row = SelcalCallRow<'static>;
    fn row(&self, _seen: Option<&Seen>, _now_s: f64, out: RowOut<'_, '_, Self::Row>) {
        let c = &self.item;
        out.put(&SelcalCallRow {
            time: self.wall.map(Time),
            signal_s: self.at,
            code: &c.code,
            code_row: &c.code,
            tones: &c.tones,
            dial: (c.dial_hz != 0.0).then_some(Hertz(c.dial_hz)),
            level_db: f64::from(c.level_db),
            aircraft: report::text(&c.aircraft),
        });
    }
}

/// A code heard.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Code {
    /// The code.
    #[serde(default)]
    pub code: String,
    /// The aircraft it belongs to.
    pub aircraft: String,
    /// The last dial frequency.
    pub dial_hz: f64,
}

impl Shift for Code {
    fn shift_times(&mut self, _by_s: f64) {}
}

impl Shaped for Code {
    type Row = SelcalCodeRow<'static>;
    fn row(&self, seen: Option<&Seen>, _now_s: f64, out: RowOut<'_, '_, Self::Row>) {
        out.put(&SelcalCodeRow {
            code: &self.code,
            aircraft: report::text(&self.aircraft),
            heard: columns::heard(seen),
            last_heard: columns::last_heard(seen).map(Time),
            dial: (self.dial_hz != 0.0).then_some(Hertz(self.dial_hz)),
        });
    }
}

/// What is kept across sessions.
#[derive(Serialize, Deserialize)]
pub struct Store {
    calls: LogRing<Heard<Logged>>,
    codes: ObjectTable<String, Code>,
}

impl Default for Store {
    fn default() -> Self {
        Store {
            calls: LogRing::new(CALL_CAP),
            codes: ObjectTable::new(CODE_CAP),
        }
    }
}

impl Shift for Store {
    fn shift_times(&mut self, by_s: f64) {
        self.calls.shift_times(by_s);
        self.codes.shift_times(by_s);
    }
}

impl sdrkit_util_objects::Tables for Store {
    const FORMAT: StateFormat = StateFormat::new("avdata.selcal", 2);
    fn clear(&mut self) {
        self.calls.clear();
        self.codes.clear();
    }
    fn adopt(&mut self, saved: Self) {
        self.calls.adopt(saved.calls);
        self.codes.adopt(saved.codes);
    }
}

sdrkit_core::settings! {
    /// The settings of [`Tables`].
    pub enum TablesSetting {
        /// The table of codes and aircraft.
        AircraftTable = AIRCRAFT_TABLE,
    }
}

sdrkit_core::commands! {
    /// The commands of [`Tables`].
    pub enum TablesCommand {
        /// Forget every call and code.
        Clear = objcmd::CLEAR,
        /// Forget one code.
        Remove = SELCAL_REMOVE.command,
    }
}

/// The results component (`avdata.selcal_tables`).
pub struct Tables {
    published: Published,
    spectrum: SeriesId,
    store: Store,
    table_text: String,
    names: IdNames,
    last_call_s: f64,
    last_carrier_s: f64,
    measures: Measures,
    summary: String,
    live: bool,
    clock: Clock,
    configured: bool,
}

impl Default for Tables {
    fn default() -> Self {
        let published = crate::typed::results(super::ID);
        let spectrum = published
            .results()
            .decl()
            .series_id(SELCAL_SPECTRUM.key)
            .expect("the declaration holds the spectrum");
        Tables {
            published,
            spectrum,
            store: Store::default(),
            table_text: String::new(),
            names: IdNames::new(IdKind::Selcal),
            last_call_s: f64::NEG_INFINITY,
            last_carrier_s: f64::NEG_INFINITY,
            measures: Measures::default(),
            summary: String::new(),
            live: false,
            clock: Clock::fixed(0.0),
            configured: false,
        }
    }
}

impl Tables {
    fn call(&mut self, t: f64, c: &SelcalCall) {
        self.last_call_s = t;
        let code = c.code.to_string();
        let ac = self.names.name(&code).unwrap_or("").to_owned();
        let dial = self.measures.dial_hz;
        let wall = report::wall(self.live, &self.clock);
        let mut at = Stamp::at(t);
        if let Some(w) = wall {
            at = at.with_wall(w);
        }
        let tones_hz = c.code.tones();
        let mut tones = String::new();
        let _ = write!(
            tones,
            "{:.1} {:.1} / {:.1} {:.1}",
            tones_hz[0], tones_hz[1], tones_hz[2], tones_hz[3]
        );
        self.store.calls.push(Heard {
            at: t,
            wall,
            item: Logged {
                code: code.clone(),
                tones_hz,
                tones,
                dial_hz: dial,
                level_db: c.level_db,
                aircraft: ac.clone(),
            },
        });
        if let Ok(up) = self.store.codes.upsert(&code, at, || Code {
            code: code.clone(),
            ..Code::default()
        }) {
            up.row.aircraft.clone_from(&ac);
            up.row.dial_hz = dial;
        }
        self.published.results_mut().log(if ac.is_empty() {
            format!("SELCAL {code}")
        } else {
            format!("SELCAL {code} ({ac})")
        });
    }

    fn publish(&mut self) {
        let m = &self.measures;
        let now = m.time_s;
        if m.carrier {
            self.last_carrier_s = now;
        }
        let audio = m.level_db > -45.0 || now - self.last_carrier_s <= 5.0;
        let state = if now - self.last_call_s <= 60.0 {
            2
        } else if audio {
            1
        } else {
            0
        };
        self.summary.clear();
        let _ = if !m.refused.is_empty() {
            write!(self.summary, "SELCAL: {}", m.refused)
        } else if let Some(c) = self
            .store
            .calls
            .newest_first()
            .next()
            .filter(|_| state == 2)
        {
            write!(
                self.summary,
                "SELCAL {} ({} calls, {} codes)",
                c.item.code,
                self.store.calls.len(),
                self.store.codes.len()
            )
        } else if audio {
            write!(self.summary, "SELCAL: listening")
        } else {
            write!(self.summary, "SELCAL: no audio")
        };
        let level = m.level_db;
        let tone = m.tone.map(|t| t.0);
        let p = &mut self.published;
        let r = p.results_mut();
        report::status(r, state, None, Some(level), None, &self.summary);
        let spectrum = &m.spectrum;
        r.update_series(self.spectrum, |s| {
            let same = s.line
                && s.points.len() == spectrum.len()
                && s.points
                    .iter()
                    .zip(spectrum.iter())
                    .all(|(a, b)| a[1] == *b);
            if same {
                return false;
            }
            s.line = true;
            s.points.clear();
            s.points.extend(
                spectrum
                    .iter()
                    .enumerate()
                    .map(|(i, v)| [(i as f32 + 0.5) * BIN_HZ, *v]),
            );
            true
        });
        let n = |v: usize| i64::try_from(v).unwrap_or(i64::MAX);
        p.put(&SelcalFields {
            calls: n(self.store.calls.len()),
            codes: n(self.store.codes.len()),
            tone: tone.map(Hertz),
        });
        shaped::log_rows(&self.store.calls, &SELCAL_CALLS, now, p);
        shaped::rows(&self.store.codes, &SELCAL_CODES, now, p);
    }
}

impl Component for Tables {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "avdata.selcal_tables",
            vec![
                Port::typed::<SelcalCall>("calls"),
                Port::typed::<Measures>("measures"),
            ],
            vec![Port::new("out", Kind::Messages).rate_from(RateFrom::Fixed(0.0))],
        )
        .with_settings(TablesSetting::schema())
    }

    fn configure(&mut self, _: &[Format]) -> Result<Vec<Format>, ConfigError> {
        // A restart forgets the signal and keeps the lists: their times move back by the old signal time.
        if self.configured {
            let by = -self.measures.time_s;
            self.store.shift_times(by);
        }
        self.configured = true;
        self.measures = Measures::default();
        self.last_call_s = f64::NEG_INFINITY;
        self.last_carrier_s = f64::NEG_INFINITY;
        Ok(vec![Format::new(Kind::Messages, 0.0)])
    }

    fn attach(&mut self, ctx: &RunContext) {
        self.live = ctx.live;
        self.clock = ctx.clock.clone();
    }

    fn process(&mut self, io: &mut Io) {
        for c in io.items::<SelcalCall>(0) {
            self.call(c.time, &c.value);
        }
        let mut report = false;
        for m in io.items::<Measures>(1) {
            self.measures.clone_from(&m.value);
            report = true;
        }
        if report {
            self.publish();
        }
    }

    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        match TablesSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            TablesSetting::AircraftTable => {
                let t = AIRCRAFT_TABLE.read(id, v)?;
                let (names, _) = IdNames::from_text(IdKind::Selcal, t);
                self.names = names;
                t.clone_into(&mut self.table_text);
            }
        }
        Ok(Applied::Live)
    }

    fn values(&self) -> Values {
        values_of([(
            TablesSetting::AircraftTable,
            Value::Text(self.table_text.clone()),
        )])
    }

    fn commands(&self) -> &Commands {
        TablesCommand::commands()
    }

    fn invoke(&mut self, id: CommandId, args: &Values) -> Result<Option<Record>, CommandError> {
        match TablesCommand::from_id(id).ok_or(CommandError::NoSuchId { id })? {
            TablesCommand::Clear => self.store.clear(),
            TablesCommand::Remove => {
                let table = self
                    .published
                    .results()
                    .decl()
                    .table_id(SELCAL_CODES.key)
                    .expect("the declaration holds the code table");
                objcmd::remove(
                    &mut self.store.codes,
                    table,
                    &mut self.published,
                    args,
                    &SELCAL_CODE_ROW,
                )?;
            }
        }
        self.publish();
        Ok(None)
    }

    fn forget(&mut self, kept: &sdrkit_graph::Kept) {
        self.measures = Measures {
            time_s: self.measures.time_s,
            ..Measures::default()
        };
        self.last_call_s = f64::NEG_INFINITY;
        self.last_carrier_s = f64::NEG_INFINITY;
        if !kept.table(SELCAL_CALLS.key) {
            self.store.calls.clear();
        }
        if !kept.table(SELCAL_CODES.key) {
            self.store.codes.clear();
        }
        self.publish();
    }

    fn results(&self) -> Option<&Results> {
        Some(self.published.results())
    }

    fn save_state(&self) -> Option<StateBlob> {
        self.store.save_state(self.measures.time_s).ok()
    }

    fn restore_state(&mut self, state: &StateBlob) -> Result<(), StateError> {
        self.store.restore_state(state, self.measures.time_s)?;
        self.publish();
        Ok(())
    }
}
