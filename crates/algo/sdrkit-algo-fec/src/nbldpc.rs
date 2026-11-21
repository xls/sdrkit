//! Non-binary LDPC codes over GF(2^m) given as data, with an encoder and a sum-product decoder over symbol probabilities.
//!
//! A repeat-accumulate code over GF(q), such as the QRA codes of the WSJT family, is one kind of such code.
//!
//! ```
//! use sdrkit_algo_fec::nbldpc::{NbCode, NbDecoder};
//! // GF(4), three symbols, one check: x0 + x1 + x2 = 0. The information symbols are x0 and x1.
//! let code = NbCode::parse("4 3 2\n0:1 1:1 2:1\n").unwrap();
//! let mut word = [0u16; 3];
//! code.encode(&[1, 2], &mut word).unwrap();
//! assert_eq!(word, [1, 2, 3]);
//! // Receive x0 and x1 with some doubt and lose x2 (a row of equal values).
//! let probs = [
//!     0.1, 0.7, 0.1, 0.1, // x0
//!     0.1, 0.1, 0.7, 0.1, // x1
//!     1.0, 1.0, 1.0, 1.0, // x2, punctured
//! ];
//! let mut dec = NbDecoder::new(&code);
//! let mut hard = [0u16; 3];
//! dec.decode(&probs, 10, &mut hard).unwrap();
//! assert_eq!(hard, word);
//! ```

use crate::error::{Decoded, FecError, Param, Result, TextFault};
use crate::gf::Gf;
use std::fmt::Write as _;

/// The largest `m` the codes accept (GF(4096)).
pub const MAX_M: u8 = 12;

/// The largest code word length the codes accept. The encoder is built by elimination, which takes time in proportion to `n³`.
pub const MAX_N: usize = 4096;

/// The primitive polynomial used for GF(2^m) when a text code provides none, for `m` from 2 to 8. GF(64) uses x^6 + x + 1.
pub fn default_poly(m: u8) -> Option<u32> {
    match m {
        2 => Some(0x7),
        3 => Some(0xB),
        4 => Some(0x13),
        5 => Some(0x25),
        6 => Some(0x43),
        7 => Some(0x89),
        8 => Some(0x11D),
        _ => None,
    }
}

/// The shape of a repeat-accumulate code over GF(q), for [`NbCode::repeat_accumulate`].
///
/// The code is built in four steps:
/// 1. Repeat: information symbol `i` is written `repeats[i]` times, in order, which produces the sequence `u` of length
///    `E = Σ repeats`.
/// 2. Permute: `v[j] = u[permutation[j]]` for `j` in `0 .. E`. `permutation` must hold each of `0 .. E` once.
/// 3. Weight: `v[j]` is multiplied by `weights[j]`, a nonzero field element.
/// 4. Accumulate: `y[t] = y[t − 1] + Σ weights[j] v[j]` over the `combine` values `j` from `t × combine`, with `y[−1] = 0`.
///    `E` must be a multiple of `combine`, and there are `E / combine` accumulator outputs.
///
/// The code word is the `k` information symbols, then the accumulator outputs `y[0], y[1], ...`. Its length is
/// `k + E / combine`.
#[derive(Clone, Copy, Debug)]
pub struct RepeatAccumulate<'a> {
    /// How many times each information symbol is repeated. Its length is `k`, and each value is at least 1.
    pub repeats: &'a [usize],
    /// The permutation of the repeated sequence, length `E`.
    pub permutation: &'a [usize],
    /// The weight of each permuted value, length `E`, each in `1 .. q`.
    pub weights: &'a [u16],
    /// How many weighted values each accumulator step adds (1 for the usual repeat-accumulate code).
    pub combine: usize,
}

/// A non-binary LDPC code: `n` symbols in GF(2^m), parity checks `Σ h_j x_j = 0` with nonzero coefficients `h_j`, and `k`
/// information symbols at given positions.
#[derive(Clone, Debug)]
pub struct NbCode {
    field: Gf,
    n: usize,
    /// Positions of the information symbols, in the order the encoder takes them.
    info: Vec<u32>,
    /// Positions of the other symbols, in the order of the rows of `enc`.
    parity: Vec<u32>,
    /// Start of each check's edges in `sym` and `coef`, with one more entry at the end.
    check_start: Vec<u32>,
    sym: Vec<u32>,
    coef: Vec<u16>,
    /// `(n − k) × k`: parity symbol `j` is `Σ enc[j k + i] info[i]`.
    enc: Vec<u16>,
}

impl NbCode {
    /// A code over GF(2^m) with field polynomial `poly`, `n` symbols, the parity checks `checks` and the information symbols at
    /// `info`. Each check lists `(symbol, coefficient)` pairs. The encoder is built here by elimination over GF(q).
    ///
    /// Errors (`OutOfRange`, `Conflict`, `Degenerate`): `m` outside 2 to [`MAX_M`], a polynomial that is not primitive of degree `m`, `n` above
    /// [`MAX_N`], no information symbol or no parity symbol, an information position out of range or listed twice, an empty
    /// check, a symbol beyond `n` or listed twice in one check, a coefficient of 0 or not below `q`, and checks that do not fix
    /// the parity symbols from the information symbols (fewer than `n − k` independent checks on the parity positions, or a check
    /// that constrains the information symbols alone).
    pub fn new(
        m: u8,
        poly: u32,
        n: usize,
        checks: &[&[(usize, u16)]],
        info: &[usize],
    ) -> Result<Self> {
        let field = field_for(m, poly)?;
        let q = 1usize << m;
        if n > MAX_N {
            return Param::N.range(1, MAX_N as i64, n);
        }
        let k = info.len();
        if k == 0 || k >= n {
            return Param::K.range(1, n as i64 - 1, k);
        }
        let mut is_info = vec![false; n];
        for &p in info {
            if p >= n || is_info[p] {
                return Param::Info.range(0, n as i64 - 1, p);
            }
            is_info[p] = true;
        }
        let mut check_start = Vec::with_capacity(checks.len() + 1);
        check_start.push(0u32);
        let mut sym = Vec::new();
        let mut coef = Vec::new();
        let mut seen = vec![false; n];
        for c in checks {
            if c.is_empty() {
                return Param::Checks.degenerate();
            }
            for &(s, h) in c.iter() {
                if s >= n {
                    return Param::Checks.range(0, n as i64 - 1, s);
                }
                if h == 0 || h as usize >= q {
                    return Param::Coefficient.range(1, q as i64 - 1, h);
                }
                if seen[s] {
                    return Param::Checks.degenerate();
                }
                seen[s] = true;
                sym.push(s as u32);
                coef.push(h);
            }
            for &(s, _) in c.iter() {
                seen[s] = false;
            }
            check_start.push(sym.len() as u32);
        }
        let info: Vec<u32> = info.iter().map(|&p| p as u32).collect();
        let parity: Vec<u32> = (0..n as u32).filter(|&p| !is_info[p as usize]).collect();
        let enc = build_encoder(&field, n, &check_start, &sym, &coef, &info, &parity)?;
        Ok(NbCode {
            field,
            n,
            info,
            parity,
            check_start,
            sym,
            coef,
            enc,
        })
    }

    /// The repeat-accumulate code described by `ra` over GF(2^m) with field polynomial `poly` (see [`RepeatAccumulate`]).
    ///
    /// Check `t` is `y[t] + y[t − 1] + Σ weights[j] x[source of j] = 0` (without `y[t − 1]` for `t = 0`). If two values of one
    /// step come from the same information symbol, their weights are added.
    ///
    /// Errors (`OutOfRange`, `Conflict`, `Degenerate`): a repeat of 0, a permutation that is not a permutation of `0 .. E`, a weights list of the wrong
    /// length or with a value of 0 or not below `q`, a `combine` of 0 or one that does not divide `E`, and the errors of
    /// [`NbCode::new`].
    pub fn repeat_accumulate(m: u8, poly: u32, ra: &RepeatAccumulate<'_>) -> Result<Self> {
        let field = field_for(m, poly)?;
        let q = 1usize << m;
        let k = ra.repeats.len();
        if ra.repeats.contains(&0) {
            return Param::Repeats.range(1, i64::MAX, 0);
        }
        let e: usize = ra.repeats.iter().sum();
        if ra.permutation.len() != e || ra.weights.len() != e {
            return Param::Permutation.conflict(Param::Repeats);
        }
        let mut seen = vec![false; e];
        for &p in ra.permutation {
            if p >= e || seen[p] {
                return Param::Permutation.degenerate();
            }
            seen[p] = true;
        }
        if ra.weights.iter().any(|&w| w == 0 || w as usize >= q) {
            return Param::Weights.range(
                1,
                q as i64 - 1,
                ra.weights
                    .iter()
                    .copied()
                    .find(|&w| w == 0 || w as usize >= q)
                    .unwrap_or(0),
            );
        }
        if ra.combine == 0 || !e.is_multiple_of(ra.combine) {
            return Param::Combine.conflict(Param::Repeats);
        }
        // The information symbol of each position of the repeated sequence.
        let source: Vec<usize> = ra
            .repeats
            .iter()
            .enumerate()
            .flat_map(|(i, &r)| std::iter::repeat_n(i, r))
            .collect();
        let steps = e / ra.combine;
        let n = k + steps;
        let mut checks: Vec<Vec<(usize, u16)>> = Vec::with_capacity(steps);
        for t in 0..steps {
            let mut c: Vec<(usize, u16)> = Vec::with_capacity(ra.combine + 2);
            for j in t * ra.combine..(t + 1) * ra.combine {
                let s = source[ra.permutation[j]];
                match c.iter_mut().find(|(x, _)| *x == s) {
                    Some(entry) => entry.1 ^= ra.weights[j],
                    None => c.push((s, ra.weights[j])),
                }
            }
            c.retain(|&(_, h)| h != 0);
            if t > 0 {
                c.push((k + t - 1, 1));
            }
            c.push((k + t, 1));
            checks.push(c);
        }
        let refs: Vec<&[(usize, u16)]> = checks.iter().map(|c| c.as_slice()).collect();
        let info: Vec<usize> = (0..k).collect();
        Self::new(field.m(), field.poly(), n, &refs, &info)
    }

    /// Reads a code from text.
    ///
    /// The format is line by line. A `#` starts a comment that runs to the end of the line. Blank lines are skipped. Values
    /// are separated by spaces or tabs.
    /// - The first line is the header `q n k` or `q n k poly`. `q` is the field size `2^m`, `n` the code word length and `k` the
    ///   number of information symbols. `poly` is the field polynomial, in decimal or in hexadecimal after `0x`. Without it,
    ///   [`default_poly`] supplies the polynomial, which exists for `q` up to 256.
    /// - An optional line `info p0 p1 ...` lists the `k` information positions in encoder order. Without it, they are
    ///   `0 .. k`.
    /// - Every other line is one parity check: pairs `symbol:coefficient`, the symbol counted from 0 and the coefficient a
    ///   nonzero field element in decimal or in hexadecimal after `0x`.
    ///
    /// Errors: [`FecError::CodeText`] with the line and the fault, and the errors of [`NbCode::new`].
    pub fn parse(text: &str) -> Result<Self> {
        let mut header: Option<(u8, u32, usize, usize)> = None;
        let mut info: Option<Vec<usize>> = None;
        let mut checks: Vec<Vec<(usize, u16)>> = Vec::new();
        let mut last = 0;
        for (index, raw) in text.lines().enumerate() {
            last = index + 1;
            let fail = |fault| FecError::CodeText {
                line: index + 1,
                fault,
            };
            let number = |w: &str| parse_number(w).map_err(fail);
            let line = raw.split('#').next().unwrap_or("").trim();
            if line.is_empty() {
                continue;
            }
            let mut words = line.split_whitespace();
            let Some((_, _, _, k)) = header else {
                let mut num = || words.next().map(number);
                let q = num().ok_or(fail(TextFault::ShortHeader))??;
                let n = num().ok_or(fail(TextFault::ShortHeader))??;
                let k = num().ok_or(fail(TextFault::ShortHeader))??;
                if q < 4 || !q.is_power_of_two() || q > 1 << MAX_M {
                    return Param::Q.not_allowed(q);
                }
                let m = q.trailing_zeros() as u8;
                let poly = match num() {
                    Some(p) => u32::try_from(p?).map_err(|_| fail(TextFault::TooLarge))?,
                    None => default_poly(m).ok_or(fail(TextFault::NoFieldPolynomial))?,
                };
                if words.next().is_some() {
                    return Err(fail(TextFault::ExtraValues));
                }
                header = Some((m, poly, n as usize, k as usize));
                continue;
            };
            if line.starts_with("info") {
                if info.is_some() || !checks.is_empty() {
                    return Err(fail(TextFault::InfoOrder));
                }
                if words.next() != Some("info") {
                    return Err(fail(TextFault::BadInfo));
                }
                let list = words
                    .map(|w| number(w).map(|v| v as usize))
                    .collect::<Result<Vec<usize>>>()?;
                if list.len() != k {
                    return Err(fail(TextFault::InfoCount));
                }
                info = Some(list);
                continue;
            }
            let mut c = Vec::new();
            for w in words {
                let (s, h) = w.split_once(':').ok_or(fail(TextFault::BadCheckEntry))?;
                let s = number(s)? as usize;
                let h = u16::try_from(number(h)?).map_err(|_| fail(TextFault::TooLarge))?;
                c.push((s, h));
            }
            checks.push(c);
        }
        let (m, poly, n, k) = header.ok_or(FecError::CodeText {
            line: last,
            fault: TextFault::NoHeader,
        })?;
        if checks.is_empty() {
            return Err(FecError::CodeText {
                line: last,
                fault: TextFault::NoChecks,
            });
        }
        let info = info.unwrap_or_else(|| (0..k).collect());
        let refs: Vec<&[(usize, u16)]> = checks.iter().map(|c| c.as_slice()).collect();
        Self::new(m, poly, n, &refs, &info)
    }

    /// The code in the text form [`NbCode::parse`] reads, with the field polynomial and the info line always written.
    pub fn to_text(&self) -> String {
        let mut s = String::new();
        let _ = writeln!(
            s,
            "{} {} {} 0x{:X}",
            self.q(),
            self.n,
            self.k(),
            self.field.poly()
        );
        s.push_str("info");
        for p in &self.info {
            let _ = write!(s, " {p}");
        }
        s.push('\n');
        for c in 0..self.checks() {
            let mut first = true;
            for (sym, h) in self.check(c) {
                if !first {
                    s.push(' ');
                }
                first = false;
                let _ = write!(s, "{sym}:{h}");
            }
            s.push('\n');
        }
        s
    }

    /// The field.
    pub fn field(&self) -> &Gf {
        &self.field
    }

    /// `m`, the bits per symbol.
    pub fn m(&self) -> u8 {
        self.field.m()
    }

    /// `q = 2^m`, the number of symbol values.
    pub fn q(&self) -> usize {
        1 << self.field.m()
    }

    /// The code word length in symbols.
    pub fn n(&self) -> usize {
        self.n
    }

    /// The number of information symbols.
    pub fn k(&self) -> usize {
        self.info.len()
    }

    /// The number of parity checks.
    pub fn checks(&self) -> usize {
        self.check_start.len() - 1
    }

    /// The number of nonzero entries of the parity-check matrix.
    pub fn edges(&self) -> usize {
        self.sym.len()
    }

    /// The information positions, in the order [`NbCode::encode`] takes the information symbols.
    pub fn info_positions(&self) -> &[u32] {
        &self.info
    }

    /// The `(symbol, coefficient)` pairs of check `c`. An empty iterator when `c` is not below [`NbCode::checks`].
    pub fn check(&self, c: usize) -> impl Iterator<Item = (usize, u16)> + '_ {
        let (a, b) = if c < self.checks() {
            (
                self.check_start[c] as usize,
                self.check_start[c + 1] as usize,
            )
        } else {
            (0, 0)
        };
        self.sym[a..b]
            .iter()
            .zip(&self.coef[a..b])
            .map(|(&s, &h)| (s as usize, h))
    }

    /// The number of checks that `word` (`n` symbols) does not satisfy. Symbol bits above `m` are ignored.
    ///
    /// Errors: `InputLength` when `word` does not hold `n` symbols.
    pub fn unsatisfied(&self, word: &[u16]) -> Result<usize> {
        if word.len() != self.n {
            return Err(FecError::InputLength {
                expected: self.n,
                got: word.len(),
            });
        }
        Ok(count_unsatisfied(
            &self.field,
            &self.check_start,
            &self.sym,
            &self.coef,
            word,
        ))
    }

    /// Writes the code word of the `k` information symbols `info` to `out` (`n` symbols). It does not allocate.
    ///
    /// Errors: `InputLength` when `info` does not hold `k` symbols, `OutputTooShort` when `out` is shorter than `n`, and
    /// `OutOfRange` when a symbol is not below `q`.
    pub fn encode(&self, info: &[u16], out: &mut [u16]) -> Result<()> {
        let k = self.k();
        if info.len() != k {
            return Err(FecError::InputLength {
                expected: k,
                got: info.len(),
            });
        }
        if out.len() < self.n {
            return Err(FecError::OutputTooShort { need: self.n });
        }
        if info.iter().any(|&v| v as usize >= self.q()) {
            return Param::Symbol.range(
                0,
                self.q() as i64 - 1,
                info.iter()
                    .copied()
                    .find(|&v| v as usize >= self.q())
                    .unwrap_or(0),
            );
        }
        for (&p, &v) in self.info.iter().zip(info) {
            out[p as usize] = v;
        }
        for (j, &p) in self.parity.iter().enumerate() {
            let row = &self.enc[j * k..(j + 1) * k];
            out[p as usize] = row
                .iter()
                .zip(info)
                .fold(0, |acc, (&a, &v)| acc ^ self.field.mul(a, v));
        }
        Ok(())
    }
}

/// When a decode stops before its iteration limit.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum NbStop {
    /// The hard decision satisfies every check.
    #[default]
    Syndrome,
    /// The extrinsic distributions are sure of their values: the sum over the `n` symbols of the largest extrinsic
    /// probability exceeds `n − slack`. A code word that the checks alone fix (the channel rows left out) passes; a hard
    /// decision that satisfies the checks by chance does not.
    Confidence {
        /// How far the sum may stay below `n`, above 0.
        slack: f32,
    },
}

/// A flooding sum-product decoder for an [`NbCode`], with `f32` probability messages. Check nodes work in the Walsh-Hadamard
/// domain. All buffers are sized at construction, so a decode does not allocate.
///
/// Method: M. Davey and D. MacKay, "Low-density parity check codes over GF(q)", IEEE Comm. Letters 2, 1998. L. Barnault and
/// D. Declercq, "Fast decoding algorithm for LDPC over GF(2^q)", ITW 2003.
#[derive(Clone, Debug)]
pub struct NbDecoder {
    code: NbCode,
    q: usize,
    /// `log h` of each edge, in check order.
    log_coef: Vec<u16>,
    /// The edges of each symbol, grouped by symbol, with `var_start` marking each group.
    var_edges: Vec<u32>,
    var_start: Vec<u32>,
    /// The normalized intrinsic probabilities, `n × q`.
    intrinsic: Vec<f32>,
    /// The posterior probabilities, `n × q`.
    post: Vec<f32>,
    /// The extrinsic probabilities (the check messages alone), `n × q`.
    ext: Vec<f32>,
    stop: NbStop,
    /// Symbol-to-check and check-to-symbol messages, `edges × q`, in check order.
    v2c: Vec<f32>,
    c2v: Vec<f32>,
    /// Scratch: transforms of one check's inputs, prefix products, one running product.
    tw: Vec<f32>,
    pre: Vec<f32>,
    run: Vec<f32>,
    tmp: Vec<f32>,
    hard: Vec<u16>,
}

/// The smallest probability a check-to-symbol message keeps, so that no message rules out a value for good.
const FLOOR: f32 = 1e-9;

impl NbDecoder {
    /// A decoder for `code`.
    pub fn new(code: &NbCode) -> Self {
        let q = code.q();
        let n = code.n();
        let e = code.edges();
        let mut deg = vec![0u32; n];
        for &s in &code.sym {
            deg[s as usize] += 1;
        }
        let mut var_start = vec![0u32; n + 1];
        for v in 0..n {
            var_start[v + 1] = var_start[v] + deg[v];
        }
        let mut fill = var_start.clone();
        let mut var_edges = vec![0u32; e];
        for (edge, &s) in code.sym.iter().enumerate() {
            var_edges[fill[s as usize] as usize] = edge as u32;
            fill[s as usize] += 1;
        }
        let max_check = (0..code.checks())
            .map(|c| (code.check_start[c + 1] - code.check_start[c]) as usize)
            .max()
            .unwrap_or(0);
        let max_var = deg.iter().copied().max().unwrap_or(0) as usize;
        let width = max_check.max(max_var) + 1;
        let log_coef = code
            .coef
            .iter()
            .map(|&h| code.field.log(h).unwrap_or(0) as u16)
            .collect();
        NbDecoder {
            code: code.clone(),
            q,
            log_coef,
            var_edges,
            var_start,
            intrinsic: vec![0.0; n * q],
            post: vec![0.0; n * q],
            ext: vec![1.0 / q as f32; n * q],
            stop: NbStop::Syndrome,
            v2c: vec![0.0; e * q],
            c2v: vec![0.0; e * q],
            tw: vec![0.0; width * q],
            pre: vec![0.0; width * q],
            run: vec![0.0; q],
            tmp: vec![0.0; q],
            hard: vec![0; n],
        }
    }

    /// The code this decoder decodes.
    pub fn code(&self) -> &NbCode {
        &self.code
    }

    /// Sets when a decode stops ([`NbStop::Syndrome`] at construction).
    pub fn set_stop(&mut self, stop: NbStop) {
        self.stop = stop;
    }

    /// Whether the extrinsic distributions pass the rule of [`NbStop::Confidence`] with `slack`.
    fn confident(&self, slack: f32) -> bool {
        let sum: f32 = self
            .ext
            .chunks_exact(self.q)
            .map(|row| row.iter().copied().fold(0.0, f32::max))
            .sum();
        sum > self.code.n as f32 - slack
    }

    /// Decodes one code word.
    ///
    /// `probs` holds `n × q` values, row `i` for code symbol `i`: entry `a` is the likelihood that the symbol is `a`. Each row
    /// can have any positive scale, and the decoder normalizes it. A punctured symbol (not sent) is a row of equal values. The
    /// decoder runs at most `max_iter` flooding iterations. It stops after the first iteration that passes the stop rule
    /// ([`NbDecoder::set_stop`]): by default, the hard decision (the most probable value of each symbol) satisfies every
    /// check. It writes the hard decision to `hard` (`n` symbols). The posterior and extrinsic probabilities stay available
    /// through [`NbDecoder::posterior`] and [`NbDecoder::extrinsic`].
    ///
    /// The result: `corrected` is the number of symbols whose hard decision differs from the most probable value of their row
    /// in `probs` (rows without a single most probable value are not counted). `metric` is the number of unsatisfied checks
    /// at the end (0 on success with [`NbStop::Syndrome`]). `iterations` is the number of iterations run.
    ///
    /// Errors: `InputLength` when `probs` does not hold `n × q` values, `OutputTooShort` when `hard` is shorter than `n`,
    /// `NotFinite` for a NaN or infinite value, `NotAllowed` for a negative value, `Degenerate` for a row of zeros. If no iteration satisfies
    /// the stop rule, the result is `Err(Uncorrectable)`, and `hard` and the posteriors hold the last iteration.
    pub fn decode(&mut self, probs: &[f32], max_iter: u32, hard: &mut [u16]) -> Result<Decoded> {
        let (n, q) = (self.code.n, self.q);
        if probs.len() != n * q {
            return Err(FecError::InputLength {
                expected: n * q,
                got: probs.len(),
            });
        }
        if hard.len() < n {
            return Err(FecError::OutputTooShort { need: n });
        }
        if let Some(at) = probs.iter().position(|p| !p.is_finite()) {
            return Err(FecError::NotFinite { at });
        }
        if probs.iter().any(|&p| p < 0.0) {
            return Param::Probabilities.not_allowed(-1);
        }
        for (dst, src) in self
            .intrinsic
            .chunks_exact_mut(q)
            .zip(probs.chunks_exact(q))
        {
            let sum: f32 = src.iter().sum();
            if !(sum > 0.0 && sum.is_finite()) {
                return Param::Probabilities.degenerate();
            }
            let inv = 1.0 / sum;
            for (d, &s) in dst.iter_mut().zip(src) {
                *d = s * inv;
            }
        }
        self.post.copy_from_slice(&self.intrinsic);
        for (edge, &s) in self.code.sym.iter().enumerate() {
            let s = s as usize;
            self.v2c[edge * q..(edge + 1) * q].copy_from_slice(&self.intrinsic[s * q..(s + 1) * q]);
        }
        let mut used = 0;
        let mut bad = usize::MAX;
        let mut done = false;
        for row in self.ext.chunks_exact_mut(q) {
            row.fill(1.0 / q as f32);
        }
        while used < max_iter {
            self.check_nodes();
            self.symbol_nodes();
            used += 1;
            for (h, row) in self.hard.iter_mut().zip(self.post.chunks_exact(q)) {
                *h = argmax(row) as u16;
            }
            bad = count_unsatisfied(
                &self.code.field,
                &self.code.check_start,
                &self.code.sym,
                &self.code.coef,
                &self.hard,
            );
            done = match self.stop {
                NbStop::Syndrome => bad == 0,
                NbStop::Confidence { slack } => self.confident(slack),
            };
            if done {
                break;
            }
        }
        if used == 0 {
            for (h, row) in self.hard.iter_mut().zip(self.post.chunks_exact(q)) {
                *h = argmax(row) as u16;
            }
            bad = count_unsatisfied(
                &self.code.field,
                &self.code.check_start,
                &self.code.sym,
                &self.code.coef,
                &self.hard,
            );
            done = bad == 0 && self.stop == NbStop::Syndrome;
        }
        hard[..n].copy_from_slice(&self.hard);
        let mut corrected = 0;
        for (row, &h) in probs.chunks_exact(q).zip(&self.hard) {
            if let Some(best) = unique_argmax(row) {
                corrected += (best != h as usize) as u32;
            }
        }
        if done {
            Ok(Decoded {
                corrected,
                metric: bad as i32,
                iterations: used,
            })
        } else {
            Err(FecError::Uncorrectable)
        }
    }

    /// The posterior probabilities of the last decode, `n × q`, row `i` for code symbol `i`, each row summing to 1.
    pub fn posterior(&self) -> &[f32] {
        &self.post
    }

    /// The extrinsic probabilities of the last decode, `n × q`, row `i` for code symbol `i`, each row summing to 1: what the
    /// checks say about a symbol without its own row of `probs`.
    pub fn extrinsic(&self) -> &[f32] {
        &self.ext
    }

    /// The posterior probabilities of code symbol `i` after the last decode. An empty slice when `i` is not below `n`.
    pub fn posterior_of(&self, i: usize) -> &[f32] {
        self.post.get(i * self.q..(i + 1) * self.q).unwrap_or(&[])
    }

    /// Check-to-symbol messages. For check `Σ h_j x_j = 0`, the value `z_j = h_j x_j` is the sum of the other `z`. The
    /// distribution of a sum in GF(2^m) is the XOR convolution of the distributions, which the Walsh-Hadamard transform turns
    /// into a product.
    fn check_nodes(&mut self) {
        let q = self.q;
        let field = &self.code.field;
        for c in 0..self.code.checks() {
            let a = self.code.check_start[c] as usize;
            let b = self.code.check_start[c + 1] as usize;
            let d = b - a;
            for (i, edge) in (a..b).enumerate() {
                let t = &mut self.tw[i * q..(i + 1) * q];
                let src = &self.v2c[edge * q..(edge + 1) * q];
                let lh = self.log_coef[edge] as usize;
                t[0] = src[0];
                for x in 1..q {
                    t[field.mul_exp(x as u16, lh) as usize] = src[x];
                }
                wht(t);
            }
            self.pre[..q].fill(1.0);
            for i in 0..d {
                let (done, rest) = self.pre.split_at_mut((i + 1) * q);
                let prev = &done[i * q..];
                let t = &self.tw[i * q..(i + 1) * q];
                for ((o, &p), &v) in rest[..q].iter_mut().zip(prev).zip(t) {
                    *o = p * v;
                }
            }
            self.run.fill(1.0);
            for (i, edge) in (a..b).enumerate().rev() {
                for ((o, &p), &r) in self
                    .tmp
                    .iter_mut()
                    .zip(&self.pre[i * q..(i + 1) * q])
                    .zip(&self.run)
                {
                    *o = p * r;
                }
                wht(&mut self.tmp);
                let dst = &mut self.c2v[edge * q..(edge + 1) * q];
                let lh = self.log_coef[edge] as usize;
                dst[0] = self.tmp[0];
                for x in 1..q {
                    dst[x] = self.tmp[field.mul_exp(x as u16, lh) as usize];
                }
                for v in dst.iter_mut() {
                    *v = v.max(0.0);
                }
                normalize(dst);
                for v in dst.iter_mut() {
                    *v = v.max(FLOOR);
                }
                let t = &self.tw[i * q..(i + 1) * q];
                for (r, &v) in self.run.iter_mut().zip(t) {
                    *r *= v;
                }
            }
        }
    }

    /// Symbol-to-check messages and posteriors: the intrinsic row times the incoming messages, leaving out the edge the message
    /// goes to.
    fn symbol_nodes(&mut self) {
        let q = self.q;
        for v in 0..self.code.n {
            let a = self.var_start[v] as usize;
            let b = self.var_start[v + 1] as usize;
            let d = b - a;
            self.pre[..q].copy_from_slice(&self.intrinsic[v * q..(v + 1) * q]);
            for i in 0..d {
                let edge = self.var_edges[a + i] as usize;
                let (done, rest) = self.pre.split_at_mut((i + 1) * q);
                let prev = &done[i * q..];
                let msg = &self.c2v[edge * q..(edge + 1) * q];
                let out = &mut rest[..q];
                for ((o, &p), &m) in out.iter_mut().zip(prev).zip(msg) {
                    *o = p * m;
                }
                normalize(out);
            }
            self.post[v * q..(v + 1) * q].copy_from_slice(&self.pre[d * q..(d + 1) * q]);
            self.run.fill(1.0);
            for i in (0..d).rev() {
                let edge = self.var_edges[a + i] as usize;
                let dst = &mut self.v2c[edge * q..(edge + 1) * q];
                for ((o, &p), &r) in dst
                    .iter_mut()
                    .zip(&self.pre[i * q..(i + 1) * q])
                    .zip(&self.run)
                {
                    *o = p * r;
                }
                normalize(dst);
                let msg = &self.c2v[edge * q..(edge + 1) * q];
                for (r, &m) in self.run.iter_mut().zip(msg) {
                    *r *= m;
                }
                normalize(&mut self.run);
            }
            let ext = &mut self.ext[v * q..(v + 1) * q];
            ext.copy_from_slice(&self.run);
            normalize(ext);
        }
    }
}

fn field_for(m: u8, poly: u32) -> Result<Gf> {
    if !(2..=MAX_M).contains(&m) {
        return Param::M.range(2, MAX_M as i64, m);
    }
    Gf::new(m, poly)
}

fn parse_number(w: &str) -> std::result::Result<u64, TextFault> {
    let r = match w.strip_prefix("0x").or_else(|| w.strip_prefix("0X")) {
        Some(h) => u64::from_str_radix(h, 16),
        None => w.parse::<u64>(),
    };
    r.map_err(|_| TextFault::NotANumber)
}

fn count_unsatisfied(field: &Gf, start: &[u32], sym: &[u32], coef: &[u16], word: &[u16]) -> usize {
    (0..start.len() - 1)
        .filter(|&c| {
            let (a, b) = (start[c] as usize, start[c + 1] as usize);
            sym[a..b]
                .iter()
                .zip(&coef[a..b])
                .fold(0u16, |acc, (&s, &h)| acc ^ field.mul(h, word[s as usize]))
                != 0
        })
        .count()
}

/// Gauss-Jordan elimination over GF(q) on the parity columns. Row `j` of the result computes parity symbol `j` from the
/// information symbols.
fn build_encoder(
    field: &Gf,
    n: usize,
    start: &[u32],
    sym: &[u32],
    coef: &[u16],
    info: &[u32],
    parity: &[u32],
) -> Result<Vec<u16>> {
    let rows = start.len() - 1;
    let np = parity.len();
    let k = info.len();
    // Columns: the parity positions first, then the information positions.
    let mut col_of = vec![0usize; n];
    for (j, &p) in parity.iter().enumerate() {
        col_of[p as usize] = j;
    }
    for (i, &p) in info.iter().enumerate() {
        col_of[p as usize] = np + i;
    }
    let mut h = vec![0u16; rows * n];
    for r in 0..rows {
        for e in start[r] as usize..start[r + 1] as usize {
            h[r * n + col_of[sym[e] as usize]] = coef[e];
        }
    }
    for (rank, col) in (0..np).enumerate() {
        let Some(pivot) = (rank..rows).find(|&r| h[r * n + col] != 0) else {
            return Param::Checks.degenerate();
        };
        if pivot != rank {
            for x in 0..n {
                h.swap(pivot * n + x, rank * n + x);
            }
        }
        let inv = field.inv(h[rank * n + col]).unwrap_or(1);
        for x in 0..n {
            h[rank * n + x] = field.mul(h[rank * n + x], inv);
        }
        for r in 0..rows {
            let f = h[r * n + col];
            if r != rank && f != 0 {
                for x in 0..n {
                    let v = field.mul(f, h[rank * n + x]);
                    h[r * n + x] ^= v;
                }
            }
        }
    }
    if (np..rows).any(|r| h[r * n + np..(r + 1) * n].iter().any(|&v| v != 0)) {
        return Param::Checks.degenerate();
    }
    // Row j now reads p_j + Σ h[j][np + i] u_i = 0. In characteristic 2, p_j = Σ h[j][np + i] u_i.
    let mut enc = vec![0u16; np * k];
    for j in 0..np {
        enc[j * k..(j + 1) * k].copy_from_slice(&h[j * n + np..(j + 1) * n]);
    }
    Ok(enc)
}

/// The fast Walsh-Hadamard transform in place, without scaling. Applying it twice multiplies by the length.
fn wht(x: &mut [f32]) {
    let n = x.len();
    let mut h = 1;
    while h < n {
        for i in (0..n).step_by(2 * h) {
            for j in i..i + h {
                let (a, b) = (x[j], x[j + h]);
                x[j] = a + b;
                x[j + h] = a - b;
            }
        }
        h *= 2;
    }
}

/// Scales `x` to sum 1. If the sum is not positive and finite, `x` becomes uniform.
fn normalize(x: &mut [f32]) {
    let s: f32 = x.iter().sum();
    if s > 0.0 && s.is_finite() {
        let inv = 1.0 / s;
        for v in x.iter_mut() {
            *v *= inv;
        }
    } else {
        let u = 1.0 / x.len() as f32;
        x.fill(u);
    }
}

fn argmax(row: &[f32]) -> usize {
    let mut best = 0;
    for (i, &v) in row.iter().enumerate() {
        if v > row[best] {
            best = i;
        }
    }
    best
}

fn unique_argmax(row: &[f32]) -> Option<usize> {
    let best = argmax(row);
    let top = row[best];
    (row.iter().filter(|&&v| v == top).count() == 1).then_some(best)
}
