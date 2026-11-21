//! Generator, impairments, receiver: every transmission checked off, no phantom.

use super::common::*;
use sdrkit_algo_sim::clock::ClockError;
use sdrkit_algo_sim::fading::{Doppler, Tap, Tdl};
use sdrkit_algo_sim::front::Quantiser;
use sdrkit_algo_sim::noise::Awgn;
use sdrkit_algo_sim::offset::FrequencyOffset;
use sdrkit_core::{Complex32, Generator, InputInfo, Receiver, Value};
use sdrkit_stack_adsb::codes;
use sdrkit_stack_adsb::commb;
use sdrkit_stack_adsb::generator::*;
use sdrkit_stack_adsb::message::Frame;
use serde_json::json;

/// Frames of every kind from a few aircraft, one per `gap` seconds, each aircraft announced by an extended squitter first.
fn mix(n: usize, gap: f64, seed: u64) -> Vec<(f64, Frame)> {
    let mut rng = Rng::new(seed);
    let addrs: Vec<u32> = (0..8)
        .map(|i| 0x400000 + i * 0x1111 + rng.below(0x100) as u32)
        .collect();
    let alt = codes::encode_ac12_25ft(31000).unwrap();
    let ac13 = codes::encode_ac13_25ft(31000).unwrap();
    let mut out = Vec::new();
    for i in 0..n {
        let t = 0.005 + i as f64 * gap;
        let a = addrs[i % 8];
        let f = if i < 8 {
            es_frame(17, 5, a, me_ident('A', 3, "MIX123"))
        } else {
            match (i / 8) % 12 {
                0 => es_frame(
                    17,
                    5,
                    a,
                    me_airborne(11, 0, alt, i % 2 == 0, 25.0 + (i % 8) as f64 * 0.3, 55.0),
                ),
                1 => es_frame(
                    17,
                    5,
                    a,
                    me_velocity_ground(100.0, -300.0, Some(-640), false, Some(100)),
                ),
                2 => df11(5, a, (i % 16) as u8),
                3 => df4_20(0, ac13, None, a),
                4 => df5_21(0, 0o1234, None, a),
                5 => df4_20(0, ac13, Some(commb::encode_20("MIX123")), a),
                6 => df5_21(
                    0,
                    0o1234,
                    Some(commb::encode_60(90.0, 280.0, 0.78, -640, -640)),
                    a,
                ),
                7 => df0_16(false, ac13, None, a),
                8 => df0_16(false, ac13, Some(0), a),
                9 => es_frame(18, 2, a ^ 0x5A_5A5A, me_ident('A', 2, "TIS")),
                10 => es_frame(17, 5, a, me_op_status(2, 9, 3)),
                _ => es_frame(17, 5, a, me_target_state(30000, 1013.2, Some(45.0), 9)),
            }
        };
        out.push((t, f));
    }
    out
}

fn render(
    rate: f64,
    frames: &[(f64, Frame)],
    amp: f32,
    snr_db: f64,
    filter: f64,
    seed: u64,
    offset: f64,
) -> (Vec<Complex32>, Vec<Transmission>) {
    let mut rng = Rng::new(seed + 99);
    let mut bursts = Vec::new();
    let mut txs = Vec::new();
    for (t, f) in frames {
        let freq = rng.range(-100e3, 100e3) + offset;
        bursts.push((
            Burst::mode_s(*t, f, amp, freq, rng.range(0.0, std::f64::consts::TAU)),
            Some(*f),
        ));
        txs.push(Transmission {
            time: *t,
            frame: *f,
            amp,
            freq,
            overlapped: false,
        });
    }
    let end = frames.last().map_or(0.01, |f| f.0) + 0.3;
    let mut g = AdsbGenerator::script(rate, filter, bursts, seed);
    let mut x = vec![Complex32::new(0.0, 0.0); (end * rate) as usize];
    g.generate(&mut x);
    add_band_noise(&mut x, rate, snr_db, filter, seed);
    (x, txs)
}

fn decode(
    rate: f64,
    x: &[Complex32],
    txs: &[Transmission],
    pattern: &[usize],
) -> (usize, usize, Run) {
    let mut rx = receiver(&[]);
    let r = run(&mut rx, rate, x, pattern);
    let (got, ph) = check_off(&r.frames, txs);
    (got.iter().filter(|&&g| g).count(), ph, r)
}

/// The transmissions as a clock error moves them (whichever sense the impairment uses: the one that matches more frames).
fn clocked(txs: &[Transmission], ppm: f64, delay: f64, x: &[Complex32]) -> Vec<Transmission> {
    let mut best = (0, Vec::new());
    for k in [1.0 + ppm * 1e-6, 1.0 / (1.0 + ppm * 1e-6), 1.0 - ppm * 1e-6] {
        for d in [delay, -delay, 0.0] {
            let t2: Vec<Transmission> = txs
                .iter()
                .map(|t| Transmission {
                    time: t.time * k + d / 4e6,
                    ..t.clone()
                })
                .collect();
            let (n, _, _) = decode(4e6, &x[..x.len().min(400_000)], &t2, &[65536]);
            if n >= best.0 {
                best = (n, t2);
            }
        }
    }
    best.1
}

fn quant8(x: &mut [Complex32]) {
    Quantiser::new(8).unwrap().process(x);
}

#[test]
fn every_kind_at_4_msps() {
    let frames = mix(400, 0.001, 1);
    let (mut x, txs) = render(4e6, &frames, 0.1, 30.0, 3.5e6, 1, 0.0);
    quant8(&mut x);
    let (n, ph, r) = decode(4e6, &x, &txs, &[65536]);
    assert_eq!(ph, 0);
    assert_eq!(n, 400, "{n}");
    // Level within 2.5 dB of the sent amplitude on average.
    let mean: f64 = r
        .frames
        .iter()
        .map(|a| a.level_dbfs.unwrap() as f64)
        .sum::<f64>()
        / r.frames.len() as f64;
    assert!((mean - 20.0 * 0.1f64.log10()).abs() < 2.5, "{mean}");
    let last = r.reports.last().unwrap();
    assert_eq!(last["messages"].as_array().unwrap().len(), 64);
    for df in [0, 4, 5, 11, 16, 17, 18, 20, 21] {
        assert!(r.frames.iter().any(|a| a.frame.df() == df), "DF{df}");
    }
    let noise = last["noise_dbfs"].as_f64().unwrap();
    let expect = 10.0 * (0.01f64 / 1e3 / 2e6 * 4e6 * 0.7).log10();
    assert!((noise - expect).abs() < 8.0, "{noise} {expect}");
}

#[test]
fn every_rate() {
    for rate in [
        2e6f64, 2.4e6, 3.2e6, 4e6, 5e6, 6e6, 8e6, 10e6, 12.5e6, 16e6, 20e6,
    ] {
        let frames = mix(300, 0.0007, 2);
        let filt = (0.875 * rate).clamp(1.75e6, 5e6);
        let (mut x, txs) = render(rate, &frames, 0.1, 28.0, filt, 2, 0.0);
        quant8(&mut x);
        let (n, ph, _) = decode(rate, &x, &txs, &[65536]);
        eprintln!("{rate}: {n} of 300");
        assert_eq!(ph, 0, "{rate}");
        assert!(n >= 297, "{rate}: {n}");
    }
}

#[test]
fn two_msps_at_every_sampling_phase() {
    for k in 0..16 {
        let frames: Vec<(f64, Frame)> = mix(40, 0.0007, 3)
            .into_iter()
            .map(|(t, f)| (t + k as f64 / 16.0 * 0.5e-6, f))
            .collect();
        let (x, txs) = render(2e6, &frames, 0.2, 40.0, 1.75e6, 3 + k, 0.0);
        let (n, ph, _) = decode(2e6, &x, &txs, &[65536]);
        assert_eq!(ph, 0);
        assert!(n >= 39, "phase {k}: {n}");
    }
}

#[test]
fn any_cut_returns_the_same_frames() {
    let frames = mix(200, 0.0009, 4);
    let (x, txs) = render(4e6, &frames, 0.06, 25.0, 3.5e6, 4, 0.0);
    let base = decode(4e6, &x, &txs, &[65536]).2.frames;
    let key = |v: &[sdrkit_stack_adsb::checks::Accepted]| {
        v.iter()
            .map(|a| (a.frame, (a.time * 1e9).round() as i64))
            .collect::<Vec<_>>()
    };
    for pattern in [
        &[1usize][..],
        &[7],
        &[4096],
        &[1, 7, 4096, 65536, 333, 20000],
        &[3, 1000],
    ] {
        let other = decode(4e6, &x, &txs, pattern).2.frames;
        assert_eq!(key(&other), key(&base), "{pattern:?}");
    }
}

#[test]
fn impairments() {
    let frames = mix(300, 0.0008, 5);
    let check = |name: &str, x: Vec<Complex32>, txs: &[Transmission], min: f64| {
        let (n, ph, _) = decode(4e6, &x, txs, &[65536]);
        eprintln!("{name}: {n}");
        assert_eq!(ph, 0, "{name}");
        assert!(n as f64 >= min * 3.0, "{name}: {n}");
    };
    let (x, txs) = render(4e6, &frames, 0.1, 24.0, 3.5e6, 5, 0.0);
    let mut c = x.clone();
    quant8(&mut c);
    check("clean", c, &txs, 99.5);
    check("no rounding", x.clone(), &txs, 99.5);
    for (dc, min) in [(0.05, 99.0), (0.15, 98.0)] {
        let mut c: Vec<Complex32> = x
            .iter()
            .map(|v| v + Complex32::new(dc, -dc * 0.5))
            .collect();
        quant8(&mut c);
        check("dc", c, &txs, min);
    }
    let iq = |x: &[Complex32], db: f32, deg: f32| -> Vec<Complex32> {
        let g = 10f32.powf(db / 20.0);
        let p = deg.to_radians();
        x.iter()
            .map(|v| Complex32::new(v.re, g * (v.im * p.cos() + v.re * p.sin())))
            .collect()
    };
    check("iq", iq(&x, 1.5, 8.0), &txs, 99.0);
    for off in [300e3, -300e3] {
        let mut c = x.clone();
        FrequencyOffset::new(4e6, off, 0.0).unwrap().process(&mut c);
        check("offset", c, &txs, 98.0);
    }
    for ppm in [100.0, -100.0] {
        let mut c = Vec::new();
        let mut ce = ClockError::new(ppm).unwrap();
        ce.process(&x, &mut c);
        let txs2 = clocked(&txs, ppm, ce.delay(), &c);
        check("clock", c, &txs2, 99.0);
    }
    // All together.
    let mut c = iq(&x, 1.0, 5.0);
    FrequencyOffset::new(4e6, 200e3, 0.0)
        .unwrap()
        .process(&mut c);
    for v in c.iter_mut() {
        *v += Complex32::new(0.05, 0.05);
    }
    let mut d = Vec::new();
    let mut ce = ClockError::new(50.0).unwrap();
    ce.process(&c, &mut d);
    let txs2 = clocked(&txs, 50.0, ce.delay(), &d);
    check("together", d, &txs2, 97.0);
    // A CW carrier 14 dB below the pulses.
    for f in [700e3, 50e3] {
        let a = 0.1 * 10f32.powf(-14.0 / 20.0);
        let c: Vec<Complex32> = x
            .iter()
            .enumerate()
            .map(|(i, v)| {
                let p = std::f64::consts::TAU * f * i as f64 / 4e6;
                v + Complex32::new(a * p.cos() as f32, a * p.sin() as f32)
            })
            .collect();
        check("cw", c, &txs, 99.0);
    }
    let (x175, txs175) = render(4e6, &frames, 0.1, 24.0, 1.75e6, 5, 0.0);
    check("filter 1.75 MHz", x175, &txs175, 97.0);
    // Multipath.
    let mut c = x.clone();
    // An echo 3 samples (0.75 µs) late, 12 dB down.
    Tdl::new(
        &[
            Tap::new(0.0, 0.0, Doppler::Static),
            Tap::new(0.75e-6, -12.0, Doppler::Static),
        ],
        4e6,
        1,
    )
    .unwrap()
    .process(&mut c);
    check("echo", c, &txs, 97.0);
}

#[test]
fn a_carrier_above_the_pulses_makes_nothing_false() {
    // A carrier 12 dB above the pulses. Reception can stop, but nothing false comes out and the
    // table holds only aircraft of the test.
    let frames = mix(300, 0.0008, 21);
    let (x, txs) = render(4e6, &frames, 0.1, 24.0, 3.5e6, 21, 0.0);
    // Every aircraft of the mix sends an extended squitter (DF17 or DF18) with its address in the clear.
    let mut addrs: Vec<u32> = frames
        .iter()
        .filter(|(_, f)| matches!(f.df(), 11 | 17 | 18))
        .map(|(_, f)| f.aa())
        .collect();
    addrs.sort_unstable();
    addrs.dedup();
    for f in [300e3, 50e3, -700e3] {
        let a = 0.1 * 10f32.powf(12.0 / 20.0);
        let mut c: Vec<Complex32> = x
            .iter()
            .enumerate()
            .map(|(i, v)| {
                let p = std::f64::consts::TAU * f * i as f64 / 4e6;
                v + Complex32::new(a * p.cos() as f32, a * p.sin() as f32)
            })
            .collect();
        quant8(&mut c);
        let mut rx = receiver(&[]);
        let r = run(&mut rx, 4e6, &c, &[65536]);
        let (got, ph) = check_off(&r.frames, &txs);
        eprintln!(
            "carrier at {f} Hz: {} decoded",
            got.iter().filter(|&&g| g).count()
        );
        assert_eq!(ph, 0, "{f}");
        let d = &view(rx.results());
        let n = d["aircraft_count"].as_u64().unwrap();
        assert!(n <= 24, "{f}: {n} aircraft");
        for a in d["aircraft"].as_array().unwrap() {
            let icao = u32::from_str_radix(a["icao"].as_str().unwrap(), 16).unwrap();
            assert!(
                addrs.contains(&icao),
                "{f}: {icao:06X} is not an aircraft of the test"
            );
        }
    }
}

#[test]
fn loud_messages() {
    // Pulses at amplitude 0.9 (-0.9 dBFS) are at least 90 % loud; 0.5 (-6 dBFS) and weaker are never loud.
    let frames: Vec<(f64, Frame)> = mix(48, 0.001, 33)
        .into_iter()
        .filter(|(_, f)| matches!(f.df(), 11 | 17 | 18))
        .collect();
    for (amp, loud_share) in [(0.9f32, Some(0.9)), (0.5, None), (0.2, None)] {
        let (x, _) = render(4e6, &frames, amp, 30.0, 3.5e6, 33, 0.0);
        let mut rx = receiver(&[]);
        run(&mut rx, 4e6, &x, &[65536]);
        let d = &view(rx.results());
        let (good, loud) = (d["good"].as_u64().unwrap(), d["loud"].as_u64().unwrap());
        eprintln!("amplitude {amp}: {loud} loud of {good}");
        assert!(good as usize >= frames.len() * 9 / 10, "{amp}: {good}");
        match loud_share {
            Some(s) => assert!(loud as f64 >= s * good as f64, "{amp}: {loud} of {good}"),
            None => assert_eq!(loud, 0, "{amp}"),
        }
    }
}

#[test]
fn sensitivity() {
    let mut bad = Vec::new();
    for (rate, snr, min) in [
        (4e6f64, 12.0, 0.99),
        (4e6, 10.0, 0.85),
        (10e6, 12.0, 0.99),
        (2.4e6, 13.0, 0.98),
        (20e6, 12.0, 0.97),
        (2e6, 20.0, 0.98),
    ] {
        // Extended squitters (the frames whose error correction the setting allows), after the address is known.
        let frames: Vec<(f64, Frame)> = mix(300 * 12 / 5, 0.0008, 6)
            .into_iter()
            .enumerate()
            .filter(|(i, (_, f))| *i < 8 || matches!(f.df(), 17 | 18))
            .map(|(_, x)| x)
            .take(300)
            .enumerate()
            .map(|(i, (_, f))| (0.005 + i as f64 * 0.0008, f))
            .collect();
        let filt = (0.875 * rate).clamp(1.75e6, 5e6);
        let (x, txs) = render(rate, &frames, 0.1, snr, filt, 6, 0.0);
        let mut rx = receiver(&[]);
        let rr = run(&mut rx, rate, &x, &[65536]);
        let (got, ph) = check_off(&rr.frames, &txs);
        let n = got.iter().filter(|&&g| g).count();
        if std::env::var("SHOW_MISSED").is_ok() {
            for (t, g) in txs.iter().zip(&got) {
                if !g {
                    eprintln!("  missed {:.6} {}", t.time, t.frame.hex());
                }
            }
        }
        eprintln!("{rate} {snr} dB: {n}");
        assert_eq!(ph, 0);
        if (n as f64) < min * txs.len() as f64 {
            bad.push(format!("{rate} {snr}: {n}"));
        }
    }
    assert!(bad.is_empty(), "{bad:?}");
}

#[test]
fn noise_only_decodes_nothing() {
    for rate in [2e6, 2.4e6, 4e6, 10e6, 20e6] {
        for eight in [false, true] {
            let n = (rate * 2.0) as usize;
            let mut x = vec![Complex32::new(0.0, 0.0); n];
            Awgn::with_power(2e-4, 7).unwrap().process(&mut x);
            if eight {
                quant8(&mut x);
            }
            let mut rx = receiver(&[]);
            let r = run(&mut rx, rate, &x, &[65536]);
            assert!(r.frames.is_empty(), "{rate}");
            let d = &view(rx.results());
            assert_eq!(d["aircraft_count"], 0);
            let c = d["candidates"].as_f64().unwrap() / 2.0;
            assert!(
                c < if rate < 3e6 && eight { 80.0 } else { 5.0 } + 1e-9 || c < 80.0,
                "{rate} {eight}: {c} per second"
            );
        }
    }
}

#[test]
fn garble() {
    for rate in [4e6, 10e6] {
        let mut strong_ok = 0;
        let mut ph = 0;
        for k in 0..40 {
            let strong = es_frame(17, 5, 0x4840D6, me_ident('A', 3, "STRONG"));
            let weak = es_frame(17, 5, 0x500000 + k, me_ident('A', 3, "WEAK"));
            let t = 0.003;
            let dt = (10.0 + 90.0 * k as f64 / 40.0) * 1e-6;
            let bursts = vec![
                (Burst::mode_s(t, &strong, 0.2, 30e3, 0.3), Some(strong)),
                (Burst::mode_s(t + dt, &weak, 0.02, -60e3, 1.0), Some(weak)),
            ];
            let mut g = AdsbGenerator::script(rate, 3.5e6, bursts, k as u64);
            let mut x = vec![Complex32::new(0.0, 0.0); (0.005 * rate) as usize];
            g.generate(&mut x);
            add_noise(&mut x, rate, 40.0, k as u64);
            let txs = vec![
                Transmission {
                    time: t,
                    frame: strong,
                    amp: 0.2,
                    freq: 0.0,
                    overlapped: true,
                },
                Transmission {
                    time: t + dt,
                    frame: weak,
                    amp: 0.02,
                    freq: 0.0,
                    overlapped: true,
                },
            ];
            let (got, p) = check_off(&run(&mut receiver(&[]), rate, &x, &[65536]).frames, &txs);
            strong_ok += got[0] as usize;
            ph += p;
        }
        assert_eq!(ph, 0);
        assert!(strong_ok >= 38, "{rate}: {strong_ok}");
    }
    // A strong burst starting 8 to 88 µs into a weak one.
    let mut ok = 0;
    for k in 0..40 {
        let strong = es_frame(17, 5, 0x4840D6, me_ident('A', 3, "STRONG"));
        let weak = es_frame(17, 5, 0x500000 + k, me_ident('A', 3, "WEAK"));
        let t = 0.003;
        let dt = (8.0 + 80.0 * k as f64 / 40.0) * 1e-6;
        let bursts = vec![
            (Burst::mode_s(t, &weak, 0.01, 0.0, 0.0), Some(weak)),
            (Burst::mode_s(t + dt, &strong, 0.2, 20e3, 2.0), Some(strong)),
        ];
        let mut g = AdsbGenerator::script(4e6, 3.5e6, bursts, 100 + k as u64);
        let mut x = vec![Complex32::new(0.0, 0.0); 20000];
        g.generate(&mut x);
        add_noise(&mut x, 4e6, 40.0, 100 + k as u64);
        let txs = vec![Transmission {
            time: t + dt,
            frame: strong,
            amp: 0.2,
            freq: 0.0,
            overlapped: true,
        }];
        let (got, _) = check_off(&run(&mut receiver(&[]), 4e6, &x, &[65536]).frames, &txs);
        ok += got[0] as usize;
    }
    assert!(ok >= 34, "{ok}");
}

#[test]
fn mode_ac_replies_and_impulses_do_not_hurt() {
    for rate in [2.4e6, 4e6, 10e6] {
        let frames = mix(200, 0.0009, 8);
        let mut rng = Rng::new(9);
        let mut bursts: Vec<(Burst, Option<Frame>)> = frames
            .iter()
            .map(|(t, f)| {
                (
                    Burst::mode_s(*t, f, 0.1, 0.0, rng.range(0.0, std::f64::consts::TAU)),
                    Some(*f),
                )
            })
            .collect();
        let end = frames.last().unwrap().0 + 0.3;
        // 2000 Mode A/C replies per second and 3000 impulses per second, up to 20 dB above the signals.
        let mut t = 0.0;
        while t < end {
            t += rng.range(0.0, 1.0 / 1000.0);
            bursts.push((
                Burst::mode_ac(
                    t,
                    rng.below(8192) as u32,
                    false,
                    rng.range(0.02, 0.3) as f32,
                    0.0,
                    0.0,
                ),
                None,
            ));
        }
        let mut t = 0.0;
        while t < end {
            t += rng.range(0.0, 1.0 / 1500.0);
            bursts.push((
                Burst::impulse(
                    t,
                    rng.range(0.1e-6, 3e-6),
                    rng.range(0.01, 0.9) as f32,
                    0.0,
                    0.0,
                ),
                None,
            ));
        }
        let spans: Vec<(f64, f64)> = bursts
            .iter()
            .filter(|b| b.1.is_none())
            .map(|b| (b.0.start, b.0.end))
            .collect();
        let txs: Vec<Transmission> = frames
            .iter()
            .map(|(t, f)| {
                let end = t + (8.0 + f.bits as f64) * 1e-6;
                let overlapped = spans.iter().any(|&(a, b)| a < end + 1e-6 && b > t - 1e-6);
                Transmission {
                    time: *t,
                    frame: *f,
                    amp: 0.1,
                    freq: 0.0,
                    overlapped,
                }
            })
            .collect();
        let mut g = AdsbGenerator::script(rate, (0.875 * rate).clamp(1.75e6, 5e6), bursts, 9);
        let mut x = vec![Complex32::new(0.0, 0.0); ((end + 0.002) * rate) as usize];
        g.generate(&mut x);
        add_noise(&mut x, rate, 30.0, 9);
        let mut rx = receiver(&[]);
        let r = run(&mut rx, rate, &x, &[65536]);
        let (got, ph) = check_off(&r.frames, &txs);
        let untouched = txs.iter().filter(|t| !t.overlapped).count();
        let ok = txs
            .iter()
            .zip(&got)
            .filter(|(t, g)| !t.overlapped && **g)
            .count();
        eprintln!("{rate}: {ok} of {untouched} untouched frames with Mode A/C and impulses");
        assert_eq!(ph, 0);
        assert!(
            ok as f64 >= 0.99 * untouched as f64,
            "{rate}: {ok} of {untouched}"
        );
    }
}

#[test]
fn gain_steps_clipping_dropout_and_bad_samples() {
    let frames = mix(300, 0.0008, 10);
    let (x, txs) = render(4e6, &frames, 0.05, 34.0, 3.5e6, 10, 0.0);
    // Gain steps of +-20 dB at 150 ms.
    for g in [10f32, 0.1] {
        let c: Vec<Complex32> = x
            .iter()
            .enumerate()
            .map(|(i, v)| if i as f64 / 4e6 >= 0.15 { v * g } else { *v })
            .collect();
        let (n, ph, _) = decode(4e6, &c, &txs, &[65536]);
        assert_eq!(ph, 0);
        assert!(n >= 296, "gain {g}: {n}");
    }
    // Clipped input 5x and 40x too strong.
    for k in [5.0f32, 40.0] {
        let c: Vec<Complex32> = x
            .iter()
            .map(|v| Complex32::new((v.re * k).clamp(-1.0, 1.0), (v.im * k).clamp(-1.0, 1.0)))
            .collect();
        let (n, ph, _) = decode(4e6, &c, &txs, &[65536]);
        assert_eq!(ph, 0);
        assert!(n >= 294, "clip {k}: {n}");
    }
    // A 5 ms dropout at 100.5 ms.
    let c: Vec<Complex32> = x
        .iter()
        .enumerate()
        .map(|(i, v)| {
            if (0.1005..0.1055).contains(&(i as f64 / 4e6)) {
                Complex32::new(0.0, 0.0)
            } else {
                *v
            }
        })
        .collect();
    let (n, ph, r) = decode(4e6, &c, &txs, &[65536]);
    assert_eq!(ph, 0);
    let inside = txs
        .iter()
        .filter(|t| (0.1005..0.1055).contains(&t.time))
        .count();
    assert!(n + inside + 4 >= 300, "dropout: {n}");
    assert!(r.reports.last().unwrap()["failed"].as_u64().unwrap() < 400);
    // NaN, infinity and 1e30.
    let mut c = x.clone();
    for (i, v) in c.iter_mut().enumerate() {
        let t = i as f64 / 4e6;
        if (0.05..0.0505).contains(&t) {
            *v = Complex32::new(f32::NAN, f32::NAN);
        } else if (0.08..0.0801).contains(&t) {
            *v = Complex32::new(f32::INFINITY, 0.0);
        } else if (0.12..0.1201).contains(&t) {
            *v = Complex32::new(1e30, -1e30);
        } else if i % 997 == 0 {
            *v = Complex32::new(f32::NAN, 0.0);
        }
    }
    let (n, ph, r) = decode(4e6, &c, &txs, &[65536]);
    assert_eq!(ph, 0);
    assert!(n >= 270, "bad samples: {n}");
    let noise = r.reports.last().unwrap()["noise_dbfs"].as_f64().unwrap();
    assert!(noise.is_finite() && noise < -35.0);
}

#[test]
fn a_reset_keeps_the_table_and_report_numbers_grow() {
    let frames = mix(400, 0.001, 11);
    let (x, txs) = render(4e6, &frames, 0.1, 30.0, 3.5e6, 11, 0.0);
    let cut = (0.2505 * 4e6) as usize;
    let mut rx = receiver(&[]);
    let a = run(&mut rx, 4e6, &x[..cut], &[65536]);
    let before = view(rx.results())["aircraft_count"].clone();
    let r1 = view(rx.results())["report"].as_u64().unwrap_or(0);
    rx.configure(&InputInfo::new(4e6, 1090e6, 1090e6).recording())
        .unwrap();
    let b = feed(&mut rx, 4e6, &x[cut..], &[65536]);
    assert_eq!(view(rx.results())["aircraft_count"], before);
    assert!(view(rx.results())["report"].as_u64().unwrap() > r1);
    let reps: Vec<u64> = b
        .reports
        .iter()
        .map(|r| r["report"].as_u64().unwrap())
        .collect();
    assert!(reps.windows(2).all(|w| w[1] > w[0]));
    // The second part's times continue from the first (shifted by the cut): check both parts off.
    let shift = cut as f64 / 4e6;
    let (got_a, ph_a) = check_off(&a.frames, &txs);
    let txs_b: Vec<Transmission> = txs
        .iter()
        .map(|t| Transmission {
            time: t.time,
            ..t.clone()
        })
        .collect();
    let fb: Vec<_> = b
        .frames
        .iter()
        .map(|f| sdrkit_stack_adsb::checks::Accepted {
            time: f.time,
            ..f.clone()
        })
        .collect();
    let (got_b, ph_b) = check_off(&fb, &txs_b);
    assert_eq!(ph_a + ph_b, 0, "{shift}");
    let n = got_a.iter().zip(&got_b).filter(|(a, b)| **a || **b).count();
    assert!(n >= 397, "{n}");
}

#[test]
fn below_2_msps_is_not_ready() {
    let mut rx = receiver(&[]);
    assert!(rx
        .configure(&InputInfo::new(1.5e6, 1090e6, 1090e6))
        .is_err());
    assert!(!rx.ready());
    let mut out = sdrkit_core::Output::default();
    rx.feed(&[Complex32::new(0.1, 0.0); 1000], &mut out);
    assert!(view(rx.results())["aircraft_count"].as_u64().unwrap_or(0) == 0);
}

#[test]
fn the_airspace() {
    for rate in [4e6, 2.4e6, 10e6] {
        let opts = AirspaceOptions::default();
        let mut g = AdsbGenerator::airspace(rate, opts.clone(), 3);
        g.keep_log = true;
        let mut x = vec![Complex32::new(0.0, 0.0); (rate * 20.0) as usize];
        g.generate(&mut x);
        add_noise(&mut x, rate, 30.0, 3);
        quant8(&mut x);
        let mut rx = receiver(&[]);
        at_station(&mut rx, CENTRE.0, CENTRE.1);
        let r = run(&mut rx, rate, &x, &[65536]);
        let (got, ph) = check_off(&r.frames, &g.log);
        let clean = g
            .log
            .iter()
            .zip(&got)
            .filter(|(t, _)| !t.overlapped)
            .count();
        let clean_ok = g
            .log
            .iter()
            .zip(&got)
            .filter(|(t, g)| !t.overlapped && **g)
            .count();
        eprintln!("{rate}: {clean_ok} of {clean} clean transmissions");
        assert_eq!(ph, 0);
        assert!(
            clean_ok as f64 >= 0.995 * clean as f64,
            "{rate}: {clean_ok} of {clean}"
        );
        let d = &view(rx.results());
        assert_eq!(d["state"], 2);
        assert_eq!(d["aircraft_count"], 12, "{rate}");
        assert_eq!(d["with_position"], 12, "{rate}");
        let air = g.airspace_ref().unwrap();
        for p in &air.planes {
            let icao = format!("{:06X}", p.address);
            let a = d["aircraft"]
                .as_array()
                .unwrap()
                .iter()
                .find(|a| a["icao"] == icao)
                .unwrap_or_else(|| panic!("{icao}"));
            assert_eq!(a["callsign"], p.callsign.as_str(), "{icao}");
            assert_eq!(a["squawk"], codes::squawk_text(p.squawk).as_str(), "{icao}");
            assert!(
                (a["lat"].as_f64().unwrap() - p.lat).abs() < 0.03
                    && (a["lon"].as_f64().unwrap() - p.lon).abs() < 0.03,
                "{icao}"
            );
            assert!(
                (a["baro_alt_ft"].as_f64().unwrap() - p.alt_sent() as f64).abs() <= 250.0,
                "{icao}"
            );
            assert_eq!(a["on_ground"], p.ground, "{icao}");
            let want_kind = if p.ground || p.address % 11 != 0 {
                "ground"
            } else {
                "ias"
            };
            assert_eq!(a["speed_kind"], want_kind, "{icao}");
            let want = if want_kind == "ias" { p.ias() } else { p.gs };
            assert!(
                (a["speed_kt"].as_f64().unwrap() - want).abs() <= 3.0 || p.ground,
                "{icao}"
            );
            assert!(trail_points(&a["trail"]) >= 3, "{icao}");
            let dd = sdrkit_stack_adsb::geo::distance_nm(25.25, 55.36, p.lat, p.lon);
            assert!(
                (a["distance_nm"].as_f64().unwrap() - dd).abs() < 2.0,
                "{icao}"
            );
        }
    }
}

#[test]
fn settings_take_effect() {
    let frames = mix(200, 0.001, 12);
    let (x, txs) = render(4e6, &frames, 0.1, 30.0, 3.5e6, 12, 0.0);
    // Expiry: a short expiry drops the aircraft at the end of a quiet second.
    let mut quiet = x.clone();
    quiet.extend(std::iter::repeat_n(
        Complex32::new(0.0, 0.0),
        (7.0 * 4e6) as usize,
    ));
    let mut rx = receiver(&[("expiry_s", json!(5.0)), ("drop_s", json!(5.0))]);
    run(&mut rx, 4e6, &quiet, &[65536]);
    assert_eq!(view(rx.results())["aircraft_count"], 0);
    // Effort 0 still decodes strong frames.
    let mut rx = receiver(&[("effort", json!("0"))]);
    let r = run(&mut rx, 4e6, &x, &[65536]);
    let (got, _) = check_off(&r.frames, &txs);
    assert!(got.iter().filter(|&&g| g).count() >= 190);
    assert_eq!(view(rx.results())["effort"], 0);
    // Clear empties the table.
    command(&mut rx, "clear");
    let mut out = sdrkit_core::Output::default();
    rx.feed(&vec![Complex32::new(0.0, 0.0); 1_000_000], &mut out);
    assert_eq!(view(rx.results())["aircraft_count"], 0);
    // An unknown setting is refused.
    assert!(set(&mut rx, "no_such_setting", Value::Bool(true)).is_err());
}

#[test]
fn events_are_raised() {
    let frames = mix(100, 0.001, 13);
    let (x, _) = render(4e6, &frames, 0.1, 30.0, 3.5e6, 13, 0.0);
    let mut rx = receiver(&[]);
    let r = run(&mut rx, 4e6, &x, &[65536]);
    let new = r
        .output
        .events
        .iter()
        .filter(|e| {
            e.is(&sdrkit_stack_adsb::typed::AIRCRAFT_SEEN)
                && e.at(&sdrkit_stack_adsb::typed::AircraftSeen::EVENT)
                    == Some(sdrkit_stack_adsb::typed::AircraftChange::New)
        })
        .count();
    assert_eq!(new, 16);
}

#[test]
fn settings_and_telemetry_from_another_thread() {
    use std::sync::{Arc, Mutex};
    let frames = mix(300, 0.001, 14);
    let (x, _) = render(4e6, &frames, 0.1, 30.0, 3.5e6, 14, 0.0);
    let rx = Arc::new(Mutex::new(receiver(&[])));
    rx.lock()
        .unwrap()
        .configure(&InputInfo::new(4e6, 1090e6, 1090e6))
        .unwrap();
    let r2 = rx.clone();
    let reader = std::thread::spawn(move || {
        let mut reports = std::collections::BTreeSet::new();
        for k in 0..200 {
            {
                let mut g = r2.lock().unwrap();
                reports.insert(view(g.results())["report"].as_u64().unwrap_or(0));
                let c = if k % 2 == 0 {
                    sdrkit_stack_adsb::checks::Correction::One
                } else {
                    sdrkit_stack_adsb::checks::Correction::Three
                };
                let _ = set(
                    &mut *g,
                    "correction",
                    Value::Choice(sdrkit_core::Choice::index(c)),
                );
            }
            std::thread::sleep(std::time::Duration::from_micros(200));
        }
        reports.len()
    });
    for _ in 0..10 {
        for c in x.chunks(20000) {
            let mut out = sdrkit_core::Output::default();
            rx.lock().unwrap().feed(c, &mut out);
        }
    }
    let _ = reader.join().unwrap();
    assert!(
        view(rx.lock().unwrap().results())["report"]
            .as_u64()
            .unwrap()
            > 10
    );
}

#[test]
fn generator_options() {
    // The emergency demonstration, radar replies off, the rate multiplier and no aircraft.
    let gen = |opts: AirspaceOptions, secs: f64| {
        let rate = 2e6;
        let mut g = AdsbGenerator::airspace(rate, opts, 3);
        g.keep_log = true;
        let mut x = vec![Complex32::new(0.0, 0.0); (rate * secs) as usize];
        g.generate(&mut x);
        (g.log.clone(), x)
    };
    let tc = |f: &Frame| (f.bytes[4] >> 3) as u32;
    let (base, _) = gen(AirspaceOptions::default(), 6.0);
    assert!(base
        .iter()
        .all(|t| !(t.frame.df() == 17 && tc(&t.frame) == 28)));
    assert!(base.iter().any(|t| t.frame.df() == 20));
    let (em, _) = gen(
        AirspaceOptions {
            emergency: true,
            ..Default::default()
        },
        6.0,
    );
    let status: Vec<&Transmission> = em
        .iter()
        .filter(|t| t.frame.df() == 17 && tc(&t.frame) == 28)
        .collect();
    assert!(status.len() >= 3, "{} TC 28 messages", status.len());
    let who = status[0].frame.aa();
    assert!(status.iter().all(|t| t.frame.aa() == who));
    match sdrkit_stack_adsb::message::es(&status[0].frame) {
        Some(sdrkit_stack_adsb::message::Es::Status {
            squawk: Some(sq), ..
        }) => assert_eq!(codes::squawk_text(sq), "7700"),
        e => panic!("{e:?}"),
    }
    let (quiet, _) = gen(
        AirspaceOptions {
            replies: false,
            ..Default::default()
        },
        6.0,
    );
    assert!(quiet.iter().all(|t| matches!(t.frame.df(), 11 | 17)));
    let (fast, _) = gen(
        AirspaceOptions {
            multiplier: 2.0,
            ..Default::default()
        },
        6.0,
    );
    let ratio = fast.len() as f64 / base.len() as f64;
    assert!((1.7..2.3).contains(&ratio), "multiplier 2 gives {ratio}");
    let (none, x) = gen(
        AirspaceOptions {
            aircraft: 0,
            ..Default::default()
        },
        0.5,
    );
    assert!(none.is_empty());
    // The clean signal: nothing to send is silence (the framework adds the noise).
    assert!(x.iter().all(|v| v.norm_sqr() == 0.0));
}
