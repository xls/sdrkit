//! [`LineImage`]: a picture that grows one row at a time (weather fax, SSTV, APT, Hell, analogue TV lines).

use crate::geom::shift_row_pixels;
use crate::raster::bytes_per_pixel;
use crate::{ImageError, PixelFormat, Raster};
use sdrkit_core::media::{Picture, PictureState};

/// What happens when a [`LineImage`] holds its most rows and another row arrives.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Overflow {
    /// The row is refused with [`ImageError::Full`] (a fax or SSTV page has an end).
    Stop,
    /// The oldest row goes and the picture scrolls up (a Hell strip, a waterfall).
    Scroll,
}

/// A picture of fixed width that grows one row at a time, up to a set number of rows.
///
/// The rows live in one buffer. [`LineImage::with_capacity`] sizes it at the start, so [`LineImage::push_row`] allocates nothing.
/// Each change makes [`LineImage::version`] grow, so a display can tell when to draw again.
///
/// ```
/// use sdrkit_codec_image::line::{LineImage, Overflow};
/// use sdrkit_codec_image::PixelFormat;
///
/// let mut img = LineImage::with_capacity(PixelFormat::Grey8, 4, 3, Overflow::Scroll).unwrap();
/// for v in [10u8, 20, 30, 40] {
///     img.push_row(&[v; 4]).unwrap();
/// }
/// // Three rows are kept: the first one scrolled away.
/// assert_eq!(img.height(), 3);
/// assert_eq!(img.row(0), Some(&[20u8; 4][..]));
/// let p = img.to_picture("fax", 1, sdrkit_codec_image::PictureState::Growing);
/// assert_eq!(p.data.len(), 12);
/// ```
#[derive(Clone, Debug)]
pub struct LineImage {
    format: PixelFormat,
    width: usize,
    max_rows: usize,
    overflow: Overflow,
    data: Vec<u8>,
    // The physical row that holds row 0, once the buffer scrolls.
    start: usize,
    rows: usize,
    version: u64,
    scratch: Vec<u8>,
}

impl LineImage {
    /// An empty picture of `width` pixels and at most `max_rows` rows. The buffer grows as rows arrive.
    pub fn new(
        format: PixelFormat,
        width: u32,
        max_rows: u32,
        overflow: Overflow,
    ) -> Result<LineImage, ImageError> {
        Self::build(format, width, max_rows, overflow, false)
    }

    /// The same as [`LineImage::new`], with the buffer for all `max_rows` rows made now, so pushing rows allocates nothing.
    pub fn with_capacity(
        format: PixelFormat,
        width: u32,
        max_rows: u32,
        overflow: Overflow,
    ) -> Result<LineImage, ImageError> {
        Self::build(format, width, max_rows, overflow, true)
    }

    fn build(
        format: PixelFormat,
        width: u32,
        max_rows: u32,
        overflow: Overflow,
        reserve: bool,
    ) -> Result<LineImage, ImageError> {
        if width == 0 || max_rows == 0 {
            return Err(ImageError::TooLarge {
                width,
                height: max_rows,
            });
        }
        let bpp = bytes_per_pixel(format).ok_or(ImageError::NotPacked { pixel: format })?;
        let stride = width as usize * bpp;
        let total = stride
            .checked_mul(max_rows as usize)
            .filter(|&n| n <= crate::raster::MAX_BYTES)
            .ok_or(ImageError::TooLarge {
                width,
                height: max_rows,
            })?;
        Ok(LineImage {
            format,
            width: width as usize,
            max_rows: max_rows as usize,
            overflow,
            data: Vec::with_capacity(if reserve { total } else { 0 }),
            start: 0,
            rows: 0,
            version: 0,
            scratch: Vec::with_capacity(if reserve { stride } else { 0 }),
        })
    }

    fn stride(&self) -> usize {
        self.width * self.bpp()
    }

    fn bpp(&self) -> usize {
        bytes_per_pixel(self.format).unwrap_or(1)
    }

    fn physical(&self, i: usize) -> usize {
        (self.start + i) % self.max_rows
    }

    /// The pixel format.
    pub fn format(&self) -> PixelFormat {
        self.format
    }
    /// The width in pixels.
    pub fn width(&self) -> u32 {
        self.width as u32
    }
    /// The rows held now.
    pub fn height(&self) -> u32 {
        self.rows as u32
    }
    /// The most rows held.
    pub fn max_rows(&self) -> u32 {
        self.max_rows as u32
    }
    /// A number that grows with every change.
    pub fn version(&self) -> u64 {
        self.version
    }
    /// True when no row has arrived.
    pub fn is_empty(&self) -> bool {
        self.rows == 0
    }

    /// Adds a row at the bottom. If `row` is not exactly one row of pixels, it returns an error. If the picture is full, it
    /// scrolls ([`Overflow::Scroll`]) or returns [`ImageError::Full`] ([`Overflow::Stop`]).
    pub fn push_row(&mut self, row: &[u8]) -> Result<(), ImageError> {
        let s = self.stride();
        if row.len() != s {
            return Err(ImageError::WrongLength {
                expected: s,
                got: row.len(),
            });
        }
        self.next_row()?.copy_from_slice(row);
        Ok(())
    }

    /// Adds a row at the bottom and returns it to fill in place (its old content is not defined). The rules of
    /// [`LineImage::push_row`] apply.
    pub fn next_row(&mut self) -> Result<&mut [u8], ImageError> {
        let s = self.stride();
        let slot = if self.rows < self.max_rows {
            let p = self.physical(self.rows);
            if self.data.len() < (p + 1) * s {
                self.data.resize((p + 1) * s, 0);
            }
            self.rows += 1;
            p
        } else {
            match self.overflow {
                Overflow::Stop => return Err(ImageError::Full),
                Overflow::Scroll => {
                    let p = self.start;
                    self.start = (self.start + 1) % self.max_rows;
                    p
                }
            }
        };
        self.version += 1;
        Ok(&mut self.data[slot * s..(slot + 1) * s])
    }

    /// Row `i` (0 is the top), or `None`.
    pub fn row(&self, i: u32) -> Option<&[u8]> {
        let i = i as usize;
        (i < self.rows).then(|| {
            let (p, s) = (self.physical(i), self.stride());
            &self.data[p * s..(p + 1) * s]
        })
    }

    /// Row `i` to change it, or `None`.
    pub fn row_mut(&mut self, i: u32) -> Option<&mut [u8]> {
        let i = i as usize;
        if i >= self.rows {
            return None;
        }
        self.version += 1;
        let (p, s) = (self.physical(i), self.stride());
        Some(&mut self.data[p * s..(p + 1) * s])
    }

    /// Shifts row `i` left by `px` pixels with linear interpolation, wrapping at the ends (a negative `px` shifts right). It
    /// allocates nothing once a row has been shifted.
    pub fn shift_row(&mut self, i: u32, px: f64) {
        let bpp = self.bpp();
        let (p, s) = (self.physical(i as usize), self.stride());
        if (i as usize) < self.rows {
            shift_row_pixels(
                &mut self.data[p * s..(p + 1) * s],
                bpp,
                px,
                &mut self.scratch,
            );
            self.version += 1;
        }
    }

    /// Straightens a slant: row `i` shifts left by `i` x `px_per_row` pixels. A fax decoder calls this when its line period
    /// estimate changes by `dP` samples, with `px_per_row = dP / samples per pixel`.
    pub fn deslant(&mut self, px_per_row: f64) {
        for i in 0..self.rows as u32 {
            self.shift_row(i, i as f64 * px_per_row);
        }
    }

    /// Turns every row left by `px` whole pixels (moves the fax margin to the edge).
    pub fn roll(&mut self, px: usize) {
        let bpp = self.bpp();
        let (s, n) = (self.stride(), self.rows);
        let k = (px % self.width) * bpp;
        if k == 0 {
            return;
        }
        for i in 0..n {
            let p = self.physical(i);
            self.data[p * s..(p + 1) * s].rotate_left(k);
        }
        self.version += 1;
    }

    /// Removes every row. The buffer is kept.
    pub fn clear(&mut self) {
        self.rows = 0;
        self.start = 0;
        self.version += 1;
    }

    /// Copies the rows, top first, into `out` (cleared first). With a reused vector, nothing is allocated once it has the size.
    pub fn copy_to(&self, out: &mut Vec<u8>) {
        out.clear();
        for i in 0..self.rows as u32 {
            if let Some(r) = self.row(i) {
                out.extend_from_slice(r);
            }
        }
    }

    /// The rows so far as a [`Raster`]. The rows are copied once.
    pub fn to_raster(&self) -> Raster {
        let mut r = Raster::zeroed(self.format, self.width as u32, self.rows as u32);
        let s = self.stride();
        for (i, d) in r.data_mut().chunks_exact_mut(s.max(1)).enumerate() {
            if let Some(row) = self.row(i as u32) {
                d.copy_from_slice(row);
            }
        }
        r
    }

    /// The rows so far as the standard picture item of raw pixels, named `name`, with the number `id` and the state
    /// `state`. The rows are copied once.
    pub fn to_picture(&self, name: &str, id: u32, state: PictureState) -> Picture {
        self.to_raster().to_picture(name, id, state)
    }
}
