//! The shared quasi-cyclic LDPC family: lengths 1920, 2304 and 7680 bits, rate indices 0 to 6 (1/4, 1/3, 1/2, 2/3, 3/4, 5/6,
//! 9/10). The base matrices are data (`ldpc_tables.txt`); each code is systematic, information bits first, with a block
//! staircase parity part, so it encodes by back substitution. [`construct`] is the rule that made the tables, kept as a check.

use sdrkit_algo_fec::ldpc::staircase::StaircaseEncoder;
use sdrkit_algo_fec::ldpc::{LdpcCode, LdpcDecoder, MinSum};
use sdrkit_util_bits::xorshift::Xorshift64;
use std::sync::{Arc, Mutex, OnceLock};

/// The rates by rate index, as (numerator, denominator).
pub const RATES: [(usize, usize); 7] = [(1, 4), (1, 3), (1, 2), (2, 3), (3, 4), (5, 6), (9, 10)];

/// The data file of the base matrices.
const TABLES: &str = include_str!("ldpc_tables.txt");

/// One base matrix of the family.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BaseMatrix {
    /// The code length, bits.
    pub n: usize,
    /// The rate index, 0 to 6.
    pub rate: usize,
    /// The information length, bits.
    pub k: usize,
    /// The lifting size.
    pub z: usize,
    /// Base columns.
    pub nb: usize,
    /// Information base columns.
    pub kb: usize,
    /// The shifts, row by row (`mb × nb`); -1 is an all-zero block.
    pub shifts: Vec<i16>,
    /// The CRC-32 of the packed test code word.
    pub test_crc: u32,
    /// The first eight parity bytes of the test code word.
    pub test_parity: [u8; 8],
}

impl BaseMatrix {
    /// Base rows.
    pub fn mb(&self) -> usize {
        self.nb - self.kb
    }

    /// The entries `(column, shift)` of each base row.
    pub fn rows(&self) -> Vec<Vec<(u16, u16)>> {
        self.shifts
            .chunks(self.nb)
            .map(|r| {
                r.iter()
                    .enumerate()
                    .filter(|(_, &s)| s >= 0)
                    .map(|(c, &s)| (c as u16, s as u16))
                    .collect()
            })
            .collect()
    }
}

/// Every base matrix of the data file, in file order.
pub fn tables() -> &'static [BaseMatrix] {
    static T: OnceLock<Vec<BaseMatrix>> = OnceLock::new();
    T.get_or_init(|| parse(TABLES))
}

fn parse(text: &str) -> Vec<BaseMatrix> {
    let mut out: Vec<BaseMatrix> = Vec::new();
    for line in text.lines().filter(|l| !l.starts_with('#') && !l.trim().is_empty()) {
        let w: Vec<&str> = line.split_whitespace().collect();
        if w[0] == "code" {
            let num = |i: usize| w[i].parse::<usize>().expect("a number in the LDPC tables");
            let p = u64::from_str_radix(w[8], 16).expect("hex parity in the LDPC tables");
            out.push(BaseMatrix {
                n: num(1),
                rate: num(2),
                k: num(3),
                z: num(4),
                nb: num(5),
                kb: num(6),
                shifts: Vec::new(),
                test_crc: u32::from_str_radix(w[7], 16).expect("hex CRC in the LDPC tables"),
                test_parity: p.to_be_bytes(),
            });
        } else if let Some(m) = out.last_mut() {
            m.shifts.extend(w.iter().map(|v| v.parse::<i16>().expect("a shift in the LDPC tables")));
        }
    }
    out
}

/// The base matrix of length `n` and rate index `rate`; `None` for a code the family does not have.
pub fn base(n: usize, rate: usize) -> Option<&'static BaseMatrix> {
    tables().iter().find(|m| m.n == n && m.rate == rate)
}

/// One code of the family, ready to encode and decode.
pub struct FamilyCode {
    /// Its base matrix.
    pub base: &'static BaseMatrix,
    /// The parity-check description the decoders use.
    pub code: LdpcCode,
    /// The back-substitution encoder.
    pub encoder: StaircaseEncoder,
}

impl FamilyCode {
    /// Encodes `k` information bits into the `n` bits of `out`.
    pub fn encode(&self, info: &[u8], out: &mut [u8]) {
        self.encoder
            .encode(info, out)
            .expect("the lengths of a family code word");
    }

    /// A layered min-sum decoder of the code, with the check rule of [`rule`].
    pub fn decoder(&self) -> LdpcDecoder {
        LdpcDecoder::new(&self.code, rule(self.base.n, self.base.rate))
    }
}

/// The min-sum check rule of the code of length `n` and rate index `rate`, measured on BPSK in white noise
/// (`tests/ldpc_scale.rs`): the lowest Eb/N0 at frame error rates of 0.1 and 0.01 among the rules with no more failures
/// 0.3 dB above. An offset of 1 gains up to 0.35 dB at 0.01 on the 7680-bit codes and on the 2304-bit codes from rate 2/3
/// up; the 1920-bit codes gain less than 0.05 dB with it, and the others nothing.
pub fn rule(n: usize, rate: usize) -> MinSum {
    let (scale_q4, offset) = match (n, rate) {
        (7680, 0 | 1) => (16, 1),
        (7680 | 2304, 3..) => (15, 1),
        (1920 | 2304, 0) => (16, 0),
        (1920, 2) => (14, 0),
        (_, 0..=2) => (15, 0),
        _ => (13, 0),
    };
    MinSum { scale_q4, offset }
}

/// The code of length `n` and rate index `rate`, made once per process; `None` for a code the family does not have.
pub fn code(n: usize, rate: usize) -> Option<Arc<FamilyCode>> {
    static CODES: OnceLock<Mutex<Vec<Arc<FamilyCode>>>> = OnceLock::new();
    let cache = CODES.get_or_init(|| Mutex::new(Vec::new()));
    let mut held = cache.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(c) = held.iter().find(|c| c.base.n == n && c.base.rate == rate) {
        return Some(c.clone());
    }
    let b = base(n, rate)?;
    let rows = b.rows();
    let refs: Vec<&[(u16, u16)]> = rows.iter().map(|r| r.as_slice()).collect();
    let name = format!("family {n} rate {}/{}", RATES[rate].0, RATES[rate].1);
    let code = LdpcCode::from_qc(&name, b.z, b.nb, b.k, &refs).ok()?;
    let encoder = StaircaseEncoder::new(b.z, b.kb, &refs).ok()?;
    let c = Arc::new(FamilyCode {
        base: b,
        code,
        encoder,
    });
    held.push(c.clone());
    Some(c)
}

/// The lifting size of the rule: the largest value at or below 160 (n = 7680) or 48 (other lengths) that divides gcd(n, k).
pub fn lifting(n: usize, k: usize) -> usize {
    let g = gcd(n, k);
    let start = if n == 7680 { 160 } else { 48 };
    (1..=start).rev().find(|z| g.is_multiple_of(*z)).unwrap_or(1)
}

fn gcd(a: usize, b: usize) -> usize {
    if b == 0 {
        a
    } else {
        gcd(b, a % b)
    }
}

/// Builds the base matrix of length `n` and rate index `rate` with the construction rule (row by row, `-1` for zero blocks).
pub fn construct(n: usize, rate: usize) -> Vec<i16> {
    let (a, d) = RATES[rate];
    let k = n * a / d;
    let z = lifting(n, k);
    let (nb, kb) = (n / z, k / z);
    let mb = nb - kb;
    let seed = Xorshift64::GOLDEN ^ ((n as u64) << 20) ^ (((rate as u64) + 1) << 8) ^ 0x4B52_4953;
    let mut g = Xorshift64::new(seed);
    for _ in 0..8 {
        g.step();
    }
    let mut h = vec![-1i16; mb * nb];
    // used[(r1 * mb + r2) * z + d]: difference d is used by the pair r1 < r2.
    let mut used = vec![false; mb * mb * z];
    for j in 0..mb.saturating_sub(1) {
        used[(j * mb + j + 1) * z] = true;
    }
    let mut degree = vec![0usize; mb];
    let heavy = (kb / 4).max(1);
    for c in 0..kb {
        let mut w = (if c < heavy { 7 } else { 3 }).min(mb);
        loop {
            let tries = if w == 1 { 1 } else { 400 };
            let mut placed: Option<Vec<(usize, usize)>> = None;
            for _ in 0..tries {
                let mut keyed: Vec<(u64, usize)> = (0..mb).map(|r| (degree[r] as u64 * 5 + g.below(6), r)).collect();
                keyed.sort();
                let chosen: Vec<usize> = keyed[..w].iter().map(|&(_, r)| r).collect();
                let mut picks: Vec<(usize, usize)> = Vec::with_capacity(w);
                let mut ok = true;
                for &r in &chosen {
                    let free: Vec<usize> = (0..z)
                        .filter(|&s| {
                            picks.iter().all(|&(rj, sj)| {
                                let (r1, s1, r2, s2) = if rj < r { (rj, sj, r, s) } else { (r, s, rj, sj) };
                                !used[(r1 * mb + r2) * z + (s1 + z - s2) % z]
                            })
                        })
                        .collect();
                    if free.is_empty() {
                        ok = false;
                        break;
                    }
                    let s = free[g.below(free.len() as u64) as usize];
                    picks.push((r, s));
                }
                if ok {
                    placed = Some(picks);
                    break;
                }
            }
            if let Some(picks) = placed {
                for (i, &(ra, sa)) in picks.iter().enumerate() {
                    for &(rb, sb) in &picks[i + 1..] {
                        let (r1, s1, r2, s2) = if ra < rb { (ra, sa, rb, sb) } else { (rb, sb, ra, sa) };
                        used[(r1 * mb + r2) * z + (s1 + z - s2) % z] = true;
                    }
                    h[ra * nb + c] = sa as i16;
                    degree[ra] += 1;
                }
                break;
            }
            w -= 1;
        }
    }
    for r in 0..mb {
        h[r * nb + kb + r] = 0;
        if r + 1 < mb {
            h[(r + 1) * nb + kb + r] = 0;
        }
    }
    h
}

#[cfg(test)]
mod tests {
    use super::*;
    use sdrkit_util_bits::crc::catalog::CRC32_ISO_HDLC;

    fn pack(bits: &[u8]) -> Vec<u8> {
        bits.chunks(8).map(|c| c.iter().fold(0u8, |a, &b| (a << 1) | b)).collect()
    }

    #[test]
    fn the_tables_hold_the_twenty_codes() {
        assert_eq!(tables().len(), 20);
        assert!(base(2304, 6).is_none());
        for m in tables() {
            let (a, d) = RATES[m.rate];
            assert_eq!(m.k, m.n * a / d);
            assert_eq!(m.z, lifting(m.n, m.k), "{} {}", m.n, m.rate);
            assert_eq!(m.shifts.len(), m.mb() * m.nb);
        }
    }

    #[test]
    fn the_rule_rebuilds_four_tables() {
        for (n, r) in [(1920, 0), (1920, 1), (1920, 6), (7680, 6)] {
            assert_eq!(construct(n, r), base(n, r).unwrap().shifts, "{n} {r}");
        }
    }

    #[test]
    fn every_code_gives_its_test_word() {
        for m in tables() {
            let c = code(m.n, m.rate).unwrap();
            let mut g = Xorshift64::new(1);
            let info: Vec<u8> = (0..m.k).map(|_| (g.step() >> 63) as u8).collect();
            let mut cw = vec![0u8; m.n];
            c.encode(&info, &mut cw);
            assert!(c.code.is_codeword(&cw), "{} {}", m.n, m.rate);
            let bytes = pack(&cw);
            assert_eq!(CRC32_ISO_HDLC.checksum(&bytes), m.test_crc, "{} {}", m.n, m.rate);
            assert_eq!(&bytes[m.k / 8..m.k / 8 + 8], &m.test_parity, "{} {}", m.n, m.rate);
        }
    }
}
