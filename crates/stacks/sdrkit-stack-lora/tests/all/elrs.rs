//! `elrs`: the worked frames in the decoder and on the air, the rules for links, the binding phrase, the settings of the plan
//! data file, the test signal and noise.

use crate::common::{dial, feed, invoke, rows};
use crate::fanet::{plan_file, requests, signal};
use sdrkit_core::interface::ReadFields;
use sdrkit_core::publish::keys;
use sdrkit_core::verify::CaseSetting;
use sdrkit_core::{Choice, Complex32, LockState, Receiver, SettingSet, Value};
use sdrkit_dev_test::{Awgn, Harness};
use sdrkit_proto_lora_phy::modulate::TxFrame;
use sdrkit_stack_lora::components::channels::{Input, Planner};
use sdrkit_stack_lora::components::elrs::{
    self as ec, Air, ElrsDecode, ElrsPlanSetting, ElrsPlanner, FrameKind, PacketRate,
};
use sdrkit_stack_lora::proto::elrs::{self, LinkId, ROWS};
use sdrkit_stack_lora::typed::{
    ElrsFields, ElrsFrameRow, ElrsLinkRow, ELRS_FRAMES, ELRS_LINK, ELRS_LINKS,
};
use sdrkit_stack_lora::ELRS;

const PHRASE: &str = "sdrkit test";
const SYNC8: [u8; 8] = [0x0E, 0x11, 0x2A, 0x03, 0xF5, 0xB1, 0xED, 0x6F];
const RC8: [u8; 8] = [0xB4, 0x64, 0x00, 0xF8, 0x3F, 0x4B, 0x80, 0x43];
const RC13: [u8; 13] = [
    0x80, 0x64, 0x00, 0xF8, 0x3F, 0x4B, 0x01, 0x02, 0x03, 0x04, 0x05, 0x65, 0x31,
];
const SYNC13: [u8; 13] = [
    0x02, 0x05, 0x2A, 0x03, 0xF5, 0xB1, 0xED, 0x00, 0x00, 0x00, 0x00, 0xFA, 0x45,
];
const STICKS: &str = "ch1-4 100 512 1023 300, armed 1";

/// True if `b` holds the bytes of `text`.
fn holds(b: &[u8], text: &str) -> bool {
    b.windows(text.len()).any(|w| w == text.as_bytes())
}

/// `bytes` as a frame that ends at `t` on hop channel 3.
fn air(bytes: &[u8], t: f64) -> Air<'_> {
    Air {
        bytes,
        start_s: t - 0.005,
        end_s: t,
        hop_channel: 3,
        freq_hz: 868.3e6,
        sf: 7,
        inverted: false,
        snr_db: 10.0,
    }
}

/// The newest row of the log: link, kind, content, encrypted.
fn newest(d: &ElrsDecode) -> (String, FrameKind, String, bool) {
    let r = d.heard.frames.newest_first().next().expect("a row");
    (r.link.clone(), r.kind, r.content.clone(), r.encrypted)
}

#[test]
fn the_sync_frame_makes_its_link_known_and_a_wrong_bit_does_not() {
    let mut d = ElrsDecode::default();
    let mut wrong = SYNC8;
    wrong[3] = 0x02;
    d.take(&air(&wrong, 1.0));
    assert_eq!((d.heard.good, d.heard.bad), (0, 1));
    assert!(d.heard.frames.is_empty() && d.heard.links.is_empty());
    // With no known link a radio-control frame cannot be checked.
    d.take(&air(&RC8, 1.1));
    assert_eq!((d.heard.good, d.heard.bad), (0, 2));
    d.take(&air(&SYNC8, 1.2));
    assert_eq!(
        newest(&d),
        (
            "B1ED".into(),
            FrameKind::Sync,
            "hop 17, nonce 42, rate index 3".into(),
            false
        )
    );
    d.take(&air(&RC8, 1.3));
    assert_eq!(
        newest(&d),
        (
            "B1ED".into(),
            FrameKind::Rc,
            "8 bytes, nonce 77".into(),
            true
        )
    );
    d.take(&air(&SYNC13, 1.4));
    assert_eq!(newest(&d).2, "hop 5, nonce 42, rate index 3");
    d.take(&air(&RC13, 1.5));
    assert_eq!(newest(&d).2, "13 bytes, nonce 9");
    // A frame of another size is not counted.
    d.take(&air(&[0u8; 9], 1.6));
    assert_eq!((d.heard.good, d.heard.bad), (4, 2));
    let l = d.heard.links.get(&LinkId(0xB1ED)).expect("the link");
    assert_eq!((l.frames, l.sync_frames, l.hop_index), (4, 2, Some(5)));
    assert_eq!(l.heard_on, [3]);
}

#[test]
fn the_phrase_shows_the_stick_values_of_its_link_only() {
    let mut d = ElrsDecode::default();
    d.set_phrase(PHRASE);
    // The link of the phrase needs no sync frame.
    d.take(&air(&RC8, 1.0));
    assert_eq!(
        newest(&d),
        ("B1ED".into(), FrameKind::Rc, STICKS.into(), false)
    );
    d.take(&air(&RC13, 1.1));
    assert_eq!(newest(&d).2, STICKS);
    // A sync frame of another link is shown, and the link of the phrase stays.
    let other = elrs::build_sync(&[0, 0, 0, 0, 0x01, 0x02], 3, 4, 1, 8);
    d.take(&air(&other, 1.2));
    assert_eq!(newest(&d).0, "0102");
    let rc_other = elrs::build_rc(LinkId(0x0102), 5, [1, 2, 3, 4], false, 8, [0; 5]);
    d.take(&air(&rc_other, 1.3));
    assert_eq!(
        newest(&d),
        (
            "0102".into(),
            FrameKind::Rc,
            "8 bytes, nonce 5".into(),
            true
        )
    );
    d.take(&air(&RC8, 1.4));
    assert_eq!(
        newest(&d),
        ("B1ED".into(), FrameKind::Rc, STICKS.into(), false)
    );
    // What the decoder keeps and what it saves hold the link and the stick values, and not the phrase.
    let kept = serde_json::to_string(&d.heard).expect("the tables as JSON");
    assert!(kept.contains("B1ED") && kept.contains("ch1-4 100 512"));
    assert!(!kept.contains(PHRASE) && !kept.contains("sdrkit"));
    let saved = sdrkit_graph::Component::save_state(&d).expect("a saved state");
    assert!(holds(saved.bytes(), "B1ED") && !holds(saved.bytes(), PHRASE));
    // An empty phrase forgets the link of the phrase: it has no sync frame, so its frames are bad.
    d.set_phrase("");
    let bad = d.heard.bad;
    d.take(&air(&RC8, 1.5));
    assert_eq!(d.heard.bad, bad + 1);
}

#[test]
fn a_frame_is_accepted_for_ten_seconds_after_a_sync_frame_of_its_link() {
    let mut d = ElrsDecode::default();
    d.take(&air(&SYNC8, 1.0));
    d.take(&air(&RC8, 10.9));
    assert_eq!((d.heard.good, d.heard.bad), (2, 0));
    d.take(&air(&RC8, 12.0));
    assert_eq!((d.heard.good, d.heard.bad), (2, 1));
    assert_eq!(d.heard.frames.len(), 2);
}

#[test]
fn a_second_copy_and_the_false_twin_of_a_frame_are_not_counted() {
    let mut d = ElrsDecode::default();
    d.take(&air(&SYNC8, 1.0));
    // The other polarity delivers the same bytes 1 ms later with a better SNR.
    let mut copy = air(&SYNC8, 1.001);
    copy.inverted = true;
    copy.snr_db = 14.0;
    d.take(&copy);
    assert_eq!((d.heard.good, d.heard.frames.len()), (1, 1));
    assert_eq!(d.heard.frames.newest_first().next().unwrap().snr_db, 14.0);
    // The other row of the same spreading factor reads the same samples as 13 bytes of no meaning: before or after.
    let twin = [0x55u8; 13];
    d.take(&air(&twin, 2.0));
    assert_eq!(d.heard.bad, 1);
    d.take(&air(&RC8, 2.001));
    assert_eq!((d.heard.good, d.heard.bad), (2, 0));
    d.take(&air(&RC8, 3.0));
    d.take(&air(&twin, 3.002));
    assert_eq!((d.heard.good, d.heard.bad), (3, 0));
    // A twin whose CRC fits one of the 256 start values by chance is not a frame of the link: it overlaps an accepted
    // frame of another size, or it is not on the rate row of the sync frames of the link.
    let lucky = elrs::build_rc(LinkId(0xB1ED), 200, [9, 8, 7, 6], false, 13, [0; 5]);
    d.take(&air(&RC8, 4.0));
    d.take(&air(&lucky, 4.002));
    assert_eq!((d.heard.good, d.heard.bad), (4, 0));
    d.take(&air(&lucky, 5.0));
    assert_eq!((d.heard.good, d.heard.bad), (4, 1));
    // On another channel it is a bad frame.
    let mut far = air(&twin, 3.002);
    far.hop_channel = 4;
    d.take(&far);
    assert_eq!(d.heard.bad, 2);
}

/// The sync frame on hop channel 6 and the radio-control frame on hop channel 7 of a plan of 13 channels 525 kHz apart, at
/// 8 Msps with the middle of the plan at the centre, 0.25 s.
fn two_frames(row: usize, inverted: bool) -> Vec<Complex32> {
    let rate = 8e6;
    let p = ec::tx_params(&ROWS[row], inverted);
    let len = usize::from(ROWS[row].bytes);
    let uid = elrs::uid(PHRASE);
    let link = elrs::link_of(PHRASE);
    let sync = elrs::build_sync(&uid, 17, 42, 3, len);
    let rc = elrs::build_rc(link, 77, [100, 512, 1023, 300], true, len, [1, 2, 3, 4, 5]);
    let mut x = vec![Complex32::default(); (rate * 0.25) as usize];
    for (bytes, at, off) in [(sync, 0.02, 400.0), (rc, 0.12, 525e3 - 300.0)] {
        TxFrame::new(&p, &bytes)
            .expect("a frame")
            .at(at)
            .carrier(off)
            .amplitude(0.3)
            .add_into(&mut x, rate, 0);
    }
    Awgn::with_sigma(0.005, 7).process(&mut x);
    x
}

fn eu_receiver(rate: PacketRate, phrase: &str) -> Box<dyn Receiver> {
    Harness::new(&ELRS)
        .set(&ec::HOP_CHANNELS, 13)
        .set(&ec::HOP_STEP_HZ, 525e3)
        .set(&ec::PACKET_RATE, rate)
        .set(&ec::BINDING_PHRASE, phrase)
        .receiver()
        .expect("the receiver")
}

#[test]
fn the_two_frames_decode_on_the_air_at_each_spreading_factor_and_polarity() {
    let dial = 866.425e6;
    for (row, rate) in [
        (0, PacketRate::Hz200),
        (1, PacketRate::Hz100),
        (2, PacketRate::Hz50),
    ] {
        for inverted in [false, true] {
            let x = two_frames(row, inverted);
            for phrase in ["", PHRASE] {
                let mut rx = eu_receiver(rate, phrase);
                feed(rx.as_mut(), &x, 8e6, dial, dial, 65536);
                let res = rx.results();
                let r = rows(res, &ELRS_FRAMES);
                let what = format!("row {row}, inverted {inverted}, phrase {phrase:?}");
                assert_eq!(r.len(), 2, "{what}");
                assert_eq!(
                    r[1].at(&ElrsFrameRow::KIND),
                    Some(FrameKind::Sync),
                    "{what}"
                );
                assert_eq!(r[1].at(&ElrsFrameRow::LINK), Some("B1ED"));
                assert_eq!(
                    r[1].at(&ElrsFrameRow::CONTENT),
                    Some("hop 17, nonce 42, rate index 3")
                );
                assert_eq!(r[1].at(&ElrsFrameRow::HOP_CHANNEL), Some(6));
                assert_eq!(r[0].at(&ElrsFrameRow::KIND), Some(FrameKind::Rc), "{what}");
                assert_eq!(r[0].at(&ElrsFrameRow::HOP_CHANNEL), Some(7));
                assert_eq!(r[0].at(&ElrsFrameRow::INVERTED), Some(inverted), "{what}");
                assert_eq!(r[0].at(&ElrsFrameRow::SF), Some(i64::from(ROWS[row].sf)));
                let content = r[0].at(&ElrsFrameRow::CONTENT).expect("content");
                if phrase.is_empty() {
                    assert_eq!(content, "8 bytes, nonce 77");
                    assert_eq!(r[0].at(&ElrsFrameRow::ENCRYPTED), Some(true));
                } else {
                    assert_eq!(content, STICKS);
                }
                let f = r[0].get(&keys::FREQ_HZ).expect("a frequency");
                assert!((f - (dial + 525e3 - 300.0)).abs() < 2e3, "{f}");
                assert_eq!(res.fields().get(&keys::FRAMES_BAD), Some(0), "{what}");
                assert_eq!(res.fields().at(&ElrsFields::HOP_CHANNELS_INSIDE), Some(13));
            }
        }
    }
}

#[test]
fn every_rate_row_decodes_with_all_rates_searched() {
    let dial = 866.425e6;
    for row in [0, 3, 4] {
        let x = two_frames(row, false);
        let x = if row == 3 {
            // The frames of the 25 Hz row are longer than the gap of the scene.
            let mut y = x;
            y.extend(std::iter::repeat_n(Complex32::default(), 400_000));
            y
        } else {
            x
        };
        let mut rx = eu_receiver(PacketRate::All, PHRASE);
        feed(rx.as_mut(), &x, 8e6, dial, dial, 65536);
        let res = rx.results();
        let r = rows(res, &ELRS_FRAMES);
        assert_eq!(r.len(), 2, "row {row}");
        assert_eq!(r[0].at(&ElrsFrameRow::CONTENT), Some(STICKS), "row {row}");
        assert_eq!(
            r[0].at(&ElrsFrameRow::BYTES),
            Some(i64::from(ROWS[row].bytes)),
            "row {row}"
        );
        assert_eq!(res.fields().get(&keys::FRAMES_BAD), Some(0), "row {row}");
    }
}

fn plan_of(channels: i64, step_hz: f64, rate: f64, dial: f64) -> ec::ElrsPlan {
    let mut p = ElrsPlanner::default();
    p.apply(ElrsPlanSetting::HopChannels.id(), &Value::I64(channels))
        .unwrap();
    p.apply(ElrsPlanSetting::HopStepHz.id(), &Value::Hz(step_hz))
        .unwrap();
    p.plan(&Input {
        rate,
        centre_hz: dial,
        tuned_hz: Some(dial),
    })
    .1
}

/// The dial, the count of channels and the spacing of the plan that the country `code` of the data file names first.
fn row_of(file: &sdrkit_data_plans::data::PlanFile, code: &str) -> (f64, i64, f64) {
    let entry = file
        .countries
        .iter()
        .find(|c| c.code == code)
        .expect("the country");
    let group = file
        .groups
        .iter()
        .find(|g| g.id == entry.groups[0])
        .expect("its group");
    let n = group.bands[0]
        .numbering
        .as_ref()
        .expect("numbered channels");
    let count = i64::from(n.last - n.first + 1);
    let dial = n.first_centre_hz + n.step_hz * (count - 1) as f64 / 2.0;
    (dial, count, n.step_hz)
}

#[test]
fn the_data_file_has_the_hop_plans_and_the_row_of_australia_decodes() {
    let file = plan_file("elrs.json");
    assert_eq!(file.groups.len(), 9);
    assert_eq!((file.countries.len(), file.regions.len()), (6, 3));
    assert!(file
        .groups
        .iter()
        .all(|g| g.bands.len() == 1 && !g.verified));
    let (dial, count, step) = row_of(&file, "AU");
    assert!((dial - 921.2e6).abs() < 1.0);
    assert_eq!((count, step), (20, 600e3));
    assert_eq!(row_of(&file, "NZ").1, 20);
    let us = row_of(&file, "US");
    assert!((us.0 - 915.2e6).abs() < 1.0);
    assert_eq!(us.1, 40);
    // The hop channels inside a capture: the whole plan of Australia at 14 Msps, 12 channels of each wide plan at 8 Msps.
    assert_eq!(plan_of(20, 600e3, 14e6, dial).inside_count(), 20);
    assert_eq!(plan_of(20, 600e3, 8e6, dial).inside_count(), 12);
    assert_eq!(plan_of(40, 600e3, 8e6, us.0).inside_count(), 12);
    assert_eq!(plan_of(13, 525e3, 8e6, 866.425e6).inside_count(), 13);
    assert_eq!(plan_of(20, 600e3, 14e6, dial).channels[0], 915.5e6);
    // The test signal with the settings of the row, at the rate of the stack: the four middle channels are inside.
    let s = [
        CaseSetting::new(&ec::HOP_CHANNELS, count),
        CaseSetting::new(&ec::HOP_STEP_HZ, step),
    ];
    let x = signal(&ELRS, 2.0, &s);
    let mut rx = Harness::new(&ELRS).with_settings(&s).receiver().unwrap();
    feed(rx.as_mut(), &x, ELRS.tuning.rate_hz, dial, dial, 65536);
    let res = rx.results();
    assert_eq!(res.status().lock_state, LockState::Locked);
    assert_eq!(res.fields().at(&ElrsFields::HOP_CHANNELS_INSIDE), Some(4));
    assert_eq!(res.fields().at(&ElrsFields::HOP_CHANNELS), Some(20));
    let links = rows(res, &ELRS_LINKS);
    assert_eq!(links.len(), 1);
    assert_eq!(links[0].get(&keys::SOURCE), Some("B1ED"));
    assert_eq!(links[0].at(&ElrsLinkRow::HEARD_ON), Some("8 9 10 11"));
    assert_eq!(res.fields().get(&keys::FRAMES_BAD), Some(0));
    assert!(links[0].at(&ElrsLinkRow::SYNC_FRAMES).unwrap_or(0) >= 10);
    assert!(res.fields().get(&keys::FRAMES_OK).unwrap_or(0) >= 180);
    // The stack holds no plan: it asks the host for no frequency.
    assert_eq!(requests(&ELRS), 0);
}

/// Runs the test signal of the stack for `secs` with these settings.
fn run(secs: f64, s: &[CaseSetting]) -> Box<dyn Receiver> {
    let dial = dial(&ELRS);
    let x = signal(&ELRS, secs, s);
    let mut rx = Harness::new(&ELRS).with_settings(s).receiver().unwrap();
    feed(rx.as_mut(), &x, ELRS.tuning.rate_hz, dial, dial, 65536);
    rx
}

#[test]
fn the_test_signal_gives_its_link_in_noise_with_both_polarities() {
    for inverted in [false, true] {
        let s = [CaseSetting::new(&ec::GEN_INVERTED, inverted)];
        let dial = dial(&ELRS);
        let rate = ELRS.tuning.rate_hz;
        let mut x = signal(&ELRS, 2.0, &s);
        // 20 dB in 500 kHz for a signal of unit power.
        Awgn::new(1.0, 20.0, 500e3, rate, 3).process(&mut x);
        let mut rx = Harness::new(&ELRS).with_settings(&s).receiver().unwrap();
        let ev = feed(rx.as_mut(), &x, rate, dial, dial, 65536);
        let res = rx.results();
        let r = rows(res, &ELRS_FRAMES);
        // 100 frames a second are sent.
        assert!(r.len() >= 180, "{} rows", r.len());
        assert!(r.iter().all(|f| f.at(&ElrsFrameRow::LINK) == Some("B1ED")));
        assert!(r
            .iter()
            .all(|f| f.at(&ElrsFrameRow::INVERTED) == Some(inverted)));
        assert!(r
            .iter()
            .any(|f| f.at(&ElrsFrameRow::KIND) == Some(FrameKind::Sync)));
        assert_eq!(rows(res, &ELRS_LINKS).len(), 1);
        assert_eq!(ev.iter().filter(|e| e.is(&ELRS_LINK)).count(), 1);
        assert_eq!(res.status().lock_state, LockState::Locked);
        // The first frame of the signal is a sync frame: no frame comes before its link is known.
        assert_eq!(res.fields().get(&keys::FRAMES_BAD), Some(0));
        assert!(res
            .status()
            .summary
            .contains("4 of 4 hop channels inside the capture"));
        assert!(res
            .status()
            .summary
            .contains("not confirmed by a public document"));
        let note = res.fields().at(&ElrsFields::NOTE).expect("the note");
        assert!(
            note.contains("not confirmed by a public document") && note.contains(ec::NO_PHRASE)
        );
    }
}

#[test]
fn a_second_link_without_sync_frames_gives_no_row() {
    let rx = run(1.0, &[CaseSetting::new(&ec::GEN_SECOND_LINK, true)]);
    let res = rx.results();
    let r = rows(res, &ELRS_FRAMES);
    assert!(r.len() >= 90, "{} rows", r.len());
    assert!(r.iter().all(|f| f.at(&ElrsFrameRow::LINK) == Some("B1ED")));
    assert_eq!(rows(res, &ELRS_LINKS).len(), 1);
    // The frames of the second link are counted as bad.
    assert!(res.fields().get(&keys::FRAMES_BAD).unwrap_or(0) >= 80);
}

#[test]
fn the_binding_phrase_is_not_in_the_saved_state_or_the_results() {
    let phrase = CaseSetting::new(&ec::BINDING_PHRASE, PHRASE);
    let mut rx = run(1.0, std::slice::from_ref(&phrase));
    let res = rx.results();
    let r = rows(res, &ELRS_FRAMES);
    assert!(r.iter().any(|f| f
        .at(&ElrsFrameRow::CONTENT)
        .is_some_and(|c| c.starts_with("ch1-4 "))));
    assert_eq!(res.fields().at(&ElrsFields::PHRASE_SET), Some(true));
    assert!(!res
        .fields()
        .at(&ElrsFields::NOTE)
        .unwrap()
        .contains(ec::NO_PHRASE));
    let state = rx.save_state().expect("a saved state");
    assert!(!holds(state.bytes(), PHRASE));
    assert!(res.log_lines().iter().all(|l| !l.contains(PHRASE)));
    assert!(!res.status().summary.contains(PHRASE));
    // The state restores without the phrase, and `clear` empties the tables and the counters.
    let dial = dial(&ELRS);
    let mut again = Harness::new(&ELRS).receiver().unwrap();
    feed(again.as_mut(), &[], ELRS.tuning.rate_hz, dial, dial, 65536);
    again.restore_state(&state).unwrap();
    assert_eq!(rows(again.results(), &ELRS_LINKS).len(), 1);
    assert_eq!(
        again.results().fields().at(&ElrsFields::PHRASE_SET),
        Some(false)
    );
    invoke(rx.as_mut(), "clear", &[]).unwrap();
    assert_eq!(rx.results().fields().get(&keys::FRAMES_OK), Some(0));
    assert!(rows(rx.results(), &ELRS_FRAMES).is_empty());
    assert!(rows(rx.results(), &ELRS_LINKS).is_empty());
}

#[test]
fn with_all_rates_searched_each_frame_of_the_test_signal_is_one_row_of_its_size() {
    for gen_row in [1i64, 2, 5] {
        let s = [
            CaseSetting::new(&ec::PACKET_RATE, PacketRate::All),
            CaseSetting::new(&ec::GEN_ROW, gen_row),
            CaseSetting::new(&ec::BINDING_PHRASE, PHRASE),
        ];
        let rx = run(2.0, &s);
        let res = rx.results();
        let row = &ROWS[gen_row as usize - 1];
        let sent = (2.0 / row.interval_s) as i64;
        let good = res.fields().get(&keys::FRAMES_OK).unwrap_or(0);
        // The last frame can end after the signal.
        assert!(
            (sent - 1..=sent).contains(&good),
            "row {gen_row}: {good} of {sent}"
        );
        assert_eq!(
            res.fields().get(&keys::FRAMES_BAD),
            Some(0),
            "row {gen_row}"
        );
        let r = rows(res, &ELRS_FRAMES);
        assert!(r.iter().all(|f| {
            f.at(&ElrsFrameRow::BYTES) == Some(i64::from(row.bytes))
                && f.at(&ElrsFrameRow::SF) == Some(i64::from(row.sf))
                && f.at(&ElrsFrameRow::LINK) == Some("B1ED")
        }));
        assert!(r.iter().any(|f| f
            .at(&ElrsFrameRow::CONTENT)
            .is_some_and(|c| c.starts_with("ch1-4 "))));
        assert_eq!(rows(res, &ELRS_LINKS).len(), 1);
    }
}

#[test]
fn the_binding_phrase_stays_after_another_setting_and_a_new_tuning() {
    let dial = dial(&ELRS);
    let rate = ELRS.tuning.rate_hz;
    let phrase = CaseSetting::new(&ec::BINDING_PHRASE, PHRASE);
    let x = signal(&ELRS, 0.5, std::slice::from_ref(&phrase));
    let mut rx = Harness::new(&ELRS)
        .with_settings(std::slice::from_ref(&phrase))
        .receiver()
        .unwrap();
    feed(rx.as_mut(), &x, rate, dial, dial, 65536);
    let id = rx.schema().id(ec::PACKET_RATE.key()).unwrap();
    sdrkit_core::owner::apply(&mut *rx, id, &Value::Choice(PacketRate::All.index())).unwrap();
    invoke(rx.as_mut(), "clear", &[]).unwrap();
    feed(rx.as_mut(), &x, rate, dial + 1e6, dial + 1e6, 65536);
    let res = rx.results();
    assert_eq!(res.fields().at(&ElrsFields::PHRASE_SET), Some(true));
    let r = rows(res, &ELRS_FRAMES);
    assert!(r.len() >= 45, "{} rows", r.len());
    assert!(r.iter().any(|f| f
        .at(&ElrsFrameRow::CONTENT)
        .is_some_and(|c| c.starts_with("ch1-4 "))));
}

#[test]
fn noise_gives_no_frame_and_no_link() {
    let dial = dial(&ELRS);
    let rate = ELRS.tuning.rate_hz;
    let x = Awgn::with_sigma(0.5, 11).noise((5.0 * rate) as usize);
    let mut rx = Harness::new(&ELRS).receiver().unwrap();
    feed(rx.as_mut(), &x, rate, dial, dial, 65536);
    let res = rx.results();
    assert!(rows(res, &ELRS_FRAMES).is_empty());
    assert!(rows(res, &ELRS_LINKS).is_empty());
    assert_eq!(res.fields().get(&keys::FRAMES_OK), Some(0));
    assert_eq!(res.fields().get(&keys::FRAMES_BAD), Some(0));
    assert_eq!(res.status().lock_state, LockState::Searching);
}

#[test]
fn a_rate_below_one_hop_channel_is_refused() {
    let mut rx = Harness::new(&ELRS).receiver().unwrap();
    let e = rx
        .configure(&sdrkit_core::InputInfo::new(1e6, 868e6, 868e6).recording())
        .expect_err("a refusal");
    assert!(
        e.to_string().contains("at least 1100000 samples a second"),
        "{e}"
    );
}

/// A change of frequency keeps the frame log, the links and the counts, and drops the signal to noise ratio and the
/// level of the present signal.
#[test]
fn a_change_of_frequency_keeps_the_links_and_drops_the_levels() {
    let mut rx = run(1.0, &[]);
    let read = |rx: &dyn Receiver| {
        let f = ElrsFields::read(rx.results().fields()).expect("the fields of elrs");
        (f.frames_ok, f.links_heard, f.snr_db, f.level_dbfs)
    };
    let frames = rows(rx.results(), &ELRS_FRAMES).len();
    let (good, links, snr, level) = read(rx.as_ref());
    assert!(frames >= 50 && good >= 50 && links == 1);
    assert!(snr.is_some() && level.is_some());
    rx.forget();
    assert_eq!(rows(rx.results(), &ELRS_FRAMES).len(), frames);
    assert_eq!(rows(rx.results(), &ELRS_LINKS).len(), 1);
    assert_eq!(read(rx.as_ref()), (good, links, None, None));
    assert_eq!(rx.results().status().snr_db, None);
}
