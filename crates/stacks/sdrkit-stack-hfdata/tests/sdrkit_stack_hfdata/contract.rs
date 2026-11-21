//! The receiver contract for every stack of the crate: descriptors, refused rates, empty and damaged input, cut independence.

use crate::common::*;
use sdrkit_core::{owner, Complex32, InputInfo, Output, StackConfig, Value, ValueKind};
use sdrkit_stack_hfdata::descriptors;

#[test]
fn descriptors_are_complete() {
    let ids: Vec<&str> = descriptors().iter().map(|d| d.id).collect();
    assert_eq!(
        ids,
        [
            "ale",
            "pactor1",
            "wavebuoy",
            "hfradar",
            "hfbeacons",
            "pactor2",
            "pactor3",
            "pactor4",
            "hfid",
            "ms110",
            "s4285",
            "s4539",
            "ale3g",
            "s4481",
            "link11",
            "efr",
            "gtor",
            "arqe",
            "autospec",
            "piccolo",
            "coquelet",
            "cis_hf",
            "crowd36",
            "xpa",
            "fsk200",
            "gwfsk",
            "fsk_modems",
            "uvb76",
            "vlf_msk",
            "rpr",
        ]
    );
    for d in descriptors() {
        assert_eq!(d.group, "HF utility");
        let mut rx = (d.make_receiver)(&StackConfig::new(values(d, &[]))).unwrap();
        let schema = rx.schema().clone();
        for (i, s) in schema.settings().iter().enumerate() {
            let id = schema.id(&s.field.key).unwrap();
            assert_eq!(id.index(), i);
            assert!(
                owner::apply(rx.as_mut(), id, &s.default).is_ok(),
                "{} {}",
                d.id,
                s.field.key
            );
        }
        assert!(rx.schema().id("no_such_setting").is_err());
        let g = generator(d, 24_000.0, 1);
        assert_eq!(g.rate_hz(), 24_000.0);
    }
    assert!(sdrkit_stack_hfdata::pactor1::PACTOR1
        .name
        .contains("experimental"));
}

#[test]
fn bad_rates_are_refused_without_a_crash() {
    for d in descriptors() {
        let mut rx = receiver(d, &[]);
        for &r in &[0.0, -1.0, 1.0, f64::NAN] {
            assert!(
                rx.configure(&InputInfo::new(r, 14e6, 14e6)).is_err(),
                "{} {r}",
                d.id
            );
            let mut out = Output::default();
            rx.feed(&[Complex32::new(1.0, 0.0); 16], &mut out);
            rx.feed(&[], &mut out);
        }
        assert!(
            rx.results().status().summary.contains("too low"),
            "{}: {}",
            d.id,
            rx.results().status().summary
        );
        // A live radio below the minimum is refused; 10^12 Hz does not crash.
        assert!(rx.configure(&InputInfo::new(100e3, 14e6, 14e6)).is_err() || d.id == "wavebuoy");
        let _ = rx.configure(&InputInfo::new(1e12, 14e6, 14e6).recording());
    }
}

#[test]
fn damaged_input_is_cleaned() {
    for d in descriptors() {
        let mut rx = receiver(d, &[]);
        let rate = if d.id == "hfradar" { 250e3 } else { 24e3 };
        rx.configure(&InputInfo::new(rate, 14e6, 14e6).recording())
            .unwrap_or_else(|e| panic!("{}: {e}", d.id));
        let mut x = vec![Complex32::new(0.01, -0.01); 48_000];
        for (i, v) in x.iter_mut().enumerate() {
            if i % 97 == 0 {
                *v = Complex32::new(f32::NAN, f32::INFINITY);
            }
        }
        let mut out = Output::default();
        rx.feed(&x, &mut out);
        rx.feed(&[], &mut out);
        end(rx.as_mut());
        let j = serde_json::to_string(rx.results()).unwrap();
        assert!(!j.contains("NaN"), "{}", d.id);
        assert!(rx.results().status().snr_db.is_none_or_finite(), "{}", d.id);
    }
}

trait FiniteOpt {
    fn is_none_or_finite(&self) -> bool;
}
impl FiniteOpt for Option<f32> {
    fn is_none_or_finite(&self) -> bool {
        self.is_none_or(|v| v.is_finite())
    }
}

fn run_cut(id: &str, rate: f64, centre: f64, secs: f64, pieces: &[usize]) -> Vec<String> {
    let d = descriptors().into_iter().find(|d| d.id == id).unwrap();
    let settings = values(d, &[]);
    let mut g = generator(d, rate, 7);
    let x = generate(g.as_mut(), secs);
    let mut res = Vec::new();
    for &p in pieces {
        let mut rx = (d.make_receiver)(&StackConfig::new(settings.clone())).unwrap();
        rx.configure(&InputInfo::new(rate, centre, centre).recording())
            .unwrap();
        let mut out = Output::default();
        if p == 0 {
            // Pieces of varying size.
            let mut i = 0;
            let mut k = 1usize;
            while i < x.len() {
                let n = (k * 7919 % 9000 + 1).min(x.len() - i);
                rx.feed(&x[i..i + n], &mut out);
                out.clear();
                i += n;
                k += 1;
            }
        } else {
            for c in x.chunks(p) {
                rx.feed(c, &mut out);
                out.clear();
            }
        }
        end(rx.as_mut());
        let mut j = serde_json::to_value(rx.results()).unwrap();
        let o = j.as_object_mut().unwrap();
        for k in ["status", "series", "rasters", "points", "log"] {
            o.remove(k);
        }
        res.push(j.to_string());
    }
    res
}

#[test]
fn cut_independence() {
    for &(id, rate, centre, secs) in &[
        ("ale", 24e3, 14.109e6, 12.0),
        ("pactor1", 24e3, 14.08e6, 10.0),
        ("wavebuoy", 24e3, 30.4735e6, 8.0),
        ("hfradar", 250e3, 13.0e6, 3.0),
        ("hfbeacons", 24e3, 3.5897e6, 6.0),
        ("pactor2", 24e3, 14.072e6, 6.0),
        ("hfid", 24e3, 4.625e6, 6.0),
        ("ms110", 24e3, 6.5e6, 5.0),
        ("s4285", 24e3, 6.5e6, 5.0),
        ("s4539", 24e3, 6.5e6, 4.0),
        ("ale3g", 24e3, 6.5e6, 5.0),
        ("s4481", 24e3, 6.5e6, 4.0),
        ("link11", 24e3, 6.5e6, 3.0),
        ("efr", 24e3, 6.5e6, 4.0),
        ("gtor", 24e3, 6.5e6, 6.0),
        ("arqe", 24e3, 6.5e6, 20.0),
        ("autospec", 24e3, 6.5e6, 30.0),
        ("piccolo", 24e3, 6.5e6, 12.0),
        ("coquelet", 24e3, 6.5e6, 12.0),
        ("cis_hf", 24e3, 6.5e6, 8.0),
        ("crowd36", 24e3, 6.5e6, 10.0),
        ("xpa", 24e3, 6.5e6, 20.0),
        ("fsk200", 24e3, 6.5e6, 12.0),
        ("gwfsk", 24e3, 6.5e6, 8.0),
        ("fsk_modems", 24e3, 6.5e6, 6.0),
        ("uvb76", 24e3, 6.5e6, 16.0),
        ("vlf_msk", 24e3, 15e3, 16.0),
        ("rpr", 24e3, 10.1473e6, 9.0),
    ] {
        let r = run_cut(id, rate, centre, secs, &[1 << 20, 0, 4801]);
        assert_eq!(r[0], r[1], "{id}: one piece and varying pieces differ");
        assert_eq!(r[0], r[2], "{id}: one piece and 4801-sample pieces differ");
    }
}

#[test]
fn engine_style_builtin_sources() {
    // Each stack's built-in source through its own receiver at the default dial (the generator's dial is the capture centre).
    for &(id, rate, secs) in &[
        ("ale", 24e3, 12.0),
        ("wavebuoy", 24e3, 20.0),
        ("hfbeacons", 250e3, 25.0),
    ] {
        let d = descriptors().into_iter().find(|d| d.id == id).unwrap();
        let settings = values(d, &[]);
        let mut g = generator(d, rate, 3);
        if id == "hfbeacons" {
            // The beacons at 15 dB in 100 Hz, through the standard noise (the engine adds it from the gen_* keys). Without noise
            // the steady carrier 150 Hz from 'K' fills K's channel and its keying is not seen.
            let noise = values(
                d,
                &[
                    ("gen_noise", json!("snr")),
                    ("gen_snr_db", json!(15.0)),
                    ("gen_seed", json!(3)),
                ],
            );
            g = sdrkit_components::sim::impaired(g, &noise).expect("impairments");
        }
        let x = generate(g.as_mut(), secs);
        let mut rx = (d.make_receiver)(&StackConfig::new(settings)).unwrap();
        let centre = if id == "hfbeacons" {
            3.65e6
        } else if id == "wavebuoy" {
            d.tuning.default_hz - 1500.0
        } else {
            d.tuning.default_hz
        };
        rx.configure(&InputInfo::new(rate, centre, centre).recording())
            .unwrap();
        feed(rx.as_mut(), &x, (rate / 10.0) as usize);
        match id {
            "ale" => {
                let ev = rows(rx.results(), &sdrkit_stack_hfdata::typed::ALE_EVENTS);
                assert!(
                    ev.iter()
                        .any(|e| text(e, "type") == "Sounding" && text(e, "from") == "TSTA"),
                    "{ev:?}"
                );
                assert!(ev.iter().any(|e| text(e, "type") == "Individual"
                    && text(e, "from") == "TSTA"
                    && text(e, "to") == "TSTB"));
                assert!(ev.iter().all(|e| cell(e, "dial") == Value::Hz(14.109e6)));
            }
            "wavebuoy" => {
                let rx = rx.as_ref();
                let num = |k: &str| field(rx, k).unwrap_or(f64::NAN);
                assert_eq!(
                    rx.results().status().lock_state,
                    sdrkit_core::LockState::Locked
                );
                assert_eq!(field_text(rx, "format"), "2FSK");
                assert_eq!(field_text(rx, "channel"), "Default");
                assert!(num("vector_count") >= 20.0);
                assert!(num("lost_count") <= 3.0);
            }
            _ => {
                let rows = rows(rx.results(), &sdrkit_stack_hfdata::typed::HFBEACONS_BEACONS);
                let heard: Vec<String> = rows
                    .iter()
                    .filter(|b| cell(b, "heard") == Value::Bool(true))
                    .map(|b| text(b, "name"))
                    .collect();
                assert!(heard.iter().any(|n| n.starts_with("D ")), "{heard:?}");
                assert!(heard.iter().any(|n| n.starts_with("K ")), "{heard:?}");
                assert!(!rows
                    .iter()
                    .any(|b| cell(b, "frequency") == Value::Hz(3_595_450.0)));
            }
        }
    }
}

/// Values of the wrong kind, or out of range, for a declared setting, in the settings JSON form. A text setting takes any text,
/// so it has none.
fn bad_values(s: &sdrkit_core::SettingDecl) -> Vec<Json> {
    match s.field.kind {
        ValueKind::Bool => vec![json!("maybe")],
        ValueKind::F64 | ValueKind::Hz | ValueKind::Duration | ValueKind::I64 => {
            let mut v = vec![json!("many")];
            if let Some(r) = &s.range {
                v.push(json!(r.max + 1.0 + r.max.abs()));
                v.push(json!(r.min - 1.0 - r.min.abs()));
            }
            v
        }
        ValueKind::Choice => vec![json!("no such choice")],
        _ => Vec::new(),
    }
}

/// For each stack, the settings that accept a bad value (as "key=value").
fn lax_settings() -> Vec<(&'static str, Vec<String>)> {
    let mut lax = Vec::new();
    for d in descriptors() {
        let mut rx = (d.make_receiver)(&StackConfig::new(values(d, &[]))).unwrap();
        let schema = rx.schema().clone();
        let mut keys = Vec::new();
        for s in schema
            .settings()
            .iter()
            .filter(|s| !s.field.key.starts_with("gen_"))
        {
            for v in bad_values(s) {
                if set(rx.as_mut(), &s.field.key, v.clone()).is_ok() {
                    keys.push(format!("{}={v}", s.field.key));
                }
            }
        }
        lax.push((d.id, keys));
    }
    lax
}

/// The stacks whose settings refuse every bad value.
const STRICT_MODES: &[&str] = &[
    "ale",
    "pactor1",
    "wavebuoy",
    "hfbeacons",
    "pactor2",
    "pactor3",
    "pactor4",
    "hfid",
    "hfradar",
    "ms110",
    "s4285",
    "s4539",
    "ale3g",
    "s4481",
    "link11",
    "efr",
    "gtor",
    "arqe",
    "autospec",
    "piccolo",
    "coquelet",
    "cis_hf",
    "rpr",
];

#[test]
fn a_bad_setting_value_is_refused() {
    let lax = lax_settings();
    for (id, keys) in &lax {
        if !keys.is_empty() {
            eprintln!("{id} accepts {keys:?}");
        }
    }
    for (id, keys) in lax {
        if STRICT_MODES.contains(&id) {
            assert!(keys.is_empty(), "{id} accepts {keys:?}");
        }
    }
}

#[test]
fn every_component_is_registered_under_its_own_name() {
    let mut r = sdrkit_graph::Registry::default();
    sdrkit_stack_hfdata::register(&mut r);
    let names: Vec<String> = r.names().map(str::to_string).collect();
    assert_eq!(names.len(), 52);
    for n in &names {
        assert!(n.starts_with("hfdata."), "{n}");
        let c = r.make(n).unwrap_or_else(|e| panic!("{n}: {e}"));
        assert_eq!(&c.info().name, n);
    }
}

/// The rate and the centre each stack's test signal runs at here (the generator's dial is the capture centre).
fn test_signal_input(d: &sdrkit_core::StackDescriptor) -> (f64, f64) {
    match d.id {
        "hfradar" => (250e3, 13.0e6),
        "hfbeacons" => (250e3, 3.65e6),
        "wavebuoy" => (24e3, d.tuning.default_hz - 1500.0),
        _ => (24e3, d.tuning.default_hz),
    }
}

#[test]
fn every_verify_case_passes_on_the_test_signal() {
    use sdrkit_core::verify::{Outcome, VerifyOptions};
    use sdrkit_core::LockState;
    let o = VerifyOptions {
        seconds: 2.0,
        clean: true,
        snr_db: 20.0,
    };
    for d in descriptors() {
        let hooks = d.verify.expect("every stack has verify hooks");
        for case in (hooks.cases)(&o) {
            assert!(case.settings.is_empty());
            let settings = values(d, &[]);
            let (rate, centre) = test_signal_input(d);
            let mut g = (d.make_generator.unwrap())(
                &sdrkit_core::GenConfig::new(rate, settings.clone())
                    .with_submode_opt(case.submode.clone())
                    .with_seed(1),
            )
            .unwrap();
            let x = generate(g.as_mut(), case.seconds);
            let cfg = StackConfig::new(settings).with_submode_opt(case.submode.clone());
            let mut rx = (d.make_receiver)(&cfg).unwrap();
            rx.configure(&InputInfo::new(rate, centre, centre).recording())
                .unwrap();
            let mut out = Output::default();
            let mut locked = false;
            for c in x.chunks((rate / 10.0) as usize) {
                rx.feed(c, &mut out);
                out.clear();
                locked |= matches!(
                    rx.results().status().lock_state,
                    LockState::Tracking | LockState::Locked
                );
            }
            end(rx.as_mut());
            if case.expect_lock {
                assert!(locked, "{} {}: never tracking or locked", d.id, case.name);
            }
            for line in (case.check.expect("every case checks its results"))(rx.results()) {
                assert!(
                    line.outcome == Outcome::Pass,
                    "{} {}: {} {}",
                    d.id,
                    case.name,
                    line.key,
                    line.detail
                );
            }
        }
    }
}

/// The six serial-tone and naval HF stacks through the family front end at 250 ksps, the dial 20 kHz below the centre of the
/// samples, where their test signals put it: each reads what it reads at 24 kHz.
#[test]
fn the_front_end_at_250k() {
    use sdrkit_stack_hfdata::serial::gen::test_dial;
    for (id, secs, key) in [
        ("ms110", 5.0, "messages_total"),
        ("s4285", 8.0, "messages_total"),
        ("s4539", 6.0, "messages_total"),
        ("ale3g", 9.0, "pdus_good"),
        ("link11", 3.0, "messages_total"),
        ("s4481", 9.5, ""),
    ] {
        let d = descriptors().into_iter().find(|d| d.id == id).unwrap();
        let rate = 250e3;
        let mut g = generator(d, rate, 2);
        let x = generate(g.as_mut(), secs);
        let mut rx = receiver(d, &[]);
        let centre = d.tuning.default_hz;
        rx.configure(&InputInfo::new(rate, centre, centre + test_dial(rate)).recording())
            .unwrap();
        feed(rx.as_mut(), &x, 25_000);
        end(rx.as_mut());
        if key.is_empty() {
            let rows = rows(rx.results(), &sdrkit_stack_hfdata::typed::S4481_CHANNELS);
            assert!(
                rows.iter()
                    .any(|r| text(r, "last_text").contains("THE QUICK BROWN FOX")),
                "{id}"
            );
        } else {
            assert!(field(rx.as_ref(), key).unwrap_or(0.0) >= 1.0, "{id}");
        }
    }
}
