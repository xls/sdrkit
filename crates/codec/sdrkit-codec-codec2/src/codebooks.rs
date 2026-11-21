//! The trained codebook tables and the pitch filter taps. They are not part of this crate. The host loads them at run time,
//! from memory or from files, and the loader makes sure that each table has the right size.
//!
//! File layout: one file per [`Table`], named by [`Table::file_name`]. A file holds the values as raw little-endian IEEE 754
//! `f32`, with no header, in row-major order: entry `j` of a table with dimension `k` is values `j k` to `j k + k - 1`. The
//! file loader also makes sure that the SHA-256 of each file is the published value ([`Table::sha256`]).

use crate::error::Error;
use crate::Mode;
use std::path::Path;

/// One table. The names are those of the tables of Codec 2. The number in a variant is the index of the table in its set.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Table {
    /// `lsp_cb[0..=9]`: the scalar LSP tables of mode 1300, in Hz (16, 16, 16, 16, 16, 16, 16, 8, 8 and 4 entries).
    Lsp(u8),
    /// `lsp_cbd[0..=9]`: the LSP difference tables of mode 3200, in Hz (32 entries each).
    LspDelta(u8),
    /// `lsp_cbjmv[0..=2]`: the LSP vector quantiser of mode 1200, in radians (512 vectors of 10, 5 and 5 values).
    LspVq(u8),
    /// `ge_cb[0]`: the joint pitch and energy quantiser of mode 1200 (256 pairs of octaves and dB).
    WoE,
    /// `newamp1vq_cb[0..=1]`: the two stages of the 700C vector quantiser, in dB (512 vectors of 20).
    Vq(u8),
    /// `newamp1_energy_cb[0]`: the 700C energy table, in dB (16 entries).
    Energy,
    /// The 48 taps of the low-pass filter of the pitch estimator. All modes need it.
    NlpFir,
}

/// The number of tables.
pub const TABLE_COUNT: usize = 28;

/// The file name, the dimension, the number of entries and the SHA-256 of each table, in [`Table::ALL`] order.
const INFO: [(&str, usize, usize, &str); TABLE_COUNT] = [
    (
        "lsp_cb_0.f32",
        1,
        16,
        "a257628f5f595fe25ea36ffeab8996dde33b84306723865f6db8c181a2d3e821",
    ),
    (
        "lsp_cb_1.f32",
        1,
        16,
        "659266857984e9347420fbed9c2149a9e28276a4c193d15c277d3714d403bfa9",
    ),
    (
        "lsp_cb_2.f32",
        1,
        16,
        "ed4f105f0d7edf7376af3752652ce1d522aea81da0f6c7a7f8e205b19fcd7de7",
    ),
    (
        "lsp_cb_3.f32",
        1,
        16,
        "c98d3857d8b929c658584210a6a02b2cb5b6d7a992a1564787bf409c866a6de4",
    ),
    (
        "lsp_cb_4.f32",
        1,
        16,
        "6f1f823281ef4b62942c2f1a8a8b8faaba275c9c439ffdfeaa44adaa4c94260f",
    ),
    (
        "lsp_cb_5.f32",
        1,
        16,
        "d60da722a404233ce682e46b00b2d6b82b015901131714eb2992335423b7bb49",
    ),
    (
        "lsp_cb_6.f32",
        1,
        16,
        "ad0d52a9529e7bc0357e359b2263e1a2669577662643abcabe827e8dc1e87a40",
    ),
    (
        "lsp_cb_7.f32",
        1,
        8,
        "d099cc6fa12b1d3292c9c477edc27f8c0ebc8f765c12e55527e01955db8bf358",
    ),
    (
        "lsp_cb_8.f32",
        1,
        8,
        "ce4a8e27d12777fa3cd4fc9d860d194bfa6a2deff0438b91bc518768eaafc499",
    ),
    (
        "lsp_cb_9.f32",
        1,
        4,
        "4c613d39eb9d83f60568f970b1230fdd0b8f85389eacbdb5ea6082acd8f80fc0",
    ),
    (
        "lsp_cbd_0.f32",
        1,
        32,
        "2bcef849c53a88c74e12c4f9ddf2c99f2cb2d829718d729056150fc9a6332c50",
    ),
    (
        "lsp_cbd_1.f32",
        1,
        32,
        "2bcef849c53a88c74e12c4f9ddf2c99f2cb2d829718d729056150fc9a6332c50",
    ),
    (
        "lsp_cbd_2.f32",
        1,
        32,
        "2bcef849c53a88c74e12c4f9ddf2c99f2cb2d829718d729056150fc9a6332c50",
    ),
    (
        "lsp_cbd_3.f32",
        1,
        32,
        "d79f93d1e888c0f47177972563403010d80366c56c84c6e60671b736e3a1f953",
    ),
    (
        "lsp_cbd_4.f32",
        1,
        32,
        "d79f93d1e888c0f47177972563403010d80366c56c84c6e60671b736e3a1f953",
    ),
    (
        "lsp_cbd_5.f32",
        1,
        32,
        "d79f93d1e888c0f47177972563403010d80366c56c84c6e60671b736e3a1f953",
    ),
    (
        "lsp_cbd_6.f32",
        1,
        32,
        "2bcef849c53a88c74e12c4f9ddf2c99f2cb2d829718d729056150fc9a6332c50",
    ),
    (
        "lsp_cbd_7.f32",
        1,
        32,
        "2bcef849c53a88c74e12c4f9ddf2c99f2cb2d829718d729056150fc9a6332c50",
    ),
    (
        "lsp_cbd_8.f32",
        1,
        32,
        "2bcef849c53a88c74e12c4f9ddf2c99f2cb2d829718d729056150fc9a6332c50",
    ),
    (
        "lsp_cbd_9.f32",
        1,
        32,
        "2bcef849c53a88c74e12c4f9ddf2c99f2cb2d829718d729056150fc9a6332c50",
    ),
    (
        "lsp_cbjmv_0.f32",
        10,
        512,
        "c8a6b52f07721bf79efa450b0e7752fb8268f52be071b5847a356d8be891d079",
    ),
    (
        "lsp_cbjmv_1.f32",
        5,
        512,
        "397188a3de5d8d8033d3615e3727afeaef4336db48228d81dd6c694ee15e8d2f",
    ),
    (
        "lsp_cbjmv_2.f32",
        5,
        512,
        "8dc2d522a73543e00c1ce8cc3a91e92422e472d0e7968d609954bd9330d5548d",
    ),
    (
        "ge_cb_0.f32",
        2,
        256,
        "9bfe3ab08bb60d5919dc0f623f8cdfcf0abfb2199b386dc26370aa0cb2d1f1e2",
    ),
    (
        "newamp1vq_cb_0.f32",
        20,
        512,
        "224a2a35a1c97a63efc2051453f1011fb5c2e2cc3d36d4833d45eb4fc3aa1fcb",
    ),
    (
        "newamp1vq_cb_1.f32",
        20,
        512,
        "6915194396fb5615a97e258ec8ff1a530242e8cb48a4c9c9ccb7b6f60795854b",
    ),
    (
        "newamp1_energy_cb_0.f32",
        1,
        16,
        "f97cf8b0f9e81b994c3ab97158958af41bc363af51c61f4ab9ed1805ffcf2e8d",
    ),
    (
        "nlp_fir_0.f32",
        1,
        48,
        "59c2bcc9acfc0b37d01296bc150a5e9f0ddb81f521208d8c201b5fa60c02c659",
    ),
];

impl Table {
    /// Every table, in a fixed order.
    pub const ALL: [Table; TABLE_COUNT] = [
        Table::Lsp(0),
        Table::Lsp(1),
        Table::Lsp(2),
        Table::Lsp(3),
        Table::Lsp(4),
        Table::Lsp(5),
        Table::Lsp(6),
        Table::Lsp(7),
        Table::Lsp(8),
        Table::Lsp(9),
        Table::LspDelta(0),
        Table::LspDelta(1),
        Table::LspDelta(2),
        Table::LspDelta(3),
        Table::LspDelta(4),
        Table::LspDelta(5),
        Table::LspDelta(6),
        Table::LspDelta(7),
        Table::LspDelta(8),
        Table::LspDelta(9),
        Table::LspVq(0),
        Table::LspVq(1),
        Table::LspVq(2),
        Table::WoE,
        Table::Vq(0),
        Table::Vq(1),
        Table::Energy,
        Table::NlpFir,
    ];

    /// The position of the table in [`Table::ALL`], or `None` if the index in the variant is out of range.
    pub fn position(self) -> Option<usize> {
        let (base, i, n) = match self {
            Table::Lsp(i) => (0, i, 10),
            Table::LspDelta(i) => (10, i, 10),
            Table::LspVq(i) => (20, i, 3),
            Table::WoE => (23, 0, 1),
            Table::Vq(i) => (24, i, 2),
            Table::Energy => (26, 0, 1),
            Table::NlpFir => (27, 0, 1),
        };
        ((i as usize) < n).then_some(base + i as usize)
    }

    fn info(self) -> (&'static str, usize, usize, &'static str) {
        self.position().map(|p| INFO[p]).unwrap_or(("?", 0, 0, ""))
    }

    /// The file name that [`Codebooks::load_dir`] reads. Empty for a table that does not exist.
    pub fn file_name(self) -> &'static str {
        self.info().0
    }

    /// The dimension `k`: the number of values in one entry.
    pub fn dim(self) -> usize {
        self.info().1
    }

    /// The number of entries `m`.
    pub fn entries(self) -> usize {
        self.info().2
    }

    /// The number of `f32` values (`k m`).
    pub fn len(self) -> usize {
        self.dim() * self.entries()
    }

    /// True for a table that does not exist (an index out of range).
    pub fn is_empty(self) -> bool {
        self.len() == 0
    }

    /// The published SHA-256 of the table file, as lowercase hexadecimal.
    pub fn sha256(self) -> &'static str {
        self.info().3
    }
}

/// The tables that the host supplies. Each table can be present or absent. An encoder or a decoder of a mode starts only if
/// every table of that mode (see [`Mode::tables`]) is present.
///
/// ```
/// use sdrkit_codec_codec2::{Codebooks, Mode, Table};
/// let mut books = Codebooks::new();
/// assert!(books.check(Mode::M700C).is_err());
/// books.set(Table::Energy, &[0.0; 16]).unwrap();
/// assert!(books.set(Table::Energy, &[0.0; 15]).is_err());
/// assert!(books.set(Table::Lsp(10), &[0.0; 4]).is_err());
/// ```
#[derive(Clone, Debug, PartialEq)]
pub struct Codebooks {
    tables: Vec<Option<Box<[f32]>>>,
}

impl Default for Codebooks {
    fn default() -> Self {
        Codebooks {
            tables: vec![None; TABLE_COUNT],
        }
    }
}

impl Codebooks {
    /// No tables.
    pub fn new() -> Self {
        Self::default()
    }

    /// Stores `values` as `table`. It does not compare a hash, so the host can supply its own data. Errors if the table does
    /// not exist, if the length is not [`Table::len`], or if a value is not finite.
    pub fn set(&mut self, table: Table, values: &[f32]) -> Result<(), Error> {
        let pos = table.position().ok_or(Error::UnknownTable(table))?;
        if values.len() != table.len() {
            return Err(Error::TableSize {
                table,
                expected: table.len(),
                got: values.len(),
            });
        }
        if let Some(index) = values.iter().position(|v| !v.is_finite()) {
            return Err(Error::TableValue { table, index });
        }
        self.tables[pos] = Some(values.into());
        Ok(())
    }

    /// Stores `table` from the bytes of its file. Errors if the SHA-256 of `bytes` is not [`Table::sha256`], and for the same
    /// reasons as [`Codebooks::set`].
    pub fn set_file_bytes(&mut self, table: Table, bytes: &[u8]) -> Result<(), Error> {
        table.position().ok_or(Error::UnknownTable(table))?;
        if bytes.len() != 4 * table.len() {
            return Err(Error::TableSize {
                table,
                expected: table.len(),
                got: bytes.len() / 4,
            });
        }
        let digest = sdrkit_algo_crypto::sha256(bytes);
        if !hex_equals(&digest, table.sha256()) {
            return Err(Error::TableHash(table));
        }
        let values: Vec<f32> = bytes
            .as_chunks::<4>()
            .0
            .iter()
            .map(|c| f32::from_le_bytes(*c))
            .collect();
        self.set(table, &values)
    }

    /// Reads `table` from the file at `path`. Errors if the file cannot be read, and for the same reasons as
    /// [`Codebooks::set_file_bytes`].
    pub fn load_file(&mut self, table: Table, path: &Path) -> Result<(), Error> {
        let bytes = std::fs::read(path).map_err(|e| Error::Io {
            table,
            reason: e.to_string(),
        })?;
        self.set_file_bytes(table, &bytes)
    }

    /// Reads every table whose file ([`Table::file_name`]) is in `dir`. A missing file leaves its table absent. Errors if a
    /// file is there but cannot be read, has the wrong size or has the wrong SHA-256.
    pub fn load_dir(dir: &Path) -> Result<Codebooks, Error> {
        let mut books = Codebooks::new();
        for table in Table::ALL {
            let path = dir.join(table.file_name());
            if path.is_file() {
                books.load_file(table, &path)?;
            }
        }
        Ok(books)
    }

    /// The values of `table`, if it is present.
    pub fn get(&self, table: Table) -> Option<&[f32]> {
        self.tables.get(table.position()?)?.as_deref()
    }

    /// Makes sure that every table of `mode` is present. Errors with the first missing table.
    pub fn check(&self, mode: Mode) -> Result<(), Error> {
        for &t in mode.tables() {
            if self.get(t).is_none() {
                return Err(Error::MissingTable(t));
            }
        }
        Ok(())
    }

    pub(crate) fn need(&self, table: Table) -> Result<Box<[f32]>, Error> {
        self.get(table)
            .map(Box::from)
            .ok_or(Error::MissingTable(table))
    }
}

/// True if `hex` is the lowercase hexadecimal form of `digest`.
fn hex_equals(digest: &[u8; 32], hex: &str) -> bool {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    hex.len() == 64
        && digest
            .iter()
            .zip(hex.as_bytes().chunks(2))
            .all(|(b, h)| h[0] == DIGITS[(b >> 4) as usize] && h[1] == DIGITS[(b & 15) as usize])
}
