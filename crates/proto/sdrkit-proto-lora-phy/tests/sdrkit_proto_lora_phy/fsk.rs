//! The FSK modulation: frames through offsets, clock errors and noise into the receiver.

use sdrkit_algo_sim::clock::ClockError;
use sdrkit_algo_sim::noise::Awgn;
use sdrkit_core::Complex32;
use sdrkit_proto_lora_phy::fsk::{self, FskFrame, FskRx};

fn run(
    rate: f64,
    offset: f64,
    payload: &[u8],
    pre: u8,
    shaped: bool,
    ppm: f64,
    noise: f64,
) -> Vec<FskFrame> {
    let mut bytes = Vec::new();
    fsk::encode(payload, pre, 5, &mut bytes).unwrap();
    let sig = fsk::modulate(&bytes, rate, shaped).unwrap();
    let lead = (0.002 * rate) as usize;
    let mut x = vec![Complex32::default(); lead + sig.len() + lead];
    let step = std::f64::consts::TAU * offset / rate;
    for (i, s) in sig.iter().enumerate() {
        let ph = step * i as f64;
        x[lead + i] = s * Complex32::new(ph.cos() as f32, ph.sin() as f32) * 0.5;
    }
    let x = if ppm != 0.0 {
        let mut c = ClockError::new(ppm).unwrap();
        let mut y = Vec::new();
        c.process(&x, &mut y);
        y
    } else {
        x
    };
    let mut x = x;
    Awgn::with_power(noise, 5).unwrap().process(&mut x);
    let mut rx = FskRx::new(rate, 0.0).unwrap();
    let mut out = Vec::new();
    for c in x.chunks(7777) {
        rx.process(c, &mut out);
    }
    rx.process(
        &vec![Complex32::default(); (0.12 * rate) as usize],
        &mut out,
    );
    out
}

#[test]
fn frames_come_back_at_many_rates_and_offsets() {
    for &(rate, off, pre, shaped, ppm) in &[
        (500e3, 0.0, 0x55, true, 0.0),
        (1e6, 20e3, 0xAA, false, 50.0),
        (2e6, -20e3, 0x55, true, -50.0),
        (8e6, 5e3, 0xAA, true, 0.0),
    ] {
        let payload: Vec<u8> = (0..40).map(|i| (i * 7) as u8).collect();
        let f = run(rate, off, &payload, pre, shaped, ppm, 0.0004);
        assert_eq!(f.len(), 1, "rate {rate} off {off}");
        assert_eq!(f[0].payload, payload);
        assert!((f[0].offset_hz - off).abs() < 2e3, "{}", f[0].offset_hz);
    }
}

#[test]
fn every_length_round_trips_and_noise_returns_nothing() {
    for len in [1usize, 2, 17, 100, 255] {
        let payload: Vec<u8> = (0..len).map(|i| (i * 13 + 1) as u8).collect();
        let f = run(1e6, 0.0, &payload, 0x55, true, 0.0, 0.0);
        assert_eq!(f.len(), 1, "len {len}");
        assert_eq!(f[0].payload, payload);
    }
    let mut rx = FskRx::new(1e6, 0.0).unwrap();
    let mut x = vec![Complex32::default(); 1_000_000];
    Awgn::with_power(0.02, 9).unwrap().process(&mut x);
    let mut out = Vec::new();
    rx.process(&x, &mut out);
    assert!(out.is_empty());
    assert!(fsk::encode(&[0; 256], 0x55, 5, &mut Vec::new()).is_err());
    assert!(fsk::modulate(b"x", 50e3, true).is_err());
    assert!(FskRx::new(100e3, 0.0).is_err());
}
