//! Robust Packet: the worked cases of the analyser, the receiver on the test signal at both speed levels, the steps sent against
//! the steps given in the `rpr.burst` events, noise, frequency offsets, and other HF data signals that must not be listed.

use crate::common::*;
use sdrkit_core::{Complex32, Event, InputInfo, Receiver};
use sdrkit_stack_hfdata::rpr::{
    self,
    analyse::{grid_hz, hex},
    gen::{RprGenOptions, RprGenerator},
    rx::RprModulation,
    CARRIERS,
};
use sdrkit_stack_hfdata::serial::gen::test_dial;
use sdrkit_stack_hfdata::typed::{RprBurstEvent, RprFields, RPR_BURST, RPR_BURSTS};

/// A burst as the stack gave it.
struct Got {
    time: f64,
    modulation: RprModulation,
    steps: Vec<u8>,
    offset_hz: f64,
}

fn bursts(events: &[Event]) -> Vec<Got> {
    events
        .iter()
        .filter(|e| e.kind() == "rpr.burst")
        .map(|e| {
            let b: RprBurstEvent = RPR_BURST.read(e).expect("a declared burst event");
            Got {
                time: e.time,
                modulation: b.modulation,
                steps: b.steps.to_vec(),
                offset_hz: b.offset_hz.0,
            }
        })
        .collect()
}

/// The receiver on `secs` of the test signal `o` at `rate` through the impairments `imp` (`gen_*` keys), the radio centre where
/// the generator puts it. Returns the receiver, the events and the generator for the steps it sent.
fn run(o: RprGenOptions, secs: f64, imp: &[(&str, Json)]) -> (Box<dyn Receiver>, Vec<Event>, RprGenerator) {
    run_with(o, secs, imp, &[], false)
}

/// As [`run`], with the stack settings `set`, and with the spectrum turned over (the signal on the lower sideband) when
/// `lower`.
fn run_with(
    o: RprGenOptions,
    secs: f64,
    imp: &[(&str, Json)],
    set: &[(&str, Json)],
    lower: bool,
) -> (Box<dyn Receiver>, Vec<Event>, RprGenerator) {
    let rate = o.rate;
    let g = RprGenerator::new(o.clone());
    let mut v = gen_values();
    put(&mut v, "gen_seed", json!(7.0));
    for (k, x) in imp {
        put(&mut v, k, x.clone());
    }
    let mut chain = sdrkit_components::sim::impaired(Box::new(g), &v).unwrap();
    let mut x = generate(chain.as_mut(), secs);
    if lower {
        for v in x.iter_mut() {
            *v = v.conj();
        }
    }
    let mut rx = receiver(&rpr::RPR, set);
    let dial = 10.1473e6;
    rx.configure(&InputInfo::new(rate, dial - test_dial(rate), dial).recording())
        .unwrap();
    let mut out = feed(rx.as_mut(), &x, (rate / 10.0) as usize);
    out.events.append(&mut end(rx.as_mut()).events);
    let d = rpr::rx::declaration();
    for e in &out.events {
        assert!(declared(&d, e), "{}", e.kind());
        assert_eq!(e.freq_hz, dial);
    }
    (rx, out.events, RprGenerator::new(o))
}

fn noise(snr_db: f64) -> Vec<(&'static str, Json)> {
    vec![("gen_noise", json!("snr")), ("gen_snr_db", json!(snr_db))]
}

/// The step errors of a burst given against the steps that `g` sent in its burst nearest in time: (wrong, compared). The stack
/// gives one step more or fewer at the ends (the ramps), so the best shift of up to 3 steps is taken, and each carrier may be
/// turned by a whole step (the index is relative to the observed axes).
fn errors(got: &Got, g: &RprGenerator) -> (usize, usize) {
    let b = (0..64u64)
        .min_by(|&a, &b| {
            let da = (g.burst_start_s(a) - got.time).abs();
            let db = (g.burst_start_s(b) - got.time).abs();
            da.total_cmp(&db)
        })
        .unwrap();
    let sent = g.steps(b);
    let order = rpr::gen::order_of(g.options().level) as u8;
    let n_got = got.steps.len() / CARRIERS;
    let mut best = (usize::MAX, 0);
    for shift in -3i64..=3 {
        let (mut wrong, mut n) = (0, 0);
        for c in 0..CARRIERS {
            let pairs: Vec<(u8, u8)> = (0..n_got)
                .filter_map(|m| {
                    let j = m as i64 + shift;
                    (j >= 0 && (j as usize) < sent.len())
                        .then(|| (got.steps[m * CARRIERS + c], sent[j as usize][c]))
                })
                .collect();
            let w = (0..order)
                .map(|t| pairs.iter().filter(|(a, s)| (a + t) % order != *s).count())
                .min()
                .unwrap_or(0);
            wrong += w;
            n += pairs.len();
        }
        if n >= 50 * CARRIERS && wrong < best.0 {
            best = (wrong, n);
        }
    }
    best
}

/// The symbol error rate over every burst given, and the share of bursts at the speed level sent.
fn score(got: &[Got], g: &RprGenerator) -> (f64, f64) {
    let want = RprModulation::of_order(rpr::gen::order_of(g.options().level));
    let (mut w, mut n) = (0, 0);
    for b in got {
        let (wb, nb) = errors(b, g);
        w += wb;
        n += nb;
    }
    let right = got.iter().filter(|b| b.modulation == want).count();
    (
        w as f64 / n.max(1) as f64,
        right as f64 / got.len().max(1) as f64,
    )
}

#[test]
fn rpr_carriers_and_worked_hex() {
    let f: Vec<f64> = (0..CARRIERS).map(|k| grid_hz(1500.0, k)).collect();
    assert_eq!(f, [1290.0, 1350.0, 1410.0, 1470.0, 1530.0, 1590.0, 1650.0, 1710.0]);
    // A comb at 1520 Hz (a tuning error of +20 Hz).
    assert_eq!(grid_hz(1520.0, 0), 1310.0);
    assert_eq!(grid_hz(1520.0, 7), 1730.0);
    // D-QPSK steps 0, 90, 180, 270 degrees on every carrier: 16 bits a symbol time.
    let steps: Vec<u8> = (0..4u8).flat_map(|k| [k; CARRIERS]).collect();
    assert_eq!(hex(&steps, 2, 4), "00005555AAAAFFFF");
    // D-BPSK steps of 180 degrees every symbol: all F.
    assert_eq!(hex(&[1u8; 3 * CARRIERS], 1, 3), "FFFFFF");
    assert_eq!(RprModulation::DBpsk.net_rate(), 200);
    assert_eq!(RprModulation::DQpsk.net_rate(), 600);
}

#[test]
fn rpr_both_levels_clean() {
    for level in [1u8, 2] {
        let o = RprGenOptions {
            rate: 24e3,
            level,
            ..Default::default()
        };
        let (rx, events, g) = run(o, 31.0, &[]);
        let got = bursts(&events);
        let (ser, right) = score(&got, &g);
        eprintln!("level {level}: {} bursts, ser {ser:.4}, right {right}", got.len());
        assert!(got.len() >= 9, "level {level}: {} bursts", got.len());
        assert_eq!(right, 1.0, "level {level}");
        assert!(ser < 0.01, "level {level}: {ser}");
        let rx = rx.as_ref();
        assert!((field(rx, RprFields::COMB_CENTRE.key).unwrap() - 1500.0).abs() <= 3.0);
        assert_eq!(field(rx, RprFields::SPEED_LEVEL.key), Some(f64::from(level)));
        assert_eq!(field(rx, RprFields::CARRIER_COUNT.key), Some(8.0));
        let sp = field(rx, RprFields::SPACING.key).unwrap();
        assert!((sp - 60.0).abs() <= 0.5, "spacing {sp}");
        let baud = field(rx, "symbol_rate_hz").unwrap();
        assert!((baud - 50.0).abs() <= 0.3, "symbol rate {baud}");
        assert_eq!(
            field(rx, RprFields::NET_RATE.key),
            Some(if level == 1 { 200.0 } else { 600.0 })
        );
        assert_eq!(
            rx.results().status().lock_state,
            sdrkit_core::LockState::Locked
        );
        assert!(rx.results().status().summary.contains("not decoded"));
        assert_eq!(rows(rx.results(), &RPR_BURSTS).len(), got.len());
    }
}

#[test]
fn rpr_symbol_errors_in_noise() {
    // The limits of the specification, SNR in 2.5 kHz: under 3 % wrong at 20 dB, under 5 % at 8 dB, under 30 % at 3 dB (level
    // 1) or 5 dB (level 2) with at least 2 bursts.
    for (snr, limit, levels, least) in [
        (20.0, 0.03, &[1u8, 2][..], 4),
        (8.0, 0.05, &[1, 2][..], 4),
        (3.0, 0.3, &[1][..], 2),
        (5.0, 0.3, &[2][..], 2),
    ] {
        for &level in levels {
            let o = RprGenOptions {
                rate: 24e3,
                level,
                ..Default::default()
            };
            let (_, events, g) = run(o, 31.0, &noise(snr));
            let got = bursts(&events);
            let (ser, right) = score(&got, &g);
            eprintln!("{snr} dB level {level}: {} bursts, ser {ser:.4}, right {right}", got.len());
            assert!(got.len() >= least, "{snr} dB level {level}: {} bursts", got.len());
            assert!(right >= 0.9, "{snr} dB level {level}: {right}");
            assert!(ser < limit, "{snr} dB level {level}: {ser}");
        }
    }
}

#[test]
fn rpr_follows_an_offset_of_50_hz() {
    // Through the front end at 250 ksps, the dial 20 kHz below the radio centre, at 20 dB.
    for err in [-50.0, 50.0] {
        for level in [1u8, 2] {
            let o = RprGenOptions {
                rate: 250e3,
                level,
                ..Default::default()
            };
            let mut imp = noise(20.0);
            imp.push(("gen_offset_hz", json!(err)));
            let (rx, events, g) = run(o, 13.0, &imp);
            let got = bursts(&events);
            let (ser, right) = score(&got, &g);
            eprintln!("{err:+} Hz level {level}: {} bursts, ser {ser:.4}", got.len());
            assert!(got.len() >= 3, "{err:+} Hz level {level}: {} bursts", got.len());
            assert_eq!(right, 1.0);
            assert!(ser < 0.03, "{err:+} Hz level {level}: {ser}");
            for b in &got {
                assert!((b.offset_hz - err).abs() <= 4.0, "{}", b.offset_hz);
            }
            let off = rx.results().status().offset_hz.unwrap();
            assert!((off - err).abs() <= 4.0, "{off}");
        }
    }
}

/// The bursts that the stack lists on `x` (24 kHz, the dial at the centre of the samples).
fn listed(x: &[Complex32]) -> usize {
    let (rx, out) = run_24k(&rpr::RPR, x, 2400);
    let n = bursts(&out.events).len();
    assert_eq!(rows(rx.results(), &RPR_BURSTS).len(), n);
    n
}

/// `x` switched on and off every `on_s` seconds.
fn keyed(mut x: Vec<Complex32>, on_s: f64, rate: f64) -> Vec<Complex32> {
    for (i, v) in x.iter_mut().enumerate() {
        if ((i as f64 / rate) / on_s) as u64 % 2 == 1 {
            *v = Complex32::new(0.0, 0.0);
        }
    }
    x
}

#[test]
fn rpr_other_signals_are_not_listed() {
    use sdrkit_stack_hfdata::pactorhf::{
        self,
        gen::{PactorHfGenOptions, PactorHfGenerator, Variant},
    };
    let rate = 24e3;
    let with_noise = |mut x: Vec<Complex32>, seed: u64| {
        let mut n = vec![Complex32::new(0.0, 0.0); x.len()];
        sdrkit_dev_test::Awgn::with_sigma(0.01, seed).process(&mut n);
        for (a, b) in x.iter_mut().zip(&n) {
            *a += b;
        }
        x
    };
    // PACTOR-I, its own test link.
    let x = test_signal(&sdrkit_stack_hfdata::pactor1::PACTOR1, rate, &[], 20.0);
    assert_eq!(listed(&with_noise(x, 1)), 0, "PACTOR-I");
    // PACTOR-II and PACTOR-III levels 1 and 2, keyed on and off like bursts.
    for (name, v) in [
        ("PACTOR-II", Variant::P2 { order: 4 }),
        ("PACTOR-III level 1", Variant::P3 { level: 1 }),
        ("PACTOR-III level 2", Variant::P3 { level: 2 }),
    ] {
        let mut g = PactorHfGenerator::new(PactorHfGenOptions {
            rate,
            variant: v,
            centre_hz: 1500.0,
            complex: true,
        });
        let x = keyed(generate(&mut g, 20.0), 1.5, rate);
        assert_eq!(listed(&with_noise(x, 2)), 0, "{name}");
    }
    let _ = pactorhf::PACTOR2.id;
    // Noise alone, a tone in noise, and eight unmodulated carriers on all the time.
    let n = (rate * 20.0) as usize;
    assert_eq!(listed(&with_noise(vec![Complex32::new(0.0, 0.0); n], 3)), 0, "noise");
    let tone = |f: f64, i: usize| {
        Complex32::from_polar(0.3, (std::f64::consts::TAU * f * i as f64 / rate) as f32)
    };
    let x: Vec<Complex32> = (0..n).map(|i| tone(1500.0, i)).collect();
    assert_eq!(listed(&with_noise(x, 4)), 0, "tone");
    let x: Vec<Complex32> = (0..n)
        .map(|i| (0..CARRIERS).map(|k| tone(grid_hz(1500.0, k), i)).sum())
        .collect();
    assert_eq!(listed(&with_noise(x, 5)), 0, "eight carriers");
    // Too little audio.
    let x = test_signal(&rpr::RPR, rate, &[], 2.0);
    assert_eq!(listed(&x), 0, "two seconds");
}

#[test]
fn rpr_level_2_from_the_settings() {
    // The generator of the descriptor with the speed level setting, through the receiver with its default settings.
    let x = test_signal(&rpr::RPR, 24e3, &[("gen_speed", json!("level2"))], 10.0);
    let (rx, out) = run_24k(&rpr::RPR, &x, 4801);
    let got = bursts(&out.events);
    assert!(!got.is_empty());
    assert!(got.iter().all(|b| b.modulation == RprModulation::DQpsk));
    assert_eq!(field(rx.as_ref(), RprFields::SPEED_LEVEL.key), Some(2.0));
}

#[test]
fn rpr_fading_at_20_db() {
    // Flat Rayleigh fading of 0.5 Hz, and the HF moderate channel of ITU-R F.520 (two paths 1 ms apart, 0.5 Hz): at least 3
    // bursts, 80 % of them at the speed level sent.
    for (name, fading) in [
        ("rayleigh", vec![("gen_fading", json!("rayleigh")), ("gen_doppler_hz", json!(0.5))]),
        ("hf_moderate", vec![("gen_fading", json!("hf_moderate"))]),
    ] {
        for level in [1u8, 2] {
            let o = RprGenOptions {
                rate: 24e3,
                level,
                ..Default::default()
            };
            let mut imp = noise(20.0);
            imp.extend(fading.iter().cloned());
            let (_, events, g) = run(o, 31.0, &imp);
            let got = bursts(&events);
            let (ser, right) = score(&got, &g);
            eprintln!("{name} level {level}: {} bursts, ser {ser:.4}, right {right}", got.len());
            assert!(got.len() >= 3, "{name} level {level}: {} bursts", got.len());
            assert!(right >= 0.8, "{name} level {level}: {right}");
        }
    }
}

#[test]
fn rpr_lower_sideband_and_a_moved_centre() {
    // The signal on the lower sideband, 300 Hz higher in the audio, with the stack set to match: no offset.
    let o = RprGenOptions {
        rate: 24e3,
        level: 2,
        ..Default::default()
    };
    let (rx, events, g) = run_with(
        o,
        13.0,
        &[("gen_offset_hz", json!(300.0))],
        &[("sideband", json!("lower")), ("audio_centre_hz", json!(1800.0))],
        true,
    );
    let got = bursts(&events);
    let (ser, right) = score(&got, &g);
    assert!(got.len() >= 3, "{} bursts", got.len());
    assert_eq!(right, 1.0);
    assert!(ser < 0.01, "{ser}");
    let c = field(rx.as_ref(), RprFields::COMB_CENTRE.key).unwrap();
    assert!((c - 1800.0).abs() <= 3.0, "{c}");
    assert!(got.iter().all(|b| b.offset_hz.abs() <= 3.0));
    // The same signal with the default settings: the upper sideband holds no comb.
    let o = RprGenOptions {
        rate: 24e3,
        level: 2,
        ..Default::default()
    };
    let (_, events, _) = run_with(o, 13.0, &[("gen_offset_hz", json!(300.0))], &[], true);
    assert!(bursts(&events).is_empty());
}
