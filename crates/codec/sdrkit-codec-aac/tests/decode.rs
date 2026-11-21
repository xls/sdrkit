//! The decoder against signals of known content: round trips through the encoder of this crate (960-sample frames), and
//! ADTS streams of AAC-LC, HE-AAC and HE-AAC v2 (1024-sample frames) of a test signal whose left channel holds a 1 kHz
//! tone at 0.5 and a 12 kHz tone at 0.1 of full scale, and whose right channel holds a 3 kHz tone at 0.25 and noise.

use sdrkit_algo_dsp::fft::Fft;
use sdrkit_algo_dsp::Complex32;
use sdrkit_codec_aac::{register, Encoder, FRAME};
use sdrkit_core::codec::{AudioCodec, CodecSet, Pcm};
use sdrkit_core::media::{AudioProps, Codec, MediaFormat, Packaging};
use sdrkit_proto_bcdata::aac::AudioSpecificConfig;
use std::f64::consts::TAU;

fn decoder(format: &MediaFormat) -> Box<dyn AudioCodec> {
    let mut set = CodecSet::default();
    register(&mut set);
    let entry = set.find(format.codec.fourcc(), "").expect("registered");
    (entry.factory)(format).expect("a decoder for the format")
}

/// The amplitude of the component of `x` at `hz` (exact when `x` holds a whole number of cycles).
fn level(x: &[f64], rate: f64, hz: f64) -> f64 {
    let (mut re, mut im) = (0.0, 0.0);
    for (i, &v) in x.iter().enumerate() {
        let a = TAU * hz * i as f64 / rate;
        re += v * a.cos();
        im += v * a.sin();
    }
    2.0 * (re * re + im * im).sqrt() / x.len() as f64
}

/// The frequency of the strongest component of `x` within 20 Hz of `hz`, to 0.1 Hz.
fn peak(x: &[f64], rate: f64, hz: f64) -> f64 {
    (0..400)
        .map(|i| hz - 20.0 + 0.1 * i as f64)
        .map(|f| (level(x, rate, f), f))
        .fold((0.0, 0.0), |a, b| if b.0 > a.0 { b } else { a })
        .1
}

/// The amplitude of a tone with the power of `x` between `lo` and `hi` Hz.
fn band_level(x: &[f64], rate: f64, lo: f64, hi: f64) -> f64 {
    let n = x.len();
    let mut buf: Vec<Complex32> = x.iter().map(|&v| Complex32::new(v as f32, 0.0)).collect();
    Fft::forward(n).process(&mut buf);
    let p: f64 = buf[(lo * n as f64 / rate) as usize..(hi * n as f64 / rate) as usize]
        .iter()
        .map(|c| f64::from(c.norm_sqr()))
        .sum();
    2.0 * p.sqrt() / n as f64
}

/// One channel of interleaved PCM as values of full scale 1.
fn channel(pcm: &Pcm, ch: usize) -> Vec<f64> {
    let n = usize::from(pcm.channels);
    pcm.samples
        .iter()
        .skip(ch)
        .step_by(n)
        .map(|&v| f64::from(v) / 32768.0)
        .collect()
}

fn db(x: f64) -> f64 {
    20.0 * x.log10()
}

#[test]
fn round_trip_of_960_sample_frames() {
    let rate = 48_000.0;
    let n = 50;
    let tone = |hz: f64, a: f64, i: usize| (a * (TAU * hz * i as f64 / rate).sin()) as f32;
    let mut noise = 1u32;
    let left: Vec<f32> = (0..n * FRAME).map(|i| tone(1000.0, 0.5, i)).collect();
    let right: Vec<f32> = (0..n * FRAME)
        .map(|i| {
            noise = noise.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            tone(3000.0, 0.25, i) + (noise as i32 as f32 / i32::MAX as f32) * 0.05
        })
        .collect();
    let mut enc = Encoder::new(48_000).unwrap();
    let mut format = MediaFormat::new(Codec::Aac, Packaging::Raw);
    let mut asc = Vec::new();
    AudioSpecificConfig {
        frame_960: true,
        ..AudioSpecificConfig::lc(48_000, 2)
    }
    .write(&mut asc)
    .unwrap();
    format.config = asc.into();
    let mut dec = decoder(&format);
    let mut pcm = Pcm::default();
    let mut block = Vec::new();
    for f in 0..n {
        let r = f * FRAME..(f + 1) * FRAME;
        enc.encode(&left[r.clone()], &right[r], &[], 400, &mut block)
            .unwrap();
        dec.decode(&block, false, &mut pcm).unwrap();
    }
    assert_eq!((pcm.rate_hz, pcm.channels), (48_000, 2));
    assert_eq!(pcm.samples.len(), 2 * n * FRAME);
    // The output lags the input by one frame.
    for (ch, src) in [&left, &right].into_iter().enumerate() {
        let out = channel(&pcm, ch);
        let (mut err, mut sig) = (0.0, 0.0);
        for i in 2 * FRAME..n * FRAME {
            let d = out[i] - f64::from(src[i - FRAME]);
            err += d * d;
            sig += f64::from(src[i - FRAME]).powi(2);
        }
        let snr = 10.0 * (sig / err).log10();
        eprintln!("channel {ch}: signal to error {snr:.1} dB");
        // The encoder spends few bits on the noise of the right channel.
        let floor = if ch == 0 { 40.0 } else { 20.0 };
        assert!(snr > floor, "channel {ch}: {snr} dB");
    }
}

/// Splits an ADTS stream into frames.
fn adts_frames(b: &[u8]) -> Vec<&[u8]> {
    let mut out = Vec::new();
    let mut at = 0;
    while at + 7 <= b.len() {
        let len = (usize::from(b[at + 3] & 3) << 11)
            | (usize::from(b[at + 4]) << 3)
            | usize::from(b[at + 5] >> 5);
        out.push(&b[at..at + len]);
        at += len;
    }
    out
}

fn decode_file(name: &str, codec: Codec) -> Pcm {
    let data = std::fs::read(format!("{}/tests/data/{name}", env!("CARGO_MANIFEST_DIR"))).unwrap();
    let mut dec = decoder(&MediaFormat::new(codec, Packaging::Adts));
    let mut pcm = Pcm::default();
    for f in adts_frames(&data) {
        dec.decode(f, false, &mut pcm).unwrap();
    }
    pcm
}

/// The tones of the test signal in one second from 0.5 s on: (left 1 kHz, left 11 to 14 kHz, right 3 kHz, left 3 kHz,
/// right 1 kHz) as levels in dB of full scale.
fn tones(pcm: &Pcm) -> [f64; 5] {
    let rate = f64::from(pcm.rate_hz);
    let r = pcm.rate_hz as usize / 2..pcm.rate_hz as usize * 3 / 2;
    let l = &channel(pcm, 0)[r.clone()];
    let rt = &channel(pcm, 1)[r];
    assert!((peak(l, rate, 1000.0) - 1000.0).abs() <= 1.0);
    assert!((peak(rt, rate, 3000.0) - 3000.0).abs() <= 1.0);
    [
        db(level(l, rate, 1000.0)),
        db(band_level(l, rate, 11_000.0, 14_000.0)),
        db(level(rt, rate, 3000.0)),
        db(level(l, rate, 3000.0)),
        db(level(rt, rate, 1000.0)),
    ]
}

#[test]
fn aac_lc_of_1024_sample_frames() {
    let pcm = decode_file("lc_48k_stereo.aac", Codec::Aac);
    assert_eq!((pcm.rate_hz, pcm.channels), (48_000, 2));
    let t = tones(&pcm);
    eprintln!("AAC-LC tones {t:.2?}");
    assert!(
        (t[0] - db(0.5)).abs() < 0.5
            && (t[2] - db(0.25)).abs() < 0.5
            && (t[1] - db(0.1)).abs() < 1.0
    );
    assert!(t[3] < -60.0 && t[4] < -60.0);
}

#[test]
fn he_aac_reproduces_the_high_band() {
    let pcm = decode_file("he_48k_stereo.aac", Codec::HeAac);
    assert_eq!((pcm.rate_hz, pcm.channels), (48_000, 2));
    let t = tones(&pcm);
    eprintln!("HE-AAC tones {t:.2?}");
    assert!((t[0] - db(0.5)).abs() < 1.0 && (t[2] - db(0.25)).abs() < 1.0);
    // The 12 kHz tone is above the core band: SBR rebuilds its energy, near the middle of a QMF band.
    assert!((t[1] - db(0.1)).abs() < 3.0, "12 kHz at {} dB", t[1]);
}

#[test]
fn he_aac_v2_rebuilds_the_stereo_image() {
    let pcm = decode_file("hev2_48k.aac", Codec::HeAacV2);
    assert_eq!((pcm.rate_hz, pcm.channels), (48_000, 2));
    let t = tones(&pcm);
    eprintln!("HE-AAC v2 tones {t:.2?}");
    assert!((t[0] - db(0.5)).abs() < 1.5 && (t[2] - db(0.25)).abs() < 1.5);
    // The 1 kHz tone is in the left channel only and the 3 kHz tone in the right only.
    assert!(t[0] - t[4] > 15.0 && t[2] - t[3] > 15.0, "{t:?}");
}

#[test]
fn erasures_fade_out_then_stay_silent() {
    let mut format = MediaFormat::new(Codec::Aac, Packaging::Raw);
    let mut asc = Vec::new();
    AudioSpecificConfig {
        frame_960: true,
        ..AudioSpecificConfig::lc(48_000, 2)
    }
    .write(&mut asc)
    .unwrap();
    format.config = asc.into();
    format.audio = Some(AudioProps {
        sample_rate: 48_000,
        channels: 2,
    });
    let mut dec = decoder(&format);
    let mut enc = Encoder::new(48_000).unwrap();
    let tone: Vec<f32> = (0..FRAME).map(|i| (i as f32 * 0.1).sin() * 0.5).collect();
    let mut block = Vec::new();
    let mut pcm = Pcm::default();
    for _ in 0..3 {
        enc.encode(&tone, &tone, &[], 300, &mut block).unwrap();
        dec.decode(&block, false, &mut pcm).unwrap();
    }
    let peak = |p: &Pcm| {
        p.samples
            .iter()
            .map(|v| v.unsigned_abs())
            .max()
            .unwrap_or(0)
    };
    let mut lost = Pcm::default();
    dec.decode(&[], true, &mut lost).unwrap();
    assert!(peak(&lost) > 1000);
    for _ in 0..2 {
        lost.clear();
        dec.decode(&[], true, &mut lost).unwrap();
    }
    assert_eq!(lost.samples.len(), 2 * FRAME);
    assert_eq!(peak(&lost), 0);
}
