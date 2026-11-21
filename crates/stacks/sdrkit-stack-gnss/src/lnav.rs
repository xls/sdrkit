//! GPS and QZSS LNAV (IS-GPS-200 20.3.2, 20.3.3, 20.3.5): word parity, subframe checks and the fields of subframes 1 to 5.
//!
//! Bits are numbered as the standard numbers them, 1 to 300 in a subframe; here `bits[n - 1]`.

use crate::geo::Klobuchar;
use crate::orbit::{Almanac, Kepler};
use crate::system::{System, GPS_PI};
use sdrkit_util_bits::pack::{field_split, field_split_signed};
use sdrkit_util_bits::HardBits;
use serde::{Deserialize, Serialize};

/// The preamble 10001011.
pub const PREAMBLE: [u8; 8] = [1, 0, 0, 0, 1, 0, 1, 1];

/// The data bits (1-based) each parity bit D25..D30 sums, with which of D29*/D30* (Table 20-XIV).
const PARITY: [(&[u8], bool); 6] = [
    (&[1, 2, 3, 5, 6, 10, 11, 12, 13, 14, 17, 18, 20, 23], false),
    (&[2, 3, 4, 6, 7, 11, 12, 13, 14, 15, 18, 19, 21, 24], true),
    (&[1, 3, 4, 5, 7, 8, 12, 13, 14, 15, 16, 19, 20, 22], false),
    (&[2, 4, 5, 6, 8, 9, 13, 14, 15, 16, 17, 20, 21, 23], true),
    (
        &[1, 3, 5, 6, 7, 9, 10, 14, 15, 16, 17, 18, 21, 22, 24],
        true,
    ),
    (&[3, 5, 6, 8, 9, 10, 11, 13, 15, 19, 22, 23, 24], false),
];

/// Checks one received 30-bit word given the last two bits of the word before (D29*, D30*). On success returns the 24 data bits
/// (with the D30* inversion undone).
pub fn check_word(word: &[u8], d29: u8, d30: u8) -> Option<[u8; 24]> {
    let mut d = [0u8; 24];
    for i in 0..24 {
        d[i] = word[i] ^ d30;
    }
    for (j, (taps, uses30)) in PARITY.iter().enumerate() {
        let mut p = if *uses30 { d30 } else { d29 };
        for &t in taps.iter() {
            p ^= d[t as usize - 1];
        }
        if p != word[24 + j] {
            return None;
        }
    }
    Some(d)
}

/// The syndrome of a word (six bits; 0 when it checks), for the parity tests.
pub fn syndrome(word: &[u8], d29: u8, d30: u8) -> u8 {
    let mut s = 0u8;
    for (j, (taps, uses30)) in PARITY.iter().enumerate() {
        let mut p = if *uses30 { d30 } else { d29 };
        for &t in taps.iter() {
            p ^= word[t as usize - 1] ^ d30;
        }
        s |= (p ^ word[24 + j]) << j;
    }
    s
}

/// A received subframe that passed its checks, as 240 data bits (ten words of 24, parity removed and the inversion undone).
#[derive(Clone, Debug, PartialEq)]
pub struct Subframe {
    /// 10 × 24 data bits; `data[w * 24 + i]` is bit i+1 of word w+1.
    pub data: Vec<u8>,
    pub id: u8,
    pub tow_count: u32,
    /// The whole subframe was received inverted (its preamble read 01110100).
    pub inverted: bool,
}

impl Subframe {
    /// Bit `n` of the subframe in the standard's numbering (1 to 300; parity positions excluded).
    pub fn bit(&self, n: usize) -> u8 {
        let w = (n - 1) / 30;
        self.data[w * 24 + (n - 1) % 30]
    }
    fn u(&self, n: usize, len: usize) -> u64 {
        let w = (n - 1) / 30;
        HardBits::at(&self.data, w * 24 + (n - 1) % 30).u(len as u32)
    }
    fn s(&self, n: usize, len: usize) -> i64 {
        let w = (n - 1) / 30;
        HardBits::at(&self.data, w * 24 + (n - 1) % 30).i(len as u32)
    }
    /// The index in `data` of subframe bit `n` (1 to 300).
    fn at(n: usize) -> usize {
        (n - 1) / 30 * 24 + (n - 1) % 30
    }
    /// A field the standard splits: `a.1` bits at subframe bit `a.0`, then `b.1` bits at `b.0` (two's complement).
    fn s2(&self, a: (usize, usize), b: (usize, usize)) -> i64 {
        let d = &self.data[..];
        field_split_signed(
            (d, Self::at(a.0), a.1 as u32),
            (d, Self::at(b.0), b.1 as u32),
        )
        .unwrap_or(0)
    }
    /// A field the standard splits, unsigned (see [`Subframe::s2`]).
    fn u2(&self, a: (usize, usize), b: (usize, usize)) -> u64 {
        let d = &self.data[..];
        field_split(
            (d, Self::at(a.0), a.1 as u32),
            (d, Self::at(b.0), b.1 as u32),
        )
        .unwrap_or(0)
    }
    /// The page's SV ID (subframes 4 and 5).
    pub fn sv_id(&self) -> u8 {
        self.u(63, 6) as u8
    }
    /// The time of the start of this subframe, seconds of week: ((TOW − 1) mod 100 800) × 6.
    pub fn start_tow(&self) -> f64 {
        ((self.tow_count as i64 - 1).rem_euclid(100_800) * 6) as f64
    }
}

/// Checks 300 received bits as a subframe: the preamble in either polarity, the ten parities (each word tried as received and
/// inverted), a subframe ID 1 to 5 and a TOW count under 100 800.
pub fn check_subframe(bits: &[u8]) -> Option<Subframe> {
    if bits.len() < 300 {
        return None;
    }
    let inverted = if bits[..8] == PREAMBLE {
        false
    } else if bits[..8].iter().zip(PREAMBLE).all(|(a, b)| *a != b) {
        true
    } else {
        return None;
    };
    let mut data = Vec::with_capacity(240);
    // The word before the first ends in 00 by design.
    let (mut d29, mut d30) = (0u8, 0u8);
    let mut word = [0u8; 30];
    for w in 0..10 {
        let raw = &bits[w * 30..w * 30 + 30];
        let mut ok = None;
        for inv in [inverted, !inverted] {
            for i in 0..30 {
                word[i] = raw[i] ^ inv as u8;
            }
            if let Some(d) = check_word(&word, d29, d30) {
                ok = Some(d);
                break;
            }
        }
        let d = ok?;
        data.extend_from_slice(&d);
        d29 = word[28];
        d30 = word[29];
    }
    let tow_count = HardBits::at(&data, 24).u(17) as u32;
    let id = HardBits::at(&data, 24 + 19).u(3) as u8;
    if !(1..=5).contains(&id) || tow_count >= 100_800 || data[..8] != PREAMBLE {
        return None;
    }
    Some(Subframe {
        data,
        id,
        tow_count,
        inverted,
    })
}

/// The UTC parameters (subframe 4 page 18).
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct UtcParams {
    pub a0: f64,
    pub a1: f64,
    pub tot: f64,
    pub wnt: u32,
    pub dtls: i32,
    pub wnlsf: u32,
    pub dn: u32,
    pub dtlsf: i32,
}

/// The result of one subframe.
#[derive(Clone, Debug, PartialEq)]
pub enum LnavContent {
    /// Subframe 1: the 10-bit week, URA, health, IODC, TGD and the clock.
    Clock {
        wn10: u32,
        ura: u32,
        health: u32,
        iodc: u32,
        tgd: f64,
        toc: f64,
        af2: f64,
        af1: f64,
        af0: f64,
    },
    /// Subframe 2.
    Ephemeris2 {
        iode: u32,
        crs: f64,
        delta_n: f64,
        m0: f64,
        cuc: f64,
        e: f64,
        cus: f64,
        sqrt_a: f64,
        toe: f64,
        fit: u32,
    },
    /// Subframe 3.
    Ephemeris3 {
        cic: f64,
        omega0: f64,
        cis: f64,
        i0: f64,
        crc: f64,
        omega: f64,
        omega_dot: f64,
        iode: u32,
        idot: f64,
    },
    /// An almanac page (SV ID 1 to 32); the week is filled from page 25.
    Almanac(Almanac),
    /// Page 18 of subframe 4: ionosphere and UTC.
    IonoUtc(Klobuchar, UtcParams),
    /// Page 25 of subframe 5: toa and WNa (8 bits).
    AlmanacTime { toa: f64, wna: u32 },
    /// Any other page.
    Other,
}

fn p2(e: i32) -> f64 {
    2f64.powi(e)
}

/// Decodes the content of a checked subframe. `system` QZSS skips the almanac pages (their layout differs).
pub fn decode(sf: &Subframe, system: System) -> LnavContent {
    match sf.id {
        1 => LnavContent::Clock {
            wn10: sf.u(61, 10) as u32,
            ura: sf.u(73, 4) as u32,
            health: sf.u(77, 6) as u32,
            iodc: sf.u2((83, 2), (211, 8)) as u32,
            tgd: sf.s(197, 8) as f64 * p2(-31),
            toc: sf.u(219, 16) as f64 * 16.0,
            af2: sf.s(241, 8) as f64 * p2(-55),
            af1: sf.s(249, 16) as f64 * p2(-43),
            af0: sf.s(271, 22) as f64 * p2(-31),
        },
        2 => LnavContent::Ephemeris2 {
            iode: sf.u(61, 8) as u32,
            crs: sf.s(69, 16) as f64 * p2(-5),
            delta_n: sf.s(91, 16) as f64 * p2(-43) * GPS_PI,
            m0: sf.s2((107, 8), (121, 24)) as f64 * p2(-31) * GPS_PI,
            cuc: sf.s(151, 16) as f64 * p2(-29),
            e: sf.u2((167, 8), (181, 24)) as f64 * p2(-33),
            cus: sf.s(211, 16) as f64 * p2(-29),
            sqrt_a: sf.u2((227, 8), (241, 24)) as f64 * p2(-19),
            toe: sf.u(271, 16) as f64 * 16.0,
            fit: sf.u(287, 1) as u32,
        },
        3 => LnavContent::Ephemeris3 {
            cic: sf.s(61, 16) as f64 * p2(-29),
            omega0: sf.s2((77, 8), (91, 24)) as f64 * p2(-31) * GPS_PI,
            cis: sf.s(121, 16) as f64 * p2(-29),
            i0: sf.s2((137, 8), (151, 24)) as f64 * p2(-31) * GPS_PI,
            crc: sf.s(181, 16) as f64 * p2(-5),
            omega: sf.s2((197, 8), (211, 24)) as f64 * p2(-31) * GPS_PI,
            omega_dot: sf.s(241, 24) as f64 * p2(-43) * GPS_PI,
            iode: sf.u(271, 8) as u32,
            idot: sf.s(279, 14) as f64 * p2(-43) * GPS_PI,
        },
        4 | 5 if system == System::Qzss => LnavContent::Other,
        4 | 5 => {
            let sv = sf.sv_id();
            match (sf.id, sv) {
                (_, 1..=32) => LnavContent::Almanac(Almanac {
                    prn: sv as u16,
                    e: sf.u(69, 16) as f64 * p2(-21),
                    toa: sf.u(91, 8) as f64 * 4096.0,
                    delta_i: sf.s(99, 16) as f64 * p2(-19) * GPS_PI,
                    omega_dot: sf.s(121, 16) as f64 * p2(-38) * GPS_PI,
                    health: sf.u(137, 8) as u32,
                    sqrt_a: sf.u(151, 24) as f64 * p2(-11),
                    omega0: sf.s(181, 24) as f64 * p2(-23) * GPS_PI,
                    omega: sf.s(211, 24) as f64 * p2(-23) * GPS_PI,
                    m0: sf.s(241, 24) as f64 * p2(-23) * GPS_PI,
                    af0: sf.s2((271, 8), (290, 3)) as f64 * p2(-20),
                    af1: sf.s(279, 11) as f64 * p2(-38),
                    week: 0,
                }),
                (4, 56) => LnavContent::IonoUtc(
                    Klobuchar {
                        alpha: [
                            sf.s(69, 8) as f64 * p2(-30),
                            sf.s(77, 8) as f64 * p2(-27),
                            sf.s(91, 8) as f64 * p2(-24),
                            sf.s(99, 8) as f64 * p2(-24),
                        ],
                        beta: [
                            sf.s(107, 8) as f64 * p2(11),
                            sf.s(121, 8) as f64 * p2(14),
                            sf.s(129, 8) as f64 * p2(16),
                            sf.s(137, 8) as f64 * p2(16),
                        ],
                    },
                    UtcParams {
                        a1: sf.s(151, 24) as f64 * p2(-50),
                        a0: sf.s2((181, 24), (211, 8)) as f64 * p2(-30),
                        tot: sf.u(219, 8) as f64 * 4096.0,
                        wnt: sf.u(227, 8) as u32,
                        dtls: sf.s(241, 8) as i32,
                        wnlsf: sf.u(249, 8) as u32,
                        dn: sf.u(257, 8) as u32,
                        dtlsf: sf.s(271, 8) as i32,
                    },
                ),
                (5, 51) => LnavContent::AlmanacTime {
                    toa: sf.u(69, 8) as f64 * 4096.0,
                    wna: sf.u(77, 8) as u32,
                },
                _ => LnavContent::Other,
            }
        }
        _ => LnavContent::Other,
    }
}

/// Collects subframes 1 to 3 of one satellite into an ephemeris.
#[derive(Clone, Debug, Default)]
pub struct LnavCollector {
    sf1: Option<LnavContent>,
    sf2: Option<LnavContent>,
    sf3: Option<LnavContent>,
}

impl LnavCollector {
    /// Adds a decoded subframe. Returns an ephemeris when all three are held, their IODEs agree and equal the low 8 bits of IODC.
    /// `week_of` resolves the 10-bit week.
    pub fn add(
        &mut self,
        c: LnavContent,
        system: System,
        prn: u16,
        week_of: impl Fn(u32) -> i64,
    ) -> Option<Kepler> {
        match c {
            LnavContent::Clock { .. } => self.sf1 = Some(c),
            LnavContent::Ephemeris2 { .. } => self.sf2 = Some(c),
            LnavContent::Ephemeris3 { .. } => self.sf3 = Some(c),
            _ => return None,
        }
        let (
            Some(LnavContent::Clock {
                wn10,
                ura,
                health,
                iodc,
                tgd,
                toc,
                af2,
                af1,
                af0,
            }),
            Some(LnavContent::Ephemeris2 {
                iode: iode2,
                crs,
                delta_n,
                m0,
                cuc,
                e,
                cus,
                sqrt_a,
                toe,
                fit,
            }),
            Some(LnavContent::Ephemeris3 {
                cic,
                omega0,
                cis,
                i0,
                crc,
                omega,
                omega_dot,
                iode: iode3,
                idot,
            }),
        ) = (&self.sf1, &self.sf2, &self.sf3)
        else {
            return None;
        };
        if iode2 != iode3 || *iode2 != iodc & 0xFF {
            return None;
        }
        Some(Kepler {
            system: Some(system),
            prn,
            week: week_of(*wn10),
            toe: *toe,
            toc: *toc,
            sqrt_a: *sqrt_a,
            e: *e,
            m0: *m0,
            delta_n: *delta_n,
            omega0: *omega0,
            i0: *i0,
            omega: *omega,
            omega_dot: *omega_dot,
            idot: *idot,
            cuc: *cuc,
            cus: *cus,
            crc: *crc,
            crs: *crs,
            cic: *cic,
            cis: *cis,
            af0: *af0,
            af1: *af1,
            af2: *af2,
            tgd: *tgd,
            iode: *iode2,
            iodc: *iodc,
            health: *health,
            ura: *ura,
            fit: *fit,
            geo: false,
        })
    }
    /// How many of subframes 1 to 3 are held.
    pub fn parts(&self) -> usize {
        [&self.sf1, &self.sf2, &self.sf3]
            .iter()
            .filter(|s| s.is_some())
            .count()
    }
    /// Which of subframes 1 to 3 are held.
    pub fn held(&self) -> [bool; 3] {
        [self.sf1.is_some(), self.sf2.is_some(), self.sf3.is_some()]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_word(data: &[u8; 24], d29: u8, d30: u8) -> [u8; 30] {
        let mut w = [0u8; 30];
        for i in 0..24 {
            w[i] = data[i] ^ d30;
        }
        for (j, (taps, uses30)) in PARITY.iter().enumerate() {
            let mut p = if *uses30 { d30 } else { d29 };
            for &t in taps.iter() {
                p ^= data[t as usize - 1];
            }
            w[24 + j] = p;
        }
        w
    }

    #[test]
    fn word_parity() {
        let data: [u8; 24] = std::array::from_fn(|i| ((i * 5 + 1) % 3 == 0) as u8);
        for (d29, d30) in [(0, 0), (0, 1), (1, 0), (1, 1)] {
            let w = make_word(&data, d29, d30);
            assert_eq!(check_word(&w, d29, d30), Some(data));
            // An inverted word checks with inverted previous bits.
            let inv: Vec<u8> = w.iter().map(|b| b ^ 1).collect();
            assert!(check_word(&inv, d29 ^ 1, d30 ^ 1).is_some());
        }
        // Single and double errors are detected; single-bit syndromes are distinct and non-zero.
        let w = make_word(&data, 0, 0);
        let mut seen = std::collections::BTreeSet::new();
        for i in 0..30 {
            let mut e = w;
            e[i] ^= 1;
            let s = syndrome(&e, 0, 0);
            assert_ne!(s, 0);
            seen.insert(s);
            for j in i + 1..30 {
                let mut e2 = e;
                e2[j] ^= 1;
                assert!(check_word(&e2, 0, 0).is_none());
            }
        }
        assert_eq!(seen.len(), 30);
        // Linear: the syndrome of a sum of errors is the sum of syndromes.
        let mut e = w;
        e[3] ^= 1;
        e[17] ^= 1;
        let mut a = w;
        a[3] ^= 1;
        let mut b = w;
        b[17] ^= 1;
        assert_eq!(syndrome(&e, 0, 0), syndrome(&a, 0, 0) ^ syndrome(&b, 0, 0));
    }
}
