//! The Z-Wave region file and the Wi-SUN band plan file of the repository's data folder: every row loads, and the
//! settings hold the channels of their plans.

use sdrkit_core::Value;
use sdrkit_presets::ism::{
    read_wisun_bands, read_wisun_plan_ranges, read_zwave_regions, wisun_country_settings, wisun_plan_at,
    wisun_settings,
    zwave_region_at, zwave_settings, WisunPlan, ZwaveRegion, WISUN_BANDS_FILE,
    WISUN_PLAN_OF_FREQUENCY_FILE, ZWAVE_REGIONS_FILE,
};
use std::path::PathBuf;

fn data() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../data")
}

fn regions() -> Vec<ZwaveRegion> {
    read_zwave_regions(&data().join(ZWAVE_REGIONS_FILE)).expect("the Z-Wave region file")
}

fn plans() -> Vec<WisunPlan> {
    read_wisun_bands(&data().join(WISUN_BANDS_FILE)).expect("the Wi-SUN band plan file")
}

fn text<'a>(settings: &'a [(String, Value)], key: &str) -> &'a str {
    match settings.iter().find(|(k, _)| k == key) {
        Some((_, Value::Text(t))) => t,
        _ => panic!("no text setting {key}"),
    }
}

#[test]
fn every_zwave_region_loads_with_its_channels() {
    let rows = regions();
    assert_eq!(rows.len(), 10);
    for r in &rows {
        assert!(!r.channels().is_empty(), "{}", r.id);
        assert!((860e6..930e6).contains(&r.centre_hz), "{}", r.id);
        let (centre, s) = zwave_settings(&rows, &r.id).unwrap();
        assert_eq!(centre, r.centre_hz);
        assert_eq!(
            text(&s, "channels").split(';').count(),
            r.channels().len(),
            "{}",
            r.id
        );
    }
    let (_, eu) = zwave_settings(&rows, "eu").unwrap();
    assert_eq!(text(&eu, "channels"), "868420000/1;868400000/2;869850000/3");
    let (_, jp) = zwave_settings(&rows, "JP").unwrap();
    assert_eq!(text(&jp, "channels"), "922500000/2;923900000/2;926300000/2");
    assert!(zwave_settings(&rows, "XX").is_none());
}

#[test]
fn the_zwave_region_of_a_centre() {
    let rows = regions();
    assert_eq!(zwave_region_at(&rows, 908.0e6, 2e6).unwrap().id, "US");
    assert_eq!(zwave_region_at(&rows, 869.13e6, 2e6).unwrap().id, "EU");
    assert!(zwave_region_at(&rows, 433.92e6, 2e6).is_none());
}

#[test]
fn every_wisun_plan_loads_with_its_classes() {
    let p = plans();
    assert_eq!(p.len(), 20);
    assert_eq!(p.iter().map(|p| p.classes.len()).sum::<usize>(), 42);
    for plan in &p {
        assert!(!plan.ranges_hz.is_empty(), "{}", plan.id);
        let s = wisun_settings(&p, &plan.id).unwrap();
        assert_eq!(
            text(&s, "classes").split(';').count(),
            plan.classes.len(),
            "{}",
            plan.id
        );
        assert_eq!(
            text(&s, "ranges").split(';').count(),
            plan.ranges_hz.len(),
            "{}",
            plan.id
        );
    }
    let anz = wisun_settings(&p, "anz").unwrap();
    assert_eq!(text(&anz, "ranges"), "915000000-928000000");
    assert_eq!(
        text(&anz, "classes"),
        "915200000/200000/64/50:1;915400000/400000/32/100:0.5 150:0.5"
    );
    let na = p.iter().find(|p| p.id == "NA").unwrap();
    assert_eq!((na.domain, na.classes.len()), (1, 3));
    assert!(na.classes[2].modes.is_empty());
    let bz = wisun_settings(&p, "BZ").unwrap();
    assert_eq!(
        text(&bz, "ranges"),
        "902000000-907500000;915000000-928000000"
    );
    assert!(wisun_settings(&p, "Atlantis").is_none());
}

#[test]
fn the_wisun_plan_of_a_frequency() {
    let p = plans();
    let ranges = read_wisun_plan_ranges(&data().join(WISUN_PLAN_OF_FREQUENCY_FILE))
        .expect("the file of the Wi-SUN plan of a frequency");
    assert_eq!(ranges.len(), 7);
    for r in &ranges {
        assert!(p.iter().any(|x| x.id == r.id), "{}", r.id);
    }
    let at = |hz: f64| wisun_plan_at(&ranges, &p, hz).map(|x| x.id.as_str());
    assert_eq!(at(921.5e6), Some("ANZ"));
    assert_eq!(at(910e6), Some("NA"));
    assert_eq!(at(868e6), Some("EU863"));
    assert_eq!(at(2440e6), Some("WW"));
    assert_eq!(at(433.92e6), None);
}

#[test]
fn a_country_takes_the_first_wisun_plan_that_lists_it() {
    let p = plans();
    let (hz, s) = wisun_country_settings(&p, "au").unwrap();
    assert_eq!(hz, 921.5e6);
    assert_eq!(text(&s, "ranges"), "915000000-928000000");
    // Brazil tunes to the middle of its wider range.
    assert_eq!(wisun_country_settings(&p, "BR").unwrap().0, 921.5e6);
    // Singapore has two plans: the one at 920 MHz comes first.
    let (hz, s) = wisun_country_settings(&p, "SG").unwrap();
    assert_eq!(hz, 922.5e6);
    assert!(text(&s, "classes").starts_with("920200000/"));
    assert_eq!(wisun_country_settings(&p, "DE").unwrap().0, 866.5e6);
    assert!(wisun_country_settings(&p, "AQ").is_none());
    // A plan without countries is never a country's.
    assert!(p.iter().find(|p| p.id == "EU870").unwrap().countries.is_empty());
}

#[test]
fn the_wisun_preset_of_a_country_has_its_plan_and_a_frequency_in_it() {
    let d = sdrkit_presets::Data::open(data()).unwrap();
    let region = sdrkit_presets::Region::new("NZ").unwrap();
    let p = d
        .preset("wisun", &region, &sdrkit_presets::Options::default())
        .unwrap();
    assert_eq!(p.frequency_hz, 921.5e6);
    let ranges = p.settings.iter().find(|(k, _)| k == "ranges").map(|(_, v)| v);
    assert_eq!(
        ranges,
        Some(&Value::Text("915000000-928000000".into()))
    );
    assert!(p.settings.iter().any(|(k, _)| k == "classes"));
}
