//! The per-aircraft message log: the messages of each aircraft that a person can read, for a detail view beside a map or
//! table.

use sdrkit_util_objects::{LogRing, ObjectTable, Shift, Stamp};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// A message's category.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Category {
    /// Not stored.
    Ignore,
    /// Free text.
    Free,
    /// Crew.
    Crew,
    /// Operations.
    Ops,
    /// Company.
    Company,
    /// Maintenance.
    Maint,
    /// Fault.
    Fault,
}

impl Category {
    /// The key used in saved and sent forms.
    pub fn key(self) -> &'static str {
        match self {
            Category::Ignore => "ignore",
            Category::Free => "free",
            Category::Crew => "crew",
            Category::Ops => "ops",
            Category::Company => "company",
            Category::Maint => "maint",
            Category::Fault => "fault",
        }
    }

    /// The name shown.
    pub fn name(self) -> &'static str {
        match self {
            Category::Ignore => "Ignore",
            Category::Free => "Free text",
            Category::Crew => "Crew",
            Category::Ops => "Operations",
            Category::Company => "Company",
            Category::Maint => "Maintenance",
            Category::Fault => "Fault",
        }
    }
}

fn label_category(label: &str) -> Option<Category> {
    let b = label.as_bytes();
    match label {
        "_d" | "Q0" | "SQ" | "SA" => return Some(Category::Ignore),
        "Q1" | "Q2" | "10" | "15" | "17" | "2N" | "44" | "H1" => return Some(Category::Ops),
        "5Z" | "5Y" | "5U" => return Some(Category::Company),
        "80" | "81" | "82" | "83" => return Some(Category::Free),
        _ => {}
    }
    (b.len() == 2 && b[0] == b'Q' && matches!(b[1], b'A'..=b'H' | b'K'..=b'N' | b'P'..=b'T'))
        .then_some(Category::Ops)
}

fn binary_looking(t: &str) -> bool {
    let n = t.chars().count();
    if n == 0 {
        return false;
    }
    let good = t
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || matches!(c, ' ' | '\n' | '\r'))
        .count();
    if (good as f64) < 0.6 * n as f64 {
        return true;
    }
    let trimmed = t.trim();
    if trimmed.len() > 24 && trimmed.chars().all(|c| c.is_ascii_hexdigit()) {
        return true;
    }
    let mut run = 0;
    for c in t.chars() {
        if c.is_ascii_alphanumeric() || matches!(c, '+' | '/' | '=') {
            run += 1;
            if run > 60 {
                return true;
            }
        } else {
            run = 0;
        }
    }
    false
}

/// The category of a message by its label and text.
pub fn classify(label: &str, text: &str) -> Category {
    let lc = label_category(label);
    if lc == Some(Category::Ignore)
        || text.chars().filter(|&c| c > ' ').count() < 4
        || binary_looking(text)
    {
        return Category::Ignore;
    }
    let readable = text.chars().filter(char::is_ascii_alphabetic).count() >= 6;
    let up = text.to_uppercase();
    let has = |w: &[&str]| w.iter().any(|k| up.contains(k));
    if readable || lc.is_some() {
        if has(&["FAULT", "FAIL", "WARN", "ECAM", "CAUTION"]) {
            return Category::Fault;
        }
        if has(&["CFDS", "MAINT", "MEL ", "DEFECT", "PFR", "ATA "]) {
            return Category::Maint;
        }
    }
    match lc {
        Some(_) if has(&["CAPT", "CREW", "CABIN", "PURSER"]) => Category::Crew,
        Some(c) => c,
        None if !readable => Category::Ignore,
        None if has(&["CAPT", "CREW", "CABIN", "PURSER", "PAX "]) => Category::Crew,
        None => Category::Free,
    }
}

fn redact_email(t: &str) -> String {
    let c: Vec<char> = t.chars().collect();
    let mut out = String::with_capacity(t.len());
    let local = |x: char| x.is_ascii_alphanumeric() || "._%+-".contains(x);
    let dom = |x: char| x.is_ascii_alphanumeric() || ".-".contains(x);
    let mut i = 0;
    while i < c.len() {
        if c[i] == '@' && i > 0 && local(c[i - 1]) {
            let mut s = i;
            while s > 0 && local(c[s - 1]) {
                s -= 1;
            }
            let mut e = i + 1;
            while e < c.len() && dom(c[e]) {
                e += 1;
            }
            let d: String = c[i + 1..e].iter().collect();
            let d = d.trim_end_matches('.');
            if d.contains('.') {
                let keep = out.chars().count() - (i - s);
                out = out.chars().take(keep).collect();
                out.push_str("[email]");
                i += 1 + d.chars().count();
                continue;
            }
        }
        out.push(c[i]);
        i += 1;
    }
    out
}

/// The text with e-mail addresses replaced by `[email]` and telephone numbers by `[number]`. A run of digits is a telephone
/// number in two cases. It starts with `+` or `00` and has 8 or more digits. Or it has 9 to 15 digits in at least three groups.
/// Groups of four digits (times) stay.
pub fn redact(text: &str) -> String {
    let t = redact_email(text);
    let c: Vec<char> = t.chars().collect();
    let mut out = String::with_capacity(t.len());
    let sep = |x: char| " -.()/".contains(x);
    let mut i = 0;
    while i < c.len() {
        let start =
            (c[i] == '+' || c[i].is_ascii_digit()) && (i == 0 || !c[i - 1].is_ascii_alphanumeric());
        if start {
            let mut e = i + 1;
            let mut last_digit = c[i].is_ascii_digit().then_some(i);
            while e < c.len() && (c[e].is_ascii_digit() || sep(c[e])) {
                if c[e].is_ascii_digit() {
                    last_digit = Some(e);
                }
                e += 1;
            }
            if let Some(ld) = last_digit {
                let end = ld + 1;
                let run: String = c[i..end].iter().collect();
                let after_ok = end >= c.len() || !c[end].is_ascii_alphanumeric();
                let digits = run.chars().filter(char::is_ascii_digit).count();
                let seps = run.chars().filter(|&x| sep(x)).count();
                let prefixed = run.starts_with('+') || run.starts_with("00");
                let times = run.split(' ').count() >= 2
                    && run
                        .split(' ')
                        .all(|g| g.len() == 4 && g.chars().all(|x| x.is_ascii_digit()));
                if after_ok
                    && !times
                    && ((prefixed && digits >= 8) || (seps >= 2 && (9..=15).contains(&digits)))
                {
                    out.push_str("[number]");
                } else {
                    out.extend(&c[i..end]);
                }
                i = end;
                continue;
            }
        }
        out.push(c[i]);
        i += 1;
    }
    out
}

/// The registration key: upper case, dashes, dots and spaces removed.
pub fn reg_key(r: &str) -> String {
    r.chars()
        .filter(|c| !"-. ".contains(*c))
        .flat_map(char::to_uppercase)
        .collect()
}

/// The flight key: upper case with leading zeros of the number removed, so `EK0201` and `EK201` match.
pub fn flight_key(f: &str) -> String {
    let u: Vec<char> = f.trim().to_uppercase().chars().collect();
    let mut p = 0;
    while p < u.len() && p < 3 && (u[p].is_ascii_alphabetic() || p < 2) {
        p += 1;
        if p >= 2 && u.get(p).is_some_and(char::is_ascii_digit) {
            break;
        }
    }
    let prefix: String = u[..p].iter().collect();
    let num: String = u[p..].iter().collect();
    format!("{prefix}{}", num.trim_start_matches('0'))
}

/// One stored message.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Entry {
    /// Wall-clock time, Unix seconds.
    pub t: f64,
    /// The link it came on ("ACARS", "VDL2", "HFDL" ...).
    pub link: String,
    /// The frequency, when known.
    pub freq_hz: Option<f64>,
    /// The ACARS label.
    pub label: String,
    /// The category.
    pub category: Category,
    /// The redacted text (at most [`MAX_TEXT`] characters).
    pub text: String,
}

/// A message offered to the log.
#[derive(Clone, Debug, Default)]
pub struct Offer<'a> {
    /// Wall-clock time, Unix seconds.
    pub t: f64,
    /// The link name.
    pub link: &'a str,
    /// The frequency, when known.
    pub freq_hz: Option<f64>,
    /// The 24-bit address, when known.
    pub icao: Option<u32>,
    /// The registration.
    pub reg: &'a str,
    /// The flight id.
    pub flight: &'a str,
    /// The label.
    pub label: &'a str,
    /// The text.
    pub text: &'a str,
}

/// Messages kept per aircraft.
pub const PER_AIRCRAFT: usize = 50;
/// Aircraft kept.
pub const AIRCRAFT: usize = 400;
/// The age limit, seconds (48 hours).
pub const MAX_AGE_S: f64 = 48.0 * 3600.0;
/// Registration-to-address links kept before the list starts again.
pub const ALIASES: usize = 4000;
/// The most characters kept of one text.
pub const MAX_TEXT: usize = 500;
/// The same label and text within this many seconds is a repeat.
pub const REPEAT_S: f64 = 30.0;

/// The key by which the log identifies an aircraft.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
enum Who {
    Address(u32),
    Registration(String),
    Flight(String),
}

/// The log. It finds an aircraft by its 24-bit address, its registration, or its flight id if the caller says that the
/// callsign is unique. It keeps at most [`AIRCRAFT`] aircraft, [`PER_AIRCRAFT`] messages each, and nothing older than
/// [`MAX_AGE_S`]. If a message provides an address and a registration, the earlier messages of the registration move under the
/// address.
///
/// ```
/// use sdrkit_proto_aero::msglog::{MessageLog, Offer, Category};
/// let mut log = MessageLog::new();
/// let o = Offer { t: 0.0, link: "VDL2", label: "80", reg: "A6-EDA", text: "PLEASE ADVISE GATE", ..Offer::default() };
/// assert!(log.offer(&o));
/// let v = log.for_aircraft(1.0, None, "A6EDA", "", false);
/// assert_eq!(v[0].category, Category::Free);
/// ```
///
/// It is saved with serde. Its times move with [`Shift`].
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MessageLog {
    rows: ObjectTable<Who, LogRing<Entry>>,
    alias: HashMap<String, u32>,
}

/// Moves the time of the entry.
impl Shift for Entry {
    fn shift_times(&mut self, by_s: f64) {
        self.t += by_s;
    }
}

/// Moves the times of every entry and of every aircraft's last message.
impl Shift for MessageLog {
    fn shift_times(&mut self, by_s: f64) {
        self.rows.shift_times(by_s);
    }
}

impl Default for MessageLog {
    fn default() -> Self {
        Self::new()
    }
}

impl MessageLog {
    /// An empty log.
    pub fn new() -> Self {
        MessageLog {
            rows: ObjectTable::new(AIRCRAFT),
            alias: HashMap::new(),
        }
    }

    /// Offers a message. Returns true when it is stored. These messages are not stored: a message that [`classify`]
    /// ignores, a repeat within [`REPEAT_S`], a time that is not finite, and a message with no address, registration or flight.
    pub fn offer(&mut self, o: &Offer) -> bool {
        let category = classify(o.label, o.text);
        if category == Category::Ignore || !o.t.is_finite() {
            return false;
        }
        let rk = reg_key(o.reg);
        let who = if let Some(a) = o.icao {
            if !rk.is_empty() {
                if self.alias.len() >= ALIASES {
                    self.alias.clear();
                }
                self.alias.insert(rk.clone(), a);
                if let Some(old) = self.rows.remove(&Who::Registration(rk)) {
                    let at = old.seen.last();
                    if let Ok(up) = self
                        .rows
                        .upsert(&Who::Address(a), at, || LogRing::new(PER_AIRCRAFT))
                    {
                        for e in old.record.iter() {
                            up.row.push(e.clone());
                        }
                    }
                }
            }
            Who::Address(a)
        } else if let Some(&a) = self.alias.get(&rk) {
            Who::Address(a)
        } else if !rk.is_empty() {
            Who::Registration(rk)
        } else if !o.flight.trim().is_empty() {
            Who::Flight(flight_key(o.flight))
        } else {
            return false;
        };
        let text: String = redact(o.text).chars().take(MAX_TEXT).collect();
        if let Some(log) = self.rows.get(&who) {
            if log
                .iter()
                .any(|x| x.label == o.label && x.text == text && (o.t - x.t).abs() < REPEAT_S)
            {
                return false;
            }
        }
        let Ok(up) = self
            .rows
            .upsert(&who, Stamp::at(o.t), || LogRing::new(PER_AIRCRAFT))
        else {
            return false;
        };
        up.row.push(Entry {
            t: o.t,
            link: o.link.to_owned(),
            freq_hz: o.freq_hz,
            label: o.label.to_owned(),
            category,
            text,
        });
        true
    }

    /// Removes the aircraft not heard within [`MAX_AGE_S`] of `now`.
    pub fn prune(&mut self, now: f64) {
        self.rows.expire(now, MAX_AGE_S);
    }

    /// The messages of an aircraft, oldest first: by its address, its registration, and its flight id when `callsign_unique`.
    pub fn for_aircraft(
        &self,
        now: f64,
        icao: Option<u32>,
        reg: &str,
        callsign: &str,
        callsign_unique: bool,
    ) -> Vec<&Entry> {
        let rk = reg_key(reg);
        let mut keys = Vec::with_capacity(4);
        if let Some(a) = icao {
            keys.push(Who::Address(a));
        }
        if !rk.is_empty() {
            if let Some(&a) = self.alias.get(&rk) {
                keys.push(Who::Address(a));
            }
            keys.push(Who::Registration(rk));
        }
        if callsign_unique && !callsign.trim().is_empty() {
            keys.push(Who::Flight(flight_key(callsign)));
        }
        keys.dedup();
        let mut v: Vec<&Entry> = keys
            .iter()
            .filter_map(|k| self.rows.get(k))
            .flat_map(|x| x.iter())
            .filter(|e| now - e.t <= MAX_AGE_S)
            .collect();
        v.sort_by(|a, b| a.t.total_cmp(&b.t));
        v
    }

    /// The number of aircraft held.
    pub fn aircraft(&self) -> usize {
        self.rows.len()
    }
}
