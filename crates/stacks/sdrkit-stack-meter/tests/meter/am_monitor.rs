//! The AM monitor: depth, offset, peaks and bandwidth at three rates and three offsets; hum; asymmetry; over-modulation; the
//! carrier rows of a cut; the noise sweep; a plain carrier in noise; white noise alone.

use sdrkit_algo_sim::chain::{
    NoiseKind, GEN_DRIFT_HZ_S, GEN_HUM_DEPTH, GEN_HUM_HZ, GEN_NOISE, GEN_OFFSET_HZ, GEN_SNR_DB,
};
use sdrkit_algo_sim::rng::Rng;
use sdrkit_core::interface::ReadFields;
use sdrkit_core::{Complex32, LockState, Results};
use sdrkit_dev_test::{Harness, Run};
use sdrkit_stack_meter::am_monitor::gen::{
    GEN_ASYMMETRY, GEN_CUT_FROM_S, GEN_CUT_UNTIL_S, GEN_DEPTH1, GEN_DEPTH2,
};
use sdrkit_stack_meter::am_monitor::DESCRIPTOR;
use sdrkit_stack_meter::typed::{
    AmCarrierChange, AmMonitorCarrierEvent, AmMonitorFields, AM_MONITOR_CARRIER, AM_MONITOR_LOG,
    AM_MONITOR_OVER_MODULATION,
};

fn h(rate: f64) -> Harness {
    Harness::new(&DESCRIPTOR).rate(rate).freq(1e6)
}

fn fields(r: &Results) -> AmMonitorFields<'_> {
    AmMonitorFields::read(r.fields()).expect("the fields read")
}

fn noisy(h: Harness, cnr_db: f64) -> Harness {
    h.set(&GEN_NOISE, NoiseKind::Snr).set(&GEN_SNR_DB, cnr_db)
}

fn carrier_changes(run: &Run) -> Vec<AmCarrierChange> {
    run.events
        .iter()
        .filter_map(|e| AM_MONITOR_CARRIER.read::<AmMonitorCarrierEvent>(e))
        .map(|e| e.change)
        .collect()
}

/// The middle of the last second that a run of 5 s measures (seconds end at 1.1, 2.1, 3.1 and 4.1 s).
const LAST_MID_S: f64 = 3.6;

#[test]
fn depth_offset_peaks_and_bandwidth_at_three_rates_and_three_offsets() {
    for rate in [96e3, 250e3, 1e6] {
        for (offset, drift) in [(0.0, 3.0), (350.0, 0.0), (-2200.0, 0.0)] {
            let run = h(rate)
                .seconds(5.0)
                .set(&GEN_OFFSET_HZ, offset)
                .set(&GEN_DRIFT_HZ_S, drift)
                .run()
                .unwrap();
            let f = fields(&run.results);
            let at = format!("{rate} sps, {offset} Hz, {drift} Hz/s");
            let d = f.depth_pct.unwrap();
            assert!((d - 53.9).abs() <= 3.0, "{at}: depth {d}");
            let o = f.carrier_offset_hz.unwrap();
            let want = offset + drift * LAST_MID_S;
            assert!((o - want).abs() <= 1.5, "{at}: offset {o}, {want} expected");
            let (p, n) = (f.peak_pos_pct.unwrap(), f.peak_neg_pct.unwrap());
            assert!((p - 69.0).abs() <= 6.0 && (n - 69.0).abs() <= 6.0, "{at}: peaks {p} {n}");
            assert!(f.over_mod_pct.unwrap() < 0.1, "{at}");
            assert_eq!(f.hum_db, None, "{at}");
            let b = f.bandwidth_khz.unwrap();
            assert!((3.2..=4.5).contains(&b), "{at}: bandwidth {b}");
            assert!(f.carrier);
            assert!(f.text.starts_with("carrier"), "{}", f.text);
            assert!(run.events.iter().all(|e| !e.is(&AM_MONITOR_OVER_MODULATION)), "{at}");
        }
    }
}

#[test]
fn hum_of_3_percent_at_100_hz_and_of_0_6_percent_at_60_hz() {
    for (hum, hz, db, tol) in [(0.03, 100.0, 30.5, 2.0), (0.006, 60.0, 44.4, 2.5)] {
        let run = h(96e3).seconds(5.0).set(&GEN_HUM_DEPTH, hum).set(&GEN_HUM_HZ, hz).run().unwrap();
        let f = fields(&run.results);
        assert_eq!(f.hum_hz, Some(hz));
        let got = f.hum_db.unwrap();
        assert!((got - db).abs() <= tol, "{hz} Hz: {got} dB");
    }
}

#[test]
fn a_raised_positive_half_reads_as_asymmetry() {
    let run = h(96e3).seconds(5.0).set(&GEN_ASYMMETRY, 0.6).run().unwrap();
    let a = fields(&run.results).asymmetry_pct.unwrap();
    assert!((5.0..=40.0).contains(&a), "{a}");
}

#[test]
fn clipped_full_modulation_logs_over_modulation_and_the_default_signal_does_not() {
    let run = h(96e3).seconds(5.0).set(&GEN_DEPTH1, 1.0).set(&GEN_DEPTH2, 0.4).run().unwrap();
    let rows = run.events.iter().filter(|e| e.is(&AM_MONITOR_OVER_MODULATION)).count();
    assert!(rows >= 3, "{rows}");
    assert!(fields(&run.results).over_mod_s >= 3);
    let log = AM_MONITOR_LOG
        .rows::<sdrkit_stack_meter::typed::AmMonitorLogRow>(&run.results)
        .filter(|(_, r)| r.kind == "Over-modulation")
        .count();
    assert_eq!(log, rows);
    let plain = h(96e3).seconds(5.0).run().unwrap();
    assert_eq!(plain.events.iter().filter(|e| e.is(&AM_MONITOR_OVER_MODULATION)).count(), 0);
}

#[test]
fn a_cut_of_the_carrier_gives_present_lost_and_returned() {
    let run = noisy(h(96e3), 60.0)
        .seconds(16.0)
        .set(&GEN_CUT_FROM_S, 5.0)
        .set(&GEN_CUT_UNTIL_S, 11.0)
        .run()
        .unwrap();
    assert_eq!(
        carrier_changes(&run),
        [AmCarrierChange::Present, AmCarrierChange::Lost, AmCarrierChange::Returned]
    );
}

#[test]
fn the_noise_sweep_keeps_depth_and_offset_down_to_30_db() {
    for cnr in [50.0, 40.0, 30.0, 20.0, 10.0] {
        let run = noisy(h(96e3), cnr).seconds(5.0).set(&GEN_OFFSET_HZ, 120.0).run().unwrap();
        let f = fields(&run.results);
        let (d, o) = (f.depth_pct.unwrap_or(f64::NAN), f.carrier_offset_hz.unwrap_or(f64::NAN));
        println!("{cnr} dB: depth {d:.1} %, offset {o:.2} Hz, hum {:?}", f.hum_db);
        if cnr >= 30.0 {
            assert!((d - 53.9).abs() <= 4.0, "{cnr} dB: depth {d}");
            assert!((o - 120.0).abs() <= 2.0, "{cnr} dB: offset {o}");
            assert_eq!(f.hum_db, None, "{cnr} dB");
        }
    }
}

fn plain_carrier_for_ten_minutes(cnr: f64) {
    for seed in 1..=10 {
        let run = noisy(h(96e3), cnr)
            .seed(seed)
            .seconds(60.0)
            .set(&GEN_DEPTH1, 0.0)
            .set(&GEN_DEPTH2, 0.0)
            .run()
            .unwrap();
        assert_eq!(carrier_changes(&run), [AmCarrierChange::Present], "{cnr} dB, seed {seed}");
        assert!(run.events.iter().all(|e| e.is(&AM_MONITOR_CARRIER)), "{cnr} dB, seed {seed}");
        assert_eq!(fields(&run.results).hum_db, None, "{cnr} dB, seed {seed}");
    }
}

#[test]
fn a_plain_carrier_at_40_db_gives_only_carrier_present() {
    plain_carrier_for_ten_minutes(40.0);
}

#[test]
fn a_plain_carrier_at_25_db_gives_only_carrier_present() {
    plain_carrier_for_ten_minutes(25.0);
}

#[test]
fn white_noise_alone_never_locks() {
    for (k, power) in [0.02f64, 0.1, 0.5].into_iter().enumerate() {
        let mut rng = Rng::new(11 + k as u64);
        let a = (power / 2.0).sqrt();
        let x: Vec<Complex32> = (0..(96e3 * 8.0) as usize)
            .map(|_| Complex32::new((a * rng.normal()) as f32, (a * rng.normal()) as f32))
            .collect();
        let run = h(96e3).feed(&x).unwrap();
        assert!(
            run.reports.iter().all(|(_, r)| r.status().lock_state != LockState::Locked),
            "noise power {power}"
        );
        assert!(!fields(&run.results).carrier);
        assert!(run.events.is_empty(), "noise power {power}");
    }
}
