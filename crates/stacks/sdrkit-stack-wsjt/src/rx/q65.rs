//! The Q65 parts of the slow decoder: the tone energies of a fitted candidate on a grid of a quarter symbol rate, the track of
//! the sync tone (offset and drift), the spread widths, the symbol probabilities, the code over GF(64) and the acceptance
//! checks.
//! Frame: "Quick-Start Guide to Q65" (2021). The metric constants are design values of the specification.

use super::slot::fft_in_place;
use super::slow::{Fit, SlowSpec};
use crate::encode::{q65_channel, q65_crc, q65_tones, Q65_SYNC};
use num_complex::Complex32;
use sdrkit_algo_fec::nbldpc::{NbCode, NbDecoder, NbStop};
use std::f64::consts::PI;

/// What the status says about the parts of Q65 that no public document states.
pub const UNCONFIRMED: &str = "Q65 parts that no public document confirms: the sync positions, the tone numbering of the \
    data symbols, the pad bit and the bit order of the symbols, the CRC-12 polynomial and its bit order (unproven until a \
    real recording decodes), the GF(64) polynomial, the code table and the order of the sent symbols, the samples per \
    symbol and the start time, the audio band and the dial frequencies; the constants of the metric are design values";

/// The symbol Es/N0 the metric assumes (linear).
const ES_N0: f64 = 2.3591;
/// Belief propagation iterations at most.
const MAX_ITER: u32 = 100;
/// The stop rule: the largest extrinsic probabilities of the 65 symbols sum to more than 65 minus this.
const SLACK: f32 = 0.01;
/// The least sum of the log probabilities of the 63 decoded channel symbols (a uniform guess gives -262).
const MIN_LOG_LIKELIHOOD: f64 = -260.0;
/// Grid points kept below tone 0 (the track reaches 7 symbol rates) and above the top tone (the widest smoothing).
const BELOW: usize = 40;
const ABOVE: usize = 24;
/// Grid points per symbol rate.
const FINE: usize = 4;
/// The spread widths tried, in grid points (standard deviation of the smoothing), in order.
const WIDTHS: [f64; 7] = [0.0, 0.6, 1.2, 2.4, 4.8, 7.0, 10.0];

/// The powers of every symbol of a fitted candidate around its 65 tones, on a grid of a quarter symbol rate.
pub struct Grid {
    p: Vec<f32>,
    width: usize,
    /// Grid points between neighbouring tones.
    step: usize,
    nsym: usize,
}

impl Grid {
    /// The grid of the fit `f`: each symbol with its frequency offset and drift removed, padded to four symbol lengths.
    pub fn new(f: &Fit, s: &SlowSpec) -> Grid {
        let n = s.sps2;
        let nf = FINE * n;
        let step = FINE * (s.spacing * s.sym_s).round().max(1.0) as usize;
        let width = BELOW + (s.ntones - 1) * step + ABOVE + 1;
        let mut p = vec![0f32; s.nsym * width];
        let mut buf = vec![Complex32::default(); nf];
        for k in 0..s.nsym {
            let start = ((f.t_audio + k as f64 * s.sym_s) * f.fs).round() as i64;
            let freq = f.df + f.drift_hz * (k as f64 / s.nsym as f64 - 0.5);
            let w = -2.0 * PI * freq / f.fs;
            for (i, b) in buf.iter_mut().enumerate() {
                let j = start + i as i64;
                *b = if i < n && j >= 0 && (j as usize) < f.bb.len() {
                    let ph = w * i as f64;
                    f.bb[j as usize] * Complex32::new(ph.cos() as f32, ph.sin() as f32)
                } else {
                    Complex32::default()
                };
            }
            fft_in_place(&mut buf, false);
            for (i, o) in p[k * width..(k + 1) * width].iter_mut().enumerate() {
                let bin = (i as i64 - BELOW as i64).rem_euclid(nf as i64) as usize;
                *o = buf[bin].norm_sqr();
            }
        }
        Grid {
            p,
            width,
            step,
            nsym: s.nsym,
        }
    }

    /// The power of symbol `k` at `fine` grid points above tone 0 (0 outside the grid).
    fn at(&self, k: usize, fine: i64) -> f32 {
        let i = fine + BELOW as i64;
        if i < 0 || i as usize >= self.width {
            0.0
        } else {
            self.p[k * self.width + i as usize]
        }
    }

    /// The offset `a` and the drift `d` over the transmission of the sync tone, in symbol rates: the track
    /// `a + d (k / symbols - 1/2)` with the most sync power, for `a` from -4 to 4 and `d` from -6 to 6 in quarters. A drift is
    /// returned only when its track holds 1.15 times the power of the best track without drift.
    pub fn track(&self) -> (f64, f64) {
        let score = |a: i64, d: i64| -> f32 {
            Q65_SYNC
                .iter()
                .map(|&k| {
                    let off = a as f64 + d as f64 * (k as f64 / self.nsym as f64 - 0.5);
                    self.at(k, off.round() as i64)
                })
                .sum()
        };
        let q = FINE as i64;
        let mut still = (f32::MIN, 0i64);
        for a in -4 * q..=4 * q {
            let v = score(a, 0);
            if v > still.0 || (v == still.0 && a.abs() < still.1.abs()) {
                still = (v, a);
            }
        }
        let mut moving = (f32::MIN, 0i64, 0i64);
        for d in (-6 * q..=6 * q).filter(|&d| d != 0) {
            for a in -4 * q..=4 * q {
                let v = score(a, d);
                if v > moving.0 {
                    moving = (v, a, d);
                }
            }
        }
        if moving.0 >= 1.15 * still.0 {
            (moving.1 as f64 / q as f64, moving.2 as f64 / q as f64)
        } else {
            (still.1 as f64 / q as f64, 0.0)
        }
    }

    /// The energies of the 63 data symbols `k` and the 64 values `v` (`out[k * 64 + v]`), smoothed over
    /// neighbouring grid points with a Gaussian of standard deviation `sigma` cut at 2.2 `sigma` (0: no smoothing).
    fn energies(&self, sigma: f64, out: &mut Vec<f32>) {
        let reach = (2.2 * sigma).ceil() as i64;
        let mut w: Vec<f32> = (-reach..=reach)
            .map(|j| {
                if sigma > 0.0 {
                    (-0.5 * (j as f64 / sigma).powi(2)).exp() as f32
                } else {
                    1.0
                }
            })
            .collect();
        let sum: f32 = w.iter().sum();
        w.iter_mut().for_each(|x| *x /= sum);
        out.clear();
        for k in (0..self.nsym).filter(|k| !Q65_SYNC.contains(k)) {
            for v in 0..64i64 {
                let centre = (v + 1) * self.step as i64;
                out.push(
                    w.iter()
                        .zip(-reach..=reach)
                        .map(|(&x, j)| x * self.at(k, centre + j))
                        .sum(),
                );
            }
        }
    }

    /// The spread widths this tone spacing allows: a width must stay under 0.45 of the spacing plus half a grid point, and
    /// the two widest need a spacing above one symbol rate.
    fn widths(&self) -> impl Iterator<Item = f64> + '_ {
        let limit = 0.45 * self.step as f64 + 0.5;
        WIDTHS
            .into_iter()
            .filter(move |&w| w <= limit && (w < 7.0 || self.step > FINE))
    }
}

/// The probabilities of the 64 values of each data symbol from its energies (`e[k * 64 + v]`), each row summing to 1: a
/// rational approximation of the logarithm of the Bessel function I0 of the scaled amplitude, at the assumed Es/N0.
fn probabilities(e: &[f32], out: &mut [f32]) {
    let mean = e.iter().map(|&x| x as f64).sum::<f64>() / e.len().max(1) as f64;
    let sigma = (mean / (1.0 + ES_N0 / 64.0) / 2.0).sqrt().max(1e-30);
    let c = (2.0 * ES_N0).sqrt() / sigma;
    let mut l = [0f64; 64];
    for (row, o) in e.as_chunks::<64>().0.iter().zip(out.as_chunks_mut::<64>().0) {
        for (x, &v) in l.iter_mut().zip(row) {
            let u = c * (v.max(0.0) as f64).sqrt();
            *x = (u * u * (u + 0.039) / (0.9931 * u * u + 2.6936 * u + 0.5185)).min(80.0);
        }
        let top = l.iter().copied().fold(f64::MIN, f64::max);
        let sum: f64 = l.iter().map(|&x| (x - top).exp()).sum();
        for (p, &x) in o.iter_mut().zip(&l) {
            *p = ((x - top).exp() / sum) as f32;
        }
    }
}

/// The SNR in 2500 Hz from the energies and the decoded channel symbols: the mean energy of the sent values against the mean
/// of all other cells, in the bandwidth of one symbol rate.
fn snr_of(e: &[f32], channel: &[u8], symbol_rate: f64) -> f64 {
    let (mut sent, mut other) = (0f64, 0f64);
    for (row, &c) in e.as_chunks::<64>().0.iter().zip(channel) {
        for (v, &x) in row.iter().enumerate() {
            if v == c as usize {
                sent += x as f64;
            } else {
                other += x as f64;
            }
        }
    }
    let s = sent / channel.len().max(1) as f64;
    let nz = (other / (63 * channel.len()).max(1) as f64).max(1e-30);
    let signal = if s > nz { s - nz } else { 0.316 * nz };
    (10.0 * (signal / nz).log10() + 10.0 * (symbol_rate / 2500.0).log10()).max(-35.0)
}

/// A decoded Q65 word.
pub struct Word {
    /// The message.
    pub text: String,
    /// The 85 tones sent.
    pub tones: Vec<u8>,
    /// The SNR in 2500 Hz, dB.
    pub snr: f64,
}

/// The decoder of the Q65 code with its buffers.
pub struct Decoder {
    dec: NbDecoder,
    e: Vec<f32>,
    /// The probabilities of the 63 symbols sent.
    sent: Vec<f32>,
    /// The probabilities of the 65 code symbols.
    probs: Vec<f32>,
    hard: Vec<u16>,
}

impl Decoder {
    /// A decoder for the loaded code (65 symbols over GF(64), the 15 information symbols first).
    pub fn new(code: &NbCode) -> Decoder {
        let mut dec = NbDecoder::new(code);
        dec.set_stop(NbStop::Confidence { slack: SLACK });
        Decoder {
            dec,
            e: Vec::new(),
            sent: vec![0.0; 63 * 64],
            // The two CRC symbols are not sent: their rows stay uniform.
            probs: vec![1.0 / 64.0; 65 * 64],
            hard: vec![0; 65],
        }
    }

    /// Decodes the grid of a candidate: each spread width in turn, until one gives a word that passes every check. `unpack`
    /// turns the 77 message bits into text, or refuses them. `symbol_rate` is in Hz.
    ///
    /// The checks, in order: belief propagation converged on a code word, the CRC symbols fit the message symbols and the
    /// pad bit is 0, the 63 decoded channel symbols are likely enough under the measured probabilities, the bits unpack.
    pub fn decode(
        &mut self,
        g: &Grid,
        symbol_rate: f64,
        mut unpack: impl FnMut(&[u8]) -> Option<String>,
    ) -> Option<Word> {
        if self.dec.code().n() != 65 || self.dec.code().q() != 64 {
            return None;
        }
        for sigma in g.widths() {
            g.energies(sigma, &mut self.e);
            // The message symbols are rows 0 to 12, the check symbols rows 15 to 64.
            probabilities(&self.e, &mut self.sent);
            self.probs[..13 * 64].copy_from_slice(&self.sent[..13 * 64]);
            self.probs[15 * 64..].copy_from_slice(&self.sent[13 * 64..]);
            let Ok(r) = self.dec.decode(&self.probs, MAX_ITER, &mut self.hard) else {
                continue;
            };
            if r.metric != 0 {
                continue;
            }
            let mut info = [0u8; 15];
            for (o, &h) in info.iter_mut().zip(&self.hard) {
                *o = h as u8;
            }
            let Ok(msg) = <&[u8; 13]>::try_from(&info[..13]) else {
                continue;
            };
            if q65_crc(msg) != info[13..] || info[12] & 1 != 0 {
                continue;
            }
            let channel = q65_channel(self.dec.code(), &info);
            if channel.len() != 63 {
                continue;
            }
            let likelihood: f64 = channel
                .iter()
                .enumerate()
                .map(|(k, &c)| (self.sent[k * 64 + c as usize].max(1e-36) as f64).ln())
                .sum();
            if likelihood < MIN_LOG_LIKELIHOOD {
                continue;
            }
            let bits: Vec<u8> = info[..13]
                .iter()
                .flat_map(|&x| (0..6).rev().map(move |b| x >> b & 1))
                .take(77)
                .collect();
            let Some(text) = unpack(&bits) else {
                continue;
            };
            return Some(Word {
                text,
                snr: snr_of(&self.e, &channel, symbol_rate),
                tones: q65_tones(&channel),
            });
        }
        None
    }
}
