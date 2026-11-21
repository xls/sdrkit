//! The format of the TDM form SH-B (ETSI EN 302 583 clauses 5.5.2.4 and 5.6): the physical layer slot with its two pilot
//! fields and its scrambling, the three constellations, the SH frame with its header (the SOF and the signalling field),
//! the frame widths of table 5.12 and the symbol rates of table 5.13.

use super::coding::{CU_BITS, IU_BITS, SIGNALLING_BITS};
use super::ofdm::Width;
use super::shframe::FrameShape;
use crate::params::Guard;
use sdrkit_algo_dsp::constellation::PointSet;
use sdrkit_algo_fec::branchdelay::ShTimeInterleaver;
use sdrkit_algo_fec::turbo_codes::sh_pattern;
use sdrkit_core::{Choice, Complex32};
use sdrkit_util_bits::crc::catalog::CRC16_IBM_3740;

/// Symbols of a physical layer slot.
pub const SLOT: usize = 2176;
/// Symbols of a pilot field.
pub const PILOTS: usize = 80;
/// Symbols of a sub-slot.
pub const SUB_SLOT: usize = 1008;
/// The first symbol of the second pilot field.
pub const SECOND_FIELD: usize = PILOTS + SUB_SLOT;
/// Data symbols of a slot.
pub const SLOT_DATA: usize = 2 * SUB_SLOT;
/// A pilot symbol before the scrambling.
pub const PILOT: Complex32 = Complex32::new(
    std::f32::consts::FRAC_1_SQRT_2,
    std::f32::consts::FRAC_1_SQRT_2,
);
/// Bits of the SOF.
pub const SOF_BITS: usize = 288;
/// Bits of the coded signalling field.
pub const SIGNALLING_CODED: usize = 5760;
/// Bits of the header of an SH frame: 3 capacity units.
pub const HEADER_BITS: usize = SOF_BITS + SIGNALLING_CODED;
/// Capacity units of the header.
pub const HEADER_CU: usize = 3;
/// The SOF, first bit in the most significant place of the first word.
pub const SOF: [u32; 9] = [
    0xF348_4536,
    0xB855_DF1B,
    0x6FD3_2468,
    0xF368_BC5A,
    0x6CD0_2627,
    0x074C_B0A4,
    0x1197_9705,
    0x08F3_1EDD,
    0xACCF_9E4F,
];
/// The highest code number of the slot scrambling.
pub const GOLD_MAX: u32 = 262_141;

sdrkit_core::choice_enum! {
    /// The modulation of the TDM form.
    #[derive(Default)]
    pub enum Modulation {
        /// QPSK.
        #[default]
        Qpsk = ("qpsk", "QPSK"),
        /// 8PSK.
        Psk8 = ("8psk", "8PSK"),
        /// 16APSK.
        Apsk16 = ("16apsk", "16APSK"),
    }
}

impl Modulation {
    /// Every modulation.
    pub const ALL: [Modulation; 3] = [Modulation::Qpsk, Modulation::Psk8, Modulation::Apsk16];

    /// Bits per symbol, which is also the capacity units of a slot (table 5.14).
    pub fn bits(self) -> usize {
        match self {
            Modulation::Qpsk => 2,
            Modulation::Psk8 => 3,
            Modulation::Apsk16 => 4,
        }
    }

    /// The points by label at unit mean energy (clause 5.6.2): the QPSK and 8PSK of EN 302 307, and 16APSK with rings
    /// of 4 and 12 points and the ring ratio 3.
    pub fn point_set(self) -> PointSet {
        match self {
            Modulation::Qpsk => PointSet::qpsk(),
            Modulation::Psk8 => PointSet::dvbs2_8psk(),
            // The ratio 3 is above 1: the constructor does not refuse it.
            Modulation::Apsk16 => PointSet::dvbs2_16apsk(3.0).unwrap_or_else(|_| PointSet::qpsk()),
        }
    }
}

sdrkit_core::choice_enum! {
    /// The roll-off of the root raised cosine.
    #[derive(Default)]
    pub enum RollOff {
        /// 0.15.
        R15 = ("0.15", "0.15"),
        /// 0.25.
        #[default]
        R25 = ("0.25", "0.25"),
        /// 0.35.
        R35 = ("0.35", "0.35"),
    }
}

impl RollOff {
    /// Every roll-off.
    pub const ALL: [RollOff; 3] = [RollOff::R15, RollOff::R25, RollOff::R35];

    /// The factor.
    pub fn value(self) -> f64 {
        match self {
            RollOff::R15 => 0.15,
            RollOff::R25 => 0.25,
            RollOff::R35 => 0.35,
        }
    }
}

fn guard_row(g: Guard) -> usize {
    match g {
        Guard::G4 => 0,
        Guard::G8 => 1,
        Guard::G16 => 2,
        Guard::G32 => 3,
    }
}

/// The width of the TDM SH frame in capacity units when the TDM and the OFDM form have the same bandwidth (table 5.12):
/// the frame lasts one OFDM SH frame of guard `guard` and of QPSK, or of 16-QAM with `ofdm_qam16`.
pub fn frame_width_cu(guard: Guard, rolloff: RollOff, ofdm_qam16: bool, m: Modulation) -> usize {
    // The column of OFDM QPSK and TDM QPSK; the rows are the guards 1/4, 1/8, 1/16, 1/32.
    const QPSK_QPSK: [[usize; 3]; 4] = [
        [952, 896, 812],
        [868, 784, 728],
        [812, 756, 700],
        [784, 728, 672],
    ];
    // The other columns hold the same number of slots (QPSK) or half of it (16-QAM), a slot being 2, 3 or 4 units.
    let slots = QPSK_QPSK[guard_row(guard)][rolloff.index() as usize] / 2;
    slots * m.bits() / if ofdm_qam16 { 2 } else { 1 }
}

/// The TDM symbol rate in symbols per second when the TDM and the OFDM form have the same bandwidth (table 5.13).
pub fn symbol_rate(width: Width, guard: Guard, rolloff: RollOff) -> f64 {
    // The rows of the 8 MHz channel as fractions of 1 MHz.
    const W8: [[(f64, f64); 3]; 4] = [
        [(34.0, 5.0), (32.0, 5.0), (29.0, 5.0)],
        [(62.0, 9.0), (56.0, 9.0), (52.0, 9.0)],
        [(116.0, 17.0), (108.0, 17.0), (100.0, 17.0)],
        [(224.0, 33.0), (208.0, 33.0), (64.0, 11.0)],
    ];
    let (n, d) = W8[guard_row(guard)][rolloff.index() as usize];
    // The rows of the other widths are those of 8 MHz in the ratio of the OFDM rates.
    n * 1e6 / d * width.ofdm_rate() / Width::W8.ofdm_rate()
}

/// The 288 bits of the SOF, in the order sent.
pub fn sof_bits() -> [u8; SOF_BITS] {
    let mut b = [0u8; SOF_BITS];
    for (i, o) in b.iter_mut().enumerate() {
        *o = ((SOF[i / 32] >> (31 - i % 32)) & 1) as u8;
    }
    b
}

/// The scrambling of a slot with code `n` (clause 5.6.4): the quarter turns of each of its 2176 symbols, the sequence of
/// EN 302 307 from its start. A code above [`GOLD_MAX`] gives code 0.
pub fn scrambling(n: u32) -> Vec<u8> {
    let mut v = vec![0u8; SLOT];
    if sdrkit_util_bits::seq::gold_dvbs2(n, 0, &mut v).is_err() {
        // Code 0 is valid.
        let _ = sdrkit_util_bits::seq::gold_dvbs2(0, 0, &mut v);
    }
    v
}

/// `s` turned by `r` quarter turns.
#[inline]
pub fn quarter(s: Complex32, r: u8) -> Complex32 {
    match r & 3 {
        0 => s,
        1 => Complex32::new(-s.im, s.re),
        2 => -s,
        _ => Complex32::new(s.im, -s.re),
    }
}

/// The scrambled pilot symbols of the two fields of a slot with scrambling `scr` ([`scrambling`]).
pub fn pilot_fields(scr: &[u8]) -> [[Complex32; PILOTS]; 2] {
    let mut f = [[Complex32::default(); PILOTS]; 2];
    for (k, field) in f.iter_mut().enumerate() {
        for (i, p) in field.iter_mut().enumerate() {
            *p = quarter(PILOT, scr[k * SECOND_FIELD + i]);
        }
    }
    f
}

/// The place in the slot of data symbol `d` (0 to 2015).
#[inline]
pub fn data_place(d: usize) -> usize {
    if d < SUB_SLOT {
        PILOTS + d
    } else {
        2 * PILOTS + d
    }
}

/// The coded bits N_BIL of a code word of Punct_Pat_ID `id` (table 5.7); `None` for a reserved id.
pub fn n_bil(id: u8) -> Option<usize> {
    let n_tcb = match sh_pattern(id)?.rate {
        (1, 5) => 61_440,
        (2, 9) => 55_296,
        (1, 4) => 49_152,
        (2, 7) => 43_008,
        (1, 3) => 36_864,
        (2, 5) => 30_720,
        (1, 2) => 24_576,
        (2, 3) => 18_432,
        _ => return None,
    };
    Some(n_tcb / 128 * 126)
}

/// The layout of a TDM SH frame.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TdmFrame {
    /// The width in capacity units.
    pub width_cu: usize,
    /// The modulation.
    pub modulation: Modulation,
    /// Slots of the frame.
    pub slots: usize,
    /// The DATA part: whole code words, all of which pass the time interleaver.
    pub shape: FrameShape,
}

impl TdmFrame {
    /// The frame of `width_cu` units in `modulation` with code words of pattern `id` and the interleaver `ti`: the DATA
    /// part holds the most whole code words that fit after the header, and the unit of the slice offset is the DATA part
    /// in interleaving units over 48. `None` for a reserved pattern, a width that is not a whole number of slots or a
    /// width with no room for a code word.
    pub fn new(width_cu: usize, modulation: Modulation, id: u8, ti: ShTimeInterleaver) -> Option<Self> {
        let n_bil = n_bil(id)?;
        let ipc = n_bil / IU_BITS;
        if width_cu <= HEADER_CU || !width_cu.is_multiple_of(modulation.bits()) {
            return None;
        }
        let n_cw = (width_cu - HEADER_CU) * (CU_BITS / IU_BITS) / ipc;
        if n_cw == 0 {
            return None;
        }
        let data_iu = n_cw * ipc;
        Some(TdmFrame {
            width_cu,
            modulation,
            slots: width_cu / modulation.bits(),
            shape: FrameShape::new(data_iu, n_bil, ti, (data_iu / 48) as u32),
        })
    }

    /// Bits of the frame.
    pub fn bits(&self) -> usize {
        self.width_cu * CU_BITS
    }

    /// Bits of the DATA part.
    pub fn data_bits(&self) -> usize {
        self.shape.data_iu() * IU_BITS
    }

    /// Bits of the PADDING part.
    pub fn padding_bits(&self) -> usize {
        self.bits() - HEADER_BITS - self.data_bits()
    }
}

fn put(bits: &mut [u8], at: usize, n: usize, v: u64) {
    for k in 0..n {
        bits[at + k] = ((v >> (n - 1 - k)) & 1) as u8;
    }
}

fn get(bits: &[u8], at: usize, n: usize) -> u64 {
    bits[at..at + n]
        .iter()
        .fold(0u64, |a, &b| (a << 1) | (b & 1) as u64)
}

/// The signalling field of the TDM form (table 5.9), without the description of the low-latency bursts.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Signalling {
    /// Frame_Width_CUs.
    pub width_cu: u16,
    /// Punct_Pat_ID.
    pub id: u8,
    /// The time interleaver profile.
    pub ti: ShTimeInterleaver,
    /// LL_par_present with a good second CRC: the frame carries a low-latency multiplex.
    pub low_latency: bool,
}

impl Signalling {
    /// Writes the 1146 bits: version 0, both CRCs, the low-latency part and the reserved bits zero.
    pub fn write(&self, bits: &mut [u8]) {
        bits[..SIGNALLING_BITS].fill(0);
        put(bits, 16, 12, self.width_cu as u64);
        put(bits, 28, 4, self.id as u64);
        put(bits, 32, 6, self.ti.common_multiplier as u64);
        put(bits, 38, 6, self.ti.late_taps as u64);
        put(bits, 44, 6, self.ti.slices as u64);
        put(bits, 50, 8, self.ti.slice_distance as u64);
        put(bits, 58, 6, self.ti.non_late_increment as u64);
        let crc = CRC16_IBM_3740.bits(&bits[..96]);
        put(bits, 96, 16, u64::from(crc));
        let crc = CRC16_IBM_3740.bits(&bits[112..384]);
        put(bits, 384, 16, u64::from(crc));
    }

    /// Whether the first CRC of the 1146 bits holds.
    pub fn crc_holds(bits: &[u8]) -> bool {
        u64::from(CRC16_IBM_3740.bits(&bits[..96])) == get(bits, 96, 16)
    }

    /// Reads the 1146 bits. `None` when the first CRC fails, the version is not 0 or the reserved byte after it is
    /// not 0.
    pub fn read(bits: &[u8]) -> Option<Self> {
        if !Self::crc_holds(bits) || get(bits, 0, 16) != 0 {
            return None;
        }
        let second = u64::from(CRC16_IBM_3740.bits(&bits[112..384])) == get(bits, 384, 16);
        Some(Signalling {
            width_cu: get(bits, 16, 12) as u16,
            id: get(bits, 28, 4) as u8,
            ti: ShTimeInterleaver {
                common_multiplier: get(bits, 32, 6) as u8,
                late_taps: get(bits, 38, 6) as u8,
                slices: get(bits, 44, 6) as u8,
                slice_distance: get(bits, 50, 8) as u8,
                non_late_increment: get(bits, 58, 6) as u8,
            },
            low_latency: second && bits[112] == 1,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dvbsh::coding::ShCodec;

    #[test]
    fn frame_width_and_symbol_rate_give_the_ofdm_sh_frame() {
        // Tables 5.12 and 5.13: a TDM frame lasts one OFDM SH frame (8 frames of 2K QPSK, half of it for 16-QAM).
        for width in Width::SEARCH {
            for guard in Guard::ALL {
                let ofdm = 8.0 * 68.0 * (2048 + guard.samples(2048)) as f64 / width.ofdm_rate();
                for ro in RollOff::ALL {
                    for qam in [false, true] {
                        for m in Modulation::ALL {
                            let cu = frame_width_cu(guard, ro, qam, m);
                            assert_eq!(cu % m.bits(), 0);
                            let t = (cu / m.bits() * SLOT) as f64 / symbol_rate(width, guard, ro);
                            let want = if qam { ofdm / 2.0 } else { ofdm };
                            assert!((t / want - 1.0).abs() < 1e-12, "{width:?} {guard:?} {ro:?} {qam} {m:?}");
                        }
                    }
                }
            }
        }
        assert_eq!(frame_width_cu(Guard::G4, RollOff::R15, false, Modulation::Qpsk), 952);
        assert_eq!(frame_width_cu(Guard::G4, RollOff::R35, true, Modulation::Psk8), 609);
        assert_eq!(frame_width_cu(Guard::G32, RollOff::R35, false, Modulation::Apsk16), 1344);
        assert_eq!(frame_width_cu(Guard::G8, RollOff::R25, false, Modulation::Psk8), 1176);
        assert!((symbol_rate(Width::W5, Guard::G4, RollOff::R15) - 4.25e6).abs() < 1e-6);
        assert!((symbol_rate(Width::W7, Guard::G32, RollOff::R35) - 56e6 / 11.0).abs() < 1e-6);
        assert!((symbol_rate(Width::W1_7, Guard::G16, RollOff::R35) - 20e6 / 17.0).abs() < 1e-6);
        assert!((symbol_rate(Width::W6, Guard::G8, RollOff::R15) - 31e6 / 6.0).abs() < 1e-6);
    }

    #[test]
    fn code_words_of_a_frame() {
        // QPSK 1/3 in 952 units: 52 code words in 936 units.
        let f = TdmFrame::new(952, Modulation::Qpsk, 4, ShTimeInterleaver::default()).unwrap();
        assert_eq!((f.shape.codewords, f.shape.data_iu() / 16, f.slots), (52, 936, 476));
        assert_eq!(f.padding_bits(), (952 - 3 - 936) * 2016);
        assert_eq!(f.shape.slice_unit, 312);
        for id in 0..12u8 {
            assert_eq!(n_bil(id), Some(ShCodec::new(id).unwrap().n_bil()), "pattern {id}");
        }
        assert_eq!(n_bil(12), None);
        assert!(TdmFrame::new(953, Modulation::Qpsk, 4, Default::default()).is_none());
        assert!(TdmFrame::new(3, Modulation::Psk8, 4, Default::default()).is_none());
    }

    #[test]
    fn signalling_example_and_damage() {
        // Width 784, pattern 4, interleaver 4 / 48 / 1 / 0 / 0.
        let s = Signalling {
            width_cu: 784,
            id: 4,
            ti: ShTimeInterleaver {
                common_multiplier: 4,
                late_taps: 48,
                slices: 1,
                slice_distance: 0,
                non_late_increment: 0,
            },
            low_latency: false,
        };
        let mut bits = vec![0u8; SIGNALLING_BITS];
        s.write(&mut bits);
        let hex: String = bits[..112]
            .chunks(4)
            .map(|c| format!("{:X}", c.iter().fold(0, |a, &b| a << 1 | b)))
            .collect();
        assert_eq!(hex, "0000310413004000000000004519");
        assert!(bits[400..].iter().all(|&b| b == 0));
        assert_eq!(Signalling::read(&bits), Some(s));
        for at in [3, 20, 60, 100] {
            let mut bad = bits.clone();
            bad[at] ^= 1;
            assert_eq!(Signalling::read(&bad), None, "bit {at}");
        }
        // A reserved byte that is not 0, with a CRC that holds.
        let mut odd = bits.clone();
        odd[12] = 1;
        let crc = CRC16_IBM_3740.bits(&odd[..96]);
        put(&mut odd, 96, 16, u64::from(crc));
        assert_eq!(Signalling::read(&odd), None);
    }

    #[test]
    fn sof_constellations_and_scrambling() {
        let b = sof_bits();
        let first = b[..32].iter().fold(0u32, |a, &x| a << 1 | x as u32);
        let last = b[272..].iter().fold(0u32, |a, &x| a << 1 | x as u32);
        assert_eq!((first, last), (0xF348_4536, 0x9E4F));
        assert_eq!(HEADER_BITS, 3 * CU_BITS);
        for m in Modulation::ALL {
            let p = m.point_set();
            assert_eq!(p.len(), 1 << m.bits());
            let e: f32 = p.points().iter().map(|z| z.norm_sqr()).sum::<f32>() / p.len() as f32;
            assert!((e - 1.0).abs() < 1e-5, "{m:?}");
        }
        let q = Modulation::Qpsk.point_set().point(0).unwrap();
        assert!(q.re > 0.0 && q.im > 0.0);
        let a = Modulation::Apsk16.point_set();
        let mut r: Vec<f32> = a.points().iter().map(|z| z.norm()).collect();
        r.sort_by(f32::total_cmp);
        assert!((r[0] - 1.0 / 7f32.sqrt()).abs() < 1e-5 && (r[3] - r[0]).abs() < 1e-5);
        assert!((r[4] - 3.0 / 7f32.sqrt()).abs() < 1e-5 && (r[15] - r[4]).abs() < 1e-5);
        // The scrambling of a slot is the sequence of EN 302 307 from its definition, code 0.
        let m = (1usize << 18) - 1;
        let mut x = vec![0u8; m + 18];
        let mut y = vec![1u8; m + 18];
        x[0] = 1;
        for i in 0..m {
            x[i + 18] = x[i + 7] ^ x[i];
            y[i + 18] = y[i + 10] ^ y[i + 7] ^ y[i + 5] ^ y[i];
        }
        let z = |i: usize| x[i % m] ^ y[i % m];
        let s = scrambling(0);
        assert_eq!(s.len(), SLOT);
        for (i, &r) in s.iter().enumerate() {
            assert_eq!(r, 2 * z((i + 131_072) % m) + z(i), "symbol {i}");
        }
        let f = pilot_fields(&s);
        assert_eq!(f[1][5], quarter(PILOT, s[SECOND_FIELD + 5]));
        assert_eq!((data_place(0), data_place(1007), data_place(1008), data_place(2015)), (80, 1087, 1168, 2175));
    }
}
