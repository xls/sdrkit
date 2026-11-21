//! The constants of the BGAN forward link (ETSI TS 102 744-2-1): the 13 bearers (tables 5.1, 5.2 and 5.10), the 15 subtypes and
//! their unique words (figure 5.15), the constellations (tables 5.3 to 5.7) and the 118 published code tables of Annex C (turbo
//! interleaver, puncturing, channel interleaving and symbol mapping), read from the crate's data file.

use sdrkit_core::Complex32;
use std::sync::OnceLock;

/// The 118 forward tables of Annex C as one file: the magic `BGANSL1\0`, the table count (u16), then per table the name length
/// (u8), the name, N (u16), the symbols per block (u16), the bits per symbol (u8), 1 when the columns are labelled `b` (u8), the
/// N interleaver entries (u16 each) and the symbols x bits raw-vector indices of the mapping (u16 each). Little-endian.
static TABLE_FILE: &[u8] = include_bytes!("../../../data/bgan_forward_tables.bin");

/// The subtypes L8 to L1, R and H1 to H6, by index 0 to 14; the code rate value of table 5.20 is `index - 8`.
pub const SUBTYPES: [&str; 15] = [
    "L8", "L7", "L6", "L5", "L4", "L3", "L2", "L1", "R", "H1", "H2", "H3", "H4", "H5", "H6",
];

/// The index of subtype R.
pub const R: usize = 8;

/// The unique words of figure 5.15, 40 bits each, by subtype index (L8 first).
pub const UNIQUE_WORDS: [u64; 15] = [
    0xE4_564A_DABD,
    0xBE_D8B3_EAD2,
    0xF2_F5F4_96A6,
    0xC9_1136_428A,
    0xF9_A42B_B1AB,
    0xD4_E357_299C,
    0x4C_B9D9_D174,
    0x6A_AF7A_6E4E,
    0xC2_40E9_6587,
    0x51_4BB8_BA62,
    0xB5_896C_CDDF,
    0xA8_7B0D_A6C9,
    0x5A_1A67_9D6F,
    0x61_FEA5_4943,
    0xA3_2AD2_81C4,
];

/// Symbols of a unique word.
pub const UW_LEN: usize = 40;

/// The bit `i` (symbol `i` of the word) of unique word `w`: with `msb_first` the first symbol is the most significant bit of
/// the hex as printed, else the least significant.
pub fn uw_bit(w: usize, i: usize, msb_first: bool) -> u8 {
    let b = if msb_first { UW_LEN - 1 - i } else { i };
    (UNIQUE_WORDS[w] >> b & 1) as u8
}

/// A modulation of the forward link.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Modulation {
    /// QPSK, table 5.6 (also the 4-QAM blocks of the X4/16 bearers).
    Qpsk,
    /// 16-QAM, table 5.3.
    Qam16,
    /// 32-QAM, table 5.4.
    Qam32,
    /// 64-QAM, table 5.5.
    Qam64,
}

impl Modulation {
    /// Bits per symbol.
    pub fn bits(self) -> usize {
        match self {
            Modulation::Qpsk => 2,
            Modulation::Qam16 => 4,
            Modulation::Qam32 => 5,
            Modulation::Qam64 => 6,
        }
    }

    /// The points by symbol value (`b[n-1] .. b0`), mean power 1.
    pub fn points(self) -> Vec<Complex32> {
        let unit = |d2: f64, v: &[(i8, i8)]| -> Vec<Complex32> {
            v.iter()
                .map(|&(i, q)| Complex32::new((f64::from(i) * d2) as f32, (f64::from(q) * d2) as f32))
                .collect()
        };
        match self {
            Modulation::Qpsk => unit(0.5f64.sqrt(), &[(1, 1), (1, -1), (-1, 1), (-1, -1)]),
            Modulation::Qam16 => unit(
                0.1f64.sqrt(),
                &[
                    (-1, -1),
                    (-1, -3),
                    (-1, 1),
                    (-1, 3),
                    (-3, -1),
                    (-3, -3),
                    (-3, 1),
                    (-3, 3),
                    (1, -1),
                    (1, -3),
                    (1, 1),
                    (1, 3),
                    (3, -1),
                    (3, -3),
                    (3, 1),
                    (3, 3),
                ],
            ),
            Modulation::Qam32 => unit(
                0.05f64.sqrt(),
                &[
                    (-1, 1),
                    (1, 1),
                    (-1, -1),
                    (1, -1),
                    (-1, 3),
                    (1, 3),
                    (-1, 5),
                    (1, 5),
                    (-1, -5),
                    (1, -5),
                    (-1, -3),
                    (1, -3),
                    (-3, 3),
                    (3, 3),
                    (-3, 5),
                    (3, 5),
                    (-3, 1),
                    (3, 1),
                    (-3, -1),
                    (3, -1),
                    (-5, 1),
                    (5, 1),
                    (-5, -1),
                    (5, -1),
                    (-3, -5),
                    (3, -5),
                    (-3, -3),
                    (3, -3),
                    (-5, 3),
                    (5, 3),
                    (-5, -3),
                    (5, -3),
                ],
            ),
            Modulation::Qam64 => {
                const LEVEL: [i8; 8] = [3, 1, 5, 7, -3, -1, -5, -7];
                let v: Vec<(i8, i8)> = (0..64).map(|x| (LEVEL[x >> 3], LEVEL[x & 7])).collect();
                unit((1.0 / 42.0f64).sqrt(), &v)
            }
        }
    }
}

/// A family of Annex C tables: the file-name prefix, its modulation and whether its names end in `_4B`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Family {
    /// The file-name prefix (F80T025Q, FR80T25X16 ...).
    pub prefix: &'static str,
    /// The modulation of its blocks.
    pub modulation: Modulation,
    /// Names end in `_4B`.
    pub four_block: bool,
}

const fn fam(prefix: &'static str, modulation: Modulation, four_block: bool) -> Family {
    Family {
        prefix,
        modulation,
        four_block,
    }
}

/// A forward bearer (tables 5.1, 5.2 and 5.10; FBearerType of TS 102 744-3-1 clause 5.7.31.2).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Bearer {
    /// The name (F80T0.25Q-1B ...).
    pub name: &'static str,
    /// The FBearerType value; `None` for F80T0.25Q-1B.
    pub fbearer_type: Option<u8>,
    /// Symbols per second.
    pub rate: f64,
    /// The roll-off of the root raised cosine.
    pub rolloff: f64,
    /// Symbols per frame (MOS).
    pub mos: usize,
    /// Pilots per frame (PS).
    pub ps: usize,
    /// Data symbols between pilots (DS).
    pub ds: usize,
    /// Symbols per FEC block (TEO).
    pub teo: usize,
    /// FEC blocks per frame.
    pub blocks: usize,
    /// The channel bandwidth, Hz.
    pub channel_hz: f64,
    /// Data symbols are spread over the blocks by the outer interleaver.
    pub outer: bool,
    /// The family of the QPSK blocks of a bearer with dynamic modulation, or of every block; `None` when no table is published.
    pub family: Option<Family>,
    /// The family of the 16-QAM blocks of a bearer with dynamic modulation.
    pub qam_family: Option<Family>,
    /// Bit 0 of a unique word is the upper-right point (the Q bearers); else bit 1 is.
    pub zero_upper: bool,
    /// The unique word and pilot amplitude per axis.
    pub uw_amp: f64,
}

impl Bearer {
    /// The modulation of a block of the given family choice (`qam` picks the 16-QAM family of a dynamic bearer).
    pub fn block_family(&self, qam: bool) -> Option<Family> {
        if qam {
            self.qam_family.or(self.family)
        } else {
            self.family
        }
    }

    /// The pilot (the upper-right unique-word point).
    pub fn pilot(&self) -> Complex32 {
        Complex32::new(self.uw_amp as f32, self.uw_amp as f32)
    }

    /// The unique-word symbol of `bit`.
    pub fn uw_symbol(&self, bit: u8) -> Complex32 {
        if (bit == 0) == self.zero_upper {
            self.pilot()
        } else {
            -self.pilot()
        }
    }

    /// Data symbols per frame (TEO x blocks).
    pub fn data_symbols(&self) -> usize {
        self.teo * self.blocks
    }

    /// The data symbols after the last pilot: TEO x blocks - PS x DS.
    pub fn tail(&self) -> usize {
        self.data_symbols() - self.ps * self.ds
    }

    /// The frame position of data symbol `d`: after the unique word, PS times (DS data symbols, one pilot), then the tail.
    pub fn data_position(&self, d: usize) -> usize {
        let comb = self.ps * self.ds;
        if d < comb {
            UW_LEN + (d / self.ds) * (self.ds + 1) + d % self.ds
        } else {
            UW_LEN + self.ps * (self.ds + 1) + (d - comb)
        }
    }

    /// The frame position of pilot `k`.
    pub fn pilot_position(&self, k: usize) -> usize {
        UW_LEN + k * (self.ds + 1) + self.ds
    }

    /// The block and the symbol in it of data symbol `d` of the frame.
    pub fn block_symbol(&self, d: usize) -> (usize, usize) {
        if self.outer {
            (d % self.blocks, d / self.blocks)
        } else {
            (d / self.teo, d % self.teo)
        }
    }

    /// The bearer has dynamic modulation (QPSK or 16-QAM per block).
    pub fn dynamic(&self) -> bool {
        self.qam_family.is_some()
    }

    /// An FR bearer (84 or 168 kBd): a ForwardBearerCodeRate applies to the next frame.
    pub fn fr(&self) -> bool {
        self.outer
    }
}

const Q025: Family = fam("F80T025Q", Modulation::Qpsk, false);
const Q1: Family = fam("F80T1Q", Modulation::Qpsk, true);
const X1: Family = fam("F80T1X", Modulation::Qam16, true);
const X45: Family = fam("F80T45X", Modulation::Qam16, false);
const R25X4: Family = fam("FR80T25X4", Modulation::Qpsk, false);
const R25X16: Family = fam("FR80T25X16", Modulation::Qam16, false);
const R25X32: Family = fam("FR80T25X32", Modulation::Qam32, false);
const R25X64: Family = fam("FR80T25X64", Modulation::Qam64, false);
const R5X4: Family = fam("FR80T5X4", Modulation::Qpsk, false);
const R5X16: Family = fam("FR80T5X16", Modulation::Qam16, false);
const R5X32: Family = fam("FR80T5X32", Modulation::Qam32, false);
const R5X64: Family = fam("FR80T5X64", Modulation::Qam64, false);

const A_QPSK: f64 = std::f64::consts::FRAC_1_SQRT_2;
/// 3 D / 2 of 16-QAM: 3 / sqrt 10.
const A_16: f64 = 0.948_683_298_050_513_8;

#[allow(clippy::too_many_arguments)]
const fn bearer(
    name: &'static str,
    fbearer_type: Option<u8>,
    rate: f64,
    mos: usize,
    ps: usize,
    ds: usize,
    teo: usize,
    blocks: usize,
    channel_hz: f64,
    family: Option<Family>,
    qam_family: Option<Family>,
    zero_upper: bool,
    uw_amp: f64,
) -> Bearer {
    let fr = rate == 84_000.0 || rate == 168_000.0;
    Bearer {
        name,
        fbearer_type,
        rate,
        rolloff: if fr { 0.13 } else { 0.25 },
        mos,
        ps,
        ds,
        teo,
        blocks,
        channel_hz,
        outer: fr,
        family,
        qam_family,
        zero_upper,
        uw_amp,
    }
}

/// The 13 forward bearers in the order of table 5.10.
pub const BEARERS: [Bearer; 13] = [
    bearer("F80T0.25Q-1B", None, 8_400.0, 672, 88, 6, 544, 1, 10_500.0, Some(Q025), None, true, A_QPSK),
    bearer("F80T1Q-1B", Some(4), 33_600.0, 2688, 88, 29, 2560, 1, 42_000.0, None, None, true, A_QPSK),
    bearer("F80T1Q-4B", Some(1), 33_600.0, 2688, 88, 29, 640, 4, 42_000.0, Some(Q1), None, true, A_QPSK),
    bearer("F80T1X-4B", Some(2), 33_600.0, 2688, 88, 29, 640, 4, 42_000.0, Some(X1), None, false, A_16),
    bearer("FR80T2.5X4/16-5B", Some(5), 84_000.0, 6720, 90, 73, 1318, 5, 94_920.0, Some(R25X4), Some(R25X16), false, A_QPSK),
    bearer("FR80T2.5X16-5B", Some(6), 84_000.0, 6720, 90, 73, 1318, 5, 94_920.0, Some(R25X16), None, false, A_16),
    bearer("FR80T2.5X32-6B", Some(7), 84_000.0, 6720, 92, 71, 1098, 6, 94_920.0, Some(R25X32), None, false, 0.922),
    bearer("FR80T2.5X64-7B", Some(8), 84_000.0, 6720, 93, 70, 941, 7, 94_920.0, Some(R25X64), None, false, 0.9386),
    bearer("F80T4.5X-8B", Some(3), 151_200.0, 12_096, 88, 136, 1496, 8, 189_000.0, Some(X45), None, false, A_16),
    bearer("FR80T5X4/16-9B", Some(9), 168_000.0, 13_440, 89, 149, 1479, 9, 189_840.0, Some(R5X4), Some(R5X16), false, A_QPSK),
    bearer("FR80T5X16-9B", Some(10), 168_000.0, 13_440, 89, 149, 1479, 9, 189_840.0, Some(R5X16), None, false, A_16),
    bearer("FR80T5X32-11B", Some(11), 168_000.0, 13_440, 90, 147, 1210, 11, 189_840.0, Some(R5X32), None, false, 0.922),
    bearer("FR80T5X64-13B", Some(12), 168_000.0, 13_440, 88, 151, 1024, 13, 189_840.0, Some(R5X64), None, false, 0.9386),
];

/// The bearer named `name`.
pub fn bearer_by_name(name: &str) -> Option<&'static Bearer> {
    BEARERS.iter().find(|b| b.name == name)
}

/// The name of FBearerType `t` (TS 102 744-3-1 clause 5.7.31.2), or "reserved".
pub fn fbearer_type_name(t: u8) -> &'static str {
    BEARERS
        .iter()
        .find(|b| b.fbearer_type == Some(t))
        .map_or("reserved", |b| b.name)
}

/// The ReturnBearerTypeShort names 0 to 15 (TS 102 744-3-1 clause 5.4.4.5).
pub const RETURN_BEARERS: [&str; 16] = [
    "R20/80T0.5Q",
    "R20T1Q",
    "R5/20T2Q",
    "R5/20T4.5Q",
    "R20/80T1Q",
    "R5/20T1X",
    "R5/20T2X",
    "R5/20T4.5X",
    "R80T2.5X4",
    "R80T5X4",
    "R80T2.5X16",
    "R80T5X16",
    "R80T2.5X32",
    "R80T5X32",
    "R80T2.5X64",
    "R80T5X64",
];

/// One code table of Annex C (C.1 and C.2).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CodeTable {
    /// The published name (F80T45XH1, F80T1QRE_4B ...).
    pub name: String,
    /// Data bits plus 4 flush bits.
    pub n: usize,
    /// Symbols per block.
    pub symbols: usize,
    /// Bits per symbol.
    pub bits: usize,
    /// The columns are labelled `b[n-1] .. b0`; else `I1 I0 Q1 Q0` or `I0 Q0`.
    pub b_labelled: bool,
    /// C.1: q-bit `i` reads data bit `tci[i]`.
    pub tci: Vec<u32>,
    /// C.2: symbol `s` takes column `j` from the raw vector at `cipm[s bits + j]` (d at 0, p at N, q at 2N).
    pub cipm: Vec<u32>,
}

impl CodeTable {
    /// Information bits per block (N - 4).
    pub fn k(&self) -> usize {
        self.n - 4
    }

    /// Parity bits of each kind that are sent.
    pub fn parity_sent(&self) -> usize {
        self.cipm.iter().filter(|&&x| (x as usize) >= self.n && (x as usize) < 2 * self.n).count()
    }
}

/// An error of the table file.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TableFileError(pub usize);

impl std::fmt::Display for TableFileError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "the BGAN table file is damaged at byte {}", self.0)
    }
}

impl std::error::Error for TableFileError {}

/// Reads the table file format of [`TABLE_FILE`] from `bytes`.
pub fn parse_tables(bytes: &[u8]) -> Result<Vec<CodeTable>, TableFileError> {
    let mut at = 0usize;
    let mut take = |n: usize| -> Result<&[u8], TableFileError> {
        let s = bytes.get(at..at + n).ok_or(TableFileError(at))?;
        at += n;
        Ok(s)
    };
    if take(8)? != b"BGANSL1\0" {
        return Err(TableFileError(0));
    }
    let u16_of = |s: &[u8]| u16::from_le_bytes([s[0], s[1]]) as usize;
    let count = u16_of(take(2)?);
    let mut out = Vec::with_capacity(count);
    for _ in 0..count {
        let len = take(1)?[0] as usize;
        let name = String::from_utf8_lossy(take(len)?).into_owned();
        let h = take(6)?;
        let (n, symbols, bits, b_labelled) = (u16_of(&h[0..2]), u16_of(&h[2..4]), h[4] as usize, h[5] == 1);
        let tci: Vec<u32> = take(2 * n)?.as_chunks::<2>().0.iter().map(|c| u16_of(c) as u32).collect();
        let cipm: Vec<u32> = take(2 * symbols * bits)?
            .as_chunks::<2>()
            .0
            .iter()
            .map(|c| u16_of(c) as u32)
            .collect();
        out.push(CodeTable {
            name,
            n,
            symbols,
            bits,
            b_labelled,
            tci,
            cipm,
        });
    }
    Ok(out)
}

/// The 118 forward tables of the crate's data file.
pub fn tables() -> &'static [CodeTable] {
    static T: OnceLock<Vec<CodeTable>> = OnceLock::new();
    T.get_or_init(|| parse_tables(TABLE_FILE).unwrap_or_default())
}

/// The published name of the table of `family` and subtype index `sub`.
pub fn table_name(family: &Family, sub: usize) -> String {
    let s = if sub == R { "RE" } else { SUBTYPES[sub] };
    format!(
        "{}{}{}",
        family.prefix,
        s,
        if family.four_block { "_4B" } else { "" }
    )
}

/// The index in [`tables`] of the table of `family` and subtype index `sub`, when it is published.
pub fn table_index(family: &Family, sub: usize) -> Option<usize> {
    static INDEX: OnceLock<std::collections::HashMap<String, usize>> = OnceLock::new();
    let map = INDEX.get_or_init(|| {
        tables()
            .iter()
            .enumerate()
            .map(|(i, t)| (t.name.clone(), i))
            .collect()
    });
    if sub >= SUBTYPES.len() {
        return None;
    }
    map.get(&table_name(family, sub)).copied()
}

/// The forward frequency of a channel number (offset bit 15, channel index bits 14-0), Hz (TS 102 744-3-1 clause 5.7.31.4).
pub fn forward_hz(channel: u16) -> f64 {
    1510.0e6 + f64::from(channel & 0x7FFF) * 2500.0 + f64::from(channel >> 15) * 1250.0
}

/// The return frequency of a channel number, Hz (clause 5.4.4.1.2); `None` for 0 and FFFF (unavailable).
pub fn return_hz(channel: u16) -> Option<f64> {
    if channel == 0 || channel == 0xFFFF {
        return None;
    }
    Some(1611.5e6 + f64::from(channel & 0x7FFF) * 2500.0 + f64::from(channel >> 15) * 1250.0)
}

/// The channel number of a forward frequency in Hz (the nearest 1.25 kHz step).
pub fn forward_channel(hz: f64) -> u16 {
    let steps = ((hz - 1510.0e6) / 1250.0).round().max(0.0) as u32;
    (((steps & 1) << 15) | (steps >> 1)) as u16
}

/// Short slots reserved by each slot plan 0 to 15 of a ReturnSchedule (figure 5.47; a long slot counts as four).
pub const PLAN_SHORT_RESERVED: [u8; 16] = [0, 8, 8, 7, 6, 5, 4, 8, 7, 6, 5, 4, 3, 2, 1, 0];
/// Reserved slots of each slot plan 0 to 15 (a long slot counts once): the count of res-plan bits and tbcn-ids.
pub const PLAN_RESERVED: [u8; 16] = [0, 2, 5, 4, 3, 2, 1, 8, 7, 6, 5, 4, 3, 2, 1, 0];
