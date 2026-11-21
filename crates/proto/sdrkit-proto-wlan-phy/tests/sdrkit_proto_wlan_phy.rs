//! The one test program of `sdrkit-proto-wlan-phy`. Each module tests one part of the crate against known answers from its standard.

#[path = "sdrkit_proto_wlan_phy/alloc.rs"]
mod alloc;
#[path = "sdrkit_proto_wlan_phy/ble.rs"]
mod ble;
#[path = "sdrkit_proto_wlan_phy/dsss.rs"]
mod dsss;
#[path = "sdrkit_proto_wlan_phy/mac.rs"]
mod mac;
#[path = "sdrkit_proto_wlan_phy/ofdm.rs"]
mod ofdm;
#[path = "sdrkit_proto_wlan_phy/s1g.rs"]
mod s1g;

use sdrkit_proto_wlan_phy::Complex32;

/// Rotates `x` by a carrier offset of `hz` at `rate`, adds complex noise of power `noise`, and puts `pad` zeros before and after.
pub fn impair(
    x: &[Complex32],
    rate: f64,
    hz: f64,
    noise: f64,
    pad: usize,
    seed: u64,
) -> Vec<Complex32> {
    let mut v = vec![Complex32::default(); pad];
    for (n, &s) in x.iter().enumerate() {
        let ph = std::f64::consts::TAU * hz * n as f64 / rate;
        v.push(s * Complex32::new(ph.cos() as f32, ph.sin() as f32));
    }
    v.extend(std::iter::repeat_n(Complex32::default(), pad));
    if noise > 0.0 {
        sdrkit_algo_sim::noise::Awgn::with_power(noise, seed)
            .unwrap()
            .process(&mut v);
    }
    v
}

/// Cuts `x` into pieces of pseudo-random sizes from 1 to `max`.
pub fn cuts(x: &[Complex32], max: usize, seed: u64) -> Vec<&[Complex32]> {
    let mut rng = sdrkit_algo_sim::rng::Rng::new(seed);
    let mut out = Vec::new();
    let mut i = 0;
    while i < x.len() {
        let n = 1 + (rng.next_u64() as usize % max);
        let e = (i + n).min(x.len());
        out.push(&x[i..e]);
        i = e;
    }
    out
}

/// `x` times `gain`.
pub fn scaled(x: &[Complex32], gain: f32) -> Vec<Complex32> {
    x.iter().map(|&s| s * gain).collect()
}

/// The mean power of `x`, dB.
pub fn power_db(x: &[Complex32]) -> f64 {
    let p = x.iter().map(|s| s.norm_sqr() as f64).sum::<f64>() / x.len() as f64;
    10.0 * p.log10()
}

#[test]
fn every_benchmark_runs() {
    for b in sdrkit_proto_wlan_phy::benchmarks() {
        let r = (b.run)(0.0);
        assert!(r.items_per_second > 0.0, "{}", b.name);
    }
}
