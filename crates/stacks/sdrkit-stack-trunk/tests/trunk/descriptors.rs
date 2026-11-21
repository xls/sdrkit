//! Every stack through its descriptor: the list of stacks, the minimum rate, the stack's own generator decoded by its receiver,
//! the declared events, and the strict settings.

use crate::common::*;
use sdrkit_core::InputInfo;
use sdrkit_dev_test::Harness;
use sdrkit_stack_trunk::{
    descriptors, edacs, ermes, gsc, ltr, mpt, multitone, opensky, pager, scada, tones, type2,
};

/// The counter each stack fills when it decodes its own test signal, and how long the signal runs.
fn counter(id: &str) -> (&'static str, f64) {
    match id {
        "mpt1327" => ("messages", 2.5),
        "ltr" => ("words_good", 2.5),
        "edacs" => ("frames_ok", 4.0),
        "tones" => ("detections", 33.0),
        "pager" => ("pages", 3.0),
        "multitone" => ("transmissions", 6.0),
        "ermes" => ("batches", 2.0),
        "motorola_type2" => ("messages", 3.0),
        "opensky" => ("blocks", 3.0),
        "gsc" => ("pages", 5.0),
        "scada" => ("messages", 2.0),
        _ => unreachable!("{id}"),
    }
}

#[test]
fn the_stacks_and_no_selcal_stack() {
    let ids: Vec<&str> = descriptors().iter().map(|d| d.id).collect();
    assert_eq!(
        ids,
        [
            "mpt1327",
            "ltr",
            "edacs",
            "tones",
            "pager",
            "multitone",
            "ermes",
            "motorola_type2",
            "opensky",
            "gsc",
            "scada"
        ]
    );
}

#[test]
fn selcal_stack_id_belongs_to_avdata() {
    assert!(descriptors()
        .iter()
        .all(|d| d.id != sdrkit_proto_tones::SELCAL_STACK_ID));
}

#[test]
fn every_descriptor_decodes_its_generator() {
    for d in descriptors() {
        let rate = d.tuning.min_rate_hz.max(match d.id {
            "edacs" => 480e3,
            _ => 240e3,
        });
        let mut rx = make(d, &[]);
        let f = d.tuning.default_hz;
        assert!(
            rx.configure(&InputInfo::new(d.tuning.min_rate_hz * 0.5, f, f))
                .is_err(),
            "{} takes a rate below its minimum",
            d.id
        );
        let (key, seconds) = counter(d.id);
        let run = run_own_signal(d, rate, seconds);
        let decl = declaration(d.id);
        assert!(
            num(&run.results, key) >= 1.0,
            "{}: {key} is {}",
            d.id,
            text(&run.results, key)
        );
        check_events(&kinds(&decl), &run.events, true);
        assert!(!run.events.is_empty(), "{} gave no event", d.id);
    }
}

/// The stacks whose test signal lies where a radio tuned with the stack's offset sees the channel. The others make it at the
/// centre.
const AT_TUNE_OFFSET: &[&str] = &["ermes", "motorola_type2", "opensky", "gsc", "scada"];

/// Runs the test signal of `d` through its receiver, the channel where the generator puts it.
fn run_own_signal(
    d: &'static sdrkit_core::StackDescriptor,
    rate: f64,
    seconds: f64,
) -> sdrkit_dev_test::Run {
    let h = Harness::new(d).rate(rate).seconds(seconds);
    if !AT_TUNE_OFFSET.contains(&d.id) {
        return h.run().unwrap();
    }
    let x = h.generate().unwrap();
    let mut rx = make(d, &[]);
    let f = d.tuning.default_hz;
    configure(
        rx.as_mut(),
        rate,
        f + d.tuning.tune_offset_hz,
        -d.tuning.tune_offset_hz,
    );
    let fed = feed(rx.as_mut(), &x, 8192);
    let mut out = sdrkit_core::Output::default();
    rx.finish(&mut out);
    sdrkit_dev_test::Run {
        results: results(rx.as_ref()).clone(),
        events: fed.events,
        requests: fed.requests,
        ..Default::default()
    }
}

#[test]
fn a_bad_setting_is_refused_not_replaced() {
    use sdrkit_core::Value;
    let bad: &[(&str, &str, Value)] = &[
        ("mpt1327", "framing", Value::Choice(9)),
        ("mpt1327", "spacing_khz", Value::F64(-5.0)),
        ("ltr", "follow", Value::Text("perhaps".into())),
        ("edacs", "follow_groups", Value::Text("777 seven".into())),
        (
            "edacs",
            "lcn_table",
            Value::Text("856.0\nnot a channel\n".into()),
        ),
        ("tones", "gate", Value::Choice(7)),
        ("tones", "gate_tone", Value::Choice(400)),
        ("pager", "read", Value::Choice(9)),
        (
            "pager",
            "read_by_address",
            Value::Text("123:alpha 99:upside".into()),
        ),
        ("pager", "combine", Value::Text("often".into())),
        ("gsc", "gen_pages", Value::Choice(9)),
        ("gsc", "gen_deviation_hz", Value::F64(-5.0)),
        ("scada", "gen_link", Value::Choice(20)),
        ("scada", "gen_traffic", Value::Choice(7)),
    ];
    for (id, key, value) in bad {
        let d = descriptors().into_iter().find(|d| d.id == *id).unwrap();
        let mut rx = make(d, &[]);
        assert!(
            set(rx.as_mut(), key, value).is_err(),
            "{id}: {key} = {value:?} was taken"
        );
    }
}

#[test]
fn random_input_never_crashes() {
    // Noise, silence, a constant, huge and tiny values, NaN and infinity, in pieces of many sizes.
    let mut r = sdrkit_algo_sim::rng::Rng::new(5);
    for d in descriptors() {
        let rate = d.tuning.min_rate_hz.max(250e3);
        let mut rx = make(d, &[]);
        rx.configure(&InputInfo::new(rate, d.tuning.default_hz, d.tuning.default_hz).recording())
            .unwrap();
        let mut x = vec![num_complex::Complex32::new(0.0, 0.0); (rate * 0.5) as usize];
        for (k, z) in x.iter_mut().enumerate() {
            *z = match (k / 4096) % 6 {
                0 => num_complex::Complex32::new(r.uniform_f32() - 0.5, r.uniform_f32() - 0.5),
                1 => num_complex::Complex32::new(1.0, 0.0),
                2 => num_complex::Complex32::new(1e6, -1e6),
                3 => num_complex::Complex32::new(f32::NAN, f32::INFINITY),
                4 => num_complex::Complex32::new(1e-30, 0.0),
                _ => num_complex::Complex32::new(0.0, 0.0),
            };
        }
        for chunk in [1usize, 7, 4096, 65536] {
            feed(rx.as_mut(), &x[..x.len().min(chunk * 40)], chunk);
        }
        assert!(!results(rx.as_ref()).decl().is_empty());
    }
}

fn declaration(id: &str) -> sdrkit_core::ResultsDecl {
    match id {
        "mpt1327" => mpt::declaration(),
        "ltr" => ltr::declaration(),
        "edacs" => edacs::declaration(),
        "tones" => tones::declaration(),
        "pager" => pager::declaration(),
        "ermes" => ermes::declaration(),
        "motorola_type2" => type2::declaration(),
        "opensky" => opensky::declaration(),
        "gsc" => gsc::declaration(),
        "scada" => scada::declaration(),
        _ => multitone::declaration(),
    }
}
