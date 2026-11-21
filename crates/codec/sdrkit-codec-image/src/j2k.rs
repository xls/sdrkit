//! JPEG 2000 decoding of a code stream or a JP2 file through `hayro-jpeg2000`, and encoding of a code stream through
//! `j2k-native` (the `jpeg2000-encode` feature).
//!
//! Standard: ITU-T T.800 (ISO/IEC 15444-1).

pub use crate::J2kFault;
use crate::{ImageError, PixelFormat, Raster};

/// The 12 bytes of the JP2 signature box (T.800 Annex I.5.1).
pub const JP2_SIGNATURE: [u8; 12] = [
    0, 0, 0, 0x0C, 0x6A, 0x50, 0x20, 0x20, 0x0D, 0x0A, 0x87, 0x0A,
];
/// The first 4 bytes of a raw code stream: SOC then SIZ (T.800 A.4.1, A.5.1).
pub const CODESTREAM_START: [u8; 4] = [0xFF, 0x4F, 0xFF, 0x51];

/// Where a JP2 file or a raw code stream starts in `data` (a file can come after a product header), and whether it is a JP2
/// file. It returns `None` when neither is found.
pub fn find_stream(data: &[u8]) -> Option<(usize, bool)> {
    if data.starts_with(&JP2_SIGNATURE) {
        return Some((0, true));
    }
    data.windows(4)
        .position(|w| w == CODESTREAM_START)
        .map(|i| (i, false))
}

/// A decoded JPEG 2000 picture at its own precision.
#[derive(Clone, Debug, PartialEq)]
pub struct J2kPicture {
    /// The width in pixels.
    pub width: u32,
    /// The height in pixels.
    pub height: u32,
    /// The bit depth of the first component (1 to 16 in practice).
    pub precision: u8,
    /// The components (one for grey, three for colour), each `width` x `height` samples, row by row, rounded to whole numbers
    /// in the range of the precision.
    pub components: Vec<Vec<u16>>,
}

/// Decodes a JPEG 2000 code stream or JP2 file. Damaged or unsupported data returns [`J2kFault::Damaged`].
pub fn decode(data: &[u8]) -> Result<J2kPicture, ImageError> {
    use hayro_jpeg2000::{DecodeSettings, DecoderContext, Image};
    let err = |_: hayro_jpeg2000::DecodeError| ImageError::J2k(J2kFault::Damaged);
    let img = Image::new(data, &DecodeSettings::default()).map_err(err)?;
    let (width, height) = (img.width(), img.height());
    let n = width as usize * height as usize;
    if n.saturating_mul(4) > crate::raster::MAX_BYTES {
        return Err(ImageError::TooLarge { width, height });
    }
    let mut ctx = DecoderContext::default();
    let decoded = img.decode(&mut ctx).map_err(err)?;
    let comps = decoded.components();
    let precision = comps.first().map_or(8, |c| c.bit_depth());
    let mut components = Vec::with_capacity(comps.len());
    for c in comps {
        let max = ((1u32 << c.bit_depth().min(16)) - 1) as f32;
        let s = c.samples();
        if s.len() < n {
            return Err(ImageError::Truncated);
        }
        components.push(
            s[..n]
                .iter()
                .map(|&v| (v + 0.5).floor().clamp(0.0, max) as u16)
                .collect(),
        );
    }
    Ok(J2kPicture {
        width,
        height,
        precision,
        components,
    })
}

impl J2kPicture {
    /// The picture in 8 bits: each sample scaled from its precision to 0 to 255. One component produces grey, three produce BGRA.
    /// Other counts return [`J2kFault::Components`], and a component of the wrong length [`J2kFault::SampleCount`].
    pub fn to_raster(&self) -> Result<Raster, ImageError> {
        let max = ((1u32 << self.precision.clamp(1, 16)) - 1) as f32;
        let to8 = |v: u16| ((v as f32) * 255.0 / max + 0.5).floor().clamp(0.0, 255.0) as u8;
        let n = self.width as usize * self.height as usize;
        let c = &self.components;
        if c.iter().any(|c| c.len() != n) {
            return Err(ImageError::J2k(J2kFault::SampleCount));
        }
        match c.len() {
            1 => {
                let mut r = Raster::new(PixelFormat::Grey8, self.width, self.height)?;
                for (d, &v) in r.data_mut().iter_mut().zip(&c[0]) {
                    *d = to8(v);
                }
                Ok(r)
            }
            3 => {
                let mut r = Raster::new(PixelFormat::Bgra, self.width, self.height)?;
                for (i, d) in r.data_mut().as_chunks_mut::<4>().0.iter_mut().enumerate() {
                    *d = [to8(c[2][i]), to8(c[1][i]), to8(c[0][i]), 255];
                }
                Ok(r)
            }
            count => Err(ImageError::J2k(J2kFault::Components { count })),
        }
    }
}

/// How [`encode`] codes a picture.
#[cfg(feature = "jpeg2000-encode")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct J2kEncode {
    /// True for the reversible 5-3 wavelet (lossless, T.800 Annex F.3.8), false for the irreversible 9-7 wavelet (lossy).
    pub lossless: bool,
    /// Wavelet decomposition levels, 0 to 32 (T.800 Table A.15). Five is usual.
    pub levels: u8,
}

#[cfg(feature = "jpeg2000-encode")]
impl Default for J2kEncode {
    /// Lossless with five levels, as GK-2A and FY-4 send their image segments.
    fn default() -> Self {
        J2kEncode {
            lossless: true,
            levels: 5,
        }
    }
}

/// Encodes `picture` as a raw JPEG 2000 code stream (from SOC to EOC) with `options`. All components must have
/// `width` x `height` samples within the precision. The precision must be 1 to 16 bits. Other shapes return a [`J2kFault`].
///
/// ```
/// use sdrkit_codec_image::j2k::{self, J2kEncode, J2kPicture};
/// let picture = J2kPicture {
///     width: 16,
///     height: 8,
///     precision: 10,
///     components: vec![(0..128).map(|i| i * 7).collect()],
/// };
/// let stream = j2k::encode(&picture, &J2kEncode::default())?;
/// assert_eq!(stream[..4], j2k::CODESTREAM_START);
/// assert_eq!(j2k::decode(&stream)?, picture);
/// # Ok::<(), sdrkit_codec_image::ImageError>(())
/// ```
#[cfg(feature = "jpeg2000-encode")]
pub fn encode(picture: &J2kPicture, options: &J2kEncode) -> Result<Vec<u8>, ImageError> {
    let (w, h) = (picture.width as usize, picture.height as usize);
    let n = w.saturating_mul(h);
    let comps = &picture.components;
    if !(1..=16).contains(&picture.precision) {
        return Err(ImageError::J2k(J2kFault::Precision {
            bits: picture.precision,
        }));
    }
    if comps.is_empty() || comps.len() > 255 {
        return Err(ImageError::J2k(J2kFault::Components { count: comps.len() }));
    }
    if n == 0 || comps.iter().any(|c| c.len() != n) {
        return Err(ImageError::J2k(J2kFault::SampleCount));
    }
    if options.levels > 32 {
        return Err(ImageError::J2k(J2kFault::Levels {
            levels: options.levels,
        }));
    }
    let max = (1u32 << picture.precision) - 1;
    if comps.iter().flatten().any(|&v| v as u32 > max) {
        return Err(ImageError::J2k(J2kFault::SampleAbovePrecision));
    }
    if n.saturating_mul(comps.len()).saturating_mul(2) > crate::raster::MAX_BYTES {
        return Err(ImageError::TooLarge {
            width: picture.width,
            height: picture.height,
        });
    }
    // The encoder takes samples pixel by pixel, the components of a pixel together: one byte each up to 8 bits, else two bytes
    // little-endian.
    let wide = picture.precision > 8;
    let mut pixels = Vec::with_capacity(n * comps.len() * if wide { 2 } else { 1 });
    for i in 0..n {
        for c in comps {
            if wide {
                pixels.extend_from_slice(&c[i].to_le_bytes());
            } else {
                pixels.push(c[i] as u8);
            }
        }
    }
    let opts = j2k_native::EncodeOptions {
        num_decomposition_levels: options.levels,
        reversible: options.lossless,
        guard_bits: if options.lossless { 1 } else { 2 },
        ..Default::default()
    };
    j2k_native::encode(
        &pixels,
        picture.width,
        picture.height,
        comps.len() as u16,
        picture.precision,
        false,
        &opts,
    )
    .map_err(|_| ImageError::J2k(J2kFault::Encoder))
}
