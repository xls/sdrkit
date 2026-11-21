//! `qrss`: QRSS and DFCW texts at two element lengths, DFCW with the dash below, a tone set by hand at two SNRs, a drifting tone,
//! and nothing from noise.

use sdrkit_algo_modem::slowmorse::Keying;
use sdrkit_core::{Complex32, InputInfo, Output, StackConfig, Value, Values};
use sdrkit_stack_hfkbd::decoder::Modulator;
use sdrkit_stack_hfkbd::qrss::{self, QrssModulator};
use sdrkit_stack_hfkbd::QRSS;

/// `text` in `keying` at 8 kHz complex: 5 s of silence, the text once, a quiet tail of `tail` s; noise of `snr` dB in 2500 Hz
/// against the carrier; the tone drifting `drift` Hz/s.
#[allow(clippy::too_many_arguments)]
fn signal(
    text: &str,
    keying: Keying,
    t: f64,
    centre: f64,
    shift: f64,
    snr: f64,
    drift: f64,
    seed: u64,
) -> Vec<Complex32> {
    let n_el = qrss::keying_of(text, keying).len();
    let n = ((5.0 + n_el as f64 * t + 4.0 * t) * 8000.0) as usize;
    let mut m = QrssModulator::new(8000.0, keying, text, centre, t, shift);
    let mut x = vec![Complex32::default(); n];
    m.fill(&mut x);
    let keyed = ((5.0 + n_el as f64 * t) * 8000.0) as usize;
    x[keyed.min(n)..].fill(Complex32::default());
    if drift != 0.0 {
        let mut ph = 0.0f64;
        for (i, s) in x.iter_mut().enumerate() {
            ph += std::f64::consts::TAU * drift * i as f64 / 8000.0 / 8000.0;
            *s *= Complex32::new(ph.cos() as f32, ph.sin() as f32);
        }
    }
    let level = sdrkit_algo_sim::noise::NoiseLevel::SnrDb {
        snr_db: snr,
        bandwidth_hz: 2500.0,
    };
    sdrkit_algo_sim::noise::Awgn::complex(level, 1.0, 8000.0, seed)
        .unwrap()
        .process(&mut x);
    x
}

/// The text, the element length and the tone read by the stack's receiver from 8 kHz complex samples (dial at 0 Hz).
fn receive(x: &[Complex32], extra: &[(&str, Value)], submode: Option<&str>) -> (String, f64, f64) {
    let schema = (QRSS.settings)();
    let mut s = Values::defaults(&schema);
    for (k, v) in extra {
        s.set(schema.id(k).unwrap(), v).unwrap();
    }
    let mut cfg = StackConfig::new(s);
    if let Some(sub) = submode {
        cfg = cfg.with_submode(sub);
    }
    let mut rx = (QRSS.make_receiver)(&cfg).unwrap();
    rx.configure(&InputInfo::new(12000.0, 10.14e6, 10.14e6).recording())
        .unwrap();
    // The receiver needs at least 12 ksps.
    let mut up = Vec::new();
    sdrkit_algo_dsp::resample::Resampler::new(8000.0, 12000.0)
        .unwrap()
        .process(x, &mut up);
    let mut out = Output::default();
    for c in up.chunks(6000) {
        rx.feed(c, &mut out);
        out.clear();
    }
    rx.finish(&mut out);
    let f = rx.results().fields();
    (
        f.get(&sdrkit_core::publish::keys::TEXT)
            .unwrap_or("")
            .to_owned(),
        f.at(&sdrkit_stack_hfkbd::typed::QrssFields::ELEMENT_S)
            .unwrap_or(0.0),
        f.at(&sdrkit_stack_hfkbd::typed::QrssFields::CENTRE_HZ)
            .unwrap_or(0.0),
    )
}

#[test]
fn texts_at_two_element_lengths_in_both_keyings() {
    for t in [3.0, 6.0] {
        for (keying, sub) in [(Keying::Qrss, None), (Keying::Dfcw, Some("dfcw"))] {
            let shift = 2.0 / (t / 3.0f64).max(1.0) + 0.5;
            let x = signal("CQ DE LY2ABC", keying, t, 1234.5, shift, 6.0, 0.0, t as u64);
            let (text, el, centre) = receive(&x, &[], sub);
            assert_eq!(text, "CQ DE LY2ABC", "{keying:?} {t}");
            assert!((el - t).abs() <= 0.12 * t + 0.1, "{keying:?} {t}: {el}");
            assert!((centre - 1234.5).abs() <= 3.5, "{keying:?} {t}: {centre}");
        }
    }
}

#[test]
fn dfcw_with_the_dash_below() {
    let x = signal("TEST", Keying::Dfcw, 6.0, 1000.0, -1.5, 10.0, 0.0, 5);
    let (text, el, _) = receive(&x, &[("dash_below", Value::Bool(true))], Some("dfcw"));
    assert_eq!(text, "TEST", "elements of {el} s");
}

#[test]
fn a_tone_set_by_hand() {
    for snr in [10.0, 0.0] {
        let x = signal("LY2ABC", Keying::Qrss, 3.0, 1500.0, 0.0, snr, 0.0, 6);
        let (text, _, _) = receive(&x, &[("centre_hz", Value::Hz(1500.0))], None);
        assert_eq!(text, "LY2ABC", "{snr} dB");
    }
    for snr in [-6.0, -12.0] {
        let x = signal("LY2ABC", Keying::Qrss, 3.0, 1500.0, 0.0, snr, 0.0, 7);
        let (text, _, _) = receive(&x, &[("centre_hz", Value::Hz(1500.0))], None);
        eprintln!("QRSS at {snr} dB: {text:?}");
    }
}

#[test]
fn a_drifting_tone() {
    let x = signal("CQ", Keying::Qrss, 3.0, 1000.0, 0.0, 6.0, 0.02, 8);
    let (text, _, _) = receive(&x, &[], None);
    assert_eq!(text, "CQ");
}

#[test]
fn noise_gives_no_text() {
    let mut x = vec![Complex32::default(); 120 * 8000];
    sdrkit_algo_sim::noise::Awgn::with_power(1.0, 9)
        .unwrap()
        .process(&mut x);
    let (text, _, _) = receive(&x, &[], None);
    assert_eq!(text, "");
}
