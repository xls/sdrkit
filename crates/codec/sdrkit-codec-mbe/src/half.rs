//! Half-rate frame (TIA-102.BABA-1 clauses 4, 5 and 7): quantiser values b0..b8, the vectors u0..u3, frame types, tone
//! frames, the quantisers and the amplitude reconstruction.

use crate::fec::{self, codes, HALF_INFO, HALF_WORDS};
use crate::full::nearest;
use crate::model::{band_of, dct, idct, predict, Params, MAX_L, MIN_L};
use crate::tables::half::{
    BLOCKS, GAIN_LEVELS, HOC_B5, HOC_B6, HOC_B7, HOC_B8, PITCH, PRBA24, PRBA58, VOICING,
};
use crate::tables::{TONE_PAIR, TONE_SINGLE, TONE_ZERO_F0};
use std::f64::consts::{PI, SQRT_2};

/// Speech bits per frame.
pub const SPEECH_BITS: usize = 49;
/// Channel bits per frame.
pub const FRAME_BITS: usize = 72;
/// Bits of b0..b8 (Table 2).
#[cfg(test)]
pub const WIDTHS: [u8; 9] = [7, 5, 5, 9, 7, 5, 4, 4, 3];

/// Prediction coefficient (Equation 13).
const RHO: f64 = 0.65;

/// The tables of one coder that quantises as the half-rate coder does: 7 bits of pitch, a voicing vector, a differential
/// gain, two PRBA vectors and four higher-order vectors. The quantisers of this module take the tables they work with.
pub struct Books {
    /// omega_0 of b0.
    pub w0: fn(u32) -> f64,
    /// L of b0.
    pub l: fn(u32) -> usize,
    /// The b0 values 0..`voice_b0` that an encoder sends.
    pub voice_b0: u32,
    /// Voicing vectors nu_0..nu_7, row b1.
    pub voicing: &'static [[u8; 8]],
    /// The number of voicing vectors the encoder searches, from row 0.
    pub voicing_search: usize,
    /// Differential gain levels, row b2.
    pub gain: &'static [f64],
    /// [G2, G3, G4], row b3.
    pub prba24: &'static [[f64; 3]],
    /// [G5, G6, G7, G8], row b4.
    pub prba58: &'static [[f64; 4]],
    /// The higher-order codebook of each block.
    pub hoc: [&'static [[f64; 4]]; 4],
    /// The row of each higher-order codebook that the field value 1 selects: field value b is row `hoc_step * b`.
    pub hoc_step: [usize; 4],
    /// L(-1) of the initial state.
    pub start_l: usize,
    /// Lambda(-1) of the initial state, every harmonic.
    pub start_lambda: f64,
}

/// The tables of the half-rate coder (Annexes A to G); the encoder searches voicing vectors 0 to 16 (clause 4.2) and
/// the state starts at Lambda(-1) = 1, L(-1) = 15 (clauses 4.3 and 4.4).
pub static BOOKS: Books = Books {
    w0: w0_of,
    l: l_of,
    voice_b0: 120,
    voicing: &VOICING,
    voicing_search: 17,
    gain: &GAIN_LEVELS,
    prba24: &PRBA24,
    prba58: &PRBA58,
    hoc: [&HOC_B5, &HOC_B6, &HOC_B7, &HOC_B8],
    hoc_step: [1; 4],
    start_l: 15,
    start_lambda: 1.0,
};

impl Books {
    /// The amplitude state before the first frame; gamma(-1) = 0.
    pub fn start(&self) -> AmpState {
        AmpState {
            lambda: [self.start_lambda; MAX_L + 2],
            l: self.start_l,
            gamma: 0.0,
        }
    }

    /// The voice b0 whose omega_0 is nearest `w0` in absolute difference (clause 4.1).
    pub fn b0_of(&self, w0: f64) -> u32 {
        let mut best = 0;
        let mut d = f64::INFINITY;
        for b in 0..self.voice_b0 {
            let e = ((self.w0)(b) - w0).abs();
            if e < d {
                d = e;
                best = b;
            }
        }
        best
    }
}

/// What b0 says a frame is (Table 4).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// A voice frame.
    Voice,
    /// An erasure: repeat the previous frame.
    Erasure,
    /// A silence frame.
    Silence,
    /// b0 126 or 127 without the six ones of a tone frame: repeated as invalid.
    Invalid,
}

/// The frame type of b0.
pub fn kind_of(b0: u32) -> Kind {
    match b0 {
        0..=119 => Kind::Voice,
        120..=123 => Kind::Erasure,
        124..=125 => Kind::Silence,
        _ => Kind::Invalid,
    }
}

/// Bit positions of the 49 speech bits (Tables 5 to 8): entry `p` is (value, bit) of speech bit `p`, the speech bits being
/// u0, u1, u2, u3 each most significant bit first.
pub static LAYOUT: [(u8, u8); SPEECH_BITS] = [
    (0, 6), (0, 5), (0, 4), (0, 3), (1, 4), (1, 3), (1, 2), (1, 1), (2, 4), (2, 3), (2, 2), (2, 1),
    (3, 8), (3, 7), (3, 6), (3, 5), (3, 4), (3, 3), (3, 2), (3, 1), (4, 6), (4, 5), (4, 4), (4, 3),
    (5, 4), (5, 3), (5, 2), (5, 1), (6, 3), (6, 2), (6, 1), (7, 3), (7, 2), (7, 1), (8, 2),
    (1, 0), (2, 0), (0, 2), (0, 1), (0, 0), (3, 0), (4, 2), (4, 1), (4, 0), (5, 0), (6, 0), (7, 0), (8, 1), (8, 0),
];

/// The speech bits of b0..b8, where speech bit `p` is bit `layout[p].1` of value `layout[p].0`.
pub fn pack_fields(layout: &[(u8, u8)], b: &[u32; 9], out: &mut [u8]) {
    for (o, &(v, i)) in out.iter_mut().zip(layout.iter()) {
        *o = (b[v as usize] >> i & 1) as u8;
    }
}

/// The inverse of [`pack_fields`].
pub fn unpack_fields(layout: &[(u8, u8)], bits: &[u8], b: &mut [u32; 9]) {
    *b = [0; 9];
    for (&x, &(v, i)) in bits.iter().zip(layout.iter()) {
        b[v as usize] |= ((x & 1) as u32) << i;
    }
}

/// The 49 speech bits of b0..b8.
pub fn pack(b: &[u32; 9], out: &mut [u8; SPEECH_BITS]) {
    pack_fields(&LAYOUT, b, out);
}

/// b0..b8 of the 49 speech bits.
pub fn unpack(bits: &[u8; SPEECH_BITS], b: &mut [u32; 9]) {
    unpack_fields(&LAYOUT, bits, b);
}

/// A tone frame's index and amplitude.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Tone {
    /// Tone index ID (Table 9).
    pub id: u8,
    /// Amplitude AD, 0..=127.
    pub ad: u8,
}

/// Whether the speech bits are a tone frame: u0 bits 11..6 all ones, tested before b0 (clause 7.2).
pub fn is_tone(bits: &[u8; SPEECH_BITS]) -> bool {
    bits[..6].iter().all(|&b| b & 1 == 1)
}

/// Bit of the tone amplitude AD's least significant bit in u3 (Table 10).
const TONE_AD_LSB_U3_BIT: usize = 4;

/// The tone of a tone frame's speech bits (Table 10). ID is read from its first copy, which the u1 Golay word protects.
pub fn tone_from(bits: &[u8; SPEECH_BITS]) -> Tone {
    let ad = fec::word_of(&bits[6..12]) << 1 | bits[35 + 13 - TONE_AD_LSB_U3_BIT] as u32;
    let id = fec::word_of(&bits[12..20]);
    Tone {
        id: id as u8,
        ad: ad as u8,
    }
}

/// The speech bits of a tone frame (Table 10).
pub fn tone_bits(t: Tone, out: &mut [u8; SPEECH_BITS]) {
    let id = t.id as u32;
    let ad = (t.ad & 127) as u32;
    let mut u = [0u32; 4];
    u[0] = 63 << 6 | ad >> 1;
    u[1] = id << 4 | id >> 4;
    u[2] = (id & 15) << 7 | id >> 1;
    u[3] = (id & 1) << 13 | id << 5 | (ad & 1) << TONE_AD_LSB_U3_BIT;
    from_vectors(&u, out);
}

/// The model parameters of a tone, through Annex J and Equations 65 to 68; `None` for an invalid ID. Pairs give both tones
/// the amplitude of Equation 68.
pub fn tone_params(t: Tone, out: &mut Params) -> Option<()> {
    let (f0, l1, l2) = match t.id {
        255 => (TONE_ZERO_F0, 0, 0),
        128..=163 => TONE_PAIR[t.id as usize - 128],
        id => {
            let &(_, _, per, l) = TONE_SINGLE.iter().find(|e| (e.0..=e.1).contains(&id))?;
            (per * id as f64, l, l)
        }
    };
    let l = ((3812.5 / f0).floor() as usize).min(MAX_L);
    *out = Params::silent(2.0 * PI * f0 / 8000.0, l);
    let amp = 16384.0 * 10f64.powf(0.03555 * (t.ad as f64 - 127.0));
    for h in [l1 as usize, l2 as usize] {
        if (1..=l).contains(&h) {
            out.voiced[h] = true;
            out.amp[h] = amp;
        }
    }
    Some(())
}

/// The 49 speech bits as u0..u3.
pub fn to_vectors(bits: &[u8; SPEECH_BITS], u: &mut [u32; 4]) {
    let mut p = 0;
    for (i, &n) in HALF_INFO.iter().enumerate() {
        u[i] = fec::word_of(&bits[p..p + n]);
        p += n;
    }
}

/// The inverse of [`to_vectors`].
pub fn from_vectors(u: &[u32; 4], bits: &mut [u8; SPEECH_BITS]) {
    let mut p = 0;
    for (i, &n) in HALF_INFO.iter().enumerate() {
        fec::put_word(u[i], &mut bits[p..p + n]);
        p += n;
    }
}

/// The modulation vector m1 for u0 (Equations 52 to 54).
pub fn modulation_word(u0: u32) -> u32 {
    let mut pr = [0u8; 23];
    fec::modulation(u0, &mut pr);
    fec::word_of(&pr)
}

/// FEC-decodes the 72 channel bits into the speech bits and the error counts of c0 and c1 (clauses 5.2 to 5.4). A c0
/// beyond the code's radius counts 4.
pub fn decode_channel(chan: &[u8], bits: &mut [u8; SPEECH_BITS], eps: &mut [u8; 2]) {
    let c = codes();
    let mut words = [0u8; FRAME_BITS];
    if c.half_layout.split(&chan[..FRAME_BITS], &mut words).is_err() {
        return;
    }
    let mut cw = [0u32; 4];
    let mut p = 0;
    for (i, &n) in HALF_WORDS.iter().enumerate() {
        cw[i] = fec::word_of(&words[p..p + n]);
        p += n;
    }
    let (u0, e0) = fec::decode_word(&c.golay24, cw[0]);
    let (u1, e1) = fec::decode_word(&c.golay23, cw[1] ^ modulation_word(u0));
    *eps = [e0, e1];
    from_vectors(&[u0, u1, cw[2], cw[3]], bits);
}

/// The 72 channel bits of the speech bits.
pub fn encode_channel(bits: &[u8; SPEECH_BITS], chan: &mut [u8]) {
    let c = codes();
    let mut u = [0u32; 4];
    to_vectors(bits, &mut u);
    let cw = [
        c.golay24.encode(u[0] as u128) as u32,
        c.golay23.encode(u[1] as u128) as u32 ^ modulation_word(u[0]),
        u[2],
        u[3],
    ];
    let mut words = [0u8; FRAME_BITS];
    let mut p = 0;
    for (i, &n) in HALF_WORDS.iter().enumerate() {
        fec::put_word(cw[i], &mut words[p..p + n]);
        p += n;
    }
    let _ = c.half_layout.join(&words, &mut chan[..FRAME_BITS]);
}

/// omega_0 of a voice b0: 2 pi times the Annex A value, whose column holds cycles per sample.
pub fn w0_of(b0: u32) -> f64 {
    2.0 * PI * PITCH[b0 as usize % 120].1
}

/// L of a voice b0 (Annex A).
pub fn l_of(b0: u32) -> usize {
    PITCH[b0 as usize % 120].0 as usize
}

/// Element of a voicing vector for harmonic `l` at `w0`: floor(16 l omega_0 / 2 pi), read with `l` in the product
/// (Equation 5), limited to 0..=7.
pub fn voicing_element(l: usize, w0: f64) -> usize {
    ((16.0 * l as f64 * w0 / (2.0 * PI)).floor().max(0.0) as usize).min(7)
}

/// Silence parameters (Equations 1 to 3).
pub const SILENCE_W0: f64 = 2.0 * PI / 32.0;
/// Harmonics of a silence frame.
pub const SILENCE_L: usize = 14;

/// The decoder's amplitude state: Lambda(-1), L(-1) and gamma(-1) of the last voice frame.
#[derive(Clone, Copy, Debug)]
pub struct AmpState {
    /// `Lambda(-1)` for `l = 1..=l`.
    pub lambda: [f64; MAX_L + 2],
    /// L(-1).
    pub l: usize,
    /// gamma(-1).
    pub gamma: f64,
}

impl Default for AmpState {
    /// The initial values of the half-rate coder.
    fn default() -> Self {
        BOOKS.start()
    }
}

fn prediction(st: &AmpState, l: usize, out: &mut [f64; MAX_L + 1]) {
    let mut prev = st.lambda;
    prev[0] = prev[1]; // Equation 14.
    predict(&prev, st.l, l, RHO, out);
}

/// Highest DCT coefficient index (1-based) that a higher-order vector fills: C_{i,k} = H_{i,k-2} for 3 <= k <= min(J, 6),
/// the inverse of the encoder's H_{i,j} = C_{i,j+2} (Equation 37).
const HOC_LAST_K: usize = 6;

/// Decodes the amplitudes of a voice or silence frame (`w0`, `l`, `voiced` set in `out`) from b2..b8 with the tables `k`
/// (clause 4.4). The new state is written to `next`; the caller keeps it only for voice frames.
pub fn dequantise_amps(k: &Books, b: &[u32; 9], st: &AmpState, next: &mut AmpState, out: &mut Params) {
    let l = out.l;
    let row = l - MIN_L;
    let gamma = k.gain[b[2] as usize % k.gain.len()] + 0.5 * st.gamma;
    let mut g = [0.0f64; 8];
    g[1..4].copy_from_slice(&k.prba24[b[3] as usize % k.prba24.len()]);
    g[4..8].copy_from_slice(&k.prba58[b[4] as usize % k.prba58.len()]);
    let mut r = [0.0f64; 8];
    idct(&g, &mut r);
    let mut t = [0.0f64; MAX_L + 1];
    let mut at = 1;
    for (i, &ji) in BLOCKS[row].iter().enumerate() {
        let ji = ji as usize;
        let mut c = [0.0f64; 17];
        c[0] = (r[2 * i] + r[2 * i + 1]) / 2.0;
        c[1] = (r[2 * i] - r[2 * i + 1]) / (2.0 * SQRT_2);
        let h = &k.hoc[i][k.hoc_step[i] * b[5 + i] as usize % k.hoc[i].len()];
        let last = ji.min(HOC_LAST_K);
        if last >= 3 {
            c[2..last].copy_from_slice(&h[..last - 2]);
        }
        idct(&c[..ji], &mut t[at..at + ji]);
        at += ji;
    }
    let mut pred = [0.0f64; MAX_L + 1];
    prediction(st, l, &mut pred);
    let mean_t = t[1..=l].iter().sum::<f64>() / l as f64;
    let big_gamma = gamma - 0.5 * (l as f64).log2() - mean_t;
    *next = AmpState {
        lambda: [0.0; MAX_L + 2],
        l,
        gamma,
    };
    let uv_scale = 0.2046 / out.w0.sqrt();
    out.amp = [0.0; MAX_L + 1];
    for li in 1..=l {
        let lam = t[li] + pred[li] + big_gamma;
        next.lambda[li] = lam;
        let m = (0.693 * lam).exp();
        out.amp[li] = if out.voiced[li] { m } else { uv_scale * m };
    }
}

/// Sets `out.w0`, `out.l` and the voicing of a voice frame from b0 and b1 with the tables `k` (clauses 4.1, 4.2).
pub fn voice_params(k: &Books, b0: u32, b1: u32, out: &mut Params) {
    *out = Params::silent((k.w0)(b0), (k.l)(b0));
    let row = &k.voicing[b1 as usize % k.voicing.len()];
    for li in 1..=out.l {
        out.voiced[li] = row[voicing_element(li, out.w0)] == 1;
    }
}

/// The smallest amplitude whose logarithm the encoder takes.
const MIN_AMP: f64 = 1e-3;

/// Quantises analysed parameters `p` into b0..b8 with the tables `k`, where `p.w0` and `p.l` are already the table values
/// of the chosen b0 and `bands[1..=K]` are the band decisions, then runs the decoder to update `st` (clause 4, B3).
pub fn quantise_frame(
    k: &Books,
    b0: u32,
    p: &Params,
    bands: &[bool],
    st: &mut AmpState,
    b: &mut [u32; 9],
    rec: &mut Params,
) {
    *b = [0; 9];
    b[0] = b0;
    let l = (k.l)(b0);
    let w0 = (k.w0)(b0);
    // b1: the voicing vector of least weighted mismatch (Equation 4).
    let mut best = (f64::INFINITY, 0usize);
    for (n, row) in k.voicing.iter().enumerate().take(k.voicing_search) {
        let mut e = 0.0;
        for li in 1..=l {
            let v = bands.get(band_of(li)).copied().unwrap_or(false) as u8;
            if v != row[voicing_element(li, w0)] {
                e += p.amp[li] * p.amp[li];
            }
        }
        if e < best.0 {
            best = (e, n);
        }
    }
    b[1] = best.1 as u32;
    voice_params(k, b0, b[1], rec);
    // Log amplitudes and gain (Equations 8 to 10).
    let mut lam = [0.0f64; MAX_L + 1];
    let log2l = (l as f64).log2();
    for li in 1..=l {
        let m = p.amp[li].max(MIN_AMP).log2();
        lam[li] = if rec.voiced[li] {
            m + 0.5 * log2l
        } else {
            m + 0.5 * (w0 * l as f64).log2() + 2.289
        };
    }
    let gamma = lam[1..=l].iter().sum::<f64>() / l as f64;
    b[2] = nearest(k.gain, gamma - 0.5 * st.gamma);
    // Residuals, blocks, PRBA and higher-order vectors (Equations 11 to 25).
    let mut pred = [0.0f64; MAX_L + 1];
    prediction(st, l, &mut pred);
    let mut t = [0.0f64; MAX_L + 1];
    for li in 1..=l {
        t[li] = lam[li] - pred[li];
    }
    let row = l - MIN_L;
    let mut rr = [0.0f64; 8];
    let mut at = 1;
    for (i, &ji) in BLOCKS[row].iter().enumerate() {
        let ji = ji as usize;
        let mut c = [0.0f64; 17];
        dct(&t[at..at + ji], &mut c[..ji]);
        rr[2 * i] = c[0] + SQRT_2 * c[1];
        rr[2 * i + 1] = c[0] - SQRT_2 * c[1];
        let n = ji.saturating_sub(2).min(4);
        if n > 0 {
            b[5 + i] = nearest_vector(k.hoc[i], k.hoc_step[i], &c[2..2 + n]);
        }
        at += ji;
    }
    let mut g = [0.0f64; 8];
    dct(&rr, &mut g);
    b[3] = nearest_vector(k.prba24, 1, &g[1..4]);
    b[4] = nearest_vector(k.prba58, 1, &g[4..8]);
    let mut next = *st;
    dequantise_amps(k, b, st, &mut next, rec);
    *st = next;
}

/// The field value of the codebook row with the smallest squared error to `x` over the first `x.len()` elements, among
/// the rows `step * value`.
fn nearest_vector<const N: usize>(book: &[[f64; N]], step: usize, x: &[f64]) -> u32 {
    let mut best = (f64::INFINITY, 0usize);
    for (i, row) in book.iter().step_by(step).enumerate() {
        let e: f64 = x.iter().zip(row.iter()).map(|(a, b)| (a - b) * (a - b)).sum();
        if e < best.0 {
            best = (e, i);
        }
    }
    best.1 as u32
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rng(x: &mut u64) -> u32 {
        *x = x.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1_442_695_040_888_963_407);
        (*x >> 33) as u32
    }

    #[test]
    fn tables_have_the_stated_sizes() {
        assert_eq!(PRBA24.len(), 512);
        assert_eq!(PRBA58.len(), 128);
        assert_eq!((HOC_B5.len(), HOC_B6.len(), HOC_B7.len(), HOC_B8.len()), (32, 16, 16, 8));
        for (i, row) in BLOCKS.iter().enumerate() {
            assert_eq!(row.iter().map(|&j| j as usize).sum::<usize>(), i + 9);
            assert!(row.windows(2).all(|p| p[0] <= p[1]));
        }
        assert!(GAIN_LEVELS.windows(2).all(|p| p[0] < p[1]));
        // Every one of the 49 bits appears once.
        let mut seen = [[false; 9]; 9];
        for &(v, i) in &LAYOUT {
            assert!(!seen[v as usize][i as usize]);
            seen[v as usize][i as usize] = true;
        }
        for (v, &w) in WIDTHS.iter().enumerate() {
            assert_eq!(seen[v].iter().filter(|&&s| s).count(), w as usize);
        }
        assert!(VOICING[16..].iter().all(|r| r.iter().all(|&x| x == 0)));
        assert_eq!(VOICING[0], [1; 8]);
    }

    #[test]
    fn pitch_round_trip_and_voicing_index() {
        for b0 in 0..120 {
            assert_eq!(BOOKS.b0_of(w0_of(b0)), b0);
            let (l, w0) = (l_of(b0), w0_of(b0));
            assert!((9..=56).contains(&l));
            assert!(voicing_element(l, w0) <= 7);
        }
        assert!(PITCH.windows(2).all(|p| p[0].1 > p[1].1));
    }

    #[test]
    fn pack_unpack_inverse() {
        let mut x = 3u64;
        for _ in 0..2000 {
            let mut b = [0u32; 9];
            for (v, &w) in WIDTHS.iter().enumerate() {
                b[v] = rng(&mut x) & ((1 << w) - 1);
            }
            let mut sb = [0u8; SPEECH_BITS];
            pack(&b, &mut sb);
            let mut back = [0u32; 9];
            unpack(&sb, &mut back);
            assert_eq!(b, back);
            let mut chan = [0u8; FRAME_BITS];
            encode_channel(&sb, &mut chan);
            let mut sb2 = [0u8; SPEECH_BITS];
            let mut eps = [9u8; 2];
            decode_channel(&chan, &mut sb2, &mut eps);
            assert_eq!(sb, sb2);
            assert_eq!(eps, [0, 0]);
        }
    }

    #[test]
    fn amplitude_loop_converges_and_matches() {
        let b0 = 70;
        let mut p = Params::silent(w0_of(b0), l_of(b0));
        for li in 1..=p.l {
            p.amp[li] = 2000.0 / li as f64;
        }
        let bands = [true; 13];
        let mut enc = AmpState::default();
        let mut dec = AmpState::default();
        let mut b = [0u32; 9];
        let mut rec = Params::silent(0.0, 0);
        let mut err = 0.0;
        for _ in 0..20 {
            quantise_frame(&BOOKS, b0, &p, &bands, &mut enc, &mut b, &mut rec);
            let mut out = Params::silent(0.0, 0);
            voice_params(&BOOKS, b[0], b[1], &mut out);
            let mut next = dec;
            dequantise_amps(&BOOKS, &b, &dec, &mut next, &mut out);
            dec = next;
            assert_eq!(rec, out);
            assert_eq!((enc.lambda, enc.l, enc.gamma), (dec.lambda, dec.l, dec.gamma));
            err = (1..=p.l).map(|li| (out.amp[li] / p.amp[li]).log2().abs()).sum::<f64>() / p.l as f64;
        }
        assert!(err < 0.35, "mean log2 error {err}");
    }

    #[test]
    fn tone_frame_dtmf5() {
        let t = Tone { id: 133, ad: 100 };
        let mut sb = [0u8; SPEECH_BITS];
        tone_bits(t, &mut sb);
        let mut chan = [0u8; FRAME_BITS];
        encode_channel(&sb, &mut chan);
        let mut back = [0u8; SPEECH_BITS];
        let mut eps = [0u8; 2];
        decode_channel(&chan, &mut back, &mut eps);
        assert!(is_tone(&back));
        assert_eq!(tone_from(&back), t);
        let mut u = [0u32; 4];
        to_vectors(&back, &mut u);
        assert_eq!(u[1] >> 4, 133);
        assert_eq!((u[1] & 15) << 4 | u[2] >> 7, 133);
        assert_eq!((u[2] & 127) << 1 | u[3] >> 13, 133);
        assert_eq!(u[3] >> 5 & 255, 133);
        let mut p = Params::silent(0.0, 0);
        assert!(tone_params(Tone { id: 0, ad: 5 }, &mut p).is_none());
        assert!(tone_params(Tone { id: 255, ad: 127 }, &mut p).is_some());
        assert!(p.amp.iter().all(|&a| a == 0.0));
    }
}
