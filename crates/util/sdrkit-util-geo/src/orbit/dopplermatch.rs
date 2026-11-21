//! Naming a carrier by its Doppler curve. A measured frequency track (times and frequencies, in any offset) is compared with
//! the Doppler shift that SGP4 predicts for every satellite of a catalogue, seen from one observer. The transmit frequency is
//! not known, so each satellite gets the constant offset that fits best (the mean difference); the satellites are ranked by
//! the rms of what is left. The best one is named when its rms is small, the track has enough Doppler of its own, and no
//! other satellite fits about as well.
//!
//! Document: Spacetrack Report 3 and Vallado et al. (2006) for SGP4 ([`super::sgp4`]). The rules of the ranking and the
//! verdict are this module's own; every number is a field of [`MatchParams`].

use super::tle::{Checksum, Tle};
use super::Satellite;
use crate::Observer;

/// The rules of [`match_track`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MatchParams {
    /// The least points of a track.
    pub min_points: usize,
    /// The most points compared, taken evenly over the track.
    pub max_points: usize,
    /// A satellite below this elevation at any compared point is skipped, degrees.
    pub min_elevation_deg: f64,
    /// A satellite whose fitted offset is larger is skipped, Hz.
    pub max_offset_hz: f64,
    /// Satellites kept in the ranking.
    pub keep: usize,
    /// No answer when the best rms is larger, Hz.
    pub believable_rms_hz: f64,
    /// No answer when the track's own spread is under this many times the best rms ...
    pub little_doppler_factor: f64,
    /// ... with the best rms taken as at least this, Hz.
    pub little_doppler_floor_hz: f64,
    /// Confident when the second rms is over this many times the best ...
    pub confident_factor: f64,
    /// Ambiguous when the second rms is under this many times the best ...
    pub ambiguous_factor: f64,
    /// ... with the best rms taken as at least this in both rules, Hz.
    pub rms_floor_hz: f64,
    /// Skip satellites of deep-space orbits (period of 225 minutes or more).
    pub skip_deep_space: bool,
}

impl Default for MatchParams {
    /// 6 points or more, 48 compared, −2° elevation, offsets up to 25 kHz, 8 kept, believable to 60 Hz rms, enough Doppler
    /// at 3 × max(1 Hz, best), confident when the second is over 3 × max(3 Hz, best), ambiguous under 1.5 × that,
    /// near-earth orbits only.
    fn default() -> Self {
        MatchParams {
            min_points: 6,
            max_points: 48,
            min_elevation_deg: -2.0,
            max_offset_hz: 25e3,
            keep: 8,
            believable_rms_hz: 60.0,
            little_doppler_factor: 3.0,
            little_doppler_floor_hz: 1.0,
            confident_factor: 3.0,
            ambiguous_factor: 1.5,
            rms_floor_hz: 3.0,
            skip_deep_space: true,
        }
    }
}

/// One ranked satellite.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Candidate {
    /// Its index in the catalogue.
    pub index: usize,
    /// The fitted offset: measured minus predicted, Hz.
    pub offset_hz: f64,
    /// The rms of the residual after the offset, Hz.
    pub rms_hz: f64,
}

/// Why a track is not named.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NoAnswer {
    /// The track has fewer points than [`MatchParams::min_points`].
    TooFewPoints,
    /// The catalogue is empty.
    EmptyCatalogue,
    /// No satellite was above the elevation limit with an offset in range.
    NoneVisible,
    /// The best rms is over [`MatchParams::believable_rms_hz`].
    NotBelievable,
    /// The track's own spread is too small to tell satellites apart.
    TooLittleDoppler,
}

/// The verdict of a match.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Verdict {
    /// Not named.
    None(NoAnswer),
    /// The best candidate is named.
    Named {
        /// The second fits much worse.
        confident: bool,
        /// The second fits about as well (satellites flying together).
        ambiguous: bool,
    },
}

/// What [`match_track`] found.
#[derive(Clone, Debug, PartialEq)]
pub struct Match {
    /// The best satellites, best first, at most [`MatchParams::keep`].
    pub ranked: Vec<Candidate>,
    /// The verdict.
    pub verdict: Verdict,
    /// The rms of the compared measurements about their mean, Hz.
    pub spread_hz: f64,
}

impl Match {
    /// The named satellite, when the verdict names one.
    pub fn named(&self) -> Option<&Candidate> {
        match self.verdict {
            Verdict::Named { .. } => self.ranked.first(),
            Verdict::None(_) => None,
        }
    }

    /// Whether a satellite is named and the second fits much worse.
    pub fn confident(&self) -> bool {
        matches!(
            self.verdict,
            Verdict::Named {
                confident: true,
                ..
            }
        )
    }

    /// Whether a satellite is named and the second fits about as well.
    pub fn ambiguous(&self) -> bool {
        matches!(
            self.verdict,
            Verdict::Named {
                ambiguous: true,
                ..
            }
        )
    }
}

/// The indices of at most `m` points taken evenly over `n`, first and last included.
fn even(n: usize, m: usize) -> impl Iterator<Item = usize> {
    let m = m.max(2).min(n);
    (0..m).map(move |i| {
        if m < 2 {
            0
        } else {
            ((i as f64 * (n - 1) as f64 / (m - 1) as f64).round() as usize).min(n - 1)
        }
    })
}

/// Matches the track `points` ((Unix time UTC, measured frequency Hz), in time order; the frequencies in any constant
/// offset) of a carrier near `carrier_hz` against `catalogue` seen from `observer`, with the rules `p`.
pub fn match_track(
    points: &[(f64, f64)],
    carrier_hz: f64,
    observer: &Observer,
    catalogue: &[Satellite],
    p: &MatchParams,
) -> Match {
    let none = |why, spread_hz| Match {
        ranked: Vec::new(),
        verdict: Verdict::None(why),
        spread_hz,
    };
    if points.len() < p.min_points.max(2) {
        return none(NoAnswer::TooFewPoints, 0.0);
    }
    let pts: Vec<(f64, f64)> = even(points.len(), p.max_points)
        .map(|i| points[i])
        .collect();
    let n = pts.len() as f64;
    let mean = pts.iter().map(|q| q.1).sum::<f64>() / n;
    let spread_hz = (pts.iter().map(|q| (q.1 - mean).powi(2)).sum::<f64>() / n).sqrt();
    if catalogue.is_empty() {
        return none(NoAnswer::EmptyCatalogue, spread_hz);
    }
    let mut ranked = Vec::new();
    let mut diff = Vec::with_capacity(pts.len());
    'sat: for (index, sat) in catalogue.iter().enumerate() {
        if p.skip_deep_space && sat.sgp4().is_deep_space() {
            continue;
        }
        diff.clear();
        for &(t, f) in &pts {
            let Ok(look) = sat.look(observer, t) else {
                continue 'sat;
            };
            if look.elevation_deg < p.min_elevation_deg {
                continue 'sat;
            }
            diff.push(f - super::doppler_hz(carrier_hz, look.range_rate_m_s));
        }
        let offset_hz = diff.iter().sum::<f64>() / n;
        if offset_hz.abs() > p.max_offset_hz {
            continue;
        }
        let rms_hz = (diff.iter().map(|d| (d - offset_hz).powi(2)).sum::<f64>() / n).sqrt();
        ranked.push(Candidate {
            index,
            offset_hz,
            rms_hz,
        });
    }
    ranked.sort_by(|a, b| a.rms_hz.total_cmp(&b.rms_hz).then(a.index.cmp(&b.index)));
    ranked.truncate(p.keep.max(1));
    let Some(best) = ranked.first().copied() else {
        return none(NoAnswer::NoneVisible, spread_hz);
    };
    let verdict = if best.rms_hz > p.believable_rms_hz {
        Verdict::None(NoAnswer::NotBelievable)
    } else if spread_hz < p.little_doppler_factor * best.rms_hz.max(p.little_doppler_floor_hz) {
        Verdict::None(NoAnswer::TooLittleDoppler)
    } else {
        let floor = best.rms_hz.max(p.rms_floor_hz);
        match ranked.get(1) {
            None => Verdict::Named {
                confident: true,
                ambiguous: false,
            },
            Some(second) => Verdict::Named {
                confident: second.rms_hz > p.confident_factor * floor,
                ambiguous: second.rms_hz < p.ambiguous_factor * floor,
            },
        }
    };
    Match {
        ranked,
        verdict,
        spread_hz,
    }
}

/// The element sets of a text of many TLEs (each two lines, or three with a name line first; blank lines ignored), with
/// their checksums checked, and the number of sets that could not be read.
pub fn read_catalogue(text: &str) -> (Vec<Tle>, usize) {
    let lines: Vec<&str> = text
        .lines()
        .map(str::trim_end)
        .filter(|l| !l.trim().is_empty())
        .collect();
    let mut out = Vec::new();
    let mut bad = 0;
    let mut i = 0;
    while i < lines.len() {
        let is = |k: usize, c: u8| {
            lines
                .get(k)
                .is_some_and(|l| l.as_bytes().first() == Some(&c))
        };
        if is(i, b'1') && is(i + 1, b'2') {
            match Tle::from_lines(None, lines[i], lines[i + 1], Checksum::Require) {
                Ok(t) => out.push(t),
                Err(_) => bad += 1,
            }
            i += 2;
        } else if is(i + 1, b'1') && is(i + 2, b'2') {
            match Tle::from_lines(
                Some(lines[i]),
                lines[i + 1],
                lines[i + 2],
                Checksum::Require,
            ) {
                Ok(t) => out.push(t),
                Err(_) => bad += 1,
            }
            i += 3;
        } else {
            bad += 1;
            i += 1;
        }
    }
    (out, bad)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::orbit::sgp4::{Elements, Gravity, OpsMode, Sgp4};
    use crate::Position;

    /// 2026-01-01 00:00 UTC.
    const EPOCH_UNIX: f64 = 1_767_225_600.0;

    fn sat(incl_deg: f64, raan_deg: f64, ma_deg: f64, rev_day: f64) -> Satellite {
        let el = Elements {
            epoch_days_1950: 7306.0 + EPOCH_UNIX / 86_400.0,
            bstar: 0.0,
            eccentricity: 0.001,
            arg_perigee_rad: 0.0,
            inclination_rad: incl_deg.to_radians(),
            mean_anomaly_rad: ma_deg.to_radians(),
            mean_motion_rad_min: rev_day * std::f64::consts::TAU / 1440.0,
            raan_rad: raan_deg.to_radians(),
        };
        Satellite::new(Sgp4::new(&el, Gravity::Wgs72, OpsMode::Improved).unwrap())
    }

    fn track(s: &Satellite, obs: &Observer, t0: f64, f: f64, offset: f64) -> Vec<(f64, f64)> {
        (0..120)
            .map(|k| {
                let t = t0 + k as f64;
                (t, offset + s.doppler_hz(obs, t, f).unwrap())
            })
            .collect()
    }

    #[test]
    fn a_pass_is_named_with_its_offset() {
        let obs = Observer::new(Position::new(-33.87, 151.21));
        let cat: Vec<Satellite> = (0..40)
            .map(|i| sat(53.0 + i as f64, 9.0 * i as f64, 7.0 * i as f64, 15.0))
            .collect();
        let target = cat
            .iter()
            .enumerate()
            .find_map(|(i, s)| {
                s.next_pass(&obs, EPOCH_UNIX, EPOCH_UNIX + 86_400.0, 20.0)
                    .ok()
                    .flatten()
                    .map(|p| (i, p))
            })
            .expect("a pass within a day");
        let (i, pass) = target;
        let pts = track(&cat[i], &obs, pass.max_unix - 60.0, 437e6, 1234.0);
        let m = match_track(&pts, 437e6, &obs, &cat, &MatchParams::default());
        let best = m.named().expect("named");
        assert_eq!(best.index, i);
        assert!((best.offset_hz - 1234.0).abs() < 1e-3, "{m:?}");
        assert!(m.confident(), "{m:?}");
    }

    #[test]
    fn no_answer_without_points_catalogue_or_doppler() {
        let obs = Observer::new(Position::new(0.0, 0.0));
        let p = MatchParams::default();
        let cat = vec![sat(51.6, 0.0, 0.0, 15.5)];
        assert_eq!(
            match_track(&[(EPOCH_UNIX, 0.0); 3], 1e8, &obs, &cat, &p).verdict,
            Verdict::None(NoAnswer::TooFewPoints)
        );
        let fixed: Vec<(f64, f64)> = (0..50).map(|k| (EPOCH_UNIX + k as f64, 5.0)).collect();
        assert_eq!(
            match_track(&fixed, 1e8, &obs, &[], &p).verdict,
            Verdict::None(NoAnswer::EmptyCatalogue)
        );
        let m = match_track(&fixed, 1e8, &obs, &cat, &p);
        assert!(m.named().is_none(), "{m:?}");
    }

    #[test]
    fn the_even_points_hold_the_ends() {
        let v: Vec<usize> = even(100, 5).collect();
        assert_eq!(v, vec![0, 25, 50, 74, 99]);
        assert_eq!(even(3, 48).collect::<Vec<_>>(), vec![0, 1, 2]);
    }

    #[test]
    fn a_catalogue_text_is_read() {
        let iss = "ISS (ZARYA)
1 25544U 98067A   08264.51782528 -.00002182  00000-0 -11606-4 0  2927
2 25544  51.6416 247.4627 0006703 130.5360 325.0288 15.72125391563537";
        let text = format!(
            "{iss}\n\n{}\njunk\n",
            iss.lines().skip(1).collect::<Vec<_>>().join("\n")
        );
        let (sets, bad) = read_catalogue(&text);
        assert_eq!(sets.len(), 2);
        assert_eq!(bad, 1);
        assert_eq!(sets[0].name.as_str(), "ISS (ZARYA)");
        assert_eq!(sets[1].catalog_number, 25544);
    }
}
