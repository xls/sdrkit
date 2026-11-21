//! Ranging codes: the L1 C/A family (GPS, SBAS and QZSS), BeiDou B1I, the GLONASS m-sequence, the Galileo E1-B memory codes
//! (read from a table), the BeiDou NH code, and code replicas sampled at a band rate. Chips are 0 or 1. On the carrier, 0 is
//! +1 and 1 is −1.
//!
//! Standard: IS-GPS-200 3.3.2, BDS SIS ICD B1I 4.2, GLONASS ICD 3.3.1, Galileo OS SIS ICD Annex C.

use crate::system::System;
use sdrkit_algo_sim::rng::Rng;
use sdrkit_util_bits::{pack, seq};

/// The highest BeiDou ranging code this crate can make.
pub const BDS_MAX_PRN: u16 = 37;

/// The BeiDou D1 Neumann-Hoffman code, one chip per 1 ms code period.
pub const NH20: [u8; 20] = [0, 0, 0, 0, 0, 1, 0, 0, 1, 1, 0, 1, 0, 1, 0, 0, 1, 1, 1, 0];

/// The GLONASS time mark, 30 chips of 10 ms.
pub const GLO_TIME_MARK: [u8; 30] = [
    1, 1, 1, 1, 1, 0, 0, 0, 1, 1, 0, 1, 1, 1, 0, 1, 0, 1, 0, 0, 0, 0, 1, 0, 0, 1, 0, 1, 1, 0,
];

/// The L1 C/A code of a GPS (1 to 32), SBAS (120 to 158) or QZSS (193 to 202) PRN, or `None` for numbers outside the families.
pub fn l1_ca(prn: u16) -> Option<Vec<u8>> {
    let mut out = [0u8; 1023];
    seq::l1_ca(prn, &mut out).ok()?;
    Some(out.to_vec())
}

/// BeiDou B1I ranging code 1 to 37 (2046 chips: the 2047-chip Gold code without its last chip).
pub fn bds_b1i(prn: u16) -> Option<Vec<u8>> {
    let mut out = [0u8; 2046];
    seq::bds_b1i(u8::try_from(prn).ok()?, &mut out).ok()?;
    Some(out.to_vec())
}

/// The GLONASS L1OF ranging code: 511 chips of 1 + x^5 + x^9, all ones at the start, output from stage 7.
pub fn glonass_code() -> Vec<u8> {
    let mut out = vec![0u8; 511];
    let mut reg = seq::GLONASS_RANGING;
    reg.fill_bits(&mut out);
    out
}

/// Why a Galileo code table was refused.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum CodeTableError {
    /// The file does not read.
    #[error("the code table could not be read: {0:?}")]
    Read(std::io::ErrorKind),
    /// The text holds no 4092-chip code.
    #[error("the code table holds no 4092-chip code")]
    NoCodes,
}

/// The Galileo E1-B memory codes: 4092 chips per code, codes 1 to 50.
///
/// The host supplies the table as a text file of the Annex C hexadecimal values (one code per line, 1023 hex digits, with an
/// optional prefix of its number and a colon), through the `galileo_codes` setting. Without it the Galileo search stays off.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct E1bTable {
    codes: Vec<Vec<u8>>,
}

impl E1bTable {
    /// Parses the Annex C text. Lines that are not 1023 hex digits (after an optional "n:" prefix) are skipped.
    ///
    /// # Errors
    ///
    /// [`CodeTableError::NoCodes`] if no line holds a code.
    pub fn parse(text: &str) -> Result<E1bTable, CodeTableError> {
        let mut codes = Vec::new();
        for line in text.lines() {
            let l = line.trim();
            let hex = l.rsplit(':').next().unwrap_or("").trim();
            let hex: String = hex.chars().filter(|c| !c.is_whitespace()).collect();
            if hex.len() == 1023 && hex.chars().all(|c| c.is_ascii_hexdigit()) {
                let mut c = Vec::with_capacity(4092);
                if pack::hex_to_bits(&hex, &mut c).is_ok() {
                    c.truncate(4092);
                    codes.push(c);
                }
            }
        }
        if codes.is_empty() {
            return Err(CodeTableError::NoCodes);
        }
        Ok(E1bTable { codes })
    }
    /// Reads and parses a table file.
    ///
    /// # Errors
    ///
    /// [`CodeTableError::Read`] if the file does not read, [`CodeTableError::NoCodes`] if it holds no code.
    pub fn load(path: &str) -> Result<E1bTable, CodeTableError> {
        let text = std::fs::read_to_string(path).map_err(|e| CodeTableError::Read(e.kind()))?;
        Self::parse(&text)
    }
    /// A table of balanced pseudo-random 4092-chip codes, for exercising the Galileo chain in tests without the ICD table. These
    /// are **not** the Galileo codes and nothing on the air uses them.
    pub fn synthetic(seed: u64, n: usize) -> E1bTable {
        let mut rng = Rng::new(seed);
        let codes = (0..n)
            .map(|_| {
                let mut c: Vec<u8> = (0..4092).map(|i| (i % 2) as u8).collect();
                for i in (1..c.len()).rev() {
                    c.swap(i, rng.below(i as u64 + 1) as usize);
                }
                c
            })
            .collect();
        E1bTable { codes }
    }
    /// Number of codes held.
    pub fn len(&self) -> usize {
        self.codes.len()
    }
    /// No codes held.
    pub fn is_empty(&self) -> bool {
        self.codes.is_empty()
    }
    /// Code `prn` (1-based).
    pub fn code(&self, prn: u16) -> Option<&[u8]> {
        self.codes
            .get((prn as usize).checked_sub(1)?)
            .map(|c| c.as_slice())
    }
    /// The table as Annex C style text (for writing a synthetic table to a file in tests).
    pub fn to_text(&self) -> String {
        self.codes
            .iter()
            .enumerate()
            .map(|(i, c)| {
                let mut hex = String::with_capacity(1023);
                pack::bits_to_hex(c, &mut hex);
                format!("{}:{}\n", i + 1, hex)
            })
            .collect()
    }
}

/// What a band searches and tracks: the kind of signal.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SignalKind {
    /// GPS, QZSS, SBAS L1 C/A.
    L1Ca,
    /// Galileo E1-B, BOC(1,1).
    E1b,
    /// BeiDou B1I.
    B1i,
    /// GLONASS L1OF.
    G1,
}

impl SignalKind {
    /// Chips per code period.
    pub fn code_len(self) -> usize {
        match self {
            SignalKind::L1Ca => 1023,
            SignalKind::E1b => 4092,
            SignalKind::B1i => 2046,
            SignalKind::G1 => 511,
        }
    }
    /// Chip rate, chips/s.
    pub fn chip_rate(self) -> f64 {
        match self {
            SignalKind::L1Ca | SignalKind::E1b => 1.023e6,
            SignalKind::B1i => 2.046e6,
            SignalKind::G1 => 0.511e6,
        }
    }
    /// The code period, s.
    pub fn period_s(self) -> f64 {
        self.code_len() as f64 / self.chip_rate()
    }
    /// The working rate of the band, samples/s.
    pub fn band_rate(self) -> f64 {
        match self {
            SignalKind::G1 => 1.024e6,
            _ => 4.096e6,
        }
    }
    /// Samples per code period at the band rate.
    pub fn period_samples(self) -> usize {
        (self.band_rate() * self.period_s()).round() as usize
    }
    /// Early and late offset from prompt, samples.
    pub fn el_offset(self) -> usize {
        match self {
            SignalKind::L1Ca => 2,
            _ => 1,
        }
    }
    /// The kind a system's signal is.
    pub fn of(system: System) -> SignalKind {
        match system {
            System::Galileo => SignalKind::E1b,
            System::Beidou => SignalKind::B1i,
            System::Glonass => SignalKind::G1,
            _ => SignalKind::L1Ca,
        }
    }
}

/// Samples a code at `fs` over one period of `n` samples: each sample takes the chip under its centre (for BOC(1,1) the half chip,
/// chip XOR half), as +1.0 or −1.0.
pub fn sample_code(chips: &[u8], chip_rate: f64, fs: f64, n: usize, boc: bool) -> Vec<f32> {
    let len = chips.len();
    (0..n)
        .map(|i| {
            let t = (i as f64 + 0.5) / fs;
            let v = if boc {
                let h = (t * chip_rate * 2.0).floor() as usize;
                chips[(h / 2) % len] ^ (h % 2) as u8
            } else {
                chips[((t * chip_rate).floor() as usize) % len]
            };
            if v == 0 {
                1.0
            } else {
                -1.0
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pm(c: &[u8]) -> Vec<i32> {
        c.iter().map(|&b| if b == 0 { 1 } else { -1 }).collect()
    }
    fn xcorr(a: &[i32], b: &[i32], shift: usize) -> i32 {
        (0..a.len()).map(|i| a[i] * b[(i + shift) % b.len()]).sum()
    }

    #[test]
    fn gps_codes() {
        // The chips themselves are tested in sdrkit_util_bits::seq; here the correlation properties the search relies on.
        for prn in 1..=32u16 {
            let p = pm(&l1_ca(prn).unwrap());
            assert_eq!(xcorr(&p, &p, 0), 1023);
        }
        assert!(l1_ca(0).is_none() && l1_ca(33).is_none());
        let a = pm(&l1_ca(1).unwrap());
        let b = pm(&l1_ca(2).unwrap());
        for s in 0..1023 {
            assert!([-65, -1, 63].contains(&xcorr(&a, &b, s)));
        }
    }

    #[test]
    fn beidou_codes() {
        let mut all = Vec::new();
        for prn in 1..=BDS_MAX_PRN {
            let c = bds_b1i(prn).unwrap();
            assert_eq!(c.len(), 2046);
            let ones = c.iter().filter(|&&b| b == 1).count() as i32;
            assert!((ones - 1023).abs() <= 33, "balance {prn}: {ones}");
            all.push(pm(&c));
        }
        assert_eq!(all[0].len(), 2046);
        assert!(bds_b1i(0).is_none() && bds_b1i(38).is_none());
    }

    #[test]
    fn glonass_ranging_code() {
        let c = glonass_code();
        assert_eq!(c.len(), 511);
        assert_eq!(c.iter().filter(|&&b| b == 1).count(), 256);
        let p = pm(&c);
        for s in 1..511 {
            assert_eq!(xcorr(&p, &p, s), -1);
        }
    }

    #[test]
    fn e1b_table_parses_and_sampling_carries_boc() {
        let t = E1bTable::synthetic(7, 3);
        let back = E1bTable::parse(&t.to_text()).unwrap();
        assert_eq!(back, t);
        let c = t.code(1).unwrap();
        assert_eq!(c.iter().filter(|&&b| b == 1).count(), 2046);
        let r = sample_code(c, 1.023e6, 4.096e6, 16384, true);
        // Each chip lasts ~4 samples: + then − for half a chip each.
        let s0 = if c[0] == 0 { 1.0 } else { -1.0 };
        assert_eq!(r[0], s0);
        assert_eq!(r[1], s0);
        assert_eq!(r[2], -s0);
        assert!(E1bTable::parse("hello").is_err());
    }
}
