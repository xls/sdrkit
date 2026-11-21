//! A small TrueType reader and rasteriser for a font file that the host reads. There is no hinting and no anti-aliasing:
//! a pixel is ink when its centre is inside the outline.
//!
//! Standard: Apple TrueType Reference Manual, OpenType 1.9 `cmap`, `loca`, `glyf`.

use crate::ImageError;

/// A loaded TrueType font.
#[derive(Clone, Debug)]
pub struct TrueTypeFont {
    data: Vec<u8>,
    upem: f64,
    descender: f64,
    long_loca: bool,
    loca: usize,
    glyf: usize,
    cmap: usize,
    cmap_format: u16,
    num_glyphs: u16,
}

/// A glyph drawn into pixels: `width` x `height` bytes, top row first, 255 for ink and 0 for blank.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GlyphMask {
    /// The width in pixels.
    pub width: u32,
    /// The height in pixels.
    pub height: u32,
    /// The pixels, row by row.
    pub data: Vec<u8>,
}

fn u16_at(d: &[u8], o: usize) -> Option<u16> {
    Some(u16::from_be_bytes([*d.get(o)?, *d.get(o + 1)?]))
}
fn i16_at(d: &[u8], o: usize) -> Option<i16> {
    u16_at(d, o).map(|v| v as i16)
}
fn u32_at(d: &[u8], o: usize) -> Option<u32> {
    Some(u32::from_be_bytes([
        *d.get(o)?,
        *d.get(o + 1)?,
        *d.get(o + 2)?,
        *d.get(o + 3)?,
    ]))
}

type Polygon = Vec<(f64, f64)>;

/// What made a font file unusable.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FontFault {
    /// The file does not start with a TrueType version tag (0x00010000 or `true`).
    NotTrueType,
    /// A table that the reader needs is not in the file.
    Missing {
        /// The table tag.
        table: [u8; 4],
    },
    /// A table ends before a field the reader needs.
    Short {
        /// The table tag.
        table: [u8; 4],
    },
    /// The `head` table provides 0 units per em.
    UnitsPerEm,
    /// No Unicode `cmap` subtable of format 4 or 12.
    NoUnicodeMap,
}

impl TrueTypeFont {
    /// Reads a font from the bytes of a `.ttf` file.
    ///
    /// # Errors
    ///
    /// [`ImageError::Font`] with the [`FontFault`]: a file that is not TrueType (a CFF OpenType font, for example), a missing
    /// table, a table that ends early, or no Unicode `cmap` of format 4 or 12.
    pub fn parse(data: Vec<u8>) -> Result<TrueTypeFont, ImageError> {
        let d = &data;
        let short = |table| ImageError::Font(FontFault::Short { table });
        let tag = u32_at(d, 0).ok_or(ImageError::Font(FontFault::NotTrueType))?;
        if tag != 0x0001_0000 && tag != u32::from_be_bytes(*b"true") {
            return Err(ImageError::Font(FontFault::NotTrueType));
        }
        let n = u16_at(d, 4).ok_or(ImageError::Font(FontFault::NotTrueType))? as usize;
        let table = |name: &[u8; 4]| -> Result<usize, ImageError> {
            (0..n)
                .find_map(|i| {
                    let r = 12 + 16 * i;
                    (d.get(r..r + 4)? == name)
                        .then(|| u32_at(d, r + 8).map(|v| v as usize))
                        .flatten()
                })
                .ok_or(ImageError::Font(FontFault::Missing { table: *name }))
        };
        let head = table(b"head")?;
        let upem = u16_at(d, head + 18).ok_or(short(*b"head"))? as f64;
        if upem <= 0.0 {
            return Err(ImageError::Font(FontFault::UnitsPerEm));
        }
        let long_loca = i16_at(d, head + 50).ok_or(short(*b"head"))? == 1;
        let num_glyphs = u16_at(d, table(b"maxp")? + 4).ok_or(short(*b"maxp"))?;
        let descender = table(b"hhea")
            .ok()
            .and_then(|h| i16_at(d, h + 6))
            .map_or(-0.2 * upem, |v| v as f64);
        let cmap_t = table(b"cmap")?;
        let nsub = u16_at(d, cmap_t + 2).ok_or(short(*b"cmap"))? as usize;
        let mut best: Option<(u32, usize, u16)> = None;
        for i in 0..nsub {
            let r = cmap_t + 4 + 8 * i;
            let sub = (|| {
                let (pid, eid) = (u16_at(d, r)?, u16_at(d, r + 2)?);
                let off = cmap_t + u32_at(d, r + 4)? as usize;
                Some((pid, eid, off, u16_at(d, off)?))
            })();
            let (pid, eid, off, fmt) = sub.ok_or(short(*b"cmap"))?;
            let score = match (pid, eid, fmt) {
                (3, 10, 12) | (0, _, 12) => 3,
                (3, 1, 4) | (0, _, 4) => 2,
                _ => 0,
            };
            if score > 0 && best.is_none_or(|b| score > b.0) {
                best = Some((score, off, fmt));
            }
        }
        let (_, cmap, cmap_format) = best.ok_or(ImageError::Font(FontFault::NoUnicodeMap))?;
        Ok(TrueTypeFont {
            upem,
            descender,
            long_loca,
            loca: table(b"loca")?,
            glyf: table(b"glyf")?,
            cmap,
            cmap_format,
            num_glyphs,
            data,
        })
    }

    /// The glyph index of `c`, or `None` when the font has no glyph for it.
    pub fn glyph_index(&self, c: char) -> Option<u16> {
        let d = &self.data;
        let cp = c as u32;
        let o = self.cmap;
        if self.cmap_format == 12 {
            let n = u32_at(d, o + 12)? as usize;
            for g in 0..n {
                let r = o + 16 + 12 * g;
                let (s, e, start) = (u32_at(d, r)?, u32_at(d, r + 4)?, u32_at(d, r + 8)?);
                if (s..=e).contains(&cp) {
                    let g = start.checked_add(cp - s)?;
                    return u16::try_from(g).ok().filter(|&g| g != 0);
                }
            }
            return None;
        }
        if cp > 0xFFFF {
            return None;
        }
        let seg2 = u16_at(d, o + 6)? as usize;
        let ends = o + 14;
        let starts = ends + seg2 + 2;
        let deltas = starts + seg2;
        let ranges = deltas + seg2;
        for i in 0..seg2 / 2 {
            let end = u16_at(d, ends + 2 * i)? as u32;
            if cp > end {
                continue;
            }
            let start = u16_at(d, starts + 2 * i)? as u32;
            if cp < start {
                return None;
            }
            let delta = u16_at(d, deltas + 2 * i)?;
            let ro = u16_at(d, ranges + 2 * i)? as usize;
            let g = if ro == 0 {
                (cp as u16).wrapping_add(delta)
            } else {
                let g = u16_at(d, ranges + 2 * i + ro + 2 * (cp - start) as usize)?;
                if g == 0 {
                    return None;
                }
                g.wrapping_add(delta)
            };
            return (g != 0).then_some(g);
        }
        None
    }

    fn glyph_range(&self, g: u16) -> Option<(usize, usize)> {
        if g >= self.num_glyphs {
            return None;
        }
        let d = &self.data;
        let g = g as usize;
        let (a, b) = if self.long_loca {
            (
                u32_at(d, self.loca + 4 * g)? as usize,
                u32_at(d, self.loca + 4 * g + 4)? as usize,
            )
        } else {
            (
                u16_at(d, self.loca + 2 * g)? as usize * 2,
                u16_at(d, self.loca + 2 * g + 2)? as usize * 2,
            )
        };
        Some((self.glyf + a, self.glyf + b))
    }

    // The contours of a glyph as closed polygons in font units, curves flattened. Composite glyphs nest at most 4 deep.
    fn outline(&self, g: u16, depth: u32, out: &mut Vec<Polygon>, dx: f64, dy: f64) -> Option<()> {
        if depth > 4 {
            return None;
        }
        let d = &self.data;
        let (a, b) = self.glyph_range(g)?;
        if b <= a {
            return Some(());
        }
        let nc = i16_at(d, a)?;
        if nc < 0 {
            let mut p = a + 10;
            loop {
                let flags = u16_at(d, p)?;
                let gi = u16_at(d, p + 2)?;
                p += 4;
                let (x, y) = if flags & 1 == 1 {
                    let v = (i16_at(d, p)? as f64, i16_at(d, p + 2)? as f64);
                    p += 4;
                    v
                } else {
                    let v = (*d.get(p)? as i8 as f64, *d.get(p + 1)? as i8 as f64);
                    p += 2;
                    v
                };
                // Scale entries are skipped: the glyphs are drawn unscaled.
                if flags & 8 != 0 {
                    p += 2;
                } else if flags & 0x40 != 0 {
                    p += 4;
                } else if flags & 0x80 != 0 {
                    p += 8;
                }
                self.outline(gi, depth + 1, out, dx + x, dy + y)?;
                if flags & 0x20 == 0 {
                    break;
                }
            }
            return Some(());
        }
        let nc = nc as usize;
        let mut ends = Vec::with_capacity(nc);
        for i in 0..nc {
            ends.push(u16_at(d, a + 10 + 2 * i)? as usize);
        }
        let npts = ends.last().map_or(0, |e| e + 1);
        let ilen = u16_at(d, a + 10 + 2 * nc)? as usize;
        let mut p = a + 12 + 2 * nc + ilen;
        let mut flags = Vec::with_capacity(npts);
        while flags.len() < npts {
            let f = *d.get(p)?;
            p += 1;
            flags.push(f);
            if f & 8 != 0 {
                let r = *d.get(p)?;
                p += 1;
                for _ in 0..r {
                    flags.push(f);
                }
            }
        }
        let coords = |short: u8, same: u8, p: &mut usize| -> Option<Vec<f64>> {
            let mut v = 0i32;
            let mut out = Vec::with_capacity(npts);
            for &f in &flags[..npts] {
                if f & short != 0 {
                    let b = *d.get(*p)? as i32;
                    *p += 1;
                    v += if f & same != 0 { b } else { -b };
                } else if f & same == 0 {
                    v += i16_at(d, *p)? as i32;
                    *p += 2;
                }
                out.push(v as f64);
            }
            Some(out)
        };
        let xs = coords(2, 16, &mut p)?;
        let ys = coords(4, 32, &mut p)?;
        let mut start = 0;
        for &e in &ends {
            if e < start || e >= npts {
                return None;
            }
            let pts: Vec<(f64, f64, bool)> = (start..=e)
                .map(|i| (xs[i] + dx, ys[i] + dy, flags[i] & 1 == 1))
                .collect();
            start = e + 1;
            // Implied on-curve points between two off-curve points.
            let mut full = Vec::with_capacity(pts.len() * 2);
            for i in 0..pts.len() {
                let (a, b) = (pts[i], pts[(i + 1) % pts.len()]);
                full.push(a);
                if !a.2 && !b.2 {
                    full.push(((a.0 + b.0) / 2.0, (a.1 + b.1) / 2.0, true));
                }
            }
            let first_on = full.iter().position(|p| p.2).unwrap_or(0);
            full.rotate_left(first_on);
            let mut poly = vec![(full[0].0, full[0].1)];
            let mut i = 1;
            while i <= full.len() {
                let p = full[i % full.len()];
                if p.2 {
                    poly.push((p.0, p.1));
                    i += 1;
                } else {
                    let e = full[(i + 1) % full.len()];
                    let s = poly[poly.len() - 1];
                    for k in 1..=4 {
                        let t = k as f64 / 4.0;
                        let u = 1.0 - t;
                        poly.push((
                            u * u * s.0 + 2.0 * u * t * p.0 + t * t * e.0,
                            u * u * s.1 + 2.0 * u * t * p.1 + t * t * e.1,
                        ));
                    }
                    i += 2;
                }
            }
            out.push(poly);
        }
        Some(())
    }

    /// Draws `c` into a mask `height` pixels tall (1 to 256): the font's descender is at the bottom row and one em fills the
    /// height. The mask is as wide as the glyph's ink (at most 4 x `height`). It returns `None` when the font has no glyph for
    /// `c` or the glyph has no outline (a space).
    pub fn render(&self, c: char, height: u32) -> Option<GlyphMask> {
        let height = height.clamp(1, 256);
        let g = self.glyph_index(c)?;
        let mut polys = Vec::new();
        self.outline(g, 0, &mut polys, 0.0, 0.0)?;
        if polys.is_empty() {
            return None;
        }
        let s = height as f64 / self.upem;
        let (mut x0, mut x1) = (f64::MAX, f64::MIN);
        for &(x, _) in polys.iter().flatten() {
            x0 = x0.min(x);
            x1 = x1.max(x);
        }
        let w = (((x1 - x0) * s).ceil() as u32).clamp(1, 4 * height);
        let mut data = vec![0u8; (w * height) as usize];
        for row in 0..height {
            // Row 0 is the top: font y grows upwards from the descender.
            let py = self.descender + ((height - 1 - row) as f64 + 0.5) / s;
            for cx in 0..w {
                let px = x0 + (cx as f64 + 0.5) / s;
                if winding(&polys, px, py) != 0 {
                    data[(row * w + cx) as usize] = 255;
                }
            }
        }
        Some(GlyphMask {
            width: w,
            height,
            data,
        })
    }
}

fn winding(polys: &[Polygon], x: f64, y: f64) -> i32 {
    let mut w = 0;
    for p in polys {
        for i in 0..p.len() {
            let (a, b) = (p[i], p[(i + 1) % p.len()]);
            let side = (b.0 - a.0) * (y - a.1) - (x - a.0) * (b.1 - a.1);
            if a.1 <= y {
                if b.1 > y && side > 0.0 {
                    w += 1;
                }
            } else if b.1 <= y && side < 0.0 {
                w -= 1;
            }
        }
    }
    w
}
