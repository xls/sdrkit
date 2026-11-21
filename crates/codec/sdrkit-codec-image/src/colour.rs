//! Colour conversion between R'G'B' and Y'CbCr, full and video range, and the analogue colour-difference signals.
//!
//! Standard: ITU-R BT.601-7 clause 2.5, ITU-R BT.709-6 items 3.2 to 3.5, ITU-R BT.2020-2 Table 4.

/// The luma weights of a standard.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Matrix {
    /// The weight of red.
    pub kr: f32,
    /// The weight of blue.
    pub kb: f32,
}

/// ITU-R BT.601-7 clause 2.5.1 (standard definition television, JPEG and JFIF, most analogue systems).
pub const BT601: Matrix = Matrix {
    kr: 0.299,
    kb: 0.114,
};
/// ITU-R BT.709-6 item 3.2 (high definition television).
pub const BT709: Matrix = Matrix {
    kr: 0.2126,
    kb: 0.0722,
};
/// ITU-R BT.2020-2 Table 4 (ultra high definition television, non-constant luminance).
pub const BT2020: Matrix = Matrix {
    kr: 0.2627,
    kb: 0.0593,
};

/// How Y'CbCr values are coded in 8 bits.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Range {
    /// Y' 0 to 255, Cb and Cr 0 to 255 about 128 (JFIF, ITU-T T.871 clause 7).
    Full,
    /// Y' 16 to 235, Cb and Cr 16 to 240 about 128 (BT.601 clause 2.5.3, BT.709 item 4.6).
    Video,
}

fn round_u8(v: f32) -> u8 {
    // Rounds half away from zero, then saturates. NaN becomes 0.
    if v.is_nan() {
        0
    } else {
        (v + 0.5).floor().clamp(0.0, 255.0) as u8
    }
}

/// The BT.601 luma of an 8-bit R'G'B' pixel, rounded: `0.299 R + 0.587 G + 0.114 B` (BT.601-7 clause 2.5.1).
pub fn luma_601(r: u8, g: u8, b: u8) -> u8 {
    // Fixed point with 16 fraction bits: the weights sum to exactly 65536.
    ((19595 * r as u32 + 38470 * g as u32 + 7471 * b as u32 + 32768) >> 16) as u8
}

/// A converter for one matrix and one range, with its factors worked out once. It allocates nothing.
///
/// ```
/// use sdrkit_codec_image::colour::{Converter, Range, BT601, BT709};
///
/// // 100 % red as the colour bars of BT.601 and BT.709 give it.
/// assert_eq!(Converter::new(BT601, Range::Video).ycbcr([255, 0, 0]), [81, 90, 240]);
/// assert_eq!(Converter::new(BT709, Range::Video).ycbcr([255, 0, 0]), [63, 102, 240]);
/// // JFIF: the same red, full range.
/// assert_eq!(Converter::new(BT601, Range::Full).ycbcr([255, 0, 0]), [76, 85, 255]);
/// ```
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Converter {
    kr: f32,
    kg: f32,
    kb: f32,
    // Scales and offsets of the 8-bit coding.
    y_scale: f32,
    y_offset: f32,
    c_scale: f32,
    // Inverse factors: R' = Y' + rv Cr, G' = Y' - gu Cb - gv Cr, B' = Y' + bu Cb.
    rv: f32,
    gu: f32,
    gv: f32,
    bu: f32,
}

impl Converter {
    /// A converter for `matrix` coded as `range`.
    pub fn new(matrix: Matrix, range: Range) -> Converter {
        let (kr, kb) = (matrix.kr, matrix.kb);
        let kg = 1.0 - kr - kb;
        let (y_scale, y_offset, c_scale) = match range {
            Range::Full => (255.0, 0.0, 255.0),
            Range::Video => (219.0, 16.0, 224.0),
        };
        Converter {
            kr,
            kg,
            kb,
            y_scale,
            y_offset,
            c_scale,
            rv: 2.0 * (1.0 - kr),
            gu: 2.0 * kb * (1.0 - kb) / kg,
            gv: 2.0 * kr * (1.0 - kr) / kg,
            bu: 2.0 * (1.0 - kb),
        }
    }

    /// Y', Cb and Cr of R'G'B' values from 0.0 to 1.0, unrounded and in the coded 8-bit scale.
    #[inline]
    pub fn ycbcr_f32(&self, r: f32, g: f32, b: f32) -> [f32; 3] {
        let y = self.kr * r + self.kg * g + self.kb * b;
        let cb = (b - y) / self.bu;
        let cr = (r - y) / self.rv;
        [
            self.y_offset + self.y_scale * y,
            128.0 + self.c_scale * cb,
            128.0 + self.c_scale * cr,
        ]
    }

    /// The coded Y', Cb and Cr of an 8-bit R'G'B' pixel, rounded and saturated.
    #[inline]
    pub fn ycbcr(&self, rgb: [u8; 3]) -> [u8; 3] {
        let [y, cb, cr] = self.ycbcr_f32(
            rgb[0] as f32 / 255.0,
            rgb[1] as f32 / 255.0,
            rgb[2] as f32 / 255.0,
        );
        [round_u8(y), round_u8(cb), round_u8(cr)]
    }

    /// R'G'B' from 0.0 to 1.0 (not clipped) of coded Y', Cb and Cr values.
    #[inline]
    pub fn rgb_f32(&self, y: f32, cb: f32, cr: f32) -> [f32; 3] {
        let y = (y - self.y_offset) / self.y_scale;
        let cb = (cb - 128.0) / self.c_scale;
        let cr = (cr - 128.0) / self.c_scale;
        [
            y + self.rv * cr,
            y - self.gu * cb - self.gv * cr,
            y + self.bu * cb,
        ]
    }

    /// The 8-bit R'G'B' of coded Y', Cb and Cr values, rounded and saturated.
    #[inline]
    pub fn rgb(&self, ycc: [u8; 3]) -> [u8; 3] {
        let [r, g, b] = self.rgb_f32(ycc[0] as f32, ycc[1] as f32, ycc[2] as f32);
        [
            round_u8(r * 255.0),
            round_u8(g * 255.0),
            round_u8(b * 255.0),
        ]
    }

    /// Converts a row of interleaved Y'CbCr pixels to interleaved RGB. If the two rows differ in pixel count, the shorter one
    /// sets the count. It allocates nothing.
    pub fn rgb_row(&self, ycc: &[u8], rgb: &mut [u8]) {
        for (s, d) in ycc
            .as_chunks::<3>()
            .0
            .iter()
            .zip(rgb.as_chunks_mut::<3>().0)
        {
            d.copy_from_slice(&self.rgb([s[0], s[1], s[2]]));
        }
    }

    /// Converts a row of interleaved RGB pixels to interleaved Y'CbCr. If the two rows differ in pixel count, the shorter one
    /// sets the count. It allocates nothing.
    pub fn ycbcr_row(&self, rgb: &[u8], ycc: &mut [u8]) {
        for (s, d) in rgb
            .as_chunks::<3>()
            .0
            .iter()
            .zip(ycc.as_chunks_mut::<3>().0)
        {
            d.copy_from_slice(&self.ycbcr([s[0], s[1], s[2]]));
        }
    }
}

/// The weight of B' - Y' in the analogue U signal (ITU-R BT.470-6 Table 2 and BT.1700: `E'U = 0.493 (E'B - E'Y)`).
pub const KU: f32 = 0.493;
/// The weight of R' - Y' in the analogue V signal (ITU-R BT.470-6 Table 2 and BT.1700: `E'V = 0.877 (E'R - E'Y)`).
pub const KV: f32 = 0.877;

/// Y', U and V of R'G'B' values from 0.0 to 1.0, with the BT.601 luma weights (the PAL and NTSC colour-difference signals).
pub fn yuv_from_rgb(r: f32, g: f32, b: f32) -> [f32; 3] {
    let y = BT601.kr * r + (1.0 - BT601.kr - BT601.kb) * g + BT601.kb * b;
    [y, KU * (b - y), KV * (r - y)]
}

/// R'G'B' of Y' and the two colour differences B' - Y' and R' - Y' (not U and V: divide those by [`KU`] and [`KV`] first), with
/// the luma weights of `matrix`.
pub fn rgb_from_differences(matrix: Matrix, y: f32, b_y: f32, r_y: f32) -> [f32; 3] {
    let kg = 1.0 - matrix.kr - matrix.kb;
    [
        y + r_y,
        y - (matrix.kr * r_y + matrix.kb * b_y) / kg,
        y + b_y,
    ]
}
