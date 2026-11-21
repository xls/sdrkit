//! The message store: every observation as a message, searchable, newest first, with a throttle for repeated reports,
//! retention by rows, size and age, and a journal of one JSON line per message in the host's storage.

use std::borrow::Cow;
use std::collections::{BTreeMap, HashMap, VecDeque};
use std::io::Write as _;
use std::sync::Arc;

use sdrkit_core::command::{Command, CommandError, CommandId, CommandSet, Commands};
use sdrkit_core::owner::{Owner, OwnerKey};
use sdrkit_core::publish::keys::{self, Family, Urgency};
use sdrkit_core::publish::{field, Declare, Field, Published, TableDef};
use sdrkit_core::settings::{Argument, Setting, SettingError, SettingId, SettingSet};
use sdrkit_core::storage::{Storage, StoreWriter, WriteMode};
use sdrkit_core::{
    EventKindId, Record, Results, ResultsDecl, Schema, StorePath, TableId, Value, Values,
};
use sdrkit_svc_events::service::{failed, path_arg, reply};
use sdrkit_svc_events::{FieldRef, KindSet, Kinds, NameError, Observation};

use crate::error::StoreError;
use crate::export::{export, Export};
use crate::message::Message;

/// The owner key of the store.
pub const KEY: OwnerKey = OwnerKey::of("service.store");

/// The most messages kept.
pub const MAX_ROWS: Setting<field::I64> =
    Setting::new("max_rows", "Most messages", "", 100_000).range(1.0, 1e9, 1.0);
/// The most bytes kept (the size of the messages in the journal).
pub const MAX_BYTES: Setting<field::I64> =
    Setting::new("max_bytes", "Most bytes", "", 64 << 20).range(1.0, 1e12, 1.0);
/// The oldest message kept, seconds. 0 keeps messages however old.
pub const MAX_AGE: Setting<field::Duration> =
    Setting::new("max_age", "Oldest message", "s", 30.0 * 86_400.0).range(0.0, 1e10, 1.0);
/// A report of the same kind and identity with the same text, within this time of the last one, is folded into it. 0 keeps
/// every report.
pub const FOLD: Setting<field::Duration> =
    Setting::new("fold", "Fold repeats within", "s", 30.0).range(0.0, 86_400.0, 1.0);

sdrkit_core::settings! {
    /// The settings of the store.
    pub enum StoreSetting {
        /// [`MAX_ROWS`].
        MaxRows = MAX_ROWS,
        /// [`MAX_BYTES`].
        MaxBytes = MAX_BYTES,
        /// [`MAX_AGE`].
        MaxAge = MAX_AGE,
        /// [`FOLD`].
        Fold = FOLD,
    }
}

/// Words that must all be in a message (any case). `"two words"` is a phrase, `-word` must not be there, `key:value` needs
/// that field to hold the value.
pub const TEXT: Setting<field::Text> = Setting::new("text", "Words", "", "");
/// An event kind or a pattern (`"adsb.*"`). Empty for every kind.
pub const KIND: Setting<field::Text> = Setting::new("kind", "Kind", "", "");
/// A family key. Empty for every family.
pub const FAMILY: Setting<field::Text> = Setting::new("family", "Family", "", "");
/// The identity, exactly (any case). Empty for every identity.
pub const IDENTITY: Setting<field::Text> = Setting::new("identity", "Identity", "", "");
/// Seen at or after, Unix seconds. 0 for no limit.
pub const SINCE: Setting<field::Time> = Setting::new("since", "Since", "", 0.0);
/// Seen at or before, Unix seconds. 0 for no limit.
pub const UNTIL: Setting<field::Time> = Setting::new("until", "Until", "", 0.0);
/// Only messages with a position.
pub const WITH_POSITION: Setting<field::Bool> =
    Setting::new("with_position", "With a position", "", false);
/// Only urgent messages.
pub const URGENT_ONLY: Setting<field::Bool> = Setting::new("urgent_only", "Urgent only", "", false);
/// The most results.
pub const LIMIT: Setting<field::I64> =
    Setting::new("limit", "Most results", "", 100).range(1.0, 10_000.0, 1.0);
/// Results to skip, for paging.
pub const OFFSET: Setting<field::I64> = Setting::new("offset", "Skip", "", 0).range(0.0, 1e9, 1.0);
/// The id of a message, as the table shows it.
pub const ROW: Argument<field::Text> = Argument::new(Field::new("row", "Message", ""));
/// The format of an export.
pub const FORMAT: Setting<field::Choice<Export>> = Setting::choice("format", "Format", Export::Csv);
/// The file in the host's storage that an export writes.
pub const PATH: Argument<field::Text> = Argument::new(Field::new("path", "File", ""));
/// The number of messages that an export wrote.
pub const WRITTEN: Field<field::I64> = Field::new("written", "Messages written", "");

/// Sets what the table `messages` shows.
pub const SEARCH: Command = Command::new("search", "Search")
    .help("Shows the messages that match, newest first.")
    .args(&[
        &TEXT,
        &KIND,
        &FAMILY,
        &IDENTITY,
        &SINCE,
        &UNTIL,
        &WITH_POSITION,
        &URGENT_ONLY,
        &LIMIT,
        &OFFSET,
    ]);
/// Deletes one message.
pub const DELETE: Command = Command::new("delete", "Delete")
    .help("Deletes one message.")
    .args(&[&ROW]);
/// Deletes every message.
pub const CLEAR: Command = Command::new("clear", "Clear").help("Deletes every message.");
/// Rewrites the journal with only the messages kept.
pub const COMPACT: Command = Command::new("compact", "Compact");
/// Exports the messages that the last search found.
pub const EXPORT: Command = Command::new("export", "Export")
    .help("Writes the messages of the last search to a file in a file format.")
    .args(&[&FORMAT, &PATH])
    .reply(&[&WRITTEN]);

sdrkit_core::commands! {
    /// The commands of the store.
    pub enum StoreCommand {
        /// [`SEARCH`].
        Search = SEARCH,
        /// [`DELETE`].
        Delete = DELETE,
        /// [`CLEAR`].
        Clear = CLEAR,
        /// [`COMPACT`].
        Compact = COMPACT,
        /// [`EXPORT`].
        Export = EXPORT,
    }
}

/// Messages kept.
pub const ROWS: Field<field::I64> = Field::new("rows", "Messages", "");
/// Their size, bytes.
pub const BYTES: Field<field::I64> = Field::new("bytes", "Bytes", "");
/// Reports folded into earlier messages.
pub const FOLDED: Field<field::I64> = Field::new("folded", "Folded", "");
/// Messages dropped by retention.
pub const PRUNED: Field<field::I64> = Field::new("pruned", "Pruned", "");
/// Journal lines that could not be read.
pub const BAD_LINES: Field<field::I64> = Field::new("bad_lines", "Unreadable lines", "");
/// Messages that the last search matched.
pub const MATCHES: Field<field::I64> = Field::new("matches", "Matches", "");

/// First seen.
pub const FIRST: Field<field::Time> = Field::new("time", "First seen", "");
/// Last seen.
pub const LAST: Field<field::Time> = Field::new("last_time", "Last seen", "");
/// The kind key.
pub const KIND_COL: Field<field::Text> = Field::new("kind", "Kind", "");
/// Distance from the receiver.
pub const DISTANCE_KM: Field<field::F64> = Field::new("distance_km", "Distance", "km");
/// Folded repeats.
pub const REPEATS: Field<field::I64> = Field::new("repeats", "Repeats", "");

/// The messages that the last search found, newest first. The row key is the message id.
pub const MESSAGES: TableDef = TableDef {
    key: "messages",
    label: "Messages",
    columns: &[
        &FIRST,
        &LAST,
        &KIND_COL,
        &keys::FAMILY,
        &keys::URGENCY,
        &keys::SOURCE,
        &keys::FREQ_HZ,
        &keys::POSITION,
        &DISTANCE_KM,
        &keys::TEXT,
        &REPEATS,
    ],
    cap: 10_000,
};

const STATUS: &[&dyn Declare] = &[&ROWS, &BYTES, &FOLDED, &PRUNED, &BAD_LINES, &MATCHES];

/// What an insert did.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Added {
    /// A new message, with its id.
    New(u64),
    /// Folded into this earlier message by the throttle.
    Folded(u64),
}

impl Added {
    /// The message's id.
    pub fn id(self) -> u64 {
        match self {
            Added::New(id) | Added::Folded(id) => id,
        }
    }
}

enum Term {
    Has(String),
    Not(String),
    Field(FieldRef, String),
}

/// A search, resolved against the event kinds.
pub struct Query {
    terms: Vec<Term>,
    kinds: Option<KindSet>,
    family: Option<Family>,
    identity: Option<String>,
    since: Option<f64>,
    until: Option<f64>,
    with_position: bool,
    urgent_only: bool,
    limit: usize,
    offset: usize,
}

impl Query {
    /// The search of the arguments of [`SEARCH`].
    ///
    /// # Errors
    ///
    /// [`NameError`] if the words name a field that no kind has, or the kind or family is unknown. The position is a byte
    /// offset in the argument that holds the name.
    pub fn new(kinds: &Kinds, args: &Values) -> Result<Query, NameError> {
        let kind = args.get(&KIND);
        let set = if kind.trim().is_empty() {
            None
        } else {
            Some(kinds.kinds(kind, 0)?)
        };
        let family = args.get(&FAMILY);
        let family = if family.trim().is_empty() {
            None
        } else {
            Some(Kinds::family(family, 0)?)
        };
        let identity = args.get(&IDENTITY).trim();
        let pos = |t: f64| (t > 0.0).then_some(t);
        Ok(Query {
            terms: terms(kinds, args.get(&TEXT), set.as_ref())?,
            kinds: set,
            family,
            identity: (!identity.is_empty()).then(|| identity.to_owned()),
            since: pos(args.get(&SINCE)),
            until: pos(args.get(&UNTIL)),
            with_position: args.get(&WITH_POSITION),
            urgent_only: args.get(&URGENT_ONLY),
            limit: usize::try_from(args.get(&LIMIT)).unwrap_or(100),
            offset: usize::try_from(args.get(&OFFSET)).unwrap_or(0),
        })
    }

    fn matches(&self, m: &Message) -> bool {
        if self.kinds.as_ref().is_some_and(|k| !k.contains(m.kind))
            || self.family.is_some_and(|f| f != m.family)
            || self.identity.as_ref().is_some_and(|i| {
                m.identity
                    .as_ref()
                    .is_none_or(|mi| !mi.eq_ignore_ascii_case(i))
            })
            || self.since.is_some_and(|s| m.last_time < s)
            || self.until.is_some_and(|u| m.time > u)
            || (self.with_position && m.position.is_none())
            || (self.urgent_only && m.urgency == Urgency::None)
        {
            return false;
        }
        self.terms.iter().all(|t| match t {
            Term::Has(w) => m.haystack.contains(w.as_str()),
            Term::Not(w) => !m.haystack.contains(w.as_str()),
            Term::Field(f, v) => {
                field_text(m, f).is_some_and(|x| x.to_lowercase().contains(v.as_str()))
            }
        })
    }
}

fn field_text(m: &Message, f: &FieldRef) -> Option<String> {
    use sdrkit_svc_events::Standard;
    match f {
        FieldRef::Standard(Standard::Kind) => Some(m.kind_key.to_string()),
        FieldRef::Standard(Standard::Identity) => m.identity.clone(),
        FieldRef::Standard(Standard::Text) => m.text.clone(),
        FieldRef::Standard(_) => None,
        FieldRef::Declared(ids) => {
            let id = (*ids.get(usize::from(m.kind.0))?)?;
            m.fields.text(id).map(|t| t.to_string())
        }
    }
}

fn terms(kinds: &Kinds, text: &str, within: Option<&KindSet>) -> Result<Vec<Term>, NameError> {
    let mut out = Vec::new();
    let mut at = text.len() - text.trim_start().len();
    let mut rest = text.trim();
    while !rest.is_empty() {
        let start = at;
        let (neg, r) = match rest.strip_prefix('-') {
            Some(r) => (true, r),
            None => (false, rest),
        };
        let (word, tail) = if let Some(q) = r.strip_prefix('"') {
            match q.find('"') {
                Some(end) => (&q[..end], &q[end + 1..]),
                None => (q, ""),
            }
        } else {
            match r.find(char::is_whitespace) {
                Some(end) => (&r[..end], &r[end..]),
                None => (r, ""),
            }
        };
        let w = word.to_lowercase();
        if !w.is_empty() {
            out.push(match (neg, w.split_once(':')) {
                (false, Some((k, v))) if !k.is_empty() && !v.is_empty() && !k.contains(' ') => {
                    let name_at = start + usize::from(r.starts_with('"'));
                    Term::Field(kinds.field(k, name_at, within)?, v.into())
                }
                (true, _) => Term::Not(w),
                _ => Term::Has(w),
            });
        }
        let trimmed = tail.trim_start();
        at += rest.len() - trimmed.len();
        rest = trimmed;
    }
    Ok(out)
}

struct Journal {
    path: StorePath,
    out: Box<dyn StoreWriter>,
    lines: usize,
}

/// The message store: the owner `service.store`.
pub struct Store {
    kinds: Arc<Kinds>,
    storage: Arc<dyn Storage>,
    messages: VecDeque<Message>,
    latest: HashMap<EventKindId, HashMap<String, u64>>,
    next_id: u64,
    bytes: u64,
    folded: u64,
    pruned: u64,
    bad_lines: u64,
    values: Values,
    journal: Option<Journal>,
    shown: Vec<u64>,
    p: Published,
}

impl Store {
    /// A store kept in memory only. Exports go to `storage`.
    pub fn in_memory(kinds: Arc<Kinds>, storage: Arc<dyn Storage>) -> Self {
        Store {
            kinds,
            storage,
            messages: VecDeque::new(),
            latest: HashMap::new(),
            next_id: 1,
            bytes: 0,
            folded: 0,
            pruned: 0,
            bad_lines: 0,
            values: Values::defaults(&StoreSetting::schema()),
            journal: None,
            shown: Vec::new(),
            p: Published::new(STATUS).with_tables(&[&MESSAGES]),
        }
    }

    /// Opens (or makes) a store kept in the journal at `path` of `storage`. Lines that cannot be read are skipped and
    /// counted.
    ///
    /// # Errors
    ///
    /// [`StoreError::Storage`] if the journal cannot be read or opened.
    pub fn open(
        kinds: Arc<Kinds>,
        storage: Arc<dyn Storage>,
        path: StorePath,
    ) -> Result<Self, StoreError> {
        let mut s = Store::in_memory(kinds, Arc::clone(&storage));
        let mut lines = 0;
        let data = match storage.read(&path) {
            Ok(d) => d,
            Err(sdrkit_core::StorageError::NotFound) => Vec::new(),
            Err(source) => return Err(StoreError::Storage { path, source }),
        };
        let mut by_id: BTreeMap<u64, Message> = BTreeMap::new();
        for line in String::from_utf8_lossy(&data).lines() {
            if line.trim().is_empty() {
                continue;
            }
            lines += 1;
            match Message::from_line(line, &s.kinds) {
                Some(m) => {
                    by_id.insert(m.id, m);
                }
                None => s.bad_lines += 1,
            }
        }
        for (_, m) in by_id {
            s.next_id = s.next_id.max(m.id + 1);
            s.push(m);
        }
        let out = storage
            .open_writer(&path, WriteMode::Append)
            .map_err(|source| StoreError::Storage {
                path: path.clone(),
                source,
            })?;
        s.journal = Some(Journal { path, out, lines });
        let now = s.messages.back().map_or(0.0, |m| m.last_time);
        s.prune(now);
        s.publish();
        Ok(s)
    }

    fn push(&mut self, m: Message) {
        if let Some(id) = &m.identity {
            self.latest
                .entry(m.kind)
                .or_default()
                .insert(id.clone(), m.id);
        }
        self.bytes += m.bytes as u64;
        self.messages.push_back(m);
    }

    fn index_of(&self, id: u64) -> Option<usize> {
        self.messages.binary_search_by_key(&id, |m| m.id).ok()
    }

    fn fold_s(&self) -> f64 {
        self.values.get(&FOLD)
    }

    /// Adds an observation. The throttle can fold it into an earlier message.
    ///
    /// # Errors
    ///
    /// [`StoreError::Storage`] if the journal cannot be written. The message is kept in memory.
    pub fn insert(&mut self, o: &Observation<'_>) -> Result<Added, StoreError> {
        let key = match self.kinds.catalog().def(o.kind) {
            Some(d) => d.kind.clone(),
            None => Cow::Owned(o.event.kind().to_owned()),
        };
        let mut m = Message::new(o, key);
        let fold_s = self.fold_s();
        if fold_s > 0.0 {
            if let Some(idx) = m
                .identity
                .as_ref()
                .and_then(|i| self.latest.get(&m.kind)?.get(i.as_str()))
                .and_then(|id| self.index_of(*id))
            {
                let old = &mut self.messages[idx];
                if old.text == m.text && m.time - old.last_time < fold_s && m.time >= old.last_time
                {
                    self.bytes -= old.bytes as u64;
                    old.last_time = m.time;
                    old.repeats += 1;
                    old.fields = m.fields;
                    old.position = m.position.or(old.position);
                    old.distance_km = m.distance_km.or(old.distance_km);
                    if m.urgency != Urgency::None {
                        old.urgency = m.urgency;
                    }
                    let line = old.refresh();
                    self.bytes += old.bytes as u64;
                    let id = old.id;
                    self.folded += 1;
                    self.publish();
                    self.write(line)?;
                    return Ok(Added::Folded(id));
                }
            }
        }
        m.id = self.next_id;
        self.next_id += 1;
        let id = m.id;
        let line = m.refresh();
        self.push(m);
        self.prune(o.unix_time);
        self.publish();
        self.write(line)?;
        self.compact_if_needed()?;
        Ok(Added::New(id))
    }

    fn write(&mut self, line: Result<String, serde_json::Error>) -> Result<(), StoreError> {
        let line = line?;
        if let Some(j) = &mut self.journal {
            let r = j
                .out
                .write_all(line.as_bytes())
                .and_then(|()| j.out.write_all(b"\n"))
                .and_then(|()| j.out.flush());
            r.map_err(|e| StoreError::Storage {
                path: j.path.clone(),
                source: e.into(),
            })?;
            j.lines += 1;
        }
        Ok(())
    }

    /// Drops the oldest messages past a limit of the retention. `now` is the wall clock, Unix seconds.
    pub fn prune(&mut self, now: f64) {
        let max_rows = usize::try_from(self.values.get(&MAX_ROWS)).unwrap_or(usize::MAX);
        let max_bytes = u64::try_from(self.values.get(&MAX_BYTES)).unwrap_or(u64::MAX);
        let age = self.values.get(&MAX_AGE);
        let oldest = (age > 0.0).then_some(now - age);
        while let Some(front) = self.messages.front() {
            let over = self.messages.len() > max_rows
                || self.bytes > max_bytes
                || oldest.is_some_and(|o| front.last_time < o);
            if !over {
                break;
            }
            if let Some(m) = self.messages.pop_front() {
                self.forget(&m);
                self.pruned += 1;
            }
        }
    }

    fn forget(&mut self, m: &Message) {
        self.bytes -= m.bytes as u64;
        if let (Some(i), Some(by_identity)) = (m.identity.as_deref(), self.latest.get_mut(&m.kind))
        {
            if by_identity.get(i) == Some(&m.id) {
                by_identity.remove(i);
            }
        }
    }

    fn compact_if_needed(&mut self) -> Result<(), StoreError> {
        let rows = self.messages.len();
        if self
            .journal
            .as_ref()
            .is_some_and(|j| j.lines > rows * 2 + 1000)
        {
            self.compact()?;
        }
        Ok(())
    }

    /// Rewrites the journal with only the messages kept, as one atomic replace.
    ///
    /// # Errors
    ///
    /// [`StoreError::Storage`] if the journal cannot be written.
    pub fn compact(&mut self) -> Result<(), StoreError> {
        let (Some(j), storage) = (&mut self.journal, &self.storage) else {
            return Ok(());
        };
        let mut data = Vec::new();
        for m in &self.messages {
            data.extend_from_slice(m.line()?.as_bytes());
            data.push(b'\n');
        }
        let err = |source| StoreError::Storage {
            path: j.path.clone(),
            source,
        };
        storage.write(&j.path, &data).map_err(err)?;
        j.out = storage
            .open_writer(&j.path, WriteMode::Append)
            .map_err(|source| StoreError::Storage {
                path: j.path.clone(),
                source,
            })?;
        j.lines = self.messages.len();
        Ok(())
    }

    /// The messages that match `q`, newest first.
    pub fn search(&self, q: &Query) -> Vec<&Message> {
        let mut hits: Vec<&Message> = self.messages.iter().filter(|m| q.matches(m)).collect();
        hits.sort_by(|a, b| b.last_time.total_cmp(&a.last_time).then(b.id.cmp(&a.id)));
        hits.into_iter().skip(q.offset).take(q.limit).collect()
    }

    /// The message with id `id`.
    pub fn get(&self, id: u64) -> Option<&Message> {
        self.index_of(id).map(|i| &self.messages[i])
    }

    /// Every message, oldest first.
    pub fn iter(&self) -> impl Iterator<Item = &Message> {
        self.messages.iter()
    }

    /// The number of messages kept.
    pub fn len(&self) -> usize {
        self.messages.len()
    }

    /// True if the store keeps no message.
    pub fn is_empty(&self) -> bool {
        self.messages.is_empty()
    }

    fn delete(&mut self, id: u64) -> Result<bool, StoreError> {
        let Some(i) = self.index_of(id) else {
            return Ok(false);
        };
        if let Some(m) = self.messages.remove(i) {
            self.forget(&m);
        }
        self.compact()?;
        Ok(true)
    }

    fn clear(&mut self) -> Result<(), StoreError> {
        self.messages.clear();
        self.latest.clear();
        self.bytes = 0;
        self.compact()
    }

    fn table_id(&self) -> Option<TableId> {
        self.p.results().decl().table_id(MESSAGES.key)
    }

    fn show(&mut self, ids: Vec<u64>) {
        let gone: Vec<u64> = self
            .shown
            .iter()
            .filter(|id| !ids.contains(id))
            .copied()
            .collect();
        if let Some(mut t) = self
            .table_id()
            .and_then(|t| self.p.results_mut().table_mut(t))
        {
            for id in gone {
                t.remove(&id.to_string());
            }
        }
        for id in ids.iter().rev() {
            let Some(m) = self.index_of(*id).map(|i| &self.messages[i]) else {
                continue;
            };
            let Some(mut row) = self.p.row_mut(&MESSAGES, &id.to_string()) else {
                continue;
            };
            row.set(&FIRST, m.time);
            row.set(&LAST, m.last_time);
            row.set(&KIND_COL, &*m.kind_key);
            row.set(&keys::FAMILY, m.family);
            row.set(&keys::URGENCY, m.urgency);
            row.opt(&keys::SOURCE, m.identity.as_deref());
            row.set(&keys::FREQ_HZ, m.freq_hz);
            row.opt(&keys::POSITION, m.position.map(|p| (p.lat_deg, p.lon_deg)));
            row.opt(&DISTANCE_KM, m.distance_km);
            row.opt(&keys::TEXT, m.text.as_deref());
            row.set(&REPEATS, i64::from(m.repeats));
        }
        self.shown = ids;
    }

    fn publish(&mut self) {
        let n = |x: u64| i64::try_from(x).unwrap_or(i64::MAX);
        self.p.set(&ROWS, n(self.messages.len() as u64));
        self.p.set(&BYTES, n(self.bytes));
        self.p.set(&FOLDED, n(self.folded));
        self.p.set(&PRUNED, n(self.pruned));
        self.p.set(&BAD_LINES, n(self.bad_lines));
    }

    fn row_id(&self, args: &Values) -> Result<u64, CommandError> {
        let row = args.arg(&ROW)?;
        row.trim()
            .parse::<u64>()
            .ok()
            .filter(|id| self.index_of(*id).is_some())
            .ok_or_else(|| CommandError::NoSuchRow {
                table: self.table_id().unwrap_or(TableId(u16::MAX)),
                key: row.to_owned(),
            })
    }
}

impl Owner for Store {
    fn schema(&self) -> &Arc<Schema> {
        self.values.schema()
    }
    fn values(&self) -> Option<&Values> {
        Some(&self.values)
    }
    fn apply(&mut self, id: SettingId, value: &Value) -> Result<(), SettingError> {
        let s = StoreSetting::from_id(id).ok_or(SettingError::NoSuchId { id })?;
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
        StoreCommand::commands()
    }
    fn invoke(&mut self, id: CommandId, args: &Values) -> Result<Option<Record>, CommandError> {
        match StoreCommand::from_id(id).ok_or(CommandError::NoSuchId { id })? {
            StoreCommand::Search => {
                let q = Query::new(&self.kinds, args).map_err(failed)?;
                let all = self.messages.iter().filter(|m| q.matches(m)).count();
                let ids = self.search(&q).iter().map(|m| m.id).collect();
                self.p.set(&MATCHES, i64::try_from(all).unwrap_or(i64::MAX));
                self.show(ids);
            }
            StoreCommand::Delete => {
                let row = self.row_id(args)?;
                self.delete(row).map_err(failed)?;
                let ids = self.shown.iter().copied().filter(|i| *i != row).collect();
                self.show(ids);
                self.publish();
            }
            StoreCommand::Clear => {
                self.clear().map_err(failed)?;
                self.show(Vec::new());
                self.publish();
            }
            StoreCommand::Compact => self.compact().map_err(failed)?,
            StoreCommand::Export => {
                let format = args.get(&FORMAT);
                let path = path_arg(args, &PATH)?;
                let shown: Vec<&Message> =
                    self.shown.iter().filter_map(|id| self.get(*id)).collect();
                let written = shown.len();
                let data = export(shown, format);
                self.storage
                    .write(&path, data.as_bytes())
                    .map_err(|source| failed(StoreError::Storage { path, source }))?;
                let mut reply = reply(StoreCommand::commands(), id)?;
                reply.set(&WRITTEN, i64::try_from(written).unwrap_or(i64::MAX));
                return Ok(Some(reply));
            }
        }
        Ok(None)
    }
}
