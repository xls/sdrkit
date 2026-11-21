//! DSRC road toll: the beacon frames at several rates, offsets, depths and noise levels, noise alone, the refused rate and the
//! data file.

use sdrkit_algo_sim::rng::Rng;
use sdrkit_core::interface::{Fields, ReadFields};
use sdrkit_core::{Complex32, InputInfo, Owner, Receiver, SettingSet, StackConfig, Values};
use sdrkit_stack_ism::dsrc::{self, burst, DsrcStackSetting, TEST_DIAL_HZ};
use sdrkit_stack_ism::typed::{DsrcFields, DsrcFrameRow, DSRC_FRAMES};

/// (broadcast, private, bad) frames read from `bursts` bursts at `rate`, moved by `offset_hz`, with complex noise of `sigma`.
fn read(rate: f64, bursts: usize, offset_hz: f64, depth: f32, sigma: f32) -> (i64, i64, i64) {
    let b = burst(rate, 500e3, depth);
    let mut x = Vec::new();
    for _ in 0..bursts {
        x.extend(vec![Complex32::default(); 4000]);
        x.extend_from_slice(&b);
    }
    x.extend(vec![Complex32::default(); 4000]);
    let mut r = Rng::new(3);
    for (i, v) in x.iter_mut().enumerate() {
        let ph = (std::f64::consts::TAU * offset_hz * i as f64 / rate) as f32;
        *v = *v * Complex32::from_polar(1.0, ph)
            + Complex32::new(sigma * r.normal_f32(), sigma * r.normal_f32());
    }
    let mut rx = dsrc::receiver(&StackConfig::new(Values::defaults(
        &DsrcStackSetting::schema(),
    )))
    .unwrap();
    rx.configure(&InputInfo::new(rate, TEST_DIAL_HZ, TEST_DIAL_HZ).recording())
        .unwrap();
    let mut out = sdrkit_core::Output::default();
    rx.feed(&x, &mut out);
    rx.finish(&mut out);
    let f = DsrcFields::read(rx.results().fields()).expect("the DSRC fields");
    (f.broadcast_frames, f.private_frames, f.frames_bad)
}

#[test]
fn both_frames_at_4_5_and_8_msps_and_offsets() {
    for rate in [4e6, 5e6, 8e6] {
        for off in [0.0, 50e3] {
            assert_eq!(read(rate, 4, off, 1.0, 0.01), (4, 4, 0), "{rate} {off}");
        }
    }
}

#[test]
fn half_depth_noise_and_noise_alone() {
    assert_eq!(read(4e6, 4, 0.0, 0.5, 0.01).0, 4);
    // 18 dB carrier to noise in the sample rate: carrier power 1, noise power 10^(-1.8).
    let sigma = (10f32.powf(-1.8) / 2.0).sqrt();
    let (b, p, _) = read(4e6, 4, 0.0, 1.0, sigma);
    assert_eq!(b + p, 8);
    let (b, p, _) = read(4e6, 0, 0.0, 1.0, 0.1);
    assert_eq!(b + p, 0);
}

#[test]
fn nothing_but_counts_and_lengths_is_kept() {
    let mut rx = dsrc::receiver(&StackConfig::new(Values::defaults(
        &DsrcStackSetting::schema(),
    )))
    .unwrap();
    assert!(rx
        .configure(&InputInfo::new(1e6, TEST_DIAL_HZ, TEST_DIAL_HZ))
        .is_err());
    // Time, kind, bytes, control byte and the text that the identifier is not shown: nothing else.
    assert_eq!(DsrcFrameRow::SHAPE.fields.len(), 5);
    let path =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../data/plans/dsrc_toll.json");
    let text = std::fs::read_to_string(path).unwrap();
    let file = sdrkit_data_plans::data::parse("dsrc_toll.json", &text).unwrap();
    assert_eq!(file.groups[0].bands.len(), 4);
}

#[test]
fn a_change_of_frequency_keeps_the_log_and_drops_the_level() {
    let rate = 4e6;
    let mut x = vec![Complex32::default(); 4000];
    x.extend(burst(rate, 500e3, 1.0));
    x.extend(vec![Complex32::default(); 4000]);
    let mut rx = dsrc::receiver(&StackConfig::new(Values::defaults(
        &DsrcStackSetting::schema(),
    )))
    .unwrap();
    rx.configure(&InputInfo::new(rate, TEST_DIAL_HZ, TEST_DIAL_HZ).recording())
        .unwrap();
    let mut out = sdrkit_core::Output::default();
    rx.feed(&x, &mut out);
    rx.finish(&mut out);
    let before = DsrcFields::read(rx.results().fields()).unwrap();
    assert_eq!(before.broadcast_frames, 1);
    assert!(before.level_dbfs.is_some());
    rx.forget();
    let after = DsrcFields::read(rx.results().fields()).unwrap();
    assert_eq!(after.broadcast_frames, 1);
    assert_eq!(after.private_frames, 1);
    assert_eq!(after.level_dbfs, None);
    let rows = DSRC_FRAMES.rows::<DsrcFrameRow>(rx.results()).count();
    assert_eq!(rows, 2);
}
