//! Live changes, gain-reduction radios, close and the log connection.

use super::fake::{self, Fake, Script};
use super::*;
use crate::rpc::{self, call, ArgInfo, ArgType, Packer, Range, Unpacker};
use crate::{SoapyRemoteSetting, SoapySource, ITEM_VALUE, OVERFLOWS, SENSORS};
use sdrkit_core::{Schema, SettingId, SettingSet};
use sdrkit_driver_core::{own_id, ring, DriverError, RadioSetting, Retuned, RingReader};
use std::sync::atomic::Ordering;
use std::sync::Arc;

fn req(n: u32, f: impl FnOnce(&mut Packer)) -> Vec<u8> {
    let mut p = Packer::call(n);
    f(&mut p);
    rpc::packet(&p.buf, rpc::VERSION)
}

fn freq(hz: f64) -> Vec<u8> {
    req(call::SET_FREQUENCY, |p| {
        p.dir_ch(1, 0).f64(hz).kwargs(&Default::default());
    })
}

fn gain(db: f64) -> Vec<u8> {
    req(call::SET_GAIN, |p| {
        p.dir_ch(1, 0).f64(db);
    })
}

fn element(name: &str, db: f64) -> Vec<u8> {
    req(call::SET_GAIN_ELEMENT, |p| {
        p.dir_ch(1, 0).string(name).f64(db);
    })
}

fn write(key: &str, v: &str) -> Vec<u8> {
    req(call::WRITE_SETTING, |p| {
        p.string(key).string(v);
    })
}

fn start(f: &Fake, v: &Values) -> (SoapySource, RingReader) {
    let mut src = drv(f).open_source(&f.addr(), v).unwrap();
    let (w, r) = ring(1 << 22, src.sample_type());
    src.start(w).unwrap();
    (src, r)
}

/// The test values with `set`.
fn vals(set: &[(&str, Value)]) -> Values {
    values(&drv(&Fake::start(Script::rtl())), set)
}

/// Sets `key` of `v` to `value`.
fn put(v: &mut Values, key: &str, value: Value) {
    let id: SettingId = DESCRIPTOR.decl().schema().id(key).unwrap();
    v.set(id, &value).unwrap();
}

fn remote(name: &str) -> (&'static str, Value) {
    ("remote", Value::Text(name.into()))
}

fn bytes_since(f: &Fake, n: usize) -> Vec<Vec<u8>> {
    f.since(n).into_iter().map(|r| r.bytes).collect()
}

fn info_list(infos: &[ArgInfo]) -> Vec<u8> {
    let mut p = Packer::default();
    p.arg_info_list(infos, rpc::VERSION);
    p.buf
}

fn bool_info(key: &str, value: &str) -> ArgInfo {
    ArgInfo {
        key: key.into(),
        value: value.into(),
        name: String::new(),
        description: String::new(),
        units: String::new(),
        kind: Some(ArgType::Bool),
        range: Range::default(),
        options: Vec::new(),
        option_names: Vec::new(),
    }
}

#[test]
fn live_retunes_send_only_what_changed() {
    let f = Fake::start(Script::rtl());
    let mut t = vals(&[remote("rtlsdr")]);
    let (mut src, _r) = start(&f, &t);
    // Frequency only.
    let n = f.control().len();
    put(&mut t, "centre_hz", Value::Hz(101e6));
    assert!(matches!(src.apply(&t).unwrap(), Retuned::Live(_)));
    assert_eq!(bytes_since(&f, n), vec![freq(101e6)]);
    // Gain only.
    let n = f.control().len();
    put(&mut t, "gain", Value::F64(30.0));
    src.apply(&t).unwrap();
    assert_eq!(bytes_since(&f, n), vec![gain(30.0)]);
    // A rate change: deactivate, rate, read back, activate. The stream stays.
    let n = f.control().len();
    put(&mut t, "rate", Value::Hz(1.024e6));
    let Retuned::Live(s) = src.apply(&t).unwrap() else {
        panic!()
    };
    assert_eq!(s.rate_hz, 1.024e6);
    let got = bytes_since(&f, n);
    let rate = req(call::SET_SAMPLE_RATE, |p| {
        p.dir_ch(1, 0).f64(1.024e6);
    });
    assert_eq!(
        got,
        vec![hex(DEACTIVATE), rate, hex(GET_RATE), hex(ACTIVATE)]
    );
    assert_eq!(f.sh.streams.lock().unwrap().len(), 1, "no new set-up");
    // A refused rate: activate is still sent, and the rate is refused.
    f.script.lock().unwrap().handler = Some(Arc::new(|n, _| {
        (n == call::SET_SAMPLE_RATE).then(|| fake::exception("Invalid sample rate"))
    }));
    let n = f.control().len();
    put(&mut t, "rate", Value::Hz(2.0e6));
    let err = src.apply(&t).unwrap_err();
    assert_eq!(
        err,
        DriverError::Refused {
            id: RadioSetting::Rate.id()
        }
    );
    assert_eq!(
        *f.since(n)
            .iter()
            .map(|r| r.call)
            .collect::<Vec<_>>()
            .last()
            .unwrap(),
        call::ACTIVATE_STREAM
    );
    f.script.lock().unwrap().handler = None;
    // A frequency outside the ranges of the radio: sent, with a note.
    put(&mut t, "rate", Value::Hz(1.024e6));
    src.apply(&t).unwrap();
    notes(&mut src);
    let n = f.control().len();
    put(&mut t, "centre_hz", Value::Hz(7e6));
    src.apply(&t).unwrap();
    assert_eq!(bytes_since(&f, n), vec![freq(7e6)]);
    let lines = notes(&mut src);
    assert!(
        lines.iter().any(|l| l.contains(
            "this radio is specified for 24 to 1766 MHz: the driver can fail to reach 7 MHz"
        )),
        "{lines:?}"
    );
    let n = f.control().len();
    src.apply(&t).unwrap();
    assert!(
        bytes_since(&f, n).is_empty(),
        "nothing changed, nothing sent"
    );
    // Frequency correction by the radio.
    let n = f.control().len();
    put(&mut t, "ppm", Value::F64(3.0));
    src.apply(&t).unwrap();
    let corr = req(call::SET_FREQ_CORRECTION, |p| {
        p.dir_ch(1, 0).f64(3.0);
    });
    assert_eq!(bytes_since(&f, n), vec![corr]);
    src.stop();
}

#[test]
fn without_its_own_correction_the_frequency_is_corrected() {
    let mut s = Script::rtl();
    s.replies
        .insert(call::HAS_FREQ_CORRECTION, fake::boolean(false));
    let f = Fake::start(s);
    let mut t = vals(&[remote("rtlsdr")]);
    let (mut src, _r) = start(&f, &t);
    let n = f.control().len();
    put(&mut t, "ppm", Value::F64(3.0));
    src.apply(&t).unwrap();
    let got = bytes_since(&f, n);
    assert_eq!(got, vec![freq(100_000_000.0 / 1.000003)]);
    assert!((100_000_000.0f64 / 1.000003 - 99_999_700.000_9).abs() < 1e-3);
    assert!(!f.calls().contains(&call::SET_FREQ_CORRECTION));
    src.stop();
}

fn gr_script(overall: (f64, f64)) -> Script {
    let mut s = Script::rtl();
    s.replies
        .insert(call::LIST_GAINS, fake::strings(&["IFGR", "RFGR"]));
    s.replies
        .insert(call::GAIN_RANGE, fake::range(overall.0, overall.1, 0.0));
    s.replies
        .insert(call::GET_GAIN_ELEMENT, fake::f64_payload(30.0));
    s.handler = Some(Arc::new(|n, args| {
        if n != call::GAIN_RANGE_ELEMENT {
            return None;
        }
        let mut u = Unpacker::new(args, rpc::VERSION);
        let _ = (u.char(), u.i32());
        Some(match u.string().unwrap().as_str() {
            "IFGR" => fake::range(20.0, 59.0, 1.0),
            _ => fake::range(0.0, 9.0, 1.0),
        })
    }));
    s
}

#[test]
fn a_gain_reduction_radio() {
    let f = Fake::start(gr_script((0.0, 48.0)));
    let mut t = vals(&[remote("sdrplay"), ("gain", Value::F64(48.0))]);
    let (mut src, _r) = start(&f, &t);
    let c = f.control();
    let i = c
        .iter()
        .position(|r| r.bytes == element("RFGR", 0.0))
        .expect("RFGR 0 at open");
    assert_eq!(c[i + 1].bytes, element("IFGR", 20.0));
    for (g, rfgr, want) in [
        (0.0, None, [9.0, 59.0]),
        (24.0, None, [5.0, 40.0]),
        (48.0, Some(6), [6.0, 20.0]),
    ] {
        let n = f.control().len();
        put(&mut t, "gain", Value::F64(g));
        if let Some(v) = rfgr {
            put(&mut t, "rfgr", Value::I64(v));
        }
        src.apply(&t).unwrap();
        assert_eq!(
            bytes_since(&f, n),
            vec![element("RFGR", want[0]), element("IFGR", want[1])],
            "gain {g}"
        );
    }
    assert_eq!(src.capabilities().gain_db, Some((0.0, 48.0, 0.0)));
    // An RF state the radio does not have is refused.
    put(&mut t, "rfgr", Value::I64(10));
    assert_eq!(
        src.apply(&t).unwrap_err(),
        DriverError::Refused {
            id: own_id(SoapyRemoteSetting::RfState)
        }
    );
    src.stop();
    // An overall range of 0 to 1 dB returns the slider 0 to 48 dB.
    let f = Fake::start(gr_script((0.0, 1.0)));
    let (mut src, _r) = start(&f, &vals(&[remote("sdrplay")]));
    assert_eq!(src.capabilities().gain_db, Some((0.0, 48.0, 0.0)));
    src.stop();
}

/// The test values of the kind's settings and of the settings that `src` adds, with `set`.
fn with_run_time(src: &SoapySource, set: &[(&str, Value)]) -> Values {
    let kind = DESCRIPTOR.decl().schema().clone();
    let own: Vec<_> = set
        .iter()
        .filter(|(k, _)| kind.id(k).is_ok())
        .cloned()
        .collect();
    let base = vals(&own);
    let mut decls = DESCRIPTOR.decl().schema().settings().to_vec();
    decls.extend(src.settings());
    let schema = Arc::new(Schema::new(decls).unwrap());
    let mut v = Values::defaults(&schema);
    for i in 0..base.schema().len() {
        let id = SettingId(i as u16);
        v.set(id, base.value(id).unwrap()).unwrap();
    }
    for (key, value) in set {
        v.set(schema.id(key).unwrap(), value).unwrap();
    }
    v
}

#[test]
fn gain_stages_set_by_hand() {
    let mut s = Script::rtl();
    s.replies
        .insert(call::LIST_GAINS, fake::strings(&["LNA", "VGA"]));
    s.replies
        .insert(call::GAIN_RANGE, fake::range(0.0, 102.0, 0.0));
    s.replies
        .insert(call::GET_GAIN_ELEMENT, fake::f64_payload(16.0));
    s.handler = Some(Arc::new(|n, args| {
        if n != call::GAIN_RANGE_ELEMENT {
            return None;
        }
        let mut u = Unpacker::new(args, rpc::VERSION);
        let _ = (u.char(), u.i32());
        Some(match u.string().unwrap().as_str() {
            "LNA" => fake::range(0.0, 40.0, 8.0),
            _ => fake::range(0.0, 62.0, 2.0),
        })
    }));
    let f = Fake::start(s);
    let (mut src, _r) = start(&f, &vals(&[remote("lime")]));
    let added = src.settings();
    let labels: Vec<_> = added
        .iter()
        .filter(|d| d.field.key.starts_with("gain."))
        .map(|d| (d.field.key.as_ref(), d.field.label.as_ref()))
        .collect();
    assert_eq!(
        labels,
        [
            ("gain.LNA", "Low-noise amplifier (LNA)"),
            ("gain.VGA", "Variable-gain amplifier (VGA)")
        ]
    );
    // A stage at its value at open follows the gain; a stage set by hand is sent after the gain.
    let n = f.control().len();
    let t = with_run_time(
        &src,
        &[
            remote("lime"),
            ("gain", Value::F64(30.0)),
            ("gain.VGA", Value::F64(12.0)),
        ],
    );
    src.apply(&t).unwrap();
    assert_eq!(bytes_since(&f, n), vec![gain(30.0), element("VGA", 12.0)]);
    // A refused stage is a refusal of its own setting.
    f.script.lock().unwrap().handler = Some(Arc::new(|n, _| {
        (n == call::SET_GAIN_ELEMENT).then(|| fake::exception("no"))
    }));
    let t = with_run_time(
        &src,
        &[
            remote("lime"),
            ("gain", Value::F64(30.0)),
            ("gain.VGA", Value::F64(14.0)),
        ],
    );
    assert_eq!(
        src.apply(&t).unwrap_err(),
        DriverError::Refused {
            id: DESCRIPTOR.run_time_id(
                added
                    .iter()
                    .position(|d| d.field.key == "gain.VGA")
                    .unwrap()
            )
        }
    );
    src.stop();
}

#[test]
fn driver_settings_are_settings_of_the_open_radio() {
    let mut s = Script::rtl();
    let mut agc = bool_info("dc_cancel", "false");
    agc.name = "DC cancel".into();
    let mut mode = bool_info("mode", "a");
    mode.kind = Some(ArgType::String);
    mode.options = vec!["a".into(), "b".into()];
    s.replies.insert(
        call::SETTING_INFO,
        info_list(&[agc, mode, bool_info("biasT_ctrl", "false")]),
    );
    s.replies.insert(call::READ_SETTING, fake::string(""));
    let f = Fake::start(s);
    let (mut src, _r) = start(&f, &vals(&[remote("sdrplay")]));
    let keys: Vec<_> = src
        .settings()
        .iter()
        .map(|d| d.field.key.to_string())
        .collect();
    assert_eq!(keys, ["soapy.dc_cancel", "soapy.mode"]);
    // Only the setting that changed is written.
    let n = f.control().len();
    let t = with_run_time(&src, &[remote("sdrplay"), ("soapy.mode", Value::Choice(1))]);
    src.apply(&t).unwrap();
    assert_eq!(bytes_since(&f, n), vec![write("mode", "b")]);
    let n = f.control().len();
    src.apply(&t).unwrap();
    assert!(bytes_since(&f, n).is_empty());
    // A value the radio refuses.
    f.script.lock().unwrap().handler = Some(Arc::new(|n, _| {
        (n == call::WRITE_SETTING).then(|| fake::exception("no"))
    }));
    let t = with_run_time(
        &src,
        &[remote("sdrplay"), ("soapy.dc_cancel", Value::Bool(true))],
    );
    assert_eq!(
        src.apply(&t).unwrap_err(),
        DriverError::Refused {
            id: DESCRIPTOR.run_time_id(0)
        }
    );
    src.stop();
}

#[test]
fn clock_source_antenna_and_sensors_at_open() {
    let mut s = Script::rtl();
    s.replies.insert(
        call::LIST_CLOCK_SOURCES,
        fake::strings(&["internal", "external"]),
    );
    s.replies
        .insert(call::GET_CLOCK_SOURCE, fake::string("internal"));
    s.replies
        .insert(call::LIST_ANTENNAS, fake::strings(&["A", "B"]));
    s.replies
        .insert(call::LIST_SENSORS, fake::strings(&["temp"]));
    s.replies.insert(call::READ_SENSOR, fake::string("41.5"));
    let f = Fake::start(s);
    let t = vals(&[
        remote("usrp"),
        ("clocksrc", Value::Text("external".into())),
        ("antenna", Value::Text("B".into())),
    ]);
    let (mut src, _r) = start(&f, &t);
    let c = f.control();
    let ant = req(call::SET_ANTENNA, |p| {
        p.dir_ch(1, 0).string("B");
    });
    let clk = req(call::SET_CLOCK_SOURCE, |p| {
        p.string("external");
    });
    let ia = c.iter().position(|r| r.bytes == ant).expect("antenna");
    let ic = c.iter().position(|r| r.bytes == clk).expect("clock source");
    let ir = c
        .iter()
        .position(|r| r.call == call::SET_SAMPLE_RATE)
        .unwrap();
    assert!(ia < ic && ic < ir);
    let mut res = results();
    src.poll(&mut res);
    let id = res.decl().table_id(SENSORS.key).unwrap();
    let table = res.table(id).unwrap();
    assert_eq!(table.row("temp").unwrap().get(&ITEM_VALUE), Some("41.5"));
    src.stop();
    // A refused clock source fails the open.
    f.script.lock().unwrap().handler = Some(Arc::new(|n, _| {
        (n == call::SET_CLOCK_SOURCE).then(|| fake::exception("no 10 MHz"))
    }));
    let err = drv(&f).open(&f.addr(), &t).err().unwrap();
    assert_eq!(
        err,
        DriverError::Refused {
            id: own_id(SoapyRemoteSetting::ClockSource)
        }
    );
    // An antenna the radio does not list fails the open.
    let t = vals(&[remote("usrp"), ("antenna", Value::Text("C".into()))]);
    let err = drv(&f).open(&f.addr(), &t).err().unwrap();
    assert_eq!(
        err,
        DriverError::Refused {
            id: own_id(SoapyRemoteSetting::Antenna)
        }
    );
}

#[test]
fn stop_runs_the_close_sequence_and_closes_everything() {
    let f = Fake::start(Script::rtl());
    let (mut src, _r) = start(
        &f,
        &vals(&[remote("rtlsdr"), ("serverlog", Value::Bool(true))]),
    );
    assert!(
        f.wait(Duration::from_secs(2), |f| f.sh.open.load(Ordering::SeqCst)
            == 2),
        "control and log"
    );
    let n = f.control().len();
    src.stop();
    let got = bytes_since(&f, n);
    assert_eq!(
        got,
        vec![hex(DEACTIVATE), hex(CLOSE_STREAM), hex(UNMAKE), hex(HANGUP)]
    );
    assert!(
        f.wait(Duration::from_secs(2), |f| f.sh.open.load(Ordering::SeqCst)
            == 0),
        "no connection left open"
    );
    src.stop(); // twice is safe
}

#[test]
fn the_bias_tee_is_turned_off_first() {
    let mut s = Script::rtl();
    s.replies.insert(
        call::SETTING_INFO,
        info_list(&[bool_info("biastee", "false")]),
    );
    s.replies.insert(call::READ_SETTING, fake::string("false"));
    let f = Fake::start(s);
    let (mut src, _r) = start(
        &f,
        &vals(&[remote("rtlsdr"), ("bias_tee", Value::Bool(true))]),
    );
    assert!(src.capabilities().has_bias_tee);
    assert!(f
        .control()
        .iter()
        .any(|r| r.bytes == write("biastee", "true")));
    let n = f.control().len();
    src.stop();
    let got = bytes_since(&f, n);
    assert_eq!(got[0], write("biastee", "false"));
    assert_eq!(
        &got[1..],
        &[hex(DEACTIVATE), hex(CLOSE_STREAM), hex(UNMAKE), hex(HANGUP)]
    );
}

#[test]
fn a_refused_deactivate_still_unmakes() {
    let f = Fake::start(Script::rtl());
    let (mut src, _r) = start(&f, &vals(&[remote("rtlsdr")]));
    f.script
        .lock()
        .unwrap()
        .replies
        .insert(call::DEACTIVATE_STREAM, fake::exception("no"));
    let n = f.control().len();
    src.stop();
    let calls: Vec<u32> = f.since(n).iter().map(|r| r.call).collect();
    assert_eq!(
        calls,
        [
            call::DEACTIVATE_STREAM,
            call::CLOSE_STREAM,
            call::UNMAKE,
            call::HANGUP
        ]
    );
}

#[test]
fn stop_is_quick_with_a_silent_server_and_during_a_pending_make() {
    let f = Fake::start(Script::rtl());
    let mut t = quick();
    t.close_call = Duration::from_secs(1);
    let d = crate::SoapyRemoteDriver::with_timing(f.transport(), t);
    let mut src = d
        .open_source(&f.addr(), &vals(&[remote("rtlsdr")]))
        .unwrap();
    let (w, _r) = ring(1 << 20, src.sample_type());
    src.start(w).unwrap();
    {
        let mut s = f.script.lock().unwrap();
        for c in [
            call::DEACTIVATE_STREAM,
            call::CLOSE_STREAM,
            call::UNMAKE,
            call::HANGUP,
        ] {
            s.silent.insert(c);
        }
    }
    let t0 = Instant::now();
    src.stop();
    assert!(t0.elapsed() < Duration::from_secs(4), "{:?}", t0.elapsed());

    // A reconnect whose make never answers: stop returns within 1 s.
    let mut s = Script::rtl();
    s.stream.stop_after = Some(20);
    let f = Fake::start(s);
    let mut src = drv(&f)
        .open_source(&f.addr(), &vals(&[remote("rtlsdr")]))
        .unwrap();
    let (w, _r) = ring(1 << 20, src.sample_type());
    src.start(w).unwrap();
    f.script.lock().unwrap().silent.insert(call::MAKE);
    assert!(f.wait(Duration::from_secs(6), |f| f
        .calls()
        .iter()
        .filter(|c| **c == call::MAKE)
        .count()
        >= 2));
    std::thread::sleep(Duration::from_millis(50));
    let t0 = Instant::now();
    src.stop();
    assert!(t0.elapsed() < Duration::from_secs(1), "{:?}", t0.elapsed());
    assert!(
        f.wait(Duration::from_secs(2), |f| f.sh.open.load(Ordering::SeqCst)
            == 0)
    );
}

#[test]
fn the_log_connection() {
    let mut p = Packer::default();
    p.char(4).string("Overflow on the radio");
    assert_eq!(rpc::packet(&p.buf, rpc::VERSION), hex(LOG_PACKET));
    let mut s = Script::rtl();
    s.log = vec![
        (4, "Overflow on the radio".into()),
        (6, "info line\n".into()),
        (9, "OOS".into()),
    ];
    let f = Fake::start(s);
    let (mut src, _r) = start(
        &f,
        &vals(&[remote("rtlsdr"), ("serverlog", Value::Bool(true))]),
    );
    assert!(f.wait(Duration::from_secs(2), |_| src
        .ctx
        .stats
        .ssi_sequence
        .load(Ordering::Relaxed)
        == 1));
    let mut res = results();
    src.poll(&mut res);
    let n: Vec<String> = res.log_lines().iter().cloned().collect();
    assert!(
        n.contains(&"server: Overflow on the radio".to_string()),
        "{n:?}"
    );
    assert!(n.contains(&"server: info line".to_string()), "{n:?}");
    assert_eq!(res.status().summary, "server: Overflow on the radio");
    assert_eq!(src.ctx.stats.ssi_overflows.load(Ordering::Relaxed), 2);
    assert_eq!(res.fields().get(&OVERFLOWS), Some(2));
    assert_eq!(
        src.ctx.stats.status(),
        "2 overflows at the server (it or its network is too slow)"
    );
    src.stop();
    let log: Vec<Vec<u8>> =
        f.sh.requests
            .lock()
            .unwrap()
            .iter()
            .filter(|r| r.log)
            .map(|r| r.bytes.clone())
            .collect();
    assert_eq!(log, vec![hex(LOG_START), hex(LOG_STOP), hex(HANGUP)]);
}

#[test]
fn a_silent_log_server_does_not_hold_up_the_radio() {
    let mut s = Script::rtl();
    s.log_silent = true;
    let f = Fake::start(s);
    let t0 = Instant::now();
    let (mut src, r) = start(
        &f,
        &vals(&[remote("rtlsdr"), ("serverlog", Value::Bool(true))]),
    );
    assert!(
        t0.elapsed() < Duration::from_millis(900),
        "{:?}",
        t0.elapsed()
    );
    assert!(r.wait(100, Duration::from_secs(1)) >= 100);
    std::thread::sleep(Duration::from_millis(1300));
    let t = notes(&mut src);
    let about: Vec<&String> = t
        .iter()
        .filter(|l| l.contains("the server's log is not forwarded"))
        .collect();
    assert_eq!(about.len(), 1, "{t:?}");
    assert!(about[0].contains("no answer within 1.0 s"), "{t:?}");
    src.stop();
}

#[test]
fn a_flood_of_log_lines_is_limited() {
    let mut s = Script::rtl();
    s.log = (0..1000).map(|i| (6i8, format!("line {i}"))).collect();
    let f = Fake::start(s);
    let (mut src, _r) = start(
        &f,
        &vals(&[remote("rtlsdr"), ("serverlog", Value::Bool(true))]),
    );
    assert!(f.wait(Duration::from_secs(3), |_| src
        .ctx
        .stats
        .log_dropped
        .load(Ordering::Relaxed)
        >= 900));
    src.stop();
    let t = notes(&mut src);
    let lines = t.iter().filter(|l| l.starts_with("server: line")).count();
    assert!(lines <= 20, "{lines}");
    let summaries: Vec<&String> = t
        .iter()
        .filter(|l| l.ends_with("more server log lines"))
        .collect();
    assert_eq!(summaries.len(), 1, "{t:?}");
}
