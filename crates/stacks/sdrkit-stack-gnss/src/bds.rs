//! BeiDou B1I D1 and D2: the subframe with its BCH(15,11,1) words, the fields that make the ephemeris and the clock, and BDT −
//! GPS from D1 subframe 5 page 9. A subframe is held as its 224 information bits after correction: word 1's 26 (preamble 11,
//! reserved 4, subframe ID 3, SOW high 8), then 22 per word for words 2 to 10. Two field placements are sdrkit's own and not
//! from the ICD: the D2 ephemeris pages (the ten pages of subframe 1 carry the concatenated D1 subframe 1 to 3 fields, 56 bits
//! a page), and A0GPS/A1GPS in subframe 5 page 9 (after 30 reserved bits).
//!
//! Standard: BDS SIS ICD 5.1 to 5.3.

use crate::orbit::Kepler;
use crate::system::{System, GPS_PI};
use sdrkit_algo_fec::small::SmallCode;
use sdrkit_util_bits::interleave::{Permutation, ReadOrder};
use sdrkit_util_bits::pack::field_split;
use sdrkit_util_bits::HardBits;
use std::sync::OnceLock;

/// The 11-bit preamble.
pub const PREAMBLE: [u8; 11] = [1, 1, 1, 0, 0, 0, 1, 0, 0, 1, 0];

/// Information bits after word 1.
pub const W1: usize = 26;

/// A decoded subframe.
#[derive(Clone, Debug, PartialEq)]
pub struct BdsSubframe {
    pub info: Vec<u8>,
    pub id: u8,
    pub sow: u32,
    pub corrected: u32,
}

impl BdsSubframe {
    fn u(&self, s: usize, l: usize) -> u64 {
        HardBits::at(&self.info, s).u(l as u32)
    }
    fn s(&self, s: usize, l: usize) -> i64 {
        HardBits::at(&self.info, s).i(l as u32)
    }
    /// The page number of a D1 subframe 4 or 5 (7 bits after SOW and a reserved bit), or of a D2 subframe 1 (4 bits after SOW).
    pub fn page(&self, d2: bool) -> u32 {
        if d2 {
            self.u(W1 + 12, 4) as u32
        } else {
            self.u(W1 + 13, 7) as u32
        }
    }
}

/// The BCH(15,11) code of every word, and the bit interleave of words 2 to 10 (two code words sent bit by bit in turn).
struct Coding {
    bch: SmallCode,
    pair: Permutation,
}

/// The coding tables, made once. BDS-SIS-ICD-B1I clause 5.1.3: BCH(15,11) with g(X) = X^4 + X + 1, and the 2 x 15 interleave.
fn coding() -> &'static Coding {
    static CODING: OnceLock<Coding> = OnceLock::new();
    CODING.get_or_init(|| Coding {
        bch: SmallCode::hamming_15_11(),
        // Written as two rows of 15 (the two code words), read by columns.
        pair: Permutation::row_column(2, 15, ReadOrder::Columns).expect("a 2 x 15 matrix"),
    })
}

/// Pushes the 15 bits of `w`, most significant first.
fn push_word(out: &mut Vec<u8>, w: u128) {
    for k in (0..15).rev() {
        out.push((w >> k & 1) as u8);
    }
}

/// Decodes one received 15-bit word and pushes its 11 information bits onto `info`. Returns 1 if a bit was corrected.
fn decode_word(info: &mut Vec<u8>, bits: &[u8]) -> u32 {
    let c = &coding().bch;
    let w = bits.iter().fold(0u128, |v, &x| (v << 1) | (x & 1) as u128);
    let (word, fixed) = c.decode(w).unwrap_or((w, 1));
    let d = c.info(word);
    for k in (0..11).rev() {
        info.push((d >> k & 1) as u8);
    }
    (fixed > 0) as u32
}

/// Encodes a subframe's 224 information bits into its 300 transmitted bits.
pub fn encode_subframe(info: &[u8]) -> Vec<u8> {
    let c = coding();
    let mut out = Vec::with_capacity(300);
    out.extend_from_slice(&info[..15]);
    push_word(&mut out, c.bch.encode(HardBits::at(info, 15).u(11) as u128));
    let mut pair = Vec::with_capacity(30);
    let mut sent = [0u8; 30];
    for wd in 0..9 {
        let base = W1 + wd * 22;
        pair.clear();
        push_word(
            &mut pair,
            c.bch.encode(HardBits::at(info, base).u(11) as u128),
        );
        push_word(
            &mut pair,
            c.bch.encode(HardBits::at(info, base + 11).u(11) as u128),
        );
        // Both slices hold 30 bits, so the permutation cannot fail.
        let _ = c.pair.apply(&pair, &mut sent);
        out.extend_from_slice(&sent);
    }
    out
}

/// Decodes 300 received bits (already in the right polarity): BCH correction, then the checks. `d2` selects the
/// SOW rule.
pub fn decode_subframe(bits: &[u8], d2: bool) -> Option<BdsSubframe> {
    if bits.len() < 300 || bits[..11] != PREAMBLE {
        return None;
    }
    let mut info = Vec::with_capacity(224);
    info.extend_from_slice(&bits[..15]);
    let mut corrected = decode_word(&mut info, &bits[15..30]);
    let mut pair = [0u8; 30];
    for wd in 0..9 {
        // Both slices hold 30 bits, so the permutation cannot fail.
        let _ = coding()
            .pair
            .invert(&bits[30 + wd * 30..60 + wd * 30], &mut pair);
        corrected += decode_word(&mut info, &pair[..15]);
        corrected += decode_word(&mut info, &pair[15..]);
    }
    let id = HardBits::at(&info, 15).u(3) as u8;
    let sow = field_split((&info[..], 18, 8), (&info[..], W1, 12)).unwrap_or(0) as u32;
    let ok = corrected <= 6
        && (1..=5).contains(&id)
        && sow < 604_800
        && if d2 {
            sow.is_multiple_of(3)
        } else {
            sow.is_multiple_of(6) && (sow / 6) % 5 == id as u32 - 1
        };
    ok.then_some(BdsSubframe {
        info,
        id,
        sow,
        corrected,
    })
}

/// The D1 field stream of subframes 1 to 3 (information bits after the SOW), 186 bits each.
pub const FIELDS: usize = 224 - W1 - 12;

fn p2(e: i32) -> f64 {
    2f64.powi(e)
}

/// Builds the ephemeris from the field streams of subframes 1, 2 and 3 (each [`FIELDS`] bits, starting after the SOW).
pub fn ephemeris(f1: &[u8], f2: &[u8], f3: &[u8], prn: u16) -> Kepler {
    let u1 = |s: usize, l: u32| HardBits::at(f1, s).u(l);
    let s1 = |s: usize, l: u32| HardBits::at(f1, s).i(l);
    let u2 = |s: usize, l: u32| HardBits::at(f2, s).u(l);
    let s2 = |s: usize, l: u32| HardBits::at(f2, s).i(l);
    let s3 = |s: usize, l: u32| HardBits::at(f3, s).i(l);
    // Subframe 1 (after SOW): SatH1 1, AODC 5, URAI 4, WN 13, toc 17, TGD1 10, TGD2 10, alpha 4x8, beta 4x8, a2 11, a0 24, a1 22,
    // AODE 5.
    let toe = field_split((f2, 184, 2), (f3, 0, 15)).unwrap_or(0) as f64 * 8.0;
    Kepler {
        system: Some(System::Beidou),
        prn,
        week: u1(10, 13) as i64,
        health: u1(0, 1) as u32,
        iodc: u1(1, 5) as u32,
        ura: u1(6, 4) as u32,
        toc: u1(23, 17) as f64 * 8.0,
        tgd: s1(40, 10) as f64 * 1e-10,
        af2: s1(124, 11) as f64 * p2(-66),
        af0: s1(135, 24) as f64 * p2(-33),
        af1: s1(159, 22) as f64 * p2(-50),
        iode: u1(181, 5) as u32,
        // Subframe 2: dn 16, Cuc 18, M0 32, e 32, Cus 18, Crc 18, Crs 18, sqrtA 32, toe high 2.
        delta_n: s2(0, 16) as f64 * p2(-43) * GPS_PI,
        cuc: s2(16, 18) as f64 * p2(-31),
        m0: s2(34, 32) as f64 * p2(-31) * GPS_PI,
        e: u2(66, 32) as f64 * p2(-33),
        cus: s2(98, 18) as f64 * p2(-31),
        crc: s2(116, 18) as f64 * p2(-6),
        crs: s2(134, 18) as f64 * p2(-6),
        sqrt_a: u2(152, 32) as f64 * p2(-19),
        toe,
        // Subframe 3: toe low 15, i0 32, Cic 18, OmegaDot 24, Cis 18, IDOT 14, Omega0 32, omega 32.
        i0: s3(15, 32) as f64 * p2(-31) * GPS_PI,
        cic: s3(47, 18) as f64 * p2(-31),
        omega_dot: s3(65, 24) as f64 * p2(-43) * GPS_PI,
        cis: s3(89, 18) as f64 * p2(-31),
        idot: s3(107, 14) as f64 * p2(-43) * GPS_PI,
        omega0: s3(121, 32) as f64 * p2(-31) * GPS_PI,
        omega: s3(153, 32) as f64 * p2(-31) * GPS_PI,
        fit: 0,
        geo: crate::system::bds_is_geo(prn),
    }
}

/// The field stream of a subframe (after its SOW).
pub fn fields(sf: &BdsSubframe) -> &[u8] {
    &sf.info[W1 + 12..]
}

/// D1 subframe 5 page 9: A0GPS (0.1 ns) and A1GPS (0.1 ns/s); `None` when both are zero (not broadcast).
pub fn bdt_gps(sf: &BdsSubframe) -> Option<(f64, f64)> {
    if sf.id != 5 || sf.page(false) != 9 {
        return None;
    }
    let base = W1 + 22;
    let a0 = sf.s(base + 30, 14);
    let a1 = sf.s(base + 44, 16);
    (a0 != 0 || a1 != 0).then_some((a0 as f64 * 1e-10, a1 as f64 * 1e-10))
}

/// Bits a D2 page carries of the 558-bit D1 field stream.
pub const D2_PAGE_BITS: usize = 56;

/// Collects D1 subframes 1 to 3 (of one frame) or D2 pages 1 to 10 (of one 30 s cycle).
#[derive(Clone, Debug, Default)]
pub struct BdsCollector {
    sf: [Option<(u32, Vec<u8>)>; 3],
    pages: [Option<(u32, Vec<u8>)>; 10],
}

impl BdsCollector {
    /// Parts held, scaled to 3.
    pub fn parts(&self, d2: bool) -> usize {
        if d2 {
            self.pages.iter().filter(|p| p.is_some()).count() * 3 / 10
        } else {
            self.sf.iter().filter(|p| p.is_some()).count()
        }
    }
    /// Adds a subframe; returns an ephemeris when complete. `week_of` resolves the 13-bit BDT week.
    pub fn add(
        &mut self,
        sf: &BdsSubframe,
        d2: bool,
        prn: u16,
        week_of: impl Fn(u32) -> i64,
    ) -> Option<Kepler> {
        let mut eph = if d2 {
            if sf.id != 1 {
                return None;
            }
            let p = sf.page(true) as usize;
            if !(1..=10).contains(&p) {
                return None;
            }
            let cycle = sf.sow / 30;
            let data = sf.info[W1 + 16..W1 + 16 + D2_PAGE_BITS].to_vec();
            self.pages[p - 1] = Some((cycle, data));
            if self
                .pages
                .iter()
                .any(|x| x.as_ref().map(|y| y.0) != Some(cycle))
            {
                return None;
            }
            let mut stream = Vec::with_capacity(560);
            for x in self.pages.iter().flatten() {
                stream.extend_from_slice(&x.1);
            }
            for x in self.pages.iter_mut() {
                *x = None;
            }
            ephemeris(
                &stream[..FIELDS],
                &stream[FIELDS..2 * FIELDS],
                &stream[2 * FIELDS..3 * FIELDS],
                prn,
            )
        } else {
            if !(1..=3).contains(&sf.id) {
                return None;
            }
            let frame = sf.sow / 30;
            self.sf[sf.id as usize - 1] = Some((frame, fields(sf).to_vec()));
            let (Some(a), Some(b), Some(c)) = (&self.sf[0], &self.sf[1], &self.sf[2]) else {
                return None;
            };
            if a.0 != frame || b.0 != frame || c.0 != frame {
                return None;
            }
            ephemeris(&a.1, &b.1, &c.1, prn)
        };
        eph.week = week_of(eph.week as u32);
        Some(eph)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sdrkit_util_bits::pack::put_u64;

    #[test]
    fn subframe_round_trip_with_errors() {
        let mut info = vec![0u8; 224];
        info[..11].copy_from_slice(&PREAMBLE);
        put_u64(&mut info, 15, 2, 3).unwrap();
        let sow = 354_486u32; // a multiple of 6 with (sow/6) mod 5 = 1
        assert_eq!((sow / 6) % 5, 1);
        put_u64(&mut info, 18, (sow >> 12) as u64, 8).unwrap();
        put_u64(&mut info, W1, (sow & 0xFFF) as u64, 12).unwrap();
        for i in W1 + 12..224 {
            info[i] = ((i * 7) % 3 == 0) as u8;
        }
        let mut tx = encode_subframe(&info);
        let sf = decode_subframe(&tx, false).unwrap();
        assert_eq!(sf.info, info);
        assert_eq!((sf.id, sf.sow, sf.corrected), (2, sow, 0));
        // One error in each of several BCH words is corrected; the interleaving puts word 2's two codewords on alternate bits.
        for k in [20usize, 31, 32, 95, 299] {
            tx[k] ^= 1;
        }
        let sf = decode_subframe(&tx, false).unwrap();
        assert_eq!(sf.info, info);
        assert_eq!(sf.corrected, 5);
    }
}
