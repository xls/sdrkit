//! Teletext in a DVB stream: [`TeletextPage`], the page item that the graph carries, and [`TeletextDecoder`], which takes
//! teletext PES payloads and returns those items. Page numbers are decimal (100 to 899). Subcodes are in nibble form ("0001").
//!
//! Standard: ETSI EN 300 472, ETS 300 706.
//!

use sdrkit_util_text::teletext::{self as tt, Glyph, Link, PageNo, PageStore};

/// Character size.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Size {
    /// Normal.
    #[default]
    Normal,
    /// Upper half of double height.
    DoubleTop,
    /// Lower half of double height.
    DoubleBottom,
}

/// One cell of the page model.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Cell {
    /// A character, or for a mosaic cell the six sextant bits as a code point (0 to 63).
    pub ch: char,
    /// A mosaic cell.
    pub mosaic: bool,
    /// Separated mosaic.
    pub separated: bool,
    /// Foreground 0 to 7 (black, red, green, yellow, blue, magenta, cyan, white).
    pub fg: u8,
    /// Background 0 to 7.
    pub bg: u8,
    /// Size.
    pub size: Size,
    /// Flashing.
    pub flash: bool,
    /// Concealed.
    pub conceal: bool,
    /// Boxed.
    pub boxed: bool,
}

impl Default for Cell {
    fn default() -> Self {
        Cell::from(&tt::Cell::default())
    }
}

impl From<&tt::Cell> for Cell {
    fn from(c: &tt::Cell) -> Self {
        let (ch, mosaic, separated) = match c.glyph {
            Glyph::Char(ch) => (ch, false, false),
            Glyph::Mosaic {
                sextants,
                separated,
            } => (
                char::from_u32(sextants as u32).unwrap_or(' '),
                true,
                separated,
            ),
        };
        Cell {
            ch,
            mosaic,
            separated,
            fg: c.fg,
            bg: c.bg,
            size: match c.size {
                tt::Size::Normal => Size::Normal,
                tt::Size::DoubleTop => Size::DoubleTop,
                tt::Size::DoubleBottom => Size::DoubleBottom,
            },
            flash: c.flash,
            conceal: c.conceal,
            boxed: c.boxed,
        }
    }
}

impl Cell {
    /// The packed 32-bit form: bits 0 to 20 code point or sextants, 21 to 23 foreground, 24 to 26 background, 27 mosaic, 28 to 29
    /// size, 30 separated, 31 flash.
    pub fn packed(&self) -> u32 {
        let size = match self.size {
            Size::Normal => 0,
            Size::DoubleTop => 1,
            Size::DoubleBottom => 2,
        };
        (self.ch as u32 & 0x1F_FFFF)
            | (self.fg as u32 & 7) << 21
            | (self.bg as u32 & 7) << 24
            | (self.mosaic as u32) << 27
            | size << 28
            | (self.separated as u32) << 30
            | (self.flash as u32) << 31
    }
}

/// A page for the screens (the typed item `teletext.page`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TeletextPage {
    /// Page number (100 to 899).
    pub page: u16,
    /// Subcode (nibble form).
    pub subcode: u16,
    /// Subcodes held for this page.
    pub subcodes: Vec<u16>,
    /// The control bits of the header.
    pub flags: tt::packet::Control,
    /// National option (C12 to C14, C12 the most significant bit).
    pub national: u8,
    /// Red, green, yellow, cyan links (0 when absent).
    pub links: [u16; 4],
    /// The index link (0 when absent).
    pub index: u16,
    /// Version.
    pub version: u64,
    /// 25 rows of 40 cells.
    pub rows: Vec<Vec<Cell>>,
    /// Whether the page had any box codes (a page without them counts as boxed in all cells).
    pub has_boxes: bool,
}

fn link_number(l: &Option<Link>) -> u16 {
    l.and_then(|l| l.page.decimal()).unwrap_or(0)
}

impl TeletextPage {
    /// The item for a page that `sdrkit-util-text` rendered. `national` is C12 to C14 of the header, `subcodes` the subpages held.
    pub fn from_page(p: &tt::Page, national: u8, subcodes: Vec<u16>) -> TeletextPage {
        TeletextPage {
            page: p.page.decimal().unwrap_or(0),
            subcode: p.subcode,
            subcodes,
            flags: p.control,
            national,
            links: [
                link_number(&p.links[0]),
                link_number(&p.links[1]),
                link_number(&p.links[2]),
                link_number(&p.links[3]),
            ],
            index: link_number(&p.links[5]),
            version: p.version,
            rows: p
                .rows
                .iter()
                .map(|r| r.iter().map(Cell::from).collect())
                .collect(),
            has_boxes: p.has_boxes,
        }
    }

    /// The rows as text. Mosaic and concealed cells become spaces.
    pub fn text(&self) -> Vec<String> {
        self.rows
            .iter()
            .map(|r| {
                r.iter()
                    .map(|c| if c.mosaic || c.conceal { ' ' } else { c.ch })
                    .collect::<String>()
            })
            .collect()
    }

    /// The packed form: 1000 words, and the conceal and boxed bitmaps (1000 bits each, as 125 bytes).
    pub fn packed(&self) -> (Vec<u32>, Vec<u8>, Vec<u8>) {
        let mut words = Vec::with_capacity(1000);
        let mut conceal = vec![0u8; 125];
        let mut boxed = vec![0u8; 125];
        for (r, row) in self.rows.iter().enumerate() {
            for (c, cell) in row.iter().enumerate() {
                let i = r * 40 + c;
                words.push(cell.packed());
                if cell.conceal {
                    conceal[i / 8] |= 1 << (i % 8);
                }
                if cell.boxed || !self.has_boxes {
                    boxed[i / 8] |= 1 << (i % 8);
                }
            }
        }
        (words, conceal, boxed)
    }

    /// Subtitle text: the boxed cells of the rows below the header, one line for each row that has text.
    pub fn subtitle_lines(&self) -> Vec<String> {
        self.rows
            .iter()
            .skip(1)
            .filter_map(|r| {
                let s: String = r
                    .iter()
                    .map(|c| {
                        if (c.boxed || !self.has_boxes) && !c.mosaic && !c.conceal {
                            c.ch
                        } else {
                            ' '
                        }
                    })
                    .collect();
                let t = s.trim().to_string();
                (!t.is_empty()).then_some(t)
            })
            .collect()
    }
}

/// Teletext decoder counters.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TeletextStats {
    /// Data units used.
    pub data_units: u64,
    /// Packets dropped for a bad address.
    pub bad_address: u64,
    /// Packets 25 to 31 seen (X/26 is used for Level 1.5 characters, the others are counted only).
    pub enhancement_packets: u64,
    /// Characters with a parity error.
    pub parity_errors: u64,
    /// Pages completed.
    pub pages: u64,
}

/// The most page and subpage entries a [`TeletextDecoder`] keeps. The least recently changed entry goes first.
pub const CACHE_CAP: usize = 4096;

/// The most completed pages that wait for [`TeletextDecoder::take_completed`]. If more come, the oldest goes.
const COMPLETED_CAP: usize = 256;

/// The page assembler and cache for one teletext stream: a [`PageStore`] with decimal page numbers.
pub struct TeletextDecoder {
    store: PageStore,
    completed: Vec<(u16, u16)>,
    /// Counters.
    pub stats: TeletextStats,
}

impl Default for TeletextDecoder {
    fn default() -> Self {
        TeletextDecoder {
            store: PageStore::new(CACHE_CAP),
            completed: Vec::with_capacity(COMPLETED_CAP),
            stats: TeletextStats::default(),
        }
    }
}

impl std::fmt::Debug for TeletextDecoder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TeletextDecoder")
            .field("stats", &self.stats)
            .finish_non_exhaustive()
    }
}

impl TeletextDecoder {
    /// A new decoder.
    pub fn new() -> Self {
        Self::default()
    }

    /// Clears the cache (a new teletext PID).
    pub fn clear(&mut self) {
        *self = Self::default();
    }

    /// The page store that decodes the packets.
    pub fn store(&self) -> &PageStore {
        &self.store
    }

    /// A teletext PES payload (starting with data_identifier, EN 300 472 4.3).
    pub fn push_pes(&mut self, d: &[u8]) {
        for p in tt::packet::pes_packets(d) {
            self.stats.data_units += 1;
            self.push_packet(&p);
        }
    }

    /// One teletext packet: 2 address bytes and 40 data bytes, with the first bit sent in bit 0.
    pub fn push_packet(&mut self, b: &[u8; 42]) {
        if let Ok((_, row)) = tt::packet::address(b) {
            if row >= 25 && row != 27 {
                self.stats.enhancement_packets += 1;
            }
        }
        self.store.push_packet(b);
        while let Some((page, sub)) = self.store.take_completed() {
            if let Some(n) = page.decimal() {
                if self.completed.len() == COMPLETED_CAP {
                    self.completed.remove(0);
                }
                self.completed.push((n, sub));
            }
        }
        let s = &self.store.stats;
        self.stats.bad_address = s.bad_address;
        self.stats.parity_errors = s.parity_errors;
        self.stats.pages = s.pages;
    }

    /// Pages completed since the last call, as (page number, subcode).
    pub fn take_completed(&mut self) -> Vec<(u16, u16)> {
        std::mem::take(&mut self.completed)
    }

    /// Pages completed since the last call, as (page number, subcode), without an allocation.
    pub fn drain_completed(&mut self) -> std::vec::Drain<'_, (u16, u16)> {
        self.completed.drain(..)
    }

    /// The page numbers held (decimal pages only), in order.
    pub fn pages(&self) -> Vec<u16> {
        self.store.pages().filter_map(PageNo::decimal).collect()
    }

    /// The subcodes held for a page.
    pub fn subcodes(&self, page: u16) -> Vec<u16> {
        match PageNo::from_decimal(page) {
            Ok(p) => self.store.subcodes(p).collect(),
            Err(_) => Vec::new(),
        }
    }

    /// A page: the newest complete copy (or the copy being received when none is complete) of `subcode`, or of the subpage that
    /// changed last. The links come from the copy being received.
    pub fn page(&self, page: u16, subcode: Option<u16>) -> Option<TeletextPage> {
        let no = PageNo::from_decimal(page).ok()?;
        let (raw, _) = self.store.raw(no, subcode)?;
        let national = raw.national;
        let mut out = Box::<tt::Page>::default();
        if !self.store.render(no, subcode, &mut out) {
            return None;
        }
        Some(TeletextPage::from_page(&out, national, self.subcodes(page)))
    }

    /// The page offered for subtitles when the stream names none: 888, or 777 when only that is sent.
    pub fn default_subtitle_page(&self) -> u16 {
        self.store.default_subtitle_page().decimal().unwrap_or(888)
    }
}

/// The page offered for teletext subtitles when the stream lists no subtitle entry: 888, or 777 when only that is sent.
pub fn default_subtitle_page(d: &TeletextDecoder) -> u16 {
    d.default_subtitle_page()
}

#[cfg(test)]
mod tests {
    use super::*;
    use tt::charset::NATIONAL_POSITIONS;
    use tt::packet::{build_row, Control};

    /// A header packet of the page with these digits (magazine 8 as 0).
    fn build_header(
        mag: u8,
        tens: u8,
        units: u8,
        subcode: u16,
        flags: Control,
        national: u8,
        text: &[u8],
    ) -> [u8; 42] {
        let page = PageNo::new(if mag == 0 { 8 } else { mag }, tens, units).unwrap();
        tt::packet::build_header(page, subcode, flags, national, text)
    }

    fn pes(packets: &[[u8; 42]]) -> Vec<u8> {
        let mut v = Vec::new();
        tt::packet::build_pes(packets, &mut v);
        v
    }

    /// A packet 27/0 with six links to decimal pages (0: no page).
    fn build_links(mag: u8, links: [u16; 6]) -> [u8; 42] {
        let l = links.map(|n| {
            PageNo::from_decimal(n).ok().map(|page| Link {
                page,
                subcode: 0x3F7F,
            })
        });
        tt::packet::build_links(mag, l)
    }

    fn decode(packets: &[[u8; 42]]) -> TeletextDecoder {
        let mut d = TeletextDecoder::new();
        d.push_pes(&pes(packets));
        d
    }

    #[test]
    fn header_colour_and_double_height() {
        let mut row1 = vec![0x02];
        row1.extend_from_slice(b"Hello");
        let mut row2 = vec![0x0D];
        row2.push(b'W');
        let d = decode(&[
            build_header(
                1,
                2,
                3,
                0,
                Control::default(),
                0,
                b"HEADER TEXT 0123456789 abcdefghi",
            ),
            build_row(1, 1, &row1),
            build_row(1, 2, &row2),
        ]);
        let p = d.page(123, None).unwrap();
        let t = p.text();
        assert_eq!(&t[0][..8], "        ");
        assert_eq!(&t[0][8..], "HEADER TEXT 0123456789 abcdefghi");
        assert_eq!(&t[1][1..6], "Hello");
        assert!(p.rows[1][1..6].iter().all(|c| c.fg == 2));
        assert_eq!(p.rows[2][1].ch, 'W');
        assert_eq!(p.rows[2][1].size, Size::DoubleTop);
        assert_eq!(p.rows[3][1].size, Size::DoubleBottom);
        assert_eq!(p.rows[3][5].ch, ' ');
        assert_eq!(d.stats.data_units, 3);
    }

    #[test]
    fn national_subsets() {
        let codes: Vec<u8> = NATIONAL_POSITIONS.to_vec();
        for (opt, expect) in [
            (2u8, "#¤ÉÄÖÅÜ_éäöåü"),
            (1, "#$§ÄÖÜ^_°äöüß"),
            (4, "éïàëêùî#èâôûç"),
            (3, "£$é°ç→↑#ùàòèì"),
            (5, "ç$¡áéíóú¿üñèà"),
            (6, "#ůčťžýířéáěúš"),
        ] {
            let d = decode(&[
                build_header(1, 0, 0, 0, Control::default(), opt, b""),
                build_row(1, 1, &codes),
            ]);
            let p = d.page(100, None).unwrap();
            assert_eq!(
                p.text()[1].chars().take(13).collect::<String>(),
                expect,
                "option {opt}"
            );
            assert_eq!(p.national, opt);
        }
    }

    #[test]
    fn attributes() {
        // Table 26: alphanumeric red (set-after), new background (set-at), flash (set-after), mosaic red (set-after), separated
        // (set-at), white (set-after), conceal (set-at).
        let row = vec![
            0x01, 0x1D, b'A', 0x08, b'B', 0x11, 0x7F, 0x1A, 0x7F, 0x07, 0x18, b'S',
        ];
        let d = decode(&[
            build_header(1, 0, 1, 0, Control::default(), 0, b""),
            build_row(1, 1, &row),
        ]);
        let p = d.page(101, None).unwrap();
        let r = &p.rows[1];
        assert_eq!((r[1].bg, r[2].bg, r[2].fg), (1, 1, 1));
        assert!(r[4].flash && !r[2].flash);
        assert!(r[6].mosaic && !r[6].separated && r[6].ch as u32 == 63);
        assert!(r[8].mosaic && r[8].separated);
        assert!(r[11].conceal);
        assert_eq!(p.text()[1].chars().nth(11), Some(' '));
    }

    #[test]
    fn links_and_subpages() {
        let h1 = build_header(2, 3, 4, 1, Control::default(), 0, b"");
        let h2 = build_header(2, 3, 4, 2, Control::default(), 0, b"");
        let l = build_links(2, [234, 100, 0, 899, 0, 200]);
        let d = decode(&[
            h1,
            l,
            h2,
            build_row(2, 1, b"two"),
            build_header(2, 0xF, 0xF, 0, Control::default(), 0, b""),
        ]);
        let p = d.page(234, Some(1)).unwrap();
        assert_eq!(p.links, [234, 100, 0, 899]);
        assert_eq!(p.index, 200);
        assert_eq!(d.subcodes(234), vec![1, 2]);
        assert_eq!(d.page(234, None).unwrap().subcode, 2);
        assert_eq!(d.pages(), vec![234]);
    }

    #[test]
    fn subtitle_page_and_erase() {
        let f = Control {
            subtitle: true,
            ..Default::default()
        };
        let mut row = vec![0x0B, 0x0B];
        row.extend_from_slice(b"Boxed text");
        row.extend_from_slice(&[0x0A, 0x0A, b'x']);
        let d0 = [build_header(8, 8, 8, 0, f, 0, b""), build_row(8, 20, &row)];
        let mut d = decode(&d0);
        let p = d.page(888, None).unwrap();
        assert!(p.flags.subtitle);
        assert_eq!(p.subtitle_lines(), vec!["Boxed text".to_string()]);
        let e = Control {
            erase: true,
            subtitle: true,
            ..Default::default()
        };
        let filler = build_header(8, 0xF, 0xF, 0, Control::default(), 0, b"");
        d.push_pes(&pes(&[build_header(8, 8, 8, 0, e, 0, b""), filler]));
        let p = d.page(888, None).unwrap();
        assert!(p.text()[1..].iter().all(|r| r.trim().is_empty()));
        assert_eq!(default_subtitle_page(&d), 888);
    }

    #[test]
    fn parity_error_repaired_by_the_next_copy() {
        let mut bad = build_row(1, 1, b"Hello");
        bad[2 + 1] ^= 0x01; // 'e' with a parity error
        let filler = build_header(1, 0xF, 0xF, 0, Control::default(), 0, b"");
        let mut d = decode(&[
            build_header(1, 0, 0, 0, Control::default(), 0, b""),
            bad,
            filler,
        ]);
        assert_eq!(&d.page(100, None).unwrap().text()[1][..5], "H llo");
        d.push_pes(&pes(&[
            build_header(1, 0, 0, 0, Control::default(), 0, b""),
            build_row(1, 1, b"Hello"),
            filler,
        ]));
        assert_eq!(&d.page(100, None).unwrap().text()[1][..5], "Hello");
        assert!(d.stats.parity_errors >= 1);
    }

    #[test]
    fn colour_codes_act_from_the_next_cell() {
        // ETS 300 706 Table 26: "alpha red" is set-after, so its own cell keeps the white of the row start.
        let d = decode(&[
            build_header(1, 0, 0, 0, Control::default(), 0, b""),
            build_row(1, 1, &[0x01, b'A']),
        ]);
        let r = &d.page(100, None).unwrap().rows[1];
        assert_eq!((r[0].fg, r[1].fg), (7, 1));
    }

    #[test]
    fn no_double_height_on_row_23() {
        // ETS 300 706 12.2 (Table 26, note on double height): rows 1 to 22 only.
        let d = decode(&[
            build_header(1, 0, 0, 0, Control::default(), 0, b""),
            build_row(1, 23, &[0x0D, b'X']),
            build_row(1, 24, b"KEEP"),
        ]);
        let p = d.page(100, None).unwrap();
        assert_eq!(p.rows[23][1].size, Size::Normal);
        assert_eq!(&p.text()[24][..4], "KEEP");
    }

    #[test]
    fn hex_pages_are_not_listed() {
        let d = decode(&[
            build_header(1, 0xA, 0x3, 0, Control::default(), 0, b""),
            build_row(1, 1, b"data"),
            build_header(1, 0, 0, 0, Control::default(), 0, b""),
        ]);
        assert!(d.pages().is_empty() || d.pages() == vec![100]);
        assert!(!d.pages().iter().any(|&p| p > 899));
    }
}
