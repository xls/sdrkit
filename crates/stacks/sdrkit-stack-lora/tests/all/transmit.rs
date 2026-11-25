//! Transmit through the graph: a stack's frames leave `lora.modulate` as IQ at `tx.sink`, and a second receiver of the
//! stack decodes them: LoRa frames, and AX.25 frames on FM of the AFSK stack. Nothing goes to a radio.

use crate::common::{band_plan, dial, feed, invoke, packets, receiver, row};
use sdrkit_algo_sim::noise::Awgn;
use sdrkit_core::publish::keys;
use sdrkit_core::verify::CaseSetting;
use sdrkit_core::{
    CommandError, Complex32, InputInfo, Output, Receiver, Record, Station, TxContent, Value,
};
use sdrkit_graph::components::{HostData, HostPort, HostStreams};
use sdrkit_graph::Graph;
use sdrkit_proto_lora_phy::TxFrame;
use sdrkit_stack_lora::gen::damaged_burst;
use sdrkit_stack_lora::mesh::types::{DeliveryState, FailureReason, Protocol};
use sdrkit_stack_lora::proto::hamlora;
use sdrkit_stack_lora::settings::*;
use sdrkit_stack_lora::typed::{
    AprsFields, AprsPacketEvent, AprsSendReply, AprsStationRow, HamFields, LoraPacketEvent,
    MeshMessageRow, MeshStationRow, APRS_PACKET, APRS_STATIONS, MESH_MESSAGES, MESH_STATIONS,
};
use sdrkit_util_geo::Position;
use std::f64::consts::{PI, TAU};
use std::sync::{Arc, Mutex};

const RATE: f64 = 1e6;
const STEP: usize = 50_000;

/// One write to the transmit stream: the samples, their rate and their centre.
type Write = (Vec<Complex32>, f64, f64);

/// The host's transmit stream: joins the writes of each burst and keeps the burst at its end mark.
#[derive(Default)]
struct Bursts(Mutex<Vec<Write>>, Mutex<Vec<Complex32>>);

impl HostStreams for Bursts {
    fn write(&self, port: HostPort, data: HostData<'_>) {
        if let (
            HostPort::Transmit,
            HostData::Iq {
                samples,
                rate_hz,
                centre_hz,
                end,
                ..
            },
        ) = (port, data)
        {
            let mut part = self.1.lock().unwrap();
            part.extend_from_slice(samples);
            if end {
                let burst = std::mem::take(&mut *part);
                self.0.lock().unwrap().push((burst, rate_hz, centre_hz));
            }
        }
    }
}

/// Returns the graph of `rx` the host stream `b`.
fn connect(rx: &mut dyn Receiver, b: &Arc<Bursts>) {
    let g = rx
        .as_any_mut()
        .and_then(|a| a.downcast_mut::<Graph>())
        .expect("a graph");
    let mut ctx = g.context().clone();
    ctx.streams = Some(Arc::clone(b) as Arc<dyn HostStreams>);
    g.set_context(ctx);
}

/// Feeds `seconds` of silence to `rx`. `act` runs after the first block. Returns the writes to `b`.
fn transmit(
    rx: &mut dyn Receiver,
    b: &Arc<Bursts>,
    seconds: f64,
    act: impl FnOnce(&mut dyn Receiver),
) -> Vec<Write> {
    let mut out = Output::default();
    let silence = vec![Complex32::default(); STEP];
    rx.feed(&silence, &mut out);
    act(rx);
    for _ in 0..(seconds * RATE) as usize / STEP {
        out.clear();
        rx.feed(&silence, &mut out);
    }
    std::mem::take(&mut *b.0.lock().unwrap())
}

/// Feeds each burst of `w` between stretches of silence to `rx`, configured at the centre of the bursts and tuned to
/// `tuned`.
fn hear(rx: &mut dyn Receiver, w: &[Write], tuned: f64) {
    rx.configure(&InputInfo::new(RATE, w[0].2, tuned).recording())
        .unwrap();
    let mut x = Vec::new();
    for (burst, _, _) in w {
        x.extend(vec![Complex32::default(); STEP]);
        x.extend_from_slice(burst);
    }
    x.extend(vec![Complex32::default(); 6 * STEP]);
    let mut out = Output::default();
    for c in x.chunks(STEP) {
        out.clear();
        rx.feed(c, &mut out);
    }
}

#[test]
fn a_meshtastic_text_leaves_as_iq_and_another_station_reads_it() {
    let dial = 869.525e6;
    let mut a = receiver(
        &sdrkit_stack_lora::MESHTASTIC,
        Some("meshtastic"),
        &[
            CaseSetting::new(&MT_NODE_ID, "!0000a001"),
            CaseSetting::new(&MT_LONG_NAME, "Alpha"),
            CaseSetting::new(&DUTY_LIMIT, false),
        ],
    );
    a.set_transmit(true);
    a.configure(&InputInfo::new(RATE, dial + 200e3, dial).recording())
        .unwrap();
    let b = Arc::new(Bursts::default());
    connect(a.as_mut(), &b);
    let w = transmit(a.as_mut(), &b, 5.0, |rx| {
        invoke(
            rx,
            "mt.send",
            &[("text", Value::Text("over the air".into()))],
        )
        .unwrap();
    });
    assert!(!w.is_empty(), "no burst at tx.sink");
    assert!(w.iter().all(|x| (x.1, x.2) == (RATE, dial)));
    let mut rx = receiver(&sdrkit_stack_lora::MESHTASTIC, Some("meshtastic"), &[]);
    hear(rx.as_mut(), &w, dial);
    let m = row(rx.results(), &MESH_MESSAGES, |r| {
        r.get(&keys::TEXT) == Some("over the air")
    });
    assert!(m.is_some(), "the text did not reach the second station");
}

#[test]
fn a_raw_frame_leaves_as_iq_and_another_receiver_reads_it() {
    let d = &sdrkit_stack_lora::LORARAW;
    let dial = dial(d);
    let mut a = receiver(d, None, &[]);
    a.set_transmit(true);
    a.configure(&InputInfo::new(RATE, dial + 200e3, dial).recording())
        .unwrap();
    let b = Arc::new(Bursts::default());
    connect(a.as_mut(), &b);
    let w = transmit(a.as_mut(), &b, 1.0, |rx| {
        invoke(
            rx,
            "send",
            &[("payload", Value::Bytes(b"sent by sdrkit"[..].into()))],
        )
        .unwrap();
    });
    assert_eq!(w.len(), 1, "one burst");
    let mut rx = receiver(d, None, &[]);
    hear(rx.as_mut(), &w, dial);
    assert!(
        rx.results().fields().get(&keys::FRAMES_OK).unwrap_or(0) >= 1,
        "the frame was not read"
    );
}

/// The modified Bessel function of the first kind, order 0.
fn bessel_i0(x: f64) -> f64 {
    let (mut sum, mut term, mut k) = (1.0, 1.0, 1.0);
    while term > 1e-14 * sum {
        term *= (x / (2.0 * k)).powi(2);
        sum += term;
        k += 1.0;
    }
    sum
}

/// Resamples `x` by the exact ratio `l / m`, `l < m`: output sample `k` is the input at time `k m / l`, from a
/// Kaiser-windowed sinc of 96 input samples (beta 10, about 100 dB) with its cut-off at 0.3 of the output rate. It shares
/// no code with the receivers.
fn resample(x: &[Complex32], l: usize, m: usize) -> Vec<Complex32> {
    const HALF: isize = 48;
    let cut = 0.6 * l as f64 / m as f64;
    let rows: Vec<Vec<f32>> = (0..l)
        .map(|p| {
            (1 - HALF..=HALF)
                .map(|j| {
                    let t = j as f64 - p as f64 / l as f64;
                    let w = (1.0 - (t / HALF as f64).powi(2)).max(0.0).sqrt();
                    let sinc = if t.abs() < 1e-12 {
                        1.0
                    } else {
                        (PI * cut * t).sin() / (PI * cut * t)
                    };
                    (cut * sinc * bessel_i0(10.0 * w) / bessel_i0(10.0)) as f32
                })
                .collect()
        })
        .collect();
    (0..x.len() * l / m)
        .map(|k| {
            let (i, p) = ((k * m / l) as isize, k * m % l);
            let mut acc = Complex32::default();
            for (h, j) in rows[p].iter().zip(1 - HALF..=HALF) {
                if let Some(v) = usize::try_from(i + j).ok().and_then(|q| x.get(q)) {
                    acc += *v * *h;
                }
            }
            acc
        })
        .collect()
}

#[test]
fn a_burst_at_a_rate_with_a_fraction_of_a_sample_per_chip_is_read_at_two_msps() {
    // The lowest rate of an AD936x converter, and the lowest rate the iiod driver sets: 8.33 and 8.4 samples per chip of
    // 250 kHz.
    for (rate, l, m) in [(25e6 / 12.0, 24, 25), (2.1e6, 20, 21)] {
        let dial = 869.525e6;
        let mut a = receiver(
            &sdrkit_stack_lora::MESHTASTIC,
            Some("meshtastic"),
            &[
                CaseSetting::new(&MT_NODE_ID, "!0000a001"),
                CaseSetting::new(&MT_LONG_NAME, "Alpha"),
                CaseSetting::new(&DUTY_LIMIT, false),
            ],
        );
        a.set_transmit(true);
        a.configure(&InputInfo::new(rate, dial, dial).recording())
            .unwrap();
        let b = Arc::new(Bursts::default());
        connect(a.as_mut(), &b);
        let w = transmit(a.as_mut(), &b, 5.0, |rx| {
            invoke(
                rx,
                "mt.send",
                &[("text", Value::Text("over the air".into()))],
            )
            .unwrap();
        });
        assert!(!w.is_empty(), "no burst at tx.sink");
        assert!(w.iter().all(|x| (x.1, x.2) == (rate, dial)));
        // The first burst (the station's node info) as a receiver at 2 Msps hears it: 4.5 kHz low, with noise 30 dB below
        // it in 250 kHz.
        let out = 2e6;
        let mut x = vec![Complex32::default(); STEP];
        x.extend(resample(&w[0].0, l, m));
        x.extend(vec![Complex32::default(); 6 * STEP]);
        for (i, v) in x.iter_mut().enumerate() {
            let (s, c) = (TAU * (-4500.0 * i as f64 / out).fract()).sin_cos();
            *v *= Complex32::new(c as f32, s as f32);
        }
        let level = w[0].0.iter().map(|v| v.norm_sqr()).fold(0.0f32, f32::max) as f64;
        Awgn::with_power(level / 1000.0 * out / 250e3, 3)
            .unwrap()
            .process(&mut x);
        let mut rx = receiver(&sdrkit_stack_lora::MESHTASTIC, Some("meshtastic"), &[]);
        let ev = feed(rx.as_mut(), &x, out, dial, dial, STEP);
        assert!(
            ev.iter().any(|e| e.kind() == "mesh.node"),
            "the node info was not read from the burst at {rate}"
        );
        let snr = packets(&ev, None)
            .iter()
            .filter_map(|e| e.get(&keys::SNR_DB))
            .fold(f64::MIN, f64::max);
        assert!(snr > 26.0, "SNR {snr} dB of the burst at {rate}");
    }
}

/// Runs command `key` of `rx` with these arguments and returns its reply.
fn call(
    rx: &mut dyn Receiver,
    key: &str,
    args: &[(&str, Value)],
) -> Result<Option<Record>, CommandError> {
    let id = rx.commands().id(key)?;
    let def = rx.commands().def(id).expect("the command").clone();
    let mut a = def.new_args();
    for (k, v) in args {
        let f = a.decl().id(k).expect("an argument of the command");
        a.set_value(f, v).expect("an argument of its kind");
    }
    sdrkit_core::owner::invoke(rx, id, &a)
}

/// The own position of the LoRa APRS tests.
const HOME: (f64, f64) = (52.3702, 4.8952);

/// A LoRa APRS station N0CALL with SSID 7 on the band plan LoRa APRS EU, its capture 200 kHz above the dial, with the
/// transmit switch `on`. Returns the station, its transmit stream and the dial.
fn aprs_station(extra: &[CaseSetting], on: bool) -> (Box<dyn Receiver>, Arc<Bursts>, f64) {
    aprs_station_ssid(7, extra, on)
}

/// As [`aprs_station`], with SSID `ssid`.
fn aprs_station_ssid(
    ssid: i64,
    extra: &[CaseSetting],
    on: bool,
) -> (Box<dyn Receiver>, Arc<Bursts>, f64) {
    let d = &sdrkit_stack_lora::LORAAPRS;
    let (dial, mut s) = band_plan("loraaprs", "LoRa APRS EU");
    s.extend([
        CaseSetting::new(&APRS_SSID, ssid),
        CaseSetting::new(&DUTY_LIMIT, false),
    ]);
    s.extend(extra.iter().cloned());
    let mut a = receiver(d, None, &s);
    a.set_station(&Station {
        callsign: "N0CALL".into(),
        position: Some(Position {
            lat_deg: HOME.0,
            lon_deg: HOME.1,
            alt_m: 0.0,
        }),
        ..Default::default()
    });
    a.set_transmit(on);
    a.configure(&InputInfo::new(RATE, dial + 200e3, dial).recording())
        .unwrap();
    let b = Arc::new(Bursts::default());
    connect(a.as_mut(), &b);
    (a, b, dial)
}

/// Sends `aprs.send` to `to` with `text`. Returns the message number of the reply.
fn aprs_send(rx: &mut dyn Receiver, to: &str, text: &str) -> i64 {
    let r = call(
        rx,
        "aprs.send",
        &[
            ("to", Value::Text(to.into())),
            ("text", Value::Text(text.into())),
        ],
    )
    .expect("the message is taken")
    .expect("a reply");
    r.at(&AprsSendReply::PACKET_ID).expect("the message number")
}

/// The own message of `rx` with this text.
fn own_message<'a>(rx: &'a dyn Receiver, text: &str) -> &'a Record {
    row(rx.results(), &MESH_MESSAGES, |r| {
        r.get(&keys::TEXT) == Some(text)
    })
    .expect("the own message")
}

/// A message and a beacon of LoRa APRS leave as IQ, and a second station reads both: the message with its source,
/// addressee and number, the beacon as a station with position, symbol and comment. The sender lists its message as sent
/// and itself as a station.
#[test]
fn a_lora_aprs_message_and_beacon_leave_as_iq_and_another_station_reads_them() {
    let (mut a, b, dial) = aprs_station(
        &[
            CaseSetting::new(&APRS_SYMBOL, "/>"),
            CaseSetting::new(&APRS_COMMENT, "sdrkit test"),
        ],
        true,
    );
    let mut no = 0;
    let w = transmit(a.as_mut(), &b, 12.0, |rx| {
        no = aprs_send(rx, "n0call-9", "hello lora");
        call(rx, "aprs.beacon", &[]).expect("the beacon is taken");
    });
    assert_eq!(w.len(), 2, "two bursts");
    assert!(w.iter().all(|x| (x.1, x.2) == (RATE, dial)));
    assert!((1..=99_999).contains(&no));
    let f = a.results().fields();
    assert_eq!(f.at(&HamFields::APRS_CALL), Some("N0CALL-7"));
    assert_eq!(f.at(&HamFields::APRS_FREQ), Some(dial));
    let own = own_message(a.as_ref(), "hello lora");
    assert_eq!(own.at(&MeshMessageRow::STATE), Some(DeliveryState::Sent));
    assert_eq!(own.at(&MeshMessageRow::PACKET_ID), Some(no));
    let me = row(a.results(), &MESH_STATIONS, |r| {
        r.at(&MeshStationRow::CALL) == Some("N0CALL-7")
    })
    .expect("the own station");
    assert_eq!(me.at(&MeshStationRow::SYMBOL), Some("/>"));

    let (_, plan) = band_plan("loraaprs", "LoRa APRS EU");
    let mut rx = receiver(&sdrkit_stack_lora::LORAAPRS, None, &plan);
    hear(rx.as_mut(), &w, dial);
    let m = row(rx.results(), &MESH_MESSAGES, |r| {
        r.get(&keys::TEXT) == Some("hello lora")
    })
    .expect("the message at the second station");
    assert_eq!(m.get(&keys::SOURCE), Some("N0CALL-7"));
    assert_eq!(m.at(&MeshMessageRow::TARGET), Some("N0CALL-9"));
    assert_eq!(m.at(&MeshMessageRow::PACKET_ID), Some(no));
    let s = row(rx.results(), &MESH_STATIONS, |r| {
        r.at(&MeshStationRow::CALL) == Some("N0CALL-7")
    })
    .expect("the beacon at the second station");
    assert_eq!(s.at(&MeshStationRow::SYMBOL), Some("/>"));
    assert_eq!(s.at(&MeshStationRow::COMMENT), Some("sdrkit test"));
    let (lat, lon) = s.get(&keys::POSITION).expect("a position");
    assert!((lat - HOME.0).abs() < 0.001 && (lon - HOME.1).abs() < 0.001);
}

/// An acknowledgement of the addressee to the own call makes the own LoRa APRS message delivered.
#[test]
fn an_acknowledgement_makes_an_own_lora_aprs_message_delivered() {
    let (mut a, b, _) = aprs_station(&[], true);
    let mut no = 0;
    let w = transmit(a.as_mut(), &b, 6.0, |rx| {
        no = aprs_send(rx, "N0CALL-9", "please ack");
    });
    assert_eq!(w.len(), 1);
    let ack = hamlora::build_lora_aprs(&format!("N0CALL-9>APRS::N0CALL-7 :ack{no}"));
    let s = TxFrame::new(&hamlora::lora_aprs_params(), &ack)
        .expect("a valid frame")
        .carrier(-200e3)
        .amplitude(0.5)
        .render(RATE, 0.0);
    let mut x = vec![Complex32::default(); STEP];
    x.extend(s);
    x.extend(vec![Complex32::default(); 6 * STEP]);
    let mut out = Output::default();
    for c in x.chunks(STEP) {
        out.clear();
        a.feed(c, &mut out);
    }
    let own = own_message(a.as_ref(), "please ack");
    assert_eq!(
        own.at(&MeshMessageRow::STATE),
        Some(DeliveryState::Delivered)
    );
}

/// The LoRa APRS commands declare that they transmit, so the engine refuses them while its switch is off; a frame lasts
/// one burst, so they have no stop command. With the transmitter off a message is not sent and its row says why; without
/// a callsign the commands are refused; a malformed addressee or text is refused.
#[test]
fn lora_aprs_sends_nothing_with_the_transmitter_off_or_without_a_callsign() {
    let (mut a, b, _) = aprs_station(&[], false);
    let defs = a.commands();
    for (key, what) in [
        ("aprs.send", TxContent::Text),
        ("aprs.beacon", TxContent::Beacon),
    ] {
        let d = defs.def(defs.id(key).unwrap()).unwrap();
        assert_eq!(d.transmits, Some(what), "{key}");
        assert_eq!(d.stopped_by, None, "{key}");
    }
    let w = transmit(a.as_mut(), &b, 3.0, |rx| {
        aprs_send(rx, "N0CALL-9", "not on the air");
        call(rx, "aprs.beacon", &[]).unwrap();
    });
    assert!(w.is_empty());
    let own = own_message(a.as_ref(), "not on the air");
    assert_eq!(own.at(&MeshMessageRow::STATE), Some(DeliveryState::Failed));
    assert_eq!(own.at(&MeshMessageRow::FAILURE), Some(FailureReason::TransmitOff));
    for (to, text) in [("N0CALL-9", "a{b"), ("NOT A CALL", "x"), ("N0CALL-9", "")] {
        let r = call(
            a.as_mut(),
            "aprs.send",
            &[
                ("to", Value::Text(to.into())),
                ("text", Value::Text(text.into())),
            ],
        );
        assert!(
            matches!(r, Err(CommandError::BadArgument { .. })),
            "{to} {text}"
        );
    }
    a.set_station(&Station::default());
    let message = [
        ("to", Value::Text("N0CALL-9".into())),
        ("text", Value::Text("x".into())),
    ];
    let r = call(a.as_mut(), "aprs.send", &message);
    assert!(matches!(r, Err(CommandError::NotNow { .. })));
    let r = call(a.as_mut(), "aprs.beacon", &[]);
    assert!(matches!(r, Err(CommandError::NotNow { .. })));
}

/// The duty-cycle limit of the band that the consumer sets holds a second message until the air time of the hour allows
/// it.
#[test]
fn the_duty_cycle_limit_holds_a_second_lora_aprs_message() {
    let (dial, _) = band_plan("loraaprs", "LoRa APRS EU");
    let band = format!("{}-{}=0.1", dial - 1e6, dial + 1e6);
    let (mut a, b, _) = aprs_station(
        &[
            CaseSetting::new(&DUTY_LIMIT, true),
            CaseSetting::new(&DUTY_BANDS, band.as_str()),
        ],
        true,
    );
    let w = transmit(a.as_mut(), &b, 10.0, |rx| {
        aprs_send(rx, "N0CALL-9", "first");
        aprs_send(rx, "N0CALL-9", "second");
    });
    assert_eq!(w.len(), 1, "one burst inside the limit");
    let first = own_message(a.as_ref(), "first");
    assert_eq!(first.at(&MeshMessageRow::STATE), Some(DeliveryState::Sent));
    let second = own_message(a.as_ref(), "second");
    assert_eq!(
        second.at(&MeshMessageRow::STATE),
        Some(DeliveryState::Waiting)
    );
}

/// A LoRa APRS frame of TNC2 text `tnc2` at the dial, as a write of the host: whole, or with a flipped bit after the
/// checksum is made when `damaged`.
fn lora_aprs_write(tnc2: &str, damaged: bool, dial: f64) -> Write {
    let p = hamlora::lora_aprs_params();
    let payload = hamlora::build_lora_aprs(tnc2);
    let x = if damaged {
        damaged_burst(&p, &payload, 3, 0.0, 0.0, 0.5, 0.0, RATE)
            .expect("a damaged frame")
            .samples
    } else {
        TxFrame::new(&p, &payload)
            .expect("a valid frame")
            .amplitude(0.5)
            .render(RATE, 0.0)
    };
    (x, RATE, dial)
}

/// The addressee of a LoRa APRS message, a second station, acknowledges each good copy of a number once: not a damaged
/// copy, and not the same number again within 30 s. The acknowledgement goes on the air, the sender reads it, marks its
/// message delivered and does not send it again.
#[test]
fn the_addressee_acknowledges_a_lora_aprs_message_and_the_sender_marks_it_delivered() {
    let (mut a, ab, dial) = aprs_station(&[], true);
    let (mut z, zb, _) = aprs_station_ssid(9, &[], true);
    let centre = dial + 200e3;
    let w = transmit(a.as_mut(), &ab, 6.0, |rx| {
        aprs_send(rx, "N0CALL-9", "please ack");
    });
    assert_eq!(w.len(), 1);
    // A damaged copy of another station's message gets no acknowledgement; a good copy gets one; the same again, none.
    let other = "N0CALL-5>APRS::N0CALL-9 :hello{42";
    let (ev, none) = air_at(
        z.as_mut(),
        &zb,
        &[lora_aprs_write(other, true, dial)],
        dial,
        centre,
        5.0,
    );
    let heard = packets(&ev, None);
    assert_eq!(heard.len(), 1, "the damaged frame is heard");
    assert_eq!(heard[0].at(&LoraPacketEvent::CRC_OK), Some(false));
    assert!(none.is_empty(), "no acknowledgement of a damaged frame");
    let good = [lora_aprs_write(other, false, dial)];
    let (_, one) = air_at(z.as_mut(), &zb, &good, dial, centre, 5.0);
    assert_eq!(one.len(), 1, "one acknowledgement of the good copy");
    let (_, again) = air_at(z.as_mut(), &zb, &good, dial, centre, 5.0);
    assert!(again.is_empty(), "the same number again within 30 s");
    // The message of the sender: one acknowledgement, which the sender reads.
    let (_, acks) = air_at(z.as_mut(), &zb, &w, dial, centre, 5.0);
    assert_eq!(acks.len(), 1, "one acknowledgement");
    let (_, echo) = air_at(a.as_mut(), &ab, &acks, dial, centre, 2.0);
    assert!(echo.is_empty(), "the sender answers nothing");
    let own = own_message(a.as_ref(), "please ack");
    assert_eq!(
        own.at(&MeshMessageRow::STATE),
        Some(DeliveryState::Delivered)
    );
    // 40 s later, past the first retry time, nothing more goes out.
    let (_, later) = air_at(a.as_mut(), &ab, &[], dial, centre, 40.0);
    assert!(later.is_empty());
}

/// An unanswered LoRa APRS message goes again 30 s after its first transmission; its frame lasts more than 1 s. Under the duty-cycle limit of the band
/// the second transmission waits until the air time of the hour allows it.
#[test]
fn an_unanswered_lora_aprs_message_goes_again_after_30_s_within_the_duty_limit() {
    let (mut a, ab, _) = aprs_station(&[], true);
    let w = transmit(a.as_mut(), &ab, 34.0, |rx| {
        aprs_send(rx, "N0CALL-9", "first");
    });
    assert_eq!(w.len(), 2, "the message and its first retry");
    assert_eq!(w[0].0, w[1].0, "the same frame");
    let own = own_message(a.as_ref(), "first");
    assert_eq!(own.at(&MeshMessageRow::STATE), Some(DeliveryState::Sent));

    let (dial, _) = band_plan("loraaprs", "LoRa APRS EU");
    let band = format!("{}-{}=0.1", dial - 1e6, dial + 1e6);
    let (mut a, ab, _) = aprs_station(
        &[
            CaseSetting::new(&DUTY_LIMIT, true),
            CaseSetting::new(&DUTY_BANDS, band.as_str()),
        ],
        true,
    );
    let w = transmit(a.as_mut(), &ab, 34.0, |rx| {
        aprs_send(rx, "N0CALL-9", "first");
    });
    assert_eq!(w.len(), 1, "the retry waits for the duty limit");
    let own = own_message(a.as_ref(), "first");
    assert_eq!(own.at(&MeshMessageRow::STATE), Some(DeliveryState::Waiting));
}

// ---- The AFSK stack `aprs`: AX.25 frames on FM ----

/// The dial of the AFSK tests.
const AFSK_DIAL: f64 = 144.8e6;
/// The centre of the capture of an AFSK station, above the dial.
const AFSK_CENTRE: f64 = AFSK_DIAL + 50e3;

/// An AFSK APRS station with callsign `call` at HOME, its capture 50 kHz above the dial, with the transmit switch `on`.
/// Returns the station and its transmit stream.
fn afsk_station(call: &str, extra: &[CaseSetting], on: bool) -> (Box<dyn Receiver>, Arc<Bursts>) {
    let mut a = receiver(&sdrkit_stack_lora::APRS, None, extra);
    a.set_station(&Station {
        callsign: call.into(),
        position: Some(Position {
            lat_deg: HOME.0,
            lon_deg: HOME.1,
            alt_m: 0.0,
        }),
        ..Default::default()
    });
    a.set_transmit(on);
    a.configure(&InputInfo::new(RATE, AFSK_CENTRE, AFSK_DIAL).recording())
        .unwrap();
    let b = Arc::new(Bursts::default());
    connect(a.as_mut(), &b);
    (a, b)
}

/// Puts the bursts `w` (centred on the dial) on the air of AFSK station `rx`, each after 0.1 s of silence, then `after_s`
/// of silence. Returns the events of `rx` and the bursts it sent meanwhile.
fn air(
    rx: &mut dyn Receiver,
    b: &Arc<Bursts>,
    w: &[Write],
    after_s: f64,
) -> (Vec<sdrkit_core::Event>, Vec<Write>) {
    air_at(rx, b, w, AFSK_DIAL, AFSK_CENTRE, after_s)
}

/// As [`air`] for a station whose capture is centred on `centre`, with bursts centred on `dial`.
fn air_at(
    rx: &mut dyn Receiver,
    b: &Arc<Bursts>,
    w: &[Write],
    dial: f64,
    centre: f64,
    after_s: f64,
) -> (Vec<sdrkit_core::Event>, Vec<Write>) {
    let mut x = Vec::new();
    for (burst, rate, c) in w {
        assert_eq!((*rate, *c), (RATE, dial));
        x.extend(vec![Complex32::default(); 2 * STEP]);
        x.extend_from_slice(burst);
    }
    x.extend(vec![Complex32::default(); (after_s * RATE) as usize]);
    let step = TAU * (dial - centre) / RATE;
    for (i, v) in x.iter_mut().enumerate() {
        let (s, c) = (step * i as f64).sin_cos();
        *v *= Complex32::new(c as f32, s as f32);
    }
    let (mut out, mut ev) = (Output::default(), Vec::new());
    for c in x.chunks(STEP) {
        out.clear();
        rx.feed(c, &mut out);
        ev.append(&mut out.events);
    }
    (ev, std::mem::take(&mut *b.0.lock().unwrap()))
}

/// The information fields of the AX.25 frames from `source` among the events `ev`.
fn infos(ev: &[sdrkit_core::Event], source: &str) -> Vec<String> {
    ev.iter()
        .filter(|e| e.is(&APRS_PACKET) && e.get(&keys::SOURCE) == Some(source))
        .filter_map(|e| e.at(&AprsPacketEvent::INFO).map(str::to_string))
        .collect()
}

/// The AX.25 message of `rx` with this text, own or received.
fn own_afsk<'a>(rx: &'a dyn Receiver, text: &str) -> &'a Record {
    row(rx.results(), &MESH_MESSAGES, |r| {
        r.get(&keys::TEXT) == Some(text)
    })
    .expect("the message")
}

/// A message and a beacon of the AFSK stack leave `tx.sink` as FM bursts at the rate and centre of the session, and a
/// second station decodes both with its own receiver: the message with source, path, addressee and number, the beacon as
/// a station with position, symbol and comment. The sender lists its message as sent and itself as a station; the second
/// station lists the received message in the same table, with its sender, addressee and number.
#[test]
fn an_afsk_message_and_beacon_leave_as_fm_and_another_station_reads_them() {
    let (mut a, b) = afsk_station(
        "N0CALL",
        &[
            CaseSetting::new(&APRS_SSID, 7),
            CaseSetting::new(&APRS_SYMBOL, "/>"),
            CaseSetting::new(&APRS_COMMENT, "sdrkit test"),
        ],
        true,
    );
    let mut no = 0;
    let w = transmit(a.as_mut(), &b, 3.0, |rx| {
        no = aprs_send(rx, "n0call-9", "hello afsk");
        call(rx, "aprs.beacon", &[]).expect("the beacon is taken");
    });
    assert_eq!(w.len(), 2, "two bursts");
    assert!(w.iter().all(|x| (x.1, x.2) == (RATE, AFSK_DIAL)));
    assert!((1..=99_999).contains(&no));
    let f = a.results().fields();
    assert_eq!(f.at(&HamFields::APRS_CALL), Some("N0CALL-7"));
    assert_eq!(f.at(&HamFields::APRS_FREQ), Some(AFSK_DIAL));
    let own = own_afsk(a.as_ref(), "hello afsk");
    assert_eq!(own.at(&MeshMessageRow::STATE), Some(DeliveryState::Sent));
    assert_eq!(own.at(&MeshMessageRow::PACKET_ID), Some(no));
    assert_eq!(own.at(&MeshMessageRow::NET), Some(Protocol::Aprs));
    assert_eq!(own.at(&MeshMessageRow::SOURCE), Some("N0CALL-7"));
    assert_eq!(own.get(&keys::TARGET), Some("N0CALL-9"));
    let me = row(a.results(), &APRS_STATIONS, |r| {
        r.at(&AprsStationRow::CALL) == Some("N0CALL-7")
    })
    .expect("the own station");
    assert_eq!(me.at(&AprsStationRow::SYMBOL), Some("/>"));

    let (mut rx, rb) = afsk_station("N0CALL-5", &[], false);
    let (ev, sent) = air(rx.as_mut(), &rb, &w, 0.5);
    assert!(sent.is_empty());
    let got = infos(&ev, "N0CALL-7");
    assert!(
        got.contains(&format!(":N0CALL-9 :hello afsk{{{no}")),
        "{got:?}"
    );
    let heard = own_afsk(rx.as_ref(), "hello afsk");
    assert_eq!(heard.at(&MeshMessageRow::NET), Some(Protocol::Aprs));
    assert_eq!(heard.at(&MeshMessageRow::SOURCE), Some("N0CALL-7"));
    assert_eq!(heard.get(&keys::TARGET), Some("N0CALL-9"));
    assert_eq!(heard.at(&MeshMessageRow::PACKET_ID), Some(no));
    assert_eq!(heard.at(&MeshMessageRow::STATE), None);
    assert_eq!(heard.at(&MeshMessageRow::CRC_OK), Some(true));
    assert!(heard.at(&MeshMessageRow::LEVEL_DBFS).is_some());
    let p = ev
        .iter()
        .find(|e| e.is(&APRS_PACKET) && e.get(&keys::SOURCE) == Some("N0CALL-7"))
        .expect("a frame");
    assert_eq!(p.at(&AprsPacketEvent::PATH), Some("WIDE1-1"));
    assert_eq!(p.get(&keys::TARGET), Some("APRS"));
    let s = row(rx.results(), &APRS_STATIONS, |r| {
        r.at(&AprsStationRow::CALL) == Some("N0CALL-7")
    })
    .expect("the beacon at the second station");
    assert_eq!(s.at(&AprsStationRow::SYMBOL), Some("/>"));
    assert_eq!(s.at(&AprsStationRow::COMMENT), Some("sdrkit test"));
    let (lat, lon) = s.get(&keys::POSITION).expect("a position");
    assert!((lat - HOME.0).abs() < 0.001 && (lon - HOME.1).abs() < 0.001);
}

/// The addressee hears the message, acknowledges it on the air once, and the acknowledgement makes the own message of the
/// sender delivered. The sender does not acknowledge its own frame heard back, and a delivered message is not sent again.
#[test]
fn the_addressee_acknowledges_an_afsk_message_and_the_sender_marks_it_delivered() {
    let (mut a, ab) = afsk_station("N0CALL-1", &[], true);
    let (mut z, zb) = afsk_station("N0CALL-9", &[], true);
    let mut no = 0;
    let w = transmit(a.as_mut(), &ab, 2.0, |rx| {
        no = aprs_send(rx, "N0CALL-9", "please ack");
    });
    assert_eq!(w.len(), 1);
    let (ev, acks) = air(z.as_mut(), &zb, &w, 2.0);
    assert!(infos(&ev, "N0CALL-1").contains(&format!(":N0CALL-9 :please ack{{{no}")));
    assert_eq!(acks.len(), 1, "one acknowledgement");
    // The same message again within 30 s is not acknowledged again.
    let (_, again) = air(z.as_mut(), &zb, &w, 2.0);
    assert!(again.is_empty());
    let (ev, echo) = air(a.as_mut(), &ab, &acks, 1.0);
    assert!(echo.is_empty(), "the sender answers nothing");
    assert_eq!(infos(&ev, "N0CALL-9"), [format!(":N0CALL-1 :ack{no}")]);
    let own = own_afsk(a.as_ref(), "please ack");
    assert_eq!(
        own.at(&MeshMessageRow::STATE),
        Some(DeliveryState::Delivered)
    );
    // 40 s later, past the first retry time, nothing more goes out.
    let (_, later) = air(a.as_mut(), &ab, &[], 40.0);
    assert!(later.is_empty());
}

/// An unanswered AFSK message goes again 30 s after its first transmission.
#[test]
fn an_unanswered_afsk_message_goes_again_after_30_s() {
    let (mut a, ab) = afsk_station("N0CALL-1", &[], true);
    let w = transmit(a.as_mut(), &ab, 32.0, |rx| {
        aprs_send(rx, "N0CALL-9", "anyone");
    });
    assert_eq!(w.len(), 2, "the message and its first retry");
    assert_eq!(w[0].0, w[1].0, "the same frame");
    let own = own_afsk(a.as_ref(), "anyone");
    assert_eq!(own.at(&MeshMessageRow::STATE), Some(DeliveryState::Sent));
}

/// The AFSK commands have the names of the LoRa APRS commands and declare that they transmit. With the transmitter off a
/// message is not sent and its row says why; without a valid AX.25 call (no callsign, or an SSID in both the callsign and
/// `aprs_ssid`) the commands are refused; a malformed addressee or text is refused.
#[test]
fn afsk_aprs_sends_nothing_with_the_transmitter_off_or_without_a_valid_call() {
    let (mut a, b) = afsk_station("N0CALL", &[], false);
    let defs = a.commands();
    for (key, what) in [
        ("aprs.send", TxContent::Text),
        ("aprs.beacon", TxContent::Beacon),
    ] {
        let d = defs.def(defs.id(key).unwrap()).unwrap();
        assert_eq!(d.transmits, Some(what), "{key}");
        assert_eq!(d.stopped_by, None, "{key}");
    }
    assert!(defs.id("send").is_err() && defs.id("beacon").is_err());
    let w = transmit(a.as_mut(), &b, 2.0, |rx| {
        aprs_send(rx, "N0CALL-9", "not on the air");
        call(rx, "aprs.beacon", &[]).unwrap();
    });
    assert!(w.is_empty());
    let own = own_afsk(a.as_ref(), "not on the air");
    assert_eq!(own.at(&MeshMessageRow::STATE), Some(DeliveryState::Failed));
    assert_eq!(own.at(&MeshMessageRow::FAILURE), Some(FailureReason::TransmitOff));
    for (to, text) in [("N0CALL-9", "a{b"), ("NOT A CALL", "x"), ("N0CALL-9", "")] {
        let r = call(
            a.as_mut(),
            "aprs.send",
            &[
                ("to", Value::Text(to.into())),
                ("text", Value::Text(text.into())),
            ],
        );
        assert!(
            matches!(r, Err(CommandError::BadArgument { .. })),
            "{to} {text}"
        );
    }
    let message = [
        ("to", Value::Text("N0CALL-9".into())),
        ("text", Value::Text("x".into())),
    ];
    for station in ["", "N0CALL-1"] {
        let (mut a, _) = afsk_station(station, &[CaseSetting::new(&APRS_SSID, 7)], true);
        let r = call(a.as_mut(), "aprs.send", &message);
        let Err(CommandError::NotNow { field, .. }) = r else {
            panic!("{station}: {r:?}");
        };
        let decl = a.results().decl();
        assert_eq!(
            decl.fields().field(field).map(|f| f.key.as_ref()),
            Some("aprs_call")
        );
        let r = call(a.as_mut(), "aprs.beacon", &[]);
        assert!(matches!(r, Err(CommandError::NotNow { .. })), "{station}");
    }
}

/// A pre-emphasised AFSK frame is read by a receiver with de-emphasis, and a flat frame by a receiver without emphasis.
#[test]
fn a_pre_emphasised_afsk_frame_is_read_with_de_emphasis() {
    for (pre, emphasis) in [(true, Emphasis::De), (false, Emphasis::None)] {
        let (mut a, b) = afsk_station("N0CALL", &[CaseSetting::new(&TX_PRE_EMPHASIS, pre)], true);
        let w = transmit(a.as_mut(), &b, 2.0, |rx| {
            aprs_send(rx, "N0CALL-9", "emphasis");
        });
        assert_eq!(w.len(), 1);
        let (mut rx, rb) =
            afsk_station("N0CALL-5", &[CaseSetting::new(&EMPHASIS, emphasis)], false);
        let (ev, _) = air(rx.as_mut(), &rb, &w, 0.5);
        assert_eq!(infos(&ev, "N0CALL").len(), 1, "pre-emphasis {pre}");
    }
}

/// The transmit settings of the AFSK stack reach the burst: `tx_delay` sets the flags before the frame, `tx_pre_emphasis`
/// changes the FM, and with `tx_baud` 9600 a receiver reads the frame at 9600 bit/s. The bursts are beacons, whose frame
/// is the same in every session: a message carries a number that starts at a random value in each session, and its
/// digits and bit stuffing change the length of the burst.
#[test]
fn the_afsk_transmit_settings_reach_the_burst() {
    let burst = |extra: &[CaseSetting]| {
        let (mut a, b) = afsk_station("N0CALL", extra, true);
        let w = transmit(a.as_mut(), &b, 2.0, |rx| {
            call(rx, "aprs.beacon", &[]).expect("the beacon is taken");
        });
        assert_eq!(w.len(), 1);
        w
    };
    let short = burst(&[CaseSetting::new(&TX_DELAY, 0.1)]);
    let long = burst(&[CaseSetting::new(&TX_DELAY, 0.5)]);
    let more = (long[0].0.len() as f64 - short[0].0.len() as f64) / RATE;
    assert!((more - 0.4).abs() < 0.01, "{more} s more");
    let flat = burst(&[]);
    let pre = burst(&[CaseSetting::new(&TX_PRE_EMPHASIS, true)]);
    assert_eq!(flat[0].0.len(), pre[0].0.len());
    assert_ne!(flat[0].0, pre[0].0);
    let fast = burst(&[CaseSetting::new(&TX_BAUD, PacketSpeed::G3ruh9600)]);
    let (mut rx, rb) = afsk_station("N0CALL-5", &[], false);
    let (ev, _) = air(rx.as_mut(), &rb, &fast, 0.5);
    assert_eq!(infos(&ev, "N0CALL").len(), 1);
    let f = rx.results().fields();
    assert_eq!(
        (
            f.at(&AprsFields::FRAMES_9600),
            f.at(&AprsFields::FRAMES_1200)
        ),
        (Some(1), Some(0))
    );
}
