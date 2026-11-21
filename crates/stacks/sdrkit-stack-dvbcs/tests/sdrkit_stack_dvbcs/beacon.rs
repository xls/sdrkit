//! The TV satellite beacon decoder: the recording against `expected.json`, and the test signal through the framework's offset,
//! drift and noise.

use crate::common::*;
use sdrkit_algo_sim::chain::{NoiseKind, GEN_CN0_DBHZ, GEN_DRIFT_HZ_S, GEN_NOISE, GEN_OFFSET_HZ};
use sdrkit_core::verify::CaseSetting;
use sdrkit_stack_dvbcs::beacon::frames::SyncWord;
use sdrkit_stack_dvbcs::beacon::{BeaconState, Coding, SATBEACON};
use sdrkit_stack_dvbcs::typed::{SatbeaconFields as F, SatbeaconFrameRow, SATBEACON_FRAMES};

#[test]
fn recorded_beacon_vector_decodes_as_expected() {
    let rec = sdrkit_dev_test::read_recording("dvbcs/tvbeacon_biphase4800_250ksps")
        .expect("the beacon recording");
    let exp = sdrkit_dev_test::sigmf::read_expected("dvbcs").unwrap();
    let e = &exp["recordings"][3]["expect"];
    let mut rx = Rx::new(&SATBEACON, &[], &recording(rec.rate_hz, rec.centre_hz));
    rx.feed(&rec.samples, 50000);
    rx.dump();
    assert_eq!(rx.at(&F::STATE), Some(BeaconState::FramesFound));
    let t = rx.num("signal_time");
    assert!((rx.num("offset_hz") - (60000.0 - 0.4 * t)).abs() < 5.0);
    assert!((54.0..=58.5).contains(&rx.num("cn0_dbhz")));
    assert!((47900.0..=48100.0).contains(&rx.num("subcarrier")));
    assert!((4798.0..=4802.0).contains(&rx.num("bit_rate_bps")));
    assert_eq!(rx.at(&F::CODING), Some(Coding::Biphase));
    assert_eq!(rx.at(&F::FRAME_BITS), Some(2048));
    assert_eq!(rx.at(&F::SYNC_WORD), Some(SyncWord::Faf320));
    assert_eq!(rx.at(&F::COUNTER_BYTE), Some(4));
    assert!(rx.num("frames_ok") >= 10.0);
    assert_eq!(rx.num("frames_bad"), 0.0);
    assert!(rx.num("first_frames") <= 10.0);
    assert!(rx
        .at(&F::TEMPLATE)
        .is_some_and(|t| t.starts_with(&[0xFA, 0xF3, 0x20, 0xFE])));
    assert_eq!(rx.num("tones"), 2.0);
    // Every listed frame equals the recorded one of the same counter, byte for byte.
    let r = rx.results();
    let frames = r
        .decl()
        .table_id(SATBEACON_FRAMES.key)
        .and_then(|id| r.table(id))
        .expect("the frames table");
    let mut matched = 0;
    for (_, row) in frames.rows_newest_first() {
        let c = row.at(&SatbeaconFrameRow::COUNTER).unwrap();
        let bytes = row.at(&SatbeaconFrameRow::BYTES).unwrap();
        let hex: String = bytes.iter().map(|b| format!("{b:02X}")).collect();
        let want = e["frames"]
            .as_array()
            .unwrap()
            .iter()
            .find(|f| f["counter"] == c);
        if let Some(w) = want {
            assert_eq!(hex, w["hex"].as_str().unwrap(), "counter {c}");
            matched += 1;
        }
        assert_eq!(
            bytes[..255].iter().fold(0u8, |a, &b| a.wrapping_add(b)),
            bytes[255]
        );
    }
    assert!(matched >= 10, "{matched}");
}

#[test]
fn the_test_signal_through_offset_drift_and_noise_decodes_to_frames() {
    let x = generate(
        &SATBEACON,
        250e3,
        12.0,
        &[
            CaseSetting::new(&GEN_OFFSET_HZ, 60e3),
            CaseSetting::new(&GEN_DRIFT_HZ_S, -0.4),
            CaseSetting::new(&GEN_NOISE, NoiseKind::Cn0),
            CaseSetting::new(&GEN_CN0_DBHZ, 60.0),
        ],
    );
    let mut rx = Rx::new(&SATBEACON, &[], &recording(250e3, 11_705.25e6));
    rx.feed(&x, 40_000);
    rx.dump();
    assert_eq!(rx.at(&F::STATE), Some(BeaconState::FramesFound));
    assert!(rx.num("frames_ok") >= 5.0, "{}", rx.num("frames_ok"));
    assert_eq!(rx.at(&F::CODING), Some(Coding::Biphase));
    let spacing = rx.num("pilot_spacing");
    assert!(
        (spacing - 545_642.0).abs() < 50.0 || spacing.is_nan(),
        "{spacing}"
    );
}

#[test]
fn a_change_of_frequency_forgets_the_frames_and_the_measurements() {
    use sdrkit_core::Receiver;
    // The frames are numbered by the learner of the carrier: nothing of the beacon is kept.
    let shape = sdrkit_stack_dvbcs::typed::shape("satbeacon");
    assert_eq!(shape.kept_keys(), Ok(Vec::new()));
    let x = generate(
        &SATBEACON,
        250e3,
        12.0,
        &[
            CaseSetting::new(&GEN_NOISE, NoiseKind::Cn0),
            CaseSetting::new(&GEN_CN0_DBHZ, 60.0),
        ],
    );
    let mut rx = Rx::new(&SATBEACON, &[], &recording(250e3, 11_705.25e6));
    rx.feed(&x, 40_000);
    assert_eq!(rx.at(&F::STATE), Some(BeaconState::FramesFound));
    assert!(rows(rx.results(), "frames") > 0);
    assert!(rows(rx.results(), "decoded") > 0);
    assert!(rows(rx.results(), "lines") > 0);
    rx.rx.forget();
    for table in ["frames", "decoded", "lines"] {
        assert_eq!(rows(rx.results(), table), 0, "{table}");
    }
    // The learner starts again: the next report holds no frame of the carrier before.
    rx.feed(&x[..25_000], 25_000);
    assert_eq!(rx.at(&F::FRAMES_OK), Some(0));
    assert_eq!(rx.at(&F::FIRST_FRAMES), None);
}
