//! Radio Jove and the solar burst spectrogram through the receiver: broadband bursts under gain drift and keyed narrowband
//! interference, a drifting burst with its drift rate, noise alone, pieces of any size, the commands, and the e-CALLISTO FITS
//! file written through the host's storage and read back against the published header.

use sci::bursts::gen::{BurstGenerator, GenBurst, GenParams, Interferer};
use sci::bursts::{self, CALLISTO, JOVE};
use sci::typed::{BurstFields, ScienceEvent, SCIENCE_EVENT, SCIENCE_EVENTS};
use sdrkit_codec_fits::{self as fits, Card, Data};
use sdrkit_core::{Complex32, Facilities, Generator, InputInfo, MemStorage, Output, Receiver, Storage, StorePath};
use sdrkit_dev_test::{Harness, Run};
use sdrkit_stack_science as sci;
use std::sync::Arc;

#[derive(Clone, Debug, PartialEq)]
struct Ev {
    t: f64,
    kind: String,
    f: f64,
    peak: f64,
    dur: f64,
    drift: f64,
    width: f64,
}

fn events(r: &Run) -> Vec<Ev> {
    r.events
        .iter()
        .filter(|e| e.is(&SCIENCE_EVENT))
        .map(|e| Ev {
            t: e.time,
            kind: e.at(&ScienceEvent::TYPE).unwrap_or("").to_string(),
            f: e.at(&ScienceEvent::FREQUENCY_HZ).unwrap_or(0.0),
            peak: e.at(&ScienceEvent::PEAK_DB).unwrap_or(0.0),
            dur: e.at(&ScienceEvent::DURATION_S).unwrap_or(0.0),
            drift: e.at(&ScienceEvent::AUX1).unwrap_or(0.0),
            width: e.at(&ScienceEvent::AUX2).unwrap_or(0.0),
        })
        .collect()
}

fn signal(rate: f64, seconds: f64, p: GenParams) -> Vec<Complex32> {
    let mut g = BurstGenerator::new(rate, p, 11);
    let mut x = vec![Complex32::default(); (rate * seconds) as usize];
    g.generate(&mut x);
    x
}

/// The `jove` case: 45 s at 250 ksps, a 1 dB gain drift over the run and a carrier at +60 kHz (amplitude 0.4, noise power 1)
/// on for 1 s every 7 s; bursts of 1.5 s and 200 kHz at 12 and 28 s at `level_db`.
fn jove_case(level_db: f64, with_bursts: bool) -> GenParams {
    let b = |t: f64| GenBurst {
        start_s: t,
        len_s: 1.5,
        width_hz: 200e3,
        centre_hz: 0.0,
        drift_hz_s: 0.0,
    };
    GenParams {
        level_db,
        bursts: if with_bursts { vec![b(12.0), b(28.0)] } else { Vec::new() },
        noise_power: 1.0,
        gain_drift_db: 1.0,
        gain_over_s: 45.0,
        interferer: Some(Interferer {
            offset_hz: 60e3,
            amplitude: 0.4,
            on_s: 1.0,
            every_s: 7.0,
        }),
        ..GenParams::quiet()
    }
}

#[test]
fn jove_finds_two_bursts_of_6_db_under_drift_and_interference() {
    let x = signal(250e3, 45.0, jove_case(6.0, true));
    let r = Harness::new(&JOVE).rate(250e3).feed(&x).unwrap();
    let ev = events(&r);
    assert_eq!(ev.len(), 2, "{ev:?}");
    for (e, t0) in ev.iter().zip([12.0, 28.0]) {
        assert!((e.t - t0).abs() <= 0.6, "{e:?}");
        assert!((e.dur - 1.5).abs() <= 0.5, "{e:?}");
        assert!(e.peak > 3.0, "{e:?}");
        assert_eq!(e.kind, "burst");
    }
    assert_eq!(r.results.fields().at(&BurstFields::BURSTS), Some(2));
}

#[test]
fn jove_finds_two_bursts_of_3_db() {
    let x = signal(250e3, 45.0, jove_case(3.0, true));
    let ev = events(&Harness::new(&JOVE).rate(250e3).feed(&x).unwrap());
    assert_eq!(ev.len(), 2, "{ev:?}");
}

#[test]
fn jove_finds_nothing_in_noise_with_drift_and_interference() {
    let x = signal(250e3, 45.0, jove_case(6.0, false));
    let ev = events(&Harness::new(&JOVE).rate(250e3).feed(&x).unwrap());
    assert!(ev.is_empty(), "{ev:?}");
}

#[test]
fn callisto_measures_the_drift_of_one_burst() {
    let p = GenParams {
        level_db: 8.0,
        bursts: vec![GenBurst {
            start_s: 15.0,
            len_s: 2.5,
            width_hz: 60e3,
            centre_hz: -50e3,
            drift_hz_s: 40e3,
        }],
        noise_power: 1.0,
        ..GenParams::quiet()
    };
    let x = signal(250e3, 30.0, p);
    let r = Harness::new(&CALLISTO).rate(250e3).feed(&x).unwrap();
    let ev = events(&r);
    assert_eq!(ev.len(), 1, "{ev:?}");
    let e = &ev[0];
    assert_eq!(e.kind, "drifting burst", "{e:?}");
    assert!((15e3..=70e3).contains(&e.drift), "{e:?}");
    assert!((e.t - 15.0).abs() <= 0.8, "{e:?}");
    assert!(e.width > 0.0 && e.f < 0.0, "{e:?}");
}

#[test]
fn the_test_signals_give_bursts_at_2_msps() {
    // jove: bursts at 0.2 and 0.55 of every 4 s cycle, none in the first 3 s: at 4.8 and 6.2 s. callisto: at 0.2 and 0.6 of
    // every 8 s cycle, none in the first 7.5 s: at 9.6 and 12.8 s, 2 s long, drifting 200 kHz/s.
    for (d, kind, s, n) in [(&JOVE, "burst", 8.0, 2), (&CALLISTO, "drifting burst", 16.0, 2)] {
        let r = Harness::new(d).rate(2e6).seconds(s).run().unwrap();
        let ev = events(&r);
        assert_eq!(ev.len(), n, "{}: {ev:?}", d.id);
        assert!(ev.iter().all(|e| e.kind == kind), "{}: {ev:?}", d.id);
        if d.id == "callisto" {
            assert!(ev.iter().all(|e| (150e3..=250e3).contains(&e.drift)), "{ev:?}");
        }
    }
}

#[test]
fn pieces_of_any_size_give_the_same_events() {
    let x = signal(250e3, 45.0, jove_case(6.0, true));
    let runs: Vec<Vec<Ev>> = [977usize, 8192, 100_000]
        .iter()
        .map(|&n| events(&Harness::new(&JOVE).rate(250e3).piece(n).feed(&x).unwrap()))
        .collect();
    assert_eq!(runs[0].len(), 2);
    assert_eq!(runs[0], runs[1]);
    assert_eq!(runs[0], runs[2]);
}

fn command(rx: &mut dyn Receiver, key: &str) {
    let id = rx.commands().id(key).unwrap();
    let args = rx.commands().def(id).unwrap().new_args();
    sdrkit_core::owner::invoke(rx, id, &args).unwrap();
}

#[test]
fn clear_empties_the_tables() {
    let x = signal(250e3, 45.0, jove_case(6.0, true));
    let mut rx = Harness::new(&JOVE).rate(250e3).receiver().unwrap();
    rx.configure(&InputInfo::new(250e3, 20.1e6, 20.1e6).recording())
        .unwrap();
    let mut out = Output::default();
    for c in x.chunks(16_384) {
        out.clear();
        rx.feed(c, &mut out);
    }
    assert_eq!(rx.results().fields().at(&BurstFields::EVENTS_TOTAL), Some(2));
    command(rx.as_mut(), "clear");
    assert_eq!(rx.results().fields().at(&BurstFields::EVENTS_TOTAL), Some(0));
    let r = rx.results();
    assert_eq!(r.table(r.decl().table_id(SCIENCE_EVENTS.key).unwrap()).map_or(0, |t| t.len()), 0);
}

/// 2026-03-20 12:00:00 UTC.
const START: f64 = 1_773_964_800.0 + 12.0 * 3600.0;

#[test]
fn callisto_writes_e_callisto_fits_files_through_the_host_storage() {
    let storage = Arc::new(MemStorage::new());
    let station = sdrkit_core::Station {
        position: Some(sdrkit_util_geo::Position::new(-33.87, 151.21)),
        height_m: Some(40.0),
        ..sdrkit_core::Station::default()
    };
    let h = Harness::new(&CALLISTO)
        .rate(2e6)
        .station(station)
        .set(&bursts::FITS, true)
        .set(&bursts::FILE_S, 10.0)
        .set(&bursts::STATION_CODE, "SYDNEY1")
        .set(&sci::mscatter::START_UTC, START);
    let mut rx = h.receiver().unwrap();
    rx.set_facilities(&Facilities {
        storage: storage.clone(),
        ..Facilities::default()
    });
    rx.configure(&InputInfo::new(2e6, 200e6, 200e6).recording())
        .unwrap();
    let mut g = BurstGenerator::new(2e6, GenParams::callisto(2e6, 6.0, 0.0), 1);
    let mut x = vec![Complex32::default(); 200_000];
    let mut out = Output::default();
    for _ in 0..125 {
        g.generate(&mut x);
        out.clear();
        rx.feed(&x, &mut out);
    }
    // 12.5 s: the file of 12:00:00 to 12:00:10 is written, the next one is open.
    assert_eq!(rx.results().fields().at(&BurstFields::FILES), Some(1));
    let path = StorePath::new("callisto/SYDNEY1_20260320_120000_59.fit").unwrap();
    let bytes = storage.read(&path).unwrap();
    assert_eq!(bytes.len() % 2880, 0);
    // The primary header, card by card, in the order of the manual.
    let cards: Vec<String> = bytes[..2880 * 2]
        .chunks(80)
        .map(|c| String::from_utf8_lossy(c).to_string())
        .take_while(|c| !c.starts_with("END "))
        .collect();
    let keys: Vec<&str> = cards.iter().map(|c| c[..8].trim_end()).collect();
    assert_eq!(
        keys,
        [
            "SIMPLE", "BITPIX", "NAXIS", "NAXIS1", "NAXIS2", "EXTEND", "COMMENT", "COMMENT", "COMMENT", "COMMENT", "DATE",
            "CONTENT", "ORIGIN", "TELESCOP", "INSTRUME", "OBJECT", "DATE-OBS", "TIME-OBS", "DATE-END", "TIME-END", "BZERO",
            "BSCALE", "BUNIT", "DATAMIN", "DATAMAX", "CRVAL1", "CRPIX1", "CTYPE1", "CDELT1", "CRVAL2", "CRPIX2", "CTYPE2",
            "CDELT2", "HISTORY", "OBS_LAT", "OBS_LAC", "OBS_LON", "OBS_LOC", "OBS_ALT", "FRQFILE", "PWM_VAL",
        ]
    );
    assert_eq!(cards[0], format!("{:<80}", "SIMPLE  =                    T"));
    assert_eq!(&cards[1][..30], "BITPIX  =                   16");
    assert_eq!(&cards[6], &format!("{:<80}", "COMMENT Warning: the value of CDELT1 may be rounded!"));
    let card = |k: &str| cards.iter().find(|c| c.starts_with(&format!("{k:<8}="))).unwrap().clone();
    assert!(card("DATE-OBS").starts_with("DATE-OBS= '2026/03/20'"), "{}", card("DATE-OBS"));
    assert!(card("DATE    ").starts_with("DATE    = '2026-03-20'"));
    assert!(card("TIME-OBS").starts_with("TIME-OBS= '12:00:00.000'"), "{}", card("TIME-OBS"));
    assert!(card("CONTENT").starts_with("CONTENT = '2026/03/20 Radio flux density (SYDNEY1)'"));
    assert!(card("BUNIT").starts_with("BUNIT   = 'digits  '"));
    assert!(card("CTYPE1").starts_with("CTYPE1  = 'Time [UT]'"));
    assert!(card("CTYPE2").starts_with("CTYPE2  = 'Frequency [MHz]'"));
    assert!(card("OBS_LAC").starts_with("OBS_LAC = 'S       '"));
    assert!(card("OBS_LOC").starts_with("OBS_LOC = 'E       '"));
    // The data: 40 rows of 0.25 s, 32 channels from the highest frequency down, values 0 to 255.
    let hdus = fits::read(&bytes).unwrap();
    assert_eq!(hdus.len(), 2);
    let p = &hdus[0];
    assert_eq!(p.int("NAXIS1"), Some(40));
    assert_eq!(p.int("NAXIS2"), Some(32));
    assert_eq!(p.real("CRVAL1"), Some(43_200.0));
    assert_eq!(p.real("CDELT1"), Some(0.25));
    assert_eq!(p.real("BZERO"), Some(0.0));
    assert_eq!(p.real("BSCALE"), Some(1.0));
    assert_eq!(p.real("OBS_ALT"), Some(40.0));
    assert!((p.real("CDELT2").unwrap() + 0.0625).abs() < 1e-12);
    let Data::Image { bitpix, pixels, .. } = &p.data else {
        panic!("no image")
    };
    assert_eq!(*bitpix, 16);
    assert!(pixels.iter().all(|v| (0.0..=255.0).contains(v) && v.fract() == 0.0));
    assert!(p.cards.iter().any(|c| matches!(c, Card::Text { key, .. } if key == "HISTORY")));
    // One binary table with TIME (seconds of the day) and FREQUENCY (MHz).
    let t = &hdus[1];
    assert_eq!(t.str("XTENSION"), Some("BINTABLE"));
    assert_eq!(t.int("TFIELDS"), Some(2));
    assert_eq!(t.int("NAXIS2"), Some(1));
    let time = &t.column("TIME").unwrap().values;
    let freq = &t.column("FREQUENCY").unwrap().values;
    assert_eq!(time.len(), 40);
    assert_eq!((time[0], time[1]), (43_200.0, 43_200.25));
    assert_eq!(freq.len(), 32);
    assert!((freq[0] - (200.0 + 1.0 - 0.03125)).abs() < 1e-9, "{}", freq[0]);
    assert!(freq.windows(2).all(|w| w[1] < w[0]));
    // The reader of the stack takes the same file and gives the date with dashes.
    let (rt, rf, rd, date) = bursts::file::read_file(&bytes).unwrap();
    assert_eq!((rt.len(), rf.len(), rd.len(), date.as_str()), (40, 32, 1280, "2026-03-20"));
    // The end of the input writes the open file: 2.5 s from 12:00:10.
    out.clear();
    rx.finish(&mut out);
    assert_eq!(rx.results().fields().at(&BurstFields::FILES), Some(2));
    let last = storage
        .read(&StorePath::new("callisto/SYDNEY1_20260320_120010_59.fit").unwrap())
        .unwrap();
    let (rt, ..) = bursts::file::read_file(&last).unwrap();
    assert_eq!(rt.len(), 10);
    assert_eq!(rt[0], 43_210.0);
}

#[test]
fn a_file_of_8_bit_pixels_reads_back() {
    use bursts::file::{file_bytes, read_file, Bitpix, FileSpec};
    let spec = FileSpec {
        station: "TEST".into(),
        focus: 1,
        bitpix: Bitpix::Eight,
        db_per_digit: 0.5,
        zero_dbfs: -60.0,
        position: None,
    };
    let rows = vec![vec![1e-6f32, 1e-3, 1.0]; 4];
    let b = file_bytes(&spec, START, 0.25, &[1e6, 2e6, 3e6], &rows);
    assert_eq!(spec.name(START), "TEST_20260320_120000_01.fit");
    let (t, f, d, _) = read_file(&b).unwrap();
    assert_eq!(t.len(), 4);
    assert_eq!(f, vec![3.0, 2.0, 1.0]);
    // Highest frequency first: 0 dBFS is 120 digits, -30 dBFS 60, -60 dBFS 0.
    assert_eq!(&d[..4], &[120.0; 4]);
    assert_eq!(&d[4..8], &[60.0; 4]);
    assert_eq!(&d[8..], &[0.0; 4]);
    let h = fits::read(&b).unwrap();
    assert_eq!(h[0].int("BITPIX"), Some(8));
}
