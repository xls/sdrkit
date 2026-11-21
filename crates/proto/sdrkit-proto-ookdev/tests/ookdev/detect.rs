//! The pulse detector on generated signals.

use crate::common::{bits_of, fsk_iq, ook_iq, pairs_us};
use sdrkit_core::Complex32;
use sdrkit_proto_ookdev::{
    burst_tones, encode, level_train, tone_train, Coding, DetectorConfig, Error, Modulation,
    PulseDetector, Slicer, Timing, Train,
};

fn run(cfg: DetectorConfig, x: &[Complex32], piece: usize) -> Vec<Train> {
    let mut d = PulseDetector::new(cfg).unwrap();
    let mut v = Vec::new();
    for p in x.chunks(piece) {
        d.process(p, |t| v.push(t.clone()));
    }
    d.flush(|t| v.push(t.clone()));
    v
}

fn pwm_pairs() -> Vec<(f32, f32)> {
    (0..24)
        .map(|k| {
            if k % 3 == 0 {
                (1404.0, 464.0)
            } else {
                (464.0, 1404.0)
            }
        })
        .collect()
}

/// The edges sit half way between the floor and the pulse level, so the widths do not depend on the level: a 60 dB and a 30 dB
/// burst give the same widths within a microsecond.
#[test]
fn ook_widths_do_not_depend_on_level() {
    let pairs = pwm_pairs();
    for amp in [1.0f32, 0.03] {
        let x = ook_iq(&pairs, 1e6, amp, 3000.0, 1e-6, 7);
        let got = run(DetectorConfig::new(1e6), &x, 4096);
        assert_eq!(got.len(), 1, "amp {amp}");
        let t = &got[0];
        assert_eq!(t.modulation, Modulation::Ook);
        let us = pairs_us(t);
        assert_eq!(us.len(), pairs.len());
        for (k, (a, b)) in us.iter().zip(&pairs).enumerate() {
            assert!(
                (a.0 - b.0).abs() < 1.0,
                "amp {amp} mark {k}: {} against {}",
                a.0,
                b.0
            );
            if k + 1 < pairs.len() {
                assert!(
                    (a.1 - b.1).abs() < 1.0,
                    "amp {amp} space {k}: {} against {}",
                    a.1,
                    b.1
                );
            }
        }
        let want_db = 20.0 * amp.log10();
        assert!((t.level_dbfs - want_db).abs() < 1.0, "{}", t.level_dbfs);
        assert!(t.snr_db > want_db + 50.0, "{}", t.snr_db);
        assert!((t.time_s() - 3e-3).abs() < 2e-6, "{}", t.time_s());
    }
}

/// The trains do not depend on how the input is split across calls.
#[test]
fn stream_cuts_do_not_matter() {
    let x = ook_iq(&pwm_pairs(), 1e6, 0.2, 3000.0, 1e-5, 3);
    let a = run(DetectorConfig::new(1e6), &x, x.len());
    for piece in [1, 7, 333] {
        assert_eq!(
            run(DetectorConfig::new(1e6), &x, piece),
            a,
            "pieces of {piece}"
        );
    }
}

/// A space longer than the reset limit ends a train. The next transmission starts another.
#[test]
fn reset_splits_transmissions() {
    let mut pairs = pwm_pairs();
    pairs.last_mut().unwrap().1 = 25_000.0;
    pairs.extend(pwm_pairs());
    let x = ook_iq(&pairs, 1e6, 0.5, 3000.0, 1e-6, 1);
    let got = run(DetectorConfig::new(1e6), &x, 4096);
    assert_eq!(got.len(), 2);
    assert_eq!(got[0].pulses.len(), 24);
    // The last space is the gap until the train ended: the reset limit.
    let last = got[0].us(got[0].pulses[23].space);
    assert!((last - 10_000.0).abs() < 50.0, "{last}");
    assert!(got[1].time_s() > got[0].time_s() + 0.04);
}

/// FSK at 10 kbit/s, deviation 30 kHz, 50 kHz above the centre: the detector finds the tones and the marks, and NRZ slicing
/// recovers the bits.
#[test]
fn fsk_carrier() {
    let bytes = [0xAA, 0xAA, 0xAA, 0x2D, 0xD4, 0x5E, 0x01, 0xC3, 0x7F, 0x80];
    let mut tr = Train::new(Modulation::Fsk, 1e6);
    encode::nrz(&bits_of(&bytes), 100.0, &mut tr);
    let pairs = pairs_us(&tr);
    let x = fsk_iq(&pairs, 500e3, 50e3, 30e3, 2000.0, 1e-4, 11);
    let got = run(DetectorConfig::new(500e3), &x, 4096);
    assert_eq!(got.len(), 1);
    let t = &got[0];
    assert_eq!(t.modulation, Modulation::Fsk);
    assert!((t.deviation_hz - 30e3).abs() < 3e3, "{}", t.deviation_hz);
    assert!((t.offset_hz - 50e3).abs() < 3e3, "{}", t.offset_hz);
    let mut s = Slicer::new();
    let mut bits = Vec::new();
    s.slice(&Timing::new(Coding::Nrz, 100.0, 100.0, 1000.0), t, |r| {
        let mut b = [0u8; 10];
        r.extract(0, 0, 80, &mut b).unwrap();
        bits.push((r.len(0), b));
    })
    .unwrap();
    assert_eq!(bits.len(), 1);
    assert_eq!(bits[0].1, bytes);
}

/// With one modulation chosen, the other returns no trains.
#[test]
fn one_modulation_only() {
    let tr = {
        let mut t = Train::new(Modulation::Fsk, 1e6);
        encode::nrz(&bits_of(&[0xAA; 8]), 100.0, &mut t);
        t
    };
    let x = fsk_iq(&pairs_us(&tr), 500e3, 0.0, 30e3, 2000.0, 1e-4, 5);
    let mut cfg = DetectorConfig::new(500e3);
    cfg.modulation = Some(Modulation::Ook);
    assert!(run(cfg, &x, 4096).is_empty());
    let y = ook_iq(&pwm_pairs(), 500e3, 0.5, 3000.0, 1e-6, 2);
    cfg.modulation = Some(Modulation::Fsk);
    assert!(run(cfg, &y, 4096).is_empty());
    cfg.modulation = None;
    assert_eq!(run(cfg, &x, 4096)[0].modulation, Modulation::Fsk);
}

/// The burst form produces the same trains as the stream form, counted from the burst's first sample.
#[test]
fn burst_form() {
    let x = ook_iq(&pwm_pairs(), 1e6, 0.3, 1000.0, 1e-6, 9);
    let mut d = PulseDetector::new(DetectorConfig::new(1e6)).unwrap();
    let mut v = Vec::new();
    d.run_burst(&x, 1e-6, |t| v.push(t.clone()));
    assert_eq!(v.len(), 1);
    assert_eq!(v[0].pulses.len(), 24);
    assert!((v[0].time_s() - 1e-3).abs() < 2e-6);
}

#[test]
fn bad_samples_and_settings() {
    let mut x = ook_iq(&pwm_pairs(), 1e6, 0.3, 3000.0, 1e-6, 4);
    x[100] = Complex32::new(f32::NAN, 0.0);
    x[5000] = Complex32::new(f32::INFINITY, 1.0);
    let _ = run(DetectorConfig::new(1e6), &x, 512);
    let mut c = DetectorConfig::new(1e6);
    c.close_db = 20.0;
    assert!(matches!(PulseDetector::new(c), Err(Error::BadParameter(_))));
    assert!(PulseDetector::new(DetectorConfig::new(0.0)).is_err());
}

/// An FSK burst at 5 dB in 100 kHz (100 µs bits, +-30 kHz around 20 kHz): the two tones are found in its spectrum within
/// 3 kHz, and their energies over 96 µs give every bit.
#[test]
fn tone_energies_read_a_weak_fsk_burst() {
    let bytes = [0xAA, 0xAA, 0xAA, 0x2D, 0xD4, 0x5E, 0x01, 0xC3, 0x7F, 0x80];
    let mut tr = Train::new(Modulation::Fsk, 1e6);
    encode::nrz(&bits_of(&bytes), 100.0, &mut tr);
    // Signal power 0.25; noise of 0.079 in 100 kHz is 0.395 in the 500 kHz of the samples.
    let x = fsk_iq(&pairs_us(&tr), 500e3, 20e3, 30e3, 2000.0, 0.395, 5);
    let (mid, dev) = burst_tones(&x, 500e3, 4000.0).unwrap();
    assert!((mid - 20e3).abs() < 3e3 && (dev - 30e3).abs() < 3e3, "{mid} {dev}");
    let t = tone_train(&x, 500e3, mid, dev, 48).unwrap();
    assert_eq!(t.modulation, Modulation::Fsk);
    let mut bits = Vec::new();
    Slicer::new()
        .slice(&Timing::new(Coding::Nrz, 100.0, 100.0, 1000.0), &t, |r| {
            let mut b = [0u8; 10];
            r.extract(0, 0, 80, &mut b).unwrap();
            bits.push(b);
        })
        .unwrap();
    assert_eq!(bits, [bytes]);
}

/// An OOK burst at 5 dB in 100 kHz: its magnitude over 160 µs, sliced between its two levels, gives every pulse within 15 %.
#[test]
fn two_levels_read_a_weak_ook_burst() {
    let pairs = pwm_pairs();
    // Mark power 1; noise of 0.316 in 100 kHz is 1.58 in the 500 kHz of the samples.
    let x = ook_iq(&pairs, 500e3, 1.0, 2000.0, 1.58, 9);
    let env: Vec<f32> = (0..x.len() - 80)
        .map(|k| (x[k..k + 80].iter().sum::<Complex32>() / 80.0).norm())
        .collect();
    let t = level_train(&env, 500e3).unwrap();
    let got = pairs_us(&t);
    assert_eq!(got.len(), pairs.len());
    for (k, (g, w)) in got.iter().zip(&pairs).enumerate() {
        assert!((g.0 - w.0).abs() < 0.15 * w.0, "mark {k}: {g:?}");
        if k + 1 < pairs.len() {
            assert!((g.1 - w.1).abs() < 0.15 * w.1, "space {k}: {g:?}");
        }
    }
}
