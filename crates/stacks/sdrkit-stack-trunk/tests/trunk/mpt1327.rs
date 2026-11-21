//! MPT 1327: the recorded vector, the eight messages at several audio rates, impairments, and following a grant.

use crate::common::*;
use num_complex::Complex32;
use sdrkit_algo_sim::chain::{NoiseKind, GEN_NOISE, GEN_OFFSET_HZ, GEN_SNR_DB};
use sdrkit_core::{HostRequest, InputInfo, Owner, Receiver, Results, SignalType, StackConfig};
use sdrkit_dev_test::{ClockError, Harness};
use sdrkit_graph::StackGraph;
use sdrkit_stack_trunk::mpt::code::{check_and_repair, code, encode, Check};
use sdrkit_stack_trunk::mpt::gen::MptSignal;
use sdrkit_stack_trunk::mpt::rx::channel_frequency;
use sdrkit_stack_trunk::typed::{MPT1327_CHANNELS, MPT1327_MESSAGES};
use sdrkit_stack_trunk::{mpt, MPT1327};

/// A receiver of audio (the real part of the samples) at any rate.
fn audio_rx(framing: &str) -> StackGraph {
    mpt::audio_receiver(&StackConfig::new(values(&MPT1327, &[("framing", framing)]))).unwrap()
}

fn run_audio(audio: &[f32], rate: f64, framing: &str) -> Results {
    let mut rx = audio_rx(framing);
    rx.configure(&InputInfo::new(rate, 0.0, 0.0).recording())
        .unwrap();
    feed(&mut rx, &audio_iq(audio), 1000);
    feed(&mut rx, &audio_iq(&vec![0.0; (rate * 0.5) as usize]), 1000);
    Owner::results(&rx).clone()
}

/// "TYPE called channel" of every message.
fn types(r: &Results) -> Vec<String> {
    rows(r, &MPT1327_MESSAGES)
        .into_iter()
        .map(|m| {
            let ch = if m["channel"].is_empty() {
                "-1".to_string()
            } else {
                m["channel"].clone()
            };
            format!("{} {} {}", m["type"], m["called"], ch)
        })
        .collect()
}

fn eight() -> Vec<String> {
    [
        "GTC 5/1234 17",
        "ALH DUMMYI -1",
        "ACK 5/77 -1",
        "AHY 9/100 -1",
        "BCAST - 3",
        "BCAST - 9",
        "GTC 100/8000 1023",
        "ACKX ALLI -1",
    ]
    .into_iter()
    .map(String::from)
    .collect()
}

#[test]
fn recorded_vector_matches_expected() {
    let Some(rec) = sdrkit_dev_test::read_recording("trunk/mpt1327_cc_240k") else {
        return;
    };
    let exp = sdrkit_dev_test::sigmf::read_expected("trunk").unwrap();
    let e = &exp["recordings"]["mpt1327_cc_240k"];
    let mut rx = make(&MPT1327, &[("framing", "auto")]);
    configure(
        rx.as_mut(),
        rec.rate_hz,
        rec.centre_hz,
        e["signal_offset_hz"].as_f64().unwrap(),
    );
    let mut x = rec.samples.clone();
    x.extend(std::iter::repeat_n(Complex32::new(0.0, 0.0), 60000));
    let fed = feed(rx.as_mut(), &x, 4096);
    let r = results(rx.as_ref());
    assert_eq!(
        num(r, "system_identity"),
        e["final"]["system_identity"].as_f64().unwrap()
    );
    assert_eq!(num(r, "messages"), e["final"]["messages"].as_f64().unwrap());
    assert_eq!(num(r, "codewords_good"), 16.0);
    assert!(num(r, "codewords_bad") <= 2.0);
    let list = rows(r, &MPT1327_MESSAGES);
    let want = e["messages"].as_array().unwrap();
    assert_eq!(list.len(), want.len());
    for (m, w) in list.iter().zip(want) {
        let ch = if m["channel"].is_empty() {
            -1
        } else {
            m["channel"].parse().unwrap()
        };
        assert_eq!(m["type"], w["type"].as_str().unwrap());
        assert_eq!(m["called"], w["called"].as_str().unwrap());
        assert_eq!(m["caller"], w["caller"].as_str().unwrap());
        assert_eq!(ch, w["channel"].as_i64().unwrap());
        assert_eq!(m["meaning"], w["meaning"].as_str().unwrap());
        let t: f64 = m["time_s"].parse().unwrap();
        assert!(
            (t - w["t"].as_f64().unwrap()).abs() <= 0.02,
            "{t} vs {}",
            w["t"]
        );
    }
    let chans = rows(r, &MPT1327_CHANNELS);
    for w in e["channel_table"].as_array().unwrap() {
        let c = chans
            .iter()
            .find(|c| c["channel"] == w["channel"].to_string().as_str())
            .expect("channel row");
        assert_eq!(c["seen"], w["seen"].to_string());
        let used = w["use"].as_str().unwrap().split(' ').next().unwrap();
        assert_eq!(c["use"], used);
        assert_eq!(c["last_called"], w["last_called"].as_str().unwrap());
        assert_eq!(c["last_caller"], w["last_caller"].as_str().unwrap());
    }
    assert_eq!(r.status().lock_state, sdrkit_core::LockState::Locked);
    check_events(&kinds(&mpt::declaration()), &fed.events, true);
}

#[test]
fn channel_frequencies() {
    let f = channel_frequency;
    assert_eq!(f(100, 100, 150e6, 12.5e3), Some(150e6));
    assert!((f(104, 100, 150e6, 12.5e3).unwrap() - 150.05e6).abs() < 1.0);
    assert!((f(96, 100, 150e6, 12.5e3).unwrap() - 149.95e6).abs() < 1.0);
    assert_eq!(f(-1, 100, 150e6, 12.5e3), None);
    assert_eq!(f(1, -1, 150e6, 12.5e3), None);
    assert_eq!(f(0, 1000, 1e6, 12.5e3), None);
}

#[test]
fn the_eight_messages_at_several_audio_rates_and_polarities() {
    for rate in [24000.0, 48000.0, 44100.0, 12000.0] {
        for swap in [false, true] {
            for lead in [0usize, 7] {
                let s = MptSignal {
                    swap,
                    ..Default::default()
                };
                let mut a = vec![0.0f32; lead];
                a.extend(s.audio(rate));
                let r = run_audio(&a, rate, "auto");
                assert_eq!(types(&r), eight(), "rate {rate} swap {swap} lead {lead}");
                assert_eq!(
                    num(&r, "codewords_bad"),
                    1.0,
                    "only the word after the last CCSC fails"
                );
                assert_eq!(num(&r, "system_identity"), 20001.0);
            }
        }
    }
}

#[test]
fn the_eight_messages_through_the_standard_impairments() {
    let run = Harness::new(&MPT1327)
        .rate(240e3)
        .seconds(MptSignal::default().audio(240e3).len() as f64 / 240e3)
        .set(&GEN_NOISE, NoiseKind::Snr)
        .set(&GEN_SNR_DB, 15.0)
        .set(&GEN_OFFSET_HZ, 600.0)
        .run()
        .unwrap();
    assert_eq!(types(&run.results), eight());
}

#[test]
fn ffsk_resampler_rejects_an_alias() {
    // FFSK at 48 kHz with a strong tone at 22.5 kHz: decimating to 24 kHz without a filter folds it onto 1.5 kHz, between the
    // two FFSK tones.
    let rate = 48000.0;
    let a: Vec<f32> = MptSignal::default()
        .audio(rate)
        .iter()
        .enumerate()
        .map(|(i, v)| v + 0.8 * (std::f64::consts::TAU * 22500.0 * i as f64 / rate).sin() as f32)
        .collect();
    assert_eq!(types(&run_audio(&a, rate, "auto")), eight());
}

#[test]
fn mpt_codeword_repair_is_the_shared_table_decoder() {
    let mut r = sdrkit_algo_sim::rng::Rng::new(9);
    for _ in 0..200 {
        let cw = encode(r.next_u64());
        assert_eq!(check_and_repair(cw), (cw, Check::Good));
        let a = r.below(64);
        let one = cw ^ (1 << a);
        // The repair is the syndrome table of sdrkit-algo-fec's MPT 1327 code, not a search.
        assert_eq!(code().decode(one as u128), Some((cw as u128, 1)));
        assert_eq!(check_and_repair(one), (cw, Check::Repaired));
        let mut b = r.below(64);
        while b == a {
            b = r.below(64);
        }
        assert_eq!(check_and_repair(one ^ (1 << b)).1, Check::Failed);
    }
}

#[test]
fn a_grant_is_followed_through_the_host() {
    let mut rx = make(&MPT1327, &[("follow", "true"), ("control_channel", "3")]);
    configure(rx.as_mut(), 240e3, 170e6, 0.0);
    let x = MptSignal::default().iq();
    let fed = feed(rx.as_mut(), &x, 4096);
    let follows: Vec<&HostRequest> = fed
        .requests
        .iter()
        .filter(|r| matches!(r, HostRequest::Follow { .. }))
        .collect();
    // GTC to channel 17 with the control channel 3 at 170 MHz: 170 MHz + 14 × 12.5 kHz.
    match follows.first() {
        Some(HostRequest::Follow {
            freq_hz, signal, ..
        }) => {
            assert!((freq_hz - 170.175e6).abs() < 1.0, "{freq_hz}");
            assert_eq!(*signal, Some(SignalType::NarrowFmVoice));
        }
        other => panic!("no follow: {other:?}"),
    }
    for r in &fed.requests {
        r.validate().unwrap();
    }
    check_events(&kinds(&mpt::declaration()), &fed.events, true);
}

#[test]
fn any_cut_returns_the_same_messages() {
    let a = MptSignal::default().audio(24000.0);
    assert_eq!(
        types(&run_audio(&a, 24000.0, "auto")),
        types(&{
            let mut rx = audio_rx("auto");
            rx.configure(&InputInfo::new(24000.0, 0.0, 0.0).recording())
                .unwrap();
            feed(&mut rx, &audio_iq(&a), 333);
            feed(&mut rx, &audio_iq(&vec![0.0; 12000]), 333);
            Owner::results(&rx).clone()
        })
    );
}

/// Good codewords of the eight messages sent three times, counted after 1.7 s (the follow-up's settling time), at `snr_db` in
/// 12.5 kHz (no noise for `None`), the carrier `offset_hz` from the channel centre, `dev_hz` peak deviation.
fn late_codewords(snr_db: Option<f64>, offset_hz: f64, dev_hz: f64, seed: u64) -> f64 {
    let rate = 96e3;
    let s = MptSignal {
        rate,
        offset_hz,
        dev_per_unit: dev_hz / 0.5,
        pad_s: 0.2,
        codewords: MptSignal::default().codewords.repeat(3),
        ..Default::default()
    };
    let mut x = s.iq();
    if let Some(snr) = snr_db {
        add_noise_snr(&mut x, rate, snr, 12.5e3, seed);
    }
    let split = (1.7 * rate) as usize;
    let mut rx = make(&MPT1327, &[]);
    configure(rx.as_mut(), rate, 170e6, 0.0);
    feed(rx.as_mut(), &x[..split], 4096);
    let first = num(results(rx.as_ref()), "codewords_good");
    feed(rx.as_mut(), &x[split..], 4096);
    feed(rx.as_mut(), &vec![Complex32::new(0.0, 0.0); 24000], 4096);
    num(results(rx.as_ref()), "codewords_good") - first
}

#[test]
fn share_of_clean_codewords_in_noise_and_with_a_carrier_error() {
    // Required shares (%) at 10, 8, 6 and 4 dB in 12.5 kHz, 8 noise seeds.
    let mut lines = Vec::new();
    let mut short = Vec::new();
    for (offset, want) in [
        (0.0, [70.0, 64.0, 56.0, 25.0]),
        (3100.0, [61.0, 57.0, 42.0, 10.0]),
        (-5000.0, [54.0, 45.0, 17.0, 1.0]),
    ] {
        let clean = late_codewords(None, offset, 1500.0, 0);
        assert!(
            clean >= 20.0,
            "carrier {offset} Hz: {clean} codewords without noise"
        );
        for (snr, want) in [10.0, 8.0, 6.0, 4.0].into_iter().zip(want) {
            let got = 100.0
                * (1..=8)
                    .map(|s| late_codewords(Some(snr), offset, 1500.0, s))
                    .sum::<f64>()
                / (8.0 * clean);
            lines.push(format!(
                "carrier {offset} Hz {snr} dB: {got:.0} % (at least {want} %)"
            ));
            if got < want {
                short.push(lines.last().cloned().unwrap_or_default());
            }
        }
    }
    eprintln!("{}", lines.join("\n"));
    assert!(short.is_empty(), "{short:?}");
}

#[test]
fn five_and_six_khz_carrier_errors_at_any_deviation() {
    // 10 dB in 12.5 kHz, deviation 1.3 to 2.7 kHz: at -5 kHz at least 54 % of the clean run's codewords.
    for dev in [1300.0, 2000.0, 2700.0] {
        for offset in [-5000.0, 6000.0] {
            let clean = late_codewords(None, offset, dev, 0);
            let got = 100.0
                * (1..=8)
                    .map(|s| late_codewords(Some(10.0), offset, dev, s))
                    .sum::<f64>()
                / (8.0 * clean.max(1.0));
            eprintln!("deviation {dev} Hz, carrier {offset} Hz: {got:.0} % of {clean}");
            if offset < 0.0 {
                assert!(got >= 54.0, "deviation {dev} Hz: {got:.0} %");
            }
        }
    }
}

#[test]
fn no_wrong_result_over_rates_offsets_deviations_polarities_and_clocks() {
    let mut cases = 0;
    let mut wrong = Vec::new();
    for rate in [48e3, 96e3, 250e3] {
        for offset in [-5000.0, 0.0, 3100.0] {
            for dev in [1300.0, 2000.0, 2700.0] {
                for swap in [false, true] {
                    for ppm in [-100.0, -30.0, 30.0, 100.0] {
                        let s = MptSignal {
                            rate,
                            offset_hz: offset,
                            dev_per_unit: dev / 0.5,
                            swap,
                            pad_s: 1.0,
                            ..Default::default()
                        };
                        let mut x = Vec::new();
                        ClockError::new(rate, ppm).process(&s.iq(), &mut x);
                        let mut rx = make(&MPT1327, &[]);
                        configure(rx.as_mut(), rate, 170e6, 0.0);
                        feed(rx.as_mut(), &x, 4096);
                        let what = format!(
                            "rate {rate} offset {offset} deviation {dev} swap {swap} clock {ppm}"
                        );
                        let got = types(results(rx.as_ref()));
                        if got != eight() {
                            wrong.push(format!("{what}: {got:?}"));
                        }
                        cases += 1;
                    }
                }
            }
        }
    }
    assert_eq!(cases, 216);
    assert!(
        wrong.is_empty(),
        "{} cases:\n{}",
        wrong.len(),
        wrong.join("\n")
    );
}
