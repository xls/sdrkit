//! The modulator and the receiver at sample rates that are not a whole number of samples per chip. A frame made at one
//! rate goes through a resampler of this file to the rate of the receiver, with a carrier offset and weak noise.

use crate::common::{add_noise, AMP};
use sdrkit_core::Complex32;
use sdrkit_proto_lora_phy::{ChannelRx, DetectorConfig, LoraFrame, LoraParams, TxFrame};
use std::f64::consts::{PI, TAU};

/// The lowest rate of an AD936x converter without its FIR.
const CONVERTER_MIN: f64 = 25e6 / 12.0;
/// The carrier offset of the frames, Hz.
const CFO: f64 = -4500.0;
/// The SNR of the noise, dB in the LoRa bandwidth.
const SNR: f64 = 30.0;

/// The setting of the frames: SF11, 250 kHz, CR 4/5, a preamble of 16.
fn setting() -> LoraParams {
    LoraParams::new(11, 250e3, 5, 16, 0x2B)
}

/// The modified Bessel function of the first kind, order 0.
fn bessel_i0(x: f64) -> f64 {
    let (mut sum, mut term, mut k) = (1.0, 1.0, 1.0);
    while term > 1e-14 * sum {
        term *= (x / (2.0 * k)).powi(2);
        sum += term;
        k += 1.0;
    }
    sum
}

/// Resamples `x` by the exact ratio `l / m`: output sample `k` is the input at time `k m / l`, from a Kaiser-windowed sinc
/// of 96 input samples (beta 10, about 100 dB). The cut-off is 0.3 of the lower rate. Output sample 0 is input sample 0. It
/// shares no code with the receiver.
fn resample(x: &[Complex32], l: usize, m: usize) -> Vec<Complex32> {
    const HALF: isize = 48;
    let beta = 10.0;
    let cut = 0.6 * (l as f64 / m as f64).min(1.0);
    let rows: Vec<Vec<f32>> = (0..l)
        .map(|p| {
            let frac = p as f64 / l as f64;
            (1 - HALF..=HALF)
                .map(|j| {
                    let t = j as f64 - frac;
                    let w = (1.0 - (t / HALF as f64).powi(2)).max(0.0).sqrt();
                    let sinc = if t.abs() < 1e-12 {
                        1.0
                    } else {
                        (PI * cut * t).sin() / (PI * cut * t)
                    };
                    (cut * sinc * bessel_i0(beta * w) / bessel_i0(beta)) as f32
                })
                .collect()
        })
        .collect();
    let n_out = x.len() * l / m;
    (0..n_out)
        .map(|k| {
            let pos = k * m;
            let (i, p) = ((pos / l) as isize, pos % l);
            let mut acc = Complex32::default();
            for (h, j) in rows[p].iter().zip(1 - HALF..=HALF) {
                if let Some(v) = usize::try_from(i + j).ok().and_then(|q| x.get(q)) {
                    acc += *v * *h;
                }
            }
            acc
        })
        .collect()
}

/// Shifts `x` at `rate` by `hz`.
fn shift(x: &mut [Complex32], rate: f64, hz: f64) {
    for (i, v) in x.iter_mut().enumerate() {
        let (s, c) = (TAU * (hz * i as f64 / rate).fract()).sin_cos();
        *v *= Complex32::new(c as f32, s as f32);
    }
}

/// A payload of `len` bytes from `seed`.
fn payload(len: usize, seed: u64) -> Vec<u8> {
    let mut s = seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1;
    (0..len)
        .map(|_| {
            s ^= s << 13;
            s ^= s >> 7;
            s ^= s << 17;
            (s >> 32) as u8
        })
        .collect()
}

/// Makes the frame of `data` at `made`, takes its samples as samples at `played`, resamples them by `l / m` to
/// `played l / m`, adds the carrier offset and the noise, and returns the frames of a receiver at that rate.
fn through(data: &[u8], made: f64, played: f64, l: usize, m: usize, seed: u64) -> Vec<LoraFrame> {
    let p = setting();
    let f = TxFrame::new(&p, data).unwrap().at(0.02).amplitude(AMP);
    let x = f.render(made, 0.05);
    let rate = played * l as f64 / m as f64;
    let mut y = if l == m { x } else { resample(&x, l, m) };
    shift(&mut y, rate, CFO);
    add_noise(&mut y, SNR, p.bw, rate, seed);
    let mut cfg = DetectorConfig::new(p.sf, p.bw, p.sync);
    cfg.preamble = p.preamble;
    let mut rx = ChannelRx::new(rate, 0.0, p.bw, vec![cfg]).unwrap();
    let mut out = Vec::new();
    for c in y.chunks(65_536) {
        rx.process(c, &mut out);
    }
    out.into_iter().map(|(f, _)| f).collect()
}

/// Runs `count` frames of `len` bytes through [`through`]. Returns the number with a good CRC and the right payload, and
/// the mean SNR of the frames found.
fn score(count: u64, len: usize, made: f64, played: f64, l: usize, m: usize) -> (u64, f64) {
    let (mut good, mut snr, mut found) = (0, 0.0, 0);
    for seed in 1..=count {
        let data = payload(len, seed);
        for f in through(&data, made, played, l, m, seed) {
            found += 1;
            snr += f.snr_db;
            good += u64::from(f.crc_ok && f.payload == data);
        }
    }
    (good, snr / f64::from(found.max(1)))
}

/// The lowest rate that the iiod driver sets on an AD936x without its FIR.
const DRIVER_MIN: f64 = 2.1e6;
/// The length of the payloads, bytes.
const LEN: usize = 86;

/// Asserts that `count` frames through [`through`] all have a good CRC, and that their mean SNR is within 1 dB of the SNR
/// of frames made at the rate of the receiver.
fn assert_as_direct(count: u64, made: f64, l: usize, m: usize) {
    let rate = made * l as f64 / m as f64;
    let (_, direct) = score(count.min(3), LEN, rate, rate, 1, 1);
    let (good, snr) = score(count, LEN, made, made, l, m);
    assert_eq!(good, count, "frames made at {made} and read at {rate}");
    assert!(
        (snr - direct).abs() < 1.0,
        "SNR {snr} dB made at {made}, {direct} dB made at {rate}"
    );
}

#[test]
fn a_frame_made_at_the_lowest_converter_rate_is_read_at_two_msps() {
    // 8.33 samples per chip.
    assert_as_direct(20, CONVERTER_MIN, 24, 25);
}

#[test]
fn a_frame_made_at_the_lowest_driver_rate_is_read_at_two_msps() {
    // 8.4 samples per chip.
    assert_as_direct(6, DRIVER_MIN, 20, 21);
}

#[test]
fn a_frame_made_at_a_whole_number_of_samples_per_chip_is_read_at_two_msps() {
    assert_as_direct(3, 1e6, 2, 1);
}

#[test]
fn a_frame_made_at_a_higher_rate_is_read_at_two_msps() {
    // 9.6 samples per chip.
    assert_as_direct(3, 2.4e6, 5, 6);
}

#[test]
fn a_frame_made_at_two_msps_is_read_at_the_converter_and_driver_rates() {
    assert_as_direct(6, 2e6, 25, 24);
    assert_as_direct(3, 2e6, 21, 20);
}

/// Correlates the symbol of value `s` that starts at chip `start` of `x` (a frame from time 0 at `rate`) with the chirp of
/// value `s + d`, from the definition of the signal: a frequency that rises by the bandwidth over `n` chips and wraps
/// from the top of the band to the bottom. Returns the part of the symbol's energy in the correlation, 0 to 1.
fn chirp_match(x: &[Complex32], rate: f64, bw: f64, n: f64, start: f64, s: f64, d: f64) -> f64 {
    let first = (start / bw * rate).ceil() as usize;
    let last = ((start + n) / bw * rate).ceil() as usize;
    let (mut re, mut im, mut energy) = (0.0f64, 0.0f64, 0.0f64);
    for (i, v) in x.iter().enumerate().take(last).skip(first) {
        let c = i as f64 * bw / rate - start;
        let k = s + d;
        // The phase in cycles: the integral of (k + c) / n - 1/2, less one cycle a chip after the wrap.
        let wrap = if c >= n - k { c - (n - k) } else { 0.0 };
        let (sin, cos) = (TAU * (k * c / n + 0.5 * c * c / n - 0.5 * c - wrap).fract()).sin_cos();
        let (a, b) = (f64::from(v.re), f64::from(v.im));
        re += a * cos + b * sin;
        im += b * cos - a * sin;
        energy += a * a + b * b;
    }
    (re * re + im * im) / (energy * (last - first) as f64)
}

/// Returns, over the preamble and the data symbols of a frame made at `rate`, the lowest part of a symbol's energy that
/// its own chirp holds, and the largest distance of the best chirp from the symbol's value, in chips.
fn worst_symbol(rate: f64) -> (f64, f64) {
    let p = setting();
    let f = TxFrame::new(&p, &payload(LEN, 1)).unwrap();
    let x = f.render(rate, 0.0);
    let n = p.n() as f64;
    let pre = (0..p.preamble as usize).map(|j| (j as f64 * n, 0.0));
    let data = f
        .symbols
        .iter()
        .enumerate()
        .map(|(j, s)| ((p.header_symbols() + j as f64) * n, f64::from(*s)));
    let (mut least, mut far) = (1.0f64, 0.0f64);
    for (start, s) in pre.chain(data) {
        least = least.min(chirp_match(&x, rate, p.bw, n, start, s, 0.0));
        let best = (-10..=10)
            .map(|q| f64::from(q) * 0.05)
            .max_by(|a, b| {
                chirp_match(&x, rate, p.bw, n, start, s, *a)
                    .total_cmp(&chirp_match(&x, rate, p.bw, n, start, s, *b))
            })
            .unwrap();
        far = far.max(best.abs());
    }
    (least, far)
}

#[test]
fn every_symbol_is_its_chirp_at_any_rate() {
    for rate in [2e6, CONVERTER_MIN, DRIVER_MIN, 1e6, 2.4e6] {
        let (least, far) = worst_symbol(rate);
        // The rest of the energy is 40 dB or more below the chirp, and the best chirp is that of the symbol, from the first
        // symbol to the last.
        assert!(least > 0.9999, "{least} at {rate}");
        assert!(far < 0.01, "{far} chips at {rate}");
    }
}
