//! Round trips, determinism, refusal of missing or wrong tables, and errors on bad input. These tests use tables of the right
//! shape with made-up values, so they need no files.

mod common;

use common::dummy_books;
use sdrkit_codec_codec2::{Codebooks, Decoder, Encoder, Error, Mode, Table};

/// A vowel: a 150 Hz pulse train through a 700 Hz resonator, 2 s at 8 kHz.
fn vowel() -> Vec<i16> {
    let (r, f) = (0.97f64, 700.0f64);
    let a1 = 2.0 * r * (std::f64::consts::TAU * f / 8000.0).cos();
    let a2 = -r * r;
    let (mut y1, mut y2) = (0.0f64, 0.0f64);
    (0..16_000)
        .map(|n| {
            let x = if n % 53 == 0 { 3000.0 } else { 0.0 };
            let y = x + a1 * y1 + a2 * y2;
            y2 = y1;
            y1 = y;
            y.clamp(-30000.0, 30000.0) as i16
        })
        .collect()
}

fn run(mode: Mode, books: &Codebooks, input: &[i16]) -> (Vec<u8>, Vec<i16>) {
    let mut enc = Encoder::new(mode, books).unwrap();
    let mut dec = Decoder::new(mode, books).unwrap();
    let (n, b) = (mode.samples_per_frame(), mode.bytes_per_frame());
    let mut bits = Vec::new();
    let mut out = Vec::new();
    let mut frame = vec![0u8; b];
    let mut speech = vec![0i16; n];
    for s in input.chunks_exact(n) {
        enc.encode(s, &mut frame).unwrap();
        dec.decode(&frame, &mut speech).unwrap();
        bits.extend_from_slice(&frame);
        out.extend_from_slice(&speech);
    }
    (bits, out)
}

/// The lag of the largest autocorrelation from 20 to 160 samples.
fn pitch_lag(x: &[i16]) -> usize {
    let x: Vec<f64> = x.iter().map(|&v| v as f64).collect();
    (20..=160)
        .max_by(|&a, &b| {
            let ra: f64 = x[a..].iter().zip(&x).map(|(p, q)| p * q).sum();
            let rb: f64 = x[b..].iter().zip(&x).map(|(p, q)| p * q).sum();
            ra.total_cmp(&rb)
        })
        .unwrap()
}

#[test]
fn a_vowel_keeps_its_pitch_through_every_mode() {
    // The pitch of mode 1200 comes from a trained table, so this test uses the real tables.
    let input = vowel();
    for mode in Mode::ALL {
        let Some(books) = common::real_books(mode) else {
            continue;
        };
        let (_, out) = run(mode, &books, &input);
        // Skip the first 0.5 s of start-up and delay.
        let tail = &out[4000..];
        let rms =
            (tail.iter().map(|&v| (v as f64).powi(2)).sum::<f64>() / tail.len() as f64).sqrt();
        assert!(rms > 50.0, "{mode:?}: the output is silent ({rms})");
        let lag = pitch_lag(&tail[..2000]);
        assert!(lag.abs_diff(53) <= 2, "{mode:?}: pitch lag {lag}, want 53");
    }
}

#[test]
fn encoder_and_decoder_are_deterministic() {
    let input = vowel();
    let books = dummy_books();
    for mode in Mode::ALL {
        assert_eq!(
            run(mode, &books, &input),
            run(mode, &books, &input),
            "{mode:?}"
        );
    }
}

#[test]
fn missing_or_wrong_tables_refuse_to_start() {
    let empty = Codebooks::new();
    for mode in Mode::ALL {
        assert!(matches!(
            Encoder::new(mode, &empty),
            Err(Error::MissingTable(_))
        ));
        assert!(matches!(
            Decoder::new(mode, &empty),
            Err(Error::MissingTable(_))
        ));
    }
    let mut b = Codebooks::new();
    for t in Table::ALL {
        let short = vec![0.0f32; t.len() - 1];
        assert!(matches!(b.set(t, &short), Err(Error::TableSize { .. })));
        let mut nan = vec![0.0f32; t.len()];
        nan[3] = f32::NAN;
        assert!(matches!(
            b.set(t, &nan),
            Err(Error::TableValue { index: 3, .. })
        ));
        assert!(matches!(
            b.set_file_bytes(t, &[0u8; 5]),
            Err(Error::TableSize { .. })
        ));
        let zeros = vec![0u8; 4 * t.len()];
        assert!(matches!(
            b.set_file_bytes(t, &zeros),
            Err(Error::TableHash(_))
        ));
    }
    assert!(matches!(
        b.set(Table::Lsp(10), &[0.0]),
        Err(Error::UnknownTable(_))
    ));
    assert!(Codebooks::load_dir(std::path::Path::new("/no/such/folder"))
        .unwrap()
        .check(Mode::M3200)
        .is_err());
}

#[test]
fn wrong_lengths_are_errors_and_any_bits_decode() {
    let books = dummy_books();
    for mode in Mode::ALL {
        let mut enc = Encoder::new(mode, &books).unwrap();
        let mut dec = Decoder::new(mode, &books).unwrap();
        let (n, b) = (mode.samples_per_frame(), mode.bytes_per_frame());
        assert!(enc.encode(&vec![0; n - 1], &mut vec![0; b]).is_err());
        assert!(enc.encode(&vec![0; n], &mut vec![0; b + 1]).is_err());
        assert!(dec.decode(&vec![0; b - 1], &mut vec![0; n]).is_err());
        assert!(dec.decode(&vec![0; b], &mut vec![0; n + 1]).is_err());
        // Every bit pattern decodes without a panic, including all ones and random frames.
        let mut speech = vec![0i16; n];
        let mut x = 12345u32;
        for k in 0..300 {
            let frame: Vec<u8> = (0..b)
                .map(|_| {
                    x = x.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
                    if k % 3 == 0 {
                        0xff
                    } else {
                        (x >> 24) as u8
                    }
                })
                .collect();
            dec.decode(&frame, &mut speech).unwrap();
        }
        // Extreme input encodes.
        let loud: Vec<i16> = (0..n)
            .map(|i| if i % 2 == 0 { i16::MAX } else { i16::MIN })
            .collect();
        for _ in 0..10 {
            enc.encode(&loud, &mut vec![0; b]).unwrap();
            enc.encode(&vec![0; n], &mut vec![0; b]).unwrap();
        }
    }
}
