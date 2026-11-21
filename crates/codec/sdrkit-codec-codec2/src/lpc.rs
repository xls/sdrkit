//! The LPC model of modes 3200, 1300 and 1200: the LPC analysis of the encoder, the pitch, energy and LSP quantisers, and the
//! decoder step that turns LSPs and an energy into harmonic amplitudes and phases.
//!
//! Standard: D. Rowe, "Codec 2 Algorithm Description" (2025), sections 3.7 and 4.

use crate::analysis::Model;
use crate::consts::{
    log10f, pow10d, powf, trunc_i, wo_max, wo_min, FFT_SIZE, LPC_ORD, MAX_AMP, M_PITCH, PI, TWO_PI,
};
use sdrkit_algo_dsp::fft_exact::ExactRealFft;
use sdrkit_algo_dsp::lpc::{
    autocorrelate, bandwidth_expand, levinson_durbin, lpc_to_lsp, lsp_to_lpc, LspSearch,
};
use sdrkit_algo_dsp::Complex32;

/// The LSPs of a flat spectrum, used when the analysis finds no usable LSPs.
fn flat_lsps(lsp: &mut [f32; LPC_ORD]) {
    for (i, v) in lsp.iter_mut().enumerate() {
        *v = (PI / LPC_ORD as f64 * i as f64) as f32;
    }
}

/// Runs the LPC analysis on the windowed buffer `sn w`. Writes the LSPs and returns the LPC energy.
pub(crate) fn analyse(sn: &[f32; M_PITCH], w: &[f32; M_PITCH], lsp: &mut [f32; LPC_ORD]) -> f32 {
    let mut wn = [0.0f32; M_PITCH];
    let mut e = 0.0f32;
    for i in 0..M_PITCH {
        wn[i] = sn[i] * w[i];
        e += wn[i] * wn[i];
    }
    if e == 0.0 {
        flat_lsps(lsp);
        return 0.0;
    }
    let mut r = [0.0f32; LPC_ORD + 1];
    let mut ak = [0.0f32; LPC_ORD + 1];
    if autocorrelate(&wn, &mut r).is_err() || levinson_durbin(&r, &mut ak).is_err() {
        flat_lsps(lsp);
        return 0.0;
    }
    let mut energy = 0.0f32;
    for i in 0..=LPC_ORD {
        energy += ak[i] * r[i];
    }
    bandwidth_expand(&mut ak, 0.994);
    match lpc_to_lsp(&ak, lsp, LspSearch::default()) {
        Ok(n) if n == LPC_ORD => {}
        _ => flat_lsps(lsp),
    }
    energy
}

/// `floorf` of an `f64` expression rounded to `f32`, as an integer.
fn floor_d(x: f64) -> i64 {
    (x as f32).floor() as i64
}

/// The 7-bit linear pitch index.
pub(crate) fn encode_wo(wo: f32) -> u32 {
    let (lo, hi) = (wo_min(), wo_max());
    let norm = (wo - lo) / (hi - lo);
    floor_d((128.0 * norm) as f64 + 0.5).clamp(0, 127) as u32
}

/// The pitch of a 7-bit linear pitch index.
pub(crate) fn decode_wo(index: u32) -> f32 {
    let (lo, hi) = (wo_min(), wo_max());
    let step = (hi - lo) / 128.0;
    lo + step * index as f32
}

/// The 5-bit energy index.
pub(crate) fn encode_energy(e: f32) -> u32 {
    let db = (10.0 * log10f(e) as f64) as f32;
    let norm = (db - -10.0) / (40.0 - -10.0);
    floor_d((32.0 * norm) as f64 + 0.5).clamp(0, 31) as u32
}

/// The energy of a 5-bit energy index.
pub(crate) fn decode_energy(index: u32) -> f32 {
    let step = (40.0f32 - -10.0) / 32.0;
    let db = -10.0 + step * index as f32;
    pow10d(db as f64 / 10.0)
}

/// The entry of `table` nearest to `x`. On a tie the lower index wins.
fn nearest(table: &[f32], x: f32) -> usize {
    let mut best = 0;
    let mut best_e = 1e32f64 as f32;
    for (i, &v) in table.iter().enumerate() {
        let d = v - x;
        let e = d * 1.0 * d * 1.0;
        if e < best_e {
            best_e = e;
            best = i;
        }
    }
    best
}

/// The ten scalar LSP tables of a mode.
pub(crate) type LspTables = [Box<[f32]>; LPC_ORD];

/// The 3200 LSP difference quantiser: writes the ten indexes.
pub(crate) fn encode_lsp_delta(
    lsp: &[f32; LPC_ORD],
    tables: &LspTables,
    index: &mut [u32; LPC_ORD],
) {
    let mut prev = 0.0f32;
    for i in 0..LPC_ORD {
        let f = ((4000.0 / PI) * lsp[i] as f64) as f32;
        let d = if i == 0 { f } else { f - prev };
        let t = &tables[i];
        let k = nearest(t, d);
        index[i] = k as u32;
        prev = if i == 0 { t[k] } else { prev + t[k] };
    }
}

/// The LSPs of ten 3200 LSP difference indexes.
pub(crate) fn decode_lsp_delta(
    index: &[u32; LPC_ORD],
    tables: &LspTables,
    lsp: &mut [f32; LPC_ORD],
) {
    let mut f = 0.0f32;
    for i in 0..LPC_ORD {
        let t = &tables[i];
        f += t[(index[i] as usize).min(t.len() - 1)];
        lsp[i] = ((PI / 4000.0) * f as f64) as f32;
    }
}

/// The 1300 scalar LSP quantiser: writes the ten indexes.
pub(crate) fn encode_lsp_scalar(
    lsp: &[f32; LPC_ORD],
    tables: &LspTables,
    index: &mut [u32; LPC_ORD],
) {
    for i in 0..LPC_ORD {
        let f = ((4000.0 / PI) * lsp[i] as f64) as f32;
        index[i] = nearest(&tables[i], f) as u32;
    }
}

/// The LSPs of ten 1300 scalar LSP indexes.
pub(crate) fn decode_lsp_scalar(
    index: &[u32; LPC_ORD],
    tables: &LspTables,
    lsp: &mut [f32; LPC_ORD],
) {
    for i in 0..LPC_ORD {
        let t = &tables[i];
        lsp[i] = ((PI / 4000.0) * t[(index[i] as usize).min(t.len() - 1)] as f64) as f32;
    }
}

/// The weighted squared distance of one entry: the sum of `(w (e - c)) (e - c)`, with the product in brackets rounded first.
fn weighted(e: &[f32], c: &[f32], w: &[f32]) -> f32 {
    let mut dist = 0.0f32;
    for i in 0..e.len() {
        let d = e[i] - c[i];
        dist += w[i] * d * d;
    }
    dist
}

/// The nearest entry of `table` (dimension `e.len()`) to `e` with weights `w`. On a tie the lower index wins.
fn nearest_weighted(table: &[f32], e: &[f32], w: &[f32]) -> usize {
    let k = e.len();
    let mut best = 0;
    let mut best_e = 1e15f64 as f32;
    for (n, c) in table.chunks_exact(k).enumerate() {
        let d = weighted(e, c, w);
        if d < best_e {
            best_e = d;
            best = n;
        }
    }
    best
}

/// The 1200 LSP vector quantiser: writes the three indexes.
pub(crate) fn encode_lsp_vq(x: &[f32; LPC_ORD], cb: &[Box<[f32]>; 3], index: &mut [u32; 3]) {
    let mut d = [0.0f32; LPC_ORD];
    d[0] = x[0].min(x[1] - x[0]);
    for i in 1..LPC_ORD - 1 {
        d[i] = (x[i] - x[i - 1]).min(x[i + 1] - x[i]);
    }
    let a = (x[9] - x[8]) as f64;
    let b = PI - x[9] as f64;
    d[9] = (if a < b { a } else { b }) as f32;
    let mut w = [0.0f32; LPC_ORD];
    for i in 0..LPC_ORD {
        w[i] = (1.0 / (0.01 + d[i] as f64)) as f32;
    }
    let mut n1 = 0;
    let mut best = 1e15f64 as f32;
    for (n, c) in cb[0].as_chunks::<LPC_ORD>().0.iter().enumerate() {
        let mut dist = 0.0f32;
        for i in 0..LPC_ORD {
            let e = x[i] - c[i];
            dist += e * e;
        }
        if dist < best {
            best = dist;
            n1 = n;
        }
    }
    let c1 = &cb[0][n1 * LPC_ORD..(n1 + 1) * LPC_ORD];
    let mut err = [0.0f32; LPC_ORD];
    for i in 0..LPC_ORD {
        err[i] = x[i] - c1[i];
    }
    let (mut ee, mut eo, mut we, mut wo) = ([0.0f32; 5], [0.0f32; 5], [0.0f32; 5], [0.0f32; 5]);
    for i in 0..5 {
        ee[i] = err[2 * i];
        eo[i] = err[2 * i + 1];
        we[i] = w[2 * i];
        wo[i] = w[2 * i + 1];
    }
    index[0] = n1 as u32;
    index[1] = nearest_weighted(&cb[1], &ee, &we) as u32;
    index[2] = nearest_weighted(&cb[2], &eo, &wo) as u32;
}

/// The LSPs of three 1200 LSP vector quantiser indexes.
pub(crate) fn decode_lsp_vq(index: &[u32; 3], cb: &[Box<[f32]>; 3], lsp: &mut [f32; LPC_ORD]) {
    let entry = |t: &[f32], k: usize, n: u32| {
        let n = (n as usize).min(t.len() / k - 1);
        n * k
    };
    let o0 = entry(&cb[0], LPC_ORD, index[0]);
    lsp.copy_from_slice(&cb[0][o0..o0 + LPC_ORD]);
    let o1 = entry(&cb[1], 5, index[1]);
    let o2 = entry(&cb[2], 5, index[2]);
    for i in 0..5 {
        lsp[2 * i] += cb[1][o1 + i];
        lsp[2 * i + 1] += cb[2][o2 + i];
    }
}

/// The predictor coefficients of the 1200 pitch and energy quantiser.
const WOE_C: [f32; 2] = [0.8, 0.9];

/// The 1200 joint pitch and energy quantiser. It keeps the predictor state of one encoder or one decoder.
#[derive(Clone)]
pub(crate) struct WoE {
    xq: [f32; 2],
}

impl WoE {
    pub fn new() -> Self {
        WoE { xq: [0.0; 2] }
    }

    /// The 8-bit index of a pitch and an energy.
    pub fn encode(&mut self, cb: &[f32], wo: f32, e: f32) -> u32 {
        let e = if e < 0.0 { 0.0 } else { e };
        let x0 = log10f(((wo as f64 / PI) * 4000.0 / 50.0) as f32) / log10f(2.0);
        let x1 = (10.0 * log10f((1e-4 + e as f64) as f32) as f64) as f32;
        let x = [x0, x1];
        let (mut w0, mut w1) = (30.0f32, 1.0f32);
        if x1 < 0.0 {
            w0 = (w0 as f64 * 0.6) as f32;
            w1 = (w1 as f64 * 0.3) as f32;
        }
        if x1 < -10.0 {
            w0 = (w0 as f64 * 0.3) as f32;
            w1 = (w1 as f64 * 0.3) as f32;
        }
        let dx = (x0 - self.xq[0]).abs() as f64;
        if dx < 0.2 {
            w0 *= 2.0;
            w1 = (w1 as f64 * 1.5) as f32;
        } else if dx > 0.5 {
            w0 = (w0 as f64 * 0.5) as f32;
        }
        if x1 < self.xq[1] - 10.0 {
            w1 = (w1 as f64 * 0.5) as f32;
        }
        if x1 < self.xq[1] - 20.0 {
            w1 = (w1 as f64 * 0.5) as f32;
        }
        let w = [w0 * w0, w1 * w1];
        let mut err = [0.0f32; 2];
        for i in 0..2 {
            err[i] = x[i] - WOE_C[i] * self.xq[i];
        }
        let best = nearest_weighted(cb, &err, &w);
        for i in 0..2 {
            self.xq[i] = WOE_C[i] * self.xq[i] + cb[2 * best + i];
        }
        best as u32
    }

    /// The pitch and the energy of an 8-bit index.
    pub fn decode(&mut self, cb: &[f32], index: u32) -> (f32, f32) {
        let n = (index as usize).min(cb.len() / 2 - 1);
        for i in 0..2 {
            self.xq[i] = WOE_C[i] * self.xq[i] + cb[2 * n + i];
        }
        let mut wo = (powf(2.0, self.xq[0]) as f64 * (PI * 50.0) / 4000.0) as f32;
        if wo < wo_min() {
            wo = wo_min();
        }
        if wo > wo_max() {
            wo = wo_max();
        }
        (wo, pow10d(self.xq[1] as f64 / 10.0))
    }
}

/// The 1300 and 1200 decoder repair of the LSPs: it swaps pairs that are out of order and then makes sure that neighbours are
/// at least 50 Hz (LSPs 0 to 3) or 100 Hz (LSPs 4 to 9) apart.
pub(crate) fn order_and_widen(lsp: &mut [f32; LPC_ORD]) {
    // A guard on the number of swaps: corrupt input must not make the scan run without end.
    let mut swaps = 0;
    let mut i = 1;
    while i < LPC_ORD {
        if lsp[i] < lsp[i - 1] && swaps < 1000 {
            swaps += 1;
            let (lo, hi) = (lsp[i - 1], lsp[i]);
            lsp[i - 1] = (hi as f64 - 0.1) as f32;
            lsp[i] = (lo as f64 + 0.1) as f32;
            i = 1;
        }
        i += 1;
    }
    for i in 1..LPC_ORD {
        let min = if i < 4 { 50.0 } else { 100.0 } * (PI / 4000.0);
        if ((lsp[i] - lsp[i - 1]) as f64) < min {
            lsp[i] = (lsp[i - 1] as f64 + min) as f32;
        }
    }
}

/// The decoder step from an LPC model to harmonic amplitudes and phases.
pub(crate) struct LpcSynth {
    fft: ExactRealFft,
    x: [f32; FFT_SIZE],
    aw: Box<[Complex32]>,
    ww: Box<[Complex32]>,
    pw: Box<[f32]>,
}

impl LpcSynth {
    pub fn new() -> Self {
        LpcSynth {
            fft: ExactRealFft::new(FFT_SIZE, false).expect("512 is a valid FFT size"),
            x: [0.0; FFT_SIZE],
            aw: vec![Complex32::new(0.0, 0.0); FFT_SIZE / 2 + 1].into_boxed_slice(),
            ww: vec![Complex32::new(0.0, 0.0); FFT_SIZE / 2 + 1].into_boxed_slice(),
            pw: vec![0.0; FFT_SIZE / 2].into_boxed_slice(),
        }
    }

    /// Writes `model.a[1..=l]` and the phase vector `h[1..=l]` from the LSPs and the energy `e`. `model.wo` and `model.l` are
    /// set on entry.
    pub fn run(
        &mut self,
        lsp: &[f32; LPC_ORD],
        e: f32,
        model: &mut Model,
        h: &mut [Complex32; MAX_AMP + 1],
    ) {
        let mut ak = [0.0f32; LPC_ORD + 1];
        if lsp_to_lpc(lsp, &mut ak).is_err() {
            ak = [0.0; LPC_ORD + 1];
            ak[0] = 1.0;
        }
        let half = FFT_SIZE / 2;

        self.x.fill(0.0);
        self.x[..=LPC_ORD].copy_from_slice(&ak);
        let _ = self.fft.forward(&self.x, &mut self.aw);
        self.x.fill(0.0);
        self.x[0] = ak[0];
        let mut c = 0.5f32;
        for i in 1..=LPC_ORD {
            self.x[i] = ak[i] * c;
            c *= 0.5;
        }
        let _ = self.fft.forward(&self.x, &mut self.ww);

        let mut e_before = 1e-4f64 as f32;
        let mut e_after = 1e-4f64 as f32;
        for i in 0..half {
            let a = self.aw[i];
            let pw = (1.0 / ((a.re * a.re + a.im * a.im) as f64 + 1e-6)) as f32;
            let wv = self.ww[i];
            let g = wv.re * wv.re + wv.im * wv.im;
            let rw = (g * pw).sqrt();
            e_before += pw;
            let t = powf(rw, 0.2);
            let pw2 = pw * (t * t);
            e_after += pw2;
            self.pw[i] = pw2;
        }
        let gain = e_before / e_after * e;
        for (i, p) in self.pw.iter_mut().enumerate() {
            *p *= gain;
            if i < half / 4 {
                *p = (*p as f64 * (1.4 * 1.4)) as f32;
            }
        }

        let r = (TWO_PI / FFT_SIZE as f64) as f32;
        let wo = model.wo;
        for m in 1..=model.l {
            let am = trunc_i((m as f64 - 0.5) * wo as f64 / r as f64 + 0.5).max(0) as usize;
            let bm =
                (trunc_i((m as f64 + 0.5) * wo as f64 / r as f64 + 0.5).max(0) as usize).min(half);
            let mut s = 0.0f32;
            for p in &self.pw[am.min(bm)..bm] {
                s += p;
            }
            model.a[m] = s.sqrt();
        }
        for a in &mut model.a[model.l + 1..] {
            *a = 0.0;
        }
        if (wo as f64) < PI * 150.0 / 4000.0 {
            model.a[1] = (model.a[1] as f64 * 0.032) as f32;
        }
        for m in 1..=model.l {
            let b = (trunc_i((m as f32 * wo / r) as f64 + 0.5).max(0) as usize).min(half);
            h[m] = self.aw[b].conj();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pitch_index_round_trip() {
        for i in 0..128 {
            assert_eq!(encode_wo(decode_wo(i)), i);
        }
        assert_eq!(encode_wo(0.0), 0);
        assert_eq!(encode_wo(10.0), 127);
    }

    #[test]
    fn energy_index_round_trip_and_zero() {
        for i in 0..32 {
            assert_eq!(encode_energy(decode_energy(i)), i);
        }
        assert_eq!(encode_energy(0.0), 0);
        assert_eq!(encode_energy(1e9), 31);
    }

    #[test]
    fn order_check_swaps_and_widens() {
        let mut lsp = [0.1f32, 0.3, 0.2, 0.5, 0.8, 1.0, 1.4, 1.8, 2.2, 2.6];
        order_and_widen(&mut lsp);
        assert!(lsp.windows(2).all(|w| w[1] > w[0]));
        let min100 = (100.0 * PI / 4000.0) as f32;
        assert!(lsp[4..].windows(2).all(|w| w[1] - w[0] >= min100 - 1e-6));
    }
}
