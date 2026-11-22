//! Radiosonde round trips (generator to receiver), the decoders' checks and the fixes of the family's bug list.

use sci::sonde::frame::{FrameDecoder, SondeType};
use sci::sonde::generator::{SondeGenOptions, SondeGenerator, SondeSpec};
use sci::typed::{SondeCarrierRow, SondeFields, SondeRow, SONDE_CARRIERS, SONDE_FRAME, SONDE_SONDES};
use sdrkit_core::publish::keys::{self, Family};
use sdrkit_core::{Complex32, Generator, Record, Station};
use sdrkit_dev_test::{Harness, Run};
use sdrkit_stack_science as sci;
use sdrkit_util_geo::time::LeapSeconds;

fn spec(model: &str, off: f64, serial: &str, vv: f64) -> SondeSpec {
    SondeSpec {
        model: model.into(),
        offset_hz: off,
        lat: 25.2,
        lon: 55.3,
        alt: 8000.0,
        vv,
        serial: serial.into(),
        frame0: 1000,
        phase_s: 0.1,
    }
}

/// The samples of a scene, with noise at `snr_db` in 10 kHz: the power of one sonde while it sends (1) over the power of the
/// white noise in 10 kHz, `N0 x 10 kHz`. Per bit, `Eb/N0 = snr_db + 10 log10(10 kHz / symbol rate)`: +3.2 dB at 4800 Bd (RS41,
/// RS92), +6.0 dB at 2500 Bd (DFM).
fn scene(rate: f64, sondes: Vec<SondeSpec>, seconds: f64, snr_db: Option<f64>) -> Vec<Complex32> {
    scene_seeded(rate, sondes, seconds, snr_db, 9, 0.0)
}

/// [`scene`] with the noise drawn from `seed` and the sondes' clocks `clock_ppm` slow (made at a rate that much higher).
fn scene_seeded(
    rate: f64,
    sondes: Vec<SondeSpec>,
    seconds: f64,
    snr_db: Option<f64>,
    seed: u64,
    clock_ppm: f64,
) -> Vec<Complex32> {
    let mut g = SondeGenerator::new(
        rate * (1.0 + 1e-6 * clock_ppm),
        SondeGenOptions {
            sondes: Some(sondes),
            ..Default::default()
        },
    );
    let n = (rate * seconds) as usize;
    let mut x = vec![Complex32::new(0.0, 0.0); n];
    g.generate(&mut x);
    if let Some(snr) = snr_db {
        // Each sonde has unit power; the noise is set in 10 kHz.
        let n0 = 10f64.powf(-snr / 10.0) / 10e3;
        let mut awgn = sdrkit_algo_sim::noise::Awgn::with_power(n0 * rate, seed).unwrap();
        awgn.process(&mut x);
    }
    x
}

/// Asserts that every map point of `r` shows a row of table `table` that is there.
fn points_show_rows(r: &sdrkit_core::Results, table: &str) {
    let want = r.decl().table_id(table);
    for p in r.points().values() {
        let link = p.row.as_ref().expect("a row");
        assert_eq!(Some(link.table), want, "{}", p.id);
        let t = r.table(link.table).expect("the table");
        assert!(t.row(&link.key).is_some(), "{}: no row {}", p.id, link.key);
    }
}

/// The rows of the sondes table, the most recently heard first.
fn sondes(r: &Run) -> Vec<Record> {
    points_show_rows(&r.results, SONDE_SONDES.key);
    let id = r.results.decl().table_id(SONDE_SONDES.key).unwrap();
    r.results
        .table(id)
        .unwrap()
        .rows_newest_first()
        .map(|(_, row)| row.clone())
        .collect()
}

/// Runs a scene through the receiver and returns the rows of the sondes heard, the most recently heard first.
fn run(
    rate: f64,
    centre: f64,
    sondes_in: Vec<SondeSpec>,
    seconds: f64,
    snr_db: Option<f64>,
    chunk: usize,
) -> Vec<Record> {
    let x = scene(rate, sondes_in, seconds, snr_db);
    let r = Harness::new(&sci::SONDE)
        .rate(rate)
        .freq(centre)
        .piece(chunk)
        .feed(&x)
        .unwrap();
    sondes(&r)
}

fn find(v: &[Record], kind: SondeType) -> &Record {
    v.iter()
        .find(|s| s.at(&SondeRow::TYPE) == Some(kind))
        .unwrap_or_else(|| {
            panic!(
                "no {kind:?}: {:?}",
                v.iter().map(|s| s.at(&SondeRow::SERIAL)).collect::<Vec<_>>()
            )
        })
}

#[test]
fn each_type_round_trips() {
    for (model, kind, serial, sub) in [
        ("RS41", SondeType::Rs41, "T4710042", "RS41-SG"),
        ("DFM-17", SondeType::Dfm, "17012345", "DFM-17"),
        ("DFM-09", SondeType::Dfm, "19076543", "DFM-09"),
        ("DFM-06", SondeType::Dfm, "0AB123", "DFM-06"),
        ("M10", SondeType::M10, "310-2-11329", "M10"),
        ("M20", SondeType::M20, "211-4-01234", "M20"),
        ("RS92", SondeType::Rs92, "P4953934", "RS92"),
    ] {
        let v = run(
            1e6,
            403e6,
            vec![spec(model, 120e3, serial, -5.0)],
            12.0,
            Some(30.0),
            4096,
        );
        let s = find(&v, kind);
        assert_eq!(s.at(&SondeRow::SERIAL), Some(serial), "{model}");
        assert_eq!(s.at(&SondeRow::SUBTYPE), Some(sub), "{model}");
        let good = s.get(&keys::FRAMES_OK).unwrap();
        let bad = s.get(&keys::FRAMES_BAD).unwrap();
        assert!(good >= 4, "{model}: {good} good");
        assert!(bad <= 1, "{model}: {bad} bad");
        let off = s.at(&SondeRow::OFFSET).unwrap();
        assert!((off - 120e3).abs() < 800.0, "{model}: {off}");
        if kind != SondeType::Rs92 {
            let (lat, _) = s.get(&keys::POSITION).unwrap();
            assert!((lat - 25.2).abs() < 0.01, "{model}");
            let climb = s.at(&SondeRow::CLIMB_M_S).unwrap();
            assert!((climb + 5.0).abs() < 0.3, "{model} {climb}");
            let kt = s.get(&keys::SPEED_KT).unwrap();
            assert!((kt / (3600.0 / 1852.0) - 10.0).abs() < 0.3, "{model}");
        } else {
            assert!(s.get(&keys::POSITION).is_none());
            assert_eq!(s.at(&SondeRow::SATS), Some(9));
        }
    }
}

#[test]
fn lms6_round_trips() {
    let mut s = sci::sonde::generator::lband_sonde();
    s.offset_hz = 100e3;
    let v = run(2e6, 1680e6, vec![s], 6.0, None, 65536);
    let s = find(&v, SondeType::Lms6);
    assert_eq!(s.at(&SondeRow::SERIAL), Some("LMS6-77668800"));
    assert!(s.get(&keys::FRAMES_OK).unwrap() >= 3);
    assert!((s.get(&keys::POSITION).unwrap().0 - 46.77).abs() < 0.01);
}

#[test]
fn the_default_scene_decodes_through_the_harness() {
    let r = Harness::new(&sci::SONDE)
        .rate(2.5e6)
        .seconds(5.0)
        .run()
        .unwrap();
    assert_eq!(
        r.results.fields().at(&SondeFields::SONDES),
        Some(3),
        "{:?}",
        r.results.status().summary
    );
}

/// Every event the receiver emits is a `sonde.frame` with the standard fields, and its frequency is known.
#[test]
fn sonde_events_carry_the_standard_fields() {
    let r = Harness::new(&sci::SONDE)
        .rate(2.5e6)
        .seconds(5.0)
        .run()
        .unwrap();
    assert!(!r.events.is_empty());
    for e in &r.events {
        assert!(e.is(&SONDE_FRAME), "{}", e.kind());
        assert!(e.freq_hz != 0.0, "{e:?}");
        assert!(e.get(&keys::SOURCE).is_some());
        assert_eq!(e.get(&keys::FAMILY), Some(Family::Sonde));
    }
    assert!(r.events.iter().any(|e| e.get(&keys::POSITION).is_some()));
    assert!(!r.results.points().is_empty());
    points_show_rows(&r.results, SONDE_SONDES.key);
}

/// The GPS time of an RS41 frame is turned into UTC with the station's leap second table, so one more leap second results in a
/// UTC one second earlier.
#[test]
fn rs41_utc_follows_the_station_leap_seconds() {
    let x = scene(250e3, vec![spec("RS41", 0.0, "T4710042", 5.0)], 3.0, None);
    let decode = |leap: &LeapSeconds| {
        let st = Station {
            leap_seconds: Some(*leap),
            ..Default::default()
        };
        let r = Harness::new(&sci::SONDE)
            .rate(250e3)
            .freq(403e6)
            .piece(4096)
            .station(st)
            .feed(&x)
            .unwrap();
        sondes(&r)[0].at(&SondeRow::TIME).unwrap()
    };
    let builtin = decode(&LeapSeconds::BUILTIN);
    // A leap second added before the scene's date.
    let more = LeapSeconds::BUILTIN.with_leap(1_767_225_600, 38).unwrap();
    let later = decode(&more);
    assert!((builtin - later - 1.0).abs() < 1e-6, "{builtin} {later}");
}

/// A long drift to the north lands where the WGS-84 geodesic
/// of the same length lands.
#[test]
fn landing_drift_follows_the_wgs84_geodesic() {
    use sdrkit_util_geo::ellipsoid::Ellipsoid;
    use sdrkit_util_geo::Position;
    let l = sci::sonde::landing::descend(1.0, 10.0, 30_000.0, -5.0, 0.0, 30.0, 0.0, None).unwrap();
    let (p, _) = Ellipsoid::WGS84
        .direct(&Position::new(1.0, 10.0), 0.0, 30.0 * l.seconds)
        .unwrap();
    let miss = Position::new(l.lat, l.lon).distance_m(&p);
    assert!(30.0 * l.seconds > 100e3);
    assert!(miss < 1.0, "{miss} m from the geodesic");
}

/// The DFM Hamming (8,4) code (`sdrkit_algo_fec::small::SmallCode`)
/// corrects every single error and reports every double error as uncorrectable.
#[test]
fn dfm_hamming_detects_every_double_error() {
    use sci::sonde::dfm::{hamming_decode, hamming_encode};
    for n in 0..16u8 {
        let cw = hamming_encode(n);
        assert_eq!(hamming_decode(cw), Some((n, false)));
        for p in 0..8 {
            let mut e = cw;
            e[p] = !e[p];
            assert_eq!(hamming_decode(e), Some((n, true)));
            for q in p + 1..8 {
                let mut e2 = e;
                e2[q] = !e2[q];
                assert_eq!(hamming_decode(e2), None, "nibble {n} errors at {p} and {q}");
            }
        }
    }
}

/// An RS41 frame with a byte error in each code word is corrected by the shared RS(255,231) code.
#[test]
fn rs41_frame_corrects_byte_errors() {
    let mut d = sci::sonde::rs41::Rs41Decoder::default();
    d.set_leap_seconds(&LeapSeconds::BUILTIN);
    let g = SondeGenerator::new(
        250e3,
        SondeGenOptions {
            sondes: Some(vec![spec("RS41", 0.0, "T4710042", 5.0)]),
            ..Default::default()
        },
    );
    let _ = g.rate_hz();
    let tx = sci::sonde::rs41::Rs41Tx {
        serial: "T4710042".into(),
        frame: 1000,
        battery_v: 2.7,
        cal: sci::sonde::generator::rs41_calibration(403e6, false),
        week: 2400,
        tow_ms: 100_000,
        pos: [3_000_000.0, 4_000_000.0, 3_500_000.0],
        vel: [1.0, 2.0, 3.0],
        sats: 9,
        ptu: [0; 12],
        extended: false,
    };
    let mut f = sci::sonde::rs41::build_frame(&tx);
    f[0x40] ^= 0x5A;
    f[0x41] ^= 0x33;
    let fr = d.decode_plain(&mut f, 0.0);
    assert!(fr.ok);
    assert_eq!(fr.corrections, 2);
    assert_eq!(fr.serial.as_deref(), Some("T4710042"));
}

/// The input rate of the sensitivity trials.
const TRIAL_RATE: f64 = 400e3;
/// The length of one sensitivity trial, s.
const TRIAL_S: f64 = 8.0;
/// Sensitivity trials for each point.
const TRIALS: u64 = 20;

/// The good frames of `kind` over every row of that type.
fn good_frames(v: &[Record], kind: SondeType) -> f64 {
    v.iter()
        .filter(|s| s.at(&SondeRow::TYPE) == Some(kind))
        .filter_map(|s| s.get(&keys::FRAMES_OK))
        .sum::<i64>() as f64
}

/// For each trial at `snr_db` in 10 kHz, the good frames of each of `kinds` as shares of a clean run's frames.
fn shares(scene_in: &[SondeSpec], kinds: &[SondeType], snr_db: Option<f64>, trials: u64) -> Vec<Vec<f64>> {
    let feed = |snr: Option<f64>, seed: u64| {
        let x = scene_seeded(TRIAL_RATE, scene_in.to_vec(), TRIAL_S, snr, seed, 0.0);
        let r = Harness::new(&sci::SONDE)
            .rate(TRIAL_RATE)
            .freq(403e6)
            .piece(4096)
            .feed(&x)
            .unwrap();
        let v = sondes(&r);
        kinds.iter().map(|&k| good_frames(&v, k)).collect::<Vec<_>>()
    };
    // The clean run: 40 dB in 10 kHz.
    let clean = feed(Some(40.0), 0);
    assert!(clean.iter().all(|&c| c >= 4.0), "clean run: {clean:?}");
    (0..trials)
        .map(|s| {
            feed(snr_db, 100 + s)
                .iter()
                .zip(&clean)
                .map(|(n, c)| n / c)
                .collect()
        })
        .collect()
}

/// The mean share of the clean frames of the first type over [`TRIALS`] trials.
fn mean_share(scene_in: &[SondeSpec], kind: SondeType, snr_db: f64) -> f64 {
    let s = shares(scene_in, &[kind], Some(snr_db), TRIALS);
    s.iter().map(|t| t[0]).sum::<f64>() / s.len() as f64
}

/// Each case: (what, measured share, required share).
fn check_shares(cases: &[(&str, f64, f64)]) {
    let failed: Vec<_> = cases.iter().filter(|c| c.1 < c.2).collect();
    assert!(failed.is_empty(), "under the required share: {failed:?}; all: {cases:?}");
}

/// RS41 and RS92 read the narrow path with tone-energy symbols: the share of the clean frames at the points of the
/// specification (SNR in 10 kHz, see [`scene`]). RS41 at 6 dB (Eb/N0 9.2 dB) has a bit error rate of 0.4 %, half the
/// `exp(-Eb / 2 N0) / 2` = 0.8 % of a tone-energy decision over one symbol of white noise. That limit does not hold here: the two
/// tones of a modulation index of 1 keep their phase from symbol to symbol, and the 4.5 kHz filter ahead of the decision spreads
/// each window over the neighbour symbols, so equal neighbours add to the tone in phase.
#[test]
fn rs41_and_rs92_reach_their_sensitivity() {
    let rs41 = [spec("RS41", 60e3, "T4710042", -5.0)];
    let rs41_far = [spec("RS41", 158e3, "T4710042", -5.0)];
    let rs92 = [spec("RS92", 60e3, "P4953934", -5.0)];
    check_shares(&[
        ("RS41 7 dB", mean_share(&rs41, SondeType::Rs41, 7.0), 0.90),
        ("RS41 6 dB", mean_share(&rs41, SondeType::Rs41, 6.0), 0.73),
        ("RS41 +158 kHz 6.5 dB", mean_share(&rs41_far, SondeType::Rs41, 6.5), 0.90),
        ("RS92 8 dB", mean_share(&rs92, SondeType::Rs92, 8.0), 0.95),
        ("RS92 6 dB", mean_share(&rs92, SondeType::Rs92, 6.0), 0.68),
    ]);
}

/// DFM reads the narrow path with tone-energy symbols, centred and with an 8 kHz carrier error.
#[test]
fn dfm_reaches_its_sensitivity() {
    let dfm = [spec("DFM-17", 60e3, "17012345", -5.0)];
    let dfm_off = [spec("DFM-17", 68e3, "17012345", -5.0)];
    check_shares(&[
        ("DFM 8 dB", mean_share(&dfm, SondeType::Dfm, 8.0), 0.60),
        ("DFM 6 dB", mean_share(&dfm, SondeType::Dfm, 6.0), 0.40),
        ("DFM +8 kHz 8 dB", mean_share(&dfm_off, SondeType::Dfm, 8.0), 0.70),
        ("DFM +8 kHz 6 dB", mean_share(&dfm_off, SondeType::Dfm, 6.0), 0.30),
    ]);
}

/// A mix of RS41, DFM-17 and M10: at 9 dB, 90 % of the trials give at least half the clean run's frames of the three together;
/// at 20 dB each sonde, DFM and M10 among them, still gives at least half its clean frames; with 20 ppm of carrier error and
/// clocks 50 ppm slow, every one of 10 trials at 10 dB gives half the clean frames.
#[test]
fn a_mix_reaches_its_sensitivity() {
    let mix = [
        spec("RS41", -120e3, "T4710042", -5.0),
        spec("DFM-17", 20e3, "17012345", -5.0),
        spec("M10", 140e3, "310-2-11329", -5.0),
    ];
    let kinds = [SondeType::Rs41, SondeType::Dfm, SondeType::M10];
    let clean = shares(&mix, &kinds, Some(40.0), 1)[0].clone();
    assert!(clean.iter().all(|&s| s >= 0.99), "{clean:?}");
    let at9 = shares(&mix, &kinds, Some(9.0), TRIALS);
    // The clean frames of each type, to weight the shares.
    let weights: Vec<f64> = {
        let x = scene_seeded(TRIAL_RATE, mix.to_vec(), TRIAL_S, Some(40.0), 0, 0.0);
        let r = Harness::new(&sci::SONDE)
            .rate(TRIAL_RATE)
            .freq(403e6)
            .piece(4096)
            .feed(&x)
            .unwrap();
        let v = sondes(&r);
        kinds.iter().map(|&k| good_frames(&v, k)).collect()
    };
    let total: f64 = weights.iter().sum();
    let pass = at9
        .iter()
        .filter(|t| t.iter().zip(&weights).map(|(s, w)| s * w).sum::<f64>() >= 0.5 * total)
        .count();
    let at20 = shares(&mix, &kinds, Some(20.0), 4);
    let low20 = at20.iter().flatten().fold(1.0f64, |a, &b| a.min(b));
    // 20 ppm of carrier error (8 kHz at 403 MHz) and clocks 50 ppm slow: 10 of 10 trials at 10 dB.
    let moved: Vec<SondeSpec> = mix
        .iter()
        .map(|s| SondeSpec {
            offset_hz: s.offset_hz + 8e3,
            ..s.clone()
        })
        .collect();
    let impaired = (0..10)
        .filter(|&seed| {
            let x = scene_seeded(TRIAL_RATE, moved.clone(), TRIAL_S, Some(10.0), 200 + seed, 50.0);
            let r = Harness::new(&sci::SONDE)
                .rate(TRIAL_RATE)
                .freq(403e6)
                .piece(4096)
                .feed(&x)
                .unwrap();
            let v = sondes(&r);
            kinds.iter().map(|&k| good_frames(&v, k)).sum::<f64>() >= 0.5 * total
        })
        .count();
    assert!(
        pass * 10 >= at9.len() * 9 && low20 >= 0.5 && impaired == 10,
        "9 dB: {pass} of {} trials; 20 dB lowest share {low20}; impaired at 10 dB: {impaired} of 10; {at9:?}",
        at9.len()
    );
}

/// The carrier search keeps an M10, whose carrier line holds about 70 % of its power, and drops an unmodulated carrier.
#[test]
fn the_search_tells_an_m10_from_an_unmodulated_carrier() {
    let m10 = [spec("M10", 140e3, "310-2-11329", -5.0)];
    for snr in [40.0, 20.0, 12.0] {
        let s = shares(&m10, &[SondeType::M10], Some(snr), 2);
        assert!(s.iter().all(|t| t[0] >= 0.8), "M10 at {snr} dB: {s:?}");
    }
    for (f, snr) in [(100e3, 30.0), (100.39e3, 30.0), (100e3, 10.0), (100.39e3, 10.0)] {
        let mut x: Vec<Complex32> = (0..(TRIAL_RATE * 2.0) as usize)
            .map(|n| Complex32::from_polar(1.0, (std::f64::consts::TAU * f * n as f64 / TRIAL_RATE) as f32))
            .collect();
        let n0 = 10f64.powf(-snr / 10.0) / 10e3;
        let mut awgn = sdrkit_algo_sim::noise::Awgn::with_power(n0 * TRIAL_RATE, 5).unwrap();
        awgn.process(&mut x);
        let r = Harness::new(&sci::SONDE)
            .rate(TRIAL_RATE)
            .freq(403e6)
            .piece(4096)
            .feed(&x)
            .unwrap();
        let id = r.results.decl().table_id(SONDE_CARRIERS.key).unwrap();
        let near = r
            .results
            .table(id)
            .unwrap()
            .rows_newest_first()
            .filter(|(_, row)| row.at(&SondeCarrierRow::OFFSET).is_some_and(|o| (o - f).abs() < 6e3))
            .count();
        assert_eq!(near, 0, "carrier at {f} Hz, {snr} dB");
    }
}

/// Noise alone gives no frame.
#[test]
fn noise_alone_gives_no_sonde_frame() {
    for seed in 0..4 {
        let mut x = vec![Complex32::new(0.0, 0.0); (TRIAL_RATE * 4.0) as usize];
        let mut awgn = sdrkit_algo_sim::noise::Awgn::with_power(1.0, 300 + seed).unwrap();
        awgn.process(&mut x);
        let r = Harness::new(&sci::SONDE)
            .rate(TRIAL_RATE)
            .freq(403e6)
            .piece(4096)
            .feed(&x)
            .unwrap();
        assert!(sondes(&r).is_empty(), "seed {seed}");
    }
}
