//! The programme guide: events per service, merged from EIT sections, queried by time window, now and next, full text and
//! series.
//!
//! Standard: ETSI EN 300 468 5.2.4, ETSI TS 101 211 4.1.

use crate::si::dvb_text as decode;
use crate::tables::{Eit, RawEvent};
use sdrkit_util_text::dvb::{Options as TextOptions, Usage as TextUse};
use std::collections::BTreeMap;

/// A service key: (original_network_id, transport_stream_id, service_id).
pub type ServiceKey = (u16, u16, u16);

/// One event of the guide.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct EpgEvent {
    /// (onid, tsid, service_id).
    pub service: ServiceKey,
    /// event_id.
    pub event_id: u16,
    /// UTC seconds since 1970.
    pub start: i64,
    /// Seconds.
    pub duration: i64,
    /// running_status (4 running, 1 not running, 2 starts in a few seconds, 3 pausing, 0 undefined).
    pub running: u8,
    /// Short event name.
    pub title: String,
    /// Short event text.
    pub text: String,
    /// Extended text (items as "description: value" lines first).
    pub extended: String,
    /// ISO 639-2 language of the descriptors used.
    pub lang: String,
    /// First level-1 content nibble (-1 unknown).
    pub genre: i8,
    /// Up to 8 content bytes.
    pub content: Vec<u8>,
    /// Highest minimum age over all countries (rating + 3; 0 none).
    pub min_age: u8,
    /// Parental ratings per country: (country, rating).
    pub ratings: Vec<(String, u8)>,
    /// Programme CRID.
    pub crid_programme: String,
    /// Series CRID.
    pub crid_series: String,
    /// free_CA_mode.
    pub free_ca: bool,
    /// From an EIT other table.
    pub other_ts: bool,
    /// From a schedule table (not only present/following).
    pub schedule: bool,
}

impl EpgEvent {
    /// When the event ends.
    pub fn end(&self) -> i64 {
        self.start + self.duration
    }
}

/// Folds the bibliographic and terminology ISO 639-2 codes (and nob/nno to nor), so "ger" equals "deu".
pub fn fold_lang(l: &str) -> String {
    let l = l.to_ascii_lowercase();
    let f = match l.as_str() {
        "ger" => "deu",
        "fre" => "fra",
        "dut" => "nld",
        "cze" => "ces",
        "gre" => "ell",
        "rum" => "ron",
        "slo" => "slk",
        "chi" => "zho",
        "ice" => "isl",
        "arm" => "hye",
        "baq" => "eus",
        "mac" => "mkd",
        "alb" => "sqi",
        "bur" => "mya",
        "geo" => "kat",
        "per" => "fas",
        "tib" => "bod",
        "wel" => "cym",
        "may" => "msa",
        "mao" => "mri",
        "nob" | "nno" => "nor",
        _ => return l,
    };
    f.to_string()
}

/// Whether two language codes are the same language after folding.
pub fn same_lang(a: &str, b: &str) -> bool {
    !a.is_empty() && fold_lang(a) == fold_lang(b)
}

/// Guide limits.
pub const MAX_EVENTS_PER_SERVICE: usize = 3000;
/// Services the guide keeps.
pub const MAX_SERVICES: usize = 400;
/// Events that ended this long ago expire.
pub const EXPIRE_S: i64 = 6 * 3600;
/// Events starting further ahead are refused.
pub const MAX_AHEAD_S: i64 = 21 * 86_400;
/// Events lasting longer are refused.
pub const MAX_DURATION_S: i64 = 48 * 3600;

/// Present and following of one service, from EIT p/f.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PresentFollowing {
    /// Section 0: now.
    pub now: Option<EpgEvent>,
    /// Section 1: next.
    pub next: Option<EpgEvent>,
}

/// The guide.
#[derive(Clone, Debug, Default)]
pub struct Epg {
    services: BTreeMap<ServiceKey, BTreeMap<u16, EpgEvent>>,
    pf: BTreeMap<ServiceKey, PresentFollowing>,
    /// The preferred language (ISO 639-2), empty for the first in the loop.
    pub preferred_lang: String,
    /// Text options.
    pub text: TextOptions,
    /// Events refused (bad time, zero duration, too far, too long).
    pub refused: u64,
}

impl Epg {
    /// An empty guide.
    pub fn new() -> Self {
        Self::default()
    }

    /// Turns a raw event into a guide event, choosing the language.
    pub fn convert(
        &self,
        key: ServiceKey,
        raw: &RawEvent,
        other_ts: bool,
        schedule: bool,
    ) -> Option<EpgEvent> {
        let start = raw.start?;
        let o = &self.text;
        // Language choice: the preferred one when present, else the first in the loop.
        let pick = |langs: &mut dyn Iterator<Item = &String>| -> Option<String> {
            let all: Vec<&String> = langs.collect();
            if !self.preferred_lang.is_empty() {
                if let Some(l) = all.iter().find(|l| same_lang(l, &self.preferred_lang)) {
                    return Some((*l).clone());
                }
            }
            all.first().map(|l| (*l).clone())
        };
        let lang = pick(
            &mut raw
                .short
                .iter()
                .map(|s| &s.0)
                .chain(raw.extended.iter().map(|p| &p.lang)),
        )
        .unwrap_or_default();
        let (mut title, mut text) = (String::new(), String::new());
        if let Some(s) = raw.short.iter().find(|s| s.0 == lang).or(raw.short.first()) {
            title = decode(&s.1, TextUse::Name, o);
            text = decode(&s.2, TextUse::Description, o);
        }
        // Extended: pieces of the chosen language joined in number order (a later piece with the same number replaces the earlier)
        // before decoding, since a character may be split between descriptors.
        let mut pieces: BTreeMap<u8, &crate::tables::ExtendedPiece> = BTreeMap::new();
        for p in raw.extended.iter().filter(|p| p.lang == lang) {
            pieces.insert(p.number, p);
        }
        let mut lines = Vec::new();
        let mut joined = Vec::new();
        let mut prefix: Option<u8> = None;
        for p in pieces.values() {
            for (d, v) in &p.items {
                lines.push(format!(
                    "{}: {}",
                    decode(d, TextUse::Name, o),
                    decode(v, TextUse::Name, o)
                ));
            }
            // The table prefix of the first piece applies to all (later pieces repeat it or not).
            let mut t: &[u8] = &p.text;
            if let Some(&b0) = t.first() {
                if b0 < 0x20 {
                    if prefix.is_none() {
                        prefix = Some(b0);
                        joined.push(b0);
                        if b0 == 0x10 && t.len() >= 3 {
                            joined.extend_from_slice(&t[1..3]);
                            t = &t[3..];
                        } else {
                            t = &t[1..];
                        }
                    } else if Some(b0) == prefix {
                        t = if b0 == 0x10 && t.len() >= 3 {
                            &t[3..]
                        } else {
                            &t[1..]
                        };
                    }
                }
            }
            joined.extend_from_slice(t);
        }
        let mut extended = lines.join("\n");
        let body = decode(&joined, TextUse::Description, o);
        if !body.is_empty() {
            if !extended.is_empty() {
                extended.push('\n');
            }
            extended.push_str(&body);
        }
        let genre = raw.content.first().map_or(-1, |c| (c >> 4) as i8);
        let min_age = raw
            .ratings
            .iter()
            .filter(|r| (0x01..=0x0F).contains(&r.1))
            .map(|r| r.1 + 3)
            .max()
            .unwrap_or(0);
        let crid = |t: u8| {
            raw.crids
                .iter()
                .find(|c| c.0 == t || c.0 == t + 0x30)
                .map(|c| c.1.clone())
                .unwrap_or_default()
        };
        Some(EpgEvent {
            service: key,
            event_id: raw.event_id,
            start,
            duration: raw.duration,
            running: raw.running,
            title,
            text,
            extended,
            lang,
            genre,
            content: raw.content.clone(),
            min_age,
            ratings: raw.ratings.clone(),
            crid_programme: crid(1),
            crid_series: crid(2),
            free_ca: raw.free_ca,
            other_ts,
            schedule,
        })
    }

    /// Applies one EIT section at UTC `now` (0 when unknown: no expiry or look-ahead checks). Returns the events stored.
    pub fn apply(&mut self, e: &Eit, now: i64) -> Vec<EpgEvent> {
        let key = (e.onid, e.tsid, e.service_id);
        let other = matches!(e.table_id, 0x4F | 0x60..=0x6F);
        let pf = matches!(e.table_id, 0x4E | 0x4F);
        let mut stored = Vec::new();
        if pf && e.section <= 1 {
            let ev = e
                .events
                .first()
                .and_then(|r| self.convert(key, r, other, false))
                .filter(|x| x.duration > 0);
            let slot = self.pf.entry(key).or_default();
            if e.section == 0 {
                slot.now = ev.clone();
            } else {
                slot.next = ev.clone();
            }
        }
        for raw in &e.events {
            let Some(ev) = self.convert(key, raw, other, !pf) else {
                self.refused += 1;
                continue;
            };
            if self.insert(ev.clone(), now) {
                stored.push(ev);
            }
        }
        stored
    }

    /// Stores an event if it passes the guide limits. Returns whether it was stored.
    pub fn insert(&mut self, mut ev: EpgEvent, now: i64) -> bool {
        if ev.duration <= 0 || ev.duration > MAX_DURATION_S {
            self.refused += 1;
            return false;
        }
        if now > 0 && (ev.start > now + MAX_AHEAD_S || ev.end() < now - EXPIRE_S) {
            self.refused += 1;
            return false;
        }
        if !self.services.contains_key(&ev.service) && self.services.len() >= MAX_SERVICES {
            self.refused += 1;
            return false;
        }
        let events = self.services.entry(ev.service).or_default();
        if let Some(old) = events.get(&ev.event_id) {
            if old.title == ev.title {
                if ev.extended.is_empty() {
                    ev.extended = old.extended.clone();
                }
                if ev.text.is_empty() {
                    ev.text = old.text.clone();
                }
                if ev.genre < 0 {
                    ev.genre = old.genre;
                    ev.content = old.content.clone();
                }
                if ev.min_age == 0 && ev.ratings.is_empty() {
                    ev.min_age = old.min_age;
                    ev.ratings = old.ratings.clone();
                }
                if ev.crid_programme.is_empty() {
                    ev.crid_programme = old.crid_programme.clone();
                }
                if ev.crid_series.is_empty() {
                    ev.crid_series = old.crid_series.clone();
                }
                ev.schedule |= old.schedule;
            }
            if ev.running == 0 && ev.start == old.start {
                ev.running = old.running;
            }
        }
        // A reschedule: any other event of the service overlapping the new one goes.
        let (s, e, id) = (ev.start, ev.end(), ev.event_id);
        events.retain(|&k, o| k == id || o.end() <= s || o.start >= e);
        events.insert(id, ev);
        if events.len() > MAX_EVENTS_PER_SERVICE {
            // Drop the earliest-starting event to respect the cap.
            if let Some((&k, _)) = events.iter().min_by_key(|(_, o)| o.start) {
                events.remove(&k);
            }
        }
        true
    }

    /// Removes events that ended more than 6 hours before `now`.
    pub fn expire(&mut self, now: i64) {
        for ev in self.services.values_mut() {
            ev.retain(|_, e| e.end() >= now - EXPIRE_S);
        }
        self.services.retain(|_, v| !v.is_empty());
    }

    /// Removes the guide of one multiplex (onid, tsid).
    pub fn clear_multiplex(&mut self, onid: u16, tsid: u16) {
        self.services.retain(|k, _| !(k.0 == onid && k.1 == tsid));
        self.pf.retain(|k, _| !(k.0 == onid && k.1 == tsid));
    }

    /// Every event of a service, sorted by start.
    pub fn events(&self, key: ServiceKey) -> Vec<&EpgEvent> {
        let mut v: Vec<&EpgEvent> = self
            .services
            .get(&key)
            .map(|m| m.values().collect())
            .unwrap_or_default();
        v.sort_by_key(|e| (e.start, e.event_id));
        v
    }

    /// Events of a service overlapping `[from, to)`, sorted by start (the guide grid).
    pub fn window(&self, key: ServiceKey, from: i64, to: i64) -> Vec<&EpgEvent> {
        self.events(key)
            .into_iter()
            .filter(|e| e.end() > from && e.start < to)
            .collect()
    }

    /// Now and next of a service: from EIT p/f when received, otherwise from the stored events at UTC `utc`.
    pub fn now_next(&self, key: ServiceKey, utc: i64) -> (Option<EpgEvent>, Option<EpgEvent>) {
        if let Some(pf) = self.pf.get(&key) {
            if pf.now.is_some() || pf.next.is_some() {
                return (pf.now.clone(), pf.next.clone());
            }
        }
        let ev = self.events(key);
        let now = ev
            .iter()
            .find(|e| e.start <= utc && utc < e.end())
            .map(|e| (*e).clone());
        let next = ev.iter().find(|e| e.start > utc).map(|e| (*e).clone());
        (now, next)
    }

    /// Full-text search: events whose title, text or extended text hold every word (case-insensitive), at most 200.
    pub fn search(&self, query: &str) -> Vec<&EpgEvent> {
        let words: Vec<String> = query.split_whitespace().map(|w| w.to_lowercase()).collect();
        let mut out = Vec::new();
        if words.is_empty() {
            return out;
        }
        for evs in self.services.values() {
            for e in evs.values() {
                let hay = format!("{} {} {}", e.title, e.text, e.extended).to_lowercase();
                if words.iter().all(|w| hay.contains(w.as_str())) {
                    out.push(e);
                    if out.len() >= 200 {
                        return out;
                    }
                }
            }
        }
        out.sort_by_key(|e| e.start);
        out
    }

    /// Every event of a series (by series CRID).
    pub fn series(&self, crid: &str) -> Vec<&EpgEvent> {
        let mut v: Vec<&EpgEvent> = self
            .services
            .values()
            .flat_map(|m| m.values())
            .filter(|e| e.crid_series == crid)
            .collect();
        v.sort_by_key(|e| e.start);
        v
    }

    /// Services that have events.
    pub fn services(&self) -> impl Iterator<Item = &ServiceKey> {
        self.services.keys()
    }

    /// Number of stored events.
    pub fn len(&self) -> usize {
        self.services.values().map(|m| m.len()).sum()
    }

    /// No events.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Every event, by service and then by event_id.
    pub fn iter(&self) -> impl Iterator<Item = &EpgEvent> {
        self.services.values().flat_map(|m| m.values())
    }
}

/// A demonstration guide for screen tests without a signal: 8 one-hour events per service from the current half
/// hour minus 30 minutes, neutral titles, the second event running.
pub fn demo_guide(services: &[ServiceKey], utc: i64) -> Epg {
    let mut g = Epg::new();
    let half = utc - utc.rem_euclid(1800) - 1800;
    for &k in services {
        for i in 0..8i64 {
            g.insert(
                EpgEvent {
                    service: k,
                    event_id: 1 + i as u16,
                    start: half + i * 3600,
                    duration: 3600,
                    running: if i == 1 { 4 } else { 1 },
                    title: format!("Programme {}", i + 1),
                    text: "Demonstration event.".into(),
                    genre: -1,
                    ..Default::default()
                },
                0,
            );
        }
    }
    g
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tables::ExtendedPiece;

    fn raw(id: u16, start: i64, dur: i64, title: &str) -> RawEvent {
        RawEvent {
            event_id: id,
            start: Some(start),
            duration: dur,
            running: 1,
            short: vec![("eng".into(), title.as_bytes().to_vec(), b"t".to_vec())],
            ..Default::default()
        }
    }

    #[test]
    fn merge_rules() {
        let mut g = Epg::new();
        let k = (1, 2, 3);
        let t0 = 1_768_467_600;
        let mut a = g.convert(k, &raw(1, t0, 3600, "A"), false, true).unwrap();
        a.extended = "long text".into();
        assert!(g.insert(a, t0));
        // A copy without extended text keeps it.
        let b = g.convert(k, &raw(1, t0, 3600, "A"), false, true).unwrap();
        g.insert(b, t0);
        assert_eq!(g.events(k)[0].extended, "long text");
        // An overlapping event replaces the old one.
        g.insert(
            g.convert(k, &raw(2, t0 + 1800, 3600, "B"), false, true)
                .unwrap(),
            t0,
        );
        assert_eq!(g.events(k).len(), 1);
        assert_eq!(g.events(k)[0].event_id, 2);
        // Limits.
        assert!(!g.insert(
            g.convert(k, &raw(3, t0 + 22 * 86_400, 600, "far"), false, true)
                .unwrap(),
            t0
        ));
        assert!(!g.insert(
            g.convert(k, &raw(4, t0 + 7200, 49 * 3600, "long"), false, true)
                .unwrap(),
            t0
        ));
        assert!(!g.insert(
            g.convert(k, &raw(5, t0 + 7200, 0, "zero"), false, true)
                .unwrap(),
            t0
        ));
        // Expiry after 6 h.
        g.expire(t0 + 1800 + 3600 + 6 * 3600 + 1);
        assert!(g.events(k).is_empty());
    }

    #[test]
    fn cap_search_series_and_json() {
        let mut g = Epg::new();
        let k = (1, 2, 3);
        for i in 0..3100u16 {
            g.insert(
                EpgEvent {
                    service: k,
                    event_id: i,
                    start: 1_000_000 + i as i64 * 60,
                    duration: 60,
                    title: format!("Ev {i}"),
                    genre: -1,
                    ..Default::default()
                },
                0,
            );
        }
        assert_eq!(g.events(k).len(), MAX_EVENTS_PER_SERVICE);
        let mut s = g
            .convert(
                k,
                &raw(9000, 2_000_000, 60, "Big Match tonight"),
                false,
                true,
            )
            .unwrap();
        s.crid_series = "/series1".into();
        g.insert(s, 0);
        assert_eq!(g.search("match BIG").len(), 1);
        assert_eq!(g.series("/series1").len(), 1);
        assert_eq!(g.iter().count(), g.len());
    }

    #[test]
    fn language_preference_and_extended_joining() {
        let mut g = Epg::new();
        g.preferred_lang = "fin".into();
        let mut r = raw(1, 1_768_467_600, 600, "");
        r.short = vec![
            ("swe".into(), b"Svensk".to_vec(), vec![]),
            ("fin".into(), b"Suomi".to_vec(), vec![]),
        ];
        // A UTF-8 'ü' split between two extended descriptors.
        r.extended = vec![
            ExtendedPiece {
                lang: "fin".into(),
                number: 0,
                last: 1,
                items: vec![],
                text: vec![0x15, b'P', b'r', 0xC3],
            },
            ExtendedPiece {
                lang: "fin".into(),
                number: 1,
                last: 1,
                items: vec![],
                text: vec![0xBC, b'f'],
            },
        ];
        let e = g.convert((0, 0, 0), &r, false, false).unwrap();
        assert_eq!(e.title, "Suomi");
        assert_eq!(e.extended, "Prüf");
        g.preferred_lang = "ger".into();
        r.short = vec![
            ("eng".into(), b"English".to_vec(), vec![]),
            ("deu".into(), b"Deutsch".to_vec(), vec![]),
        ];
        assert_eq!(
            g.convert((0, 0, 0), &r, false, false).unwrap().title,
            "Deutsch"
        );
    }
}
