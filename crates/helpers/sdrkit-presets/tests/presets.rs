//! The presets of the fixture folder `tests/fixture`, and one start through the engine.

use sdrkit_core::Value;
use sdrkit_host_engine::OwnerRef;
use sdrkit_presets::{
    start, Data, DataError, Options, PresetError, Region, RegionError, StartError, StationKind,
};
use std::path::PathBuf;

fn fixture() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixture")
}

fn data() -> Data {
    Data::open(fixture()).expect("the fixture opens")
}

fn region(code: &str) -> Region {
    Region::new(code).expect("a country code")
}

fn setting<'a>(settings: &'a [(String, Value)], key: &str) -> Option<&'a str> {
    settings
        .iter()
        .find(|(k, _)| k == key)
        .and_then(|(_, v)| match v {
            Value::Text(t) => Some(t.as_str()),
            _ => None,
        })
}

#[test]
fn dvbt_in_a_city_starts_on_its_strongest_station() {
    let d = data();
    let sydney = region("au").with_city("sydney");
    let p = d.preset("dvbt", &sydney, &Options::default()).unwrap();
    // Two rows have 28 dB: the one with more reports. The 35 dB row is DVB-T2, which this stack does not receive.
    assert_eq!(p.frequency_hz, 226.5e6);
    assert_eq!(p.width_hz, Some(7e6));
    assert_eq!(
        p.station.as_ref().map(|s| s.network.as_str()),
        Some("ABC Sydney")
    );
    assert_eq!(p.plan, "tv_au");
    assert!(setting(&p.settings, "channels").is_some_and(|c| c.split(';').count() == 6
        && c.contains("226500000")));
    assert_eq!(setting(&p.settings, "bandwidth"), Some("7"));
    assert_eq!(p.channels.len(), 6);
    assert_eq!(p.channels[0].label, "6");
    assert_eq!(p.channels[4].frequency_hz, 529.5e6);
}

#[test]
fn without_stations_the_first_channel_of_the_plan() {
    let d = data();
    let p = d
        .preset("dvbt", &region("DE"), &Options::default())
        .unwrap();
    assert_eq!((p.frequency_hz, p.width_hz), (474e6, Some(8e6)));
    assert!(p.station.is_none());
    assert!(setting(&p.settings, "channels").is_some_and(|c| c.starts_with("474000000")));
    assert_eq!(setting(&p.settings, "bandwidth"), Some("8"));
    // A country without a city takes no station.
    let p = d
        .preset("dvbt", &region("AU"), &Options::default())
        .unwrap();
    assert_eq!((p.frequency_hz, p.width_hz), (177.5e6, Some(7e6)));
    // A city that the list does not have.
    let perth = region("AU").with_city("Perth");
    let p = d.preset("dvbt", &perth, &Options::default()).unwrap();
    assert_eq!(p.frequency_hz, 177.5e6);
}

#[test]
fn the_options_replace_parts_of_the_preset() {
    let d = data();
    let options = Options {
        frequency_hz: Some(529.5e6),
        submode: None,
        settings: vec![("bandwidth".into(), Value::Text("8".into()))],
    };
    let sydney = region("AU").with_city("Sydney");
    let p = d.preset("dvbt", &sydney, &options).unwrap();
    assert_eq!(p.frequency_hz, 529.5e6);
    assert!(p.station.is_none());
    assert_eq!(setting(&p.settings, "bandwidth"), Some("8"));
    assert!(setting(&p.settings, "channels").is_some_and(|c| c.contains("529500000")));
}

#[test]
fn fm_dab_and_a_stack_that_only_a_plan_names() {
    let d = data();
    let sydney = region("AU").with_city("Sydney");
    // FM in Australia: the station of the city, the plan of ITU Region 3, the presets of the country.
    let p = d.preset("fm", &sydney, &Options::default()).unwrap();
    assert_eq!(p.frequency_hz, 104.9e6);
    let hz = |key: &str| p.settings.iter().find(|(k, _)| k == key).map(|(_, v)| v.clone());
    assert_eq!(hz("band_low_hz"), Some(Value::Hz(87.5e6)));
    assert_eq!(hz("raster_hz"), Some(Value::Hz(100e3)));
    assert_eq!(setting(&p.settings, "deemphasis"), Some("50"));
    assert_eq!(setting(&p.settings, "rds_variant"), Some("rds"));
    assert_eq!(p.channels.len(), 206);
    assert_eq!(p.channels[1].frequency_hz, 87.6e6);
    // DAB has no list of stations here: the first channel of the plan.
    let p = d.preset("dab", &sydney, &Options::default()).unwrap();
    assert_eq!((p.frequency_hz, p.width_hz), (174.928e6, Some(1.536e6)));
    assert!(setting(&p.settings, "channels").is_some_and(|c| c.starts_with("174928000")));
    // A CSV plan names its stacks: each stack gets its own rows, in every country.
    let p = d
        .preset("goes_gvar", &region("DE"), &Options::default())
        .unwrap();
    assert_eq!((p.frequency_hz, p.channels.len()), (1685.7e6, 1));
    assert!(p.settings.is_empty());
    let p = d
        .preset("noaa_apt", &region("AU"), &Options::default())
        .unwrap();
    assert_eq!(p.channels[0].label, "Series A, low orbit");
}

#[test]
fn a_stack_region_replaces_the_region_for_that_stack() {
    let d = data();
    let r = region("AU")
        .with_city("Sydney")
        .with_stack("dvbt", region("DE"));
    assert_eq!(
        d.preset("dvbt", &r, &Options::default())
            .unwrap()
            .frequency_hz,
        474e6
    );
    assert_eq!(
        d.preset("fm", &r, &Options::default())
            .unwrap()
            .frequency_hz,
        104.9e6
    );
}

#[test]
fn no_data_for_a_stack_or_a_country() {
    let d = data();
    for (stack, code) in [
        ("atsc", "AU"),
        ("dvbt2", "AU"),
        ("dvbt", "FR"),
        ("nothing", "DE"),
    ] {
        match d.preset(stack, &region(code), &Options::default()) {
            Err(PresetError::NoData {
                stack: s,
                region: r,
            }) => {
                assert_eq!((s.as_str(), r.as_str()), (stack, code));
            }
            other => panic!("{stack} in {code}: {other:?}"),
        }
    }
    assert_eq!(
        Region::new("itu1"),
        Err(RegionError::NotACountry("itu1".into()))
    );
}

#[test]
fn missing_data_is_reported_with_its_path() {
    let nowhere = fixture().join("nowhere");
    assert!(matches!(Data::open(&nowhere), Err(DataError::Missing { path }) if path == nowhere));
    // A folder without `plans`.
    let stations = fixture().join("stations");
    assert!(
        matches!(Data::open(&stations), Err(DataError::Missing { path }) if path == stations.join("plans"))
    );
    let d = data();
    let want = fixture().join("stations/dvb-t/DE.csv");
    assert!(
        matches!(d.known_stations(StationKind::DvbT, &region("DE")), Err(DataError::Missing { path }) if path == want)
    );
    assert!(matches!(
        d.cities(&region("DE")),
        Err(DataError::Missing { .. })
    ));
    assert!(matches!(
        d.dab_transmitters(&region("DE")),
        Err(DataError::Missing { .. })
    ));
    assert!(d.newest().is_some());
    assert!(d.age().is_some());
}

#[test]
fn the_lists_of_the_folder() {
    let d = data();
    let sydney = region("AU").with_city("Sydney");
    let rows = d.known_stations(StationKind::DvbT, &sydney).unwrap();
    let cities: Vec<&str> = rows.iter().map(|s| s.city.as_str()).collect();
    assert_eq!(
        cities,
        [
            "Sydney",
            "Sydney",
            "Sydney",
            "Sydney",
            "Melbourne",
            "Wollongong"
        ]
    );
    // Quoted fields: a comma, and a doubled quote.
    assert_eq!(rows[1].network, "Nine, Sydney");
    assert_eq!(rows[1].services, ["9HD", "9Gem", "\"9Go!\""]);
    assert_eq!((rows[1].snr_db, rows[1].reports), (Some(28.0), 3));
    assert_eq!(rows[0].last_seen, "2026-11-01");
    let fm = d.known_stations(StationKind::Fm, &sydney).unwrap();
    assert_eq!((fm[1].snr_db, fm[1].standard.as_str()), (None, ""));
    let c = d.cities(&sydney).unwrap();
    assert_eq!((c[0].name.as_str(), c[0].population), ("Sydney", 4_627_345));
    let t = d.dab_transmitters(&sydney).unwrap();
    assert_eq!(t[0].site, "Artarmon, Hampden Road");
    assert_eq!((t[0].frequency_hz, t[1].power_kw), (202.928e6, None));
    assert_eq!(d.countries().unwrap().len(), 2);
    assert_eq!(StationKind::of_stack("dvbt2"), Some(StationKind::DvbT));
    assert_eq!(StationKind::from_key("isdb-t"), Some(StationKind::IsdbT));
}

#[test]
fn the_data_folder_of_the_repository_opens() {
    let folder = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../data");
    let d = Data::open(folder).expect("the data folder opens");
    let p = d
        .preset("dvbt", &region("AU"), &Options::default())
        .unwrap();
    assert_eq!((p.frequency_hz, p.width_hz), (177.5e6, Some(7e6)));
    let p = d
        .preset("dvbt", &region("DE"), &Options::default())
        .unwrap();
    assert_eq!(p.width_hz, Some(8e6));
    for stack in ["fm", "dab", "noaa_apt"] {
        assert!(
            d.preset(stack, &region("AU"), &Options::default()).is_ok(),
            "{stack}"
        );
    }
    assert!(d.preset("atsc", &region("US"), &Options::default()).is_ok());
    // The plan file of FANET names no stack in its bands: its group ids do.
    let p = d
        .preset("fanet", &region("AU"), &Options::default())
        .unwrap();
    assert_eq!((p.frequency_hz, p.width_hz), (920.8e6, Some(500e3)));
    assert!(d.preset("adsl", &region("DE"), &Options::default()).is_ok());
    // A band that lists a stack is not where the stack starts: AIS keeps its own frequency between its two channels.
    for stack in ["ais", "drm", "lorawan"] {
        assert!(matches!(
            d.preset(stack, &region("AU"), &Options::default()),
            Err(PresetError::NoData { .. })
        ));
    }
    // `general` does not start on a band that lists it either: it starts on the dial of its family file, with the
    // presets of that family.
    let p = d
        .preset("general", &region("AU"), &Options::default())
        .unwrap();
    assert_eq!((p.frequency_hz, p.channels.len()), (145.5e6, 0));
    assert!(p.settings.iter().any(|(k, _)| k == "presets"));
    let p = d.preset("ft8", &region("AU"), &Options::default()).unwrap();
    let keys: Vec<&str> = p.settings.iter().map(|(k, _)| k.as_str()).collect();
    assert_eq!(keys, ["dials", "band_edges"]);
    let p = d.preset("jt65", &region("AU"), &Options::default()).unwrap();
    assert!(p.settings.iter().all(|(k, _)| k != "dials"));
}

#[test]
fn start_tunes_the_session_and_sets_the_bandwidth() {
    let d = data();
    let engine = sdrkit_host_catalog::engine().expect("an engine");
    let radio = engine.open("synth:dvbt").expect("the test signal opens");
    let sydney = region("AU").with_city("Sydney");
    let options = Options::default();
    let preset = d.preset("dvbt", &sydney, &options).unwrap();
    let session = start(&d, &engine, radio, "dvbt", &sydney, &options).expect("the stack starts");
    let text = |owner: OwnerRef, key: &str| {
        engine
            .owner(owner, |o| {
                let id = o.schema().id(key).ok()?;
                let decl = o.schema().setting(id)?.field.clone();
                match o.values()?.value(id)? {
                    Value::Choice(i) => {
                        decl.choices.get(usize::from(*i)).map(|c| c.key.to_string())
                    }
                    Value::Hz(hz) => Some(hz.to_string()),
                    Value::Text(t) => Some(t.clone()),
                    other => Some(format!("{other:?}")),
                }
            })
            .expect("the owner exists")
    };
    assert_eq!(
        text(OwnerRef::Session(session), "tuned_hz"),
        Some(preset.frequency_hz.to_string())
    );
    assert_eq!(preset.frequency_hz, 226.5e6);
    assert_eq!(
        text(OwnerRef::Stack(session), "bandwidth").as_deref(),
        Some("7")
    );
    assert!(text(OwnerRef::Stack(session), "channels").is_some_and(|c| c.contains("226500000")));
    engine.stop_session(session).unwrap();
    // A value that the stack's setting does not have is refused before the start.
    let bad = Options {
        settings: vec![("bandwidth".into(), Value::Text("9".into()))],
        ..Options::default()
    };
    assert!(matches!(
        start(&d, &engine, radio, "dvbt", &sydney, &bad),
        Err(StartError::Setting { key, .. }) if key == "bandwidth"
    ));
}

/// The data folder of the repository.
fn repo_data() -> Data {
    Data::open(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../data")).expect("the data folder opens")
}

#[test]
fn satellite_downlinks_come_from_the_lists() {
    let d = repo_data();
    let eodb = d.downlinks("eodb").unwrap();
    let n20 = eodb.iter().find(|r| r.name == "NOAA-20").unwrap();
    assert_eq!((n20.submode.as_str(), n20.frequency_hz, n20.spacecraft), ("jpss_hrd", 7812e6, Some(159)));
    assert_eq!(n20.symbol_rate, Some(15e6));
    let filecast = d.downlinks("sat_filecast").unwrap();
    assert_eq!(filecast.len(), 2);
    assert!(d.downlinks("bgan").unwrap().is_empty());
    // A stack without plan and station starts on its downlink, in the submode of the options or of the first row.
    let au = region("au");
    let p = d
        .preset("rdas", &au, &Options { submode: Some("arktika_m".into()), ..Options::default() })
        .unwrap();
    assert_eq!((p.frequency_hz, p.width_hz, p.submode.as_deref()), (7865e6, Some(45e6), Some("arktika_m")));
    let p = d.preset("eodb", &au, &Options::default()).unwrap();
    assert_eq!((p.frequency_hz, p.submode.as_deref()), (7812e6, Some("jpss_hrd")));
}

#[test]
fn the_files_a_stack_reads_are_listed() {
    let d = repo_data();
    let files = d.stack_files("eodb").unwrap();
    assert!(files.contains(&"links/xband_downlinks.csv".to_string()), "{files:?}");
    assert!(files.contains(&"satimg/avhrr_calibration.csv".to_string()), "{files:?}");
    assert!(!d.stack_files("rdas").unwrap().contains(&"satimg/avhrr_calibration.csv".to_string()));
}
