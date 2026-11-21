//! The bit-stream components in small graphs: line decoders, descramblers, the sync search, deinterleavers and the conversions,
//! each against a known answer, cut into pieces of several sizes, with hard bits and LLRs, and with their settings refused when
//! bad.

mod bits_util;

use bits_util::*;
use sdrkit_core::{SettingError, Value};
use sdrkit_graph::stream::Data;
use sdrkit_graph::{Format, Kind};
use sdrkit_util_bits::interleave::{DelayLanes, Permutation, ReadOrder};
use sdrkit_util_bits::lfsr::{Additive, Lfsr};
use sdrkit_util_bits::line::{Manchester, ManchesterConv, NrzM, Nrzi};
use sdrkit_util_bits::{pack, seq, BitOrder};

fn hb(rate: f64) -> Format {
    Format::hard_bits(rate)
}

fn txt(s: &str) -> Value {
    Value::Text(s.into())
}

fn num(x: f64) -> Value {
    Value::F64(x)
}

/// The 32 bits of the CCSDS attached sync marker.
fn asm() -> Vec<u8> {
    let mut v = Vec::new();
    pack::unpack(&[0x1A, 0xCF, 0xFC, 0x1D], BitOrder::Msb, &mut v);
    v
}

fn not(b: &[u8]) -> Vec<u8> {
    b.iter().map(|x| x ^ 1).collect()
}

// ----- line decoders -----

#[test]
fn nrzi_nrzm_and_diff_undo_their_encoders_hard_and_soft() {
    let data = random_bits(501, 1);
    type Enc = fn(&mut [u8], u8);
    let nrzi: Enc = |b, l| Nrzi::new(l).encode(b);
    let nrzm: Enc = |b, l| NrzM::new(l).encode(b);
    for (name, enc) in [
        ("bits.nrzi", nrzi),
        ("bits.nrzm", nrzm),
        ("bits.diff", nrzm),
    ] {
        for level in [0u8, 1] {
            let mut line = data.clone();
            enc(&mut line, level);
            let set = [("d.level", num(level as f64))];
            for &piece in PIECES {
                let r = run_chain(
                    hb(9600.0),
                    Data::HardBits(line.clone()),
                    piece,
                    &[("d", name)],
                    &set,
                    "d.out",
                );
                assert_eq!(hard(&r.data), data, "{name} level {level} piece {piece}");
                let r = run_chain(
                    Format::llr(9600.0, Some(4.0)),
                    Data::Llr(soft(&line, 40)),
                    piece,
                    &[("d", &format!("{name}_llr"))],
                    &set,
                    "d.out",
                );
                let l = llrs(&r.data);
                assert_eq!(decide(&l), data, "{name} soft, level {level} piece {piece}");
                // The XOR of two levels of magnitude 40 is as sure as either.
                assert!(l.iter().all(|v| v.unsigned_abs() == 40), "{name}");
            }
        }
    }
    // The LLR stream keeps its scale.
    let mut g = chain(
        Format::llr(9600.0, Some(4.0)),
        Data::Llr(vec![1, -1]),
        2,
        &[("d", "bits.nrzi_llr")],
        &[],
    );
    run(&mut g, 2, "d.out");
    assert_eq!(g.format("d.out").unwrap().llr_scale, Some(4.0));
    assert_eq!(g.format("d.out").unwrap().kind, Kind::Llr);
}

#[test]
fn manchester_decodes_both_conventions_hard_and_soft() {
    let data = random_bits(301, 2);
    for (conv, key) in [
        (ManchesterConv::Ieee, "ieee"),
        (ManchesterConv::Thomas, "thomas"),
    ] {
        let mut chips = Vec::new();
        Manchester::encode(&data, conv, &mut chips);
        let set = [("m.convention", txt(key))];
        for &piece in PIECES {
            let r = run_chain(
                hb(2400.0),
                Data::HardBits(chips.clone()),
                piece,
                &[("m", "bits.manchester")],
                &set,
                "m.out",
            );
            assert_eq!(hard(&r.data), data, "{key} piece {piece}");
            let r = run_chain(
                Format::llr(2400.0, None),
                Data::Llr(soft(&chips, 30)),
                piece,
                &[("m", "bits.manchester_llr")],
                &set,
                "m.out",
            );
            let l = llrs(&r.data);
            assert_eq!(decide(&l), data, "{key} soft piece {piece}");
            // Two chips of 30 add to 60.
            assert!(l.iter().all(|v| v.unsigned_abs() == 60));
        }
    }
    // The output runs at half the chip rate.
    let mut g = chain(
        hb(2400.0),
        Data::HardBits(vec![0, 1]),
        2,
        &[("m", "bits.manchester")],
        &[],
    );
    run(&mut g, 2, "m.out");
    assert_eq!(g.format("m.out").unwrap().rate, 1200.0);
}

#[test]
fn manchester_counts_bad_pairs_and_takes_its_pair_phase_from_a_frame_start() {
    let data = random_bits(200, 3);
    let mut chips = Vec::new();
    Manchester::encode(&data, ManchesterConv::Ieee, &mut chips);
    let mut bad = chips.clone();
    bad[10] = bad[11]; // pair 5 is 00 or 11
    let mut g = chain(
        hb(4.0),
        Data::HardBits(bad),
        16,
        &[("m", "bits.manchester")],
        &[],
    );
    let r = run(&mut g, 40, "m.out");
    assert_eq!(hard(&r.data).len(), data.len());
    assert_eq!(result_i64(&g, "m", "pairs_bad"), 1);

    // One stray chip, then the sync word, then the chips: the pairs start after the word, where bits.sync puts its tag.
    let mut line = vec![1u8];
    line.extend(asm());
    line.extend(&chips);
    for &piece in PIECES {
        let r = run_chain(
            hb(2400.0),
            Data::HardBits(line.clone()),
            piece,
            &[("s", "bits.sync"), ("m", "bits.manchester")],
            &[],
            "m.out",
        );
        let starts = frame_starts(&r.tags);
        assert_eq!(starts.len(), 1, "piece {piece}");
        let at = starts[0].0 as usize;
        assert_eq!(
            at, 16,
            "the tag lands on the first bit of the pairs after the word"
        );
        assert_eq!(&hard(&r.data)[at..], &data[..], "piece {piece}");
    }
}

// ----- descramblers -----

#[test]
fn dvb_dispersal_returns_its_published_first_bytes() {
    // EN 300 421 clause 4.4.1: the first output bytes of the PRBS on zero data are 0x03, 0xF6.
    let zeros = vec![0u8; 64];
    for &piece in PIECES {
        let r = run_chain(
            hb(1e6),
            Data::HardBits(zeros.clone()),
            piece,
            &[("d", "bits.descramble"), ("p", "bits.pack")],
            &[
                ("d.sequence", txt("dvb_dispersal")),
                ("p.frame_bits", num(16.0)),
                ("p.start", txt("continuous")),
            ],
            "p.out",
        );
        let p = packets(&r.data);
        assert_eq!(p[0], vec![0x03, 0xF6], "piece {piece}");
        let mut want = vec![0u8; 8];
        Additive::new(seq::DVB_DISPERSAL).apply_bytes(&mut want, BitOrder::Msb);
        assert_eq!(p.concat(), want);
    }
}

#[test]
fn ccsds_randomiser_returns_its_published_first_bytes() {
    // CCSDS 131.0-B clause 10: FF 48 0E C0 9A.
    let r = run_chain(
        hb(1e6),
        Data::HardBits(vec![0u8; 40]),
        5,
        &[("d", "bits.descramble"), ("p", "bits.pack")],
        &[
            ("d.sequence", txt("ccsds_randomiser")),
            ("p.frame_bits", num(40.0)),
            ("p.start", txt("continuous")),
        ],
        "p.out",
    );
    assert_eq!(packets(&r.data), vec![vec![0xFF, 0x48, 0x0E, 0xC0, 0x9A]]);
}

#[test]
fn descrambler_flips_llr_signs_restarts_at_frames_and_by_period() {
    let mut prbs = vec![0u8; 200];
    Additive::new(seq::DVB_DISPERSAL).apply_bits(&mut prbs);
    // Soft: zeros at +50 come out as -50 where the sequence is 1.
    for &piece in PIECES {
        let r = run_chain(
            Format::llr(1e6, Some(2.0)),
            Data::Llr(vec![50; 200]),
            piece,
            &[("d", "bits.descramble_llr")],
            &[("d.sequence", txt("dvb_dispersal"))],
            "d.out",
        );
        let l = llrs(&r.data);
        assert_eq!(decide(&l), prbs);
        assert!(l.iter().all(|v| v.unsigned_abs() == 50));
    }
    // A period of 16 bits.
    let r = run_chain(
        hb(1e6),
        Data::HardBits(vec![0; 100]),
        9,
        &[("d", "bits.descramble")],
        &[
            ("d.sequence", txt("dvb_dispersal")),
            ("d.period", num(16.0)),
        ],
        "d.out",
    );
    let out = hard(&r.data);
    for (k, &b) in out.iter().enumerate() {
        assert_eq!(b, prbs[k % 16]);
    }
    // Frames found by bits.sync: the sequence starts again at each frame's first bit.
    let mut line = random_bits(23, 4);
    for _ in 0..3 {
        line.extend(asm());
        line.extend(vec![0u8; 64]);
        line.extend(random_bits(11, 5));
    }
    for &piece in PIECES {
        let r = run_chain(
            hb(1e6),
            Data::HardBits(line.clone()),
            piece,
            &[("s", "bits.sync"), ("d", "bits.descramble")],
            &[("d.sequence", txt("dvb_dispersal"))],
            "d.out",
        );
        let out = hard(&r.data);
        let starts = frame_starts(&r.tags);
        assert_eq!(starts.len(), 3);
        for (at, _, _) in starts {
            let at = at as usize;
            assert_eq!(&out[at..at + 64], &prbs[..64], "piece {piece}");
        }
    }
}

#[test]
fn custom_registers_match_the_plain_ones() {
    let mut dvb = vec![0u8; 300];
    Additive::new(seq::DVB_DISPERSAL).apply_bits(&mut dvb);
    let r = run_chain(
        hb(1e6),
        Data::HardBits(vec![0; 300]),
        13,
        &[("d", "bits.descramble")],
        &[
            ("d.sequence", txt("custom")),
            ("d.poly", num(0xC001 as f64)),
            ("d.seed", num(0xA9 as f64)),
        ],
        "d.out",
    );
    assert_eq!(hard(&r.data), dvb);
    // A Galois register, and a Fibonacci one read at its oldest stage.
    for (form, output, l) in [
        ("galois", "feedback", Lfsr::galois(0x18D, 0x74)),
        (
            "fibonacci",
            "oldest",
            Lfsr::fibonacci(0x12B, 0xFF).with_output(0x80),
        ),
    ] {
        let (poly, seed) = if form == "galois" {
            (0x18D, 0x74)
        } else {
            (0x12B, 0xFF)
        };
        let mut want = vec![0u8; 300];
        Additive::new(l).apply_bits(&mut want);
        let r = run_chain(
            hb(1e6),
            Data::HardBits(vec![0; 300]),
            7,
            &[("d", "bits.descramble")],
            &[
                ("d.sequence", txt("custom")),
                ("d.poly", num(poly as f64)),
                ("d.seed", num(seed as f64)),
                ("d.form", txt(form)),
                ("d.output", txt(output)),
            ],
            "d.out",
        );
        assert_eq!(hard(&r.data), want, "{form}");
    }
}

#[test]
fn self_sync_descrambler_locks_without_a_seed() {
    let data = random_bits(600, 6);
    for (name, mut tx, n) in [
        ("g3ruh", seq::g3ruh(0x1ABCD), 17),
        ("ieee80211b", seq::ieee80211b(0x5A), 7),
        ("custom", seq::g3ruh(0x0F0F), 17),
    ] {
        let mut line = data.clone();
        tx.scramble(&mut line);
        let set = [("d.scrambler", txt(name)), ("d.poly", num(0x21001 as f64))];
        for &piece in PIECES {
            let r = run_chain(
                hb(9600.0),
                Data::HardBits(line.clone()),
                piece,
                &[("d", "bits.self_sync")],
                &set,
                "d.out",
            );
            assert_eq!(&hard(&r.data)[n..], &data[n..], "{name} piece {piece}");
            let r = run_chain(
                Format::llr(9600.0, None),
                Data::Llr(soft(&line, 99)),
                piece,
                &[("d", "bits.self_sync_llr")],
                &set,
                "d.out",
            );
            assert_eq!(&decide(&llrs(&r.data))[n..], &data[n..], "{name} soft");
        }
    }
}

// ----- sync search -----

/// A stream with a sync word, a payload, the word with two errors after which the channel inverts everything, and the word
/// with three errors (too many). Returns (what was sent, what was received).
fn sync_stream() -> (Vec<u8>, Vec<u8>) {
    let mut sent = random_bits(100, 7);
    sent.extend(asm());
    sent.extend(random_bits(40, 8));
    let mut w = asm();
    w[3] ^= 1;
    w[20] ^= 1;
    sent.extend(&w);
    sent.extend(random_bits(40, 9));
    let mut w3 = asm();
    w3[1] ^= 1;
    w3[2] ^= 1;
    w3[30] ^= 1;
    sent.extend(&w3);
    sent.extend(random_bits(40, 10));
    let mut got = sent[..172].to_vec();
    got.extend(not(&sent[172..]));
    (sent, got)
}

#[test]
fn sync_marks_frame_starts_with_errors_and_inverts_back() {
    let (sent, got) = sync_stream();
    let set = [
        ("s.pattern", txt("0x1ACFFC1D")),
        ("s.max_errors", num(2.0)),
        ("s.inverted", Value::Bool(true)),
    ];
    for &piece in PIECES {
        let r = run_chain(
            hb(1e3),
            Data::HardBits(got.clone()),
            piece,
            &[("s", "bits.sync")],
            &set,
            "s.out",
        );
        assert_eq!(
            frame_starts(&r.tags),
            vec![(132, false, 0), (204, true, 2)],
            "piece {piece}"
        );
        let out = hard(&r.data);
        assert_eq!(&out[..204], &got[..204]);
        assert_eq!(&out[204..], &sent[204..]);
        // The same from LLRs.
        let r = run_chain(
            Format::llr(1e3, Some(1.0)),
            Data::Llr(soft(&got, 9)),
            piece,
            &[("s", "bits.sync_llr")],
            &set,
            "s.out",
        );
        assert_eq!(frame_starts(&r.tags), vec![(132, false, 0), (204, true, 2)]);
        assert_eq!(&decide(&llrs(&r.data))[204..], &sent[204..]);
    }
    // Without inverted words, only the first; with a hold-off longer than the rest, only the first too.
    let r = run_chain(
        hb(1e3),
        Data::HardBits(got.clone()),
        50,
        &[("s", "bits.sync")],
        &set[..2],
        "s.out",
    );
    assert_eq!(frame_starts(&r.tags), vec![(132, false, 0)]);
    let mut held = set.to_vec();
    held.push(("s.holdoff", num(1000.0)));
    let mut g = chain(
        hb(4.0),
        Data::HardBits(got.clone()),
        50,
        &[("s", "bits.sync")],
        &held,
    );
    let r = run(&mut g, 10, "s.out");
    assert_eq!(frame_starts(&r.tags), vec![(132, false, 0)]);
    assert_eq!(result_i64(&g, "s", "sync_words"), 1);
    // A binary pattern: the HDLC flag.
    let mut line = random_bits(30, 11).iter().map(|_| 0).collect::<Vec<u8>>();
    line.extend([0, 1, 1, 1, 1, 1, 1, 0, 1, 0]);
    let r = run_chain(
        hb(1e3),
        Data::HardBits(line),
        4,
        &[("s", "bits.sync")],
        &[("s.pattern", txt("0b01111110"))],
        "s.out",
    );
    assert_eq!(frame_starts(&r.tags), vec![(38, false, 0)]);
}

// ----- deinterleavers -----

#[test]
fn block_deinterleavers_undo_their_interleavers() {
    let cases: Vec<(Permutation, Vec<(&str, Value)>)> = vec![
        (
            Permutation::row_column(8, 6, ReadOrder::Columns).unwrap(),
            vec![
                ("d.kind", txt("row_column")),
                ("d.rows", num(8.0)),
                ("d.cols", num(6.0)),
            ],
        ),
        (
            Permutation::wifi(192, 4, 16).unwrap(),
            vec![
                ("d.kind", txt("wifi")),
                ("d.n_cbps", num(192.0)),
                ("d.n_bpsc", num(4.0)),
                ("d.n_col", num(16.0)),
            ],
        ),
        (
            Permutation::dvbt_bit(3).unwrap(),
            vec![("d.kind", txt("dvbt_bit")), ("d.branch", num(3.0))],
        ),
        (
            Permutation::hfdl(1800, 2).unwrap(),
            vec![
                ("d.kind", txt("hfdl")),
                ("d.length", num(1800.0)),
                ("d.shift", num(2.0)),
            ],
        ),
        (
            Permutation::lora(7, 8).unwrap(),
            vec![
                ("d.kind", txt("lora")),
                ("d.rows", num(7.0)),
                ("d.cols", num(8.0)),
            ],
        ),
        (
            Permutation::drm(1000, 5).unwrap(),
            vec![
                ("d.kind", txt("drm")),
                ("d.length", num(1000.0)),
                ("d.t0", num(5.0)),
            ],
        ),
    ];
    for (perm, mut set) in cases {
        let n = perm.len();
        let data = random_bits(3 * n, 12);
        let mut line = vec![0u8; 3 * n];
        for (i, o) in data.chunks(n).zip(line.chunks_mut(n)) {
            perm.apply(i, o).unwrap();
        }
        set.push(("d.start", txt("continuous")));
        for &piece in PIECES {
            let r = run_chain(
                hb(1e3),
                Data::HardBits(line.clone()),
                piece,
                &[("d", "bits.deinterleave")],
                &set,
                "d.out",
            );
            assert_eq!(hard(&r.data), data, "{:?} piece {piece}", set[0]);
            let r = run_chain(
                Format::llr(1e3, None),
                Data::Llr(soft(&line, 77)),
                piece,
                &[("d", "bits.deinterleave_llr")],
                &set,
                "d.out",
            );
            assert_eq!(decide(&llrs(&r.data)), data);
        }
    }
}

#[test]
fn block_deinterleaver_takes_its_blocks_from_frame_starts() {
    let perm = Permutation::row_column(8, 8, ReadOrder::Columns).unwrap();
    let (a, b) = (random_bits(64, 13), random_bits(64, 14));
    let mut ia = vec![0u8; 64];
    let mut ib = vec![0u8; 64];
    perm.apply(&a, &mut ia).unwrap();
    perm.apply(&b, &mut ib).unwrap();
    let mut line = random_bits(13, 15);
    line.extend(asm());
    line.extend(&ia);
    line.extend(random_bits(5, 16));
    line.extend(asm());
    line.extend(&ib);
    for &piece in PIECES {
        let r = run_chain(
            hb(1e3),
            Data::HardBits(line.clone()),
            piece,
            &[("s", "bits.sync"), ("d", "bits.deinterleave")],
            &[("d.rows", num(8.0)), ("d.cols", num(8.0))],
            "d.out",
        );
        assert_eq!(
            hard(&r.data),
            [a.clone(), b.clone()].concat(),
            "piece {piece}"
        );
        assert_eq!(frame_starts(&r.tags), vec![(0, false, 0), (64, false, 0)]);
    }
}

#[test]
fn delay_lane_deinterleavers_undo_their_interleavers() {
    let data = random_bits(5000, 17);
    // DVB Forney (12, 17): sender and receiver together delay every item by 11 x 17 x 12 = 2244.
    let mut line = data.clone();
    DelayLanes::forney(12, 17, false, 0u8).process(&mut line);
    for &piece in PIECES {
        let r = run_chain(
            hb(1e3),
            Data::HardBits(line.clone()),
            piece,
            &[("d", "bits.deinterleave")],
            &[
                ("d.kind", txt("forney")),
                ("d.branches", num(12.0)),
                ("d.unit", num(17.0)),
            ],
            "d.out",
        );
        assert_eq!(
            &hard(&r.data)[2244..],
            &data[..5000 - 2244],
            "piece {piece}"
        );
    }
    // DAB time interleaving of 64-bit logical frames: 15 frames of delay; LLRs come out with erasures first.
    let mut line = data.clone();
    DelayLanes::dab_time(64, false, 0u8)
        .unwrap()
        .process(&mut line);
    let r = run_chain(
        Format::llr(1e3, None),
        Data::Llr(soft(&line, 5)),
        37,
        &[("d", "bits.deinterleave_llr")],
        &[("d.kind", txt("dab_time")), ("d.cif_bits", num(64.0))],
        "d.out",
    );
    let l = llrs(&r.data);
    assert_eq!(&decide(&l)[15 * 64..], &data[..5000 - 15 * 64]);
    assert!(l[..64].iter().all(|&v| v == 0 || v.unsigned_abs() == 5));
}

// ----- conversions -----

#[test]
fn hard_decision_and_soft_turn_the_llr_sign_round() {
    // Hard and soft.
    let r = run_chain(
        Format::llr(100.0, Some(1.0)),
        Data::Llr(vec![-5, 3, 0, -127]),
        3,
        &[("h", "dsp.hard_decision")],
        &[],
        "h.out",
    );
    assert_eq!(hard(&r.data), vec![1, 0, 0, 1]);
    let mut g = chain(
        hb(100.0),
        Data::HardBits(vec![0, 1, 1]),
        2,
        &[("s", "bits.soft")],
        &[],
    );
    let r = run(&mut g, 4, "s.out");
    assert_eq!(llrs(&r.data), vec![127, -127, -127]);
    assert_eq!(g.format("s.out").unwrap().llr_scale, None);
}

// ----- strict settings -----

#[test]
fn bad_settings_are_refused() {
    let r = sdrkit_components::registry();
    let cases: &[(&str, &str, Value)] = &[
        ("bits.nrzi", "level", num(2.0)),
        ("bits.nrzm", "level", txt("high")),
        ("bits.diff", "level", num(0.5)),
        ("bits.manchester", "convention", txt("manchester")),
        ("bits.descramble", "sequence", txt("dvb")),
        ("bits.descramble", "poly", num(1.0)),
        ("bits.descramble", "seed", num(0x400 as f64)), // does not fit x^9 + x^5 + 1
        ("bits.descramble", "form", txt("ring")),
        ("bits.descramble", "period", num(-1.0)),
        ("bits.self_sync", "scrambler", txt("g3ruh9600")),
        ("bits.self_sync", "poly", num(f64::NAN)),
        ("bits.sync", "pattern", txt("1ACFFC1G")),
        ("bits.sync", "length", num(16.0)), // the 32-bit default word does not fit
        ("bits.sync", "max_errors", num(16.0)),
        ("bits.sync", "inverted", txt("maybe")),
        ("bits.deinterleave", "kind", txt("helical")),
        ("bits.deinterleave", "rows", num(0.0)),
        ("bits.deinterleave", "start", txt("never")),
    ];
    for (name, key, bad) in cases {
        let mut c = r.make(name).unwrap();
        assert!(c.info().settings.id(key).is_ok(), "{name} has no {key}");
        assert!(set(&mut *c, key, bad).is_err(), "{name}.{key} took {bad:?}");
    }
    // Parameters that do not fit their kind together.
    let mut d = r.make("bits.deinterleave").unwrap();
    set(&mut *d, "length", &num(41.0)).unwrap();
    assert!(set(&mut *d, "kind", &txt("hfdl")).is_err());
    set(&mut *d, "length", &num(80.0)).unwrap();
    set(&mut *d, "kind", &txt("hfdl")).unwrap();
    assert!(set(&mut *d, "length", &num(81.0)).is_err());
    // Unknown keys, and the components without settings.
    for name in ["bits.nrzi", "bits.sync", "bits.soft", "bits.deinterleave"] {
        let mut c = r.make(name).unwrap();
        assert!(
            matches!(
                set(&mut *c, "colour", &num(1.0)),
                Err(SettingError::Unknown { .. })
            ),
            "{name}"
        );
    }
    // Through a graph: a value of the wrong kind is refused.
    let mut g = chain(
        hb(1e3),
        Data::HardBits(vec![0; 8]),
        8,
        &[("s", "bits.sync")],
        &[],
    );
    assert!(gset(&mut g, "s.inverted", &txt("perhaps")).is_err());
    assert!(gset(&mut g, "s.pattern", &txt("zz")).is_err());
    assert!(gset(&mut g, "s.pattern", &txt("7E")).is_ok());
}
