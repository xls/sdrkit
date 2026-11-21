//! Horus packets: the binary payloads of v1 and v2 with their CRC (horusdemodlib wiki, "4 Packet Format Details"), the custom
//! fields of v2 ("5 Customising a Horus Binary v2 Packet"), the UKHAS sentence of the RTTY packets and of the binary ones
//! written as text, the payload ID list and the list of custom formats that the consumer gives.

use sdrkit_util_bits::crc::catalog::CRC16_IBM_3740;
use std::collections::BTreeMap;
use std::fmt::Write;

/// A binary payload of v1 (22 bytes) or v2 (32 bytes).
#[derive(Clone, Debug, PartialEq)]
pub struct Binary {
    /// 1 or 2.
    pub version: u8,
    /// The payload ID: 0 to 255 in v1, 0 to 65535 in v2.
    pub id: u16,
    /// The sequence number.
    pub seq: u16,
    /// The time of day, UTC: hours, minutes, seconds.
    pub time: (u8, u8, u8),
    /// Latitude, degrees (0 when the value is not a finite number).
    pub lat: f64,
    /// Longitude, degrees (0 when the value is not a finite number).
    pub lon: f64,
    /// Altitude, m.
    pub alt_m: u16,
    /// Speed, km/h.
    pub speed_kmh: u8,
    /// Satellites.
    pub sats: u8,
    /// Temperature, degrees C.
    pub temp_c: i8,
    /// The battery byte (V = raw x 5 / 255).
    pub battery: u8,
    /// The 9 custom bytes of v2.
    pub custom: Option<[u8; 9]>,
}

impl Binary {
    /// The battery voltage.
    pub fn battery_v(&self) -> f64 {
        f64::from(self.battery) * 5.0 / 255.0
    }

    /// A position of exactly 0, 0 means no fix.
    pub fn has_fix(&self) -> bool {
        !(self.lat == 0.0 && self.lon == 0.0)
    }

    /// The bytes of the payload, the CRC last (low byte first).
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut b = Vec::with_capacity(32);
        if self.version == 1 {
            b.push(self.id as u8);
        } else {
            b.extend_from_slice(&self.id.to_le_bytes());
        }
        b.extend_from_slice(&self.seq.to_le_bytes());
        b.extend_from_slice(&[self.time.0, self.time.1, self.time.2]);
        b.extend_from_slice(&(self.lat as f32).to_le_bytes());
        b.extend_from_slice(&(self.lon as f32).to_le_bytes());
        b.extend_from_slice(&self.alt_m.to_le_bytes());
        b.extend_from_slice(&[self.speed_kmh, self.sats, self.temp_c as u8, self.battery]);
        if self.version != 1 {
            b.extend_from_slice(&self.custom.unwrap_or([0; 9]));
        }
        let crc = CRC16_IBM_3740.checksum(&b);
        b.extend_from_slice(&crc.to_le_bytes());
        b
    }

    /// The payload of `b` (22 or 32 bytes) when its CRC is good.
    pub fn parse(b: &[u8]) -> Option<Binary> {
        let version = match b.len() {
            22 => 1,
            32 => 2,
            _ => return None,
        };
        let n = b.len();
        if CRC16_IBM_3740.checksum(&b[..n - 2]) != u16::from_le_bytes([b[n - 2], b[n - 1]]) {
            return None;
        }
        let (id, r) = if version == 1 {
            (u16::from(b[0]), &b[1..])
        } else {
            (u16::from_le_bytes([b[0], b[1]]), &b[2..])
        };
        let f = |o: usize| {
            let v = f64::from(f32::from_le_bytes([r[o], r[o + 1], r[o + 2], r[o + 3]]));
            if v.is_finite() {
                v
            } else {
                0.0
            }
        };
        let custom = (version == 2).then(|| {
            let mut c = [0u8; 9];
            c.copy_from_slice(&r[19..28]);
            c
        });
        Some(Binary {
            version,
            id,
            seq: u16::from_le_bytes([r[0], r[1]]),
            time: (r[2], r[3], r[4]),
            lat: f(5),
            lon: f(9),
            alt_m: u16::from_le_bytes([r[13], r[14]]),
            speed_kmh: r[15],
            sats: r[16],
            temp_c: r[17] as i8,
            battery: r[18],
            custom,
        })
    }

    /// The packet as a sentence: `$$CALL,seq,HH:MM:SS,lat,lon,alt,speed,sats,temp,batt[,custom values]*CCCC`.
    pub fn sentence(&self, callsign: &str, custom: &[(String, String)]) -> String {
        let mut s = format!(
            "{callsign},{},{:02}:{:02}:{:02},{:.5},{:.5},{},{},{},{},{:.2}",
            self.seq,
            self.time.0,
            self.time.1,
            self.time.2,
            self.lat,
            self.lon,
            self.alt_m,
            self.speed_kmh,
            self.sats,
            self.temp_c,
            self.battery_v()
        );
        for (_, v) in custom {
            s.push(',');
            s.push_str(v);
        }
        with_checksum(&s)
    }
}

/// `$$text*CCCC`: the checksum is CRC-16/IBM-3740 of `text`, 4 upper-case hex digits.
pub fn with_checksum(text: &str) -> String {
    format!("$${text}*{:04X}", CRC16_IBM_3740.checksum(text.as_bytes()))
}

/// How a custom field is written.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Post {
    /// Integers as plain decimals, floats with 6 decimals.
    #[default]
    None,
    /// The value x 5 / 255, 2 decimals.
    Battery5vByte,
    /// The value / 10.
    DivideBy10,
    /// The value / 100.
    DivideBy100,
}

impl Post {
    /// The post-processing of a name; an unknown name acts as `none`.
    pub fn of(name: &str) -> Post {
        match name.trim() {
            "battery_5v_byte" => Post::Battery5vByte,
            "divide_by_10" => Post::DivideBy10,
            "divide_by_100" => Post::DivideBy100,
            _ => Post::None,
        }
    }
}

/// The custom field format of one callsign.
#[derive(Clone, Debug, PartialEq)]
pub struct CustomFormat {
    /// The callsign.
    pub callsign: String,
    /// A comment.
    pub comment: String,
    /// The layout of the 9 custom bytes.
    pub structure: String,
    /// The name and post-processing of each item that is not a pad byte.
    pub fields: Vec<(String, Post)>,
    /// The names of the RTTY sentence fields after the altitude.
    pub sentence: Vec<String>,
    /// Other payload IDs that share this format.
    pub other_payloads: Vec<u16>,
}

/// The format of the RS41ng tracker: the default content, and the format of a callsign that has none.
pub fn rs41ng() -> CustomFormat {
    CustomFormat {
        callsign: "4FSKTEST-V2".into(),
        comment: "Default custom fields for RS41ng".into(),
        structure: "<hhBHxx".into(),
        fields: vec![
            ("ascent_rate".into(), Post::DivideBy100),
            ("ext_temperature".into(), Post::DivideBy10),
            ("ext_humidity".into(), Post::None),
            ("ext_pressure".into(), Post::DivideBy10),
        ],
        sentence: Vec::new(),
        other_payloads: Vec::new(),
    }
}

/// One item of a struct layout.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Item {
    U8,
    I8,
    U16,
    I16,
    F32,
    Pad,
}

impl Item {
    fn size(self) -> usize {
        match self {
            Item::U8 | Item::I8 | Item::Pad => 1,
            Item::U16 | Item::I16 => 2,
            Item::F32 => 4,
        }
    }
}

/// The items and the byte order of a layout, or `None` for an unknown character or a repeat over 64.
fn layout(s: &str) -> Option<(bool, Vec<Item>)> {
    let mut big = false;
    let mut items = Vec::new();
    let mut count: Option<usize> = None;
    for c in s.chars().filter(|c| !c.is_whitespace()) {
        if let Some(d) = c.to_digit(10) {
            let n = count.unwrap_or(0) * 10 + d as usize;
            if n > 64 {
                return None;
            }
            count = Some(n);
            continue;
        }
        let item = match c {
            '<' | '=' => {
                big = false;
                continue;
            }
            '>' | '!' => {
                big = true;
                continue;
            }
            'B' => Item::U8,
            'b' => Item::I8,
            'H' => Item::U16,
            'h' => Item::I16,
            'f' => Item::F32,
            'x' => Item::Pad,
            _ => return None,
        };
        items.extend(std::iter::repeat_n(item, count.take().unwrap_or(1)));
    }
    Some((big, items))
}

/// The shortest decimal that reads back as `v`, with ".0" after a whole number.
fn short(v: f64) -> String {
    let s = format!("{v}");
    if s.contains('.') || s.contains('e') || !v.is_finite() {
        s
    } else {
        format!("{s}.0")
    }
}

/// The custom fields of the 9 bytes `b` by the format `fmt` (the RS41ng format when there is none): (name, value). One field
/// "custom" with the bytes as 18 hex digits when the layout needs more than 9 bytes or has an unknown character.
pub fn custom_fields(fmt: Option<&CustomFormat>, b: &[u8; 9]) -> Vec<(String, String)> {
    let default;
    let fmt = match fmt {
        Some(f) => f,
        None => {
            default = rs41ng();
            &default
        }
    };
    let raw = || {
        let mut h = String::new();
        for x in b {
            let _ = write!(h, "{x:02X}");
        }
        vec![("custom".to_string(), h)]
    };
    let Some((big, items)) = layout(&fmt.structure) else {
        return raw();
    };
    if items.iter().map(|i| i.size()).sum::<usize>() > b.len() {
        return raw();
    }
    let mut out = Vec::new();
    let mut at = 0;
    let mut k = 0;
    for item in items {
        let bytes = &b[at..at + item.size()];
        at += item.size();
        if item == Item::Pad {
            continue;
        }
        let two = |x: &[u8]| if big { [x[0], x[1]] } else { [x[1], x[0]] };
        let (int, float) = match item {
            Item::U8 => (Some(i64::from(bytes[0])), None),
            Item::I8 => (Some(i64::from(bytes[0] as i8)), None),
            Item::U16 => (Some(i64::from(u16::from_be_bytes(two(bytes)))), None),
            Item::I16 => (Some(i64::from(i16::from_be_bytes(two(bytes)))), None),
            _ => {
                let a = [bytes[0], bytes[1], bytes[2], bytes[3]];
                let v = if big {
                    f32::from_be_bytes(a)
                } else {
                    f32::from_le_bytes(a)
                };
                (None, Some(f64::from(v)))
            }
        };
        let v = float.unwrap_or_else(|| int.unwrap_or(0) as f64);
        k += 1;
        let (name, post) = fmt
            .fields
            .get(k - 1)
            .cloned()
            .unwrap_or_else(|| (format!("field {k}"), Post::None));
        let text = match post {
            Post::None => match int {
                Some(i) => i.to_string(),
                None => format!("{v:.6}"),
            },
            Post::Battery5vByte => format!("{:.2}", v * 5.0 / 255.0),
            Post::DivideBy10 => short(v / 10.0),
            Post::DivideBy100 => short(v / 100.0),
        };
        out.push((name, text));
    }
    out
}

/// The payload ID list: callsign by ID, and the lines that could not be read.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct IdList {
    /// Callsign by payload ID.
    pub ids: BTreeMap<u16, String>,
    /// Lines that do not fit.
    pub unreadable: usize,
}

impl IdList {
    /// Reads one "id, callsign" per line. `#` starts a comment; comma, tab, semicolon and spaces separate; empty lines are
    /// skipped. An ID is 0 to 65535, a callsign one word.
    pub fn parse(text: &str) -> IdList {
        let mut l = IdList::default();
        for line in text.lines() {
            let line = line.split('#').next().unwrap_or("");
            let w: Vec<&str> = line
                .split([',', '\t', ';', ' '])
                .filter(|s| !s.is_empty())
                .collect();
            if w.is_empty() {
                continue;
            }
            match (w.len(), w[0].parse::<u16>()) {
                (2, Ok(id)) => {
                    l.ids.insert(id, w[1].to_string());
                }
                _ => l.unreadable += 1,
            }
        }
        l
    }

    /// The callsign of `id`, or "ID n".
    pub fn callsign(&self, id: u16) -> String {
        self.ids
            .get(&id)
            .cloned()
            .unwrap_or_else(|| format!("ID {id}"))
    }
}

/// Reads the list of custom formats: one format per line, the parts separated by "|": callsign, struct, the fields as
/// "name:post-processing" separated by commas, the sentence names separated by commas, the other payload IDs separated by
/// commas, and a comment. Parts at the end may be left out. `Err` with the count of lines that do not read.
pub fn parse_formats(text: &str) -> Result<Vec<CustomFormat>, usize> {
    let mut out = Vec::new();
    let mut bad = 0;
    for line in text
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
    {
        let p: Vec<&str> = line.split('|').map(str::trim).collect();
        let list = |i: usize| -> Vec<&str> {
            p.get(i)
                .map(|s| {
                    s.split(',')
                        .map(str::trim)
                        .filter(|x| !x.is_empty())
                        .collect()
                })
                .unwrap_or_default()
        };
        let ids: Option<Vec<u16>> = list(4).iter().map(|s| s.parse().ok()).collect();
        let (Some(call), Some(st), Some(ids)) =
            (p.first().filter(|c| !c.is_empty()), p.get(1), ids)
        else {
            bad += 1;
            continue;
        };
        if layout(st).is_none() {
            bad += 1;
            continue;
        }
        out.push(CustomFormat {
            callsign: call.to_string(),
            comment: p.get(5).copied().unwrap_or("").to_string(),
            structure: st.to_string(),
            fields: list(2)
                .iter()
                .map(|f| match f.split_once(':') {
                    Some((n, post)) => (n.trim().to_string(), Post::of(post)),
                    None => (f.to_string(), Post::None),
                })
                .collect(),
            sentence: list(3).iter().map(|s| s.to_string()).collect(),
            other_payloads: ids,
        });
    }
    if bad > 0 {
        Err(bad)
    } else {
        Ok(out)
    }
}

/// The format of `callsign` (or of payload `id` through `other_payloads`) in `formats`.
pub fn format_of<'a>(
    formats: &'a [CustomFormat],
    callsign: &str,
    id: Option<u16>,
) -> Option<&'a CustomFormat> {
    formats
        .iter()
        .find(|f| f.callsign == callsign)
        .or_else(|| id.and_then(|id| formats.iter().find(|f| f.other_payloads.contains(&id))))
}

/// An RTTY sentence.
#[derive(Clone, Debug, PartialEq)]
pub struct Sentence {
    /// The callsign.
    pub callsign: String,
    /// The sequence number.
    pub seq: u32,
    /// The time of day: hours, minutes, seconds.
    pub time: (u8, u8, u8),
    /// Latitude, degrees.
    pub lat: f64,
    /// Longitude, degrees.
    pub lon: f64,
    /// Altitude, m.
    pub alt_m: f64,
    /// The fields after the altitude: (name, value).
    pub fields: Vec<(String, String)>,
    /// The sentence as received, from the first `$`.
    pub text: String,
}

/// Reads `$$CALLSIGN,sequence,HH:MM:SS,lat,lon,alt[,more]*CCCC` (`HHMMSS` too; the checksum in either case). Fields 7 on
/// take the names of `names`, else "field N". `None` when the checksum fails or a field does not read.
pub fn parse_sentence(line: &str, names: &[String]) -> Option<Sentence> {
    let line = line.trim_end_matches(['\r', '\n']);
    let start = line.find('$')?;
    let body = line[start..].trim_start_matches('$');
    let (text, sum) = body.rsplit_once('*')?;
    if sum.len() != 4
        || u16::from_str_radix(sum, 16).ok()? != CRC16_IBM_3740.checksum(text.as_bytes())
    {
        return None;
    }
    let f: Vec<&str> = text.split(',').collect();
    if f.len() < 6 || f[0].is_empty() {
        return None;
    }
    let t: String = f[2].chars().filter(|c| *c != ':').collect();
    if t.len() != 6 || !t.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    let n = |a: usize| t[a..a + 2].parse::<u8>().ok();
    let fields = f[6..]
        .iter()
        .enumerate()
        .map(|(i, v)| {
            let name = names
                .get(i)
                .cloned()
                .unwrap_or_else(|| format!("field {}", i + 7));
            (name, v.to_string())
        })
        .collect();
    Some(Sentence {
        callsign: f[0].to_string(),
        seq: f[1].trim().parse().ok()?,
        time: (n(0)?, n(2)?, n(4)?),
        lat: f[3].trim().parse().ok()?,
        lon: f[4].trim().parse().ok()?,
        alt_m: f[5].trim().parse().ok()?,
        fields,
        text: line[start..].to_string(),
    })
}
