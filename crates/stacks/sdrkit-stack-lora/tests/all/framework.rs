//! The family on the framework: every setting of every schema is accepted, the band plan requests the host to tune only when
//! the user changes it, the minimum rate, the declared events of each generator, and saved state.

use crate::common::{band_plan, data, feed, gen_run, gen_run_at, invoke, rows, settings_of};
use sdrkit_core::command::CommandError;
use sdrkit_core::publish::keys;
use sdrkit_core::{
    owner, Complex32, HostRequest, InputInfo, Output, Owner, Receiver, SettingError, StackConfig,
    StackDescriptor, Value,
};
use sdrkit_dev_test::Harness;
use sdrkit_stack_lora::receivers;
use sdrkit_stack_lora::typed::{
    MeshChannelRow, MeshMtFields, MESH_CHANNELS, MESH_NODE, MESH_NODES, MESH_TEXT,
};

fn defaults(d: &'static StackDescriptor) -> StackConfig {
    StackConfig::new(Harness::new(d).values().unwrap())
}

/// The refusal of value `v` for setting `key` of `rx`.
fn refusal(rx: &mut dyn Receiver, key: &str, v: Value) -> SettingError {
    let id = rx.schema().id(key).unwrap();
    owner::apply(rx, id, &v).expect_err("the value is refused")
}

#[test]
fn every_receiver_builds_with_its_defaults() {
    use sdrkit_stack_lora as l;
    for d in [
        &l::MESH,
        &l::MESHTASTIC,
        &l::MESHCORE,
        &l::LORAWAN,
        &l::LORAAPRS,
        &l::MESHCOM,
    ] {
        receivers::mesh_receiver(d.id, &defaults(d)).unwrap();
    }
    receivers::wide_receiver(&defaults(&l::LORAWANWIDE)).unwrap();
    receivers::raw_receiver(&defaults(&l::LORARAW)).unwrap();
    receivers::aprs_receiver(&defaults(&l::APRS)).unwrap();
}

#[test]
fn a_bad_value_is_refused_not_replaced() {
    let mut g =
        receivers::mesh_receiver("meshtastic", &defaults(&sdrkit_stack_lora::MESHTASTIC)).unwrap();
    assert!(matches!(
        refusal(&mut g, "mt_hop_limit", Value::I64(9)),
        SettingError::OutOfRange { .. }
    ));
    assert!(matches!(
        refusal(&mut g, "custom_sync", Value::Text("zz".into())),
        SettingError::WrongKind { .. }
    ));
    assert!(matches!(
        invoke(&mut g, "mt.send", &[]),
        Err(CommandError::MissingArgument { .. })
    ));
    assert!(matches!(
        invoke(
            &mut g,
            "mt.send",
            &[
                ("to", Value::Text("nobody".into())),
                ("text", Value::Text("x".into()))
            ]
        ),
        Err(CommandError::BadArgument { .. })
    ));
    // A channel list that does not parse is refused.
    let mut g = receivers::mesh_receiver("mesh", &defaults(&sdrkit_stack_lora::MESH)).unwrap();
    assert!(matches!(
        refusal(&mut g, "channels", Value::Text("x=lorawan/1/1/7/5".into())),
        SettingError::Malformed { .. }
    ));
    let mut a = receivers::aprs_receiver(&defaults(&sdrkit_stack_lora::APRS)).unwrap();
    assert!(matches!(
        refusal(&mut a, "aprs_symbol", Value::Text("/".into())),
        SettingError::Malformed { .. }
    ));
    // An AX.25 address has at most six characters before the SSID.
    assert!(matches!(
        refusal(&mut a, "aprs_tocall", Value::Text("APRS123".into())),
        SettingError::Malformed { .. }
    ));
    assert!(matches!(
        invoke(&mut a, "aprs.send", &[("to", Value::Text("N0CALL".into()))]),
        Err(CommandError::MissingArgument { .. })
    ));
}

#[test]
fn the_band_plan_asks_for_a_retune_only_when_the_user_changes_it() {
    for d in [
        &sdrkit_stack_lora::LORAAPRS,
        &sdrkit_stack_lora::LORAWAN,
        &sdrkit_stack_lora::MESHCOM,
    ] {
        let run = Harness::new(d).seconds(0.5).run().unwrap();
        assert!(run.requests.is_empty(), "{}: {:?}", d.id, run.requests);
    }
    let mut g = receivers::mesh_receiver("mesh", &defaults(&sdrkit_stack_lora::MESH)).unwrap();
    let rate = 1e6;
    g.configure(&InputInfo::new(rate, 869.825e6, 869.525e6).recording())
        .unwrap();
    let mut out = Output::default();
    g.feed(&vec![Complex32::default(); 100_000], &mut out);
    assert!(out.requests.is_empty());
    // The values of the band plan US 915.
    for c in band_plan("mesh", "US 915").1 {
        let id = g.schema().id(c.key).unwrap();
        owner::apply(&mut g, id, &c.value).unwrap();
    }
    out.clear();
    g.feed(&vec![Complex32::default(); 100_000], &mut out);
    assert_eq!(out.requests.len(), 1);
    match &out.requests[0] {
        HostRequest::Retune { tuned_hz, .. } => assert_eq!(*tuned_hz, 906.875e6),
        r => panic!("{r:?}"),
    }
}

#[test]
fn below_the_minimum_rate_the_status_says_why() {
    let mut g = receivers::raw_receiver(&defaults(&sdrkit_stack_lora::LORARAW)).unwrap();
    assert!(g.configure(&InputInfo::new(100e3, 868e6, 868e6)).is_err());
    assert_eq!(
        g.results().status().summary,
        "LoRa (raw) needs a sample rate of at least 250 ksps"
    );
    // APRS from a channel bank takes any rate.
    let h =
        Harness::new(&sdrkit_stack_lora::APRS).set(&sdrkit_stack_lora::settings::WIDE_INPUT, true);
    let mut a = receivers::aprs_receiver(&StackConfig::new(h.values().unwrap())).unwrap();
    assert!(a
        .configure(&InputInfo::new(192e3, 144.8e6, 144.8e6))
        .is_ok());
}

/// The mesh scene produces `mesh.node` and `mesh.text` events, and only declared kinds.
#[test]
fn the_mesh_scene_emits_declared_mesh_node_and_mesh_text_events() {
    let d = sdrkit_stack_lora::typed::shape("mesh").results_decl();
    let m = &sdrkit_stack_lora::MESH;
    let (ev, _) = gen_run(&Harness::new(m).seconds(30.0), m, 65536);
    let nodes = ev.iter().filter(|e| e.is(&MESH_NODE)).count();
    let texts = ev.iter().filter(|e| e.is(&MESH_TEXT)).count();
    assert!(
        nodes > 0 && texts > 0,
        "{nodes} node and {texts} text events"
    );
    for e in &ev {
        assert!(
            d.events().iter().any(|x| x.kind == e.kind()),
            "{}",
            e.kind()
        );
    }
}

#[test]
fn every_generator_emits_only_declared_events() {
    use sdrkit_stack_lora as l;
    for (d, decl, secs) in [
        (
            &l::LORAWANWIDE,
            l::typed::shape("lorawanwide").results_decl(),
            8.0,
        ),
        (&l::LORARAW, l::typed::shape("loraraw").results_decl(), 8.0),
        (&l::APRS, l::typed::shape("aprs").results_decl(), 12.0),
    ] {
        // LoRaWAN (all channels) has no frequency of its own: the run takes the values of the plan EU868.
        let (ev, _) = if d.id == "lorawanwide" {
            let row = data().lorawan_settings("EU868", 0).unwrap();
            let h = Harness::new(d)
                .seconds(secs)
                .with_settings(&settings_of(&row));
            gen_run_at(&h, d, row.frequency_hz.unwrap(), 65536)
        } else {
            gen_run(&Harness::new(d).seconds(secs), d, 65536)
        };
        assert!(!ev.is_empty(), "{}", d.id);
        for e in &ev {
            assert!(
                decl.events().iter().any(|x| x.kind == e.kind()),
                "{}: {}",
                d.id,
                e.kind()
            );
            assert!(e.freq_hz > 0.0, "{}: {} sent at 0 Hz", d.id, e.kind());
        }
    }
}

#[test]
fn saved_state_comes_back_with_its_ages() {
    let rec = sdrkit_dev_test::read_recording("lora/meshtastic_longfast_600k")
        .expect("the recording lora/meshtastic_longfast_600k");
    let mut rx: Box<dyn Receiver> = Box::new(
        receivers::mesh_receiver("meshtastic", &defaults(&sdrkit_stack_lora::MESHTASTIC)).unwrap(),
    );
    feed(
        rx.as_mut(),
        &rec.samples,
        rec.rate_hz,
        rec.centre_hz,
        869.525e6,
        65536,
    );
    let nodes = rows(rx.results(), &MESH_NODES).len();
    assert!(nodes >= 3);
    let state = rx.save_state().expect("a saved state");
    let mut back: Box<dyn Receiver> = Box::new(
        receivers::mesh_receiver("meshtastic", &defaults(&sdrkit_stack_lora::MESHTASTIC)).unwrap(),
    );
    back.configure(&InputInfo::new(rec.rate_hz, rec.centre_hz, 869.525e6).recording())
        .unwrap();
    back.restore_state(&state).unwrap();
    let mut out = Output::default();
    back.feed(
        &vec![Complex32::default(); (rec.rate_hz * 0.5) as usize],
        &mut out,
    );
    assert_eq!(rows(back.results(), &MESH_NODES).len(), nodes);
    // A state of another stack is refused.
    let mut raw: Box<dyn Receiver> =
        Box::new(receivers::raw_receiver(&defaults(&sdrkit_stack_lora::LORARAW)).unwrap());
    assert!(raw.restore_state(&state).is_err());
    // Clear forgets what was heard.
    invoke(back.as_mut(), "clear", &[]).unwrap();
    back.feed(
        &vec![Complex32::default(); (rec.rate_hz * 0.5) as usize],
        &mut out,
    );
    assert_eq!(rows(back.results(), &MESH_NODES).len(), 0);
}

/// Each stack publishes fields, and `StackGraph` counts a sample that is not finite in the declared `bad_samples`.
#[test]
fn every_stack_publishes_its_results() {
    for d in sdrkit_stack_lora::descriptors() {
        let (_, rx) = gen_run(&Harness::new(d).seconds(4.0), d, 65536);
        let res = rx.results();
        assert!(
            res.fields().iter().any(|(_, v)| *v != Value::None),
            "{}",
            d.id
        );
    }
    let mut g = receivers::raw_receiver(&defaults(&sdrkit_stack_lora::LORARAW)).unwrap();
    g.configure(&InputInfo::new(1e6, 868.4e6, 868.1e6).recording())
        .unwrap();
    let mut x = vec![Complex32::default(); 300_000];
    x[10] = Complex32::new(f32::NAN, 0.0);
    g.feed(&x, &mut Output::default());
    g.refresh_results();
    assert!(g.results().fields().get(&keys::BAD_SAMPLES).is_some());
}

/// The own key and the channel keys live in the saved state, never in a table: a fresh receiver that restores the state has
/// the same public key, and the `channels` table shows only that a key is set.
#[test]
fn keys_live_in_the_saved_state() {
    let d = &sdrkit_stack_lora::MESHTASTIC;
    let mut rx: Box<dyn Receiver> =
        Box::new(receivers::mesh_receiver("meshtastic", &defaults(d)).unwrap());
    let key = "AQIDBAUGBwgJCgsMDQ4PEBESExQVFhcYGRobHB0eHyA=";
    invoke(
        rx.as_mut(),
        "mt.identity",
        &[("key", Value::Text(key.into()))],
    )
    .unwrap();
    invoke(
        rx.as_mut(),
        "mt.channel.add",
        &[
            ("name", Value::Text("Private".into())),
            ("key", Value::Text("1PG7OiApB1nwvP+rz05pAQ==".into())),
        ],
    )
    .unwrap();
    rx.configure(&InputInfo::new(2e6, 869.825e6, 869.525e6).recording())
        .unwrap();
    let mut out = Output::default();
    rx.feed(&vec![Complex32::default(); 2_000_000], &mut out);
    let public = rx
        .results()
        .fields()
        .at(&MeshMtFields::MT_PUBLIC_KEY)
        .map(str::to_owned);
    assert!(public.as_deref().is_some_and(|k| !k.is_empty()));
    let channel = |rx: &dyn Receiver| {
        rows(rx.results(), &MESH_CHANNELS)
            .into_iter()
            .find(|r| r.get(&keys::CHANNEL) == Some("Private"))
            .map(|r| r.at(&MeshChannelRow::KEY_SET))
    };
    assert_eq!(channel(rx.as_ref()), Some(Some(true)));
    let state = rx.save_state().expect("a saved state");
    let mut back: Box<dyn Receiver> =
        Box::new(receivers::mesh_receiver("meshtastic", &defaults(d)).unwrap());
    back.configure(&InputInfo::new(2e6, 869.825e6, 869.525e6).recording())
        .unwrap();
    back.restore_state(&state).unwrap();
    back.feed(&vec![Complex32::default(); 2_000_000], &mut out);
    assert_eq!(
        back.results()
            .fields()
            .at(&MeshMtFields::MT_PUBLIC_KEY)
            .map(str::to_owned),
        public
    );
    assert_eq!(channel(back.as_ref()), Some(Some(true)));
    // A channel the user removes leaves the table.
    invoke(
        back.as_mut(),
        "channel.remove",
        &[("row", Value::Text("meshtastic/Private".into()))],
    )
    .unwrap();
    assert_eq!(channel(back.as_ref()), None);
    assert!(matches!(
        invoke(
            back.as_mut(),
            "channel.remove",
            &[("row", Value::Text("meshtastic/Private".into()))],
        ),
        Err(CommandError::NoSuchRow { .. })
    ));
}

/// The mesh stack takes each of its sub-modes: the sub-mode chooses the band plan, and the settings, the results and
/// the commands stay those of the description of `mesh`.
#[test]
fn the_mesh_stack_is_built_with_every_sub_mode() {
    let d = &sdrkit_stack_lora::MESH;
    let want: Vec<_> = sdrkit_stack_lora::typed::shape("mesh")
        .commands()
        .defs()
        .iter()
        .map(|c| c.key.to_string())
        .collect();
    for s in sdrkit_stack_lora::MESH_SUBMODES {
        let cfg = defaults(d).with_submode(s.id);
        let rx = receivers::mesh_receiver("mesh", &cfg)
            .unwrap_or_else(|e| panic!("mesh as {}: {e}", s.id));
        let have: Vec<_> = sdrkit_core::Owner::commands(&rx)
            .defs()
            .iter()
            .map(|c| c.key.to_string())
            .collect();
        assert_eq!(have, want, "{}", s.id);
    }
}

/// A change of frequency keeps the nodes, the messages and the counts of the tables, and drops the signal to noise
/// ratio, the offset and the level of the present signal.
#[test]
fn a_change_of_frequency_keeps_the_mesh_nodes_and_drops_the_levels() {
    use sdrkit_core::interface::ReadFields;
    use sdrkit_stack_lora::typed::{MeshFields, MESH_MESSAGES};
    let m = &sdrkit_stack_lora::MESH;
    let (_, mut rx) = gen_run(&Harness::new(m).seconds(30.0), m, 65536);
    let read = |rx: &dyn Receiver| {
        let f = MeshFields::read(rx.results().fields()).expect("the fields of mesh");
        (f.mt_nodes, f.messages, f.snr_db, f.offset_hz, f.level_dbfs)
    };
    let counts = |rx: &dyn Receiver| {
        let r = rx.results();
        (rows(r, &MESH_NODES).len(), rows(r, &MESH_MESSAGES).len())
    };
    let before = counts(rx.as_ref());
    let (nodes, messages, snr, offset, level) = read(rx.as_ref());
    assert!(nodes > 0 && messages > 0 && before.0 > 0 && before.1 > 0);
    assert!(snr.is_some() && offset.is_some() && level.is_some());
    rx.forget();
    assert_eq!(counts(rx.as_ref()), before);
    assert_eq!(read(rx.as_ref()), (nodes, messages, None, None, None));
}

/// A change of frequency keeps the LoRaWAN frame log and the devices, and drops the level of the present signal.
#[test]
fn a_change_of_frequency_keeps_the_lorawan_devices_and_drops_the_level() {
    use sdrkit_core::interface::ReadFields;
    use sdrkit_stack_lora::typed::{WideFields, MESH_DEVICES, WIDE_FRAMES};
    let d = &sdrkit_stack_lora::LORAWANWIDE;
    let (_, mut rx) = gen_run(&Harness::new(d).seconds(8.0), d, 65536);
    let counts = |rx: &dyn Receiver| {
        let r = rx.results();
        (rows(r, &WIDE_FRAMES).len(), rows(r, &MESH_DEVICES).len())
    };
    let read = |rx: &dyn Receiver| {
        let f = WideFields::read(rx.results().fields()).expect("the fields of lorawanwide");
        (f.frames_ok, f.devices, f.level_dbfs)
    };
    let before = counts(rx.as_ref());
    let (good, devices, level) = read(rx.as_ref());
    assert!(good > 0 && devices > 0 && before.0 > 0 && before.1 > 0);
    assert!(level.is_some());
    rx.forget();
    assert_eq!(counts(rx.as_ref()), before);
    assert_eq!(read(rx.as_ref()), (good, devices, None));
}

/// A stack without a frequency of its own locks on its test signal at its default settings and its default tuning, 0 Hz:
/// every channel lies at the dial.
#[test]
fn the_stacks_without_a_frequency_lock_at_their_defaults() {
    use sdrkit_stack_lora as l;
    use sdrkit_stack_lora::mesh::types::Protocol;
    for (d, protocol, secs) in [
        (&l::MESH, Some(Protocol::Meshtastic), 3.0),
        (&l::MESHTASTIC, Some(Protocol::Meshtastic), 3.0),
        (&l::MESHCORE, Some(Protocol::MeshCore), 40.0),
        (&l::LORAWAN, Some(Protocol::Lorawan), 3.0),
        (&l::LORAAPRS, Some(Protocol::LoraAprs), 6.0),
        (&l::MESHCOM, Some(Protocol::MeshCom), 26.0),
        (&l::LORAWANWIDE, None, 3.0),
    ] {
        assert_eq!(d.tuning.default_hz, 0.0, "{}", d.id);
        let (ev, rx) = gen_run(&Harness::new(d).seconds(secs), d, 65536);
        assert!(
            !crate::common::packets(&ev, protocol).is_empty(),
            "{}: {:?}",
            d.id,
            rx.results().log_lines()
        );
    }
}

/// Every setting of every stack has a section of the setup, and a stack of one protocol lists no setting of another:
/// `loraaprs` none of Meshtastic or MeshCore, `meshtastic` none of MeshCore or APRS.
#[test]
fn every_setting_has_a_section_and_a_protocol_stack_only_its_own() {
    for d in sdrkit_stack_lora::descriptors() {
        let schema = (d.settings)();
        for s in schema.settings() {
            assert!(
                !s.section.is_empty(),
                "{}: {} has no section",
                d.id,
                s.field.key
            );
        }
    }
    let sections = |d: &StackDescriptor| -> Vec<String> {
        (d.settings)()
            .settings()
            .iter()
            .map(|s| s.section.to_string())
            .collect()
    };
    let aprs = sections(&sdrkit_stack_lora::LORAAPRS);
    assert!(
        aprs.iter().all(|s| s != "Meshtastic" && s != "MeshCore"),
        "{aprs:?}"
    );
    let mt = sections(&sdrkit_stack_lora::MESHTASTIC);
    assert!(mt.iter().all(|s| s != "MeshCore" && s != "APRS"), "{mt:?}");
    let mesh = sections(&sdrkit_stack_lora::MESH);
    for s in ["Meshtastic", "MeshCore", "APRS", "LoRaWAN", "Channels"] {
        assert!(mesh.iter().any(|x| x == s), "mesh lacks the section {s}");
    }
}
