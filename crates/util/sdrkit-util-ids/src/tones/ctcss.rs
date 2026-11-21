//! CTCSS (continuous tone-coded squelch system) tones.
//!
//! The 38 tones of the EIA standard (EIA RS-220-A, carried into TIA-603) and the 12 further tones that radios commonly offer,
//! 50 in all, with the Motorola "PL" code of each where one is assigned. Source: Midian Electronics "Tone Signaling Charts"
//! (RS-220-A EIA Standard CTCSS Tone Frequencies, with the Motorola reed codes, and the non-standard tones), and the 50-tone list
//! of TIA-603. Midian prints the first non-standard tone as 69.4 Hz; TIA-603 and radio makers use 69.3 Hz, which is kept.
//!
//! ```
//! use sdrkit_util_ids::tones::ctcss;
//!
//! assert_eq!(ctcss::nearest(123.4, 1.2).map(|t| t.hz), Some(123.0));
//! assert_eq!(ctcss::by_code("1Z").map(|t| t.hz), Some(100.0));
//! assert_eq!(ctcss::TONES.len(), 50);
//! ```

/// One CTCSS tone.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Tone {
    /// The frequency, Hz.
    pub hz: f32,
    /// One of the 38 EIA RS-220-A tones (otherwise one of the 12 further tones).
    pub eia: bool,
    /// The Motorola PL code, when one is assigned.
    pub motorola: Option<&'static str>,
}

const fn t(hz: f32, eia: bool, motorola: &'static str) -> Tone {
    Tone {
        hz,
        eia,
        motorola: if motorola.is_empty() {
            None
        } else {
            Some(motorola)
        },
    }
}

/// The 50 tones in ascending order.
pub static TONES: [Tone; 50] = [
    t(67.0, true, "XZ"),
    t(69.3, false, ""),
    t(71.9, true, "XA"),
    t(74.4, true, "WA"),
    t(77.0, true, "XB"),
    t(79.7, true, "SP"),
    t(82.5, true, "YZ"),
    t(85.4, true, "YA"),
    t(88.5, true, "YB"),
    t(91.5, true, "ZZ"),
    t(94.8, true, "ZA"),
    t(97.4, true, "ZB"),
    t(100.0, true, "1Z"),
    t(103.5, true, "1A"),
    t(107.2, true, "1B"),
    t(110.9, true, "2Z"),
    t(114.8, true, "2A"),
    t(118.8, true, "2B"),
    t(123.0, true, "3Z"),
    t(127.3, true, "3A"),
    t(131.8, true, "3B"),
    t(136.5, true, "4Z"),
    t(141.3, true, "4A"),
    t(146.2, true, "4B"),
    t(151.4, true, "5Z"),
    t(156.7, true, "5A"),
    t(159.8, false, ""),
    t(162.2, true, "5B"),
    t(165.5, false, ""),
    t(167.9, true, "6Z"),
    t(171.3, false, ""),
    t(173.8, true, "6A"),
    t(177.3, false, ""),
    t(179.9, true, "6B"),
    t(183.5, false, ""),
    t(186.2, true, "7Z"),
    t(189.9, false, ""),
    t(192.8, true, "7A"),
    t(196.6, false, ""),
    t(199.5, false, ""),
    t(203.5, true, "M1"),
    t(206.5, false, ""),
    t(210.7, true, "M2"),
    t(218.1, true, "M3"),
    t(225.7, true, "M4"),
    t(229.1, false, ""),
    t(233.6, true, ""),
    t(241.8, true, ""),
    t(250.3, true, ""),
    t(254.1, false, ""),
];

/// The tone nearest to `hz` when it is within `tolerance_hz`. Does not allocate.
pub fn nearest(hz: f32, tolerance_hz: f32) -> Option<Tone> {
    if !hz.is_finite() || tolerance_hz.is_nan() || tolerance_hz < 0.0 {
        return None;
    }
    let i = TONES.partition_point(|t| t.hz < hz);
    let cands = [i.checked_sub(1), Some(i)];
    cands
        .into_iter()
        .flatten()
        .filter_map(|j| TONES.get(j))
        .filter(|t| (t.hz - hz).abs() <= tolerance_hz)
        .min_by(|a, b| (a.hz - hz).abs().total_cmp(&(b.hz - hz).abs()))
        .copied()
}

/// The tone of a Motorola PL code (either case).
pub fn by_code(code: &str) -> Option<Tone> {
    TONES
        .iter()
        .find(|t| {
            t.motorola
                .is_some_and(|m| m.eq_ignore_ascii_case(code.trim()))
        })
        .copied()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The 38 tones of EIA RS-220-A (TIA-603 table of standard CTCSS frequencies), in order.
    #[test]
    fn eia_known_answer() {
        let eia: Vec<f32> = TONES.iter().filter(|t| t.eia).map(|t| t.hz).collect();
        assert_eq!(
            eia,
            [
                67.0, 71.9, 74.4, 77.0, 79.7, 82.5, 85.4, 88.5, 91.5, 94.8, 97.4, 100.0, 103.5,
                107.2, 110.9, 114.8, 118.8, 123.0, 127.3, 131.8, 136.5, 141.3, 146.2, 151.4, 156.7,
                162.2, 167.9, 173.8, 179.9, 186.2, 192.8, 203.5, 210.7, 218.1, 225.7, 233.6, 241.8,
                250.3
            ]
        );
    }

    #[test]
    fn the_50_tones() {
        assert!(TONES.windows(2).all(|w| w[0].hz < w[1].hz));
        assert_eq!(TONES.iter().filter(|t| t.eia).count(), 38);
        let extended: Vec<f32> = TONES.iter().filter(|t| !t.eia).map(|t| t.hz).collect();
        assert_eq!(
            extended,
            [69.3, 159.8, 165.5, 171.3, 177.3, 183.5, 189.9, 196.6, 199.5, 206.5, 229.1, 254.1]
        );
        for t in TONES {
            assert_eq!(nearest(t.hz + 0.4, 1.2), Some(t));
            assert_eq!(nearest(t.hz - 0.4, 1.2), Some(t));
        }
        assert_eq!(nearest(140.0, 1.2), None);
        assert_eq!(nearest(f32::NAN, 1.2), None);
        assert_eq!(nearest(100.0, f32::NAN), None);
        assert_eq!(by_code("zb").unwrap().hz, 97.4);
        assert_eq!(by_code("M4").unwrap().hz, 225.7);
        assert_eq!(by_code("QQ"), None);
    }
}
