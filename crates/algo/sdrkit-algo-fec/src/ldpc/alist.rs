//! The alist text format of D. MacKay for sparse parity-check matrices: read, write, and turn into an [`LdpcCode`].
//!
//! Standard: D. MacKay, "alist format", Encyclopedia of Sparse Graph Codes
//! (<http://www.inference.org.uk/mackay/codes/alist.html>).

use super::{InfoAt, LdpcCode};
use crate::error::{FecError, Param, Result};
use std::fmt;

/// Why an alist text was refused: the reason and where it was found.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AlistError {
    /// The number (1-based, comments not counted) where the problem was found. One past the last number when the text ends early.
    pub token: usize,
    /// The line (1-based) of that number. The last line when the text ends early.
    pub line: usize,
    /// What is wrong.
    pub reason: AlistFault,
}

/// What is wrong in an alist text.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum AlistFault {
    /// A word is not a whole number below 2^32.
    NotANumber,
    /// The text ends before the matrix is complete.
    EndsEarly,
    /// The matrix has no columns or no rows.
    Empty,
    /// The text holds fewer numbers than the matrix size or the weights need.
    TooShort,
    /// A weight is above the largest weight, or the largest weight is above the matrix size.
    WeightTooLarge,
    /// The column weights and the row weights have different totals.
    WeightTotals,
    /// An index is above the matrix size.
    IndexTooLarge,
    /// A list names an index twice.
    IndexTwice,
    /// A row list and the column lists disagree.
    ListsDisagree,
    /// Nonzero numbers follow the last row list.
    ExtraNumbers,
}

impl fmt::Display for AlistError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "alist: {} (number {}, line {})",
            self.reason, self.token, self.line
        )
    }
}

impl std::error::Error for AlistError {}

impl fmt::Display for AlistFault {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            AlistFault::NotANumber => "a word is not a whole number below 2^32",
            AlistFault::EndsEarly => "the text ends early",
            AlistFault::Empty => "the matrix has no columns or no rows",
            AlistFault::TooShort => "the text has too few numbers",
            AlistFault::WeightTooLarge => "a weight is too large",
            AlistFault::WeightTotals => {
                "the column weights and the row weights have different totals"
            }
            AlistFault::IndexTooLarge => "an index is above the matrix size",
            AlistFault::IndexTwice => "a list names an index twice",
            AlistFault::ListsDisagree => "a row list and the column lists disagree",
            AlistFault::ExtraNumbers => "nonzero numbers follow the last row list",
        })
    }
}

impl From<AlistError> for FecError {
    fn from(e: AlistError) -> Self {
        FecError::Alist(e)
    }
}

/// A sparse binary matrix of `n` columns, held as the sorted column indices (0-based) of each row. As a parity-check matrix,
/// each row is a check and each column a code word bit.
///
/// The alist text holds whitespace-separated numbers: the columns `N` and rows `M`; the largest column and row weights; the `N`
/// column weights; the `M` row weights; then for each column its rows, and for each row its columns, all 1-based. Zeros pad a
/// list to the largest weight and are skipped. A line that starts with `#` is a comment. The column lists and the row lists
/// must describe the same matrix.
///
/// ```
/// use sdrkit_algo_fec::ldpc::alist::Alist;
/// // The (7,4) Hamming code: three checks on seven bits.
/// let a = Alist::from_rows(7, vec![vec![0, 1, 2, 4], vec![1, 2, 3, 5], vec![0, 1, 3, 6]]).unwrap();
/// let text = a.to_text();
/// assert_eq!(Alist::parse(&text).unwrap(), a);
/// let code = a.to_code("hamming", 4, sdrkit_algo_fec::ldpc::InfoAt::Start).unwrap();
/// assert!(code.can_encode());
/// ```
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Alist {
    n: usize,
    rows: Vec<Vec<u32>>,
}

/// The numbers of an alist text with their lines, read one at a time.
struct Tokens {
    v: Vec<(u32, usize)>,
    at: usize,
    last_line: usize,
}

impl Tokens {
    fn err(&self, token: usize, reason: AlistFault) -> AlistError {
        let line = self.v.get(token).map_or(self.last_line, |t| t.1);
        AlistError {
            token: token + 1,
            line,
            reason,
        }
    }

    fn next(&mut self) -> std::result::Result<u32, AlistError> {
        match self.v.get(self.at) {
            Some(&(x, _)) => {
                self.at += 1;
                Ok(x)
            }
            None => Err(self.err(self.at, AlistFault::EndsEarly)),
        }
    }
}

impl Alist {
    /// A matrix of `n` columns from its rows (0-based column indices, in any order). An error when an index is not below `n`,
    /// when a row lists a column twice, or when `n` is 0.
    pub fn from_rows(n: usize, mut rows: Vec<Vec<u32>>) -> Result<Self> {
        if n == 0 {
            return Param::N.range(1, i64::MAX, 0);
        }
        for r in &mut rows {
            r.sort_unstable();
            if r.iter().any(|&c| c as usize >= n) {
                return Param::Checks.conflict(Param::N);
            }
            if r.windows(2).any(|w| w[0] == w[1]) {
                return Param::Checks.degenerate();
            }
        }
        Ok(Alist { n, rows })
    }

    /// The parity-check matrix of `code`, one row per check, columns in code word order. A bit that a check reaches twice
    /// cancels.
    pub fn from_code(code: &LdpcCode) -> Self {
        let mut rows = Vec::with_capacity(code.checks());
        code.for_each_check(|b| {
            let mut r: Vec<u32> = b.iter().map(|&x| x as u32).collect();
            r.sort_unstable();
            let mut out = Vec::with_capacity(r.len());
            let mut i = 0;
            while i < r.len() {
                if i + 1 < r.len() && r[i] == r[i + 1] {
                    i += 2;
                } else {
                    out.push(r[i]);
                    i += 1;
                }
            }
            rows.push(out);
        });
        Alist { n: code.n(), rows }
    }

    /// Reads an alist text. An error, with the number and line where it was found, when a token is not a number, the text ends
    /// early, a weight or an index is out of range, a list names an index twice, nonzero numbers follow the last list, or the
    /// column lists and the row lists do not describe the same matrix.
    pub fn parse(text: &str) -> std::result::Result<Self, AlistError> {
        let mut t = Tokens {
            v: Vec::new(),
            at: 0,
            last_line: 1,
        };
        for (li, line) in text.lines().enumerate() {
            t.last_line = li + 1;
            if line.trim_start().starts_with('#') {
                continue;
            }
            for w in line.split_whitespace() {
                let Ok(x) = w.parse::<u32>() else {
                    return Err(AlistError {
                        token: t.v.len() + 1,
                        line: li + 1,
                        reason: AlistFault::NotANumber,
                    });
                };
                t.v.push((x, li + 1));
            }
        }
        let n = t.next()? as usize;
        let m = t.next()? as usize;
        if n == 0 || m == 0 {
            return Err(t.err(t.at - 1, AlistFault::Empty));
        }
        // Each column and each row has a weight, so the size cannot pass the count of numbers.
        if n.saturating_add(m) > t.v.len() {
            return Err(t.err(0, AlistFault::TooShort));
        }
        let maxc = t.next()? as usize;
        if maxc > m {
            return Err(t.err(t.at - 1, AlistFault::WeightTooLarge));
        }
        let maxr = t.next()? as usize;
        if maxr > n {
            return Err(t.err(t.at - 1, AlistFault::WeightTooLarge));
        }
        let mut cw = Vec::with_capacity(n);
        let mut total_c = 0usize;
        for _ in 0..n {
            let w = t.next()? as usize;
            if w > maxc {
                return Err(t.err(t.at - 1, AlistFault::WeightTooLarge));
            }
            total_c += w;
            cw.push(w);
        }
        let mut rw = Vec::with_capacity(m);
        let mut total_r = 0usize;
        for _ in 0..m {
            let w = t.next()? as usize;
            if w > maxr {
                return Err(t.err(t.at - 1, AlistFault::WeightTooLarge));
            }
            total_r += w;
            rw.push(w);
        }
        if total_c != total_r {
            return Err(t.err(t.at - 1, AlistFault::WeightTotals));
        }
        if total_c > t.v.len() {
            return Err(t.err(t.at - 1, AlistFault::TooShort));
        }
        // `seen[i]` holds the number of the last list that named index `i`, to find an index named twice in one list.
        let mut seen = vec![0usize; n.max(m)];
        let mut list_no = 0usize;
        let mut list =
            |t: &mut Tokens, w: usize, limit: usize| -> std::result::Result<Vec<u32>, AlistError> {
                list_no += 1;
                let mut v = Vec::with_capacity(w);
                while v.len() < w {
                    let x = t.next()? as usize;
                    if x == 0 {
                        continue;
                    }
                    if x > limit {
                        return Err(t.err(t.at - 1, AlistFault::IndexTooLarge));
                    }
                    if std::mem::replace(&mut seen[x - 1], list_no) == list_no {
                        return Err(t.err(t.at - 1, AlistFault::IndexTwice));
                    }
                    v.push((x - 1) as u32);
                }
                Ok(v)
            };
        let mut from_cols: Vec<Vec<u32>> = vec![Vec::new(); m];
        for (c, &w) in cw.iter().enumerate() {
            let rs = list(&mut t, w, m)?;
            for r in rs {
                from_cols[r as usize].push(c as u32);
            }
        }
        let mut rows = Vec::with_capacity(m);
        for (r, &w) in rw.iter().enumerate() {
            // The first number of the list, past the zeros that pad the list before it.
            let first = (t.at..t.v.len()).find(|&i| t.v[i].0 != 0).unwrap_or(t.at);
            let mut cs = list(&mut t, w, n)?;
            cs.sort_unstable();
            if cs != from_cols[r] {
                return Err(t.err(first, AlistFault::ListsDisagree));
            }
            rows.push(cs);
        }
        if let Some(i) = (t.at..t.v.len()).find(|&i| t.v[i].0 != 0) {
            return Err(t.err(i, AlistFault::ExtraNumbers));
        }
        Ok(Alist { n, rows })
    }

    /// Columns (the code word length).
    pub fn n(&self) -> usize {
        self.n
    }

    /// Rows (the parity checks).
    pub fn m(&self) -> usize {
        self.rows.len()
    }

    /// The sorted column indices (0-based) of each row.
    pub fn rows(&self) -> &[Vec<u32>] {
        &self.rows
    }

    /// The alist text, laid out as D. MacKay's `write_alist` writes it: every list padded with zeros to the largest weight.
    pub fn to_text(&self) -> String {
        self.to_string()
    }

    /// The LDPC code of this parity-check matrix with `k` information bits at `info_at` (see [`LdpcCode::from_checks`]). An
    /// error when [`LdpcCode::from_checks`] refuses the sizes or a row is empty.
    pub fn to_code(&self, name: &str, k: usize, info_at: InfoAt) -> Result<LdpcCode> {
        let checks: Vec<&[u32]> = self.rows.iter().map(Vec::as_slice).collect();
        LdpcCode::from_checks(name, self.n, k, &checks, info_at)
    }
}

impl fmt::Display for Alist {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut cols: Vec<Vec<usize>> = vec![Vec::new(); self.n];
        for (r, row) in self.rows.iter().enumerate() {
            for &c in row {
                cols[c as usize].push(r + 1);
            }
        }
        let maxc = cols.iter().map(Vec::len).max().unwrap_or(0);
        let maxr = self.rows.iter().map(Vec::len).max().unwrap_or(0);
        let line =
            |f: &mut fmt::Formatter<'_>, v: &mut dyn Iterator<Item = usize>| -> fmt::Result {
                let mut first = true;
                for x in v {
                    if !first {
                        f.write_str(" ")?;
                    }
                    write!(f, "{x}")?;
                    first = false;
                }
                f.write_str("\n")
            };
        writeln!(f, "{} {}", self.n, self.rows.len())?;
        writeln!(f, "{maxc} {maxr}")?;
        line(f, &mut cols.iter().map(Vec::len))?;
        line(f, &mut self.rows.iter().map(Vec::len))?;
        for c in &cols {
            line(
                f,
                &mut c.iter().copied().chain(std::iter::repeat(0)).take(maxc),
            )?;
        }
        for r in &self.rows {
            line(
                f,
                &mut r
                    .iter()
                    .map(|&c| c as usize + 1)
                    .chain(std::iter::repeat(0))
                    .take(maxr),
            )?;
        }
        Ok(())
    }
}
