//! Tower positions, a host service: lookups in the public OpenCelliD database through a fetch function the host
//! supplies (the engine opens no network connection), a cache kept as CSV, a rate limit, the site fallback for LTE sectors, and
//! tower files the user downloads. Tower data (c) OpenCelliD, CC BY-SA 4.0; nothing is bundled.

use std::collections::{BTreeMap, VecDeque};

/// The attribution shown wherever positions are drawn.
pub const ATTRIBUTION: &str = "Tower data (c) OpenCelliD, CC BY-SA 4.0";

/// The radio technology of a lookup.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Radio {
    /// LTE.
    Lte,
    /// GSM.
    Gsm,
    /// UMTS.
    Umts,
}

impl Radio {
    fn name(self) -> &'static str {
        match self {
            Radio::Lte => "LTE",
            Radio::Gsm => "GSM",
            Radio::Umts => "UMTS",
        }
    }
}

/// What identifies a cell in the database.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TowerKey {
    /// Radio.
    pub radio: Radio,
    /// MCC.
    pub mcc: u16,
    /// MNC.
    pub mnc: u16,
    /// TAC or LAC.
    pub area: u32,
    /// Cell identity.
    pub cell: u32,
}

/// A position.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Position {
    /// Latitude.
    pub lat: f64,
    /// Longitude.
    pub lon: f64,
    /// Range, metres.
    pub range_m: f64,
    /// Samples behind it.
    pub samples: u32,
}

/// The answer to one lookup.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Answer {
    /// Found.
    Found(Position),
    /// Not in the database (code 1).
    NotFound,
    /// The key is bad (code 2).
    BadKey,
    /// A limit was reached (codes 6, 7).
    Limit,
    /// Anything else.
    Error,
}

/// Percent-encodes a query value.
pub fn percent(s: &str) -> String {
    let mut o = String::new();
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || b"-_.~".contains(&b) {
            o.push(b as char);
        } else {
            o.push_str(&format!("%{b:02X}"));
        }
    }
    o
}

/// The lookup URL.
pub fn url(key: &str, k: &TowerKey) -> String {
    format!(
        "https://opencellid.org/cell/get?key={}&mcc={}&mnc={}&lac={}&cellid={}&radio={}&format=json",
        percent(key),
        k.mcc,
        k.mnc,
        k.area,
        k.cell,
        k.radio.name()
    )
}

/// The fields of an answer of the database that the lookup reads.
#[derive(serde::Deserialize)]
struct Reply {
    lat: Option<f64>,
    lon: Option<f64>,
    range: Option<f64>,
    samples: Option<u32>,
    code: Option<i64>,
}

/// Parses an answer.
pub fn parse_answer(body: &str) -> Answer {
    let Ok(v) = serde_json::from_str::<Reply>(body) else {
        return Answer::Error;
    };
    if let (Some(lat), Some(lon)) = (v.lat, v.lon) {
        return Answer::Found(Position {
            lat,
            lon,
            range_m: v.range.unwrap_or(0.0),
            samples: v.samples.unwrap_or(0),
        });
    }
    match v.code {
        Some(1) => Answer::NotFound,
        Some(2) => Answer::BadKey,
        Some(6) | Some(7) => Answer::Limit,
        _ => Answer::Error,
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Entry {
    Found(Position),
    NotFound(f64),
}

/// The lookup service: queue, rate limit, cache, tower file.
pub struct Towers {
    key: String,
    cache: BTreeMap<TowerKey, Entry>,
    file: BTreeMap<(u16, u16, u32), Position>,
    queue: VecDeque<TowerKey>,
    last_request: f64,
    halted: bool,
}

impl Towers {
    /// A service with this API key (empty: no lookups).
    pub fn new(key: &str) -> Self {
        Towers {
            key: key.into(),
            cache: BTreeMap::new(),
            file: BTreeMap::new(),
            queue: VecDeque::new(),
            last_request: f64::NEG_INFINITY,
            halted: false,
        }
    }
    /// A new key (lifts a halt).
    pub fn set_key(&mut self, key: &str) {
        self.key = key.into();
        self.halted = false;
    }
    /// Whether lookups are halted (bad key, daily limit).
    pub fn halted(&self) -> bool {
        self.halted
    }
    /// Requests a cell (never blocks; a cell that is cached or queued is not requested again). `now` is wall time, seconds.
    pub fn want(&mut self, k: TowerKey, now: f64) {
        match self.cache.get(&k) {
            Some(Entry::Found(_)) => return,
            Some(Entry::NotFound(t)) if now - t < 86_400.0 => return,
            _ => {}
        }
        if !self.queue.contains(&k) {
            self.queue.push_back(k);
        }
    }
    /// Runs at most one request a second with `fetch` (from a worker thread in a host). `fetch` returns the body of the answer,
    /// or `None` if the request failed.
    pub fn pump(&mut self, now: f64, fetch: &mut dyn FnMut(&str) -> Option<String>) -> bool {
        if self.halted || self.key.is_empty() || now - self.last_request < 1.0 {
            return false;
        }
        let Some(k) = self.queue.pop_front() else {
            return false;
        };
        self.last_request = now;
        let answer = match fetch(&url(&self.key, &k)) {
            Some(body) => parse_answer(&body),
            None => Answer::Error,
        };
        match answer {
            Answer::Found(p) => {
                self.cache.insert(k, Entry::Found(p));
            }
            Answer::NotFound => {
                self.cache.insert(k, Entry::NotFound(now));
            }
            Answer::BadKey | Answer::Limit => self.halted = true,
            Answer::Error => {}
        }
        true
    }
    /// The position of a cell: the database's answer, else the tower file, else (LTE) a cached sector of the same site.
    pub fn position(&self, k: &TowerKey) -> Option<(Position, &'static str)> {
        if let Some(Entry::Found(p)) = self.cache.get(k) {
            return Some((*p, "database"));
        }
        if let Some(p) = self.file.get(&(k.mcc, k.mnc, k.cell & 0x0FFF_FFFF)) {
            return Some((*p, "CSV"));
        }
        if k.radio == Radio::Lte {
            let enb = k.cell >> 8;
            for (ck, e) in &self.cache {
                if let Entry::Found(p) = e {
                    if ck.radio == Radio::Lte
                        && ck.mcc == k.mcc
                        && ck.mnc == k.mnc
                        && ck.cell >> 8 == enb
                    {
                        return Some((*p, "site"));
                    }
                }
            }
        }
        None
    }
    /// Other sectors of an LTE cell's eNB to ask when the cell itself is not in the database.
    pub fn sector_keys(k: &TowerKey, sectors: &[u32]) -> Vec<TowerKey> {
        sectors
            .iter()
            .map(|s| TowerKey {
                cell: (k.cell & !0xFF) | (s & 0xFF),
                ..*k
            })
            .filter(|x| x != k)
            .collect()
    }
    /// Loads a tower file (OpenCelliD CSV: radio, mcc, net, area, cell, unit, lon, lat, range, samples, ...).
    pub fn load_csv(&mut self, text: &str) -> usize {
        let mut n = 0;
        for line in text.lines() {
            let f = sdrkit_util_text::csv::split_line(line, ',');
            if f.len() < 8 {
                continue;
            }
            let (Ok(mcc), Ok(mnc), Ok(cell), Ok(lon), Ok(lat)) = (
                f[1].trim().parse::<u16>(),
                f[2].trim().parse::<u16>(),
                f[4].trim().parse::<u32>(),
                f[6].trim().parse::<f64>(),
                f[7].trim().parse::<f64>(),
            ) else {
                continue;
            };
            let range_m = f.get(8).and_then(|v| v.trim().parse().ok()).unwrap_or(0.0);
            let samples = f.get(9).and_then(|v| v.trim().parse().ok()).unwrap_or(0);
            self.file.insert(
                (mcc, mnc, cell & 0x0FFF_FFFF),
                Position {
                    lat,
                    lon,
                    range_m,
                    samples,
                },
            );
            n += 1;
        }
        n
    }
    /// The cache as CSV (found positions and recent not-found answers).
    pub fn cache_csv(&self) -> String {
        let mut s = String::from("radio,mcc,mnc,area,cell,state,lat,lon,range,samples,time\n");
        for (k, e) in &self.cache {
            match e {
                Entry::Found(p) => s.push_str(&format!(
                    "{},{},{},{},{},found,{},{},{},{},\n",
                    k.radio.name(),
                    k.mcc,
                    k.mnc,
                    k.area,
                    k.cell,
                    p.lat,
                    p.lon,
                    p.range_m,
                    p.samples
                )),
                Entry::NotFound(t) => s.push_str(&format!(
                    "{},{},{},{},{},notfound,,,,,{}\n",
                    k.radio.name(),
                    k.mcc,
                    k.mnc,
                    k.area,
                    k.cell,
                    t
                )),
            }
        }
        s
    }
    /// Restores the cache from CSV.
    pub fn load_cache_csv(&mut self, text: &str) {
        for line in text.lines().skip(1) {
            let f = sdrkit_util_text::csv::split_line(line, ',');
            if f.len() < 11 {
                continue;
            }
            let radio = match f[0].as_str() {
                "LTE" => Radio::Lte,
                "GSM" => Radio::Gsm,
                _ => Radio::Umts,
            };
            let k = TowerKey {
                radio,
                mcc: f[1].parse().unwrap_or(0),
                mnc: f[2].parse().unwrap_or(0),
                area: f[3].parse().unwrap_or(0),
                cell: f[4].parse().unwrap_or(0),
            };
            let e = if f[5] == "found" {
                Entry::Found(Position {
                    lat: f[6].parse().unwrap_or(0.0),
                    lon: f[7].parse().unwrap_or(0.0),
                    range_m: f[8].parse().unwrap_or(0.0),
                    samples: f[9].parse().unwrap_or(0),
                })
            } else {
                Entry::NotFound(f[10].parse().unwrap_or(0.0))
            };
            self.cache.insert(k, e);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(cell: u32) -> TowerKey {
        TowerKey {
            radio: Radio::Lte,
            mcc: 246,
            mnc: 1,
            area: 1234,
            cell,
        }
    }

    #[test]
    fn tower_lookups() {
        assert_eq!(percent("pk.ab+c/d"), "pk.ab%2Bc%2Fd");
        assert_eq!(
            url("pk.ab+c/d", &key(6553603)),
            "https://opencellid.org/cell/get?key=pk.ab%2Bc%2Fd&mcc=246&mnc=1&lac=1234&cellid=6553603&radio=LTE&format=json"
        );
        assert!(
            matches!(parse_answer(r#"{"lat": 54.6, "lon": 25.2, "range": 900, "samples": 12}"#), Answer::Found(p) if p.samples == 12)
        );
        assert_eq!(
            parse_answer(r#"{"error": "x", "code": 1}"#),
            Answer::NotFound
        );
        assert_eq!(parse_answer(r#"{"error": "x", "code": 2}"#), Answer::BadKey);
        assert_eq!(parse_answer(r#"{"error": "x", "code": 6}"#), Answer::Limit);

        let mut t = Towers::new("k");
        let mut calls = 0;
        let mut fetch = |u: &str| -> Option<String> {
            calls += 1;
            if u.contains("cellid=6553603") {
                Some(r#"{"lat": 54.6, "lon": 25.2, "range": 900, "samples": 12}"#.into())
            } else if u.contains("cellid=7") {
                Some(r#"{"code": 1}"#.into())
            } else {
                None
            }
        };
        t.want(key(6553603), 0.0);
        t.want(key(6553603), 0.0);
        t.want(key(7), 0.0);
        t.want(key(9), 0.0);
        assert!(t.pump(0.0, &mut fetch));
        // Rate limit: at most one a second.
        assert!(!t.pump(0.5, &mut fetch));
        assert!(t.pump(1.0, &mut fetch));
        assert!(t.pump(2.0, &mut fetch));
        assert!(!t.pump(3.0, &mut fetch));
        assert_eq!(calls, 3);
        // Found is kept; not found kept a day; errors are not stored.
        t.want(key(6553603), 10.0);
        t.want(key(7), 100.0);
        t.want(key(9), 100.0);
        assert_eq!(t.queue.len(), 1);
        t.want(key(7), 90_000.0);
        assert_eq!(t.queue.len(), 2);
        // Site fallback: another sector of the same eNB.
        let other = TowerKey {
            cell: (6553603 & !0xFF) | 2,
            ..key(0)
        };
        assert_eq!(t.position(&other).map(|p| p.1), Some("site"));
        // A tower file matches by MCC, MNC and cell; the database wins.
        let mut f = Towers::new("");
        assert_eq!(
            f.load_csv("LTE,246,1,1234,42,0,25.0,54.0,500,3,1,0,0,0\n"),
            1
        );
        assert_eq!(f.position(&key(42)).map(|p| p.1), Some("CSV"));
        assert_eq!(t.position(&key(6553603)).map(|p| p.1), Some("database"));
        // The cache survives a round trip through CSV.
        let csv = t.cache_csv();
        let mut u = Towers::new("k");
        u.load_cache_csv(&csv);
        assert_eq!(u.position(&key(6553603)).map(|p| p.1), Some("database"));
        // A bad key halts.
        let mut h = Towers::new("bad");
        h.want(key(1), 0.0);
        h.want(key(2), 0.0);
        h.pump(0.0, &mut |_: &str| Some(r#"{"code": 2}"#.to_string()));
        assert!(h.halted());
        assert!(!h.pump(5.0, &mut |_: &str| Some(String::new())));
        h.set_key("good");
        assert!(!h.halted());
        assert_eq!(Towers::sector_keys(&key(0x123401), &[1, 2, 3]).len(), 2);
    }
}
