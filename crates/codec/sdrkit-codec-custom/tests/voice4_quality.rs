//! Voice codec 4 quality and behaviour floors: segSNRlo per profile, output peak, delay, DTX, digital
//! silence, damaged classes, bit errors and garbage input.

mod common;

use sdrkit_codec_custom::voice4::{
    Config, Damage, Decoded, Decoder, DecoderSettings, Encoded, Encoder, EncoderSettings,
    FrameLength, Input, Profile, SampleRate,
};

fn cfg(rate: u32, ms: u32, profile: Profile, ib: bool) -> Config {
    Config {
        rate: if rate == 16000 {
            SampleRate::Hz16000
        } else {
            SampleRate::Hz8000
        },
        frame: if ms == 20 {
            FrameLength::Ms20
        } else {
            FrameLength::Ms40
        },
        profile,
        type_in_band: ib,
    }
}

/// Encodes `pcm` and returns the frames (kind and bytes).
fn encode(c: Config, s: EncoderSettings, pcm: &[i16]) -> Vec<(Encoded, Vec<u8>)> {
    let mut enc = Encoder::new(c, s).unwrap();
    let n = enc.frame_samples();
    let frames = (pcm.len() + enc.delay() + n).div_ceil(n);
    let mut x = pcm.to_vec();
    x.resize(frames * n, 0);
    let mut buf = [0u8; 128];
    (0..frames)
        .map(|f| {
            let e = enc.encode(&x[f * n..(f + 1) * n], &mut buf).unwrap();
            (e, buf[..e.len()].to_vec())
        })
        .collect()
}

/// Decodes frames. `damage(i)` returns the input change for frame i: None for a lost frame, or the damage flags.
fn decode(
    c: Config,
    frames: &[(Encoded, Vec<u8>)],
    mut damage: impl FnMut(usize, &mut Vec<u8>) -> Option<Damage>,
) -> Vec<i16> {
    let mut dec = Decoder::new(c, DecoderSettings::PLAIN).unwrap();
    let n = dec.frame_samples();
    let mut out = Vec::new();
    let mut buf = vec![0i16; n];
    for (i, (_, bytes)) in frames.iter().enumerate() {
        let mut b = bytes.clone();
        match damage(i, &mut b) {
            None => dec
                .decode(Input::Lost, Damage::default(), &mut buf)
                .unwrap(),
            Some(d) => dec.decode(Input::Frame(&b), d, &mut buf).unwrap(),
        };
        out.extend_from_slice(&buf);
    }
    out
}

fn clean(_: usize, _: &mut Vec<u8>) -> Option<Damage> {
    Some(Damage::default())
}

fn peak(x: &[i16]) -> i32 {
    x.iter().map(|&v| (v as i32).abs()).max().unwrap_or(0)
}

#[test]
fn segsnr_floors_per_profile() {
    let Some(dir) = common::vectors() else { return };
    let (_, syn16) = common::read_wav(&dir.join("input/syn16.wav"));
    let (_, syn8) = common::read_wav(&dir.join("input/syn8.wav"));
    let rows = [
        (16000, 40, Profile::P3_2, 4.5),
        (16000, 40, Profile::P4_8, 6.5),
        (16000, 40, Profile::P6_0, 8.0),
        (16000, 40, Profile::P8_0, 9.0),
        (16000, 20, Profile::P4_8, 6.0),
        (16000, 20, Profile::P6_0, 7.5),
        (16000, 20, Profile::P8_0, 8.5),
        (16000, 20, Profile::Hq12, 11.0),
        (16000, 20, Profile::Hq16, 12.5),
        (8000, 40, Profile::P6_0, 8.0),
        (8000, 40, Profile::Hq16, 12.5),
    ];
    for (rate, ms, profile, floor) in rows {
        let input = if rate == 16000 { &syn16 } else { &syn8 };
        let c = cfg(rate, ms, profile, false);
        let frames = encode(c, EncoderSettings::default(), input);
        let out = decode(c, &frames, clean);
        let delay = if rate == 16000 { 173 } else { 40 };
        let snr = common::seg_snr_lo(input, &out, rate, delay);
        eprintln!("{rate} {ms} {profile:?}: segSNRlo {snr:.2} dB (floor {floor})");
        assert!(snr >= floor, "{profile:?} {ms} {rate}: {snr}");
        assert!((peak(&out) as f64) < 1.6 * peak(input) as f64);
    }
}

#[test]
fn measured_delay_matches() {
    let Some(dir) = common::vectors() else { return };
    let (_, syn16) = common::read_wav(&dir.join("input/syn16.wav"));
    let c = cfg(16000, 20, Profile::P6_0, false);
    let out = decode(c, &encode(c, EncoderSettings::default(), &syn16), clean);
    let mut best = (0, f64::MIN);
    for d in 0..1200usize {
        let mut s = 0f64;
        for i in (0..syn16.len()).step_by(2) {
            if i + d < out.len() {
                s += syn16[i] as f64 * out[i + d] as f64;
            }
        }
        if s > best.1 {
            best = (d, s);
        }
    }
    assert!(best.0.abs_diff(173) <= 2, "delay {}", best.0);
}

#[test]
fn dtx_frame_shares_and_silence() {
    let Some(dir) = common::vectors() else { return };
    let (_, x) = common::read_wav(&dir.join("input/test_dtx.wav"));
    for ms in [20, 40] {
        for ib in [false, true] {
            let c = cfg(16000, ms, Profile::P6_0, ib);
            let frames = encode(
                c,
                EncoderSettings {
                    dtx: true,
                    noise_suppressor: false,
                },
                &x,
            );
            let n = frames.len() as f64;
            let sid = frames
                .iter()
                .filter(|f| matches!(f.0, Encoded::Sid(_)))
                .count() as f64;
            let speech = frames
                .iter()
                .filter(|f| matches!(f.0, Encoded::Speech(_)))
                .count() as f64;
            let none = frames
                .iter()
                .filter(|f| matches!(f.0, Encoded::NoData))
                .count() as f64;
            assert!(
                none > (n - speech) / 3.0,
                "{ms} {ib}: no-data {none} of {n}"
            );
            assert!(sid > 2.0 && sid < n / 8.0, "{ms} {ib}: sid {sid} of {n}");
            assert!(
                speech > n / 3.0 && speech < 0.75 * n,
                "{ms} {ib}: speech {speech} of {n}"
            );
            let out = decode(c, &frames, clean);
            assert_eq!(out.len(), frames.len() * (16 * ms as usize));
        }
    }
    // Digital silence: at most a tenth of the frames sent, output peak below 64.
    let c = cfg(16000, 20, Profile::P6_0, false);
    let frames = encode(
        c,
        EncoderSettings {
            dtx: true,
            noise_suppressor: false,
        },
        &[0; 32000],
    );
    let sent = frames.iter().filter(|f| !f.0.is_empty()).count();
    assert!(sent * 10 <= frames.len(), "{sent} of {}", frames.len());
    assert!(peak(&decode(c, &frames, clean)) < 64);
}

#[test]
fn damaged_classes_and_losses() {
    let Some(dir) = common::vectors() else { return };
    let (_, x) = common::read_wav(&dir.join("input/test_male4.wav"));
    let c = cfg(16000, 40, Profile::P6_0, true);
    let frames = encode(c, EncoderSettings::default(), &x);
    let snr = |out: &[i16]| common::seg_snr_lo(&x, out, 16000, 173);
    let clean_out = decode(c, &frames, clean);
    let erased = decode(c, &frames, |i, _| {
        if i % 10 == 5 {
            None
        } else {
            Some(Damage::default())
        }
    });
    let class_c = decode(c, &frames, |i, _| {
        Some(Damage {
            c: i % 10 == 5,
            ..Damage::default()
        })
    });
    let class_bc = decode(c, &frames, |i, _| {
        Some(Damage {
            b: i % 10 == 5,
            c: i % 10 == 5,
            ..Damage::default()
        })
    });
    let class_a = decode(c, &frames, |i, _| {
        Some(Damage {
            a: i % 10 == 5,
            ..Damage::default()
        })
    });
    eprintln!(
        "clean {:.2}, erased {:.2}, class C {:.2}, class B+C {:.2}",
        snr(&clean_out),
        snr(&erased),
        snr(&class_c),
        snr(&class_bc)
    );
    assert!(snr(&class_c) > snr(&erased));
    assert_eq!(class_a, erased);
    for pct in [1, 3, 10, 20] {
        let out = decode(c, &frames, |i, _| {
            if (i * 37 + 11) % 100 < pct {
                None
            } else {
                Some(Damage::default())
            }
        });
        assert!((peak(&out) as f64) < 1.6 * peak(&x) as f64, "{pct} %");
        let near = out.iter().filter(|&&v| (v as i32).abs() > 32000).count();
        assert!(near * 1000 < out.len());
    }
    // Bit errors at 1e-2 of the payload bits.
    let mut s = 1u32;
    let out = decode(c, &frames, |_, b| {
        for byte in b.iter_mut() {
            for k in 0..8 {
                s = s.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
                if s >> 24 < 3 {
                    *byte ^= 1 << k;
                }
            }
        }
        Some(Damage::default())
    });
    let near = out.iter().filter(|&&v| (v as i32).abs() > 32000).count();
    assert!(near * 100 < out.len());
}

#[test]
fn garbage_then_a_valid_stream_recovers() {
    let Some(dir) = common::vectors() else { return };
    let (_, x) = common::read_wav(&dir.join("input/test_male4.wav"));
    for ib in [false, true] {
        let c = cfg(16000, 20, Profile::P6_0, ib);
        let frames = encode(c, EncoderSettings::default(), &x);
        let mut dec = Decoder::new(c, DecoderSettings::PLAIN).unwrap();
        let n = dec.frame_samples();
        let mut buf = vec![0i16; n];
        let mut s = 7u32;
        for i in 0..300 {
            let len = [0usize, 1, 5, 15, 16, 40][i % 6];
            let g: Vec<u8> = (0..len)
                .map(|_| {
                    s = s.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
                    (s >> 24) as u8
                })
                .collect();
            dec.decode(Input::Frame(&g), Damage::default(), &mut buf)
                .unwrap();
        }
        let mut out = Vec::new();
        for (_, b) in &frames {
            dec.decode(Input::Frame(b), Damage::default(), &mut buf)
                .unwrap();
            out.extend_from_slice(&buf);
        }
        let clean_out = decode(c, &frames, clean);
        let snr = common::seg_snr(
            &clean_out[16000..]
                .iter()
                .map(|&v| v as f64)
                .collect::<Vec<_>>(),
            &out[16000..].iter().map(|&v| v as f64).collect::<Vec<_>>(),
            16000,
        );
        assert!(snr > 15.0, "ib {ib}: {snr}");
    }
}

#[test]
fn decoder_reports_frame_types() {
    let c = cfg(16000, 40, Profile::P6_0, true);
    let mut dec = Decoder::new(c, DecoderSettings::PLAIN).unwrap();
    let mut buf = vec![0i16; dec.frame_samples()];
    assert_eq!(
        dec.decode(Input::Lost, Damage::default(), &mut buf)
            .unwrap(),
        Decoded::Concealed
    );
    assert_eq!(
        dec.decode(Input::Frame(&[0x80; 30]), Damage::default(), &mut buf)
            .unwrap(),
        Decoded::Sid
    );
    assert_eq!(
        dec.decode(Input::Frame(&[]), Damage::default(), &mut buf)
            .unwrap(),
        Decoded::ComfortNoise
    );
    assert_eq!(
        dec.decode(Input::Frame(&[0; 30]), Damage::default(), &mut buf)
            .unwrap(),
        Decoded::Speech
    );
    assert!(dec
        .decode(Input::Lost, Damage::default(), &mut buf[..10])
        .is_err());
}

#[test]
fn custom_budgets_round_trip() {
    let Some(dir) = common::vectors() else { return };
    let (_, syn16) = common::read_wav(&dir.join("input/syn16.wav"));
    let (_, syn8) = common::read_wav(&dir.join("input/syn8.wav"));
    for (rate, ms, bits, input) in [
        (16000, 20, 200u16, &syn16),
        (8000, 40, 140, &syn8),
        (16000, 40, 640, &syn16),
    ] {
        let c = cfg(rate, ms, Profile::Custom(bits), true);
        let frames = encode(c, EncoderSettings::default(), input);
        assert!(frames
            .iter()
            .all(|f| f.1.len() == (bits as usize).div_ceil(8)));
        let out = decode(c, &frames, clean);
        assert_eq!(
            out.len(),
            frames.len() * (rate as usize * ms as usize / 1000)
        );
        let delay = if rate == 16000 { 173 } else { 40 };
        let snr = common::seg_snr_lo(input, &out, rate, delay);
        eprintln!("custom {bits} bits {ms} ms {rate}: segSNRlo {snr:.2}");
        assert!(snr > 5.0, "custom {bits}: {snr}");
    }
    // 800 bits per 40 ms needs pulse indices of more than 63 bits.
    assert!(Encoder::new(
        cfg(16000, 40, Profile::Custom(800), true),
        EncoderSettings::default()
    )
    .is_err());
}
