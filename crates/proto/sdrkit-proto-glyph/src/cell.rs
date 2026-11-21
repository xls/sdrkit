//! 16 × 16 dot cells: a glyph as 16 rows of 16 bits, row 0 the top, bit 15 of a row the left column. The sources: an HZK16 file
//! (the raw 16 × 16 font of GB 2312, 32 bytes a glyph), a TrueType font drawn 16 pixels to the em, and the shared Hell glyphs for
//! Latin, Cyrillic and Greek. A character no source has is drawn as a box.

use crate::gb2312;
use sdrkit_codec_image::font::{self, ttf::TrueTypeFont};

/// A 16 × 16 cell: 16 rows, row 0 the top; bit 15 of a row is column 0, the left column.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Cell(pub [u16; 16]);

impl Cell {
    /// Whether the dot at `row`, `col` is inked.
    pub fn dot(&self, row: usize, col: usize) -> bool {
        (self.0[row] >> (15 - col)) & 1 == 1
    }
    /// The inked dots.
    pub fn ink(&self) -> u32 {
        self.0.iter().map(|r| r.count_ones()).sum()
    }
    /// The box of a missing character: rows 2 and 13 inked from column 2 to 13, and columns 2 and 13 of rows 3 to 12.
    pub fn missing() -> Cell {
        let mut c = [0u16; 16];
        let full = 0b0011_1111_1111_1100;
        let sides = 0b0010_0000_0000_0100;
        c[2] = full;
        c[13] = full;
        for r in c.iter_mut().take(13).skip(3) {
            *r = sides;
        }
        Cell(c)
    }
}

/// Bytes of a glyph of an HZK16 file.
pub const HZK16_GLYPH: usize = 32;

/// The cell of `c` in an HZK16 file: the glyph of its EUC-CN code (lead, trail) at ((lead − A1) × 94 + (trail − A1)) × 32, two
/// bytes a row, the first byte the left 8 columns. `None` for a character outside GB 2312 or past the end of the file.
pub fn hzk16(file: &[u8], c: char) -> Option<Cell> {
    let (lead, trail) = gb2312::code_of(c)?;
    let at = (usize::from(lead - 0xA1) * 94 + usize::from(trail - 0xA1)) * HZK16_GLYPH;
    let g = file.get(at..at + HZK16_GLYPH)?;
    let mut rows = [0u16; 16];
    for (r, row) in rows.iter_mut().enumerate() {
        *row = u16::from_be_bytes([g[2 * r], g[2 * r + 1]]);
    }
    Some(Cell(rows))
}

/// The cell of `c` drawn by a TrueType font at 16 pixels to the em (the descender on the bottom row), its ink centred across
/// the 16 columns. `None` when the font has no outline for `c`.
pub fn truetype(f: &TrueTypeFont, c: char) -> Option<Cell> {
    let m = f.render(c, 16)?;
    let w = m.width.min(16) as usize;
    let left = (16 - w) / 2;
    let mut rows = [0u16; 16];
    for (y, row) in rows.iter_mut().enumerate().take(m.height.min(16) as usize) {
        for x in 0..w {
            if m.data[y * m.width as usize + x] >= 100 {
                *row |= 1 << (15 - (left + x));
            }
        }
    }
    Some(Cell(rows))
}

/// The cell of `c` from the shared Hell glyphs (Latin, Cyrillic, Greek): each glyph pixel two dots wide and two high, in rows
/// 1 to 14, centred. The full-width forms U+FF01 to U+FF5E use the glyph of their ASCII character. `None` when the Hell font has
/// no glyph for `c`.
pub fn hell(c: char) -> Option<Cell> {
    let c = match u32::from(c) {
        0xFF01..=0xFF5E => char::from_u32(u32::from(c) - 0xFF01 + 0x21)?,
        _ => c,
    };
    let g = font::hell::glyph(c)?;
    let (w, h) = (usize::from(g.width).min(8), usize::from(g.height).min(7));
    let left = (16 - 2 * w) / 2;
    let top = 1 + (14 - 2 * h) / 2;
    let mut rows = [0u16; 16];
    for y in 0..h {
        for x in 0..w {
            if g.pixel(x as u8, y as u8) {
                for (dy, dx) in [(0, 0), (0, 1), (1, 0), (1, 1)] {
                    rows[top + 2 * y + dy] |= 1 << (15 - (left + 2 * x + dx));
                }
            }
        }
    }
    Some(Cell(rows))
}

/// Where the cells of a text come from, in order: an HZK16 file, a TrueType font, then the Hell glyphs; a missing character is
/// a box.
#[derive(Clone, Debug, Default)]
pub struct Cells {
    hzk16: Option<Vec<u8>>,
    ttf: Option<TrueTypeFont>,
}

impl Cells {
    /// The Hell glyphs only.
    pub fn new() -> Cells {
        Cells::default()
    }
    /// With a font file: an HZK16 file (a multiple of 32 bytes that is not a TrueType file) or a TrueType font. `None` for
    /// bytes that are neither.
    pub fn with_font(bytes: Vec<u8>) -> Option<Cells> {
        if let Ok(f) = TrueTypeFont::parse(bytes.clone()) {
            return Some(Cells {
                hzk16: None,
                ttf: Some(f),
            });
        }
        (bytes.len() >= HZK16_GLYPH && bytes.len().is_multiple_of(HZK16_GLYPH)).then_some(Cells {
            hzk16: Some(bytes),
            ttf: None,
        })
    }
    /// Whether a font is loaded.
    pub fn has_font(&self) -> bool {
        self.hzk16.is_some() || self.ttf.is_some()
    }
    /// The cell of `c` from the first source that has it.
    pub fn get(&self, c: char) -> Option<Cell> {
        self.hzk16
            .as_deref()
            .and_then(|f| hzk16(f, c))
            .filter(|g| g.ink() > 0)
            .or_else(|| self.ttf.as_ref().and_then(|f| truetype(f, c)))
            .or_else(|| hell(c))
    }
    /// The cell of `c`, or the box. A blank cell (U+3000) is empty.
    pub fn cell(&self, c: char) -> Cell {
        if c == '\u{3000}' {
            return Cell::default();
        }
        self.get(c).unwrap_or_else(Cell::missing)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_box_and_the_hell_cells() {
        let b = Cell::missing();
        assert_eq!(b.ink(), 2 * 12 + 2 * 10);
        assert!(b.dot(2, 2) && b.dot(13, 13) && b.dot(7, 2) && !b.dot(7, 7));
        let a = hell('A').unwrap();
        assert_eq!(hell('Ａ'), Some(a));
        assert!(a.ink() > 20);
        assert_eq!(a.0[0], 0);
        assert_eq!(a.0[15], 0);
        assert_eq!(Cells::new().cell('\u{3000}').ink(), 0);
        assert_eq!(Cells::new().cell('中'), Cell::missing());
    }

    #[test]
    fn an_hzk16_file() {
        // A file of three rows of the table: the glyph of 中 lies at row D6.
        let (l, t) = gb2312::code_of('中').unwrap();
        let at = (usize::from(l - 0xA1) * 94 + usize::from(t - 0xA1)) * 32;
        let mut f = vec![0u8; at + 32];
        f[at] = 0x80;
        f[at + 31] = 0x01;
        let c = hzk16(&f, '中').unwrap();
        assert!(c.dot(0, 0) && c.dot(15, 15) && c.ink() == 2);
        let cells = Cells::with_font(f).unwrap();
        assert!(cells.has_font());
        assert_eq!(cells.cell('中'), c);
        assert_eq!(cells.cell('A'), hell('A').unwrap());
    }
}
