//! `pager.tables`: the page list (kept across retunes and in the saved state), the statistics of each speed, the fields, the
//! events and the state of the pager stack. It merges the two decoders: a page that the other decoder gave with the same
//! protocol, speed, address, function, type and text less than 8 s before is dropped; the code word counts and the refused pages of a speed
//! are the larger of the two; the lock flags are either decoder's. The status text names the refused pages and how many of
//! them a rule refused that no public document confirms.

use super::demod::PagerLock;
use super::parse::WordStat;
use super::PageItem;
use crate::front::CarrierState;
use crate::set_status;
use crate::typed::{
    FlexPageEvent, PagerFields, PagerHeardRow, PagerPageRow, PagerSpeedRow, PocsagPageEvent,
    FLEX_PAGE, PAGER_HEARD, PAGER_PAGES, PAGER_SPEEDS, POCSAG_PAGE,
};
use sdrkit_core::interface::{Hertz, Time};
use sdrkit_core::publish::keys::Family;
use sdrkit_core::publish::Published;
use sdrkit_core::{
    CommandError, CommandId, CommandSet, Commands, ConfigError, Event, LockState, Record, Results,
    ResultsDecl, StateBlob, StateError, Values,
};
use sdrkit_graph::{no_values, Component, ComponentInfo, Format, Io, Kind, Port, RateFrom};
use sdrkit_proto_pager::{Kind as PageKind, Protocol};
use sdrkit_proto_tones::selcall::SelcallCall;
use sdrkit_util_objects::commands::CLEAR;
use sdrkit_util_objects::publish::columns;
use sdrkit_util_objects::shaped::{self, RowOut, Shaped};
use sdrkit_util_objects::{
    Format as StateFormat, LogRing, ObjectTable, Seen, Shift, Stamp, Tables,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// The pages the list keeps.
pub const PAGES_KEPT: usize = 2000;
/// A page the other decoder gave this recently is the same page, s.
const SAME_PAGE_S: f64 = 8.0;

sdrkit_core::choice_enum! {
    /// What a page carries.
    #[derive(Default, Serialize, Deserialize)]
    pub enum PageType {
        /// No message: an alert only.
        #[default]
        Tone = ("tone", "Tone"),
        /// Digits and the other numeric characters.
        Numeric = ("numeric", "Numeric"),
        /// Text.
        Alpha = ("alpha", "Text"),
        /// A FLEX secure message.
        Secure = ("secure", "Secure"),
        /// FLEX binary data.
        Binary = ("binary", "Binary"),
        /// A FLEX instruction.
        Instruction = ("instruction", "Instruction"),
        /// A sequential call on the paging channel.
        Selcall = ("selcall", "Sequential call"),
    }
}

impl From<PageKind> for PageType {
    fn from(k: PageKind) -> Self {
        match k {
            PageKind::Tone => PageType::Tone,
            PageKind::Numeric => PageType::Numeric,
            PageKind::Alpha => PageType::Alpha,
            PageKind::Secure => PageType::Secure,
            PageKind::Binary => PageType::Binary,
            PageKind::Instruction => PageType::Instruction,
        }
    }
}

sdrkit_core::choice_enum! {
    /// The paging protocol of an address.
    #[derive(Default, Serialize, Deserialize)]
    pub enum PagerProtocol {
        /// POCSAG.
        #[default]
        Pocsag = ("pocsag", "POCSAG"),
        /// FLEX.
        Flex = ("flex", "FLEX"),
    }
}

/// An address heard on one protocol: the key of a row of [`PAGER_HEARD`] ("pocsag/1234567").
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct HeardKey {
    /// The protocol.
    pub protocol: PagerProtocol,
    /// The address.
    pub address: u32,
}

impl std::fmt::Display for HeardKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let key = &<PagerProtocol as sdrkit_core::Choice>::CHOICES
            [usize::from(sdrkit_core::Choice::index(self.protocol))]
        .key;
        write!(f, "{key}/{}", self.address)
    }
}

/// One address heard.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct HeardRow {
    /// The key.
    pub key: HeardKey,
    /// Pages.
    pub pages: u32,
    /// The protocol and speed of the last page.
    pub speed: String,
    /// The last text (a tone-only page does not erase it).
    pub last_text: String,
}

impl Shift for HeardRow {
    fn shift_times(&mut self, _by_s: f64) {}
}

impl Default for HeardKey {
    fn default() -> Self {
        HeardKey {
            protocol: PagerProtocol::Pocsag,
            address: 0,
        }
    }
}

impl Shaped for HeardRow {
    type Row = PagerHeardRow<'static>;
    fn row(&self, seen: Option<&Seen>, _now_s: f64, out: RowOut<'_, '_, Self::Row>) {
        out.put(&PagerHeardRow {
            address: i64::from(self.key.address),
            protocol: self.key.protocol,
            pages: i64::from(self.pages),
            speed: &self.speed,
            last_text: (!self.last_text.is_empty()).then_some(self.last_text.as_str()),
            last_heard: columns::last_heard(seen).map(Time),
        });
    }
}

/// The declaration of what the pager stack publishes.
pub fn declaration() -> ResultsDecl {
    crate::typed::shape(super::ID).results_decl()
}

const HEARD_ROW: sdrkit_core::settings::Argument<sdrkit_core::publish::field::Text> =
    sdrkit_util_objects::commands::row_of(PAGER_HEARD.def().key);
const REMOVE: sdrkit_core::command::Command =
    sdrkit_util_objects::commands::remove_command(&[&HEARD_ROW]);

sdrkit_core::commands! {
    /// The commands of `pager.tables`.
    pub enum PagerCommand {
        /// Empties the page list and the statistics.
        Clear = CLEAR,
        /// Forgets one address of the heard table.
        Remove = REMOVE,
    }
}

/// One row of the page list.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct PageRow {
    /// Signal time, s.
    pub time_s: f64,
    /// "POCSAG 1200", "FLEX 3200/4", "Selcall ZVEI1".
    pub protocol: String,
    /// The RIC or capcode (0 for a sequential call).
    pub address: u32,
    /// The POCSAG function code.
    pub function: Option<u8>,
    /// What it carries.
    pub kind: PageType,
    /// The message.
    pub message: String,
    /// A word was lost or the end is not sure.
    pub damaged: bool,
    /// Bits the repairs changed.
    pub bits_repaired: u32,
    /// Copies voted into the page.
    pub copies: u32,
    /// The carrier's frequency, Hz, when it is known.
    pub freq_hz: Option<f64>,
}

impl Shift for PageRow {
    fn shift_times(&mut self, by_s: f64) {
        self.time_s += by_s;
    }
}

impl Shaped for PageRow {
    type Row = PagerPageRow<'static>;
    fn row(&self, _seen: Option<&Seen>, _now_s: f64, out: RowOut<'_, '_, Self::Row>) {
        out.put(&PagerPageRow {
            time_s: self.time_s,
            protocol: &self.protocol,
            address: i64::from(self.address),
            function: self.function.map(i64::from),
            r#type: self.kind,
            message: &self.message,
            damaged: self.damaged,
            bits_repaired: i64::from(self.bits_repaired),
            copies: i64::from(self.copies),
            freq_hz: self.freq_hz.map(Hertz),
        });
    }
}

/// The statistics of one speed.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct SpeedRow {
    /// Code words valid as received.
    pub good: u64,
    /// Repaired.
    pub repaired: u64,
    /// Lost.
    pub lost: u64,
    /// Pages.
    pub pages: u64,
    /// Batches (POCSAG) or frames (FLEX).
    pub transmissions: u64,
    /// Pages refused by the acceptance rules.
    #[serde(default)]
    pub refused: u64,
    /// Of them, pages refused by a rule that no public document states.
    #[serde(default)]
    pub refused_unconfirmed: u64,
}

impl Shift for SpeedRow {
    fn shift_times(&mut self, _by_s: f64) {}
}

/// What the pager stack keeps across sessions: the page list and the statistics of each speed.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PagerState {
    /// The pages.
    pub pages: LogRing<PageRow>,
    /// The statistics, by speed (the pages, and the code words of the first decoder).
    pub speeds: BTreeMap<String, SpeedRow>,
    /// The code words of the second decoder, by speed.
    #[serde(default)]
    pub narrow: BTreeMap<String, SpeedRow>,
    /// The addresses heard.
    pub heard: ObjectTable<HeardKey, HeardRow>,
}

impl Default for PagerState {
    fn default() -> Self {
        PagerState {
            pages: LogRing::new(PAGES_KEPT),
            speeds: BTreeMap::new(),
            narrow: BTreeMap::new(),
            heard: ObjectTable::new(PAGER_HEARD.cap),
        }
    }
}

impl Shift for PagerState {
    fn shift_times(&mut self, by_s: f64) {
        self.pages.shift_times(by_s);
        self.heard.shift_times(by_s);
    }
}

impl Tables for PagerState {
    const FORMAT: StateFormat = StateFormat::new("pager.pages", 1);
    fn clear(&mut self) {
        self.pages.clear();
        self.speeds.clear();
        self.narrow.clear();
        self.heard.clear();
    }
    fn adopt(&mut self, saved: Self) {
        self.pages.adopt(saved.pages);
        self.speeds = saved.speeds;
        self.narrow = saved.narrow;
        self.heard.adopt(saved.heard);
    }
}

/// The refused pages of the status text: ", 3 pages refused (1 by rules that no public document confirms)", or nothing.
struct Refusals {
    total: u64,
    unconfirmed: u64,
}

impl std::fmt::Display for Refusals {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.total == 0 {
            return Ok(());
        }
        write!(f, ", {} pages refused", self.total)?;
        if self.unconfirmed > 0 {
            write!(
                f,
                " ({} by rules that no public document confirms)",
                self.unconfirmed
            )?;
        }
        Ok(())
    }
}

/// `pager.tables` (module documentation).
pub struct PagerTables {
    p: Published,
    state: PagerState,
    now: f64,
    logged: u32,
    receiving: bool,
    locked: bool,
    carrier: bool,
    snr_db: f32,
    level_dbfs: f32,
    last_speed: &'static str,
    offset_hz: f64,
    freq_hz: f64,
    events: Vec<Event>,
    speeds_dirty: bool,
    summary: String,
    /// The last lock state of each decoder.
    locks: [PagerLock; 2],
    /// The pages of the last [`SAME_PAGE_S`]: the decoder, the time and the page.
    recent: std::collections::VecDeque<(usize, f64, PageItem)>,
    merged: Vec<(f64, usize, PageItem)>,
}

impl Default for PagerTables {
    fn default() -> Self {
        PagerTables {
            p: Published::from_decl(std::sync::Arc::new(declaration())),
            state: PagerState::default(),
            now: 0.0,
            logged: 0,
            receiving: false,
            locked: false,
            carrier: false,
            snr_db: 0.0,
            level_dbfs: -200.0,
            last_speed: "",
            offset_hz: 0.0,
            freq_hz: 0.0,
            events: Vec::new(),
            speeds_dirty: true,
            summary: String::new(),
            locks: [PagerLock::default(); 2],
            recent: std::collections::VecDeque::new(),
            merged: Vec::new(),
        }
    }
}

impl PagerTables {
    /// The statistics row of `speed` of decoder `d` (0 the first, 1 the second).
    fn speed_of(&mut self, d: usize, speed: &str) -> &mut SpeedRow {
        self.speeds_dirty = true;
        let map = if d == 0 {
            &mut self.state.speeds
        } else {
            &mut self.state.narrow
        };
        map.entry(speed.to_string()).or_default()
    }
    fn speed(&mut self, speed: &str) -> &mut SpeedRow {
        self.speed_of(0, speed)
    }
    /// The published statistics of each speed: the pages, and the larger code word counts of the two decoders.
    fn merged_speeds(&self) -> BTreeMap<String, SpeedRow> {
        let mut out = self.state.speeds.clone();
        for (name, b) in &self.state.narrow {
            let r = out.entry(name.clone()).or_default();
            r.good = r.good.max(b.good);
            r.repaired = r.repaired.max(b.repaired);
            r.lost = r.lost.max(b.lost);
            r.transmissions = r.transmissions.max(b.transmissions);
            r.refused = r.refused.max(b.refused);
            r.refused_unconfirmed = r.refused_unconfirmed.max(b.refused_unconfirmed);
        }
        out
    }
    /// Whether the other decoder than `d` gave the same page (the same speed too) less than [`SAME_PAGE_S`] before or after `t`.
    fn seen_by_other(&self, d: usize, t: f64, item: &PageItem) -> bool {
        let p = &item.page;
        self.recent.iter().any(|(od, ot, o)| {
            *od != d
                && (t - ot).abs() < SAME_PAGE_S
                && o.page.rate == p.rate
                && o.page.address == p.address
                && o.page.function == p.function
                && o.page.kind == p.kind
                && o.page.text == p.text
        })
    }
    /// Counts a page of `address` on `protocol` in the heard table.
    fn heard(
        &mut self,
        protocol: PagerProtocol,
        address: u32,
        t: f64,
        speed: &str,
        text: Option<&str>,
    ) {
        let key = HeardKey { protocol, address };
        if let Ok(up) = self.state.heard.upsert(&key, Stamp::at(t), || HeardRow {
            key,
            ..HeardRow::default()
        }) {
            up.row.pages += 1;
            speed.clone_into(&mut up.row.speed);
            if let Some(text) = text {
                text.clone_into(&mut up.row.last_text);
            }
        }
    }
    fn take(&mut self, item: &PageItem, freq_hz: f64) {
        let p = &item.page;
        let protocol = match p.protocol {
            Protocol::Pocsag => PagerProtocol::Pocsag,
            Protocol::Flex => PagerProtocol::Flex,
        };
        let text = (p.kind != PageKind::Tone).then_some(p.text.as_str());
        self.heard(protocol, p.address, p.time, p.rate, text);
        self.speed(p.rate).pages += 1;
        if self.logged < 40 {
            self.logged += 1;
            self.p.results_mut().log(format!(
                "Pagers: {} {} {}",
                p.rate,
                p.address,
                p.kind.name()
            ));
        }
        let address = p.address.to_string();
        let kind = PageType::from(p.kind);
        let ev = match p.protocol {
            Protocol::Pocsag => POCSAG_PAGE.event(
                p.time,
                freq_hz,
                &PocsagPageEvent {
                    source: &address,
                    text: &p.text,
                    function: p.function.map(i64::from),
                    baud: p.rate.trim_start_matches("POCSAG ").parse::<i64>().ok(),
                    r#type: kind,
                    damaged: p.damaged,
                    family: Family::Pager,
                },
            ),
            Protocol::Flex => FLEX_PAGE.event(
                p.time,
                freq_hz,
                &FlexPageEvent {
                    source: &address,
                    text: &p.text,
                    speed: p.rate,
                    r#type: kind,
                    damaged: p.damaged,
                    family: Family::Pager,
                },
            ),
        };
        self.events.push(ev);
        self.state.pages.push(PageRow {
            time_s: p.time,
            protocol: p.rate.to_string(),
            address: p.address,
            function: p.function,
            kind: PageType::from(p.kind),
            message: p.text.clone(),
            damaged: p.damaged,
            bits_repaired: p.bits_repaired,
            copies: p.copies,
            freq_hz: (freq_hz > 0.0).then_some(freq_hz),
        });
    }
    fn selcall(&mut self, t: f64, c: &SelcallCall, freq_hz: f64) {
        self.p.results_mut().log(format!(
            "Pagers: {} sequential call {}",
            c.set.name,
            c.digits()
        ));
        self.state.pages.push(PageRow {
            time_s: t,
            protocol: format!("Selcall {}", c.set.name),
            kind: PageType::Selcall,
            message: c.digits().to_string(),
            copies: 1,
            freq_hz: (freq_hz > 0.0).then_some(freq_hz),
            ..PageRow::default()
        });
    }
    fn publish(&mut self) {
        let speeds = self.merged_speeds();
        let (mut good, mut repaired, mut lost) = (0, 0, 0);
        for s in speeds.values() {
            good += s.good;
            repaired += s.repaired;
            lost += s.lost;
        }
        let pages: u64 = speeds.values().map(|s| s.pages).sum();
        let refused: u64 = speeds.values().map(|s| s.refused).sum();
        let unconfirmed: u64 = speeds.values().map(|s| s.refused_unconfirmed).sum();
        let lock_state = if self.receiving {
            LockState::Locked
        } else {
            LockState::Searching
        };
        let speed = if self.locked {
            self.last_speed
        } else {
            "no sync"
        };
        let r = self.p.results_mut();
        let refusals = Refusals {
            total: refused,
            unconfirmed,
        };
        if pages == 0 {
            set_status(
                r,
                lock_state,
                Some(self.level_dbfs),
                &mut self.summary,
                format_args!(
                    "Pagers: searching, level {:.1} dBFS{refusals}",
                    self.level_dbfs
                ),
            );
        } else {
            set_status(
                r,
                lock_state,
                Some(self.level_dbfs),
                &mut self.summary,
                format_args!(
                    "Pagers: {speed} {pages} pages, {} good and {lost} lost code words{refusals}",
                    good + repaired
                ),
            );
        }
        let snr = self.carrier.then_some(self.snr_db);
        // The offset of the last sync, once a decoder had sync.
        let offset = (!self.last_speed.is_empty()).then_some(self.offset_hz);
        r.update_status(|s| {
            let changed = s.snr_db != snr || s.offset_hz != offset;
            s.snr_db = snr;
            s.offset_hz = offset;
            changed
        });
        let p = &mut self.p;
        p.put(&PagerFields {
            receiving: self.receiving,
            carrier: self.carrier,
            locked: self.locked,
            last_speed: self.last_speed,
            offset_hz: offset.map(Hertz),
            snr_db: snr.map(f64::from),
            level_dbfs: f64::from(self.level_dbfs),
            pages: pages as i64,
            codewords_good: (good + repaired) as i64,
            codewords_repaired: repaired as i64,
            codewords_lost: lost as i64,
            pages_refused: refused as i64,
        });
        shaped::log_rows(&self.state.pages, &PAGER_PAGES, self.now, p);
        shaped::rows(&self.state.heard, &PAGER_HEARD, self.now, p);
        if std::mem::take(&mut self.speeds_dirty) {
            for (name, s) in &speeds {
                PAGER_SPEEDS.put(
                    p,
                    name,
                    &PagerSpeedRow {
                        speed: name,
                        good: s.good as i64,
                        repaired: s.repaired as i64,
                        lost: s.lost as i64,
                        pages: s.pages as i64,
                        transmissions: s.transmissions as i64,
                        refused: s.refused as i64,
                    },
                );
            }
        }
    }
    /// The pages, oldest first.
    pub fn pages(&self) -> impl Iterator<Item = &PageRow> {
        self.state.pages.iter()
    }
}

impl Component for PagerTables {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "pager.tables",
            vec![
                Port::typed::<PageItem>("pages"),
                Port::typed::<WordStat>("stats"),
                Port::typed::<PagerLock>("lock"),
                Port::typed::<CarrierState>("carrier"),
                Port::typed::<SelcallCall>("selcall"),
                Port::typed::<PageItem>("narrow_pages"),
                Port::typed::<WordStat>("narrow_stats"),
                Port::typed::<PagerLock>("narrow_lock"),
            ],
            vec![Port::new("events", Kind::Messages).rate_from(RateFrom::Fixed(0.0))],
        )
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        // The page list survives a retune: its signal times move back so that they stay relative to the new start.
        let shift = self.now;
        self.state.shift_times(-shift);
        self.now = 0.0;
        self.locked = false;
        self.carrier = false;
        self.receiving = false;
        self.locks = [PagerLock::default(); 2];
        self.recent.clear();
        self.freq_hz = inputs.first().map_or(0.0, |f| f.tuned_hz);
        self.p
            .results_mut()
            .log("Pagers: listening for POCSAG and FLEX");
        self.publish();
        Ok(vec![Format::new(Kind::Messages, 0.0)])
    }
    fn process(&mut self, io: &mut Io) {
        let freq = self.freq_hz;
        for (d, port) in [(0, 1), (1, 6)] {
            for s in io.items::<WordStat>(port) {
                let w = s.value;
                let row = self.speed_of(d, w.speed);
                row.good += w.good;
                row.repaired += w.repaired;
                row.lost += w.lost;
                row.transmissions += w.transmissions;
                row.refused += w.refused;
                row.refused_unconfirmed += w.refused_unconfirmed;
                if d == 0 || !self.locks[0].locked {
                    self.offset_hz = w.offset_hz;
                }
            }
        }
        // The pages of both decoders in time order; a page the other decoder gave already is dropped.
        let mut merged = std::mem::take(&mut self.merged);
        merged.clear();
        for (d, port) in [(0, 0), (1, 5)] {
            merged.extend(
                io.items::<PageItem>(port)
                    .iter()
                    .map(|p| (p.time, d, p.value.clone())),
            );
        }
        merged.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
        for (t, d, item) in merged.drain(..) {
            self.now = self.now.max(t);
            while self.recent.front().is_some_and(|r| r.1 < t - SAME_PAGE_S) {
                self.recent.pop_front();
            }
            if self.seen_by_other(d, t, &item) {
                continue;
            }
            self.take(&item, freq);
            self.recent.push_back((d, t, item));
        }
        self.merged = merged;
        for c in io.items::<SelcallCall>(4) {
            self.now = self.now.max(c.time);
            self.selcall(c.time, &c.value, freq);
        }
        for (d, port) in [(0, 2), (1, 7)] {
            if let Some(l) = io.items::<PagerLock>(port).last() {
                self.locks[d] = l.value;
            }
        }
        let [a, b] = self.locks;
        self.locked = a.locked || b.locked;
        self.receiving = a.receiving || b.receiving;
        // The first decoder's lock, else the second's.
        let l = if a.locked || !b.locked { a } else { b };
        if !l.speed.is_empty() {
            self.last_speed = l.speed;
            if l.locked {
                self.offset_hz = l.offset_hz;
            }
        }
        for c in io.items::<CarrierState>(3) {
            self.now = self.now.max(c.time);
            self.carrier = c.value.present;
            self.snr_db = c.value.snr_db;
            self.level_dbfs = c.value.input_dbfs;
        }
        io.messages_out(0).append(&mut self.events);
        if io.tick() {
            self.publish();
        }
    }
    fn values(&self) -> Values {
        no_values()
    }
    fn results(&self) -> Option<&Results> {
        Some(self.p.results())
    }
    fn commands(&self) -> &Commands {
        PagerCommand::commands()
    }
    fn invoke(&mut self, id: CommandId, args: &Values) -> Result<Option<Record>, CommandError> {
        match PagerCommand::from_id(id).ok_or(CommandError::NoSuchId { id })? {
            PagerCommand::Remove => {
                let row = args.arg(&HEARD_ROW)?;
                if !sdrkit_util_objects::rows::remove_row(&mut self.state.heard, row) {
                    return Err(self.p.no_such_row(&PAGER_HEARD, row));
                }
                self.p.remove_row(&PAGER_HEARD, row);
            }
            PagerCommand::Clear => {
                self.state.clear();
                self.p.clear_rows(&PAGER_PAGES);
                self.p.clear_rows(&PAGER_SPEEDS);
                self.p.clear_rows(&PAGER_HEARD);
                self.publish();
            }
        }
        Ok(None)
    }
    fn save_state(&self) -> Option<StateBlob> {
        self.state.save_state(self.now).ok()
    }
    fn restore_state(&mut self, state: &StateBlob) -> Result<(), StateError> {
        self.state.restore_state(state, self.now)?;
        self.p.clear_rows(&PAGER_PAGES);
        self.p.clear_rows(&PAGER_SPEEDS);
        self.speeds_dirty = true;
        self.publish();
        Ok(())
    }
}
