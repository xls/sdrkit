//! Where the signals are in a capture: "all signals" (strongest first) and the region a click lands on.

use crate::psd::{moving_average, Psd};
use sdrkit_algo_dsp::meter::power_to_db;
use serde::{Deserialize, Serialize};

/// A stretch of the spectrum holding one signal, in Hz from the capture's centre.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Region {
    /// The low edge (Hz from the centre).
    pub lo_hz: f64,
    /// The high edge (Hz from the centre).
    pub hi_hz: f64,
    /// Its strongest bin over the noise floor (dB).
    pub peak_db: f64,
}

impl Region {
    /// A region from its centre and width.
    pub fn around(centre_hz: f64, width_hz: f64) -> Self {
        Region {
            lo_hz: centre_hz - width_hz / 2.0,
            hi_hz: centre_hz + width_hz / 2.0,
            peak_db: 0.0,
        }
    }
    /// Its middle.
    pub fn centre(&self) -> f64 {
        0.5 * (self.lo_hz + self.hi_hz)
    }
    /// Its width.
    pub fn width(&self) -> f64 {
        self.hi_hz - self.lo_hz
    }
    /// Whether `hz` is inside.
    pub fn contains(&self, hz: f64) -> bool {
        hz >= self.lo_hz && hz <= self.hi_hz
    }
}

/// The most signals that "every signal" lists.
pub const MAX_REGIONS: usize = 16;
/// How far over the floor a signal must stand to be listed, dB.
pub const MIN_OVER_FLOOR_DB: f64 = 8.0;
/// The width taken around a click that lands on no signal.
pub const CLICK_WIDTH_HZ: f64 = 25_000.0;

/// Every signal at least `min_db` over the floor, strongest first, at most `max`.
pub fn find_regions(psd: &Psd, min_db: f64, max: usize) -> Vec<Region> {
    let n = psd.nfft;
    if n < 16 {
        return Vec::new();
    }
    let floor = psd.floor().max(1e-30);
    let pfloor = psd.peak_floor().max(1e-30);
    let smooth = moving_average(&psd.density, 3);
    let speak = moving_average(&psd.peak, 3);
    let edge = (n * 2 / 100).max(1);
    let extent = floor * 10f64.powf(4.0 / 10.0);
    let pextent = pfloor * 10f64.powf(6.0 / 10.0);
    // A bin is busy when its average stands over the floor, or when some frame had a burst there.
    let busy = |i: usize| (smooth[i] as f64) > extent || (speak[i] as f64) > pextent;
    let gap = ((2000.0 / psd.bin_hz()).ceil() as usize).clamp(2, n / 64 + 2);
    let mut out = Vec::new();
    let mut i = edge;
    while i < n - edge {
        if !busy(i) {
            i += 1;
            continue;
        }
        let start = i;
        let mut end = i;
        let mut peak = 0f64;
        let mut quiet = 0;
        while i < n - edge {
            if busy(i) {
                end = i;
                quiet = 0;
                peak = peak
                    .max(psd.density[i] as f64 / floor)
                    .max(psd.peak[i] as f64 / pfloor);
            } else {
                quiet += 1;
                if quiet > gap {
                    break;
                }
            }
            i += 1;
        }
        let over = power_to_db(peak);
        if over >= min_db {
            out.push(Region {
                lo_hz: psd.freq(start as f64 - 0.5),
                hi_hz: psd.freq(end as f64 + 0.5),
                peak_db: over,
            });
        }
    }
    out.sort_by(|a, b| b.peak_db.total_cmp(&a.peak_db));
    out.truncate(max);
    out
}

/// The region a click at `hz` (from the centre) lands on: the signal there, or [`CLICK_WIDTH_HZ`] around the click when none.
pub fn click_region(psd: &Psd, hz: f64) -> Region {
    let found = find_regions(psd, 6.0, 64);
    let slack = 2.0 * psd.bin_hz();
    found
        .into_iter()
        .filter(|r| hz >= r.lo_hz - slack && hz <= r.hi_hz + slack)
        .max_by(|a, b| a.peak_db.total_cmp(&b.peak_db))
        .unwrap_or_else(|| Region::around(hz, CLICK_WIDTH_HZ.min(psd.rate * 0.9)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use sdrkit_core::Complex32;

    #[test]
    fn two_tones_over_noise_strongest_first() {
        let rate = 1e6;
        let mut rng = 1u64;
        let mut g = || {
            rng ^= rng << 13;
            rng ^= rng >> 7;
            rng ^= rng << 17;
            ((rng >> 11) as f64 / (1u64 << 53) as f64 - 0.5) as f32
        };
        let x: Vec<Complex32> = (0..1 << 17)
            .map(|i| {
                let t = i as f64 / rate;
                let a = std::f64::consts::TAU * 100e3 * t;
                let b = std::f64::consts::TAU * -200e3 * t;
                Complex32::new(
                    (0.1 * a.cos() + 0.03 * b.cos()) as f32 + 0.01 * g(),
                    (0.1 * a.sin() + 0.03 * b.sin()) as f32 + 0.01 * g(),
                )
            })
            .collect();
        let p = Psd::welch(&x, rate, 4096, 64);
        let r = find_regions(&p, 8.0, 16);
        assert_eq!(r.len(), 2, "{r:?}");
        assert!(r[0].contains(100e3) && r[1].contains(-200e3));
        assert!(click_region(&p, 100.1e3).contains(100e3));
        let empty = click_region(&p, 300e3);
        assert!((empty.width() - 25e3).abs() < 1.0);
    }
}
