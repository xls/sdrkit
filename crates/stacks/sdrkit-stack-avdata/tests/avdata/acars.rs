//! ACARS: the generator through impairments into the receiver, the recording, cut independence, saved state and settings.

use crate::common::{self, flag, int, num, rows, text, Row};
use sdrkit_core::{Complex32, Generator, InputInfo, LockState, Output, Receiver};
use sdrkit_dev_test::sigmf;
use sdrkit_stack_avdata::acars::generator::{GenParams, Sent};
use sdrkit_stack_avdata::acars::{self, AcarsGenerator};
use sdrkit_stack_avdata::typed::{ACARS_AIRCRAFT, ACARS_CHANNELS, ACARS_MESSAGES};

/// The European set of the channel file.
fn europe() -> Vec<f64> {
    common::channel_set("acars", "Europe")
}

fn rx_with(rate: f64, centre: f64, settings: &[(&str, &str)]) -> Box<dyn Receiver> {
    common::receiver(&acars::DESCRIPTOR, rate, centre, centre, settings)
}

/// The test signal with noise at `snr_db` in each 7 kHz channel, and the blocks sent.
fn signal(p: GenParams, seconds: f64, snr_db: f64) -> (Vec<Complex32>, Vec<Sent>) {
    let mut g = AcarsGenerator::new(p.clone());
    let mut x = vec![Complex32::default(); (p.rate_hz * seconds) as usize];
    for c in x.chunks_mut(65_536) {
        g.generate(c);
    }
    let power = g.carrier_power();
    common::noise(&mut x, power, snr_db, 7e3, p.rate_hz, p.seed ^ 0x5151);
    (x, g.answer_key().to_vec())
}

fn run(rx: &mut dyn Receiver, x: &[Complex32], chunk: usize) {
    common::run(rx, x, chunk);
}

fn messages(rx: &dyn Receiver) -> Vec<Row> {
    rows(rx.results(), &ACARS_MESSAGES)
}

/// Fraction of the blocks sent (ending before `until`) decoded with the right content, and how many wrong ones were decoded.
fn score(got: &[Row], sent: &[Sent], until: f64) -> (f64, usize, usize) {
    let want: Vec<&Sent> = sent.iter().filter(|s| s.end_s < until).collect();
    let same = |m: &Row, s: &Sent| {
        text(m, "text") == s.msg.text
            && text(m, "registration") == s.msg.registration
            && text(m, "block_id") == s.msg.block_id.to_string()
    };
    let ok = want
        .iter()
        .filter(|s| {
            got.iter().any(|m| {
                (num(m, "signal_s") - s.end_s).abs() < 0.1
                    && num(m, "frequency") == s.freq_hz
                    && text(m, "label") == s.msg.label
                    && same(m, s)
            })
        })
        .count();
    let wrong = got
        .iter()
        .filter(|m| !sent.iter().any(|s| same(m, s)))
        .count();
    (ok as f64 / want.len().max(1) as f64, wrong, want.len())
}

fn base() -> GenParams {
    GenParams {
        rate_factor: 3.0,
        centre_hz: 131.5e6,
        channels: europe(),
        ..Default::default()
    }
}

fn quick(
    p: GenParams,
    secs: f64,
    snr_db: f64,
    settings: &[(&str, &str)],
    imp: &mut dyn FnMut(Vec<Complex32>) -> Vec<Complex32>,
) -> (f64, usize, usize, Box<dyn Receiver>) {
    let (rate, centre) = (p.rate_hz, p.centre_hz);
    let (x, sent) = signal(p, secs, snr_db);
    let x = imp(x);
    let mut rx = rx_with(rate, centre, settings);
    run(rx.as_mut(), &x, 65_536);
    let (r, w, n) = score(&messages(rx.as_ref()), &sent, secs - 1.2);
    (r, w, n, rx)
}

#[test]
fn default_scene_decodes() {
    let secs = 24.0;
    let (x, sent) = signal(base(), secs, 35.0);
    let mut rx = rx_with(2e6, 131.5e6, &[("join_blocks", "0")]);
    run(rx.as_mut(), &x, 65_536);
    let got = messages(rx.as_ref());
    let (rate, wrong, n) = score(&got, &sent, secs - 1.2);
    assert!(n >= 40);
    assert!(rate >= 0.999, "decoded {rate}");
    assert_eq!(wrong, 0);
    let r = rx.results();
    assert_eq!(r.status().lock_state, LockState::Locked);
    let aircraft = rows(r, &ACARS_AIRCRAFT);
    assert_eq!(aircraft.len(), 6);
    assert!(aircraft.iter().all(|a| !text(a, "flight").is_empty()));
    assert_eq!(rows(r, &ACARS_CHANNELS).len(), 3);
    assert!(
        got.iter()
            .filter(|m| text(m, "reading").starts_with("from "))
            .count()
            >= 3
    );
    let times: Vec<f64> = got.iter().map(|m| num(m, "signal_s")).collect();
    assert!(times.windows(2).all(|w| w[0] >= w[1]), "newest first");
}

#[test]
fn the_recording_decodes_as_expected() {
    let rec = sdrkit_dev_test::read_recording("avdata/acars_1m").expect("the ACARS recording");
    let exp = sigmf::read_expected("avdata").expect("expected");
    let e = &exp["acars_1m.sigmf-meta"];
    let mut rx = rx_with(rec.rate_hz, rec.centre_hz, &[]);
    run(rx.as_mut(), &rec.samples, 65_536);
    let r = rx.results();
    let got = rows(r, &ACARS_MESSAGES);
    let want = e["messages"].as_array().unwrap();
    assert_eq!(got.len(), want.len(), "{got:#?}");
    for (m, w) in got.iter().rev().zip(want) {
        assert!((num(m, "signal_s") - w["end_s"].as_f64().unwrap()).abs() < 0.25);
        assert_eq!(num(m, "frequency"), w["freq_hz"].as_f64().unwrap());
        for k in [
            "mode",
            "registration",
            "ack",
            "label",
            "label_meaning",
            "block_id",
            "msn",
            "flight",
            "text",
        ] {
            assert_eq!(text(m, k), w[k].as_str().unwrap(), "{k}");
        }
        assert_eq!(
            common::flag(m, "downlink"),
            w["downlink"].as_bool().unwrap()
        );
        assert_eq!(int(m, "blocks") as u64, w["blocks"].as_u64().unwrap());
        assert_eq!(
            int(m, "bits_fixed") as u64,
            w["bits_fixed"].as_u64().unwrap()
        );
    }
    let a = &rows(r, &ACARS_AIRCRAFT)[0];
    assert_eq!(
        (
            text(a, "registration"),
            text(a, "flight"),
            int(a, "messages")
        ),
        ("TST001", "TEST01", 3)
    );
    assert_eq!(common::field(r, "frames_ok"), 3.0);
    assert_eq!(common::field(r, "frames_bad"), 0.0);
    assert!(common::field(r, "frames_started") >= 3.0);
    assert_eq!(r.status().lock_state, LockState::Locked);
    let ch = rows(r, &ACARS_CHANNELS);
    assert_eq!(ch.len(), 1);
    assert_eq!(num(&ch[0], "frequency"), 131_525_000.0);
}

#[test]
fn any_cut_returns_the_same_messages() {
    let p = GenParams {
        rate_factor: 6.0,
        centre_hz: 131.5e6,
        channels: europe(),
        ..Default::default()
    };
    let (x, _) = signal(p, 5.0, 35.0);
    let mut first: Option<Vec<(String, String, String)>> = None;
    for chunk in [65_536usize, 4096, 7, 1] {
        let mut rx = rx_with(2e6, 131.5e6, &[("join_blocks", "0")]);
        run(rx.as_mut(), &x, chunk);
        let v: Vec<(String, String, String)> = messages(rx.as_ref())
            .iter()
            .map(|m| {
                (
                    format!("{:.4}", num(m, "signal_s")),
                    text(m, "registration").to_owned(),
                    text(m, "text").to_owned(),
                )
            })
            .collect();
        assert!(v.len() >= 8, "chunk {chunk}: {}", v.len());
        match &first {
            None => first = Some(v),
            Some(f) => assert_eq!(f, &v, "chunk {chunk}"),
        }
    }
}

#[test]
fn an_833_channel_needs_the_list() {
    let p = GenParams {
        aircraft: 3,
        channels: vec![131.516_667e6],
        ..base()
    };
    let (r, _, n, _) = quick(p.clone(), 20.0, 35.0, &[("join_blocks", "0")], &mut |x| x);
    assert!(r < 0.05 && n >= 8, "{r} of {n}");
    let (r, w, _, _) = quick(
        p,
        20.0,
        35.0,
        &[("join_blocks", "0"), ("channels", "131.516667")],
        &mut |x| x,
    );
    assert!(r >= 0.999 && w == 0, "{r} {w}");
}

#[test]
fn an_off_grid_centre() {
    let p = GenParams {
        centre_hz: 131.512e6,
        ..base()
    };
    let (r, w, _, rx) = quick(p, 12.0, 35.0, &[("join_blocks", "0")], &mut |x| x);
    assert!(r >= 0.999 && w == 0, "{r} {w}");
    let mut f: Vec<f64> = rows(rx.results(), &ACARS_CHANNELS)
        .iter()
        .filter(|c| int(c, "good") > 0)
        .map(|c| num(c, "frequency"))
        .collect();
    f.sort_by(f64::total_cmp);
    assert_eq!(f, vec![131.525e6, 131.725e6, 131.825e6]);
}

#[test]
fn one_msps() {
    let p = GenParams {
        rate_hz: 1e6,
        channels: vec![131.525e6, 131.725e6, 131.125e6],
        ..base()
    };
    let (r, w, n, _) = quick(p, 20.0, 35.0, &[("join_blocks", "0")], &mut |x| x);
    assert!(r >= 0.999 && w == 0 && n >= 40, "{r} {w} {n}");
}

#[test]
fn impairments() {
    for off in [-6600.0, 3000.0, 1300.0] {
        let (r, w, _, _) = quick(base(), 12.0, 30.0, &[("join_blocks", "0")], &mut |mut x| {
            common::offset(&mut x, 2e6, off);
            x
        });
        assert!(r >= 0.98 && w == 0, "offset {off}: {r} {w}");
    }
    let (r, w, _, _) = quick(base(), 12.0, 35.0, &[("join_blocks", "0")], &mut |mut x| {
        common::offset(&mut x, 2e6, -6600.0);
        let mut y = common::clock(&x, 80.0);
        common::echo(&mut y, 3, -8.0);
        common::quantise(&mut y, 8, 2.0);
        y
    });
    assert!(r >= 0.98 && w == 0, "together: {r} {w}");
    for ppm in [-200.0, 200.0] {
        let (r, _, _, _) = quick(base(), 12.0, 35.0, &[("join_blocks", "0")], &mut |x| {
            common::clock(&x, ppm)
        });
        assert!(r >= 0.98, "clock {ppm}: {r}");
    }
}

#[test]
fn a_restart_keeps_the_list_and_the_state_restores_it() {
    let (x, _) = signal(base(), 16.0, 35.0);
    let mut rx = rx_with(2e6, 131.5e6, &[("join_blocks", "0")]);
    let half = 12 * 2_000_000;
    run(rx.as_mut(), &x[..half], 65_536);
    let before = messages(rx.as_ref()).len();
    rx.retuned(&InputInfo::new(2e6, 131.5e6, 131.5e6).recording());
    run(rx.as_mut(), &x[half..], 65_536);
    let got = messages(rx.as_ref());
    assert!(got.len() > before, "the list is kept and grows");
    assert!(
        got.iter().all(|m| num(m, "signal_s") <= 4.5),
        "times moved back"
    );
    let state = rx.save_state().expect("a saved state");
    let mut other = rx_with(2e6, 131.5e6, &[]);
    other.restore_state(&state).unwrap();
    other.feed(&vec![Complex32::default(); 600_000], &mut Output::default());
    assert_eq!(messages(other.as_ref()).len(), got.len());
    assert!(other
        .restore_state(&sdrkit_core::StateBlob::new("vdl2", 1, Vec::new()))
        .is_err());
    assert_eq!(messages(other.as_ref()).len(), got.len());
    common::command(other.as_mut(), "clear");
    other.feed(&vec![Complex32::default(); 600_000], &mut Output::default());
    assert!(messages(other.as_ref()).is_empty());
}

#[test]
fn zeros_and_the_rate_refusal() {
    let mut rx = rx_with(2e6, 131.5e6, &[]);
    let z = vec![Complex32::default(); 2_000_000];
    for chunk in [4096usize, 7, 1] {
        run(
            rx.as_mut(),
            &z[..if chunk == 1 { 200_000 } else { 2_000_000 }],
            chunk,
        );
    }
    assert_eq!(rx.results().status().lock_state, LockState::Searching);
    let cfg = sdrkit_core::StackConfig::new(common::values(&acars::DESCRIPTOR, &[]));
    let mut r = acars::receiver(&cfg).unwrap();
    assert!(r
        .configure(&InputInfo::new(500e3, 131.5e6, 131.5e6))
        .is_err());
    assert!(sdrkit_core::Owner::results(&r)
        .status()
        .summary
        .contains("1 MHz"));
}

#[test]
fn joined_messages_through_the_receiver() {
    let p = GenParams {
        rate_factor: 6.0,
        centre_hz: 131.5e6,
        channels: europe(),
        ..Default::default()
    };
    let (x, sent) = signal(p, 20.0, 35.0);
    let mut rx = rx_with(2e6, 131.5e6, &[]);
    run(rx.as_mut(), &x, 65_536);
    let two = sent.iter().filter(|s| s.msg.incomplete).count();
    let joined = messages(rx.as_ref())
        .iter()
        .filter(|m| int(m, "blocks") == 2)
        .count();
    assert!(
        two == 0 || joined >= 1,
        "{two} two-block messages sent, {joined} joined"
    );
}

#[test]
fn eight_channels_with_neighbours() {
    let ch = vec![
        131.125e6, 131.150e6, 131.525e6, 131.550e6, 131.725e6, 131.825e6, 131.850e6, 131.875e6,
    ];
    let p = GenParams {
        aircraft: 16,
        rate_factor: 2.0,
        channels: ch.clone(),
        ..base()
    };
    let (r, w, n, rx) = quick(p, 30.0, 35.0, &[("join_blocks", "0")], &mut |x| x);
    assert!(r >= 0.999 && w == 0 && n >= 80, "{r} {w} {n}");
    let c = rows(rx.results(), &ACARS_CHANNELS);
    for f in ch {
        assert!(
            c.iter()
                .any(|c| num(c, "frequency") == f && int(c, "good") > 0),
            "{f} not heard"
        );
    }
}

#[test]
fn eight_bit_samples_and_dc() {
    for dc in [None, Some((0.06f32, -0.05f32))] {
        let (r, w, _, _) = quick(base(), 12.0, 35.0, &[("join_blocks", "0")], &mut |mut x| {
            if let Some((i, q)) = dc {
                for v in x.iter_mut() {
                    *v += Complex32::new(i, q);
                }
            }
            common::quantise(&mut x, 8, 1.0);
            x
        });
        assert!(r >= 0.98 && w == 0, "dc {dc:?}: {r} {w}");
    }
}

#[test]
fn a_gap_in_the_samples() {
    let (x, sent) = signal(base(), 24.0, 35.0);
    let cut = (13.37 * 2e6) as usize;
    let gap = (0.02 * 2e6) as usize;
    let mut y = x[..cut].to_vec();
    y.extend_from_slice(&x[cut + gap..]);
    let mut rx = rx_with(2e6, 131.5e6, &[("join_blocks", "0")]);
    run(rx.as_mut(), &y, 65_536);
    // Times after the gap are 20 ms early. Blocks overlapping it are not counted.
    let shifted: Vec<Sent> = sent
        .iter()
        .filter(|s| s.end_s < 13.37 || s.start_s > 13.39)
        .map(|s| {
            let mut s = s.clone();
            if s.start_s > 13.39 {
                s.end_s -= 0.02;
            }
            s
        })
        .collect();
    let (r, w, _) = score(&messages(rx.as_ref()), &shifted, 22.8);
    assert!(r >= 0.98 && w == 0, "{r} {w}");
    assert!(common::field(rx.results(), "frames_ok") > 40.0);
}

#[test]
fn a_weak_channel_beside_a_strong_one() {
    for diff in [20.0, 30.0, 40.0] {
        let p = GenParams {
            last_atten_db: diff,
            channels: vec![131.525e6, 131.550e6],
            ..base()
        };
        let (x, sent) = signal(p, 24.0, diff + 28.0);
        let mut rx = rx_with(2e6, 131.5e6, &[("join_blocks", "0")]);
        run(rx.as_mut(), &x, 65_536);
        let got = messages(rx.as_ref());
        let (r, w, _) = score(&got, &sent, 22.8);
        assert!(r >= 0.98 && w == 0, "{diff} dB: {r} {w}");
        for f in [131.525e6, 131.550e6] {
            let n = got.iter().filter(|m| num(m, "frequency") == f).count();
            assert!(n >= 10, "{diff} dB: {n} on {f}");
        }
    }
}

#[test]
fn rates() {
    for rate in [1e6, 2.048e6, 2.4e6, 3.2e6, 8e6] {
        let p = GenParams {
            rate_hz: rate,
            channels: vec![131.525e6, 131.725e6, 131.125e6],
            ..base()
        };
        let (r, w, n, _) = quick(p, 12.0, 35.0, &[("join_blocks", "0")], &mut |x| x);
        assert!(r >= 0.98 && w == 0 && n > 0, "{rate}: {r} {w} {n}");
    }
    let p = GenParams {
        rate_hz: 10e6,
        channels: vec![127.825e6, 135.125e6, 131.725e6],
        ..base()
    };
    let (r, w, n, _) = quick(p, 8.0, 35.0, &[("join_blocks", "0")], &mut |x| x);
    assert!(r >= 0.98 && w == 0 && n > 0, "10 MHz: {r} {w} {n}");
}

#[test]
fn snr_sweep() {
    let mut last = 1.0f64;
    let mut at = Vec::new();
    for snr in [24.0, 16.0, 13.0, 11.0] {
        let p = GenParams { seed: 3, ..base() };
        let (r, w, _, _) = quick(p, 16.0, snr, &[("join_blocks", "0")], &mut |x| x);
        assert!(w == 0, "{snr} dB: {w} wrong");
        assert!(r <= last + 0.05, "{snr} dB: {r} rose from {last}");
        last = r;
        at.push((snr, r));
    }
    assert!(at[0].1 >= 0.98 && at[1].1 >= 0.98, "{at:?}");
    assert!(at[2].1 >= 0.90, "{at:?}");
}

#[test]
fn am_depth() {
    for depth in [0.3, 0.5, 0.7, 0.9] {
        let p = GenParams { depth, ..base() };
        let (r, w, _, _) = quick(p, 12.0, 30.0, &[("join_blocks", "0")], &mut |x| x);
        assert!(r >= 0.98 && w == 0, "depth {depth}: {r} {w}");
    }
}

#[test]
fn every_event_is_declared() {
    let p = GenParams {
        positions: true,
        ..base()
    };
    let (x, _) = signal(p, 12.0, 35.0);
    let mut rx = rx_with(2e6, 131.5e6, &[]);
    let out = common::run(rx.as_mut(), &x, 65_536);
    assert!(!out.events.is_empty());
    for e in &out.events {
        assert!(e.freq_hz > 0.0);
    }
}

/// A FANS-1/A CPDLC position report (a real envelope) as the text of every downlink of 3 aircraft, at three carrier offsets and
/// noise levels; the last with ADS-C position reports beside it.
#[test]
fn cpdlc_envelopes_are_decoded() {
    const AT1: &str = "/SOUCAYA.AT1.HL8251243F880C3D903BB412903604FE326C2479F4A64F7F62528B1A9CF8382738186AC28B16668E013DF464D8A7F0";
    for (off, cn, positions) in [
        (0.0, 30.0, false),
        (2500.0, 16.0, false),
        (-1800.0, 25.0, true),
    ] {
        let p = GenParams {
            aircraft: 3,
            fixed_text: Some(AT1.into()),
            positions,
            rate_factor: 3.0,
            centre_hz: 131.5e6,
        channels: europe(),
        ..Default::default()
        };
        let (mut x, _) = signal(p.clone(), 12.0, cn);
        common::offset(&mut x, p.rate_hz, off);
        let mut rx = rx_with(p.rate_hz, p.centre_hz, &[]);
        run(rx.as_mut(), &x, 65_536);
        let got = messages(rx.as_ref());
        let cpdlc: Vec<&Row> = got
            .iter()
            .filter(|m| text(m, "cpdlc_imi") == "AT1")
            .collect();
        assert!(
            cpdlc.len() >= 2,
            "{off} Hz {cn} dB: {} CPDLC messages",
            cpdlc.len()
        );
        for m in &cpdlc {
            assert!(
                flag(m, "cpdlc_crc_ok") && flag(m, "cpdlc_parsed"),
                "{off} Hz {cn} dB"
            );
            assert!(text(m, "cpdlc_text").contains("POSITION REPORT"));
            assert_eq!(text(m, "reading_kind"), "CPDLC message");
        }
        if positions {
            let adsc: Vec<&Row> = got
                .iter()
                .filter(|m| text(m, "reading").starts_with("ADS-C report"))
                .collect();
            assert!(!adsc.is_empty());
            assert!(adsc.iter().all(|m| text(m, "cpdlc_imi").is_empty()));
        }
    }
}

/// The receiver watches the set that the host gives: two sets of the channel file.
#[test]
fn two_channel_sets_of_the_file() {
    for set in ["Europe", "North America"] {
        let mut ch = common::channel_set("acars", set);
        ch.sort_by(f64::total_cmp);
        let centre = ((ch[0] + ch[ch.len() - 1]) / 2.0 / 25e3).round() * 25e3 + 12.5e3;
        let p = GenParams {
            centre_hz: centre,
            channels: ch.clone(),
            ..base()
        };
        let list = common::channels_text(&ch);
        let (r, w, _, rx) = quick(p, 12.0, 35.0, &[("channels", &list)], &mut |x| x);
        assert!(r >= 0.999 && w == 0, "{set} {r} {w}");
        let mut f: Vec<f64> = rows(rx.results(), &ACARS_CHANNELS)
            .iter()
            .filter(|c| int(c, "good") > 0)
            .map(|c| num(c, "frequency"))
            .collect();
        f.sort_by(f64::total_cmp);
        assert_eq!(f, ch, "{set}");
    }
}

/// The default test signal locks the receiver with its default settings.
#[test]
fn the_default_signal_and_receiver() {
    let rx = common::default_run(&acars::DESCRIPTOR, 2e6, 20.0);
    let r = rx.results();
    assert!(common::field(r, "frames_ok") >= 3.0 && common::field(r, "messages") >= 3.0);
}
