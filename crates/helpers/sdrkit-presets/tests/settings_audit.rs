//! Every setting that the helper makes from the data folder of the repository is a setting of its stack today, and
//! its value is one that the setting takes.

use sdrkit_core::Value;
use sdrkit_presets::{preset_values, Data, Options, Preset, PresetError, Region};

type Settings = Vec<(String, Value)>;

fn data() -> Data {
    Data::open(concat!(env!("CARGO_MANIFEST_DIR"), "/../../../data")).expect("the data folder")
}

/// The settings that each helper makes: the stack, where they come from, the settings.
fn made(d: &Data) -> Vec<(String, String, Settings)> {
    let mut out: Vec<(String, String, Settings)> = Vec::new();
    let mut add = |stack: &str, from: String, s: Settings| out.push((stack.to_owned(), from, s));
    for b in d.fm_bands() {
        add("fm", format!("fm band {}", b.id), b.settings());
    }
    for p in d.marine_plans().unwrap() {
        for stack in ["marinevhf", "atis"] {
            add(stack, format!("marine plan {}", p.id), p.settings());
        }
    }
    for b in d.wefax_broadcasts().unwrap() {
        add("marine", "radiofax broadcast".into(), b.settings());
    }
    let groups = d.dab_groups().unwrap();
    let ids: Vec<&str> = groups.iter().map(|g| g.id.as_str()).collect();
    add("dab", "dab groups".into(), sdrkit_presets::dab::dab_settings(&groups, &ids).unwrap());
    for p in d.atv_plans() {
        add("atv", format!("analogue plan {}", p.id), p.settings());
    }
    if let Some(p) = d.fpv_channels() {
        add("fpv", "fpv channels".into(), p.settings());
    }
    let cell = sdrkit_presets::cell::read_cell_lists(&d.folder().join("cell/band_order.csv")).unwrap();
    for r in cell.iter().filter(|r| r.list == "band_order") {
        let s = sdrkit_presets::cell::cell_settings(&cell, &r.stack, &r.name).unwrap();
        add(&r.stack, format!("band order {}", r.name), s);
    }
    let links = d.datalink_channels().unwrap();
    for r in &links {
        let s = sdrkit_presets::avdata::settings(&links, &r.stack, &r.set).unwrap();
        add(&r.stack, format!("data link set {}", r.set), s);
    }
    for r in d.zwave_regions().unwrap() {
        add("zwave", format!("zwave region {}", r.id), r.settings());
    }
    for p in d.wisun_bands().unwrap() {
        add("wisun", format!("wisun plan {}", p.id), p.settings());
    }
    let wifi = d.wifi_domains().unwrap();
    for w in &wifi {
        let s = sdrkit_presets::wlan::wifi_settings(&wifi, &w.domain, false).unwrap();
        add("wifi", format!("wifi domain {}", w.domain), s);
    }
    let halow = d.halow_regions().unwrap();
    for h in &halow {
        let s = sdrkit_presets::wlan::halow_settings(&halow, &h.code).unwrap();
        add("halow", format!("halow region {}", h.code), s);
    }
    let general = sdrkit_presets::general::presets_setting(&d.general_presets().unwrap());
    add("general", "general presets".into(), vec![general]);
    for p in d.mesh_band_plans().unwrap() {
        for stack in &p.stacks {
            let s = d.mesh_band_plan_settings(&p.plan, stack).unwrap();
            add(stack, format!("mesh band plan {}", p.plan), s.settings);
        }
    }
    for p in d.lorawan_plans().unwrap() {
        let s = d.lorawan_settings(&p.plan, 0).unwrap();
        add("lorawanwide", format!("lorawan plan {}", p.plan), s.settings);
    }
    // The preset of every stack in the countries of the owner's tests and in every eighth country that has plans.
    let regions = d.regions();
    let picked = |i: usize, code: &str| i.is_multiple_of(8) || ["AU", "US", "DE", "JP", "GB"].contains(&code);
    for (_, region) in regions.iter().enumerate().filter(|(i, r)| picked(*i, &r.code)) {
        let r = Region::new(&region.code).unwrap();
        for s in sdrkit_host_catalog::all() {
            match d.preset(s.id, &r, &Options::default()) {
                Ok(p) => add(s.id, format!("preset in {}", region.code), p.settings),
                Err(PresetError::NoData { .. }) => {}
                Err(e) => panic!("{} in {}: {e}", s.id, region.code),
            }
        }
    }
    out
}

#[test]
fn every_setting_of_the_helper_is_a_setting_of_its_stack() {
    let d = data();
    let engine = sdrkit_host_catalog::engine().expect("an engine");
    let mut bad: Vec<String> = Vec::new();
    let mut checked = 0;
    for (stack, from, settings) in made(&d) {
        // One setting at a time, so that the message names each wrong key.
        for (key, value) in settings {
            checked += 1;
            let p = Preset {
                stack: stack.clone(),
                submode: None,
                plan: String::new(),
                settings: vec![(key.clone(), value)],
                frequency_hz: 0.0,
                width_hz: None,
                station: None,
                channels: Vec::new(),
            };
            if let Err(e) = preset_values(&engine, &p) {
                let line = format!("{stack}.{key}: {e}");
                if !bad.iter().any(|b| b.starts_with(&format!("{stack}.{key}:"))) {
                    bad.push(format!("{line} ({from})"));
                }
            }
        }
    }
    assert!(checked > 500, "{checked}");
    assert!(bad.is_empty(), "{} wrong settings:\n{}", bad.len(), bad.join("\n"));
}
