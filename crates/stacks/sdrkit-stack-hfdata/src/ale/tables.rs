//! Call relations, the event log, the station, word, link quality and message tables and the ALE results.
//!
//! Standard: MIL-STD-188-141 Appendix A.

use super::calls::{AleCall, CallKind, CallType};
use super::sync::{AleSync, SyncStatus};
use super::word::{self, Lqa, WordType};
use crate::typed::{
    AleCallEvent, AleEventRow, AleFields, AleLqaRow, AleMessageRow, AleNetRow, AleStationRow,
    AleWordRow, ALE_CALL, ALE_EVENTS, ALE_LEARNED_NETS, ALE_LQA, ALE_MESSAGES, ALE_REMOVE,
    ALE_STATIONS, ALE_STATION_ROW, ALE_WORDS,
};
use crate::util::Origin;
use sdrkit_core::command::{CommandError, CommandId, CommandSet, Commands};
use sdrkit_core::interface::{Fields as _, Hertz, Seconds, Time};
use sdrkit_core::publish::keys::Family;
use sdrkit_core::publish::{field, Published};
use sdrkit_core::{
    ConfigError, LockState, Record, Results, ResultsDecl, Setting, SettingError, SettingId,
    SettingSet, StateBlob, StateError, Status, Value, Values,
};
use sdrkit_graph::{
    values_of, Applied, Component, ComponentInfo, Format, Io, Kind, Port, RunContext,
};
use sdrkit_util_objects::commands::{self as objcmd, Tables};
use sdrkit_util_objects::publish::columns;
use sdrkit_util_objects::shaped::{self, RowOut, Shaped};
use sdrkit_util_objects::{Format as StateFormat, LogRing, ObjectTable, Seen, Shift, Stamp};
use serde::{Deserialize, Serialize};
use std::fmt::Write as _;

const EVENTS_CAP: usize = 500;
const STATIONS_CAP: usize = 500;
const WORDS_CAP: usize = 400;
const NETS_CAP: usize = 64;
const LQA_CAP: usize = 500;
const MESSAGES_CAP: usize = 200;
const START_LOG: &str =
    "ALE: 8-FSK 125 baud words on the upper sideband, 750 to 2500 Hz above the dial";

/// What the ALE stack publishes.
pub fn declaration() -> ResultsDecl {
    ResultsDecl::new()
        .with_fields(AleFields::SHAPE.fields)
        .with_table(&ALE_EVENTS)
        .with_table(&ALE_STATIONS)
        .with_table(&ALE_WORDS)
        .with_table(&ALE_LEARNED_NETS)
        .with_table(&ALE_LQA)
        .with_table(&ALE_MESSAGES)
        .event(&ALE_CALL)
}

/// One call in the event log.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct EventRow {
    /// Signal time of the first word, seconds.
    pub time: f64,
    /// UTC of the first word (seconds since 1970), when the clock is known.
    pub utc: Option<f64>,
    /// The dial it was heard on, when known.
    pub dial_hz: Option<f64>,
    /// The caller.
    pub from: String,
    /// The TO addresses then the THRU members, joined by ", ".
    pub to: String,
    /// The TO addresses.
    pub to_list: Vec<String>,
    /// The THRU members.
    pub members: Vec<String>,
    /// The type after the relations.
    pub kind: CallType,
    /// TIS (else TWS).
    pub tis: bool,
    /// The AMD text.
    pub amd: String,
    /// Commands joined by "; ".
    pub cmd: String,
    /// Vote errors / (147 x words).
    pub ber: f64,
    /// Mean word SNR in 3 kHz, dB.
    pub snr_db: f64,
    /// The AFC at the end, Hz.
    pub offset_hz: f64,
    /// Words in the transmission.
    pub words: u32,
}

impl EventRow {
    /// The call for people: "Individual A -> B  AMD: text".
    pub fn line(&self) -> String {
        let mut line = self.kind.to_string();
        if !self.from.is_empty() || !self.to.is_empty() {
            line.push(' ');
            line.push_str(&self.from);
            if !self.to.is_empty() {
                line.push_str(" -> ");
                line.push_str(&self.to);
            }
        }
        if !self.amd.is_empty() {
            line.push_str("  AMD: ");
            line.push_str(&self.amd);
        }
        line
    }
}

impl Shift for EventRow {
    fn shift_times(&mut self, by_s: f64) {
        self.time += by_s;
    }
}

impl Shaped for EventRow {
    type Row = AleEventRow<'static>;
    fn row(&self, _: Option<&Seen>, _: f64, out: RowOut<'_, '_, Self::Row>) {
        out.put(&AleEventRow {
            start: Seconds(self.time),
            time: self.utc.map(Time),
            dial: self.dial_hz.map(Hertz),
            from: (!self.from.is_empty()).then_some(self.from.as_str()),
            to: &self.to,
            r#type: self.kind,
            tis: self.tis,
            amd: &self.amd,
            commands: &self.cmd,
            ber: self.ber,
            snr_db: self.snr_db,
            offset_hz: Hertz(self.offset_hz),
            words: i64::from(self.words),
        });
    }
}

/// One caller.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct StationRow {
    /// The address.
    pub address: String,
    /// The dial of the last call, when known.
    pub dial_hz: Option<f64>,
    /// SNR of the last call, dB.
    pub snr_db: f64,
    /// BER of the last call.
    pub ber: f64,
    /// The type of the last call.
    pub last: CallType,
}

impl Shift for StationRow {
    fn shift_times(&mut self, _: f64) {}
}

impl Shaped for StationRow {
    type Row = AleStationRow<'static>;
    fn row(&self, seen: Option<&Seen>, _: f64, out: RowOut<'_, '_, Self::Row>) {
        out.put(&AleStationRow {
            address: &self.address,
            dial: self.dial_hz.map(Hertz),
            snr_db: self.snr_db,
            ber: self.ber,
            last_type: self.last,
            heard: columns::heard(seen),
            last_heard: columns::last_heard(seen).map(Time),
        });
    }
}

/// One decoded word.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct WordRow {
    /// Signal time of its first symbol, seconds.
    pub time: f64,
    /// UTC, when the clock is known.
    pub utc: Option<f64>,
    /// The word type.
    pub kind: WordType,
    /// The three characters.
    pub text: String,
    /// Vote errors.
    pub votes: u32,
    /// Golay corrections.
    pub golay: u32,
}

impl Shift for WordRow {
    fn shift_times(&mut self, by_s: f64) {
        self.time += by_s;
    }
}

impl Shaped for WordRow {
    type Row = AleWordRow<'static>;
    fn row(&self, _: Option<&Seen>, _: f64, out: RowOut<'_, '_, Self::Row>) {
        out.put(&AleWordRow {
            start: Seconds(self.time),
            time: self.utc.map(Time),
            r#type: self.kind,
            text: &self.text,
            votes: i64::from(self.votes),
            golay: i64::from(self.golay),
        });
    }
}

/// The link quality of one station on one channel.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LqaRow {
    /// The address.
    pub address: String,
    /// The dial, when known.
    pub dial_hz: Option<f64>,
    /// The figures of its last call as heard here.
    pub measured: Lqa,
    /// The figures it reported of this station's signal: the LQA command of its last call to the own address that had
    /// figures.
    pub reported: Option<Lqa>,
}

impl Shift for LqaRow {
    fn shift_times(&mut self, _: f64) {}
}

impl Shaped for LqaRow {
    type Row = AleLqaRow<'static>;
    fn row(&self, seen: Option<&Seen>, _: f64, out: RowOut<'_, '_, Self::Row>) {
        let r = self.reported;
        out.put(&AleLqaRow {
            address: &self.address,
            dial: self.dial_hz.map(Hertz),
            lqa_ber: i64::from(self.measured.ber),
            sinad_db: i64::from(self.measured.sinad),
            multipath: i64::from(self.measured.mp),
            reported_ber: r.map(|q| i64::from(q.ber)),
            reported_sinad_db: r.map(|q| i64::from(q.sinad)),
            reported_multipath: r.map(|q| i64::from(q.mp)),
            heard: columns::heard(seen),
            last_heard: columns::last_heard(seen).map(Time),
        });
    }
}

/// The key of the link quality row of `address` on `dial_hz`.
pub fn lqa_key(address: &str, dial_hz: Option<f64>) -> String {
    match dial_hz {
        Some(d) => format!("{address} {d:.0}"),
        None => address.to_owned(),
    }
}

/// One AMD text heard.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MessageRow {
    /// Signal time of the call's first word, seconds.
    pub time: f64,
    /// UTC of the call's first word, when the clock is known.
    pub utc: Option<f64>,
    /// The dial it was heard on, when known.
    pub dial_hz: Option<f64>,
    /// The caller.
    pub from: String,
    /// The called addresses, joined by ", ".
    pub to: String,
    /// The text.
    pub text: String,
}

impl Shift for MessageRow {
    fn shift_times(&mut self, by_s: f64) {
        self.time += by_s;
    }
}

impl Shaped for MessageRow {
    type Row = AleMessageRow<'static>;
    fn row(&self, _: Option<&Seen>, _: f64, out: RowOut<'_, '_, Self::Row>) {
        out.put(&AleMessageRow {
            start: Seconds(self.time),
            time: self.utc.map(Time),
            dial: self.dial_hz.map(Hertz),
            from: (!self.from.is_empty()).then_some(self.from.as_str()),
            to: &self.to,
            text: &self.text,
        });
    }
}

/// What ALE keeps across sessions: the calls, the callers, the words, the learned nets, the link quality and the AMD
/// texts.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AleLists {
    /// The calls, oldest first.
    pub events: LogRing<EventRow>,
    /// The callers, by address.
    pub stations: ObjectTable<String, StationRow>,
    /// The words, oldest first.
    pub words: LogRing<WordRow>,
    /// The net addresses learned from answers, oldest first.
    pub learned: LogRing<String>,
    /// The link quality, by station and channel.
    pub lqa: ObjectTable<String, LqaRow>,
    /// The AMD texts, oldest first.
    pub messages: LogRing<MessageRow>,
}

impl Default for AleLists {
    fn default() -> Self {
        AleLists {
            events: LogRing::new(EVENTS_CAP),
            stations: ObjectTable::new(STATIONS_CAP),
            words: LogRing::new(WORDS_CAP),
            learned: LogRing::new(NETS_CAP),
            lqa: ObjectTable::new(LQA_CAP),
            messages: LogRing::new(MESSAGES_CAP),
        }
    }
}

impl Shift for AleLists {
    fn shift_times(&mut self, by_s: f64) {
        self.events.shift_times(by_s);
        self.stations.shift_times(by_s);
        self.words.shift_times(by_s);
        self.lqa.shift_times(by_s);
        self.messages.shift_times(by_s);
    }
}

impl Tables for AleLists {
    const FORMAT: StateFormat = StateFormat::new("hfdata.ale_lists", 2);
    fn clear(&mut self) {
        self.events.clear();
        self.stations.clear();
        self.words.clear();
        self.learned.clear();
        self.lqa.clear();
        self.messages.clear();
    }
    fn adopt(&mut self, saved: Self) {
        self.events.adopt(saved.events);
        self.stations.adopt(saved.stations);
        self.words.adopt(saved.words);
        self.learned.adopt(saved.learned);
        self.lqa.adopt(saved.lqa);
        self.messages.adopt(saved.messages);
    }
}

/// The user's net addresses, separated by commas or spaces.
pub const NETS: Setting<field::Text> = Setting::new("nets", "Net addresses", "", "").help(
    "Net addresses you know, separated by commas. Calls to them are net calls at once. Others are found when a station other than the one called answers.",
);

sdrkit_core::settings! {
    /// The settings of [`AleTables`].
    pub enum AleTablesSetting {
        /// Net addresses.
        Nets = NETS,
        /// The own address: the LQA report of a call to it is about this station's signal.
        SelfAddress = super::tx::SELF_ADDRESS,
    }
}

sdrkit_core::commands! {
    /// The commands of [`AleTables`].
    pub enum AleTablesCommand {
        /// Forget every call, station, word and learned net.
        Clear = objcmd::CLEAR,
        /// Forget one station.
        Remove = ALE_REMOVE.command,
    }
}

/// `hfdata.ale_tables`: the relations, tables and results of ALE. `ale.word` and `ale.call` items in, `ale.call` events out.
pub struct AleTables {
    p: Published,
    shown: Status,
    origin: Origin,
    live: bool,
    dial_hz: Option<f64>,
    nets_text: String,
    nets: Vec<String>,
    own: String,
    lists: AleLists,
    status: SyncStatus,
    last_word_time: Option<f64>,
    last_snr: Option<f64>,
    last_ber: Option<f64>,
    configured: bool,
}

impl Default for AleTables {
    fn default() -> Self {
        let mut p = Published::from_decl(std::sync::Arc::new(declaration()));
        p.results_mut().set_status(&Status::new(LockState::Off));
        AleTables {
            p,
            shown: Status::new(LockState::Off),
            origin: Origin::default(),
            live: false,
            dial_hz: None,
            nets_text: String::new(),
            nets: Vec::new(),
            own: String::new(),
            lists: AleLists::default(),
            status: SyncStatus::default(),
            last_word_time: None,
            last_snr: None,
            last_ber: None,
            configured: false,
        }
    }
}

fn parse_nets(s: &str) -> Vec<String> {
    s.split(|c: char| c == ',' || c.is_whitespace())
        .filter(|t| !t.is_empty())
        .map(|t| t.to_ascii_uppercase())
        .collect()
}

impl AleTables {
    /// The lists.
    pub fn lists(&self) -> &AleLists {
        &self.lists
    }

    fn is_net(&self, a: &str) -> bool {
        self.nets.iter().any(|n| n == a) || self.lists.learned.iter().any(|n| n == a)
    }

    /// Adds a call: the relations, the event log and the callers. Returns the event row.
    pub fn add_call(&mut self, c: &AleCall) -> EventRow {
        let mut kind = c.kind.call_type();
        if c.kind == CallKind::Individual && c.to.len() == 1 && !c.from.is_empty() {
            let called = c.to[0].clone();
            if self.is_net(&called) {
                kind = CallType::Net;
            } else if let Some(e) = self
                .lists
                .events
                .iter_mut()
                .rev()
                .find(|e| e.from == called)
            {
                if c.time - e.time <= 20.0 {
                    let to_caller = e.to_list.len() == 1 && e.to_list[0] == c.from;
                    let indiv = matches!(e.kind, CallType::Individual | CallType::Net);
                    if indiv && to_caller {
                        kind = CallType::Response;
                    } else if e.kind == CallType::Response && to_caller {
                        kind = CallType::Ack;
                    } else if indiv && e.to_list.len() == 1 {
                        kind = CallType::NetResponse;
                        let net = e.to_list[0].clone();
                        e.kind = CallType::Net;
                        if !self.lists.learned.iter().any(|n| *n == net) {
                            self.lists.learned.push(net.clone());
                        }
                        self.p.results_mut().log(format!(
                            "ALE: {net} is a net ({} answered {called})",
                            c.from
                        ));
                    }
                }
            }
        }
        let mut to_all = c.to.clone();
        to_all.extend(c.members.iter().cloned());
        let row = EventRow {
            time: c.time,
            utc: self.origin.wall(c.time),
            dial_hz: self.dial_hz,
            from: c.from.clone(),
            to: to_all.join(", "),
            to_list: c.to.clone(),
            members: c.members.clone(),
            kind,
            tis: c.tis,
            amd: c.amd.clone(),
            cmd: c.commands.join("; "),
            ber: c.ber,
            snr_db: c.snr_db,
            offset_hz: c.offset_hz,
            words: c.words,
        };
        self.lists.events.push(row.clone());
        if !c.from.is_empty() {
            let at = match row.utc {
                Some(w) => Stamp::at(row.time).with_wall(w),
                None => Stamp::at(row.time),
            };
            if let Ok(u) = self.lists.stations.upsert(&c.from, at, || StationRow {
                address: c.from.clone(),
                dial_hz: None,
                snr_db: 0.0,
                ber: 0.0,
                last: kind,
            }) {
                u.row.dial_hz = row.dial_hz;
                u.row.snr_db = row.snr_db;
                u.row.ber = row.ber;
                u.row.last = row.kind;
            }
            let key = lqa_key(&c.from, row.dial_hz);
            if let Ok(u) = self.lists.lqa.upsert(&key, at, || LqaRow {
                address: c.from.clone(),
                dial_hz: row.dial_hz,
                measured: Lqa::REQUEST,
                reported: None,
            }) {
                u.row.measured = c.measured();
                let to_own = !self.own.is_empty() && c.to.len() == 1 && c.to[0] == self.own;
                if to_own && c.lqa.is_some_and(|q| q != Lqa::REQUEST) {
                    u.row.reported = c.lqa;
                }
            }
        }
        if !c.amd.is_empty() {
            self.lists.messages.push(MessageRow {
                time: row.time,
                utc: row.utc,
                dial_hz: row.dial_hz,
                from: row.from.clone(),
                to: row.to.clone(),
                text: row.amd.clone(),
            });
        }
        self.p.results_mut().log(format!("ALE: {}", row.line()));
        row
    }

    fn publish(&mut self) {
        let st = &self.status;
        let now = st.time;
        let recent = self.last_word_time.is_some_and(|t| now - t <= 60.0);
        let lists = &self.lists;
        let p = &mut self.p;
        p.put(&AleFields {
            in_sync: st.in_sync,
            words_recent: recent,
            words_ok: st.words as i64,
            words_missed: st.missed as i64,
            calls: lists.events.len() as i64,
            stations: lists.stations.len() as i64,
            offset_hz: Hertz(st.afc_hz),
            snr_db: self.last_snr,
            ber: self.last_ber,
            level_dbfs: st.level_dbfs,
            clock: self.origin.source(),
        });
        shaped::log_rows(&lists.events, &ALE_EVENTS, now, p);
        shaped::rows(&lists.stations, &ALE_STATIONS, now, p);
        shaped::log_rows(&lists.words, &ALE_WORDS, now, p);
        shaped::rows(&lists.lqa, &ALE_LQA, now, p);
        shaped::log_rows(&lists.messages, &ALE_MESSAGES, now, p);
        for n in lists.learned.iter() {
            ALE_LEARNED_NETS.put(p, n, &AleNetRow { address: n });
        }
        let s = &mut self.shown;
        s.summary.clear();
        // Writing to a String does not fail.
        let _ = match lists.events.iter().next_back() {
            None => write!(
                s.summary,
                "{}, {} words",
                if st.in_sync { "in sync" } else { "listening" },
                st.words
            ),
            Some(e) => {
                let _ = write!(
                    s.summary,
                    "{} calls, {} stations; {} {}",
                    lists.events.len(),
                    lists.stations.len(),
                    e.kind,
                    e.from
                );
                if !e.to.is_empty() {
                    s.summary.push_str(" -> ");
                    s.summary.push_str(&e.to);
                }
                Ok(())
            }
        };
        s.lock_state = if !self.configured {
            LockState::Off
        } else if st.in_sync {
            LockState::Locked
        } else {
            LockState::Searching
        };
        s.snr_db = self.last_snr.map(|v| v as f32);
        s.level_dbfs = Some(st.level_dbfs as f32);
        s.offset_hz = Some(st.afc_hz);
        p.results_mut().set_status(s);
    }

    /// Forgets every list and empties the tables.
    fn clear(&mut self) {
        self.lists.clear();
        self.p.clear_rows(&ALE_LEARNED_NETS);
        self.publish();
    }
}

impl Component for AleTables {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "hfdata.ale_tables",
            vec![
                Port::typed::<AleSync>("words"),
                Port::typed::<AleCall>("calls"),
            ],
            vec![Port::new("events", Kind::Messages)],
        )
        .with_settings(AleTablesSetting::schema())
    }
    fn attach(&mut self, ctx: &RunContext) {
        self.live = ctx.live;
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        let f = &inputs[0];
        self.origin = Origin::new(self.live);
        self.dial_hz = (f.tuned_hz > 0.0).then_some(f.tuned_hz);
        if !self.configured {
            self.p.results_mut().log(START_LOG);
        }
        self.configured = true;
        self.status = SyncStatus::default();
        self.publish();
        Ok(vec![Format::new(Kind::Messages, 0.0)])
    }
    fn process(&mut self, io: &mut Io) {
        let mut changed = false;
        for w in io.items::<AleSync>(0) {
            match &w.value {
                AleSync::Word(x) => {
                    self.last_word_time = Some(x.time);
                    self.last_snr = Some(x.snr_db);
                    self.last_ber = Some(x.votes as f64 / 147.0);
                    self.lists.words.push(WordRow {
                        time: x.time,
                        utc: self.origin.wall(x.time),
                        kind: WordType::of(x.wtype),
                        text: x.chars.iter().map(|&c| word::printable(c)).collect(),
                        votes: x.votes,
                        golay: x.golay,
                    });
                    changed = true;
                }
                AleSync::Status(s) => {
                    self.status = s.clone();
                    changed = true;
                }
                AleSync::Reset(dial) => {
                    self.dial_hz = (*dial > 0.0).then_some(*dial);
                }
                _ => {}
            }
        }
        for c in io.items::<AleCall>(1) {
            let row = self.add_call(&c.value);
            let ev = ALE_CALL.event(
                c.time,
                row.dial_hz.unwrap_or(0.0),
                &AleCallEvent {
                    r#type: row.kind,
                    source: &row.from,
                    target: &row.to,
                    text: &row.line(),
                    tis: row.tis,
                    amd: &row.amd,
                    commands: &row.cmd,
                    ber: row.ber,
                    snr_db: row.snr_db,
                    offset_hz: Hertz(row.offset_hz),
                    words: i64::from(row.words),
                    family: Family::Message,
                },
            );
            io.messages_out(0).push(ev);
            changed = true;
        }
        if changed || io.tick() {
            self.publish();
        }
    }
    fn apply(&mut self, id: SettingId, value: &Value) -> Result<Applied, SettingError> {
        match AleTablesSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            AleTablesSetting::Nets => {
                let text = NETS.read(id, value)?;
                self.nets = parse_nets(text);
                self.nets_text = text.to_owned();
            }
            AleTablesSetting::SelfAddress => {
                self.own = super::tx::SELF_ADDRESS
                    .read(id, value)?
                    .trim()
                    .to_ascii_uppercase();
            }
        }
        self.publish();
        Ok(Applied::Live)
    }
    fn values(&self) -> Values {
        values_of([
            (AleTablesSetting::Nets, Value::Text(self.nets_text.clone())),
            (AleTablesSetting::SelfAddress, Value::Text(self.own.clone())),
        ])
    }
    fn commands(&self) -> &Commands {
        AleTablesCommand::commands()
    }
    fn invoke(&mut self, id: CommandId, args: &Values) -> Result<Option<Record>, CommandError> {
        match AleTablesCommand::from_id(id).ok_or(CommandError::NoSuchId { id })? {
            AleTablesCommand::Clear => self.clear(),
            AleTablesCommand::Remove => {
                let table = self
                    .p
                    .results()
                    .decl()
                    .table_id(ALE_STATIONS.key)
                    .expect("the declaration holds the station table");
                objcmd::remove(
                    &mut self.lists.stations,
                    table,
                    &mut self.p,
                    args,
                    &ALE_STATION_ROW,
                )?;
                self.publish();
            }
        }
        Ok(None)
    }
    fn results(&self) -> Option<&Results> {
        Some(self.p.results())
    }
    fn save_state(&self) -> Option<StateBlob> {
        self.lists.save_state(self.status.time).ok()
    }
    fn restore_state(&mut self, state: &StateBlob) -> Result<(), StateError> {
        self.lists.restore_state(state, self.status.time)?;
        self.publish();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn call(time: f64, from: &str, to: &[&str], kind: CallKind) -> AleCall {
        AleCall {
            time,
            kind,
            from: from.into(),
            to: to.iter().map(|s| s.to_string()).collect(),
            members: vec![],
            tis: true,
            amd: String::new(),
            commands: vec![],
            lqa: None,
            words: 4,
            votes: 0,
            ber: 0.0,
            snr_db: 20.0,
            offset_hz: 0.0,
        }
    }

    #[test]
    fn relations() {
        let mut t = AleTables::default();
        assert_eq!(
            t.add_call(&call(0.0, "A", &["B"], CallKind::Individual))
                .kind,
            CallType::Individual
        );
        assert_eq!(
            t.add_call(&call(5.0, "B", &["A"], CallKind::Individual))
                .kind,
            CallType::Response
        );
        assert_eq!(
            t.add_call(&call(9.0, "A", &["B"], CallKind::Individual))
                .kind,
            CallType::Ack
        );
        assert_eq!(
            t.add_call(&call(20.0, "C", &["N"], CallKind::Individual))
                .kind,
            CallType::Individual
        );
        assert_eq!(
            t.add_call(&call(24.0, "M", &["C"], CallKind::Individual))
                .kind,
            CallType::NetResponse
        );
        assert_eq!(t.lists.events.iter().nth(3).unwrap().kind, CallType::Net);
        assert_eq!(
            t.lists.learned.iter().cloned().collect::<Vec<_>>(),
            vec!["N".to_string()]
        );
        assert_eq!(
            t.add_call(&call(30.0, "C", &["N"], CallKind::Individual))
                .kind,
            CallType::Net
        );
        let state = t.lists.save_state(30.0).unwrap();
        let mut u = AleTables::default();
        u.lists.restore_state(&state, 30.0).unwrap();
        assert_eq!(u.lists.events.len(), 6);
        let garbage = StateBlob::new("hfdata.ale_lists", 1, b"garbage".to_vec());
        assert!(u.lists.restore_state(&garbage, 30.0).is_err());
        assert_eq!(u.lists.events.len(), 6);
    }
}
