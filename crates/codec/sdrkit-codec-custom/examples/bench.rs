//! Times the coders: `cargo run --release -p sdrkit-codec-custom --example bench`. Speech lines give microseconds per frame and
//! the speed as a multiple of real time. Picture lines give milliseconds per picture.

use sdrkit_codec_custom::{picture, voice3, voice4};
use std::hint::black_box;
use std::time::Instant;

fn best(mut step: impl FnMut()) -> f64 {
    step();
    let start = Instant::now();
    let mut best = f64::INFINITY;
    let mut runs = 0;
    while runs < 3 || start.elapsed().as_secs_f64() < 1.0 {
        let t = Instant::now();
        step();
        best = best.min(t.elapsed().as_secs_f64());
        runs += 1;
    }
    best
}

fn speech(n: usize) -> Vec<i16> {
    let mut s = 1u32;
    (0..n)
        .map(|i| {
            s = s.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            let t = i as f64 / 16000.0;
            let f0 = 120.0 + 40.0 * (2.0 * std::f64::consts::PI * 0.7 * t).sin();
            let mut v = 0.0;
            for h in 1..12 {
                v += (2.0 * std::f64::consts::PI * f0 * h as f64 * t).sin() / h as f64;
            }
            (2500.0 * v + ((s >> 22) as f64 - 512.0)) as i16
        })
        .collect()
}

fn report(name: &str, frames: usize, frame_ms: f64, secs: f64) {
    let per = secs / frames as f64;
    println!(
        "{name:40} {:9.1} us/frame {:8.0}x real time",
        per * 1e6,
        frame_ms / 1000.0 / per
    );
}

fn main() {
    let pcm = speech(16000 * 4);
    let cases = [
        (
            "voice4 6.0 kbit/s 40 ms 16 kHz",
            voice4::SampleRate::Hz16000,
            voice4::FrameLength::Ms40,
            voice4::Profile::P6_0,
        ),
        (
            "voice4 3.2 kbit/s 40 ms 16 kHz",
            voice4::SampleRate::Hz16000,
            voice4::FrameLength::Ms40,
            voice4::Profile::P3_2,
        ),
        (
            "voice4 HQ16 20 ms 16 kHz",
            voice4::SampleRate::Hz16000,
            voice4::FrameLength::Ms20,
            voice4::Profile::Hq16,
        ),
        (
            "voice4 8.0 kbit/s 20 ms 8 kHz",
            voice4::SampleRate::Hz8000,
            voice4::FrameLength::Ms20,
            voice4::Profile::P8_0,
        ),
    ];
    for (name, rate, frame, profile) in cases {
        let cfg = voice4::Config {
            rate,
            frame,
            profile,
            type_in_band: false,
        };
        let n = voice4::Encoder::new(cfg, voice4::EncoderSettings::default())
            .unwrap()
            .frame_samples();
        let frames = pcm.len() / n;
        let mut coded = vec![[0u8; 128]; frames];
        let mut lens = vec![0usize; frames];
        let t = best(|| {
            let mut enc = voice4::Encoder::new(cfg, voice4::EncoderSettings::default()).unwrap();
            for f in 0..frames {
                lens[f] = enc
                    .encode(&pcm[f * n..(f + 1) * n], &mut coded[f])
                    .unwrap()
                    .len();
            }
        });
        report(&format!("{name} encode"), frames, frame.ms() as f64, t);
        let mut out = vec![0i16; n];
        for (label, settings) in [
            ("decode plain", voice4::DecoderSettings::PLAIN),
            ("decode default", voice4::DecoderSettings::default()),
        ] {
            let t = best(|| {
                let mut dec = voice4::Decoder::new(cfg, settings).unwrap();
                for f in 0..frames {
                    dec.decode(
                        voice4::Input::Frame(&coded[f][..lens[f]]),
                        voice4::Damage::default(),
                        &mut out,
                    )
                    .unwrap();
                    black_box(&out);
                }
            });
            report(&format!("{name} {label}"), frames, frame.ms() as f64, t);
        }
    }
    for (name, v) in [
        ("v3 16 kbit/s", voice3::Variant::V3(voice3::V3Rate::R16)),
        ("v3 32 kbit/s", voice3::Variant::V3(voice3::V3Rate::R32)),
        ("v1 16 kbit/s", voice3::Variant::V1),
    ] {
        let frames = pcm.len() / 320;
        let mut coded = vec![[0u8; 80]; frames];
        let t = best(|| {
            let mut enc = voice3::Encoder::new(v);
            for f in 0..frames {
                enc.encode(&pcm[f * 320..(f + 1) * 320], &mut coded[f])
                    .unwrap();
            }
        });
        report(&format!("{name} encode"), frames, 20.0, t);
        let mut out = [0i16; 320];
        let t = best(|| {
            let mut dec = voice3::Decoder::new(v);
            for c in &coded {
                dec.decode(Some(&c[..v.frame_bytes()]), &mut out).unwrap();
                black_box(&out);
            }
        });
        report(&format!("{name} decode"), frames, 20.0, t);
    }
    let (w, h) = (1920u32, 1080u32);
    let rgb: Vec<u8> = (0..(w * h) as usize)
        .flat_map(|i| {
            let (x, y) = ((i % w as usize) as f64, (i / w as usize) as f64);
            let r = 128.0 + 100.0 * (x / 97.0).sin() * (y / 53.0).cos();
            let g = 128.0 + 90.0 * ((x + y) / 211.0).sin();
            let b = (x / 7.5) % 256.0;
            [r as u8, g as u8, b as u8]
        })
        .collect();
    let mut enc = picture::Encoder::new();
    let mut dec = picture::Decoder::new();
    let mut stream = Vec::new();
    let mut pic = Vec::new();
    let mut hd = enc.encode(&rgb, w, h, 258_438, &mut stream).unwrap();
    let t = best(|| hd = enc.encode(&rgb, w, h, 258_438, &mut stream).unwrap());
    println!(
        "{:40} {:9.1} ms/picture ({} decisions)",
        "picture 1920x1080 encode, 258438 bytes",
        t * 1e3,
        hd.decisions
    );
    let t = best(|| {
        dec.decode(&hd, &stream, &mut pic).unwrap();
    });
    println!("{:40} {:9.1} ms/picture", "picture 1920x1080 decode", t * 1e3);
}
