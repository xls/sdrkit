//! NDB identification in noise: the ident "BND" at 7 WPM on a 400 Hz tone of 80 % depth, a cycle of 5.8 s. The SNR is the mean
//! power over the noise in 300 Hz; the signal is declared 3 kHz wide, so the harness takes the SNR minus 10 dB. The weak path
//! (synchronous tone detection and the guards against false words), the average of the repeats, and noise alone.
use sdrkit_algo_sim::chain::{
    NoiseKind, GEN_CLOCK_PPM, GEN_LEVEL_DBFS, GEN_NOISE, GEN_NOISE_FLOOR_DBFS, GEN_OFFSET_HZ,
    GEN_SNR_DB,
};
use sdrkit_dev_test::Harness;
use sdrkit_stack_avnav::typed::{IdentFields, NdbFields};
use sdrkit_stack_avnav::{ndb, NDB};

const RATE: f64 = 16e3;
/// "BND" at 7 WPM is 27 units of 1.2 / 7 s.
const PAUSE_S: f64 = 5.8 - 27.0 * 1.2 / 7.0;

fn beacon(seconds: f64, seed: u64) -> Harness {
    Harness::new(&NDB)
        .rate(RATE)
        .seconds(seconds)
        .seed(seed)
        .set(&ndb::GEN_IDENT, "BND")
        .set(&ndb::GEN_WPM, 7.0)
        .set(&ndb::GEN_TONE, 400.0)
        .set(&ndb::GEN_DEPTH, 0.8)
        .set(&ndb::GEN_PAUSE, PAUSE_S)
        .set(&ndb::GEN_OTHERS, 0)
}

/// The correct identifications heard.
fn idents(h: &Harness) -> usize {
    let r = h.run().unwrap_or_else(|e| panic!("{e}"));
    r.events_of("morse.word")
        .filter(|e| format!("{e:?}").contains("\"BND\""))
        .count()
}

/// Trials of each case of the weak identification.
const TRIALS: u64 = 40;

/// How many of [`TRIALS`] trials of 40 s at `snr_db` have half the correct identifications of a clean run or more.
fn passing(snr_db: f64, offset_hz: f64, ppm: f64, sync: bool, clean: usize) -> usize {
    (1..=TRIALS)
        .filter(|&s| {
            let h = beacon(40.0, s)
                .set(&GEN_NOISE, NoiseKind::Snr)
                .set(&GEN_SNR_DB, snr_db - 10.0)
                .set(&GEN_OFFSET_HZ, offset_hz)
                .set(&GEN_CLOCK_PPM, ppm)
                .set(&ndb::SYNC_TONES, sync);
            2 * idents(&h) >= clean
        })
        .count()
}

/// Synchronous tone detection: 9 of 10 trials pass at 9 dB (36 of 40 seeds), also with 8 Hz of carrier error, a clock 50 ppm
/// off and the carrier 60 Hz off centre. The envelope does not reach 9 dB.
#[test]
fn ndb_reads_a_weak_ident_with_synchronous_tone_detection() {
    let clean = idents(&beacon(40.0, 1));
    assert!(clean >= 6, "{clean} identifications in a clean run");
    for (offset, ppm) in [(0.0, 0.0), (8.0, 0.0), (0.0, 50.0), (60.0, 0.0)] {
        let n = passing(9.0, offset, ppm, true, clean);
        println!("{offset} Hz, {ppm} ppm: {n} of {TRIALS}");
        assert!(
            10 * n >= 9 * TRIALS as usize,
            "{offset} Hz, {ppm} ppm: {n} of {TRIALS}"
        );
    }
    let n = passing(9.0, 0.0, 0.0, false, clean);
    println!("envelope: {n} of {TRIALS}");
    assert!(10 * n < 9 * TRIALS as usize, "envelope: {n} of {TRIALS}");
}

/// The identification from the average of its repeats: of 12 trials of 60 s at 9, 7, 6, 5, 4 and 3 dB at least 12, 12, 10, 5,
/// 6 and 2 show "BND" at the end (the shares of the specification: 100, 100, 83, 42, 50 and 17 %), and none shows another
/// word.
#[test]
fn ndb_reads_a_weaker_ident_from_the_average_of_its_repeats() {
    for (snr_db, need) in [
        (9.0, 12),
        (7.0, 12),
        (6.0, 10),
        (5.0, 5),
        (4.0, 6),
        (3.0, 2),
    ] {
        let mut right = 0;
        for s in 1..=12u64 {
            let r = beacon(60.0, 200 + s)
                .set(&GEN_NOISE, NoiseKind::Snr)
                .set(&GEN_SNR_DB, snr_db - 10.0)
                .run()
                .unwrap_or_else(|e| panic!("{e}"));
            match r.results.fields().at(&IdentFields::IDENT) {
                Some("BND") => right += 1,
                Some(other) => panic!("{snr_db} dB, trial {s}: {other}"),
                None => {}
            }
        }
        println!("{snr_db} dB: {right} of 12");
        assert!(right >= need, "{snr_db} dB: {right} of 12, need {need}");
    }
}

/// With the average switched off the results of the average are absent, and a beacon at 5 dB is not identified.
#[test]
fn ndb_average_is_a_setting() {
    let weak = |average: bool| {
        beacon(60.0, 201)
            .set(&GEN_NOISE, NoiseKind::Snr)
            .set(&GEN_SNR_DB, 5.0 - 10.0)
            .set(&ndb::IDENT_AVERAGE, average)
            .run()
            .unwrap_or_else(|e| panic!("{e}"))
    };
    let on = weak(true);
    let f = on.results.fields();
    assert_eq!(f.at(&NdbFields::IDENT_FOLD_TEXT), Some("BND"));
    let period = f.at(&NdbFields::IDENT_PERIOD_S).unwrap_or(f64::NAN);
    assert!((period - 5.8).abs() < 0.05, "{period}");
    assert!(f.at(&NdbFields::IDENT_PERIOD_Z).is_some_and(|z| z >= 3.0));
    assert!(f
        .at(&NdbFields::IDENT_FOLD_QUALITY)
        .is_some_and(|q| q >= 6.0));
    let off = weak(false);
    let f = off.results.fields();
    assert_eq!(f.at(&NdbFields::IDENT_FOLD_TEXT), None);
    assert_eq!(f.at(&NdbFields::IDENT_PERIOD_S), None);
    assert_eq!(f.at(&IdentFields::IDENT), None);
}

/// Noise alone, 12 runs of 120 s: no identification and no word.
#[test]
fn ndb_reads_nothing_in_noise_alone() {
    for s in 1..=12 {
        let r = beacon(120.0, 100 + s)
            .set(&GEN_LEVEL_DBFS, -150.0)
            .set(&GEN_NOISE, NoiseKind::Floor)
            .set(&GEN_NOISE_FLOOR_DBFS, -20.0)
            .run()
            .unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(r.events_of("morse.word").count(), 0, "run {s}");
        assert_eq!(r.results.fields().at(&IdentFields::IDENT), None, "run {s}");
    }
}

/// Noise alone for 900 s, three seeds: no identification, no word and no reading of the average. The average is evaluated 180
/// times in each run, on three minutes of noise once they are full.
#[test]
fn ndb_identifies_nothing_in_900_s_of_noise() {
    for s in 1..=3 {
        let r = beacon(900.0, 300 + s)
            .set(&GEN_LEVEL_DBFS, -150.0)
            .set(&GEN_NOISE, NoiseKind::Floor)
            .set(&GEN_NOISE_FLOOR_DBFS, -20.0)
            .run()
            .unwrap_or_else(|e| panic!("{e}"));
        let f = r.results.fields();
        assert_eq!(f.at(&IdentFields::IDENT), None, "run {s}");
        assert_eq!(f.at(&NdbFields::IDENT_FOLD_TEXT), None, "run {s}");
        assert_eq!(r.events_of("morse.word").count(), 0, "run {s}");
    }
}
