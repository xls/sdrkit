//! The stacks with the values of rows of the data folder: the plan that a band plan, a Meshtastic region or a country gives,
//! and every mesh stack on its test signal with the values of two rows.

use crate::common::{band_plan, data, feed, packets, receiver, rows, settings_of, slot_band};
use sdrkit_core::publish::keys;
use sdrkit_core::verify::CaseSetting;
use sdrkit_core::{Complex32, InputInfo, Output, StackDescriptor, Value};
use sdrkit_dev_test::Harness;
use sdrkit_presets::PresetError;
use sdrkit_stack_lora::gen::mesh_config;
use sdrkit_stack_lora::mesh::plan::SLOTS_CAP;
use sdrkit_stack_lora::mesh::plan::{build, MeshConfig, PlanEntry};
use sdrkit_stack_lora::mesh::types::Protocol;
use sdrkit_stack_lora::proto::meshtastic as mt;
use sdrkit_stack_lora::settings::{MT_CHANNEL_NUM, MT_OVERRIDE_FREQ};
use sdrkit_stack_lora::typed::{MeshSlotRow, MESH_SLOTS};

/// The channel settings of the mesh stack `stack` with the test settings `s`, as its receiver takes them.
fn config(d: &'static StackDescriptor, stack: &str, s: &[CaseSetting]) -> MeshConfig {
    let values = Harness::new(d)
        .with_settings(s)
        .values()
        .expect("the values");
    mesh_config(stack, &values)
}

#[test]
fn the_eu_868_plan_hears_longfast_and_meshcore_narrow() {
    let (dial, s) = band_plan("mesh", "EU 868");
    let cfg = config(&sdrkit_stack_lora::MESH, "mesh", &s);
    let p = build(&cfg, Some(dial), 869.825e6, 2e6);
    let lf = p
        .entries
        .iter()
        .find(|e| e.name.starts_with("LongFast"))
        .unwrap();
    assert_eq!(lf.freq_hz, 869.525e6);
    assert!(lf.in_band);
    // MeshCom 868 shares LongFast's setting.
    assert!(lf.name.contains("MeshCom 868"), "{}", lf.name);
    assert_eq!(lf.mask & 9, 9);
    let mc = p
        .entries
        .iter()
        .find(|e| e.name.contains("EU/UK (Narrow)"))
        .unwrap();
    assert!(mc.in_band);
    assert!(
        p.line.starts_with("Mesh: listening to LongFast"),
        "{}",
        p.line
    );
    // 433 MHz presets are out of band.
    assert!(p
        .entries
        .iter()
        .filter(|e| e.freq_hz < 500e6)
        .all(|e| !e.in_band));
}

#[test]
fn meshcore_networks_on_one_frequency_get_one_detector_per_sf() {
    let (dial, s) = band_plan("meshcore", "EU 868");
    let cfg = config(&sdrkit_stack_lora::MESHCORE, "meshcore", &s);
    let p = build(&cfg, Some(dial), 869.825e6, 2e6);
    let at: Vec<&PlanEntry> = p
        .entries
        .iter()
        .filter(|e| e.freq_hz == 869.618e6)
        .collect();
    let sfs: std::collections::BTreeSet<u8> = at.iter().map(|e| e.params.sf).collect();
    assert_eq!(at.len(), sfs.len());
    assert!(sfs.contains(&7) && sfs.contains(&8));
}

#[test]
fn lorawan_us_plan_lists_only_what_fits() {
    let (dial, s) = band_plan("lorawan", "LoRaWAN US915");
    let cfg = config(&sdrkit_stack_lora::LORAWAN, "lorawan", &s);
    let p = build(&cfg, Some(dial), 903.9e6, 2e6);
    assert!(p.entries.iter().all(|e| e.in_band));
    assert!(!p.entries.is_empty());
    let (dial, s) = band_plan("lorawan", "LoRaWAN EU868");
    let cfg = config(&sdrkit_stack_lora::LORAWAN, "lorawan", &s);
    let p = build(&cfg, Some(dial), 868.3e6, 1e6);
    assert!(p.line.contains("SF7-12"), "{}", p.line);
}

#[test]
fn a_station_in_australia_takes_the_anz_plan_and_longfast_slot() {
    let region = data().meshtastic_region_of("au").unwrap().unwrap();
    assert_eq!(region, "ANZ");
    let plan = data().mesh_band_plan_in("mesh", &region).unwrap().unwrap();
    assert_eq!(plan, "ANZ 915");
    let (dial, s) = band_plan("mesh", &plan);
    let cfg = config(&sdrkit_stack_lora::MESH, "mesh", &s);
    assert_eq!(cfg.dial(None), 919.875e6);
    assert_eq!(dial, 919.875e6);
    assert_eq!(cfg.mt_slot_hz(dial), Some(919.875e6));
}

#[test]
fn a_region_that_no_plan_covers_dials_its_longfast_slot() {
    let region = data().meshtastic_region_of("KR").unwrap().unwrap();
    assert_eq!(region, "KR");
    assert_eq!(
        data().mesh_band_plan_in("meshtastic", &region).unwrap(),
        None
    );
    let s = settings_of(&data().meshtastic_settings(&region, "LongFast").unwrap());
    let cfg = config(&sdrkit_stack_lora::MESHTASTIC, "meshtastic", &s);
    let kr = slot_band("KR");
    let dial = cfg.dial(None);
    assert!(dial > kr.start_hz && dial < kr.end_hz);
    assert_eq!(Some(dial), cfg.mt_slot_hz(dial));
}

#[test]
fn a_band_set_by_hand_replaces_the_band_of_the_plan() {
    let (_, mut s) = band_plan("mesh", "EU 868");
    s.extend(settings_of(
        &data().meshtastic_settings("US", "LongFast").unwrap(),
    ));
    let cfg = config(&sdrkit_stack_lora::MESH, "mesh", &s);
    assert_eq!(cfg.dial(None), 869.525e6);
    assert_eq!(cfg.mt_band, slot_band("US"));
    assert!(data().meshtastic_region_of("XX").unwrap().is_none());
}

/// Two rows of the data for each mesh stack: the settings and the frequency to tune to.
fn two_rows(stack: &str) -> [(f64, Vec<CaseSetting>); 2] {
    match stack {
        "mesh" => [band_plan(stack, "EU 868"), band_plan(stack, "US 915")],
        "meshtastic" => [band_plan(stack, "EU 868"), band_plan(stack, "ANZ 915")],
        "meshcore" => [band_plan(stack, "US 915"), band_plan(stack, "ANZ 915")],
        "loraaprs" => [
            band_plan(stack, "LoRa APRS EU"),
            band_plan(stack, "LoRa APRS PL"),
        ],
        "lorawan" => [
            band_plan(stack, "LoRaWAN EU868"),
            band_plan(stack, "LoRaWAN US915"),
        ],
        _ => {
            // MeshCom: the band plan, and the channel MeshCom 868 of the amateur channels as the dial.
            let first = band_plan(stack, "MeshCom EU");
            let c = data()
                .ham_lora_channels()
                .unwrap()
                .into_iter()
                .find(|c| c.label == "MeshCom 868")
                .unwrap();
            let mut s = first.1.clone();
            s.retain(|c| c.key != "dial_freq");
            s.push(CaseSetting {
                key: "dial_freq",
                value: Value::Hz(c.frequency_hz),
            });
            [first, (c.frequency_hz, s)]
        }
    }
}

#[test]
fn every_mesh_stack_runs_with_two_rows_of_the_data() {
    use sdrkit_stack_lora as l;
    for (d, protocol, secs) in [
        (&l::MESH, Protocol::Meshtastic, 3.0),
        (&l::MESHTASTIC, Protocol::Meshtastic, 3.0),
        (&l::MESHCORE, Protocol::MeshCore, 10.0),
        (&l::LORAAPRS, Protocol::LoraAprs, 8.0),
        (&l::MESHCOM, Protocol::MeshCom, 3.0),
        (&l::LORAWAN, Protocol::Lorawan, 3.0),
    ] {
        for (dial, s) in two_rows(d.id) {
            let h = Harness::new(d).seconds(secs).with_settings(&s);
            let x = h.generate().expect("the test signal");
            let mut rx = h.receiver().expect("the receiver");
            let rate = d.tuning.rate_hz;
            let ev = feed(
                rx.as_mut(),
                &x,
                rate,
                dial + d.tuning.tune_offset_hz,
                dial,
                65536,
            );
            assert!(
                !packets(&ev, Some(protocol)).is_empty(),
                "{} at {dial}: {:?}",
                d.id,
                rx.results().log_lines()
            );
        }
    }
}

/// The `slots` table of a Meshtastic receiver on the settings of a region: (slot, frequency, current, preset default) of
/// every row, lowest slot first.
fn slot_rows(region: &str, extra: &[CaseSetting]) -> Vec<(i64, f64, bool, bool)> {
    let mut s = settings_of(&data().meshtastic_settings(region, "LongFast").unwrap());
    s.extend_from_slice(extra);
    let mut rx = receiver(&sdrkit_stack_lora::MESHTASTIC, Some("meshtastic"), &s);
    let dial = config(&sdrkit_stack_lora::MESHTASTIC, "meshtastic", &s).dial(None);
    rx.configure(&InputInfo::new(2e6, dial + 200e3, dial).recording())
        .unwrap();
    let mut out = Output::default();
    rx.feed(&vec![Complex32::default(); 400_000], &mut out);
    let mut v: Vec<_> = rows(rx.results(), &MESH_SLOTS)
        .into_iter()
        .map(|r| {
            (
                r.at(&MeshSlotRow::SLOT).unwrap(),
                r.get(&keys::FREQ_HZ).unwrap(),
                r.at(&MeshSlotRow::CURRENT).unwrap(),
                r.at(&MeshSlotRow::PRESET_DEFAULT).unwrap(),
            )
        })
        .collect();
    v.sort_by_key(|r| r.0);
    v
}

#[test]
fn the_slot_table_lists_every_slot_of_the_band() {
    // ANZ, LongFast: 52 slots of 250 kHz from 915.125 MHz; the default channel takes slot 20 at 919.875 MHz.
    let anz = slot_rows("ANZ", &[]);
    assert_eq!(anz.len(), 52);
    assert_eq!(anz[0], (1, 915.125e6, false, false));
    assert_eq!(anz[51].1, 927.875e6);
    let cur: Vec<_> = anz.iter().filter(|r| r.2).collect();
    assert_eq!(cur, [&(20, 919.875e6, true, true)]);
    // US, LongFast: 104 slots; slot 20 at 906.875 MHz.
    let us = slot_rows("US", &[]);
    assert_eq!(us.len(), 104);
    assert_eq!(
        us.iter().filter(|r| r.2).collect::<Vec<_>>(),
        [&(20, 906.875e6, true, true)]
    );
    // A channel number moves the current slot; the preset default stays.
    let num = slot_rows("ANZ", &[CaseSetting::new(&MT_CHANNEL_NUM, 3)]);
    assert_eq!(num.iter().find(|r| r.2).map(|r| r.0), Some(3));
    assert_eq!(num.iter().find(|r| r.3).map(|r| r.0), Some(20));
    // An override frequency between slots leaves no slot current; one at the centre of a slot makes it current.
    let over = slot_rows("ANZ", &[CaseSetting::new(&MT_OVERRIDE_FREQ, 916.0e6)]);
    assert_eq!(over.len(), 52);
    assert!(over.iter().all(|r| !r.2));
    let at = slot_rows("ANZ", &[CaseSetting::new(&MT_OVERRIDE_FREQ, 915.625e6)]);
    assert_eq!(
        at.iter().filter(|r| r.2).collect::<Vec<_>>(),
        [&(3, 915.625e6, true, false)]
    );
}

#[test]
fn the_slots_without_a_session_are_the_rows_of_a_session() {
    // The helper of the data folder and a running stack give the same rows.
    for (region, num) in [("ANZ", 0), ("ANZ", 3), ("US", 0), ("EU_868", 0)] {
        let helper: Vec<_> = data()
            .meshtastic_slots(region, "LongFast", "", num)
            .unwrap()
            .into_iter()
            .map(|r| (i64::from(r.slot), r.freq_hz, r.current, r.preset_default))
            .collect();
        let extra = [CaseSetting::new(&MT_CHANNEL_NUM, i64::from(num))];
        assert_eq!(helper, slot_rows(region, &extra), "{region} {num}");
    }
}

#[test]
fn a_channel_number_above_the_slots_of_the_band_is_refused() {
    // ANZ has 52 LongFast slots: 52 is taken, 53 is refused, by a session and without one.
    let mut s = settings_of(&data().meshtastic_settings("ANZ", "LongFast").unwrap());
    s.push(CaseSetting::new(&MT_CHANNEL_NUM, 52));
    assert!(Harness::new(&sdrkit_stack_lora::MESHTASTIC)
        .with_settings(&s)
        .receiver()
        .is_ok());
    s.push(CaseSetting::new(&MT_CHANNEL_NUM, 53));
    assert!(Harness::new(&sdrkit_stack_lora::MESHTASTIC)
        .with_settings(&s)
        .receiver()
        .is_err());
    assert!(matches!(
        data().meshtastic_slots("ANZ", "LongFast", "", 53),
        Err(PresetError::ChannelNumTooHigh(e)) if e.slots == 52 && e.channel_num == 53
    ));
    // Without a slot band any number is taken: the stack listens at the dial.
    assert!(receiver_ok(&[CaseSetting::new(&MT_CHANNEL_NUM, 900)]));
}

fn receiver_ok(s: &[CaseSetting]) -> bool {
    Harness::new(&sdrkit_stack_lora::MESHTASTIC)
        .with_settings(s)
        .receiver()
        .is_ok()
}

#[test]
fn the_channel_number_covers_every_slot_of_every_region_and_preset() {
    // The setting ends at the cap of the table; every region of the data with every preset has no more slots.
    assert_eq!(MT_CHANNEL_NUM.range.map(|r| r.max), Some(SLOTS_CAP as f64));
    let mut largest = (0, String::new());
    for r in data().meshtastic_regions().unwrap() {
        let band = mt::SlotBand {
            start_hz: r.start_hz,
            end_hz: r.end_hz,
        };
        for p in mt::PRESETS {
            let n = mt::slot_count(p, &band);
            if n > largest.0 {
                largest = (n, format!("{} {}", r.name, p.name));
            }
        }
    }
    assert!(largest.0 as usize <= SLOTS_CAP, "{largest:?}");
    assert_eq!(largest, (640, "CN EdgeFastLow".to_string()));
}

#[test]
fn the_slot_grid_follows_the_channel_settings() {
    let s = settings_of(&data().meshtastic_settings("ANZ", "LongFast").unwrap());
    let mut cfg = config(&sdrkit_stack_lora::MESHTASTIC, "meshtastic", &s);
    let g = cfg.slot_grid(0.0).unwrap();
    assert_eq!(g.count(), 52);
    // The current slot is the one the stack listens on, for every preset and a channel name.
    for p in mt::PRESETS {
        cfg.mt_preset = p.name.into();
        for name in ["", "Private"] {
            cfg.mt_channel_name = name.into();
            let g = cfg.slot_grid(0.0).unwrap();
            assert_eq!(
                g.current.map(|i| g.freq_hz(i)),
                cfg.mt_slot_hz(0.0),
                "{} {name:?}",
                p.name
            );
            let d = mt::slot_hz(p, &slot_band("ANZ"), "", 0, 0.0, 0.0);
            assert_eq!(g.preset_default.map(|i| g.freq_hz(i)), d, "{}", p.name);
        }
    }
    // Without a band: one slot at the dial. With the custom setting or without Meshtastic: none.
    let mut bare = MeshConfig::for_submode("meshtastic");
    let g = bare.slot_grid(916e6).unwrap();
    assert_eq!((g.count(), g.freq_hz(0)), (1, 916e6));
    bare.custom = true;
    assert!(bare.slot_grid(916e6).is_none());
    assert!(MeshConfig::for_submode("meshcore")
        .slot_grid(916e6)
        .is_none());
}

#[test]
fn the_refusal_of_a_channel_number_does_not_depend_on_the_order_of_the_settings() {
    let mut s = vec![CaseSetting::new(&MT_CHANNEL_NUM, 53)];
    s.extend(settings_of(
        &data().meshtastic_settings("ANZ", "LongFast").unwrap(),
    ));
    assert!(!receiver_ok(&s));
}

/// The packet stack `aprs` starts on the APRS channel of the country: the plan of its country, else of its ITU region, and
/// the starting dial of `lora/default_dials.csv` only where no plan has a channel. Each start is a supplied entry of the
/// frequency database for the country too.
#[test]
fn aprs_starts_on_the_channel_of_the_country() {
    use sdrkit_presets::{Options, Region};
    let start = |code: &str| {
        let p = data()
            .preset("aprs", &Region::new(code).unwrap(), &Options::default())
            .unwrap();
        (p.plan, p.frequency_hz)
    };
    for (code, plan, hz) in [
        ("AU", "aprs_au", 145.175e6),
        ("US", "aprs_na", 144.39e6),
        ("CA", "aprs_na", 144.39e6),
        ("NZ", "aprs_nz", 144.575e6),
        ("JP", "aprs_jp", 144.64e6),
        ("BR", "aprs_br", 145.57e6),
        ("DE", "aprs_r1", 144.8e6),
        ("GB", "aprs_r1", 144.8e6),
    ] {
        assert_eq!(start(code), (plan.to_owned(), hz), "{code}");
        let known = data()
            .frequency_rows("aprs", &Region::new(code).unwrap())
            .unwrap();
        assert!(
            known.iter().any(|r| r.frequency_hz == hz),
            "{code}: {known:?}"
        );
    }
    // Argentina has no plan: the starting dial.
    assert_eq!(start("AR"), (String::new(), 144.8e6));
}
