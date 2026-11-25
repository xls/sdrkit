//! The results of an object: their declaration, and the snapshot of what changed after a change counter.

use crate::error::Error;
use crate::handle::{run, str_in, Handle, Out};
use crate::value::{count, small, Id, Str, Variant, SDRKIT_NO_ID};
use sdrkit_core::{
    FieldId, LockState, MapPoint, RasterId, Record, ResultsDecl, SeriesId, Status, TableId,
};
use sdrkit_host_engine::EngineError;
use std::ffi::c_char;
use std::sync::Arc;

/// The object does not run.
pub const SDRKIT_STATE_OFF: i32 = 0;
/// The object looks for a signal.
pub const SDRKIT_STATE_SEARCHING: i32 = 1;
/// The object has a signal and does not decode it yet.
pub const SDRKIT_STATE_TRACKING: i32 = 2;
/// The object decodes the signal.
pub const SDRKIT_STATE_LOCKED: i32 = 3;

/// The size of the results of an object.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct ResultsInfo {
    /// The number of bytes of the struct. The caller sets it to the sizeof of its struct before the call; the call sets it to
    /// the number of bytes that it wrote.
    pub size: u32,
    /// The number of result fields. Field ids go from 0 to `fields` - 1.
    pub fields: u32,
    /// The number of tables.
    pub tables: u32,
    /// The number of series.
    pub series: u32,
    /// The number of rasters.
    pub rasters: u32,
    /// The label of the map points, or empty if the object has none.
    pub points_label: Str,
}

/// The declaration of one table.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct TableInfo {
    /// The number of bytes of the struct. The caller sets it to the sizeof of its struct before the call; the call sets it to
    /// the number of bytes that it wrote.
    pub size: u32,
    /// The key.
    pub key: Str,
    /// The name for people.
    pub label: Str,
    /// The number of columns. Column ids go from 0 to `columns` - 1.
    pub columns: u32,
    /// The most rows that the table keeps. When the table is full, a new row removes the oldest row.
    pub cap: usize,
    /// The id of the column that names a row for people (a call sign, a station name), or `SDRKIT_NO_ID`.
    pub label_column: Id,
    /// The id of the column that orders the rows by default, or `SDRKIT_NO_ID`: the newest row is first. It is a hint
    /// for a front end. The rows of a snapshot are always newest first.
    pub sort_column: Id,
    /// With `sort_column`: true for the highest value first, false for the lowest value first.
    pub sort_descending: bool,
    /// The id of the column of type `SDRKIT_TYPE_POSITION` that holds the place of a row on the map, or
    /// `SDRKIT_NO_ID`.
    pub position_column: Id,
    /// The id of the column of the latitude in degrees north, for a table without a position column, or
    /// `SDRKIT_NO_ID`.
    pub latitude_column: Id,
    /// The id of the column of the longitude in degrees east, for a table without a position column, or
    /// `SDRKIT_NO_ID`.
    pub longitude_column: Id,
    /// The id of the column of the height, or `SDRKIT_NO_ID`. The declaration of the column has the unit.
    pub altitude_column: Id,
    /// The id of the column of the direction of travel in degrees from north, or `SDRKIT_NO_ID`.
    pub course_column: Id,
    /// The id of the column of the speed, or `SDRKIT_NO_ID`. The declaration of the column has the unit.
    pub speed_column: Id,
    /// An `SDRKIT_TABLE_ROLE_*` constant: what the rows are. A front end finds the message log, the call log, the
    /// nodes and the groups of an object by this member and not by the key of the table.
    pub role: i32,
    /// The id of the column of the sender of a message or a call, or `SDRKIT_NO_ID`.
    pub sender_column: Id,
    /// The id of the column of the addressee of a message or a call, or `SDRKIT_NO_ID`.
    pub addressee_column: Id,
    /// The id of the column of the text of a message, or `SDRKIT_NO_ID`.
    pub text_column: Id,
    /// The id of the column that holds the time of a row, or `SDRKIT_NO_ID`: a column of type `SDRKIT_TYPE_TIME`, or
    /// a number of seconds of signal with the unit `s`. Only a table with a role, or with a column that states it,
    /// has it.
    pub time_column: Id,
    /// The id of the column of type `SDRKIT_TYPE_BOOL` that is true for a message that this station sent, or
    /// `SDRKIT_NO_ID`. Without it, in a table with a `delivery_column`, a row with a delivery state is an own message.
    pub own_column: Id,
    /// The id of the column of the delivery state of an own message, or `SDRKIT_NO_ID`: a column of type
    /// `SDRKIT_TYPE_ENUM` that has no value in the row of a received message.
    pub delivery_column: Id,
    /// The id of the column of the reason of a failed delivery, as text for people, or `SDRKIT_NO_ID`.
    pub failure_column: Id,
}

/// The table states nothing about its rows.
pub const SDRKIT_TABLE_ROLE_NONE: i32 = 0;
/// Each row is a message: the main log of messages of the object.
pub const SDRKIT_TABLE_ROLE_MESSAGES: i32 = 1;
/// Each row is a call or a transmission: who, to whom, when.
pub const SDRKIT_TABLE_ROLE_CALLS: i32 = 2;
/// Each row is a station, a node or a unit that was heard.
pub const SDRKIT_TABLE_ROLE_NODES: i32 = 3;
/// Each row is a group that stations talk in: a talkgroup, a channel, a net.
pub const SDRKIT_TABLE_ROLE_GROUPS: i32 = 4;
/// Each row is a frame, a packet or a report that arrived: a running log. An object can have several.
pub const SDRKIT_TABLE_ROLE_LOG: i32 = 5;

/// The `SDRKIT_TABLE_ROLE_*` code of `r`.
fn table_role(r: Option<sdrkit_core::TableRole>) -> i32 {
    use sdrkit_core::TableRole as R;
    match r {
        None => SDRKIT_TABLE_ROLE_NONE,
        Some(R::Messages) => SDRKIT_TABLE_ROLE_MESSAGES,
        Some(R::Calls) => SDRKIT_TABLE_ROLE_CALLS,
        Some(R::Nodes) => SDRKIT_TABLE_ROLE_NODES,
        Some(R::Groups) => SDRKIT_TABLE_ROLE_GROUPS,
        Some(R::Log) => SDRKIT_TABLE_ROLE_LOG,
    }
}

/// The declaration of one series (a small plot) or raster (an image of values, such as a waterfall).
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct PlotInfo {
    /// The number of bytes of the struct. The caller sets it to the sizeof of its struct before the call; the call sets it to
    /// the number of bytes that it wrote.
    pub size: u32,
    /// The key.
    pub key: Str,
    /// The name for people.
    pub label: Str,
    /// The unit of the x axis.
    pub x_unit: Str,
    /// The unit of the y axis.
    pub y_unit: Str,
    /// The unit of the values of a raster, or empty for a series.
    pub unit: Str,
}

/// The snapshot state that `sdrkit_snapshot_read` writes.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct SnapshotInfo {
    /// The number of bytes of the struct. The caller sets it to the sizeof of its struct before the call; the call sets it to
    /// the number of bytes that it wrote.
    pub size: u32,
    /// The change counter of the results. Pass it as `since` to the next `sdrkit_snapshot_read`.
    pub counter: u64,
    /// The declaration of the results changed: read it again. The snapshot holds everything, as for `since` 0.
    pub declaration_changed: bool,
    /// A result field changed after `since`. The snapshot holds every field.
    pub fields_changed: bool,
    /// An `SDRKIT_STATE_*` constant.
    pub state: i32,
    /// The signal to noise ratio in dB, or NaN if not known.
    pub snr_db: f32,
    /// The level in dB full scale, or NaN if not known.
    pub level_dbfs: f32,
    /// The offset from the tuned frequency in Hz, or NaN if not known.
    pub offset_hz: f64,
    /// One line for people.
    pub summary: Str,
    /// The number of log lines in the snapshot.
    pub log_lines: u32,
    /// The number of map points in the snapshot.
    pub points: u32,
}

/// The rows of one table in the snapshot.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct RowsInfo {
    /// The number of bytes of the struct. The caller sets it to the sizeof of its struct before the call; the call sets it to
    /// the number of bytes that it wrote.
    pub size: u32,
    /// The number of rows that changed after `since`, newest first. A new row counts as changed.
    pub rows: u32,
    /// The number of rows in the table.
    pub total: u32,
    /// The change counter of the table.
    pub counter: u64,
}

/// A point on a map.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct Point {
    /// The number of bytes of the struct. The caller sets it to the sizeof of its struct before the call; the call sets it to
    /// the number of bytes that it wrote.
    pub size: u32,
    /// The id of the point.
    pub id: Str,
    /// Degrees north.
    pub lat: f64,
    /// Degrees east.
    pub lon: f64,
    /// The name for people.
    pub label: Str,
    /// Degrees from north for a moving target, or NaN.
    pub heading: f32,
    /// The id of the table that holds the row of the point, or `SDRKIT_NO_ID` for a point without a row.
    pub table: Id,
    /// The key of that row, or empty. The row can be absent from the table: a table drops its oldest rows.
    pub row: Str,
}

/// The points of a series.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct SeriesInfo {
    /// The number of bytes of the struct. The caller sets it to the sizeof of its struct before the call; the call sets it to
    /// the number of bytes that it wrote.
    pub size: u32,
    /// The series changed after `since`, and `points` holds it.
    pub changed: bool,
    /// The change counter of the series.
    pub counter: u64,
    /// `count` points, each x then y.
    pub points: *const f32,
    /// The number of points.
    pub count: usize,
    /// True if the points make a line, false if they are separate points.
    pub line: bool,
}

/// The values of a raster.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct RasterInfo {
    /// The number of bytes of the struct. The caller sets it to the sizeof of its struct before the call; the call sets it to
    /// the number of bytes that it wrote.
    pub size: u32,
    /// The raster changed after `since`, and `values` holds it.
    pub changed: bool,
    /// The change counter of the raster.
    pub counter: u64,
    /// `width` times `height` values, row by row.
    pub values: *const f32,
    /// The number of cells in a row.
    pub width: usize,
    /// The number of rows.
    pub height: usize,
    /// The x value at the centre of the first cell of a row, in the x unit of the raster.
    pub x_start: f64,
    /// The change of x from one cell to the next. It can be negative.
    pub x_step: f64,
    /// The y value at the centre of the first row, in the y unit of the raster.
    pub y_start: f64,
    /// The change of y from one row to the next. It can be negative.
    pub y_step: f64,
}

#[derive(Default)]
struct TableView {
    rows: Vec<(String, Record)>,
    used: usize,
    total: usize,
    counter: u64,
}

impl TableView {
    fn push(&mut self, key: &str, rec: &Record) {
        match self.rows.get_mut(self.used) {
            Some((k, r)) => {
                k.clear();
                k.push_str(key);
                r.clone_from(rec);
            }
            None => self.rows.push((key.to_owned(), rec.clone())),
        }
        self.used += 1;
    }
}

/// The results snapshot of one handle.
#[derive(Default)]
pub(crate) struct View {
    /// The declaration that the last declaration call read.
    held: Option<Arc<ResultsDecl>>,
    /// The declaration of the snapshot.
    decl: Option<Arc<ResultsDecl>>,
    status: Status,
    fields: Option<Record>,
    tables: Vec<TableView>,
    log: Vec<String>,
    log_used: usize,
    points: Vec<MapPoint>,
    points_used: usize,
    series: Vec<[f32; 2]>,
    raster: Vec<f32>,
}

impl View {
    /// The result fields of the snapshot. `None` if no snapshot was taken.
    pub(crate) fn fields(&self) -> Option<&Record> {
        self.fields.as_ref()
    }

    /// The table `key` of the snapshot. `None` if no snapshot was taken or it has no such table.
    pub(crate) fn changed_rows(&self, key: &str) -> Option<ChangedRows<'_>> {
        let decl = self.decl.as_ref()?;
        let id = decl.table_id(key)?;
        let t = self.tables.get(usize::from(id.0))?;
        Some(ChangedRows {
            columns: &decl.table(id)?.columns,
            rows: t.rows.get(..t.used)?,
            total: t.total,
            counter: t.counter,
        })
    }
}

/// One table of a snapshot.
pub(crate) struct ChangedRows<'a> {
    /// The columns.
    pub columns: &'a std::sync::Arc<sdrkit_core::RecordDecl>,
    /// The changed rows, newest first, each with its key.
    pub rows: &'a [(String, Record)],
    /// The number of rows in the table.
    pub total: usize,
    /// The change counter of the table.
    pub counter: u64,
}

fn copy_line(list: &mut Vec<String>, used: &mut usize, line: &str) {
    match list.get_mut(*used) {
        Some(s) => {
            s.clear();
            s.push_str(line);
        }
        None => list.push(line.to_owned()),
    }
    *used += 1;
}

/// The id of column `c`, or `SDRKIT_NO_ID` for none.
fn column(c: Option<u16>) -> Id {
    c.map_or(SDRKIT_NO_ID, Id::from)
}

fn copy_point(list: &mut Vec<MapPoint>, used: &mut usize, p: &MapPoint) {
    match list.get_mut(*used) {
        Some(d) => d.clone_from(p),
        None => list.push(p.clone()),
    }
    *used += 1;
}

fn state(l: LockState) -> i32 {
    match l {
        LockState::Off => SDRKIT_STATE_OFF,
        LockState::Searching => SDRKIT_STATE_SEARCHING,
        LockState::Tracking => SDRKIT_STATE_TRACKING,
        LockState::Locked => SDRKIT_STATE_LOCKED,
    }
}

fn no_key(key: &str) -> Error {
    Error::Engine(EngineError::NoSuchKey {
        key: key.to_owned(),
    })
}

/// Runs `f` on the results declaration of the object of `h`, which the handle keeps for the views that `f` returns.
fn with_decl(h: Handle, f: impl FnOnce(&ResultsDecl) -> Result<(), Error>) -> i32 {
    run(h, |e| {
        let d = e.owner(|o| Arc::clone(o.results_decl()))?;
        let mut st = e.state();
        f(st.results.held.insert(d))
    })
}

/// Fills `info` with the size of the results of the object of `handle`.
/// The strings of `info` are views that the library owns, valid until the next call on `handle`. Copy what you keep.
#[no_mangle]
pub unsafe extern "C" fn sdrkit_results_decl(handle: Handle, info: *mut ResultsInfo) -> i32 {
    // SAFETY: the caller's contract.
    let out = unsafe { Out::new(info) };
    with_decl(handle, |d| {
        out?.put(ResultsInfo {
            size: 0,
            fields: count(d.fields().len()),
            tables: count(d.tables().len()),
            series: count(d.series_decls().len()),
            rasters: count(d.rasters().len()),
            points_label: Str::of(d.points_label()),
        });
        Ok(())
    })
}

/// Writes the id of the table with the key `key` to `id`. Returns `SDRKIT_E_NO_SUCH_KEY` if no table has the key.
#[no_mangle]
pub unsafe extern "C" fn sdrkit_table_find(handle: Handle, key: *const c_char, id: *mut Id) -> i32 {
    // SAFETY: the caller's contract.
    let (key, out) = unsafe { (str_in(key), Out::new(id)) };
    with_decl(handle, |d| {
        let key = key?;
        let t = d.table_id(key).ok_or_else(|| no_key(key))?;
        out?.put(Id::from(t.0));
        Ok(())
    })
}

/// Fills `info` with the declaration of table `id`. Returns `SDRKIT_E_NO_SUCH_ID` if the table does not exist.
/// The strings of `info` are views that the library owns, valid until the next call on `handle`. Copy what you keep.
#[no_mangle]
pub unsafe extern "C" fn sdrkit_table_decl(handle: Handle, id: Id, info: *mut TableInfo) -> i32 {
    // SAFETY: the caller's contract.
    let out = unsafe { Out::new(info) };
    with_decl(handle, |d| {
        let t = d.table(TableId(small(id)?)).ok_or(Error::NoSuchId)?;
        let m = &t.marks;
        out?.put(TableInfo {
            size: 0,
            key: Str::of(&t.key),
            label: Str::of(&t.label),
            columns: count(t.columns.len()),
            cap: t.cap,
            label_column: column(m.label),
            sort_column: column(m.sort.map(|(c, _)| c)),
            sort_descending: matches!(m.sort, Some((_, sdrkit_core::SortOrder::Descending))),
            position_column: column(m.position),
            latitude_column: column(m.latitude),
            longitude_column: column(m.longitude),
            altitude_column: column(m.altitude),
            course_column: column(m.course),
            speed_column: column(m.speed),
            role: table_role(m.role),
            sender_column: column(m.sender),
            addressee_column: column(m.addressee),
            text_column: column(m.text),
            time_column: column(m.time),
            own_column: column(m.own),
            delivery_column: column(m.delivery),
            failure_column: column(m.failure),
        });
        Ok(())
    })
}

/// Writes the id of the series with the key `key` to `id`. Returns `SDRKIT_E_NO_SUCH_KEY` if no series has the key.
#[no_mangle]
pub unsafe extern "C" fn sdrkit_series_find(
    handle: Handle,
    key: *const c_char,
    id: *mut Id,
) -> i32 {
    // SAFETY: the caller's contract.
    let (key, out) = unsafe { (str_in(key), Out::new(id)) };
    with_decl(handle, |d| {
        let key = key?;
        let s = d.series_id(key).ok_or_else(|| no_key(key))?;
        out?.put(Id::from(s.0));
        Ok(())
    })
}

/// Fills `info` with the declaration of series `id`. Returns `SDRKIT_E_NO_SUCH_ID` if the series does not exist.
/// The strings of `info` are views that the library owns, valid until the next call on `handle`. Copy what you keep.
#[no_mangle]
pub unsafe extern "C" fn sdrkit_series_decl(handle: Handle, id: Id, info: *mut PlotInfo) -> i32 {
    // SAFETY: the caller's contract.
    let out = unsafe { Out::new(info) };
    with_decl(handle, |d| {
        let i = usize::try_from(id).map_err(|_| Error::NoSuchId)?;
        let s = d.series_decls().get(i).ok_or(Error::NoSuchId)?;
        out?.put(PlotInfo {
            size: 0,
            key: Str::of(&s.key),
            label: Str::of(&s.label),
            x_unit: Str::of(&s.x_unit),
            y_unit: Str::of(&s.y_unit),
            unit: Str::of(""),
        });
        Ok(())
    })
}

/// Writes the id of the raster with the key `key` to `id`. Returns `SDRKIT_E_NO_SUCH_KEY` if no raster has the key.
#[no_mangle]
pub unsafe extern "C" fn sdrkit_raster_find(
    handle: Handle,
    key: *const c_char,
    id: *mut Id,
) -> i32 {
    // SAFETY: the caller's contract.
    let (key, out) = unsafe { (str_in(key), Out::new(id)) };
    with_decl(handle, |d| {
        let key = key?;
        let r = d.raster_id(key).ok_or_else(|| no_key(key))?;
        out?.put(Id::from(r.0));
        Ok(())
    })
}

/// Fills `info` with the declaration of raster `id`. Returns `SDRKIT_E_NO_SUCH_ID` if the raster does not exist.
/// The strings of `info` are views that the library owns, valid until the next call on `handle`. Copy what you keep.
#[no_mangle]
pub unsafe extern "C" fn sdrkit_raster_decl(handle: Handle, id: Id, info: *mut PlotInfo) -> i32 {
    // SAFETY: the caller's contract.
    let out = unsafe { Out::new(info) };
    with_decl(handle, |d| {
        let i = usize::try_from(id).map_err(|_| Error::NoSuchId)?;
        let r = d.rasters().get(i).ok_or(Error::NoSuchId)?;
        out?.put(PlotInfo {
            size: 0,
            key: Str::of(&r.key),
            label: Str::of(&r.label),
            x_unit: Str::of(&r.x_unit),
            y_unit: Str::of(&r.y_unit),
            unit: Str::of(&r.unit),
        });
        Ok(())
    })
}

/// Takes a snapshot of the results of the object of `handle` and fills `info`. The snapshot holds the state, the log, the
/// map points, every result field if one changed after the change counter `since`, and the rows of each table that changed
/// after `since`. Pass 0 to take everything, and `info->counter` on the next call. If a table in your copy holds more rows
/// than `total` after you merge the changed rows, rows were removed: take a snapshot with `since` 0.
/// The strings of `info` are views that the library owns, valid until the next call on `handle`. Copy what you keep.
#[no_mangle]
pub unsafe extern "C" fn sdrkit_snapshot_read(
    handle: Handle,
    since: u64,
    info: *mut SnapshotInfo,
) -> i32 {
    // SAFETY: the caller's contract.
    let out = unsafe { Out::new(info) };
    run(handle, |e| {
        let out = out?;
        let mut st = e.state();
        let v = &mut st.results;
        let (counter, declaration_changed, fields_changed) = e.owner(|o| {
            let res = o.results();
            let new_decl = !v.decl.as_ref().is_some_and(|d| Arc::ptr_eq(d, res.decl()));
            let since = if new_decl { 0 } else { since };
            if new_decl {
                v.decl = Some(Arc::clone(res.decl()));
                v.fields = None;
                v.tables.clear();
                v.tables
                    .resize_with(res.decl().tables().len(), Default::default);
            }
            v.status.clone_from(res.status());
            let fields_changed = new_decl || res.fields_counter() > since;
            if fields_changed {
                match &mut v.fields {
                    Some(f) => f.clone_from(res.fields()),
                    None => v.fields = Some(res.fields().clone()),
                }
            }
            for (i, tv) in v.tables.iter_mut().enumerate() {
                tv.used = 0;
                let Some(t) = u16::try_from(i).ok().and_then(|i| res.table(TableId(i))) else {
                    continue;
                };
                tv.total = t.len();
                tv.counter = t.change_counter();
                if tv.counter > since {
                    for (k, r) in t.rows_changed_since(since) {
                        tv.push(k, r);
                    }
                }
            }
            if new_decl || res.change_counter() > since {
                v.log_used = 0;
                for line in res.log_lines() {
                    copy_line(&mut v.log, &mut v.log_used, line);
                }
                v.points_used = 0;
                for p in res.points().values() {
                    copy_point(&mut v.points, &mut v.points_used, p);
                }
            }
            (res.change_counter(), new_decl, fields_changed)
        })?;
        let s = &v.status;
        out.put(SnapshotInfo {
            size: 0,
            counter,
            declaration_changed,
            fields_changed,
            state: state(s.lock_state),
            snr_db: s.snr_db.unwrap_or(f32::NAN),
            level_dbfs: s.level_dbfs.unwrap_or(f32::NAN),
            offset_hz: s.offset_hz.unwrap_or(f64::NAN),
            summary: Str::of(&s.summary),
            log_lines: count(v.log_used),
            points: count(v.points_used),
        });
        Ok(())
    })
}

/// Writes result field `id` of the snapshot to `value`. Returns `SDRKIT_E_NO_SUCH_ID` if the field does not exist or no
/// snapshot was taken.
/// A string or bytes in `value` is a view that the library owns, valid until the next call on `handle`. Copy what you keep.
/// For a result set that a family header declares, the typed function (`sdrkit_<stack>_fields_read`) is the first choice.
#[no_mangle]
pub unsafe extern "C" fn sdrkit_snapshot_field(handle: Handle, id: Id, value: *mut Variant) -> i32 {
    // SAFETY: the caller's contract.
    let out = unsafe { Out::new(value) };
    run(handle, |e| {
        let out = out?;
        let id = FieldId(small(id)?);
        let st = e.state();
        let v = st
            .results
            .fields
            .as_ref()
            .and_then(|f| f.value(id))
            .ok_or(Error::NoSuchId)?;
        out.put(Variant::of(v));
        Ok(())
    })
}

/// Fills `rows` with the rows of table `table` in the snapshot. Returns `SDRKIT_E_NO_SUCH_ID` if the table does not exist.
#[no_mangle]
pub unsafe extern "C" fn sdrkit_snapshot_table(
    handle: Handle,
    table: Id,
    rows: *mut RowsInfo,
) -> i32 {
    // SAFETY: the caller's contract.
    let out = unsafe { Out::new(rows) };
    run(handle, |e| {
        let out = out?;
        let st = e.state();
        let t = table_view(&st.results, table)?;
        out.put(RowsInfo {
            size: 0,
            rows: count(t.used),
            total: count(t.total),
            counter: t.counter,
        });
        Ok(())
    })
}

fn table_view(v: &View, table: Id) -> Result<&TableView, Error> {
    usize::try_from(table)
        .ok()
        .and_then(|i| v.tables.get(i))
        .ok_or(Error::NoSuchId)
}

fn row_of(v: &View, table: Id, n: u32) -> Result<&(String, Record), Error> {
    let t = table_view(v, table)?;
    usize::try_from(n)
        .ok()
        .filter(|&r| r < t.used)
        .and_then(|r| t.rows.get(r))
        .ok_or(Error::NoSuchId)
}

/// Writes the key of changed row `n` of table `table` in the snapshot to `key`. Returns `SDRKIT_E_NO_SUCH_ID` if the table
/// or the row does not exist.
/// The string is a view that the library owns, valid until the next call on `handle`. Copy what you keep.
#[no_mangle]
pub unsafe extern "C" fn sdrkit_snapshot_row(
    handle: Handle,
    table: Id,
    n: u32,
    key: *mut Str,
) -> i32 {
    // SAFETY: the caller's contract.
    let out = unsafe { Out::new(key) };
    run(handle, |e| {
        let out = out?;
        let st = e.state();
        out.put(Str::of(&row_of(&st.results, table, n)?.0));
        Ok(())
    })
}

/// Writes column `column` of changed row `n` of table `table` in the snapshot to `value`. Returns `SDRKIT_E_NO_SUCH_ID` if
/// the table, the row or the column does not exist.
/// A string or bytes in `value` is a view that the library owns, valid until the next call on `handle`. Copy what you keep.
/// For a table that a family header declares, the typed function (`sdrkit_<stack>_<table>_rows`) is the first choice.
#[no_mangle]
pub unsafe extern "C" fn sdrkit_snapshot_cell(
    handle: Handle,
    table: Id,
    n: u32,
    column: Id,
    value: *mut Variant,
) -> i32 {
    // SAFETY: the caller's contract.
    let out = unsafe { Out::new(value) };
    run(handle, |e| {
        let out = out?;
        let col = FieldId(small(column)?);
        let st = e.state();
        let v = row_of(&st.results, table, n)?
            .1
            .value(col)
            .ok_or(Error::NoSuchId)?;
        out.put(Variant::of(v));
        Ok(())
    })
}

/// Writes log line `n` of the snapshot to `line`. Line 0 is the oldest. Returns `SDRKIT_E_NO_SUCH_ID` if the line does not
/// exist.
/// The string is a view that the library owns, valid until the next call on `handle`. Copy what you keep.
#[no_mangle]
pub unsafe extern "C" fn sdrkit_snapshot_log(handle: Handle, n: u32, line: *mut Str) -> i32 {
    // SAFETY: the caller's contract.
    let out = unsafe { Out::new(line) };
    run(handle, |e| {
        let out = out?;
        let st = e.state();
        let v = &st.results;
        let l = usize::try_from(n)
            .ok()
            .filter(|&l| l < v.log_used)
            .and_then(|l| v.log.get(l))
            .ok_or(Error::NoSuchId)?;
        out.put(Str::of(l));
        Ok(())
    })
}

/// Fills `point` with map point `n` of the snapshot. Returns `SDRKIT_E_NO_SUCH_ID` if the point does not exist.
/// The strings of `point` are views that the library owns, valid until the next call on `handle`. Copy what you keep.
#[no_mangle]
pub unsafe extern "C" fn sdrkit_snapshot_point(handle: Handle, n: u32, point: *mut Point) -> i32 {
    // SAFETY: the caller's contract.
    let out = unsafe { Out::new(point) };
    run(handle, |e| {
        let out = out?;
        let st = e.state();
        let v = &st.results;
        let p = usize::try_from(n)
            .ok()
            .filter(|&i| i < v.points_used)
            .and_then(|i| v.points.get(i))
            .ok_or(Error::NoSuchId)?;
        out.put(Point {
            size: 0,
            id: Str::of(&p.id),
            lat: p.lat,
            lon: p.lon,
            label: Str::of(&p.label),
            heading: p.heading.unwrap_or(f32::NAN),
            table: p.row.as_ref().map_or(SDRKIT_NO_ID, |r| Id::from(r.table.0)),
            row: p.row.as_ref().map_or(Str::of(""), |r| Str::of(&r.key)),
        });
        Ok(())
    })
}

/// Copies the points of series `id` into the snapshot if the series changed after the change counter `since`, and fills
/// `info`. Returns `SDRKIT_E_NO_SUCH_ID` if the series does not exist.
/// The arrays and strings of `info` are views that the library owns, valid until the next call on `handle`. Copy what you keep.
#[no_mangle]
pub unsafe extern "C" fn sdrkit_snapshot_series(
    handle: Handle,
    id: Id,
    since: u64,
    info: *mut SeriesInfo,
) -> i32 {
    // SAFETY: the caller's contract.
    let out = unsafe { Out::new(info) };
    run(handle, |e| {
        let out = out?;
        let id = SeriesId(small(id)?);
        let r = e.owner_ref()?;
        let mut st = e.state();
        let buf = &mut st.results.series;
        let got = e.host.engine.series(r, id, since, buf)?;
        out.put(SeriesInfo {
            size: 0,
            changed: got.is_some(),
            counter: got.map_or(since, |g| g.counter),
            points: buf.as_ptr().cast(),
            count: if got.is_some() { buf.len() } else { 0 },
            line: got.is_some_and(|g| g.line),
        });
        Ok(())
    })
}

/// Copies the values of raster `id` into the snapshot if the raster changed after the change counter `since`, and fills
/// `info`. Returns `SDRKIT_E_NO_SUCH_ID` if the raster does not exist.
/// The arrays and strings of `info` are views that the library owns, valid until the next call on `handle`. Copy what you keep.
#[no_mangle]
pub unsafe extern "C" fn sdrkit_snapshot_raster(
    handle: Handle,
    id: Id,
    since: u64,
    info: *mut RasterInfo,
) -> i32 {
    // SAFETY: the caller's contract.
    let out = unsafe { Out::new(info) };
    run(handle, |e| {
        let out = out?;
        let id = RasterId(small(id)?);
        let r = e.owner_ref()?;
        let mut st = e.state();
        let buf = &mut st.results.raster;
        let got = e.host.engine.raster(r, id, since, buf)?;
        out.put(RasterInfo {
            size: 0,
            changed: got.is_some(),
            counter: got.map_or(since, |g| g.counter),
            values: buf.as_ptr(),
            width: got.map_or(0, |g| g.width),
            height: got.map_or(0, |g| g.height),
            x_start: got.map_or(0.0, |g| g.x.start),
            x_step: got.map_or(0.0, |g| g.x.step),
            y_start: got.map_or(0.0, |g| g.y.start),
            y_step: got.map_or(0.0, |g| g.y.step),
        });
        Ok(())
    })
}

crate::handle::sized_structs!(
    ResultsInfo,
    TableInfo,
    PlotInfo,
    SnapshotInfo,
    RowsInfo,
    Point,
    SeriesInfo,
    RasterInfo
);
