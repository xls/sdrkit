//! A page as received ([`RawPage`]) and as displayed ([`Page`], 25 rows of 40 cells): the Level 1 spacing attributes of ETS 300 706
//! 12.2 (Table 26) and the Level 1.5 enhancements of packets X/26 (12.3: G2 characters and G0 characters with diacritical marks).

use super::charset::{designation, latin_g0, latin_g2, National, G0};
use super::packet::{Control, Link, PageNo};
use crate::compose::Mark;

/// A code not received with good parity.
pub const MISSING: u8 = 0xFF;

/// A page as received: 7-bit codes per row and the enhancement triplets.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RawPage {
    /// The page.
    pub page: PageNo,
    /// The subcode (nibble form).
    pub subcode: u16,
    /// The header control bits.
    pub control: Control,
    /// C12 to C14.
    pub national: u8,
    /// The default set designation from packet X/28/0 or M/29/0 (bits 14 to 11 of Table 32), when one was received.
    pub designation: Option<u8>,
    /// 25 rows of 40 codes ([`MISSING`] for a code never received with good parity). Row 0 columns 0 to 7 are not used.
    pub rows: [[u8; 40]; 25],
    /// Bit `r` set when row `r` was received.
    pub received: u32,
    /// Packets X/26/0 to X/26/15: 13 triplets each (`u32::MAX` for a triplet with a double error).
    pub enhancements: [[u32; 13]; 16],
    /// Bit `d` set when X/26/d was received.
    pub enhanced: u16,
    /// Red, green, yellow, cyan, fifth and index links (X/27/0).
    pub links: [Option<Link>; 6],
    /// Bumped on every change.
    pub version: u64,
}

impl RawPage {
    /// An empty page.
    pub fn new(page: PageNo, subcode: u16) -> Self {
        RawPage {
            page,
            subcode,
            control: Control::default(),
            national: 0,
            designation: None,
            rows: [[MISSING; 40]; 25],
            received: 0,
            enhancements: [[u32::MAX; 13]; 16],
            enhanced: 0,
            links: [None; 6],
            version: 0,
        }
    }

    /// Clears the content (erase page), keeping the identity.
    pub fn erase(&mut self) {
        self.rows = [[MISSING; 40]; 25];
        self.received = 0;
        self.enhanced = 0;
        self.links = [None; 6];
    }

    /// The national option sub-set for the Latin G0 set (Table 32 with the page's designation, English when reserved).
    pub fn national_set(&self) -> National {
        let group = self.designation.unwrap_or(0) & 0x78;
        match designation(group | (self.national & 7)) {
            Some((G0::Latin(n), _)) => n,
            _ => National::English,
        }
    }
}

/// Character size.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Size {
    /// Normal.
    #[default]
    Normal,
    /// The upper half of a double-height character.
    DoubleTop,
    /// The lower half of a double-height character.
    DoubleBottom,
}

/// What a cell shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Glyph {
    /// A character.
    Char(char),
    /// A G1 block mosaic: six sextants (bit 0 top left, bit 1 top right, ... bit 5 bottom right).
    Mosaic {
        /// The sextants.
        sextants: u8,
        /// Separated (gaps between the blocks).
        separated: bool,
    },
}

/// One cell of a page.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Cell {
    /// The glyph.
    pub glyph: Glyph,
    /// Foreground colour 0 to 7 (black, red, green, yellow, blue, magenta, cyan, white).
    pub fg: u8,
    /// Background colour 0 to 7.
    pub bg: u8,
    /// Size.
    pub size: Size,
    /// Flashing.
    pub flash: bool,
    /// Concealed.
    pub conceal: bool,
    /// Inside a box (shown on subtitle and newsflash pages).
    pub boxed: bool,
}

impl Default for Cell {
    fn default() -> Self {
        Cell {
            glyph: Glyph::Char(' '),
            fg: 7,
            bg: 0,
            size: Size::Normal,
            flash: false,
            conceal: false,
            boxed: false,
        }
    }
}

impl Cell {
    /// The character shown (a space for a mosaic or a concealed cell).
    pub fn char(&self) -> char {
        match self.glyph {
            Glyph::Char(c) if !self.conceal => c,
            _ => ' ',
        }
    }

    /// A packed 32-bit form: bits 0 to 20 the code point or sextants, 21 to 23 foreground, 24 to 26 background, 27 mosaic, 28 and
    /// 29 size, 30 separated, 31 flash.
    pub fn packed(&self) -> u32 {
        let (v, mosaic, sep) = match self.glyph {
            Glyph::Char(c) => (c as u32, 0, 0),
            Glyph::Mosaic {
                sextants,
                separated,
            } => (sextants as u32, 1, separated as u32),
        };
        let size = match self.size {
            Size::Normal => 0,
            Size::DoubleTop => 1,
            Size::DoubleBottom => 2,
        };
        (v & 0x1F_FFFF)
            | (self.fg as u32 & 7) << 21
            | (self.bg as u32 & 7) << 24
            | mosaic << 27
            | size << 28
            | sep << 30
            | (self.flash as u32) << 31
    }
}

/// A displayed page.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Page {
    /// The page.
    pub page: PageNo,
    /// The subcode.
    pub subcode: u16,
    /// The header control bits.
    pub control: Control,
    /// The links.
    pub links: [Option<Link>; 6],
    /// The version of the raw page it was made from.
    pub version: u64,
    /// 25 rows of 40 cells. Row 0 columns 0 to 7 are left blank for the receiver's own page number.
    pub rows: [[Cell; 40]; 25],
    /// Whether any box was opened (a subtitle page without boxes counts as wholly boxed).
    pub has_boxes: bool,
}

impl Default for Page {
    fn default() -> Self {
        Page {
            page: PageNo::default(),
            subcode: 0,
            control: Control::default(),
            links: [None; 6],
            version: 0,
            rows: [[Cell::default(); 40]; 25],
            has_boxes: false,
        }
    }
}

impl Page {
    /// Appends the text of a row (mosaic and concealed cells as spaces).
    pub fn row_text(&self, row: usize, out: &mut String) {
        if let Some(r) = self.rows.get(row) {
            out.extend(r.iter().map(Cell::char));
        }
    }

    /// Appends the subtitle text: the boxed cells of rows 1 to 24 (all cells when the page has no box), one trimmed line per row
    /// with text, separated by `'\n'`.
    pub fn subtitle_into(&self, out: &mut String) {
        let mut first = true;
        for r in &self.rows[1..] {
            let chars: [char; 40] = core::array::from_fn(|i| {
                if r[i].boxed || !self.has_boxes {
                    r[i].char()
                } else {
                    ' '
                }
            });
            let (Some(a), Some(b)) = (
                chars.iter().position(|&c| c != ' '),
                chars.iter().rposition(|&c| c != ' '),
            ) else {
                continue;
            };
            if !first {
                out.push('\n');
            }
            out.extend(&chars[a..=b]);
            first = false;
        }
    }
}

/// Renders a raw page at presentation Level 1.5 into `out` (no allocation).
pub fn render(p: &RawPage, out: &mut Page) {
    out.page = p.page;
    out.subcode = p.subcode;
    out.control = p.control;
    out.links = p.links;
    out.version = p.version;
    out.has_boxes = false;
    let national = p.national_set();
    let mut skip = false;
    for r in 0..25 {
        if skip {
            skip = false;
            continue;
        }
        let double_row = render_row(p, r, national, out);
        if double_row && (1..=22).contains(&r) {
            for c in 0..40 {
                let top = out.rows[r][c];
                out.rows[r + 1][c] = if top.size == Size::DoubleTop {
                    Cell {
                        size: Size::DoubleBottom,
                        ..top
                    }
                } else {
                    Cell {
                        glyph: Glyph::Char(' '),
                        ..top
                    }
                };
            }
            skip = true;
        } else {
            for c in out.rows[r].iter_mut() {
                if c.size == Size::DoubleTop {
                    c.size = Size::Normal;
                }
            }
        }
    }
    enhance(p, national, out);
}

/// Renders one row with the spacing attributes (Table 26: "set-at" codes act on their own cell, "set-after" codes from the next);
/// returns whether the row requests double height.
fn render_row(p: &RawPage, r: usize, national: National, out: &mut Page) -> bool {
    let src = &p.rows[r];
    let (mut fg, mut bg) = (7u8, 0u8);
    let (mut mosaic, mut separated, mut flash, mut conceal, mut boxed, mut double, mut hold) =
        (false, false, false, false, false, false, false);
    let space = Glyph::Char(' ');
    let mut held = space;
    let mut any_double = false;
    for (c, &raw) in src.iter().enumerate() {
        let code = if r == 0 && c < 8 { 0x20 } else { raw };
        let code = if code == MISSING { 0x20 } else { code };
        // Set-at.
        match code {
            0x09 => flash = false,
            0x0C => {
                if double {
                    held = space;
                }
                double = false;
            }
            0x18 => conceal = true,
            0x19 => separated = false,
            0x1A => separated = true,
            0x1C => bg = 0,
            0x1D => bg = fg,
            0x1E => hold = true,
            _ => {}
        }
        let glyph = if code < 0x20 {
            // A control cell shows a space, or the held mosaic in hold mode.
            if hold && mosaic {
                held
            } else {
                space
            }
        } else if mosaic && !(0x40..=0x5F).contains(&code) {
            held = Glyph::Mosaic {
                sextants: (code & 0x1F) | ((code & 0x40) >> 1),
                separated,
            };
            held
        } else {
            latin_g0(code, national).map_or(space, Glyph::Char)
        };
        out.rows[r][c] = Cell {
            glyph,
            fg,
            bg,
            size: if double {
                Size::DoubleTop
            } else {
                Size::Normal
            },
            flash,
            conceal,
            boxed,
        };
        // Set-after. 0x00 and 0x10 (black) and 0x0E, 0x0F (double width and size) are Level 2.5; 0x1B (ESC) switches to a second G0
        // set only designated at Level 2.5.
        match code {
            0x01..=0x07 | 0x11..=0x17 => {
                fg = code & 7;
                let m = code >= 0x10;
                if m != mosaic {
                    held = space;
                }
                mosaic = m;
                conceal = false;
            }
            0x08 => flash = true,
            0x0A => boxed = false,
            0x0B => {
                boxed = true;
                out.has_boxes = true;
            }
            0x0D => {
                if !double {
                    held = space;
                }
                double = true;
                any_double |= r > 0;
            }
            0x1F => hold = false,
            _ => {}
        }
    }
    any_double
}

/// Applies the X/26 triplets that Level 1.5 decoders act on.
fn enhance(p: &RawPage, national: National, out: &mut Page) {
    let mut row = 0usize;
    for d in 0..16 {
        if p.enhanced & (1 << d) == 0 {
            continue;
        }
        for &t in &p.enhancements[d] {
            if t == u32::MAX {
                continue;
            }
            let (addr, mode, data) = ((t & 63) as u8, (t >> 6 & 31) as u8, (t >> 11 & 127) as u8);
            if addr >= 40 {
                match mode {
                    0x1F => return, // termination marker
                    0x01 | 0x04 => row = if addr == 40 { 24 } else { (addr - 40) as usize },
                    0x07 if addr == 63 => row = 0,
                    _ => {}
                }
                continue;
            }
            let col = addr as usize;
            if data < 0x20 || (row == 0 && col < 8) {
                continue;
            }
            let ch = match mode {
                0x0F => latin_g2(data),
                0x10 => latin_g0(data, national),
                0x11..=0x1F => {
                    let base = data as char;
                    Mark::from_index(mode - 0x10).and_then(|m| m.compose(base).or(Some(base)))
                }
                _ => None,
            };
            if let Some(ch) = ch {
                let cell = &mut out.rows[row][col];
                cell.glyph = Glyph::Char(ch);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn page_with(rows: &[(usize, &[u8])]) -> RawPage {
        let mut p = RawPage::new(PageNo::from_decimal(100).unwrap(), 0);
        for &(r, codes) in rows {
            for (c, &x) in codes.iter().enumerate() {
                p.rows[r][c] = x;
            }
        }
        p
    }

    fn rendered(p: &RawPage) -> Page {
        let mut out = Page::default();
        render(p, &mut out);
        out
    }

    #[test]
    fn colours_are_set_after_and_background_set_at() {
        // Red text, new background (set-at: the control cell itself takes it), flash, mosaics, separated, conceal.
        let p = page_with(&[(
            1,
            &[
                0x01, 0x1D, b'A', 0x08, b'B', 0x11, 0x7F, 0x1A, 0x7F, 0x07, 0x18, b'S',
            ],
        )]);
        let o = rendered(&p);
        let r = &o.rows[1];
        assert_eq!(r[0].fg, 7, "the colour code cell keeps the old colour");
        assert_eq!((r[1].bg, r[2].bg, r[2].fg), (1, 1, 1));
        assert!(!r[3].flash && r[4].flash, "flash is set-after");
        assert_eq!(
            r[6].glyph,
            Glyph::Mosaic {
                sextants: 63,
                separated: false
            }
        );
        assert_eq!(
            r[8].glyph,
            Glyph::Mosaic {
                sextants: 63,
                separated: true
            }
        );
        assert!(r[11].conceal && r[11].char() == ' ');
        assert!(!r[9].conceal);
    }

    #[test]
    fn blast_through_and_hold_mosaics() {
        // In mosaic mode 0x40 to 0x5F are capital letters; hold mosaics repeats the last mosaic in control cells.
        let p = page_with(&[(2, &[0x12, b'A', 0x23, 0x1E, 0x13, 0x24])]);
        let o = rendered(&p);
        let r = &o.rows[2];
        assert_eq!(r[1].glyph, Glyph::Char('A'));
        assert!(matches!(r[2].glyph, Glyph::Mosaic { sextants: 3, .. }));
        assert!(
            matches!(r[3].glyph, Glyph::Mosaic { sextants: 3, .. }),
            "held mosaic in the hold cell"
        );
        assert!(
            matches!(r[4].glyph, Glyph::Mosaic { sextants: 3, .. }),
            "held mosaic in the colour cell"
        );
        assert_eq!(r[4].fg, 2, "set-after colour");
    }

    #[test]
    fn double_height_covers_the_next_row() {
        let p = page_with(&[(2, &[0x0D, b'W']), (3, b"hidden")]);
        let o = rendered(&p);
        assert_eq!(o.rows[2][1].size, Size::DoubleTop);
        assert_eq!(o.rows[3][1].size, Size::DoubleBottom);
        assert_eq!(o.rows[3][1].char(), 'W');
        assert_eq!(o.rows[3][3].char(), ' ');
        // Not on row 23.
        let p = page_with(&[(23, &[0x0D, b'W']), (24, b"shown")]);
        let o = rendered(&p);
        assert_eq!(o.rows[23][1].size, Size::Normal);
        assert_eq!(o.rows[24][0].char(), 's');
    }

    #[test]
    fn national_subsets_and_x28_designation() {
        let mut p = page_with(&[(1, &[0x23, 0x7B, 0x7E])]);
        p.national = 1; // German
        let mut s = String::new();
        rendered(&p).row_text(1, &mut s);
        assert!(s.starts_with("#äß"));
        // X/28 group 0001 with C12-C14 000: Polish.
        p.national = 0;
        p.designation = Some(0b000_1000);
        s.clear();
        rendered(&p).row_text(1, &mut s);
        assert!(s.starts_with("#żź"), "{s}");
    }

    #[test]
    fn level_1_5_enhancements() {
        use super::super::packet::triplet;
        let mut p = page_with(&[(5, b"Lodz cafe")]);
        p.enhancements[0][0] = triplet(45, 0x04, 0); // row 5
        p.enhancements[0][1] = triplet(0, 0x0F, 0x68); // G2 0x68: Ł
        p.enhancements[0][2] = triplet(1, 0x12, b'o'); // o + acute
        p.enhancements[0][3] = triplet(3, 0x12, b'z'); // z + acute
        p.enhancements[0][4] = triplet(8, 0x12, b'e'); // e + acute
        p.enhancements[0][5] = triplet(63, 0x1F, 0); // termination
        p.enhancements[0][6] = triplet(2, 0x12, b'q'); // after the end: ignored
        p.enhanced = 1;
        let mut s = String::new();
        rendered(&p).row_text(5, &mut s);
        assert!(s.starts_with("Łódź café"), "{s}");
    }

    #[test]
    fn subtitles_are_the_boxed_text() {
        let mut row = vec![0x0B, 0x0B];
        row.extend_from_slice(b"Boxed text");
        row.extend_from_slice(&[0x0A, 0x0A, b'x']);
        let p = page_with(&[(20, &row), (22, &[0x0B, 0x0B, b'T', b'w', b'o'])]);
        let mut s = String::new();
        rendered(&p).subtitle_into(&mut s);
        assert_eq!(s, "Boxed text\nTwo");
        let p = page_with(&[(21, b"  no boxes ")]);
        let mut s = String::new();
        rendered(&p).subtitle_into(&mut s);
        assert_eq!(s, "no boxes");
    }
}
