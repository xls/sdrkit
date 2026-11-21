//! IEEE 802.15.4 MAC frames: the header with the PAN-ID presence rules of every frame version, the auxiliary
//! security header, and the vendor of an extended address.
//! Standard: IEEE 802.15.4-2020 clause 7.2.

use serde::{Deserialize, Serialize};

/// An address.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum Addr {
    /// None.
    #[default]
    None,
    /// Short.
    Short(u16),
    /// Extended (EUI-64).
    Ext(u64),
}

impl Addr {
    /// "0x1A2B", "broadcast", "00:12:4B:...", "-".
    pub fn text(&self) -> String {
        match self {
            Addr::None => "-".into(),
            Addr::Short(0xFFFF) => "broadcast".into(),
            Addr::Short(s) => format!("0x{s:04X}"),
            Addr::Ext(e) => eui(*e),
        }
    }
}

/// An EUI-64 as "00:12:4B:00:01:02:03:04".
pub fn eui(e: u64) -> String {
    e.to_be_bytes()
        .iter()
        .map(|b| format!("{b:02X}"))
        .collect::<Vec<_>>()
        .join(":")
}

/// The auxiliary security header.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Default)]
pub struct Aux {
    /// Security level.
    pub level: u8,
    /// Key identifier mode.
    pub key_id_mode: u8,
    /// Frame counter (when sent).
    pub counter: Option<u32>,
    /// Key index (last octet of the key identifier).
    pub key_index: Option<u8>,
    /// MIC length.
    pub mic_len: usize,
}

/// A parsed MAC header.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Default)]
pub struct Mac {
    /// Frame type.
    pub ftype: u8,
    /// Security enabled.
    pub security: bool,
    /// Frame pending.
    pub pending: bool,
    /// Ack request.
    pub ack_req: bool,
    /// PAN ID compression.
    pub pan_comp: bool,
    /// Sequence number suppressed.
    pub seq_suppressed: bool,
    /// IE present.
    pub ie_present: bool,
    /// Frame version.
    pub version: u8,
    /// Sequence number.
    pub seq: Option<u8>,
    /// Destination PAN.
    pub dst_pan: Option<u16>,
    /// Destination.
    pub dst: Addr,
    /// Source PAN (the destination PAN when absent and the destination PAN is present).
    pub src_pan: Option<u16>,
    /// Whether the source PAN was on air.
    pub src_pan_sent: bool,
    /// Source.
    pub src: Addr,
    /// Security header.
    pub aux: Option<Aux>,
    /// Header length (offset of the IEs or the payload).
    pub hdr_len: usize,
}

impl Mac {
    /// The frame type name.
    pub fn type_name(&self) -> &'static str {
        match self.ftype {
            0 => "Beacon",
            1 => "Data",
            2 => "Ack",
            3 => "MAC command",
            5 => "Multipurpose",
            6 => "Fragment",
            7 => "Extended",
            _ => "Reserved",
        }
    }
    /// Payload encrypted (security level 4 to 7).
    pub fn encrypted(&self) -> bool {
        self.aux.as_ref().is_some_and(|a| a.level >= 4)
    }
}

fn le16(b: &[u8], i: usize) -> Option<u16> {
    Some(u16::from_le_bytes([*b.get(i)?, *b.get(i + 1)?]))
}

/// Parses a MAC header from a PSDU without its FCS.
pub fn parse(b: &[u8]) -> Option<Mac> {
    let fc = le16(b, 0)?;
    let mut m = Mac {
        ftype: (fc & 7) as u8,
        security: fc & 0x08 != 0,
        pending: fc & 0x10 != 0,
        ack_req: fc & 0x20 != 0,
        pan_comp: fc & 0x40 != 0,
        seq_suppressed: fc & 0x100 != 0,
        ie_present: fc & 0x200 != 0,
        version: ((fc >> 12) & 3) as u8,
        ..Default::default()
    };
    let dm = ((fc >> 10) & 3) as u8;
    let sm = ((fc >> 14) & 3) as u8;
    if dm == 1 || sm == 1 || m.version == 3 {
        return None;
    }
    let mut i = 2;
    if !(m.version == 2 && m.seq_suppressed) {
        m.seq = Some(*b.get(i)?);
        i += 1;
    }
    let (dpan, span) = if m.version < 2 {
        (dm != 0, sm != 0 && !m.pan_comp)
    } else {
        match (dm, sm) {
            (0, 0) => (m.pan_comp, false),
            (0, _) => (false, !m.pan_comp),
            (_, 0) => (!m.pan_comp, false),
            (3, 3) => (!m.pan_comp, false),
            _ => (true, !m.pan_comp),
        }
    };
    if dpan {
        m.dst_pan = Some(le16(b, i)?);
        i += 2;
    }
    let addr = |mode: u8, i: &mut usize| -> Option<Addr> {
        match mode {
            2 => {
                let a = le16(b, *i)?;
                *i += 2;
                Some(Addr::Short(a))
            }
            3 => {
                let s = b.get(*i..*i + 8)?;
                *i += 8;
                Some(Addr::Ext(u64::from_le_bytes(s.try_into().ok()?)))
            }
            _ => Some(Addr::None),
        }
    };
    m.dst = addr(dm, &mut i)?;
    if span {
        m.src_pan = Some(le16(b, i)?);
        m.src_pan_sent = true;
        i += 2;
    } else if m.dst_pan.is_some() && sm != 0 {
        m.src_pan = m.dst_pan;
    }
    m.src = addr(sm, &mut i)?;
    if m.security {
        let sc = *b.get(i)?;
        i += 1;
        let level = sc & 7;
        let kim = (sc >> 3) & 3;
        let suppress = m.version == 2 && sc & 0x20 != 0;
        let counter = if suppress {
            None
        } else {
            let c = u32::from_le_bytes(b.get(i..i + 4)?.try_into().ok()?);
            i += 4;
            Some(c)
        };
        let kl = [0, 1, 5, 9][kim as usize];
        let key_index = if kl > 0 {
            Some(*b.get(i + kl - 1)?)
        } else {
            None
        };
        i += kl;
        m.aux = Some(Aux {
            level,
            key_id_mode: kim,
            counter,
            key_index,
            mic_len: [0, 4, 8, 16][(level & 3) as usize],
        });
    }
    m.hdr_len = i;
    Some(m)
}

/// The PAN that a frame belongs to: for a beacon its source PAN; else the destination PAN when it is not 0xFFFF; else the source
/// PAN when it is not 0xFFFF.
pub fn pan_of(mac: &Mac) -> Option<u16> {
    if mac.ftype == 0 {
        mac.src_pan
    } else {
        mac.dst_pan
            .filter(|&p| p != 0xFFFF)
            .or(mac.src_pan.filter(|&p| p != 0xFFFF))
    }
}

/// The fields of a beacon in front of its beacon payload.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Beacon {
    /// The sender is the PAN coordinator (bit 14 of the superframe field).
    pub coordinator: bool,
    /// The sender permits association (bit 15 of the superframe field).
    pub permit: bool,
    /// The offset of the beacon payload in the MAC payload: after the superframe, guaranteed-slot and pending-address fields.
    pub payload: usize,
}

/// Reads the superframe, guaranteed-slot and pending-address fields at the start of the MAC payload `pay` of a beacon.
pub fn beacon(pay: &[u8]) -> Beacon {
    let sf = le16(pay, 0).unwrap_or(0);
    let mut i = 2;
    let gts = *pay.get(i).unwrap_or(&0);
    i += 1;
    if gts & 7 != 0 {
        i += 1 + 3 * (gts & 7) as usize;
    }
    let pend = *pay.get(i).unwrap_or(&0);
    i += 1 + 2 * (pend & 7) as usize + 8 * ((pend >> 4) & 7) as usize;
    Beacon {
        coordinator: sf & 0x4000 != 0,
        permit: sf & 0x8000 != 0,
        payload: i,
    }
}

/// The text of a beacon of PAN `pan` that carries no payload of a known network layer.
pub fn beacon_text(pan: u16, b: &Beacon) -> String {
    format!(
        "Beacon: PAN 0x{pan:04X}{}{}",
        if b.coordinator {
            ", PAN coordinator"
        } else {
            ""
        },
        if b.permit { ", join allowed" } else { "" }
    )
}

/// The text of the MAC command at the start of the MAC payload `pay` of a command frame, and the (extended, short) address
/// pair that an association response gives to a device that joined.
pub fn command_text(mac: &Mac, pay: &[u8]) -> (String, Option<(u64, u16)>) {
    let mut bind = None;
    let text = match *pay.first().unwrap_or(&0) {
        1 => {
            let c = *pay.get(1).unwrap_or(&0);
            format!(
                "Association request: {}, {}, {}",
                if c & 2 != 0 {
                    "full-function"
                } else {
                    "reduced-function"
                },
                if c & 4 != 0 {
                    "mains powered"
                } else {
                    "battery"
                },
                if c & 8 != 0 {
                    "rx on when idle"
                } else {
                    "rx off when idle"
                }
            )
        }
        2 => {
            let a = le16(pay, 1).unwrap_or(0xFFFF);
            let st = *pay.get(3).unwrap_or(&0xFF);
            if st == 0 {
                if let Addr::Ext(e) = mac.dst {
                    bind = Some((e, a));
                }
            }
            format!(
                "Association response: address 0x{a:04X}, {}",
                match st {
                    0 => "joined",
                    1 => "PAN at capacity",
                    2 => "access denied",
                    _ => "refused",
                }
            )
        }
        3 => "Disassociation notification".into(),
        4 => "Data request".into(),
        5 => "PAN ID conflict notification".into(),
        6 => "Orphan notification".into(),
        7 => "Beacon request".into(),
        8 => "Coordinator realignment".into(),
        9 => "GTS request".into(),
        _ => "Command".into(),
    };
    (text, bind)
}

/// Builds a MAC header (no security, no IEs) for tests and generators: frame control fields, sequence, PANs and addresses.
#[allow(clippy::too_many_arguments)]
pub fn build_header(
    ftype: u8,
    ack_req: bool,
    pan_comp: bool,
    version: u8,
    seq: u8,
    dst_pan: Option<u16>,
    dst: Addr,
    src_pan: Option<u16>,
    src: Addr,
    security: bool,
) -> Vec<u8> {
    let dm = match dst {
        Addr::None => 0,
        Addr::Short(_) => 2,
        Addr::Ext(_) => 3,
    };
    let sm = match src {
        Addr::None => 0,
        Addr::Short(_) => 2,
        Addr::Ext(_) => 3,
    };
    let fc: u16 = ftype as u16
        | (security as u16) << 3
        | (ack_req as u16) << 5
        | (pan_comp as u16) << 6
        | dm << 10
        | (version as u16) << 12
        | sm << 14;
    let mut v = fc.to_le_bytes().to_vec();
    v.push(seq);
    if let Some(p) = dst_pan {
        v.extend(p.to_le_bytes());
    }
    let put = |v: &mut Vec<u8>, a: Addr| match a {
        Addr::Short(s) => v.extend(s.to_le_bytes()),
        Addr::Ext(e) => v.extend(e.to_le_bytes()),
        Addr::None => {}
    };
    put(&mut v, dst);
    if let Some(p) = src_pan {
        v.extend(p.to_le_bytes());
    }
    put(&mut v, src);
    v
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version2_pan_rules() {
        // one frame per row of the rule list
        let cases: [(Addr, Addr, bool, bool, bool); 6] = [
            (Addr::None, Addr::None, true, true, false),
            (Addr::None, Addr::Short(1), false, false, true),
            (Addr::Short(1), Addr::None, false, true, false),
            (Addr::Ext(1), Addr::Ext(2), false, true, false),
            (Addr::Ext(1), Addr::Ext(2), true, false, false),
            (Addr::Short(1), Addr::Ext(2), false, true, true),
        ];
        for (d, s, comp, dpan, span) in cases {
            let h = build_header(
                1,
                false,
                comp,
                2,
                5,
                dpan.then_some(0x1234),
                d,
                span.then_some(0x5678),
                s,
                false,
            );
            let m = parse(&h).unwrap();
            assert_eq!(m.dst, d);
            assert_eq!(m.src, s);
            assert_eq!(m.dst_pan.is_some(), dpan, "{d:?} {s:?} {comp}");
            assert_eq!(m.src_pan_sent, span);
            assert_eq!(m.hdr_len, h.len());
        }
    }
}
