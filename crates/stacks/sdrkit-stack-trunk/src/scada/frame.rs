//! The frames in a burst of bytes: DNP3 link frames, Modbus RTU frames and exceptions of known functions with their accepted
//! lengths, Modbus ASCII frames, and the damaged or unknown Modbus frames of a burst with no good frame.
//!
//! Modbus over Serial Line V1.02 (RTU and ASCII framing, CRC-16 and LRC); IEEE Std 1815 (DNP3 link frames, CRC-16/DNP).

use sdrkit_util_bits::checksum::sum8;
use sdrkit_util_bits::crc::catalog::{CRC16_DNP, CRC16_MODBUS};
use sdrkit_util_bits::crc::Trailer;

/// The protocol of a frame.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Protocol {
    /// Modbus RTU (binary, CRC-16).
    ModbusRtu,
    /// Modbus ASCII (hex characters, LRC).
    ModbusAscii,
    /// DNP3 (IEEE 1815).
    Dnp3,
}

impl Protocol {
    /// Its name.
    pub fn label(self) -> &'static str {
        match self {
            Protocol::ModbusRtu => "Modbus RTU",
            Protocol::ModbusAscii => "Modbus ASCII",
            Protocol::Dnp3 => "DNP3",
        }
    }
}

/// A frame found in a burst.
#[derive(Clone, Debug, PartialEq)]
pub struct Found {
    /// Its protocol.
    pub protocol: Protocol,
    /// Its first byte in the burst.
    pub start: usize,
    /// Its bytes in the burst.
    pub len: usize,
    /// The frame as the reader takes it: an RTU frame with its CRC, the binary bytes of an ASCII frame with the LRC, a DNP3
    /// frame with its CRCs.
    pub bytes: Vec<u8>,
    /// Every check holds.
    pub crc_ok: bool,
    /// A note: the data block whose CRC failed, a function that is not known.
    pub note: Option<String>,
}

/// The Modbus functions that the reader knows.
pub const KNOWN_FUNCTIONS: [u8; 19] = [
    1, 2, 3, 4, 5, 6, 7, 8, 11, 12, 15, 16, 17, 20, 21, 22, 23, 24, 43,
];
/// The Modbus exception codes that the reader knows.
pub const EXCEPTION_CODES: [u8; 10] = [1, 2, 3, 4, 5, 6, 7, 8, 10, 11];
/// The functions that a broadcast (address 0) may carry.
pub const BROADCAST_FUNCTIONS: [u8; 5] = [5, 6, 8, 15, 16];
/// The longest Modbus RTU frame.
pub const MAX_RTU: usize = 256;

fn be16(b: &[u8], at: usize) -> Option<usize> {
    Some(usize::from(*b.get(at)?) << 8 | usize::from(*b.get(at + 1)?))
}

/// The accepted lengths (address to CRC) of a Modbus frame of function `f` whose bytes start `b` (at least the address and
/// the function), up to four of them; 0 marks an empty place. Function 43 has none: it is found by its CRC only.
pub fn accepted_lengths(f: u8, b: &[u8]) -> [usize; 4] {
    let n2 = b.get(2).map(|&v| usize::from(v));
    let mut out = [0usize; 4];
    let mut k = 0;
    let mut add = |l: Option<usize>| {
        if let Some(l) = l {
            if k < 4 && l >= 4 {
                out[k] = l;
                k += 1;
            }
        }
    };
    match f {
        1 | 2 => {
            add(Some(8));
            add(n2.filter(|&n| n > 0).map(|n| 5 + n));
        }
        3 | 4 => {
            add(Some(8));
            add(n2.filter(|&n| n > 0 && n % 2 == 0).map(|n| 5 + n));
        }
        5 | 6 | 8 => add(Some(8)),
        7 => {
            add(Some(4));
            add(Some(5));
        }
        11 => {
            add(Some(4));
            add(Some(8));
        }
        12 | 17 => {
            add(Some(4));
            add(n2.map(|n| 5 + n));
        }
        15 | 16 => {
            add(Some(8));
            let q = be16(b, 4);
            let n6 = b.get(6).map(|&v| usize::from(v));
            if let (Some(q), Some(n)) = (q, n6) {
                let want = if f == 15 { q.div_ceil(8) } else { 2 * q };
                if n == want && n > 0 {
                    add(Some(9 + n));
                }
            }
        }
        20 | 21 => add(n2.filter(|&n| n > 0).map(|n| 5 + n)),
        22 => add(Some(10)),
        23 => {
            if let (Some(w), Some(m)) = (be16(b, 8), b.get(10)) {
                if usize::from(*m) == 2 * w && w > 0 {
                    add(Some(13 + usize::from(*m)));
                }
            }
            add(n2.filter(|&n| n > 0 && n % 2 == 0).map(|n| 5 + n));
        }
        24 => {
            add(Some(6));
            add(be16(b, 2).map(|n| 6 + n));
        }
        _ => {}
    }
    out
}

/// Whether `a` is a station address that may carry function `f`.
fn address_ok(a: u8, f: u8) -> bool {
    (1..=247).contains(&a) || (a == 0 && BROADCAST_FUNCTIONS.contains(&f))
}

fn crc_ok(frame: &[u8]) -> bool {
    CRC16_MODBUS.verify(frame, Trailer::Little)
}

/// The length of a DNP3 link frame of user-data length `len` (the length byte), with its CRCs.
pub fn dnp3_frame_len(len: u8) -> usize {
    let user = usize::from(len).saturating_sub(5);
    10 + user + 2 * user.div_ceil(16)
}

/// The user data of a DNP3 frame without its CRCs, and the first data block (from 1) whose CRC fails.
pub fn dnp3_user_data(frame: &[u8]) -> (Vec<u8>, Option<usize>) {
    let mut data = Vec::new();
    let mut bad = None;
    let mut at = 10;
    let mut block = 1;
    while at + 2 < frame.len() {
        let take = (frame.len() - at - 2).min(16);
        let chunk = &frame[at..at + take + 2];
        if bad.is_none() && !CRC16_DNP.verify(chunk, Trailer::Little) {
            bad = Some(block);
        }
        data.extend_from_slice(&chunk[..take]);
        at += take + 2;
        block += 1;
    }
    (data, bad)
}

/// A DNP3 frame at the start of `b`: 05 64, a length of at least 5, a good header CRC, the whole frame in `b`.
fn dnp3_at(b: &[u8], start: usize) -> Option<Found> {
    if b.len() < 10 || b[0] != 0x05 || b[1] != 0x64 || b[2] < 5 {
        return None;
    }
    if !CRC16_DNP.verify(&b[..10], Trailer::Little) {
        return None;
    }
    let len = dnp3_frame_len(b[2]);
    if b.len() < len {
        return None;
    }
    let bytes = b[..len].to_vec();
    let (_, bad) = dnp3_user_data(&bytes);
    Some(Found {
        protocol: Protocol::Dnp3,
        start,
        len,
        bytes,
        crc_ok: bad.is_none(),
        note: bad.map(|n| format!("CRC failed in data block {n}")),
    })
}

/// A Modbus exception at the start of `b`.
fn exception_at(b: &[u8], start: usize) -> Option<Found> {
    if b.len() < 5 || !(1..=247).contains(&b[0]) || b[1] & 0x80 == 0 {
        return None;
    }
    if !KNOWN_FUNCTIONS.contains(&(b[1] & 0x7F)) || !EXCEPTION_CODES.contains(&b[2]) {
        return None;
    }
    crc_ok(&b[..5]).then(|| Found {
        protocol: Protocol::ModbusRtu,
        start,
        len: 5,
        bytes: b[..5].to_vec(),
        crc_ok: true,
        note: None,
    })
}

/// A Modbus frame of a known function at the start of `b`, with an accepted length and a good CRC.
fn modbus_at(b: &[u8], start: usize) -> Option<Found> {
    if b.len() < 4 || !KNOWN_FUNCTIONS.contains(&b[1]) || !address_ok(b[0], b[1]) {
        return None;
    }
    let good = |len: usize| len <= b.len() && len <= MAX_RTU && crc_ok(&b[..len]);
    let len = if b[1] == 43 {
        (4..=b.len().min(MAX_RTU)).find(|&l| good(l))?
    } else {
        accepted_lengths(b[1], b)
            .into_iter()
            .find(|&l| l > 0 && good(l))?
    };
    Some(Found {
        protocol: Protocol::ModbusRtu,
        start,
        len,
        bytes: b[..len].to_vec(),
        crc_ok: true,
        note: None,
    })
}

/// The good frames of a burst, in order: at each place a DNP3 frame, else a Modbus exception, else a Modbus frame.
pub fn find_frames(burst: &[u8], out: &mut Vec<Found>) {
    let mut at = 0;
    while at + 4 <= burst.len() {
        let b = &burst[at..];
        let found = dnp3_at(b, at)
            .or_else(|| exception_at(b, at))
            .or_else(|| modbus_at(b, at));
        match found {
            Some(f) => {
                at += f.len;
                out.push(f);
            }
            None => at += 1,
        }
    }
}

/// The whole of a burst with no good frame read as one Modbus frame: of an unknown function when its CRC is good, or a
/// damaged frame of a known function when its length is an accepted one. `None` otherwise.
pub fn whole_burst(burst: &[u8]) -> Option<Found> {
    let n = burst.len();
    if !(4..=MAX_RTU).contains(&n) || !(1..=247).contains(&burst[0]) {
        return None;
    }
    let f = burst[1];
    if crc_ok(burst) {
        return (!KNOWN_FUNCTIONS.contains(&(f & 0x7F))).then(|| Found {
            protocol: Protocol::ModbusRtu,
            start: 0,
            len: n,
            bytes: burst.to_vec(),
            crc_ok: true,
            note: Some(format!("function {f} is not known")),
        });
    }
    let accepted = if f & 0x80 != 0 {
        KNOWN_FUNCTIONS.contains(&(f & 0x7F)) && n == 5
    } else {
        KNOWN_FUNCTIONS.contains(&f) && accepted_lengths(f, burst).contains(&n)
    };
    accepted.then(|| Found {
        protocol: Protocol::ModbusRtu,
        start: 0,
        len: n,
        bytes: burst.to_vec(),
        crc_ok: false,
        note: Some("CRC failed".into()),
    })
}

fn hex_value(c: u8) -> Option<u8> {
    match c {
        b'0'..=b'9' => Some(c - b'0'),
        b'A'..=b'F' => Some(c - b'A' + 10),
        b'a'..=b'f' => Some(c - b'a' + 10),
        _ => None,
    }
}

/// The Modbus ASCII frames of a run of characters (7 bits each): a colon, hex pairs (address, function, data, LRC), CR LF.
/// The LRC is the two's complement of the 8-bit sum of the bytes before it.
pub fn find_ascii(chars: &[u8], out: &mut Vec<Found>) {
    let mut at = 0;
    while at < chars.len() {
        if chars[at] & 0x7F != b':' {
            at += 1;
            continue;
        }
        let Some(end) = chars[at + 1..]
            .windows(2)
            .position(|w| w[0] & 0x7F == b'\r' && w[1] & 0x7F == b'\n')
        else {
            return;
        };
        let body = &chars[at + 1..at + 1 + end];
        let len = end + 3;
        let start = at;
        at += 1;
        if body.len() < 6 || !body.len().is_multiple_of(2) || body.len() > 2 * MAX_RTU {
            continue;
        }
        let bytes: Option<Vec<u8>> = body
            .chunks(2)
            .map(|p| Some(hex_value(p[0] & 0x7F)? << 4 | hex_value(p[1] & 0x7F)?))
            .collect();
        let Some(bytes) = bytes else {
            continue;
        };
        if !address_ok(bytes[0], bytes[1] & 0x7F) && bytes[1] & 0x80 == 0 {
            continue;
        }
        let ok = sum8(&bytes) == 0;
        out.push(Found {
            protocol: Protocol::ModbusAscii,
            start,
            len,
            bytes,
            crc_ok: ok,
            note: (!ok).then(|| "LRC failed".to_string()),
        });
        at = start + len;
    }
}

/// The ASCII characters of a Modbus ASCII frame of `bytes` (address to data): colon, hex pairs, the LRC, CR LF.
pub fn ascii_frame(bytes: &[u8]) -> Vec<u8> {
    let lrc = 0u8.wrapping_sub(sum8(bytes));
    let mut out = vec![b':'];
    for &b in bytes.iter().chain(std::iter::once(&lrc)) {
        for nib in [b >> 4, b & 0xF] {
            out.push(if nib < 10 {
                b'0' + nib
            } else {
                b'A' + nib - 10
            });
        }
    }
    out.extend_from_slice(b"\r\n");
    out
}

/// A Modbus RTU frame of `bytes` (address to data) with its CRC.
pub fn rtu_frame(bytes: &[u8]) -> Vec<u8> {
    let mut f = bytes.to_vec();
    CRC16_MODBUS
        .append(&mut f, Trailer::Little)
        .expect("a 16-bit CRC appends");
    f
}

/// A DNP3 link frame: the header of `control`, `dest` and `src`, then `user` in blocks of 16 bytes, each with its CRC.
pub fn dnp3_frame(control: u8, dest: u16, src: u16, user: &[u8]) -> Vec<u8> {
    let len = u8::try_from(5 + user.len()).expect("at most 250 bytes of user data");
    let mut f = vec![0x05, 0x64, len, control];
    f.extend_from_slice(&dest.to_le_bytes());
    f.extend_from_slice(&src.to_le_bytes());
    CRC16_DNP
        .append(&mut f, Trailer::Little)
        .expect("a 16-bit CRC appends");
    for chunk in user.chunks(16) {
        let mut c = chunk.to_vec();
        CRC16_DNP
            .append(&mut c, Trailer::Little)
            .expect("a 16-bit CRC appends");
        f.extend_from_slice(&c);
    }
    f
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hex(s: &str) -> Vec<u8> {
        s.split_whitespace()
            .map(|h| u8::from_str_radix(h, 16).unwrap())
            .collect()
    }

    #[test]
    fn the_check_values() {
        assert_eq!(CRC16_MODBUS.checksum(b"123456789"), 0x4B37);
        assert_eq!(CRC16_DNP.checksum(b"123456789"), 0xEA82);
        assert_eq!(
            sdrkit_util_bits::crc::catalog::CRC16_IBM_SDLC.checksum(b"123456789"),
            0x906E
        );
    }

    #[test]
    fn the_worked_frames_are_found() {
        for f in [
            "11 03 00 6B 00 03 76 87",
            "11 03 06 02 2B 00 00 00 64 C8 BA",
            "01 03 00 00 00 0A C5 CD",
            "01 83 02 C0 F1",
        ] {
            let b = hex(f);
            let mut out = Vec::new();
            find_frames(&b, &mut out);
            assert_eq!(out.len(), 1, "{f}");
            assert_eq!(out[0].bytes, b);
            assert!(out[0].crc_ok);
            assert_eq!(out[0].protocol, Protocol::ModbusRtu);
        }
        for f in [
            "05 64 05 C9 01 00 03 00 75 3E",
            "05 64 14 C4 01 00 03 00 04 09 C0 C1 01 3C 02 06 3C 03 06 3C 04 06 3C 01 06 7A 6F",
        ] {
            let b = hex(f);
            let mut out = Vec::new();
            find_frames(&b, &mut out);
            assert_eq!(out.len(), 1, "{f}");
            assert_eq!(out[0].protocol, Protocol::Dnp3);
            assert!(out[0].crc_ok);
        }
        assert_eq!(
            rtu_frame(&hex("11 03 00 6B 00 03")),
            hex("11 03 00 6B 00 03 76 87")
        );
        assert_eq!(
            dnp3_frame(0xC9, 1, 3, &[]),
            hex("05 64 05 C9 01 00 03 00 75 3E")
        );
    }

    #[test]
    fn garbage_around_frames_and_damaged_frames() {
        let a = hex("11 03 00 6B 00 03 76 87");
        let d = hex("05 64 05 C9 01 00 03 00 75 3E");
        let mut b = hex("FF 00 13 55");
        b.extend_from_slice(&a);
        b.extend_from_slice(&hex("AA 37"));
        b.extend_from_slice(&d);
        b.push(0x99);
        let mut out = Vec::new();
        find_frames(&b, &mut out);
        assert_eq!(out.len(), 2);
        assert_eq!((out[0].start, out[0].bytes.clone()), (4, a.clone()));
        assert_eq!(out[1].bytes, d);
        // A damaged frame is no good frame, but the whole burst reads as a damaged frame of function 3.
        let mut bad = a.clone();
        bad[5] ^= 0x10;
        let mut out = Vec::new();
        find_frames(&bad, &mut out);
        assert!(out.is_empty());
        let w = whole_burst(&bad).unwrap();
        assert!(!w.crc_ok);
        // An unknown function with a good CRC.
        let u = rtu_frame(&hex("11 41 01 02 03"));
        let mut out = Vec::new();
        find_frames(&u, &mut out);
        assert!(out.is_empty());
        let w = whole_burst(&u).unwrap();
        assert!(w.crc_ok);
        assert_eq!(w.note.as_deref(), Some("function 65 is not known"));
        // A DNP3 frame with a damaged data block.
        let mut f =
            hex("05 64 14 C4 01 00 03 00 04 09 C0 C1 01 3C 02 06 3C 03 06 3C 04 06 3C 01 06 7A 6F");
        f[14] ^= 1;
        let mut out = Vec::new();
        find_frames(&f, &mut out);
        assert_eq!(out.len(), 1);
        assert!(!out[0].crc_ok);
        assert_eq!(out[0].note.as_deref(), Some("CRC failed in data block 1"));
    }

    #[test]
    fn random_bytes_give_no_frame() {
        let mut x = 0x1234_5678u32;
        let mut b = vec![0u8; 20_000];
        for v in &mut b {
            x ^= x << 13;
            x ^= x >> 17;
            x ^= x << 5;
            *v = x as u8;
        }
        let mut out = Vec::new();
        find_frames(&b, &mut out);
        // Each place passes a 16-bit check by chance with 2^-16 at most a few lengths: well under 2 in 20 000 places.
        assert!(out.len() <= 2, "{}", out.len());
    }

    #[test]
    fn ascii_frames_round_trip() {
        let a = ascii_frame(&hex("11 03 00 6B 00 03"));
        assert_eq!(a, b":1103006B00037E\r\n");
        let mut chars = b"xx".to_vec();
        chars.extend_from_slice(&a);
        let mut out = Vec::new();
        find_ascii(&chars, &mut out);
        assert_eq!(out.len(), 1);
        assert!(out[0].crc_ok);
        assert_eq!(out[0].bytes, hex("11 03 00 6B 00 03 7E"));
        let mut bad = a.clone();
        bad[4] = b'4';
        let mut out = Vec::new();
        find_ascii(&bad, &mut out);
        assert!(!out[0].crc_ok);
    }
}
