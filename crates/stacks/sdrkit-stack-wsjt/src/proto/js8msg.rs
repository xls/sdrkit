//! JS8 frames: the 72 frame bits (heartbeat, compound, directed and data frames), their 12-character form, the callsign, group,
//! grid and command packing, and the Huffman text code.
//! Standard: the JS8Call protocol documentation.

use super::msg72::{pack_call, unpack_call, NBASE};
use super::msg77::PackError;

/// The 64 characters of the raw frame text.
pub const CHARS: &str = "0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz-+";
/// Flag: the first frame of a transmission.
pub const FLAG_FIRST: u8 = 1;
/// Flag: the last frame of a transmission.
pub const FLAG_LAST: u8 = 2;
/// Flag: fast data (JSC compressed).
pub const FLAG_FAST: u8 = 4;

/// The groups, as fixed call values after the 28-bit call base.
pub const GROUPS: [&str; 64] = [
    "<....>",
    "@ALLCALL",
    "@JS8NET",
    "@DX/NA",
    "@DX/SA",
    "@DX/EU",
    "@DX/AS",
    "@DX/AF",
    "@DX/OC",
    "@DX/AN",
    "@REGION/1",
    "@REGION/2",
    "@REGION/3",
    "@GROUP/0",
    "@GROUP/1",
    "@GROUP/2",
    "@GROUP/3",
    "@GROUP/4",
    "@GROUP/5",
    "@GROUP/6",
    "@GROUP/7",
    "@GROUP/8",
    "@GROUP/9",
    "@COMMAND",
    "@CONTROL",
    "@NET",
    "@NTS",
    "@RESERVE/0",
    "@RESERVE/1",
    "@RESERVE/2",
    "@RESERVE/3",
    "@RESERVE/4",
    "@APRSIS",
    "@RAGCHEW",
    "@JS8",
    "@EMCOMM",
    "@ARES",
    "@MARS",
    "@AMRRON",
    "@RACES",
    "@RAYNET",
    "@RADAR",
    "@SKYWARN",
    "@CQ",
    "@HB",
    "@QSO",
    "@QSOPARTY",
    "@CONTEST",
    "@FIELDDAY",
    "@SOTA",
    "@IOTA",
    "@POTA",
    "@QRP",
    "@QRO",
    "",
    "",
    "",
    "",
    "",
    "",
    "",
    "",
    "",
    "",
];

/// The 32 directed commands, by code.
pub const COMMANDS: [&str; 32] = [
    " SNR?",
    " DIT DIT",
    " NACK",
    " HEARING?",
    " GRID?",
    ">",
    " STATUS?",
    " STATUS",
    " HEARING",
    " MSG",
    " MSG TO:",
    " QUERY",
    " QUERY MSGS",
    " QUERY CALL",
    " ACK",
    " GRID",
    " INFO?",
    " INFO",
    " FB",
    " HW CPY?",
    " SK",
    " RR",
    " QSL?",
    " QSL",
    " CMD",
    " SNR",
    " NO",
    " YES",
    " 73",
    " HEARTBEAT SNR",
    " AGN?",
    " ",
];

/// The CQ choices of a heartbeat frame.
pub const CQS: [&str; 8] = [
    "CQ CQ CQ",
    "CQ DX",
    "CQ QRP",
    "CQ CONTEST",
    "CQ FIELD",
    "CQ FD",
    "CQ CQ",
    "CQ",
];

/// The Huffman code of JS8 free text.
pub const HUFFMAN: [(char, &str); 44] = [
    (' ', "01"),
    ('E', "100"),
    ('T', "1101"),
    ('A', "0011"),
    ('O', "11111"),
    ('I', "11100"),
    ('N', "10111"),
    ('S', "10100"),
    ('H', "00011"),
    ('R', "00000"),
    ('D', "111011"),
    ('L', "110011"),
    ('C', "110001"),
    ('U', "101101"),
    ('M', "101011"),
    ('W', "001011"),
    ('F', "001001"),
    ('G', "000101"),
    ('Y', "000011"),
    ('P', "1111011"),
    ('B', "1111001"),
    ('.', "1110100"),
    ('V', "1100101"),
    ('K', "1100100"),
    ('-', "1100001"),
    ('+', "1100000"),
    ('?', "1011001"),
    ('!', "1011000"),
    ('"', "1010101"),
    ('X', "1010100"),
    ('0', "0010101"),
    ('J', "0010100"),
    ('1', "0010001"),
    ('Q', "0010000"),
    ('2', "0001001"),
    ('Z', "0001000"),
    ('3', "0000101"),
    ('5', "0000100"),
    ('4', "11110101"),
    ('9', "11110100"),
    ('8', "11110001"),
    ('6', "11110000"),
    ('7', "11101011"),
    ('/', "11101010"),
];

const A38: &str = "0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZ /";

/// The 12 characters of 72 frame bits.
pub fn bits_to_chars(bits: &[u8]) -> String {
    bits.chunks(6)
        .take(12)
        .map(|c| CHARS.as_bytes()[c.iter().fold(0usize, |a, &b| (a << 1) | b as usize)] as char)
        .collect()
}

/// The 72 frame bits of 12 characters.
pub fn chars_to_bits(s: &str) -> Option<Vec<u8>> {
    if s.len() != 12 {
        return None;
    }
    let mut out = Vec::with_capacity(72);
    for c in s.chars() {
        let v = CHARS.find(c)?;
        for b in (0..6).rev() {
            out.push(((v >> b) & 1) as u8);
        }
    }
    Some(out)
}

use sdrkit_util_bits::crc::cyclic::JS8_CRC12;
use sdrkit_util_bits::pack::{push_u64 as put, to_u64 as get};

/// A 28-bit call or group value; the `/P` flag is returned separately.
pub fn pack_call28(call: &str) -> Option<(u32, bool)> {
    if let Some(i) = GROUPS.iter().position(|g| !g.is_empty() && *g == call) {
        return Some((NBASE + 1 + i as u32, false));
    }
    let (base, p) = match call.strip_suffix("/P") {
        Some(b) => (b, true),
        None => (call, false),
    };
    let v = pack_call(base)?;
    (v < NBASE).then_some((v, p))
}

/// The text of a 28-bit call or group value.
pub fn unpack_call28(v: u32, portable: bool) -> Option<String> {
    if v > NBASE {
        let i = (v - NBASE - 1) as usize;
        return GROUPS
            .get(i)
            .filter(|g| !g.is_empty())
            .map(|g| g.to_string());
    }
    let c = unpack_call(v)?;
    Some(if portable { format!("{c}/P") } else { c })
}

/// A call of up to 11 characters (compound calls) in 50 bits.
pub fn pack_call50(call: &str) -> Option<u64> {
    let mut w: Vec<u8> = call
        .bytes()
        .filter(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || *c == b'/')
        .collect();
    if w.is_empty() || w.len() > 11 {
        return None;
    }
    if w.len() > 3 && w[3] != b'/' {
        w.insert(3, b' ');
    }
    if w.len() > 7 && w[7] != b'/' {
        w.insert(7, b' ');
    }
    if w.len() > 11 {
        return None;
    }
    w.resize(11, b' ');
    let mut v: u64 = 0;
    for (i, &c) in w.iter().enumerate() {
        if i == 3 || i == 7 {
            v = v * 2 + (c == b'/') as u64;
        } else {
            v = v * 38 + A38.find(c as char)? as u64;
        }
    }
    Some(v)
}

/// The call of 50 bits.
pub fn unpack_call50(v: u64) -> Option<String> {
    let mut v = v;
    let mut s = [b' '; 11];
    for i in (0..11).rev() {
        if i == 3 || i == 7 {
            s[i] = if v % 2 == 1 { b'/' } else { b' ' };
            v /= 2;
        } else {
            s[i] = A38.as_bytes()[(v % 38) as usize];
            v /= 38;
        }
    }
    if v != 0 {
        return None;
    }
    let t: String = String::from_utf8_lossy(&s)
        .chars()
        .filter(|&c| c != ' ')
        .collect();
    (!t.is_empty()).then_some(t)
}

/// A 4-character locator as 15 bits (32767: none), packed as the 72-bit message packs it.
pub fn pack_grid(g: &str) -> u16 {
    g.get(..4)
        .and_then(super::msg72::grid_value)
        .map_or(32767, |v| v as u16)
}

/// The locator of 15 bits.
pub fn unpack_grid(v: u16) -> Option<String> {
    super::msg72::grid_text(v as u32)
}

/// Huffman-codes `text`; `None` when a character has no code.
pub fn huff_encode(text: &str) -> Option<Vec<u8>> {
    let mut out = Vec::new();
    for c in text.chars() {
        let code = HUFFMAN
            .iter()
            .find(|(h, _)| *h == c.to_ascii_uppercase())?
            .1;
        out.extend(code.bytes().map(|b| b - b'0'));
    }
    Some(out)
}

/// Decodes Huffman bits (stops at an incomplete code).
pub fn huff_decode(bits: &[u8]) -> String {
    let mut out = String::new();
    let mut cur = String::new();
    for &b in bits {
        cur.push(if b == 1 { '1' } else { '0' });
        if let Some((c, _)) = HUFFMAN.iter().find(|(_, code)| *code == cur) {
            out.push(*c);
            cur.clear();
        }
    }
    out
}

/// A JS8 frame, as packed and unpacked.
#[derive(Clone, Debug, PartialEq)]
pub enum Frame {
    /// A heartbeat or a CQ.
    Heartbeat {
        /// The call.
        call: String,
        /// The 4-character locator (empty: none).
        grid: String,
        /// A CQ (else a heartbeat).
        cq: bool,
        /// The CQ choice (0..7) or heartbeat kind.
        choice: u8,
    },
    /// A compound call with a grid or a command (`directed`: the first frame of a directed message from a compound call).
    Compound {
        /// The call.
        call: String,
        /// The locator, or empty.
        grid: String,
        /// The command code, when a command.
        cmd: Option<u8>,
        /// The number of the command.
        num: i32,
        /// A compound directed frame.
        directed: bool,
    },
    /// A directed command.
    Directed {
        /// From.
        from: String,
        /// To (a call or a group).
        to: String,
        /// The command code (0..31).
        cmd: u8,
        /// Its number (an SNR for the SNR commands).
        num: i32,
    },
    /// Free text.
    Data {
        /// The text.
        text: String,
        /// JSC-compressed (not decoded).
        compressed: bool,
    },
}

fn is_snr_cmd(cmd: u8) -> bool {
    COMMANDS[cmd as usize] == " SNR" || COMMANDS[cmd as usize] == " HEARTBEAT SNR"
}

impl Frame {
    /// The 72 frame bits.
    pub fn pack(&self) -> Result<Vec<u8>, PackError> {
        let mut b = Vec::with_capacity(72);
        match self {
            Frame::Heartbeat {
                call,
                grid,
                cq,
                choice,
            } => {
                let c = pack_call50(call).ok_or(PackError::Heartbeat)?;
                let g = pack_grid(grid);
                if !grid.is_empty() && g == 32767 {
                    return Err(PackError::Heartbeat);
                }
                put(&mut b, 0, 3);
                put(&mut b, c, 50);
                put(&mut b, *cq as u64, 1);
                put(&mut b, g as u64, 15);
                put(&mut b, (*choice & 7) as u64, 3);
            }
            Frame::Compound {
                call,
                grid,
                cmd,
                num,
                directed,
            } => {
                let c = pack_call50(call).ok_or(PackError::NotJs8)?;
                put(&mut b, if *directed { 2 } else { 1 }, 3);
                put(&mut b, c, 50);
                let v: u64 = match cmd {
                    Some(k) => {
                        (1 << 15) | ((*k as u64 & 31) << 8) | ((*num + 31).clamp(0, 255) as u64)
                    }
                    None => pack_grid(grid) as u64,
                };
                put(&mut b, v, 16);
                put(&mut b, 0, 3);
            }
            Frame::Directed { from, to, cmd, num } => {
                let (f, pf) = pack_call28(from).ok_or(PackError::NotJs8)?;
                let (t, pt) = pack_call28(to).ok_or(PackError::NotJs8)?;
                if *cmd > 31 {
                    return Err(PackError::UnknownCommand);
                }
                put(&mut b, 3, 3);
                put(&mut b, f as u64, 28);
                put(&mut b, t as u64, 28);
                put(&mut b, *cmd as u64, 5);
                let v = if is_snr_cmd(*cmd) {
                    (*num + 31).clamp(0, 63) as u64
                } else {
                    (*num).clamp(0, 63) as u64
                };
                put(&mut b, ((pf as u64) << 7) | ((pt as u64) << 6) | v, 8);
            }
            Frame::Data { text, compressed } => {
                let h = huff_encode(text).ok_or(PackError::NotJs8)?;
                if h.len() > 69 {
                    return Err(PackError::FrameFull);
                }
                b.push(1);
                b.push(*compressed as u8);
                b.extend(h);
                b.push(0);
                while b.len() < 72 {
                    b.push(1);
                }
            }
        }
        debug_assert_eq!(b.len(), 72);
        Ok(b)
    }

    /// Unpacks 72 frame bits.
    pub fn unpack(b: &[u8]) -> Option<Frame> {
        if b.len() < 72 {
            return None;
        }
        if b[0] == 1 {
            let compressed = b[1] == 1;
            // Strip the padding: trailing ones, then the zero.
            let mut end = 72;
            while end > 2 && b[end - 1] == 1 {
                end -= 1;
            }
            if end <= 2 {
                return None;
            }
            end -= 1;
            let text = if compressed {
                "[compressed]".to_string()
            } else {
                huff_decode(&b[2..end])
            };
            return Some(Frame::Data { text, compressed });
        }
        match get(&b[0..3]) {
            0 => {
                let call = unpack_call50(get(&b[3..53]))?;
                let cq = b[53] == 1;
                let g = get(&b[54..69]) as u16;
                let grid = if g == 32767 {
                    String::new()
                } else {
                    unpack_grid(g)?
                };
                Some(Frame::Heartbeat {
                    call,
                    grid,
                    cq,
                    choice: get(&b[69..72]) as u8,
                })
            }
            k @ (1 | 2) => {
                let call = unpack_call50(get(&b[3..53]))?;
                let v = get(&b[53..69]);
                let (grid, cmd, num) = if v >> 15 == 1 {
                    (
                        String::new(),
                        Some(((v >> 8) & 31) as u8),
                        (v & 255) as i32 - 31,
                    )
                } else if v == 32767 {
                    (String::new(), None, 0)
                } else {
                    (unpack_grid(v as u16)?, None, 0)
                };
                Some(Frame::Compound {
                    call,
                    grid,
                    cmd,
                    num,
                    directed: k == 2,
                })
            }
            3 => {
                let flags = get(&b[64..72]) as u32;
                let from = unpack_call28(get(&b[3..31]) as u32, flags >> 7 & 1 == 1)?;
                let to = unpack_call28(get(&b[31..59]) as u32, flags >> 6 & 1 == 1)?;
                let cmd = get(&b[59..64]) as u8;
                let v = (flags & 63) as i32;
                let num = if is_snr_cmd(cmd) { v - 31 } else { v };
                Some(Frame::Directed { from, to, cmd, num })
            }
            _ => None,
        }
    }

    /// The display text.
    pub fn display(&self) -> String {
        match self {
            Frame::Heartbeat {
                call,
                grid,
                cq,
                choice,
            } => {
                let g = if grid.is_empty() {
                    String::new()
                } else {
                    format!(" {grid}")
                };
                if *cq {
                    format!("{call}: @ALLCALL {}{g}", CQS[(*choice & 7) as usize])
                } else {
                    format!("{call}: @HB HEARTBEAT{g}")
                }
            }
            Frame::Compound {
                call,
                grid,
                cmd,
                num,
                ..
            } => match cmd {
                Some(k) => {
                    let c = COMMANDS[*k as usize].trim();
                    if is_snr_cmd(*k) {
                        format!("{call}: {c} {}", snr_text(*num))
                    } else {
                        format!("{call}: {c}")
                    }
                }
                None if grid.is_empty() => format!("{call}:"),
                None => format!("{call}: {grid}"),
            },
            Frame::Directed { from, to, cmd, num } => {
                let c = COMMANDS[*cmd as usize];
                if is_snr_cmd(*cmd) {
                    format!("{from}: {to}{c} {}", snr_text(*num))
                } else {
                    format!("{from}: {to}{c}")
                }
            }
            Frame::Data { text, .. } => text.clone(),
        }
    }

    /// The sender (not a group, not a hash).
    pub fn from(&self) -> Option<&str> {
        let c = match self {
            Frame::Heartbeat { call, .. } | Frame::Compound { call, .. } => call.as_str(),
            Frame::Directed { from, .. } => from.as_str(),
            Frame::Data { .. } => return None,
        };
        (!c.starts_with('@') && !c.starts_with('<')).then_some(c)
    }
}

/// An SNR as JS8 writes it (`+05`, `-12`).
pub fn snr_text(v: i32) -> String {
    if v >= 0 {
        format!("+{v:02}")
    } else {
        format!("-{:02}", -v)
    }
}

/// The frame bits followed by the flags and the CRC-12: the 87 information bits of the code.
pub fn info87(frame72: &[u8], flags: u8) -> Vec<u8> {
    let mut v = frame72.to_vec();
    put(&mut v, flags as u64 & 7, 3);
    let crc = JS8_CRC12.compute(&v);
    put(&mut v, crc, 12);
    v
}

/// Checks 87 information bits; returns the frame bits and the flags.
pub fn check87(info: &[u8]) -> Option<(Vec<u8>, u8)> {
    (info.len() == 87 && JS8_CRC12.check(info))
        .then(|| (info[..72].to_vec(), get(&info[72..75]) as u8))
}

/// The command code of its text (` SNR?`, `?`, ` QUERY MSGS?` ...).
pub fn command_code(text: &str) -> Option<u8> {
    let t = if text == "?" {
        " SNR?"
    } else if text == " QUERY MSGS?" {
        " QUERY MSGS"
    } else {
        text
    };
    COMMANDS.iter().position(|c| *c == t).map(|i| i as u8)
}

/// What the sender sends.
#[derive(Clone, Debug, PartialEq)]
pub enum Outgoing {
    /// A heartbeat (`cq = None`) or a CQ (`Some(choice)`).
    Heartbeat {
        /// My call.
        call: String,
        /// My grid.
        grid: String,
        /// The CQ choice.
        cq: Option<u8>,
    },
    /// A directed command (with its number).
    Command {
        /// My call.
        from: String,
        /// The destination.
        to: String,
        /// The command text (` SNR`, ` GRID?`, `?` ...).
        cmd: String,
        /// Its number.
        num: i32,
    },
    /// A directed message of free text.
    Text {
        /// My call.
        from: String,
        /// The destination.
        to: String,
        /// The text.
        text: String,
    },
    /// A relay: to `via`, for `dest`.
    Relay {
        /// My call.
        from: String,
        /// The relaying station.
        via: String,
        /// The final destination.
        dest: String,
        /// The text.
        text: String,
    },
}

/// The frames of a message, with their flags.
pub fn frames_of(m: &Outgoing) -> Result<Vec<(Frame, u8)>, PackError> {
    let both = FLAG_FIRST | FLAG_LAST;
    let text_frames = |head: Frame, text: &str| -> Result<Vec<(Frame, u8)>, PackError> {
        head.pack()?;
        let mut out = vec![(head, FLAG_FIRST)];
        let mut chunk = String::new();
        let mut bits = 0usize;
        let mut chunks = Vec::new();
        for c in text.chars() {
            let n = huff_encode(&c.to_string()).ok_or(PackError::NotJs8)?.len();
            if bits + n > 69 {
                chunks.push(std::mem::take(&mut chunk));
                bits = 0;
            }
            chunk.push(c.to_ascii_uppercase());
            bits += n;
        }
        if !chunk.is_empty() {
            chunks.push(chunk);
        }
        for c in chunks {
            out.push((
                Frame::Data {
                    text: c,
                    compressed: false,
                },
                0,
            ));
        }
        if let Some(last) = out.last_mut() {
            last.1 |= FLAG_LAST;
        }
        Ok(out)
    };
    match m {
        Outgoing::Heartbeat { call, grid, cq } => {
            let f = Frame::Heartbeat {
                call: call.clone(),
                grid: grid.clone(),
                cq: cq.is_some(),
                choice: cq.unwrap_or(0),
            };
            f.pack()?;
            Ok(vec![(f, both)])
        }
        Outgoing::Command { from, to, cmd, num } => {
            let code = command_code(cmd).ok_or(PackError::UnknownCommand)?;
            let f = Frame::Directed {
                from: from.clone(),
                to: to.clone(),
                cmd: code,
                num: *num,
            };
            f.pack()?;
            Ok(vec![(f, both)])
        }
        Outgoing::Text { from, to, text } => text_frames(
            Frame::Directed {
                from: from.clone(),
                to: to.clone(),
                cmd: 31,
                num: 0,
            },
            text,
        ),
        Outgoing::Relay {
            from,
            via,
            dest,
            text,
        } => text_frames(
            Frame::Directed {
                from: from.clone(),
                to: via.clone(),
                cmd: 5,
                num: 0,
            },
            &format!("{dest} {text}"),
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frames_of_the_vectors() {
        let hb = Frame::Heartbeat {
            call: "LY2ABC".into(),
            grid: "KO24".into(),
            cq: false,
            choice: 0,
        };
        assert_eq!(bits_to_chars(&hb.pack().unwrap()), "2jAmkFVDORMW");
        assert_eq!(hb.display(), "LY2ABC: @HB HEARTBEAT KO24");
        let d = Frame::Directed {
            from: "OK1XX".into(),
            to: "LY2ABC".into(),
            cmd: command_code(" SNR").unwrap(),
            num: -12,
        };
        assert_eq!(bits_to_chars(&d.pack().unwrap()), "TByV0oYetlaJ");
        assert_eq!(d.display(), "OK1XX: LY2ABC SNR -12");
        let cq = Frame::Heartbeat {
            call: "DL1XYZ".into(),
            grid: "JO62".into(),
            cq: true,
            choice: 0,
        };
        assert_eq!(bits_to_chars(&cq.pack().unwrap()), "1hCAf9V4vTTG");
        assert_eq!(cq.display(), "DL1XYZ: @ALLCALL CQ CQ CQ JO62");
        let t = Frame::Data {
            text: "HELLO WORLD".into(),
            compressed: false,
        };
        assert_eq!(bits_to_chars(&t.pack().unwrap()), "XpFFwNy6VR++");
        for f in [hb, d, cq, t] {
            assert_eq!(Frame::unpack(&f.pack().unwrap()), Some(f));
        }
    }

    #[test]
    fn calls_and_grids_round_trip() {
        for c in [
            "LY2ABC", "K1ABC", "G4ABC", "W1AW", "OK1XX", "3DA0XYZ", "LY2ABC/P", "@ALLCALL", "@HB",
        ] {
            let (v, p) = pack_call28(c).unwrap();
            assert_eq!(unpack_call28(v, p).as_deref(), Some(c));
        }
        for c in ["PJ4/K1ABC", "VE3/LY2ABC", "LY2ABC"] {
            assert_eq!(unpack_call50(pack_call50(c).unwrap()).as_deref(), Some(c));
        }
        for g in [
            "KO24", "JO62", "FN42", "IO91", "PM95", "AA00", "RR99", "JN11",
        ] {
            assert_eq!(unpack_grid(pack_grid(g)).as_deref(), Some(g), "{g}");
        }
    }

    #[test]
    fn messages_become_frames() {
        let f = frames_of(&Outgoing::Text {
            from: "G4ABC".into(),
            to: "DL1XYZ".into(),
            text: "HELLO FROM LONDON. HOW COPY?".into(),
        })
        .unwrap();
        assert_eq!(f.len(), 4);
        assert_eq!(f[0].1, FLAG_FIRST);
        assert_eq!(f[3].1, FLAG_LAST);
        assert!(frames_of(&Outgoing::Command {
            from: "G4ABC".into(),
            to: "DL1XYZ".into(),
            cmd: " FOO".into(),
            num: 0
        })
        .is_err());
    }
}
