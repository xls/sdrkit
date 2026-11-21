//! First-generation 406 MHz beacon messages (C/S T.001): fields, the 15-hex ID, protocols, modified Baudot, positions, and the two
//! BCH codes. Bits are numbered from 1 as in T.001 (bit k is `bits[k - 1]`).

#![allow(clippy::unusual_byte_groupings)] // the groups follow the message fields

use sdrkit_algo_fec::bch::{codes, Bch};
use serde::{Deserialize, Serialize};
use std::sync::OnceLock;

/// Bit sync and the two frame syncs.
pub const FRAME_SYNC_NORMAL: [bool; 9] = [false, false, false, true, false, true, true, true, true];
pub const FRAME_SYNC_TEST: [bool; 9] = [false, true, true, false, true, false, false, false, false];

/// The two BCH codes of a first-generation message, with their working buffer, so a repair allocates nothing.
///
/// Standard: C/S T.001 Annex B (BCH-1 (82, 61) over PDF-1, BCH-2 (38, 26) over PDF-2).
#[derive(Clone, Debug)]
pub struct Codes {
    bch1: Bch,
    bch2: Bch,
    word: Vec<u8>,
}

impl Default for Codes {
    fn default() -> Self {
        Self::new()
    }
}

impl Codes {
    /// The decoders of both codes.
    pub fn new() -> Codes {
        Codes {
            bch1: Bch::from_code(&codes::SARSAT_82_61).expect("a named code builds"),
            bch2: Bch::from_code(&codes::SARSAT_38_26).expect("a named code builds"),
            word: Vec::with_capacity(82),
        }
    }
    /// Repairs the 82 bits of PDF-1 and BCH-1 in place. Returns the number of bits corrected, or `None` when the field cannot
    /// be repaired (the bits are then left as received) or `field` is not 82 bits long.
    pub fn repair1(&mut self, field: &mut [bool]) -> Option<usize> {
        repair(&mut self.bch1, &mut self.word, field)
    }
    /// Repairs the 38 bits of PDF-2 and BCH-2 in place, as [`Codes::repair1`].
    pub fn repair2(&mut self, field: &mut [bool]) -> Option<usize> {
        repair(&mut self.bch2, &mut self.word, field)
    }
}

fn repair(bch: &mut Bch, word: &mut Vec<u8>, field: &mut [bool]) -> Option<usize> {
    word.clear();
    word.extend(field.iter().map(|&b| b as u8));
    let d = bch.decode(word).ok()?;
    for (f, &w) in field.iter_mut().zip(word.iter()) {
        *f = w != 0;
    }
    Some(d.corrected as usize)
}

/// The parity of `info` under `code`, for the generator.
fn parity(code: &'static OnceLock<Bch>, which: &codes::BchCode, info: &[bool]) -> Vec<bool> {
    let bch = code.get_or_init(|| Bch::from_code(which).expect("a named code builds"));
    let mut w: Vec<u8> = info.iter().map(|&b| b as u8).collect();
    w.resize(which.n, 0);
    // The length is the code's own, so encoding cannot fail.
    let _ = bch.encode(&mut w);
    w[which.k..].iter().map(|&b| b != 0).collect()
}

static ENC1: OnceLock<Bch> = OnceLock::new();
static ENC2: OnceLock<Bch> = OnceLock::new();

/// The value of bits read first bit first.
pub(crate) fn value_of(bits: &[bool]) -> u64 {
    bits.iter().fold(0, |v, &b| (v << 1) | b as u64)
}

/// The modified Baudot code (6 bits) of T.001 Annex: letters with a leading 1, figures with a leading 0.
const BAUDOT6: &[(char, u8)] = &[
    ('A', 0b111000),
    ('B', 0b110011),
    ('C', 0b101110),
    ('D', 0b110010),
    ('E', 0b110000),
    ('F', 0b110110),
    ('G', 0b101011),
    ('H', 0b100101),
    ('I', 0b101100),
    ('J', 0b111010),
    ('K', 0b111110),
    ('L', 0b101001),
    ('M', 0b100111),
    ('N', 0b100110),
    ('O', 0b100011),
    ('P', 0b101101),
    ('Q', 0b111101),
    ('R', 0b101010),
    ('S', 0b110100),
    ('T', 0b100001),
    ('U', 0b111100),
    ('V', 0b101111),
    ('W', 0b111001),
    ('X', 0b110111),
    ('Y', 0b110101),
    ('Z', 0b110001),
    (' ', 0b100100),
    ('-', 0b011000),
    ('/', 0b010111),
    ('0', 0b001101),
    ('1', 0b011101),
    ('2', 0b011001),
    ('3', 0b010000),
    ('4', 0b001010),
    ('5', 0b000001),
    ('6', 0b010101),
    ('7', 0b011100),
    ('8', 0b001100),
    ('9', 0b000011),
];

/// A modified Baudot character from its 6-bit code ('?' when not in the table).
pub fn baudot_char(v: u8) -> char {
    BAUDOT6
        .iter()
        .find(|e| e.1 == v)
        .map(|e| e.0)
        .unwrap_or('?')
}

/// The 6-bit code of a character (space when not in the table).
pub fn baudot_code(c: char) -> u8 {
    BAUDOT6
        .iter()
        .find(|e| e.0 == c.to_ascii_uppercase())
        .map(|e| e.1)
        .unwrap_or(0b100100)
}

/// The 5-bit (letters only) code of the aircraft operator designator.
fn baudot5(c: char) -> u8 {
    baudot_code(c) & 0x1F
}
fn baudot5_char(v: u8) -> char {
    baudot_char(v | 0x20)
}

/// A letter of the 5-bit shortened Baudot ('?' for a value that is no letter).
fn letter5(v: u8) -> char {
    let c = baudot5_char(v);
    if c.is_ascii_uppercase() {
        c
    } else {
        '?'
    }
}

/// Three letters of the 5-bit shortened Baudot from 15 bits.
fn three_letters(v: u64) -> String {
    (0..3).map(|i| letter5((v >> (10 - 5 * i) & 0x1F) as u8)).collect()
}

/// The 15 bits of three letters in the 5-bit shortened Baudot.
fn three_letters_code(t: &str) -> u64 {
    t.chars()
        .chain("   ".chars())
        .take(3)
        .fold(0, |a, c| (a << 5) | u64::from(baudot5(c)))
}

/// What a beacon identifies itself with (the generator's description; the decoder fills [`Fgb`]).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum FgbId {
    /// Location protocol 2: the last 6 digits of the MMSI and a beacon number.
    StdMmsi { mmsi6: u32, beacon: u8 },
    /// Location protocol 3.
    StdAircraft { addr: u32 },
    /// Location protocols 4, 6, 7.
    StdSerial { code: u8, cert: u16, serial: u16 },
    /// Location protocol 5.
    StdOperator { operator: String, serial: u16 },
    /// Location protocol 12.
    Ssas { mmsi6: u32 },
    /// Location protocol 14.
    StdTest { data: u32 },
    /// National location protocols 8, 10, 11, 15.
    National { code: u8, id: u32 },
    /// ELT(DT) location protocol 9: the identity type (bits 41-42) and the 24 identity bits; PDF-2: the means of activation
    /// (2 bits), the altitude code (4 bits), the freshness of the location (2 bits) and, with freshness 0, the aircraft
    /// operator designator of rotating field type 000. `cancel` makes the cancellation message.
    EltDt {
        id_type: u8,
        addr: u32,
        activation: u8,
        altitude: u8,
        freshness: u8,
        operator: Option<String>,
        cancel: bool,
    },
    /// User protocol 2: six characters (call sign or MMSI digits) and a beacon character.
    UserMaritime { text: String, beacon: char },
    /// User protocol 6.
    UserCallSign { text: String, beacon: char },
    /// User protocol 1: the aircraft marking (7 characters) and the ELT number.
    UserAviation { marking: String, elt: u8 },
    /// User protocol 3: beacon type bits (40 to 42) and a 20-bit serial.
    UserSerial { btype: u8, serial: u32 },
    /// User protocols 0, 4, 7 with raw bits 40 to 85.
    UserOther { code: u8, bits: u64 },
}

/// A beacon to encode.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FgbSpec {
    pub mid: u32,
    pub long: bool,
    pub self_test: bool,
    pub id: FgbId,
    pub position: Option<(f64, f64)>,
    pub homing: bool,
}

/// A decoded first-generation message.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Fgb {
    pub long: bool,
    pub self_test: bool,
    pub hex_id: String,
    /// "location" or "user".
    pub protocol: String,
    pub protocol_code: u8,
    pub protocol_name: String,
    pub beacon_type: String,
    pub mid: u32,
    pub country: String,
    pub identification: String,
    pub position: bool,
    pub fine: bool,
    pub lat: Option<f64>,
    pub lon: Option<f64>,
    /// Half the cell that holds the true position, degrees.
    pub resolution_deg: f64,
    pub homing_121_5: bool,
    pub bch1_ok: bool,
    /// `None` for a short message.
    pub bch2_ok: Option<bool>,
    pub bits_corrected: usize,
    /// The message (bits 1 to 112 or 144) as hex.
    pub raw_hex: String,
}

fn put(bits: &mut [bool], first: usize, last: usize, v: u64) {
    let n = last - first + 1;
    for i in 0..n {
        bits[first - 1 + i] = (v >> (n - 1 - i)) & 1 == 1;
    }
}

fn get(bits: &[bool], first: usize, last: usize) -> u64 {
    value_of(&bits[first - 1..last])
}

fn bits_hex(bits: &[bool]) -> String {
    let mut s = String::new();
    let mut i = 0;
    while i < bits.len() {
        let n = (bits.len() - i).min(4);
        let mut v = value_of(&bits[i..i + n]);
        v <<= 4 - n;
        s.push(
            std::char::from_digit(v as u32, 16)
                .expect("a value below 16")
                .to_ascii_uppercase(),
        );
        i += 4;
    }
    s
}

/// Location protocol names.
pub fn location_name(code: u8) -> &'static str {
    match code {
        2 => "EPIRB, MMSI and location",
        3 => "ELT, aircraft address and location",
        4 => "ELT, serial and location",
        5 => "ELT, aircraft operator and location",
        6 => "EPIRB, serial and location",
        7 => "PLB, serial and location",
        8 => "ELT, national location",
        9 => "ELT-DT location",
        10 => "EPIRB, national location",
        11 => "PLB, national location",
        12 => "Ship security (SSAS)",
        13 => "RLS location",
        14 => "Location test",
        15 => "National location test",
        _ => "Spare",
    }
}

/// User protocol names.
pub fn user_name(code: u8) -> &'static str {
    match code {
        0 => "Orbitography",
        1 => "Aviation user",
        2 => "Maritime user",
        3 => "Serial user",
        4 => "National user",
        6 => "Radio call sign user",
        7 => "Test user",
        _ => "Spare",
    }
}

fn location_type(code: u8) -> &'static str {
    match code {
        2 | 6 | 10 => "EPIRB",
        3..=5 | 8 => "ELT",
        9 => "ELT-DT",
        7 | 11 => "PLB",
        12 => "Ship security",
        13 => "RLS",
        14 | 15 => "Test",
        _ => "?",
    }
}

fn serial_type(btype: u8) -> &'static str {
    match btype {
        0 | 1 | 3 => "ELT",
        2 | 4 => "EPIRB",
        6 => "PLB",
        _ => "?",
    }
}

/// The offset of a signed coordinate from its signed coarse cell: (true = plus, minutes up to `max_min`, 4-second units 0 to 14).
fn offset(actual: f64, coarse: f64, max_min: u64) -> (bool, u64, u64) {
    let d = actual - coarse;
    let units = ((d.abs() * 3600.0) / 4.0).round() as u64;
    let secs = units * 4;
    let (m, s) = (secs / 60, (secs % 60) / 4);
    (d >= 0.0, m.min(max_min), s.min(14))
}

/// The altitude of an ELT(DT) altitude code (T.001 A3.3.8).
fn altitude_text(code: u64) -> &'static str {
    const T: [&str; 16] = [
        "<=400 m", ">400 m", ">800 m", ">1200 m", ">1600 m", ">2200 m", ">2800 m", ">3400 m", ">4000 m", ">4800 m",
        ">5600 m", ">6600 m", ">7600 m", ">8800 m", ">10000 m", "not available",
    ];
    T[(code & 15) as usize]
}

/// Computes BCH-1 over bits 25-85 into 86-106 and, for a long message (144 bits), BCH-2 over bits 107-132 into 133-144.
pub fn seal(b: &mut [bool]) {
    let p1 = parity(&ENC1, &codes::SARSAT_82_61, &b[24..85]);
    b[85..106].copy_from_slice(&p1);
    if b.len() >= 144 {
        let p2 = parity(&ENC2, &codes::SARSAT_38_26, &b[106..132]);
        b[132..144].copy_from_slice(&p2);
    }
}

/// The signed coarse value of a coordinate in cells of `cell` degrees.
fn coarse(v: f64, cell: f64) -> f64 {
    v.signum() * (v.abs() / cell).round() * cell
}

fn apply_offset(coarse: f64, plus: bool, min: u64, s4: u64) -> f64 {
    let d = min as f64 / 60.0 + (s4 * 4) as f64 / 3600.0;
    if plus {
        coarse + d
    } else {
        coarse - d
    }
}

/// Encodes a beacon to its 112 or 144 bits (with sync and both BCH codes).
pub fn encode(spec: &FgbSpec) -> Vec<bool> {
    let mut b = vec![false; 144];
    put(&mut b, 1, 15, 0x7FFF);
    let fs = if spec.self_test {
        FRAME_SYNC_TEST
    } else {
        FRAME_SYNC_NORMAL
    };
    b[15..24].copy_from_slice(&fs);
    b[24] = spec.long;
    put(&mut b, 27, 36, spec.mid as u64);
    // PDF-2 offsets default to all ones (no fine position).
    let mut pdf2_set = false;
    match &spec.id {
        FgbId::StdMmsi { .. }
        | FgbId::StdAircraft { .. }
        | FgbId::StdSerial { .. }
        | FgbId::StdOperator { .. }
        | FgbId::Ssas { .. }
        | FgbId::StdTest { .. } => {
            let code = match &spec.id {
                FgbId::StdMmsi { .. } => 2,
                FgbId::StdAircraft { .. } => 3,
                FgbId::StdSerial { code, .. } => *code,
                FgbId::StdOperator { .. } => 5,
                FgbId::Ssas { .. } => 12,
                _ => 14,
            };
            b[25] = false;
            put(&mut b, 37, 40, code as u64);
            match &spec.id {
                FgbId::StdMmsi { mmsi6, beacon } => {
                    put(&mut b, 41, 60, *mmsi6 as u64);
                    put(&mut b, 61, 64, *beacon as u64);
                }
                FgbId::StdAircraft { addr } => put(&mut b, 41, 64, *addr as u64),
                FgbId::StdSerial { cert, serial, .. } => {
                    put(&mut b, 41, 50, *cert as u64);
                    put(&mut b, 51, 64, *serial as u64);
                }
                FgbId::StdOperator { operator, serial } => {
                    let c: Vec<char> = operator.chars().chain("   ".chars()).take(3).collect();
                    for (i, ch) in c.iter().enumerate() {
                        put(&mut b, 41 + 5 * i, 45 + 5 * i, baudot5(*ch) as u64);
                    }
                    put(&mut b, 56, 64, *serial as u64);
                }
                FgbId::Ssas { mmsi6 } => put(&mut b, 41, 60, *mmsi6 as u64),
                FgbId::StdTest { data } => put(&mut b, 41, 64, *data as u64),
                _ => {}
            }
            // PDF-2: 107-110 fixed 1101, 111 position source (1 = internal), 112 homing; offsets of the signed coordinates: sign
            // (1 = plus), minutes (5 bits), 4-second units (4 bits); 1 00000 1111 is no offset.
            put(&mut b, 107, 110, 0b1101);
            b[110] = true;
            b[111] = spec.homing;
            match spec.position {
                Some((lat, lon)) => {
                    let clat = coarse(lat, 0.25);
                    let clon = coarse(lon, 0.25);
                    b[64] = lat < 0.0;
                    put(&mut b, 66, 74, (clat.abs() * 4.0).round() as u64);
                    b[74] = lon < 0.0;
                    put(&mut b, 76, 85, (clon.abs() * 4.0).round() as u64);
                    let (sp, m, s) = offset(lat, clat, 30);
                    b[112] = sp;
                    put(&mut b, 114, 118, m);
                    put(&mut b, 119, 122, s);
                    let (sp, m, s) = offset(lon, clon, 30);
                    b[122] = sp;
                    put(&mut b, 124, 128, m);
                    put(&mut b, 129, 132, s);
                }
                None => {
                    b[64] = false;
                    put(&mut b, 66, 74, 0x1FF);
                    b[74] = false;
                    put(&mut b, 76, 85, 0x3FF);
                    put(&mut b, 113, 122, 0b1_00000_1111);
                    put(&mut b, 123, 132, 0b1_00000_1111);
                }
            }
            pdf2_set = true;
        }
        FgbId::National { code, id } => {
            put(&mut b, 37, 40, *code as u64);
            put(&mut b, 41, 58, *id as u64);
            put(&mut b, 107, 109, 0b110);
            b[109] = true;
            b[110] = true;
            b[111] = spec.homing;
            match spec.position {
                Some((lat, lon)) => {
                    // 2-minute cells.
                    let cl = (lat.abs() * 30.0).round() / 30.0;
                    let co = (lon.abs() * 30.0).round() / 30.0;
                    b[58] = lat < 0.0;
                    put(&mut b, 60, 66, cl.trunc() as u64);
                    put(&mut b, 67, 71, ((cl.fract() * 30.0).round() as u64).min(29));
                    b[71] = lon < 0.0;
                    put(&mut b, 73, 80, co.trunc() as u64);
                    put(&mut b, 81, 85, ((co.fract() * 30.0).round() as u64).min(29));
                    let sl = if lat < 0.0 { -cl } else { cl };
                    let so = if lon < 0.0 { -co } else { co };
                    let (sp, m, s) = offset(lat, sl, 3);
                    b[112] = sp;
                    put(&mut b, 114, 115, m.min(3));
                    put(&mut b, 116, 119, s.min(15));
                    let (sp, m, s) = offset(lon, so, 3);
                    b[119] = sp;
                    put(&mut b, 121, 122, m.min(3));
                    put(&mut b, 123, 126, s.min(15));
                }
                None => {
                    put(&mut b, 59, 71, 0b0_1111111_00000);
                    put(&mut b, 72, 85, 0b0_11111111_00000);
                    put(&mut b, 113, 126, 0x3FFF);
                }
            }
            pdf2_set = true;
        }
        FgbId::EltDt {
            id_type,
            addr,
            activation,
            altitude,
            freshness,
            operator,
            cancel,
        } => {
            put(&mut b, 37, 40, 9);
            put(&mut b, 41, 42, u64::from(*id_type));
            put(&mut b, 43, 66, u64::from(*addr));
            if *cancel {
                put(&mut b, 67, 75, 0x1FA);
                put(&mut b, 76, 85, 0x3FA);
                put(&mut b, 107, 114, 0x3C);
                put(&mut b, 115, 132, 0x1E0F0);
            } else {
                let mut offsets = None;
                match spec.position {
                    Some((lat, lon)) => {
                        let cl = coarse(lat, 0.5);
                        let co = coarse(lon, 0.5);
                        b[66] = lat < 0.0;
                        put(&mut b, 68, 75, (cl.abs() * 2.0).round() as u64);
                        b[75] = lon < 0.0;
                        put(&mut b, 77, 85, (co.abs() * 2.0).round() as u64);
                        offsets = Some((offset(lat, cl, 15), offset(lon, co, 15)));
                    }
                    None => {
                        put(&mut b, 67, 75, 0x0FF);
                        put(&mut b, 76, 85, 0x1FF);
                    }
                }
                put(&mut b, 107, 108, u64::from(*activation & 3));
                put(&mut b, 109, 112, u64::from(*altitude & 15));
                put(&mut b, 113, 114, u64::from(*freshness & 3));
                if *freshness == 0 {
                    put(&mut b, 115, 117, 0);
                    put(&mut b, 118, 132, three_letters_code(operator.as_deref().unwrap_or("")));
                } else {
                    let ((ap, am, asec), (op, om, osec)) =
                        offsets.unwrap_or(((true, 0, 15), (true, 0, 15)));
                    b[114] = ap;
                    put(&mut b, 116, 119, am);
                    put(&mut b, 120, 123, asec);
                    b[123] = op;
                    put(&mut b, 125, 128, om);
                    put(&mut b, 129, 132, osec);
                }
            }
            pdf2_set = true;
        }
        FgbId::UserMaritime { text, beacon } | FgbId::UserCallSign { text, beacon } => {
            let code = if matches!(spec.id, FgbId::UserMaritime { .. }) {
                2
            } else {
                6
            };
            b[25] = true;
            put(&mut b, 37, 39, code);
            let c: Vec<char> = text.chars().chain(std::iter::repeat(' ')).take(6).collect();
            for (i, ch) in c.iter().enumerate() {
                put(&mut b, 40 + 6 * i, 45 + 6 * i, baudot_code(*ch) as u64);
            }
            put(&mut b, 76, 81, baudot_code(*beacon) as u64);
            put(&mut b, 84, 85, if spec.homing { 1 } else { 0 });
        }
        FgbId::UserAviation { marking, elt } => {
            b[25] = true;
            put(&mut b, 37, 39, 1);
            let c: Vec<char> = marking
                .chars()
                .chain(std::iter::repeat(' '))
                .take(7)
                .collect();
            for (i, ch) in c.iter().enumerate() {
                put(&mut b, 40 + 6 * i, 45 + 6 * i, baudot_code(*ch) as u64);
            }
            put(&mut b, 82, 83, *elt as u64);
            put(&mut b, 84, 85, if spec.homing { 1 } else { 0 });
        }
        FgbId::UserSerial { btype, serial } => {
            b[25] = true;
            put(&mut b, 37, 39, 3);
            put(&mut b, 40, 42, *btype as u64);
            put(&mut b, 44, 63, *serial as u64);
            put(&mut b, 84, 85, if spec.homing { 1 } else { 0 });
        }
        FgbId::UserOther { code, bits } => {
            b[25] = true;
            put(&mut b, 37, 39, *code as u64);
            put(&mut b, 40, 85, *bits & ((1u64 << 46) - 1));
        }
    }
    if !pdf2_set && spec.long {
        // User-location: 107-132 position in 4-minute steps when given, else zeros.
        if let Some((lat, lon)) = spec.position {
            b[106] = lat < 0.0;
            let la = lat.abs();
            put(&mut b, 108, 114, la.trunc() as u64);
            put(
                &mut b,
                115,
                118,
                ((la.fract() * 15.0).round() as u64).min(14),
            );
            b[118] = lon < 0.0;
            let lo = lon.abs();
            put(&mut b, 120, 127, lo.trunc() as u64);
            put(
                &mut b,
                128,
                131,
                ((lo.fract() * 15.0).round() as u64).min(14),
            );
            b[131] = true;
        }
    }
    if !spec.long {
        b.truncate(112);
    }
    seal(&mut b);
    b
}

/// Decodes a message (112 or 144 bits, sync included) with the BCH repair of `codes`. Returns `None` when the message is short
/// or the first field cannot be repaired.
pub fn decode(raw: &[bool], codes: &mut Codes) -> Option<Fgb> {
    if raw.len() < 112 {
        return None;
    }
    let mut b = raw.to_vec();
    let long = b[24];
    if long && b.len() < 144 {
        return None;
    }
    b.truncate(if long { 144 } else { 112 });
    let mut corrected = 0;
    corrected += codes.repair1(&mut b[24..106])?;
    let mut bch2_ok = None;
    if long {
        match codes.repair2(&mut b[106..144]) {
            Some(c) => {
                corrected += c;
                bch2_ok = Some(true);
            }
            None => bch2_ok = Some(false),
        }
    }
    let self_test = b[15..24] == FRAME_SYNC_TEST;
    let user = b[25];
    let mid = get(&b, 27, 36) as u32;
    let mut d = Fgb {
        long,
        self_test,
        mid,
        country: sdrkit_util_ids::mid::country(mid as u16)
            .unwrap_or("")
            .to_string(),
        bch1_ok: true,
        bch2_ok,
        bits_corrected: corrected,
        raw_hex: bits_hex(&b),
        ..Default::default()
    };
    let pdf2 = long && bch2_ok == Some(true);
    // The hex ID: bits 26 to 85 with the position bits at their defaults.
    let mut idb = b[25..85].to_vec();
    let set_id = |idb: &mut Vec<bool>, first: usize, last: usize, v: u64| {
        let n = last - first + 1;
        for i in 0..n {
            idb[first - 26 + i] = (v >> (n - 1 - i)) & 1 == 1;
        }
    };
    if !user {
        let code = get(&b, 37, 40) as u8;
        d.protocol = "location".into();
        d.protocol_code = code;
        d.protocol_name = location_name(code).into();
        d.beacon_type = location_type(code).into();
        match code {
            2 | 3 | 4 | 5 | 6 | 7 | 12 | 14 => {
                d.identification = match code {
                    2 => format!(
                        "MMSI {}, beacon {}",
                        mid as u64 * 1_000_000 + get(&b, 41, 60),
                        get(&b, 61, 64)
                    ),
                    3 => format!("aircraft address {:06X}", get(&b, 41, 64)),
                    5 => {
                        let op: String = (0..3)
                            .map(|i| baudot5_char(get(&b, 41 + 5 * i, 45 + 5 * i) as u8))
                            .collect();
                        format!(
                            "aircraft operator {}, serial {}",
                            op.trim(),
                            get(&b, 56, 64)
                        )
                    }
                    12 => format!("MMSI {}", mid as u64 * 1_000_000 + get(&b, 41, 60)),
                    14 => format!("test data {:06X}", get(&b, 41, 64)),
                    _ => format!(
                        "certificate {}, serial {}",
                        get(&b, 41, 50),
                        get(&b, 51, 64)
                    ),
                };
                set_id(&mut idb, 65, 74, 0x1FF);
                set_id(&mut idb, 75, 85, 0x3FF);
                let lat_v = get(&b, 66, 74);
                let lon_v = get(&b, 76, 85);
                if !(lat_v == 0x1FF && lon_v == 0x3FF && !b[64] && !b[74]) {
                    let mut lat = lat_v as f64 / 4.0 * if b[64] { -1.0 } else { 1.0 };
                    let mut lon = lon_v as f64 / 4.0 * if b[74] { -1.0 } else { 1.0 };
                    d.resolution_deg = 0.125;
                    let seconds = pdf2.then(|| (get(&b, 119, 122), get(&b, 129, 132)));
                    if let Some((las, los)) = seconds.filter(|&(a, o)| a <= 14 && o <= 14) {
                        lat = apply_offset(lat, b[112], get(&b, 114, 118), las);
                        lon = apply_offset(lon, b[122], get(&b, 124, 128), los);
                        d.fine = true;
                        d.resolution_deg = 2.0 / 3600.0;
                    }
                    if lat.abs() <= 90.0 && lon.abs() <= 180.0 {
                        d.position = true;
                        d.lat = Some(lat);
                        d.lon = Some(lon);
                    }
                }
                d.homing_121_5 = long && b[111];
            }
            8 | 10 | 11 | 15 => {
                d.identification = format!("national id {}", get(&b, 41, 58));
                set_id(&mut idb, 59, 71, 0b0_1111111_00000);
                set_id(&mut idb, 72, 85, 0b0_11111111_00000);
                let def =
                    get(&b, 59, 71) == 0b0_1111111_00000 && get(&b, 72, 85) == 0b0_11111111_00000;
                if !def {
                    let mut lat = (get(&b, 60, 66) as f64 + get(&b, 67, 71) as f64 / 30.0)
                        * if b[58] { -1.0 } else { 1.0 };
                    let mut lon = (get(&b, 73, 80) as f64 + get(&b, 81, 85) as f64 / 30.0)
                        * if b[71] { -1.0 } else { 1.0 };
                    d.resolution_deg = 1.0 / 60.0;
                    if pdf2 && get(&b, 113, 126) != 0x3FFF {
                        lat = apply_offset(lat, b[112], get(&b, 114, 115), get(&b, 116, 119));
                        lon = apply_offset(lon, b[119], get(&b, 121, 122), get(&b, 123, 126));
                        d.fine = true;
                        d.resolution_deg = 2.0 / 3600.0;
                    }
                    d.position = true;
                    d.lat = Some(lat);
                    d.lon = Some(lon);
                }
                d.homing_121_5 = long && b[111];
            }
            9 => elt_dt(&b, pdf2, &mut d, &mut idb),
            13 => {
                d.identification = format!("bits {:011X}", get(&b, 41, 85));
                d.homing_121_5 = long && b[107];
            }
            _ => {
                d.identification = format!("bits {:011X}", get(&b, 41, 85));
            }
        }
    } else {
        let code = get(&b, 37, 39) as u8;
        d.protocol = "user".into();
        d.protocol_code = code;
        d.protocol_name = user_name(code).into();
        let aux = get(&b, 84, 85);
        // The orbitography protocol (user code 0) has no homing bits.
        d.homing_121_5 = code != 0 && aux == 1;
        match code {
            2 | 6 => {
                let txt: String = (0..6)
                    .map(|i| baudot_char(get(&b, 40 + 6 * i, 45 + 6 * i) as u8))
                    .collect();
                let bc = baudot_char(get(&b, 76, 81) as u8);
                d.identification = format!("call sign / MMSI digits {}, beacon {bc}", txt.trim());
                d.beacon_type = "EPIRB".into();
            }
            1 => {
                let txt: String = (0..7)
                    .map(|i| baudot_char(get(&b, 40 + 6 * i, 45 + 6 * i) as u8))
                    .collect();
                d.identification =
                    format!("aircraft marking {}, ELT {}", txt.trim(), get(&b, 82, 83));
                d.beacon_type = "ELT".into();
            }
            3 => {
                let bt = get(&b, 40, 42) as u8;
                d.identification = format!("serial {}", get(&b, 44, 63));
                d.beacon_type = serial_type(bt).into();
            }
            0 | 7 => {
                d.identification = format!("bits {:012X}", get(&b, 40, 85));
                d.beacon_type = "Test".into();
            }
            _ => {
                d.identification = format!("bits {:012X}", get(&b, 40, 85));
                d.beacon_type = "?".into();
            }
        }
        if long && pdf2 && b[131] {
            let mut lat = get(&b, 108, 114) as f64 + get(&b, 115, 118) as f64 / 15.0;
            let mut lon = get(&b, 120, 127) as f64 + get(&b, 128, 131) as f64 / 15.0;
            if b[106] {
                lat = -lat;
            }
            if b[118] {
                lon = -lon;
            }
            d.position = true;
            d.lat = Some(lat);
            d.lon = Some(lon);
            d.resolution_deg = 2.0 / 60.0;
        }
    }
    if self_test {
        d.beacon_type = "Test".into();
    }
    d.hex_id = bits_hex(&idb);
    Some(d)
}

/// Reads the ELT(DT) location protocol (T.001 A3.3.8) of message `b` into `d`, and puts the default position into the bits of
/// the hex ID `idb`. `pdf2` is set when the second BCH field checked.
fn elt_dt(b: &[bool], pdf2: bool, d: &mut Fgb, idb: &mut [bool]) {
    let it = get(b, 41, 42);
    let ident = get(b, 43, 66);
    d.identification = match it {
        0 => format!("aircraft address {ident:06X}"),
        1 => format!("operator {}, serial {}", three_letters(ident >> 9), ident & 0x1FF),
        2 => format!("certificate {}, serial {}", ident >> 14, ident & 0x3FFF),
        _ => format!("identity type 11 (reserved), {ident:06X}"),
    };
    // The hex ID holds the default PDF-1 position: 0 11111111 0 111111111.
    for (first, n, v) in [(67usize, 9usize, 0x0FFu64), (76, 10, 0x1FF)] {
        for k in 0..n {
            idb[first - 26 + k] = (v >> (n - 1 - k)) & 1 == 1;
        }
    }
    let test = ident == 0 || ident == 0xFF_FFFF;
    if test {
        d.beacon_type = "Test".into();
        d.identification.push_str(", location test");
    }
    let cancel = get(b, 67, 75) == 0x1FA
        && get(b, 76, 85) == 0x3FA
        && (!pdf2 || (get(b, 107, 114) == 0x3C && get(b, 115, 132) == 0x1E0F0));
    if cancel {
        d.identification.push_str(", cancellation message (alert cancelled)");
        return;
    }
    let (lat_v, lon_v) = (get(b, 68, 75), get(b, 77, 85));
    let default = !b[66] && lat_v == 0xFF && !b[75] && lon_v == 0x1FF;
    let mut position = None;
    if !default && lat_v <= 180 && lon_v <= 360 {
        let lat = lat_v as f64 / 2.0 * if b[66] { -1.0 } else { 1.0 };
        let lon = lon_v as f64 / 2.0 * if b[75] { -1.0 } else { 1.0 };
        d.resolution_deg = 0.25;
        position = Some((lat, lon));
    }
    if pdf2 {
        let t = &mut d.identification;
        t.push_str(match get(b, 107, 108) {
            0 => ", manual activation",
            1 => ", automatic activation by the beacon",
            2 => ", automatic activation by external means",
            _ => ", activation spare",
        });
        t.push_str(", altitude ");
        t.push_str(altitude_text(get(b, 109, 112)));
        match get(b, 113, 114) {
            0 => {
                let kind = get(b, 115, 117);
                if kind == 0 {
                    t.push_str(&format!(", operator {}", three_letters(get(b, 118, 132))));
                } else {
                    t.push_str(&format!(", rotating field type {kind}"));
                }
            }
            f => {
                t.push_str(match f {
                    1 => ", location more than 60 s old or default",
                    2 => ", location 2 to 60 s old",
                    _ => ", location current",
                });
                let (las, los) = (get(b, 120, 123), get(b, 129, 132));
                if let Some((lat, lon)) = position.as_mut() {
                    if las <= 14 && los <= 14 {
                        *lat = apply_offset(*lat, b[114], get(b, 116, 119), las);
                        *lon = apply_offset(*lon, b[123], get(b, 125, 128), los);
                        d.fine = true;
                        d.resolution_deg = 2.0 / 3600.0;
                    }
                }
            }
        }
    }
    if let Some((lat, lon)) = position {
        d.position = true;
        d.lat = Some(lat);
        d.lon = Some(lon);
    }
    // ELT(DT) has no 121.5 MHz homing bit.
    d.homing_121_5 = false;
}

/// The test beacons of the generator (made-up, never registered), in the order the generator sends them.
pub fn test_beacons() -> Vec<FgbSpec> {
    vec![
        FgbSpec {
            mid: 277,
            long: true,
            self_test: false,
            id: FgbId::StdMmsi {
                mmsi6: 123456,
                beacon: 3,
            },
            position: Some((55.70444, 20.98778)),
            homing: true,
        },
        FgbSpec {
            mid: 366,
            long: false,
            self_test: false,
            id: FgbId::StdSerial {
                code: 7,
                cert: 421,
                serial: 4021,
            },
            position: Some((37.75, -122.5)),
            homing: false,
        },
        FgbSpec {
            mid: 211,
            long: true,
            self_test: false,
            id: FgbId::StdAircraft { addr: 0x3C6444 },
            position: Some((54.13111, 12.09556)),
            homing: true,
        },
        FgbSpec {
            mid: 219,
            long: false,
            self_test: false,
            id: FgbId::UserMaritime {
                text: "OXTEST".into(),
                beacon: '1',
            },
            position: None,
            homing: false,
        },
        FgbSpec {
            mid: 512,
            long: true,
            self_test: false,
            id: FgbId::StdSerial {
                code: 6,
                cert: 695,
                serial: 777,
            },
            position: Some((-36.84889, 174.76333)),
            homing: false,
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use sdrkit_algo_sim::rng::Rng;

    fn dec(b: &[bool]) -> Option<Fgb> {
        decode(b, &mut Codes::new())
    }

    #[test]
    fn round_trip_of_the_test_beacons() {
        for s in test_beacons() {
            let bits = encode(&s);
            assert_eq!(bits.len(), if s.long { 144 } else { 112 });
            let d = dec(&bits).unwrap();
            assert_eq!(d.hex_id.len(), 15);
            assert_eq!(d.mid, s.mid);
            assert!(!d.country.is_empty());
            assert_eq!(d.homing_121_5, s.homing);
            assert_eq!(d.bch2_ok.is_some(), s.long);
            match s.position {
                Some((lat, lon)) => {
                    let (la, lo) = (d.lat.unwrap(), d.lon.unwrap());
                    assert!((la - lat).abs() <= d.resolution_deg + 1e-9, "{la} {lat}");
                    assert!((lo - lon).abs() <= d.resolution_deg + 1e-9, "{lo} {lon}");
                    assert_eq!(d.fine, s.long);
                }
                None => assert!(!d.position),
            }
        }
    }

    #[test]
    fn hex_id_does_not_follow_the_position() {
        let mut s = test_beacons()[0].clone();
        let a = dec(&encode(&s)).unwrap().hex_id;
        s.position = Some((10.0, 10.0));
        let b = dec(&encode(&s)).unwrap().hex_id;
        s.position = None;
        let c = dec(&encode(&s)).unwrap().hex_id;
        assert_eq!(a, b);
        assert_eq!(a, c);
    }

    #[test]
    fn repair_rules() {
        let mut rng = Rng::new(4);
        let s = test_beacons()[0].clone();
        let good = encode(&s);
        for ne in 1..=3 {
            let mut b = good.clone();
            for k in 0..ne {
                b[30 + 17 * k] = !b[30 + 17 * k];
            }
            let d = dec(&b).unwrap();
            assert_eq!(d.bits_corrected, ne);
        }
        let mut b = good.clone();
        b[110] = !b[110];
        b[120] = !b[120];
        assert_eq!(dec(&b).unwrap().bch2_ok, Some(true));
        b[125] = !b[125];
        assert_eq!(dec(&b).unwrap().bch2_ok, Some(false));
        let mut refused = 0;
        for _ in 0..200 {
            let mut b = good.clone();
            let mut pos = Vec::new();
            while pos.len() < 4 {
                let p = 24 + rng.below(82) as usize;
                if !pos.contains(&p) {
                    pos.push(p);
                }
            }
            for p in pos {
                b[p] = !b[p];
            }
            if dec(&b).is_none() {
                refused += 1;
            }
        }
        assert!(refused >= 180, "{refused}");
    }

    #[test]
    fn baudot_round_trip() {
        for t in ["OXTEST", "N123-9"] {
            let s: String = t.chars().map(|c| baudot_char(baudot_code(c))).collect();
            assert_eq!(s, t);
        }
    }

    #[test]
    fn elt_dt_and_self_test() {
        let s = FgbSpec {
            mid: 227,
            long: true,
            self_test: false,
            id: FgbId::EltDt {
                id_type: 0,
                addr: 0x39C0D4,
                activation: 0,
                altitude: 14,
                freshness: 3,
                operator: None,
                cancel: false,
            },
            position: Some((43.56, 1.48111)),
            homing: true,
        };
        let d = dec(&encode(&s)).unwrap();
        assert_eq!(d.protocol_code, 9);
        assert_eq!(d.protocol_name, "ELT-DT location");
        assert!((d.lat.unwrap() - 43.56).abs() < 0.001 && (d.lon.unwrap() - 1.48111).abs() < 0.001);
        let id1 = d.hex_id.clone();
        let mut s2 = s.clone();
        s2.position = None;
        assert_eq!(dec(&encode(&s2)).unwrap().hex_id, id1);
        let mut s3 = s.clone();
        s3.self_test = true;
        assert_eq!(dec(&encode(&s3)).unwrap().beacon_type, "Test");
    }
}
