//! Generator, standard impairments, receiver, for each stack: every frame sent comes back with its tables, and any cut of the
//! input produces the same frames.

use crate::common::{band_plan, count, data, feed, packets, receiver, settings_of};
use sdrkit_algo_sim::chain::{NoiseKind, GEN_ADC_BITS, GEN_NOISE, GEN_OFFSET_HZ, GEN_SNR_DB};
use sdrkit_core::publish::keys;
use sdrkit_core::verify::CaseSetting;
use sdrkit_core::{Complex32, Event, Receiver, StackDescriptor};
use sdrkit_dev_test::Harness;
use sdrkit_stack_lora::mesh::types::Protocol;
use sdrkit_stack_lora::results::Modulation;
use sdrkit_stack_lora::settings::*;
use sdrkit_stack_lora::typed::{
    LoraPacketEvent, MeshDeviceRow, MeshNodeRow, MeshPacketRow, MeshStationRow, APRS_PACKET,
    APRS_STATIONS, MESH_DEVICES, MESH_MESSAGES, MESH_NODES, MESH_STATIONS, MESH_TYPES,
};

/// The test signal of `d` at `rate` with the `gen_*` and stack settings in `s`, at 20 dB SNR.
fn signal(
    d: &'static StackDescriptor,
    sub: Option<&str>,
    rate: f64,
    secs: f64,
    s: &[CaseSetting],
) -> Vec<Complex32> {
    let mut h = Harness::new(d)
        .rate(rate)
        .seconds(secs)
        .seed(7)
        .set(&GEN_NOISE, NoiseKind::Snr)
        .set(&GEN_SNR_DB, 20.0);
    if let Some(x) = sub {
        h = h.submode(x);
    }
    h = h.with_settings(s);
    h.generate().expect("the test signal")
}

#[allow(clippy::too_many_arguments)]
fn receive(
    d: &'static StackDescriptor,
    sub: Option<&str>,
    x: &[Complex32],
    rate: f64,
    centre: f64,
    dial: f64,
    piece: usize,
    s: &[CaseSetting],
) -> (Vec<Event>, Box<dyn Receiver>) {
    let mut rx = receiver(d, sub, s);
    let ev = feed(rx.as_mut(), x, rate, centre, dial, piece);
    (ev, rx)
}

/// The payload of an event in hex.
fn hex(e: &Event) -> String {
    e.at(&LoraPacketEvent::PAYLOAD)
        .unwrap_or(&[])
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

#[test]
fn the_eu_mesh_scene() {
    let d = &sdrkit_stack_lora::MESH;
    let rate = 2e6;
    let x = signal(
        d,
        None,
        rate,
        40.0,
        &[CaseSetting::new(&GEN_OFFSET_HZ, 400.0)],
    );
    let (ev, rx) = receive(d, None, &x, rate, 869.825e6, 869.525e6, 50_000, &[]);
    let res = rx.results();
    let mt = packets(&ev, Some(Protocol::Meshtastic));
    let mc = packets(&ev, Some(Protocol::MeshCore));
    let bad: Vec<_> = packets(&ev, None)
        .into_iter()
        .filter(|e| e.at(&LoraPacketEvent::CRC_OK) != Some(true))
        .map(|e| {
            (
                e.time,
                e.freq_hz,
                e.at(&LoraPacketEvent::SF),
                e.get(&keys::SNR_DB),
            )
        })
        .collect();
    assert!(bad.is_empty(), "{bad:?}");
    assert!(mt.len() >= 31, "{} Meshtastic frames", mt.len());
    assert!(mc.len() >= 10, "{} MeshCore frames", mc.len());
    let net = |p: Protocol| move |r: &sdrkit_core::Record| r.at(&LoraPacketEvent::NET) == Some(p);
    assert_eq!(count(res, &MESH_NODES, net(Protocol::Meshtastic)), 6);
    assert_eq!(count(res, &MESH_NODES, net(Protocol::MeshCore)), 3);
    assert_eq!(
        count(res, &MESH_NODES, |r| r
            .at(&MeshNodeRow::BATTERY_PCT)
            .is_some_and(|b| b <= 100.0)),
        6
    );
    let messages = count(res, &MESH_MESSAGES, |_| true);
    assert!(messages >= 12, "{messages}");
    let text = |t: &'static str| move |r: &sdrkit_core::Record| r.get(&keys::TEXT) == Some(t);
    assert_eq!(count(res, &MESH_MESSAGES, text("relayed reply")), 1);
    assert_eq!(count(res, &MESH_MESSAGES, text("the weather is fine")), 1);
    let ty = |t: &'static str| move |r: &sdrkit_core::Record| r.at(&MeshPacketRow::TYPE) == Some(t);
    assert_eq!(count(res, &MESH_TYPES, ty("TRACEROUTE_APP")), 1);
    assert_eq!(count(res, &MESH_TYPES, ty("ADVERT")), 1);
    let first = packets(&ev, None)[0];
    assert!((first.time - 0.20).abs() <= 0.01, "{}", first.time);
    assert_eq!(res.status().lock_state, sdrkit_core::LockState::Locked);
    let summary = &res.status().summary;
    assert!(summary.starts_with("Mesh (LoRa): decoding"), "{summary}");
}

#[test]
fn the_70cm_scene() {
    let d = &sdrkit_stack_lora::MESH;
    let rate = 2e6;
    let (dial, plan) = band_plan("mesh", "MeshCom EU");
    let mut scene = plan.clone();
    scene.push(CaseSetting::new(&GEN_SCENE, GenScene::Amateur));
    let x = signal(d, None, rate, 28.0, &scene);
    let (ev, rx) = receive(d, None, &x, rate, dial + 300e3, dial, 65536, &plan);
    assert_eq!(dial, 433.175e6);
    let res = rx.results();
    assert_eq!(packets(&ev, Some(Protocol::LoraAprs)).len(), 7);
    assert_eq!(packets(&ev, Some(Protocol::MeshCom)).len(), 5);
    assert_eq!(
        count(res, &MESH_STATIONS, |r| r
            .at(&MeshStationRow::COMMENT)
            .is_some_and(|c| c.contains("Abu Dhabi, 35°C"))),
        1
    );
    assert_eq!(
        count(res, &MESH_STATIONS, |r| r.at(&MeshNodeRow::BATTERY_PCT)
            == Some(64.0)),
        1
    );
    assert_eq!(
        count(res, &MESH_STATIONS, |r| r.at(&MeshStationRow::PATH)
            == Some("N0CALL-10*,WIDE1*")),
        1
    );
    for t in ["hello from sdrkit", "QSL, 59 here", "group hello"] {
        assert!(
            count(res, &MESH_MESSAGES, |r| r.get(&keys::TEXT) == Some(t)) >= 1,
            "{t}"
        );
    }
}

#[test]
fn the_lorawan_sub_mode_scene() {
    let d = &sdrkit_stack_lora::LORAWAN;
    let rate = 2e6;
    let x = signal(d, Some("lorawan"), rate, 12.0, &[]);
    let (ev, rx) = receive(d, Some("lorawan"), &x, rate, 868.4e6, 868.1e6, 65536, &[]);
    assert_eq!(packets(&ev, Some(Protocol::Lorawan)).len(), 6);
    let res = rx.results();
    assert_eq!(count(res, &MESH_DEVICES, |_| true), 6);
    assert_eq!(
        count(res, &MESH_DEVICES, |r| r.at(&MeshDeviceRow::NETWORK)
            == Some("The Things Network")),
        1
    );
}

#[test]
fn the_raw_scene_and_cut_independence() {
    let d = &sdrkit_stack_lora::LORARAW;
    let rate = 2e6;
    let x = signal(d, None, rate, 5.5, &[]);
    let (ev, rx) = receive(d, None, &x, rate, 868.4e6, 868.1e6, 65536, &[]);
    let res = rx.results();
    assert!(ev.len() >= 8, "{} frames", ev.len());
    assert_eq!(res.fields().get(&keys::FRAMES_BAD), Some(0));
    let sf7 = ev
        .iter()
        .find(|e| hex(e) == "7364726b6974204c6f52612072617720534637203132356b")
        .unwrap();
    assert!((sf7.freq_hz - 868.1e6 - 1500.0).abs() < 150.0);
    use sdrkit_core::interface::ReadFields;
    use sdrkit_stack_lora::typed::RawFields;
    let f = RawFields::read(res.fields()).expect("the fields of loraraw");
    for (n, key) in [
        (f.sf7, "sf7"),
        (f.sf12, "sf12"),
        (f.bw125, "bw125"),
        (f.bw250, "bw250"),
        (f.bw500, "bw500"),
    ] {
        assert!(n > 0, "{key}");
    }
    // Pieces of another size give the same frames.
    let (ev2, _) = receive(d, None, &x, rate, 868.4e6, 868.1e6, 3333, &[]);
    let a: Vec<_> = ev.iter().map(hex).collect();
    let b: Vec<_> = ev2.iter().map(hex).collect();
    assert_eq!(a, b);
}

#[test]
fn the_implicit_scene() {
    let d = &sdrkit_stack_lora::LORARAW;
    let rate = 2e6;
    let x = signal(d, None, rate, 0.9, &[CaseSetting::new(&GEN_IMPLICIT, true)]);
    let s = |len: i64| {
        vec![
            CaseSetting::new(&IMPLICIT, true),
            CaseSetting::new(&AUTO_SF, false),
            CaseSetting::new(&SF, 8),
            CaseSetting::new(&AUTO_BW, false),
            CaseSetting::new(&BW, LoraBandwidth::Bw250),
            CaseSetting::new(&CR, 6),
            CaseSetting::new(&LENGTH, len),
        ]
    };
    let (ev, _) = receive(d, None, &x, rate, 868.4e6, 868.1e6, 65536, &s(12));
    assert_eq!(ev.len(), 3);
    let (ev, _) = receive(d, None, &x, rate, 868.4e6, 868.1e6, 65536, &s(11));
    assert_eq!(ev.len(), 0);
}

#[test]
fn the_wide_scene() {
    let d = &sdrkit_stack_lora::LORAWANWIDE;
    let rate = 4e6;
    let eu868 = settings_of(&data().lorawan_settings("EU868", 0).unwrap());
    let x = signal(d, None, rate, 7.5, &eu868);
    let (ev, rx) = receive(d, None, &x, rate, 868.4e6, 868.3e6, 65536, &eu868);
    let res = rx.results();
    let lw = packets(&ev, Some(Protocol::Lorawan));
    let values = Harness::new(d).with_settings(&eu868).values().unwrap();
    let sent =
        sdrkit_stack_lora::gen::wide_frames(&sdrkit_stack_lora::gen::wide_config(&values), 0, 0.0);
    let hexes: Vec<String> = sent
        .0
        .iter()
        .map(|f| f.3.iter().map(|b| format!("{b:02x}")).collect())
        .collect();
    let missing: Vec<&String> = hexes
        .iter()
        .filter(|h| !lw.iter().any(|e| hex(e) == **h))
        .collect();
    assert!(missing.is_empty(), "{missing:?} {:?}", res.log_lines());
    assert_eq!(
        lw.len(),
        sent.0.len() + sent.1.len(),
        "{:?}",
        res.log_lines()
    );
    assert_eq!(res.fields().get(&keys::FRAMES_BAD), Some(0));
    assert!(lw
        .iter()
        .any(|e| e.at(&LoraPacketEvent::MODULATION) == Some(Modulation::Fsk)));
    let up = lw
        .iter()
        .find(|e| hex(e).starts_with("4034120b26806400"))
        .unwrap();
    assert!(
        (up.freq_hz - 868.1e6 - 1300.0).abs() < 150.0,
        "{}",
        up.freq_hz
    );
    assert!(up.get(&keys::SNR_DB).unwrap() > 15.0);
}

#[test]
fn the_aprs_scene_with_impairments() {
    let d = &sdrkit_stack_lora::APRS;
    let rate = 240e3;
    let x = signal(
        d,
        None,
        rate,
        12.0,
        &[
            CaseSetting::new(&GEN_OFFSET_HZ, 1000.0),
            CaseSetting::new(&GEN_ADC_BITS, 8),
        ],
    );
    let (ev, rx) = receive(d, None, &x, rate, 144.85e6, 144.8e6, 24_000, &[]);
    let heard: Vec<&Event> = ev.iter().filter(|e| e.is(&APRS_PACKET)).collect();
    let sent = sdrkit_stack_lora::gen::aprs_frames(0);
    assert_eq!(
        heard.len(),
        sent.len(),
        "{:?}",
        heard.iter().map(|e| e.get(&keys::TEXT)).collect::<Vec<_>>()
    );
    let res = rx.results();
    assert_eq!(res.fields().get(&keys::FRAMES_OK), Some(sent.len() as i64));
    assert!(count(res, &APRS_STATIONS, |_| true) >= 5);
    // Noise alone: nothing.
    let mut rng = sdrkit_algo_sim::rng::Rng::new(3);
    let n: Vec<Complex32> = (0..(rate * 3.0) as usize)
        .map(|_| Complex32::new(rng.normal() as f32, rng.normal() as f32) * 0.01)
        .collect();
    let (ev, rx) = receive(d, None, &n, rate, 144.85e6, 144.8e6, 24_000, &[]);
    assert!(ev.iter().all(|e| !e.is(&APRS_PACKET)));
    assert_eq!(
        rx.results().status().lock_state,
        sdrkit_core::LockState::Searching
    );
}

#[test]
fn listening_returns_8_khz_audio() {
    let d = &sdrkit_stack_lora::APRS;
    let rate = 240e3;
    let x = signal(
        d,
        None,
        rate,
        6.0,
        &[CaseSetting::new(&GEN_SPEEDS, Speeds::Only1200)],
    );
    let mut rx = receiver(d, None, &[CaseSetting::new(&LISTEN, true)]);
    rx.configure(&sdrkit_core::InputInfo::new(rate, 144.85e6, 144.8e6).recording())
        .unwrap();
    let mut out = sdrkit_core::Output::default();
    for c in x.chunks(24_000) {
        rx.feed(c, &mut out);
    }
    assert_eq!(out.audio_format().map(|f| f.1), Some(1));
    let frames = out.audio().len();
    assert!((frames as i64 - 48_000).abs() <= 800, "{frames}");
}

#[test]
fn the_lora_aprs_test_signal_sits_on_the_default_frequency() {
    let d = &sdrkit_stack_lora::LORAAPRS;
    let t = &d.tuning;
    let rate = 2e6;
    let x = signal(d, None, rate, 26.0, &[]);
    let centre = t.default_hz + t.tune_offset_hz;
    let (ev, _) = receive(d, None, &x, rate, centre, t.default_hz, 65536, &[]);
    assert_eq!(packets(&ev, Some(Protocol::LoraAprs)).len(), 7);
}

#[test]
fn the_mesh_verification_cases_last_until_their_protocol_frames_end() {
    use sdrkit_stack_lora::gen::{mesh_config, mesh_cycle, MeshScene};
    let o = sdrkit_core::verify::VerifyOptions {
        seconds: 3.0,
        clean: false,
        snr_db: 30.0,
    };
    let (_, eu) = band_plan("mesh", "EU 868");
    let values = Harness::new(&sdrkit_stack_lora::MESH)
        .with_settings(&eu)
        .values()
        .unwrap();
    let scene = MeshScene::of(GenScene::Texting, &mesh_config("mesh", &values), 2e6);
    let cycle = mesh_cycle(&scene, 0, 0.0, 2e6);
    let last = |hz: f64| {
        cycle
            .iter()
            .filter(|f| f.1 == hz)
            .map(|f| f.0 + f.2.airtime(f.3.len()))
            .fold(0.0, f64::max)
    };
    // MeshCore on 869.618 MHz, Meshtastic LongFast on 869.525 MHz.
    for (id, hz) in [
        ("mesh", 869.618e6),
        ("meshcore", 869.618e6),
        ("meshtastic", 869.525e6),
    ] {
        let hooks = sdrkit_stack_lora::descriptors()
            .into_iter()
            .find(|d| d.id == id)
            .and_then(|d| d.verify);
        let cases = (hooks.expect("the hooks").cases)(&o);
        assert!(
            cases[0].seconds > last(hz),
            "{id}: {} s, the last frame ends at {} s",
            cases[0].seconds,
            last(hz)
        );
    }
}

#[test]
fn the_eu_cycle_has_enough_frames_and_none_overlap_on_a_frequency() {
    use sdrkit_stack_lora::gen::{mesh_config, mesh_cycle, MeshScene};
    let (_, eu) = band_plan("mesh", "EU 868");
    let values = Harness::new(&sdrkit_stack_lora::MESH)
        .with_settings(&eu)
        .values()
        .unwrap();
    let s = MeshScene::of(GenScene::Texting, &mesh_config("mesh", &values), 2e6);
    let v = mesh_cycle(&s, 0, 0.0, 2e6);
    let mt = v.iter().filter(|f| f.1 == 869.525e6).count();
    let mcn = v.iter().filter(|f| f.1 == 869.618e6).count();
    assert!(mt >= 31, "{mt}");
    assert!(mcn >= 10, "{mcn}");
    for f in [869.525e6, 869.618e6] {
        let mut on: Vec<(f64, f64)> = v
            .iter()
            .filter(|x| x.1 == f)
            .map(|x| (x.0, x.0 + x.2.airtime(x.3.len())))
            .collect();
        on.sort_by(|a, b| a.0.total_cmp(&b.0));
        for w in on.windows(2) {
            assert!(w[1].0 >= w[0].1, "overlap at {f}");
        }
        assert!(on.last().unwrap().1 < 85.0);
    }
}

/// The Meshtastic test signal gives a texting host what it shows: the messages with the name of the sender, the SNR, the
/// level, the hops and the packet id; the nodes with their position and battery; the readings with a position. The
/// expected values are those that the generator sent.
#[test]
fn the_meshtastic_scene_gives_messages_nodes_and_readings() {
    use sdrkit_stack_lora::gen::MT_NODES;
    use sdrkit_stack_lora::typed::{MeshMessageRow, MeshReadingRow, MESH_READINGS};
    let d = &sdrkit_stack_lora::MESHTASTIC;
    let rate = 2e6;
    let (dial, plan) = band_plan("meshtastic", "EU 868");
    let x = signal(d, None, rate, 40.0, &plan);
    let (_, rx) = receive(d, None, &x, rate, dial + 300e3, dial, 65536, &plan);
    let res = rx.results();
    let node = |num: u32| format!("!{num:08x}");
    // Every node of the scene, with its name, position and battery.
    for (k, &(num, long, short, ..)) in MT_NODES.iter().enumerate() {
        let id = node(num);
        let n = crate::common::row(res, &MESH_NODES, |r| r.at(&MeshNodeRow::ID) == Some(&id))
            .unwrap_or_else(|| panic!("node {id}"));
        assert_eq!(n.at(&MeshNodeRow::NAME), Some(long));
        assert_eq!(n.at(&MeshNodeRow::SHORT), Some(short));
        assert_eq!(n.at(&MeshNodeRow::BATTERY_PCT), Some(60.0 + 5.0 * k as f64));
        let (lat, lon) = n.at(&MeshNodeRow::POSITION).expect("a position");
        assert!((lat - (52.3702 + 0.01 * k as f64)).abs() < 1e-4, "{lat}");
        assert!((lon - (4.8952 + 0.01 * k as f64)).abs() < 1e-4, "{lon}");
    }
    assert!(
        count(res, &MESH_READINGS, |r| r
            .at(&MeshReadingRow::POSITION)
            .is_some())
            >= 2
    );
    // The texts: the first text of the first cycle has the packet id after the four packets of each of the six nodes.
    let text = |t: &'static str| {
        crate::common::row(res, &MESH_MESSAGES, move |r| r.get(&keys::TEXT) == Some(t))
            .unwrap_or_else(|| panic!("the text {t}"))
    };
    let first = text("hello from sdrkit");
    assert_eq!(first.at(&MeshMessageRow::SENDER), Some(MT_NODES[0].1));
    assert_eq!(first.get(&keys::SOURCE), Some(node(MT_NODES[0].0).as_str()));
    assert_eq!(first.at(&MeshMessageRow::HOPS), Some(0));
    assert_eq!(
        first.at(&MeshMessageRow::PACKET_ID),
        Some(0x1000 + 4 * 6 + 1)
    );
    let snr = first.at(&MeshMessageRow::SNR_DB).expect("an SNR");
    assert!(snr > 5.0, "{snr}");
    let level = first.at(&MeshMessageRow::LEVEL_DBFS).expect("a level");
    assert!(level < 0.0, "{level}");
    // A text that one node relayed has taken one hop.
    let relayed = text("relayed reply");
    assert_eq!(relayed.at(&MeshMessageRow::SENDER), Some(MT_NODES[2].1));
    assert_eq!(relayed.at(&MeshMessageRow::HOPS), Some(1));
    // Without a clock the time of a text is the signal time only.
    assert_eq!(first.at(&MeshMessageRow::UTC), None);
}

/// With `gen_damaged_copy` the Meshtastic test signal sends a text first with a flipped bit and a failed checksum, and later
/// as a good copy: the receiver shows the damaged text with `crc_ok` false, then the good copy corrects the row.
#[test]
fn the_damaged_copy_shows_first_and_the_good_copy_corrects_it() {
    use sdrkit_stack_lora::gen::{mesh_config, mesh_cycle, MeshScene};
    use sdrkit_stack_lora::mesh::store::HOLD_S;
    let d = &sdrkit_stack_lora::MESHTASTIC;
    let rate = 2e6;
    let (dial, plan) = band_plan("meshtastic", "EU 868");
    let mut s = plan.clone();
    s.push(CaseSetting::new(&GEN_DAMAGED_COPY, true));
    let values = Harness::new(d).with_settings(&s).values().unwrap();
    let mut scene = MeshScene::of(GenScene::Texting, &mesh_config("meshtastic", &values), rate);
    scene.damaged_copy = true;
    let cycle = mesh_cycle(&scene, 0, 0.0, rate);
    let damaged = cycle
        .iter()
        .find(|f| f.5.is_some())
        .expect("the damaged copy");
    let good = cycle
        .iter()
        .find(|f| f.5.is_none() && f.3 == damaged.3)
        .expect("the good copy");
    let mid = damaged.0 + damaged.2.airtime(damaged.3.len()) + HOLD_S + 1.0;
    assert!(mid < good.0, "{mid} {}", good.0);
    let end = good.0 + good.2.airtime(good.3.len()) + 1.0;
    // The test signal at the defaults of the generator, as a front end opens it, and at 20 dB SNR.
    let plain = Harness::new(d)
        .rate(rate)
        .seconds(end)
        .with_settings(&s)
        .generate()
        .expect("the test signal");
    for x in [plain, signal(d, None, rate, end, &s)] {
        damaged_then_corrected(d, &x, rate, dial, &plan, mid);
    }
}

/// Feeds `x` to a receiver of `d` up to `mid` s and checks the damaged text, then to its end and checks the correction.
fn damaged_then_corrected(
    d: &'static StackDescriptor,
    x: &[Complex32],
    rate: f64,
    dial: f64,
    plan: &[CaseSetting],
    mid: f64,
) {
    use sdrkit_stack_lora::gen::DAMAGED_COPY_TEXT;
    use sdrkit_stack_lora::typed::MeshMessageRow;
    let mut rx = receiver(d, None, plan);
    rx.configure(&sdrkit_core::InputInfo::new(rate, dial + 300e3, dial).recording())
        .unwrap();
    let mut out = sdrkit_core::Output::default();
    let cut = (mid * rate) as usize;
    let mut bad: Vec<u8> = DAMAGED_COPY_TEXT.as_bytes().to_vec();
    bad[0] ^= 1;
    let bad = String::from_utf8(bad).unwrap();
    for c in x[..cut].chunks(65536) {
        rx.feed(c, &mut out);
    }
    let res = rx.results();
    let shown = crate::common::row(res, &MESH_MESSAGES, |r| r.get(&keys::TEXT) == Some(&bad))
        .expect("the damaged text");
    assert_eq!(shown.at(&MeshMessageRow::CRC_OK), Some(false));
    for c in x[cut..].chunks(65536) {
        rx.feed(c, &mut out);
    }
    rx.finish(&mut out);
    let res = rx.results();
    assert_eq!(
        count(res, &MESH_MESSAGES, |r| r.get(&keys::TEXT) == Some(&bad)),
        0
    );
    let fixed = crate::common::row(res, &MESH_MESSAGES, |r| {
        r.get(&keys::TEXT) == Some(DAMAGED_COPY_TEXT)
    })
    .expect("the corrected text");
    assert_eq!(fixed.at(&MeshMessageRow::CRC_OK), Some(true));
    assert_eq!(fixed.at(&MeshMessageRow::COPIES), Some(2));
}
