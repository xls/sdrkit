//! Bitmap fonts and text drawing for generators of test pictures: the 5 x 7 font, the Hell 5 x 5 font, and a TrueType
//! reader for a font the user supplies ([`ttf`]).

mod fixed5x7;
pub mod hell;
pub mod ttf;

use crate::Raster;

/// One glyph: up to 8 rows of up to 8 pixels. Bit `width - 1 - x` of `rows[y]` is pixel (x, y), with row 0 at the top.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Glyph {
    /// The width in pixels (1 to 8).
    pub width: u8,
    /// The height in pixels (1 to 8).
    pub height: u8,
    /// The rows, top first.
    pub rows: [u8; 8],
}

impl Glyph {
    /// A glyph from text rows, top first: `#` is ink, any other character is blank. The widest row sets the width. Rows past
    /// 8 and columns past 8 are not used.
    pub const fn from_rows(rows: &[&str]) -> Glyph {
        let mut g = Glyph {
            width: 1,
            height: 0,
            rows: [0; 8],
        };
        let mut y = 0;
        while y < rows.len() && y < 8 {
            let b = rows[y].as_bytes();
            if b.len() > g.width as usize && b.len() <= 8 {
                g.width = b.len() as u8;
            }
            y += 1;
        }
        g.height = y as u8;
        let mut y = 0;
        while y < g.height as usize {
            let b = rows[y].as_bytes();
            let mut x = 0;
            while x < b.len() && x < 8 {
                if b[x] == b'#' {
                    g.rows[y] |= 1 << (g.width as usize - 1 - x);
                }
                x += 1;
            }
            y += 1;
        }
        g
    }

    /// A glyph from 5-bit rows, most significant bit on the left (the usual coding of 5 x 7 character generators).
    pub const fn from_bits5(rows: [u8; 7]) -> Glyph {
        Glyph {
            width: 5,
            height: 7,
            rows: [
                rows[0], rows[1], rows[2], rows[3], rows[4], rows[5], rows[6], 0,
            ],
        }
    }

    /// True when pixel (`x`, `y`) is ink.
    pub fn pixel(&self, x: u8, y: u8) -> bool {
        x < self.width && y < self.height && self.rows[y as usize] >> (self.width - 1 - x) & 1 == 1
    }

    /// Column `x` as bits, bottom row first: bit k is pixel (x, height - 1 - k). Hell sends a glyph column by column like this.
    pub fn column(&self, x: u8) -> u8 {
        let mut c = 0u8;
        for k in 0..self.height {
            if self.pixel(x, self.height - 1 - k) {
                c |= 1 << k;
            }
        }
        c
    }
}

/// A bitmap font.
#[derive(Clone, Copy, Debug)]
pub struct BitmapFont {
    /// Its name.
    pub name: &'static str,
    /// The rows of every glyph.
    pub height: u8,
    /// Blank columns after each glyph.
    pub spacing: u8,
    /// The glyph of a character, or `None` when the font does not cover it.
    pub glyph: fn(char) -> Option<Glyph>,
    /// The glyph drawn for a character the font does not cover.
    pub missing: Glyph,
}

impl BitmapFont {
    /// The glyph of `c`, or the glyph for a missing character.
    pub fn glyph_or_missing(&self, c: char) -> Glyph {
        (self.glyph)(c).unwrap_or(self.missing)
    }
}

/// The 5 x 7 font: capitals, digits and common signs on a 6-pixel pitch. Lower case shows as capitals. A space is blank.
pub const FIXED_5X7: BitmapFont = BitmapFont {
    name: "fixed 5x7",
    height: 7,
    spacing: 1,
    glyph: fixed5x7::glyph,
    missing: Glyph::from_bits5([14, 17, 1, 2, 4, 0, 4]),
};

/// The 5 x 5 Hell font (see [`hell`]). A space is a blank 3 pixels wide. A character it does not cover shows as `?`.
pub const HELL_5X5: BitmapFont = BitmapFont {
    name: "hell 5x5",
    height: 5,
    spacing: 1,
    glyph: hell_or_space,
    missing: Glyph::from_rows(&["###.", "...#", ".##.", "....", ".#.."]),
};

fn hell_or_space(c: char) -> Option<Glyph> {
    if c == ' ' {
        return Some(Glyph {
            width: 3,
            height: 5,
            rows: [0; 8],
        });
    }
    hell::glyph(c)
}

/// The width in pixels of `text` drawn at `scale`, without the spacing after the last glyph.
pub fn text_width(font: &BitmapFont, text: &str, scale: u32) -> u32 {
    let mut w = 0u32;
    for c in text.chars() {
        w += (font.glyph_or_missing(c).width as u32 + font.spacing as u32) * scale;
    }
    w.saturating_sub(font.spacing as u32 * scale)
}

/// Draws `text` with its top-left corner at (`x`, `y`), each font pixel as a `scale` x `scale` square of `colour` (given as
/// for [`Raster::put`]). Pixels outside the raster are not drawn. Returns the width drawn, the same value as [`text_width`].
///
/// ```
/// use sdrkit_codec_image::font::{draw_text, text_width, FIXED_5X7};
/// use sdrkit_codec_image::{PixelFormat, Raster};
///
/// let mut r = Raster::new(PixelFormat::Grey8, 40, 9).unwrap();
/// let w = draw_text(&mut r, &FIXED_5X7, "HI", 1, 1, 1, &[255]);
/// assert_eq!(w, text_width(&FIXED_5X7, "HI", 1));
/// assert_eq!(r.row(1).unwrap()[1], 255); // the top-left pixel of H
/// ```
pub fn draw_text(
    r: &mut Raster,
    font: &BitmapFont,
    text: &str,
    x: i64,
    y: i64,
    scale: u32,
    colour: &[u8],
) -> u32 {
    let s = scale.max(1) as i64;
    let mut cx = x;
    for c in text.chars() {
        let g = font.glyph_or_missing(c);
        for gy in 0..g.height {
            for gx in 0..g.width {
                if g.pixel(gx, gy) {
                    let (px, py) = (cx + gx as i64 * s, y + gy as i64 * s);
                    r.fill_rect(px, py, px + s, py + s, colour);
                }
            }
        }
        cx += (g.width as i64 + font.spacing as i64) * s;
    }
    text_width(font, text, scale.max(1))
}
