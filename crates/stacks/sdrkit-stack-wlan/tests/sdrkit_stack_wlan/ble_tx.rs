//! The advertising senders of `ble` and `bletracker`: packets sent through the stack's transmit stream come back from the
//! stack's own receiver with the same address, PDU type and data on each advertising channel; the data that the fields
//! build; the count, the interval and halt; the refusals.

use super::help::{cell, command, config, feed, num, rows, text};
use sdrkit_core::interface::ReadFields;
use sdrkit_core::{
    Choice, CommandError, Complex32, InputInfo, Output, Owner, Receiver, StackDescriptor, Value,
};
use sdrkit_graph::components::{HostData, HostPort, HostStreams};
use sdrkit_graph::{Graph, RunContext};
use sdrkit_proto_wlan_phy::ble::{ad, build_adv, AdvReceiver, PduType};
use sdrkit_stack_wlan::ble::gen::addr;
use sdrkit_stack_wlan::ble::BLE;
use sdrkit_stack_wlan::tracker::book::{recognise, TrackerKind};
use sdrkit_stack_wlan::tracker::BLETRACKER;
use sdrkit_stack_wlan::typed::{BlePacketRow as P, BleTxFields, BletrackerTrackerRow as T};
use std::sync::{Arc, Mutex};

const RATE: f64 = 4e6;
/// The advertising channels and their centres, Hz.
const CHANNELS: [(u8, f64); 3] = [(37, 2402e6), (38, 2426e6), (39, 2480e6)];

/// The transmit stream: each write with its end mark, the centre and the stated start of each write, and the lead
/// that the host gives.
#[derive(Default)]
struct Sink {
    writes: Mutex<Vec<(Vec<Complex32>, bool)>>,
    centres: Mutex<Vec<f64>>,
    starts: Mutex<Vec<f64>>,
    lead: std::time::Duration,
}

impl HostStreams for Sink {
    fn write(&self, port: HostPort, data: HostData<'_>) {
        if let (
            HostPort::Transmit,
            HostData::Iq {
                samples,
                end,
                centre_hz,
                start,
                ..
            },
        ) = (port, data)
        {
            self.writes.lock().unwrap().push((samples.to_vec(), end));
            self.centres.lock().unwrap().push(centre_hz);
            self.starts.lock().unwrap().extend(start);
        }
    }
    fn transmit_lead(&self) -> std::time::Duration {
        self.lead
    }
}

impl Sink {
    /// The bursts: their samples and whether the end mark came.
    fn bursts(&self) -> Vec<(Vec<Complex32>, bool)> {
        let mut out: Vec<(Vec<Complex32>, bool)> = Vec::new();
        let mut open = false;
        for (s, end) in self.writes.lock().unwrap().iter() {
            if !open {
                out.push((Vec::new(), false));
            }
            let b = out.last_mut().unwrap();
            b.0.extend_from_slice(s);
            b.1 = *end;
            open = !end;
        }
        out
    }
}

/// Stack `d` at [`RATE`], its input centred on `centre` and tuned to `tuned`, transmit on or off, and its transmit stream.
fn station(
    d: &StackDescriptor,
    centre: f64,
    tuned: f64,
    transmit: bool,
) -> (Box<dyn Receiver>, Arc<Sink>) {
    station_with(
        d,
        InputInfo::new(RATE, centre, tuned).recording(),
        Sink::default(),
        transmit,
    )
}

/// Stack `d` on `input` with the transmit stream `sink`, transmit on or off.
fn station_with(
    d: &StackDescriptor,
    input: InputInfo,
    sink: Sink,
    transmit: bool,
) -> (Box<dyn Receiver>, Arc<Sink>) {
    let mut rx = (d.make_receiver)(&config((d.settings)(), &[])).unwrap();
    rx.configure(&input).unwrap();
    let sink = Arc::new(sink);
    let g = rx
        .as_any_mut()
        .and_then(|a| a.downcast_mut::<Graph>())
        .expect("the stack is a graph");
    let mut ctx: RunContext = g.context().clone();
    ctx.streams = Some(sink.clone());
    g.set_context(ctx);
    assert!(rx.transmits());
    rx.set_transmit(transmit);
    (rx, sink)
}

/// Feeds `seconds` of silence in pieces of `piece` samples; calls `each` after every piece.
fn idle(rx: &mut dyn Receiver, seconds: f64, piece: usize, mut each: impl FnMut(usize)) {
    let zeros = vec![Complex32::default(); piece];
    let mut out = Output::default();
    for i in 0..(seconds * RATE / piece as f64).round() as usize {
        out.clear();
        rx.feed(&zeros, &mut out);
        each(i);
    }
}

/// The PDUs that the advertising receiver of the stacks decodes from `x`, with their channel.
fn decode(x: &[Complex32]) -> Vec<(u8, Vec<u8>)> {
    let mut rx = AdvReceiver::new(4).unwrap();
    let mut out = Vec::new();
    let mut padded = x.to_vec();
    padded.extend(std::iter::repeat_n(Complex32::default(), 400));
    rx.process(&padded, |p| out.push((p.channel, p.pdu.to_vec())));
    out
}

/// The text arguments `args` as values.
fn args<'a>(a: &[(&'a str, &str)]) -> Vec<(&'a str, Value)> {
    a.iter()
        .map(|(k, v)| (*k, Value::Text((*v).into())))
        .collect()
}

/// The transmit fields.
fn fields(rx: &dyn Receiver) -> BleTxFields {
    BleTxFields::read(Owner::results(rx).fields()).expect("the transmit fields")
}

/// The advertising data of the tests.
fn kitchen_data() -> Vec<u8> {
    let mut d = Vec::new();
    ad::push(&mut d, ad::FLAGS, &[6]).unwrap();
    ad::push(&mut d, ad::COMPLETE_NAME, b"Kitchen").unwrap();
    ad::push_manufacturer(&mut d, 0x0059, &[0xDE, 0xAD]).unwrap();
    d
}

#[test]
fn a_sent_packet_decodes_to_the_same_pdu_on_each_channel() {
    let data = kitchen_data();
    let mut hex = String::new();
    sdrkit_core::base64::hex_into(&data, &mut hex);
    for (ch, hz) in CHANNELS {
        for (key, t, random) in [
            (0u16, PduType::AdvNonconnInd, true),
            (1, PduType::AdvInd, false),
            (2, PduType::AdvScanInd, true),
        ] {
            let (mut rx, sink) = station(&BLE, hz, hz, true);
            let mut a = args(&[("address", "C0:FF:EE:12:34:56"), ("data", &hex)]);
            a.push(("pdu_type", Value::Choice(key)));
            a.push(("address_type", Value::Choice(u16::from(random))));
            command(rx.as_mut(), "send", &a).unwrap();
            idle(rx.as_mut(), 0.01, 4000, |_| {});
            let bursts = sink.bursts();
            assert_eq!(bursts.len(), 1, "channel {ch}");
            assert!(bursts[0].1, "the end mark");
            let mut want = Vec::new();
            build_adv(&mut want, t, &addr("C0:FF:EE:12:34:56", random), &data).unwrap();
            assert_eq!(decode(&bursts[0].0), vec![(ch, want)], "channel {ch} {t:?}");
            let peak = bursts[0].0.iter().map(|z| z.norm()).fold(0.0f32, f32::max);
            assert!((peak - 0.5).abs() < 0.01, "level {peak}");

            // The stack's own receive side, as a recording: the whitening that passes the CRC names the channel.
            let (mut back, _) = station(&BLE, hz, hz, false);
            let mut x = bursts[0].0.clone();
            x.extend(std::iter::repeat_n(Complex32::default(), 40_000));
            feed(back.as_mut(), &x, 4096);
            let r = Owner::results(back.as_ref());
            let packets = rows(r, "packets");
            assert_eq!(packets.len(), 1, "channel {ch}");
            assert_eq!(text(packets[0], &P::PDU), t.name());
            assert_eq!(text(packets[0], &P::ADDRESS), "C0:FF:EE:12:34:56");
            assert_eq!(
                cell(packets[0], &P::CHANNEL),
                Some(&Value::I64(i64::from(ch)))
            );
            assert_eq!(
                cell(packets[0], &P::LENGTH),
                Some(&Value::I64(6 + data.len() as i64))
            );
        }
    }
}

#[test]
fn the_carrier_sits_on_the_channel_when_the_tuning_is_near() {
    // Tuned 60 kHz above channel 38: the carrier goes on the channel, 60 kHz below the centre of the samples.
    let (mut rx, sink) = station(&BLE, 2426.06e6, 2426.06e6, true);
    command(
        rx.as_mut(),
        "send",
        &args(&[("address", "112233445566"), ("name", "Near")]),
    )
    .unwrap();
    idle(rx.as_mut(), 0.01, 4000, |_| {});
    let x = sink.bursts().remove(0).0;
    let (mut back, _) = station(&BLE, 2426.06e6, 2426e6, false);
    let mut y = x.clone();
    y.extend(std::iter::repeat_n(Complex32::default(), 40_000));
    feed(back.as_mut(), &y, 4096);
    let r = Owner::results(back.as_ref());
    let packets = rows(r, "packets");
    assert_eq!(packets.len(), 1);
    assert_eq!(cell(packets[0], &P::CHANNEL), Some(&Value::I64(38)));
    let offset = num(r, "offset_hz").unwrap();
    assert!(offset.abs() < 10e3, "offset {offset}");
}

#[test]
fn after_a_retune_the_packet_goes_out_on_the_new_channel() {
    // Offset tuning as a radio does: the centre of the samples 1 MHz below the tuned frequency.
    let (mut rx, sink) = station(&BLE, 2401e6, 2402e6, true);
    let a = args(&[("address", "C0:FF:EE:12:34:56"), ("name", "Hop")]);
    command(rx.as_mut(), "send", &a).unwrap();
    idle(rx.as_mut(), 0.01, 4000, |_| {});
    rx.retuned(&InputInfo::new(RATE, 2425e6, 2426e6).recording());
    command(rx.as_mut(), "send", &a).unwrap();
    idle(rx.as_mut(), 0.01, 4000, |_| {});
    let bursts = sink.bursts();
    assert_eq!(bursts.len(), 2);
    assert_eq!(decode(&bursts[0].0)[0].0, 37);
    assert_eq!(decode(&bursts[1].0)[0].0, 38);
    assert_eq!(sink.centres.lock().unwrap()[0], 2402e6);
    assert_eq!(sink.centres.lock().unwrap().last(), Some(&2426e6));
}

#[test]
fn with_a_host_that_times_bursts_each_packet_states_its_time() {
    let sink = Sink {
        lead: std::time::Duration::from_millis(5),
        ..Sink::default()
    };
    let (mut rx, sink) = station_with(&BLE, InputInfo::new(RATE, 2402e6, 2402e6), sink, true);
    let mut a = args(&[("address", "C0:FF:EE:12:34:56")]);
    a.push(("count", Value::I64(3)));
    a.push(("interval_ms", Value::I64(50)));
    command(rx.as_mut(), "send", &a).unwrap();
    idle(rx.as_mut(), 0.2, 4000, |_| {});
    let starts = sink.starts.lock().unwrap().clone();
    assert_eq!(starts.len(), 3, "{starts:?}");
    for w in starts.windows(2) {
        assert!((w[1] - w[0] - 0.05).abs() < 1e-9, "{starts:?}");
    }
}

#[test]
fn the_fields_build_the_data() {
    let (mut rx, sink) = station(&BLE, 2480e6, 2480e6, true);
    let a = args(&[
        ("address", "c0-ff-ee-00-00-01"),
        ("name", "Probe"),
        ("uuid16", "180F"),
        ("company", "004C"),
        ("manufacturer_data", "01 02 03"),
    ]);
    command(rx.as_mut(), "send", &a).unwrap();
    idle(rx.as_mut(), 0.01, 4000, |_| {});
    let mut want = Vec::new();
    ad::push(&mut want, ad::COMPLETE_NAME, b"Probe").unwrap();
    ad::push_uuids16(&mut want, &[0x180F]).unwrap();
    ad::push_manufacturer(&mut want, 0x004C, &[1, 2, 3]).unwrap();
    let mut pdu = Vec::new();
    build_adv(
        &mut pdu,
        PduType::AdvNonconnInd,
        &addr("C0:FF:EE:00:00:01", true),
        &want,
    )
    .unwrap();
    assert_eq!(decode(&sink.bursts()[0].0), vec![(39, pdu)]);
}

#[test]
fn count_packets_go_out_interval_apart_each_with_its_end_mark() {
    let (mut rx, sink) = station(&BLE, 2402e6, 2402e6, true);
    let mut a = args(&[("address", "C0:FF:EE:12:34:56"), ("name", "Tick")]);
    a.push(("count", Value::I64(3)));
    a.push(("interval_ms", Value::I64(20)));
    command(rx.as_mut(), "send", &a).unwrap();
    assert_eq!(fields(rx.as_ref()).tx_left, 3);
    // Pieces of 1 ms: the pass in which each burst comes.
    let mut seen = Vec::new();
    let mut n = 0;
    idle(rx.as_mut(), 0.1, 4000, |i| {
        let b = sink.bursts().len();
        if b > n {
            seen.push(i);
            n = b;
        }
    });
    assert_eq!(seen, vec![0, 20, 40]);
    let bursts = sink.bursts();
    assert!(bursts.iter().all(|b| b.1));
    for b in &bursts {
        assert_eq!(decode(&b.0).len(), 1);
    }
    let f = fields(rx.as_ref());
    assert!(!f.tx_sending && f.tx_left == 0, "{f:?}");
}

#[test]
fn halt_drops_the_packets_that_wait() {
    let (mut rx, sink) = station(&BLE, 2402e6, 2402e6, true);
    let mut a = args(&[("address", "C0:FF:EE:12:34:56")]);
    a.push(("count", Value::I64(10)));
    a.push(("interval_ms", Value::I64(20)));
    command(rx.as_mut(), "send", &a).unwrap();
    idle(rx.as_mut(), 0.03, 4000, |_| {});
    command(rx.as_mut(), "halt", &[]).unwrap();
    idle(rx.as_mut(), 0.3, 4000, |_| {});
    assert_eq!(sink.bursts().len(), 2);
    assert!(!fields(rx.as_ref()).tx_sending);
}

#[test]
fn a_send_during_another_waits_for_it() {
    let (mut rx, sink) = station(&BLE, 2402e6, 2402e6, true);
    let mut a = args(&[("address", "C0:FF:EE:12:34:56")]);
    a.push(("count", Value::I64(2)));
    a.push(("interval_ms", Value::I64(20)));
    command(rx.as_mut(), "send", &a).unwrap();
    command(rx.as_mut(), "send", &a).unwrap();
    assert_eq!(fields(rx.as_ref()).tx_left, 4);
    let mut seen = Vec::new();
    let mut n = 0;
    idle(rx.as_mut(), 0.1, 4000, |i| {
        let b = sink.bursts().len();
        if b > n {
            seen.push(i);
            n = b;
        }
    });
    assert_eq!(seen, vec![0, 20, 40, 60]);
}

#[test]
fn refusals_send_nothing() {
    let refused = |hz: f64, transmit: bool, a: &[(&str, Value)]| -> CommandError {
        let (mut rx, sink) = station(&BLE, hz, hz, transmit);
        let e = command(rx.as_mut(), "send", a).unwrap_err();
        idle(rx.as_mut(), 0.01, 4000, |_| {});
        assert!(sink.bursts().is_empty());
        e
    };
    let ok = args(&[("address", "C0:FF:EE:12:34:56")]);
    let e = refused(2402e6, false, &ok);
    assert!(e.to_string().contains("transmitter is off"), "{e}");
    let e = refused(2410e6, true, &ok);
    assert!(e.to_string().contains("not an advertising channel"), "{e}");
    let e = refused(2402e6, true, &args(&[("address", "C0:FF:EE:12:34")]));
    assert!(e.to_string().contains("12 hex digits"), "{e}");
    let both = args(&[
        ("address", "C0:FF:EE:12:34:56"),
        ("data", "020106"),
        ("name", "x"),
    ]);
    assert!(refused(2402e6, true, &both)
        .to_string()
        .contains("not both"));
    let long = args(&[("address", "C0:FF:EE:12:34:56"), ("data", &"00".repeat(32))]);
    assert!(refused(2402e6, true, &long).to_string().contains("over 31"));
    let bad = args(&[("address", "C0:FF:EE:12:34:56"), ("data", "0G")]);
    assert!(refused(2402e6, true, &bad).to_string().contains("not hex"));
    let orphan = args(&[
        ("address", "C0:FF:EE:12:34:56"),
        ("manufacturer_data", "01"),
    ]);
    assert!(refused(2402e6, true, &orphan)
        .to_string()
        .contains("company"));
}

#[test]
fn a_tracker_frame_decodes_to_its_family_on_each_channel() {
    let key = "0102030405060708090A0B0C0D0E0F101112131415161718";
    for (ch, hz) in CHANNELS {
        for (family, payload, status, protection) in [
            (TrackerKind::FindMy, key, 0x10i64, false),
            (TrackerKind::Google, "A1B2C3D4E5F60718", 0, true),
        ] {
            let tracker = match family {
                TrackerKind::FindMy => 0u16,
                _ => 1,
            };
            let (mut rx, sink) = station(&BLETRACKER, hz, hz, true);
            let mut a = args(&[("address", "E9:2E:77:C4:31:5A"), ("payload", payload)]);
            a.push(("tracker", Value::Choice(tracker)));
            a.push(("status", Value::I64(status)));
            a.push(("protection", Value::Bool(protection)));
            command(rx.as_mut(), "send", &a).unwrap();
            idle(rx.as_mut(), 0.01, 4000, |_| {});
            let bursts = sink.bursts();
            assert_eq!(bursts.len(), 1);
            let got = decode(&bursts[0].0);
            assert_eq!(got.len(), 1);
            assert_eq!(got[0].0, ch);
            let pdu = sdrkit_proto_wlan_phy::ble::AdvPdu::parse(&got[0].1).unwrap();
            assert_eq!(pdu.header.pdu_type, PduType::AdvNonconnInd);
            assert_eq!(pdu.adv_a(), Some(addr("E9:2E:77:C4:31:5A", true)));
            let r = recognise(pdu.data().unwrap()).expect("a tracker");
            assert_eq!(r.kind, family);
            match family {
                TrackerKind::FindMy => assert_eq!(r.status, Some(0x10)),
                _ => assert!(r.unwanted_protection),
            }

            // The stack's own receive side lists the tracker.
            let (mut back, _) = station(&BLETRACKER, hz, hz, false);
            let mut x = bursts[0].0.clone();
            x.extend(std::iter::repeat_n(Complex32::default(), 40_000));
            feed(back.as_mut(), &x, 4096);
            let r = Owner::results(back.as_ref());
            let trackers = rows(r, "trackers");
            assert_eq!(trackers.len(), 1, "channel {ch} {family:?}");
            assert_eq!(
                cell(trackers[0], &T::KIND),
                Some(&Value::Choice(family.index()))
            );
        }
    }
}

#[test]
fn a_tracker_frame_of_the_wrong_length_is_refused() {
    let (mut rx, sink) = station(&BLETRACKER, 2402e6, 2402e6, true);
    let a = args(&[("address", "E9:2E:77:C4:31:5A"), ("payload", "0102")]);
    let e = command(rx.as_mut(), "send", &a).unwrap_err();
    assert!(e.to_string().contains("not 24"), "{e}");
    let mut a = args(&[
        ("address", "E9:2E:77:C4:31:5A"),
        ("payload", &"00".repeat(27)),
    ]);
    a.push(("tracker", Value::Choice(1)));
    let e = command(rx.as_mut(), "send", &a).unwrap_err();
    assert!(e.to_string().contains("over 26"), "{e}");
    idle(rx.as_mut(), 0.01, 4000, |_| {});
    assert!(sink.bursts().is_empty());
}

/// A Tile, Chipolo or SmartTag send: the tracker choice, the arguments, the advertising data it must build, the mode and
/// the service data hex that the table must show.
struct TagCase {
    family: TrackerKind,
    args: Vec<(&'static str, Value)>,
    data: Vec<u8>,
    mode: sdrkit_stack_wlan::tracker::layout::TagMode,
    service_data: Option<&'static str>,
}

fn tag_cases() -> Vec<TagCase> {
    use sdrkit_stack_wlan::tracker::layout::{SmartTagState, TagMode};
    let tile_value = [0x02, 0x00, 0x8C, 0x1F, 0x3A, 0x77, 0x05, 0xE1, 0x42, 0x9B];
    let mut tile = Vec::new();
    ad::push_uuids16(&mut tile, &[0xFEED]).unwrap();
    ad::push_service_data16(&mut tile, 0xFEED, &tile_value).unwrap();
    let mut shipping = Vec::new();
    ad::push_uuids16(&mut shipping, &[0xFEEC]).unwrap();
    let mut chipolo = Vec::new();
    ad::push_uuids16(&mut chipolo, &[0xFE33]).unwrap();
    ad::push_service_data16(&mut chipolo, 0xFE33, &[0x10, 0x01, 0xF6, 0x55, 0x44]).unwrap();
    // State overmature lost (3), aging 0x0A1B2C, region 5, E2E, UWB, battery 1.
    let value = [
        0x13, 0x2C, 0x1B, 0x0A, 1, 2, 3, 4, 5, 6, 7, 8, 0x5D, 0, 0, 0, 0xDE, 0xAD, 0xBE, 0xEF,
    ];
    let mut smarttag = Vec::new();
    ad::push_service_data16(&mut smarttag, 0xFD5A, &value).unwrap();
    let text = |k: &'static str, v: &str| (k, Value::Text(v.into()));
    vec![
        TagCase {
            family: TrackerKind::Tile,
            args: vec![
                ("tracker", Value::Choice(2)),
                text("service_data", "02 00 8C 1F 3A 77 05 E1 42 9B"),
            ],
            data: tile,
            mode: TagMode::Activated,
            service_data: Some("02008C1F3A7705E1429B"),
        },
        TagCase {
            family: TrackerKind::Tile,
            args: vec![
                ("tracker", Value::Choice(2)),
                ("shipping", Value::Bool(true)),
            ],
            data: shipping,
            mode: TagMode::Shipping,
            service_data: None,
        },
        TagCase {
            family: TrackerKind::Chipolo,
            args: vec![
                ("tracker", Value::Choice(3)),
                text("service_data", "1001F65544"),
            ],
            data: chipolo,
            mode: TagMode::Disconnected,
            service_data: Some("1001F65544"),
        },
        TagCase {
            family: TrackerKind::SmartTag,
            args: vec![
                ("tracker", Value::Choice(4)),
                (
                    "tag_state",
                    Value::Choice(SmartTagState::OvermatureLost.index()),
                ),
                ("aging_counter", Value::I64(0x0A1B2C)),
                text("privacy_id", "0102030405060708"),
                ("region", Value::I64(5)),
                ("e2e", Value::Bool(true)),
                ("uwb", Value::Bool(true)),
                ("battery", Value::I64(1)),
                text("signature", "DEADBEEF"),
            ],
            data: smarttag,
            mode: TagMode::Registered,
            service_data: Some("132C1B0A01020304050607085D000000DEADBEEF"),
        },
    ]
}

#[test]
fn a_tile_chipolo_or_smarttag_frame_decodes_to_its_fields_on_each_channel() {
    use sdrkit_stack_wlan::tracker::layout::SmartTagState;
    for (ch, hz) in CHANNELS {
        for c in tag_cases() {
            let (mut rx, sink) = station(&BLETRACKER, hz, hz, true);
            let mut a = args(&[("address", "D1:22:33:44:55:66")]);
            a.extend(c.args.iter().cloned());
            a.push(("pdu_type", Value::Choice(1)));
            command(rx.as_mut(), "send", &a).unwrap();
            idle(rx.as_mut(), 0.01, 4000, |_| {});
            let bursts = sink.bursts();
            assert_eq!(bursts.len(), 1);
            let mut want = Vec::new();
            build_adv(
                &mut want,
                PduType::AdvInd,
                &addr("D1:22:33:44:55:66", true),
                &c.data,
            )
            .unwrap();
            assert_eq!(
                decode(&bursts[0].0),
                vec![(ch, want)],
                "channel {ch} {:?}",
                c.mode
            );

            let (mut back, _) = station(&BLETRACKER, hz, hz, false);
            let mut x = bursts[0].0.clone();
            x.extend(std::iter::repeat_n(Complex32::default(), 40_000));
            feed(back.as_mut(), &x, 4096);
            let r = Owner::results(back.as_ref());
            let trackers = rows(r, "trackers");
            assert_eq!(trackers.len(), 1, "channel {ch} {:?}", c.mode);
            let row = trackers[0];
            assert_eq!(cell(row, &T::KIND), Some(&Value::Choice(c.family.index())));
            assert_eq!(cell(row, &T::MODE), Some(&Value::Choice(c.mode.index())));
            match c.service_data {
                Some(h) => assert_eq!(text(row, &T::SERVICE_DATA), h),
                None => assert!(cell(row, &T::SERVICE_DATA).is_none()),
            }
            if c.family == TrackerKind::SmartTag {
                assert_eq!(
                    cell(row, &T::TAG_STATE),
                    Some(&Value::Choice(SmartTagState::OvermatureLost.index()))
                );
                assert_eq!(cell(row, &T::TAG_STATE_CODE), Some(&Value::I64(3)));
                assert_eq!(cell(row, &T::AGING_COUNTER), Some(&Value::I64(0x0A1B2C)));
                assert_eq!(cell(row, &T::REGION), Some(&Value::I64(5)));
                assert_eq!(cell(row, &T::E2E), Some(&Value::Bool(true)));
                assert_eq!(cell(row, &T::UWB), Some(&Value::Bool(true)));
                assert_eq!(cell(row, &T::BATTERY), Some(&Value::I64(1)));
                assert_eq!(text(row, &T::IDENTIFIER), "0102030405060708");
            }
        }
    }
}

#[test]
fn a_tile_chipolo_or_smarttag_frame_out_of_its_sizes_is_refused() {
    let (mut rx, sink) = station(&BLETRACKER, 2402e6, 2402e6, true);
    let refused = |rx: &mut dyn Receiver, extra: Vec<(&'static str, Value)>| {
        let mut a = args(&[("address", "D1:22:33:44:55:66")]);
        a.extend(extra);
        command(rx, "send", &a).unwrap_err().to_string()
    };
    let t = |v: &str| Value::Text(v.into());
    let e = refused(rx.as_mut(), vec![("tracker", Value::Choice(2))]);
    assert!(e.contains("service_data is 0 bytes, not 1 to 23"), "{e}");
    let e = refused(
        rx.as_mut(),
        vec![
            ("tracker", Value::Choice(3)),
            ("service_data", t(&"00".repeat(24))),
        ],
    );
    assert!(e.contains("not 1 to 23"), "{e}");
    let e = refused(
        rx.as_mut(),
        vec![
            ("tracker", Value::Choice(4)),
            ("privacy_id", t("0102")),
            ("signature", t("01020304")),
        ],
    );
    assert!(e.contains("privacy_id is 2 bytes, not 8"), "{e}");
    let e = refused(
        rx.as_mut(),
        vec![
            ("tracker", Value::Choice(4)),
            ("privacy_id", t("0102030405060708")),
        ],
    );
    assert!(e.contains("signature is 0 bytes, not 4"), "{e}");
    let e = refused(
        rx.as_mut(),
        vec![
            ("tracker", Value::Choice(4)),
            ("privacy_id", t("0102030405060708")),
            ("signature", t("01020304")),
            ("battery", Value::I64(4)),
        ],
    );
    assert!(e.contains("outside 0 to 3"), "{e}");
    idle(rx.as_mut(), 0.01, 4000, |_| {});
    assert!(sink.bursts().is_empty());
}
