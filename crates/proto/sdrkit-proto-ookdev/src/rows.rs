//! Bit rows: what a slicer makes from a train and a device decoder reads.
//!
//! A [`BitRows`] holds up to `max_rows` rows of up to `max_bits` bits each, packed most significant bit first within each byte
//! (bit position 0 is the top bit of byte 0). Slicers start a new row at a sync mark or a long gap, so one transmission sent
//! several times produces several rows. Bits pushed past `max_bits`, and rows past `max_rows`, are dropped and counted in
//! [`BitRows::overflow`].
//!
//! The operations device decoders use:
//!
//! | Operation | What |
//! |---|---|
//! | [`bit`](BitRows::bit), [`row`](BitRows::row), [`Row::byte`] | read bits and bytes (0 past the end, like a zero-padded row) |
//! | [`search`](BitRows::search) | first position of a bit pattern in a row |
//! | [`extract`](BitRows::extract) | `n` bits from any position into bytes |
//! | [`invert`](BitRows::invert), [`reflect`](BitRows::reflect) | flip every bit, or reverse the bit order within each byte |
//! | [`repeated_row`](BitRows::repeated_row) | a row sent at least `k` times |
//! | [`decode_manchester`](BitRows::decode_manchester) | Manchester (IEEE 802.3 or G. E. Thomas pairs), stopping at the first pair without a change |
//! | [`decode_fm0`](BitRows::decode_fm0) | biphase space (FM0, ISO/IEC 18000-6), finding the clock phase first |
//!
//! The line-code rules are those of `sdrkit_util_bits::line`. This module only walks the rows.
//!
//! ```
//! use sdrkit_proto_ookdev::BitRows;
//! let rows = BitRows::parse("0000 0001 0100 1010 / 1").unwrap();
//! assert_eq!(rows.num_rows(), 2);
//! assert_eq!(rows.search(0, 0, &[0x01, 0x40], 12), Some(0));
//! let mut b = [0u8; 1];
//! rows.extract(0, 8, 8, &mut b).unwrap();
//! assert_eq!(b, [0x4A]);
//! ```

use crate::Error;
use sdrkit_util_bits::line::{Biphase, BiphaseKind, Manchester, ManchesterConv};

/// Rows a [`BitRows`] holds by default.
pub const MAX_ROWS: usize = 50;
/// Bits a row holds by default.
pub const MAX_BITS: usize = 4096;

/// Chips decoded per piece by the line decoders (the size of their scratch).
const PIECE: usize = 256;

/// Rows of packed bits (module documentation).
#[derive(Clone, Debug)]
pub struct BitRows {
    data: Vec<u8>,
    lens: Vec<usize>,
    rows: usize,
    row_bytes: usize,
    max_bits: usize,
    overflow: u32,
    /// The bit width the NRZ slicer measured, microseconds. It is 0 when another slicer made the rows.
    pub bit_us: f32,
    chips: Vec<u8>,
    bits: Vec<u8>,
}

impl Default for BitRows {
    /// [`MAX_ROWS`] rows of [`MAX_BITS`] bits.
    fn default() -> Self {
        Self::new(MAX_ROWS, MAX_BITS)
    }
}

impl PartialEq for BitRows {
    /// The same rows with the same bits (capacity and the measured width are not compared).
    fn eq(&self, o: &Self) -> bool {
        self.rows == o.rows && (0..self.rows).all(|r| self.row(r) == o.row(r))
    }
}

/// One row: its packed bytes and its length in bits.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Row<'a> {
    /// The bytes that hold bits (`ceil(len / 8)`), MSB first. Bits past `len` in the last byte are 0.
    pub bytes: &'a [u8],
    /// Number of bits.
    pub len: usize,
}

impl Row<'_> {
    /// Byte `i`, or 0 past the end (a row reads as if padded with zero bytes).
    #[inline]
    pub fn byte(&self, i: usize) -> u8 {
        self.bytes.get(i).copied().unwrap_or(0)
    }

    /// Bit `i` (0 or 1), 0 past the end.
    #[inline]
    pub fn bit(&self, i: usize) -> u8 {
        if i >= self.len {
            0
        } else {
            (self.bytes[i / 8] >> (7 - i % 8)) & 1
        }
    }

    /// Copies the row into `out` and fills the rest of `out` with zeros (decoders that index bytes past the row's end).
    pub fn copy_padded(&self, out: &mut [u8]) {
        let n = self.bytes.len().min(out.len());
        out[..n].copy_from_slice(&self.bytes[..n]);
        out[n..].fill(0);
    }
}

impl BitRows {
    /// Room for `max_rows` rows (at least 1) of `max_bits` bits (at least 8).
    pub fn new(max_rows: usize, max_bits: usize) -> Self {
        let max_rows = max_rows.max(1);
        let max_bits = max_bits.max(8);
        let row_bytes = max_bits.div_ceil(8);
        BitRows {
            data: vec![0; max_rows * row_bytes],
            lens: vec![0; max_rows],
            rows: 1,
            row_bytes,
            max_bits,
            overflow: 0,
            bit_us: 0.0,
            chips: Vec::with_capacity(PIECE),
            bits: Vec::with_capacity(PIECE / 2),
        }
    }

    /// Rows from text: `0` and `1` are bits, `/` starts a new row, anything else is ignored (tests and examples). Errors when
    /// the text holds more rows or bits than the default capacity.
    pub fn parse(text: &str) -> Result<Self, Error> {
        let mut b = BitRows::default();
        for c in text.chars() {
            match c {
                '0' => b.push(0),
                '1' => b.push(1),
                '/' => b.new_row(),
                _ => {}
            }
        }
        if b.overflow > 0 {
            return Err(Error::Capacity);
        }
        Ok(b)
    }

    /// One row holding the first `bits` bits of `bytes` (MSB first).
    pub fn from_bytes(bytes: &[u8], bits: usize) -> Result<Self, Error> {
        if bits > bytes.len() * 8 {
            return Err(Error::InputTooShort);
        }
        let mut b = BitRows::new(1, bits.max(8));
        for i in 0..bits {
            b.push((bytes[i / 8] >> (7 - i % 8)) & 1);
        }
        Ok(b)
    }

    /// Empties every row. One empty row remains.
    pub fn clear(&mut self) {
        self.lens[..self.rows].fill(0);
        self.rows = 1;
        self.overflow = 0;
        self.bit_us = 0.0;
    }

    /// The number of rows. There is at least 1, and the last can be empty.
    pub fn num_rows(&self) -> usize {
        self.rows
    }

    /// The most bits a row holds.
    pub fn max_bits(&self) -> usize {
        self.max_bits
    }

    /// Bits and rows dropped because the buffer was full since the last [`clear`](Self::clear).
    pub fn overflow(&self) -> u32 {
        self.overflow
    }

    /// The length of row `r` in bits (0 for a row that does not exist).
    pub fn len(&self, r: usize) -> usize {
        if r < self.rows {
            self.lens[r]
        } else {
            0
        }
    }

    /// Whether every row is empty.
    pub fn is_empty(&self) -> bool {
        self.lens[..self.rows].iter().all(|&l| l == 0)
    }

    /// Whether the current (last) row is empty.
    pub fn row_empty(&self) -> bool {
        self.lens[self.rows - 1] == 0
    }

    /// Row `r` (empty for a row that does not exist).
    pub fn row(&self, r: usize) -> Row<'_> {
        if r >= self.rows {
            return Row { bytes: &[], len: 0 };
        }
        let len = self.lens[r];
        let a = r * self.row_bytes;
        Row {
            bytes: &self.data[a..a + len.div_ceil(8)],
            len,
        }
    }

    /// Every row in order.
    pub fn iter(&self) -> impl Iterator<Item = Row<'_>> + '_ {
        (0..self.rows).map(move |r| self.row(r))
    }

    /// Bit `i` of row `r` (0 past the end).
    #[inline]
    pub fn bit(&self, r: usize, i: usize) -> u8 {
        if r >= self.rows || i >= self.lens[r] {
            return 0;
        }
        (self.data[r * self.row_bytes + i / 8] >> (7 - i % 8)) & 1
    }

    /// Appends a bit (its lowest bit) to the current row.
    #[inline]
    pub fn push(&mut self, bit: u8) {
        let r = self.rows - 1;
        let n = self.lens[r];
        if n >= self.max_bits {
            self.overflow = self.overflow.saturating_add(1);
            return;
        }
        let at = r * self.row_bytes + n / 8;
        if n.is_multiple_of(8) {
            self.data[at] = 0;
        }
        self.data[at] |= (bit & 1) << (7 - n % 8);
        self.lens[r] = n + 1;
    }

    /// Starts a new row (counted as overflow when the buffer has no room).
    pub fn new_row(&mut self) {
        if self.rows < self.lens.len() {
            self.lens[self.rows] = 0;
            self.rows += 1;
        } else {
            self.overflow = self.overflow.saturating_add(1);
        }
    }

    /// Removes empty rows at the end (one row always remains).
    pub fn trim(&mut self) {
        while self.rows > 1 && self.lens[self.rows - 1] == 0 {
            self.rows -= 1;
        }
    }

    /// Inverts every bit of every row (bits past each row's length stay 0).
    pub fn invert(&mut self) {
        for r in 0..self.rows {
            let n = self.lens[r];
            let a = r * self.row_bytes;
            for (i, b) in self.data[a..a + n.div_ceil(8)].iter_mut().enumerate() {
                *b = !*b;
                let left = n - i * 8;
                if left < 8 {
                    *b &= !(0xFFu8 >> left);
                }
            }
        }
    }

    /// Reverses the bit order within every byte of every row (devices that send each byte least significant bit first). A
    /// partial last byte is reversed as a whole byte, so its bits move to the low end.
    pub fn reflect(&mut self) {
        for r in 0..self.rows {
            let a = r * self.row_bytes;
            let n = self.lens[r].div_ceil(8);
            sdrkit_util_bits::pack::reverse_each_byte(&mut self.data[a..a + n]);
        }
    }

    /// The first position at or after `start` in row `r` where the top `bits` bits of `pattern` (MSB first) appear, or `None`.
    pub fn search(&self, r: usize, start: usize, pattern: &[u8], bits: usize) -> Option<usize> {
        if bits == 0 || bits > pattern.len() * 8 {
            return None;
        }
        let n = self.len(r);
        let mut i = start;
        while i + bits <= n {
            if (0..bits).all(|k| self.bit(r, i + k) == (pattern[k / 8] >> (7 - k % 8)) & 1) {
                return Some(i);
            }
            i += 1;
        }
        None
    }

    /// Writes `n` bits of row `r` from `pos` into `out` (MSB first, zero padded). Bits past the row read as 0. Errors when `out`
    /// is shorter than `ceil(n / 8)` bytes.
    pub fn extract(&self, r: usize, pos: usize, n: usize, out: &mut [u8]) -> Result<(), Error> {
        let need = n.div_ceil(8);
        if out.len() < need {
            return Err(Error::OutputTooShort { need });
        }
        out[..need].fill(0);
        for k in 0..n {
            out[k / 8] |= self.bit(r, pos + k) << (7 - k % 8);
        }
        Ok(())
    }

    /// `n` bits (at most 64) of row `r` from `pos` as a number, the first bit the most significant (0 past the row).
    pub fn bits_u64(&self, r: usize, pos: usize, n: usize) -> u64 {
        (0..n.min(64)).fold(0u64, |a, k| (a << 1) | self.bit(r, pos + k) as u64)
    }

    /// The first row of at least `min_bits` bits that appears at least `k` times (same length, same bits, itself included).
    pub fn repeated_row(&self, k: usize, min_bits: usize) -> Option<usize> {
        (0..self.rows).find(|&i| {
            self.lens[i] >= min_bits
                && (0..self.rows)
                    .filter(|&j| self.row(j) == self.row(i))
                    .count()
                    >= k
        })
    }

    /// Manchester-decodes row `r` of `self` from bit `pos` into `out` (cleared first, one row): each pair of bits decodes to one bit
    /// by `conv` (`sdrkit_util_bits::line::Manchester`). [`ManchesterConv::Ieee`] reads 01 as 1 and 10 as 0. Stops at the first pair
    /// without a change, at the end of the row, or after `max` bits. Returns the bits decoded.
    pub fn decode_manchester(
        &self,
        r: usize,
        pos: usize,
        max: usize,
        conv: ManchesterConv,
        out: &mut BitRows,
    ) -> usize {
        out.clear();
        let n = self.len(r);
        let mut i = pos;
        while i + 1 < n && out.lens[0] < max {
            let pairs = ((n - i) / 2).min(PIECE / 2).min(max - out.lens[0]);
            out.chips.clear();
            out.chips.extend((0..2 * pairs).map(|k| self.bit(r, i + k)));
            out.bits.clear();
            let stopped = Manchester::decode(&out.chips, conv, &mut out.bits).is_err();
            out.push_scratch();
            if stopped {
                break;
            }
            i += 2 * pairs;
        }
        out.lens[0]
    }

    /// FM0-decodes (biphase space, `sdrkit_util_bits::line::Biphase`) row `r` of `self` from bit `pos` into `out` (cleared first, one
    /// row): every bit cell starts with a change of level, and a 1 has no change in its middle, a 0 has one. The clock phase is
    /// not known, so both phases (cells starting at `pos` and at `pos + 1`) are tried and the longer result is kept. A cell
    /// needs the bit before it, so a cell at bit 0 moves to bit 2. Stops at the first cell without its starting change,
    /// at the end of the row, or after `max` bits. Returns the bits decoded.
    ///
    /// Some device documentation calls this code "differential Manchester". That name belongs to the IEEE 802.5 code, where the
    /// change in the middle is always there (`sdrkit_util_bits::line::DiffManchester`).
    pub fn decode_fm0(&self, r: usize, pos: usize, max: usize, out: &mut BitRows) -> usize {
        out.clear();
        let mut best = (0usize, 0usize);
        for phase in 0..2 {
            let n = self.fm0_run(r, pos + phase, max, out, false);
            if n > best.0 {
                best = (n, phase);
            }
        }
        if best.0 > 0 {
            self.fm0_run(r, pos + best.1, max, out, true);
        }
        out.lens[0]
    }

    /// Decodes FM0 cells starting at `start` with the scratch of `out`. It returns how many, and appends them to `out` when `write` is true.
    fn fm0_run(&self, r: usize, start: usize, max: usize, out: &mut BitRows, write: bool) -> usize {
        let n = self.len(r);
        // A cell needs the bit before it: a cell at 0 moves on by one cell.
        let start = if start == 0 { 2 } else { start };
        if start >= n {
            return 0;
        }
        let mut coder = Biphase::new(BiphaseKind::Space, self.bit(r, start - 1));
        let (mut done, mut i) = (0usize, start);
        while i + 1 < n && done < max {
            let pairs = ((n - i) / 2).min(PIECE / 2).min(max - done);
            out.chips.clear();
            out.chips.extend((0..2 * pairs).map(|k| self.bit(r, i + k)));
            out.bits.clear();
            let stopped = coder.decode(&out.chips, &mut out.bits).is_err();
            done += out.bits.len();
            if write {
                out.push_scratch();
            }
            if stopped {
                break;
            }
            i += 2 * pairs;
        }
        done
    }

    fn push_scratch(&mut self) {
        for k in 0..self.bits.len() {
            let b = self.bits[k];
            self.push(b);
        }
    }
}
