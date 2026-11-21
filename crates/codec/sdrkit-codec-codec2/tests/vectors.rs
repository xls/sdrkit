//! The acceptance tests against the Codec 2 vectors: every encoder frame bit for bit, identical decoder PCM, the interface
//! sizes and the table files. Tests that need the vectors or the table files skip when they are missing.

mod common;

use common::*;
use sdrkit_codec_codec2::{Codebooks, Decoder, Encoder, Mode, Table};

/// Encodes the whole input with a new encoder.
fn encode_all(mode: Mode, books: &Codebooks, input: &[i16]) -> Vec<u8> {
    let mut enc = Encoder::new(mode, books).unwrap();
    let (n, b) = (mode.samples_per_frame(), mode.bytes_per_frame());
    let mut out = vec![0u8; input.len() / n * b];
    for (s, f) in input.chunks_exact(n).zip(out.chunks_exact_mut(b)) {
        enc.encode(s, f).unwrap();
    }
    out
}

/// Decodes a whole `.c2` stream with a new decoder.
fn decode_all(mode: Mode, books: &Codebooks, bits: &[u8]) -> Vec<i16> {
    let mut dec = Decoder::new(mode, books).unwrap();
    let (n, b) = (mode.samples_per_frame(), mode.bytes_per_frame());
    let mut out = vec![0i16; bits.len() / b * n];
    for (f, s) in bits.chunks_exact(b).zip(out.chunks_exact_mut(n)) {
        dec.decode(f, s).unwrap();
    }
    out
}

/// Every encoder frame matches the vector bit for bit.
#[test]
fn encoder_frames_match_the_vectors() {
    let Some(input) = wav("codec2_input_8k.wav") else {
        return;
    };
    let mut report = Vec::new();
    for mode in Mode::ALL {
        let (Some(books), Some(want)) = (real_books(mode), vector(&format!("{}.c2", stem(mode))))
        else {
            continue;
        };
        let got = encode_all(mode, &books, &input);
        let b = mode.bytes_per_frame();
        let frames = want.len() / b;
        let mut same = 0;
        for (i, (g, w)) in got.chunks_exact(b).zip(want.chunks_exact(b)).enumerate() {
            if g == w {
                same += 1;
            } else if frames - same <= 3 {
                eprintln!(
                    "{mode:?} frame {i}: {:?} want {:?}",
                    fields(mode, g),
                    fields(mode, w)
                );
            }
        }
        eprintln!("{mode:?}: {same} of {frames} frames identical");
        report.push((mode, same, frames));
    }
    for (mode, same, frames) in report {
        assert_eq!(
            same, frames,
            "{mode:?}: {same} of {frames} frames identical"
        );
    }
}

/// The decoder output of the vector bits is identical to the vector speech.
#[test]
fn decoder_output_matches_the_vectors() {
    let mut report = Vec::new();
    for mode in Mode::ALL {
        let Some(books) = real_books(mode) else {
            continue;
        };
        let (Some(bits), Some(want)) = (
            vector(&format!("{}.c2", stem(mode))),
            wav(&format!("{}_decoded.wav", stem(mode))),
        ) else {
            continue;
        };
        let got = decode_all(mode, &books, &bits);
        assert_eq!(got.len(), 64_000);
        let differ = got.iter().zip(&want).filter(|(a, b)| a != b).count();
        let first = got.iter().zip(&want).position(|(a, b)| a != b);
        let max = got
            .iter()
            .zip(&want)
            .map(|(a, b)| (*a as i32 - *b as i32).abs())
            .max()
            .unwrap_or(0);
        eprintln!("{mode:?}: {differ} samples differ, first at {first:?}, max error {max}");
        report.push((mode, differ));
    }
    for (mode, differ) in report {
        assert_eq!(differ, 0, "{mode:?}: {differ} samples differ");
    }
}

/// Frame counts, lengths and zero tail bits.
#[test]
fn interface_sizes_and_tail_bits() {
    let Some(input) = wav("codec2_input_8k.wav") else {
        return;
    };
    assert_eq!(input.len(), 64_000);
    let books = dummy_books();
    for (mode, frames, bytes) in [
        (Mode::M3200, 400, 3200),
        (Mode::M1300, 200, 1400),
        (Mode::M1200, 200, 1200),
        (Mode::M700C, 200, 800),
    ] {
        let bits = encode_all(mode, &books, &input);
        assert_eq!(bits.len(), bytes);
        assert_eq!(bits.len() / mode.bytes_per_frame(), frames);
        let tail = match mode {
            Mode::M1300 | Mode::M700C => 0x0f,
            Mode::M1200 => 0x01,
            Mode::M3200 => 0,
        };
        assert!(bits
            .chunks_exact(mode.bytes_per_frame())
            .all(|f| f[f.len() - 1] & tail == 0));
        if let Some(v) = vector(&format!("{}.c2", stem(mode))) {
            assert_eq!(v.len(), bytes);
            assert!(v
                .chunks_exact(mode.bytes_per_frame())
                .all(|f| f[f.len() - 1] & tail == 0));
            assert_eq!(decode_all(mode, &books, &v).len(), 64_000);
        }
    }
}

/// The 28 table files load with the right sizes and SHA-256 values, and a changed file is refused.
#[test]
fn table_files_load_with_their_sizes_and_hashes() {
    let dir = tables_dir();
    if !dir.is_dir() {
        eprintln!("skipped: {} is missing", dir.display());
        return;
    }
    let books = Codebooks::load_dir(&dir).unwrap();
    for t in Table::ALL {
        assert_eq!(books.get(t).map(|v| v.len()), Some(t.len()), "{t:?}");
    }
    for mode in Mode::ALL {
        books.check(mode).unwrap();
    }
    let mut bytes = std::fs::read(dir.join(Table::Energy.file_name())).unwrap();
    bytes[0] ^= 1;
    let mut b = Codebooks::new();
    assert!(b.set_file_bytes(Table::Energy, &bytes).is_err());
}
