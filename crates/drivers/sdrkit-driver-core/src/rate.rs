//! Sample-rate choice as pure functions of the limits of a radio and the needs of a stack. The driver has the last word on
//! the rate it runs.

use crate::{fmt_m, RateLimits};
use sdrkit_core::Hz;

/// The rates offered in the dropdown when a radio takes any rate in a range (Msps).
pub const LADDER_MSPS: [f64; 17] = [
    0.25, 0.5, 1.0, 1.024, 2.0, 2.048, 2.4, 2.56, 3.2, 4.0, 5.0, 6.0, 8.0, 10.0, 12.5, 16.0, 20.0,
];

/// Added to the ladder when the radio's maximum is above 20 Msps (those within the maximum).
pub const LADDER_FAST_MSPS: [f64; 6] = [25.0, 30.72, 40.0, 50.0, 56.0, 61.44];

/// The ranges that count: those not starting above `max + 1`, each cut at `max` (when the max is known).
fn usable_ranges(l: &RateLimits) -> Vec<(Hz, Hz)> {
    l.ranges
        .iter()
        .map(|&(a, b)| (a.min(b), a.max(b)))
        .filter(|&(lo, _)| l.max_hz <= 0.0 || lo <= l.max_hz + 1.0)
        .map(|(lo, hi)| {
            if l.max_hz > 0.0 {
                (lo, hi.min(l.max_hz))
            } else {
                (lo, hi)
            }
        })
        .collect()
}

/// The rate a radio actually runs at when `r` is requested.
///
/// With ranges: the smallest range-clamped value at or above `r - 1`, else the fastest value. Without: `r` clamped to the known
/// bounds.
pub fn delivered_rate(l: &RateLimits, r: Hz) -> Hz {
    let ranges = usable_ranges(l);
    if ranges.is_empty() {
        let mut v = r;
        if l.max_hz > 0.0 {
            v = v.min(l.max_hz);
        }
        if l.min_hz > 0.0 {
            v = v.max(l.min_hz);
        }
        return v;
    }
    let clamped = ranges.iter().map(|&(lo, hi)| r.clamp(lo, hi));
    clamped
        .clone()
        .filter(|&v| v >= r - 1.0)
        .fold(None, |best: Option<Hz>, v| {
            Some(best.map_or(v, |b| b.min(v)))
        })
        .unwrap_or_else(|| clamped.fold(f64::MIN, f64::max))
}

/// One entry of the rate dropdown.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RateChoice {
    /// What is sent to the radio.
    pub asked_hz: Hz,
    /// What the radio then runs.
    pub delivered_hz: Hz,
}

/// The rate dropdown: the radio's single rates, or the ladder, without rates the radio cannot give or that are below the stack's
/// minimum, without duplicates, sorted by delivered rate. `mode_min_hz` 0 means no minimum.
pub fn rate_list(l: &RateLimits, mode_min_hz: Hz) -> Vec<RateChoice> {
    let ranges = usable_ranges(l);
    let all_single = !ranges.is_empty() && ranges.iter().all(|&(lo, hi)| (hi - lo).abs() <= 1.0);
    let candidates: Vec<Hz> = if all_single {
        ranges.iter().map(|r| r.0).collect()
    } else {
        let mut v: Vec<Hz> = LADDER_MSPS.iter().map(|m| m * 1e6).collect();
        if l.max_hz > 20e6 {
            v.extend(
                LADDER_FAST_MSPS
                    .iter()
                    .map(|m| m * 1e6)
                    .filter(|&r| r <= l.max_hz + 1.0),
            );
        }
        v
    };
    let mut out: Vec<RateChoice> = Vec::new();
    for asked in candidates {
        if (l.min_hz > 0.0 && asked < l.min_hz - 1.0) || (l.max_hz > 0.0 && asked > l.max_hz + 1.0)
        {
            continue;
        }
        let delivered = delivered_rate(l, asked);
        if mode_min_hz > 0.0 && delivered < mode_min_hz - 1.0 {
            continue;
        }
        if !all_single && !ranges.is_empty() && (delivered - asked).abs() > 1.0 {
            continue;
        }
        if out
            .iter()
            .any(|c| (c.delivered_hz - delivered).abs() <= 1.0)
        {
            continue;
        }
        out.push(RateChoice {
            asked_hz: asked,
            delivered_hz: delivered,
        });
    }
    out.sort_by(|a, b| a.delivered_hz.total_cmp(&b.delivered_hz));
    out
}

/// The baseband filter to ask for: the stack's filter `filter_hz`, meant for the stack's automatic rate `auto_rate_hz` (0 when
/// unknown), when the radio runs at `rate_hz`. 0 means the radio's own choice.
pub fn baseband_filter(filter_hz: Hz, auto_rate_hz: Hz, rate_hz: Hz) -> Hz {
    if filter_hz <= 0.0
        || filter_hz > rate_hz
        || (auto_rate_hz > 0.0 && rate_hz > auto_rate_hz * 1.01)
    {
        0.0
    } else {
        filter_hz
    }
}

sdrkit_core::choice_enum! {
    /// What carries the samples of a radio at its default USB address.
    pub enum LinkKind {
        /// Take the default address as the USB cable.
        Auto = ("auto", "Auto"),
        /// The USB cable.
        Usb = ("usb", "USB cable"),
        /// Ethernet (a Pluto+ or a USB Ethernet adapter): no cap.
        Ethernet = ("ethernet", "Ethernet"),
    }
}

/// The steady link rate assumed for a Pluto on its USB cable when none is known.
pub const PLUTO_USB_CAP_HZ: Hz = 3e6;

/// The link cap: what the link carries without loss; 0 means no cap. `steady_hz` is the radio's known steady rate.
/// `default_address` says the radio is at its USB-gadget address, where the `link` setting decides. The cap doubles with CS8.
pub fn link_cap(steady_hz: Option<Hz>, default_address: bool, link: LinkKind, cs8: bool) -> Hz {
    let cap = if default_address {
        match link {
            LinkKind::Ethernet => 0.0,
            LinkKind::Usb | LinkKind::Auto => {
                steady_hz.filter(|&s| s > 0.0).unwrap_or(PLUTO_USB_CAP_HZ)
            }
        }
    } else {
        steady_hz.unwrap_or(0.0).max(0.0)
    };
    if cs8 {
        cap * 2.0
    } else {
        cap
    }
}

/// The rate to run on a capped link: `hard` = min(stack rate, radio max); within the cap when the stack's minimum fits it,
/// else `hard` (some loss beats no decode). `max_hz` and `cap_hz` 0 mean none.
pub fn rate_on_link(mode_rate_hz: Hz, mode_min_hz: Hz, max_hz: Hz, cap_hz: Hz) -> Hz {
    let hard = if max_hz > 0.0 {
        mode_rate_hz.min(max_hz)
    } else {
        mode_rate_hz
    };
    if cap_hz <= 0.0 {
        hard
    } else if mode_min_hz <= cap_hz {
        hard.min(cap_hz)
    } else {
        hard
    }
}

/// The note shown when a Pluto runs above its link cap, or None when the rate fits.
pub fn link_note(rate_hz: Hz, cap_hz: Hz, cs8: bool, tezuka: bool) -> Option<String> {
    if cap_hz <= 0.0 || rate_hz <= cap_hz * 1.01 {
        return None;
    }
    let advice = if cs8 {
        "Connect the Pluto over Ethernet (a USB Ethernet adapter on the Pluto) to stop it."
    } else if tezuka {
        "Set the radio's IQ format to CS8 (half the data, up to 8 Msps over USB), or connect the Pluto over Ethernet."
    } else {
        "Connect the Pluto over Ethernet, or install the Tezuka firmware and set the IQ format to CS8 (up to 8 Msps over USB)."
    };
    Some(format!(
        "PlutoSDR over its USB cable carries about {} Msps without loss; this mode runs it at {} Msps, so samples are lost and the \
         picture or sound may break up. {advice}",
        fmt_m(cap_hz),
        fmt_m(rate_hz)
    ))
}

/// A DVB-T channel's native rate: 64/7 Msps x (bandwidth MHz / 8).
pub fn dvb_native_rate(bandwidth_mhz: f64) -> Hz {
    64e6 / 7.0 * bandwidth_mhz / 8.0
}

#[cfg(test)]
mod tests {
    use super::*;

    const M: f64 = 1e6;

    fn rtl() -> RateLimits {
        RateLimits {
            min_hz: 225_001.0,
            max_hz: 2.56 * M,
            ranges: vec![(225_001.0, 0.3 * M), (900_001.0, 2.56 * M)],
            steady_hz: None,
        }
    }
    fn span(lo: f64, hi: f64) -> RateLimits {
        RateLimits {
            min_hz: lo,
            max_hz: hi,
            ..Default::default()
        }
    }

    #[test]
    fn delivered_rates_on_rtl_ranges() {
        assert_eq!(delivered_rate(&rtl(), 0.5 * M), 900_001.0);
        assert_eq!(delivered_rate(&rtl(), 0.28 * M), 0.28 * M);
        assert_eq!(delivered_rate(&rtl(), 3.0 * M), 2.56 * M);
        assert_eq!(delivered_rate(&span(2.0 * M, 20.0 * M), 1.0 * M), 2.0 * M);
        assert_eq!(delivered_rate(&RateLimits::default(), 7.0 * M), 7.0 * M);
    }

    #[test]
    fn a_range_starting_above_the_max_is_ignored() {
        let mut l = rtl();
        l.ranges.push((3.2 * M, 3.2 * M));
        assert_eq!(delivered_rate(&l, 3.2 * M), 2.56 * M);
    }

    fn msps(list: &[RateChoice]) -> Vec<f64> {
        list.iter()
            .map(|c| (c.delivered_hz / 1e3).round() / 1e3)
            .collect()
    }

    #[test]
    fn the_dropdown_for_a_range_radio() {
        assert_eq!(
            msps(&rate_list(&span(2.0 * M, 20.0 * M), 0.0)),
            [2.0, 2.048, 2.4, 2.56, 3.2, 4.0, 5.0, 6.0, 8.0, 10.0, 12.5, 16.0, 20.0]
        );
    }

    #[test]
    fn the_dropdown_for_rtl_ranges_leaves_out_gaps_and_the_top() {
        assert_eq!(
            msps(&rate_list(&rtl(), 0.0)),
            [0.25, 1.0, 1.024, 2.0, 2.048, 2.4, 2.56]
        );
    }

    #[test]
    fn single_rates_are_offered_exactly() {
        let l = RateLimits {
            min_hz: 2.5 * M,
            max_hz: 10.0 * M,
            ranges: vec![(2.5 * M, 2.5 * M), (10.0 * M, 10.0 * M)],
            steady_hz: None,
        };
        assert_eq!(msps(&rate_list(&l, 0.0)), [2.5, 10.0]);
        assert_eq!(msps(&rate_list(&l, 4.0 * M)), [10.0]);
    }

    #[test]
    fn no_limits_offers_the_whole_ladder_and_fast_radios_get_more() {
        assert_eq!(rate_list(&RateLimits::default(), 0.0).len(), 17);
        let pluto = span(2.048 * M, 61.44 * M);
        let l = msps(&rate_list(&pluto, 0.0));
        assert_eq!(l.first(), Some(&2.048));
        assert!(l.ends_with(&[20.0, 25.0, 30.72, 40.0, 50.0, 56.0, 61.44]));
        assert!(msps(&rate_list(&span(0.0, 40.0 * M), 0.0)).ends_with(&[30.72, 40.0]));
    }

    #[test]
    fn the_mode_minimum_filters_the_dropdown() {
        assert_eq!(
            msps(&rate_list(&span(2.0 * M, 20.0 * M), 7.9 * M)),
            [8.0, 10.0, 12.5, 16.0, 20.0]
        );
    }

    #[test]
    fn baseband_filter_examples() {
        assert_eq!(baseband_filter(1.75 * M, 2.0 * M, 2.0 * M), 1.75 * M);
        assert_eq!(baseband_filter(1.75 * M, 2.0 * M, 10.0 * M), 0.0);
        assert_eq!(baseband_filter(1.75 * M, 2.0 * M, 2.01 * M), 1.75 * M);
        assert_eq!(baseband_filter(7.0 * M, 8.0 * M, 4.0 * M), 0.0);
        assert_eq!(baseband_filter(2.5 * M, 4.0 * M, 3.2 * M), 2.5 * M);
        assert_eq!(baseband_filter(0.0, 2.0 * M, 10.0 * M), 0.0);
        assert_eq!(baseband_filter(1.75 * M, 0.0, 10.0 * M), 1.75 * M);
    }

    #[test]
    fn link_caps() {
        assert_eq!(link_cap(None, true, LinkKind::Auto, false), 3.0 * M);
        assert_eq!(link_cap(None, true, LinkKind::Usb, true), 6.0 * M);
        assert_eq!(
            link_cap(Some(4.0 * M), true, LinkKind::Ethernet, false),
            0.0
        );
        assert_eq!(
            link_cap(Some(4.0 * M), false, LinkKind::Auto, true),
            8.0 * M
        );
        assert_eq!(link_cap(None, false, LinkKind::Auto, false), 0.0);
    }

    #[test]
    fn rates_on_a_capped_link() {
        let cap = 4.0 * M;
        assert_eq!(rate_on_link(2.048 * M, 2.0 * M, 61.44 * M, cap), 2.048 * M);
        assert_eq!(rate_on_link(4.0 * M, 0.25 * M, 61.44 * M, cap), 4.0 * M);
        assert_eq!(rate_on_link(10.0 * M, 1.6 * M, 61.44 * M, cap), 4.0 * M);
        assert_eq!(rate_on_link(8.0 * M, 6.5 * M, 61.44 * M, cap), 8.0 * M);
        assert_eq!(
            rate_on_link(dvb_native_rate(8.0), 7.9 * M, 61.44 * M, 8.0 * M),
            8.0 * M
        );
        assert_eq!(rate_on_link(80.0 * M, 1.0 * M, 61.44 * M, 0.0), 61.44 * M);
    }

    #[test]
    fn link_notes() {
        assert_eq!(link_note(3.0 * M, 3.0 * M, false, false), None);
        assert_eq!(link_note(10.0 * M, 0.0, false, false), None);
        let n = link_note(10.0 * M, 3.0 * M, false, true).unwrap();
        assert!(n.starts_with("PlutoSDR over its USB cable carries about 3 Msps without loss; this mode runs it at 10 Msps"));
        assert!(n.ends_with("Set the radio's IQ format to CS8 (half the data, up to 8 Msps over USB), or connect the Pluto over Ethernet."));
        assert!(link_note(10.0 * M, 6.0 * M, true, true)
            .unwrap()
            .ends_with("to stop it."));
        assert!(link_note(10.0 * M, 3.0 * M, false, false)
            .unwrap()
            .contains("install the Tezuka firmware"));
    }

    #[test]
    fn dvb_native_rates() {
        assert!((dvb_native_rate(8.0) - 9_142_857.14).abs() < 1.0);
        assert!((dvb_native_rate(7.0) - 8e6).abs() < 1e-6);
        assert!((dvb_native_rate(6.0) - 6_857_142.86).abs() < 1.0);
    }
}
