//! The rules of a configure: the rates offered, the decimation chosen, the tuning range, the frequency, gain and format sent,
//! and the notes.

use crate::proto::{format, ClientSync, DeviceInfo};
use crate::WireFormat;
use sdrkit_core::Hz;

/// The frequency field is 32-bit Hz: the highest frequency it holds.
pub const MAX_FREQ: Hz = 4.29e9;

/// The rates a server offers, fastest first: index k is decimation `min_iq_decimation + k`. Empty only without a maximum rate.
pub fn offered_rates(d: &DeviceInfo) -> Vec<Hz> {
    if d.max_sample_rate == 0 {
        return Vec::new();
    }
    let first = d.min_iq_decimation.min(19);
    let end = d.decimation_stages.max(first + 1).min(20);
    (first..end)
        .map(|n| d.max_sample_rate as Hz / (1u64 << n) as Hz)
        .collect()
}

/// The decimation chosen for a requested rate.
#[derive(Clone, Debug, PartialEq)]
pub struct RatePlan {
    /// The decimation stage sent.
    pub decimation: u32,
    /// The rate it produces: the maximum rate / 2^decimation, exactly.
    pub rate_hz: Hz,
    /// The target rate (the requested rate, or the fastest when 0).
    pub wanted_hz: Hz,
}

impl RatePlan {
    /// The warning when the rate is below 97 % of the target rate.
    pub fn warning(&self) -> Option<String> {
        (self.rate_hz < 0.97 * self.wanted_hz).then(|| {
            format!(
                "this radio only offers {:.2} Msps, {:.2} Msps were requested for this channel width",
                self.rate_hz / 1e6,
                self.wanted_hz / 1e6
            )
        })
    }
}

/// Chooses the decimation: the slowest rate at or above 0.9999 x the target rate, else the fastest. None when no rate is offered.
pub fn plan_rate(d: &DeviceInfo, asked_hz: Hz) -> Option<RatePlan> {
    let rates = offered_rates(d);
    let fastest = *rates.first()?;
    let wanted = if asked_hz > 0.0 { asked_hz } else { fastest };
    let k = rates
        .iter()
        .rposition(|&r| r >= wanted * 0.9999)
        .unwrap_or(0);
    Some(RatePlan {
        decimation: d.min_iq_decimation.min(19) + k as u32,
        rate_hz: rates[k],
        wanted_hz: wanted,
    })
}

/// The tuning range: the range of DEVICE_INFO, range, widened (never narrowed) by CLIENT_SYNC's IQ centre range when that is valid.
pub fn tuning_range(d: &DeviceInfo, sync: Option<&ClientSync>) -> (Hz, Hz) {
    let (mut lo, mut hi) = (d.min_frequency as Hz, d.max_frequency as Hz);
    if let Some(s) = sync.filter(|s| s.iq_range_valid()) {
        hi = hi.max(s.max_iq_center as Hz);
        let m = s.min_iq_center as Hz;
        if m > 0.0 && (m < lo || lo == 0.0) {
            lo = m;
        }
    }
    (lo, hi)
}

/// The frequency sent, in whole Hz. `None` below 0 or above [`MAX_FREQ`].
pub fn frequency_sent(centre_hz: Hz) -> Option<u32> {
    (0.0..=MAX_FREQ)
        .contains(&centre_hz)
        .then(|| centre_hz.round() as u32)
}

/// The gain index sent: the gain rounded. `None` below 0 or above `max_index`.
pub fn gain_sent(gain: f64, max_index: u32) -> Option<u32> {
    let g = gain.round();
    (0.0..=f64::from(max_index))
        .contains(&g)
        .then_some(g as u32)
}

/// The IQ format sent: the forced format of the server when it has one, else the requested format.
pub fn iq_format(asked: WireFormat, forced: u32) -> u32 {
    if (format::UINT8..=format::FLOAT).contains(&forced) {
        forced
    } else {
        match asked {
            WireFormat::Bits8 => format::UINT8,
            WireFormat::Bits16 => format::INT16,
        }
    }
}

/// A number in MHz with up to 6 significant digits, trailing zeros dropped ("24", "1800", "433.92").
pub fn mhz6(hz: Hz) -> String {
    let m = hz / 1e6;
    let digits = if m.abs() >= 1.0 {
        m.abs().log10().floor() as i32 + 1
    } else {
        1
    };
    let dec = (6 - digits).max(0) as usize;
    let s = format!("{m:.dec$}");
    let s = if s.contains('.') {
        s.trim_end_matches('0').trim_end_matches('.').to_string()
    } else {
        s
    };
    if s == "-0" {
        "0".into()
    } else {
        s
    }
}

/// The note for a frequency outside the tuning range, or None (also when the range is unknown).
pub fn range_note(centre_hz: Hz, range: (Hz, Hz)) -> Option<String> {
    let (lo, hi) = range;
    if hi <= lo || (centre_hz >= lo && centre_hz <= hi) {
        return None;
    }
    Some(format!(
        "this SpyServer radio is specified for {} to {} MHz; the driver may not reach {} MHz or the signal may be weak or absent",
        mhz6(lo),
        mhz6(hi),
        mhz6(centre_hz)
    ))
}

/// The note for a read-only client asking for a frequency outside the IQ band the server provides, or None.
pub fn read_only_note(label: &str, sent_hz: Hz, sync: &ClientSync) -> Option<String> {
    if sync.can_control || !sync.iq_range_valid() {
        return None;
    }
    let (lo, hi) = (sync.min_iq_center as Hz, sync.max_iq_center as Hz);
    if sent_hz >= lo && sent_hz <= hi {
        return None;
    }
    Some(format!(
        "the SpyServer at {label} is read only (another client controls it): it is tuned to {:.3} MHz and gives {:.3} to {:.3} MHz",
        sync.device_center as Hz / 1e6,
        lo / 1e6,
        hi / 1e6
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    const M: f64 = 1e6;

    fn dev(max: u32, stages: u32, min: u32) -> DeviceInfo {
        DeviceInfo {
            max_sample_rate: max,
            decimation_stages: stages,
            min_iq_decimation: min,
            ..DeviceInfo::airspy_r2()
        }
    }

    #[test]
    fn the_rates_offered() {
        let r = offered_rates(&dev(10_000_000, 9, 0));
        assert_eq!(r.len(), 9);
        assert_eq!((r[0], r[8]), (10e6, 39_062.5));
        assert_eq!(
            offered_rates(&dev(768_000, 6, 1)),
            [384_000.0, 192_000.0, 96_000.0, 48_000.0, 24_000.0]
        );
        assert_eq!(offered_rates(&dev(2_000_000, 0, 0)), [2e6]);
        assert_eq!(offered_rates(&dev(10_000_000, 3, 5)), [10e6 / 32.0]);
        assert_eq!(offered_rates(&dev(10_000_000, 40, 0)).len(), 20);
        assert!(offered_rates(&dev(0, 9, 0)).is_empty());
    }

    #[test]
    fn the_rate_choice_table() {
        type Case = (u32, u32, u32, f64, u32, f64, Option<&'static str>);
        let cases: &[Case] = &[
            (10_000_000, 9, 0, 2.2 * M, 2, 2.5 * M, None),
            (10_000_000, 9, 0, 2.5 * M, 2, 2.5 * M, None),
            (10_000_000, 9, 0, 2.49975 * M, 2, 2.5 * M, None),
            (10_000_000, 9, 0, 10.0 * M, 0, 10.0 * M, None),
            (
                10_000_000,
                9,
                0,
                20.0 * M,
                0,
                10.0 * M,
                Some("this radio only offers 10.00 Msps, 20.00 Msps were requested for this channel width"),
            ),
            (10_000_000, 9, 0, 0.03 * M, 8, 39_062.5, None),
            (10_000_000, 9, 0, 0.0, 0, 10.0 * M, None),
            (768_000, 6, 1, 0.25 * M, 1, 384_000.0, None),
            (
                768_000,
                6,
                1,
                0.5 * M,
                1,
                384_000.0,
                Some("this radio only offers 0.38 Msps, 0.50 Msps were requested for this channel width"),
            ),
            (768_000, 6, 1, 48_000.0, 4, 48_000.0, None),
            (2_400_000, 0, 0, 1.0 * M, 0, 2.4 * M, None),
        ];
        for &(max, st, min, asked, dec, rate, warn) in cases {
            let p = plan_rate(&dev(max, st, min), asked).unwrap();
            assert_eq!(
                (p.decimation, p.rate_hz),
                (dec, rate),
                "{max} {st} {min} {asked}"
            );
            assert_eq!(p.warning().as_deref(), warn);
        }
        assert_eq!(plan_rate(&dev(0, 9, 0), 1e6), None);
    }

    #[test]
    fn the_range_widens_but_never_narrows() {
        let d = DeviceInfo::airspy_r2();
        let mut s = ClientSync {
            min_iq_center: 99_000_000,
            max_iq_center: 101_000_000,
            ..Default::default()
        };
        assert_eq!(tuning_range(&d, Some(&s)), (24e6, 1800e6));
        s.min_iq_center = 24_000_000;
        s.max_iq_center = 1_900_000_000;
        assert_eq!(tuning_range(&d, Some(&s)), (24e6, 1900e6));
        s.min_iq_center = 10_000_000;
        assert_eq!(tuning_range(&d, Some(&s)), (10e6, 1900e6));
        // An invalid range changes nothing.
        s.max_iq_center = 0;
        assert_eq!(tuning_range(&d, Some(&s)), (24e6, 1800e6));
        // A device minimum of 0 takes the IQ minimum.
        let d0 = DeviceInfo {
            min_frequency: 0,
            ..d
        };
        let s = ClientSync {
            min_iq_center: 5,
            max_iq_center: 6,
            ..Default::default()
        };
        assert_eq!(tuning_range(&d0, Some(&s)).0, 5.0);
    }

    #[test]
    fn frequency_gain_and_format_sent() {
        assert_eq!(frequency_sent(100e6), Some(100_000_000));
        assert_eq!(frequency_sent(-5.0), None);
        assert_eq!(frequency_sent(5e9), None);
        assert_eq!(frequency_sent(MAX_FREQ), Some(4_290_000_000));
        assert_eq!(frequency_sent(433.92e6 + 0.4), Some(433_920_000));
        assert_eq!(
            [10.0, 30.0, -3.0, 7.5].map(|g| gain_sent(g, 21)),
            [Some(10), None, None, Some(8)]
        );
        assert_eq!(iq_format(WireFormat::Bits8, 0), format::UINT8);
        assert_eq!(iq_format(WireFormat::Bits16, 0), format::INT16);
        assert_eq!(iq_format(WireFormat::Bits8, 3), format::INT24);
        assert_eq!(iq_format(WireFormat::Bits16, 9), format::INT16);
    }

    #[test]
    fn notes() {
        assert_eq!(mhz6(24e6), "24");
        assert_eq!(mhz6(1800e6), "1800");
        assert_eq!(mhz6(433.92e6), "433.92");
        assert_eq!(mhz6(0.1e6), "0.1");
        assert_eq!(mhz6(1234.5678e6), "1234.57");
        let n = range_note(3e9, (24e6, 1800e6)).unwrap();
        assert_eq!(
            n,
            "this SpyServer radio is specified for 24 to 1800 MHz; the driver may not reach 3000 MHz or the signal may be weak or absent"
        );
        assert_eq!(range_note(100e6, (24e6, 1800e6)), None);
        assert_eq!(range_note(3e9, (0.0, 0.0)), None);
        let s = ClientSync {
            can_control: false,
            device_center: 100_000_000,
            min_iq_center: 99_000_000,
            max_iq_center: 101_000_000,
            ..Default::default()
        };
        assert_eq!(
            read_only_note("127.0.0.1:5", 200e6, &s).unwrap(),
            "the SpyServer at 127.0.0.1:5 is read only (another client controls it): it is tuned to 100.000 MHz and gives 99.000 to \
             101.000 MHz"
        );
        assert_eq!(read_only_note("a", 99.5e6, &s), None);
        assert_eq!(
            read_only_note(
                "a",
                200e6,
                &ClientSync {
                    can_control: true,
                    ..s
                }
            ),
            None
        );
    }
}
