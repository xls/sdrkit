//! The accumulator codes of the FreeDV OFDM voice waveforms: the tables, the encoder against the worked code words of the
//! specification, and sum-product decoding without noise and at a stated Eb/N0.

mod common;
use common::*;
use sdrkit_algo_fec::ldpc::tables::freedv::{FREEDV_700D_224_112, FREEDV_700E_112_56};
use sdrkit_algo_fec::ldpc::{BpDecoder, LdpcCode};

fn code(rows: &[[u8; 3]]) -> LdpcCode {
    let refs: Vec<&[u8]> = rows.iter().map(|r| r.as_slice()).collect();
    LdpcCode::from_accumulator("freedv", rows.len(), &refs).unwrap()
}

/// The test-frame data bits of the specification: `x0 = 1`, `x(i+1) = (1103515245 x(i) + 12345) mod 32768`, bit `i` is one
/// when `x(i+1) > 16384`.
fn test_frame_bits(k: usize) -> Vec<u8> {
    let mut x = 1u64;
    (0..k)
        .map(|_| {
            x = (1_103_515_245 * x + 12_345) % 32_768;
            (x > 16_384) as u8
        })
        .collect()
}

const DATA_700D: &str = "1010110001010111100110001110100010011100100000010110001010101110101101100110101111011110101000101110101000101001";
const PARITY_700D: &str = "0010111110011000100100011101011001100101011111010110100001110100011001101110001001110110101101010111000011001101";
const DATA_700E: &str = "10101100010101111001100011101000100111001000000101100010";
const PARITY_700E: &str = "00011000110000011001001001010111110001011011100001101101";

#[test]
fn every_row_and_every_column_of_the_tables_has_three_ones() {
    for rows in [&FREEDV_700D_224_112[..], &FREEDV_700E_112_56[..]] {
        let k = rows.len();
        let mut weight = vec![0u32; k];
        for r in rows {
            assert!(r[0] < r[1] && r[1] < r[2], "{r:?}");
            for &b in r {
                weight[b as usize] += 1;
            }
        }
        assert!(weight.iter().all(|&w| w == 3), "k = {k}");
    }
}

#[test]
fn the_encoder_gives_the_worked_code_words_of_the_specification() {
    for (rows, data, parity) in [
        (&FREEDV_700D_224_112[..], DATA_700D, PARITY_700D),
        (&FREEDV_700E_112_56[..], DATA_700E, PARITY_700E),
    ] {
        let c = code(rows);
        let k = rows.len();
        assert_eq!((c.n(), c.k(), c.checks()), (2 * k, k, k));
        let d = test_frame_bits(k);
        assert_eq!(d, bits_of(data));
        let mut cw = vec![0u8; 2 * k];
        c.encode(&d, &mut cw).unwrap();
        assert_eq!(cw[..k], d[..]);
        assert_eq!(cw[k..], bits_of(parity)[..], "k = {k}");
        assert!(c.is_codeword(&cw));
        // The accumulator rule itself, bit by bit.
        let mut p = 0u8;
        for (i, r) in rows.iter().enumerate() {
            p ^= r.iter().fold(0, |s, &b| s ^ d[b as usize]);
            assert_eq!(cw[k + i], p, "parity bit {i}");
        }
    }
}

#[test]
fn five_weak_wrong_signs_in_the_worked_code_words_are_repaired() {
    for (rows, wrong) in [
        (&FREEDV_700D_224_112[..], [0usize, 50, 100, 150, 200]),
        (&FREEDV_700E_112_56[..], [0, 25, 50, 75, 100]),
    ] {
        let c = code(rows);
        let k = rows.len();
        let d = test_frame_bits(k);
        let mut cw = vec![0u8; 2 * k];
        c.encode(&d, &mut cw).unwrap();
        let mut llr: Vec<f32> = cw.iter().map(|&b| if b == 0 { 4.0 } else { -4.0 }).collect();
        for i in wrong {
            llr[i] = if cw[i] == 0 { -1.0 } else { 1.0 };
        }
        let mut dec = BpDecoder::new(&c);
        let mut out = vec![0u8; 2 * k];
        let r = dec.decode(&llr, 100, &mut out).unwrap();
        assert_eq!(out, cw);
        assert_eq!(r.corrected, 5);
    }
}

#[test]
fn every_word_decodes_without_noise() {
    for rows in [&FREEDV_700D_224_112[..], &FREEDV_700E_112_56[..]] {
        let c = code(rows);
        let k = rows.len();
        let mut dec = BpDecoder::new(&c);
        let mut rng = Rng::new(11);
        for _ in 0..200 {
            let d = rng.bits(k);
            let mut cw = vec![0u8; 2 * k];
            c.encode(&d, &mut cw).unwrap();
            let llr: Vec<f32> = cw.iter().map(|&b| if b == 0 { 4.0 } else { -4.0 }).collect();
            let mut out = vec![0u8; 2 * k];
            let r = dec.decode(&llr, 100, &mut out).unwrap();
            assert_eq!((out, r.iterations), (cw, 0));
        }
    }
}

/// The words of 500 that decode to the sent word at `ebn0_db` (BPSK, white noise, at most 100 iterations), one word per seed.
fn good_of_500(rows: &[[u8; 3]], ebn0_db: f64) -> usize {
    let c = code(rows);
    let k = rows.len();
    let mut dec = BpDecoder::new(&c);
    (0..500u64)
        .filter(|&seed| {
            let mut rng = Rng::new(seed);
            let d = rng.bits(k);
            let mut cw = vec![0u8; 2 * k];
            c.encode(&d, &mut cw).unwrap();
            let llr = bpsk_llr(&mut rng, &cw, 0.5, ebn0_db);
            let mut out = vec![0u8; 2 * k];
            dec.decode(&llr, 100, &mut out).is_ok() && out == cw
        })
        .count()
}

/// Measured over the seeds 0 to 499: at Eb/N0 = 3 dB the (224,112) code decodes 490 words (98.0 %) and the (112,56) code 485
/// (97.0 %); at 5 dB both decode all 500.
#[test]
fn frame_success_at_3_db_and_5_db_eb_n0() {
    assert!(good_of_500(&FREEDV_700D_224_112, 3.0) >= 480);
    assert!(good_of_500(&FREEDV_700E_112_56, 3.0) >= 475);
    assert_eq!(good_of_500(&FREEDV_700D_224_112, 5.0), 500);
    assert_eq!(good_of_500(&FREEDV_700E_112_56, 5.0), 500);
}

#[test]
fn bad_tables_are_refused() {
    let rows: [&[u8]; 2] = [&[0, 1], &[1, 2]];
    assert!(LdpcCode::from_accumulator("bad", 2, &rows).is_err());
    assert!(LdpcCode::from_accumulator("bad", 3, &rows).is_err());
}
