//! VLF MSK: the worked example of the station hint, the rate and carrier over rates, offsets and drift, a carrier far off
//! the dial, a transmitter keyed on and off with a change of rate, low levels, and no "on" row in noise.

use crate::common::*;
use sdrkit_core::{Complex32, Generator};
use sdrkit_stack_hfdata::vlf_msk::{MskGen, VLF_MSK};

/// The station list of the data file as the setting text.
fn stations() -> String {
    let text = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../../data/vlf/supersid_transmitters.csv"),
    )
    .unwrap();
    text.lines()
        .filter(|l| !l.starts_with('#'))
        .skip(1)
        .filter_map(|l| {
            let mut c = l.splitn(3, ',');
            let (label, f, rest) = (c.next()?, c.next()?, c.next()?);
            let site = rest.trim_start_matches('"');
            let site = &site[..site.find('"')?];
            Some(format!("{f}={label}, {site}"))
        })
        .collect::<Vec<_>>()
        .join(";")
}

/// `secs` of MSK at `rate` (bit rate `r`, carrier `fc` from the dial, on from `from` to `to`) with the dial at the centre.
fn msk(rate: f64, r: f64, fc: f64, from: f64, to: f64, secs: f64, seed: u64) -> Vec<Complex32> {
    let mut g = MskGen::new(rate, r, fc, from, to, 0.0, seed).unwrap();
    let mut x = vec![Complex32::default(); (rate * secs) as usize];
    g.generate(&mut x);
    x
}

fn ch(snr: Option<f64>, drift: f64, seed: u64) -> Channel {
    Channel {
        snr_db: snr,
        bw_hz: 2000.0,
        drift_hz_s: drift,
        seed,
        ..Channel::default()
    }
}

fn run(
    x: &[Complex32],
    rate: f64,
    tuned: f64,
    set: &[(&str, Json)],
) -> (Box<dyn sdrkit_core::Receiver>, Vec<(String, String)>) {
    let mut rx = receiver(&VLF_MSK, set);
    rx.configure(&sdrkit_core::InputInfo::new(rate, tuned, tuned).recording())
        .unwrap();
    feed(rx.as_mut(), x, (rate / 10.0) as usize);
    end(rx.as_mut());
    let rows = sdrkit_stack_hfdata::typed::SENDER_ROWS
        .rows::<sdrkit_stack_hfdata::typed::SenderRow>(rx.results())
        .map(|(_, r)| (r.id.to_string(), r.text.to_string()))
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect();
    (rx, rows)
}

#[test]
fn the_hint_names_the_listed_station() {
    let set = [("stations", json!(stations()))];
    let x = through(
        &msk(24e3, 200.0, 30.0, 0.0, 0.0, 30.0, 1),
        24e3,
        &ch(Some(12.0), 0.0, 1),
    );
    let (rx, rows) = run(&x, 24e3, 24_000.0, &set);
    assert_eq!(
        field_text(rx.as_ref(), "hint"),
        "likely NAA, Cutler, Maine, USA"
    );
    assert!(
        rows[0]
            .1
            .ends_with("near the listed frequency of NAA, Cutler, Maine, USA"),
        "{rows:?}"
    );
    let (rx, _) = run(&x, 24e3, 25_000.0, &set);
    assert_eq!(field_text(rx.as_ref(), "hint"), "");
}

#[test]
fn rate_and_carrier_over_rates_offsets_and_drift() {
    let mut k = 0;
    for rate in [24e3, 48e3] {
        for r in [50.0, 100.0, 200.0, 300.0] {
            for (fc, drift) in [(0.0, 0.5), (140.0, 0.0), (-260.0, 0.0)] {
                k += 1;
                let x = through(
                    &msk(rate, r, fc, 0.0, 0.0, 40.0, k),
                    rate,
                    &ch(Some(12.0), drift, k),
                );
                let (rx, _) = run(&x, rate, 20_000.0, &[]);
                let class = field_text(rx.as_ref(), "signal_class");
                let got_r = field(rx.as_ref(), "bit_rate");
                let got_fc = field(rx.as_ref(), "carrier_hz");
                // With drift, the carrier of the last report (about 38 s) against the value at 34 s.
                let want = fc + drift * 34.0;
                assert!(
                    class.starts_with("MSK")
                        && got_r.is_some_and(|g| (g - r).abs() <= 2.5)
                        && got_fc.is_some_and(|g| (g - want).abs() <= 4.0),
                    "{rate} {r} bit/s at {fc} Hz: {class} {got_r:?} {got_fc:?}"
                );
            }
        }
    }
}

#[test]
fn a_carrier_far_from_the_dial_is_read_when_the_dial_follows_it() {
    let x = through(
        &msk(48e3, 200.0, 600.0, 0.0, 0.0, 40.0, 3),
        48e3,
        &ch(Some(0.0), 0.0, 3),
    );
    let mut rx = receiver(&VLF_MSK, &[]);
    rx.configure(&sdrkit_core::InputInfo::new(48e3, 20_000.0, 20_600.0).recording())
        .unwrap();
    feed(rx.as_mut(), &x, 4800);
    let fc = field(rx.as_ref(), "carrier_hz");
    assert!(fc.is_some_and(|f| f.abs() <= 3.0), "{fc:?}");
}

#[test]
fn on_off_and_a_change_of_rate_are_logged_once() {
    let a = msk(48e3, 50.0, 0.0, 8.0, 24.0, 24.0, 4);
    let mut x = a;
    let mut b = msk(48e3, 100.0, 0.0, 0.0, 8.0, 24.0, 5);
    x.append(&mut b);
    let x = through(&x, 48e3, &ch(Some(12.0), 0.0, 4));
    let (_, rows) = run(&x, 48e3, 20_000.0, &[]);
    let n = |k: &str| rows.iter().filter(|r| r.0 == k).count();
    assert!(
        n("on") == 1 && n("off") == 1 && n("changed") == 1,
        "{rows:?}"
    );
}

#[test]
fn low_levels_keep_the_rate() {
    for snr in [12.0, 6.0, 3.0] {
        let x = through(
            &msk(24e3, 200.0, 0.0, 0.0, 0.0, 40.0, 6),
            24e3,
            &ch(Some(snr), 0.0, 6),
        );
        let (rx, _) = run(&x, 24e3, 20_000.0, &[]);
        let r = field(rx.as_ref(), "bit_rate");
        assert!(
            field_text(rx.as_ref(), "signal_class") == "MSK 200 bit/s"
                && r.is_some_and(|r| (r - 200.0).abs() <= 3.0),
            "{snr} dB: {r:?}"
        );
    }
}

#[test]
fn noise_gives_no_on_row() {
    for seed in 0..30u64 {
        let x = through(
            &vec![Complex32::default(); 24_000 * 40],
            24e3,
            &ch(Some(0.0), 0.0, 100 + seed),
        );
        let (_, rows) = run(&x, 24e3, 20_000.0, &[]);
        assert!(rows.is_empty(), "{seed}: {rows:?}");
    }
}
