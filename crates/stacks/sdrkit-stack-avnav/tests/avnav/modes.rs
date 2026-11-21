//! Each stack's test signal, with noise, offset and clock error from the framework, through its receiver.

use crate::common::{get, noisy, rows, run};
use sdrkit_algo_sim::chain::{GEN_CLOCK_PPM, GEN_OFFSET_HZ};
use sdrkit_core::{Choice, LockState};
use sdrkit_dev_test::Harness;
use sdrkit_stack_avnav::dgps::GenBitRate;
use sdrkit_stack_avnav::dme::Heard;
use sdrkit_stack_avnav::dme::Transmitter;
use sdrkit_stack_avnav::ils::{IlsCommand, IlsPart};
use sdrkit_stack_avnav::ndb::{GenKeying, GEN_KEYING};
use sdrkit_stack_avnav::pairing::DmeMode;
use sdrkit_stack_avnav::typed::{
    CarrierFields, DgpsCorrectionRow, DgpsFields, DmeFields, IdentFields, IlsFields, KeyerFields,
    LoranFields, LoranStationRow, NdbScanRow, VorFields, DGPS_ALMANAC, DGPS_CORRECTIONS,
    LORAN_STATIONS, NDB_SCAN,
};
use sdrkit_stack_avnav::vor::ToFrom;
use sdrkit_stack_avnav::vor::VorKind;
use sdrkit_stack_avnav::{dgps, dme, ils, loran, ndb, vor, DGPS, DME, ILS, LORAN, NDB, VOR};
use sdrkit_util_geo::position::{wrap180, wrap360};

#[test]
fn ndb_keyed_tone_with_noise_offset_and_clock_error() {
    let r = run(&noisy(&NDB, 50.0)
        .rate(250e3)
        .seconds(18.0)
        .piece(4096)
        .set(&ndb::GEN_IDENT, "ML")
        .set(&GEN_OFFSET_HZ, 47.0)
        .set(&GEN_CLOCK_PPM, 20.0));
    let f = &r.results;
    assert_eq!(get(f, &IdentFields::IDENT), Some("ML"), "{:?}", f.fields());
    let offset = get(f, &CarrierFields::OFFSET_HZ).unwrap_or(f64::NAN);
    assert!((offset - 47.0).abs() < 2.0, "{:?}", f.fields());
    assert!(!r.audio.is_empty());
    assert!(r.events_of("morse.word").next().is_some());
    assert_eq!(f.status().lock_state, LockState::Locked);
}

#[test]
fn ndb_keyed_carrier_and_400_hz_tone() {
    let r = run(&noisy(&NDB, 50.0)
        .rate(96e3)
        .seconds(18.0)
        .seed(3)
        .set(&GEN_KEYING, GenKeying::A1a)
        .set(&ndb::GEN_IDENT, "WG")
        .set(&GEN_OFFSET_HZ, -120.0));
    assert_eq!(get(&r.results, &IdentFields::IDENT), Some("WG"));
    let r = run(&noisy(&NDB, 50.0)
        .rate(96e3)
        .seconds(18.0)
        .seed(5)
        .set(&ndb::GEN_TONE, 400.0)
        .set(&ndb::GEN_IDENT, "TW"));
    assert_eq!(get(&r.results, &IdentFields::IDENT), Some("TW"));
    assert_eq!(get(&r.results, &KeyerFields::ID_TONE_HZ), Some(400.0));
}

#[test]
fn ndb_band_scan_lists_every_beacon() {
    let r = run(&noisy(&NDB, 65.0)
        .rate(250e3)
        .seconds(5.0)
        .seed(7)
        .set(&ndb::GEN_OTHERS, 4));
    let khz: Vec<f64> = rows(&r.results, &NDB_SCAN)
        .iter()
        .filter_map(|row| row.at(&NdbScanRow::FREQ_HZ))
        .map(|hz| hz / 1e3)
        .collect();
    for want in [344.0, 348.0, 340.0, 351.1, 336.9] {
        assert!(
            khz.iter().any(|k| (k - want).abs() < 0.02),
            "{want} not in {khz:?}"
        );
    }
    assert_eq!(khz.len(), 5, "{khz:?}");
}

#[test]
fn vor_radials_conventional_and_doppler() {
    for (kind, radial, seed) in [
        (VorKind::Conventional, 10.0, 1),
        (VorKind::Conventional, 271.0, 2),
        (VorKind::Doppler, 135.0, 3),
        (VorKind::Doppler, 359.5, 4),
    ] {
        let r = run(&noisy(&VOR, 64.0)
            .rate(250e3)
            .freq(116.0e6)
            .seconds(9.0)
            .piece(8192)
            .seed(seed)
            .set(&vor::GEN_KIND, kind)
            .set(&vor::GEN_RADIAL, radial)
            .set(&vor::GEN_IDENT, "CB")
            .set(&GEN_OFFSET_HZ, 350.0)
            .set(&GEN_CLOCK_PPM, 30.0)
            .set(&vor::OBS, wrap360(radial + 4.0)));
        let f = &r.results;
        let got = get(f, &VorFields::RADIAL_DEG).unwrap_or(f64::NAN);
        assert!(
            wrap180(got - radial).abs() < 1.0,
            "{} {radial}: {:?}",
            kind.label(),
            f.fields()
        );
        assert_eq!(get(f, &VorFields::TO_FROM), Some(ToFrom::From));
        let cdi = get(f, &VorFields::CDI_DEG).unwrap_or(f64::NAN);
        assert!((cdi - 4.0).abs() < 1.0, "{:?}", f.fields());
        assert_eq!(get(f, &IdentFields::IDENT), Some("CB"), "{:?}", f.fields());
        let dev = get(f, &VorFields::DEVIATION_HZ).unwrap_or(f64::NAN);
        assert!((dev - 480.0).abs() < 25.0);
        let am30 = get(f, &VorFields::AM30_PCT).unwrap_or(f64::NAN);
        assert!((am30 - 30.0).abs() < 3.0);
        assert_eq!(get(f, &VorFields::DME_CHANNEL), Some("107X"));
    }
}

#[test]
fn vor_follows_a_moving_radial() {
    let r = run(&noisy(&VOR, 69.0)
        .rate(128e3)
        .seconds(6.0)
        .piece(5000)
        .seed(9)
        .set(&vor::GEN_RADIAL, 100.0)
        .set(&vor::GEN_ROTATE, 2.0)
        .set(&vor::AVERAGE, 0.3));
    let got = get(&r.results, &VorFields::RADIAL_DEG).unwrap_or(f64::NAN);
    // 112 at the end. The 0.3 s average lags by about 0.6 degrees.
    assert!((got - 112.0).abs() < 1.5, "{got}");
}

#[test]
fn vor_flag_without_a_station() {
    let h = Harness::new(&VOR).rate(128e3).seconds(3.0);
    let mut rng = sdrkit_algo_sim::rng::Rng::new(5);
    let x: Vec<_> = (0..(128e3 * 3.0) as usize)
        .map(|_| rng.complex_normal(1e-4))
        .collect();
    let r = h.feed(&x).unwrap();
    assert_eq!(
        get(&r.results, &VorFields::TO_FROM),
        Some(ToFrom::Off),
        "{:?}",
        r.results.fields()
    );
    assert_eq!(get(&r.results, &VorFields::RADIAL_DEG), None);
}

#[test]
fn ils_localizer_and_glide_path() {
    for (sub, ddm, cmd, freq) in [
        ("localizer", 0.08, IlsCommand::FlyRight, 109.5e6),
        ("localizer", -0.04, IlsCommand::FlyLeft, 109.5e6),
        ("glideslope", 0.06, IlsCommand::FlyDown, 332.6e6),
        ("glideslope", -0.1, IlsCommand::FlyUp, 332.6e6),
    ] {
        let r = run(&noisy(&ILS, 60.0)
            .rate(250e3)
            .submode(sub)
            .freq(freq)
            .seconds(13.0)
            .piece(6000)
            .seed(3)
            .set(&ils::GEN_DDM, ddm)
            .set(&ils::GEN_IDENT, "IBK")
            .set(&GEN_OFFSET_HZ, -400.0)
            .set(&GEN_CLOCK_PPM, 10.0));
        let f = &r.results;
        let got = get(f, &IlsFields::DDM).unwrap_or(f64::NAN);
        assert!((got - ddm).abs() < 0.004, "{sub} {ddm}: {:?}", f.fields());
        assert_eq!(get(f, &IlsFields::COMMAND), Some(cmd));
        assert_eq!(get(f, &IlsFields::FLAG), Some(false));
        assert_eq!(get(f, &IlsFields::DME_CHANNEL), Some("32X"));
        let sdm = get(f, &IlsFields::SDM_PCT).unwrap_or(f64::NAN);
        if sub == "localizer" {
            assert_eq!(get(f, &IdentFields::IDENT), Some("IBK"), "{:?}", f.fields());
            assert_eq!(get(f, &IlsFields::PAIRED_HZ), Some(332.6e6));
            assert!((sdm - 40.0).abs() < 2.0);
        } else {
            assert_eq!(get(f, &IlsFields::PAIRED_HZ), Some(109.5e6));
            assert!((sdm - 80.0).abs() < 3.0);
        }
    }
}

#[test]
fn ils_part_follows_the_frequency() {
    let x = Harness::new(&ILS)
        .rate(64e3)
        .submode("glideslope")
        .seconds(2.0)
        .generate()
        .unwrap();
    let r = Harness::new(&ILS)
        .rate(64e3)
        .freq(335.0e6)
        .feed(&x)
        .unwrap();
    assert_eq!(get(&r.results, &IlsFields::PART), Some(IlsPart::Glideslope));
}

#[test]
fn dme_beacons_x_and_y_and_interrogations() {
    for (mode, kind, freq, chan, heard) in [
        (DmeMode::X, Transmitter::Beacon, 978e6, "17X", Heard::Beacon),
        (DmeMode::Y, Transmitter::Beacon, 1088e6, "1Y", Heard::Beacon),
        (
            DmeMode::X,
            Transmitter::Beacon,
            1185e6,
            "98X",
            Heard::Beacon,
        ),
        (
            DmeMode::Y,
            Transmitter::Aircraft,
            1041e6,
            "17Y",
            Heard::Interrogations,
        ),
    ] {
        let r = run(&noisy(&DME, 80.0)
            .rate(2e6)
            .freq(freq)
            .seconds(6.0)
            .piece(65_536)
            .seed(11)
            .set(&dme::GEN_MODE, mode)
            .set(&dme::GEN_KIND, kind)
            .set(&dme::GEN_IDENT, "TS")
            .set(&GEN_OFFSET_HZ, 30e3));
        let f = &r.results;
        assert_eq!(get(f, &DmeFields::CHANNEL), Some(chan), "{:?}", f.fields());
        assert_eq!(get(f, &DmeFields::HEARD), Some(heard));
        let pps = get(f, &DmeFields::PAIRS_PER_S).unwrap_or(f64::NAN);
        assert!(pps > 1500.0 && pps < 3000.0, "{:?}", f.fields());
        if kind == Transmitter::Beacon {
            assert_eq!(get(f, &IdentFields::IDENT), Some("TS"), "{:?}", f.fields());
        }
        let width = get(f, &DmeFields::WIDTH_US).unwrap_or(f64::NAN);
        assert!((width - 3.5).abs() < 0.5, "{:?}", f.fields());
    }
}

#[test]
fn dme_vhf_pairing() {
    let r = run(&Harness::new(&DME).rate(2e6).freq(978e6).seconds(1.5));
    assert_eq!(get(&r.results, &DmeFields::PAIRED_VHF_HZ), Some(108.0e6));
}

/// A pulse pair 12 µs apart whose first pulse rises from sample 2 is read: the pulse detector's start index does not go
/// below 0.
#[test]
fn dme_pulse_in_the_first_samples_is_timed() {
    let rate = 2e6;
    let mut x = vec![sdrkit_core::Complex32::new(1e-3, 0.0); (0.3 * rate) as usize];
    let sigma = 3.5e-6 / (2.0 * (2.0 * std::f64::consts::LN_2).sqrt());
    for (i, s) in x.iter_mut().enumerate().take(200) {
        let t = i as f64 / rate;
        for centre in [6e-6, 18e-6] {
            let d = t - centre;
            s.re += (-(d * d) / (2.0 * sigma * sigma)).exp() as f32;
        }
    }
    let r = Harness::new(&DME).rate(rate).freq(978e6).feed(&x).unwrap();
    assert_eq!(
        get(&r.results, &DmeFields::PAIRS),
        Some(1),
        "{:?}",
        r.results.fields()
    );
}

fn stations(r: &sdrkit_core::Results) -> Vec<(String, f64, f64)> {
    let mut v: Vec<(String, f64, f64)> = rows(r, &LORAN_STATIONS)
        .into_iter()
        .map(|row| {
            (
                row.at(&LoranStationRow::STATION).unwrap_or("").to_string(),
                row.at(&LoranStationRow::TD_US).unwrap_or(f64::NAN),
                row.at(&LoranStationRow::ECD_US).unwrap_or(f64::NAN),
            )
        })
        .collect();
    v.sort_by(|a, b| a.1.total_cmp(&b.1));
    v
}

#[test]
fn loran_chain_with_noise() {
    let r = run(&noisy(&LORAN, 43.0)
        .rate(200e3)
        .freq(100e3)
        .seconds(6.0)
        .piece(50_000)
        .seed(21)
        .set(&loran::GEN_ECD, -2.0)
        .set(&loran::GRI_SETTING, 9960)
        .set(&loran::CHAINS, crate::loran::CHAIN_FILE));
    let st = stations(&r.results);
    assert_eq!(st.len(), 4, "{st:?}");
    for (s, want) in st[1..].iter().zip([13797.2, 26969.93, 42221.65]) {
        assert!((s.1 - want).abs() < 0.2, "{st:?}");
    }
    assert!((st[1].2 + 2.0).abs() < 1.0, "{st:?}");
    assert_eq!(st[1].0, "W");
    assert_eq!(get(&r.results, &LoranFields::CHAIN), Some("Northeast U.S."));
}

#[test]
fn loran_finds_the_gri() {
    let r = run(&noisy(&LORAN, 53.0)
        .rate(250e3)
        .freq(100e3)
        .seconds(5.0)
        .piece(65_536)
        .seed(23)
        .set(&loran::GEN_GRI, 6731)
        .set(&loran::GEN_TDS, "12000.5,27500.25")
        .set(&loran::CHAINS, crate::loran::CHAIN_FILE));
    assert_eq!(
        get(&r.results, &LoranFields::GRI),
        Some(6731),
        "{:?}",
        r.results.fields()
    );
    assert_eq!(get(&r.results, &LoranFields::CHAIN), Some("Lessay"));
    let st = stations(&r.results);
    assert!((st[1].1 - 12000.5).abs() < 0.2, "{st:?}");
    assert!((st[2].1 - 27500.25).abs() < 0.2, "{st:?}");
}

#[test]
fn dgps_every_bit_rate_with_noise_offset_and_clock_error() {
    for (br, hz, secs) in [
        (GenBitRate::B200, 200.0, 12.0),
        (GenBitRate::B100, 100.0, 18.0),
        (GenBitRate::B50, 50.0, 30.0),
    ] {
        let r = run(&noisy(&DGPS, 42.0)
            .rate(96e3)
            .freq(304e3)
            .seconds(secs)
            .piece(7777)
            .seed(31)
            .set(&dgps::GEN_BITRATE, br)
            .set(&dgps::GEN_STATION, 812)
            .set(&GEN_OFFSET_HZ, 37.0)
            .set(&GEN_CLOCK_PPM, 50.0));
        let f = &r.results;
        assert_eq!(
            get(f, &DgpsFields::SYMBOL_RATE_HZ),
            Some(hz),
            "{:?}",
            f.fields()
        );
        assert_eq!(
            get(f, &DgpsFields::STATION_ID),
            Some(812),
            "{:?}",
            f.fields()
        );
        let corrections = rows(f, &DGPS_CORRECTIONS);
        assert_eq!(corrections.len(), 6);
        assert_eq!(get(f, &DgpsFields::TEXT), Some("SDRKIT TEST BEACON"));
        let (lat, _) = get(f, &DgpsFields::BASE_STATION).unwrap();
        assert!((lat + 33.86).abs() < 1e-4);
        assert_eq!(rows(f, &DGPS_ALMANAC).len(), 1);
        assert!(r.events_of("rtcm.frame").next().is_some());
        let c5 = corrections
            .iter()
            .find(|c| c.at(&DgpsCorrectionRow::PRN) == Some(5))
            .unwrap();
        assert!((c5.at(&DgpsCorrectionRow::PRC_M).unwrap() - 12.34).abs() < 0.011);
    }
}
