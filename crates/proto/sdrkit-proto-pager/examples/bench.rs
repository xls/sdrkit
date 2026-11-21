//! Times the hot paths: `cargo run --release -p sdrkit-proto-pager --example bench`. Each line is the best of several runs, in
//! millions of items per second (the name says what an item is).

use sdrkit_proto_pager::flex::{self, Fiw, FlexDecoder, FlexTx, Speed, PHASE_BITS, PHASE_WORDS};
use sdrkit_proto_pager::pocsag::{self, Content, PocsagDecoder, PocsagTx};
use sdrkit_proto_pager::vote::vote;
use sdrkit_proto_pager::word::{self, Checked, SoftRepair};
use sdrkit_proto_pager::Kind;
use std::hint::black_box;
use std::time::Instant;

fn measure(name: &str, items: usize, mut step: impl FnMut()) {
    step();
    let start = Instant::now();
    let mut best = f64::INFINITY;
    let mut runs = 0;
    while runs < 5 || start.elapsed().as_secs_f64() < 0.5 {
        let t = Instant::now();
        step();
        best = best.min(t.elapsed().as_secs_f64());
        runs += 1;
    }
    println!("{name:44} {:10.2} M/s", items as f64 / best / 1e6);
}

fn main() {
    let words: Vec<u32> = (0..4096u32)
        .map(|i| word::encode(i.wrapping_mul(2_654_435_761) >> 11))
        .collect();
    let two: Vec<u32> = words
        .iter()
        .enumerate()
        .map(|(i, &w)| w ^ (1 << (i % 32)) ^ (1 << ((i + 7) % 32)))
        .collect();
    let three: Vec<u32> = words.iter().map(|&w| w ^ 0x8000_0101).collect();
    let mut rel = [1.0f32; 32];
    // The three wrong bits (31, 8 and 0) sit at indices 0, 23 and 31.
    for b in [0, 23, 31, 5, 9, 12] {
        rel[b] = 0.1;
    }
    measure("word repair, clean (words)", words.len(), || {
        for &w in &words {
            black_box(word::repair(black_box(w)));
        }
    });
    measure("word repair, 2 errors (words)", two.len(), || {
        for &w in &two {
            black_box(word::repair(black_box(w)));
        }
    });
    measure(
        "word soft repair, 3 errors, Chase (words)",
        three.len(),
        || {
            for &w in &three {
                black_box(word::repair_soft(black_box(w), &rel, SoftRepair::MESSAGE));
            }
        },
    );
    let copies: Vec<[Checked; 3]> = three
        .iter()
        .map(|&w| {
            [
                Checked::hard(w),
                Checked::hard(w ^ 0x30),
                Checked::hard(w ^ 0x0300_0000),
            ]
        })
        .collect();
    measure("vote of 3 lost copies (words)", copies.len(), || {
        for c in &copies {
            black_box(vote(black_box(c)));
        }
    });

    let tx = [
        PocsagTx {
            ric: 1_234_567,
            function: 3,
            content: Content::Alpha("The quick brown fox jumps over the lazy dog"),
        },
        PocsagTx {
            ric: 222,
            function: 0,
            content: Content::Numeric("0123456789"),
        },
    ];
    let mut w = Vec::new();
    pocsag::encode_batches(&tx, pocsag::NumericSet::Itu, &mut w).expect("valid pages");
    let mut bits = Vec::new();
    pocsag::transmission_bits(&w, 576, &mut bits);
    let llr: Vec<f32> = bits
        .iter()
        .map(|&b| if b == 1 { -1.0 } else { 1.0 })
        .collect();
    let mut rx = PocsagDecoder::new(pocsag::Config::default()).expect("default settings");
    measure("POCSAG decoder, hard (bits)", bits.len(), || {
        for &b in &bits {
            rx.push_bit(b, 0.0, |p| {
                black_box(p);
            });
        }
    });
    measure("POCSAG decoder, soft (bits)", llr.len(), || {
        for &l in &llr {
            rx.push_llr(l, 0.0, |p| {
                black_box(p);
            });
        }
    });

    let ftx = [
        FlexTx {
            capcode: 1_234_567,
            kind: Kind::Alpha,
            text: "The quick brown fox jumps over the lazy dog",
        },
        FlexTx {
            capcode: 2_201_234,
            kind: Kind::Numeric,
            text: "0123456789",
        },
    ];
    let mut fw = [0u32; PHASE_WORDS];
    flex::encode_phase(&ftx, &mut fw).expect("valid pages");
    let mut fbits = vec![0u8; PHASE_BITS];
    flex::interleave(&fw, &mut fbits).expect("phase length");
    let fllr: Vec<f32> = fbits
        .iter()
        .map(|&b| if b == 1 { -1.0 } else { 1.0 })
        .collect();
    let mut dec = FlexDecoder::new(flex::Config::default()).expect("default settings");
    measure("FLEX phase decode, soft (bits)", PHASE_BITS, || {
        dec.decode_llrs(Speed::F1600_2, Fiw::default(), 0, &fllr, 0.0, |p| {
            black_box(p);
        })
        .expect("phase length");
    });
    measure("FLEX phase decode, hard (bits)", PHASE_BITS, || {
        dec.decode_bits(Speed::F1600_2, Fiw::default(), 0, &fbits, 0.0, |p| {
            black_box(p);
        })
        .expect("phase length");
    });
}
