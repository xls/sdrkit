//! The tables of GY/T 268.1 that have no free source: the LDPC base matrices and the bit interleavers. The receiver and the
//! generator load them at run time from a JSON file and decode nothing without them.
//!
//! Standard: GY/T 268.1-2013 (CDR physical layer), clauses on the LDPC code and the bit interleaving.

use sdrkit_algo_fec::ldpc::LdpcCode;
use std::collections::BTreeMap;

/// Circulant size of the LDPC codes.
pub const Z: usize = 256;
/// Bits of an LDPC code word.
pub const N: usize = 9216;
/// Column groups of a base matrix.
pub const COLS: usize = N / Z;
/// Coded bits of the system information.
pub const SI_BITS: usize = 216;

sdrkit_core::choice_enum! {
    /// The four LDPC code rates, by their code in the system information.
    #[derive(PartialOrd, Ord, serde::Serialize)]
    pub enum LdpcRate {
        /// 1/4.
        R1_4 = ("0", "1/4"),
        /// 1/3.
        R1_3 = ("1", "1/3"),
        /// 1/2.
        R1_2 = ("2", "1/2"),
        /// 3/4.
        R3_4 = ("3", "3/4"),
    }
}

impl LdpcRate {
    /// Every rate, in the order of the system information index.
    pub const ALL: [LdpcRate; 4] = [
        LdpcRate::R1_4,
        LdpcRate::R1_3,
        LdpcRate::R1_2,
        LdpcRate::R3_4,
    ];
    /// From the system information index (0 1/4, 1 1/3, 2 1/2, 3 3/4).
    pub fn from_index(i: u32) -> Option<LdpcRate> {
        Self::ALL.get(i as usize).copied()
    }
    /// The index.
    pub fn index(self) -> u32 {
        self as u32
    }
    /// Information column groups.
    pub fn kb(self) -> usize {
        [9, 12, 18, 27][self as usize]
    }
    /// Information bits per code word.
    pub fn k(self) -> usize {
        self.kb() * Z
    }
    /// "3/4".
    pub fn text(self) -> &'static str {
        ["1/4", "1/3", "1/2", "3/4"][self as usize]
    }
    /// From its text.
    pub fn from_text(s: &str) -> Option<LdpcRate> {
        Self::ALL.iter().copied().find(|r| r.text() == s)
    }
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct RawCode {
    rate: String,
    rows: Vec<Vec<(u16, u16)>>,
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct RawTables {
    #[serde(default)]
    note: String,
    #[serde(default)]
    ldpc: Vec<RawCode>,
    si: Vec<u32>,
    #[serde(default)]
    sdis: BTreeMap<String, Vec<u32>>,
    msds: Vec<u32>,
}

/// The loaded tables. Each interleaver is a destination table: coded bit `i` is sent at position `dest[i]`.
#[derive(Clone, Debug)]
pub struct CdrTables {
    note: String,
    codes: [Option<LdpcCode>; 4],
    si: Vec<u32>,
    sdis: BTreeMap<usize, Vec<u32>>,
    msds: Vec<u32>,
}

/// An interleaver of the tables file.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Interleaver {
    /// The interleaver of the system information.
    Si,
    /// The interleaver of the MSDS code words.
    Msds,
    /// The SDIS interleaver of this many bits.
    Sdis {
        /// Its length in bits.
        bits: usize,
    },
}

impl std::fmt::Display for Interleaver {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Interleaver::Si => f.write_str("si"),
            Interleaver::Msds => f.write_str("msds"),
            Interleaver::Sdis { bits } => write!(f, "sdis {bits}"),
        }
    }
}

/// Why an interleaver table is not a permutation.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum PermutationFault {
    /// The table has another length.
    #[error("{got} entries, the length is {expected}")]
    Length {
        /// The entries given.
        got: usize,
        /// The length.
        expected: usize,
    },
    /// An entry is beyond the last position.
    #[error("entry {entry} is {position}, beyond {last}")]
    Beyond {
        /// The entry.
        entry: usize,
        /// Its position.
        position: usize,
        /// The last position.
        last: usize,
    },
    /// A position is used twice.
    #[error("position {position} is used twice")]
    Twice {
        /// The position.
        position: usize,
    },
}

/// Why a tables file is refused. The text starts with the field of the file that is wrong.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum TablesError {
    /// The file cannot be read.
    #[error("tables file: cannot be read ({kind})")]
    Read {
        /// What the system said.
        kind: std::io::ErrorKind,
    },
    /// The text is not JSON of the tables.
    #[error("tables file: not the JSON of the tables at line {line}, column {column}")]
    Json {
        /// The line.
        line: usize,
        /// The column.
        column: usize,
    },
    /// A code has a rate that is not one of the four.
    #[error("ldpc: a rate that is not 1/4, 1/3, 1/2 or 3/4")]
    UnknownRate,
    /// A code has another number of rows than its base matrix.
    #[error("ldpc rate {}: {got} rows, the base matrix has {expected}", rate.text())]
    Rows {
        /// The rate.
        rate: LdpcRate,
        /// The rows given.
        got: usize,
        /// The rows of the base matrix.
        expected: usize,
    },
    /// A column is beyond the base matrix.
    #[error("ldpc rate {} row {row}: column {column} is beyond the base matrix (40 columns)", rate.text())]
    Column {
        /// The rate.
        rate: LdpcRate,
        /// The row.
        row: usize,
        /// The column.
        column: u16,
    },
    /// A shift is not below the lift size.
    #[error("ldpc rate {} row {row}: shift {shift} is not below the lift size", rate.text())]
    Shift {
        /// The rate.
        rate: LdpcRate,
        /// The row.
        row: usize,
        /// The shift.
        shift: u16,
    },
    /// The code cannot be made.
    #[error("ldpc rate {}: {error}", rate.text())]
    Code {
        /// The rate.
        rate: LdpcRate,
        /// Why.
        error: sdrkit_algo_fec::FecError,
    },
    /// A rate is given twice.
    #[error("ldpc: rate {} is given twice", rate.text())]
    RateTwice {
        /// The rate.
        rate: LdpcRate,
    },
    /// An interleaver is not a permutation.
    #[error("{interleaver}: {fault}")]
    Permutation {
        /// The interleaver.
        interleaver: Interleaver,
        /// What is wrong.
        fault: PermutationFault,
    },
    /// A key of `sdis` is not a length in bits.
    #[error("sdis: a key that is not a length in bits")]
    SdisKey,
}

/// Makes sure that `dest` is a permutation of `0..n`.
fn check_permutation(interleaver: Interleaver, dest: &[u32], n: usize) -> Result<(), TablesError> {
    let refuse = |fault| TablesError::Permutation { interleaver, fault };
    if dest.len() != n {
        return Err(refuse(PermutationFault::Length {
            got: dest.len(),
            expected: n,
        }));
    }
    let mut seen = vec![false; n];
    for (entry, &d) in dest.iter().enumerate() {
        let position = d as usize;
        if position >= n {
            return Err(refuse(PermutationFault::Beyond {
                entry,
                position,
                last: n - 1,
            }));
        }
        if std::mem::replace(&mut seen[position], true) {
            return Err(refuse(PermutationFault::Twice { position }));
        }
    }
    Ok(())
}

impl CdrTables {
    /// Reads the tables from their JSON text.
    ///
    /// # Errors
    ///
    /// [`TablesError`]: its text starts with the field that is wrong.
    pub fn from_json(text: &str) -> Result<CdrTables, TablesError> {
        let raw: RawTables = serde_json::from_str(text).map_err(|e| TablesError::Json {
            line: e.line(),
            column: e.column(),
        })?;
        let mut codes: [Option<LdpcCode>; 4] = Default::default();
        for c in &raw.ldpc {
            let rate = LdpcRate::from_text(&c.rate).ok_or(TablesError::UnknownRate)?;
            let expected = COLS - rate.kb();
            if c.rows.len() != expected {
                return Err(TablesError::Rows {
                    rate,
                    got: c.rows.len(),
                    expected,
                });
            }
            for (row, r) in c.rows.iter().enumerate() {
                if let Some(&(column, _)) = r.iter().find(|e| e.0 as usize >= COLS) {
                    return Err(TablesError::Column { rate, row, column });
                }
                if let Some(&(_, shift)) = r.iter().find(|e| e.1 as usize >= Z) {
                    return Err(TablesError::Shift { rate, row, shift });
                }
            }
            let rows: Vec<&[(u16, u16)]> = c.rows.iter().map(Vec::as_slice).collect();
            let name = format!("cdr ldpc {}", rate.text());
            let code = LdpcCode::from_qc(&name, Z, COLS, rate.k(), &rows)
                .map_err(|error| TablesError::Code { rate, error })?;
            if codes[rate as usize].replace(code).is_some() {
                return Err(TablesError::RateTwice { rate });
            }
        }
        check_permutation(Interleaver::Si, &raw.si, SI_BITS)?;
        check_permutation(Interleaver::Msds, &raw.msds, N)?;
        let mut sdis = BTreeMap::new();
        for (k, v) in raw.sdis {
            let bits: usize = k.parse().map_err(|_| TablesError::SdisKey)?;
            check_permutation(Interleaver::Sdis { bits }, &v, bits)?;
            sdis.insert(bits, v);
        }
        Ok(CdrTables {
            note: raw.note,
            codes,
            si: raw.si,
            sdis,
            msds: raw.msds,
        })
    }

    /// Reads the tables from the file at `path`.
    ///
    /// # Errors
    ///
    /// [`TablesError::Read`] if the file cannot be read, else as [`CdrTables::from_json`].
    pub fn load(path: &str) -> Result<CdrTables, TablesError> {
        let text =
            std::fs::read_to_string(path).map_err(|e| TablesError::Read { kind: e.kind() })?;
        Self::from_json(&text)
    }

    /// The note of the file (where the tables come from).
    pub fn note(&self) -> &str {
        &self.note
    }
    /// The LDPC code of `rate`, when the file has it.
    pub fn code(&self, rate: LdpcRate) -> Option<&LdpcCode> {
        self.codes[rate as usize].as_ref()
    }
    /// The interleaver of the 216 coded bits of the system information.
    pub fn si(&self) -> &[u32] {
        &self.si
    }
    /// The interleaver of the `n` coded bits of the service description channel, when the file has one of that length.
    pub fn sdis(&self, n: usize) -> Option<&[u32]> {
        self.sdis.get(&n).map(Vec::as_slice)
    }
    /// The interleaver of the bits of one LDPC code word.
    pub fn msds(&self) -> &[u32] {
        &self.msds
    }
    /// What the file lacks for a configuration with `sdis_bits` coded SDIS bits and LDPC `rate`, or `None` when it has it all.
    pub fn missing(&self, sdis_bits: usize, rate: LdpcRate) -> Option<String> {
        let mut m = Vec::new();
        if self.code(rate).is_none() {
            m.push(format!("the LDPC matrix of rate {}", rate.text()));
        }
        if self.sdis(sdis_bits).is_none() {
            m.push(format!("the SDIS interleaver of {sdis_bits} bits"));
        }
        (!m.is_empty()).then(|| m.join(" and "))
    }
}

/// Puts coded bit `i` at `out[dest[i]]`.
pub fn scatter<T: Copy>(dest: &[u32], x: &[T], out: &mut [T]) {
    for (&d, &v) in dest.iter().zip(x) {
        out[d as usize] = v;
    }
}

/// The inverse of [`scatter`]: `out[i] = x[dest[i]]`.
pub fn gather<T: Copy>(dest: &[u32], x: &[T], out: &mut [T]) {
    for (o, &d) in out.iter_mut().zip(dest) {
        *o = x[d as usize];
    }
}
