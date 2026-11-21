//! Tone round trips: frames from the AAC-LC and Layer II encoders of the workspace decode to the same tone. E-AC-3 streams
//! of known tones decode with each tone on its channel.
//!
//! The files `data/eac3_51_48k.ec3` (5.1, 384 kbit/s) and `data/eac3_20_48k.ec3` (stereo, 96 kbit/s) hold 0.4 s of E-AC-3
//! made with the E-AC-3 encoder of FFmpeg: L 1 kHz at 0.5, R 2 kHz at 0.4, C 500 Hz at 0.3, LFE 50 Hz at 0.3, Ls 3 kHz at
//! 0.2, Rs 4 kHz at 0.1 of full scale (stereo: L and R only). Each syncframe is independent substream 0 with 6 blocks.

#![cfg(any(target_os = "macos", target_os = "ios"))]

use sdrkit_core::codec::{AudioCodec, CodecSet, Pcm};
use sdrkit_core::media::{AudioProps, ChannelLayout, Codec, MediaFormat, Packaging};

const RATE: f64 = 48_000.0;
const TONE_HZ: f64 = 1000.0;
const AMPLITUDE: f64 = 0.5;

fn decoder(format: &MediaFormat) -> Box<dyn AudioCodec> {
    let mut set = CodecSet::default();
    sdrkit_codec_apple::register(&mut set);
    let entry = set.find(format.fourcc(), "").expect("registered");
    (entry.factory)(format).expect("a decoder for this format")
}

fn tone(n: usize, from: usize) -> Vec<f32> {
    (from..from + n)
        .map(|i| {
            (AMPLITUDE * (2.0 * std::f64::consts::PI * TONE_HZ * i as f64 / RATE).sin()) as f32
        })
        .collect()
}

/// The frequency (from the zero crossings, interpolated) and the RMS level in dB relative to the input tone's, of the left
/// channel of interleaved stereo `pcm` at `rate`, after `skip` frames.
fn measure(pcm: &[i16], rate: u32, skip: usize) -> (f64, f64) {
    let left: Vec<f64> = pcm
        .chunks(2)
        .skip(skip)
        .map(|f| f64::from(f[0]) / 32768.0)
        .collect();
    let mut crossings = Vec::new();
    for i in 1..left.len() {
        if left[i - 1] < 0.0 && left[i] >= 0.0 {
            crossings.push(i as f64 - 1.0 + left[i - 1] / (left[i - 1] - left[i]));
        }
    }
    let (first, last) = (crossings[0], crossings[crossings.len() - 1]);
    let freq = (crossings.len() - 1) as f64 * f64::from(rate) / (last - first);
    let rms = (left.iter().map(|x| x * x).sum::<f64>() / left.len() as f64).sqrt();
    let level = 20.0 * (rms / (AMPLITUDE / 2f64.sqrt())).log10();
    (freq, level)
}

#[test]
fn aac_lc_frames_of_960_samples_are_refused() {
    let mut set = CodecSet::default();
    sdrkit_codec_apple::register(&mut set);
    let mut format = MediaFormat::new(Codec::Aac, Packaging::Raw);
    // AAC-LC, 48 kHz, two channels, 960-sample frames (ISO/IEC 14496-3 clause 1.6.2.1).
    format.config = [0x11u8, 0x94][..].into();
    format.audio = Some(AudioProps {
        sample_rate: 48_000,
        channels: 2,
    });
    let entry = set.find(Codec::Aac.fourcc(), "").unwrap();
    assert!((entry.factory)(&format).is_none());
}

#[test]
fn layer_ii_frames_decode_to_the_tone() {
    let mut dec = decoder(&MediaFormat::new(Codec::Mp2, Packaging::Raw));
    let mut enc = sdrkit_codec_mpa::Encoder::new(192, 48_000, true).unwrap();
    let (mut frame, mut pcm, mut all) = (Vec::new(), Pcm::default(), Vec::new());
    for k in 0..50 {
        let x = tone(1152, k * 1152);
        enc.encode(&x, &x, 0, &mut frame).unwrap();
        pcm.clear();
        dec.decode(&frame, false, &mut pcm).unwrap();
        all.extend_from_slice(&pcm.samples);
    }
    assert_eq!((pcm.rate_hz, pcm.channels), (48_000, 2));
    let (freq, level) = measure(&all, pcm.rate_hz, 4 * 1152);
    assert!((freq - TONE_HZ).abs() < 1.0, "{freq} Hz");
    assert!(level.abs() < 1.0, "{level} dB");
}

#[test]
fn ac3_and_eac3_streams_are_taken_and_latm_is_refused() {
    let mut set = CodecSet::default();
    sdrkit_codec_apple::register(&mut set);
    for c in [Codec::Ac3, Codec::EAc3] {
        let f = MediaFormat::new(c, Packaging::Raw);
        assert!((set.find(f.fourcc(), "").unwrap().factory)(&f).is_some());
    }
    let latm = MediaFormat::new(Codec::HeAac, Packaging::Latm);
    assert!((set.find(latm.fourcc(), "").unwrap().factory)(&latm).is_none());
}

const EAC3_51: &[u8] = include_bytes!("data/eac3_51_48k.ec3");
const EAC3_20: &[u8] = include_bytes!("data/eac3_20_48k.ec3");

/// The syncframes of an E-AC-3 stream, from frmsiz (ETSI TS 102 366 E.1.2.2).
fn syncframes(mut d: &[u8]) -> Vec<&[u8]> {
    let mut v = Vec::new();
    while d.len() >= 4 && d[0] == 0x0B && d[1] == 0x77 {
        let len = 2 * (((usize::from(d[2] & 7) << 8) | usize::from(d[3])) + 1);
        v.push(&d[..len.min(d.len())]);
        d = &d[len.min(d.len())..];
    }
    v
}

/// Which of `tones` is the strongest in channel `ch` of interleaved `pcm` with `n` channels at 48 kHz, after `skip` frames.
fn strongest(pcm: &[i16], n: usize, ch: usize, skip: usize, tones: &[f64]) -> f64 {
    let x: Vec<f64> = pcm.chunks(n).skip(skip).map(|f| f64::from(f[ch])).collect();
    let level = |hz: f64| {
        let (mut re, mut im) = (0.0, 0.0);
        for (i, v) in x.iter().enumerate() {
            let a = 2.0 * std::f64::consts::PI * hz * i as f64 / RATE;
            re += v * a.cos();
            im += v * a.sin();
        }
        re.hypot(im)
    };
    tones
        .iter()
        .copied()
        .fold(0.0, |b, hz| if level(hz) > level(b) { hz } else { b })
}

#[test]
fn eac3_51_decodes_with_each_tone_on_its_channel_and_a_change_to_stereo_is_reported() {
    let mut dec = decoder(&MediaFormat::new(Codec::EAc3, Packaging::Raw));
    let (mut pcm, mut all) = (Pcm::default(), Vec::new());
    for f in syncframes(EAC3_51) {
        pcm.clear();
        dec.decode(f, false, &mut pcm).unwrap();
        assert_eq!((pcm.rate_hz, pcm.channels), (48_000, 6));
        assert_eq!(pcm.layout, ChannelLayout::Surround51);
        all.extend_from_slice(&pcm.samples);
    }
    assert!(all.len() >= 6 * 12_000, "{} samples", all.len());
    // L C R Ls Rs LFE: the strongest of the six tones in each channel.
    let tones = [1000.0, 500.0, 2000.0, 3000.0, 4000.0, 50.0];
    let got: Vec<f64> = (0..6)
        .map(|ch| strongest(&all, 6, ch, 1536, &tones))
        .collect();
    assert_eq!(got, tones);
    // The stream goes on in stereo: each call reports the channels of its own samples.
    let mut stereo = Vec::new();
    for f in syncframes(EAC3_20) {
        pcm.clear();
        dec.decode(f, false, &mut pcm).unwrap();
        assert_eq!((pcm.rate_hz, pcm.channels), (48_000, 2));
        assert_eq!(pcm.layout, ChannelLayout::Stereo);
        assert_eq!(pcm.samples.len() % 2, 0);
        stereo.extend_from_slice(&pcm.samples);
    }
    let got: Vec<f64> = (0..2)
        .map(|ch| strongest(&stereo, 2, ch, 1536, &tones))
        .collect();
    assert_eq!(got, [1000.0, 2000.0]);
}

#[test]
fn eac3_51_is_mixed_down_when_the_format_asks_for_stereo() {
    let mut format = MediaFormat::new(Codec::EAc3, Packaging::Raw);
    format.audio = Some(AudioProps {
        sample_rate: 48_000,
        channels: 2,
    });
    let mut dec = decoder(&format);
    let (mut pcm, mut all) = (Pcm::default(), Vec::new());
    for f in syncframes(EAC3_51) {
        pcm.clear();
        dec.decode(f, false, &mut pcm).unwrap();
        assert_eq!((pcm.channels, pcm.layout), (2, ChannelLayout::Stereo));
        all.extend_from_slice(&pcm.samples);
    }
    assert!(all.len() >= 2 * 12_000);
    // Lo = L + 0.707 C + 0.707 Ls, Ro = R + 0.707 C + 0.707 Rs.
    let tones = [1000.0, 500.0, 2000.0, 3000.0, 4000.0];
    let got: Vec<f64> = (0..2)
        .map(|ch| strongest(&all, 2, ch, 1536, &tones))
        .collect();
    assert_eq!(got, [1000.0, 2000.0]);
}

#[test]
fn an_eac3_unit_must_start_with_independent_substream_0() {
    let mut dec = decoder(&MediaFormat::new(Codec::EAc3, Packaging::Raw));
    let mut f = syncframes(EAC3_51)[0].to_vec();
    // strmtyp 1: a dependent substream.
    f[2] = (f[2] & 0x3F) | 0x40;
    assert_eq!(
        dec.decode(&f, false, &mut Pcm::default()),
        Err(sdrkit_core::codec::CodecError::Frame)
    );
}
