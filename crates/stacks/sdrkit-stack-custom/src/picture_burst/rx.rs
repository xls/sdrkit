//! The receiver of a captured `picture_burst` burst at 632 ksps: the start sweeps and the carrier offset, the header with
//! trials of clock and timing, the end sweeps and the clock error, pilot tracking, and the layer blocks until the first that
//! fails; then the picture from the good bytes.

use super::{block_seed, parse_header, pilot, rrc, scrambler, slots_for, up_sweep, Header, FS, HEADER_CODED, HEADER_SYMBOLS, N, SLOT0, SWEEP};
use crate::family::{self, ldpc, qam};
use sdrkit_algo_dsp::fft::Fft;
use sdrkit_algo_dsp::interp::Cubic;
use sdrkit_algo_fec::conv::codes::K7_171_133_165_117;
use sdrkit_algo_fec::conv::{Ends, Viterbi};
use sdrkit_algo_fec::ldpc::BpDecoder;
use sdrkit_algo_modem::Demap;
use sdrkit_core::Complex32;
use std::f64::consts::TAU;

/// What a burst gave.
#[derive(Clone, Debug, PartialEq)]
pub struct Decoded {
    /// The header.
    pub header: Header,
    /// Good blocks of each layer.
    pub good: Vec<usize>,
    /// Layers with every block good, from the first.
    pub layers_complete: usize,
    /// The good bytes before the first failed block.
    pub stream: Vec<u8>,
    /// SNR in 190 kHz, dB.
    pub snr_db: f64,
    /// Carrier offset, Hz.
    pub cfo_hz: f64,
    /// Sample clock error, ppm (`None` when the end sweep was not found).
    pub clock_ppm: Option<f64>,
    /// The first sample of the burst in the input.
    pub start: f64,
    /// The picture, RGB, when it decodes.
    pub picture: Option<Vec<u8>>,
}

/// `|sum_k x[n + k] conj(t[k])|^2` for every `n` of `x`, with zeros after its end, by overlap-save.
pub fn xcorr_power(x: &[Complex32], t: &[Complex32]) -> Vec<f32> {
    let m = t.len();
    if x.is_empty() {
        return Vec::new();
    }
    let size = (4 * m).next_power_of_two();
    let step = size - m + 1;
    let mut ft = vec![Complex32::new(0.0, 0.0); size];
    ft[..m].copy_from_slice(t);
    let mut fwd = Fft::forward(size);
    let mut inv = Fft::inverse(size);
    fwd.process(&mut ft);
    let outs = x.len();
    let mut out = Vec::with_capacity(outs);
    let mut buf = vec![Complex32::new(0.0, 0.0); size];
    let mut at = 0;
    while at < outs {
        buf.fill(Complex32::new(0.0, 0.0));
        let end = (at + size).min(x.len());
        buf[..end - at].copy_from_slice(&x[at..end]);
        fwd.process(&mut buf);
        for (b, f) in buf.iter_mut().zip(&ft) {
            *b *= f.conj();
        }
        inv.process(&mut buf);
        let s = 1.0 / size as f32;
        for v in buf.iter().take(step.min(outs - at)) {
            out.push((v * s).norm_sqr());
        }
        at += step;
    }
    out
}

fn refine(c: &[f32], p: usize) -> f64 {
    if p == 0 || p + 1 >= c.len() {
        return p as f64;
    }
    let (a, b, d) = (f64::from(c[p - 1]).sqrt(), f64::from(c[p]).sqrt(), f64::from(c[p + 1]).sqrt());
    p as f64 + crate::family::fit::parabola(a, b, d)
}

fn argmax(c: &[f32], lo: usize, hi: usize) -> Option<usize> {
    let hi = hi.min(c.len());
    (lo..hi).max_by(|&a, &b| c[a].total_cmp(&c[b]))
}

/// What the start sweeps give: the start, the carrier offset (Hz), the up and down correlations and their mean.
struct Start {
    at: f64,
    cfo: f64,
    cu: Vec<f32>,
    cd: Vec<f32>,
    mean: f64,
}

/// The start sweeps.
fn find_start(x: &[Complex32]) -> Option<Start> {
    let up = up_sweep();
    let down: Vec<Complex32> = up.iter().map(|v| v.conj()).collect();
    let cu = xcorr_power(x, up);
    let cd = xcorr_power(x, &down);
    if cu.is_empty() {
        return None;
    }
    let mean = cu.iter().map(|&v| f64::from(v)).sum::<f64>() / cu.len() as f64;
    let p1 = argmax(&cu, 0, cu.len())?;
    if f64::from(cu[p1]) < 40.0 * mean {
        return None;
    }
    // The end sweep is as strong: a second peak more than 2 s earlier is the start.
    let guard = (0.5 * FS) as usize;
    let mut masked = cu.clone();
    for v in &mut masked[p1.saturating_sub(guard)..(p1 + guard).min(cu.len())] {
        *v = 0.0;
    }
    let mut start = p1;
    if let Some(p2) = argmax(&masked, 0, masked.len()) {
        if masked[p2] > 0.3 * cu[p1] && p2 < p1 && (p1 - p2) as f64 > 2.0 * FS {
            start = p2;
        }
    }
    let pu = argmax(&cu, start.saturating_sub(2), start + 3)?;
    if f64::from(cu[pu]) < 40.0 * mean {
        return None;
    }
    let nu = refine(&cu, pu);
    let w = (2.0 * 15_000.0 / super::SWEEP_RATE * FS) as usize + 8;
    let c = pu + SWEEP;
    let pd = argmax(&cd, c.saturating_sub(w), c + w + 1)?;
    if f64::from(cd[pd]) < 0.5 * 40.0 * mean {
        return None;
    }
    let nd = refine(&cd, pd);
    let st = (nu + nd - SWEEP as f64) / 2.0;
    let cfo = super::SWEEP_RATE * (nd - SWEEP as f64 - nu) / (2.0 * FS);
    if cfo.abs() > 15_000.0 {
        return None;
    }
    Some(Start { at: st, cfo, cu, cd, mean })
}

/// Pilot tracking of a run of slots from slot 0 (E.11.4). Returns the pilot Es/N0 and the noise variance after the gain is
/// removed; `None` with fewer than 8 pilots.
pub fn track(s: &mut [Complex32], seg_pilots: usize) -> Option<(f64, f64, f64)> {
    let np = s.len() / 32;
    if np < 8 {
        return None;
    }
    let z_of = |s: &[Complex32]| -> Vec<Complex32> { (0..np).map(|i| s[32 * i + 31] * pilot(i as u64)).collect() };
    let mut freq = 0f64;
    for pass in 0..2 {
        let z = z_of(s);
        let sg = (seg_pilots >> pass).min(np / 3).max(4);
        let sums: Vec<Complex32> = z.chunks(sg).filter(|c| c.len() == sg).map(|c| c.iter().sum()).collect();
        let acc: Complex32 = sums.windows(2).map(|w| w[1] * w[0].conj()).sum();
        let w = f64::from(acc.arg()) / (sg * 32) as f64;
        freq += w;
        for (k, v) in s.iter_mut().enumerate() {
            let a = -w * k as f64;
            *v *= Complex32::new(a.cos() as f32, a.sin() as f32);
        }
    }
    let z = z_of(s);
    let smooth = |z: &[Complex32], w: usize| -> Vec<Complex32> {
        let h = w / 2;
        let mut pre = vec![Complex32::new(0.0, 0.0); z.len() + 1];
        for i in 0..z.len() {
            pre[i + 1] = pre[i] + z[i];
        }
        (0..z.len())
            .map(|i| {
                let lo = i.saturating_sub(h);
                let hi = (i + h + 1).min(z.len());
                (pre[hi] - pre[lo]) / (hi - lo) as f32
            })
            .collect()
    };
    let odd = |w: usize| if w.is_multiple_of(2) { w.saturating_sub(1).max(1) } else { w };
    let w0 = odd(257.min(np));
    let g0 = smooth(&z, w0);
    let ps = g0.iter().map(|v| f64::from(v.norm_sqr())).sum::<f64>() / np as f64;
    let pn = z.iter().zip(&g0).map(|(a, b)| f64::from((a - b).norm_sqr())).sum::<f64>() / np as f64;
    let snr = ps / pn.max(1e-20);
    let w = odd(((30.0 / snr).round() as usize).clamp(5, 401).min(np));
    let g = smooth(&z, w);
    let gp = g.iter().map(|v| f64::from(v.norm_sqr())).sum::<f64>() / np as f64;
    let noise = z.iter().zip(&g).map(|(a, b)| f64::from((a - b).norm_sqr())).sum::<f64>() / np as f64 * w as f64
        / (w as f64 - 1.0).max(1.0);
    for (k, v) in s.iter_mut().enumerate() {
        // Pilot i sits at slot 32 i + 31.
        let pos = (k as f64 - 31.0) / 32.0;
        let gk = if pos <= 0.0 {
            g[0]
        } else if pos >= (np - 1) as f64 {
            g[np - 1]
        } else {
            let i = pos.floor() as usize;
            let f = (pos - i as f64) as f32;
            g[i] * (1.0 - f) + g[i + 1] * f
        };
        *v = if gk.norm() > 1e-20 { *v / gk } else { *v };
    }
    Some((gp / noise.max(1e-20), noise / gp.max(1e-20), freq))
}

/// The matched-filter output of the mixed input, with the slot reader.
struct Front {
    z: Vec<Complex32>,
    /// Absolute sample of `z[0]`.
    base: i64,
}

impl Front {
    fn new(x: &[Complex32], from: i64, to: i64, cfo: f64) -> Front {
        let h = rrc();
        let from = from.max(0);
        let to = to.min(x.len() as i64);
        let n = (to - from).max(0) as usize;
        let mixed: Vec<Complex32> = (0..n)
            .map(|i| {
                let k = from + i as i64;
                let a = -TAU * (cfo * k as f64 / FS).fract();
                x[k as usize] * Complex32::new(a.cos() as f32, a.sin() as f32)
            })
            .collect();
        let z = (0..n)
            .map(|i| {
                if i < 24 || i + 25 > n {
                    return Complex32::new(0.0, 0.0);
                }
                mixed[i - 24..i + 25].iter().zip(h).map(|(a, &b)| a * b).sum()
            })
            .collect();
        Front { z, base: from }
    }

    fn at(&self, pos: f64) -> Complex32 {
        let fl = pos.floor() as i64 - self.base;
        if fl < 1 || fl as usize + 3 > self.z.len() {
            return Complex32::new(0.0, 0.0);
        }
        let f = (pos - pos.floor()) as f32;
        let w = Cubic::weights(f);
        let b = (fl - 1) as usize;
        self.z[b] * w[0] + self.z[b + 1] * w[1] + self.z[b + 2] * w[2] + self.z[b + 3] * w[3]
    }

    fn slots(&self, start: f64, ratio: f64, n: usize) -> Vec<Complex32> {
        (0..n).map(|k| self.at(start + (SLOT0 as f64 + 4.0 * k as f64) * ratio)).collect()
    }
}

/// The data symbols of tracked slots: the pilots taken out.
fn data_of(s: &[Complex32]) -> Vec<Complex32> {
    s.iter().enumerate().filter(|(k, _)| k % 32 != 31).map(|(_, &v)| v).collect()
}

fn header_of(vit: &mut Viterbi, data: &[Complex32], nv: f64) -> Option<Header> {
    let mut sum = vec![0f32; HEADER_CODED];
    let w = (2.0 / nv.max(1e-6)) as f32;
    for c in 0..16 {
        let s = scrambler(0x4000 + c as u32, HEADER_CODED);
        for i in 0..HEADER_CODED {
            let l = data[c * HEADER_CODED + i].re * w;
            sum[i] += if s[i] == 1 { -l } else { l };
        }
    }
    let mean = sum.iter().map(|v| v.abs()).sum::<f32>() / sum.len() as f32;
    let llr: Vec<i8> = sum.iter().map(|v| (v * 20.0 / mean.max(1e-12)).round().clamp(-127.0, 127.0) as i8).collect();
    let mut bits = vec![0u8; super::HEADER_BITS];
    vit.decode(&llr, Ends::Zero, &mut bits).ok()?;
    parse_header(&bits)
}

/// Decodes the first burst in `x` (632 ksps).
pub fn decode(x: &[Complex32]) -> Option<Decoded> {
    let Start { at: start0, cfo, cu, cd, mean } = find_start(x)?;
    let front = Front::new(x, start0 as i64 + SLOT0 as i64 - 200, start0 as i64 + (10.6 * FS) as i64, cfo);
    let hslots = slots_for(HEADER_SYMBOLS);
    let mut vit = Viterbi::new(K7_171_133_165_117, super::HEADER_BITS + 6).ok()?;
    let mut found = None;
    'trials: for d in [0.0, 30e-6, -30e-6, 60e-6, -60e-6, 90e-6, -90e-6] {
        for off in [0.0, 0.25, -0.25, 0.5, -0.5, 0.75, -0.75] {
            let ratio = 1.0 + d;
            let mut s = front.slots(start0 + off, ratio, hslots);
            let Some((_, nv, _)) = track(&mut s, 64) else {
                continue;
            };
            if let Some(h) = header_of(&mut vit, &data_of(&s), nv) {
                found = Some((h, ratio, off));
                break 'trials;
            }
        }
    }
    let (header, ratio0, off0) = found?;
    let start = start0 + off0;
    let data_total = HEADER_SYMBOLS + header.layers.iter().map(|l| usize::from(l.blocks) * l.block_symbols()).sum::<usize>();
    let s_total = slots_for(data_total);
    let e = (SLOT0 + super::GUARD + 4 * s_total) as f64;
    // The end sweeps give the clock.
    let mut ratio = ratio0;
    let mut clock_ppm = None;
    let ce = (start + e * ratio0).round() as usize;
    let w = (3e-4 * e) as usize + 40;
    if let Some(pu) = argmax(&cu, ce.saturating_sub(w), ce + w + 1) {
        if f64::from(cu[pu]) >= 20.0 * mean {
            let nu = refine(&cu, pu);
            let wd = (2.0 * 15_000.0 / super::SWEEP_RATE * FS) as usize + 8;
            let c = pu + SWEEP;
            if let Some(pd) = argmax(&cd, c.saturating_sub(wd), c + wd + 1) {
                if f64::from(cd[pd]) >= 10.0 * mean {
                    let nd = refine(&cd, pd);
                    let end_start = (nu + nd - SWEEP as f64) / 2.0;
                    let r = (end_start - start0) / e;
                    if (r - 1.0).abs() < 3e-4 {
                        ratio = r;
                        clock_ppm = Some((r - 1.0) * 1e6);
                    }
                }
            }
        }
    }
    // Fine timing by the pilot Es/N0 over the first slots.
    let probe = s_total.min(60_000);
    let mut best = (f64::MIN, 0.0);
    for i in 0..=24 {
        let t = -1.5 + 0.125 * f64::from(i);
        let mut s = front.slots(start + t, ratio, probe);
        if let Some((es, _, _)) = track(&mut s, 128) {
            if es > best.0 {
                best = (es, t);
            }
        }
    }
    let mut s = front.slots(start + best.1, ratio, s_total);
    let (esn0, nv, freq) = track(&mut s, 256)?;
    let data = data_of(&s);
    // The layers.
    let mut at = HEADER_SYMBOLS;
    let mut g = 0;
    let mut good = Vec::with_capacity(header.layers.len());
    let mut stream = Vec::new();
    let mut failed = false;
    for l in &header.layers {
        let mut ok = 0;
        let per = N / l.bits();
        let c = qam::constellation(l.bits())?;
        let code = ldpc::code(N, usize::from(l.rate))?;
        let mut dec = code.decoder();
        let mut bp: Option<BpDecoder> = None;
        for _ in 0..l.blocks {
            if failed {
                at += l.block_symbols();
                g += 1;
                continue;
            }
            let mut sum = vec![0f32; N];
            let mut llr = vec![0f32; N];
            for r in 0..usize::from(l.reps) {
                let sym = &data[at + r * per..at + (r + 1) * per];
                let _ = c.demap_block(sym, nv.max(1e-6) as f32, Demap::MaxLog, &mut llr);
                let sc = scrambler(block_seed(g, r), N);
                for i in 0..N {
                    sum[i] += if sc[i] == 1 { -llr[i] } else { llr[i] };
                }
            }
            at += l.block_symbols();
            g += 1;
            let mean = sum.iter().map(|v| v.abs()).sum::<f32>() / N as f32;
            let q: Vec<i8> = sum.iter().map(|v| (v * 12.0 / mean.max(1e-12)).round().clamp(-127.0, 127.0) as i8).collect();
            let mut out = vec![0u8; N];
            let k = l.k();
            let crc_ok = |out: &[u8]| family::crc16(&family::pack(&out[..k - 16])) == family::read_bits(out, k - 16, 16) as u16;
            let mut ok_block = dec.decode(&q, 50, &mut out).is_ok() && crc_ok(&out);
            if !ok_block {
                // Sum-product decoding in f32 recovers blocks near the threshold of the 8-bit min-sum decoder.
                let bp = bp.get_or_insert_with(|| BpDecoder::new(&code.code));
                ok_block = bp.decode(&sum, 50, &mut out).is_ok() && crc_ok(&out);
            }
            let bytes = family::pack(&out[..k - 16]);
            if ok_block {
                ok += 1;
                stream.extend_from_slice(&bytes);
            } else {
                failed = true;
            }
        }
        good.push(ok);
    }
    let complete = good
        .iter()
        .zip(&header.layers)
        .take_while(|(g, l)| **g == usize::from(l.blocks))
        .count();
    let ph = sdrkit_codec_custom::picture::Header {
        width: header.width,
        height: header.height,
        levels: header.levels,
        top_plane: header.top_plane,
        decisions: header.decisions,
    };
    let mut rgb = Vec::new();
    let picture = sdrkit_codec_custom::picture::Decoder::new().decode(&ph, &stream, &mut rgb).ok().map(|_| rgb);
    Some(Decoded {
        header,
        good,
        layers_complete: complete,
        stream,
        snr_db: 10.0 * esn0.log10() - 10.0 * (190_000f64 / 158_000.0).log10(),
        cfo_hz: cfo + freq / TAU * super::SYMBOL_RATE,
        clock_ppm,
        start: start + best.1,
        picture,
    })
}

/// The PSNR of `b` against `a` (8-bit samples), dB.
pub fn psnr(a: &[u8], b: &[u8]) -> f64 {
    let n = a.len().min(b.len()).max(1);
    let mse = a.iter().zip(b).map(|(&x, &y)| (f64::from(x) - f64::from(y)).powi(2)).sum::<f64>() / n as f64;
    10.0 * (255.0 * 255.0 / mse.max(1e-12)).log10()
}
