//! Typed result keys ([`Field`]) and the store that declares and writes them from one list ([`Published`]). The standard keys
//! are in [`keys`].

use crate::command::CommandError;
use crate::event::{Event, EventDef};
use crate::marks::{Role, RowMarks, TableMarks};

use crate::owner::OwnerKey;
use crate::record::{refuse_write, ContentType, FieldDecl, FieldId, Kind, RecordDecl, RefersTo};
use crate::results::{Results, ResultsDecl, RowMut, TableDecl};
use crate::value::{ChoiceDecl, Value, ValueKind};
use std::borrow::Cow;
use std::marker::PhantomData;
use std::sync::Arc;

/// The kinds a [`Field`] can have, as marker types.
pub mod field {
    /// A number. The unit is in the key's suffix.
    pub enum F64 {}
    /// A whole number.
    pub enum I64 {}
    /// Yes or no.
    pub enum Bool {}
    /// A frequency in Hz.
    pub enum Hz {}
    /// A time, seconds since 1970 (UTC).
    pub enum Time {}
    /// A length of time in seconds.
    pub enum Duration {}
    /// A position in degrees, latitude first.
    pub enum Position {}
    /// Text for people.
    pub enum Text {}
    /// One variant of the Rust enum `E`, held as its index (see [`crate::Choice`]).
    pub struct Choice<E>(std::marker::PhantomData<fn() -> E>);
    /// A raw payload.
    pub enum Bytes {}
    /// A value of any kind, which carries its own kind (see [`crate::ValueKind::Any`]).
    pub enum Any {}
}

pub(crate) mod sealed {
    pub trait Sealed {}
    impl Sealed for super::field::F64 {}
    impl Sealed for super::field::I64 {}
    impl Sealed for super::field::Bool {}
    impl Sealed for super::field::Hz {}
    impl Sealed for super::field::Time {}
    impl Sealed for super::field::Duration {}
    impl Sealed for super::field::Position {}
    impl Sealed for super::field::Text {}
    impl<E> Sealed for super::field::Choice<E> {}
    impl Sealed for super::field::Bytes {}
    impl Sealed for super::field::Any {}
}

/// One result field, named once: its key, label, unit and (in the type) its kind. Make it a `const` and use it in the
/// declaration ([`ResultsDecl::with_fields`]) and to write ([`Published::set`], [`crate::Record::set`]).
///
/// Rules for keys and units:
/// - A key is `snake_case` ASCII and names the quantity (`snr_db`, not `SNR (dB)`).
/// - A number of kind `F64` or `I64` has its unit as a key suffix: `_db`, `_dbfs`, `_dbm`, `_s`, `_ms`, `_ppm`, `_m`, `_km`,
///   `_kt`, `_deg`, `_pct`, `_hz_s`. A count has no suffix (`frames_ok`).
/// - The kinds `Hz` (hertz), `Time` (seconds since 1970, UTC), `Duration` (seconds) and `Position` (degrees) carry their unit,
///   so the key does not repeat it.
/// - One unit per quantity: Hz, seconds, metres and degrees.
/// - A label carries no unit. The unit is in [`FieldDecl::unit`].
/// - Use the standard keys of [`keys`] wherever they fit.
pub struct Field<K> {
    /// The key.
    pub key: &'static str,
    /// What people see.
    pub label: &'static str,
    /// The unit people see (`"dB"`, `"Hz"`). Empty for none.
    pub unit: &'static str,
    refers_to: Option<(Option<&'static str>, &'static str)>,
    refers_column: &'static str,
    content: Option<ContentType>,
    role: Option<Role>,
    reading: u8,
    kind: PhantomData<fn() -> K>,
}

impl<K> Clone for Field<K> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<K> Copy for Field<K> {}
impl<K> std::fmt::Debug for Field<K> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Field").field("key", &self.key).finish()
    }
}

impl<K: Kind> Field<K> {
    /// A field `key` shown as `label` in `unit`.
    pub const fn new(key: &'static str, label: &'static str, unit: &'static str) -> Self {
        Field {
            key,
            label,
            unit,
            refers_to: None,
            refers_column: "",
            content: None,
            role: None,
            reading: 0,
            kind: PhantomData,
        }
    }
    /// The same field as key reading number `order` of its owner: what a person looks at first. The order counts
    /// from 1, and an owner has at most [`FieldDecl::MAX_READINGS`] key readings.
    ///
    /// # Panics
    ///
    /// If `order` is 0 or above [`FieldDecl::MAX_READINGS`]. In a `const`, this stops the build.
    #[must_use]
    pub const fn reading(mut self, order: u8) -> Self {
        assert!(
            order >= 1 && order <= FieldDecl::MAX_READINGS,
            "the order of a key reading is 1 to 6"
        );
        self.reading = order;
        self
    }
    /// The same field with the role `role`: what it holds for a front end that places a row on a map.
    #[must_use]
    pub const fn role(mut self, role: Role) -> Self {
        self.role = Some(role);
        self
    }
    /// The role of the field, if it has one.
    pub const fn role_of(&self) -> Option<Role> {
        self.role
    }
    /// The same field, holding row keys of table `table` of owner `owner`.
    ///
    /// # Panics
    ///
    /// If `owner` is not a valid [`OwnerKey`]. In a `const`, this stops the build.
    #[must_use]
    pub const fn refers_to(mut self, owner: &'static str, table: &'static str) -> Self {
        assert!(OwnerKey::is_valid(owner), "not a valid owner key");
        self.refers_to = Some((Some(owner), table));
        self
    }
    /// The same field, holding row keys of table `table` of the same owner.
    #[must_use]
    pub const fn refers_to_table(mut self, table: &'static str) -> Self {
        self.refers_to = Some((None, table));
        self
    }
    /// The same field, whose value can be taken from column `column` of a row of table `table` of the same owner:
    /// the addressee of a send command from the row of a node. Other values stay valid.
    #[must_use]
    pub const fn refers_to_column(mut self, table: &'static str, column: &'static str) -> Self {
        self.refers_to = Some((None, table));
        self.refers_column = column;
        self
    }
    /// Its declaration.
    pub fn decl(&self) -> FieldDecl {
        FieldDecl {
            key: Cow::Borrowed(self.key),
            label: Cow::Borrowed(self.label),
            unit: Cow::Borrowed(self.unit),
            kind: K::VALUE_KIND,
            choices: K::choices(),
            refers_to: self.refers_to.map(|(owner, table)| RefersTo {
                owner: owner.map(OwnerKey::of),
                table: Cow::Borrowed(table),
                column: Cow::Borrowed(self.refers_column),
            }),
            content: self.content,
            reading: self.reading,
        }
    }
}

impl Field<field::Bytes> {
    /// The same field with content type `content` (a picture or a file of that type).
    #[must_use]
    pub const fn content(mut self, content: ContentType) -> Self {
        self.content = Some(content);
        self
    }
}

/// A field of any kind, for lists of declarations (`&[&dyn Declare]`).
pub trait Declare: Sync {
    /// The key.
    fn key(&self) -> &'static str;
    /// The declared kind.
    fn kind(&self) -> ValueKind;
    /// The declaration.
    fn decl(&self) -> FieldDecl;
    /// What the field holds for a front end that places a row on a map. `None` for a field without a role.
    fn role(&self) -> Option<Role> {
        None
    }
}

impl<K: Kind> Declare for Field<K> {
    fn key(&self) -> &'static str {
        self.key
    }
    fn kind(&self) -> ValueKind {
        K::VALUE_KIND
    }
    fn role(&self) -> Option<Role> {
        self.role
    }
    fn decl(&self) -> FieldDecl {
        Field::decl(self)
    }
}

/// A table named once: its key, label, typed columns and the most rows it keeps.
pub struct TableDef {
    /// The key.
    pub key: &'static str,
    /// What people see.
    pub label: &'static str,
    /// The columns, in order.
    pub columns: &'static [&'static dyn Declare],
    /// The most rows kept. The oldest row goes first.
    pub cap: usize,
    /// The column that names a row and the default order of the rows. [`RowMarks::NONE`] for a table that states
    /// neither.
    pub marks: RowMarks,
}

impl TableDef {
    /// The same table, whose rows are what `role` says ([`TableMarks::role`]). For a table constant of a shared crate.
    #[must_use]
    pub const fn with_role(mut self, role: crate::marks::TableRole) -> Self {
        self.marks.role = Some(role);
        self
    }
    /// Its declaration.
    ///
    /// # Panics
    ///
    /// If two columns share a key, or if the marks name a column that the table does not have.
    pub fn decl(&self) -> TableDecl {
        TableDecl {
            key: Cow::Borrowed(self.key),
            label: Cow::Borrowed(self.label),
            columns: Arc::new(RecordDecl::of(self.columns)),
            cap: self.cap,
            marks: TableMarks::of(self.key, self.columns, &self.marks),
        }
    }
}

impl std::fmt::Debug for TableDef {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TableDef").field("key", &self.key).finish()
    }
}

/// A choice field whose list is a catalogue that is read when the declaration is made (the field form of
/// [`CatalogueSetting`](crate::CatalogueSetting)). The value is the index of an entry in the list that `choices` returns.
#[derive(Clone, Copy, Debug)]
pub struct CatalogueField {
    /// The key.
    pub key: &'static str,
    /// What people see.
    pub label: &'static str,
    /// The entries of the catalogue, in order.
    pub choices: fn() -> Vec<ChoiceDecl>,
}

impl Declare for CatalogueField {
    fn key(&self) -> &'static str {
        self.key
    }
    fn kind(&self) -> ValueKind {
        ValueKind::Choice
    }
    fn decl(&self) -> FieldDecl {
        FieldDecl::with_choices(self.key, self.label, (self.choices)())
    }
}

/// A series named once: its key, a label, and the units of its two axes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SeriesDef {
    /// The key.
    pub key: &'static str,
    /// What people see.
    pub label: &'static str,
    /// The unit of the first value of a point.
    pub x_unit: &'static str,
    /// The unit of the second value of a point.
    pub y_unit: &'static str,
}

/// A raster named once: its key, a label, and the units of its two axes and of its cells.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RasterDef {
    /// The key.
    pub key: &'static str,
    /// What people see.
    pub label: &'static str,
    /// The unit along a row.
    pub x_unit: &'static str,
    /// The unit from row to row.
    pub y_unit: &'static str,
    /// The unit of the cells.
    pub unit: &'static str,
}

/// A receiver's results, written through its declared fields and tables only.
///
/// ```
/// use sdrkit_core::publish::{field, keys, Field, Published};
/// const STATION: Field<field::Text> = Field::new("station", "Station", "");
/// let mut p = Published::new(&[&STATION, &keys::SNR_DB]);
/// p.set(&STATION, "Triple M");
/// p.set(&keys::SNR_DB, 17.5);
/// assert_eq!(p.results().fields().get(&STATION), Some("Triple M"));
/// ```
///
/// A write to a field or table that the declaration does not have panics in debug builds. In release builds the write is
/// dropped and counted ([`crate::record::dropped_writes`]).
#[derive(Clone, Debug, PartialEq)]
pub struct Published {
    results: Results,
}

impl Published {
    /// A store that can write these fields.
    ///
    /// # Panics
    ///
    /// If two fields share a key.
    pub fn new(fields: &[&dyn Declare]) -> Self {
        Self::from_decl(Arc::new(ResultsDecl::new().with_fields(fields)))
    }
    /// A store of this declaration.
    pub fn from_decl(decl: Arc<ResultsDecl>) -> Self {
        Published {
            results: Results::new(decl),
        }
    }
    /// The same store, also able to write these tables. Field values written before stay. Rows written before are dropped.
    ///
    /// # Panics
    ///
    /// If a table key or a column key is declared twice.
    pub fn with_tables(self, tables: &[&TableDef]) -> Self {
        let mut decl = ResultsDecl::clone(self.results.decl());
        for t in tables {
            decl = decl.with_table(t);
        }
        Published {
            results: self.results.with_decl(Arc::new(decl)),
        }
    }
    /// Writes a declared field.
    pub fn set<K: Kind>(&mut self, f: &Field<K>, v: K::In<'_>) {
        self.results.fields_mut().set(f, v);
    }
    /// Marks a declared field as having no value now.
    pub fn none<K: Kind>(&mut self, f: &Field<K>) {
        self.results.fields_mut().none(f);
    }
    /// Writes a declared field, or marks it as having no value.
    pub fn opt<K: Kind>(&mut self, f: &Field<K>, v: Option<K::In<'_>>) {
        self.results.fields_mut().opt(f, v);
    }
    /// The error for a command that names row `key` of declared table `t`, which the table does not hold:
    /// [`CommandError::NoSuchRow`], or [`CommandError::NoSuchTable`] if `t` is not declared.
    pub fn no_such_row(&self, t: &TableDef, key: &str) -> CommandError {
        match self.results.decl().table_id(t.key) {
            Some(table) => CommandError::NoSuchRow {
                table,
                key: key.to_owned(),
            },
            None => CommandError::NoSuchTable { key: t.key },
        }
    }
    /// Writes the catalogue entry `index` into the declared field `f`.
    pub fn set_entry(&mut self, f: &CatalogueField, index: u16) {
        self.results.fields_mut().set_entry(f, index);
    }
    /// The row `id` of declared table `t`, made the newest, to write cell by cell. `None` (after a panic in debug builds) if
    /// `t` is not declared.
    pub fn row_mut(&mut self, t: &TableDef, id: &str) -> Option<RowMut<'_>> {
        let Some(tid) = self.results.decl().table_id(t.key) else {
            refuse_write(t.key, "a table that is not declared");
            return None;
        };
        Some(self.results.table_mut(tid)?.into_row(id))
    }
    /// Inserts or replaces row `id` of declared table `t`, one cell per column. The oldest row goes when the table is full.
    ///
    /// # Panics
    ///
    /// In debug builds, if the table is not declared, the number of cells differs from the number of columns, or a cell is of
    /// another kind than its column. In release builds such a row or cell is dropped and counted.
    pub fn row(&mut self, t: &TableDef, id: &str, cells: Vec<Value>) {
        let Some(tid) = self.results.decl().table_id(t.key) else {
            refuse_write(t.key, "a table that is not declared");
            return;
        };
        let Some(mut table) = self.results.table_mut(tid) else {
            return;
        };
        if cells.len() != table.columns().len() {
            refuse_write(t.key, "the wrong number of cells");
            return;
        }
        let mut row = table.row_mut(id);
        for (i, c) in cells.iter().enumerate() {
            let fid = FieldId(u16::try_from(i).unwrap_or(u16::MAX));
            if row.set_value(fid, c).is_err() {
                refuse_write(t.key, "a cell of another kind");
            }
        }
    }
    /// Removes row `id` of declared table `t`. Returns true if the row was there.
    pub fn remove_row(&mut self, t: &TableDef, id: &str) -> bool {
        self.results
            .decl()
            .table_id(t.key)
            .and_then(|tid| self.results.table_mut(tid))
            .is_some_and(|mut table| {
                let there = table.row(id).is_some();
                table.remove(id);
                there
            })
    }
    /// Removes every row of declared table `t` and keeps its columns.
    pub fn clear_rows(&mut self, t: &TableDef) {
        if let Some(mut table) = self
            .results
            .decl()
            .table_id(t.key)
            .and_then(|id| self.results.table_mut(id))
        {
            table.clear();
        }
    }
    /// The results.
    pub fn results(&self) -> &Results {
        &self.results
    }
    /// The results, for the status, log, series, rasters and points.
    pub fn results_mut(&mut self) -> &mut Results {
        &mut self.results
    }
}

/// A decoded message of kind `def` at signal time `t` on `freq_hz`, with its source, its text and its SNR (left out when `None`
/// or not finite). `def` must declare [`keys::SOURCE`], [`keys::TEXT`] and [`keys::SNR_DB`].
///
/// ```
/// use sdrkit_core::publish::{decoded_event, keys};
/// use sdrkit_core::EventDef;
/// static MESSAGE: EventDef = EventDef::new("acars.message", "Message", &[&keys::SOURCE, &keys::TEXT, &keys::SNR_DB]);
/// let e = decoded_event(&MESSAGE, 12.0, 131.525e6, "G-EUUA", "TEXT", Some(14.0));
/// assert!(e.is(&MESSAGE));
/// assert_eq!((e.get(&keys::SOURCE), e.get(&keys::TEXT)), (Some("G-EUUA"), Some("TEXT")));
/// assert_eq!(e.get(&keys::SNR_DB), Some(14.0));
/// ```
pub fn decoded_event(
    def: &'static EventDef,
    t: f64,
    freq_hz: f64,
    source: &str,
    text: &str,
    snr_db: Option<f64>,
) -> Event {
    def.event(t, freq_hz)
        .set(&keys::SOURCE, source)
        .set(&keys::TEXT, text)
        .opt(&keys::SNR_DB, snr_db.filter(|s| s.is_finite()))
}

/// The standard result keys, defined once for every stack and component. Use them wherever they fit, with their meaning.
pub mod keys {
    use super::{field, Field};

    /// The sender of a decoded message: a registration, an address, a call sign or a code.
    pub const SOURCE: Field<field::Text> = Field::new("source", "Source", "");
    /// The text of a decoded message.
    pub const TEXT: Field<field::Text> = Field::new("text", "Text", "");
    /// Signal to noise ratio of what is being decoded, dB (in the signal's own bandwidth, or per symbol where the stack says so).
    pub const SNR_DB: Field<field::F64> = Field::new("snr_db", "SNR", "dB");
    /// Level of the signal or channel, dBFS (a mean power of 1 is 0 dBFS).
    pub const LEVEL_DBFS: Field<field::F64> = Field::new("level_dbfs", "Level", "dBFS");
    /// The squelch of the receiver is open: its audio carries a signal. A receiver without a squelch leaves it out.
    pub const SQUELCH_OPEN: Field<field::Bool> = Field::new("squelch_open", "Squelch open", "");
    /// Noise floor, dBFS.
    pub const NOISE_DBFS: Field<field::F64> = Field::new("noise_dbfs", "Noise floor", "dBFS");
    /// Carrier offset from where it was expected, Hz.
    pub const OFFSET_HZ: Field<field::Hz> = Field::new("offset_hz", "Offset", "Hz");
    /// Frequency drift, Hz per second.
    pub const DRIFT_HZ_S: Field<field::F64> = Field::new("drift_hz_s", "Drift", "Hz/s");
    /// Sample-clock or symbol-clock error, parts per million.
    pub const CLOCK_PPM: Field<field::F64> = Field::new("clock_ppm", "Clock error", "ppm");
    /// Symbol rate found, Hz.
    pub const SYMBOL_RATE_HZ: Field<field::Hz> = Field::new("symbol_rate_hz", "Symbol rate", "Hz");
    /// Frames (packets, blocks, bursts) that passed their check.
    pub const FRAMES_OK: Field<field::I64> = Field::new("frames_ok", "Frames good", "");
    /// Frames that failed their check.
    pub const FRAMES_BAD: Field<field::I64> = Field::new("frames_bad", "Frames bad", "");
    /// Bits corrected by error correction since the start.
    pub const BITS_CORRECTED: Field<field::I64> =
        Field::new("bits_corrected", "Bits corrected", "");
    /// Bit error rate before error correction (0 to 1), measured by re-encoding.
    pub const BER: Field<field::F64> = Field::new("ber", "Bit error rate", "");
    /// Modulation error ratio, dB.
    pub const MER_DB: Field<field::F64> = Field::new("mer_db", "MER", "dB");
    /// Error vector magnitude, percent.
    pub const EVM_PCT: Field<field::F64> = Field::new("evm_pct", "EVM", "%");
    /// The receiver holds lock (symbol, carrier and frame sync as the stack defines it).
    pub const LOCKED: Field<field::Bool> = Field::new("locked", "Locked", "");
    /// Time the last good frame was received.
    pub const LAST_FRAME: Field<field::Time> = Field::new("last_frame", "Last frame", "");
    /// Time since lock was gained, seconds.
    pub const LOCKED_FOR: Field<field::Duration> = Field::new("locked_for", "Locked for", "s");
    /// Input samples replaced because they were not finite or far beyond full scale.
    pub const BAD_SAMPLES: Field<field::I64> = Field::new("bad_samples", "Bad samples", "");
    /// Input items dropped because a helper thread did not keep up.
    pub const DROPPED: Field<field::I64> = Field::new("dropped", "Dropped", "");

    /// The call sign of a station heard.
    pub const CALLSIGN: Field<field::Text> = Field::new("callsign", "Callsign", "");
    /// The Maidenhead locator of a station heard.
    pub const LOCATOR: Field<field::Text> = Field::new("locator", "Locator", "");
    /// The frequency a station was heard on.
    pub const FREQ_HZ: Field<field::Hz> = Field::new("freq_hz", "Frequency", "Hz");
    /// The emission type of a spot (`"FT8"`, `"CW"`). The key is "mode", the word that spot networks use for it.
    pub const SPOT_MODE: Field<field::Text> = Field::new("mode", "Mode", "");

    /// How urgent an event is. A stack sets it when the decoded message is urgent. Services alert on it.
    pub const URGENCY: Field<field::Choice<Urgency>> = Field::new("urgency", "Urgency", "");
    /// What an event is about (aircraft, vessel, pager). Outputs and the store filter on it.
    pub const FAMILY: Field<field::Choice<Family>> = Field::new("family", "Family", "");
    /// The content of the event is private (voice, encrypted traffic). Services never read its words or keep its text.
    pub const PRIVATE: Field<field::Bool> = Field::new("private", "Private", "");
    /// The position of the object that the event is about.
    pub const POSITION: Field<field::Position> = Field::new("position", "Position", "");
    /// The height of the object that the event is about, metres above mean sea level.
    pub const HEIGHT_M: Field<field::F64> = Field::new("height_m", "Height", "m").role(crate::Role::Altitude);
    /// The speed over the ground of the object that the event is about, knots.
    pub const SPEED_KT: Field<field::F64> = Field::new("speed_kt", "Speed", "kt").role(crate::Role::Speed);
    /// The course over the ground of the object that the event is about, degrees clockwise from true north.
    pub const COURSE_DEG: Field<field::F64> = Field::new("course_deg", "Course", "°").role(crate::Role::Course);
    /// The receiver of a message or call: an address, a call sign or a talk group member.
    pub const TARGET: Field<field::Text> = Field::new("target", "Target", "");
    /// The channel or talk group of a call.
    pub const CHANNEL: Field<field::Text> = Field::new("channel", "Channel", "");

    crate::choice_enum! {
        /// How urgent an event is, from no urgency to an emergency.
        pub enum Urgency {
            /// Nothing urgent.
            None = ("none", "None"),
            /// Worth a note to people (a weather warning).
            Notice = ("notice", "Notice"),
            /// Urgent (an urgency call, an alarm announcement).
            Urgent = ("urgent", "Urgent"),
            /// An emergency (distress, a hijack code).
            Emergency = ("emergency", "Emergency"),
        }
    }

    crate::choice_enum! {
        /// What an event is about.
        pub enum Family {
            /// Aircraft.
            Aircraft = ("aircraft", "Aircraft"),
            /// Ships and coast stations.
            Vessel = ("vessel", "Vessel"),
            /// APRS stations and packets.
            Aprs = ("aprs", "APRS"),
            /// Pagers.
            Pager = ("pager", "Pager"),
            /// Radiosondes.
            Sonde = ("sonde", "Radiosonde"),
            /// Broadcast radio and TV.
            Broadcast = ("broadcast", "Broadcast"),
            /// Voice traffic.
            Voice = ("voice", "Voice"),
            /// Data messages that are not in another family.
            Message = ("message", "Message"),
            /// Sensors and devices.
            Sensor = ("sensor", "Sensor"),
            /// Satellites.
            Satellite = ("satellite", "Satellite"),
            /// The receiver itself (tuning, scans, sync lost).
            Receiver = ("receiver", "Receiver"),
            /// Distress beacons and other alarms.
            Alert = ("alert", "Alert"),
            /// An event that does not say its family.
            Other = ("other", "Other"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn standard_keys_follow_the_unit_rules() {
        let all: [&dyn Declare; 25] = [
            &keys::SNR_DB,
            &keys::LEVEL_DBFS,
            &keys::NOISE_DBFS,
            &keys::OFFSET_HZ,
            &keys::DRIFT_HZ_S,
            &keys::CLOCK_PPM,
            &keys::SYMBOL_RATE_HZ,
            &keys::FRAMES_OK,
            &keys::FRAMES_BAD,
            &keys::BITS_CORRECTED,
            &keys::BER,
            &keys::MER_DB,
            &keys::EVM_PCT,
            &keys::LOCKED,
            &keys::LAST_FRAME,
            &keys::LOCKED_FOR,
            &keys::BAD_SAMPLES,
            &keys::DROPPED,
            &keys::CALLSIGN,
            &keys::LOCATOR,
            &keys::FREQ_HZ,
            &keys::SPOT_MODE,
            &keys::HEIGHT_M,
            &keys::SPEED_KT,
            &keys::COURSE_DEG,
        ];
        let mut seen = std::collections::BTreeSet::new();
        for f in all {
            let d = f.decl();
            assert!(seen.insert(d.key.clone()), "{} twice", d.key);
            assert!(
                d.key
                    .bytes()
                    .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_'),
                "{}",
                d.key
            );
            // A number with a unit says it in its key.
            if d.kind == ValueKind::F64 && !d.unit.is_empty() {
                let suffix = match d.unit.as_ref() {
                    "dB" => "_db",
                    "dBFS" => "_dbfs",
                    "ppm" => "_ppm",
                    "%" => "_pct",
                    "Hz/s" => "_hz_s",
                    "m" => "_m",
                    "kt" => "_kt",
                    "°" => "_deg",
                    u => panic!("unit {u} has no suffix rule"),
                };
                assert!(d.key.ends_with(suffix), "{} in {}", d.key, d.unit);
            }
        }
    }

    #[test]
    fn a_missing_row_names_its_table() {
        const NAME: Field<field::Text> = Field::new("name", "Name", "");
        static LIST: TableDef = TableDef {
            key: "list",
            label: "List",
            columns: &[&NAME],
            cap: 4,
            marks: RowMarks::NONE,
        };
        static OTHER: TableDef = TableDef {
            key: "other",
            label: "Other",
            columns: &[&NAME],
            cap: 4,
            marks: RowMarks::NONE,
        };
        let p = Published::from_decl(Arc::new(ResultsDecl::new().with_table(&LIST)));
        assert_eq!(
            p.no_such_row(&LIST, "7"),
            CommandError::NoSuchRow {
                table: crate::TableId(0),
                key: "7".into()
            }
        );
        assert_eq!(
            p.no_such_row(&OTHER, "7"),
            CommandError::NoSuchTable { key: "other" }
        );
    }
}
