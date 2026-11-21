//! The session book: observations become sessions that begin, update and end. A track is the same thing seen again within
//! a gap. A thread is the messages to one pager address within a window. A conversation is the calls on one channel between
//! the same parties. Every change returns an event of the book's own kinds.

use std::collections::{HashMap, VecDeque};

use std::sync::Arc;

use sdrkit_core::command::{Command, CommandError, CommandId, CommandSet, Commands};
use sdrkit_core::owner::{Owner, OwnerKey};
use sdrkit_core::publish::keys::{self, Family, Urgency};
use sdrkit_core::publish::{field, Field, Published, TableDef};
use sdrkit_core::settings::{Argument, Setting, SettingError, SettingId, SettingSet};
use sdrkit_core::{Event, EventDef, Record, Results, ResultsDecl, Schema, Value, Values};
use sdrkit_svc_events::service::no_such_row;
use sdrkit_svc_events::{Observation, Position};

sdrkit_core::choice_enum! {
    /// What kind of session.
    pub enum SessionType {
        /// The same thing across gaps.
        Track = ("track", "Track"),
        /// A pager address within a window.
        Thread = ("thread", "Thread"),
        /// Calls on a channel.
        Conversation = ("conversation", "Conversation"),
    }
}

/// The session id.
pub const SESSION: Field<field::I64> = Field::new("session", "Session", "");
/// The session type.
pub const TYPE: Field<field::Choice<SessionType>> = Field::new("type", "Type", "");
/// The first part of the event kind key that the session follows.
pub const STACK: Field<field::Text> = Field::new("stack", "Stack", "");
/// The observations in the session.
pub const COUNT: Field<field::I64> = Field::new("count", "Observations", "");
/// How long the session lasted.
pub const DURATION: Field<field::Duration> = Field::new("duration", "Duration", "s");
/// The parties of a conversation, joined by `/`.
pub const PARTIES: Field<field::Text> = Field::new("parties", "Parties", "");

const SESSION_FIELDS: &[&dyn sdrkit_core::publish::Declare] = &[
    &SESSION,
    &TYPE,
    &keys::SOURCE,
    &STACK,
    &keys::FAMILY,
    &COUNT,
    &DURATION,
    &keys::URGENCY,
    &PARTIES,
    &keys::POSITION,
];

/// A session began.
pub static SESSION_BEGIN: EventDef =
    EventDef::new("session.begin", "Session began", SESSION_FIELDS);
/// A session got new observations.
pub static SESSION_UPDATE: EventDef =
    EventDef::new("session.update", "Session updated", SESSION_FIELDS);
/// A session ended.
pub static SESSION_END: EventDef = EventDef::new("session.end", "Session ended", SESSION_FIELDS);

/// The event kinds of the session book, for the engine's catalog.
pub static SESSION_EVENTS: [&EventDef; 3] = [&SESSION_BEGIN, &SESSION_UPDATE, &SESSION_END];

/// The owner key of the session book.
pub const KEY: OwnerKey = OwnerKey::of("service.sessions");

/// A track ends after this long without a report, if its family has no gap of its own.
pub const TRACK_GAP: Setting<field::Duration> =
    Setting::new("track_gap", "Track gap", "s", 300.0).range(1.0, 86_400.0, 1.0);
/// The track gap of aircraft.
pub const AIRCRAFT_GAP: Setting<field::Duration> =
    Setting::new("aircraft_gap", "Aircraft gap", "s", 120.0).range(1.0, 86_400.0, 1.0);
/// The track gap of vessels.
pub const VESSEL_GAP: Setting<field::Duration> =
    Setting::new("vessel_gap", "Vessel gap", "s", 900.0).range(1.0, 86_400.0, 1.0);
/// The track gap of sondes.
pub const SONDE_GAP: Setting<field::Duration> =
    Setting::new("sonde_gap", "Sonde gap", "s", 600.0).range(1.0, 86_400.0, 1.0);
/// A pager thread ends after this long without a message.
pub const THREAD_WINDOW: Setting<field::Duration> =
    Setting::new("thread_window", "Thread window", "s", 600.0).range(1.0, 86_400.0, 1.0);
/// A conversation ends after this long of silence on its channel.
pub const CONVERSATION_GAP: Setting<field::Duration> =
    Setting::new("conversation_gap", "Conversation gap", "s", 30.0).range(1.0, 86_400.0, 1.0);
/// The most track points kept per session. The oldest go first.
pub const TRAIL_CAP: Setting<field::I64> =
    Setting::new("trail_cap", "Most track points", "", 1000).range(1.0, 100_000.0, 1.0);
/// The most messages kept per thread.
pub const MESSAGES_CAP: Setting<field::I64> =
    Setting::new("messages_cap", "Most thread messages", "", 100).range(1.0, 10_000.0, 1.0);
/// The most ended sessions kept.
pub const HISTORY_CAP: Setting<field::I64> =
    Setting::new("history_cap", "Most ended sessions", "", 10_000).range(0.0, 10_000.0, 1.0);
/// An update event comes at most this often per session.
pub const UPDATE_EVERY: Setting<field::Duration> =
    Setting::new("update_every", "Update every", "s", 10.0).range(0.0, 86_400.0, 1.0);
/// The most open sessions. The one seen longest ago ends first.
pub const OPEN_CAP: Setting<field::I64> =
    Setting::new("open_cap", "Most open sessions", "", 20_000).range(1.0, 20_000.0, 1.0);

sdrkit_core::settings! {
    /// The settings of the session book.
    pub enum SessionSetting {
        /// [`TRACK_GAP`].
        TrackGap = TRACK_GAP,
        /// [`AIRCRAFT_GAP`].
        AircraftGap = AIRCRAFT_GAP,
        /// [`VESSEL_GAP`].
        VesselGap = VESSEL_GAP,
        /// [`SONDE_GAP`].
        SondeGap = SONDE_GAP,
        /// [`THREAD_WINDOW`].
        ThreadWindow = THREAD_WINDOW,
        /// [`CONVERSATION_GAP`].
        ConversationGap = CONVERSATION_GAP,
        /// [`TRAIL_CAP`].
        TrailCap = TRAIL_CAP,
        /// [`MESSAGES_CAP`].
        MessagesCap = MESSAGES_CAP,
        /// [`HISTORY_CAP`].
        HistoryCap = HISTORY_CAP,
        /// [`UPDATE_EVERY`].
        UpdateEvery = UPDATE_EVERY,
        /// [`OPEN_CAP`].
        OpenCap = OPEN_CAP,
    }
}

/// First seen.
pub const FIRST_SEEN: Field<field::Time> = Field::new("first_seen", "First seen", "");
/// Last seen.
pub const LAST_SEEN: Field<field::Time> = Field::new("last_seen", "Last seen", "");
/// True while the session is open.
pub const OPEN: Field<field::Bool> = Field::new("open", "Open", "");

/// The open sessions and the ended ones kept. The row key is the session id.
pub const SESSIONS: TableDef = TableDef {
    key: "sessions",
    label: "Sessions",
    columns: &[
        &TYPE,
        &keys::SOURCE,
        &STACK,
        &keys::FAMILY,
        &FIRST_SEEN,
        &LAST_SEEN,
        &COUNT,
        &DURATION,
        &keys::URGENCY,
        &PARTIES,
        &keys::POSITION,
        &keys::FREQ_HZ,
        &OPEN,
    ],
    cap: 30_000,
};

/// The id of a session, as the table shows it.
pub const ROW: Argument<field::Text> = Argument::new(Field::new("row", "Session", ""));

/// Ends one open session.
pub const END: Command = Command::new("end", "End")
    .help("Ends one open session.")
    .args(&[&ROW]);
/// Forgets the ended sessions.
pub const FORGET: Command = Command::new("forget", "Forget").help("Forgets the ended sessions.");

sdrkit_core::commands! {
    /// The commands of the session book.
    pub enum SessionCommand {
        /// [`END`].
        End = END,
        /// [`FORGET`].
        Forget = FORGET,
    }
}

/// One point of a track.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TrackPoint {
    /// Unix seconds.
    pub time: f64,
    /// Where.
    pub position: Position,
}

/// A session.
#[derive(Clone, Debug, PartialEq)]
pub struct Session {
    /// Unique within the book.
    pub id: u64,
    /// Track, thread or conversation.
    pub ty: SessionType,
    /// The family.
    pub family: Family,
    /// The first part of the event kind key.
    pub stack: String,
    /// What the session follows: the identity, or the channel for a conversation.
    pub identity: String,
    /// The parties of a conversation (the sources and targets that the stack named).
    pub parties: Vec<String>,
    /// First seen, Unix seconds.
    pub first_seen: f64,
    /// Last seen, Unix seconds.
    pub last_seen: f64,
    /// Observations in the session.
    pub count: u64,
    /// The frequency first seen on, Hz.
    pub freq_hz: f64,
    /// The newest fields.
    pub last_fields: Record,
    /// The track, oldest first.
    pub trail: Vec<TrackPoint>,
    /// The texts of a thread, oldest first.
    pub messages: Vec<String>,
    /// The highest urgency seen.
    pub urgency: Urgency,
    /// True while the session is open.
    pub open: bool,
    last_update_event: f64,
}

impl Session {
    /// How long the session lasted, seconds.
    pub fn duration_s(&self) -> f64 {
        self.last_seen - self.first_seen
    }

    fn event(&self, def: &'static EventDef) -> Event {
        let parties = self.parties.join("/");
        def.event(self.last_seen, self.freq_hz)
            .set(&SESSION, i64::try_from(self.id).unwrap_or(i64::MAX))
            .set(&TYPE, self.ty)
            .set(&keys::SOURCE, &self.identity)
            .set(&STACK, &self.stack)
            .set(&keys::FAMILY, self.family)
            .set(&COUNT, i64::try_from(self.count).unwrap_or(i64::MAX))
            .set(&DURATION, self.duration_s())
            .set(&keys::URGENCY, self.urgency)
            .opt(&PARTIES, (!parties.is_empty()).then_some(parties.as_str()))
            .opt(
                &keys::POSITION,
                self.trail
                    .last()
                    .map(|p| (p.position.lat_deg, p.position.lon_deg)),
            )
    }
}

/// A change in the book.
#[derive(Clone, Debug, PartialEq)]
pub enum SessionChange {
    /// A session began.
    Begin(u64),
    /// A session got a new observation (at most once per `update_every_s`).
    Update(u64),
    /// A session ended.
    End(u64),
}

/// The session book: the owner `service.sessions`.
pub struct SessionBook {
    values: Values,
    open: HashMap<(SessionType, String, String), Session>,
    history: VecDeque<Session>,
    next_id: u64,
    ended: Vec<(SessionChange, Event)>,
    p: Published,
}

impl Default for SessionBook {
    fn default() -> Self {
        SessionBook::new()
    }
}

fn session_key(o: &Observation<'_>) -> Option<(SessionType, String, Vec<String>)> {
    match o.family {
        Family::Pager => Some((SessionType::Thread, o.identity?.to_owned(), vec![])),
        Family::Voice => {
            let channel = o.channel.map_or_else(
                || format!("{:.0}", (o.event.freq_hz / 100.0).round() * 100.0),
                str::to_owned,
            );
            let mut parties: Vec<String> = [o.identity, o.target]
                .into_iter()
                .flatten()
                .map(str::to_owned)
                .collect();
            parties.sort();
            parties.dedup();
            let key = if parties.is_empty() {
                channel
            } else {
                format!("{channel} {}", parties.join("/"))
            };
            Some((SessionType::Conversation, key, parties))
        }
        Family::Receiver | Family::Other => None,
        _ => Some((SessionType::Track, o.identity?.to_owned(), vec![])),
    }
}

impl SessionBook {
    /// A book with the default settings.
    pub fn new() -> Self {
        SessionBook {
            values: Values::defaults(&SessionSetting::schema()),
            open: HashMap::new(),
            history: VecDeque::new(),
            next_id: 1,
            ended: Vec::new(),
            p: Published::from_decl(Arc::new(
                ResultsDecl::new()
                    .event(&SESSION_BEGIN)
                    .event(&SESSION_UPDATE)
                    .event(&SESSION_END),
            ))
            .with_tables(&[&SESSIONS]),
        }
    }

    fn cap(&self, s: &Setting<field::I64>) -> usize {
        usize::try_from(self.values.get(s)).unwrap_or(0)
    }

    fn gap(&self, ty: SessionType, family: Family) -> f64 {
        let v = &self.values;
        match (ty, family) {
            (SessionType::Track, Family::Aircraft) => v.get(&AIRCRAFT_GAP),
            (SessionType::Track, Family::Vessel) => v.get(&VESSEL_GAP),
            (SessionType::Track, Family::Sonde) => v.get(&SONDE_GAP),
            (SessionType::Track, _) => v.get(&TRACK_GAP),
            (SessionType::Thread, _) => v.get(&THREAD_WINDOW),
            (SessionType::Conversation, _) => v.get(&CONVERSATION_GAP),
        }
    }

    fn show(&mut self, id: u64) {
        let Some(s) = self
            .open
            .values()
            .find(|s| s.id == id)
            .or_else(|| self.history.iter().rev().find(|s| s.id == id))
        else {
            return;
        };
        let Some(mut row) = self.p.row_mut(&SESSIONS, &id.to_string()) else {
            return;
        };
        let parties = s.parties.join("/");
        row.set(&TYPE, s.ty);
        row.set(&keys::SOURCE, &s.identity);
        row.set(&STACK, &s.stack);
        row.set(&keys::FAMILY, s.family);
        row.set(&FIRST_SEEN, s.first_seen);
        row.set(&LAST_SEEN, s.last_seen);
        row.set(&COUNT, i64::try_from(s.count).unwrap_or(i64::MAX));
        row.set(&DURATION, s.duration_s());
        row.set(&keys::URGENCY, s.urgency);
        row.opt(&PARTIES, (!parties.is_empty()).then_some(parties.as_str()));
        row.opt(
            &keys::POSITION,
            s.trail
                .last()
                .map(|p| (p.position.lat_deg, p.position.lon_deg)),
        );
        row.set(&keys::FREQ_HZ, s.freq_hz);
        row.set(&OPEN, s.open);
    }

    /// Adds an observation and returns the changes with their events. Sessions of other things that have timed out end first,
    /// as [`SessionBook::tick`] does. The book's own events make no session.
    pub fn observe(&mut self, o: &Observation<'_>) -> Vec<(SessionChange, Event)> {
        let mut out = self.tick(o.unix_time);
        if SESSION_EVENTS.iter().any(|d| o.event.is(d)) {
            return out;
        }
        let Some((ty, ident, parties)) = session_key(o) else {
            return out;
        };
        let kind = o.event.kind();
        let stack = kind.split('.').next().unwrap_or(kind).to_owned();
        let key = (ty, stack.clone(), ident.clone());
        let family = o.family;
        let gap = self.gap(ty, family);
        if let Some(s) = self.open.get(&key) {
            if o.unix_time - s.last_seen > gap {
                if let Some(s) = self.open.remove(&key) {
                    out.push(self.close(s));
                }
            }
        }
        let (trail_cap, messages_cap) = (self.cap(&TRAIL_CAP), self.cap(&MESSAGES_CAP));
        let update_every = self.values.get(&UPDATE_EVERY);
        let is_new = !self.open.contains_key(&key);
        let s = self.open.entry(key).or_insert_with(|| {
            let id = self.next_id;
            self.next_id += 1;
            Session {
                id,
                ty,
                family,
                stack,
                identity: ident,
                parties,
                first_seen: o.unix_time,
                last_seen: o.unix_time,
                count: 0,
                freq_hz: o.event.freq_hz,
                last_fields: o.event.fields.clone(),
                trail: Vec::new(),
                messages: Vec::new(),
                urgency: Urgency::None,
                open: true,
                last_update_event: o.unix_time,
            }
        });
        s.count += 1;
        s.last_seen = s.last_seen.max(o.unix_time);
        s.last_fields.clone_from(&o.event.fields);
        if o.urgency != Urgency::None {
            s.urgency = o.urgency;
        }
        if let Some(p) = o.position {
            if s.trail.last().is_none_or(|l| l.position != p) {
                s.trail.push(TrackPoint {
                    time: o.unix_time,
                    position: p,
                });
                if s.trail.len() > trail_cap {
                    let extra = s.trail.len() - trail_cap;
                    s.trail.drain(..extra);
                }
            }
        }
        if ty == SessionType::Thread {
            if let Some(t) = o.text {
                s.messages.push(t.to_owned());
                if s.messages.len() > messages_cap {
                    s.messages.remove(0);
                }
            }
        }
        let id = s.id;
        if is_new {
            out.push((SessionChange::Begin(s.id), s.event(&SESSION_BEGIN)));
        } else if o.unix_time - s.last_update_event >= update_every {
            s.last_update_event = o.unix_time;
            out.push((SessionChange::Update(s.id), s.event(&SESSION_UPDATE)));
        }
        self.show(id);
        if self.open.len() > self.cap(&OPEN_CAP) {
            if let Some(k) = self
                .open
                .iter()
                .min_by(|a, b| a.1.last_seen.total_cmp(&b.1.last_seen))
                .map(|(k, _)| k.clone())
            {
                if let Some(s) = self.open.remove(&k) {
                    out.push(self.close(s));
                }
            }
        }
        out
    }

    fn close(&mut self, mut s: Session) -> (SessionChange, Event) {
        s.open = false;
        let id = s.id;
        let ev = (SessionChange::End(s.id), s.event(&SESSION_END));
        self.history.push_back(s);
        self.show(id);
        while self.history.len() > self.cap(&HISTORY_CAP) {
            if let Some(old) = self.history.pop_front() {
                self.p.remove_row(&SESSIONS, &old.id.to_string());
            }
        }
        ev
    }

    /// Ends the sessions whose gap has passed at `now` (Unix seconds). The changes include the sessions that the command
    /// `end` ended since the last call.
    pub fn tick(&mut self, now: f64) -> Vec<(SessionChange, Event)> {
        let expired: Vec<_> = self
            .open
            .iter()
            .filter(|(k, s)| now - s.last_seen > self.gap(k.0, s.family))
            .map(|(k, _)| k.clone())
            .collect();
        let mut out = std::mem::take(&mut self.ended);
        let mut ended: Vec<Session> = expired
            .into_iter()
            .filter_map(|k| self.open.remove(&k))
            .collect();
        ended.sort_by_key(|s| s.id);
        for s in ended {
            out.push(self.close(s));
        }
        out
    }

    /// Ends every open session. The engine calls it when it stops.
    pub fn end_all(&mut self) -> Vec<(SessionChange, Event)> {
        let mut all: Vec<Session> = self.open.drain().map(|(_, s)| s).collect();
        all.sort_by_key(|s| s.id);
        let mut out = std::mem::take(&mut self.ended);
        for s in all {
            out.push(self.close(s));
        }
        out
    }

    /// A session by id, open or ended.
    pub fn get(&self, id: u64) -> Option<&Session> {
        self.open
            .values()
            .find(|s| s.id == id)
            .or_else(|| self.history.iter().rev().find(|s| s.id == id))
    }
}

impl Owner for SessionBook {
    fn schema(&self) -> &Arc<Schema> {
        self.values.schema()
    }
    fn values(&self) -> Option<&Values> {
        Some(&self.values)
    }
    fn apply(&mut self, id: SettingId, value: &Value) -> Result<(), SettingError> {
        let s = SessionSetting::from_id(id).ok_or(SettingError::NoSuchId { id })?;
        self.values.set(s.id(), value)
    }
    fn results_decl(&self) -> &Arc<ResultsDecl> {
        self.p.results().decl()
    }
    fn results(&self) -> &Results {
        self.p.results()
    }
    fn problem(&mut self, text: &str) {
        self.p.results_mut().problem(text);
    }
    fn commands(&self) -> &Commands {
        SessionCommand::commands()
    }
    fn invoke(&mut self, id: CommandId, args: &Values) -> Result<Option<Record>, CommandError> {
        match SessionCommand::from_id(id).ok_or(CommandError::NoSuchId { id })? {
            SessionCommand::End => {
                let row = args.arg(&ROW)?;
                let key = row
                    .trim()
                    .parse::<u64>()
                    .ok()
                    .and_then(|n| self.open.iter().find(|(_, s)| s.id == n))
                    .map(|(k, _)| k.clone())
                    .ok_or_else(|| no_such_row(&self.p, &SESSIONS, row))?;
                if let Some(s) = self.open.remove(&key) {
                    let change = self.close(s);
                    self.ended.push(change);
                }
            }
            SessionCommand::Forget => {
                for s in std::mem::take(&mut self.history) {
                    self.p.remove_row(&SESSIONS, &s.id.to_string());
                }
            }
        }
        Ok(None)
    }
}
