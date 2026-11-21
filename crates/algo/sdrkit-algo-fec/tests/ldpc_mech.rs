//! LDPC mechanisms: the alist reader and writer, early stop of belief propagation, the sparse encoder for large codes, and
//! quasi-cyclic codes with masked lanes.
#![allow(unsafe_code)]

mod common;
use common::*;
use sdrkit_algo_fec::ldpc::alist::{Alist, AlistError};
use sdrkit_algo_fec::ldpc::tables::{dvb_s2, wsjt};
use sdrkit_algo_fec::ldpc::{BpDecoder, InfoAt, LdpcCode, LdpcDecoder, LdpcEncoder, MinSum};
use sdrkit_algo_fec::FecError;
use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;

// Counts the allocations of the current thread only, so tests that run at the same time do not disturb each other.
struct Counting;

thread_local! {
    static ALLOCS: Cell<usize> = const { Cell::new(0) };
}

fn bump() {
    let _ = ALLOCS.try_with(|c| c.set(c.get() + 1));
}

unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, l: Layout) -> *mut u8 {
        bump();
        unsafe { System.alloc(l) }
    }
    unsafe fn dealloc(&self, p: *mut u8, l: Layout) {
        unsafe { System.dealloc(p, l) }
    }
    unsafe fn realloc(&self, p: *mut u8, l: Layout, n: usize) -> *mut u8 {
        bump();
        unsafe { System.realloc(p, l, n) }
    }
}

#[global_allocator]
static A: Counting = Counting;

fn allocs_of(mut f: impl FnMut()) -> usize {
    f();
    let before = ALLOCS.with(|c| c.get());
    for _ in 0..20 {
        f();
    }
    ALLOCS.with(|c| c.get()) - before
}

/// The example alist "12.4.3.111" of D. MacKay, "alist format" (<http://www.inference.org.uk/mackay/codes/alist.html>,
/// page SHA-256 cf1b2a3f05e2554e5539f55cb6bec5e88242d28e23a273b4d010340b082dc520), copied as printed.
const MACKAY_12_16: &str = r#"12 16
4 3
4 4 4 4 4 4 4 4 4 4 4 4 
3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 
3 8 10 13 
4 7 9 13 
2 5 7 10 
4 6 11 14 
3 9 15 16 
1 6 9 10 
4 8 12 15 
2 6 12 16 
1 7 14 16 
3 5 12 14 
2 11 13 15 
1 5 8 11 
6 9 12 
3 8 11 
1 5 10 
2 4 7 
3 10 12 
4 6 8 
2 3 9 
1 7 12 
2 5 6 
1 3 6 
4 11 12 
7 8 10 
1 2 11 
4 9 10 
5 7 11 
5 8 9 
"#;

/// The same page prints this matrix "in ordinary 1/0 format": 12 lines of 16. Line `i` marks the rows of alist column `i`
/// (the page says the matrix is printed transposed).
const MACKAY_12_16_PRINTED: &str = r#" 0 0 1 0 0 0 0 1 0 1 0 0 1 0 0 0
 0 0 0 1 0 0 1 0 1 0 0 0 1 0 0 0
 0 1 0 0 1 0 1 0 0 1 0 0 0 0 0 0
 0 0 0 1 0 1 0 0 0 0 1 0 0 1 0 0
 0 0 1 0 0 0 0 0 1 0 0 0 0 0 1 1
 1 0 0 0 0 1 0 0 1 1 0 0 0 0 0 0
 0 0 0 1 0 0 0 1 0 0 0 1 0 0 1 0
 0 1 0 0 0 1 0 0 0 0 0 1 0 0 0 1
 1 0 0 0 0 0 1 0 0 0 0 0 0 1 0 1
 0 0 1 0 1 0 0 0 0 0 0 1 0 1 0 0
 0 1 0 0 0 0 0 0 0 0 1 0 1 0 1 0
 1 0 0 0 1 0 0 1 0 0 1 0 0 0 0 0
"#;

/// The code 96.3.963 (N = 96, K = 48, M = 48) of D. MacKay's Encyclopedia of Sparse Graph Codes
/// (<http://www.inference.org.uk/mackay/codes/data.html>, file <http://www.inference.org.uk/mackay/codes/EN/C/96.3.963>,
/// SHA-256 4efb2e58e5293e30d6efeaaa5ddbb8ff7b94611204f880f154871b14fd5d2efb5). The archive offers its codes freely on condition
/// that the authors and the archive are acknowledged: the code is by D. MacKay, from that archive.
const MACKAY_96_3_963: &str = r#"96 48
3 6
3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 3 
6 6 6 6 6 6 6 6 6 6 6 6 6 6 6 6 6 6 6 6 6 6 6 6 6 6 6 6 6 6 6 6 6 6 6 6 6 6 6 6 6 6 6 6 6 6 6 6 
10	30	40
5	32	45
16	18	39
12	22	38
15	19	47
2	17	34
9	24	42
1	29	33
4	27	36
3	26	35
11	31	43
7	21	44
8	20	48
14	23	46
6	28	37
13	25	41
14	32	43
5	23	37
2	31	36
1	28	34
7	25	47
10	21	33
15	30	35
16	26	48
3	22	46
12	20	41
8	18	38
4	19	45
6	24	40
9	27	39
13	17	42
11	29	44
8	24	34
6	25	36
9	19	43
1	20	46
14	27	42
7	22	39
13	18	35
4	26	40
16	29	38
15	21	48
11	23	45
3	17	47
5	28	44
12	32	33
2	30	41
10	31	37
10	18	36
4	23	44
9	29	40
2	27	38
8	30	42
12	28	43
11	20	37
1	19	35
15	31	39
16	32	41
5	26	33
3	25	45
13	21	34
14	24	48
7	17	46
6	22	47
7	27	40
11	18	33
2	32	35
10	28	47
5	24	41
12	25	37
3	19	39
14	31	44
16	30	34
13	20	38
9	22	36
6	17	45
4	21	42
15	29	46
8	26	43
1	23	48
1	25	42
15	22	40
8	21	41
9	18	47
6	27	43
11	30	46
7	31	35
5	20	36
14	17	38
16	28	45
4	32	37
13	23	33
12	26	44
3	29	48
2	24	39
10	19	34
8	20	36	56	80	81
6	19	47	52	67	95
10	25	44	60	71	94
9	28	40	50	77	91
2	18	45	59	69	88
15	29	34	64	76	85
12	21	38	63	65	87
13	27	33	53	79	83
7	30	35	51	75	84
1	22	48	49	68	96
11	32	43	55	66	86
4	26	46	54	70	93
16	31	39	61	74	92
14	17	37	62	72	89
5	23	42	57	78	82
3	24	41	58	73	90
6	31	44	63	76	89
3	27	39	49	66	84
5	28	35	56	71	96
13	26	36	55	74	88
12	22	42	61	77	83
4	25	38	64	75	82
14	18	43	50	80	92
7	29	33	62	69	95
16	21	34	60	70	81
10	24	40	59	79	93
9	30	37	52	65	85
15	20	45	54	68	90
8	32	41	51	78	94
1	23	47	53	73	86
11	19	48	57	72	87
2	17	46	58	67	91
8	22	46	59	66	92
6	20	33	61	73	96
10	23	39	56	67	87
9	19	34	49	75	88
15	18	48	55	70	91
4	27	41	52	74	89
3	30	38	57	71	95
1	29	40	51	65	82
16	26	47	58	69	83
7	31	37	53	77	81
11	17	35	54	79	85
12	32	45	50	72	93
2	28	43	60	76	90
14	25	36	63	78	86
5	21	44	64	68	84
13	24	42	62	80	94
"#;

fn checks_of(code: &LdpcCode) -> Vec<Vec<usize>> {
    let mut v = Vec::new();
    code.for_each_check(|b| {
        let mut b = b.to_vec();
        b.sort_unstable();
        v.push(b);
    });
    v
}

fn ft8() -> LdpcCode {
    LdpcCode::from_checks("ft8", 174, 91, &wsjt::FT8_CHECKS, InfoAt::Start).unwrap()
}

// ---------------------------------------------------------------------------------------------------------------------------
// A. alist

#[test]
fn alist_reads_mackays_printed_example() {
    let a = Alist::parse(MACKAY_12_16).unwrap();
    assert_eq!((a.n(), a.m()), (12, 16));
    let printed: Vec<Vec<u8>> = MACKAY_12_16_PRINTED
        .lines()
        .map(|l| l.split_whitespace().map(|t| t.parse().unwrap()).collect())
        .collect();
    assert_eq!(printed.len(), 12);
    for (c, line) in printed.iter().enumerate() {
        assert_eq!(line.len(), 16);
        for (r, &one) in line.iter().enumerate() {
            assert_eq!(
                a.rows()[r].contains(&(c as u32)),
                one == 1,
                "row {r} column {c}"
            );
        }
    }
    // The page: "The alist row '3 8 11' says the indices of the 1s in the 2nd column" (rows 3, 8, 11 of the printed lines).
    assert_eq!(a.rows()[1], [2, 7, 10]);
    assert_eq!(Alist::parse(&a.to_text()).unwrap(), a);
}

#[test]
fn alist_reads_the_encyclopedia_code_96_3_963() {
    let a = Alist::parse(MACKAY_96_3_963).unwrap();
    // The name and the archive page: N = 96, M = 48, column weight 3, row weight 6.
    assert_eq!((a.n(), a.m()), (96, 48));
    assert!(a.rows().iter().all(|r| r.len() == 6));
    let mut colw = vec![0; 96];
    for r in a.rows() {
        for &c in r {
            colw[c as usize] += 1;
        }
    }
    assert!(colw.iter().all(|&w| w == 3));
    // The first column list of the file is "10 30 40".
    let rows_of_0: Vec<usize> = (0..48).filter(|&r| a.rows()[r].contains(&0)).collect();
    assert_eq!(rows_of_0, [9, 29, 39]);
    // Written in MacKay's layout and read back, it is the same matrix.
    assert_eq!(Alist::parse(&a.to_text()).unwrap(), a);
    // The archive lists K = 48 (N - M), but two of the 48 checks depend on the others: the matrix has rank 46. No layout
    // of 48 information bits has a unique parity, so both encoders refuse.
    let code = a.to_code("96.3.963", 48, InfoAt::Start).unwrap();
    assert!(!code.can_encode());
    assert!(LdpcEncoder::new(&code).is_err());
    // The all-zero word still decodes through noise.
    let mut rng = Rng::new(3);
    let llr = bpsk_llr(&mut rng, &[0u8; 96], 0.5, 6.0);
    let mut out = [1u8; 96];
    BpDecoder::new(&code).decode(&llr, 50, &mut out).unwrap();
    assert_eq!(out, [0u8; 96]);
}

#[test]
fn alist_of_ft8_builds_the_same_code() {
    let code = ft8();
    let rows: Vec<Vec<u32>> = wsjt::FT8_CHECKS
        .iter()
        .map(|r| r.iter().map(|&b| b as u32).collect())
        .collect();
    let a = Alist::from_rows(174, rows).unwrap();
    assert_eq!(a, Alist::from_code(&code));
    let text = a.to_text();
    let b = Alist::parse(&text).unwrap();
    assert_eq!(b, a);
    let back = b.to_code("ft8", 91, InfoAt::Start).unwrap();
    assert_eq!(checks_of(&back), checks_of(&code));
    assert_eq!(back.parity_rows(), code.parity_rows());
    // Comments, tabs, CR LF and missing padding are accepted.
    let noisy = format!(
        "# FT8 (174,91)\r\n{}",
        text.replace(' ', "\t").replace('\n', "\r\n")
    );
    assert_eq!(Alist::parse(&noisy).unwrap(), a);
}

#[test]
fn alist_padding_is_optional() {
    // An irregular matrix: rows of weight 3 and 2.
    let a = Alist::from_rows(4, vec![vec![0, 1, 2], vec![2, 3]]).unwrap();
    let padded = a.to_text();
    assert_eq!(
        padded,
        "4 2\n2 3\n1 1 2 1\n3 2\n1 0\n1 0\n1 2\n2 0\n1 2 3\n3 4 0\n"
    );
    let bare = "4 2\n2 3\n1 1 2 1\n3 2\n1\n1\n1 2\n2\n1 2 3\n3 4\n";
    assert_eq!(Alist::parse(bare).unwrap(), a);
}

fn err(text: &str) -> AlistError {
    Alist::parse(text).unwrap_err()
}

#[test]
fn alist_refuses_malformed_text_with_its_position() {
    let good = "4 2\n2 3\n1 1 2 1\n3 2\n1 0\n1 0\n1 2\n2 0\n1 2 3\n3 4 0\n";
    assert!(Alist::parse(good).is_ok());
    let edit = |from: &str, to: &str| good.replacen(from, to, 1);
    let cases: Vec<(String, usize, usize)> = vec![
        ("".into(), 1, 1),
        ("4".into(), 2, 1),
        ("4 x".into(), 2, 1),
        ("4 -2".into(), 2, 1),
        ("4 2.0".into(), 2, 1),
        ("99999999999 2".into(), 1, 1),
        ("0 2".into(), 2, 1),
        ("4 0".into(), 2, 1),
        // Sizes larger than the text can hold.
        ("4000000 2\n1 3\n".into(), 1, 1),
        // Largest weights above M or N, a weight above the largest, different totals.
        (edit("4 2\n2 3", "4 2\n3 3"), 3, 2),
        (edit("4 2\n2 3", "4 2\n2 5"), 4, 2),
        (edit("4 2\n2 3", "4 2\n1 3"), 7, 3),
        (edit("4 2\n2 3", "4 2\n2 1"), 9, 4),
        (edit("3 2\n1 0", "3 3\n1 0"), 10, 4),
        // A row index above M, a column index above N, an index twice.
        (edit("3 2\n1 0", "3 2\n3 0"), 11, 5),
        (edit("1 2 3\n", "1 2 5\n"), 21, 9),
        (edit("1 2\n2 0", "1 1\n2 0"), 16, 7),
        // A row list that disagrees with the column lists.
        (edit("1 2 3\n", "1 2 4\n"), 19, 9),
        // The text ends in a list; numbers after the last list.
        (good[..good.len() - 9].to_string(), 21, 9),
        (format!("{good}0 7\n"), 26, 11),
        ("# only a comment\n".into(), 1, 1),
    ];
    for (text, token, line) in cases {
        let e = err(&text);
        assert_eq!((e.token, e.line), (token, line), "{text:?}: {e}");
        assert!(!e.to_string().is_empty());
        assert!(matches!(FecError::from(e), FecError::Alist(_)));
    }
    // Every cut of a valid file and every one-number change is refused or read, never a panic.
    let text = Alist::from_code(&ft8()).to_text();
    let toks: Vec<&str> = text.split_whitespace().collect();
    for cut in 0..toks.len() {
        // A cut that removes only padding zeros leaves the same matrix.
        let r = Alist::parse(&toks[..cut].join(" "));
        assert_eq!(
            r.is_err(),
            toks[cut..].iter().any(|&t| t != "0"),
            "cut {cut}"
        );
    }
    let mut rng = Rng::new(5);
    for _ in 0..3000 {
        let mut t: Vec<String> = toks.iter().map(|s| s.to_string()).collect();
        let i = rng.below(t.len());
        t[i] = match rng.below(5) {
            0 => "0".into(),
            1 => (rng.below(200)).to_string(),
            2 => u32::MAX.to_string(),
            3 => "1e3".into(),
            _ => String::new(),
        };
        let _ = Alist::parse(&t.join(" "));
    }
    // Building a code from a matrix that LdpcCode refuses is an error too.
    let a = Alist::from_rows(4, vec![vec![0, 1], vec![]]).unwrap();
    assert!(a.to_code("x", 2, InfoAt::Start).is_err());
    assert!(Alist::from_rows(4, vec![vec![0, 4]]).is_err());
    assert!(Alist::from_rows(4, vec![vec![1, 1]]).is_err());
    assert!(Alist::from_rows(0, vec![]).is_err());
}

// ---------------------------------------------------------------------------------------------------------------------------
// B. early stop

#[test]
fn early_stop_abandons_a_hopeless_word() {
    let code = ft8();
    let mut rng = Rng::new(21);
    let mut out = vec![0u8; 174];
    let mut plain = BpDecoder::new(&code);
    let mut early = BpDecoder::new(&code).with_early_stop(5);
    let mut stopped = 0;
    for _ in 0..20 {
        // Pure noise: no code word is near.
        let llr: Vec<f32> = (0..174).map(|_| 2.0 * rng.gauss() as f32).collect();
        if plain.decode(&llr, 200, &mut out).is_ok() {
            continue;
        }
        assert_eq!(plain.last_iterations(), 200);
        assert_eq!(
            early.decode(&llr, 200, &mut out),
            Err(FecError::Uncorrectable)
        );
        assert!(early.last_iterations() < 200);
        assert!(early.last_iterations() >= 5);
        stopped += 1;
    }
    assert!(stopped >= 15, "{stopped}");
    // Patience 0 turns it off again.
    early.set_early_stop(0);
    let llr: Vec<f32> = (0..174).map(|_| 2.0 * rng.gauss() as f32).collect();
    if early.decode(&llr, 60, &mut out).is_err() {
        assert_eq!(early.last_iterations(), 60);
    }
}

#[test]
fn early_stop_keeps_every_success() {
    let code = ft8();
    let mut rng = Rng::new(22);
    let mut plain = BpDecoder::new(&code);
    let mut early = BpDecoder::new(&code).with_early_stop(8);
    let (mut a, mut b) = (vec![0u8; 174], vec![0u8; 174]);
    let (mut ok_plain, mut ok_early) = (0, 0);
    let enc = LdpcEncoder::new(&code).unwrap();
    let mut cw = vec![0u8; 174];
    for _ in 0..200 {
        enc.encode(&rng.bits(91), &mut cw).unwrap();
        let llr = bpsk_llr(&mut rng, &cw, 91.0 / 174.0, 2.5);
        let rp = plain.decode(&llr, 50, &mut a);
        let re = early.decode(&llr, 50, &mut b);
        if let Ok(d) = re {
            // A success with early stop is the same decode as without it.
            assert_eq!(rp, Ok(d));
            assert_eq!(a, b);
            ok_early += 1;
        }
        ok_plain += rp.is_ok() as u32;
    }
    assert!(ok_plain >= 150, "{ok_plain}");
    assert!(ok_early + 3 >= ok_plain, "{ok_early} {ok_plain}");
}

#[test]
fn early_stop_decode_allocates_nothing() {
    let code = ft8();
    let mut rng = Rng::new(23);
    let mut dec = BpDecoder::new(&code).with_early_stop(4);
    let llr: Vec<f32> = (0..174).map(|_| 2.0 * rng.gauss() as f32).collect();
    let mut out = vec![0u8; 174];
    assert_eq!(
        allocs_of(|| {
            let _ = dec.decode(&llr, 100, &mut out);
        }),
        0
    );
}

// ---------------------------------------------------------------------------------------------------------------------------
// C. the sparse encoder

#[test]
fn sparse_encoder_equals_the_existing_encoders() {
    let mut rng = Rng::new(31);
    // A code word is unique for each information word when the parity part has full rank, so both encoders must agree bit for bit.
    let mut codes = vec![
        ft8(),
        LdpcCode::from_checks("js8", 174, 87, &wsjt::JS8_CHECKS, InfoAt::End).unwrap(),
        LdpcCode::from_dvb(&dvb_s2::S2_S_1_2).unwrap(),
        LdpcCode::from_dvb(&dvb_s2::S2_N_1_2).unwrap(),
        LdpcCode::from_dvb(&dvb_s2::S2_S_8_9).unwrap(),
    ];
    for (bg, z) in [(1u8, 2usize), (1, 15), (1, 64), (2, 3), (2, 36), (2, 52)] {
        codes.push(LdpcCode::nr(bg, z).unwrap());
    }
    for (i, code) in codes.iter().enumerate() {
        assert!(code.can_encode());
        let enc = LdpcEncoder::new(code).unwrap();
        assert_eq!((enc.n(), enc.k()), (code.n(), code.k()));
        for _ in 0..3 {
            let info = rng.bits(code.k());
            let (mut a, mut b) = (vec![0u8; code.n()], vec![0u8; code.n()]);
            code.encode(&info, &mut a).unwrap();
            enc.encode(&info, &mut b).unwrap();
            assert_eq!(a, b, "{}", code.name());
        }
        if (2..5).contains(&i) {
            // The DVB accumulator is dual-diagonal: back-substitution alone.
            assert_eq!(enc.gap(), 0, "{}", code.name());
        }
    }
}

#[test]
fn sparse_encoder_encodes_nr_at_the_largest_lifting() {
    let mut rng = Rng::new(32);
    for bg in [1u8, 2] {
        let code = LdpcCode::nr(bg, 384).unwrap();
        assert!(!code.can_encode());
        let enc = LdpcEncoder::new(&code).unwrap();
        assert!(enc.gap() <= 4 * 384, "gap {}", enc.gap());
        for _ in 0..4 {
            let info = rng.bits(code.k());
            let mut cw = vec![0u8; code.n()];
            enc.encode(&info, &mut cw).unwrap();
            assert!(code.is_codeword(&cw), "bg {bg}");
            assert_eq!(&cw[..code.k()], &info[..]);
        }
        // A single wrong bit is never a code word.
        let mut cw = vec![0u8; code.n()];
        enc.encode(&rng.bits(code.k()), &mut cw).unwrap();
        cw[code.n() - 1] ^= 1;
        assert!(!code.is_codeword(&cw));
    }
}

#[test]
fn sparse_encoder_refuses_bad_codes_and_lengths() {
    // Parity bit 3 is in no check: the parity part is singular.
    let c = LdpcCode::from_checks("s", 4, 2, &[&[0u32, 2][..], &[1, 2]], InfoAt::Start).unwrap();
    assert!(LdpcEncoder::new(&c).is_err());
    // Three independent checks on two parity bits: rank above n - k.
    let c =
        LdpcCode::from_checks("r", 4, 2, &[&[0u32, 2][..], &[1, 3], &[0]], InfoAt::Start).unwrap();
    assert!(LdpcEncoder::new(&c).is_err());
    // A repeated check is harmless.
    let c = LdpcCode::from_checks(
        "d",
        4,
        2,
        &[&[0u32, 2][..], &[1, 3], &[0, 2]],
        InfoAt::Start,
    )
    .unwrap();
    let enc = LdpcEncoder::new(&c).unwrap();
    let mut cw = [0u8; 4];
    enc.encode(&[1, 0], &mut cw).unwrap();
    assert_eq!(cw, [1, 0, 1, 0]);
    assert!(enc.encode(&[1], &mut cw).is_err());
    assert!(enc.encode(&[1, 0], &mut [0u8; 3]).is_err());
}

#[test]
fn sparse_encode_allocates_nothing() {
    let code = LdpcCode::nr(1, 64).unwrap();
    let enc = LdpcEncoder::new(&code).unwrap();
    assert!(enc.gap() > 0);
    let info = Rng::new(33).bits(code.k());
    let mut cw = vec![0u8; code.n()];
    assert_eq!(allocs_of(|| enc.encode(&info, &mut cw).unwrap()), 0);
}

// ---------------------------------------------------------------------------------------------------------------------------
// D. masked lanes

/// A small quasi-cyclic code with z = 3 and two holes, and the same code written out check by check from the definition: lane `l`
/// of a block row checks bit `(l + shift) mod z` of each column group, except the masked lane of an entry.
fn small_masked() -> (LdpcCode, LdpcCode) {
    let rows: [&[(u16, u16, Option<u16>)]; 2] = [
        &[(0, 1, None), (1, 0, None), (2, 2, Some(0))],
        &[(0, 2, Some(2)), (2, 0, None), (1, 2, None)],
    ];
    let qc = LdpcCode::from_qc_masked("masked", 3, 3, 3, &rows).unwrap();
    // Row 0: lane 0 {1, 3} (no column group 2), lane 1 {2, 4, 6}, lane 2 {0, 5, 7}.
    // Row 1: lane 0 {2, 6, 5}, lane 1 {0, 7, 3}, lane 2 {8, 4} (no column group 0).
    let checks: [&[u32]; 6] = [
        &[1, 3],
        &[2, 4, 6],
        &[0, 5, 7],
        &[2, 5, 6],
        &[0, 3, 7],
        &[4, 8],
    ];
    let plain = LdpcCode::from_checks("plain", 9, 3, &checks, InfoAt::Start).unwrap();
    (qc, plain)
}

#[test]
fn masked_lanes_equal_the_written_out_checks() {
    let (qc, plain) = small_masked();
    assert_eq!(checks_of(&qc), checks_of(&plain));
    assert_eq!(qc.edges(), 16);
    assert_eq!(qc.edges(), plain.edges());
    for w in 0..512u32 {
        let word: Vec<u8> = (0..9).map(|i| (w >> i) as u8 & 1).collect();
        assert_eq!(qc.is_codeword(&word), plain.is_codeword(&word), "{w:09b}");
    }
    // The decoders see the same graph. The checks of one block row touch disjoint bits, so the layered order of min-sum
    // produces the same messages as one check at a time.
    let mut rng = Rng::new(41);
    let (mut ms_a, mut ms_b) = (
        LdpcDecoder::new(&qc, MinSum::default()),
        LdpcDecoder::new(&plain, MinSum::default()),
    );
    let (mut bp_a, mut bp_b) = (BpDecoder::new(&qc), BpDecoder::new(&plain));
    let (mut a, mut b) = (vec![0u8; 9], vec![0u8; 9]);
    for _ in 0..200 {
        let llr8: Vec<i8> = (0..9).map(|_| (rng.below(61) as i8) - 30).collect();
        assert_eq!(
            ms_a.decode(&llr8, 10, &mut a),
            ms_b.decode(&llr8, 10, &mut b)
        );
        assert_eq!(a, b);
        let llr: Vec<f32> = llr8.iter().map(|&v| v as f32 / 4.0).collect();
        assert_eq!(bp_a.decode(&llr, 10, &mut a), bp_b.decode(&llr, 10, &mut b));
        assert_eq!(a, b);
    }
}

#[test]
fn masked_lanes_are_validated() {
    let bad: [&[(u16, u16, Option<u16>)]; 1] = [&[(0, 0, Some(3)), (1, 0, None)]];
    assert!(LdpcCode::from_qc_masked("x", 3, 2, 3, &bad).is_err());
    let max: [&[(u16, u16, Option<u16>)]; 1] = [&[(0, 0, Some(u16::MAX)), (1, 0, None)]];
    assert!(LdpcCode::from_qc_masked("x", 3, 2, 3, &max).is_err());
    // Without masks the new constructor builds the same code as from_qc.
    let rows: [&[(u16, u16)]; 2] = [&[(0, 1), (1, 0)], &[(0, 2), (1, 1), (2, 0)]];
    let masked: Vec<Vec<(u16, u16, Option<u16>)>> = rows
        .iter()
        .map(|r| r.iter().map(|&(c, s)| (c, s, None)).collect())
        .collect();
    let mrefs: Vec<&[(u16, u16, Option<u16>)]> = masked.iter().map(Vec::as_slice).collect();
    let a = LdpcCode::from_qc("a", 4, 3, 4, &rows).unwrap();
    let b = LdpcCode::from_qc_masked("a", 4, 3, 4, &mrefs).unwrap();
    assert_eq!(checks_of(&a), checks_of(&b));
    assert_eq!(a.parity_rows(), b.parity_rows());
}

/// A code of the CDR size (9216 bits, z = 256, 36 column groups, rate 1/2): random information circulants and a dual-diagonal
/// parity part closed by an entry whose lane 0 is masked, as in DVB. Too large for the dense encoder.
#[test]
fn masked_cdr_sized_code_encodes_and_decodes() {
    let (z, cols, kb) = (256usize, 36usize, 18usize);
    let mut rng = Rng::new(42);
    let mut rows: Vec<Vec<(u16, u16, Option<u16>)>> = Vec::new();
    for i in 0..cols - kb {
        let mut r = Vec::new();
        let mut used = Vec::new();
        while r.len() < 5 {
            let c = rng.below(kb);
            if !used.contains(&c) {
                used.push(c);
                r.push((c as u16, rng.below(z) as u16, None));
            }
        }
        r.push(((kb + i) as u16, 0, None));
        if i > 0 {
            r.push(((kb + i - 1) as u16, 0, None));
        } else {
            r.push(((cols - 1) as u16, (z - 1) as u16, Some(0)));
        }
        rows.push(r);
    }
    let refs: Vec<&[(u16, u16, Option<u16>)]> = rows.iter().map(Vec::as_slice).collect();
    let code = LdpcCode::from_qc_masked("cdr-sized", z, cols, kb * z, &refs).unwrap();
    assert_eq!(code.n(), 9216);
    assert!(!code.can_encode());
    let enc = LdpcEncoder::new(&code).unwrap();
    assert_eq!(enc.gap(), 0);
    let info = rng.bits(code.k());
    let mut cw = vec![0u8; code.n()];
    enc.encode(&info, &mut cw).unwrap();
    assert!(code.is_codeword(&cw));
    // The hole removes exactly one edge.
    let mut open = rows.clone();
    open[0].last_mut().unwrap().2 = None;
    let orefs: Vec<&[(u16, u16, Option<u16>)]> = open.iter().map(Vec::as_slice).collect();
    let unmasked = LdpcCode::from_qc_masked("open", z, cols, kb * z, &orefs).unwrap();
    assert_eq!(unmasked.edges(), code.edges() + 1);
    let llr = bpsk_llr8(&mut rng, &cw, 0.5, 3.0, 16.0);
    let mut dec = LdpcDecoder::new(&code, MinSum::default());
    let mut out = vec![0u8; code.n()];
    dec.decode(&llr, 40, &mut out).unwrap();
    assert_eq!(out, cw);
}
