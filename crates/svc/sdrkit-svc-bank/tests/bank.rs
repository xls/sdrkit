//! The bank service read as an owner, the channelizer on a wide input, the carrier scan in a graph, and spans that start and stop
//! receivers.

use sdrkit_core::command::CommandError;
use sdrkit_core::{
    owner, AutoGain, Choice, Complex32, ConfigError, InputInfo, LockProfile, Output, Owner,
    Receiver, Record, Schema, StackConfig, StackDescriptor, Tuning, Value,
};
use sdrkit_graph::{Data, GraphBuilder};
use sdrkit_svc_bank::{
    BankService, CarrierScan, ChannelChange, ChannelOut, ChannelState, Channelize, Reading,
    SpanOpen, KEY,
};
use std::f64::consts::TAU;
use std::sync::Arc;
use std::time::Instant;

fn no_stack(id: &str) -> Option<&'static StackDescriptor> {
    let _ = id;
    None
}

static NARROW: StackDescriptor = StackDescriptor {
    id: "narrow",
    name: "Narrow",
    group: "Test",
    blurb: "A narrow test stack.",
    tuning: Tuning::anywhere(145e6, 48e3, 25e3, 12.5e3),
    submodes: &[],
    settings: Arc::<Schema>::default,
    make_receiver: |_: &StackConfig| -> Result<Box<dyn Receiver>, ConfigError> {
        unreachable!("the bank service never makes a receiver")
    },
    make_generator: None,
    lock_profile: LockProfile::DEFAULT,
    signal_types: &[],
    auto_gain: AutoGain::Radio,
    verify: None,
    benchmarks: None,
};

/// A lookup that resolves every stack id to the descriptor of a test stack.
fn any_stack(_: &str) -> Option<&'static StackDescriptor> {
    Some(&NARROW)
}

fn call(
    s: &mut BankService,
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

fn rows(s: &BankService, key: &str) -> Vec<(String, Record)> {
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

fn text(v: &str) -> Value {
    Value::Text(v.into())
}

fn row_key(reply: Option<Record>) -> String {
    match cell(&reply.unwrap(), "row") {
        Value::Text(t) => t.clone(),
        v => panic!("{v:?}"),
    }
}

/// `n` samples at `rate` around `centre` from sample `start`, with tones at these frequencies and amplitudes and a little noise.
fn capture(
    rate: f64,
    centre: f64,
    tones: &[(f64, f32)],
    start: usize,
    n: usize,
    seed: &mut u64,
) -> Vec<Complex32> {
    let mut x = vec![Complex32::default(); n];
    for &(freq, amp) in tones {
        let f = (freq - centre) / rate;
        for (i, v) in x.iter_mut().enumerate() {
            let p = TAU * ((f * (start + i) as f64) % 1.0);
            *v += Complex32::new(p.cos() as f32, p.sin() as f32) * amp;
        }
    }
    for v in &mut x {
        *seed ^= *seed << 13;
        *seed ^= *seed >> 7;
        *seed ^= *seed << 17;
        let a = (*seed & 0xffff) as f32 / 65536.0 - 0.5;
        let b = ((*seed >> 16) & 0xffff) as f32 / 65536.0 - 0.5;
        *v += Complex32::new(a, b) * 2e-3;
    }
    x
}

/// The frequency of the strongest tone in `x` at `rate`, from the mean phase step of its second half.
fn tone_hz(x: &[Complex32], rate: f64) -> f64 {
    let tail = &x[x.len() / 2..];
    let s: Complex32 = tail.windows(2).map(|w| w[1] * w[0].conj()).sum();
    f64::from(s.arg()) * rate / TAU
}

#[test]
fn the_service_is_an_owner_and_checks_arguments_first() {
    let mut s = BankService::new(Arc::new(any_stack));
    assert_eq!(KEY.as_str(), "service.bank");
    for t in ["channels", "carriers", "spans"] {
        assert!(s.results_decl().table_id(t).is_some(), "{t}");
    }
    for c in ["add", "remove", "set", "span.add", "span.remove"] {
        assert!(s.commands().id(c).is_ok(), "{c}");
    }
    assert!(matches!(
        s.commands().id("nope"),
        Err(CommandError::Unknown { .. })
    ));
    s.take_changes();
    let e = call(
        &mut s,
        "add",
        &[
            ("freq_hz", Value::Hz(145e6)),
            ("radio", text("1")),
            ("stack", text("fm")),
            ("bandwidth_hz", Value::Hz(-1.0)),
        ],
    )
    .unwrap_err();
    assert!(matches!(e, CommandError::BadArgument { .. }), "{e:?}");
    let e = call(
        &mut s,
        "add",
        &[("radio", text("1")), ("stack", text("fm"))],
    )
    .unwrap_err();
    assert!(matches!(e, CommandError::MissingArgument { .. }), "{e:?}");
    assert!(s.take_changes().is_empty(), "nothing ran");
    let e = call(&mut s, "remove", &[("row", text("c99"))]).unwrap_err();
    assert_eq!(
        e,
        CommandError::NoSuchRow {
            table: s.results_decl().table_id("channels").unwrap(),
            key: "c99".into()
        }
    );
    let e = call(
        &mut s,
        "span.add",
        &[
            ("radio", text("1")),
            ("stack", text("fm")),
            ("raster", text("1e6,x")),
        ],
    )
    .unwrap_err();
    assert!(matches!(e, CommandError::BadArgument { .. }), "{e:?}");
    let mut none = BankService::new(Arc::new(no_stack));
    let e = call(
        &mut none,
        "add",
        &[
            ("freq_hz", Value::Hz(145e6)),
            ("radio", text("1")),
            ("stack", text("fm")),
        ],
    )
    .unwrap_err();
    assert!(matches!(e, CommandError::BadArgument { .. }), "{e:?}");
}

#[test]
fn a_channel_row_refers_to_its_session_and_set_changes_it() {
    let mut s = BankService::new(Arc::new(any_stack));
    s.input(2.4e6, 145e6);
    let key = row_key(
        call(
            &mut s,
            "add",
            &[
                ("freq_hz", Value::Hz(145.5e6)),
                ("radio", text("1")),
                ("stack", text("fm")),
                ("label", text("Calling")),
            ],
        )
        .unwrap(),
    );
    let changes = s.take_changes();
    assert!(changes.iter().any(
        |c| matches!(c, ChannelChange::Start { channel, freq_hz, .. } if *channel == key && *freq_hz == 145.5e6)
    ));
    s.attached(&key, "7").unwrap();
    let r = &rows(&s, "channels")[0].1;
    assert_eq!(cell(r, "session"), &text("7"));
    assert_eq!(
        cell(r, "state"),
        &Value::Choice(ChannelState::Running.index())
    );
    let decl = s.results_decl();
    let t = decl.table(decl.table_id("channels").unwrap()).unwrap();
    let col = t.columns.field(t.columns.id("session").unwrap()).unwrap();
    assert_eq!(col.refers_to.as_ref().unwrap().table, "sessions");
    call(
        &mut s,
        "set",
        &[
            ("row", text(&key)),
            ("freq_hz", Value::Hz(145.6e6)),
            ("squelch", Value::Choice(2)),
            ("squelch_db", Value::F64(12.0)),
        ],
    )
    .unwrap();
    let r = &rows(&s, "channels")[0].1;
    assert_eq!(cell(r, "freq_hz"), &Value::Hz(145.6e6));
    assert_eq!(cell(r, "squelch_db"), &Value::F64(12.0));
    assert_eq!(cell(r, "label"), &text("Calling"));
    call(&mut s, "remove", &[("row", text(&key))]).unwrap();
    assert!(s
        .take_changes()
        .iter()
        .any(|c| matches!(c, ChannelChange::Stop { session: Some(x), .. } if x == "7")));
    assert!(rows(&s, "channels").is_empty());
}

#[test]
fn three_channels_on_one_wide_input_give_three_outputs_at_their_centres_in_one_pass() {
    let rate = 2.4e6;
    let centre = 145e6;
    let freqs = [144.6e6, 145.1e6, 145.7e6];
    let chans: Vec<ChannelOut> = freqs
        .iter()
        .map(|&f| ChannelOut {
            freq_hz: f,
            bandwidth_hz: 12.5e3,
            rate_hz: 25e3,
        })
        .collect();
    let mut g = split_graph(&chans);
    g.configure(&InputInfo::new(rate, centre, centre)).unwrap();
    assert!(g
        .component_as::<Channelize>("split")
        .unwrap()
        .bins()
        .is_some());
    let mut seed = 5;
    let tones: Vec<(f64, f32)> = freqs
        .iter()
        .enumerate()
        .map(|(k, f)| (f + 200.0 * (k as f64 + 1.0), 0.05))
        .collect();
    let x = capture(rate, centre, &tones, 0, 240_000, &mut seed);
    let mut out = Output::default();
    g.feed(&x, &mut out);
    assert_eq!(g.component_as::<Channelize>("split").unwrap().passes(), 1);
    for k in 0..3 {
        let b = g.buffer(&format!("split.ch{k}")).unwrap();
        let Data::Iq(v) = &b.data else { panic!() };
        let f = tone_hz(v, 25e3);
        let want = 200.0 * (k as f64 + 1.0);
        assert!((f - want).abs() < 2.0, "channel {k}: {f} Hz, want {want}");
    }
    let formats = g.report().ports.clone();
    for (k, &f) in freqs.iter().enumerate() {
        let (_, fmt) = formats
            .iter()
            .find(|(p, _)| *p == format!("split.ch{k}"))
            .unwrap();
        assert_eq!(fmt.centre_hz, f);
        assert_eq!(fmt.rate, 25e3);
    }
}

#[test]
fn thirty_two_channels_each_get_their_own_carrier_and_the_filterbank_is_fast() {
    let rate = 2.4e6;
    let centre = 100e6;
    // 32 channels 62.5 kHz apart. Each holds a carrier 100 * (k - 16) Hz from its centre.
    let freqs: Vec<f64> = (0..32)
        .map(|k| centre - 968.75e3 + k as f64 * 62.5e3)
        .collect();
    let tones: Vec<(f64, f32)> = freqs
        .iter()
        .enumerate()
        .map(|(k, &f)| (f + 100.0 * (k as f64 - 16.0), 0.02))
        .collect();
    let mut seed = 1;
    let x = capture(rate, centre, &tones, 0, 240_000, &mut seed);
    let chans: Vec<ChannelOut> = freqs
        .iter()
        .map(|&f| ChannelOut {
            freq_hz: f,
            bandwidth_hz: 12.5e3,
            rate_hz: 25e3,
        })
        .collect();
    let mut g = split_graph(&chans);
    g.configure(&InputInfo::new(rate, centre, centre)).unwrap();
    let m = g
        .component_as::<Channelize>("split")
        .unwrap()
        .bins()
        .unwrap();
    assert!(m >= 64, "{m} bins");
    let mut out = Output::default();
    g.feed(&x, &mut out);
    let mut best = f64::INFINITY;
    for _ in 0..5 {
        out.clear();
        let t0 = Instant::now();
        g.feed(&x, &mut out);
        best = best.min(t0.elapsed().as_secs_f64());
    }
    for k in 0..32 {
        let b = g.buffer(&format!("split.ch{k}")).unwrap();
        let Data::Iq(v) = &b.data else { panic!() };
        let f = tone_hz(v, 25e3);
        let want = 100.0 * (k as f64 - 16.0);
        assert!((f - want).abs() < 2.0, "channel {k}: {f} Hz, want {want}");
    }
    // 0.1 s of signal: the filterbank keeps up with real time even in a test build.
    assert!(best * 10.0 < 1.0, "{:.3} s for 1 s of signal", best * 10.0);
}

#[test]
fn carrier_scan_reports_on_and_off_in_a_graph() {
    let rate = 1e6;
    let centre = 100e6;
    let scan = CarrierScan::watching(&[(100.1e6, 10e3), (100.2e6, 10e3), (99.8e6, 10e3)]);
    let mut g = GraphBuilder::new()
        .add("scan", scan)
        .input("scan")
        .output("scan")
        .build()
        .unwrap();
    g.configure(&InputInfo::new(rate, centre, centre)).unwrap();
    let mut out = Output::default();
    let mut seed = 9;
    let on = capture(
        rate,
        centre,
        &[(100.1e6, 0.1), (99.8e6, 0.1)],
        0,
        200_000,
        &mut seed,
    );
    g.feed(&on, &mut out);
    let kinds: Vec<(&str, f64)> = out.events.iter().map(|e| (e.kind(), e.freq_hz)).collect();
    assert!(kinds.contains(&("carrier.on", 100.1e6)), "{kinds:?}");
    assert!(kinds.contains(&("carrier.on", 99.8e6)), "{kinds:?}");
    assert!(kinds.iter().all(|(_, f)| *f != 100.2e6));
    let readings: Vec<Reading> = g
        .component_as::<CarrierScan>("scan")
        .unwrap()
        .readings()
        .to_vec();
    assert_eq!(readings.len(), 3);
    assert!(
        readings[0].snr_db > 20.0 && readings[1].snr_db < 6.0,
        "{readings:?}"
    );
    out.clear();
    let off = capture(rate, centre, &[], 0, 3_000_000, &mut seed);
    for c in off.chunks(100_000) {
        g.feed(c, &mut out);
    }
    assert!(
        out.events
            .iter()
            .any(|e| e.kind() == "carrier.off" && e.freq_hz == 100.1e6),
        "{:?}",
        out.events
    );
}

/// A graph of one channelizer with a sink on every output, so every channel runs.
fn split_graph(chans: &[ChannelOut]) -> sdrkit_graph::Graph {
    let mut b = GraphBuilder::new()
        .add("split", Channelize::new(chans, 4096))
        .input("split");
    for k in 0..chans.len() {
        b = b
            .add(
                &format!("sink{k}"),
                sdrkit_graph::components::NullSink::default(),
            )
            .connect(&format!("split.ch{k}"), &format!("sink{k}"));
    }
    b.build().unwrap()
}

fn quiet(n: usize) -> Vec<Reading> {
    vec![
        Reading {
            level_dbfs: -90.0,
            snr_db: 0.0,
        };
        n
    ]
}

#[test]
fn span_receivers_start_on_activity_strongest_first_and_stop_when_idle() {
    let mut s = BankService::new(Arc::new(any_stack));
    s.input(2.4e6, 162e6);
    let span = row_key(
        call(
            &mut s,
            "span.add",
            &[
                ("label", text("Data")),
                ("radio", text("1")),
                ("stack", text("ais")),
                ("raster", text("161.5e6:25e3:40")),
                ("open", Value::Choice(SpanOpen::OnActivity.index())),
                ("threshold_db", Value::F64(10.0)),
                ("idle_s", Value::Duration(0.5)),
                ("max_receivers", Value::I64(2)),
            ],
        )
        .unwrap(),
    );
    assert!(rows(&s, "channels").is_empty());
    assert_eq!(rows(&s, "carriers").len(), 40);
    s.take_changes();
    // Carriers on raster channels 4 (161.6 MHz), 20 (162.0 MHz) and 32 (162.3 MHz), of different strengths.
    let mut r = quiet(40);
    for (i, snr) in [(4, 15.0), (20, 30.0), (32, 20.0)] {
        r[i] = Reading {
            level_dbfs: -40.0,
            snr_db: snr,
        };
    }
    for k in 0..5 {
        let mut all = quiet(rows(&s, "channels").len());
        all.extend_from_slice(&r);
        s.levels(k as f64 * 0.1, &all).unwrap();
    }
    let open: Vec<f64> = rows(&s, "channels")
        .iter()
        .map(|(_, r)| match cell(r, "freq_hz") {
            Value::Hz(f) => *f,
            v => panic!("{v:?}"),
        })
        .collect();
    assert_eq!(open.len(), 2, "{open:?}");
    assert!(
        open.contains(&162.0e6) && open.contains(&162.3e6),
        "{open:?}"
    );
    let starts = s
        .take_changes()
        .iter()
        .filter(|c| matches!(c, ChannelChange::Start { .. }))
        .count();
    assert_eq!(starts, 2);
    let waiting = s
        .results()
        .fields()
        .value(s.results_decl().field_id("waiting").unwrap())
        .cloned();
    assert_eq!(waiting, Some(Value::I64(1)));
    let carriers = rows(&s, "carriers");
    assert_eq!(
        carriers
            .iter()
            .filter(|(_, r)| cell(r, "active") == &Value::Bool(true))
            .count(),
        3
    );
    assert!(carriers
        .iter()
        .all(|(_, r)| cell(r, "span") == &text(&span)));
    // The carriers stop: the receivers stop after the idle time.
    let q = quiet(2 + 40);
    s.levels(1.0, &q).unwrap();
    assert_eq!(rows(&s, "channels").len(), 2, "still within the idle time");
    s.levels(1.6, &q).unwrap();
    assert!(rows(&s, "channels").is_empty());
    assert!(s.levels(2.0, &q).is_err(), "the watched channels changed");
    let stops = s
        .take_changes()
        .iter()
        .filter(|c| matches!(c, ChannelChange::Stop { .. }))
        .count();
    assert_eq!(stops, 2);
}

#[test]
fn a_span_of_fixed_channels_starts_them_all_and_channels_survive_a_retune() {
    let mut s = BankService::new(Arc::new(any_stack));
    s.input(2.4e6, 162e6);
    call(
        &mut s,
        "span.add",
        &[
            ("label", text("AIS")),
            ("radio", text("1")),
            ("stack", text("ais")),
            ("raster", text("161.975e6, 162.025e6")),
            ("bandwidth_hz", Value::Hz(25e3)),
        ],
    )
    .unwrap();
    call(
        &mut s,
        "add",
        &[
            ("freq_hz", Value::Hz(162.4e6)),
            ("radio", text("1")),
            ("stack", text("fm")),
        ],
    )
    .unwrap();
    assert_eq!(rows(&s, "channels").len(), 3);
    assert_eq!(cell(&rows(&s, "spans")[0].1, "radio"), &text("1"));
    let changes = s.take_changes();
    let starts = changes
        .iter()
        .filter(|c| matches!(c, ChannelChange::Start { radio, .. } if radio == "1"))
        .count();
    assert_eq!(starts, 3);
    assert!(changes.contains(&ChannelChange::Layout {
        radio: Some("1".to_owned())
    }));
    let state = |s: &BankService| -> Vec<Value> {
        rows(s, "channels")
            .iter()
            .map(|(_, r)| cell(r, "state").clone())
            .collect()
    };
    let outside = Value::Choice(ChannelState::Outside.index());
    assert!(state(&s).iter().all(|v| *v != outside));
    // A retune by 700 kHz keeps every channel inside. A retune far away keeps them, listed as outside.
    s.input(2.4e6, 162.7e6);
    assert!(state(&s).iter().all(|v| *v != outside));
    assert!(s
        .take_changes()
        .contains(&ChannelChange::Layout { radio: None }));
    s.input(2.4e6, 170e6);
    assert!(state(&s).iter().all(|v| *v == outside));
    // A channel moved by `set` follows.
    let manual = rows(&s, "channels")
        .into_iter()
        .find(|(_, r)| cell(r, "span") == &Value::None)
        .unwrap()
        .0;
    call(
        &mut s,
        "set",
        &[("row", text(&manual)), ("freq_hz", Value::Hz(170.1e6))],
    )
    .unwrap();
    let r = rows(&s, "channels")
        .into_iter()
        .find(|(k, _)| *k == manual)
        .unwrap()
        .1;
    assert_ne!(cell(&r, "state"), &outside);
    // The saved state produces the same channels and spans.
    let blob = s.save_state().unwrap();
    let mut t = BankService::new(Arc::new(any_stack));
    t.restore_state(&blob).unwrap();
    assert_eq!(rows(&t, "channels").len(), 3);
    assert_eq!(rows(&t, "spans").len(), 1);
}
