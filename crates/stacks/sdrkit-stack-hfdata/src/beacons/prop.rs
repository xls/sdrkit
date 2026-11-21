//! The propagation dashboard: this receiver's own observations per amateur band in 15-minute bins over 24 hours,
//! a score per band, and the band to try now. Nothing is downloaded.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// The amateur bands: name and MHz range (each widened by 5 kHz when an observation is placed).
pub const BANDS: &[(&str, f64, f64)] = &[
    ("160 m", 1.800, 2.000),
    ("80 m", 3.500, 3.800),
    ("60 m", 5.250, 5.450),
    ("40 m", 7.000, 7.200),
    ("30 m", 10.100, 10.150),
    ("20 m", 14.000, 14.350),
    ("17 m", 18.068, 18.168),
    ("15 m", 21.000, 21.450),
    ("12 m", 24.890, 24.990),
    ("10 m", 28.000, 29.700),
    ("6 m", 50.000, 54.000),
];

/// Where an observation came from.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Source {
    Ncdxf = 0,
    Letters = 1,
    Dashes = 2,
    TimeSignal = 3,
    Wspr = 4,
    Ft8 = 5,
    Ft4 = 6,
    Ft2 = 7,
}

/// A station heard.
#[derive(Clone, Debug, PartialEq)]
pub struct Observation {
    pub utc_s: f64,
    pub mhz: f64,
    pub source: Source,
    pub id: String,
    pub snr_db: f64,
    pub km: Option<f64>,
}

#[derive(Clone, Debug, Default, PartialEq)]
struct Bin {
    count: u32,
    ids: Vec<String>,
    stand_ins: u32,
    best: Option<f64>,
    far: Option<f64>,
    sources: [u32; 8],
}

impl Bin {
    fn stations(&self) -> u32 {
        self.ids.len() as u32 + self.stand_ins
    }
}

sdrkit_core::choice_enum! {
    /// Whether a band is open.
    #[derive(Serialize, Deserialize)]
    pub enum BandState {
        /// Three stations or more and a score of 0.4 or more.
        Open = ("open", "open"),
        /// Some stations.
        Marginal = ("marginal", "marginal"),
        /// No station.
        Closed = ("closed", "closed"),
    }
}

/// The band to try now.
#[derive(Clone, Debug, PartialEq)]
pub struct BestBand {
    /// The band name.
    pub band: String,
    /// Distinct stations in 30 minutes.
    pub stations: u32,
    /// The best SNR, dB.
    pub best_snr: Option<f64>,
    /// The farthest station, km.
    pub farthest_km: Option<f64>,
}

/// A band's state now.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct BandNow {
    pub band: String,
    pub stations: u32,
    pub best_snr: Option<f64>,
    pub farthest_km: Option<f64>,
    pub score: f64,
    /// Whether the band is open.
    pub state: BandState,
    pub sources: [u32; 8],
}

/// The model: per band, bins keyed by their start (UTC seconds, a multiple of 900).
#[derive(Clone, Debug, Default)]
pub struct PropModel {
    bins: Vec<BTreeMap<i64, Bin>>,
}

/// The band index of a frequency.
pub fn band_of(mhz: f64) -> Option<usize> {
    BANDS
        .iter()
        .position(|b| mhz >= b.1 - 0.005 && mhz <= b.2 + 0.005)
}

impl PropModel {
    fn ensure(&mut self) {
        if self.bins.len() != BANDS.len() {
            self.bins = vec![BTreeMap::new(); BANDS.len()];
        }
    }
    /// Adds an observation.
    pub fn add(&mut self, o: &Observation) {
        self.ensure();
        let Some(b) = band_of(o.mhz) else {
            return;
        };
        let start = (o.utc_s / 900.0).floor() as i64 * 900;
        let bin = self.bins[b].entry(start).or_default();
        bin.count += 1;
        if !bin.ids.contains(&o.id) && bin.ids.len() < 400 {
            bin.ids.push(o.id.clone());
        }
        if o.snr_db.is_finite() {
            bin.best = Some(bin.best.map_or(o.snr_db, |x| x.max(o.snr_db)));
        }
        if let Some(k) = o.km {
            bin.far = Some(bin.far.map_or(k, |x| x.max(k)));
        }
        bin.sources[o.source as usize] += 1;
        // 96 bins of 15 minutes.
        while self.bins[b].len() > 96 {
            self.bins[b].pop_first();
        }
    }
    /// Every band's state at `now` (bins overlapping the last 30 minutes).
    pub fn now(&mut self, now: f64) -> Vec<BandNow> {
        self.ensure();
        let mut v = Vec::new();
        for (bi, band) in BANDS.iter().enumerate() {
            let mut ids: Vec<String> = Vec::new();
            let mut stand = 0;
            let (mut best, mut far) = (None::<f64>, None::<f64>);
            let mut sources = [0u32; 8];
            for (&s, bin) in &self.bins[bi] {
                let s = s as f64;
                if s + 900.0 > now - 1800.0 && s <= now && now - s <= 86_400.0 {
                    for id in &bin.ids {
                        if !ids.contains(id) {
                            ids.push(id.clone());
                        }
                    }
                    stand += bin.stand_ins;
                    if let Some(b) = bin.best {
                        best = Some(best.map_or(b, |x| x.max(b)));
                    }
                    if let Some(f) = bin.far {
                        far = Some(far.map_or(f, |x| x.max(f)));
                    }
                    for (s, b) in sources.iter_mut().zip(bin.sources) {
                        *s += b;
                    }
                }
            }
            let stations = ids.len() as u32 + stand;
            let score = if stations == 0 {
                0.0
            } else {
                0.5 * (stations as f64 / 8.0).min(1.0)
                    + 0.3 * ((best.unwrap_or(-20.0) + 20.0) / 30.0).clamp(0.0, 1.0)
                    + 0.2 * (far.unwrap_or(0.0) / 10_000.0).min(1.0)
            };
            let state = if stations >= 3 && score >= 0.4 {
                BandState::Open
            } else if stations > 0 {
                BandState::Marginal
            } else {
                BandState::Closed
            };
            v.push(BandNow {
                band: band.0.into(),
                stations,
                best_snr: best,
                farthest_km: far,
                score,
                state,
                sources,
            });
        }
        v
    }
    /// The band to try now, or `None` when nothing was heard in the last 30 minutes.
    pub fn best(&mut self, now: f64) -> Option<BestBand> {
        let bands = self.now(now);
        bands
            .into_iter()
            .filter(|b| b.state != BandState::Closed)
            .max_by(|a, b| {
                a.score
                    .partial_cmp(&b.score)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .map(|b| BestBand {
                band: b.band,
                stations: b.stations,
                best_snr: b.best_snr,
                farthest_km: b.farthest_km,
            })
    }
    /// The model as text: "hfprop 1", then one line per bin.
    pub fn save(&self, _now: f64) -> String {
        let mut s = String::from("hfprop 1\n");
        for (bi, m) in self.bins.iter().enumerate() {
            for (start, b) in m {
                s.push_str(&format!(
                    "{} {} {} {} {} {}",
                    BANDS[bi].0.replace(' ', ""),
                    start,
                    b.count,
                    b.stations(),
                    b.best.map_or("-".into(), |x| format!("{x:.1}")),
                    b.far.map_or("-".into(), |x| format!("{x:.0}")),
                ));
                for c in b.sources {
                    s.push_str(&format!(" {c}"));
                }
                s.push('\n');
            }
        }
        s
    }
    /// Loads a saved model, skipping damaged lines and bins older than 24 hours or in the future; false when it is not one.
    pub fn load(&mut self, text: &str, now: f64) -> bool {
        let mut lines = text.lines();
        if lines.next().map(str::trim) != Some("hfprop 1") {
            return false;
        }
        self.bins = vec![BTreeMap::new(); BANDS.len()];
        for l in lines {
            let f: Vec<&str> = l.split_whitespace().collect();
            if f.len() != 14 {
                continue;
            }
            let Some(bi) = BANDS.iter().position(|b| b.0.replace(' ', "") == f[0]) else {
                continue;
            };
            let (Ok(start), Ok(count), Ok(st)) = (
                f[1].parse::<i64>(),
                f[2].parse::<u32>(),
                f[3].parse::<u32>(),
            ) else {
                continue;
            };
            if (start as f64) > now || now - start as f64 > 86_400.0 {
                continue;
            }
            let best = f[4].parse::<f64>().ok();
            let far = f[5].parse::<f64>().ok();
            let mut sources = [0u32; 8];
            let mut ok = true;
            for k in 0..8 {
                match f[6 + k].parse() {
                    Ok(v) => sources[k] = v,
                    Err(_) => ok = false,
                }
            }
            if !ok {
                continue;
            }
            self.bins[bi].insert(
                start,
                Bin {
                    count,
                    ids: Vec::new(),
                    stand_ins: st,
                    best,
                    far,
                    sources,
                },
            );
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn b11_propagation_facts() {
        let t0 = 1_700_000_000.0;
        let mut m = PropModel::default();
        for i in 0..6 {
            m.add(&Observation {
                utc_s: t0 - 600.0 + i as f64 * 60.0,
                mhz: 14.074,
                source: Source::Ft8,
                id: format!("ST{i}"),
                snr_db: -10.0,
                km: Some(9000.0),
            });
        }
        m.add(&Observation {
            utc_s: t0 - 300.0,
            mhz: 7.074,
            source: Source::Ft8,
            id: "X".into(),
            snr_db: -15.0,
            km: Some(500.0),
        });
        let now = m.now(t0);
        let b20 = now.iter().find(|b| b.band == "20 m").unwrap();
        assert_eq!(
            (b20.state, b20.stations, b20.farthest_km),
            (BandState::Open, 6, Some(9000.0))
        );
        assert_eq!(b20.sources[Source::Ft8 as usize], 6);
        assert_eq!(
            now.iter().find(|b| b.band == "40 m").unwrap().state,
            BandState::Marginal
        );
        let b10 = now.iter().find(|b| b.band == "10 m").unwrap();
        assert_eq!((b10.state, b10.score), (BandState::Closed, 0.0));
        let best = m.best(t0).unwrap();
        assert_eq!(best.band, "20 m");
        assert_eq!(best.stations, 6);
        assert_eq!(
            m.now(t0 + 3000.0)
                .iter()
                .find(|b| b.band == "20 m")
                .unwrap()
                .state,
            BandState::Closed
        );
        assert!(m.best(t0 + 3000.0).is_none());
        let mut m2 = PropModel::default();
        for i in 0..3 {
            m2.add(&Observation {
                utc_s: t0,
                mhz: 21.150,
                source: Source::Ncdxf,
                id: format!("B{i}"),
                snr_db: 12.0,
                km: Some(8000.0),
            });
        }
        assert_eq!(
            m2.now(t0).iter().find(|b| b.band == "15 m").unwrap().state,
            BandState::Open
        );
        // Save and reload: the counts stay, damaged lines are skipped.
        let text = m.save(t0) + "garbage line\n";
        let mut m3 = PropModel::default();
        assert!(m3.load(&text, t0));
        assert_eq!(
            m3.now(t0)
                .iter()
                .find(|b| b.band == "20 m")
                .unwrap()
                .stations,
            6
        );
        assert!(!m3.load("nonsense", t0));
    }
}
