//! The memory service as an owner: memories, bookmarks and presets as tables, and the commands that change them.

use crate::bookmark::{Bookmark, BookmarkStore};
use crate::error::{LineProblem, MemoryError, Problem};
use crate::exchange::{self, Format};
use crate::files::{load_json, load_memories, save_json, save_memories};
use crate::memory::{put_setting, remove_setting, Memory};
use crate::preset::{Preset, PresetStore};
use crate::store::{MemoryFilter, MemoryStore, OnSame, SortKey, Stored};
use sdrkit_core::command::{Command, CommandError, CommandId, CommandSet, Commands};
use sdrkit_core::publish::{field, Declare, Field, Published, TableDef};
use sdrkit_core::settings::{Argument, SettingError, SettingSet};
use sdrkit_core::stack::StageError;
use sdrkit_core::TuneTarget;
use sdrkit_core::{
    Clock, HostRequest, Owner, OwnerKey, OwnerSetting, Record, Results, ResultsDecl, Schema,
    Setting, SettingId, Storage, StorePath, TableId, Value, Values,
};
use std::collections::HashSet;
use std::sync::Arc;

/// The owner key of the memory service.
pub const KEY: OwnerKey = OwnerKey::of("service.memory");
const OWNER: &str = "service.memory";

/// The most memories the service keeps.
pub const MEMORY_CAP: usize = 10_000;

const MEMORIES_FILE: &str = "memory/memories.json";
const BOOKMARKS_FILE: &str = "memory/bookmarks.json";
const PRESETS_FILE: &str = "memory/presets.json";

sdrkit_core::choice_enum! {
    /// Which memories the memory table shows.
    pub enum FilterKind {
        /// Every memory.
        All = ("all", "All"),
        /// The memories marked "include in scan".
        InScan = ("in_scan", "In scan"),
        /// One group, named by the argument `group`.
        Group = ("group", "Group"),
        /// The frequencies from `lo_hz` to `hi_hz`.
        Range = ("range", "Range"),
        /// The starred memories.
        Starred = ("starred", "Starred"),
    }
}

sdrkit_core::choice_enum! {
    /// A list of stored settings of a memory or a preset.
    pub enum Part {
        /// A setting of the stack of a memory.
        MemorySetting = ("memory_setting", "Memory setting"),
        /// A gain of the radio of a memory.
        MemoryGain = ("memory_gain", "Memory gain"),
        /// A setting of the stack of a preset.
        PresetSetting = ("preset_setting", "Preset setting"),
        /// A gain of the radio of a preset.
        PresetGain = ("preset_gain", "Preset gain"),
        /// An audio setting of a preset, for the host.
        PresetAudio = ("preset_audio", "Preset audio"),
    }
}

sdrkit_core::choice_enum! {
    /// What to do in "store current" if a memory of the same channel exists.
    pub enum OnSameChoice {
        /// Change nothing and give the existing memory.
        Ask = ("ask", "Ask"),
        /// Update the existing memory.
        Update = ("update", "Update"),
        /// Add a new memory beside it.
        Add = ("add", "Add"),
    }
}

sdrkit_core::choice_enum! {
    /// What "store current" did.
    pub enum Outcome {
        /// A new memory was added.
        Added = ("added", "Added"),
        /// The existing memory was updated.
        Updated = ("updated", "Updated"),
        /// A memory of the same channel exists. Nothing changed.
        Exists = ("exists", "Exists"),
    }
}

// Settings.
const UNDO_WINDOW: Setting<field::Duration> = Setting::new("undo_window", "Undo window", "s", 10.0)
    .range(0.0, 3600.0, 0.0)
    .help("How long after a delete the command undo puts the memories back.");
const SAME_CHANNEL: Setting<field::Hz> = Setting::new("same_channel", "Same channel", "Hz", 1.0)
    .range(0.0, 1e6, 0.0)
    .help("Two memories of one stack closer than this are the same channel.");

sdrkit_core::settings! {
    /// The settings of the memory service.
    pub enum MemorySetting {
        /// How long a delete can be undone.
        UndoWindow = UNDO_WINDOW,
        /// The frequency tolerance of "the same channel".
        SameChannel = SAME_CHANNEL,
    }
}

// Results.
const TOTAL: Field<field::I64> = Field::new("total", "Matching memories", "");
const CAN_UNDO: Field<field::Bool> = Field::new("can_undo", "Undo possible", "");

const POSITION: Field<field::I64> = Field::new("position", "Position", "");
const NAME: Field<field::Text> = Field::new("name", "Name", "");
const FREQ: Field<field::Hz> = Field::new("freq_hz", "Frequency", "Hz");
const END: Field<field::Hz> = Field::new("end_hz", "End", "Hz");
const STACK: Field<field::Text> = Field::new("stack", "Stack", "");
const SUBMODE: Field<field::Text> = Field::new("submode", "Demodulator", "");
const RATE: Field<field::Hz> = Field::new("rate_hz", "Rate", "Hz");
const BANDWIDTH: Field<field::Hz> = Field::new("bandwidth_hz", "Bandwidth", "Hz");
const GROUP: Field<field::Text> = Field::new("group", "Group", "");
const NOTES: Field<field::Text> = Field::new("notes", "Notes", "");
const COLOR: Field<field::Text> = Field::new("color", "Colour", "");
const STARRED: Field<field::Bool> = Field::new("starred", "Starred", "");
const SCAN: Field<field::Bool> = Field::new("scan", "In scan", "");
const LOCKOUT: Field<field::Bool> = Field::new("lockout", "Locked out", "");
const PRIORITY: Field<field::Bool> = Field::new("priority", "Priority", "");
const DWELL: Field<field::Duration> = Field::new("dwell_s", "Dwell", "s");
const ORDER: Field<field::I64> = Field::new("order", "Order", "");
const CREATED: Field<field::Time> = Field::new("created", "Created", "s");
const LAST_USED: Field<field::Time> = Field::new("last_used", "Last used", "s");
const HITS: Field<field::I64> = Field::new("hits", "Hits", "");
const LAST_HIT: Field<field::Time> = Field::new("last_hit", "Last hit", "s");
const TONE: Field<field::Text> = Field::new("tone", "Tone", "");
const ITEM_KEY: Field<field::Text> = Field::new("key", "Key", "");
const ITEM_OWNER: Field<field::Text> = Field::new("owner", "Owner", "");
const ITEM_VALUE: Field<field::Any> = Field::new("value", "Value", "");
const OF_MEMORY: Field<field::Text> =
    Field::new("memory", "Memory", "").refers_to(OWNER, "memories");
const OF_PRESET: Field<field::Text> =
    Field::new("preset", "Preset", "").refers_to(OWNER, "presets");
const BUILTIN: Field<field::Bool> = Field::new("builtin", "Built in", "");
const IS_DEFAULT: Field<field::Bool> = Field::new("is_default", "Default", "");
const SQUELCH: Field<field::Text> = Field::new("squelch", "Squelch", "");
const LINE: Field<field::I64> = Field::new("line", "Line", "");
const KIND: Field<field::Choice<Problem>> = Field::new("kind", "Problem", "");

/// The memories that the last query produces, one page.
pub const MEMORIES: TableDef = TableDef {
    key: "memories",
    label: "Memories",
    columns: &[
        &POSITION, &NAME, &FREQ, &STACK, &SUBMODE, &RATE, &BANDWIDTH, &GROUP, &NOTES, &COLOR,
        &STARRED, &SCAN, &LOCKOUT, &PRIORITY, &DWELL, &ORDER, &CREATED, &LAST_USED, &HITS,
        &LAST_HIT, &TONE,
    ],
    cap: MAX_PAGE,
};
/// The tags of the memories in [`MEMORIES`].
pub const MEMORY_TAGS: TableDef =
    item_table("memory_tags", "Memory tags", &[&OF_MEMORY, &ITEM_KEY]);
/// The stored stack settings of the memories in [`MEMORIES`]: the owner key, the setting key and the typed value.
pub const MEMORY_SETTINGS: TableDef = item_table(
    "memory_settings",
    "Memory settings",
    &[&OF_MEMORY, &ITEM_OWNER, &ITEM_KEY, &ITEM_VALUE],
);
/// The stored radio gains of the memories in [`MEMORIES`], in the form of [`MEMORY_SETTINGS`].
pub const MEMORY_GAINS: TableDef = item_table(
    "memory_gains",
    "Memory gains",
    &[&OF_MEMORY, &ITEM_OWNER, &ITEM_KEY, &ITEM_VALUE],
);
/// The groups in use.
pub const GROUPS: TableDef = TableDef {
    key: "groups",
    label: "Groups",
    columns: &[&NAME],
    cap: MEMORY_CAP,
};
/// The tags in use.
pub const TAGS: TableDef = TableDef {
    key: "tags",
    label: "Tags",
    columns: &[&NAME],
    cap: MEMORY_CAP,
};
/// The bookmarks.
pub const BOOKMARKS: TableDef = TableDef {
    key: "bookmarks",
    label: "Bookmarks",
    columns: &[&NAME, &FREQ, &END, &COLOR, &STACK, &NOTES],
    cap: crate::bookmark::MAX_BOOKMARKS,
};
/// The presets of every stack.
pub const PRESETS: TableDef = TableDef {
    key: "presets",
    label: "Presets",
    columns: &[
        &STACK,
        &NAME,
        &SUBMODE,
        &STARRED,
        &BUILTIN,
        &IS_DEFAULT,
        &FREQ,
        &RATE,
        &BANDWIDTH,
        &SQUELCH,
    ],
    cap: crate::preset::MAX_PRESETS,
};
/// The stored stack settings of the presets, in the form of [`MEMORY_SETTINGS`].
pub const PRESET_SETTINGS: TableDef = item_table(
    "preset_settings",
    "Preset settings",
    &[&OF_PRESET, &ITEM_OWNER, &ITEM_KEY, &ITEM_VALUE],
);
/// The stored radio gains of the presets, in the form of [`MEMORY_SETTINGS`].
pub const PRESET_GAINS: TableDef = item_table(
    "preset_gains",
    "Preset gains",
    &[&OF_PRESET, &ITEM_OWNER, &ITEM_KEY, &ITEM_VALUE],
);
/// The stored audio settings of the presets, in the form of [`MEMORY_SETTINGS`].
pub const PRESET_AUDIO: TableDef = item_table(
    "preset_audio",
    "Preset audio",
    &[&OF_PRESET, &ITEM_OWNER, &ITEM_KEY, &ITEM_VALUE],
);
/// The problems of the last import, one row for each line that was not read.
pub const PROBLEMS: TableDef = TableDef {
    key: "problems",
    label: "Import problems",
    columns: &[&LINE, &KIND],
    cap: MAX_PROBLEMS,
};

const MAX_PAGE: usize = 1000;
const MAX_ITEMS: usize = 100_000;
const MAX_PROBLEMS: usize = 1000;

const fn item_table(
    key: &'static str,
    label: &'static str,
    columns: &'static [&'static dyn Declare],
) -> TableDef {
    TableDef {
        key,
        label,
        columns,
        cap: MAX_ITEMS,
    }
}

// Arguments.
/// The row key of the memory, bookmark or preset that a command acts on.
pub const ROW: Argument<field::Text> =
    Argument::new(Field::new("row", "Row", "")).help("The row key, as the table shows it.");
/// The session that a recall or a preset tunes: its row key in the engine's sessions table.
pub const SESSION: Argument<field::Text> =
    Argument::new(Field::new("session", "Session", "").refers_to("engine", "sessions"));
const A_SEARCH: Setting<field::Text> = Setting::new("search", "Search", "", "")
    .help("Words to find in name, notes, tags, group or stack, or a frequency.");
const A_SORT: Setting<field::Choice<SortKey>> = Setting::choice("sort", "Sort by", SortKey::Manual);
const A_ASCENDING: Setting<field::Bool> = Setting::new("ascending", "Ascending", "", true);
const A_FILTER: Setting<field::Choice<FilterKind>> =
    Setting::choice("filter", "Filter", FilterKind::All);
const A_GROUP: Setting<field::Text> = Setting::new("group", "Group", "", "");
const A_LO: Setting<field::Hz> = Setting::new("lo_hz", "From", "Hz", 0.0).range(0.0, 1e12, 0.0);
const A_HI: Setting<field::Hz> = Setting::new("hi_hz", "To", "Hz", 1e12).range(0.0, 1e12, 0.0);
/// The most rows of a page.
pub const A_LIMIT: Setting<field::I64> =
    Setting::new("limit", "Rows", "", 100).range(1.0, MAX_PAGE as f64, 1.0);
/// The place of the first row of a page in the whole result.
pub const A_OFFSET: Setting<field::I64> =
    Setting::new("offset", "First row", "", 0).range(0.0, MEMORY_CAP as f64, 1.0);
const A_NAME: Setting<field::Text> = Setting::new("name", "Name", "", "");
const A_FREQ: Argument<field::Hz> = Argument::new(FREQ);
const A_STACK: Setting<field::Text> = Setting::new("stack", "Stack", "", "");
const A_SUBMODE: Setting<field::Text> = Setting::new("submode", "Demodulator", "", "");
const A_RATE: Setting<field::Hz> = Setting::new("rate_hz", "Rate", "Hz", 0.0)
    .range(0.0, 1e10, 0.0)
    .zero_means("No change");
const A_BANDWIDTH: Setting<field::Hz> = Setting::new("bandwidth_hz", "Bandwidth", "Hz", 0.0)
    .range(0.0, 1e10, 0.0)
    .zero_means("The bandwidth of the stack");
const A_NOTES: Setting<field::Text> = Setting::new("notes", "Notes", "", "");
const A_COLOR: Setting<field::Text> =
    Setting::new("color", "Colour", "", "").help("#rrggbb, or empty for none.");
const A_STARRED: Setting<field::Bool> = Setting::new("starred", "Starred", "", false);
const A_SCAN: Setting<field::Bool> = Setting::new("scan", "In scan", "", true);
const A_LOCKOUT: Setting<field::Bool> = Setting::new("lockout", "Locked out", "", false);
const A_PRIORITY: Setting<field::Bool> = Setting::new("priority", "Priority", "", false);
const A_DWELL: Setting<field::Duration> = Setting::new("dwell_s", "Dwell", "s", 0.0)
    .range(0.0, 86_400.0, 0.0)
    .zero_means("The dwell time of the scan");
const A_TONE: Setting<field::Text> = Setting::new("tone", "Tone", "", "");
const A_ON_SAME: Setting<field::Choice<OnSameChoice>> =
    Setting::choice("on_same", "If it exists", OnSameChoice::Ask);
const A_INDEX: Argument<field::I64> = Argument::new(Field::new("index", "Place", ""));
const A_PART: Argument<field::Choice<Part>> = Argument::new(Field::new("part", "Part", ""));
const A_KEY: Argument<field::Text> = Argument::new(ITEM_KEY);
const A_OWNER: Argument<field::Text> =
    Argument::new(ITEM_OWNER).help("The owner key of the setting (\"stack.general\").");
const A_VALUE: Argument<field::Any> =
    Argument::new(ITEM_VALUE).help("The value, of the kind that the owner's schema gives.");
const A_PATH: Argument<field::Text> = Argument::new(Field::new("path", "File", ""))
    .help("A file in the storage of the host, as a relative path.");
const A_FORMAT: Setting<field::Choice<Format>> = Setting::choice("format", "Format", Format::Auto);
const A_START: Argument<field::Hz> = Argument::new(FREQ);
const A_END: Setting<field::Hz> = Setting::new("end_hz", "End", "Hz", 0.0)
    .range(0.0, 1e12, 0.0)
    .help("0 for a point.");
const A_STACK_REQ: Argument<field::Text> = Argument::new(STACK);
const A_PRESET_NAME: Argument<field::Text> = Argument::new(NAME);
const A_PRESET_FREQ: Setting<field::Hz> = Setting::new("freq_hz", "Frequency", "Hz", 0.0)
    .range(0.0, 1e12, 0.0)
    .zero_means("No change");
const A_SQUELCH: Setting<field::Text> = Setting::new("squelch", "Squelch", "", "");
const A_ON: Setting<field::Bool> = Setting::new("on", "On", "", true);
const A_PRESET_ROW: Setting<field::Text> =
    Setting::new("row", "Preset", "", "").help("The row key of the preset, or empty for none.");

// Replies.
const R_ID: Field<field::Text> = Field::new("row", "Row", "");
const R_OUTCOME: Field<field::Choice<Outcome>> = Field::new("outcome", "Outcome", "");
const R_ADDED: Field<field::I64> = Field::new("added", "Added", "");
const R_DUPLICATES: Field<field::I64> = Field::new("duplicates", "Duplicates", "");
const R_OVER_CAP: Field<field::I64> = Field::new("over_cap", "Over the cap", "");
const R_PROBLEMS: Field<field::I64> = Field::new("problems", "Problems", "");
const R_WRITTEN: Field<field::I64> = Field::new("written", "Rows written", "");

const MEMORY_ARGS: &[&dyn sdrkit_core::DeclareSetting] = &[
    &A_NAME,
    &A_FREQ,
    &A_STACK,
    &A_SUBMODE,
    &A_RATE,
    &A_BANDWIDTH,
    &A_GROUP,
    &A_NOTES,
    &A_COLOR,
    &A_STARRED,
    &A_SCAN,
    &A_LOCKOUT,
    &A_PRIORITY,
    &A_DWELL,
    &A_TONE,
];
const UPDATE_ARGS: &[&dyn sdrkit_core::DeclareSetting] = &[
    &ROW,
    &A_NAME,
    &A_FREQ,
    &A_STACK,
    &A_SUBMODE,
    &A_RATE,
    &A_BANDWIDTH,
    &A_GROUP,
    &A_NOTES,
    &A_COLOR,
    &A_STARRED,
    &A_SCAN,
    &A_LOCKOUT,
    &A_PRIORITY,
    &A_DWELL,
    &A_TONE,
];
const STORE_ARGS: &[&dyn sdrkit_core::DeclareSetting] = &[
    &A_NAME,
    &A_FREQ,
    &A_STACK,
    &A_SUBMODE,
    &A_RATE,
    &A_BANDWIDTH,
    &A_GROUP,
    &A_NOTES,
    &A_COLOR,
    &A_STARRED,
    &A_SCAN,
    &A_LOCKOUT,
    &A_PRIORITY,
    &A_DWELL,
    &A_TONE,
    &A_ON_SAME,
];

const QUERY: Command = Command::new("query", "Show memories")
    .help("Sets which memories the memory table shows: a search, a filter, a sort and a page.")
    .args(&[
        &A_SEARCH,
        &A_SORT,
        &A_ASCENDING,
        &A_FILTER,
        &A_GROUP,
        &A_LO,
        &A_HI,
        &A_LIMIT,
        &A_OFFSET,
    ]);
const ADD: Command = Command::new("add", "Add memory")
    .args(MEMORY_ARGS)
    .reply(&[&R_ID]);
const UPDATE: Command = Command::new("update", "Change memory")
    .help("Replaces every value of the memory. Its tags, settings and gains stay.")
    .args(UPDATE_ARGS);
const REMOVE: Command = Command::new("remove", "Delete memory")
    .help("Deletes the memory. The command undo puts it back within the undo window.")
    .args(&[&ROW]);
const UNDO: Command = Command::new("undo", "Undo delete");
const MOVE: Command = Command::new("move", "Move memory")
    .help("Moves the memory to a place in the manual order.")
    .args(&[&ROW, &A_INDEX]);
const RECALL: Command = Command::new("recall", "Recall memory")
    .help("Notes the recall and asks the engine to tune the session to the memory.")
    .args(&[&ROW, &SESSION]);
const STORE_CURRENT: Command = Command::new("store_current", "Store current")
    .help("Adds a memory, or acts on the memory of the same channel as on_same says.")
    .args(STORE_ARGS)
    .reply(&[&R_ID, &R_OUTCOME]);
const TAG_ADD: Command = Command::new("tag.add", "Add tag").args(&[&ROW, &A_KEY]);
const TAG_REMOVE: Command = Command::new("tag.remove", "Remove tag").args(&[&ROW, &A_KEY]);
const ITEM_SET: Command = Command::new("item.set", "Set stored setting")
    .help("Adds or replaces a stored setting, gain or audio setting of a memory or a preset.")
    .args(&[&ROW, &A_PART, &A_OWNER, &A_KEY, &A_VALUE]);
const ITEM_REMOVE: Command =
    Command::new("item.remove", "Remove stored setting").args(&[&ROW, &A_PART, &A_OWNER, &A_KEY]);
const IMPORT: Command = Command::new("import", "Import memories")
    .help("Adds the memories of the file `path`. Duplicates are skipped. The problems table lists the lines not read.")
    .args(&[&A_PATH, &A_FORMAT])
    .reply(&[&R_ADDED, &R_DUPLICATES, &R_OVER_CAP, &R_PROBLEMS]);
const EXPORT: Command = Command::new("export", "Export memories")
    .help("Writes every memory that the query takes to the file `path`. Automatic writes JSON.")
    .args(&[&A_PATH, &A_FORMAT])
    .reply(&[&R_WRITTEN]);
const BOOKMARK_ADD: Command = Command::new("bookmark.add", "Add bookmark")
    .args(&[&A_NAME, &A_START, &A_END, &A_COLOR, &A_STACK, &A_NOTES])
    .reply(&[&R_ID]);
const BOOKMARK_REMOVE: Command = Command::new("bookmark.remove", "Delete bookmark").args(&[&ROW]);
const BOOKMARK_IMPORT: Command = Command::new("bookmark.import", "Import bookmarks")
    .args(&[&A_PATH])
    .reply(&[&R_ADDED, &R_PROBLEMS]);
const BOOKMARK_EXPORT: Command = Command::new("bookmark.export", "Export bookmarks")
    .args(&[&A_PATH])
    .reply(&[&R_WRITTEN]);
const PRESET_SAVE: Command = Command::new("preset.save", "Save preset")
    .help("Saves a preset. A preset of the same stack and name is replaced.")
    .args(&[
        &A_STACK_REQ,
        &A_PRESET_NAME,
        &A_SUBMODE,
        &A_PRESET_FREQ,
        &A_RATE,
        &A_BANDWIDTH,
        &A_SQUELCH,
    ])
    .reply(&[&R_ID]);
const PRESET_RENAME: Command =
    Command::new("preset.rename", "Rename preset").args(&[&ROW, &A_PRESET_NAME]);
const PRESET_DELETE: Command = Command::new("preset.delete", "Delete preset").args(&[&ROW]);
const PRESET_STAR: Command = Command::new("preset.star", "Star preset").args(&[&ROW, &A_ON]);
const PRESET_SET_DEFAULT: Command = Command::new("preset.set_default", "Default preset")
    .help("Makes the preset the default of the stack, or clears the default.")
    .args(&[&A_STACK_REQ, &A_PRESET_ROW]);
const PRESET_APPLY: Command = Command::new("preset.apply", "Apply preset")
    .help("Asks the engine to tune the session with the preset.")
    .args(&[&ROW, &SESSION]);

sdrkit_core::commands! {
    /// The commands of the memory service.
    pub enum MemoryCommand {
        /// Sets what the memory table shows.
        Query = QUERY,
        /// Adds a memory.
        Add = ADD,
        /// Replaces the values of a memory.
        Update = UPDATE,
        /// Deletes a memory.
        Remove = REMOVE,
        /// Puts back the last delete.
        Undo = UNDO,
        /// Moves a memory in the manual order.
        Move = MOVE,
        /// Records a recall and requests a tune from the engine.
        Recall = RECALL,
        /// Stores the current channel.
        StoreCurrent = STORE_CURRENT,
        /// Adds a tag to a memory.
        TagAdd = TAG_ADD,
        /// Removes a tag from a memory.
        TagRemove = TAG_REMOVE,
        /// Sets a stored setting, gain or audio setting.
        ItemSet = ITEM_SET,
        /// Removes a stored setting, gain or audio setting.
        ItemRemove = ITEM_REMOVE,
        /// Imports memories.
        Import = IMPORT,
        /// Exports memories.
        Export = EXPORT,
        /// Adds a bookmark.
        BookmarkAdd = BOOKMARK_ADD,
        /// Deletes a bookmark.
        BookmarkRemove = BOOKMARK_REMOVE,
        /// Imports bookmarks.
        BookmarkImport = BOOKMARK_IMPORT,
        /// Exports bookmarks.
        BookmarkExport = BOOKMARK_EXPORT,
        /// Saves a preset.
        PresetSave = PRESET_SAVE,
        /// Renames a preset.
        PresetRename = PRESET_RENAME,
        /// Deletes a preset.
        PresetDelete = PRESET_DELETE,
        /// Stars a preset.
        PresetStar = PRESET_STAR,
        /// Sets the default preset of a stack.
        PresetSetDefault = PRESET_SET_DEFAULT,
        /// Asks the engine to tune with a preset.
        PresetApply = PRESET_APPLY,
    }
}

/// What the memory table shows.
#[derive(Clone, Debug)]
struct Query {
    search: String,
    sort: SortKey,
    ascending: bool,
    filter: MemoryFilter,
    limit: usize,
    offset: usize,
}

impl Default for Query {
    fn default() -> Self {
        Query {
            search: String::new(),
            sort: SortKey::Manual,
            ascending: true,
            filter: MemoryFilter::All,
            limit: 100,
            offset: 0,
        }
    }
}

/// The table ids, found once.
#[derive(Clone, Copy, Debug)]
struct Ids {
    memories: TableId,
    bookmarks: TableId,
    presets: TableId,
}

/// The memory service: memories, bookmarks and presets, kept in the host's storage.
pub struct MemoryService {
    storage: Arc<dyn Storage>,
    clock: Clock,
    values: Values,
    memories: MemoryStore,
    bookmarks: BookmarkStore,
    presets: PresetStore,
    query: Query,
    p: Published,
    ids: Ids,
    requests: Vec<HostRequest>,
}

impl std::fmt::Debug for MemoryService {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MemoryService")
            .field("memories", &self.memories.len())
            .finish()
    }
}

fn decl() -> Arc<ResultsDecl> {
    let mut d = ResultsDecl::new().with_fields(&[&TOTAL, &CAN_UNDO]);
    for t in [
        &MEMORIES,
        &MEMORY_TAGS,
        &MEMORY_SETTINGS,
        &MEMORY_GAINS,
        &GROUPS,
        &TAGS,
        &BOOKMARKS,
        &PRESETS,
        &PRESET_SETTINGS,
        &PRESET_GAINS,
        &PRESET_AUDIO,
        &PROBLEMS,
    ] {
        d = d.with_table(t);
    }
    Arc::new(d)
}

/// The store path of a file of the service.
fn path(p: &'static str) -> StorePath {
    StorePath::new(p).expect("the file names of the service are store paths")
}

impl MemoryService {
    /// Opens the service on the host's `storage`, with the host's wall `clock` for creation, recall and undo times. A file
    /// that does not parse is moved aside and the service starts without its data.
    ///
    /// # Errors
    ///
    /// [`MemoryError::Storage`] if the storage fails.
    pub fn open(storage: Arc<dyn Storage>, clock: Clock) -> Result<Self, MemoryError> {
        let loaded = load_memories(&*storage, &path(MEMORIES_FILE), MEMORY_CAP)?;
        let bookmarks = load_json::<BookmarkStore>(&*storage, &path(BOOKMARKS_FILE))?.value;
        let mut presets = load_json::<PresetStore>(&*storage, &path(PRESETS_FILE))?.value;
        presets.add_builtins();
        let p = Published::from_decl(decl());
        let d = p.results().decl();
        let id = |t: &TableDef| d.table_id(t.key).expect("the service declares its tables");
        let ids = Ids {
            memories: id(&MEMORIES),
            bookmarks: id(&BOOKMARKS),
            presets: id(&PRESETS),
        };
        let mut s = MemoryService {
            storage,
            clock,
            values: Values::defaults(&MemorySetting::schema()),
            memories: loaded.store,
            bookmarks,
            presets,
            query: Query::default(),
            p,
            ids,
            requests: Vec::new(),
        };
        if let Some(aside) = loaded.set_aside {
            s.p.results_mut().log(format!(
                "The memory file did not read. It is kept as {aside}."
            ));
        }
        s.publish();
        Ok(s)
    }

    /// The memories, for the engine (a scan list of memories).
    pub fn store(&self) -> &MemoryStore {
        &self.memories
    }

    /// Changes the memories through `f` (a scan writes its hits back), saves them and publishes them.
    ///
    /// # Errors
    ///
    /// [`MemoryError::Storage`] or [`MemoryError::Encode`] if the save fails. The change stays.
    pub fn update_store<T>(
        &mut self,
        f: impl FnOnce(&mut MemoryStore) -> T,
    ) -> Result<T, MemoryError> {
        let out = f(&mut self.memories);
        self.save_memories()?;
        self.publish();
        Ok(out)
    }

    /// The preset with row key `id`, for the engine to apply.
    pub fn preset(&self, id: u64) -> Option<&Preset> {
        self.presets.get(id)
    }

    /// The default preset of `stack`, for the engine to apply when the stack starts.
    pub fn default_preset(&self, stack: &str) -> Option<&Preset> {
        self.presets.default_for(stack)
    }

    /// Takes the requests that the commands made since the last call (a recall, a preset to apply), for the engine to carry
    /// out in order.
    pub fn take_requests(&mut self) -> Vec<HostRequest> {
        std::mem::take(&mut self.requests)
    }

    fn undo_window(&self) -> f64 {
        self.values.get(&UNDO_WINDOW)
    }

    fn tolerance(&self) -> f64 {
        self.values.get(&SAME_CHANNEL)
    }

    fn read_file(&self, args: &Values) -> Result<String, CommandError> {
        let path = file_arg(args)?;
        let bytes = self
            .storage
            .read(&path)
            .map_err(|source| failed(MemoryError::Storage { path, source }))?;
        Ok(String::from_utf8_lossy(&bytes).into_owned())
    }

    fn write_file(&self, args: &Values, text: &str) -> Result<(), CommandError> {
        let path = file_arg(args)?;
        self.storage
            .write(&path, text.as_bytes())
            .map_err(|source| failed(MemoryError::Storage { path, source }))
    }

    fn save_memories(&self) -> Result<(), MemoryError> {
        save_memories(&*self.storage, &path(MEMORIES_FILE), &self.memories)
    }

    fn save_bookmarks(&self) -> Result<(), MemoryError> {
        save_json(&*self.storage, &path(BOOKMARKS_FILE), &self.bookmarks)
    }

    fn save_presets(&self) -> Result<(), MemoryError> {
        save_json(
            &*self.storage,
            &path(PRESETS_FILE),
            &self.presets.to_saved(),
        )
    }

    fn publish(&mut self) {
        self.publish_memories();
        self.publish_bookmarks();
        self.publish_presets();
    }

    fn publish_memories(&mut self) {
        let can_undo = self.memories.can_undo(self.clock.now(), self.undo_window());
        let q = (self.query.offset, self.query.limit);
        let found = found(&self.memories, &self.query);
        let total = found.len();
        let page: Vec<&Memory> = found.into_iter().skip(q.0).take(q.1).collect();
        let p = &mut self.p;
        p.set(&TOTAL, count(total));
        p.set(&CAN_UNDO, can_undo);
        let mut keep = HashSet::new();
        let (mut tags, mut settings, mut gains) = (HashSet::new(), HashSet::new(), HashSet::new());
        for (i, m) in page.iter().enumerate().rev() {
            let key = m.id.to_string();
            if let Some(mut r) = p.row_mut(&MEMORIES, &key) {
                r.set(&POSITION, count(i));
                write_memory(&mut r, m);
            }
            for t in &m.tags {
                let row = format!("{key}/{t}");
                if let Some(mut r) = p.row_mut(&MEMORY_TAGS, &row) {
                    r.set(&OF_MEMORY, &key);
                    r.set(&ITEM_KEY, t);
                }
                tags.insert(row);
            }
            stored(
                p,
                &MEMORY_SETTINGS,
                &OF_MEMORY,
                &key,
                &m.settings,
                &mut settings,
            );
            stored(p, &MEMORY_GAINS, &OF_MEMORY, &key, &m.gains, &mut gains);
            keep.insert(key);
        }
        prune(p, &MEMORIES, &keep);
        prune(p, &MEMORY_TAGS, &tags);
        prune(p, &MEMORY_SETTINGS, &settings);
        prune(p, &MEMORY_GAINS, &gains);
        names(p, &GROUPS, self.memories.groups());
        names(p, &TAGS, self.memories.tags());
    }

    fn publish_bookmarks(&mut self) {
        let p = &mut self.p;
        let mut keep = HashSet::new();
        for b in self.bookmarks.bookmarks.iter().rev() {
            let key = b.id.to_string();
            if let Some(mut r) = p.row_mut(&BOOKMARKS, &key) {
                r.set(&NAME, &b.name);
                r.set(&FREQ, b.start_hz);
                r.opt(&END, b.end_hz);
                r.set(&COLOR, &b.color);
                r.set(&STACK, &b.stack);
                r.set(&NOTES, &b.notes);
            }
            keep.insert(key);
        }
        prune(p, &BOOKMARKS, &keep);
    }

    fn publish_presets(&mut self) {
        let p = &mut self.p;
        let mut keep = HashSet::new();
        let (mut s, mut g, mut a) = (HashSet::new(), HashSet::new(), HashSet::new());
        for pr in self.presets.presets.iter().rev() {
            let key = pr.id.to_string();
            if let Some(mut r) = p.row_mut(&PRESETS, &key) {
                r.set(&STACK, &pr.stack);
                r.set(&NAME, &pr.name);
                r.set(&SUBMODE, &pr.submode);
                r.set(&STARRED, pr.starred);
                r.set(&BUILTIN, pr.builtin);
                r.set(
                    &IS_DEFAULT,
                    self.presets.defaults.get(&pr.stack) == Some(&pr.id),
                );
                r.opt(&FREQ, pr.freq_hz);
                r.opt(&RATE, pr.rate_hz);
                r.opt(&BANDWIDTH, pr.bandwidth_hz);
                r.set(&SQUELCH, &pr.squelch);
            }
            stored(p, &PRESET_SETTINGS, &OF_PRESET, &key, &pr.settings, &mut s);
            stored(p, &PRESET_GAINS, &OF_PRESET, &key, &pr.gains, &mut g);
            stored(p, &PRESET_AUDIO, &OF_PRESET, &key, &pr.audio, &mut a);
            keep.insert(key);
        }
        prune(p, &PRESETS, &keep);
        prune(p, &PRESET_SETTINGS, &s);
        prune(p, &PRESET_GAINS, &g);
        prune(p, &PRESET_AUDIO, &a);
    }

    fn publish_problems(&mut self, problems: &[LineProblem]) {
        self.p.clear_rows(&PROBLEMS);
        for (i, pr) in problems.iter().take(MAX_PROBLEMS).enumerate().rev() {
            if let Some(mut r) = self.p.row_mut(&PROBLEMS, &i.to_string()) {
                r.set(&LINE, count(pr.line));
                r.set(&KIND, pr.kind);
            }
        }
    }

    fn memory_id(&self, args: &Values) -> Result<u64, CommandError> {
        row_id(args.arg(&ROW)?, self.ids.memories, |id| {
            self.memories.get(id).is_some()
        })
    }

    fn preset_id(&self, row: &str) -> Result<u64, CommandError> {
        row_id(row, self.ids.presets, |id| self.presets.get(id).is_some())
    }

    fn run(&mut self, cmd: MemoryCommand, args: &Values) -> Result<Option<Record>, CommandError> {
        let mut reply = MemoryCommand::commands()
            .def(cmd.id())
            .map(|d| d.new_reply())
            .filter(|r| !r.decl().is_empty());
        match cmd {
            MemoryCommand::Query => self.query = query_of(args),
            MemoryCommand::Add
            | MemoryCommand::Update
            | MemoryCommand::Remove
            | MemoryCommand::Undo
            | MemoryCommand::Move
            | MemoryCommand::Recall
            | MemoryCommand::StoreCurrent
            | MemoryCommand::TagAdd
            | MemoryCommand::TagRemove => self.memory_command(cmd, args, reply.as_mut())?,
            MemoryCommand::ItemSet | MemoryCommand::ItemRemove => self.item(cmd, args)?,
            MemoryCommand::Import => self.import(args, reply.as_mut())?,
            MemoryCommand::Export => {
                let list = found(&self.memories, &self.query);
                let text = match args.get(&A_FORMAT) {
                    Format::Auto | Format::Json => exchange::export_json(&list),
                    Format::Csv => exchange::export_csv(&list),
                    Format::Plain => exchange::export_plain(&list),
                };
                let n = list.len();
                self.write_file(args, &text)?;
                set_count(reply.as_mut(), &R_WRITTEN, n);
            }
            MemoryCommand::BookmarkAdd
            | MemoryCommand::BookmarkRemove
            | MemoryCommand::BookmarkImport
            | MemoryCommand::BookmarkExport => self.bookmark_command(cmd, args, reply.as_mut())?,
            MemoryCommand::PresetSave
            | MemoryCommand::PresetRename
            | MemoryCommand::PresetDelete
            | MemoryCommand::PresetStar
            | MemoryCommand::PresetSetDefault
            | MemoryCommand::PresetApply => self.preset_command(cmd, args, reply.as_mut())?,
        }
        self.publish();
        Ok(reply)
    }

    fn memory_command(
        &mut self,
        cmd: MemoryCommand,
        args: &Values,
        reply: Option<&mut Record>,
    ) -> Result<(), CommandError> {
        let now = self.clock.now();
        let table = self.ids.memories;
        let fail = move |e| failed_in(e, table);
        match cmd {
            MemoryCommand::Add => {
                let id = self.memories.insert(memory_of(args)?, now).map_err(fail)?;
                set_row(reply, id);
            }
            MemoryCommand::Update => {
                let id = self.memory_id(args)?;
                let m = memory_of(args)?;
                self.memories
                    .update(id, |e| {
                        let keep = std::mem::take(e);
                        *e = Memory {
                            id,
                            tags: keep.tags,
                            settings: keep.settings,
                            gains: keep.gains,
                            order: keep.order,
                            created_s: keep.created_s,
                            last_used_s: keep.last_used_s,
                            hits: keep.hits,
                            last_hit_s: keep.last_hit_s,
                            ..m
                        };
                    })
                    .map_err(fail)?;
            }
            MemoryCommand::Remove => {
                let id = self.memory_id(args)?;
                self.memories.remove(&[id], now);
            }
            MemoryCommand::Undo => {
                if self.memories.undo_remove(now, self.undo_window()) == 0 {
                    return Ok(());
                }
            }
            MemoryCommand::Move => {
                let id = self.memory_id(args)?;
                let index = usize::try_from(args.arg(&A_INDEX)?.max(0)).unwrap_or(usize::MAX);
                self.memories.move_to(id, index).map_err(fail)?;
            }
            MemoryCommand::Recall => {
                let id = self.memory_id(args)?;
                let session = args.arg(&SESSION)?.to_owned();
                self.memories.mark_used(id, now).map_err(fail)?;
                if let Some(m) = self.memories.get(id) {
                    self.requests.push(HostRequest::Tune {
                        session,
                        freq_hz: Some(m.freq_hz),
                        target: TuneTarget::of_stack(&m.stack),
                        submode: m.submode.clone(),
                        rate_hz: m.rate_hz,
                        bandwidth_hz: m.bandwidth_hz,
                        settings: m.settings.iter().chain(&m.gains).cloned().collect(),
                    });
                }
            }
            MemoryCommand::StoreCurrent => {
                let on_same = match args.get(&A_ON_SAME) {
                    OnSameChoice::Ask => OnSame::Ask,
                    OnSameChoice::Update => OnSame::Update,
                    OnSameChoice::Add => OnSame::Add,
                };
                let tolerance = self.tolerance();
                let stored = self
                    .memories
                    .store_current(memory_of(args)?, on_same, now, tolerance)
                    .map_err(fail)?;
                let (id, outcome) = match stored {
                    Stored::Added(id) => (id, Outcome::Added),
                    Stored::Updated(id) => (id, Outcome::Updated),
                    Stored::Exists(id) => (id, Outcome::Exists),
                };
                if let Some(r) = reply {
                    r.set(&R_ID, &id.to_string());
                    r.set(&R_OUTCOME, outcome);
                }
                if outcome == Outcome::Exists {
                    return Ok(());
                }
            }
            MemoryCommand::TagAdd | MemoryCommand::TagRemove => {
                let id = self.memory_id(args)?;
                let tag = args.arg(&A_KEY)?;
                let add = cmd == MemoryCommand::TagAdd;
                self.memories
                    .update(id, |m| {
                        m.tags.retain(|t| t != tag);
                        if add {
                            m.tags.push(tag.to_owned());
                        }
                    })
                    .map_err(fail)?;
            }
            _ => return Ok(()),
        }
        self.save_memories().map_err(fail)
    }

    fn item(&mut self, cmd: MemoryCommand, args: &Values) -> Result<(), CommandError> {
        let owner = owner_arg(args)?;
        let key = args.arg(&A_KEY)?;
        let value = match cmd {
            MemoryCommand::ItemSet => Some(args.arg(&A_VALUE)?.clone()),
            _ => None,
        };
        let edit = |list: &mut Vec<OwnerSetting>| match &value {
            Some(v) => put_setting(list, owner.clone(), key, v.clone()),
            None => {
                remove_setting(list, &owner, key);
            }
        };
        match args.arg(&A_PART)? {
            part @ (Part::MemorySetting | Part::MemoryGain) => {
                let id = self.memory_id(args)?;
                let table = self.ids.memories;
                self.memories
                    .update(id, |m| match part {
                        Part::MemorySetting => edit(&mut m.settings),
                        _ => edit(&mut m.gains),
                    })
                    .map_err(|e| failed_in(e, table))?;
                self.save_memories().map_err(|e| failed_in(e, table))
            }
            part @ (Part::PresetSetting | Part::PresetGain | Part::PresetAudio) => {
                let id = self.preset_id(args.arg(&ROW)?)?;
                if let Some(pr) = self.presets.presets.iter_mut().find(|p| p.id == id) {
                    match part {
                        Part::PresetSetting => edit(&mut pr.settings),
                        Part::PresetGain => edit(&mut pr.gains),
                        _ => edit(&mut pr.audio),
                    }
                }
                self.save_presets().map_err(failed)
            }
        }
    }

    fn import(&mut self, args: &Values, reply: Option<&mut Record>) -> Result<(), CommandError> {
        let text = self.read_file(args)?;
        let (now, tolerance) = (self.clock.now(), self.tolerance());
        let r = exchange::import(
            &mut self.memories,
            &text,
            args.get(&A_FORMAT),
            now,
            tolerance,
        );
        if r.added > 0 {
            self.save_memories().map_err(failed)?;
        }
        self.publish_problems(&r.problems);
        if let Some(out) = reply {
            out.set(&R_ADDED, count(r.added));
            out.set(&R_DUPLICATES, count(r.duplicates));
            out.set(&R_OVER_CAP, count(r.over_cap));
            out.set(&R_PROBLEMS, count(r.problems.len()));
        }
        Ok(())
    }

    fn bookmark_command(
        &mut self,
        cmd: MemoryCommand,
        args: &Values,
        reply: Option<&mut Record>,
    ) -> Result<(), CommandError> {
        match cmd {
            MemoryCommand::BookmarkAdd => {
                let (name, start, end) = (args.get(&A_NAME), args.arg(&A_START)?, args.get(&A_END));
                let mut b = if end > 0.0 {
                    Bookmark::range(name, start, end)
                } else {
                    Bookmark::point(name, start)
                };
                b.color = args.get(&A_COLOR).to_owned();
                b.stack = args.get(&A_STACK).to_owned();
                b.notes = args.get(&A_NOTES).to_owned();
                let id = self.bookmarks.add(b).map_err(failed)?;
                set_row(reply, id);
            }
            MemoryCommand::BookmarkRemove => {
                let table = self.ids.bookmarks;
                let id = row_id(args.arg(&ROW)?, table, |id| {
                    self.bookmarks.bookmarks.iter().any(|b| b.id == id)
                })?;
                self.bookmarks.remove(id);
            }
            MemoryCommand::BookmarkImport => {
                let text = self.read_file(args)?;
                let (added, problems) = self.bookmarks.import_csv(&text);
                self.publish_problems(&problems);
                if let Some(out) = reply {
                    out.set(&R_ADDED, count(added));
                    out.set(&R_PROBLEMS, count(problems.len()));
                }
                if added == 0 {
                    return Ok(());
                }
            }
            _ => {
                self.write_file(args, &self.bookmarks.export_csv())?;
                set_count(reply, &R_WRITTEN, self.bookmarks.bookmarks.len());
                return Ok(());
            }
        }
        self.save_bookmarks().map_err(failed)
    }

    fn preset_command(
        &mut self,
        cmd: MemoryCommand,
        args: &Values,
        reply: Option<&mut Record>,
    ) -> Result<(), CommandError> {
        let table = self.ids.presets;
        let fail = move |e| failed_in(e, table);
        match cmd {
            MemoryCommand::PresetSave => {
                let opt = |v: f64| (v > 0.0).then_some(v);
                let pr = Preset {
                    stack: args.arg(&A_STACK_REQ)?.to_owned(),
                    name: args.arg(&A_PRESET_NAME)?.to_owned(),
                    submode: args.get(&A_SUBMODE).to_owned(),
                    freq_hz: opt(args.get(&A_PRESET_FREQ)),
                    rate_hz: opt(args.get(&A_RATE)),
                    bandwidth_hz: opt(args.get(&A_BANDWIDTH)),
                    squelch: args.get(&A_SQUELCH).to_owned(),
                    ..Preset::default()
                };
                let id = self.presets.save(pr).map_err(fail)?;
                set_row(reply, id);
            }
            MemoryCommand::PresetRename => {
                let id = self.preset_id(args.arg(&ROW)?)?;
                self.presets
                    .rename(id, args.arg(&A_PRESET_NAME)?)
                    .map_err(fail)?;
            }
            MemoryCommand::PresetDelete => {
                let id = self.preset_id(args.arg(&ROW)?)?;
                self.presets.delete(id).map_err(fail)?;
            }
            MemoryCommand::PresetStar => {
                let id = self.preset_id(args.arg(&ROW)?)?;
                self.presets.star(id, args.get(&A_ON)).map_err(fail)?;
            }
            MemoryCommand::PresetSetDefault => {
                let row = args.get(&A_PRESET_ROW);
                let id = if row.is_empty() {
                    None
                } else {
                    Some(self.preset_id(row)?)
                };
                self.presets
                    .set_default(args.arg(&A_STACK_REQ)?, id)
                    .map_err(fail)?;
            }
            _ => {
                let id = self.preset_id(args.arg(&ROW)?)?;
                let session = args.arg(&SESSION)?.to_owned();
                if let Some(pr) = self.presets.get(id) {
                    self.requests.push(HostRequest::Tune {
                        session,
                        freq_hz: pr.freq_hz,
                        target: TuneTarget::of_stack(&pr.stack),
                        submode: pr.submode.clone(),
                        rate_hz: pr.rate_hz,
                        bandwidth_hz: pr.bandwidth_hz,
                        settings: pr
                            .settings
                            .iter()
                            .chain(&pr.gains)
                            .chain(&pr.audio)
                            .cloned()
                            .collect(),
                    });
                }
                return Ok(());
            }
        }
        self.save_presets().map_err(fail)
    }
}

impl Owner for MemoryService {
    fn schema(&self) -> &Arc<Schema> {
        self.values.schema()
    }
    fn values(&self) -> Option<&Values> {
        Some(&self.values)
    }
    fn apply(&mut self, id: SettingId, value: &Value) -> Result<(), SettingError> {
        match MemorySetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            MemorySetting::UndoWindow | MemorySetting::SameChannel => self.values.set(id, value)?,
        }
        self.publish_memories();
        Ok(())
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
        MemoryCommand::commands()
    }
    fn invoke(&mut self, id: CommandId, args: &Values) -> Result<Option<Record>, CommandError> {
        let cmd = MemoryCommand::from_id(id).ok_or(CommandError::NoSuchId { id })?;
        self.run(cmd, args)
    }
}

/// The error of a text argument that does not read: [`CommandError::BadArgument`] with [`SettingError::Malformed`].
fn malformed<K: sdrkit_core::record::Kind>(args: &Values, a: &Argument<K>) -> CommandError {
    match args.schema().id(a.field.key) {
        Ok(arg) => CommandError::BadArgument {
            arg,
            error: SettingError::Malformed { id: arg, at: None },
        },
        Err(_) => CommandError::NotAnArgument { key: a.field.key },
    }
}

/// The file that the argument `path` names.
///
/// # Errors
///
/// [`CommandError::BadArgument`] with [`SettingError::Malformed`] if the text is not a relative store path.
fn file_arg(args: &Values) -> Result<StorePath, CommandError> {
    StorePath::new(args.arg(&A_PATH)?).map_err(|_| malformed(args, &A_PATH))
}

/// The owner that the argument `owner` names.
///
/// # Errors
///
/// [`CommandError::BadArgument`] with [`SettingError::Malformed`] if the text is not an owner key.
fn owner_arg(args: &Values) -> Result<OwnerKey, CommandError> {
    OwnerKey::new(args.arg(&A_OWNER)?.to_owned()).map_err(|_| malformed(args, &A_OWNER))
}

fn count(n: usize) -> i64 {
    i64::try_from(n).unwrap_or(i64::MAX)
}

fn set_count(reply: Option<&mut Record>, f: &Field<field::I64>, n: usize) {
    if let Some(r) = reply {
        r.set(f, count(n));
    }
}

fn no_row(table: TableId, key: &str) -> CommandError {
    CommandError::NoSuchRow {
        table,
        key: key.to_owned(),
    }
}

fn failed(e: MemoryError) -> CommandError {
    CommandError::Failed {
        source: StageError::new(e),
    }
}

/// [`failed`], with an unknown item as [`CommandError::NoSuchRow`] of `table`.
fn failed_in(e: MemoryError, table: TableId) -> CommandError {
    match e {
        MemoryError::NoItem { id } => no_row(table, &id.to_string()),
        e => failed(e),
    }
}

/// The id in row key `row` of `table`.
///
/// # Errors
///
/// [`CommandError::NoSuchRow`] if the key is not a number or no row has it.
fn row_id(row: &str, table: TableId, exists: impl Fn(u64) -> bool) -> Result<u64, CommandError> {
    row.parse::<u64>()
        .ok()
        .filter(|id| exists(*id))
        .ok_or_else(|| no_row(table, row))
}

fn set_row(reply: Option<&mut Record>, id: u64) {
    if let Some(r) = reply {
        r.set(&R_ID, &id.to_string());
    }
}

/// The memories that query `q` takes, in its order.
fn found<'a>(memories: &'a MemoryStore, q: &Query) -> Vec<&'a Memory> {
    memories
        .search(&q.search, q.sort, q.ascending)
        .into_iter()
        .filter(|m| q.filter.matches(m))
        .collect()
}

fn query_of(args: &Values) -> Query {
    Query {
        search: args.get(&A_SEARCH).to_owned(),
        sort: args.get(&A_SORT),
        ascending: args.get(&A_ASCENDING),
        filter: match args.get(&A_FILTER) {
            FilterKind::All => MemoryFilter::All,
            FilterKind::InScan => MemoryFilter::InScan,
            FilterKind::Group => MemoryFilter::Group {
                name: args.get(&A_GROUP).to_owned(),
            },
            FilterKind::Range => MemoryFilter::Range {
                lo_hz: args.get(&A_LO),
                hi_hz: args.get(&A_HI),
            },
            FilterKind::Starred => MemoryFilter::Starred,
        },
        limit: usize::try_from(args.get(&A_LIMIT)).unwrap_or(1),
        offset: usize::try_from(args.get(&A_OFFSET)).unwrap_or(0),
    }
}

fn memory_of(args: &Values) -> Result<Memory, CommandError> {
    let opt = |v: f64| (v > 0.0).then_some(v);
    Ok(Memory {
        name: args.get(&A_NAME).to_owned(),
        freq_hz: args.arg(&A_FREQ)?,
        stack: args.get(&A_STACK).to_owned(),
        submode: args.get(&A_SUBMODE).to_owned(),
        rate_hz: opt(args.get(&A_RATE)),
        bandwidth_hz: opt(args.get(&A_BANDWIDTH)),
        group: args.get(&A_GROUP).to_owned(),
        notes: args.get(&A_NOTES).to_owned(),
        color: args.get(&A_COLOR).to_owned(),
        starred: args.get(&A_STARRED),
        scan: args.get(&A_SCAN),
        lockout: args.get(&A_LOCKOUT),
        priority: args.get(&A_PRIORITY),
        dwell_s: opt(args.get(&A_DWELL)),
        tone: args.get(&A_TONE).to_owned(),
        ..Memory::default()
    })
}

fn write_memory(r: &mut sdrkit_core::results::RowMut<'_>, m: &Memory) {
    r.set(&NAME, &m.name);
    r.set(&FREQ, m.freq_hz);
    r.set(&STACK, &m.stack);
    r.set(&SUBMODE, &m.submode);
    r.opt(&RATE, m.rate_hz);
    r.opt(&BANDWIDTH, m.bandwidth_hz);
    r.set(&GROUP, &m.group);
    r.set(&NOTES, &m.notes);
    r.set(&COLOR, &m.color);
    r.set(&STARRED, m.starred);
    r.set(&SCAN, m.scan);
    r.set(&LOCKOUT, m.lockout);
    r.set(&PRIORITY, m.priority);
    r.opt(&DWELL, m.dwell_s);
    r.set(&ORDER, i64::try_from(m.order).unwrap_or(i64::MAX));
    r.set(&CREATED, m.created_s);
    r.opt(&LAST_USED, m.last_used_s);
    r.set(&HITS, i64::try_from(m.hits).unwrap_or(i64::MAX));
    r.opt(&LAST_HIT, m.last_hit_s);
    r.set(&TONE, &m.tone);
}

/// Writes the stored settings of `of_row` into `def`, one row each, keyed `<row>/<owner>/<key>`, and notes the row keys in
/// `keep`.
fn stored(
    p: &mut Published,
    def: &TableDef,
    of: &Field<field::Text>,
    of_row: &str,
    list: &[OwnerSetting],
    keep: &mut HashSet<String>,
) {
    for s in list {
        let row = format!("{of_row}/{}/{}", s.owner.as_str(), s.key);
        if let Some(mut r) = p.row_mut(def, &row) {
            r.set(of, of_row);
            r.set(&ITEM_OWNER, s.owner.as_str());
            r.set(&ITEM_KEY, &s.key);
            r.set(&ITEM_VALUE, &s.value);
        }
        keep.insert(row);
    }
}

fn names(p: &mut Published, def: &TableDef, list: Vec<String>) {
    let mut keep = HashSet::new();
    for n in list.iter().rev() {
        if let Some(mut r) = p.row_mut(def, n) {
            r.set(&NAME, n);
        }
        keep.insert(n.clone());
    }
    prune(p, def, &keep);
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

#[cfg(test)]
mod tests;
