//! Voice codec 4 against the codec vectors: the plain decoder sample for sample, and the encoder.

mod common;

use sdrkit_codec_custom::voice4::{
    Config, Damage, Decoder, DecoderSettings, FrameLength, Input, Profile, SampleRate,
};

/// One vector configuration: stem, input file, rate, frame ms, profile, dtx, type bit in band, noise suppressor, loss.
pub struct Vector {
    pub stem: &'static str,
    pub input: &'static str,
    pub rate: u32,
    pub ms: u32,
    pub profile: Profile,
    pub dtx: bool,
    pub ib: bool,
    pub ns: bool,
    pub loss: bool,
}

const fn v(
    stem: &'static str,
    input: &'static str,
    rate: u32,
    ms: u32,
    profile: Profile,
    flags: u8,
) -> Vector {
    Vector {
        stem,
        input,
        rate,
        ms,
        profile,
        dtx: flags & 1 != 0,
        ib: flags & 2 != 0,
        ns: flags & 4 != 0,
        loss: flags & 8 != 0,
    }
}

use Profile::*;

pub const VECTORS: &[Vector] = &[
    v("syn16_3.2_40", "syn16", 16000, 40, P3_2, 0),
    v("real16_3.2_40", "real16", 16000, 40, P3_2, 0),
    v("syn16_4.8_40", "syn16", 16000, 40, P4_8, 0),
    v("real16_4.8_40", "real16", 16000, 40, P4_8, 0),
    v("syn16_6.0_40", "syn16", 16000, 40, P6_0, 0),
    v("real16_6.0_40", "real16", 16000, 40, P6_0, 0),
    v("syn16_8.0_40", "syn16", 16000, 40, P8_0, 0),
    v("real16_8.0_40", "real16", 16000, 40, P8_0, 0),
    v("syn16_12_40", "syn16", 16000, 40, Hq12, 0),
    v("real16_12_40", "real16", 16000, 40, Hq12, 0),
    v("syn16_16_40", "syn16", 16000, 40, Hq16, 0),
    v("real16_16_40", "real16", 16000, 40, Hq16, 0),
    v("syn16_4.8_20", "syn16", 16000, 20, P4_8, 0),
    v("real16_4.8_20", "real16", 16000, 20, P4_8, 0),
    v("syn16_6.0_20", "syn16", 16000, 20, P6_0, 0),
    v("real16_6.0_20", "real16", 16000, 20, P6_0, 0),
    v("syn16_8.0_20", "syn16", 16000, 20, P8_0, 0),
    v("real16_8.0_20", "real16", 16000, 20, P8_0, 0),
    v("syn16_12_20", "syn16", 16000, 20, Hq12, 0),
    v("real16_12_20", "real16", 16000, 20, Hq12, 0),
    v("syn16_16_20", "syn16", 16000, 20, Hq16, 0),
    v("real16_16_20", "real16", 16000, 20, Hq16, 0),
    v("syn8_6.0_40", "syn8", 8000, 40, P6_0, 0),
    v("real8_6.0_40", "real8", 8000, 40, P6_0, 0),
    v("syn8_16_40", "syn8", 8000, 40, Hq16, 0),
    v("real8_16_40", "real8", 8000, 40, Hq16, 0),
    v("syn8_4.8_20", "syn8", 8000, 20, P4_8, 0),
    v("real8_4.8_20", "real8", 8000, 20, P4_8, 0),
    v("slot_noisy16_8.0", "noisy16", 16000, 40, P8_0, 3),
    v("slot_noisy16_6.0", "noisy16", 16000, 40, P6_0, 3),
    v("slot_noisy16_4.8", "noisy16", 16000, 40, P4_8, 3),
    v("slot_noisy16_3.2", "noisy16", 16000, 40, P3_2, 3),
    v("slot_pause16_8.0", "pause16", 16000, 40, P8_0, 3),
    v("slot_pause16_6.0", "pause16", 16000, 40, P6_0, 3),
    v("slot_pause16_4.8", "pause16", 16000, 40, P4_8, 3),
    v("slot_pause16_3.2", "pause16", 16000, 40, P3_2, 3),
    v("dtx_noisy16_12_20", "noisy16", 16000, 20, Hq12, 1),
    v("dtx_pause16_6.0_20", "pause16", 16000, 20, P6_0, 1),
    v("dtx_pause8_8.0_20", "pause8", 8000, 20, P8_0, 1),
    v("ns_noisy16_6.0_20", "noisy16", 16000, 20, P6_0, 4),
    v("loss13_syn16_6.0_40", "syn16", 16000, 40, P6_0, 8),
    v("loss13_slot_noisy16_6.0", "noisy16", 16000, 40, P6_0, 11),
];

pub fn config(x: &Vector) -> Config {
    Config {
        rate: if x.rate == 16000 {
            SampleRate::Hz16000
        } else {
            SampleRate::Hz8000
        },
        frame: if x.ms == 20 {
            FrameLength::Ms20
        } else {
            FrameLength::Ms40
        },
        profile: x.profile,
        type_in_band: x.ib,
    }
}

/// Splits a `.kv4` file into frames: (type byte, payload).
pub fn records(data: &[u8], speech_bytes: usize, sid_bytes: usize) -> Vec<(u8, Vec<u8>)> {
    let mut out = Vec::new();
    let mut i = 0;
    while i < data.len() {
        let t = data[i];
        i += 1;
        let n = match t {
            0 => speech_bytes,
            1 => sid_bytes,
            _ => 0,
        };
        out.push((t, data[i..i + n].to_vec()));
        i += n;
    }
    out
}

fn decode_vector(x: &Vector, settings: DecoderSettings) -> Option<(Vec<i16>, Vec<i16>)> {
    let dir = common::vectors()?;
    let kv4 = std::fs::read(dir.join(format!("kv4/{}.kv4", x.stem))).unwrap();
    let suffix = if settings == DecoderSettings::PLAIN {
        "plain"
    } else {
        "default"
    };
    let (_, want) = common::read_wav(&dir.join(format!("kv4/{}.{suffix}.wav", x.stem)));
    let mut dec = Decoder::new(config(x), settings).unwrap();
    let speech = dec.layout().speech_bytes();
    let sid = dec.layout().sid_bytes;
    let n = dec.frame_samples();
    let mut got = Vec::new();
    let mut buf = vec![0i16; n];
    for (i, (t, payload)) in records(&kv4, speech, sid).iter().enumerate() {
        let input = if x.loss && i % 13 == 7 {
            Input::Lost
        } else if *t == 2 {
            Input::Frame(&[])
        } else {
            Input::Frame(payload)
        };
        dec.decode(input, Damage::default(), &mut buf).unwrap();
        got.extend_from_slice(&buf);
    }
    Some((got, want))
}

fn compare(got: &[i16], want: &[i16], frame: usize) -> Result<(), String> {
    if got.len() != want.len() {
        return Err(format!("length {} want {}", got.len(), want.len()));
    }
    if let Some(i) = got.iter().zip(want).position(|(a, b)| a != b) {
        let bad = got.iter().zip(want).filter(|(a, b)| a != b).count();
        let maxd = got
            .iter()
            .zip(want)
            .map(|(a, b)| (*a as i32 - *b as i32).abs())
            .max()
            .unwrap();
        return Err(format!(
            "first diff at sample {i} (frame {}), {bad} samples differ, max {maxd}: got {} want {}",
            i / frame,
            got[i],
            want[i]
        ));
    }
    Ok(())
}

#[test]
fn plain_decoder_matches_every_vector() {
    if common::vectors().is_none() {
        eprintln!("codec vectors not found, test skipped");
        return;
    }
    let only = std::env::var("VOICE4_ONLY").ok();
    let mut fails = Vec::new();
    for x in VECTORS {
        if only.as_deref().is_some_and(|o| !x.stem.contains(o)) {
            continue;
        }
        let (got, want) = decode_vector(x, DecoderSettings::PLAIN).unwrap();
        let frame = (x.rate * x.ms / 1000) as usize;
        match compare(&got, &want, frame) {
            Ok(()) => eprintln!("{:28} exact", x.stem),
            Err(e) => {
                eprintln!("{:28} {e}", x.stem);
                fails.push(x.stem);
            }
        }
    }
    assert!(fails.is_empty(), "plain decode differs: {fails:?}");
}

fn encode_vector(x: &Vector) -> Option<(Vec<u8>, Vec<u8>)> {
    use sdrkit_codec_custom::voice4::{Encoded, Encoder, EncoderSettings};
    let dir = common::vectors()?;
    let want = std::fs::read(dir.join(format!("kv4/{}.kv4", x.stem))).unwrap();
    let (_, pcm) = common::read_wav(&dir.join(format!("input/{}.wav", x.input)));
    let mut enc = Encoder::new(
        config(x),
        EncoderSettings {
            dtx: x.dtx,
            noise_suppressor: x.ns,
        },
    )
    .unwrap();
    let n = enc.frame_samples();
    let total = pcm.len() + enc.delay() + n;
    let frames = total.div_ceil(n);
    let mut padded = pcm.clone();
    padded.resize(frames * n, 0);
    let mut out = vec![0u8; 128];
    let mut got = Vec::new();
    for f in 0..frames {
        let e = enc.encode(&padded[f * n..(f + 1) * n], &mut out).unwrap();
        match e {
            Encoded::Speech(len) => {
                got.push(0);
                got.extend_from_slice(&out[..len]);
            }
            Encoded::Sid(len) => {
                got.push(1);
                got.extend_from_slice(&out[..len]);
            }
            Encoded::NoData => got.push(2),
        }
    }
    Some((got, want))
}

#[test]
fn encoder_matches_every_vector() {
    if common::vectors().is_none() {
        eprintln!("codec vectors not found, test skipped");
        return;
    }
    let only = std::env::var("VOICE4_ONLY").ok();
    let mut fails = Vec::new();
    for x in VECTORS {
        if only.as_deref().is_some_and(|o| !x.stem.contains(o)) {
            continue;
        }
        let (got, want) = encode_vector(x).unwrap();
        if got == want {
            eprintln!("{:28} exact", x.stem);
            continue;
        }
        let lay = Decoder::new(config(x), DecoderSettings::PLAIN).unwrap();
        let (sp, sid) = (lay.layout().speech_bytes(), lay.layout().sid_bytes);
        let g = records(&got, sp, sid);
        let w = records(&want, sp, sid);
        let first = g.iter().zip(&w).position(|(a, b)| a != b);
        let same = g.iter().zip(&w).filter(|(a, b)| a == b).count();
        eprintln!(
            "{:28} differs: frames {} vs {}, first diff frame {first:?}, {same} frames equal",
            x.stem,
            g.len(),
            w.len()
        );
        fails.push(x.stem);
    }
    assert!(fails.is_empty(), "encoder differs: {fails:?}");
}

#[test]
fn default_decoder_matches_every_vector() {
    if common::vectors().is_none() {
        return;
    }
    let only = std::env::var("VOICE4_ONLY").ok();
    let mut fails = Vec::new();
    for x in VECTORS {
        if only.as_deref().is_some_and(|o| !x.stem.contains(o)) {
            continue;
        }
        let (got, want) = decode_vector(x, DecoderSettings::default()).unwrap();
        let frame = (x.rate * x.ms / 1000) as usize;
        match compare(&got, &want, frame) {
            Ok(()) => eprintln!("{:28} default exact", x.stem),
            Err(e) => {
                eprintln!("{:28} default {e}", x.stem);
                fails.push(x.stem);
            }
        }
    }
    assert!(fails.is_empty(), "default decode differs: {fails:?}");
}
