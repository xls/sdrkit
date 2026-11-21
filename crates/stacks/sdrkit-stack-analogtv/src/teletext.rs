//! The line format of teletext in the field blanking of 625-line pictures, and the generator's test service. The packets, the
//! codes and the page store are those of `sdrkit-util-text`.
//!
//! Standard: ITU-R BT.653 system B, ETS 300 706.

use sdrkit_util_text::teletext::packet::{build_header, build_row, Control};
use sdrkit_util_text::teletext::PageNo;

/// Bits a second (444 times the line rate).
pub const BIT_RATE: f64 = 444.0 * 15_625.0;
/// The start of the clock run-in after 0H, µs.
pub const START_US: f64 = 12.0;
/// The level of a one (white is 1).
pub const ONE_LEVEL: f64 = 0.66;
/// The framing code.
pub const FRAMING: u8 = 0x27;
/// Bits on a line: run-in (16), framing code (8), 42 bytes.
pub const LINE_BITS: usize = 16 + 8 + 42 * 8;

/// Whether 625-line line `l` (1-based) carries teletext in this receiver and generator: lines 7 to 22 and 320 to 335.
pub fn is_teletext_line(l: u32) -> bool {
    (7..=22).contains(&l) || (320..=335).contains(&l)
}

/// The bits of a line that carries the packet `p` (42 bytes), in the order sent: the clock run-in, the framing code, then each
/// byte least significant bit first.
pub fn line_bits(p: &[u8; 42]) -> [u8; LINE_BITS] {
    let mut bits = [0u8; LINE_BITS];
    let mut k = 0;
    let mut put = |b: u8| {
        for i in 0..8 {
            bits[k] = (b >> i) & 1;
            k += 1;
        }
    };
    put(0x55);
    put(0x55);
    put(FRAMING);
    for &b in p {
        put(b);
    }
    bits
}

/// The test service of the generator: three pages of plain text in magazine 1, sent in turn. The header shows the clock.
pub struct Service {
    pages: Vec<(PageNo, Vec<String>)>,
    page: usize,
    row: usize,
}

impl Default for Service {
    fn default() -> Self {
        let mk = |no: u16, title: &str, lines: &[&str]| {
            let mut rows = vec![format!("{title:^40}")];
            rows.extend(lines.iter().map(|l| format!("{l:<40}")));
            (PageNo::from_decimal(no).unwrap_or_default(), rows)
        };
        Service {
            pages: vec![
                mk(
                    100,
                    "SDRKIT TEST TELETEXT",
                    &[
                        "",
                        "  Index",
                        "  101  About this service",
                        "  102  Test lines",
                    ],
                ),
                mk(
                    101,
                    "ABOUT",
                    &[
                        "",
                        "  This text is sent in the field",
                        "  blanking of the test picture.",
                    ],
                ),
                mk(
                    102,
                    "TEST LINES",
                    &[
                        "",
                        "  ABCDEFGHIJKLMNOPQRSTUVWXYZ",
                        "  abcdefghijklmnopqrstuvwxyz",
                        "  0123456789 !\"%&'()*+,-./:;<=>?",
                    ],
                ),
            ],
            page: 0,
            row: 0,
        }
    }
}

impl Service {
    /// The next packet (42 bytes). `clock_s` is the time the header shows, seconds of the day.
    pub fn next_packet(&mut self, clock_s: f64) -> [u8; 42] {
        let (no, rows) = &self.pages[self.page];
        let p = if self.row == 0 {
            let s = clock_s.max(0.0) as u64;
            let text = format!(
                "SDRKIT {:>3} {:02}:{:02}:{:02}",
                no,
                (s / 3600) % 24,
                (s / 60) % 60,
                s % 60
            );
            build_header(*no, 0, Control::default(), 0, text.as_bytes())
        } else {
            build_row(no.magazine(), self.row as u8, rows[self.row - 1].as_bytes())
        };
        self.row += 1;
        if self.row > rows.len() {
            self.row = 0;
            self.page = (self.page + 1) % self.pages.len();
        }
        p
    }

    /// The pages it sends, with their rows (row 1 first).
    pub fn pages(&self) -> &[(PageNo, Vec<String>)] {
        &self.pages
    }
}
