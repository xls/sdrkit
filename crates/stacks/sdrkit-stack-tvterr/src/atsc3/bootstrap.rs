//! The ATSC 3.0 bootstrap: four 500 µs symbols at 6.144 Msps, its generator and its detector (position, carrier offset and the
//! signalling fields).
//!
//! Standard: ATSC A/321.

use sdrkit_algo_dsp::fft::Fft;
use sdrkit_algo_modem::ofdm::LagCorrelator;
use sdrkit_core::Complex32;
use sdrkit_util_bits::lfsr::Lfsr;
use sdrkit_util_bits::seq::{self, ZadoffChu};

/// The bootstrap rate.
pub const RATE: f64 = 6.144e6;
/// FFT size.
pub const N: usize = 2048;
/// Zadoff-Chu length.
pub const NZC: usize = 1499;
/// Samples per symbol (C 520 + A 2048 + B 504).
pub const SYM: usize = 3072;

/// The minimum time to the next frame, ms, of a 5-bit code (A/321 table).
pub fn min_time_ms(code: u8) -> u32 {
    let k = code as u32;
    match k {
        0..=7 => 50 * (k + 1),
        8..=15 => 500 + 100 * (k - 8),
        16..=23 => 1300 + 200 * (k - 16),
        _ => 2900 + 400 * (k - 24),
    }
}

/// The post-bootstrap sample rate of a coefficient N.
pub fn post_rate(n: u8) -> f64 {
    (n as f64 + 16.0) * 0.384e6
}

/// The fields a bootstrap carries.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Fields {
    /// Minor version.
    pub minor: u8,
    /// Emergency alert wake-up.
    pub ea_wake: bool,
    /// Minimum time to the next frame, code 0..31.
    pub min_time: u8,
    /// System bandwidth: 0 6 MHz, 1 7 MHz, 2 8 MHz, 3 more.
    pub bandwidth: u8,
    /// Sample-rate coefficient N.
    pub rate_n: u8,
    /// Preamble structure 0..255 (160 and above reserved).
    pub preamble: u8,
}

/// The frequency-domain sequence (NZC values on carriers −749 .. +749 around 0 Hz, the middle one at 0 Hz set to zero).
pub fn freq_seq(minor: u8) -> Vec<Complex32> {
    let Ok(z) = ZadoffChu::new(137, NZC as u32) else {
        return Vec::new();
    };
    let mut pn = vec![0u8; NZC];
    let mut l: Lfsr = seq::atsc3_bootstrap(0x019D ^ ((minor as u16) << 8));
    l.fill_bits(&mut pn);
    (0..NZC)
        .map(|k| {
            if k == NZC / 2 {
                return Complex32::new(0.0, 0.0);
            }
            let [re, im] = z.value(k as u64);
            let v = Complex32::new(re as f32, im as f32);
            if pn[k] == 1 {
                -v
            } else {
                v
            }
        })
        .collect()
}

/// The time-domain A part of a symbol with cyclic shift `shift`.
pub fn symbol_a(minor: u8, shift: usize) -> Vec<Complex32> {
    let f = freq_seq(minor);
    let mut buf = vec![Complex32::new(0.0, 0.0); N];
    for (k, v) in f.iter().enumerate() {
        let c = k as isize - (NZC / 2) as isize;
        buf[((c + N as isize) % N as isize) as usize] = *v;
    }
    Fft::inverse(N).process(&mut buf);
    let s = 1.0 / (NZC as f32).sqrt();
    let a: Vec<Complex32> = (0..N).map(|i| buf[(i + N - shift % N) % N] * s).collect();
    a
}

fn gray(v: u16) -> u16 {
    v ^ (v >> 1)
}

fn ungray(mut g: u16) -> u16 {
    let mut v = g;
    while g > 0 {
        g >>= 1;
        v ^= g;
    }
    v
}

/// The signalling bits of symbols 1..3: (value, bits).
fn symbol_bits(f: &Fields) -> [(u16, u32); 3] {
    [
        (
            ((f.ea_wake as u16) << 7) | ((f.min_time as u16 & 31) << 2) | (f.bandwidth as u16 & 3),
            8,
        ),
        (f.rate_n as u16 & 0x7F, 7),
        (f.preamble as u16, 8),
    ]
}

fn shift_of(value: u16, bits: u32) -> usize {
    ((gray(value) as usize) << (11 - bits)) % N
}

/// The four bootstrap symbols (4 × 3072 samples at 6.144 Msps, unit power).
pub fn bootstrap(f: &Fields) -> Vec<Complex32> {
    let mut out = Vec::with_capacity(4 * SYM);
    let a0 = symbol_a(f.minor, 0);
    let rot = |n: usize| {
        let ph = std::f64::consts::TAU * n as f64 / N as f64;
        Complex32::new(ph.cos() as f32, ph.sin() as f32)
    };
    out.extend_from_slice(&a0[N - 520..]);
    out.extend_from_slice(&a0);
    for n in 0..504 {
        out.push(a0[N - 504 + n] * rot(n));
    }
    let bits = symbol_bits(f);
    for (k, &(v, nb)) in bits.iter().enumerate() {
        let mut a = symbol_a(f.minor, shift_of(v, nb));
        if k == 2 {
            // The last symbol is inverted: the end of the bootstrap.
            a.iter_mut().for_each(|s| *s = -*s);
        }
        // B-C-A for the later symbols.
        for n in 0..504 {
            out.push(a[N - 504 + n] * rot(n));
        }
        out.extend_from_slice(&a[N - 520..]);
        out.extend_from_slice(&a);
    }
    out
}

/// Finds bootstraps in samples at 6.144 Msps: positions (start of symbol 0's C part), fields and carrier offset (Hz).
pub struct Detector {
    fft: Fft,
    ifft: Fft,
    known: Vec<Complex32>,
    buf: Vec<Complex32>,
    prod: Vec<Complex32>,
    lag: LagCorrelator,
    fed: u64,
    cand: Vec<(u64, f32, Complex32)>,
    accepted_until: u64,
    /// Search a carrier offset of up to this (Hz).
    pub max_offset_hz: f64,
}

/// The window of the lag correlation: the C part repeats the end of A over 520 samples.
const WINDOW: usize = 520;

impl Default for Detector {
    fn default() -> Self {
        Detector::new()
    }
}

impl Detector {
    /// For minor version 0.
    pub fn new() -> Detector {
        let mut r = vec![Complex32::new(0.0, 0.0); N];
        for (k, v) in freq_seq(0).iter().enumerate() {
            let c = k as isize - (NZC / 2) as isize;
            r[((c + N as isize) % N as isize) as usize] = *v;
        }
        Detector {
            fft: Fft::forward(N),
            ifft: Fft::inverse(N),
            known: r,
            buf: vec![Complex32::new(0.0, 0.0); N],
            prod: vec![Complex32::new(0.0, 0.0); N],
            // The lag and the window are constants that the correlator takes.
            lag: LagCorrelator::new(N, WINDOW, 1.0, 1.0).unwrap_or_else(|_| unreachable!()),
            fed: 0,
            cand: Vec::new(),
            accepted_until: 0,
            max_offset_hz: 20e3,
        }
    }

    /// The cyclic shift of the A part at `at`, after removing `cfo_hz`, and the peak's share of the energy.
    fn shift_at(
        &mut self,
        x: &[Complex32],
        at: usize,
        cfo_hz: f64,
        int_bins: isize,
    ) -> (usize, f32) {
        for (i, b) in self.buf.iter_mut().enumerate() {
            let ph = -std::f64::consts::TAU * cfo_hz * (at + i) as f64 / RATE;
            *b = x[at + i] * Complex32::new(ph.cos() as f32, ph.sin() as f32);
        }
        self.fft.process(&mut self.buf);
        for (k, p) in self.prod.iter_mut().enumerate() {
            *p = self.buf[((k as isize + int_bins).rem_euclid(N as isize)) as usize]
                * self.known[k].conj();
        }
        self.ifft.process(&mut self.prod);
        let e: f64 = self.prod.iter().map(|v| v.norm_sqr() as f64).sum();
        let (i, v) = self
            .prod
            .iter()
            .enumerate()
            .map(|(i, v)| (i, v.norm_sqr()))
            .max_by(|a, b| a.1.total_cmp(&b.1))
            .unwrap_or((0, 0.0));
        (i, (v as f64 / e.max(1e-30)) as f32)
    }

    /// Searches `x`, the stream from sample `base` on (counted from the start of the stream), and writes into `out` (position in
    /// `x`, fields, offset Hz) of every bootstrap found. The correlation goes on from the samples of the last call, so `x` must
    /// start at or before the end of what the last call searched.
    pub fn find(&mut self, x: &[Complex32], base: u64, out: &mut Vec<(usize, Fields, f64)>) {
        out.clear();
        if x.len() < 4 * SYM + N {
            return;
        }
        // Lag-2048 correlation over 520 samples (the C part repeats the end of A). The shared correlator keeps its sums exact
        // over any length of input. Its window over x[i + N ..] against x[i ..] is complete once x[i + N + w - 1] is in.
        let w = WINDOW;
        let last = x.len() - 4 * SYM - N;
        let end = base + (last + N + w - 1) as u64;
        self.fed = self.fed.max(base);
        while self.fed < end {
            let j = self.fed;
            self.fed += 1;
            let (v, p) = self.lag.push(x[(j - base) as usize]);
            let Some(i) = (j + 1).checked_sub((N + w) as u64) else {
                continue;
            };
            // The window's correlation runs newer against older: the conjugate is older against newer.
            let c = p.conj();
            let sym = SYM as u64;
            if v > 0.6 && self.cand.last().is_none_or(|l| i > l.0 + sym) {
                self.cand.push((i, v, c));
            } else if let Some(l) = self.cand.last_mut() {
                if i <= l.0 + sym && v > l.1 {
                    *l = (i, v, c);
                }
            }
        }
        // A candidate is final once the correlation has moved a symbol past it.
        let newest = self.fed.saturating_sub((N + w) as u64);
        let done = self
            .cand
            .iter()
            .take_while(|c| c.0 + (SYM as u64) < newest)
            .count();
        for k in 0..done {
            let (abs, _, corr) = self.cand[k];
            if abs < base || abs < self.accepted_until {
                // Gone from `x`, or a later symbol of a bootstrap, whose A part repeats too.
                continue;
            }
            let pos = (abs - base) as usize;
            // Fractional offset from the correlation phase, the whole carriers by testing the sequence.
            let frac = -(corr.arg() as f64) / std::f64::consts::TAU * RATE / N as f64;
            let spacing = RATE / N as f64;
            let mut best = (0isize, 0f32, 0usize);
            let kmax = (self.max_offset_hz / spacing).ceil() as isize;
            for k in -kmax..=kmax {
                let (s, q) = self.shift_at(x, pos + 520, frac, k);
                if q > best.1 {
                    best = (k, q, s);
                }
            }
            if best.1 < 0.1 {
                continue;
            }
            let cfo = frac + best.0 as f64 * spacing;
            let base = best.2;
            let mut vals = [0u16; 3];
            let nbits = [8u32, 7, 8];
            for k in 0..3 {
                // Symbol k+1: B (504), C (520), then A.
                let a_at = pos + SYM * (k + 1) + 504 + 520;
                let (s, _) = self.shift_at(x, a_at, cfo, 0);
                let rel = (s + N - base) % N;
                let g = ((rel + (1 << (11 - nbits[k] - 1)) % N) >> (11 - nbits[k])) as u16;
                vals[k] = ungray(g & ((1 << nbits[k]) - 1));
            }
            let f = Fields {
                minor: 0,
                ea_wake: vals[0] >> 7 != 0,
                min_time: ((vals[0] >> 2) & 31) as u8,
                bandwidth: (vals[0] & 3) as u8,
                rate_n: vals[1] as u8,
                preamble: vals[2] as u8,
            };
            out.push((pos, f, cfo));
            self.accepted_until = abs + (4 * SYM) as u64;
        }
        self.cand.drain(..done);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bootstrap_tables_and_round_trip() {
        assert_eq!(
            (
                min_time_ms(0),
                min_time_ms(10),
                min_time_ms(17),
                min_time_ms(30)
            ),
            (50, 700, 1500, 5300)
        );
        assert!((post_rate(8) - 9.216e6).abs() < 1.0);
        let f = Fields {
            minor: 0,
            ea_wake: true,
            min_time: 10,
            bandwidth: 0,
            rate_n: 2,
            preamble: 3,
        };
        let b = bootstrap(&f);
        assert_eq!(b.len(), 4 * 3072);
        let p: f32 = b.iter().map(|v| v.norm_sqr()).sum::<f32>() / b.len() as f32;
        assert!((p - 1.0).abs() < 0.05, "unit power: {p}");
        // In noise, at an offset: found within 2 samples, offset within 150 Hz, every field read back.
        let mut x = noise(30_000, 0.1, 3);
        let start = 7_777;
        let cfo = 8_300.0;
        let mut y = vec![Complex32::new(0.0, 0.0); start];
        y.extend_from_slice(&b);
        y.extend(std::iter::repeat_n(Complex32::new(0.0, 0.0), 10_000));
        for (i, v) in y.iter_mut().enumerate() {
            let ph = std::f64::consts::TAU * cfo * i as f64 / RATE;
            *v = *v * Complex32::new(ph.cos() as f32, ph.sin() as f32) + x[i % x.len()];
        }
        let mut d = Detector::new();
        let mut found = Vec::new();
        d.find(&y, 0, &mut found);
        assert_eq!(found.len(), 1, "{found:?}");
        let (pos, ff, off) = found[0];
        assert!((pos as i64 - start as i64).abs() <= 2, "{pos}");
        assert!((off - cfo).abs() < 150.0, "{off}");
        assert_eq!(ff, f);
        // Noise alone: nothing.
        x.extend(noise(30_000, 1.0, 9));
        let mut d = Detector::new();
        d.find(&x, 0, &mut found);
        assert!(found.is_empty());
    }

    fn noise(n: usize, sigma: f32, seed: u64) -> Vec<Complex32> {
        let mut r = sdrkit_algo_sim::rng::Rng::new(seed);
        (0..n)
            .map(|_| r.complex_normal((sigma * sigma) as f64))
            .collect()
    }

    #[test]
    fn a_bootstrap_after_loud_noise_is_still_found() {
        // Half a second of noise 60 dB louder than the signal, then a bootstrap.
        let f = Fields {
            preamble: 3,
            ..Default::default()
        };
        let mut x = noise(3_072_000, 1000.0, 5);
        let start = x.len() + 1000;
        x.extend(std::iter::repeat_n(Complex32::new(0.0, 0.0), 1000));
        x.extend_from_slice(&bootstrap(&f));
        x.extend(std::iter::repeat_n(Complex32::new(0.0, 0.0), 10_000));
        let mut d = Detector::new();
        let mut found = Vec::new();
        d.find(&x, 0, &mut found);
        assert_eq!(found.len(), 1, "{found:?}");
        assert!((found[0].0 as i64 - start as i64).abs() <= 2);
        assert_eq!(found[0].1, f);
    }
}
