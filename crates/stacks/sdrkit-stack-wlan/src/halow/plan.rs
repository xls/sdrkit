//! S1G channel numbering (IEEE 802.11-2020 Annex E): channel numbers on a channel starting frequency, the channels of
//! each width that the host lists, and the frequencies that operating classes name.

/// The widths of S1G channels, MHz.
pub const WIDTHS_MHZ: [u32; 5] = [1, 2, 4, 8, 16];

/// A channel of the plan.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Channel {
    /// Channel number.
    pub number: u32,
    /// Centre, MHz.
    pub mhz: f64,
    /// Width, MHz.
    pub width: u32,
}

/// The channel plan that the host sets: a channel starting frequency and the channel numbers of each width. Channel n is
/// centred at the starting frequency plus n x 500 kHz.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Plan {
    /// Channel starting frequency, MHz. 0: no plan.
    pub start: f64,
    /// The channel numbers of each width of [`WIDTHS_MHZ`], in the same order.
    pub numbers: [Vec<u32>; 5],
}

impl Plan {
    /// The channels of a width, in the order of the list.
    pub fn channels(&self, width: u32) -> Vec<Channel> {
        let Some(i) = WIDTHS_MHZ.iter().position(|&w| w == width) else {
            return Vec::new();
        };
        if self.start <= 0.0 {
            return Vec::new();
        }
        self.numbers[i]
            .iter()
            .map(|&number| Channel {
                number,
                mhz: self.start + 0.5 * f64::from(number),
                width,
            })
            .collect()
    }
    /// The nearest channel of a width within 1 MHz.
    pub fn nearest(&self, width: u32, mhz: f64) -> Option<Channel> {
        self.channels(width)
            .into_iter()
            .filter(|c| (c.mhz - mhz).abs() <= 1.0)
            .min_by(|a, b| {
                (a.mhz - mhz)
                    .abs()
                    .partial_cmp(&(b.mhz - mhz).abs())
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
    }
}

/// The channel numbers of a list text: numbers from 1 apart by spaces or commas. `None` when an entry is not one.
pub fn parse_numbers(text: &str) -> Option<Vec<u32>> {
    text.split([' ', ','])
        .filter(|t| !t.is_empty())
        .map(|t| t.parse::<u32>().ok().filter(|&n| n > 0))
        .collect()
}

/// The channel starting frequency of an S1G operating class (Annex E), as a received element names it.
pub fn class_start(class: u8) -> Option<f64> {
    Some(match class {
        1..=5 | 17..=20 | 26..=29 => 902.0,
        6 | 7 | 14..=16 | 24 | 25 => 863.0,
        8..=10 => 916.5,
        11..=13 => 917.5,
        21..=23 => 755.0,
        _ => return None,
    })
}

/// The frequency of a channel named by an operating class.
pub fn class_channel_mhz(class: u8, number: u32) -> Option<f64> {
    class_start(class).map(|s| s + 0.5 * number as f64)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plan_facts() {
        let p = Plan {
            start: 863.0,
            numbers: [vec![1, 3, 5, 7, 9], vec![2, 6], vec![], vec![], vec![]],
        };
        let e1 = p.channels(1);
        assert_eq!(e1.len(), 5);
        assert_eq!((e1[0].mhz, e1[4].mhz), (863.5, 867.5));
        assert_eq!(p.channels(2).len(), 2);
        assert!(p.channels(4).is_empty() && p.channels(3).is_empty());
        assert_eq!(p.nearest(1, 864.4).unwrap().number, 3);
        assert!(p.nearest(1, 880.0).is_none());
        assert!(Plan::default().channels(1).is_empty());
        assert_eq!(parse_numbers("1, 3 5"), Some(vec![1, 3, 5]));
        assert_eq!(parse_numbers("1 x"), None);
        assert_eq!(parse_numbers("0"), None);
        assert_eq!(class_channel_mhz(6, 3), Some(864.5));
        assert_eq!(class_channel_mhz(2, 4), Some(904.0));
        assert_eq!(class_channel_mhz(99, 4), None);
    }
}
