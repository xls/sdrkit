//! The DAB channels. The Band III and L-band blocks of ETSI EN 300 401 (`sdrkit_data_plans::dab`) name a frequency; the
//! channels that the stack may tune to are frequencies that the host sets.

/// A DAB channel.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Channel {
    /// Its name ("12B"); empty for a frequency that is not a block of EN 300 401.
    pub name: &'static str,
    /// Centre frequency in Hz.
    pub hz: f64,
}

fn channel(b: &sdrkit_data_plans::dab::DabBlock) -> Channel {
    Channel {
        name: b.name,
        hz: b.centre_hz,
    }
}

/// The 38 Band III channels, 5A to 13F.
pub fn band_iii() -> impl Iterator<Item = Channel> {
    sdrkit_data_plans::dab::BAND_III.iter().map(channel)
}

/// The 23 L-band channels, LA to LW.
pub fn l_band() -> impl Iterator<Item = Channel> {
    sdrkit_data_plans::dab::L_BAND.iter().map(channel)
}

/// The most entries of a channel list.
pub const LIST_MAX: usize = 128;

/// Every block of EN 300 401: Band III, then L-band.
pub fn blocks() -> Vec<Channel> {
    band_iii().chain(l_band()).collect()
}

/// The channel list of a setting: frequencies in Hz separated by ";", each named after the block within 50 kHz of it.
/// An empty text is every block of [`blocks`]. `None` for a malformed text or more than [`LIST_MAX`] entries.
pub fn parse_list(text: &str) -> Option<Vec<Channel>> {
    let entries: Vec<&str> = text
        .split(';')
        .map(str::trim)
        .filter(|e| !e.is_empty())
        .collect();
    if entries.is_empty() {
        return Some(blocks());
    }
    if entries.len() > LIST_MAX {
        return None;
    }
    entries
        .into_iter()
        .map(|e| {
            let hz: f64 = e.parse().ok()?;
            (hz.is_finite() && hz > 0.0).then(|| Channel {
                name: channel_at(hz).map_or("", |c| c.name),
                hz,
            })
        })
        .collect()
}

/// The channel within 50 kHz of `hz`.
pub fn channel_at(hz: f64) -> Option<Channel> {
    band_iii()
        .chain(l_band())
        .find(|c| (c.hz - hz).abs() <= 50e3)
}

/// The next (`up`) or previous channel of `table` from `hz`, wrapping at the ends (tuning by knob moves between channels).
/// `None` for an empty table.
pub fn step(table: &[Channel], hz: f64, up: bool) -> Option<Channel> {
    if up {
        table
            .iter()
            .find(|c| c.hz > hz + 50e3)
            .or(table.first())
            .copied()
    } else {
        table
            .iter()
            .rev()
            .find(|c| c.hz < hz - 50e3)
            .or(table.last())
            .copied()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tables() {
        assert_eq!(band_iii().count(), 38);
        assert_eq!(l_band().count(), 23);
        assert_eq!(
            band_iii().next().unwrap(),
            Channel {
                name: "5A",
                hz: 174.928e6
            }
        );
        assert_eq!(
            l_band().last().unwrap(),
            Channel {
                name: "LW",
                hz: 1490.624e6
            }
        );
        assert_eq!(channel_at(225.648e6).unwrap().name, "12B");
        assert_eq!(channel_at(225.690e6).unwrap().name, "12B");
        assert!(channel_at(225.75e6).is_none());
        let l: Vec<Channel> = l_band().collect();
        for w in l.windows(2) {
            assert!((w[1].hz - w[0].hz - 1.712e6).abs() < 1.0);
        }
        let b3: Vec<Channel> = band_iii().collect();
        assert_eq!(step(&b3, 225.648e6, true).unwrap().name, "12C");
        assert_eq!(step(&b3, 225.648e6, false).unwrap().name, "12A");
        assert!(step(&[], 225.648e6, true).is_none());
        assert_eq!(parse_list("").unwrap().len(), 61);
        let list = parse_list("225648000; 175280000").unwrap();
        assert_eq!((list[0].name, list[1].name), ("12B", ""));
        assert!(parse_list("12B").is_none());
    }
}
