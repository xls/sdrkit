//! The HamDRM frame as the "DRM mode HAM" document fixes it: the timing, the carriers, the cell maps, the FAC fields and the MSC
//! capacity. What the document does not change follows ES 201 980.
//!
//! Standard: HB9TLK, DRM mode "HAM" specification (<http://www.qslnet.de/member1/hb9tlk/drm_h.html>), with ETSI ES 201 980.

use crate::drm::mlc::{Mlc, Rate};
use crate::drm::params::{level_rates, Qam, Robustness};
use sdrkit_util_bits::crc::catalog::CRC8_SAE_J1850;
use sdrkit_util_bits::pack::{push_u64, to_u64};

/// The OFDM rate of the receiver and the generator: a quarter of the document's 48 kHz.
pub const RATE: f64 = 12_000.0;
/// The sample rate of the document's timing ("Timing": "Based on 48'000 sample/sec").
pub const DOC_RATE: f64 = 48_000.0;
/// Samples per frame at 48 kHz ("Timing": "Total samples per frame = 19200").
pub const FRAME_SAMPLES_48K: usize = 19_200;
/// FFT_SIZE at 48 kHz of modes A, B and E ("Timing").
pub const FFT_SIZE_48K: [usize; 3] = [1152, 1024, 640];
/// SYM_PER_FRAME of modes A, B and E ("Timing").
pub const SYM_PER_FRAME: [usize; 3] = [15, 15, 20];
/// ENUM_TG_TU and DENOM_TG_TU of modes A, B and E ("Timing"): the guard interval is FFT_SIZE times this ratio.
pub const TG_TU: [(usize, usize); 3] = [(1, 9), (1, 4), (1, 2)];
/// Number of carriers of modes A, B and E at 2.3 kHz (spectrum occupancy 0) and 2.5 kHz (1) ("Carrier Config").
pub const CARRIERS: [[usize; 2]; 3] = [[53, 57], [45, 51], [29, 31]];
/// "tot MSC cells" of modes A, B and E at 2.3 and 2.5 kHz as the document prints them ("MSC capacity"). The map of mode A at
/// 2.5 kHz has 704 MSC cells, and the printed bit counts of that occupancy follow from 704.
pub const MSC_CELLS_PRINTED: [[usize; 2]; 3] = [[647, 707], [455, 530], [339, 369]];
/// MSC bits per frame as the document prints them ("MSC capacity"), for modes A, B and E at 2.3 and 2.5 kHz, in the order
/// protection 0 16-QAM, protection 0 64-QAM, protection 1 4-QAM, protection 1 16-QAM, protection 1 64-QAM.
pub const CAPACITY_PRINTED: [[[usize; 5]; 2]; 3] = [
    [[1281, 1921, 768, 1601, 2305], [1395, 2094, 837, 1745, 2511]],
    [[897, 1345, 537, 1121, 1613], [1047, 1572, 627, 1310, 1883]],
    [[666, 997, 399, 831, 1198], [762, 1087, 435, 1087, 1306]],
];
/// FAC cells per frame ("FAC Config": "lower the number of FAC-Cells to 45").
pub const FAC_CELLS: usize = 45;
/// FAC data bits per frame, the CRC included ("FAC Config": "total of 40 bits of data").
pub const FAC_BITS: usize = 40;
/// Frames per super frame: FRAME-ID takes the values 0, 1 and 2 ("FAC Config").
pub const FRAMES_PER_SUPERFRAME: usize = 3;
/// Label characters per frame: "3*7 bit ascii character (9 char in superframe)" ("FAC Config").
pub const LABEL_CHARS: usize = 3;
/// The frequency of carrier 0 in the audio, Hz: the WinDRM default "DC offset" (WinDRM Docs Release 1.3, "DC Offset").
pub const DC_HZ: f64 = 350.0;
/// The audio frequencies of the three frequency pilots, Hz (WinDRM Docs Release 1.3, "Receive Spectrum").
pub const FREQ_PILOT_HZ: [f64; 3] = [725.0, 1475.0, 1850.0];
/// The depth of the long interleaver in frames: 2 s ("Interleaver Depth (400ms / 2s)") of 400 ms frames, the long MSC cell
/// interleaver of ES 201 980 clause 7.6.
pub const LONG_DEPTH: usize = 5;
/// The 4-QAM code rate. It is the only rate of ES 201 980 table 27 that produces the printed 4-QAM bit counts of "MSC
/// capacity".
pub const RATE_QAM4: Rate = Rate::new(3, 5);

sdrkit_core::choice_enum! {
    /// A HamDRM robustness mode.
    #[derive(PartialOrd, Ord, serde::Serialize)]
    pub enum HamMode {
        /// Mode A.
        A = ("A", "A"),
        /// Mode B.
        B = ("B", "B"),
        /// Mode E.
        E = ("E", "E"),
    }
}

impl HamMode {
    /// Every mode.
    pub const ALL: [HamMode; 3] = [HamMode::A, HamMode::B, HamMode::E];
    /// The index into the tables (A 0, B 1, E 2).
    pub fn index(self) -> usize {
        self as usize
    }
    /// "A", "B" or "E".
    pub fn letter(self) -> &'static str {
        ["A", "B", "E"][self as usize]
    }
    /// The mode of a letter.
    pub fn from_letter(s: &str) -> Option<HamMode> {
        Self::ALL.into_iter().find(|m| m.letter() == s)
    }
    /// FFT size at 12 kHz.
    pub fn nfft(self) -> usize {
        FFT_SIZE_48K[self as usize] / 4
    }
    /// Guard interval at 12 kHz: FFT_SIZE * ENUM_TG_TU / DENOM_TG_TU.
    pub fn cp(self) -> usize {
        let (n, d) = TG_TU[self as usize];
        self.nfft() * n / d
    }
    /// Symbols per frame.
    pub fn nsym(self) -> usize {
        SYM_PER_FRAME[self as usize]
    }
    /// Carrier spacing, Hz.
    pub fn spacing(self) -> f64 {
        RATE / self.nfft() as f64
    }
    /// Frame duration, s.
    pub fn frame_s(self) -> f64 {
        (self.nsym() * (self.nfft() + self.cp())) as f64 / RATE
    }
    /// Carriers at 2.5 kHz (`wide`) or 2.3 kHz.
    pub fn carriers(self, wide: bool) -> usize {
        CARRIERS[self as usize][wide as usize]
    }
    /// The DRM robustness mode whose gain pilot pattern and phases the mode keeps (A and B), or `None` (E).
    pub fn drm(self) -> Option<Robustness> {
        match self {
            HamMode::A => Some(Robustness::A),
            HamMode::B => Some(Robustness::B),
            HamMode::E => None,
        }
    }
    /// The carrier numbers of the three frequency pilots: `(FREQ_PILOT_HZ - DC_HZ) / spacing`.
    pub fn freq_pilot_carriers(self) -> [i32; 3] {
        FREQ_PILOT_HZ.map(|f| ((f - DC_HZ) / self.spacing()).round() as i32)
    }
    /// The occupancy whose map the document draws: 2.5 kHz in modes A and B, 2.3 kHz in mode E.
    pub fn drawn_wide(self) -> bool {
        self != HamMode::E
    }
    /// The map the document draws for this mode.
    pub fn drawn_rows(self) -> &'static [&'static str] {
        match self {
            HamMode::A => &MAP_A_WIDE,
            HamMode::B => &MAP_B_WIDE,
            HamMode::E => &MAP_E_NARROW,
        }
    }
}

/// The occupancy as text: "2.3" or "2.5" (kHz).
pub fn occupancy_text(wide: bool) -> &'static str {
    if wide {
        "2.5"
    } else {
        "2.3"
    }
}

/// What a cell of a map carries (the legend of "Carrier Config").
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CellKind {
    /// "." MSC cell.
    Msc,
    /// "X" FAC cell.
    Fac,
    /// "T" time pilot.
    Time,
    /// "f" frequency pilot.
    Frequency,
    /// "0" scattered pilot.
    Scattered,
    /// "*" boosted scattered pilot.
    Boosted,
}

impl CellKind {
    /// The kind of a legend character.
    pub fn from_char(c: char) -> Option<CellKind> {
        Some(match c {
            '.' => CellKind::Msc,
            'X' => CellKind::Fac,
            'T' => CellKind::Time,
            'f' => CellKind::Frequency,
            '0' => CellKind::Scattered,
            '*' => CellKind::Boosted,
            _ => return None,
        })
    }
    /// Whether the cell is a pilot.
    pub fn is_pilot(self) -> bool {
        !matches!(self, CellKind::Msc | CellKind::Fac)
    }
}

/// The cells of one frame of a mode and occupancy. Column `c` of a map is carrier `kmin + c`: the column of the first frequency
/// pilot is the carrier of 725 Hz.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CellMap {
    mode: HamMode,
    wide: bool,
    kmin: i32,
    carriers: usize,
    cells: Vec<CellKind>,
}

/// Why the rows of a map are refused.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum MapFault {
    /// The number of rows is not the number of symbols of the mode.
    #[error("{got} rows, the mode has {expected} symbols")]
    Rows {
        /// The rows given.
        got: usize,
        /// The symbols of the mode.
        expected: usize,
    },
    /// A row does not have one column per carrier of the occupancy.
    #[error("row {row} has {got} columns, the occupancy has {expected} carriers")]
    Columns {
        /// The row.
        row: usize,
        /// The columns given.
        got: usize,
        /// The carriers of the occupancy.
        expected: usize,
    },
    /// A character is not a cell of the legend.
    #[error("row {row} column {column}: \"{ch}\" is not a cell of the legend")]
    Legend {
        /// The row.
        row: usize,
        /// The column.
        column: usize,
        /// The character.
        ch: char,
    },
    /// A time pilot is outside symbol 0.
    #[error("row {row} column {column}: a time pilot outside symbol 0")]
    TimePilot {
        /// The row.
        row: usize,
        /// The column.
        column: usize,
    },
    /// The frequency pilots are not the same three carriers in every symbol.
    #[error("the frequency pilots are not the same three carriers in every symbol")]
    FrequencyPilots,
    /// The frequency pilots are not at 725, 1475 and 1850 Hz.
    #[error("the frequency pilots are not at 725, 1475 and 1850 Hz")]
    FrequencyPilotPlace,
    /// The number of FAC cells is not 45.
    #[error("{got} FAC cells, the frame has 45")]
    FacCells {
        /// The FAC cells given.
        got: usize,
    },
}

impl CellMap {
    /// Reads a map from its rows of legend characters.
    ///
    /// # Errors
    ///
    /// [`MapFault`]: the number of rows (the symbols of the mode) or columns (the carriers of the occupancy), an unknown
    /// character, time pilots outside symbol 0, frequency pilots that are not the same three carriers in every symbol at 725,
    /// 1475 and 1850 Hz, or a FAC cell count other than 45.
    pub fn parse<S: AsRef<str>>(
        mode: HamMode,
        wide: bool,
        rows: &[S],
    ) -> Result<CellMap, MapFault> {
        let k = mode.carriers(wide);
        if rows.len() != mode.nsym() {
            return Err(MapFault::Rows {
                got: rows.len(),
                expected: mode.nsym(),
            });
        }
        let mut cells = Vec::with_capacity(k * rows.len());
        for (s, r) in rows.iter().enumerate() {
            let r = r.as_ref();
            if r.chars().count() != k {
                return Err(MapFault::Columns {
                    row: s,
                    got: r.chars().count(),
                    expected: k,
                });
            }
            for (c, ch) in r.chars().enumerate() {
                let kind = CellKind::from_char(ch).ok_or(MapFault::Legend {
                    row: s,
                    column: c,
                    ch,
                })?;
                if kind == CellKind::Time && s != 0 {
                    return Err(MapFault::TimePilot { row: s, column: c });
                }
                cells.push(kind);
            }
        }
        let fcols = |s: usize| -> Vec<usize> {
            (0..k)
                .filter(|&c| cells[s * k + c] == CellKind::Frequency)
                .collect()
        };
        let f0 = fcols(0);
        if f0.len() != 3 || (1..rows.len()).any(|s| fcols(s) != f0) {
            return Err(MapFault::FrequencyPilots);
        }
        let want = mode.freq_pilot_carriers();
        let kmin = want[0] - f0[0] as i32;
        if (0..3).any(|i| kmin + f0[i] as i32 != want[i]) || kmin < 1 {
            return Err(MapFault::FrequencyPilotPlace);
        }
        let fac = cells.iter().filter(|&&x| x == CellKind::Fac).count();
        if fac != FAC_CELLS {
            return Err(MapFault::FacCells { got: fac });
        }
        Ok(CellMap {
            mode,
            wide,
            kmin,
            carriers: k,
            cells,
        })
    }

    /// The map the document draws for `mode` (2.5 kHz in modes A and B, 2.3 kHz in mode E).
    pub fn drawn(mode: HamMode) -> CellMap {
        Self::parse(mode, mode.drawn_wide(), mode.drawn_rows())
            .expect("the drawn maps are well formed")
    }

    /// The mode.
    pub fn mode(&self) -> HamMode {
        self.mode
    }
    /// 2.5 kHz (else 2.3 kHz).
    pub fn wide(&self) -> bool {
        self.wide
    }
    /// The carrier number of column 0.
    pub fn kmin(&self) -> i32 {
        self.kmin
    }
    /// The carrier number of the last column.
    pub fn kmax(&self) -> i32 {
        self.kmin + self.carriers as i32 - 1
    }
    /// Carriers (columns).
    pub fn carriers(&self) -> usize {
        self.carriers
    }
    /// Symbols (rows).
    pub fn nsym(&self) -> usize {
        self.cells.len() / self.carriers
    }
    /// The kind of the cell of symbol `s` and column `c`.
    pub fn kind(&self, s: usize, c: usize) -> CellKind {
        self.cells[s * self.carriers + c]
    }
    /// The cells of `kind`, as indices `s * carriers + c`, in symbol then carrier order.
    pub fn cells_of(&self, kind: CellKind) -> Vec<usize> {
        (0..self.cells.len())
            .filter(|&i| self.cells[i] == kind)
            .collect()
    }
    /// How many cells are of `kind`.
    pub fn count(&self, kind: CellKind) -> usize {
        self.cells.iter().filter(|&&x| x == kind).count()
    }
}

/// The meaning of the FAC bits whose values the document names but does not number: the bit value that signals each option.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct FacValues {
    /// Interleaver Depth: the value of 2 s.
    pub long_interleave: u8,
    /// MSC Mode: the value of 64-QAM.
    pub qam64: u8,
    /// Audio/Data: the value of data.
    pub data: u8,
    /// Extended MSC mode: the value of 4-QAM.
    pub qam4: u8,
}

/// What the MSC of a frame carries.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
pub enum Content {
    /// Audio: the Audio Coding value (2 bits) and the Text Flag.
    Audio {
        /// Audio Coding.
        coding: u8,
        /// Text Flag.
        text: bool,
    },
    /// Data in packets of this Packet ID.
    Data {
        /// Packet ID (0 to 3).
        packet_id: u8,
    },
}

/// The FAC of one frame ("FAC Config").
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
pub struct FrameInfo {
    /// FRAME-ID: the frame of the super frame (0, 1 or 2).
    pub frame_id: u8,
    /// Spectrum Occupancy: 2.5 kHz (else 2.3 kHz).
    pub wide: bool,
    /// Interleaver Depth: 2 s (else 400 ms).
    pub long_interleave: bool,
    /// The MSC constellation.
    pub qam: Qam,
    /// Prot Level (0 or 1).
    pub protection: u8,
    /// Audio or data.
    pub content: Content,
    /// Three 7-bit characters of the label.
    pub label: [u8; LABEL_CHARS],
}

impl FrameInfo {
    /// The 40 FAC bits in the order of "FAC Config": FRAME-ID (2), Spectrum Occupancy (1), Interleaver Depth (1), MSC Mode (1),
    /// Prot Level (1), Audio/Data (1), then Audio Coding (2) and Text Flag (1) or Packet ID (2) and extended MSC mode (1), the
    /// Label (21), a dummy bit (0), and the CRC-8 of the FAC of ES 201 980 (clause 6.3.5 and annex D). `v` supplies the bit
    /// values of the options.
    pub fn bits(&self, v: &FacValues) -> Vec<u8> {
        let mut b = Vec::with_capacity(FAC_BITS);
        let flag = |on: bool, val: u8| if on { val & 1 } else { !val & 1 } as u64;
        push_u64(&mut b, self.frame_id as u64 & 3, 2);
        push_u64(&mut b, self.wide as u64, 1);
        push_u64(&mut b, flag(self.long_interleave, v.long_interleave), 1);
        push_u64(&mut b, flag(self.qam == Qam::Q64, v.qam64), 1);
        push_u64(&mut b, self.protection as u64 & 1, 1);
        match self.content {
            Content::Audio { coding, text } => {
                push_u64(&mut b, flag(false, v.data), 1);
                push_u64(&mut b, coding as u64 & 3, 2);
                push_u64(&mut b, text as u64, 1);
            }
            Content::Data { packet_id } => {
                push_u64(&mut b, flag(true, v.data), 1);
                push_u64(&mut b, packet_id as u64 & 3, 2);
                push_u64(&mut b, flag(self.qam == Qam::Q4, v.qam4), 1);
            }
        }
        for &c in &self.label {
            push_u64(&mut b, c as u64 & 0x7F, 7);
        }
        push_u64(&mut b, 0, 1);
        CRC8_SAE_J1850.append_bits(&mut b);
        b
    }

    /// Reads 40 FAC bits. `None` when the CRC is wrong or FRAME-ID is 3.
    pub fn from_bits(b: &[u8], v: &FacValues) -> Option<FrameInfo> {
        if b.len() < FAC_BITS || !CRC8_SAE_J1850.verify_bits(&b[..FAC_BITS]) {
            return None;
        }
        let frame_id = to_u64(&b[0..2]) as u8;
        if frame_id as usize >= FRAMES_PER_SUPERFRAME {
            return None;
        }
        let is = |i: usize, val: u8| b[i] & 1 == val & 1;
        let q64 = is(4, v.qam64);
        let (content, qam) = if is(6, v.data) {
            let q = if is(9, v.qam4) {
                Qam::Q4
            } else if q64 {
                Qam::Q64
            } else {
                Qam::Q16
            };
            (
                Content::Data {
                    packet_id: to_u64(&b[7..9]) as u8,
                },
                q,
            )
        } else {
            let q = if q64 { Qam::Q64 } else { Qam::Q16 };
            (
                Content::Audio {
                    coding: to_u64(&b[7..9]) as u8,
                    text: b[9] == 1,
                },
                q,
            )
        };
        let mut label = [0u8; LABEL_CHARS];
        for (i, c) in label.iter_mut().enumerate() {
            *c = to_u64(&b[10 + 7 * i..17 + 7 * i]) as u8;
        }
        Some(FrameInfo {
            frame_id,
            wide: b[2] == 1,
            long_interleave: is(3, v.long_interleave),
            qam,
            protection: b[5] & 1,
            content,
            label,
        })
    }
}

/// The MSC code rates of each level for a constellation and protection level. 16-QAM and 64-QAM use the DRM rates of
/// protection levels 0 and 1 (ES 201 980 tables 29 to 32). 4-QAM has protection 1 only, at [`RATE_QAM4`] ("Notes": "High
/// Protection is not supported for QAM4"). `None` for any other combination.
pub fn rates(q: Qam, protection: u8) -> Option<&'static [Rate]> {
    const Q4: [Rate; 1] = [RATE_QAM4];
    match (q, protection) {
        (Qam::Q4, 1) => Some(&Q4),
        (Qam::Q4, _) => None,
        (_, 0 | 1) => level_rates(Robustness::A, q, protection).map(|r| r.0),
        _ => None,
    }
}

/// The MSC code of `cells` MSC cells: the multilevel code of ES 201 980 clause 7.3 with one protection level.
pub fn msc_code(q: Qam, protection: u8, cells: usize) -> Option<Mlc> {
    Mlc::eep(q, rates(q, protection)?, cells).ok()
}

/// The MSC bits of a frame of `cells` MSC cells.
pub fn capacity(q: Qam, protection: u8, cells: usize) -> Option<usize> {
    msc_code(q, protection, cells).map(|m| m.info_bits())
}

/// The cell map of mode A at 2.5 kHz as the document draws it ("Robustness mode A / Spectrum occupancy 1"). Rows are symbols,
/// columns are carriers from the lowest up.
pub const MAP_A_WIDE: [&str; 15] = [
    "*...TT.f.TT..TT.....0T...f.TT..TT.f.TT.T0..TT............",
    "....*..fX...........X...0f..X.....f.........0...X........",
    ".......f0...X...........Xf..0...X.f.............0........",
    ".......f....0...X........f..X...0.f.X...............*....",
    ".......f........0...X....f......X.f.0...X...............*",
    "*......f........X...0...Xf........f.X...0...X............",
    "....*..f............X...0f..X.....f.....X...0...X........",
    ".......f0...............Xf..0...X.f.........X...0........",
    ".......fX...0............f..X...0.f.X...........X...*....",
    ".......f....X...0........f......X.f.0...X...............*",
    "*......f........X...0....f........f.X...0...X............",
    "....*..fX...........X...0f........f.....X...0...X........",
    ".......f0...X...........Xf..0.....f.........X...0........",
    ".......f....0...X........f..X...0.f.................*....",
    ".......f........0...X....f......X.f.0...................*",
];
/// The cell map of mode B at 2.5 kHz as the document draws it ("Robustness mode B / Spectrum occupancy 1").
pub const MAP_B_WIDE: [&str; 15] = [
    "*....T0f.TT.0T..TT0.X..f0.TT.T0fTT..0T.TT.0T....*..",
    "..*....f0.X...0.....0.Xf..0....f0.X...0.....0.....*",
    "....0..f..0.X...0.....0fX...0..f..0.X...0.....0....",
    "*.....0f....0.X...0....f0.X...0f....0.X...0.....*..",
    "..*.X..f0.....0.X...0..f..0.X..f0.....0.X...0.....*",
    "....0.Xf..0.....0.X...0f....0.Xf..0.....0.....0....",
    "*.....0fX...0.....0.X..f0.....0fX...0.....0.....*..",
    "..*....f0.X...0.....0.Xf..0....f0.X...0.....0.....*",
    "....0..f..0.X...0.....0fX...0..f..0.X...0.....0....",
    "*.....0f....0.X...0....f0.X...0f....0.X...0.....*..",
    "..*.X..f0.....0.X...0..f..0.X..f0.....0.X...0.....*",
    "....0.Xf..0.....0.X...0f....0.Xf..0.....0.....0....",
    "*.....0fX...0.....0.X..f0.....0fX...0.....0.....*..",
    "..*....f0.X...0.....0.Xf..0....f0.X...0.....0.....*",
    "....0..f..0.X...0.....0fX...0..f..0.X...0.....0....",
];
/// The cell map of mode E at 2.3 kHz as the document draws it ("Robustness mode E / Spectrum occupancy 0").
pub const MAP_E_NARROW: [&str; 20] = [
    "*...f.TT0...TTf.0..fTT..0TT.*",
    ".0..f0X..0...0f..0.f.0X..0...",
    "..0.f.0X..0...fX..0f..0X..0..",
    "...0f..0X..0..f0X..f...0...0.",
    "*...f...0X..0.f.0X.f0...0...*",
    ".0..f0...0X..0f..0Xf.0...0...",
    "..0Xf.0...0X..f...0f..0...0..",
    "...0f..0...0X.f0...fX..0...0.",
    "*...fX..0...0Xf.0..f0X..0...*",
    ".0..f0X..0...0f..0.f.0X..0...",
    "..0.f.0X..0...fX..0f..0X..0..",
    "...0f..0X..0X.f0X..f...0...0.",
    "*...f...0X..0.f.0X.f0...0...*",
    ".0..f0...0X..0f..0Xf.0...0...",
    "..0Xf.0...0X..fX..0f..0...0..",
    "...0f..0...0X.f0...fX..0...0.",
    "*...fX..0...0Xf.0..f0X..0...*",
    ".0..f0X..0...0f..0.f.0X..0...",
    "..0.f.0X..0...fX..0f..0X..0..",
    "...0f..0X..0..f0X..f...0...0.",
];
