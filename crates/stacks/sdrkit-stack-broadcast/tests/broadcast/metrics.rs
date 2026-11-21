//! A measurement, not a check: each mode's own test signal through the framework's impairments and its receiver, printing the
//! time to lock, the SNR the receiver reports and its count of good frames. Run with `--ignored --nocapture`.

use super::support::*;
use sdrkit_core::{GenConfig, InputInfo, LockState, Output, StackConfig, StackDescriptor};
use serde_json::json;

fn measure(
    d: &StackDescriptor,
    rate: f64,
    tuned: f64,
    seconds: f64,
    key: &str,
    extra: &[(&str, &str)],
) {
    let extra: Vec<Set> = extra.iter().map(|(k, v)| (*k, json!(*v))).collect();
    let values = values(&(d.settings)(), &extra);
    let cfg = GenConfig::new(rate, values.clone()).with_seed(1);
    let g = (d.make_generator.expect("a generator"))(&cfg).unwrap();
    let mut s = snr(20.0).to_vec();
    s.push(("gen_offset_hz", json!(37.0)));
    s.push(("gen_seed", json!(4)));
    let x = impair(g, seconds, &s);
    let mut rx = (d.make_receiver)(&StackConfig::new(values)).unwrap();
    rx.configure(&InputInfo::new(rate, tuned, tuned).recording())
        .unwrap();
    let mut out = Output::default();
    let piece = (rate / 20.0) as usize;
    let mut lock = None;
    for (i, c) in x.chunks(piece).enumerate() {
        rx.feed(c, &mut out);
        out.clear();
        if lock.is_none() && rx.results().status().lock_state == LockState::Locked {
            lock = Some((i + 1) as f64 * piece as f64 / rate);
        }
    }
    let r = rx.results();
    println!(
        "METRIC {} {extra:?}: lock {} s, snr {} dB, {key} {}",
        d.id,
        lock.map_or("never".into(), |t| format!("{t:.2}")),
        r.status().snr_db.map_or("-".into(), |v| format!("{v:.1}")),
        num(r, key)
    );
}

#[test]
#[ignore]
fn every_mode_on_its_test_signal() {
    use sdrkit_stack_broadcast as b;
    measure(&b::DRM, 48e3, 9.5e6, 12.0, "fac_ok", &[]);
    let v = format!(
        "{}/../../../tests/vectors/broadcast",
        env!("CARGO_MANIFEST_DIR")
    );
    let ham = format!("{v}/hamdrm/self-check-format.json");
    let cdr = format!("{v}/cdr/self-check-tables.json");
    measure(
        &b::HAMDRM,
        48e3,
        14.233e6,
        20.0,
        "segments_received",
        &[("hamdrm_format", &ham)],
    );
    measure(&b::HDRADIO, b::hd::l1::FM_RATE, 98.5e6, 8.0, "p1_ok", &[]);
    measure(
        &b::HDRADIO,
        200e3,
        1.03e6,
        12.0,
        "p1_ok",
        &[("gen_waveform", "ma1")],
    );
    measure(&b::CDR, 816e3, 106.1e6, 3.0, "subframes", &[]);
    measure(
        &b::CDR,
        816e3,
        106.1e6,
        3.0,
        "control_ok",
        &[("cdr_tables", &cdr)],
    );
    measure(&b::CQUAM, 250e3, 999e3, 5.0, "pilot_checks_ok", &[]);
}
