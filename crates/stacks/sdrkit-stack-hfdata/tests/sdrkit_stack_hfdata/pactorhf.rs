//! PACTOR-II/III/IV: the analyser on generated audio, the speed of the PACTOR-IV line search, the recordings and the receivers.

use crate::common::*;
use sdrkit_core::{Complex32, InputInfo, Receiver, Value, Values};
use sdrkit_stack_hfdata::pactorhf::{
    self,
    analyse::{analyse, power_line, Analysis},
    gen::{PactorHfGenOptions, PactorHfGenerator, Variant},
    rx::{self as r},
};
use sdrkit_stack_hfdata::typed::{PactorhfFields, PACTORHF_CARRIERS, PACTORHF_EVENTS};

/// The standard impairments `imp` (gen_* keys) as values.
fn imp_values(imp: &[(&str, Json)]) -> Values {
    let mut v = gen_values();
    put(&mut v, "gen_seed", json!(7.0));
    for (k, x) in imp {
        put(&mut v, k, x.clone());
    }
    v
}

/// The noise keys for an SNR of the real audio in 2.5 kHz. The real part keeps half the signal power and half the noise power, but
/// the noise of both sides of 0 Hz falls on the audio band, so the complex SNR is 3 dB more.
fn snr(db: f64) -> [(&'static str, Json); 2] {
    [
        ("gen_noise", json!("snr")),
        ("gen_snr_db", json!(db + 3.01)),
    ]
}

/// `secs` of 8 kHz real audio of `o` through the standard impairments `imp`.
fn audio(o: PactorHfGenOptions, secs: f64, imp: &[(&str, Json)]) -> (Vec<f32>, PactorHfGenerator) {
    let o = PactorHfGenOptions {
        rate: 8000.0,
        complex: true,
        ..o
    };
    let g = PactorHfGenerator::new(o.clone());
    let mut chain = sdrkit_components::sim::impaired(Box::new(g), &imp_values(imp)).unwrap();
    let x = generate(chain.as_mut(), secs);
    (x.iter().map(|v| v.re).collect(), PactorHfGenerator::new(o))
}

fn clean(o: PactorHfGenOptions, secs: f64) -> (Vec<f32>, PactorHfGenerator) {
    audio(o, secs, &[])
}

fn summary(a: &Analysis) -> String {
    format!(
        "found {} v{} n{} lvl {} c {:.1} sp {:.1} baud {:.3} ppm {:.0} line {:.1} orders {:?} frame {} {} {} len {} cons {} note {}",
        a.found, a.variant, a.carrier_count, a.level, a.centre_hz, a.spacing_hz, a.baud, a.clock_ppm, a.clock_line_db,
        a.carriers.iter().map(|c| c.order).collect::<Vec<_>>(), a.frame.found, a.frame.period_symbols, a.frame.const_hex, a.frame.const_len, a.constellation, a.note
    )
}

fn agreement(got: &[u8], sent: &[u32], order: u32) -> f64 {
    let mut best: f64 = 0.0;
    for shift in -3i64..=3 {
        for c in 0..order {
            let mut ok = 0;
            let mut n = 0;
            for (k, &g) in got.iter().enumerate() {
                let j = k as i64 + shift;
                if j < 1 || j as usize >= sent.len() {
                    continue;
                }
                n += 1;
                if (g as u32 + c) % order == sent[j as usize] {
                    ok += 1;
                }
            }
            if n > 0 {
                best = best.max(ok as f64 / n as f64);
            }
        }
    }
    best
}

#[test]
fn a1_pactor2_clean() {
    for order in [2u32, 4] {
        let (x, g) = clean(
            PactorHfGenOptions {
                variant: Variant::P2 { order },
                ..Default::default()
            },
            30.0,
        );
        let a = analyse(&x, 2);
        eprintln!("{}", summary(&a));
        for (ci, c) in a.carriers.iter().enumerate() {
            let ag = agreement(&c.symbols, &g.sent(ci, 3100), order);
            assert!(ag > 0.995, "carrier {ci} agreement {ag}");
        }
        assert!(a.found && a.variant == 2 && a.carrier_count == 2);
        assert!((a.spacing_hz - 200.0).abs() <= 3.0);
        assert!((a.centre_hz - 885.0).abs() <= 5.0);
        assert!((a.baud - 100.0).abs() <= 0.05);
        for c in &a.carriers {
            assert_eq!(c.order, order);
        }
        assert_eq!(a.frame.period_symbols, 362);
        assert!(
            (38..=42).contains(&a.frame.const_len),
            "{}",
            a.frame.const_len
        );
    }
}

#[test]
fn a2_pactor3_levels() {
    for level in [2u8, 4, 6] {
        let count = pactorhf::analyse::count_of(level);
        let (x, g) = clean(
            PactorHfGenOptions {
                variant: Variant::P3 { level },
                ..Default::default()
            },
            30.0,
        );
        let a = analyse(&x, 3);
        eprintln!("level {level}: {}", summary(&a));
        assert_eq!(a.variant, 3);
        assert_eq!(a.carrier_count, count);
        assert_eq!(a.level, level);
        assert!((a.spacing_hz - 120.0).abs() <= 2.0);
        assert!((a.centre_hz - 1500.0).abs() <= 4.0);
        assert_eq!(a.frame.period_symbols, 362);
        let order = if level <= 3 { 2 } else { 4 };
        let worst = a
            .carriers
            .iter()
            .enumerate()
            .map(|(ci, c)| agreement(&c.symbols, &g.sent(ci, 3100), order))
            .fold(1.0, f64::min);
        assert!(worst > 0.99, "level {level}: {worst}");
    }
}

#[test]
fn a3_impairments() {
    // (signal, SNR of the audio, tuning error, sender clock in ppm, flat fading Doppler).
    let p2 = PactorHfGenOptions::default();
    let p3 = PactorHfGenOptions {
        variant: Variant::P3 { level: 4 },
        ..Default::default()
    };
    let cases = [
        (p2.clone(), Some(12.0), 0.0, 0.0, None),
        (p2.clone(), Some(15.0), 12.0, 0.0, None),
        (p2.clone(), None, 0.0, 200.0, None),
        (p2, Some(25.0), 0.0, 0.0, Some(0.5)),
        (p3, Some(20.0), 7.0, 100.0, None),
    ];
    for (o, s, err, ppm, fade) in cases {
        let mut imp: Vec<(&str, Json)> = vec![
            ("gen_offset_hz", json!(err)),
            // The receiver clock runs slow by the sender's clock error.
            ("gen_clock_ppm", json!(-ppm)),
        ];
        if let Some(s) = s {
            imp.extend(snr(s));
        }
        if let Some(d) = fade {
            imp.push(("gen_fading", json!("rayleigh")));
            imp.push(("gen_doppler_hz", json!(d)));
        }
        let (x, _) = audio(o, 30.0, &imp);
        let a = analyse(&x, 0);
        eprintln!("{}", summary(&a));
        assert_eq!(a.frame.period_symbols, 362, "{}", summary(&a));
        assert!((a.clock_ppm - ppm).abs() <= 40.0, "{}", summary(&a));
    }
}

#[test]
fn a4_pactor4() {
    for &(c, s, err, ppm) in &[
        (2u32, 25.0, 0.0, 0.0),
        (4, 25.0, 0.0, 0.0),
        (8, 28.0, 0.0, 0.0),
        (16, 30.0, 0.0, 0.0),
        (4, 22.0, 30.0, 0.0),
        (2, 25.0, -40.0, 100.0),
        (8, 30.0, 20.0, 50.0),
    ] {
        let mut imp = vec![
            ("gen_offset_hz", json!(err)),
            ("gen_clock_ppm", json!(-ppm)),
        ];
        imp.extend(snr(s));
        let (x, _) = audio(
            PactorHfGenOptions {
                variant: Variant::P4 { constellation: c },
                ..Default::default()
            },
            12.0,
            &imp,
        );
        let a = analyse(&x, 4);
        eprintln!("{c} {err} {ppm}: {}", summary(&a));
        assert_eq!(a.variant, 4);
        assert!((a.baud - 1800.0).abs() <= 1.0);
        assert!((a.centre_hz - 1500.0 - err).abs() <= 12.0);
        assert_eq!(a.constellation as u32, c);
    }
}

#[test]
fn a5_nothing() {
    let mut rng = sdrkit_algo_sim::rng::Rng::new(3);
    let noise: Vec<f32> = (0..8000 * 20)
        .map(|_| (rng.normal() * 0.1) as f32)
        .collect();
    for asked in [2, 3, 4] {
        let a = analyse(&noise, asked);
        eprintln!("noise {asked}: {}", summary(&a));
        assert!(!a.found);
    }
    let tone: Vec<f32> = noise
        .iter()
        .enumerate()
        .map(|(i, v)| v + 0.3 * (std::f64::consts::TAU * 1000.0 * i as f64 / 8000.0).cos() as f32)
        .collect();
    assert!(!analyse(&tone, 2).found);
    let (short, _) = clean(PactorHfGenOptions::default(), 1.0);
    assert!(!analyse(&short, 2).found);
}

/// The m-th power line search of the PACTOR-IV analysis is one FFT per power, not a direct sum per trial frequency: the three
/// searches of an analysis (18 000 symbols) take well under 100 ms in the test profile (a direct sum of 401 trial frequencies needed
/// about 350 ms). The known answer is the frequency of QPSK symbols turned 0.004 cycles per symbol.
#[test]
fn a7_pactor4_line_search_is_fast() {
    let mut rng = sdrkit_algo_sim::rng::Rng::new(5);
    let f = 0.004;
    let z: Vec<Complex32> = (0..18_000)
        .map(|k| {
            let p = std::f64::consts::TAU * (f * k as f64 + 0.125 + 0.25 * rng.below(4) as f64);
            Complex32::new(p.cos() as f32, p.sin() as f32)
        })
        .collect();
    let t = std::time::Instant::now();
    let lines: Vec<(f64, f64)> = [2u32, 4, 8]
        .iter()
        .map(|&m| power_line(&z, m, (m as f64 * 40.0 / 1800.0).min(0.5)))
        .collect();
    let took = t.elapsed();
    eprintln!("power lines {lines:?} in {took:?}");
    assert!(lines[1].0 > 0.95, "{lines:?}");
    assert!((lines[1].1 / 4.0 - f).abs() < 1e-4, "{lines:?}");
    assert!(lines[0].0 < 0.1, "{lines:?}");
    assert!(took.as_secs_f64() < 0.1, "the line search took {took:?}");
}

fn vec_run(name: &str, desc: &sdrkit_core::StackDescriptor) -> Box<dyn Receiver> {
    let rec = vector(name);
    let mut rx = receiver(desc, &[]);
    run_vector(rx.as_mut(), &rec);
    rx
}

#[test]
fn r8_recordings() {
    let rx = vec_run("pactor2_24k", &pactorhf::PACTOR2);
    let rx = rx.as_ref();
    let f = |k: &str| field(rx, k).unwrap_or(f64::NAN);
    eprintln!(
        "p2 {} {:?}",
        rx.results().status().summary,
        rx.results().fields()
    );
    assert_eq!(f(PactorhfFields::VARIANT.key), 2.0);
    assert_eq!(f(PactorhfFields::CARRIER_COUNT.key), 2.0);
    assert!((f("symbol_rate_hz") - 100.01).abs() <= 0.05);
    assert!((60.0..=140.0).contains(&f("clock_ppm")));
    assert!((892.0..=904.0).contains(&f(PactorhfFields::CENTRE.key)));
    assert_eq!(f(PactorhfFields::FRAME_SYMBOLS.key), 362.0);
    assert_eq!(
        rx.results().status().lock_state,
        sdrkit_core::LockState::Locked
    );
    assert_eq!(
        rx.results().status().summary,
        "PACTOR-II: 2 carriers, frame 3.62 s"
    );
    assert_eq!(rows(rx.results(), &PACTORHF_CARRIERS).len(), 2);

    let rx = vec_run("pactor3_24k", &pactorhf::PACTOR3);
    let rx = rx.as_ref();
    let f = |k: &str| field(rx, k).unwrap_or(f64::NAN);
    eprintln!("p3 {}", rx.results().status().summary);
    assert_eq!(f(PactorhfFields::VARIANT.key), 3.0);
    assert_eq!(f(PactorhfFields::CARRIER_COUNT.key), 14.0);
    assert_eq!(f(PactorhfFields::SPEED_LEVEL.key), 4.0);
    assert!((f(PactorhfFields::CENTRE.key) - 1508.0).abs() <= 4.0);
    assert_eq!(f(PactorhfFields::FRAME_SYMBOLS.key), 362.0);
    assert_eq!(
        rx.results().status().summary,
        "PACTOR-III: 14 carriers, frame 3.62 s"
    );
    let rx = vec_run("pactor3_24k", &pactorhf::PACTOR2);
    assert_eq!(field(rx.as_ref(), PactorhfFields::VARIANT.key), Some(3.0));
    assert_eq!(
        rx.results().status().summary,
        "PACTOR-II: this looks like PACTOR-III"
    );
    assert_eq!(
        value(rx.as_ref(), PactorhfFields::OTHER_VARIANT.key),
        Value::Bool(true)
    );

    let rx = vec_run("pactor4_24k", &pactorhf::PACTOR4);
    let rx = rx.as_ref();
    let f = |k: &str| field(rx, k).unwrap_or(f64::NAN);
    eprintln!(
        "p4 {} {:?}",
        rx.results().status().summary,
        value(rx, PactorhfFields::NOTE.key)
    );
    assert_eq!(f(PactorhfFields::VARIANT.key), 4.0);
    assert_eq!(f(PactorhfFields::CONSTELLATION.key), 4.0);
    assert!((f("symbol_rate_hz") - 1800.0).abs() <= 1.0);
    assert!((f(PactorhfFields::CENTRE.key) - 1520.0).abs() <= 12.0);
    assert!(f(PactorhfFields::CLOCK_LINE_DB.key) >= 12.0);
    assert_eq!(
        rx.results().status().summary,
        "PACTOR-IV: 1800 baud single carrier, constellation estimated"
    );
}

/// The receiver on `secs` of the clean signal of `o` at 250 ksps, the dial 20 kHz below the radio centre and `err` Hz off, with
/// `snr_db` in 2.5 kHz when given. Also returns the events sent.
fn rx_run(
    desc: &sdrkit_core::StackDescriptor,
    o: PactorHfGenOptions,
    secs: f64,
    err: f64,
    snr_db: Option<f64>,
) -> (Box<dyn Receiver>, Vec<sdrkit_core::Event>) {
    let rate = 250e3;
    let g = PactorHfGenerator::new(PactorHfGenOptions {
        rate,
        complex: true,
        ..o
    });
    let mut imp = vec![("gen_offset_hz", json!(-20e3 + err))];
    if let Some(s) = snr_db {
        imp.push(("gen_noise", json!("snr")));
        imp.push(("gen_snr_db", json!(s)));
    }
    let mut chain = sdrkit_components::sim::impaired(Box::new(g), &imp_values(&imp)).unwrap();
    let x = generate(chain.as_mut(), secs);
    let mut rx = receiver(desc, &[]);
    rx.configure(&InputInfo::new(rate, 14.072e6 + 20e3, 14.072e6).recording())
        .unwrap();
    let out = feed(rx.as_mut(), &x, 25_000);
    (rx, out.events)
}

#[test]
fn r1_r2_receiver() {
    let (rx, events) = rx_run(
        &pactorhf::PACTOR2,
        PactorHfGenOptions::default(),
        24.0,
        0.0,
        Some(20.0),
    );
    let rx = rx.as_ref();
    assert_eq!(field(rx, PactorhfFields::CARRIER_COUNT.key), Some(2.0));
    assert_eq!(
        rx.results().status().lock_state,
        sdrkit_core::LockState::Locked
    );
    assert_eq!(
        value(rx, PactorhfFields::FRAME_FOUND.key),
        Value::Bool(true)
    );
    assert_eq!(field(rx, PactorhfFields::FRAME_SYMBOLS.key), Some(362.0));
    assert!((field(rx, "symbol_rate_hz").unwrap() - 100.0).abs() <= 0.1);
    for k in [PactorhfFields::SYMBOLS_1.key, PactorhfFields::SYMBOLS_2.key] {
        assert!(!field_text(rx, k).is_empty(), "{k}");
    }
    assert!(field_text(rx, PactorhfFields::SYMBOLS_3.key).is_empty());
    assert!(rows(rx.results(), &PACTORHF_EVENTS)
        .iter()
        .any(|e| text(e, "text").starts_with("PACTOR-II signal")));
    let d = r::declaration();
    assert!(!events.is_empty());
    for e in &events {
        assert!(declared(&d, e));
        assert_eq!(e.freq_hz, 14.072e6);
    }

    let (rx, _) = rx_run(
        &pactorhf::PACTOR2,
        PactorHfGenOptions::default(),
        24.0,
        40.0,
        None,
    );
    let off = rx.results().status().offset_hz.unwrap();
    assert!((off - 40.0).abs() <= 6.0, "{off}");
    assert_eq!(
        field(rx.as_ref(), PactorhfFields::FRAME_SYMBOLS.key),
        Some(362.0)
    );
}

#[test]
fn r5_noise_in_each_mode() {
    let rate = 24e3;
    let mut x = vec![Complex32::new(0.0, 0.0); (rate * 12.0) as usize];
    sdrkit_dev_test::Awgn::with_sigma(0.05, 5).process(&mut x);
    for desc in [&pactorhf::PACTOR2, &pactorhf::PACTOR3, &pactorhf::PACTOR4] {
        let mut rx = receiver(desc, &[]);
        rx.configure(&InputInfo::new(rate, 14.072e6, 14.072e6).recording())
            .unwrap();
        feed(rx.as_mut(), &x, 2400);
        let rx = rx.as_ref();
        assert_eq!(field(rx, PactorhfFields::VARIANT.key), Some(0.0));
        assert!(field(rx, PactorhfFields::ANALYSES.key).unwrap() >= 2.0);
        assert_eq!(
            rx.results().status().lock_state,
            sdrkit_core::LockState::Searching
        );
        assert!(rows(rx.results(), &PACTORHF_EVENTS).is_empty());
    }
}

#[test]
fn events_survive_save_and_restore() {
    let (rx, _) = rx_run(
        &pactorhf::PACTOR2,
        PactorHfGenOptions::default(),
        12.0,
        0.0,
        None,
    );
    let state = rx.save_state().expect("a saved state");
    let n = rows(rx.results(), &PACTORHF_EVENTS).len();
    assert!(n > 0);
    let mut b = receiver(&pactorhf::PACTOR2, &[]);
    b.configure(&InputInfo::new(24e3, 14.072e6, 14.072e6).recording())
        .unwrap();
    b.restore_state(&state).unwrap();
    assert_eq!(rows(b.results(), &PACTORHF_EVENTS).len(), n);
    invoke(b.as_mut(), "clear", None);
    assert!(rows(b.results(), &PACTORHF_EVENTS).is_empty());
    assert!(num(&rows(rx.results(), &PACTORHF_EVENTS)[0], "start") > 0.0);
}
