//! Round trips for every stack: generator -> impairments (sdrkit-dev-test: noise, offset, clock error, multipath) -> receiver.

use crate::common::*;
use sdrkit_core::{Complex32, Generator, Value};
use sdrkit_dev_test::{ClockError, Echo, FrequencyOffset};
use sdrkit_stack_marine::ais::generator::{BurstSpec, DemoOptions, PortDemo, Render};
use sdrkit_stack_marine::atis::{AtisGen, AtisGenerator};
use sdrkit_stack_marine::dsc::call;
use sdrkit_stack_marine::hfgen;
use sdrkit_stack_marine::vdes::{Modulation, VdesModulationRow};
use sdrkit_stack_marine::{navtex, AIS, ATIS, MARINE, VDES};
use serde_json::json;

fn gen(g: &mut dyn Generator, secs: f64) -> Vec<Complex32> {
    let mut x = vec![Complex32::default(); (secs * g.rate_hz()) as usize];
    let n = g.generate(&mut x);
    x.truncate(n);
    x
}

fn impair(
    mut x: Vec<Complex32>,
    rate: f64,
    offset: f64,
    ppm: f64,
    echo: Option<(usize, f64)>,
) -> Vec<Complex32> {
    FrequencyOffset::new(rate, offset).process(&mut x);
    if let Some((d, l)) = echo {
        Echo::new(d, l).process(&mut x);
    }
    if ppm != 0.0 {
        let mut c = ClockError::new(rate, ppm);
        let mut y = Vec::with_capacity(x.len() + 100);
        c.process(&x, &mut y);
        x = y;
    }
    x
}

#[test]
fn ais_port_demo_round_trip() {
    let rate = 2e6;
    let mut demo = PortDemo::new(DemoOptions {
        render: Render {
            rate,
            duration_s: 0.0,
            ..Default::default()
        },
        ..Default::default()
    });
    let x = gen(&mut demo, 35.0);
    // The demo's own randomness is kept apart from the noise: the same bursts with noise added.
    let mut noise = Impair::snr(30.0).apply(Box::new(Replay::new(x, rate)));
    let x = generate(noise.as_mut(), 35.0);
    let x = impair(x, rate, 1500.0, 20.0, Some((3, -12.0)));
    let mut rx = receiver(&AIS, None, &[]);
    configure(rx.as_mut(), rate, 162e6, 162e6);
    let _ = run(rx.as_mut(), &x, 100_000);
    let r = rx.results();
    let (good, bad, stations) = (
        count(r, "frames_ok"),
        count(r, "frames_bad"),
        count(r, "stations"),
    );
    let sent = demo.sent.iter().filter(|s| s.time < 34.9).count() as u64;
    println!("sent {sent}, good {good}, bad {bad}, stations {stations}");
    assert!(sent > 100);
    assert!(good >= sent - 1, "missing bursts");
    assert!(good <= sent);
    assert_eq!(stations, 19);
    let ch = r.table(r.decl().table_id("channels").unwrap()).unwrap();
    let good_on = |id: &str| {
        ch.row(id)
            .and_then(|row| row.get(&sdrkit_core::publish::keys::FRAMES_OK))
            .unwrap_or(0)
    };
    assert!(good_on("A") > 20 && good_on("B") > 20);
}

#[test]
fn ais_bursts_with_offsets_and_clock_errors() {
    let rate = 250e3;
    let mut bursts = Vec::new();
    let offs = [
        -8500.0, -6000.0, -3500.0, -2000.0, -800.0, 0.0, 800.0, 2000.0, 3500.0, 6000.0, 8500.0,
    ];
    for (k, o) in offs.iter().enumerate() {
        let bits = sdrkit_stack_marine::ais::encode::position_a(
            &sdrkit_stack_marine::ais::encode::PositionA {
                msg_type: 1,
                mmsi: 244_000_000 + k as u32,
                status: 0,
                rot_raw: 0,
                sog_kn: Some(10.0),
                lat: Some(51.95),
                lon: Some(4.3),
                cog_deg: Some(90.0),
                heading: Some(90),
                second: 1,
            },
        );
        bursts.push(BurstSpec {
            channel: if k % 2 == 0 { 'A' } else { 'B' },
            start_s: 0.1 + 0.05 * k as f64,
            level: 1.0,
            offset_hz: *o,
            phase: 0.0,
            bits,
        });
    }
    for ppm in [-200.0, 0.0, 200.0] {
        // A slow sample clock produces fewer samples: 1.05 s keeps the report that releases the last burst.
        let x = ais_signal(
            &bursts,
            &Render {
                rate,
                duration_s: 1.05,
                ..Default::default()
            },
            &Impair {
                clock_ppm: ppm,
                ..Impair::snr(18.0)
            },
        );
        let mut rx = receiver(&AIS, None, &[]);
        configure(rx.as_mut(), rate, 162e6, 162e6);
        let _ = run(rx.as_mut(), &x, 10_000);
        assert_eq!(
            count(rx.results(), "frames_ok"),
            offs.len() as u64,
            "ppm {ppm}"
        );
    }
}

#[test]
fn ais_cut_independence() {
    let rate = 250e3;
    let demo = PortDemo::new(DemoOptions {
        vessels: 4,
        render: Render {
            rate,
            duration_s: 0.0,
            ..Default::default()
        },
        ..Default::default()
    });
    let mut g = Impair::snr(25.0).apply(Box::new(demo));
    let x = gen(g.as_mut(), 3.0);
    let mut out = Vec::new();
    for chunk in [1usize, 777, 65_536] {
        let mut rx = receiver(&AIS, None, &[("time_base", json!(1.8e9))]);
        configure(rx.as_mut(), rate, 162e6, 162e6);
        let ev = run(rx.as_mut(), &x, chunk);
        let s: Vec<String> = ev
            .iter()
            .filter(|e| e.kind() == "ais.nmea")
            .map(|e| field_json(e, "sentence").to_string())
            .collect();
        out.push((
            s,
            count(rx.results(), "frames_ok"),
            count(rx.results(), "reports"),
        ));
    }
    assert!(!out[0].0.is_empty());
    assert_eq!(out[0], out[1]);
    assert_eq!(out[1], out[2]);
}

#[test]
fn ais_noise_only_and_rates() {
    let rate = 2e6;
    let mut rng = sdrkit_dev_test::Awgn::with_sigma(0.01, 3);
    let x = rng.noise((rate * 3.0) as usize);
    let mut rx = receiver(&AIS, None, &[]);
    configure(rx.as_mut(), rate, 162e6, 162e6);
    let _ = run(rx.as_mut(), &x, 65_536);
    let r = rx.results();
    assert_eq!(
        (
            count(r, "frames_ok"),
            count(r, "stations"),
            text(r, "state")
        ),
        (0, 0, Some("searching".to_string()))
    );
    assert!(count(r, "frames_bad") < 5);
    // 100 ksps is refused, with the message.
    let mut rx = receiver(&AIS, None, &[]);
    assert!(rx
        .configure(&sdrkit_core::InputInfo::new(100e3, 162e6, 162e6))
        .is_err());
    assert_eq!(
        rx.results().status().summary,
        "AIS needs a sample rate of at least 250 ksps"
    );
}

/// SNR in the signal's own bandwidth for an SNR in 3 kHz (304 Hz for NAVTEX and MF/HF DSC).
fn in_304_hz(snr_3k_db: f64) -> f64 {
    snr_3k_db + 10.0 * (3000.0f64 / 304.0).log10()
}

fn with(a: &[(&'static str, Json)], b: &[(&'static str, Json)]) -> Vec<(&'static str, Json)> {
    a.iter().chain(b).cloned().collect()
}

#[test]
fn navtex_round_trip_with_noise_and_offset() {
    let rate = 12e3;
    let msgs = navtex::demo_messages();
    let x = hf_signal(
        hfgen::navtex_programme(&msgs, 3.0),
        hfgen::navtex_info(),
        rate,
        75.0,
        &snr(in_304_hz(6.0)),
    );
    let x = impair(x, rate, 40.0, 50.0, None);
    let mut rx = receiver(&MARINE, Some("navtex"), &[]);
    configure(rx.as_mut(), rate, 518e3, 518e3);
    let ev = run(rx.as_mut(), &x, 4096);
    let got: Vec<_> = ev
        .iter()
        .filter(|e| e.kind() == "navtex.message" && field_json(e, "complete") == true)
        .collect();
    assert!(got.len() >= 3, "{}", got.len());
    for (m, want) in got.iter().zip(&msgs) {
        let w: Vec<&str> = want.lines().collect();
        assert_eq!(
            field_json(m, "header"),
            w[0].trim_start_matches("ZCZC ").trim()
        );
        assert_eq!(field_json(m, "text"), w[1..w.len() - 1].join("\n"));
    }
}

#[test]
fn navtex_with_fading() {
    let rate = 12e3;
    let msgs = navtex::demo_messages();
    let imp = with(
        &snr(in_304_hz(15.0)),
        &[("gen_fading", json!("hf_moderate")), ("gen_seed", json!(5))],
    );
    let x = hf_signal(
        hfgen::navtex_programme(&msgs, 3.0),
        hfgen::navtex_info(),
        rate,
        75.0,
        &imp,
    );
    let mut rx = receiver(&MARINE, Some("navtex"), &[]);
    configure(rx.as_mut(), rate, 518e3, 518e3);
    let ev = run(rx.as_mut(), &x, 4096);
    let n = ev.iter().filter(|e| e.kind() == "navtex.message").count();
    assert!(n >= 2, "{n}");
}

#[test]
fn dsc_mf_round_trip_at_2msps() {
    let rate = 2e6;
    let calls = call::test_calls(false);
    // The dial at -20 kHz in the stream (the radio 20 kHz above the dial), and 30 Hz of mistuning.
    let imp = with(
        &snr(in_304_hz(10.0)),
        &[("gen_offset_hz", json!(-20e3 + 30.0))],
    );
    let x = hf_signal(
        hfgen::dsc_programme(&calls, 1.0),
        hfgen::dsc_info(),
        rate,
        40.0,
        &imp,
    );
    let mut rx = receiver(&MARINE, None, &[]);
    configure(rx.as_mut(), rate, 2187.5e3 + 20e3, 2187.5e3);
    let ev = run(rx.as_mut(), &x, 262_144);
    let got: Vec<_> = ev.iter().filter(|e| e.kind() == "dsc.call").collect();
    let kinds: Vec<&str> = got
        .iter()
        .map(|e| {
            e.at(&sdrkit_stack_marine::marine::DscCallEvent::KIND)
                .unwrap()
        })
        .collect();
    assert_eq!(
        kinds,
        [
            "Distress alert",
            "All ships safety call",
            "Individual call",
            "Distress alert acknowledgement"
        ]
    );
    assert!(got.iter().all(|e| (e.freq_hz - 2187.5e3).abs() < 1.0));
}

#[test]
fn dsc_vhf_round_trip_and_cut_independence() {
    let rate = 48e3;
    let calls = call::test_calls(true);
    // 18 dB in 12.5 kHz is 16.9 dB in the 16 kHz of the signal.
    let x = hf_signal(
        hfgen::dsc_vhf_programme(&calls, 0.3),
        hfgen::dsc_vhf_info(),
        rate,
        6.0,
        &snr(18.0 - 10.0 * (16.0f64 / 12.5).log10()),
    );
    let x = impair(x, rate, 200.0, 0.0, None);
    let mut texts = Vec::new();
    for chunk in [1usize, 1000, 32_768] {
        let mut rx = receiver(&MARINE, Some("dsc"), &[]);
        configure(rx.as_mut(), rate, 156.525e6, 156.525e6);
        let ev = run(rx.as_mut(), &x, chunk);
        let t: Vec<String> = ev
            .iter()
            .filter(|e| e.kind() == "dsc.call")
            .map(|e| field_json(e, "text").to_string())
            .collect();
        texts.push(t);
    }
    assert_eq!(texts[0].len(), 4, "{:?}", texts[0]);
    assert_eq!(texts[0], texts[1]);
    assert_eq!(texts[1], texts[2]);
}

/// The VHF DSC calls of one trial (`seed`) at `snr_25k` dB in 25 kHz, moved by `offset` Hz and a clock error of `ppm`.
fn vhf_dsc_calls(snr_25k: f64, offset: f64, ppm: f64, seed: u64, signal: bool) -> usize {
    let rate = 48e3;
    let prog = if signal {
        let mut p = hfgen::dsc_vhf_programme(&call::test_calls(true), 0.3);
        p.push(hfgen::Segment::Silence { secs: 0.5 });
        p
    } else {
        vec![hfgen::Segment::Silence { secs: 6.0 }]
    };
    let imp = with(
        &snr(snr_25k + 10.0 * (25.0f64 / 16.0).log10()),
        &[("gen_seed", json!(seed))],
    );
    let x = hf_signal(prog, hfgen::dsc_vhf_info(), rate, 6.0, &imp);
    let x = impair(x, rate, offset, ppm, None);
    let mut rx = receiver(&MARINE, Some("dsc"), &[]);
    configure(rx.as_mut(), rate, 156.525e6, 156.525e6);
    run(rx.as_mut(), &x, 32_768)
        .iter()
        .filter(|e| e.kind() == "dsc.call")
        .count()
}

/// VHF DSC behind the discriminator-mean follow-up and its 4.5 kHz filter. A trial passes with half the clean run's calls; of 16
/// trials (SNR in 25 kHz) at least 90 % pass at 2 dB, with a 3 kHz carrier error (20 ppm) at 2 dB, with a 50 ppm clock at 2 dB
/// and 5 kHz above the centre at 4 dB. Noise alone gives no call in 40 runs.
#[test]
fn dsc_vhf_follow_up_near_the_threshold() {
    let clean = vhf_dsc_calls(40.0, 0.0, 0.0, 1, true);
    assert_eq!(clean, 4);
    let mut short = Vec::new();
    for (name, snr_db, offset, ppm) in [
        ("AWGN", 2.0, 0.0, 0.0),
        ("3 kHz carrier error", 2.0, 3000.0, 0.0),
        ("50 ppm clock", 2.0, 0.0, 50.0),
        ("5 kHz above the centre", 4.0, 5000.0, 0.0),
    ] {
        let pass = (0..16)
            .filter(|&s| 2 * vhf_dsc_calls(snr_db, offset, ppm, 30 + s, true) >= clean)
            .count();
        println!("{name} at {snr_db} dB: {pass} of 16");
        if pass * 10 < 16 * 9 {
            short.push(format!("{name} at {snr_db} dB: {pass} of 16"));
        }
    }
    let false_calls: usize = (0..40)
        .map(|s| vhf_dsc_calls(-10.0, 0.0, 0.0, 100 + s, false))
        .sum();
    assert_eq!(false_calls, 0);
    assert!(short.is_empty(), "{short:#?}");
}

#[test]
fn fax_round_trip_with_slant() {
    let rate = 12e3;
    let rows = hfgen::chart(1809, 60);
    let imp = with(&snr(25.0), &[("gen_clock_ppm", json!(30.0))]);
    let x = hf_signal(
        hfgen::fax_programme(rows.clone(), 120.0, 576, 10.0, 2.0),
        hfgen::fax_info(),
        rate,
        60.0,
        &imp,
    );
    let mut rx = receiver(&MARINE, Some("wefax"), &[]);
    configure(rx.as_mut(), rate, 7878.1e3, 7878.1e3);
    let _ = run(rx.as_mut(), &x, 4096);
    let r = rx.results();
    assert_eq!(text(r, "fax_state").as_deref(), Some("stopped"));
    assert_eq!(count(r, "lines"), 61);
    let slant = num(r, "slant_ppm").unwrap();
    assert!((slant - 30.0).abs() < 4.0, "{slant}");
}

#[test]
fn vdes_round_trip() {
    let rate = 400e3;
    let g = sdrkit_stack_marine::vdes::VdesGenerator::new(rate, 3);
    // A 25 kHz burst at -20 dBFS, 20 dB above the noise in 25 kHz, the carrier 300 Hz off.
    let mut g = impaired(
        Box::new(g),
        &[
            ("gen_level_dbfs", json!(-20.0)),
            ("gen_noise", json!("snr")),
            ("gen_snr_db", json!(20.0)),
            ("gen_offset_hz", json!(300.0)),
        ],
    );
    let x = gen(g.as_mut(), 2.4);
    let mut rx = receiver(&VDES, None, &[]);
    configure(rx.as_mut(), rate, 161.9e6, 161.9e6);
    let ev = run(rx.as_mut(), &x, 50_000);
    check_events(
        &sdrkit_stack_marine::typed::shape("vdes").results_decl(),
        &ev,
    );
    let r = rx.results();
    let asm = typed_rows(r, "asm");
    assert!(asm.len() >= 7, "{}", asm.len());
    let b = typed_rows(r, "bursts");
    assert!(b.len() >= 11, "{}", b.len());
    for m in ["pi/4-QPSK", "8PSK", "16QAM"] {
        let n = rows(r, "bursts")
            .iter()
            .filter(|row| row.at(&VdesModulationRow::MODULATION).map(Modulation::name) == Some(m))
            .count();
        assert!(n >= 3, "{m}: {n}");
        let by = r
            .table(r.decl().table_id("by_modulation").unwrap())
            .unwrap();
        assert_eq!(
            by.row(m).and_then(|row| row.at(&VdesModulationRow::COUNT)),
            Some(n as i64),
            "{m}"
        );
    }
}

#[test]
fn atis_round_trip_at_1msps() {
    let rate = 1e6;
    let o = AtisGen {
        rate,
        channel_hz: -150e3,
        ..Default::default()
    };
    // The carrier at -14 dBFS, C/N 20 dB in 12.5 kHz, 700 Hz off, the clock 200 ppm fast.
    let mut g = impaired(
        Box::new(AtisGenerator::new(o)),
        &[
            ("gen_level_dbfs", json!(-14.0)),
            ("gen_noise", json!("snr")),
            ("gen_snr_db", json!(20.0)),
            ("gen_offset_hz", json!(700.0)),
            ("gen_clock_ppm", json!(200.0)),
        ],
    );
    let x = gen(g.as_mut(), 12.0);
    let mut rx = receiver(&ATIS, None, &[]);
    configure(rx.as_mut(), rate, 156.65e6, 156.5e6);
    let ev = run(rx.as_mut(), &x, 100_000);
    check_events(
        &sdrkit_stack_marine::typed::shape("atis").results_decl(),
        &ev,
    );
    let good: Vec<_> = ev
        .iter()
        .filter(|e| e.kind() == "atis.message" && field_json(e, "check_ok") == true)
        .collect();
    assert!(good.len() >= 4, "{}", good.len());
    let r = rx.results();
    assert_eq!(count(r, "frames_bad"), 0);
    let st = typed_rows(r, "stations");
    let text = |s: &str| Value::Text(s.into());
    assert!(st
        .iter()
        .any(|s| s[1] == text("PC8075") && s[3] == text("Netherlands")));
}

#[test]
fn atis_inverted_spectrum() {
    let rate = 48e3;
    let o = AtisGen {
        rate,
        invert: true,
        ..Default::default()
    };
    let mut g = impaired(
        Box::new(AtisGenerator::new(o)),
        &[
            ("gen_level_dbfs", json!(-14.0)),
            ("gen_noise", json!("snr")),
            ("gen_snr_db", json!(30.0)),
        ],
    );
    let x = gen(g.as_mut(), 6.0);
    let mut rx = receiver(&ATIS, None, &[]);
    configure(rx.as_mut(), rate, 156.5e6, 156.5e6);
    let _ = run(rx.as_mut(), &x, 4096);
    // Messages: check is column 6, inverted column 8.
    let m = typed_rows(rx.results(), "messages");
    assert!(!m.is_empty());
    let yes = Value::Bool(true);
    assert!(m.iter().all(|m| m[8] == yes && m[6] == yes));
}

fn collision_counts(
    bursts: &[BurstSpec],
    render: &Render,
    imp: &Impair,
) -> Vec<(Vec<String>, u64)> {
    let x = ais_signal(bursts, render, imp);
    let mut out = Vec::new();
    for cc in [true, false] {
        let mut rx = receiver(&AIS, None, &[("collision_cancel", json!(cc))]);
        configure(rx.as_mut(), render.rate, 162e6, 162e6);
        let ev = run(rx.as_mut(), &x, 8192);
        let s: Vec<String> = ev
            .iter()
            .filter(|e| e.kind() == "ais.nmea")
            .map(|e| field_json(e, "sentence").to_string())
            .collect();
        out.push((s, count(rx.results(), "collision_recoveries")));
    }
    out
}

fn mmsi_of(s: &str) -> u32 {
    let bits = bits_of(&[s.trim_matches('"')]).expect("a good sentence");
    sdrkit_stack_marine::ais::message::header(&bits)
        .expect("a header")
        .1
}

fn pos_a(m: u32) -> Vec<u8> {
    sdrkit_stack_marine::ais::encode::position_a(&sdrkit_stack_marine::ais::encode::PositionA {
        msg_type: 1,
        mmsi: m,
        status: 0,
        rot_raw: 0,
        sog_kn: Some(5.0),
        lat: Some(50.0),
        lon: Some(-1.0),
        cog_deg: Some(10.0),
        heading: Some(10),
        second: 3,
    })
}

/// a strong burst then a weak one 6 ms later on the same channel: all 20 strong, at least 18 weak.
#[test]
fn ais_collisions_on_one_channel() {
    let rate = 250e3;
    let mut bursts = Vec::new();
    for k in 0..20 {
        let t = 0.1 + 0.08 * k as f64;
        bursts.push(BurstSpec {
            channel: 'A',
            start_s: t,
            level: 1.0,
            offset_hz: 0.0,
            phase: 0.0,
            bits: pos_a(244_100_000 + k),
        });
        bursts.push(BurstSpec {
            channel: 'A',
            start_s: t + 0.006,
            level: 0.5,
            offset_hz: 600.0,
            phase: 1.0,
            bits: pos_a(244_200_000 + k),
        });
    }
    // The last frames are released 0.2 s after their burst, at a report.
    let c = collision_counts(
        &bursts,
        &Render {
            rate,
            duration_s: 2.2,
            ..Default::default()
        },
        &Impair::snr(25.0),
    );
    let strong = |s: &[String]| s.len();
    println!(
        "{:?}",
        c.iter().map(|x| (x.0.len(), x.1)).collect::<Vec<_>>()
    );
    assert!(strong(&c[0].0) >= 20 + 18);
    assert!(c[0].0.len() >= c[1].0.len() + 4);
    assert_eq!(c[1].1, 0);
    assert!(c[0].1 >= 4);
    // Nothing invented: every sentence with cancellation is one that was sent.
    let sent: std::collections::BTreeSet<u32> = (0..20)
        .flat_map(|k| [244_100_000 + k, 244_200_000 + k])
        .collect();
    for s in &c[0].0 {
        let m = mmsi_of(s);
        assert!(sent.contains(&m), "{m}");
    }
}

/// Section 2.5: colliding pairs (the weaker 2 to 12 ms later, 6 to 14 dB lower, 300 to 900 Hz apart) with carrier and clock
/// errors, noise, DC and 8-bit samples: cancellation finds at least 4 more weak messages, invents nothing, never does worse; on
/// clean traffic the result is the same on and off.
#[test]
fn ais_collision_cancellation_facts() {
    let rate = 250e3;
    let mut bursts = Vec::new();
    let mut rng = 12345u64;
    let mut next = || {
        rng = rng
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        (rng >> 33) as f64 / (1u64 << 31) as f64
    };
    for k in 0..16 {
        let t = 0.1 + 0.1 * k as f64;
        let delay = 0.002 + 0.010 * next();
        let db = 6.0 + 8.0 * next();
        let df = (300.0 + 600.0 * next()) * if next() < 0.5 { -1.0 } else { 1.0 };
        bursts.push(BurstSpec {
            channel: 'A',
            start_s: t,
            level: 1.0,
            offset_hz: 0.0,
            phase: 0.0,
            bits: pos_a(244_100_000 + k),
        });
        bursts.push(BurstSpec {
            channel: 'A',
            start_s: t + delay,
            level: 10f64.powf(-db / 20.0),
            offset_hz: df,
            phase: next() * 6.0,
            bits: pos_a(244_200_000 + k),
        });
    }
    let render = Render {
        rate,
        duration_s: 2.1,
        ..Default::default()
    };
    let imp = Impair {
        offset_hz: 350.0,
        clock_ppm: 30.0,
        dc: 0.05,
        quantize8: true,
        ..Impair::snr(25.0)
    };
    let c = collision_counts(&bursts, &render, &imp);
    println!(
        "{:?}",
        c.iter().map(|x| (x.0.len(), x.1)).collect::<Vec<_>>()
    );
    let weak = |s: &[String]| s.iter().filter(|s| mmsi_of(s) >= 244_200_000).count();
    assert!(weak(&c[0].0) >= weak(&c[1].0) + 4);
    assert!(c[0].0.len() >= c[1].0.len());
    for s in &c[1].0 {
        assert!(c[0].0.contains(s), "lost with cancellation: {s}");
    }
    assert!(c[0].1 >= 4);
    assert_eq!(c[1].1, 0);
    // Clean traffic: the same on and off.
    let clean: Vec<BurstSpec> = (0..12)
        .map(|k| BurstSpec {
            channel: if k % 2 == 0 { 'A' } else { 'B' },
            start_s: 0.1 + 0.07 * k as f64,
            level: 1.0,
            offset_hz: 0.0,
            phase: 0.0,
            bits: pos_a(244_300_000 + k),
        })
        .collect();
    let c = collision_counts(
        &clean,
        &Render {
            rate,
            duration_s: 1.4,
            ..Default::default()
        },
        &Impair::snr(20.0),
    );
    assert_eq!(c[0].0, c[1].0);
    assert_eq!(c[0].0.len(), 12);
    // Noise alone: no message.
    let c = collision_counts(
        &[],
        &Render {
            rate,
            duration_s: 1.0,
            ..Default::default()
        },
        &Impair::snr(20.0),
    );
    assert!(c[0].0.is_empty() && c[1].0.is_empty());
}

/// The DSC scanner steps a single radio (a retune request each move) through the distress channels in their weighted order, one
/// dwell each; a call heard on a channel keeps the radio there for the hold time after it; parallel decoding stops the scanner.
#[test]
fn dsc_scanner_steps_the_radio_and_holds_on_a_call() {
    let rate = 24e3;
    let calls = call::test_calls(false);
    let mut rx = receiver(
        &MARINE,
        Some("dsc"),
        &[("dsc_scan", json!(true)), ("dsc_scan_dwell_s", json!(3.0))],
    );
    let mut dial_khz = 2187.5;
    rx.configure(&sdrkit_core::InputInfo::new(
        rate,
        dial_khz * 1e3,
        dial_khz * 1e3,
    ))
    .unwrap();
    // The radio: noise everywhere; on 8414.5 kHz the individual test call (20 dot bits), once, after 20 s.
    let noise = |seed: u64| with(&snr(in_304_hz(20.0)), &[("gen_seed", json!(seed))]);
    let make = |prog: Vec<hfgen::Segment>, seed: u64| -> Box<dyn Generator> {
        let g = hfgen::HfGenerator::new(
            prog,
            hfgen::HfOptions {
                rate,
                once: false,
                info: hfgen::dsc_info(),
            },
        )
        .unwrap();
        impaired(Box::new(g), &noise(seed))
    };
    let silence_prog = || vec![hfgen::Segment::Silence { secs: 1e6 }];
    let mut g = make(silence_prog(), 1);
    let step = (rate * 0.05) as usize;
    let mut buf = vec![Complex32::default(); step];
    let mut moves: Vec<(f64, f64)> = Vec::new();
    let mut calls_seen = Vec::new();
    let mut sent = false;
    for k in 0..(70.0 / 0.05) as usize {
        let t = (k + 1) as f64 * 0.05;
        if !sent && t >= 20.0 && (dial_khz - 8414.5).abs() < 0.01 {
            let mut prog = hfgen::dsc_programme(&calls[2..3], 0.3);
            prog.extend(silence_prog());
            g = make(prog, 2);
            sent = true;
        }
        let n = g.generate(&mut buf);
        buf[n..].iter_mut().for_each(|s| *s = Complex32::default());
        let mut out = sdrkit_core::Output::default();
        rx.feed(&buf, &mut out);
        for e in &out.events {
            if e.kind() == "dsc.call" {
                calls_seen.push((t, dial_khz));
            }
        }
        for r in &out.requests {
            let sdrkit_core::HostRequest::Retune { tuned_hz, .. } = r else {
                panic!("{r:?}");
            };
            let khz = tuned_hz / 1e3;
            if (khz - dial_khz).abs() > 0.01 {
                dial_khz = khz;
                moves.push((t, khz));
                rx.retuned(&sdrkit_core::InputInfo::new(rate, khz * 1e3, khz * 1e3));
                if !sent || (khz - 8414.5).abs() > 0.01 {
                    g = make(silence_prog(), k as u64);
                }
            }
        }
    }
    println!("{moves:?}\n{calls_seen:?}");
    // The weighted order of the distress set: 2187.5 (4), 4207.5 (3), 6312 (3), 8414.5 (4), 12577 (3), 16804.5 (3), round by round.
    let first: Vec<f64> = moves.iter().take(5).map(|m| m.1).collect();
    assert_eq!(first, [4207.5, 6312.0, 8414.5, 12577.0, 16804.5]);
    // One dwell (3 s) per slot while nothing is heard.
    for w in moves.windows(2).take(4) {
        let d = w[1].0 - w[0].0;
        assert!((2.9..3.4).contains(&d), "{moves:?}");
    }
    // The call on 8414.5 kHz was heard, and the radio stayed there for the hold time after it.
    let (tc, f) = calls_seen
        .first()
        .copied()
        .expect("the call is heard while the radio is there");
    assert!((f - 8414.5).abs() < 0.01);
    let left = moves
        .iter()
        .find(|m| m.0 > tc)
        .expect("it moves on after the hold")
        .0;
    assert!(left - tc >= 19.5, "left {left} after the call at {tc}");
}

fn sentences_of(ev: &[sdrkit_core::Event]) -> Vec<String> {
    ev.iter()
        .filter(|e| e.kind() == "ais.nmea")
        .map(|e| field_json(e, "sentence").as_str().unwrap().to_string())
        .collect()
}

/// at 6 Msps at 159.4 MHz all four channels are covered; type 27 on 75 / 76 produces long-range stations with channel letters C
/// and D; at 156.8 MHz (2 Msps) only 75 + 76 are heard.
#[test]
fn a16_long_range_channels() {
    let mut bursts = Vec::new();
    for k in 0..8u32 {
        let ch = if k % 2 == 0 { 'C' } else { 'D' };
        let bits = sdrkit_stack_marine::ais::encode::long_range(
            244_500_000 + k,
            0,
            Some(52.0 + k as f64 * 0.1),
            Some(4.0),
            Some(10.0),
            Some(90.0),
            false,
        );
        bursts.push(BurstSpec {
            channel: ch,
            start_s: 0.1 + 0.1 * k as f64,
            level: 1.0,
            offset_hz: 0.0,
            phase: 0.0,
            bits,
        });
    }
    for k in 0..4u32 {
        let ch = if k % 2 == 0 { 'A' } else { 'B' };
        bursts.push(BurstSpec {
            channel: ch,
            start_s: 0.15 + 0.1 * k as f64,
            level: 1.0,
            offset_hz: 0.0,
            phase: 0.0,
            bits: pos_a(244_600_000 + k),
        });
    }
    // 6 Msps at 159.4 MHz: the A + B middle 2.6 MHz above the centre, C + D 2.6 MHz below.
    let rate = 6e6;
    let x = ais_signal(
        &bursts,
        &Render {
            rate,
            duration_s: 1.4,
            tune_offset_hz: -2.6e6,
            long_range_middle_hz: -5.2e6,
        },
        &Impair::snr(25.0),
    );
    let mut rx = receiver(&AIS, None, &[]);
    configure(rx.as_mut(), rate, 159.4e6, 159.4e6);
    let ev = run(rx.as_mut(), &x, 1 << 18);
    let r = rx.results();
    assert_eq!(count(r, "frames_ok"), 12);
    assert_eq!(count(r, "long_range_stations"), 8);
    let s = sentences_of(&ev);
    assert_eq!(s.iter().filter(|s| s.contains(",C,")).count(), 4);
    assert_eq!(s.iter().filter(|s| s.contains(",D,")).count(), 4);
    // 2 Msps at 156.8 MHz: only 75 + 76.
    let rate = 2e6;
    let x = ais_signal(
        &bursts,
        &Render {
            rate,
            duration_s: 1.4,
            tune_offset_hz: -5.2e6,
            long_range_middle_hz: -5.2e6,
        },
        &Impair::snr(25.0),
    );
    let mut rx = receiver(&AIS, None, &[]);
    configure(rx.as_mut(), rate, 156.8e6, 156.8e6);
    let ev = run(rx.as_mut(), &x, 1 << 17);
    let s = sentences_of(&ev);
    assert_eq!(s.len(), 8);
    assert!(s.iter().all(|s| s.contains(",C,") || s.contains(",D,")));
}

/// A17 (part): satellite mode decodes a Doppler sweep of +-4 kHz, and two bursts in one slot 8 dB apart (most of the weaker
/// ones by cancellation).
#[test]
fn a17_satellite_doppler_and_one_slot_pairs() {
    let rate = 250e3;
    let bursts: Vec<BurstSpec> = (0..12)
        .map(|k| BurstSpec {
            channel: if k % 2 == 0 { 'A' } else { 'B' },
            start_s: 0.1 + 0.06 * k as f64,
            level: 1.0,
            offset_hz: -4000.0 + 8000.0 * k as f64 / 11.0,
            phase: 0.3 * k as f64,
            bits: pos_a(244_700_000 + k),
        })
        .collect();
    let x = ais_signal(
        &bursts,
        &Render {
            rate,
            duration_s: 1.3,
            ..Default::default()
        },
        &Impair::snr(15.0),
    );
    let mut rx = receiver(&AIS, None, &[("satellite", json!(true))]);
    configure(rx.as_mut(), rate, 162e6, 162e6);
    let ev = run(rx.as_mut(), &x, 8192);
    assert_eq!(sentences_of(&ev).len(), 12);
    // Pairs in one slot (the weaker 1 ms later), 8 dB apart, 1.5 kHz apart in Doppler.
    let mut bursts = Vec::new();
    for k in 0..10u32 {
        let t = 0.1 + 0.08 * k as f64;
        bursts.push(BurstSpec {
            channel: 'A',
            start_s: t,
            level: 1.0,
            offset_hz: 2000.0,
            phase: 0.0,
            bits: pos_a(244_800_000 + k),
        });
        bursts.push(BurstSpec {
            channel: 'A',
            start_s: t + 0.001,
            level: 0.4,
            offset_hz: 500.0,
            phase: 2.0,
            bits: pos_a(244_900_000 + k),
        });
    }
    let x = ais_signal(
        &bursts,
        &Render {
            rate,
            duration_s: 1.3,
            ..Default::default()
        },
        &Impair::snr(25.0),
    );
    let mut rx = receiver(&AIS, None, &[("satellite", json!(true))]);
    configure(rx.as_mut(), rate, 162e6, 162e6);
    let ev = run(rx.as_mut(), &x, 8192);
    let s = sentences_of(&ev);
    let cancelled = count(rx.results(), "cancelled");
    println!("{} decoded, cancelled {cancelled}", s.len());
    assert!(s.len() >= 18, "{}", s.len());
    assert!(cancelled >= 6);
}

/// 8-bit samples, a DC offset of 0.06 and of 0.2: all decoded; a 20 ms gap of zeros costs at most the burst it hits.
#[test]
fn a10_quantisation_dc_and_a_gap() {
    let rate = 250e3;
    let bursts: Vec<BurstSpec> = (0..16)
        .map(|k| BurstSpec {
            channel: if k % 2 == 0 { 'A' } else { 'B' },
            start_s: 0.1 + 0.07 * k as f64,
            level: 1.0,
            offset_hz: 0.0,
            phase: 0.0,
            bits: pos_a(244_110_000 + k),
        })
        .collect();
    for dc in [0.0, 0.06, 0.2] {
        let x = ais_signal(
            &bursts,
            &Render {
                rate,
                duration_s: 1.6,
                ..Default::default()
            },
            &Impair {
                dc,
                quantize8: true,
                ..Impair::snr(20.0)
            },
        );
        let mut rx = receiver(&AIS, None, &[]);
        configure(rx.as_mut(), rate, 162e6, 162e6);
        let ev = run(rx.as_mut(), &x, 8192);
        assert_eq!(sentences_of(&ev).len(), 16, "DC {dc}");
    }
    let mut x = ais_signal(
        &bursts,
        &Render {
            rate,
            duration_s: 1.6,
            ..Default::default()
        },
        &Impair::snr(20.0),
    );
    // The gap hits the burst at 0.59 s.
    let a = (0.60 * rate) as usize;
    x[a..a + (0.02 * rate) as usize]
        .iter_mut()
        .for_each(|s| *s = Complex32::default());
    let mut rx = receiver(&AIS, None, &[]);
    configure(rx.as_mut(), rate, 162e6, 162e6);
    let ev = run(rx.as_mut(), &x, 8192);
    let s = sentences_of(&ev);
    assert!(s.len() >= 15, "{}", s.len());
    assert!(count(rx.results(), "frames_bad") <= 3);
    for k in 8..16 {
        assert!(
            s.iter().any(|x| mmsi_of(x) == 244_110_000 + k),
            "burst {k} after the gap"
        );
    }
}

/// parallel decoding. Of a list with one channel 5 kHz from the dial and one 4 MHz away at 250 ksps, one parallel channel
/// runs, and both the main and the parallel channel decode; the parallel channel's calls carry its frequency.
#[test]
fn d7_parallel_dsc_channels() {
    let rate = 250e3;
    let calls = call::test_calls(false);
    // The dial at 4202.5 kHz (20 kHz below the stream centre); 4207.5 kHz is 5 kHz above it, 8414.5 kHz far outside.
    let main = hf_signal(
        hfgen::dsc_programme(&calls[2..3], 0.5),
        hfgen::dsc_info(),
        rate,
        12.0,
        &with(&snr(in_304_hz(20.0)), &[("gen_offset_hz", json!(-20e3))]),
    );
    let par = hf_signal(
        hfgen::dsc_programme(&calls[1..2], 1.0),
        hfgen::dsc_info(),
        rate,
        12.0,
        &[("gen_offset_hz", json!(-15e3))],
    );
    let x: Vec<Complex32> = main.iter().zip(&par).map(|(a, b)| a + b).collect();
    let mut rx = receiver(&MARINE, Some("dsc"), &[("dsc_parallel", json!(true))]);
    configure(rx.as_mut(), rate, 4222.5e3, 4202.5e3);
    let ev = run(rx.as_mut(), &x, 16_384);
    let got: Vec<(String, f64)> = ev
        .iter()
        .filter(|e| e.kind() == "dsc.call")
        .map(|e| {
            (
                field_json(e, "kind").as_str().unwrap().to_string(),
                e.freq_hz,
            )
        })
        .collect();
    println!("{got:?}");
    assert!(got
        .iter()
        .any(|(k, f)| k == "Individual call" && (f - 4202.5e3).abs() < 1.0));
    assert!(got
        .iter()
        .any(|(k, f)| k == "All ships safety call" && (f - 4207.5e3).abs() < 1.0));
}
