//! The protocol data units of 3G-ALE (MIL-STD-188-141B Appendix C, sections C.4.12, C.5.2.2, C.5.3.4, C.5.4.4 and C.5.5.4): the
//! link-setup PDUs of BW0, the traffic-management and HDL PDUs of BW1, the HDL and LDL packets, the BW4 bits, the CRCs, the
//! LinkID and the sequence numbers. Bits are written first-sent first.

/// A CRC of `width` bits over `bits`: most significant bit first, register 0, no inversion; `mask` the polynomial's terms below
/// `x^width` (Section C.4.12).
pub fn crc(bits: &[u8], width: u32, mask: u32) -> u32 {
    let top = 1u32 << (width - 1);
    let all = if width == 32 {
        u32::MAX
    } else {
        (1 << width) - 1
    };
    let mut reg = 0u32;
    for &b in bits {
        let fb = ((reg & top) != 0) ^ (b & 1 == 1);
        reg = (reg << 1) & all;
        if fb {
            reg ^= mask;
        }
    }
    reg
}

/// CRC-4 of the LE_Call and LE_Notification PDUs.
pub const CRC4: (u32, u32) = (4, 0xB);
/// CRC-8 of the other BW0 PDUs.
pub const CRC8: (u32, u32) = (8, 0x9B);
/// CRC-12 of the TM PDU.
pub const CRC12: (u32, u32) = (12, 0xBCF);
/// CRC-16 of the HDL_ACK PDU.
pub const CRC16: (u32, u32) = (16, 0x897B);
/// CRC-32 of the HDL and LDL packets.
pub const CRC32: (u32, u32) = (32, 0x04C1_1DB7);

/// The low `n` bits of `v`, most significant first.
pub fn bits(v: u64, n: usize) -> Vec<u8> {
    (0..n).rev().map(|i| ((v >> i) & 1) as u8).collect()
}

/// The number of `b` (most significant first).
pub fn num(b: &[u8]) -> u64 {
    b.iter().fold(0, |a, &x| (a << 1) | u64::from(x & 1))
}

/// The LinkID of a caller and a responder (11-bit addresses), Section C.5.2.2.2.
pub fn link_id(caller: u32, responder: u32) -> u8 {
    let k = 0x13_C6EFu32;
    let a = caller.wrapping_mul(k) >> 4;
    let b = responder.wrapping_mul(k) >> 15;
    (a.wrapping_add(b) & 0x3F) as u8
}

/// An 11-bit address of a member (6 bits) and a group (5 bits).
pub fn address(member: u32, group: u32) -> u32 {
    ((member & 0x3F) << 5) | (group & 0x1F)
}

/// "member 17 of group 3" for an 11-bit address, or "all" for 7FF.
pub fn address_name(a: u32) -> String {
    if a == 0x7FF {
        "all".into()
    } else {
        format!("member {} of group {}", a >> 5, a & 0x1F)
    }
}

/// The call types (Table C-XXII).
pub const CALL_TYPES: [&str; 8] = [
    "3G ARQ packet data",
    "HF modem circuit",
    "analog voice circuit",
    "high-quality circuit",
    "unicast ARQ packet",
    "unicast circuit",
    "multicast circuit",
    "control",
];
/// The handshake commands (Table C-XXIII); code 5 is reserved.
pub const COMMANDS: [&str; 8] = [
    "continue handshake",
    "commence traffic setup",
    "voice traffic",
    "link release",
    "sync check",
    "reserved command 5",
    "data",
    "abort handshake",
];
/// The handshake reasons (Table C-XXIV).
pub const REASONS: [&str; 4] = [
    "no response",
    "rejected",
    "no traffic channel",
    "low quality",
];

/// The notification status (Table C-XXV): only 0, 1, 6 and 7 are defined.
pub fn status_name(s: u32) -> String {
    match s {
        0 => "nominal".into(),
        1 => "time server".into(),
        6 => "commencing EMCON".into(),
        7 => "leaving network".into(),
        x => format!("undefined status {x}"),
    }
}

/// The traffic-management arguments (Table C-XXXVII).
pub fn tm_argument(a: u32) -> String {
    match a {
        0 => "HDL 24 packets".into(),
        1 => "HDL 12 packets".into(),
        2 => "HDL 6 packets".into(),
        3 => "HDL 3 packets".into(),
        4 => "LDL 512 bytes".into(),
        5 => "LDL 256 bytes".into(),
        6 => "LDL 128 bytes".into(),
        7 => "LDL 64 bytes".into(),
        8 => "ANDVT voice".into(),
        9 => "digital voice".into(),
        10 => "analog voice".into(),
        11 => "MIL-STD-188-110 serial".into(),
        12 => "high quality 9600".into(),
        13 => "high quality 6400".into(),
        14 => "high quality 4800".into(),
        15 => "high quality 3200".into(),
        16 => "STANAG 4285".into(),
        61 => "packet".into(),
        62 => "circuit".into(),
        63 => "no traffic".into(),
        x => format!("reserved argument {x}"),
    }
}

/// The TM_TERM reasons.
pub const TERM_REASONS: [&str; 7] = [
    "abort",
    "relink",
    "sign off",
    "unlink",
    "unlink acknowledged",
    "unlink, not all received",
    "suspend",
];

/// The priorities of a TM PDU.
pub const PRIORITIES: [&str; 4] = ["highest", "high", "routine", "low"];

/// A PDU read: its type, whether its check passed, and its content in words.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Reading {
    /// The PDU type ("LE_Call").
    pub kind: &'static str,
    /// The check passed.
    pub good: bool,
    /// The content in words.
    pub text: String,
}

fn with_crc(mut b: Vec<u8>, c: (u32, u32)) -> Vec<u8> {
    let v = crc(&b, c.0, c.1);
    b.extend(bits(u64::from(v), c.0 as usize));
    b
}

fn crc_ok(b: &[u8], c: (u32, u32)) -> bool {
    let n = b.len() - c.0 as usize;
    crc(&b[..n], c.0, c.1) == num(&b[n..]) as u32
}

/// An LE_Call (Figure C-17): prefix 10, called member, call type, caller member, caller group, CRC-4.
pub fn le_call(called: u32, call_type: u32, member: u32, group: u32) -> Vec<u8> {
    let mut b = vec![1, 0];
    b.extend(bits(u64::from(called), 6));
    b.extend(bits(u64::from(call_type), 3));
    b.extend(bits(u64::from(member), 6));
    b.extend(bits(u64::from(group), 5));
    with_crc(b, CRC4)
}

/// An LE_Notification: prefix 10, called member 63, status, caller member, caller group, CRC-4.
pub fn le_notification(status: u32, member: u32, group: u32) -> Vec<u8> {
    le_call(63, status, member, group)
}

/// An LE_Handshake: prefix 00, LinkID, command, argument, CRC-8.
pub fn le_handshake(link: u32, command: u32, argument: u32) -> Vec<u8> {
    let mut b = vec![0, 0];
    b.extend(bits(u64::from(link), 6));
    b.extend(bits(u64::from(command), 3));
    b.extend(bits(u64::from(argument), 7));
    with_crc(b, CRC8)
}

/// An LE_Broadcast: prefix 01110, countdown, call type, channel, CRC-8.
pub fn le_broadcast(countdown: u32, call_type: u32, channel: u32) -> Vec<u8> {
    let mut b = vec![0, 1, 1, 1, 0];
    b.extend(bits(u64::from(countdown), 3));
    b.extend(bits(u64::from(call_type), 3));
    b.extend(bits(u64::from(channel), 7));
    with_crc(b, CRC8)
}

/// Reads a BW0 PDU of 26 bits.
pub fn read_bw0(b: &[u8]) -> Reading {
    let f = |a: usize, n: usize| num(&b[a..a + n]) as u32;
    match (b[0], b[1]) {
        (1, 0) => {
            let good = crc_ok(b, CRC4);
            let called = f(2, 6);
            let (m, g) = (f(11, 6), f(17, 5));
            if called == 63 {
                Reading {
                    kind: "LE_Notification",
                    good,
                    text: format!("member {m} of group {g}: {}", status_name(f(8, 3))),
                }
            } else {
                Reading {
                    kind: "LE_Call",
                    good,
                    text: format!(
                        "member {m} of group {g} calls member {called}: {}",
                        CALL_TYPES[f(8, 3) as usize]
                    ),
                }
            }
        }
        (0, 0) => {
            let cmd = f(8, 3);
            let arg = f(11, 7);
            let what = match cmd {
                0 | 7 => format!(
                    "{}, {}",
                    COMMANDS[cmd as usize],
                    REASONS
                        .get(arg as usize)
                        .copied()
                        .unwrap_or("reason unknown")
                ),
                1..=3 => format!("{}, channel {arg}", COMMANDS[cmd as usize]),
                _ => format!("{}, argument {arg}", COMMANDS[cmd as usize]),
            };
            Reading {
                kind: "LE_Handshake",
                good: crc_ok(b, CRC8),
                text: format!("link {:02X}: {what}", f(2, 6)),
            }
        }
        (0, 1) => {
            let good = crc_ok(b, CRC8);
            match f(2, 3) {
                0b110 => Reading {
                    kind: "LE_Broadcast",
                    good,
                    text: format!(
                        "countdown {}, {}, channel {}",
                        f(5, 3),
                        CALL_TYPES[f(8, 3) as usize],
                        f(11, 7)
                    ),
                },
                0b100 => Reading {
                    kind: "LE_TimeOffset",
                    good,
                    text: format!(
                        "time quality {}, offset {}{}",
                        f(5, 3),
                        if b[8] == 1 { "-" } else { "+" },
                        f(9, 9)
                    ),
                },
                0b101 => Reading {
                    kind: "LE_GroupTimeBroadcast",
                    good,
                    text: format!(
                        "server group {}, dwell {}, slot {}",
                        f(6, 5),
                        f(11, 4),
                        f(15, 3)
                    ),
                },
                0b111 if f(5, 2) == 3 => Reading {
                    kind: "LE_ScanningCall",
                    good,
                    text: format!("to {}", address_name(f(7, 11))),
                },
                _ => Reading {
                    kind: "3G-ALE PDU",
                    good,
                    text: format!("{:07X}", num(b)),
                },
            }
        }
        _ => Reading {
            kind: "3G-ALE PDU",
            good: false,
            text: format!("{:07X}", num(b)),
        },
    }
}

/// A TM PDU (Table C-XXXVI): 001, priority, destination, source, type, argument, CRC-12.
pub fn tm(priority: u32, destination: u32, source: u32, kind: u32, argument: u32) -> Vec<u8> {
    let mut b = vec![0, 0, 1];
    b.extend(bits(u64::from(priority), 2));
    b.extend(bits(u64::from(destination), 11));
    b.extend(bits(u64::from(source), 11));
    b.extend(bits(u64::from(kind), 3));
    b.extend(bits(u64::from(argument), 6));
    with_crc(b, CRC12)
}

/// An HDL_ACK: 000, 0, the 24-bit acknowledgement mask (packet 1 first), 4 zero bits, CRC-16 over type and mask.
pub fn hdl_ack(mask: u32) -> Vec<u8> {
    let mut b = vec![0, 0, 0, 0];
    b.extend(bits(u64::from(mask), 24));
    let c = crc(&b[3..28], CRC16.0, CRC16.1);
    b.extend([0, 0, 0, 0]);
    b.extend(bits(u64::from(c), 16));
    b
}

/// The HDL_EOM pattern.
pub const HDL_EOM: u64 = 0xA5A_5A5A_5A5A;

/// An HDL_EOM: 000, 1, the 44-bit pattern.
pub fn hdl_eom() -> Vec<u8> {
    let mut b = vec![0, 0, 0, 1];
    b.extend(bits(HDL_EOM, 44));
    b
}

/// Reads a BW1 PDU of 48 bits.
pub fn read_bw1(b: &[u8]) -> Reading {
    let f = |a: usize, n: usize| num(&b[a..a + n]) as u32;
    match f(0, 3) {
        1 => {
            let kind = match f(27, 3) {
                0 => "TM_REQUEST",
                1 => "TM_CONFIRM",
                2 => "TM_TERM",
                _ => "TM",
            };
            let arg = f(30, 6);
            let what = if kind == "TM_TERM" {
                TERM_REASONS
                    .get(arg as usize)
                    .map_or_else(|| format!("reason {arg}"), |s| (*s).to_string())
            } else {
                tm_argument(arg)
            };
            Reading {
                kind,
                good: crc_ok(b, CRC12),
                text: format!(
                    "{} to {}, {} priority: {what}",
                    address_name(f(16, 11)),
                    address_name(f(5, 11)),
                    PRIORITIES[f(3, 2) as usize]
                ),
            }
        }
        0 if b[3] == 0 => {
            let good = f(28, 4) == 0 && crc(&b[3..28], CRC16.0, CRC16.1) == f(32, 16);
            let mask = f(4, 24);
            let acked: Vec<String> = (0..24)
                .filter(|i| (mask >> (23 - i)) & 1 == 1)
                .map(|i| (i + 1).to_string())
                .collect();
            Reading {
                kind: "HDL_ACK",
                good,
                text: format!("packets received: {}", acked.join(" ")),
            }
        }
        0 => Reading {
            kind: "HDL_EOM",
            good: num(&b[4..]) == HDL_EOM,
            text: "end of the HDL datagram".into(),
        },
        _ => Reading {
            kind: "3G-ALE PDU",
            good: false,
            text: format!("{:012X}", num(b)),
        },
    }
}

/// A sequence number (Tables C-XLVI and C-LVI): bit 16 the end of the message, bit 15 its start, then the value.
pub fn sequence(start: bool, end: bool, value: u32) -> u32 {
    (u32::from(end) << 16) | (u32::from(start) << 15) | (value & 0x7FFF)
}

/// An HDL packet of 1913 bits: 233 data bytes (most significant bit first), the 17-bit sequence number, CRC-32.
pub fn hdl_packet(data: &[u8], seq: u32) -> Vec<u8> {
    let mut b: Vec<u8> = (0..233)
        .flat_map(|i| bits(u64::from(data.get(i).copied().unwrap_or(0)), 8))
        .collect();
    b.extend(bits(u64::from(seq), 17));
    with_crc(b, CRC32)
}

/// An LDL packet of `8 n + 57` bits: `n` data bytes, the sequence number, 8 zero control bits, CRC-32.
pub fn ldl_packet(data: &[u8], n: usize, seq: u32) -> Vec<u8> {
    let mut b: Vec<u8> = (0..n)
        .flat_map(|i| bits(u64::from(data.get(i).copied().unwrap_or(0)), 8))
        .collect();
    b.extend(bits(u64::from(seq), 17));
    b.extend([0u8; 8]);
    with_crc(b, CRC32)
}

/// A packet read: its bytes, its sequence number, whether its CRC-32 holds.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Packet {
    /// The data bytes.
    pub data: Vec<u8>,
    /// The sequence number.
    pub seq: u32,
    /// The CRC holds.
    pub good: bool,
}

/// Reads a packet of `n` data bytes and `extra` bits after the sequence number (HDL 0, LDL 8).
pub fn read_packet(b: &[u8], n: usize, extra: usize) -> Packet {
    let data = b[..8 * n].chunks(8).map(|c| num(c) as u8).collect();
    let seq = num(&b[8 * n..8 * n + 17]) as u32;
    let body = 8 * n + 17 + extra;
    Packet {
        data,
        seq,
        good: crc(&b[..body], 32, CRC32.1) == num(&b[body..body + 32]) as u32,
    }
}

/// The BW4 words: `b1 b0`.
pub fn bw4_name(w: usize) -> &'static str {
    match w & 3 {
        0 => "NAK, datagram not complete",
        1 => "NAK, datagram complete",
        2 => "ACK, datagram not complete",
        _ => "ACK, datagram complete (or LDL_EOM)",
    }
}

/// The bytes shown: as text in quotes when 90 % are printable, else hex, at most 64 bytes.
pub fn show_data(d: &[u8]) -> String {
    let printable = d
        .iter()
        .filter(|&&c| (0x20..0x7F).contains(&c) || c == b'\n' || c == b'\r')
        .count();
    let cut = &d[..d.len().min(64)];
    let more = if d.len() > 64 { "..." } else { "" };
    if !d.is_empty() && printable * 10 >= d.len() * 9 {
        let s: String = cut
            .iter()
            .map(|&c| {
                if (0x20..0x7F).contains(&c) {
                    c as char
                } else {
                    '.'
                }
            })
            .collect();
        format!("\"{s}\"{more}")
    } else {
        format!("{}{more}", crate::serial::board::hex(cut))
    }
}

/// A datagram being rebuilt from its packets.
#[derive(Clone, Debug, Default)]
pub struct Datagram {
    parts: Vec<Option<Vec<u8>>>,
}

impl Datagram {
    /// Takes a packet; returns the whole datagram when this packet completes it.
    pub fn push(&mut self, p: &Packet, low_bits: u32) -> Option<Vec<u8>> {
        let start = (p.seq >> 15) & 1 == 1;
        let end = (p.seq >> 16) & 1 == 1;
        let v = p.seq & 0x7FFF;
        let count_mask = (1 << low_bits) - 1;
        if start && end {
            self.parts.clear();
            let n = (v & count_mask) as usize;
            return Some(p.data[..n.min(p.data.len())].to_vec());
        }
        if start {
            self.parts = vec![None; v as usize + 1];
            self.parts[0] = Some(p.data.clone());
            return None;
        }
        if self.parts.is_empty() {
            return None;
        }
        let total = self.parts.len();
        if end {
            let n = (v & count_mask) as usize;
            self.parts[total - 1] = Some(p.data[..n.min(p.data.len())].to_vec());
        } else if (v as usize) < total {
            self.parts[total - 1 - v as usize] = Some(p.data.clone());
        }
        if self.parts.iter().all(Option::is_some) {
            let d = self.parts.iter().flatten().flatten().copied().collect();
            self.parts.clear();
            return Some(d);
        }
        None
    }

    /// The packets the datagram has.
    pub fn parts(&self) -> usize {
        self.parts.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crcs() {
        assert_eq!(crc(&[1], 4, 0xB), 0xB);
        assert_eq!(crc(&[1], 8, 0x9B), 0x9B);
        let b: Vec<u8> = b"123456789"
            .iter()
            .flat_map(|&c| bits(u64::from(c), 8))
            .collect();
        assert_eq!(crc(&b, 32, CRC32.1), 0x89A1_897F);
    }

    #[test]
    fn link_ids() {
        for (c, r, id) in [
            (1, 2, 0x3D),
            (1, 3, 0x24),
            (2, 1, 0x04),
            (3, 1, 0x33),
            (1951, 1, 0x1E),
            (547, 163, 0x39),
        ] {
            assert_eq!(link_id(c, r), id, "{c} {r}");
        }
    }

    #[test]
    fn worked_pdus() {
        let call = le_call(5, 2, 17, 3);
        assert_eq!(num(&call), 0x215_2230);
        let r = read_bw0(&call);
        assert_eq!((r.kind, r.good), ("LE_Call", true));
        let hs = le_handshake(0x3D, 1, 42);
        assert_eq!(num(&hs), 0x0F4_AAE0);
        assert_eq!(read_bw0(&hs).kind, "LE_Handshake");
        let mut bad = hs.clone();
        bad[9] ^= 1;
        assert!(!read_bw0(&bad).good);
        let bc = le_broadcast(2, 0, 77);
        assert_eq!(&bc[..5], &[0, 1, 1, 1, 0]);
        assert_eq!(read_bw0(&bc).kind, "LE_Broadcast");
        let n = read_bw0(&le_notification(7, 30, 12));
        assert_eq!((n.kind, n.good), ("LE_Notification", true));
        assert!(n.text.contains("leaving network"));
        let t = read_bw1(&tm(2, 163, 547, 0, 7));
        assert_eq!((t.kind, t.good), ("TM_REQUEST", true));
        assert!(t.text.contains("LDL 64 bytes"));
        assert!(read_bw1(&hdl_ack(0x80_0001)).good);
        assert_eq!(read_bw1(&hdl_eom()).kind, "HDL_EOM");
    }

    #[test]
    fn packets_and_datagrams() {
        let p = ldl_packet(b"ABC", 64, sequence(true, true, 3));
        assert_eq!(p.len(), 8 * 64 + 57);
        let r = read_packet(&p, 64, 8);
        assert!(r.good);
        let mut d = Datagram::default();
        assert_eq!(d.push(&r, 10), Some(b"ABC".to_vec()));
        let h: Vec<Packet> = [
            (sequence(true, false, 2), b"AA".as_slice()),
            (sequence(false, false, 1), b"BB"),
            (sequence(false, true, 2), b"CC"),
        ]
        .iter()
        .map(|(s, data)| read_packet(&hdl_packet(data, *s), 233, 0))
        .collect();
        assert_eq!(hdl_packet(b"", 0).len(), 1913);
        let mut d = Datagram::default();
        assert!(d.push(&h[0], 9).is_none());
        assert!(d.push(&h[1], 9).is_none());
        let all = d.push(&h[2], 9).unwrap();
        assert_eq!(&all[..2], b"AA");
        assert_eq!(&all[233..235], b"BB");
        assert_eq!(&all[466..], b"CC");
    }
}
