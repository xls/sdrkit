//! The NFM meter: five cases of rate, tone, deviation and offset; over-deviation; a tone that stops; a sweep of the noise;
//! noise alone.

use crate::stream::{stream, Streamed};
use sdrkit_algo_sim::chain::{
    NoiseKind, GEN_DRIFT_HZ_S, GEN_LEVEL_DBFS, GEN_NOISE, GEN_NOISE_FLOOR_DBFS, GEN_OFFSET_HZ,
    GEN_SNR_DB,
};
use sdrkit_core::interface::ReadFields;
use sdrkit_dev_test::Harness;
use sdrkit_stack_meter::nfm_meter::gen::{
    voice, GEN_CTCSS_DEV_HZ, GEN_CTCSS_HZ, GEN_CTCSS_OFF_S, GEN_VOICE_HZ,
};
use sdrkit_stack_meter::nfm_meter::DESCRIPTOR;
use sdrkit_stack_meter::typed::{NfmMeterEventRow, NfmMeterFields, NfmMeterKind, NFM_METER_EVENTS};

const TUNED: f64 = 146.52e6;

fn at(cnr: f64) -> Harness {
    Harness::new(&DESCRIPTOR)
        .set(&GEN_NOISE, NoiseKind::Snr)
        .set(&GEN_SNR_DB, cnr)
        .set(&GEN_DRIFT_HZ_S, 3.0)
}

fn rows(r: &Streamed) -> Vec<NfmMeterEventRow> {
    NFM_METER_EVENTS
        .rows::<NfmMeterEventRow>(&r.results)
        .map(|(_, e)| e)
        .collect()
}

fn fields(r: &Streamed) -> NfmMeterFields {
    NfmMeterFields::read(r.results.fields()).unwrap()
}

#[test]
fn five_cases_read_the_voice_peak_the_tone_and_its_deviation() {
    let peak = (0..20 * 48_000)
        .map(|i| voice(i as f64 / 48e3).abs())
        .fold(0.0, f64::max)
        * 1600.0;
    let cases = [
        (48e3, 100.0, 400.0, 1500.0),
        (96e3, 123.0, 350.0, -800.0),
        (250e3, 67.0, 500.0, 2500.0),
        (48e3, 254.1, 600.0, -2000.0),
        (48e3, 88.5, 380.0, 0.0),
    ];
    for (k, (rate, tone, dev, off)) in cases.into_iter().enumerate() {
        let seed = 1 + k as u64;
        let h = at(30.0)
            .set(&GEN_CTCSS_HZ, tone)
            .set(&GEN_CTCSS_DEV_HZ, dev)
            .set(&GEN_OFFSET_HZ, off)
            .seed(seed);
        let f = fields(&stream(&DESCRIPTOR, &h, rate, TUNED, 10.0, seed, |_, _| {}));
        let (v, c, s, d, o) = (
            f.voice_peak_khz.unwrap() * 1000.0,
            f.ctcss_hz.unwrap().0,
            f.ctcss_standard_hz.unwrap().0,
            f.ctcss_deviation_hz.unwrap().0,
            f.carrier_offset_hz.unwrap().0,
        );
        println!("{rate} sps, {tone} Hz: voice {v:.0} (sent {peak:.0}), CTCSS {c:.3} ({s}) at {d:.1} Hz, offset {o:.1}");
        assert!((v - peak).abs() <= 0.08 * peak, "{rate} {tone}: voice {v}");
        assert!(
            (c - tone).abs() <= 0.4 && (s - tone).abs() < 0.05,
            "{rate} {tone}: {c} {s}"
        );
        assert!((d - dev).abs() <= 0.08 * dev, "{rate} {tone}: {d}");
        assert!((o - (off + 15.0)).abs() <= 40.0, "{rate} {tone}: {o}");
    }
}

#[test]
fn a_loud_voice_is_over_the_limits() {
    for seed in [11, 12] {
        let h = at(30.0)
            .set(&GEN_CTCSS_HZ, 0.0)
            .set(&GEN_VOICE_HZ, 3500.0)
            .seed(seed);
        let r = stream(&DESCRIPTOR, &h, 48e3, TUNED, 10.0, seed, |_, _| {});
        assert!(fields(&r).ctcss_hz.is_none());
        let over = rows(&r)
            .iter()
            .filter(|e| matches!(e.kind, NfmMeterKind::OverNarrow | NfmMeterKind::OverWide))
            .count();
        assert!(over >= 1, "{:?}", rows(&r));
        let h = at(30.0)
            .set(&GEN_CTCSS_HZ, 0.0)
            .set(&GEN_VOICE_HZ, 6500.0)
            .seed(seed);
        let f = fields(&stream(&DESCRIPTOR, &h, 48e3, TUNED, 10.0, seed, |_, _| {}));
        assert!(f.over_wide >= 3, "{f:?}");
    }
}

#[test]
fn a_tone_that_stops_gives_a_gone_row() {
    for seed in [21, 22] {
        let h = at(30.0).set(&GEN_CTCSS_OFF_S, 6.0).seed(seed);
        let mut r = rows(&stream(&DESCRIPTOR, &h, 48e3, TUNED, 14.0, seed, |_, _| {}));
        r.sort_by(|a, b| a.time_s.0.total_cmp(&b.time_s.0));
        let kinds: Vec<_> = r
            .iter()
            .map(|e| e.kind)
            .filter(|k| matches!(k, NfmMeterKind::Tone | NfmMeterKind::ToneGone))
            .collect();
        assert_eq!(
            kinds,
            vec![NfmMeterKind::Tone, NfmMeterKind::ToneGone],
            "{r:?}"
        );
    }
}

#[test]
fn the_tone_holds_down_to_15_db() {
    for (k, cnr) in [30.0, 20.0, 15.0, 12.0, 10.0].into_iter().enumerate() {
        let seed = 31 + k as u64;
        let f = fields(&stream(
            &DESCRIPTOR,
            &at(cnr).seed(seed),
            48e3,
            TUNED,
            10.0,
            seed,
            |_, _| {},
        ));
        println!("{cnr} dB: CTCSS {:?}", f.ctcss_hz);
        if cnr >= 15.0 {
            let c = f.ctcss_hz.unwrap().0;
            assert!((c - 100.0).abs() <= 0.5, "{cnr}: {c}");
        }
    }
}

#[test]
fn noise_alone_shows_no_tone_no_voice_peak_and_no_row() {
    for seed in [41, 42, 43] {
        let h = Harness::new(&DESCRIPTOR)
            .set(&GEN_NOISE, NoiseKind::Floor)
            .set(&GEN_LEVEL_DBFS, -150.0)
            .set(&GEN_NOISE_FLOOR_DBFS, -20.0)
            .seed(seed);
        let r = stream(&DESCRIPTOR, &h, 48e3, TUNED, 10.0, seed, |_, _| {});
        let f = fields(&r);
        assert!(
            f.ctcss_hz.is_none() && f.voice_peak_khz.is_none() && rows(&r).is_empty(),
            "{f:?} {:?}",
            rows(&r)
        );
    }
}
