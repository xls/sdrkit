//! The submode `ppm72` of `rc_links`: 2 to 9 channels in both polarities with carrier offsets and clock errors, the channel
//! times, a noise sweep with jitter and drift, and no frame from noise, a steady carrier or a slow AM.

use sdrkit_algo_sim::chain::{
    NoiseKind, GEN_DRIFT_HZ_S, GEN_HUM_DEPTH, GEN_HUM_HZ, GEN_LEVEL_DBFS, GEN_NOISE, GEN_OFFSET_HZ,
    GEN_SNR_DB,
};
use sdrkit_core::interface::ReadFields;
use sdrkit_dev_test::{Harness, Run};
use sdrkit_stack_drones::rc_links::ppm::{
    GEN_CHANNELS_MS, GEN_FRAME_MS, GEN_JITTER_US, GEN_PPM_CLOCK_PPM, GEN_PPM_INVERT,
};
use sdrkit_stack_drones::rc_links::{Submode, RC_LINKS, SUBMODE};
use sdrkit_stack_drones::typed::{RcLinksPpmFields, RcPpmFrameEvent, RC_PPM_FRAME};

fn h(seconds: f64) -> Harness {
    Harness::new(&RC_LINKS)
        .set(&SUBMODE, Submode::Ppm72)
        .rate(1e6)
        .freq(72.35e6)
        .seconds(seconds)
}

fn fields(r: &Run) -> RcLinksPpmFields<'_> {
    RcLinksPpmFields::read(r.results.fields()).expect("the PPM fields")
}

fn frames(r: &Run) -> Vec<RcPpmFrameEvent<'_>> {
    r.events
        .iter()
        .filter_map(|e| RC_PPM_FRAME.read::<RcPpmFrameEvent>(e))
        .collect()
}

fn channels(n: usize) -> String {
    (0..n)
        .map(|k| format!("{:.1}", 1.0 + 0.1 * k as f64))
        .collect::<Vec<_>>()
        .join(", ")
}

#[test]
fn two_to_nine_channels_read_in_both_polarities_with_offsets_and_clock_errors() {
    for n in [2usize, 4, 6, 8, 9] {
        for invert in [false, true] {
            for (off, ppm) in [(0.0, 0.0), (12e3, 50_000.0), (-15e3, -30_000.0)] {
                let r = h(1.0)
                    .set(&GEN_CHANNELS_MS, &channels(n))
                    .set(&GEN_FRAME_MS, 22.5)
                    .set(&GEN_PPM_INVERT, invert)
                    .set(&GEN_OFFSET_HZ, off)
                    .set(&GEN_PPM_CLOCK_PPM, ppm)
                    .run()
                    .expect("the run");
                let f = fields(&r);
                let at = format!("{n} channels, inverted {invert}, {off} Hz, {ppm} ppm");
                assert!(
                    f.ppm_frames_ok >= 30,
                    "{at}: {} good, {} bad",
                    f.ppm_frames_ok,
                    f.ppm_frames_bad
                );
                assert_eq!(
                    frames(&r).first().map(|e| e.channels),
                    Some(n as i64),
                    "{at}"
                );
                assert_eq!(f.channels, Some(n as i64), "{at}");
            }
        }
    }
}

#[test]
fn channel_times_read_within_20_us() {
    let sent = [1.0, 1.25, 1.5, 1.75, 2.0];
    let r = h(1.0)
        .set(&GEN_CHANNELS_MS, "1.0, 1.25, 1.5, 1.75, 2.0")
        .run()
        .expect("the run");
    let f = fields(&r);
    let got: Vec<f64> = f
        .channel_ms
        .expect("channel times")
        .split(',')
        .map(|v| v.trim().parse().unwrap())
        .collect();
    assert_eq!(got.len(), 5);
    for k in [0, 2, 4] {
        assert!(
            (got[k] - sent[k]).abs() <= 0.02,
            "channel {}: {} for {}",
            k + 1,
            got[k],
            sent[k]
        );
    }
    let pct = f.channel_pct.expect("percent");
    assert!(
        pct.starts_with("-100") || pct.starts_with("-99") || pct.starts_with("-98"),
        "{pct}"
    );
}

#[test]
fn frames_hold_in_noise_with_jitter_and_drift() {
    let mut line = String::new();
    for cnr in [30.0, 20.0, 14.0, 10.0] {
        let r = h(2.0)
            .set(&GEN_CHANNELS_MS, "1.2, 1.4, 1.6, 1.8, 1.5, 1.3")
            .set(&GEN_JITTER_US, 8.0)
            .set(&GEN_OFFSET_HZ, 8e3)
            .set(&GEN_DRIFT_HZ_S, 200.0)
            .set(&GEN_NOISE, NoiseKind::Snr)
            .set(&GEN_SNR_DB, cnr)
            .run()
            .expect("the run");
        let good = fields(&r).ppm_frames_ok;
        line.push_str(&format!("{cnr} dB {good}/100; "));
        if cnr >= 20.0 {
            assert!(good >= 70, "{line}");
        } else if cnr >= 14.0 {
            assert!(good >= 30, "{line}");
        }
    }
    println!("ppm72, 6 channels, 8 us jitter, CNR in 40 kHz: {line}");
}

#[test]
fn noise_a_steady_carrier_and_a_slow_am_give_no_frame() {
    for seed in 1..=3u64 {
        let noise = h(2.0)
            .set(&GEN_CHANNELS_MS, "")
            .set(&GEN_LEVEL_DBFS, -150.0)
            .set(&GEN_NOISE, NoiseKind::Floor)
            .seed(seed)
            .run()
            .expect("the run");
        assert_eq!(fields(&noise).ppm_frames_ok, 0, "noise, seed {seed}");
        assert!(frames(&noise).is_empty(), "noise, seed {seed}");
    }
    // A steady carrier 30 dB over the noise in 40 kHz, and the same carrier with 50 % AM at 7 Hz.
    for depth in [0.0, 0.5] {
        let r = h(1.0)
            .set(&GEN_CHANNELS_MS, "")
            .set(&GEN_HUM_HZ, 7.0)
            .set(&GEN_HUM_DEPTH, depth)
            .set(&GEN_NOISE, NoiseKind::Snr)
            .set(&GEN_SNR_DB, 30.0)
            .run()
            .expect("the run");
        assert_eq!(fields(&r).ppm_frames_ok, 0, "AM depth {depth}");
        assert!(frames(&r).is_empty(), "AM depth {depth}");
    }
}

#[test]
fn the_verify_case_and_the_benchmark_of_ppm72_pass() {
    use sdrkit_core::verify::{Outcome, VerifyOptions};
    let o = VerifyOptions {
        seconds: 1.0,
        clean: false,
        snr_db: 20.0,
    };
    let hooks = RC_LINKS.verify.expect("a verify case");
    let case = (hooks.cases)(&o)
        .into_iter()
        .find(|c| c.name.contains("ppm72"))
        .expect("the ppm72 case");
    let mut h = Harness::new(&RC_LINKS)
        .with_settings(&case.settings)
        .seconds(case.seconds);
    if let Some(snr) = case.snr_db {
        h = h.set(&GEN_NOISE, NoiseKind::Snr).set(&GEN_SNR_DB, snr);
    }
    let r = h.run().expect("the run");
    for line in (case.check.expect("checks"))(&r.results) {
        assert_eq!(
            line.outcome,
            Outcome::Pass,
            "{}: {} {}",
            case.name,
            line.key,
            line.detail
        );
    }
    let b = (RC_LINKS.benchmarks.expect("benchmarks"))()
        .into_iter()
        .find(|b| b.name == "rc_links.ppm72_1m")
        .expect("the PPM benchmark");
    let res = (b.run)(0.5);
    println!(
        "{}: {:.2} % of one core in real time",
        b.name,
        100.0 * res.cpu_fraction_of_realtime
    );
    assert!(res.cpu_fraction_of_realtime > 0.0 && res.cpu_fraction_of_realtime < 1.0);
}
