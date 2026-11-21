//! Signals for the tests.

use sdrkit_algo_sim::modulate::Ook;
use sdrkit_algo_sim::noise::Awgn;
use sdrkit_core::Complex32;
use sdrkit_proto_ookdev::{Modulation, Train};
use std::f64::consts::TAU;

/// The (mark, space) pairs of a train in µs.
pub fn pairs_us(t: &Train) -> Vec<(f32, f32)> {
    t.pulses
        .iter()
        .map(|p| (t.us(p.mark), t.us(p.space)))
        .collect()
}

/// An OOK burst of `pairs` (µs) at amplitude `amp` with 1.5 µs edges, `lead_us` of silence before and after, over complex noise
/// of power `noise`.
pub fn ook_iq(
    pairs: &[(f32, f32)],
    rate: f64,
    amp: f32,
    lead_us: f64,
    noise: f64,
    seed: u64,
) -> Vec<Complex32> {
    let mut k = Ook::new(rate, 1.5e-6).unwrap();
    let mut env = Vec::new();
    k.push(false, lead_us * 1e-6, &mut env).unwrap();
    for (i, &(m, s)) in pairs.iter().enumerate() {
        k.push(true, m as f64 * 1e-6, &mut env).unwrap();
        let s = if i + 1 == pairs.len() {
            lead_us as f32
        } else {
            s
        };
        k.push(false, s as f64 * 1e-6, &mut env).unwrap();
    }
    k.flush(&mut env);
    let mut x: Vec<Complex32> = env.iter().map(|&a| Complex32::new(a * amp, 0.0)).collect();
    Awgn::with_power(noise, seed).unwrap().process(&mut x);
    x
}

/// An FSK carrier: `lead_us` of silence, then the train's marks on the upper tone and spaces on the lower tone (`offset_hz` +-
/// `dev_hz`), the last space included, then silence, over complex noise of power `noise`.
pub fn fsk_iq(
    pairs: &[(f32, f32)],
    rate: f64,
    offset_hz: f64,
    dev_hz: f64,
    lead_us: f64,
    noise: f64,
    seed: u64,
) -> Vec<Complex32> {
    let lead = (lead_us * 1e-6 * rate) as usize;
    let mut x = vec![Complex32::default(); lead];
    let mut ph = 0f64;
    let mut t = 0f64;
    let mut emitted = 0usize;
    for &(m, s) in pairs {
        for (len, f) in [(m, offset_hz + dev_hz), (s, offset_hz - dev_hz)] {
            t += len as f64 * 1e-6 * rate;
            while (emitted as f64) < t {
                ph += TAU * f / rate;
                x.push(Complex32::from_polar(0.5, ph as f32));
                emitted += 1;
            }
        }
    }
    x.extend(std::iter::repeat_n(Complex32::default(), lead));
    Awgn::with_power(noise, seed).unwrap().process(&mut x);
    x
}

/// Bits of `bytes`, MSB first.
pub fn bits_of(bytes: &[u8]) -> Vec<u8> {
    bytes
        .iter()
        .flat_map(|&b| (0..8).map(move |k| (b >> (7 - k)) & 1))
        .collect()
}

/// A train from µs pairs at `rate`.
pub fn train(m: Modulation, rate: f64, pairs: &[(f32, f32)]) -> Train {
    Train::from_us(m, rate, pairs)
}
