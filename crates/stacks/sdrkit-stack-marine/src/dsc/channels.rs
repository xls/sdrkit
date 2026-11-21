//! The DSC channel plan (ITU RR Appendix 17), the scanner's visiting order and hold rules, parallel channel selection, and the
//! marine band table.

use serde::{Deserialize, Serialize};

/// A group of DSC channels.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Group {
    Distress,
    Vhf,
    WorldwideShip,
    WorldwideCoast,
    SeriesAShip,
    SeriesACoast,
    SeriesBShip,
    SeriesBCoast,
    Us455,
}

/// One DSC channel.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct DscChannel {
    pub khz: f64,
    pub group: Group,
    pub weight: u8,
}

const fn ch(khz: f64, group: Group, weight: u8) -> DscChannel {
    DscChannel { khz, group, weight }
}

/// The 59 DSC channels.
pub const CHANNELS: [DscChannel; 59] = {
    use Group::*;
    [
        ch(2187.5, Distress, 4),
        ch(4207.5, Distress, 3),
        ch(6312.0, Distress, 3),
        ch(8414.5, Distress, 4),
        ch(12577.0, Distress, 3),
        ch(16804.5, Distress, 3),
        ch(156_525.0, Vhf, 4),
        ch(2189.5, WorldwideShip, 1),
        ch(4208.0, WorldwideShip, 1),
        ch(6312.5, WorldwideShip, 1),
        ch(8415.0, WorldwideShip, 1),
        ch(12577.5, WorldwideShip, 1),
        ch(16805.0, WorldwideShip, 1),
        ch(18898.5, WorldwideShip, 1),
        ch(22374.5, WorldwideShip, 1),
        ch(25208.5, WorldwideShip, 1),
        ch(2177.0, WorldwideCoast, 1),
        ch(4219.5, WorldwideCoast, 1),
        ch(6331.0, WorldwideCoast, 1),
        ch(8436.5, WorldwideCoast, 1),
        ch(12657.0, WorldwideCoast, 1),
        ch(16903.0, WorldwideCoast, 1),
        ch(19703.5, WorldwideCoast, 1),
        ch(22444.0, WorldwideCoast, 1),
        ch(26121.0, WorldwideCoast, 1),
        ch(4208.5, SeriesAShip, 1),
        ch(6313.0, SeriesAShip, 1),
        ch(8415.5, SeriesAShip, 1),
        ch(12578.0, SeriesAShip, 1),
        ch(16805.5, SeriesAShip, 1),
        ch(18899.0, SeriesAShip, 1),
        ch(22375.0, SeriesAShip, 1),
        ch(25209.0, SeriesAShip, 1),
        ch(4220.0, SeriesACoast, 1),
        ch(6331.5, SeriesACoast, 1),
        ch(8437.0, SeriesACoast, 1),
        ch(12657.5, SeriesACoast, 1),
        ch(16903.5, SeriesACoast, 1),
        ch(19704.0, SeriesACoast, 1),
        ch(22444.5, SeriesACoast, 1),
        ch(26121.5, SeriesACoast, 1),
        ch(4209.0, SeriesBShip, 1),
        ch(6313.5, SeriesBShip, 1),
        ch(8416.0, SeriesBShip, 1),
        ch(12578.5, SeriesBShip, 1),
        ch(16806.0, SeriesBShip, 1),
        ch(18899.5, SeriesBShip, 1),
        ch(22375.5, SeriesBShip, 1),
        ch(25209.5, SeriesBShip, 1),
        ch(4220.5, SeriesBCoast, 1),
        ch(6332.0, SeriesBCoast, 1),
        ch(8437.5, SeriesBCoast, 1),
        ch(12658.0, SeriesBCoast, 1),
        ch(16904.0, SeriesBCoast, 1),
        ch(19704.5, SeriesBCoast, 1),
        ch(22445.0, SeriesBCoast, 1),
        ch(26122.0, SeriesBCoast, 1),
        ch(458.5, Us455, 1),
        ch(455.5, Us455, 1),
    ]
};

/// The scan sets named in a setting's text: "distress", "worldwide", "series_a", "series_b", "vhf". Names are read without
/// regard to case and separated by commas, semicolons, `|` or spaces. "series A", "Series-A", "seriesA" and "series_a" all name
/// series A. Unknown names are left out.
pub fn parse_scan_sets(text: &str) -> Vec<&'static str> {
    let lower = text.to_ascii_lowercase().replace(['-', '_'], " ");
    let words: Vec<&str> = lower
        .split([',', ';', '|', ' ', '\t'])
        .filter(|s| !s.is_empty())
        .collect();
    let mut out: Vec<&'static str> = Vec::new();
    let mut i = 0;
    while i < words.len() {
        let w = words[i];
        let next = words.get(i + 1).copied();
        let name = match (w, next) {
            ("series", Some("a")) => {
                i += 1;
                Some("series_a")
            }
            ("series", Some("b")) => {
                i += 1;
                Some("series_b")
            }
            ("seriesa", _) => Some("series_a"),
            ("seriesb", _) => Some("series_b"),
            ("distress", _) => Some("distress"),
            ("worldwide", _) => Some("worldwide"),
            ("vhf", _) => Some("vhf"),
            _ => None,
        };
        if let Some(n) = name {
            if !out.contains(&n) {
                out.push(n);
            }
        }
        i += 1;
    }
    out
}

/// The scan sets: "distress", "worldwide", "series_a", "series_b", "vhf".
pub fn scan_set(names: &[&str]) -> Vec<DscChannel> {
    CHANNELS
        .iter()
        .copied()
        .filter(|c| {
            names.iter().any(|n| match *n {
                "distress" => c.group == Group::Distress,
                "worldwide" => matches!(c.group, Group::WorldwideShip | Group::WorldwideCoast),
                "series_a" => matches!(c.group, Group::SeriesAShip | Group::SeriesACoast),
                "series_b" => matches!(c.group, Group::SeriesBShip | Group::SeriesBCoast),
                "vhf" => c.group == Group::Vhf,
                _ => false,
            })
        })
        .collect()
}

/// The visiting order: each channel repeated by its weight in rounds (round r holds every channel of weight > r).
pub fn visiting_order(list: &[(f64, u8)]) -> Vec<f64> {
    let max = list.iter().map(|c| c.1).max().unwrap_or(0);
    let mut v = Vec::new();
    for r in 0..max {
        for c in list {
            if c.1 > r {
                v.push(c.0);
            }
        }
    }
    v
}

/// The scanner: steps a single radio through the chosen channels.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Scanner {
    pub order: Vec<f64>,
    pub dwell_s: f64,
    pub hold_s: f64,
    pos: usize,
    slot_start: f64,
    hold_until: f64,
    locked_since: Option<f64>,
}

impl Scanner {
    /// A scanner over `list` (kHz, weight) with this dwell.
    pub fn new(list: &[(f64, u8)], dwell_s: f64) -> Self {
        Scanner {
            order: visiting_order(list),
            dwell_s: dwell_s.clamp(1.0, 30.0),
            hold_s: 20.0,
            pos: 0,
            slot_start: 0.0,
            hold_until: f64::NEG_INFINITY,
            locked_since: None,
        }
    }
    /// Starts the first slot at `t` (a scanner switched on part way through).
    pub fn start(&mut self, t: f64) {
        self.slot_start = t;
        self.hold_until = f64::NEG_INFINITY;
    }
    /// The channel to be on now (kHz).
    pub fn current(&self) -> Option<f64> {
        self.order.get(self.pos).copied()
    }
    /// A call was heard on the current channel at `t`.
    pub fn call(&mut self, t: f64) {
        self.hold_until = self.hold_until.max(t + self.hold_s);
    }
    /// Advances the clock to `t` with the decoder's lock state. Returns the new channel when the radio must move.
    pub fn tick(&mut self, t: f64, locked: bool) -> Option<f64> {
        if self.order.is_empty() {
            return None;
        }
        if locked {
            // A locked decoder keeps the radio there, and for at least a quarter of the hold time after.
            self.locked_since.get_or_insert(t);
            self.hold_until = self.hold_until.max(t + self.hold_s / 4.0);
        } else {
            self.locked_since = None;
        }
        if t < self.hold_until || t - self.slot_start < self.dwell_s {
            return None;
        }
        self.pos = (self.pos + 1) % self.order.len();
        self.slot_start = t;
        self.current()
    }
}

/// The channels of `list` (kHz) that lie inside a capture with 8 kHz of margin (|f - dial| <= rate / 2 - |offset| - 8 kHz).
pub fn in_capture(list: &[f64], dial_khz: f64, rate_hz: f64, offset_hz: f64) -> Vec<f64> {
    let half_khz = rate_hz / 2.0 / 1000.0 - offset_hz.abs() / 1000.0 - 8.0;
    list.iter()
        .copied()
        .filter(|f| (f - dial_khz).abs() <= half_khz)
        .collect()
}

/// The channels of `list` that get a parallel decoder: inside the capture and more than 500 Hz from the dial.
pub fn parallel_channels(list: &[f64], dial_khz: f64, rate_hz: f64, offset_hz: f64) -> Vec<f64> {
    in_capture(list, dial_khz, rate_hz, offset_hz)
        .into_iter()
        .filter(|f| (f - dial_khz).abs() > 0.5)
        .collect()
}

/// A row of the marine band table.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct BandRow {
    pub band: &'static str,
    pub khz: f64,
    pub what: &'static str,
    /// The stack and service to open ("marine:dsc", "marine:navtex", "ais", "vdes", "" for tune only).
    pub opens: &'static str,
}

/// The marine band table.
pub fn band_table() -> Vec<BandRow> {
    let mut v = vec![
        BandRow {
            band: "MF",
            khz: 2187.5,
            what: "DSC distress",
            opens: "marine:dsc",
        },
        BandRow {
            band: "MF",
            khz: 2177.0,
            what: "DSC coast",
            opens: "marine:dsc",
        },
        BandRow {
            band: "MF",
            khz: 2189.5,
            what: "DSC ship",
            opens: "marine:dsc",
        },
        BandRow {
            band: "MF",
            khz: 518.0,
            what: "NAVTEX international",
            opens: "marine:navtex",
        },
        BandRow {
            band: "MF",
            khz: 490.0,
            what: "NAVTEX national",
            opens: "marine:navtex",
        },
        BandRow {
            band: "MF",
            khz: 424.0,
            what: "NAVTEX (some Baltic and Turkish stations)",
            opens: "marine:navtex",
        },
        BandRow {
            band: "MF",
            khz: 2182.0,
            what: "Radiotelephony distress (voice)",
            opens: "",
        },
        BandRow {
            band: "MF",
            khz: 2174.5,
            what: "NBDP distress",
            opens: "",
        },
    ];
    let hf = [
        ("4 MHz", 4207.5, 4208.0, 4219.5, 4125.0, 4177.5),
        ("6 MHz", 6312.0, 6312.5, 6331.0, 6215.0, 6268.0),
        ("8 MHz", 8414.5, 8415.0, 8436.5, 8291.0, 8376.5),
        ("12 MHz", 12577.0, 12577.5, 12657.0, 12290.0, 12520.0),
        ("16 MHz", 16804.5, 16805.0, 16903.0, 16420.0, 16695.0),
    ];
    for (b, d, s, c, voice, nbdp) in hf {
        v.push(BandRow {
            band: b,
            khz: d,
            what: "DSC distress",
            opens: "marine:dsc",
        });
        v.push(BandRow {
            band: b,
            khz: s,
            what: "DSC calling, ship",
            opens: "marine:dsc",
        });
        v.push(BandRow {
            band: b,
            khz: c,
            what: "DSC calling, coast",
            opens: "marine:dsc",
        });
        v.push(BandRow {
            band: b,
            khz: voice,
            what: "Voice distress",
            opens: "",
        });
        v.push(BandRow {
            band: b,
            khz: nbdp,
            what: "NBDP distress",
            opens: "",
        });
    }
    v.push(BandRow {
        band: "4 MHz",
        khz: 4209.5,
        what: "NAVTEX HF",
        opens: "marine:navtex",
    });
    v.push(BandRow {
        band: "HF",
        khz: 0.0,
        what: "Weather fax (station directory)",
        opens: "marine:wefax",
    });
    v.push(BandRow {
        band: "VHF",
        khz: 156_525.0,
        what: "DSC channel 70",
        opens: "marine:dsc",
    });
    v.push(BandRow {
        band: "VHF",
        khz: 156_800.0,
        what: "Channel 16 (voice)",
        opens: "marinevhf",
    });
    v.push(BandRow {
        band: "VHF",
        khz: 161_975.0,
        what: "AIS 1",
        opens: "ais",
    });
    v.push(BandRow {
        band: "VHF",
        khz: 162_025.0,
        what: "AIS 2",
        opens: "ais",
    });
    v.push(BandRow {
        band: "VHF",
        khz: 161_950.0,
        what: "VDES",
        opens: "vdes",
    });
    v.push(BandRow {
        band: "UHF",
        khz: 406_037.0,
        what: "EPIRB beacons",
        opens: "",
    });
    v.push(BandRow {
        band: "VHF",
        khz: 121_500.0,
        what: "121.5 MHz homing",
        opens: "",
    });
    v.push(BandRow {
        band: "L",
        khz: 1_537_100.0,
        what: "Inmarsat-C",
        opens: "",
    });
    v
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn channel_list_facts() {
        let mut v: Vec<i64> = CHANNELS.iter().map(|c| (c.khz * 10.0) as i64).collect();
        v.sort();
        v.dedup();
        assert_eq!(v.len(), 59);
        assert_eq!(
            CHANNELS
                .iter()
                .filter(|c| c.group == Group::Distress)
                .count(),
            6
        );
        for f in [2177.0, 4208.0, 25208.5, 4209.0] {
            assert!(CHANNELS.iter().any(|c| c.khz == f), "{f}");
        }
        assert!(
            band_table()
                .iter()
                .filter(|r| r.opens == "marine:dsc")
                .count()
                >= 15
        );
    }

    #[test]
    fn scan_set_names() {
        assert_eq!(parse_scan_sets("distress"), vec!["distress"]);
        assert_eq!(
            parse_scan_sets("Distress, series A; Series-B|VHF worldwide"),
            vec!["distress", "series_a", "series_b", "vhf", "worldwide"]
        );
        assert_eq!(
            parse_scan_sets("series_a seriesB"),
            vec!["series_a", "series_b"]
        );
        assert_eq!(parse_scan_sets("nonsense, series"), Vec::<&str>::new());
        assert_eq!(scan_set(&parse_scan_sets("Series A")).len(), 16);
        assert_eq!(scan_set(&parse_scan_sets("distress")).len(), 6);
        assert_eq!(scan_set(&parse_scan_sets("VHF")).len(), 1);
    }

    #[test]
    fn scanner_order_and_hold() {
        let list = [(2187.5, 2), (8414.5, 1), (12577.0, 1)];
        assert_eq!(visiting_order(&list), vec![2187.5, 8414.5, 12577.0, 2187.5]);
        let mut s = Scanner::new(&list, 4.0);
        let mut moves = 0;
        let mut seq = vec![s.current().unwrap()];
        let mut t = 0.0;
        while t < 40.0 {
            t += 0.25;
            if let Some(c) = s.tick(t, false) {
                moves += 1;
                seq.push(c);
            }
        }
        assert!(moves >= 9);
        assert_eq!(
            &seq[..6],
            &[2187.5, 8414.5, 12577.0, 2187.5, 2187.5, 8414.5]
        );
        // A decoder that stays locked keeps the radio on its channel for 30 s.
        let mut s = Scanner::new(&list, 4.0);
        let mut t = 0.0;
        let mut moved = false;
        while t < 30.0 {
            t += 0.25;
            moved |= s.tick(t, true).is_some();
        }
        assert!(!moved);
        // A call keeps it for the hold.
        let mut s = Scanner::new(&list, 4.0);
        s.call(1.0);
        let mut t = 1.0;
        let mut first_move = None;
        while t < 40.0 {
            t += 0.25;
            if s.tick(t, false).is_some() && first_move.is_none() {
                first_move = Some(t);
            }
        }
        assert!(first_move.unwrap() >= 21.0);
        // Of 4207.5, 8414.5, 6312 and 4208, a 100 kHz capture centred on 4208 holds two.
        let p = in_capture(&[4207.5, 8414.5, 6312.0, 4208.0], 4208.0, 100e3, 0.0);
        assert_eq!(p, vec![4207.5, 4208.0]);
        // One channel 5 kHz away and one 5 MHz away at 250 ksps: one parallel channel.
        assert_eq!(
            parallel_channels(&[2192.5, 7187.5], 2187.5, 250e3, 20e3),
            vec![2192.5]
        );
    }
}
