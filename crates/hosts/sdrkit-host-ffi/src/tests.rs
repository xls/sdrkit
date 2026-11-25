//! Each family of the C API, called through its C functions, on an engine that keeps its files in memory.

use crate::engine::make_engine;
use crate::*;
use sdrkit_core::Facilities;
use std::ffi::CString;
use std::time::{Duration, Instant};

const OK: i32 = SDRKIT_OK;
const T: f64 = 20.0;

fn engine() -> Handle {
    make_engine(Facilities::default()).expect("an engine")
}

fn c(s: &str) -> CString {
    CString::new(s).expect("no NUL")
}

fn string(t: Str) -> String {
    // SAFETY: the library returns `len` bytes of UTF-8 at `ptr`, valid until the next call on the handle.
    let b = unsafe { std::slice::from_raw_parts(t.ptr.cast::<u8>(), t.len) };
    String::from_utf8(b.to_vec()).expect("UTF-8")
}

fn zero<T>() -> T {
    // SAFETY: every out struct of the API is plain data for which all zero bytes are a valid value.
    unsafe { std::mem::zeroed() }
}

fn find(h: Handle, key: &str) -> Handle {
    let mut o = 0;
    // SAFETY: a NUL-terminated key and a valid out pointer.
    assert_eq!(
        unsafe { sdrkit_object_find(h, c(key).as_ptr(), &mut o) },
        OK,
        "{key}"
    );
    o
}

fn wait_for<R>(seconds: f64, mut f: impl FnMut() -> Option<R>) -> Option<R> {
    let t = Instant::now();
    while t.elapsed().as_secs_f64() < seconds {
        if let Some(r) = f() {
            return Some(r);
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    None
}

/// Opens the test-signal radio of the FM stack and starts an FM session on it.
fn fm_session(e: Handle) -> (Handle, Handle) {
    let mut radio = 0;
    let mut session = 0;
    // SAFETY: NUL-terminated strings, no options, valid out pointers.
    unsafe {
        assert_eq!(
            sdrkit_radio_open(e, c("synth:fm").as_ptr(), std::ptr::null(), &mut radio),
            OK
        );
        let input = sdrkit_input_radio(radio);
        assert_eq!(
            sdrkit_stack_start(e, c("fm").as_ptr(), std::ptr::null(), &input, &mut session),
            OK
        );
    }
    (radio, session)
}

#[test]
fn the_engine_returns_its_version_and_its_objects() {
    let e = engine();
    let mut v = zero::<Str>();
    // SAFETY: valid out pointers in this test.
    unsafe {
        assert_eq!(sdrkit_version(&mut v), OK);
        assert_eq!(string(v), env!("CARGO_PKG_VERSION"));
        let memory = find(e, "service.memory");
        let mut t = -1;
        assert_eq!(sdrkit_object_type(memory, &mut t), OK);
        assert_eq!(t, SDRKIT_OBJECT_SERVICE);
        let mut key = zero::<Str>();
        assert_eq!(sdrkit_object_key(memory, &mut key), OK);
        assert_eq!(string(key), "service.memory");
        let mut engine_key = zero::<Str>();
        assert_eq!(sdrkit_object_key(e, &mut engine_key), OK);
        assert_eq!(string(engine_key), "engine");
        let mut h = 0;
        assert_eq!(
            sdrkit_object_find(e, c("service.nothing").as_ptr(), &mut h),
            SDRKIT_E_NO_SUCH_KEY
        );
        let (_radio, session) = fm_session(e);
        let mut session_key = zero::<Str>();
        assert_eq!(sdrkit_object_key(session, &mut session_key), OK);
        let session_key = string(session_key);
        let mut n = 0;
        assert_eq!(sdrkit_objects(e, &mut n), OK);
        let mut seen = Vec::new();
        for i in 0..n {
            let (mut k, mut t) = (zero::<Str>(), -1);
            assert_eq!(sdrkit_object(e, i, &mut k, &mut t), OK);
            seen.push((string(k), t));
        }
        assert_eq!(seen[0], ("engine".to_owned(), SDRKIT_OBJECT_ENGINE));
        assert!(seen.contains(&(session_key, SDRKIT_OBJECT_SESSION)));
        assert!(seen.contains(&("stack.fm".to_owned(), SDRKIT_OBJECT_STACK_TYPE)));
        assert!(seen.iter().any(|(_, t)| *t == SDRKIT_OBJECT_RADIO));
        let (mut k, mut t) = (zero::<Str>(), -1);
        assert_eq!(sdrkit_object(e, n, &mut k, &mut t), SDRKIT_E_NO_SUCH_ID);
    }
    assert_eq!(sdrkit_free(e), OK);
}

#[test]
fn a_stack_type_lists_its_settings_with_labels_options_and_help() {
    let e = engine();
    let fm = find(e, "stack.fm");
    let mut n = 0;
    // SAFETY: valid out pointers in this test.
    unsafe {
        assert_eq!(sdrkit_settings_count(fm, &mut n), OK);
        assert!(n > 0);
        let mut enums = 0;
        for id in 0..n {
            let mut d = Decl::default();
            assert_eq!(sdrkit_setting_decl(fm, id, &mut d), OK);
            assert!(!string(d.key).is_empty() && !string(d.label).is_empty());
            let mut v = zero::<Variant>();
            assert_eq!(sdrkit_setting_get(fm, id, &mut v), OK);
            assert_eq!(v.r#type, d.default_value.r#type);
            if d.r#type == SDRKIT_TYPE_ENUM {
                assert!(d.options > 0);
                let mut o = EnumOption::default();
                assert_eq!(sdrkit_setting_option(fm, id, 0, &mut o), OK);
                assert!(!string(o.label).is_empty());
                assert_eq!(
                    sdrkit_setting_option(fm, id, d.options, &mut o),
                    SDRKIT_E_NO_SUCH_ID
                );
                enums += 1;
            }
            let mut j = 0;
            let key = c(&string(d.key));
            assert_eq!(sdrkit_setting_find(fm, key.as_ptr(), &mut j), OK);
            assert_eq!(j, id);
        }
        assert!(enums > 0, "the FM stack has an enum setting");
        let mut d = Decl::default();
        assert_eq!(sdrkit_setting_decl(fm, n, &mut d), SDRKIT_E_NO_SUCH_ID);
    }
}

#[test]
fn a_refused_setting_returns_its_code_and_detail_and_changes_nothing() {
    let e = engine();
    // SAFETY: valid pointers in this test.
    unsafe {
        let mut id = 0;
        assert_eq!(sdrkit_setting_find(e, c("height_m").as_ptr(), &mut id), OK);
        let mut d = Decl::default();
        assert_eq!(sdrkit_setting_decl(e, id, &mut d), OK);
        assert!(d.flags & SDRKIT_DECL_RANGE != 0);
        let mut v = d.default_value;
        v.number = d.max + 1.0;
        assert_eq!(sdrkit_setting_set(e, id, &v), SDRKIT_E_OUT_OF_RANGE);
        let mut det = ErrorDetail::default();
        assert_eq!(sdrkit_error_detail(e, &mut det), OK);
        assert_eq!((det.code, det.setting), (SDRKIT_E_OUT_OF_RANGE, id));
        assert_eq!((det.max, det.value), (d.max, d.max + 1.0));
        assert_eq!(det.command, SDRKIT_NO_ID);
        let mut code = 0;
        assert_eq!(sdrkit_error_code(e, &mut code), OK);
        assert_eq!(code, SDRKIT_E_OUT_OF_RANGE);
        let mut t = zero::<Str>();
        assert_eq!(sdrkit_error_message(e, &mut t), OK);
        // The message has the label and the key of the setting, never its number.
        let text = string(t);
        assert!(
            text.contains("(height_m)") && !text.contains("Id("),
            "{text}"
        );
        let mut got = zero::<Variant>();
        assert_eq!(sdrkit_setting_get(e, id, &mut got), OK);
        assert_eq!(got.number, d.default_value.number);
        v.number = d.min;
        assert_eq!(sdrkit_setting_set(e, id, &v), OK);
        let mut view = zero::<Bytes>();
        assert_eq!(sdrkit_settings_export(e, &mut view), OK);
        let saved = std::slice::from_raw_parts(view.ptr, view.len).to_vec();
        v.number = d.max;
        assert_eq!(sdrkit_setting_set(e, id, &v), OK);
        assert_eq!(sdrkit_settings_import(e, saved.as_ptr(), saved.len()), OK);
        assert_eq!(sdrkit_setting_get(e, id, &mut got), OK);
        assert_eq!(got.number, d.min, "the saved value is back");
        let json = b"{}";
        assert_eq!(sdrkit_settings_import(e, json.as_ptr(), json.len()), SDRKIT_E_IMPORT);
        assert_eq!(sdrkit_settings_import(e, std::ptr::null(), 0), SDRKIT_E_IMPORT);
        assert_eq!(sdrkit_settings_import(e, std::ptr::null(), 4), SDRKIT_E_NULL_POINTER);
        let mut later = saved.clone();
        later[4..6].copy_from_slice(&2u16.to_le_bytes());
        assert_eq!(sdrkit_settings_import(e, later.as_ptr(), later.len()), SDRKIT_E_VERSION);
        let mut seed = 0x2545_F491_4F6C_DD1Du64;
        for _ in 0..300 {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            let mut b = saved.clone();
            let at = (seed % b.len() as u64) as usize;
            b[at] ^= (seed >> 32) as u8 | 1;
            let cut = if seed & 1 == 0 { b.len() } else { at };
            let code = sdrkit_settings_import(e, b.as_ptr(), cut);
            assert!(code != OK && code != SDRKIT_E_PANIC, "{code} at {at}");
        }
        assert_eq!(sdrkit_setting_get(e, id, &mut got), OK);
        assert_eq!(got.number, d.min, "a refused blob changes nothing");
    }
}

#[test]
fn a_table_is_read_by_counter_and_only_the_changed_rows_come_again() {
    let e = engine();
    // SAFETY: valid pointers in this test.
    unsafe {
        let mut info = ResultsInfo::default();
        assert_eq!(sdrkit_results_decl(e, &mut info), OK);
        assert!(info.tables > 0);
        let mut stacks = 0;
        assert_eq!(sdrkit_table_find(e, c("stacks").as_ptr(), &mut stacks), OK);
        let mut t = TableInfo::default();
        assert_eq!(sdrkit_table_decl(e, stacks, &mut t), OK);
        assert_eq!(string(t.key), "stacks");
        let mut name = 0;
        assert_eq!(
            sdrkit_column_find(e, stacks, c("name").as_ptr(), &mut name),
            OK
        );
// The table names its rows by the column `name`, and states no order and no place.
        assert_eq!(t.label_column, name);
        assert_eq!(t.sort_column, crate::value::SDRKIT_NO_ID);
        assert_eq!(t.position_column, crate::value::SDRKIT_NO_ID);
        assert_eq!(t.speed_column, crate::value::SDRKIT_NO_ID);
        assert_eq!(t.role, crate::results::SDRKIT_TABLE_ROLE_NONE);
        assert_eq!(t.sender_column, crate::value::SDRKIT_NO_ID);
        assert_eq!(t.time_column, crate::value::SDRKIT_NO_ID);
        assert_eq!(t.own_column, crate::value::SDRKIT_NO_ID);
        assert_eq!(t.delivery_column, crate::value::SDRKIT_NO_ID);
        assert_eq!(t.failure_column, crate::value::SDRKIT_NO_ID);
                let mut col = Decl::default();
        assert_eq!(sdrkit_column_decl(e, stacks, name, &mut col), OK);
        assert_eq!(col.r#type, SDRKIT_TYPE_STRING);
        assert_eq!(col.reading, 0);
        assert_eq!(col.refers_column.len, 0);
        let mut r = SnapshotInfo::default();
        assert_eq!(sdrkit_snapshot_read(e, 0, &mut r), OK);
        let mut rows = RowsInfo::default();
        assert_eq!(sdrkit_snapshot_table(e, stacks, &mut rows), OK);
        assert!(rows.rows > 0 && rows.rows == rows.total, "{rows:?}");
        let mut key = zero::<Str>();
        assert_eq!(sdrkit_snapshot_row(e, stacks, 0, &mut key), OK);
        assert!(!string(key).is_empty());
        let mut v = zero::<Variant>();
        assert_eq!(sdrkit_snapshot_cell(e, stacks, 0, name, &mut v), OK);
        // Nothing changed after the counter: no row comes again.
        let since = r.counter;
        assert_eq!(sdrkit_snapshot_read(e, since, &mut r), OK);
        assert_eq!(sdrkit_snapshot_table(e, stacks, &mut rows), OK);
        assert_eq!((rows.rows, rows.counter <= since), (0, true));
        // A new radio is one changed row of the table of radios.
        let mut radios = 0;
        assert_eq!(sdrkit_table_find(e, c("radios").as_ptr(), &mut radios), OK);
        let (_radio, _session) = fm_session(e);
        let mut r2 = SnapshotInfo::default();
        assert_eq!(sdrkit_snapshot_read(e, since, &mut r2), OK);
        assert!(r2.counter > since);
        assert_eq!(sdrkit_snapshot_table(e, radios, &mut rows), OK);
        assert_eq!((rows.rows, rows.total), (1, 1));
        assert_eq!(sdrkit_snapshot_table(e, stacks, &mut rows), OK);
        assert_eq!(rows.rows, 0);
    }
}

#[test]
fn audio_of_a_stack_session_reaches_the_caller() {
    let e = engine();
    let (radio, session) = fm_session(e);
    let mut buf = vec![0.0f32; 4800];
    let got = wait_for(T, || {
        let mut a = AudioInfo::default();
        // SAFETY: `buf` holds `buf.len()` floats, `a` is valid.
        let code = unsafe { sdrkit_audio_read(session, buf.as_mut_ptr(), buf.len(), &mut a) };
        assert_eq!(code, OK);
        (a.frames > 0).then_some(a)
    })
    .expect("audio");
    assert!(got.channels >= 1 && got.rate_hz >= 8e3, "{got:?}");
    let n = got.frames * usize::from(got.channels);
    assert!(buf[..n].iter().all(|x| x.is_finite()));
    // The stack of the session is an object with the declarations of the stack.
    let mut stack = 0;
    // SAFETY: a valid out pointer.
    assert_eq!(unsafe { sdrkit_object_stack(session, &mut stack) }, OK);
    let mut info = ResultsInfo::default();
    // SAFETY: a valid out pointer.
    assert_eq!(unsafe { sdrkit_results_decl(stack, &mut info) }, OK);
    assert_eq!(sdrkit_radio_close(radio), SDRKIT_E_RADIO_IN_USE);
    assert_eq!(sdrkit_session_close(session), SDRKIT_E_STILL_RUNNING);
    assert_eq!(sdrkit_session_stop(session), OK);
    assert_eq!(sdrkit_session_close(session), OK);
    assert_eq!(sdrkit_radio_close(radio), OK);
}

#[test]
fn an_event_has_the_id_of_its_type() {
    let e = engine();
    let (radio, _session) = fm_session(e);
    let found = wait_for(T, || {
        let mut n = 0;
        // SAFETY: valid out pointers in this test.
        unsafe {
            assert_eq!(sdrkit_events_take(radio, &mut n), OK);
            (0..n).find_map(|i| {
                let mut ev = EventInfo::default();
                assert_eq!(sdrkit_event(radio, i, &mut ev), OK);
                let mut t = EventTypeInfo::default();
                assert_eq!(sdrkit_event_type_decl(radio, ev.r#type, &mut t), OK);
                (string(t.key) == "radio.connected").then_some((ev, t))
            })
        }
    });
    let (ev, t) = found.expect("the radio connected");
    assert_eq!(ev.fields, t.fields);
    let mut id = 0;
    // SAFETY: valid pointers.
    unsafe {
        assert_eq!(
            sdrkit_event_type_find(e, c("radio.connected").as_ptr(), &mut id),
            OK
        );
        let mut count = 0;
        assert_eq!(sdrkit_event_types_count(e, &mut count), OK);
        assert!(id < count);
        if t.fields > 0 {
            let mut d = Decl::default();
            assert_eq!(sdrkit_event_field_decl(e, id, 0, &mut d), OK);
            let mut j = SDRKIT_NO_ID;
            let key = c(&string(d.key));
            assert_eq!(sdrkit_event_field_find(e, id, key.as_ptr(), &mut j), OK);
            assert_eq!(j, 0);
        }
    }
    assert_eq!(ev.r#type, id);
}

#[test]
fn a_command_runs_with_arguments_and_returns_its_reply() {
    let e = engine();
    // SAFETY: valid pointers in this test.
    unsafe {
        let mut id = 0;
        assert_eq!(
            sdrkit_command_find(e, c("state.save").as_ptr(), &mut id),
            OK
        );
        let mut d = CommandInfo::default();
        assert_eq!(sdrkit_command_decl(e, id, &mut d), OK);
        assert_eq!((string(d.key).as_str(), d.reply_fields), ("state.save", 1));
        let mut rf = Decl::default();
        assert_eq!(sdrkit_reply_field_decl(e, id, 0, &mut rf), OK);
        assert_eq!(rf.r#type, SDRKIT_TYPE_BYTES);
        let mut fields = 0;
        assert_eq!(
            sdrkit_command_invoke(e, id, std::ptr::null(), 0, &mut fields),
            OK
        );
        assert_eq!(fields, 1);
        let mut v = zero::<Variant>();
        assert_eq!(sdrkit_reply(e, 0, &mut v), OK);
        assert_eq!(v.r#type, SDRKIT_TYPE_BYTES);
        assert!(v.bytes.len > 0);
        // A required argument that is missing is refused before the command runs.
        let mut restore = 0;
        assert_eq!(
            sdrkit_command_find(e, c("state.restore").as_ptr(), &mut restore),
            OK
        );
        let mut state = SDRKIT_NO_ID;
        assert_eq!(
            sdrkit_argument_find(e, restore, c("state").as_ptr(), &mut state),
            OK
        );
        let mut a = Decl::default();
        assert_eq!(sdrkit_argument_decl(e, restore, state, &mut a), OK);
        assert!(a.flags & SDRKIT_DECL_REQUIRED != 0);
        assert_eq!(
            sdrkit_command_invoke(e, restore, std::ptr::null(), 0, &mut fields),
            SDRKIT_E_MISSING_ARGUMENT
        );
        let mut t = zero::<Str>();
        assert_eq!(sdrkit_error_message(e, &mut t), OK);
        // The message has the label and the key of the command and of the argument.
        let text = string(t);
        assert!(text.contains("(state)") && !text.contains("Id("), "{text}");
        let mut det = ErrorDetail::default();
        assert_eq!(sdrkit_error_detail(e, &mut det), OK);
        assert_eq!(det.setting, state);
        // The saved state is restored.
        let arg = Arg {
            id: state,
            value: v,
        };
        assert_eq!(sdrkit_command_invoke(e, restore, &arg, 1, &mut fields), OK);
        let mut n = 0;
        assert_eq!(sdrkit_commands_count(e, &mut n), OK);
        assert!(id < n);
    }
}

#[test]
fn a_graph_is_built_from_types_and_started() {
    let e = engine();
    let mut g = 0;
    // SAFETY: valid pointers in this test.
    unsafe {
        assert_eq!(sdrkit_graph_new(e, &mut g), OK);
        assert_eq!(
            sdrkit_graph_add(g, c("gen").as_ptr(), c("gen.stack").as_ptr()),
            OK
        );
        assert_eq!(
            sdrkit_graph_add(g, c("tx").as_ptr(), c("tx.sink").as_ptr()),
            OK
        );
        let mut ports = 0;
        assert_eq!(sdrkit_graph_ports(g, c("gen").as_ptr(), &mut ports), OK);
        assert_eq!(ports, 1);
        let mut p = PortInfo::default();
        assert_eq!(sdrkit_graph_port(g, c("tx").as_ptr(), 0, &mut p), OK);
        assert_eq!((p.r#type, p.output), (SDRKIT_STREAM_IQ, false));
        // The settings of a component are those of its type.
        let gen_type = find(e, "component.gen.stack");
        let mut stack = 0;
        assert_eq!(
            sdrkit_setting_find(gen_type, c("stack").as_ptr(), &mut stack),
            OK
        );
        let fm = "fm";
        let v = Variant {
            r#type: SDRKIT_TYPE_STRING,
            string: Str {
                ptr: fm.as_ptr().cast(),
                len: fm.len(),
            },
            ..zero()
        };
        assert_eq!(sdrkit_graph_set(g, c("gen").as_ptr(), stack, &v), OK);
        assert_eq!(
            sdrkit_graph_connect(g, c("gen.out").as_ptr(), c("tx.in").as_ptr()),
            OK
        );
        // A used input is refused, and the error detail names the ports.
        assert_eq!(
            sdrkit_graph_connect(g, c("gen").as_ptr(), c("tx").as_ptr()),
            SDRKIT_E_CONNECT
        );
        let mut d = ErrorDetail::default();
        assert_eq!(sdrkit_error_detail(e, &mut d), OK);
        assert_eq!(
            (string(d.component), string(d.from), string(d.to)),
            ("tx".into(), "gen.out".into(), "tx.in".into())
        );
        let mut message = zero::<Str>();
        assert_eq!(sdrkit_error_message(e, &mut message), OK);
        assert!(!string(message).is_empty());
        assert_eq!(
            sdrkit_graph_connect(g, c("gen.none").as_ptr(), c("tx").as_ptr()),
            SDRKIT_E_NO_SUCH_KEY
        );
        let input = sdrkit_input_inside();
        let mut s = 0;
        assert_eq!(sdrkit_graph_start(g, &input, &mut s), OK);
        let mut buf = vec![0.0f32; 2 * 4096];
        let got = wait_for(T, || {
            let mut r = IqInfo::default();
            assert_eq!(sdrkit_transmit_read(s, buf.as_mut_ptr(), 4096, &mut r), OK);
            (r.samples > 0).then_some(r)
        })
        .expect("transmit samples");
        assert!(got.rate_hz > 0.0, "{got:?}");
        let mut comp = 0;
        assert_eq!(sdrkit_object_component(s, c("gen").as_ptr(), &mut comp), OK);
        let mut none = 0;
        assert_eq!(
            sdrkit_object_component(s, c("nope").as_ptr(), &mut none),
            SDRKIT_E_NO_SUCH_KEY
        );
        let mut got_stack = zero::<Variant>();
        assert_eq!(sdrkit_setting_get(comp, stack, &mut got_stack), OK);
        assert_eq!(string(got_stack.string), "fm");
    }
}

#[test]
fn a_graph_is_validated_and_the_samples_of_the_caller_are_tapped() {
    let e = engine();
    let mut g = 0;
    // SAFETY: valid pointers in this test.
    unsafe {
        assert_eq!(sdrkit_graph_new(e, &mut g), OK);
        assert_eq!(
            sdrkit_graph_add(g, c("tx").as_ptr(), c("tx.sink").as_ptr()),
            OK
        );
        assert_eq!(
            sdrkit_graph_connect(g, c("input").as_ptr(), c("tx").as_ptr()),
            OK
        );
        let mut report = ReportInfo::default();
        assert_eq!(sdrkit_graph_validate(g, 48e3, 1e6, &mut report), OK);
        assert!(report.ports >= 1, "{report:?}");
        let mut rp = ReportPort::default();
        assert_eq!(sdrkit_graph_report_port(g, 0, &mut rp), OK);
        assert_eq!((string(rp.name).as_str(), rp.rate_hz), ("input", 48e3));
        let mut feed = 0;
        assert_eq!(sdrkit_feed_open(e, 48e3, 1e6, &mut feed), OK);
        let mut t = 0;
        assert_eq!(sdrkit_object_type(feed, &mut t), OK);
        assert_eq!(t, SDRKIT_OBJECT_FEED);
        let input = sdrkit_input_host(feed);
        let mut s = 0;
        assert_eq!(sdrkit_graph_start(g, &input, &mut s), OK);
        let mut tap = 0;
        assert_eq!(sdrkit_tap_open(s, c("input").as_ptr(), 8, &mut tap), OK);
        let iq: Vec<i16> = (0..2000).map(|i| (i % 100) as i16).collect();
        let mut buf = vec![0.0f32; 4096];
        let got = wait_for(T, || {
            let code =
                sdrkit_samples_push(feed, SDRKIT_SAMPLE_CS16, iq.as_ptr().cast(), iq.len() * 2);
            assert!(code == OK || code == SDRKIT_E_BEHIND, "{code}");
            let mut r = TapInfo::default();
            assert_eq!(
                sdrkit_tap_read(s, tap, buf.as_mut_ptr(), buf.len(), &mut r),
                OK
            );
            (r.values > 0).then_some(r)
        })
        .expect("tapped samples");
        assert_eq!(got.r#type, SDRKIT_STREAM_IQ);
        assert_eq!(got.centre_hz, 1e6);
        assert_eq!(
            sdrkit_samples_push(feed, SDRKIT_SAMPLE_CS16, iq.as_ptr().cast(), 3),
            SDRKIT_E_PART_SAMPLE
        );
        assert_eq!(
            sdrkit_samples_push(s, SDRKIT_SAMPLE_CS16, iq.as_ptr().cast(), 4),
            SDRKIT_E_WRONG_HANDLE
        );
        assert_eq!(sdrkit_tap_close(s, tap), OK);
        assert_eq!(sdrkit_tap_close(s, tap), SDRKIT_E_NO_SUCH_ID);
        let mut spec = SpectrumFrame::default();
        assert_eq!(
            sdrkit_spectrum_read(s, std::ptr::null_mut(), 0, &mut spec),
            OK
        );
        assert!(!spec.found && spec.bins == 0);
        assert_eq!(
            sdrkit_spectrum_read(s, std::ptr::null_mut(), 4, &mut spec),
            SDRKIT_E_NULL_POINTER
        );
        let mut m = MediaInfo::default();
        assert_eq!(sdrkit_media_read(s, 0, &mut m), OK);
        assert!(!m.found);
    }
}

#[test]
fn a_dead_handle_returns_no_such_object_and_never_crashes() {
    let e = engine();
    let memory = find(e, "service.memory");
    assert_eq!(sdrkit_free(memory), OK);
    let mut n = 0;
    // SAFETY: a valid out pointer.
    let code = unsafe { sdrkit_commands_count(memory, &mut n) };
    assert_eq!(code, SDRKIT_E_NO_SUCH_OBJECT);
    assert_eq!(sdrkit_free(memory), SDRKIT_E_NO_SUCH_OBJECT);
    // The handle of a closed session is refused.
    let (radio, session) = fm_session(e);
    assert_eq!(sdrkit_session_stop(session), OK);
    assert_eq!(sdrkit_session_close(session), OK);
    let mut a = AudioInfo::default();
    // SAFETY: no buffer, a valid out pointer.
    let code = unsafe { sdrkit_audio_read(session, std::ptr::null_mut(), 0, &mut a) };
    assert_eq!(code, SDRKIT_E_NO_SUCH_OBJECT);
    // Freeing the engine frees every handle of it.
    let scan = find(e, "service.scan");
    assert_eq!(sdrkit_free(e), OK);
    assert_eq!(sdrkit_radio_close(radio), SDRKIT_E_NO_SUCH_OBJECT);
    // SAFETY: a valid out pointer.
    let code = unsafe { sdrkit_commands_count(scan, &mut n) };
    assert_eq!(code, SDRKIT_E_NO_SUCH_OBJECT);
    assert_eq!(sdrkit_free(0), SDRKIT_E_NO_SUCH_OBJECT);
}

#[test]
fn null_pointers_and_bad_constants_are_refused() {
    let e = engine();
    // SAFETY: NULL pointers are what the test passes.
    unsafe {
        assert_eq!(
            sdrkit_settings_count(e, std::ptr::null_mut()),
            SDRKIT_E_NULL_POINTER
        );
        let mut h: Handle = 0;
        assert_eq!(
            sdrkit_object_find(e, std::ptr::null(), &mut h),
            SDRKIT_E_NULL_POINTER
        );
        let v = Variant {
            r#type: 42,
            ..zero()
        };
        assert_eq!(sdrkit_setting_set(e, 0, &v), SDRKIT_E_BAD_ARGUMENT);
        let bad = Input {
            r#type: 42,
            ..sdrkit_input_inside()
        };
        let mut s = 0;
        assert_eq!(
            sdrkit_stack_start(e, c("fm").as_ptr(), std::ptr::null(), &bad, &mut s),
            SDRKIT_E_BAD_ARGUMENT
        );
        let mut r = 0;
        assert_eq!(
            sdrkit_radio_open(e, c("none:here").as_ptr(), std::ptr::null(), &mut r),
            SDRKIT_E_NO_SUCH_KEY
        );
        // The scheme `file:` reaches the file driver, which looks for the recording in the storage of the engine.
        assert_eq!(
            sdrkit_radio_open(e, c("file:none.wav").as_ptr(), std::ptr::null(), &mut r),
            SDRKIT_E_NOT_FOUND
        );
    }
}

#[test]
fn a_stack_starts_with_its_options() {
    let e = engine();
    // SAFETY: valid pointers in this test.
    unsafe {
        let fm = find(e, "stack.fm");
        let mut n = 0;
        assert_eq!(sdrkit_settings_count(fm, &mut n), OK);
        let mut d = Decl::default();
        let id = (0..n)
            .find(|&id| {
                assert_eq!(sdrkit_setting_decl(fm, id, &mut d), OK);
                d.r#type == SDRKIT_TYPE_ENUM && d.options > 1 && d.flags & SDRKIT_DECL_FIXED == 0
            })
            .expect("an enum setting");
        let mut value = d.default_value;
        value.enum_index = (value.enum_index + 1) % d.options;
        let args = [Arg { id, value }];
        let opts = StackOptions {
            settings: args.as_ptr(),
            count: args.len(),
            ..StackOptions::default()
        };
        let mut radio = 0;
        let ropts = RadioOptions::default();
        assert_eq!(
            sdrkit_radio_open(e, c("synth:fm").as_ptr(), &ropts, &mut radio),
            OK
        );
        let input = sdrkit_input_radio(radio);
        let mut s = 0;
        assert_eq!(
            sdrkit_stack_start(e, c("fm").as_ptr(), &opts, &input, &mut s),
            OK
        );
        let mut stack = 0;
        assert_eq!(sdrkit_object_stack(s, &mut stack), OK);
        let mut got = zero::<Variant>();
        assert_eq!(sdrkit_setting_get(stack, id, &mut got), OK);
        assert_eq!(got.enum_index, value.enum_index);
    }
}

#[test]
fn a_panic_inside_a_call_returns_its_code_and_the_engine_stays_usable() {
    let e = engine();
    let code = crate::handle::run(e, |_| panic!("a fault inside"));
    assert_eq!(code, SDRKIT_E_PANIC);
    let mut d = ErrorDetail::default();
    // SAFETY: a valid out pointer.
    assert_eq!(unsafe { sdrkit_error_detail(e, &mut d) }, OK);
    assert_eq!(d.code, SDRKIT_E_PANIC);
    let mut n = 0;
    // SAFETY: a valid out pointer.
    assert_eq!(unsafe { sdrkit_commands_count(e, &mut n) }, OK);
}

#[test]
fn every_code_of_the_header_has_its_name() {
    let header = include_str!("../include/sdrkit.h");
    let mut n = 0;
    for line in header.lines() {
        let mut words = line.split_whitespace();
        let (Some("#define"), Some(name), Some(value)) = (words.next(), words.next(), words.next())
        else {
            continue;
        };
        if name != "SDRKIT_OK" && !name.starts_with("SDRKIT_E_") {
            continue;
        }
        let code: i32 = value.parse().expect("a number");
        // SAFETY: the library returns a static NUL-terminated string.
        let got = unsafe { std::ffi::CStr::from_ptr(sdrkit_code_name(code)) };
        assert_eq!(got.to_str(), Ok(name));
        n += 1;
    }
    assert!(n > 50, "{n} codes");
    // SAFETY: as above.
    let none = unsafe { std::ffi::CStr::from_ptr(sdrkit_code_name(-7)) };
    assert!(none.to_bytes().is_empty());
}

#[test]
fn an_engine_without_a_folder_keeps_its_files_in_memory() {
    let mut e = 0;
    // SAFETY: valid pointers in this test.
    unsafe {
        assert_eq!(sdrkit_engine_new(std::ptr::null(), &mut e), OK);
        let mut view = zero::<Bytes>();
        assert_eq!(sdrkit_settings_export(e, &mut view), OK);
        let saved = std::slice::from_raw_parts(view.ptr, view.len).to_vec();
        assert_eq!(sdrkit_settings_import(e, saved.as_ptr(), saved.len()), OK);
    }
    assert_eq!(sdrkit_free(e), OK);
}

#[test]
fn a_failed_call_without_a_handle_leaves_its_error_on_handle_0() {
    let mut e = 0;
    // SAFETY: valid pointers in this test.
    unsafe {
        let code = sdrkit_engine_new(c("/dev/null/sdrkit").as_ptr(), &mut e);
        assert_ne!(code, OK);
        let mut got = 0;
        assert_eq!(sdrkit_error_code(0, &mut got), OK);
        assert_eq!(got, code);
        let mut message = zero::<Str>();
        assert_eq!(sdrkit_error_message(0, &mut message), OK);
        assert!(!string(message).is_empty());
        let mut d = ErrorDetail::default();
        assert_eq!(sdrkit_error_detail(0, &mut d), OK);
        assert_eq!(d.code, code);
        assert!(string(d.component).is_empty());
    }
}

/// The address command of a radio type without its required part: the message says which part.
#[test]
fn a_missing_address_part_is_named_in_the_message() {
    let e = engine();
    let kind = find(e, "driver.iiod");
    // SAFETY: valid pointers in this test.
    unsafe {
        let mut id = 0;
        assert_eq!(
            sdrkit_command_find(kind, c("address").as_ptr(), &mut id),
            OK
        );
        let mut fields = 0;
        assert_eq!(
            sdrkit_command_invoke(kind, id, std::ptr::null(), 0, &mut fields),
            SDRKIT_E_MISSING_ARGUMENT
        );
        let mut t = zero::<Str>();
        assert_eq!(sdrkit_error_message(kind, &mut t), OK);
        let text = string(t);
        assert!(
            text.contains("(address)") && text.contains('"') && !text.contains("Id("),
            "{text}"
        );
        println!("{text}");
    }
}

#[test]
fn the_members_of_a_struct_are_looked_up_once_for_a_handle() {
    use crate::typed::RESOLVES;
    use std::sync::atomic::Ordering;
    let e = engine();
    let (_radio, session) = fm_session(e);
    // Storage for one struct of result fields and for rows, aligned for every member.
    let mut fields = vec![0u64; 512];
    let mut rows = vec![0u64; 512];
    let (set, row, table) = (c("fm.FmFields"), c("fm.AfRow"), c("af"));
    // SAFETY: valid pointers, and buffers of the sizes passed.
    let mut read = || unsafe {
        let mut r = SnapshotInfo::default();
        assert_eq!(sdrkit_snapshot_read(session, 0, &mut r), OK);
        assert_eq!(
            sdrkit_typed_fields(session, set.as_ptr(), fields.as_mut_ptr().cast(), 4096),
            OK
        );
        let mut info = RowsInfo::default();
        assert_eq!(
            sdrkit_typed_rows(
                session,
                row.as_ptr(),
                table.as_ptr(),
                rows.as_mut_ptr().cast(),
                64,
                64,
                &mut info
            ),
            OK
        );
    };
    read();
    let first = RESOLVES.load(Ordering::Relaxed);
    assert!(first >= 2);
    for _ in 0..20 {
        std::thread::sleep(std::time::Duration::from_millis(20));
        read();
    }
    assert_eq!(RESOLVES.load(Ordering::Relaxed), first);
}

/// The test signal of a stack that the engine does not have names the stack: `SDRKIT_E_NO_SUCH_KEY` with the id in the
/// member `key` of the detail. An unknown driver names its key the same way.
#[test]
fn a_test_signal_of_an_unknown_stack_names_the_stack() {
    let e = engine();
    let mut radio = 0;
    let mut d = ErrorDetail::default();
    let mut t = zero::<Str>();
    // SAFETY: NUL-terminated strings, no options, valid out pointers.
    unsafe {
        for (address, key) in [("synth:no_such_stack", "no_such_stack"), ("synth:nope/fast", "nope"), ("nodriver:x", "nodriver")] {
            assert_eq!(
                sdrkit_radio_open(e, c(address).as_ptr(), std::ptr::null(), &mut radio),
                SDRKIT_E_NO_SUCH_KEY,
                "{address}"
            );
            assert_eq!(sdrkit_error_detail(e, &mut d), OK);
            assert!(string(d.key).contains(key), "{address}: {}", string(d.key));
            assert_eq!(sdrkit_error_message(e, &mut t), OK);
            assert!(string(t).contains(key), "{address}: {}", string(t));
        }
        assert_eq!(
            sdrkit_radio_open(e, c("synth:fm").as_ptr(), std::ptr::null(), &mut radio),
            OK
        );
        assert_eq!(sdrkit_error_detail(e, &mut d), OK);
    }
}

/// The test signal of a stack takes its settings in the open call: the settings that an open of `synth:fm` takes list
/// those of the signal after those of the driver, the radio starts with the station name that the open gave, and the
/// FM session decodes that name.
#[test]
fn a_test_signal_takes_the_settings_of_its_signal_at_open() {
    let e = engine();
    let at = c("synth:fm");
    let name = "QA TEST ";
    // SAFETY: NUL-terminated strings, valid pointers, and `name` outlives the open call.
    unsafe {
        let kind = find(e, "driver.synth");
        let (mut own, mut all) = (0, 0);
        assert_eq!(sdrkit_settings_count(kind, &mut own), OK);
        assert_eq!(
            sdrkit_radio_open_settings_count(e, at.as_ptr(), &mut all),
            OK
        );
        assert!(all > own, "{all} {own}");
        // The settings of the driver keep their ids.
        let mut d = Decl::default();
        for id in 0..own {
            assert_eq!(
                sdrkit_radio_open_setting_decl(e, at.as_ptr(), id, &mut d),
                OK
            );
            let key = string(d.key);
            let mut same = 0;
            assert_eq!(sdrkit_setting_find(kind, c(&key).as_ptr(), &mut same), OK);
            assert_eq!(same, id, "{key}");
        }
        let mut ps = 0;
        assert_eq!(
            sdrkit_setting_find(kind, c("gen_ps").as_ptr(), &mut ps),
            SDRKIT_E_NO_SUCH_KEY
        );
        assert_eq!(
            sdrkit_radio_open_setting_find(e, at.as_ptr(), c("gen_ps").as_ptr(), &mut ps),
            OK
        );
        assert!(ps >= own);
        assert_eq!(
            sdrkit_radio_open_setting_decl(e, at.as_ptr(), ps, &mut d),
            OK
        );
        assert_eq!(d.r#type, SDRKIT_TYPE_STRING);
        assert_eq!(
            sdrkit_radio_open_setting_decl(e, at.as_ptr(), all, &mut d),
            SDRKIT_E_NO_SUCH_ID
        );
        assert_eq!(
            sdrkit_radio_open_settings_count(e, c("synth:no_such_stack").as_ptr(), &mut all),
            SDRKIT_E_NO_SUCH_KEY
        );
        let value = Variant {
            r#type: SDRKIT_TYPE_STRING,
            string: Str {
                ptr: name.as_ptr().cast(),
                len: name.len(),
            },
            ..zero::<Variant>()
        };
        let args = [Arg { id: ps, value }];
        let opts = RadioOptions {
            settings: args.as_ptr(),
            count: args.len(),
            ..RadioOptions::default()
        };
        let mut radio = 0;
        assert_eq!(sdrkit_radio_open(e, at.as_ptr(), &opts, &mut radio), OK);
        let mut id = 0;
        assert_eq!(
            sdrkit_setting_find(radio, c("gen_ps").as_ptr(), &mut id),
            OK
        );
        let mut got = zero::<Variant>();
        assert_eq!(sdrkit_setting_get(radio, id, &mut got), OK);
        assert_eq!(string(got.string), name);
        let input = sdrkit_input_radio(radio);
        let mut session = 0;
        assert_eq!(
            sdrkit_stack_start(e, c("fm").as_ptr(), std::ptr::null(), &input, &mut session),
            OK
        );
        let mut station = 0;
        assert_eq!(
            sdrkit_field_find(session, c("station").as_ptr(), &mut station),
            OK
        );
        let first = wait_for(T, || {
            let mut r = SnapshotInfo::default();
            assert_eq!(sdrkit_snapshot_read(session, 0, &mut r), OK);
            let mut v = zero::<Variant>();
            assert_eq!(sdrkit_snapshot_field(session, station, &mut v), OK);
            let s = if v.r#type == SDRKIT_TYPE_STRING {
                string(v.string)
            } else {
                String::new()
            };
            (!s.is_empty()).then_some(s)
        });
        assert_eq!(first.as_deref(), Some(name));
    }
}

/// Opens the test-signal radio of stack `stack`, starts a session of the stack on it and returns the handle of the
/// stack object of the session.
fn stack_of_session(e: Handle, stack: &str) -> Handle {
    let mut radio = 0;
    let mut session = 0;
    let mut object = 0;
    let address = c(&format!("synth:{stack}"));
    // SAFETY: NUL-terminated strings, no options, valid out pointers.
    unsafe {
        assert_eq!(
            sdrkit_radio_open(e, address.as_ptr(), std::ptr::null(), &mut radio),
            OK
        );
        let input = sdrkit_input_radio(radio);
        assert_eq!(
            sdrkit_stack_start(e, c(stack).as_ptr(), std::ptr::null(), &input, &mut session),
            OK
        );
        assert_eq!(sdrkit_object_stack(session, &mut object), OK);
    }
    object
}

/// The id of command `key` of `h`.
fn command(h: Handle, key: &str) -> Id {
    let mut id = 0;
    // SAFETY: a NUL-terminated key and a valid out pointer.
    let code = unsafe { sdrkit_command_find(h, c(key).as_ptr(), &mut id) };
    assert_eq!(code, OK, "{key}");
    id
}

/// The id of argument `key` of command `command` of `h`.
fn argument(h: Handle, command: Id, key: &str) -> Id {
    let mut id = 0;
    // SAFETY: a NUL-terminated key and a valid out pointer.
    let code = unsafe { sdrkit_argument_find(h, command, c(key).as_ptr(), &mut id) };
    assert_eq!(code, OK, "{key}");
    id
}

/// A view of `b` as a bytes value.
fn bytes_value(b: &[u8]) -> Variant {
    Variant {
        r#type: SDRKIT_TYPE_BYTES,
        bytes: Bytes {
            ptr: b.as_ptr(),
            len: b.len(),
        },
        ..zero::<Variant>()
    }
}

fn integer_value(integer: i64) -> Variant {
    Variant {
        r#type: SDRKIT_TYPE_I64,
        integer,
        ..zero::<Variant>()
    }
}

/// Runs command `id` of `h` with `args` and returns the code.
fn invoke(h: Handle, id: Id, args: &[Arg]) -> i32 {
    let mut fields = 0;
    // SAFETY: `args` holds views of live buffers of the caller, and the out pointer is valid.
    unsafe { sdrkit_command_invoke(h, id, args.as_ptr(), args.len(), &mut fields) }
}

/// True if `code` is an error that the command returned, not a caught panic.
fn refused(code: i32) -> bool {
    code != OK && code != SDRKIT_E_PANIC
}

#[test]
fn transport_packets_are_added_through_the_stack_object_of_a_session() {
    let e = engine();
    let stack = stack_of_session(e, "dvbt");
    let append = command(stack, "append");
    let id = argument(stack, append, "packets");
    let mut packets = vec![0u8; 500 * 188];
    for p in packets.as_chunks_mut::<188>().0 {
        p[..4].copy_from_slice(&[0x47, 0x01, 0x00, 0x10]);
    }
    let whole = [Arg {
        id,
        value: bytes_value(&packets),
    }];
    assert_eq!(invoke(stack, append, &whole), OK);
    // A part of a packet, a packet and a part, and no argument.
    for cut in [&packets[..187], &packets[..189]] {
        let part = [Arg {
            id,
            value: bytes_value(cut),
        }];
        assert!(refused(invoke(stack, append, &part)), "{} bytes", cut.len());
    }
    assert_eq!(invoke(stack, append, &[]), SDRKIT_E_MISSING_ARGUMENT);
}

#[test]
fn a_picture_is_set_through_the_stack_object_of_a_session() {
    let e = engine();
    let stack = stack_of_session(e, "atv");
    let frame = command(stack, "frame");
    let format = argument(stack, frame, "pixel_format");
    let mut d = Decl::default();
    // SAFETY: valid out pointers.
    let bgra = unsafe {
        assert_eq!(sdrkit_argument_decl(stack, frame, format, &mut d), OK);
        (0..d.options)
            .find(|&n| {
                let mut o = EnumOption::default();
                assert_eq!(sdrkit_argument_option(stack, frame, format, n, &mut o), OK);
                string(o.key) == "bgra"
            })
            .expect("the option bgra")
    };
    let pixels = [0x80u8; 8 * 8 * 4];
    let picture = |pixels: &[u8], width: i64| {
        [
            Arg {
                id: argument(stack, frame, "pixels"),
                value: bytes_value(pixels),
            },
            Arg {
                id: argument(stack, frame, "width"),
                value: integer_value(width),
            },
            Arg {
                id: argument(stack, frame, "height"),
                value: integer_value(8),
            },
            Arg {
                id: format,
                value: Variant {
                    r#type: SDRKIT_TYPE_ENUM,
                    enum_index: bgra,
                    ..zero::<Variant>()
                },
            },
        ]
    };
    assert_eq!(invoke(stack, frame, &picture(&pixels, 8)), OK);
    // Pixels for a part of the picture, and a width that the pixels do not fill.
    assert!(refused(invoke(stack, frame, &picture(&pixels[..255], 8))));
    assert!(refused(invoke(stack, frame, &picture(&pixels, 9))));
}
