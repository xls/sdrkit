//! Galileo I/NAV on E1-B (OS SIS ICD 4.1.4, 4.3, 5.1): the page part (sync pattern, interleaver, convolutional code), the page
//! (even and odd parts, CRC-24Q), and the word types that make the ephemeris, the clock and the GST-GPS offset.

use crate::fec::{SoftViterbi, GALILEO_K7};
use crate::orbit::Kepler;
use crate::system::{System, GPS_PI};
use sdrkit_algo_fec::conv::Ends;
use sdrkit_util_bits::crc::catalog::CRC24_LTE_A;
use sdrkit_util_bits::interleave::{Permutation, ReadOrder};
use sdrkit_util_bits::HardBits;
use std::sync::OnceLock;

/// The 10-symbol sync pattern that opens a page part.
pub const SYNC: [u8; 10] = [0, 1, 0, 1, 1, 0, 0, 0, 0, 0];
/// Symbols in a page part.
pub const PART_SYMBOLS: usize = 250;
/// Interleaver columns and rows.
pub const COLS: usize = 30;
pub const ROWS: usize = 8;

/// The page part interleaver (OS SIS ICD clause 4.1.4.2): 240 symbols written in 30 columns of 8 and read in 8 rows of 30.
pub fn interleaver() -> &'static Permutation {
    static P: OnceLock<Permutation> = OnceLock::new();
    // Input symbol c * 8 + r (column c, row r) goes out at r * 30 + c.
    P.get_or_init(|| {
        Permutation::row_column(COLS, ROWS, ReadOrder::Columns).expect("a 30 x 8 matrix")
    })
}

/// Decodes the 240 soft coded symbols of a page part (after the sync pattern) into its 120 bits.
pub struct PartDecoder {
    viterbi: SoftViterbi,
    deint: Vec<f32>,
    bits: Vec<u8>,
}

impl Default for PartDecoder {
    fn default() -> Self {
        PartDecoder {
            viterbi: SoftViterbi::new(GALILEO_K7, 120),
            deint: vec![0.0; 240],
            bits: Vec::with_capacity(120),
        }
    }
}

impl PartDecoder {
    /// Decodes; the soft symbols are positive for a 0 symbol.
    pub fn decode(&mut self, coded: &[f32]) -> &[u8] {
        // Both slices hold 240 symbols, so the permutation cannot fail.
        let _ = interleaver().invert(&coded[..240], &mut self.deint);
        self.viterbi.decode(&self.deint, Ends::Zero, &mut self.bits);
        &self.bits
    }
}

/// A checked I/NAV page: the 128-bit word and its type.
#[derive(Clone, Debug, PartialEq)]
pub struct Page {
    pub word: Vec<u8>,
    pub word_type: u8,
}

/// Puts an even and an odd part together and checks the CRC. Returns `None` if they are not an even then an odd part, or the
/// CRC fails.
pub fn assemble(even: &[u8], odd: &[u8]) -> Option<Page> {
    if even.len() < 120 || odd.len() < 120 || even[0] != 0 || odd[0] != 1 {
        return None;
    }
    let mut crc_in = Vec::with_capacity(196);
    crc_in.extend_from_slice(&even[..114]);
    crc_in.extend_from_slice(&odd[..82]);
    let crc = HardBits::at(odd, 82).u(24) as u32;
    if CRC24_LTE_A.bits(&crc_in) != crc {
        return None;
    }
    let mut word = Vec::with_capacity(128);
    word.extend_from_slice(&even[2..114]);
    word.extend_from_slice(&odd[2..18]);
    let word_type = HardBits::at(&word, 0).u(6) as u8;
    Some(Page { word, word_type })
}

/// The result of one word.
#[derive(Clone, Debug, PartialEq)]
pub enum InavWord {
    W1 {
        iod: u32,
        toe: f64,
        m0: f64,
        e: f64,
        sqrt_a: f64,
    },
    W2 {
        iod: u32,
        omega0: f64,
        i0: f64,
        omega: f64,
        idot: f64,
    },
    W3 {
        iod: u32,
        omega_dot: f64,
        delta_n: f64,
        cuc: f64,
        cus: f64,
        crc: f64,
        crs: f64,
        sisa: u32,
    },
    W4 {
        iod: u32,
        svid: u32,
        cic: f64,
        cis: f64,
        toc: f64,
        af0: f64,
        af1: f64,
        af2: f64,
    },
    W5 {
        ai: [f64; 3],
        bgd_e5a: f64,
        bgd_e5b: f64,
        e5b_hs: u32,
        e1b_hs: u32,
        e5b_dvs: u32,
        e1b_dvs: u32,
        wn: u32,
        tow: u32,
    },
    W6 {
        a0: f64,
        a1: f64,
        dtls: i32,
        tot: f64,
        wnot: u32,
        wnlsf: u32,
        dn: u32,
        dtlsf: i32,
        tow: u32,
    },
    /// Word 10: the GST-GPS offset (`None` when all ones: not available).
    W10 {
        ggto: Option<Ggto>,
    },
    /// Word 0 with the time field 2: week and TOW.
    W0 {
        wn: u32,
        tow: u32,
    },
    Other(u8),
}

/// The GST-GPS time offset parameters.
#[derive(Clone, Copy, Debug, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Ggto {
    pub a0g: f64,
    pub a1g: f64,
    pub t0g: f64,
    pub wn0g: u32,
}

impl Ggto {
    /// GST − GPS (s) at GPS time `t` of GPS week `week` (the 6-bit week difference taken within ±31).
    pub fn offset(&self, week: i64, t: f64) -> f64 {
        let mut dwn = (week - self.wn0g as i64).rem_euclid(64);
        if dwn > 31 {
            dwn -= 64;
        }
        self.a0g + self.a1g * (t - self.t0g + 604_800.0 * dwn as f64)
    }
}

fn p2(e: i32) -> f64 {
    2f64.powi(e)
}

/// Decodes a word.
pub fn decode_word(w: &[u8]) -> InavWord {
    let t = HardBits::at(w, 0).u(6) as u8;
    let u = |s: usize, l: u32| HardBits::at(w, s).u(l);
    let s = |st: usize, l: u32| HardBits::at(w, st).i(l);
    match t {
        1 => InavWord::W1 {
            iod: u(6, 10) as u32,
            toe: u(16, 14) as f64 * 60.0,
            m0: s(30, 32) as f64 * p2(-31) * GPS_PI,
            e: u(62, 32) as f64 * p2(-33),
            sqrt_a: u(94, 32) as f64 * p2(-19),
        },
        2 => InavWord::W2 {
            iod: u(6, 10) as u32,
            omega0: s(16, 32) as f64 * p2(-31) * GPS_PI,
            i0: s(48, 32) as f64 * p2(-31) * GPS_PI,
            omega: s(80, 32) as f64 * p2(-31) * GPS_PI,
            idot: s(112, 14) as f64 * p2(-43) * GPS_PI,
        },
        3 => InavWord::W3 {
            iod: u(6, 10) as u32,
            omega_dot: s(16, 24) as f64 * p2(-43) * GPS_PI,
            delta_n: s(40, 16) as f64 * p2(-43) * GPS_PI,
            cuc: s(56, 16) as f64 * p2(-29),
            cus: s(72, 16) as f64 * p2(-29),
            crc: s(88, 16) as f64 * p2(-5),
            crs: s(104, 16) as f64 * p2(-5),
            sisa: u(120, 8) as u32,
        },
        4 => InavWord::W4 {
            iod: u(6, 10) as u32,
            svid: u(16, 6) as u32,
            cic: s(22, 16) as f64 * p2(-29),
            cis: s(38, 16) as f64 * p2(-29),
            toc: u(54, 14) as f64 * 60.0,
            af0: s(68, 31) as f64 * p2(-34),
            af1: s(99, 21) as f64 * p2(-46),
            af2: s(120, 6) as f64 * p2(-59),
        },
        5 => InavWord::W5 {
            ai: [
                u(6, 11) as f64 * p2(-2),
                s(17, 11) as f64 * p2(-8),
                s(28, 14) as f64 * p2(-15),
            ],
            bgd_e5a: s(47, 10) as f64 * p2(-32),
            bgd_e5b: s(57, 10) as f64 * p2(-32),
            e5b_hs: u(67, 2) as u32,
            e1b_hs: u(69, 2) as u32,
            e5b_dvs: u(71, 1) as u32,
            e1b_dvs: u(72, 1) as u32,
            wn: u(73, 12) as u32,
            tow: u(85, 20) as u32,
        },
        6 => InavWord::W6 {
            a0: s(6, 32) as f64 * p2(-30),
            a1: s(38, 24) as f64 * p2(-50),
            dtls: s(62, 8) as i32,
            tot: u(70, 8) as f64 * 3600.0,
            wnot: u(78, 8) as u32,
            wnlsf: u(86, 8) as u32,
            dn: u(94, 3) as u32,
            dtlsf: s(97, 8) as i32,
            tow: u(105, 20) as u32,
        },
        10 => {
            let a0 = u(86, 16);
            let a1 = u(102, 12);
            let t0 = u(114, 8);
            let wn = u(122, 6);
            let na = a0 == 0xFFFF && a1 == 0xFFF && t0 == 0xFF && wn == 0x3F;
            InavWord::W10 {
                ggto: (!na).then(|| Ggto {
                    a0g: s(86, 16) as f64 * p2(-35),
                    a1g: s(102, 12) as f64 * p2(-51),
                    t0g: t0 as f64 * 3600.0,
                    wn0g: wn as u32,
                }),
            }
        }
        0 if u(6, 2) == 2 => InavWord::W0 {
            wn: u(96, 12) as u32,
            tow: u(108, 20) as u32,
        },
        t => InavWord::Other(t),
    }
}

/// Collects words 1 to 5 of one satellite.
#[derive(Clone, Debug, Default)]
pub struct InavCollector {
    w: [Option<InavWord>; 5],
    pub week: Option<u32>,
}

impl InavCollector {
    /// Parts held of words 1 to 4 for the newest IODnav (0 to 4).
    pub fn parts(&self) -> usize {
        self.w[..4].iter().filter(|w| w.is_some()).count()
    }
    /// Adds a word; returns an ephemeris when words 1 to 4 of one IODnav and word 5 are held. `full_week` maps the GST week.
    pub fn add(&mut self, w: InavWord, prn: u16, full_week: impl Fn(u32) -> i64) -> Option<Kepler> {
        let iod_of = |w: &InavWord| match w {
            InavWord::W1 { iod, .. }
            | InavWord::W2 { iod, .. }
            | InavWord::W3 { iod, .. }
            | InavWord::W4 { iod, .. } => Some(*iod),
            _ => None,
        };
        let slot = match &w {
            InavWord::W1 { .. } => 0,
            InavWord::W2 { .. } => 1,
            InavWord::W3 { .. } => 2,
            InavWord::W4 { .. } => 3,
            InavWord::W5 { wn, .. } => {
                self.week = Some(*wn);
                4
            }
            InavWord::W0 { wn, .. } => {
                self.week = Some(*wn);
                return None;
            }
            _ => return None,
        };
        if let Some(iod) = iod_of(&w) {
            // A new IODnav clears the words of the old one.
            for k in 0..4 {
                if let Some(o) = &self.w[k] {
                    if iod_of(o) != Some(iod) {
                        self.w[k] = None;
                    }
                }
            }
        }
        self.w[slot] = Some(w);
        let [Some(InavWord::W1 {
            iod,
            toe,
            m0,
            e,
            sqrt_a,
        }), Some(InavWord::W2 {
            omega0,
            i0,
            omega,
            idot,
            ..
        }), Some(InavWord::W3 {
            omega_dot,
            delta_n,
            cuc,
            cus,
            crc,
            crs,
            sisa,
            ..
        }), Some(InavWord::W4 {
            cic,
            cis,
            toc,
            af0,
            af1,
            af2,
            ..
        }), Some(InavWord::W5 {
            bgd_e5b,
            e1b_hs,
            e1b_dvs,
            ..
        })] = &self.w
        else {
            return None;
        };
        let wn = self.week?;
        Some(Kepler {
            system: Some(System::Galileo),
            prn,
            week: full_week(wn),
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
            tgd: *bgd_e5b,
            iode: *iod,
            iodc: *iod,
            health: e1b_hs | e1b_dvs,
            ura: *sisa,
            fit: 0,
            geo: false,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn interleaver_round_trip() {
        let a: Vec<u16> = (0..240).collect();
        let mut b = vec![0u16; 240];
        let mut c = vec![0u16; 240];
        interleaver().apply(&a, &mut b).unwrap();
        interleaver().invert(&b, &mut c).unwrap();
        assert_eq!(a, c);
        // OS SIS ICD clause 4.1.4.2: the second symbol sent is the first symbol of the second column.
        assert_eq!(b[1], 8);
        assert_eq!(b[30], 1);
    }

    #[test]
    fn ggto_week_difference_wraps() {
        let g = Ggto {
            a0g: 25e-9,
            a1g: 0.0,
            t0g: 0.0,
            wn0g: 63,
        };
        assert!((g.offset(2431, 0.0) - 25e-9).abs() < 1e-15);
        assert!((g.offset(2432, 0.0) - 25e-9).abs() < 1e-15);
    }
}
