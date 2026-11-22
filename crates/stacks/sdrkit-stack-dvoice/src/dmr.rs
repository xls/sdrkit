//! DMR: the bursts of base stations, mobiles and direct mode, the CACH and short LC, slot types, voice LC headers and
//! terminators, the embedded LC with talker alias and GPS, CSBKs, data headers and data blocks, and the AMBE+2 voice frames.
//! Standard: ETSI TS 102 361-1 to -4.

use crate::bits::{bits_of, bits_to_bytes, bytes_to_bits};
use crate::decoder::{Ctx, ModeParams, Protocol};
use crate::gen::{bits_to_levels, silence, FskModulator, Prepared, TxPulse};
use crate::items::*;
use crate::symbols::{Sym, SymAnswer, SyncAnswer, SyncPattern};
use crate::typed::DmrFields;
use sdrkit_algo_fec::conv::tcm::{TcmCodec, DMR_3_4};
use sdrkit_algo_fec::rs::{codes::DMR_12_9, Rs};
use sdrkit_algo_fec::small::{ProductCode, SmallCode};
use sdrkit_core::interface::Fields;
use sdrkit_core::media::Codec;
use sdrkit_core::publish::Published;
use sdrkit_core::publish::Declare;
use sdrkit_core::{Complex32, GenConfig, Value};
use sdrkit_util_bits::crc::catalog::{crc32_dmr_packet, CRC16_GSM, CRC8_SMBUS};
use sdrkit_util_bits::interleave::Permutation;
use sdrkit_util_bits::pack::{push_u64, to_u64};
use std::sync::LazyLock;

sdrkit_core::choice_enum! {
    /// Who sends the bursts of the channel.
    pub enum Link {
        /// A base station.
        BaseStation = ("base_station", "base station"),
        /// A mobile.
        Mobile = ("mobile", "mobile"),
        /// Two radios in direct mode.
        DirectMode = ("direct_mode", "direct mode"),
    }
}

sdrkit_core::choice_enum! {
    /// The activity of a slot that the short LC of the CACH announces. Standard: ETSI TS 102 361-2 clause 7.1.3.
    pub enum Activity {
        /// No activity.
        None = ("none", "none"),
        /// A group CSBK.
        GroupCsbk = ("group_csbk", "group CSBK"),
        /// A private CSBK.
        PrivateCsbk = ("private_csbk", "private CSBK"),
        /// Group voice.
        GroupVoice = ("group_voice", "group voice"),
        /// Private voice.
        PrivateVoice = ("private_voice", "private voice"),
        /// Private data.
        PrivateData = ("private_data", "private data"),
        /// Group data.
        GroupData = ("group_data", "group data"),
        /// Emergency group voice.
        EmergencyGroupVoice = ("emergency_group_voice", "emergency group voice"),
        /// Emergency private voice.
        EmergencyPrivateVoice = ("emergency_private_voice", "emergency private voice"),
        /// Another activity.
        Other = ("other", "other"),
    }
}

impl Activity {
    /// The activity of the 4-bit code `a`.
    pub fn of(a: u8) -> Activity {
        match a {
            0 => Activity::None,
            2 => Activity::GroupCsbk,
            3 => Activity::PrivateCsbk,
            8 => Activity::GroupVoice,
            9 => Activity::PrivateVoice,
            10 => Activity::PrivateData,
            11 => Activity::GroupData,
            12 => Activity::EmergencyGroupVoice,
            13 => Activity::EmergencyPrivateVoice,
            _ => Activity::Other,
        }
    }
}

/// The nine sync patterns (TS 102 361-1 table 9.2).
pub const SYNCS: [(&str, &str); 9] = [
    ("bs_voice", "755FD7DF75F7"),
    ("bs_data", "DFF57D75DF5D"),
    ("ms_voice", "7F7D5DD57DFD"),
    ("ms_data", "D5D7F77FD757"),
    ("rc", "77D55F7DFD77"),
    ("dm1_voice", "5D577F7757FF"),
    ("dm1_data", "F7FDD5DDFD55"),
    ("dm2_voice", "7DFFD5F55D5F"),
    ("dm2_data", "D7557F5FF7F5"),
];

fn sync_bits(i: usize) -> [u8; 48] {
    let v = u64::from_str_radix(SYNCS[i].1, 16).expect("a constant hex word");
    std::array::from_fn(|k| ((v >> (47 - k)) & 1) as u8)
}

/// Data types of the slot type field.
pub mod dt {
    pub const PI_HEADER: u8 = 0;
    pub const VOICE_LC_HEADER: u8 = 1;
    pub const TERMINATOR: u8 = 2;
    pub const CSBK: u8 = 3;
    pub const MBC_HEADER: u8 = 4;
    pub const MBC_CONT: u8 = 5;
    pub const DATA_HEADER: u8 = 6;
    pub const RATE_12: u8 = 7;
    pub const RATE_34: u8 = 8;
    pub const IDLE: u8 = 9;
    pub const RATE_1: u8 = 10;
}

/// CRC masks per data type (TS 102 361-1 B.3.12).
fn crc_mask(data_type: u8) -> u16 {
    match data_type {
        dt::PI_HEADER => 0x6969,
        dt::CSBK => 0xA5A5,
        dt::MBC_HEADER => 0xAAAA,
        dt::DATA_HEADER => 0xCCCC,
        _ => 0,
    }
}

/// A full link control word.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Lc {
    pub pf: bool,
    pub flco: u8,
    pub fid: u8,
    pub opts: u8,
    pub dst: u32,
    pub src: u32,
}

impl Lc {
    pub fn from_bytes(b: &[u8]) -> Lc {
        Lc {
            pf: b[0] & 0x80 != 0,
            flco: b[0] & 0x3F,
            fid: b[1],
            opts: b[2],
            dst: u32::from_be_bytes([0, b[3], b[4], b[5]]),
            src: u32::from_be_bytes([0, b[6], b[7], b[8]]),
        }
    }
    pub fn to_bytes(&self) -> [u8; 9] {
        let d = self.dst.to_be_bytes();
        let s = self.src.to_be_bytes();
        [
            ((self.pf as u8) << 7) | self.flco,
            self.fid,
            self.opts,
            d[1],
            d[2],
            d[3],
            s[1],
            s[2],
            s[3],
        ]
    }
    pub fn emergency(&self) -> bool {
        self.opts & 0x80 != 0
    }
    pub fn privacy(&self) -> bool {
        self.opts & 0x40 != 0
    }
    /// The call kind of a voice LC.
    pub fn kind(&self) -> CallKind {
        match self.flco {
            0 if self.dst == 0xFF_FFFF => CallKind::AllCall,
            0 => CallKind::GroupVoice,
            3 => CallKind::PrivateVoice,
            _ => CallKind::Voice,
        }
    }
}

/// Golay (20,8) of the slot type (TS 102 361-1 clause B.3.1).
static SLOT_TYPE: LazyLock<SmallCode> = LazyLock::new(SmallCode::golay_20_8);
/// Hamming (7,4) of the CACH TACT bits (clause B.3.5).
static TACT: LazyLock<SmallCode> = LazyLock::new(SmallCode::hamming_7_4);
/// QR (16,7) of the EMB field (clause B.3.2).
static EMB: LazyLock<SmallCode> = LazyLock::new(SmallCode::qr_16_7);
/// Hamming (17,12) of the short LC rows (clause B.3.3).
static SHORT_LC_ROW: LazyLock<SmallCode> = LazyLock::new(SmallCode::hamming_17_12);
/// Hamming (16,11) of the embedded LC rows (clause B.2.2).
static EMBEDDED_ROW: LazyLock<SmallCode> = LazyLock::new(SmallCode::hamming_16_11);
/// BPTC (196,96): Hamming (15,11) rows and (13,9) columns (clause B.1.1).
static BPTC: LazyLock<ProductCode> =
    LazyLock::new(|| ProductCode::new(SmallCode::hamming_15_11(), SmallCode::hamming_13_9()));
/// The BPTC interleave: matrix bit `i` is sent at position `181 i mod 196` (clause B.1.1).
static BPTC_ORDER: LazyLock<Permutation> =
    LazyLock::new(|| Permutation::multiplicative(181, 0, 196).expect("181 is prime to 196"));
/// The short LC interleave: matrix bit `i` (below 67) is sent at position `4 i mod 67` (TS 102 361-2 clause 7.1.3).
static SHORT_LC_ORDER: LazyLock<Permutation> =
    LazyLock::new(|| Permutation::multiplicative(4, 0, 67).expect("4 is prime to 67"));
/// The RS(12,9) encoder of the full LC (clause B.3.6).
static FULL_LC_RS: LazyLock<Rs> = LazyLock::new(|| Rs::new(DMR_12_9).expect("valid RS code"));

/// The decoders a DMR receiver keeps (they hold scratch space, so each receiver has its own).
#[derive(Clone, Debug)]
pub struct Codecs {
    /// RS(12,9) of the full LC.
    pub rs: Rs,
    /// The rate 3/4 trellis code.
    pub tcm: TcmCodec,
}

impl Default for Codecs {
    fn default() -> Self {
        Codecs {
            rs: Rs::new(DMR_12_9).expect("valid RS code"),
            tcm: TcmCodec::new(DMR_3_4).expect("valid trellis code"),
        }
    }
}

/// The RS mask of a full LC: 0x96 in a voice LC header, 0x99 in a terminator (TS 102 361-1 clause B.3.11).
fn full_lc_mask(data_type: u8) -> u8 {
    if data_type == dt::TERMINATOR {
        0x99
    } else {
        0x96
    }
}

/// Encodes a full LC with its RS(12,9) parity masked for `data_type`.
pub fn encode_full_lc(lc: &[u8; 9], data_type: u8) -> [u8; 12] {
    let mut w = [0u8; 12];
    w[..9].copy_from_slice(lc);
    let mut p = [0u8; 3];
    FULL_LC_RS
        .encode(lc, &mut p)
        .expect("9 data and 3 parity symbols");
    let mask = full_lc_mask(data_type);
    for i in 0..3 {
        w[9 + i] = p[i] ^ mask;
    }
    w
}

/// Decodes a full LC (12 bytes from BPTC): the LC and the symbols corrected, or `None` when RS(12,9) cannot correct it.
pub fn decode_full_lc(rs: &mut Rs, w: &[u8], data_type: u8) -> Option<([u8; 9], usize)> {
    let mut v = [0u8; 12];
    v.copy_from_slice(w.get(..12)?);
    let mask = full_lc_mask(data_type);
    for x in v[9..].iter_mut() {
        *x ^= mask;
    }
    let n = rs.decode(&mut v, &[]).ok()?.corrected as usize;
    let mut lc = [0u8; 9];
    lc.copy_from_slice(&v[..9]);
    Some((lc, n))
}

/// Whether the CRC-CCITT of a 12-byte block, masked for its data type, checks (TS 102 361-1 clause B.3.8 and table B.21).
pub fn check_crc16(b: &[u8], data_type: u8) -> bool {
    b.len() >= 12
        && CRC16_GSM.verify_masked(
            &b[..12],
            sdrkit_util_bits::crc::Trailer::Big,
            crc_mask(data_type),
        )
}

/// Appends the masked CRC-CCITT to 10 bytes.
pub fn with_crc16(b: &[u8; 10], data_type: u8) -> [u8; 12] {
    let crc = CRC16_GSM.checksum(b) ^ crc_mask(data_type);
    let mut o = [0u8; 12];
    o[..10].copy_from_slice(b);
    o[10..].copy_from_slice(&crc.to_be_bytes());
    o
}

/// The 5-bit checksum of an embedded LC: the sum of the nine LC octets modulo 31 (TS 102 361-1 clause B.3.11).
pub fn embedded_lc_checksum(lc: &[u8; 9]) -> u8 {
    (lc.iter().map(|&b| b as u32).sum::<u32>() % 31) as u8
}

/// Encodes 96 information bits into the 196 bits of a data burst, in the order sent (TS 102 361-1 clause B.1.1).
pub fn encode_bptc(info: &[u8; 96]) -> [u8; 196] {
    // Three reserved bits lead the 99 information positions of the 9 x 11 matrix. The first sent bit is reserved too.
    let mut i99 = [0u8; 99];
    i99[3..].copy_from_slice(info);
    let mut m = [0u8; 196];
    BPTC.encode(&i99, &mut m[1..]).expect("BPTC sizes");
    let mut out = [0u8; 196];
    BPTC_ORDER.apply(&m, &mut out).expect("196 bits");
    out
}

/// Decodes the 196 bits of a data burst: the 96 information bits, the bits corrected, and whether every row and column checks.
pub fn decode_bptc(raw: &[u8]) -> ([u8; 96], usize, bool) {
    let mut m = [0u8; 196];
    let mut info = [0u8; 96];
    if raw.len() != 196 || BPTC_ORDER.invert(raw, &mut m).is_err() {
        return (info, 0, false);
    }
    let (fixed, good) = match BPTC.decode(&mut m[1..], 5) {
        Ok(d) => (d.corrected as usize, true),
        Err(_) => (0, false),
    };
    let mut k = 0;
    for row in 0..9 {
        let start = if row == 0 { 3 } else { 0 };
        for col in start..11 {
            info[k] = m[1 + row * 15 + col];
            k += 1;
        }
    }
    (info, fixed, good)
}

/// The matrix bit (row by row, 16 columns) sent `o`-th among the 128 embedded bits: `16 o mod 127`, the last bit last (TS 102
/// 361-1 clause B.2.1).
fn embedded_matrix_index(o: usize) -> usize {
    if o == 127 {
        127
    } else {
        (o * 16) % 127
    }
}

/// Encodes 72 LC bits with their 5-bit checksum into the 128 embedded bits, in the order sent (four fragments of 32).
pub fn encode_embedded(lc: &[u8; 72], checksum: u8) -> [u8; 128] {
    let mut m = [0u8; 128];
    let mut k = 0;
    for row in 0..7 {
        let n = if row < 2 { 11 } else { 10 };
        m[row * 16..row * 16 + n].copy_from_slice(&lc[k..k + n]);
        k += n;
        if row >= 2 {
            m[row * 16 + 10] = (checksum >> (6 - row)) & 1;
        }
        let w = EMBEDDED_ROW.encode(to_u64(&m[row * 16..row * 16 + 11]) as u128);
        for c in 0..16 {
            m[row * 16 + c] = (w >> (15 - c)) as u8 & 1;
        }
    }
    for col in 0..16 {
        m[7 * 16 + col] = (0..7).fold(0, |p, row| p ^ m[row * 16 + col]);
    }
    let mut out = [0u8; 128];
    for (o, slot) in out.iter_mut().enumerate() {
        *slot = m[embedded_matrix_index(o)];
    }
    out
}

/// Decodes the 128 embedded bits (in the order received): the 72 LC bits, the checksum, and whether every row and column
/// checks after the row corrections.
pub fn decode_embedded(raw: &[u8]) -> ([u8; 72], u8, bool) {
    let mut lc = [0u8; 72];
    if raw.len() < 128 {
        return (lc, 0, false);
    }
    let mut m = [0u8; 128];
    for (o, &bit) in raw[..128].iter().enumerate() {
        m[embedded_matrix_index(o)] = bit;
    }
    let mut good = true;
    for row in 0..7 {
        let w = to_u64(&m[row * 16..row * 16 + 16]) as u128;
        match EMBEDDED_ROW.decode(w) {
            Some((cw, _)) => {
                for c in 0..16 {
                    m[row * 16 + c] = (cw >> (15 - c)) as u8 & 1;
                }
            }
            None => good = false,
        }
    }
    for col in 0..16 {
        good &= (0..8).fold(0, |p, row| p ^ m[row * 16 + col]) == 0;
    }
    let mut cs = 0u8;
    let mut k = 0;
    for row in 0..7 {
        let n = if row < 2 { 11 } else { 10 };
        lc[k..k + n].copy_from_slice(&m[row * 16..row * 16 + n]);
        k += n;
        if row >= 2 {
            cs = (cs << 1) | m[row * 16 + 10];
        }
    }
    (lc, cs, good)
}

/// Whether a packet data message (user data, pad octets, then the 4 CRC octets) carries its CRC-32 (TS 102 361-1 clause B.3.9):
/// CRC-32/DMR over the octets taken in pairs, sent least significant octet first.
pub fn packet_crc_ok(message: &[u8]) -> bool {
    let Some(n) = message.len().checked_sub(4) else {
        return false;
    };
    let (body, crc) = message.split_at(n);
    crc32_dmr_packet(body) == u32::from_le_bytes([crc[0], crc[1], crc[2], crc[3]])
}

/// The TACT bits (AT, TC, LCSS) of a CACH and their positions among its 24 bits.
const TACT_POS: [usize; 7] = [0, 4, 8, 12, 14, 18, 22];

/// Encodes a CACH: access type, slot of the next burst (0 or 1), LCSS and 17 short LC bits.
pub fn encode_cach(at: bool, tc: u8, lcss: u8, payload: &[u8]) -> [u8; 24] {
    let info = ((at as u128) << 3) | ((tc as u128 & 1) << 2) | (lcss as u128 & 3);
    let w = TACT.encode(info);
    let mut out = [0u8; 24];
    let mut p = 0;
    for (i, slot) in out.iter_mut().enumerate() {
        if let Some(k) = TACT_POS.iter().position(|&x| x == i) {
            *slot = ((w >> (6 - k)) & 1) as u8;
        } else {
            *slot = payload[p];
            p += 1;
        }
    }
    out
}

/// Decodes a CACH: (AT, TC, LCSS, ok) and the 17 payload bits.
pub fn decode_cach(bits: &[u8]) -> ((bool, u8, u8, bool), [u8; 17]) {
    let mut w = 0u128;
    let mut payload = [0u8; 17];
    let mut p = 0;
    for (i, &b) in bits.iter().take(24).enumerate() {
        if TACT_POS.contains(&i) {
            w = (w << 1) | b as u128;
        } else {
            payload[p] = b;
            p += 1;
        }
    }
    let (d, ok) = match TACT.decode(w) {
        Some((cw, _)) => (TACT.info(cw) as u8, true),
        None => (0, false),
    };
    ((d & 8 != 0, (d >> 2) & 1, d & 3, ok), payload)
}

/// Short LC (TS 102 361-2 clause 7.1.3): 36 bits (SLCO 4, data 24, CRC-8) in a 4 x 17 matrix of 68 bits, rows 0..2 Hamming
/// (17,12), row 3 the column parity, sent interleaved over four CACH fragments.
pub fn encode_short_lc(slco: u8, data: u32) -> [u8; 68] {
    let mut bits = [0u8; 36];
    for i in 0..4 {
        bits[i] = (slco >> (3 - i)) & 1;
    }
    for i in 0..24 {
        bits[4 + i] = ((data >> (23 - i)) & 1) as u8;
    }
    let crc = CRC8_SMBUS.bits(&bits[..28]);
    for i in 0..8 {
        bits[28 + i] = (crc >> (7 - i)) & 1;
    }
    let mut m = [0u8; 68];
    for r in 0..3 {
        let w = SHORT_LC_ROW.encode(to_u64(&bits[r * 12..r * 12 + 12]) as u128);
        for c in 0..17 {
            m[r * 17 + c] = (w >> (16 - c)) as u8 & 1;
        }
    }
    for c in 0..17 {
        m[51 + c] = m[c] ^ m[17 + c] ^ m[34 + c];
    }
    let mut out = [0u8; 68];
    SHORT_LC_ORDER
        .apply(&m[..67], &mut out[..67])
        .expect("67 bits");
    out[67] = m[67];
    out
}

/// Decodes the 68 short LC bits: each row is corrected by its Hamming (17,12) code, then the column parity and the CRC-8 must
/// check. Returns the SLCO, the 24 data bits and the bits corrected.
pub fn decode_short_lc(raw: &[u8; 68]) -> Option<(u8, u32, u32)> {
    let mut m = [0u8; 68];
    SHORT_LC_ORDER.invert(&raw[..67], &mut m[..67]).ok()?;
    m[67] = raw[67];
    let mut fixed = 0u32;
    for r in 0..3 {
        let (cw, c) = SHORT_LC_ROW.decode(to_u64(&m[r * 17..r * 17 + 17]) as u128)?;
        fixed += c as u32;
        for k in 0..17 {
            m[r * 17 + k] = (cw >> (16 - k)) as u8 & 1;
        }
    }
    if (0..17).any(|c| m[c] ^ m[17 + c] ^ m[34 + c] ^ m[51 + c] != 0) {
        return None;
    }
    let mut bits = [0u8; 36];
    for r in 0..3 {
        bits[r * 12..r * 12 + 12].copy_from_slice(&m[r * 17..r * 17 + 12]);
    }
    if !CRC8_SMBUS.verify_bits(&bits) {
        return None;
    }
    Some((to_u64(&bits[..4]) as u8, to_u64(&bits[4..28]) as u32, fixed))
}

/// Defined short data formats (TS 102 361-3 9.3.?): the names DMR text messages report.
pub fn dd_format_name(f: u8) -> &'static str {
    match f {
        0 => "binary",
        1 => "BCD",
        2 => "7-bit",
        3 => "ISO 8859-1",
        4 => "ISO 8859-2",
        5..=15 => "ISO 8859",
        16 => "UTF-8",
        17 => "UTF-16",
        18 => "UTF-16BE",
        19 => "UTF-16LE",
        20 => "UTF-32",
        _ => "other",
    }
}

/// Text of short data in a defined format.
pub fn decode_text(format: u8, data: &[u8]) -> String {
    let wide = matches!(format, 17..=19);
    let data: Vec<u8> = if wide {
        // Up to the first zero code unit.
        let n = data
            .chunks(2)
            .take_while(|c| c.iter().any(|&b| b != 0))
            .count()
            * 2;
        data[..n.min(data.len())].to_vec()
    } else {
        data.iter().copied().take_while(|&b| b != 0).collect()
    };
    match format {
        2 => {
            let bits = bytes_to_bits(&data);
            bits.as_chunks::<7>()
                .0
                .iter()
                .map(|c| to_u64(c) as u8 as char)
                .filter(|c| *c != '\0')
                .collect()
        }
        16 => String::from_utf8_lossy(&data).into_owned(),
        17 | 18 => {
            let u: Vec<u16> = data
                .as_chunks::<2>()
                .0
                .iter()
                .map(|c| u16::from_be_bytes([c[0], c[1]]))
                .collect();
            String::from_utf16_lossy(&u)
        }
        19 => {
            let u: Vec<u16> = data
                .as_chunks::<2>()
                .0
                .iter()
                .map(|c| u16::from_le_bytes([c[0], c[1]]))
                .collect();
            String::from_utf16_lossy(&u)
        }
        _ => data.iter().map(|&b| b as char).collect(),
    }
}

#[derive(Clone, Debug, Default)]
struct Alias {
    parts: [Option<Vec<u8>>; 4],
    format: u8,
    len: u8,
}

impl Alias {
    fn text(&self) -> Option<String> {
        let hdr = self.parts[0].as_ref()?;
        let mut bits = Vec::new();
        bits.extend_from_slice(&hdr[..]);
        let needed = match self.format {
            0 => self.len as usize * 7,
            3 => self.len as usize * 16,
            _ => self.len as usize * 8,
        };
        let mut i = 1;
        while bits.len() < needed + if self.format == 0 { 0 } else { 1 } {
            bits.extend_from_slice(self.parts.get(i).and_then(|p| p.as_ref())?);
            i += 1;
        }
        let s: String = match self.format {
            0 => bits
                .as_chunks::<7>()
                .0
                .iter()
                .take(self.len as usize)
                .map(|c| to_u64(c) as u8 as char)
                .collect(),
            1 => bits[1..]
                .as_chunks::<8>()
                .0
                .iter()
                .take(self.len as usize)
                .map(|c| to_u64(c) as u8 as char)
                .collect(),
            2 => {
                let b: Vec<u8> = bits[1..]
                    .as_chunks::<8>()
                    .0
                    .iter()
                    .take(self.len as usize)
                    .map(|c| to_u64(c) as u8)
                    .collect();
                String::from_utf8_lossy(&b).into_owned()
            }
            _ => {
                let u: Vec<u16> = bits[1..]
                    .as_chunks::<16>()
                    .0
                    .iter()
                    .take(self.len as usize)
                    .map(|c| to_u64(c) as u16)
                    .collect();
                String::from_utf16_lossy(&u)
            }
        };
        Some(s.trim_end_matches('\0').trim_end().to_string())
    }
}

#[derive(Clone, Debug, Default)]
struct DataAsm {
    dpf: u8,
    group: bool,
    dst: u32,
    src: u32,
    blocks_left: u8,
    pad_octets: u8,
    dd_format: Option<u8>,
    data: Vec<u8>,
    rate: u8,
    started_s: f64,
    confirmed: bool,
}

#[derive(Clone, Debug, Default)]
struct SlotState {
    /// Position in the voice superframe of the last voice burst (0 = A), when one is running.
    voice_pos: Option<u8>,
    emb: Vec<u8>,
    emb_ok: bool,
    alias: Alias,
    /// The talker alias as far as it is decoded.
    alias_text: String,
    lc: Option<Lc>,
    last_s: f64,
    data: Option<DataAsm>,
    last_control: Option<(u8, u32, u32, f64)>,
    pending_frames: u32,
    enc: Option<Encryption>,
    seen: bool,
    last_end: Option<(u64, Lc, f64)>,
}

/// The DMR protocol.
pub struct Dmr {
    slots: [SlotState; 2],
    buf: Vec<Sym>,
    has_cach: bool,
    next_slot: u8,
    bad_cycles: u32,
    cc: Option<u8>,
    link: Option<Link>,
    activity: Option<[u8; 2]>,
    activity_last: Option<[u8; 2]>,
    slc: [u8; 68],
    slc_pos: Option<usize>,
    slots_locked: [bool; 2],
    last_ok_cycle: [u32; 2],
    cycle: u32,
    /// The cycle being read is the silent slot of a direct-mode voice superframe.
    hold: bool,
    codecs: Codecs,
}

impl Default for Dmr {
    fn default() -> Self {
        Dmr {
            slots: Default::default(),
            buf: Vec::with_capacity(160),
            has_cach: true,
            next_slot: 0,
            bad_cycles: 0,
            cc: None,
            link: None,
            activity: None,
            activity_last: None,
            slc: [0; 68],
            slc_pos: None,
            slots_locked: [false; 2],
            last_ok_cycle: [0; 2],
            cycle: 0,
            hold: false,
            codecs: Codecs::default(),
        }
    }
}

fn sym_bits(s: &[Sym], out: &mut Vec<u8>) {
    for x in s {
        let d = x.dibit();
        out.push(d >> 1);
        out.push(d & 1);
    }
}

impl Dmr {
    fn key(slot: u8) -> u32 {
        slot as u32 + 1
    }

    fn process_burst(&mut self, ctx: &mut Ctx) {
        self.cycle += 1;
        if crate::symbols::carrier_gone(&self.buf[12..]) {
            // In direct mode a radio sends in its own slot only: the cycle of the other slot is empty while the voice
            // superframe of the last cycle runs, and the timing holds for it.
            let voice = self.next_slot ^ 1;
            let running = self.slots[voice as usize].voice_pos.is_some_and(|p| p < 5)
                && self.last_ok_cycle[voice as usize] + 1 == self.cycle;
            if !self.has_cach && running {
                self.next_slot = voice;
                return;
            }
            self.bad_cycles = 8;
            return;
        }
        let mut bits = Vec::with_capacity(288);
        sym_bits(&self.buf, &mut bits);
        let cach = &bits[..24];
        let burst = &bits[24..288];
        // Which sync, if any, is in the centre.
        let centre = &burst[108..156];
        let mut sync: Option<usize> = None;
        for i in 0..SYNCS.len() {
            let s = sync_bits(i);
            let errs = (0..24)
                .filter(|&k| centre[2 * k] != s[2 * k] || centre[2 * k + 1] != s[2 * k + 1])
                .count();
            if errs <= 4 {
                sync = Some(i);
                break;
            }
        }
        // Slot of this burst.
        let mut slot = self.next_slot;
        // The CACH is read only with a burst that carries a sync or continues a voice superframe: a random word passes its
        // Hamming check one time in eight, so alone it proves nothing.
        let continues = sync.is_none() && {
            let guess = if self.has_cach {
                decode_cach(cach).0 .1
            } else {
                self.next_slot
            };
            self.slots[guess as usize].voice_pos.is_some_and(|p| p < 5)
        };
        if self.has_cach && (sync.is_some() || continues) {
            let ((_at, tc, lcss, ok), payload) = decode_cach(cach);
            ctx.block(ok, 7, 0);
            if ok {
                slot = tc;
                self.short_lc(lcss, &payload, ctx);
            }
        }
        let cach_ok = false;
        match sync {
            Some(5) | Some(6) => slot = 0,
            Some(7) | Some(8) => slot = 1,
            _ => {}
        }
        if let Some(s) = sync {
            self.link = Some(match s {
                0 | 1 => Link::BaseStation,
                2..=4 => Link::Mobile,
                _ => Link::DirectMode,
            });
        }
        self.next_slot = slot ^ 1;
        let mut useful = cach_ok;
        match sync {
            Some(0) | Some(2) | Some(5) | Some(7) => {
                self.voice_burst(slot, 0, burst, ctx);
                useful = true;
            }
            Some(1) | Some(3) | Some(6) | Some(8) => {
                // The soft levels of the 98 information dibits, for the rate 3/4 trellis code.
                let mut levels = [0f32; 98];
                for (k, l) in levels.iter_mut().enumerate() {
                    let at = if k < 49 { 12 + k } else { 12 + 83 + (k - 49) };
                    *l = self.buf[at].v;
                }
                useful |= self.data_burst(slot, burst, &levels, ctx);
            }
            _ => {
                if let Some(p) = self.slots[slot as usize].voice_pos {
                    if p < 5 {
                        self.voice_burst(slot, p + 1, burst, ctx);
                        useful = true;
                    }
                }
            }
        }
        if useful {
            self.bad_cycles = 0;
        } else {
            self.bad_cycles += 1;
        }
        for s in 0..2 {
            self.slots_locked[s] =
                self.cycle.saturating_sub(self.last_ok_cycle[s]) <= 67 && self.slots[s].seen;
        }
    }

    fn short_lc(&mut self, lcss: u8, payload: &[u8; 17], ctx: &mut Ctx) {
        match lcss {
            1 => {
                self.slc[..17].copy_from_slice(payload);
                self.slc_pos = Some(17);
            }
            3 => {
                if let Some(p) = self.slc_pos {
                    if p + 17 <= 51 {
                        self.slc[p..p + 17].copy_from_slice(payload);
                        self.slc_pos = Some(p + 17);
                    } else {
                        self.slc_pos = None;
                    }
                }
            }
            2 => {
                if self.slc_pos == Some(51) {
                    self.slc[51..].copy_from_slice(payload);
                    let ok = decode_short_lc(&self.slc);
                    ctx.block(ok.is_some(), 68, ok.map_or(0, |(_, _, f)| f as usize));
                    if let Some((slco, data, _)) = ok {
                        if slco == 1 {
                            let a = [((data >> 20) & 0xF) as u8, ((data >> 16) & 0xF) as u8];
                            self.activity = Some(a);
                            if a != [0, 0] {
                                self.activity_last = Some(a);
                            }
                        }
                    }
                }
                self.slc_pos = None;
            }
            _ => {}
        }
    }

    fn slot_type(&mut self, burst: &[u8], ctx: &mut Ctx) -> Option<(u8, u8)> {
        let mut w = Vec::with_capacity(20);
        w.extend_from_slice(&burst[98..108]);
        w.extend_from_slice(&burst[156..166]);
        let got = SLOT_TYPE.decode(to_u64(&w) as u128);
        ctx.identity(got.is_some());
        ctx.block(got.is_some(), 20, got.map_or(0, |(_, e)| e as usize));
        let (cw, _) = got?;
        let d = SLOT_TYPE.info(cw) as u8;
        Some((d >> 4, d & 0xF))
    }

    fn info196(burst: &[u8]) -> Vec<u8> {
        let mut v = Vec::with_capacity(196);
        v.extend_from_slice(&burst[..98]);
        v.extend_from_slice(&burst[166..264]);
        v
    }

    fn data_burst(&mut self, slot: u8, burst: &[u8], levels: &[f32], ctx: &mut Ctx) -> bool {
        let Some((cc, dtype)) = self.slot_type(burst, ctx) else {
            return false;
        };
        self.cc = Some(cc);
        let s = slot as usize;
        self.slots[s].seen = true;
        self.last_ok_cycle[s] = self.cycle;
        let info = Self::info196(burst);
        let key = Self::key(slot);
        let access = Some(Access::new(AccessKind::ColourCode, u32::from(cc)));
        match dtype {
            dt::VOICE_LC_HEADER | dt::TERMINATOR => {
                let (b, fixed, good) = decode_bptc(&info);
                let bytes = bits_to_bytes(&b);
                let lc = decode_full_lc(&mut self.codecs.rs, &bytes, dtype);
                ctx.block(lc.is_some() && good, 96, fixed);
                let Some((lcb, _)) = lc else {
                    return true;
                };
                let lc = Lc::from_bytes(&lcb);
                if dtype == dt::VOICE_LC_HEADER {
                    self.start_voice(slot, lc, false, access, ctx);
                    self.slots[s].voice_pos = None;
                } else {
                    // A repeated terminator of the call that just ended moves its end.
                    let now = ctx.now;
                    if let Some((id, l, t)) = self.slots[s].last_end {
                        if !ctx.has_call(key) && l.src == lc.src && l.dst == lc.dst && now - t < 1.0
                        {
                            ctx.extend_call(id);
                            self.slots[s].last_end = Some((id, l, now));
                            return true;
                        }
                    }
                    if !ctx.has_call(key) {
                        self.start_voice(slot, lc, true, access, ctx);
                    }
                    self.slots[s].last_end = ctx.call_id(key).map(|id| (id, lc, now));
                    let alias = self.slots[s].alias.text();
                    ctx.update_call(key, |c| {
                        if let Some(a) = alias {
                            c.alias = a;
                        }
                    });
                    ctx.end_call(key, true);
                    self.slots[s].voice_pos = None;
                    self.slots[s].alias = Alias::default();
                    self.slots[s].alias_text.clear();
                    self.slots[s].lc = None;
                    self.slots[s].enc = None;
                }
            }
            dt::PI_HEADER => {
                let (b, fixed, good) = decode_bptc(&info);
                let bytes = bits_to_bytes(&b);
                let ok = good && check_crc16(&bytes, dtype);
                ctx.block(ok, 96, fixed);
                if ok {
                    let alg = bytes[0] & 0x07;
                    let key_id = bytes[2];
                    let name = match alg {
                        1 => "RC4",
                        2 => "DES",
                        4 => "AES-128",
                        5 => "AES-256",
                        _ => "privacy",
                    };
                    self.slots[s].enc = Some(Encryption::encrypted(
                        Some(alg as u32),
                        name,
                        Some(key_id as u32),
                    ));
                }
            }
            dt::CSBK | dt::MBC_HEADER => {
                let (b, fixed, good) = decode_bptc(&info);
                let bytes = bits_to_bytes(&b);
                let ok = good && check_crc16(&bytes, dtype);
                ctx.block(ok, 96, fixed);
                if ok {
                    self.csbk(slot, &bytes, access, ctx);
                }
            }
            dt::DATA_HEADER => {
                let (b, fixed, good) = decode_bptc(&info);
                let bytes = bits_to_bytes(&b);
                let ok = good && check_crc16(&bytes, dtype);
                ctx.block(ok, 96, fixed);
                if ok {
                    self.data_header(slot, &bytes, ctx);
                }
            }
            dt::RATE_12 | dt::RATE_34 | dt::RATE_1 => {
                let (block, ok, fixed): (Vec<u8>, bool, usize) = match dtype {
                    dt::RATE_12 => {
                        let (b, fixed, good) = decode_bptc(&info);
                        (bits_to_bytes(&b), good, fixed)
                    }
                    dt::RATE_34 => {
                        let mut b = [0u8; 144];
                        match self.codecs.tcm.decode_levels(levels, &mut b) {
                            Ok(d) => (bits_to_bytes(&b), true, d.corrected as usize),
                            Err(_) => (bits_to_bytes(&b), false, 0),
                        }
                    }
                    _ => {
                        // Rate 1: 96 bits, 4 unused bits around the centre, 96 bits.
                        let mut b = info[..96].to_vec();
                        b.extend_from_slice(&info[100..196]);
                        (bits_to_bytes(&b), true, 0)
                    }
                };
                ctx.block(ok, block.len() * 8, fixed);
                self.data_block(slot, &block, dtype, ctx);
            }
            dt::IDLE => {
                let (_, fixed, good) = decode_bptc(&info);
                ctx.block(good, 196, fixed);
            }
            _ => {}
        }
        true
    }

    fn start_voice(&mut self, slot: u8, lc: Lc, late: bool, access: Option<Access>, ctx: &mut Ctx) {
        let key = Self::key(slot);
        let s = slot as usize;
        if let Some(c) = ctx.call(key) {
            if c.src == Ident::Number(lc.src as u64) && c.dst == Ident::Number(lc.dst as u64) {
                return;
            }
        }
        let enc = if lc.privacy() {
            self.slots[s]
                .enc
                .clone()
                .unwrap_or_else(|| Encryption::encrypted(None, "privacy", None))
        } else {
            Encryption::clear()
        };
        let mut rec = CallRecord {
            slot: slot + 1,
            kind: lc.kind(),
            src: Ident::Number(lc.src as u64),
            dst: Ident::Number(lc.dst as u64),
            group: lc.flco == 0,
            emergency: lc.emergency(),
            encryption: enc.clone(),
            access,
            ..Default::default()
        };
        rec.set(CallDetail::Fid, Value::I64(i64::from(lc.fid)));
        rec.set(CallDetail::ServiceOptions, Value::I64(i64::from(lc.opts)));
        rec.voice_frames = std::mem::take(&mut self.slots[s].pending_frames);
        ctx.start_call(key, rec, late);
        ctx.set_encryption(key, enc);
        self.slots[s].lc = Some(lc);
    }

    fn voice_burst(&mut self, slot: u8, pos: u8, burst: &[u8], ctx: &mut Ctx) {
        let s = slot as usize;
        let key = Self::key(slot);
        self.slots[s].voice_pos = Some(pos);
        self.slots[s].seen = true;
        self.last_ok_cycle[s] = self.cycle;
        self.slots[s].last_s = ctx.now;
        // Embedded signalling in bursts B..F.
        if pos > 0 {
            let mut emb = Vec::with_capacity(16);
            emb.extend_from_slice(&burst[108..116]);
            emb.extend_from_slice(&burst[148..156]);
            let got = EMB.decode(to_u64(&emb) as u128);
            ctx.identity(got.is_some());
            ctx.block(got.is_some(), 16, got.map_or(0, |(_, e)| e as usize));
            if let Some((cw, _)) = got {
                let d = EMB.info(cw) as u8;
                let cc = d >> 3;
                self.cc = Some(cc);
                let lcss = d & 3;
                let frag = &burst[116..148];
                let st = &mut self.slots[s];
                match lcss {
                    1 => {
                        st.emb.clear();
                        st.emb.extend_from_slice(frag);
                        st.emb_ok = true;
                    }
                    3 => {
                        if st.emb_ok && st.emb.len() < 96 {
                            st.emb.extend_from_slice(frag);
                        } else {
                            st.emb_ok = false;
                        }
                    }
                    2 => {
                        if st.emb_ok && st.emb.len() == 96 {
                            st.emb.extend_from_slice(frag);
                            let (lc, cs, good) = decode_embedded(&st.emb);
                            let lcb: [u8; 9] = bits_to_bytes(&lc).try_into().unwrap_or([0; 9]);
                            let ok = good && embedded_lc_checksum(&lcb) == cs;
                            ctx.block(ok, 77, 0);
                            if ok {
                                self.embedded_lc(slot, &lcb, ctx);
                            }
                        }
                        self.slots[s].emb_ok = false;
                    }
                    _ => {}
                }
            }
        }
        // Three AMBE+2 frames.
        let mut frames = [[0u8; 72]; 3];
        frames[0].copy_from_slice(&burst[0..72]);
        frames[1][..36].copy_from_slice(&burst[72..108]);
        frames[1][36..].copy_from_slice(&burst[156..192]);
        frames[2].copy_from_slice(&burst[192..264]);
        if ctx.has_call(key) {
            ctx.update_call(key, |c| c.voice_frames += 3);
            for (i, f) in frames.iter().enumerate() {
                let cw = crate::voice::amb2_codewords(f);
                let t = ctx.now + i as f64 * 0.02;
                let spec = crate::voice::FrameSpec {
                    codec: Codec::Ambe2,
                    packaging: sdrkit_graph::SpeechPackaging::Codewords,
                    codec_mode: None,
                    duration_ms: 20,
                    stack: "dmr",
                    carrier_hz: ctx.carrier_hz,
                    slot: slot + 1,
                };
                ctx.speech(key, spec, &cw, t, false, None);
            }
        } else {
            self.slots[s].pending_frames += 3;
        }
    }

    fn embedded_lc(&mut self, slot: u8, lcb: &[u8; 9], ctx: &mut Ctx) {
        let s = slot as usize;
        let key = Self::key(slot);
        let lc = Lc::from_bytes(lcb);
        match lc.flco {
            0 | 3 => {
                if !ctx.has_call(key) {
                    let access = self
                        .cc
                        .map(|c| Access::new(AccessKind::ColourCode, u32::from(c)));
                    self.start_voice(slot, lc, true, access, ctx);
                }
            }
            4..=7 => {
                let idx = (lc.flco - 4) as usize;
                let mut bits = bytes_to_bits(&lcb[2..9]);
                if idx == 0 {
                    self.slots[s].alias.format = lcb[2] >> 6;
                    self.slots[s].alias.len = (lcb[2] >> 1) & 0x1F;
                    bits = bits[7..].to_vec();
                }
                self.slots[s].alias.parts[idx] = Some(bits);
                if let Some(a) = self.slots[s].alias.text() {
                    self.slots[s].alias_text.clone_from(&a);
                    if !a.is_empty() {
                        ctx.update_call(key, |c| c.alias = a);
                    }
                }
            }
            8 => {
                // GPS info: position error 3 bits, longitude 25, latitude 24.
                let v = u64::from_be_bytes([0, 0, 0, lcb[2], lcb[3], lcb[4], lcb[5], lcb[6]]) << 16
                    | u16::from_be_bytes([lcb[7], lcb[8]]) as u64;
                let v = v & ((1u64 << 56) - 1);
                let lon_raw = ((v >> 24) & 0x1FF_FFFF) as i64;
                let lat_raw = (v & 0xFF_FFFF) as i64;
                let lon = sign_extend(lon_raw, 25) as f64 * 360.0 / (1u64 << 25) as f64;
                let lat = sign_extend(lat_raw, 24) as f64 * 180.0 / (1u64 << 24) as f64;
                let src = ctx.call(key).map(|c| c.src).unwrap_or_default();
                ctx.position(PositionRecord {
                    src,
                    lat,
                    lon,
                    source: PositionFormat::DmrGps,
                    time: ctx.now,
                    ..Default::default()
                });
            }
            _ => {}
        }
    }

    fn csbk(&mut self, slot: u8, b: &[u8], access: Option<Access>, ctx: &mut Ctx) {
        let opcode = b[0] & 0x3F;
        let fid = b[1];
        let (dst, src) = (
            u32::from_be_bytes([0, b[4], b[5], b[6]]),
            u32::from_be_bytes([0, b[7], b[8], b[9]]),
        );
        let s = slot as usize;
        let name = match opcode {
            0x3D => "preamble",
            0x04 => "unit to unit voice request",
            0x05 => "unit to unit answer response",
            0x07 => "channel timing",
            0x1F => "call alert",
            0x20 => "acknowledge response",
            0x24 => "unit to unit voice service request",
            0x26 => "negative acknowledge",
            0x28 => "aloha",
            0x2E => "clear",
            0x30 => "private voice grant",
            0x31 => "talkgroup voice grant",
            0x38 => "Tier III broadcast",
            0x3B => "announcement",
            _ => "CSBK",
        };
        let now = ctx.now;
        let dup = self.slots[s]
            .last_control
            .is_some_and(|(o, d, sr, t)| o == opcode && d == dst && sr == src && now - t < 1.0);
        self.slots[s].last_control = Some((opcode, dst, src, now));
        let text = format!("TS{} CSBK {opcode:02X} {name}: {src} -> {dst}", slot + 1);
        if dup {
            return;
        }
        ctx.control(ControlRecord {
            opcode: opcode as u32,
            mfid: fid as u32,
            name: name.into(),
            text,
            time: now,
            slot: slot + 1,
        });
        let mut rec = CallRecord {
            slot: slot + 1,
            kind: CallKind::Control,
            src: Ident::Number(src as u64),
            dst: Ident::Number(dst as u64),
            group: opcode == 0x3D && b[2] & 0x40 != 0,
            access,
            text: format!("{name} CSBK (opcode {opcode:02X})"),
            ..Default::default()
        };
        rec.set(
            CallDetail::Note,
            Value::Text(format!("{name} CSBK (opcode {opcode:02X})")),
        );
        rec.set(CallDetail::Opcode, Value::I64(i64::from(opcode)));
        ctx.log_single(rec);
    }

    fn data_header(&mut self, slot: u8, b: &[u8], _ctx: &mut Ctx) {
        let dpf = b[0] & 0x0F;
        let group = b[0] & 0x80 != 0;
        let dst = u32::from_be_bytes([0, b[2], b[3], b[4]]);
        let src = u32::from_be_bytes([0, b[5], b[6], b[7]]);
        let mut d = DataAsm {
            dpf,
            group,
            dst,
            src,
            started_s: _ctx.now,
            ..Default::default()
        };
        match dpf {
            // Unconfirmed and confirmed packet data: blocks to follow in byte 8, pad octets in byte 1.
            2 | 3 => {
                d.blocks_left = b[8] & 0x7F;
                d.pad_octets = (b[0] & 0x10) | (b[1] & 0x0F);
                d.confirmed = dpf == 3;
            }
            // Defined short data: appended blocks in byte 0 bits 5-4 and byte 1 bits 7-4. Format in byte 1 bits 3-0 + byte 8.
            // Short data: appended blocks in byte 0 bits 5-4 and byte 1 bits 3-0 (SAP in bits 7-4). Defined format in byte 8,
            // padding bits in byte 9.
            13 => {
                d.blocks_left = (b[0] & 0x30) | (b[1] & 0x0F);
                d.dd_format = Some(b[8] >> 2);
                d.pad_octets = b[9] / 8;
            }
            14 => {
                d.blocks_left = (b[0] & 0x30) | (b[1] & 0x0F);
                d.pad_octets = b[9] / 8;
            }
            _ => {
                d.blocks_left = 0;
            }
        }
        self.slots[slot as usize].data = (d.blocks_left > 0).then_some(d);
    }

    fn data_block(&mut self, slot: u8, block: &[u8], dtype: u8, ctx: &mut Ctx) {
        let s = slot as usize;
        let Some(d) = self.slots[s].data.as_mut() else {
            return;
        };
        d.rate = dtype;
        if d.confirmed {
            // Serial number and CRC-9 in front of each block.
            d.data.extend_from_slice(&block[2..]);
        } else {
            d.data.extend_from_slice(block);
        }
        d.blocks_left = d.blocks_left.saturating_sub(1);
        if d.blocks_left > 0 {
            return;
        }
        let Some(d) = self.slots[s].data.take() else {
            return;
        };
        let n = d.data.len();
        if n < 4 {
            return;
        }
        let body = &d.data[..n - 4];
        let crc_ok = packet_crc_ok(&d.data);
        let payload_len = body.len().saturating_sub(d.pad_octets as usize);
        let payload = &body[..payload_len.min(body.len())];
        let (text, format) = match d.dd_format {
            Some(f) => (decode_text(f, payload), MessageFormat::ShortData),
            None => (decode_payload_text(payload), MessageFormat::PacketData),
        };
        let mut rec = CallRecord {
            slot: slot + 1,
            kind: CallKind::Data,
            src: Ident::Number(d.src as u64),
            dst: Ident::Number(d.dst as u64),
            group: d.group,
            access: self
                .cc
                .map(|c| Access::new(AccessKind::ColourCode, u32::from(c))),
            text: text.clone(),
            ..Default::default()
        };
        if let Some(f) = d.dd_format {
            rec.set(CallDetail::Format, Value::I64(i64::from(f)));
        }
        rec.set(CallDetail::CrcOk, Value::Bool(crc_ok));
        rec.set(CallDetail::Dpf, Value::I64(i64::from(d.dpf)));
        let now = ctx.now;
        ctx.now = d.started_s;
        ctx.log_single(rec);
        ctx.now = now;
        ctx.message(MessageRecord {
            src: Ident::Number(d.src as u64),
            dst: Ident::Number(d.dst as u64),
            group: d.group,
            format,
            data_format: d.dd_format,
            text,
            crc_ok,
            time: d.started_s,
            slot: slot + 1,
            sms: false,
        });
    }
}

/// The text of packet data: an IP/UDP payload's text part, or printable bytes.
fn decode_payload_text(p: &[u8]) -> String {
    // IPv4 + UDP: skip 28 bytes of headers when they look like one.
    let body = if p.len() > 28 && p[0] >> 4 == 4 {
        &p[28..]
    } else {
        p
    };
    let utf16 = body.len() >= 4 && body.iter().skip(1).step_by(2).take(4).all(|&b| b == 0);
    if utf16 {
        let u: Vec<u16> = body
            .as_chunks::<2>()
            .0
            .iter()
            .map(|c| u16::from_le_bytes([c[0], c[1]]))
            .collect();
        return String::from_utf16_lossy(&u)
            .trim_end_matches('\0')
            .to_string();
    }
    body.iter()
        .filter(|&&b| b != 0)
        .map(|&b| b as char)
        .collect()
}

fn sign_extend(v: i64, bits: u32) -> i64 {
    let s = 64 - bits;
    (v << s) >> s
}

impl Protocol for Dmr {
    fn params(&self) -> ModeParams {
        ModeParams {
            mode: "dmr",
            name: "DMR",
            baud: 4800.0,
            two_level: false,
            channel_hz: 12_500.0,
            calls: &crate::results::CALLS_100,
            message_cap: 16,
            polarity_rule: true,
            clock_loop: true,
            extra_baud: None,
            stream_rate: crate::front::RATE,
        }
    }

    fn patterns(&self) -> Vec<SyncPattern> {
        SYNCS
            .iter()
            .enumerate()
            .map(|(i, (_, hex))| {
                SyncPattern::from_hex_dibits(i as u8, hex, 66)
                    .with_max_err(4)
                    .no_auto_invert()
            })
            .collect()
    }

    fn on_sync(
        &mut self,
        id: u8,
        _inverted: bool,
        pre: &[Sym],
        _at: u64,
        tracking: bool,
        _ctx: &mut Ctx,
    ) -> SyncAnswer {
        // While tracking, a sync where a burst centre is expected only confirms the timing.
        let expected_len = 12 + 54 + 24;
        if tracking && self.buf.len() >= expected_len - 1 && self.buf.len() <= expected_len + 1 {
            self.buf.clear();
            self.buf.extend_from_slice(pre);
            return SyncAnswer::Take;
        }
        self.has_cach = id <= 1;
        self.buf.clear();
        self.buf.extend_from_slice(pre);
        self.bad_cycles = 0;
        SyncAnswer::Take
    }

    fn on_symbol(&mut self, s: Sym, _at: u64, ctx: &mut Ctx) -> SymAnswer {
        self.buf.push(s);
        let hold = self.hold;
        if self.buf.len() == 144 {
            self.process_burst(ctx);
            self.buf.clear();
            if self.bad_cycles >= 8 {
                for st in self.slots.iter_mut() {
                    st.voice_pos = None;
                }
                self.hold = false;
                return SymAnswer::Stop;
            }
            // In direct mode the cycle after a voice burst is the other slot, silent or noise while the superframe runs.
            let s = (self.next_slot ^ 1) as usize;
            self.hold = !self.has_cach
                && self.slots[s].voice_pos.is_some_and(|p| p < 5)
                && self.last_ok_cycle[s] == self.cycle;
        }
        if hold {
            SymAnswer::Hold
        } else {
            SymAnswer::Continue
        }
    }

    fn on_tick(&mut self, ctx: &mut Ctx) {
        let now = ctx.now;
        for st in self.slots.iter_mut() {
            if now - st.last_s > 0.5 {
                st.voice_pos = None;
            }
            if st.data.as_ref().is_some_and(|d| now - d.started_s > 3.0) {
                st.data = None;
            }
        }
    }

    fn on_reset(&mut self, _ctx: &mut Ctx) {
        let keep_cc = self.cc;
        *self = Dmr::default();
        self.cc = keep_cc;
    }

    fn fields(&self) -> Vec<&'static dyn Declare> {
        <DmrFields as Fields>::SHAPE.fields.to_vec()
    }

    fn publish(&self, r: &mut Published) {
        r.put(&DmrFields {
            colour_code: self.cc.map(i64::from),
            link: self.link,
            slots_locked: self.slots_locked.iter().filter(|x| **x).count() as i64,
            cach_activity_1: self.activity.map(|a| Activity::of(a[0])),
            cach_activity_2: self.activity.map(|a| Activity::of(a[1])),
            cach_activity_last_1: self.activity_last.map(|a| Activity::of(a[0])),
            cach_activity_last_2: self.activity_last.map(|a| Activity::of(a[1])),
            talker_alias_1: &self.slots[0].alias_text,
            talker_alias_2: &self.slots[1].alias_text,
        });
    }

    fn codec(&self) -> Codec {
        Codec::Ambe2
    }
}

// ---------------------------------------------------------------------------------------------------------------------------
// Generator.

/// A transmission in the DMR test schedule.
#[derive(Clone, Debug)]
pub enum DmrTx {
    Voice {
        slot: u8,
        src: u32,
        dst: u32,
        group: bool,
        superframes: u32,
        alias: String,
        emergency: bool,
        privacy: bool,
    },
    Text {
        slot: u8,
        src: u32,
        dst: u32,
        text: String,
    },
}

/// A short data message of defined format (TS 102 361-3): from `src` to `dst` (a talkgroup when `group`), the octets
/// `data` in the defined data format `format` (3 for ISO 8859-1, 19 for UTF-16LE).
#[derive(Clone, Copy, Debug)]
pub struct ShortData<'a> {
    pub src: u32,
    pub dst: u32,
    pub group: bool,
    pub format: u8,
    pub data: &'a [u8],
}

/// The most rate 1/2 blocks of one message: the appended blocks field has 6 bits.
pub const SHORT_DATA_BLOCKS_MAX: usize = 63;

/// The bursts of a short data message, in order, with colour code `cc` and the data sync `sync` (an index of [`SYNCS`]):
/// two preamble CSBKs (opcode 0x3D, last block, the blocks that follow each), the data header (defined short data, SAP
/// 10, the format and the pad bits) and the rate 1/2 blocks of 12 octets. The octets are padded with zeros so that with
/// the CRC-32, least significant octet first, they fill whole blocks. At most [`SHORT_DATA_BLOCKS_MAX`] blocks: longer
/// octets are cut.
pub fn text_bursts(m: &ShortData, cc: u8, sync: usize) -> Vec<[u8; 264]> {
    let mut data = m.data.to_vec();
    data.truncate(SHORT_DATA_BLOCKS_MAX * 12 - 4);
    let total = (data.len() + 4).div_ceil(12) * 12;
    let pad = total - 4 - data.len();
    data.resize(total - 4, 0);
    let crc = crc32_dmr_packet(&data);
    data.extend_from_slice(&crc.to_le_bytes());
    let blocks = (total / 12) as u8;
    let mut out = Vec::with_capacity(blocks as usize + 3);
    let group = if m.group { 0x80 } else { 0 };
    for left in (0..2u8).rev() {
        let mut c = [0u8; 10];
        c[0] = 0x80 | 0x3D;
        c[2] = 0x80 | (group >> 1);
        c[3] = left + 1 + blocks;
        c[4..7].copy_from_slice(&m.dst.to_be_bytes()[1..]);
        c[7..10].copy_from_slice(&m.src.to_be_bytes()[1..]);
        let w = with_crc16(&c, dt::CSBK);
        out.push(burst_bits(&encode_bptc(&bits_of::<96>(&w)), cc, dt::CSBK, sync));
    }
    let mut h = [0u8; 10];
    h[0] = group | (blocks & 0x30) | 0x0D;
    h[1] = 0xA0 | (blocks & 0x0F);
    h[2..5].copy_from_slice(&m.dst.to_be_bytes()[1..]);
    h[5..8].copy_from_slice(&m.src.to_be_bytes()[1..]);
    h[8] = (m.format << 2) | 1;
    h[9] = (pad * 8) as u8;
    let w = with_crc16(&h, dt::DATA_HEADER);
    out.push(burst_bits(&encode_bptc(&bits_of::<96>(&w)), cc, dt::DATA_HEADER, sync));
    for blk in data.chunks(12) {
        out.push(burst_bits(&encode_bptc(&bits_of::<96>(blk)), cc, dt::RATE_12, sync));
    }
    out
}

/// The DMR test signal: a base station downlink, both slots, colour code `cc`. The voice bursts carry the frames of
/// `speech` in turn (72 channel bits each, as sent), or pseudo-random bits when it is empty.
pub struct DmrGen {
    pub cc: u8,
    pub schedule: Vec<(f64, DmrTx)>,
    pub speech: Vec<[u8; 72]>,
}

fn burst_bits(info196: &[u8], cc: u8, dtype: u8, sync: usize) -> [u8; 264] {
    let mut b = [0u8; 264];
    b[..98].copy_from_slice(&info196[..98]);
    b[166..].copy_from_slice(&info196[98..196]);
    let st = SLOT_TYPE.encode(((cc as u128) << 4) | dtype as u128);
    for k in 0..20 {
        let bit = ((st >> (19 - k)) & 1) as u8;
        if k < 10 {
            b[98 + k] = bit;
        } else {
            b[156 + k - 10] = bit;
        }
    }
    b[108..156].copy_from_slice(&sync_bits(sync));
    b
}

/// The burst of the full LC `lc` as a voice LC header ([`dt::VOICE_LC_HEADER`]) or a terminator with LC
/// ([`dt::TERMINATOR`]), with colour code `cc` and the data sync `sync` (an index of [`SYNCS`]).
pub fn lc_burst(lc: &[u8; 9], dtype: u8, cc: u8, sync: usize) -> [u8; 264] {
    let w = encode_full_lc(lc, dtype);
    burst_bits(&encode_bptc(&bits_of::<96>(&w)), cc, dtype, sync)
}

/// The 128 bits of embedded signalling that carry the embedded LC `elc` in bursts B to E of a voice superframe.
pub fn embedded_bits(elc: &[u8; 9]) -> [u8; 128] {
    encode_embedded(&bits_of::<72>(elc), embedded_lc_checksum(elc))
}

/// Voice burst `pos` (0 is A, 5 is F) of a voice superframe: the three speech frames `frames` (72 channel bits each, as
/// sent) around the centre. Burst A has the voice sync `sync` (an index of [`SYNCS`]) in the centre. Bursts B to E have
/// the EMB of colour code `cc` (PI 0) with LCSS 1, 3, 3 and 2 around the four fragments of `embedded`. Burst F has the
/// EMB with LCSS 0 around 32 zero bits (no embedded message).
pub fn voice_burst(
    frames: &[[u8; 72]; 3],
    pos: u8,
    cc: u8,
    sync: usize,
    embedded: &[u8; 128],
) -> [u8; 264] {
    let centre: [u8; 48] = if pos == 0 {
        sync_bits(sync)
    } else {
        let lcss = match pos {
            1 => 1,
            2 | 3 => 3,
            4 => 2,
            _ => 0,
        };
        let emb = EMB.encode((((cc & 15) as u128) << 3) | lcss);
        let mut c = [0u8; 48];
        for k in 0..8 {
            c[k] = ((emb >> (15 - k)) & 1) as u8;
            c[40 + k] = ((emb >> (7 - k)) & 1) as u8;
        }
        if (1..=4).contains(&pos) {
            let f = (pos - 1) as usize;
            c[8..40].copy_from_slice(&embedded[f * 32..f * 32 + 32]);
        }
        c
    };
    let mut b = [0u8; 264];
    b[..72].copy_from_slice(&frames[0]);
    b[72..108].copy_from_slice(&frames[1][..36]);
    b[108..156].copy_from_slice(&centre);
    b[156..192].copy_from_slice(&frames[1][36..]);
    b[192..].copy_from_slice(&frames[2]);
    b
}

impl DmrGen {
    /// The default schedule: group, private and all calls with talker aliases, and short data text messages.
    pub fn default_schedule(cc: u8) -> DmrGen {
        DmrGen {
            cc,
            speech: Vec::new(),
            schedule: vec![
                (
                    0.0,
                    DmrTx::Voice {
                        slot: 0,
                        src: 3101234,
                        dst: 91,
                        group: true,
                        superframes: 4,
                        alias: "Ops Desk".into(),
                        emergency: false,
                        privacy: false,
                    },
                ),
                (
                    0.05,
                    DmrTx::Text {
                        slot: 1,
                        src: 2145016,
                        dst: 2145007,
                        text: "Battery low on unit 7 #1".into(),
                    },
                ),
                (
                    0.9,
                    DmrTx::Voice {
                        slot: 1,
                        src: 2504105,
                        dst: 2149,
                        group: true,
                        superframes: 4,
                        alias: "Station Two".into(),
                        emergency: false,
                        privacy: false,
                    },
                ),
                (
                    2.2,
                    DmrTx::Voice {
                        slot: 0,
                        src: 2623266,
                        dst: 250997,
                        group: false,
                        superframes: 3,
                        alias: String::new(),
                        emergency: false,
                        privacy: false,
                    },
                ),
                (
                    3.9,
                    DmrTx::Voice {
                        slot: 1,
                        src: 2621042,
                        dst: 9,
                        group: true,
                        superframes: 3,
                        alias: String::new(),
                        emergency: true,
                        privacy: false,
                    },
                ),
                (
                    4.5,
                    DmrTx::Voice {
                        slot: 0,
                        src: 7001,
                        dst: 300,
                        group: true,
                        superframes: 2,
                        alias: String::new(),
                        emergency: false,
                        privacy: true,
                    },
                ),
                (
                    6.0,
                    DmrTx::Text {
                        slot: 1,
                        src: 250997,
                        dst: 2308155,
                        text: "Status: all clear #2".into(),
                    },
                ),
            ],
        }
    }

    /// The bursts of each slot, as (start time, bits) where a burst is 264 bits.
    fn slot_bursts(&self, rng: &mut sdrkit_algo_sim::rng::Rng) -> [Vec<(f64, [u8; 264])>; 2] {
        let mut out: [Vec<(f64, [u8; 264])>; 2] = [Vec::new(), Vec::new()];
        let cc = self.cc;
        let mut next_speech = 0usize;
        for (t0, tx) in &self.schedule {
            match tx {
                DmrTx::Voice {
                    slot,
                    src,
                    dst,
                    group,
                    superframes,
                    alias,
                    emergency,
                    privacy,
                } => {
                    let opts = (*emergency as u8) << 7 | (*privacy as u8) << 6;
                    let lc = Lc {
                        pf: false,
                        flco: if *group { 0 } else { 3 },
                        fid: 0,
                        opts,
                        dst: *dst,
                        src: *src,
                    };
                    let lcb = lc.to_bytes();
                    let v = &mut out[*slot as usize];
                    let mut t = *t0;
                    for _ in 0..3 {
                        v.push((t, lc_burst(&lcb, dt::VOICE_LC_HEADER, cc, 1)));
                        t += 0.06;
                    }
                    // Embedded LCs: the voice LC, and the talker alias parts in turn.
                    let mut embedded: Vec<[u8; 9]> = vec![lcb];
                    if !alias.is_empty() {
                        let a = alias.as_bytes();
                        let len = a.len().min(31) as u8;
                        let mut bits = vec![0u8]; // the header's single data bit
                        bits.extend(bytes_to_bits(a));
                        bits.resize(49 + 3 * 56, 0);
                        let mut hdr = [0u8; 9];
                        hdr[0] = 4;
                        let mut hb = Vec::new();
                        push_u64(&mut hb, 1, 2); // ISO 8-bit
                        push_u64(&mut hb, len as u64, 5);
                        hb.extend_from_slice(&bits[..49]);
                        hdr[2..].copy_from_slice(&bits_to_bytes(&hb));
                        embedded.push(hdr);
                        let blocks = (len as usize * 8 + 1).saturating_sub(49).div_ceil(56);
                        for k in 0..blocks {
                            let mut blk = [0u8; 9];
                            blk[0] = 5 + k as u8;
                            blk[2..].copy_from_slice(&bits_to_bytes(
                                &bits[49 + k * 56..49 + (k + 1) * 56],
                            ));
                            embedded.push(blk);
                        }
                    }
                    for sf in 0..*superframes {
                        let emb_bits = embedded_bits(&embedded[sf as usize % embedded.len()]);
                        for p in 0..6u8 {
                            let frames: [[u8; 72]; 3] = if self.speech.is_empty() {
                                std::array::from_fn(|_| std::array::from_fn(|_| rng.bit()))
                            } else {
                                std::array::from_fn(|_| {
                                    next_speech += 1;
                                    self.speech[(next_speech - 1) % self.speech.len()]
                                })
                            };
                            v.push((t, voice_burst(&frames, p, cc, 0, &emb_bits)));
                            t += 0.06;
                        }
                    }
                    for _ in 0..2 {
                        v.push((t, lc_burst(&lcb, dt::TERMINATOR, cc, 1)));
                        t += 0.06;
                    }
                }
                DmrTx::Text {
                    slot,
                    src,
                    dst,
                    text,
                } => {
                    let v = &mut out[*slot as usize];
                    let latin: Vec<u8> = text.chars().map(|c| c as u32 as u8).collect();
                    let short = ShortData {
                        src: *src,
                        dst: *dst,
                        group: false,
                        format: 3,
                        data: &latin,
                    };
                    for (k, b) in text_bursts(&short, cc, 1).into_iter().enumerate() {
                        v.push((*t0 + 0.06 * k as f64, b));
                    }
                }
            }
        }
        for v in out.iter_mut() {
            v.sort_by(|a, b| a.0.total_cmp(&b.0));
        }
        out
    }

    /// The whole signal at 48 kHz: a continuous base station carrier with CACHs and idle bursts between the schedule's bursts.
    pub fn render(&self, seed: u64) -> Vec<Complex32> {
        let mut rng = sdrkit_algo_sim::rng::Rng::new(seed);
        let bursts = self.slot_bursts(&mut rng);
        let end = bursts
            .iter()
            .filter_map(|v| v.last().map(|b| b.0))
            .fold(0.0, f64::max)
            + 0.5;
        let mut m = FskModulator::new(4800.0, TxPulse::RootRaisedCosine(0.2), 648.0);
        let mut out = Vec::new();
        silence(0.05, &mut out);
        let cycles = (end / 0.03).ceil() as usize;
        let mut idx = [0usize; 2];
        // Short LC: activity update, refreshed every 4 CACHs.
        let mut slc_frag = 0usize;
        let mut slc_bits = [0u8; 68];
        let idle = {
            let mut i = [0u8; 12];
            i[0] = 0;
            encode_bptc(&bits_of::<96>(&i))
        };
        for k in 0..cycles {
            let slot = (k % 2) as u8;
            let t = k as f64 * 0.03;
            let v = &bursts[slot as usize];
            let b = if idx[slot as usize] < v.len() && v[idx[slot as usize]].0 <= t + 1e-9 {
                idx[slot as usize] += 1;
                v[idx[slot as usize] - 1].1
            } else {
                burst_bits(&idle, self.cc, dt::IDLE, 1)
            };
            if slc_frag == 0 {
                let act = |s: usize| -> u8 {
                    let v = &bursts[s];
                    let busy = v.iter().any(|(bt, _)| (bt - t).abs() < 0.12);
                    if busy {
                        8
                    } else {
                        0
                    }
                };
                let data = ((act(0) as u32) << 20) | ((act(1) as u32) << 16);
                slc_bits = encode_short_lc(1, data);
            }
            let lcss = match slc_frag {
                0 => 1,
                3 => 2,
                _ => 3,
            };
            let cach = encode_cach(
                true,
                slot,
                lcss,
                &slc_bits[slc_frag * 17..slc_frag * 17 + 17],
            );
            slc_frag = (slc_frag + 1) % 4;
            let mut bits = Vec::with_capacity(288);
            bits.extend_from_slice(&cach);
            bits.extend_from_slice(&b);
            m.symbols(&bits_to_levels(&bits), &mut out);
        }
        m.flush(16, &mut out);
        silence(0.3, &mut out);
        out
    }
}

/// The colour code of the test signal.
pub const GEN_CC: sdrkit_core::Setting<sdrkit_core::publish::field::I64> =
    sdrkit_core::Setting::new("gen_cc", "Colour code", "", 1)
        .range(0.0, 15.0, 1.0)
        .section("Test signal")
        .advanced();

/// The DMR test signal of the settings in `cfg` ([`GEN_CC`]).
pub fn generator(cfg: &GenConfig) -> Prepared {
    let cc = cfg.settings.get(&GEN_CC) as u8;
    let g = DmrGen::default_schedule(cc & 15);
    Prepared::new(g.render(cfg.seed), cfg.rate_hz, true).with_info(12_500.0, 4800.0, 9600.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn full_lc_round_trips_and_corrects() {
        let lc = Lc {
            pf: false,
            flco: 0,
            fid: 0,
            opts: 0x80,
            dst: 91,
            src: 3101234,
        };
        let mut rs = Codecs::default().rs;
        let mut w = encode_full_lc(&lc.to_bytes(), dt::VOICE_LC_HEADER);
        w[2] ^= 0x55;
        let (b, n) = decode_full_lc(&mut rs, &w, dt::VOICE_LC_HEADER).unwrap();
        assert_eq!((Lc::from_bytes(&b), n), (lc, 1));
        assert!(decode_full_lc(
            &mut rs,
            &encode_full_lc(&lc.to_bytes(), dt::VOICE_LC_HEADER),
            dt::TERMINATOR
        )
        .is_none_or(|x| x.1 > 0));
    }

    #[test]
    fn cach_and_short_lc_round_trip() {
        let payload: [u8; 17] = std::array::from_fn(|i| (i % 3 == 0) as u8);
        let c = encode_cach(true, 1, 2, &payload);
        assert_eq!(decode_cach(&c), ((true, 1, 2, true), payload));
        let s = encode_short_lc(1, 0x08_0000);
        assert_eq!(decode_short_lc(&s), Some((1, 0x08_0000, 0)));
    }

    #[test]
    fn text_formats() {
        assert_eq!(decode_text(3, b"Caf\xe9"), "Café");
        assert_eq!(decode_text(17, &[0, b'H', 0, b'i']), "Hi");
    }
}
