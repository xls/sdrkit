//! FSK 200/500 (submode `f01`) and FSK 200/1000 (submode `f06`): 2-FSK number senders at 200 Bd. `f01`: 500 Hz shift,
//! asynchronous ITA2 with 1.5 stop units, figure blocks of 25 characters. `f06`: 1000 Hz shift, synchronous blocks of 288 bits
//! with a 32-bit sync word, an 11-bit block number and 16 data bytes under a CRC-16; figure blocks (F06), ASCII blocks and
//! files (F06a), told apart from the content.
//!
//! Standard: no issuing body publishes these signals. The formats are those of the public descriptions: the priyom.org
//! protocol pages of F01, F06 and F06a, and the "M42, M42b, F01, F06, F06a, X06" profile of Numbers and Oddities (chapters 5,
//! 7, 8 and 9). The CRC of the data is CRC-16/IBM-SDLC of the published catalogue of CRC algorithms, which alone gives the CRC
//! of the example payload of the profile.
//!
//! ```text
//! iq -> [hf_front 24 kHz] -> [fsk200: low pass to 8 kHz, tone-pair search and replay, two matched filters of one bit;
//!        f01: two start-stop readers (either tone as mark), the gate and the blocks; f06: the symbol clock, the sync word,
//!        the block, the CRC and its one-bit correction, the block number, the traffic kind] -> rows, text
//! ```

use crate::receiver::GEN_SECTION;
use crate::senders::{Cx, Layer, Lcg, Playback, RowLog, Sender, StatusLine, TextLog, Tone};
use crate::typed::{
    F06BlockEvent, Fsk200Fields, SenderRow, TextEvent, TonesFoundEvent, TonesLostEvent, F06_BLOCK,
    SENDER_ROWS, TEXT, TONES_FOUND, TONES_LOST,
};
use sdrkit_algo_modem::charframe::CharFramer;
use sdrkit_algo_modem::fsk::{MfskConfig, MfskDemod, ToneCorrelator};
use sdrkit_algo_modem::toneset::{CombSpec, SearchConfig, SearchEvent, ToneSetSearch};
use sdrkit_core::interface::{Fields as _, Hertz, Time};
use sdrkit_core::publish::{field, Published};
use sdrkit_core::{
    AutoGain, Choice, Complex32, ConfigError, GenConfig, Generator, LockProfile, Receiver,
    ResultsDecl, Schema, Setting, SettingError, SettingId, SettingSet, SignalInfo, StackConfig,
    StackDescriptor, SubMode, Value, Values,
};
use sdrkit_graph::{values_of, Kept};
use sdrkit_util_bits::crc::catalog::{CRC16_IBM_SDLC, CRC32_ISO_HDLC};
use sdrkit_util_text::ita2;
use std::collections::VecDeque;
use std::sync::Arc;

/// The channel rate.
pub const RATE: f64 = 8000.0;
/// The speed, Bd.
pub const BAUD: f64 = 200.0;
/// Samples a bit.
const SPB: usize = 40;
/// The sync word of an F06 block.
pub const SYNC: u32 = 0x7D12_B0E6;
/// Bits a block.
pub const BLOCK_BITS: usize = 288;

sdrkit_core::choice_enum! {
    /// The signal.
    pub enum Submode {
        /// FSK 200/500, asynchronous ITA2.
        F01 = ("f01", "FSK200/500 (F01)"),
        /// FSK 200/1000, synchronous blocks (F06, F06a).
        F06 = ("f06", "FSK200/1000 (F06)"),
    }
}

impl Submode {
    /// The shift, Hz.
    pub fn shift(self) -> f64 {
        match self {
            Submode::F01 => 500.0,
            Submode::F06 => 1000.0,
        }
    }

    fn id(self) -> &'static str {
        match self {
            Submode::F01 => "FSK200/500",
            Submode::F06 => "FSK200/1000",
        }
    }
}

sdrkit_core::choice_enum! {
    /// The sense of the tones.
    pub enum Polarity {
        /// Both senses are read; the one that fits wins.
        Auto = ("auto", "Find it"),
        /// The published sense: the low tone is mark (f01) or 0 (f06).
        Normal = ("normal", "Low tone is mark or 0"),
        /// The high tone is mark (f01) or 0 (f06).
        Reversed = ("reversed", "High tone is mark or 0"),
    }
}

sdrkit_core::choice_enum! {
    /// The case f01 prints.
    pub enum Case {
        /// The figures case always.
        Figures = ("figures", "Figures"),
        /// The figure and letter shifts are followed.
        Ita2 = ("ita2", "Follow the shifts"),
    }
}

sdrkit_core::choice_enum! {
    /// The traffic of the f06 test signal.
    pub enum GenKind {
        /// Figure blocks.
        F06 = ("f06", "F06 figure blocks"),
        /// ASCII blocks.
        Ascii = ("ascii", "ASCII blocks"),
        /// A file.
        F06a = ("f06a", "F06a file"),
    }
}

/// The submode.
pub const SUBMODE: Setting<field::Choice<Submode>> =
    Setting::choice("submode", "Signal", Submode::F01);
/// The sense of the tones.
pub const POLARITY: Setting<field::Choice<Polarity>> =
    Setting::choice("polarity", "Tone sense", Polarity::Auto).help(
        "Find it reads both senses side by side; f06 tests the sync word and its complement.",
    );
/// The case of f01.
pub const CASE: Setting<field::Choice<Case>> = Setting::choice("text_case", "Case", Case::Figures)
    .help("f01: the traffic is figure groups; Follow the shifts reads other ITA2 traffic.");
/// The sync word errors accepted by f06.
pub const SYNC_ERRORS: Setting<field::I64> =
    Setting::new("sync_errors", "Sync word errors accepted", "bits", 3)
        .range(0.0, 3.0, 1.0)
        .help("f06: above 1 a block is kept only when its CRC holds.");
/// The one-bit correction of f06.
pub const CORRECT_ONE_BIT: Setting<field::Bool> =
    Setting::new("correct_one_bit", "Correct one bit by the CRC", "", true)
        .help("f06: a block whose CRC fails is tried with each bit flipped, weakest first.");
/// The groups of the f01 test signal.
pub const GEN_GROUPS: Setting<field::I64> = Setting::new("gen_groups", "Groups", "", 60)
    .range(3.0, 2997.0, 3.0)
    .help("f01: random five-figure groups from the seed, three to a block.")
    .section(GEN_SECTION)
    .advanced();
/// The figure blocks of the f06 test signal.
pub const GEN_BLOCKS: Setting<field::I64> = Setting::new("gen_blocks", "Blocks", "", 20)
    .range(1.0, 1000.0, 1.0)
    .help("f06: figure blocks of random numbers from the seed.")
    .section(GEN_SECTION)
    .advanced();
/// The traffic of the f06 test signal.
pub const GEN_KIND: Setting<field::Choice<GenKind>> =
    Setting::choice("gen_kind", "Traffic", GenKind::F06)
        .section(GEN_SECTION)
        .advanced();
/// The text of the ASCII blocks and of the file.
pub const GEN_TEXT: Setting<field::Text> = Setting::new(
    "gen_text",
    "Text",
    "",
    "FIRST LINE OF THE TEST FILE\r\nSECOND LINE\r\n",
)
.section(GEN_SECTION)
.advanced();
/// The name of the file.
pub const GEN_NAME: Setting<field::Text> = Setting::new("gen_name", "File name", "", "TEST.TXT")
    .section(GEN_SECTION)
    .advanced();
/// Bits flipped in the f06 test signal.
pub const GEN_BIT_ERRORS: Setting<field::Text> =
    Setting::new("gen_bit_errors", "Bits flipped", "", "")
        .help("f06: block:bit pairs separated by commas, the bit counted from 0 at the first bit of the block.")
        .section(GEN_SECTION)
        .advanced();
/// The centre of the tones of the test signal.
pub const GEN_CENTRE_HZ: Setting<field::Hz> = Setting::new("gen_centre_hz", "Centre", "Hz", 1500.0)
    .range(600.0, 3000.0, 1.0)
    .section(GEN_SECTION)
    .advanced();
/// The test signal in the reversed sense.
pub const GEN_REVERSED: Setting<field::Bool> =
    Setting::new("gen_reversed", "Reversed sense", "", false)
        .section(GEN_SECTION)
        .advanced();

sdrkit_core::settings! {
    /// The settings of the FSK200 receiver.
    pub enum Fsk200RxSetting {
        /// Signal.
        Submode = SUBMODE,
        /// Tone sense.
        Polarity = POLARITY,
        /// Case.
        Case = CASE,
        /// Sync word errors.
        SyncErrors = SYNC_ERRORS,
        /// One-bit correction.
        CorrectOneBit = CORRECT_ONE_BIT,
    }
}

sdrkit_core::settings! {
    /// The settings of the FSK200 stack.
    pub enum Fsk200Setting {
        /// Signal.
        Submode = SUBMODE,
        /// Tone sense.
        Polarity = POLARITY,
        /// Case.
        Case = CASE,
        /// Sync word errors.
        SyncErrors = SYNC_ERRORS,
        /// One-bit correction.
        CorrectOneBit = CORRECT_ONE_BIT,
        /// Groups of the f01 test signal.
        GenGroups = GEN_GROUPS,
        /// Blocks of the f06 test signal.
        GenBlocks = GEN_BLOCKS,
        /// Traffic of the f06 test signal.
        GenKind = GEN_KIND,
        /// Text of the test signal.
        GenText = GEN_TEXT,
        /// File name of the test signal.
        GenName = GEN_NAME,
        /// Bits flipped.
        GenBitErrors = GEN_BIT_ERRORS,
        /// Centre of the test signal.
        GenCentreHz = GEN_CENTRE_HZ,
        /// Reversed sense.
        GenReversed = GEN_REVERSED,
    }
}

/// What no public document confirms.
pub const NOTE_TEXT: &str = "No issuing body publishes F01, F06 or F06a; the formats follow priyom.org and the Numbers and \
Oddities profile. f01: the three check digits and three check characters of a block are shown, not checked (their rule is \
not known); the figures of codes other than 15, 17, 18, 30 and the digits are not confirmed; the reversals before the \
traffic are the test signal's. f06: the 5-bit code of the block number and the 96 bits of bytes 24 to 35 are an error code \
whose rule is not known, so they are not used (a block number outside the CRC that is read wrong stays wrong unless the \
numbering before predicts it); the reversed sync word, the ASCII kind (34 36 at the start of block 1) and the one-bit \
correction by the CRC are not in a document; the F06a file check is shown and compared with CRC-32 and its complement, which \
the two documents name differently.";

/// The figures case of f01: ITA2 with `+` at code 17 and `=` at code 30 (code 5 is the bell).
pub const F01_TABLE: ita2::Table = ita2::Table {
    name: "FSK200/500 figures",
    letters: ita2::ITU.letters,
    figures: [
        None,
        Some('3'),
        Some('\n'),
        Some('-'),
        Some(' '),
        None,
        Some('8'),
        Some('7'),
        Some('\r'),
        Some('$'),
        Some('4'),
        Some('\''),
        Some(','),
        Some('!'),
        Some(':'),
        Some('('),
        Some('5'),
        Some('+'),
        Some(')'),
        Some('2'),
        Some('#'),
        Some('6'),
        Some('0'),
        Some('1'),
        Some('9'),
        Some('?'),
        Some('&'),
        None,
        Some('.'),
        Some('/'),
        Some('='),
        None,
    ],
};

/// The f01 code of figure `c`.
pub fn f01_code(c: char) -> Option<u8> {
    F01_TABLE
        .figures
        .iter()
        .position(|&f| f == Some(c))
        .map(|k| k as u8)
}

// ---- The f01 layer ----

/// What the f01 text layer publishes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum F01Row {
    /// A data block: type, check digits, number, groups, check characters.
    Block(String),
    /// A header block: type, check digits, groups.
    Header(String),
    /// Text outside a block.
    Text(String),
}

/// The text layer of f01: characters to text lines and blocks.
#[derive(Clone, Debug)]
pub struct F01Text {
    case: Case,
    figs: bool,
    line: String,
    block: Option<String>,
    /// Characters read.
    pub chars: i64,
    /// Framing errors.
    pub framing: i64,
}

impl F01Text {
    /// A text layer printing `case`.
    pub fn new(case: Case) -> Self {
        F01Text {
            case,
            figs: true,
            line: String::new(),
            block: None,
            chars: 0,
            framing: 0,
        }
    }

    /// Publishes the open line and the open block.
    pub fn flush(&mut self, out: &mut Vec<F01Row>) {
        if let Some(b) = self.block.take() {
            self.line.push_str(&b);
        }
        let n = self.line.chars().count();
        let t = self.line.trim().to_string();
        self.line.clear();
        if n >= 4 && !t.is_empty() {
            out.push(F01Row::Text(t));
        }
    }

    fn block_row(b: &str) -> F01Row {
        let c: Vec<char> = b.chars().collect();
        let part = |a: usize, z: usize| c[a..z].iter().collect::<String>();
        if c[0] == '(' {
            let groups: Vec<String> = (0..5).map(|g| part(4 + 5 * g, 9 + 5 * g)).collect();
            F01Row::Header(format!("( {} {}", part(1, 4), groups.join(" ")))
        } else {
            F01Row::Block(format!(
                "{} {} {} {} {} {} {}",
                c[0],
                part(1, 4),
                part(4, 7).trim(),
                part(7, 12),
                part(12, 17),
                part(17, 22),
                part(22, 25)
            ))
        }
    }

    /// Takes code `code`, `good` when its stop unit was mark.
    pub fn push(&mut self, code: u8, good: bool, out: &mut Vec<F01Row>) {
        let open = self.line.chars().count() + self.block.as_ref().map_or(0, |b| b.chars().count());
        if !good {
            self.framing += 1;
            if open < 6 {
                self.line.clear();
                self.block = None;
                return;
            }
        }
        if good {
            match code {
                0 => return,
                ita2::FIGS => {
                    self.figs = true;
                    return;
                }
                ita2::LTRS => {
                    if self.case == Case::Ita2 {
                        self.figs = false;
                    }
                    return;
                }
                2 | 8 => {
                    self.flush(out);
                    return;
                }
                _ => {}
            }
        }
        let c = if !good {
            '?'
        } else if code == 5 {
            self.append_str("<BELL>", out);
            return;
        } else {
            let figs = self.figs || self.case == Case::Figures;
            F01_TABLE.char_of(code, figs).unwrap_or('?')
        };
        self.chars += 1;
        if matches!(c, '=' | ')' | '(') {
            self.flush(out);
            self.block = Some(c.to_string());
            return;
        }
        if let Some(b) = &mut self.block {
            b.push(c);
            let len = if b.starts_with('(') { 29 } else { 25 };
            if b.chars().count() == len {
                let row = Self::block_row(b);
                self.block = None;
                out.push(row);
            }
            return;
        }
        self.line.push(c);
        if self.line.chars().count() >= 80 {
            self.flush(out);
        }
    }

    fn append_str(&mut self, s: &str, out: &mut Vec<F01Row>) {
        self.chars += 1;
        self.line.push_str(s);
        if self.line.chars().count() >= 80 {
            self.flush(out);
        }
    }
}

/// One start-stop reader of f01: either tone as mark.
struct F01Reader {
    framer: CharFramer,
    valid: VecDeque<bool>,
    quality: VecDeque<f32>,
    kept: VecDeque<(u8, bool)>,
}

impl F01Reader {
    fn new(inverted: bool) -> Self {
        F01Reader {
            framer: CharFramer::new(SPB as f64, 5, inverted)
                .expect("valid framer")
                .without_clock(),
            valid: VecDeque::new(),
            quality: VecDeque::new(),
            kept: VecDeque::new(),
        }
    }

    fn valid_ratio(&self) -> Option<f64> {
        (self.valid.len() == 24).then(|| self.valid.iter().filter(|&&v| v).count() as f64 / 24.0)
    }

    fn quality(&self) -> f32 {
        if self.quality.is_empty() {
            0.0
        } else {
            self.quality.iter().sum::<f32>() / self.quality.len() as f32
        }
    }
}

// ---- The f06 block layer ----

/// The bytes of a block from its bits (first bit the most significant of byte 0).
pub fn bytes_of(bits: &[u8]) -> [u8; 36] {
    let mut b = [0u8; 36];
    for (i, &v) in bits.iter().take(BLOCK_BITS).enumerate() {
        b[i / 8] |= (v & 1) << (7 - i % 8);
    }
    b
}

/// The places of the data nibbles in bytes 6 to 23: for nibble `k` (data byte `k / 2`, high first, then the 4 CRC nibbles as
/// numbers 32 to 35), the byte and whether it is the high nibble of that byte.
fn nibble_place(k: usize) -> (usize, bool) {
    // Even bytes 6, 8, .. 22 hold nibbles 0 to 17: all their high nibbles, then all their low nibbles.
    // Odd bytes 7, 9, .. 23 hold nibbles 18 to 35 the same way.
    let (base, j) = if k < 18 { (6, k) } else { (7, k - 18) };
    if j < 9 {
        (base + 2 * j, true)
    } else {
        (base + 2 * (j - 9), false)
    }
}

/// A block unpacked.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Unpacked {
    /// The block number.
    pub number: u16,
    /// The data bytes.
    pub data: [u8; 16],
    /// The CRC received.
    pub crc: u16,
}

impl Unpacked {
    /// Whether the CRC holds.
    pub fn crc_ok(&self) -> bool {
        CRC16_IBM_SDLC.checksum(&self.data) == self.crc
    }
}

/// Unpacks block bytes.
pub fn unpack(b: &[u8; 36]) -> Unpacked {
    let mut nib = [0u8; 36];
    for (k, n) in nib.iter_mut().enumerate() {
        let (byte, high) = nibble_place(k);
        *n = if high { b[byte] >> 4 } else { b[byte] & 15 };
    }
    let mut data = [0u8; 16];
    for (i, d) in data.iter_mut().enumerate() {
        *d = (nib[2 * i] << 4) | nib[2 * i + 1];
    }
    let crc = nib[32..36]
        .iter()
        .fold(0u16, |a, &n| (a << 4) | u16::from(n));
    Unpacked {
        number: (u16::from(b[4]) << 3) | u16::from(b[5] >> 5),
        data,
        crc,
    }
}

/// Whether bytes 4 to 35 of a block are zero but for at most one: a divider (the sync word and 256 zero bits).
pub fn is_divider(b: &[u8; 36]) -> bool {
    b[4..].iter().filter(|&&v| v != 0).count() <= 1
}

/// The 38 digits of a figure block: twelve 10-bit numbers (000 to 999) and two 4-bit figures, or `None` when one is out of
/// range.
pub fn figures(d: &[u8; 16]) -> Option<String> {
    let bit = |i: usize| (d[i / 8] >> (7 - i % 8)) & 1;
    let num = |a: usize, n: usize| (a..a + n).fold(0u32, |v, i| (v << 1) | u32::from(bit(i)));
    let mut s = String::new();
    for k in 0..12 {
        let v = num(10 * k, 10);
        if v > 999 {
            return None;
        }
        s.push_str(&format!("{v:03}"));
    }
    for k in 0..2 {
        let v = num(120 + 4 * k, 4);
        if v > 9 {
            return None;
        }
        s.push_str(&v.to_string());
    }
    Some(s)
}

/// The header of a block numbered by a multiple of 16: the block count, the message count and the message starts.
pub fn header(d: &[u8; 16]) -> (u16, u8, Vec<u16>) {
    let pair = |i: usize| (u16::from(d[i]) << 3) | u16::from(d[i + 1] >> 5);
    let starts = (1..=7)
        .map(|i| pair(2 * i))
        .take_while(|&s| s != 0)
        .collect();
    (pair(0), d[1] & 31, starts)
}

/// The traffic kind of f06.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Traffic {
    /// Figure blocks.
    Figures,
    /// ASCII blocks.
    Ascii,
    /// A file.
    File,
}

impl Traffic {
    fn label(self) -> &'static str {
        match self {
            Traffic::Figures => "F06 figure groups",
            Traffic::Ascii => "ASCII",
            Traffic::File => "F06a file",
        }
    }
}

/// What the block layer publishes: kind, id and text.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct F06Row {
    /// Divider, Header, Block or File.
    pub kind: &'static str,
    /// "#N", or empty for a divider.
    pub id: String,
    /// The text.
    pub text: String,
    /// The block number, absent for a divider.
    pub number: Option<u16>,
    /// The CRC result.
    pub crc: &'static str,
}

/// The block layer of f06: good blocks to rows, the traffic kind, the files.
#[derive(Clone, Debug)]
pub struct F06Blocks {
    /// The traffic kind.
    pub traffic: Traffic,
    starts: Vec<u16>,
    file_at: Option<u16>,
    file_text: bool,
    file_ended: bool,
    file: Vec<u8>,
    file_size: Option<u32>,
    file_check: Option<u32>,
}

impl Default for F06Blocks {
    fn default() -> Self {
        Self::new()
    }
}

impl F06Blocks {
    /// A layer with the figure kind.
    pub fn new() -> Self {
        F06Blocks {
            traffic: Traffic::Figures,
            starts: Vec::new(),
            file_at: None,
            file_text: true,
            file_ended: false,
            file: Vec::new(),
            file_size: None,
            file_check: None,
        }
    }

    /// The row of a divider.
    pub fn divider() -> F06Row {
        F06Row {
            kind: "Divider",
            id: String::new(),
            text: "divider block".into(),
            number: None,
            crc: "",
        }
    }

    /// The rows of block `number` with `data`; `crc` is the result as shown, `ok` whether the CRC holds.
    pub fn block(
        &mut self,
        number: u16,
        data: &[u8; 16],
        ok: bool,
        crc: &'static str,
        out: &mut Vec<F06Row>,
    ) {
        let id = format!("#{number}");
        let row = |kind: &'static str, text: String| F06Row {
            kind,
            id: id.clone(),
            text: format!("{text} | {crc}"),
            number: Some(number),
            crc,
        };
        if number.is_multiple_of(16) {
            let (count, msgs, starts) = header(data);
            let valid = ok && msgs <= 7;
            let mut t = format!("{count} blocks, {msgs} message(s) | {crc}");
            if valid {
                if number == 0 {
                    self.traffic = Traffic::Figures;
                    self.file_at = None;
                    self.file_text = true;
                }
                self.starts = starts.clone();
                let list: Vec<String> = starts
                    .iter()
                    .enumerate()
                    .map(|(i, s)| format!("Message {} at block #{s}", i + 1))
                    .collect();
                if !list.is_empty() {
                    t.push_str(&format!(" | {}", list.join(", ")));
                }
            }
            out.push(F06Row {
                kind: "Header",
                id: id.clone(),
                text: t,
                number: Some(number),
                crc,
            });
            return;
        }
        if ok && number == 1 {
            if data[0] == 0x1B {
                self.traffic = Traffic::Figures;
            } else if data[0] == 0x34 && data[1] == 0x36 {
                self.traffic = Traffic::Ascii;
            } else if data[0] == 0 && data[1] == 0 {
                self.traffic = Traffic::File;
            }
        }
        if ok && self.traffic == Traffic::Figures && number > 1 && figures(data).is_none() {
            self.traffic = Traffic::File;
        }
        match self.traffic {
            Traffic::Figures => {
                let t = figures(data).unwrap_or_else(|| "(numbers out of range)".into());
                out.push(row("Block", t));
            }
            Traffic::Ascii => {
                let t: String = data
                    .iter()
                    .map(|&b| {
                        if (0x20..0x7F).contains(&b) {
                            b as char
                        } else {
                            '.'
                        }
                    })
                    .collect();
                out.push(row("Block", t));
            }
            Traffic::File => self.file_block(number, data, ok, &row, out),
        }
    }

    fn file_block(
        &mut self,
        number: u16,
        data: &[u8; 16],
        ok: bool,
        row: &dyn Fn(&'static str, String) -> F06Row,
        out: &mut Vec<F06Row>,
    ) {
        let start = self
            .starts
            .iter()
            .copied()
            .filter(|&s| s <= number)
            .max()
            .unwrap_or(1);
        if number == start {
            self.file_at = Some(number);
            self.file.clear();
            self.file_ended = false;
            self.file_text = true;
            let size = u32::from_be_bytes([data[0], data[1], data[2], data[3]]);
            let name: String = data[4..]
                .iter()
                .filter(|&&b| b != 0)
                .map(|&b| {
                    if (0x20..0x7F).contains(&b) {
                        (b as char).to_string()
                    } else {
                        format!("[0x{b:02x}]")
                    }
                })
                .collect();
            self.file_size = Some(size);
            out.push(row("File", format!("file named {name}, size {size} bytes")));
            return;
        }
        let first = self.file_at.is_some_and(|s| number == s + 1);
        let bytes: &[u8] = if first {
            let check = u32::from_be_bytes([data[0], data[1], data[2], data[3]]);
            self.file_check = Some(check);
            out.push(row(
                "File",
                format!("CRC of the file contents 0x{check:08x}"),
            ));
            &data[4..]
        } else {
            &data[..]
        };
        if self.file_ended {
            return;
        }
        let odd = bytes
            .iter()
            .take_while(|&&b| b != 0)
            .filter(|&&b| !(0x20..0x7F).contains(&b) && b != 0x0D && b != 0x0A)
            .count();
        if ok && odd > 10 {
            self.file_text = false;
        }
        let mut t = String::new();
        for &b in bytes {
            if self.file_text {
                match b {
                    0 if ok => {
                        t.push_str("<EOF>");
                        self.file_ended = true;
                        break;
                    }
                    0x0D if ok => t.push_str("<CR>"),
                    0x0A if ok => t.push_str("<LF>"),
                    0x20..=0x7E => t.push(b as char),
                    _ => t.push_str(&format!("[0x{b:02x}]")),
                }
            } else {
                t.push_str(&format!("{b:02x}"));
            }
            self.file.push(b);
        }
        out.push(row("Block", t));
        if self.file_ended {
            if let Some(check) = self.file_check {
                let c = CRC32_ISO_HDLC.checksum(&self.file);
                let verdict = if check == !c {
                    "the file check is the complement of the CRC-32 of the contents"
                } else if check == c {
                    "the file check is the CRC-32 of the contents"
                } else {
                    "the file check does not match the contents"
                };
                out.push(F06Row {
                    kind: "File",
                    id: format!("#{number}"),
                    text: format!("end of the file, {} bytes: {verdict}", self.file.len()),
                    number: Some(number),
                    crc: "",
                });
            }
        }
    }
}

/// The receiver of f06 blocks on a bit stream with soft values.
#[derive(Clone, Debug)]
pub struct F06Reader {
    max_sync_errors: u32,
    correct: bool,
    polarity: Polarity,
    bits: VecDeque<(u8, f32)>,
    hold: usize,
    count: u64,
    last: Option<(u16, u64)>,
    confirmed: bool,
    recent: VecDeque<u16>,
    /// The block layer.
    pub layer: F06Blocks,
    /// Good blocks.
    pub good: i64,
    /// Blocks corrected by one bit.
    pub corrected: i64,
    /// Blocks with a CRC error.
    pub crc_errors: i64,
    /// The sense of the last good block: true when reversed.
    pub reversed: Option<bool>,
}

impl F06Reader {
    /// A reader accepting `max_sync_errors` errors in the sync word.
    pub fn new(max_sync_errors: u32, correct: bool, polarity: Polarity) -> Self {
        F06Reader {
            max_sync_errors,
            correct,
            polarity,
            bits: VecDeque::with_capacity(BLOCK_BITS + 1),
            hold: 0,
            count: 0,
            last: None,
            confirmed: false,
            recent: VecDeque::new(),
            layer: F06Blocks::new(),
            good: 0,
            corrected: 0,
            crc_errors: 0,
            reversed: None,
        }
    }

    /// Forgets the bits and the numbering (the kind of traffic stays).
    pub fn restart(&mut self) {
        self.bits.clear();
        self.hold = 0;
        self.last = None;
        self.confirmed = false;
    }

    /// Takes one bit (1 for the high tone) with its soft value.
    pub fn push(&mut self, bit: u8, soft: f32, out: &mut Vec<F06Row>) {
        self.count += 1;
        self.bits.push_back((bit, soft));
        if self.bits.len() > BLOCK_BITS {
            self.bits.pop_front();
        }
        if self.hold > 0 {
            self.hold -= 1;
            return;
        }
        if self.bits.len() < BLOCK_BITS {
            return;
        }
        let word = self
            .bits
            .iter()
            .take(32)
            .fold(0u32, |w, &(b, _)| (w << 1) | u32::from(b));
        let senses: &[bool] = match self.polarity {
            Polarity::Auto => &[false, true],
            Polarity::Normal => &[false],
            Polarity::Reversed => &[true],
        };
        for &rev in senses {
            let want = if rev { !SYNC } else { SYNC };
            let errors = (word ^ want).count_ones();
            if errors > self.max_sync_errors {
                continue;
            }
            self.candidate(rev, errors, out);
            return;
        }
    }

    fn candidate(&mut self, rev: bool, sync_errors: u32, out: &mut Vec<F06Row>) {
        let mut bits: Vec<u8> = self.bits.iter().map(|&(b, _)| b ^ u8::from(rev)).collect();
        let mut b = bytes_of(&bits);
        let mut u = unpack(&b);
        let divider = is_divider(&b);
        let mut fixed = false;
        if !divider && !u.crc_ok() && self.correct {
            let mut order: Vec<usize> = (48..192).collect();
            order.sort_by(|&x, &y| self.bits[x].1.total_cmp(&self.bits[y].1));
            for i in order {
                bits[i] ^= 1;
                let tb = bytes_of(&bits);
                let tu = unpack(&tb);
                if tu.crc_ok() && !is_divider(&tb) {
                    b = tb;
                    u = tu;
                    fixed = true;
                    break;
                }
                bits[i] ^= 1;
            }
        }
        let good = divider || u.crc_ok();
        if !good && sync_errors > 1 {
            return;
        }
        self.hold = BLOCK_BITS - 40;
        if divider {
            out.push(F06Blocks::divider());
            self.good += 1;
            return;
        }
        let mut number = u.number;
        let mut crc: &'static str = if !good {
            "CRC error"
        } else if fixed {
            "CRC OK (one bit corrected)"
        } else {
            "CRC OK"
        };
        if good {
            self.reversed = Some(rev);
            let predicted = self.last.and_then(|(n0, b0)| {
                let d = self.count - b0;
                (d > 0 && d.is_multiple_of(BLOCK_BITS as u64) && d / BLOCK_BITS as u64 <= 64)
                    .then(|| (u64::from(n0) + d / BLOCK_BITS as u64) as u16)
            });
            match predicted {
                Some(p) if p == number => self.confirmed = true,
                Some(p) if self.confirmed => {
                    number = p;
                    if !fixed {
                        crc = "CRC OK (block number from the one before)";
                    }
                }
                _ => self.confirmed = false,
            }
            self.last = Some((number, self.count));
            if self.recent.contains(&number) {
                return;
            }
            self.recent.push_back(number);
            if self.recent.len() > 8 {
                self.recent.pop_front();
            }
            self.good += 1;
            self.corrected += i64::from(fixed);
        } else {
            self.crc_errors += 1;
        }
        let _ = b;
        self.layer.block(number, &u.data, good, crc, out);
    }
}

// ---- The layer ----

/// A row as kept.
#[derive(Clone, Debug)]
struct Row {
    time: f64,
    kind: &'static str,
    id: String,
    text: String,
    snr: Option<f64>,
}

/// The FSK200 layer.
pub struct Fsk200 {
    sub: Submode,
    polarity: Polarity,
    case: Case,
    sync_errors: i64,
    correct: bool,
    search: ToneSetSearch,
    corr: ToneCorrelator,
    readers: [F01Reader; 2],
    ring: VecDeque<f32>,
    n: u64,
    winner: Option<usize>,
    gate: bool,
    text: F01Text,
    demod: MfskDemod,
    f06: F06Reader,
    mixed: Vec<Complex32>,
    energies: Vec<f32>,
    f01_rows: Vec<F01Row>,
    f06_rows: Vec<F06Row>,
    rows: RowLog<Row>,
    log: TextLog,
    snr: Option<f64>,
    read_in_lock: i64,
}

impl Default for Fsk200 {
    fn default() -> Self {
        Self::new()
    }
}

fn search_config(sub: Submode) -> SearchConfig {
    let h = sub.shift() / 2.0;
    let (half, span) = match sub {
        Submode::F01 => (175.0, 3200.0),
        Submode::F06 => (200.0, 3000.0),
    };
    let comb = CombSpec::new(&[-h, h], half, -span, span)
        .with_thresholds(9.0, 1.25)
        .centred();
    SearchConfig::new(comb, None, 20.0)
}

impl Fsk200 {
    /// A layer at the default settings.
    pub fn new() -> Self {
        let mut f = Fsk200 {
            sub: Submode::F01,
            polarity: Polarity::Auto,
            case: Case::Figures,
            sync_errors: 3,
            correct: true,
            search: ToneSetSearch::new(RATE, search_config(Submode::F01)).expect("valid search"),
            corr: ToneCorrelator::new(&[0.0, 0.1], SPB).expect("valid filters"),
            readers: [F01Reader::new(false), F01Reader::new(true)],
            ring: VecDeque::new(),
            n: 0,
            winner: None,
            gate: false,
            text: F01Text::new(Case::Figures),
            demod: f06_demod(),
            f06: F06Reader::new(3, true, Polarity::Auto),
            mixed: Vec::new(),
            energies: Vec::new(),
            f01_rows: Vec::new(),
            f06_rows: Vec::new(),
            rows: RowLog::default(),
            log: TextLog::default(),
            snr: None,
            read_in_lock: 0,
        };
        f.rebuild();
        f
    }

    fn rebuild(&mut self) {
        self.search = ToneSetSearch::new(RATE, search_config(self.sub)).expect("valid search");
        self.text = F01Text::new(self.case);
        self.f06 = F06Reader::new(self.sync_errors as u32, self.correct, self.polarity);
        self.restart();
    }

    fn restart(&mut self) {
        let h = self.sub.shift() / 2.0 / RATE;
        self.corr = ToneCorrelator::new(&[-h, h], SPB).expect("valid filters");
        self.readers = [F01Reader::new(false), F01Reader::new(true)];
        self.ring.clear();
        self.n = 0;
        self.winner = match self.polarity {
            Polarity::Auto => None,
            Polarity::Normal => Some(0),
            Polarity::Reversed => Some(1),
        };
        self.gate = false;
        self.demod = f06_demod();
        self.f06.restart();
    }

    /// One mixed sample of f01.
    fn f01_sample(&mut self, v: Complex32) {
        self.corr.push(v);
        self.n += 1;
        let mut e = [0f32; 2];
        let _ = self.corr.energies(&mut e);
        let d = if e[0] + e[1] > 0.0 {
            (e[0] - e[1]) / (e[0] + e[1])
        } else {
            0.0
        };
        self.ring.push_back(d.abs());
        if self.ring.len() > 512 {
            self.ring.pop_front();
        }
        for r in 0..2 {
            let code = self.readers[r].framer.push(d);
            let bad = self.readers[r].framer.framing_error();
            if code.is_none() && !bad {
                continue;
            }
            let good = code.is_some();
            let value = code.unwrap_or(0);
            let reader = &mut self.readers[r];
            reader.valid.push_back(good);
            if reader.valid.len() > 24 {
                reader.valid.pop_front();
            }
            let bits: Vec<f64> = reader.framer.bits().iter().skip(1).map(|b| b.0).collect();
            for t in bits {
                let back = (self.n as f64 - 1.0 - t).round() as usize;
                let q = self
                    .ring
                    .len()
                    .checked_sub(back + 1)
                    .and_then(|i| self.ring.get(i));
                reader.quality.push_back(q.copied().unwrap_or(0.0));
                if reader.quality.len() > 24 {
                    reader.quality.pop_front();
                }
            }
            self.f01_char(r, value, good);
        }
        let gate = self.readers[0].quality().max(self.readers[1].quality()) >= 0.6;
        if self.gate && !gate {
            self.text.flush(&mut self.f01_rows);
        }
        self.gate = gate;
        if self.winner.is_none() {
            let v = [self.readers[0].valid_ratio(), self.readers[1].valid_ratio()];
            for r in 0..2 {
                if let (Some(a), b) = (v[r], v[1 - r].unwrap_or(0.0)) {
                    if a >= 0.85 && a > b + 0.2 {
                        self.winner = Some(r);
                        let kept: Vec<(u8, bool)> = self.readers[r].kept.drain(..).collect();
                        if self.gate {
                            for (c, g) in kept {
                                self.text.push(c, g, &mut self.f01_rows);
                            }
                        }
                        break;
                    }
                }
            }
        }
    }

    fn f01_char(&mut self, r: usize, code: u8, good: bool) {
        match self.winner {
            Some(w) if w == r => {
                if self.gate {
                    self.text.push(code, good, &mut self.f01_rows);
                }
            }
            Some(_) => {}
            None => {
                let k = &mut self.readers[r].kept;
                k.push_back((code, good));
                if k.len() > 64 {
                    k.pop_front();
                }
            }
        }
    }

    fn f06_symbols(&mut self) {
        let e = std::mem::take(&mut self.energies);
        for s in e.as_chunks::<2>().0 {
            let [lo, hi] = *s;
            let bit = u8::from(hi > lo);
            let soft = if lo + hi > 0.0 {
                (hi - lo).abs() / (hi + lo)
            } else {
                0.0
            };
            self.f06.push(bit, soft, &mut self.f06_rows);
        }
        self.energies = e;
        self.energies.clear();
    }

    fn take(&mut self, cx: &mut Cx) {
        let id = self.sub.id();
        for r in std::mem::take(&mut self.f01_rows) {
            let (kind, text) = match r {
                F01Row::Block(t) => ("Block", t),
                F01Row::Header(t) => ("Header", t),
                F01Row::Text(t) => ("Text", t),
            };
            self.read_in_lock += 1;
            self.log.line(&text);
            cx.events
                .push(TEXT.event(cx.time, cx.dial, &TextEvent { text: &text }));
            self.rows.push(Row {
                time: cx.time,
                kind,
                id: id.to_string(),
                text,
                snr: self.snr,
            });
        }
        for r in std::mem::take(&mut self.f06_rows) {
            self.read_in_lock += 1;
            self.log.line(&r.text);
            cx.events.push(F06_BLOCK.event(
                cx.time,
                cx.dial,
                &F06BlockEvent {
                    text: &r.text,
                    number: r.number.map(i64::from),
                    kind: r.kind,
                    crc: r.crc,
                },
            ));
            self.rows.push(Row {
                time: cx.time,
                kind: r.kind,
                id: r.id,
                text: r.text,
                snr: self.snr,
            });
        }
    }
}

fn f06_demod() -> MfskDemod {
    let mut cfg = MfskConfig::from_hz(&[-500.0, 500.0], BAUD, RATE);
    cfg.average_symbols = 16.0;
    MfskDemod::new(cfg).expect("valid demodulator")
}

impl Layer for Fsk200 {
    const NAME: &'static str = "hfdata.fsk200";
    const DECIM: usize = 3;
    const PASS_HZ: f64 = 3400.0;
    fn schema() -> Arc<Schema> {
        Fsk200RxSetting::schema()
    }
    fn declaration() -> ResultsDecl {
        ResultsDecl::new()
            .with_fields(Fsk200Fields::SHAPE.fields)
            .with_table(&SENDER_ROWS)
            .event(&TEXT)
            .event(&F06_BLOCK)
            .event(&TONES_FOUND)
            .event(&TONES_LOST)
    }
    fn values(&self) -> Values {
        values_of([
            (Fsk200RxSetting::Submode, Value::Choice(self.sub.index())),
            (
                Fsk200RxSetting::Polarity,
                Value::Choice(self.polarity.index()),
            ),
            (Fsk200RxSetting::Case, Value::Choice(self.case.index())),
            (Fsk200RxSetting::SyncErrors, Value::I64(self.sync_errors)),
            (Fsk200RxSetting::CorrectOneBit, Value::Bool(self.correct)),
        ])
    }
    fn apply(&mut self, id: SettingId, value: &Value) -> Result<(), SettingError> {
        match Fsk200RxSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            Fsk200RxSetting::Submode => self.sub = SUBMODE.read(id, value)?,
            Fsk200RxSetting::Polarity => self.polarity = POLARITY.read(id, value)?,
            Fsk200RxSetting::Case => self.case = CASE.read(id, value)?,
            Fsk200RxSetting::SyncErrors => self.sync_errors = SYNC_ERRORS.read(id, value)?,
            Fsk200RxSetting::CorrectOneBit => self.correct = CORRECT_ONE_BIT.read(id, value)?,
        }
        self.rebuild();
        Ok(())
    }
    fn push(&mut self, x: &[Complex32], cx: &mut Cx) {
        for &v in x {
            match self.search.push(v) {
                Some(SearchEvent::Found { df_hz, snr_db }) => {
                    self.restart();
                    self.read_in_lock = 0;
                    self.snr = Some(snr_db);
                    cx.events.push(TONES_FOUND.event(
                        cx.time,
                        cx.dial,
                        &TonesFoundEvent {
                            text: &format!("{} tones found, centre {df_hz:+.1} Hz", self.sub.id()),
                            offset: Hertz(df_hz),
                            snr: snr_db,
                        },
                    ));
                }
                Some(SearchEvent::Lost { df_hz }) => {
                    self.text.flush(&mut self.f01_rows);
                    self.take(cx);
                    cx.events.push(TONES_LOST.event(
                        cx.time,
                        cx.dial,
                        &TonesLostEvent {
                            text: &format!("{} tones lost", self.sub.id()),
                            offset: Hertz(df_hz),
                            read: self.read_in_lock,
                        },
                    ));
                    self.restart();
                    self.snr = None;
                }
                None => {}
            }
            if let Some(f) = self.search.last_fit().filter(|_| self.search.locked()) {
                self.snr = Some(f.snr_db);
            }
            loop {
                self.mixed.clear();
                if self.search.pull(80, &mut self.mixed) == 0 {
                    break;
                }
                let mixed = std::mem::take(&mut self.mixed);
                match self.sub {
                    Submode::F01 => mixed.iter().for_each(|&y| self.f01_sample(y)),
                    Submode::F06 => {
                        self.demod.process(&mixed, &mut self.energies);
                        self.f06_symbols();
                    }
                }
                self.mixed = mixed;
            }
            self.take(cx);
        }
    }
    fn finish(&mut self, cx: &mut Cx) {
        self.text.flush(&mut self.f01_rows);
        self.take(cx);
    }
    fn reset(&mut self) {
        self.search.reset();
        self.text.flush(&mut Vec::new());
        self.f06.layer = F06Blocks::new();
        self.restart();
        self.snr = None;
    }
    fn clear(&mut self) {
        self.rows.clear();
        self.log.clear();
        self.text.chars = 0;
        self.text.framing = 0;
        self.f06.good = 0;
        self.f06.corrected = 0;
        self.f06.crc_errors = 0;
    }
    fn forget(&mut self, kept: &Kept) {
        self.reset();
        if !kept.table(SENDER_ROWS.key) {
            self.rows.clear();
        }
        if !kept.field("text") {
            self.log.clear();
        }
        self.text.chars = 0;
        self.text.framing = 0;
        self.f06.good = 0;
        self.f06.corrected = 0;
        self.f06.crc_errors = 0;
    }
    fn report(&mut self, res: &mut Published, level_dbfs: f64) -> StatusLine {
        self.rows.report(
            res,
            |r| r.clear_rows(&SENDER_ROWS),
            |r, key, row| {
                SENDER_ROWS.put(
                    r,
                    key,
                    &SenderRow {
                        time: Time(row.time),
                        kind: row.kind,
                        id: &row.id,
                        text: &row.text,
                        snr: row.snr,
                    },
                );
            },
        );
        let df = self.search.offset_hz();
        let snr = df.and(self.snr);
        let h = self.sub.shift() / 2.0;
        let (sense, decoding) = match self.sub {
            Submode::F01 => (
                self.winner.map(|w| {
                    if w == 0 {
                        "(low tone is mark)"
                    } else {
                        "(high tone is mark: reversed)"
                    }
                }),
                self.winner.is_some() && self.text.chars > 0,
            ),
            Submode::F06 => (
                self.f06.reversed.map(|r| if r { "(reversed)" } else { "" }),
                self.f06.good > 0,
            ),
        };
        let tones = df.map(|d| {
            format!(
                "{:.0} / {:.0} Hz from the dial {}",
                d - h,
                d + h,
                sense.unwrap_or("")
            )
            .trim_end()
            .to_string()
        });
        let state: u8 = match (df, decoding) {
            (Some(_), true) => 2,
            (Some(_), false) => 1,
            _ => 0,
        };
        let f06 = self.sub == Submode::F06;
        let traffic = f06.then(|| self.f06.layer.traffic.label());
        res.put(&Fsk200Fields {
            note: NOTE_TEXT,
            text: self.log.as_str(),
            state: i64::from(state),
            offset_hz: df.map(Hertz),
            snr_db: snr.map(|v| (v * 10.0).round() / 10.0),
            tones: tones.as_deref(),
            traffic,
            characters: (!f06).then_some(self.text.chars),
            framing_errors: (!f06).then_some(self.text.framing),
            blocks_good: f06.then_some(self.f06.good),
            blocks_corrected: f06.then_some(self.f06.corrected),
            crc_errors: f06.then_some(self.f06.crc_errors),
            level_dbfs,
        });
        let summary = match (self.sub, df) {
            (_, None) => format!("searching for {} tones", self.sub.id()),
            (Submode::F01, Some(_)) => format!("FSK200/500: {} characters", self.text.chars),
            (Submode::F06, Some(_)) => format!(
                "FSK200/1000 {}: {} blocks, {} with a CRC error",
                self.f06.layer.traffic.label(),
                self.f06.good,
                self.f06.crc_errors
            ),
        };
        StatusLine {
            state,
            summary,
            snr_db: snr,
            offset_hz: df,
        }
    }
}

/// The submode of a configuration: the submode named, else the `submode` setting.
pub fn submode_of(sub: Option<&str>, settings: &Values) -> Submode {
    sub.and_then(Submode::from_key)
        .unwrap_or_else(|| settings.get(&SUBMODE))
}

/// The schema of the stack.
pub fn schema() -> Arc<Schema> {
    Fsk200Setting::schema()
}

// ---- The test signals ----

/// The CRC-16 of the generator: polynomial 1021 reflected, start FFFF, final XOR FFFF, bit by bit.
pub fn gen_crc16(data: &[u8]) -> u16 {
    let mut c: u16 = 0xFFFF;
    for &b in data {
        c ^= u16::from(b);
        for _ in 0..8 {
            c = if c & 1 != 0 {
                (c >> 1) ^ 0x8408
            } else {
                c >> 1
            };
        }
    }
    !c
}

/// The CRC-32 of the generator (CRC-32/ISO-HDLC), bit by bit.
pub fn gen_crc32(data: &[u8]) -> u32 {
    let mut c: u32 = 0xFFFF_FFFF;
    for &b in data {
        c ^= u32::from(b);
        for _ in 0..8 {
            c = if c & 1 != 0 {
                (c >> 1) ^ 0xEDB8_8320
            } else {
                c >> 1
            };
        }
    }
    !c
}

/// The 36 bytes of a block numbered `number` with `data` (the 5-bit code of the number and bytes 24 to 35 from `rest`, whose
/// rules are not known).
pub fn pack(number: u16, data: &[u8; 16], rest: &[u8; 12]) -> [u8; 36] {
    let mut b = [0u8; 36];
    b[..4].copy_from_slice(&SYNC.to_be_bytes());
    b[4] = (number >> 3) as u8;
    b[5] = ((number & 7) << 5) as u8;
    let crc = gen_crc16(data);
    let mut nib = [0u8; 36];
    for i in 0..16 {
        nib[2 * i] = data[i] >> 4;
        nib[2 * i + 1] = data[i] & 15;
    }
    for k in 0..4 {
        nib[32 + k] = ((crc >> (12 - 4 * k)) & 15) as u8;
    }
    for (k, &n) in nib.iter().enumerate() {
        let (byte, high) = nibble_place(k);
        b[byte] |= if high { n << 4 } else { n };
    }
    b[24..].copy_from_slice(rest);
    b
}

/// The data of a figure block of `numbers` (each under 1000) and two figures.
pub fn figure_data(numbers: &[u16; 12], figs: [u8; 2]) -> [u8; 16] {
    let mut bits = Vec::with_capacity(128);
    for &n in numbers {
        bits.extend((0..10).rev().map(|i| ((n >> i) & 1) as u8));
    }
    for f in figs {
        bits.extend((0..4).rev().map(|i| (f >> i) & 1));
    }
    let mut d = [0u8; 16];
    for (i, b) in bits.iter().enumerate() {
        d[i / 8] |= b << (7 - i % 8);
    }
    d
}

/// The data of a header block: `count` blocks and the message starts.
pub fn header_data(count: u16, starts: &[u16]) -> [u8; 16] {
    let mut d = [0u8; 16];
    d[0] = (count >> 3) as u8;
    d[1] = (((count & 7) << 5) as u8) | (starts.len() as u8 & 31);
    for (i, &s) in starts.iter().take(7).enumerate() {
        d[2 + 2 * i] = (s >> 3) as u8;
        d[3 + 2 * i] = ((s & 7) << 5) as u8;
    }
    d
}

/// The blocks of the f06 test signal (number and bytes) and, for figures, the digits each figure block carries. A header block
/// 0 (the block count, one message at block 1), the message from block 1, a header block at each number that is a multiple
/// of 16.
pub fn f06_blocks(
    kind: GenKind,
    blocks: usize,
    text: &str,
    name: &str,
    seed: u64,
) -> Vec<(u16, [u8; 36], Option<String>)> {
    let mut r = Lcg(seed ^ 0xF06);
    let mut rest = || -> [u8; 12] { std::array::from_fn(|_| r.below(256) as u8) };
    let mut payload: Vec<([u8; 16], Option<String>)> = Vec::new();
    let mut rnd = Lcg(seed ^ 0x6F0);
    match kind {
        GenKind::F06 => {
            for k in 0..blocks {
                let mut n: [u16; 12] = std::array::from_fn(|_| rnd.below(1000) as u16);
                if k == 0 {
                    n[0] = 111;
                }
                let f = [rnd.below(10) as u8, rnd.below(10) as u8];
                let d = figure_data(&n, f);
                payload.push((d, figures(&d)));
            }
        }
        GenKind::Ascii => {
            let mut bytes = vec![0x34u8, 0x36];
            bytes.extend(b"ABC 123 XYZ...");
            bytes.extend(text.bytes());
            for c in bytes.chunks(16) {
                let mut d = [b' '; 16];
                d[..c.len()].copy_from_slice(c);
                payload.push((d, None));
            }
        }
        GenKind::F06a => {
            let content = text.as_bytes();
            let mut file = Vec::new();
            file.extend((content.len() as u32).to_be_bytes());
            let nm: Vec<u8> = name.bytes().take(12).collect();
            file.extend(std::iter::repeat_n(0u8, 12 - nm.len()));
            file.extend(&nm);
            file.extend((!gen_crc32(content)).to_be_bytes());
            file.extend(content);
            file.push(0);
            while !file.len().is_multiple_of(16) {
                file.push(0x99);
            }
            for c in file.chunks(16) {
                payload.push((c.try_into().unwrap_or([0; 16]), None));
            }
        }
    }
    let mut numbered = Vec::new();
    let mut n = 1u16;
    for p in payload {
        while n.is_multiple_of(16) {
            numbered.push((n, None));
            n += 1;
        }
        numbered.push((n, Some(p)));
        n += 1;
    }
    let count = n;
    let mut out = vec![(0u16, pack(0, &header_data(count, &[1]), &rest()), None)];
    for (n, p) in numbered {
        match p {
            None => out.push((n, pack(n, &header_data(count, &[1]), &rest()), None)),
            Some((d, digits)) => out.push((n, pack(n, &d, &rest()), digits)),
        }
    }
    out
}

/// `n` random five-figure groups from `seed`.
pub fn test_groups(n: usize, seed: u64) -> Vec<String> {
    let mut r = Lcg(seed ^ 0xF01);
    (0..n)
        .map(|_| (0..5).map(|_| (b'0' + r.below(10) as u8) as char).collect())
        .collect()
}

/// The figures of the f01 test message: a header block and the data blocks of `groups` (three to a block; the last block
/// typed `)` and filled with `+++++`).
pub fn f01_text(groups: &[String], seed: u64) -> String {
    let mut r = Lcg(seed ^ 0x10F);
    let mut digits =
        |n: usize| -> String { (0..n).map(|_| (b'0' + r.below(10) as u8) as char).collect() };
    let mut s = format!(
        "({}11177{}{}{}{:05}",
        digits(3),
        digits(5),
        digits(5),
        digits(5),
        groups.len()
    );
    let blocks = groups.len().div_ceil(3);
    for b in 0..blocks {
        let t = if b + 1 == blocks { ')' } else { '=' };
        let mut g: Vec<String> = groups.iter().skip(3 * b).take(3).cloned().collect();
        while g.len() < 3 {
            g.push("+++++".into());
        }
        s.push_str(&format!(
            "{t}{}{:>3}{}{}",
            digits(3),
            b + 1,
            g.concat(),
            digits(3)
        ));
    }
    s
}

/// The tone sequence of the f01 test signal: 1 s of reversals, 4 units of mark, FIGS, [`f01_text`], 10 units of mark.
pub fn f01_sequence(text: &str, centre: f64, reversed: bool) -> Vec<Tone> {
    let unit = 1.0 / BAUD;
    let (mark, space) = if reversed {
        (centre + 250.0, centre - 250.0)
    } else {
        (centre - 250.0, centre + 250.0)
    };
    let mut seq = vec![Tone::silence(1.0)];
    for i in 0..200 {
        seq.push(Tone::new(unit, if i % 2 == 0 { mark } else { space }));
    }
    seq.push(Tone::new(4.0 * unit, mark));
    let mut codes = vec![ita2::FIGS];
    codes.extend(text.chars().filter_map(f01_code));
    for c in codes {
        seq.push(Tone::new(unit, space));
        for k in 0..5 {
            seq.push(Tone::new(
                unit,
                if (c >> k) & 1 == 1 { mark } else { space },
            ));
        }
        seq.push(Tone::new(1.5 * unit, mark));
    }
    seq.push(Tone::new(10.0 * unit, mark));
    seq.push(Tone::silence(1.0));
    seq
}

/// The tone sequence of the f06 test signal: 1 s of reversals, the blocks back to back (with `flips` of (block index, bit)
/// flipped), 20 bits of reversals.
pub fn f06_sequence(
    blocks: &[(u16, [u8; 36], Option<String>)],
    flips: &[(usize, usize)],
    centre: f64,
    reversed: bool,
) -> Vec<Tone> {
    let unit = 1.0 / BAUD;
    let tone = |b: u8| {
        let hi = (b == 1) != reversed;
        Tone::new(unit, if hi { centre + 500.0 } else { centre - 500.0 })
    };
    let mut seq = vec![Tone::silence(1.0)];
    for i in 0..200 {
        seq.push(tone((i % 2) as u8));
    }
    for (k, (_, b, _)) in blocks.iter().enumerate() {
        for i in 0..BLOCK_BITS {
            let mut v = (b[i / 8] >> (7 - i % 8)) & 1;
            if flips.contains(&(k, i)) {
                v ^= 1;
            }
            seq.push(tone(v));
        }
    }
    for i in 0..20 {
        seq.push(tone((i % 2) as u8));
    }
    seq.push(Tone::silence(1.0));
    seq
}

/// The test signal of the submode.
pub fn generator(cfg: &GenConfig) -> Result<Box<dyn Generator>, ConfigError> {
    let s = &cfg.settings;
    let centre = s.get(&GEN_CENTRE_HZ);
    let rev = s.get(&GEN_REVERSED);
    let seq = match submode_of(cfg.submode.as_deref(), s) {
        Submode::F01 => {
            let n = s.get(&GEN_GROUPS).clamp(3, 2997) as usize;
            f01_sequence(&f01_text(&test_groups(n, cfg.seed), cfg.seed), centre, rev)
        }
        Submode::F06 => {
            let blocks = f06_blocks(
                s.get(&GEN_KIND),
                s.get(&GEN_BLOCKS).clamp(1, 1000) as usize,
                s.get(&GEN_TEXT),
                s.get(&GEN_NAME),
                cfg.seed,
            );
            let flips: Vec<(usize, usize)> = s
                .get(&GEN_BIT_ERRORS)
                .split(',')
                .filter_map(|p| {
                    let (a, b) = p.trim().split_once(':')?;
                    let n: u16 = a.trim().parse().ok()?;
                    let k = blocks.iter().position(|(m, _, _)| *m == n)?;
                    Some((k, b.trim().parse().ok()?))
                })
                .collect();
            f06_sequence(&blocks, &flips, centre, rev)
        }
    };
    Ok(Box::new(Playback::new(
        &seq,
        cfg.rate_hz,
        SignalInfo {
            symbol_rate_hz: Some(BAUD),
            ..SignalInfo::new(3000.0)
        },
    )))
}

/// `cfg` with its submode written into the `submode` setting.
fn with_submode(cfg: &StackConfig) -> Result<StackConfig, ConfigError> {
    let mut cfg = cfg.clone();
    if let Some(sub) = cfg.submode.as_deref().and_then(Submode::from_key) {
        if let Ok(id) = schema().id(SUBMODE.field.key) {
            cfg.settings.set(id, &Value::Choice(sub.index()))?;
        }
    }
    Ok(cfg)
}

fn make_receiver(cfg: &StackConfig) -> Result<Box<dyn Receiver>, ConfigError> {
    crate::receiver::decoder_receiver(
        crate::typed::shape("fsk200"),
        "FSK200",
        Sender::new(Fsk200::new()),
        &with_submode(cfg)?,
    )
}

/// The dial of the test signal. It is not a station frequency: the host gives the frequency.
pub const TEST_SIGNAL_HZ: f64 = 6.5e6;

/// FSK 200/500 and FSK 200/1000.
pub static FSK200: StackDescriptor = StackDescriptor {
    id: "fsk200",
    name: "FSK200/500 and FSK200/1000",
    group: "HF utility",
    blurb: "200 Bd 2-FSK number senders: F01 (500 Hz shift, ITA2 figure blocks) and F06 (1000 Hz shift, 288-bit blocks under a CRC-16: figure blocks, ASCII, F06a files).",
    tuning: crate::ms110::hf_tuning(TEST_SIGNAL_HZ),
    submodes: &[
        SubMode {
            id: "f01",
            name: "FSK200/500 (F01)",
            default_hz: TEST_SIGNAL_HZ,
        },
        SubMode {
            id: "f06",
            name: "FSK200/1000 (F06)",
            default_hz: TEST_SIGNAL_HZ,
        },
    ],
    settings: schema,
    make_receiver,
    make_generator: Some(generator),
    lock_profile: LockProfile::DEFAULT,
    signal_types: &[],
    auto_gain: AutoGain::Radio,
    verify: Some(&crate::verify::FSK200),
    benchmarks: Some(crate::benchmarks),
};
