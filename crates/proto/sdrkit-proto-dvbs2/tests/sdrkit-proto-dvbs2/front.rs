//! The survey of the front end on DVB-S carriers: symbol rate and carrier offset.

use sdrkit_algo_sim::noise::{Awgn, NoiseLevel};
use sdrkit_core::Complex32;
use sdrkit_proto_dvbs2::front::{Acquisition, Front, FrontConfig};
use sdrkit_proto_dvbs2::inner::PunctureRate;
use sdrkit_proto_dvbs2::s1encode::S1Encoder;
use sdrkit_proto_dvbs2::transmit::TxShaper;

/// A null transport packet.
fn null_packet() -> [u8; 188] {
    let mut p = [0xFFu8; 188];
    p[..4].copy_from_slice(&[0x47, 0x1F, 0xFF, 0x10]);
    p
}

/// `seconds` of a DVB-S carrier of null packets at `rs` symbols per second, sampled at `fs`.
fn null_carrier(fs: f64, rs: f64, rate: PunctureRate, seconds: f64) -> Vec<Complex32> {
    let mut enc = S1Encoder::new(rate);
    let mut sh = TxShaper::new(fs, rs, 0.35).unwrap();
    let n = (seconds * fs) as usize;
    let mut syms = Vec::new();
    while sh.ready() < n {
        enc.packet(&null_packet(), &mut syms);
        sh.push(&syms);
    }
    let mut x = vec![Complex32::default(); n];
    sh.pop(&mut x);
    x
}

/// `x` with noise at `esn0_db` for a carrier of `rs` symbols per second, or as it is.
fn noisy(mut x: Vec<Complex32>, fs: f64, rs: f64, esn0_db: Option<f64>) -> Vec<Complex32> {
    if let Some(e) = esn0_db {
        let p = x.iter().map(|z| f64::from(z.norm_sqr())).sum::<f64>() / x.len() as f64;
        let level = NoiseLevel::EsN0Db {
            esn0_db: e,
            symbol_rate_hz: rs,
        };
        Awgn::complex(level, p, fs, 3).unwrap().process(&mut x);
    }
    x
}

/// The acquisition of a front end with the symbol rate `manual_rs` set (0: automatic) on `x` at `fs`.
fn acquire(x: &[Complex32], fs: f64, manual_rs: f64) -> Acquisition {
    let cfg = FrontConfig {
        manual_rs,
        ..FrontConfig::default()
    };
    let mut f = Front::new(fs, cfg).unwrap();
    let mut out = Vec::new();
    for c in x.chunks(16384) {
        out.clear();
        f.process(c, &mut out);
    }
    f.acq
        .unwrap_or_else(|| panic!("no carrier at {fs} samples/s: {:?}", f.log))
}

/// The noise levels of the tests: none, and Es/N0 6 dB.
const LEVELS: [Option<f64>; 2] = [None, Some(6.0)];

/// A carrier of null packets repeats every 8 packets, so its squared envelope has a line at every multiple of that
/// repetition rate. The survey takes the line of the symbol rate, also at 2 samples per symbol, where that line is at half
/// the sample rate.
#[test]
fn null_packets_near_two_samples_per_symbol_give_the_symbol_rate() {
    let cases = [
        (2e6, 1e6, &PunctureRate::ALL[..]),
        (4e6, 2e6, &[PunctureRate::R34][..]),
        (2.3e6, 1e6, &[PunctureRate::R12, PunctureRate::R78][..]),
        (2e6, 0.9e6, &[PunctureRate::R34][..]),
    ];
    for (fs, rs, rates) in cases {
        for &r in rates {
            for es in LEVELS {
                let x = noisy(null_carrier(fs, rs, r, 0.45), fs, rs, es);
                let a = acquire(&x, fs, 0.0);
                let what = format!("{rs} sym/s at {fs} samples/s, {r:?}, Es/N0 {es:?}: {a:?}");
                assert!(a.cyclic_hz.is_some(), "{what}");
                assert!((a.rate_hz / rs - 1.0).abs() < 2e-5, "{what}");
                assert!(a.carrier.offset_hz.abs() < 0.02 * rs, "{what}");
            }
        }
    }
}

/// With the symbol rate set, the survey places the carrier within 5 % of the symbol rate or 2 points of its spectrum. At
/// the start of a transmission the interleaver of the sender is still empty and the spectrum has strong lines; no part of
/// that spectrum is taken for the carrier.
#[test]
fn a_set_symbol_rate_places_the_carrier() {
    for (fs, rs, seconds, rates) in [
        (2e6, 125e3, 0.45, &PunctureRate::ALL[..]),
        (2e6, 25e3, 0.9, &[PunctureRate::R12, PunctureRate::R34][..]),
        (4e6, 25e3, 1.5, &[PunctureRate::R34][..]),
        (50e3, 25e3, 4.0, &[PunctureRate::R34][..]),
    ] {
        for &r in rates {
            for es in LEVELS {
                let x = noisy(null_carrier(fs, rs, r, seconds), fs, rs, es);
                let a = acquire(&x, fs, rs);
                let what = format!("{rs} sym/s at {fs} samples/s, {r:?}, Es/N0 {es:?}: {a:?}");
                assert_eq!(a.rate_hz, rs, "{what}");
                let limit = (0.05 * rs).max(2.0 * fs / 4096.0);
                assert!(a.carrier.offset_hz.abs() < limit, "{what}");
            }
        }
    }
}

/// The first survey of a transmission at 125 kS/s sees the lines of the empty interleaver and refuses the carrier. What
/// the blanking of that carrier leaves of its skirts is not listed as a carrier.
#[test]
fn the_rest_of_a_refused_carrier_is_no_carrier() {
    let (fs, rs) = (2e6, 125e3);
    let x = null_carrier(fs, rs, PunctureRate::R34, 0.2);
    let mut f = Front::new(fs, FrontConfig::default()).unwrap();
    let mut out = Vec::new();
    // Up to the first survey: 48 spectra, 4 ms apart.
    for c in x.chunks(16384) {
        if f.spectrum.is_empty() {
            f.process(c, &mut out);
        }
    }
    assert!(!f.spectrum.is_empty(), "the survey ran");
    assert!(
        f.carriers.iter().all(|c| c.rate_hz > 0.7 * rs),
        "{:?}",
        f.carriers
    );
}

/// With the symbol rate set, the survey takes the carrier of that width, not a narrower one nearer to the centre.
#[test]
fn a_set_symbol_rate_chooses_the_carrier_of_that_width() {
    let fs = 2e6;
    let shift = |x: Vec<Complex32>, hz: f64| -> Vec<Complex32> {
        let w = 2.0 * std::f64::consts::PI * hz / fs;
        x.iter()
            .enumerate()
            .map(|(k, z)| {
                let ph = w * k as f64;
                z * Complex32::new(ph.cos() as f32, ph.sin() as f32)
            })
            .collect()
    };
    // Equal power densities: the narrow carrier has a quarter of the power of the wide one.
    let narrow = shift(null_carrier(fs, 125e3, PunctureRate::R34, 0.9), -200e3);
    let wide = shift(null_carrier(fs, 500e3, PunctureRate::R34, 0.9), 400e3);
    let x: Vec<Complex32> = narrow.iter().zip(&wide).map(|(a, b)| a * 0.5 + b).collect();
    let a = acquire(&x, fs, 500e3);
    assert!((a.carrier.offset_hz - 400e3).abs() < 25e3, "{a:?}");
    let a = acquire(&x, fs, 125e3);
    assert!((a.carrier.offset_hz + 200e3).abs() < 6e3, "{a:?}");
}
