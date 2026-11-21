//! LDPC codes of terrestrial television: the ATSC 3.0 codes of A/322 (sizes, checks, encoding, decoding in noise) and the
//! DTMB mechanism of GB 20600 with a synthetic code of the same shape. A counting allocator (per thread, so the tests can run
//! in parallel) makes sure that decoding allocates nothing; it needs `unsafe impl GlobalAlloc`.
#![allow(unsafe_code)]

mod common;
use common::*;
use sdrkit_algo_fec::ldpc::{InfoAt, LdpcCode, LdpcDecoder, LdpcEncoder, MinSum};
use sdrkit_algo_fec::FecError;
use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;

struct Counting;

thread_local! {
    static ALLOCS: Cell<usize> = const { Cell::new(0) };
}

unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, l: Layout) -> *mut u8 {
        let _ = ALLOCS.try_with(|c| c.set(c.get() + 1));
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

/// Decoding a type A code, a type B code and the DTMB-shaped code allocates nothing once the decoder exists.
#[test]
fn decoding_allocates_nothing() {
    let rows = synthetic_dtmb_rows();
    let refs: Vec<&[(u16, u16)]> = rows.iter().map(|r| r.as_slice()).collect();
    let codes = [
        LdpcCode::atsc3(16200, 3).unwrap(),
        LdpcCode::atsc3(64800, 8).unwrap(),
        LdpcCode::dtmb(&refs, InfoAt::End).unwrap(),
    ];
    let mut rng = Rng::new(50);
    for code in &codes {
        let mut cw = vec![0u8; code.n()];
        let info = rng.bits(code.k());
        LdpcEncoder::new(code)
            .unwrap()
            .encode(&info, &mut cw)
            .unwrap();
        let llr = bpsk_llr8(&mut rng, &cw, code.k() as f64 / code.n() as f64, 4.0, 16.0);
        let mut dec = LdpcDecoder::new(code, MinSum::default());
        let mut out = vec![0u8; code.n()];
        dec.decode(&llr, 50, &mut out).unwrap();
        let before = ALLOCS.with(|c| c.get());
        for _ in 0..20 {
            dec.decode(&llr, 50, &mut out).unwrap();
        }
        assert_eq!(ALLOCS.with(|c| c.get()), before, "{}", code.name());
        assert_eq!(out, cw);
    }
}

const RATES: std::ops::RangeInclusive<u8> = 2..=13;

/// The address tables of A/322:2026-06 Annex A, tables A.1.1 to A.1.12 and A.2.1 to A.2.12. Each entry was compared number
/// by number and row by row with the text of the published document. This test keeps the result: for each code length and
/// rate, the number of rows, the number of addresses and the sum of the addresses.
#[test]
fn atsc3_tables_keep_the_a322_annex_a_contents() {
    use sdrkit_algo_fec::ldpc::tables::atsc3::{Atsc3Table, ALL};
    let want: [(u32, u8, usize, usize, u64); 24] = [
        (64800, 2, 29, 546, 14347754),
        (64800, 3, 41, 600, 12907882),
        (64800, 4, 53, 639, 12196238),
        (64800, 5, 64, 692, 11480218),
        (64800, 6, 72, 566, 10963632),
        (64800, 7, 87, 765, 9160463),
        (64800, 8, 96, 624, 9431080),
        (64800, 9, 108, 647, 10108992),
        (64800, 10, 120, 660, 7013928),
        (64800, 11, 132, 679, 7502776),
        (64800, 12, 144, 718, 4738718),
        (64800, 13, 156, 729, 3789492),
        (16200, 2, 15, 78, 551964),
        (16200, 3, 12, 129, 793565),
        (16200, 4, 15, 131, 644721),
        (16200, 5, 17, 159, 721629),
        (16200, 6, 18, 145, 668166),
        (16200, 7, 21, 154, 689360),
        (16200, 8, 24, 167, 775371),
        (16200, 9, 27, 152, 485144),
        (16200, 10, 30, 179, 554492),
        (16200, 11, 33, 162, 359602),
        (16200, 12, 36, 176, 321084),
        (16200, 13, 39, 187, 189068),
    ];
    assert_eq!(ALL.len(), want.len());
    for (&(n, r, t), &(wn, wr, rows, count, sum)) in ALL.iter().zip(want.iter()) {
        assert_eq!((n, r), (wn, wr));
        let table: &[&[u16]] = match t {
            Atsc3Table::A(a) => a.rows,
            Atsc3Table::B(b) => b.rows,
        };
        let got = (
            table.len(),
            table.iter().map(|row| row.len()).sum::<usize>(),
            table
                .iter()
                .flat_map(|row| row.iter())
                .map(|&a| a as u64)
                .sum::<u64>(),
        );
        assert_eq!(got, (rows, count, sum), "{n} {r}/15");
        // Within a row the addresses rise, as printed.
        assert!(
            table.iter().all(|row| row.windows(2).all(|w| w[0] < w[1])),
            "{n} {r}/15"
        );
    }
}

/// A/322 clause 6.1.3: `K = N r / 15` and `N − K` parity checks for every length and rate. Table 6.4: type A codes (rates
/// 2/15 to 5/15, and 7/15 at 64800) have no built-in `encode`; type B codes have the DVB-S2 encoder.
#[test]
fn atsc3_sizes_and_types_follow_a322() {
    for n in [16200usize, 64800] {
        for r in RATES {
            let code = LdpcCode::atsc3(n, r).unwrap();
            assert_eq!((code.n(), code.k()), (n, n * r as usize / 15), "{n} {r}");
            assert_eq!(code.checks(), n - code.k());
            assert_eq!(code.z(), 360);
            let type_a = (2..=5).contains(&r) || (n == 64800 && r == 7);
            assert_eq!(code.can_encode(), !type_a, "{n} {r}");
        }
    }
    for (n, r) in [(16200, 1), (16200, 14), (32400, 8), (64800, 0)] {
        assert!(matches!(
            LdpcCode::atsc3(n, r),
            Err(FecError::Conflict { .. })
        ));
    }
}

/// The general encoder and the DVB-S2 accumulator give the same words for type B codes; every word passes every check.
#[test]
fn atsc3_encoders_agree_and_satisfy_every_check() {
    let mut rng = Rng::new(31);
    for n in [16200usize, 64800] {
        for r in RATES {
            let code = LdpcCode::atsc3(n, r).unwrap();
            let info = rng.bits(code.k());
            let mut cw = vec![0u8; n];
            LdpcEncoder::new(&code)
                .unwrap()
                .encode(&info, &mut cw)
                .unwrap();
            assert!(code.is_codeword(&cw), "{}", code.name());
            assert_eq!(&cw[code.info_range()], &info[..]);
            if code.can_encode() {
                let mut cw2 = vec![0u8; n];
                code.encode(&info, &mut cw2).unwrap();
                assert_eq!(cw2, cw, "{}", code.name());
            }
        }
    }
}

/// BPSK in white Gaussian noise at Eb/N0 = 4 dB, well above the threshold of every rate: each code decodes the whole word.
#[test]
fn atsc3_decodes_every_rate_in_noise() {
    let mut rng = Rng::new(32);
    for n in [16200usize, 64800] {
        for r in RATES {
            let code = LdpcCode::atsc3(n, r).unwrap();
            let info = rng.bits(code.k());
            let mut cw = vec![0u8; n];
            LdpcEncoder::new(&code)
                .unwrap()
                .encode(&info, &mut cw)
                .unwrap();
            let llr = bpsk_llr8(&mut rng, &cw, r as f64 / 15.0, 4.0, 16.0);
            let raw = llr
                .iter()
                .zip(&cw)
                .filter(|(&l, &b)| (l < 0) as u8 != b)
                .count();
            let mut out = vec![0u8; n];
            let d = LdpcDecoder::new(&code, MinSum::default())
                .decode(&llr, 50, &mut out)
                .unwrap_or_else(|e| panic!("{}: {e:?}", code.name()));
            assert_eq!(out, cw, "{}", code.name());
            assert!(raw > 0 && d.iterations >= 1, "{}", code.name());
        }
    }
}

/// A synthetic code of the DTMB shape (59 groups of 127, rate 0.8, information last): a dual-diagonal parity part and three
/// circulants in each information column group.
fn synthetic_dtmb_rows() -> Vec<Vec<(u16, u16)>> {
    let mut rng = Rng::new(40);
    let mut rows: Vec<Vec<(u16, u16)>> = (0..11)
        .map(|r| {
            let mut v = vec![(r as u16, 0u16)];
            if r > 0 {
                v.push((r as u16 - 1, 0));
            }
            v
        })
        .collect();
    for c in 11..59u16 {
        let mut used = Vec::new();
        while used.len() < 3 {
            let r = rng.below(11);
            if !used.contains(&r) {
                used.push(r);
                rows[r].push((c, rng.below(127) as u16));
            }
        }
    }
    rows
}

#[test]
fn dtmb_mechanism_decodes_a_synthetic_code_with_the_deleted_bits() {
    let rows = synthetic_dtmb_rows();
    let refs: Vec<&[(u16, u16)]> = rows.iter().map(|r| r.as_slice()).collect();
    let code = LdpcCode::dtmb(&refs, InfoAt::End).unwrap();
    assert_eq!((code.n(), code.k(), code.z()), (7493, 6096, 127));
    assert_eq!(code.checks(), 11 * 127);
    assert_eq!(code.edges(), (11 + 10 + 48 * 3) * 127);
    assert_eq!(code.info_range(), 1397..7493);
    assert!(!code.can_encode());
    let enc = LdpcEncoder::new(&code).unwrap();
    let mut rng = Rng::new(41);
    let mut dec = LdpcDecoder::new(&code, MinSum::default());
    for _ in 0..4 {
        let info = rng.bits(code.k());
        let mut cw = vec![0u8; code.n()];
        enc.encode(&info, &mut cw).unwrap();
        assert!(code.is_codeword(&cw));
        // The transmitter sends 7488 bits; the receiver puts LLR 0 in front of them.
        let d = LdpcCode::DTMB_DELETED;
        let sent = &cw[d..];
        assert_eq!(sent.len(), 7488);
        let rx = bpsk_llr8(&mut rng, sent, 0.8, 4.5, 16.0);
        let mut llr = vec![0i8; d];
        llr.extend_from_slice(&rx);
        let mut out = vec![0u8; code.n()];
        dec.decode(&llr, 50, &mut out).unwrap();
        assert_eq!(out, cw);
    }
    // Noise alone: the decoder must report failure, not a word.
    let llr: Vec<i8> = (0..code.n()).map(|_| (rng.below(41) as i8) - 20).collect();
    let mut out = vec![0u8; code.n()];
    assert!(matches!(
        dec.decode(&llr, 50, &mut out),
        Err(FecError::Uncorrectable)
    ));
}

#[test]
fn dtmb_rates_and_bad_tables() {
    for (rows, k, name) in [
        (35, 3048, "dtmb 0.4"),
        (23, 4572, "dtmb 0.6"),
        (11, 6096, "dtmb 0.8"),
    ] {
        let t: Vec<Vec<(u16, u16)>> = (0..rows)
            .map(|r| vec![(r as u16, 0), (58, r as u16)])
            .collect();
        let refs: Vec<&[(u16, u16)]> = t.iter().map(|r| r.as_slice()).collect();
        let code = LdpcCode::dtmb(&refs, InfoAt::Start).unwrap();
        assert_eq!((code.k(), code.name()), (k, name));
    }
    let ok: Vec<(u16, u16)> = vec![(0, 0)];
    let mut t: Vec<&[(u16, u16)]> = vec![&ok; 11];
    assert!(LdpcCode::dtmb(&t[..10], InfoAt::End).is_err());
    let bad_col = [(59u16, 0u16)];
    t[3] = &bad_col;
    assert!(LdpcCode::dtmb(&t, InfoAt::End).is_err());
    let bad_shift = [(1u16, 127u16)];
    t[3] = &bad_shift;
    assert!(LdpcCode::dtmb(&t, InfoAt::End).is_err());
    let twice = [(1u16, 5u16), (1, 5)];
    t[3] = &twice;
    assert!(LdpcCode::dtmb(&t, InfoAt::End).is_err());
    let empty: [(u16, u16); 0] = [];
    t[3] = &empty;
    assert!(LdpcCode::dtmb(&t, InfoAt::End).is_err());
    // A weight-2 circulant is two pairs with one column group.
    let w2 = [(1u16, 5u16), (1, 9)];
    t[3] = &w2;
    assert!(LdpcCode::dtmb(&t, InfoAt::End).is_ok());
}
