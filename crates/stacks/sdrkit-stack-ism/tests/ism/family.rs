//! The stacks on the framework: every descriptor has a generator and a receiver, the minimum rate says why, results do not depend
//! on how the input is cut, and saved state comes back with its ages.

use crate::util::{clear, gen_defaults, rows, set, stack_defaults};
use sdrkit_core::{
    Complex32, InputInfo, Output, Receiver, Results, StackDescriptor, StateBlob, Value, ValueKind,
};
use sdrkit_stack_ism::descriptors;

/// The stack ids of the crate.
const IDS: [&str; 11] = [
    "ism",
    "wmbus",
    "zwave",
    "zigbee",
    "wisun",
    "sigfox",
    "ant",
    "nrf24",
    "subghzchat",
    "dect",
    "nfc",
];

/// The clean test signal of a stack at its default rate, `seconds` long, as a radio tuned with the stack's offset sees it.
fn signal(m: &StackDescriptor, seconds: f64, seed: u64) -> Vec<Complex32> {
    let rate = m.tuning.rate_hz;
    let mut g =
        (m.make_generator.expect("a generator"))(&gen_defaults(m, rate).with_seed(seed)).unwrap();
    let mut x = vec![Complex32::default(); (rate * seconds) as usize];
    let n = g.generate(&mut x);
    x.truncate(n);
    // As the engine does: the station moves to where a radio tuned with the stack's offset sees it.
    let own = m.tuning.tune_offset_hz;
    let shift = -own - g.station_offset_hz(own);
    if shift.abs() > 0.5 {
        sdrkit_algo_dsp::nco::Nco::new(rate, shift).mix_in_place(&mut x);
    }
    x
}

/// Where the receiver is tuned for the test signal of a stack.
fn input(m: &StackDescriptor) -> InputInfo {
    InputInfo::new(
        m.tuning.rate_hz,
        m.tuning.default_hz + m.tuning.tune_offset_hz,
        m.tuning.default_hz,
    )
    .recording()
}

/// A receiver of a stack at its defaults, configured for its test signal.
fn receiver(m: &StackDescriptor) -> Box<dyn Receiver> {
    let mut r = (m.make_receiver)(&stack_defaults(m)).unwrap();
    r.configure(&input(m))
        .unwrap_or_else(|e| panic!("{}: {e}", m.id));
    r
}

fn feed(r: &mut dyn Receiver, x: &[Complex32], piece: usize) {
    let mut out = Output::default();
    for c in x.chunks(piece) {
        r.feed(c, &mut out);
        out.clear();
    }
}

/// How long the test signal of a stack runs before it has decoded something.
fn seconds(id: &str) -> f64 {
    match id {
        "sigfox" => 8.0,
        "mioty" => 2.0,
        "subghzchat" | "wmbus" | "zwave" | "ism" => 1.6,
        _ => 0.5,
    }
}

/// The fields and tables of the results without the ones that hold wall-clock times.
fn comparable(r: &Results) -> serde_json::Value {
    let all = serde_json::to_value(r).expect("results as JSON");
    let mut v = serde_json::json!({ "fields": all["fields"], "tables": all["tables"] });
    strip_wall_clock(&mut v);
    v
}

fn strip_wall_clock(v: &mut serde_json::Value) {
    match v {
        serde_json::Value::Object(o) => {
            for k in ["time", "first_heard", "last_heard"] {
                o.remove(k);
            }
            o.values_mut().for_each(strip_wall_clock);
        }
        serde_json::Value::Array(a) => a.iter_mut().for_each(strip_wall_clock),
        _ => {}
    }
}

#[test]
fn every_mode_has_a_descriptor_generator_and_receiver() {
    let d = descriptors();
    let ids: Vec<&str> = d.iter().map(|x| x.id).collect();
    for id in IDS {
        assert!(ids.contains(&id), "{id}");
    }
    for m in d {
        let x = signal(m, 0.05, 1);
        let mut r = receiver(m);
        feed(r.as_mut(), &x, 65536);
        let low = r.configure(&InputInfo::new(
            m.tuning.min_rate_hz * 0.5,
            m.tuning.default_hz,
            m.tuning.default_hz,
        ));
        assert!(low.is_err(), "{}", m.id);
        assert!(
            r.results()
                .status()
                .summary
                .contains("needs a sample rate of at least"),
            "{}: {}",
            m.id,
            r.results().status().summary
        );
    }
}

#[test]
fn every_schema_key_is_accepted_at_its_default() {
    for m in descriptors() {
        let schema = (m.settings)();
        let mut r = (m.make_receiver)(&stack_defaults(m)).unwrap();
        for s in schema.settings() {
            set(r.as_mut(), &s.field.key, s.default.clone())
                .unwrap_or_else(|e| panic!("{}: {} at its default: {e}", m.id, s.field.key));
        }
    }
}

#[test]
fn a_bad_value_of_a_number_setting_is_refused() {
    for m in descriptors() {
        let schema = (m.settings)();
        let mut r = (m.make_receiver)(&stack_defaults(m)).unwrap();
        for s in schema.settings() {
            if s.field.key.starts_with("gen_")
                || !matches!(
                    s.field.kind,
                    ValueKind::F64 | ValueKind::I64 | ValueKind::Hz | ValueKind::Duration
                )
            {
                continue;
            }
            assert!(
                set(r.as_mut(), &s.field.key, Value::Text("not a number".into())).is_err(),
                "{}: {} took text",
                m.id,
                s.field.key
            );
        }
    }
}

#[test]
fn every_mode_is_cut_independent() {
    for m in descriptors() {
        let x = signal(m, seconds(m.id).min(2.5), 3);
        let mut seen = Vec::new();
        for piece in [4093usize, 65536] {
            let mut r = receiver(m);
            feed(r.as_mut(), &x, piece);
            seen.push(comparable(r.results()));
        }
        assert_eq!(seen[0], seen[1], "{}", m.id);
    }
}

#[test]
fn every_mode_saves_and_restores_its_state() {
    let mut states: Vec<(&str, StateBlob)> = Vec::new();
    for m in descriptors() {
        let x = signal(m, seconds(m.id), 3);
        let mut r = receiver(m);
        feed(r.as_mut(), &x, 65536);
        let before = comparable(r.results());
        let rows = rows(r.results());
        assert!(rows > 0, "{}: nothing decoded to save", m.id);
        let state = r
            .save_state()
            .unwrap_or_else(|| panic!("{}: no saved state", m.id));

        // A fresh receiver that has run for a while takes it, with the same rows.
        let mut back = receiver(m);
        let quiet = vec![Complex32::default(); (m.tuning.rate_hz * 0.5) as usize];
        feed(back.as_mut(), &quiet, 65536);
        back.restore_state(&state)
            .unwrap_or_else(|e| panic!("{}: {e}", m.id));
        feed(back.as_mut(), &quiet[..16], 16);
        let after = comparable(back.results());
        assert_eq!(
            before["tables"]
                .as_object()
                .map(|t| t.keys().collect::<Vec<_>>()),
            after["tables"]
                .as_object()
                .map(|t| t.keys().collect::<Vec<_>>()),
            "{}",
            m.id
        );
        assert_eq!(super::util::rows(back.results()), rows, "{}", m.id);

        // Garbage is refused, and nothing changes.
        let garbage = StateBlob::new(m.id, 1, b"{".to_vec());
        assert!(back.restore_state(&garbage).is_err(), "{}", m.id);
        assert_eq!(super::util::rows(back.results()), rows, "{}", m.id);

        // Clear forgets what was heard.
        clear(back.as_mut());
        feed(back.as_mut(), &quiet[..16], 16);
        states.push((m.id, state));
    }
    // A state of another stack is refused.
    for m in descriptors() {
        let (other, state) = states.iter().find(|(id, _)| *id != m.id).unwrap();
        let mut r = receiver(m);
        assert!(
            r.restore_state(state).is_err(),
            "{} took the state of {other}",
            m.id
        );
    }
}
