//! The data files of the stack (through sdrkit-presets): the shortwave and medium-wave broadcast bands, the band keys,
//! the amateur voice segments, and the presets that the stack applies.

use crate::common::{data, presets_text, receiver};
use num_complex::Complex32;
use sdrkit_core::Owner as _;
use sdrkit_core::{owner, HostRequest, Output, Receiver, Value};
use sdrkit_presets::general::{ham_plan, ham_segment_at};
use sdrkit_stack_general::bands::{ham_key, sw_band_key};
use sdrkit_stack_general::Demod;

#[test]
fn shortwave_bands() {
    let d = data();
    let bands = d.shortwave_bands();
    assert_eq!(bands.len(), 14);
    for w in bands.windows(2) {
        assert!(w[0].hull.1 < w[1].hull.0, "sorted and not overlapping");
    }
    for b in &bands {
        assert!(b.hull.0 >= 2300e3 && b.hull.1 <= 30_000e3);
        if b.hull.0 != 13_570e3 && b.hull.0 != 25_670e3 {
            assert_eq!(b.hull.0 % 5e3, 0.0, "{} on the 5 kHz raster", b.name);
        }
        assert_eq!(d.shortwave_band_of(b.centre_hz(None)).unwrap().name, b.name);
        let (mut lo, mut hi) = (f64::MAX, f64::MIN);
        for r in 1..=3 {
            if let Some((a, c)) = b.edges(r) {
                assert!(a >= b.hull.0 && c <= b.hull.1);
                lo = lo.min(a);
                hi = hi.max(c);
            }
        }
        assert_eq!((lo, hi), b.hull, "the hull is the widest");
        assert_eq!(
            b.tropical,
            ["120 m", "90 m", "60 m"].contains(&b.name.as_str())
        );
    }
    assert!(d.shortwave_band_of(8000e3).is_none());
    assert!(d.shortwave_band_of(14_100e3).is_none());
    let b75 = &bands[2];
    assert_eq!(b75.edges(2), None);
    assert_eq!(b75.edges(1), Some((3950e3, 4000e3)));
    assert_eq!(b75.edges(3), Some((3900e3, 4000e3)));
    assert_eq!(bands[5].edges(2), Some((7300e3, 7400e3)));
    assert_eq!(bands[3].hull, (4750e3, 5060e3));
    assert!(b75.help().contains("Region 2: no broadcasting"));
    let spacing: Vec<f64> = (1..=3).map(|r| d.medium_wave(r).unwrap().2).collect();
    assert_eq!(spacing, [9000.0, 10_000.0, 9000.0]);
}

#[test]
fn region_band_plans() {
    let d = data();
    let all_sw = d.shortwave_bands();
    for plan in d.am_band_plans().iter().filter(|b| b.has_medium_wave()) {
        let sw: Vec<_> = plan.bands.iter().filter(|b| b.name != "MW").collect();
        let want: Vec<_> = all_sw
            .iter()
            .filter_map(|b| b.edges(plan.region).map(|e| (b.name.as_str(), e)))
            .collect();
        assert_eq!(sw.len(), want.len());
        for (b, (name, (lo, hi))) in sw.iter().zip(want) {
            assert_eq!((b.name.as_str(), b.low_hz, b.high_hz), (name, lo, hi));
        }
    }
    let mw: Vec<(f64, f64)> = d
        .am_band_plans()
        .iter()
        .map(|b| (b.bands[0].low_hz, b.bands[0].high_hz))
        .collect();
    assert_eq!(mw, [(531e3, 1602e3), (540e3, 1700e3), (531e3, 1602e3)]);
}

#[test]
fn ham_plan_keys_light_the_narrower_segment() {
    let segs = data().ham_voice().unwrap();
    let s = ham_segment_at(&segs, 1, 145.65e6, "nfm").unwrap();
    assert_eq!(s.key, "2m rpt");
    assert_eq!(
        ham_segment_at(&segs, 2, 3.7e6, "lsb").unwrap().tune_hz,
        3.850e6
    );
    assert!(ham_segment_at(&segs, 1, 3.7e6, "usb").is_none());
    for r in 1..=3 {
        assert!(ham_plan(&segs, r)
            .iter()
            .all(|s| s.tune_hz >= s.low_hz && s.tune_hz <= s.high_hz));
    }
}

#[test]
fn key_actions_from_the_data() {
    let d = data();
    let bands = d.shortwave_bands();
    let k = sw_band_key(bands[6].centre_hz(None), false);
    assert_eq!(k.freq_hz, 9_650_000.0);
    assert_eq!(
        sw_band_key(bands[5].centre_hz(Some(2)), true).freq_hz,
        7_350_000.0
    );
    let segs = d.ham_voice().unwrap();
    let seg = ham_segment_at(&segs, 1, 145.5e6, "nfm").unwrap();
    let demod = <Demod as sdrkit_core::Choice>::from_key(&seg.demod).unwrap();
    let h = ham_key(seg.tune_hz, demod);
    assert_eq!(h.freq_hz, 145.5e6);
    assert_eq!(h.width_hz, 12_500.0);
    let usb = d.general_band_keys("usb").unwrap();
    assert_eq!(usb[2].demod, "lsb");
    assert_eq!(usb[3].demod, "usb");
    assert_eq!(d.general_band_keys("am").unwrap().len(), 16);
    for group in ["usb", "cw", "nfm", "wfm", "am", "mw"] {
        for k in d.general_band_keys(group).unwrap() {
            assert!(
                <Demod as sdrkit_core::Choice>::from_key(&k.demod).is_some(),
                "{}",
                k.label
            );
        }
    }
}

/// The stack applies the presets of two rows of `general/presets.csv`: the demodulator, the width and the retune of
/// each.
#[test]
fn the_stack_applies_two_rows_of_the_preset_file() {
    let text = presets_text();
    let mut rx = receiver(2e6, -250e3, &[("presets", &text)]);
    let id = rx.commands().id("preset.apply").unwrap();
    let def = rx.commands().def(id).unwrap().clone();
    let row = def.args.record_decl().id("row").unwrap();
    for (key, demod, width, hz) in [
        ("pmr446", Demod::Nfm, 8500.0, 446_006_250.0),
        ("ham_40m_lsb", Demod::Lsb, 2700.0, 7.1e6),
    ] {
        let mut args = def.new_args();
        args.set_value(row, &Value::Text(key.into())).unwrap();
        owner::invoke(&mut rx, id, &args).unwrap();
        assert_eq!(
            (rx.settings().demod, rx.settings().width()),
            (demod, width),
            "{key}"
        );
        let mut out = Output::default();
        rx.feed(&[Complex32::default(); 1000], &mut out);
        assert!(
            matches!(out.requests.as_slice(), [HostRequest::Retune { tuned_hz, .. }] if *tuned_hz == hz),
            "{key}"
        );
    }
    let bad = crate::common::set(&mut rx, "presets", "x=1,dsb,1,X");
    assert!(bad.is_err(), "bad text is refused");
    assert_eq!(
        rx.settings().presets,
        text,
        "a refused value changes nothing"
    );
}
