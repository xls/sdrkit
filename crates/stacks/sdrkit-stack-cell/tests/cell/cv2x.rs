//! C-V2X: the sidelink structure detector on its own test signal, the LTE uplink shape, plain OFDM and noise.

use crate::common::*;
use sdrkit_core::{Complex32, GenConfig, InputInfo, Output, Owner, Receiver};
use sdrkit_dev_test::Awgn;
use sdrkit_stack_cell::cv2x::gen::Cv2xGenerator;
use sdrkit_stack_cell::typed::Cv2xFields;
use sdrkit_stack_cell::CV2X;
use serde_json::Value as J;

const RATE: f64 = 15.36e6;

/// What the receiver published after `x`.
struct Seen {
    detected: bool,
    offset: Option<f64>,
    slot: Option<i64>,
    busy: Option<f64>,
    guard: Option<bool>,
}

fn receive(x: &[Complex32]) -> Seen {
    receive_at(x, RATE, &[])
}

fn receive_at(x: &[Complex32], rate: f64, extra: &[(&str, J)]) -> Seen {
    let mut rx = stack_graph(&CV2X, extra);
    rx.configure(&InputInfo::new(rate, 5915e6, 5915e6).recording())
        .expect("configure");
    let mut out = Output::default();
    for c in x.chunks(65_536) {
        rx.feed(c, &mut out);
        out.clear();
    }
    rx.finish(&mut out);
    let r = rx.results();
    Seen {
        detected: r.fields().at(&Cv2xFields::DETECTED).unwrap_or(false),
        offset: value(r, "offset_hz"),
        slot: r.fields().at(&Cv2xFields::SLOT_TIMING),
        busy: r.fields().at(&Cv2xFields::BUSY_SHARE),
        guard: r.fields().at(&Cv2xFields::GUARD_SEEN),
    }
}

fn sidelink(busy: f64, guard: bool, offset: f64, snr_db: f64, seconds: f64) -> Vec<Complex32> {
    let s = settings(
        &CV2X,
        &[("gen_busy", J::from(busy)), ("gen_guard", J::from(guard))],
    );
    let gen = Cv2xGenerator::from_config(&GenConfig::new(RATE, s).with_seed(5)).expect("a generator");
    let mut keys = snr(snr_db);
    keys.push(("gen_offset_hz", J::from(offset)));
    impaired(gen, &keys, seconds)
}

#[test]
fn case_1_busy_channel_clean() {
    let r = receive(&sidelink(100.0, true, 0.0, 20.0, 0.6));
    assert!(r.detected);
    assert!(r.offset.unwrap().abs() < 200.0, "{:?}", r.offset);
    let t = r.slot.unwrap();
    assert!(t <= 2 || t >= 7680 - 2, "{t}");
    assert_eq!(r.guard, Some(true));
}

#[test]
fn case_2_offset_at_5_db() {
    let r = receive(&sidelink(100.0, true, 3000.0, 5.0, 0.6));
    assert!(r.detected);
    assert!((r.offset.unwrap() - 3000.0).abs() < 300.0, "{:?}", r.offset);
}

#[test]
fn case_3_busy_share() {
    let r = receive(&sidelink(20.0, true, -2000.0, 10.0, 1.5));
    assert!(r.detected);
    let b = r.busy.unwrap();
    assert!((b - 20.0).abs() <= 5.0, "{b}");
}

#[test]
fn case_4_seldom_busy() {
    let r = receive(&sidelink(5.0, true, 0.0, 10.0, 2.0));
    assert!(r.detected);
}

#[test]
fn case_5_uplink_shape_has_no_guard() {
    let r = receive(&sidelink(100.0, false, 0.0, 20.0, 0.6));
    assert!(r.detected);
    assert_ne!(r.guard, Some(true));
}

#[test]
fn case_6_plain_ofdm_is_not_sidelink() {
    // N = 1024 with a 72-sample prefix on every symbol (period 1096), continuous.
    let n = 1024;
    let mut s = 9u64;
    let mut x = Vec::new();
    while x.len() < (0.6 * RATE) as usize {
        let mut sym = vec![Complex32::default(); n];
        for k in (1..300).chain(n - 300..n) {
            s ^= s << 13;
            s ^= s >> 7;
            s ^= s << 17;
            sym[k] = Complex32::new(if s & 1 == 0 { 1.0 } else { -1.0 }, if s & 2 == 0 { 1.0 } else { -1.0 });
        }
        let mut fft = sdrkit_algo_dsp::fft::Fft::inverse(n);
        fft.process(&mut sym);
        sym.iter_mut().for_each(|v| *v *= 1.0 / 600f32.sqrt());
        x.extend_from_slice(&sym[n - 72..]);
        x.extend_from_slice(&sym);
    }
    let noise = Awgn::with_sigma(0.1, 4).noise(x.len());
    x.iter_mut().zip(&noise).for_each(|(a, b)| *a += b);
    assert!(!receive(&x).detected);
}

#[test]
fn case_7_noise() {
    let x = Awgn::with_sigma(0.1, 2).noise((0.6 * RATE) as usize);
    assert!(!receive(&x).detected);
}

#[test]
fn a_20_mhz_channel() {
    let rate = 30.72e6;
    let s = settings(&CV2X, &[("channel_mhz", J::from("20")), ("gen_busy", J::from(100.0)), ("gen_resource_blocks", J::from(100))]);
    let gen = Cv2xGenerator::from_config(&GenConfig::new(rate, s).with_seed(3)).expect("a generator");
    let mut keys = snr(15.0);
    keys.push(("gen_offset_hz", J::from(1000.0)));
    let x = impaired(gen, &keys, 0.4);
    let r = receive_at(&x, rate, &[("channel_mhz", J::from("20"))]);
    assert!(r.detected);
    assert!((r.offset.unwrap() - 1000.0).abs() < 200.0, "{:?}", r.offset);
    assert_eq!(r.guard, Some(true));
}
