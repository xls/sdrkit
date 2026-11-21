//! The codes of the family. FT8, FT4, FT2 and JS8 use the tables that `sdrkit-algo-fec` ships. The FST4, FST4W, MSK144 and Q65
//! tables are not shipped, so a host loads them at run time from files: the binary codes in the alist format, the Q65 code over
//! GF(64) in the text form of [`NbCode::parse`].
//! Standard: QEX July/August 2020 "The FT4 and FT8 Communication Protocols" section 3.

use crate::mode::Mode;
use sdrkit_algo_fec::ldpc::alist::{Alist, AlistError};
use sdrkit_algo_fec::ldpc::{tables::wsjt, InfoAt, LdpcCode};
use sdrkit_algo_fec::nbldpc::NbCode;
use sdrkit_algo_fec::FecError;
use std::collections::BTreeMap;
use std::path::Path;
use std::sync::{Arc, OnceLock};

/// The (174,91) code of FT8, FT4 and FT2: 91 information bits (77 message bits and the CRC-14), then 83 parity bits.
pub fn ft8() -> &'static LdpcCode {
    static C: OnceLock<LdpcCode> = OnceLock::new();
    C.get_or_init(|| {
        LdpcCode::from_checks("ft8 (174,91)", 174, 91, &wsjt::FT8_CHECKS, InfoAt::Start)
            .expect("the FT8 table of sdrkit-algo-fec is consistent")
    })
}

/// The (174,87) code of JS8 in transmission order: 87 parity bits, then 87 information bits (75 frame bits and the CRC-12).
pub fn js8() -> &'static LdpcCode {
    static C: OnceLock<LdpcCode> = OnceLock::new();
    C.get_or_init(|| {
        LdpcCode::from_checks("js8 (174,87)", 174, 87, &wsjt::JS8_CHECKS, InfoAt::End)
            .expect("the JS8 table of sdrkit-algo-fec is consistent")
    })
}

/// A code that sdrkit does not ship and a host loads at run time.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CodeId {
    /// The (240,101) code of FST4.
    Fst4,
    /// The (240,74) code of FST4W.
    Fst4w,
    /// The (128,90) code of MSK144.
    Msk144,
    /// The (65,15) repeat-accumulate code of Q65 over GF(64), counted in six-bit symbols.
    Q65,
}

impl CodeId {
    /// Every code a host can load.
    pub const ALL: [CodeId; 4] = [CodeId::Fst4, CodeId::Fst4w, CodeId::Msk144, CodeId::Q65];

    /// The code of `mode`, if that mode needs a loaded code.
    pub fn of(mode: Mode) -> Option<CodeId> {
        match mode {
            Mode::Fst4 => Some(CodeId::Fst4),
            Mode::Fst4w => Some(CodeId::Fst4w),
            Mode::Msk144 => Some(CodeId::Msk144),
            Mode::Q65 => Some(CodeId::Q65),
            _ => None,
        }
    }

    /// The code length `n` (Q65: symbols).
    pub fn n(self) -> usize {
        match self {
            CodeId::Fst4 | CodeId::Fst4w => 240,
            CodeId::Msk144 => 128,
            CodeId::Q65 => 65,
        }
    }

    /// The information bits `k` (Q65: symbols).
    pub fn k(self) -> usize {
        match self {
            CodeId::Fst4 => 101,
            CodeId::Fst4w => 74,
            CodeId::Msk144 => 90,
            CodeId::Q65 => 15,
        }
    }

    /// Whether the code is over GF(64) (a file in the text form of [`NbCode::parse`]) and not a binary code (an alist file).
    pub fn over_gf64(self) -> bool {
        self == CodeId::Q65
    }

    /// A name for people, such as "FST4 LDPC (240,101)".
    pub fn name(self) -> &'static str {
        match self {
            CodeId::Fst4 => "FST4 LDPC (240,101)",
            CodeId::Fst4w => "FST4W LDPC (240,74)",
            CodeId::Msk144 => "MSK144 LDPC (128,90)",
            CodeId::Q65 => "Q65 GF(64) (65,15)",
        }
    }

    /// The file name [`CodeStore::load_dir`] looks for.
    pub fn file_name(self) -> &'static str {
        match self {
            CodeId::Fst4 => "fst4_240_101.alist",
            CodeId::Fst4w => "fst4w_240_74.alist",
            CodeId::Msk144 => "msk144_128_90.alist",
            CodeId::Q65 => "q65_65_15.nbcode",
        }
    }
}

/// Why a mode cannot decode or encode: its code is not there.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum CodeMissing {
    /// A host has not loaded the table of this code.
    #[error("No {} table: load {} to decode", .0.name(), .0.file_name())]
    Table(CodeId),
}

impl CodeMissing {
    /// Why `mode` cannot decode with the codes of `codes`, or `None` if it can.
    pub fn of(mode: Mode, codes: &CodeStore) -> Option<CodeMissing> {
        let id = CodeId::of(mode)?;
        (!codes.has(id)).then_some(CodeMissing::Table(id))
    }
}

/// Why a code table was refused.
#[derive(Clone, Debug, PartialEq, thiserror::Error)]
pub enum CodeError {
    /// The text is not an alist file.
    #[error(transparent)]
    Alist(#[from] AlistError),
    /// The matrix has the wrong size for the code.
    #[error("{} needs {} columns and at least {} rows, the file has {} columns and {} rows", code.name(), code.n(), code.n() - code.k(), found.0, found.1)]
    Size {
        /// The code.
        code: CodeId,
        /// Columns and rows found.
        found: (usize, usize),
    },
    /// The checks do not make a code.
    #[error(transparent)]
    Checks(#[from] FecError),
    /// The file of the code over GF(64) is not in the text form.
    #[error("{}: {fault}", code.file_name())]
    Text {
        /// The code whose file it is.
        code: CodeId,
        /// What the parser refused.
        fault: FecError,
    },
    /// The checks do not give an encoder with the information bits first.
    #[error("the checks of {} do not give an encoder with the information bits first", .0.name())]
    NoEncoder(CodeId),
    /// The file of a code cannot be read.
    #[error("{} cannot be read: {kind}", code.file_name())]
    Io {
        /// The code whose file it is.
        code: CodeId,
        /// What went wrong.
        kind: std::io::ErrorKind,
    },
}

/// A loaded code: the parity checks of one [`CodeId`], with the information bits (or symbols) first, checked to give an
/// encoder.
#[derive(Clone, Debug)]
pub struct CodeTable {
    id: CodeId,
    code: Loaded,
}

/// The two kinds of loaded code.
#[derive(Clone, Debug)]
enum Loaded {
    Bits(Arc<LdpcCode>),
    Symbols(Arc<NbCode>),
}

impl CodeTable {
    /// The code `id` from its parity checks (0-based code word bits, information bits first).
    ///
    /// # Errors
    ///
    /// [`CodeError::Size`] if the size is wrong, [`CodeError::Checks`] if the checks make no code, [`CodeError::NoEncoder`] if
    /// they give no encoder.
    pub fn from_checks(id: CodeId, n: usize, rows: &[Vec<u16>]) -> Result<CodeTable, CodeError> {
        if n != id.n() || rows.len() < id.n() - id.k() {
            return Err(CodeError::Size {
                code: id,
                found: (n, rows.len()),
            });
        }
        let checks: Vec<&[u16]> = rows.iter().map(Vec::as_slice).collect();
        let code = LdpcCode::from_checks(id.name(), n, id.k(), &checks, InfoAt::Start)?;
        if !code.can_encode() {
            return Err(CodeError::NoEncoder(id));
        }
        Ok(CodeTable {
            id,
            code: Loaded::Bits(Arc::new(code)),
        })
    }

    /// The code `id` over GF(64) from the text form of [`NbCode::parse`]: 64 values a symbol, `n` symbols, the information
    /// symbols the first `k`.
    ///
    /// # Errors
    ///
    /// [`CodeError::Text`] if the text is refused, [`CodeError::Size`] if the field or the size is wrong,
    /// [`CodeError::NoEncoder`] if the information symbols are not the first `k`.
    pub fn parse_symbols(id: CodeId, text: &str) -> Result<CodeTable, CodeError> {
        let code = NbCode::parse(text).map_err(|fault| CodeError::Text { code: id, fault })?;
        if !id.over_gf64() || code.q() != 64 || code.n() != id.n() || code.k() != id.k() {
            return Err(CodeError::Size {
                code: id,
                found: (code.n(), code.checks()),
            });
        }
        if code.info_positions().iter().enumerate().any(|(i, &p)| p as usize != i) {
            return Err(CodeError::NoEncoder(id));
        }
        Ok(CodeTable {
            id,
            code: Loaded::Symbols(Arc::new(code)),
        })
    }

    /// The code `id` from the text of its file: [`CodeTable::parse_symbols`] for a code over GF(64), else
    /// [`CodeTable::parse_alist`].
    ///
    /// # Errors
    ///
    /// The errors of the parser used.
    pub fn parse(id: CodeId, text: &str) -> Result<CodeTable, CodeError> {
        if id.over_gf64() {
            Self::parse_symbols(id, text)
        } else {
            Self::parse_alist(id, text)
        }
    }

    /// The code `id` from the text of an alist file.
    ///
    /// # Errors
    ///
    /// [`CodeError::Alist`] if the text is not an alist file, else the errors of [`CodeTable::from_checks`].
    pub fn parse_alist(id: CodeId, text: &str) -> Result<CodeTable, CodeError> {
        let a = Alist::parse(text)?;
        let rows: Vec<Vec<u16>> = a
            .rows()
            .iter()
            .map(|r| r.iter().map(|&c| c as u16).collect())
            .collect();
        Self::from_checks(id, a.n(), &rows)
    }

    /// Which code it is.
    pub fn id(&self) -> CodeId {
        self.id
    }

    /// The binary code (`None` for a code over GF(64)).
    pub fn code(&self) -> Option<&Arc<LdpcCode>> {
        match &self.code {
            Loaded::Bits(c) => Some(c),
            Loaded::Symbols(_) => None,
        }
    }

    /// The code over GF(64) (`None` for a binary code).
    pub fn symbol_code(&self) -> Option<&Arc<NbCode>> {
        match &self.code {
            Loaded::Symbols(c) => Some(c),
            Loaded::Bits(_) => None,
        }
    }
}

/// The loaded codes of a receiver or a generator. Cloning is cheap: the codes are shared and never change.
#[derive(Clone, Debug, Default)]
pub struct CodeStore {
    codes: BTreeMap<CodeId, Loaded>,
}

impl CodeStore {
    /// Adds (or replaces) a code.
    pub fn install(&mut self, table: CodeTable) {
        self.codes.insert(table.id, table.code);
    }

    /// Whether the code `id` is loaded.
    pub fn has(&self, id: CodeId) -> bool {
        self.codes.contains_key(&id)
    }

    /// The binary code `id`, if loaded.
    pub fn get(&self, id: CodeId) -> Option<&Arc<LdpcCode>> {
        match self.codes.get(&id)? {
            Loaded::Bits(c) => Some(c),
            Loaded::Symbols(_) => None,
        }
    }

    /// The code `id` over GF(64), if loaded.
    pub fn symbols(&self, id: CodeId) -> Option<&Arc<NbCode>> {
        match self.codes.get(&id)? {
            Loaded::Symbols(c) => Some(c),
            Loaded::Bits(_) => None,
        }
    }

    /// Loads every code whose file ([`CodeId::file_name`]) is in the folder `dir`, and returns the codes loaded.
    ///
    /// # Errors
    ///
    /// [`CodeError::Io`] for a file that cannot be read, and the errors of [`CodeTable::parse`] for a file that is
    /// refused. Nothing is loaded then.
    pub fn load_dir(&mut self, dir: &Path) -> Result<Vec<CodeId>, CodeError> {
        let mut found = Vec::new();
        for id in CodeId::ALL {
            let p = dir.join(id.file_name());
            if !p.is_file() {
                continue;
            }
            let text = std::fs::read_to_string(&p).map_err(|e| CodeError::Io {
                code: id,
                kind: e.kind(),
            })?;
            found.push(CodeTable::parse(id, &text)?);
        }
        let ids = found.iter().map(CodeTable::id).collect();
        for t in found {
            self.install(t);
        }
        Ok(ids)
    }
}
