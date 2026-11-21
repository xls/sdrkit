//! Band scans, a host behaviour: the plan of radio centres for the chosen bands, and the stepping rule. The host chooses
//! the bands, their order and the ranges; this module holds no list of its own.

use crate::bands::{gscn_to_hz, lte_band, lte_bands, nbiot_band, nr_bands};

/// The 5 GHz LTE bands that a scan does not offer.
const NOT_OFFERED: [u16; 3] = [46, 47, 49];

/// Every LTE band a scan offers, in the order shown: the bands of `first`, then every other band of the table except the
/// 5 GHz bands 46, 47 and 49.
pub fn lte_offered(first: &[u16]) -> Vec<u16> {
    let mut v: Vec<u16> = Vec::new();
    for b in first.iter().copied().chain(lte_bands(&[]).iter().map(|b| b.number())) {
        if !NOT_OFFERED.contains(&b) && !v.contains(&b) {
            v.push(b);
        }
    }
    v
}

/// The EARFCN range of a custom LTE range (Hz), for display: the nearest EARFCNs of its two ends (-1 where no band holds
/// it), the bands of `order` first.
pub fn custom_earfcns(lo: f64, hi: f64, order: &[u16]) -> (i64, i64) {
    (
        crate::bands::nearest_earfcn(lo / 1e6, None, order),
        crate::bands::nearest_earfcn(hi / 1e6, None, order),
    )
}

/// The step of a plan at `rate`: max(1.5 MHz, 0.75 x rate).
pub fn span(rate: f64) -> f64 {
    (0.75 * rate).max(1.5e6)
}

/// Centres covering [lo, hi] Hz: from lo + span/2 in steps of span until a step's lower edge passes hi, each rounded to 0.1 MHz.
pub fn cover(lo: f64, hi: f64, rate: f64) -> Vec<f64> {
    let s = span(rate);
    let mut v = Vec::new();
    let mut c = lo + s / 2.0;
    while c - s / 2.0 <= hi {
        v.push((c / 1e5).round() * 1e5);
        c += s;
    }
    v
}

/// The LTE plan: the chosen bands' downlinks, then an optional custom range (Hz).
pub fn lte_plan(bands: &[u16], custom: Option<(f64, f64)>, rate: f64) -> Vec<f64> {
    let mut v = Vec::new();
    for &b in bands {
        if let Some(x) = lte_band(b) {
            v.extend(cover(x.dl_low_hz, x.dl_high_hz, rate));
        }
    }
    if let Some((lo, hi)) = custom {
        v.extend(cover(lo, hi, rate));
    }
    v
}

/// The NR plan: the sync raster of the chosen bands.
pub fn nr_plan(bands: &[u16], rate: f64) -> Vec<f64> {
    let mut v = Vec::new();
    let rows = nr_bands(bands)
        .into_iter()
        .filter(|b| bands.contains(&b.number()))
        .flat_map(|b| b.info.sync.iter());
    for s in rows {
        let (Some(lo), Some(hi)) = (gscn_to_hz(s.first), gscn_to_hz(s.last)) else {
            continue;
        };
        for c in cover(lo, hi, rate) {
            if !v.iter().any(|x: &f64| (x - c).abs() < 1.0) {
                v.push(c);
            }
        }
    }
    v
}

/// The NB-IoT plan: the chosen bands that the band table marks for NB-IoT, in the order given, on the LTE table.
pub fn nbiot_plan(bands: &[u16], rate: f64) -> Vec<f64> {
    let chosen: Vec<u16> = bands.iter().copied().filter(|&b| nbiot_band(b)).collect();
    lte_plan(&chosen, None, rate)
}

/// Steps a scan: a step ends when the dwell has passed and two captures were searched at this centre, or after 30 s.
#[derive(Clone, Debug)]
pub struct Stepper {
    /// The centres.
    pub plan: Vec<f64>,
    /// The current step.
    pub index: usize,
    /// Dwell, seconds.
    pub dwell_s: f64,
    started: f64,
    captures: u32,
    /// Stopped (end of plan, Stop, or the source stopped).
    pub done: bool,
}

impl Stepper {
    /// A scan of `plan` starting at time `now`.
    pub fn new(plan: Vec<f64>, now: f64) -> Self {
        Stepper {
            done: plan.is_empty(),
            plan,
            index: 0,
            dwell_s: 3.0,
            started: now,
            captures: 0,
        }
    }
    /// The centre to tune to.
    pub fn centre(&self) -> Option<f64> {
        (!self.done).then(|| self.plan[self.index])
    }
    /// A capture searched at `centre` (captures of an older centre are not counted).
    pub fn captured(&mut self, centre: f64) {
        if self.centre().is_some_and(|c| (c - centre).abs() <= 1.0) {
            self.captures += 1;
        }
    }
    /// The time is `now`: the next centre when the step is over.
    pub fn tick(&mut self, now: f64) -> Option<f64> {
        if self.done {
            return None;
        }
        let elapsed = now - self.started;
        if (elapsed >= self.dwell_s && self.captures >= 2) || elapsed >= 30.0 {
            self.index += 1;
            self.captures = 0;
            self.started = now;
            if self.index >= self.plan.len() {
                self.done = true;
                return None;
            }
            return self.centre();
        }
        None
    }
    /// Stop.
    pub fn stop(&mut self) {
        self.done = true;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plans_and_stepping() {
        // Band 20 (791 to 821 MHz) at 20 Msps: 15 MHz steps.
        let p = lte_plan(&[20], None, 20e6);
        assert_eq!(p, vec![798.5e6, 813.5e6, 828.5e6]);
        let c = lte_plan(&[], Some((790e6, 830e6)), 2e6);
        assert!(c.len() >= 26);
        assert!(!nr_plan(&[78], 20e6).is_empty());
        let railway = crate::bands::data_row("gsmr", "scan_mhz", "europe_railway");
        assert_eq!(cover(railway[0] * 1e6, railway[1] * 1e6, 5e6), vec![922.9e6, 926.6e6]);
        let mut s = Stepper::new(vec![1.0e9, 1.1e9], 0.0);
        s.captured(1.0e9);
        assert_eq!(s.tick(4.0), None);
        s.captured(0.5e9);
        s.captured(1.0e9);
        assert_eq!(s.tick(4.0), Some(1.1e9));
        assert_eq!(s.tick(40.0), None);
        assert!(s.done);
        // The offered LTE bands: presets first, no 5 GHz band 46, 47 or 49, every other band once.
        let presets = crate::bands::data_order_list("cell", "scan_bands", "europe");
        let first = [presets.clone(), crate::bands::data_order_list("cell", "scan_offer", "europe")].concat();
        let o = lte_offered(&first);
        assert_eq!(&o[..5], &presets[..]);
        assert_eq!(o[5], 28);
        assert!(!o.contains(&46) && !o.contains(&47) && !o.contains(&49));
        assert!(o.contains(&88) && o.contains(&65));
        let mut d = o.clone();
        d.sort();
        d.dedup();
        assert_eq!(d.len(), o.len());
        // NB-IoT: only NB-IoT bands; band 20 as for LTE; band 7 is not an NB-IoT band.
        assert_eq!(nbiot_plan(&[20], 20e6), p);
        assert!(nbiot_plan(&[7], 20e6).is_empty());
        let custom = crate::bands::data_row("cell", "scan_mhz", "europe");
        let order = crate::bands::data_order("cell", "europe");
        let (a, _) = custom_earfcns(custom[0] * 1e6, custom[1] * 1e6, &order);
        assert_eq!(a, 9530, "790 MHz is band 28");
        assert_eq!(custom_earfcns(806e6, 806e6, &order).0, 6300);
    }
}
