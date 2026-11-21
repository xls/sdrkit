//! Wireless M-Bus frames: the CRC blocks and the link layer of EN 13757-4, and the application data records of EN 13757-3.
//!
//! Standard: EN 13757-4 (link layer, frame formats A and B), EN 13757-3 (application layer)

use sdrkit_util_bits::crc::catalog::CRC16_EN_13757;
use serde::{Deserialize, Serialize};

/// The three-letter manufacturer code.
pub fn manufacturer(m: u16) -> String {
    let c = |s: u16| char::from(64 + ((m >> s) & 31) as u8);
    [c(10), c(5), c(0)].iter().collect()
}

/// The 15-bit code of three letters.
pub fn manufacturer_code(s: &str) -> u16 {
    let b = s.as_bytes();
    ((b[0] as u16 - 64) << 10) | ((b[1] as u16 - 64) << 5) | (b[2] as u16 - 64)
}

/// The device type name, `None` for a reserved type.
pub fn device_type_name(t: u8) -> Option<&'static str> {
    Some(match t {
        0x00 => "other",
        0x01 => "oil",
        0x02 => "electricity",
        0x03 => "gas",
        0x04 => "heat (outlet)",
        0x05 => "steam",
        0x06 => "warm water",
        0x07 => "water",
        0x08 => "heat cost allocator",
        0x09 => "compressed air",
        0x0A => "cooling (outlet)",
        0x0B => "cooling (inlet)",
        0x0C => "heat (inlet)",
        0x0D => "heat / cooling",
        0x0E => "bus / system",
        0x0F => "unknown",
        0x15 => "hot water",
        0x16 => "cold water",
        0x17 => "dual water",
        0x18 => "pressure",
        0x19 => "A/D converter",
        0x1A => "smoke detector",
        0x1B => "room sensor",
        0x1C => "gas detector",
        0x20 => "breaker (electricity)",
        0x21 => "valve (gas or water)",
        0x25 => "customer unit",
        0x28 => "waste water",
        0x29 => "garbage",
        0x2A => "carbon dioxide",
        0x31 => "communication controller",
        0x32 => "unidirectional repeater",
        0x33 => "bidirectional repeater",
        0x36 => "radio converter (system)",
        0x37 => "radio converter (meter)",
        _ => return None,
    })
}

/// The C-field name.
pub fn c_name(c: u8) -> &'static str {
    match c {
        0x44 => "SND-NR",
        0x46 => "SND-IR",
        0x47 => "ACC-NR",
        0x48 => "ACC-DMD",
        0x08 => "RSP-UD",
        0x40 => "SND-NKE",
        0x43 | 0x53 | 0x73 => "SND-UD",
        0x5A | 0x7A => "REQ-UD1",
        0x5B | 0x7B => "REQ-UD2",
        _ => "other",
    }
}

/// The CI name.
pub fn ci_name(ci: u8) -> &'static str {
    match ci {
        0x72 => "long header, variable data",
        0x7A => "short header, variable data",
        0x78 => "variable data, no header",
        0x7B => "compact data, short header",
        0x79 => "compact data, no header",
        0x8C => "extended link layer (short)",
        0x8D => "extended link layer (with payload CRC)",
        0x51 => "data send",
        0x52 => "selection of slaves",
        0x70 => "application error",
        0x71 => "alarm",
        0x69..=0x6B => "reduced format",
        0x90 => "authentication / fragmentation",
        0xA0..=0xB7 => "manufacturer specific",
        _ => "other",
    }
}

/// Frame format.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Format {
    /// Format A: blocks of 10 then 16 bytes, each with its CRC.
    A,
    /// Format B: one or two blocks.
    B,
}

/// The number of bytes on air for a frame of length octet `l` (CRCs included), `None` for a bad length.
pub fn air_len(l: u8, f: Format) -> Option<usize> {
    let n = l as usize + 1;
    match f {
        Format::A => Some(if n <= 10 {
            n + 2
        } else {
            n + 2 + 2 * (n - 10).div_ceil(16)
        }),
        Format::B => {
            if n <= 3 || n == 129 || n == 130 {
                None
            } else {
                Some(n)
            }
        }
    }
}

/// Checks the CRCs of a frame on air and removes them. `None` when a CRC fails or the length is bad.
pub fn strip_crcs(air: &[u8], f: Format) -> Option<Vec<u8>> {
    let l = *air.first()?;
    let total = air_len(l, f)?;
    if air.len() < total {
        return None;
    }
    let mut out = Vec::with_capacity(total);
    // Each block is its data and its CRC, sent most significant byte first.
    let mut blk = |d: &[u8], c: &[u8]| -> bool {
        if CRC16_EN_13757.checksum(d) != u16::from_be_bytes([c[0], c[1]]) {
            return false;
        }
        out.extend_from_slice(d);
        true
    };
    match f {
        Format::A => {
            let n = l as usize + 1;
            let first = n.min(10);
            if !blk(&air[..first], &air[first..first + 2]) {
                return None;
            }
            let mut pos = first + 2;
            let mut left = n - first;
            while left > 0 {
                let k = left.min(16);
                if !blk(&air[pos..pos + k], &air[pos + k..pos + k + 2]) {
                    return None;
                }
                pos += k + 2;
                left -= k;
            }
        }
        Format::B => {
            let n = l as usize + 1;
            if n <= 128 {
                if !blk(&air[..n - 2], &air[n - 2..n]) {
                    return None;
                }
            } else {
                if !blk(&air[..126], &air[126..128]) {
                    return None;
                }
                if !blk(&air[128..n - 2], &air[n - 2..n]) {
                    return None;
                }
            }
        }
    }
    Some(out)
}

/// Adds the CRCs to frame bytes (length octet first) for the air.
pub fn add_crcs(data: &[u8], f: Format) -> Vec<u8> {
    let mut out = Vec::new();
    let mut put = |d: &[u8]| {
        out.extend_from_slice(d);
        let c = CRC16_EN_13757.checksum(d);
        out.extend_from_slice(&c.to_be_bytes());
    };
    match f {
        Format::A => {
            let first = data.len().min(10);
            put(&data[..first]);
            for c in data[first..].chunks(16) {
                put(c);
            }
        }
        Format::B => {
            if data.len() <= 126 {
                put(data);
            } else {
                put(&data[..126]);
                put(&data[126..]);
            }
        }
    }
    out
}

/// One data record.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Record {
    /// The quantity ("Volume").
    pub quantity: String,
    /// The value with its unit ("123.456 m3").
    pub value: String,
    /// Notes ("storage 1").
    pub notes: Vec<String>,
}

impl Record {
    /// "Quantity value (notes)".
    pub fn text(&self) -> String {
        if self.notes.is_empty() {
            format!("{} {}", self.quantity, self.value)
        } else {
            format!(
                "{} {} ({})",
                self.quantity,
                self.value,
                self.notes.join(", ")
            )
        }
    }
}

/// A parsed frame (after CRC removal).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Parsed {
    /// L.
    pub l: u8,
    /// C.
    pub c: u8,
    /// Manufacturer letters.
    pub manufacturer: String,
    /// Id (8 digits).
    pub id: String,
    /// Version.
    pub version: u8,
    /// Device type.
    pub dev_type: u8,
    /// The CI field (the real one after an extended link layer).
    pub ci: u8,
    /// Encrypted.
    pub encrypted: bool,
    /// Access number, when the application header has one.
    pub access: Option<u8>,
    /// Status text.
    pub status: String,
    /// The records.
    pub records: Vec<Record>,
    /// Manufacturer data seen.
    pub mfr_data: bool,
    /// A parse error ("record cut off").
    pub error: Option<String>,
}

impl Parsed {
    /// The device type name or its hex.
    pub fn type_name(&self) -> String {
        device_type_name(self.dev_type)
            .map(String::from)
            .unwrap_or_else(|| "reserved".into())
    }
    /// The summary of the frame log.
    pub fn summary(&self) -> String {
        if let Some(e) = &self.error {
            if self.records.is_empty() {
                return e.clone();
            }
        }
        if self.encrypted {
            return "encrypted payload".into();
        }
        if !self.records.is_empty() {
            return self
                .records
                .iter()
                .take(4)
                .map(|r| format!("{} {}", r.quantity, r.value))
                .collect::<Vec<_>>()
                .join("; ");
        }
        if self.mfr_data {
            return "manufacturer data".into();
        }
        if matches!(self.ci, 0x7A | 0x72 | 0x78) {
            "no data".into()
        } else {
            ci_name(self.ci).into()
        }
    }
}

fn status_text(s: u8) -> String {
    let mut t = ["ok", "busy", "application error", "alarm"][(s & 3) as usize].to_string();
    if s & 4 != 0 {
        t.push_str(", low power");
    }
    if s & 8 != 0 {
        t.push_str(", permanent error");
    }
    if s & 16 != 0 {
        t.push_str(", temporary error");
    }
    t
}

/// Parses a frame after CRC removal.
pub fn parse(d: &[u8]) -> Option<Parsed> {
    if d.len() < 11 {
        return None;
    }
    let id = format!("{:02X}{:02X}{:02X}{:02X}", d[7], d[6], d[5], d[4]);
    let mut p = Parsed {
        l: d[0],
        c: d[1],
        manufacturer: manufacturer((d[3] as u16) << 8 | d[2] as u16),
        id,
        version: d[8],
        dev_type: d[9],
        ci: d[10],
        encrypted: false,
        access: None,
        status: String::new(),
        records: vec![],
        mfr_data: false,
        error: None,
    };
    let mut i = 11;
    let mut ci = d[10];
    if ci == 0x8C {
        if d.len() < i + 3 {
            p.error = Some("record cut off".into());
            return Some(p);
        }
        ci = d[i + 2];
        i += 3;
    } else if ci == 0x8D {
        if d.len() < i + 6 {
            p.error = Some("record cut off".into());
            return Some(p);
        }
        let sn = u32::from_le_bytes([d[i + 2], d[i + 3], d[i + 4], d[i + 5]]);
        if sn >> 29 != 0 {
            p.encrypted = true;
            return Some(p);
        }
        if d.len() < i + 9 {
            p.error = Some("record cut off".into());
            return Some(p);
        }
        ci = d[i + 8];
        i += 9;
    }
    p.ci = ci;
    let hdr = match ci {
        0x7A | 0x7B => 4,
        0x72 => 12,
        _ => 0,
    };
    if hdr > 0 {
        if d.len() < i + hdr {
            p.error = Some("record cut off".into());
            return Some(p);
        }
        let h = &d[i + hdr - 4..i + hdr];
        p.access = Some(h[0]);
        p.status = status_text(h[1]);
        let cfg = u16::from_le_bytes([h[2], h[3]]);
        if (cfg >> 8) & 0x1F != 0 {
            p.encrypted = true;
            return Some(p);
        }
        i += hdr;
    }
    if !matches!(ci, 0x7A | 0x72 | 0x78) {
        return Some(p);
    }
    records(&d[i..], &mut p);
    Some(p)
}

fn unit_time(n: u8) -> &'static str {
    ["s", "min", "h", "d"][(n & 3) as usize]
}

fn records(d: &[u8], p: &mut Parsed) {
    let mut i = 0;
    while i < d.len() {
        let dif = d[i];
        if dif == 0x2F {
            i += 1;
            continue;
        }
        if matches!(dif, 0x0F | 0x1F | 0x7F) {
            p.mfr_data = true;
            return;
        }
        i += 1;
        let mut notes = Vec::new();
        let mut storage = ((dif >> 6) & 1) as u32;
        let (mut tariff, mut subunit) = (0u32, 0u32);
        let mut ext = dif & 0x80 != 0;
        let mut k = 0;
        while ext {
            if i >= d.len() || k >= 10 {
                p.error = Some("record cut off".into());
                return;
            }
            let e = d[i];
            storage |= ((e & 0xF) as u32) << (1 + 4 * k);
            tariff |= (((e >> 4) & 3) as u32) << (2 * k);
            subunit |= (((e >> 6) & 1) as u32) << k;
            ext = e & 0x80 != 0;
            i += 1;
            k += 1;
        }
        if i >= d.len() {
            p.error = Some("record cut off".into());
            return;
        }
        let vif = d[i];
        i += 1;
        let mut quantity: String;
        let mut unit = "";
        let mut exp: i32 = 0;
        let mut kind = 0u8; // 0 number, 1 date G, 2 date F
        let v = vif & 0x7F;
        let mut skip_vife = vif & 0x80 != 0;
        if vif == 0xFD || vif == 0xFB {
            if i >= d.len() {
                p.error = Some("record cut off".into());
                return;
            }
            let x = d[i];
            i += 1;
            skip_vife = x & 0x80 != 0;
            quantity = if vif == 0xFD {
                match x & 0x7F {
                    0x08 => "Access number".into(),
                    0x09 => "Medium".into(),
                    0x0A => "Manufacturer".into(),
                    0x0B => "Parameter set identification".into(),
                    0x0C => "Model / version".into(),
                    0x0D => "Hardware version".into(),
                    0x0E => "Firmware version".into(),
                    0x0F => "Software version".into(),
                    0x10 => "Customer location".into(),
                    0x11 => "Customer".into(),
                    0x17 => "Error flags".into(),
                    0x74 => {
                        unit = "days";
                        "Remaining battery life".into()
                    }
                    _ => format!("VIF FD {x:02X}"),
                }
            } else {
                format!("VIF FB {x:02X}")
            };
        } else {
            let n = (v & 7) as i32;
            quantity = match v {
                0x00..=0x07 => {
                    unit = "Wh";
                    exp = n - 3;
                    "Energy".into()
                }
                0x08..=0x0F => {
                    unit = "J";
                    exp = n;
                    "Energy".into()
                }
                0x10..=0x17 => {
                    unit = "m3";
                    exp = n - 6;
                    "Volume".into()
                }
                0x18..=0x1F => {
                    unit = "kg";
                    exp = n - 3;
                    "Mass".into()
                }
                0x20..=0x23 => {
                    unit = unit_time(v);
                    "On time".into()
                }
                0x24..=0x27 => {
                    unit = unit_time(v);
                    "Operating time".into()
                }
                0x28..=0x2F => {
                    unit = "W";
                    exp = n - 3;
                    "Power".into()
                }
                0x30..=0x37 => {
                    unit = "J/h";
                    exp = n;
                    "Power".into()
                }
                0x38..=0x3F => {
                    unit = "m3/h";
                    exp = n - 6;
                    "Volume flow".into()
                }
                0x40..=0x47 => {
                    unit = "m3/min";
                    exp = n - 7;
                    "Volume flow".into()
                }
                0x48..=0x4F => {
                    unit = "m3/s";
                    exp = n - 9;
                    "Volume flow".into()
                }
                0x50..=0x57 => {
                    unit = "kg/h";
                    exp = n - 3;
                    "Mass flow".into()
                }
                0x58..=0x5B => {
                    unit = "C";
                    exp = (v & 3) as i32 - 3;
                    "Flow temperature".into()
                }
                0x5C..=0x5F => {
                    unit = "C";
                    exp = (v & 3) as i32 - 3;
                    "Return temperature".into()
                }
                0x60..=0x63 => {
                    unit = "K";
                    exp = (v & 3) as i32 - 3;
                    "Temperature difference".into()
                }
                0x64..=0x67 => {
                    unit = "C";
                    exp = (v & 3) as i32 - 3;
                    "External temperature".into()
                }
                0x68..=0x6B => {
                    unit = "bar";
                    exp = (v & 3) as i32 - 3;
                    "Pressure".into()
                }
                0x6C => {
                    kind = 1;
                    "Date".into()
                }
                0x6D => {
                    kind = 2;
                    "Date and time".into()
                }
                0x6E => "Units for heat cost allocators".into(),
                0x70..=0x73 => {
                    unit = unit_time(v);
                    "Averaging duration".into()
                }
                0x74..=0x77 => {
                    unit = unit_time(v);
                    "Actuality duration".into()
                }
                0x78 => "Fabrication number".into(),
                0x79 => "Enhanced identification".into(),
                0x7A => "Bus address".into(),
                0x7C => {
                    if i >= d.len() {
                        p.error = Some("record cut off".into());
                        return;
                    }
                    let n = d[i] as usize;
                    if i + 1 + n > d.len() {
                        p.error = Some("record cut off".into());
                        return;
                    }
                    let s: String = d[i + 1..i + 1 + n]
                        .iter()
                        .rev()
                        .map(|&c| c as char)
                        .collect();
                    i += 1 + n;
                    s
                }
                0x7F => "manufacturer specific".into(),
                _ => format!("VIF {vif:02X}"),
            };
        }
        let mut k = 0;
        while skip_vife {
            if i >= d.len() || k >= 10 {
                p.error = Some("record cut off".into());
                return;
            }
            skip_vife = d[i] & 0x80 != 0;
            i += 1;
            k += 1;
        }
        let code = dif & 0x0F;
        let len = match code {
            0 | 8 => 0,
            1 | 9 => 1,
            2 | 0xA => 2,
            3 | 0xB => 3,
            4 | 5 | 0xC => 4,
            6 | 0xE => 6,
            7 => 8,
            0xD => {
                if i >= d.len() {
                    p.error = Some("record cut off".into());
                    return;
                }
                let lb = d[i];
                if lb >= 0xC0 {
                    p.mfr_data = true;
                    return;
                }
                i += 1;
                lb as usize
            }
            _ => {
                return;
            }
        };
        if i + len > d.len() {
            p.error = Some("record cut off".into());
            return;
        }
        let data = &d[i..i + len];
        i += len;
        let value = match code {
            0 | 8 | 0xF => "-".to_string(),
            0xD => data
                .iter()
                .rev()
                .map(|&c| {
                    if (0x20..0x7F).contains(&c) {
                        c as char
                    } else {
                        '.'
                    }
                })
                .collect(),
            _ => {
                if kind == 1 && data.len() >= 2 {
                    let (p0, p1) = (data[0], data[1]);
                    let y = 2000 + (((p0 & 0xE0) >> 5) | ((p1 & 0xF0) >> 1)) as u32;
                    format!("{y:04}-{:02}-{:02}", p1 & 15, p0 & 31)
                } else if kind == 2 && data.len() >= 4 {
                    let (p0, p1, p2, p3) = (data[0], data[1], data[2], data[3]);
                    let y = 2000 + (((p2 & 0xE0) >> 5) | ((p3 & 0xF0) >> 1)) as u32;
                    format!(
                        "{y:04}-{:02}-{:02} {:02}:{:02}",
                        p3 & 15,
                        p2 & 31,
                        p1 & 31,
                        p0 & 63
                    )
                } else {
                    let num: Option<f64> = match code {
                        1..=4 | 6 | 7 => {
                            let mut v: i64 = if data.last().is_some_and(|b| b & 0x80 != 0) {
                                -1
                            } else {
                                0
                            };
                            for &b in data.iter().rev() {
                                v = (v << 8) | b as i64;
                            }
                            Some(v as f64)
                        }
                        5 => Some(f32::from_le_bytes([data[0], data[1], data[2], data[3]]) as f64),
                        // EN 13757-3 type A: little-endian BCD, a top nibble 0xF in the last byte is the sign.
                        9..=0xC | 0xE => sdrkit_util_bits::bcd::decode_le(data)
                            .ok()
                            .map(|v| v as f64),
                        _ => None,
                    };
                    match num {
                        Some(x) => {
                            let val = x * 10f64.powi(exp);
                            let s = if exp < 0 {
                                format!("{:.*}", (-exp).min(9) as usize, val)
                            } else {
                                format!("{}", val.round() as i64)
                            };
                            if unit.is_empty() {
                                s
                            } else {
                                format!("{s} {unit}")
                            }
                        }
                        None => "-".into(),
                    }
                }
            }
        };
        if storage > 0 {
            notes.push(format!("storage {storage}"));
        }
        if tariff > 0 {
            notes.push(format!("tariff {tariff}"));
        }
        if subunit > 0 {
            notes.push(format!("subunit {subunit}"));
        }
        match (dif >> 4) & 3 {
            1 => notes.push("max".into()),
            2 => notes.push("min".into()),
            3 => notes.push("error".into()),
            _ => {}
        }
        quantity.shrink_to_fit();
        p.records.push(Record {
            quantity,
            value,
            notes,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codes_and_names() {
        assert_eq!(manufacturer(0x2C2D), "KAM");
        assert_eq!(manufacturer_code("KAM"), 0x2C2D);
    }

    #[test]
    fn air_lengths() {
        assert_eq!(air_len(9, Format::A), Some(12));
        assert_eq!(air_len(10, Format::A), Some(15));
        assert_eq!(air_len(25, Format::A), Some(30));
        assert_eq!(air_len(26, Format::A), Some(33));
        assert_eq!(air_len(200, Format::A), Some(201 + 2 + 2 * 12));
        assert_eq!(air_len(127, Format::B), Some(128));
        assert_eq!(air_len(200, Format::B), Some(201));
        assert_eq!(air_len(128, Format::B), None);
        assert_eq!(air_len(129, Format::B), None);
    }
}
