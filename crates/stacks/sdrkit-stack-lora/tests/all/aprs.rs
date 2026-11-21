//! APRS messaging and the station tables: messages with acknowledgements and retries, digipeated copies, objects and
//! beacons; and the AFSK modulator alone: its tones, phase, bit timing and a frame through noise.

use sdrkit_algo_sim::noise::Awgn;
use sdrkit_core::{Complex32, Receiver};
use sdrkit_proto_link::ax25::Frame;
use sdrkit_proto_link::modem::Afsk1200;
use sdrkit_stack_lora::aprs::rx::{AprsRx, RawFrame, RxConfig};
use sdrkit_stack_lora::aprs::station::{position_text, AprsTables};
use sdrkit_stack_lora::components::modulate::{packet_audio, packet_iq, DEVIATION_HZ};
use sdrkit_stack_lora::mesh::ham_client::{ax25_ui, HamClient, HamConfig, HamHeard, HamLink};
use sdrkit_stack_lora::mesh::types::{AfskTx, Delivery, Failure, TxChannel, TxKind};
use sdrkit_stack_lora::proto::hamlora;
use sdrkit_stack_lora::typed::{AprsFields, APRS_FRAMES};
use sdrkit_util_geo::Position;
use std::f64::consts::TAU;

fn heard(bytes: &[u8], time: f64) -> RawFrame {
    RawFrame {
        bytes: bytes.to_vec(),
        speed: 1200,
        time,
    }
}

fn ui(from: &str, info: &str) -> Vec<u8> {
    sdrkit_stack_lora::gen::ui_frame(from, "APRS", &[], false, info.as_bytes())
}

fn afsk_client(call: &str) -> HamClient {
    HamClient::new(
        HamConfig {
            tx_enabled: true,
            link: Some(HamLink::Afsk { freq_hz: 144.8e6 }),
            callsign: call.into(),
            ..Default::default()
        },
        1,
    )
}

fn tables(call: &str) -> AprsTables {
    AprsTables {
        own_call: call.into(),
        ..Default::default()
    }
}

/// A message on AX.25 is numbered; the addressee's tables pass it to its client, which acknowledges it once in 30 s; the
/// sender's tables pass the acknowledgement to its client, which marks the message delivered and sends it no more.
/// Without an answer it goes again 30, 60, 120 and 240 s after each transmission, five times in all, then fails (APRS
/// 1.0.1 chapter 14). On LoRa APRS a message goes again at the same times, as the same frame.
#[test]
fn aprs_messages_are_acknowledged_and_retried() {
    let (mut a, mut ta) = (afsk_client("N0CALL-1"), tables("N0CALL-1"));
    let (mut b, mut tb) = (afsk_client("N0CALL-9"), tables("N0CALL-9"));
    let n = a.next_number();
    let o = a.send_message(0.0, "N0CALL-9", "Test message", n);
    assert_eq!(o.tx.len(), 1);
    assert_eq!(o.messages[0].state, Delivery::Queued);
    let o = a.on_sent(0.0, o.tx[0].message_key).messages;
    assert_eq!((o[0].state, o[0].sent), (Delivery::Sent, 1));
    let sent = Frame::parse(&a_frame(&mut a, n)).unwrap().info.to_vec();
    assert_eq!(sent, format!(":N0CALL-9 :Test message{{{n}").into_bytes());
    let (mut h, mut ev) = (Vec::new(), Vec::new());
    let frame = ax25_ui(
        "N0CALL-1",
        "APRS",
        "WIDE1-1",
        &format!(":N0CALL-9 :Test message{{{n}"),
    )
    .unwrap();
    tb.frame(&heard(&frame, 1.0), &mut h, &mut ev);
    let ask = HamHeard::Message {
        time: 1.0,
        from: "N0CALL-1".into(),
        number: n.to_string(),
    };
    assert_eq!(h, std::slice::from_ref(&ask));
    let ack = b.heard(1.0, &ask).tx;
    assert_eq!(ack.len(), 1);
    assert_eq!(ack[0].what, TxKind::Ack);
    let f = Frame::parse(&ack[0].payload).unwrap();
    assert_eq!(
        String::from_utf8_lossy(f.info),
        format!(":N0CALL-1 :ack{n}")
    );
    assert_eq!(f.source().to_string(), "N0CALL-9");
    // The same message again within 30 s is not acknowledged twice.
    assert!(b.heard(2.0, &ask).tx.is_empty());
    let mut h = Vec::new();
    ta.frame(&heard(&ack[0].payload, 2.0), &mut h, &mut ev);
    let answer = HamHeard::Answer {
        from: "N0CALL-9".into(),
        number: n,
        state: Delivery::Delivered,
    };
    assert_eq!(h, std::slice::from_ref(&answer));
    let o = a.heard(2.0, &answer).messages;
    assert_eq!(o[0].state, Delivery::Delivered);
    assert!((0..1000).all(|s| a.tick(f64::from(s), true).tx.is_empty()));

    let mut c = afsk_client("N0CALL-1");
    let n = c.next_number();
    let key = c.send_message(0.0, "N0CALL-5", "hello", n).tx[0].message_key;
    c.on_sent(0.0, key);
    let (mut times, mut last) = (vec![0], None);
    for s in 1..1000 {
        let o = c.tick(f64::from(s), true);
        if !o.tx.is_empty() {
            times.push(s);
            c.on_sent(f64::from(s), key);
        }
        if let Some(m) = o.messages.last() {
            last = Some((s, m.state, m.sent));
        }
    }
    assert_eq!(times, vec![0, 30, 90, 210, 450]);
    assert_eq!(last, Some((930, Delivery::Failed(Failure::NoAck), 5)));

    let mut l = afsk_client("N0CALL-1");
    l.cfg.link = Some(HamLink::Lora(TxChannel {
        params: hamlora::lora_aprs_params(),
        freq_hz: 433.775e6,
    }));
    let first = l.send_message(0.0, "N0CALL-5", "again", 1).tx.remove(0);
    l.on_sent(0.0, first.message_key);
    let mut times = vec![0];
    for s in 1..1000 {
        let o = l.tick(f64::from(s), true);
        if let Some(r) = o.tx.first() {
            assert_eq!((&r.payload, r.lora.is_some()), (&first.payload, true));
            times.push(s);
            l.on_sent(f64::from(s), first.message_key);
        }
    }
    assert_eq!(times, vec![0, 30, 90, 210, 450]);
}

/// An acknowledgement proves delivery whenever it comes: a message that failed after its fifth transmission turns to
/// delivered when its addressee answers later. A message that never went on the air stays failed.
#[test]
fn an_acknowledgement_after_the_last_transmission_still_delivers() {
    let mut c = afsk_client("N0CALL-1");
    let n = c.next_number();
    let key = c.send_message(0.0, "N0CALL-5", "late", n).tx[0].message_key;
    c.on_sent(0.0, key);
    let mut last = None;
    for s in 1..1000 {
        let o = c.tick(f64::from(s), true);
        if !o.tx.is_empty() {
            c.on_sent(f64::from(s), key);
        }
        last = o.messages.last().map(|m| m.state).or(last);
    }
    assert_eq!(last, Some(Delivery::Failed(Failure::NoAck)));
    let answer = |number| HamHeard::Answer {
        from: "N0CALL-5".into(),
        number,
        state: Delivery::Delivered,
    };
    let o = c.heard(1500.0, &answer(n)).messages;
    assert_eq!(o.len(), 1);
    assert_eq!(
        (Some(o[0].key), o[0].state, o[0].sent),
        (key, Delivery::Delivered, 5)
    );
    // The answer ends the message: a second copy changes nothing.
    assert!(c.heard(1501.0, &answer(n)).messages.is_empty());

    let mut off = afsk_client("N0CALL-1");
    off.cfg.tx_enabled = false;
    let n = off.next_number();
    let o = off.send_message(0.0, "N0CALL-5", "never sent", n);
    assert_eq!(o.messages[0].state, Delivery::Failed(Failure::TransmitOff));
    assert!(off.heard(10.0, &answer(n)).messages.is_empty());
}

/// The frame of the next transmission of message `n` of `c`: sent again by a tick past its retry time.
fn a_frame(c: &mut HamClient, n: u32) -> Vec<u8> {
    let mut c = c.clone();
    let o = c.tick(1e6, true);
    assert_eq!(o.tx.len(), 1, "message {n}");
    o.tx[0].payload.clone()
}

/// A second copy of a frame within 30 s is a digipeated copy. An object is stored under its own name, sent by its station.
#[test]
fn digipeated_copies_and_objects() {
    let mut t = AprsTables::default();
    let (mut tx, mut ev) = (Vec::new(), Vec::new());
    let f = ui("N0CALL", ";LEADER   *092345z4903.50N/07201.75W>088/036");
    t.frame(&heard(&f, 1.0), &mut tx, &mut ev);
    let r = t.frame(&heard(&f, 2.0), &mut tx, &mut ev).unwrap();
    assert!(
        r.summary.starts_with("(again, via direct)"),
        "{}",
        r.summary
    );
    let o = t.heard.stations.get("LEADER").unwrap();
    assert!(o.object);
    assert_eq!(o.via, "N0CALL");
    assert_eq!(t.heard.aprs_frames, 1);
    assert_eq!(t.heard.copies_count, 1);
    // A third-party packet updates the inner station through its gateway.
    let f = ui("N0GATE", "}N0INNER>APRS,TCPIP*:!4903.50N/07201.75W-inner");
    t.frame(&heard(&f, 3.0), &mut tx, &mut ev);
    let s = t.heard.stations.get("N0INNER").expect("the inner station");
    assert_eq!(s.via, "N0GATE");
    assert!(s.position.is_some());
}

/// The beacon position is rounded to hundredths of a minute, with the carry (59.999' becomes the next degree).
#[test]
fn the_beacon_position_rounds_its_minutes() {
    let p = Position::new(52.370167, 4.895167);
    assert_eq!(
        position_text(&p, "/>", "hi").unwrap(),
        "!5222.21N/00453.71E>hi"
    );
    let p = Position::new(10.999999, -0.5);
    assert_eq!(&position_text(&p, "/-", "").unwrap()[1..9], "1100.00N");
}

/// A change of frequency keeps the frame log, the stations and the counts, and drops the signal to noise ratio and the
/// level of the present signal.
#[test]
fn a_change_of_frequency_keeps_the_aprs_stations_and_drops_the_levels() {
    use sdrkit_core::interface::ReadFields;
    use sdrkit_dev_test::Harness;
    use sdrkit_stack_lora::typed::APRS_STATIONS;
    let d = &sdrkit_stack_lora::APRS;
    let (_, mut rx) = crate::common::gen_run(&Harness::new(d).seconds(12.0), d, 65536);
    let read = |rx: &dyn Receiver| {
        let f = AprsFields::read(rx.results().fields()).expect("the fields of aprs");
        (f.frames_ok, f.stations, f.snr_db, f.level_dbfs)
    };
    let counts = |rx: &dyn Receiver| {
        let r = rx.results();
        (
            crate::common::rows(r, &APRS_FRAMES).len(),
            crate::common::rows(r, &APRS_STATIONS).len(),
        )
    };
    let before = counts(rx.as_ref());
    let (good, stations, snr, level) = read(rx.as_ref());
    assert!(good >= 3 && stations >= 2 && before.0 >= 3 && before.1 >= 2);
    assert!(snr.is_some() && level.is_some());
    rx.forget();
    assert_eq!(counts(rx.as_ref()), before);
    assert_eq!(read(rx.as_ref()), (good, stations, None, None));
}

// ---- The AFSK modulator alone ----

/// The audio rate of the modulator tests: 40 samples a bit.
const AUDIO: f64 = 48_000.0;

/// The frame of the modulator tests (bytes without the FCS).
fn test_frame() -> Vec<u8> {
    ax25_ui(
        "N0CALL-7",
        "APRS",
        "WIDE1-1",
        ":N0CALL-9 :modulator test{12",
    )
    .unwrap()
}

/// The power of `w` at `hz`.
fn tone_power(w: &[f32], hz: f64) -> f64 {
    let (mut re, mut im) = (0.0, 0.0);
    for (i, &x) in w.iter().enumerate() {
        let (s, c) = (TAU * hz * i as f64 / AUDIO).sin_cos();
        re += f64::from(x) * c;
        im += f64::from(x) * s;
    }
    re * re + im * im
}

/// The tones are 1200 Hz (mark) and 2200 Hz (space): the periods between rising zero crossings inside a run of one tone
/// are 1/1200 or 1/2200 s.
#[test]
fn the_afsk_modulator_sends_1200_and_2200_hz() {
    let a = packet_audio(&test_frame(), &AfskTx::default(), AUDIO).unwrap();
    let rising: Vec<f64> = a
        .windows(2)
        .enumerate()
        .filter(|(_, w)| w[0] < 0.0 && w[1] >= 0.0)
        .map(|(i, w)| i as f64 + f64::from(w[0] / (w[0] - w[1])))
        .collect();
    let periods: Vec<f64> = rising.windows(2).map(|r| (r[1] - r[0]) / AUDIO).collect();
    for hz in [1200.0, 2200.0] {
        let near: Vec<f64> = periods
            .iter()
            .copied()
            .filter(|p| (p * hz - 1.0).abs() < 0.005)
            .collect();
        assert!(near.len() > 100, "{hz} Hz: {} periods", near.len());
        let mean = near.iter().sum::<f64>() / near.len() as f64;
        assert!(
            (1.0 / mean - hz).abs() < 1.0,
            "{hz} Hz: mean {:.2} Hz",
            1.0 / mean
        );
    }
}

/// The phase is continuous: no step between two samples is larger than a 2200 Hz sine of peak 1 makes, in the audio and
/// in the FM, whose peak deviation is 3 kHz.
#[test]
fn the_afsk_modulator_keeps_its_phase_continuous() {
    let f = test_frame();
    let a = packet_audio(&f, &AfskTx::default(), AUDIO).unwrap();
    let step = a
        .windows(2)
        .map(|w| (w[1] - w[0]).abs())
        .fold(0.0f32, f32::max);
    assert!(
        f64::from(step) <= TAU * 2200.0 / AUDIO * 1.001,
        "step {step}"
    );
    let rate = 240e3;
    for pre in [false, true] {
        let tx = AfskTx {
            pre_emphasis: pre,
            ..Default::default()
        };
        let iq = packet_iq(&f, &tx, DEVIATION_HZ, rate).unwrap();
        assert!(iq.iter().all(|v| (v.norm() - 1.0).abs() < 1e-3));
        let dev = iq
            .windows(2)
            .map(|w| f64::from((w[1] * w[0].conj()).arg()).abs() * rate / TAU)
            .fold(0.0, f64::max);
        assert!(
            (2900.0..=3001.0).contains(&dev),
            "pre-emphasis {pre}: peak deviation {dev:.0} Hz"
        );
    }
}

/// Each bit lasts 1/1200 s and holds one tone; NRZI turns the tones back into the bits, which start with the flags of
/// the TX delay (0x7E, sent from the least significant bit), as many as 0.3 s takes.
#[test]
fn the_afsk_modulator_keys_at_1200_bit_s_with_the_flags_of_the_tx_delay() {
    let tx = AfskTx::default();
    assert_eq!(tx.flags(), 45);
    let a = packet_audio(&test_frame(), &tx, AUDIO).unwrap();
    let mark: Vec<bool> = a
        .as_chunks::<40>()
        .0
        .iter()
        .map(|w| tone_power(w, 1200.0) > tone_power(w, 2200.0))
        .collect();
    let bits: Vec<u8> = mark.windows(2).map(|m| u8::from(m[0] == m[1])).collect();
    let flag = [0u8, 1, 1, 1, 1, 1, 1, 0];
    // The first bit has no tone before it: the flags follow from its end.
    let flags = bits[7..7 + 44 * 8].chunks(8).filter(|c| *c == flag).count();
    assert_eq!(flags, 44);
    // The air time of the scheduler is an upper bound: it counts the most bit stuffing.
    let (air, len) = (a.len() as f64 / AUDIO, test_frame().len());
    let least = (45 * 8 + (len + 2) * 8) as f64 / 1200.0;
    assert!(least < air && air <= tx.air_s(len), "air time {air}");
}

/// A flat frame survives noise 12 dB and 10 dB below the audio over the whole 24 kHz band, the level that the receiver's
/// modem tests count (about 10 dB more in the band of the tones).
#[test]
fn an_afsk_frame_survives_noise_12_db_below_the_audio() {
    let f = test_frame();
    for (snr_db, seed) in [(12.0, 1), (10.0, 3)] {
        let mut a = packet_audio(&f, &AfskTx::default(), AUDIO).unwrap();
        a.extend(vec![0.0; 4800]);
        // The audio has peak 1, so mean power 0.5.
        Awgn::with_power(0.5 / 10f64.powf(snr_db / 10.0), seed)
            .unwrap()
            .process_real(&mut a);
        let mut rx = Afsk1200::new(AUDIO).unwrap();
        let mut got = Vec::new();
        rx.process(&a, |x| got.push(x.bytes.to_vec()));
        assert_eq!(got, std::slice::from_ref(&f), "{snr_db} dB");
    }
}

/// The FM of a frame survives a carrier 12 dB above the noise in its Carson bandwidth (2 (3 + 2.2) kHz) through the FM
/// front end of the stack: flat with no emphasis in the receiver, and pre-emphasised with its de-emphasis.
#[test]
fn an_afsk_frame_on_fm_survives_a_carrier_12_db_above_the_noise() {
    let (f, rate) = (test_frame(), 240e3);
    for (pre, emphasis, seed) in [(false, 0, 5), (true, 1, 6)] {
        let tx = AfskTx {
            pre_emphasis: pre,
            ..Default::default()
        };
        let mut x = vec![Complex32::default(); 24_000];
        x.extend(packet_iq(&f, &tx, DEVIATION_HZ, rate).unwrap());
        x.extend(vec![Complex32::default(); 24_000]);
        let carson = 2.0 * (DEVIATION_HZ + 2200.0);
        Awgn::with_power(rate / carson / 10f64.powf(1.2), seed)
            .unwrap()
            .process(&mut x);
        let cfg = RxConfig {
            emphasis,
            ..Default::default()
        };
        let mut rx = AprsRx::new(rate, cfg).unwrap();
        let mut got = Vec::new();
        for c in x.chunks(4800) {
            rx.process(c, &mut got);
        }
        let got: Vec<Vec<u8>> = got.into_iter().map(|r| r.bytes).collect();
        assert_eq!(got, std::slice::from_ref(&f), "pre-emphasis {pre}");
    }
}
