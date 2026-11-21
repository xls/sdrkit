//! The 802.11 data frames that wfb-ng and OpenHD inject, and the count of frames lost by FEC block and fragment. No public
//! document gives these layouts; the payload stays encrypted.

/// The frame control of the frames: a data frame to the distribution system.
pub const FRAME_CONTROL: [u8; 2] = [0x08, 0x01];
/// The first bytes of address 2 and address 3 of a wfb-ng frame ("WB").
pub const WFB_TAG: [u8; 2] = [0x57, 0x42];
/// The offset of the payload in the MPDU.
pub const PAYLOAD: usize = 24;
/// The authentication tag after the encrypted payload, bytes.
pub const TAG_LEN: usize = 16;

/// What a frame is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// A wfb-ng data packet of one FEC block and fragment.
    Data {
        /// The FEC block.
        block: u64,
        /// The fragment in the block.
        fragment: u8,
    },
    /// A wfb-ng session key packet.
    Session,
    /// An OpenHD frame with its nonce.
    Frame {
        /// The nonce.
        nonce: u64,
    },
}

impl Kind {
    /// "data", "session" or "frame".
    pub fn text(self) -> &'static str {
        match self {
            Kind::Data { .. } => "data",
            Kind::Session => "session",
            Kind::Frame { .. } => "frame",
        }
    }
}

/// The link of a frame.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum LinkId {
    /// wfb-ng: the link id (24 bits) and the radio port.
    Wfb {
        /// The link id.
        link: u32,
        /// The radio port.
        port: u8,
    },
    /// OpenHD: the direction (1 air, 2 ground) and the radio port.
    OpenHd {
        /// 1 from the air unit, 2 from the ground unit.
        direction: u8,
        /// The radio port.
        port: u8,
    },
}

impl LinkId {
    /// "wfb-ng" or "OpenHD".
    pub fn system(&self) -> &'static str {
        match self {
            LinkId::Wfb { .. } => "wfb-ng",
            LinkId::OpenHd { .. } => "OpenHD",
        }
    }
    /// "link 0A1B2C port 3" or "air port 5".
    pub fn text(&self) -> String {
        match *self {
            LinkId::Wfb { link, port } => format!("link {link:06X} port {port}"),
            LinkId::OpenHd { direction, port } => format!(
                "{} port {port}",
                if direction == 1 { "air" } else { "ground" }
            ),
        }
    }
}

/// A frame read from an MPDU.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WfbFrame {
    /// The link.
    pub link: LinkId,
    /// What it is.
    pub kind: Kind,
}

/// Reads an MPDU (without its FCS): frame control 08 01, address 1 broadcast, then the wfb-ng or the OpenHD layout. `None`
/// for any other frame.
pub fn parse(m: &[u8]) -> Option<WfbFrame> {
    if m.len() < PAYLOAD || m[..2] != FRAME_CONTROL || m[4..10].iter().any(|&b| b != 0xFF) {
        return None;
    }
    let a2 = &m[10..16];
    let a3 = &m[16..22];
    let p = &m[PAYLOAD..];
    if a2[..2] == WFB_TAG && a2 == a3 {
        let id = u32::from_be_bytes([a2[2], a2[3], a2[4], a2[5]]);
        let link = LinkId::Wfb {
            link: id >> 8,
            port: id as u8,
        };
        let kind = match p.first()? {
            1 if p.len() >= 9 + TAG_LEN => {
                let nonce = u64::from_be_bytes(p[1..9].try_into().ok()?);
                Kind::Data {
                    block: nonce >> 8,
                    fragment: nonce as u8,
                }
            }
            2 if p.len() >= 25 + TAG_LEN => Kind::Session,
            _ => return None,
        };
        return Some(WfbFrame { link, kind });
    }
    if (a2[0] == 1 || a2[0] == 2) && a2[0] == a3[0] && a2[5] == a3[5] && a2[1..5] != a3[1..5] {
        let hi = u32::from_le_bytes([a3[1], a3[2], a3[3], a3[4]]);
        let lo = u32::from_le_bytes([a2[1], a2[2], a2[3], a2[4]]);
        return Some(WfbFrame {
            link: LinkId::OpenHd {
                direction: a2[0],
                port: a2[5],
            },
            kind: Kind::Frame {
                nonce: u64::from(hi) << 32 | u64::from(lo),
            },
        });
    }
    None
}

/// The header of a frame: frame control, duration 0, address 1 broadcast, addresses 2 and 3, sequence control 0.
fn header(a2: [u8; 6], a3: [u8; 6], out: &mut Vec<u8>) {
    out.extend(FRAME_CONTROL);
    out.extend([0, 0]);
    out.extend([0xFF; 6]);
    out.extend(a2);
    out.extend(a3);
    out.extend([0, 0]);
}

/// The MPDU (without FCS) of a wfb-ng data packet of `link` and `port`, FEC `block` and `fragment`, with `body` as the
/// encrypted payload and its tag.
pub fn wfb_data(link: u32, port: u8, block: u64, fragment: u8, body: &[u8]) -> Vec<u8> {
    let id = (link << 8 | u32::from(port)).to_be_bytes();
    let a = [WFB_TAG[0], WFB_TAG[1], id[0], id[1], id[2], id[3]];
    let mut m = Vec::with_capacity(PAYLOAD + 9 + body.len());
    header(a, a, &mut m);
    m.push(1);
    m.extend((block << 8 | u64::from(fragment)).to_be_bytes());
    m.extend_from_slice(body);
    m
}

/// The MPDU (without FCS) of a wfb-ng session key packet of `link` and `port` with `body` after the type byte.
pub fn wfb_session(link: u32, port: u8, body: &[u8]) -> Vec<u8> {
    let id = (link << 8 | u32::from(port)).to_be_bytes();
    let a = [WFB_TAG[0], WFB_TAG[1], id[0], id[1], id[2], id[3]];
    let mut m = Vec::with_capacity(PAYLOAD + 1 + body.len());
    header(a, a, &mut m);
    m.push(2);
    m.extend_from_slice(body);
    m
}

/// The MPDU (without FCS) of an OpenHD frame from `direction` (1 air, 2 ground) on `port` with `nonce` and `body`.
pub fn openhd(direction: u8, port: u8, nonce: u64, body: &[u8]) -> Vec<u8> {
    let lo = (nonce as u32).to_le_bytes();
    let hi = ((nonce >> 32) as u32).to_le_bytes();
    let a2 = [direction, lo[0], lo[1], lo[2], lo[3], port];
    let a3 = [direction, hi[0], hi[1], hi[2], hi[3], port];
    let mut m = Vec::with_capacity(PAYLOAD + body.len());
    header(a2, a3, &mut m);
    m.extend_from_slice(body);
    m
}

/// The count of frames lost of one wfb-ng link and port.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct LossCount {
    /// The last block and fragment seen.
    pub last: Option<(u64, u8)>,
    /// The largest fragment index seen, plus one (the fragments of a block, as far as known).
    pub width: u16,
    /// Frames lost so far.
    pub lost: u64,
}

impl LossCount {
    /// Takes the frame of `block` and `fragment`: a later fragment of the same block adds the fragments between; a later block
    /// (less than a million blocks on) adds the rest of the last block, the whole blocks between and the fragments before
    /// this one, once the block width is known.
    pub fn take(&mut self, block: u64, fragment: u8) {
        let f = u64::from(fragment);
        if let Some((b, last)) = self.last {
            let lf = u64::from(last);
            if block == b && f > lf {
                self.lost += f - lf - 1;
            } else if block > b && block - b < 1_000_000 && self.width > 0 {
                let w = u64::from(self.width);
                self.lost += w.saturating_sub(1 + lf) + (block - b - 1) * w + f;
            }
        }
        self.width = self.width.max(u16::from(fragment) + 1);
        if self.last.is_none_or(|(b, l)| (block, fragment) > (b, l)) {
            self.last = Some((block, fragment));
        }
    }
}
