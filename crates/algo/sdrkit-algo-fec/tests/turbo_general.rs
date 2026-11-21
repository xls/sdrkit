//! The general turbo decoder in its 3GPP2 (EN 302 583) and Family SL (TS 102 744-2-1) configurations, and the block
//! interleavers.

mod common;
use common::*;
use sdrkit_algo_fec::blockinterleave::{ConvBlocks, KeptPermutation};
use sdrkit_algo_fec::turbo::{TurboDecoder, RSC_SL};
use sdrkit_algo_fec::turbo_codes::{
    sh_code, sh_coded_len, sh_interleaver, sh_map, sl_code, sl_map, SH_PATTERNS,
};

#[test]
fn sh_interleaver_addresses() {
    let a = sh_interleaver(1146).unwrap();
    assert_eq!(
        &a[..16],
        &[3, 1051, 527, 285, 769, 131, 655, 401, 935, 83, 1115, 591, 365, 865, 205, 719]
    );
    assert_eq!(&a[1142..], &[192, 704, 448, 960]);
    let b = sh_interleaver(12_282).unwrap();
    assert_eq!(
        &b[..16],
        &[
            13, 8527, 4183, 2063, 10241, 6477, 1037, 9217, 5241, 3073, 11439, 7589, 1021, 8919,
            4655, 2855
        ]
    );
    assert_eq!(&b[12_278..], &[5632, 3584, 11776, 7680]);
    for v in [a, b] {
        let mut seen = vec![false; v.len()];
        for &x in &v {
            assert!(!seen[x as usize]);
            seen[x as usize] = true;
        }
    }
    assert!(sh_interleaver(1000).is_err());
}

#[test]
fn sh_coded_lengths() {
    let want = [
        61_440, 55_296, 49_152, 43_008, 36_864, 36_864, 30_720, 30_720, 24_576, 24_576, 18_432,
        18_432,
    ];
    let tails = [30, 27, 24, 21, 18, 18, 16, 16, 12, 12, 10, 8];
    for (p, (&n, &t)) in SH_PATTERNS.iter().zip(want.iter().zip(&tails)) {
        assert_eq!(sh_coded_len(12_282, p), n, "pattern {}", p.id);
        assert_eq!(sh_map(12_282, p).len(), n);
        let tail: usize = p.tail.iter().flatten().map(|&x| x as usize).sum();
        assert_eq!(tail, t, "tail of pattern {}", p.id);
    }
    assert_eq!(sh_coded_len(1146, &SH_PATTERNS[0]), 5760);
}

/// Eb/N0 (dB) at which the BPSK-input AWGN capacity equals `rate`.
fn bpsk_limit(rate: f64) -> f64 {
    let cap = |esn0: f64| {
        let s2 = 1.0 / (2.0 * esn0);
        let s = s2.sqrt();
        let (mut acc, mut wsum) = (0.0, 0.0);
        for i in -4000..=4000 {
            let z = i as f64 / 500.0;
            let w = (-z * z / 2.0).exp();
            let y = 1.0 + s * z;
            acc += w * (1.0 + (-2.0 * y / s2).exp()).log2();
            wsum += w;
        }
        1.0 - acc / wsum
    };
    let (mut lo, mut hi) = (-20.0f64, 10.0f64);
    for _ in 0..60 {
        let mid = (lo + hi) / 2.0;
        if cap(10f64.powf(mid / 10.0)) < rate {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    lo - 10.0 * rate.log10()
}

/// Every pattern of EN 302 583 decodes K = 12 282 without errors 1.5 dB above the BPSK limit of its rate.
#[test]
fn sh_patterns_decode_above_the_bpsk_limit() {
    let k = 12_282;
    let code = sh_code(k).unwrap();
    let mut dec = TurboDecoder::new(code.clone());
    let mut rng = Rng::new(7);
    let mut raw = vec![0u8; code.streams() * code.periods()];
    let mut out = vec![0u8; k];
    for p in &SH_PATTERNS {
        let map = sh_map(k, p);
        let rate = k as f64 / map.len() as f64;
        let ebn0 = bpsk_limit(rate) + 1.5;
        for _ in 0..2 {
            let bits = rng.bits(k);
            code.encode(&bits, &mut raw).unwrap();
            let mut tx = vec![0u8; map.len()];
            code.transmit(&map, &raw, &mut tx).unwrap();
            let llr = bpsk_llr8(&mut rng, &tx, rate, ebn0, 12.0);
            dec.load(&map, &llr).unwrap();
            let r = dec.decode(8, |b| b == &bits[..], &mut out);
            assert!(r.is_ok(), "pattern {} at {ebn0:.2} dB", p.id);
        }
    }
    // Below the limit the same decoder fails.
    let p = &SH_PATTERNS[10];
    let map = sh_map(k, p);
    let rate = k as f64 / map.len() as f64;
    let bits = rng.bits(k);
    code.encode(&bits, &mut raw).unwrap();
    let mut tx = vec![0u8; map.len()];
    code.transmit(&map, &raw, &mut tx).unwrap();
    let llr = bpsk_llr8(&mut rng, &tx, rate, bpsk_limit(rate) - 0.5, 12.0);
    dec.load(&map, &llr).unwrap();
    assert!(dec.decode(8, |b| b == &bits[..], &mut out).is_err());
}

#[test]
fn sh_signalling_block_round_trip() {
    let code = sh_code(1146).unwrap();
    let map = sh_map(1146, &SH_PATTERNS[0]);
    let mut rng = Rng::new(3);
    let bits = rng.bits(1146);
    let mut raw = vec![0u8; code.streams() * code.periods()];
    code.encode(&bits, &mut raw).unwrap();
    let mut tx = vec![0u8; map.len()];
    code.transmit(&map, &raw, &mut tx).unwrap();
    let llr = bpsk_llr8(&mut rng, &tx, 0.2, 1.0, 12.0);
    let mut dec = TurboDecoder::new(code);
    dec.load(&map, &llr).unwrap();
    let mut out = vec![0u8; 1146];
    assert!(dec.decode(8, |b| b == &bits[..], &mut out).is_ok());
}

/// TS 102 744-2-1 clause 5.3.8: the update rule, the flush table 5.11 and the worked example.
#[test]
fn sl_constituent_and_flush() {
    // State written T1 T2 T3 T4 is the integer with T1 as the top bit, as the code holds it.
    let flush = [
        "0000", "1000", "1100", "0100", "0110", "1110", "1010", "0010", "0011", "1011", "1111",
        "0111", "0101", "1101", "1001", "0001",
    ];
    for (st, f) in flush.iter().enumerate() {
        let mut s = st;
        let mut got = String::new();
        for _ in 0..4 {
            let u = RSC_SL.zeroing_input(s);
            got.push((b'0' + u) as char);
            s = RSC_SL.step(s, u).1;
        }
        assert_eq!(&got, f, "state {st:04b}");
        assert_eq!(s, 0);
    }
    let mut s = 0;
    let mut par = String::new();
    for u in bits_of("10110010") {
        let (p, n) = RSC_SL.step(s, u);
        par.push((b'0' + p) as char);
        s = n;
    }
    assert_eq!(s, 0b1011);
    for _ in 0..4 {
        let u = RSC_SL.zeroing_input(s);
        let (p, n) = RSC_SL.step(s, u);
        par.push((b'0' + p) as char);
        s = n;
    }
    assert_eq!(par, "110110100001");
}

/// A Family SL code built from rule-made tables (a random interleaver and a symbol map that sends every d bit and part of p
/// and q): clean blocks decode exactly, noisy ones at a moderate SNR too.
#[test]
fn sl_round_trip_with_rule_made_tables() {
    let mut rng = Rng::new(11);
    for &(n, pq) in &[(436usize, 422usize), (3444, 1270)] {
        let mut tci: Vec<u32> = (0..n as u32).collect();
        for i in (1..n).rev() {
            tci.swap(i, rng.below(i + 1));
        }
        let mut idx: Vec<u32> = (0..n as u32).collect();
        for i in 0..pq {
            idx.push((n + i * n / pq) as u32);
            idx.push((2 * n + i * n / pq) as u32);
        }
        let code = sl_code(&tci).unwrap();
        let map = sl_map(&idx, n).unwrap();
        code.check_map(&map).unwrap();
        let k = n - 4;
        let mut dec = TurboDecoder::new(code.clone());
        let mut raw = vec![0u8; code.streams() * code.periods()];
        let mut tx = vec![0u8; map.len()];
        let mut out = vec![0u8; k];
        let bits = rng.bits(k);
        code.encode(&bits, &mut raw).unwrap();
        code.transmit(&map, &raw, &mut tx).unwrap();
        dec.load(&map, &hard_llr(&tx, 20)).unwrap();
        assert!(dec.decode(1, |b| b == &bits[..], &mut out).is_ok());
        let rate = k as f64 / map.len() as f64;
        let llr = bpsk_llr8(&mut rng, &tx, rate, bpsk_limit(rate) + 2.5, 12.0);
        dec.load(&map, &llr).unwrap();
        assert!(dec.decode(8, |b| b == &bits[..], &mut out).is_ok(), "N {n}");
    }
}

#[test]
fn conv_blocks_delay_every_block_the_same() {
    let rx = [0u32, 3, 1, 2];
    let tx = ConvBlocks::<i8>::transmit_delays(&rx);
    assert_eq!(tx, vec![3, 0, 2, 1]);
    let mut t = ConvBlocks::new(&tx, 2, 0i8).unwrap();
    let mut r = ConvBlocks::new(&rx, 2, 0i8).unwrap();
    let mut mid = [0i8; 2];
    let mut out = [0i8; 2];
    let mut got = Vec::new();
    for b in 0..40i8 {
        t.push(&[b, -b], &mut mid);
        let ok = r.push(&mid, &mut out);
        got.push((ok, out[0]));
    }
    // Every block passes 3 cells of 4 blocks: 12 blocks in all.
    for (i, &(_, v)) in got.iter().enumerate().skip(12) {
        assert_eq!(v as usize, i - 12);
    }
    // The receiver marks a branch's output valid only once that branch has been filled.
    assert!(got[0].0);
    assert!(!got[1].0);
    assert!(got[13].0);
}

#[test]
fn kept_permutation_round_trip() {
    let p = KeptPermutation::new(157, 24_576, 126, 128).unwrap();
    assert_eq!(p.kept(), 24_192);
    let a: Vec<i16> = (0..24_576).map(|i| (i % 1000) as i16 + 1).collect();
    let mut x = vec![0i16; p.kept()];
    p.apply(&a, &mut x).unwrap();
    assert_eq!(x[1], a[157]);
    let mut back = vec![0i16; 24_576];
    p.invert(&x, 0, &mut back).unwrap();
    assert_eq!(back.iter().filter(|&&v| v == 0).count(), 384);
    assert!(back.iter().zip(&a).all(|(b, a)| *b == 0 || b == a));
    assert!(KeptPermutation::new(2, 24_576, 126, 128).is_err());
}
