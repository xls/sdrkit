//! The 802.11ah (S1G, "HaLow") numerology (IEEE Std 802.11-2020 clause 23): channel widths, FFT sizes, subcarrier plans, the
//! MCS table, data bits per symbol and rates.
//!
//! S1G is the VHT PHY clocked ten times slower: subcarriers are 31.25 kHz apart and a symbol lasts 40 µs (36 µs with the short
//! guard interval). The 2, 4, 8 and 16 MHz plans are the VHT 20, 40, 80 and 160 MHz plans (clause 21.3.10.10). The 1 MHz plan
//! has 32 subcarriers, 24 data and 2 pilots (clause 23.3.8.3). MCS 10 is BPSK rate 1/2 sent twice, at 1 MHz only.
//!
//! ```
//! use sdrkit_proto_wlan_phy::s1g;
//! assert_eq!(s1g::n_dbps(1, 0, 1), Some(12));
//! assert_eq!(s1g::rate_bps(1, 10, 1, false), Some(150_000.0));
//! assert_eq!(s1g::rate_bps(2, 9, 1, false), None); // 52 * 8 * 5 / 6 is not whole
//! assert_eq!(s1g::plan(2).unwrap().data.len(), 52);
//! ```

use crate::{Error, Param, Result};

/// The channel widths, MHz.
pub const WIDTHS: [u32; 5] = [1, 2, 4, 8, 16];
/// The subcarrier spacing, Hz.
pub const SPACING_HZ: f64 = 31_250.0;
/// The symbol length with the normal guard interval, seconds.
pub const SYMBOL_S: f64 = 40e-6;
/// The symbol length with the short guard interval, seconds.
pub const SYMBOL_SGI_S: f64 = 36e-6;

/// The FFT size of a width (32 per MHz, 32 for 1 MHz). `None` for another width.
pub fn fft_size(width_mhz: u32) -> Option<usize> {
    WIDTHS
        .contains(&width_mhz)
        .then_some(32 * width_mhz as usize)
}

/// The subcarriers of a width: used, pilots and data, each in increasing order.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TonePlan {
    /// The width, MHz.
    pub width_mhz: u32,
    /// Pilot subcarrier indices.
    pub pilots: Vec<i32>,
    /// Data subcarrier indices.
    pub data: Vec<i32>,
}

impl TonePlan {
    /// Every used subcarrier (pilots and data), in increasing order.
    pub fn used(&self) -> Vec<i32> {
        let mut v: Vec<i32> = self.pilots.iter().chain(&self.data).copied().collect();
        v.sort_unstable();
        v
    }
}

fn mirrored(lo: i32, hi: i32, shift: i32, out: &mut Vec<i32>) {
    out.extend((lo..=hi).map(|k| shift - k));
    out.extend((lo..=hi).map(|k| shift + k));
}

/// The subcarrier plan of a width (clause 23.3.8.3). The 2 to 16 MHz plans are the VHT plans of clause 21.3.7.2 and 21.3.10.10.
pub fn plan(width_mhz: u32) -> Result<TonePlan> {
    let mut used = Vec::new();
    let pilots: Vec<i32> = match width_mhz {
        1 => {
            mirrored(1, 13, 0, &mut used);
            vec![-7, 7]
        }
        2 => {
            mirrored(1, 28, 0, &mut used);
            vec![-21, -7, 7, 21]
        }
        4 => {
            mirrored(2, 58, 0, &mut used);
            vec![-53, -25, -11, 11, 25, 53]
        }
        8 => {
            mirrored(2, 122, 0, &mut used);
            vec![-103, -75, -39, -11, 11, 39, 75, 103]
        }
        16 => {
            mirrored(2, 122, -128, &mut used);
            mirrored(2, 122, 128, &mut used);
            let p8 = [-103, -75, -39, -11, 11, 39, 75, 103];
            p8.iter()
                .map(|k| k - 128)
                .chain(p8.iter().map(|k| k + 128))
                .collect()
        }
        _ => return Err(Error::BadParameter(Param::S1gWidth)),
    };
    used.sort_unstable();
    let data = used
        .iter()
        .copied()
        .filter(|k| !pilots.contains(k))
        .collect();
    Ok(TonePlan {
        width_mhz,
        pilots,
        data,
    })
}

/// Data subcarriers of a width (N_SD).
pub fn n_sd(width_mhz: u32) -> Option<usize> {
    Some(match width_mhz {
        1 => 24,
        2 => 52,
        4 => 108,
        8 => 234,
        16 => 468,
        _ => return None,
    })
}

/// The interleaver columns of a width (N_COL). The 16 MHz width is interleaved as two 8 MHz segments.
pub fn n_col(width_mhz: u32) -> Option<usize> {
    Some(match width_mhz {
        1 => 8,
        2 => 13,
        4 => 18,
        8 | 16 => 26,
        _ => return None,
    })
}

/// Bits per subcarrier and the coding rate (numerator, denominator) of an MCS (clause 23.5, tables 23-41 to 23-80). MCS 10 is
/// MCS 0 sent twice.
pub fn mcs(mcs: u8) -> Option<(usize, (usize, usize))> {
    Some(match mcs {
        0 | 10 => (1, (1, 2)),
        1 => (2, (1, 2)),
        2 => (2, (3, 4)),
        3 => (4, (1, 2)),
        4 => (4, (3, 4)),
        5 => (6, (2, 3)),
        6 => (6, (3, 4)),
        7 => (6, (5, 6)),
        8 => (8, (3, 4)),
        9 => (8, (5, 6)),
        _ => return None,
    })
}

/// Returns the data bits per symbol (N_DBPS) of a width, MCS and number of spatial streams (1 to 4). Returns `None` when the
/// result is not a whole number, or for MCS 10 at a width other than 1 MHz.
pub fn n_dbps(width_mhz: u32, mcs_index: u8, nss: u8) -> Option<usize> {
    if !(1..=4).contains(&nss) {
        return None;
    }
    let (bits, (a, b)) = mcs(mcs_index)?;
    let sd = n_sd(width_mhz)?;
    if mcs_index == 10 {
        return (width_mhz == 1).then_some(6 * nss as usize);
    }
    let x = sd * bits * nss as usize * a;
    x.is_multiple_of(b).then_some(x / b)
}

/// The number of BCC encoders in parallel (N_ES). This function uses one encoder for each 2400 data bits per symbol. That is
/// the VHT rule of one encoder for each 600 Mbit/s, at the 10 times slower S1G clock. The MCS tables of the standard give N_ES
/// for each combination.
pub fn n_es(n_dbps: usize) -> usize {
    n_dbps.div_ceil(2400).max(1)
}

/// Whether a width, MCS and stream count is a combination the PHY sends: N_DBPS whole and a multiple of N_ES (the rule of
/// clause 21.5 that excludes the VHT combinations).
pub fn valid(width_mhz: u32, mcs_index: u8, nss: u8) -> bool {
    n_dbps(width_mhz, mcs_index, nss).is_some_and(|d| d % n_es(d) == 0)
}

/// The data rate in bit/s, `None` for a combination the PHY does not send.
pub fn rate_bps(width_mhz: u32, mcs_index: u8, nss: u8, short_gi: bool) -> Option<f64> {
    if !valid(width_mhz, mcs_index, nss) {
        return None;
    }
    let d = n_dbps(width_mhz, mcs_index, nss)? as f64;
    // 25,000 symbols per second with the normal guard interval, 1e6 / 36 with the short one.
    Some(if short_gi {
        d * 1e6 / 36.0
    } else {
        d * 25_000.0
    })
}
