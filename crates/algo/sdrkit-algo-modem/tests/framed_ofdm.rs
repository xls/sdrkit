//! The framed OFDM modem: frames through the modulator and back through the streaming demodulator, with a carrier offset, a sample
//! clock offset and noise, and a refused layout.

use sdrkit_algo_modem::ofdm::{CellFrame, FrameDemodulator, FrameLayout, FrameModulator};
use sdrkit_algo_sim::clock::ClockError;
use sdrkit_algo_sim::rng::Rng;
use sdrkit_core::Complex32;
use std::f64::consts::TAU;
use std::sync::Arc;

/// Moves `x` up by `hz` at `rate` and adds noise of power `noise` (0 for none).
fn impair(x: &mut [Complex32], hz: f64, rate: f64, noise: f64, seed: u64) {
    let mut r = Rng::new(seed);
    for (i, s) in x.iter_mut().enumerate() {
        let ph = TAU * hz * i as f64 / rate;
        *s *= Complex32::new(ph.cos() as f32, ph.sin() as f32);
        if noise > 0.0 {
            *s += r.complex_normal(noise);
        }
    }
}

fn layout() -> FrameLayout {
    let k = 101;
    let mut known = vec![Vec::new(); 10];
    let mut r = Rng::new(3);
    // symbol 0: every other carrier known
    for c in (0..k).step_by(2) {
        let ph = (r.next_u64() % 4) as f32 * std::f32::consts::FRAC_PI_2;
        known[0].push((c, Complex32::new(ph.cos(), ph.sin()) * 1.4));
    }
    for (s, row) in known.iter_mut().enumerate().skip(1) {
        for c in ((s % 3)..k).step_by(6) {
            row.push((c, Complex32::new(1.4, 0.0)));
        }
        // continuous pilots
        for &c in &[10usize, 40, 70] {
            if !row.iter().any(|x| x.0 == c) {
                row.push((c, Complex32::new(1.4, 0.0)));
            }
        }
    }
    FrameLayout {
        rate: 12_800.0,
        nfft: 256,
        cp: 32,
        cp0: 32,
        nsym: 10,
        kmin: -50,
        kmax: 50,
        known,
        null: vec![50],
        max_shift: 10,
        adv: 16,
        sync_threshold: 0.5,
        guard_threshold: 0.3,
        search_taps: None,
        pair_band: None,
    }
}

/// `frames` frames of QPSK after 777 samples of silence: the samples and the data cells of each frame.
fn signal(l: &FrameLayout, frames: usize) -> (Vec<Complex32>, Vec<Vec<Complex32>>) {
    let mut m = FrameModulator::new(l.clone());
    let g = l.known_grid();
    let data = l.data_cells();
    let k = l.ncarriers();
    let mut r = Rng::new(5);
    let mut tx = vec![Complex32::new(0.0, 0.0); 777];
    let mut sent = Vec::new();
    for _ in 0..frames {
        let mut cells: Vec<Complex32> = g.iter().map(|v| v.unwrap_or_default()).collect();
        let mut f = Vec::new();
        for &(s, c) in &data {
            let v = Complex32::new(
                if r.next_u64() & 1 == 1 { 0.707 } else { -0.707 },
                if r.next_u64() & 1 == 1 { 0.707 } else { -0.707 },
            );
            cells[s * k + c] = v;
            f.push(v);
        }
        sent.push(f);
        m.frame(&cells, &mut tx);
    }
    (tx, sent)
}

fn demodulate(l: &FrameLayout, x: &[Complex32], piece: usize) -> Vec<Arc<CellFrame>> {
    let mut d = FrameDemodulator::new(l.clone());
    let mut got = Vec::new();
    for c in x.chunks(piece) {
        d.push(c);
        while let Some(f) = d.next_frame() {
            got.push(f);
        }
    }
    got
}

/// The QPSK symbol errors of a received frame against the sent frame it matches best.
fn errors(l: &FrameLayout, f: &CellFrame, sent: &[Vec<Complex32>]) -> usize {
    let k = l.ncarriers();
    let cells: Vec<Complex32> = l
        .data_cells()
        .iter()
        .map(|&(s, c)| f.z[s * k + c])
        .collect();
    sent.iter()
        .map(|sf| {
            sf.iter()
                .zip(&cells)
                .filter(|(a, b)| a.re.signum() != b.re.signum() || a.im.signum() != b.im.signum())
                .count()
        })
        .min()
        .unwrap_or(usize::MAX)
}

#[test]
fn frames_round_trip_with_offset() {
    let l = layout();
    let (mut tx, sent) = signal(&l, 8);
    // carrier offset of 3.3 carriers
    let fo = 3.3 * l.spacing();
    impair(&mut tx, fo, l.rate, 0.0, 1);
    let got = demodulate(&l, &tx, 333);
    assert!(got.len() >= 4, "{}", got.len());
    assert!((got[1].cfo_hz - fo).abs() < 2.0, "{}", got[1].cfo_hz);
    for f in &got[1..] {
        assert_eq!(errors(&l, f, &sent), 0, "frame {}", f.index);
    }
    // The frames do not depend on how the input is split.
    let again = demodulate(&l, &tx, 1000);
    assert_eq!(again.len(), got.len());
    for (a, b) in got.iter().zip(&again) {
        assert_eq!(a.start, b.start);
        assert_eq!(a.z, b.z);
    }
}

#[test]
fn a_sample_clock_offset_is_followed() {
    let l = layout();
    let (tx, sent) = signal(&l, 40);
    for ppm in [-300.0, 300.0] {
        let mut c = ClockError::new(ppm).unwrap();
        let mut x = Vec::new();
        c.process(&tx, &mut x);
        impair(&mut x, 0.6 * l.spacing(), l.rate, 0.0, 2);
        let got = demodulate(&l, &x, 500);
        assert!(got.len() >= 36, "{ppm}: {}", got.len());
        for f in &got[1..] {
            assert_eq!(errors(&l, f, &sent), 0, "{ppm}: frame {}", f.index);
        }
        let last = got.last().unwrap();
        assert!(
            (last.ppm - ppm).abs() < 0.2 * ppm.abs(),
            "{ppm}: {}",
            last.ppm
        );
    }
}

#[test]
fn frames_survive_noise_and_the_snr_is_measured() {
    let l = layout();
    let (mut tx, sent) = signal(&l, 12);
    // The signal power per sample is about 1 / nfft of the cell power: an SNR of 12 dB per cell.
    let p = tx[777..].iter().map(|v| v.norm_sqr() as f64).sum::<f64>() / (tx.len() - 777) as f64;
    let snr_db = 12.0;
    impair(
        &mut tx,
        -1.7 * l.spacing(),
        l.rate,
        p / 10f64.powf(snr_db / 10.0),
        3,
    );
    // The SNR of a pilot cell: the FFT gathers the power of the 100 active carriers into each, and pilots are 1.4 high.
    let active = (l.ncarriers() - l.null.len()) as f64;
    let pilot_db = snr_db + 10.0 * (l.nfft as f64 / active * 1.4 * 1.4).log10();
    let got = demodulate(&l, &tx, 400);
    assert!(got.len() >= 9, "{}", got.len());
    let data = l.data_cells().len();
    for f in &got[1..] {
        // QPSK at 12 dB: a symbol error rate near 1e-4, a few in a frame at most.
        let e = errors(&l, f, &sent);
        assert!(e * 200 < data, "frame {}: {e} of {data}", f.index);
        assert!(
            (f.snr_db as f64 - pilot_db).abs() < 2.5,
            "frame {}: {} against {pilot_db}",
            f.index,
            f.snr_db
        );
    }
}

/// A layout the shared OFDM layout refuses returns an error from `try_new`, and a demodulator from `new` that never locks.
#[test]
fn a_refused_layout_never_locks() {
    let mut l = layout();
    l.cp = l.nfft;
    assert!(l.ofdm().is_err());
    assert!(FrameDemodulator::try_new(l.clone()).is_err());
    let (x, _) = signal(&layout(), 4);
    let mut d = FrameDemodulator::new(l);
    d.push(&x);
    assert!(d.next_frame().is_none());
    let mut l = layout();
    l.adv = l.cp + 1;
    assert!(FrameDemodulator::try_new(l).is_err());
}
