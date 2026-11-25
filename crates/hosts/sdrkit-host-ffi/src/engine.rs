//! The engine calls, the object handles and the lifecycle of radios and sessions.

use crate::error::Error;
use crate::handle::{
    get, insert, optional_in, remove, run, run_free, sized_in, slice_in, str_in, Entry, Handle,
    Host, Out, What,
};
use crate::value::{count, Arg, Str};
use sdrkit_core::{Facilities, Schema, SettingId, Values};
use sdrkit_host_engine::{
    split_address, Engine, EngineError, Input as EngineInput, OwnerRef, Setup, StdNet,
};
use std::ffi::c_char;
use std::sync::{Arc, Mutex};

/// The engine: its settings, the catalogs and the tables of radios and sessions.
pub const SDRKIT_OBJECT_ENGINE: i32 = 0;
/// An open radio.
pub const SDRKIT_OBJECT_RADIO: i32 = 1;
/// A session.
pub const SDRKIT_OBJECT_SESSION: i32 = 2;
/// The stack of a stack session.
pub const SDRKIT_OBJECT_STACK: i32 = 3;
/// A component of the graph of a session.
pub const SDRKIT_OBJECT_COMPONENT: i32 = 4;
/// A stack type of the catalog.
pub const SDRKIT_OBJECT_STACK_TYPE: i32 = 5;
/// A driver type of the catalog.
pub const SDRKIT_OBJECT_DRIVER_TYPE: i32 = 6;
/// A component type of the catalog.
pub const SDRKIT_OBJECT_COMPONENT_TYPE: i32 = 7;
/// A service.
pub const SDRKIT_OBJECT_SERVICE: i32 = 8;
/// A graph that the caller builds. It is not an object.
pub const SDRKIT_OBJECT_GRAPH: i32 = 9;
/// A host feed that `sdrkit_feed_open` opened. It is not an object.
pub const SDRKIT_OBJECT_FEED: i32 = 10;
/// A share of a radio: the samples of the radio served to network clients.
pub const SDRKIT_OBJECT_SHARE: i32 = 11;

/// The session reads the open radio `radio`.
pub const SDRKIT_INPUT_RADIO: i32 = 0;
/// The session reads the host feed `feed`: the samples that the caller pushes with `sdrkit_samples_push`.
pub const SDRKIT_INPUT_HOST: i32 = 1;
/// The session reads no samples: a component of the graph makes the signal.
pub const SDRKIT_INPUT_INSIDE: i32 = 2;

/// The input of a new session. Make it with `sdrkit_input_radio`, `sdrkit_input_host` or `sdrkit_input_inside`.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct Input {
    /// The number of bytes of the struct. The caller sets it to the sizeof of its struct.
    pub size: u32,
    /// An `SDRKIT_INPUT_*` constant.
    pub r#type: i32,
    /// For `SDRKIT_INPUT_RADIO`: the handle of the radio. For `SDRKIT_INPUT_HOST`: the handle of the feed.
    pub radio: Handle,
    /// Not read. The rate of a host feed is set by `sdrkit_feed_open`.
    pub rate_hz: f64,
    /// Not read. The centre of a host feed is set by `sdrkit_feed_open`.
    pub centre_hz: f64,
}

/// The options of `sdrkit_stack_start`. All members 0 selects the defaults.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct StackOptions {
    /// The number of bytes of the struct. The caller sets it to the sizeof of its struct.
    pub size: u32,
    /// The submode of the stack, a NUL-terminated string. NULL for the default submode.
    pub submode: *const c_char,
    /// `count` values of the settings of the object `stack.<id>`. The other settings keep their defaults.
    pub settings: *const Arg,
    /// The number of entries in `settings`.
    pub count: usize,
    /// On a radio: the frequency to tune the session to, Hz. 0 selects the default frequency of the stack or submode.
    pub centre_hz: f64,
}

/// The options of `sdrkit_radio_open`. All members 0 selects the defaults.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct RadioOptions {
    /// The number of bytes of the struct. The caller sets it to the sizeof of its struct.
    pub size: u32,
    /// `count` values of the settings that an open of the address takes, by the ids of `sdrkit_radio_open_setting_find()`:
    /// those of the object `driver.<key>` with the same ids, then those that the radio at the address adds, such as the
    /// settings of the test signal of a stack. The other settings keep their defaults.
    pub settings: *const Arg,
    /// The number of entries in `settings`.
    pub count: usize,
}

/// Makes an engine with `facilities`, the codec plug-ins of the workspace and the USB bus of the system, and returns its
/// handle. On a platform without USB access the bus has no devices and says so.
pub(crate) fn make_engine(facilities: Facilities) -> Result<Handle, Error> {
    make_engine_on(facilities, sdrkit_driver_usb::platform_bus())
}

/// As [`make_engine`], with the USB radios of the bus `usb`.
pub(crate) fn make_engine_on(
    mut facilities: Facilities,
    usb: Arc<dyn sdrkit_driver_usb::UsbBus>,
) -> Result<Handle, Error> {
    facilities.codecs = Arc::new(sdrkit_host_catalog::codecs());
    let engine = Engine::with_usb(
        Setup {
            components: sdrkit_host_catalog::registry(&facilities),
            facilities,
            transport: Arc::new(StdNet),
            stacks: sdrkit_host_catalog::all(),
            families: sdrkit_host_catalog::families(),
        },
        usb,
    )?;
    let host = Arc::new(Host {
        engine,
        last: Mutex::default(),
    });
    Ok(insert(Entry {
        host,
        what: What::Engine,
        state: Mutex::default(),
        audio_in: None,
    }))
}

/// A new handle of object `r` of the engine of `e`.
pub(crate) fn object_handle(e: &Entry, r: OwnerRef) -> Handle {
    let audio_in = match r {
        OwnerRef::Session(id) => e.host.engine.audio_input(id).ok(),
        _ => None,
    };
    insert(Entry {
        host: Arc::clone(&e.host),
        what: What::Object(r),
        state: Mutex::default(),
        audio_in,
    })
}

/// The `SDRKIT_OBJECT_*` constant of `r`.
fn object_type(r: OwnerRef) -> i32 {
    match r {
        OwnerRef::Engine => SDRKIT_OBJECT_ENGINE,
        OwnerRef::Radio(_) => SDRKIT_OBJECT_RADIO,
        OwnerRef::Session(_) => SDRKIT_OBJECT_SESSION,
        OwnerRef::Stack(_) => SDRKIT_OBJECT_STACK,
        OwnerRef::Component(..) => SDRKIT_OBJECT_COMPONENT,
        OwnerRef::StackKind(_) => SDRKIT_OBJECT_STACK_TYPE,
        OwnerRef::DriverKind(_) => SDRKIT_OBJECT_DRIVER_TYPE,
        OwnerRef::ComponentKind(_) => SDRKIT_OBJECT_COMPONENT_TYPE,
        OwnerRef::Service(_) => SDRKIT_OBJECT_SERVICE,
        OwnerRef::Share(_) => SDRKIT_OBJECT_SHARE,
    }
}

/// The values of the settings of the object `type_key` (`stack.<id>`): its defaults with `args` applied.
unsafe fn type_values(e: &Entry, type_key: &str, args: &[Arg]) -> Result<Values, Error> {
    let engine = &e.host.engine;
    let r = engine.find(type_key)?;
    let schema = engine.owner(r, |o| Arc::clone(o.schema()))?;
    // SAFETY: the caller's contract on the views of `args`.
    unsafe { schema_values(&schema, args) }
}

/// The values of `schema`: its defaults with `args` applied.
unsafe fn schema_values(schema: &Arc<Schema>, args: &[Arg]) -> Result<Values, Error> {
    let mut v = Values::defaults(schema);
    for a in args {
        // SAFETY: the caller's contract on the views of `args`.
        let value = unsafe { a.value.to_value()? };
        let id = SettingId(crate::value::small(a.id)?);
        v.set(id, &value)
            .map_err(|err| Error::Engine(EngineError::setting(schema, err)))?;
    }
    Ok(v)
}

/// The engine's input of `input`.
fn input_of(e: &Entry, input: &Input) -> Result<EngineInput, Error> {
    match input.r#type {
        SDRKIT_INPUT_RADIO => {
            let radio = get(input.radio).ok_or(Error::Engine(EngineError::NoRadio))?;
            match radio.what {
                What::Object(OwnerRef::Radio(id)) if Arc::ptr_eq(&radio.host, &e.host) => {
                    Ok(EngineInput::Radio(id))
                }
                _ => Err(Error::WrongHandle),
            }
        }
        SDRKIT_INPUT_HOST => {
            let feed = get(input.radio).ok_or(Error::WrongHandle)?;
            match feed.what {
                What::Feed(id) if Arc::ptr_eq(&feed.host, &e.host) => Ok(EngineInput::Feed(id)),
                _ => Err(Error::WrongHandle),
            }
        }
        SDRKIT_INPUT_INSIDE => Ok(EngineInput::Inside),
        _ => Err(Error::BadArgument),
    }
}

/// The session of a session handle.
pub(crate) fn session_of(e: &Entry) -> Result<sdrkit_host_engine::SessionId, Error> {
    match e.owner_ref()? {
        OwnerRef::Session(id) => Ok(id),
        _ => Err(Error::WrongHandle),
    }
}

/// Writes the version of the library, `major.minor.patch`, to `version`.
/// The string belongs to the library and is valid while the library is loaded.
#[no_mangle]
pub unsafe extern "C" fn sdrkit_version(version: *mut Str) -> i32 {
    // SAFETY: the caller's contract.
    let out = unsafe { Out::new(version) };
    run_free(|| {
        out?.put(Str::of(env!("CARGO_PKG_VERSION")));
        Ok(())
    })
}

/// Makes an engine that keeps its files in the folder `folder`, and writes its handle to `engine`. The call makes the
/// folder if it does not exist. If `folder` is NULL, the engine keeps its files in memory, and they are lost when the
/// engine is freed. Returns `SDRKIT_E_NOT_FOUND`, `SDRKIT_E_DENIED` or another storage code if it cannot make the folder.
/// On error, `sdrkit_error_message(0, ...)` returns the message.
#[no_mangle]
pub unsafe extern "C" fn sdrkit_engine_new(folder: *const c_char, engine: *mut Handle) -> i32 {
    // SAFETY: the caller's contract.
    let (folder, out) = unsafe {
        let folder = (!folder.is_null()).then(|| str_in(folder)).transpose();
        (folder, Out::new(engine))
    };
    run_free(|| {
        let out = out?;
        let f = match folder? {
            Some(folder) => {
                sdrkit_host_engine::system_facilities(folder).map_err(Error::Storage)?
            }
            None => sdrkit_host_engine::memory_facilities(),
        };
        out.put(make_engine(f)?);
        Ok(())
    })
}

/// Frees `handle`. Freeing the engine stops every session, closes every radio and frees every handle of the engine. Freeing
/// an object handle frees the handle only: a session runs until `sdrkit_session_stop`. Freeing a graph frees the graph.
#[no_mangle]
pub extern "C" fn sdrkit_free(handle: Handle) -> i32 {
    run_free(|| {
        if remove(handle) {
            Ok(())
        } else {
            Err(Error::Engine(EngineError::NoSuchOwner(OwnerRef::Engine)))
        }
    })
}

/// Writes a new handle of the object with the key `key` to `object`. `handle` is any handle of the engine. Returns
/// `SDRKIT_E_BAD_KEY` if `key` is not an object key, `SDRKIT_E_NO_SUCH_KEY` if no object has the key.
#[no_mangle]
pub unsafe extern "C" fn sdrkit_object_find(
    handle: Handle,
    key: *const c_char,
    object: *mut Handle,
) -> i32 {
    // SAFETY: the caller's contract.
    let (key, out) = unsafe { (str_in(key), Out::new(object)) };
    run(handle, |e| {
        let (key, out) = (key?, out?);
        let r = e.host.engine.find(key)?;
        out.put(object_handle(e, r));
        Ok(())
    })
}

/// Writes a new handle of the stack of the stack session `session` to `object`. Returns `SDRKIT_E_WRONG_HANDLE` if `session`
/// is not a session.
#[no_mangle]
pub unsafe extern "C" fn sdrkit_object_stack(session: Handle, object: *mut Handle) -> i32 {
    // SAFETY: the caller's contract.
    let out = unsafe { Out::new(object) };
    run(session, |e| {
        let out = out?;
        let id = session_of(e)?;
        out.put(object_handle(e, OwnerRef::Stack(id)));
        Ok(())
    })
}

/// Writes a new handle of the component named `component` of the graph of `session` to `object`. Returns
/// `SDRKIT_E_WRONG_HANDLE` if `session` is not a session, `SDRKIT_E_NO_SUCH_KEY` if its graph has no component of that name.
#[no_mangle]
pub unsafe extern "C" fn sdrkit_object_component(
    session: Handle,
    component: *const c_char,
    object: *mut Handle,
) -> i32 {
    // SAFETY: the caller's contract.
    let (name, out) = unsafe { (str_in(component), Out::new(object)) };
    run(session, |e| {
        let (name, out) = (name?, out?);
        let id = session_of(e)?;
        let c = e.host.engine.component(id, name)?;
        out.put(object_handle(e, OwnerRef::Component(id, c)));
        Ok(())
    })
}

/// Writes the key of the object of `handle` to `key`, as `sdrkit_object_find` reads it. The key is empty for a stack or a
/// component of a session and for a graph.
/// The string is a view that the library owns, valid until the next call on `handle`. Copy what you keep.
#[no_mangle]
pub unsafe extern "C" fn sdrkit_object_key(handle: Handle, key: *mut Str) -> i32 {
    // SAFETY: the caller's contract.
    let out = unsafe { Out::new(key) };
    run(handle, |e| {
        let out = out?;
        let k = match e.what {
            What::Graph | What::Feed(_) => None,
            _ => e.host.engine.key_of(e.owner_ref()?),
        };
        let mut st = e.state();
        st.string = k.unwrap_or_default();
        out.put(Str::of(&st.string));
        Ok(())
    })
}

/// Writes the type of the object of `handle`, an `SDRKIT_OBJECT_*` constant, to `object_type`.
#[no_mangle]
pub unsafe extern "C" fn sdrkit_object_type(handle: Handle, object_type: *mut i32) -> i32 {
    // SAFETY: the caller's contract.
    let out = unsafe { Out::new(object_type) };
    run(handle, |e| {
        out?.put(match e.what {
            What::Engine => SDRKIT_OBJECT_ENGINE,
            What::Graph => SDRKIT_OBJECT_GRAPH,
            What::Feed(_) => SDRKIT_OBJECT_FEED,
            What::Object(r) => self::object_type(r),
        });
        Ok(())
    })
}

/// The keys of the live objects of the engine: the engine, the radios, the sessions, the services, then the stack, driver
/// and component types.
fn object_keys(e: &Entry) -> Result<Vec<String>, Error> {
    use sdrkit_host_engine::{
        COMPONENT_KINDS, DRIVERS, RADIOS, SERVICES, SESSIONS, SHARES, STACKS,
    };
    let lists = [
        (&RADIOS, ""),
        (&SESSIONS, ""),
        (&*SHARES, ""),
        (&SERVICES, ""),
        (&STACKS, "stack."),
        (&DRIVERS, "driver."),
        (&COMPONENT_KINDS, "component."),
    ];
    Ok(e.host.engine.owner(OwnerRef::Engine, |o| {
        let res = o.results();
        let mut keys = vec!["engine".to_owned()];
        for (def, prefix) in lists {
            let Some(t) = res.decl().table_id(def.key).and_then(|id| res.table(id)) else {
                continue;
            };
            let mut rows: Vec<String> = t
                .rows_newest_first()
                .map(|(k, _)| format!("{prefix}{k}"))
                .collect();
            rows.reverse();
            keys.extend(rows);
        }
        keys
    })?)
}

/// Takes a list of the live objects of the engine of `handle` and writes their number to `count`: the engine, the radios,
/// the sessions, the shares, the services, then the stack, driver and component types. `sdrkit_object` reads the entries.
#[no_mangle]
pub unsafe extern "C" fn sdrkit_objects(handle: Handle, count: *mut u32) -> i32 {
    // SAFETY: the caller's contract.
    let out = unsafe { Out::new(count) };
    run(handle, |e| {
        let out = out?;
        let engine = &e.host.engine;
        let mut list = Vec::new();
        for key in object_keys(e)? {
            if let Ok(r) = engine.find(&key) {
                list.push((key, object_type(r)));
            }
        }
        out.put(self::count(list.len()));
        e.state().objects = list;
        Ok(())
    })
}

/// Writes the key and the `SDRKIT_OBJECT_*` type of entry `n` of the object list of the handle to `key` and `object_type`.
/// Returns `SDRKIT_E_NO_SUCH_ID` if `n` is not in the list.
/// The string is a view that the library owns, valid until the next call on `handle`. Copy what you keep.
#[no_mangle]
pub unsafe extern "C" fn sdrkit_object(
    handle: Handle,
    n: u32,
    key: *mut Str,
    object_type: *mut i32,
) -> i32 {
    // SAFETY: the caller's contract.
    let (key, t) = unsafe { (Out::new(key), Out::new(object_type)) };
    run(handle, |e| {
        let (key, t) = (key?, t?);
        let st = e.state();
        let (k, ty) = usize::try_from(n)
            .ok()
            .and_then(|n| st.objects.get(n))
            .ok_or(Error::NoSuchId)?;
        key.put(Str::of(k));
        t.put(*ty);
        Ok(())
    })
}

/// Returns the input of a session that reads the open radio `radio`.
#[no_mangle]
pub extern "C" fn sdrkit_input_radio(radio: Handle) -> Input {
    Input {
        r#type: SDRKIT_INPUT_RADIO,
        radio,
        ..Input::default()
    }
}

/// Returns the input of a session that reads the host feed `feed`. Any number of sessions can read one feed.
#[no_mangle]
pub extern "C" fn sdrkit_input_host(feed: Handle) -> Input {
    Input {
        r#type: SDRKIT_INPUT_HOST,
        radio: feed,
        ..Input::default()
    }
}

/// Opens a host feed of complex samples at `rate_hz` around `centre_hz` and writes its handle to `feed`. Sessions read it
/// with `sdrkit_input_host`, and the caller writes it with `sdrkit_samples_push`. `sdrkit_free` closes it.
#[no_mangle]
pub unsafe extern "C" fn sdrkit_feed_open(
    engine: Handle,
    rate_hz: f64,
    centre_hz: f64,
    feed: *mut Handle,
) -> i32 {
    // SAFETY: the caller's contract.
    let out = unsafe { Out::new(feed) };
    run(engine, |e| {
        let out = out?;
        let id = e.host.engine.open_feed(rate_hz, centre_hz);
        out.put(insert(Entry {
            host: Arc::clone(&e.host),
            what: What::Feed(id),
            state: Mutex::default(),
            audio_in: None,
        }));
        Ok(())
    })
}

/// Returns the input of a session that reads no samples, for a graph whose component makes the signal.
#[no_mangle]
pub extern "C" fn sdrkit_input_inside() -> Input {
    Input {
        r#type: SDRKIT_INPUT_INSIDE,
        radio: 0,
        ..Input::default()
    }
}

/// Opens the radio at `address` and writes its handle to `radio`. The scheme of the address selects the driver:
/// `rtl_tcp:<host>:<port>`, `synth:<stack>` for a test signal, `file:<path>` for a recording in the folder of the engine.
/// `options` is NULL for the defaults; its settings are those that `sdrkit_radio_open_setting_decl()` declares for the
/// address. Returns `SDRKIT_E_NO_SUCH_KEY` for an unknown driver and for the test signal of an
/// unknown stack (the member `key` of the error detail holds the driver key or the stack id), a setting code for a
/// refused setting, `SDRKIT_E_RADIO_OPEN` if a radio of this address is open and the device has one receiver, or
/// `SDRKIT_E_DRIVER`.
#[no_mangle]
pub unsafe extern "C" fn sdrkit_radio_open(
    engine: Handle,
    address: *const c_char,
    options: *const RadioOptions,
    radio: *mut Handle,
) -> i32 {
    // SAFETY: the caller's contract.
    let (address, settings, out) = unsafe {
        let settings = match optional_in(options) {
            Ok(Some(o)) => slice_in(o.settings, o.count),
            Ok(None) => Ok(&[][..]),
            Err(e) => Err(e),
        };
        (str_in(address), settings, Out::new(radio))
    };
    run(engine, |e| {
        let (address, settings, out) = (address?, settings?, out?);
        let (driver, at) = split_address(address);
        let schema = e.host.engine.radio_schema(driver, at)?;
        // SAFETY: the caller's contract on the views of `settings`.
        let values = unsafe { schema_values(&schema, settings)? };
        let id = e.host.engine.open_radio(driver, at, &values)?;
        out.put(object_handle(e, OwnerRef::Radio(id)));
        Ok(())
    })
}

/// Stops and closes the radio of `radio`. The share of the radio is closed first. Returns `SDRKIT_E_RADIO_IN_USE` while
/// sessions read it.
#[no_mangle]
pub extern "C" fn sdrkit_radio_close(radio: Handle) -> i32 {
    run(radio, |e| match e.owner_ref()? {
        OwnerRef::Radio(id) => Ok(e.host.engine.close_radio(id)?),
        _ => Err(Error::WrongHandle),
    })
}

/// Opens the share of the radio `radio` and writes its handle to `share`: an object with the key `share.h<n>` that serves
/// the samples of the radio to network clients. Its command `listen` starts a listener for rtl_tcp or SoapyRemote, and its
/// settings say who steers the radio and which addresses may connect; `include/sdrkit/share.h` has its typed calls. The
/// share never sets the rate or the frequency of the radio by itself and never stops a session from doing so. No client
/// transmits until the command `grant` of the share lets it and the engine setting `transmit_allowed` is on. Returns
/// `SDRKIT_E_WRONG_HANDLE` if `radio` is not a radio, `SDRKIT_E_SHARE_OPEN` if the radio has a share.
#[no_mangle]
pub unsafe extern "C" fn sdrkit_share_open(radio: Handle, share: *mut Handle) -> i32 {
    // SAFETY: the caller's contract.
    let out = unsafe { Out::new(share) };
    run(radio, |e| {
        let out = out?;
        let OwnerRef::Radio(id) = e.owner_ref()? else {
            return Err(Error::WrongHandle);
        };
        let s = e.host.engine.open_share(id)?;
        out.put(object_handle(e, OwnerRef::Share(s)));
        Ok(())
    })
}

/// Closes the share of `share`: its listeners stop and its clients are disconnected. Returns `SDRKIT_E_WRONG_HANDLE` if
/// `share` is not a share.
#[no_mangle]
pub extern "C" fn sdrkit_share_close(share: Handle) -> i32 {
    run(share, |e| match e.owner_ref()? {
        OwnerRef::Share(id) => Ok(e.host.engine.close_share(id)?),
        _ => Err(Error::WrongHandle),
    })
}

/// Starts a session of the stack `stack` on `input`, and writes its handle to `session`. `options` is NULL for the defaults.
/// On a radio, the session is tuned to `options->centre_hz` or to the default frequency of the stack. If no other session
/// reads the radio, the radio is first set to the stack's sample rate and to that frequency. A radio with one fixed
/// frequency (a recording) is not tuned. Returns `SDRKIT_E_BAD_ARGUMENT` for a negative or not finite `centre_hz`,
/// `SDRKIT_E_NO_SUCH_KEY` for an unknown stack, a setting code for a refused setting or centre, `SDRKIT_E_CONFIG`, or
/// `SDRKIT_E_NO_SUCH_OBJECT` for a closed radio.
#[no_mangle]
pub unsafe extern "C" fn sdrkit_stack_start(
    engine: Handle,
    stack: *const c_char,
    options: *const StackOptions,
    input: *const Input,
    session: *mut Handle,
) -> i32 {
    // SAFETY: the caller's contract.
    let (stack, submode, settings, centre_hz, input, out) = unsafe {
        let (submode, settings, centre_hz) = match optional_in(options) {
            Ok(Some(o)) => (
                (!o.submode.is_null())
                    .then(|| str_in(o.submode))
                    .transpose(),
                slice_in(o.settings, o.count),
                o.centre_hz,
            ),
            Ok(None) => (Ok(None), Ok(&[][..]), 0.0),
            Err(e) => (Ok(None), Err(e), 0.0),
        };
        (
            str_in(stack),
            submode,
            settings,
            centre_hz,
            sized_in(input),
            Out::new(session),
        )
    };
    run(engine, |e| {
        let (stack, submode, settings, out) = (stack?, submode?, settings?, out?);
        let input = input_of(e, &input?)?;
        // SAFETY: the caller's contract on the views of `settings`.
        let values = unsafe { type_values(e, &format!("stack.{stack}"), settings)? };
        if !centre_hz.is_finite() || centre_hz < 0.0 {
            return Err(Error::BadArgument);
        }
        let centre = (centre_hz > 0.0).then_some(centre_hz);
        let id = e
            .host
            .engine
            .start_stack(stack, submode, &values, input, centre)?;
        out.put(object_handle(e, OwnerRef::Session(id)));
        Ok(())
    })
}

/// What a stack needs on this device by its recorded cost, before a session starts it (`sdrkit_stack_need`).
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct StackNeed {
    /// The number of bytes of the struct. The caller sets it to the sizeof of its struct.
    pub size: u32,
    /// The fast cores of this device that the stack needs. 0 for a stack with no recorded cost.
    pub cores: f64,
    /// The cores that the running sessions leave of the setting `cores_max`.
    pub cores_free: f64,
    /// The threads that the engine would give the stack now.
    pub threads: u32,
    /// The stack keeps up with its signal here: its largest stage fits one core, and the stack fits the free cores.
    pub real_time: bool,
    /// The recorded cost of the stack in calibration units per second of signal.
    pub units: f64,
    /// The calibration units per second of one fast core of this device.
    pub core_units: f64,
    /// The same of one slow core. Equal to `core_units` where the platform has one class of cores.
    pub core_units_slow: f64,
    /// The stages of the stack.
    pub stages: u32,
}

crate::handle::sized_structs!(StackNeed);

/// Writes to `need` what the stack `stack` in `submode` (NULL for the default) with its default settings needs on this
/// device, by its recorded cost, before a session starts it: the cores, the cores free, and the threads that the engine
/// would use. The call never waits. Returns `SDRKIT_E_NOT_NOW` while the calibration of the device has not finished (the
/// first tens of milliseconds after the engine started), `SDRKIT_E_NO_SUCH_KEY` for an unknown stack, and
/// `SDRKIT_E_CONFIG` if the stack cannot be made in `submode`.
#[no_mangle]
pub unsafe extern "C" fn sdrkit_stack_need(
    engine: Handle,
    stack: *const c_char,
    submode: *const c_char,
    need: *mut StackNeed,
) -> i32 {
    // SAFETY: the caller's contract.
    let (stack, submode, out) = unsafe {
        (
            str_in(stack),
            (!submode.is_null()).then(|| str_in(submode)).transpose(),
            Out::new(need),
        )
    };
    run(engine, |e| {
        let (stack, submode, out) = (stack?, submode?, out?);
        let n = e
            .host
            .engine
            .stack_need(stack, submode)?
            .ok_or(Error::NotYet)?;
        out.put(StackNeed {
            cores: n.cores,
            cores_free: n.cores_free,
            threads: n.threads,
            real_time: n.real_time,
            units: n.units,
            core_units: n.device.fast_units,
            core_units_slow: n.device.slow_units,
            stages: u32::try_from(n.stages.len()).unwrap_or(u32::MAX),
            ..StackNeed::default()
        });
        Ok(())
    })
}

/// Starts a session of the graph `graph` on `input`, and writes its handle to `session`. The graph can start more sessions.
/// Returns `SDRKIT_E_GRAPH`, `SDRKIT_E_CONNECT` or `SDRKIT_E_CONFIG` if the graph does not build.
#[no_mangle]
pub unsafe extern "C" fn sdrkit_graph_start(
    graph: Handle,
    input: *const Input,
    session: *mut Handle,
) -> i32 {
    // SAFETY: the caller's contract.
    let (input, out) = unsafe { (sized_in(input), Out::new(session)) };
    run(graph, |e| {
        let out = out?;
        let input = input_of(e, &input?)?;
        let plan = crate::graph::plan_of(e)?;
        let id = e.host.engine.start_graph(plan, input)?;
        out.put(object_handle(e, OwnerRef::Session(id)));
        Ok(())
    })
}

/// Stops the session of `session`. Its results stay until `sdrkit_session_close`.
#[no_mangle]
pub extern "C" fn sdrkit_session_stop(session: Handle) -> i32 {
    run(session, |e| {
        Ok(e.host.engine.stop_session(session_of(e)?)?)
    })
}

/// Closes the stopped session of `session`. The handle stays until `sdrkit_free`. Returns `SDRKIT_E_STILL_RUNNING` if the
/// session was not stopped.
#[no_mangle]
pub extern "C" fn sdrkit_session_close(session: Handle) -> i32 {
    run(session, |e| {
        Ok(e.host.engine.close_session(session_of(e)?)?)
    })
}

crate::handle::sized_structs!(Input, StackOptions, RadioOptions);
