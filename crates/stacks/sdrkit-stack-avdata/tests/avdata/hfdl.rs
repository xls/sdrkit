//! HFDL: the burst pair at every rate through the receiver, with noise, offsets, clock error and the HF channels of sdrkit-algo-sim;
//! the traffic scene. The recording. Noise only.

use crate::common::{self, int, num, rows, text, Row};
use sdrkit_algo_sim::chain::ChainConfig;
use sdrkit_algo_sim::fading::profiles::Watterson;
use sdrkit_algo_sim::fading::{Doppler, Profile, Tap};
use sdrkit_algo_sim::noise::NoiseLevel;
use sdrkit_core::{Complex32, Generator, Receiver};
use sdrkit_stack_avdata::hfdl::generator::test_pair;
use sdrkit_stack_avdata::hfdl::{self, GenParams, HfdlGenerator};
use sdrkit_stack_avdata::typed::{HFDL_AIRCRAFT, HFDL_CHANNELS, HFDL_MESSAGES, HFDL_STATIONS};

const DIAL: f64 = 8.927e6;

fn rx_set(rate: f64, centre: f64, settings: &[(&str, &str)]) -> Box<dyn Receiver> {
    common::receiver(&hfdl::DESCRIPTOR, rate, centre, centre, settings)
}

fn rx(rate: f64, centre: f64, channels: &str) -> Box<dyn Receiver> {
    rx_set(rate, centre, &[("channels", channels)])
}

/// The impairments: Es/N0 `snr` (no noise when `None`), a carrier offset, and the rest of `cfg`.
fn impair(snr: Option<f64>, offset_hz: f64, cfg: ChainConfig) -> ChainConfig {
    ChainConfig {
        noise: snr.map_or(NoiseLevel::Off, |s| NoiseLevel::EsN0Db {
            esn0_db: s,
            symbol_rate_hz: 1800.0,
        }),
        offset_hz,
        ..cfg
    }
}

fn run_with(
    p: GenParams,
    cfg: ChainConfig,
    secs: f64,
    settings: &[(&str, &str)],
) -> Box<dyn Receiver> {
    let (rate, centre) = (p.rate_hz, p.centre_hz);
    let mut g = HfdlGenerator::new(p);
    let mut ch = common::chain(&g, cfg);
    let mut r = rx_set(rate, centre, settings);
    common::stream(&mut g, &mut ch, r.as_mut(), secs, 16_384);
    r
}

fn run(p: GenParams, cfg: ChainConfig, secs: f64, channels: &str) -> Box<dyn Receiver> {
    run_with(p, cfg, secs, &[("channels", channels)])
}

fn messages(r: &dyn Receiver) -> Vec<Row> {
    rows(r.results(), &HFDL_MESSAGES)
}

fn acars_ok(ms: &[Row]) -> usize {
    ms.iter()
        .filter(|m| text(m, "hfnpdu") == "ACARS" && common::flag(m, "acars_ok"))
        .count()
}

/// One squitter and one ACARS message with a good check. The position within 0.01 degree (not at 300 bit/s). The offset within
/// 3 Hz.
fn decoded(r: &dyn Receiver, idx: usize, offset: f64) -> Result<(), String> {
    let ms = messages(r);
    let sq = ms.iter().filter(|m| text(m, "kind") == "SPDU").count();
    let ac = acars_ok(&ms);
    if sq != 1 || ac != 1 {
        return Err(format!(
            "squitters {sq} acars {ac} lost (preamble, short, squitter, header, LPDU, type) {:?}",
            [
                "lost_preamble",
                "lost_short",
                "lost_squitter_fcs",
                "lost_header_fcs",
                "lost_lpdu_fcs",
                "lost_lpdu_type"
            ]
            .map(|k| common::field(r.results(), k))
        ));
    }
    if !idx.is_multiple_of(4) {
        let (lat, lon) = ms
            .iter()
            .find_map(|m| common::position(m, "position"))
            .ok_or("no position")?;
        if (lat - 51.5).abs() > 0.01 || (lon + 30.25).abs() > 0.01 {
            return Err(format!("position {lat} {lon}"));
        }
    }
    for m in &ms {
        if (num(m, "offset_hz") - offset).abs() > 3.0 {
            return Err(format!("offset {}", num(m, "offset_hz")));
        }
    }
    Ok(())
}

fn pair(idx: usize) -> GenParams {
    GenParams {
        rate_hz: 48e3,
        centre_hz: 8.92e6,
        bursts: test_pair(0.5, DIAL, idx),
        ..Default::default()
    }
}

fn secs(idx: usize) -> f64 {
    if idx >= 4 {
        9.0
    } else {
        6.0
    }
}

#[test]
fn every_rate_clean() {
    for idx in 0..8 {
        let r = run(
            pair(idx),
            impair(Some(40.0), 0.0, ChainConfig::default()),
            secs(idx),
            "8927",
        );
        decoded(r.as_ref(), idx, 0.0).unwrap_or_else(|e| panic!("M1 {idx}: {e}"));
    }
}

#[test]
fn offsets_and_clock() {
    for idx in [1usize, 2, 3, 5] {
        for (off, ppm) in [
            (200.0, 0.0),
            (-200.0, 0.0),
            (150.0, 100.0),
            (-150.0, -100.0),
        ] {
            let cfg = ChainConfig {
                clock_ppm: ppm,
                ..ChainConfig::default()
            };
            let r = run(pair(idx), impair(Some(25.0), off, cfg), secs(idx), "8927");
            decoded(r.as_ref(), idx, off).unwrap_or_else(|e| panic!("M1 {idx} {off} {ppm}: {e}"));
        }
    }
}

#[test]
fn noise() {
    for (idx, snr, off) in [
        (1usize, 7.0, 20.0),
        (0, 4.0, -20.0),
        (2, 11.0, 40.0),
        (3, 16.0, -40.0),
    ] {
        let r = run(
            pair(idx),
            impair(Some(snr), off, ChainConfig::default()),
            secs(idx),
            "8927",
        );
        decoded(r.as_ref(), idx, off).unwrap_or_else(|e| panic!("M1 {idx} at {snr} dB: {e}"));
    }
}

#[test]
fn two_static_paths() {
    for (idx, dly, g, off) in [(3usize, 0.001, -3.0, 70.0), (2, 0.0025, -1.0, -70.0)] {
        let cfg = ChainConfig {
            fading: Profile::Taps(vec![
                Tap::new(0.0, 0.0, Doppler::Static),
                Tap::new(dly, g, Doppler::Static),
            ]),
            ..ChainConfig::default()
        };
        let r = run(pair(idx), impair(Some(20.0), off, cfg), secs(idx), "8927");
        decoded(r.as_ref(), idx, off).unwrap_or_else(|e| panic!("M1 {idx} two paths: {e}"));
    }
}

#[test]
fn noise_only() {
    let p = GenParams {
        rate_hz: 48e3,
        centre_hz: 8.92e6,
        bursts: vec![],
        stations: vec![],
        aircraft: 1,
        duration_s: 0.0,
        ..Default::default()
    };
    let cfg = ChainConfig {
        noise: NoiseLevel::FloorDbfs(-30.0),
        ..ChainConfig::default()
    };
    let r = run(p, cfg, 20.0, "8927");
    assert!(messages(r.as_ref()).is_empty());
}

#[test]
fn the_recording_decodes_as_expected() {
    let rec = sdrkit_dev_test::read_recording("avdata/hfdl_48k").expect("the HFDL recording");
    let mut r = rx(rec.rate_hz, rec.centre_hz, "8927");
    common::run(r.as_mut(), &rec.samples, 65_536);
    let res = r.results();
    let ms: Vec<Row> = messages(r.as_ref()).into_iter().rev().collect();
    assert_eq!(ms.len(), 4);
    let sq = &ms[0];
    assert_eq!(text(sq, "kind"), "SPDU");
    assert_eq!(int(sq, "gs_id"), 1);
    assert_eq!(
        text(sq, "summary"),
        "Squitter San Francisco, frame 777, 2 frequencies in use"
    );
    let detail = text(sq, "detail");
    assert!(detail.contains("06:54:24"));
    assert!(detail.contains("station 7 Shannon (UTC): 11384 10081 kHz"));
    assert!(detail.contains("frequencies in use: 21934 8927 kHz"));
    assert!(detail.contains("system table version 50"));
    assert_eq!(int(sq, "bps"), 600);
    let perf = ms
        .iter()
        .find(|m| text(m, "hfnpdu") == "Performance data")
        .unwrap();
    assert_eq!(text(perf, "flight"), "TEST01");
    assert!(!common::flag(perf, "uplink") && int(perf, "gs_id") == 1 && int(perf, "ac_id") == 23);
    let (lat, lon) = common::position(perf, "position").unwrap();
    assert!((lat - 51.50013).abs() < 2e-4 && (lon + 30.25017).abs() < 2e-4);
    assert!(text(perf, "detail").contains("11:06:40 UTC"));
    let ac: Vec<_> = ms.iter().filter(|m| text(m, "hfnpdu") == "ACARS").collect();
    assert_eq!(ac.len(), 2);
    assert!(ac.iter().all(|m| common::flag(m, "acars_ok")));
    let a0 = ac[0];
    assert_eq!(
        (
            text(a0, "text"),
            text(a0, "registration"),
            text(a0, "label"),
            text(a0, "block_id")
        ),
        ("SYNTHETIC HFDL TEST MESSAGE", "TST001", "H1", "2")
    );
    assert_eq!((text(a0, "msn"), text(a0, "flight")), ("M01", "TEST01"));
    assert!(common::flag(ac[1], "uplink"));
    assert_eq!(
        (text(ac[1], "text"), text(ac[1], "block_id")),
        ("- #M1/TEST UPLINK OK", "A")
    );
    for m in &ms {
        assert!(
            (num(m, "offset_hz") - 40.0).abs() <= 3.0,
            "{}",
            num(m, "offset_hz")
        );
    }
    // Burst start times (the prekey) within 0.05 s.
    for (m, t) in ms.iter().zip([0.2, 2.8, 2.8, 5.4]) {
        assert!(
            (num(m, "signal_s") - t).abs() <= 0.05,
            "{} vs {t}",
            num(m, "signal_s")
        );
    }
    assert_eq!(int(perf, "bps"), 1200);
    assert_eq!(int(ac[1], "bps"), 1800);
    let f = |k: &str| common::field(res, k);
    assert_eq!(
        (
            f("pdus"),
            f("bursts_bad"),
            f("squitters"),
            f("lpdus"),
            f("acars"),
            f("positions")
        ),
        (3.0, 0.0, 1.0, 3.0, 2.0, 1.0)
    );
    assert_eq!(
        (
            f("bursts_300"),
            f("bursts_600"),
            f("bursts_1200"),
            f("bursts_1800")
        ),
        (0.0, 1.0, 1.0, 1.0)
    );
    let ch = rows(res, &HFDL_CHANNELS);
    assert_eq!(ch.len(), 1);
    assert_eq!(
        (int(&ch[0], "good"), int(&ch[0], "bad"), int(&ch[0], "bps")),
        (3, 0, 1800)
    );
    let gs = rows(res, &HFDL_STATIONS);
    let gs = gs.iter().find(|s| int(s, "gs_id") == 1).expect("station 1");
    assert_eq!((int(gs, "squitters"), int(gs, "frame_index")), (1, 777));
    let a = &rows(res, &HFDL_AIRCRAFT)[0];
    assert_eq!(
        text(a, "key"),
        "ground station 1, aircraft id 23 (address not known)"
    );
    assert_eq!(
        (
            text(a, "registration"),
            text(a, "flight"),
            int(a, "gs_id"),
            int(a, "count"),
            common::flag(a, "logged_on")
        ),
        ("TST001", "TEST01", 1, 3, false)
    );
}

/// How many of `seeds` burst pairs at M1 `idx` give their ACARS message, through `fading`, at `snr` with `offset(seed)` and
/// `ppm`.
fn tally(
    idx: usize,
    snr: f64,
    seeds: u64,
    fading: Profile,
    ppm: f64,
    offset: &dyn Fn(u64) -> f64,
) -> usize {
    (0..seeds)
        .filter(|&s| {
            let mut p = pair(idx);
            p.seed = 100 + s;
            let cfg = ChainConfig {
                seed: 100 + s,
                fading: fading.clone(),
                clock_ppm: ppm,
                ..ChainConfig::default()
            };
            let r = run(p, impair(Some(snr), offset(s), cfg), secs(idx), "8927");
            acars_ok(&messages(r.as_ref())) >= 1
        })
        .count()
}

#[test]
fn offset_and_timing() {
    for idx in 0..8 {
        for (off, delay) in [(100.0, 0.37), (-100.0, 13.6)] {
            let cfg = ChainConfig {
                fading: Profile::Taps(vec![Tap::new(delay / 48e3, 0.0, Doppler::Static)]),
                ..ChainConfig::default()
            };
            let r = run(pair(idx), impair(Some(25.0), off, cfg), secs(idx), "8927");
            decoded(r.as_ref(), idx, off)
                .unwrap_or_else(|e| panic!("M1 {idx} {off} Hz delay {delay}: {e}"));
        }
    }
}

#[test]
fn noise_and_offset() {
    for (idx, snr, off) in [
        (1usize, 10.0, 180.0),
        (4, 10.0, -180.0),
        (2, 13.0, 120.0),
        (7, 15.0, -120.0),
    ] {
        let r = run(
            pair(idx),
            impair(Some(snr), off, ChainConfig::default()),
            secs(idx),
            "8927",
        );
        decoded(r.as_ref(), idx, off).unwrap_or_else(|e| panic!("M1 {idx} {snr} dB {off} Hz: {e}"));
    }
}

#[test]
fn weak_bursts() {
    let off = |s: u64| [-40.0, 0.0, 40.0][(s % 3) as usize];
    for (idx, snr, want) in [
        (1usize, 0.0, 5usize),
        (0, -3.0, 8),
        (1, 4.0, 12),
        (0, 2.0, 12),
    ] {
        let n = tally(idx, snr, 12, Profile::None, 0.0, &off);
        assert!(n >= want, "M1 {idx} at {snr} dB: {n} of 12");
    }
}

#[test]
fn weak_with_a_clock_error() {
    let n = tally(1, -1.0, 12, Profile::None, 100.0, &|s| {
        [-40.0, 0.0, 40.0][(s % 3) as usize]
    });
    assert!(n >= 1, "{n} of 12");
}

#[test]
fn ccir_good() {
    for idx in 0..8 {
        let n = tally(
            idx,
            25.0,
            4,
            Profile::Watterson(Watterson::Good),
            0.0,
            &|_| 0.0,
        );
        assert!(n >= 3, "M1 {idx}: {n} of 4");
    }
}

#[test]
fn ccir_moderate() {
    for idx in 0..8 {
        let n = tally(
            idx,
            25.0,
            8,
            Profile::Watterson(Watterson::Moderate),
            0.0,
            &|_| 0.0,
        );
        let want = match idx % 4 {
            0 => 7,
            3 => 4,
            _ => 5,
        };
        assert!(n >= want, "M1 {idx}: {n} of 8");
    }
}

#[test]
fn ccir_poor() {
    for idx in [0usize, 1, 2, 5] {
        let n = tally(
            idx,
            25.0,
            8,
            Profile::Watterson(Watterson::Poor),
            0.0,
            &|_| 0.0,
        );
        let want = if idx == 5 { 3 } else { 4 };
        assert!(n >= want, "M1 {idx}: {n} of 8");
    }
}

#[test]
fn three_channels_at_once() {
    let dials = [8.927e6, 8.942e6, 8.977e6];
    let mut bursts = Vec::new();
    for (k, &dial) in dials.iter().enumerate() {
        let mut b = test_pair(0.5 + 0.4 * k as f64, dial, 2);
        for t in b.iter_mut() {
            t.level_db = -3.0 * k as f64;
        }
        bursts.extend(b);
    }
    let p = GenParams {
        rate_hz: 96e3,
        centre_hz: 8.95e6,
        bursts,
        ..Default::default()
    };
    let r = run(
        p,
        impair(Some(20.0), 0.0, ChainConfig::default()),
        7.0,
        "8927 8942 8977",
    );
    let ms = messages(r.as_ref());
    for dial in dials {
        let on = |m: &&Row| num(m, "dial") == dial;
        let sq = ms
            .iter()
            .filter(on)
            .filter(|m| text(m, "kind") == "SPDU")
            .count();
        let ac = acars_ok(&ms.iter().filter(on).cloned().collect::<Vec<_>>());
        assert!(sq == 1 && ac == 1, "{dial}: squitters {sq} acars {ac}");
    }
    let res = r.results();
    assert!(rows(res, &HFDL_AIRCRAFT)
        .iter()
        .any(|a| common::position(a, "position").is_some()));
    assert!(rows(res, &HFDL_STATIONS)
        .iter()
        .any(|s| int(s, "gs_id") == 1 && common::flag(s, "heard_direct")));
}

#[test]
fn the_traffic_scene() {
    let p = GenParams {
        rate_hz: 96e3,
        centre_hz: 8.94e6,
        aircraft: 4,
        stations: vec![(1, 8.927e6), (7, 8.942e6), (2, 8.936e6)],
        rate_factor: 4.0,
        duration_s: 100.0,
        ..Default::default()
    };
    let g = HfdlGenerator::new(p.clone());
    let sent = g
        .sent
        .iter()
        .filter(|b| b.time_s + hfdl::phy::rate(b.rate).total_symbols() as f64 / 1800.0 < 99.0)
        .count();
    let r = run(
        p,
        impair(Some(25.0), 50.0, ChainConfig::default()),
        100.0,
        "8927 8942 8936",
    );
    let res = r.results();
    let ch = rows(res, &HFDL_CHANNELS);
    let good: i64 = ch.iter().map(|c| int(c, "good")).sum();
    let bad: i64 = ch.iter().map(|c| int(c, "bad")).sum();
    assert_eq!(bad, 0, "bad bursts");
    assert!(good as usize >= sent, "{good} of {sent}");
    for k in ["bursts_300", "bursts_600", "bursts_1200", "bursts_1800"] {
        assert!(common::field(res, k) > 0.0, "{k}");
    }
    let logged = rows(res, &HFDL_AIRCRAFT)
        .iter()
        .filter(|a| common::flag(a, "logged_on") && !text(a, "address").is_empty())
        .count();
    assert!(logged >= 3, "{logged} logged on");
    assert!(common::field(res, "positions") >= 3.0 && common::field(res, "acars") >= 3.0);
}

fn whole_table() -> String {
    hfdl::stations::all_khz()
        .iter()
        .map(|k| k.to_string())
        .collect::<Vec<_>>()
        .join(" ")
}

#[test]
fn a_far_channel_beside_spurs() {
    let dial = 8.942e6;
    for centre in [9.343e6, 8.942e6, 8.9e6] {
        let rate = 2e6;
        let p = GenParams {
            rate_hz: rate,
            centre_hz: centre,
            bursts: test_pair(0.5, dial, 1),
            ..Default::default()
        };
        let mut g = HfdlGenerator::new(p);
        let mut ch = common::chain(&g, impair(Some(20.0), 30.0, ChainConfig::default()));
        let mut r = rx_set(rate, centre, &[("channels", &whole_table())]);
        let mut out = sdrkit_core::Output::default();
        let mut buf = vec![Complex32::default(); 65_536];
        let mut y = Vec::new();
        let total = (6.0 * rate) as usize;
        let am = std::f64::consts::TAU * (dial + 15e3 - centre) / rate;
        let mut n = 0usize;
        while n < total {
            let k = buf.len().min(total - n);
            g.generate(&mut buf[..k]);
            y.clear();
            ch.process(&buf[..k], &mut y);
            for (i, v) in y.iter_mut().enumerate() {
                let t = (n + i) as f64;
                // A strong DC spur and a strong AM carrier 15 kHz above the dial.
                let env = 0.3 * (1.0 + 0.5 * (std::f64::consts::TAU * 1e3 * t / rate).sin());
                *v += Complex32::new(0.2, -0.1)
                    + Complex32::new((env * (am * t).cos()) as f32, (env * (am * t).sin()) as f32);
            }
            out.clear();
            r.feed(&y, &mut out);
            n += k;
        }
        let res = r.results();
        assert!(
            rows(res, &HFDL_CHANNELS)
                .iter()
                .any(|c| num(c, "dial") == dial && common::flag(c, "in_band")),
            "centre {centre}: 8.942 not a channel"
        );
        let ms = messages(r.as_ref());
        let acars: Vec<_> = ms
            .iter()
            .filter(|m| text(m, "hfnpdu") == "ACARS" && common::flag(m, "acars_ok"))
            .collect();
        assert_eq!(acars.len(), 1, "centre {centre}");
        assert_eq!(num(acars[0], "dial"), dial);
        assert!(
            (num(acars[0], "offset_hz") - 30.0).abs() <= 5.0,
            "centre {centre}: offset {}",
            num(acars[0], "offset_hz")
        );
    }
}

#[test]
fn the_shared_channeliser() {
    let rate = 32e6;
    let centre = 12e6;
    let dials = [4.66e6, 11.327e6, 21.997e6];
    for (snr, slack) in [(12.0, 0usize), (3.0, 1)] {
        let bursts: Vec<_> = dials
            .iter()
            .enumerate()
            .map(|(k, &d)| {
                let mut b = test_pair(0.0, d, 2).pop().unwrap();
                b.time_s = 0.4 + 0.05 * k as f64;
                b
            })
            .collect();
        let p = GenParams {
            rate_hz: rate,
            centre_hz: centre,
            bursts,
            seed: 16,
            ..Default::default()
        };
        let mut g = HfdlGenerator::new(p);
        let cfg = ChainConfig {
            seed: 16,
            ..impair(Some(snr), 0.0, ChainConfig::default())
        };
        let mut ch = common::chain(&g, cfg);
        let list = dials
            .iter()
            .map(|d| format!("{}", d / 1e3))
            .collect::<Vec<_>>()
            .join(" ");
        let mut shared = rx_set(rate, centre, &[("channels", &whole_table())]);
        let mut own = rx_set(
            rate,
            centre,
            &[("channels", &list), ("shared_channeliser", "0")],
        );
        let mut out = sdrkit_core::Output::default();
        let mut buf = vec![Complex32::default(); 1 << 20];
        let mut y = Vec::new();
        let total = (3.0 * rate) as usize;
        let mut n = 0;
        while n < total {
            let k = buf.len().min(total - n);
            g.generate(&mut buf[..k]);
            y.clear();
            ch.process(&buf[..k], &mut y);
            out.clear();
            shared.feed(&y, &mut out);
            out.clear();
            own.feed(&y, &mut out);
            n += k;
        }
        let count = |r: &dyn Receiver, dial: f64| {
            messages(r)
                .iter()
                .filter(|m| num(m, "dial") == dial && text(m, "hfnpdu") == "ACARS")
                .filter(|m| common::flag(m, "acars_ok"))
                .count()
        };
        let (mut ta, mut tb) = (0, 0);
        for &d in &dials {
            ta += count(shared.as_ref(), d);
            tb += count(own.as_ref(), d);
        }
        assert!(
            rows(shared.results(), &HFDL_CHANNELS)
                .iter()
                .filter(|c| common::flag(c, "in_band"))
                .count()
                > 50,
            "the whole table in band"
        );
        if slack == 0 {
            assert_eq!((ta, tb), (3, 3), "at {snr} dB");
        } else {
            assert!(ta.abs_diff(tb) <= slack, "at {snr} dB: {ta} vs {tb}");
        }
        assert!(
            messages(shared.as_ref())
                .iter()
                .all(|m| dials.contains(&num(m, "dial"))),
            "nothing on other channels"
        );
    }
}

#[test]
fn any_cut_returns_the_same_messages() {
    let rec = sdrkit_dev_test::read_recording("avdata/hfdl_48k").expect("the HFDL recording");
    let mut got = Vec::new();
    for chunk in [65_536usize, 1000, 7, 1] {
        let mut r = rx(rec.rate_hz, rec.centre_hz, "8927");
        common::run(r.as_mut(), &rec.samples, chunk);
        got.push(
            messages(r.as_ref())
                .iter()
                .map(|m| {
                    (
                        (num(m, "signal_s") * 1e4).round() as i64,
                        (num(m, "offset_hz") * 100.0).round() as i64,
                        text(m, "summary").to_owned(),
                    )
                })
                .collect::<Vec<_>>(),
        );
    }
    assert_eq!(got[0].len(), 4);
    for g in &got[1..] {
        assert_eq!(&got[0], g);
    }
}

#[test]
fn every_event_is_declared() {
    let p = GenParams {
        rate_hz: 48e3,
        centre_hz: 8.92e6,
        aircraft: 2,
        stations: vec![(1, 8.927e6)],
        rate_factor: 4.0,
        duration_s: 40.0,
        ..Default::default()
    };
    let mut g = HfdlGenerator::new(p);
    let mut ch = common::chain(&g, impair(Some(25.0), 0.0, ChainConfig::default()));
    let mut r = rx(48e3, 8.92e6, "8927");
    let total = (40.0 * 48e3) as usize;
    let mut buf = vec![Complex32::default(); total];
    g.generate(&mut buf);
    let mut y = Vec::new();
    ch.process(&buf, &mut y);
    let out = common::run(r.as_mut(), &y, 65_536);
    assert!(!out.events.is_empty());
    for e in &out.events {
        assert!(e.freq_hz > 0.0);
    }
}
