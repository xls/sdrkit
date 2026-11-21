//! ERMES: the data file, the test batch through the receiver over rates, carriers, clocks and noise, the last batch, noise
//! alone and the table commands.

use crate::common::*;
use num_complex::Complex32;
use sdrkit_core::{Generator, InputInfo, LockState};
use sdrkit_dev_test::{Awgn, ClockError, FrequencyOffset};
use sdrkit_stack_trunk::ermes::gen::{batch_words, test_messages, test_system, ErmesGenerator};
use sdrkit_stack_trunk::ermes::code;
use sdrkit_stack_trunk::typed::ERMES_ROWS;
use sdrkit_stack_trunk::ERMES;

const DIAL: f64 = 169.65e6;

/// A plan file of the repository's `data/trunk/` folder, read as the host's plan import reads it.
pub fn plan_file(name: &str) -> sdrkit_data_plans::data::PlanFile {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../data/trunk")
        .join(name);
    let text = std::fs::read_to_string(&path).expect("the data file");
    let mut plans = sdrkit_data_plans::Plans::new();
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

/// One copy of a batch at `rate` with the channel at the centre, 0.05 s before it and 0.3 s after it.
fn one_batch(rate: f64, last: bool) -> Vec<Complex32> {
    let words = if last {
        batch_words(&test_system(code::LAST_BATCH), &[], &[])
    } else {
        batch_words(&test_system(4), &[1000, 200, 30], &test_messages())
    };
    let mut g = ErmesGenerator::new(rate, &words, 0.0, 0.3);
    let len = ((0.05 + words.len() as f64 * 15.0 / 3125.0 + 0.3) * rate) as usize;
    let mut x = vec![Complex32::new(0.0, 0.0); len];
    g.generate(&mut x);
    x
}

/// A receiver fed `x` at `rate`, the channel at the centre.
fn receive(x: &[Complex32], rate: f64) -> (Box<dyn sdrkit_core::Receiver>, Fed) {
    let mut rx = make(&ERMES, &[]);
    configure(rx.as_mut(), rate, DIAL, 0.0);
    let fed = feed(rx.as_mut(), x, 8192);
    let mut out = sdrkit_core::Output::default();
    rx.finish(&mut out);
    (rx, fed)
}

fn check_test_batch(rx: &dyn sdrkit_core::Receiver, what: &str) {
    let r = results(rx);
    assert_eq!(num(r, "batches"), 1.0, "{what}: {}", text(r, "bad_batches"));
    let rows = rows(r, &ERMES_ROWS);
    assert_eq!(rows.len(), 3, "{what}: {rows:?}");
    assert_eq!(rows[0]["text"], "1000 200 30");
    assert_eq!(rows[1]["text"], "HELLO WORLD");
    assert_eq!(rows[1]["address"], "2800853");
    assert!(rows[2]["text"].starts_with("1234567890"));
    assert_eq!(rows[2]["address"], format!("{}", 0x123454));
    assert_eq!(
        text(r, "network"),
        "country 40 operator 2 area 5, cycle 12 subsequence 3 batch E, zone 3 hour 18 day 21"
    );
}

#[test]
fn the_data_file_has_the_channels() {
    let file = plan_file("ermes.json");
    let band = &file.groups[0].bands[0];
    let channels = sdrkit_data_plans::band_channels(band);
    assert_eq!(channels.len(), 16);
    assert_eq!(channels[0].centre_hz, 169.425e6);
    assert_eq!(channels[15].centre_hz, 169.8e6);
}

#[test]
fn the_test_batch_over_rates_carriers_and_clocks() {
    for rate in [250e3, 1e6] {
        for carrier in [-1500.0, 0.0, 1200.0] {
            for ppm in [-50.0, 0.0, 50.0] {
                let x = one_batch(rate, false);
                let mut y = Vec::new();
                ClockError::new(rate, ppm).process(&x, &mut y);
                FrequencyOffset::new(rate, carrier).process(&mut y);
                let (rx, fed) = receive(&y, rate);
                check_test_batch(rx.as_ref(), &format!("{rate} {carrier} {ppm}"));
                assert_eq!(
                    fed.events
                        .iter()
                        .filter(|e| e.kind() == "ermes.message")
                        .count(),
                    2
                );
                assert_eq!(
                    fed.events
                        .iter()
                        .filter(|e| e.kind() == "ermes.batch")
                        .count(),
                    1
                );
            }
        }
    }
}

#[test]
fn the_last_batch_has_no_rows() {
    let (rx, _) = receive(&one_batch(250e3, true), 250e3);
    let r = results(rx.as_ref());
    assert_eq!(num(r, "batches"), 1.0);
    assert!(rows(r, &ERMES_ROWS).is_empty());
}

#[test]
fn noise_sweep() {
    let rate = 250e3;
    let mut lines = Vec::new();
    for snr in [35.0, 25.0, 20.0, 16.0, 13.0, 10.0] {
        let mut ok = 0;
        for seed in 1..=5u64 {
            let mut x = one_batch(rate, false);
            FrequencyOffset::new(rate, 300.0).process(&mut x);
            Awgn::new(1.0, snr, 25e3, rate, seed).process(&mut x);
            let (rx, _) = receive(&x, rate);
            let r = results(rx.as_ref());
            let rows = rows(r, &ERMES_ROWS);
            if num(r, "batches") == 1.0 && rows.len() == 3 && rows[1]["text"] == "HELLO WORLD" {
                ok += 1;
            }
        }
        lines.push(format!("C/N {snr} dB in 25 kHz: {ok} of 5"));
        if snr >= 20.0 {
            assert_eq!(ok, 5, "{lines:?}");
        }
    }
    println!("{}", lines.join("\n"));
}

#[test]
fn noise_gives_no_batch() {
    let rate = 250e3;
    let x = Awgn::with_sigma(0.1, 3).noise((rate * 4.0) as usize);
    let (rx, fed) = receive(&x, rate);
    let r = results(rx.as_ref());
    assert_eq!(num(r, "batches"), 0.0);
    assert!(r.status().lock_state != LockState::Locked);
    assert!(fed.events.is_empty());
}

#[test]
fn the_test_signal_locks_with_the_default_settings() {
    let rate = ERMES.tuning.rate_hz;
    let v = sdrkit_dev_test::Harness::new(&ERMES).values().unwrap();
    let mut g = (ERMES.make_generator.unwrap())(&sdrkit_core::GenConfig::new(rate, v)).unwrap();
    let mut x = vec![Complex32::new(0.0, 0.0); (rate * 3.0) as usize];
    g.generate(&mut x);
    let mut rx = make(&ERMES, &[]);
    configure(rx.as_mut(), rate, DIAL + 50e3, -50e3);
    let fed = feed(rx.as_mut(), &x, 8192);
    let r = results(rx.as_ref());
    assert!(num(r, "batches") >= 2.0, "{}", num(r, "batches"));
    assert_eq!(num(r, "bad_batches"), 0.0);
    assert_eq!(r.status().lock_state, LockState::Locked);
    check_events(
        &kinds(&sdrkit_stack_trunk::ermes::declaration()),
        &fed.events,
        true,
    );
    assert!(text(r, "note").contains("Not confirmed by a public document"));
}

#[test]
fn a_rate_below_the_least_is_refused() {
    let mut rx = make(&ERMES, &[]);
    assert!(rx.configure(&InputInfo::new(48e3, DIAL, DIAL)).is_err());
}

#[test]
fn retune_keeps_clear_empties_and_the_state_round_trips() {
    let rate = 250e3;
    let (mut rx, _) = receive(&one_batch(rate, false), rate);
    check_test_batch(rx.as_ref(), "first");
    let saved = rx.save_state().expect("a saved state");
    rx.retuned(&InputInfo::new(rate, DIAL + 25e3, DIAL + 25e3).recording());
    feed(rx.as_mut(), &vec![Complex32::new(0.0, 0.0); 25_000], 8192);
    assert_eq!(rows(results(rx.as_ref()), &ERMES_ROWS).len(), 3);
    invoke(rx.as_mut(), "clear");
    feed(rx.as_mut(), &vec![Complex32::new(0.0, 0.0); 25_000], 8192);
    assert!(rows(results(rx.as_ref()), &ERMES_ROWS).is_empty());
    assert_eq!(num(results(rx.as_ref()), "batches"), 0.0);
    let mut back = make(&ERMES, &[]);
    configure(back.as_mut(), rate, DIAL, 0.0);
    back.restore_state(&saved).expect("the state is taken");
    feed(back.as_mut(), &vec![Complex32::new(0.0, 0.0); 25_000], 8192);
    assert_eq!(rows(results(back.as_ref()), &ERMES_ROWS).len(), 3);
}
