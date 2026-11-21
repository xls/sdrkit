//! The SAME message format and its checks: the event, originator, state and province tables, the county table, the issue time
//! and the trimming of received bursts.
//! Standard: 47 CFR 11.31 (c) to (e)

use crate::calendar as timeutil;

sdrkit_core::choice_enum! {
    /// What a message's event is.
    #[derive(serde::Serialize, serde::Deserialize)]
    #[serde(rename_all = "lowercase")]
    pub enum EventKind {
        /// A warning.
        Warning = ("warning", "Warning"),
        /// A watch.
        Watch = ("watch", "Watch"),
        /// An emergency.
        Emergency = ("emergency", "Emergency"),
        /// A statement.
        Statement = ("statement", "Statement"),
        /// A test.
        Test = ("test", "Test"),
        /// A message.
        Message = ("message", "Message"),
    }
}

sdrkit_core::choice_enum! {
    /// Why a message is damaged.
    #[derive(serde::Serialize, serde::Deserialize)]
    #[serde(rename_all = "snake_case")]
    pub enum Damage {
        /// Does not start with ZCZC-.
        NoStart = ("no_start", "Does not start with ZCZC-"),
        /// Bad originator.
        Originator = ("originator", "Bad originator"),
        /// Missing dash after the originator.
        OriginatorDash = ("originator_dash", "Missing dash after the originator"),
        /// Bad event code.
        EventCode = ("event_code", "Bad event code"),
        /// Missing dash after the event code.
        EventDash = ("event_dash", "Missing dash after the event code"),
        /// Bad location code.
        LocationCode = ("location_code", "Bad location code"),
        /// Message ends in the location list.
        EndsInLocations = ("ends_in_locations", "Message ends in the location list"),
        /// More than 31 locations.
        TooManyLocations = ("too_many_locations", "More than 31 locations"),
        /// Bad separator in the location list.
        LocationSeparator = ("location_separator", "Bad separator in the location list"),
        /// Bad purge time.
        PurgeTime = ("purge_time", "Bad purge time"),
        /// Purge time minutes over 59.
        PurgeMinutes = ("purge_minutes", "Purge time minutes over 59"),
        /// Purge time zero.
        PurgeZero = ("purge_zero", "Purge time zero"),
        /// Missing dash after the purge time.
        PurgeDash = ("purge_dash", "Missing dash after the purge time"),
        /// Bad issue time.
        IssueTime = ("issue_time", "Bad issue time"),
        /// Issue day out of range.
        IssueDay = ("issue_day", "Issue day out of range"),
        /// Issue time out of range.
        IssueRange = ("issue_range", "Issue time out of range"),
        /// Missing dash after the issue time.
        IssueDash = ("issue_dash", "Missing dash after the issue time"),
        /// Missing dash after the call sign.
        CallsignDash = ("callsign_dash", "Missing dash after the call sign"),
        /// Call sign length.
        CallsignLength = ("callsign_length", "Call sign length"),
        /// Bad call sign.
        Callsign = ("callsign", "Bad call sign"),
        /// Text after the message.
        Trailing = ("trailing", "Text after the message"),
    }
}

use EventKind::*;

/// The event table of 47 CFR 11.31 (e).
pub const EVENTS: &[(&str, &str, EventKind)] = &[
    ("EAN", "Emergency Action Notification (national)", Emergency),
    ("NIC", "National Information Center", Message),
    ("NPT", "National Periodic Test", Test),
    ("RMT", "Required Monthly Test", Test),
    ("RWT", "Required Weekly Test", Test),
    ("ADR", "Administrative Message", Message),
    ("AVA", "Avalanche Watch", Watch),
    ("AVW", "Avalanche Warning", Warning),
    ("BLU", "Blue Alert", Warning),
    ("BZW", "Blizzard Warning", Warning),
    ("CAE", "Child Abduction Emergency", Emergency),
    ("CDW", "Civil Danger Warning", Warning),
    ("CEM", "Civil Emergency Message", Emergency),
    ("CFA", "Coastal Flood Watch", Watch),
    ("CFW", "Coastal Flood Warning", Warning),
    ("DMO", "Practice / Demo Warning", Test),
    ("DSW", "Dust Storm Warning", Warning),
    ("EQW", "Earthquake Warning", Warning),
    ("EVI", "Evacuation Immediate", Emergency),
    ("EWW", "Extreme Wind Warning", Warning),
    ("FFA", "Flash Flood Watch", Watch),
    ("FFS", "Flash Flood Statement", Statement),
    ("FFW", "Flash Flood Warning", Warning),
    ("FLA", "Flood Watch", Watch),
    ("FLS", "Flood Statement", Statement),
    ("FLW", "Flood Warning", Warning),
    ("FRW", "Fire Warning", Warning),
    ("FSW", "Flash Freeze Warning", Warning),
    ("FZW", "Freeze Warning", Warning),
    ("HLS", "Hurricane Statement", Statement),
    ("HMW", "Hazardous Materials Warning", Warning),
    ("HUA", "Hurricane Watch", Watch),
    ("HUW", "Hurricane Warning", Warning),
    ("HWA", "High Wind Watch", Watch),
    ("HWW", "High Wind Warning", Warning),
    ("LAE", "Local Area Emergency", Emergency),
    ("LEW", "Law Enforcement Warning", Warning),
    ("LSW", "Land Slide Warning", Warning),
    ("NMN", "Network Message Notification", Message),
    ("NUW", "Nuclear Power Plant Warning", Warning),
    ("POS", "Power Outage Statement", Statement),
    ("RHW", "Radiological Hazard Warning", Warning),
    ("SMW", "Special Marine Warning", Warning),
    ("SPS", "Special Weather Statement", Statement),
    ("SPW", "Shelter in Place Warning", Warning),
    ("SSA", "Storm Surge Watch", Watch),
    ("SSW", "Storm Surge Warning", Warning),
    ("SVA", "Severe Thunderstorm Watch", Watch),
    ("SVR", "Severe Thunderstorm Warning", Warning),
    ("SVS", "Severe Weather Statement", Statement),
    ("TOA", "Tornado Watch", Watch),
    ("TOE", "911 Telephone Outage Emergency", Emergency),
    ("TOR", "Tornado Warning", Warning),
    ("TRA", "Tropical Storm Watch", Watch),
    ("TRW", "Tropical Storm Warning", Warning),
    ("TSA", "Tsunami Watch", Watch),
    ("TSW", "Tsunami Warning", Warning),
    ("TXB", "Transmitter backup on", Message),
    ("TXF", "Transmitter carrier off", Message),
    ("TXO", "Transmitter carrier on", Message),
    ("TXP", "Transmitter primary on", Message),
    ("VOW", "Volcano Warning", Warning),
    ("WSA", "Winter Storm Watch", Watch),
    ("WSW", "Winter Storm Warning", Warning),
];

/// The known originators.
pub const ORIGINATORS: &[(&str, &str)] = &[
    ("PEP", "Primary Entry Point System"),
    ("CIV", "Civil authorities"),
    ("WXR", "National Weather Service"),
    ("EAS", "EAS participant"),
];

/// US states, DC and territories (FIPS PUB 5-2): code, abbreviation, name, rounded centre.
pub const US_STATES: &[(u8, &str, &str, f64, f64)] = &[
    (1, "AL", "Alabama", 32.8, -86.8),
    (2, "AK", "Alaska", 64.0, -152.0),
    (4, "AZ", "Arizona", 34.3, -111.7),
    (5, "AR", "Arkansas", 34.9, -92.4),
    (6, "CA", "California", 37.2, -119.5),
    (8, "CO", "Colorado", 39.0, -105.5),
    (9, "CT", "Connecticut", 41.6, -72.7),
    (10, "DE", "Delaware", 39.0, -75.5),
    (11, "DC", "District of Columbia", 38.9, -77.0),
    (12, "FL", "Florida", 28.6, -82.4),
    (13, "GA", "Georgia", 32.7, -83.4),
    (15, "HI", "Hawaii", 20.3, -156.4),
    (16, "ID", "Idaho", 44.4, -114.6),
    (17, "IL", "Illinois", 40.0, -89.2),
    (18, "IN", "Indiana", 39.9, -86.3),
    (19, "IA", "Iowa", 42.1, -93.5),
    (20, "KS", "Kansas", 38.5, -98.4),
    (21, "KY", "Kentucky", 37.5, -85.3),
    (22, "LA", "Louisiana", 31.1, -92.0),
    (23, "ME", "Maine", 45.4, -69.2),
    (24, "MD", "Maryland", 39.0, -76.8),
    (25, "MA", "Massachusetts", 42.3, -71.8),
    (26, "MI", "Michigan", 44.3, -85.4),
    (27, "MN", "Minnesota", 46.3, -94.3),
    (28, "MS", "Mississippi", 32.7, -89.7),
    (29, "MO", "Missouri", 38.4, -92.5),
    (30, "MT", "Montana", 47.0, -109.6),
    (31, "NE", "Nebraska", 41.5, -99.8),
    (32, "NV", "Nevada", 39.3, -116.6),
    (33, "NH", "New Hampshire", 43.7, -71.6),
    (34, "NJ", "New Jersey", 40.2, -74.7),
    (35, "NM", "New Mexico", 34.4, -106.1),
    (36, "NY", "New York", 42.9, -75.5),
    (37, "NC", "North Carolina", 35.6, -79.4),
    (38, "ND", "North Dakota", 47.5, -100.5),
    (39, "OH", "Ohio", 40.3, -82.8),
    (40, "OK", "Oklahoma", 35.6, -97.5),
    (41, "OR", "Oregon", 43.9, -120.6),
    (42, "PA", "Pennsylvania", 40.9, -77.8),
    (44, "RI", "Rhode Island", 41.7, -71.5),
    (45, "SC", "South Carolina", 33.9, -80.9),
    (46, "SD", "South Dakota", 44.4, -100.2),
    (47, "TN", "Tennessee", 35.9, -86.4),
    (48, "TX", "Texas", 31.5, -99.3),
    (49, "UT", "Utah", 39.3, -111.7),
    (50, "VT", "Vermont", 44.1, -72.7),
    (51, "VA", "Virginia", 37.5, -78.8),
    (53, "WA", "Washington", 47.4, -120.5),
    (54, "WV", "West Virginia", 38.6, -80.6),
    (55, "WI", "Wisconsin", 44.6, -89.9),
    (56, "WY", "Wyoming", 43.0, -107.6),
    (60, "AS", "American Samoa", -14.3, -170.7),
    (66, "GU", "Guam", 13.4, 144.8),
    (69, "MP", "Northern Mariana Islands", 15.2, 145.7),
    (72, "PR", "Puerto Rico", 18.2, -66.5),
    (78, "VI", "Virgin Islands", 18.3, -64.9),
];

/// NWS marine areas in the state field (named, no centre).
pub const MARINE: &[(u8, &str)] = &[
    (57, "Eastern North Pacific (marine)"),
    (58, "North Pacific near Alaska (marine)"),
    (59, "Central Pacific near Hawaii (marine)"),
    (61, "South Central Pacific (marine)"),
    (65, "Western Pacific (marine)"),
    (73, "Western North Atlantic (marine)"),
    (75, "Western North Atlantic and Caribbean (marine)"),
    (77, "Gulf of Mexico (marine)"),
    (91, "Lake Superior (marine)"),
    (92, "Lake Michigan (marine)"),
    (93, "Lake Huron (marine)"),
    (94, "Lake St. Clair (marine)"),
    (96, "Lake Erie (marine)"),
    (97, "Lake Ontario (marine)"),
    (98, "St. Lawrence River (marine)"),
];

/// Canadian provinces and territories (Statistics Canada SGC): code, abbreviation, name, centre.
pub const CANADA: &[(u8, &str, &str, f64, f64)] = &[
    (10, "NL", "Newfoundland and Labrador", 53.1, -57.7),
    (11, "PE", "Prince Edward Island", 46.5, -63.4),
    (12, "NS", "Nova Scotia", 45.0, -63.0),
    (13, "NB", "New Brunswick", 46.6, -66.5),
    (24, "QC", "Quebec", 52.9, -73.5),
    (35, "ON", "Ontario", 50.0, -85.0),
    (46, "MB", "Manitoba", 54.0, -97.0),
    (47, "SK", "Saskatchewan", 54.0, -106.0),
    (48, "AB", "Alberta", 54.5, -115.0),
    (59, "BC", "British Columbia", 54.0, -125.0),
    (60, "YT", "Yukon", 63.0, -135.0),
    (61, "NT", "Northwest Territories", 64.8, -124.8),
    (62, "NU", "Nunavut", 70.3, -83.1),
];

/// The PCA (Partial County Alerting) names, 0–9.
pub const PCA: [&str; 10] = [
    "whole",
    "northwest",
    "north",
    "northeast",
    "west",
    "central",
    "east",
    "southwest",
    "south",
    "southeast",
];

sdrkit_core::choice_enum! {
    /// Whether the region names come from the US or the Canadian table.
    pub enum Country {
        /// FIPS states.
        Us = ("us", "United States"),
        /// Canadian provinces.
        Canada = ("canada", "Canada"),
    }
}

/// A state or province: name and centre (none for a marine area).
pub fn state(code: u8, country: Country) -> Option<(&'static str, Option<(f64, f64)>)> {
    match country {
        Country::Us => US_STATES
            .iter()
            .find(|s| s.0 == code)
            .map(|s| (s.2, Some((s.3, s.4))))
            .or_else(|| MARINE.iter().find(|m| m.0 == code).map(|m| (m.1, None))),
        Country::Canada => CANADA
            .iter()
            .find(|s| s.0 == code)
            .map(|s| (s.2, Some((s.3, s.4)))),
    }
}

/// An event: name and kind.
pub fn event(code: &str) -> Option<(&'static str, EventKind)> {
    EVENTS.iter().find(|e| e.0 == code).map(|e| (e.1, e.2))
}

/// An originator's name.
pub fn originator(code: &str) -> Option<&'static str> {
    ORIGINATORS.iter().find(|o| o.0 == code).map(|o| o.1)
}

/// A character the format allows (ASCII 0x20–0x7E).
pub fn valid_char(c: u8) -> bool {
    (0x20..=0x7E).contains(&c)
}

/// Why a county table was not loaded.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CountyError {
    /// The text has no lines.
    Empty,
    /// No line names a county. `refused` lines were not read.
    NoCounty {
        /// The lines that were not read.
        refused: usize,
    },
}

impl std::fmt::Display for CountyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CountyError::Empty => f.write_str("the county table is empty"),
            CountyError::NoCounty { refused } => {
                write!(f, "no line names a county ({refused} lines not read)")
            }
        }
    }
}

impl std::error::Error for CountyError {}

/// A loaded county table.
#[derive(Clone, Debug, Default)]
pub struct Counties {
    /// (state, county) → name.
    pub names: std::collections::BTreeMap<(u8, u16), String>,
}

impl Counties {
    /// Loads a CSV ("SSCCC,Name", "SS,CCC,Name" or the Census national_county.txt form).
    ///
    /// # Errors
    ///
    /// [`CountyError`] if no line names a county.
    pub fn load(text: &str) -> Result<Counties, CountyError> {
        let mut names = std::collections::BTreeMap::new();
        let mut bad = 0;
        for line in text.lines() {
            let l = line.trim();
            if l.is_empty() {
                continue;
            }
            let f = sdrkit_util_text::csv::split_line(l, ',');
            let parsed = match f.len() {
                2 if f[0].len() == 5 && f[0].bytes().all(|b| b.is_ascii_digit()) => Some((
                    f[0][..2].parse().ok(),
                    f[0][2..].parse().ok(),
                    f[1].to_string(),
                )),
                3 if f[0].bytes().all(|b| b.is_ascii_digit())
                    && f[1].bytes().all(|b| b.is_ascii_digit()) =>
                {
                    Some((f[0].parse().ok(), f[1].parse().ok(), f[2].to_string()))
                }
                n if n >= 4
                    && f[1].bytes().all(|b| b.is_ascii_digit())
                    && f[2].bytes().all(|b| b.is_ascii_digit())
                    && !f[1].is_empty() =>
                {
                    Some((f[1].parse().ok(), f[2].parse().ok(), f[3].to_string()))
                }
                _ => None,
            };
            match parsed {
                Some((Some(s), Some(c), name)) if !name.is_empty() => {
                    names.insert((s, c), name);
                }
                _ => bad += 1,
            }
        }
        if names.is_empty() {
            return Err(if bad > 0 {
                CountyError::NoCounty { refused: bad }
            } else {
                CountyError::Empty
            });
        }
        Ok(Counties { names })
    }
}

/// One location of a message.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Location {
    /// The six digits.
    pub code: String,
    /// PCA digit.
    pub pca: u8,
    /// PCA name ("whole", "northeast" …).
    pub pca_name: String,
    /// State or province code.
    pub state: u8,
    /// Its name, when known.
    pub state_name: Option<String>,
    /// County code.
    pub county: u16,
    /// The county's name from the loaded table.
    pub county_name: Option<String>,
    /// The text shown.
    pub text: String,
    /// The marker position (the state centre moved toward the PCA direction).
    pub marker: Option<(f64, f64)>,
}

/// A parsed message.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Message {
    /// The text as received (trimmed).
    pub raw: String,
    /// It passed every check.
    pub valid: bool,
    /// End of message ("NNNN").
    pub eom: bool,
    /// Why it is not valid.
    pub error: Option<Damage>,
    /// Originator code and name.
    pub originator: String,
    /// Originator name.
    pub originator_name: Option<String>,
    /// Event code.
    pub event: String,
    /// Event name.
    pub event_name: Option<String>,
    /// Event kind.
    pub event_kind: Option<EventKind>,
    /// Locations.
    pub locations: Vec<Location>,
    /// Purge time, minutes.
    pub purge_min: u32,
    /// "1 h 30 min".
    pub purge_text: String,
    /// Issue day of year, hour, minute (UTC).
    pub issue_day: u32,
    /// Hour.
    pub issue_hour: u32,
    /// Minute.
    pub issue_min: u32,
    /// "May 10 15:30 UTC".
    pub issue_text: String,
    /// Call sign ("KHGX/NWS").
    pub callsign: String,
}

impl Message {
    fn empty(raw: &str) -> Message {
        Message {
            raw: raw.into(),
            valid: false,
            eom: false,
            error: None,
            originator: String::new(),
            originator_name: None,
            event: String::new(),
            event_name: None,
            event_kind: None,
            locations: Vec::new(),
            purge_min: 0,
            purge_text: String::new(),
            issue_day: 0,
            issue_hour: 0,
            issue_min: 0,
            issue_text: String::new(),
            callsign: String::new(),
        }
    }
}

/// "1 h 30 min", "45 min", "2 h".
pub fn purge_text(min: u32) -> String {
    match (min / 60, min % 60) {
        (0, m) => format!("{m} min"),
        (h, 0) => format!("{h} h"),
        (h, m) => format!("{h} h {m} min"),
    }
}

/// "May 10 15:30 UTC" for a day of year of `year`; empty when the day does not exist in that year.
pub fn issue_text(year: i64, yday: u32, h: u32, m: u32) -> String {
    const MONTHS: [&str; 12] = [
        "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
    ];
    match timeutil::from_yday(year, yday) {
        Some((mo, d)) => format!("{} {} {:02}:{:02} UTC", MONTHS[mo as usize - 1], d, h, m),
        None => String::new(),
    }
}

/// The Unix time of an issue time in a year (None when the day does not exist).
pub fn issue_epoch(year: i64, yday: u32, h: u32, m: u32) -> Option<i64> {
    let (mo, d) = timeutil::from_yday(year, yday)?;
    timeutil::unix(year, mo, d, h, m, 0)
}

fn marker(centre: (f64, f64), pca: u8) -> (f64, f64) {
    let (dlat, dlon) = match pca {
        1 => (1.0, -1.0),
        2 => (1.0, 0.0),
        3 => (1.0, 1.0),
        4 => (0.0, -1.0),
        6 => (0.0, 1.0),
        7 => (-1.0, -1.0),
        8 => (-1.0, 0.0),
        9 => (-1.0, 1.0),
        _ => (0.0, 0.0),
    };
    (centre.0 + dlat, centre.1 + dlon)
}

fn location(code: &str, country: Country, counties: &Counties) -> Location {
    let b = code.as_bytes();
    let pca = b[0] - b'0';
    let st: u8 = code[1..3].parse().unwrap_or(0);
    let cc: u16 = code[3..].parse().unwrap_or(0);
    let s = state(st, country);
    let state_name = s.map(|x| x.0.to_string());
    let county_name = counties.names.get(&(st, cc)).cloned();
    let sname = state_name
        .clone()
        .unwrap_or_else(|| format!("state {st:02}"));
    let text = if cc == 0 {
        if pca == 0 {
            format!("All of {sname}")
        } else {
            format!("{} part of {sname}", capital(PCA[pca as usize]))
        }
    } else {
        match (&county_name, pca) {
            (Some(n), 0) => n.clone(),
            (Some(n), p) => format!("{} part of {n}", capital(PCA[p as usize])),
            (None, 0) => format!("County FIPS {cc}, {sname}"),
            (None, p) => format!(
                "{} part of county FIPS {cc}, {sname}",
                capital(PCA[p as usize])
            ),
        }
    };
    Location {
        code: code.into(),
        pca,
        pca_name: PCA[pca as usize].into(),
        state: st,
        state_name,
        county: cc,
        county_name,
        text,
        marker: s.and_then(|x| x.1).map(|c| marker(c, pca)),
    }
}

fn capital(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
        None => String::new(),
    }
}

/// Parses and checks a message. `year` is the host clock's year (for the issue-time text).
pub fn parse(text: &str, year: i64, country: Country, counties: &Counties) -> Message {
    let mut m = Message::empty(text);
    if text == "NNNN" {
        m.valid = true;
        m.eom = true;
        return m;
    }
    let fail = |mut m: Message, why: Damage| {
        m.error = Some(why);
        m.valid = false;
        m
    };
    let b = text.as_bytes();
    if !text.starts_with("ZCZC-") {
        return fail(m, Damage::NoStart);
    }
    let mut p = 5;
    let take = |p: usize, n: usize| text.get(p..p + n);
    let org = match take(p, 3) {
        Some(o) if o.bytes().all(|c| c.is_ascii_uppercase()) => o,
        _ => return fail(m, Damage::Originator),
    };
    m.originator = org.into();
    m.originator_name = originator(org).map(String::from);
    p += 3;
    if b.get(p) != Some(&b'-') {
        return fail(m, Damage::OriginatorDash);
    }
    p += 1;
    let ev = match take(p, 3) {
        Some(e)
            if e.bytes()
                .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit()) =>
        {
            e
        }
        _ => return fail(m, Damage::EventCode),
    };
    m.event = ev.into();
    if let Some((n, k)) = event(ev) {
        m.event_name = Some(n.into());
        m.event_kind = Some(k);
    }
    p += 3;
    if b.get(p) != Some(&b'-') {
        return fail(m, Damage::EventDash);
    }
    p += 1;
    loop {
        let loc = match take(p, 6) {
            Some(l) if l.bytes().all(|c| c.is_ascii_digit()) => l,
            Some(_) => return fail(m, Damage::LocationCode),
            None => return fail(m, Damage::EndsInLocations),
        };
        if m.locations.len() >= 31 {
            return fail(m, Damage::TooManyLocations);
        }
        m.locations.push(location(loc, country, counties));
        p += 6;
        match b.get(p) {
            Some(b'-') => p += 1,
            Some(b'+') => {
                p += 1;
                break;
            }
            None => return fail(m, Damage::EndsInLocations),
            _ => return fail(m, Damage::LocationSeparator),
        }
    }
    let tt = match take(p, 4) {
        Some(t) if t.bytes().all(|c| c.is_ascii_digit()) => t,
        _ => return fail(m, Damage::PurgeTime),
    };
    let (ph, pm): (u32, u32) = (
        tt[..2].parse().expect("checked as ASCII digits"),
        tt[2..].parse().expect("checked as ASCII digits"),
    );
    if pm > 59 {
        return fail(m, Damage::PurgeMinutes);
    }
    if ph == 0 && pm == 0 {
        return fail(m, Damage::PurgeZero);
    }
    m.purge_min = ph * 60 + pm;
    m.purge_text = purge_text(m.purge_min);
    p += 4;
    if b.get(p) != Some(&b'-') {
        return fail(m, Damage::PurgeDash);
    }
    p += 1;
    let it = match take(p, 7) {
        Some(t) if t.bytes().all(|c| c.is_ascii_digit()) => t,
        _ => return fail(m, Damage::IssueTime),
    };
    let (d, h, mi): (u32, u32, u32) = (
        it[..3].parse().expect("checked as ASCII digits"),
        it[3..5].parse().expect("checked as ASCII digits"),
        it[5..].parse().expect("checked as ASCII digits"),
    );
    if !(1..=366).contains(&d) {
        return fail(m, Damage::IssueDay);
    }
    if h > 23 || mi > 59 {
        return fail(m, Damage::IssueRange);
    }
    m.issue_day = d;
    m.issue_hour = h;
    m.issue_min = mi;
    m.issue_text = issue_text(year, d, h, mi);
    p += 7;
    if b.get(p) != Some(&b'-') {
        return fail(m, Damage::IssueDash);
    }
    p += 1;
    let rest = &text[p..];
    let Some(end) = rest.find('-') else {
        return fail(m, Damage::CallsignDash);
    };
    let cs = &rest[..end];
    if cs.len() != 8 {
        return fail(m, Damage::CallsignLength);
    }
    if !cs
        .bytes()
        .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == b'/' || c == b' ')
    {
        return fail(m, Damage::Callsign);
    }
    m.callsign = cs.into();
    if end + 1 != rest.len() {
        return fail(m, Damage::Trailing);
    }
    m.valid = true;
    m
}

/// Trims a received burst to the message it holds: from "ZCZC" or "NNNN" to the end of the format (the dash after the call sign);
/// empty when there is none.
pub fn trim(raw: &[u8]) -> String {
    let s: String = raw
        .iter()
        .map(|&c| if valid_char(c) { c as char } else { '\u{1}' })
        .collect();
    if let Some(i) = s.find("ZCZC") {
        let t = &s[i..];
        // The end: the purge time '+' then "TTTT-JJJHHMM-LLLLLLLL-".
        if let Some(plus) = t.find('+') {
            let end = plus + 1 + 4 + 1 + 7 + 1 + 8 + 1;
            if t.len() >= end {
                return t[..end].to_string();
            }
        }
        let e = t.find('\u{1}').unwrap_or(t.len());
        return t[..e].to_string();
    }
    if s.contains("NNNN") {
        return "NNNN".into();
    }
    String::new()
}

#[cfg(test)]
mod tests {
    use super::*;

    const TOR: &str = "ZCZC-WXR-TOR-048201-348113-029047+0030-1301530-KHGX/NWS-";

    #[test]
    fn tornado_warning() {
        let m = parse(TOR, 2026, Country::Us, &Counties::default());
        assert!(m.valid, "{:?}", m.error);
        assert_eq!(
            m.originator_name.as_deref(),
            Some("National Weather Service")
        );
        assert_eq!(m.event_name.as_deref(), Some("Tornado Warning"));
        assert_eq!(m.event_kind, Some(EventKind::Warning));
        assert_eq!(m.locations.len(), 3);
        assert_eq!(m.locations[0].text, "County FIPS 201, Texas");
        assert_eq!(
            m.locations[1].text,
            "Northeast part of county FIPS 113, Texas"
        );
        assert_eq!(m.locations[2].state_name.as_deref(), Some("Missouri"));
        let (lat, lon) = state(48, Country::Us).unwrap().1.unwrap();
        assert!((25.0..37.0).contains(&lat) && (-107.0..-93.0).contains(&lon));
        assert_eq!(m.purge_min, 30);
        assert_eq!((m.issue_day, m.issue_hour, m.issue_min), (130, 15, 30));
        assert_eq!(m.callsign, "KHGX/NWS");
    }

    #[test]
    fn variants() {
        let c = Counties::default();
        for (p, n) in PCA.iter().enumerate() {
            assert!(!n.is_empty() && p < 10);
        }
        let m = parse(
            "ZCZC-WXR-RWT-048000+0100-1301545-KEAX/NWS-",
            2026,
            Country::Us,
            &c,
        );
        assert_eq!(m.purge_min, 60);
        assert_eq!(m.locations[0].text, "All of Texas");
        let locs31: Vec<String> = (0..31).map(|i| format!("0480{i:02}")).collect();
        let m31 = format!("ZCZC-WXR-RWT-{}+0015-1301545-KEAX/NWS-", locs31.join("-"));
        assert!(parse(&m31, 2026, Country::Us, &c).valid);
        let m32 = format!(
            "ZCZC-WXR-RWT-{}-048099+0015-1301545-KEAX/NWS-",
            locs31.join("-")
        );
        assert_eq!(
            parse(&m32, 2026, Country::Us, &c).error,
            Some(Damage::TooManyLocations)
        );
        for (bad, why) in [
            (
                "ZCZX-WXR-RWT-029047+0015-1301545-KEAX/NWS-",
                Damage::NoStart,
            ),
            (
                "ZCZC-wXR-RWT-029047+0015-1301545-KEAX/NWS-",
                Damage::Originator,
            ),
            (
                "ZCZC-WXRxRWT-029047+0015-1301545-KEAX/NWS-",
                Damage::OriginatorDash,
            ),
            (
                "ZCZC-WXR-R#T-029047+0015-1301545-KEAX/NWS-",
                Damage::EventCode,
            ),
            (
                "ZCZC-WXR-RWTx029047+0015-1301545-KEAX/NWS-",
                Damage::EventDash,
            ),
            (
                "ZCZC-WXR-RWT-02904x+0015-1301545-KEAX/NWS-",
                Damage::LocationCode,
            ),
            ("ZCZC-WXR-RWT-029047", Damage::EndsInLocations),
            (
                "ZCZC-WXR-RWT-029047*0015-1301545-KEAX/NWS-",
                Damage::LocationSeparator,
            ),
            (
                "ZCZC-WXR-RWT-029047+00x5-1301545-KEAX/NWS-",
                Damage::PurgeTime,
            ),
            (
                "ZCZC-WXR-RWT-029047+0075-1301545-KEAX/NWS-",
                Damage::PurgeMinutes,
            ),
            (
                "ZCZC-WXR-RWT-029047+0000-1301545-KEAX/NWS-",
                Damage::PurgeZero,
            ),
            (
                "ZCZC-WXR-RWT-029047+0015x1301545-KEAX/NWS-",
                Damage::PurgeDash,
            ),
            (
                "ZCZC-WXR-RWT-029047+0015-13x1545-KEAX/NWS-",
                Damage::IssueTime,
            ),
            (
                "ZCZC-WXR-RWT-029047+0015-3671545-KEAX/NWS-",
                Damage::IssueDay,
            ),
            (
                "ZCZC-WXR-RWT-029047+0015-1302545-KEAX/NWS-",
                Damage::IssueRange,
            ),
            (
                "ZCZC-WXR-RWT-029047+0015-1301545xKEAX/NWS-",
                Damage::IssueDash,
            ),
            (
                "ZCZC-WXR-RWT-029047+0015-1301545-KEAX/NWS",
                Damage::CallsignDash,
            ),
            (
                "ZCZC-WXR-RWT-029047+0015-1301545-KEAX/NW-",
                Damage::CallsignLength,
            ),
            (
                "ZCZC-WXR-RWT-029047+0015-1301545-KEAX/nWS-",
                Damage::Callsign,
            ),
            (
                "ZCZC-WXR-RWT-029047+0015-1301545-KEAX/NWS-x",
                Damage::Trailing,
            ),
        ] {
            assert_eq!(parse(bad, 2026, Country::Us, &c).error, Some(why), "{bad}");
        }
        assert!(parse("NNNN", 2026, Country::Us, &c).eom);
        assert_eq!(state(6, Country::Us).unwrap().0, "California");
        assert_eq!(state(72, Country::Us).unwrap().0, "Puerto Rico");
        assert_eq!(state(66, Country::Us).unwrap().0, "Guam");
        assert!(state(3, Country::Us).is_none());
        assert_eq!(state(35, Country::Canada).unwrap().0, "Ontario");
        assert_eq!(state(35, Country::Us).unwrap().0, "New Mexico");
        assert!(state(77, Country::Us).unwrap().1.is_none());
        assert_eq!(event("CAE").unwrap().0, "Child Abduction Emergency");
        assert_eq!(event("RMT").unwrap().0, "Required Monthly Test");
        assert!(event("ZZZ").is_none());
        assert_eq!(issue_text(2026, 130, 15, 30), "May 10 15:30 UTC");
        assert_eq!(issue_text(2024, 60, 0, 5), "Feb 29 00:05 UTC");
        assert_eq!(issue_text(2026, 366, 0, 0), "");
        assert_eq!(
            issue_epoch(2026, 1, 0, 0).unwrap() - issue_epoch(2025, 365, 0, 0).unwrap(),
            86_400
        );
        assert!(valid_char(b'A') && valid_char(b'-') && valid_char(b'+'));
        assert!(!valid_char(0xAB) && !valid_char(0) && valid_char(b'a'));
        assert_eq!(
            trim(b"\x01xZCZC-WXR-RWT-029047+0015-1301545-KEAX/NWS-\x12\x34zz"),
            "ZCZC-WXR-RWT-029047+0015-1301545-KEAX/NWS-"
        );
        assert_eq!(trim(b"NNNNxy"), "NNNN");
        assert_eq!(trim(b"hello"), "");
    }

    #[test]
    fn county_csv() {
        assert!(Counties::load("garbage\nmore junk").is_err());
        let c = Counties::load("SS,CCC,Name\n48113,\"Testville, Co\"\n48,201,Sample County\nTX,48,005,Census County,H1\n").unwrap();
        assert_eq!(c.names.len(), 3);
        assert_eq!(c.names[&(48, 113)], "Testville, Co");
        assert_eq!(c.names[&(48, 201)], "Sample County");
        assert_eq!(c.names[&(48, 5)], "Census County");
        let m = parse(
            "ZCZC-WXR-TOR-548113+0030-1301530-KHGX/NWS-",
            2026,
            Country::Us,
            &c,
        );
        assert_eq!(m.locations[0].text, "Central part of Testville, Co");
    }
}
