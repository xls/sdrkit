//! The parts of `slotted_voice` that both frame versions use: its own 16-QAM map, the QPSK map, the (640, 320) LDPC code with
//! shortening, the interleaver permutations, the slot scramblers, the RRC pulses and the bit-serial CRCs.

use sdrkit_algo_dsp::constellation::{PointSet, QamMap};
use sdrkit_algo_dsp::pulse::{taps, Norm, Shape};
use sdrkit_algo_fec::ldpc::{LdpcCode, LdpcDecoder, MinSum};
use sdrkit_algo_modem::{Constellation, Demap};
use sdrkit_core::Complex32;
use sdrkit_util_bits::xorshift::Xorshift64;
use std::sync::{Arc, Mutex, OnceLock};

/// The seed of the interleaver permutations.
pub const PERM_SEED: u64 = 0x4B41_414C;

/// The code length.
pub const N: usize = 640;
/// The information length of the full code.
pub const K: usize = 320;
/// The lifting size.
pub const Z: usize = 32;
/// The LLR given to a shortened bit (known 0), in the decoder's units.
pub const SHORT_LLR: i8 = 60;

/// The base matrix of the code: 10 rows of 20 shifts, -1 a zero block; columns 10 to 19 are parity.
pub const BASE: [[i8; 20]; 10] = [
    [17, -1, 28, -1, -1, -1, -1, -1, 24, -1, 1, 0, -1, -1, -1, -1, -1, -1, -1, -1],
    [2, -1, -1, -1, 30, -1, 10, -1, -1, 30, -1, 0, 0, -1, -1, -1, -1, -1, -1, -1],
    [0, -1, -1, -1, -1, 29, -1, -1, 9, 26, -1, -1, 0, 0, -1, -1, -1, -1, -1, -1],
    [-1, 12, -1, -1, 15, -1, -1, 28, -1, -1, -1, -1, -1, 0, 0, -1, -1, -1, -1, -1],
    [-1, 22, -1, 24, -1, 5, -1, -1, -1, -1, -1, -1, -1, -1, 0, 0, -1, -1, -1, -1],
    [29, -1, -1, -1, 6, -1, 7, -1, -1, -1, 0, -1, -1, -1, -1, 0, 0, -1, -1, -1],
    [-1, -1, 10, 1, -1, -1, -1, 16, -1, 4, -1, -1, -1, -1, -1, -1, 0, 0, -1, -1],
    [2, -1, -1, -1, -1, 29, -1, -1, 25, -1, -1, -1, -1, -1, -1, -1, -1, 0, 0, -1],
    [-1, 28, 8, -1, -1, -1, -1, 4, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, 0, 0],
    [27, -1, -1, 18, -1, -1, 28, -1, -1, -1, 1, -1, -1, -1, -1, -1, -1, -1, -1, 0],
];

/// The (640, 320) code, made once.
pub fn code() -> &'static LdpcCode {
    static C: OnceLock<LdpcCode> = OnceLock::new();
    C.get_or_init(|| {
        let rows: Vec<Vec<(u16, u16)>> = BASE
            .iter()
            .map(|r| {
                r.iter()
                    .enumerate()
                    .filter(|(_, &s)| s >= 0)
                    .map(|(c, &s)| (c as u16, s as u16))
                    .collect()
            })
            .collect();
        let refs: Vec<&[(u16, u16)]> = rows.iter().map(|r| r.as_slice()).collect();
        LdpcCode::from_qc("slotted voice 640", Z, 20, K, &refs).expect("the slotted voice code")
    })
}

/// The min-sum check rule of the code: scale 14/16, no offset. In white noise (`tests/ldpc_scale.rs`) 15/16 gains 0.2 dB
/// on the QPSK slot of 128 bits, but it loses frames on a fading path with a moving phase
/// (`tests/slotted_voice_v4.rs`); an offset of 1 loses up to 1.7 dB on the shortened words.
pub const RULE: MinSum = MinSum::with_scale(14);

/// A decoder of the code with [`RULE`].
pub fn decoder() -> LdpcDecoder {
    LdpcDecoder::new(code(), RULE)
}

/// Encodes `k` information bits (the rest of the 320 are zero) into the 640-bit code word.
pub fn encode(info: &[u8], out: &mut [u8; N]) {
    let mut full = [0u8; K];
    full[..info.len()].copy_from_slice(info);
    code().encode(&full, out).expect("the slotted voice code word");
}

/// Decodes the soft code word `llr` (positive means 0) of a word that carries `k` information bits: the shortened bits get
/// [`SHORT_LLR`]. Returns the `k` bits when every check holds and every shortened bit decodes as 0.
pub fn decode(dec: &mut LdpcDecoder, llr: &[i8; N], k: usize, iters: u32) -> Option<Vec<u8>> {
    let (good, out) = decode_word(dec, llr, k, iters);
    good.then(|| out[..k].to_vec())
}

/// Decodes as [`decode`] does and returns the hard code word of the last iteration with the verdict: true when every check
/// holds and every shortened bit decodes as 0. The word of a failed decode is the decoder's best reading of the bits sent.
pub fn decode_word(dec: &mut LdpcDecoder, llr: &[i8; N], k: usize, iters: u32) -> (bool, [u8; N]) {
    let mut l = *llr;
    for v in &mut l[k..K] {
        *v = SHORT_LLR;
    }
    let mut out = [0u8; N];
    let good = dec.decode(&l, iters, &mut out).is_ok() && out[k..K].iter().all(|&b| b == 0);
    (good, out)
}

/// Turns float LLRs into the decoder's 8-bit units: `scale` per unit, limited to +-127.
pub fn quantise(llr: f32, scale: f32) -> i8 {
    (llr * scale).round().clamp(-127.0, 127.0) as i8
}

/// The interleaver permutation of length `n`: a Fisher-Yates shuffle from [`PERM_SEED`] with `j = r mod (i + 1)` of the full
/// draw. Interleave: `out[i] = in[p[i]]`.
pub fn permutation(n: usize) -> Arc<Vec<u16>> {
    static CACHE: OnceLock<Mutex<Vec<Arc<Vec<u16>>>>> = OnceLock::new();
    let cache = CACHE.get_or_init(|| Mutex::new(Vec::new()));
    let mut held = cache.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(p) = held.iter().find(|p| p.len() == n) {
        return p.clone();
    }
    let mut p: Vec<u16> = (0..n as u16).collect();
    let mut g = Xorshift64::new(PERM_SEED);
    for i in (1..n).rev() {
        let j = (g.step() % (i as u64 + 1)) as usize;
        p.swap(i, j);
    }
    let p = Arc::new(p);
    held.push(p.clone());
    p
}

/// `count` bits of the scrambler from state `seed` (0 becomes 1): bit 23 of each draw.
pub fn scrambler(seed: u64, count: usize) -> Vec<u8> {
    let mut g = Xorshift64::with_zero_seed(seed, 1);
    (0..count).map(|_| ((g.step() >> 23) & 1) as u8).collect()
}

/// The 16-QAM of the stack: the first two bits give I, the last two Q, with 00, 01, 11, 10 giving -3, -1, +1, +3, over sqrt(10).
pub fn qam16() -> &'static Constellation {
    static C: OnceLock<Constellation> = OnceLock::new();
    C.get_or_init(|| {
        Constellation::new(PointSet::qam(16, QamMap::IEEE_802_11).expect("16-QAM")).expect("16-QAM")
    })
}

/// QPSK: the first bit gives I, the second Q, bit 0 is +1/sqrt(2).
pub fn qpsk() -> &'static Constellation {
    static C: OnceLock<Constellation> = OnceLock::new();
    C.get_or_init(|| Constellation::new(crate::family::qam::point_set(2).expect("QPSK")).expect("QPSK"))
}

/// Maps bits to symbols of `c` (4 or 2 bits each).
pub fn map(c: &Constellation, bits: &[u8], out: &mut Vec<Complex32>) {
    let per = c.bits();
    for s in bits.chunks(per) {
        let label = s.iter().fold(0usize, |a, &b| (a << 1) | usize::from(b & 1));
        out.push(c.points()[label]);
    }
}

/// Max-log LLRs of the symbols `z` (unit-power scale) for noise variance `nv`, appended to `out`.
pub fn demap(c: &Constellation, z: &[Complex32], nv: f32, out: &mut Vec<f32>) {
    let per = c.bits();
    let start = out.len();
    out.resize(start + z.len() * per, 0.0);
    let nv = nv.max(1e-6);
    let _ = c.demap_block(z, nv, Demap::MaxLog, &mut out[start..]);
}

/// The unit-energy RRC with roll-off 0.25, `span` symbols at `sps` samples a symbol (`span × sps + 1` taps).
pub fn rrc(sps: usize, span: usize, beta: f64) -> Vec<f32> {
    taps(Shape::Rrc { beta }, sps as f64, span as f64, Norm::Energy).expect("RRC taps")
}

/// The CRC-8 of the sweep identity: polynomial 0x07, register all ones, no reflection, no final XOR.
pub fn crc8(bytes: &[u8]) -> u8 {
    sdrkit_util_bits::crc::catalog::CRC8_ESB.checksum(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::family::{pack, push_bits};

    fn hex_bits(h: &str, n: usize) -> Vec<u8> {
        let mut v = Vec::new();
        for c in h.chars() {
            push_bits(&mut v, u64::from(c.to_digit(16).unwrap()), 4);
        }
        v.truncate(n);
        v
    }

    fn message() -> Vec<u8> {
        let mut g = Xorshift64::new(0x1234);
        (0..320).map(|_| ((g.step() >> 40) & 1) as u8).collect()
    }

    #[test]
    fn code_word_and_rate_matching() {
        let m = message();
        assert_eq!(m, hex_bits("3ae5d1e1e0940cfdcc8e22e87775e092550c4a36d842ae2898a8bb0dbd8a371a2c328df64734db4e", 320));
        assert_eq!(pack(&m).len(), 40);
        let mut cw = [0u8; N];
        encode(&m, &mut cw);
        assert_eq!(cw.to_vec(), hex_bits("3ae5d1e1e0940cfdcc8e22e87775e092550c4a36d842ae2898a8bb0dbd8a371a2c328df64734db4ee61085c01507aee958f2fb5125184040eb1417be4e4e8246984db6e30a7add9b16bdaac19e357369", 640));
        let mut z = [0u8; N];
        encode(&[0u8; 320], &mut z);
        assert!(z.iter().all(|&b| b == 0));
        // k = 240: the 240 information bits, then the first 240 parity bits.
        let mut cw240 = [0u8; N];
        encode(&m[..240], &mut cw240);
        let mut w: Vec<u8> = m[..240].to_vec();
        w.extend_from_slice(&cw240[320..560]);
        assert_eq!(w, hex_bits("3ae5d1e1e0940cfdcc8e22e87775e092550c4a36d842ae2898a8bb0dbd8ad5fa1d8584feacee58c6cf85792b4ba117271f2eb27d8ad65794263681f4", 480));
    }

    #[test]
    fn the_decoder_corrects_24_flips_and_shortening_tells_k_apart() {
        let m = message();
        let mut cw = [0u8; N];
        encode(&m, &mut cw);
        let mut llr = [0i8; N];
        for i in 0..N {
            llr[i] = if cw[i] == 0 { 4 } else { -4 };
        }
        let mut dec = decoder();
        assert_eq!(decode(&mut dec, &llr, 320, 40), Some(m.clone()));
        for p in [210, 282, 288, 43, 419, 20, 146, 578, 341, 33, 323, 10, 606, 489, 455, 452, 72, 624, 529, 430, 417, 495, 92, 28] {
            llr[p] = -llr[p];
        }
        assert_eq!(decode(&mut dec, &llr, 320, 40), Some(m.clone()));
        // A word of k = 240 does not decode as 320 with its bits beyond 240 forced to 0 unless they are 0.
        let mut cw240 = [0u8; N];
        encode(&m[..240], &mut cw240);
        let mut l2 = [0i8; N];
        for i in 0..N {
            l2[i] = if cw240[i] == 0 { 8 } else { -8 };
        }
        assert_eq!(decode(&mut dec, &l2, 240, 40), Some(m[..240].to_vec()));
    }

    #[test]
    fn permutations_scramblers_and_maps() {
        let checks: [(usize, [u16; 16], u16); 5] = [
            (1920, [1696, 699, 1562, 758, 1621, 253, 345, 98, 480, 166, 1445, 1728, 1230, 1024, 1178, 1142], 334),
            (2560, [1715, 59, 2161, 2371, 933, 975, 1044, 1616, 903, 1651, 1277, 2521, 611, 212, 1563, 1496], 2254),
            (544, [100, 127, 397, 209, 340, 342, 421, 139, 18, 300, 374, 16, 511, 475, 210, 467], 302),
            (272, [25, 57, 93, 235, 225, 232, 5, 240, 104, 260, 190, 102, 191, 72, 136, 155], 30),
            (640, [16, 173, 395, 119, 17, 59, 131, 362, 140, 413, 214, 308, 159, 67, 33, 545], 334),
        ];
        for (n, first, last) in checks {
            let p = permutation(n);
            assert_eq!(&p[..16], &first, "{n}");
            assert_eq!(p[n - 1], last, "{n}");
        }
        let firsts = [
            (0x4B41_414C_3400_0001u64, 0xaaf3774d985d3b2cu64),
            (0x4B41_414C_3400_0002, 0x1363f5659f33e3b0),
            (0x4B41_414C_3400_0003, 0xb072539698a45944),
            (0x4B41_414C_3400_0004, 0x01af725c3415156d),
            (0x4B41_414C_3443_544C, 0x88d644e9fe2971db),
        ];
        for (seed, want) in firsts {
            let s = scrambler(seed, 64);
            assert_eq!(s.iter().fold(0u64, |a, &b| (a << 1) | u64::from(b)), want);
        }
        // 16-QAM levels of the 64 bits 299a13939f77cbe2.
        let bits = hex_bits("299a13939f77cbe2", 64);
        let mut sym = Vec::new();
        map(qam16(), &bits, &mut sym);
        let want = [(-3, 3), (3, -1), (3, -1), (3, 3), (-3, -1), (-3, 1), (3, -1), (-3, 1), (3, -1), (1, 1), (-1, 1), (-1, 1), (1, -3), (3, 1), (1, 3), (-3, 3)];
        let k = 10f32.sqrt();
        for (s, (i, q)) in sym.iter().zip(want) {
            assert!((s.re * k - i as f32).abs() < 1e-5 && (s.im * k - q as f32).abs() < 1e-5);
        }
        // Soft demap of 8 symbols at noise variance 0.1.
        let rx = [(-0.512, 1.278), (3.742, 0.248), (0.793, 1.316), (-3.292, 3.939), (0.393, 2.653), (1.310, -3.302), (3.245, -1.320), (-1.516, -1.559)];
        let z: Vec<Complex32> = rx.iter().map(|&(a, b)| Complex32::new(a / k, b / k)).collect();
        let mut l = Vec::new();
        demap(qam16(), &z, 0.1, &mut l);
        let want = [2.048, -5.952, -5.112, -2.888, -21.936, 6.968, -0.992, -7.008, -3.172, -4.828, -5.264, -2.736, 18.336, 5.168, -23.512, 7.756, -1.572, -6.428, -13.224, 2.612, -5.240, -2.760, 18.416, 5.208, -17.960, 4.980, 5.280, -2.720, 6.064, -1.936, 6.236, -1.764];
        for (a, b) in l.iter().zip(want) {
            assert!((a - b).abs() < 2e-3, "{a} {b}");
        }
        // The rate 1/2 code of the first 64 message bits, without a tail.
        let m = message();
        let mut enc = sdrkit_algo_fec::conv::Encoder::new(sdrkit_algo_fec::conv::codes::K7_171_133);
        let mut out = Vec::new();
        enc.encode(&m[..64], &mut out);
        assert_eq!(out, hex_bits("0dac87d26c141659a65a2c9bb7d40241", 128));
        // The RRC pulses.
        let h = rrc(6, 8, 0.25);
        assert_eq!(h.len(), 49);
        for (i, v) in [(0, 0.008665328), (1, 0.006582527), (3, -0.007471234), (7, -0.004912148), (24, 0.436237888)] {
            assert!((h[i] as f64 - v).abs() < 1e-6, "{i}");
        }
        let h4 = rrc(3, 10, 0.25);
        assert_eq!(h4.len(), 31);
        assert!((h4[0] as f64 + 0.004332194).abs() < 1e-6 && (h4[15] as f64 - 0.616866642).abs() < 1e-6);
        assert_eq!(crc8(b"123456789"), 0xFB);
    }
}
