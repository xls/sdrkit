//! Known-answer tests of the voice tables: the dPMR Hamming (12,8) code and interleaves (ETSI TS 102 490 V1.9.1 clause 7), the
//! NXDN puncturing patterns and vocoder test frames (NXDN TS 1-A Version 1.3 clauses 4.5 and 7.2), and the run-time vocoder
//! layout. The last test counts allocations on the frame paths. (The counting allocator needs `unsafe impl GlobalAlloc`.)
#![allow(unsafe_code)]

use sdrkit_algo_fec::conv::{depuncture_frame, puncture, Puncture};
use sdrkit_algo_fec::small::SmallCode;
use sdrkit_algo_fec::voice::*;
use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;

// ----- dPMR -----

/// ETSI TS 102 490 V1.9.1 table 7.1 (and TS 102 658 V2.6.1 table 7.2) as printed: columns X7 X6 X5 X4 X3 X2 X1 1 C3 C2 C1 C0.
const DPMR_TABLE_7_1: [&str; 8] = [
    "100000001110",
    "010000000111",
    "001000001010",
    "000100000101",
    "000010001011",
    "000001001100",
    "000000100110",
    "000000010011",
];

#[test]
fn dpmr_hamming_12_8_is_the_generator_of_table_7_1() {
    let c = SmallCode::hamming_12_8();
    assert_eq!((c.n(), c.k()), (12, 8));
    for (j, row) in DPMR_TABLE_7_1.iter().enumerate() {
        let g = u128::from_str_radix(row, 2).unwrap();
        // Row j is the code word of data bit X(7 - j), sent first as the most significant bit.
        assert_eq!(c.encode(1 << (7 - j)), g, "row {}", j + 1);
        assert_eq!(c.rows()[j] as u128, g & 0xF);
    }
    // Sums of rows: all ones encodes to the XOR of every parity column.
    assert_eq!(c.encode(0x80), 0x80E);
    assert_eq!(c.encode(0x01), 0x013);
    assert_eq!(c.encode(0xFF), 0xFF4);
    // A Hamming code: minimum distance 3, so every single error is corrected.
    let dmin = (1..256u128)
        .map(|d| c.encode(d).count_ones())
        .min()
        .unwrap();
    assert_eq!(dmin, 3);
    for d in [0x00u128, 0x5A, 0xFF] {
        let w = c.encode(d);
        for e in 0..12 {
            assert_eq!(c.decode(w ^ 1 << e), Some((w, 1)));
        }
    }
}

#[test]
fn dpmr_tch_interleave_matches_table_7_2() {
    let p = dpmr_tch_interleave();
    assert_eq!(p.len(), 72);
    let input: Vec<u16> = (1..=72).collect();
    let mut sent = vec![0u16; 72];
    p.apply(&input, &mut sent).unwrap();
    // Clause 7.5: "the output is a vector ordered as [1,13,25,37,49,61 ... 72]".
    assert_eq!(&sent[..8], &[1, 13, 25, 37, 49, 61, 2, 14]);
    assert_eq!(sent[71], 72);
    // Table 7.2: row r holds r, 12 + r ... 60 + r, and the rows leave in turn.
    for (r, row) in sent.chunks(6).enumerate() {
        let want: Vec<u16> = (0..6).map(|c| (c * 12 + r + 1) as u16).collect();
        assert_eq!(row, &want[..], "row {}", r + 1);
    }
    // Clause 7.5: the receive side outputs "[1,7,13,19,25,31 ... 72]" for the input 1 ... 72.
    let mut back = vec![0u16; 72];
    p.invert(&input, &mut back).unwrap();
    assert_eq!(&back[..6], &[1, 7, 13, 19, 25, 31]);
    p.invert(&sent, &mut back).unwrap();
    assert_eq!(back, input);
}

#[test]
fn dpmr_hi_interleave_matches_table_7_3() {
    let p = dpmr_hi_interleave();
    assert_eq!(p.len(), 120);
    let input: Vec<u16> = (1..=120).collect();
    let mut sent = vec![0u16; 120];
    p.apply(&input, &mut sent).unwrap();
    // Table 7.3, rows 1 and 12 as printed.
    assert_eq!(&sent[..10], &[1, 13, 25, 37, 49, 61, 73, 85, 97, 109]);
    assert_eq!(&sent[110..], &[12, 24, 36, 48, 60, 72, 84, 96, 108, 120]);
    for (r, row) in sent.chunks(10).enumerate() {
        let want: Vec<u16> = (0..10).map(|c| (c * 12 + r + 1) as u16).collect();
        assert_eq!(row, &want[..], "row {}", r + 1);
    }
}

// ----- NXDN puncturing -----

/// Serial pattern of a matrix printed as a G1 row over a G2 row: one column at a time, G1 first.
fn serial(g1: &str, g2: &str) -> Vec<u8> {
    g1.bytes()
        .zip(g2.bytes())
        .flat_map(|(a, b)| [a - b'0', b - b'0'])
        .collect()
}

#[test]
fn nxdn_puncturing_matches_the_printed_matrices() {
    // NXDN TS 1-A Version 1.3 clause 4.5, item (5) of each channel, as printed.
    let printed: [(Puncture<'static>, &str, &str); 6] = [
        (NXDN_CAC_PUNCTURE, "1111111", "1011101"),
        (NXDN_LONG_CAC_PUNCTURE, "1111111111111", "0110001110111"),
        (NXDN_SACCH_PUNCTURE, "111111", "110110"),
        (NXDN_FACCH1_PUNCTURE, "11", "01"),
        (NXDN_UDCH_PUNCTURE, "1111111", "1011101"),
        (NXDN_FACCH2_PUNCTURE, "1111111", "1011101"),
    ];
    for (p, g1, g2) in printed {
        assert_eq!(p.keep(), &serial(g1, g2)[..], "{g1} / {g2}");
    }
}

#[test]
fn nxdn_cac_puncturing_follows_the_worked_example() {
    // Clause 4.5.4: of X1 ... X14, X4 and X12 are erased; X1 ... X350 become Y1 ... Y300.
    let x: Vec<u16> = (1..=350).collect();
    let mut y = Vec::new();
    puncture(&NXDN_CAC_PUNCTURE, 0, &x, &mut y);
    assert_eq!(y.len(), 300);
    assert_eq!(&y[..12], &[1, 2, 3, 5, 6, 7, 8, 9, 10, 11, 13, 14]);
    assert_eq!(*y.last().unwrap(), 350);
}

#[test]
fn nxdn_puncturing_produces_the_coded_lengths() {
    // Code bits = 2 x (information + CRC + 4 tail bits); sent bits = the interleave matrix of clause 4.5.
    let cases: [(Puncture<'static>, usize, usize); 5] = [
        (NXDN_CAC_PUNCTURE, 2 * (155 + 16 + 4), 12 * 25),
        (NXDN_LONG_CAC_PUNCTURE, 2 * (136 + 16 + 4), 12 * 21),
        (NXDN_SACCH_PUNCTURE, 2 * (26 + 6 + 4), 12 * 5),
        (NXDN_FACCH1_PUNCTURE, 2 * (80 + 12 + 4), 16 * 9),
        (NXDN_UDCH_PUNCTURE, 2 * (184 + 15 + 4), 12 * 29),
    ];
    for (p, coded, sent) in cases {
        assert_eq!(coded % p.period(), 0, "whole periods");
        assert_eq!(p.punctured_len(0, coded), sent);
        // A depunctured frame has the sent values in the kept places and erasures elsewhere.
        let values: Vec<i8> = (0..sent).map(|i| (i % 100 + 1) as i8).collect();
        let mut full = vec![0i8; coded];
        depuncture_frame(&p, 0, &values, &mut full).unwrap();
        assert_eq!(full.iter().filter(|&&v| v == 0).count(), coded - sent);
        let mut again = Vec::new();
        puncture(&p, 0, &full, &mut again);
        assert_eq!(again, values);
    }
    // The short CAC has no puncturing: 2 x (106 + 16 + 4) = 252 = 12 x 21.
    assert_eq!(2 * (106 + 16 + 4), 12 * 21);
}

// ----- Vocoder frames -----

#[test]
fn nxdn_test_frames_have_their_printed_lengths() {
    for f in [NXDN_EHR_TONE, NXDN_EHR_SILENCE] {
        assert_eq!((f.info_bits, f.coded_bits), (49, 72));
        assert_eq!(f.info.len(), 7);
        assert_eq!(f.coded.len(), 9);
    }
    for f in NXDN_EFR_TONE.iter().chain(&NXDN_EFR_SILENCE) {
        assert_eq!((f.info_bits, f.coded_bits), (88, 144));
        assert_eq!((f.info.len(), f.coded.len()), (11, 18));
    }
    // Clause 7.2.1.1: "FEE2 1212 1210 0 (first 49 bit are valid.)".
    assert_eq!(NXDN_EHR_TONE.info_bit(0), 1);
    assert_eq!(NXDN_EHR_TONE.info_bit(7), 0);
    assert_eq!(NXDN_EHR_TONE.info_bit(49), 0);
    assert_eq!(NXDN_EHR_TONE.coded_bit(71), 0);
    assert_eq!(NXDN_EHR_SILENCE.coded_bit(71), 1);
    // Clause 7.2.2: the two frames of each pair differ only in their last bits.
    assert_eq!(NXDN_EFR_TONE[0].info, NXDN_EFR_TONE[1].info);
    assert_eq!(
        NXDN_EFR_SILENCE[0].coded[..17],
        NXDN_EFR_SILENCE[1].coded[..17]
    );
}

#[test]
fn voice_layout_rejects_bad_tables() {
    assert!(VoiceLayout::new(&[0, 1, 1], &[3]).is_err());
    assert!(VoiceLayout::new(&[0, 1, 3], &[3]).is_err());
    assert!(VoiceLayout::new(&[0, 1, 2], &[2]).is_err());
    assert!(VoiceLayout::new(&[0, 1, 2], &[]).is_err());
    assert!(VoiceLayout::new(&[0, 1, 2], &[0, 3]).is_err());
    let l = VoiceLayout::new(&[2, 0, 1], &[1, 2]).unwrap();
    assert_eq!((l.len(), l.word_lens()), (3, &[1usize, 2][..]));
    assert!(l.split(&[0u8; 2], &mut [0u8; 3]).is_err());
    assert!(l.join(&[0u8; 3], &mut [0u8; 4]).is_err());
    assert!(l.word(&[0u8; 3], 2).is_err());
    assert!(l.word(&[0u8; 2], 0).is_err());
}

/// A layout made to agree with the two EHR frames: each checked place takes an unused channel bit with the same values in both
/// frames, and C1 takes the rest.
fn layout_agreeing_with_the_ehr_frames() -> VoiceLayout {
    let golay = SmallCode::golay_24_12();
    let frames = [NXDN_EHR_TONE, NXDN_EHR_SILENCE];
    let want = |f: &VocoderTestFrame, place: usize| -> Option<u8> {
        let c0 = golay.encode((0..12).fold(0u128, |v, i| v << 1 | f.info_bit(i) as u128));
        match place {
            0..=23 => Some((c0 >> (23 - place) & 1) as u8),
            24..=46 => None,
            _ => Some(f.info_bit(place - 47 + 24)),
        }
    };
    let mut dest = [u16::MAX; 72];
    for place in (0..72).filter(|p| !(24..=46).contains(p)) {
        let pair = (want(&frames[0], place), want(&frames[1], place));
        let i = (0..72)
            .find(|&i| {
                dest[i] == u16::MAX
                    && (Some(frames[0].coded_bit(i)), Some(frames[1].coded_bit(i))) == pair
            })
            .expect("a free channel bit with these values");
        dest[i] = place as u16;
    }
    let mut c1 = 24u16..47;
    for d in dest.iter_mut().filter(|d| **d == u16::MAX) {
        *d = c1.next().unwrap();
    }
    VoiceLayout::new(&dest, &AMBE_HALF_RATE_WORDS).unwrap()
}

#[test]
fn half_rate_check_accepts_only_layouts_that_agree_with_the_ehr_frames() {
    let good = layout_agreeing_with_the_ehr_frames();
    assert!(check_ambe_half_rate(&good).is_ok());
    let identity: Vec<u16> = (0..72).collect();
    let plain = VoiceLayout::new(&identity, &AMBE_HALF_RATE_WORDS).unwrap();
    assert!(check_ambe_half_rate(&plain).is_err());
    let other_shape = VoiceLayout::new(&identity, &[24, 24, 24]).unwrap();
    assert!(check_ambe_half_rate(&other_shape).is_err());
    // Split then join returns the original frame.
    let ch: Vec<u8> = (0..72).map(|i| NXDN_EHR_TONE.coded_bit(i)).collect();
    let (mut w, mut back) = ([0u8; 72], [0u8; 72]);
    good.split(&ch, &mut w).unwrap();
    good.join(&w, &mut back).unwrap();
    assert_eq!(&back[..], &ch[..]);
}

// ----- Allocations -----

struct Counting;

thread_local! {
    static ALLOCS: Cell<usize> = const { Cell::new(0) };
}

unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, l: Layout) -> *mut u8 {
        let _ = ALLOCS.try_with(|n| n.set(n.get() + 1));
        unsafe { System.alloc(l) }
    }
    unsafe fn dealloc(&self, p: *mut u8, l: Layout) {
        unsafe { System.dealloc(p, l) }
    }
    unsafe fn realloc(&self, p: *mut u8, l: Layout, n: usize) -> *mut u8 {
        let _ = ALLOCS.try_with(|c| c.set(c.get() + 1));
        unsafe { System.realloc(p, l, n) }
    }
}

#[global_allocator]
static A: Counting = Counting;

#[test]
fn frame_paths_allocate_nothing() {
    let layout = layout_agreeing_with_the_ehr_frames();
    let tch = dpmr_tch_interleave();
    let ch = [1u8; 72];
    let mut w = [0u8; 72];
    let mut back = [0u8; 72];
    let sent = [5i8; 60];
    let mut full = [0i8; 72];
    let before = ALLOCS.with(|n| n.get());
    for _ in 0..1000 {
        layout.split(&ch, &mut w).unwrap();
        layout.join(&w, &mut back).unwrap();
        let _ = layout.word(&w, 3).unwrap();
        tch.apply(&ch, &mut w).unwrap();
        tch.invert(&w, &mut back).unwrap();
        depuncture_frame(&NXDN_SACCH_PUNCTURE, 0, &sent, &mut full).unwrap();
    }
    assert_eq!(ALLOCS.with(|n| n.get()) - before, 0);
}

#[test]
fn the_named_ambe_half_rate_layout_agrees_with_the_ehr_frames() {
    let l = VoiceLayout::ambe_half_rate();
    assert_eq!((l.len(), l.word_lens()), (72, &AMBE_HALF_RATE_WORDS[..]));
    check_ambe_half_rate(&l).unwrap();
}
