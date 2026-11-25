//! The texting clients against simulated nodes, and the protocol cases that need the decoder.

#![allow(clippy::field_reassign_with_default)]

use sdrkit_algo_sim::rng::Rng;
use sdrkit_proto_lora_phy::rx::LoraFrame;
use sdrkit_proto_lora_phy::LoraParams;
use sdrkit_stack_lora::mesh::decode::{Decoded, MeshDecoder};
use sdrkit_stack_lora::mesh::mc_client::{McClient, McConfig};
use sdrkit_stack_lora::mesh::mt_client::{MtClient, MtConfig, Role, TextOptions, REPEAT_CW_MAX};
use sdrkit_stack_lora::mesh::plan::PlanEntry;
use sdrkit_stack_lora::mesh::types::{Delivery, Failure, MeshText, TxChannel, TxKind, TxRequest};
use sdrkit_stack_lora::proto::meshcore::{self as mc, Identity};
use sdrkit_stack_lora::proto::meshtastic::{self as mt, Data, Header};

fn entry(mask: u32, sync: u8) -> PlanEntry {
    PlanEntry {
        mask,
        name: "test".into(),
        freq_hz: 869.525e6,
        params: LoraParams::new(11, 250e3, 5, 16, sync),
        in_band: true,
        detector: 0,
    }
}

/// The channel of [`entry`], as a client receives it from the plan.
fn channel(sync: u8) -> TxChannel {
    let e = entry(1, sync);
    TxChannel {
        params: e.params,
        freq_hz: e.freq_hz,
    }
}

fn frame(payload: &[u8], sync: u8) -> LoraFrame {
    LoraFrame {
        payload: payload.to_vec(),
        sf: 11,
        bw: 250e3,
        cr: 5,
        crc_present: true,
        crc_ok: true,
        sync,
        snr_db: 5.0,
        channel_hz: 869.525e6,
        ..Default::default()
    }
}

struct Node {
    client: MtClient,
    dec: MeshDecoder,
}

fn node(num: u32, role: Role, seed: u64) -> Node {
    let key = mt::new_private_key();
    let mut c = MtConfig::new(num, key);
    c.tx_enabled = true;
    c.tx_channel = Some(channel(0x2B));
    c.role = role;
    let mut dec = MeshDecoder::default();
    dec.mt_own = Some((num, key));
    Node {
        client: MtClient::new(c, seed),
        dec,
    }
}

/// Node `n` hears a payload at `t`: decode, then the client.
fn hear(
    n: &mut Node,
    t: f64,
    payload: &[u8],
) -> (Decoded, sdrkit_stack_lora::mesh::mt_client::ClientOut) {
    let d = n.dec.frame(&frame(payload, 0x2B), &entry(1, 0x2B), t);
    let o = n.client.on_packet(t, d.mt.as_ref().unwrap());
    (d, o)
}

fn state_of(texts: &[MeshText]) -> Option<Delivery> {
    texts.last().and_then(|m| m.state)
}

#[test]
fn broadcast_acked_by_the_mesh_and_rebroadcast_rules() {
    let mut a = node(0x0a0a0a01, Role::ClientMute, 1);
    let mut b = node(0x0b0b0b02, Role::Client, 2);
    let o = a
        .client
        .send_text(0.0, mt::BROADCAST, 0, "hello mesh", TextOptions::default());
    // The first text of a session is preceded by the own NodeInfo.
    assert_eq!(o.tx.len(), 2);
    assert_eq!(o.tx[0].what, TxKind::NodeInfo);
    assert_eq!(state_of(&o.texts), Some(Delivery::Queued));
    let text = o.tx[1].clone();
    let key = text.message_key;
    let s = a.client.on_sent(0.5, key, 0.4);
    assert_eq!(state_of(&s.texts), Some(Delivery::Sent));
    // B (role CLIENT) rebroadcasts with hop limit - 1 and its own relay byte.
    let (_, ob) = hear(&mut b, 1.0, &text.payload);
    let rb = ob
        .tx
        .iter()
        .find(|r| r.what == TxKind::Rebroadcast)
        .expect("a rebroadcast");
    let h = Header::parse(&rb.payload).unwrap();
    assert_eq!(h.hop_limit, 2);
    assert_eq!(h.relay, 0x02);
    // A hears it: acknowledged by the mesh.
    let (_, oa) = hear(&mut a, 2.0, &rb.payload);
    assert_eq!(state_of(&oa.texts), Some(Delivery::AckedByMesh));
    // A CLIENT_MUTE never rebroadcasts.
    let mut m = node(0x0c0c0c03, Role::ClientMute, 3);
    let (_, om) = hear(&mut m, 1.0, &text.payload);
    assert!(om.tx.iter().all(|r| r.what != TxKind::Rebroadcast));
    // A CLIENT cancels its rebroadcast when another node's is heard first.
    let mut c = node(0x0d0d0d04, Role::Client, 4);
    let (_, oc) = hear(&mut c, 1.0, &text.payload);
    assert!(oc.tx.iter().any(|r| r.what == TxKind::Rebroadcast));
    let (_, oc2) = hear(&mut c, 1.1, &rb.payload);
    assert_eq!(oc2.cancel.len(), 1);
}

/// A node that heard a text well repeats it after up to `2 * 8 + 2^8` back-off slots. The sender waits that long before
/// it sends the text again, and a repeat that comes after the sender gave up still marks the text acknowledged.
#[test]
fn a_channel_text_waits_for_the_longest_repeat_and_takes_a_late_one() {
    let mut a = node(0x0a0a0a01, Role::ClientMute, 1);
    let mut b = node(0x0b0b0b02, Role::Client, 2);
    let o = a
        .client
        .send_text(0.0, mt::BROADCAST, 0, "hello", TextOptions::default());
    let text = o.tx.last().unwrap().clone();
    let air = 0.6;
    a.client.on_sent(0.0, text.message_key, air);
    let (_, ob) = hear(&mut b, air, &text.payload);
    let repeat = ob
        .tx
        .iter()
        .find(|r| r.what == TxKind::Rebroadcast)
        .expect("a repeat")
        .clone();
    let slot = 2.5 * channel(0x2B).params.symbol_s() + 0.2e-3;
    let longest = air + f64::from(2 * REPEAT_CW_MAX + (1 << REPEAT_CW_MAX)) * slot;
    let mut t = 0.0;
    while t < longest {
        t += 0.05;
        let o = a.client.tick(t);
        assert!(
            o.tx.iter().all(|r| r.what != TxKind::Retransmission),
            "sent again after {t} s, before the longest repeat at {longest} s"
        );
    }
    let (_, oa) = hear(&mut a, longest + air, &repeat.payload);
    assert_eq!(state_of(&oa.texts), Some(Delivery::AckedByMesh));

    // No repeat until the sender gave up, then one.
    let o = a
        .client
        .send_text(100.0, mt::BROADCAST, 0, "again", TextOptions::default());
    let text = o.tx.last().unwrap().clone();
    a.client.on_sent(100.0, text.message_key, air);
    let (_, ob) = hear(&mut b, 100.0 + air, &text.payload);
    let repeat = ob
        .tx
        .iter()
        .find(|r| r.what == TxKind::Rebroadcast)
        .expect("a repeat")
        .clone();
    let (mut t, mut last) = (100.0, None);
    while t < 200.0 {
        t += 0.25;
        let o = a.client.tick(t);
        for r in o.tx.iter().filter(|r| r.what == TxKind::Retransmission) {
            a.client.on_sent(t, r.message_key, air);
        }
        last = state_of(&o.texts).or(last);
    }
    assert_eq!(
        last,
        Some(Delivery::Failed(Failure::Routing(
            mt::ROUTING_MAX_RETRANSMIT
        )))
    );
    assert_eq!(a.client.in_flight(), 0);
    let (_, oa) = hear(&mut a, 201.0, &repeat.payload);
    assert_eq!(state_of(&oa.texts), Some(Delivery::AckedByMesh));
}

#[test]
fn direct_messages_delivered_retried_failed() {
    let mut a = node(0x0a0a0a01, Role::ClientMute, 1);
    let mut b = node(0x0b0b0b02, Role::ClientMute, 2);
    // Exchange NodeInfo so both know the public keys (PKI direct messages).
    let oa = a
        .client
        .send_text(0.0, mt::BROADCAST, 0, "hi", TextOptions::default());
    hear(&mut b, 0.5, &oa.tx[0].payload);
    let ob = b
        .client
        .send_text(0.0, mt::BROADCAST, 0, "hi", TextOptions::default());
    hear(&mut a, 0.6, &ob.tx[0].payload);
    // A direct text to B: PKI (channel byte 0), want-ack.
    let o = a
        .client
        .send_text(1.0, 0x0b0b0b02, 0, "direct hello", TextOptions::default());
    let dm = o.tx.last().unwrap().clone();
    let h = Header::parse(&dm.payload).unwrap();
    assert_eq!(h.channel, 0);
    assert!(h.want_ack);
    a.client.on_sent(1.1, dm.message_key, 0.4);
    let (d, ob) = hear(&mut b, 1.5, &dm.payload);
    assert_eq!(d.texts[0].text, "direct hello");
    assert_eq!(d.texts[0].channel, "DM");
    let ack = ob
        .tx
        .iter()
        .find(|r| r.what == TxKind::Ack)
        .expect("an ack");
    // The ack names the packet.
    let (_, oa) = hear(&mut a, 2.0, &ack.payload);
    assert_eq!(state_of(&oa.texts), Some(Delivery::Delivered));
    // A duplicate is acknowledged again but shown once.
    let (d2, ob2) = hear(&mut b, 3.0, &dm.payload);
    assert!(d2.texts.is_empty());
    assert!(ob2.tx.iter().any(|r| r.what == TxKind::Ack));
    // No answer: 3 retransmissions with the same id, then failed: MAX_RETRANSMIT.
    let o = a
        .client
        .send_text(10.0, 0x0b0b0b02, 0, "anyone?", TextOptions::default());
    let first = o.tx.last().unwrap().clone();
    let id = Header::parse(&first.payload).unwrap().id;
    a.client.on_sent(10.0, first.message_key, 0.4);
    let mut retries = 0;
    let mut last = None;
    let mut t = 10.0;
    while t < 200.0 {
        t += 0.25;
        let o = a.client.tick(t);
        for r in &o.tx {
            if r.what == TxKind::Retransmission {
                retries += 1;
                assert_eq!(Header::parse(&r.payload).unwrap().id, id);
                a.client.on_sent(t, r.message_key, 0.4);
            }
        }
        if let Some(s) = state_of(&o.texts) {
            last = Some(s);
        }
    }
    assert_eq!(retries, 3);
    assert_eq!(
        last,
        Some(Delivery::Failed(Failure::Routing(
            mt::ROUTING_MAX_RETRANSMIT
        )))
    );
    // A routing error names the failure.
    let o = a
        .client
        .send_text(300.0, 0x0b0b0b02, 0, "bad channel", TextOptions::default());
    let p = o.tx.last().unwrap().clone();
    let pid = Header::parse(&p.payload).unwrap().id;
    a.client.on_sent(300.0, p.message_key, 0.4);
    let mut err = Data {
        port: 5,
        payload: mt::Routing::encode_error(6),
        request_id: pid,
        ..Default::default()
    };
    err.bitfield = Some(0);
    let h = Header {
        to: 0x0a0a0a01,
        from: 0x0b0b0b02,
        id: 77,
        hop_limit: 3,
        hop_start: 3,
        channel: mt::channel_hash("LongFast", &mt::DEFAULT_KEY),
        ..Default::default()
    };
    let pkt = mt::build_channel_packet(&h, &mt::DEFAULT_KEY, &err.encode());
    let (_, oa) = hear(&mut a, 301.0, &pkt);
    assert_eq!(
        state_of(&oa.texts),
        Some(Delivery::Failed(Failure::Routing(
            mt::routing_error_number("NO_CHANNEL").unwrap()
        )))
    );
}

#[test]
fn pki_between_two_identities_and_a_third() {
    let ka = mt::new_private_key();
    let kb = mt::new_private_key();
    let kc = mt::new_private_key();
    let plain = Data {
        port: 1,
        payload: b"secret talk".to_vec(),
        bitfield: Some(0),
        ..Default::default()
    }
    .encode();
    let shared_ab = mt::pki_shared_key(&ka, &mt::public_key(&kb)).unwrap();
    let body = mt::pki_encrypt(&shared_ab, 1234, 0xA, 99, &plain).unwrap();
    let shared_ba = mt::pki_shared_key(&kb, &mt::public_key(&ka)).unwrap();
    assert_eq!(
        mt::pki_decrypt(&shared_ba, 1234, 0xA, &body).unwrap(),
        plain
    );
    let shared_ca = mt::pki_shared_key(&kc, &mt::public_key(&ka)).unwrap();
    assert!(mt::pki_decrypt(&shared_ca, 1234, 0xA, &body).is_none());
    let mut bad = body.clone();
    bad[3] ^= 1;
    assert!(mt::pki_decrypt(&shared_ba, 1234, 0xA, &bad).is_none());
    // Through the decoder: "PKI decryption failed" for the changed byte.
    let mut dec = MeshDecoder::default();
    dec.mt_own = Some((0xB, kb));
    let user = mt::User {
        id: "!0000000a".into(),
        long_name: "A".into(),
        public_key: mt::public_key(&ka).to_vec(),
        ..Default::default()
    };
    let h = Header {
        to: mt::BROADCAST,
        from: 0xA,
        id: 1,
        hop_limit: 3,
        hop_start: 3,
        channel: mt::channel_hash("LongFast", &mt::DEFAULT_KEY),
        ..Default::default()
    };
    let ni = mt::build_channel_packet(
        &h,
        &mt::DEFAULT_KEY,
        &Data {
            port: 4,
            payload: user.encode(),
            ..Default::default()
        }
        .encode(),
    );
    dec.frame(&frame(&ni, 0x2B), &entry(1, 0x2B), 0.0);
    let h = Header {
        to: 0xB,
        from: 0xA,
        id: 1234,
        hop_limit: 3,
        hop_start: 3,
        channel: 0,
        ..Default::default()
    };
    let mut pkt = h.bytes().to_vec();
    pkt.extend_from_slice(&bad);
    let d = dec.frame(&frame(&pkt, 0x2B), &entry(1, 0x2B), 1.0);
    assert_eq!(d.packet.note, "PKI decryption failed");
    let mut pkt = h.bytes().to_vec();
    pkt.extend_from_slice(&body);
    let d = dec.frame(&frame(&pkt, 0x2B), &entry(1, 0x2B), 2.0);
    assert_eq!(d.texts[0].text, "secret talk");
}

#[test]
fn keys_and_channels() {
    let mut dec = MeshDecoder::default();
    assert!(dec.add_mt_channel("x", "not base64 !!").is_err());
    assert!(dec
        .add_mt_channel("x", &mt::base64_encode(&[7u8; 40]))
        .is_err());
    dec.add_mt_channel("Sixteen", &mt::base64_encode(&[9u8; 16]))
        .unwrap();
    dec.add_mt_channel("Two", "Ag==").unwrap();
    dec.add_mt_channel("Open", "AA==").unwrap();
    let names = dec.channel_names();
    assert_eq!(names[0], "LongFast");
    assert!(dec.mt_channels.len() >= 14);
    // A text on the default key under another channel name: decrypted, the note says the hash differs.
    let data = Data {
        port: 1,
        payload: b"hi".to_vec(),
        bitfield: Some(0),
        ..Default::default()
    };
    let h = Header {
        to: mt::BROADCAST,
        from: 5,
        id: 9,
        hop_limit: 3,
        hop_start: 3,
        channel: mt::channel_hash("Elsewhere", &mt::DEFAULT_KEY),
        ..Default::default()
    };
    let p = mt::build_channel_packet(&h, &mt::DEFAULT_KEY, &data.encode());
    let d = dec.frame(&frame(&p, 0x2B), &entry(1, 0x2B), 0.0);
    assert!(
        d.packet.note.contains("header hash differs"),
        "{}",
        d.packet.note
    );
    // A known hash with a wrong key.
    let h2 = Header {
        channel: mt::channel_hash("Sixteen", &[9u8; 16]),
        id: 10,
        ..h
    };
    let p = mt::build_channel_packet(&h2, &[3u8; 16], &data.encode());
    let d = dec.frame(&frame(&p, 0x2B), &entry(1, 0x2B), 0.0);
    assert_eq!(d.packet.note, "wrong key or damaged payload");
    // Clearing user channels makes their packets unreadable again.
    let h3 = Header {
        channel: mt::channel_hash("Sixteen", &[9u8; 16]),
        id: 11,
        ..h
    };
    let p = mt::build_channel_packet(&h3, &[9u8; 16], &data.encode());
    assert!(
        dec.frame(&frame(&p, 0x2B), &entry(1, 0x2B), 0.0)
            .packet
            .decrypted
    );
    dec.clear_user_channels();
    assert!(
        !dec.frame(&frame(&p, 0x2B), &entry(1, 0x2B), 0.0)
            .packet
            .decrypted
    );
}

#[test]
fn header_rules_and_random_packets() {
    let mut dec = MeshDecoder::default();
    for n in 0..16 {
        let d = dec.frame(&frame(&vec![1u8; n], 0x2B), &entry(1, 0x2B), 0.0);
        assert!(!d.packet.parsed);
    }
    let d = dec.frame(&frame(&[0u8; 16], 0x2B), &entry(1, 0x2B), 0.0);
    assert_eq!(d.packet.ptype, "EMPTY");
    let h = Header {
        hop_limit: 7,
        hop_start: 7,
        via_mqtt: true,
        ..Default::default()
    };
    assert!(Header::parse(&h.bytes()).unwrap().via_mqtt);
    let h = Header {
        hop_limit: 3,
        hop_start: 0,
        ..Default::default()
    };
    assert_eq!(h.hops(), -1);
    // Of 200 000 random packets fewer than 100 are taken for messages.
    let mut rng = Rng::new(42);
    let mut taken = 0;
    let mut dec = MeshDecoder::default();
    for i in 0..200_000u32 {
        let len = 20 + (rng.below(60) as usize);
        let mut b: Vec<u8> = (0..len).map(|_| rng.next_u32() as u8).collect();
        b[13] = 0x08;
        b[8..12].copy_from_slice(&i.to_le_bytes());
        let d = dec.frame(&frame(&b, 0x2B), &entry(1, 0x2B), 0.0);
        if !d.texts.is_empty() {
            taken += 1;
        }
    }
    assert!(taken < 100, "{taken}");
}

#[test]
fn random_bytes_are_not_lora_aprs_or_meshcom() {
    let mut rng = Rng::new(5);
    let mut dec = MeshDecoder::default();
    let mut accepted = 0;
    for i in 0..20_000 {
        let len = 16 + rng.below(80) as usize;
        let mut b: Vec<u8> = (0..len).map(|_| rng.next_u32() as u8).collect();
        if i % 4 == 0 {
            b[0] = b":!@A"[(i / 4) % 4];
        }
        let d = dec.frame(&frame(&b, 0x12), &entry(4 | 8, 0x12), 0.0);
        if d.packet.parsed {
            accepted += 1;
        }
    }
    assert!(accepted <= 2, "{accepted}");
}

fn mc_frame(dec: &mut MeshDecoder, b: &[u8], t: f64) -> Decoded {
    dec.frame(&frame(b, 0x12), &entry(2, 0x12), t)
}

#[test]
fn two_identities_exchange_direct_texts() {
    let ia = Identity::from_seed(&[1; 32]);
    let ib = Identity::from_seed(&[2; 32]);
    let mut ca = McConfig::new(ia.clone());
    ca.name = "Alpha".into();
    ca.tx_enabled = true;
    ca.tx_channel = Some(channel(0x12));
    let mut cb = McConfig::new(ib.clone());
    cb.name = "Bravo".into();
    cb.tx_enabled = true;
    cb.tx_channel = Some(channel(0x12));
    let mut a = McClient::new(ca, 1);
    let mut b = McClient::new(cb, 2);
    let mut da = MeshDecoder::default();
    da.mc_own = Some(ia.clone());
    let mut db = MeshDecoder::default();
    db.mc_own = Some(ib.clone());
    // Adverts make contacts.
    let adv_a = a.advert(0.0, true).tx.remove(0);
    let adv_b = b.advert(0.0, true).tx.remove(0);
    let x = mc_frame(&mut db, &adv_a.payload, 0.1);
    b.on_packet(0.1, x.mc.as_ref().unwrap());
    let x = mc_frame(&mut da, &adv_b.payload, 0.2);
    a.on_packet(0.2, x.mc.as_ref().unwrap());
    assert_eq!(a.contacts.len(), 1);
    assert_eq!(b.contacts.len(), 1);
    // A to B by flood: B answers with a PATH packet carrying the ack.
    let o = a.send_dm(1.0, "Bravo", "hello Bravo");
    let r = o.tx[0].clone();
    a.on_sent(1.0, r.message_key, 0.5);
    let x = mc_frame(&mut db, &r.payload, 1.5);
    assert_eq!(x.texts[0].text, "hello Bravo");
    let ob = b.on_packet(1.5, x.mc.as_ref().unwrap());
    let ack = ob.tx[0].clone();
    let y = mc_frame(&mut da, &ack.payload, 2.0);
    let oa = a.on_packet(2.0, y.mc.as_ref().unwrap());
    assert_eq!(oa.texts.last().unwrap().state, Some(Delivery::Delivered));
    // The second goes direct along the learned path (empty path: route direct).
    let o = a.send_dm(3.0, "Bravo", "second");
    let p = mc::McPacket::parse(&o.tx[0].payload).unwrap();
    assert_eq!(p.route, 2);
    // B to A.
    let o = b.send_dm(4.0, "Alpha", "hello Alpha");
    let x = mc_frame(&mut da, &o.tx[0].payload, 4.5);
    assert_eq!(x.texts[0].text, "hello Alpha");
    // A dropped ack: retries with attempts 1 and 2, then failed.
    let o = b.send_dm(10.0, "Alpha", "lost");
    let key = o.tx[0].message_key.unwrap();
    b.on_sent(10.0, Some(key), 0.5);
    let mut attempts = vec![];
    let mut last = None;
    let mut t = 10.0;
    while t < 100.0 {
        t += 0.25;
        let o = b.tick(t);
        for r in &o.tx {
            if let Some(n) = b.attempt_of(key) {
                attempts.push(n);
            }
            b.on_sent(t, r.message_key, 0.5);
        }
        if let Some(m) = o.texts.last() {
            last = m.state;
        }
    }
    assert_eq!(attempts, vec![1, 2]);
    assert!(matches!(last, Some(Delivery::Failed(_))));
    let _ = TxRequest::clone(&r);
}

#[test]
fn channels_and_types() {
    let mut dec = MeshDecoder::default();
    // A hashtag channel: unknown until added with an empty secret.
    let b = mc::build_group_text(
        &mc::hashtag_secret("#test"),
        1760000000,
        "Sara",
        "channel talk",
        vec![],
    )
    .unwrap();
    let d = mc_frame(&mut dec, &b, 0.0);
    assert!(d.packet.note.starts_with("unknown channel"));
    dec.add_mc_channel("#test", "").unwrap();
    let d = mc_frame(&mut dec, &b, 1.0);
    assert_eq!(d.texts[0].from, "Sara");
    assert_eq!(d.texts[0].hops, 0);
    // A 32-byte secret.
    let big = [5u8; 32];
    dec.add_mc_channel("big", &mt::hex(&big)).unwrap();
    let b = mc::build_group_text(&big, 1760000001, "Eve", "long key", vec![]).unwrap();
    assert_eq!(mc_frame(&mut dec, &b, 2.0).texts[0].text, "long key");
    assert!(dec.add_mc_channel("short", "abcd").is_err());
    // A wrong key on a known hash.
    let mut b2 = b.clone();
    let n = b2.len();
    b2[n - 1] ^= 1;
    assert!(mc_frame(&mut dec, &b2, 3.0)
        .packet
        .note
        .starts_with("MAC mismatch"));
    dec.clear_user_channels();
    assert_eq!(dec.mc_channels.len(), 1);
    // Direct types without the key.
    let p = mc::McPacket {
        route: 1,
        ptype: 2,
        version: 0,
        hash_size: 1,
        path: vec![],
        payload: vec![0x5a, 0xc3, 1, 2, 9, 9, 9, 9],
    };
    let d = mc_frame(&mut dec, &p.encode(), 4.0);
    assert_eq!(d.packet.note, "encrypted (direct)");
    assert_eq!((d.packet.to.as_str(), d.packet.from.as_str()), ("5a", "c3"));
    let p = mc::McPacket {
        route: 1,
        ptype: 3,
        version: 0,
        hash_size: 1,
        path: vec![vec![0xab]],
        payload: 0x12345678u32.to_le_bytes().to_vec(),
    };
    let d = mc_frame(&mut dec, &p.encode(), 5.0);
    assert_eq!(d.packet.summary, "ack 12345678, 1 hop(s)");
    let p = mc::McPacket {
        route: 1,
        ptype: 9,
        version: 0,
        hash_size: 1,
        path: vec![vec![20], vec![0xF8]],
        payload: vec![1, 0, 0, 0, 0, 0, 0, 0, 0, 0xa1, 0xa2, 0xa3],
    };
    let d = mc_frame(&mut dec, &p.encode(), 6.0);
    assert_eq!(d.packet.summary, "trace 1, 3 nodes, SNR 5.00 -2.00 dB");
    // A reserved hash size.
    let d = mc_frame(&mut dec, &[0x05, 0xC1, 1, 2, 3], 7.0);
    assert!(!d.packet.parsed);
}

/// Each known Meshtastic routing error number has the failure reason whose key is the lower-case protocol name; an
/// unknown number has the reason of an unknown routing error.
#[test]
fn routing_errors_have_their_failure_reasons() {
    use sdrkit_core::Choice;
    use sdrkit_stack_lora::mesh::types::FailureReason;
    for n in 1..64 {
        let name = mt::routing_error_name(n);
        let reason = Failure::Routing(n).reason();
        if name.starts_with("ERROR_") {
            assert_eq!(reason, FailureReason::RoutingError, "{n}");
        } else {
            assert_eq!(reason.key(), name.to_ascii_lowercase(), "{n}");
        }
    }
    assert_eq!(Failure::NoChannel.reason(), FailureReason::NotListening);
}
