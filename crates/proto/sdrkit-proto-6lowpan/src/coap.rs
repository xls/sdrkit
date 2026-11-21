//! The CoAP message header (RFC 7252 section 3): type, code, token length, message id, and the Uri-Path options.

/// The message type, from bits 5 and 4 of the first byte.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// Confirmable.
    Con,
    /// Non-confirmable.
    Non,
    /// Acknowledgement.
    Ack,
    /// Reset.
    Rst,
}

impl Kind {
    /// "CON", "NON", "ACK" or "RST".
    pub fn name(self) -> &'static str {
        match self {
            Kind::Con => "CON",
            Kind::Non => "NON",
            Kind::Ack => "ACK",
            Kind::Rst => "RST",
        }
    }
}

/// The fixed header of a CoAP message and its Uri-Path.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Message {
    /// The message type.
    pub kind: Kind,
    /// The token length, 0 to 8.
    pub token_len: u8,
    /// The code: the class in the 3 high bits, the detail in the 5 low bits.
    pub code: u8,
    /// The message id.
    pub message_id: u16,
    /// The Uri-Path options, joined with "/". Empty without one.
    pub uri_path: String,
}

impl Message {
    /// The code as "class.detail" with two digits of detail: "0.01", "2.05".
    pub fn code_text(&self) -> String {
        format!("{}.{:02}", self.code >> 5, self.code & 0x1F)
    }

    /// The name of the code, for the codes that have a common name.
    pub fn code_name(&self) -> Option<&'static str> {
        Some(match self.code {
            0x01 => "GET",
            0x02 => "POST",
            0x03 => "PUT",
            0x04 => "DELETE",
            0x41 => "Created",
            0x44 => "Changed",
            0x45 => "Content",
            _ => return None,
        })
    }
}

/// An option delta or length nibble with its extension bytes (RFC 7252 section 3.1).
fn extended(nibble: u8, b: &[u8], at: &mut usize) -> Option<usize> {
    match nibble {
        13 => {
            let v = usize::from(*b.get(*at)?) + 13;
            *at += 1;
            Some(v)
        }
        14 => {
            let v = usize::from(u16::from_be_bytes([*b.get(*at)?, *b.get(*at + 1)?])) + 269;
            *at += 2;
            Some(v)
        }
        15 => None,
        n => Some(usize::from(n)),
    }
}

/// Reads a CoAP message. `None` for fewer than 4 bytes, for a version other than 1, and for a token length above 8.
pub fn read(b: &[u8]) -> Option<Message> {
    if b.len() < 4 || b[0] >> 6 != 1 {
        return None;
    }
    let token_len = b[0] & 0x0F;
    if token_len > 8 {
        return None;
    }
    let kind = match (b[0] >> 4) & 3 {
        0 => Kind::Con,
        1 => Kind::Non,
        2 => Kind::Ack,
        _ => Kind::Rst,
    };
    let mut uri_path = String::new();
    let mut at = 4 + usize::from(token_len);
    let mut number = 0usize;
    // The options end at the payload marker 0xFF, at the end of the message, or where one is cut off.
    while let Some(&h) = b.get(at) {
        if h == 0xFF {
            break;
        }
        at += 1;
        let (Some(delta), Some(len)) =
            (extended(h >> 4, b, &mut at), extended(h & 0x0F, b, &mut at))
        else {
            break;
        };
        number += delta;
        let Some(value) = b.get(at..at + len) else {
            break;
        };
        at += len;
        // Option 11 is Uri-Path.
        if number == 11 {
            if !uri_path.is_empty() {
                uri_path.push('/');
            }
            uri_path.extend(value.iter().map(|&c| {
                if (32..127).contains(&c) {
                    c as char
                } else {
                    '?'
                }
            }));
        }
    }
    Some(Message {
        kind,
        token_len,
        code: b[1],
        message_id: u16::from_be_bytes([b[2], b[3]]),
        uri_path,
    })
}
