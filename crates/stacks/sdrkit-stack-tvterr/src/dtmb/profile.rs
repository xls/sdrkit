//! The DTMB transmission profiles: mapping, LDPC rate and time interleaver; the 22 system-information words; the places of the
//! system-information symbols and the frequency interleaver of C = 3780; the constellations; the rates per profile.
//!
//! Standard: GB 20600-2006 clauses 4.3 to 4.6. The words, places, interleaver and constellations come from the specification of
//! the DTMB frame decoder; no printed copy of the standard was compared with them.

use super::pn::Header;
use super::BODY;
use sdrkit_algo_dsp::constellation::{PointSet, Shape};
use sdrkit_algo_fec::digitrev::digit_reverse;
use sdrkit_core::{Choice, Complex32};
use std::sync::OnceLock;

/// System-information symbols per frame.
pub const SI: usize = 36;
/// Data symbols per frame.
pub const DATA: usize = BODY - SI;
/// Sent bits per LDPC code word.
pub const WORD: usize = 7488;
/// Branches of the time interleaver.
pub const BRANCHES: usize = 52;
/// Payload bits of a transport packet.
pub const PACKET_BITS: usize = 1504;

sdrkit_core::choice_enum! {
    /// The symbol mapping.
    pub enum Mapping {
        /// 4QAM after the Nordstrom-Robinson code.
        Qam4Nr = ("4qam-nr", "4QAM-NR"),
        /// 4QAM.
        Qam4 = ("4qam", "4QAM"),
        /// 16QAM.
        Qam16 = ("16qam", "16QAM"),
        /// 32QAM (cross).
        Qam32 = ("32qam", "32QAM"),
        /// 64QAM.
        Qam64 = ("64qam", "64QAM"),
    }
}

sdrkit_core::choice_enum! {
    /// The LDPC code rate.
    pub enum CodeRate {
        /// Rate 0.4 (k = 3048).
        R04 = ("0.4", "0.4"),
        /// Rate 0.6 (k = 4572).
        R06 = ("0.6", "0.6"),
        /// Rate 0.8 (k = 6096).
        R08 = ("0.8", "0.8"),
    }
}

sdrkit_core::choice_enum! {
    /// The time interleaver mode.
    pub enum Interleaver {
        /// Mode 1, M = 240.
        Mode1 = ("1", "mode 1 (M = 240)"),
        /// Mode 2, M = 720.
        Mode2 = ("2", "mode 2 (M = 720)"),
    }
}

sdrkit_core::choice_enum! {
    /// The carrier form of the frame body.
    pub enum Carriers {
        /// 3780 carriers (C = 3780).
        Multi = ("3780", "3780 (multi-carrier)"),
        /// Single carrier (C = 1).
        Single = ("1", "1 (single carrier)"),
    }
}

impl CodeRate {
    /// BCH blocks (and LDPC information bits / 762) per code word.
    pub fn bch_blocks(self) -> usize {
        match self {
            CodeRate::R04 => 4,
            CodeRate::R06 => 6,
            CodeRate::R08 => 8,
        }
    }
    /// Transport packets per code word.
    pub fn packets(self) -> usize {
        self.bch_blocks() / 2
    }
    /// LDPC information bits.
    pub fn k(self) -> usize {
        762 * self.bch_blocks()
    }
    /// The parity-check rows of the code.
    pub fn rows(self) -> &'static [&'static [(u16, u16)]] {
        use sdrkit_algo_fec::ldpc::tables::dtmb;
        match self {
            CodeRate::R04 => dtmb::RATE_0_4,
            CodeRate::R06 => dtmb::RATE_0_6,
            CodeRate::R08 => dtmb::RATE_0_8,
        }
    }
}

impl Interleaver {
    /// The depth `M`.
    pub fn m(self) -> usize {
        match self {
            Interleaver::Mode1 => 240,
            Interleaver::Mode2 => 720,
        }
    }
    /// Frames of delay through transmitter and receiver: 51 x 52 x M symbols over 3744 a frame.
    pub fn delay_frames(self) -> usize {
        (BRANCHES - 1) * BRANCHES * self.m() / DATA
    }
}

impl Mapping {
    /// Coded bits per data symbol (2 for 4QAM-NR, of which one is NR parity).
    pub fn bits(self) -> usize {
        match self {
            Mapping::Qam4Nr | Mapping::Qam4 => 2,
            Mapping::Qam16 => 4,
            Mapping::Qam32 => 5,
            Mapping::Qam64 => 6,
        }
    }
    /// Stream bits per frame (before the NR code for 4QAM-NR).
    pub fn stream_bits(self) -> usize {
        match self {
            Mapping::Qam4Nr => DATA,
            _ => DATA * self.bits(),
        }
    }
    /// Frames of the smallest run that holds whole code words.
    pub fn frames_per_group(self) -> usize {
        match self {
            Mapping::Qam4Nr | Mapping::Qam32 => 2,
            _ => 1,
        }
    }
    /// Code words per group.
    pub fn words_per_group(self) -> usize {
        self.stream_bits() * self.frames_per_group() / WORD
    }
}

/// A transmission profile: mapping, rate and interleaver.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Profile {
    /// The mapping.
    pub mapping: Mapping,
    /// The LDPC rate.
    pub rate: CodeRate,
    /// The time interleaver.
    pub interleaver: Interleaver,
}

/// The profiles in the order of their numbers 0 to 10.
const PROFILES: [(Mapping, CodeRate); 11] = [
    (Mapping::Qam4Nr, CodeRate::R08),
    (Mapping::Qam4, CodeRate::R04),
    (Mapping::Qam4, CodeRate::R06),
    (Mapping::Qam4, CodeRate::R08),
    (Mapping::Qam16, CodeRate::R04),
    (Mapping::Qam16, CodeRate::R06),
    (Mapping::Qam16, CodeRate::R08),
    (Mapping::Qam32, CodeRate::R08),
    (Mapping::Qam64, CodeRate::R04),
    (Mapping::Qam64, CodeRate::R06),
    (Mapping::Qam64, CodeRate::R08),
];

/// The 32-chip system-information words of indices 3 to 24, in sent order from the most significant bit (the four leading zero
/// chips are not in them).
const WORDS: [u32; 22] = [
    0x78C82ED5, 0x8737D12A, 0x77C721DA, 0x8838DE25, 0x2292748F, 0xDD6D8B70, 0x4BFB1DE6, 0xB404E219,
    0x11A147BC, 0xEE5EB843, 0x78372E2A, 0x87C8D1D5, 0x2D9D7B80, 0xD262847F, 0x77382125, 0x88C7DEDA,
    0x226D7470, 0xDD928B8F, 0x440B1216, 0xBBF4EDE9, 0x115E4743, 0xEEA1B8BC,
];

/// The first and last system-information index.
pub const SI_INDICES: std::ops::RangeInclusive<usize> = 3..=24;

impl Profile {
    /// A profile; `None` for a mapping that has no such rate (4QAM-NR and 32QAM exist at rate 0.8 only).
    pub fn new(mapping: Mapping, rate: CodeRate, interleaver: Interleaver) -> Option<Profile> {
        PROFILES.contains(&(mapping, rate)).then_some(Profile {
            mapping,
            rate,
            interleaver,
        })
    }
    /// The profile of system-information index `x` (3 to 24).
    pub fn from_index(x: usize) -> Option<Profile> {
        if !SI_INDICES.contains(&x) {
            return None;
        }
        let (mapping, rate) = PROFILES[(x - 3) / 2];
        let interleaver = if x.is_multiple_of(2) {
            Interleaver::Mode2
        } else {
            Interleaver::Mode1
        };
        Some(Profile {
            mapping,
            rate,
            interleaver,
        })
    }
    /// The system-information index: 3 + 2 x profile number + 1 for interleaver mode 2.
    pub fn index(&self) -> usize {
        let n = PROFILES
            .iter()
            .position(|p| *p == (self.mapping, self.rate))
            .unwrap_or(0);
        3 + 2 * n + usize::from(self.interleaver == Interleaver::Mode2)
    }
    /// Transport packets per frame.
    pub fn packets_per_frame(&self) -> usize {
        self.mapping.words_per_group() * self.rate.packets() / self.mapping.frames_per_group()
    }
    /// The net bit rate, bit/s, for a header and symbol rate.
    pub fn net_rate(&self, header: Header, fsym: f64) -> f64 {
        (self.packets_per_frame() * PACKET_BITS) as f64 * fsym / (header.len() + BODY) as f64
    }
    /// "16QAM, rate 0.8, interleaver mode 2".
    pub fn describe(&self) -> String {
        format!(
            "{}, rate {}, interleaver mode {}",
            self.mapping.label(),
            self.rate.key(),
            self.interleaver.key()
        )
    }
}

/// The 36 system-information chips of index `x` (+1 for chip 1, -1 for chip 0), in sent order: four zero chips then the word.
pub fn si_chips(x: usize) -> Option<[f32; SI]> {
    if !SI_INDICES.contains(&x) {
        return None;
    }
    let w = WORDS[x - 3];
    let mut c = [-1f32; SI];
    for (i, v) in c[4..].iter_mut().enumerate() {
        if w >> (31 - i) & 1 == 1 {
            *v = 1.0;
        }
    }
    Some(c)
}

/// The system-information symbol of a chip: chip 1 is (1 + j)/sqrt 2, chip 0 is (-1 - j)/sqrt 2.
pub fn si_symbol(chip: f32) -> Complex32 {
    let a = chip * std::f32::consts::FRAC_1_SQRT_2;
    Complex32::new(a, a)
}

/// The physical carriers of C = 3780: the system-information symbols and the data symbols in stream order.
#[derive(Debug)]
pub struct CarrierMap {
    /// Physical carrier of system-information symbol `i`.
    pub si: [u16; SI],
    /// Physical carrier of data symbol `i`.
    pub data: Vec<u16>,
}

/// Logical position of system-information symbol `4 g + i`: `420 g` + 0, 140, 279, 419.
pub fn si_logical(n: usize) -> usize {
    420 * (n / 4) + [0, 140, 279, 419][n % 4]
}

/// The carrier map, built once: the logical order (system information at [`si_logical`], data in the rest) through the
/// frequency interleaver, the mixed-radix digit reversal of radices 3, 3, 3, 2, 2, 5, 7.
pub fn carriers() -> &'static CarrierMap {
    static MAP: OnceLock<CarrierMap> = OnceLock::new();
    MAP.get_or_init(|| {
        let p = digit_reverse(&[3, 3, 3, 2, 2, 5, 7])
            .unwrap_or_else(|e| unreachable!("the radices are fixed: {e}"));
        let phys = |l: usize| p.dest(l).unwrap_or(0) as u16;
        let mut si = [0u16; SI];
        let mut is_si = [false; BODY];
        for (n, s) in si.iter_mut().enumerate() {
            let l = si_logical(n);
            is_si[l] = true;
            *s = phys(l);
        }
        let data = (0..BODY).filter(|&l| !is_si[l]).map(phys).collect();
        CarrierMap { si, data }
    })
}

/// The 32QAM points of label `b0 + 2 b1 + 4 b2 + 8 b3 + 16 b4` (units of 1 / sqrt 20).
const QAM32: [(i8, i8); 32] = [
    (-1, -1),
    (-5, -3),
    (1, -1),
    (5, -3),
    (-3, -1),
    (-5, -1),
    (3, -1),
    (5, -1),
    (-1, 1),
    (-5, 3),
    (1, 1),
    (5, 3),
    (-3, 1),
    (-5, 1),
    (3, 1),
    (5, 1),
    (-1, -3),
    (-1, -5),
    (1, -3),
    (1, -5),
    (-3, -3),
    (-3, -5),
    (3, -3),
    (3, -5),
    (-1, 3),
    (-1, 5),
    (1, 3),
    (1, 5),
    (-3, 3),
    (-3, 5),
    (3, 3),
    (3, 5),
];

/// The level of one axis of 4, 16 or 64QAM from its bits in sent order: a reflected Gray code with the first bit least
/// significant, level 0 the most negative.
fn axis_level(bits: &[u8]) -> f32 {
    // The Gray label with the first bit as its least significant bit, decoded to the level number.
    let gray = bits
        .iter()
        .enumerate()
        .fold(0usize, |a, (i, &b)| a | ((b as usize & 1) << i));
    let mut level = gray;
    let mut s = gray >> 1;
    while s != 0 {
        level ^= s;
        s >>= 1;
    }
    let m = 1usize << bits.len();
    (2 * level) as f32 - (m - 1) as f32
}

/// The point of the `q` bits of one symbol (sent order) of `mapping`, at unit mean power. 4QAM-NR maps as 4QAM.
pub fn map_symbol(mapping: Mapping, bits: &[u8]) -> Complex32 {
    match mapping {
        Mapping::Qam32 => {
            let l = bits
                .iter()
                .take(5)
                .enumerate()
                .fold(0usize, |a, (i, &b)| a | ((b as usize & 1) << i));
            let (i, q) = QAM32[l];
            Complex32::new(i as f32, q as f32) / 20f32.sqrt()
        }
        _ => {
            let h = mapping.bits() / 2;
            let scale = match mapping {
                Mapping::Qam16 => 10f32,
                Mapping::Qam64 => 42.0,
                _ => 2.0,
            }
            .sqrt();
            Complex32::new(axis_level(&bits[..h]), axis_level(&bits[h..2 * h])) / scale
        }
    }
}

/// The constellation of `mapping` as a point set whose label, read from the most significant bit, is the bits in sent order.
pub fn point_set(mapping: Mapping) -> PointSet {
    let q = mapping.bits();
    let mut bits = vec![0u8; q];
    let points = (0..1usize << q)
        .map(|label| {
            for (k, b) in bits.iter_mut().enumerate() {
                *b = (label >> (q - 1 - k) & 1) as u8;
            }
            map_symbol(mapping, &bits)
        })
        .collect();
    let shape = match mapping {
        Mapping::Qam32 => Shape::CrossQam,
        _ => Shape::SquareQam { side: 1 << (q / 2) },
    };
    PointSet::new(points, shape).unwrap_or_else(|e| unreachable!("fixed points: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn system_information_words() {
        for x in SI_INDICES {
            let p = Profile::from_index(x).expect("a profile");
            assert_eq!(p.index(), x);
            let c = si_chips(x).expect("a word");
            assert_eq!(&c[..4], &[-1.0; 4], "four leading zero chips");
        }
        assert!(Profile::from_index(2).is_none() && Profile::from_index(25).is_none());
        assert!(si_chips(2).is_none() && si_chips(25).is_none());
        assert!(Profile::new(Mapping::Qam4Nr, CodeRate::R04, Interleaver::Mode1).is_none());
        assert!(Profile::new(Mapping::Qam32, CodeRate::R06, Interleaver::Mode1).is_none());
        // Words of different profiles differ in 16 chips, the two words of a profile in all 32.
        for a in SI_INDICES {
            for b in SI_INDICES {
                if a >= b {
                    continue;
                }
                let d = (WORDS[a - 3] ^ WORDS[b - 3]).count_ones();
                let same = (a - 3) / 2 == (b - 3) / 2;
                assert_eq!(d, if same { 32 } else { 16 }, "{a} {b}");
            }
        }
        assert_eq!(WORDS[10 - 3], 0b1011_0100_0000_0100_1110_0010_0001_1001);
        assert_eq!(WORDS[16 - 3], 0b1101_0010_0110_0010_1000_0100_0111_1111);
        let p = Profile::from_index(16).unwrap();
        assert_eq!(p.describe(), "16QAM, rate 0.8, interleaver mode 2");
    }

    #[test]
    fn carrier_places_and_rates() {
        let m = carriers();
        let logical: Vec<usize> = (0..SI).map(si_logical).collect();
        assert_eq!(&logical[..5], &[0, 140, 279, 419, 420]);
        assert_eq!(logical[35], 3779);
        assert_eq!(
            m.si.to_vec(),
            [
                0, 9, 3762, 3771, 3, 12, 3765, 3774, 6, 15, 3768, 3777, 1, 10, 3763, 3772, 4, 13,
                3766, 3775, 7, 16, 3769, 3778, 2, 11, 3764, 3773, 5, 14, 3767, 3776, 8, 17, 3770,
                3779
            ]
        );
        assert_eq!(
            &m.data[..20],
            &[
                540, 1080, 1620, 2160, 2700, 3240, 108, 648, 1188, 1728, 2268, 2808, 3348, 216,
                756, 1296, 1836, 2376, 2916, 3456
            ]
        );
        let mut all: Vec<u16> = m.si.iter().chain(&m.data).copied().collect();
        all.sort_unstable();
        assert!(
            all.iter().enumerate().all(|(i, &v)| v as usize == i),
            "a permutation"
        );
        let rate = |m, r, h, fsym| {
            Profile::new(m, r, Interleaver::Mode1)
                .unwrap()
                .net_rate(h, fsym)
        };
        assert!((rate(Mapping::Qam4, CodeRate::R04, Header::Pn945, 7.56e6) - 4.8128e6).abs() < 1e3);
        assert!(
            (rate(Mapping::Qam64, CodeRate::R08, Header::Pn420, 7.56e6) - 32.4864e6).abs() < 1e3
        );
        assert!(
            (rate(Mapping::Qam16, CodeRate::R06, Header::Pn420, 7.56e6) - 16.2432e6).abs() < 1e3
        );
        let ppf: Vec<usize> = PROFILES
            .iter()
            .map(|&(m, r)| {
                Profile::new(m, r, Interleaver::Mode1)
                    .unwrap()
                    .packets_per_frame()
            })
            .collect();
        assert_eq!(ppf, [2, 2, 3, 4, 4, 6, 8, 10, 6, 9, 12]);
        assert_eq!(
            (
                Interleaver::Mode1.delay_frames(),
                Interleaver::Mode2.delay_frames()
            ),
            (170, 510)
        );
    }

    #[test]
    fn constellations() {
        let bits = |l: usize, q: usize| (0..q).map(|k| (l >> k & 1) as u8).collect::<Vec<u8>>();
        // 4QAM and 16QAM labels of the specification (label = b0 + 2 b1 + ...).
        let q4: Vec<Complex32> = (0..4)
            .map(|l| map_symbol(Mapping::Qam4, &bits(l, 2)) * 2f32.sqrt())
            .collect();
        let want4 = [(-1.0, -1.0), (1.0, -1.0), (-1.0, 1.0), (1.0, 1.0)];
        for (p, w) in q4.iter().zip(want4) {
            assert!((p.re - w.0).abs() < 1e-5 && (p.im - w.1).abs() < 1e-5);
        }
        let i16: Vec<i32> = (0..4)
            .map(|l| (map_symbol(Mapping::Qam16, &bits(l, 4)).re * 10f32.sqrt()).round() as i32)
            .collect();
        assert_eq!(i16, [-3, -1, 3, 1]);
        let i64: Vec<i32> = (0..8)
            .map(|l| (map_symbol(Mapping::Qam64, &bits(l, 6)).re * 42f32.sqrt()).round() as i32)
            .collect();
        assert_eq!(i64, [-7, -5, -1, -3, 7, 5, 1, 3]);
        for m in [
            Mapping::Qam4,
            Mapping::Qam16,
            Mapping::Qam32,
            Mapping::Qam64,
        ] {
            let q = m.bits();
            let pts: Vec<Complex32> = (0..1usize << q)
                .map(|l| map_symbol(m, &bits(l, q)))
                .collect();
            let e = pts.iter().map(|p| p.norm_sqr()).sum::<f32>() / pts.len() as f32;
            assert!((e - 1.0).abs() < 1e-5, "{m:?} power {e}");
            for a in 0..pts.len() {
                for b in a + 1..pts.len() {
                    let d = (pts[a] - pts[b]).norm();
                    assert!(d > 0.1, "{m:?} points distinct");
                    // Neighbours along an axis differ in one bit (square mappings).
                    if m != Mapping::Qam32 && d < 1.01 * min_step(m) {
                        assert_eq!((a ^ b).count_ones(), 1, "{m:?} {a} {b}");
                    }
                }
            }
            if m == Mapping::Qam32 {
                assert!(pts
                    .iter()
                    .all(|p| (p.re.abs() * 20f32.sqrt() - 5.0).abs() > 0.1
                        || (p.im.abs() * 20f32.sqrt() - 5.0).abs() > 0.1));
            }
            assert_eq!(point_set(m).points().len(), 1 << q);
        }
    }

    fn min_step(m: Mapping) -> f32 {
        match m {
            Mapping::Qam16 => 2.0 / 10f32.sqrt(),
            Mapping::Qam64 => 2.0 / 42f32.sqrt(),
            _ => 2.0 / 2f32.sqrt(),
        }
    }
}
