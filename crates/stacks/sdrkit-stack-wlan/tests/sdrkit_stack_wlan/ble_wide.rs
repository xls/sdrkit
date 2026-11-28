//! The data channels of the `ble` stack: a connection followed from its start on every channel inside the capture, a
//! connection found by its packets, the estimates of its interval and channel selection, the counters of each channel,
//! and what the capture holds, on the busy scene of the generator.

use super::help::{cnum, config, feed, impaired, num, rows, snr, text, value};
use sdrkit_core::{Complex32, InputInfo, Owner, Receiver, Results, SettingSet, Value};
use sdrkit_graph::{Decoder, Format, Kind, Outbox, Pool, RunContext};
use sdrkit_proto_wlan_phy::ble::{access_address_ok, BlindPacket, Phy};
use sdrkit_stack_wlan::ble::blind::{interval_units, selection, Blind, Outcome, CONFIRM_PACKETS};
use sdrkit_stack_wlan::ble::conn::conn_key;
use sdrkit_stack_wlan::ble::scene::{BusyScene, SceneConn, SceneGenerator};
use sdrkit_stack_wlan::ble::{self, AdvDecoder, AdvStats, Advert, BleSetting, LinkNote};
use sdrkit_stack_wlan::typed::{BleChannelRow as Ch, BleConnectionRow as C};

fn rx() -> sdrkit_graph::StackGraph {
    ble::receiver(&config(BleSetting::schema(), &[])).unwrap()
}

/// `seconds` of scene `s` at 25 dB.
fn signal(s: &BusyScene, seconds: f64) -> Vec<Complex32> {
    impaired(Box::new(SceneGenerator::new(s, 7)), &snr(25.0), 3, seconds)
}

/// The scene `s` decoded as a recording.
fn decoded(s: &BusyScene, seconds: f64) -> sdrkit_graph::StackGraph {
    let x = signal(s, seconds);
    let mut r = rx();
    r.configure(&InputInfo::new(s.rate_hz, s.centre_hz, s.centre_hz).recording()).unwrap();
    feed(&mut r, &x, 65536);
    r
}

/// The packets of `c` in `seconds` on the channels inside `capture` (bit `c` for channel `c`), and those channels.
fn inside(c: &SceneConn, first_event_s: f64, seconds: f64, capture: u64) -> (i64, u64) {
    let (mut n, mut seen) = (0, 0u64);
    for k in 0..u16::MAX {
        // The last event that counts ends well before the end of the signal.
        if first_event_s + f64::from(k) * f64::from(c.interval) * 1.25e-3 > seconds - 5e-3 {
            break;
        }
        let ch = c.channel(k);
        if capture >> ch & 1 == 1 {
            n += 2;
            seen |= 1 << ch;
        }
    }
    (n, seen)
}

fn capture_of(rate: f64, centre: f64) -> u64 {
    ble::bank::channels_in_capture(centre, rate).iter().fold(0, |m, c| m | 1 << c.0)
}

#[test]
fn the_capture_says_which_channels_it_holds() {
    // Centre 2440 MHz: data channels only. Centre 2412 MHz at 24 Msps: channel 37 and data channels 0 to 9.
    for (rate, centre, adv, data) in [
        (8e6, 2440e6, 0, 3),
        (12.5e6, 2440e6, 0, 5),
        (16e6, 2440e6, 0, 7),
        (20e6, 2440e6, 0, 9),
        (24e6, 2412e6, 1, 10),
        (40e6, 2414e6, 2, 13),
    ] {
        let c = capture_of(rate, centre);
        assert_eq!(((c >> 37).count_ones(), (c & ((1 << 37) - 1)).count_ones()), (adv, data), "{rate} {centre}");
    }
    assert_eq!(
        ble::capture_text(capture_of(20e6, 2440e6), 20e6),
        "9 of 40 LE channels inside the capture at 20 Msps: 0 advertising, 9 data"
    );
    assert_eq!(
        ble::capture_text(capture_of(12.5e6, 2440e6), 12.5e6),
        "5 of 40 LE channels inside the capture at 12.5 Msps: 0 advertising, 5 data"
    );
    let mut r = rx();
    r.configure(&InputInfo::new(12.5e6, 2440e6, 2440e6).recording()).unwrap();
    feed(&mut r, &vec![Complex32::default(); 2_000_000], 65536);
    let res = Owner::results(&r);
    assert_eq!((num(res, "channels_advertising"), num(res, "channels_data")), (Some(0.0), Some(5.0)));
    let note = "5 of 40 LE channels inside the capture at 12.5 Msps: 0 advertising, 5 data";
    assert!(matches!(value(res, "capture_note"), Some(Value::Text(t)) if t == note));
    assert!(matches!(value(res, "quiet_reason"), Some(Value::Text(t)) if t == ble::NO_ADVERTISING_CHANNEL_WIDE));
    let s = &res.status().summary;
    assert!(s.contains(note) && s.contains(ble::NO_ADVERTISING_CHANNEL_WIDE), "{s}");
    let ch = rows(res, "channels");
    let list: Vec<i64> = ch.iter().map(|r| cnum(r, &Ch::CHANNEL) as i64).collect();
    assert_eq!(list, [15, 16, 17, 18, 19]);
    assert!(ch.iter().all(|r| r.at(&Ch::RECEIVED) == Some(true) && r.at(&Ch::ADVERTISING) == Some(false)));
    assert_eq!(cnum(ch[2], &Ch::FREQUENCY_HZ), 2440e6);
}

#[test]
fn a_heard_connection_is_followed_on_every_channel_inside_with_no_bad_crc() {
    let (rate, centre, seconds) = (24e6, 2412e6, 0.9);
    let capture = capture_of(rate, centre);
    for (hop, two_m) in [(Some(7), false), (None, false), (Some(11), true)] {
        let mut c = SceneConn::new(0x5A3C_9617, 0x0A_BCDE, 8, hop, 0.021);
        c.start = true;
        c.two_m = two_m;
        let s = BusyScene {
            rate_hz: rate,
            centre_hz: centre,
            advertisers: 4,
            conns: vec![c],
        };
        let r = decoded(&s, seconds);
        let res = Owner::results(&r);
        let conns = rows(res, "connections");
        assert_eq!(conns.len(), 1, "{hop:?} {two_m}");
        let row = conns[0];
        // CONNECT_IND: 44 bytes on the air, then 1.25 ms, WinOffset 1.25 ms and 0.3 ms into the window.
        let first = 0.021 + 44.0 * 8e-6 + 2.5e-3 + 0.3e-3;
        let (packets, seen) = inside(&c, first, seconds, capture);
        assert!(packets >= 30, "{packets}");
        assert_eq!(text(row, &C::ACCESS_ADDRESS), conn_key(c.aa, false));
        assert_eq!(text(row, &C::ACCESS_ADDRESS).len(), 6);
        assert_eq!(row.at(&C::START_HEARD), Some(true));
        assert_eq!(cnum(row, &C::PACKETS) as i64, packets, "{hop:?} {two_m}");
        assert_eq!(cnum(row, &C::BAD_CRC), 0.0);
        assert_eq!(cnum(row, &C::CHANNELS_SEEN) as u32, seen.count_ones());
        assert_eq!((cnum(row, &C::CHANNELS_USED), cnum(row, &C::CHANNELS_INSIDE)), (37.0, 10.0));
        assert_eq!(cnum(row, &C::INTERVAL_MS), 10.0);
        assert_eq!(cnum(row, &C::CSA), if hop.is_some() { 1.0 } else { 2.0 });
        assert_eq!(row.at(&C::HOP), hop.map(i64::from));
        assert_eq!(text(row, &C::PHY), if two_m { "LE 2M" } else { "LE 1M" });
        assert!(cnum(row, &C::EMPTY_PDUS) > 50.0);
        assert_eq!(num(res, "connections_found"), Some(0.0));
        // The devices of the advertisers show as before; the channels table counts the packets of each channel.
        // The four advertisers and the two ends of the connection.
        assert!(rows(res, "devices").len() >= 4);
        let ch = rows(res, "channels");
        assert_eq!(ch.len(), 11);
        let data: f64 = ch.iter().filter(|r| r.at(&Ch::ADVERTISING) == Some(false)).map(|r| cnum(r, &Ch::PACKETS)).sum();
        assert_eq!(data as i64, packets);
        assert!(ch.iter().all(|r| cnum(r, &Ch::BAD_CRC) == 0.0));
    }
}

#[test]
fn connections_whose_start_was_not_heard_are_found_with_interval_and_channel_selection() {
    // 20 Msps centred on 2440 MHz: data channels 13 to 21, no advertising channel.
    let (rate, centre, seconds) = (20e6, 2440e6, 2.5);
    let capture = capture_of(rate, centre);
    let one = SceneConn::new(0xAF9A_B4D1, 0x12_3456, 16, Some(13), 0.0017);
    let mut two = SceneConn::new(0x7193_A2C5, 0x5D_1E07, 12, None, 0.0041);
    two.two_m = true;
    // hop 5 puts the LL_START_ENC_REQ (event 3) on a channel inside this capture, so the encryption is seen.
    let mut enc = SceneConn::new(0x2B65_D8E3, 0xC3_5A11, 8, Some(5), 0.0127);
    enc.encrypted = true;
    enc.data_every = 2;
    assert!([one, two, enc].iter().all(|c| access_address_ok(c.aa)));
    let s = BusyScene {
        rate_hz: rate,
        centre_hz: centre,
        advertisers: 6,
        conns: vec![one, two, enc],
    };
    let r = decoded(&s, seconds);
    let res = Owner::results(&r);
    let conns = rows(res, "connections");
    assert_eq!(conns.len(), 3);
    assert_eq!(num(res, "connections_found"), Some(3.0));
    assert_eq!(rows(res, "devices").len(), 0);
    for (c, interval_ms, csa, phy, hop) in [
        (one, 20.0, 1.0, "LE 1M", Some(13)),
        (two, 15.0, 2.0, "LE 2M", None),
        (enc, 10.0, 1.0, "LE 1M", Some(5)),
    ] {
        let key = conn_key(c.aa, false);
        let row = conns.iter().find(|r| text(r, &C::ACCESS_ADDRESS) == key).expect("the connection");
        let (packets, seen) = inside(&c, c.at_s, seconds, capture);
        // Most inside packets are heard: the three connections hop independently and now and then collide on a channel,
        // losing both packets, and the channel filter rolls off at the edge of the capture.
        let got = cnum(row, &C::PACKETS) as i64;
        assert!(got >= packets * 60 / 100 && got <= packets, "{key}: {got} of {packets}");
        // A collision corrupts a packet now and then while its access address still matches: a few bad CRC are fine, but
        // most packets are good.
        assert!(cnum(row, &C::BAD_CRC) <= got as f64 * 0.15, "{key}: {} bad of {got}", cnum(row, &C::BAD_CRC));
        assert_eq!(row.at(&C::START_HEARD), Some(false));
        // Each channel seen is one of its inside channels.
        assert!(cnum(row, &C::CHANNELS_SEEN) as u32 <= seen.count_ones());
        assert!(cnum(row, &C::CHANNELS_SEEN) >= 4.0, "{key}");
        assert_eq!(row.at(&C::CHANNELS_INSIDE), None::<i64>);
        assert_eq!(cnum(row, &C::INTERVAL_MS), interval_ms, "{key}");
        assert_eq!(cnum(row, &C::CSA), csa, "{key}");
        assert_eq!(row.at(&C::HOP), hop.map(i64::from));
        assert_eq!(text(row, &C::PHY), phy);
        assert_eq!(row.at(&C::ENCRYPTED), Some(c.encrypted), "{key}");
        assert_eq!(text(row, &C::CENTRAL), "");
    }
    assert!(rows(res, "att").is_empty());
}

fn packet(aa: u32, crc_init: u32, channel: u8, llid: u8) -> BlindPacket {
    BlindPacket {
        access_address: aa,
        crc_init,
        header: [llid, 0],
        opcode: None,
        channel,
        sample: 0,
        phy: Phy::Le1M,
        level_dbfs: -20.0,
        snr_db: 20.0,
    }
}

#[test]
fn a_link_is_found_by_the_second_packet_that_agrees_on_the_crc_preset() {
    assert_eq!(CONFIRM_PACKETS, 2);
    let mut b = Blind::default();
    // Packets that never agree are no link, however many.
    for i in 0..20u32 {
        assert_eq!(b.take(f64::from(i) * 0.01, &packet(0x5065_17A3, 0x1000 + i, 3, 1)), Outcome::Unknown);
    }
    assert!(b.links().is_empty());
    assert_eq!(b.take(1.0, &packet(0xAF9A_B4D1, 0x12_3456, 3, 1)), Outcome::Unknown);
    // The same packet again does not tell: a packet read a few bits off agrees with its own copy.
    assert_eq!(b.take(1.0003, &packet(0xAF9A_B4D1, 0x12_3456, 3, 1)), Outcome::Unknown);
    assert_eq!(b.take(1.02, &packet(0xAF9A_B4D1, 0x12_3456, 9, 5)), Outcome::Good { first: true });
    assert_eq!(b.link(0xAF9A_B4D1).map(|l| l.before), Some(2));
    assert_eq!(b.take(1.04, &packet(0xAF9A_B4D1, 0x12_3456, 9, 1)), Outcome::Good { first: false });
    assert_eq!(b.take(1.0403, &packet(0xAF9A_B4D1, 0x65_4321, 9, 1)), Outcome::Bad);
    // A link that sends LLID 3 only is a periodic train whose SyncInfo was not heard: it is not listed.
    for i in 0..6u32 {
        let mut p = packet(0x7193_A2C5, 0x77, 5, 3);
        p.header[1] = i as u8;
        assert_eq!(b.take(2.0 + f64::from(i) * 0.1, &p), Outcome::Unknown);
    }
    assert_eq!(b.links().iter().filter(|l| l.connection).count(), 1);
}

#[test]
fn the_interval_and_the_channel_selection_come_from_the_events() {
    // The events of a connection on the channels inside a capture, with one event in ten taken 0.23 ms late (the
    // central's packet missed, the peripheral's taken as the first).
    let events = |c: &SceneConn, inside: std::ops::RangeInclusive<u8>, n: usize| -> Vec<(f64, u8)> {
        (0..u16::MAX)
            .map(|k| (0.5 + f64::from(k) * f64::from(c.interval) * 1.25e-3, c.channel(k)))
            .filter(|e| inside.contains(&e.1))
            .enumerate()
            .map(|(i, (t, ch))| (if i % 10 == 9 { t + 0.23e-3 } else { t }, ch))
            .take(n)
            .collect()
    };
    for (interval, hop) in [(6u16, Some(5u8)), (16, Some(13)), (80, Some(16)), (12, None), (40, None)] {
        let c = SceneConn::new(0xAF9A_B4D1, 0x12_3456, interval, hop, 0.0);
        let e = events(&c, 15..=19, 24);
        assert_eq!(interval_units(&e), Some(u32::from(interval)), "{interval} {hop:?}");
        let want = if hop.is_some() { (1, hop) } else { (2, None) };
        assert_eq!(selection(&e, u32::from(interval), c.aa), Some(want), "{interval} {hop:?}");
        assert_eq!(interval_units(&e[..4]), None);
    }
    // One channel inside, algorithm #1 over all channels: the channel comes back every 37 events.
    let c = SceneConn::new(0xAF9A_B4D1, 0x12_3456, 8, Some(7), 0.0);
    let e = events(&c, 17..=17, 8);
    assert_eq!(interval_units(&e), Some(8));
    assert_eq!(selection(&e, 8, c.aa), None);
    // A reduced channel map fits neither algorithm as tried: the selection stays unknown.
    let mut r = SceneConn::new(0xAF9A_B4D1, 0x12_3456, 8, Some(7), 0.0);
    r.map = [0x00, 0xF0, 0xFF, 0x00, 0x00];
    let e = events(&r, 15..=19, 24);
    assert_eq!(interval_units(&e), Some(8));
    assert_eq!(selection(&e, 8, r.aa), None);
}

#[test]
fn noise_alone_lists_no_connection() {
    let (rate, centre) = (12.5e6, 2440e6);
    let s = BusyScene {
        rate_hz: rate,
        centre_hz: centre,
        advertisers: 0,
        conns: Vec::new(),
    };
    let x = impaired(Box::new(SceneGenerator::new(&s, 1)), &snr(0.0), 5, 1.0);
    let mut r = rx();
    r.configure(&InputInfo::new(rate, centre, centre).recording()).unwrap();
    feed(&mut r, &x, 65536);
    let res = Owner::results(&r);
    assert!(rows(res, "connections").is_empty() && rows(res, "devices").is_empty());
    assert_eq!(num(res, "connection_packets"), Some(0.0));
}

/// Everything the decoder puts out for `x`, with `helpers` helper threads, as a recording.
fn outputs(x: &[Complex32], rate: f64, centre: f64, helpers: usize) -> (Vec<Advert>, Vec<LinkNote>, AdvStats) {
    let mut dec = AdvDecoder::wide();
    dec.attach(&RunContext {
        pool: Pool::new(helpers),
        ..RunContext::default()
    });
    dec.configure(&Format::iq(rate, centre, centre)).unwrap();
    let (mut adverts, mut notes, mut stats) = (Vec::new(), Vec::new(), AdvStats::default());
    let mut first = 0u64;
    for p in x.chunks(65_536) {
        let mut out = Outbox::new(&[Kind::typed::<Advert>(), Kind::typed::<AdvStats>(), Kind::typed::<LinkNote>()]);
        dec.process(p, first, &mut out);
        first += p.len() as u64;
        adverts.extend(out.items::<Advert>(0).drain(..).map(|t| t.value));
        notes.extend(out.items::<LinkNote>(2).drain(..).map(|t| t.value));
        if let Some(s) = out.items::<AdvStats>(1).last() {
            stats = s.value;
        }
    }
    (adverts, notes, stats)
}

#[test]
fn a_recording_gives_the_same_with_any_number_of_helpers_and_never_sheds() {
    let (rate, centre) = (24e6, 2412e6);
    let x = signal(&BusyScene::busy(rate, centre), 0.5);
    let base = outputs(&x, rate, centre, 0);
    assert!(base.0.len() > 40 && base.1.len() > 20, "{} {}", base.0.len(), base.1.len());
    assert!(base.2.wide && !base.2.shed);
    for helpers in 1..=4 {
        assert_eq!(outputs(&x, rate, centre, helpers), base, "{helpers} helpers");
    }
}

/// The result set of the busy scene, for the cost measurement and the report.
pub fn summary(res: &Results) -> String {
    let conns = rows(res, "connections");
    let bad: f64 = conns.iter().map(|r| cnum(r, &C::BAD_CRC)).sum();
    let packets: f64 = conns.iter().map(|r| cnum(r, &C::PACKETS)).sum();
    format!(
        "{} connections ({} found), {} connection packets, {} bad CRC, {} devices, shed {:?}",
        conns.len(),
        num(res, "connections_found").unwrap_or(0.0),
        packets,
        bad,
        rows(res, "devices").len(),
        value(res, "data_channels_shed"),
    )
}

/// The cost of the busy scene fed at signal pace as live input: run with `--ignored --nocapture` in a release build.
#[test]
#[ignore = "a measurement: run it alone on a quiet machine"]
fn cost_of_the_busy_scene_at_signal_pace() {
    for (rate, centre) in [(8e6, 2440e6), (12.5e6, 2440e6), (20e6, 2440e6), (20e6, 2410e6)] {
        let seconds = 6.0;
        let x = signal(&BusyScene::busy(rate, centre), seconds);
        let mut r = rx();
        r.configure(&InputInfo::new(rate, centre, centre)).unwrap();
        let piece = (rate * 0.01) as usize;
        let started = std::time::Instant::now();
        let (mut work, mut late) = (0.0f64, 0.0f64);
        let mut out = sdrkit_core::Output::default();
        for (i, p) in x.chunks(piece).enumerate() {
            // Each piece is fed when its last sample would have arrived.
            let due = (i + 1) as f64 * 0.01;
            let now = started.elapsed().as_secs_f64();
            if now < due {
                std::thread::sleep(std::time::Duration::from_secs_f64(due - now));
            }
            let t = std::time::Instant::now();
            out.clear();
            r.feed(p, &mut out);
            work += t.elapsed().as_secs_f64();
            late = late.max(started.elapsed().as_secs_f64() - due);
        }
        let res = Owner::results(&r);
        println!(
            "{} Msps centre {} MHz: {:.3} s of work per second of signal, at most {:.0} ms behind; {}; {}",
            rate / 1e6,
            centre / 1e6,
            work / seconds,
            late * 1e3,
            summary(res),
            res.status().summary
        );
    }
}
