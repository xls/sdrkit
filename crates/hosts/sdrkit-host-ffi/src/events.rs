//! The event types of the catalog of the engine, and the events of an object.

use crate::error::Error;
use crate::handle::{run, str_in, Handle, Out};
use crate::value::{count, small, Id, Str, Variant};
use sdrkit_core::{Event, EventKindId, FieldId};
use sdrkit_host_engine::EngineError;
use std::ffi::c_char;

/// The declaration of one event type of the catalog.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct EventTypeInfo {
    /// The number of bytes of the struct. The caller sets it to the sizeof of its struct before the call; the call sets it to
    /// the number of bytes that it wrote.
    pub size: u32,
    /// The key.
    pub key: Str,
    /// The name for people.
    pub label: Str,
    /// The number of fields. Field ids go from 0 to `fields` - 1.
    pub fields: u32,
}

/// One event of the last `sdrkit_events_take`.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct EventInfo {
    /// The number of bytes of the struct. The caller sets it to the sizeof of its struct before the call; the call sets it to
    /// the number of bytes that it wrote.
    pub size: u32,
    /// The id of its event type in the catalog.
    pub r#type: Id,
    /// Seconds since the object started, in the time of the signal.
    pub time: f64,
    /// The frequency of the signal in Hz.
    pub freq_hz: f64,
    /// The number of fields.
    pub fields: u32,
}

/// The events of the last take of a handle.
#[derive(Default)]
pub(crate) struct Taken {
    events: Vec<Event>,
}

/// Writes the number of event types in the catalog of the engine to `count`. Event type ids go from 0 to `count` - 1.
#[no_mangle]
pub unsafe extern "C" fn sdrkit_event_types_count(handle: Handle, count: *mut u32) -> i32 {
    // SAFETY: the caller's contract.
    let out = unsafe { Out::new(count) };
    run(handle, |e| {
        out?.put(self::count(e.host.engine.event_catalog().len()));
        Ok(())
    })
}

/// Writes the id of the event type with the key `key` to `id`. Returns `SDRKIT_E_NO_SUCH_KEY` if no event type has the key.
#[no_mangle]
pub unsafe extern "C" fn sdrkit_event_type_find(
    handle: Handle,
    key: *const c_char,
    id: *mut Id,
) -> i32 {
    // SAFETY: the caller's contract.
    let (key, out) = unsafe { (str_in(key), Out::new(id)) };
    run(handle, |e| {
        let (key, out) = (key?, out?);
        let id = e.host.engine.event_catalog().id(key).ok_or_else(|| {
            Error::Engine(EngineError::NoSuchKey {
                key: key.to_owned(),
            })
        })?;
        out.put(Id::from(id.0));
        Ok(())
    })
}

/// Fills `info` with the declaration of event type `id`. Returns `SDRKIT_E_NO_SUCH_ID` if the event type does not exist.
/// The strings of `info` are views that the library owns, valid until the next call on `handle`. Copy what you keep.
#[no_mangle]
pub unsafe extern "C" fn sdrkit_event_type_decl(
    handle: Handle,
    id: Id,
    info: *mut EventTypeInfo,
) -> i32 {
    // SAFETY: the caller's contract.
    let out = unsafe { Out::new(info) };
    run(handle, |e| {
        let out = out?;
        let d = e
            .host
            .engine
            .event_catalog()
            .def(EventKindId(small(id)?))
            .ok_or(Error::NoSuchId)?;
        out.put(EventTypeInfo {
            size: 0,
            key: Str::of(&d.kind),
            label: Str::of(&d.label),
            fields: count(d.fields.len()),
        });
        Ok(())
    })
}

/// Takes the waiting events of the object of `handle` and writes their number to `count`. The engine, radios, sessions and
/// services have events. The events of the previous take are dropped.
#[no_mangle]
pub unsafe extern "C" fn sdrkit_events_take(handle: Handle, count: *mut u32) -> i32 {
    // SAFETY: the caller's contract.
    let out = unsafe { Out::new(count) };
    run(handle, |e| {
        let out = out?;
        let r = e.owner_ref()?;
        let mut st = e.state();
        let taken = &mut st.events.events;
        taken.clear();
        e.host.engine.take_events(r, taken)?;
        out.put(self::count(taken.len()));
        Ok(())
    })
}

pub(crate) fn event_at(st: &Taken, n: u32) -> Result<&Event, Error> {
    usize::try_from(n)
        .ok()
        .and_then(|i| st.events.get(i))
        .ok_or(Error::NoSuchId)
}

/// Fills `event` with event `n` of the last take. Returns `SDRKIT_E_NO_SUCH_ID` if the event does not exist.
#[no_mangle]
pub unsafe extern "C" fn sdrkit_event(handle: Handle, n: u32, event: *mut EventInfo) -> i32 {
    // SAFETY: the caller's contract.
    let out = unsafe { Out::new(event) };
    run(handle, |e| {
        let out = out?;
        let st = e.state();
        let ev = event_at(&st.events, n)?;
        let t = ev
            .kind_id()
            .or_else(|| e.host.engine.event_catalog().id(ev.kind()))
            .ok_or(Error::NoSuchId)?;
        out.put(EventInfo {
            size: 0,
            r#type: Id::from(t.0),
            time: ev.time,
            freq_hz: ev.freq_hz,
            fields: count(ev.fields.decl().len()),
        });
        Ok(())
    })
}

/// Writes field `id` of event `n` of the last take to `value`. Returns `SDRKIT_E_NO_SUCH_ID` if the event or the field does
/// not exist.
/// A string or bytes in `value` is a view that the library owns, valid until the next call on `handle`. Copy what you keep.
/// For a event type that a family header declares, the typed function (`sdrkit_<type>_event_read`) is the first choice.
#[no_mangle]
pub unsafe extern "C" fn sdrkit_event_field(
    handle: Handle,
    n: u32,
    id: Id,
    value: *mut Variant,
) -> i32 {
    // SAFETY: the caller's contract.
    let out = unsafe { Out::new(value) };
    run(handle, |e| {
        let out = out?;
        let st = e.state();
        let ev = event_at(&st.events, n)?;
        let id = FieldId(small(id)?);
        out.put(Variant::of(ev.fields.value(id).ok_or(Error::NoSuchId)?));
        Ok(())
    })
}

crate::handle::sized_structs!(EventTypeInfo, EventInfo);
