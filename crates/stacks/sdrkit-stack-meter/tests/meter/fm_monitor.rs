//! The FM monitor: the readings at three rates and three offsets, mono, over-deviation, the changes of the pilot, RDS and a
//! subcarrier, the noise sweep, a plain carrier in noise, white noise alone, and the floating MPX power.

use sdrkit_algo_sim::chain::{NoiseKind, GEN_DRIFT_HZ_S, GEN_NOISE, GEN_OFFSET_HZ, GEN_SNR_DB};
use sdrkit_algo_sim::rng::Rng;
use sdrkit_core::interface::ReadFields;
use sdrkit_core::{Complex32, Generator, LockState, Results};
use sdrkit_dev_test::{Harness, Run};
use sdrkit_stack_meter::fm_monitor::gen::{
    FmGenerator, Mpx, GEN_AUDIO_KHZ, GEN_PILOT_KHZ, GEN_PILOT_UNTIL_S, GEN_RDS_KHZ, GEN_RDS_UNTIL_S, GEN_SCA67_KHZ,
    GEN_SCA92_KHZ, GEN_SCALE, GEN_SCA_FROM_S, GEN_STEREO_KHZ, GEN_TONE2_KHZ,
};
use sdrkit_stack_meter::fm_monitor::DESCRIPTOR;
use sdrkit_stack_meter::typed::{
    FmMonitorFields, FmMonitorLogRow, FmMonitorPilotEvent, FmPilotChange, FmPilotRange, FM_MONITOR_LOG,
    FM_MONITOR_OVER_DEVIATION, FM_MONITOR_PILOT, FM_MONITOR_RDS, FM_MONITOR_SCA,
};

fn h(rate: f64) -> Harness {
    Harness::new(&DESCRIPTOR).rate(rate).freq(100e6).seed(1)
}

fn fields(r: &Results) -> FmMonitorFields<'_> {
    FmMonitorFields::read(r.fields()).expect("the fields read")
}

fn noisy(h: Harness, cnr_db: f64) -> Harness {
    h.set(&GEN_NOISE, NoiseKind::Snr).set(&GEN_SNR_DB, cnr_db)
}

fn kinds(run: &Run) -> Vec<String> {
    let mut rows: Vec<_> = FM_MONITOR_LOG
        .rows::<FmMonitorLogRow>(&run.results)
        .map(|(_, r)| format!("{} {}", r.kind, r.identity))
        .collect();
    rows.reverse();
    rows
}

/// The largest |MPX| of `seconds` of the multiplex `mpx` at `rate` from seed 1, Hz.
fn true_peak(rate: f64, mpx: Mpx, seconds: f64) -> f64 {
    let mut g = FmGenerator::new(rate, mpx, 1);
    let mut x = vec![Complex32::default(); (rate * seconds) as usize];
    g.generate(&mut x);
    g.peak_hz()
}

/// The middle of the last second that a run of 6 s measures (seconds end at 1.1 to 5.1 s).
const LAST_MID_S: f64 = 4.6;

#[test]
fn the_readings_at_three_rates_and_three_offsets() {
    let mpx = Mpx {
        sca: (4e3, 3e3),
        ..Mpx::default()
    };
    for rate in [250e3, 500e3, 1e6] {
        let peak = true_peak(rate, mpx.clone(), 6.0);
        for (offset, drift) in [(0.0, 400.0), (12e3, 0.0), (-35e3, 0.0)] {
            let run = h(rate)
                .seconds(6.0)
                .set(&GEN_SCA67_KHZ, 4.0)
                .set(&GEN_SCA92_KHZ, 3.0)
                .set(&GEN_OFFSET_HZ, offset)
                .set(&GEN_DRIFT_HZ_S, drift)
                .run()
                .unwrap();
            let at = format!("{rate} sps, {offset} Hz, {drift} Hz/s");
            let read = run
                .reports
                .iter()
                .filter_map(|(_, r)| FmMonitorFields::read(r.fields()).and_then(|f| f.peak_deviation_khz))
                .fold(0.0, f64::max);
            assert!((read * 1e3 - peak).abs() <= 0.15 * peak, "{at}: peak {read} kHz, {peak} Hz true");
            let f = fields(&run.results);
            let pilot = f.pilot_khz.unwrap();
            assert!((pilot - 7.5).abs() <= 0.4, "{at}: pilot {pilot}");
            let rds = f.rds_khz.unwrap();
            assert!((rds - 2.0).abs() <= 0.35, "{at}: RDS {rds}");
            let lr = f.stereo_band_khz.unwrap();
            assert!((lr - 15.0 / 2f64.sqrt()).abs() <= 0.9, "{at}: L-R {lr}");
            let audio = f.audio_khz.unwrap();
            assert!((audio - 30f64.hypot(12.0)).abs() <= 6.0 && audio > 25.0, "{at}: audio {audio}");
            let o = f.carrier_offset_khz.unwrap() * 1e3;
            let tol = if drift != 0.0 { 2400.0 } else { 1200.0 };
            assert!((o - offset).abs() <= tol, "{at}: offset {o} Hz");
            let want = offset + drift * LAST_MID_S;
            assert!((o - want).abs() <= 1200.0, "{at}: offset {o} Hz, {want} Hz expected");
            assert!(f.rds && f.stereo, "{at}");
            assert!(f.sca67_db.is_some() && f.sca92_db.is_some(), "{at}: {:?} {:?}", f.sca67_db, f.sca92_db);
            assert_eq!(f.pilot, FmPilotRange::InRange, "{at}");
            let summary = &run.results.status().summary;
            assert!(summary.starts_with("stereo"), "{at}: {summary}");
            assert_eq!(run.reports.last().unwrap().1.status().lock_state, LockState::Locked);
        }
    }
}

#[test]
fn no_pilot_no_rds_and_no_subcarrier_read_as_mono() {
    let run = h(250e3).seconds(6.0).set(&GEN_PILOT_KHZ, 0.0).set(&GEN_RDS_KHZ, 0.0).run().unwrap();
    let f = fields(&run.results);
    let summary = &run.results.status().summary;
    assert!(summary.starts_with("mono"), "{summary}");
    assert_eq!((f.sca67_db, f.sca92_db), (None, None));
    assert!(!f.rds && !f.stereo);
    assert_eq!(f.pilot, FmPilotRange::Absent);
    assert_eq!(kinds(&run), ["Pilot absent"]);
}

#[test]
fn a_multiplex_1_6_times_too_strong_is_logged_and_a_normal_one_is_not() {
    let loud = h(250e3).seconds(6.0).set(&GEN_SCALE, 1.6).run().unwrap();
    let rows = loud.events.iter().filter(|e| e.is(&FM_MONITOR_OVER_DEVIATION)).count();
    assert!(rows >= 4, "{rows}");
    assert_eq!(fields(&loud.results).over_deviation_s as usize, rows);
    let normal = h(250e3).seconds(6.0).run().unwrap();
    assert_eq!(normal.events.iter().filter(|e| e.is(&FM_MONITOR_OVER_DEVIATION)).count(), 0);
}

#[test]
fn the_pilot_and_rds_stop_and_a_subcarrier_starts() {
    let run = h(250e3)
        .seconds(11.0)
        .set(&GEN_PILOT_UNTIL_S, 6.0)
        .set(&GEN_RDS_UNTIL_S, 6.0)
        .set(&GEN_SCA_FROM_S, 3.0)
        .set(&GEN_SCA67_KHZ, 4.0)
        .run()
        .unwrap();
    let pilot: Vec<_> = run
        .events
        .iter()
        .filter_map(|e| FM_MONITOR_PILOT.read::<FmMonitorPilotEvent>(e))
        .map(|e| e.change)
        .collect();
    assert_eq!(pilot, [FmPilotChange::Present, FmPilotChange::Lost]);
    assert_eq!(run.events.iter().filter(|e| e.is(&FM_MONITOR_RDS)).count(), 1);
    assert_eq!(run.events.iter().filter(|e| e.is(&FM_MONITOR_SCA)).count(), 1);
    assert_eq!(kinds(&run), ["Pilot present", "SCA 67 kHz on", "Pilot lost", "RDS lost"]);
}

#[test]
fn the_noise_sweep_keeps_the_pilot_and_rds_down_to_30_db() {
    for cnr in [40.0, 35.0, 30.0, 25.0, 20.0, 15.0] {
        let run = noisy(h(250e3), cnr).seconds(6.0).set(&GEN_SCA67_KHZ, 4.0).run().unwrap();
        let f = fields(&run.results);
        let pilot = f.pilot_khz.unwrap();
        println!(
            "{cnr} dB: pilot {pilot:.2} kHz, RDS {}, SCA 67 {:?}, peak {:.1} kHz",
            f.rds,
            f.sca67_db.map(|d| (d * 10.0).round() / 10.0),
            f.peak_deviation_khz.unwrap()
        );
        if cnr >= 30.0 {
            assert!((pilot - 7.5).abs() <= 0.6, "{cnr} dB: pilot {pilot}");
            assert!(f.rds, "{cnr} dB");
        }
    }
}

fn plain_carrier_for_ten_minutes(cnr: f64) {
    for seed in 1..=10 {
        let run = noisy(h(250e3), cnr)
            .seed(seed)
            .seconds(60.0)
            .set(&GEN_AUDIO_KHZ, 0.0)
            .set(&GEN_TONE2_KHZ, 0.0)
            .set(&GEN_PILOT_KHZ, 0.0)
            .set(&GEN_STEREO_KHZ, 0.0)
            .set(&GEN_RDS_KHZ, 0.0)
            .run()
            .unwrap();
        assert_eq!(kinds(&run), ["Pilot absent"], "{cnr} dB, seed {seed}");
    }
}

#[test]
fn a_plain_carrier_at_35_db_logs_only_pilot_absent() {
    plain_carrier_for_ten_minutes(35.0);
}

#[test]
fn a_plain_carrier_at_22_db_logs_only_pilot_absent() {
    plain_carrier_for_ten_minutes(22.0);
}

#[test]
fn white_noise_alone_never_locks() {
    for (k, power) in [0.02f64, 0.1, 0.5].into_iter().enumerate() {
        let mut rng = Rng::new(21 + k as u64);
        let a = (power / 2.0).sqrt();
        let x: Vec<Complex32> = (0..(250e3 * 8.0) as usize)
            .map(|_| Complex32::new((a * rng.normal()) as f32, (a * rng.normal()) as f32))
            .collect();
        let run = h(250e3).feed(&x).unwrap();
        assert!(
            run.reports.iter().all(|(_, r)| r.status().lock_state != LockState::Locked),
            "noise power {power}"
        );
        assert_eq!(fields(&run.results).pilot, FmPilotRange::Absent);
    }
}

#[test]
fn the_floating_mpx_power_needs_60_seconds_and_is_the_mean_power() {
    let run = h(250e3).seconds(62.0).run().unwrap();
    let f = fields(&run.results);
    let one = f.mpx_power_dbr.unwrap();
    let sixty = f.mpx_power_60s_dbr.unwrap();
    assert!((one - sixty).abs() < 0.3, "{one} {sixty}");
    let early = run.reports.iter().find(|(t, _)| (*t - 30.0).abs() < 0.05).unwrap();
    assert_eq!(fields(&early.1).mpx_power_60s_dbr, None);
}
