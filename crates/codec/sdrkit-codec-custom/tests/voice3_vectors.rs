//! V3 and V1 against the codec vectors.

mod common;

use sdrkit_codec_custom::voice3::{Decoder, Encoder, V3Rate, Variant, FRAME_SAMPLES};

const CASES: &[(&str, &str, Variant)] = &[
    ("kv3/syn16_v3_12", "syn16", Variant::V3(V3Rate::R12)),
    ("kv3/syn16_v3_16", "syn16", Variant::V3(V3Rate::R16)),
    ("kv3/syn16_v3_24", "syn16", Variant::V3(V3Rate::R24)),
    ("kv3/syn16_v3_32", "syn16", Variant::V3(V3Rate::R32)),
    ("kv3/real16_v3_12", "real16", Variant::V3(V3Rate::R12)),
    ("kv3/real16_v3_16", "real16", Variant::V3(V3Rate::R16)),
    ("kv3/real16_v3_24", "real16", Variant::V3(V3Rate::R24)),
    ("kv3/real16_v3_32", "real16", Variant::V3(V3Rate::R32)),
    ("kv3/tone3_v3_12", "tone3", Variant::V3(V3Rate::R12)),
    ("kv3/tone3_v3_16", "tone3", Variant::V3(V3Rate::R16)),
    ("kv3/tone3_v3_24", "tone3", Variant::V3(V3Rate::R24)),
    ("kv3/tone3_v3_32", "tone3", Variant::V3(V3Rate::R32)),
    ("kv1/syn16_v1_16", "syn16", Variant::V1),
    ("kv1/real16_v1_16", "real16", Variant::V1),
    ("kv1/tone1_v1_16", "tone1", Variant::V1),
];

fn encode_all(pcm: &[i16], v: Variant) -> Vec<u8> {
    let frames = (pcm.len() + 640).div_ceil(FRAME_SAMPLES);
    let mut x = pcm.to_vec();
    x.resize(frames * FRAME_SAMPLES, 0);
    let mut enc = Encoder::new(v);
    let mut out = Vec::new();
    let mut buf = vec![0u8; enc.frame_bytes()];
    for f in 0..frames {
        enc.encode(&x[f * FRAME_SAMPLES..(f + 1) * FRAME_SAMPLES], &mut buf)
            .unwrap();
        out.extend_from_slice(&buf);
    }
    out
}

fn decode_all(bits: &[u8], v: Variant) -> Vec<i16> {
    let mut dec = Decoder::new(v);
    let mut out = Vec::new();
    let mut buf = [0i16; FRAME_SAMPLES];
    for f in bits.chunks(v.frame_bytes()) {
        dec.decode(Some(f), &mut buf).unwrap();
        out.extend_from_slice(&buf);
    }
    out
}

fn hex(s: &str) -> Vec<u8> {
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap())
        .collect()
}

fn tone3() -> Vec<i16> {
    let mut s: u64 = 12345;
    (0..16000)
        .map(|i| {
            let t = i as f64 / 16000.0;
            let env = 0.6 + 0.4 * (2.0 * std::f64::consts::PI * 3.0 * t).sin();
            s = (s.wrapping_mul(1_103_515_245).wrapping_add(12345)) & 0x7FFF_FFFF;
            let noise = ((s >> 16) % 1201) as f64 - 600.0;
            let pi2 = 2.0 * std::f64::consts::PI;
            ((env
                * (4000.0 * (pi2 * 200.0 * t).sin()
                    + 2500.0 * (pi2 * 600.0 * t).sin()
                    + 1500.0 * (pi2 * 2400.0 * t).sin()))
                + noise)
                .trunc() as i16
        })
        .collect()
}

fn tone1() -> Vec<i16> {
    (0..16000)
        .map(|i| {
            let pi = std::f64::consts::PI;
            let a = (2.0 * pi * 440.0 * i as f64 / 16000.0).sin();
            let b = (2.0 * pi * 1250.0 * i as f64 / 16000.0).sin();
            (6000.0 * a + 3000.0 * b).trunc() as i16
        })
        .collect()
}

// Section 6.4 item 4: the first two V3 12 kbit/s frames and the first V1 frame of the tone signals.
#[test]
fn golden_frames_from_the_spec_text() {
    let b = encode_all(&tone3(), Variant::V3(V3Rate::R12));
    assert_eq!(
        &b[..30],
        hex("c9bde55533216afb881a308ef90e19ba6edad1099605a412b0658b1a0000")
    );
    assert_eq!(
        &b[30..60],
        hex("ca312fdabba2fbf8b92fb013ac17729b043219571746881507db3b5a0000")
    );
    let b = encode_all(&tone1(), Variant::V1);
    assert_eq!(
        &b[..40],
        hex("0cdb1fdfc0788b32a686f8835af6e258614fffba41b0f6e5bf1cd0a769637d6d2f691f75245f5600")
    );
}

#[test]
fn tone_signals_match_the_input_files() {
    let Some(dir) = common::vectors() else { return };
    assert_eq!(common::read_wav(&dir.join("input/tone3.wav")).1, tone3());
    assert_eq!(common::read_wav(&dir.join("input/tone1.wav")).1, tone1());
}

#[test]
fn encoder_and_decoder_match_every_vector() {
    let Some(dir) = common::vectors() else {
        eprintln!("codec vectors not found, test skipped");
        return;
    };
    let mut fails = Vec::new();
    for &(stem, input, v) in CASES {
        let want = std::fs::read(dir.join(format!("{stem}.bits"))).unwrap();
        let (_, pcm) = common::read_wav(&dir.join(format!("input/{input}.wav")));
        let got = encode_all(&pcm, v);
        let fb = v.frame_bytes();
        let enc_ok = got == want;
        let first = got
            .chunks(fb)
            .zip(want.chunks(fb))
            .position(|(a, b)| a != b);
        let (_, dec_want) = common::read_wav(&dir.join(format!("{stem}.dec.wav")));
        let dec = decode_all(&want, v);
        let maxd = dec
            .iter()
            .zip(&dec_want)
            .map(|(a, b)| (*a as i32 - *b as i32).abs())
            .max()
            .unwrap_or(0);
        let dec_ok = dec.len() == dec_want.len() && maxd <= 1;
        eprintln!(
            "{stem:20} encoder {} (first diff frame {first:?}), decoder max diff {maxd} LSB",
            if enc_ok { "exact" } else { "differs" }
        );
        if !enc_ok || !dec_ok {
            fails.push(stem);
        }
    }
    assert!(fails.is_empty(), "{fails:?}");
}

// Section 6.4 item 5: frames of the wrong profile, the wrong size or a reserved type are concealed.
#[test]
fn bad_frames_are_concealed() {
    use sdrkit_codec_custom::voice3::Decoded;
    let b16 = encode_all(&tone3(), Variant::V3(V3Rate::R16));
    let b24 = encode_all(&tone3(), Variant::V3(V3Rate::R24));
    let mut dec = Decoder::new(Variant::V3(V3Rate::R16));
    let mut out = [0i16; FRAME_SAMPLES];
    assert_eq!(
        dec.decode(Some(&b16[..40]), &mut out).unwrap(),
        Decoded::Good
    );
    assert_eq!(
        dec.decode(Some(&b24[60..100]), &mut out).unwrap(),
        Decoded::Concealed
    );
    assert_eq!(
        dec.decode(Some(&[0u8]), &mut out).unwrap(),
        Decoded::Concealed
    );
    let b1 = encode_all(&tone1(), Variant::V1);
    let mut dec = Decoder::new(Variant::V1);
    assert_eq!(
        dec.decode(Some(&b1[..40]), &mut out).unwrap(),
        Decoded::Good
    );
    assert_eq!(
        dec.decode(Some(&[0xFF; 40]), &mut out).unwrap(),
        Decoded::Concealed
    );
    assert_eq!(
        dec.decode(Some(&[0u8]), &mut out).unwrap(),
        Decoded::Concealed
    );
    assert_eq!(dec.decode(None, &mut out).unwrap(), Decoded::Concealed);
}

#[test]
fn random_frames_never_panic() {
    let mut s = 99u32;
    let mut next = || {
        s = s.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        (s >> 24) as u8
    };
    let mut out = [0i16; FRAME_SAMPLES];
    for v in [
        Variant::V3(V3Rate::R12),
        Variant::V3(V3Rate::R16),
        Variant::V3(V3Rate::R24),
        Variant::V3(V3Rate::R32),
        Variant::V1,
    ] {
        let mut dec = Decoder::new(v);
        for _ in 0..300 {
            let f: Vec<u8> = (0..v.frame_bytes()).map(|_| next()).collect();
            dec.decode(Some(&f), &mut out).unwrap();
        }
    }
}

fn snr_db(x: &[i16], y: &[i16]) -> f64 {
    let s: f64 = x.iter().map(|&v| (v as f64).powi(2)).sum();
    let e: f64 = x
        .iter()
        .zip(y)
        .map(|(&a, &b)| (a as f64 - b as f64).powi(2))
        .sum();
    10.0 * (s / e.max(1e-9)).log10()
}

// Section 7: round-trip SNR of tone3 at every rate (samples 2000 to 13999) and of tone1 (15000 samples from 640), aligned by 320.
#[test]
fn round_trip_snr() {
    let t3 = tone3();
    for r in [V3Rate::R12, V3Rate::R16, V3Rate::R24, V3Rate::R32] {
        let v = Variant::V3(r);
        let y = decode_all(&encode_all(&t3, v), v);
        let snr = snr_db(&t3[2000..14000], &y[2320..14320]);
        assert!(snr > 10.0, "{r:?}: {snr}");
    }
    let t1 = tone1();
    let y = decode_all(&encode_all(&t1, Variant::V1), Variant::V1);
    let snr = snr_db(&t1[640..15640], &y[960..15960]);
    assert!(snr > 8.0, "V1: {snr}");
}
