//! GBAS VDB: the detector on its test signal (slots, symbol-rate line, SNR), on noise alone, and its data file.

use crate::common::{self, field, noise, offset, rows};
use sdrkit_core::{Complex32, Generator, LockState};
use sdrkit_stack_avdata::gbas_vdb::gen::{slots_of, GbasGenerator};
use sdrkit_stack_avdata::gbas_vdb::{self, slots_busy, NOTE};
use sdrkit_stack_avdata::typed::{GbasVdbFields, GBAS_VDB_BURSTS};

const F: f64 = gbas_vdb::TEST_SIGNAL_HZ;

fn signal(rate: f64, slots: &str, frames: u64, seconds: f64) -> Vec<Complex32> {
    let mut g = GbasGenerator::new(rate, slots_of(slots), frames, 0.0, 3);
    let mut x = vec![Complex32::default(); (rate * seconds) as usize];
    g.generate(&mut x);
    x
}

/// Feeds `x`, then 0.3 s of silence, and returns the results.
fn detect(x: &[Complex32], rate: f64) -> sdrkit_core::Results {
    let mut rx = common::receiver(&gbas_vdb::DESCRIPTOR, rate, F, F, &[]);
    common::run(rx.as_mut(), x, 50_000);
    common::run(
        rx.as_mut(),
        &vec![Complex32::default(); (0.3 * rate) as usize],
        50_000,
    );
    sdrkit_core::owner::Owner::results(rx.as_ref()).clone()
}

/// The power of the non-zero samples.
fn on_power(x: &[Complex32]) -> f64 {
    let on: Vec<f64> = x
        .iter()
        .map(|v| f64::from(v.norm_sqr()))
        .filter(|p| *p > 0.0)
        .collect();
    on.iter().sum::<f64>() / on.len() as f64
}

#[test]
fn three_slots_at_20_db_with_an_offset() {
    let rate = 250e3;
    let mut x = signal(rate, "1 3 5", 10, 5.0);
    offset(&mut x, rate, 500.0);
    let p = on_power(&x);
    noise(&mut x, p, 20.0, rate, rate, 1);
    // The frames end at 5 s: the slots are read before the end, the counters after it.
    let mut rx = common::receiver(&gbas_vdb::DESCRIPTOR, rate, F, F, &[]);
    common::run(rx.as_mut(), &x[..(4.9 * rate) as usize], 50_000);
    let r = sdrkit_core::owner::Owner::results(rx.as_ref());
    assert_eq!(field(r, "slots_busy"), 3.0);
    assert_eq!(r.status().lock_state, LockState::Locked);
    assert_eq!(
        r.status().summary,
        "3 of 8 slots busy, symbol rate 10 500 /s seen; content not decoded"
    );
    common::run(rx.as_mut(), &x[(4.9 * rate) as usize..], 50_000);
    let r = sdrkit_core::owner::Owner::results(rx.as_ref());
    assert!(field(r, "bursts") >= 25.0, "{}", field(r, "bursts"));
    assert!((field(r, "length_ms") - 50.0).abs() < 3.0);
    assert!(field(r, "snr_db") > 15.0);
    assert!(!rows(r, &GBAS_VDB_BURSTS).is_empty());
    assert_eq!(r.fields().at(&GbasVdbFields::NOTE), Some(NOTE));
}

#[test]
fn all_eight_slots_at_15_db() {
    let rate = 250e3;
    let mut x = signal(rate, "0 1 2 3 4 5 6 7", 6, 3.0);
    let p = on_power(&x);
    noise(&mut x, p, 15.0, rate, rate, 2);
    let mut rx = common::receiver(&gbas_vdb::DESCRIPTOR, rate, F, F, &[]);
    common::run(rx.as_mut(), &x[..(2.9 * rate) as usize], 50_000);
    let r = sdrkit_core::owner::Owner::results(rx.as_ref());
    assert_eq!(field(r, "slots_busy"), 8.0);
    assert!(r.status().summary.starts_with("8 of 8 slots busy"));
}

#[test]
fn noise_alone_gives_nothing() {
    let rate = 250e3;
    let mut x = vec![Complex32::default(); (3.0 * rate) as usize];
    noise(&mut x, 1.0, 0.0, rate, rate, 5);
    let r = detect(&x, rate);
    assert_eq!(field(&r, "bursts"), 0.0);
    assert_eq!(field(&r, "slots_busy"), 0.0);
    assert_ne!(r.status().lock_state, LockState::Locked);
    let mut rx = (gbas_vdb::DESCRIPTOR.make_receiver)(&sdrkit_core::StackConfig::new(
        common::values(&gbas_vdb::DESCRIPTOR, &[]),
    ))
    .unwrap();
    assert!(rx
        .configure(&sdrkit_core::InputInfo::new(48e3, F, F).recording())
        .is_err());
    assert!(sdrkit_core::owner::Owner::results(rx.as_ref())
        .status()
        .summary
        .contains("needs a sample rate of at least"));
}

#[test]
fn slots_count_from_the_first_burst() {
    assert_eq!(slots_busy([0.1, 0.1625, 0.6, 0.7255].into_iter()), 3);
    assert_eq!(slots_busy([0.0, 0.0624, 0.1251, 0.4999].into_iter()), 3);
    assert_eq!(slots_busy(std::iter::empty()), 0);
}

#[test]
fn the_data_file_has_the_vdb_channels() {
    let file = crate::plan_file("gbas_vdb.json");
    let b = &file.groups[0].bands[0];
    let n = b.numbering.as_ref().unwrap();
    assert_eq!(n.last - n.first + 1, 398);
    assert_eq!(n.first_centre_hz, 108.025e6);
}

#[test]
fn the_clean_test_signal_through_the_harness() {
    let run = sdrkit_dev_test::Harness::new(&gbas_vdb::DESCRIPTOR)
        .seconds(3.0)
        .run()
        .unwrap();
    assert_eq!(field(&run.results, "slots_busy"), 3.0);
    assert_eq!(run.results.status().lock_state, LockState::Locked);
}
