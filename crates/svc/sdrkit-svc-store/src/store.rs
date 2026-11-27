//! The message store: every observation as a message, searchable, newest first, with a throttle for repeated reports,
//! retention by rows, size and age, and a journal of one JSON line per message in the host's storage.

use std::borrow::Cow;
use std::collections::{HashMap, VecDeque};
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
use crate::journal::{self, Disk, Files, Loading};
use crate::message::Message;

/// The owner key of the store.
pub const KEY: OwnerKey = OwnerKey::of("service.store");

/// The most messages kept.
pub const MAX_ROWS: Setting<field::I64> =
    Setting::new("max_rows", "Most messages", "", 100_000).range(1.0, 1e9, 1.0);
/// The most bytes kept: of the messages in memory, and of the journal files on disk. The journal rotates when it passes
/// half of it, so the two journal files hold at most this many bytes.
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
/// Every message of the files is read. Until then a search shows the newest messages only, and runs again when the
/// read ends.
pub const LOADED: Field<field::Bool> = Field::new("loaded", "Every message read", "");

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
    marks: sdrkit_core::RowMarks::NONE,
};

const STATUS: &[&dyn Declare] = &[
    &ROWS, &BYTES, &FOLDED, &PRUNED, &BAD_LINES, &MATCHES, &LOADED,
];

/// What an insert did.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Added {
    /// A new message, with its id.
    New(u64),
    /// Folded into this earlier message by the throttle.
    Folded(u64),
    /// Kept until the full load gives the store its ids: the message is written then.
    Waiting,
}

impl Added {
    /// The message's id. `None` for [`Added::Waiting`].
    pub fn id(self) -> Option<u64> {
        match self {
            Added::New(id) | Added::Folded(id) => Some(id),
            Added::Waiting => None,
        }
    }
}

/// Opens `path` of `storage` to append to it.
fn open_append(
    storage: &dyn Storage,
    path: &StorePath,
) -> Result<Box<dyn StoreWriter>, StoreError> {
    storage
        .open_writer(path, WriteMode::Append)
        .map_err(|source| StoreError::Storage {
            path: path.clone(),
            source,
        })
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
    files: Files,
    /// `None` only while a rotation or a compaction replaces the file.
    out: Option<Box<dyn StoreWriter>>,
    disk: Disk,
    /// `disk.len` when the side index was last written.
    indexed_len: u64,
}

/// The message store: the owner `service.store`.
///
/// An open reads the side index and the newest lines of the journal only, so it takes the same time for any size of
/// the journal. Every message of the files is read by a full load on a thread of its own, which starts at the first
/// search, delete or compaction; until it ends, a search shows the newest messages, and it runs again when the load
/// ends. A rotation bounds the files: when the journal passes half of [`MAX_BYTES`], it becomes the older journal and
/// the older one before it is deleted.
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
    /// Every message of the files is in `messages`.
    loaded: bool,
    /// `next_id` is exact, so new messages can be written. False after an open without a fitting side index, until
    /// the full load ends.
    ids_known: bool,
    loading: Option<Loading>,
    /// Counts the clears: a load that a clear overtook is dropped.
    generation: u64,
    /// New messages that wait for `ids_known`, oldest first.
    waiting: Vec<Message>,
    /// Messages deleted before the full load ended.
    deleted: Vec<u64>,
    compact_after_load: bool,
    search_after_load: Option<Values>,
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
            loaded: true,
            ids_known: true,
            loading: None,
            generation: 0,
            waiting: Vec::new(),
            deleted: Vec::new(),
            compact_after_load: false,
            search_after_load: None,
        }
    }

    /// Opens (or makes) a store kept in the journal at `path` of `storage`, with the older journal `<path>.1` and the
    /// side index `<path>.idx`. It reads the side index and at most a few hundred KiB of the end of the journal. Lines
    /// that cannot be read are skipped and counted; a last line that was cut off stays a line of its own.
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
        let files = Files::of(&path)?;
        let opened = journal::open(&*storage, &files, &s.kinds)?;
        s.bad_lines = opened.bad_lines;
        s.next_id = opened.disk.next_id;
        for (_, m) in opened.tail {
            s.push(m);
        }
        s.loaded = opened.whole;
        s.ids_known = opened.indexed;
        let mut out = open_append(&*storage, &files.current)?;
        let mut disk = opened.disk;
        if opened.torn {
            out.write_all(b"\n")
                .and_then(|()| out.flush())
                .map_err(|e| StoreError::Storage {
                    path: files.current.clone(),
                    source: e.into(),
                })?;
            disk.len += 1;
        }
        s.journal = Some(Journal {
            files,
            out: Some(out),
            disk,
            indexed_len: disk.len,
        });
        let now = s.messages.back().map_or(0.0, |m| m.last_time);
        s.drop_old(now);
        if !s.ids_known {
            s.start_load();
        }
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
        self.poll_load(false);
        let key = match self.kinds.catalog().def(o.kind) {
            Some(d) => d.kind.clone(),
            None => Cow::Owned(o.event.kind().to_owned()),
        };
        let m = Message::new(o, key);
        if !self.ids_known {
            self.waiting.push(m);
            self.publish();
            return Ok(Added::Waiting);
        }
        self.add(m, o.unix_time)
    }

    fn add(&mut self, mut m: Message, now: f64) -> Result<Added, StoreError> {
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
                    self.write(line, false)?;
                    return Ok(Added::Folded(id));
                }
            }
        }
        m.id = self.next_id;
        self.next_id += 1;
        let id = m.id;
        let line = m.refresh();
        self.push(m);
        self.drop_old(now);
        self.publish();
        self.write(line, true)?;
        Ok(Added::New(id))
    }

    /// Appends a line to the journal. `row` is true for the first line of a message.
    fn write(
        &mut self,
        line: Result<String, serde_json::Error>,
        row: bool,
    ) -> Result<(), StoreError> {
        let line = line?;
        let Some(j) = &mut self.journal else {
            return Ok(());
        };
        // A rotation or a compaction that failed left no writer: the journal is opened again.
        let out = match &mut j.out {
            Some(out) => out,
            None => j.out.insert(open_append(&*self.storage, &j.files.current)?),
        };
        out.write_all(line.as_bytes())
            .and_then(|()| out.write_all(b"\n"))
            .and_then(|()| out.flush())
            .map_err(|e| StoreError::Storage {
                path: j.files.current.clone(),
                source: e.into(),
            })?;
        j.disk.len += line.len() as u64 + 1;
        if row {
            j.disk.rows += 1;
        }
        let half = u64::try_from(self.values.get(&MAX_BYTES)).unwrap_or(u64::MAX) / 2;
        if j.disk.len > half && self.loading.is_none() {
            return self.rotate();
        }
        if j.disk.len - j.indexed_len >= journal::INDEX_EVERY {
            self.write_index()?;
        }
        Ok(())
    }

    /// Writes the side index.
    fn write_index(&mut self) -> Result<(), StoreError> {
        let next_id = self.next_id;
        let Some(j) = &mut self.journal else {
            return Ok(());
        };
        if !self.ids_known {
            return Ok(());
        }
        j.disk.next_id = next_id;
        j.disk.write(&*self.storage, &j.files)?;
        j.indexed_len = j.disk.len;
        Ok(())
    }

    /// Makes the journal the older journal, deleting the older one before it, and starts an empty journal.
    fn rotate(&mut self) -> Result<(), StoreError> {
        let Some(j) = &mut self.journal else {
            return Ok(());
        };
        let storage = &*self.storage;
        let err = |source| StoreError::Storage {
            path: j.files.current.clone(),
            source,
        };
        j.out = None;
        storage
            .rename(&j.files.current, &j.files.older)
            .map_err(err)?;
        j.out = Some(open_append(storage, &j.files.current)?);
        j.disk.old_rows = j.disk.rows;
        j.disk.old_bytes = j.disk.len;
        j.disk.rows = 0;
        j.disk.len = 0;
        self.write_index()
    }

    /// Takes the result of a full load that has ended (`wait` waits for it) and merges it with the messages in memory.
    /// A failed load is reported as a problem of the store.
    fn poll_load(&mut self, wait: bool) {
        let Some(l) = self.loading.as_mut() else {
            return;
        };
        let Some(r) = l.take(wait) else {
            return;
        };
        let rows_before = l.rows_before;
        self.loading = None;
        let r = r.and_then(|loaded| self.merge(loaded, rows_before));
        if let Err(e) = r {
            if !self.ids_known {
                // The ids stay as the tail gave them.
                self.ids_known = true;
                let _ = self.flush_waiting();
            }
            self.p.results_mut().problem(&e.to_string());
            self.publish();
        }
    }

    fn merge(&mut self, l: journal::Loaded, rows_before: u64) -> Result<(), StoreError> {
        if l.generation != self.generation {
            return Ok(());
        }
        let mut all = l.messages;
        // A message in memory is as new as the files or newer.
        for m in self.messages.drain(..) {
            all.insert(m.id, m);
        }
        for id in self.deleted.drain(..) {
            all.remove(&id);
        }
        self.latest.clear();
        self.bytes = 0;
        self.bad_lines += l.bad_lines;
        let top = all.keys().next_back().copied().unwrap_or(0);
        for (_, m) in all {
            self.push(m);
        }
        self.next_id = self.next_id.max(top + 1);
        self.loaded = true;
        self.ids_known = true;
        if let Some(j) = &mut self.journal {
            j.disk.old_rows = l.old_rows;
            j.disk.rows = l.rows + j.disk.rows.saturating_sub(rows_before);
        }
        let now = self.messages.back().map_or(0.0, |m| m.last_time);
        self.drop_old(now);
        // Files past the limit (written with a higher limit) are cut to the messages kept.
        let max_bytes = u64::try_from(self.values.get(&MAX_BYTES)).unwrap_or(u64::MAX);
        let over = self
            .journal
            .as_ref()
            .is_some_and(|j| j.disk.len + j.disk.old_bytes > max_bytes);
        if std::mem::take(&mut self.compact_after_load) || over {
            self.compact()?;
        }
        self.flush_waiting()?;
        if let Some(args) = self.search_after_load.take() {
            if let Ok(q) = Query::new(&self.kinds, &args) {
                self.run_search(&q);
            }
        }
        self.write_index()?;
        self.publish();
        Ok(())
    }

    /// Writes the messages that waited for their ids.
    fn flush_waiting(&mut self) -> Result<(), StoreError> {
        for m in std::mem::take(&mut self.waiting) {
            let now = m.time;
            self.add(m, now)?;
        }
        Ok(())
    }

    /// Starts the full load, if the store has not loaded every message and no load runs.
    fn start_load(&mut self) {
        if self.loaded || self.loading.is_some() {
            return;
        }
        let Some(j) = &self.journal else {
            return;
        };
        self.loading = Some(Loading::start(
            Arc::clone(&self.storage),
            j.files.clone(),
            Arc::clone(&self.kinds),
            j.disk.len,
            self.generation,
            j.disk.rows,
        ));
    }

    /// True when every message of the files is in memory.
    pub fn is_loaded(&self) -> bool {
        self.loaded
    }

    /// Starts the full load if it has not run, and waits for it to end.
    pub fn wait_for_load(&mut self) {
        self.start_load();
        self.poll_load(true);
    }

    /// Takes the result of a full load that has ended, then drops the oldest messages past a limit of the retention.
    /// `now` is the wall clock, Unix seconds.
    pub fn prune(&mut self, now: f64) {
        self.poll_load(false);
        self.drop_old(now);
    }

    fn drop_old(&mut self, now: f64) {
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

    /// Rewrites the journal files with only the messages kept, each file as one atomic replace: past half of
    /// [`MAX_BYTES`], the oldest half of the limit in the older journal and the newer messages in the journal. Before the full load has ended, it
    /// starts the load and compacts when it ends.
    ///
    /// # Errors
    ///
    /// [`StoreError::Storage`] if a file cannot be written.
    pub fn compact(&mut self) -> Result<(), StoreError> {
        if !self.loaded {
            self.compact_after_load = true;
            self.start_load();
            return Ok(());
        }
        // Past half of the limit, the oldest half goes to the older journal, so the journal rotates as late as it can.
        let half = u64::try_from(self.values.get(&MAX_BYTES)).unwrap_or(u64::MAX) / 2;
        let mut split = 0;
        if self.bytes > half {
            let mut oldest = 0;
            for m in &self.messages {
                oldest += m.bytes as u64;
                if oldest > half {
                    break;
                }
                split += 1;
            }
        }
        let mut older = Vec::new();
        let mut current = Vec::new();
        for (i, m) in self.messages.iter().enumerate() {
            let out = if i < split { &mut older } else { &mut current };
            out.extend_from_slice(m.line()?.as_bytes());
            out.push(b'\n');
        }
        let rows = (self.messages.len() - split) as u64;
        self.replace_files(&older, split as u64, &current, rows)
    }

    /// Replaces the older journal with `older` (`old_rows` messages; deleted if empty) and the journal with `current`
    /// (`rows` messages).
    fn replace_files(
        &mut self,
        older: &[u8],
        old_rows: u64,
        current: &[u8],
        rows: u64,
    ) -> Result<(), StoreError> {
        let Some(j) = &mut self.journal else {
            return Ok(());
        };
        let storage = &*self.storage;
        let err = |path: &StorePath, source| StoreError::Storage {
            path: path.clone(),
            source,
        };
        j.out = None;
        if older.is_empty() {
            match storage.remove(&j.files.older) {
                Ok(()) | Err(sdrkit_core::StorageError::NotFound) => {}
                Err(e) => return Err(err(&j.files.older, e)),
            }
        } else {
            storage
                .write(&j.files.older, older)
                .map_err(|e| err(&j.files.older, e))?;
        }
        storage
            .write(&j.files.current, current)
            .map_err(|e| err(&j.files.current, e))?;
        j.out = Some(open_append(storage, &j.files.current)?);
        j.disk.len = current.len() as u64;
        j.disk.rows = rows;
        j.disk.old_rows = old_rows;
        j.disk.old_bytes = older.len() as u64;
        self.write_index()
    }

    /// The messages in memory that match `q`, newest first. Before the full load has ended these are the newest
    /// messages only ([`Store::is_loaded`]).
    pub fn search(&self, q: &Query) -> Vec<&Message> {
        let mut hits: Vec<&Message> = self.messages.iter().filter(|m| q.matches(m)).collect();
        hits.sort_by(|a, b| b.last_time.total_cmp(&a.last_time).then(b.id.cmp(&a.id)));
        hits.into_iter().skip(q.offset).take(q.limit).collect()
    }

    fn run_search(&mut self, q: &Query) {
        let all = self.messages.iter().filter(|m| q.matches(m)).count();
        let ids = self.search(q).iter().map(|m| m.id).collect();
        self.p.set(&MATCHES, i64::try_from(all).unwrap_or(i64::MAX));
        self.show(ids);
    }

    /// The message with id `id`, if it is in memory.
    pub fn get(&self, id: u64) -> Option<&Message> {
        self.index_of(id).map(|i| &self.messages[i])
    }

    /// Every message in memory, oldest first.
    pub fn iter(&self) -> impl Iterator<Item = &Message> {
        self.messages.iter()
    }

    /// The number of messages in memory.
    pub fn len(&self) -> usize {
        self.messages.len()
    }

    /// True if the store keeps no message in memory.
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
        if !self.loaded {
            self.deleted.push(id);
        }
        self.compact()?;
        Ok(true)
    }

    fn clear(&mut self) -> Result<(), StoreError> {
        self.messages.clear();
        self.latest.clear();
        self.bytes = 0;
        self.deleted.clear();
        self.compact_after_load = false;
        self.search_after_load = None;
        self.generation += 1;
        self.loading = None;
        self.loaded = true;
        self.ids_known = true;
        self.replace_files(&[], 0, &[], 0)?;
        self.flush_waiting()
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
        // Before the full load the counts are those of the files.
        let (rows, bytes) = match &self.journal {
            Some(j) if !self.loaded => (
                j.disk.rows + j.disk.old_rows + self.waiting.len() as u64,
                j.disk.len + j.disk.old_bytes,
            ),
            _ => (self.messages.len() as u64, self.bytes),
        };
        self.p.set(&ROWS, n(rows));
        self.p.set(&BYTES, n(bytes));
        self.p.set(&FOLDED, n(self.folded));
        self.p.set(&PRUNED, n(self.pruned));
        self.p.set(&BAD_LINES, n(self.bad_lines));
        self.p.set(&LOADED, self.loaded);
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

impl Drop for Store {
    /// Writes the messages that wait for their ids, which waits for the full load, and the side index.
    fn drop(&mut self) {
        if !self.waiting.is_empty() {
            self.wait_for_load();
        }
        let _ = self.write_index();
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
        let command = StoreCommand::from_id(id).ok_or(CommandError::NoSuchId { id })?;
        self.poll_load(false);
        match command {
            StoreCommand::Search => {
                let q = Query::new(&self.kinds, args).map_err(failed)?;
                self.run_search(&q);
                if !self.loaded {
                    // The search runs again on every message when the load ends.
                    self.search_after_load = Some(args.clone());
                    self.start_load();
                }
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
