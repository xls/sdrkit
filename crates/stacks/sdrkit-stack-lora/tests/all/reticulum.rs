//! `reticulum`: the worked announce on the air with the fixed plan and with the search, a split packet, the plan
//! data file, the test signal and noise.

use crate::common::{dial, feed, rows};
use crate::fanet::{band_of, plan_file, requests, signal};
use sdrkit_core::interface::ReadFields;
use sdrkit_core::publish::keys;
use sdrkit_core::verify::CaseSetting;
use sdrkit_core::{Complex32, Receiver};
use sdrkit_dev_test::{Awgn, Harness};
use sdrkit_proto_lora_phy::modulate::TxFrame;
use sdrkit_stack_lora::components::reticulum::{self as rc, FixedBw};
use sdrkit_stack_lora::proto::reticulum::{self as rns, Identity, PacketType};
use sdrkit_stack_lora::typed::{
    RnsDestinationRow, RnsFields, RnsPacketRow, RNS_ANNOUNCE, RNS_DESTINATION, RNS_DESTINATIONS,
    RNS_PACKETS,
};
use sdrkit_stack_lora::RETICULUM;

/// The worked announce of the specification.
fn r1() -> Vec<u8> {
    let seed: [u8; 32] = std::array::from_fn(|i| i as u8 + 1);
    let x: [u8; 32] = std::array::from_fn(|i| i as u8 + 0x21);
    let random: [u8; 10] = std::array::from_fn(|i| 0xA0 + i as u8);
    Identity::new(x, seed).announce("lxmf.delivery", 2, random, None, b"node one")
}

/// The radio frames `frames` one after the other at `bw` and `sf`, with the carrier `carrier_hz`, in noise of 0.01 for each
/// component at the amplitude 0.3.
fn air(frames: &[Vec<u8>], bw: f64, sf: u8, carrier_hz: f64) -> Vec<Complex32> {
    let p = rc::tx_params(bw, sf);
    let mut t = 0.05;
    let mut tx = Vec::new();
    for f in frames {
        tx.push(
            TxFrame::new(&p, f)
                .unwrap()
                .at(t)
                .carrier(carrier_hz)
                .amplitude(0.3),
        );
        t += p.airtime(f.len()) + 0.1;
    }
    let mut x = vec![Complex32::default(); ((t + 0.1) * 1e6) as usize];
    for f in &tx {
        f.add_into(&mut x, 1e6, 0);
    }
    Awgn::with_sigma(0.01, 11).process(&mut x);
    x
}

fn receive(x: &[Complex32], s: &[CaseSetting]) -> Box<dyn Receiver> {
    let mut rx = Harness::new(&RETICULUM)
        .with_settings(s)
        .receiver()
        .expect("the receiver");
    feed(rx.as_mut(), x, 1e6, 868.0e6, 868.0e6, 65536);
    rx
}

fn fixed(bw: FixedBw, sf: i64) -> Vec<CaseSetting> {
    vec![
        CaseSetting::new(&rc::AUTO_PLAN, false),
        CaseSetting::new(&rc::BW, bw),
        CaseSetting::new(&rc::SF, sf),
    ]
}

fn announce_rows(rx: &dyn Receiver) -> usize {
    rows(rx.results(), &RNS_PACKETS)
        .iter()
        .filter(|r| r.at(&RnsPacketRow::TYPE) == Some(PacketType::Announce))
        .count()
}

#[test]
fn the_worked_announce_decodes_with_the_fixed_plan() {
    let frames = rns::radio_frames(&r1(), 1, false, 0);
    assert_eq!(frames[0][0], 0x10);
    for (bw, sf) in [
        (FixedBw::Bw125, 8),
        (FixedBw::Bw250, 9),
        (FixedBw::Bw62_5, 7),
    ] {
        for carrier in [0.0, 900.0] {
            let x = air(&frames, bw.hz(), sf, carrier);
            let rx = receive(&x, &fixed(bw, i64::from(sf)));
            let res = rx.results();
            let r = rows(res, &RNS_PACKETS);
            assert_eq!(r.len(), 1, "{bw:?} SF{sf} {carrier}");
            assert_eq!(r[0].at(&RnsPacketRow::TYPE), Some(PacketType::Announce));
            assert_eq!(
                r[0].at(&RnsPacketRow::DESTINATION),
                Some("a4f1fb4ade20aacc46af885cc8c7c9e8")
            );
            assert_eq!(
                r[0].at(&RnsPacketRow::CONTENT),
                Some("identity d3659460b83a4336, 2 hops, signature good, \"node one\"")
            );
            assert_eq!(r[0].at(&RnsPacketRow::ENCRYPTED), Some(false));
            let f = r[0].get(&keys::FREQ_HZ).unwrap();
            assert!((f - 868.0e6 - carrier).abs() < 300.0, "{f}");
            let d = rows(res, &RNS_DESTINATIONS);
            assert_eq!(
                d[0].at(&RnsDestinationRow::IDENTITY),
                Some("d3659460b83a4336c91a295c1dbc4432")
            );
            assert_eq!(d[0].at(&RnsDestinationRow::APP_TEXT), Some("node one"));
        }
    }
}

#[test]
fn the_search_finds_the_announce_and_joins_a_split_packet() {
    let whole = rns::radio_frames(&r1(), 1, false, 0);
    let x = air(&whole, 125e3, 8, 400.0);
    let rx = receive(&x, &[]);
    assert_eq!(announce_rows(rx.as_ref()), 1);
    assert_eq!(
        rows(rx.results(), &RNS_PACKETS)[0].at(&RnsPacketRow::SETTING),
        Some("125 kHz SF8")
    );
    let two = rns::radio_frames(&r1(), 5, true, 100);
    assert_eq!((two[0][0], two[1][0]), (0x51, 0x51));
    let x = air(&two, 125e3, 8, 0.0);
    let rx = receive(&x, &fixed(FixedBw::Bw125, 8));
    assert_eq!(announce_rows(rx.as_ref()), 1);
    // The first part alone gives nothing.
    let x = air(&two[..1], 125e3, 8, 0.0);
    let rx = receive(&x, &fixed(FixedBw::Bw125, 8));
    assert!(rows(rx.results(), &RNS_PACKETS).is_empty());
}

#[test]
fn a_changed_announce_is_refused_and_an_authenticated_interface_gives_a_row() {
    let mut bad = r1();
    bad[60] ^= 0x04;
    let mut access = vec![0x80, 0x07];
    access.extend([0x5C; 40]);
    let frames = [
        rns::radio_frames(&bad, 2, false, 0).remove(0),
        rns::radio_frames(&access, 3, false, 0).remove(0),
    ];
    let x = air(&frames, 125e3, 8, 0.0);
    let rx = receive(&x, &fixed(FixedBw::Bw125, 8));
    let res = rx.results();
    assert_eq!(res.fields().get(&keys::FRAMES_BAD), Some(1));
    let r = rows(res, &RNS_PACKETS);
    assert_eq!(r.len(), 1);
    assert_eq!(
        r[0].at(&RnsPacketRow::CONTENT),
        Some("authenticated interface, 40 bytes")
    );
    assert_eq!(r[0].at(&RnsPacketRow::DESTINATION), Some(""));
    assert!(rows(res, &RNS_DESTINATIONS).is_empty());
}

#[test]
fn noise_gives_no_packet() {
    let x = Awgn::with_sigma(0.5, 4).noise(1_000_000);
    let rx = receive(&x, &[]);
    assert!(rows(rx.results(), &RNS_PACKETS).is_empty());
    assert_eq!(rx.results().fields().get(&keys::FRAMES_OK), Some(0));
}

#[test]
fn the_data_file_has_the_rows_of_the_specification_and_its_settings_decode() {
    let file = plan_file("reticulum.json");
    assert_eq!(file.groups.len(), 5);
    assert_eq!((file.countries.len(), file.regions.len()), (4, 3));
    // The row of Australia: a band of 915 to 928 MHz. The consumer tunes the session into it. The network of this test
    // uses 250 kHz and SF9, which the consumer sets as the fixed plan.
    let (middle, width) = band_of(&file, "AU");
    assert_eq!((middle - width / 2.0, middle + width / 2.0), (915e6, 928e6));
    let dial = 921.5e6;
    let s = fixed(FixedBw::Bw250, 9);
    let x = signal(&RETICULUM, 8.0, &s);
    let mut rx = Harness::new(&RETICULUM)
        .with_settings(&s)
        .receiver()
        .unwrap();
    feed(
        rx.as_mut(),
        &x,
        RETICULUM.tuning.rate_hz,
        dial + rc::TUNE_OFFSET_HZ,
        dial,
        65536,
    );
    let res = rx.results();
    assert_eq!(res.status().lock_state, sdrkit_core::LockState::Locked);
    assert_eq!(rows(res, &RNS_DESTINATIONS).len(), 3);
    assert_eq!(
        rows(res, &RNS_PACKETS)[0].at(&RnsPacketRow::SETTING),
        Some("250 kHz SF9")
    );
    assert_eq!(requests(&RETICULUM), 0);
}

#[test]
fn the_test_signal_locks_with_the_default_settings() {
    let dial = dial(&RETICULUM);
    let x = signal(&RETICULUM, 8.0, &[]);
    let mut rx = Harness::new(&RETICULUM).receiver().unwrap();
    let ev = feed(
        rx.as_mut(),
        &x,
        RETICULUM.tuning.rate_hz,
        dial + rc::TUNE_OFFSET_HZ,
        dial,
        65536,
    );
    let res = rx.results();
    assert_eq!(res.status().lock_state, sdrkit_core::LockState::Locked);
    assert_eq!(res.fields().get(&keys::FRAMES_BAD), Some(0));
    let log = rows(res, &RNS_PACKETS);
    let n = |k: PacketType| {
        log.iter()
            .filter(|r| r.at(&RnsPacketRow::TYPE) == Some(k))
            .count()
    };
    assert!(n(PacketType::Announce) >= 3);
    assert!(n(PacketType::Data) >= 2 && n(PacketType::Proof) >= 1);
    assert!(log
        .iter()
        .any(|r| r.at(&RnsPacketRow::ENCRYPTED) == Some(true)));
    assert_eq!(rows(res, &RNS_DESTINATIONS).len(), 3);
    assert_eq!(ev.iter().filter(|e| e.is(&RNS_DESTINATION)).count(), 3);
    assert!(ev.iter().filter(|e| e.is(&RNS_ANNOUNCE)).count() >= 3);
}

#[test]
fn a_tampered_announce_of_the_test_signal_counts_as_bad() {
    let h = Harness::new(&RETICULUM)
        .seconds(8.0)
        .set(&rc::GEN_TAMPERED, true);
    let (_, rx) = crate::common::gen_run(&h, &RETICULUM, 65536);
    let f = rx.results().fields();
    assert!(f.get(&keys::FRAMES_BAD).unwrap_or(0) >= 1);
    assert!(f.get(&keys::FRAMES_OK).unwrap_or(0) >= 6);
}

/// A change of frequency keeps the packet log, the destinations and the counts, and drops the signal to noise ratio,
/// the offset and the level of the present signal.
#[test]
fn a_change_of_frequency_keeps_the_destinations_and_drops_the_levels() {
    let dial = dial(&RETICULUM);
    let x = signal(&RETICULUM, 8.0, &[]);
    let mut rx = Harness::new(&RETICULUM).receiver().unwrap();
    let rate = RETICULUM.tuning.rate_hz;
    feed(
        rx.as_mut(),
        &x,
        rate,
        dial + rc::TUNE_OFFSET_HZ,
        dial,
        65536,
    );
    let read = |rx: &dyn Receiver| {
        let f = RnsFields::read(rx.results().fields()).expect("the fields of reticulum");
        (
            f.frames_ok,
            f.destinations_heard,
            f.snr_db,
            f.offset_hz,
            f.level_dbfs,
        )
    };
    let counts = |rx: &dyn Receiver| {
        let r = rx.results();
        (
            rows(r, &RNS_PACKETS).len(),
            rows(r, &RNS_DESTINATIONS).len(),
        )
    };
    let before = counts(rx.as_ref());
    let (good, destinations, snr, offset, level) = read(rx.as_ref());
    assert!(good >= 3 && destinations >= 2 && before.0 >= 3);
    assert!(snr.is_some() && offset.is_some() && level.is_some());
    rx.forget();
    assert_eq!(counts(rx.as_ref()), before);
    assert_eq!(read(rx.as_ref()), (good, destinations, None, None, None));
    assert_eq!(rx.results().status().snr_db, None);
}
