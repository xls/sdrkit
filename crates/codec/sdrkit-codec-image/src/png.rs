//! PNG files through the `png` crate: [`encode`] writes 8-bit grey, RGB or RGBA, and [`decode`] reads any PNG into 8-bit
//! grey or BGRA.
//!
//! Standard: ISO/IEC 15948 (W3C PNG Second Edition).

use crate::{ImageError, PixelFormat, Raster};
use sdrkit_core::media::Picture;

/// The 8 bytes every PNG file starts with (PNG Second Edition clause 5.2).
pub const SIGNATURE: [u8; 8] = [0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];

/// Why the `png` crate refused a file or a picture.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PngFault {
    /// The file breaks the PNG format (a bad signature, chunk, CRC or compressed data), or it is cut short.
    Format,
    /// The picture passes the size limits of the decoder.
    Limits,
    /// The encoder refused a parameter (a size of 0, for example).
    Parameter,
}

fn decode_fault(e: png::DecodingError) -> ImageError {
    ImageError::Png(match e {
        png::DecodingError::LimitsExceeded => PngFault::Limits,
        png::DecodingError::Parameter(_) => PngFault::Parameter,
        png::DecodingError::IoError(_) | png::DecodingError::Format(_) => PngFault::Format,
    })
}

fn encode_fault(e: png::EncodingError) -> ImageError {
    ImageError::Png(match e {
        png::EncodingError::LimitsExceeded => PngFault::Limits,
        png::EncodingError::IoError(_) | png::EncodingError::Format(_) => PngFault::Format,
        png::EncodingError::Parameter(_) => PngFault::Parameter,
    })
}

/// The PNG file of a raster. Grey stays grey. BGRA becomes RGB when every alpha byte is 255, else RGBA.
///
/// ```
/// use sdrkit_codec_image::{png, PixelFormat, Raster};
///
/// let r = Raster::from_vec(PixelFormat::Grey8, 2, 2, vec![0, 85, 170, 255]).unwrap();
/// let bytes = png::encode(&r).unwrap();
/// assert_eq!(&bytes[..8], &png::SIGNATURE);
/// assert_eq!(png::decode(&bytes).unwrap(), r);
/// ```
pub fn encode(r: &Raster) -> Result<Vec<u8>, ImageError> {
    let mut out = Vec::new();
    encode_into(r, &mut out)?;
    Ok(out)
}

/// The same as [`encode`], into `out` (cleared first).
pub fn encode_into(r: &Raster, out: &mut Vec<u8>) -> Result<(), ImageError> {
    out.clear();
    let mut enc = png::Encoder::new(&mut *out, r.width(), r.height());
    enc.set_depth(png::BitDepth::Eight);
    let colour;
    let data: &[u8] = if r.format() == PixelFormat::Grey8 {
        enc.set_color(png::ColorType::Grayscale);
        r.data()
    } else if r.data().as_chunks::<4>().0.iter().all(|c| c[3] == 255) {
        enc.set_color(png::ColorType::Rgb);
        colour = r.rgb().flatten().collect::<Vec<u8>>();
        &colour
    } else {
        enc.set_color(png::ColorType::Rgba);
        colour = r
            .data()
            .as_chunks::<4>()
            .0
            .iter()
            .flat_map(|c| [c[2], c[1], c[0], c[3]])
            .collect::<Vec<u8>>();
        &colour
    };
    let mut w = enc.write_header().map_err(encode_fault)?;
    w.write_image_data(data).map_err(encode_fault)?;
    w.finish().map_err(encode_fault)
}

/// The PNG file of a raw picture item (grey or BGRA).
pub fn encode_picture(p: &Picture) -> Result<Vec<u8>, ImageError> {
    encode(&Raster::from_picture(p)?)
}

/// Reads a PNG file. Grey without alpha is returned as [`PixelFormat::Grey8`], everything else as [`PixelFormat::Bgra`] (alpha
/// 255 when the file has none). Palettes are expanded and 16-bit samples keep their high byte. A damaged or truncated file
/// returns an error.
pub fn decode(bytes: &[u8]) -> Result<Raster, ImageError> {
    let mut d = png::Decoder::new(std::io::Cursor::new(bytes));
    d.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
    let mut reader = d.read_info().map_err(decode_fault)?;
    let (w, h) = (reader.info().width, reader.info().height);
    let size = reader.output_buffer_size().ok_or(ImageError::TooLarge {
        width: w,
        height: h,
    })?;
    if size > crate::raster::MAX_BYTES {
        return Err(ImageError::TooLarge {
            width: w,
            height: h,
        });
    }
    let (colour, _) = reader.output_color_type();
    if colour == png::ColorType::Grayscale && size == w as usize * h as usize {
        let mut r = Raster::new(PixelFormat::Grey8, w, h)?;
        reader.next_frame(r.data_mut()).map_err(decode_fault)?;
        return Ok(r);
    }
    let mut buf = vec![0u8; size];
    let info = reader.next_frame(&mut buf).map_err(decode_fault)?;
    let line = info.line_size;
    let format = if info.color_type == png::ColorType::Grayscale {
        PixelFormat::Grey8
    } else {
        PixelFormat::Bgra
    };
    let mut r = Raster::new(format, w, h)?;
    let out_row = r.stride();
    for (src, dst) in buf
        .chunks(line.max(1))
        .zip(r.data_mut().chunks_exact_mut(out_row.max(1)))
    {
        match info.color_type {
            png::ColorType::Grayscale => dst.copy_from_slice(&src[..out_row]),
            png::ColorType::GrayscaleAlpha => {
                for (d, c) in dst
                    .as_chunks_mut::<4>()
                    .0
                    .iter_mut()
                    .zip(src.as_chunks::<2>().0)
                {
                    *d = [c[0], c[0], c[0], c[1]];
                }
            }
            png::ColorType::Rgb => {
                for (d, c) in dst
                    .as_chunks_mut::<4>()
                    .0
                    .iter_mut()
                    .zip(src.as_chunks::<3>().0)
                {
                    *d = [c[2], c[1], c[0], 255];
                }
            }
            png::ColorType::Rgba => {
                for (d, c) in dst
                    .as_chunks_mut::<4>()
                    .0
                    .iter_mut()
                    .zip(src.as_chunks::<4>().0)
                {
                    *d = [c[2], c[1], c[0], c[3]];
                }
            }
            png::ColorType::Indexed => return Err(ImageError::Png(PngFault::Format)),
        }
    }
    Ok(r)
}
