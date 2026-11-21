//! The reader of ESP-NOW frames: the 802.11 management action frame (IEEE Std 802.11-2020 clause 9.6) of category 127 with
//! the Espressif organisation identifier, its random value, and the vendor-specific elements (element id 221, type 4) that
//! carry the payload, as the maker's public ESP-NOW guide gives them. A protected frame is opened with CCMP when a key the
//! user entered fits. Nothing reads past the end of a frame.

use super::ccmp::{self, CcmpCipher, FLAG_MANAGEMENT, HEADER_LEN};
use crate::report::mac_text;

/// The category of a vendor-specific action frame.
pub const CATEGORY_VENDOR: u8 = 127;
/// The Espressif organisation identifier.
pub const ESPRESSIF_OUI: [u8; 3] = [0x18, 0xFE, 0x34];
/// The element id of a vendor-specific element.
pub const ELEMENT_ID: u8 = 221;
/// The vendor type of an ESP-NOW element.
pub const ESPNOW_TYPE: u8 = 4;
/// The longest body of one element.
pub const MAX_ELEMENT_BODY: usize = 250;
/// The longest payload of a frame (version 2).
pub const MAX_PAYLOAD: usize = 1470;
/// Bit 4 of the version byte: more data follows (version 2).
pub const MORE_DATA: u8 = 0x10;

/// The MAC header fields of an action frame.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ActionHeader {
    /// 13 (action) or 14 (action no ack).
    pub subtype: u8,
    /// The Protected bit.
    pub protected: bool,
    /// Address 1, the destination.
    pub to: [u8; 6],
    /// Address 2, the sender.
    pub from: [u8; 6],
    /// Address 3.
    pub addr3: [u8; 6],
    /// The sequence number (12 bits).
    pub seq: u16,
    /// The fragment number.
    pub fragment: u8,
}

impl ActionHeader {
    /// Whether address 1 is the broadcast address.
    pub fn broadcast(&self) -> bool {
        self.to == [0xFF; 6]
    }
}

/// The header of a management action frame (type 0, subtype 13 or 14, protocol version 0) of at least 24 bytes; `None`
/// for any other frame.
pub fn action_header(mpdu: &[u8]) -> Option<ActionHeader> {
    if mpdu.len() < HEADER_LEN || mpdu[0] & 0x03 != 0 || (mpdu[0] >> 2) & 3 != 0 {
        return None;
    }
    let subtype = mpdu[0] >> 4;
    if subtype != 13 && subtype != 14 {
        return None;
    }
    let addr = |at: usize| -> [u8; 6] { mpdu[at..at + 6].try_into().unwrap_or_default() };
    let sc = u16::from_le_bytes([mpdu[22], mpdu[23]]);
    Some(ActionHeader {
        subtype,
        protected: mpdu[1] & 0x40 != 0,
        to: addr(4),
        from: addr(10),
        addr3: addr(16),
        seq: sc >> 4,
        fragment: (sc & 0x0F) as u8,
    })
}

/// The vendor content of an ESP-NOW frame.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Content {
    /// The four random bytes, the first byte highest.
    pub random: u32,
    /// The version of the first ESP-NOW element (bits 3 to 0).
    pub version: u8,
    /// The organisation identifier of the first ESP-NOW element.
    pub element_oui: [u8; 3],
    /// ESP-NOW elements read.
    pub elements: u32,
    /// "More data" of the last ESP-NOW element.
    pub more_data: bool,
    /// An element ran past the end of the frame and ended the reading.
    pub truncated: bool,
    /// The bodies of the ESP-NOW elements joined in order, at most [`MAX_PAYLOAD`] bytes.
    pub payload: Vec<u8>,
}

/// Reads the vendor content of an action frame body `body` (from the category on, FCS removed): `None` unless the body has
/// at least 8 bytes, category 127, the Espressif identifier and at least one element of id 221, length 5 or more and type
/// 4. Elements of another kind are skipped. An element that runs past the end marks the content truncated and ends the
/// reading.
pub fn read_content(body: &[u8]) -> Option<Content> {
    if body.len() < 8 || body[0] != CATEGORY_VENDOR || body[1..4] != ESPRESSIF_OUI {
        return None;
    }
    let mut c = Content {
        random: u32::from_be_bytes([body[4], body[5], body[6], body[7]]),
        ..Content::default()
    };
    let mut at = 8;
    while at + 2 <= body.len() {
        let (id, len) = (body[at], usize::from(body[at + 1]));
        let end = at + 2 + len;
        if end > body.len() {
            c.truncated = true;
            break;
        }
        let e = &body[at + 2..end];
        if id == ELEMENT_ID && len >= 5 && e[3] == ESPNOW_TYPE {
            if c.elements == 0 {
                c.version = e[4] & 0x0F;
                c.element_oui = [e[0], e[1], e[2]];
            }
            c.elements += 1;
            c.more_data = e[4] & MORE_DATA != 0;
            let room = MAX_PAYLOAD - c.payload.len();
            c.payload.extend_from_slice(&e[5..e.len().min(5 + room)]);
        }
        at = end;
    }
    (c.elements > 0).then_some(c)
}

/// What a protected action frame shows without its key.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Sealed {
    /// The packet number.
    pub pn: u64,
    /// The key id.
    pub key_id: u8,
    /// The length of the encrypted body.
    pub len: usize,
}

/// One frame as the stack lists it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Message {
    /// The MAC header.
    pub header: ActionHeader,
    /// The content, when the frame is clear or was opened.
    pub content: Option<Content>,
    /// The CCMP fields of a protected frame.
    pub sealed: Option<Sealed>,
    /// The frame was opened with a key.
    pub decrypted: bool,
}

impl Message {
    /// Whether the payload is known.
    pub fn readable(&self) -> bool {
        self.content.is_some()
    }
}

/// What [`read`] makes of a frame.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Reading {
    /// Not a management action frame.
    Other,
    /// An action frame that is not ESP-NOW (counted only).
    Action(ActionHeader),
    /// An ESP-NOW frame, clear, opened, or protected and taken as ESP-NOW by its addresses.
    EspNow(Message),
}

/// Reads `mpdu` (FCS removed). A protected frame is tried with each of `keys`, with the management nonce flag and without;
/// the first MIC that holds opens it. A protected frame that no key opens is ESP-NOW when `espressif` holds for its sender
/// or its address 3 is the broadcast address. `scratch` holds the opened body.
pub fn read(
    mpdu: &[u8],
    keys: &[CcmpCipher],
    espressif: impl Fn(&[u8; 6]) -> bool,
    scratch: &mut Vec<u8>,
) -> Reading {
    let Some(h) = action_header(mpdu) else {
        return Reading::Other;
    };
    if !h.protected {
        return match read_content(&mpdu[HEADER_LEN..]) {
            Some(c) => Reading::EspNow(Message {
                header: h,
                content: Some(c),
                sealed: None,
                decrypted: false,
            }),
            None => Reading::Action(h),
        };
    }
    let Some(ccmp::Protected {
        header,
        pn,
        key_id,
        ciphertext,
        mic,
    }) = ccmp::split(mpdu)
    else {
        return Reading::Action(h);
    };
    let sealed = Sealed {
        pn,
        key_id,
        len: ciphertext.len(),
    };
    scratch.clear();
    scratch.resize(ciphertext.len(), 0);
    for k in keys {
        for flags in [FLAG_MANAGEMENT, 0] {
            if ccmp::open(k, flags, header, pn, ciphertext, mic, scratch) {
                return match read_content(scratch) {
                    Some(c) => Reading::EspNow(Message {
                        header: h,
                        content: Some(c),
                        sealed: Some(sealed),
                        decrypted: true,
                    }),
                    None => Reading::Action(h),
                };
            }
        }
    }
    if espressif(&h.from) || h.addr3 == [0xFF; 6] {
        Reading::EspNow(Message {
            header: h,
            content: None,
            sealed: Some(sealed),
            decrypted: false,
        })
    } else {
        Reading::Action(h)
    }
}

/// Whether `b` reads as text: at least 3 bytes, of which at least 80 % are printable ASCII, tab, CR, LF or NUL.
pub fn is_text(b: &[u8]) -> bool {
    if b.len() < 3 {
        return false;
    }
    let ok = b
        .iter()
        .filter(|&&c| (0x20..0x7F).contains(&c) || matches!(c, 0 | b'\t' | b'\r' | b'\n'))
        .count();
    ok * 5 >= b.len() * 4
}

/// Up to 60 characters of `b`, `.` for a byte that is not printable ASCII.
pub fn quoted(b: &[u8]) -> String {
    b.iter()
        .take(60)
        .map(|&c| {
            if (0x20..0x7F).contains(&c) {
                char::from(c)
            } else {
                '.'
            }
        })
        .collect()
}

/// "broadcast" or the address as text.
pub fn destination(h: &ActionHeader) -> String {
    if h.broadcast() {
        "broadcast".into()
    } else {
        mac_text(&h.to)
    }
}

/// The summary of a message: `v1, 9 bytes to broadcast: "temp=21.5"` or `encrypted (CCMP, PN 1) to 24:6F:28:11:22:33`, with
/// ", several elements", ", decrypted", ", more data" and ", truncated" added as they apply.
pub fn summary(m: &Message) -> String {
    let to = destination(&m.header);
    let Some(c) = &m.content else {
        let pn = m.sealed.map_or(0, |s| s.pn);
        return format!("encrypted (CCMP, PN {pn}) to {to}");
    };
    let mut s = format!("v{}, {} bytes to {to}", c.version, c.payload.len());
    if is_text(&c.payload) {
        s.push_str(": \"");
        s.push_str(&quoted(&c.payload));
        s.push('"');
    }
    if c.elements > 1 {
        s.push_str(", several elements");
    }
    if m.decrypted {
        s.push_str(", decrypted");
    }
    if c.more_data {
        s.push_str(", more data");
    }
    if c.truncated {
        s.push_str(", truncated");
    }
    if c.element_oui != ESPRESSIF_OUI {
        s.push_str(", element identifier ");
        s.push_str(&crate::report::hex(&c.element_oui));
    }
    s
}

/// Parses 16-byte keys written as 32 hex digits each, apart by spaces, commas or semicolons; ':' and '-' inside a key are
/// ignored. `None` when an entry is not 32 hex digits.
pub fn parse_keys(text: &str) -> Option<Vec<[u8; 16]>> {
    text.split([' ', ',', ';', '\n', '\t'])
        .filter(|t| !t.is_empty())
        .map(|t| {
            let digits: Vec<u8> = t.bytes().filter(|&c| c != b':' && c != b'-').collect();
            if digits.len() != 32 || !digits.iter().all(u8::is_ascii_hexdigit) {
                return None;
            }
            let mut k = [0u8; 16];
            for (i, pair) in digits.chunks(2).enumerate() {
                let s = std::str::from_utf8(pair).ok()?;
                k[i] = u8::from_str_radix(s, 16).ok()?;
            }
            Some(k)
        })
        .collect()
}
