//! The span-block scan and the survey of the scan service on the test-signal radio: the Morse test signal is on the air
//! at one frequency, so one block of the scanned range holds it.

mod common;

use common::wait_for;
use sdrkit_core::net::FakeTransport;
use sdrkit_core::{CommandError, Facilities, MemStorage, Random, Record, Value, Values};
use sdrkit_host_engine::{Engine, Input, OwnerRef, SessionId, Setup};
use std::sync::Arc;

/// Where the Morse signal is on the air.
const AT_HZ: f64 = 7_035_000.0;
/// The time a test waits for the scan.
const T: f64 = 90.0;

/// An engine on the system clock with the Morse stack.
fn engine() -> Engine {
    Engine::new(Setup {
        facilities: Facilities {
            storage: Arc::new(MemStorage::new()),
            clock: sdrkit_host_engine::system_clock(),
            random: Random::fixed(7),
            codecs: Arc::default(),
        },
        transport: Arc::new(FakeTransport::new()),
        stacks: vec![&sdrkit_stack_hfkbd::CW],
        families: Vec::new(),
        components: sdrkit_graph::Registry::standard(),
    })
    .unwrap()
}

/// A session of the Morse stack on the test-signal radio, which has the signal at [`AT_HZ`] only. The radio starts
/// `away_hz` from the signal.
fn session(e: &Engine, away_hz: f64) -> SessionId {
    let kind = e.find("driver.synth").unwrap();
    let v: Values = common::defaults(e, kind);
    let radio = e.open_radio("synth", "synth:cw", &v).unwrap();
    for (key, value) in [
        ("centre_hz", Value::Hz(AT_HZ + away_hz)),
        ("gen_at_hz", Value::Hz(AT_HZ)),
        ("gen_wpm", Value::F64(30.0)),
        // Noise at a fixed ratio to the signal, and no grain of an 8-bit converter: the floor is steady.
        ("gen_noise", Value::Choice(1)),
        ("gen_snr_db", Value::F64(25.0)),
        ("quantize", Value::Bool(false)),
    ] {
        e.owner(OwnerRef::Radio(radio), |o| {
            let id = o.schema().id(key).unwrap();
            sdrkit_core::owner::apply(o, id, &value).unwrap();
        })
        .unwrap();
    }
    let stack = common::defaults(e, e.find("stack.cw").unwrap());
    e.start_stack("cw", None, &stack, Input::Radio(radio), None)
        .unwrap()
}

fn call(e: &Engine, cmd: &str, args: &[(&str, Value)]) -> Result<Option<Record>, CommandError> {
    let scan = e.find("service.scan").unwrap();
    e.owner(scan, |o| {
        let id = o.commands().id(cmd)?;
        let mut rec = o.commands().def(id).unwrap().new_args();
        for (k, v) in args {
            let f = rec.decl().id(k).unwrap();
            rec.set_value(f, v).unwrap();
        }
        sdrkit_core::owner::invoke(o, id, &rec)
    })
    .unwrap()
}

/// The status field `key` of the scan service.
fn status(e: &Engine, key: &str) -> Option<Value> {
    common::field(e, e.find("service.scan").unwrap(), key)
}

/// The key of the option in the choice field `key` of the scan service.
fn choice_key(e: &Engine, key: &str) -> String {
    use sdrkit_core::Choice;
    use sdrkit_svc_scan::{EndReason, ScanKind, ScanState};
    let Some(Value::Choice(i)) = status(e, key) else {
        return String::new();
    };
    match key {
        "state" => ScanState::from_index(i).map(Choice::key),
        "kind" => ScanKind::from_index(i).map(Choice::key),
        _ => EndReason::from_index(i).map(Choice::key),
    }
    .unwrap_or_default()
    .to_owned()
}

/// The value of setting `key` of the scan service.
fn setting(e: &Engine, key: &str) -> Option<Value> {
    let scan = e.find("service.scan").unwrap();
    e.owner(scan, |o| {
        let v = o.values()?;
        v.value(v.schema().id(key).ok()?).cloned()
    })
    .unwrap()
}

/// Sets setting `key` of the scan service to `value`.
fn set_setting(e: &Engine, key: &str, value: Value) {
    let scan = e.find("service.scan").unwrap();
    e.owner(scan, |o| {
        let id = o.schema().id(key).unwrap();
        sdrkit_core::owner::apply(o, id, &value).unwrap();
    })
    .unwrap();
}

/// The count of rows of table `table` of the scan service.
fn rows(e: &Engine, table: &str) -> usize {
    let scan = e.find("service.scan").unwrap();
    e.owner(scan, |o| {
        let res = o.results();
        res.table(res.decl().table_id(table).unwrap())
            .map_or(0, |t| t.rows_newest_first().count())
    })
    .unwrap()
}

fn num(v: Option<Value>) -> f64 {
    match v {
        Some(Value::Hz(x) | Value::F64(x) | Value::Duration(x)) => x,
        Some(Value::I64(x)) => x as f64,
        other => panic!("no number: {other:?}"),
    }
}

fn key(s: SessionId) -> Value {
    Value::Text(format!("session.s{}", s.to_u64()))
}

/// The span that the Morse stack of session `s` watches, from its result `span_hz`.
fn span_of(e: &Engine, s: SessionId) -> f64 {
    wait_for(T, || {
        match common::field(e, OwnerRef::Session(s), "span_hz") {
            Some(Value::Hz(hz)) if hz > 0.0 => Some(hz),
            _ => None,
        }
    })
    .expect("the stack gives its span")
}

/// The arguments of a span scan of five blocks of `span` with the signal in the fourth.
fn span_args(s: SessionId, span: f64, dwell_s: f64, total_s: f64) -> Vec<(&'static str, Value)> {
    vec![
        ("session", key(s)),
        ("start_hz", Value::Hz(AT_HZ - 3.5 * span)),
        ("stop_hz", Value::Hz(AT_HZ + 1.5 * span)),
        ("span_hz", Value::Hz(span)),
        ("watch", Value::Text("signals".into())),
        ("above", Value::F64(0.0)),
        ("dwell_s", Value::Duration(dwell_s)),
        ("total_s", Value::Duration(total_s)),
    ]
}

#[test]
fn a_dry_start_gives_the_plan_and_starts_nothing() {
    let e = engine();
    let s = session(&e, 0.0);
    let span = span_of(&e, s);
    let mut args = span_args(s, span, 4.0, 100.0);
    args.push(("dry", Value::Bool(true)));
    let r = call(&e, "span", &args).unwrap().expect("a plan");
    let get = |k: &str| num(r.value(r.decl().id(k).unwrap()).cloned());
    let settle = num(setting(&e, "settle"));
    assert_eq!(get("channels"), 5.0);
    assert_eq!(get("dwell_s"), 4.0);
    let pass = 5.0 * (settle + 4.0);
    assert!((get("pass_s") - pass).abs() < 1e-9, "{}", get("pass_s"));
    assert_eq!(get("total_s"), 100.0);
    assert!((get("passes") - 100.0 / pass).abs() < 1e-9);
    assert!((get("span_pass_s") - pass).abs() < 1e-9);
    assert_eq!(get("visit_s"), 0.0);
    // Nothing runs.
    std::thread::sleep(std::time::Duration::from_millis(400));
    assert_eq!(choice_key(&e, "state"), "idle");
    assert!(matches!(status(&e, "session"), None | Some(Value::None)));
}

/// Runs a span scan of five blocks with the signal in block `index`, and checks that the scan locks on that block
/// with one find.
fn locks_on_block(index: usize) {
    let e = engine();
    let s = session(&e, -40e3);
    let span = span_of(&e, s);
    let mut args = span_args(s, span, 12.0, 0.0);
    let first = AT_HZ - index as f64 * span;
    args[1] = ("start_hz", Value::Hz(first - 0.5 * span));
    args[2] = ("stop_hz", Value::Hz(first + 4.5 * span));
    call(&e, "span", &args).unwrap();
    let locked = wait_for(T, || (choice_key(&e, "state") == "locked").then_some(()));
    assert!(locked.is_some(), "state {}", choice_key(&e, "state"));
    // The centre of the block is the frequency of the signal.
    let at = num(status(&e, "freq_hz"));
    assert!((at - AT_HZ).abs() < 1.0, "locked at {at}");
    assert_eq!(num(status(&e, "index")), index as f64);
    assert!(num(status(&e, "watched")) > 0.0);
    assert_eq!(rows(&e, "hits"), 1);
    assert_eq!(choice_key(&e, "kind"), "span");
    // The scan leaves the find: the next block starts with no result of this one, so it gives no find.
    let skip = sdrkit_core::Choice::index(sdrkit_svc_scan::Action::Skip);
    call(&e, "control", &[("action", Value::Choice(skip))]).unwrap();
    let next = wait_for(T, || {
        (num(status(&e, "index")) == index as f64 + 1.0).then_some(())
    });
    assert!(
        next.is_some(),
        "the scan stays on block {:?}",
        status(&e, "index")
    );
    let t = std::time::Instant::now();
    while t.elapsed().as_secs_f64() < 3.0 {
        assert_ne!(choice_key(&e, "state"), "locked");
        assert!(matches!(
            status(&e, "watched"),
            None | Some(Value::None) | Some(Value::F64(0.0))
        ));
        assert_eq!(rows(&e, "hits"), 1);
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
    call(&e, "stop", &[]).unwrap();
}

#[test]
fn a_span_scan_finds_the_block_with_the_signal_and_locks() {
    locks_on_block(0);
}

#[test]
fn a_span_scan_locks_on_a_later_block() {
    locks_on_block(3);
}

#[test]
fn the_total_time_ends_a_scan() {
    let e = engine();
    let s = session(&e, -400e3);
    let span = span_of(&e, s);
    // Blocks far below the signal: nothing is found.
    let mut args = span_args(s, span, 0.5, 2.0);
    args[1] = ("start_hz", Value::Hz(AT_HZ - 500e3));
    args[2] = ("stop_hz", Value::Hz(AT_HZ - 500e3 + 3.0 * span));
    let t = std::time::Instant::now();
    call(&e, "span", &args).unwrap();
    let done = wait_for(T, || (choice_key(&e, "end") == "time_over").then_some(()));
    assert!(done.is_some(), "end {}", choice_key(&e, "end"));
    let took = t.elapsed().as_secs_f64();
    assert!((2.0..4.0).contains(&took), "the scan ended after {took} s");
    assert_eq!(choice_key(&e, "state"), "done");
    assert_eq!(rows(&e, "hits"), 0);
}

#[test]
fn a_survey_finds_the_keyed_carrier_in_one_wide_block_and_visits_it() {
    let e = engine();
    let s = session(&e, -300e3);
    let span = span_of(&e, s);
    let rate = 192e3;
    set_setting(&e, "look", Value::Duration(1.5));
    call(
        &e,
        "survey",
        &[
            ("session", key(s)),
            ("start_hz", Value::Hz(AT_HZ - 500e3)),
            ("stop_hz", Value::Hz(AT_HZ + 300e3)),
            ("span_hz", Value::Hz(span)),
            ("watch", Value::Text("signals".into())),
            ("above", Value::F64(0.0)),
            ("dwell_s", Value::Duration(8.0)),
            ("total_s", Value::Duration(0.0)),
            ("rate_hz", Value::Hz(rate)),
        ],
    )
    .unwrap();
    assert_eq!(choice_key(&e, "kind"), "survey");
    // Stage one sees the carrier in one wide block, stage two visits it and the stack finds it.
    let found = wait_for(T, || (rows(&e, "hits") > 0).then_some(()));
    assert!(
        found.is_some(),
        "state {}, {} candidates",
        choice_key(&e, "state"),
        rows(&e, "candidates")
    );
    assert!(rows(&e, "candidates") >= 1);
    let at = num(status(&e, "freq_hz"));
    assert!((at - AT_HZ).abs() < span / 2.0, "found at {at}");
    call(&e, "stop", &[]).unwrap();
}

/// A session of the Morse stack on the test-signal radio as an application opens it: the defaults of the radio (no noise,
/// rounding to 8 bits) with the station at `at_hz`, the radio settings `radio_set`, and the stack settings `stack`.
fn plain_session(
    e: &Engine,
    at_hz: f64,
    radio_set: &[(&str, Value)],
    stack: &[(&str, Value)],
) -> SessionId {
    let kind = e.find("driver.synth").unwrap();
    let radio = e
        .open_radio("synth", "synth:cw", &common::defaults(e, kind))
        .unwrap();
    let mut set = vec![
        ("centre_hz", Value::Hz(at_hz)),
        ("gen_at_hz", Value::Hz(at_hz)),
    ];
    set.extend(radio_set.iter().cloned());
    for (key, value) in set {
        e.owner(OwnerRef::Radio(radio), |o| {
            let id = o.schema().id(key).unwrap();
            sdrkit_core::owner::apply(o, id, &value).unwrap();
        })
        .unwrap();
    }
    let mut v = common::defaults(e, e.find("stack.cw").unwrap());
    for (key, value) in stack {
        let id = v.schema().id(key).unwrap();
        v.set(id, value).unwrap();
    }
    e.start_stack("cw", None, &v, Input::Radio(radio), None)
        .unwrap()
}

/// The value in column `col` of every row of table `table` of the scan service.
fn column(e: &Engine, table: &str, col: &str) -> Vec<Value> {
    let scan = e.find("service.scan").unwrap();
    e.owner(scan, |o| {
        let res = o.results();
        let t = res.table(res.decl().table_id(table).unwrap()).unwrap();
        let c = t.columns().id(col).unwrap();
        t.rows_newest_first()
            .filter_map(|(_, row)| row.value(c).cloned())
            .collect()
    })
    .unwrap()
}

/// A noise floor of about one step of the 8-bit rounding of the test radio (full scale 4, steps of 1/32), set by the
/// consumer above the noise that the radio adds itself.
const DITHER: &[(&str, Value)] = &[
    ("gen_noise", Value::Choice(5)),
    ("gen_noise_floor_dbfs", Value::F64(-27.0)),
];

/// The carriers of the default test signal of the Morse stack, Hz from the station.
const CARRIERS: [f64; 3] = [400.0, -1500.0, 2500.0];

/// Asserts that the newest find is within the width of the channel filter (100 Hz) of a carrier of the station at `at_hz`.
fn found_at_station(e: &Engine, at_hz: f64) {
    let f = num(column(e, "hits", "freq_hz").first().cloned());
    assert!(
        CARRIERS.iter().any(|c| (f - (at_hz + c)).abs() <= 100.0),
        "found at {f}, the station is at {at_hz}"
    );
}

/// Runs a span scan of 7.0 to 7.2 MHz with a dwell of 10 s, as an application starts it, with the station at `at_hz` and
/// the radio settings `radio_set`, and checks that the first find is on the block that holds the station, at a carrier of it.
fn span_scan_finds_the_station(at_hz: f64, radio_set: &[(&str, Value)]) {
    let e = engine();
    let s = plain_session(&e, at_hz, radio_set, &[]);
    let span = span_of(&e, s);
    let centres = sdrkit_svc_scan::blocks(7.0e6, 7.2e6, span);
    let block = (0..centres.len())
        .min_by(|&a, &b| {
            (centres[a] - at_hz)
                .abs()
                .total_cmp(&(centres[b] - at_hz).abs())
        })
        .unwrap();
    call(
        &e,
        "span",
        &[
            ("session", key(s)),
            ("start_hz", Value::Hz(7.0e6)),
            ("stop_hz", Value::Hz(7.2e6)),
            ("span_hz", Value::Hz(span)),
            ("watch", Value::Text("signals".into())),
            ("dwell_s", Value::Duration(10.0)),
            ("total_s", Value::Duration(0.0)),
        ],
    )
    .unwrap();
    let wait = 10.5 * (block + 3) as f64;
    let locked = wait_for(wait, || (choice_key(&e, "state") == "locked").then_some(()));
    assert!(locked.is_some(), "state {}", choice_key(&e, "state"));
    assert_eq!(
        num(status(&e, "index")),
        block as f64,
        "found at {:?}",
        column(&e, "hits", "freq_hz")
    );
    assert_eq!(rows(&e, "hits"), 1);
    found_at_station(&e, at_hz);
    call(&e, "stop", &[]).unwrap();
}

#[test]
fn a_span_scan_finds_the_station_in_the_second_block() {
    span_scan_finds_the_station(7.0203e6, DITHER);
}

#[test]
fn a_span_scan_finds_the_station_in_the_third_block() {
    span_scan_finds_the_station(7.035e6, DITHER);
}

#[test]
fn a_span_scan_finds_the_station_in_the_eighth_block() {
    span_scan_finds_the_station(7.1e6, DITHER);
}

#[test]
fn a_span_scan_on_the_radio_defaults_finds_the_station_in_the_second_block() {
    span_scan_finds_the_station(7.0203e6, &[]);
}

#[test]
fn a_span_scan_on_the_radio_defaults_finds_the_station_in_the_third_block() {
    span_scan_finds_the_station(7.035e6, &[]);
}

#[test]
fn a_span_scan_on_the_radio_defaults_finds_the_station_in_the_eighth_block() {
    span_scan_finds_the_station(7.1e6, &[]);
}

#[test]
fn a_signal_in_focus_that_proves_nothing_is_no_find() {
    let e = engine();
    // Noise only: the focus puts a picked signal without text in the table of every block.
    let s = plain_session(
        &e,
        7.0e6,
        &[("gen_noise_only", Value::Bool(true))],
        &[("focus_hz", Value::Hz(500.0))],
    );
    let span = span_of(&e, s);
    call(
        &e,
        "span",
        &[
            ("session", key(s)),
            ("start_hz", Value::Hz(7.0e6)),
            ("stop_hz", Value::Hz(7.0e6 + 2.0 * span)),
            ("span_hz", Value::Hz(span)),
            ("watch", Value::Text("signals".into())),
            ("dwell_s", Value::Duration(3.0)),
            ("total_s", Value::Duration(8.0)),
        ],
    )
    .unwrap();
    let done = wait_for(T, || (choice_key(&e, "end") == "time_over").then_some(()));
    assert!(done.is_some(), "end {}", choice_key(&e, "end"));
    assert_eq!(rows(&e, "hits"), 0);
}

#[test]
fn a_watch_that_the_stack_of_the_session_does_not_show_is_refused() {
    let e = engine();
    let s = plain_session(&e, 7.0e6, &[], &[]);
    let mut args = span_args(s, 48e3, 3.0, 8.0);
    args.retain(|(k, _)| *k != "watch");
    args.push(("watch", Value::Text("nothing".into())));
    let e1 = call(&e, "span", &args).unwrap_err().to_string();
    assert!(
        e1.contains("the stack cw has no result nothing to watch") && e1.contains("signals"),
        "{e1}"
    );
    assert_eq!(choice_key(&e, "state"), "idle", "nothing started");
}

#[test]
fn a_survey_called_at_once_after_the_session_starts_takes_the_rate_the_session_asked_for() {
    let e = engine();
    let s = plain_session(&e, 434.5e6, &[], &[]);
    let args = [
        ("session", key(s)),
        ("start_hz", Value::Hz(430e6)),
        ("stop_hz", Value::Hz(440e6)),
        ("span_hz", Value::Hz(48e3)),
        ("watch", Value::Text("signals".into())),
        ("total_s", Value::Duration(0.0)),
    ];
    let r = call(&e, "survey", &args).unwrap().expect("a plan");
    // The rate that the session asked of its radio, which its samples then have.
    let cw = &sdrkit_stack_hfkbd::CW;
    let rate = cw.rate_hz(&Values::defaults(&(cw.settings)()));
    let looked = sdrkit_svc_scan::blocks(430e6, 440e6, rate * sdrkit_svc_scan::USED_SHARE);
    let channels = r.value(r.decl().id("channels").unwrap());
    assert_eq!(channels, Some(&Value::I64(looked.len() as i64)), "rate {rate}");
    let look = wait_for(T, || (choice_key(&e, "state") == "look").then_some(()));
    assert!(look.is_some(), "state {}", choice_key(&e, "state"));
}

/// Runs a survey of 430 to 440 MHz with the defaults of the command and the station at 434.5 MHz, inside the third block,
/// and checks that the station is a candidate and the scan locks on it within two passes, at a carrier of it.
fn survey_finds_the_station(radio_set: &[(&str, Value)]) {
    let e = engine();
    let at = 434.5e6;
    let s = plain_session(&e, at, radio_set, &[]);
    let span = span_of(&e, s);
    let args = [
        ("session", key(s)),
        ("start_hz", Value::Hz(430e6)),
        ("stop_hz", Value::Hz(440e6)),
        ("span_hz", Value::Hz(span)),
        ("watch", Value::Text("signals".into())),
        ("total_s", Value::Duration(0.0)),
    ];
    call(&e, "survey", &args).unwrap();
    let locked = wait_for(T, || (choice_key(&e, "state") == "locked").then_some(()));
    assert!(
        locked.is_some(),
        "state {}, {} candidates",
        choice_key(&e, "state"),
        rows(&e, "candidates")
    );
    assert!(num(status(&e, "pass")) <= 1.0);
    let listed = column(&e, "candidates", "freq_hz");
    assert!(
        listed.iter().any(|f| CARRIERS
            .iter()
            .any(|c| (num(Some(f.clone())) - (at + c)).abs() <= 200.0)),
        "candidates {listed:?}"
    );
    found_at_station(&e, at);
    call(&e, "stop", &[]).unwrap();
}

#[test]
fn a_survey_lists_the_station_and_locks_on_it() {
    survey_finds_the_station(DITHER);
}

#[test]
fn a_survey_on_the_radio_defaults_lists_the_station_and_locks_on_it() {
    survey_finds_the_station(&[]);
}

#[test]
fn a_survey_that_is_stopped_gives_the_radio_its_rate_back() {
    let e = engine();
    let s = session(&e, -300e3);
    let span = span_of(&e, s);
    let radio = e.find("radio.r1").unwrap();
    // The rate that the radio is set to.
    let rate = || {
        num(e
            .owner(radio, |o| {
                let v = o.values()?;
                v.value(v.schema().id("rate").ok()?).cloned()
            })
            .unwrap())
    };
    let before = rate();
    assert_ne!(before, 192e3);
    set_setting(&e, "look", Value::Duration(3.0));
    call(
        &e,
        "survey",
        &[
            ("session", key(s)),
            ("start_hz", Value::Hz(AT_HZ - 500e3)),
            ("stop_hz", Value::Hz(AT_HZ + 300e3)),
            ("span_hz", Value::Hz(span)),
            ("watch", Value::Text("signals".into())),
            // Without a total time a survey of this range runs its stage one.
            ("total_s", Value::Duration(0.0)),
            ("rate_hz", Value::Hz(192e3)),
        ],
    )
    .unwrap();
    assert_eq!(choice_key(&e, "kind"), "survey");
    // Stage one runs at the wide rate.
    let wide = wait_for(T, || (rate() == 192e3).then_some(()));
    assert!(wide.is_some(), "the radio runs at {}", rate());
    call(&e, "stop", &[]).unwrap();
    let back = wait_for(T, || (rate() == before).then_some(()));
    assert!(
        back.is_some(),
        "the radio stays at {}, it had {before}",
        rate()
    );
}
