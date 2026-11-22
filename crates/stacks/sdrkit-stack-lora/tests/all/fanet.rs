//! `fanet`: the worked frames on the air at each bandwidth, the settings of the plan data file, the test signal, noise and
//! the saved state.

use crate::common::{dial, feed, rows};
use sdrkit_core::interface::ReadFields;
use sdrkit_core::publish::keys;
use sdrkit_core::verify::CaseSetting;
use sdrkit_core::{Complex32, InputInfo, Output, Receiver, StackDescriptor};
use sdrkit_dev_test::Awgn;
use sdrkit_dev_test::Harness;
use sdrkit_proto_lora_phy::modulate::TxFrame;
use sdrkit_proto_lora_phy::params::LoraParams;
use sdrkit_stack_lora::components::fanet::{self as fc, Bandwidth, FrameKind, StationKind};
use sdrkit_stack_lora::typed::{
    FanetFields, FanetFrameRow, FanetStationRow, FANET_FRAMES, FANET_MESSAGE, FANET_STATION,
    FANET_STATIONS, FANET_TRACKS,
};
use sdrkit_stack_lora::FANET;

/// The tracking frame of 11:2233 at 47.5 N, 8.25 W.
const F1: [u8; 15] = [
    0x01, 0x11, 0x33, 0x22, 0x15, 0x8E, 0x43, 0x25, 0x22, 0xFA, 0xB0, 0x94, 0x28, 0x0C, 0x40,
];

/// The plan data file `name` of the repository's `data/plans`. It is not part of the library: a consumer reads it and sets
/// the settings.
pub fn plan_file(name: &str) -> sdrkit_data_plans::data::PlanFile {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../data/plans")
        .join(name);
    let text = std::fs::read_to_string(&path).expect("the data file");
    // The file goes through the import of the plan registry, as the engine's `plans.import` takes it.
    let mut plans = (*sdrkit_dev_test::plans()).clone();
    sdrkit_data_plans::api::import_as(
        &mut plans,
        &text,
        sdrkit_data_plans::api::PlanFormat::Json,
        "",
        "",
    )
    .expect("the registry takes the file");
    sdrkit_data_plans::data::parse(name, &text).expect("a plan file")
}

/// The centre and the width of the one band of the group that the country `code` of `file` names.
pub fn band_of(file: &sdrkit_data_plans::data::PlanFile, code: &str) -> (f64, f64) {
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
    let b = &group.bands[0];
    ((b.start_hz + b.end_hz) / 2.0, b.end_hz - b.start_hz)
}

/// The clean test signal of stack `d` with these settings, at the rate of the stack.
pub fn signal(d: &'static StackDescriptor, secs: f64, s: &[CaseSetting]) -> Vec<Complex32> {
    Harness::new(d)
        .seconds(secs)
        .with_settings(s)
        .generate()
        .expect("the test signal")
}

/// `payload` as one frame at `at_s` with the carrier `carrier_hz` from the centre of the samples.
fn frame(p: &LoraParams, payload: &[u8], at_s: f64, carrier_hz: f64, ppm: f64) -> TxFrame {
    TxFrame::new(p, payload)
        .expect("a frame")
        .at(at_s)
        .carrier(carrier_hz)
        .ppm(ppm)
}

fn render(frames: &[TxFrame], rate: f64, secs: f64) -> Vec<Complex32> {
    let mut x = vec![Complex32::default(); (rate * secs) as usize];
    for f in frames {
        f.add_into(&mut x, rate, 0);
    }
    x
}

/// Feeds `x`, with the channel at the centre of the samples, to a receiver with the bandwidth `bw`.
fn receive(bw: Bandwidth, x: &[Complex32], rate: f64, dial: f64) -> Box<dyn Receiver> {
    let mut rx = Harness::new(&FANET)
        .set(&fc::BANDWIDTH, bw)
        .receiver()
        .expect("the receiver");
    feed(rx.as_mut(), x, rate, dial, dial, 65536);
    rx
}

fn good(rx: &dyn Receiver) -> i64 {
    rx.results().fields().get(&keys::FRAMES_OK).unwrap_or(0)
}

#[test]
fn the_tracking_frame_decodes_on_the_250_khz_plan_with_carrier_and_clock_errors() {
    let p = fc::tx_params(250e3);
    for (off, ppm) in [(-30e3, 30.0), (0.0, -30.0), (25e3, 30.0)] {
        let x = render(&[frame(&p, &F1, 0.05, off, ppm)], 1e6, 0.4);
        let rx = receive(Bandwidth::Bw250, &x, 1e6, 868.2e6);
        let res = rx.results();
        let r = rows(res, &FANET_FRAMES);
        assert_eq!(r.len(), 1, "offset {off}");
        assert_eq!(r[0].at(&FanetFrameRow::KIND), Some(FrameKind::Tracking));
        assert_eq!(r[0].get(&keys::SOURCE), Some("11:2233"));
        assert_eq!(
            r[0].at(&FanetFrameRow::DECODED),
            Some("paraglider, 1200 m, 20.0 km/h, climb 1.2 m/s, heading 90")
        );
        let (lat, lon) = r[0].get(&keys::POSITION).expect("a position");
        assert!((lat - 47.5).abs() < 2e-5 && (lon + 8.25).abs() < 3e-5);
        let pt = &res.points()["aircraft/11:2233"];
        assert!((pt.lat - 47.5).abs() < 2e-5 && (pt.lon + 8.25).abs() < 3e-5);
        let link = pt.row.as_ref().expect("a row");
        assert_eq!(Some(link.table), res.decl().table_id(FANET_STATIONS.key));
        assert!(res.table(link.table).unwrap().row(&link.key).is_some());
        let s = rows(res, &FANET_STATIONS);
        assert_eq!(s[0].at(&FanetStationRow::KIND), Some(StationKind::Aircraft));
    }
}

#[test]
fn the_tracking_frame_decodes_on_the_500_khz_plan() {
    let p = fc::tx_params(500e3);
    let x = render(&[frame(&p, &F1, 0.05, 1500.0, 0.0)], 2e6, 0.3);
    let rx = receive(Bandwidth::Bw500, &x, 2e6, 920.8e6);
    let r = rows(rx.results(), &FANET_FRAMES);
    assert_eq!(r.len(), 1);
    assert_eq!(r[0].get(&keys::SOURCE), Some("11:2233"));
    // A receiver set to 250 kHz does not decode a 500 kHz frame.
    assert_eq!(
        good(receive(Bandwidth::Bw250, &x, 2e6, 920.8e6).as_ref()),
        0
    );
}

#[test]
fn frames_decode_in_noise_and_close_together() {
    let p = fc::tx_params(250e3);
    for snr in [10.0, 0.0, -4.0] {
        let frames: Vec<TxFrame> = (0..5)
            .map(|k| frame(&p, &F1, 0.05 + 0.12 * f64::from(k), 2000.0, 0.0))
            .collect();
        let mut x = render(&frames, 1e6, 0.75);
        Awgn::new(1.0, snr, 250e3, 1e6, 5).process(&mut x);
        assert_eq!(
            good(receive(Bandwidth::Bw250, &x, 1e6, 868.2e6).as_ref()),
            5,
            "{snr} dB"
        );
    }
    let air = p.airtime(F1.len());
    let frames: Vec<TxFrame> = (0..3)
        .map(|k| frame(&p, &F1, 0.05 + (air + 0.01) * f64::from(k), 0.0, 0.0))
        .collect();
    let x = render(&frames, 1e6, 0.5);
    assert_eq!(
        good(receive(Bandwidth::Bw250, &x, 1e6, 868.2e6).as_ref()),
        3
    );
}

#[test]
fn noise_and_other_sync_words_give_no_frame() {
    let x = Awgn::with_sigma(0.5, 3).noise(1_000_000);
    let rx = receive(Bandwidth::Bw250, &x, 1e6, 868.2e6);
    assert_eq!(good(rx.as_ref()), 0);
    assert!(rows(rx.results(), &FANET_FRAMES).is_empty());
    for sync in [0x12, 0x34] {
        let mut p = fc::tx_params(250e3);
        p.sync = sync;
        let x = render(&[frame(&p, &F1, 0.05, 0.0, 0.0)], 1e6, 0.3);
        let rx = receive(Bandwidth::Bw250, &x, 1e6, 868.2e6);
        assert!(
            rows(rx.results(), &FANET_FRAMES).is_empty(),
            "sync {sync:#x}"
        );
    }
}

#[test]
fn a_wrong_crc_and_a_short_frame_count_as_bad() {
    let p = fc::tx_params(250e3);
    let mut bad = TxFrame::new(&p, &F1).unwrap().at(0.05);
    fc::spoil(&mut bad);
    let short = frame(&p, &[0x02, 0x01], 0.25, 0.0, 0.0);
    let x = render(&[bad, short], 1e6, 0.5);
    let rx = receive(Bandwidth::Bw250, &x, 1e6, 868.2e6);
    let f = rx.results().fields();
    assert_eq!(f.get(&keys::FRAMES_OK), Some(0));
    assert_eq!(f.get(&keys::FRAMES_BAD), Some(2));
}

/// The retune requests of a receiver of `d` at its defaults in its first block.
pub fn requests(d: &'static StackDescriptor) -> usize {
    let mut rx = Harness::new(d).receiver().expect("the receiver");
    let dial = dial(d);
    rx.configure(
        &InputInfo::new(d.tuning.rate_hz, dial + d.tuning.tune_offset_hz, dial).recording(),
    )
    .expect("configure");
    let mut out = Output::default();
    rx.feed(&vec![Complex32::default(); 4096], &mut out);
    out.requests.len()
}

#[test]
fn the_data_file_has_the_plans_of_the_specification_and_its_settings_decode() {
    let file = plan_file("fanet.json");
    // One group for each row of the table of the specification.
    assert_eq!(file.groups.len(), 9);
    assert_eq!((file.countries.len(), file.regions.len()), (9, 3));
    assert!(file.groups.iter().all(|g| g.bands.len() == 1));
    // The row of Australia: 920.8 MHz, 500 kHz. The consumer sets the bandwidth and tunes the session there.
    let (dial, width) = band_of(&file, "AU");
    assert_eq!((dial, width), (920.8e6, 500e3));
    let bw = CaseSetting::new(&fc::BANDWIDTH, Bandwidth::Bw500);
    let x = signal(&FANET, 8.0, std::slice::from_ref(&bw));
    let mut rx = Harness::new(&FANET)
        .with_settings(&[bw])
        .receiver()
        .unwrap();
    let rate = FANET.tuning.rate_hz;
    feed(
        rx.as_mut(),
        &x,
        rate,
        dial + fc::TUNE_OFFSET_HZ,
        dial,
        65536,
    );
    let res = rx.results();
    assert_eq!(res.status().lock_state, sdrkit_core::LockState::Locked);
    assert!(good(rx.as_ref()) >= 9);
    assert_eq!(
        res.fields().at(&FanetFields::LISTENING),
        Some("920.8 MHz, 500 kHz")
    );
    // The stack holds no plan: it asks the host for no frequency.
    assert_eq!(requests(&FANET), 0);
}

#[test]
fn the_test_signal_locks_with_the_default_settings_and_the_state_survives() {
    let dial = dial(&FANET);
    let x = signal(&FANET, 8.0, &[]);
    let mut rx = Harness::new(&FANET).receiver().unwrap();
    let rate = FANET.tuning.rate_hz;
    let ev = feed(
        rx.as_mut(),
        &x,
        rate,
        dial + fc::TUNE_OFFSET_HZ,
        dial,
        65536,
    );
    let res = rx.results();
    assert_eq!(res.status().lock_state, sdrkit_core::LockState::Locked);
    assert!(good(rx.as_ref()) >= 9, "{}", good(rx.as_ref()));
    assert_eq!(res.fields().get(&keys::FRAMES_BAD), Some(0));
    assert_eq!(rows(res, &FANET_STATIONS).len(), 5);
    let kinds: Vec<_> = rows(res, &FANET_FRAMES)
        .iter()
        .filter_map(|r| r.at(&FanetFrameRow::KIND))
        .collect();
    for k in [
        FrameKind::Tracking,
        FrameKind::Ground,
        FrameKind::Service,
        FrameKind::Name,
        FrameKind::Message,
    ] {
        assert!(kinds.contains(&k), "no {k:?}");
    }
    assert!(ev.iter().any(|e| e.is(&FANET_MESSAGE)));
    assert!(ev.iter().filter(|e| e.is(&FANET_STATION)).count() == 5);
    assert!(res.points().keys().any(|k| k.starts_with("weather/")));
    assert!(res.points().keys().any(|k| k.starts_with("people/")));
    assert!(!rows(res, &FANET_TRACKS).is_empty());
    let state = rx.save_state().expect("a saved state");
    let mut again = Harness::new(&FANET).receiver().unwrap();
    feed(again.as_mut(), &[], rate, dial, dial, 65536);
    again.restore_state(&state).unwrap();
    assert_eq!(good(again.as_ref()), good(rx.as_ref()));
    crate::common::invoke(again.as_mut(), "clear", &[]).unwrap();
    assert_eq!(good(again.as_ref()), 0);
}

#[test]
fn every_nth_frame_of_the_test_signal_can_have_a_wrong_crc() {
    let h = Harness::new(&FANET).seconds(4.0).set(&fc::GEN_BAD_CRC, 3);
    let (_, rx) = crate::common::gen_run(&h, &FANET, 65536);
    let f = rx.results().fields();
    assert!(f.get(&keys::FRAMES_BAD).unwrap_or(0) >= 1);
    assert!(f.get(&keys::FRAMES_OK).unwrap_or(0) >= 3);
}

/// A change of frequency keeps the frame log, the stations, their tracks and the counts, and drops the signal to
/// noise ratio, the offset and the level of the present signal.
#[test]
fn a_change_of_frequency_keeps_the_stations_and_drops_the_levels() {
    let dial = dial(&FANET);
    let x = signal(&FANET, 8.0, &[]);
    let mut rx = Harness::new(&FANET).receiver().unwrap();
    let rate = FANET.tuning.rate_hz;
    feed(
        rx.as_mut(),
        &x,
        rate,
        dial + fc::TUNE_OFFSET_HZ,
        dial,
        65536,
    );
    let read = |rx: &dyn Receiver| {
        let f = FanetFields::read(rx.results().fields()).expect("the fields of fanet");
        (
            f.frames_ok,
            f.stations_heard,
            f.snr_db,
            f.offset_hz,
            f.level_dbfs,
        )
    };
    let counts = |rx: &dyn Receiver| {
        let r = rx.results();
        (
            rows(r, &FANET_FRAMES).len(),
            rows(r, &FANET_STATIONS).len(),
            rows(r, &FANET_TRACKS).len(),
        )
    };
    let before = counts(rx.as_ref());
    let (good, stations, snr, offset, level) = read(rx.as_ref());
    assert!(good >= 9 && stations == 5 && before.2 > 0);
    assert!(snr.is_some() && offset.is_some() && level.is_some());
    rx.forget();
    assert_eq!(counts(rx.as_ref()), before);
    assert_eq!(read(rx.as_ref()), (good, stations, None, None, None));
    assert_eq!(rx.results().status().snr_db, None);
}
