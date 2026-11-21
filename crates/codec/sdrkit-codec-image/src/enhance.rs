//! Picture enhancement for grey satellite and fax pictures. Each step makes a new picture and never changes the decoded
//! data. Level 0 is "no data" and stays 0.

use crate::ImageError;

fn check(len: usize, w: usize, h: usize) -> Result<(), ImageError> {
    match w.checked_mul(h) {
        Some(n) if n <= len => Ok(()),
        _ => Err(ImageError::WrongLength {
            expected: w.saturating_mul(h),
            got: len,
        }),
    }
}

/// Runs the satellite preset on a grey picture of `w` by `h` pixels. If `src` holds fewer than `w` x `h` bytes, it returns an
/// error. A picture smaller than 3 x 3 comes back unchanged.
///
/// ```
/// use sdrkit_codec_image::enhance::satellite;
///
/// let (w, h) = (64, 32);
/// let mut p: Vec<u8> = (0..w * h).map(|i| (40 + (i % w) * 2) as u8).collect();
/// p[0] = 0; // no data
/// let out = satellite(&p, w, h).unwrap();
/// assert_eq!(out[0], 0);
/// assert_eq!(out.len(), w * h);
/// ```
pub fn satellite(src: &[u8], w: usize, h: usize) -> Result<Vec<u8>, ImageError> {
    check(src.len(), w, h)?;
    let src = &src[..w * h];
    if w < 3 || h < 3 {
        return Ok(src.to_vec());
    }
    let mut p: Vec<f32> = src.iter().map(|&v| v as f32).collect();
    let valid: Vec<bool> = src.iter().map(|&v| v != 0).collect();
    line_repair(&mut p, w, h, 0.35)?;
    remove_impulses(&mut p, w, h, 0.25)?;
    destripe(&mut p, w, h, 15)?;
    bilateral(&mut p, w, h, 15.0)?;
    levels(&mut p, &valid, 0.003, 0.997, 4.0);
    let mut out: Vec<u8> = p
        .iter()
        .map(|&v| v.round().clamp(0.0, 255.0) as u8)
        .collect();
    clahe(&mut out, w, h, 8, 2.0)?;
    let mut f: Vec<f32> = out.iter().map(|&v| v as f32).collect();
    unsharp(&mut f, w, h, 0.3)?;
    Ok(f.iter()
        .zip(&valid)
        .map(|(&v, &ok)| {
            if ok {
                v.round().clamp(1.0, 255.0) as u8
            } else {
                0
            }
        })
        .collect())
}

fn corr(a: &[f32], b: &[f32]) -> f32 {
    let n = a.len() as f32;
    let (ma, mb) = (a.iter().sum::<f32>() / n, b.iter().sum::<f32>() / n);
    let (mut sab, mut saa, mut sbb) = (0.0, 0.0, 0.0);
    for (x, y) in a.iter().zip(b) {
        let (dx, dy) = (x - ma, y - mb);
        sab += dx * dy;
        saa += dx * dx;
        sbb += dy * dy;
    }
    if saa < 1e-6 || sbb < 1e-6 {
        1.0
    } else {
        sab / (saa * sbb).sqrt()
    }
}

/// Replaces each damaged line by the mean of its two neighbours. A line is damaged when its correlation with both neighbours is
/// below `limit`.
pub fn line_repair(p: &mut [f32], w: usize, h: usize, limit: f32) -> Result<(), ImageError> {
    check(p.len(), w, h)?;
    for y in 1..h.saturating_sub(1) {
        let (up, rest) = p.split_at_mut(y * w);
        let (cur, down) = rest.split_at_mut(w);
        let prev = &up[(y - 1) * w..];
        let next = &down[..w];
        if corr(cur, prev) < limit && corr(cur, next) < limit {
            for x in 0..w {
                cur[x] = (prev[x] + next[x]) / 2.0;
            }
        }
    }
    Ok(())
}

/// A switching median filter. If a pixel differs from the median of its 3 x 3 neighbours by more than `t` x 255, the median
/// replaces it.
pub fn remove_impulses(p: &mut [f32], w: usize, h: usize, t: f32) -> Result<(), ImageError> {
    check(p.len(), w, h)?;
    let src = p[..w * h].to_vec();
    for y in 1..h.saturating_sub(1) {
        for x in 1..w.saturating_sub(1) {
            let mut n = [0f32; 9];
            for (k, v) in n.iter_mut().enumerate() {
                *v = src[(y + k / 3 - 1) * w + x + k % 3 - 1];
            }
            n.sort_by(f32::total_cmp);
            let m = n[4];
            if (src[y * w + x] - m).abs() > t * 255.0 {
                p[y * w + x] = m;
            }
        }
    }
    Ok(())
}

/// Pulls the mean and the spread of each line to the running medians over `span` lines. The gain stays from 0.5 to 2.
pub fn destripe(p: &mut [f32], w: usize, h: usize, span: usize) -> Result<(), ImageError> {
    check(p.len(), w, h)?;
    if w == 0 {
        return Ok(());
    }
    let stats: Vec<(f32, f32)> = (0..h)
        .map(|y| {
            let r = &p[y * w..(y + 1) * w];
            let m = r.iter().sum::<f32>() / w as f32;
            let s = (r.iter().map(|v| (v - m) * (v - m)).sum::<f32>() / w as f32).sqrt();
            (m, s)
        })
        .collect();
    let half = span / 2;
    let mut ms = Vec::with_capacity(span + 1);
    let mut ss = Vec::with_capacity(span + 1);
    for y in 0..h {
        let (lo, hi) = (y.saturating_sub(half), (y + half + 1).min(h));
        ms.clear();
        ss.clear();
        ms.extend(stats[lo..hi].iter().map(|s| s.0));
        ss.extend(stats[lo..hi].iter().map(|s| s.1));
        ms.sort_by(f32::total_cmp);
        ss.sort_by(f32::total_cmp);
        let (tm, ts) = (ms[ms.len() / 2], ss[ss.len() / 2]);
        let (m, s) = stats[y];
        let g = if s > 1e-3 {
            (ts / s).clamp(0.5, 2.0)
        } else {
            1.0
        };
        for v in p[y * w..(y + 1) * w].iter_mut() {
            *v = (*v - m) * g + tm;
        }
    }
    Ok(())
}

/// A 3 x 3 bilateral filter with a range spread of `sigma` grey levels.
pub fn bilateral(p: &mut [f32], w: usize, h: usize, sigma: f32) -> Result<(), ImageError> {
    check(p.len(), w, h)?;
    let src = p[..w * h].to_vec();
    let k = -1.0 / (2.0 * sigma * sigma);
    for y in 1..h.saturating_sub(1) {
        for x in 1..w.saturating_sub(1) {
            let c = src[y * w + x];
            let (mut acc, mut ws) = (0.0, 0.0);
            for dy in 0..3 {
                for dx in 0..3 {
                    let v = src[(y + dy - 1) * w + x + dx - 1];
                    let spatial = match (dx == 1, dy == 1) {
                        (true, true) => 1.0,
                        (true, false) | (false, true) => 0.6,
                        _ => 0.37,
                    };
                    let wt = spatial * ((v - c) * (v - c) * k).exp();
                    acc += wt * v;
                    ws += wt;
                }
            }
            p[y * w + x] = acc / ws;
        }
    }
    Ok(())
}

/// Percentile levels: the `lo` and `hi` points of the valid pixels go to 0 and 255, with a gain of at most `max_gain`. If fewer
/// than 16 pixels are valid, nothing changes.
pub fn levels(p: &mut [f32], valid: &[bool], lo: f32, hi: f32, max_gain: f32) {
    let mut v: Vec<f32> = p
        .iter()
        .zip(valid)
        .filter(|(_, &ok)| ok)
        .map(|(&x, _)| x)
        .collect();
    if v.len() < 16 {
        return;
    }
    v.sort_by(f32::total_cmp);
    let a = v[((v.len() - 1) as f32 * lo) as usize];
    let b = v[((v.len() - 1) as f32 * hi) as usize];
    let gain = (255.0 / (b - a).max(1e-3)).min(max_gain);
    for x in p.iter_mut() {
        *x = (*x - a) * gain;
    }
}

/// CLAHE on `tiles` x `tiles` tiles with the clip limit `clip` (in multiples of the mean histogram count), with bilinear
/// interpolation between the tiles.
pub fn clahe(p: &mut [u8], w: usize, h: usize, tiles: usize, clip: f32) -> Result<(), ImageError> {
    check(p.len(), w, h)?;
    if w == 0 || h == 0 || tiles == 0 {
        return Ok(());
    }
    let tw = w.div_ceil(tiles);
    let th = h.div_ceil(tiles);
    let mut luts = vec![[0u8; 256]; tiles * tiles];
    for ty in 0..tiles {
        for tx in 0..tiles {
            let mut hist = [0f32; 256];
            let mut n = 0.0;
            for y in ty * th..((ty + 1) * th).min(h) {
                for x in tx * tw..((tx + 1) * tw).min(w) {
                    hist[p[y * w + x] as usize] += 1.0;
                    n += 1.0;
                }
            }
            let lut = &mut luts[ty * tiles + tx];
            if n == 0.0 {
                *lut = std::array::from_fn(|i| i as u8);
                continue;
            }
            let limit = clip * n / 256.0;
            let mut excess = 0.0;
            for v in hist.iter_mut() {
                if *v > limit {
                    excess += *v - limit;
                    *v = limit;
                }
            }
            let add = excess / 256.0;
            let mut acc = 0.0;
            for (i, v) in hist.iter().enumerate() {
                acc += v + add;
                lut[i] = (acc / n * 255.0).round().clamp(0.0, 255.0) as u8;
            }
        }
    }
    let src = p[..w * h].to_vec();
    let last = (tiles - 1) as f32;
    for y in 0..h {
        let fy = (y as f32 / th as f32 - 0.5).clamp(0.0, last);
        let y0 = fy.floor() as usize;
        let y1 = (y0 + 1).min(tiles - 1);
        let ay = fy - y0 as f32;
        for x in 0..w {
            let fx = (x as f32 / tw as f32 - 0.5).clamp(0.0, last);
            let x0 = fx.floor() as usize;
            let x1 = (x0 + 1).min(tiles - 1);
            let ax = fx - x0 as f32;
            let v = src[y * w + x] as usize;
            let a =
                luts[y0 * tiles + x0][v] as f32 * (1.0 - ax) + luts[y0 * tiles + x1][v] as f32 * ax;
            let b =
                luts[y1 * tiles + x0][v] as f32 * (1.0 - ax) + luts[y1 * tiles + x1][v] as f32 * ax;
            p[y * w + x] = (a * (1.0 - ay) + b * ay).round() as u8;
        }
    }
    Ok(())
}

/// An unsharp mask of radius 1: each pixel moves away from its 3 x 3 mean by `amount` times the difference.
pub fn unsharp(p: &mut [f32], w: usize, h: usize, amount: f32) -> Result<(), ImageError> {
    check(p.len(), w, h)?;
    let src = p[..w * h].to_vec();
    for y in 1..h.saturating_sub(1) {
        for x in 1..w.saturating_sub(1) {
            let mut blur = 0.0;
            for dy in 0..3 {
                for dx in 0..3 {
                    blur += src[(y + dy - 1) * w + x + dx - 1];
                }
            }
            blur /= 9.0;
            let c = src[y * w + x];
            p[y * w + x] = c + amount * (c - blur);
        }
    }
    Ok(())
}
