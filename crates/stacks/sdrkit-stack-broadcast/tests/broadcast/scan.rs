//! Band scans: the bands from the data folder, the steps, the scan centres, the signal finder, and scans of generated stations
//! (a recording scanned as one capture, and a live radio stepped over a band).

use super::support::*;
use sdrkit_core::{Complex32, ConfigError, InputInfo, Output, SettingSet, Values};
use serde_json::json;

/// The data folder of the repository.
fn data() -> &'static sdrkit_presets::Data {
    static DATA: std::sync::OnceLock<sdrkit_presets::Data> = std::sync::OnceLock::new();
    DATA.get_or_init(|| {
        sdrkit_presets::Data::open(concat!(env!("CARGO_MANIFEST_DIR"), "/../../../data")).unwrap()
    })
}

/// The bands of `stack` in the data folder for the medium wave raster `mw_raster_hz`.
fn bands(stack: &str, mw_raster_hz: f64) -> Vec<Band> {
    data()
        .broadcast_bands(stack, mw_raster_hz)
        .unwrap()
        .into_iter()
        .filter_map(|b| {
            Band::new(
                &b.stack,
                &b.id,
                &b.name,
                b.low_hz,
                b.high_hz,
                b.first_hz,
                b.raster_hz,
            )
            .map(|x| x.shortcut(b.shortcut_hz))
        })
        .collect()
}

/// The band `id` of `stack` in the data folder for the medium wave raster `mw_raster_hz`.
fn band(stack: &str, id: &str, mw_raster_hz: f64) -> Option<Band> {
    bands(stack, mw_raster_hz).into_iter().find(|b| b.id == id)
}

/// The settings of the stack of `b` at their defaults.
fn defaults(b: &Band) -> Values {
    Values::defaults(&(descriptor(b.stack).expect("a scanned stack").settings)())
}
use sdrkit_stack_broadcast::cquam::{CquamGenerator, GenKind, GenOptions};
use sdrkit_stack_broadcast::drm::{params::Robustness, tx::TxConfig, DrmGenerator};
use sdrkit_stack_broadcast::scan::*;

/// C-QUAM stations at absolute frequencies, as a radio centred on `centre` at `rate` hears them, `seconds` long, each 45 dB over
/// the noise in its channel. A station outside the radio's band is not heard (its filter takes it out).
fn am_capture(
    rate: f64,
    centre: f64,
    stations: &[(f64, GenKind)],
    seconds: f64,
    seed: u64,
) -> Vec<Complex32> {
    // The radio's own white noise floor.
    let mut r = sdrkit_algo_sim::rng::Rng::new(seed + 100);
    let mut x: Vec<Complex32> = (0..(rate * seconds).round() as usize)
        .map(|_| Complex32::new(r.normal_f32() * 1e-3, r.normal_f32() * 1e-3))
        .collect();
    for (i, (f, kind)) in stations.iter().enumerate() {
        if (f - centre).abs() > 0.45 * rate {
            continue;
        }
        let g = CquamGenerator::new(
            rate,
            GenOptions {
                kind: *kind,
                centre_offset_hz: f - centre,
                ..GenOptions::default()
            },
        );
        let mut s = snr(45.0).to_vec();
        s.push(("gen_seed", json!(seed + i as u64)));
        let part = impair(Box::new(g), seconds, &s);
        for (a, b) in x.iter_mut().zip(&part) {
            *a += *b;
        }
    }
    x
}

/// Runs a scan over a capture until it is done or the samples end; returns the requests seen.
fn run(scan: &mut BandScan, x: &[Complex32], piece: usize) -> Vec<ScanRequest> {
    let mut out = Output::default();
    let mut seen = Vec::new();
    for c in x.chunks(piece) {
        let r = scan.feed(c, &mut out);
        out.clear();
        if r != ScanRequest::Continue {
            seen.push(r);
        }
        if scan.is_done() {
            break;
        }
    }
    seen
}

/// A clean generator moved by `offset_hz`, `snr_db` over the noise in its own bandwidth.
fn shifted(
    g: Box<dyn sdrkit_core::Generator>,
    seconds: f64,
    offset_hz: f64,
    snr_db: f64,
    seed: u64,
) -> Vec<Complex32> {
    let mut s = snr(snr_db).to_vec();
    s.push(("gen_offset_hz", json!(offset_hz)));
    s.push(("gen_seed", json!(seed)));
    impair(g, seconds, &s)
}

#[test]
fn a_recording_with_two_am_stations_lists_stereo_and_mono() {
    let rate = 1e6;
    let centre = 1100e3;
    let x = am_capture(
        rate,
        centre,
        &[(999e3, GenKind::Stereo), (891e3, GenKind::PlainAm)],
        12.0,
        1,
    );
    let b = band("cquam", "mw", 9e3).unwrap();
    let mut s = BandScan::new(b.clone(), defaults(&b), ScanSettings::default()).unwrap();
    let r = s
        .configure(&InputInfo::new(rate, centre, 999e3).recording())
        .unwrap();
    assert_eq!(r, ScanRequest::Continue);
    let seen = run(&mut s, &x, 32_768);
    assert!(s.is_done(), "{:?}", s.results().log_lines());
    assert_eq!(seen.last(), Some(&ScanRequest::Done(999e3)));
    let st = s.stations();
    assert_eq!(st.len(), 2, "{st:?}");
    let mono = st.iter().find(|s| s.freq_hz == 891e3).expect("891 kHz");
    assert_eq!(mono.state, StationState::Found);
    assert_eq!(mono.label, "AM mono");
    let stereo = st.iter().find(|s| s.freq_hz == 999e3).expect("999 kHz");
    assert_eq!(stereo.state, StationState::Decoded);
    assert_eq!(stereo.label, "AM stereo");
    assert!(stereo.over_floor_db > 20.0, "{stereo:?}");
    assert_eq!(row_ids(s.results(), "stations").len(), 2);
    assert!(s
        .results()
        .status()
        .summary
        .starts_with("Scan done, 2 stations. Strongest: 999 kHz"));
}

#[test]
fn a_live_scan_steps_the_radio_with_host_requests_and_tunes_to_the_strongest() {
    let rate = 1e6;
    let stations = [(1008e3, GenKind::Stereo), (603e3, GenKind::Stereo)];
    let b = band("cquam", "mw", 9e3).unwrap();
    let mut s = BandScan::new(
        b.clone(),
        defaults(&b),
        ScanSettings {
            min_over_db: 10.0,
            ..ScanSettings::default()
        },
    )
    .unwrap();
    let mut req = s.configure(&InputInfo::new(rate, 1e6, 702e3)).unwrap();
    let mut visited = Vec::new();
    let mut out = Output::default();
    let mut seed = 10;
    loop {
        match req.clone() {
            ScanRequest::Radio(h) => {
                assert!(matches!(h, sdrkit_core::HostRequest::Retune { .. }));
                let c = req.radio_hz().unwrap();
                visited.push(c);
                s.radio_tuned(c);
                // The radio now hears the band around `c`: up to 12 s of it.
                seed += 1;
                let x = am_capture(rate, c, &stations, 12.0, seed);
                req = ScanRequest::Continue;
                for piece in x.chunks(20_000) {
                    req = s.feed(piece, &mut out);
                    out.clear();
                    if req != ScanRequest::Continue {
                        break;
                    }
                }
                assert_ne!(
                    req,
                    ScanRequest::Continue,
                    "a centre took more than 12 s: {:?}",
                    s.results().log_lines()
                );
            }
            ScanRequest::Done(f) => {
                assert!(f == 1008e3 || f == 603e3, "{f}");
                break;
            }
            ScanRequest::Continue => unreachable!(),
        }
    }
    assert_eq!(visited, s.centres());
    let mut f: Vec<f64> = s.stations().iter().map(|x| x.freq_hz).collect();
    f.sort_by(f64::total_cmp);
    assert_eq!(f, vec![603e3, 1008e3], "{:?}", s.results().log_lines());
    assert!(s
        .stations()
        .iter()
        .all(|x| x.state == StationState::Decoded));
}

#[test]
fn a_drm_station_in_a_recording_is_found_with_its_label() {
    let rate = 250e3;
    let centre = 9.6e6;
    let station = 9.54e6;
    let secs = 16.0;
    let g = DrmGenerator::new(TxConfig::usual(Robustness::B), rate).expect("valid configuration");
    let x = shifted(Box::new(g), secs, station - centre, 30.0, 9);
    let b = band("drm", "sw31", 9e3).unwrap();
    let mut s = BandScan::new(b.clone(), defaults(&b), ScanSettings::default()).unwrap();
    s.configure(&InputInfo::new(rate, centre, 9.5e6).recording())
        .unwrap();
    let seen = run(&mut s, &x, 25_000);
    assert!(s.is_done(), "{:?}", s.results().log_lines());
    assert_eq!(
        seen.last(),
        Some(&ScanRequest::Done(station)),
        "{:?}",
        s.results().log_lines()
    );
    let st = s.stations();
    assert_eq!(st.len(), 1, "{st:?} {:?}", s.results().log_lines());
    assert_eq!(st[0].state, StationState::Decoded);
    assert_eq!(st[0].label, "Test DRM");
    assert!(st[0].at_s < secs);
}

#[test]
fn an_hd_am_station_in_a_recording_is_found_with_its_call_sign() {
    use sdrkit_stack_broadcast::hd::{self, HdGenerator, Waveform};
    let rate = 1e6;
    let centre = 1000e3;
    let station = 1120e3;
    let secs = 16.0;
    let g = HdGenerator::new(
        hd::GenOptions {
            waveform: Waveform::Ma1,
            analog_db: 0.0,
        },
        rate,
    )
    .unwrap();
    let x = shifted(Box::new(g), secs, station - centre, 30.0, 5);
    let b = band("hdradio", "am", 10e3).unwrap();
    let mut s = BandScan::new(b.clone(), defaults(&b), ScanSettings::default()).unwrap();
    s.configure(&InputInfo::new(rate, centre, 1030e3).recording())
        .unwrap();
    let seen = run(&mut s, &x, 50_000);
    assert!(s.is_done(), "{:?}", s.results().log_lines());
    assert_eq!(
        seen.last(),
        Some(&ScanRequest::Done(station)),
        "{:?}",
        s.results().log_lines()
    );
    let st = s.stations();
    assert_eq!(st.len(), 1, "{st:?}");
    assert_eq!(st[0].state, StationState::Decoded);
    assert_eq!(st[0].label, "WTST", "{st:?}");
}

#[test]
fn a_cdr_multiplex_in_a_recording_is_found_with_its_network() {
    use sdrkit_stack_broadcast::cdr::{tables::CdrTables, CdrGenerator, GenOptions as CdrOptions};
    let tables = format!(
        "{}/../../../tests/vectors/broadcast/cdr/self-check-tables.json",
        env!("CARGO_MANIFEST_DIR")
    );
    let rate = 2e6;
    let centre = 106.0e6;
    let station = 106.3e6;
    let secs = 10.0;
    let g = CdrGenerator::new(
        CdrOptions {
            tables: Some(std::sync::Arc::new(CdrTables::load(&tables).unwrap())),
            ..CdrOptions::default()
        },
        rate,
    )
    .unwrap();
    let x = shifted(Box::new(g), secs, station - centre, 30.0, 7);
    let b = band("cdr", "fm", 9e3).unwrap();
    let v = values(
        &sdrkit_stack_broadcast::cdr::CdrSetting::schema(),
        &[("cdr_tables", json!(tables.as_str()))],
    );
    let mut s = BandScan::new(b, v, ScanSettings::default()).unwrap();
    s.configure(&InputInfo::new(rate, centre, 106.1e6).recording())
        .unwrap();
    let seen = run(&mut s, &x, 100_000);
    assert!(s.is_done(), "{:?}", s.results().log_lines());
    assert_eq!(
        seen.last(),
        Some(&ScanRequest::Done(station)),
        "{:?}",
        s.results().log_lines()
    );
    let st = s.stations();
    assert_eq!(st.len(), 1, "{st:?}");
    assert_eq!(st[0].state, StationState::Decoded);
    assert!(!st[0].label.is_empty(), "{st:?}");
}

#[test]
fn plans_match_the_tuning_tables() {
    let d = bands("drm", 9e3);
    let sc: Vec<(&str, f64)> = d.iter().map(|b| (b.name.as_str(), b.shortcut_hz)).collect();
    for (name, hz) in [
        ("SW 41 m", 7.325e6),
        ("SW 31 m", 9.5e6),
        ("SW 25 m", 11.65e6),
        ("SW 19 m", 15.1e6),
        ("VHF I", 47.1e6),
        ("VHF II", 87.6e6),
        ("VHF III", 174.1e6),
    ] {
        assert!(sc.contains(&(name, hz)), "{name}");
    }
    let fm = band("hdradio", "fm", 10e3).unwrap();
    let ch = fm.channels();
    assert_eq!(ch.len(), 100);
    assert_eq!(ch[0], 88.1e6);
    assert_eq!(*ch.last().unwrap(), 107.9e6);
    let am10 = band("hdradio", "am", 10e3).unwrap();
    assert_eq!(am10.shortcut_hz, 1030e3);
    assert_eq!(am10.channels().len(), 117);
    let am9 = band("cquam", "mw", 9e3).unwrap();
    assert_eq!(am9.channels().len(), 120);
    for p in data().cquam_presets().unwrap() {
        // The presets are stations of the band (1570 kHz is between two 9 kHz channels: the receiver pulls it in).
        assert!(am9.contains(p), "{p}");
        assert!((am9.nearest_channel(p) - p).abs() <= 4.5e3, "{p}");
    }
    let cdr = band("cdr", "fm", 9e3).unwrap();
    assert_eq!(cdr.channels().len(), 206);
    assert!(cdr.channels().contains(&106.1e6));
    assert!(bands("hamdrm", 9e3).is_empty());
    assert!(profile("hamdrm").is_none());
    // Every band lies inside its mode's tuning range.
    for m in ["drm", "hdradio", "cdr", "cquam"] {
        let t = &descriptor(m).unwrap().tuning;
        for b in bands(m, 10e3).into_iter().chain(bands(m, 9e3)) {
            assert!(
                b.low_hz >= t.min_hz && b.high_hz <= t.max_hz,
                "{m} {}",
                b.id
            );
            assert!(b.contains(b.shortcut_hz), "{m} {}", b.id);
        }
    }
}

#[test]
fn steps_and_band_memory() {
    assert_eq!(step(&bands("drm", 9e3), "drm", 9.5e6, true, 9e3), 9.6e6);
    assert_eq!(step(&bands("drm", 9e3), "drm", 9.5e6, false, 9e3), 9.4e6);
    assert_eq!(
        step(&bands("hdradio", 10e3), "hdradio", 98.5e6, true, 10e3),
        98.7e6
    );
    assert_eq!(
        step(&bands("hdradio", 10e3), "hdradio", 98.6e6, false, 10e3),
        98.5e6
    );
    assert_eq!(
        step(&bands("hdradio", 10e3), "hdradio", 1030e3, true, 10e3),
        1040e3
    );
    assert_eq!(
        step(&bands("hdradio", 10e3), "hdradio", 1035e3, true, 10e3),
        1040e3
    );
    assert_eq!(
        step(&bands("cquam", 9e3), "cquam", 999e3, true, 9e3),
        1008e3
    );
    assert_eq!(
        step(&bands("cquam", 10e3), "cquam", 999e3, false, 10e3),
        990e3
    );
    assert_eq!(step(&bands("cdr", 9e3), "cdr", 106.1e6, true, 9e3), 106.2e6);
    // The band edge holds.
    assert_eq!(
        step(&bands("hdradio", 10e3), "hdradio", 107.9e6, true, 10e3),
        107.9e6
    );
    let mut m = BandMemory::new(bands("hdradio", 10e3));
    assert_eq!(m.switch_to("fm"), Some(98.5e6));
    assert_eq!(m.switch_to("am"), Some(1030e3));
    m.tuned(101.1e6);
    m.tuned(880e3);
    assert_eq!(m.switch_to("fm"), Some(101.1e6));
    assert_eq!(m.switch_to("am"), Some(880e3));
}

#[test]
fn centres_cover_every_channel() {
    for (mode, raster, rate) in [
        ("drm", 9e3, 2e6),
        ("drm", 10e3, 250e3),
        ("hdradio", 10e3, 2e6),
        ("cdr", 9e3, 2e6),
        ("cquam", 9e3, 1e6),
        ("cquam", 10e3, 250e3),
    ] {
        let p = profile(mode).unwrap();
        for b in bands(mode, raster) {
            let s = ScanSettings::default();
            let reach = 0.5 * s.usable * rate - 0.5 * b.width_hz;
            let guard = if p.one_sided {
                s.dc_guard_hz + 0.5 * b.width_hz
            } else {
                0.0
            };
            if reach <= guard {
                continue;
            }
            let ch = b.channels();
            let c = centres(&ch, reach, guard, p.one_sided);
            for f in &ch {
                assert!(
                    c.iter().any(|&x| {
                        let d = f - x;
                        d.abs() <= reach && !(p.one_sided && d.abs() < guard)
                    }),
                    "{mode} {} {f}",
                    b.id
                );
            }
            if b.id == "fm" && mode == "hdradio" {
                // Six channels of 200 kHz per centre at 2 Msps: 17 centres for the 100 channels.
                assert_eq!(c.len(), 17);
            }
        }
    }
}

#[test]
fn signals_are_found_on_the_raster() {
    // A synthetic spectrum: floor 1, an AM station 4 bins wide at +27 kHz, a DRM-like flat 10 kHz signal at -63 kHz, a spike at DC.
    let rate = 250e3;
    let n = 1024;
    let bin = rate / n as f64;
    let centre = 1_000e3;
    let mut p = vec![1.0f32; n];
    let at = |hz: f64| ((hz / bin) + (n / 2) as f64).round() as usize;
    p[at(27e3 - 1.5e3)..=at(27e3 + 1.5e3)].fill(1e4);
    p[at(-63e3 - 5e3)..=at(-63e3 + 5e3)].fill(100.0);
    p[n / 2] = 1e6;
    let b = band("cquam", "mw", 9e3).unwrap();
    let c = find_signals(&p, rate, centre, &b, 8.0, |_| true);
    let f: Vec<f64> = c.iter().map(|x| x.freq_hz).collect();
    assert_eq!(f, vec![1026e3, 936e3], "{c:?}");
    assert!((c[0].over_db - 40.0).abs() < 1.0);
    assert!((c[1].over_db - 20.0).abs() < 1.0);
    // A run three stations wide produces three candidates.
    let mut q = vec![1.0f32; n];
    q[at(-30e3)..=at(30e3)].fill(1e3);
    let w = Band {
        width_hz: 20e3,
        ..b.clone()
    };
    let c = find_signals(&q, rate, centre, &w, 8.0, |_| true);
    assert_eq!(c.len(), 3, "{c:?}");
    // A filter on the channels.
    let c = find_signals(&p, rate, centre, &b, 8.0, |f| f > centre);
    assert_eq!(c.len(), 1);
    // Noise only, averaged over 64 spectra: nothing.
    let mut s = sdrkit_algo_sim::rng::Rng::new(5);
    let mut avg = vec![0.0f32; n];
    for _ in 0..64 {
        for a in avg.iter_mut() {
            *a += (s.normal().powi(2) + s.normal().powi(2)) as f32 * 0.5 / 64.0;
        }
    }
    assert!(find_signals(&avg, rate, centre, &b, 8.0, |_| true).is_empty());
}

#[test]
fn a_scan_refuses_low_rates_and_unknown_modes() {
    let b = band("drm", "vhf2", 9e3).unwrap();
    let mut s = BandScan::new(b.clone(), defaults(&b), ScanSettings::default()).unwrap();
    let e = s.configure(&InputInfo::new(96e3, 90e6, 90e6)).unwrap_err();
    assert!(
        matches!(e, ConfigError::RateTooLow { need, .. } if need == sdrkit_stack_broadcast::drm::MIN_RATE_E),
        "{e}"
    );
    let b = band("hdradio", "fm", 10e3).unwrap();
    let mut s = BandScan::new(b.clone(), defaults(&b), ScanSettings::default()).unwrap();
    assert!(matches!(
        s.configure(&InputInfo::new(1e6, 98e6, 98e6)),
        Err(ConfigError::RateTooLow { .. })
    ));
    let ham = Band {
        stack: "hamdrm",
        ..band("cquam", "mw", 9e3).unwrap()
    };
    let none = Values::defaults(&sdrkit_stack_broadcast::hamdrm::HamSetting::schema());
    assert!(BandScan::new(ham, none, ScanSettings::default()).is_err());
}

#[test]
fn an_empty_live_band_steps_every_centre_and_returns() {
    let b = band("cquam", "mw", 10e3).unwrap();
    let mut s = BandScan::new(b.clone(), defaults(&b), ScanSettings::default()).unwrap();
    let rate = 1e6;
    let r = s.configure(&InputInfo::new(rate, 1e6, 1000e3)).unwrap();
    let n = s.centres().len();
    assert!(n >= 2);
    let mut req = r;
    let mut visited = Vec::new();
    let mut noise = sdrkit_algo_sim::rng::Rng::new(3);
    let mut buf = vec![Complex32::new(0.0, 0.0); 50_000];
    let mut out = Output::default();
    for _ in 0..1000 {
        match req {
            ScanRequest::Radio(_) => {
                let c = req.radio_hz().unwrap();
                visited.push(c);
                s.radio_tuned(c);
                req = ScanRequest::Continue;
            }
            ScanRequest::Done(f) => {
                assert_eq!(f, 1000e3);
                break;
            }
            ScanRequest::Continue => {
                buf.iter_mut().for_each(|x| {
                    *x = Complex32::new(noise.normal_f32() * 7e-4, noise.normal_f32() * 7e-4)
                });
                req = s.feed(&buf, &mut out);
            }
        }
    }
    assert!(s.is_done());
    assert_eq!(visited.len(), n);
    assert!(s.stations().is_empty());
    assert_eq!(s.results().status().summary, "Scan done. No station found.");
    // Stopping again only repeats where to go.
    assert_eq!(s.stop(), ScanRequest::Done(1000e3));
}

/// The scan of each stack with the values of two rows of the data file: the medium wave band at 9 kHz and at 10 kHz (DRM,
/// HD Radio and C-QUAM), and for CDR its FM band and the values of the FM band of HD Radio. Each live scan steps the radio to
/// centres inside the band of its row, and the two rows give two plans of centres.
#[test]
fn each_stack_scans_the_values_of_two_rows_of_the_data_file() {
    let hd_fm = band("hdradio", "fm", 10e3).unwrap();
    let cdr_us = Band::new(
        "cdr",
        "fm",
        "FM",
        hd_fm.low_hz,
        hd_fm.high_hz,
        hd_fm.first_hz,
        hd_fm.raster_hz,
    )
    .unwrap();
    let pairs = [
        (
            band("drm", "mw", 9e3).unwrap(),
            band("drm", "mw", 10e3).unwrap(),
            250e3,
        ),
        (
            band("hdradio", "am", 9e3).unwrap(),
            band("hdradio", "am", 10e3).unwrap(),
            250e3,
        ),
        (
            band("cquam", "mw", 9e3).unwrap(),
            band("cquam", "mw", 10e3).unwrap(),
            250e3,
        ),
        (band("cdr", "fm", 9e3).unwrap(), cdr_us, 2e6),
    ];
    for (a, b, rate) in pairs {
        assert_ne!(a.channels(), b.channels(), "{}", a.stack);
        let mut plans = Vec::new();
        for band in [a, b] {
            let mut s =
                BandScan::new(band.clone(), defaults(&band), ScanSettings::default()).unwrap();
            let r = s
                .configure(&InputInfo::new(rate, 1e6, band.shortcut_hz))
                .unwrap();
            let first = r.radio_hz().expect("a first centre");
            assert!(
                band.low_hz - rate <= first && first <= band.high_hz + rate,
                "{} {first}",
                band.stack
            );
            assert!(s.centres().len() >= 2, "{}", band.stack);
            plans.push(s.centres().to_vec());
        }
        assert_ne!(plans[0], plans[1]);
    }
}
