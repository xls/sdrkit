//! The commands of an object: their declarations, a call with arguments, and its reply.

use crate::error::Error;
use crate::handle::{run, slice_in, str_in, Handle, Out};
use crate::value::{count, small, Arg, Id, Str, Variant};
use sdrkit_core::{CommandDef, CommandId, FieldId, Record, TxContent};
use sdrkit_host_engine::EngineError;
use std::ffi::c_char;

/// The declaration of one command.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct CommandInfo {
    /// The number of bytes of the struct. The caller sets it to the sizeof of its struct before the call; the call sets it to
    /// the number of bytes that it wrote.
    pub size: u32,
    /// The key.
    pub key: Str,
    /// The name for people.
    pub label: Str,
    /// One or two sentences for people.
    pub help: Str,
    /// The number of arguments. Argument ids go from 0 to `arguments` - 1.
    pub arguments: u32,
    /// The number of reply fields. Most commands have none.
    pub reply_fields: u32,
    /// The command transmits. The engine refuses it with `SDRKIT_E_TRANSMIT_OFF` while the transmitter of the session is
    /// off.
    pub transmits: bool,
    /// What the command sends on the air, one of `SDRKIT_TX_CONTENT_*`. -1 if the command does not transmit.
    pub content: i32,
    /// The key of the command that stops the transmission that this command starts. Empty for a transmission that ends by
    /// itself and for a command that does not transmit.
    pub stopped_by: Str,
}

/// A command sends a text message: a value of the member `content` of `sdrkit_command_info`.
pub const SDRKIT_TX_CONTENT_TEXT: i32 = 0;
/// A command sends speech.
pub const SDRKIT_TX_CONTENT_VOICE: i32 = 1;
/// A command sends a beacon or an announcement of the station.
pub const SDRKIT_TX_CONTENT_BEACON: i32 = 2;
/// A command sends a file, a picture or a stream of media.
pub const SDRKIT_TX_CONTENT_FILE: i32 = 3;
/// A command sends a frame or a request of its protocol.
pub const SDRKIT_TX_CONTENT_FRAME: i32 = 4;
/// A command sends an unmodulated carrier, to tune an antenna or an amplifier.
pub const SDRKIT_TX_CONTENT_CARRIER: i32 = 5;

/// The `SDRKIT_TX_CONTENT_*` constant of `c`.
fn content(c: TxContent) -> i32 {
    match c {
        TxContent::Text => SDRKIT_TX_CONTENT_TEXT,
        TxContent::Voice => SDRKIT_TX_CONTENT_VOICE,
        TxContent::Beacon => SDRKIT_TX_CONTENT_BEACON,
        TxContent::File => SDRKIT_TX_CONTENT_FILE,
        TxContent::Frame => SDRKIT_TX_CONTENT_FRAME,
        TxContent::Carrier => SDRKIT_TX_CONTENT_CARRIER,
    }
}

/// The last command declaration and the last reply of a handle.
#[derive(Default)]
pub(crate) struct Called {
    def: Option<CommandDef>,
    pub(crate) reply: Option<Record>,
}

/// Writes the number of commands of the object of `handle` to `count`. Command ids go from 0 to `count` - 1.
#[no_mangle]
pub unsafe extern "C" fn sdrkit_commands_count(handle: Handle, count: *mut u32) -> i32 {
    // SAFETY: the caller's contract.
    let out = unsafe { Out::new(count) };
    run(handle, |e| {
        let out = out?;
        out.put(self::count(e.owner(|o| o.commands().len())?));
        Ok(())
    })
}

/// Writes the id of the command with the key `key` to `id`. Returns `SDRKIT_E_NO_SUCH_KEY` if no command has the key.
#[no_mangle]
pub unsafe extern "C" fn sdrkit_command_find(
    handle: Handle,
    key: *const c_char,
    id: *mut Id,
) -> i32 {
    // SAFETY: the caller's contract.
    let (key, out) = unsafe { (str_in(key), Out::new(id)) };
    run(handle, |e| {
        let (key, out) = (key?, out?);
        let id = e.owner(|o| o.commands().id(key))?.map_err(|error| {
            Error::Engine(EngineError::Command {
                text: error.to_string(),
                error,
            })
        })?;
        out.put(Id::from(id.0));
        Ok(())
    })
}

/// Fills `info` with the declaration of command `id`. Returns `SDRKIT_E_NO_SUCH_ID` if the command does not exist.
/// The strings of `info` are views that the library owns, valid until the next call on `handle`. Copy what you keep.
#[no_mangle]
pub unsafe extern "C" fn sdrkit_command_decl(
    handle: Handle,
    id: Id,
    info: *mut CommandInfo,
) -> i32 {
    // SAFETY: the caller's contract.
    let out = unsafe { Out::new(info) };
    run(handle, |e| {
        let out = out?;
        let id = CommandId(small(id)?);
        let def = e
            .owner(|o| o.commands().def(id).cloned())?
            .ok_or(Error::NoSuchId)?;
        let mut st = e.state();
        let d = &*st.command.def.insert(def);
        out.put(CommandInfo {
            size: 0,
            key: Str::of(&d.key),
            label: Str::of(&d.label),
            help: Str::of(&d.help),
            arguments: count(d.args.len()),
            reply_fields: count(d.reply.len()),
            transmits: d.transmits.is_some(),
            content: d.transmits.map_or(-1, content),
            stopped_by: Str::of(d.stopped_by.as_deref().unwrap_or("")),
        });
        Ok(())
    })
}

/// Runs command `id` of the object of `handle` with the `count` arguments in `args`, and writes the number of reply fields
/// to `reply_fields`. An argument that is not in `args` has its default value. The call checks the arguments against the
/// declaration before the command runs. `sdrkit_reply` reads the reply. Returns `SDRKIT_E_NO_SUCH_ID`,
/// `SDRKIT_E_MISSING_ARGUMENT`, a setting code for a refused argument (the error detail holds its id), `SDRKIT_E_NOT_NOW`,
/// `SDRKIT_E_NO_SUCH_ROW`, `SDRKIT_E_TRANSMIT_OFF` or `SDRKIT_E_COMMAND_FAILED`. For a command that a family header declares, its typed function
/// (`sdrkit_<stack>_<command>`) is the first choice.
#[no_mangle]
pub unsafe extern "C" fn sdrkit_command_invoke(
    handle: Handle,
    id: Id,
    args: *const Arg,
    count: usize,
    reply_fields: *mut u32,
) -> i32 {
    // SAFETY: the caller's contract.
    let (args, out) = unsafe { (slice_in(args, count), Out::new(reply_fields)) };
    run(handle, |e| {
        let (args, out) = (args?, out?);
        let id = CommandId(small(id)?);
        let mut given = Vec::with_capacity(args.len());
        for a in args {
            // SAFETY: the caller's contract on the views of `args`.
            given.push((FieldId(small(a.id)?), unsafe { a.value.to_value()? }));
        }
        let got = e.owner(|o| {
            let def = o.commands().def(id).ok_or(Error::NoSuchId)?;
            let mut rec = def.new_args();
            for (f, v) in &given {
                rec.set_value(*f, v).map_err(Error::Record)?;
            }
            sdrkit_core::owner::invoke(o, id, &rec)
                .map_err(|err| Error::Engine(EngineError::command(o, id, err)))
        })??;
        let n = got.as_ref().map_or(0, |r| r.decl().len());
        e.state().command.reply = got;
        out.put(self::count(n));
        Ok(())
    })
}

/// Writes reply field `id` of the last command that ran on `handle` to `value`. Returns `SDRKIT_E_NO_SUCH_ID` if the field
/// does not exist.
/// A string or bytes in `value` is a view that the library owns, valid until the next call on `handle`. Copy what you keep.
#[no_mangle]
pub unsafe extern "C" fn sdrkit_reply(handle: Handle, id: Id, value: *mut Variant) -> i32 {
    // SAFETY: the caller's contract.
    let out = unsafe { Out::new(value) };
    run(handle, |e| {
        let out = out?;
        let id = FieldId(small(id)?);
        let st = e.state();
        let v = st
            .command
            .reply
            .as_ref()
            .and_then(|r| r.value(id))
            .ok_or(Error::NoSuchId)?;
        out.put(Variant::of(v));
        Ok(())
    })
}

crate::handle::sized_structs!(CommandInfo);

#[cfg(test)]
mod transmit_tests {
    use super::*;
    use sdrkit_core::Choice;

    /// The `SDRKIT_TX_CONTENT_*` numbers are the indexes of the options of [`TxContent`], as the enums of the family
    /// headers number them.
    #[test]
    fn the_content_numbers_are_the_option_indexes() {
        let mut n = 0;
        while let Some(c) = TxContent::from_index(n) {
            assert_eq!(content(c), i32::from(n), "{}", c.key());
            n += 1;
        }
        assert_eq!(n, 6);
    }
}
