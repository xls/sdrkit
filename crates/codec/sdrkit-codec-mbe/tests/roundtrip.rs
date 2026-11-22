//! Encode then decode of synthetic signals through both coders, and tone frames.

use sdrkit_codec_mbe::{Coder, Decoder, Encoder, FrameKind, FRAME};
use std::f64::consts::TAU;

/// A vowel-like envelope: formants at 500 and 1500 Hz over a falling slope.
fn envelope(hz: f64) -> f64 {
    let g = |c: f64, w: f64| (-((hz - c) / w).powi(2)).exp();
    3000.0 * g(500.0, 250.0) + 1500.0 * g(1500.0, 300.0) + 200.0 * (-hz / 1000.0).exp()
}

fn vowel(f: f64, k: usize, out: &mut [i16; FRAME]) {
    for (i, o) in out.iter_mut().enumerate() {
        let t = (k * FRAME + i) as f64 / 8000.0;
        let mut v = 0.0;
        let mut h = 1.0;
        while h * f < 3800.0 {
            v += envelope(h * f) * (TAU * h * f * t).cos();
            h += 1.0;
        }
        *o = v as i16;
    }
}

/// The pitch of `x` in Hz: the shortest lag in 20..=130 whose normalised autocorrelation is within 5 % of the best,
/// refined by a parabola.
fn pitch(x: &[f64]) -> f64 {
    let ac = |lag: usize| -> f64 {
        let n = x.len() - lag;
        let (mut s, mut e0, mut e1) = (0.0, 0.0, 0.0);
        for i in 0..n {
            s += x[i] * x[i + lag];
            e0 += x[i] * x[i];
            e1 += x[i + lag] * x[i + lag];
        }
        s / (e0 * e1).sqrt().max(1e-9)
    };
    let r: Vec<f64> = (0..=131).map(ac).collect();
    let best = r[20..=130].iter().cloned().fold(f64::MIN, f64::max);
    let lag = (20..=130).find(|&l| r[l] >= 0.95 * best && r[l] >= r[l - 1] && r[l] >= r[l + 1]).unwrap();
    let (a, b, c) = (r[lag - 1], r[lag], r[lag + 1]);
    let d = 0.5 * (a - c) / (a - 2.0 * b + c);
    8000.0 / (lag as f64 + if d.is_finite() { d } else { 0.0 })
}

/// Magnitude of `x` at `hz`.
fn level(x: &[f64], hz: f64) -> f64 {
    let (mut re, mut im) = (0.0, 0.0);
    let n = x.len() as f64;
    for (i, &v) in x.iter().enumerate() {
        let w = 0.5 - 0.5 * (TAU * i as f64 / n).cos();
        let ph = TAU * hz * i as f64 / 8000.0;
        re += w * v * ph.cos();
        im -= w * v * ph.sin();
    }
    (re * re + im * im).sqrt() / (n / 4.0)
}

fn voiced_round_trip(freqs: &[f64]) {
    for coder in Coder::ALL {
        for &f in freqs {
            let mut enc = Encoder::new(coder);
            let mut dec = Decoder::new(coder);
            let mut pcm = [0i16; FRAME];
            let mut frame = vec![0u8; coder.frame_bits()];
            let mut out = [0i16; FRAME];
            let mut y = Vec::new();
            let mut err_db = Vec::new();
            for k in 0..40 {
                vowel(f, k, &mut pcm);
                enc.encode(&pcm, &mut frame);
                let s = dec.decode(&frame, &mut out);
                assert_eq!(s.kind, FrameKind::Voice);
                if k >= 10 {
                    y.extend(out.iter().map(|&v| v as f64));
                    // The decoded envelope at the input harmonics inside the formants (amplitude M = a / 2).
                    let p = dec.params();
                    let mut h = 1.0;
                    while h * f < 2000.0 {
                        let a = envelope(h * f);
                        let l = (h * f / p.f0_hz()).round() as usize;
                        if a > 300.0 && (1..=p.l).contains(&l) {
                            err_db.push(20.0 * (p.amp[l].max(1e-3) / (a / 2.0)).log10());
                        }
                        h += 1.0;
                    }
                }
            }
            let got = pitch(&y[..2048]);
            assert!((got - f).abs() / f < 0.02, "{coder:?} {f} Hz: output pitch {got}, model f0 {}", dec.params().f0_hz());
            let rms_db = (err_db.iter().map(|e| e * e).sum::<f64>() / err_db.len() as f64).sqrt();
            let mean_db = err_db.iter().sum::<f64>() / err_db.len() as f64;
            println!("{coder:?} {f} Hz: pitch {got:.1} Hz, envelope error {rms_db:.2} dB RMS (mean {mean_db:.2} dB)");
            assert!(rms_db < 6.0, "{coder:?} {f} Hz: envelope error {rms_db} dB");
            // The output level at the first formant follows the input.
            let h500 = (500.0 / f).round() * f;
            let at = peak(&y[..4096], 0.98 * h500, 1.02 * h500);
            let ratio = level(&y[..4096], at) / envelope(h500);
            assert!(ratio > 0.5 && ratio < 2.0, "{coder:?} {f} Hz: level ratio {ratio}");
        }
    }
}

#[test]
fn voiced_speech_keeps_pitch_and_envelope() {
    voiced_round_trip(&[100.0, 150.0, 180.0, 220.0, 260.0, 300.0]);
}

#[test]
fn noise_stays_unvoiced() {
    for coder in Coder::ALL {
        let mut enc = Encoder::new(coder);
        let mut dec = Decoder::new(coder);
        let mut x = 99u64;
        let mut pcm = [0i16; FRAME];
        let mut frame = vec![0u8; coder.frame_bits()];
        let mut out = [0i16; FRAME];
        let (mut voiced, mut total) = (0usize, 0usize);
        let (mut ein, mut eout) = (0.0, 0.0);
        for k in 0..60 {
            for p in pcm.iter_mut() {
                x = x.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1_442_695_040_888_963_407);
                *p = ((x >> 40) as i32 % 6000 - 3000) as i16;
            }
            enc.encode(&pcm, &mut frame);
            dec.decode(&frame, &mut out);
            if k >= 10 {
                ein += pcm.iter().map(|&v| (v as f64).powi(2)).sum::<f64>();
                eout += out.iter().map(|&v| (v as f64).powi(2)).sum::<f64>();
                let p = dec.params();
                voiced += (1..=p.l).filter(|&l| p.voiced[l]).count();
                total += p.l;
            }
        }
        let share = voiced as f64 / total as f64;
        let db = 10.0 * (eout / ein).log10();
        println!("{coder:?}: {:.1} % of the noise harmonics voiced, level {db:.2} dB", 100.0 * share);
        assert!(db.abs() < 3.0, "{coder:?}: noise level {db} dB");
        assert!(share < 0.2, "{coder:?}: {share}");
    }
}

/// The frequency of the strongest component of `x` between `lo` and `hi` Hz, on a 0.5 Hz grid.
fn peak(x: &[f64], lo: f64, hi: f64) -> f64 {
    let mut best = (0.0, lo);
    let mut hz = lo;
    while hz <= hi {
        let v = level(x, hz);
        if v > best.0 {
            best = (v, hz);
        }
        hz += 0.5;
    }
    best.1
}

#[test]
fn tone_frames_give_the_tone() {
    let mut enc = Encoder::new(Coder::HalfRate);
    let mut dec = Decoder::new(Coder::HalfRate);
    let mut frame = [0u8; 72];
    let mut out = [0i16; FRAME];
    // ID 40: 1250 Hz; ID 133 (DTMF 5): 770 and 1336 Hz, which Annex J places at 766.7 and 1341.8 Hz.
    for (id, tones) in [(40u8, vec![1250.0]), (133, vec![770.0, 1336.0])] {
        let mut y = Vec::new();
        for k in 0..20 {
            assert_eq!(enc.encode_tone(id, 100, &mut frame), 72);
            let s = dec.decode(&frame, &mut out);
            assert_eq!(s.kind, FrameKind::Tone { id, amplitude: 100 });
            if k >= 4 {
                y.extend(out.iter().map(|&v| v as f64));
            }
        }
        for &t in &tones {
            let got = peak(&y, t - 40.0, t + 40.0);
            println!("tone {id}: {t} Hz measured at {got} Hz");
            assert!((got - t).abs() / t < 0.008, "tone {id}: {t} Hz measured {got}");
        }
        let rms = (y.iter().map(|v| v * v).sum::<f64>() / y.len() as f64).sqrt();
        assert!(rms > 1000.0, "tone {id}: RMS {rms}");
    }
    // ID 255 is silent whatever the amplitude; an invalid ID repeats the frame.
    for _ in 0..4 {
        enc.encode_tone(255, 127, &mut frame);
        dec.decode(&frame, &mut out);
    }
    assert!(out.iter().all(|&v| v.abs() <= 1), "{:?}", &out[..8]);
    enc.encode_tone(0, 100, &mut frame);
    assert_eq!(dec.decode(&frame, &mut out).kind, FrameKind::Repeated);
}
