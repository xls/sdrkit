//! VDL Mode 2: the generator through impairments into the receiver, and the recording.

use crate::common::{self, int, num, rows, text, Row};
use sdrkit_core::{Complex32, Generator, LockState, Receiver};
use sdrkit_dev_test::sigmf;
use sdrkit_stack_avdata::typed::{VDL2_AIRCRAFT, VDL2_CHANNELS, VDL2_MESSAGES, VDL2_STATIONS};
use sdrkit_stack_avdata::vdl2::generator::{burst_power, SentFrame, AMPLITUDE};
use sdrkit_stack_avdata::vdl2::{self, GenParams, Vdl2Generator};

/// A receiver with these settings; without a channel list, the European set of the channel file.
fn rx_with(rate: f64, centre: f64, settings: &[(&str, &str)]) -> Box<dyn Receiver> {
    let europe = common::channels_text(&common::channel_set("vdl2", "Europe"));
    let mut set = settings.to_vec();
    if !set.iter().any(|s| s.0 == "channels") {
        set.push(("channels", &europe));
    }
    common::receiver(&vdl2::DESCRIPTOR, rate, centre, centre, &set)
}

/// The channels of the test scenes, MHz.
const SCENE_MHZ: [f64; 6] = [136.725, 136.775, 136.825, 136.875, 136.900, 136.975];

/// The channels of the test scenes, Hz.
fn scene_channels() -> Vec<f64> {
    SCENE_MHZ.iter().map(|f| (f * 1e6).round()).collect()
}

fn run(rx: &mut dyn Receiver, x: &[Complex32], chunk: usize) {
    common::run(rx, x, chunk);
}

fn frames(rx: &dyn Receiver) -> Vec<Row> {
    rows(rx.results(), &VDL2_MESSAGES)
}

fn hex(r: &Row, key: &str) -> u32 {
    u32::from_str_radix(text(r, key), 16).unwrap_or(u32::MAX)
}

/// Adds noise at `esn0` dB for a burst at 0 dB.
fn noise(x: &mut [Complex32], esn0: f64, rate: f64, seed: u64) {
    common::noise(x, burst_power(), esn0, 10.5e3, rate, seed);
}

fn scene(p: GenParams, secs: f64) -> (Vec<Complex32>, Vec<SentFrame>, Vec<SentFrame>) {
    let mut g = Vdl2Generator::new(p.clone());
    let mut x = vec![Complex32::default(); (p.rate_hz * secs) as usize];
    for c in x.chunks_mut(16_384) {
        g.generate(c);
    }
    let sent: Vec<SentFrame> = g
        .answer_key()
        .iter()
        .filter(|f| f.time_s < secs - 0.2)
        .cloned()
        .collect();
    (x, sent, g.answer_key().to_vec())
}

fn score(got: &[Row], sent: &[SentFrame], all: &[SentFrame]) -> (f64, usize) {
    let found = sent
        .iter()
        .filter(|s| {
            got.iter().any(|m| {
                hex(m, "from") == s.src
                    && hex(m, "to") == s.dst
                    && num(m, "frequency") == s.freq_hz
                    && (num(m, "signal_s") - s.time_s).abs() < 0.006
            })
        })
        .count();
    let extra = got
        .iter()
        .filter(|m| {
            !all.iter().any(|s| {
                hex(m, "from") == s.src
                    && hex(m, "to") == s.dst
                    && (num(m, "signal_s") - s.time_s).abs() < 0.006
            })
        })
        .count();
    (found as f64 / sent.len().max(1) as f64, extra)
}

fn base() -> GenParams {
    GenParams {
        rate_hz: 1e6,
        aircraft: 8,
        rate_factor: 2.5,
        until_s: 5.8,
        centre_hz: 136.85e6,
        channels: scene_channels(),
        ..Default::default()
    }
}

#[test]
fn clean_traffic_is_all_found() {
    let (mut x, sent, all) = scene(base(), 6.0);
    noise(&mut x, 30.0, 1e6, 1);
    let mut rx = rx_with(1e6, 136.85e6, &[]);
    run(rx.as_mut(), &x, 16_384);
    let (r, extra) = score(&frames(rx.as_ref()), &sent, &all);
    assert!(r >= 0.99, "{r}");
    assert_eq!(extra, 0);
}

#[test]
fn the_recording_decodes_as_expected() {
    let rec = sdrkit_dev_test::read_recording("avdata/vdl2_250k").expect("the VDL2 recording");
    let exp = sigmf::read_expected("avdata").expect("expected");
    let e = &exp["vdl2_250k.sigmf-meta"];
    let mut rx = rx_with(
        rec.rate_hz,
        rec.centre_hz,
        &[("channels", "136.900 136.775")],
    );
    run(rx.as_mut(), &rec.samples, 65_536);
    let r = rx.results();
    let got = rows(r, &VDL2_MESSAGES);
    let want = e["frames"].as_array().unwrap();
    assert_eq!(got.len(), want.len());
    for w in want {
        let t = w["t_s"].as_f64().unwrap();
        let src = u32::from_str_radix(w["src"].as_str().unwrap(), 16).unwrap();
        let m = got
            .iter()
            .find(|m| {
                hex(m, "from") == src
                    && (num(m, "signal_s") - t).abs() < 0.006
                    && num(m, "frequency") == w["freq_hz"].as_f64().unwrap()
                    && w["kind"].as_str() == Some(text(m, "frame_kind"))
            })
            .unwrap_or_else(|| panic!("no frame for {w}"));
        if let Some(n) = w["name"].as_str() {
            assert_eq!(text(m, "frame"), n, "{w}");
        }
        if let Some(b) = w["response"].as_bool() {
            assert_eq!(common::flag(m, "response"), b, "{w}");
        }
        if let Some(p) = w["proto"].as_str() {
            assert_eq!(text(m, "protocol"), p, "{w}");
        }
        if let Some(s) = w["summary"].as_str() {
            assert_eq!(text(m, "summary"), s, "{w}");
        }
        if let Some(p) = w["position"].as_array() {
            let (lat, lon) = common::position(m, "position").expect("position");
            assert!(
                (lat - p[0].as_f64().unwrap()).abs() < 1e-4
                    && (lon - p[1].as_f64().unwrap()).abs() < 1e-4,
                "{w}"
            );
        }
        if let Some(t) = w["acars_text"].as_str() {
            assert_eq!(text(m, "text"), t);
        }
        if let Some(ok) = w["acars_crc_ok"].as_bool() {
            assert_eq!(common::flag(m, "acars_ok"), ok);
        }
        if let Some(c) = w["called_dte"].as_str() {
            assert!(text(m, "detail").contains(&format!("called DTE {c}")));
        }
    }
    assert_eq!(common::field(r, "bursts_bad"), 0.0);
    assert_eq!(common::field(r, "fcs_bad"), 0.0);
    assert_eq!(common::field(r, "positions"), 3.0);
    assert_eq!(r.status().lock_state, LockState::Locked);
    let gs = rows(r, &VDL2_STATIONS);
    let gs = gs.iter().find(|s| text(s, "address") == "10A000").unwrap();
    assert_eq!(int(gs, "count"), 3);
    assert_eq!(text(gs, "airports"), "ZZZZ");
    assert!(common::field(r, "tuning_error").abs() <= 150.0);
    let a = rows(r, &VDL2_AIRCRAFT);
    let a = a.iter().find(|a| text(a, "address") == "4CA2B1").unwrap();
    assert_eq!(text(a, "ground_station"), "10A000");
    assert_eq!(int(a, "trail_points"), 2);
    assert!((num(a, "alt_ft") - 36000.0).abs() < 1e-6);
    assert!((num(a, "track_deg") - 90.0).abs() < 0.1 && (num(a, "gs_kt") - 450.0).abs() < 0.6);
    assert_eq!(text(a, "registration"), "TST001");
    assert_eq!(text(a, "flight"), "TEST01");
    assert_eq!(int(a, "count"), 6);
}

struct Case {
    rate: f64,
    esn0: f64,
    offset: f64,
    ppm: f64,
    seed: u64,
}

fn case(
    c: Case,
    settings: &[(&str, &str)],
    chunk: &mut dyn FnMut() -> usize,
) -> (f64, usize, Box<dyn Receiver>) {
    let p = GenParams {
        rate_hz: c.rate,
        seed: c.seed,
        level_spread_db: 0.0,
        ..base()
    };
    let (mut x, sent, all) = scene(p, 6.0);
    if c.ppm != 0.0 {
        x = common::clock(&x, c.ppm);
    }
    if c.offset != 0.0 {
        common::offset(&mut x, c.rate, c.offset);
    }
    noise(&mut x, c.esn0, c.rate, c.seed + 100);
    let mut rx = rx_with(c.rate, 136.85e6, settings);
    let mut i = 0;
    let mut out = sdrkit_core::Output::default();
    while i < x.len() {
        let n = chunk().clamp(1, x.len() - i);
        out.clear();
        rx.feed(&x[i..i + n], &mut out);
        i += n;
    }
    let (r, e) = score(&frames(rx.as_ref()), &sent, &all);
    (r, e, rx)
}

fn fixed(n: usize) -> impl FnMut() -> usize {
    move || n
}

#[test]
fn a_radio_off_by_5_khz() {
    for off in [5e3, -5e3] {
        let c = Case {
            rate: 1e6,
            esn0: 25.0,
            offset: off,
            ppm: 0.0,
            seed: 2,
        };
        let (r, e, rx) = case(c, &[], &mut fixed(16_384));
        let err = common::field(rx.results(), "tuning_error");
        assert!(r >= 0.95, "{r}");
        assert_eq!(e, 0);
        assert!((err - off).abs() < 300.0, "{err}");
    }
}

#[test]
fn a_clock_error() {
    for ppm in [50.0, -50.0] {
        let c = Case {
            rate: 1e6,
            esn0: 25.0,
            offset: 0.0,
            ppm,
            seed: 3,
        };
        let (r, _, _) = case(c, &[], &mut fixed(16_384));
        assert!(r >= 0.97, "{r}");
    }
}

#[test]
fn the_threshold() {
    let at = |esn0| {
        let c = Case {
            rate: 1e6,
            esn0,
            offset: 0.0,
            ppm: 0.0,
            seed: 4,
        };
        case(c, &[], &mut fixed(16_384))
    };
    let (r17, _, _) = at(17.0);
    let (r15, e, _) = at(15.0);
    assert!(r17 >= 0.90, "{r17}");
    assert!(r15 >= 0.30, "{r15}");
    assert_eq!(e, 0);
}

#[test]
fn random_buffer_sizes() {
    let mut rng = sdrkit_algo_sim::rng::Rng::new(12345);
    let mut next = move || 1 + rng.below(20_000) as usize;
    let c = Case {
        rate: 1e6,
        esn0: 30.0,
        offset: 0.0,
        ppm: 0.0,
        seed: 5,
    };
    let (r, _, _) = case(c, &[], &mut next);
    assert!(r >= 0.99, "{r}");
}

#[test]
fn two_msps() {
    let c = Case {
        rate: 2e6,
        esn0: 25.0,
        offset: 0.0,
        ppm: 0.0,
        seed: 8,
    };
    let (r, e, _) = case(c, &[], &mut fixed(65_536));
    assert!(r >= 0.99, "{r}");
    assert_eq!(e, 0);
}

#[test]
fn noise_with_bad_samples_decodes_nothing() {
    let rate = 1e6;
    let mut x = vec![Complex32::default(); 3_000_000];
    common::noise(&mut x, 1.0, 37.0, rate, rate, 9);
    for i in 0..100 {
        x[i * 29_989 + 7] = if i % 2 == 0 {
            Complex32::new(f32::NAN, 0.0)
        } else {
            Complex32::new(f32::INFINITY, -f32::INFINITY)
        };
    }
    let mut rx = rx_with(rate, 136.85e6, &[]);
    run(rx.as_mut(), &x, 16_384);
    assert!(frames(rx.as_ref()).is_empty());
    assert!(common::field(rx.results(), "tuning_error").is_finite());
    assert_eq!(common::field(rx.results(), "bad_samples"), 100.0);
}

#[test]
fn listed_channels_and_the_capture() {
    // The worldwide set of the channel file and one more channel: every listed channel is a row, the ones inside the
    // capture are watched.
    let mut all = common::channel_set("vdl2", "Worldwide");
    assert_eq!(all.len(), 22);
    all.push(136.5e6);
    let list = common::channels_text(&all);
    let mut rx = rx_with(2e6, 136.85e6, &[("channels", &list)]);
    run(rx.as_mut(), &vec![Complex32::default(); 600_000], 600_000);
    let chans = rows(rx.results(), &VDL2_CHANNELS);
    assert_eq!(chans.len(), all.len());
    let mut inb: Vec<f64> = chans
        .iter()
        .filter(|c| common::flag(c, "in_band"))
        .map(|c| num(c, "frequency"))
        .collect();
    inb.sort_by(f64::total_cmp);
    let mut want: Vec<f64> = all
        .iter()
        .copied()
        .filter(|f| (f - 136.85e6f64).abs() < 1e6 - 14e3)
        .collect();
    want.sort_by(f64::total_cmp);
    assert_eq!(inb, want);
    // An empty list: the tuned frequency and the common signalling channel.
    let mut rx = common::receiver(&vdl2::DESCRIPTOR, 2e6, 136.85e6, 136.85e6, &[]);
    run(rx.as_mut(), &vec![Complex32::default(); 600_000], 600_000);
    let chans = rows(rx.results(), &VDL2_CHANNELS);
    let mut f: Vec<f64> = chans.iter().map(|c| num(c, "frequency")).collect();
    f.sort_by(f64::total_cmp);
    assert_eq!(f, [136.85e6, 136.975e6]);
    // Tuned to the common signalling channel: that channel only.
    let mut rx = common::receiver(&vdl2::DESCRIPTOR, 2e6, 136.975e6, 136.975e6, &[]);
    run(rx.as_mut(), &vec![Complex32::default(); 600_000], 600_000);
    let chans = rows(rx.results(), &VDL2_CHANNELS);
    assert_eq!(chans.len(), 1);
    assert_eq!(num(&chans[0], "frequency"), 136.975e6);
}

/// An ACARS downlink frame from `src` to ground station 0x10A000 with `n` characters of text.
fn acars_frame(src: u32, n: usize) -> Vec<u8> {
    use sdrkit_proto_aero::acars::{BlockSpec, End};
    use sdrkit_stack_avdata::vdl2::avlc::{acars_info, header, i_control};
    let text: String = (0..n).map(|i| (b'A' + (i % 26) as u8) as char).collect();
    let t = format!("M01ATEST01{text}");
    let mut b = Vec::new();
    let c = BlockSpec::new('2', "TST001", b'1', End::Etx)
        .label("H1")
        .text(&t)
        .body(&mut b)
        .unwrap();
    let mut f = header(0x10A000, 4, false, src, 1, false, i_control(0, 0, false));
    f.extend(acars_info(&b, c));
    f
}

#[test]
fn overlapping_bursts() {
    let rate = 1e6;
    let quiet = |seed| {
        Vdl2Generator::new(GenParams {
            rate_hz: rate,
            centre_hz: 136.85e6,
            channels: vec![136.975e6],
            until_s: 0.0,
            seed,
            ..Default::default()
        })
    };
    let (mut strong, mut weak) = (quiet(1), quiet(2));
    let mut strong_sent = Vec::new();
    for k in 0..8 {
        let t = 0.1 + 0.4 * k as f64;
        let wsrc = 0x400100 + k as u32;
        let ssrc = 0x400200 + k as u32;
        if k % 2 == 0 {
            // The strong one first, the weak one starting inside it.
            let end = strong.send(t, 0, 0.0, 0.0, &[acars_frame(ssrc, 120)]);
            weak.send((t + end) / 2.0, 0, -20.0, 0.0, &[acars_frame(wsrc, 120)]);
        } else {
            // The strong one starting in the middle of the weak one.
            let end = weak.send(t, 0, -20.0, 0.0, &[acars_frame(wsrc, 120)]);
            strong.send((t + end) / 2.0, 0, 0.0, 0.0, &[acars_frame(ssrc, 120)]);
        }
        strong_sent.push(ssrc);
    }
    let n = (3.6 * rate) as usize;
    let mut x = vec![Complex32::default(); n];
    let mut y = vec![Complex32::default(); n];
    strong.generate(&mut x);
    weak.generate(&mut y);
    for (a, b) in x.iter_mut().zip(&y) {
        *a += *b;
    }
    noise(&mut x, 25.0, rate, 6);
    let mut rx = rx_with(rate, 136.85e6, &[("channels", "136.975")]);
    run(rx.as_mut(), &x, 16_384);
    let got = frames(rx.as_ref());
    for s in strong_sent {
        assert!(
            got.iter().any(|m| hex(m, "from") == s),
            "strong burst {s:06X} lost"
        );
    }
}

#[test]
fn a_strong_neighbour() {
    let rate = 1e6;
    let p = GenParams {
        rate_hz: rate,
        channels: vec![136.900e6],
        seed: 7,
        level_spread_db: 0.0,
        ..base()
    };
    let (mut x, sent, all) = scene(p, 6.0);
    // A continuous carrier 25 kHz below, 30 dB above a burst. The whole signal scaled to stay within full scale.
    let a = AMPLITUDE * 10f64.powf(30.0 / 20.0);
    let w = std::f64::consts::TAU * (136.875e6 - 136.85e6) / rate;
    let g = 0.3f32;
    for (i, v) in x.iter_mut().enumerate() {
        let ph = w * i as f64;
        *v += Complex32::new((a * ph.cos()) as f32, (a * ph.sin()) as f32);
        *v *= g;
    }
    let p = burst_power() * (g * g) as f64;
    common::noise(&mut x, p, 25.0, 10.5e3, rate, 7);
    let mut rx = rx_with(rate, 136.85e6, &[("channels", "136.875 136.900")]);
    run(rx.as_mut(), &x, 16_384);
    let (r, e) = score(&frames(rx.as_ref()), &sent, &all);
    assert!(r >= 0.95, "{r}");
    assert_eq!(e, 0);
}

#[test]
fn any_cut_returns_the_same_frames() {
    let p = GenParams {
        rate_hz: 1e6,
        aircraft: 6,
        rate_factor: 3.0,
        until_s: 2.0,
        centre_hz: 136.85e6,
        channels: scene_channels(),
        ..Default::default()
    };
    let (mut x, _, _) = scene(p, 2.0);
    noise(&mut x, 25.0, 1e6, 11);
    let mut got = Vec::new();
    for chunk in [65_536usize, 1000, 7] {
        let mut rx = rx_with(1e6, 136.85e6, &[]);
        run(rx.as_mut(), &x, chunk);
        got.push(
            frames(rx.as_ref())
                .iter()
                .map(|m| {
                    (
                        (num(m, "signal_s") * 1e5).round() as i64,
                        text(m, "from").to_owned(),
                        text(m, "summary").to_owned(),
                    )
                })
                .collect::<Vec<_>>(),
        );
    }
    assert!(got[0].len() >= 5, "{}", got[0].len());
    assert_eq!(got[0], got[1]);
    assert_eq!(got[0], got[2]);
}

#[test]
fn every_event_is_declared() {
    let (mut x, _, _) = scene(base(), 4.0);
    noise(&mut x, 30.0, 1e6, 12);
    let mut rx = rx_with(1e6, 136.85e6, &[]);
    let out = common::run(rx.as_mut(), &x, 65_536);
    assert!(!out.events.is_empty());
    for e in &out.events {
        assert!(e.freq_hz > 0.0);
    }
}

/// The receiver watches the set that the host gives: two sets of the channel file.
#[test]
fn two_channel_sets_of_the_file() {
    for set in ["Europe", "North America"] {
        let ch = common::channel_set("vdl2", set);
        let centre = ((ch[0] + ch[ch.len() - 1]) / 2.0 / 25e3).round() * 25e3 + 12.5e3;
        let p = GenParams {
            rate_hz: 2e6,
            centre_hz: centre,
            channels: ch.clone(),
            ..base()
        };
        let (mut x, sent, all) = scene(p, 6.0);
        noise(&mut x, 30.0, 2e6, 1);
        let list = common::channels_text(&ch);
        let mut rx = rx_with(2e6, centre, &[("channels", &list)]);
        run(rx.as_mut(), &x, 16_384);
        let (r, extra) = score(&frames(rx.as_ref()), &sent, &all);
        assert!(r >= 0.99, "{set} {r}");
        assert_eq!(extra, 0, "{set}");
    }
}

/// The default test signal locks the receiver with its default settings, tuned to the common signalling channel as the
/// verify case is.
#[test]
fn the_default_signal_and_receiver() {
    let csc = (vdl2::front::CSC_MHZ * 1e6).round();
    let rx = common::default_run_at(&vdl2::DESCRIPTOR, 1e6, 10.0, csc);
    assert!(common::field(rx.results(), "frames_ok") >= 5.0);
}
