//! The station health check: one sweep over a few fixed bands, the noise floor and the signals of each, and plain advice about the
//! antenna and the radio.

use crate::floor::hop_floor_db;
use crate::grid::{Grid, UNCOVERED_DB};
use crate::profile::{Band, ScanProfile};
use crate::schedule::Schedule;

/// A band the check measures.
#[derive(Clone, Debug, PartialEq)]
pub struct HealthBand {
    /// Its name.
    pub name: &'static str,
    /// Low edge, Hz.
    pub low_hz: f64,
    /// High edge, Hz.
    pub high_hz: f64,
    /// Strong stations are expected here (FM broadcast).
    pub expect_stations: bool,
}

/// The bands checked, those a radio tuning `min_hz .. max_hz` can reach.
pub fn default_bands(min_hz: f64, max_hz: f64) -> Vec<HealthBand> {
    let all = [
        ("40 m", 7.0e6, 7.3e6, false),
        ("FM broadcast", 88e6, 108e6, true),
        ("Airband", 118e6, 137e6, false),
        ("2 m", 144e6, 146e6, false),
        ("70 cm", 430e6, 440e6, false),
        ("ADS-B", 1085e6, 1095e6, false),
    ];
    all.iter()
        .filter(|b| b.1 >= min_hz && b.2 <= max_hz)
        .map(|&(name, lo, hi, st)| HealthBand {
            name,
            low_hz: lo,
            high_hz: hi,
            expect_stations: st,
        })
        .collect()
}

/// Makes `p` the profile that sweeps `bands` once.
pub fn profile(p: &mut ScanProfile, bands: &[HealthBand]) {
    p.name = "Station health";
    p.bands = bands
        .iter()
        .map(|b| Band {
            label: b.name,
            low_hz: b.low_hz,
            high_hz: b.high_hz,
        })
        .collect();
    p.bin_hz = 25e3;
    p.averages = 8;
    p.schedule = Schedule::once();
    p.min_sweeps = 1;
}

sdrkit_core::choice_enum! {
    /// How a result reads, from the best to the worst.
    pub enum Level {
        /// Fine.
        Good = ("good", "Good"),
        /// Worth a look.
        Warning = ("warning", "Warning"),
        /// Needs fixing.
        Problem = ("problem", "Problem"),
    }
}

/// One piece of advice.
#[derive(Clone, Debug, PartialEq)]
pub struct Advice {
    /// How serious.
    pub level: Level,
    /// What to do, in plain words.
    pub text: String,
}

/// One band's measurement.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BandHealth {
    /// Its name.
    pub name: &'static str,
    /// The noise floor, dBFS.
    pub floor_db: f32,
    /// The strongest bin, dBFS.
    pub peak_db: f32,
    /// Signals 10 dB or more over the floor.
    pub signals: usize,
}

/// The check's result.
#[derive(Clone, Debug, PartialEq)]
pub struct HealthReport {
    /// Per band.
    pub bands: Vec<BandHealth>,
    /// The advice, worst first.
    pub advice: Vec<Advice>,
    /// The worst level.
    pub overall: Level,
}

/// Measures the bands in a stitched sweep and returns advice.
pub fn assess(grid: &Grid, sweep_db: &[f32], bands: &[HealthBand]) -> HealthReport {
    let mut out = Vec::new();
    let mut scratch = Vec::new();
    for b in bands {
        let r = grid.bins_between(b.low_hz, b.high_hz);
        let v = &sweep_db[r.start.min(sweep_db.len())..r.end.min(sweep_db.len())];
        if v.iter().all(|&x| x <= UNCOVERED_DB) {
            continue;
        }
        let floor = hop_floor_db(v, &mut scratch);
        let peak = v.iter().copied().fold(UNCOVERED_DB, f32::max);
        let mut signals = 0;
        let mut inside = false;
        for &x in v {
            let on = x > floor + 10.0;
            if on && !inside {
                signals += 1;
            }
            inside = on;
        }
        out.push((
            b,
            BandHealth {
                name: b.name,
                floor_db: floor,
                peak_db: peak,
                signals,
            },
        ));
    }
    let mut advice = Vec::new();
    if out.is_empty() {
        advice.push(Advice {
            level: Level::Problem,
            text: "None of the check's bands could be measured: the radio's range or the sweep did not cover them.".into(),
        });
    } else {
        let lowest = out
            .iter()
            .map(|(_, h)| h.floor_db)
            .fold(f32::INFINITY, f32::min);
        let highest = out
            .iter()
            .map(|(_, h)| h.floor_db)
            .fold(f32::NEG_INFINITY, f32::max);
        let total: usize = out.iter().map(|(_, h)| h.signals).sum();
        if out.iter().any(|(_, h)| h.peak_db >= -3.0) {
            advice.push(Advice {
                level: Level::Problem,
                text: "The radio is overloaded: a signal reaches full scale. Lower the gain, or add a filter for the strong band.".into(),
            });
        }
        if total == 0 && highest - lowest < 3.0 {
            advice.push(Advice {
                level: Level::Problem,
                text: "Nothing is heard on any band and the noise floor is the same everywhere: the antenna is probably not connected, its cable is broken, or the amplifier's power (bias tee) is off.".into(),
            });
        } else {
            for (b, h) in &out {
                if b.expect_stations && h.signals == 0 {
                    advice.push(Advice {
                        level: Level::Warning,
                        text: format!("No stations on {}: the antenna may be poor at these frequencies, or the transmitters are far away.", h.name),
                    });
                }
                if b.expect_stations && h.peak_db > -15.0 {
                    advice.push(Advice {
                        level: Level::Warning,
                        text: format!("Very strong signals on {}: a band-stop filter for it would stop them overloading the radio.", h.name),
                    });
                }
                if !b.expect_stations && h.floor_db - lowest >= 10.0 {
                    advice.push(Advice {
                        level: Level::Warning,
                        text: format!(
                            "The noise floor on {} is {:.0} dB above the quietest band: local interference (power supplies, LED lights, computers) near the antenna, or an amplifier that oscillates.",
                            h.name,
                            h.floor_db - lowest
                        ),
                    });
                }
            }
        }
    }
    if advice.is_empty() {
        advice.push(Advice {
            level: Level::Good,
            text: "The antenna and the radio look healthy.".into(),
        });
    }
    advice.sort_by_key(|a| std::cmp::Reverse(a.level as u8));
    let overall = advice
        .iter()
        .map(|a| a.level)
        .max_by_key(|l| *l as u8)
        .unwrap_or(Level::Good);
    HealthReport {
        bands: out.into_iter().map(|(_, h)| h).collect(),
        advice,
        overall,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setup() -> (Grid, Vec<HealthBand>) {
        let bands = default_bands(24e6, 1766e6);
        assert_eq!(bands.len(), 5, "40 m is out of range");
        let g = Grid::new(88e6, 1095e6, 100e3).unwrap();
        (g, bands)
    }

    fn base(g: &Grid) -> Vec<f32> {
        let mut r = crate::fake::Rng::new(1);
        (0..g.bins).map(|_| -90.0 + r.gauss() as f32).collect()
    }

    #[test]
    fn healthy() {
        let (g, bands) = setup();
        let mut v = base(&g);
        for f in [90e6, 95e6, 101e6] {
            v[g.index_of(f).unwrap()] = -40.0;
        }
        let r = assess(&g, &v, &bands);
        assert_eq!(r.overall, Level::Good, "{:?}", r.advice);
        assert_eq!(r.bands[0].signals, 3);
        let mut p = crate::profile::tests::profile(1e6, 2e6, 1e3);
        profile(&mut p, &bands);
        assert!(p.validate().is_ok());
    }

    #[test]
    fn disconnected_overloaded_and_noisy() {
        let (g, bands) = setup();
        let r = assess(&g, &base(&g), &bands);
        assert_eq!(r.overall, Level::Problem);
        assert!(r.advice[0]
            .text
            .contains("antenna is probably not connected"));
        let mut v = base(&g);
        v[g.index_of(98e6).unwrap()] = -1.0;
        let r = assess(&g, &v, &bands);
        assert!(r.advice.iter().any(|a| a.text.contains("overloaded")));
        assert!(r.advice.iter().any(|a| a.text.contains("band-stop")));
        let mut v = base(&g);
        v[g.index_of(98e6).unwrap()] = -40.0;
        for k in g.bins_between(430e6, 440e6) {
            v[k] += 15.0;
        }
        let r = assess(&g, &v, &bands);
        assert_eq!(r.overall, Level::Warning);
        assert!(r.advice.iter().any(|a| a.text.contains("70 cm")));
    }
}
