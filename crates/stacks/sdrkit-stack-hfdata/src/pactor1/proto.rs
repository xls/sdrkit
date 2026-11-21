//! The PACTOR-I protocol layer from its public description: packets, status byte, CRC-16 CCITT,
//! control signals, connect packets, text and the Huffman table.

use sdrkit_util_bits::crc::catalog::CRC16_IBM_3740;
use sdrkit_util_bits::{pack, BitOrder, Pad};

/// The packet CRC: CRC-16/IBM-3740 (x^16 + x^12 + x^5 + 1, start FFFF, not reflected, not inverted) over the data and status
/// bytes.
pub fn packet_crc(data: &[u8]) -> u16 {
    CRC16_IBM_3740.checksum(data)
}

/// The four control signals (first bit sent = the most significant bit).
pub const CS: [u16; 4] = [0x4D5, 0xAB2, 0x34B, 0xD2C];

/// What a control signal means.
pub fn cs_meaning(i: usize) -> &'static str {
    [
        "acknowledge",
        "reject / busy",
        "changeover (the other side sends next)",
        "speed change",
    ][i.min(3)]
}

/// The 12 bits of control signal `i`, in sending order.
pub fn cs_bits(i: usize) -> [u8; 12] {
    let v = CS[i];
    let mut b = [0u8; 12];
    for (k, x) in b.iter_mut().enumerate() {
        *x = ((v >> (11 - k)) & 1) as u8;
    }
    b
}

/// The nearest control signal to 12 received bits (sending order) and its distance.
pub fn cs_match(bits: &[u8]) -> (usize, u32) {
    let mut v = 0u16;
    for &b in bits.iter().take(12) {
        v = (v << 1) | (b & 1) as u16;
    }
    let mut best = (0usize, 99u32);
    for (i, &c) in CS.iter().enumerate() {
        let d = (v ^ c).count_ones();
        if d < best.1 {
            best = (i, d);
        }
    }
    best
}

/// The status byte.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Status {
    pub number: u8,
    pub huffman: bool,
    pub break_in: bool,
    pub qrt: bool,
}

impl Status {
    pub fn byte(self) -> u8 {
        (self.number & 3)
            | (self.huffman as u8) << 2
            | (self.break_in as u8) << 3
            | (self.qrt as u8) << 4
    }
    pub fn from_byte(b: u8) -> Status {
        Status {
            number: b & 3,
            huffman: b & 4 != 0,
            break_in: b & 8 != 0,
            qrt: b & 16 != 0,
        }
    }
}

/// Appends the bits of `bytes` to `out`, each byte least significant bit first (the PACTOR bit order).
pub fn bytes_to_bits(bytes: &[u8], out: &mut Vec<u8>) {
    pack::unpack(bytes, BitOrder::Lsb, out);
}

/// Packs bits (each byte least significant bit first) into bytes. A last partial byte is padded with zero bits.
pub fn bits_to_bytes(bits: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(bits.len().div_ceil(8));
    pack::pack(bits, BitOrder::Lsb, Pad::Zero, &mut out);
    out
}

/// The bits of a packet: header, data (8 or 20 bytes), status, CRC high byte then low byte.
pub fn packet_bits(header: u8, data: &[u8], status: Status) -> Vec<u8> {
    let mut body = data.to_vec();
    body.push(status.byte());
    let crc = packet_crc(&body);
    let mut bytes = vec![header];
    bytes.extend_from_slice(&body);
    bytes.push((crc >> 8) as u8);
    bytes.push(crc as u8);
    let mut bits = Vec::with_capacity(bytes.len() * 8);
    bytes_to_bits(&bytes, &mut bits);
    bits
}

/// A packet read from its bits.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Packet {
    pub header: u8,
    /// Bit errors of the header from the nearer of 55 and AA.
    pub header_errors: u32,
    pub data: Vec<u8>,
    pub status: Status,
    pub crc_ok: bool,
}

/// Whether hard bits (96 or 192) are a packet with a good CRC and a header within one bit of 55 or AA, as [`packet_ok`] of
/// [`read_packet`] says. `bytes` is scratch space, so a check allocates nothing once it has its size.
pub fn check_packet(bits: &[u8], bytes: &mut Vec<u8>) -> bool {
    bytes.clear();
    pack::pack(bits, BitOrder::Lsb, Pad::Zero, bytes);
    let n = bytes.len();
    if n < 3 {
        return false;
    }
    let header = bytes[0];
    let he = (header ^ 0x55)
        .count_ones()
        .min((header ^ 0xAA).count_ones());
    let crc_rx = (u16::from(bytes[n - 2]) << 8) | u16::from(bytes[n - 1]);
    he <= 1 && packet_crc(&bytes[1..n - 2]) == crc_rx
}

/// Reads hard bits (96 or 192) as a packet.
pub fn read_packet(bits: &[u8]) -> Packet {
    let bytes = bits_to_bytes(bits);
    let n = bytes.len();
    let header = bytes[0];
    let he = (header ^ 0x55)
        .count_ones()
        .min((header ^ 0xAA).count_ones());
    let body = &bytes[1..n - 2];
    let crc_rx = ((bytes[n - 2] as u16) << 8) | bytes[n - 1] as u16;
    let crc = packet_crc(body);
    Packet {
        header: if (header ^ 0x55).count_ones() <= (header ^ 0xAA).count_ones() {
            0x55
        } else {
            0xAA
        },
        header_errors: he,
        data: body[..body.len() - 1].to_vec(),
        status: Status::from_byte(body[body.len() - 1]),
        crc_ok: crc == crc_rx,
    }
}

/// Whether a packet's bits pass: CRC good and the header at most 1 bit from 55 or AA.
pub fn packet_ok(p: &Packet) -> bool {
    p.crc_ok && p.header_errors <= 1
}

/// The 8 data bytes of a connect packet: the callsign padded with 0F.
pub fn connect_data(call: &str) -> Vec<u8> {
    let mut d: Vec<u8> = call.bytes().take(8).collect();
    while d.len() < 8 {
        d.push(0x0F);
    }
    d
}

fn call_char(c: u8) -> bool {
    c.is_ascii_uppercase() || c.is_ascii_digit() || c == b'-' || c == b'/'
}

/// The callsign of a connect packet's data: 3 to 7 characters padded with 0F (8 without padding count only when they read as a call).
pub fn connect_call(data: &[u8]) -> Option<String> {
    if data.len() != 8 {
        return None;
    }
    let n = data.iter().position(|&b| b == 0x0F).unwrap_or(8);
    if data[n..].iter().any(|&b| b != 0x0F) {
        return None;
    }
    let s = &data[..n];
    if !(3..=8).contains(&s.len()) || !s.iter().all(|&c| call_char(c)) {
        return None;
    }
    if n == 8 {
        // Eight characters have no padding and read as text.
        return None;
    }
    let s = String::from_utf8_lossy(s).to_string();
    s.chars().any(|c| c.is_ascii_digit()).then_some(s)
}

/// Whether a word looks like an amateur callsign: 1 to 3 letters or digits (at most one leading digit), a digit, 1 to 3 letters; 4 to 8
/// characters in all.
pub fn is_callsign(w: &str) -> bool {
    let b = w.as_bytes();
    if !(4..=8).contains(&b.len())
        || !b
            .iter()
            .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit())
    {
        return false;
    }
    // The suffix: 1 to 3 letters at the end.
    let suf = b
        .iter()
        .rev()
        .take_while(|c| c.is_ascii_uppercase())
        .count();
    if !(1..=3).contains(&suf) || suf >= b.len() {
        return false;
    }
    let rest = &b[..b.len() - suf];
    // The digit before the suffix.
    if !rest.last().is_some_and(|c| c.is_ascii_digit()) {
        return false;
    }
    let pre = &rest[..rest.len() - 1];
    if !(1..=3).contains(&pre.len()) {
        return false;
    }
    let leading_digits = pre.iter().take_while(|c| c.is_ascii_digit()).count();
    leading_digits <= 1 && pre.iter().any(|c| c.is_ascii_uppercase())
}

/// Plain 8-bit text: printable shown, CR and LF as a line break, others '.'; trailing spaces and NULs removed.
pub fn ascii_text(data: &[u8]) -> String {
    let mut end = data.len();
    while end > 0 && (data[end - 1] == b' ' || data[end - 1] == 0) {
        end -= 1;
    }
    let mut s = String::new();
    let mut last_cr = false;
    for &c in &data[..end] {
        match c {
            b'\r' => {
                s.push('\n');
                last_cr = true;
                continue;
            }
            b'\n' => {
                if !last_cr {
                    s.push('\n');
                }
            }
            0x20..=0x7E => s.push(c as char),
            _ => s.push('.'),
        }
        last_cr = false;
    }
    s
}

/// The Huffman table: (character, code bits in sending order).
#[derive(Clone, Debug, PartialEq)]
pub struct Huffman {
    pub entries: Vec<(u8, Vec<u8>)>,
}

fn code(s: &str) -> Vec<u8> {
    s.bytes()
        .filter(|c| *c == b'0' || *c == b'1')
        .map(|c| c - b'0')
        .collect()
}

impl Default for Huffman {
    /// The entries the public description shows as examples.
    fn default() -> Self {
        Huffman {
            entries: vec![
                (b' ', code("10")),
                (b'e', code("011")),
                (b't', code("00000")),
                (b's', code("00100")),
                (b'a', code("01000")),
                (b'\r', code("001100")),
                (b'\n', code("001101")),
            ],
        }
    }
}

impl Huffman {
    /// Loads a table from text: one entry per line, the character itself or its decimal code, a tab, the code bits; '#' lines and
    /// empty lines skipped. Fewer than 3 entries is refused.
    pub fn parse(text: &str) -> Option<Huffman> {
        let mut entries = Vec::new();
        for line in text.lines() {
            if line.trim().is_empty() || line.starts_with('#') {
                continue;
            }
            let (k, v) = line.split_once('\t')?;
            let c = if k.chars().count() == 1 {
                let ch = k.chars().next()?;
                if !ch.is_ascii() {
                    return None;
                }
                ch as u8
            } else {
                k.trim().parse::<u8>().ok()?
            };
            let bits = code(v);
            if bits.is_empty()
                || bits.len() > 15
                || v.chars().any(|ch| !matches!(ch, '0' | '1' | ' '))
            {
                return None;
            }
            entries.push((c, bits));
        }
        (entries.len() >= 3).then_some(Huffman { entries })
    }
    /// All 95 printable characters are in it.
    pub fn complete(&self) -> bool {
        (0x20u8..0x7F).all(|c| self.entries.iter().any(|e| e.0 == c))
    }
    /// The code bits of a text, or `None` when a character is not in the table.
    pub fn encode(&self, text: &str) -> Option<Vec<u8>> {
        let mut out = Vec::new();
        for c in text.bytes() {
            let e = self.entries.iter().find(|e| e.0 == c)?;
            out.extend_from_slice(&e.1);
        }
        Some(out)
    }
    /// Decodes bits: the text and whether an unknown code stopped it ('~').
    pub fn decode(&self, bits: &[u8]) -> (String, bool) {
        let mut s = String::new();
        let mut cur: Vec<u8> = Vec::with_capacity(16);
        for &b in bits {
            cur.push(b & 1);
            if let Some(e) = self.entries.iter().find(|e| e.1 == cur) {
                match e.0 {
                    b'\r' => s.push('\n'),
                    b'\n' => {
                        if !s.ends_with('\n') {
                            s.push('\n')
                        }
                    }
                    c => s.push(c as char),
                }
                cur.clear();
                continue;
            }
            let prefix = self
                .entries
                .iter()
                .any(|e| e.1.len() > cur.len() && e.1[..cur.len()] == cur[..]);
            if !prefix || cur.len() > 15 {
                s.push('~');
                return (s, true);
            }
        }
        (s, false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn u1_crc_and_status() {
        assert_eq!(packet_crc(b"123456789"), 0x29B1);
        for b in 0..32u8 {
            assert_eq!(Status::from_byte(b).byte(), b);
        }
    }

    #[test]
    fn u2_packets() {
        for &(n, h) in &[(8usize, 0x55u8), (20, 0xAA)] {
            let data: Vec<u8> = (0..n as u8).map(|i| i.wrapping_mul(37)).collect();
            let st = Status {
                number: 2,
                huffman: true,
                ..Default::default()
            };
            let bits = packet_bits(h, &data, st);
            assert_eq!(bits.len(), (n + 4) * 8);
            let p = read_packet(&bits);
            assert!(packet_ok(&p));
            assert_eq!((p.header, p.data.clone(), p.status), (h, data.clone(), st));
            let mut b = bits.clone();
            b[8 + 3] ^= 1;
            assert!(!read_packet(&b).crc_ok);
            let mut b = bits.clone();
            b[2] ^= 1;
            let p = read_packet(&b);
            assert!(packet_ok(&p) && p.header_errors == 1);
            for i in 8..bits.len() {
                let mut b = bits.clone();
                b[i] ^= 1;
                assert!(!read_packet(&b).crc_ok, "bit {i}");
            }
        }
    }

    #[test]
    fn a_complemented_crc_is_refused() {
        let mut bits = packet_bits(0x55, &[1, 2, 3, 4, 5, 6, 7, 8], Status::default());
        assert!(read_packet(&bits).crc_ok);
        let n = bits.len();
        for b in &mut bits[n - 16..] {
            *b ^= 1;
        }
        assert!(!read_packet(&bits).crc_ok);
    }

    #[test]
    fn u3_control_signals() {
        assert_eq!(CS, [0x4D5, 0xAB2, 0x34B, 0xD2C]);
        for (i, &cs) in CS.iter().enumerate() {
            let b = cs_bits(i);
            assert_eq!(cs_match(&b), (i, 0));
            let mut c = b;
            c[3] ^= 1;
            assert_eq!(cs_match(&c), (i, 1));
            c[7] ^= 1;
            assert_eq!(cs_match(&c), (i, 2));
            for (j, &other) in CS.iter().enumerate() {
                if i != j {
                    assert!((cs ^ other).count_ones() >= 6);
                }
            }
        }
        assert_eq!(cs_bits(0), [0, 1, 0, 0, 1, 1, 0, 1, 0, 1, 0, 1]);
    }

    #[test]
    fn u4_callsigns() {
        assert_eq!(
            connect_call(&connect_data("XX1AA")).as_deref(),
            Some("XX1AA")
        );
        assert!(!is_callsign("HELLOWOR"));
        assert!(is_callsign("XX1AA") && is_callsign("XX2BB"));
        assert_eq!(
            connect_data("AB1CD"),
            vec![b'A', b'B', b'1', b'C', b'D', 0x0F, 0x0F, 0x0F]
        );
    }

    #[test]
    fn u5_huffman() {
        let h = Huffman::default();
        assert!(h.entries.len() >= 7 && !h.complete());
        let b = h.encode("tea set at sea").unwrap();
        assert_eq!(h.decode(&b), ("tea set at sea".to_string(), false));
        assert!(h.encode("xyz").is_none());
        assert_eq!(h.decode(&[1; 8]), ("~".to_string(), true));
        let mut file = String::from(
            "# test table\n \t10\ne\t011\nt\t00000\ns\t00100\na\t01000\n13\t001100\n10\t001101\n",
        );
        file.push_str("n\t110\no\t111\n");
        let h9 = Huffman::parse(&file).unwrap();
        assert_eq!(h9.entries.len(), 9);
        let b = h9.encode("no tea").unwrap();
        assert_eq!(h9.decode(&b).0, "no tea");
        assert!(Huffman::parse("a\t01\n").is_none());
    }

    #[test]
    fn u6_bit_order() {
        let mut bits = Vec::new();
        bytes_to_bits(&[0x01, 0x80], &mut bits);
        assert_eq!(bits, vec![1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1]);
        assert_eq!(bits_to_bytes(&bits), vec![0x01, 0x80]);
    }
}
