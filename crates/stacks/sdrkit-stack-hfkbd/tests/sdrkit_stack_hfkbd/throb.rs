//! `throb`: every variant at several centres, the shifted characters, the pass marks in noise, nothing from noise, a carrier or
//! an FSK441-like signal, and a long text streamed through the receiver.

use crate::common::*;
use sdrkit_core::Complex32;
use sdrkit_stack_hfkbd::decoder::{Modulator, GEN_CENTRE_HZ, GEN_MESSAGE};
use sdrkit_stack_hfkbd::throb::{
    self, decode, encode, ThrobModulator, VariantChoice, GEN_VARIANT, VARIANTS,
};

/// `text` sent once in `v` at `centre`, 8 kHz complex, padded with silence to `seconds`, with noise of `snr` dB in 2500 Hz
/// against the mean power of a pair of tones (0.5).
fn signal(v: usize, text: &str, centre: f64, seconds: f64, snr: f64, seed: u64) -> Vec<Complex32> {
    let var = VARIANTS[v];
    let n_sym = encode(&var, text).len();
    let n = (n_sym as f64 * var.symbol_s * 8000.0) as usize;
    let mut m = ThrobModulator::new(var, 8000.0, centre, text);
    let mut x = vec![Complex32::default(); n.max((seconds * 8000.0) as usize)];
    m.fill(&mut x[..n]);
    let level = sdrkit_algo_sim::noise::NoiseLevel::SnrDb {
        snr_db: snr,
        bandwidth_hz: 2500.0,
    };
    sdrkit_algo_sim::noise::Awgn::complex(level, 0.5, 8000.0, seed)
        .unwrap()
        .process(&mut x);
    x
}

fn bare(s: &str) -> String {
    s.chars().filter(|c| !c.is_whitespace()).collect()
}

#[test]
fn every_variant_at_each_centre() {
    for (v, var) in VARIANTS.iter().enumerate() {
        for centre in [900.0, 1500.0, 2700.0] {
            let x = signal(v, "CQ CQ DE W1AW 73", centre, 16.0, 30.0, v as u64);
            let r = decode(&x, VariantChoice::Auto)
                .unwrap_or_else(|| panic!("{} at {centre}: nothing", var.name));
            assert_eq!(r.variant.name, var.name, "at {centre}");
            assert!(
                bare(&r.text).contains("CQCQDEW1AW73"),
                "{} {centre}: {}",
                r.variant.name,
                r.text
            );
            assert!(
                (r.centre_hz - centre).abs() <= 3.0,
                "{} {centre}: {}",
                r.variant.name,
                r.centre_hz
            );
        }
    }
}

#[test]
fn shifted_characters() {
    let x = signal(1, "TEST? A@B=C", 1500.0, 16.0, 30.0, 9);
    let r = decode(&x, VariantChoice::Auto).expect("a reading");
    assert_eq!(bare(&r.text), "TEST?A@B=C");
}

fn passes(v: usize, snr: f64) -> usize {
    (0..4)
        .filter(|&seed| {
            let x = signal(v, "CQ CQ DE W1AW 73", 1400.0, 16.0, snr, 20 + seed);
            decode(&x, VariantChoice::Auto).is_some_and(|r| {
                r.variant.name == VARIANTS[v].name && bare(&r.text).contains("W1AW")
            })
        })
        .count()
}

#[test]
fn pass_marks_in_noise() {
    for v in [0, 2, 3] {
        assert_eq!(passes(v, 10.0), 4, "{}", VARIANTS[v].name);
        for snr in [0.0, -6.0, -12.0] {
            eprintln!("{} at {snr} dB: {} of 4", VARIANTS[v].name, passes(v, snr));
        }
    }
}

#[test]
fn noise_a_carrier_and_other_tones_give_nothing() {
    for seed in 0..10 {
        let x = noise_8k(20.0, seed);
        assert!(decode(&x, VariantChoice::Auto).is_none(), "{seed}");
    }
    let mut c = noise_8k(20.0, 30);
    for (i, s) in c.iter_mut().enumerate() {
        let p = std::f64::consts::TAU * 1500.0 * i as f64 / 8000.0;
        *s += Complex32::new(p.cos() as f32, p.sin() as f32);
    }
    assert!(decode(&c, VariantChoice::Auto).is_none());
    // Four tones 441 Hz apart keyed at 441 baud, as FSK441 sends.
    let mut f = noise_8k(20.0, 31);
    let mut ph = 0.0f64;
    for (i, s) in f.iter_mut().enumerate() {
        let k = (i * 441 / 8000 * 7 + i / 300) % 4;
        ph += std::f64::consts::TAU * (882.0 + 441.0 * k as f64) / 8000.0;
        *s += Complex32::new(ph.cos() as f32, ph.sin() as f32);
    }
    assert!(decode(&f, VariantChoice::Auto).is_none());
}

/// Noise of unit power in 2500 Hz at 8 kHz.
fn noise_8k(seconds: f64, seed: u64) -> Vec<Complex32> {
    let mut x = vec![Complex32::default(); (seconds * 8000.0) as usize];
    sdrkit_algo_sim::noise::Awgn::with_power(8000.0 / 2500.0, seed)
        .unwrap()
        .process(&mut x);
    x
}

#[test]
fn a_long_text_streams_once_and_in_order() {
    let h = snr_2500(
        harness(&sdrkit_stack_hfkbd::THROB)
            .seconds(60.0)
            .set(&GEN_VARIANT, throb::GenVariant::Throb4)
            .set(
                &GEN_MESSAGE,
                "THE QUICK BROWN FOX JUMPS OVER THE LAZY DOG 0123456789 ",
            )
            .set(&GEN_CENTRE_HZ, 1500.0),
        6.0,
    );
    let r = run(&h);
    let t = bare(&text(&r));
    let want = "THEQUICKBROWNFOXJUMPSOVERTHELAZYDOG";
    assert!(t.contains(want), "{t}");
    assert_eq!(field_text(&r.results, "variant"), "Throb4");
}

#[test]
fn the_test_signal_reads_its_text() {
    let r = run(&harness(&sdrkit_stack_hfkbd::THROB).seconds(40.0));
    assert!(bare(&text(&r)).contains("CQCQDEK0TSTK"), "{}", text(&r));
}
