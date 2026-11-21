//! Voice codec 4 linear prediction: LSF stabilisation, LSF to LPC, bandwidth weighting, LPC analysis and LPC to LSF.

use super::math::{cos, PI};
use super::tables::LAG_WINDOW;

/// The LPC order.
pub(crate) const ORDER: usize = 10;

/// LPC coefficients a[0..=10] with a[0] = 1.
pub(crate) type Lpc = [f32; ORDER + 1];

/// Sorts `q` and keeps a gap of at least `g` between neighbours and to 0 and pi (section 3.5.2).
pub(crate) fn stabilise(q: &mut [f64; ORDER], g: f64) {
    for i in 1..ORDER {
        let v = q[i];
        let mut j = i;
        while j > 0 && v < q[j - 1] {
            q[j] = q[j - 1];
            j -= 1;
        }
        q[j] = v;
    }
    if q[0] < g {
        q[0] = g;
    }
    for i in 1..ORDER {
        if q[i] < q[i - 1] + g {
            q[i] = q[i - 1] + g;
        }
    }
    if q[9] > PI - g {
        q[9] = PI - g;
    }
    for i in (0..ORDER - 1).rev() {
        if q[i] > q[i + 1] - g {
            q[i] = q[i + 1] - g;
        }
    }
}

/// Interpolates two LSF vectors: (1 - w) a + w b.
pub(crate) fn interp(a: &[f64; ORDER], b: &[f64; ORDER], w: f64) -> [f64; ORDER] {
    let mut o = [0f64; ORDER];
    for i in 0..ORDER {
        o[i] = (1.0 - w) * a[i] + w * b[i];
    }
    o
}

/// Converts an LSF vector to LPC coefficients (section 3.5.3 step 3).
pub(crate) fn lsf_to_lpc(lsf: &[f64; ORDER]) -> Lpc {
    let mut p = [0f64; 12];
    let mut q = [0f64; 12];
    p[0] = 1.0;
    q[0] = 1.0;
    for i in 0..5 {
        for (f, w) in [(&mut p, lsf[2 * i]), (&mut q, lsf[2 * i + 1])] {
            let c = -2.0 * cos(w);
            for j in (1..=2 * (i + 1)).rev() {
                let mut v = f[j] + c * f[j - 1];
                if j >= 2 {
                    v += f[j - 2];
                }
                f[j] = v;
            }
        }
    }
    let mut a = [0f32; ORDER + 1];
    a[0] = 1.0;
    for j in 1..=ORDER {
        let pj = p[j] + p[j - 1];
        let qj = q[j] - q[j - 1];
        a[j] = (0.5 * (pj + qj)) as f32;
    }
    a
}

/// Bandwidth weighting: out[i] = a[i] g^i (section 3.5.3 step 4).
pub(crate) fn weight(a: &Lpc, g: f64) -> Lpc {
    let mut o = [0f32; ORDER + 1];
    let mut gi = 1.0f64;
    for i in 0..=ORDER {
        o[i] = (a[i] as f64 * gi) as f32;
        gi *= g;
    }
    o
}

/// The all-pole filter 1/A(z) on one sample with memory `mem` (newest first).
#[inline]
pub(crate) fn synth_step(a: &Lpc, mem: &mut [f32; ORDER], x: f32) -> f32 {
    let mut s = x;
    for k in 1..=ORDER {
        s -= a[k] * mem[k - 1];
    }
    for k in (1..ORDER).rev() {
        mem[k] = mem[k - 1];
    }
    mem[0] = s;
    s
}

/// The FIR filter A(z) on one sample with memory `mem` (past inputs, newest first).
#[inline]
pub(crate) fn fir_step(a: &Lpc, mem: &mut [f32; ORDER], x: f32) -> f32 {
    let mut s = x;
    for k in 1..=ORDER {
        s += a[k] * mem[k - 1];
    }
    for k in (1..ORDER).rev() {
        mem[k] = mem[k - 1];
    }
    mem[0] = x;
    s
}

/// The LPC analysis of 280 samples into an LSF vector (section 3.8.3 steps 1 to 6). Returns `None` when it fails.
pub(crate) fn analyse(x: &[f32], window: &[f32; 280]) -> Option<[f64; ORDER]> {
    let mut xw = [0f32; 280];
    for i in 0..280 {
        xw[i] = x[i] * window[i];
    }
    let mut r = [0f64; ORDER + 1];
    for (k, rk) in r.iter_mut().enumerate() {
        let mut acc = 0f64;
        for i in k..280 {
            acc += xw[i] as f64 * xw[i - k] as f64;
        }
        *rk = acc;
    }
    r[0] = r[0] * 1.0001 + 1e-3;
    for k in 1..=ORDER {
        r[k] *= LAG_WINDOW[k];
    }
    let a = levinson(&r)?;
    lpc_to_lsf(&a)
}

fn levinson(r: &[f64; ORDER + 1]) -> Option<[f64; ORDER + 1]> {
    let mut a = [0f64; ORDER + 1];
    a[0] = 1.0;
    let mut err = r[0];
    if !(err > 0.0) {
        return None;
    }
    for i in 1..=ORDER {
        let mut acc = r[i];
        for j in 1..i {
            acc += a[j] * r[i - j];
        }
        let k = -acc / err;
        if !(k > -0.9995 && k < 0.9995) {
            return None;
        }
        let mut na = a;
        for j in 1..i {
            na[j] = a[j] + k * a[i - j];
        }
        na[i] = k;
        a = na;
        err *= 1.0 - k * k;
        if !(err > 0.0) {
            return None;
        }
    }
    Some(a)
}

fn cheb(f: &[f64; 6], x: f64) -> f64 {
    let c = [f[5] / 2.0, f[4], f[3], f[2], f[1], f[0]];
    let mut b1 = 0.0;
    let mut b2 = 0.0;
    for k in (1..=5).rev() {
        let b0 = 2.0 * x * b1 - b2 + c[k];
        b2 = b1;
        b1 = b0;
    }
    x * b1 - b2 + c[0]
}

fn lpc_to_lsf(a: &[f64; ORDER + 1]) -> Option<[f64; ORDER]> {
    let mut f1 = [0f64; 6];
    let mut f2 = [0f64; 6];
    f1[0] = 1.0;
    f2[0] = 1.0;
    for i in 0..5 {
        f1[i + 1] = a[i + 1] + a[10 - i] - f1[i];
        f2[i + 1] = a[i + 1] - a[10 - i] + f2[i];
    }
    let mut lsf = [0f64; ORDER];
    for (which, f) in [&f1, &f2].into_iter().enumerate() {
        let mut roots = 0;
        let mut prev_w = 0.0;
        let mut prev_v = cheb(f, cos(0.0));
        for i in 1..=200 {
            let w = PI * i as f64 / 200.0;
            let v = cheb(f, cos(w));
            if roots < 5 && ((prev_v <= 0.0 && v > 0.0) || (prev_v >= 0.0 && v < 0.0)) {
                let (mut lo, mut hi) = (prev_w, w);
                let mut vlo = prev_v;
                for _ in 0..10 {
                    let mid = (lo + hi) / 2.0;
                    let vm = cheb(f, cos(mid));
                    if (vm <= 0.0) == (vlo <= 0.0) {
                        lo = mid;
                        vlo = vm;
                    } else {
                        hi = mid;
                    }
                }
                lsf[2 * roots + which] = (lo + hi) / 2.0;
                roots += 1;
            }
            prev_w = w;
            prev_v = v;
        }
        if roots != 5 {
            return None;
        }
    }
    for i in 1..ORDER {
        if !(lsf[i] > lsf[i - 1]) {
            return None;
        }
    }
    Some(lsf)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lsf_round_trip_through_lpc() {
        let lsf = super::super::tables::LSF_MEAN;
        let a = lsf_to_lpc(&lsf);
        let mut ad = [0f64; ORDER + 1];
        for i in 0..=ORDER {
            ad[i] = a[i] as f64;
        }
        let back = lpc_to_lsf(&ad).unwrap();
        for i in 0..ORDER {
            assert!(
                (back[i] - lsf[i]).abs() < 0.01,
                "{i} {} {}",
                back[i],
                lsf[i]
            );
        }
    }

    #[test]
    fn stabilise_sorts_and_spaces() {
        let mut q = [3.2, 0.0, 0.5, 0.5, 1.0, 1.2, 1.4, 1.6, 1.8, 2.0];
        stabilise(&mut q, 0.025);
        for i in 1..ORDER {
            assert!(q[i] - q[i - 1] >= 0.025 - 1e-12);
        }
        assert!(q[9] <= PI - 0.025 + 1e-12 && q[0] >= 0.025 - 1e-12);
    }
}
