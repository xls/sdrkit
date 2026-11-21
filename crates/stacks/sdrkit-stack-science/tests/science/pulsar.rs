//! Pulsar folding: the search on filter-bank rows (detection, DM, period, a nominal period off by 1.2e-4, short and long
//! periods, weak pulses and noise), the filter bank on IQ, pieces of any size, the area estimate and the delay, `search_now`
//! and `clear`, and the data file of pulsars.

use sci::pulsar::bank::{FilterBank, Layout};
use sci::pulsar::gen::{rows, PulsarGenerator, RowsParams};
use sci::pulsar::{area_needed_m2, delay_s, search, Outcome, Target, DESCRIPTOR, DM, PERIOD_S};
use sci::typed::{PulsarFields, PULSAR_DETECTION, PULSAR_SEARCHES};
use sdrkit_core::{Complex32, Generator, InputInfo, Output, Receiver};
use sdrkit_dev_test::Harness;
use sdrkit_stack_science as sci;

const PULSARS_CSV: &str = include_str!("../../../../../data/science/pulsars.csv");

/// The radio of the tests: 408 MHz, 2.4 Msps.
const CENTRE: f64 = 408e6;
const RATE: f64 = 2.4e6;

/// Rows of `seconds` of a pulsar of period `p` and DM `dm` with per-channel per-row S/N `snr` and duty `duty`, searched for
/// the target (`p_nominal`, `dm`) with the default trials (DM ± 4 in 9, period ± 2e-4 in 21, 64 bins), in rows of 0.5 ms
/// (0.25 ms for a period below 0.1 s).
fn searched(p: f64, p_nominal: f64, dm: f64, snr: f64, duty: f64, seconds: f64, seed: u64) -> Outcome {
    let sample_ms = if p < 0.1 { 0.25 } else { 0.5 };
    let layout = Layout::new(RATE, 64, sample_ms);
    let cells = rows(&RowsParams {
        centre_hz: CENTRE,
        rate_hz: RATE,
        layout,
        period_s: p,
        dm,
        snr,
        duty,
        seconds,
        seed,
    });
    let target = Target {
        period_s: p_nominal,
        dm,
        ..Target::default()
    };
    search(&cells, &layout, &layout.freqs(CENTRE, RATE), &target, seconds).unwrap()
}

fn rel(a: f64, b: f64) -> f64 {
    (a - b).abs() / b
}

#[test]
fn a_bright_pulsar_in_120_s_of_rows_is_detected() {
    let o = searched(0.714520, 0.714520, 26.76, 0.4, 0.006, 120.0, 1);
    assert!(o.ok, "{:?} {:?}", o.snr, o.period_curve);
    assert!(rel(o.period_s, 0.714520) <= 1.2e-4, "{} {:?}", o.period_s, o.period_curve);
    assert_eq!(o.profile.len(), 64);
    assert_eq!(o.dm_curve.len(), 9);
    assert_eq!(o.period_curve.len(), 21);
    // The peak sits in the middle of the profile.
    let top = (0..64).max_by(|&a, &b| o.profile[a].total_cmp(&o.profile[b])).unwrap();
    assert!((30..=34).contains(&top), "{top} {:?}", o.profile);
}

/// The limits of the specification for the bright pulsar: DM within 1.5 and period within 4e-5. One bin of the profile (11 ms)
/// resolves neither (one DM unit moves the bottom channel by 0.29 ms; 4e-5 of the period moves the pulse by 0.43 bins over
/// 120 s); the search times the pulse over parts of the band and of the time.
#[test]
fn a_bright_pulsar_in_120_s_of_rows_is_found_at_its_dm_and_period() {
    let o = searched(0.714520, 0.714520, 26.76, 0.4, 0.006, 120.0, 1);
    assert!(o.ok, "{:?}", o.snr);
    assert!((o.dm - 26.76).abs() <= 1.5, "{} {:?}", o.dm, o.dm_curve);
    assert!(rel(o.period_s, 0.714520) <= 4e-5, "{} {:?}", o.period_s, o.period_curve);
}

#[test]
fn a_nominal_period_too_high_by_1_2e_4_is_detected() {
    let truth = 0.714520;
    let o = searched(truth, truth * (1.0 + 1.2e-4), 26.76, 0.4, 0.006, 120.0, 2);
    assert!(o.ok, "{:?}", o.snr);
    assert!(rel(o.period_s, truth) <= 1.2e-4, "{} {:?}", o.period_s, o.period_curve);
}

/// The limit of the specification: the true period within 4e-5 (see the case of the bright pulsar).
#[test]
fn a_nominal_period_too_high_by_1_2e_4_finds_the_true_period() {
    let truth = 0.714520;
    let o = searched(truth, truth * (1.0 + 1.2e-4), 26.76, 0.4, 0.006, 120.0, 2);
    assert!(o.ok, "{:?}", o.snr);
    assert!(rel(o.period_s, truth) <= 4e-5, "{} {:?}", o.period_s, o.period_curve);
}

#[test]
fn short_and_long_periods_are_found() {
    for (p, dm, seconds) in [(0.253065, 2.97, 120.0), (1.187911, 4.84, 120.0), (0.033392, 56.77, 60.0)] {
        let o = searched(p, p, dm, 0.5, 0.01, seconds, 3);
        assert!(o.ok, "P {p}: {:?}", o.snr);
        assert!(rel(o.period_s, p) <= 8e-5, "P {p}: {} {:?}", o.period_s, o.period_curve);
    }
}

#[test]
fn weak_pulses_and_noise_are_not_detected() {
    let o = searched(0.714520, 0.714520, 26.76, 0.02, 0.006, 120.0, 4);
    assert!(!o.ok, "{:?}", o.snr);
    for seed in 10..16 {
        let o = searched(0.714520, 0.714520, 26.76, 0.0, 0.006, 100.0, seed);
        assert!(!o.ok, "seed {seed}: {:?}", o.snr);
    }
}

#[test]
fn too_few_rows_give_no_search() {
    let layout = Layout::new(RATE, 64, 0.5);
    let cells = vec![1.0f32; 64 * 199];
    assert!(search(&cells, &layout, &layout.freqs(CENTRE, RATE), &Target::default(), 0.1).is_none());
}

#[test]
fn a_tone_of_0_1_s_gives_0_1_s_of_rows() {
    let x: Vec<Complex32> = (0..240_000)
        .map(|n| {
            let p = std::f64::consts::TAU * 300e3 * n as f64 / RATE;
            Complex32::new(0.5 * p.cos() as f32, 0.5 * p.sin() as f32)
        })
        .collect();
    let r = Harness::new(&DESCRIPTOR).feed(&x).unwrap();
    let rows = r.results.fields().at(&PulsarFields::ROWS_HELD).unwrap() as f64;
    let ms = r.results.fields().at(&PulsarFields::ROW_MS).unwrap();
    assert!((rows * ms * 1e-3 - 0.1).abs() <= 0.002, "{rows} rows of {ms} ms");
}

#[test]
fn pieces_of_any_size_give_the_same_rows() {
    let mut g = PulsarGenerator::new(RATE, 64, 0.1, 0.5, 20.0, 1);
    let mut x = vec![Complex32::default(); 300_000];
    g.generate(&mut x);
    let layout = Layout::new(RATE, 64, 0.5);
    let runs: Vec<Vec<f32>> = [97usize, 1216, 16_384]
        .iter()
        .map(|&n| {
            let mut b = FilterBank::new(layout);
            let mut out = Vec::new();
            for c in x.chunks(n) {
                b.push(c, |r| out.extend_from_slice(r));
            }
            out
        })
        .collect();
    assert_eq!(runs[0].len(), 246 * 64);
    assert_eq!(runs[0], runs[1]);
    assert_eq!(runs[0], runs[2]);
}

#[test]
fn the_area_estimate_and_the_delay() {
    let a = area_needed_m2(1.5, 2.4e6, 80.0, 1.0, 0.03);
    assert!((0.5..=2.0).contains(&a), "{a}");
    assert!(area_needed_m2(1.5, 2.4e6, 80.0, 4.0, 0.03) < 0.55 * a);
    for dm in [1.0, 26.76, 500.0] {
        let want = 4.148808e3 * dm * (1.0 / (407.0f64 * 407.0) - 1.0 / (409.0f64 * 409.0));
        assert!(rel(delay_s(dm, 407e6, 409e6), want) < 1e-12, "{dm}");
    }
}

fn command(rx: &mut dyn Receiver, key: &str) {
    let id = rx.commands().id(key).unwrap();
    let args = rx.commands().def(id).unwrap().new_args();
    sdrkit_core::owner::invoke(rx, id, &args).unwrap();
}

fn table_rows(rx: &dyn Receiver) -> usize {
    let r = rx.results();
    r.table(r.decl().table_id(PULSAR_SEARCHES.key).unwrap())
        .map_or(0, |t| t.len())
}

#[test]
fn the_test_signal_at_1_msps_is_found_by_search_now_and_clear_keeps_the_rows() {
    let rate = 1e6;
    let h = Harness::new(&DESCRIPTOR).rate(rate);
    let mut g = PulsarGenerator::new(rate, 64, 0.1, 0.5, 20.0, 5);
    let mut x = vec![Complex32::default(); 8_000_000];
    g.generate(&mut x);
    let mut rx = h.receiver().unwrap();
    rx.configure(&InputInfo::new(rate, CENTRE, CENTRE).recording())
        .unwrap();
    let mut out = Output::default();
    let mut events = Vec::new();
    for c in x.chunks(65_536) {
        out.clear();
        rx.feed(c, &mut out);
        events.append(&mut out.events);
    }
    // 8 s is before the first search at 20 s.
    assert_eq!(rx.results().fields().at(&PulsarFields::SEARCHES), Some(0));
    command(rx.as_mut(), "search_now");
    let f = rx.results().fields();
    assert_eq!(f.at(&PulsarFields::SEARCHES), Some(1));
    assert_eq!(f.at(&PulsarFields::OK), Some(true), "{}", rx.results().status().summary);
    assert!(f.at(&PulsarFields::SNR).unwrap() >= 7.0);
    assert!(rel(f.at(&PulsarFields::PERIOD_FOUND_S).unwrap(), 0.5) <= 2e-4);
    assert_eq!(table_rows(rx.as_ref()), 1);
    // The event of the detection leaves with the next samples.
    out.clear();
    rx.feed(&x[..1000], &mut out);
    assert_eq!(out.events.iter().filter(|e| e.is(&PULSAR_DETECTION)).count(), 1);

    command(rx.as_mut(), "clear");
    assert_eq!(table_rows(rx.as_ref()), 0);
    assert_eq!(rx.results().fields().at(&PulsarFields::SEARCHES), Some(0));
    // The rows are kept: a search right after the clear finds the pulsar again.
    assert!(rx.results().fields().at(&PulsarFields::ROWS_HELD).unwrap() > 10_000);
    command(rx.as_mut(), "search_now");
    assert_eq!(rx.results().fields().at(&PulsarFields::OK), Some(true));
    assert_eq!(table_rows(rx.as_ref()), 1);
}

#[test]
fn the_first_search_comes_at_20_s_and_a_new_target_drops_the_rows() {
    let rate = 1e6;
    let mut g = PulsarGenerator::new(rate, 64, 0.1, 0.5, 20.0, 6);
    let mut rx = Harness::new(&DESCRIPTOR).rate(rate).receiver().unwrap();
    rx.configure(&InputInfo::new(rate, CENTRE, CENTRE).recording())
        .unwrap();
    let mut out = Output::default();
    let mut detections = 0;
    let mut chunk = vec![Complex32::default(); 100_000];
    for _ in 0..205 {
        g.generate(&mut chunk);
        out.clear();
        rx.feed(&chunk, &mut out);
        detections += out.events.iter().filter(|e| e.is(&PULSAR_DETECTION)).count();
    }
    assert_eq!(rx.results().fields().at(&PulsarFields::SEARCHES), Some(1));
    assert_eq!(detections, 1);
    let id = rx.schema().id(PERIOD_S.key()).unwrap();
    sdrkit_core::owner::apply(rx.as_mut(), id, &sdrkit_core::Value::F64(0.7)).unwrap();
    // After 0.3 s more, only rows of that 0.3 s (at most 586) are held; the results are those of the last report.
    for _ in 0..3 {
        g.generate(&mut chunk);
        out.clear();
        rx.feed(&chunk, &mut out);
    }
    let held = rx.results().fields().at(&PulsarFields::ROWS_HELD).unwrap();
    assert!((1..=586).contains(&held), "{held}");
}

#[test]
fn the_pulsars_of_the_data_file_run_as_settings() {
    let mut n = 0;
    let mut header = false;
    for line in PULSARS_CSV.lines() {
        if line.starts_with('#') {
            continue;
        }
        if !header {
            assert_eq!(line, "name,period_s,dm,flux_400mhz_jy,note");
            header = true;
            continue;
        }
        let cols: Vec<&str> = line.splitn(5, ',').collect();
        assert_eq!(cols.len(), 5, "{line}");
        let p: f64 = cols[1].parse().unwrap();
        let dm: f64 = cols[2].parse().unwrap();
        let flux: f64 = cols[3].parse().unwrap();
        assert!(flux > 0.0);
        let h = Harness::new(&DESCRIPTOR).set(&PERIOD_S, p).set(&DM, dm);
        let rx = h.receiver().unwrap_or_else(|e| panic!("{line}: {e}"));
        assert_eq!(rx.values().unwrap().get(&PERIOD_S), p);
        if cols[0] == "B0833-45" {
            assert_eq!((p, dm), (0.089323, 67.771));
        }
        n += 1;
    }
    assert_eq!(n, 9);
}

#[test]
fn the_end_of_a_recording_runs_a_search_on_the_rows_not_searched() {
    let r = Harness::new(&DESCRIPTOR).rate(1e6).seconds(6.0).run().unwrap();
    let f = r.results.fields();
    assert_eq!(f.at(&PulsarFields::SEARCHES), Some(1));
    assert_eq!(f.at(&PulsarFields::OK), Some(true), "{}", r.results.status().summary);
    assert_eq!(r.events.iter().filter(|e| e.is(&PULSAR_DETECTION)).count(), 1);
}
