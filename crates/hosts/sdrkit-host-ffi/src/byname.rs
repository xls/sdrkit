//! Calls by name, for a program that knows what it wants: start a stack on a radio with the defaults of the stack, set a
//! setting and run a command by key with values written as text, and read an event field as text. The calls by id
//! stay for a program that reads the declarations.

use crate::engine::object_handle;
use crate::error::Error;
use crate::events::event_at;
use crate::handle::{run, slice_in, str_in, Handle, Out};
use crate::value::Str;
use sdrkit_host_engine::OwnerRef;
use std::ffi::c_char;

/// Starts a session of the stack `stack` on the radio of `radio` with the defaults of the stack, on the default
/// frequency of the stack, and writes its handle to `session`. A stack whose default frequency is 0 stays on the
/// frequency of the radio until `sdrkit_session_set_tuned_hz` or the setting `tuned_hz` sets one.
/// Returns `SDRKIT_E_NO_SUCH_KEY` for an unknown stack, `SDRKIT_E_WRONG_HANDLE` if `radio` is not a radio, or the codes
/// of `sdrkit_stack_start`. `sdrkit_stack_start` is the first choice: it takes the frequency, the settings and any input.
#[no_mangle]
pub unsafe extern "C" fn sdrkit_start(
    radio: Handle,
    stack: *const c_char,
    session: *mut Handle,
) -> i32 {
    // SAFETY: the caller's contract.
    let (stack, out) = unsafe { (str_in(stack), Out::new(session)) };
    run(radio, |e| {
        let (stack, out) = (stack?, out?);
        let OwnerRef::Radio(r) = e.owner_ref()? else {
            return Err(Error::WrongHandle);
        };
        let id = e.host.engine.start(stack, r, None)?;
        out.put(object_handle(e, OwnerRef::Session(id)));
        Ok(())
    })
}

/// Sets the setting with the key `key` of the object of `handle` to `value`, written as text: `yes` or `no` for a
/// switch, a number, the key or the label of an option, or the text itself. Returns `SDRKIT_E_NO_SUCH_KEY` if no
/// setting has the key, `SDRKIT_E_MALFORMED` if the text is not a value of the setting, or the codes of
/// `sdrkit_setting_set`. `sdrkit_error_message` names the setting. The typed function of the setting in the header of
/// its family (`sdrkit_fm_set_squelch`, `sdrkit_session_set_tuned_hz`) is the first choice: the compiler checks its name
/// and the type of its value.
#[no_mangle]
pub unsafe extern "C" fn sdrkit_set(
    handle: Handle,
    key: *const c_char,
    value: *const c_char,
) -> i32 {
    // SAFETY: the caller's contract.
    let (key, value) = unsafe { (str_in(key), str_in(value)) };
    run(handle, |e| {
        Ok(e.host.engine.set_text(e.owner_ref()?, key?, value?)?)
    })
}

/// Runs the command with the key `command` of the object of `handle`. `args` holds `count` pairs of strings: the key of
/// an argument, then its value written as text (as `sdrkit_set` reads a value). `args` can be NULL if `count` is 0. An
/// argument that is not given has its default. The reply is read with `sdrkit_reply`. Returns `SDRKIT_E_NO_SUCH_KEY` if there is no such command, or the
/// codes of `sdrkit_command_invoke`. `sdrkit_error_message` names the command and the argument. The typed function of
/// the command in the header of its family (`sdrkit_fm_clear`) is the first choice: its arguments are a struct.
#[no_mangle]
pub unsafe extern "C" fn sdrkit_call(
    handle: Handle,
    command: *const c_char,
    args: *const *const c_char,
    count: u32,
) -> i32 {
    // SAFETY: the caller's contract.
    let (command, args) = unsafe {
        let args = match count {
            0 => Ok(&[][..]),
            n => slice_in(args, 2 * n as usize),
        };
        (str_in(command), args)
    };
    run(handle, |e| {
        let mut pairs = Vec::with_capacity(count as usize);
        for p in args?.as_chunks::<2>().0 {
            // SAFETY: the caller's contract on the strings of `args`.
            pairs.push(unsafe { (str_in(p[0])?, str_in(p[1])?) });
        }
        let reply = e.host.engine.call_text(e.owner_ref()?, command?, &pairs)?;
        e.state().command.reply = reply;
        Ok(())
    })
}

/// Writes to `is` whether event `n` of the last take is of the event type with the key `event_type` (`mesh.text`).
/// Returns `SDRKIT_E_NO_SUCH_ID` if the event does not exist. The typed function of the event type in the header of its
/// family (`sdrkit_fm_text_event_read`) is the first choice: it returns `SDRKIT_E_WRONG_TYPE` for another type and fills
/// a struct.
#[no_mangle]
pub unsafe extern "C" fn sdrkit_event_is(
    handle: Handle,
    n: u32,
    event_type: *const c_char,
    is: *mut bool,
) -> i32 {
    // SAFETY: the caller's contract.
    let (key, out) = unsafe { (str_in(event_type), Out::new(is)) };
    run(handle, |e| {
        let (key, out) = (key?, out?);
        let st = e.state();
        out.put(event_at(&st.events, n)?.kind() == key);
        Ok(())
    })
}

/// Writes the field with the key `key` of event `n` of the last take to `text`, as text for people. The text is empty if
/// the event has no such field or the field has no value. The string is valid until the next call on `handle` that
/// writes a string. Returns `SDRKIT_E_NO_SUCH_ID` if the event does not exist. The typed function of the event type
/// (`sdrkit_fm_text_event_read`) is the first choice: it gives each field with its type.
#[no_mangle]
pub unsafe extern "C" fn sdrkit_event_text(
    handle: Handle,
    n: u32,
    key: *const c_char,
    text: *mut Str,
) -> i32 {
    // SAFETY: the caller's contract.
    let (key, out) = unsafe { (str_in(key), Out::new(text)) };
    run(handle, |e| {
        let (key, out) = (key?, out?);
        let mut st = e.state();
        let t = event_at(&st.events, n)?.text(key);
        st.string = t;
        out.put(Str::of(&st.string));
        Ok(())
    })
}

/// Writes what is wrong with the session of `session` now to `text`, as sentences: the problem of its receiver, the last
/// refused request, and what stops it from transmitting (the transmit switch is off, the transmitter is muted). The text
/// is empty if nothing is wrong. Returns `SDRKIT_E_WRONG_HANDLE` if `session` is not a session. `sdrkit_session_fields_read`
/// gives the same problems as separate members.
/// The string is a view that the library owns, valid until the next call on `session`. Copy what you keep.
#[no_mangle]
pub unsafe extern "C" fn sdrkit_problem(session: Handle, text: *mut Str) -> i32 {
    // SAFETY: the caller's contract.
    let out = unsafe { Out::new(text) };
    run(session, |e| {
        let out = out?;
        let OwnerRef::Session(id) = e.owner_ref()? else {
            return Err(Error::WrongHandle);
        };
        let p = e.host.engine.problem(id)?;
        let mut st = e.state();
        st.string = p.unwrap_or_default();
        out.put(Str::of(&st.string));
        Ok(())
    })
}
