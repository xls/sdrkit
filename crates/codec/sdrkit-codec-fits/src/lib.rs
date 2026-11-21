//! FITS files: a writer and a reader of header and data units (HDUs) held in memory.
//!
//! A file is a primary HDU and optional extensions. Each HDU is a header of 80-character cards in 2880-byte records, ended by
//! `END` and padded with spaces, then its data padded with zero bytes to a whole record. The writer puts the mandatory cards
//! first (`SIMPLE` or `XTENSION`, `BITPIX`, `NAXIS`, `NAXISn`, `EXTEND` in a primary HDU followed by extensions, `PCOUNT`,
//! `GCOUNT`, and for a table `TFIELDS` and the `TTYPEn`, `TFORMn`, `TUNITn` of each column), then the caller's cards in the
//! caller's order. Values use the fixed format: a logical, an integer or a real ends in column 30, a string starts in column 11
//! with at least eight characters between its quotes. Pixels and table cells are big-endian; no `BZERO` or `BSCALE` is applied.
//!
//! Supported data: images (`IMAGE` extension or primary) with `BITPIX` 8, 16, 32, 64, -32 and -64, and binary tables
//! (`BINTABLE`) whose columns are `rB`, `rI`, `rJ`, `rK`, `rE` or `rD` (a repeat count `r` of one type).
//!
//! Standard: FITS Standard 4.0 (IAU FITS Working Group, 2016).
//!
//! ```
//! use sdrkit_codec_fits::{read, write, Card, Data, Hdu};
//! let mut primary = Hdu::image(16, vec![3, 2], vec![0.0, 1.0, 2.0, 3.0, 4.0, 255.0]);
//! primary.cards.push(Card::value("OBJECT", "Sun"));
//! let bytes = write(&[primary]);
//! assert_eq!(bytes.len(), 2 * 2880);
//! let hdus = read(&bytes).unwrap();
//! assert_eq!(hdus[0].str("OBJECT"), Some("Sun"));
//! let Data::Image { bitpix, axes, pixels } = &hdus[0].data else { panic!() };
//! assert_eq!((*bitpix, axes.as_slice(), pixels[5]), (16, &[3, 2][..], 255.0));
//! ```

#![warn(missing_docs)]
#![forbid(unsafe_code)]

/// The length of a header or data record, bytes.
pub const RECORD: usize = 2880;
/// The length of a card, bytes.
pub const CARD: usize = 80;

/// The value of a keyword card.
#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    /// A logical, `T` or `F`.
    Bool(bool),
    /// An integer.
    Int(i64),
    /// A real number.
    Real(f64),
    /// A character string.
    Str(String),
}

impl From<bool> for Value {
    fn from(v: bool) -> Self {
        Value::Bool(v)
    }
}
impl From<i64> for Value {
    fn from(v: i64) -> Self {
        Value::Int(v)
    }
}
impl From<f64> for Value {
    fn from(v: f64) -> Self {
        Value::Real(v)
    }
}
impl From<&str> for Value {
    fn from(v: &str) -> Self {
        Value::Str(v.to_string())
    }
}
impl From<String> for Value {
    fn from(v: String) -> Self {
        Value::Str(v)
    }
}

/// One header card.
#[derive(Clone, Debug, PartialEq)]
pub enum Card {
    /// `KEY = value / comment`. The key is at most 8 characters of `A-Z`, `0-9`, `-` and `_`.
    Value {
        /// The keyword.
        key: String,
        /// The value.
        value: Value,
        /// The comment after the value; empty for none.
        comment: String,
    },
    /// A commentary card: `COMMENT`, `HISTORY` or a blank keyword, with free text of at most 72 characters.
    Text {
        /// The keyword: `COMMENT`, `HISTORY` or empty.
        key: String,
        /// The text.
        text: String,
    },
}

impl Card {
    /// The card `key = value` with no comment.
    pub fn value(key: &str, value: impl Into<Value>) -> Card {
        Card::Value {
            key: key.to_string(),
            value: value.into(),
            comment: String::new(),
        }
    }

    /// A `COMMENT` card.
    pub fn comment(text: &str) -> Card {
        Card::Text {
            key: "COMMENT".into(),
            text: text.into(),
        }
    }

    /// A `HISTORY` card.
    pub fn history(text: &str) -> Card {
        Card::Text {
            key: "HISTORY".into(),
            text: text.into(),
        }
    }

    /// The keyword of the card.
    pub fn key(&self) -> &str {
        match self {
            Card::Value { key, .. } | Card::Text { key, .. } => key,
        }
    }

    /// The 80 characters of the card. A value that does not fit is cut at column 80.
    pub fn text(&self) -> String {
        let mut s = match self {
            Card::Text { key, text } => format!("{key:<8}{text}"),
            Card::Value {
                key,
                value,
                comment,
            } => {
                let v = match value {
                    Value::Bool(b) => format!("{:>20}", if *b { "T" } else { "F" }),
                    Value::Int(i) => format!("{i:>20}"),
                    Value::Real(r) => format!("{:>20}", real_text(*r)),
                    Value::Str(s) => format!("'{:<8}'", s.replace('\'', "''")),
                };
                let mut c = format!("{key:<8}= {v}");
                if !comment.is_empty() {
                    c.push_str(" / ");
                    c.push_str(comment);
                }
                c
            }
        };
        s.truncate(CARD);
        format!("{s:<80}")
    }
}

/// A real number in the fixed format: the shortest decimal text that reads back as `r`, with a decimal point, or the
/// exponent form when that is longer than 20 characters.
fn real_text(r: f64) -> String {
    if !r.is_finite() {
        return "0.".into();
    }
    let s = format!("{r}");
    let s = if s.contains('.') { s } else { format!("{s}.") };
    if s.len() <= 20 {
        s
    } else {
        format!("{r:.12E}")
    }
}

/// A column type of a binary table.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ColumnType {
    /// `B`: unsigned byte.
    U8,
    /// `I`: 16-bit integer.
    I16,
    /// `J`: 32-bit integer.
    I32,
    /// `K`: 64-bit integer.
    I64,
    /// `E`: 32-bit real.
    F32,
    /// `D`: 64-bit real.
    F64,
}

impl ColumnType {
    /// The `TFORM` letter.
    pub fn letter(self) -> char {
        match self {
            ColumnType::U8 => 'B',
            ColumnType::I16 => 'I',
            ColumnType::I32 => 'J',
            ColumnType::I64 => 'K',
            ColumnType::F32 => 'E',
            ColumnType::F64 => 'D',
        }
    }

    /// Bytes of one cell.
    pub fn size(self) -> usize {
        match self {
            ColumnType::U8 => 1,
            ColumnType::I16 => 2,
            ColumnType::I32 | ColumnType::F32 => 4,
            ColumnType::I64 | ColumnType::F64 => 8,
        }
    }

    fn of(letter: char) -> Option<ColumnType> {
        Some(match letter {
            'B' => ColumnType::U8,
            'I' => ColumnType::I16,
            'J' => ColumnType::I32,
            'K' => ColumnType::I64,
            'E' => ColumnType::F32,
            'D' => ColumnType::F64,
            _ => return None,
        })
    }
}

/// One column of a binary table: `repeat` cells of `kind` in each row.
#[derive(Clone, Debug, PartialEq)]
pub struct Column {
    /// `TTYPEn`.
    pub name: String,
    /// `TUNITn`; empty for none (then no card is written).
    pub unit: String,
    /// The cell type.
    pub kind: ColumnType,
    /// Cells per row.
    pub repeat: usize,
    /// The cells, row after row: `rows × repeat` values.
    pub values: Vec<f64>,
}

/// The data of an HDU.
#[derive(Clone, Debug, PartialEq)]
pub enum Data {
    /// No data (`NAXIS = 0`).
    None,
    /// An image: `axes[0]` (`NAXIS1`) varies fastest.
    Image {
        /// `BITPIX`: 8, 16, 32, 64, -32 or -64.
        bitpix: i32,
        /// `NAXIS1`, `NAXIS2`, ...
        axes: Vec<usize>,
        /// The pixels as stored (no scaling), `axes[0]` fastest.
        pixels: Vec<f64>,
    },
    /// A binary table.
    Table {
        /// `NAXIS2`.
        rows: usize,
        /// The columns.
        columns: Vec<Column>,
    },
}

/// One header and data unit.
#[derive(Clone, Debug, PartialEq)]
pub struct Hdu {
    /// The cards. The writer adds the mandatory cards itself and skips them here; the reader returns every card read except
    /// `END`.
    pub cards: Vec<Card>,
    /// The data.
    pub data: Data,
}

impl Hdu {
    /// An HDU with no data.
    pub fn empty() -> Hdu {
        Hdu {
            cards: Vec::new(),
            data: Data::None,
        }
    }

    /// An image HDU.
    pub fn image(bitpix: i32, axes: Vec<usize>, pixels: Vec<f64>) -> Hdu {
        Hdu {
            cards: Vec::new(),
            data: Data::Image {
                bitpix,
                axes,
                pixels,
            },
        }
    }

    /// A binary table HDU of `rows` rows.
    pub fn table(rows: usize, columns: Vec<Column>) -> Hdu {
        Hdu {
            cards: Vec::new(),
            data: Data::Table { rows, columns },
        }
    }

    /// The value of the first card `key`.
    pub fn get(&self, key: &str) -> Option<&Value> {
        self.cards.iter().find_map(|c| match c {
            Card::Value { key: k, value, .. } if k == key => Some(value),
            _ => None,
        })
    }

    /// The string value of `key`.
    pub fn str(&self, key: &str) -> Option<&str> {
        match self.get(key)? {
            Value::Str(s) => Some(s),
            _ => None,
        }
    }

    /// The integer value of `key`.
    pub fn int(&self, key: &str) -> Option<i64> {
        match self.get(key)? {
            Value::Int(i) => Some(*i),
            _ => None,
        }
    }

    /// The number value of `key` (an integer or a real).
    pub fn real(&self, key: &str) -> Option<f64> {
        match self.get(key)? {
            Value::Int(i) => Some(*i as f64),
            Value::Real(r) => Some(*r),
            _ => None,
        }
    }

    /// The column `name` of a table HDU.
    pub fn column(&self, name: &str) -> Option<&Column> {
        match &self.data {
            Data::Table { columns, .. } => columns.iter().find(|c| c.name == name),
            _ => None,
        }
    }
}

/// Why bytes are not a file this reader takes.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum FitsError {
    /// The bytes do not start with `SIMPLE  =                    T` or are not whole records.
    #[error("not a FITS file")]
    NotFits,
    /// The file ends inside a header or its data.
    #[error("the file ends early")]
    Truncated,
    /// A mandatory card is missing or has the wrong type.
    #[error("missing or bad card {0}")]
    BadCard(String),
    /// An extension type or a data type this reader does not take.
    #[error("unsupported: {0}")]
    Unsupported(String),
}

/// True for the keywords the writer makes itself.
fn mandatory(key: &str) -> bool {
    let indexed = |p: &str| {
        key.strip_prefix(p)
            .is_some_and(|n| !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()))
    };
    matches!(
        key,
        "SIMPLE" | "XTENSION" | "BITPIX" | "NAXIS" | "EXTEND" | "PCOUNT" | "GCOUNT" | "TFIELDS" | "END"
    ) || indexed("NAXIS")
        || indexed("TTYPE")
        || indexed("TFORM")
        || indexed("TUNIT")
}

fn pad(out: &mut Vec<u8>, fill: u8) {
    while !out.len().is_multiple_of(RECORD) {
        out.push(fill);
    }
}

fn put(out: &mut Vec<u8>, c: &Card) {
    out.extend_from_slice(c.text().as_bytes());
}

fn push_cell(out: &mut Vec<u8>, kind: ColumnType, v: f64) {
    match kind {
        ColumnType::U8 => out.push(v.round().clamp(0.0, 255.0) as u8),
        ColumnType::I16 => out.extend_from_slice(&(v.round() as i16).to_be_bytes()),
        ColumnType::I32 => out.extend_from_slice(&(v.round() as i32).to_be_bytes()),
        ColumnType::I64 => out.extend_from_slice(&(v.round() as i64).to_be_bytes()),
        ColumnType::F32 => out.extend_from_slice(&(v as f32).to_be_bytes()),
        ColumnType::F64 => out.extend_from_slice(&v.to_be_bytes()),
    }
}

fn pixel_type(bitpix: i32) -> Option<ColumnType> {
    Some(match bitpix {
        8 => ColumnType::U8,
        16 => ColumnType::I16,
        32 => ColumnType::I32,
        64 => ColumnType::I64,
        -32 => ColumnType::F32,
        -64 => ColumnType::F64,
        _ => return None,
    })
}

/// The bytes of a file of `hdus`: the first is the primary HDU (a table there is written as no data), the others are
/// extensions. A missing value of an image or a table is written as 0; an image with an unsupported `BITPIX` is written as
/// `BITPIX = -32`.
pub fn write(hdus: &[Hdu]) -> Vec<u8> {
    let mut out = Vec::new();
    for (i, h) in hdus.iter().enumerate() {
        let primary = i == 0;
        let mut head: Vec<Card> = Vec::new();
        head.push(if primary {
            Card::value("SIMPLE", true)
        } else {
            Card::value(
                "XTENSION",
                match h.data {
                    Data::Table { .. } => "BINTABLE",
                    _ => "IMAGE",
                },
            )
        });
        let table = !primary && matches!(h.data, Data::Table { .. });
        match &h.data {
            Data::Image { bitpix, axes, .. } => {
                let bp = if pixel_type(*bitpix).is_some() {
                    *bitpix
                } else {
                    -32
                };
                head.push(Card::value("BITPIX", i64::from(bp)));
                head.push(Card::value("NAXIS", axes.len() as i64));
                for (k, n) in axes.iter().enumerate() {
                    head.push(Card::value(&format!("NAXIS{}", k + 1), *n as i64));
                }
            }
            Data::Table { rows, columns } if table => {
                let width: usize = columns.iter().map(|c| c.repeat * c.kind.size()).sum();
                head.push(Card::value("BITPIX", 8i64));
                head.push(Card::value("NAXIS", 2i64));
                head.push(Card::value("NAXIS1", width as i64));
                head.push(Card::value("NAXIS2", *rows as i64));
            }
            _ => {
                head.push(Card::value("BITPIX", 8i64));
                head.push(Card::value("NAXIS", 0i64));
            }
        }
        if primary && hdus.len() > 1 {
            head.push(Card::value("EXTEND", true));
        }
        if !primary {
            head.push(Card::value("PCOUNT", 0i64));
            head.push(Card::value("GCOUNT", 1i64));
        }
        if let (true, Data::Table { columns, .. }) = (table, &h.data) {
            head.push(Card::value("TFIELDS", columns.len() as i64));
            for (k, c) in columns.iter().enumerate() {
                head.push(Card::value(&format!("TTYPE{}", k + 1), c.name.as_str()));
                head.push(Card::value(
                    &format!("TFORM{}", k + 1),
                    format!("{}{}", c.repeat, c.kind.letter()),
                ));
                if !c.unit.is_empty() {
                    head.push(Card::value(&format!("TUNIT{}", k + 1), c.unit.as_str()));
                }
            }
        }
        for c in head
            .iter()
            .chain(h.cards.iter().filter(|c| !mandatory(c.key())))
        {
            put(&mut out, c);
        }
        put(
            &mut out,
            &Card::Text {
                key: "END".into(),
                text: String::new(),
            },
        );
        pad(&mut out, b' ');
        match &h.data {
            Data::Image {
                bitpix,
                axes,
                pixels,
            } => {
                let kind = pixel_type(*bitpix).unwrap_or(ColumnType::F32);
                let n: usize = axes.iter().product();
                for k in 0..n {
                    push_cell(&mut out, kind, pixels.get(k).copied().unwrap_or(0.0));
                }
            }
            Data::Table { rows, columns } if table => {
                for r in 0..*rows {
                    for c in columns {
                        for j in 0..c.repeat {
                            let v = c.values.get(r * c.repeat + j).copied().unwrap_or(0.0);
                            push_cell(&mut out, c.kind, v);
                        }
                    }
                }
            }
            _ => {}
        }
        pad(&mut out, 0);
    }
    out
}

/// One card from its 80 characters. `None` for `END`.
fn parse_card(s: &str) -> Option<Card> {
    let key = s.get(0..8).unwrap_or(s).trim_end().to_string();
    if key == "END" {
        return None;
    }
    let rest = s.get(8..).unwrap_or("");
    let Some(v) = rest.strip_prefix("= ") else {
        return Some(Card::Text {
            key,
            text: rest.trim_end().to_string(),
        });
    };
    let v = v.trim_start();
    let (value, after) = if let Some(body) = v.strip_prefix('\'') {
        // A string ends at a quote that is not doubled.
        let b = body.as_bytes();
        let mut text = String::new();
        let mut i = 0;
        while i < b.len() {
            if b[i] == b'\'' {
                if b.get(i + 1) == Some(&b'\'') {
                    text.push('\'');
                    i += 2;
                    continue;
                }
                break;
            }
            text.push(b[i] as char);
            i += 1;
        }
        (
            Value::Str(text.trim_end().to_string()),
            body.get(i + 1..).unwrap_or(""),
        )
    } else {
        let (num, after) = v.split_once('/').unwrap_or((v, ""));
        let num = num.trim();
        let value = match num {
            "T" => Value::Bool(true),
            "F" => Value::Bool(false),
            _ => match num.parse::<i64>() {
                Ok(i) => Value::Int(i),
                Err(_) => Value::Real(num.replace('D', "E").parse().unwrap_or(f64::NAN)),
            },
        };
        return Some(Card::Value {
            key,
            value,
            comment: after.trim().to_string(),
        });
    };
    let comment = after
        .trim_start()
        .strip_prefix('/')
        .unwrap_or("")
        .trim()
        .to_string();
    Some(Card::Value {
        key,
        value,
        comment,
    })
}

fn cell(b: &[u8], kind: ColumnType) -> f64 {
    match kind {
        ColumnType::U8 => f64::from(b[0]),
        ColumnType::I16 => f64::from(i16::from_be_bytes([b[0], b[1]])),
        ColumnType::I32 => f64::from(i32::from_be_bytes([b[0], b[1], b[2], b[3]])),
        ColumnType::I64 => {
            i64::from_be_bytes([b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7]]) as f64
        }
        ColumnType::F32 => f64::from(f32::from_be_bytes([b[0], b[1], b[2], b[3]])),
        ColumnType::F64 => f64::from_be_bytes([b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7]]),
    }
}

/// A `TFORM` value: the repeat count (1 when absent) and the type.
fn tform(s: &str) -> Option<(usize, ColumnType)> {
    let s = s.trim();
    let digits = s.bytes().take_while(u8::is_ascii_digit).count();
    let repeat = if digits == 0 {
        1
    } else {
        s[..digits].parse().ok()?
    };
    let letter = s[digits..].chars().next()?;
    Some((repeat, ColumnType::of(letter)?))
}

/// The HDUs of a file, the primary first. Every card except `END` is kept in order. Images are read for `BITPIX` 8, 16,
/// 32, 64, -32 and -64; binary tables for the column types of [`ColumnType`].
///
/// # Errors
///
/// [`FitsError`] for bytes that are not such a file.
pub fn read(data: &[u8]) -> Result<Vec<Hdu>, FitsError> {
    if data.len() < RECORD
        || !data.len().is_multiple_of(RECORD)
        || !data.starts_with(b"SIMPLE  =                    T")
    {
        return Err(FitsError::NotFits);
    }
    let mut hdus = Vec::new();
    let mut pos = 0;
    while pos < data.len() {
        let mut cards = Vec::new();
        loop {
            let s = data.get(pos..pos + CARD).ok_or(FitsError::Truncated)?;
            pos += CARD;
            let s = String::from_utf8_lossy(s);
            match parse_card(&s) {
                Some(c) => cards.push(c),
                None => break,
            }
        }
        pos = pos.div_ceil(RECORD) * RECORD;
        let mut h = Hdu {
            cards,
            data: Data::None,
        };
        let need = |k: &str| {
            h.int(k)
                .filter(|v| *v >= 0)
                .map(|v| v as usize)
                .ok_or_else(|| FitsError::BadCard(k.into()))
        };
        let naxis = need("NAXIS")?;
        let bitpix = h.int("BITPIX").ok_or(FitsError::BadCard("BITPIX".into()))? as i32;
        let xt = h.str("XTENSION").map(str::to_string);
        let axes: Vec<usize> = (1..=naxis)
            .map(|k| need(&format!("NAXIS{k}")))
            .collect::<Result<_, _>>()?;
        let pcount = h.int("PCOUNT").unwrap_or(0).max(0) as usize;
        let size = if naxis == 0 {
            0
        } else {
            axes.iter().product::<usize>() * bitpix.unsigned_abs() as usize / 8 + pcount
        };
        let bytes = data.get(pos..pos + size).ok_or(FitsError::Truncated)?;
        match xt.as_deref() {
            Some("BINTABLE") => {
                let (width, rows) = (axes.first().copied().unwrap_or(0), axes.get(1).copied().unwrap_or(0));
                let n = need("TFIELDS")?;
                let mut columns = Vec::with_capacity(n);
                let mut at = 0;
                for k in 1..=n {
                    let form = h
                        .str(&format!("TFORM{k}"))
                        .ok_or_else(|| FitsError::BadCard(format!("TFORM{k}")))?;
                    let (repeat, kind) =
                        tform(form).ok_or_else(|| FitsError::Unsupported(form.to_string()))?;
                    let mut values = Vec::with_capacity(rows * repeat);
                    for r in 0..rows {
                        for j in 0..repeat {
                            let o = r * width + at + j * kind.size();
                            let b = bytes.get(o..o + kind.size()).ok_or(FitsError::Truncated)?;
                            values.push(cell(b, kind));
                        }
                    }
                    at += repeat * kind.size();
                    columns.push(Column {
                        name: h.str(&format!("TTYPE{k}")).unwrap_or("").to_string(),
                        unit: h.str(&format!("TUNIT{k}")).unwrap_or("").to_string(),
                        kind,
                        repeat,
                        values,
                    });
                }
                h.data = Data::Table { rows, columns };
            }
            None | Some("IMAGE") => {
                if naxis > 0 {
                    let kind = pixel_type(bitpix)
                        .ok_or_else(|| FitsError::Unsupported(format!("BITPIX {bitpix}")))?;
                    let pixels = bytes
                        .chunks_exact(kind.size())
                        .map(|b| cell(b, kind))
                        .collect();
                    h.data = Data::Image {
                        bitpix,
                        axes,
                        pixels,
                    };
                }
            }
            Some(other) => return Err(FitsError::Unsupported(other.to_string())),
        }
        pos = (pos + size).div_ceil(RECORD) * RECORD;
        hdus.push(h);
    }
    Ok(hdus)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cards_use_the_fixed_format() {
        assert_eq!(
            Card::value("SIMPLE", true).text(),
            format!("{:<80}", "SIMPLE  =                    T")
        );
        assert_eq!(&Card::value("NAXIS1", 631i64).text()[..30], "NAXIS1  =                  631");
        assert_eq!(&Card::value("BZERO", 0.0).text()[..30], "BZERO   =                   0.");
        assert_eq!(&Card::value("CDELT1", 0.25).text()[..30], "CDELT1  =                 0.25");
        assert_eq!(&Card::value("OBJECT", "Sun").text()[..20], "OBJECT  = 'Sun     '");
        assert_eq!(&Card::value("ORIGIN", "it's").text()[..20], "ORIGIN  = 'it''s   '");
        assert_eq!(Card::comment("").text(), format!("{:<80}", "COMMENT"));
        assert_eq!(Card::value("X", 1e-30).text().trim_end().len(), 30);
    }

    #[test]
    fn a_file_with_an_image_and_a_table_reads_back() {
        let mut p = Hdu::image(16, vec![4, 3], (0..12).map(f64::from).collect());
        p.cards.push(Card::comment("first"));
        p.cards.push(Card::value("CRVAL1", 45923.0));
        p.cards.push(Card::value("NAXIS", 9i64)); // mandatory cards of the caller are skipped
        let t = Hdu::table(
            1,
            vec![
                Column {
                    name: "TIME".into(),
                    unit: "s".into(),
                    kind: ColumnType::F64,
                    repeat: 4,
                    values: vec![0.0, 0.25, 0.5, 0.75],
                },
                Column {
                    name: "FREQUENCY".into(),
                    unit: String::new(),
                    kind: ColumnType::F32,
                    repeat: 3,
                    values: vec![200.0, 199.0, 198.0],
                },
            ],
        );
        let b = write(&[p, t]);
        assert_eq!(b.len(), 4 * RECORD);
        let h = read(&b).unwrap();
        assert_eq!(h.len(), 2);
        let keys: Vec<&str> = h[0].cards.iter().map(Card::key).collect();
        assert_eq!(
            keys,
            ["SIMPLE", "BITPIX", "NAXIS", "NAXIS1", "NAXIS2", "EXTEND", "COMMENT", "CRVAL1"]
        );
        assert_eq!(h[0].real("CRVAL1"), Some(45923.0));
        let Data::Image { pixels, axes, .. } = &h[0].data else {
            panic!("no image")
        };
        assert_eq!((axes.as_slice(), pixels[11]), (&[4, 3][..], 11.0));
        assert_eq!(h[1].str("XTENSION"), Some("BINTABLE"));
        assert_eq!(h[1].column("TIME").unwrap().values[3], 0.75);
        assert_eq!(h[1].column("FREQUENCY").unwrap().values, vec![200.0, 199.0, 198.0]);
        assert_eq!(h[1].column("TIME").unwrap().unit, "s");
    }

    #[test]
    fn bad_bytes_are_refused() {
        assert_eq!(read(b"SIMPLE"), Err(FitsError::NotFits));
        let mut b = write(&[Hdu::image(8, vec![3000], vec![1.0; 3000])]);
        b.truncate(2 * RECORD);
        assert_eq!(read(&b), Err(FitsError::Truncated));
    }
}
