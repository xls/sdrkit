//! The tables of the mesh receiver: caps, the de-duplication of messages, the copies of a packet with and without a good
//! CRC, and the saved state with its ages.

use sdrkit_proto_lora_phy::rx::LoraFrame;
use sdrkit_proto_lora_phy::LoraParams;
use sdrkit_stack_lora::gen::mt_packet;
use sdrkit_stack_lora::mesh::decode::{Decoded, MeshDecoder};
use sdrkit_stack_lora::mesh::plan::PlanEntry;
use sdrkit_stack_lora::mesh::store::{MeshStore, HOLD_S, MESSAGES_CAP, NODES_CAP, PACKETS_CAP};
use sdrkit_stack_lora::mesh::types::{Delivery, Failure, MeshPacket, MeshText, Protocol};
use sdrkit_stack_lora::proto::meshtastic::{self as mt, Data, User};
use sdrkit_util_objects::Tables;

#[test]
fn caps_hold_and_the_least_recent_node_leaves() {
    let mut s = MeshStore::default();
    for i in 0..250 {
        s.add_packet(MeshPacket {
            time: i as f64,
            ..Default::default()
        });
        s.heard(
            Protocol::Meshtastic,
            &format!("!{i:08x}"),
            i as f64,
            5.0,
            -40.0,
            0,
        );
    }
    assert_eq!(s.packets.len(), PACKETS_CAP);
    assert_eq!(s.nodes.len(), NODES_CAP);
    assert!(s.node(Protocol::Meshtastic, "!00000000").is_none());
    assert!(s.node(Protocol::Meshtastic, "!000000f9").is_some());
}

#[test]
fn a_message_heard_again_is_kept_once() {
    let mut s = MeshStore::default();
    let m = |t: f64| MeshText {
        time: t,
        ..Default::default()
    };
    assert!(s.add_message(m(1.0), "a".into()));
    assert!(!s.add_message(m(2.0), "a".into()));
    assert!(s.add_message(m(3.0), "b".into()));
    assert_eq!(s.messages.len(), 2);
    for i in 0..(MESSAGES_CAP + 10) {
        s.add_message(m(4.0 + i as f64), format!("k{i}"));
    }
    assert_eq!(s.messages.len(), MESSAGES_CAP);
}

/// An own message is added while it is new and updated after; a late state of a message whose row has left the log adds
/// no row, so a late acknowledgement delivers a message only while its row is known.
#[test]
fn a_late_state_updates_a_known_own_row_and_adds_none() {
    let mut s = MeshStore::default();
    let own = |key: u64, state: Delivery| MeshText {
        protocol: Protocol::LoraAprs,
        own: true,
        key,
        state: Some(state),
        text: format!("own {key}"),
        ..Default::default()
    };
    s.upsert_own(own(1, Delivery::Queued));
    s.upsert_own(own(1, Delivery::Failed(Failure::NoAck)));
    s.upsert_own(own(1, Delivery::Delivered));
    let states: Vec<_> = s.messages.iter().map(|m| m.state).collect();
    assert_eq!(states, [Some(Delivery::Delivered)]);
    for i in 0..MESSAGES_CAP {
        s.add_message(MeshText::default(), format!("k{i}"));
    }
    assert!(s.messages.iter().all(|m| !m.own));
    s.upsert_own(own(1, Delivery::Delivered));
    assert!(s.messages.iter().all(|m| !m.own));
    // A new message that fails at once has its row.
    s.upsert_own(own(2, Delivery::Failed(Failure::TransmitOff)));
    assert_eq!(s.messages.iter().filter(|m| m.own).count(), 1);
}

#[test]
fn the_saved_state_keeps_ages_and_refuses_another_format() {
    let mut s = MeshStore::default();
    s.heard(Protocol::Meshtastic, "!a1b2c3d4", 100.0, 7.5, -30.0, 1);
    s.heard(Protocol::Meshtastic, "!a1b2c3d4", 130.0, 9.5, -30.0, 1);
    let state = s.save_state(160.0).unwrap();
    let mut t = MeshStore::default();
    t.restore_state(&state, 10.0).unwrap();
    let seen = t.node_seen(Protocol::Meshtastic, "!a1b2c3d4").unwrap();
    assert_eq!(seen.count, 2);
    assert!((seen.age(10.0) - 30.0).abs() < 1e-9);
    let n = t.node(Protocol::Meshtastic, "!a1b2c3d4").unwrap();
    assert_eq!(n.snr.max, Some(9.5));
    let other = sdrkit_util_objects::Format::new("lora.other", 1)
        .save(0.0, &1)
        .unwrap();
    assert!(t.restore_state(&other, 0.0).is_err());
    assert_eq!(t.nodes.len(), 1);
}

const NODE: u32 = 0x1234_abcd;

/// The frame of packet `id` of [`NODE`] with `data`, heard at `time`. With `damaged` one byte of the body is wrong and the
/// CRC has failed.
fn copy(dec: &mut MeshDecoder, id: u32, data: &Data, time: f64, damaged: bool) -> Decoded {
    let mut payload = mt_packet(NODE, mt::BROADCAST, id, 3, 3, false, 0x34, data);
    if damaged {
        // The fifth byte of the content of the Data message, after the header of 16 bytes and four bytes of field tags.
        payload[24] ^= 0x01;
    }
    let frame = LoraFrame {
        payload,
        sf: 11,
        bw: 250e3,
        cr: 5,
        crc_present: true,
        crc_ok: !damaged,
        sync: 0x2b,
        snr_db: 5.0,
        channel_hz: 869.525e6,
        ..Default::default()
    };
    let entry = PlanEntry {
        mask: 1,
        name: "test".into(),
        freq_hz: 869.525e6,
        params: LoraParams::new(11, 250e3, 5, 16, 0x2b),
        in_band: true,
        detector: 0,
    };
    dec.frame(&frame, &entry, time)
}

fn text(t: &str) -> Data {
    Data {
        port: 1,
        payload: t.as_bytes().to_vec(),
        bitfield: Some(0),
        ..Default::default()
    }
}

fn shown(dec: &MeshDecoder) -> Vec<String> {
    dec.store.messages.iter().map(|m| m.text.clone()).collect()
}

#[test]
fn a_good_copy_is_shown_at_once_and_later_copies_are_dropped() {
    let mut dec = MeshDecoder::default();
    let d = copy(&mut dec, 7, &text("hello"), 1.0, false);
    assert_eq!(d.texts.len(), 1);
    assert_eq!(shown(&dec), ["hello"]);
    assert!(copy(&mut dec, 7, &text("hello"), 1.5, false)
        .texts
        .is_empty());
    assert!(copy(&mut dec, 7, &text("hello"), 2.0, true)
        .texts
        .is_empty());
    assert!(dec.store.release_held(100.0, true).is_empty());
    assert_eq!(shown(&dec), ["hello"]);
}

#[test]
fn a_damaged_copy_waits_and_is_shown_when_no_good_copy_comes() {
    let mut dec = MeshDecoder::default();
    let d = copy(&mut dec, 7, &text("hello"), 1.0, true);
    assert!(d.texts.is_empty());
    assert!(!d.packet.crc_ok);
    assert!(shown(&dec).is_empty());
    assert!(dec.store.release_held(1.0 + HOLD_S - 0.1, false).is_empty());
    let late = dec.store.release_held(1.0 + HOLD_S, false);
    assert_eq!(late.len(), 1);
    assert_eq!(shown(&dec), ["helln"]);
    // A second damaged copy is not shown again.
    copy(&mut dec, 7, &text("hello"), 6.0, true);
    assert!(dec.store.release_held(100.0, true).is_empty());
    assert_eq!(shown(&dec).len(), 1);
}

#[test]
fn a_good_copy_within_the_wait_is_the_one_shown() {
    let mut dec = MeshDecoder::default();
    copy(&mut dec, 7, &text("hello"), 1.0, true);
    let d = copy(&mut dec, 7, &text("hello"), 2.0, false);
    assert_eq!(d.texts.len(), 1);
    assert!(dec.store.release_held(100.0, true).is_empty());
    assert_eq!(shown(&dec), ["hello"]);
}

#[test]
fn a_good_copy_corrects_the_row_of_a_damaged_copy_that_was_shown() {
    let mut dec = MeshDecoder::default();
    copy(&mut dec, 7, &text("hello"), 1.0, true);
    copy(&mut dec, 8, &text("other"), 2.0, false);
    assert_eq!(dec.store.release_held(10.0, false).len(), 1);
    assert_eq!(shown(&dec), ["other", "helln"]);
    let d = copy(&mut dec, 7, &text("hello"), 20.0, false);
    assert_eq!(d.texts.len(), 1);
    assert_eq!(shown(&dec), ["other", "hello"]);
    // The corrected message is now a known one.
    assert!(copy(&mut dec, 7, &text("hello"), 21.0, false)
        .texts
        .is_empty());
    assert_eq!(shown(&dec).len(), 2);
}

#[test]
fn the_end_of_the_signal_shows_what_still_waits() {
    let mut dec = MeshDecoder::default();
    copy(&mut dec, 7, &text("hello"), 1.0, true);
    assert_eq!(dec.store.release_held(1.5, true).len(), 1);
    assert_eq!(shown(&dec), ["helln"]);
}

#[test]
fn a_damaged_copy_changes_no_node() {
    let mut dec = MeshDecoder::default();
    let user = User {
        id: mt::node_name(NODE),
        long_name: "Base camp".into(),
        short_name: "BC".into(),
        ..Default::default()
    };
    let info = Data {
        port: 4,
        payload: user.encode(),
        bitfield: Some(0),
        ..Default::default()
    };
    let id = mt::node_name(NODE);
    // A damaged copy makes no node, from a node information packet or from a text.
    let d = copy(&mut dec, 1, &info, 1.0, true);
    assert!(d.nodes.is_empty());
    copy(&mut dec, 2, &text("hello"), 2.0, true);
    assert_eq!(dec.store.nodes.len(), 0);
    // A good copy names the node. A damaged copy after it changes neither the name nor the count.
    copy(&mut dec, 1, &info, 3.0, false);
    let name = dec
        .store
        .node(Protocol::Meshtastic, &id)
        .map(|n| n.long_name.clone());
    assert_eq!(name.as_deref(), Some("Base camp"));
    let count = dec
        .store
        .node_seen(Protocol::Meshtastic, &id)
        .unwrap()
        .count;
    let d = copy(&mut dec, 3, &info, 4.0, true);
    assert!(d.nodes.is_empty());
    assert_eq!(
        dec.store.node(Protocol::Meshtastic, &id).unwrap().long_name,
        "Base camp"
    );
    assert_eq!(
        dec.store
            .node_seen(Protocol::Meshtastic, &id)
            .unwrap()
            .count,
        count
    );
    // The client takes acknowledgements only from packets with a good CRC.
    assert!(!d.mt.unwrap().crc_ok);
}

/// The CRC state and the copies of the shown message with this text: (damaged, copies).
fn state(dec: &MeshDecoder, t: &str) -> (bool, u32) {
    let m = dec
        .store
        .messages
        .iter()
        .find(|m| m.text == t)
        .expect("a shown message");
    (m.damaged, m.copies)
}

#[test]
fn copies_are_counted_and_a_good_copy_marks_the_row_good() {
    // Good, then good and damaged repeats: one good row with three copies.
    let mut dec = MeshDecoder::default();
    copy(&mut dec, 7, &text("hello"), 1.0, false);
    copy(&mut dec, 7, &text("hello"), 1.5, false);
    copy(&mut dec, 7, &text("hello"), 2.0, true);
    dec.store.release_held(100.0, true);
    assert_eq!(state(&dec, "hello"), (false, 3));

    // Damaged, damaged, then good within the wait: the good text with three copies.
    let mut dec = MeshDecoder::default();
    copy(&mut dec, 7, &text("hello"), 1.0, true);
    copy(&mut dec, 7, &text("hello"), 1.5, true);
    copy(&mut dec, 7, &text("hello"), 2.0, false);
    dec.store.release_held(100.0, true);
    assert_eq!(shown(&dec), ["hello"]);
    assert_eq!(state(&dec, "hello"), (false, 3));

    // Damaged only: shown as damaged after the wait; a later damaged copy is counted, a good copy replaces the text.
    let mut dec = MeshDecoder::default();
    copy(&mut dec, 7, &text("hello"), 1.0, true);
    assert_eq!(dec.store.release_held(1.0 + HOLD_S, false).len(), 1);
    assert_eq!(state(&dec, "helln"), (true, 1));
    copy(&mut dec, 7, &text("hello"), 6.0, true);
    assert_eq!(state(&dec, "helln"), (true, 2));
    let d = copy(&mut dec, 7, &text("hello"), 9.0, false);
    assert_eq!(d.texts.len(), 1);
    assert!(!d.texts[0].damaged);
    assert_eq!(shown(&dec), ["hello"]);
    assert_eq!(state(&dec, "hello"), (false, 3));
    // A repeat after the correction is a copy of the good row.
    copy(&mut dec, 7, &text("hello"), 10.0, true);
    assert_eq!(state(&dec, "hello"), (false, 4));
}

#[test]
fn a_good_repeat_corrects_a_text_shown_at_once_from_a_damaged_frame() {
    // A protocol without the wait shows a damaged text at once; a good repeat of the same key corrects the row.
    let mut s = MeshStore::default();
    let m = |t: f64, damaged: bool| MeshText {
        time: t,
        protocol: Protocol::MeshCom,
        text: "hello".into(),
        damaged,
        ..Default::default()
    };
    assert!(s.add_message(m(1.0, true), "k".into()));
    assert!(!s.add_message(m(2.0, true), "k".into()));
    assert!(s.add_message(m(3.0, false), "k".into()));
    assert!(!s.add_message(m(4.0, true), "k".into()));
    assert_eq!(s.messages.len(), 1);
    let x = s.messages.iter().next().unwrap();
    assert_eq!((x.damaged, x.copies, x.time), (false, 4, 3.0));
}

/// Decodes `payload` as a frame of the protocols `mask` with the sync word `sync`, heard at `time`; `damaged` marks its CRC
/// as failed and leaves the payload as it is.
fn heard(
    dec: &mut MeshDecoder,
    payload: Vec<u8>,
    mask: u32,
    sync: u8,
    time: f64,
    damaged: bool,
) -> Decoded {
    let frame = LoraFrame {
        payload,
        sf: 8,
        bw: 62.5e3,
        cr: 8,
        crc_present: true,
        crc_ok: !damaged,
        sync,
        snr_db: 5.0,
        channel_hz: 433.775e6,
        ..Default::default()
    };
    let entry = PlanEntry {
        mask,
        name: "test".into(),
        freq_hz: 433.775e6,
        params: LoraParams::new(8, 62.5e3, 8, 16, sync),
        in_band: true,
        detector: 0,
    };
    dec.frame(&frame, &entry, time)
}

#[test]
fn a_damaged_frame_changes_no_advert_and_no_station() {
    use sdrkit_stack_lora::proto::hamlora;
    use sdrkit_stack_lora::proto::meshcore::{self as mc, Identity, McPacket};
    let mut dec = MeshDecoder::default();
    // A MeshCore advert: a damaged copy makes no node, no reading and no contact for the client.
    let advert = McPacket {
        route: 1,
        ptype: 4,
        version: 0,
        hash_size: 1,
        path: Vec::new(),
        payload: mc::build_advert(
            &Identity::from_seed(&[7; 32]),
            1000,
            1,
            Some((52.0, 4.0)),
            "Sara",
        ),
    }
    .encode();
    let d = heard(&mut dec, advert.clone(), 2, 0x12, 1.0, true);
    assert!(d.nodes.is_empty() && d.readings.is_empty());
    let rx = d.mc.expect("a MeshCore packet");
    assert!(!rx.crc_ok && rx.advert.is_none());
    assert_eq!(dec.store.nodes.len(), 0);
    let d = heard(&mut dec, advert, 2, 0x12, 2.0, false);
    assert_eq!(d.nodes.len(), 1);
    assert!(d.mc.unwrap().advert.is_some());
    // LoRa APRS: a damaged position makes no station; a damaged message still shows, marked.
    let pos = hamlora::build_lora_aprs("N0CALL-7>APLRT1:!5222.21N/00453.71E>tracker");
    heard(&mut dec, pos.clone(), 4, 0x12, 3.0, true);
    assert_eq!(dec.store.stations.len(), 0);
    let msg = hamlora::build_lora_aprs("N0CALL-7>APLRT1::N0CALL-9 :hello there{12");
    let d = heard(&mut dec, msg, 4, 0x12, 4.0, true);
    assert_eq!(dec.store.stations.len(), 0);
    assert_eq!(d.texts.len(), 1);
    assert!(d.texts[0].damaged);
    heard(&mut dec, pos, 4, 0x12, 5.0, false);
    assert_eq!(dec.store.stations.len(), 1);
    // MeshCom: a damaged node frame makes no station.
    let node = hamlora::build_meshcom(
        b'!',
        0x11223344,
        4,
        0x10,
        "N0MC-2",
        "*",
        "5222.21N/00453.71E#node",
    );
    heard(&mut dec, node.clone(), 8, 0x2B, 6.0, true);
    assert_eq!(dec.store.stations.len(), 1);
    heard(&mut dec, node, 8, 0x2B, 7.0, false);
    assert_eq!(dec.store.stations.len(), 2);
}
