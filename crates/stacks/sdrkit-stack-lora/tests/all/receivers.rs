//! Receiver cases that need whole scenes: LoRa raw at every rate, LoRaWAN on every plan, every Meshtastic preset, noise, and
//! the saved state of every LoRa stack.

use crate::common::{catalogue, data, feed, receiver, row, rows, settings_of, slot_band};
use sdrkit_algo_sim::chain::{NoiseKind, GEN_NOISE, GEN_OFFSET_HZ, GEN_SNR_DB};
use sdrkit_core::publish::keys;
use sdrkit_core::verify::CaseSetting;
use sdrkit_core::{Complex32, Event, Receiver, StackDescriptor, StateBlob};
use sdrkit_dev_test::Harness;
use sdrkit_stack_lora::mesh::types::Protocol;
use sdrkit_stack_lora::settings::*;
use sdrkit_stack_lora::typed::{
    LoraPacketEvent, WideFields, MESH_DEVICES, MESH_MESSAGES, MESH_NODES, RAW_FRAMES, WIDE_FRAMES,
};
use std::time::Instant;

/// The test signal of stack `d`: its generator and the standard impairments of the `gen_*` settings in `s`.
fn signal(d: &'static StackDescriptor, rate: f64, secs: f64, s: &[CaseSetting]) -> Vec<Complex32> {
    let mut h = Harness::new(d).rate(rate).seconds(secs).seed(7);
    h = h.with_settings(s);
    h.generate().expect("the test signal")
}

/// The same signal with white noise at `snr_db` in the signal's bandwidth.
fn noisy(
    d: &'static StackDescriptor,
    rate: f64,
    secs: f64,
    snr_db: f64,
    s: &[CaseSetting],
) -> Vec<Complex32> {
    let mut v = vec![
        CaseSetting::new(&GEN_NOISE, NoiseKind::Snr),
        CaseSetting::new(&GEN_SNR_DB, snr_db),
    ];
    v.extend(s.iter().cloned());
    signal(d, rate, secs, &v)
}

fn noise(rate: f64, secs: f64, sigma: f32, seed: u64) -> Vec<Complex32> {
    let mut rng = sdrkit_algo_sim::rng::Rng::new(seed);
    (0..(rate * secs) as usize)
        .map(|_| Complex32::new(rng.normal() as f32, rng.normal() as f32) * sigma)
        .collect()
}

fn receive(
    d: &'static StackDescriptor,
    sub: Option<&str>,
    x: &[Complex32],
    rate: f64,
    centre: f64,
    dial: f64,
    s: &[CaseSetting],
) -> (Vec<Event>, Box<dyn Receiver>) {
    let mut rx = receiver(d, sub, s);
    let ev = feed(rx.as_mut(), x, rate, centre, dial, 65536);
    (ev, rx)
}

fn packets(ev: &[Event]) -> Vec<&Event> {
    crate::common::packets(ev, None)
}

fn find<'a>(ev: &'a [&'a Event], payload: &[u8]) -> Option<&'a Event> {
    ev.iter()
        .copied()
        .find(|e| e.at(&LoraPacketEvent::PAYLOAD) == Some(payload))
}

/// Saves the state of `rx`, restores it into a fresh receiver of the same stack and returns that receiver.
fn saved_and_restored(
    d: &'static StackDescriptor,
    sub: Option<&str>,
    rx: &dyn Receiver,
    rate: f64,
    centre: f64,
    dial: f64,
) -> Box<dyn Receiver> {
    let state = rx.save_state().expect("a saved state");
    let (_, mut rx2) = receive(d, sub, &[], rate, centre, dial, &[]);
    rx2.restore_state(&state).unwrap();
    // A damaged state is refused and changes nothing.
    let half = &state.bytes()[..state.bytes().len() / 2];
    let damaged = StateBlob::new(state.owner().to_owned(), state.version(), half.to_vec());
    assert!(rx2.restore_state(&damaged).is_err());
    assert!(rx2
        .restore_state(&StateBlob::new("other", 1, Vec::new()))
        .is_err());
    let mut out = sdrkit_core::Output::default();
    rx2.feed(&vec![Complex32::default(); (rate * 0.3) as usize], &mut out);
    rx2
}

// ---------------------------------------------------------------------------------------------------------------------------------
// LoRa raw.

#[test]
fn the_fields_of_every_raw_frame() {
    let d = &sdrkit_stack_lora::LORARAW;
    let rate = 2e6;
    let x = signal(d, rate, 5.5, &[]);
    let (ev, _) = receive(d, None, &x, rate, 868.4e6, 868.1e6, &[]);
    let pk = packets(&ev);
    for (sf, bw, cr, crc, _off, _db, text) in sdrkit_stack_lora::gen::raw_frames() {
        let e = find(&pk, &text).unwrap_or_else(|| panic!("{:?}", String::from_utf8_lossy(&text)));
        assert_eq!(e.at(&LoraPacketEvent::SF), Some(i64::from(sf)));
        assert_eq!(e.at(&LoraPacketEvent::BW), Some(bw));
        assert_eq!(e.at(&LoraPacketEvent::CR), Some(i64::from(cr)));
        assert_eq!(e.at(&LoraPacketEvent::CRC), Some(crc));
        assert!(e.get(&keys::SNR_DB).unwrap() > 10.0);
        let lv = e.get(&keys::LEVEL_DBFS).unwrap();
        assert!((-80.0..10.0).contains(&lv), "{lv}");
    }
    let sf12 = find(&pk, b"sdrkit SF12").unwrap();
    assert!(
        (sf12.freq_hz - 868.1e6 + 900.0).abs() < 150.0,
        "{}",
        sf12.freq_hz
    );
}

#[test]
fn raw_frames_weak_and_off_centre() {
    let d = &sdrkit_stack_lora::LORARAW;
    let rate = 2e6;
    let x = noisy(d, rate, 5.5, 0.0, &[]);
    let (ev, _) = receive(d, None, &x, rate, 868.4e6, 868.1e6, &[]);
    let pk = packets(&ev);
    assert!(pk.len() >= 5, "{}", pk.len());
    assert!(find(&pk, b"sdrkit SF12").is_some());
    assert!(find(&pk, b"sdrkit LoRa raw SF7 125k").is_some());
    // Off centre: the offset setting moves the receiver with the signal.
    let off = [CaseSetting::new(&OFFSET, 120e3)];
    let x = signal(d, rate, 5.5, &off);
    let (ev, rx) = receive(d, None, &x, rate, 868.4e6, 868.1e6, &off);
    assert!(packets(&ev).len() >= 8, "{}", packets(&ev).len());
    assert_eq!(rx.results().fields().get(&keys::FRAMES_BAD), Some(0));
}

#[test]
fn a_fixed_raw_setting_sf9_250k() {
    let d = &sdrkit_stack_lora::LORARAW;
    let rate = 2e6;
    // One cycle of the scene.
    let cycle: f64 = sdrkit_stack_lora::gen::raw_frames()
        .iter()
        .map(|f| {
            let mut p = sdrkit_proto_lora_phy::LoraParams::new(f.0, f.1, f.2, 8, 0x12);
            p.crc = f.3;
            p.airtime(f.6.len()) + 0.15
        })
        .sum::<f64>()
        + 0.2;
    let x = signal(d, rate, cycle, &[]);
    let s = [
        CaseSetting::new(&AUTO_SF, false),
        CaseSetting::new(&SF, 9),
        CaseSetting::new(&AUTO_BW, false),
        CaseSetting::new(&BW, LoraBandwidth::Bw250),
    ];
    let (ev, _) = receive(d, None, &x, rate, 868.4e6, 868.1e6, &s);
    let pk = packets(&ev);
    assert_eq!(pk.len(), 1);
    assert_eq!(
        pk[0].at(&LoraPacketEvent::PAYLOAD),
        Some(&b"sdrkit raw SF9 250k"[..])
    );
}

#[test]
fn raw_sync_words_noise_and_saved_state() {
    let d = &sdrkit_stack_lora::LORARAW;
    let rate = 2e6;
    let s34 = [CaseSetting::new(&SYNC, RawSync::Lorawan)];
    let x = signal(d, rate, 5.5, &s34);
    let (ev, _) = receive(d, None, &x, rate, 868.4e6, 868.1e6, &[]);
    assert!(packets(&ev).is_empty(), "{}", packets(&ev).len());
    let any = [CaseSetting::new(&SYNC, RawSync::Any)];
    let (ev, rx) = receive(d, None, &x, rate, 868.4e6, 868.1e6, &any);
    let pk = packets(&ev);
    assert!(pk.len() >= 8, "{}", pk.len());
    assert!(pk
        .iter()
        .all(|e| e.at(&LoraPacketEvent::SYNC) == Some(0x34)));
    // The saved state comes back in a fresh receiver.
    let n = rows(rx.results(), &RAW_FRAMES).len();
    assert!(n >= 8);
    let rx2 = saved_and_restored(d, None, rx.as_ref(), rate, 868.4e6, 868.1e6);
    assert_eq!(rows(rx2.results(), &RAW_FRAMES).len(), n);
    // Noise alone: nothing.
    let n = noise(rate, 3.0, 0.02, 5);
    let (ev, rx) = receive(d, None, &n, rate, 868.4e6, 868.1e6, &[]);
    assert!(packets(&ev).is_empty());
    assert_eq!(rx.results().fields().get(&keys::FRAMES_OK), Some(0));
}

/// A change of frequency keeps the frame log and the counts of frames, and drops the signal to noise ratio, the offset
/// and the level of the present signal.
#[test]
fn a_change_of_frequency_keeps_the_raw_frames_and_drops_the_levels() {
    use sdrkit_core::interface::ReadFields;
    use sdrkit_stack_lora::typed::RawFields;
    let d = &sdrkit_stack_lora::LORARAW;
    let rate = 2e6;
    let x = signal(d, rate, 5.5, &[]);
    let (_, mut rx) = receive(d, None, &x, rate, 868.4e6, 868.1e6, &[]);
    let read = |rx: &dyn Receiver| {
        let f = RawFields::read(rx.results().fields()).expect("the fields of loraraw");
        (f.frames_ok, f.sf7, f.snr_db, f.offset_hz, f.level_dbfs)
    };
    let frames = rows(rx.results(), &RAW_FRAMES).len();
    let (good, sf7, snr, offset, level) = read(rx.as_ref());
    assert!(frames >= 8 && good >= 8 && sf7 > 0);
    assert!(snr.is_some() && offset.is_some() && level.is_some());
    rx.forget();
    assert_eq!(rows(rx.results(), &RAW_FRAMES).len(), frames);
    assert_eq!(read(rx.as_ref()), (good, sf7, None, None, None));
}

#[test]
fn lora_raw_at_every_rate() {
    let d = &sdrkit_stack_lora::LORARAW;
    for rate in [
        1.0e6, 1.024e6, 2.0e6, 2.048e6, 2.4e6, 2.5e6, 3.2e6, 4e6, 5e6, 6e6, 8e6,
    ] {
        // The channel 100 kHz below the centre, so that the 500 kHz channel fits at 1 Msps too.
        let off = [CaseSetting::new(&OFFSET, 200e3)];
        let x = noisy(d, rate, 5.5, 20.0, &off);
        let (ev, rx) = receive(d, None, &x, rate, 868.4e6, 868.1e6, &off);
        let pk = packets(&ev);
        let want = sdrkit_stack_lora::gen::raw_frames();
        let missing: Vec<String> = want
            .iter()
            .filter(|f| {
                sdrkit_proto_lora_phy::channel::in_band(rate, -100e3, f.1)
                    && find(&pk, &f.6).is_none()
            })
            .map(|f| format!("SF{} {} kHz", f.0, f.1 / 1e3))
            .collect();
        assert!(missing.is_empty(), "{rate}: {missing:?}");
        assert_eq!(
            rx.results().fields().get(&keys::FRAMES_BAD),
            Some(0),
            "{rate}"
        );
    }
}

// ---------------------------------------------------------------------------------------------------------------------------------
// LoRaWAN, every channel.

#[test]
fn lorawan_weak_downlinks_off_channel_limit_noise_and_saved_state() {
    let d = &sdrkit_stack_lora::LORAWANWIDE;
    let rate = 4e6;
    let eu = settings_of(&data().lorawan_settings("EU868", 0).unwrap());
    let with = |c: CaseSetting| {
        let mut v = eu.clone();
        v.push(c);
        v
    };
    let x = noisy(
        d,
        rate,
        7.5,
        6.0,
        &with(CaseSetting::new(&GEN_OFFSET_HZ, 2500.0)),
    );
    let (ev, rx) = receive(d, None, &x, rate, 868.4e6, 868.3e6, &eu);
    let pk = packets(&ev);
    assert!(pk.len() >= 7, "{}", pk.len());
    assert!(pk.iter().any(|e| e
        .at(&LoraPacketEvent::PAYLOAD)
        .unwrap()
        .starts_with(&[0x40, 0x2b, 0x1a, 0x01, 0x26])));
    // The saved state comes back in a fresh receiver.
    let devices = rows(rx.results(), &MESH_DEVICES).len();
    assert!(devices > 0);
    let rx2 = saved_and_restored(d, None, rx.as_ref(), rate, 868.4e6, 868.3e6);
    assert_eq!(rows(rx2.results(), &MESH_DEVICES).len(), devices);
    // Downlinks off.
    let x = signal(d, rate, 7.5, &eu);
    let (ev, _) = receive(
        d,
        None,
        &x,
        rate,
        868.4e6,
        868.3e6,
        &with(CaseSetting::new(&DOWNLINKS, false)),
    );
    let pk = packets(&ev);
    assert!(pk.len() >= 5, "{}", pk.len());
    assert!(
        pk.iter()
            .all(|e| e.at(&LoraPacketEvent::IQ_INVERTED) != Some(true)),
        "inverted frame"
    );
    // A channel limit of 3.
    let (_, rx) = receive(
        d,
        None,
        &x,
        rate,
        868.4e6,
        868.3e6,
        &with(CaseSetting::new(&MAX_CHANNELS, 3)),
    );
    assert_eq!(
        rx.results().fields().at(&WideFields::CHANNELS_DECODED),
        Some(3)
    );
    assert!(rx.results().fields().get(&keys::FRAMES_OK).unwrap_or(0) > 0);
    // Noise alone: nothing logged.
    let n = noise(rate, 3.0, 0.02, 9);
    let (ev, rx) = receive(d, None, &n, rate, 868.4e6, 868.3e6, &eu);
    assert!(packets(&ev).is_empty());
    assert!(rows(rx.results(), &WIDE_FRAMES).is_empty());
}

#[test]
fn lorawan_every_plan_with_its_scene() {
    let d = &sdrkit_stack_lora::LORAWANWIDE;
    let rate = 4e6;
    for p in data().lorawan_plans().unwrap() {
        let plan = &p.plan;
        let row = data().lorawan_settings(plan, 0).unwrap();
        let s = settings_of(&row);
        let x = signal(d, rate, 7.5, &s);
        let centre = row.frequency_hz.unwrap() + 100e3;
        let (ev, _) = receive(d, None, &x, rate, centre, centre - 100e3, &s);
        let values = Harness::new(d).with_settings(&s).values().unwrap();
        let cfg = sdrkit_stack_lora::gen::wide_config(&values);
        let sent = sdrkit_stack_lora::gen::wide_frames(&cfg, 0, 0.0);
        let n = sent.0.len() + sent.1.len();
        let pk = packets(&ev);
        let missing: Vec<String> = sent
            .0
            .iter()
            .filter(|f| find(&pk, &f.3).is_none())
            .map(|f| format!("{} MHz SF{} inv {}", f.1 / 1e6, f.2.sf, f.2.invert_iq))
            .collect();
        assert!(
            pk.len() + 1 >= n && missing.len() <= 1,
            "{plan}: {} of {n}, missing {missing:?}",
            pk.len()
        );
    }
}

// ---------------------------------------------------------------------------------------------------------------------------------
// The mesh receiver.

#[test]
fn every_meshtastic_preset_in_its_eu_slot() {
    use sdrkit_proto_lora_phy::modulate::TxFrame;
    use sdrkit_stack_lora::proto::meshtastic as mt;
    let rate = 2e6;
    let centre = 869.825e6;
    let eu = &slot_band("EU_868");
    let d = &sdrkit_stack_lora::MESH;
    for p in mt::PRESETS {
        let mut s = settings_of(&data().meshtastic_settings("EU_868", p.name).unwrap());
        let over = s
            .iter()
            .find(|c| c.key == "mt_override_freq")
            .map_or(0.0, |c| match c.value {
                sdrkit_core::Value::Hz(h) => h,
                _ => 0.0,
            });
        let Some(f) = mt::slot_hz(p, eu, "", 0, 0.0, over) else {
            continue;
        };
        s.push(catalogue(&MT_PRESET, p.name));
        let lp = mt::lora_params(p, eu);
        let text = format!("hello on {}", p.name);
        let data = mt::Data {
            port: 1,
            payload: text.as_bytes().to_vec(),
            bitfield: Some(0),
            ..Default::default()
        };
        let pkt =
            sdrkit_stack_lora::gen::mt_packet(0x1234, mt::BROADCAST, 77, 3, 3, false, 0x34, &data);
        let fr = TxFrame::new(&lp, &pkt)
            .expect("a valid frame")
            .at(0.2)
            .carrier(f - centre)
            .amplitude(0.3);
        let mut x = vec![Complex32::default(); ((fr.end_s() + 0.5) * rate) as usize];
        fr.add_into(&mut x, rate, 0);
        let n = noise(rate, x.len() as f64 / rate, 0.002, 3);
        for (a, b) in x.iter_mut().zip(n) {
            *a += b;
        }
        let secs = x.len() as f64 / rate;
        let t = Instant::now();
        let (ev, rx) = receive(d, Some("meshtastic"), &x, rate, centre, 869.525e6, &s);
        let cpu = t.elapsed().as_secs_f64();
        let mt: Vec<_> = packets(&ev)
            .into_iter()
            .filter(|e| e.at(&LoraPacketEvent::NET) == Some(Protocol::Meshtastic))
            .collect();
        assert_eq!(mt.len(), 1, "{}: {:?}", p.name, rx.results().log_lines());
        assert!(mt
            .iter()
            .all(|e| e.at(&LoraPacketEvent::CRC_OK) == Some(true)));
        assert!(
            row(rx.results(), &MESH_MESSAGES, |r| r.get(&keys::TEXT)
                == Some(text.as_str()))
            .is_some(),
            "{}",
            p.name
        );
        assert!(
            cpu * 3.0 < secs || cfg!(debug_assertions),
            "{}: {cpu} s",
            p.name
        );
    }
}

#[test]
fn noise_only_and_the_scene_faster_than_real_time() {
    let d = &sdrkit_stack_lora::MESH;
    let rate = 2e6;
    let n = noise(rate, 10.0, 0.01, 1);
    let t = Instant::now();
    let (ev, _) = receive(d, None, &n, rate, 869.825e6, 869.525e6, &[]);
    let cpu = t.elapsed().as_secs_f64();
    assert!(packets(&ev).is_empty());
    if !cfg!(debug_assertions) {
        assert!(cpu * 3.0 < 10.0, "noise: {cpu} s for 10 s");
    }
    let x = signal(d, rate, 10.0, &[]);
    let t = Instant::now();
    let (ev, _) = receive(d, None, &x, rate, 869.825e6, 869.525e6, &[]);
    let cpu = t.elapsed().as_secs_f64();
    assert!(!packets(&ev).is_empty());
    if !cfg!(debug_assertions) {
        assert!(cpu * 3.0 < 10.0, "scene: {cpu} s for 10 s");
    }
}

#[test]
fn the_mesh_tables_come_back_from_a_saved_state() {
    let d = &sdrkit_stack_lora::MESH;
    let rate = 2e6;
    let x = signal(d, rate, 8.0, &[]);
    let (_, rx) = receive(d, None, &x, rate, 869.825e6, 869.525e6, &[]);
    let nodes = rows(rx.results(), &MESH_NODES).len();
    assert!(nodes > 0);
    let rx2 = saved_and_restored(d, None, rx.as_ref(), rate, 869.825e6, 869.525e6);
    assert_eq!(rows(rx2.results(), &MESH_NODES).len(), nodes);
}
