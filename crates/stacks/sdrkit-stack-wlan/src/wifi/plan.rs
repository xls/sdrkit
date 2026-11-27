//! Wi-Fi channel numbering (IEEE 802.11-2020 Annex E, 802.11ax-2021 Annex E.4): bands, channels, sub-bands, preferred
//! scanning channels, wider-channel centres, operating classes, the channels a capture holds and hop lists.

use serde::{Deserialize, Serialize};

sdrkit_core::choice_enum! {
    /// A Wi-Fi band.
    #[derive(Serialize, Deserialize, PartialOrd, Ord)]
    pub enum Band {
        /// 2.4 GHz.
        G2 = ("2.4", "2.4 GHz"),
        /// 5 GHz.
        G5 = ("5", "5 GHz"),
        /// 6 GHz.
        G6 = ("6", "6 GHz"),
    }
}

impl Band {
    /// "2.4 GHz", "5 GHz", "6 GHz".
    pub fn name(self) -> &'static str {
        match self {
            Band::G2 => "2.4 GHz",
            Band::G5 => "5 GHz",
            Band::G6 => "6 GHz",
        }
    }
    /// The band of a frequency in MHz.
    pub fn of_mhz(mhz: f64) -> Option<Band> {
        if (2400.0..=2500.0).contains(&mhz) {
            Some(Band::G2)
        } else if (5150.0..=5895.0).contains(&mhz) {
            Some(Band::G5)
        } else if (5925.0..=7125.0).contains(&mhz) {
            Some(Band::G6)
        } else {
            None
        }
    }
    /// Every band.
    pub const ALL: [Band; 3] = [Band::G2, Band::G5, Band::G6];
}

/// The name of an optional band ("" for none).
pub fn band_name(b: Option<Band>) -> &'static str {
    b.map(|b| b.name()).unwrap_or("")
}

/// Whether a channel number is a valid 20 MHz channel of a band.
pub fn valid(band: Band, ch: u32) -> bool {
    match band {
        Band::G2 => (1..=14).contains(&ch),
        Band::G5 => {
            ((36..=64).contains(&ch) && ch.is_multiple_of(4))
                || ((100..=144).contains(&ch) && ch.is_multiple_of(4))
                || ((149..=177).contains(&ch) && ch % 4 == 1)
        }
        Band::G6 => ch == 2 || ((1..=233).contains(&ch) && ch % 4 == 1),
    }
}

/// The centre frequency in MHz of a channel of any width, from the channel number of its centre (the CCFS of the operation
/// elements, IEEE Std 802.11-2020 clause 21.3.14). `None` for a number outside the band.
pub fn wide_centre_mhz(band: Band, centre: u32) -> Option<f64> {
    match band {
        Band::G2 => centre_mhz(band, centre),
        Band::G5 => (32..=177)
            .contains(&centre)
            .then(|| 5000.0 + 5.0 * f64::from(centre)),
        Band::G6 if centre == 2 => Some(5935.0),
        Band::G6 => (1..=233)
            .contains(&centre)
            .then(|| 5950.0 + 5.0 * f64::from(centre)),
    }
}

/// The centre frequency of a channel in MHz, `None` when not a channel.
pub fn centre_mhz(band: Band, ch: u32) -> Option<f64> {
    if !valid(band, ch) {
        return None;
    }
    Some(match band {
        Band::G2 => {
            if ch == 14 {
                2484.0
            } else {
                2407.0 + 5.0 * ch as f64
            }
        }
        Band::G5 => 5000.0 + 5.0 * ch as f64,
        Band::G6 => {
            if ch == 2 {
                5935.0
            } else {
                5950.0 + 5.0 * ch as f64
            }
        }
    })
}

/// The channel of a band within 1 MHz (2.4 GHz) or 2.5 MHz (5 and 6 GHz) of a frequency.
pub fn channel_at(band: Band, mhz: f64) -> Option<u32> {
    let tol = if band == Band::G2 { 1.0 } else { 2.5 };
    all_channels(band)
        .into_iter()
        .find(|&c| centre_mhz(band, c).is_some_and(|f| (f - mhz).abs() <= tol + 1e-9))
}

/// Every valid 20 MHz channel of a band, ascending.
pub fn all_channels(band: Band) -> Vec<u32> {
    let mut v: Vec<u32> = (1..=233).filter(|&c| valid(band, c)).collect();
    v.sort();
    v
}

/// The UNII sub-band of a 5 or 6 GHz channel.
pub fn sub_band(band: Band, ch: u32) -> &'static str {
    match band {
        Band::G2 => "",
        Band::G5 => match ch {
            36..=48 => "UNII-1",
            52..=64 => "UNII-2A",
            100..=144 => "UNII-2C",
            149..=165 => "UNII-3",
            169..=177 => "UNII-4",
            _ => "",
        },
        Band::G6 => match ch {
            1..=93 => "UNII-5",
            97..=113 => "UNII-6",
            117..=185 => "UNII-7",
            189..=233 => "UNII-8",
            _ => "",
        },
    }
}

/// A preferred scanning channel of 6 GHz.
pub fn psc(band: Band, ch: u32) -> bool {
    band == Band::G6 && (5..=229).contains(&ch) && (ch - 5).is_multiple_of(16)
}

/// The CCFS0 centre channel of the `width` MHz block that holds a 20 MHz primary (`None` when no such block exists).
pub fn wide_centre(band: Band, primary: u32, width: u32) -> Option<u32> {
    if width == 20 {
        return valid(band, primary).then_some(primary);
    }
    match band {
        Band::G2 => (width == 40).then_some(primary + 2),
        Band::G5 => {
            let starts: &[u32] = match width {
                40 => &[
                    36, 44, 52, 60, 100, 108, 116, 124, 132, 140, 149, 157, 165, 173,
                ],
                80 => &[36, 52, 100, 116, 132, 149, 165],
                160 => &[36, 100, 149],
                _ => return None,
            };
            let span = width / 20 * 4;
            starts
                .iter()
                .find(|&&s| primary >= s && primary < s + span)
                .map(|&s| s + span / 2 - 2)
        }
        Band::G6 => {
            if primary == 2 || !valid(band, primary) {
                return None;
            }
            let span = width / 20 * 4;
            let start = 1 + (primary - 1) / span * span;
            (start + span - 4 <= 233).then_some(start + span / 2 - 2)
        }
    }
}

/// The band of a global operating class (Annex E): 81 to 84 are 2.4 GHz, 115 to 130 are 5 GHz, 131 to 137 are 6 GHz.
pub fn class_band(class: u8) -> Option<Band> {
    match class {
        81..=84 => Some(Band::G2),
        115..=130 => Some(Band::G5),
        131..=137 => Some(Band::G6),
        _ => None,
    }
}

/// The width of a global operating class in MHz (80+80 is reported as 160).
pub fn class_width(class: u8) -> Option<u32> {
    Some(match class {
        81 | 82 | 115 | 118 | 121 | 124 | 125 | 131 | 136 => 20,
        83 | 84 | 116 | 117 | 119 | 120 | 122 | 123 | 126 | 127 | 132 => 40,
        128 | 133 => 80,
        129 | 134 | 130 | 135 => 160,
        137 => 320,
        _ => return None,
    })
}

/// Whether a channel at `f_mhz` lies wholly inside a capture centred at `c_mhz` with rate `rate` (Hz).
pub fn inside(f_mhz: f64, c_mhz: f64, rate: f64) -> bool {
    (f_mhz - c_mhz).abs() + 10.0 <= 0.46 * rate / 1e6 + 1e-9
}

/// The channels of a band wholly inside a capture.
pub fn channels_in_capture(band: Band, c_mhz: f64, rate: f64) -> Vec<u32> {
    all_channels(band)
        .into_iter()
        .filter(|&ch| centre_mhz(band, ch).is_some_and(|f| inside(f, c_mhz, rate)))
        .collect()
}

/// The 2.4 GHz channels that every region permits.
pub const WORLDWIDE_24: std::ops::RangeInclusive<u32> = 1..=11;

/// Whether a 5 GHz channel needs radar detection (5250 to 5350 and 5470 to 5730 MHz): few access points use these.
pub fn dfs(band: Band, ch: u32) -> bool {
    band == Band::G5 && (52..=144).contains(&ch)
}

/// The 5 GHz channels in the order of a hop without a list: 36 to 48 and 149 to 165, where most access points are, then the
/// radar detection channels, then 169 to 177.
pub fn default_5() -> Vec<u32> {
    let mut v = all_channels(Band::G5);
    v.sort_by_key(|&c| (u8::from(dfs(Band::G5, c)) + 2 * u8::from(c > 165), c));
    v
}

/// The hop list of a band: the channels of `channels` that are valid in the band, in their order, or, when `channels` is
/// empty, the channels of the band (on 2.4 GHz the channels of [`WORLDWIDE_24`], on 5 GHz in the order of [`default_5`]); on
/// 6 GHz only the preferred scanning channels when `psc_only` is set.
pub fn hop_list(band: Band, channels: &[u32], psc_only: bool) -> Vec<u32> {
    let listed: Vec<u32> = if channels.is_empty() && band == Band::G2 {
        WORLDWIDE_24.collect()
    } else if channels.is_empty() && band == Band::G5 {
        default_5()
    } else if channels.is_empty() {
        all_channels(band)
    } else {
        channels
            .iter()
            .copied()
            .filter(|&c| valid(band, c))
            .collect()
    };
    listed
        .into_iter()
        .filter(|&c| band != Band::G6 || !psc_only || psc(band, c))
        .collect()
}

/// The channel numbers of a list text: numbers apart by spaces or commas. `None` when an entry is not a valid channel of
/// `band`.
pub fn parse_channels(band: Band, text: &str) -> Option<Vec<u32>> {
    text.split([' ', ','])
        .filter(|t| !t.is_empty())
        .map(|t| t.parse::<u32>().ok().filter(|&c| valid(band, c)))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plan_facts() {
        for (c, f) in [
            (36, 5180.0),
            (64, 5320.0),
            (100, 5500.0),
            (144, 5720.0),
            (149, 5745.0),
            (165, 5825.0),
            (177, 5885.0),
        ] {
            assert_eq!(centre_mhz(Band::G5, c), Some(f));
        }
        for c in [37, 68, 148, 34] {
            assert!(!valid(Band::G5, c));
        }
        for (c, f) in [
            (1, 5955.0),
            (5, 5975.0),
            (93, 6415.0),
            (233, 7115.0),
            (2, 5935.0),
        ] {
            assert_eq!(centre_mhz(Band::G6, c), Some(f));
        }
        assert!(!valid(Band::G6, 3));
        assert_eq!(channel_at(Band::G5, 5500.4), Some(100));
        assert_eq!(channel_at(Band::G6, 5975.0), Some(5));
        assert_eq!(channel_at(Band::G5, 5190.0), None);
        assert_eq!(channel_at(Band::G5, 5181.5), Some(36));
        for (c, f) in [(1, 2412.0), (6, 2437.0), (13, 2472.0), (14, 2484.0)] {
            assert_eq!(centre_mhz(Band::G2, c), Some(f));
        }
        assert_eq!(channel_at(Band::G2, 2437.3), Some(6));
        assert_eq!(channel_at(Band::G2, 2440.0), None);
    }

    #[test]
    fn marks_and_widths() {
        assert_eq!(sub_band(Band::G5, 44), "UNII-1");
        assert_eq!(sub_band(Band::G6, 100), "UNII-6");
        assert!(psc(Band::G6, 133) && !psc(Band::G6, 9));
        assert_eq!((1..=233).filter(|&c| psc(Band::G6, c)).count(), 15);
        let w5 = |p, w| wide_centre(Band::G5, p, w);
        assert_eq!(w5(36, 80), Some(42));
        assert_eq!(w5(48, 80), Some(42));
        assert_eq!(w5(52, 80), Some(58));
        assert_eq!(w5(100, 80), Some(106));
        assert_eq!(w5(149, 80), Some(155));
        assert_eq!(w5(165, 80), Some(171));
        assert_eq!(w5(36, 160), Some(50));
        assert_eq!(w5(64, 160), Some(50));
        assert_eq!(w5(100, 160), Some(114));
        assert_eq!(w5(149, 160), Some(163));
        assert_eq!(w5(36, 40), Some(38));
        assert_eq!(w5(40, 40), Some(38));
        assert_eq!(w5(44, 40), Some(46));
        let w6 = |p, w| wide_centre(Band::G6, p, w);
        assert_eq!(w6(5, 80), Some(7));
        assert_eq!(w6(21, 80), Some(23));
        assert_eq!(w6(5, 160), Some(15));
        assert_eq!(w6(37, 160), Some(47));
        assert_eq!(w6(5, 40), Some(3));
        assert_eq!(w6(5, 320), Some(31));
        assert_eq!(w6(5, 20), Some(5));
        assert_eq!(wide_centre(Band::G2, 6, 40), Some(8));
        assert_eq!(class_band(83), Some(Band::G2));
        assert_eq!(class_band(128), Some(Band::G5));
        assert_eq!(class_band(131), Some(Band::G6));
        assert_eq!(class_width(137), Some(320));
        assert_eq!(class_width(133), Some(80));
        assert_eq!(channels_in_capture(Band::G5, 5190.0, 50e6), vec![36, 40]);
        assert!(channels_in_capture(Band::G5, 5180.0, 20e6).is_empty());
        assert_eq!(channels_in_capture(Band::G5, 5180.0, 25e6).len(), 1);
        assert!(channels_in_capture(Band::G6, 5975.0, 80e6).len() >= 3);
        assert_eq!(hop_list(Band::G6, &[], true).len(), 15);
        // Without a list the 5 GHz hop takes the channels without radar detection first.
        let five = hop_list(Band::G5, &[], true);
        assert_eq!(five[..9], [36, 40, 44, 48, 149, 153, 157, 161, 165]);
        assert_eq!((five[9], five[24], &five[25..]), (52, 144, &[169, 173, 177][..]));
        assert_eq!(five.len(), all_channels(Band::G5).len());
        assert_eq!(
            hop_list(Band::G5, &[36, 52, 1, 149], false),
            vec![36, 52, 149]
        );
        assert_eq!(
            parse_channels(Band::G5, "36, 40 44"),
            Some(vec![36, 40, 44])
        );
        assert_eq!(parse_channels(Band::G5, "36 37"), None);
        assert_eq!(parse_channels(Band::G2, ""), Some(vec![]));
    }
}
