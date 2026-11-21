//! The marker part of ILS: each marker named in noise with its keying rate, a steady tone that is not a marker, the worked
//! passages through the detector, and the choice of the part by frequency.

use crate::common::{get, rows, run};
use sdrkit_algo_sim::chain::{NoiseKind, GEN_NOISE, GEN_SNR_DB};
use sdrkit_core::Complex32;
use sdrkit_dev_test::{Harness, Run};
use sdrkit_stack_avnav::ils::{
    IlsPart, GEN_MARKER, GEN_MARKER_KEYING, GEN_MARKER_START, GEN_MARKER_STOP,
};
use sdrkit_stack_avnav::marker::{
    GenMarker, GenMarkerKeying, MarkerDetector, MarkerKind, MarkerSummary, PassageKind, CARRIER_HZ,
};
use sdrkit_stack_avnav::typed::{IlsFields, MarkerFields, MarkerPassageRow, MARKER_PASSAGES};
use sdrkit_stack_avnav::ILS;

/// `m` keyed from 1 s to 9 s with `snr_db` in 12 kHz, 11 s at 48 ksps.
fn marker_run(m: GenMarker, keying: GenMarkerKeying, snr_db: f64, seed: u64) -> Run {
    run(&Harness::new(&ILS)
        .submode("marker")
        .freq(CARRIER_HZ)
        .rate(48e3)
        .seconds(11.0)
        .seed(seed)
        .set(&GEN_NOISE, NoiseKind::Snr)
        .set(&GEN_SNR_DB, snr_db)
        .set(&GEN_MARKER, m)
        .set(&GEN_MARKER_KEYING, keying)
        .set(&GEN_MARKER_START, 1.0)
        .set(&GEN_MARKER_STOP, 9.0))
}

/// The kind and marker of every passage row.
fn passages(r: &Run) -> Vec<(PassageKind, MarkerKind)> {
    rows(&r.results, &MARKER_PASSAGES)
        .iter()
        .map(|row| {
            (
                row.at(&MarkerPassageRow::KIND).unwrap(),
                row.at(&MarkerPassageRow::MARKER).unwrap(),
            )
        })
        .collect()
}

const MARKERS: [(GenMarker, MarkerKind); 3] = [
    (GenMarker::Outer, MarkerKind::Outer),
    (GenMarker::Middle, MarkerKind::Middle),
    (GenMarker::Inner, MarkerKind::Inner),
];

#[test]
fn each_marker_is_named_at_20_and_10_db() {
    for (snr, seed) in [(20.0, 1), (10.0, 2)] {
        for (m, want) in MARKERS {
            let r = marker_run(m, GenMarkerKeying::Standard, snr, seed);
            let f = &r.results;
            assert_eq!(
                passages(&r),
                [(PassageKind::Marker, want)],
                "{want:?} at {snr} dB: {:?}",
                f.fields()
            );
            assert_eq!(get(f, &MarkerFields::MARKER), Some(want));
            assert_eq!(get(f, &MarkerFields::KEYING_FITS), Some(true));
            assert_eq!(get(f, &MarkerFields::PASSAGES), Some(1));
            let depth = get(f, &MarkerFields::MARKER_DEPTH_PCT).unwrap_or(f64::NAN);
            // The envelope's mean holds the noise as well as the carrier, so the depth reads low at 10 dB.
            let low = if snr < 15.0 { 70.0 } else { 87.0 };
            assert!(
                (low..103.0).contains(&depth),
                "{want:?} at {snr} dB: depth {depth}"
            );
            let tone = get(f, &MarkerFields::MARKER_TONE_HZ).unwrap_or(f64::NAN);
            let nominal = want.tone_hz().unwrap();
            assert!((tone - nominal).abs() < 5.0, "{want:?}: tone {tone}");
            assert_eq!(r.events_of("ils.marker").count(), 1);
            // The keyer of the identification hears nothing in a marker tone.
            assert_eq!(r.events_of("morse.word").count(), 0, "{want:?}");
        }
    }
}

#[test]
fn the_keying_rate_is_measured_within_5_percent() {
    for (m, want) in MARKERS {
        let r = marker_run(m, GenMarkerKeying::Standard, 20.0, 3);
        let f = &r.results;
        let (got, sent) = match want {
            MarkerKind::Middle => (get(f, &MarkerFields::PAIRS_PER_MIN), 90.0),
            MarkerKind::Inner => (get(f, &MarkerFields::KEYING_RATE), 6.0),
            _ => (get(f, &MarkerFields::KEYING_RATE), 2.0),
        };
        let got = got.unwrap_or(f64::NAN);
        assert!(
            (got / sent - 1.0).abs() < 0.05,
            "{want:?}: rate {got}, sent {sent}"
        );
    }
}

/// Keyed from the first sample: the first element is cut by the start of the receiver, and the marker is still named.
#[test]
fn a_marker_keyed_from_the_first_sample() {
    for (m, want) in MARKERS {
        let r = run(&Harness::new(&ILS)
            .submode("marker")
            .freq(CARRIER_HZ)
            .rate(250e3)
            .seconds(8.0)
            .seed(7)
            .set(&GEN_NOISE, NoiseKind::Snr)
            .set(&GEN_SNR_DB, 20.0)
            .set(&GEN_MARKER, m)
            .set(&GEN_MARKER_STOP, 6.0));
        assert_eq!(passages(&r), [(PassageKind::Marker, want)], "{want:?}");
    }
}

#[test]
fn a_steady_1300_hz_tone_is_not_a_middle_marker() {
    let r = marker_run(GenMarker::Middle, GenMarkerKeying::Steady, 20.0, 4);
    let f = &r.results;
    assert!(
        !passages(&r).contains(&(PassageKind::Marker, MarkerKind::Middle)),
        "{:?}",
        passages(&r)
    );
    assert_ne!(get(f, &MarkerFields::KEYING_FITS), Some(true));
    assert_eq!(r.events_of("ils.marker").count(), 0);
}

#[test]
fn the_part_follows_the_frequency() {
    let x = Harness::new(&ILS)
        .rate(48e3)
        .submode("marker")
        .seconds(5.0)
        .generate()
        .unwrap();
    let r = Harness::new(&ILS).rate(48e3).freq(75.0e6).feed(&x).unwrap();
    let f = &r.results;
    assert_eq!(get(f, &IlsFields::PART), Some(IlsPart::Marker));
    assert_eq!(get(f, &MarkerFields::MARKER), Some(MarkerKind::Outer));
    assert_eq!(get(f, &MarkerFields::KEYING_FITS), Some(true));
}

/// Ideal periods of 10 ms through the detector: a tone of index `tone` (400, 1300, 3000 Hz) at depth 0.95 while a keying
/// element covers the middle of the period, from `from` s to `to` s, with elements of `(on, off)` seconds in turn. Returns
/// every passage that ended, with 3 s of silence after.
fn worked(tone: usize, elements: &[(f64, f64)], from: f64, to: f64) -> Vec<MarkerSummary> {
    let period: f64 = elements.iter().map(|(on, off)| on + off).sum();
    let mut d = MarkerDetector::new(16e3, 160);
    let mut out = Vec::new();
    for i in 0..((to + 3.0) / 0.01) as usize {
        let mid = (i as f64 + 0.5) * 0.01;
        let mut on = false;
        if (from..to).contains(&mid) {
            let mut at = (mid - from) % period;
            for &(a, b) in elements {
                if at < a + b {
                    on = at < a;
                    break;
                }
                at -= a + b;
            }
        }
        let mut amps = [Complex32::default(); 3];
        if on {
            amps[tone] = Complex32::new(0.95, 0.0);
        }
        out.extend(d.period((i + 1) as f64 * 0.01, &amps));
    }
    out.extend(d.finish());
    out
}

#[test]
fn worked_passages() {
    // 1: outer, 0.30 s on and 0.20 s off: 16 dashes over 7.80 s at 2 a second.
    let p = worked(0, &[(0.30, 0.20)], 1.0, 9.0);
    assert_eq!(p.len(), 1);
    let s = p[0];
    assert_eq!((s.marker, s.elements, s.dashes, s.mixed), (MarkerKind::Outer, 16, 16, false));
    assert!((s.duration_s - 7.80).abs() < 1e-6 && (s.per_s - 2.0).abs() < 1e-6);
    assert!(s.fits);
    let mut text = String::new();
    s.write_keying(&mut text);
    assert_eq!(text, "2.0 dashes per second (0.30 s long)");
    // 2: inner, 0.08 s on and 0.0867 s off: 48 dots at 6 a second.
    let p = worked(2, &[(0.08, 0.0867)], 1.0, 9.0);
    assert_eq!(p.len(), 1);
    assert_eq!((p[0].marker, p[0].elements, p[0].dots), (MarkerKind::Inner, 48, 48));
    assert!((p[0].per_s - 6.0).abs() < 0.1 && p[0].fits, "{:?}", p[0]);
    // 3: middle, a dot of 0.10 s and a dash of 0.30 s in 0.63 s: 13 of each, 95 pairs a minute.
    let p = worked(1, &[(0.10, 0.10), (0.30, 0.13)], 1.0, 9.0);
    assert_eq!(p.len(), 1);
    let s = p[0];
    assert_eq!((s.marker, s.dots, s.dashes, s.mixed), (MarkerKind::Middle, 13, 13, true));
    assert!((s.pairs_per_min.unwrap() - 95.2).abs() < 0.5 && s.fits, "{s:?}");
    // 4: 400 Hz keyed as the inner marker: tone only.
    let p = worked(0, &[(0.08, 0.0867)], 1.0, 9.0);
    assert_eq!((p.len(), p[0].marker, p[0].kind()), (1, MarkerKind::Outer, PassageKind::ToneOnly));
    // 5: 1300 Hz with dashes only: tone only.
    let p = worked(1, &[(0.30, 0.20)], 1.0, 9.0);
    assert_eq!((p.len(), p[0].kind()), (1, PassageKind::ToneOnly));
    let mut text = String::new();
    p[0].write_row(&mut text);
    assert!(text.ends_with("; the keying does not fit the standard"), "{text}");
    // 6: two elements only: dropped.
    assert!(worked(0, &[(0.30, 0.20)], 1.0, 1.9).is_empty());
}

#[test]
fn two_tones_at_once_are_no_marker() {
    let mut d = MarkerDetector::new(16e3, 160);
    let mut out = Vec::new();
    for i in 0..1000 {
        let on = (i / 25) % 2 == 0;
        let a = if on { 0.6 } else { 0.0 };
        let amps = [Complex32::new(a, 0.0), Complex32::new(a * 0.9, 0.0), Complex32::default()];
        out.extend(d.period((i + 1) as f64 * 0.01, &amps));
    }
    out.extend(d.finish());
    assert!(out.is_empty(), "{out:?}");
}

/// The carrier 2.5 kHz above and 3.5 kHz below the tuned frequency: each marker is still named. The 10 kHz channel cuts one
/// sideband of the 3000 Hz tone at -3.5 kHz, so its depth reads about half.
#[test]
fn markers_with_the_carrier_off_centre() {
    use sdrkit_algo_sim::chain::GEN_OFFSET_HZ;
    for offset in [2500.0, -3500.0] {
        for (m, want) in MARKERS {
            let r = run(&Harness::new(&ILS)
                .submode("marker")
                .freq(CARRIER_HZ)
                .rate(48e3)
                .seconds(11.0)
                .seed(5)
                .set(&GEN_NOISE, NoiseKind::Snr)
                .set(&GEN_SNR_DB, 20.0)
                .set(&GEN_OFFSET_HZ, offset)
                .set(&GEN_MARKER, m)
                .set(&GEN_MARKER_START, 1.0)
                .set(&GEN_MARKER_STOP, 9.0));
            assert_eq!(
                passages(&r),
                [(PassageKind::Marker, want)],
                "{want:?} at {offset} Hz"
            );
        }
    }
}

