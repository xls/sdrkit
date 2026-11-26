//! The Wi-Fi domain file and the HaLow region file of the repository's data folder: every row loads, and the lists hold
//! the channels of their plans.

use sdrkit_core::Value;
use sdrkit_presets::wlan::{
    halow_settings, read_halow_regions, read_wifi_domains, wifi_settings, HalowRegion, WifiDomain,
    HALOW_REGIONS_FILE, WIFI_DOMAINS_FILE,
};
use std::path::PathBuf;

fn data() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../data")
}

fn domains() -> Vec<WifiDomain> {
    read_wifi_domains(&data().join(WIFI_DOMAINS_FILE)).expect("the Wi-Fi domain file")
}

fn regions() -> Vec<HalowRegion> {
    read_halow_regions(&data().join(HALOW_REGIONS_FILE)).expect("the HaLow region file")
}

fn text<'a>(settings: &'a [(String, Value)], key: &str) -> &'a str {
    match settings.iter().find(|(k, _)| k == key) {
        Some((_, Value::Text(t))) => t,
        _ => panic!("no text setting {key}"),
    }
}

#[test]
fn every_wifi_domain_row_loads() {
    let rows = domains();
    assert_eq!(rows.len(), 18);
    for d in ["all", "eu", "us", "jp", "cn", "au"] {
        for b in ["2.4", "5", "6"] {
            assert_eq!(
                rows.iter().filter(|r| r.domain == d && r.band == b).count(),
                1,
                "{d} {b}"
            );
        }
    }
    let row = |d: &str, b: &str| rows.iter().find(|r| r.domain == d && r.band == b).unwrap();
    let n = |d: &str, b: &str| row(d, b).channels.len();
    assert_eq!(n("all", "5"), 28);
    assert_eq!(n("eu", "5"), 19);
    assert_eq!(n("us", "5"), 28);
    assert_eq!(n("cn", "5"), 13);
    assert_eq!(n("all", "6"), 60);
    assert_eq!(n("eu", "6"), 24);
    assert_eq!(n("us", "6"), 60);
    assert_eq!(n("all", "2.4"), 14);
    assert_eq!(n("us", "2.4"), 11);
    assert_eq!(n("jp", "2.4"), 14);
    assert_eq!(n("au", "5"), 22);
    assert_eq!(n("au", "6"), 33);
    for r in &rows {
        assert!(r.setting_key().is_some(), "{r:?}");
        assert!(r.channels.windows(2).all(|w| w[0] < w[1]), "{r:?}");
        assert!(
            r.dfs
                .iter()
                .chain(&r.not_hopped)
                .all(|c| r.channels.contains(c)),
            "{r:?}"
        );
    }
    assert!(row("us", "6").channels.contains(&2));
    assert!(!row("eu", "6").channels.contains(&2));
    let dfs = &row("all", "5").dfs;
    assert_eq!(dfs.len(), 16);
    assert!(dfs.contains(&52) && dfs.contains(&144) && !dfs.contains(&149));
    assert!(!row("all", "2.4").hop_list(false).contains(&14));
    assert_eq!(row("eu", "5").hop_list(true), vec![36, 40, 44, 48]);
}

#[test]
fn wifi_settings_of_a_domain() {
    let rows = domains();
    let eu = wifi_settings(&rows, "EU", false).unwrap();
    assert_eq!(eu.len(), 3);
    assert_eq!(text(&eu, "channels_24"), "1 2 3 4 5 6 7 8 9 10 11 12 13");
    assert!(text(&eu, "channels_5").ends_with("136 140"));
    let us = wifi_settings(&rows, "us", true).unwrap();
    assert_eq!(
        text(&us, "channels_5"),
        "36 40 44 48 149 153 157 161 165 169 173 177"
    );
    assert!(wifi_settings(&rows, "xx", false).is_none());
}

#[test]
fn every_halow_region_row_loads() {
    let rows = regions();
    assert_eq!(rows.len(), 10);
    for r in &rows {
        assert_eq!(r.widths_mhz.len(), r.classes.len(), "{}", r.code);
        assert!(r.start_hz > 0.0 && !r.bands_hz.is_empty(), "{}", r.code);
        for (w, list) in [1u32, 2, 4, 8, 16].iter().zip(&r.channels) {
            assert_eq!(!list.is_empty(), r.widths_mhz.contains(w), "{} {w}", r.code);
            for &n in list {
                let (lo, hi) = (
                    r.centre_hz(n) - f64::from(*w) * 0.5e6,
                    r.centre_hz(n) + f64::from(*w) * 0.5e6,
                );
                assert!(
                    r.bands_hz
                        .iter()
                        .any(|&(a, b)| lo >= a - 1.0 && hi <= b + 1.0),
                    "{} {w} {n}",
                    r.code
                );
            }
        }
    }
    let by = |c: &str| rows.iter().find(|r| r.code == c).unwrap();
    let mhz = |r: &HalowRegion, w: usize| -> Vec<f64> {
        r.channels[w]
            .iter()
            .map(|&n| r.centre_hz(n) / 1e6)
            .collect()
    };
    let (us, eu) = (by("US"), by("EU"));
    let u1 = mhz(us, 0);
    assert_eq!((u1.len(), u1[0], u1[25]), (26, 902.5, 927.5));
    assert_eq!(us.channels[1].len(), 13);
    assert_eq!(us.channels[2], vec![8, 16, 24, 32, 40, 48]);
    assert_eq!((mhz(us, 2)[0], mhz(us, 2)[5]), (906.0, 926.0));
    assert_eq!(mhz(us, 3), vec![908.0, 916.0, 924.0]);
    assert_eq!(mhz(us, 4), vec![912.0]);
    let e1 = mhz(eu, 0);
    assert_eq!((e1.len(), e1[0], e1[4]), (5, 863.5, 867.5));
    assert_eq!(eu.channels[1].len(), 2);
    assert_eq!(
        us.channels[0].iter().position(|&n| n == 27).map(|i| u1[i]),
        Some(915.5)
    );
}

#[test]
fn halow_settings_of_a_region() {
    let rows = regions();
    let eu = halow_settings(&rows, "eu").unwrap();
    assert_eq!(eu.len(), 6);
    assert_eq!(eu[0], ("start_hz".to_owned(), Value::Hz(863e6)));
    assert_eq!(text(&eu, "channels_1"), "1 3 5 7 9");
    assert_eq!(text(&eu, "channels_4"), "");
    assert!(halow_settings(&rows, "XX").is_none());
}
