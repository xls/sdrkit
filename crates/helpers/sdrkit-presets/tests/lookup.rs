//! The lookups of the helper over the repository's data folder: the plans of a country, scan channels, search,
//! labels, names, steps and the user's plan file.

use sdrkit_data_plans::api::PlanFormat;
use sdrkit_data_plans::PlanKind;
use sdrkit_presets::{Data, DataError, HitKind};
use sdrkit_svc_freqdb::View;

fn data() -> Data {
    Data::open(concat!(env!("CARGO_MANIFEST_DIR"), "/../../../data")).unwrap()
}

fn first(d: &Data, kind: PlanKind, cc: &str) -> Option<String> {
    d.plan_ids_of(kind, cc).into_iter().next()
}

#[test]
fn each_country_uses_its_own_plans() {
    let d = data();
    let tv = |cc: &str| first(&d, PlanKind::Tv, cc);
    for (cc, id) in [
        ("AU", "tv_au"),
        ("au", "tv_au"),
        ("US", "tv_us"),
        ("DE", "tv_eu_uhf"),
        ("GB", "tv_gb"),
        ("NZ", "tv_nz"),
        ("JP", "tv_jp"),
        ("CN", "tv_cn"),
        ("BR", "tv_isdbtb"),
        // A country without a television plan uses the plan of its ITU region.
        ("CM", "tv_eu"),
        ("SR", "tv_americas"),
        ("TH", "tv_wide"),
    ] {
        assert_eq!(tv(cc).as_deref(), Some(id), "{cc}");
    }
    assert_eq!(d.plan_ids_of(PlanKind::Tv, "AU").len(), 1);
    assert_eq!(first(&d, PlanKind::Dab, "AU").as_deref(), Some("dab_au"));
    assert_eq!(first(&d, PlanKind::Dab, "US"), None);
    assert_eq!(first(&d, PlanKind::Dab, "NZ"), None);
    assert_eq!(first(&d, PlanKind::Fm, "NZ").as_deref(), Some("fm_nz"));
    assert_eq!(first(&d, PlanKind::Amateur, "AU").as_deref(), Some("ham_au"));
    assert_eq!(first(&d, PlanKind::Amateur, "US").as_deref(), Some("ham_r2"));
    assert_eq!(first(&d, PlanKind::Amateur, "DE").as_deref(), Some("ham_r1"));
    assert_eq!(first(&d, PlanKind::Marine, "US").as_deref(), Some("marine_us"));
    assert_eq!(first(&d, PlanKind::Aviation, "DE").as_deref(), Some("air_833"));
    assert_eq!(first(&d, PlanKind::AnalogTv, "AU").as_deref(), Some("an_au"));
    assert_eq!(first(&d, PlanKind::Cable, "AU"), None);
    assert_eq!(d.plan_ids_of(PlanKind::Personal, "AU"), ["uhf_cb", "cb40"]);
    assert_eq!(
        d.plan_ids_of(PlanKind::Personal, "DE"),
        ["pmr446", "dpmr446", "cb80_de", "lpd433"]
    );
    assert!(d
        .plan_ids_of(PlanKind::Personal, "US")
        .contains(&"frs_gmrs".to_owned()));
    // A code that no row names has no plan.
    assert!(d.plan_ids("XX").is_empty());
    let all = d.plan_ids("AU");
    assert!(all.contains(&"tv_au".to_owned()) && all.contains(&"uhf_cb".to_owned()));
    assert!(!all.contains(&"tv_us".to_owned()));
}

#[test]
fn scan_channels_come_from_the_named_plans() {
    let d = data();
    let au = d.scan_channels(&["tv_au", "no_such_plan"]);
    let dvbt = &au["dvbt"];
    assert!(dvbt.windows(2).all(|w| w[0].1 < w[1].1));
    assert!(dvbt.contains(&("12".to_owned(), 226.5e6)));
    let de = d.scan_channels(&["tv_eu_uhf"]);
    assert!(de.values().flatten().any(|c| c.0 == "21" && c.1 == 474e6));
    assert!(!de.values().flatten().any(|c| c.1 == 226.5e6));
    assert!(d.scan_channels(&[]).is_empty());
}

#[test]
fn a_search_finds_plans_channels_and_entries() {
    let d = data();
    let entries = d.frequency_entries().unwrap();
    let au: Vec<String> = d.plan_ids("AU");
    let au: Vec<&str> = au.iter().map(String::as_str).collect();
    // A plan by its id, with its channels because the plan is named.
    let hits = d.search("tv_au", &au, &[]);
    assert_eq!((hits[0].kind, hits[0].id.as_str()), (HitKind::Plan, "tv_au"));
    assert!(hits
        .iter()
        .any(|h| h.kind == HitKind::Channel && h.id == "tv_au/12"));
    // The same plan without its channels when the host did not name it.
    let hits = d.search("tv_au", &[], &[]);
    assert!(hits.iter().all(|h| h.kind == HitKind::Plan));
    // A frequency: the channel that holds it, then the entries near it.
    let hits = d.search("226.5", &au, &entries);
    assert_eq!(hits[0].kind, HitKind::Channel);
    assert_eq!(hits[0].frequency_hz, Some(226.5e6));
    let hits = d.search("145.175", &au, &entries);
    assert!(hits
        .iter()
        .any(|h| h.kind == HitKind::Entry && h.id == "b:aprs-au"));
    // A channel name.
    let hits = d.search("ch 12", &["tv_au"], &[]);
    assert_eq!(hits[0].id, "tv_au/12");
    // An entry by a word of its name.
    let hits = d.search("aprs", &[], &entries);
    assert!(hits.iter().any(|h| h.kind == HitKind::Entry));
    assert!(hits.windows(2).all(|w| w[0].score >= w[1].score));
    assert!(d.search("", &au, &entries).is_empty());
}

#[test]
fn labels_of_a_span() {
    let d = data();
    let entries = d.frequency_entries().unwrap();
    let view = View {
        lo_hz: 174e6,
        hi_hz: 230e6,
        width_px: 1200.0,
        char_px: 7.0,
        rows: 3,
    };
    let l = d.labels(&[], &["tv_au"], &[], &view);
    assert!(l.points.iter().any(|p| p.text == "12" && p.hz == 226.5e6));
    assert!(l.points.iter().all(|p| p.hz >= 174e6 && p.hz <= 230e6));
    let none = d.labels(&[], &[], &[], &view);
    assert!(none.points.is_empty() && none.spans.is_empty());
    let two_m = View {
        lo_hz: 144e6,
        hi_hz: 146e6,
        ..view
    };
    let au = d.labels(&entries, &[], &d.frequency_scope("AU"), &two_m);
    let nz = d.labels(&entries, &[], &d.frequency_scope("NZ"), &two_m);
    assert!(au.points.iter().any(|p| p.id == "b:aprs-au"));
    assert!(!nz.points.iter().any(|p| p.id == "b:aprs-au"));
}

#[test]
fn names_and_steps() {
    let d = data();
    assert_eq!(d.name_of(226.5e6, &["tv_au"]).unwrap().name, "12");
    assert_eq!(d.name_of(474.1e6, &["tv_eu_uhf"]).unwrap().name, "21");
    assert!(d.name_of(226.5e6, &[]).is_none());
    assert!(d.places(226e6, &["tv_au", "dab_band3"]).len() >= 2);
    assert_eq!(
        d.channel_named("ch 9A", &["tv_au"]).unwrap().centre_hz,
        205.5e6
    );
    assert_eq!(
        d.channel_named("9A", &["dab_band3", "tv_au"])
            .unwrap()
            .centre_hz,
        202.928e6
    );
    let (up, name) = d.step("tv_au", 226.5e6, true, false).unwrap();
    assert!(up > 226.5e6 && !name.is_empty());
    let (down, _) = d.step("tv_au", up, false, false).unwrap();
    assert_eq!(down, 226.5e6);
    assert!(d.step("no_such_plan", 1e6, true, false).is_none());
}

#[test]
fn the_users_plan_file() {
    let dir = std::env::temp_dir().join(format!("sdrkit-presets-lookup-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let file = dir.join("mine/plans.json");
    let mut d = data();
    assert_eq!(d.open_personal_plans(&file).unwrap(), 0);
    let csv = "name,centre_hz,width_hz\nA,100000000,200000\nB,100200000,200000\n";
    let ids = d
        .import_plan(csv, PlanFormat::Csv, "my_list", "My list")
        .unwrap();
    assert_eq!(ids, ["my_list"]);
    assert_eq!(d.name_of(100.2e6, &["my_list"]).unwrap().name, "B");
    assert_eq!(d.step("my_list", 100e6, true, false).unwrap().0, 100.2e6);
    // A text that is not a plan adds nothing.
    assert!(matches!(
        d.import_plan("{", PlanFormat::Json, "bad", "Bad"),
        Err(DataError::Format { .. })
    ));
    // The export of a plan reads back as the same channels.
    let text = d.export_plan("my_list", PlanFormat::Csv).unwrap();
    assert!(text.contains(",B,100.2,0.2"), "{text}");
    assert!(d.export_plan("tv_au", PlanFormat::Json).unwrap().contains("\"tv_au\""));
    assert!(d.export_plan("no_such_plan", PlanFormat::Csv).is_none());
    // Another open of the folder with the same file has the plan.
    let mut again = data();
    assert_eq!(again.open_personal_plans(&file).unwrap(), 1);
    assert_eq!(again.plans().get("my_list").unwrap().channels.len(), 2);
    assert!(again.remove_plan("my_list").unwrap());
    assert!(!again.remove_plan("my_list").unwrap());
    assert!(!again.remove_plan("tv_au").unwrap());
    let mut last = data();
    assert_eq!(last.open_personal_plans(&file).unwrap(), 0);
    assert!(last.plans().get("my_list").is_none());
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn the_regions_are_the_countries_with_plans() {
    let d = data();
    let regions = d.regions();
    let au = regions.iter().find(|r| r.code == "AU").expect("AU");
    assert_eq!(au.line(), "AU  Australia");
    assert!(regions.windows(2).all(|w| w[0].code < w[1].code));
    assert!(regions.iter().all(|r| !d.plan_ids(&r.code).is_empty()));
    assert_eq!(d.region("au").map(|r| r.country().to_owned()), Ok("AU".to_owned()));
    let list = d.region("XX").unwrap_err();
    assert!(list.lines().any(|l| l == "AU  Australia"), "{list}");
    assert!(d.region("Australia").is_err());
}

#[test]
fn the_channels_of_a_stack_come_with_label_frequency_and_width() {
    let d = data();
    let au = sdrkit_presets::Region::new("AU").unwrap();
    let dvbt = d.stack_channels("dvbt", &au).unwrap();
    let seven = dvbt.iter().find(|c| c.label == "7").expect("channel 7");
    assert_eq!((seven.frequency_hz, seven.width_hz), (184.5e6, Some(7e6)));
    let de = sdrkit_presets::Region::new("DE").unwrap();
    let dvbt = d.stack_channels("dvbt", &de).unwrap();
    assert!(dvbt.iter().all(|c| c.width_hz == Some(8e6)));
    // A stack without a plan: the dials of its family file.
    assert!(d
        .stack_channels("lbj", &au)
        .unwrap()
        .iter()
        .any(|c| c.frequency_hz == 821.2375e6));
    assert_eq!(d.dial_hz("freedv", Some("700D")).unwrap(), Some(14.236e6));
    assert!(d.stack_channels("no_such_stack", &au).unwrap().is_empty());
}

#[test]
fn the_channels_of_a_stack_hold_the_users_plans_for_the_stack() {
    let dir = std::env::temp_dir().join(format!("sdrkit-presets-own-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let mut d = data();
    d.open_personal_plans(dir.join("plans.json")).unwrap();
    let au = sdrkit_presets::Region::new("AU").unwrap();
    let before = d.stack_channels("dvbt", &au).unwrap();
    // One channel at a new frequency and one at the frequency of channel 7 of the folder.
    let csv = "# modes: dvbt\n# countries: AU\nname,centre_hz,width_hz\nMine,600000000,7000000\nSeven,184500000,7000000\n";
    d.import_plan(csv, PlanFormat::Csv, "my_tv", "My TV").unwrap();
    let after = d.stack_channels("dvbt", &au).unwrap();
    assert_eq!(after.len(), before.len() + 1);
    assert!(after.windows(2).all(|w| w[0].frequency_hz <= w[1].frequency_hz));
    let at = |hz: f64| after.iter().filter(|c| c.frequency_hz == hz).map(|c| c.label.as_str()).collect::<Vec<_>>();
    assert_eq!((at(600e6), at(184.5e6)), (vec!["Mine"], vec!["Seven"]));
    // Another country and another stack do not get the plan.
    let de = sdrkit_presets::Region::new("DE").unwrap();
    assert!(d.stack_channels("dvbt", &de).unwrap().iter().all(|c| c.label != "Mine"));
    assert!(d.stack_channels("dvbt2", &au).unwrap().iter().all(|c| c.label != "Mine"));
    // A plan without stacks is not a channel list of any stack.
    let csv = "name,centre_hz,width_hz\nLoose,610000000,7000000\n";
    d.import_plan(csv, PlanFormat::Csv, "loose", "Loose").unwrap();
    assert!(d.stack_channels("dvbt", &au).unwrap().iter().all(|c| c.label != "Loose"));
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn the_dials_of_a_stack_are_rows_of_the_dial_file() {
    let d = data();
    let au = sdrkit_presets::Region::new("AU").unwrap();
    let ft8 = d.dials("ft8", &au, None).unwrap();
    assert!(ft8.iter().any(|c| c.label == "20m" && c.frequency_hz == 14.074e6), "{ft8:?}");
    assert!(ft8.windows(2).all(|w| w[0].frequency_hz <= w[1].frequency_hz));
    // The family stack in a submode has the dials of the submode; without one, every dial with its stack.
    let in_ft4 = d.dials("wsjt", &au, Some("ft4")).unwrap();
    assert!(!in_ft4.is_empty() && in_ft4.iter().all(|c| !c.label.contains(' ')));
    let every = d.dials("wsjt", &au, None).unwrap();
    assert!(every.len() > ft8.len() && every.iter().any(|c| c.label == "FT8 20m"));
    // A stack with rows of its own but without the setting has its rows; a stack without either has none.
    assert!(!d.dials("jt65", &au, None).unwrap().is_empty());
    assert!(d.dials("dvbt", &au, None).unwrap().is_empty());
}

/// The plan file of the Swift test of the personal plans: one plan of one channel for the stack nfm.
const PERSONAL_PLAN: &str = r#"{"format": 1, "groups": [{"id": "mine", "name": "Mine", "kind": "other",
"source": {"publisher": "The user", "title": "Mine", "date": "2026"}, "verified": false,
"unverified_reason": "made by the user", "bands": [{"name": "Mine", "service": "land_mobile",
"start_hz": 150000000, "end_hz": 152000000, "modes": ["nfm"],
"channels": [{"name": "Mine", "centre_hz": 151000000, "width_hz": 12500}]}]}]}"#;

#[test]
fn a_personal_plan_file_gives_channels_to_its_stack_in_every_country() {
    let dir = std::env::temp_dir().join(format!("sdrkit-presets-file-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("plans.json");
    std::fs::write(&file, PERSONAL_PLAN).unwrap();
    let mut d = data();
    assert_eq!(d.open_personal_plans(&file).unwrap(), 1);
    for cc in ["AU", "DE"] {
        let region = sdrkit_presets::Region::new(cc).unwrap();
        let mine: Vec<_> = d
            .stack_channels("nfm", &region)
            .unwrap()
            .into_iter()
            .filter(|c| c.label == "Mine")
            .map(|c| (c.frequency_hz, c.width_hz))
            .collect();
        assert_eq!(mine, [(151e6, Some(12500.0))], "{cc}");
    }
    std::fs::remove_dir_all(&dir).unwrap();
}

/// A mesh stack whose plans are band plans with a dial lists the dial of every band plan of its country, not only the
/// first.
#[test]
fn a_mesh_stack_lists_every_band_plan_of_its_country() {
    let d = data();
    let au = sdrkit_presets::Region::new("AU").unwrap();
    let hz = |stack: &str, r: &sdrkit_presets::Region| -> Vec<f64> {
        d.stack_channels(stack, r)
            .unwrap()
            .iter()
            .map(|c| c.frequency_hz)
            .collect()
    };
    assert_eq!(hz("meshtastic", &au), [915.075e6, 919.875e6]);
    assert_eq!(hz("meshcore", &au), [915.075e6, 915.8e6]);
    let names: Vec<String> = d
        .stack_channels("meshtastic", &au)
        .unwrap()
        .into_iter()
        .map(|c| c.label)
        .collect();
    assert!(names.iter().all(|n| n.starts_with("ANZ 915")), "{names:?}");
    let de = sdrkit_presets::Region::new("DE").unwrap();
    assert_eq!(hz("loraaprs", &de), [433.775e6, 434.855e6, 439.9125e6]);
    assert_eq!(hz("mesh", &de).len(), 6);
    assert_eq!(hz("loraaprs", &au), [433.775e6]);
}

/// The supplied frequency entries of a stack in a country: those of its stack column, and those of the general receiver
/// with the stack id as a tag, where their scope reaches the country.
#[test]
fn the_frequency_entries_of_a_stack_in_a_country() {
    let d = data();
    let region = |cc: &str| sdrkit_presets::Region::new(cc).unwrap();
    let ft8 = d.frequency_rows("ft8", &region("AU")).unwrap();
    assert_eq!(ft8.len(), 12);
    assert!(ft8.windows(2).all(|w| w[0].frequency_hz <= w[1].frequency_hz));
    let twenty = ft8.iter().find(|r| r.frequency_hz == 14.074e6).unwrap();
    assert!(twenty.id.starts_with("b:ft8-") && !twenty.name.is_empty());
    assert_eq!(twenty.bandwidth_hz, Some(3000.0));
    assert!(!twenty.note.is_empty());
    let ids = |stack: &str, cc: &str| -> Vec<String> {
        d.frequency_rows(stack, &region(cc))
            .unwrap()
            .into_iter()
            .map(|r| r.id)
            .collect()
    };
    // An entry of a country applies there only; an entry of an ITU region in its countries; LoRa APRS is not APRS.
    assert_eq!(ids("aprs", "AU"), ["b:aprs-au", "b:iss-aprs"]);
    assert!(!ids("aprs", "NZ").contains(&"b:aprs-au".to_owned()));
    assert!(ids("aprs", "DE").contains(&"b:aprs-r1".to_owned()));
    assert!(ids("no_such_stack", "AU").is_empty());
}

#[test]
fn the_meshtastic_slots_of_a_region_band_plan_or_country() {
    let d = data();
    // ANZ LongFast: 52 slots, the default channel at slot 20 on 919.875 MHz.
    for region in ["ANZ", "ANZ 915", "AU"] {
        let anz = d.meshtastic_slots(region, "LongFast", "", 0).unwrap();
        assert_eq!(anz.len(), 52, "{region}");
        let cur: Vec<_> = anz.iter().filter(|r| r.current).collect();
        assert_eq!(cur.len(), 1);
        assert_eq!(
            (cur[0].slot, cur[0].freq_hz, cur[0].preset_default),
            (20, 919.875e6, true)
        );
    }
    // US LongFast: 104 slots, slot 20 at 906.875 MHz; a channel number moves the current slot.
    let us = d.meshtastic_slots("US", "", "", 0).unwrap();
    assert_eq!(us.len(), 104);
    assert_eq!(us[19].freq_hz, 906.875e6);
    assert!(us[19].current && us[19].preset_default);
    let num = d.meshtastic_slots("US", "LongFast", "", 7).unwrap();
    assert_eq!(num.iter().position(|r| r.current), Some(6));
    // US LongSlow: 208 narrow slots, the last one can be chosen and the next is refused.
    assert_eq!(
        d.meshtastic_slots("US", "LongSlow", "", 208).unwrap().len(),
        208
    );
    assert!(matches!(
        d.meshtastic_slots("US", "LongSlow", "", 209),
        Err(sdrkit_presets::PresetError::ChannelNumTooHigh(_))
    ));
    assert!(matches!(
        d.meshtastic_slots("Atlantis", "LongFast", "", 0),
        Err(sdrkit_presets::PresetError::NoData { .. })
    ));
    assert!(matches!(
        d.meshtastic_slots("US", "NoSuchPreset", "", 0),
        Err(sdrkit_presets::PresetError::NoData { .. })
    ));
}
