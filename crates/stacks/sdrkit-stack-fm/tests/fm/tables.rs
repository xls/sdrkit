//! The programme type tables, the call letters and the US raster; and the band plans.

use super::common::band_settings;
use sdrkit_stack_fm::bands::BandPlan;
use sdrkit_stack_fm::pty::{
    call_letters, on_us_raster, pi_from_call_letters, pty_name, Evidence, Variant,
};

#[test]
fn tables_and_conversions() {
    assert_eq!(pty_name(10, false), "Pop Music");
    assert_eq!(pty_name(10, true), "Country");
    assert_eq!(pty_name(33, true), "News");
    assert_eq!(pty_name(31, true), "ALERT! ALERT!");
    assert_eq!(pty_name(0, false), "None");
    assert_eq!(pty_name(27, true), "");
    for (pi, call) in [
        (0x1000, "KAAA"),
        (0x1001, "KAAB"),
        (0x101A, "KABA"),
        (0x54A7, "KZZZ"),
        (0x54A8, "WAAA"),
        (0x994F, "WZZZ"),
    ] {
        assert_eq!(call_letters(pi).as_deref(), Some(call), "{pi:04X}");
        assert_eq!(pi_from_call_letters(call), pi);
    }
    for pi in 0x1000..=0x994Fu16 {
        assert_eq!(pi_from_call_letters(&call_letters(pi).unwrap()), pi);
    }
    assert_eq!(call_letters(0x0FFF), None);
    assert_eq!(call_letters(0x9950), None);
    assert_eq!(pi_from_call_letters("wabc"), pi_from_call_letters("WABC"));
    for bad in ["XABC", "WAB", "W1BC", ""] {
        assert_eq!(pi_from_call_letters(bad), 0, "{bad}");
    }
    let at = |mhz: f64| Evidence {
        tuned_hz: mhz * 1e6,
        ..Default::default()
    };
    for mhz in [87.9, 93.9, 101.1, 107.9] {
        assert!(
            on_us_raster(mhz * 1e6) && Variant::Auto.rbds_applies(&at(mhz)),
            "{mhz}"
        );
    }
    for mhz in [88.0, 100.0, 98.2, 98.25] {
        assert!(!Variant::Auto.rbds_applies(&at(mhz)), "{mhz}");
    }
    let ecc = |e: u8| Evidence {
        ecc: Some(e),
        ..at(104.1)
    };
    assert!(Variant::Auto.rbds_applies(&ecc(0xA0)));
    assert!(!Variant::Auto.rbds_applies(&ecc(0xF0)));
    assert!(Variant::Rbds.rbds_applies(&ecc(0xF0)));
    assert!(!Variant::Rds.rbds_applies(&at(101.1)));
}

/// The band of the data folder's FM band `id`.
fn band_plan(id: &str) -> BandPlan {
    let hz = |key: &str| {
        let s = band_settings(id);
        s.iter().find(|(k, _)| k == key).unwrap().1.parse::<f64>().unwrap()
    };
    BandPlan {
        min_hz: hz("band_low_hz"),
        max_hz: hz("band_high_hz"),
        step_hz: hz("raster_hz"),
        first_hz: hz("first_hz"),
    }
}

#[test]
fn bands_come_from_the_plan_data() {
    let eu = band_plan("europe");
    assert_eq!(eu, BandPlan::default());
    let ch = eu.channels();
    assert_eq!(
        (ch[0], *ch.last().unwrap(), ch.len()),
        (87.5e6, 108.0e6, 206)
    );
    let us = band_plan("fm_north_america");
    assert_eq!((us.first_hz, us.step_hz), (88.1e6, 200e3));
    assert_eq!(us.snap(101.05e6), 101.1e6);
    let oirt = band_plan("oirt");
    assert!(oirt.contains(70e6) && !oirt.contains(88e6));
}
