//! The duty-cycle limit of own transmissions, over the last hour of air time, in the bands that the host sets.

use std::collections::VecDeque;

/// A band with a duty-cycle limit.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DutyBand {
    /// The lower edge, Hz.
    pub low_hz: f64,
    /// The upper edge, Hz.
    pub high_hz: f64,
    /// The limit, a fraction of the hour.
    pub fraction: f64,
}

/// The most bands of a list.
pub const MAX_BANDS: usize = 64;

/// The bands of the text of the setting `duty_bands`: entries separated by ";", each the lower and the upper edge in Hz
/// separated by "-", "=" and the limit in percent. `None` when an entry does not parse or there are more than
/// [`MAX_BANDS`].
pub fn parse_bands(text: &str) -> Option<Vec<DutyBand>> {
    let v = text
        .split(';')
        .map(str::trim)
        .filter(|e| !e.is_empty())
        .map(|e| {
            let (range, pct) = e.split_once('=')?;
            let (lo, hi) = range.split_once('-')?;
            let (low_hz, high_hz): (f64, f64) = (lo.trim().parse().ok()?, hi.trim().parse().ok()?);
            let pct: f64 = pct.trim().parse().ok()?;
            let ok = low_hz.is_finite()
                && high_hz > low_hz
                && pct.is_finite()
                && (0.0..=100.0).contains(&pct);
            ok.then_some(DutyBand {
                low_hz,
                high_hz,
                fraction: pct / 100.0,
            })
        })
        .collect::<Option<Vec<_>>>()?;
    (v.len() <= MAX_BANDS).then_some(v)
}

/// Air time used in the last hour.
#[derive(Clone, Debug, Default)]
pub struct DutyCycle {
    sent: VecDeque<(f64, f64, f64)>,
    /// The limit applies (setting `duty_limit`).
    pub on: bool,
    /// The bands with a limit (setting `duty_bands`).
    pub bands: Vec<DutyBand>,
}

impl DutyCycle {
    /// A tracker, on, with no band.
    pub fn new() -> Self {
        DutyCycle {
            sent: VecDeque::new(),
            on: true,
            bands: Vec::new(),
        }
    }
    /// The band of a frequency, `None` when it has no limit.
    fn band(&self, freq_hz: f64) -> Option<usize> {
        self.bands
            .iter()
            .position(|b| (b.low_hz..=b.high_hz).contains(&freq_hz))
    }
    /// The limit (fraction) of a frequency, `None` when there is none.
    pub fn limit_for(&self, freq_hz: f64) -> Option<f64> {
        self.band(freq_hz).map(|i| self.bands[i].fraction)
    }
    fn used(&mut self, now: f64, freq: f64) -> f64 {
        while self.sent.front().is_some_and(|s| s.0 < now - 3600.0) {
            self.sent.pop_front();
        }
        let band = self.band(freq);
        self.sent
            .iter()
            .filter(|s| self.band(s.2) == band)
            .map(|s| s.1)
            .sum()
    }
    /// The earliest time from `now` at which `air_s` more at `freq` keeps within the limit.
    pub fn earliest(&mut self, now: f64, freq: f64, air_s: f64) -> f64 {
        let Some(lim) = self.limit_for(freq).filter(|_| self.on) else {
            return now;
        };
        let budget = lim * 3600.0;
        let mut used = self.used(now, freq);
        if used + air_s <= budget {
            return now;
        }
        // Wait until enough of the oldest transmissions leave the hour.
        let band = self.band(freq);
        for s in self.sent.iter().filter(|s| self.band(s.2) == band) {
            used -= s.1;
            if used + air_s <= budget {
                return s.0 + 3600.0;
            }
        }
        now + 3600.0
    }
    /// Records a transmission.
    pub fn record(&mut self, at: f64, air_s: f64, freq: f64) {
        self.sent.push_back((at, air_s, freq));
        if self.sent.len() > 100_000 {
            self.sent.pop_front();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_band_list_parses() {
        let v = parse_bands("100000000-100250000=10; 101000000-101600000=1").unwrap();
        assert_eq!(v.len(), 2);
        assert_eq!(v[1].fraction, 0.01);
        assert!(parse_bands("").unwrap().is_empty());
        assert!(parse_bands("2-1=1").is_none());
        assert!(parse_bands("1-2=101").is_none());
        let d = DutyCycle {
            bands: v,
            ..DutyCycle::new()
        };
        assert_eq!(d.limit_for(100.1e6), Some(0.1));
        assert_eq!(d.limit_for(102e6), None);
    }
}
