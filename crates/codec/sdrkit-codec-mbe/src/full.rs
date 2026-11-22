//! Full-rate frame (TIA-102.BABA chapters 6 and 7): quantiser values b0..b_{L+2}, the priority scan into u0..u7, the
//! quantisers and the amplitude reconstruction.

use crate::fec::{self, codes, FULL_INFO, FULL_WORDS};
use crate::model::{band_of, bands_for, dct, idct, predict, Params, MAX_L, MIN_L};
use crate::tables::full::{BLOCKS, GAIN_ALLOC, GAIN_LEVELS, HOC_BITS, HOC_OFFSET};
use crate::tables::{HOC_SIGMA, HOC_STEP};
use std::f64::consts::PI;

/// Speech bits per frame.
pub const SPEECH_BITS: usize = 88;
/// Channel bits per frame.
pub const FRAME_BITS: usize = 144;
/// The largest valid b0.
pub const B0_MAX: u32 = 207;
/// Quantiser values b0..b_{L+2}.
pub const MAX_VALUES: usize = MAX_L + 3;

/// The quantiser values of one frame: `b[m]` for `m = 0..=l + 2`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Values {
    /// The values.
    pub b: [u32; MAX_VALUES],
}

impl Default for Values {
    fn default() -> Self {
        Values { b: [0; MAX_VALUES] }
    }
}

/// omega_0 for b0 (Equation 46).
pub fn w0_of(b0: u32) -> f64 {
    4.0 * PI / (b0 as f64 + 39.5)
}

/// The number of harmonics for omega_0 (Equations 31 and 47).
pub fn l_of(w0: f64) -> usize {
    (0.9254 * (PI / w0 + 0.25).floor()).floor() as usize
}

/// b0 for omega_0 (Equation 45), limited to 0..=207.
pub fn b0_of(w0: f64) -> u32 {
    (4.0 * PI / w0 - 39.0).floor().clamp(0.0, B0_MAX as f64) as u32
}

/// Bits of `b_m` for `m = 3..=l + 1`; 0 elsewhere.
pub fn bits_of(l: usize, out: &mut [u8; MAX_VALUES]) {
    *out = [0; MAX_VALUES];
    let row = l - MIN_L;
    for (i, &(b, _)) in GAIN_ALLOC[row].iter().enumerate() {
        out[3 + i] = b;
    }
    let hoc = &HOC_BITS[HOC_OFFSET[row] as usize..HOC_OFFSET[row + 1] as usize];
    for (i, &b) in hoc.iter().enumerate() {
        out[8 + i] = b;
    }
}

/// The priority scan (section 7.1, Figure 22): the bits of b3..b_{L+1}, highest bit plane first and left to right in a
/// plane, as (m, bit). Returns the count, 73 - K for every valid `l`.
pub fn scan(l: usize, out: &mut [(u8, u8); 80]) -> usize {
    let mut bits = [0u8; MAX_VALUES];
    bits_of(l, &mut bits);
    let mut n = 0;
    for plane in (0..=10u8).rev() {
        for m in 3..=l + 1 {
            if bits[m] > plane && n < out.len() {
                out[n] = (m as u8, plane);
                n += 1;
            }
        }
    }
    n
}

/// The 88 speech bits u0..u7 (each vector most significant bit first) of the values `v` for `l` harmonics.
pub fn pack(v: &Values, l: usize, out: &mut [u8; SPEECH_BITS]) {
    let k = bands_for(l);
    let mut order = [(0u8, 0u8); 80];
    let n = scan(l, &mut order);
    let bit = |m: usize, i: u8| (v.b[m] >> i & 1) as u8;
    let mut p = 0;
    let mut put = |x: u8| {
        out[p] = x;
        p += 1;
    };
    for i in (2..=7).rev() {
        put(bit(0, i));
    }
    for i in (3..=5).rev() {
        put(bit(2, i));
    }
    for &(m, i) in &order[..39.min(n)] {
        put(bit(m as usize, i));
    }
    for i in (0..k as u8).rev() {
        put(bit(1, i));
    }
    put(bit(2, 2));
    put(bit(2, 1));
    for &(m, i) in &order[39.min(n)..n] {
        put(bit(m as usize, i));
    }
    put(bit(2, 0));
    put(bit(0, 1));
    put(bit(0, 0));
    put(bit(l + 2, 0));
}

/// b0 from the 88 speech bits.
pub fn b0_from(bits: &[u8; SPEECH_BITS]) -> u32 {
    fec::word_of(&bits[..6]) << 2 | (bits[85] as u32) << 1 | bits[86] as u32
}

/// The inverse of [`pack`] for a valid b0 (`l` from b0).
pub fn unpack(bits: &[u8; SPEECH_BITS], l: usize, v: &mut Values) {
    *v = Values::default();
    let k = bands_for(l);
    let mut order = [(0u8, 0u8); 80];
    let n = scan(l, &mut order);
    let mut p = 0;
    let mut get = || {
        let x = bits[p] as u32 & 1;
        p += 1;
        x
    };
    let set = |v: &mut Values, m: usize, i: u8, x: u32| v.b[m] |= x << i;
    for i in (2..=7).rev() {
        let x = get();
        set(v, 0, i, x);
    }
    for i in (3..=5).rev() {
        let x = get();
        set(v, 2, i, x);
    }
    for &(m, i) in &order[..39.min(n)] {
        let x = get();
        set(v, m as usize, i, x);
    }
    for i in (0..k as u8).rev() {
        let x = get();
        set(v, 1, i, x);
    }
    let x = get();
    set(v, 2, 2, x);
    let x = get();
    set(v, 2, 1, x);
    for &(m, i) in &order[39.min(n)..n] {
        let x = get();
        set(v, m as usize, i, x);
    }
    let x = get();
    set(v, 2, 0, x);
    let x = get();
    set(v, 0, 1, x);
    let x = get();
    set(v, 0, 0, x);
    let x = get();
    set(v, l + 2, 0, x);
}

/// The 88 speech bits as the vectors u0..u7.
pub fn to_vectors(bits: &[u8; SPEECH_BITS], u: &mut [u32; 8]) {
    let mut p = 0;
    for (i, &n) in FULL_INFO.iter().enumerate() {
        u[i] = fec::word_of(&bits[p..p + n]);
        p += n;
    }
}

/// The inverse of [`to_vectors`].
pub fn from_vectors(u: &[u32; 8], bits: &mut [u8; SPEECH_BITS]) {
    let mut p = 0;
    for (i, &n) in FULL_INFO.iter().enumerate() {
        fec::put_word(u[i], &mut bits[p..p + n]);
        p += n;
    }
}

/// Error counts of the seven protected words of a frame.
pub type Errors = [u8; 7];

/// FEC-decodes the 144 channel bits into the speech bits and the error count of each word (sections 7.3 to 7.5).
pub fn decode_channel(chan: &[u8], bits: &mut [u8; SPEECH_BITS], eps: &mut Errors) {
    let c = codes();
    let mut words = [0u8; FRAME_BITS];
    let mut cw = [0u32; 8];
    if c.full_layout.split(&chan[..FRAME_BITS], &mut words).is_err() {
        return;
    }
    let mut p = 0;
    for (i, &n) in FULL_WORDS.iter().enumerate() {
        cw[i] = fec::word_of(&words[p..p + n]);
        p += n;
    }
    let mut u = [0u32; 8];
    let (u0, e0) = fec::decode_word(&c.golay23, cw[0]);
    u[0] = u0;
    eps[0] = e0;
    let m = modulation_words(u0);
    for i in 1..7 {
        let code = if i < 4 { &c.golay23 } else { &c.hamming };
        let (ui, ei) = fec::decode_word(code, cw[i] ^ m[i]);
        u[i] = ui;
        eps[i] = ei;
    }
    u[7] = cw[7];
    from_vectors(&u, bits);
}

/// The 144 channel bits of the speech bits (sections 7.3 to 7.5).
pub fn encode_channel(bits: &[u8; SPEECH_BITS], chan: &mut [u8]) {
    let c = codes();
    let mut u = [0u32; 8];
    to_vectors(bits, &mut u);
    let m = modulation_words(u[0]);
    let mut words = [0u8; FRAME_BITS];
    let mut p = 0;
    for (i, &n) in FULL_WORDS.iter().enumerate() {
        let v = match i {
            0..=3 => c.golay23.encode(u[i] as u128) as u32,
            4..=6 => c.hamming.encode(u[i] as u128) as u32,
            _ => u[i],
        };
        fec::put_word(v ^ m[i], &mut words[p..p + n]);
        p += n;
    }
    let _ = c.full_layout.join(&words, &mut chan[..FRAME_BITS]);
}

/// The modulation vectors m0..m7 for u0 (Equations 86 to 93).
fn modulation_words(u0: u32) -> [u32; 8] {
    let mut pr = [0u8; 114];
    fec::modulation(u0, &mut pr);
    [
        0,
        fec::word_of(&pr[0..23]),
        fec::word_of(&pr[23..46]),
        fec::word_of(&pr[46..69]),
        fec::word_of(&pr[69..84]),
        fec::word_of(&pr[84..99]),
        fec::word_of(&pr[99..114]),
        0,
    ]
}

/// Uniform quantiser of Equations 62 and 63.
fn quantise(x: f64, bits: u8, step: f64) -> u32 {
    if bits == 0 {
        return 0;
    }
    let half = 1i64 << (bits - 1);
    let q = (x / step).floor();
    if q.is_nan() || q < -(half as f64) {
        0
    } else if q >= half as f64 {
        (2 * half - 1) as u32
    } else {
        (q as i64 + half) as u32
    }
}

/// Reconstruction of Equations 68 and 71.
fn dequantise(b: u32, bits: u8, step: f64) -> f64 {
    if bits == 0 {
        return 0.0;
    }
    step * (b as f64 - (1u64 << (bits - 1)) as f64 + 0.5)
}

/// The first coefficient index k of every block that the higher-order values b8..b_{L+1} carry, for blocks 1 to 6
/// (Equations 64 and 72; the range "2 <= i <= 6, 1 <= k" of the decoding text is read as 1 <= i <= 6, 2 <= k).
const HOC_FIRST_K: usize = 2;

/// Step size of a higher-order coefficient C_{i,k} with `bits` bits (Tables 3 and 4).
pub fn hoc_step(bits: u8, k: usize) -> f64 {
    if bits == 0 {
        return 0.0;
    }
    HOC_STEP[bits as usize - 1] * HOC_SIGMA[k - 2]
}

/// The decoder's amplitude state: log2 of the previous frame's reconstructed (not enhanced) amplitudes and its L.
#[derive(Clone, Copy, Debug)]
pub struct AmpState {
    /// `log2 M(-1)` for `l = 0..=l + 1`.
    pub log2m: [f64; MAX_L + 2],
    /// L(-1).
    pub l: usize,
}

impl Default for AmpState {
    /// The initial values of Annex A: M(-1) = 1 for all l, L(-1) = 30.
    fn default() -> Self {
        AmpState {
            log2m: [0.0; MAX_L + 2],
            l: 30,
        }
    }
}

/// Prediction coefficient rho (Equation 55).
fn rho(l: usize) -> f64 {
    if l <= 15 {
        0.4
    } else if l <= 24 {
        0.03 * l as f64 - 0.05
    } else {
        0.7
    }
}

/// The prediction of the current frame's log2 amplitudes from `st` (Equations 52 to 57, 75 to 79).
fn prediction(st: &AmpState, l: usize, out: &mut [f64; MAX_L + 1]) {
    let mut prev = st.log2m;
    prev[0] = 0.0; // M(-1) at l = 0 is 1.0.
    predict(&prev, st.l, l, rho(l), out);
}

/// Decodes the values of a frame with a valid b0 into the model parameters (section 6.4) and updates the amplitude state.
pub fn dequantise_frame(v: &Values, st: &mut AmpState, out: &mut Params) {
    let w0 = w0_of(v.b[0]);
    let l = l_of(w0);
    let k = bands_for(l);
    let mut bits = [0u8; MAX_VALUES];
    bits_of(l, &mut bits);
    out.w0 = w0;
    out.l = l;
    out.voiced = [false; MAX_L + 1];
    out.amp = [0.0; MAX_L + 1];
    for li in 1..=l {
        let kappa = band_of(li);
        out.voiced[li] = v.b[1] >> (k - kappa) & 1 == 1;
    }
    // Gain vector and its inverse six-point DCT.
    let row = l - MIN_L;
    let mut g = [0.0f64; 6];
    g[0] = GAIN_LEVELS[v.b[2] as usize & 63];
    for m in 3..=7 {
        let (b, step) = GAIN_ALLOC[row][m - 3];
        g[m - 2] = dequantise(v.b[m], b, step);
    }
    let mut r = [0.0f64; 6];
    idct(&g, &mut r);
    // Blocks: C_{i,1} = R_i, then the higher-order coefficients in order.
    let mut t = [0.0f64; MAX_L + 1];
    let mut m = 8;
    let mut at = 1;
    for (i, &ji) in BLOCKS[row].iter().enumerate() {
        let ji = ji as usize;
        let mut c = [0.0f64; 10];
        c[0] = r[i];
        for (kk, ck) in c.iter_mut().enumerate().take(ji).skip(HOC_FIRST_K - 1) {
            *ck = dequantise(v.b[m], bits[m], hoc_step(bits[m], kk + 1));
            m += 1;
        }
        idct(&c[..ji], &mut t[at..at + ji]);
        at += ji;
    }
    let mut pred = [0.0f64; MAX_L + 1];
    prediction(st, l, &mut pred);
    let mut log2m = [0.0f64; MAX_L + 2];
    for li in 1..=l {
        log2m[li] = t[li] + pred[li];
        out.amp[li] = log2m[li].exp2();
    }
    st.log2m = log2m;
    st.l = l;
}

/// The smallest amplitude whose logarithm the encoder takes.
const MIN_AMP: f64 = 1e-3;

/// Quantises the analysed parameters `p` (with `l` and the band decisions `bands[1..=K]` taken from b0) into `v`, and
/// updates `st` by running the decoder on the result (Figure 16). The sync bit is not set.
pub fn quantise_frame(p: &Params, bands: &[bool], st: &mut AmpState, v: &mut Values, rec: &mut Params) {
    *v = Values::default();
    let b0 = b0_of(p.w0);
    let l = l_of(w0_of(b0));
    let k = bands_for(l);
    v.b[0] = b0;
    for kk in 1..=k {
        if bands.get(kk).copied().unwrap_or(false) {
            v.b[1] |= 1 << (k - kk);
        }
    }
    let mut pred = [0.0f64; MAX_L + 1];
    prediction(st, l, &mut pred);
    let mut t = [0.0f64; MAX_L + 1];
    for li in 1..=l {
        let a = if li <= p.l { p.amp[li] } else { p.amp[p.l] };
        t[li] = a.max(MIN_AMP).log2() - pred[li];
    }
    let row = l - MIN_L;
    let mut bits = [0u8; MAX_VALUES];
    bits_of(l, &mut bits);
    let mut r = [0.0f64; 6];
    let mut at = 1;
    let mut m = 8;
    for (i, &ji) in BLOCKS[row].iter().enumerate() {
        let ji = ji as usize;
        let mut c = [0.0f64; 10];
        dct(&t[at..at + ji], &mut c[..ji]);
        r[i] = c[0];
        for (kk, &ck) in c.iter().enumerate().take(ji).skip(HOC_FIRST_K - 1) {
            v.b[m] = quantise(ck, bits[m], hoc_step(bits[m], kk + 1));
            m += 1;
        }
        at += ji;
    }
    let mut g = [0.0f64; 6];
    dct(&r, &mut g);
    v.b[2] = nearest(&GAIN_LEVELS, g[0]);
    for mm in 3..=7 {
        let (b, step) = GAIN_ALLOC[row][mm - 3];
        v.b[mm] = quantise(g[mm - 2], b, step);
    }
    dequantise_frame(v, st, rec);
}

/// The index of the level nearest `x` (the first on a tie).
pub fn nearest(levels: &[f64], x: f64) -> u32 {
    let mut best = 0;
    let mut d = f64::INFINITY;
    for (i, &y) in levels.iter().enumerate() {
        let e = (y - x).abs();
        if e < d {
            d = e;
            best = i;
        }
    }
    best as u32
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tables::full::{HOC_BITS, HOC_OFFSET};

    fn rng(x: &mut u64) -> u32 {
        *x = x.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1_442_695_040_888_963_407);
        (*x >> 33) as u32
    }

    #[test]
    fn tables_have_the_stated_sums() {
        for l in 9..=56 {
            let row = l - 9;
            assert_eq!(BLOCKS[row].iter().map(|&j| j as usize).sum::<usize>(), l);
            for i in 0..5 {
                assert!(BLOCKS[row][i] <= BLOCKS[row][i + 1]);
                assert!(BLOCKS[row][i] as usize >= l / 6 && BLOCKS[row][i + 1] as usize <= l.div_ceil(6));
            }
            let hoc = (HOC_OFFSET[row + 1] - HOC_OFFSET[row]) as usize;
            assert_eq!(hoc, l - 6);
            let mut bits = [0u8; MAX_VALUES];
            bits_of(l, &mut bits);
            let total: usize = bits.iter().map(|&b| b as usize).sum();
            assert_eq!(total, 73 - bands_for(l), "L = {l}");
            let mut order = [(0u8, 0u8); 80];
            assert_eq!(scan(l, &mut order), 73 - bands_for(l));
        }
        assert_eq!(HOC_BITS.len(), 1272);
        let s: f64 = crate::tables::full::W_I.iter().skip(1).map(|w| 2.0 * w * w).sum::<f64>()
            + crate::tables::full::W_I[0].powi(2);
        assert!((s - 1.0).abs() < 1e-6, "{s}");
        let wi = &crate::tables::full::W_I;
        let sum_i: f64 = wi[0] + 2.0 * wi[1..].iter().sum::<f64>();
        assert!((sum_i - 14.71252893).abs() < 1e-6, "{sum_i}");
        let wr = &crate::tables::full::W_R;
        let sum: f64 = wr[0] + 2.0 * wr[1..].iter().sum::<f64>();
        let sq: f64 = wr[0] * wr[0] + 2.0 * wr[1..].iter().map(|w| w * w).sum::<f64>();
        assert!((sum - 110.019872).abs() < 1e-5 && (sq - 80.683623).abs() < 1e-5, "{sum} {sq}");
        let lpf = &crate::tables::full::H_LPF;
        let ls: f64 = lpf[0] + 2.0 * lpf[1..].iter().sum::<f64>();
        assert!((ls - 1.00138).abs() < 1e-5, "{ls}");
        assert!(GAIN_LEVELS.windows(2).all(|p| p[0] < p[1]));
    }

    #[test]
    fn worked_example_l16() {
        // Table 5 of the parameter encoding example (L = 16, K = 6).
        assert_eq!(BLOCKS[16 - 9], [2, 2, 3, 3, 3, 3]);
        assert_eq!(bands_for(16), 6);
        // Table 6: bits and steps of G2..G6.
        let t6 = [(6, 0.04650), (6, 0.03015), (6, 0.02520), (5, 0.04060), (5, 0.03696)];
        for (i, &(b, s)) in t6.iter().enumerate() {
            assert_eq!(GAIN_ALLOC[7][i].0, b);
            assert!((GAIN_ALLOC[7][i].1 - s).abs() < 1e-9);
        }
        // Table 7: bits and steps of b8..b17 (coefficient index k of each).
        let t7 = [
            (6, 2, 0.04605),
            (6, 2, 0.04605),
            (5, 2, 0.08596),
            (4, 3, 0.09640),
            (4, 2, 0.12280),
            (3, 3, 0.15665),
            (3, 2, 0.19955),
            (3, 3, 0.15665),
            (3, 2, 0.19955),
            (2, 3, 0.20485),
        ];
        let mut bits = [0u8; MAX_VALUES];
        bits_of(16, &mut bits);
        for (i, &(b, k, s)) in t7.iter().enumerate() {
            assert_eq!(bits[8 + i], b, "b{}", 8 + i);
            assert!((hoc_step(b, k) - s).abs() < 1e-5, "b{}", 8 + i);
        }
        // Table 3 and 4 example: 4 bits on C_{i,3}.
        assert!((hoc_step(4, 3) - 0.0964).abs() < 1e-12);
        // Figure 22: bit plane 5 holds b3, b4, b5, b8, b9 in that order; they are u0 bits 2..0 and u1 bits 11, 10.
        let mut order = [(0u8, 0u8); 80];
        let n = scan(16, &mut order);
        let plane5: Vec<u8> = order[..n].iter().filter(|e| e.1 == 5).map(|e| e.0).collect();
        assert_eq!(plane5, [3, 4, 5, 8, 9]);
        assert_eq!(order[0], (3, 5));
        assert_eq!(order[4], (9, 5));
        // With one value set at a time, its bit lands where Figure 22 says: u0 bit 2 is speech bit 9.
        let mut v = Values::default();
        v.b[0] = 40;
        v.b[3] = 1 << 5;
        let mut sb = [0u8; SPEECH_BITS];
        pack(&v, 16, &mut sb);
        assert_eq!(sb[9], 1);
        v.b[3] = 0;
        v.b[9] = 1 << 5;
        pack(&v, 16, &mut sb);
        assert_eq!(sb[12 + 1], 1, "u1 bit 10");
        // Corrected Table 10 entries, whose bit numbers count from 1: u7 bit 6 is the LSB of b16 (0-based u7 bit 5,
        // speech bit 82), u7 bit 1 the sync bit b18 (0-based u7 bit 0, speech bit 87).
        v.b[9] = 0;
        v.b[16] = 1;
        pack(&v, 16, &mut sb);
        assert_eq!(sb.iter().map(|&b| b as usize).sum::<usize>(), sb[..6].iter().map(|&b| b as usize).sum::<usize>() + 1);
        assert_eq!(sb[82], 1, "u7 bit 6");
        v.b[16] = 0;
        v.b[18] = 1;
        pack(&v, 16, &mut sb);
        assert_eq!(sb[87], 1, "u7 bit 0");
    }

    #[test]
    fn pack_unpack_inverse() {
        let mut x = 7u64;
        for _ in 0..2000 {
            let b0 = rng(&mut x) % 208;
            let l = l_of(w0_of(b0));
            let mut bits = [0u8; MAX_VALUES];
            bits_of(l, &mut bits);
            let mut v = Values::default();
            v.b[0] = b0;
            v.b[1] = rng(&mut x) & ((1 << bands_for(l)) - 1);
            v.b[2] = rng(&mut x) & 63;
            for m in 3..=l + 1 {
                v.b[m] = rng(&mut x) & ((1u32 << bits[m]) - 1);
            }
            v.b[l + 2] = rng(&mut x) & 1;
            let mut sb = [0u8; SPEECH_BITS];
            pack(&v, l, &mut sb);
            assert_eq!(b0_from(&sb), b0);
            let mut w = Values::default();
            unpack(&sb, l, &mut w);
            assert_eq!(v, w);
            let mut chan = [0u8; FRAME_BITS];
            encode_channel(&sb, &mut chan);
            let mut back = [0u8; SPEECH_BITS];
            let mut eps = [9u8; 7];
            decode_channel(&chan, &mut back, &mut eps);
            assert_eq!(back, sb);
            assert_eq!(eps, [0; 7]);
        }
    }

    #[test]
    fn pitch_round_trip() {
        for b0 in 0..=207 {
            assert_eq!(b0_of(w0_of(b0)), b0);
            let l = l_of(w0_of(b0));
            assert!((9..=56).contains(&l));
        }
    }

    #[test]
    fn zero_bit_coefficients_decode_to_zero() {
        // L = 32: the last coefficient has no bits.
        let mut bits = [0u8; MAX_VALUES];
        bits_of(32, &mut bits);
        assert_eq!(bits[33], 0);
        assert_eq!(dequantise(5, 0, 0.1), 0.0);
        assert_eq!(quantise(3.0, 0, 0.1), 0);
    }

    #[test]
    fn amplitude_loop_converges_and_matches() {
        let mut p = Params::silent(2.0 * PI / 60.0, 0);
        p.l = l_of(w0_of(b0_of(p.w0)));
        for li in 1..=p.l {
            p.amp[li] = 2000.0 / li as f64;
        }
        let bands = [true; 13];
        let mut enc = AmpState::default();
        let mut dec = AmpState::default();
        let mut v = Values::default();
        let mut rec = Params::silent(0.0, 0);
        let mut out = Params::silent(0.0, 0);
        let mut err = 0.0;
        for _ in 0..20 {
            quantise_frame(&p, &bands, &mut enc, &mut v, &mut rec);
            dequantise_frame(&v, &mut dec, &mut out);
            assert_eq!(rec, out);
            assert_eq!(enc.log2m, dec.log2m);
            err = (1..=p.l).map(|li| (out.amp[li] / p.amp[li]).log2().abs()).sum::<f64>() / p.l as f64;
        }
        assert!(err < 0.25, "mean log2 error {err}");
    }
}
