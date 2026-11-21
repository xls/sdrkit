//! FlySky AFHDS and AFHDS 2A packets after the ID code. No public document gives them; there is no CRC on the air, so a
//! packet is taken on the ID code and the checks of its format and values. The bit order on the air is not confirmed.

/// The rate, bit/s.
pub const RATE: f64 = 500e3;
/// The air pattern: the last preamble bytes and the ID code.
pub const PATTERN: [u8; 6] = [0xAA, 0xAA, 0x54, 0x75, 0xC5, 0x2A];
/// Bytes of an AFHDS packet.
pub const AFHDS_LEN: usize = 21;
/// Bytes of an AFHDS 2A packet.
pub const AFHDS2A_LEN: usize = 38;

/// The kind of a packet.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// AFHDS channel values.
    AfhdsChannels,
    /// AFHDS bind.
    AfhdsBind,
    /// AFHDS 2A channel values.
    Channels2A,
    /// AFHDS 2A failsafe values.
    Failsafe2A,
    /// AFHDS 2A bind.
    Bind2A,
    /// AFHDS 2A telemetry.
    Telemetry2A,
}

impl Kind {
    /// The type text of the frame log.
    pub fn text(self) -> &'static str {
        match self {
            Kind::AfhdsChannels => "AFHDS channels",
            Kind::AfhdsBind => "AFHDS bind",
            Kind::Channels2A => "AFHDS 2A channels",
            Kind::Failsafe2A => "AFHDS 2A failsafe",
            Kind::Bind2A => "AFHDS 2A bind",
            Kind::Telemetry2A => "AFHDS 2A telemetry",
        }
    }
}

/// A packet.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Flysky {
    /// What it is.
    pub kind: Kind,
    /// The transmitter id as shown (AFHDS: p4 p3 p2 p1; AFHDS 2A: p1 p2 p3 p4).
    pub id: u32,
    /// The receiver id of AFHDS 2A (p5..p8), 0 for AFHDS.
    pub rx_id: u32,
    /// The channel values, µs (8 for AFHDS, 14 for AFHDS 2A; none for bind and telemetry).
    pub channels: Vec<u16>,
    /// Bytes of the packet.
    pub len: usize,
}

impl Flysky {
    /// The fields as text.
    pub fn detail(&self) -> String {
        let mut s = String::new();
        if !self.channels.is_empty() {
            let v: Vec<String> = self.channels.iter().map(u16::to_string).collect();
            s = format!("CH1-{}: {}", self.channels.len(), v.join(" "));
        }
        if self.rx_id != 0 {
            if !s.is_empty() {
                s.push_str(", ");
            }
            s.push_str(&format!("receiver {:08X}", self.rx_id));
        }
        s
    }
}

/// The 21 bytes of an AFHDS channel packet of transmitter `id` (shown p4 p3 p2 p1).
pub fn afhds(id: u32, ch: &[u16; 8]) -> [u8; AFHDS_LEN] {
    let mut p = [0u8; AFHDS_LEN];
    p[0] = 0x55;
    p[1..5].copy_from_slice(&id.to_le_bytes());
    for (k, &c) in ch.iter().enumerate() {
        p[5 + 2 * k..7 + 2 * k].copy_from_slice(&c.to_le_bytes());
    }
    p
}

/// The 38 bytes of an AFHDS 2A channel packet of transmitter `id` (shown p1 p2 p3 p4) to receiver `rx_id`.
pub fn afhds2a(id: u32, rx_id: u32, ch: &[u16; 14]) -> [u8; AFHDS2A_LEN] {
    let mut p = [0u8; AFHDS2A_LEN];
    p[0] = 0x58;
    p[1..5].copy_from_slice(&id.to_be_bytes());
    p[5..9].copy_from_slice(&rx_id.to_be_bytes());
    for (i, &c) in ch.iter().enumerate() {
        p[9 + 2 * i] = c as u8;
        p[10 + 2 * i] = ((c >> 8) & 0x0F) as u8;
    }
    p
}

/// Reads a packet from the bytes after the ID code; `long` says that the burst holds the bytes of an AFHDS 2A packet (it
/// settles a first byte of AA, which both layouts use). `None` when the first byte or a value is out of range.
pub fn parse(b: &[u8], long: bool) -> Option<Flysky> {
    let p0 = *b.first()?;
    let two_a = match p0 {
        0x58 | 0x56 | 0xBB | 0xBC | 0xAC => true,
        0x55 => false,
        0xAA => long,
        _ => return None,
    };
    if two_a {
        if b.len() < AFHDS2A_LEN {
            return None;
        }
        let kind = match p0 {
            0x58 => Kind::Channels2A,
            0x56 => Kind::Failsafe2A,
            0xBB | 0xBC => Kind::Bind2A,
            _ => Kind::Telemetry2A,
        };
        let channels: Vec<u16> = if matches!(kind, Kind::Channels2A | Kind::Failsafe2A) {
            (0..14)
                .map(|i| u16::from(b[9 + 2 * i]) | u16::from(b[10 + 2 * i] & 0x0F) << 8)
                .collect()
        } else {
            Vec::new()
        };
        if kind == Kind::Channels2A && channels.iter().any(|&v| !(800..=2300).contains(&v)) {
            return None;
        }
        return Some(Flysky {
            kind,
            id: u32::from_be_bytes([b[1], b[2], b[3], b[4]]),
            rx_id: u32::from_be_bytes([b[5], b[6], b[7], b[8]]),
            channels,
            len: AFHDS2A_LEN,
        });
    }
    if b.len() < AFHDS_LEN {
        return None;
    }
    let kind = if p0 == 0x55 {
        Kind::AfhdsChannels
    } else {
        Kind::AfhdsBind
    };
    let channels: Vec<u16> = if kind == Kind::AfhdsChannels {
        (0..8)
            .map(|k| u16::from_le_bytes([b[5 + 2 * k], b[6 + 2 * k]]))
            .collect()
    } else {
        Vec::new()
    };
    if channels.iter().any(|&v| !(900..=2200).contains(&v)) {
        return None;
    }
    Some(Flysky {
        kind,
        id: u32::from_le_bytes([b[1], b[2], b[3], b[4]]),
        rx_id: 0,
        channels,
        len: AFHDS_LEN,
    })
}
