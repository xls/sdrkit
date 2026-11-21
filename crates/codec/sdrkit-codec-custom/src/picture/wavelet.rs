//! The CDF 9/7 wavelet of the picture codec in lifting form, in binary32 with the stated order of operations.

const ALPHA: u32 = 0xBFCB_0673;
const BETA: u32 = 0xBD59_01AE;
const GAMMA: u32 = 0x3F62_0676;
const DELTA: u32 = 0x3EE3_1355;
const LOW_SCALE: u32 = 0x3F93_263D;
const HIGH_SCALE: u32 = 0x3F5E_AF70;

#[inline]
fn refl(i: isize, n: usize) -> usize {
    if i < 0 {
        (-i) as usize
    } else if i as usize >= n {
        2 * (n - 1) - i as usize
    } else {
        i as usize
    }
}

#[inline]
fn lift(x: &mut [f32], start: usize, k: f32, sub: bool) {
    let n = x.len();
    let mut i = start;
    while i < n {
        let s = x[refl(i as isize - 1, n)] + x[refl(i as isize + 1, n)];
        let t = k * s;
        x[i] = if sub { x[i] - t } else { x[i] + t };
        i += 2;
    }
}

/// The forward 1-D transform of `x` in place, then the reorder (low-pass samples first). `tmp` is scratch.
pub(crate) fn forward_1d(x: &mut [f32], tmp: &mut Vec<f32>) {
    let n = x.len();
    if n < 2 {
        return;
    }
    let f = f32::from_bits;
    lift(x, 1, f(ALPHA), false);
    lift(x, 0, f(BETA), false);
    lift(x, 1, f(GAMMA), false);
    lift(x, 0, f(DELTA), false);
    let (lo, hi) = (f(LOW_SCALE), f(HIGH_SCALE));
    for (i, v) in x.iter_mut().enumerate() {
        *v *= if i % 2 == 0 { lo } else { hi };
    }
    tmp.clear();
    tmp.extend(x.iter().step_by(2));
    tmp.extend(x.iter().skip(1).step_by(2));
    x.copy_from_slice(tmp);
}

/// The inverse of [`forward_1d`].
pub(crate) fn inverse_1d(x: &mut [f32], tmp: &mut Vec<f32>) {
    let n = x.len();
    if n < 2 {
        return;
    }
    let nl = n.div_ceil(2);
    tmp.clear();
    tmp.resize(n, 0.0);
    for i in 0..nl {
        tmp[2 * i] = x[i];
    }
    for i in 0..n - nl {
        tmp[2 * i + 1] = x[nl + i];
    }
    x.copy_from_slice(tmp);
    let f = f32::from_bits;
    let (lo, hi) = (f(LOW_SCALE), f(HIGH_SCALE));
    for (i, v) in x.iter_mut().enumerate() {
        *v /= if i % 2 == 0 { lo } else { hi };
    }
    lift(x, 0, f(DELTA), true);
    lift(x, 1, f(GAMMA), true);
    lift(x, 0, f(BETA), true);
    lift(x, 1, f(ALPHA), true);
}

fn column(
    p: &mut [f32],
    stride: usize,
    x: usize,
    h: usize,
    line: &mut Vec<f32>,
    tmp: &mut Vec<f32>,
    inverse: bool,
) {
    line.clear();
    line.extend((0..h).map(|y| p[y * stride + x]));
    if inverse {
        inverse_1d(line, tmp);
    } else {
        forward_1d(line, tmp);
    }
    for (y, &v) in line.iter().enumerate() {
        p[y * stride + x] = v;
    }
}

/// The forward 2-D transform of a plane of `pw` x `ph` with `levels` levels: rows then columns at each level.
pub(crate) fn forward_2d(
    p: &mut [f32],
    pw: usize,
    ph: usize,
    levels: u32,
    col: &mut Vec<f32>,
    tmp: &mut Vec<f32>,
) {
    let (mut w, mut h) = (pw, ph);
    for _ in 0..levels {
        for y in 0..h {
            forward_1d(&mut p[y * pw..y * pw + w], tmp);
        }
        for x in 0..w {
            column(p, pw, x, h, col, tmp, false);
        }
        w = w.div_ceil(2);
        h = h.div_ceil(2);
    }
}

/// The inverse 2-D transform: columns then rows at each level, from the last level down.
pub(crate) fn inverse_2d(
    p: &mut [f32],
    pw: usize,
    ph: usize,
    levels: u32,
    col: &mut Vec<f32>,
    tmp: &mut Vec<f32>,
) {
    let mut ws = [0usize; 9];
    let mut hs = [0usize; 9];
    ws[0] = pw;
    hs[0] = ph;
    for l in 1..=levels as usize {
        ws[l] = ws[l - 1].div_ceil(2);
        hs[l] = hs[l - 1].div_ceil(2);
    }
    for l in (0..levels as usize).rev() {
        let (w, h) = (ws[l], hs[l]);
        for x in 0..w {
            column(p, pw, x, h, col, tmp, true);
        }
        for y in 0..h {
            inverse_1d(&mut p[y * pw..y * pw + w], tmp);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inverse_undoes_forward() {
        let mut tmp = Vec::new();
        for n in [1usize, 2, 3, 7, 16, 33] {
            let x: Vec<f32> = (0..n).map(|i| ((i * 37) % 23) as f32 - 11.0).collect();
            let mut y = x.clone();
            forward_1d(&mut y, &mut tmp);
            inverse_1d(&mut y, &mut tmp);
            for i in 0..n {
                assert!((x[i] - y[i]).abs() < 1e-3, "n {n}");
            }
        }
    }
}
