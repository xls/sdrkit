//! The frequency database as an owner: a search or a lookup sets the page that the entries table shows, the scopes, tags
//! and settings of those entries are tables that refer to it, and commands change the entries of the user.

use crate::db::{FreqDb, Query, UserState};
use crate::entry::{Category, Entry};
use crate::error::{DbError, LineProblem};
use crate::io;
use sdrkit_core::command::{Command, CommandError, CommandId, CommandSet, Commands};
use sdrkit_core::publish::{field, Field, Published, TableDef};
use sdrkit_core::settings::{Argument, SettingError};
use sdrkit_core::stack::StageError;
use sdrkit_core::{
    HostRequest, Owner, OwnerKey, OwnerSetting, Record, Results, ResultsDecl, Setting, SettingId,
    Storage, StorageError, StorePath, TableId, TuneTarget, Values,
};
use sdrkit_data_plans::{PlanKind, Plans};
use std::collections::HashSet;
use std::sync::Arc;

/// The owner key of the frequency database.
pub const KEY: OwnerKey = OwnerKey::of("service.freqdb");
const OWNER: &str = "service.freqdb";

const USER_FILE: &str = "freqdb/user.json";
const MAX_PAGE: usize = 1000;
const MAX_ITEMS: usize = 100_000;
const MAX_PROBLEMS: usize = 1000;

sdrkit_core::choice_enum! {
    /// The format of an exported file.
    pub enum ExportFormat {
        /// CSV with the columns of the memory store.
        Csv = ("csv", "CSV"),
        /// A JSON array of entries.
        Json = ("json", "JSON"),
    }
}

sdrkit_core::choice_enum! {
    /// The entries that an export writes.
    pub enum Which {
        /// Every entry that the last search or lookup found.
        Found = ("found", "Found"),
        /// The entries of the user.
        User = ("user", "User entries"),
        /// The starred entries.
        Starred = ("starred", "Starred"),
        /// Every entry.
        All = ("all", "All"),
    }
}

// Results.
const TOTAL: Field<field::I64> = Field::new("total", "Matching entries", "");
const POSITION: Field<field::I64> = Field::new("position", "Position", "");
const NAME: Field<field::Text> = Field::new("name", "Name", "");
const FREQ: Field<field::Hz> = Field::new("freq_hz", "Frequency", "Hz");
const END: Field<field::Hz> = Field::new("end_hz", "Upper edge", "Hz");
const BANDWIDTH: Field<field::Hz> = Field::new("bandwidth_hz", "Bandwidth", "Hz");
const STACK: Field<field::Text> = Field::new("stack", "Stack", "");
const SUBMODE: Field<field::Text> = Field::new("submode", "Demodulator", "");
const CATEGORY: Field<field::Choice<Category>> = Field::new("category", "Category", "");
const NOTES: Field<field::Text> = Field::new("notes", "Notes", "");
const SOURCE: Field<field::Text> = Field::new("source", "Source", "");
const STARRED: Field<field::Bool> = Field::new("starred", "Starred", "");
const USER: Field<field::Bool> = Field::new("user", "User entry", "");
const DISTANCE: Field<field::Hz> = Field::new("distance_hz", "Distance", "Hz");
const OF_ENTRY: Field<field::Text> = Field::new("entry", "Entry", "").refers_to(OWNER, "entries");
const ITEM_KEY: Field<field::Text> = Field::new("key", "Key", "");
const ITEM_OWNER: Field<field::Text> = Field::new("owner", "Owner", "");
const ITEM_VALUE: Field<field::Any> = Field::new("value", "Value", "");
const LINE: Field<field::I64> = Field::new("line", "Line", "");
const PROBLEM: Field<field::Choice<crate::error::Problem>> = Field::new("kind", "Problem", "");

/// The entries that the last search or lookup found, one page. The row key is the id of the entry.
pub const ENTRIES: TableDef = TableDef {
    key: "entries",
    label: "Entries",
    columns: &[
        &POSITION, &NAME, &FREQ, &END, &BANDWIDTH, &STACK, &SUBMODE, &CATEGORY, &NOTES, &SOURCE,
        &STARRED, &USER, &DISTANCE,
    ],
    cap: MAX_PAGE,
    marks: sdrkit_core::RowMarks::NONE,
};
/// The scope tags of the entries in [`ENTRIES`]: where each applies.
pub const ENTRY_SCOPES: TableDef = TableDef {
    key: "entry_scopes",
    label: "Entry scopes",
    columns: &[&OF_ENTRY, &ITEM_KEY],
    cap: MAX_ITEMS,
    marks: sdrkit_core::RowMarks::NONE,
};
/// The tags of the entries in [`ENTRIES`].
pub const ENTRY_TAGS: TableDef = TableDef {
    key: "entry_tags",
    label: "Entry tags",
    columns: &[&OF_ENTRY, &ITEM_KEY],
    cap: MAX_ITEMS,
    marks: sdrkit_core::RowMarks::NONE,
};
/// The settings of the entries in [`ENTRIES`]: the owner key, the setting key and the typed value.
pub const ENTRY_SETTINGS: TableDef = TableDef {
    key: "entry_settings",
    label: "Entry settings",
    columns: &[&OF_ENTRY, &ITEM_OWNER, &ITEM_KEY, &ITEM_VALUE],
    cap: MAX_ITEMS,
    marks: sdrkit_core::RowMarks::NONE,
};
/// The problems of the last import, one row for each line that was not read.
pub const PROBLEMS: TableDef = TableDef {
    key: "problems",
    label: "Import problems",
    columns: &[&LINE, &PROBLEM],
    cap: MAX_PROBLEMS,
    marks: sdrkit_core::RowMarks::NONE,
};

// Arguments.
/// The row key (the id) of the entry that a command acts on.
pub const ROW: Argument<field::Text> = Argument::new(Field::new("row", "Entry", ""))
    .help("The id of the entry, as the table shows it.");
/// The session that a tune acts on: its row key in the engine's sessions table.
pub const SESSION: Argument<field::Text> =
    Argument::new(Field::new("session", "Session", "").refers_to("engine", "sessions"));
const A_TEXT: Setting<field::Text> = Setting::new("text", "Search", "", "")
    .help("Words that start words of the name, notes, tags, category or stack, or a frequency.");
const A_BY_CATEGORY: Setting<field::Bool> =
    Setting::new("by_category", "Of one category", "", false);
const A_CATEGORY: Setting<field::Choice<Category>> =
    Setting::choice("category", "Category", Category::Other);
const A_TAG: Setting<field::Text> =
    Setting::new("tag", "Tag", "", "").help("Only entries with this tag. Empty for all.");
const A_STACK: Setting<field::Text> = Setting::new("stack", "Stack", "", "");
const A_LO: Setting<field::Hz> = Setting::new("lo_hz", "From", "Hz", 0.0).range(0.0, 1e12, 0.0);
const A_HI: Setting<field::Hz> = Setting::new("hi_hz", "To", "Hz", 1e12).range(0.0, 1e12, 0.0);
const A_SCOPE: Setting<field::Text> = Setting::new("scope", "Scope", "", "")
    .help("Only entries that apply everywhere or in one of these scope tags, apart by spaces. Empty for all.");
const A_STARRED_ONLY: Setting<field::Bool> =
    Setting::new("starred_only", "Starred only", "", false);
const A_USER_ONLY: Setting<field::Bool> = Setting::new("user_only", "User entries only", "", false);
const A_LIMIT: Setting<field::I64> =
    Setting::new("limit", "Rows", "", 100).range(1.0, MAX_PAGE as f64, 1.0);
const A_OFFSET: Setting<field::I64> =
    Setting::new("offset", "First row", "", 0).range(0.0, 1e6, 1.0);
const A_FREQ: Argument<field::Hz> = Argument::new(FREQ);
const A_TOLERANCE: Setting<field::Hz> = Setting::new("tolerance_hz", "Tolerance", "Hz", 1000.0)
    .range(0.0, 1e9, 0.0)
    .help("How far from the frequency an entry can be. Half its bandwidth counts at least.");
const A_NAME: Argument<field::Text> = Argument::new(NAME);
const A_END: Setting<field::Hz> = Setting::new("end_hz", "Upper edge", "Hz", 0.0)
    .range(0.0, 1e12, 0.0)
    .help("0 for a point.");
const A_BANDWIDTH: Setting<field::Hz> = Setting::new("bandwidth_hz", "Bandwidth", "Hz", 0.0)
    .range(0.0, 1e9, 0.0)
    .help("0 for none.");
const A_SUBMODE: Setting<field::Text> = Setting::new("submode", "Demodulator", "", "");
const A_NOTES: Setting<field::Text> = Setting::new("notes", "Notes", "", "");
const A_SOURCE: Setting<field::Text> = Setting::new("source", "Source", "", "");
const A_ON: Setting<field::Bool> = Setting::new("on", "On", "", true);
const A_KEY: Argument<field::Text> = Argument::new(ITEM_KEY);
const A_OWNER: Argument<field::Text> =
    Argument::new(ITEM_OWNER).help("The owner key of the setting (\"stack.general\").");
const A_VALUE: Argument<field::Any> =
    Argument::new(ITEM_VALUE).help("The value, of the kind that the owner's schema gives.");
const A_PATH: Argument<field::Text> = Argument::new(Field::new("path", "File", ""))
    .help("A file in the storage of the host, as a relative path.");
const A_FORMAT: Setting<field::Choice<ExportFormat>> =
    Setting::choice("format", "Format", ExportFormat::Csv);
const A_WHICH: Setting<field::Choice<Which>> = Setting::choice("which", "Which", Which::User);

// Replies.
const R_ROW: Field<field::Text> = Field::new("row", "Entry", "");
const R_ADDED: Field<field::I64> = Field::new("added", "Added", "");
const R_DUPLICATES: Field<field::I64> = Field::new("duplicates", "Duplicates", "");
const R_OVER_CAP: Field<field::I64> = Field::new("over_cap", "Over the cap", "");
const R_PROBLEMS: Field<field::I64> = Field::new("problems", "Problems", "");
const R_WRITTEN: Field<field::I64> = Field::new("written", "Rows written", "");

const ENTRY_ARGS: &[&dyn sdrkit_core::DeclareSetting] = &[
    &A_NAME,
    &A_FREQ,
    &A_END,
    &A_BANDWIDTH,
    &A_STACK,
    &A_SUBMODE,
    &A_CATEGORY,
    &A_NOTES,
    &A_SOURCE,
];
const UPDATE_ARGS: &[&dyn sdrkit_core::DeclareSetting] = &[
    &ROW,
    &A_NAME,
    &A_FREQ,
    &A_END,
    &A_BANDWIDTH,
    &A_STACK,
    &A_SUBMODE,
    &A_CATEGORY,
    &A_NOTES,
    &A_SOURCE,
];

const C_SEARCH: Command = Command::new("search", "Search")
    .help("Sets which entries the entries table shows: starred first, then by name, then nearest, then by frequency.")
    .args(&[
        &A_TEXT,
        &A_BY_CATEGORY,
        &A_CATEGORY,
        &A_TAG,
        &A_STACK,
        &A_LO,
        &A_HI,
        &A_SCOPE,
        &A_STARRED_ONLY,
        &A_USER_ONLY,
        &A_LIMIT,
        &A_OFFSET,
    ]);
const C_NEAR: Command = Command::new("near", "What is here")
    .help(
        "Sets the entries table to the entries on a frequency, points nearest first, then ranges.",
    )
    .args(&[&A_FREQ, &A_TOLERANCE, &A_SCOPE, &A_LIMIT, &A_OFFSET]);
const C_ADD: Command = Command::new("add", "Add entry")
    .args(ENTRY_ARGS)
    .reply(&[&R_ROW]);
const C_UPDATE: Command = Command::new("update", "Change entry")
    .help("Replaces the values of an entry of the user. Its scope, tags and settings stay.")
    .args(UPDATE_ARGS);
const C_REMOVE: Command = Command::new("remove", "Delete entry").args(&[&ROW]);
const C_STAR: Command = Command::new("star", "Star entry").args(&[&ROW, &A_ON]);
const C_TAG_ADD: Command = Command::new("tag.add", "Add tag").args(&[&ROW, &A_KEY]);
const C_TAG_REMOVE: Command = Command::new("tag.remove", "Remove tag").args(&[&ROW, &A_KEY]);
const C_SCOPE_ADD: Command = Command::new("scope.add", "Add scope")
    .help("Adds a scope tag where the entry applies.")
    .args(&[&ROW, &A_KEY]);
const C_SCOPE_REMOVE: Command =
    Command::new("scope.remove", "Remove scope").args(&[&ROW, &A_KEY]);
const C_ITEM_SET: Command = Command::new("item.set", "Set setting")
    .help("Adds or replaces a setting that the entry applies when it is tuned.")
    .args(&[&ROW, &A_OWNER, &A_KEY, &A_VALUE]);
const C_ITEM_REMOVE: Command =
    Command::new("item.remove", "Remove setting").args(&[&ROW, &A_OWNER, &A_KEY]);
const C_TUNE: Command = Command::new("tune", "Tune")
    .help("Asks the engine to tune the session to the entry.")
    .args(&[&ROW, &SESSION]);
const C_IMPORT: Command = Command::new("import", "Import entries")
    .help("Adds the entries of the CSV or JSON file `path` as entries of the user. The problems table lists the lines not read.")
    .args(&[&A_PATH])
    .reply(&[&R_ADDED, &R_DUPLICATES, &R_OVER_CAP, &R_PROBLEMS]);
const C_EXPORT: Command = Command::new("export", "Export entries")
    .help("Writes entries to the file `path`.")
    .args(&[&A_PATH, &A_FORMAT, &A_WHICH])
    .reply(&[&R_WRITTEN]);

sdrkit_core::commands! {
    /// The commands of the frequency database.
    pub enum FreqDbCommand {
        /// Sets the entries table to a search.
        Search = C_SEARCH,
        /// Sets the entries table to what is on a frequency.
        Near = C_NEAR,
        /// Adds an entry of the user.
        Add = C_ADD,
        /// Changes an entry of the user.
        Update = C_UPDATE,
        /// Deletes an entry of the user.
        Remove = C_REMOVE,
        /// Stars an entry.
        Star = C_STAR,
        /// Adds a tag.
        TagAdd = C_TAG_ADD,
        /// Removes a tag.
        TagRemove = C_TAG_REMOVE,
        /// Adds a scope tag.
        ScopeAdd = C_SCOPE_ADD,
        /// Removes a scope tag.
        ScopeRemove = C_SCOPE_REMOVE,
        /// Sets a setting of an entry.
        ItemSet = C_ITEM_SET,
        /// Removes a setting of an entry.
        ItemRemove = C_ITEM_REMOVE,
        /// Asks the engine to tune to an entry.
        Tune = C_TUNE,
        /// Imports entries.
        Import = C_IMPORT,
        /// Exports entries.
        Export = C_EXPORT,
    }
}

/// What the entries table shows.
#[derive(Clone, Debug)]
enum View {
    Search(Query),
    Near {
        hz: f64,
        tolerance_hz: f64,
        scope: Vec<String>,
    },
}

/// The frequency database as an owner, with the entries of the user kept in the host's storage.
pub struct FreqDbService {
    storage: Arc<dyn Storage>,
    db: FreqDb,
    view: View,
    limit: usize,
    offset: usize,
    p: Published,
    entries: TableId,
    requests: Vec<HostRequest>,
}

impl std::fmt::Debug for FreqDbService {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FreqDbService")
            .field("user", &self.db.user_entries().len())
            .finish()
    }
}

fn decl() -> Arc<ResultsDecl> {
    let mut d = ResultsDecl::new().with_fields(&[&TOTAL]);
    for t in [
        &ENTRIES,
        &ENTRY_SCOPES,
        &ENTRY_TAGS,
        &ENTRY_SETTINGS,
        &PROBLEMS,
    ] {
        d = d.with_table(t);
    }
    Arc::new(d)
}

fn user_file() -> StorePath {
    StorePath::new(USER_FILE).expect("the file name of the service is a store path")
}

fn count(n: usize) -> i64 {
    i64::try_from(n).unwrap_or(i64::MAX)
}

fn failed(e: DbError) -> CommandError {
    CommandError::Failed {
        source: StageError::new(e),
    }
}

/// [`CommandError::BadArgument`] for the argument `key`, with the error that `error` makes from its id.
fn refused(
    args: &Values,
    key: &'static str,
    error: impl Fn(SettingId) -> SettingError,
) -> CommandError {
    match args.schema().id(key) {
        Ok(arg) => CommandError::BadArgument {
            arg,
            error: error(arg),
        },
        Err(_) => CommandError::NotAnArgument { key },
    }
}

fn file_arg(args: &Values) -> Result<StorePath, CommandError> {
    StorePath::new(args.arg(&A_PATH)?).map_err(|_| {
        refused(args, A_PATH.field.key, |id| SettingError::Malformed {
            id,
            at: None,
        })
    })
}

/// The tags of the argument `scope`.
fn scope_arg(args: &Values) -> Vec<String> {
    args.get(&A_SCOPE)
        .split_whitespace()
        .map(String::from)
        .collect()
}

fn entry_of(args: &Values) -> Result<Entry, CommandError> {
    let opt = |v: f64| (v > 0.0).then_some(v);
    let mut e = Entry::new(
        "",
        args.arg(&A_NAME)?,
        args.arg(&A_FREQ)?,
        args.get(&A_CATEGORY),
    );
    e.end_hz = opt(args.get(&A_END));
    e.bandwidth_hz = opt(args.get(&A_BANDWIDTH));
    e.stack = args.get(&A_STACK).to_owned();
    e.submode = args.get(&A_SUBMODE).to_owned();
    e.notes = args.get(&A_NOTES).to_owned();
    e.source = args.get(&A_SOURCE).to_owned();
    Ok(e)
}

impl FreqDbService {
    /// Opens the database on the host's `storage` with the built-in entries and the entries of the user. A file that does
    /// not parse is left, and the service starts with no entries of the user.
    ///
    /// # Errors
    ///
    /// [`DbError::Storage`] if the storage fails.
    pub fn open(storage: Arc<dyn Storage>) -> Result<Self, DbError> {
        let path = user_file();
        let mut db = FreqDb::new();
        let mut unread = false;
        match storage.read(&path) {
            Ok(bytes) => match serde_json::from_slice::<UserState>(&bytes) {
                Ok(state) => {
                    db.load_user_state(state);
                }
                Err(_) => unread = true,
            },
            Err(StorageError::NotFound) => {}
            Err(source) => return Err(DbError::Storage { path, source }),
        }
        let p = Published::from_decl(decl());
        let entries = p
            .results()
            .decl()
            .table_id(ENTRIES.key)
            .expect("the service declares its tables");
        let mut s = FreqDbService {
            storage,
            db,
            view: View::Search(Query {
                user_only: true,
                ..Query::default()
            }),
            limit: 100,
            offset: 0,
            p,
            entries,
            requests: Vec::new(),
        };
        if unread {
            s.p.results_mut()
                .log("The file of the user entries did not read. It is left as it is.");
        }
        s.publish();
        Ok(s)
    }

    /// The database, for the engine: what is on a frequency, and the labels of a spectrum.
    pub fn db(&self) -> &FreqDb {
        &self.db
    }

    /// Replaces the supplied entries (see [`FreqDb::set_supplied`]).
    pub fn set_supplied(&mut self, entries: Vec<Entry>) {
        self.db.set_supplied(entries);
        self.publish();
    }

    /// Replaces the plan channels with those of every plan of `kinds`. The host calls it when its plans change.
    pub fn set_plan_channels(&mut self, plans: &Plans, kinds: &[PlanKind]) {
        self.db.set_plan_channels(plans, kinds);
        self.publish();
    }

    /// Takes the requests that the commands made since the last call (a tune), for the engine to carry out in order.
    pub fn take_requests(&mut self) -> Vec<HostRequest> {
        std::mem::take(&mut self.requests)
    }

    fn save(&self) -> Result<(), DbError> {
        let bytes = serde_json::to_vec(&self.db.user_state()).map_err(|_| DbError::Encode)?;
        let path = user_file();
        self.storage
            .write(&path, &bytes)
            .map_err(|source| DbError::Storage { path, source })
    }

    fn publish(&mut self) {
        let found = found(&self.db, &self.view);
        let total = found.len();
        let page: Vec<(&Entry, Option<f64>, bool)> = found
            .into_iter()
            .skip(self.offset)
            .take(self.limit)
            .map(|(e, d)| (e, d, self.db.is_starred(&e.id)))
            .collect();
        let p = &mut self.p;
        p.set(&TOTAL, count(total));
        let mut keep: [HashSet<String>; 4] = Default::default();
        for (i, (e, distance, starred)) in page.iter().enumerate().rev() {
            if let Some(mut r) = p.row_mut(&ENTRIES, &e.id) {
                r.set(&POSITION, count(i));
                r.set(&NAME, &e.name);
                r.set(&FREQ, e.hz);
                r.opt(&END, e.end_hz);
                r.opt(&BANDWIDTH, e.bandwidth_hz);
                r.set(&STACK, &e.stack);
                r.set(&SUBMODE, &e.submode);
                r.set(&CATEGORY, e.category);
                r.set(&NOTES, &e.notes);
                r.set(&SOURCE, &e.source);
                r.set(&STARRED, *starred);
                r.set(&USER, e.is_user());
                r.opt(&DISTANCE, *distance);
            }
            for c in &e.scope {
                let key = format!("{}/{c}", e.id);
                if let Some(mut r) = p.row_mut(&ENTRY_SCOPES, &key) {
                    r.set(&OF_ENTRY, &e.id);
                    r.set(&ITEM_KEY, c);
                }
                keep[1].insert(key);
            }
            for t in &e.tags {
                let key = format!("{}/{t}", e.id);
                if let Some(mut r) = p.row_mut(&ENTRY_TAGS, &key) {
                    r.set(&OF_ENTRY, &e.id);
                    r.set(&ITEM_KEY, t);
                }
                keep[2].insert(key);
            }
            for st in &e.settings {
                let key = format!("{}/{}/{}", e.id, st.owner.as_str(), st.key);
                if let Some(mut r) = p.row_mut(&ENTRY_SETTINGS, &key) {
                    r.set(&OF_ENTRY, &e.id);
                    r.set(&ITEM_OWNER, st.owner.as_str());
                    r.set(&ITEM_KEY, &st.key);
                    r.set(&ITEM_VALUE, &st.value);
                }
                keep[3].insert(key);
            }
            keep[0].insert(e.id.clone());
        }
        let tables = [
            &ENTRIES,
            &ENTRY_SCOPES,
            &ENTRY_TAGS,
            &ENTRY_SETTINGS,
        ];
        for (def, keep) in tables.into_iter().zip(&keep) {
            prune(p, def, keep);
        }
    }

    fn publish_problems(&mut self, problems: &[LineProblem]) {
        self.p.clear_rows(&PROBLEMS);
        for (i, pr) in problems.iter().take(MAX_PROBLEMS).enumerate().rev() {
            if let Some(mut r) = self.p.row_mut(&PROBLEMS, &i.to_string()) {
                r.set(&LINE, count(pr.line));
                r.set(&PROBLEM, pr.kind);
            }
        }
    }

    /// The entry that the argument `row` names.
    fn entry(&self, args: &Values) -> Result<&Entry, CommandError> {
        let row = args.arg(&ROW)?;
        self.db.get(row).ok_or_else(|| CommandError::NoSuchRow {
            table: self.entries,
            key: row.to_owned(),
        })
    }

    /// Changes the entry of the user that the argument `row` names through `f`, saves and publishes.
    fn change(&mut self, args: &Values, f: impl FnOnce(&mut Entry)) -> Result<(), CommandError> {
        let mut e = self.entry(args)?.clone();
        f(&mut e);
        self.db.update_user(e).map_err(failed)?;
        self.save().map_err(failed)
    }

    fn run(&mut self, cmd: FreqDbCommand, args: &Values) -> Result<Option<Record>, CommandError> {
        let mut reply = FreqDbCommand::commands()
            .def(cmd.id())
            .map(|d| d.new_reply())
            .filter(|r| !r.decl().is_empty());
        let mut set = |f: &Field<field::I64>, n: usize| {
            if let Some(r) = reply.as_mut() {
                r.set(f, count(n));
            }
        };
        match cmd {
            FreqDbCommand::Search => {
                let lo = args.get(&A_LO);
                let hi = args.get(&A_HI);
                self.view = View::Search(Query {
                    text: args.get(&A_TEXT).to_owned(),
                    categories: if args.get(&A_BY_CATEGORY) {
                        vec![args.get(&A_CATEGORY)]
                    } else {
                        Vec::new()
                    },
                    tags: match args.get(&A_TAG).trim() {
                        "" => Vec::new(),
                        t => vec![t.to_owned()],
                    },
                    stack: args.get(&A_STACK).to_owned(),
                    lo_hz: (lo > 0.0).then_some(lo),
                    hi_hz: (hi < 1e12).then_some(hi),
                    scope: scope_arg(args),
                    starred_only: args.get(&A_STARRED_ONLY),
                    user_only: args.get(&A_USER_ONLY),
                });
                self.page(args);
            }
            FreqDbCommand::Near => {
                self.view = View::Near {
                    hz: args.arg(&A_FREQ)?,
                    tolerance_hz: args.get(&A_TOLERANCE),
                    scope: scope_arg(args),
                };
                self.page(args);
            }
            FreqDbCommand::Add => {
                let id = self.db.add_user(entry_of(args)?).map_err(failed)?;
                self.save().map_err(failed)?;
                if let Some(r) = reply.as_mut() {
                    r.set(&R_ROW, &id);
                }
            }
            FreqDbCommand::Update => {
                let new = entry_of(args)?;
                self.change(args, |e| {
                    *e = Entry {
                        id: std::mem::take(&mut e.id),
                        tags: std::mem::take(&mut e.tags),
                        scope: std::mem::take(&mut e.scope),
                        settings: std::mem::take(&mut e.settings),
                        ..new
                    };
                })?;
            }
            FreqDbCommand::Remove => {
                let id = self.entry(args)?.id.clone();
                self.db.remove_user(&id).map_err(failed)?;
                self.save().map_err(failed)?;
            }
            FreqDbCommand::Star => {
                let id = self.entry(args)?.id.clone();
                self.db.star(&id, args.get(&A_ON));
                self.save().map_err(failed)?;
            }
            FreqDbCommand::TagAdd | FreqDbCommand::TagRemove => {
                let tag = args.arg(&A_KEY)?.to_owned();
                let add = cmd == FreqDbCommand::TagAdd;
                self.change(args, |e| {
                    e.tags.retain(|t| *t != tag);
                    if add {
                        e.tags.push(tag);
                    }
                })?;
            }
            FreqDbCommand::ScopeAdd | FreqDbCommand::ScopeRemove => {
                let tag = args.arg(&A_KEY)?.trim().to_owned();
                let add = cmd == FreqDbCommand::ScopeAdd;
                if add && (tag.is_empty() || tag.contains(char::is_whitespace)) {
                    return Err(refused(args, A_KEY.field.key, |id| {
                        SettingError::NotPermitted { id }
                    }));
                }
                self.change(args, |e| {
                    e.scope.retain(|c| *c != tag);
                    if add {
                        e.scope.push(tag);
                    }
                })?;
            }
            FreqDbCommand::ItemSet | FreqDbCommand::ItemRemove => {
                let owner = OwnerKey::new(args.arg(&A_OWNER)?.to_owned()).map_err(|_| {
                    refused(args, A_OWNER.field.key, |id| SettingError::Malformed {
                        id,
                        at: None,
                    })
                })?;
                let key = args.arg(&A_KEY)?.to_owned();
                let value = match cmd {
                    FreqDbCommand::ItemSet => Some(args.arg(&A_VALUE)?.clone()),
                    _ => None,
                };
                self.change(args, |e| {
                    e.settings.retain(|s| !(s.owner == owner && s.key == key));
                    if let Some(value) = value {
                        e.settings.push(OwnerSetting { owner, key, value });
                    }
                })?;
            }
            FreqDbCommand::Tune => {
                let session = args.arg(&SESSION)?.to_owned();
                let e = self.entry(args)?;
                let request = HostRequest::Tune {
                    session,
                    freq_hz: Some(e.hz),
                    target: TuneTarget::of_stack(&e.stack),
                    submode: e.submode.clone(),
                    rate_hz: None,
                    bandwidth_hz: e.bandwidth_hz,
                    settings: e.settings.clone(),
                };
                self.requests.push(request);
                return Ok(reply);
            }
            FreqDbCommand::Import => {
                let path = file_arg(args)?;
                let bytes = self
                    .storage
                    .read(&path)
                    .map_err(|source| failed(DbError::Storage { path, source }))?;
                let r = io::import(&mut self.db, &String::from_utf8_lossy(&bytes));
                if !r.added.is_empty() {
                    self.save().map_err(failed)?;
                }
                set(&R_ADDED, r.added.len());
                set(&R_DUPLICATES, r.duplicates);
                set(&R_OVER_CAP, r.over_cap);
                set(&R_PROBLEMS, r.problems.len());
                self.publish_problems(&r.problems);
            }
            FreqDbCommand::Export => {
                let path = file_arg(args)?;
                let list: Vec<&Entry> = match args.get(&A_WHICH) {
                    Which::Found => found(&self.db, &self.view)
                        .into_iter()
                        .map(|(e, _)| e)
                        .collect(),
                    Which::User => self.db.user_entries().iter().collect(),
                    Which::Starred => self
                        .db
                        .entries()
                        .filter(|e| self.db.is_starred(&e.id))
                        .collect(),
                    Which::All => self.db.entries().collect(),
                };
                let text = match args.get(&A_FORMAT) {
                    ExportFormat::Csv => io::to_csv(&self.db, &list),
                    ExportFormat::Json => io::to_json(&self.db, &list),
                };
                let n = list.len();
                self.storage
                    .write(&path, text.as_bytes())
                    .map_err(|source| failed(DbError::Storage { path, source }))?;
                set(&R_WRITTEN, n);
                return Ok(reply);
            }
        }
        self.publish();
        Ok(reply)
    }

    fn page(&mut self, args: &Values) {
        self.limit = usize::try_from(args.get(&A_LIMIT)).unwrap_or(1);
        self.offset = usize::try_from(args.get(&A_OFFSET)).unwrap_or(0);
    }
}

impl Owner for FreqDbService {
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
        FreqDbCommand::commands()
    }
    fn invoke(&mut self, id: CommandId, args: &Values) -> Result<Option<Record>, CommandError> {
        let cmd = FreqDbCommand::from_id(id).ok_or(CommandError::NoSuchId { id })?;
        self.run(cmd, args)
    }
}

/// The entries that `view` takes, in order, each with its distance for a lookup.
fn found<'a>(db: &'a FreqDb, view: &View) -> Vec<(&'a Entry, Option<f64>)> {
    match view {
        View::Search(q) => db
            .search(q)
            .into_iter()
            .map(|h| (h.entry, h.distance_hz))
            .collect(),
        View::Near {
            hz,
            tolerance_hz,
            scope,
        } => db
            .near(*hz, *tolerance_hz, scope)
            .into_iter()
            .map(|e| (e, Some((e.hz - hz).abs())))
            .collect(),
    }
}

/// Removes the rows of `def` whose key is not in `keep`.
fn prune(p: &mut Published, def: &TableDef, keep: &HashSet<String>) {
    let Some(id) = p.results().decl().table_id(def.key) else {
        return;
    };
    let stale: Vec<String> = p
        .results()
        .table(id)
        .map(|t| {
            t.rows_newest_first()
                .filter(|(k, _)| !keep.contains(*k))
                .map(|(k, _)| k.to_owned())
                .collect()
        })
        .unwrap_or_default();
    if let Some(mut t) = p.results_mut().table_mut(id) {
        for k in &stale {
            t.remove(k);
        }
    }
}
