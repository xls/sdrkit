//! Band searches: the radio centres that cover the raster of a band at a sample rate, and the survey: one row per cell
//! found in the same columns for every generation, so that a host can draw the coverage of a search. This module holds no
//! list of bands of its own.

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

/// The half width, Hz, around a radio centre in which the LTE search tries carrier centres at `rate`: 0.46 x `rate` less
/// 0.7 MHz, half the narrowest carrier (`lte::search::candidates`).
pub fn lte_window(rate: f64) -> f64 {
    (0.46 * rate - 0.7e6).max(0.0)
}

/// The half width, Hz, around a radio centre in which the NR search tries sync raster points of SSBs of `scs_khz` at
/// `rate`; negative when not one whole SSB fits beside the centre (`nr::search::candidates`). The search then tries the
/// raster point at the centre only.
pub fn nr_window(rate: f64, scs_khz: u32) -> f64 {
    0.46 * rate - (120.0 * f64::from(scs_khz) * 1e3 + 0.2e6)
}

/// The least rate, Hz, at which the NR search takes SSBs of `scs_khz`: 256 subcarriers of the spacing, less 3 %.
pub fn nr_least_rate(scs_khz: u32) -> f64 {
    0.97 * 256.0 * f64::from(scs_khz) * 1e3
}

/// Radio centres that put every point of `points` (Hz, ascending) within `window` of a centre, each centre rounded down to
/// `snap` Hz. With a window below half the snap step every point is a centre.
fn cover_points(points: &[f64], window: f64, snap: f64) -> Vec<f64> {
    let mut out: Vec<f64> = Vec::new();
    let mut i = 0;
    while i < points.len() {
        let c = if window < snap / 2.0 {
            points[i]
        } else {
            ((points[i] + window) / snap).floor() * snap
        };
        out.push(c);
        while i < points.len() && points[i] <= c + window.max(0.0) + 1.0 {
            i += 1;
        }
    }
    out
}

/// The points of `points` (Hz) that lie in one of `ranges` (low and high edge, Hz) and outside `window` of every centre of
/// `taken` (a step of an earlier band holds them). When no range holds a point, every point: a range keeps only the bands
/// it overlaps to it.
fn within(points: Vec<f64>, ranges: &[(f64, f64)], taken: &[f64], window: f64) -> Vec<f64> {
    let inside = |p: &f64| ranges.iter().any(|(lo, hi)| (*lo..=*hi).contains(p));
    let all = !points.iter().any(inside);
    points
        .into_iter()
        .filter(|p| all || inside(p))
        .filter(|p| !taken.iter().any(|c| (p - c).abs() <= window.max(0.0)))
        .collect()
}

/// The radio centres of an LTE search of band `band` at `rate`: every 100 kHz raster point of the downlink where a carrier
/// of 1.4 MHz or more can have its centre lies in the window ([`lte_window`]) of one centre, lowest first. Only the points
/// in `ranges` (Hz) when one of them holds a point of the band, and none in the window of a centre of `taken`. Empty for a
/// band the table does not hold.
pub fn lte_steps(band: u16, rate: f64, ranges: &[(f64, f64)], taken: &[f64]) -> Vec<f64> {
    let Some(b) = lte_band(band) else {
        return Vec::new();
    };
    let lo = b.dl_low_hz + 0.7e6;
    let hi = b.dl_high_hz - 0.7e6;
    let n = ((hi - lo) / 100e3).floor().max(0.0) as usize;
    let points: Vec<f64> = (0..=n).map(|k| ((lo + k as f64 * 100e3) / 1e3).round() * 1e3).collect();
    cover_points(&within(points, ranges, taken, lte_window(rate)), lte_window(rate), 100e3)
}

/// The radio centres of an NR search of band `band` at `rate`: every sync raster point of the band, of each SSB spacing the
/// rate takes ([`nr_least_rate`]), lies in the window ([`nr_window`]) of one centre, lowest first. Only the points in
/// `ranges` (Hz) when one of them holds a point of the band, and none in the window of a centre of `taken`. Empty for a
/// band the table does not hold, or a rate below the least of every spacing of the band.
pub fn nr_steps(band: u16, rate: f64, ranges: &[(f64, f64)], taken: &[f64]) -> Vec<f64> {
    let mut out: Vec<f64> = Vec::new();
    for b in nr_bands(&[band]).into_iter().filter(|b| b.number() == band) {
        for s in b.info.sync.iter().filter(|s| rate >= nr_least_rate(s.scs_khz)) {
            let points: Vec<f64> = (s.first..=s.last)
                .step_by(s.step.max(1) as usize)
                .filter_map(gscn_to_hz)
                .collect();
            let window = nr_window(rate, s.scs_khz);
            for c in cover_points(&within(points, ranges, taken, window), window, 10e3) {
                if !out.iter().any(|x| (x - c).abs() < 1e3) {
                    out.push(c);
                }
            }
        }
    }
    out.sort_by(f64::total_cmp);
    out
}

/// The NB-IoT bands of `bands`: those that the band table marks for NB-IoT, in the order given.
pub fn nbiot_bands(bands: &[u16]) -> Vec<u16> {
    bands.iter().copied().filter(|&b| nbiot_band(b)).collect()
}

sdrkit_core::choice_enum! {
    /// The generation of the radio system of a cell.
    #[derive(Default, serde::Serialize, serde::Deserialize)]
    pub enum Generation {
        /// 2G: GSM.
        #[default]
        #[serde(rename = "2g")]
        Second = ("2g", "2G"),
        /// 3G: UMTS, TD-SCDMA, CDMA2000.
        #[serde(rename = "3g")]
        Third = ("3g", "3G"),
        /// 4G: LTE, NB-IoT.
        #[serde(rename = "4g")]
        Fourth = ("4g", "4G"),
        /// 5G: NR.
        #[serde(rename = "5g")]
        Fifth = ("5g", "5G"),
    }
}

/// The name of a cell in a cells table or a survey: operator, generation, band and the physical identity, "Telstra 4G B3
/// PCI 301". Missing parts are left out.
pub fn cell_name(operator: Option<&str>, generation: Generation, band: Option<&str>, id: &str) -> String {
    let g = &<Generation as sdrkit_core::Choice>::CHOICES[usize::from(sdrkit_core::Choice::index(generation))].label;
    let mut parts: Vec<&str> = Vec::new();
    parts.extend(operator);
    parts.push(g);
    parts.extend(band);
    parts.push(id);
    parts.join(" ")
}

/// A cell of a survey, as a stack reports it: only what it decoded.
#[derive(Clone, Debug, PartialEq)]
pub struct SurveyCell {
    /// The row key of the cell in the cells table of its stack.
    pub key: String,
    /// The generation.
    pub generation: Generation,
    /// The network, "505-01".
    pub plmn: Option<String>,
    /// The operator name of the network from the list of networks, when it names it.
    pub operator_name: Option<String>,
    /// The band: "B3", "n78", "GSM 900".
    pub band: Option<String>,
    /// The channel number: ARFCN, UARFCN, EARFCN or NR-ARFCN.
    pub channel: Option<i64>,
    /// The frequency, Hz.
    pub frequency_hz: f64,
    /// The word for the physical identity: "PCI", "PSC" or "BSIC".
    pub id_word: &'static str,
    /// The physical identity: PCI, primary scrambling code or BSIC.
    pub pci: i64,
    /// The cell identity of the broadcast.
    pub cell_identity: Option<i64>,
    /// The area code: TAC (LTE, NR) or LAC (GSM, UMTS).
    pub area_code: Option<i64>,
    /// The level: RSRP, RSCP or the received level, dBFS.
    pub level_dbfs: Option<f64>,
}

impl SurveyCell {
    /// The operator column: the name, else the network, else none.
    pub fn operator(&self) -> Option<&str> {
        self.operator_name.as_deref().or(self.plmn.as_deref())
    }
    /// The label of the row: operator, generation, band and physical identity, "Telstra 4G B3 PCI 301".
    pub fn label(&self) -> String {
        let id = format!("{} {}", self.id_word, self.pci);
        cell_name(self.operator(), self.generation, self.band.as_deref(), &id)
    }
}

/// Sorts survey cells into the order of the survey: by generation, operator, band, channel, cell identity, physical
/// identity and key. The order is total, so the same cells give the same order.
pub fn survey_order(cells: &mut [SurveyCell]) {
    cells.sort_by(|a, b| {
        sdrkit_core::Choice::index(a.generation)
            .cmp(&sdrkit_core::Choice::index(b.generation))
            .then_with(|| a.operator().cmp(&b.operator()))
            .then_with(|| a.band.cmp(&b.band))
            .then_with(|| a.channel.cmp(&b.channel))
            .then_with(|| a.cell_identity.cmp(&b.cell_identity))
            .then_with(|| a.pci.cmp(&b.pci))
            .then_with(|| a.key.cmp(&b.key))
    });
}

/// Writes the survey rows of `cells`, sorted, into `table`.
pub fn write_survey(
    table: &'static sdrkit_core::interface::Table<crate::typed::SurveyRow<'static>>,
    mut cells: Vec<SurveyCell>,
    p: &mut sdrkit_core::publish::Published,
) {
    survey_order(&mut cells);
    let mut rows = crate::decoder::RowSync::new(table);
    for c in &cells {
        let label = c.label();
        let row = crate::typed::SurveyRow {
            name: &label,
            generation: c.generation,
            operator: c.operator(),
            plmn: c.plmn.as_deref(),
            band: c.band.as_deref(),
            channel: c.channel,
            frequency: sdrkit_core::interface::Hertz(c.frequency_hz),
            pci: c.pci,
            cell_identity: c.cell_identity,
            area_code: c.area_code,
            level_dbfs: c.level_dbfs,
        };
        rows.put(p, c.key.clone(), &row);
    }
    rows.finish(p);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn survey_order_and_labels() {
        let cell = |key: &str, generation, plmn: Option<&str>, name: Option<&str>, band: &str, channel, pci| SurveyCell {
            key: key.into(),
            generation,
            plmn: plmn.map(str::to_string),
            operator_name: name.map(str::to_string),
            band: Some(band.into()),
            channel: Some(channel),
            frequency_hz: 0.0,
            id_word: "PCI",
            pci,
            cell_identity: None,
            area_code: None,
            level_dbfs: None,
        };
        let mut v = vec![
            cell("c", Generation::Fifth, Some("505-01"), Some("Telstra"), "n78", 643_334, 7),
            cell("b", Generation::Fourth, Some("505-02"), None, "B3", 1300, 301),
            cell("a", Generation::Fourth, Some("505-01"), Some("Telstra"), "B3", 1275, 12),
            cell("d", Generation::Fourth, None, None, "B28", 9410, 5),
        ];
        survey_order(&mut v);
        let keys: Vec<&str> = v.iter().map(|c| c.key.as_str()).collect();
        // LTE before NR; a cell without a network first; "505-02" before "Telstra".
        assert_eq!(keys, ["d", "b", "a", "c"]);
        assert_eq!(v[2].label(), "Telstra 4G B3 PCI 12");
        assert_eq!(v[1].label(), "505-02 4G B3 PCI 301");
        assert_eq!(v[0].label(), "4G B28 PCI 5");
    }

    #[test]
    fn search_steps() {
        // Band 28 (758 to 803 MHz) at 20 Msps: windows of 8.5 MHz, three centres; at 2.56 Msps windows of 0.48 MHz.
        let wide = lte_steps(28, 20e6, &[], &[]);
        assert_eq!(wide, vec![767.2e6, 784.3e6, 801.4e6]);
        let narrow = lte_steps(28, 2.56e6, &[], &[]);
        assert_eq!(narrow.len(), 49);
        let w = lte_window(2.56e6);
        for k in 0..=436 {
            let f = 758.7e6 + f64::from(k) * 100e3;
            assert!(narrow.iter().any(|c| (c - f).abs() <= w + 1.0), "{f} not covered");
        }
        assert!(lte_steps(9999, 20e6, &[], &[]).is_empty());
        // Band 5 lies inside band 26: after the steps of band 26 it has none of its own.
        let b26 = lte_steps(26, 12e6, &[], &[]);
        assert!(lte_steps(5, 12e6, &[], &b26).is_empty());
        assert_eq!(lte_steps(5, 12e6, &[], &[]).len(), 3);
        // n78 at 20 Msps: every GSCN inside a window; at 3.84 Msps each 15 kHz point is a centre, 30 kHz points need 7.45 Msps.
        let n78 = nr_steps(78, 20e6, &[], &[]);
        assert!(!n78.is_empty() && n78.len() < 50, "{}", n78.len());
        // A range keeps the band to it; a range that holds no point of the band leaves the band whole.
        let upper = nr_steps(78, 20e6, &[(3400e6, 3800e6), (758e6, 803e6)], &[]);
        assert!(!upper.is_empty() && upper.len() < n78.len(), "{} of {}", upper.len(), n78.len());
        assert!(upper.iter().all(|c| *c > 3390e6), "{upper:?}");
        assert_eq!(nr_steps(78, 20e6, &[(758e6, 803e6)], &[]), n78);
        assert!(nr_steps(78, 3.84e6, &[], &[]).is_empty(), "n78 has 30 kHz blocks only");
        let n28 = nr_steps(28, 3.84e6, &[], &[]);
        assert!(n28.iter().all(|f| crate::bands::hz_to_gscn(*f).is_some()));
        assert!(n28.len() > 30, "{}", n28.len());
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
        // NB-IoT: only NB-IoT bands; band 7 is not an NB-IoT band.
        assert_eq!(nbiot_bands(&[20, 7, 8]), vec![20, 8]);
        let custom = crate::bands::data_row("cell", "scan_mhz", "europe");
        let order = crate::bands::data_order("cell", "europe");
        let (a, _) = custom_earfcns(custom[0] * 1e6, custom[1] * 1e6, &order);
        assert_eq!(a, 9530, "790 MHz is band 28");
        assert_eq!(custom_earfcns(806e6, 806e6, &order).0, 6300);
    }
}
