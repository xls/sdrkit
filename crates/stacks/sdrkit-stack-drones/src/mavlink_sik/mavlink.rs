//! MAVLink v1 and v2 frames: the layout, the checksum with its CRC_EXTRA byte, a stream that finds frames across packets, and
//! the fields of the messages the stack describes.
//! Standard: MAVLink Developer Guide (serialization, CRC_EXTRA); message definitions `minimal.xml` and `common.xml`.

use sdrkit_util_bits::crc::catalog::CRC16_MCRF4XX;

/// The start byte of a v1 frame.
pub const STX_V1: u8 = 0xFE;
/// The start byte of a v2 frame.
pub const STX_V2: u8 = 0xFD;
/// Bytes that may wait in a stream for the rest of a frame.
pub const STREAM_CAP: usize = 300;

/// A message the stack knows: its id, name, CRC_EXTRA byte and payload length without extensions.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MessageInfo {
    /// The message id.
    pub id: u32,
    /// The name.
    pub name: &'static str,
    /// The CRC_EXTRA byte.
    pub crc_extra: u8,
    /// The payload length without extension fields, bytes.
    pub len: usize,
}

/// The messages the stack knows (MAVLink `minimal.xml`, `common.xml`). A frame of another id is not accepted.
pub const MESSAGES: [MessageInfo; 7] = [
    MessageInfo {
        id: 0,
        name: "HEARTBEAT",
        crc_extra: 50,
        len: 9,
    },
    MessageInfo {
        id: 1,
        name: "SYS_STATUS",
        crc_extra: 124,
        len: 31,
    },
    MessageInfo {
        id: 24,
        name: "GPS_RAW_INT",
        crc_extra: 24,
        len: 30,
    },
    MessageInfo {
        id: 30,
        name: "ATTITUDE",
        crc_extra: 39,
        len: 28,
    },
    MessageInfo {
        id: 33,
        name: "GLOBAL_POSITION_INT",
        crc_extra: 104,
        len: 28,
    },
    MessageInfo {
        id: 74,
        name: "VFR_HUD",
        crc_extra: 20,
        len: 20,
    },
    MessageInfo {
        id: 253,
        name: "STATUSTEXT",
        crc_extra: 83,
        len: 51,
    },
];

/// The message of id `id`, if the stack knows it.
pub fn message(id: u32) -> Option<&'static MessageInfo> {
    MESSAGES.iter().find(|m| m.id == id)
}

/// The CRC_EXTRA byte of a message by the MAVLink rule: the CRC-16/MCRF4XX of the name and a space, then of each field in
/// wire order (extension fields left out) its type, a space, its name and a space, and for an array the array length as one
/// byte; the result is the low byte XOR the high byte. `fields` are `(type, name, array length)`, the type as the definition
/// file writes it without a `_mavlink_version` suffix (`uint8_t`).
pub fn crc_extra(name: &str, fields: &[(&str, &str, u8)]) -> u8 {
    let mut d = CRC16_MCRF4XX.digest();
    d.update(name.as_bytes());
    d.update(b" ");
    for &(ty, field, n) in fields {
        d.update(ty.as_bytes());
        d.update(b" ");
        d.update(field.as_bytes());
        d.update(b" ");
        if n > 0 {
            d.update(&[n]);
        }
    }
    let c = d.finalize();
    (c & 0xFF) as u8 ^ (c >> 8) as u8
}

/// A frame that passed its checksum.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Frame {
    /// 1 or 2.
    pub version: u8,
    /// The sequence number.
    pub seq: u8,
    /// The system id.
    pub sys: u8,
    /// The component id.
    pub comp: u8,
    /// The message id.
    pub msg: u32,
    /// The payload as sent (a v2 sender leaves out trailing zero bytes).
    pub payload: Vec<u8>,
    /// A v2 frame with a signature (not checked).
    pub signed: bool,
}

impl Frame {
    /// The byte at `off` of the payload, 0 past what was sent.
    pub fn byte(&self, off: usize) -> u8 {
        self.payload.get(off).copied().unwrap_or(0)
    }

    fn bytes<const N: usize>(&self, off: usize) -> [u8; N] {
        std::array::from_fn(|i| self.byte(off + i))
    }

    /// A little-endian `u16` at `off`.
    pub fn u16(&self, off: usize) -> u16 {
        u16::from_le_bytes(self.bytes(off))
    }
    /// A little-endian `i16` at `off`.
    pub fn i16(&self, off: usize) -> i16 {
        i16::from_le_bytes(self.bytes(off))
    }
    /// A little-endian `u32` at `off`.
    pub fn u32(&self, off: usize) -> u32 {
        u32::from_le_bytes(self.bytes(off))
    }
    /// A little-endian `i32` at `off`.
    pub fn i32(&self, off: usize) -> i32 {
        i32::from_le_bytes(self.bytes(off))
    }
    /// A little-endian `f32` at `off`.
    pub fn f32(&self, off: usize) -> f32 {
        f32::from_le_bytes(self.bytes(off))
    }

    /// The name of the message, "MSG <id>" for one the stack does not know.
    pub fn name(&self) -> String {
        message(self.msg).map_or_else(|| format!("MSG {}", self.msg), |m| m.name.to_string())
    }
}

/// The checksum of a frame: CRC-16/MCRF4XX over `body` (the bytes after the start byte up to the end of the payload), then
/// over `extra`.
pub fn checksum(body: &[u8], extra: u8) -> u16 {
    let mut d = CRC16_MCRF4XX.digest();
    d.update(body);
    d.update(&[extra]);
    d.finalize()
}

/// The bytes of a frame of version `version` (1 or 2) with message `msg` and `payload`. A v2 frame leaves out the trailing zero
/// bytes of the payload (at least one byte stays). `None` for an id the stack does not know or a v1 id above 255.
pub fn build(version: u8, seq: u8, sys: u8, comp: u8, msg: u32, payload: &[u8]) -> Option<Vec<u8>> {
    let info = message(msg)?;
    let mut out = Vec::with_capacity(payload.len() + 12);
    let p = if version == 2 {
        let keep = payload.iter().rposition(|&b| b != 0).map_or(1, |i| i + 1);
        &payload[..keep.min(payload.len())]
    } else {
        payload
    };
    if version == 1 {
        let id = u8::try_from(msg).ok()?;
        out.extend([STX_V1, p.len() as u8, seq, sys, comp, id]);
    } else {
        let id = msg.to_le_bytes();
        out.extend([STX_V2, p.len() as u8, 0, 0, seq, sys, comp, id[0], id[1], id[2]]);
    }
    out.extend_from_slice(p);
    let c = checksum(&out[1..], info.crc_extra);
    out.extend(c.to_le_bytes());
    Some(out)
}

/// What [`parse_at`] makes of the bytes at a start byte.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Parse {
    /// A whole frame with a good checksum, and its length in bytes.
    Frame(Frame, usize),
    /// Too few bytes yet.
    Short,
    /// Not a frame (a bad checksum, an unknown id or flags this reader does not know).
    Bad,
}

/// Reads the frame that starts at `b[0]`.
pub fn parse_at(b: &[u8]) -> Parse {
    let Some(&stx) = b.first() else {
        return Parse::Short;
    };
    let (head, version) = match stx {
        STX_V1 => (6, 1),
        STX_V2 => (10, 2),
        _ => return Parse::Bad,
    };
    if b.len() < head {
        return Parse::Short;
    }
    let len = usize::from(b[1]);
    let (seq, sys, comp, msg, signed) = if version == 1 {
        (b[2], b[3], b[4], u32::from(b[5]), false)
    } else {
        let incompat = b[2];
        if incompat & !1 != 0 {
            return Parse::Bad;
        }
        let msg = u32::from(b[7]) | u32::from(b[8]) << 8 | u32::from(b[9]) << 16;
        (b[4], b[5], b[6], msg, incompat & 1 == 1)
    };
    let Some(info) = message(msg) else {
        return Parse::Bad;
    };
    let total = head + len + 2 + if signed { 13 } else { 0 };
    if b.len() < total {
        return Parse::Short;
    }
    let c = u16::from_le_bytes([b[head + len], b[head + len + 1]]);
    if checksum(&b[1..head + len], info.crc_extra) != c {
        return Parse::Bad;
    }
    Parse::Frame(
        Frame {
            version,
            seq,
            sys,
            comp,
            msg,
            payload: b[head..head + len].to_vec(),
            signed,
        },
        total,
    )
}

/// The bytes of one link, gathered over its packets, and the frames found in them.
#[derive(Clone, Debug, Default)]
pub struct Stream {
    buf: Vec<u8>,
}

impl Stream {
    /// An empty stream.
    pub fn new() -> Stream {
        Stream {
            buf: Vec::with_capacity(2 * STREAM_CAP),
        }
    }

    /// Bytes waiting for the rest of a frame.
    pub fn waiting(&self) -> usize {
        self.buf.len()
    }

    /// Appends `data` and gives every whole frame to `found`. Bytes that start no frame are dropped; the bytes from the first
    /// start byte of a frame not yet whole wait. More than [`STREAM_CAP`] waiting bytes are dropped.
    pub fn push(&mut self, data: &[u8], mut found: impl FnMut(Frame)) {
        self.buf.extend_from_slice(data);
        let mut i = 0;
        let mut keep = self.buf.len();
        while i < self.buf.len() {
            match parse_at(&self.buf[i..]) {
                Parse::Frame(f, n) => {
                    found(f);
                    i += n;
                }
                Parse::Short => {
                    keep = i;
                    break;
                }
                Parse::Bad => i += 1,
            }
        }
        if i >= self.buf.len() {
            keep = self.buf.len();
        }
        self.buf.drain(..keep);
        if self.buf.len() > STREAM_CAP {
            self.buf.clear();
        }
    }
}

/// The fields of a frame of a message the stack describes, as one line of text, and the position it holds.
#[derive(Clone, Debug, PartialEq)]
pub struct Described {
    /// The line.
    pub text: String,
    /// Latitude, longitude (degrees) and altitude (m), when the frame gives a position worth a map point.
    pub position: Option<(f64, f64, f64)>,
}

/// Describes frame `f` (MAVLink message definitions).
pub fn describe(f: &Frame) -> Described {
    let mut position = None;
    let text = match f.msg {
        0 => {
            let base = f.byte(6);
            format!(
                "type {}, autopilot {}, base mode {:02X}, status {}, {}",
                f.byte(4),
                f.byte(5),
                base,
                f.byte(7),
                if base & 0x80 != 0 { "armed" } else { "not armed" }
            )
        }
        1 => format!(
            "battery {:.2} V, {} %",
            f64::from(f.u16(14)) / 1000.0,
            f.byte(30) as i8
        ),
        24 => {
            let (lat, lon) = (f64::from(f.i32(8)) * 1e-7, f64::from(f.i32(12)) * 1e-7);
            let alt = f64::from(f.i32(16)) / 1000.0;
            let fix = f.byte(28);
            if fix >= 2 && f.i32(8) != 0 {
                position = Some((lat, lon, alt));
            }
            format!(
                "fix {fix}, {} satellites, {lat:.5} {lon:.5}, {alt:.0} m",
                f.byte(29)
            )
        }
        30 => format!(
            "roll {:.1}°, pitch {:.1}°, yaw {:.1}°",
            f64::from(f.f32(4)).to_degrees(),
            f64::from(f.f32(8)).to_degrees(),
            f64::from(f.f32(12)).to_degrees()
        ),
        33 => {
            let (lat, lon) = (f64::from(f.i32(4)) * 1e-7, f64::from(f.i32(8)) * 1e-7);
            let alt = f64::from(f.i32(12)) / 1000.0;
            let rel = f64::from(f.i32(16)) / 1000.0;
            if f.i32(4) != 0 || f.i32(8) != 0 {
                position = Some((lat, lon, alt));
            }
            format!("{lat:.5} {lon:.5}, {alt:.0} m, {rel:.0} m above home")
        }
        74 => format!(
            "airspeed {:.1} m/s, ground speed {:.1} m/s, altitude {:.0} m, climb {:.1} m/s, heading {}°",
            f.f32(0),
            f.f32(4),
            f.f32(8),
            f.f32(12),
            f.i16(16)
        ),
        253 => {
            let t: String = (1..51)
                .map(|i| f.byte(i))
                .take_while(|&c| c != 0)
                .map(|c| if (32..127).contains(&c) { c as char } else { '?' })
                .collect();
            format!("severity {}: {t}", f.byte(0))
        }
        _ => format!("{} payload bytes", f.payload.len()),
    };
    let text = if f.signed {
        format!("{text}, signed")
    } else {
        text
    };
    Described { text, position }
}

/// The payload of a GLOBAL_POSITION_INT at `lat`, `lon` (degrees) and `alt_m` (also the height above home), at rest.
pub fn global_position(time_ms: u32, lat: f64, lon: f64, alt_m: f64) -> [u8; 28] {
    let mut p = [0u8; 28];
    p[0..4].copy_from_slice(&time_ms.to_le_bytes());
    p[4..8].copy_from_slice(&((lat * 1e7).round() as i32).to_le_bytes());
    p[8..12].copy_from_slice(&((lon * 1e7).round() as i32).to_le_bytes());
    p[12..16].copy_from_slice(&((alt_m * 1000.0).round() as i32).to_le_bytes());
    p[16..20].copy_from_slice(&((alt_m * 1000.0).round() as i32).to_le_bytes());
    p[26..28].copy_from_slice(&u16::MAX.to_le_bytes());
    p
}
