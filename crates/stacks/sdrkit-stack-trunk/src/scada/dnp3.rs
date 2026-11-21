//! The meaning of DNP3 frames: the link header, the transport segments joined into fragments, the application header with
//! its internal indications, the object headers and the values of the point objects.
//!
//! IEEE Std 1815 as the specification states it; the object sizes follow the DNP Users Group note AN2013-004b (group 43 has the
//! sizes of group 42). Secure Authentication (group 120, functions 32, 33 and 131) is shown raw.

use super::modbus::hex;

/// The link header of a frame.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Link {
    /// The length byte.
    pub len: u8,
    /// DIR: from the master.
    pub dir: bool,
    /// PRM: a primary frame.
    pub prm: bool,
    /// FCB.
    pub fcb: bool,
    /// FCV in a primary frame, DFC in a secondary frame.
    pub fcv_dfc: bool,
    /// The link function.
    pub function: u8,
    /// The destination.
    pub dest: u16,
    /// The source.
    pub src: u16,
}

impl Link {
    /// The header of a frame (at least 10 bytes).
    pub fn of(frame: &[u8]) -> Option<Link> {
        if frame.len() < 10 {
            return None;
        }
        let c = frame[3];
        Some(Link {
            len: frame[2],
            dir: c & 0x80 != 0,
            prm: c & 0x40 != 0,
            fcb: c & 0x20 != 0,
            fcv_dfc: c & 0x10 != 0,
            function: c & 0x0F,
            dest: u16::from_le_bytes([frame[4], frame[5]]),
            src: u16::from_le_bytes([frame[6], frame[7]]),
        })
    }

    /// Whether the frame carries user data (a primary frame of function 3 or 4).
    pub fn has_user_data(&self) -> bool {
        self.prm && matches!(self.function, 3 | 4)
    }

    /// The name of the link function.
    pub fn function_name(&self) -> &'static str {
        if self.prm {
            match self.function {
                0 => "reset link states",
                2 => "test link states",
                3 => "confirmed user data",
                4 => "unconfirmed user data",
                9 => "request link status",
                _ => "unknown primary function",
            }
        } else {
            match self.function {
                0 => "ack",
                1 => "nack",
                11 => "link status",
                15 => "not supported",
                _ => "unknown secondary function",
            }
        }
    }

    /// The flags of the control byte as text: FCB and FCV in a primary frame, DFC in a secondary frame.
    pub fn flags(&self) -> String {
        if self.prm {
            format!("FCB {}, FCV {}", u8::from(self.fcb), u8::from(self.fcv_dfc))
        } else {
            format!("DFC {}", u8::from(self.fcv_dfc))
        }
    }
}

/// Whether an address is a broadcast address.
pub fn is_broadcast(a: u16) -> bool {
    a >= 0xFFFD
}

/// The transport byte.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Transport {
    /// The final segment.
    pub fin: bool,
    /// The first segment.
    pub fir: bool,
    /// The sequence, 0 to 63.
    pub seq: u8,
}

impl Transport {
    /// The transport byte `b`.
    pub fn of(b: u8) -> Transport {
        Transport {
            fin: b & 0x80 != 0,
            fir: b & 0x40 != 0,
            seq: b & 0x3F,
        }
    }
}

/// The largest fragment kept, bytes.
pub const MAX_FRAGMENT: usize = 4096;

/// The fragments being joined, one for each source and destination.
#[derive(Clone, Debug, Default)]
pub struct Segments {
    open: Vec<((u16, u16), Vec<u8>, u8)>,
}

impl Segments {
    /// Takes the segment `data` (after the transport byte `t`) from `src` to `dst`. Returns the whole fragment when the
    /// segment is the last, `Ok(None)` while more are to come, and a note when the segment is dropped.
    pub fn push(
        &mut self,
        src: u16,
        dst: u16,
        t: Transport,
        data: &[u8],
    ) -> Result<Option<Vec<u8>>, String> {
        let key = (src, dst);
        let at = self.open.iter().position(|(k, _, _)| *k == key);
        if t.fir {
            if let Some(i) = at {
                self.open.swap_remove(i);
            }
            if t.fin {
                return Ok(Some(data.to_vec()));
            }
            if self.open.len() >= 64 {
                self.open.remove(0);
            }
            self.open.push((key, data.to_vec(), (t.seq + 1) & 0x3F));
            return Ok(None);
        }
        let Some(i) = at else {
            return Err(format!(
                "segment {} came without the first segment; fragment dropped",
                t.seq
            ));
        };
        if self.open[i].2 != t.seq {
            self.open.swap_remove(i);
            return Err(format!(
                "segment {} came out of order; fragment dropped",
                t.seq
            ));
        }
        self.open[i].1.extend_from_slice(data);
        self.open[i].2 = (t.seq + 1) & 0x3F;
        if self.open[i].1.len() > MAX_FRAGMENT {
            self.open.swap_remove(i);
            return Err("fragment over 4096 bytes; dropped".into());
        }
        if t.fin {
            let (_, frag, _) = self.open.swap_remove(i);
            return Ok(Some(frag));
        }
        Ok(None)
    }

    /// Forgets every open fragment.
    pub fn clear(&mut self) {
        self.open.clear();
    }
}

/// The name of an application function.
pub fn app_function_name(f: u8) -> &'static str {
    match f {
        0 => "confirm",
        1 => "read",
        2 => "write",
        3 => "select",
        4 => "operate",
        5 => "direct operate",
        6 => "direct operate no ack",
        7 => "immediate freeze",
        8 => "immediate freeze no ack",
        9 => "freeze and clear",
        10 => "freeze and clear no ack",
        11 => "freeze at time",
        12 => "freeze at time no ack",
        13 => "cold restart",
        14 => "warm restart",
        15 => "initialize data",
        16 => "initialize application",
        17 => "start application",
        18 => "stop application",
        19 => "save configuration",
        20 => "enable unsolicited",
        21 => "disable unsolicited",
        22 => "assign class",
        23 => "delay measurement",
        24 => "record current time",
        25 => "open file",
        26 => "close file",
        27 => "delete file",
        28 => "get file info",
        29 => "authenticate file",
        30 => "abort file",
        31 => "activate config",
        32 => "authentication request",
        33 => "authentication request no ack",
        129 => "response",
        130 => "unsolicited response",
        131 => "authentication response",
        _ => "unknown function",
    }
}

const IIN1: [&str; 8] = [
    "all stations",
    "class 1 events",
    "class 2 events",
    "class 3 events",
    "need time",
    "local control",
    "device trouble",
    "device restart",
];
const IIN2: [&str; 6] = [
    "function not supported",
    "object unknown",
    "parameter error",
    "event buffer overflow",
    "already executing",
    "config corrupt",
];

/// The internal indications set, by name ("none" when none is set).
pub fn iin_text(iin1: u8, iin2: u8) -> String {
    let mut names: Vec<&str> = Vec::new();
    for (k, n) in IIN1.iter().enumerate() {
        if iin1 >> k & 1 != 0 {
            names.push(n);
        }
    }
    for (k, n) in IIN2.iter().enumerate() {
        if iin2 >> k & 1 != 0 {
            names.push(n);
        }
    }
    if names.is_empty() {
        "none".into()
    } else {
        names.join(", ")
    }
}

/// The size of one object of a group and variation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Size {
    /// Bytes for each object.
    Bytes(usize),
    /// Bits for each object, packed.
    Bits(usize),
    /// The variation is the size (strings and octets).
    Variation,
    /// No data (class objects).
    None,
    /// Not known: the rest of the fragment is not read.
    Unknown,
}

/// The size of an object of group `g`, variation `v`.
pub fn size(g: u8, v: u8) -> Size {
    let pick = |t: &[usize]| {
        usize::from(v)
            .checked_sub(1)
            .and_then(|i| t.get(i).copied())
            .map_or(Size::Unknown, Size::Bytes)
    };
    match (g, v) {
        (1 | 10, 1) | (12, 3) | (80, 1) => Size::Bits(1),
        (3, 1) => Size::Bits(2),
        (1 | 3 | 10, 2) => Size::Bytes(1),
        (2 | 4, _) => pick(&[1, 7, 3]),
        (11 | 13, _) => pick(&[1, 7]),
        (12, 1 | 2) => Size::Bytes(11),
        (20, _) => pick(&[5, 3, 5, 3, 4, 2, 4, 2]),
        (21, _) => pick(&[5, 3, 5, 3, 11, 9, 11, 9, 4, 2, 4, 2]),
        (22 | 23, _) => pick(&[5, 3, 5, 3, 11, 9, 11, 9]),
        (30, _) => pick(&[5, 3, 4, 2, 5, 9]),
        (31, _) => pick(&[5, 3, 11, 9, 4, 2, 5, 9]),
        (32 | 33 | 42 | 43, _) => pick(&[5, 3, 11, 9, 5, 9, 11, 15]),
        (34, _) => pick(&[2, 4, 4]),
        (40 | 41, _) => pick(&[5, 3, 5, 9]),
        (50, _) => pick(&[6, 10, 6, 11]),
        (51, _) => pick(&[6, 6]),
        (52, _) => pick(&[2, 2]),
        (60, _) => Size::None,
        (110..=113, _) => Size::Variation,
        _ => Size::Unknown,
    }
}

/// A point value read from a response or a command.
#[derive(Clone, Debug, PartialEq)]
pub struct Point {
    /// The kind of point: BI, DBI, BO, CTR, AI, AO.
    pub kind: &'static str,
    /// The index.
    pub index: u32,
    /// The value as text.
    pub value: String,
}

/// The kind of point of a group, if it carries a value.
fn point_kind(g: u8) -> Option<&'static str> {
    match g {
        1 | 2 => Some("BI"),
        3 | 4 => Some("DBI"),
        10..=13 => Some("BO"),
        20..=23 => Some("CTR"),
        30..=34 => Some("AI"),
        40..=43 => Some("AO"),
        _ => None,
    }
}

fn le(b: &[u8]) -> u64 {
    b.iter().rev().fold(0u64, |a, &x| a << 8 | u64::from(x))
}

fn int_text(b: &[u8], signed: bool) -> String {
    let v = le(b);
    if signed {
        let bits = 8 * b.len() as u32;
        let shift = 64 - bits;
        (((v << shift) as i64) >> shift).to_string()
    } else {
        v.to_string()
    }
}

fn float_text(b: &[u8]) -> String {
    match b.len() {
        4 => format!("{}", f32::from_le_bytes([b[0], b[1], b[2], b[3]])),
        8 => format!(
            "{}",
            f64::from_le_bytes([b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7]])
        ),
        _ => hex(b),
    }
}

fn state(flags: u8) -> &'static str {
    if flags & 0x80 != 0 {
        "ON"
    } else {
        "OFF"
    }
}

fn double_bit(v: u8) -> &'static str {
    match v & 3 {
        0 => "intermediate",
        1 => "off",
        2 => "on",
        _ => "indeterminate",
    }
}

/// The value of one object of `size` bytes of group `g`, variation `v`.
fn value_text(g: u8, v: u8, b: &[u8]) -> Option<String> {
    let flag = b.first().copied().unwrap_or(0);
    let s = match g {
        1 | 2 | 10 | 11 | 13 => state(flag).to_string(),
        3 | 4 => double_bit(flag >> 6).to_string(),
        20..=23 => {
            let flagless = (g == 20 && v >= 5) || (g == 21 && v >= 9);
            let body = if flagless { b } else { b.get(1..)? };
            let width = if v % 2 == 1 { 4 } else { 2 };
            int_text(body.get(..width)?, false)
        }
        30 => match v {
            1 => int_text(b.get(1..5)?, true),
            2 => int_text(b.get(1..3)?, true),
            3 => int_text(b.get(..4)?, true),
            4 => int_text(b.get(..2)?, true),
            5 => float_text(b.get(1..5)?),
            6 => float_text(b.get(1..9)?),
            _ => return None,
        },
        32 | 33 | 42 | 43 => match v {
            1 | 3 => int_text(b.get(1..5)?, true),
            2 | 4 => int_text(b.get(1..3)?, true),
            5 | 7 => float_text(b.get(1..5)?),
            6 | 8 => float_text(b.get(1..9)?),
            _ => return None,
        },
        40 => match v {
            1 => int_text(b.get(1..5)?, true),
            2 => int_text(b.get(1..3)?, true),
            3 => float_text(b.get(1..5)?),
            4 => float_text(b.get(1..9)?),
            _ => return None,
        },
        41 => match v {
            1 => int_text(b.get(..4)?, true),
            2 => int_text(b.get(..2)?, true),
            3 => float_text(b.get(..4)?),
            4 => float_text(b.get(..8)?),
            _ => return None,
        },
        _ => return None,
    };
    Some(s)
}

/// The text of a 48-bit time (milliseconds since 1970-01-01 UTC).
pub fn time_text(b: &[u8]) -> String {
    let ms = le(&b[..6.min(b.len())]);
    match sdrkit_util_geo::time::iso8601(ms as f64 / 1000.0, 3) {
        Ok(t) => t.as_str().to_string(),
        Err(_) => format!("{ms} ms"),
    }
}

/// The application header of a fragment.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AppHeader {
    /// FIR.
    pub fir: bool,
    /// FIN.
    pub fin: bool,
    /// CON: the receiver is to confirm.
    pub con: bool,
    /// UNS: unsolicited.
    pub uns: bool,
    /// The sequence, 0 to 15.
    pub seq: u8,
    /// The function.
    pub function: u8,
    /// IIN1 and IIN2 of a response.
    pub iin: Option<(u8, u8)>,
}

/// A fragment read.
#[derive(Clone, Debug, PartialEq)]
pub struct App {
    /// The header.
    pub header: AppHeader,
    /// The object headers as text ("g60v2 all", "g30v1 0-3").
    pub objects: Vec<String>,
    /// The classes polled (0 is the integrity poll), in order.
    pub polls: Vec<u8>,
    /// The point values.
    pub points: Vec<Point>,
    /// The time of a time object (a time sync when written).
    pub time: Option<String>,
    /// A time delay, ms.
    pub delay_ms: Option<u64>,
    /// Secure Authentication: shown raw.
    pub encrypted: bool,
    /// What could not be read.
    pub note: Option<String>,
}

impl App {
    /// Whether the fragment is a response or an unsolicited response.
    pub fn is_response(&self) -> bool {
        matches!(self.header.function, 129 | 130)
    }
}

/// Whether the objects of function `f` carry data.
fn carries_data(f: u8) -> bool {
    matches!(f, 2..=6 | 11 | 12 | 129 | 130)
}

fn take<'a>(b: &'a [u8], at: &mut usize, n: usize) -> Option<&'a [u8]> {
    let s = b.get(*at..*at + n)?;
    *at += n;
    Some(s)
}

/// Reads a fragment: the header, the object headers, the points and the times.
pub fn read_app(frag: &[u8]) -> Option<App> {
    let c = *frag.first()?;
    let function = *frag.get(1)?;
    let mut at = 2;
    let iin = if matches!(function, 129 | 130) {
        let b = take(frag, &mut at, 2)?;
        Some((b[0], b[1]))
    } else {
        None
    };
    let mut app = App {
        header: AppHeader {
            fir: c & 0x80 != 0,
            fin: c & 0x40 != 0,
            con: c & 0x20 != 0,
            uns: c & 0x10 != 0,
            seq: c & 0x0F,
            function,
            iin,
        },
        objects: Vec::new(),
        polls: Vec::new(),
        points: Vec::new(),
        time: None,
        delay_ms: None,
        encrypted: matches!(function, 32 | 33 | 131),
        note: None,
    };
    if app.encrypted {
        app.objects.push("Secure Authentication, shown raw".into());
        return Some(app);
    }
    let data = carries_data(function);
    while at + 3 <= frag.len() {
        let (g, v, q) = (frag[at], frag[at + 1], frag[at + 2]);
        at += 3;
        let prefix = (q >> 4) & 7;
        let range = q & 0x0F;
        let (count, start, text) = match range {
            0..=2 => {
                let w = [1, 2, 4][usize::from(range)];
                let Some(b) = take(frag, &mut at, 2 * w) else {
                    app.note = Some("the fragment ends in an object header".into());
                    break;
                };
                let (s, e) = (le(&b[..w]), le(&b[w..]));
                (
                    (e + 1).saturating_sub(s) as usize,
                    s as u32,
                    format!("g{g}v{v} {s}-{e}"),
                )
            }
            6 => (0, 0, format!("g{g}v{v} all")),
            7..=9 | 0xB => {
                let w = match range {
                    7 | 0xB => 1,
                    8 => 2,
                    _ => 4,
                };
                let Some(b) = take(frag, &mut at, w) else {
                    app.note = Some("the fragment ends in an object header".into());
                    break;
                };
                let n = le(b) as usize;
                (n, 0, format!("g{g}v{v} count {n}"))
            }
            _ => {
                app.objects.push(format!("g{g}v{v}"));
                app.note = Some(format!(
                    "unknown qualifier {q:02X}; the rest of the fragment is skipped"
                ));
                break;
            }
        };
        app.objects.push(text);
        if g == 120 {
            app.encrypted = true;
            app.note = Some("Secure Authentication object, shown raw".into());
            break;
        }
        if g == 60 {
            app.polls.push(v.saturating_sub(1));
            continue;
        }
        let index_len = match prefix {
            1 => 1,
            2 => 2,
            3 => 4,
            _ => 0,
        };
        let size_len = match prefix {
            4 => 1,
            5 => 2,
            6 => 4,
            _ => 0,
        };
        if !data && size_len == 0 {
            // A request without data: only the index prefixes.
            at += count * index_len;
            continue;
        }
        let sz = size(g, v);
        if let (Size::Bits(bits), 0) = (sz, index_len) {
            let n = (count * bits).div_ceil(8);
            let Some(b) = take(frag, &mut at, n) else {
                app.note = Some("the fragment ends in packed objects".into());
                break;
            };
            if let Some(kind) = point_kind(g) {
                for k in 0..count {
                    let bit = k * bits;
                    let val = (b[bit / 8] >> (bit % 8)) & ((1 << bits) - 1);
                    let value = if bits == 2 {
                        double_bit(val).to_string()
                    } else if val != 0 {
                        "ON".into()
                    } else {
                        "OFF".into()
                    };
                    app.points.push(Point {
                        kind,
                        index: start + k as u32,
                        value,
                    });
                }
            }
            continue;
        }
        let mut stop = false;
        for k in 0..count {
            let index = match take(frag, &mut at, index_len) {
                Some(b) if index_len > 0 => le(b) as u32,
                Some(_) => start + k as u32,
                None => {
                    stop = true;
                    break;
                }
            };
            let n = if size_len > 0 {
                match take(frag, &mut at, size_len) {
                    Some(b) => le(b) as usize,
                    None => {
                        stop = true;
                        break;
                    }
                }
            } else {
                match sz {
                    Size::Bytes(n) => n,
                    Size::Variation => usize::from(v),
                    Size::Bits(bits) => (bits).div_ceil(8),
                    Size::None => 0,
                    Size::Unknown => {
                        app.note = Some(format!(
                            "g{g}v{v} has an unknown size; the rest of the fragment is not read"
                        ));
                        stop = true;
                        break;
                    }
                }
            };
            let Some(b) = take(frag, &mut at, n) else {
                stop = true;
                break;
            };
            match g {
                50 | 51 => app.time = Some(time_text(b)),
                52 => app.delay_ms = Some(le(b)),
                _ => {
                    if let (Some(kind), Some(value)) = (point_kind(g), value_text(g, v, b)) {
                        app.points.push(Point { kind, index, value });
                    }
                }
            }
        }
        if stop {
            if app.note.is_none() {
                app.note = Some("the fragment ends inside an object".into());
            }
            break;
        }
    }
    Some(app)
}

/// The text of the class polls: "integrity poll", "class poll (class 1)", "class 1, 2, 3 poll and integrity poll".
pub fn polls_text(polls: &[u8]) -> Option<String> {
    let classes: Vec<String> = polls
        .iter()
        .filter(|&&c| c > 0)
        .map(|c| c.to_string())
        .collect();
    let integrity = polls.contains(&0);
    match (classes.len(), integrity) {
        (0, false) => None,
        (0, true) => Some("integrity poll".into()),
        (1, false) => Some(format!("class poll (class {})", classes[0])),
        (_, false) => Some(format!("class {} poll", classes.join(", "))),
        (_, true) => Some(format!(
            "class {} poll and integrity poll",
            classes.join(", ")
        )),
    }
}

/// The summary of a fragment: the objects, the polls, the indications, the time and the points.
pub fn summary(app: &App) -> String {
    let mut s = app.objects.join(", ");
    if let Some(p) = polls_text(&app.polls) {
        s.push_str(": ");
        s.push_str(&p);
    }
    let mut add = |part: String| {
        if !s.is_empty() {
            s.push_str("; ");
        }
        s.push_str(&part);
    };
    if let Some((a, b)) = app.header.iin {
        add(format!("IIN {}", iin_text(a, b)));
    }
    if let Some(t) = &app.time {
        if app.header.function == 2 {
            add(format!("time sync {t}"));
        } else {
            add(format!("time {t}"));
        }
    }
    if let Some(d) = app.delay_ms {
        add(format!("time delay {d} ms"));
    }
    if !app.points.is_empty() {
        let list: Vec<String> = app
            .points
            .iter()
            .take(16)
            .map(|p| format!("{} {} = {}", p.kind, p.index, p.value))
            .collect();
        let more = if app.points.len() > 16 {
            format!(" and {} more", app.points.len() - 16)
        } else {
            String::new()
        };
        add(format!("{}{more}", list.join(", ")));
    }
    if let Some(n) = &app.note {
        add(n.clone());
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scada::frame::{dnp3_frame, dnp3_user_data};

    #[test]
    fn the_worked_link_frames() {
        let f = [0x05, 0x64, 0x05, 0xC9, 0x01, 0x00, 0x03, 0x00, 0x75, 0x3E];
        let l = Link::of(&f).unwrap();
        assert!(l.dir && l.prm);
        assert_eq!((l.src, l.dest, l.function), (3, 1, 9));
        assert_eq!(l.function_name(), "request link status");
        let f = [
            0x05, 0x64, 0x14, 0xC4, 0x01, 0x00, 0x03, 0x00, 0x04, 0x09, 0xC0, 0xC1, 0x01, 0x3C,
            0x02, 0x06, 0x3C, 0x03, 0x06, 0x3C, 0x04, 0x06, 0x3C, 0x01, 0x06, 0x7A, 0x6F,
        ];
        let l = Link::of(&f).unwrap();
        assert_eq!(l.function_name(), "unconfirmed user data");
        let (user, bad) = dnp3_user_data(&f);
        assert_eq!(bad, None);
        let t = Transport::of(user[0]);
        assert!(t.fir && t.fin && t.seq == 0);
        let app = read_app(&user[1..]).unwrap();
        assert!(app.header.fir && app.header.fin && app.header.seq == 1);
        assert_eq!(app_function_name(app.header.function), "read");
        assert_eq!(
            summary(&app),
            "g60v2 all, g60v3 all, g60v4 all, g60v1 all: class 1, 2, 3 poll and integrity poll"
        );
    }

    #[test]
    fn responses_with_points_time_and_indications() {
        // Response, IIN device restart and class 1 events; g30v1 0-1; g1v2 count 2 with 1-byte indexes; g20v5 3-3.
        let mut frag = vec![0xC2, 129, 0x82, 0x00];
        frag.extend_from_slice(&[
            30, 1, 0x00, 0, 1, 0x01, 0xE8, 0x03, 0, 0, 0x01, 0xFE, 0xFF, 0xFF, 0xFF,
        ]);
        frag.extend_from_slice(&[1, 2, 0x17, 2, 4, 0x81, 7, 0x01]);
        frag.extend_from_slice(&[20, 5, 0x00, 3, 3, 0x10, 0x27, 0, 0]);
        let app = read_app(&frag).unwrap();
        assert_eq!(app.points.len(), 5, "{:?}", app);
        assert_eq!(app.points[0].value, "1000");
        assert_eq!(app.points[1].value, "-2");
        assert_eq!(
            (app.points[2].index, app.points[2].value.as_str()),
            (4, "ON")
        );
        assert_eq!(
            (app.points[3].index, app.points[3].value.as_str()),
            (7, "OFF")
        );
        assert_eq!(
            (app.points[4].kind, app.points[4].value.as_str()),
            ("CTR", "10000")
        );
        assert_eq!(iin_text(0x82, 0), "class 1 events, device restart");
        // A write of g50v1: a time sync.
        let ms: u64 = 1_794_134_527_250;
        let mut frag = vec![0xC3, 2, 50, 1, 0x07, 1];
        frag.extend_from_slice(&ms.to_le_bytes()[..6]);
        let app = read_app(&frag).unwrap();
        assert_eq!(
            summary(&app),
            "g50v1 count 1; time sync 2026-11-08T10:42:07.250Z"
        );
        // Packed binary inputs and double-bit inputs.
        let frag = [
            0xC0,
            129,
            0,
            0,
            1,
            1,
            0x00,
            0,
            9,
            0x05,
            0x01,
            3,
            1,
            0x00,
            0,
            3,
            0b1110_0100,
        ];
        let app = read_app(&frag).unwrap();
        assert_eq!(app.points.len(), 14);
        assert_eq!(app.points[0].value, "ON");
        assert_eq!(app.points[8].value, "ON");
        assert_eq!(app.points[9].value, "OFF");
        assert_eq!(app.points[10].value, "intermediate");
        assert_eq!(app.points[13].value, "indeterminate");
    }

    #[test]
    fn segments_join_and_lost_first_segments_are_noted() {
        let mut s = Segments::default();
        assert_eq!(s.push(1, 3, Transport::of(0x40), &[1, 2]), Ok(None));
        assert_eq!(
            s.push(1, 3, Transport::of(0x81), &[3]),
            Ok(Some(vec![1, 2, 3]))
        );
        assert_eq!(
            s.push(1, 3, Transport::of(0x85), &[3]),
            Err("segment 5 came without the first segment; fragment dropped".into())
        );
        // The class 43 sizes are those of group 42.
        assert_eq!(size(43, 1), Size::Bytes(5));
        assert_eq!(size(43, 8), Size::Bytes(15));
        let f = dnp3_frame(0x44, 3, 1, &[0xC0, 0xC0, 0x81, 0, 0, 120, 1, 0x5B, 1, 0]);
        let (user, _) = dnp3_user_data(&f);
        let app = read_app(&user[1..]).unwrap();
        assert!(app.encrypted);
    }
}
