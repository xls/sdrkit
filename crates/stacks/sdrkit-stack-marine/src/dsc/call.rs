//! DSC call parsing (ITU-R M.493-15 sections 4 to 8, M.541): the order of fields per format specifier, the code tables, the kind of
//! call, the one-line text, and builders for the test calls.

use super::decoder::RawCall;
use super::symbols::{self, mmsi_symbols};
use sdrkit_util_ids::mmsi;
use sdrkit_util_ids::names::IdNames;
use serde::{Deserialize, Serialize};

/// Format specifier names.
pub fn format_name(f: u8) -> &'static str {
    match f {
        102 => "Geographic area",
        112 => "Distress",
        114 => "Group",
        116 => "All ships",
        120 => "Individual",
        121 => "ATIS (inland)",
        123 => "Individual (automatic)",
        _ => "Unknown format",
    }
}

/// Category names.
pub fn category_name(c: u8) -> &'static str {
    match c {
        100 => "Routine",
        106 => "Ship's business",
        108 => "Safety",
        110 => "Urgency",
        112 => "Distress",
        _ => "",
    }
}

/// Nature of distress names.
pub fn nature_name(n: u8) -> &'static str {
    match n {
        100 => "Fire, explosion",
        101 => "Flooding",
        102 => "Collision",
        103 => "Grounding",
        104 => "Listing, capsizing",
        105 => "Sinking",
        106 => "Disabled and adrift",
        107 => "Undesignated",
        108 => "Abandoning ship",
        109 => "Piracy / armed robbery",
        110 => "Man overboard",
        112 => "EPIRB emission",
        _ => "",
    }
}

/// First telecommand names.
pub fn tc1_name(t: u8) -> &'static str {
    match t {
        100 => "F3E/G3E all modes telephony",
        101 => "F3E/G3E duplex telephony",
        103 => "Polling",
        104 => "Unable to comply",
        105 => "End of call",
        106 => "Data",
        109 => "J3E telephony",
        110 => "Distress acknowledgement",
        112 => "Distress relay",
        113 => "F1B/J2B FEC",
        115 => "F1B/J2B ARQ",
        118 => "Test",
        121 => "Position update",
        126 => "No information",
        _ => "",
    }
}

/// Second telecommand names.
pub fn tc2_name(t: u8) -> &'static str {
    match t {
        100 => "No reason",
        101 => "Congestion at maritime switching centre",
        102 => "Busy",
        103 => "Queue indication",
        104 => "Station barred",
        105 => "No operator available",
        106 => "Operator temporarily unavailable",
        107 => "Equipment disabled",
        108 => "Unable to use proposed channel",
        109 => "Unable to use proposed mode",
        110 => "Ships and aircraft of states not party to a conflict",
        111 => "Medical transports",
        112 => "Pay-phone / public call office",
        113 => "Facsimile / data",
        126 => "No information",
        _ => "",
    }
}

/// EOS names.
pub fn eos_name(e: u8) -> &'static str {
    match e {
        117 => "Acknowledgement requested",
        122 => "Acknowledgement",
        127 => "End of sequence",
        _ => "",
    }
}

/// A position.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Position {
    pub lat: f64,
    pub lon: f64,
}

impl Position {
    /// "50d51'N 001d18'W".
    pub fn text(&self) -> String {
        let la = self.lat.abs();
        let lo = self.lon.abs();
        let (ld, lm) = (la.floor(), ((la - la.floor()) * 60.0).round());
        let (od, om) = (lo.floor(), ((lo - lo.floor()) * 60.0).round());
        format!(
            "{:02}d{:02}'{} {:03}d{:02}'{}",
            ld as u32,
            lm as u32,
            if self.lat >= 0.0 { 'N' } else { 'S' },
            od as u32,
            om as u32,
            if self.lon >= 0.0 { 'E' } else { 'W' }
        )
    }
}

/// A position from 5 symbols (quadrant, latitude degrees and minutes, longitude degrees and minutes). All 9s: no position.
pub fn position(syms: &[Option<u8>]) -> Option<Position> {
    if syms.len() < 5 {
        return None;
    }
    let d = symbols::digits(&syms[..5]);
    if d.contains('~') || d == "9999999999" {
        return None;
    }
    let v: Vec<u32> = d.bytes().map(|c| u32::from(c - b'0')).collect();
    let q = v[0];
    let lat = (v[1] * 10 + v[2]) as f64 + (v[3] * 10 + v[4]) as f64 / 60.0;
    let lon = (v[5] * 100 + v[6] * 10 + v[7]) as f64 + (v[8] * 10 + v[9]) as f64 / 60.0;
    if q > 3 || lat > 90.0 || lon > 180.0 {
        return None;
    }
    Some(Position {
        lat: if q >= 2 { -lat } else { lat },
        lon: if q == 1 || q == 3 { -lon } else { lon },
    })
}

/// The 5 symbols of a position.
pub fn position_symbols(lat: f64, lon: f64) -> Vec<u8> {
    let q = match (lat >= 0.0, lon >= 0.0) {
        (true, true) => 0,
        (true, false) => 1,
        (false, true) => 2,
        (false, false) => 3,
    };
    let la = lat.abs();
    let lo = lon.abs();
    let (ld, mut lm) = (la.floor() as u32, ((la - la.floor()) * 60.0).round() as u32);
    let (od, mut om) = (lo.floor() as u32, ((lo - lo.floor()) * 60.0).round() as u32);
    lm = lm.min(59);
    om = om.min(59);
    symbols::number_symbols(&format!("{q}{ld:02}{lm:02}{od:03}{om:02}"))
}

/// Time from 2 symbols: hours and minutes. 88 88 is not available. Out of range is invalid.
pub fn time(syms: &[Option<u8>]) -> Option<(u8, u8)> {
    match syms {
        [Some(h), Some(m), ..] if *h <= 23 && *m <= 59 => Some((*h, *m)),
        _ => None,
    }
}

/// A frequency or channel field (3 symbols): the text shown.
pub fn frequency(syms: &[Option<u8>]) -> String {
    if syms.len() < 3 {
        return String::new();
    }
    if syms.iter().take(3).all(|s| *s == Some(126)) {
        return String::new();
    }
    let d = symbols::digits(&syms[..3]);
    if d.contains('~') {
        return format!("? ({d})");
    }
    let v: Vec<u32> = d.bytes().map(|c| u32::from(c - b'0')).collect();
    match v[0] {
        0..=2 => {
            let hz100: u32 = d.parse().unwrap_or(0);
            format!("{:.1} kHz", hz100 as f64 / 10.0)
        }
        3 => format!("MF/HF channel {}", d[1..].parse::<u32>().unwrap_or(0)),
        9 => {
            let variant = v[1];
            let ch: u32 = d[2..].parse().unwrap_or(0);
            match variant {
                1 => format!("VHF ch {variant}{ch:03} (ship simplex)"),
                2 => format!("VHF ch {variant}{ch:03} (coast simplex)"),
                _ => format!("VHF ch {ch}"),
            }
        }
        _ => format!("? ({d})"),
    }
}

/// Symbols of a frequency in kHz (100 Hz resolution).
pub fn frequency_symbols_khz(khz: f64) -> Vec<u8> {
    symbols::number_symbols(&format!("{:06}", (khz * 10.0).round() as u32))
}

/// Symbols of a VHF channel.
pub fn vhf_symbols(ch: u32) -> Vec<u8> {
    symbols::number_symbols(&format!("90{ch:04}"))
}

/// A parsed call.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct DscCall {
    pub format: u8,
    pub format_name: String,
    pub category: Option<u8>,
    pub category_name: String,
    /// Nine digits (`~` where unreadable). Empty when no digit is readable.
    pub from: String,
    pub to: String,
    pub to_type: String,
    pub distress_mmsi: String,
    pub nature: Option<u8>,
    pub nature_name: String,
    pub lat: Option<f64>,
    pub lon: Option<f64>,
    pub position_text: String,
    pub utc: String,
    pub telecommand1: Option<u8>,
    pub telecommand2: Option<u8>,
    pub telecommands: String,
    pub freq_rx: String,
    pub freq_tx: String,
    pub area: String,
    pub eos: Option<u8>,
    pub ecc_ok: bool,
    pub kind: String,
    pub distress: bool,
    pub from_country: String,
    pub from_kind: String,
    pub to_country: String,
    pub distress_country: String,
    pub from_name: String,
    pub to_name: String,
    pub repaired: usize,
    pub erasures: usize,
    pub recovered: usize,
    pub vhf: bool,
    pub text: String,
    pub symbols: Vec<Option<u8>>,
    pub inverted: bool,
}

fn mmsi_of(syms: &[Option<u8>]) -> String {
    if syms.len() < 5 {
        return String::new();
    }
    let d = symbols::digits(&syms[..5]);
    let nine: String = d.chars().take(9).collect();
    if nine.chars().all(|c| c == '~') {
        String::new()
    } else {
        nine
    }
}

/// Parses a merged call.
pub fn parse(raw: &RawCall, names: Option<&IdNames>) -> DscCall {
    let s = &raw.message;
    let fmt = raw.format.unwrap_or(0);
    let mut c = DscCall {
        format: fmt,
        format_name: format_name(fmt).into(),
        eos: raw.eos,
        ecc_ok: raw.check_ok,
        repaired: raw.repaired,
        erasures: raw.erasures,
        recovered: raw.recovered,
        symbols: s.clone(),
        inverted: raw.inverted,
        ..Default::default()
    };
    let at = |i: usize| s.get(i).copied().flatten();
    let slice = |a: usize, n: usize| -> Vec<Option<u8>> {
        (a..a + n).map(|i| s.get(i).copied().flatten()).collect()
    };
    let mut k;
    let distress_tail = |c: &mut DscCall, k: &mut usize, with_mmsi: bool| {
        if with_mmsi {
            c.distress_mmsi = mmsi_of(&slice(*k, 5));
            *k += 5;
        }
        c.nature = at(*k);
        *k += 1;
        if let Some(p) = position(&slice(*k, 5)) {
            c.lat = Some(p.lat);
            c.lon = Some(p.lon);
            c.position_text = p.text();
        }
        *k += 5;
        if let Some((h, m)) = time(&slice(*k, 2)) {
            c.utc = format!("{h:02}:{m:02}");
        }
        *k += 2;
        c.telecommand2 = at(*k);
        *k += 1;
    };
    match fmt {
        112 => {
            c.from = mmsi_of(&slice(0, 5));
            k = 5;
            distress_tail(&mut c, &mut k, false);
            c.to = "all ships".into();
        }
        116 | 114 | 120 | 123 | 102 => {
            k = 0;
            if fmt == 116 {
                c.to = "all ships".into();
            } else if fmt == 102 {
                let a = slice(0, 5);
                let d = symbols::digits(&a);
                if !d.contains('~') {
                    let v: Vec<u32> = d.bytes().map(|x| u32::from(x - b'0')).collect();
                    let q = v[0];
                    c.area = format!(
                        "area ref {}{:02} {}{:03}, {:02} deg x {:02} deg",
                        if q >= 2 { 'S' } else { 'N' },
                        v[1] * 10 + v[2],
                        if q == 1 || q == 3 { 'W' } else { 'E' },
                        v[3] * 100 + v[4] * 10 + v[5],
                        v[6] * 10 + v[7],
                        v[8] * 10 + v[9]
                    );
                }
                c.to = c.area.clone();
                k = 5;
            } else {
                c.to = mmsi_of(&slice(0, 5));
                k = 5;
            }
            c.category = at(k);
            k += 1;
            c.from = mmsi_of(&slice(k, 5));
            k += 5;
            c.telecommand1 = at(k);
            k += 1;
            let tc1 = c.telecommand1.unwrap_or(0);
            if c.category == Some(112) && (tc1 == 110 || tc1 == 112) {
                distress_tail(&mut c, &mut k, true);
            } else {
                c.telecommand2 = at(k);
                k += 1;
                let rest = s.len().saturating_sub(k);
                if at(k) == Some(55) {
                    if let Some(p) = position(&slice(k + 1, 5)) {
                        c.lat = Some(p.lat);
                        c.lon = Some(p.lon);
                        c.position_text = p.text();
                    }
                    if let Some((h, m)) = time(&slice(k + 6, 2)) {
                        c.utc = format!("{h:02}:{m:02}");
                    }
                } else {
                    if rest >= 3 {
                        c.freq_rx = frequency(&slice(k, 3));
                    }
                    if rest >= 6 {
                        if at(k + 3) == Some(55) {
                            if let Some(p) = position(&slice(k + 4, 5)) {
                                c.lat = Some(p.lat);
                                c.lon = Some(p.lon);
                                c.position_text = p.text();
                            }
                        } else {
                            c.freq_tx = frequency(&slice(k + 3, 3));
                        }
                    }
                }
            }
        }
        _ => {}
    }
    c.category_name = c
        .category
        .map_or(String::new(), |v| category_name(v).to_string());
    c.nature_name = c
        .nature
        .map_or(String::new(), |v| nature_name(v).to_string());
    c.distress = fmt == 112 || c.category == Some(112);
    // Telecommand text.
    let tc1 = c.telecommand1.map(tc1_name).unwrap_or("");
    let tc2 = c.telecommand2.unwrap_or(126);
    c.telecommands = if fmt == 112 || (c.category == Some(112) && c.distress_mmsi.len() == 9) {
        let sub = tc1_name(tc2);
        let mut t = tc1.to_string();
        if !sub.is_empty() {
            if !t.is_empty() {
                t.push_str(", ");
            }
            t.push_str(&format!("then {sub}"));
        }
        t
    } else if tc2 != 126 && !tc2_name(tc2).is_empty() {
        format!("{tc1}, {}", tc2_name(tc2))
    } else {
        tc1.to_string()
    };
    c.kind = kind(&c);
    // Countries, kinds and names.
    if !c.from.is_empty() {
        let i = mmsi::classify(&c.from);
        c.from_country = i.country.unwrap_or("").into();
        c.from_kind = i.kind.name().into();
        if let Some(n) = names.and_then(|t| station_name(t, &c.from)) {
            c.from_name = n;
        }
    }
    if c.to.len() == 9 && c.to.chars().all(|x| x.is_ascii_digit() || x == '~') {
        let i = mmsi::classify(&c.to);
        c.to_country = i.country.unwrap_or("").into();
        c.to_type = i.kind.name().into();
        if let Some(n) = names.and_then(|t| station_name(t, &c.to)) {
            c.to_name = n;
        }
    }
    if c.distress_mmsi.len() == 9 {
        c.distress_country = mmsi::classify(&c.distress_mmsi)
            .country
            .unwrap_or("")
            .into();
    }
    c.text = text(&c);
    c
}

/// The name of a station in the user's list: the name, or "name, location" for a coast station
/// whose row has a location.
pub fn station_name(names: &IdNames, mmsi: &str) -> Option<String> {
    let e = names.get(mmsi)?;
    if e.name.is_empty() {
        return None;
    }
    match e.field("location").filter(|l| !l.is_empty()) {
        Some(l) if mmsi::kind(mmsi) == mmsi::Kind::CoastStation => Some(format!("{}, {l}", e.name)),
        _ => Some(e.name.clone()),
    }
}

fn kind(c: &DscCall) -> String {
    let tc1 = c.telecommand1.unwrap_or(0);
    let who = match c.format {
        116 => "All ships",
        102 => "Area",
        114 => "Group",
        120 | 123 => "Individual",
        112 => return "Distress alert".into(),
        f => return format_name(f).into(),
    };
    if c.category == Some(112) && tc1 == 110 {
        return if c.format == 116 {
            "Distress alert acknowledgement".into()
        } else {
            "Distress alert relay acknowledgement".into()
        };
    }
    if c.category == Some(112) && tc1 == 112 {
        return match c.format {
            116 => "Distress alert relay (all ships)".into(),
            102 => "Distress alert relay (area)".into(),
            _ => "Distress alert relay".into(),
        };
    }
    let ack = c.eos == Some(122);
    match tc1 {
        118 => {
            return if ack {
                "Test acknowledgement".into()
            } else {
                "Test call".into()
            }
        }
        121 => return "Position update".into(),
        103 => {
            return if ack || c.lat.is_some() {
                "Position reply".into()
            } else {
                "Position request (polling)".into()
            }
        }
        _ => {}
    }
    if ack && c.lat.is_some() {
        return "Position reply".into();
    }
    if ack {
        return format!("{who} call acknowledgement");
    }
    match c.category {
        Some(110) => format!("{who} urgency call"),
        Some(108) => format!("{who} safety call"),
        _ => format!("{who} call"),
    }
}

fn text(c: &DscCall) -> String {
    let from = if c.from.is_empty() {
        "?".to_string()
    } else {
        c.from.clone()
    };
    let to = if c.to.is_empty() {
        "?".to_string()
    } else {
        c.to.clone()
    };
    let mut t = format!("{} from {from} to {to}", c.format_name);
    if !c.nature_name.is_empty() {
        t.push_str(&format!(" - {}", c.nature_name));
    }
    if !c.position_text.is_empty() {
        t.push_str(&format!(" at {}", c.position_text));
    }
    if !c.utc.is_empty() {
        t.push_str(&format!(" {} UTC", c.utc));
    }
    if !c.ecc_ok {
        t.push_str(" (check failed)");
    }
    t
}

/// A call to send: the format, message symbols and EOS, and the dot pattern length.
#[derive(Clone, Debug, PartialEq)]
pub struct Outgoing {
    pub format: u8,
    pub message: Vec<u8>,
    pub eos: u8,
    pub dots: usize,
}

impl Outgoing {
    /// The bits on the air.
    pub fn bits(&self) -> Vec<u8> {
        symbols::call_bits(self.dots, self.format, &self.message, self.eos)
    }
}

/// A distress alert.
pub fn distress(
    from: &str,
    nature: u8,
    lat: f64,
    lon: f64,
    hh: u8,
    mm: u8,
    subsequent: u8,
) -> Outgoing {
    let mut m = mmsi_symbols(from);
    m.push(nature);
    m.extend(position_symbols(lat, lon));
    m.extend([hh, mm, subsequent]);
    Outgoing {
        format: 112,
        message: m,
        eos: 127,
        dots: 200,
    }
}

/// An all-ships call (category, telecommands, RX and optional TX frequency symbols).
pub fn all_ships(from: &str, category: u8, tc1: u8, tc2: u8, rx: &[u8], tx: &[u8]) -> Outgoing {
    let mut m = vec![category];
    m.extend(mmsi_symbols(from));
    m.extend([tc1, tc2]);
    m.extend_from_slice(rx);
    m.extend_from_slice(tx);
    Outgoing {
        format: 116,
        message: m,
        eos: 127,
        dots: 200,
    }
}

/// An individual call.
#[allow(clippy::too_many_arguments)]
pub fn individual(
    to: &str,
    from: &str,
    category: u8,
    tc1: u8,
    tc2: u8,
    rx: &[u8],
    tx: &[u8],
    eos: u8,
) -> Outgoing {
    let mut m = mmsi_symbols(to);
    m.push(category);
    m.extend(mmsi_symbols(from));
    m.extend([tc1, tc2]);
    m.extend_from_slice(rx);
    m.extend_from_slice(tx);
    Outgoing {
        format: 120,
        message: m,
        eos,
        dots: 20,
    }
}

/// A distress acknowledgement (all ships).
#[allow(clippy::too_many_arguments)]
pub fn distress_ack(
    from: &str,
    distress_mmsi: &str,
    nature: u8,
    lat: f64,
    lon: f64,
    hh: u8,
    mm: u8,
    subsequent: u8,
) -> Outgoing {
    let mut m = vec![112];
    m.extend(mmsi_symbols(from));
    m.push(110);
    m.extend(mmsi_symbols(distress_mmsi));
    m.push(nature);
    m.extend(position_symbols(lat, lon));
    m.extend([hh, mm, subsequent]);
    Outgoing {
        format: 116,
        message: m,
        eos: 127,
        dots: 200,
    }
}

/// The four test calls: MF/HF (`vhf` false) or VHF channel 70 contents.
pub fn test_calls(vhf: bool) -> Vec<Outgoing> {
    let sub = if vhf { 100 } else { 109 };
    let freq = if vhf {
        vhf_symbols(6)
    } else {
        frequency_symbols_khz(8291.0)
    };
    let mut v = vec![
        distress("232123456", 103, 50.85, -1.3, 12, 34, sub),
        all_ships("235012345", 108, sub, 126, &freq, &[]),
        individual("002320064", "232123456", 100, sub, 126, &freq, &freq, 117),
        distress_ack("002320064", "232123456", 103, 50.85, -1.3, 12, 34, sub),
    ];
    if vhf {
        for c in &mut v {
            c.dots = 20;
        }
    }
    v
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dsc::decoder::DscDecoder;

    fn decode(o: &Outgoing) -> DscCall {
        let mut b = o.bits();
        b.extend((0..30).map(|i| (i % 2) as u8));
        let soft: Vec<f32> = b.iter().map(|&x| if x == 1 { 1.0 } else { -1.0 }).collect();
        let mut d = DscDecoder::new(false);
        d.push(&soft);
        let raw = d.take_calls().remove(0);
        parse(&raw, None)
    }

    #[test]
    fn field_facts() {
        assert_eq!(position_symbols(50.85, -1.3), vec![15, 5, 10, 1, 18]);
        let p = position(&[Some(15), Some(5), Some(10), Some(1), Some(18)]).unwrap();
        assert_eq!(p.text(), "50d51'N 001d18'W");
        assert!(position(&[Some(99); 5]).is_none());
        assert!(
            position(&[Some(35), Some(5), Some(10), Some(1), Some(18)])
                .unwrap()
                .lat
                < 0.0
        );
        assert_eq!(time(&[Some(88), Some(88)]), None);
        assert_eq!(time(&[Some(24), Some(0)]), None);
        assert_eq!(frequency_symbols_khz(8414.5), vec![8, 41, 45]);
        assert_eq!(frequency(&[Some(8), Some(29), Some(10)]), "8291.0 kHz");
        assert_eq!(
            frequency(&[Some(31), Some(23), Some(4)]),
            "MF/HF channel 12304"
        );
        assert_eq!(frequency(&[Some(90), Some(0), Some(16)]), "VHF ch 16");
        assert_eq!(
            frequency(&[Some(91), Some(0), Some(16)]),
            "VHF ch 1016 (ship simplex)"
        );
        assert_eq!(frequency(&[Some(126); 3]), "");
        assert_eq!(symbols::digits(&[Some(23), None, Some(5)]), "23~~05");
    }

    #[test]
    fn the_test_calls_parse() {
        let calls = test_calls(false);
        let c = decode(&calls[0]);
        assert_eq!(
            (
                c.format,
                c.from.as_str(),
                c.nature,
                c.utc.as_str(),
                c.telecommand2
            ),
            (112, "232123456", Some(103), "12:34", Some(109))
        );
        assert_eq!(c.kind, "Distress alert");
        assert_eq!(c.position_text, "50d51'N 001d18'W");
        assert_eq!(
            (c.from_country.as_str(), c.from_kind.as_str()),
            ("United Kingdom", "ship")
        );
        let c = decode(&calls[1]);
        assert_eq!(
            (
                c.format,
                c.category,
                c.freq_rx.as_str(),
                c.freq_tx.as_str(),
                c.telecommand1
            ),
            (116, Some(108), "8291.0 kHz", "", Some(109))
        );
        assert_eq!(c.kind, "All ships safety call");
        let c = decode(&calls[2]);
        assert_eq!(
            (c.format, c.to.as_str(), c.from.as_str(), c.category, c.eos),
            (120, "002320064", "232123456", Some(100), Some(117))
        );
        assert_eq!(
            (c.kind.as_str(), c.to_type.as_str(), c.to_country.as_str()),
            ("Individual call", "coast station", "United Kingdom")
        );
        let c = decode(&calls[3]);
        assert_eq!(
            (
                c.format,
                c.category,
                c.from.as_str(),
                c.distress_mmsi.as_str(),
                c.telecommand1
            ),
            (116, Some(112), "002320064", "232123456", Some(110))
        );
        assert_eq!(c.kind, "Distress alert acknowledgement");
        // Test calls.
        let t = individual("002320064", "232123456", 100, 118, 126, &[], &[], 117);
        assert_eq!(decode(&t).kind, "Test call");
        let t = individual("002320064", "232123456", 100, 118, 126, &[], &[], 122);
        assert_eq!(decode(&t).kind, "Test acknowledgement");
    }
}
