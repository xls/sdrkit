//! Sensitivity and lock time targets. Each condition of the threshold table is run a little above its target (0.5 dB in AWGN
//! and in the SFN case, 2 dB on the fading channels, which use the COST 207 profiles and sum-of-sinusoids fading): at least 90
//! % of FIBs pass, and at least 90 % of the access units of the 48 kbit/s EEP 3-A DAB+ service that the 6 s after 1.5 s of lock
//! carry are decoded. The SNR is over 2.048 MHz; `SHOW=1` prints the figures.

use super::common::*;
use sdrkit_core::{Complex32, LockState, StreamKind};
use sdrkit_stack_dab::generator::GenParams;
use sdrkit_stack_dab::typed::*;

const RATE: f64 = 2.048e6;
/// Signal per point: 1.5 s to lock and settle, then 6 s measured.
const SECONDS: f64 = 7.5;

/// The FIB share and the share of the access units expected in the measured 6 s that were decoded.
fn measure(x: &[Complex32]) -> (f64, f64) {
    let r = run(x, RATE, 1, 65_536);
    let lock = r.at(&TimingFields::TIMING_LOCK).unwrap_or(0.0);
    let from = lock + 1.5;
    let to = from + 6.0;
    let units = r
        .units(StreamKind::Aac)
        .iter()
        .filter(|u| u.packet.time >= from && u.packet.time < to)
        .count() as f64;
    // Six access units every 120 ms; the units of the last superframe can still be in the de-interleaver at the end of the input.
    let expected = ((SECONDS - from).min(6.0) / 0.02 - 6.0).max(1.0);
    let au = if r.lock_state() == LockState::Locked {
        units / expected
    } else {
        0.0
    };
    let fic = if r.lock_state() == LockState::Locked {
        fib_share(&r)
    } else {
        0.0
    };
    (fic, au.min(1.0))
}

fn clean() -> Vec<Complex32> {
    default_signal(GenParams::default(), Air::default(), SECONDS)
}

fn noise(x: &mut [Complex32], snr: f64, seed: u64) {
    awgn(x, 0.04, snr, RATE, RATE, seed);
}

/// Paths in (µs, dB), normalised to unit total power, as samples at 2.048 Msps.
fn paths(p: &[(f64, f64)]) -> Vec<(usize, f64)> {
    let total: f64 = p.iter().map(|&(_, db)| 10f64.powf(db / 10.0)).sum();
    let norm = 10.0 * total.log10();
    p.iter()
        .map(|&(us, db)| ((us * 1e-6 * RATE).round() as usize, db - norm))
        .collect()
}

/// COST 207 typical urban, 6 paths.
fn tu6() -> Vec<(usize, f64)> {
    paths(&[
        (0.0, -3.0),
        (0.2, 0.0),
        (0.5, -2.0),
        (1.6, -6.0),
        (2.3, -8.0),
        (5.0, -10.0),
    ])
}

/// COST 207 rural area, 4 paths.
fn ra4() -> Vec<(usize, f64)> {
    paths(&[(0.0, 0.0), (0.2, -2.0), (0.4, -10.0), (0.6, -20.0)])
}

fn check(name: &str, x: &[Complex32], fic_min: Option<f64>, au_min: Option<f64>) {
    let (fic, au) = measure(x);
    if std::env::var("SHOW").is_ok() {
        eprintln!(
            "{name}: FIBs {:.1} %, access units {:.1} %",
            fic * 100.0,
            au * 100.0
        );
    }
    if let Some(m) = fic_min {
        assert!(fic >= m, "{name}: FIB share {fic}");
    }
    if let Some(m) = au_min {
        assert!(au >= m, "{name}: access unit share {au}");
    }
}

/// A condition at the FIC target and at the DAB+ target (each plus `margin`), from a channel applied to the clean signal.
fn condition(
    name: &str,
    fic_db: f64,
    dab_db: f64,
    margin: f64,
    channel: impl Fn(&[Complex32]) -> Vec<Complex32>,
) {
    let x = channel(&clean());
    let mut y = x.clone();
    noise(&mut y, fic_db + margin, 1);
    check(
        &format!("{name}, FIC at {:.1} dB", fic_db + margin),
        &y,
        Some(0.9),
        None,
    );
    let mut y = x;
    noise(&mut y, dab_db + margin, 2);
    check(
        &format!("{name}, DAB+ at {:.1} dB", dab_db + margin),
        &y,
        None,
        Some(0.9),
    );
}

#[test]
fn awgn_thresholds() {
    condition("AWGN", 1.6, 2.9, 0.5, |x| x.to_vec());
    for off in [-5000.0, 5000.0] {
        let x = default_signal(
            GenParams::default(),
            Air {
                offset_hz: off,
                ..Air::default()
            },
            SECONDS,
        );
        condition(&format!("AWGN, carrier {off} Hz"), 1.6, 2.9, 0.5, |_| {
            x.clone()
        });
    }
    for (ppm, dab) in [(-50.0, 2.9), (50.0, 3.0)] {
        condition(&format!("AWGN, clock {ppm} ppm"), 1.6, dab, 0.5, |x| {
            clock(x, ppm)
        });
    }
}

#[test]
fn tu6_thresholds() {
    condition("TU6, 10 Hz", 5.5, 7.5, 2.0, |x| fading(x, &tu6(), 10.0, 21));
    condition("TU6, 60 Hz", 10.5, 8.5, 2.0, |x| {
        fading(x, &tu6(), 60.0, 22)
    });
}

#[test]
fn ra4_and_sfn_thresholds() {
    condition("RA4, 30 Hz", 11.0, 8.0, 2.0, |x| {
        fading(x, &ra4(), 30.0, 23)
    });
    // Two transmitters of equal strength 146 µs (299 samples) apart, static.
    condition("SFN, two 0 dB paths", 2.5, 5.0, 0.5, |x| {
        fading(x, &paths(&[(0.0, 0.0), (146.0, 0.0)]), 0.0, 24)
    });
}

/// Time to lock from a cold start at a random point of the loop, 15 starts each at 4 and 20 dB: sync and the service list within
/// 0.2 s (median) and 0.25 s (90th percentile); first DAB+ audio within 0.6 s and 0.65 s.
#[test]
fn lock_time_from_a_cold_start() {
    let long = default_signal(GenParams::default(), Air::default(), 14.0);
    let mut rng = sdrkit_algo_sim::rng::Rng::new(7);
    for snr in [4.0, 20.0] {
        let (mut lock, mut list, mut audio) = (Vec::new(), Vec::new(), Vec::new());
        for k in 0..15 {
            let start = (rng.uniform() * 12.0 * RATE) as usize;
            let mut x = long[start..start + (1.2 * RATE) as usize].to_vec();
            noise(&mut x, snr, 100 + k);
            let r = run(&x, RATE, 1, 20_480);
            let get = |c: &sdrkit_core::interface::Column<
                sdrkit_core::publish::field::Duration,
            >| { r.at(c).filter(|&v| v > 0.0).unwrap_or(f64::INFINITY) };
            lock.push(get(&TimingFields::TIMING_LOCK).max(get(&TimingFields::TIMING_FIRST_FIB)));
            list.push(get(&TimingFields::TIMING_ENSEMBLE_VALID));
            audio.push(get(&TimingFields::TIMING_FIRST_ACCESS_UNIT));
        }
        let stat = |v: &mut Vec<f64>| {
            v.sort_by(|a, b| a.total_cmp(b));
            (v[7], v[13])
        };
        let (l, s, a) = (stat(&mut lock), stat(&mut list), stat(&mut audio));
        if std::env::var("SHOW").is_ok() {
            eprintln!(
                "{snr} dB: lock {l:?}, service list {s:?}, first audio {a:?} (median, 90th percentile)"
            );
        }
        assert!(l.0 <= 0.2 && l.1 <= 0.25, "{snr} dB: lock {l:?}");
        assert!(s.0 <= 0.2 && s.1 <= 0.25, "{snr} dB: service list {s:?}");
        assert!(a.0 <= 0.6 && a.1 <= 0.65, "{snr} dB: first audio {a:?}");
    }
}
