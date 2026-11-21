//! [`Raster`]: an uncompressed picture held in memory, and its conversion to and from the standard [`Picture`] item.

use crate::ImageError;
use sdrkit_core::media::{Codec, Picture, PixelFormat, PixelLayout};
use std::sync::Arc;

/// The largest number of pixel bytes a [`Raster`] holds (1 GiB). A larger size is refused with [`ImageError::TooLarge`].
pub const MAX_BYTES: usize = 1 << 30;

/// The bytes of one pixel of a packed format that a [`Raster`] holds: 1 for [`PixelFormat::Grey8`], 4 for
/// [`PixelFormat::Bgra`]. Other formats are not packed with whole bytes per pixel and give `None`.
pub const fn bytes_per_pixel(format: PixelFormat) -> Option<usize> {
    match format {
        PixelFormat::Grey8 => Some(1),
        PixelFormat::Bgra => Some(4),
        PixelFormat::Nv12
        | PixelFormat::Nv12Full
        | PixelFormat::I420
        | PixelFormat::Yuy2
        | PixelFormat::Rgb8 => None,
    }
}

/// An uncompressed picture: rows of pixels, top row first, packed with no padding, in [`PixelFormat::Grey8`] or
/// [`PixelFormat::Bgra`]. The pixel bytes are shared: [`Raster::to_picture`] and [`Clone`] do not copy them.
///
/// ```
/// use sdrkit_codec_image::{PixelFormat, Raster};
///
/// let mut r = Raster::new(PixelFormat::Grey8, 4, 2).unwrap();
/// r.row_mut(1).unwrap().fill(200);
/// let p = r.to_picture("test");
/// assert_eq!(&p.data[4..], &[200; 4]);
/// ```
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Raster {
    format: PixelFormat,
    width: u32,
    height: u32,
    data: Arc<[u8]>,
}

fn byte_len(format: PixelFormat, width: u32, height: u32) -> Result<usize, ImageError> {
    let bpp = bytes_per_pixel(format).ok_or(ImageError::NotPacked { pixel: format })?;
    (width as usize)
        .checked_mul(height as usize)
        .and_then(|n| n.checked_mul(bpp))
        .filter(|&n| n <= MAX_BYTES)
        .ok_or(ImageError::TooLarge { width, height })
}

impl Raster {
    /// A picture of `width` by `height` pixels, all bytes 0 (black, and transparent for [`PixelFormat::Bgra`]). A format that
    /// is not packed returns [`ImageError::NotPacked`].
    pub fn new(format: PixelFormat, width: u32, height: u32) -> Result<Raster, ImageError> {
        let n = byte_len(format, width, height)?;
        Ok(Raster {
            format,
            width,
            height,
            data: std::iter::repeat_n(0u8, n).collect(),
        })
    }

    /// A picture of the pixel bytes `data`, copied once into the shared buffer. If `data` does not hold exactly `width` x
    /// `height` pixels, it is refused.
    pub fn from_vec(
        format: PixelFormat,
        width: u32,
        height: u32,
        data: Vec<u8>,
    ) -> Result<Raster, ImageError> {
        let n = byte_len(format, width, height)?;
        if data.len() != n {
            return Err(ImageError::WrongLength {
                expected: n,
                got: data.len(),
            });
        }
        Ok(Raster {
            format,
            width,
            height,
            data: data.into(),
        })
    }

    /// A zeroed raster whose size the caller has made sure of: a packed format and at most [`MAX_BYTES`].
    pub(crate) fn zeroed(format: PixelFormat, width: u32, height: u32) -> Raster {
        let n = width as usize * height as usize * bytes_per_pixel(format).unwrap_or(1);
        debug_assert!(bytes_per_pixel(format).is_some() && n <= MAX_BYTES);
        Raster {
            format,
            width,
            height,
            data: std::iter::repeat_n(0u8, n).collect(),
        }
    }

    /// A BGRA picture of `width` x `height` pixels given as red, green and blue bytes (alpha 255). If `rgb` does not hold
    /// exactly that many pixels, it is refused.
    pub fn from_rgb(width: u32, height: u32, rgb: &[u8]) -> Result<Raster, ImageError> {
        let mut r = Raster::new(PixelFormat::Bgra, width, height)?;
        let n = width as usize * height as usize;
        if rgb.len() != n * 3 {
            return Err(ImageError::WrongLength {
                expected: n * 3,
                got: rgb.len(),
            });
        }
        let out = r.data_mut();
        for (d, s) in out
            .as_chunks_mut::<4>()
            .0
            .iter_mut()
            .zip(rgb.as_chunks::<3>().0)
        {
            *d = [s[2], s[1], s[0], 255];
        }
        Ok(r)
    }

    /// The pixel format.
    pub fn format(&self) -> PixelFormat {
        self.format
    }
    /// The width in pixels.
    pub fn width(&self) -> u32 {
        self.width
    }
    /// The height in pixels.
    pub fn height(&self) -> u32 {
        self.height
    }
    /// The bytes of one pixel.
    pub fn bytes_per_pixel(&self) -> usize {
        bytes_per_pixel(self.format).unwrap_or(1)
    }
    /// The bytes of one row.
    pub fn stride(&self) -> usize {
        self.width as usize * self.bytes_per_pixel()
    }
    /// All pixel bytes, row by row.
    pub fn data(&self) -> &[u8] {
        &self.data
    }
    /// All pixel bytes, to change them. If a [`Picture`] or a clone still shares the bytes, they are copied first.
    pub fn data_mut(&mut self) -> &mut [u8] {
        Arc::make_mut(&mut self.data)
    }
    /// The shared pixel bytes.
    pub fn shared(&self) -> &Arc<[u8]> {
        &self.data
    }
    /// Row `y`, or `None` below the last row.
    pub fn row(&self, y: u32) -> Option<&[u8]> {
        let s = self.stride();
        (y < self.height).then(|| &self.data[y as usize * s..(y as usize + 1) * s])
    }
    /// Row `y` to change it, or `None` below the last row.
    pub fn row_mut(&mut self, y: u32) -> Option<&mut [u8]> {
        let s = self.stride();
        let h = self.height;
        (y < h).then(|| &mut self.data_mut()[y as usize * s..(y as usize + 1) * s])
    }

    /// The red, green and blue bytes of each pixel, row by row. Grey produces equal red, green and blue.
    pub fn rgb(&self) -> impl Iterator<Item = [u8; 3]> + '_ {
        let bpp = self.bytes_per_pixel();
        self.data.chunks_exact(bpp).map(move |p| match bpp {
            4 => [p[2], p[1], p[0]],
            _ => [p[0], p[0], p[0]],
        })
    }

    /// Sets pixel (`x`, `y`) to `colour`, given as grey or BGRA to match the format (extra bytes are not used, missing bytes
    /// are 0). A pixel outside the picture is not drawn.
    pub fn put(&mut self, x: i64, y: i64, colour: &[u8]) {
        if x < 0 || y < 0 || x >= self.width as i64 || y >= self.height as i64 {
            return;
        }
        let bpp = self.bytes_per_pixel();
        let i = (y as usize * self.width as usize + x as usize) * bpp;
        for (k, d) in self.data_mut()[i..i + bpp].iter_mut().enumerate() {
            *d = colour.get(k).copied().unwrap_or(0);
        }
    }

    /// Fills the rectangle from (`x0`, `y0`) to (`x1`, `y1`), right and bottom edges not included, cut to the picture.
    pub fn fill_rect(&mut self, x0: i64, y0: i64, x1: i64, y1: i64, colour: &[u8]) {
        let (w, h) = (self.width as i64, self.height as i64);
        let (x0, x1) = (x0.clamp(0, w), x1.clamp(0, w));
        let (y0, y1) = (y0.clamp(0, h), y1.clamp(0, h));
        for y in y0..y1 {
            for x in x0..x1 {
                self.put(x, y, colour);
            }
        }
    }

    /// The same picture in another packed pixel format. Grey becomes equal red, green and blue with alpha 255. Colour becomes
    /// grey by the BT.601 luma weights (ITU-R BT.601-7 clause 2.5.1), and alpha is dropped. A format that is not packed returns
    /// [`ImageError::NotPacked`].
    pub fn convert(&self, to: PixelFormat) -> Result<Raster, ImageError> {
        if to == self.format {
            return Ok(self.clone());
        }
        let mut out = Raster::new(to, self.width, self.height)?;
        let dst = out.data_mut();
        match to {
            PixelFormat::Grey8 => {
                for (d, [r, g, b]) in dst.iter_mut().zip(self.rgb()) {
                    *d = crate::colour::luma_601(r, g, b);
                }
            }
            _ => {
                for (d, [r, g, b]) in dst.as_chunks_mut::<4>().0.iter_mut().zip(self.rgb()) {
                    *d = [b, g, r, 255];
                }
            }
        }
        Ok(out)
    }

    /// The standard picture item of raw pixels, named `name` (no name if empty). The picture shares the pixel bytes.
    pub fn to_picture(&self, name: &str) -> Picture {
        Picture {
            codec: Codec::Pixels(self.format),
            layout: Some(PixelLayout::packed(self.format, self.width, self.height)),
            name: (!name.is_empty()).then(|| name.to_string()),
            data: Arc::clone(&self.data),
        }
    }

    /// A raster of a raw picture item (grey or BGRA, tightly packed or with row padding). A tightly packed picture shares its
    /// bytes. A compressed picture returns [`ImageError::NotPixels`]: decode it first ([`crate::decode`]).
    pub fn from_picture(p: &Picture) -> Result<Raster, ImageError> {
        let Codec::Pixels(format) = p.codec else {
            return Err(ImageError::NotPixels { codec: p.codec });
        };
        let l = p.layout.as_ref().ok_or(ImageError::NoLayout)?;
        let n = byte_len(format, l.width, l.height)?;
        let row = l.width as usize * bytes_per_pixel(format).unwrap_or(1);
        let stride = l.strides.first().copied().unwrap_or(row);
        if stride < row {
            return Err(ImageError::StrideShort { stride, row });
        }
        if stride == row && p.data.len() == n {
            return Ok(Raster {
                format,
                width: l.width,
                height: l.height,
                data: Arc::clone(&p.data),
            });
        }
        let mut out = Raster::new(format, l.width, l.height)?;
        let dst = out.data_mut();
        for y in 0..l.height as usize {
            let src = p
                .data
                .get(y * stride..y * stride + row)
                .ok_or(ImageError::Truncated)?;
            dst[y * row..(y + 1) * row].copy_from_slice(src);
        }
        Ok(out)
    }
}
