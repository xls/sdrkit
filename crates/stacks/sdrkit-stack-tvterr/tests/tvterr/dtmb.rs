//! DTMB tests: the recording, the generator for every header with noise and offset, the timing loop, noise only, the rate rule;
//! the frame decoder in `decode`.

mod decode;

use super::common::{self, chain, impair, Run};
use sdrkit_core::{Complex32, InputInfo};
use sdrkit_dev_test::sigmf;
use sdrkit_stack_tvterr::dtmb::gen::{DtmbGenParams, DtmbGenerator};
use sdrkit_stack_tvterr::dtmb::pn::Header;
use sdrkit_stack_tvterr::dtmb::{DESCRIPTOR, FSYM_8};

fn run(x: &[Complex32], rate: f64, chunk: usize, width: &str) -> Run {
    let mut rx = common::receiver(&DESCRIPTOR, &[("width", width)]);
    common::run(rx.as_mut(), rate, x, chunk, "")
}

/// A test signal with a carrier offset, a clock error and C/N in the symbol bandwidth.
fn signal(p: DtmbGenParams, seconds: f64, cfo: f64, ppm: f64, cn: Option<f64>) -> Vec<Complex32> {
    let rate = p.rate_hz;
    let fsym = if p.width6 { 5.67e6 } else { FSYM_8 };
    impair(
        &common::generate(DtmbGenerator::new(p).unwrap(), seconds),
        rate,
        &chain(cfo, ppm, cn, fsym),
    )
}

#[test]
fn recording_headers_offset_and_cn() {
    let exp = sigmf::read_expected("tvterr").expect("expected.json");
    let e = &exp["files"]["dtmb-pn420-16qam.sigmf-meta"]["expect"];
    let rec = sdrkit_dev_test::read_recording("tvterr/dtmb-pn420-16qam")
        .unwrap_or_else(|| panic!("the recording tvterr/dtmb-pn420-16qam (set SDRKIT_RECORDINGS)"));
    let r = run(&rec.samples, rec.rate_hz, 65536, "8");
    assert_eq!(
        r.t("header"),
        e["header"].as_str().unwrap_or("").to_lowercase(),
        "{:?}",
        r.results.fields()
    );
    assert_eq!(
        r.b("pn_phase_rotates"),
        e["pn_phase_rotates"].as_bool().unwrap_or(false)
    );
    let cfo = r.f("offset_hz");
    assert!(
        (cfo - e["offset_hz"]["value"].as_f64().unwrap_or(0.0)).abs()
            <= e["offset_hz"]["tolerance"].as_f64().unwrap_or(0.0),
        "cfo {cfo}"
    );
    let cn = r.f("cn_db");
    assert!(
        (cn - e["cn_db"]["value"].as_f64().unwrap_or(0.0)).abs()
            <= e["cn_db"]["tolerance"].as_f64().unwrap_or(0.0),
        "C/N {cn}"
    );
}

#[test]
fn every_header_is_found_with_offset_and_noise() {
    for (h, fixed) in [
        (Header::Pn420, false),
        (Header::Pn945, false),
        (Header::Pn945, true),
        (Header::Pn420, true),
    ] {
        for cfo in [0.0, 19_990.0, -27_300.0] {
            let p = DtmbGenParams {
                header: h,
                fixed_phase: fixed,
                ..Default::default()
            };
            let x = signal(p, 0.3, cfo, 0.0, Some(20.0));
            let r = run(&x, 10e6, 40_000, "8");
            assert_eq!(r.t("header"), h.name().to_lowercase(), "{h:?} {cfo}");
            assert_eq!(r.b("pn_phase_rotates"), !fixed, "{h:?} fixed {fixed}");
            let e = r.f("offset_hz");
            assert!((e - cfo).abs() < 150.0, "{h:?} cfo {cfo}: {e}");
            let cn = r.f("cn_db");
            assert!((cn - 20.0).abs() < 1.5, "{h:?} C/N {cn}");
        }
    }
}

#[test]
fn the_timing_loop_follows_the_clock() {
    // The tracker reads the clock error within 6 ppm and keeps the headers usable while the error lasts.
    for (ppm, rate, cfo) in [
        (100.0, 12.5e6, -12_000.0),
        (-100.0, 12.5e6, -12_000.0),
        (40.0, 10e6, 0.0),
        (0.0, 10e6, 0.0),
    ] {
        let p = DtmbGenParams {
            header: Header::Pn420,
            rate_hz: rate,
            ..Default::default()
        };
        let x = signal(p, 1.2, cfo, ppm, Some(25.0));
        let r = run(&x, rate, 65536, "8");
        assert_eq!(
            r.t("header"),
            "pn420",
            "{ppm} ppm: {:?}",
            r.results.fields()
        );
        let est = r.f("clock_ppm");
        assert!((est - ppm).abs() < 6.0, "{ppm} ppm read as {est}");
        assert!(
            r.f("no_header_pct") < 5.0,
            "{ppm} ppm: {:?}",
            r.results.fields()
        );
    }
}

#[test]
fn pn595_single_carrier_is_found() {
    let p = DtmbGenParams {
        single_carrier: true,
        ..Default::default()
    };
    let x = signal(p, 0.2, 310.0, 0.0, Some(25.0));
    let r = run(&x, 10e6, 65536, "8");
    assert_eq!(r.t("header"), "pn595");
}

/// PN595 through three paths of nearly equal strength within two symbols, at a C/N of 14 dB and a clock 30 ppm fast: the
/// strongest tap of the header fit hops between the paths, and the timing loop still reads the clock and keeps the headers.
#[test]
fn pn595_timing_holds_on_a_group_of_equal_paths() {
    let p = DtmbGenParams {
        single_carrier: true,
        ..Default::default()
    };
    let rate = p.rate_hz;
    let clean = common::generate(DtmbGenerator::new(p).unwrap(), 2.5);
    let turn = |a: f32| Complex32::new(a.cos(), a.sin());
    let x: Vec<Complex32> = (0..clean.len())
        .map(|i| {
            let d = |k: usize| {
                if i >= k {
                    clean[i - k]
                } else {
                    Complex32::new(0.0, 0.0)
                }
            };
            d(0) + d(1) * turn(0.9) * 0.95 + d(2) * turn(2.3) * 0.9
        })
        .collect();
    let x = impair(&x, rate, &chain(-8_000.0, 30.0, Some(14.0), FSYM_8));
    let r = run(&x, rate, 65536, "8");
    assert_eq!(r.t("header"), "pn595", "{:?}", r.results.fields());
    let est = r.f("clock_ppm");
    assert!((est - 30.0).abs() < 6.0, "30 ppm read as {est}");
    assert!(r.f("no_header_pct") < 5.0, "{:?}", r.results.fields());
}

#[test]
fn noise_alone_finds_nothing_and_rates_are_checked() {
    let x = common::noise(3_000_000, 0.04, 4);
    let r = run(&x, 10e6, 65536, "8");
    assert_eq!(r.t("header"), "");
    assert!(!r.b("tracking"));
    let mut rx = common::receiver(&DESCRIPTOR, &[]);
    assert!(
        rx.configure(&InputInfo::new(6e6, 538e6, 538e6)).is_err(),
        "8 MHz at 6 Msps is refused"
    );
    let mut rx6 = common::receiver(&DESCRIPTOR, &[("width", "6")]);
    assert!(
        rx6.configure(&InputInfo::new(6e6, 538e6, 538e6)).is_ok(),
        "6 MHz at 6 Msps is accepted"
    );
}

#[test]
fn six_mhz_signal_is_found_at_six_mhz_only() {
    let p = DtmbGenParams {
        header: Header::Pn420,
        width6: true,
        rate_hz: 8e6,
        ..Default::default()
    };
    let x = signal(p, 0.3, 0.0, 0.0, Some(25.0));
    let r6 = run(&x, 8e6, 65536, "6");
    assert_eq!(r6.t("header"), "pn420");
    assert_eq!(r6.f("width"), 6e6);
    let r8 = run(&x, 8e6, 65536, "8");
    assert_eq!(
        r8.t("header"),
        "",
        "a 6 MHz signal is not found by an 8 MHz receiver"
    );
}

/// Zeroes the stretches `gaps` (start s, length s) of `x` at `rate`, or with `remove` cuts them out.
fn with_gaps(x: &[Complex32], rate: f64, gaps: &[(f64, f64)], remove: bool) -> Vec<Complex32> {
    let mut y = x.to_vec();
    for &(at, len) in gaps.iter().rev() {
        let a = (at * rate) as usize;
        let b = ((at + len) * rate) as usize;
        if remove {
            y.drain(a..b);
        } else {
            y[a..b].fill(Complex32::new(0.0, 0.0));
        }
    }
    y
}

/// The log lines of a run that contain `s`.
fn log_count(r: &Run, s: &str) -> usize {
    r.results
        .log_lines()
        .iter()
        .filter(|l| l.contains(s))
        .count()
}

#[test]
fn the_grid_coasts_over_zero_filled_gaps() {
    // PN945 with a fixed phase, -110 Hz, 8 ppm, 25 dB; five gaps a second of 2 to 10 ms filled with zeros after the noise
    // (3 % of the signal), then 100 ms of zeros.
    let p = DtmbGenParams {
        header: Header::Pn945,
        fixed_phase: true,
        ..Default::default()
    };
    let rate = p.rate_hz;
    let x = signal(p, 1.6, -110.0, 8.0, Some(25.0));
    let gaps = [
        (0.20, 0.002),
        (0.40, 0.006),
        (0.60, 0.010),
        (0.80, 0.004),
        (1.00, 0.008),
        (1.20, 0.100),
    ];
    let filled = run(&with_gaps(&x, rate, &gaps, false), rate, 40_000, "8");
    assert_eq!(
        (
            log_count(&filled, "headers found"),
            log_count(&filled, "lock lost")
        ),
        (1, 0),
        "{:?}",
        filled.results.log_lines()
    );
    assert!(filled.b("tracking"));
    let erased = filled.f("frames_erased");
    // 130 ms of zeros are about 208 frames of 0.625 ms; the frames at the edges of a gap are partly zero and not erased.
    assert!((190.0..=210.0).contains(&erased), "erased {erased}");
    // The same gaps cut out: the grid moves, the frames at each cut are erased, and the lock holds.
    let removed = run(&with_gaps(&x, rate, &gaps, true), rate, 40_000, "8");
    assert!(
        log_count(&removed, "samples lost") >= 1 && log_count(&removed, "lock lost") == 0,
        "{:?}",
        removed.results.log_lines()
    );
    assert!(removed.b("tracking"));
}

/// The raw 8-bit recording `name` in the folder that the variable `SDRKIT_LIVE_RECORDINGS` names, or None. The recordings
/// of real signals are kept outside the repository; without the variable the test is skipped.
fn live_recording(name: &str) -> Option<Vec<Complex32>> {
    let Ok(folder) = std::env::var("SDRKIT_LIVE_RECORDINGS") else {
        eprintln!("skipped: SDRKIT_LIVE_RECORDINGS is not set");
        return None;
    };
    let p = std::path::PathBuf::from(folder).join(name);
    let Ok(bytes) = std::fs::read(&p) else {
        eprintln!("skipped: {name} is not here");
        return None;
    };
    Some(
        bytes
            .as_chunks::<2>()
            .0
            .iter()
            .map(|b| Complex32::new(b[0] as i8 as f32 / 128.0, b[1] as i8 as f32 / 128.0))
            .collect(),
    )
}

/// Plays a recording at 10 Msps and returns the results at its end and the lowest count of good LDPC blocks that one
/// second added after the first two.
fn live(name: &str) -> Option<(sdrkit_core::Results, i64)> {
    let x = live_recording(name)?;
    let mut rx = common::receiver(&DESCRIPTOR, &[("width", "8")]);
    rx.configure(&InputInfo::new(10e6, 539e6, 539e6).recording())
        .expect("rate accepted");
    let mut out = sdrkit_core::Output::default();
    let (mut last, mut least) = (0, i64::MAX);
    for (i, c) in x.chunks(500_000).enumerate() {
        rx.feed(c, &mut out);
        out.clear();
        if i % 20 == 19 {
            let r = Run {
                packets: Vec::new(),
                results: sdrkit_core::Owner::results(rx.as_ref()).clone(),
                events: Vec::new(),
                requests: Vec::new(),
            };
            let good = r.results.status().lock_state;
            let ok = r.f("ldpc_ok") as i64;
            eprintln!(
                "{:4.1} s  {good:?}  cn {:.1}  mer {:.1}  clock {:.2} ppm  offset {:.0}  ldpc {ok}/{}  no header {:.0} %",
                (i + 1) as f64 * 0.05,
                r.f("cn_db"),
                r.f("mer_db"),
                r.f("clock_ppm"),
                r.f("offset_hz"),
                r.f("ldpc_bad"),
                r.f("no_header_pct"),
            );
            if i >= 40 {
                least = least.min(ok - last);
            }
            last = ok;
        }
    }
    Some((sdrkit_core::Owner::results(rx.as_ref()).clone(), least))
}

/// Two recordings of real multiplexes at 10 Msps, a multi-carrier one with PN945 headers and a single-carrier one with
/// PN595 headers: the lock holds to the end of each, and every second decodes LDPC blocks.
#[test]
#[ignore = "needs the recordings of real signals"]
fn live_lock_holds_over_the_whole_recording() {
    for name in ["dtmb1_mpeg2_10M.cs8", "dtmb2_avs_10M.cs8"] {
        let Some((r, least)) = live(name) else {
            return;
        };
        assert_eq!(
            r.status().lock_state,
            sdrkit_core::LockState::Locked,
            "{name}"
        );
        assert!(least > 500, "{name}: a second with {least} good blocks");
    }
}
