//! The ACARS results: the message list, the aircraft and channel tables, the counters, the status line, the log lines and the
//! events. The message list is kept through a restart and in the saved state.

use super::{Block, ChannelEvent, FrontState, Message, StopReason};
use crate::messages::{self, Kept};
use crate::report::{self, Heard};
use crate::typed::{
    AcarsAircraftRow, AcarsChannelRow, AcarsFields, AcarsMessageEvent, AcarsMessageRow,
    ACARS_AIRCRAFT, ACARS_CHANNELS, ACARS_MESSAGE, ACARS_MESSAGES,
};
use sdrkit_core::interface::{Hertz, Time};
use sdrkit_core::publish::keys::Family;
use sdrkit_core::publish::Published;
use sdrkit_core::{
    Clock, CommandError, CommandId, CommandSet, Commands, ConfigError, Event, Record, Results,
    StateBlob, StateError, Values,
};
use sdrkit_graph::{
    no_values, Component, ComponentInfo, Format, Io, Kind, Port, RateFrom, RunContext,
};
use sdrkit_proto_aero::aircraft::{Aircraft, AircraftTable};
use sdrkit_util_objects::commands as objcmd;
use sdrkit_util_objects::publish::Point;
use sdrkit_util_objects::shaped::{self, RowOut, Shaped};
use sdrkit_util_objects::{Format as StateFormat, LogRing, Seen, Shift, Stamp, Tables as _};
use serde::{Deserialize, Serialize};
use std::fmt::Write;

/// Messages kept.
pub const MESSAGE_CAP: usize = 200;
/// Aircraft kept.
pub const AIRCRAFT_CAP: usize = 100;
/// Channels kept.
pub const CHANNEL_CAP: usize = 24;

impl Shaped for Heard<Kept> {
    type Row = AcarsMessageRow<'static>;
    fn row(&self, _seen: Option<&Seen>, _now_s: f64, out: RowOut<'_, '_, Self::Row>) {
        let m = &self.item;
        let t = report::text;
        let r = messages::reading_cells(Some(m));
        out.put(&AcarsMessageRow {
            time: self.wall.map(Time),
            signal_s: self.at,
            frequency: Hertz(m.freq_hz),
            level_db: f64::from(m.level_db),
            mode: t(&m.mode),
            registration: t(&m.registration),
            aircraft: t(&m.aircraft),
            flight: t(&m.flight),
            label: t(&m.label),
            label_meaning: t(&m.label_meaning),
            block_id: t(&m.block_id),
            msn: t(&m.msn),
            downlink: m.downlink,
            ack: t(&m.ack),
            text: t(&m.text),
            reading: t(&m.reading),
            reading_kind: r.kind,
            reading_category: r.category,
            cpdlc_imi: r.imi,
            cpdlc_ground: r.ground,
            cpdlc_aircraft: r.aircraft,
            cpdlc_crc_ok: r.crc_ok,
            cpdlc_parsed: r.parsed,
            cpdlc_text: r.text,
            blocks: i64::from(m.blocks),
            bits_fixed: i64::from(m.bits_fixed),
            position: m.position,
            alt_ft: report::finite(m.alt_ft),
        });
    }
}

/// What the aircraft table keeps of the link for each aircraft.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Link {
    /// The channel of the last message, Hz.
    pub freq_hz: f64,
    /// The level of the last downlink, dB.
    pub level_db: f32,
}

impl Shift for Link {
    fn shift_times(&mut self, _by_s: f64) {}
}

/// The map point of an aircraft: its last fix, labelled with its registration and headed along its track.
fn aircraft_point(row: &Aircraft<Link>) -> Option<Point<'_>> {
    let (_, fix) = row.fix?;
    let p = Point::new(fix.position, row.registration.as_str());
    Some(match row.track_deg {
        Some(t) => p.heading(t as f32),
        None => p,
    })
}

/// A channel in the table.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ChannelRow {
    /// Hz.
    pub freq_hz: f64,
    /// Level now.
    pub level_db: f32,
    /// SNR in 7 kHz.
    pub snr_db: f32,
    /// Carrier offset, Hz.
    pub cfo_hz: f32,
    /// Being decoded.
    pub active: bool,
    /// Good blocks.
    pub good: u64,
    /// Bad blocks.
    pub bad: u64,
    /// The wall-clock time of the last good block, when known.
    pub last_heard: Option<f64>,
}

impl Shaped for ChannelRow {
    type Row = AcarsChannelRow;
    fn row(&self, _seen: Option<&Seen>, _now_s: f64, out: RowOut<'_, '_, Self::Row>) {
        out.put(&AcarsChannelRow {
            frequency: Hertz(self.freq_hz),
            level_db: (self.level_db > -120.0).then_some(f64::from(self.level_db)),
            snr_db: f64::from(self.snr_db),
            offset_hz: Hertz(f64::from(self.cfo_hz)),
            active: self.active,
            good: i64::try_from(self.good).unwrap_or(i64::MAX),
            bad: i64::try_from(self.bad).unwrap_or(i64::MAX),
            last_heard: self.last_heard.map(|w| Time(w.floor())),
        });
    }
}

/// Counters of the current input.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Counters {
    /// Good blocks.
    pub blocks_ok: u64,
    /// Bad blocks.
    pub blocks_bad: u64,
    /// Blocks that needed repair.
    pub repaired: u64,
    /// SYN SYN SOH seen.
    pub frames_started: u64,
    /// Messages with a position.
    pub positions: u64,
}

/// What is kept across sessions: the message list and the aircraft.
#[derive(Serialize, Deserialize)]
pub struct Store {
    messages: LogRing<Heard<Kept>>,
    aircraft: AircraftTable<Link>,
}

impl Default for Store {
    fn default() -> Self {
        Store {
            messages: LogRing::new(MESSAGE_CAP),
            aircraft: AircraftTable::new(AIRCRAFT_CAP),
        }
    }
}

impl Shift for Store {
    fn shift_times(&mut self, by_s: f64) {
        self.messages.shift_times(by_s);
        self.aircraft.shift_times(by_s);
    }
}

impl sdrkit_util_objects::Tables for Store {
    const FORMAT: StateFormat = StateFormat::new("avdata.acars", 2);
    fn clear(&mut self) {
        self.messages.clear();
        self.aircraft.clear();
    }
    fn adopt(&mut self, saved: Self) {
        self.messages.adopt(saved.messages);
        self.aircraft.adopt(saved.aircraft);
    }
}

sdrkit_core::commands! {
    /// The commands of [`Tables`].
    pub enum TablesCommand {
        /// Forget every message, aircraft and channel.
        Clear = objcmd::CLEAR,
    }
}

/// The results component (`avdata.acars_tables`).
pub struct Tables {
    published: Published,
    store: Store,
    channels: Vec<ChannelRow>,
    counters: Counters,
    events: Vec<Event>,
    last_message_s: f64,
    now_s: f64,
    watching: String,
    summary: String,
    live: bool,
    clock: Clock,
    configured_once: bool,
}

impl Default for Tables {
    fn default() -> Self {
        Tables {
            published: crate::typed::results(super::ID),
            store: Store::default(),
            channels: Vec::new(),
            counters: Counters::default(),
            events: Vec::new(),
            last_message_s: f64::NEG_INFINITY,
            now_s: 0.0,
            watching: String::new(),
            summary: String::new(),
            live: false,
            clock: Clock::fixed(0.0),
            configured_once: false,
        }
    }
}

impl Tables {
    /// The aircraft heard.
    pub fn aircraft(&self) -> &AircraftTable<Link> {
        &self.store.aircraft
    }

    fn channel(&mut self, f: f64) -> &mut ChannelRow {
        let i = match self.channels.iter().position(|c| c.freq_hz == f) {
            Some(i) => i,
            None => {
                let i = self.channels.partition_point(|c| c.freq_hz < f);
                self.channels.insert(
                    i,
                    ChannelRow {
                        freq_hz: f,
                        level_db: -120.0,
                        ..Default::default()
                    },
                );
                i
            }
        };
        &mut self.channels[i]
    }

    fn block(&mut self, b: &Block) {
        let wall = report::wall(self.live, &self.clock);
        let c = self.channel(b.freq_hz);
        if b.good {
            c.good += 1;
            c.last_heard = wall.or(c.last_heard);
            self.counters.blocks_ok += 1;
            if b.bits_fixed > 0 {
                self.counters.repaired += 1;
            }
        } else {
            c.bad += 1;
            self.counters.blocks_bad += 1;
        }
    }

    fn message(&mut self, m: &Message) {
        let wall = report::wall(self.live, &self.clock);
        self.last_message_s = m.end_s;
        let msg = &m.msg;
        let mut kept = Kept::of(msg, m.freq_hz, m.level_db);
        if kept.position.is_some() {
            self.counters.positions += 1;
        }
        self.events.push(ACARS_MESSAGE.event(
            m.end_s,
            m.freq_hz,
            &AcarsMessageEvent {
                source: &msg.registration,
                target: None,
                text: &msg.text,
                snr_db: None,
                family: Family::Aircraft,
                flight: &kept.flight,
                label: &msg.label,
                downlink: msg.downlink,
                msn: &msg.msn,
                position: kept.position,
                alt_ft: report::finite(kept.alt_ft),
            },
        ));
        if !msg.registration.is_empty() {
            let new = self.store.aircraft.get(None, &msg.registration).is_none();
            let mut at = Stamp::at(m.end_s);
            if let Some(w) = wall {
                at = at.with_wall(w);
            }
            if let Some(a) = self.store.aircraft.heard(None, &msg.registration, at) {
                messages::apply(a, msg, m.end_s);
                messages::fill_motion(a, 1.0, 7200.0);
                a.extra.freq_hz = m.freq_hz;
                if msg.downlink {
                    a.extra.level_db = m.level_db;
                }
                kept.aircraft = messages::aircraft_key(a);
            }
            if new {
                self.published.results_mut().log(format!(
                    "new aircraft {} on {:.3} MHz",
                    msg.registration,
                    m.freq_hz / 1e6
                ));
            }
        }
        self.store.messages.push(Heard {
            at: m.end_s,
            wall,
            item: kept,
        });
    }

    fn event(&mut self, e: &ChannelEvent) {
        let line = if e.start {
            format!(
                "carrier on {:.3} MHz ({:.0} dB over the noise)",
                e.freq_hz / 1e6,
                e.cn_db
            )
        } else {
            let why = match e.reason {
                Some(StopReason::NotAcars) => "no ACARS data",
                Some(StopReason::Preempted) => "a stronger carrier needs the decoder",
                _ => "carrier gone",
            };
            format!("{:.3} MHz idle ({why})", e.freq_hz / 1e6)
        };
        self.published.results_mut().log(line);
        let _ = self.channel(e.freq_hz);
    }

    fn report(&mut self, st: &FrontState) {
        self.now_s = st.time_s;
        self.counters.frames_started = st.frames_started;
        for l in st.channels.iter() {
            let c = self.channel(l.freq_hz);
            c.level_db = l.level_db;
            c.snr_db = l.snr_db;
            c.cfo_hz = l.cfo_hz;
            c.active = l.active;
        }
        // At most CHANNEL_CAP channels: the active ones, then the busiest.
        if self.channels.len() > CHANNEL_CAP {
            self.channels.sort_by(|a, b| {
                b.active
                    .cmp(&a.active)
                    .then((b.good + b.bad).cmp(&(a.good + a.bad)))
            });
            self.channels.truncate(CHANNEL_CAP);
            self.channels
                .sort_by(|a, b| a.freq_hz.total_cmp(&b.freq_hz));
        }
        self.watching.clear();
        if st.list.is_empty() {
            self.watching.push_str("every carrier on the 25 kHz grid");
        } else {
            for (i, f) in st.list.iter().enumerate() {
                let sep = if i == 0 { "" } else { " " };
                let _ = write!(self.watching, "{sep}{:.4}", f / 1e6);
            }
        }
        self.publish(&st.refused);
    }

    fn publish(&mut self, refused: &str) {
        let now = self.now_s;
        let mut n_active = 0;
        let mut best: Option<&ChannelRow> = None;
        for c in self.channels.iter().filter(|c| c.active) {
            n_active += 1;
            if best.is_none_or(|b| c.snr_db > b.snr_db) {
                best = Some(c);
            }
        }
        let state = if now - self.last_message_s <= 30.0 {
            2
        } else if n_active > 0 {
            1
        } else {
            0
        };
        let c = &self.counters;
        let tot = c.blocks_ok + c.blocks_bad;
        self.summary.clear();
        let _ = if !refused.is_empty() {
            write!(self.summary, "ACARS: {refused}")
        } else if state == 0 {
            write!(self.summary, "ACARS: searching")
        } else {
            write!(
                self.summary,
                "ACARS: {}, {n_active} channels, {} good, {} bad, {} aircraft",
                if state == 2 { "decoding" } else { "signal" },
                c.blocks_ok,
                c.blocks_bad,
                self.store.aircraft.len()
            )
        };
        let (snr, cfo) = (best.map(|c| c.snr_db), best.map(|c| f64::from(c.cfo_hz)));
        let p = &mut self.published;
        report::status(p.results_mut(), state, snr, None, cfo, &self.summary);
        let count = |n: u64| i64::try_from(n).unwrap_or(i64::MAX);
        p.put(&AcarsFields {
            frames_ok: count(c.blocks_ok),
            frames_bad: count(c.blocks_bad),
            blocks_repaired: count(c.repaired),
            frames_started: count(c.frames_started),
            positions: count(c.positions),
            channels_active: n_active,
            aircraft: count(self.store.aircraft.len() as u64),
            messages: count(self.store.messages.len() as u64),
            good_pct: (tot > 0).then(|| 100.0 * c.blocks_ok as f64 / tot as f64),
            watching: &self.watching,
        });
        shaped::log_rows(&self.store.messages, &ACARS_MESSAGES, now, p);
        shaped::rows_by(
            self.store.aircraft.rows(),
            &ACARS_AIRCRAFT,
            p,
            |_| true,
            |r, out| {
                let mut address = [0u8; 7];
                let s = r.record.standard(Some(r.seen), &mut address);
                let x = &r.record.extra;
                out.put(&AcarsAircraftRow {
                    address: s.address,
                    registration: s.registration,
                    country: s.country,
                    flight: s.flight,
                    label: s.label,
                    messages: s.messages,
                    position: s.position,
                    alt_ft: s.alt_ft,
                    track_deg: s.track_deg,
                    gs_kt: s.gs_kt,
                    vs_fpm: s.vs_fpm,
                    last_heard: s.last_heard.map(Time),
                    frequency: Hertz(x.freq_hz),
                    level_db: f64::from(x.level_db),
                });
            },
        );
        shaped::points_by(
            self.store.aircraft.rows(),
            "aircraft/",
            p.results_mut(),
            aircraft_point,
        );
        shaped::rows_of(
            &self.channels,
            |c, w| write!(w, "{}", c.freq_hz.round() as u64),
            &ACARS_CHANNELS,
            now,
            p,
        );
    }
}

impl Component for Tables {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "avdata.acars_tables",
            vec![
                Port::typed::<Message>("messages"),
                Port::typed::<ChannelEvent>("events"),
                Port::typed::<FrontState>("state"),
                Port::typed::<Block>("blocks"),
            ],
            vec![Port::new("out", Kind::Messages).rate_from(RateFrom::Fixed(0.0))],
        )
    }

    fn configure(&mut self, _: &[Format]) -> Result<Vec<Format>, ConfigError> {
        // A restart forgets the signal and keeps the lists: their times move back by the old signal time.
        if self.configured_once {
            let by = -self.now_s;
            self.store.shift_times(by);
            self.last_message_s += by;
        }
        self.configured_once = true;
        self.now_s = 0.0;
        self.counters = Counters::default();
        self.channels.clear();
        Ok(vec![Format::new(Kind::Messages, 0.0)])
    }

    fn attach(&mut self, ctx: &RunContext) {
        self.live = ctx.live;
        self.clock = ctx.clock.clone();
    }

    fn process(&mut self, io: &mut Io) {
        for b in io.items::<Block>(3) {
            self.block(&b.value);
        }
        for e in io.items::<ChannelEvent>(1) {
            self.event(&e.value);
        }
        for m in io.items::<Message>(0) {
            self.message(&m.value);
        }
        for s in io.items::<FrontState>(2) {
            self.report(&s.value);
        }
        if !self.events.is_empty() {
            io.messages_out(0).append(&mut self.events);
        }
    }

    fn values(&self) -> Values {
        no_values()
    }

    fn commands(&self) -> &Commands {
        TablesCommand::commands()
    }

    fn invoke(&mut self, id: CommandId, _args: &Values) -> Result<Option<Record>, CommandError> {
        match TablesCommand::from_id(id).ok_or(CommandError::NoSuchId { id })? {
            TablesCommand::Clear => {
                self.store.clear();
                self.channels.clear();
                self.counters = Counters::default();
            }
        }
        self.publish("");
        Ok(None)
    }

    fn forget(&mut self, kept: &sdrkit_graph::Kept) {
        self.channels.clear();
        self.counters = Counters::default();
        self.last_message_s = f64::NEG_INFINITY;
        if !kept.table(ACARS_MESSAGES.key) {
            self.store.messages.clear();
        }
        if !kept.table(ACARS_AIRCRAFT.key) {
            self.store.aircraft.clear();
        }
        self.publish("");
    }

    fn results(&self) -> Option<&Results> {
        Some(self.published.results())
    }

    fn save_state(&self) -> Option<StateBlob> {
        self.store.save_state(self.now_s).ok()
    }

    fn restore_state(&mut self, state: &StateBlob) -> Result<(), StateError> {
        self.store.restore_state(state, self.now_s)?;
        self.publish("");
        Ok(())
    }
}
