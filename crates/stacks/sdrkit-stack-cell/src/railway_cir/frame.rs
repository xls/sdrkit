//! The application frame of the railway CIR data channel: the header, the CRC-16/XMODEM (high byte first) and the payloads
//! of the end-of-train device (KLW) and of dispatch (TDCS). The header bytes 2 to 9 are checked as the constants of the two
//! protocols; their meaning is not published.

use crate::lbj::message::Locomotives;
use sdrkit_util_bits::crc::catalog::CRC16_XMODEM;

/// The first byte of a frame.
pub const START: u8 = 0x0C;
/// The least and the largest length of a frame, bytes.
pub const LEN_RANGE: (usize, usize) = (13, 257);

/// The protocol of a frame.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Protocol {
    /// End-of-train device data.
    Klw,
    /// Dispatch data.
    Tdcs,
}

impl Protocol {
    /// The header bytes 2 to 6 and 9 (byte 6 of KLW may also be 1).
    fn header(self) -> [u8; 6] {
        match self {
            Protocol::Klw => [0x41, 0x3F, 0x1F, 0x00, 0x00, 0xA5],
            Protocol::Tdcs => [0x00, 0x3F, 0x1F, 0x00, 0x00, 0x30],
        }
    }

    /// The short name.
    pub fn name(self) -> &'static str {
        match self {
            Protocol::Klw => "KLW",
            Protocol::Tdcs => "TDCS",
        }
    }
}

/// A decoded end-of-train device payload.
#[derive(Clone, Debug, PartialEq)]
pub struct Klw {
    /// The locomotive: `<class>-<serial>`, `<code>-<serial>` or the raw characters.
    pub locomotive: String,
    /// The reserved command byte.
    pub reserved: u8,
    /// The action code.
    pub action: u8,
    /// The name of the action.
    pub action_name: String,
    /// The parameter as 4 characters; `None` when absent.
    pub parameter: Option<String>,
    /// The pressure of a pressure action with an all-digit parameter, kPa.
    pub pressure_kpa: Option<u32>,
    /// The tail device id (6 digits).
    pub tail_id: Option<String>,
    /// Extra bytes as hex.
    pub extra: Option<String>,
    /// The calibration clock `20YY-MM-DDThh:mm:ss`; `None` when absent or invalid.
    pub clock: Option<String>,
}

/// A decoded dispatch payload.
#[derive(Clone, Debug, PartialEq)]
pub struct Tdcs {
    /// The signal type.
    pub signal_type: u8,
    /// The semaphore type.
    pub semaphore_type: u8,
    /// The locomotive role (bits 0 and 1).
    pub role: u8,
    /// The train: the prefix and the number.
    pub train: String,
    /// The locomotive type code.
    pub loco_code: u16,
    /// The locomotive serial.
    pub loco_serial: u16,
    /// The locomotive: `<class>-<serial>` or `<code>-<serial>`.
    pub locomotive: String,
    /// The kilometre post; `None` for a test frame or no value.
    pub km_post: Option<f64>,
    /// The mileage increases.
    pub increasing: bool,
    /// The frame marks itself as a test frame.
    pub test_frame: bool,
    /// The speed, km/h.
    pub speed_kmh: u16,
    /// The weight, t.
    pub weight_t: u16,
    /// The length field divided by 10 (its unit is not published).
    pub length: f64,
    /// The car count.
    pub cars: u8,
    /// The station.
    pub station: u16,
    /// The track.
    pub track: u8,
    /// The actual track.
    pub actual_track: u8,
    /// The driver id.
    pub driver: u32,
}

/// The payload of a frame.
#[derive(Clone, Debug, PartialEq)]
pub enum Payload {
    /// End-of-train device data.
    Klw(Klw),
    /// Dispatch data.
    Tdcs(Tdcs),
}

impl Payload {
    /// The protocol.
    pub fn protocol(&self) -> Protocol {
        match self {
            Payload::Klw(_) => Protocol::Klw,
            Payload::Tdcs(_) => Protocol::Tdcs,
        }
    }

    /// The summary line: `KLW <action> <locomotive>` or `TDCS <train> <locomotive>`.
    pub fn summary(&self) -> String {
        match self {
            Payload::Klw(k) => format!("KLW {} {}", k.action_name, k.locomotive),
            Payload::Tdcs(t) => format!("TDCS {} {}", t.train, t.locomotive),
        }
    }
}

/// The length a frame states in its byte 1: `byte 1 + 2`.
pub fn stated_len(b1: u8) -> usize {
    usize::from(b1) + 2
}

/// Whether the first bytes of a frame can begin a frame: the start byte, a length in range, and once 12 bytes exist, the
/// payload length and the constant bytes 7 and 8.
pub fn prefix_ok(b: &[u8]) -> bool {
    if b.first().is_some_and(|&x| x != START) {
        return false;
    }
    if let Some(&l) = b.get(1) {
        let l = stated_len(l);
        if !(LEN_RANGE.0..=LEN_RANGE.1).contains(&l) {
            return false;
        }
    }
    if b.len() >= 12 && (usize::from(b[1]) != usize::from(b[10]) + 11 || b[7] != 0x1F || b[8] != 0x8C) {
        return false;
    }
    true
}

/// Checks a whole frame (header, CRC, protocol) and decodes its payload, the locomotive classes named from `locos`.
/// `None` for any failure.
pub fn parse(f: &[u8], locos: &Locomotives) -> Option<Payload> {
    let l = f.len();
    if !(LEN_RANGE.0..=LEN_RANGE.1).contains(&l)
        || f[0] != START
        || stated_len(f[1]) != l
        || usize::from(f[10]) + 13 != l
        || f[7] != 0x1F
        || f[8] != 0x8C
    {
        return None;
    }
    let crc = u16::from_be_bytes([f[l - 2], f[l - 1]]);
    if CRC16_XMODEM.checksum(&f[..l - 2]) != crc {
        return None;
    }
    let head = [f[2], f[3], f[4], f[5], f[6], f[9]];
    let p = &f[11..l - 2];
    let klw = Protocol::Klw.header();
    if head[..4] == klw[..4] && head[4] <= 1 && head[5] == klw[5] {
        return parse_klw(p, locos).map(Payload::Klw);
    }
    if head == Protocol::Tdcs.header() && p.len() == 32 {
        return parse_tdcs(p, locos).map(Payload::Tdcs);
    }
    None
}

/// The frame of `protocol` around `payload`: the header, the payload and the CRC. The destination byte of KLW is 0.
pub fn build(protocol: Protocol, payload: &[u8]) -> Vec<u8> {
    let h = protocol.header();
    let l = payload.len() + 13;
    let mut f = vec![START, (l - 2) as u8, h[0], h[1], h[2], h[3], h[4], 0x1F, 0x8C, h[5], payload.len() as u8];
    f.extend_from_slice(payload);
    let crc = CRC16_XMODEM.checksum(&f);
    f.extend_from_slice(&crc.to_be_bytes());
    f
}

/// The character of a BCD nibble.
fn bcd_char(n: u8) -> char {
    match n {
        0..=9 => char::from(b'0' + n),
        0xA => '?',
        0xB => 'B',
        0xC => ' ',
        0xD => '-',
        0xE => ']',
        _ => '[',
    }
}

/// The characters of BCD bytes, high nibble first.
fn bcd(bytes: &[u8]) -> String {
    bytes.iter().flat_map(|&b| [bcd_char(b >> 4), bcd_char(b & 0xF)]).collect()
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02X}")).collect()
}

/// The name of a KLW action, and whether its parameter is a pressure.
pub fn action(code: u8) -> (String, bool) {
    let (name, pressure) = match code {
        0x24 => ("Vent", false),
        0x25 => ("Vent response", false),
        0x91 => ("Pressure query", true),
        0x92 => ("Pressure response", true),
        0x83 => ("Disconnect", false),
        0x84 => ("Disconnect response", false),
        0xB5 => ("Connect", false),
        0xB6 => ("Connect response", false),
        0x30 => ("Tail pressure alarm", true),
        0x31 => ("Pressure alarm response", true),
        0xB0 => ("Tail low battery alarm", false),
        0xB1 => ("Low battery response", false),
        0x5B => ("Tail connection request", false),
        0x93 => ("Automatic pressure query", true),
        0x94 => ("Automatic pressure response", true),
        0xA1 => ("Clock calibration", false),
        0xA2 => ("Clock calibration response", false),
        0xA3 => ("Pressure calibration", true),
        0xA4 => ("Pressure calibration response", true),
        _ => return (format!("Unknown 0x{code:02x}"), false),
    };
    (name.to_string(), pressure)
}

/// The action codes that have a name.
pub const ACTIONS: [u8; 19] = [
    0x24, 0x25, 0x91, 0x92, 0x83, 0x84, 0xB5, 0xB6, 0x30, 0x31, 0xB0, 0xB1, 0x5B, 0x93, 0x94, 0xA1, 0xA2, 0xA3, 0xA4,
];

/// `<class>-<serial>`, or `<code>-<serial>` for a code without a class.
fn loco_name(code: u32, serial: &str, locos: &Locomotives) -> String {
    match locos.class(&code.to_string()) {
        Some(c) => format!("{c}-{serial}"),
        None => format!("{code:03}-{serial}"),
    }
}

/// The locomotive of 8 BCD characters: code (characters 1 to 3) and serial (5 to 8) when all are digits, else the raw text.
fn klw_loco(chars: &str, locos: &Locomotives) -> String {
    if chars.len() == 8 && chars.bytes().all(|c| c.is_ascii_digit()) {
        let code: u32 = chars[..3].parse().unwrap_or(0);
        loco_name(code, &chars[4..], locos)
    } else {
        chars.to_string()
    }
}

fn klw_clock(b: &[u8]) -> Option<String> {
    let c = bcd(b);
    if !c.bytes().all(|x| x.is_ascii_digit()) {
        return None;
    }
    let n = |i: usize| c[i..i + 2].parse::<u32>().unwrap_or(99);
    let (y, mo, d, h, mi, s) = (2000 + n(0), n(2), n(4), n(6), n(8), n(10));
    let leap = (y % 4 == 0 && y % 100 != 0) || y % 400 == 0;
    let days = match mo {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if leap => 29,
        2 => 28,
        _ => return None,
    };
    (d >= 1 && d <= days && h < 24 && mi < 60 && s < 60)
        .then(|| format!("{y:04}-{mo:02}-{d:02}T{h:02}:{mi:02}:{s:02}"))
}

fn parse_klw(p: &[u8], locos: &Locomotives) -> Option<Klw> {
    // (locomotive characters, command bytes, parameter bytes, tail id, extra, clock)
    let (loco, cmd, param, tail, extra, clock) = match p.len() {
        6 => (bcd(&p[0..2]), [p[2], p[3]], [p[4], p[5]], None, None, None),
        11 if p[6..9] == [0xAA, 0xAA, 0xA0] => (
            bcd(&p[0..2]),
            [p[4], p[5]],
            [p[9], p[10]],
            None,
            Some(format!("{}{}", hex(&p[2..4]), hex(&p[6..9]))),
            None,
        ),
        11 => (bcd(&p[0..4]), [p[4], p[5]], [p[9], p[10]], Some(bcd(&p[6..9])), None, None),
        17 if p[5] == 0xA1 || p[5] == 0xA2 => (
            bcd(&p[0..4]),
            [p[4], p[5]],
            [p[9], p[10]],
            Some(bcd(&p[6..9])),
            None,
            Some(klw_clock(&p[11..17])),
        ),
        15 => (
            bcd(&[p[7], p[8], p[0], p[1]]),
            [p[2], p[3]],
            [p[4], p[5]],
            Some(bcd(&[p[6], p[9], p[10]])),
            Some(hex(&p[11..15])),
            None,
        ),
        _ => return None,
    };
    let (action_name, pressure_action) = action(cmd[1]);
    let parameter = (!(param[0] == 0xFF && (param[1] == 0xFF || param[1] == 0xA5))).then(|| bcd(&param));
    let pressure_kpa = parameter
        .as_deref()
        .filter(|s| pressure_action && s.bytes().all(|c| c.is_ascii_digit()))
        .and_then(|s| s.parse().ok());
    Some(Klw {
        locomotive: klw_loco(&loco, locos),
        reserved: cmd[0],
        action: cmd[1],
        action_name,
        parameter,
        pressure_kpa,
        tail_id: tail,
        extra,
        clock: clock.flatten(),
    })
}

fn le(b: &[u8]) -> u32 {
    b.iter().rev().fold(0, |a, &x| a << 8 | u32::from(x))
}

fn parse_tdcs(p: &[u8], locos: &Locomotives) -> Option<Tdcs> {
    let mut prefix = String::new();
    for &c in &p[3..7] {
        match c {
            0 | b' ' => {}
            32..=126 => prefix.push(char::from(c)),
            _ => return None,
        }
    }
    let code = le(&p[10..12]) as u16;
    let serial = le(&p[12..14]) as u16;
    let km = le(&p[14..17]);
    let test_frame = km == 9_999_888 || km == 8_888_888;
    let no_value = km == 9_999_999 || km == 0xFF_FFFF;
    let metres = f64::from(km & 0x3F_FFFF);
    let km_post = (!test_frame && !no_value).then(|| if km & 0x80_0000 != 0 { -metres } else { metres } / 1000.0);
    Some(Tdcs {
        signal_type: p[0],
        semaphore_type: p[1],
        role: p[2] & 3,
        train: format!("{prefix}{}", le(&p[7..10])),
        loco_code: code,
        loco_serial: serial,
        locomotive: loco_name(u32::from(code), &format!("{serial:04}"), locos),
        km_post,
        increasing: km & 0x40_0000 != 0,
        test_frame,
        speed_kmh: (le(&p[17..20]) & 0x3FF) as u16,
        weight_t: le(&p[20..22]) as u16,
        length: f64::from(le(&p[22..24])) / 10.0,
        cars: p[24],
        station: le(&p[25..27]) as u16,
        track: p[27],
        actual_track: p[28],
        driver: le(&p[29..32]),
    })
}
