//! Where each channel comes from: a bin of the shared filterbank, its own channel filter when it is too wide for a bin, or
//! nothing when it is outside the capture.

use crate::pfb;

/// The part of the capture a channel may use, as a fraction of the rate on each side of the centre (the radio's own filter rolls off
/// beyond it).
pub const USABLE_FRACTION: f64 = 0.45;

/// The fewest bins the filterbank is planned with. A channel wider than the flat part of a bin at this size gets its own filter.
pub const MIN_BINS: usize = 8;

/// Where one channel's samples come from.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Path {
    /// A bin of the filterbank, then a shift by `-residual_hz`.
    Bin {
        /// The bin.
        bin: usize,
        /// The channel's distance from the bin's centre.
        residual_hz: f64,
    },
    /// Its own mixer and decimator from the capture: the channel is too wide for a bin.
    Direct,
    /// Outside the usable part of the capture: not decoded until a retune brings it back.
    Outside,
}

/// The filterbank size (if any) and every channel's path.
#[derive(Clone, Debug, PartialEq)]
pub struct Plan {
    /// The number of bins, when the filterbank is used.
    pub bins: Option<usize>,
    /// One per channel, in the order given.
    pub paths: Vec<Path>,
}

/// Whether a channel at `offset_hz` of `bandwidth_hz` lies in the usable part of a capture at `rate`.
pub fn inside(rate: f64, offset_hz: f64, bandwidth_hz: f64) -> bool {
    offset_hz.abs() + bandwidth_hz / 2.0 <= USABLE_FRACTION * rate
}

/// Plans `channels` (offset from the capture's centre and width, in Hz) on a capture at `rate`, with at most `max_bins` bins.
///
/// Every channel that fits a bin at [`MIN_BINS`] uses the filterbank. The filterbank gets the largest power of two of bins (up to
/// `max_bins`) for which every such channel lies in the flat part of its nearest bin.
pub fn plan(rate: f64, channels: &[(f64, f64)], max_bins: usize) -> Plan {
    let max_bins = max_bins.max(MIN_BINS).next_power_of_two();
    let eligible: Vec<bool> = channels
        .iter()
        .map(|&(o, b)| inside(rate, o, b) && b <= pfb::PASS_FRACTION * rate / MIN_BINS as f64)
        .collect();
    let bins = eligible.iter().any(|&e| e).then(|| {
        let mut m = max_bins;
        loop {
            let all = channels
                .iter()
                .zip(&eligible)
                .filter(|(_, &e)| e)
                .all(|(&(o, b), _)| pfb::fits(rate, m, o, b));
            if all || m <= MIN_BINS {
                break m;
            }
            m /= 2;
        }
    });
    let paths = channels
        .iter()
        .zip(&eligible)
        .map(|(&(o, b), &e)| match (inside(rate, o, b), e, bins) {
            (false, _, _) => Path::Outside,
            (true, true, Some(m)) if pfb::fits(rate, m, o, b) => {
                let (bin, residual_hz) = pfb::nearest(rate, m, o);
                Path::Bin { bin, residual_hz }
            }
            _ => Path::Direct,
        })
        .collect();
    Plan { bins, paths }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn narrow_channels_share_a_large_filterbank() {
        let rate = 2.4e6;
        let ch: Vec<(f64, f64)> = (0..32)
            .map(|i| (-1.0e6 + i as f64 * 62.5e3, 12.5e3))
            .collect();
        let p = plan(rate, &ch, 4096);
        let m = p.bins.unwrap();
        assert!(m >= 64, "{m}");
        assert!(p.paths.iter().all(|x| matches!(x, Path::Bin { .. })));
        for (&(o, b), path) in ch.iter().zip(&p.paths) {
            if let Path::Bin { bin, residual_hz } = *path {
                assert!((pfb::bin_offset(rate, m, bin) + residual_hz - o).abs() < 1e-6);
                assert!(residual_hz.abs() + b / 2.0 <= 0.75 * rate / m as f64 + 1e-9);
            }
        }
    }

    #[test]
    fn wide_or_outside_channels_go_their_own_way() {
        let rate = 2.4e6;
        // Two narrow channels share the filterbank.
        let p = plan(rate, &[(0.0, 25e3), (100e3, 25e3)], 4096);
        assert!(p.bins.is_some());
        assert!(p.paths.iter().all(|x| matches!(x, Path::Bin { .. })));
        // A 400 kHz channel is too wide for a bin; one at 1.15 MHz is outside.
        let p = plan(
            rate,
            &[
                (0.0, 12.5e3),
                (50e3, 12.5e3),
                (75e3, 12.5e3),
                (-300e3, 400e3),
                (1.15e6, 12.5e3),
            ],
            4096,
        );
        assert!(matches!(p.paths[0], Path::Bin { .. }));
        assert_eq!(p.paths[3], Path::Direct);
        assert_eq!(p.paths[4], Path::Outside);
        let p = plan(rate, &[(-300e3, 400e3)], 4096);
        assert_eq!(p.bins, None);
    }
}
