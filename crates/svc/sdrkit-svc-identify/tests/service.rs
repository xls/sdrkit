//! The identify service read as an owner: tables and commands by key, refused arguments, a capture of a synthetic signal
//! identified through the tables, a cancelled job, and a candidate opened as a host request.

use sdrkit_core::command::CommandError;
use sdrkit_core::TuneTarget;
use sdrkit_core::{
    owner, Choice, Complex32, Event, HostRequest, Owner, OwnerKey, OwnerSetting, Record, Value,
};
use sdrkit_dev_test::Awgn;
use sdrkit_svc_identify::synth::fsk;
use sdrkit_svc_identify::table::dmr_syncs;
use sdrkit_svc_identify::{IdentifyService, IdentifyState, OpenWith, Target, KEY};
use std::sync::Arc;

const RATE: f64 = 250e3;

fn opens(signal: &str) -> Option<OpenWith> {
    (signal == "dmr").then(|| OpenWith {
        stack: "voice".into(),
        submode: "dmr".into(),
        settings: vec![OwnerSetting {
            owner: OwnerKey::of("stack.voice"),
            key: "slot".into(),
            value: Value::I64(1),
        }],
    })
}

fn call(
    s: &mut IdentifyService,
    key: &str,
    args: &[(&str, Value)],
) -> Result<Option<Record>, CommandError> {
    let id = s.commands().id(key)?;
    let mut given = s.commands().def(id).unwrap().new_args();
    for (k, v) in args {
        let fid = given.decl().id(k).unwrap();
        given.set_value(fid, v).unwrap();
    }
    owner::invoke(s, id, &given)
}

fn rows(s: &IdentifyService, key: &str) -> Vec<(String, Record)> {
    let id = s.results_decl().table_id(key).unwrap();
    let mut v: Vec<(String, Record)> = s
        .results()
        .table(id)
        .unwrap()
        .rows_newest_first()
        .map(|(k, r)| (k.to_owned(), r.clone()))
        .collect();
    v.sort_by(|a, b| a.0.cmp(&b.0));
    v
}

fn cell<'a>(r: &'a Record, key: &str) -> &'a Value {
    r.value(r.decl().id(key).unwrap()).unwrap()
}

fn field<'a>(s: &'a IdentifyService, key: &str) -> &'a Value {
    s.results()
        .fields()
        .value(s.results_decl().field_id(key).unwrap())
        .unwrap()
}

fn state(s: &IdentifyService) -> Value {
    field(s, "state").clone()
}

fn choice<C: Choice>(c: C) -> Value {
    Value::Choice(c.index())
}

/// DMR-like 4FSK at 4800 Bd with its base station voice sync, at `offset_hz`, over noise.
fn dmr(offset_hz: f64) -> Vec<Complex32> {
    let syncs = dmr_syncs();
    let mut x = fsk(
        RATE,
        4800.0,
        &[-3, -1, 1, 3],
        648.0,
        300_000,
        31,
        Some((&syncs[0], 144)),
        None,
    );
    for (i, v) in x.iter_mut().enumerate() {
        let p = std::f64::consts::TAU * offset_hz * i as f64 / RATE;
        *v *= Complex32::new(p.cos() as f32, p.sin() as f32);
    }
    Awgn::with_sigma(0.01, 3).process(&mut x);
    x
}

#[test]
fn the_service_is_an_owner_and_checks_arguments_first() {
    let mut s = IdentifyService::new(Arc::new(opens));
    assert_eq!(KEY.as_str(), "service.identify");
    for t in [
        "signals",
        "candidates",
        "evidence",
        "notes",
        "priors",
        "open_settings",
        "why",
        "systems",
    ] {
        assert!(s.results_decl().table_id(t).is_some(), "{t}");
    }
    for c in ["arm", "capture", "open", "cancel"] {
        assert!(s.commands().id(c).is_ok(), "{c}");
    }
    for k in ["keep", "verify_top"] {
        assert!(s.schema().id(k).is_ok(), "{k}");
    }
    assert!(matches!(
        s.commands().id("nope"),
        Err(CommandError::Unknown { .. })
    ));
    // Refused by the declaration before the service runs: the state does not move.
    let e = call(&mut s, "arm", &[("offset_hz", Value::Hz(1e12))]).unwrap_err();
    assert!(matches!(e, CommandError::BadArgument { .. }), "{e:?}");
    let e = call(&mut s, "open", &[("session", Value::Text("1".into()))]).unwrap_err();
    assert!(matches!(e, CommandError::MissingArgument { .. }), "{e:?}");
    assert_eq!(state(&s), choice(IdentifyState::Idle));
    // A region with its edges the wrong way round.
    let e = call(
        &mut s,
        "arm",
        &[
            ("target", choice(Target::Region)),
            ("lo_hz", Value::Hz(5e3)),
            ("hi_hz", Value::Hz(-5e3)),
        ],
    )
    .unwrap_err();
    assert!(matches!(e, CommandError::BadArgument { .. }), "{e:?}");
    // No capture is held yet.
    let e = call(&mut s, "capture", &[]).unwrap_err();
    assert!(matches!(e, CommandError::NotNow { .. }), "{e:?}");
    let e = call(
        &mut s,
        "open",
        &[
            ("row", Value::Text("s0/c0".into())),
            ("session", Value::Text("1".into())),
        ],
    )
    .unwrap_err();
    assert_eq!(
        e,
        CommandError::NoSuchRow {
            table: s.results_decl().table_id("candidates").unwrap(),
            key: "s0/c0".into()
        }
    );
    assert!(
        s.deliver(&[Complex32::default(); 8], RATE, 446e6).is_err(),
        "not armed"
    );
}

#[test]
fn a_capture_returns_candidates_a_verdict_and_an_open_request() {
    let mut s = IdentifyService::new(Arc::new(opens));
    call(
        &mut s,
        "arm",
        &[
            ("target", choice(Target::Click)),
            ("offset_hz", Value::Hz(5e3)),
        ],
    )
    .unwrap();
    assert_eq!(state(&s), choice(IdentifyState::Armed));
    assert!(s.wanted(RATE).is_some());
    let x = dmr(5e3);
    s.deliver(&x, RATE, 446e6).unwrap();
    assert_eq!(state(&s), choice(IdentifyState::Working));
    assert!(s.wanted(RATE).is_none());
    let job = s.take_job().unwrap();
    (job.job)(&job.control).unwrap();
    s.tick(1.0);
    assert_eq!(state(&s), choice(IdentifyState::Done));
    assert_eq!(field(&s, "progress"), &Value::F64(1.0));
    let signals = rows(&s, "signals");
    assert_eq!(signals.len(), 1);
    let cands = rows(&s, "candidates");
    let best = cands.iter().find(|(k, _)| k == "s0/c0").unwrap();
    assert_eq!(cell(&best.1, "signal_type"), &Value::Text("dmr".into()));
    assert_eq!(cell(&best.1, "stack"), &Value::Text("voice".into()));
    assert_ne!(
        field(&s, "verdict"),
        &Value::Choice(0),
        "a verdict other than unknown"
    );
    assert!(!rows(&s, "evidence").is_empty());
    assert!(!rows(&s, "why").is_empty());
    let settings = rows(&s, "open_settings");
    assert_eq!(
        settings.len(),
        rows(&s, "candidates")
            .iter()
            .filter(|(_, r)| cell(r, "stack") == &Value::Text("voice".into()))
            .count()
    );
    let mut ev: Vec<Event> = Vec::new();
    s.take_events(&mut ev);
    assert_eq!(ev.len(), 1);
    assert_eq!(ev[0].kind(), "identify.verdict");
    // Open the best candidate on session 1.
    call(
        &mut s,
        "open",
        &[
            ("row", Value::Text("s0/c0".into())),
            ("session", Value::Text("1".into())),
        ],
    )
    .unwrap();
    match s.take_requests().as_slice() {
        [HostRequest::Tune {
            session,
            target: TuneTarget::Stack(stack),
            submode,
            freq_hz: Some(f),
            settings,
            ..
        }] => {
            assert_eq!(session, "1");
            assert_eq!(stack, "voice");
            assert_eq!(submode, "dmr");
            assert!((f - 446.005e6).abs() < 2e3, "{f}");
            assert_eq!(settings.len(), 1);
        }
        other => panic!("{other:?}"),
    }
    // A signal type that no stack opens fails with the service's error.
    let other = cands
        .iter()
        .find(|(_, r)| cell(r, "signal_type") != &Value::Text("dmr".into()));
    if let Some((k, _)) = other {
        let e = call(
            &mut s,
            "open",
            &[
                ("row", Value::Text(k.clone())),
                ("session", Value::Text("1".into())),
            ],
        )
        .unwrap_err();
        assert!(matches!(e, CommandError::Failed { .. }), "{e:?}");
    }
    // The held capture serves a new look with no new samples.
    call(&mut s, "capture", &[("target", choice(Target::All))]).unwrap();
    assert!(s.take_job().is_some());
}

#[test]
fn a_cancelled_job_ends_cancelled() {
    let mut s = IdentifyService::new(Arc::new(opens));
    call(&mut s, "arm", &[]).unwrap();
    s.deliver(&dmr(0.0), RATE, 446e6).unwrap();
    let job = s.take_job().unwrap();
    call(&mut s, "cancel", &[]).unwrap();
    assert_eq!(state(&s), choice(IdentifyState::Cancelled));
    assert!(job.control.cancelled());
    (job.job)(&job.control).unwrap();
    s.tick(1.0);
    assert_eq!(state(&s), choice(IdentifyState::Cancelled));
    assert!(rows(&s, "signals").is_empty());
}

#[test]
fn a_job_cancelled_while_it_runs_ends_cancelled_with_an_event() {
    let mut s = IdentifyService::new(Arc::new(opens));
    call(&mut s, "arm", &[]).unwrap();
    s.deliver(&dmr(0.0), RATE, 446e6).unwrap();
    let job = s.take_job().unwrap();
    job.control.cancel();
    (job.job)(&job.control).unwrap();
    s.tick(1.0);
    assert_eq!(state(&s), choice(IdentifyState::Cancelled));
    let mut ev = Vec::new();
    s.take_events(&mut ev);
    assert_eq!(ev.len(), 1);
}

#[test]
fn the_armed_capture_names_its_session() {
    let mut s = IdentifyService::new(Arc::new(opens));
    assert_eq!(s.armed_session(), None);
    call(&mut s, "arm", &[]).unwrap();
    assert_eq!(s.armed_session(), None);
    call(&mut s, "arm", &[("session", Value::Text("7".into()))]).unwrap();
    assert_eq!(s.armed_session(), Some("7"));
    s.deliver(&dmr(0.0), RATE, 446e6).unwrap();
    assert_eq!(s.armed_session(), None);
}
