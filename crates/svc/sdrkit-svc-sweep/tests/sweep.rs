//! Sweeps driven end to end by a stand-in for the engine, with the fake radio. Everything is read through the owner.

mod common;

use common::fake::{FakeRadio, FakeSignal, Rng};
use common::*;
use sdrkit_core::owner::Owner;
use sdrkit_core::{CommandError, HostRequest, Storage, StorePath, Value};
use sdrkit_svc_sweep::{SpectrumFrame, Step, SweepService, HIT};

/// The centres of the hops of a sweep from `low_hz` to `high_hz` at `bin_hz`, as the service tunes them.
fn hop_centres(low_hz: f64, high_hz: f64, bin_hz: f64) -> Vec<f64> {
    let (mut s, _) = service();
    set(&mut s, "settle_auto", Value::Bool(false));
    set(&mut s, "sweeps", Value::I64(1));
    set(&mut s, "bin_hz", Value::Hz(bin_hz));
    let mut d = Driver::new(FakeRadio::new(RATE, low_hz, 1));
    d.start(
        &mut s,
        &[
            ("low_hz", Value::Hz(low_hz)),
            ("high_hz", Value::Hz(high_hz)),
        ],
    );
    d.run(&mut s, 30.0).unwrap();
    let mut c = d.tunes.clone();
    c.dedup();
    c
}

fn near<'a>(rows: &[&'a sdrkit_core::Record], hz: f64) -> Option<&'a sdrkit_core::Record> {
    rows.iter()
        .copied()
        .find(|r| (num(r, "freq_hz") - hz).abs() < 8e3)
}

fn range(low_hz: f64, high_hz: f64) -> [(&'static str, Value); 2] {
    [
        ("low_hz", Value::Hz(low_hz)),
        ("high_hz", Value::Hz(high_hz)),
    ]
}

#[test]
fn finds_channels_flags_images_and_spurs_and_measures_settle() {
    let centres = hop_centres(100e6, 110e6, 2.5e3);
    assert!(centres.len() >= 5);
    let (mut s, _) = service();
    set(&mut s, "bin_hz", Value::Hz(2.5e3));
    set(&mut s, "averages", Value::I64(4));
    set(&mut s, "hang", Value::Duration(0.05));
    set(&mut s, "sweeps", Value::I64(30));
    let strong_hz = centres[2] + 400e3;
    let image_hz = centres[2] - 400e3;
    let mut radio = FakeRadio::new(RATE, 99e6, 7);
    radio.settle_s = 0.002;
    radio.rolloff = true;
    radio.dc_dbfs = Some(-35.0);
    radio.image_db = Some(-38.0);
    radio.spur = Some((312.5e3, -68.0));
    radio.add(FakeSignal::carrier(101.0e6 + 7e3, -62.0));
    radio.add(FakeSignal::carrier(strong_hz, -22.0));
    radio.add(FakeSignal::wide(centres[4] - 500e3, -50.0, 16e3));
    radio.add(FakeSignal::carrier(centres[1] + 200e3, -55.0).keyed(0.12, 0.3));
    let mut d = Driver::new(radio);
    d.start(&mut s, &range(100e6, 110e6));
    let back = d.run(&mut s, 30.0);
    assert_eq!(back, Some(99e6), "returns to the user's frequency");
    assert_eq!(field_num(&s, "sweeps"), 30.0);

    let settle = field_num(&s, "settle");
    assert!(settle > 0.0015 && settle < 0.0035, "settle {settle}");
    assert_eq!(field(&s, "settle_measured"), &Value::Bool(true));

    let visible = visible(&s);
    let weak = near(&visible, 101.007e6).expect("weak carrier");
    assert_eq!(choice(weak, "type"), "carrier");
    assert!(near(&visible, strong_hz).is_some(), "strong carrier");
    let wide = near(&visible, centres[4] - 500e3).expect("wide signal");
    assert!(
        matches!(choice(wide, "type"), "nfm" | "digital"),
        "{}",
        choice(wide, "type")
    );
    let all: Vec<_> = rows(&s, "findings").into_iter().map(|r| r.1).collect();
    let keyed = near(&all, centres[1] + 200e3).expect("keyed signal");
    assert!(num(keyed, "count") >= 2.0, "returning signal counted");
    assert!(num(keyed, "present") < 0.8);

    // Hidden: the image and the spurs. Nothing at DC.
    assert!(
        all.iter()
            .any(|c| flag(c, "image") && (num(c, "freq_hz") - image_hz).abs() < 8e3),
        "image flagged"
    );
    assert!(
        all.iter().filter(|c| flag(c, "spur")).count() >= 3,
        "spurs flagged"
    );
    for c in &visible {
        let hz = num(c, "freq_hz");
        assert!(
            centres.iter().all(|h| (hz - h).abs() > 10e3),
            "nothing at a hop centre: {hz}"
        );
    }

    // The floor is flat despite the roll-off, and the pseudo-spectrum is published.
    let mut f: Vec<f32> = series(&s, "sweep.floor")
        .iter()
        .map(|p| p[1])
        .filter(|v| *v > -150.0)
        .collect();
    f.sort_by(|a, b| a.total_cmp(b));
    let (p10, p50, p90) = (f[f.len() / 10], f[f.len() / 2], f[f.len() * 9 / 10]);
    assert!(p90 - p10 < 3.0, "flat floor {p10} {p50} {p90}");
    for k in [
        "sweep.current",
        "sweep.average",
        "sweep.max",
        "sweep.floor",
        "sweep.threshold",
    ] {
        assert!(!series(&s, k).is_empty(), "{k}");
    }
    assert!(rows(&s, "findings").len() >= 3);
    let hits: Vec<_> = d.events.iter().filter(|e| e.is(&HIT)).collect();
    let state = |e: &&sdrkit_core::Event| choice(&e.fields, "hit").to_owned();
    assert!(hits.iter().any(|e| state(e) == "started"));
    assert!(hits.iter().any(|e| state(e) == "ended"));
}

fn floor_median(s: &SweepService) -> f32 {
    let mut f: Vec<f32> = series(s, "sweep.floor")
        .iter()
        .map(|p| p[1])
        .filter(|v| *v > -150.0)
        .collect();
    f.sort_by(|a, b| a.total_cmp(b));
    f[f.len() / 2]
}

#[test]
fn follows_a_gain_change_and_takes_actions() {
    let (mut s, st) = service();
    set(&mut s, "settle_auto", Value::Bool(false));
    set(&mut s, "settle", Value::Duration(0.001));
    set(&mut s, "sweeps", Value::I64(6));
    set(&mut s, "bin_hz", Value::Hz(5e3));
    let mut radio = FakeRadio::new(RATE, 145e6, 9);
    radio.add(FakeSignal::carrier(145.5e6 + 3e3, -50.0));
    let mut d = Driver::new(radio);
    d.start(&mut s, &range(144e6, 146e6));
    d.run(&mut s, 10.0);
    assert_eq!(field_num(&s, "settle"), 0.001);
    assert_eq!(field(&s, "settle_measured"), &Value::Bool(false));
    let before = floor_median(&s);
    let saved = call(&mut s, "floor.save", &[("path", text("floors/2m.floor"))])
        .unwrap()
        .unwrap();
    assert!(num(&saved, "bins") > 300.0);

    // Ten dB more gain: the floor follows within one sweep.
    let mut s2 = SweepService::new(st.clone(), sdrkit_core::Clock::fixed(0.0));
    set(&mut s2, "settle_auto", Value::Bool(false));
    set(&mut s2, "settle", Value::Duration(0.001));
    set(&mut s2, "sweeps", Value::I64(1));
    set(&mut s2, "bin_hz", Value::Hz(5e3));
    d.radio.gain_db = 10.0;
    d.start(&mut s2, &range(144e6, 146e6));
    let restored = call(
        &mut s2,
        "floor.restore",
        &[("path", text("floors/2m.floor"))],
    )
    .unwrap()
    .unwrap();
    assert_eq!(num(&restored, "bins"), num(&saved, "bins"));
    d.run(&mut s2, 20.0);
    let after = floor_median(&s2);
    assert!((after - before - 10.0).abs() < 2.0, "{before} -> {after}");

    // Actions on the carrier.
    let id = visible(&s)[0];
    let row = rows(&s, "findings")
        .into_iter()
        .find(|r| std::ptr::eq(r.1, id))
        .unwrap()
        .0
        .to_owned();
    let t = call(
        &mut s,
        "hit_action",
        &[("row", text(&row)), ("action", Value::Choice(0))],
    )
    .unwrap()
    .unwrap();
    assert_eq!(cell(&t, "demod"), &text("cw"));
    let t = call(
        &mut s,
        "hit_action",
        &[
            ("row", text(&row)),
            ("action", Value::Choice(0)),
            ("demod", text("nfm")),
        ],
    )
    .unwrap()
    .unwrap();
    assert_eq!(cell(&t, "demod"), &text("nfm"));
    let req = s.take_requests();
    assert_eq!(req.len(), 2);
    assert!(
        matches!(&req[1], HostRequest::Tune { session, submode, freq_hz: Some(f), .. }
            if session == "1" && submode == "nfm" && (f - 145.503e6).abs() < 5e3)
    );
    call(
        &mut s,
        "hit_action",
        &[("row", text(&row)), ("action", Value::Choice(3))],
    )
    .unwrap();
    let l = call(
        &mut s,
        "hit_action",
        &[("row", text(&row)), ("action", Value::Choice(2))],
    )
    .unwrap()
    .unwrap();
    assert!(num(&l, "high_hz") > num(&l, "low_hz"));
    assert_eq!(rows(&s, "findings").len(), 0);
    assert!(matches!(
        call(
            &mut s,
            "hit_action",
            &[("row", text("999")), ("action", Value::Choice(0))]
        ),
        Err(CommandError::NoSuchRow { .. })
    ));
    assert!(matches!(
        call(&mut s, "nonsense", &[]),
        Err(CommandError::Unknown { .. })
    ));
    let written = call(
        &mut s,
        "export",
        &[
            ("path", text("out/sweep.csv")),
            ("format", Value::Choice(0)),
        ],
    )
    .unwrap()
    .unwrap();
    let csv = st.read(&StorePath::new("out/sweep.csv").unwrap()).unwrap();
    assert_eq!(num(&written, "written"), csv.len() as f64);
    assert!(String::from_utf8(csv)
        .unwrap()
        .starts_with("1970-01-01, 00:00:00, 144000000"));
    call(
        &mut s,
        "export",
        &[
            ("path", text("out/traces.csv")),
            ("format", Value::Choice(1)),
        ],
    )
    .unwrap();
    let tr = st.read(&StorePath::new("out/traces.csv").unwrap()).unwrap();
    assert!(tr.starts_with(b"frequency_hz,live_dbfs"));
}

#[test]
fn scheduled_sweeps_sleep_in_between() {
    let (mut s, _) = service();
    set(&mut s, "settle_auto", Value::Bool(false));
    set(&mut s, "settle", Value::Duration(0.001));
    set(&mut s, "every", Value::Duration(2.0));
    set(&mut s, "sweeps", Value::I64(3));
    let mut d = Driver::new(FakeRadio::new(RATE, 431e6, 3));
    d.start(&mut s, &range(430e6, 432e6));
    assert_eq!(d.run(&mut s, 60.0), Some(431e6));
    assert_eq!(field_num(&s, "sweeps"), 3.0);
    assert_eq!(d.waits, 2);
    assert!(d.radio.time_s() >= 4.0 && d.radio.time_s() < 4.5);
}

#[test]
fn radio_sweep_is_used_and_falls_back() {
    let setup = |s: &mut SweepService| {
        set(s, "settle_auto", Value::Bool(false));
        set(s, "settle", Value::Duration(0.0));
        set(s, "sweeps", Value::I64(3));
    };
    let mut radio = FakeRadio::new(RATE, 410e6, 4);
    radio.add(FakeSignal::carrier(407.3e6, -50.0));
    // The radio sweeps by itself.
    let (mut s, _) = service();
    setup(&mut s);
    let mut d = Driver::new(radio.clone());
    d.can_sweep = true;
    d.radio_sweeps = true;
    d.start(&mut s, &range(400e6, 420e6));
    assert_eq!(d.run(&mut s, 30.0), Some(410e6));
    assert_eq!(field_choice(&s, "radio_sweep"), "used");
    assert!(
        series(&s, "sweep.current").iter().all(|p| p[1] > -150.0),
        "every bin covered by the radio's sweep"
    );
    assert!(near(&visible(&s), 407.3e6).is_some());
    // It cannot: retuning, and the field says so.
    let (mut s, _) = service();
    setup(&mut s);
    let mut d = Driver::new(radio);
    d.can_sweep = true;
    d.start(&mut s, &range(400e6, 420e6));
    assert_eq!(d.run(&mut s, 30.0), Some(410e6));
    assert_eq!(field_choice(&s, "radio_sweep"), "refused");
    assert!(near(&visible(&s), 407.3e6).is_some());
}

#[test]
fn survey_hunt_and_health_purposes() {
    // Survey.
    let (mut s, st) = service();
    set(&mut s, "settle_auto", Value::Bool(false));
    set(&mut s, "settle", Value::Duration(0.001));
    set(&mut s, "sweeps", Value::I64(8));
    set(&mut s, "bin_hz", Value::Hz(5e3));
    let mut radio = FakeRadio::new(RATE, 162.5e6, 11);
    radio.add(FakeSignal::wide(162.4e6, -50.0, 15e3));
    radio.add(FakeSignal::carrier(162.8e6, -55.0).keyed(0.05, 0.2));
    let mut d = Driver::new(radio);
    let mut args = range(162e6, 163e6).to_vec();
    args.push(("purpose", Value::Choice(1)));
    d.start(&mut s, &args);
    d.run(&mut s, 30.0);
    assert_eq!(field_num(&s, "survey_sweeps"), 8.0);
    let p = rows(&s, "persistent");
    assert!(
        p.iter()
            .any(|(_, r)| num(r, "low_hz") <= 162.4e6 && num(r, "high_hz") >= 162.4e6),
        "persistent"
    );
    call(
        &mut s,
        "quiet",
        &[("width_hz", Value::Hz(25e3)), ("count", Value::I64(2))],
    )
    .unwrap();
    assert!(!rows(&s, "quiet").is_empty());
    let id = s.results_decl().raster_id("survey").unwrap();
    assert_eq!(s.results().raster(id).unwrap().height, 8);
    call(
        &mut s,
        "export",
        &[("path", text("old.survey")), ("format", Value::Choice(4))],
    )
    .unwrap();
    assert!(st.read(&StorePath::new("old.survey").unwrap()).is_ok());
    let diff = call(&mut s, "diff", &[("path", text("old.survey"))])
        .unwrap()
        .unwrap();
    assert_eq!(num(&diff, "changes"), 0.0);

    // Hunt: learn, then a new carrier appears.
    let (mut s, _) = service();
    set(&mut s, "settle_auto", Value::Bool(false));
    set(&mut s, "settle", Value::Duration(0.001));
    set(&mut s, "sweeps", Value::I64(20));
    set(&mut s, "bin_hz", Value::Hz(5e3));
    let mut radio = FakeRadio::new(RATE, 162.5e6, 12);
    radio.add(FakeSignal::carrier(162.2e6, -50.0));
    radio.add(FakeSignal::carrier(162.65e6 + 2e3, -60.0).between(0.03, 1e9));
    let mut d = Driver::new(radio);
    let mut args = range(162e6, 163e6).to_vec();
    args.push(("purpose", Value::Choice(2)));
    d.start(&mut s, &args);
    d.run(&mut s, 60.0);
    assert_eq!(field(&s, "learnt"), &Value::Bool(true));
    let found = rows(&s, "interference");
    assert_eq!(found.len(), 1);
    assert!((num(found[0].1, "peak_hz") - 162.652e6).abs() < 6e3);
    assert!(field_num(&s, "meter_rise_db") > 10.0);

    // Health, on a radio that reaches FM broadcast and airband only.
    let (mut s, _) = service();
    set(&mut s, "settle_auto", Value::Bool(false));
    set(&mut s, "settle", Value::Duration(0.001));
    let mut radio = FakeRadio::new(RATE, 100e6, 13);
    radio.add(FakeSignal::carrier(95e6, -30.0));
    let mut d = Driver::new(radio);
    d.max_hz = 140e6;
    d.start(&mut s, &[("purpose", Value::Choice(3))]);
    assert_eq!(d.run(&mut s, 60.0), Some(100e6));
    let bands = rows(&s, "health_bands");
    assert_eq!(bands.len(), 2);
    let fm = bands.iter().find(|b| b.0 == "FM broadcast").unwrap();
    assert!(num(fm.1, "signals") >= 1.0);
    assert!(!rows(&s, "advice").is_empty());
    // The gap between FM broadcast and airband is not visited.
    assert!(d.tunes.iter().all(|&c| !(109.0e6..=117.0e6).contains(&c)));
}

#[test]
fn synthetic_spectrum_frames_give_findings_through_the_tables() {
    let (mut s, _) = service();
    set(&mut s, "settle_auto", Value::Bool(false));
    set(&mut s, "settle", Value::Duration(0.0));
    set(&mut s, "averages", Value::I64(2));
    set(&mut s, "sweeps", Value::I64(3));
    call(
        &mut s,
        "start",
        &[
            ("session", text("1")),
            ("low_hz", Value::Hz(100e6)),
            ("high_hz", Value::Hz(104e6)),
        ],
    )
    .unwrap();
    let radio = sdrkit_svc_sweep::RadioInfo {
        min_hz: 24e6,
        max_hz: 1766e6,
        rate_hz: RATE,
        can_sweep: false,
        tuned_hz: 100e6,
    };
    let mut step = s.step(0.0, &radio).unwrap().unwrap();
    let need = s.spectrum().unwrap();
    let n = need.bins;
    let mut t = 0.0;
    let mut rng = Rng::new(1);
    let target = 102.31e6;
    let mut done = false;
    let mut bins = vec![0.0f32; n];
    for _ in 0..10_000 {
        let centre = match step {
            Step::Tune { centre_hz, .. } => centre_hz,
            Step::Done { .. } => {
                done = true;
                break;
            }
            other => panic!("{other:?}"),
        };
        loop {
            t += 0.01;
            for b in bins.iter_mut() {
                *b = -90.0 + rng.gauss() as f32;
            }
            let k = ((target - centre) / (RATE / n as f64) + (n / 2) as f64).round();
            if k >= 0.0 && (k as usize) < n {
                bins[k as usize] = -40.0;
            }
            let frame = SpectrumFrame {
                time_s: t,
                centre_hz: centre,
                span_hz: RATE,
                bins_db: &bins,
            };
            if let Some(s) = s.feed(&frame).unwrap() {
                step = s;
                break;
            }
        }
    }
    assert!(done);
    assert!(near(&visible(&s), target).is_some());
    // A frame of another size is refused.
    let short = SpectrumFrame {
        time_s: t,
        centre_hz: 100e6,
        span_hz: RATE,
        bins_db: &bins[..n / 2],
    };
    call(
        &mut s,
        "start",
        &[
            ("session", text("1")),
            ("low_hz", Value::Hz(100e6)),
            ("high_hz", Value::Hz(104e6)),
        ],
    )
    .unwrap();
    let Some(Step::Tune { centre_hz, .. }) = s.step(t, &radio).unwrap() else {
        panic!("tune");
    };
    assert!(matches!(
        s.feed(&SpectrumFrame { centre_hz, ..short }),
        Err(sdrkit_svc_sweep::SweepError::FrameShape { .. })
    ));
}

#[test]
fn bursts_and_drifting_carriers_through_the_sweep() {
    let (mut s, _) = service();
    set(&mut s, "settle_auto", Value::Bool(false));
    set(&mut s, "settle", Value::Duration(0.0));
    set(&mut s, "averages", Value::I64(2));
    set(&mut s, "every", Value::Duration(0.05));
    set(&mut s, "sweeps", Value::I64(120));
    set(&mut s, "bin_hz", Value::Hz(2e3));
    set(&mut s, "burst_hz", Value::Hz(150.3e6));
    set(&mut s, "drift", Value::Bool(true));
    let mut radio = FakeRadio::new(RATE, 150.5e6, 21);
    radio.add(FakeSignal::carrier(150.3e6, -70.0));
    radio.add(FakeSignal::carrier(150.3e6, -55.0).between(2.0, 2.5));
    radio.add(FakeSignal::carrier(150.7e6, -55.0).drifting(150.0));
    let mut d = Driver::new(radio);
    d.start(&mut s, &range(150e6, 151e6));
    d.run(&mut s, 60.0);
    let b = rows(&s, "bursts");
    assert_eq!(b.len(), 1);
    assert!((num(b[0].1, "duration") - 0.5).abs() < 0.11);
    assert_eq!(
        d.events
            .iter()
            .filter(|e| e.is(&sdrkit_svc_sweep::BURST))
            .count(),
        1
    );
    let drift = rows(&s, "drift");
    assert!(
        drift
            .iter()
            .any(|(_, t)| (num(t, "drift_hz_s") - 150.0).abs() < 30.0
                && (num(t, "first_hz") - 150.7e6).abs() < 5e3),
        "{drift:?}"
    );
}

#[test]
fn parks_on_a_hop_while_recording() {
    let (mut s, _) = service();
    set(&mut s, "settle_auto", Value::Bool(false));
    set(&mut s, "settle", Value::Duration(0.0));
    set(&mut s, "record", Value::Bool(true));
    set(&mut s, "record_max_park", Value::Duration(0.3));
    set(&mut s, "sweeps", Value::I64(12));
    let mut radio = FakeRadio::new(RATE, 103e6, 31);
    radio.add(FakeSignal::carrier(101.2e6, -50.0));
    let mut d = Driver::new(radio);
    d.start(&mut s, &range(100e6, 106e6));
    let mut seen = false;
    d.run_with(&mut s, 30.0, |s, _| {
        if let Some(r) = s.recordings().first() {
            seen = true;
            assert!((r.hz - 101.2e6).abs() < 10e3, "{r:?}");
            assert_eq!(rows(s, "recordings").len(), s.recordings().len());
        }
    });
    assert!(seen);
    assert!(field_num(&s, "sweeps") >= 3.0);
}

#[test]
fn a_new_start_keeps_where_to_tune_back() {
    let (mut s, _) = service();
    set(&mut s, "settle_auto", Value::Bool(false));
    set(&mut s, "sweeps", Value::I64(1));
    let mut d = Driver::new(FakeRadio::new(RATE, 88e6, 2));
    d.start(&mut s, &range(100e6, 110e6));
    // The first sweep moved the radio to a hop. A new start replaces it and still tunes back to 88 MHz.
    d.run(&mut s, 0.01);
    assert!((d.radio.centre_hz() - 88e6).abs() > 1e6);
    call(
        &mut s,
        "exclude",
        &[("low_hz", Value::Hz(101e6)), ("high_hz", Value::Hz(102e6))],
    )
    .unwrap();
    d.start(&mut s, &range(430e6, 432e6));
    assert_eq!(d.run(&mut s, 30.0), Some(88e6));
    // Nothing runs: an exclusion is refused.
    assert!(matches!(
        call(
            &mut s,
            "exclude",
            &[("low_hz", Value::Hz(1e8)), ("high_hz", Value::Hz(2e8))]
        ),
        Err(CommandError::NotNow { .. })
    ));
}
