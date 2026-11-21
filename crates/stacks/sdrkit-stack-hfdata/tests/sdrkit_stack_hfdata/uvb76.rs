//! UVB-76: the worked example of the train and the break, the buzz train over periods, carrier offsets, carrier to noise
//! ratios and period errors, a break and a long transmission, and no row on noise or a plain carrier.

use crate::common::*;
use crate::senders::*;
use sdrkit_stack_hfdata::uvb76::UVB76;

/// The carrier to noise `cnr` in 4.5 kHz.
fn ch(cnr: Option<f64>, offset_hz: f64, drift_hz_s: f64, seed: u64) -> Channel {
    Channel {
        snr_db: cnr,
        bw_hz: 4500.0,
        offset_hz,
        drift_hz_s,
        seed,
        ..Channel::default()
    }
}

fn run(
    rate: f64,
    secs: f64,
    set: &[(&str, Json)],
    c: &Channel,
) -> (Box<dyn sdrkit_core::Receiver>, Vec<(String, String)>) {
    let x = through(&test_signal(&UVB76, rate, set, secs), rate, c);
    let (rx, _) = run_at(&UVB76, &x, rate, dial_of(rate), &[]);
    let rows = sdrkit_stack_hfdata::typed::SENDER_ROWS
        .rows::<sdrkit_stack_hfdata::typed::SenderRow>(rx.results())
        .map(|(_, r)| (r.kind.to_string(), format!("{} | {}", r.id, r.text)))
        .collect();
    (rx, rows)
}

#[test]
fn the_worked_example_locks_and_logs_one_break() {
    let set = [
        ("gen_period_s", json!(2.4)),
        ("gen_buzz_s", json!(1.2)),
        ("gen_break_at_s", json!(30.0)),
        ("gen_break_s", json!(20.0)),
    ];
    let (rx, rows) = run(24e3, 60.0, &set, &ch(Some(40.0), 0.0, 0.0, 1));
    let train: Vec<&(String, String)> = rows.iter().filter(|r| r.0 == "Buzz train").collect();
    assert_eq!(train.len(), 1, "{rows:?}");
    assert!(
        train[0].1.starts_with("25.0 /min | period 2.40 s, buzz "),
        "{rows:?}"
    );
    let breaks: Vec<&(String, String)> = rows.iter().filter(|r| r.0 == "Train break").collect();
    assert_eq!(breaks.len(), 1, "{rows:?}");
    assert_eq!(breaks[0].1, "7 s | the buzz train stopped");
    assert_eq!(field(rx.as_ref(), "rate_per_min"), Some(25.0));
}

#[test]
fn the_worked_example_row_is_exact() {
    let (_, rows) = run(24e3, 30.0, &[], &ch(Some(40.0), 0.0, 0.0, 1));
    let train: Vec<&(String, String)> = rows.iter().filter(|r| r.0 == "Buzz train").collect();
    assert_eq!(train[0].1, "25.0 /min | period 2.40 s, buzz 1.20 s");
}

/// The train over the periods, carrier offsets, carrier to noise ratios `cnrs` and period errors.
fn train_cases(cnrs: &[f64]) {
    let mut k = 0;
    for period in [2.0, 2.4, 3.0] {
        for (offset, drift) in [(0.0, 0.0), (700.0, 3.0), (-2500.0, 3.0)] {
            for &cnr in cnrs {
                for err in [0.0, 0.02] {
                    k += 1;
                    let p = period * (1.0 + err);
                    let set = [("gen_period_s", json!(p)), ("gen_buzz_s", json!(p / 2.0))];
                    let (rx, _) = run(96e3, 40.0, &set, &ch(Some(cnr), offset, drift, k));
                    let rate = field(rx.as_ref(), "rate_per_min");
                    let len = field(rx.as_ref(), "buzz_s");
                    assert!(
                        rate.is_some_and(|r| (r - 60.0 / p).abs() <= 0.6)
                            && len.is_some_and(|l| (l - p / 2.0).abs() <= 0.12),
                        "period {p} offset {offset} cnr {cnr}: rate {rate:?} length {len:?}"
                    );
                }
            }
        }
    }
}

#[test]
fn the_train_is_measured_over_periods_offsets_levels_and_period_errors() {
    train_cases(&[40.0, 25.0]);
}

#[test]
fn the_train_is_measured_at_15_db() {
    train_cases(&[15.0]);
}

#[test]
fn a_break_and_a_long_transmission_are_logged_once() {
    let set = [
        ("gen_break_at_s", json!(30.0)),
        ("gen_break_s", json!(20.0)),
        ("gen_voice_at_s", json!(60.0)),
        ("gen_voice_s", json!(9.0)),
    ];
    let (_, rows) = run(24e3, 100.0, &set, &ch(Some(30.0), 0.0, 0.0, 2));
    let n = |k: &str| rows.iter().filter(|r| r.0 == k).count();
    assert!(
        n("Train break") == 1 && n("Long transmission") == 1 && n("Buzz train") >= 1,
        "{rows:?}"
    );
}

#[test]
fn noise_and_a_plain_carrier_give_no_row() {
    use sdrkit_stack_hfdata::senders::Tone;
    for (seq, c, rate, what) in [
        (
            vec![Tone::silence(60.0)],
            ch(Some(0.0), 0.0, 0.0, 3),
            24e3,
            "noise",
        ),
        (
            vec![Tone::new(60.0, 0.0)],
            ch(Some(40.0), 0.0, 0.0, 4),
            24e3,
            "a carrier",
        ),
    ] {
        let x = signal(&seq, rate, 0.0, &c);
        let (rx, _) = run_at(&UVB76, &x, rate, 0.0, &[]);
        let rows = sender_rows(rx.as_ref());
        assert!(
            rows.is_empty() && field(rx.as_ref(), "buzzes") == Some(0.0),
            "{what}: {rows:?}"
        );
    }
    for sigma in [0.02f64, 0.1, 0.5] {
        let mut r = sdrkit_algo_sim::rng::Rng::new(5);
        let x: Vec<sdrkit_core::Complex32> = (0..(8.0 * 250e3) as usize)
            .map(|_| r.complex_normal(sigma * sigma * 2.0))
            .collect();
        let (rx, _) = run_at(&UVB76, &x, 250e3, dial_of(250e3), &[]);
        assert!(
            sender_rows(rx.as_ref()).is_empty() && field(rx.as_ref(), "buzzes") == Some(0.0),
            "sigma {sigma}"
        );
    }
}
