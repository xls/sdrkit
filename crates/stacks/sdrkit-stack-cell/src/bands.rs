//! The cell family's view of the shared band tables (`sdrkit_data_plans::cellular`): EARFCNs in MHz, the band choice of a
//! cell, the usual Lmax, CORESET#0 and the GSM-R band.
//!
//! Where the rasters of two bands overlap, a lookup takes the bands of an order list first, in its order, and the other
//! bands after them in band number order. The host sets the order (the setting `band_order`); an empty list is band number
//! order.

use sdrkit_data_plans::cellular::{self, CellSystem, Duplex};
pub use sdrkit_data_plans::cellular::{gscn_hz as gscn_to_hz, gscn_of as hz_to_gscn, CellBand};

/// The LTE bands, those of `order` first.
pub fn lte_bands(order: &[u16]) -> Vec<&'static CellBand> {
    cellular::bands(CellSystem::Lte, order)
}

/// The NR bands, those of `order` first.
pub fn nr_bands(order: &[u16]) -> Vec<&'static CellBand> {
    cellular::bands(CellSystem::Nr, order)
}

/// Whether the band table marks LTE band `b` for NB-IoT.
pub fn nbiot_band(b: u16) -> bool {
    lte_band(b).is_some_and(|x| x.modes.contains(&"nbiot"))
}

/// The LTE band with number `b`.
pub fn lte_band(b: u16) -> Option<&'static CellBand> {
    cellular::band(CellSystem::Lte, b)
}

/// The downlink frequency (MHz) of EARFCN `n`; 0 when no band holds it.
pub fn earfcn_to_mhz(n: u32) -> f64 {
    cellular::channel_hz(CellSystem::Lte, n).map_or(0.0, |(hz, _)| hz / 1e6)
}

/// The band of EARFCN `n`; 0 when none.
pub fn earfcn_band(n: u32) -> u16 {
    cellular::channel_hz(CellSystem::Lte, n).map_or(0, |(_, b)| b.number())
}

fn earfcn_search(mhz: f64, preferred: Option<u16>, order: &[u16], tol_hz: Option<f64>) -> i64 {
    let mut prefer = Vec::with_capacity(order.len() + 1);
    prefer.extend(preferred);
    prefer.extend_from_slice(order);
    cellular::channel_of(CellSystem::Lte, mhz * 1e6, &prefer, tol_hz).map_or(-1, |(n, _)| n.into())
}

/// The EARFCN of `mhz` (within 20 kHz of a raster point), preferring band `preferred`, then the bands of `order`; -1 when
/// none.
pub fn mhz_to_earfcn(mhz: f64, preferred: Option<u16>, order: &[u16]) -> i64 {
    earfcn_search(mhz, preferred, order, Some(20e3 + 1e-3))
}

/// The nearest EARFCN of `mhz` (no 20 kHz condition), preferring band `preferred`, then the bands of `order`; -1 when no
/// band holds it.
pub fn nearest_earfcn(mhz: f64, preferred: Option<u16>, order: &[u16]) -> i64 {
    earfcn_search(mhz, preferred, order, None)
}

/// Every LTE band whose raster covers `mhz`, those of `order` first.
pub fn bands_holding(mhz: f64, order: &[u16]) -> Vec<u16> {
    cellular::bands_at(CellSystem::Lte, mhz * 1e6, order)
        .iter()
        .map(|b| b.number())
        .collect()
}

/// The band chosen for a cell.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct BandChoice {
    /// The band, 0 for none.
    pub band: u16,
    /// SIB1 confirmed it.
    pub confirmed: bool,
    /// Other bands that hold the frequency (or SIB1's multi-band list).
    pub also: Vec<u16>,
    /// The "also" list is SIB1's multi-band list.
    pub mfbi: bool,
}

/// The band of a cell at `mhz`, given SIB1's band indicator `s` (0 when SIB1 is not read); of several bands that hold
/// `mhz`, those of `order` come first.
pub fn band_choice(mhz: f64, s: u16, order: &[u16]) -> BandChoice {
    let holding = bands_holding(mhz, order);
    if (1..=63).contains(&s) || (65..=256).contains(&s) {
        return BandChoice {
            band: s,
            confirmed: true,
            also: holding.into_iter().filter(|&b| b != s).collect(),
            mfbi: false,
        };
    }
    let pool: Vec<u16> = if s == 64 {
        holding.into_iter().filter(|&b| b > 64).collect()
    } else {
        holding
    };
    match pool.first() {
        None => BandChoice::default(),
        Some(&b) => BandChoice {
            band: b,
            confirmed: s == 64 && pool.len() == 1,
            also: pool[1..].to_vec(),
            mfbi: false,
        },
    }
}

/// Applies SIB1's multi-band list to a choice.
pub fn apply_mfbi(c: &mut BandChoice, list: &[u16]) {
    if list.is_empty() {
        return;
    }
    let mut also = Vec::new();
    for &b in list {
        if b != c.band && b != 64 && !also.contains(&b) {
            also.push(b);
        }
    }
    c.also = also;
    c.mfbi = true;
}

/// The NR band of a GSCN with this SSB spacing (any spacing when `scs` is none), the bands of `order` first.
pub fn nr_band_of(gscn: u32, scs: Option<u32>, order: &[u16]) -> Option<&'static CellBand> {
    cellular::sync_band(gscn, scs, order).map(|(b, _)| b)
}

/// The usual Lmax of a band at `hz` (FR1, Cases A, B, C).
pub fn usual_lmax(band: Option<&CellBand>, hz: f64) -> u32 {
    let tdd = band.is_some_and(|b| b.duplex() == Duplex::Tdd);
    if hz > if tdd { 2.4e9 } else { 3e9 } {
        8
    } else {
        4
    }
}

/// CORESET#0 (RBs, symbols, RB offset) of index `idx` for SSB and PDCCH spacings (kHz), or none when reserved.
pub fn coreset0(ssb_scs: u32, pdcch_scs: u32, idx: usize) -> Option<(u32, u32, u32)> {
    const T1: [(u32, u32, u32); 15] = [
        (24, 2, 0),
        (24, 2, 2),
        (24, 2, 4),
        (24, 3, 0),
        (24, 3, 2),
        (24, 3, 4),
        (48, 1, 12),
        (48, 1, 16),
        (48, 2, 12),
        (48, 2, 16),
        (48, 3, 12),
        (48, 3, 16),
        (96, 1, 38),
        (96, 2, 38),
        (96, 3, 38),
    ];
    const T2: [(u32, u32, u32); 14] = [
        (24, 2, 5),
        (24, 2, 6),
        (24, 2, 7),
        (24, 2, 8),
        (24, 3, 5),
        (24, 3, 6),
        (24, 3, 7),
        (24, 3, 8),
        (48, 1, 18),
        (48, 1, 20),
        (48, 2, 18),
        (48, 2, 20),
        (48, 3, 18),
        (48, 3, 20),
    ];
    const T3: [(u32, u32, u32); 9] = [
        (48, 1, 2),
        (48, 1, 6),
        (48, 2, 2),
        (48, 2, 6),
        (48, 3, 2),
        (48, 3, 6),
        (96, 1, 28),
        (96, 2, 28),
        (96, 3, 28),
    ];
    const T4: [(u32, u32, u32); 16] = [
        (24, 2, 0),
        (24, 2, 1),
        (24, 2, 2),
        (24, 2, 3),
        (24, 2, 4),
        (24, 3, 0),
        (24, 3, 1),
        (24, 3, 2),
        (24, 3, 3),
        (24, 3, 4),
        (48, 1, 12),
        (48, 1, 14),
        (48, 1, 16),
        (48, 2, 12),
        (48, 2, 14),
        (48, 2, 16),
    ];
    let t: &[(u32, u32, u32)] = match (ssb_scs, pdcch_scs) {
        (15, 15) => &T1,
        (15, 30) => &T2,
        (30, 15) => &T3,
        (30, 30) => &T4,
        _ => return None,
    };
    t.get(idx).copied()
}

fn gsm_hz(arfcn: u32, bands: &[u16]) -> Option<f64> {
    cellular::bands(CellSystem::Gsm, bands)
        .into_iter()
        .filter(|b| bands.contains(&b.number()))
        .find_map(|b| b.channel_hz(arfcn))
}

fn gsm_arfcn(hz: f64, bands: &[u16]) -> Option<u32> {
    cellular::bands(CellSystem::Gsm, bands)
        .into_iter()
        .filter(|b| bands.contains(&b.number()))
        .filter_map(|b| b.nearest_channel(hz))
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .map(|x| x.0)
}

/// The downlink frequency (Hz) of a GSM ARFCN on the 900 MHz raster (P-GSM, E-GSM and R-GSM).
pub fn arfcn_to_hz(arfcn: u32) -> Option<f64> {
    gsm_hz(arfcn, &[900])
}

/// The ARFCN of `hz` on the 900 MHz raster, or none outside it.
pub fn hz_to_arfcn(hz: f64) -> Option<u32> {
    gsm_arfcn(hz, &[900])
}

/// The downlink frequency (Hz) of an ARFCN on the 900 MHz, GSM 850 and DCS 1800 rasters (the "GSM band" setting's extension).
pub fn arfcn_to_hz_ext(arfcn: u32) -> Option<f64> {
    gsm_hz(arfcn, &[900, 850, 1800])
}

/// The ARFCN of `hz` on the extended rasters.
pub fn hz_to_arfcn_ext(hz: f64) -> Option<u32> {
    gsm_arfcn(hz, &[900, 850, 1800])
}

/// Whether an ARFCN is in the railway band (R-GSM / GSM-R, 955 to 973).
pub fn railway_band(arfcn: u32) -> bool {
    (955..=973).contains(&arfcn)
}

/// The values of the row (`stack`, `list`, `name`) of the repository's `data/cell/band_order.csv`, for the tests.
#[cfg(test)]
pub(crate) fn data_row(stack: &str, list: &str, name: &str) -> Vec<f64> {
    const TEXT: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../../data/cell/band_order.csv"));
    let key = format!("{stack},{list},{name},");
    let line = TEXT
        .lines()
        .find_map(|l| l.strip_prefix(key.as_str()))
        .unwrap_or_else(|| panic!("no row {key}"));
    line.split_whitespace().map(|v| v.parse().expect("a number")).collect()
}

/// The band numbers of a row of the data file, for the tests.
#[cfg(test)]
pub(crate) fn data_order_list(stack: &str, list: &str, name: &str) -> Vec<u16> {
    data_row(stack, list, name).into_iter().map(|b| b as u16).collect()
}

/// The band numbers of a `band_order` row of the data file, for the tests.
#[cfg(test)]
pub(crate) fn data_order(stack: &str, name: &str) -> Vec<u16> {
    data_order_list(stack, "band_order", name)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-6
    }

    #[test]
    fn earfcn_in_the_family_order() {
        let lte = data_order("cell", "europe");
        assert!(close(earfcn_to_mhz(6300), 806.0));
        assert_eq!(earfcn_band(6300), 20);
        assert_eq!(earfcn_band(4960), 0);
        assert_eq!(mhz_to_earfcn(806.0, None, &lte), 6300);
        assert_eq!(mhz_to_earfcn(1844.9, None, &lte), 1599);
        assert_eq!(mhz_to_earfcn(1844.9, Some(9), &lte), 3800);
        assert_eq!(mhz_to_earfcn(1000.0, None, &lte), -1);
        assert_eq!(nearest_earfcn(2140.0, Some(66), &lte), 66736);
        assert_eq!(nearest_earfcn(2140.0, Some(4), &lte), 2250);
        assert_eq!(nearest_earfcn(2140.0, None, &lte), 300);
        assert_eq!(nearest_earfcn(760.0, None, &lte), 9230);
        assert_eq!(nearest_earfcn(617.0, None, &lte), 68586);
        assert_eq!(lte_bands(&lte)[0].number(), 1);
        assert_eq!(lte_bands(&lte).len(), 66);
    }

    #[test]
    fn bands_holding_a_frequency() {
        let lte = data_order("cell", "europe");
        let b = bands_holding(2125.0, &lte);
        assert_eq!(b, vec![1, 4, 66, 10, 65]);
        let b = bands_holding(765.0, &lte);
        assert!(b.contains(&14) && b.contains(&28));
        assert_eq!(bands_holding(640.0, &lte), vec![71]);
        assert!(bands_holding(1000.0, &lte).is_empty());
    }

    #[test]
    fn band_choice_of_a_frequency() {
        let lte = data_order("cell", "europe");
        let c = band_choice(630.0, 64, &lte);
        assert_eq!((c.band, c.confirmed), (71, true));
        assert!(c.also.is_empty());
        let c = band_choice(2125.0, 64, &lte);
        assert!(c.band == 65 || c.band == 66);
        assert!(!c.confirmed);
        assert!(c.also.contains(&(if c.band == 65 { 66 } else { 65 })));
        assert_eq!(band_choice(1000.0, 64, &lte).band, 0);
        let c = band_choice(2125.0, 4, &lte);
        assert_eq!((c.band, c.confirmed), (4, true));
        for x in [1, 10, 66] {
            assert!(c.also.contains(&x));
        }
        let c = band_choice(763.0, 14, &lte);
        assert_eq!((c.band, c.confirmed), (14, true));
        assert!(c.also.contains(&28));
        assert_eq!(band_choice(763.0, 13, &lte).band, 13);
        let c = band_choice(2125.0, 0, &lte);
        assert!(!c.confirmed && c.also.len() >= 3);
        let c = band_choice(640.0, 0, &lte);
        assert_eq!((c.band, c.confirmed), (71, false));
        assert!(c.also.is_empty() && !c.mfbi);
        let mut c = band_choice(940.0, 106, &lte);
        apply_mfbi(&mut c, &[8, 106, 64]);
        assert_eq!((c.band, c.mfbi, c.also.clone()), (106, true, vec![8]));
        let c = band_choice(2125.0, 66, &lte);
        assert_eq!(
            (c.band, c.confirmed, c.also.clone()),
            (66, true, vec![1, 4, 10, 65])
        );
    }

    #[test]
    fn nr_order() {
        let nr = data_order("nr", "europe");
        assert_eq!(nr_band_of(7875, Some(30), &nr).unwrap().number(), 78);
        assert_eq!(nr_band_of(1990, Some(15), &nr).unwrap().number(), 28);
        assert!(nr_band_of(1990, Some(30), &nr).is_none());
        assert_eq!(usual_lmax(nr_band_of(1990, Some(15), &nr), 795.85e6), 4);
        assert_eq!(usual_lmax(nr_band_of(7875, Some(30), &nr), 3541.44e6), 8);
        assert_eq!(usual_lmax(nr_band_of(6252, Some(30), &nr), 2.5e9), 8);
        assert_eq!(usual_lmax(nr_band_of(5800, Some(30), &nr), 2.35e9), 4);
        assert_eq!(nr_bands(&nr)[0].number(), 78);
    }

    #[test]
    fn coreset0_cases() {
        assert_eq!(coreset0(15, 15, 3), Some((24, 3, 0)));
        assert_eq!(coreset0(30, 30, 12), Some((48, 1, 16)));
        assert_eq!(coreset0(15, 15, 15), None);
    }

    #[test]
    fn gsm_raster() {
        assert!(close(arfcn_to_hz(960).unwrap(), 922.2e6));
        assert!(close(arfcn_to_hz(955).unwrap(), 921.2e6));
        assert!(close(arfcn_to_hz(973).unwrap(), 924.8e6));
        assert!(close(arfcn_to_hz(0).unwrap(), 935.0e6));
        assert!(close(arfcn_to_hz(124).unwrap(), 959.8e6));
        assert_eq!(hz_to_arfcn(922.2e6), Some(960));
        assert_eq!(hz_to_arfcn(921.0e6), None);
        assert!(railway_band(955) && railway_band(973) && !railway_band(954) && !railway_band(974));
        assert_eq!(hz_to_arfcn_ext(1805.2e6), Some(512));
        assert!(close(arfcn_to_hz_ext(128).unwrap(), 869.2e6));
    }
}
