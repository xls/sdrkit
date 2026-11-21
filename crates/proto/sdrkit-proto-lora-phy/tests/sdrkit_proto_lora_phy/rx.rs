//! Round trips through the modulator, impairments and the receivers.

use crate::common::{add_noise, run, AMP};
use sdrkit_algo_dsp::interp::Polyphase;
use sdrkit_core::Complex32;
use sdrkit_proto_lora_phy::bank::{Bank, BankChannel, BankPlan};
use sdrkit_proto_lora_phy::{ChannelRx, DetectorConfig, Ldro, LoraParams, TxFrame};

#[test]
fn every_sf_decodes_clean() {
    for sf in 5..=12u8 {
        let p = LoraParams::new(sf, 125e3, 5, 8, 0x12);
        let payload: Vec<u8> = (0..20).map(|i| i as u8 * 3 + sf).collect();
        let fr = run(1e6, 200e3, &p, &payload, 1500.0, 20.0, 0.0, 10_000);
        assert_eq!(fr.len(), 1, "sf{sf}");
        assert!(fr[0].crc_ok, "sf{sf}");
        assert_eq!(fr[0].payload, payload);
        assert!(
            (fr[0].cfo_hz - 1500.0).abs() < 100.0,
            "cfo {}",
            fr[0].cfo_hz
        );
        // The start is the first preamble chirp at 20 ms.
        assert!(
            (fr[0].start as f64 - 0.02 * 1e6).abs() < 2.0 * 1e6 / 125e3,
            "start {}",
            fr[0].start
        );
    }
}

#[test]
fn sf5_and_sf6_implicit_inverted_and_ldro() {
    for sf in [5u8, 6] {
        let mut p = LoraParams::new(sf, 250e3, 6, 8, 0x34);
        p.implicit = true;
        p.implicit_len = 12;
        let payload = b"implicit sf5".to_vec();
        let fr = run(1e6, 0.0, &p, &payload, -800.0, 15.0, 0.0, 4096);
        assert_eq!(fr.len(), 1, "sf{sf}");
        assert!(fr[0].crc_ok && fr[0].payload == payload && fr[0].implicit);
        assert_eq!(fr[0].sync, 0x34);
    }
    let mut p = LoraParams::new(9, 62.5e3, 7, 10, 0x12);
    p.invert_iq = true;
    p.ldro = Ldro::On;
    let fr = run(500e3, 50e3, &p, b"inverted", 300.0, 10.0, 0.0, 4096);
    assert_eq!(fr.len(), 1);
    assert!(fr[0].crc_ok && fr[0].inverted);
    // The other IQ direction hears nothing.
    p.invert_iq = false;
    let f = TxFrame::new(
        &LoraParams {
            invert_iq: true,
            ..p.clone()
        },
        b"x",
    )
    .unwrap()
    .at(0.01)
    .amplitude(AMP);
    let x = f.render(500e3, 0.05);
    let mut rx = ChannelRx::new(
        500e3,
        0.0,
        62.5e3,
        vec![DetectorConfig::new(9, 62.5e3, 0x12)],
    )
    .unwrap();
    let mut out = Vec::new();
    rx.process(&x, &mut out);
    assert!(out.is_empty());
}

fn settings3() -> Vec<LoraParams> {
    let mut sf12 = LoraParams::new(12, 125e3, 8, 16, 0x12);
    sf12.ldro = Ldro::On;
    vec![
        LoraParams::new(11, 250e3, 5, 16, 0x2B),
        LoraParams::new(8, 62.5e3, 8, 32, 0x12),
        sf12,
    ]
}

#[test]
fn carrier_offsets() {
    for p in settings3() {
        for cfo in [-20e3, -3e3, 12e3] {
            let payload: Vec<u8> = (0..40).map(|i| (i * 11) as u8).collect();
            let fr = run(1e6, 200e3, &p, &payload, cfo, 5.0, 0.0, 65_536);
            assert_eq!(fr.len(), 1, "{} cfo {cfo}", p.describe());
            assert!(
                fr[0].crc_ok && fr[0].payload == payload,
                "{} cfo {cfo}",
                p.describe()
            );
            let tol = 0.05 * p.bw / p.n() as f64 + 5.0;
            assert!(
                (fr[0].cfo_hz - cfo).abs() < tol,
                "{} cfo {cfo}: {}",
                p.describe(),
                fr[0].cfo_hz
            );
        }
    }
}

#[test]
fn clock_offsets() {
    for p in settings3() {
        for ppm in [-50.0, 30.0] {
            let len = if p.sf == 12 { 255 } else { 120 };
            let payload: Vec<u8> = (0..len).map(|i| (i * 13 + 1) as u8).collect();
            let fr = run(4.0 * p.bw, 0.0, &p, &payload, 1000.0, 10.0, ppm, 65_536);
            assert_eq!(fr.len(), 1, "{} ppm {ppm}", p.describe());
            assert!(
                fr[0].crc_ok && fr[0].payload == payload,
                "{} ppm {ppm}",
                p.describe()
            );
        }
    }
}

#[test]
fn sensitivity_near_the_limit() {
    let p7 = LoraParams::new(7, 125e3, 5, 8, 0x12);
    let mut p12 = LoraParams::new(12, 125e3, 5, 8, 0x12);
    p12.ldro = Ldro::On;
    for (p, snr) in [(p7, -6.0), (p12, -18.0)] {
        let mut ok = 0;
        for seed in 0..5u8 {
            let payload: Vec<u8> = (0..20).map(|i| i as u8 ^ seed).collect();
            let fr = run(
                1e6,
                100e3,
                &p,
                &payload,
                700.0 * seed as f64,
                snr,
                0.0,
                65_536,
            );
            if fr.iter().any(|f| f.crc_ok && f.payload == payload) {
                ok += 1;
            }
        }
        assert!(ok >= 4, "SF{} at {snr} dB: {ok} of 5", p.sf);
    }
}

#[test]
fn any_piece_size_returns_the_same_frames() {
    let p = LoraParams::new(9, 125e3, 6, 8, 0x12);
    let payload = b"cut independence".to_vec();
    let a = run(1e6, -150e3, &p, &payload, 2000.0, 0.0, 0.0, 1_000_000);
    for piece in [1usize, 333, 77_777] {
        let b = run(1e6, -150e3, &p, &payload, 2000.0, 0.0, 0.0, piece);
        assert_eq!(a, b, "piece {piece}");
    }
    assert_eq!(a.len(), 1);
}

/// Three channels with per-channel filters, and the same plan through the shared channeliser, give the same frames at the
/// same input indices.
#[test]
fn bank_with_and_without_the_shared_channeliser() {
    let rate = 2e6;
    let centre = 868e6;
    let freqs = [867.7e6, 868.1e6, 868.5e6];
    let params = [
        LoraParams::new(7, 125e3, 5, 8, 0x34),
        LoraParams::new(9, 125e3, 5, 8, 0x34),
        LoraParams::new(8, 250e3, 6, 8, 0x34),
    ];
    let mut scene = sdrkit_algo_sim::scene::Scene::new(rate)
        .unwrap()
        .with_duration(0.25)
        .unwrap();
    for (k, (f, p)) in freqs.iter().zip(&params).enumerate() {
        let payload = vec![k as u8 + 1; 10 + k];
        let fr = TxFrame::new(p, &payload)
            .unwrap()
            .at(0.01 + 0.03 * k as f64)
            .carrier(f - centre + 300.0)
            .amplitude(AMP);
        scene.add_source({
            let mut b = Some(fr.burst(rate));
            Box::new(move || b.take())
        });
    }
    let mut x = vec![Complex32::default(); (0.25 * rate) as usize];
    sdrkit_core::Generator::generate(&mut scene, &mut x);
    add_noise(&mut x, 10.0, 125e3, rate, 3);
    let plan = |fft_from| BankPlan {
        channels: freqs
            .iter()
            .zip(&params)
            .enumerate()
            .map(|(k, (f, p))| BankChannel {
                freq_hz: *f,
                bw: p.bw,
                detectors: vec![DetectorConfig {
                    tag: k as u32,
                    ..DetectorConfig::new(p.sf, p.bw, 0x34)
                }],
            })
            .collect(),
        fft_from,
    };
    let mut results = Vec::new();
    for fft_from in [0usize, 3] {
        let mut b = Bank::new(rate, centre, plan(fft_from), 0).unwrap();
        assert_eq!(b.in_band(), 3);
        let mut out = Vec::new();
        for c in x.chunks(50_000) {
            b.process(c, &mut out);
        }
        out.sort_by_key(|f| f.0.tag);
        assert_eq!(out.len(), 3, "fft_from {fft_from}");
        for (k, (f, _)) in out.iter().enumerate() {
            assert!(f.crc_ok);
            assert_eq!(f.payload, vec![k as u8 + 1; 10 + k]);
            assert_eq!(f.channel_hz, freqs[k]);
            let want = (0.01 + 0.03 * k as f64) * rate;
            assert!(
                (f.start as f64 - want).abs() < 0.25 * rate / params[k].bw,
                "start {} want {want}",
                f.start
            );
        }
        results.push(out);
    }
    for k in 0..3 {
        let (a, b) = (&results[0][k].0, &results[1][k].0);
        // Within half a chip of each other.
        assert!(
            (a.start as f64 - b.start as f64).abs() <= 0.5 * rate / params[k].bw,
            "{} vs {}",
            a.start,
            b.start
        );
    }
}

/// The interpolator of the chip rings is the windowed sinc the receiver was tuned with: its rows equal the formula
/// `0.5 sinc(0.5 x)` under a Kaiser window, normalised to unity gain.
#[test]
fn chip_interpolator_rows_match_the_formula() {
    const TAPS: usize = 32;
    let beta = sdrkit_algo_dsp::fir::design_beta(50.0);
    let p = Polyphase::<TAPS, 64>::new(0.5, beta).unwrap();
    let half = TAPS as f64 / 2.0;
    for ph in 0..=64 {
        let f = ph as f64 / 64.0;
        let mut row = [0f64; TAPS];
        let mut sum = 0.0;
        for (i, r) in row.iter_mut().enumerate() {
            let x = i as f64 - half + 1.0 - f;
            let s = if x.abs() < 1e-12 {
                0.5
            } else {
                (std::f64::consts::PI * 0.5 * x).sin() / (std::f64::consts::PI * x)
            };
            let wx = x / (half + 0.5);
            let w = if wx.abs() >= 1.0 {
                0.0
            } else {
                sdrkit_algo_dsp::window::bessel_i0(beta * (1.0 - wx * wx).sqrt())
                    / sdrkit_algo_dsp::window::bessel_i0(beta)
            };
            *r = s * w;
            sum += *r;
        }
        for (i, (&got, want)) in p.row(ph).iter().zip(row).enumerate() {
            assert!((got as f64 - want / sum).abs() < 1e-6, "phase {ph} tap {i}");
        }
    }
}

#[test]
fn bad_receiver_parameters_are_errors() {
    assert!(ChannelRx::new(100e3, 0.0, 125e3, vec![DetectorConfig::new(7, 125e3, 0x12)]).is_err());
    assert!(ChannelRx::new(1e6, f64::NAN, 125e3, vec![]).is_err());
    assert!(ChannelRx::new(1e6, 0.0, 125e3, vec![DetectorConfig::new(13, 125e3, 0x12)]).is_err());
    assert!(ChannelRx::new(
        1e6,
        0.0,
        125e3,
        vec![DetectorConfig {
            syncs: vec![],
            ..DetectorConfig::new(7, 125e3, 0x12)
        }]
    )
    .is_err());
    // Garbage input returns no frame and no panic.
    let mut rx =
        ChannelRx::new(1e6, 0.0, 125e3, vec![DetectorConfig::new(7, 125e3, 0x12)]).unwrap();
    let mut out = Vec::new();
    let x: Vec<Complex32> = (0..200_000)
        .map(|i| match i % 5 {
            0 => Complex32::new(f32::NAN, 0.0),
            1 => Complex32::new(1e30, -1e30),
            _ => Complex32::new((i as f32).sin(), 0.0),
        })
        .collect();
    rx.process(&x, &mut out);
    rx.reset().unwrap();
    rx.process(&x[..1000], &mut out);
    // A frame still decodes after a reset.
    let p = LoraParams::new(7, 125e3, 5, 8, 0x12);
    let y = TxFrame::new(&p, b"after")
        .unwrap()
        .at(0.01)
        .render(1e6, 0.02);
    out.clear();
    rx.reset().unwrap();
    rx.process(&y, &mut out);
    assert_eq!(out.len(), 1);
    assert_eq!(out[0].0.payload, b"after");
}

/// One frame with the carrier offset `cfo` and a carrier ramp of `ramp` Hz/s, into a receiver with or without Doppler tracking.
fn doppler_run(
    p: &LoraParams,
    payload: &[u8],
    cfo: f64,
    ramp: f64,
    doppler: bool,
) -> Vec<sdrkit_proto_lora_phy::LoraFrame> {
    let rate = 4.0 * p.bw;
    let mut f = TxFrame::new(p, payload)
        .unwrap()
        .at(0.02)
        .carrier(cfo)
        .amplitude(AMP);
    f.drift_hz_s = ramp;
    let mut x = f.render(rate, 0.05);
    add_noise(&mut x, 10.0, p.bw, rate, 11);
    let mut cfg = DetectorConfig::new(p.sf, p.bw, p.sync);
    cfg.doppler = doppler;
    cfg.carrier_hz = 437e6;
    let mut rx = ChannelRx::new(rate, 0.0, p.bw, vec![cfg]).unwrap();
    let mut out = Vec::new();
    for c in x.chunks(65_536) {
        rx.process(c, &mut out);
    }
    out.into_iter().map(|(f, _)| f).collect()
}

/// Doppler tracking: the carrier is followed during the frame, and the frame reports the carrier at its
/// end and the rate of change. A low-orbit pass at 437 MHz ramps by up to about 100 Hz/s.
#[test]
fn doppler_tracking_follows_a_carrier_ramp() {
    let p = LoraParams::new(10, 125e3, 5, 8, 0x12);
    let payload: Vec<u8> = (0..60).map(|i| (i * 5) as u8).collect();
    let ramp = -150.0;
    let fr = doppler_run(&p, &payload, 2000.0, ramp, true);
    assert_eq!(fr.len(), 1);
    assert!(fr[0].crc_ok && fr[0].payload == payload);
    // One bin is 122 Hz. Over a frame of about 0.6 s, a tolerance of a third of the ramp is 0.25 bin at the end.
    assert!(
        (fr[0].cfo_rate_hz_s - ramp).abs() < 50.0,
        "rate {}",
        fr[0].cfo_rate_hz_s
    );
    let dur = (fr[0].end - fr[0].start) as f64 / (4.0 * p.bw);
    assert!(
        fr[0].cfo_end_hz < fr[0].cfo_hz,
        "{} then {}",
        fr[0].cfo_hz,
        fr[0].cfo_end_hz
    );
    assert!(dur > 0.3);
    // Without Doppler tracking a small ramp still decodes.
    let fr = doppler_run(&p, &payload, 2000.0, -20.0, false);
    assert_eq!(fr.len(), 1);
    assert!(fr[0].crc_ok);
    // Without Doppler tracking the carrier loop of the data symbols follows the ramp.
    assert!(
        (fr[0].cfo_rate_hz_s + 20.0).abs() < 15.0,
        "rate {}",
        fr[0].cfo_rate_hz_s
    );
}
