//! `fec.trellis`: the ATSC 8-VSB trellis code (A/53 part 2 clause 6.4), twelve trellises interleaved, noisy levels decoded to
//! the exact bits for any cut, a frame start mid-stream, and strict settings.

mod fec_support;
use fec_support::*;

use sdrkit_algo_fec::conv::tcm::{self, TcmCodec};
use sdrkit_algo_fec::conv::Trellis;
use sdrkit_components::fec::TrellisDecode;
use sdrkit_components::testing::Source;
use sdrkit_core::{Complex32, Value};
use sdrkit_graph::stream::Data;
use sdrkit_graph::{Format, Kind};

/// `symbols` 8-VSB symbols from 12 interleaved encoders (each starting in state 0): the levels with noise, and the bits (Z2, Z1)
/// of each symbol.
fn atsc(rng: &mut Rng, symbols: usize, noise: f32) -> (Vec<f32>, Vec<u8>) {
    let t = Trellis::atsc_8vsb();
    let mut state = [0usize; 12];
    let mut levels = Vec::with_capacity(symbols);
    let mut bits = Vec::with_capacity(2 * symbols);
    for i in 0..symbols {
        let p = i % 12;
        let (z2, x1) = (rng.below(2), rng.below(2));
        let label = t.label(state[p], x1) as i32;
        state[p] = t.next(state[p], x1);
        let level = if z2 == 0 {
            2 * label - 7
        } else {
            2 * label + 1
        };
        levels.push(level as f32 + noise * rng.gauss());
        bits.push(z2 as u8);
        bits.push(x1 as u8);
    }
    (levels, bits)
}

#[test]
fn twelve_interleaved_trellises_decode_exactly_for_any_cut() {
    let mut rng = Rng(12);
    let (levels, bits) = atsc(&mut rng, 12 * 700, 0.45);
    // Hard slicing alone makes errors at this noise.
    let raw_errors = levels
        .iter()
        .enumerate()
        .filter(|(i, &r)| {
            let nearest = ((r + 7.0) / 2.0).round().clamp(0.0, 7.0) as usize;
            (nearest >> 2) as u8 != bits[2 * i] || ((nearest >> 1) & 1) as u8 != bits[2 * i + 1]
        })
        .count();
    assert!(raw_errors > 0);
    let mut first_cut: Option<Vec<u8>> = None;
    for per_pass in [1usize, 100, 12 * 700] {
        let mut g = Tb::new()
            .add(
                "src",
                Source::new(
                    Format::new(Kind::Real, 10_762_238.0),
                    Data::Real(levels.clone()),
                    per_pass,
                ),
            )
            .add("t", TrellisDecode::default())
            .connect("src", "t")
            .set("t.chunk", Value::F64(128.0))
            .set("t.depth", Value::F64(32.0))
            .build()
            .unwrap();
        let got = run(&mut g, levels.len() / per_pass + 2, &["t.out"]);
        let out = hard_bits(&got[0].data);
        // Decided in rounds of 128 symbols per trellis: 640 of the 700 per trellis are out.
        assert_eq!(out.len(), 2 * 12 * 640, "pieces of {per_pass}");
        // The uncoded bit Z2 is sliced; the coded bit Z1 comes from the trellis. Check both on symbols away from the start.
        assert_eq!(
            out[2 * 12 * 8..],
            bits[2 * 12 * 8..out.len()],
            "pieces of {per_pass}"
        );
        match &first_cut {
            None => first_cut = Some(out),
            Some(r) => assert_eq!(&out, r),
        }
    }
}

#[test]
fn symbols_and_a_frame_start_decide_everything_held() {
    let mut rng = Rng(13);
    let (levels, bits) = atsc(&mut rng, 12 * 50, 0.0);
    let symbols: Vec<Complex32> = levels.iter().map(|&r| Complex32::new(r, 0.3)).collect();
    // A frame start after all the symbols' trellises: everything held is decided there.
    let mut sym = symbols.clone();
    sym.extend(std::iter::repeat_n(Complex32::new(1.0, 0.0), 12));
    let n = symbols.len() as u64;
    let mut g = Tb::new()
        .add(
            "src",
            TaggedSource::new(
                Format::new(Kind::Symbols, 1000.0),
                Data::Symbols(sym),
                &[n],
                64,
            ),
        )
        .add("t", TrellisDecode::on_symbols())
        .connect("src", "t")
        .build()
        .unwrap();
    let got = run(&mut g, 12, &["t.out"]);
    let out = hard_bits(&got[0].data);
    assert_eq!(out.len(), bits.len());
    assert_eq!(out, bits);
}

#[test]
fn settings_are_strict() {
    let mut t = TrellisDecode::default();
    assert!(set(&mut t, "trellis", &Value::Text("dmr34".into())).is_err());
    assert!(set(&mut t, "trellis", &Value::Text("p25_1_2".into())).is_ok());
    assert!(set(&mut t, "phases", &Value::F64(0.0)).is_err());
    assert!(set(&mut t, "chunk", &Value::F64(8.0)).is_err());
    assert!(set(&mut t, "depth", &Value::Text("deep".into())).is_err());
    assert!(set(&mut t, "rate", &Value::F64(1.0)).is_err());
    let mut g = Tb::new()
        .add(
            "src",
            Source::new(Format::new(Kind::Real, 1.0), Data::Real(vec![]), 1),
        )
        .add("t", TrellisDecode::default())
        .connect("src", "t")
        .set("t.chunk", Value::F64(16.0))
        .set("t.depth", Value::F64(64.0))
        .build()
        .unwrap();
    assert!(g.configure_input(Format::iq(1.0, 0.0, 0.0)).is_err());
}

#[test]
fn dmr_and_p25_words_decode_after_a_frame_start() {
    for (name, code) in [
        ("dmr", tcm::DMR_3_4),
        ("p25_3_4", tcm::P25_3_4),
        ("p25_1_2", tcm::P25_1_2),
    ] {
        let mut rng = Rng(21);
        let mut codec = TcmCodec::new(code).unwrap();
        // 5 symbols of junk, then (from a frame start) three words back to back with noise.
        let mut levels: Vec<f32> = vec![0.5; 5];
        let mut bits = Vec::new();
        for _ in 0..3 {
            let info: Vec<u8> = (0..code.info_bits()).map(|_| rng.below(2) as u8).collect();
            let mut sent = [0u8; 196];
            codec.encode(&info, &mut sent).unwrap();
            levels.extend(
                sent.as_chunks::<2>()
                    .0
                    .iter()
                    .map(|c| tcm::dibit_symbol((c[0] << 1) | c[1]) as f32 + 0.3 * rng.gauss()),
            );
            bits.extend(info);
        }
        for per_pass in [1usize, 50, levels.len()] {
            let mut g = Tb::new()
                .add(
                    "src",
                    TaggedSource::new(
                        Format::new(Kind::Real, 4800.0),
                        Data::Real(levels.clone()),
                        &[5],
                        per_pass,
                    ),
                )
                .add("t", TrellisDecode::default())
                .connect("src", "t")
                .set("t.trellis", txt(name))
                .build()
                .unwrap();
            let got = run(&mut g, levels.len() / per_pass + 2, &["t.out"]);
            assert_eq!(
                hard_bits(&got[0].data),
                bits,
                "{name}, pieces of {per_pass}"
            );
        }
    }
}
