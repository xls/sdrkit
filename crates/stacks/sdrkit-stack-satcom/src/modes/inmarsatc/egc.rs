//! Inmarsat-C packets and system information, and EGC messages: the bulletin board, the signalling channel, the EGC header, the
//! assembly rules, the message text and what a message is about (area, warning number, subject).
//! Standard: Inmarsat-C System Definition Manual; IMO SafetyNET Manual.

use sdrkit_util_objects::commands::Tables;
use sdrkit_util_objects::{Format, ObjectTable, Shift, Stamp};
use sdrkit_util_text::ia5::{printable_into, Printable};
use sdrkit_util_text::ita2::{Decoder as Ita2Decoder, Encoder as Ita2Encoder, ITU};
use serde::{Deserialize, Serialize};

/// The NCS information from a bulletin board.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Bulletin {
    pub network_version: u8,
    pub frame_number: u16,
    pub signalling_channel: u8,
    pub count: u8,
    pub channel_type: u8,
    pub ocean_region: u8,
    pub station_id: u8,
    pub status: u8,
    pub services: u16,
    pub random_interval: u8,
}

impl Bulletin {
    pub fn parse(p: &[u8]) -> Option<Bulletin> {
        if p.len() < 14 || p[0] != 0x7D {
            return None;
        }
        Some(Bulletin {
            network_version: p[1],
            frame_number: u16::from_be_bytes([p[2], p[3]]),
            signalling_channel: p[4] >> 2,
            count: (p[5] >> 4) * 2,
            channel_type: p[6] >> 5,
            ocean_region: p[7] >> 6,
            station_id: p[7] & 63,
            status: p[8],
            services: u16::from_be_bytes([p[9], p[10]]),
            random_interval: p[11],
        })
    }
    pub fn channel_type_name(&self) -> &'static str {
        channel_type_name(self.channel_type)
    }
    pub fn status_text(&self) -> String {
        let s = self.status;
        let mut v = Vec::new();
        if s & 0x80 != 0 {
            v.push("600 baud");
        }
        if s & 0x40 != 0 {
            v.push("operational");
        }
        if s & 0x20 != 0 {
            v.push("in service");
        }
        if s & 0x10 != 0 {
            v.push("clear");
        }
        if s & 0x08 != 0 {
            v.push("links open");
        }
        v.join(", ")
    }
    pub fn services_text(&self) -> String {
        const NAMES: [&str; 16] = [
            "Distress alerting",
            "SafetyNET",
            "Inmarsat-C",
            "Store and forward",
            "Half duplex",
            "Full duplex",
            "Closed network",
            "FleetNET",
            "Prefix SF",
            "Land mobile alerting",
            "Aero-C",
            "ITA2",
            "Data",
            "Basic X.400",
            "Enhanced X.400",
            "Low power CMES",
        ];
        (0..16)
            .filter(|i| self.services & (0x8000 >> i) != 0)
            .map(|i| NAMES[i])
            .collect::<Vec<_>>()
            .join(", ")
    }
}

/// The time of a frame number (frame x 8.64 s) as h:mm:ss.s.
pub fn frame_time(frame: u32) -> String {
    let t = frame as f64 * 8.64;
    let h = (t / 3600.0).floor();
    let m = ((t - h * 3600.0) / 60.0).floor();
    let s = t - h * 3600.0 - m * 60.0;
    format!(
        "{}:{:02}:{:04.1}",
        h as u32,
        m as u32,
        (s * 10.0).floor() / 10.0
    )
}

pub fn channel_type_name(t: u8) -> &'static str {
    match t {
        1 => "NCS",
        2 => "LES TDM",
        3 => "Joint NCS and TDM",
        4 => "Stand-by NCS",
        _ => "Reserved",
    }
}

pub fn region_name(r: u8) -> &'static str {
    match r {
        0 => "Atlantic Ocean Region West (AOR-W)",
        1 => "Atlantic Ocean Region East (AOR-E)",
        2 => "Pacific Ocean Region (POR)",
        3 => "Indian Ocean Region (IOR)",
        9 => "All ocean regions of the station",
        _ => "Unknown",
    }
}

/// A station's name by region and id (the published LES list; key region x 100 + station).
pub fn station_name(region: u8, id: u8) -> &'static str {
    if id == 44 {
        return "NCS";
    }
    match region as u32 * 100 + id as u32 {
        1 | 101 | 201 | 301 => "Vizada-Telenor USA",
        2 | 12 | 102 | 112 | 302 | 312 => "Stratos Global Burum Netherlands",
        202 => "Stratos Global Auckland New Zealand",
        3 | 103 | 203 | 303 => "KDDI Japan",
        4 | 104 | 204 | 304 => "Vizada-Telenor Norway",
        21 | 121 | 221 | 321 => "Vizada (FT) France",
        105 | 335 => "Telecom Italia",
        120 | 305 => "OTESTAT Greece",
        306 => "VSNL India",
        110 | 310 => "Turk Telecom",
        211 | 311 => "Beijing MCN China",
        114 => "Embratel Brazil",
        116 | 316 => "Telekomunikacja Polska",
        117 | 217 | 317 => "Morsviazsputnik Russia",
        127 | 327 => "Bezeq Israel",
        210 | 328 => "Singapore Telecom",
        330 => "VISHIPEL Vietnam",
        _ => "",
    }
}

/// The uplink frequency of a signalling channel packet (0x6C), MHz.
pub fn signalling_uplink_mhz(p: &[u8]) -> Option<f64> {
    if p.len() < 6 || p[0] != 0x6C {
        return None;
    }
    let ch = u16::from_be_bytes([p[2], p[3]]) as f64;
    Some(1626.5 + (ch - 6000.0) * 0.0025)
}

/// The name of a packet descriptor.
pub fn descriptor_name(d: u8) -> &'static str {
    match d {
        0x7D => "Bulletin board",
        0x6C => "Signalling channel",
        0xB1 => "EGC double header 1",
        0xB2 => "EGC double header 2",
        0xBD => "Multiframe packet",
        0xBE => "Multiframe continuation",
        0x27 => "Logical channel clear",
        0x2A => "Inbound message ack",
        0x08 => "Acknowledgement request",
        0x81 => "Announcement",
        0x83 => "Logical channel assignment",
        0x91 => "Distress alert ack",
        0x92 => "Login ack",
        0x9A => "Enhanced data report ack",
        0xA0 => "Distress test request",
        0xA3 => "Individual poll",
        0xA8 => "Confirmation",
        0xAA => "Message",
        0xAB => "LES list",
        0xAC => "Request status",
        0xAD => "Test result",
        _ => "Unknown",
    }
}

/// An EGC service.
pub fn service(code: u8) -> (&'static str, &'static str, usize) {
    match code {
        0x00 => ("System, all ships (general call)", "system", 3),
        0x02 => ("FleetNET, group call", "FleetNET", 5),
        0x04 => ("SafetyNET, warning to a rectangular area", "SafetyNET", 7),
        0x11 => ("System, Inmarsat system message", "system", 4),
        0x13 => ("SafetyNET, coastal warning", "SafetyNET", 6),
        0x14 => (
            "SafetyNET, shore-to-ship distress alert to a circular area",
            "SafetyNET",
            7,
        ),
        0x23 => ("System, EGC system message", "system", 6),
        0x24 => ("SafetyNET, warning to a circular area", "SafetyNET", 7),
        0x31 => (
            "SafetyNET, NAVAREA/METAREA warning or forecast",
            "SafetyNET",
            4,
        ),
        0x33 => ("System, download group identity", "system", 6),
        0x34 => (
            "SafetyNET, SAR coordination to a rectangular area",
            "SafetyNET",
            7,
        ),
        0x44 => (
            "SafetyNET, SAR coordination to a circular area",
            "SafetyNET",
            7,
        ),
        0x72 => ("FleetNET, chart correction service", "FleetNET", 5),
        0x73 => (
            "SafetyNET, chart correction service for fixed areas",
            "SafetyNET",
            6,
        ),
        _ => ("Unknown service", "other", 3),
    }
}

pub fn priority_name(p: u8) -> &'static str {
    match p {
        0 => "Routine",
        1 => "Safety",
        2 => "Urgency",
        _ => "Distress",
    }
}

/// An EGC packet's header and payload.
#[derive(Clone, Debug, PartialEq)]
pub struct EgcPacket {
    pub descriptor: u8,
    pub service: u8,
    pub continuation: bool,
    pub priority: u8,
    pub repetition: u8,
    pub message_number: u16,
    pub packet_number: u8,
    pub presentation: u8,
    pub address: Vec<u8>,
    pub payload: Vec<u8>,
}

impl EgcPacket {
    /// Parses a 0xB1 / 0xB2 packet (check bytes included at its end; `checked` false when there are none).
    pub fn parse(p: &[u8], checked: bool) -> Option<EgcPacket> {
        if p.len() < 9 || !(p[0] == 0xB1 || p[0] == 0xB2) {
            return None;
        }
        let end = if checked { p.len() - 2 } else { p.len() };
        let svc = p[2];
        let flags = p[3];
        let (_, _, alen) = service(svc);
        if 8 + alen > end {
            return None;
        }
        Some(EgcPacket {
            descriptor: p[0],
            service: svc,
            continuation: flags & 0x80 != 0,
            priority: (flags >> 5) & 3,
            repetition: flags & 0x1F,
            message_number: u16::from_be_bytes([p[4], p[5]]),
            packet_number: p[6],
            presentation: p[7],
            address: p[8..8 + alen].to_vec(),
            payload: p[8 + alen..end].to_vec(),
        })
    }

    /// Builds the packet bytes (with check bytes).
    pub fn build(&self) -> Vec<u8> {
        let mut v = vec![self.descriptor, 0, self.service];
        v.push(
            (self.continuation as u8) << 7 | (self.priority & 3) << 5 | (self.repetition & 0x1F),
        );
        v.extend_from_slice(&self.message_number.to_be_bytes());
        v.push(self.packet_number);
        v.push(self.presentation);
        v.extend_from_slice(&self.address);
        v.extend_from_slice(&self.payload);
        v.extend_from_slice(&[0, 0]);
        v[1] = (v.len() - 2) as u8;
        super::coding::seal(&mut v);
        v
    }
}

/// IA5 text: ETX ends it, a carriage return and line feed become one line break, tabs stay, other control characters go.
pub fn ia5(b: &[u8]) -> String {
    let opt = Printable {
        replacement: None,
        tabs: true,
        stop_at_etx: true,
        ..Printable::default()
    };
    let mut s = String::new();
    printable_into(b, &opt, &mut s);
    s
}

/// ITA2 text (5-bit codes, one per byte) in the international figures case of ITU-R M.625 Table 1 (ITU-T S.1). Carriage
/// returns go, line feeds stay.
pub fn ita2(b: &[u8]) -> String {
    let mut d = Ita2Decoder::new(&ITU);
    b.iter()
        .filter_map(|&c| d.push(c & 0x1F))
        .filter(|&c| c != '\r')
        .collect()
}

/// Encodes text to ITA2 codes in the international table (for the generator). Characters the table has no code for are left
/// out.
pub fn ita2_encode(t: &str) -> Vec<u8> {
    let mut e = Ita2Encoder::new(&ITU);
    let mut out = Vec::new();
    for c in t.chars() {
        let _ = e.encode_char(c, &mut |code| out.push(code));
    }
    out
}

/// Text of a payload by its presentation.
pub fn text_of(presentation: u8, payload: &[u8]) -> String {
    match presentation {
        0 => ia5(payload),
        6 => ita2(payload),
        _ => format!("[binary, {} bytes]", payload.len()),
    }
}

/// A Roman numeral I to XXI, or a number 1 to 21; 0 otherwise.
pub fn area_number(s: &str) -> u32 {
    let u = s.to_ascii_uppercase();
    if let Ok(n) = u.parse::<u32>() {
        return if (1..=21).contains(&n) { n } else { 0 };
    }
    const R: [&str; 21] = [
        "I", "II", "III", "IV", "V", "VI", "VII", "VIII", "IX", "X", "XI", "XII", "XIII", "XIV",
        "XV", "XVI", "XVII", "XVIII", "XIX", "XX", "XXI",
    ];
    R.iter()
        .position(|&r| r == u)
        .map(|i| i as u32 + 1)
        .unwrap_or(0)
}

/// The area named in a text: "NAVAREA IV" (Roman) or "METAREA 9" (number), as written.
pub fn area(text: &str) -> String {
    let words: Vec<&str> = text
        .split(|c: char| !c.is_ascii_alphanumeric())
        .filter(|w| !w.is_empty())
        .collect();
    for (i, w) in words.iter().enumerate() {
        let u = w.to_ascii_uppercase();
        if (u == "NAVAREA" || u == "METAREA") && i + 1 < words.len() {
            let n = words[i + 1];
            if area_number(n) > 0 {
                return format!("{u} {}", n.to_ascii_uppercase());
            }
        }
    }
    String::new()
}

/// The first `n/yy` (1 to 4 digits, a slash, exactly 2 digits) in the first 200 characters.
pub fn warning_number(text: &str) -> String {
    let t: Vec<char> = text.chars().take(200).collect();
    for i in 0..t.len() {
        if t[i] != '/' {
            continue;
        }
        let mut a = i;
        while a > 0 && t[a - 1].is_ascii_digit() {
            a -= 1;
        }
        let nd = i - a;
        if !(1..=4).contains(&nd) || (a > 0 && t[a - 1].is_ascii_alphanumeric()) {
            continue;
        }
        if i + 2 < t.len() && t[i + 1].is_ascii_digit() && t[i + 2].is_ascii_digit() {
            if i + 3 < t.len() && t[i + 3].is_ascii_digit() {
                continue;
            }
            return t[a..i + 3].iter().collect();
        }
    }
    String::new()
}

/// What a message is about.
pub fn subject(service_code: u8, priority: u8, text: &str) -> &'static str {
    if matches!(service_code, 0x00 | 0x11 | 0x23 | 0x33) {
        return "";
    }
    let u = text.to_ascii_uppercase();
    let has = |w: &str| u.contains(w);
    if priority == 3
        || has("MAYDAY")
        || (has("DISTRESS") && !has("DISTRESS CALL"))
        || service_code == 0x14
    {
        return "Distress relay";
    }
    if matches!(service_code, 0x34 | 0x44)
        || has("SEARCH AND RESCUE")
        || has(" SAR ")
        || has("SAR OPERATION")
    {
        return "Search and rescue";
    }
    if has("FORECAST") || has("WEATHER BULLETIN") || has("SYNOPSIS") || has("OUTLOOK") {
        return "Weather forecast";
    }
    if has("GALE")
        || has("STORM")
        || has("CYCLONE")
        || has("HURRICANE")
        || has("TYPHOON")
        || has("SEVERE WEATHER")
    {
        return "Weather warning";
    }
    if has("PIRAT") || has("ARMED ROBBERY") || has("SECURITY") {
        return "Security / piracy";
    }
    if has("ICE ") || has("ICEBERG") || has("SEA ICE") {
        return "Ice report";
    }
    if service_code == 0x13 {
        return "Coastal warning";
    }
    if matches!(service_code, 0x72 | 0x73) {
        return "Chart correction";
    }
    if service_code == 0x02 {
        return "FleetNET message";
    }
    if matches!(service_code, 0x31 | 0x04 | 0x24)
        || has("NAVAREA")
        || has("HYDROLANT")
        || has("HYDROPAC")
        || has("HYDROARC")
        || has("NAV WARN")
    {
        return "Navigational warning";
    }
    ""
}

/// One EGC message as kept.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct EgcMessage {
    pub message_number: u16,
    pub service_code: u8,
    pub service_text: String,
    pub kind: String,
    pub priority: u8,
    pub priority_name: String,
    pub presentation: String,
    pub packets: usize,
    pub complete: bool,
    pub times_heard_complete: u32,
    pub channel: usize,
    pub first_frame: u32,
    pub area: String,
    pub subject: String,
    pub warning_number: String,
    pub address_hex: String,
    pub station_guess: String,
    pub text: String,
    pub truncated: bool,
    /// Parts by (packet number, descriptor).
    #[serde(skip)]
    parts: Vec<((u8, u8), Vec<u8>)>,
    #[serde(skip)]
    pass: Vec<((u8, u8), Vec<u8>)>,
    #[serde(skip)]
    last_number: Option<u8>,
    #[serde(skip)]
    first_part: Vec<u8>,
    #[serde(skip)]
    presentation_code: u8,
    /// The event for this message was sent.
    pub announced: bool,
}

impl Shift for EgcMessage {
    /// A message holds no signal times of its own (the table's `Seen` holds them).
    fn shift_times(&mut self, _by_s: f64) {}
}

/// The key of an EGC message: its number and its service code.
#[derive(
    Clone, Copy, Debug, Default, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize,
)]
pub struct MsgKey {
    pub number: u16,
    pub service: u8,
}

impl std::fmt::Display for MsgKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}:{:02X}", self.number, self.service)
    }
}

/// The longest text kept for one message, in characters.
pub const TEXT_CAP: usize = 20_000;

/// The EGC message list with the assembly rules: the least recently heard message leaves first when the list is full.
#[derive(Serialize, Deserialize)]
pub struct EgcAssembler {
    pub messages: ObjectTable<MsgKey, EgcMessage>,
    /// Messages seen since the start (or the last clear); it can exceed the list's cap.
    pub seen: u64,
}

impl Shift for EgcAssembler {
    fn shift_times(&mut self, by_s: f64) {
        self.messages.shift_times(by_s);
    }
}

impl Tables for EgcAssembler {
    const FORMAT: Format = Format::new("satcom.inmarsatc", 1);
    fn clear(&mut self) {
        self.messages.clear();
        self.seen = 0;
    }
    fn adopt(&mut self, saved: Self) {
        self.messages.adopt(saved.messages);
        self.seen = saved.seen;
    }
}

impl EgcAssembler {
    pub fn new(cap: usize) -> Self {
        EgcAssembler {
            messages: ObjectTable::new(cap),
            seen: 0,
        }
    }

    fn complete_set(parts: &[((u8, u8), Vec<u8>)], last: Option<u8>) -> bool {
        let Some(l) = last else { return false };
        (1..=l).all(|n| parts.iter().any(|((pn, _), _)| *pn == n))
    }

    /// Adds a packet with a good check, heard at `at`. Returns the key of a message that just became complete for the first time.
    pub fn add(&mut self, p: &EgcPacket, at: Stamp, channel: usize, frame: u32) -> Option<MsgKey> {
        let key = MsgKey {
            number: p.message_number,
            service: p.service,
        };
        let first = p.packet_number == 1 && p.descriptor == 0xB1;
        let raw: &[u8] = &p.payload;
        let up = self.messages.upsert(&key, at, EgcMessage::default).ok()?;
        if up.new {
            self.seen += 1;
            up.row.first_frame = frame;
        }
        let m = up.row;
        if first && !m.first_part.is_empty() && m.first_part != raw {
            // The number was reused by another message: start again.
            *m = EgcMessage {
                first_frame: frame,
                ..Default::default()
            };
        } else if first && m.complete && m.first_part == raw {
            // A new broadcast pass of a complete message.
            m.pass.clear();
        }
        m.message_number = p.message_number;
        m.service_code = p.service;
        let (name, kind, _) = service(p.service);
        m.service_text = name.into();
        m.kind = kind.into();
        m.priority = p.priority;
        m.priority_name = priority_name(p.priority).into();
        m.presentation_code = p.presentation;
        m.presentation = match p.presentation {
            0 => "IA5",
            6 => "ITA2",
            7 => "binary",
            _ => "other",
        }
        .into();
        m.channel = channel;
        if first {
            m.first_part = raw.to_vec();
            if !p.address.is_empty() {
                m.address_hex = p.address[1..].iter().map(|b| format!("{b:02X}")).collect();
                let a = p.address[0];
                m.station_guess = format!("region {} station {} (a guess)", a >> 6, a & 63);
            }
        }
        let k = (p.packet_number, if p.descriptor == 0xB1 { 0 } else { 1 });
        if !p.continuation {
            m.last_number = Some(p.packet_number);
        }
        let store = |v: &mut Vec<((u8, u8), Vec<u8>)>| match v.iter_mut().find(|(kk, _)| *kk == k) {
            Some(e) => e.1 = raw.to_vec(),
            None => {
                v.push((k, raw.to_vec()));
                v.sort_by_key(|e| e.0);
            }
        };
        let was_complete = m.complete;
        if m.complete {
            store(&mut m.pass);
            if Self::complete_set(&m.pass, m.last_number) && m.pass.len() >= m.parts.len() {
                m.parts = std::mem::take(&mut m.pass);
                m.times_heard_complete += 1;
            }
        } else {
            store(&mut m.parts);
            m.complete = Self::complete_set(&m.parts, m.last_number);
            if m.complete {
                m.times_heard_complete += 1;
            }
        }
        let body: Vec<u8> = m
            .parts
            .iter()
            .flat_map(|(_, b)| b.iter().cloned())
            .collect();
        let text = text_of(m.presentation_code, &body);
        m.truncated = text.chars().count() > TEXT_CAP;
        m.text = if m.truncated {
            text.chars().take(TEXT_CAP).collect()
        } else {
            text
        };
        m.packets = m.parts.len();
        m.area = area(&m.text);
        m.warning_number = warning_number(&m.text);
        m.subject = subject(m.service_code, m.priority, &m.text).into();
        let newly = m.complete && !was_complete && !m.announced;
        if newly {
            m.announced = true;
        }
        newly.then_some(key)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pkt(num: u8, desc: u8, cont: bool, text: &str, msg: u16, svc: u8, prio: u8) -> EgcPacket {
        EgcPacket {
            descriptor: desc,
            service: svc,
            continuation: cont,
            priority: prio,
            repetition: 0,
            message_number: msg,
            packet_number: num,
            presentation: 0,
            address: vec![0xC4; service(svc).2],
            payload: text.as_bytes().to_vec(),
        }
    }

    #[test]
    fn assembly_rules() {
        let mut a = EgcAssembler::new(150);
        let at = Stamp::at;
        let k10 = MsgKey {
            number: 10,
            service: 0x31,
        };
        let k11 = MsgKey {
            number: 11,
            service: 0x34,
        };
        // Out of order.
        a.add(&pkt(2, 0xB2, true, "B", 10, 0x31, 1), at(1.0), 0, 1);
        a.add(&pkt(3, 0xB1, false, "C", 10, 0x31, 1), at(2.0), 0, 1);
        assert!(!a.messages.get(&k10).unwrap().complete);
        assert!(a
            .add(&pkt(1, 0xB1, true, "A", 10, 0x31, 1), at(3.0), 0, 1)
            .is_some());
        assert_eq!(a.messages.get(&k10).unwrap().text, "ABC");
        // A duplicate changes nothing.
        a.add(&pkt(2, 0xB2, true, "B", 10, 0x31, 1), at(4.0), 0, 1);
        assert_eq!(a.messages.get(&k10).unwrap().text, "ABC");
        // A new pass counts once complete.
        for (n, d, c, t) in [
            (1, 0xB1, true, "A"),
            (2, 0xB2, true, "B"),
            (3, 0xB1, false, "C"),
        ] {
            a.add(&pkt(n, d, c, t, 10, 0x31, 1), at(5.0), 0, 1);
        }
        assert_eq!(a.messages.get(&k10).unwrap().times_heard_complete, 2);
        // A reused number replaces the old message.
        a.add(&pkt(1, 0xB1, false, "NEW", 10, 0x31, 1), at(6.0), 0, 1);
        assert_eq!(a.messages.get(&k10).unwrap().text, "NEW");
        assert_eq!(a.seen, 1);
        // A gap stays incomplete until the repeat heals it.
        a.add(&pkt(1, 0xB1, true, "X", 11, 0x34, 3), at(7.0), 0, 1);
        a.add(&pkt(3, 0xB1, false, "Z", 11, 0x34, 3), at(7.0), 0, 1);
        assert!(!a.messages.get(&k11).unwrap().complete);
        a.add(&pkt(2, 0xB2, true, "Y", 11, 0x34, 3), at(367.0), 0, 1);
        assert!(a.messages.get(&k11).unwrap().complete);
        assert_eq!(a.messages.get(&k11).unwrap().subject, "Distress relay");
    }

    #[test]
    fn text_and_areas() {
        let codes = ita2_encode("FLEET 14 PORT: DUBAI, ETA 0630.");
        assert_eq!(ita2(&codes), "FLEET 14 PORT: DUBAI, ETA 0630.");
        assert_eq!(area_number("IV"), 4);
        assert_eq!(area_number("xxi"), 21);
        assert_eq!(area_number("9"), 9);
        for s in ["22", "XXII", "", "ABC"] {
            assert_eq!(area_number(s), 0);
        }
        let t = "NAVAREA IV 123/26 SOMETHING";
        assert_eq!(area(t), "NAVAREA IV");
        assert_eq!(warning_number(t), "123/26");
        assert_eq!(subject(0x31, 1, t), "Navigational warning");
        assert_eq!(subject(0x31, 0, "METAREA 9 FORECAST"), "Weather forecast");
        let l = "navarea xii 5/26 gale warning";
        assert_eq!(area(l), "NAVAREA XII");
        assert_eq!(warning_number(l), "5/26");
        assert_eq!(subject(0x31, 1, l), "Weather warning");
        assert_eq!(area("NAVAREAS I"), "");
        assert_eq!(area("NAVAREA 99"), "");
        assert_eq!(warning_number("NUMBER 12345/26"), "");
        assert_eq!(frame_time(1001), "2:24:08.6");
    }

    #[test]
    fn egc_ita2_figures_follow_itu_r_m625() {
        // ITU-R M.625 Table 1, figures case: V is "=", Z is "+", S is the apostrophe, J the bell, D "Who are you?".
        let figs = |code: u8| ita2(&[0x1B, code]);
        assert_eq!(figs(30), "=");
        assert_eq!(figs(17), "+");
        assert_eq!(figs(5), "'");
        assert_eq!(figs(11), "\u{7}");
        assert_eq!(figs(9), "\u{5}");
        // F, G and H are left to national use: nothing is shown for them.
        assert_eq!(figs(13), "");
        assert_eq!(figs(26), "");
        assert_eq!(figs(20), "");
    }
}
