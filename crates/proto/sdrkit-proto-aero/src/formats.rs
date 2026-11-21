//! Plain-text ACARS formats: position, OOOI, ETA, fault, route and weather messages that airlines write as text, read by label.
//!
//! Each label has a list of layouts tried in order; the first that matches every field wins. A text that does not match gives no
//! reading, never half of one. The layouts are airline formats, not public standards. Rows `1J`, `2J`, `2A`, `4J`, and the texts of
//! `2P` and `H1` that no other layout reads, use the ARINC 702 form (`POS`, `FPN`, `INI` ... with `/`-separated fields).
//!
//! A downlink is read with its prefix (message number, sequence character and flight, 10 characters) in front of the text, as it
//! was sent; when that gives nothing, the text alone is read.
//!
//! ```
//! use sdrkit_proto_aero::formats::{read, Category};
//! let r = read("44", "00ETA03,N38241W081357,330,KBNA,KBWI,1107,0123,0208,008.1").unwrap();
//! assert_eq!((r.kind, r.category), ("ETA report", Category::Eta));
//! assert_eq!(r.origin.as_deref(), Some("KBNA"));
//! assert!(r.line().starts_with("ETA report: 38.402, -81.595"));
//! ```

use crate::textpos;
use sdrkit_util_geo::Position;

/// What a reading is about.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Category {
    /// Anything else.
    Other,
    /// A position report.
    Position,
    /// Out, off, on, in times and other flight events.
    Oooi,
    /// An arrival estimate.
    Eta,
    /// A fault or warning.
    Fault,
    /// A flight plan or route.
    Route,
    /// A load sheet.
    Loadsheet,
    /// Weather.
    Weather,
    /// Free text.
    Text,
}

impl Category {
    /// The name in lower case.
    pub fn name(self) -> &'static str {
        match self {
            Category::Other => "other",
            Category::Position => "position",
            Category::Oooi => "OOOI",
            Category::Eta => "ETA",
            Category::Fault => "fault",
            Category::Route => "route",
            Category::Loadsheet => "loadsheet",
            Category::Weather => "weather",
            Category::Text => "text",
        }
    }
}

/// A plain-text format read from a message.
#[derive(Clone, Debug, PartialEq)]
pub struct FormatReading {
    /// What the message is ("Position report", "OOOI: wheels off" ...).
    pub kind: &'static str,
    /// Its category.
    pub category: Category,
    /// The other fields, name and value, in order.
    pub fields: Vec<(&'static str, String)>,
    /// The position.
    pub position: Option<Position>,
    /// Feet, from -2000 to 70000 (exclusive).
    pub alt_ft: Option<f64>,
    /// Degrees, 0 to 360.
    pub heading_deg: Option<f64>,
    /// Airport of departure (3 or 4 letters or digits).
    pub origin: Option<String>,
    /// Airport of destination.
    pub destination: Option<String>,
    /// Flight.
    pub flight: Option<String>,
}

impl FormatReading {
    /// The one-line form: "Kind:", then the position with 3 decimals, the airports, the flight, the altitude, and each field as
    /// "Name value" ("Text" and "Route" without their name), separated by ", ".
    pub fn line(&self) -> String {
        let mut parts: Vec<String> = Vec::new();
        if let Some(p) = self.position {
            parts.push(format!("{:.3}, {:.3}", p.lat_deg, p.lon_deg));
        }
        if let Some(o) = &self.origin {
            parts.push(format!("From {o}"));
        }
        if let Some(d) = &self.destination {
            parts.push(format!("To {d}"));
        }
        if let Some(f) = &self.flight {
            parts.push(format!("Flight {f}"));
        }
        if let Some(a) = self.alt_ft {
            parts.push(format!("Altitude {a:.0} ft"));
        }
        if let Some(h) = self.heading_deg {
            parts.push(format!("Heading {h:.0}"));
        }
        for (n, v) in &self.fields {
            parts.push(match *n {
                "Text" | "Route" => v.clone(),
                _ => format!("{n} {v}"),
            });
        }
        format!("{}: {}", self.kind, parts.join(", "))
    }
}

/// The labels that have plain-text layouts.
pub const LABELS: [&str; 37] = [
    "10", "11", "12", "13", "14", "15", "16", "17", "18", "1J", "1L", "1M", "20", "21", "22", "24", "2A", "2J", "2P", "30",
    "44", "4A", "4J", "4N", "4T", "58", "5Z", "80", "83", "8E", "B6", ":;", "H1", "H2", "HX", "SQ", "QQ",
];

/// Reads the plain-text format of a message text with its label. `None` when no layout of the label matches.
pub fn read(label: &str, text: &str) -> Option<FormatReading> {
    match label {
        "10" => r1(text).or_else(|| r2(text)).or_else(|| r3(text)),
        "11" => r4(text),
        "12" => r5(text).or_else(|| r6(text)),
        "13" | "14" | "17" | "18" => r7(label, text),
        "15" => r8(text).or_else(|| r9(text)).or_else(|| r7(label, text)),
        "16" => r10(text)
            .or_else(|| r11(text))
            .or_else(|| r12(text))
            .or_else(|| r13(text))
            .or_else(|| r14(text))
            .or_else(|| r7(label, text)),
        "1L" => r15(text).or_else(|| r16(text)).or_else(|| r17(text)).or_else(|| r18(text)),
        "1M" => r19(text),
        "20" => r20(text),
        "21" => r21(text),
        "22" => r22(text).or_else(|| r23(text)).or_else(|| r24(text)).or_else(|| r25(text)),
        "24" => r26(text),
        "2P" => r27(text).or_else(|| r28(text)).or_else(|| r29(text)).or_else(|| arinc702(text)),
        "30" => r31(text),
        "44" => r32(text)
            .or_else(|| r33(text))
            .or_else(|| r34(text, "IN0", "OOOI: gate in", "In"))
            .or_else(|| r34(text, "ON0", "OOOI: wheels on", "On"))
            .or_else(|| r36(text))
            .or_else(|| r37(text)),
        "4A" => r38(text)
            .or_else(|| r39(text))
            .or_else(|| r40(text))
            .or_else(|| r41(text))
            .or_else(|| r42(text))
            .or_else(|| r43(text)),
        "4N" => r44(text).or_else(|| r45(text)),
        "4T" => r46(text).or_else(|| r47(text)),
        "58" => r48(text),
        "5Z" => r49(text, "B3", "Takeoff data request")
            .or_else(|| r49(text, "C3", "Gate request"))
            .or_else(|| r51(text)),
        "80" => r80(text),
        "83" => r56(text).or_else(|| r57(text)).or_else(|| r58(text)),
        "8E" => r59(text),
        "B6" => r60(text),
        ":;" => r61(text),
        "H1" => r62(text)
            .or_else(|| r63(text))
            .or_else(|| r64(text))
            .or_else(|| r65(text))
            .or_else(|| r66(text))
            .or_else(|| r67(text))
            .or_else(|| arinc702(text))
            .or_else(|| r69(text)),
        "H2" => r70(text),
        "HX" => r71(text).or_else(|| r72(text)),
        "SQ" => r73(text),
        "QQ" => r74(text),
        "1J" | "2J" | "2A" | "4J" => arinc702(text),
        _ => None,
    }
}

/// Reads a downlink with its prefix (message number, sequence character, flight) first, then the text alone.
pub fn read_message(label: &str, prefix: &str, text: &str) -> Option<FormatReading> {
    if !prefix.is_empty() {
        let full = format!("{prefix}{text}");
        if let Some(r) = read(label, &full) {
            return Some(r);
        }
    }
    read(label, text)
}

// Text helpers. Every index goes through `get`, so no text makes them fail.

fn sl(s: &str, a: usize, b: usize) -> Option<&str> {
    s.get(a..b)
}

fn from(s: &str, a: usize) -> Option<&str> {
    s.get(a..)
}

fn trim(s: &str) -> &str {
    s.trim_matches([' ', '\r', '\n'])
}

fn fields(s: &str, sep: char) -> Vec<&str> {
    s.split(sep).collect()
}

fn words(s: &str) -> Vec<&str> {
    s.split_whitespace().collect()
}

fn all_digits(s: &str) -> bool {
    !s.is_empty() && s.bytes().all(|c| c.is_ascii_digit())
}

fn num(s: &str) -> Option<f64> {
    let t: String = s.chars().filter(|c| *c != ' ').collect();
    let t = t.strip_prefix('+').unwrap_or(&t);
    t.parse::<f64>().ok().filter(|v| v.is_finite())
}

/// The number at the start of `s` (after spaces, with an optional sign).
fn lead_num(s: &str) -> Option<f64> {
    let t = s.trim_start();
    let end = t
        .char_indices()
        .find(|&(i, c)| !(c.is_ascii_digit() || c == '.' || (i == 0 && (c == '-' || c == '+'))))
        .map_or(t.len(), |(i, _)| i);
    num(&t[..end])
}

fn sign(c: Option<char>) -> Option<f64> {
    match c? {
        'N' | 'E' => Some(1.0),
        'S' | 'W' => Some(-1.0),
        _ => None,
    }
}

/// A hemisphere letter then a number, divided by `div`.
fn hemi_num(s: &str, div: f64) -> Option<f64> {
    let t = trim(s);
    Some(sign(t.chars().next())? * num(from(t, 1)?)? / div)
}

/// Form D: a hemisphere letter then a decimal number, or a signed decimal number; spaces removed.
fn coord_d(s: &str) -> Option<f64> {
    let t: String = s.chars().filter(|c| *c != ' ').collect();
    match sign(t.chars().next()) {
        Some(sg) => Some(sg * num(from(&t, 1)?)?),
        None => num(&t),
    }
}

/// Forms T and M: hemisphere, 5 digits, hemisphere (at character 6, or 7 after one space), 6 digits (a space in them reads as 0).
fn coord_tm(s: &str, thousandths: bool) -> Option<(f64, f64)> {
    let b = s.as_bytes();
    let mut buf = [0u8; 13];
    buf[..6].copy_from_slice(b.get(..6)?);
    let mut i = 6;
    if b.get(6) == Some(&b' ') && matches!(b.get(7), Some(b'E' | b'W')) {
        i = 7;
    }
    buf[6] = *b.get(i)?;
    for (k, c) in b.get(i + 1..i + 7)?.iter().enumerate() {
        buf[7 + k] = if *c == b' ' { b'0' } else { *c };
    }
    let (lat, lon, _) = textpos::packed(&buf, thousandths)?;
    Some((lat, lon))
}

/// Hemisphere, degrees of `w` digits, minutes, seconds.
fn dms(s: &str, w: usize) -> Option<f64> {
    let sg = sign(s.chars().next())?;
    let d = sl(s, 1, 1 + w).filter(|x| all_digits(x))?.parse::<f64>().ok()?;
    let m = sl(s, 1 + w, 3 + w).filter(|x| all_digits(x))?.parse::<f64>().ok()?;
    let sec = sl(s, 3 + w, 5 + w).filter(|x| all_digits(x))?.parse::<f64>().ok()?;
    Some(sg * (d + m / 60.0 + sec / 3600.0))
}

/// Hemisphere, degrees of `w` digits, minutes with decimals.
fn dmm(s: &str, w: usize) -> Option<f64> {
    let sg = sign(s.chars().next())?;
    let d = sl(s, 1, 1 + w).filter(|x| all_digits(x))?.parse::<f64>().ok()?;
    let m = num(from(s, 1 + w)?)?;
    Some(sg * (d + m / 60.0))
}

fn time_text(s: &str) -> String {
    let t = trim(s);
    if all_digits(t) && t.len() == 4 {
        format!("{}:{}", &t[..2], &t[2..])
    } else if all_digits(t) && t.len() == 6 {
        format!("{}:{}:{}", &t[..2], &t[2..4], &t[4..])
    } else {
        t.to_owned()
    }
}

fn airport(s: &str) -> Option<String> {
    let t = trim(s);
    (matches!(t.len(), 3 | 4) && t.bytes().all(|c| c.is_ascii_uppercase() || c.is_ascii_digit()))
        .then(|| t.to_owned())
}

/// A reading being built.
struct B(FormatReading);

impl B {
    fn new(kind: &'static str, category: Category) -> B {
        B(FormatReading {
            kind,
            category,
            fields: Vec::new(),
            position: None,
            alt_ft: None,
            heading_deg: None,
            origin: None,
            destination: None,
            flight: None,
        })
    }

    /// Sets the position; `None` (the format fails) when it is out of range, not finite or exactly 0, 0.
    fn pos(&mut self, lat: f64, lon: f64) -> Option<()> {
        let ok = lat.is_finite()
            && lon.is_finite()
            && lat.abs() <= 90.0
            && lon.abs() <= 180.0
            && !(lat == 0.0 && lon == 0.0);
        if !ok {
            return None;
        }
        self.0.position = Some(Position::new(lat, lon));
        Some(())
    }

    fn pos_pair(&mut self, p: (f64, f64)) -> Option<()> {
        self.pos(p.0, p.1)
    }

    fn alt(&mut self, ft: Option<f64>) {
        if let Some(a) = ft.filter(|a| *a > -2000.0 && *a < 70000.0) {
            self.0.alt_ft = Some(a);
        }
    }

    fn alt100(&mut self, s: &str) {
        self.alt(num(s).map(|v| v * 100.0));
    }

    fn heading(&mut self, s: &str) {
        if let Some(h) = num(s).filter(|h| (0.0..=360.0).contains(h)) {
            self.0.heading_deg = Some(h);
        }
    }

    fn origin(&mut self, s: &str) {
        if let Some(a) = airport(s) {
            self.0.origin = Some(a);
        }
    }

    fn dest(&mut self, s: &str) {
        if let Some(a) = airport(s) {
            self.0.destination = Some(a);
        }
    }

    fn flight(&mut self, s: &str) {
        let t = trim(s);
        if !t.is_empty() {
            self.0.flight = Some(t.to_owned());
        }
    }

    fn f(&mut self, name: &'static str, v: &str) {
        let t = trim(v);
        if !t.is_empty() {
            self.0.fields.push((name, t.to_owned()));
        }
    }

    fn time(&mut self, name: &'static str, v: &str) {
        let t = time_text(v);
        if !t.is_empty() {
            self.0.fields.push((name, t));
        }
    }

    fn done(self) -> Option<FormatReading> {
        Some(self.0)
    }
}

const POS: (&str, Category) = ("Position report", Category::Position);

fn pos_b() -> B {
    B::new(POS.0, POS.1)
}

// Label 10.

fn r1(t: &str) -> Option<FormatReading> {
    if !t.starts_with("POS") {
        return None;
    }
    let f = fields(t, ',');
    if f.len() != 12 {
        return None;
    }
    let mut b = pos_b();
    b.pos(hemi_num(f[1], 100.0)?, hemi_num(f[2], 100.0)?)?;
    b.alt(num(f[7]));
    b.done()
}

fn r2(t: &str) -> Option<FormatReading> {
    if !t.starts_with('/') {
        return None;
    }
    let f = fields(t, '/');
    if f.len() < 17 {
        return None;
    }
    let mut b = pos_b();
    b.pos(coord_d(f[1])?, coord_d(f[2])?)?;
    b.heading(f[5]);
    b.alt100(f[6]);
    b.dest(f[7]);
    b.time("ETA", f[8]);
    let route = format!(
        "{} {} > {} {} > {}",
        trim(f[11]),
        time_text(f[13]),
        trim(f[12]),
        time_text(f[15]),
        trim(f[14])
    );
    b.f("Route", &route);
    b.origin(f[16]);
    b.done()
}

fn r3(t: &str) -> Option<FormatReading> {
    if !t.starts_with("LRW01,") {
        return None;
    }
    let f = fields(t, ',');
    if f.len() < 8 || trim(f[7]).is_empty() {
        return None;
    }
    let mut b = B::new("Landing runway request", Category::Eta);
    b.origin(f[6]);
    b.dest(f[7]);
    b.done()
}

// Label 11.

fn r4(t: &str) -> Option<FormatReading> {
    if !t.starts_with("LRW01,") || sl(t, 10, 27) != Some(" LANDING RUNWAYS,") {
        return None;
    }
    let mut b = B::new("Landing runways", Category::Eta);
    b.f("Airport", sl(t, 6, 10)?);
    b.f("Text", from(t, 27)?);
    b.done()
}

// Labels 12 and 16: decimal positions.

/// The altitude field of rows 5 and 12: feet, or "ground" for `GRD` and `***`.
fn alt_or_ground(b: &mut B, s: &str) {
    let a = trim(s);
    if a == "GRD" || a == "***" {
        b.f("Altitude", "ground");
    } else {
        b.alt(num(a));
    }
}

fn r5(t: &str) -> Option<FormatReading> {
    if !(t.starts_with("N ") || t.starts_with("S ")) {
        return None;
    }
    let f = fields(t, ',');
    if f.len() < 5 {
        return None;
    }
    let p = textpos::read("12", t)?;
    let mut b = pos_b();
    b.pos(p.position.lat_deg, p.position.lon_deg)?;
    alt_or_ground(&mut b, f[2]);
    b.done()
}

fn r6(t: &str) -> Option<FormatReading> {
    let rest = t.strip_prefix("POS")?;
    let f = fields(rest, ',');
    if f.len() != 12 || f[0].len() < 16 {
        return None;
    }
    let mut b = pos_b();
    b.pos(dms(sl(f[0], 0, 8)?, 3)?, dms(from(f[0], 8)?, 3)?)?;
    b.time("Time", f[2]);
    b.alt(num(f[3]).map(|v| v * 10.0));
    b.f("Fuel", &format!("{} t", trim(from(f[7], 3).unwrap_or(""))));
    b.time("ETA", from(f[8], 3).unwrap_or(""));
    b.origin(f[9]);
    b.dest(f[10]);
    b.done()
}

// Labels 13 to 18: flight events.

fn r7(label: &str, t: &str) -> Option<FormatReading> {
    if !t.starts_with('/') {
        return None;
    }
    let mut lines = t.split('\n').map(|l| l.trim_end_matches('\r'));
    let first = lines.next()?;
    let f = fields(first, '/');
    let want = if label == "18" { 7 } else { 4 };
    if f.len() != want || !sl(f[1], 0, 2).is_some_and(all_digits) {
        return None;
    }
    let w = words(f[2]);
    if w.len() < 5 {
        return None;
    }
    let (kind, name) = match label {
        "13" => ("OOOI: gate out", "Out"),
        "14" => ("OOOI: wheels off", "Off"),
        "15" => ("OOOI: wheels on", "On"),
        "16" => ("OOOI: gate in", "In"),
        _ => ("Flight status", "Time"),
    };
    let mut b = B::new(kind, Category::Oooi);
    b.origin(w[1]);
    b.dest(w[2]);
    b.f("Day", w[3]);
    b.time(name, w[4]);
    for l in lines {
        if let Some(loc) = l.strip_prefix("/LOC") {
            let c = fields(from(loc, 1).unwrap_or(""), ',');
            if c.len() == 2 {
                let (a, o) = (trim(c[0]), trim(c[1]));
                let p = if a.len() >= 7 && o.len() >= 8 && sign(a.chars().next()).is_some() {
                    dms(a, 2).zip(dms(o, 3))
                } else {
                    num(a).zip(num(o))
                };
                if let Some((la, lo)) = p {
                    b.pos(la, lo)?;
                }
            }
        }
    }
    b.done()
}

// Label 15.

/// The body of `(2 ... (Z` texts: without the first 2 and the last 2 characters.
fn paren_body(t: &str) -> Option<&str> {
    t.strip_prefix("(2")?.get(..t.len().checked_sub(4)?)
}

fn r8(t: &str) -> Option<FormatReading> {
    if !t.ends_with("(Z") {
        return None;
    }
    let b0 = paren_body(t)?;
    if b0.len() < 19 {
        return None;
    }
    let mut b = pos_b();
    b.pos_pair(coord_tm(sl(b0, 0, 13)?, false)?)?;
    if b0.len() == 25 {
        b.alt100(sl(b0, 19, 22)?);
        b.f("Temperature", &format!("{} C", from(b0, 22)?.replace(' ', "0")));
    } else if sl(b0, 13, 16) == Some("OFF") {
        b.0.kind = "OOOI: wheels off";
        b.time("Off", sl(b0, 22, 26)?);
    }
    b.done()
}

fn r9(t: &str) -> Option<FormatReading> {
    if !t.starts_with("FST01") {
        return None;
    }
    let h = *words(t).first()?;
    if h.len() < 28 {
        return None;
    }
    let c = from(h, 13)?;
    let lat = sign(c.chars().next())? * sl(c, 1, 7).filter(|x| all_digits(x))?.parse::<f64>().ok()? / 10000.0;
    let lon = sign(from(c, 7)?.chars().next())? * sl(c, 8, 15).filter(|x| all_digits(x))?.parse::<f64>().ok()? / 10000.0;
    let mut b = B::new("Flight status", Category::Position);
    b.origin(sl(h, 5, 9)?);
    b.dest(sl(h, 9, 13)?);
    b.pos(lat, lon)?;
    b.alt(lead_num(from(c, 15)?).map(|v| v * 100.0));
    b.done()
}

// Label 16.

fn r10(t: &str) -> Option<FormatReading> {
    if sl(t, 6, 18) != Some("/AUTPOS/LLD ") {
        return None;
    }
    let lines: Vec<&str> = t.split('\n').map(|l| l.trim_end_matches('\r')).collect();
    let l0 = trim(lines[0]);
    if lines.len() < 5 || l0.len() < 34 {
        return None;
    }
    let mut b = pos_b();
    b.pos(dms(sl(l0, 18, 25)?, 2)?, dms(sl(l0, 26, 34)?, 3)?)?;
    b.flight(sl(l0, 2, 6)?);
    for l in &lines {
        if let Some(i) = l.find("/ALT ") {
            b.alt(sl(l, i + 5, i + 11).and_then(lead_num));
        }
        if let Some(i) = l.find("/FOB ") {
            if let Some(v) = sl(l, i + 5, i + 11).and_then(lead_num) {
                b.f("Fuel", &format!("{v} kg"));
            }
        }
    }
    b.done()
}

fn r11(t: &str) -> Option<FormatReading> {
    if t.len() < 29 {
        return None;
    }
    let b0 = paren_body(t)?;
    if b0.len() < 25 {
        return None;
    }
    let mut b = B::new("Position and route", Category::Position);
    b.pos_pair(coord_tm(sl(b0, 4, 17)?, false)?)?;
    let ap = (b0.len() >= 28)
        .then(|| airport(sl(b0, 17, 21)?).zip(airport(sl(b0, 21, 25)?)))
        .flatten();
    match ap {
        Some((o, d)) => {
            b.origin(&o);
            b.dest(&d);
        }
        None => {
            let r = format!(
                "{} > {}",
                trim(sl(b0, 18, 23).unwrap_or("")),
                trim(sl(b0, 23, 28).unwrap_or(""))
            );
            b.f("Route", &r);
        }
    }
    b.done()
}

fn r12(t: &str) -> Option<FormatReading> {
    if !(t.starts_with("N ") || t.starts_with("S ")) {
        return None;
    }
    if t.contains('/') && !t.contains(',') {
        if fields(t, '/').len() != 2 {
            return None;
        }
        let p = textpos::read("16", t)?;
        let mut b = pos_b();
        b.pos(p.position.lat_deg, p.position.lon_deg)?;
        return b.done();
    }
    r5(t)
}

fn r13(t: &str) -> Option<FormatReading> {
    if !t.starts_with("POSA1") {
        return None;
    }
    let f = fields(t, ',');
    if f.len() != 11 {
        return None;
    }
    let mut b = pos_b();
    b.pos_pair(coord_tm(from(f[0], 5)?, true)?)?;
    b.alt100(f[3]);
    let r = format!(
        "{} {} > {} {}",
        trim(f[1]),
        time_text(f[2]),
        trim(f[4]),
        time_text(f[5])
    );
    b.f("Route", &r);
    b.done()
}

fn r14(t: &str) -> Option<FormatReading> {
    let f = fields(t, ',');
    if f.len() != 5 || !(trim(f[0]).len() == 6 && all_digits(trim(f[0]))) {
        return None;
    }
    let mut b = pos_b();
    b.time("Time", f[0]);
    if !trim(f[1]).is_empty() {
        b.alt(num(f[1]));
    }
    b.time("ETA", f[2]);
    let (p, fl) = f[4].split_once('/').unwrap_or((f[4], ""));
    let w = words(p);
    match w.len() {
        4 => b.pos(sign(w[0].chars().next())? * num(w[1])?, sign(w[2].chars().next())? * num(w[3])?)?,
        2 => b.pos(hemi_num(w[0], 1000.0)?, hemi_num(w[1], 1000.0)?)?,
        _ => return None,
    }
    b.flight(fl);
    b.done()
}

// Label 1L.

fn r15(t: &str) -> Option<FormatReading> {
    let f = fields(t.strip_prefix("000000070")?, ',');
    if f.len() != 7 {
        return None;
    }
    let mut b = pos_b();
    b.pos(coord_d(f[4])?, coord_d(f[5])?)?;
    b.origin(f[0]);
    b.dest(f[1]);
    b.time("Time", f[2]);
    b.time("ETA", f[3]);
    b.done()
}

fn r16(t: &str) -> Option<FormatReading> {
    let f = fields(t.strip_prefix("000000660")?, ',');
    if f.len() != 5 {
        return None;
    }
    let mut b = pos_b();
    b.pos_pair(coord_tm(f[0], false)?)?;
    b.time("Time", sl(f[1], 0, 6)?);
    b.alt100(sl(f[1], 6, 9)?);
    b.f("Route", from(f[1], 9)?);
    b.done()
}

/// "KEY value" items: the key up to the first space, the value after it.
fn key_value(s: &str) -> (&str, &str) {
    let t = trim(s);
    match t.split_once(' ') {
        Some((k, v)) => (k, trim(v)),
        None => (t, ""),
    }
}

fn r17(t: &str) -> Option<FormatReading> {
    if !(t.starts_with('+') || t.starts_with('-')) {
        return None;
    }
    let f = fields(t, '/');
    if f.len() != 7 {
        return None;
    }
    let mut b = pos_b();
    b.pos(coord_d(f[0])?, coord_d(f[1])?)?;
    for x in &f[2..7] {
        match key_value(x) {
            ("UTC", v) => b.time("Time", v),
            ("ALT", v) => b.alt(num(v)),
            ("FOB", v) => b.f("Fuel", &format!("{v} kg")),
            ("ETA", v) => b.time("ETA", v),
            _ => {}
        }
    }
    b.done()
}

fn r18(t: &str) -> Option<FormatReading> {
    let t = t.replace('\r', "");
    if t.split('\n').count() != 3 {
        return None;
    }
    let t = t.replace('\n', "/");
    let mut b = pos_b();
    let (mut lat, mut lon, mut airport_seen) = (None, None, false);
    for x in t.split('/') {
        match key_value(x) {
            ("DEP", v) => {
                b.origin(v);
                airport_seen = true;
            }
            ("DES", v) => {
                b.dest(v);
                airport_seen = true;
            }
            ("ETA", v) => b.time("ETA", v),
            ("ALT", v) => b.alt(num(v)),
            ("FN", v) => b.flight(v),
            ("LAT", _) => lat = from(trim(x), 4).and_then(coord_d),
            ("LON", _) => lon = from(trim(x), 4).and_then(coord_d),
            ("UTC", v) => b.time("Time", v),
            _ => {}
        }
    }
    if !airport_seen {
        return None;
    }
    if let (Some(a), Some(o)) = (lat, lon) {
        b.pos(a, o)?;
    }
    b.done()
}

// Label 1M.

fn r19(t: &str) -> Option<FormatReading> {
    if !t.starts_with('/') {
        return None;
    }
    let t = t.replace(['\r'], "").replace('\n', "/");
    let f = fields(&t, '/');
    if f.len() <= 8 {
        return None;
    }
    let mut b = B::new("ETA report", Category::Eta);
    b.flight(f[1]);
    b.origin(f[4]);
    b.dest(f[5]);
    b.f("Alternate", f[6]);
    b.time("ETA", &format!("{}00", trim(f[8])));
    b.done()
}

// Labels 20 and 21.

fn r20(t: &str) -> Option<FormatReading> {
    let rest = t.strip_prefix("POS")?;
    if !matches!(fields(rest, ',').len(), 11 | 5) {
        return None;
    }
    let p = textpos::read("20", t)?;
    let mut b = pos_b();
    b.pos(p.position.lat_deg, p.position.lon_deg)?;
    b.alt(p.alt_ft);
    b.done()
}

fn r21(t: &str) -> Option<FormatReading> {
    let f = fields(t.strip_prefix("POS")?, ',');
    let v = trim(f[0]);
    if f.len() != 9 || v.len() != 16 {
        return None;
    }
    let lat = sign(v.chars().next())? * num(sl(v, 1, 7)?)? / 1000.0;
    let lon = sign(from(v, 8)?.chars().next())? * num(sl(v, 9, 15)?)? / 1000.0;
    let mut b = pos_b();
    b.pos(lat, lon)?;
    b.time("Time", f[2]);
    b.alt(num(f[3]));
    b.time("ETA", f[7]);
    b.dest(f[8]);
    b.done()
}

// Label 22.

fn r22(t: &str) -> Option<FormatReading> {
    let f = fields(t.strip_prefix("OFF01")?, '/');
    if f.len() != 2 || f[1].len() < 22 {
        return None;
    }
    let mut b = B::new("OOOI: wheels off", Category::Oooi);
    b.flight(f[0]);
    b.time("Time", sl(f[1], 4, 8)?);
    b.origin(sl(f[1], 8, 12)?);
    b.dest(sl(f[1], 12, 16)?);
    b.time("Off", sl(f[1], 16, 22)?);
    b.done()
}

fn r23(t: &str) -> Option<FormatReading> {
    let rest = t.strip_prefix("OFF02")?;
    let rest = rest.strip_prefix("\r\n").or_else(|| rest.strip_prefix('\n'))?;
    let line = rest.split('\n').next()?.trim_end_matches('\r');
    let f = fields(line, ',');
    if f.len() != 4 {
        return None;
    }
    let mut b = B::new("OOOI: wheels off", Category::Oooi);
    b.origin(f[0]);
    b.dest(f[1]);
    b.time("Off", f[2]);
    b.done()
}

fn r24(t: &str) -> Option<FormatReading> {
    let f = fields(t.strip_prefix("OFF02")?, '/');
    if f.len() != 2 || f[1].len() < 40 {
        return None;
    }
    let mut b = B::new("OOOI: wheels off", Category::Oooi);
    b.pos_pair(coord_tm(sl(f[1], 0, 14)?, false)?)?;
    b.flight(f[0]);
    b.time("Time", sl(f[1], 16, 20)?);
    b.origin(sl(f[1], 20, 24)?);
    b.dest(sl(f[1], 24, 28)?);
    b.time("Off", sl(f[1], 28, 32)?);
    b.time("ETA", sl(f[1], 36, 40)?);
    b.done()
}

fn r25(t: &str) -> Option<FormatReading> {
    if !(t.starts_with('N') || t.starts_with('S')) {
        return None;
    }
    let f = fields(t, ',');
    if f.len() != 11 || f[0].len() < 15 {
        return None;
    }
    let squeeze = |s: &str| s.replace(' ', "");
    let lat = sign(f[0].chars().next())? * num(&squeeze(sl(f[0], 1, 8)?))? / 10000.0;
    let lon = sign(from(f[0], 8)?.chars().next())? * num(&squeeze(from(f[0], 9)?))? / 10000.0;
    let mut b = pos_b();
    b.pos(lat, lon)?;
    b.time("Time", f[2]);
    b.alt(num(f[3]));
    b.done()
}

// Label 24.

fn r26(t: &str) -> Option<FormatReading> {
    let f = fields(t, '/');
    if f.len() != 10 || !f[0].is_empty() || !f[9].is_empty() {
        return None;
    }
    let mut b = pos_b();
    b.pos(coord_d(f[5])?, coord_d(f[6])?)?;
    b.flight(f[3]);
    b.alt(num(f[4]));
    b.time("ETA", f[8]);
    b.done()
}

// Label 2P.

fn r27(t: &str) -> Option<FormatReading> {
    let f = fields(t, ',');
    let at = t.find("FM3 ")?;
    if f.len() != 7 || at >= f[0].len() {
        return None;
    }
    let mut b = pos_b();
    b.pos(coord_d(f[2])?, coord_d(f[3])?)?;
    b.time("ETA", f[1]);
    b.alt(num(f[4]));
    b.flight(sl(f[0], 4, at)?);
    let d: String = from(f[0], at + 4)?.chars().take_while(char::is_ascii_digit).collect();
    b.time("Time", &d);
    b.done()
}

fn r28(t: &str) -> Option<FormatReading> {
    let f = fields(t, ',');
    let at = f[0].find("FM4")?;
    if f.len() != 10 {
        return None;
    }
    let mut b = pos_b();
    b.pos(coord_d(f[4])?, coord_d(f[5])?)?;
    b.flight(sl(f[0], 4, at)?);
    b.origin(from(f[0], at + 3)?);
    b.dest(f[1]);
    b.time("Time", from(f[2], 2).unwrap_or(""));
    b.time("ETA", f[3]);
    b.alt(num(f[6]));
    b.heading(f[7]);
    b.done()
}

fn r29(t: &str) -> Option<FormatReading> {
    let f = fields(t, ',');
    let at = f[0].find("FM5 ")?;
    if f.len() != 12 {
        return None;
    }
    let mut b = pos_b();
    b.pos(coord_d(f[4])?, coord_d(f[5])?)?;
    b.origin(from(f[0], at + 4)?);
    b.dest(f[1]);
    b.time("Time", f[2]);
    b.time("ETA", f[3]);
    b.alt(num(f[6]));
    b.flight(f[10]);
    b.done()
}

// Label 30.

fn r31(t: &str) -> Option<FormatReading> {
    if !t.starts_with("/EA") {
        return None;
    }
    let t = t.replace('\r', "").replace('\n', "/");
    let f = fields(&t, '/');
    let mut b = B::new("ETA report", Category::Eta);
    b.time("ETA", sl(f.get(1)?, 2, 6)?);
    if let Some(d) = f.get(2).filter(|d| d.starts_with("DS")) {
        b.dest(sl(d, 2, 6).unwrap_or(""));
    }
    b.done()
}

// Label 44.

fn prefix44(t: &str) -> &str {
    let p = t.split(',').next().unwrap_or("");
    p.strip_prefix("00").unwrap_or(p)
}

fn r32(t: &str) -> Option<FormatReading> {
    let f = fields(t, ',');
    if !prefix44(t).starts_with("ETA") || f.len() < 9 {
        return None;
    }
    let mut b = B::new("ETA report", Category::Eta);
    b.pos_pair(coord_tm(f[1], false)?)?;
    b.alt100(f[2]);
    b.origin(f[3]);
    b.dest(f[4]);
    b.f("Date", &format!("{}-{}", sl(f[5], 0, 2)?, sl(f[5], 2, 4)?));
    b.time("Time", f[6]);
    b.time("ETA", f[7]);
    b.f("Fuel", &format!("{} t", trim(f[8])));
    b.done()
}

fn r33(t: &str) -> Option<FormatReading> {
    let f = fields(t, ',');
    let n = f.len();
    if !prefix44(t).starts_with("POS") || n < 9 {
        return None;
    }
    let mut b = pos_b();
    b.pos_pair(coord_tm(f[n - 8], false)?)?;
    if all_digits(trim(f[n - 7])) {
        b.alt100(f[n - 7]);
    } else {
        b.f("Altitude", "ground");
    }
    b.origin(f[n - 6]);
    b.dest(f[n - 5]);
    b.f("Date", f[n - 4]);
    b.time("Time", f[n - 3]);
    b.time("ETA", f[n - 2]);
    b.f("Fuel", f[n - 1]);
    b.done()
}

fn r34(t: &str, key: &str, kind: &'static str, name: &'static str) -> Option<FormatReading> {
    let f = fields(t, ',');
    if !prefix44(t).starts_with(key) || f.len() < 7 {
        return None;
    }
    let mut b = B::new(kind, Category::Oooi);
    b.pos_pair(coord_tm(f[1], false)?)?;
    b.origin(f[2]);
    b.dest(f[3]);
    b.f("Date", f[4]);
    b.time(name, f[5]);
    b.f("Fuel", f[6]);
    b.done()
}

fn r36(t: &str) -> Option<FormatReading> {
    let f = fields(t, ',');
    if !prefix44(t).starts_with("OFF") || f.len() < 8 {
        return None;
    }
    let mut b = B::new("OOOI: wheels off", Category::Oooi);
    b.pos_pair(coord_tm(f[1], false)?)?;
    b.origin(f[2]);
    b.dest(f[3]);
    b.f("Date", f[4]);
    b.time("Off", f[5]);
    b.time("ETA", f[6]);
    b.f("Fuel", f[7]);
    b.done()
}

fn r37(t: &str) -> Option<FormatReading> {
    if !(t.starts_with("/FB") || t.starts_with(" /FB")) {
        return None;
    }
    let f = fields(t, '/');
    if f.len() != 4 {
        return None;
    }
    let d = fields(f[3], ',');
    if d.len() < 6 {
        return None;
    }
    let signed = |s: &str, neg: char| -> Option<f64> {
        let s = trim(s);
        let v = num(from(s, 1)?)?;
        Some(if s.starts_with(neg) { -v } else { v })
    };
    let mut b = pos_b();
    b.pos(signed(d[0], 'S')?, signed(d[1], 'W')?)?;
    b.flight(d[2]);
    b.dest(d[4]);
    b.time("ETA", d[5]);
    b.done()
}

// Label 4A.

fn r38(t: &str) -> Option<FormatReading> {
    if !t.starts_with("DIS") {
        return None;
    }
    let f = fields(t, ',');
    if f.len() < 3 || f[1].len() < 3 || f[2].is_empty() {
        return None;
    }
    let mut b = B::new("Dispatch message", Category::Text);
    b.time("Time", &format!("{}00", from(f[1], 2)?));
    b.f("Callsign", f[2]);
    b.f("Text", &f[3..].join(","));
    b.done()
}

fn r39(t: &str) -> Option<FormatReading> {
    if !t.starts_with("DOOR") {
        return None;
    }
    let w = words(t);
    if w.len() != 3 {
        return None;
    }
    let mut b = B::new("Door event", Category::Oooi);
    b.f("Door", w[0].split('/').nth(1)?);
    b.f("State", w[1]);
    b.time("Time", &format!("{}00", w[2]));
    b.done()
}

fn r40(t: &str) -> Option<FormatReading> {
    if !t.starts_with("01") || !t.contains('\n') {
        return None;
    }
    let l = trim(t.split('\n').next()?);
    let s = l.find('/')?;
    if l.len() < 8 || l.len() < s + 16 {
        return None;
    }
    let mut b = B::new("State change", Category::Oooi);
    b.f("From state", sl(l, 2, 4)?);
    b.f("To state", sl(l, 4, 6)?);
    b.f("Callsign", sl(l, 6, s)?);
    b.time("Time", sl(l, s + 1, s + 7)?);
    b.origin(sl(l, s + 7, s + 11)?);
    b.dest(sl(l, s + 11, s + 15)?);
    b.done()
}

fn r41(t: &str) -> Option<FormatReading> {
    let f = fields(t, ',');
    if f.len() != 11 || !(f[0].len() == 6 && all_digits(f[0])) {
        return None;
    }
    let mut b = B::new("Flight report", Category::Oooi);
    b.time("Time", f[0]);
    b.f("Registration", f[2]);
    b.f("Callsign", f[3]);
    b.origin(f[4]);
    b.dest(f[5]);
    b.done()
}

fn r42(t: &str) -> Option<FormatReading> {
    let f = fields(t, ',');
    if f.len() != 6 || !(f[0].starts_with('N') || f[0].starts_with('S')) {
        return None;
    }
    let mut b = pos_b();
    b.pos_pair(coord_tm(f[0], true)?)?;
    b.alt100(sl(f[1], 6, 9)?);
    let r = format!(
        "{} {} > {} {}",
        trim(from(f[0], 13).unwrap_or("")),
        time_text(sl(f[1], 0, 6)?),
        trim(from(f[1], 9).unwrap_or("")),
        time_text(f[2])
    );
    b.f("Route", &r);
    b.done()
}

fn r43(t: &str) -> Option<FormatReading> {
    let f = fields(t, ',');
    if f.len() != 6 || !(f[0].len() == 6 && all_digits(f[0])) {
        return None;
    }
    let p: String = format!("{}{}", f[4], f[5])
        .chars()
        .filter(|c| *c != ' ' && *c != '.')
        .collect();
    let mut b = pos_b();
    b.pos_pair(coord_tm(&p, true)?)?;
    b.time("Time", f[0]);
    b.time("ETA", f[1]);
    b.alt100(f[3]);
    b.done()
}

// Label 4N.

fn r44(t: &str) -> Option<FormatReading> {
    if t.len() != 51 {
        return None;
    }
    let c = sl(t, 30, 45)?;
    if sl(c, 1, 2) != Some("0") {
        return None;
    }
    let p = format!("{}{}", sl(c, 0, 1)?, from(c, 2)?);
    let mut b = pos_b();
    b.pos_pair(coord_tm(&p, true)?)?;
    b.origin(sl(t, 8, 11)?);
    b.dest(sl(t, 13, 16)?);
    b.alt100(sl(t, 48, 51)?);
    b.done()
}

fn r45(t: &str) -> Option<FormatReading> {
    let f = fields(t, ',');
    if f.len() != 33 {
        return None;
    }
    let mut b = B::new("Flight data", Category::Route);
    b.f("Date", f[3]);
    if trim(f[1]) == "B" {
        if let (Some(a), Some(o)) = (num(f[4]), num(f[5])) {
            b.pos(a, o)?;
        }
        b.alt(num(f[6]));
    }
    b.origin(f[8]);
    b.dest(f[9]);
    b.f("Alternate", f[10]);
    b.f("Arrival runway", f[11].split('/').next().unwrap_or(""));
    b.done()
}

// Label 4T.

fn r46(t: &str) -> Option<FormatReading> {
    let d = fields(t.strip_prefix("AGFSR")?, '/');
    if d.len() != 20 || d[6].len() < 15 {
        return None;
    }
    let p = d[6];
    let part = |a: usize, b: usize| sl(p, a, b).filter(|x| all_digits(x)).and_then(|x| x.parse::<f64>().ok());
    let lat = part(0, 2)? + part(2, 6)? / 100.0 / 60.0;
    let lon = part(7, 10)? + part(10, 14)? / 100.0 / 60.0;
    let mut b = pos_b();
    b.pos(
        sign(from(p, 6)?.chars().next())? * lat,
        sign(from(p, 14)?.chars().next())? * lon,
    )?;
    b.flight(d[0]);
    b.origin(sl(d[3], 0, 3).unwrap_or(""));
    b.dest(from(d[3], 3).unwrap_or(""));
    b.time("Time", sl(d[4], 0, 4).unwrap_or(""));
    b.alt100(d[7]);
    b.done()
}

fn r47(t: &str) -> Option<FormatReading> {
    let d = fields(t.strip_prefix("ETA")?, '/');
    if d.len() != 3 {
        return None;
    }
    let w = words(d[2]);
    if w.len() < 3 {
        return None;
    }
    let mut b = B::new("ETA report", Category::Eta);
    b.flight(d[0]);
    b.dest(w[1]);
    b.time("ETA", sl(w[2], 0, 4)?);
    b.done()
}

// Label 58.

fn r48(t: &str) -> Option<FormatReading> {
    let d = fields(t, '/');
    if d.len() != 8 {
        return None;
    }
    let mut b = pos_b();
    b.pos(coord_d(d[3])?, coord_d(d[4])?)?;
    b.flight(d[0]);
    b.time("Time", d[2]);
    b.alt(num(d[5]));
    b.done()
}

// Label 5Z.

fn r49(t: &str, key: &str, kind: &'static str) -> Option<FormatReading> {
    if !t.starts_with('/') {
        return None;
    }
    let first = t.split('\n').next()?.trim_end_matches('\r');
    let f = fields(first, '/');
    if words(f.get(1)?).first() != Some(&key) {
        return None;
    }
    let f2 = f.get(2)?;
    let w = words(f2);
    if f2.len() <= 4 || w.len() < 4 {
        return None;
    }
    let mut b = B::new(kind, Category::Eta);
    b.origin(w[0]);
    b.dest(w[1]);
    b.done()
}

fn r51(t: &str) -> Option<FormatReading> {
    let f = fields(t, '/');
    if f.len() < 4 {
        return None;
    }
    let w = words(f[2]);
    if w.len() < 3 || trim(f[3]).is_empty() {
        return None;
    }
    let (o, d) = (airport(w[0])?, airport(w[1])?);
    let mut b = B::new("Airline report", Category::Eta);
    b.origin(&o);
    b.dest(&d);
    let w3 = words(f[3]);
    b.time("Time", w3.get(1).or(w3.first()).copied().unwrap_or(""));
    b.done()
}

// Label 80.

/// The header of label 80 lines: flight, origin, destination and registration.
fn header80(b: &mut B, line: &str) -> Option<()> {
    let p = fields(line, '/');
    if p.len() < 3 {
        return None;
    }
    let w0 = words(p[0]);
    let fl = w0.get(2).filter(|x| all_digits(x))?;
    let w1 = words(p[1]);
    if w1.len() != 2 {
        return None;
    }
    let w2 = words(p[2]);
    b.flight(fl);
    b.origin(w1[1]);
    b.dest(w2.first()?);
    if let Some(r) = w2.get(1) {
        b.f("Registration", r);
    }
    Some(())
}

fn r80(t: &str) -> Option<FormatReading> {
    let t = t.replace('\r', "");
    let lines: Vec<&str> = t.split('\n').filter(|l| !l.is_empty()).collect();
    let first = *lines.first()?;
    if first.contains(',') && first.contains("POSRPT") {
        // Row 52.
        let f = fields(first, ',');
        let g = f.iter().position(|x| x.contains("POSRPT"))?;
        if g < 2 {
            return None;
        }
        let la = f[g - 2].rsplit('/').next()?;
        let mut b = pos_b();
        b.pos(hemi_num(la, 100.0)?, hemi_num(f[g - 1], 100.0)?)?;
        header80(&mut b, f[g])?;
        return b.done();
    }
    if lines.len() == 1 {
        if first.contains(',') {
            // Row 53.
            let f = fields(first, ',');
            let w = words(f[0]);
            if w.get(1) != Some(&"POS") {
                return None;
            }
            let mut b = pos_b();
            b.pos_pair(coord_tm(w.get(2)?, true)?)?;
            b.time("Time", f.get(2)?);
            let _ = header80(&mut b, f[0]);
            return b.done();
        }
        // Row 54.
        let mut b = B::new("Flight status", Category::Oooi);
        header80(&mut b, first)?;
        return b.done();
    }
    // Row 55.
    if fields(first, '/').len() < 3 {
        return None;
    }
    let mut b = pos_b();
    let mut keys = 0;
    for l in &lines[1..] {
        for part in l.split('/') {
            let (k, v) = key_value(part);
            keys += 1;
            match k {
                "POS" => {
                    let e = v.find(['E', 'W'])?;
                    let div = if v.contains('.') { 100.0 } else { 1000.0 };
                    b.pos(hemi_num(&v[..e], div)?, hemi_num(&v[e..], div)?)?;
                }
                "ALT" => b.alt(num(v)),
                "FL" => b.alt100(v),
                "HDG" => b.heading(v),
                "FB" | "FOB" => {
                    let d: String = v.chars().filter(char::is_ascii_digit).collect();
                    b.f("Fuel", &format!("{d} lb or kg"));
                }
                "ETA" => b.time("ETA", v.split('.').next().unwrap_or("")),
                "UTC" => b.time("Time", v),
                "TAS" => b.f("True airspeed", &format!("{v} kn")),
                "SPD" => b.f("Ground speed", &format!("{v} kn")),
                "MCH" => b.f("Mach", v),
                _ => keys -= 1,
            }
        }
    }
    if keys == 0 {
        return None;
    }
    header80(&mut b, first)?;
    b.done()
}

// Label 83.

fn r56(t: &str) -> Option<FormatReading> {
    if !t.starts_with("001PR") {
        return None;
    }
    let p = sl(t, 13, 28)?.replace('.', "");
    let mut b = pos_b();
    b.pos_pair(coord_tm(&p, false)?)?;
    b.alt(num(sl(t, 28, 33)?));
    b.done()
}

fn r57(t: &str) -> Option<FormatReading> {
    if !t.starts_with("4DH3 ETAT2") {
        return None;
    }
    let w = words(t);
    if w.len() < 7 {
        return None;
    }
    let (o, d) = w[3].split_once('/')?;
    let mut b = B::new("ETA report", Category::Eta);
    b.origin(o);
    b.dest(d);
    b.f("Registration", w[4]);
    b.time("ETA", &format!("{}00", w[6]));
    b.done()
}

fn r58(t: &str) -> Option<FormatReading> {
    let t: String = t.chars().filter(|c| !matches!(c, ' ' | '\r' | '\n')).collect();
    let f = fields(&t, ',');
    if f.len() != 9 {
        return None;
    }
    let (la, lo) = (num(f[3])?, num(f[4])?);
    let mut b = pos_b();
    b.pos(la, lo)?;
    b.origin(f[0]);
    b.dest(f[1]);
    b.alt(num(f[5]));
    b.f("Ground speed", &format!("{} kn", f[6]));
    b.heading(f[7]);
    b.done()
}

// Labels 8E, B6, :;.

fn r59(t: &str) -> Option<FormatReading> {
    let f = fields(t, ',');
    let ok = f.len() == 2
        && f[0].len() == 4
        && f[0].bytes().all(|c| c.is_ascii_uppercase() || c.is_ascii_digit())
        && f[1].len() == 4
        && all_digits(f[1]);
    if !ok {
        return None;
    }
    let mut b = B::new("ETA report", Category::Eta);
    b.dest(f[0]);
    b.time("ETA", f[1]);
    b.done()
}

fn r60(t: &str) -> Option<FormatReading> {
    if !t.starts_with('/') || t.len() <= 1 {
        return None;
    }
    let mut b = B::new("Airline message", Category::Text);
    b.f("Text", &t[1..]);
    b.done()
}

fn r61(t: &str) -> Option<FormatReading> {
    let d = trim(t);
    if d.len() < 5 || !all_digits(d) {
        return None;
    }
    let v: f64 = d.parse().ok()?;
    let mut b = B::new("Datalink frequency", Category::Other);
    b.f("Frequency", &format!("{:.3} MHz", v / 1000.0));
    b.done()
}

// Label H1.

fn r62(t: &str) -> Option<FormatReading> {
    let (kind, key) = if t.starts_with("FLR") || t.starts_with("#CFBFLR") {
        ("Fault report", "/FR")
    } else if t.starts_with("WRN") || t.starts_with("#CFBWRN") {
        ("Warning message", "/WN")
    } else {
        return None;
    };
    let at = t.find(key)? + 3;
    let d = sl(t, at, at + 20)?;
    let mut b = B::new(kind, Category::Fault);
    let when = format!(
        "{}-{} {}:{} UTC",
        sl(d, 4, 6)?,
        sl(d, 2, 4)?,
        sl(d, 6, 8)?,
        sl(d, 8, 10)?
    );
    b.f("Time", &when);
    b.f("Text", from(t, at + 20)?);
    b.done()
}

fn r63(t: &str) -> Option<FormatReading> {
    if !t.starts_with("*POS") || t.len() != 43 {
        return None;
    }
    let n = |a, b| sl(t, a, b).filter(|x| all_digits(x)).and_then(|x| x.parse::<f64>().ok());
    let lat = sign(from(t, 12)?.chars().next())? * (n(13, 15)? + n(15, 17)? / 60.0);
    let lon = sign(from(t, 17)?.chars().next())? * (n(18, 21)? + n(21, 23)? / 60.0);
    let mut b = pos_b();
    b.pos(lat, lon)?;
    b.f("Date", &format!("{}-{}", sl(t, 4, 6)?, sl(t, 6, 8)?));
    b.time("Time", sl(t, 8, 12)?);
    b.alt(num(sl(t, 23, 28)?));
    b.done()
}

fn r64(t: &str) -> Option<FormatReading> {
    let rest = t.strip_prefix("(POS-")?;
    let sp = rest.find(' ')?;
    let slash = rest.find('/')?;
    if slash <= sp {
        return None;
    }
    let mid = trim(&rest[sp..slash]);
    let mid = mid.strip_prefix('-').unwrap_or(mid);
    let h = mid.find(['N', 'S'])?;
    let la = &mid[..h];
    if la.len() < 3 || !all_digits(la) {
        return None;
    }
    let lat_deg: f64 = la[..la.len() - 2].parse().ok()?;
    let lat_min: f64 = la[la.len() - 2..].parse().ok()?;
    let lat = sign(mid[h..].chars().next())? * (lat_deg + lat_min / 60.0);
    let lo = from(mid, h + 1)?;
    if lo.len() != 6 || !lo.is_ascii() || !all_digits(&lo[..5]) {
        return None;
    }
    let lon = sign(lo[5..].chars().next())? * (lo[..3].parse::<f64>().ok()? + lo[3..5].parse::<f64>().ok()? / 60.0);
    let mut b = pos_b();
    b.pos(lat, lon)?;
    b.flight(&rest[..sp]);
    b.time("Time", sl(rest, slash + 1, slash + 7)?);
    if let Some(i) = rest.find(" F") {
        b.alt100(sl(rest, i + 2, i + 5).filter(|x| all_digits(x)).unwrap_or(""));
    }
    b.done()
}

fn r65(t: &str) -> Option<FormatReading> {
    if !t.starts_with("76401") {
        return None;
    }
    let lines: Vec<&str> = t.split('\n').map(|l| l.trim_end_matches('\r')).collect();
    if lines.len() < 3 {
        return None;
    }
    let l1 = trim(lines[1]);
    let mut b = pos_b();
    b.origin(sl(l1, 5, 9).unwrap_or(""));
    b.dest(sl(l1, 9, 13).unwrap_or(""));
    let mut found = false;
    for l in &lines[2..] {
        if l.len() < 19 {
            continue;
        }
        if let Some(p) = sl(l, 0, 13).and_then(|p| coord_tm(p, true)) {
            if !found {
                b.pos_pair(p)?;
                b.time("Time", sl(l, 13, 19)?);
                found = true;
            }
        }
    }
    if !found {
        return None;
    }
    b.done()
}

fn r66(t: &str) -> Option<FormatReading> {
    let b0 = t.as_bytes();
    let ok = t.len() > 10
        && b0[0] == b'M'
        && b0[1..3].iter().all(u8::is_ascii_digit)
        && b0[3] == b'A'
        && b0[6..10].iter().all(u8::is_ascii_digit);
    if !ok {
        return None;
    }
    let f = fields(from(t, 10)?, ',');
    if f.len() < 7 {
        return None;
    }
    let mut b = pos_b();
    b.pos(num(f[3])?, num(f[4])?)?;
    b.flight(sl(t, 4, 10)?);
    b.origin(f[0]);
    b.dest(f[1]);
    b.alt(num(f[5]));
    b.heading(f[6]);
    b.done()
}

fn r67(t: &str) -> Option<FormatReading> {
    let b0 = t.as_bytes();
    if t.len() <= 12 || b0[0] != b'L' || !b0[1..3].iter().all(u8::is_ascii_digit) {
        return None;
    }
    let slash = t.find('/')?;
    let ti = slash + t[slash..].find(".TI2/")?;
    let mut b = B::new("D-ATIS request", Category::Weather);
    b.flight(sl(t, 4, slash)?);
    b.f("Airport", sl(t, ti + 5, ti + 9)?);
    b.done()
}

fn r69(t: &str) -> Option<FormatReading> {
    if !t.starts_with("(FPL-") {
        return None;
    }
    let t = t.replace('\r', "").replace('\n', " ");
    let p = fields(&t, '-');
    if p.len() < 7 {
        return None;
    }
    let mut b = B::new("Flight plan", Category::Route);
    b.flight(p[1]);
    b.origin(sl(p[4], 0, 4).unwrap_or(""));
    if p[5].len() > 8 {
        let s: String = p[5].chars().take(180).collect();
        b.f("Text", &s);
    }
    for x in &p[6..] {
        let x = trim(x);
        if let (Some(a), Some(e)) = (sl(x, 0, 4).and_then(airport), sl(x, 4, 8).filter(|e| all_digits(e))) {
            if a.len() == 4 {
                b.dest(&a);
                b.time("ETA", e);
                break;
            }
        }
    }
    b.done()
}

// Labels H2, HX, SQ, QQ.

fn r70(t: &str) -> Option<FormatReading> {
    if !t.starts_with("02E") {
        return None;
    }
    let w = words(t);
    if w.last() != Some(&"Q") || w[0].len() != 45 {
        return None;
    }
    let mut b = B::new("Wind report", Category::Weather);
    b.origin(sl(w[0], 5, 9)?);
    b.dest(sl(w[0], 9, 13)?);
    let mut entries = vec![from(w[0], 13)?];
    for x in &w[1..w.len() - 1] {
        entries.push(from(x, 1).unwrap_or(""));
    }
    let mut n = 0;
    for e in entries {
        if e.len() != 32 {
            continue;
        }
        let Some(p) = sl(e, 0, 13).and_then(|p| coord_tm(p, true)) else {
            continue;
        };
        if n == 0 {
            b.pos_pair(p)?;
        }
        let wind = format!(
            "FL{}: {} deg / {} kn",
            sl(e, 17, 20)?,
            sl(e, 25, 28)?,
            sl(e, 28, 31)?
        );
        b.f("Wind", &wind);
        n += 1;
    }
    if n == 0 {
        return None;
    }
    b.done()
}

fn r71(t: &str) -> Option<FormatReading> {
    if !t.starts_with("RA FMT ") {
        return None;
    }
    let w = words(t);
    if w.get(2) != Some(&"LOCATION") || w.get(3)?.len() < 7 || w.get(4)?.len() < 8 {
        return None;
    }
    let mut b = pos_b();
    b.pos(dmm(w[3], 2)?, dmm(w[4], 3)?)?;
    b.done()
}

fn r72(t: &str) -> Option<FormatReading> {
    if !t.starts_with("RA FMT ") {
        return None;
    }
    let w = words(t);
    if w.get(2) != Some(&"43") {
        return None;
    }
    let mut b = B::new("Flight data", Category::Route);
    b.origin(w.get(3)?);
    b.done()
}

fn r73(t: &str) -> Option<FormatReading> {
    let b0 = t.as_bytes();
    if t.len() < 16 || b0[0] != b'0' || b0[2] != b'X' {
        return None;
    }
    let v = 10 + t.get(10..)?.find('V')?;
    let rest = sl(t, 12, v)?;
    let h = rest.find(['N', 'S'])?;
    let e = rest.find(['E', 'W'])?;
    if e <= h {
        return None;
    }
    let lat = sign(rest[h..].chars().next())? * num(&rest[..h])? / 100.0;
    let lon = sign(rest[e..].chars().next())? * num(&rest[h + 1..e])? / 100.0;
    let mut b = B::new("Ground station squitter", Category::Other);
    b.pos(lat, lon)?;
    b.f("Station", &format!("{} / {}", sl(t, 4, 7)?, sl(t, 7, 11)?));
    let fr: String = t[v + 1..].chars().take_while(|c| *c != '/').collect();
    if let Some(x) = num(&fr) {
        b.f("VDL frequency", &format!("{:.3} MHz", x / 1000.0));
    }
    b.done()
}

fn r74(t: &str) -> Option<FormatReading> {
    if sl(t, 12, 19) != Some("\r\n001FE") || t.len() < 48 {
        return None;
    }
    let lat = sign(from(t, 27)?.chars().next())? * (num(sl(t, 28, 30)?)? + num(sl(t, 30, 34)?)? / 60.0);
    let lon = sign(from(t, 34)?.chars().next())? * (num(sl(t, 35, 38)?)? + num(sl(t, 38, 42)?)? / 60.0);
    let mut b = B::new("OOOI: wheels off", Category::Oooi);
    b.pos(lat, lon)?;
    b.origin(sl(t, 0, 4)?);
    b.dest(sl(t, 4, 8)?);
    b.time("Off", sl(t, 21, 27)?);
    if let Some(g) = sl(t, 45, 48).filter(|g| all_digits(g)) {
        b.f("Ground speed", &format!("{g} kn"));
    }
    b.done()
}

/// Reads a QQ text when it is the wheels-off layout with a position (tried before the OOOI columns of the Q labels).
pub fn read_qq(text: &str) -> Option<FormatReading> {
    r74(text)
}

// The ARINC 702 form.

const TYPES: [(&str, &str, Category); 10] = [
    ("POS", "Position report", Category::Position),
    ("FPN", "Flight plan", Category::Route),
    ("INI", "Flight initialisation", Category::Other),
    ("FTX", "Free text", Category::Text),
    ("PRG", "Performance data", Category::Other),
    ("PRF", "Performance data", Category::Other),
    ("REQ", "Request", Category::Other),
    ("LDI", "Landing data", Category::Other),
    ("SUM", "Flight summary", Category::Other),
    ("PER", "Report", Category::Other),
];

/// A position element: M-form position, [waypoint], [time HHMMSS], [flight level].
fn position_element(b: &mut B, s: &str) -> Option<()> {
    let items = fields(s, ',');
    b.pos_pair(coord_tm(items[0], false)?)?;
    let mut i = 1;
    if let Some(w) = items.get(1) {
        if !(w.len() == 6 && all_digits(w)) {
            b.f("Waypoint", w);
            i = 2;
        }
    }
    if let Some(tm) = items.get(i) {
        b.time("Time", tm);
    }
    if let Some(fl) = items.get(i + 1).filter(|x| all_digits(trim(x))) {
        b.alt100(fl);
    }
    Some(())
}

fn arinc702(text: &str) -> Option<FormatReading> {
    let mut flight = None;
    let mut t = text;
    if let Some(h) = text.find('#') {
        if text.len() >= h + 4 {
            if h >= 8 {
                flight = sl(text, 4, h);
            }
            let skip = if ["#M1B", "#T1B", "#CFB"].iter().any(|p| text[h..].starts_with(p)) {
                4
            } else {
                3
            };
            t = from(text, h + skip)?;
        }
    }
    let (at, kind, cat) = TYPES
        .iter()
        .filter_map(|(k, kind, cat)| {
            let mut from_i = 0;
            while let Some(i) = t.get(from_i..).and_then(|s| s.find(k)).map(|i| i + from_i) {
                if i > 14 {
                    return None;
                }
                if matches!(t.as_bytes().get(i + 3), Some(b'/' | b'N' | b'S')) {
                    return Some((i, *kind, *cat));
                }
                from_i = i + 1;
            }
            None
        })
        .min_by_key(|x| x.0)?;
    let body = &t[at..];
    let f = fields(body, '/');
    let mut b = B::new(kind, cat);
    if let Some(fl) = flight {
        b.flight(fl);
    }
    if kind == "Position report" {
        let after = from(f[0], 3).unwrap_or("");
        if !after.is_empty() {
            position_element(&mut b, after)?;
        }
    }
    for x in &f[1..] {
        let (id, v) = (sl(x, 0, 2).unwrap_or(""), from(x, 2).unwrap_or(""));
        match id {
            "ID" => {
                let p = fields(v, ',');
                b.f("Tail", p[0]);
                if let Some(fl) = p.get(1) {
                    b.flight(fl);
                }
            }
            "FN" => b.flight(v),
            "PS" => position_element(&mut b, v)?,
            "AF" => {
                let p = fields(v, ',');
                b.origin(p[0]);
                if let Some(d) = p.get(1) {
                    b.dest(d);
                }
            }
            "DT" => {
                if let Some(a) = v.split(',').find(|a| airport(a).is_some_and(|a| a.len() == 4)) {
                    b.dest(a);
                }
            }
            "FX" => b.f("Text", v),
            "ET" => {
                let v = trim(v);
                b.time("ETA", v.get(v.len().saturating_sub(4)..).unwrap_or(""));
            }
            "RP" | "RI" | "RM" | "RF" | "RS" => {
                let items = fields(v, ':');
                for (k, w) in items.iter().zip(items.iter().skip(1)) {
                    match *k {
                        "DA" => b.origin(w),
                        "AA" => b.dest(w),
                        _ => {}
                    }
                }
            }
            _ => {}
        }
    }
    b.done()
}
