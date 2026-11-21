//! Geometry: quarter turns, mirror images, thumbnails, and the fractional row shift that straightens a slanted picture.

use crate::Raster;

/// Turns a picture by `turns` quarter turns clockwise (a negative number turns counter-clockwise). One turn maps (x, y) to
/// (h - 1 - y, x). An odd number of turns swaps the width and the height.
///
/// ```
/// use sdrkit_codec_image::{geom, PixelFormat, Raster};
///
/// let r = Raster::from_vec(PixelFormat::Grey8, 3, 2, vec![1, 2, 3, 4, 5, 6]).unwrap();
/// let t = geom::rotate(&r, 1); // one quarter turn clockwise
/// assert_eq!((t.width(), t.height()), (2, 3));
/// assert_eq!(t.data(), &[4, 1, 5, 2, 6, 3]);
/// ```
pub fn rotate(r: &Raster, turns: i32) -> Raster {
    let t = turns.rem_euclid(4);
    if t == 0 {
        return r.clone();
    }
    let (w, h) = (r.width() as usize, r.height() as usize);
    let bpp = r.bytes_per_pixel();
    let (ow, oh) = if t % 2 == 1 { (h, w) } else { (w, h) };
    let mut raster = Raster::zeroed(r.format(), ow as u32, oh as u32);
    let out = raster.data_mut();
    let src = r.data();
    for y in 0..h {
        for x in 0..w {
            let (nx, ny) = match t {
                1 => (h - 1 - y, x),
                2 => (w - 1 - x, h - 1 - y),
                _ => (y, w - 1 - x),
            };
            let s = (y * w + x) * bpp;
            let d = (ny * ow + nx) * bpp;
            out[d..d + bpp].copy_from_slice(&src[s..s + bpp]);
        }
    }
    raster
}

/// The mirror image left to right.
pub fn flip_horizontal(r: &Raster) -> Raster {
    let mut out = r.clone();
    let bpp = r.bytes_per_pixel();
    for y in 0..r.height() {
        if let Some(row) = out.row_mut(y) {
            let w = row.len() / bpp.max(1);
            for x in 0..w / 2 {
                for k in 0..bpp {
                    row.swap(x * bpp + k, (w - 1 - x) * bpp + k);
                }
            }
        }
    }
    out
}

/// The mirror image top to bottom.
pub fn flip_vertical(r: &Raster) -> Raster {
    let mut out = r.clone();
    let s = r.stride();
    let h = r.height() as usize;
    let d = out.data_mut();
    for y in 0..h / 2 {
        let (a, b) = d.split_at_mut((h - 1 - y) * s);
        a[y * s..(y + 1) * s].swap_with_slice(&mut b[..s]);
    }
    out
}

/// A smaller copy that fits in `max_w` by `max_h` pixels and keeps the shape. Each output pixel is the mean of the input
/// pixels it covers, with partial pixels weighted by their area (a box filter). A picture that already fits is returned
/// sharing its bytes. A size of 0 returns a 1 x 1 picture.
pub fn thumbnail(r: &Raster, max_w: u32, max_h: u32) -> Raster {
    let (w, h) = (r.width().max(1), r.height().max(1));
    if r.width() <= max_w && r.height() <= max_h {
        return r.clone();
    }
    let s = (max_w as f64 / w as f64).min(max_h as f64 / h as f64);
    let ow = ((w as f64 * s).round() as u32).clamp(1, w);
    let oh = ((h as f64 * s).round() as u32).clamp(1, h);
    resize_box(r, ow, oh)
}

/// The picture scaled down to `ow` by `oh` pixels with an area-weighted box filter. Sizes above the input size are cut to it.
pub fn resize_box(r: &Raster, ow: u32, oh: u32) -> Raster {
    let (w, h) = (r.width() as usize, r.height() as usize);
    let (ow, oh) = (
        ow.clamp(1, w.max(1) as u32) as usize,
        oh.clamp(1, h.max(1) as u32) as usize,
    );
    let bpp = r.bytes_per_pixel();
    if w == 0 || h == 0 {
        return r.clone();
    }
    let sx = w as f64 / ow as f64;
    let sy = h as f64 / oh as f64;
    let mut raster = Raster::zeroed(r.format(), ow as u32, oh as u32);
    let out = raster.data_mut();
    let mut acc = vec![0f64; bpp];
    let src = r.data();
    for oy in 0..oh {
        let (y0, y1) = (oy as f64 * sy, (oy + 1) as f64 * sy);
        for ox in 0..ow {
            let (x0, x1) = (ox as f64 * sx, (ox + 1) as f64 * sx);
            acc.iter_mut().for_each(|a| *a = 0.0);
            let mut area = 0.0;
            let mut y = y0.floor() as usize;
            while (y as f64) < y1 && y < h {
                let wy = (y1.min(y as f64 + 1.0) - y0.max(y as f64)).max(0.0);
                let mut x = x0.floor() as usize;
                while (x as f64) < x1 && x < w {
                    let wx = (x1.min(x as f64 + 1.0) - x0.max(x as f64)).max(0.0);
                    let a = wx * wy;
                    let i = (y * w + x) * bpp;
                    for k in 0..bpp {
                        acc[k] += a * src[i + k] as f64;
                    }
                    area += a;
                    x += 1;
                }
                y += 1;
            }
            let d = (oy * ow + ox) * bpp;
            for k in 0..bpp {
                out[d + k] = (acc[k] / area.max(1e-12)).round().clamp(0.0, 255.0) as u8;
            }
        }
    }
    raster
}

/// Shifts a row of grey pixels left by `px` pixels, with linear interpolation between pixels, wrapping round at the ends. A
/// negative `px` shifts right. `scratch` holds a copy of the row: give the same vector each time and nothing is allocated
/// once it has the row's size. A row of colour pixels is shifted with [`shift_row_pixels`].
pub fn shift_row(row: &mut [u8], px: f64, scratch: &mut Vec<u8>) {
    shift_row_pixels(row, 1, px, scratch)
}

/// [`shift_row`] for pixels of `bpp` bytes each.
pub fn shift_row_pixels(row: &mut [u8], bpp: usize, px: f64, scratch: &mut Vec<u8>) {
    let bpp = bpp.max(1);
    let n = (row.len() / bpp) as isize;
    if n == 0 || !px.is_finite() || px.abs() < 1e-9 {
        return;
    }
    scratch.clear();
    scratch.extend_from_slice(row);
    let base = px.floor();
    let f = px - base;
    let base = base as isize;
    for x in 0..n {
        let i = (x + base).rem_euclid(n) as usize;
        let j = (x + base + 1).rem_euclid(n) as usize;
        for k in 0..bpp {
            let a = scratch[i * bpp + k] as f64;
            let b = scratch[j * bpp + k] as f64;
            row[x as usize * bpp + k] = (a * (1.0 - f) + b * f).round() as u8;
        }
    }
}
