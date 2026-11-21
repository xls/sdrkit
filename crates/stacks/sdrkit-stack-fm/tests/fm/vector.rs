//! The recorded test vector (`tests/vectors/fm/`) must decode as `expected.json` says, in blocks of any size.

use super::common::*;
use sdrkit_core::owner::Owner;
use sdrkit_core::{Output, Receiver};
use sdrkit_dev_test::sigmf;

fn feed_in_blocks(
    samples: &[sdrkit_core::Complex32],
    rate: f64,
    centre: f64,
    sizes: &mut dyn FnMut() -> usize,
) -> (Rx, Vec<f32>) {
    let mut rx = receiver(rate, centre, &[("rds_variant", "auto")]);
    let mut out = Output::default();
    let mut audio = Vec::new();
    let mut i = 0;
    while i < samples.len() {
        let n = sizes().clamp(1, samples.len() - i);
        out.clear();
        rx.feed(&samples[i..i + n], &mut out);
        stereo(&out, &mut audio);
        i += n;
    }
    (rx, audio)
}

#[test]
fn the_recording_decodes_as_expected() {
    let Some(rec) = sdrkit_dev_test::read_recording("fm/fm_stereo_rds_500k") else {
        return;
    };
    let exp = sigmf::read_expected("fm").expect("expected.json");
    let block = 50_000;
    // Report by report, in blocks of 50 000 samples (0.1 s).
    let mut rx = receiver(rec.rate_hz, rec.centre_hz, &[]);
    let mut out = Output::default();
    let mut audio = Vec::new();
    let mut reports = Vec::new();
    for b in rec.samples.chunks(block) {
        out.clear();
        rx.feed(b, &mut out);
        assert!(out.audio_runs().all(|p| p.rate_hz == 48_000.0));
        stereo(&out, &mut audio);
        reports.push(detail(&rx));
    }
    let d = detail(&rx);
    let rds = &exp["rds"];
    let sig = &exp["signal"];
    assert_eq!(format!("{:04X}", d.pi), rds["pi"].as_str().unwrap());
    assert_eq!(d.station, rds["ps"].as_str().unwrap());
    assert_eq!(d.text, rds["rt"].as_str().unwrap());
    assert_eq!(d.text_complete, rds["rt_complete"].as_bool().unwrap());
    assert_eq!(d.pty, Some(rds["pty"].as_u64().unwrap() as u8));
    assert_eq!(d.programme, rds["pty_name"].as_str().unwrap());
    assert_eq!(d.traffic, rds["tp"].as_bool().unwrap());
    assert_eq!(d.ta, rds["ta"].as_bool().unwrap());
    assert_eq!(d.music, rds["music"].as_bool().unwrap());
    assert_eq!(d.rbds, rds["rbds"].as_bool().unwrap());
    assert_eq!(d.call_letters, rds["call_sign"].as_str().unwrap());
    assert_eq!(d.rds, rds["sync"].as_bool().unwrap());
    assert!(d.rds_ok >= 99.0, "usable blocks {}", d.rds_ok);
    assert!(d.groups >= 38, "groups {}", d.groups);
    assert_eq!(d.state, 2, "locked");
    assert_eq!(d.carrier, sig["carrier"].as_bool().unwrap());
    assert_eq!(d.stereo, sig["stereo"].as_bool().unwrap());
    assert!(d.stereo_pct > 99.0, "stereo {} %", d.stereo_pct);
    assert!((7.0..=11.0).contains(&d.pilot_pct), "pilot {}", d.pilot_pct);
    assert!(
        (1000.0..=2000.0).contains(&d.offset_hz),
        "offset {}",
        d.offset_hz
    );
    assert!(d.snr_db >= 40.0, "SNR {}", d.snr_db);
    assert!(
        ((d.deviation_hz / 1e3) - 52.7).abs() < 3.0,
        "deviation {}",
        (d.deviation_hz / 1e3)
    );
    assert!(
        d.multipath_pct < 3.0 && !d.equaliser,
        "multipath {} equaliser {}",
        d.multipath_pct,
        d.equaliser
    );
    // Audio.
    let frames = audio.len() / 2;
    assert!((frames as f64 - 192_000.0).abs() < 200.0, "{frames} frames");
    let (l, r) = sdrkit_dev_test::measure::deinterleave(&audio[audio.len() - 2 * 48_000..]);
    let (l1, l3, r3, r1) = (
        amp(&l, 1000.0),
        amp(&l, 3000.0),
        amp(&r, 3000.0),
        amp(&r, 1000.0),
    );
    assert!((0.30..=0.50).contains(&l1), "left 1 kHz {l1}");
    assert!((0.30..=0.50).contains(&r3), "right 3 kHz {r3}");
    assert!(
        20.0 * (l1 / l3).log10() >= 30.0 && 20.0 * (r3 / r1).log10() >= 30.0,
        "separation {l1}/{l3}, {r3}/{r1}"
    );
    // Timing.
    let first = |f: &dyn Fn(&sdrkit_stack_fm::Reading) -> bool| {
        reports.iter().position(f).map(|i| (i + 1) as f64 * 0.1)
    };
    let t_ps = first(&|d| d.station == "TUNEIT  ").unwrap();
    let t_rt = first(&|d| d.text_complete).unwrap();
    let t_lock = first(&|d| d.state == 2 && d.stereo && d.rds).unwrap();
    assert!(t_ps <= 1.5, "PS after {t_ps} s");
    assert!(t_rt <= 3.9, "RT complete after {t_rt} s");
    assert!(t_lock <= 0.3, "lock, stereo and RDS after {t_lock} s");
    eprintln!("vector: PS {t_ps:.1} s, RT {t_rt:.1} s, lock {t_lock:.1} s, SNR {:.1}, offset {:.0}, pilot {:.2}, groups {}, dev {:.1}, L {l1:.3}/{l3:.5} R {r3:.3}/{r1:.5}", d.snr_db, d.offset_hz, d.pilot_pct, d.groups, (d.deviation_hz / 1e3));
}

#[test]
fn blocks_of_any_size_give_the_same_result() {
    let Some(rec) = sdrkit_dev_test::read_recording("fm/fm_stereo_rds_500k") else {
        return;
    };
    let x = &rec.samples[..rec.samples.len() / 2];
    let (a, audio_a) = feed_in_blocks(x, rec.rate_hz, rec.centre_hz, &mut || 65_536);
    let (b, audio_b) = feed_in_blocks(x, rec.rate_hz, rec.centre_hz, &mut || 777);
    let mut rng = sdrkit_algo_sim::rng::Rng::new(9);
    let (c, audio_c) = feed_in_blocks(x, rec.rate_hz, rec.centre_hz, &mut || {
        1 + rng.below(30_000) as usize
    });
    assert_eq!(Owner::results(&a), Owner::results(&b));
    assert_eq!(Owner::results(&a), Owner::results(&c));
    assert_eq!(audio_a, audio_b);
    assert_eq!(audio_a, audio_c);
    assert_eq!(detail(&a).station, "TUNEIT  ");
}
