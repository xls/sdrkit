//! MSK144 decoding: bursts found from the spectrum of the squared analytic signal, whose two lines at 2(fc +- 500)
//! give the carrier; frames found by correlation with the two sync words; 1, 2 and 3 frames averaged coherently; soft bits from
//! the half-sine matched filter; belief propagation of the (128,90) code and the CRC-13.

use super::slot::*;
use crate::encode::MSK_SYNC;
use crate::proto::msg77::{unpack77_learn, CallHashes};
use num_complex::Complex32;
use crate::proto::codes;
use sdrkit_util_bits::crc::cyclic::MSK144_CRC13;
use std::f64::consts::PI;

const FS: f64 = 16000.0;
/// Samples per bit at 16 kHz.
const NB: usize = 8;
/// Samples per frame.
const NF: usize = 144 * NB;

/// The complex template of the sync bits of a frame (bit k: a half-sine of two bit lengths at bit k's start, on the real part for
/// even k, the imaginary part for odd k, of sign -s_k).
fn sync_template(swap: bool) -> Vec<(usize, Complex32)> {
    let mut t = sync_word(swap, 0);
    t.extend(sync_word(swap, 56));
    t
}

/// The template of one sync word (at bit `pos`).
fn sync_word(swap: bool, pos: usize) -> Vec<(usize, Complex32)> {
    let mut t = Vec::new();
    {
        for (i, &b) in MSK_SYNC.iter().enumerate() {
            let k = pos + i;
            let s = -(2.0 * b as f32 - 1.0);
            for n in 0..2 * NB {
                let off = n as i64 - NB as i64;
                let w = (PI * (n as f64) / (2 * NB) as f64).sin() as f32;
                let idx = (k * NB) as i64 + off;
                let v = if k.is_multiple_of(2) != swap {
                    Complex32::new(s * w, 0.0)
                } else {
                    Complex32::new(0.0, s * w)
                };
                if idx >= 0 {
                    t.push((idx as usize, v));
                }
            }
        }
    }
    t
}

/// Soft bits of the frame at `start`, the carrier phase followed through the frame from the phases `c1` and `c2` of its two sync
/// words (a residual frequency error turns the phase by the end of a frame).
fn soft_bits(
    x: &[Complex32],
    start: usize,
    c1: Complex32,
    c2: Complex32,
    swap: bool,
    out: &mut [f32; 144],
) {
    let th1 = c1.arg() as f64;
    let d = (c2 * c1.conj()).arg() as f64;
    for (k, o) in out.iter_mut().enumerate() {
        let th = th1 + d * (k as f64 - 3.5) / 56.0;
        let phase = Complex32::new(th.cos() as f32, th.sin() as f32);
        let mut acc = Complex32::default();
        for n in 0..2 * NB {
            let idx = (start + k * NB + n) as i64 - NB as i64;
            if idx >= 0 && (idx as usize) < x.len() {
                let w = (PI * n as f64 / (2 * NB) as f64).sin() as f32;
                acc += x[idx as usize] * w;
            }
        }
        let v = acc * phase.conj();
        *o = if k.is_multiple_of(2) != swap {
            -v.re
        } else {
            -v.im
        };
    }
}

/// Decodes one MSK144 period.
#[allow(clippy::needless_range_loop)]
pub fn decode(slot: &Slot, hashes: &mut CallHashes) -> Vec<Decode> {
    let code = codes::msk144();
    let audio: Vec<f32> = slot
        .audio
        .iter()
        .map(|&x| if x.is_finite() { x } else { 0.0 })
        .collect();
    let n = smooth_at_least(audio.len());
    let spec = SlotSpectrum::new(&audio, n);
    // The analytic signal at 16 kHz.
    let mut z = Vec::new();
    spec.baseband(0.0, 300.0, 3700.0, 100.0, 2 * n, &mut z);
    z.truncate(audio.len() * 2);
    let total = z.len();
    // Detection: every 18 ms, the spectrum of the square over one frame.
    let hop = (0.018 * FS) as usize;
    let nfft = 2048;
    let df = FS / nfft as f64;
    let mut buf = vec![Complex32::default(); nfft];
    let mut dets: Vec<(f32, usize, f64, f32)> = Vec::new(); // (detection, start, fc, line ratio)
    let mut avg = vec![0f32; nfft];
    let mut w = 0;
    let mut nwin = 0;
    while w + NF <= total {
        for (i, b) in buf.iter_mut().enumerate() {
            *b = if i < NF {
                z[w + i] * z[w + i]
            } else {
                Complex32::default()
            };
        }
        fft_in_place(&mut buf, false);
        let p: Vec<f32> = buf.iter().map(|c| c.norm_sqr()).collect();
        for (a, &x) in avg.iter_mut().zip(&p) {
            *a += x;
        }
        nwin += 1;
        // The noise floor: the median over the band where the lines can be (2 x 500 .. 2 x 2800 Hz).
        let med = median(&p[(1000.0 / df) as usize..(5600.0 / df) as usize]).max(1e-30);
        let (mut best, mut bfc) = (0f32, 0f64);
        let mut fc = 1000.0;
        while fc <= 2300.0 {
            let b1 = (((2.0 * fc - 1000.0) / df).round() as i64).rem_euclid(nfft as i64) as usize;
            let b2 = (((2.0 * fc + 1000.0) / df).round() as i64).rem_euclid(nfft as i64) as usize;
            let v = p[b1] + p[b2];
            if v > best {
                best = v;
                bfc = fc;
            }
            fc += df / 4.0;
        }
        dets.push((best / med, w, bfc, best / (2.0 * med)));
        w += hop;
    }
    if dets.is_empty() {
        return Vec::new();
    }
    let dmed = median(&dets.iter().map(|d| d.0).collect::<Vec<_>>()).max(1e-30);
    let mut order: Vec<usize> = (0..dets.len()).collect();
    order.sort_by(|&a, &b| dets[b].0.total_cmp(&dets[a].0));
    // (start, carrier, continuous)
    let mut cands: Vec<(usize, f64, bool)> = Vec::new();
    for &i in &order {
        let (d, st, fc, ratio) = dets[i];
        if d < 3.0 * dmed && ratio < 40.0 {
            break;
        }
        if cands
            .iter()
            .any(|&(s, _, _)| (s as i64 - st as i64).unsigned_abs() < 4 * NF as u64)
        {
            continue;
        }
        cands.push((st, fc, false));
        if cands.len() >= 12 {
            break;
        }
    }
    // A transmission that never stops shows in the average.
    if nwin > 0 {
        let med = median(&avg[(1000.0 / df) as usize..(5600.0 / df) as usize]).max(1e-30);
        let (mut best, mut bfc) = (0f32, 0f64);
        let mut fc = 1000.0;
        while fc <= 2300.0 {
            let b1 = (((2.0 * fc - 1000.0) / df).round() as i64).rem_euclid(nfft as i64) as usize;
            let b2 = (((2.0 * fc + 1000.0) / df).round() as i64).rem_euclid(nfft as i64) as usize;
            let v = avg[b1] + avg[b2];
            if v > best {
                best = v;
                bfc = fc;
            }
            fc += df / 4.0;
        }
        if best / med >= 1.8 * 2.0 {
            let step = total / 8;
            for k in 0..8 {
                let st = k * step;
                cands.push((st, bfc, true));
            }
        }
    }
    let tmpls = [sync_template(false), sync_template(true)];
    let words = [
        [sync_word(false, 0), sync_word(false, 56)],
        [sync_word(true, 0), sync_word(true, 56)],
    ];
    let mut dec = LdpcDecoders::new(code);
    let mut cw = [0u8; 128];
    let mut out: Vec<Decode> = Vec::new();
    let mut x = Vec::new();
    for (st, fc0, continuous) in cands {
        // A transmission that never stops is averaged over up to 8 frames.
        let (back, fwd, max_nav) = if continuous { (0, 9, 8) } else { (2, 3, 3) };
        let mut done = false;
        // The convention of the transmitter (I and Q swapped, or the tones mirrored): all four are tried, the one whose sync
        // correlation peaks highest first (a wrong one correlates almost as well half a bit away).
        let a = st.saturating_sub(back * NF);
        let b = (st + fwd * NF).min(total);
        let mut order: Vec<(usize, f32)> = Vec::new();
        for variant in 0..4usize {
            let swap = variant & 1 == 1;
            let conj = variant & 2 == 2;
            x.clear();
            for i in a..b {
                let p = -2.0 * PI * fc0 * i as f64 / FS;
                let v = z[i] * Complex32::new(p.cos() as f32, p.sin() as f32);
                x.push(if conj { v.conj() } else { v });
            }
            let tm = &tmpls[swap as usize];
            let lags = x.len().saturating_sub(NF);
            let mut pk = 0f32;
            for l in (0..lags).step_by(2) {
                let mut acc = Complex32::default();
                for &(i, v) in tm {
                    if l + i < x.len() {
                        acc += x[l + i] * v.conj();
                    }
                }
                pk = pk.max(acc.norm());
            }
            order.push((variant, pk));
        }
        order.sort_by(|p, q| q.1.total_cmp(&p.1));
        for (dfc, variant) in [0.0, -2.0, 2.0, -4.0, 4.0]
            .into_iter()
            .flat_map(|d| order.clone().into_iter().map(move |v| (d, v.0)))
        {
            if done {
                break;
            }
            // The conventions of other transmitters: I and Q swapped, or the tones mirrored.
            let swap = variant & 1 == 1;
            let conj = variant & 2 == 2;
            let tmpl = &tmpls[swap as usize];
            let fc = fc0 + dfc;
            x.clear();
            for i in a..b {
                let p = -2.0 * PI * fc * i as f64 / FS;
                let v = z[i] * Complex32::new(p.cos() as f32, p.sin() as f32);
                x.push(if conj { v.conj() } else { v });
            }
            // Frame start: correlation with the sync words over one frame of lags.
            let corr = |lag: usize| -> Complex32 {
                let mut acc = Complex32::default();
                for &(i, v) in tmpl {
                    if lag + i < x.len() {
                        acc += x[lag + i] * v.conj();
                    }
                }
                acc
            };
            let lags = x.len().saturating_sub(NF);
            let mut peaks: Vec<(f32, usize)> = (0..lags).map(|l| (corr(l).norm(), l)).collect();
            peaks.sort_by(|p, q| q.0.total_cmp(&p.0));
            let mut tried = Vec::new();
            for &(_, l0) in peaks.iter() {
                if tried.len() >= 3 {
                    break;
                }
                if tried
                    .iter()
                    .any(|&t: &usize| (t as i64 - l0 as i64).abs() < 4)
                {
                    continue;
                }
                tried.push(l0);
                'outer: for l in [l0, l0.saturating_sub(1), l0 + 1] {
                    for nav in (1..=max_nav).filter(|&n| n <= 3 || n == 5 || n == max_nav) {
                        let mut soft = [0f32; 144];
                        let mut acc = [0f32; 144];
                        let mut used = 0;
                        for f in 0..nav {
                            let lf = l + f * NF;
                            if lf + NF > x.len() {
                                break;
                            }
                            let cw = |w: &[(usize, Complex32)]| -> Complex32 {
                                let mut acc = Complex32::default();
                                for &(i, v) in w {
                                    if lf + i < x.len() {
                                        acc += x[lf + i] * v.conj();
                                    }
                                }
                                acc
                            };
                            let (c1, c2) =
                                (cw(&words[swap as usize][0]), cw(&words[swap as usize][1]));
                            if c1.norm() == 0.0 || c2.norm() == 0.0 {
                                continue;
                            }
                            soft_bits(&x, lf, c1, c2, swap, &mut soft);
                            for (a2, s2) in acc.iter_mut().zip(&soft) {
                                *a2 += s2;
                            }
                            used += 1;
                        }
                        if used == 0 {
                            continue;
                        }
                        let mean = acc.iter().map(|v| v.abs()).sum::<f32>() / 144.0;
                        let var = acc.iter().map(|v| (v.abs() - mean).powi(2)).sum::<f32>() / 144.0;
                        let sig2 = var.max(1e-12);
                        let mut llr = [0f32; 128];
                        for (i, l2) in llr.iter_mut().enumerate() {
                            let k = if i < 48 { 8 + i } else { 16 + i };
                            // A positive soft value means 1, so the LLR (positive means 0) takes its negative.
                            *l2 = (-2.0 * acc[k] * mean / sig2).clamp(-25.0, 25.0);
                        }
                        if dec.decode(&llr, 30, false, &mut cw).is_none() {
                            continue;
                        }
                        if !MSK144_CRC13.check(&cw[..90]) {
                            continue;
                        }
                        let Some(u) = unpack77_learn(&cw[..77], hashes) else {
                            continue;
                        };
                        let t = slot.t_first + (a + l) as f64 / FS;
                        let snr_bit = 10.0 * ((mean * mean) / sig2).max(1e-6).log10() as f64
                            - 10.0 * (nav as f64).log10();
                        let snr = snr_bit - 3.0 + 10.0 * (2000.0f64 / 2500.0).log10();
                        if !out.iter().any(|o| o.text == u.text) {
                            out.push(Decode::new(slot, u.text, snr, t, fc - 500.0, false, 0));
                        }
                        done = true;
                        break 'outer;
                    }
                }
            }
        }
    }
    out
}
