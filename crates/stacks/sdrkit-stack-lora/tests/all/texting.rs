//! The texting clients end to end, through the receivers: two stations share one channel. Each frame a station requests to
//! send (a `tx.request` event) is made by the family's transmitter and put on the air that both stations hear. Nothing goes
//! to a radio.

use crate::common::{band_plan, catalogue, count, invoke, receiver, row, slot_band};
use sdrkit_core::publish::keys;
use sdrkit_core::verify::CaseSetting;
use sdrkit_core::{Complex32, Event, InputInfo, Output, Receiver, Record, Station, Value};
use sdrkit_proto_lora_phy::{LoraParams, TxFrame};
use sdrkit_stack_lora::mesh::types::{DeliveryState, TxKind};
use sdrkit_stack_lora::proto::meshtastic::{self as mt, Header};
use sdrkit_stack_lora::settings::*;
use sdrkit_stack_lora::typed::{
    MeshChannelRow, MeshMcFields, MeshMessageRow, MeshMtFields, MeshNodeRow, TxRequestEvent,
    MESH_CHANNELS, MESH_MESSAGES, MESH_NODES, TX_REQUEST,
};

const RATE: f64 = 2e6;
const CENTRE: f64 = 869.825e6;
const DIAL: f64 = 869.525e6;
const STEP: usize = 200_000;

/// The channel both stations share, received at `centre`.
struct Air {
    x: Vec<Complex32>,
    centre: f64,
}

impl Air {
    /// Puts a frame on the air at `at_s`.
    fn send(&mut self, at_s: f64, freq_hz: f64, payload: &[u8], p: &LoraParams) {
        let f = TxFrame::new(p, payload)
            .expect("a valid frame")
            .carrier(freq_hz - self.centre)
            .amplitude(0.5);
        let s = f.render(RATE, 0.0);
        let i0 = (at_s * RATE) as usize;
        if self.x.len() < i0 + s.len() {
            self.x.resize(i0 + s.len(), Complex32::default());
        }
        for (a, b) in self.x[i0..].iter_mut().zip(s) {
            *a += b;
        }
    }
}

/// A text value.
fn text(t: &str) -> Value {
    Value::Text(t.into())
}

/// The newest message of `res` with this text.
fn message<'a>(res: &'a sdrkit_core::Results, t: &str) -> Option<&'a Record> {
    row(res, &MESH_MESSAGES, |r| r.get(&keys::TEXT) == Some(t))
}

/// The sender as people see it: the name, or the id.
fn sender(r: &Record) -> &str {
    r.at(&MeshMessageRow::SENDER)
        .or(r.get(&keys::SOURCE))
        .unwrap_or("")
}

/// A Meshtastic station with transmit on, as `node` named `name`.
fn station(node: &str, name: &str, extra: &[CaseSetting]) -> Box<dyn Receiver> {
    let mut s = vec![
        CaseSetting::new(&MT_NODE_ID, node),
        CaseSetting::new(&MT_LONG_NAME, name),
        CaseSetting::new(&MT_SHORT_NAME, &name[..2]),
        CaseSetting::new(&DUTY_LIMIT, false),
    ];
    s.extend(extra.iter().cloned());
    let mut rx = receiver(&sdrkit_stack_lora::MESHTASTIC, Some("meshtastic"), &s);
    rx.set_transmit(true);
    rx.configure(&InputInfo::new(RATE, CENTRE, DIAL).recording())
        .unwrap();
    rx
}

/// Runs the stations for `seconds`. `act(step, stations)` makes the user's actions. Every frame a station requests to send
/// goes on the air 10 ms after the step that requested it. Returns the events of each station.
fn exchange(
    stations: &mut [Box<dyn Receiver>],
    seconds: f64,
    params: &LoraParams,
    act: impl FnMut(usize, &mut [Box<dyn Receiver>]),
) -> Vec<Vec<Event>> {
    exchange_at(stations, seconds, params, CENTRE, act)
}

/// As [`exchange`], with the stations' radios centred on `centre`.
fn exchange_at(
    stations: &mut [Box<dyn Receiver>],
    seconds: f64,
    params: &LoraParams,
    centre: f64,
    mut act: impl FnMut(usize, &mut [Box<dyn Receiver>]),
) -> Vec<Vec<Event>> {
    let mut air = Air {
        x: Vec::new(),
        centre,
    };
    let mut events = vec![Vec::new(); stations.len()];
    let mut out = Output::default();
    let steps = (seconds * RATE) as usize / STEP;
    for k in 0..steps {
        act(k, stations);
        let a = k * STEP;
        if air.x.len() < a + STEP {
            air.x.resize(a + STEP, Complex32::default());
        }
        let piece = air.x[a..a + STEP].to_vec();
        for (i, st) in stations.iter_mut().enumerate() {
            out.clear();
            st.feed(&piece, &mut out);
            for e in out.events.drain(..) {
                if e.is(&TX_REQUEST) {
                    let bytes = e.at(&TxRequestEvent::PAYLOAD).unwrap_or(&[]).to_vec();
                    let f = e.get(&keys::FREQ_HZ).unwrap_or(DIAL);
                    air.send((a + STEP) as f64 / RATE + 0.01, f, &bytes, params);
                }
                events[i].push(e);
            }
        }
    }
    events
}

fn longfast() -> LoraParams {
    let eu = slot_band("EU_868");
    mt::lora_params(mt::preset("LongFast").unwrap(), &eu)
}

fn sent(ev: &[Event], what: TxKind) -> Vec<&Event> {
    ev.iter()
        .filter(|e| e.is(&TX_REQUEST) && e.at(&TxRequestEvent::WHAT) == Some(what))
        .collect()
}

fn header(e: &Event) -> Header {
    Header::parse(e.at(&TxRequestEvent::PAYLOAD).unwrap()).unwrap()
}

/// A channel text with the published default key reaches the other station. A NodeInfo request provides each station with the
/// other's name and public key, so the direct text that follows goes by PKI: the other station reads it, acknowledges it, and
/// the own message shows "delivered".
#[test]
fn a_channel_text_and_a_pki_direct_text_are_delivered() {
    let mut st = vec![
        station(
            "!0000a001",
            "Alpha",
            &[
                CaseSetting::new(&MT_HOP_LIMIT, 2),
                CaseSetting::new(&MT_ACK_BROADCAST, false),
            ],
        ),
        station("!0000b002", "Bravo", &[]),
    ];
    let p = longfast();
    let ev = exchange(&mut st, 40.0, &p, |k, s| {
        if k == 5 {
            invoke(s[0].as_mut(), "mt.send", &[("text", text("hello mesh"))]).unwrap();
            invoke(s[0].as_mut(), "mt.request", &[("node", text("!0000b002"))]).unwrap();
        }
        if k == 60 {
            invoke(
                s[0].as_mut(),
                "mt.send",
                &[("to", text("!0000b002")), ("text", text("now by pki"))],
            )
            .unwrap();
        }
    });
    // The station's identity, its NodeInfo, and the hop limit of its own packets.
    let a = st[0].results();
    assert_eq!(a.fields().at(&MeshMtFields::MT_NODE_ID), Some("!0000a001"));
    assert!(!a
        .fields()
        .at(&MeshMtFields::MT_PUBLIC_KEY)
        .unwrap_or("")
        .is_empty());
    assert!(!sent(&ev[0], TxKind::NodeInfo).is_empty());
    assert!(
        !sent(&ev[1], TxKind::NodeInfo).is_empty(),
        "Bravo answers the NodeInfo request"
    );
    let texts = sent(&ev[0], TxKind::Text);
    assert_eq!(texts.len(), 2);
    assert_eq!(header(texts[0]).hop_limit, 2);
    assert_eq!(header(texts[0]).to, mt::BROADCAST);
    // Bravo read the channel text with the default key and the direct text by PKI.
    let b = st[1].results();
    let m = message(b, "hello mesh").expect("the channel text at Bravo");
    assert_eq!(sender(m), "Alpha");
    assert_eq!(m.get(&keys::CHANNEL), Some("LongFast"));
    let m = message(b, "now by pki").expect("the PKI text at Bravo");
    assert_eq!(m.get(&keys::CHANNEL), Some("DM"));
    assert!(header(texts[1]).want_ack);
    assert_eq!(
        header(texts[1]).channel,
        0,
        "a PKI packet has channel hash 0"
    );
    assert!(!sent(&ev[1], TxKind::Ack).is_empty(), "Bravo acknowledges");
    let m = message(a, "now by pki").expect("the own message");
    assert_eq!(m.at(&MeshMessageRow::STATE), Some(DeliveryState::Delivered));
    // The node lists hold each other, with names.
    let n = row(b, &MESH_NODES, |r| {
        r.at(&MeshNodeRow::ID) == Some("!0000a001")
    })
    .expect("Alpha at Bravo");
    assert_eq!(n.at(&MeshNodeRow::NAME), Some("Alpha"));
    let n = row(a, &MESH_NODES, |r| {
        r.at(&MeshNodeRow::ID) == Some("!0000b002")
    })
    .expect("Bravo at Alpha");
    assert_eq!(n.at(&MeshNodeRow::NAME), Some("Bravo"));
}

/// A direct text before the other station's key is known goes with the channel key, and its acknowledgement makes it
/// "delivered".
#[test]
fn a_direct_text_with_the_channel_key_is_delivered() {
    let mut st = vec![
        station("!0000a001", "Alpha", &[]),
        station("!0000b002", "Bravo", &[]),
    ];
    let p = longfast();
    let ev = exchange(&mut st, 20.0, &p, |k, s| {
        if k == 5 {
            invoke(
                s[0].as_mut(),
                "mt.send",
                &[("to", text("!0000b002")), ("text", text("hello bravo"))],
            )
            .unwrap();
        }
    });
    let t = sent(&ev[0], TxKind::Text);
    assert_eq!(t.len(), 1);
    assert!(header(t[0]).want_ack);
    assert_ne!(header(t[0]).channel, 0);
    let m = message(st[1].results(), "hello bravo").expect("the text");
    assert_eq!(m.get(&keys::CHANNEL), Some("DM"));
    let m = message(st[0].results(), "hello bravo").unwrap();
    assert_eq!(m.at(&MeshMessageRow::STATE), Some(DeliveryState::Delivered));
}

/// Without an answer, a direct text is sent again and then fails. The state shows it.
#[test]
fn a_meshtastic_text_without_an_answer_is_retried_then_failed() {
    let mut st = vec![station("!0000a001", "Alpha", &[])];
    let p = longfast();
    let ev = exchange(&mut st, 120.0, &p, |k, s| {
        if k == 5 {
            invoke(
                s[0].as_mut(),
                "mt.send",
                &[("to", text("!0000c003")), ("text", text("anyone there"))],
            )
            .unwrap();
        }
    });
    let retries = sent(&ev[0], TxKind::Retransmission).len();
    assert!(retries >= 1, "{retries} retransmissions");
    let m = message(st[0].results(), "anyone there").unwrap();
    assert_eq!(m.at(&MeshMessageRow::STATE), Some(DeliveryState::Failed));
}

/// The station's position goes out in a position packet, and the other station's node list shows it.
#[test]
fn the_station_position_reaches_the_other_node() {
    let mut a = station(
        "!0000a001",
        "Alpha",
        &[
            CaseSetting::new(&MT_POSITION_MODE, PositionMode::Station),
            CaseSetting::new(&MT_POSITION_INTERVAL, 30.0),
        ],
    );
    a.set_station(&Station::at(sdrkit_util_geo::Position::new(
        52.3702, 4.8952,
    )));
    let mut st = vec![a, station("!0000b002", "Bravo", &[])];
    let p = longfast();
    let ev = exchange(&mut st, 12.0, &p, |_, _| {});
    assert!(!sent(&ev[0], TxKind::Position).is_empty());
    let n = row(st[1].results(), &MESH_NODES, |r| {
        r.at(&MeshNodeRow::ID) == Some("!0000a001")
    })
    .expect("Alpha at Bravo");
    // The default channel sends the position with 13 bits of precision (about 3 km).
    let (lat, lon) = n.get(&keys::POSITION).unwrap();
    assert!(
        (lat - 52.3702).abs() < 0.05 && (lon - 4.8952).abs() < 0.05,
        "{lat}, {lon}"
    );
}

/// A channel text in licensed mode goes out without encryption, with the station's callsign in front. A station whose
/// channel has no key reads it.
#[test]
fn licensed_mode_sends_in_the_clear_under_the_callsign() {
    let mut a = station(
        "",
        "Alpha",
        &[
            CaseSetting::new(&MT_LICENSED, true),
            CaseSetting::new(&MT_CALLSIGN_PREFIX, true),
        ],
    );
    a.set_station(&Station {
        callsign: "VK2ABC".into(),
        ..Default::default()
    });
    let mut b = station("!0000b002", "Bravo", &[]);
    invoke(
        b.as_mut(),
        "mt.channel.add",
        &[("name", text("LongFast")), ("key", text("none"))],
    )
    .unwrap();
    let mut st = vec![a, b];
    let p = longfast();
    // The first copy starts near 2 s. The sender waits for a repeat: the air time, 272 back-off slots of 20.7 ms, the air
    // time of an answer and a random part of up to 1 s, then a back-off of the scheduler. The second copy thus ends by
    // about 10.7 s, and a third one cannot start before 15 s. 13 s holds exactly two copies for every random wait.
    exchange(&mut st, 13.0, &p, |k, s| {
        if k == 3 {
            invoke(s[0].as_mut(), "mt.send", &[("text", text("cq"))]).unwrap();
        }
    });
    let b = st[1].results();
    assert_eq!(
        count(b, &MESH_MESSAGES, |r| r.get(&keys::TEXT)
            == Some("VK2ABC: cq")),
        1
    );
    // No relay repeats the broadcast, so the sender sends it again: the receiver hears two good copies. An own text has
    // neither column.
    let got = message(b, "VK2ABC: cq").unwrap();
    assert_eq!(got.at(&MeshMessageRow::CRC_OK), Some(true));
    assert_eq!(got.at(&MeshMessageRow::COPIES), Some(2));
    let own = row(st[0].results(), &MESH_MESSAGES, |r| {
        r.get(&keys::TEXT).is_some_and(|t| t.ends_with("cq"))
    })
    .unwrap();
    assert_eq!(own.at(&MeshMessageRow::CRC_OK), None);
    assert_eq!(own.at(&MeshMessageRow::COPIES), None);
    // The channel key shows only as set.
    let c = row(b, &MESH_CHANNELS, |r| {
        r.get(&keys::CHANNEL) == Some("LongFast")
    })
    .unwrap();
    assert_eq!(c.at(&MeshChannelRow::KEY_SET), Some(true));
}

/// The band of the slots and the preset decide the frequency of the own packets.
#[test]
fn the_region_preset_decides_the_frequency() {
    let (_, mut s) = band_plan("meshtastic", "ANZ 915");
    s.extend([
        catalogue(&MT_PRESET, "MediumFast"),
        CaseSetting::new(&DUTY_LIMIT, false),
    ]);
    let mut rx = receiver(&sdrkit_stack_lora::MESHTASTIC, Some("meshtastic"), &s);
    rx.set_transmit(true);
    let anz = slot_band("ANZ");
    let want = mt::slot_hz(mt::preset("MediumFast").unwrap(), &anz, "", 0, 0.0, 0.0).unwrap();
    rx.configure(&InputInfo::new(RATE, want + 200e3, want).recording())
        .unwrap();
    let mut out = Output::default();
    rx.feed(&vec![Complex32::default(); STEP], &mut out);
    invoke(rx.as_mut(), "mt.send", &[("text", text("hello"))]).unwrap();
    let mut ev = Vec::new();
    for _ in 0..30 {
        out.clear();
        rx.feed(&vec![Complex32::default(); STEP], &mut out);
        ev.append(&mut out.events);
    }
    let t = sent(&ev, TxKind::Text);
    assert_eq!(t.len(), 1);
    assert_eq!(t[0].get(&keys::FREQ_HZ), Some(want));
    assert_eq!(rx.results().fields().at(&MeshMtFields::MT_FREQ), Some(want));
}

/// A client sends only on a channel the session listens to. A mesh session on the ANZ plan at 919.875 MHz listens to the
/// Meshtastic primary channel and to no MeshCore channel: a Meshtastic text leaves on 919.875 MHz, and a MeshCore advert is
/// not sent. After the capture moves off the plan, a Meshtastic text fails and nothing is sent.
#[test]
fn a_client_sends_only_on_a_channel_the_session_listens_to() {
    let (dial, mut s) = band_plan("mesh", "ANZ 915");
    s.push(CaseSetting::new(&DUTY_LIMIT, false));
    let mut rx = receiver(&sdrkit_stack_lora::MESH, Some("mesh"), &s);
    rx.set_transmit(true);
    assert_eq!(dial, 919.875e6);
    rx.configure(&InputInfo::new(RATE, dial + 200e3, dial).recording())
        .unwrap();
    let mut out = Output::default();
    let silence = vec![Complex32::default(); STEP];
    let mut run = |rx: &mut dyn Receiver, steps: usize| {
        let mut ev = Vec::new();
        for _ in 0..steps {
            out.clear();
            rx.feed(&silence, &mut out);
            ev.append(&mut out.events);
        }
        ev
    };
    run(rx.as_mut(), 12);
    assert_eq!(rx.results().fields().at(&MeshMtFields::MT_FREQ), Some(dial));
    assert_eq!(rx.results().fields().at(&MeshMcFields::MC_FREQ), None);
    invoke(rx.as_mut(), "mc.advert", &[]).unwrap();
    invoke(rx.as_mut(), "mt.send", &[("text", text("on the plan"))]).unwrap();
    let ev = run(rx.as_mut(), 30);
    let all: Vec<&Event> = ev.iter().filter(|e| e.is(&TX_REQUEST)).collect();
    assert!(!all.is_empty());
    assert!(all.iter().all(|e| e.get(&keys::FREQ_HZ) == Some(dial)));
    assert!(sent(&ev, TxKind::Advert).is_empty());
    assert_eq!(sent(&ev, TxKind::Text).len(), 1);
    // The capture moves to 906 MHz: no channel of the plan is inside it.
    rx.configure(&InputInfo::new(RATE, 906.2e6, 906e6).recording())
        .unwrap();
    run(rx.as_mut(), 12);
    assert_eq!(rx.results().fields().at(&MeshMtFields::MT_FREQ), None);
    invoke(rx.as_mut(), "mt.send", &[("text", text("off the plan"))]).unwrap();
    let ev = run(rx.as_mut(), 30);
    assert!(ev.iter().all(|e| !e.is(&TX_REQUEST)));
    let m = message(rx.results(), "off the plan").expect("the own message");
    assert_eq!(m.at(&MeshMessageRow::STATE), Some(DeliveryState::Failed));
    assert_eq!(
        m.at(&MeshMessageRow::FAILURE),
        Some(sdrkit_stack_lora::mesh::types::NO_CHANNEL)
    );
}

/// A MeshCore station with transmit on, named `name`.
fn mc_station(name: &str) -> Box<dyn Receiver> {
    let s = [
        CaseSetting::new(&MC_NAME, name),
        CaseSetting::new(&DUTY_LIMIT, false),
    ];
    let mut rx = receiver(&sdrkit_stack_lora::MESHCORE, Some("meshcore"), &s);
    rx.set_transmit(true);
    rx.configure(&InputInfo::new(RATE, 869.618e6 + 300e3, 869.618e6).recording())
        .unwrap();
    rx
}

/// MeshCore texting: adverts make contacts, a direct text is read and acknowledged ("delivered"), and a text on the Public
/// channel is read.
#[test]
fn meshcore_stations_exchange_adverts_and_texts() {
    let mut st = vec![mc_station("Alpha"), mc_station("Bravo")];
    let p = sdrkit_stack_lora::proto::meshcore::params(8, 62.5e3, 8);
    let ev = exchange_at(&mut st, 50.0, &p, 869.618e6 + 300e3, |k, s| match k {
        5 => invoke(s[0].as_mut(), "mc.advert", &[]).unwrap(),
        30 => invoke(s[1].as_mut(), "mc.advert", &[]).unwrap(),
        80 => invoke(
            s[0].as_mut(),
            "mc.send",
            &[("to", text("Bravo")), ("text", text("hello bravo"))],
        )
        .unwrap(),
        150 => invoke(s[1].as_mut(), "mc.send", &[("text", text("hello public"))]).unwrap(),
        _ => {}
    });
    let a = st[0].results();
    let b = st[1].results();
    assert_eq!(a.fields().at(&MeshMcFields::MC_CONTACTS), Some(1));
    assert_eq!(b.fields().at(&MeshMcFields::MC_CONTACTS), Some(1));
    let m = message(b, "hello bravo").expect("the direct text at Bravo");
    assert_eq!(sender(m), "Alpha");
    let m = message(a, "hello bravo").expect("the own message");
    assert_eq!(m.at(&MeshMessageRow::STATE), Some(DeliveryState::Delivered));
    let m = message(a, "hello public").expect("the channel text at Alpha");
    assert_eq!(m.get(&keys::CHANNEL), Some("Public"));
    assert!(!ev.is_empty());
}
