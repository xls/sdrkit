//! The typed calls: the structs of a family header (`include/sdrkit/<family>.h`) filled from the snapshot, the events
//! and the command replies of a handle, the commands with their arguments struct, and the settings by key.
//!
//! The layout of a struct comes from the interface description of the family ([`sdrkit_core::interface::layout`]),
//! which the header writer uses too. A handle resolves the ids of the members of a struct once and keeps them: a
//! call compares one name and copies the values.

use crate::error::Error;
use crate::events::event_at;
use crate::handle::{run, run_free, slice_in, str_in, Entry, Handle, Out};
use crate::results::RowsInfo;
use crate::value::{count, Bytes, Position, Str, Variant};
use sdrkit_core::interface::layout::{self, CType, Head, Layout, Member};
use sdrkit_core::interface::Family;
use sdrkit_core::{CommandId, FieldId, Record, RecordDecl, SettingId, Value, ValueKind};
use sdrkit_host_engine::EngineError;
use std::ffi::{c_char, c_void};
use std::mem::size_of;
use std::sync::{Arc, OnceLock};

/// The major version of the C interface of this library: the base header and the rules of the family headers. A
/// program built for another major version does not run with this library.
pub const SDRKIT_INTERFACE_VERSION_MAJOR: u32 = 1;
/// The minor version of the C interface of this library. It rises when the base header gets a function, a constant or
/// a member at the end of a struct that starts with `size`.
pub const SDRKIT_INTERFACE_VERSION_MINOR: u32 = 5;

/// The families that the library holds a typed interface of: the stack families of the catalog it was built with,
/// the shared interfaces that they use, then the session, the radio and the engine.
pub fn families() -> &'static [&'static Family] {
    static ALL: OnceLock<Vec<&'static Family>> = OnceLock::new();
    ALL.get_or_init(|| {
        let mut all = sdrkit_host_catalog::families();
        let shared = sdrkit_core::interface::shared_of(&all);
        all.extend(shared);
        all.extend(sdrkit_host_engine::object_families().iter());
        all
    })
}

// The layout crate places a variant member by the size of `sdrkit_variant`.
const _: () = assert!(CType::Variant.size_align_const().0 == size_of::<Variant>());

/// One struct of a family header.
struct Struct {
    /// `<family>.<Shape>`.
    name: String,
    head: Head,
    layout: Layout,
}

fn structs() -> &'static [Struct] {
    static ALL: OnceLock<Vec<Struct>> = OnceLock::new();
    ALL.get_or_init(|| {
        let mut out: Vec<Struct> = Vec::new();
        let mut add = |name: String, head: Head, layout: Layout| {
            if !out.iter().any(|s| s.name == name && s.head == head) {
                out.push(Struct { name, head, layout });
            }
        };
        for family in families() {
            // A shape of a shared interface has the key and the versions of that interface.
            let own = |shape: &str| family.owner_of_shape(shape);
            let name = |shape: &str| format!("{}.{shape}", own(shape).key);
            for s in &family.stacks {
                let sets = s.fields.iter();
                let of_components = s.components.iter().filter_map(|c| c.shape.as_ref());
                for f in sets.chain(of_components) {
                    add(
                        name(f.name),
                        Head::Plain,
                        layout::of_shape(own(f.name), f, Head::Plain),
                    );
                }
                let decoders = s.decoders();
                if !decoders.is_empty() {
                    add(
                        format!("{}.{}.dropped", family.key, s.id),
                        Head::Plain,
                        layout::of_dropped(&decoders),
                    );
                }
                for t in &s.tables {
                    add(
                        name(t.row.name),
                        Head::Row,
                        layout::of_shape(own(t.row.name), &t.row, Head::Row),
                    );
                }
                for e in &s.events {
                    add(
                        name(e.fields.name),
                        Head::Event,
                        layout::of_shape(own(e.fields.name), &e.fields, Head::Event),
                    );
                }
                for c in &s.commands {
                    // The header writer refuses arguments without a fixed layout, so a family of the library has none.
                    if let Ok(layout) = layout::of_args(&c.args) {
                        if !c.args.args.is_empty() {
                            add(name(c.args.name), Head::Plain, layout);
                        }
                    }
                    if !c.reply.fields.is_empty() {
                        add(
                            name(c.reply.name),
                            Head::Plain,
                            layout::of_shape(family, &c.reply, Head::Plain),
                        );
                    }
                }
            }
        }
        out
    })
}

fn no_such_key(key: &str) -> Error {
    Error::Engine(EngineError::NoSuchKey {
        key: key.to_owned(),
    })
}

fn find(name: &str, head: Head) -> Result<&'static Struct, Error> {
    structs()
        .iter()
        .find(|s| s.head == head && s.name == name)
        .ok_or_else(|| no_such_key(name))
}

/// A struct bound to a record declaration of a handle: the id of each member.
struct Binding {
    st: &'static Struct,
    /// The table key or the event type. Empty for a result set and a reply.
    context: String,
    decl: Arc<RecordDecl>,
    ids: Arc<[FieldId]>,
}

/// What a handle has resolved for the typed calls: struct members, settings and commands, each once.
#[derive(Default)]
pub(crate) struct Resolved {
    bindings: Vec<Binding>,
    settings: Vec<(String, SettingId)>,
    commands: Vec<(String, CommandId)>,
}

impl Resolved {
    /// The ids of the members of `st` in `decl`. They are looked up by key the first time, and again if the handle has
    /// another declaration now.
    ///
    /// # Errors
    ///
    /// [`Error::WrongHandle`] if `decl` does not hold a member with its kind.
    fn ids(
        &mut self,
        st: &'static Struct,
        context: &str,
        decl: &Arc<RecordDecl>,
    ) -> Result<Arc<[FieldId]>, Error> {
        let at = self
            .bindings
            .iter()
            .position(|b| std::ptr::eq(b.st, st) && b.context == context);
        if let Some(b) = at.map(|i| &self.bindings[i]) {
            if Arc::ptr_eq(&b.decl, decl) {
                return Ok(Arc::clone(&b.ids));
            }
        }
        // The result set of a named component has the context `@<component>`: its fields have the keys
        // `<component>.<key>`, and a field that the component shares with the stack has the key of the stack.
        let prefix = context
            .strip_prefix('@')
            .map(|component| format!("{component}."))
            .unwrap_or_default();
        let ids: Arc<[FieldId]> = resolve(&st.layout.members, &prefix, decl)?.into();
        let binding = Binding {
            st,
            context: context.to_owned(),
            decl: Arc::clone(decl),
            ids: Arc::clone(&ids),
        };
        match at {
            Some(i) => self.bindings[i] = binding,
            None => self.bindings.push(binding),
        }
        Ok(ids)
    }
}

/// The number of lookups of the members of a struct by key, for the test of the lookup once per handle.
#[cfg(test)]
pub(crate) static RESOLVES: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

fn resolve(members: &[Member], prefix: &str, decl: &RecordDecl) -> Result<Vec<FieldId>, Error> {
    #[cfg(test)]
    RESOLVES.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    // The members of a component set found under `<component>.<key>`.
    let mut named = 0;
    let ids: Result<Vec<FieldId>, Error> = members
        .iter()
        .map(|m| {
            let of_kind =
                |id: &FieldId| decl.field(*id).is_some_and(|f| f.kind == m.kind);
            if prefix.is_empty() {
                return decl.id(&m.key).filter(of_kind).ok_or(Error::WrongHandle);
            }
            if let Some(id) = decl.id(&format!("{prefix}{}", m.key)).filter(of_kind) {
                named += 1;
                return Ok(id);
            }
            // A field that the component shares with the stack has the key of the stack, without the prefix.
            decl.id(&m.key).filter(of_kind).ok_or(Error::WrongHandle)
        })
        .collect();
    // A stack without this component has none of its keys: the keys of the stack alone do not make the set.
    if !prefix.is_empty() && named == 0 {
        return Err(Error::WrongHandle);
    }
    ids
}

/// Writes `v` at `at` of the struct at `base`, if the member lies within `limit` bytes.
///
/// # Safety
///
/// `base` is valid for a write of `limit` bytes.
unsafe fn put<T>(base: *mut u8, limit: usize, at: usize, v: T) {
    if at + size_of::<T>() <= limit {
        // SAFETY: the member lies within the `limit` bytes that the caller holds. The write needs no alignment.
        unsafe { base.add(at).cast::<T>().write_unaligned(v) }
    }
}

/// Fills the struct at `base` from `rec`: zeros, then `size`, then each member that lies within `caller` bytes.
///
/// # Safety
///
/// `base` is valid for a write of `caller` bytes.
unsafe fn fill(base: *mut u8, caller: usize, st: &Struct, ids: &[FieldId], rec: &Record) {
    let limit = caller.min(st.layout.size);
    // SAFETY: the caller's contract. A member that the library does not know stays 0.
    unsafe {
        base.write_bytes(0, caller);
        put(base, caller, 0, u32::try_from(limit).unwrap_or(u32::MAX));
    }
    for (m, id) in st.layout.members.iter().zip(ids) {
        let v = rec.value(*id).unwrap_or(&Value::None);
        // SAFETY: `put` writes within `limit`, which is within `caller`.
        unsafe {
            if let Some(h) = m.has_at {
                put(base, limit, h, !v.is_none());
            }
            // A member of any kind holds the value with its type.
            if m.ctype == CType::Variant {
                put(base, limit, m.at, Variant::of(v));
                continue;
            }
            match v {
                Value::None => match m.ctype {
                    CType::Str => put(base, limit, m.at, Str::of("")),
                    CType::Bytes => put(base, limit, m.at, Bytes::of(&[])),
                    _ => {}
                },
                Value::Bool(b) => put(base, limit, m.at, *b),
                Value::I64(n) => put(base, limit, m.at, *n),
                Value::F64(x) | Value::Hz(x) | Value::Time(x) | Value::Duration(x) => {
                    put(base, limit, m.at, *x);
                }
                Value::Text(t) => put(base, limit, m.at, Str::of(t)),
                Value::Position { lat, lon } => put(base, limit, m.at, Position::at(*lat, *lon)),
                Value::Choice(i) => put(base, limit, m.at, i32::from(*i)),
                Value::Flags(mask) => put(base, limit, m.at, *mask),
                Value::Bytes(b) => put(base, limit, m.at, Bytes::of(b)),
            }
        }
    }
}

/// The value of member `m` of the struct at `base`, as a value of the kind of the member.
///
/// # Safety
///
/// `base` is valid for a read of the struct, and its string and bytes members are valid views.
unsafe fn take(base: *const u8, m: &Member) -> Result<Value, Error> {
    // SAFETY: the caller's contract. The read needs no alignment.
    unsafe {
        let at = base.add(m.at);
        let number = || at.cast::<f64>().read_unaligned();
        Ok(match m.kind {
            ValueKind::Bool => Value::Bool(at.read() != 0),
            ValueKind::I64 => Value::I64(at.cast::<i64>().read_unaligned()),
            ValueKind::F64 => Value::F64(number()),
            ValueKind::Hz => Value::Hz(number()),
            ValueKind::Time => Value::Time(number()),
            ValueKind::Duration => Value::Duration(number()),
            ValueKind::Choice => choice(at.cast::<i32>().read_unaligned())?,
            ValueKind::Flags => Value::Flags(at.cast::<u64>().read_unaligned()),
            ValueKind::Text => text(at.cast::<Str>().read_unaligned())?,
            ValueKind::Bytes => {
                let b = at.cast::<Bytes>().read_unaligned();
                Value::Bytes(Arc::from(slice_in(b.ptr, b.len)?))
            }
            ValueKind::Position => {
                let p = at.cast::<Position>().read_unaligned();
                Value::Position {
                    lat: p.lat,
                    lon: p.lon,
                }
            }
            ValueKind::Any => return Err(Error::BadArgument),
        })
    }
}

fn choice(index: i32) -> Result<Value, Error> {
    u16::try_from(index)
        .map(Value::Choice)
        .map_err(|_| Error::NotAnOption {
            index: u32::try_from(index).unwrap_or(u32::MAX),
        })
}

/// # Safety
///
/// `s` is valid for a read of `s.len` bytes.
unsafe fn text(s: Str) -> Result<Value, Error> {
    // SAFETY: the caller's contract.
    let b = unsafe { slice_in(s.ptr.cast::<u8>(), s.len)? };
    Ok(Value::Text(
        std::str::from_utf8(b)
            .map_err(|_| Error::BadString)?
            .to_owned(),
    ))
}

/// Writes the version of the C interface of this library to `major` and `minor`. The base header that a program was
/// built with has the version `SDRKIT_INTERFACE_VERSION_MAJOR`.`SDRKIT_INTERFACE_VERSION_MINOR`: the program runs with a
/// library of the same major version and of the same or a higher minor version.
#[no_mangle]
pub unsafe extern "C" fn sdrkit_interface_version(major: *mut u32, minor: *mut u32) -> i32 {
    // SAFETY: the caller's contract.
    let (major, minor) = unsafe { (Out::new(major), Out::new(minor)) };
    run_free(|| {
        if let Ok(o) = major {
            o.put(SDRKIT_INTERFACE_VERSION_MAJOR);
        }
        if let Ok(o) = minor {
            o.put(SDRKIT_INTERFACE_VERSION_MINOR);
        }
        Ok(())
    })
}

/// Writes the number of stack families that the library holds a typed interface of to `count`.
#[no_mangle]
pub unsafe extern "C" fn sdrkit_families_count(count: *mut u32) -> i32 {
    // SAFETY: the caller's contract.
    let out = unsafe { Out::new(count) };
    run_free(|| {
        out?.put(self::count(families().len()));
        Ok(())
    })
}

/// Writes the key of family `n` to `key` (a string of the library, valid while the library is loaded), and the version
/// of its interface to `major` and `minor`. Returns `SDRKIT_E_NO_SUCH_ID` if the family does not exist.
#[no_mangle]
pub unsafe extern "C" fn sdrkit_family(
    n: u32,
    key: *mut Str,
    major: *mut u32,
    minor: *mut u32,
) -> i32 {
    // SAFETY: the caller's contract.
    let (key, major, minor) = unsafe { (Out::new(key), Out::new(major), Out::new(minor)) };
    run_free(|| {
        let f = families().get(n as usize).ok_or(Error::NoSuchId)?;
        if let Ok(o) = key {
            o.put(Str::of(f.key));
        }
        if let Ok(o) = major {
            o.put(u32::from(f.version.major));
        }
        if let Ok(o) = minor {
            o.put(u32::from(f.version.minor));
        }
        Ok(())
    })
}

/// Writes the version of the interface of the stack family `family` in this library to `major` and `minor`. Returns
/// `SDRKIT_E_NO_SUCH_KEY` if the library was built without the family.
#[no_mangle]
pub unsafe extern "C" fn sdrkit_family_version(
    family: *const c_char,
    major: *mut u32,
    minor: *mut u32,
) -> i32 {
    // SAFETY: the caller's contract.
    let (family, major, minor) = unsafe { (str_in(family), Out::new(major), Out::new(minor)) };
    run_free(|| {
        let key = family?;
        let f = families()
            .iter()
            .find(|f| f.key == key)
            .ok_or_else(|| no_such_key(key))?;
        if let Ok(o) = major {
            o.put(u32::from(f.version.major));
        }
        if let Ok(o) = minor {
            o.put(u32::from(f.version.minor));
        }
        Ok(())
    })
}

/// Fills `rows` with the changed rows of the table `table` in the snapshot of `handle`, newest first, at most
/// `capacity` structs of `row_size` bytes each, and fills `info` as `sdrkit_snapshot_table` does. `type_name` names the
/// struct. The functions of a family header call this with the right names and size: use them. Returns
/// `SDRKIT_E_NO_SUCH_KEY` if the library has no such struct, and `SDRKIT_E_WRONG_HANDLE` if the snapshot does not hold
/// the table with the members of the struct.
#[no_mangle]
pub unsafe extern "C" fn sdrkit_typed_rows(
    handle: Handle,
    type_name: *const c_char,
    table: *const c_char,
    rows: *mut c_void,
    row_size: usize,
    capacity: u32,
    info: *mut RowsInfo,
) -> i32 {
    // SAFETY: the caller's contract.
    let (type_name, table, info) = unsafe { (str_in(type_name), str_in(table), Out::new(info)) };
    run(handle, |e| {
        let st = find(type_name?, Head::Row)?;
        let table = table?;
        if rows.is_null() && capacity > 0 {
            return Err(Error::Null);
        }
        if row_size < size_of::<u32>() {
            return Err(Error::BadArgument);
        }
        let mut guard = e.state();
        let state = &mut *guard;
        let t = state
            .results
            .changed_rows(table)
            .ok_or(Error::WrongHandle)?;
        let ids = state.typed.ids(st, table, t.columns)?;
        for (n, (key, rec)) in t.rows.iter().take(capacity as usize).enumerate() {
            // SAFETY: `rows` holds `capacity` structs of `row_size` bytes, and `n` is below `capacity`.
            unsafe {
                let base = rows.cast::<u8>().add(n * row_size);
                fill(base, row_size, st, &ids, rec);
                put(
                    base,
                    row_size.min(st.layout.size),
                    st.layout.head_at,
                    Str::of(key),
                );
            }
        }
        if let Some(o) = crate::handle::optional(info)? {
            o.put(RowsInfo {
                size: 0,
                rows: count(t.rows.len()),
                total: count(t.total),
                counter: t.counter,
            });
        }
        Ok(())
    })
}

/// Fills the struct of `size` bytes at `fields` with the result fields in the snapshot of `handle`. `type_name` names
/// the struct. With `@<component>` after the name, the fields are those of the named component of the stack, which a
/// generic consumer reads under the keys `<component>.<key>`. The functions of a family header call this with the right name and size: use them. Returns
/// `SDRKIT_E_NO_SUCH_KEY` if the library has no such struct, and `SDRKIT_E_WRONG_HANDLE` if the snapshot does not hold
/// the fields of the struct.
#[no_mangle]
pub unsafe extern "C" fn sdrkit_typed_fields(
    handle: Handle,
    type_name: *const c_char,
    fields: *mut c_void,
    size: usize,
) -> i32 {
    // SAFETY: the caller's contract.
    let type_name = unsafe { str_in(type_name) };
    run(handle, |e| {
        let type_name = type_name?;
        let at = type_name.find('@').unwrap_or(type_name.len());
        let (type_name, component) = type_name.split_at(at);
        let st = find(type_name, Head::Plain)?;
        if fields.is_null() {
            return Err(Error::Null);
        }
        if size < size_of::<u32>() {
            return Err(Error::BadArgument);
        }
        let mut guard = e.state();
        let state = &mut *guard;
        let rec = state.results.fields().ok_or(Error::WrongHandle)?;
        let ids = state.typed.ids(st, component, rec.decl())?;
        // SAFETY: `fields` holds `size` bytes.
        unsafe { fill(fields.cast(), size, st, &ids, rec) };
        Ok(())
    })
}

/// Fills the struct of `size` bytes at `event` with event `n` of the last `sdrkit_events_take` on `handle`.
/// `event_type` is the key of the event type and `type_name` names the struct. The functions of a family header call
/// this with the right names and size: use them. Returns `SDRKIT_E_NO_SUCH_ID` if the event does not exist,
/// `SDRKIT_E_WRONG_TYPE` if it is of another type, `SDRKIT_E_NO_SUCH_KEY` if the library has no such struct, and
/// `SDRKIT_E_WRONG_HANDLE` if the event does not hold the members of the struct.
#[no_mangle]
pub unsafe extern "C" fn sdrkit_typed_event(
    handle: Handle,
    n: u32,
    event_type: *const c_char,
    type_name: *const c_char,
    event: *mut c_void,
    size: usize,
) -> i32 {
    // SAFETY: the caller's contract.
    let (event_type, type_name) = unsafe { (str_in(event_type), str_in(type_name)) };
    run(handle, |e| {
        let (event_type, st) = (event_type?, find(type_name?, Head::Event)?);
        if event.is_null() {
            return Err(Error::Null);
        }
        if size < size_of::<u32>() {
            return Err(Error::BadArgument);
        }
        let mut guard = e.state();
        let state = &mut *guard;
        let ev = event_at(&state.events, n)?;
        if ev.kind() != event_type {
            return Err(Error::WrongType);
        }
        let ids = state.typed.ids(st, event_type, ev.fields.decl())?;
        // SAFETY: `event` holds `size` bytes.
        unsafe {
            let base = event.cast::<u8>();
            fill(base, size, st, &ids, &ev.fields);
            let limit = size.min(st.layout.size);
            put(base, limit, st.layout.head_at, ev.time);
            put(
                base,
                limit,
                st.layout.head_at + size_of::<f64>(),
                ev.freq_hz,
            );
        }
        Ok(())
    })
}

/// The id of the command `key` of the object of `e`, looked up once for the handle.
fn command_id(e: &Entry, key: &str) -> Result<CommandId, Error> {
    if let Some((_, id)) = e.state().typed.commands.iter().find(|(k, _)| k == key) {
        return Ok(*id);
    }
    let id = e
        .owner(|o| o.commands().id(key))?
        .map_err(|_| Error::WrongHandle)?;
    e.state().typed.commands.push((key.to_owned(), id));
    Ok(id)
}

/// Runs the command with the key `command` of the object of `handle`. `args` is the arguments struct that `args_type`
/// names, with its `size` set, or NULL with a NULL `args_type` for a command without arguments. An optional argument
/// whose `has_` member is false, or that lies after the `size` bytes of the struct, takes its default. If `reply` is not
/// NULL, the call fills the struct of `reply_size` bytes that `reply_type` names with the reply. The functions of a
/// family header call this with the right names and sizes: use them. Returns `SDRKIT_E_WRONG_HANDLE` if the object does
/// not have the command with the arguments and the reply of the structs, `SDRKIT_E_NO_SUCH_KEY` if the library has no
/// such struct, or the codes of `sdrkit_command_invoke`.
#[no_mangle]
pub unsafe extern "C" fn sdrkit_typed_command(
    handle: Handle,
    command: *const c_char,
    args_type: *const c_char,
    args: *const c_void,
    reply_type: *const c_char,
    reply: *mut c_void,
    reply_size: usize,
) -> i32 {
    // SAFETY: the caller's contract.
    let command = unsafe { str_in(command) };
    run(handle, |e| {
        let id = command_id(e, command?)?;
        let mut given: Vec<(FieldId, Value)> = Vec::new();
        let mut members = 0;
        if !args_type.is_null() {
            // SAFETY: the caller's contract.
            let st = find(unsafe { str_in(args_type)? }, Head::Plain)?;
            if args.is_null() {
                return Err(Error::Null);
            }
            let base = args.cast::<u8>();
            // SAFETY: an arguments struct starts with its size.
            let size = unsafe { crate::handle::size_at(base)? };
            members = st.layout.members.len();
            for m in &st.layout.members {
                let field = FieldId(u16::try_from(m.index).map_err(|_| Error::NoSuchId)?);
                if m.at + m.ctype.size_align().0 > size {
                    // A struct of an older header ends before an argument that was added later.
                    if m.has_at.is_none() {
                        return Err(Error::BadArgument);
                    }
                    continue;
                }
                // SAFETY: the member and its `has_` member lie within the `size` bytes of the struct.
                unsafe {
                    if m.has_at.is_some_and(|h| base.add(h).read() == 0) {
                        continue;
                    }
                    given.push((field, take(base, m)?));
                }
            }
        }
        let got = e.owner(|o| {
            let def = o.commands().def(id).ok_or(Error::WrongHandle)?;
            let mut rec = def.new_args();
            if members != def.args.len() {
                return Err(Error::WrongHandle);
            }
            for (f, v) in &given {
                rec.set_value(*f, v).map_err(|_| Error::WrongHandle)?;
            }
            sdrkit_core::owner::invoke(o, id, &rec)
                .map_err(|err| Error::Engine(EngineError::command(o, id, err)))
        })??;
        let mut guard = e.state();
        let state = &mut *guard;
        state.command.reply = got;
        if reply.is_null() {
            return Ok(());
        }
        // SAFETY: the caller's contract.
        let st = find(unsafe { str_in(reply_type)? }, Head::Plain)?;
        if reply_size < size_of::<u32>() {
            return Err(Error::BadArgument);
        }
        let rec = state.command.reply.as_ref().ok_or(Error::WrongHandle)?;
        let ids = state.typed.ids(st, "", rec.decl())?;
        // SAFETY: `reply` holds `reply_size` bytes.
        unsafe { fill(reply.cast(), reply_size, st, &ids, rec) };
        Ok(())
    })
}

/// The id of the setting `key` of the object of `e`, looked up once for the handle.
fn setting_id(e: &Entry, key: &str) -> Result<SettingId, Error> {
    if let Some((_, id)) = e.state().typed.settings.iter().find(|(k, _)| k == key) {
        return Ok(*id);
    }
    let id = e
        .owner(|o| o.schema().id(key))?
        .map_err(|_| Error::WrongHandle)?;
    e.state().typed.settings.push((key.to_owned(), id));
    Ok(id)
}

/// Reads the setting `key` of the object of `handle` into the value buffer of the handle, and passes it to `read`.
fn setting_get(
    handle: Handle,
    key: Result<&str, Error>,
    read: impl FnOnce(&Value) -> Result<(), Error>,
) -> i32 {
    run(handle, |e| {
        let id = setting_id(e, key?)?;
        let mut st = e.state();
        let found = e.owner(|o| {
            let v: Option<&Value> = match o.values() {
                Some(v) => v.value(id),
                None => o.schema().setting(id).map(|s| &s.default),
            };
            v.map(|v| st.value.clone_from(v)).is_some()
        })?;
        if !found {
            return Err(Error::WrongHandle);
        }
        read(&st.value)
    })
}

/// Sets the setting `key` of the object of `handle` to the value that `make` returns for the kind of the setting.
fn setting_set(
    handle: Handle,
    key: Result<&str, Error>,
    make: impl FnOnce(ValueKind) -> Result<Value, Error>,
) -> i32 {
    run(handle, |e| {
        let id = setting_id(e, key?)?;
        let kind = e
            .owner(|o| o.schema().setting(id).map(|s| s.field.kind))?
            .ok_or(Error::WrongHandle)?;
        let v = make(kind)?;
        e.owner(|o| {
            sdrkit_core::owner::apply(o, id, &v)
                .map_err(|err| Error::Engine(EngineError::setting(o.schema(), err)))
        })?
    })
}

/// Writes the value of the setting `key` of the object of `handle` to `value`: a switch. For a type of a catalog,
/// the value is the default. The functions of a family header call this with the key and the type of each setting:
/// use them. Returns `SDRKIT_E_WRONG_HANDLE` if the object has no setting `key`, and `SDRKIT_E_WRONG_TYPE` if the
/// setting is of another type.
#[no_mangle]
pub unsafe extern "C" fn sdrkit_typed_setting_get_bool(
    handle: Handle,
    key: *const c_char,
    value: *mut bool,
) -> i32 {
    // SAFETY: the caller's contract.
    let (key, out) = unsafe { (str_in(key), Out::new(value)) };
    setting_get(handle, key, |v| {
        let got: Option<bool> = match v {
            Value::Bool(b) => Some(*b),
            _ => None,
        };
        out?.put(got.ok_or(Error::WrongType)?);
        Ok(())
    })
}

/// Sets the setting `key` of the object of `handle` to `value`: a switch. The value must match the declaration of the
/// setting. A refused value changes nothing. Returns `SDRKIT_E_WRONG_HANDLE` if the object has no setting `key`,
/// `SDRKIT_E_WRONG_TYPE` if the setting is of another type, or the codes of `sdrkit_setting_set`.
#[no_mangle]
pub unsafe extern "C" fn sdrkit_typed_setting_set_bool(
    handle: Handle,
    key: *const c_char,
    value: bool,
) -> i32 {
    // SAFETY: the caller's contract.
    let key = unsafe { str_in(key) };
    setting_set(handle, key, |kind| {
        if kind == ValueKind::Bool {
            Ok(Value::Bool(value))
        } else {
            Err(Error::WrongType)
        }
    })
}

/// Writes the value of the setting `key` of the object of `handle` to `value`: a whole number. For a type of a catalog,
/// the value is the default. The functions of a family header call this with the key and the type of each setting:
/// use them. Returns `SDRKIT_E_WRONG_HANDLE` if the object has no setting `key`, and `SDRKIT_E_WRONG_TYPE` if the
/// setting is of another type.
#[no_mangle]
pub unsafe extern "C" fn sdrkit_typed_setting_get_i64(
    handle: Handle,
    key: *const c_char,
    value: *mut i64,
) -> i32 {
    // SAFETY: the caller's contract.
    let (key, out) = unsafe { (str_in(key), Out::new(value)) };
    setting_get(handle, key, |v| {
        let got: Option<i64> = match v {
            Value::I64(n) => Some(*n),
            _ => None,
        };
        out?.put(got.ok_or(Error::WrongType)?);
        Ok(())
    })
}

/// Sets the setting `key` of the object of `handle` to `value`: a whole number. The value must match the declaration of the
/// setting. A refused value changes nothing. Returns `SDRKIT_E_WRONG_HANDLE` if the object has no setting `key`,
/// `SDRKIT_E_WRONG_TYPE` if the setting is of another type, or the codes of `sdrkit_setting_set`.
#[no_mangle]
pub unsafe extern "C" fn sdrkit_typed_setting_set_i64(
    handle: Handle,
    key: *const c_char,
    value: i64,
) -> i32 {
    // SAFETY: the caller's contract.
    let key = unsafe { str_in(key) };
    setting_set(handle, key, |kind| {
        if kind == ValueKind::I64 {
            Ok(Value::I64(value))
        } else {
            Err(Error::WrongType)
        }
    })
}

/// Writes the value of the setting `key` of the object of `handle` to `value`: a number, a frequency in Hz, a time or a length of time in seconds. For a type of a catalog,
/// the value is the default. The functions of a family header call this with the key and the type of each setting:
/// use them. Returns `SDRKIT_E_WRONG_HANDLE` if the object has no setting `key`, and `SDRKIT_E_WRONG_TYPE` if the
/// setting is of another type.
#[no_mangle]
pub unsafe extern "C" fn sdrkit_typed_setting_get_f64(
    handle: Handle,
    key: *const c_char,
    value: *mut f64,
) -> i32 {
    // SAFETY: the caller's contract.
    let (key, out) = unsafe { (str_in(key), Out::new(value)) };
    setting_get(handle, key, |v| {
        let got: Option<f64> = match v {
            Value::F64(x) | Value::Hz(x) | Value::Time(x) | Value::Duration(x) => Some(*x),
            _ => None,
        };
        out?.put(got.ok_or(Error::WrongType)?);
        Ok(())
    })
}

/// Sets the setting `key` of the object of `handle` to `value`: a number, a frequency in Hz, a time or a length of time in seconds. The value must match the declaration of the
/// setting. A refused value changes nothing. Returns `SDRKIT_E_WRONG_HANDLE` if the object has no setting `key`,
/// `SDRKIT_E_WRONG_TYPE` if the setting is of another type, or the codes of `sdrkit_setting_set`.
#[no_mangle]
pub unsafe extern "C" fn sdrkit_typed_setting_set_f64(
    handle: Handle,
    key: *const c_char,
    value: f64,
) -> i32 {
    // SAFETY: the caller's contract.
    let key = unsafe { str_in(key) };
    setting_set(handle, key, |kind| match kind {
        ValueKind::F64 => Ok(Value::F64(value)),
        ValueKind::Hz => Ok(Value::Hz(value)),
        ValueKind::Time => Ok(Value::Time(value)),
        ValueKind::Duration => Ok(Value::Duration(value)),
        _ => Err(Error::WrongType),
    })
}

/// Writes the value of the setting `key` of the object of `handle` to `value`: the index of an option. For a type of a catalog,
/// the value is the default. The functions of a family header call this with the key and the type of each setting:
/// use them. Returns `SDRKIT_E_WRONG_HANDLE` if the object has no setting `key`, and `SDRKIT_E_WRONG_TYPE` if the
/// setting is of another type.
#[no_mangle]
pub unsafe extern "C" fn sdrkit_typed_setting_get_enum(
    handle: Handle,
    key: *const c_char,
    value: *mut i32,
) -> i32 {
    // SAFETY: the caller's contract.
    let (key, out) = unsafe { (str_in(key), Out::new(value)) };
    setting_get(handle, key, |v| {
        let got: Option<i32> = match v {
            Value::Choice(i) => Some(i32::from(*i)),
            _ => None,
        };
        out?.put(got.ok_or(Error::WrongType)?);
        Ok(())
    })
}

/// Sets the setting `key` of the object of `handle` to `value`: the index of an option. The value must match the declaration of the
/// setting. A refused value changes nothing. Returns `SDRKIT_E_WRONG_HANDLE` if the object has no setting `key`,
/// `SDRKIT_E_WRONG_TYPE` if the setting is of another type, or the codes of `sdrkit_setting_set`.
#[no_mangle]
pub unsafe extern "C" fn sdrkit_typed_setting_set_enum(
    handle: Handle,
    key: *const c_char,
    value: i32,
) -> i32 {
    // SAFETY: the caller's contract.
    let key = unsafe { str_in(key) };
    setting_set(handle, key, |kind| {
        if kind == ValueKind::Choice {
            choice(value)
        } else {
            Err(Error::WrongType)
        }
    })
}

/// Writes the value of the setting `key` of the object of `handle` to `value`: a string. The string is valid until the next call on `handle`. For a type of a catalog,
/// the value is the default. The functions of a family header call this with the key and the type of each setting:
/// use them. Returns `SDRKIT_E_WRONG_HANDLE` if the object has no setting `key`, and `SDRKIT_E_WRONG_TYPE` if the
/// setting is of another type.
#[no_mangle]
pub unsafe extern "C" fn sdrkit_typed_setting_get_string(
    handle: Handle,
    key: *const c_char,
    value: *mut Str,
) -> i32 {
    // SAFETY: the caller's contract.
    let (key, out) = unsafe { (str_in(key), Out::new(value)) };
    setting_get(handle, key, |v| {
        let got: Option<Str> = match v {
            Value::Text(t) => Some(Str::of(t)),
            _ => None,
        };
        out?.put(got.ok_or(Error::WrongType)?);
        Ok(())
    })
}

/// Sets the setting `key` of the object of `handle` to `value`: a string. The value must match the declaration of the
/// setting. A refused value changes nothing. Returns `SDRKIT_E_WRONG_HANDLE` if the object has no setting `key`,
/// `SDRKIT_E_WRONG_TYPE` if the setting is of another type, or the codes of `sdrkit_setting_set`.
#[no_mangle]
pub unsafe extern "C" fn sdrkit_typed_setting_set_string(
    handle: Handle,
    key: *const c_char,
    value: Str,
) -> i32 {
    // SAFETY: the caller's contract.
    let key = unsafe { str_in(key) };
    setting_set(handle, key, |kind| {
        if kind == ValueKind::Text {
            // SAFETY: the caller's contract on the view.
            unsafe { text(value) }
        } else {
            Err(Error::WrongType)
        }
    })
}

/// Writes the value of the setting `key` of the object of `handle` to `value`: a position. For a type of a catalog,
/// the value is the default. The functions of a family header call this with the key and the type of each setting:
/// use them. Returns `SDRKIT_E_WRONG_HANDLE` if the object has no setting `key`, and `SDRKIT_E_WRONG_TYPE` if the
/// setting is of another type.
#[no_mangle]
pub unsafe extern "C" fn sdrkit_typed_setting_get_position(
    handle: Handle,
    key: *const c_char,
    value: *mut Position,
) -> i32 {
    // SAFETY: the caller's contract.
    let (key, out) = unsafe { (str_in(key), Out::new(value)) };
    setting_get(handle, key, |v| {
        let got: Option<Position> = match v {
            Value::Position { lat, lon } => Some(Position::at(*lat, *lon)),
            _ => None,
        };
        out?.put(got.ok_or(Error::WrongType)?);
        Ok(())
    })
}

/// Sets the setting `key` of the object of `handle` to `value`: a position. The value must match the declaration of the
/// setting. A refused value changes nothing. Returns `SDRKIT_E_WRONG_HANDLE` if the object has no setting `key`,
/// `SDRKIT_E_WRONG_TYPE` if the setting is of another type, or the codes of `sdrkit_setting_set`.
#[no_mangle]
pub unsafe extern "C" fn sdrkit_typed_setting_set_position(
    handle: Handle,
    key: *const c_char,
    value: Position,
) -> i32 {
    // SAFETY: the caller's contract.
    let key = unsafe { str_in(key) };
    setting_set(handle, key, |kind| {
        if kind == ValueKind::Position {
            Ok(Value::Position {
                lat: value.lat,
                lon: value.lon,
            })
        } else {
            Err(Error::WrongType)
        }
    })
}

#[cfg(test)]
mod component_tests {
    use super::*;
    use sdrkit_core::interface::{Fields, StackShape};

    /// The members of the result set of a named component are found under `<component>.<key>`, and the members of
    /// the struct of dropped counts under `<decoder>.dropped`.
    #[test]
    fn the_members_of_a_component_set_have_the_name_of_the_component_before_their_keys() {
        let stack = StackShape::new("demo")
            .fields::<sdrkit_graph::DecoderFields>()
            .component("rx", |c| c.fields::<sdrkit_graph::DecoderFields>())
            .decoder("modem");
        let decl = stack.results_decl();
        let record = decl.fields();
        let family = Family::new("demo", 1, 0);
        let shape = <sdrkit_graph::DecoderFields as Fields>::SHAPE;
        let set = layout::of_shape(&family, &shape, Head::Plain);
        let own = resolve(&set.members, "", record).expect("the set of the stack");
        let of_rx = resolve(&set.members, "rx.", record).expect("the set of the component");
        assert_eq!(record.field(own[0]).map(|f| f.key.as_ref()), Some("dropped"));
        assert_eq!(record.field(of_rx[0]).map(|f| f.key.as_ref()), Some("rx.dropped"));
        assert!(resolve(&set.members, "nobody.", record).is_err());
        let counts = layout::of_dropped(&stack.decoders());
        let ids = resolve(&counts.members, "", record).expect("the counts");
        assert_eq!(record.field(ids[0]).map(|f| f.key.as_ref()), Some("modem.dropped"));
        assert!(counts.members[0].has_at.is_some());
    }

    /// The totals of a stack and the counts of one of its decoders, which shares one field with the stack.
    #[derive(Fields)]
    #[allow(dead_code)]
    struct TotalFields {
        /// The good frames of every decoder.
        #[field(label = "Frames good")]
        frames_ok: i64,
    }

    /// The counts of one decoder.
    #[derive(Fields)]
    #[allow(dead_code)]
    struct LinkFields {
        /// The good frames.
        #[field(label = "Frames good")]
        frames_ok: i64,
        /// The bad frames.
        #[field(label = "Frames bad")]
        frames_bad: i64,
    }

    /// A member that the component shares with the stack is found under the key of the stack. A stack without the
    /// component does not make the set from its own keys.
    #[test]
    fn a_shared_member_of_a_component_set_has_the_key_of_the_stack() {
        let stack = StackShape::new("demo")
            .fields::<TotalFields>()
            .component("link", |c| c.fields::<LinkFields>().shares(&["frames_ok"]));
        let decl = stack.results_decl();
        let record = decl.fields();
        let family = Family::new("demo", 1, 0);
        let set = layout::of_shape(&family, &<LinkFields as Fields>::SHAPE, Head::Plain);
        let ids = resolve(&set.members, "link.", record).expect("the set of the component");
        let keys: Vec<&str> = ids
            .iter()
            .filter_map(|id| record.field(*id).map(|f| f.key.as_ref()))
            .collect();
        assert_eq!(keys.len(), 2);
        assert!(keys.contains(&"frames_ok") && keys.contains(&"link.frames_bad"));
        // The stack alone has `frames_ok`, and no key of a component "other".
        let total = layout::of_shape(&family, &<TotalFields as Fields>::SHAPE, Head::Plain);
        assert!(resolve(&total.members, "other.", record).is_err());
    }

    sdrkit_core::flag_set! {
        /// What is wrong with a frame.
        struct Faults {
            CRC = ("crc", "CRC", "The checksum does not match."),
            LATE = ("late", "Late", "The frame is after its slot."),
        }
    }

    /// A set with a flag member between members of other sizes.
    #[derive(Fields)]
    #[allow(dead_code)]
    struct FaultFields {
        /// The decoder is on.
        #[field(label = "On")]
        on: bool,
        /// The faults of the last frame.
        #[field(label = "Faults")]
        faults: Faults,
        /// The frames.
        #[field(label = "Frames")]
        frames: i64,
    }

    /// The C struct of [`FaultFields`], as a header declares it.
    #[repr(C)]
    struct CFaultFields {
        size: u32,
        on: bool,
        faults: u64,
        frames: i64,
    }

    /// A flag member is a 64-bit mask at its aligned place, and reads back as the same mask.
    #[test]
    fn a_flag_member_is_a_mask_in_the_struct() {
        let family = Family::new("demo", 1, 0);
        let shape = <FaultFields as Fields>::SHAPE;
        let st = Struct {
            name: "demo.FaultFields".into(),
            head: Head::Plain,
            layout: layout::of_shape(&family, &shape, Head::Plain),
        };
        assert_eq!(st.layout.size, size_of::<CFaultFields>());
        let mut rec = Record::new(Arc::new(RecordDecl::of(shape.fields)));
        let values = [Value::Bool(true), Value::Flags(0b10), Value::I64(7)];
        for (i, v) in values.iter().enumerate() {
            rec.set_value(FieldId(i as u16), v).expect("a value of the field");
        }
        let ids: Vec<FieldId> = st
            .layout
            .members
            .iter()
            .map(|m| FieldId(m.index as u16))
            .collect();
        let mut c = std::mem::MaybeUninit::<CFaultFields>::zeroed();
        // SAFETY: `c` is valid for a write of its size, and holds a value of every member after the fill.
        let c = unsafe {
            fill(c.as_mut_ptr().cast(), size_of::<CFaultFields>(), &st, &ids, &rec);
            c.assume_init()
        };
        assert_eq!((c.on, c.faults, c.frames), (true, 0b10, 7));
        let m = st.layout.members.iter().find(|m| m.key == "faults").expect("the member");
        // SAFETY: `c` is a live struct of this layout.
        let back = unsafe { take(std::ptr::from_ref(&c).cast(), m) };
        assert_eq!(back.ok(), Some(Value::Flags(0b10)));
    }
}
