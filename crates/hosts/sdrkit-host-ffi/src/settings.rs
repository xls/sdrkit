//! The values of the settings of an object, and the saved settings of the whole engine.

use crate::error::Error;
use crate::handle::{run, slice_in, Handle, Out};
use crate::value::{small, Bytes, Id, Variant};
use sdrkit_core::{SettingId, Value};
use sdrkit_host_engine::EngineError;

/// Writes the value of setting `id` of the object of `handle` to `value`. For a type of a catalog, the value is the default.
/// Returns `SDRKIT_E_NO_SUCH_ID` if the setting does not exist. For a setting that a family header declares, its typed
/// function (`sdrkit_<stack>_get_<key>`) is the first choice. This call is for a program that shows any setting from its
/// declaration.
/// A string or bytes in `value` is a view that the library owns, valid until the next call on `handle`. Copy what you keep.
#[no_mangle]
pub unsafe extern "C" fn sdrkit_setting_get(handle: Handle, id: Id, value: *mut Variant) -> i32 {
    // SAFETY: the caller's contract.
    let out = unsafe { Out::new(value) };
    run(handle, |e| {
        let out = out?;
        let mut st = e.state();
        let id = SettingId(small(id)?);
        let found = e.owner(|o| {
            let v: Option<&Value> = match o.values() {
                Some(v) => v.value(id),
                None => o.schema().setting(id).map(|s| &s.default),
            };
            v.map(|v| st.value.clone_from(v)).is_some()
        })?;
        if !found {
            return Err(Error::NoSuchId);
        }
        out.put(Variant::of(&st.value));
        Ok(())
    })
}

/// Sets setting `id` of the object of `handle` to `value`. The value must match the declaration: its type, range and
/// options. A refused value changes nothing. Returns a setting code on error: `SDRKIT_E_WRONG_TYPE`,
/// `SDRKIT_E_OUT_OF_RANGE`, `SDRKIT_E_NOT_AN_OPTION` and others. For a setting that a family header declares, its typed
/// function (`sdrkit_<stack>_set_<key>`) is the first choice.
#[no_mangle]
pub unsafe extern "C" fn sdrkit_setting_set(handle: Handle, id: Id, value: *const Variant) -> i32 {
    // SAFETY: the caller's contract.
    let value = unsafe { value.as_ref().ok_or(Error::Null) };
    run(handle, |e| {
        // SAFETY: the caller's contract on the views of `value`.
        let v = unsafe { value?.to_value()? };
        let id = SettingId(small(id)?);
        e.owner(|o| {
            sdrkit_core::owner::apply(o, id, &v)
                .map_err(|err| Error::Engine(EngineError::setting(o.schema(), err)))
        })?
    })
}

/// Writes the settings of every object of the engine to `blob` as saved settings: bytes that the host stores as they are
/// and gives to `sdrkit_settings_import` in a later run. Only the library reads them. `transmit_allowed` is not saved.
/// The bytes are a view that the library owns, valid until the next call on `handle`. Copy what you keep.
#[no_mangle]
pub unsafe extern "C" fn sdrkit_settings_export(handle: Handle, blob: *mut Bytes) -> i32 {
    // SAFETY: the caller's contract.
    let out = unsafe { Out::new(blob) };
    run(handle, |e| {
        let out = out?;
        let saved = e.host.engine.export_settings()?;
        let mut st = e.state();
        st.bytes = saved;
        out.put(Bytes::of(&st.bytes));
        Ok(())
    })
}

/// Applies the `size` bytes at `data` that `sdrkit_settings_export` wrote, in this run or an earlier one. An object or a
/// setting that the engine does not have now is skipped. A setting of a saved object that the bytes do not hold takes its
/// default. Returns `SDRKIT_E_VERSION` if a later version of the library wrote the bytes, and `SDRKIT_E_IMPORT` if they
/// are not saved settings, are damaged, or hold a value that a setting refuses; the call then changes nothing.
#[no_mangle]
pub unsafe extern "C" fn sdrkit_settings_import(handle: Handle, data: *const u8, size: usize) -> i32 {
    // SAFETY: the caller's contract.
    let data = unsafe { slice_in(data, size) };
    run(handle, |e| Ok(e.host.engine.import_settings(data?)?))
}
