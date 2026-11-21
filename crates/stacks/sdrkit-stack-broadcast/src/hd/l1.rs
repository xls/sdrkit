//! HD Radio Layer 1: the FM and AM frame layouts, the logical channels P1, P3 and PIDS with their codes and interleavers, the
//! system control data sequence on the pilot subcarriers, the AM subcarrier mapping and levels, and the bit mapping of each
//! block.
//!
//! Standard: NRSC-5-E documents 1011s and 1012s, clauses 6 and 9 to 12, and 1082s table 4-6.

use crate::qam::Qam;
use sdrkit_algo_fec::conv::{
    codes, depuncture_frame, encode_tail_biting, puncture, reencode_errors, tail_biting_state,
    ConvCode, Ends, Puncture, Viterbi,
};
use sdrkit_algo_fec::{Llr8, Quantizer};
use sdrkit_algo_modem::ofdm::FrameLayout;
use sdrkit_core::Complex32;
use sdrkit_util_bits::lfsr::Additive;
use sdrkit_util_bits::seq;

/// FM OFDM rate.
pub const FM_RATE: f64 = 744_187.5;
/// AM OFDM rate (FM rate / 16).
pub const AM_RATE: f64 = FM_RATE / 16.0;
/// Symbols per block.
pub const NSYM: usize = 32;

/// The band.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
pub enum Band {
    /// FM.
    Fm,
    /// AM.
    Am,
}

sdrkit_core::choice_enum! {
    /// The service mode.
    #[derive(serde::Serialize)]
    pub enum ServiceMode {
        /// FM hybrid.
        Mp1 = ("mp1", "MP1"),
        /// FM extended hybrid with P3 on the extended partitions.
        Mp3 = ("mp3", "MP3"),
        /// AM hybrid.
        Ma1 = ("ma1", "MA1"),
        /// AM all digital.
        Ma3 = ("ma3", "MA3"),
    }
}

impl ServiceMode {
    /// "MP1" ...
    pub fn name(self) -> &'static str {
        match self {
            ServiceMode::Mp1 => "MP1",
            ServiceMode::Mp3 => "MP3",
            ServiceMode::Ma1 => "MA1",
            ServiceMode::Ma3 => "MA3",
        }
    }
    /// The band.
    pub fn band(self) -> Band {
        match self {
            ServiceMode::Mp1 | ServiceMode::Mp3 => Band::Fm,
            _ => Band::Am,
        }
    }
    /// The Service Mode Indicator of an AM mode (1012s table 11-3: MA1 is 00001, MA3 is 00010). `None` for an FM mode.
    pub fn smi(self) -> Option<u8> {
        smi_of(self)
    }
    /// The AM mode of a Service Mode Indicator, or `None` for a reserved value (1012s table 11-3).
    pub fn from_smi(smi: u8) -> Option<ServiceMode> {
        match smi {
            1 => Some(ServiceMode::Ma1),
            2 => Some(ServiceMode::Ma3),
            _ => None,
        }
    }
    /// The Primary Service Mode Indicator of an FM mode (1011s table 6-2: MP1 is 000001, MP3 is 000011). `None` for an AM mode.
    pub fn psmi(self) -> Option<u8> {
        match self {
            ServiceMode::Mp1 => Some(1),
            ServiceMode::Mp3 => Some(3),
            _ => None,
        }
    }
    /// The FM mode a receiver decodes for a Primary Service Mode Indicator: the mode itself or, for a reserved value, the
    /// default mode of 1011s table 6-5. `None` for a mode without a default that sdrkit decodes (MP2, MP5, MP6, the advanced
    /// mode 0). A station in MP11 is decoded as MP3, its default.
    pub fn from_psmi(psmi: u8) -> Option<ServiceMode> {
        match FM_PSMI_DEFAULT.get(psmi as usize)? {
            1 => Some(ServiceMode::Mp1),
            3 | 11 => Some(ServiceMode::Mp3),
            _ => None,
        }
    }
    /// Blocks per frame.
    pub fn blocks(self) -> usize {
        if self.band() == Band::Fm {
            16
        } else {
            8
        }
    }
    /// P1 information bits per transfer frame (FM: per frame; AM: per block).
    pub fn p1_bits(self) -> usize {
        if self.band() == Band::Fm {
            146_176
        } else {
            3750
        }
    }
    /// P3 information bits per transfer frame (FM: per block; AM: per frame); 0 when the mode has none.
    pub fn p3_bits(self) -> usize {
        match self {
            ServiceMode::Mp1 => 0,
            ServiceMode::Mp3 => 2304,
            ServiceMode::Ma1 => 24_000,
            ServiceMode::Ma3 => 30_000,
        }
    }
}

/// The primary service mode a first-generation receiver decodes, by Primary Service Mode Indicator 0 to 63, as the number of
/// the mode (1 for MP1). The modes MP1, MP2, MP3, MP5 and MP6 of 1011s table 6-2 map to themselves, the reserved values to the
/// default of table 6-5. Value 0 is the advanced mode 0, which has no default.
pub const FM_PSMI_DEFAULT: [u8; 64] = [
    0, 1, 2, 3, 1, 5, 6, 5, 6, 1, 2, 3, 1, 5, 6, 5, // 0 to 15
    6, 1, 2, 3, 1, 5, 6, 5, 6, 1, 2, 11, 1, 5, 6, 5, // 16 to 31
    6, 1, 2, 3, 1, 5, 6, 5, 6, 1, 2, 11, 1, 5, 6, 5, // 32 to 47
    6, 1, 2, 3, 1, 5, 6, 5, 6, 1, 2, 11, 1, 5, 6, 5, // 48 to 63
];

/// The sync bits that every system control data sequence holds, as (bit index, value): Sync10:4 = 0110010 in bits 31 to 25,
/// Sync3 = 1 in bit 22, Sync2 = 0 in bit 17 and Sync1:0 = 11 in bits 10 and 9 (1011s tables 11-1 and 11-2, 1012s table 11-1).
/// Bit 31 goes in the first OFDM symbol of a block.
pub const SYNC_BITS: [(u32, u8); 11] = [
    (31, 0),
    (30, 1),
    (29, 1),
    (28, 0),
    (27, 0),
    (26, 1),
    (25, 0),
    (22, 1),
    (17, 0),
    (10, 1),
    (9, 1),
];

/// The identification RSID1:0 of each pilot column of the FM matrix r, columns 0 to 60 (1011s table 11-3).
pub const FM_RSID: [u8; 61] = [
    2, 1, 0, 3, 2, 1, 0, 3, 2, 1, 0, 3, 2, 1, 0, 3, // 0 to 15
    2, 1, 0, 3, 2, 1, 0, 3, 2, 1, 0, 3, 2, 1, 0, 1, // 16 to 31
    2, 3, 0, 1, 2, 3, 0, 1, 2, 3, 0, 1, 2, 3, 0, 1, // 32 to 47
    2, 3, 0, 1, 2, 3, 0, 1, 2, 3, 0, 1, 2, // 48 to 60
];

/// The pilot subcarriers of the primary sidebands and the column of R each one carries, for the widest primary modes (MP11,
/// MP5, MP6 and the modes beside them in 1011s table 12-24). MP1 uses the subcarriers from ±356 out, MP3 those from ±318 out.
pub const FM_PILOT_COLUMNS: [(i32, u8); 30] = [
    (-546, 0),
    (-527, 1),
    (-508, 2),
    (-489, 3),
    (-470, 4),
    (-451, 5),
    (-432, 6),
    (-413, 7),
    (-394, 8),
    (-375, 9),
    (-356, 10),
    (-337, 11),
    (-318, 12),
    (-299, 13),
    (-280, 14),
    (280, 46),
    (299, 47),
    (318, 48),
    (337, 49),
    (356, 50),
    (375, 51),
    (394, 52),
    (413, 53),
    (432, 54),
    (451, 55),
    (470, 56),
    (489, 57),
    (508, 58),
    (527, 59),
    (546, 60),
];

/// The column of R that the FM primary pilot subcarrier `k` carries (1011s table 12-24), or `None`.
pub fn fm_pilot_column(k: i32) -> Option<usize> {
    FM_PILOT_COLUMNS
        .iter()
        .find(|&&(s, _)| s == k)
        .map(|&(_, c)| c as usize)
}

/// The even parity bit of a value: 1 when it has an odd number of ones.
fn parity(v: u32) -> u32 {
    v.count_ones() & 1
}

/// The fields of the primary system control data sequence of 1011s table 11-1 that the station sets.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FmControl {
    /// Secondary Channel Indicator: secondary sidebands present (All Digital).
    pub sci: bool,
    /// Advanced Service Mode Control bit 1 (bit 19).
    pub asm1: bool,
    /// Advanced Service Mode Control bit 0 (bit 8).
    pub asm0: bool,
    /// Reserved1 (bit 16).
    pub reserved1: bool,
    /// Reserved0 (bit 7).
    pub reserved0: bool,
    /// L1 block count BC3:0, 0 to 15.
    pub block: u8,
    /// Primary Service Mode Indicator PSMI5:0.
    pub psmi: u8,
}

impl FmControl {
    /// The control of a station in a standard mode: no secondary sidebands, ASM0 = 1 and ASM1 = 0 for a non-zero PSMI (1011s
    /// clause 11.2.2), reserved bits 0.
    pub fn standard(block: u8, psmi: u8) -> FmControl {
        FmControl {
            asm0: psmi != 0,
            block: block & 15,
            psmi: psmi & 63,
            ..FmControl::default()
        }
    }
    /// The 32-bit sequence r of pilot column `column` (bit 31 first), with its parity bits (1011s table 11-1).
    pub fn sequence(&self, column: usize) -> u32 {
        let rsid = FM_RSID.get(column).copied().unwrap_or(0) as u32;
        let (sci, asm1, asm0) = (self.sci as u32, self.asm1 as u32, self.asm0 as u32);
        let (res1, res0) = (self.reserved1 as u32, self.reserved0 as u32);
        let bc = (self.block & 15) as u32;
        let psmi = (self.psmi & 63) as u32;
        let mut r = SYNC_BITS
            .iter()
            .fold(0u32, |a, &(b, v)| a | (v as u32) << b);
        r |= sci << 24 | parity(sci) << 23;
        r |= rsid << 20 | asm1 << 19 | parity(rsid ^ asm1 << 2) << 18;
        r |= res1 << 16 | bc << 12 | parity(bc ^ res1 << 4) << 11;
        r |= asm0 << 8 | res0 << 7 | psmi << 1 | parity(psmi ^ res0 << 6 ^ asm0 << 7);
        r
    }
    /// Reads the fields of a received sequence. `None` when a sync bit or the parity bits 23, 11 and 0 do not agree (bit 18 is
    /// not checked: it covers the RSID, which differs between columns).
    pub fn parse(r: u32) -> Option<FmControl> {
        let sync_ok = SYNC_BITS.iter().all(|&(b, v)| (r >> b) & 1 == v as u32);
        let bits = |hi: u32, lo: u32| (r >> lo) & ((1 << (hi - lo + 1)) - 1);
        let ok = sync_ok
            && parity(bits(24, 23)) == 0
            && parity(bits(16, 11)) == 0
            && parity(bits(8, 0)) == 0;
        ok.then(|| FmControl {
            sci: bits(24, 24) != 0,
            asm1: bits(19, 19) != 0,
            asm0: bits(8, 8) != 0,
            reserved1: bits(16, 16) != 0,
            reserved0: bits(7, 7) != 0,
            block: bits(15, 12) as u8,
            psmi: bits(6, 1) as u8,
        })
    }
}

/// The differential encoding of one column of r into R (1011s clause 11.3): `R[i] = r[i] xor R[i - 1]`, starting from 0. Row
/// `i` is bit `31 - i`. The inverse is `R ^ (R >> 1)`.
pub fn differential(r: u32) -> u32 {
    let mut out = 0u32;
    let mut state = 0u32;
    for row in 0..32 {
        state ^= (r >> (31 - row)) & 1;
        out |= state << (31 - row);
    }
    out
}

/// The rows of R (bit `31 - row` set) whose value on a column is the same for every setting of the station: every SCI,
/// ASM, reserved bit, block count and PSMI. These cells are known to the receiver in advance.
pub fn fm_fixed_rows() -> u32 {
    let first = differential(FmControl::default().sequence(0));
    let mut same = !0u32;
    for bits in 0..32u32 {
        for block in 0..16u8 {
            for psmi in 0..64u8 {
                let c = FmControl {
                    sci: bits & 1 != 0,
                    asm1: bits & 2 != 0,
                    asm0: bits & 4 != 0,
                    reserved1: bits & 8 != 0,
                    reserved0: bits & 16 != 0,
                    block,
                    psmi,
                };
                same &= !(differential(c.sequence(0)) ^ first);
            }
        }
    }
    same
}

/// The cell of a bit of R on an FM pilot subcarrier: `-1 - j` for 0 and `1 + j` for 1 (1011s table 12-18), at the level of a
/// QPSK data cell.
pub fn fm_pilot_cell(bit: u32) -> Complex32 {
    let a = std::f32::consts::FRAC_1_SQRT_2;
    if bit & 1 == 1 {
        Complex32::new(a, a)
    } else {
        Complex32::new(-a, -a)
    }
}

/// The frame plan of a service mode.
#[derive(Clone, Debug)]
pub struct BandPlan {
    /// The band.
    pub band: Band,
    /// The modem layout. MP1 and MP3 share one (the union of their pilots); MA1 and MA3 each have their own.
    pub layout: FrameLayout,
    /// Pilot subcarriers (as carrier numbers).
    pub refs: Vec<i32>,
}

/// The FM primary main data subcarriers (lower side first, then upper), 180 per side.
pub fn fm_primary() -> Vec<i32> {
    let mut v = Vec::new();
    for side in [-1i32, 1] {
        let mut part = Vec::new();
        for p in 0..10 {
            for d in 1..19 {
                part.push(356 + 19 * p + d);
            }
        }
        if side < 0 {
            part.reverse();
        }
        v.extend(part.into_iter().map(|k| side * k));
    }
    v
}

/// The FM extended partitions' data subcarriers (MP3), 36 per side.
pub fn fm_extended() -> Vec<i32> {
    let mut v = Vec::new();
    for side in [-1i32, 1] {
        for p in 0..2 {
            for d in 1..19 {
                v.push(side * (318 + 19 * p + d));
            }
        }
    }
    v
}

/// The FM pilot subcarriers of MP3 (and MP1 from ±356 out), as in 1011s table 12-24.
pub fn fm_refs() -> Vec<i32> {
    FM_PILOT_COLUMNS
        .iter()
        .map(|p| p.0)
        .filter(|k| k.abs() >= 318)
        .collect()
}

// ---------------------------------------------------------------------------------------------------------------------------
// AM

/// The AM Service Mode Indicator SMI4:0 of 1012s table 11-3 (MA1 is 00001, MA3 is 00010), as a function of the mode.
fn smi_of(mode: ServiceMode) -> Option<u8> {
    match mode {
        ServiceMode::Ma1 => Some(1),
        ServiceMode::Ma3 => Some(2),
        _ => None,
    }
}

/// The fields of the AM system control data sequence of 1012s table 11-1 that the station sets.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct AmControl {
    /// Power Level Indicator (bit 24): the secondary, tertiary and PIDS1 levels CHS2, CHT2, CHI2.
    pub pli: bool,
    /// Reserved4 (bit 21).
    pub reserved4: bool,
    /// High-Power PIDS Indicator (bit 20).
    pub hppi: bool,
    /// Analog Audio Bandwidth Indicator (bit 19): 8 kHz.
    pub aabi: bool,
    /// Reduced Digital Bandwidth Indicator (bit 16).
    pub rdbi: bool,
    /// Reserved3 (bit 15).
    pub reserved3: bool,
    /// L1 block count BC2:0, 0 to 7.
    pub block: u8,
    /// Reserved2:0 (bits 8 to 6).
    pub reserved: u8,
    /// Service Mode Indicator SMI4:0.
    pub smi: u8,
}

impl AmControl {
    /// The control of a station in `mode` at block `block` with every indicator and reserved bit 0.
    pub fn standard(block: u8, mode: ServiceMode) -> AmControl {
        AmControl {
            block: block & 7,
            smi: smi_of(mode).unwrap_or(0),
            ..AmControl::default()
        }
    }
    /// The 32-bit sequence (bit 31 first) with its parity bits (1012s table 11-1).
    pub fn sequence(&self) -> u32 {
        let b = |v: bool| v as u32;
        let (r4, hp, aa) = (b(self.reserved4), b(self.hppi), b(self.aabi));
        let (rd, r3) = (b(self.rdbi), b(self.reserved3));
        let bc = (self.block & 7) as u32;
        let (res, smi) = ((self.reserved & 7) as u32, (self.smi & 31) as u32);
        let mut r = SYNC_BITS
            .iter()
            .fold(0u32, |a, &(bit, v)| a | (v as u32) << bit);
        r |= b(self.pli) << 24 | b(self.pli) << 23;
        r |= r4 << 21 | hp << 20 | aa << 19 | parity(r4 << 2 | hp << 1 | aa) << 18;
        r |= rd << 16 | r3 << 15 | bc << 12 | parity(rd << 4 | r3 << 3 | bc) << 11;
        r |= res << 6 | smi << 1 | parity(res << 5 | smi);
        r
    }
    /// Reads the fields of a received sequence. `None` when a sync bit or a parity bit does not agree.
    pub fn parse(r: u32) -> Option<AmControl> {
        let bits = |hi: u32, lo: u32| (r >> lo) & ((1 << (hi - lo + 1)) - 1);
        let ok = SYNC_BITS.iter().all(|&(b, v)| (r >> b) & 1 == v as u32)
            && parity(bits(24, 23)) == 0
            && parity(bits(21, 18)) == 0
            && parity(bits(16, 11)) == 0
            && parity(bits(8, 0)) == 0;
        ok.then(|| AmControl {
            pli: bits(24, 24) != 0,
            reserved4: bits(21, 21) != 0,
            hppi: bits(20, 20) != 0,
            aabi: bits(19, 19) != 0,
            rdbi: bits(16, 16) != 0,
            reserved3: bits(15, 15) != 0,
            block: bits(14, 12) as u8,
            reserved: bits(8, 6) as u8,
            smi: bits(5, 1) as u8,
        })
    }
}

/// The rows of an AM block (bit `31 - row` set) that every system control data sequence fixes: the sync rows. The sequence
/// is not differentially encoded (1012s clause 11), so no other row is the same for every setting.
pub const AM_FIXED_ROWS: u32 = 0xFE42_0600;

/// The cell of a bit of R on the AM pilot subcarrier 1: `-j` for 0 and `j` for 1 (1012s table 12-10, at unit power before the
/// scale factor). Subcarrier -1 carries the negated conjugate, which is the same value.
pub fn am_pilot_cell(bit: u32) -> Complex32 {
    Complex32::new(0.0, if bit & 1 == 1 { 1.0 } else { -1.0 })
}

/// The AM pilot subcarriers: R on 1 and its negated conjugate on -1 (1012s table 12-12).
pub const AM_PILOTS: [i32; 2] = [-1, 1];

/// The AM subcarriers that are never used: -56 to -54, 0 and 54 to 56 (1012s clause 12.2).
pub const AM_UNUSED: [i32; 7] = [-56, -55, -54, 0, 54, 55, 56];

/// Where one column of an AM interleaver matrix goes: subcarrier `k`, negated and conjugated when `conj` is set, and a copy on
/// `-k`, negated and conjugated, when `twin` is set.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Place {
    /// The subcarrier.
    pub k: i32,
    /// The subcarrier holds the negated conjugate of the value.
    pub conj: bool,
    /// Subcarrier `-k` holds the negated conjugate too.
    pub twin: bool,
}

/// The subcarriers of the columns of the AM matrices of a service mode, and the constellations of S and T.
#[derive(Clone, Debug)]
pub struct AmLayout {
    /// PU column `c` (64-QAM).
    pub pu: [Place; 25],
    /// PL column `c` (64-QAM).
    pub pl: [Place; 25],
    /// S column `c`.
    pub s: [Place; 25],
    /// T column `c`.
    pub t: [Place; 25],
    /// PIDS columns 0 and 1 (16-QAM).
    pub pids: [Place; 2],
    /// The constellation of S.
    pub s_qam: Qam,
    /// The constellation of T.
    pub t_qam: Qam,
}

/// The AM layout of a service mode. MA1 (1012s tables 12-2, 12-6 and 12-7): PU on 57 to 81, PL negated and conjugated on -57
/// to -81, T on 2 to 26 and S on 28 to 52 with their negated conjugates on -2 to -26 and -28 to -52, PIDS column 0 on ±27 and
/// column 1 on ±53. MA3 (tables 12-3, 12-8 and 12-9): PU on 2 to 26, PL on -2 to -26, S on 28 to 52, T on -28 to -52, PIDS
/// column 0 on -27 and column 1 on 27, the lower ones negated and conjugated.
pub fn am_layout(mode: ServiceMode) -> AmLayout {
    let up = |a: i32, twin: bool| {
        std::array::from_fn(|c| Place {
            k: a + c as i32,
            conj: false,
            twin,
        })
    };
    let down = |a: i32| {
        std::array::from_fn(|c| Place {
            k: -(a + c as i32),
            conj: true,
            twin: false,
        })
    };
    let at = |k: i32, twin: bool| Place {
        k,
        conj: k < 0,
        twin,
    };
    if mode == ServiceMode::Ma3 {
        AmLayout {
            pu: up(2, false),
            pl: down(2),
            s: up(28, false),
            t: down(28),
            pids: [at(-27, false), at(27, false)],
            s_qam: Qam::Q64,
            t_qam: Qam::Q64,
        }
    } else {
        AmLayout {
            pu: up(57, false),
            pl: down(57),
            s: up(28, true),
            t: up(2, true),
            pids: [at(27, true), at(53, true)],
            s_qam: Qam::Q16,
            t_qam: Qam::Q4,
        }
    }
}

/// The level of each AM sideband in dBc per subcarrier, against the unmodulated carrier (1082s table 4-6).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AmLevels {
    /// PU and PL.
    pub primary: f32,
    /// S.
    pub secondary: f32,
    /// T, by column.
    pub tertiary: [f32; 25],
    /// PIDS columns 0 and 1.
    pub pids: [f32; 2],
    /// The pilot subcarriers ±1.
    pub pilot: f32,
}

impl AmLevels {
    /// The amplitude of a sideband at `dbc` against a primary subcarrier.
    pub fn gain(&self, dbc: f32) -> f32 {
        10f32.powf((dbc - self.primary) / 20.0)
    }
}

/// The tapered tertiary scale factor CHT1 of MA1 by column, dBc: -44 on column 0, 0.5 dB less on each column, -50 from column
/// 12 on (1082s table 4-6).
pub const MA1_CHT1_DBC: [f32; 25] = [
    -44.0, -44.5, -45.0, -45.5, -46.0, -46.5, -47.0, -47.5, -48.0, -48.5, -49.0, -49.5, -50.0,
    -50.0, -50.0, -50.0, -50.0, -50.0, -50.0, -50.0, -50.0, -50.0, -50.0, -50.0, -50.0,
];

/// The levels of an AM mode with the power level control `pl` and the high-power PIDS control `hpp` (1012s tables 6-4 and
/// 6-5, levels of 1082s table 4-6, the reduced digital bandwidth off). MA1: CHPU and CHPL -30, CHB -26; PL 0 sets CHS1 -43,
/// CHT1 and CHI1 -43 on PIDS1 (±27), PL 1 sets CHS2 -37, CHT2 -44 and CHI2 -37; PIDS2 (±53) is 13 dB (CHI3), 7 dB (CHI4, PL 1)
/// or 0 dB (CHI5, HPP 1) below the primary. MA3: CDP and CDB -15, CDE -30, both PIDS 15 dB (CDI1) or 0 dB (CDI2, HPP 1) below
/// the primary.
pub fn am_levels(mode: ServiceMode, pl: bool, hpp: bool) -> AmLevels {
    if mode == ServiceMode::Ma3 {
        let pids = if hpp { -15.0 } else { -30.0 };
        return AmLevels {
            primary: -15.0,
            secondary: -30.0,
            tertiary: [-30.0; 25],
            pids: [pids, pids],
            pilot: -15.0,
        };
    }
    let pids2 = match (hpp, pl) {
        (true, _) => -30.0,
        (false, true) => -37.0,
        (false, false) => -43.0,
    };
    AmLevels {
        primary: -30.0,
        secondary: if pl { -37.0 } else { -43.0 },
        tertiary: if pl { [-44.0; 25] } else { MA1_CHT1_DBC },
        pids: [if pl { -37.0 } else { -43.0 }, pids2],
        pilot: -26.0,
    }
}

/// The training cell of a constellation: the training word of 1012s table 10-5 mapped by tables 12-1, 12-4 and 12-5.
pub fn am_training_cell(qam: Qam) -> Complex32 {
    qam.map(am_training(qam.bits()))
}

/// One cell of a logical channel: where it is, its constellation and level, and its complementary copy.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Cell {
    /// Grid index: symbol times carriers plus the carrier less `kmin`.
    pub at: usize,
    /// The constellation.
    pub qam: Qam,
    /// The amplitude against a primary cell.
    pub gain: f32,
    /// The cell holds the negated conjugate of the value.
    pub conj: bool,
    /// A second grid index that holds the value too, and whether it holds the negated conjugate.
    pub twin: Option<(usize, bool)>,
}

impl Cell {
    /// An FM data cell: QPSK at unit level.
    pub fn fm(at: usize) -> Cell {
        Cell {
            at,
            qam: Qam::Q4,
            gain: 1.0,
            conj: false,
            twin: None,
        }
    }
    /// Writes the value `v` (before the gain) into the grid.
    pub fn put(&self, v: Complex32, grid: &mut [Complex32]) {
        let v = v * self.gain;
        grid[self.at] = if self.conj { -v.conj() } else { v };
        if let Some((t, c)) = self.twin {
            grid[t] = if c { -v.conj() } else { v };
        }
    }
    /// The received value (before the gain) and its weight, from the equalised cells `z` and their weights `w`. A complementary
    /// pair is averaged: an analogue programme under it cancels.
    pub fn value(&self, z: &[Complex32], w: &[f32]) -> (Complex32, f32) {
        let get = |i: usize, c: bool| if c { -z[i].conj() } else { z[i] };
        let (mut v, mut wt) = (get(self.at, self.conj), w[self.at]);
        if let Some((t, c)) = self.twin {
            let (v2, w2) = (get(t, c), w[t]);
            v = (v + v2) * 0.5;
            wt = if wt + w2 > 0.0 {
                4.0 * wt * w2 / (wt + w2)
            } else {
                0.0
            };
        }
        (v / self.gain, wt * self.gain * self.gain)
    }
    /// The cell on the mirrored spectrum: each carrier `c` moved to `-c`, and the conjugation of each copy reversed.
    fn mirrored(&self, k: usize, kmin: i32) -> Cell {
        let flip = |i: usize| {
            let (s, c) = (i / k, (i % k) as i32 + kmin);
            s * k + (-c - kmin) as usize
        };
        Cell {
            at: flip(self.at),
            conj: !self.conj,
            twin: self.twin.map(|(t, c)| (flip(t), !c)),
            ..*self
        }
    }
}

/// The AM plan of a service mode: the pilot subcarriers ±1 (known on the sync rows), the training cells of the matrices as
/// known cells, and the unused subcarriers. The training cells of PIDS are not known cells: their level is the station's
/// choice (HPP). In MA1 only the training cells of PU and PL are known: S and T sit under the analogue programme and their
/// level is the station's choice (PL), and the demodulator estimates each known cell on its own.
fn am_plan(mode: ServiceMode) -> BandPlan {
    let (kmin, kmax) = (-81i32, 81i32);
    let a = am_layout(mode);
    let lv = am_levels(mode, false, false);
    let ma3 = mode == ServiceMode::Ma3;
    let mut known = vec![Vec::new(); NSYM];
    let mut put = |s: usize, p: Place, v: Complex32| {
        known[s].push(((p.k - kmin) as usize, if p.conj { -v.conj() } else { v }));
        if p.twin {
            known[s].push(((-p.k - kmin) as usize, -v.conj()));
        }
    };
    let ctrl = AmControl::standard(0, mode).sequence();
    for s in (0..NSYM).filter(|s| (AM_FIXED_ROWS >> (31 - s)) & 1 == 1) {
        let v = am_pilot_cell(ctrl >> (31 - s)) * lv.gain(lv.pilot);
        put(
            s,
            Place {
                k: 1,
                conj: false,
                twin: true,
            },
            v,
        );
    }
    for k in 750..800 {
        let (row, col) = am_row_col(k);
        let p = am_training_cell(Qam::Q64);
        put(row, a.pu[col], p);
        put(row, a.pl[col], p);
        if ma3 {
            put(
                row,
                a.s[col],
                am_training_cell(a.s_qam) * lv.gain(lv.secondary),
            );
            put(
                row,
                a.t[col],
                am_training_cell(a.t_qam) * lv.gain(lv.tertiary[col]),
            );
        }
    }
    for row in &mut known {
        row.sort_by_key(|x| x.0);
    }
    let mut used: Vec<i32> = AM_PILOTS.to_vec();
    for p in
        a.pu.iter()
            .chain(&a.pl)
            .chain(&a.s)
            .chain(&a.t)
            .chain(&a.pids)
    {
        used.push(p.k);
        if p.twin {
            used.push(-p.k);
        }
    }
    let null = (kmin..=kmax)
        .filter(|k| !used.contains(k))
        .map(|k| (k - kmin) as usize)
        .collect();
    // The guard correlation sees 5.4 to 15.5 kHz either side (the outer sidebands of both modes), so neither the carrier nor
    // the analogue programme reaches it.
    let mut taps = sdrkit_algo_dsp::fir::bandpass_complex(AM_RATE, 5.4e3, 15.5e3, 0.6e3, 50.0)
        .expect("the filter settings are valid");
    let low = sdrkit_algo_dsp::fir::bandpass_complex(AM_RATE, -15.5e3, -5.4e3, 0.6e3, 50.0)
        .expect("the filter settings are valid");
    for (x, y) in taps.iter_mut().zip(&low) {
        *x += *y;
    }
    BandPlan {
        band: Band::Am,
        layout: FrameLayout {
            rate: AM_RATE,
            nfft: 256,
            cp: 14,
            cp0: 14,
            nsym: NSYM,
            kmin,
            kmax,
            known,
            null,
            max_shift: 6,
            adv: 7,
            sync_threshold: 0.45,
            guard_threshold: 0.2,
            search_taps: Some(taps),
            pair_band: None,
        },
        refs: AM_PILOTS.to_vec(),
    }
}

/// The FM plan: the pilot subcarriers of MP3 with the rows of R that every station sends the same as known cells.
fn fm_plan() -> BandPlan {
    let refs = fm_refs();
    let (kmin, kmax) = (-546, 546);
    let k = (kmax - kmin + 1) as usize;
    let fixed = fm_fixed_rows();
    let mut known = vec![Vec::new(); NSYM];
    for (s, row) in known.iter_mut().enumerate() {
        if (fixed >> (31 - s)) & 1 == 0 {
            continue;
        }
        for &r in &refs {
            let col = fm_pilot_column(r).unwrap_or(0);
            let big_r = differential(FmControl::default().sequence(col));
            row.push(((r - kmin) as usize, fm_pilot_cell(big_r >> (31 - s))));
        }
    }
    let used: std::collections::BTreeSet<i32> = refs
        .iter()
        .copied()
        .chain(fm_primary())
        .chain(fm_extended())
        .collect();
    let null = (0..k)
        .filter(|&c| !used.contains(&(kmin + c as i32)))
        .collect();
    // The guard correlation looks at the sidebands only: the analogue host is taken out.
    let mut taps: Vec<Complex32> = sdrkit_algo_dsp::fir::lowpass(FM_RATE, 105e3, 125e3, 50.0)
        .expect("the filter settings are valid")
        .iter()
        .map(|&v| Complex32::new(-v, 0.0))
        .collect();
    let c = taps.len() / 2;
    taps[c] += Complex32::new(1.0, 0.0);
    BandPlan {
        band: Band::Fm,
        layout: FrameLayout {
            rate: FM_RATE,
            nfft: 2048,
            cp: 112,
            cp0: 112,
            nsym: NSYM,
            kmin,
            kmax,
            known,
            null,
            max_shift: 30,
            adv: 28,
            sync_threshold: 0.4,
            guard_threshold: 0.25,
            search_taps: Some(taps),
            pair_band: None,
        },
        refs,
    }
}

/// The plan of a service mode. MP1 and MP3 have the same plan.
pub fn mode_plan(mode: ServiceMode) -> BandPlan {
    match mode.band() {
        Band::Fm => fm_plan(),
        Band::Am => am_plan(mode),
    }
}

/// The plan of a band: the FM plan, or the MA1 plan for AM.
pub fn plan(band: Band) -> BandPlan {
    match band {
        Band::Fm => fm_plan(),
        Band::Am => am_plan(ServiceMode::Ma1),
    }
}

/// The cells of the logical channels of one block, in symbol then carrier order.
#[derive(Clone, Debug)]
pub struct CellPlan {
    /// P1 (FM: with the PIDS bits among them).
    pub p1: Vec<Cell>,
    /// PIDS cells (AM only; FM PIDS share the P1 cells).
    pub pids: Vec<Cell>,
    /// P3 cells.
    pub p3: Vec<Cell>,
}

impl CellPlan {
    /// The plan for a mirrored spectrum (each subcarrier `c` read on `-c`), for a grid of `k` carriers from `kmin`.
    pub fn mirrored(&self, k: usize, kmin: i32) -> CellPlan {
        let m = |v: &[Cell]| v.iter().map(|c| c.mirrored(k, kmin)).collect();
        CellPlan {
            p1: m(&self.p1),
            pids: m(&self.pids),
            p3: m(&self.p3),
        }
    }
}

/// Bits a list of cells carries.
pub fn cell_bits(c: &[Cell]) -> usize {
    c.iter().map(|c| c.qam.bits()).sum()
}

/// The cell plan of a service mode, with the AM levels of the power level control `pl` and the high-power PIDS control `hpp`.
pub fn cells(mode: ServiceMode, pl: bool, hpp: bool) -> CellPlan {
    let bp = mode_plan(mode);
    let l = &bp.layout;
    let k = l.ncarriers();
    let idx = |s: usize, kk: i32| s * k + (kk - l.kmin) as usize;
    let mut p1 = Vec::new();
    let mut pids = Vec::new();
    let mut p3 = Vec::new();
    match mode.band() {
        Band::Fm => {
            let prim = fm_primary();
            let ext = fm_extended();
            for s in 0..NSYM {
                p1.extend(prim.iter().map(|&kk| Cell::fm(idx(s, kk))));
                if mode == ServiceMode::Mp3 {
                    p3.extend(ext.iter().map(|&kk| Cell::fm(idx(s, kk))));
                }
            }
        }
        Band::Am => {
            let a = am_layout(mode);
            let lv = am_levels(mode, pl, hpp);
            let cell = |s: usize, p: Place, qam: Qam, dbc: f32| Cell {
                at: idx(s, p.k),
                qam,
                gain: lv.gain(dbc),
                conj: p.conj,
                twin: p.twin.then(|| (idx(s, -p.k), true)),
            };
            for s in 0..NSYM {
                for c in 0..25 {
                    p1.push(cell(s, a.pu[c], Qam::Q64, lv.primary));
                    p1.push(cell(s, a.pl[c], Qam::Q64, lv.primary));
                }
                for (c, &p) in a.pids.iter().enumerate() {
                    pids.push(cell(s, p, Qam::Q16, lv.pids[c]));
                }
                for c in 0..25 {
                    p3.push(cell(s, a.s[c], a.s_qam, lv.secondary));
                    p3.push(cell(s, a.t[c], a.t_qam, lv.tertiary[c]));
                }
            }
        }
    }
    CellPlan { p1, pids, p3 }
}

/// Bits of the primary main sidebands per block: 32 rows of 20 partitions of 36 columns (NRSC-5 1011s figure 10-2).
pub const FM_BLOCK_BITS: usize = 23_040;

/// The partition assignment vector of the PM interleaver (1011s tables 10-16 and 10-17).
pub const PM_V: [usize; 20] = [
    10, 2, 18, 6, 14, 8, 16, 0, 12, 4, 11, 3, 19, 7, 15, 9, 17, 1, 13, 5,
];

/// Row and column of index `k` within an interleaver block of `c` columns (1011s clause 10.2.3.3): `(11 k) mod 32` and
/// `(11 k + 9 INT(k / 32)) mod c`.
fn pm_row_col(k: usize, c: usize) -> (usize, usize) {
    ((11 * k) % 32, (11 * k + 9 * (k / 32)) % c)
}

/// Where P1 bit `i` (0 to 365 439) of a transfer frame goes in the L1 frame: `block * 23 040 + row * 720 + partition * 36 +
/// column`, by Interleaver I with J = 20, B = 16, C = 36, M = 1 (1011s clauses 10.2.3 and 10.4.1, table 10-16).
pub fn pm_p1_position(i: usize) -> usize {
    let (j, b, c) = (20, 16, 36);
    let partition = PM_V[i % j];
    let block = (i / j + 7 * partition) % b;
    let (row, col) = pm_row_col(i / (j * b), c);
    block * FM_BLOCK_BITS + row * 720 + partition * c + col
}

/// Where PIDS bit `n` (0 to 199) of block `block` goes in that block, by Interleaver II with b = 200 and I0 = 1142: the holes
/// Interleaver I leaves (1011s clauses 10.2.4 and 10.4.1, tables 10-17 and 10-18).
pub fn pm_pids_position(block: usize, n: usize) -> usize {
    let (j, c, b, i0) = (20, 36, 200, 1142);
    let i = block * b + n;
    let partition = PM_V[i % j];
    let k = (i % b) / j + i0;
    let (row, col) = pm_row_col(k, c);
    row * 720 + partition * c + col
}

/// Row and column of index `k` (0 to 799, 750 and up are training) within a 32 x 25 block of the PU, PL, S and T matrices
/// (NRSC-5 1012s clause 10.3.1).
pub fn am_row_col(k: usize) -> (usize, usize) {
    (
        (11 * ((9 * k) % 25) + 16 * (k / 25) + 11 * (k / 50)) % 32,
        (9 * k) % 25,
    )
}

/// The row of data index `k` (0 to 29) within a 32-row block of the PIDS matrix (1012s clause 10.3.2).
pub fn am_pids_row(k: usize) -> usize {
    (11 * (k + k / 15) + 3) % 32
}

/// The rows of the two training elements of a PIDS block: 8 and 24, as figure 10-3 shows. The equation of clause 10.3.2 results
/// in rows 3 and 14 for k = 30 and 31, which data already uses, so the figure is followed.
pub const AM_PIDS_TRAINING_ROWS: [usize; 2] = [8, 24];

/// The training bits of an element, `x0` first (bit position `p` of an element is `xp`): the words `100101` for 6 bits, `1001`
/// for 4 and `10` for 2 of 1012s table 10-5, written `x5` to `x0` as in table 12-1.
pub fn am_training(bits: usize) -> &'static [u8] {
    match bits {
        6 => &[1, 0, 1, 0, 0, 1],
        4 => &[1, 0, 0, 1],
        _ => &[0, 1],
    }
}

/// How one AM matrix block (32 rows of `columns` elements of `bits` bits) is filled: the place of data bit `n` of a block
/// as `(row, column, bit)`, data elements first in the order of the standard's `k`, then the training elements.
#[derive(Clone, Debug)]
pub struct AmBlockMap {
    /// Element bits.
    pub bits: usize,
    /// Columns.
    pub columns: usize,
    /// `(row, column)` of each data element, by `k`.
    pub data: Vec<(usize, usize)>,
    /// `(row, column)` of each training element.
    pub training: Vec<(usize, usize)>,
}

impl AmBlockMap {
    /// A PU, PL, S or T block of `bits` bits per element (25 columns, 750 data and 50 training elements).
    pub fn primary(bits: usize) -> AmBlockMap {
        AmBlockMap {
            bits,
            columns: 25,
            data: (0..750).map(am_row_col).collect(),
            training: (750..800).map(am_row_col).collect(),
        }
    }
    /// A PIDS block: 2 columns (IL, IU) of 30 data and 2 training elements of 4 bits.
    pub fn pids() -> AmBlockMap {
        let mut data = Vec::new();
        let mut training = Vec::new();
        for col in 0..2 {
            data.extend((0..30).map(|k| (am_pids_row(k), col)));
            training.extend(AM_PIDS_TRAINING_ROWS.iter().map(|&r| (r, col)));
        }
        AmBlockMap {
            bits: 4,
            columns: 2,
            data,
            training,
        }
    }
    /// The bit index in a block laid out row by row, element by element: `(row * columns + column) * bits + bit`.
    pub fn index(&self, row: usize, column: usize, bit: usize) -> usize {
        (row * self.columns + column) * self.bits + bit
    }
    /// Bits per block.
    pub fn block_bits(&self) -> usize {
        32 * self.columns * self.bits
    }
    /// Writes the training elements into a block of bits.
    pub fn put_training(&self, block: &mut [u8]) {
        let t = am_training(self.bits);
        for &(r, c) in &self.training {
            for (b, &v) in t.iter().enumerate() {
                block[self.index(r, c, b)] = v;
            }
        }
    }
}

/// The PIDS bits `n` (0 to 239) of a block, as `(column, k, bit)`: the subframes IL (column 0) and IU (column 1) with their bit
/// orders of 1012s figure 10-5.
pub fn am_pids_place(n: usize) -> (usize, usize, usize) {
    const IL: [usize; 12] = [0, 1, 12, 13, 6, 5, 18, 17, 11, 7, 23, 19];
    let group = n / 24;
    let r = n % 24;
    match IL.iter().position(|&x| x == r) {
        Some(pos) => {
            let m = group * 12 + pos;
            (0, (m + m / 60 + 11) % 30, m % 4)
        }
        None => {
            const IU: [usize; 12] = [2, 4, 14, 16, 3, 8, 15, 20, 9, 10, 21, 22];
            let pos = IU.iter().position(|&x| x == r).unwrap_or(0);
            let m = group * 12 + pos;
            (1, (m + m / 60) % 30, m % 4)
        }
    }
}

/// A logical channel's code: the scrambler, the tail-biting convolutional code and its puncturing (NRSC-5 1011s clauses 8 and
/// 9, 1012s clauses 8 and 9). Interleaving is done by the frame layout.
#[derive(Clone, Debug)]
pub struct Channel {
    info: usize,
    code: ConvCode,
    pattern: Puncture<'static>,
    vit: Viterbi,
    quant: Quantizer,
    mother: Vec<u8>,
    soft: Vec<f32>,
    llr: Vec<Llr8>,
}

impl Channel {
    /// A channel of `info` bits with the tail-biting `code` punctured by `pattern`. `None` when the code cannot be decoded at
    /// this length.
    pub fn new(info: usize, code: ConvCode, pattern: Puncture<'static>) -> Option<Channel> {
        if info < code.k() as usize {
            return None;
        }
        let n = code.n();
        Some(Channel {
            info,
            vit: Viterbi::new(code, info).ok()?,
            code,
            pattern,
            quant: Quantizer::default(),
            mother: Vec::with_capacity(info * n),
            soft: vec![0.0; info * n],
            llr: vec![0; info * n],
        })
    }
    /// FM P1: 146 176 bits at rate 2/5 (1011s clause 9.3).
    pub fn fm_p1() -> Channel {
        Self::new(146_176, codes::HD_FM_K7_R3, codes::HD_FM_2_5).unwrap_or_else(|| unreachable!())
    }
    /// FM PIDS: 80 bits at rate 2/5.
    pub fn fm_pids() -> Channel {
        Self::new(80, codes::HD_FM_K7_R3, codes::HD_FM_2_5).unwrap_or_else(|| unreachable!())
    }
    /// FM P3 (MP3): 4608 bits a block pair at rate 1/2 (one transfer frame of 9216 coded bits, 1011s table 10-19).
    pub fn fm_p3() -> Channel {
        Self::new(4608, codes::HD_FM_K7_R3, codes::HD_FM_1_2).unwrap_or_else(|| unreachable!())
    }
    /// AM P1: 3750 bits a block, code E1 at rate 5/12 (1012s clause 9.1.4.1).
    pub fn am_p1() -> Channel {
        Self::new(3750, codes::HD_AM_E1, codes::HD_AM_E1_5_12).unwrap_or_else(|| unreachable!())
    }
    /// AM PIDS: 80 bits a block, code E3 at rate 1/3 (1012s clause 9.1.4.3).
    pub fn am_pids() -> Channel {
        Self::new(80, codes::HD_AM_E2, AM_RATE_THIRD).unwrap_or_else(|| unreachable!())
    }
    /// AM P3 of a frame: MA1 24 000 bits with code E2 at rate 2/3, MA3 30 000 bits with code E1 at rate 5/12 (1012s clause 9.1).
    pub fn am_p3(mode: ServiceMode) -> Channel {
        let c = if mode == ServiceMode::Ma3 {
            Self::new(30_000, codes::HD_AM_E1, codes::HD_AM_E1_5_12)
        } else {
            Self::new(24_000, codes::HD_AM_E2, codes::HD_AM_E2_2_3)
        };
        c.unwrap_or_else(|| unreachable!())
    }
    /// Information bits.
    pub fn info(&self) -> usize {
        self.info
    }
    /// Coded bits.
    pub fn coded(&self) -> usize {
        self.pattern.punctured_len(0, self.info * self.code.n())
    }
    /// Scrambles, encodes and punctures the first `info()` bits of `bits` (missing bits are zeros) into `out`.
    pub fn encode(&mut self, bits: &[u8], out: &mut Vec<u8>) {
        let mut b = vec![0u8; self.info];
        for (o, &x) in b.iter_mut().zip(bits) {
            *o = x & 1;
        }
        Additive::new(seq::HD_RADIO).apply_bits(&mut b);
        self.mother.resize(self.info * self.code.n(), 0);
        let _ = encode_tail_biting(&self.code, &b, &mut self.mother);
        out.clear();
        puncture(&self.pattern, 0, &self.mother, out);
    }
    /// Decodes the LLRs of the coded bits (positive means 0) into the information bits. It returns the channel bit error rate
    /// measured by encoding the decision again (1011s clause 3.4), or `None` when `llr` does not have `coded()` values.
    pub fn decode(&mut self, llr: &[f32], out: &mut Vec<u8>) -> Option<f32> {
        depuncture_frame(&self.pattern, 0, llr, &mut self.soft).ok()?;
        self.quant.quantize_auto(&self.soft, &mut self.llr).ok()?;
        out.clear();
        out.resize(self.info, 0);
        self.vit.decode(&self.llr, Ends::TailBiting, out).ok()?;
        // The channel bit error rate before the decoder.
        let start = tail_biting_state(&self.code, out);
        let errs = reencode_errors(&self.code, start, out, &self.llr).unwrap_or(0);
        Additive::new(seq::HD_RADIO).apply_bits(out);
        Some(errs as f32 / self.coded().max(1) as f32)
    }
}

/// Rate 1/3: every output of the mother code sent.
const AM_RATE_THIRD: Puncture<'static> = Puncture::new(&[1, 1, 1]);

/// Maps bits to the cells of a role, writing into `grid`.
pub fn map_bits(cells: &[Cell], bits: &[u8], grid: &mut [Complex32]) {
    let mut p = 0;
    for c in cells {
        let n = c.qam.bits();
        c.put(c.qam.map(bits.get(p..p + n).unwrap_or(&[])), grid);
        p += n;
    }
}

/// The LLRs (positive means 0) of the bits of the cells of a role, from equalised cells and their weights.
pub fn soft_bits(cells: &[Cell], z: &[Complex32], w: &[f32], out: &mut Vec<f32>) {
    out.clear();
    for c in cells {
        let (v, wt) = c.value(z, w);
        c.qam.demap(v, wt, out);
    }
}

/// The subframes of a 72 000-bit input (8 P1 transfer frames, or one MA3 P3 transfer frame): the input index of each bit of BL,
/// ML, BU and MU (EBL, EML, EBU and EMU for P3), by `index mod 12` (1012s figures 10-4 and 10-6).
pub fn am_subframes() -> [Vec<u32>; 4] {
    const LISTS: [[usize; 3]; 4] = [[2, 1, 5], [11, 6, 7], [10, 8, 9], [4, 3, 0]];
    LISTS.map(|l| {
        (0..72_000 / 12)
            .flat_map(|g| l.iter().map(move |&r| (12 * g + r) as u32))
            .collect()
    })
}

/// The subframes EL and EU of an MA1 P3 transfer frame (36 000 bits), by `index mod 6` (1012s figure 10-4).
pub fn am_p3_ma1_subframes() -> [Vec<u32>; 2] {
    let el = (0..6000)
        .flat_map(|g| [6 * g, 6 * g + 1])
        .map(|x| x as u32)
        .collect();
    let eu = (0..6000)
        .flat_map(|g| [6 * g + 2, 6 * g + 3, 6 * g + 5, 6 * g + 4])
        .map(|x| x as u32)
        .collect();
    [el, eu]
}

/// The place `(block, k, bit)` of bit `n` of subframe `sub` (0 BL, 1 ML, 2 BU, 3 MU) of P1 in PL (BL, ML) or PU (BU, MU) (1012s
/// figure 10-4).
pub fn am_p1_place(sub: usize, n: usize) -> (usize, usize, usize) {
    match sub {
        0 => (n / 2250, (n + n / 750 + 1) % 750, n % 3),
        1 => ((3 * n + 3) % 8, (n + n / 3000 + 3) % 750, 3 + n % 3),
        2 => (n / 2250, (n + n / 750) % 750, n % 3),
        _ => ((3 * n) % 8, (n + n / 3000 + 2) % 750, 3 + n % 3),
    }
}

/// The place `(block, k, bit)` of bit `n` of the MA3 P3 subframe `sub` (0 EBL, 1 EML in T; 2 EBU, 3 EMU in S) (1012s figure
/// 10-6).
pub fn am_p3_ma3_place(sub: usize, n: usize) -> (usize, usize, usize) {
    let half = if sub.is_multiple_of(2) { 0 } else { 3 };
    if sub < 2 {
        ((3 * n + 3) % 8, (n + n / 3000 + 3) % 750, half + n % 3)
    } else {
        ((3 * n) % 8, (n + n / 3000 + 2) % 750, half + n % 3)
    }
}

/// The place `(block, k, bit)` of bit `n` of the MA1 P3 subframe EL (`sub` 0, in T) or EU (`sub` 1, in S) (1012s figure 10-4).
pub fn am_p3_ma1_place(sub: usize, n: usize) -> (usize, usize, usize) {
    if sub == 0 {
        ((3 * n + n / 3000) % 8, (n + n / 6000) % 750, n % 2)
    } else {
        (
            (3 * n + n / 3000 + 2 * (n / 12_000)) % 8,
            (n + n / 6000) % 750,
            n % 4,
        )
    }
}

/// Frames of digital diversity delay of the backup subframes (1012s clause 3.5: Ndd = 3).
pub const AM_DIVERSITY_FRAMES: usize = 3;

/// The PX1 convolutional interleaver of MP3 (Interleaver IV of 1011s clause 10.2.5, table 10-19): J = 4, B = 32, C = 36, M = 2,
/// v = [0, 1, 2, 3], b = 9216, N = 147 456. Input bit `i` of a cycle of N is written at [`Px1::write_position`]; the internal
/// matrix is read row by row, one bit for each bit written.
#[derive(Clone, Debug)]
pub struct Px1 {
    /// For each input index of a cycle, the linear position it is written to.
    pos: Vec<u32>,
}

impl Px1 {
    /// Bits a cycle (the internal matrix).
    pub const N: usize = 147_456;
    /// Bits a transfer frame.
    pub const B: usize = 9216;

    /// The interleaver.
    pub fn new() -> Px1 {
        let (j, bb, c, m) = (4usize, 32usize, 36usize, 2usize);
        let bk_bits = 32 * c;
        let bk_adj = bk_bits - 1;
        let mut pt = [0usize; 4];
        let pos = (0..Self::N)
            .map(|i| {
                let partition = (i / m) % j;
                let p = pt[partition];
                pt[partition] += 1;
                let block = (p + 7 * partition + bb * bk_adj - bk_adj * (p / bk_bits)) % bb;
                let w = (11 * p) % bk_bits;
                let (row, col) = (w / c, (11 * p) % c);
                ((block * 32 + row) * (j * c) + partition * c + col) as u32
            })
            .collect();
        Px1 { pos }
    }
    /// The linear position (row * 144 + column) input bit `i` of a cycle is written to.
    pub fn write_position(&self, i: usize) -> usize {
        self.pos[i % Self::N] as usize
    }
}

impl Default for Px1 {
    fn default() -> Self {
        Self::new()
    }
}

/// The AM interleaving of one service mode at the frame level (1012s clause 10): the subframes, their places in the PU, PL, S, T
/// and PIDS matrices with the training elements, and the three frames of digital diversity delay of the backup subframes. The
/// matrices are laid out per block in the order of [`cells`]: P1 cells `PU 0, PL 0, PU 1, PL 1 ...` of 6 bits, P3 cells
/// `S 0, T 0, S 1 ...`, PIDS cells `IL, IU` of 4 bits, symbol by symbol.
#[derive(Clone, Debug)]
pub struct AmFrames {
    mode: ServiceMode,
    /// For each of the four P1 subframes, the input index (of 72 000) of each bit, and its block-array position.
    p1_src: [Vec<u32>; 4],
    p1_dst: [Vec<u32>; 4],
    /// P3: the input index and position of each bit of each subframe (MA1: EL, EU; MA3: EBL, EML, EBU, EMU).
    p3_src: Vec<Vec<u32>>,
    p3_dst: Vec<Vec<u32>>,
    /// PIDS: the position of each of the 240 bits in a block.
    pids_dst: Vec<u32>,
    /// Training bits: (position, bit) per block of P1, P3 and PIDS.
    p1_training: Vec<(u32, u8)>,
    p3_training: Vec<(u32, u8)>,
    pids_training: Vec<(u32, u8)>,
    p3_cell_bits: usize,
}

/// Bits of the P1 cells of an AM block: 32 symbols of 50 cells of 6 bits.
pub const AM_P1_BLOCK: usize = 32 * 50 * 6;
/// Bits of the PIDS cells of an AM block: 32 symbols of 2 cells of 4 bits.
pub const AM_PIDS_BLOCK: usize = 32 * 2 * 4;

impl AmFrames {
    /// The interleaving of `mode` (MA1 or MA3).
    pub fn new(mode: ServiceMode) -> AmFrames {
        let ma3 = mode == ServiceMode::Ma3;
        let (sb, tb) = if ma3 { (6, 6) } else { (4, 2) };
        let p3_cell_bits = sb + tb;
        let pos_p1 = |m: usize, b: usize, k: usize, p: usize| {
            let (r, c) = am_row_col(k);
            (b * AM_P1_BLOCK + (r * 50 + 2 * c + m) * 6 + p) as u32
        };
        let p3_block = 32 * 25 * p3_cell_bits;
        let pos_p3 = |t: bool, b: usize, k: usize, p: usize| {
            let (r, c) = am_row_col(k);
            (b * p3_block + (r * 25 + c) * p3_cell_bits + if t { sb } else { 0 } + p) as u32
        };
        let subs = am_subframes();
        let p1_dst = std::array::from_fn(|sub| {
            (0..18_000)
                .map(|n| {
                    let (b, k, p) = am_p1_place(sub, n);
                    pos_p1(if sub < 2 { 1 } else { 0 }, b, k, p)
                })
                .collect()
        });
        let (p3_src, p3_dst): (Vec<Vec<u32>>, Vec<Vec<u32>>) = if ma3 {
            (
                subs.to_vec(),
                (0..4)
                    .map(|sub| {
                        (0..18_000)
                            .map(|n| {
                                let (b, k, p) = am_p3_ma3_place(sub, n);
                                pos_p3(sub < 2, b, k, p)
                            })
                            .collect()
                    })
                    .collect(),
            )
        } else {
            let s = am_p3_ma1_subframes();
            (
                s.to_vec(),
                (0..2)
                    .map(|sub| {
                        (0..s[sub].len())
                            .map(|n| {
                                let (b, k, p) = am_p3_ma1_place(sub, n);
                                pos_p3(sub == 0, b, k, p)
                            })
                            .collect()
                    })
                    .collect(),
            )
        };
        let pids_dst = (0..240)
            .map(|n| {
                let (col, k, p) = am_pids_place(n);
                ((am_pids_row(k) * 2 + col) * 4 + p) as u32
            })
            .collect();
        let mut p1_training = Vec::new();
        let mut p3_training = Vec::new();
        for k in 750..800 {
            let (r, c) = am_row_col(k);
            for m in 0..2 {
                for (p, &v) in am_training(6).iter().enumerate() {
                    p1_training.push((((r * 50 + 2 * c + m) * 6 + p) as u32, v));
                }
            }
            for (p, &v) in am_training(sb).iter().enumerate() {
                p3_training.push((((r * 25 + c) * p3_cell_bits + p) as u32, v));
            }
            for (p, &v) in am_training(tb).iter().enumerate() {
                p3_training.push((((r * 25 + c) * p3_cell_bits + sb + p) as u32, v));
            }
        }
        let mut pids_training = Vec::new();
        for row in AM_PIDS_TRAINING_ROWS {
            for col in 0..2 {
                for (p, &v) in am_training(4).iter().enumerate() {
                    pids_training.push((((row * 2 + col) * 4 + p) as u32, v));
                }
            }
        }
        AmFrames {
            mode,
            p1_src: subs,
            p1_dst,
            p3_src,
            p3_dst,
            pids_dst,
            p1_training,
            p3_training,
            pids_training,
            p3_cell_bits,
        }
    }
    /// The service mode.
    pub fn mode(&self) -> ServiceMode {
        self.mode
    }
    /// Bits of the P3 cells of a block.
    pub fn p3_block(&self) -> usize {
        32 * 25 * self.p3_cell_bits
    }
    /// Whether P3 subframe `sub` is a backup subframe (delayed by [`AM_DIVERSITY_FRAMES`]).
    fn p3_backup(&self, sub: usize) -> bool {
        self.mode == ServiceMode::Ma3 && sub.is_multiple_of(2)
    }
    /// Fills the P1 cells of a frame (`8 * AM_P1_BLOCK` bits) from the 72 000 coded bits of this frame's eight transfer frames
    /// (main subframes) and of the frame [`AM_DIVERSITY_FRAMES`] before it (backup subframes), with the training bits.
    pub fn p1_frame(&self, now: &[u8], before: &[u8], out: &mut [u8]) {
        for b in 0..8 {
            for &(pos, v) in &self.p1_training {
                out[b * AM_P1_BLOCK + pos as usize] = v;
            }
        }
        for sub in 0..4 {
            let src = if sub % 2 == 0 { before } else { now };
            for (&i, &d) in self.p1_src[sub].iter().zip(&self.p1_dst[sub]) {
                out[d as usize] = src.get(i as usize).copied().unwrap_or(0);
            }
        }
    }
    /// The inverse of [`AmFrames::p1_frame`]: the LLRs of the main subframes of this frame go to `now`, those of the backup
    /// subframes (of the frame [`AM_DIVERSITY_FRAMES`] before) to `before` (72 000 each).
    pub fn p1_unframe(&self, llr: &[f32], now: &mut [f32], before: &mut [f32]) {
        for sub in 0..4 {
            let dst = if sub % 2 == 0 {
                &mut *before
            } else {
                &mut *now
            };
            for (&i, &d) in self.p1_src[sub].iter().zip(&self.p1_dst[sub]) {
                dst[i as usize] = llr[d as usize];
            }
        }
    }
    /// Fills the P3 cells of a frame from this frame's P3 transfer frame and, in MA3, the backup subframes of the frame
    /// [`AM_DIVERSITY_FRAMES`] before.
    pub fn p3_frame(&self, now: &[u8], before: &[u8], out: &mut [u8]) {
        let blk = self.p3_block();
        for b in 0..8 {
            for &(pos, v) in &self.p3_training {
                out[b * blk + pos as usize] = v;
            }
        }
        for sub in 0..self.p3_src.len() {
            let src = if self.p3_backup(sub) { before } else { now };
            for (&i, &d) in self.p3_src[sub].iter().zip(&self.p3_dst[sub]) {
                out[d as usize] = src.get(i as usize).copied().unwrap_or(0);
            }
        }
    }
    /// The inverse of [`AmFrames::p3_frame`].
    pub fn p3_unframe(&self, llr: &[f32], now: &mut [f32], before: &mut [f32]) {
        for sub in 0..self.p3_src.len() {
            let dst = if self.p3_backup(sub) {
                &mut *before
            } else {
                &mut *now
            };
            for (&i, &d) in self.p3_src[sub].iter().zip(&self.p3_dst[sub]) {
                dst[i as usize] = llr[d as usize];
            }
        }
    }
    /// Fills the PIDS cells of a block from its 240 coded bits, with the training bits.
    pub fn pids_block(&self, coded: &[u8], out: &mut [u8]) {
        for &(pos, v) in &self.pids_training {
            out[pos as usize] = v;
        }
        for (n, &d) in self.pids_dst.iter().enumerate() {
            out[d as usize] = coded.get(n).copied().unwrap_or(0);
        }
    }
    /// The inverse of [`AmFrames::pids_block`].
    pub fn pids_unblock(&self, llr: &[f32], out: &mut [f32]) {
        for (n, &d) in self.pids_dst.iter().enumerate() {
            out[n] = llr[d as usize];
        }
    }
    /// Every position of the P1, P3 and PIDS cells and how often the layout uses it (data and training), for checks.
    pub fn usage(&self) -> (Vec<u8>, Vec<u8>, Vec<u8>) {
        let mut p1 = vec![0u8; 8 * AM_P1_BLOCK];
        for d in self.p1_dst.iter().flatten() {
            p1[*d as usize] += 1;
        }
        for b in 0..8 {
            for &(pos, _) in &self.p1_training {
                p1[b * AM_P1_BLOCK + pos as usize] += 1;
            }
        }
        let mut p3 = vec![0u8; 8 * self.p3_block()];
        for d in self.p3_dst.iter().flatten() {
            p3[*d as usize] += 1;
        }
        for b in 0..8 {
            for &(pos, _) in &self.p3_training {
                p3[b * self.p3_block() + pos as usize] += 1;
            }
        }
        let mut pids = vec![0u8; AM_PIDS_BLOCK];
        for &d in &self.pids_dst {
            pids[d as usize] += 1;
        }
        for &(pos, _) in &self.pids_training {
            pids[pos as usize] += 1;
        }
        (p1, p3, pids)
    }
}
