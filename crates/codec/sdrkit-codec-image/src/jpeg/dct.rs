//! The 8 x 8 forward and inverse DCT with the level shift, as two passes of 8-point transforms. Nothing here allocates.
//!
//! Standard: ITU-T T.81 clauses A.3.1 and A.3.3.

// cos(k pi / 16) for k = 0 to 8.
const COS16: [f64; 9] = [
    1.0,
    0.980_785_280_403_230_4,
    0.923_879_532_511_286_7,
    0.831_469_612_302_545_2,
    std::f64::consts::FRAC_1_SQRT_2,
    0.555_570_233_019_602_2,
    0.382_683_432_365_089_8,
    0.195_090_322_016_128_3,
    0.0,
];

const fn cos16(k: usize) -> f64 {
    let m = k % 32;
    if m <= 8 {
        COS16[m]
    } else if m <= 16 {
        -COS16[16 - m]
    } else if m <= 24 {
        -COS16[m - 16]
    } else {
        COS16[32 - m]
    }
}

// M[x][u] = C(u) / 2 * cos((2x + 1) u pi / 16): one 8-point transform in each direction produces the 1/4 C(u) C(v) factor.
const fn matrix() -> [[f32; 8]; 8] {
    let mut m = [[0f32; 8]; 8];
    let mut x = 0;
    while x < 8 {
        let mut u = 0;
        while u < 8 {
            let c = if u == 0 { COS16[4] } else { 1.0 };
            m[x][u] = (c / 2.0 * cos16((2 * x + 1) * u)) as f32;
            u += 1;
        }
        x += 1;
    }
    m
}

const M: [[f32; 8]; 8] = matrix();

/// The inverse DCT of `coefs` (natural order, already dequantised), level-shifted by 128, rounded and held from 0 to 255, into
/// 8 rows of `out` that start `stride` bytes apart. If `out` is shorter than 7 x `stride` + 8 bytes, nothing is written.
pub fn idct_8x8(coefs: &[f32; 64], out: &mut [u8], stride: usize) {
    if out.len() < 7 * stride + 8 {
        return;
    }
    let mut tmp = [0f32; 64];
    // Rows: tmp[v][x] = sum_u M[x][u] S(v,u). A row of zeros stays zero.
    for v in 0..8 {
        let row = &coefs[v * 8..v * 8 + 8];
        if row.iter().all(|&c| c == 0.0) {
            continue;
        }
        for x in 0..8 {
            let mut s = 0.0;
            for u in 0..8 {
                s += M[x][u] * row[u];
            }
            tmp[v * 8 + x] = s;
        }
    }
    // Columns: s(y,x) = sum_v M[y][v] tmp[v][x].
    for y in 0..8 {
        let m = &M[y];
        let o = &mut out[y * stride..y * stride + 8];
        for x in 0..8 {
            let mut s = 128.0;
            for v in 0..8 {
                s += m[v] * tmp[v * 8 + x];
            }
            o[x] = (s + 0.5).floor().clamp(0.0, 255.0) as u8;
        }
    }
}

/// The forward DCT of 8 rows of `pixels` (8 bytes each, `stride` bytes apart), level-shifted by -128, into `coefs` in natural
/// order. If `pixels` is shorter than 7 x `stride` + 8 bytes, `coefs` is all zeros.
pub fn fdct_8x8(pixels: &[u8], stride: usize, coefs: &mut [f32; 64]) {
    *coefs = [0.0; 64];
    if pixels.len() < 7 * stride + 8 {
        return;
    }
    let mut tmp = [0f32; 64];
    // Rows: tmp[y][u] = sum_x M[x][u] (s(y,x) - 128).
    for y in 0..8 {
        let p = &pixels[y * stride..y * stride + 8];
        for u in 0..8 {
            let mut s = 0.0;
            for x in 0..8 {
                s += M[x][u] * (p[x] as f32 - 128.0);
            }
            tmp[y * 8 + u] = s;
        }
    }
    // Columns: S(v,u) = sum_y M[y][v] tmp[y][u].
    for v in 0..8 {
        for u in 0..8 {
            let mut s = 0.0;
            for y in 0..8 {
                s += M[y][v] * tmp[y * 8 + u];
            }
            coefs[v * 8 + u] = s;
        }
    }
}

/// Dequantises a block of quantised coefficients in zig-zag order with a quantisation table in natural order (T.81 A.3.4:
/// `R = Sq x Q`) into natural order, ready for [`idct_8x8`].
#[inline]
pub fn dequantise(zz: &[i16; 64], q: &[u16; 64], out: &mut [f32; 64]) {
    for (k, &z) in super::tables::ZIGZAG.iter().enumerate() {
        let n = z as usize;
        out[n] = zz[k] as f32 * q[n] as f32;
    }
}

/// Quantises DCT coefficients in natural order with a table in natural order (T.81 A.3.4: `Sq = round(S / Q)`) into zig-zag
/// order, saturated to the range of `i16`.
#[inline]
pub fn quantise(coefs: &[f32; 64], q: &[u16; 64], zz: &mut [i16; 64]) {
    for (k, &z) in super::tables::ZIGZAG.iter().enumerate() {
        let n = z as usize;
        zz[k] = (coefs[n] / q[n].max(1) as f32)
            .round()
            .clamp(i16::MIN as f32, i16::MAX as f32) as i16;
    }
}
