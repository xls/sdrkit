//! Cellular channel numbers: the GSM ARFCN, the LTE EARFCN, the NR-ARFCN and the NR GSCN of a frequency, and back.
//! The band table of the standards is a constant of this crate; this module holds it, the lookups and the formulas of the NR
//! global rasters.
//!
//! Standard: 3GPP TS 45.005 clause 2, TS 36.101 clause 5.7.3, TS 38.104 clauses 5.4.2.1 and 5.4.3.1.
//!
//! # Example
//!
//! ```
//! use sdrkit_data_plans::cellular::{self, CellSystem};
//!
//! let (hz, band) = cellular::channel_hz(CellSystem::Lte, 6300).unwrap();
//! assert_eq!((hz, band.number()), (806e6, 20));
//! assert_eq!(cellular::nr_arfcn_hz(620_000), Some(3.3e9));
//! ```

pub use crate::data::{CellNumbers, CellSystem, Cellular, Duplex, SyncRaster};

/// The channel numbers of a cellular band.
#[derive(Clone, Copy, Debug)]
pub struct CellInfo {
    /// The system.
    pub system: CellSystem,
    /// The band number (LTE band 20 is 20, NR band n78 is 78). GSM uses the band name (900, 850, 1800, 1900).
    pub band: u16,
    /// How the band uses its frequencies.
    pub duplex: Duplex,
    /// The downlink channel numbers, in one or more ranges.
    pub numbers: &'static [CellNumbers],
    /// The NR synchronisation raster (GSCN), one range for each SSB subcarrier spacing.
    pub sync: &'static [SyncRaster],
}

/// One cellular band: its name, its downlink edges and its channel numbers.
#[derive(Clone, Copy, Debug)]
pub struct CellBand {
    /// The name ("LTE band 20 (EU 800 DD)").
    pub name: &'static str,
    /// The lower downlink edge, Hz.
    pub dl_low_hz: f64,
    /// The upper downlink edge, Hz.
    pub dl_high_hz: f64,
    /// The ids of the stacks that receive it.
    pub modes: &'static [&'static str],
    /// The channel numbers.
    pub info: CellInfo,
}

impl CellBand {
    /// The system.
    pub fn system(&self) -> CellSystem {
        self.info.system
    }

    /// The band number (n78 is 78; GSM uses the band name, 900 or 1800).
    pub fn number(&self) -> u16 {
        self.info.band
    }

    /// How the band uses its frequencies.
    pub fn duplex(&self) -> Duplex {
        self.info.duplex
    }

    /// The downlink frequency of channel number `n`, Hz, or `None` when the band does not hold `n`.
    pub fn channel_hz(&self, n: u32) -> Option<f64> {
        self.info.numbers.iter().find_map(|r| r.hz(n))
    }

    /// The channel number nearest to `hz` and its distance from `hz`, Hz. `None` when `hz` rounds outside every range of
    /// the band.
    pub fn nearest_channel(&self, hz: f64) -> Option<(u32, f64)> {
        self.info
            .numbers
            .iter()
            .filter_map(|r| r.nearest(hz))
            .min_by(|a, b| a.1.total_cmp(&b.1))
    }

    /// The GSCN range of the band for SSB spacing `scs_khz` (NR only).
    pub fn sync(&self, scs_khz: u32) -> Option<&'static SyncRaster> {
        self.info.sync.iter().find(|s| s.scs_khz == scs_khz)
    }
}

/// Every cellular band, GSM, then LTE, then NR, each by band number.
pub fn cell_bands() -> &'static [CellBand] {
    crate::cell_table::BANDS
}

/// The bands of `system`: first the bands whose numbers are in `prefer` (in that order), then the others in table order.
pub fn bands(system: CellSystem, prefer: &[u16]) -> Vec<&'static CellBand> {
    let all = cell_bands().iter().filter(move |b| b.system() == system);
    let mut out: Vec<&CellBand> = prefer
        .iter()
        .flat_map(|&p| all.clone().filter(move |b| b.number() == p))
        .collect();
    for b in all {
        if !prefer.contains(&b.number()) {
            out.push(b);
        }
    }
    out
}

/// The first band of `system` with number `band` ("LTE band 20" for `(Lte, 20)`).
pub fn band(system: CellSystem, band: u16) -> Option<&'static CellBand> {
    cell_bands()
        .iter()
        .find(|b| b.system() == system && b.number() == band)
}

/// The downlink frequency of channel number `n` of `system`, Hz, and the first band (in table order) that holds it. EARFCN
/// follows TS 36.101 5.7.3: F_DL = F_DL_low + 0.1 MHz (N_DL - N_Offs-DL).
pub fn channel_hz(system: CellSystem, n: u32) -> Option<(f64, &'static CellBand)> {
    cell_bands()
        .iter()
        .filter(|b| b.system() == system)
        .find_map(|b| Some((b.channel_hz(n)?, b)))
}

/// The channel number of `system` at `hz` and its band. The bands in `prefer` are tried first (see [`bands`]). If
/// `tolerance_hz` is given, the channel must be within it of `hz`. Otherwise the nearest channel of the first band whose
/// raster covers `hz` is taken.
pub fn channel_of(
    system: CellSystem,
    hz: f64,
    prefer: &[u16],
    tolerance_hz: Option<f64>,
) -> Option<(u32, &'static CellBand)> {
    bands(system, prefer).into_iter().find_map(|b| {
        let (n, err) = b.nearest_channel(hz)?;
        (tolerance_hz.is_none_or(|t| err <= t)).then_some((n, b))
    })
}

/// The bands of `system` whose channel raster covers `hz`, in the order of [`bands`].
pub fn bands_at(system: CellSystem, hz: f64, prefer: &[u16]) -> Vec<&'static CellBand> {
    bands(system, prefer)
        .into_iter()
        .filter(|b| b.nearest_channel(hz).is_some())
        .collect()
}

/// The NR band that lists GSCN `gscn` in its sync raster, for SSB spacing `scs_khz` (any spacing when `None`). The bands
/// in `prefer` are tried first.
pub fn sync_band(
    gscn: u32,
    scs_khz: Option<u32>,
    prefer: &[u16],
) -> Option<(&'static CellBand, &'static SyncRaster)> {
    bands(CellSystem::Nr, prefer).into_iter().find_map(|b| {
        b.info
            .sync
            .iter()
            .find(|s| {
                gscn >= s.first
                    && gscn <= s.last
                    && (gscn - s.first).is_multiple_of(s.step.max(1))
                    && scs_khz.is_none_or(|c| c == s.scs_khz)
            })
            .map(|s| (b, s))
    })
}

/// One range of a global raster: numbers `first..=last` start at `first_hz` and are `step_hz` apart.
struct Range {
    first: u32,
    last: u32,
    first_hz: f64,
    step_hz: f64,
}

/// The NR global frequency raster, TS 38.104 table 5.4.2.1-1: (N_REF-Offs, last N_REF, F_REF-Offs, delta F_Global).
const NR_RASTER: [Range; 3] = [
    Range {
        first: 0,
        last: 599_999,
        first_hz: 0.0,
        step_hz: 5.0e3,
    },
    Range {
        first: 600_000,
        last: 2_016_666,
        first_hz: 3.0e9,
        step_hz: 15.0e3,
    },
    Range {
        first: 2_016_667,
        last: 3_279_165,
        first_hz: 24.25008e9,
        step_hz: 60.0e3,
    },
];

/// The GSCN ranges above 3 GHz, TS 38.104 table 5.4.3.1-1: (first GSCN, last GSCN, SS_REF of the first, step).
const GSCN_HIGH: [Range; 2] = [
    Range {
        first: 7499,
        last: 22_255,
        first_hz: 3.0e9,
        step_hz: 1.44e6,
    },
    Range {
        first: 22_256,
        last: 26_639,
        first_hz: 24.25008e9,
        step_hz: 17.28e6,
    },
];

fn range_hz(ranges: &[Range], n: u32) -> Option<f64> {
    let r = ranges.iter().find(|r| (r.first..=r.last).contains(&n))?;
    Some(r.first_hz + f64::from(n - r.first) * r.step_hz)
}

/// The nearest number to `hz` in each range, with its distance.
fn range_nearest(ranges: &[Range], hz: f64) -> impl Iterator<Item = (u32, f64)> + '_ {
    ranges.iter().map(move |r| {
        let k = ((hz - r.first_hz) / r.step_hz)
            .round()
            .clamp(0.0, f64::from(r.last - r.first));
        let n = r.first + k as u32;
        (n, (r.first_hz + k * r.step_hz - hz).abs())
    })
}

/// The frequency F_REF of NR-ARFCN `n`, Hz (TS 38.104 5.4.2.1: F_REF = F_REF-Offs + delta F_Global (N_REF -
/// N_REF-Offs)). `None` above 3279165.
pub fn nr_arfcn_hz(n: u32) -> Option<f64> {
    range_hz(&NR_RASTER, n)
}

/// The NR-ARFCN nearest to `hz` (TS 38.104 5.4.2.1). `None` when `hz` is not between 0 and 100 GHz.
pub fn nr_arfcn_of(hz: f64) -> Option<u32> {
    let i = if !(0.0..=100.0e9).contains(&hz) {
        return None;
    } else if hz < 3.0e9 {
        0
    } else if hz < 24.25e9 {
        1
    } else {
        2
    };
    range_nearest(&NR_RASTER[i..=i], hz).next().map(|x| x.0)
}

/// The SS block frequency SS_REF of GSCN `gscn`, Hz (TS 38.104 table 5.4.3.1-1). Below 3 GHz, GSCN = 3N + (M - 3) / 2
/// and SS_REF = N x 1200 kHz + M x 50 kHz, with N from 1 to 2499 and M one of 1, 3 or 5. `None` outside 2 to 26639.
pub fn gscn_hz(gscn: u32) -> Option<f64> {
    if (2..=7498).contains(&gscn) {
        let n = (gscn + 1) / 3;
        let m = 2 * i64::from(gscn) - 6 * i64::from(n) + 3;
        Some(f64::from(n) * 1.2e6 + m as f64 * 50.0e3)
    } else {
        range_hz(&GSCN_HIGH, gscn)
    }
}

/// The GSCN nearest to `hz` (TS 38.104 table 5.4.3.1-1). `None` when `hz` is not between 0 and 100 GHz.
pub fn gscn_of(hz: f64) -> Option<u32> {
    if !(0.0..=100.0e9).contains(&hz) {
        return None;
    }
    let n0 = (hz / 1.2e6).round() as i64;
    let low = (n0 - 1..=n0 + 1)
        .filter(|n| (1..=2499).contains(n))
        .flat_map(|n| [1i64, 3, 5].map(|m| (n, m)))
        .map(|(n, m)| {
            let g = (3 * n + (m - 3) / 2) as u32;
            (g, (n as f64 * 1.2e6 + m as f64 * 50.0e3 - hz).abs())
        });
    low.chain(range_nearest(&GSCN_HIGH, hz))
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .map(|x| x.0)
}
