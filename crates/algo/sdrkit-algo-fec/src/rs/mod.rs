//! Reed-Solomon codes over GF(2^m), `m` from 2 to 16: any field polynomial, first consecutive root `fcr`, root spacing `prim`,
//! shortened lengths, either symbol order, the CCSDS dual basis, interleaving, and errors-and-erasures decoding.
//!
//! # Conventions
//!
//! A code word of length `n` holds `k` data symbols and `n − k` parity symbols. Its polynomial is `c(x) = Σ c_p x^p`, and the
//! generator is `g(x) = Π_{j=0}^{n−k−1} (x − α^{prim (fcr + j)})`. [`SymbolOrder::HighFirst`] (DVB, ATSC, CCSDS, almost every
//! standard) sends the highest power first, so the data come first and the parity last; [`SymbolOrder::LowFirst`] sends `c_0`
//! first, so the parity comes first (RS41, RS92). A shortened code is the full-length code with leading zero data symbols that
//! are not sent; `n` is the length actually sent.
//!
//! # Decoding
//!
//! Syndromes, then Berlekamp-Massey with the erasure locator as its start (so `2 × errors + erasures ≤ n − k` are corrected),
//! a Chien search over the sent positions only, and Forney's formula. Before anything is written the decoder checks that the
//! locator has exactly as many roots as its degree, all inside the shortened word, and that the corrected word has zero
//! syndromes; otherwise it returns [`FecError::Uncorrectable`] and leaves the word untouched.
//!
//! Syndromes and the Chien search run on a backend of [`BACKENDS`]: NEON or SSSE3 for `m ≤ 8` (16 symbols per instruction
//! through nibble multiply tables), and the plain scalar kernels for every code.
//!
//! ```
//! use sdrkit_algo_fec::rs::{codes, Rs};
//! let mut rs = Rs::new(codes::DVB_204_188).unwrap();
//! let mut word = [0u8; 204];
//! for (i, b) in word[..188].iter_mut().enumerate() {
//!     *b = (i * 7 + 1) as u8;
//! }
//! rs.encode_word(&mut word).unwrap();
//! let sent = word;
//! for i in [3, 50, 100, 150, 160, 170, 190, 203] {
//!     word[i] ^= 0x5A;
//! }
//! let r = rs.decode(&mut word, &[]).unwrap();
//! assert_eq!(word, sent);
//! assert_eq!(r.corrected, 8);
//! ```

pub mod codes;
mod simd;

use crate::error::{Decoded, FecError, Param, Result};
use crate::gf::Gf;
use sdrkit_core::backend::{Backend, Registry};

/// The order in which the symbols of a code word are sent.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SymbolOrder {
    /// The coefficient of the highest power first: data, then parity.
    HighFirst,
    /// The coefficient of `x^0` first: parity, then data.
    LowFirst,
}

/// How each 8-bit symbol is represented on air.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Basis {
    /// The polynomial basis of the field.
    Conventional,
    /// The CCSDS dual basis (CCSDS 131.0-B-5 clause 4.3.9 and annex F): sent symbols are converted with the matrix `T⁻¹` before
    /// decoding and back with `T` after. Only for the CCSDS field (`m = 8`, `0x187`).
    Dual,
}

/// The parameters of a Reed-Solomon code.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct RsCode {
    /// A name for the code.
    pub name: &'static str,
    /// Bits per symbol (2 to 16).
    pub m: u8,
    /// Field polynomial (degree `m`, primitive).
    pub poly: u32,
    /// Code word length sent (at most `2^m − 1`; less for a shortened code).
    pub n: u16,
    /// Data symbols per code word.
    pub k: u16,
    /// First consecutive root, as a power of `α^prim`.
    pub fcr: u16,
    /// Root spacing: the roots are powers of `α^prim` (coprime with `2^m − 1`).
    pub prim: u16,
    /// Symbol order on air.
    pub order: SymbolOrder,
    /// Symbol representation on air.
    pub basis: Basis,
}

impl RsCode {
    /// Parity symbols per code word, `n − k`.
    pub const fn nroots(&self) -> usize {
        (self.n - self.k) as usize
    }

    /// The same code shortened to `n` symbols (`k` shrinks by as much).
    pub const fn shortened(self, n: u16) -> Self {
        RsCode {
            k: self.k - (self.n - n),
            n,
            ..self
        }
    }
}

/// Result of decoding an interleaved frame.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FrameDecoded {
    /// Symbols corrected over the code words that decoded.
    pub corrected: u32,
    /// Code words that could not be decoded (left as received).
    pub failed: u32,
    /// Bit mask of the failed code words (bit `i` for code word `i`, up to 32).
    pub failed_mask: u32,
}

/// The CCSDS dual-basis matrix `T` (CCSDS 131.0-B-5 clause 4.3.9.3): row `r` (1 to 8) is the dual-basis image of `α^(8−r)`,
/// written `z0 .. z7`.
pub const CCSDS_T: [u8; 8] = [
    0b1000_1101,
    0b1110_1111,
    0b1110_1100,
    0b1000_0110,
    0b1111_1010,
    0b1001_1001,
    0b1010_1111,
    0b0111_1011,
];
/// The CCSDS matrix `T⁻¹` (clause 4.3.9.3): row `r` is the conventional image (`u7 .. u0`) of dual-basis bit `z(r−1)`.
pub const CCSDS_T_INV: [u8; 8] = [
    0b1100_0101,
    0b0100_0010,
    0b0010_1110,
    0b1111_1101,
    0b1111_0000,
    0b0111_1001,
    0b1010_1100,
    0b1100_1100,
];

/// Applies an 8 x 8 bit matrix given by rows (row `r` is added when bit `7 − r` of `x` is set).
const fn mat8(rows: &[u8; 8], x: u8) -> u8 {
    let mut y = 0u8;
    let mut r = 0;
    while r < 8 {
        if x >> (7 - r) & 1 != 0 {
            y ^= rows[r];
        }
        r += 1;
    }
    y
}

const fn table8(rows: &[u8; 8]) -> [u8; 256] {
    let mut t = [0u8; 256];
    let mut i = 0;
    while i < 256 {
        t[i] = mat8(rows, i as u8);
        i += 1;
    }
    t
}

/// Conventional (`u7 .. u0`, `u_j` the coefficient of `α^j`, most significant bit first) to the CCSDS dual basis (`z0 .. z7`,
/// `z0` most significant): `TO_DUAL[u] = [u7 .. u0] × T`.
pub static TO_DUAL: [u8; 256] = table8(&CCSDS_T);
/// CCSDS dual basis to conventional: `FROM_DUAL[z] = [z0 .. z7] × T⁻¹`.
pub static FROM_DUAL: [u8; 256] = table8(&CCSDS_T_INV);

/// A backend of the Reed-Solomon family: the syndromes and the Chien search of the decoder. Every backend returns the same
/// values as the plain one.
///
/// A minimal backend that runs the plain kernels:
///
/// ```
/// use sdrkit_core::backend::Backend;
/// use sdrkit_algo_fec::gf::Gf;
/// use sdrkit_algo_fec::rs::{check_backend, RsBackend, BACKENDS};
///
/// struct Delegate;
/// impl Backend for Delegate {
///     fn name(&self) -> &'static str { "delegate" }
/// }
/// impl RsBackend for Delegate {
///     fn prepare(&self, _gf: &Gf, _roots: &[u16]) -> Option<Vec<u8>> {
///         Some(Vec::new())
///     }
///     fn syndromes(&self, tables: &[u8], gf: &Gf, word: &[u16], roots: &[u16], synd: &mut [u16]) {
///         BACKENDS.plain().syndromes(tables, gf, word, roots, synd)
///     }
///     fn chien(&self, tables: &[u8], gf: &Gf, lambda: &[u16], prim: usize, n: usize, locs: &mut [u16]) -> usize {
///         BACKENDS.plain().chien(tables, gf, lambda, prim, n, locs)
///     }
/// }
///
/// check_backend(&Delegate);
/// ```
pub trait RsBackend: Backend {
    /// The backend's tables for a code over `gf` with the generator roots `roots`. `None` when the backend cannot do the code.
    fn prepare(&self, gf: &Gf, roots: &[u16]) -> Option<Vec<u8>>;

    /// `synd[j] = Σ_p word[p] roots[j]^p`, with `word` the coefficients by power. `tables` comes from
    /// [`RsBackend::prepare`] for `gf` and `roots`.
    fn syndromes(&self, tables: &[u8], gf: &Gf, word: &[u16], roots: &[u16], synd: &mut [u16]);

    /// The Chien search: the powers `p < n` with `Λ(γ^−p) = 0`, `Λ` the coefficients `lambda` (lowest power first) and
    /// `γ = α^prim`, written to `locs` in increasing order. Returns how many: `locs.len() + 1` means more than `locs` holds.
    fn chien(
        &self,
        tables: &[u8],
        gf: &Gf,
        lambda: &[u16],
        prim: usize,
        n: usize,
        locs: &mut [u16],
    ) -> usize;
}

impl std::fmt::Debug for dyn RsBackend {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.name())
    }
}

/// The Reed-Solomon backends of this platform, the default first and the plain backend last.
pub static BACKENDS: Registry<dyn RsBackend> = Registry::new(TABLE);

#[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
const TABLE: &[&dyn RsBackend] = &[&simd::Vector, &Plain];
#[cfg(not(any(target_arch = "aarch64", target_arch = "x86_64")))]
const TABLE: &[&dyn RsBackend] = &[&Plain];

/// Makes sure that `backend` agrees with the plain backend: the syndromes of 50 random words and the Chien search of 50 random
/// locators (degree 1 to 16, and the true locators of 1 to 16 errors) on RS(255,223), RS(204,188) and RS(63,12) over GF(64).
/// The values must be equal.
///
/// # Panics
/// When a value differs, or when the backend does none of the codes.
pub fn check_backend(backend: &dyn RsBackend) {
    let mut seed = 0x5851_f42d_4c95_7f2du64;
    let mut next = move || {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        seed
    };
    let mut done = 0;
    for code in [codes::CCSDS_255_223, codes::DVB_204_188, codes::JT65_63_12] {
        let rs = Rs::new(code).expect("a named code");
        let Some(tables) = backend.prepare(&rs.gf, &rs.roots) else {
            continue;
        };
        done += 1;
        let plain = Plain
            .prepare(&rs.gf, &rs.roots)
            .expect("the plain backend does every code");
        let order = rs.gf.order() as u64 + 1;
        let n = code.n as usize;
        for trial in 0..50 {
            let word: Vec<u16> = (0..n).map(|_| (next() % order) as u16).collect();
            let mut a = vec![0u16; rs.roots.len()];
            let mut b = vec![1u16; rs.roots.len()];
            Plain.syndromes(&plain, &rs.gf, &word, &rs.roots, &mut a);
            backend.syndromes(&tables, &rs.gf, &word, &rs.roots, &mut b);
            assert_eq!(a, b, "{} syndromes of {}", backend.name(), code.name);
            // A locator with roots at chosen positions, or random coefficients.
            let deg = 1 + trial % 16;
            let lambda: Vec<u16> = if trial % 2 == 0 {
                let mut l = vec![1u16];
                for _ in 0..deg {
                    let p = (next() % n as u64) as usize;
                    let x = rs.gf.exp(rs.prim * p % rs.gf.order());
                    let mut m = vec![0u16; l.len() + 1];
                    for (i, &c) in l.iter().enumerate() {
                        m[i] ^= c;
                        m[i + 1] ^= rs.gf.mul(c, x);
                    }
                    l = m;
                }
                l
            } else {
                (0..=deg).map(|_| (next() % order) as u16).collect()
            };
            let mut la = vec![0u16; 16];
            let mut lb = vec![0u16; 16];
            let fa = Plain.chien(&plain, &rs.gf, &lambda, rs.prim, n, &mut la);
            let fb = backend.chien(&tables, &rs.gf, &lambda, rs.prim, n, &mut lb);
            assert_eq!(
                (fa, la),
                (fb, lb),
                "{} Chien search of {}",
                backend.name(),
                code.name
            );
        }
    }
    assert!(done > 0, "{} does none of the codes", backend.name());
}

/// The plain backend: scalar Horner syndromes and a scalar Chien search, for every code.
struct Plain;

impl Backend for Plain {
    fn name(&self) -> &'static str {
        "plain"
    }
}

impl RsBackend for Plain {
    fn prepare(&self, _gf: &Gf, _roots: &[u16]) -> Option<Vec<u8>> {
        Some(Vec::new())
    }

    fn syndromes(&self, _tables: &[u8], gf: &Gf, word: &[u16], roots: &[u16], synd: &mut [u16]) {
        syndromes_scalar(gf, word, roots, synd);
    }

    fn chien(
        &self,
        _tables: &[u8],
        gf: &Gf,
        lambda: &[u16],
        prim: usize,
        n: usize,
        locs: &mut [u16],
    ) -> usize {
        chien_scalar(gf, lambda, prim, n, locs)
    }
}

/// A Reed-Solomon encoder and decoder for one [`RsCode`], with all scratch sized at construction.
#[derive(Clone, Debug)]
pub struct Rs {
    code: RsCode,
    gf: Gf,
    /// Generator coefficients `g_0 .. g_{nroots}` (monic, `g_nroots = 1`).
    gen: Vec<u16>,
    /// `γ^(fcr + j)` for each root, `γ = α^prim`.
    roots: Vec<u16>,
    /// Exponent of `γ` as a power of `α`.
    prim: usize,
    /// The backend of the syndromes and the Chien search, and its tables for this code.
    backend: &'static dyn RsBackend,
    tables: Vec<u8>,
    // Scratch.
    word: Vec<u16>,
    synd: Vec<u16>,
    lambda: Vec<u16>,
    b: Vec<u16>,
    t: Vec<u16>,
    omega: Vec<u16>,
    locs: Vec<u16>,
    vals: Vec<u16>,
}

impl Rs {
    /// A codec for `code`. An error when the parameters are inconsistent (field not primitive, `n` above `2^m − 1`, `k` not
    /// below `n`, `prim` not coprime with `2^m − 1`, dual basis on a field other than the CCSDS one).
    pub fn new(code: RsCode) -> Result<Self> {
        let gf = Gf::new(code.m, code.poly)?;
        let nn = gf.order();
        Param::N.check(code.n, 2, nn as i64)?;
        Param::K.check(code.k, 1, code.n as i64 - 1)?;
        if code.prim == 0 || gcd(code.prim as usize, nn) != 1 {
            return Param::Prim.not_allowed(code.prim);
        }
        if code.basis == Basis::Dual && (code.m != 8 || code.poly != 0x187) {
            return Param::Basis.conflict(Param::Polynomial);
        }
        let nroots = code.nroots();
        let prim = code.prim as usize;
        let roots: Vec<u16> = (0..nroots)
            .map(|j| gf.exp(prim * ((code.fcr as usize + j) % nn)))
            .collect();
        // g(x) = Π (x + root_j), lowest power first.
        let mut gen = vec![0u16; nroots + 1];
        gen[0] = 1;
        for (j, &r) in roots.iter().enumerate() {
            // Multiply by (x + r): new[i] = old[i - 1] + r * old[i].
            for i in (0..=j + 1).rev() {
                let lower = if i > 0 { gen[i - 1] } else { 0 };
                gen[i] = lower ^ gf.mul(gen[i], r);
            }
        }
        let (backend, tables) = BACKENDS
            .build(|b| b.prepare(&gf, &roots).map(|t| (b, t)))
            .unwrap_or((BACKENDS.plain(), Vec::new()));
        let n = code.n as usize;
        Ok(Rs {
            code,
            gen,
            roots,
            prim,
            backend,
            tables,
            word: vec![0; n],
            synd: vec![0; nroots],
            lambda: vec![0; nroots + 1],
            b: vec![0; nroots + 1],
            t: vec![0; nroots + 1],
            omega: vec![0; nroots + 1],
            locs: vec![0; nroots],
            vals: vec![0; nroots],
            gf,
        })
    }

    /// Runs the syndromes and the Chien search on `backend`. Every backend produces the same results. An error when the backend
    /// is not available or cannot do the code.
    pub fn set_backend(&mut self, backend: &'static dyn RsBackend) -> Result<()> {
        if !backend.available() {
            return Param::Kernel.unsupported();
        }
        let Some(tables) = backend.prepare(&self.gf, &self.roots) else {
            return Param::Kernel.conflict(Param::M);
        };
        self.backend = backend;
        self.tables = tables;
        Ok(())
    }

    /// The name of the backend in use.
    pub fn backend(&self) -> &'static str {
        self.backend.name()
    }

    /// The code.
    pub fn code(&self) -> &RsCode {
        &self.code
    }

    /// The field.
    pub fn field(&self) -> &Gf {
        &self.gf
    }

    /// The generator polynomial's coefficients, `g_0` (the constant) first, `g_{n−k} = 1` last.
    pub fn generator(&self) -> &[u16] {
        &self.gen
    }

    #[inline]
    fn conv_of(&self, s: u16) -> u16 {
        match self.code.basis {
            Basis::Conventional => s,
            Basis::Dual => FROM_DUAL[s as u8 as usize] as u16,
        }
    }

    #[inline]
    fn air_of(&self, s: u16) -> u16 {
        match self.code.basis {
            Basis::Conventional => s,
            Basis::Dual => TO_DUAL[s as u8 as usize] as u16,
        }
    }

    /// The power of the coefficient at word index `w`.
    #[inline]
    fn power(&self, w: usize) -> usize {
        match self.code.order {
            SymbolOrder::HighFirst => self.code.n as usize - 1 - w,
            SymbolOrder::LowFirst => w,
        }
    }

    /// The parity of `k` data symbols into `nroots` parity symbols, both in word order and in the code's basis (symbols as `u16`).
    fn parity_of(&self, data: impl Iterator<Item = u16>, parity: &mut [u16]) {
        let nr = self.code.nroots();
        // LFSR division by g(x); `reg[i]` is the coefficient of x^i of the running remainder.
        let reg = parity;
        reg.fill(0);
        for d in data {
            let fb = self.conv_of(d) ^ reg[nr - 1];
            for i in (1..nr).rev() {
                reg[i] = reg[i - 1] ^ self.gf.mul(fb, self.gen[i]);
            }
            reg[0] = self.gf.mul(fb, self.gen[0]);
        }
    }

    /// Encodes `data` (`k` symbols, word order) into `parity` (`n − k` symbols, word order). Symbols are bytes, so `m ≤ 8`.
    pub fn encode(&self, data: &[u8], parity: &mut [u8]) -> Result<()> {
        let (k, nr) = (self.code.k as usize, self.code.nroots());
        if self.code.m > 8 {
            return Param::M.range(2, 8, self.code.m);
        }
        if data.len() != k {
            return Err(FecError::InputLength {
                expected: k,
                got: data.len(),
            });
        }
        if parity.len() != nr {
            return Err(FecError::OutputTooShort { need: nr });
        }
        let mut reg = [0u16; 256];
        self.encode_reg(data.iter().map(|&d| d as u16), &mut reg[..nr]);
        for (i, p) in parity.iter_mut().enumerate() {
            *p = reg[i] as u8;
        }
        Ok(())
    }

    /// Encodes with `u16` symbols (any `m`).
    pub fn encode16(&self, data: &[u16], parity: &mut [u16]) -> Result<()> {
        let (k, nr) = (self.code.k as usize, self.code.nroots());
        if data.len() != k {
            return Err(FecError::InputLength {
                expected: k,
                got: data.len(),
            });
        }
        if parity.len() != nr {
            return Err(FecError::OutputTooShort { need: nr });
        }
        self.encode_reg(data.iter().copied(), parity);
        Ok(())
    }

    /// Parity in word order and the code's basis.
    fn encode_reg(&self, data: impl DoubleEndedIterator<Item = u16>, out: &mut [u16]) {
        match self.code.order {
            // Data from the highest power down.
            SymbolOrder::HighFirst => self.parity_of(data, out),
            // Word order is lowest power first, so the highest data power is the last data symbol.
            SymbolOrder::LowFirst => self.parity_of(data.rev(), out),
        }
        // out[i] is the coefficient of x^i; HighFirst sends x^(nr - 1) first.
        if self.code.order == SymbolOrder::HighFirst {
            out.reverse();
        }
        for v in out.iter_mut() {
            *v = self.air_of(*v);
        }
    }

    /// Fills the parity part of a whole code word (`n` symbols, word order) from its data part.
    pub fn encode_word(&self, word: &mut [u8]) -> Result<()> {
        let (k, n) = (self.code.k as usize, self.code.n as usize);
        if word.len() != n {
            return Err(FecError::InputLength {
                expected: n,
                got: word.len(),
            });
        }
        match self.code.order {
            SymbolOrder::HighFirst => {
                let (d, p) = word.split_at_mut(k);
                self.encode(d, p)
            }
            SymbolOrder::LowFirst => {
                let (p, d) = word.split_at_mut(n - k);
                self.encode(d, p)
            }
        }
    }

    /// Loads a word (any symbol type) into the conventional-basis scratch, indexed by power.
    fn load(&mut self, word: impl Iterator<Item = u16>) {
        let n = self.code.n as usize;
        for (w, s) in word.enumerate() {
            let p = match self.code.order {
                SymbolOrder::HighFirst => n - 1 - w,
                SymbolOrder::LowFirst => w,
            };
            self.word[p] = match self.code.basis {
                Basis::Conventional => s,
                Basis::Dual => FROM_DUAL[s as u8 as usize] as u16,
            };
        }
    }

    /// Whether the syndromes of the loaded word are all zero; computes them into `synd`.
    fn syndromes(&mut self) -> bool {
        self.backend.syndromes(
            &self.tables,
            &self.gf,
            &self.word,
            &self.roots,
            &mut self.synd,
        );
        self.synd.iter().all(|&s| s == 0)
    }

    /// Whether a word (byte symbols) is a code word.
    pub fn check(&mut self, word: &[u8]) -> Result<bool> {
        self.check_len(word.len())?;
        self.load(word.iter().map(|&s| s as u16));
        Ok(self.syndromes())
    }

    fn check_len(&self, len: usize) -> Result<()> {
        if len != self.code.n as usize {
            return Err(FecError::InputLength {
                expected: self.code.n as usize,
                got: len,
            });
        }
        if self.code.m > 8 {
            return Param::M.range(2, 8, self.code.m);
        }
        Ok(())
    }

    /// Decodes a code word in place (byte symbols, word order, the code's basis). `erasures` are word indices of symbols known
    /// to be unreliable (each at most once). `corrected` in the result counts the symbols changed and `metric` the erasures
    /// given. On failure the word is left as received.
    pub fn decode(&mut self, word: &mut [u8], erasures: &[u16]) -> Result<Decoded> {
        self.check_len(word.len())?;
        self.load(word.iter().map(|&s| s as u16));
        let count = self.decode_loaded(erasures)?;
        for i in 0..count {
            let w = self.word_index(self.locs[i] as usize);
            let c = self.conv_of(word[w] as u16) ^ self.vals[i];
            word[w] = self.air_of(c) as u8;
        }
        Ok(Decoded {
            corrected: count as u32,
            metric: erasures.len() as i32,
            iterations: 1,
        })
    }

    /// [`Rs::decode`] with `u16` symbols (any `m`).
    pub fn decode16(&mut self, word: &mut [u16], erasures: &[u16]) -> Result<Decoded> {
        if word.len() != self.code.n as usize {
            return Err(FecError::InputLength {
                expected: self.code.n as usize,
                got: word.len(),
            });
        }
        let q = 1u32 << self.code.m;
        if word.iter().any(|&s| s as u32 >= q) {
            return Param::Symbol.range(
                0,
                q as i64 - 1,
                word.iter()
                    .find(|&&s| s as u32 >= q)
                    .map_or(0, |&s| s as i64),
            );
        }
        self.load(word.iter().copied());
        let count = self.decode_loaded(erasures)?;
        for i in 0..count {
            let w = self.word_index(self.locs[i] as usize);
            let c = self.conv_of(word[w]) ^ self.vals[i];
            word[w] = self.air_of(c);
        }
        Ok(Decoded {
            corrected: count as u32,
            metric: erasures.len() as i32,
            iterations: 1,
        })
    }

    #[inline]
    fn word_index(&self, p: usize) -> usize {
        // The map between word index and power is its own inverse.
        self.power(p)
    }

    /// Decodes an interleaved frame of `depth` code words: symbol `i` of the frame belongs to code word `i mod depth`, at index
    /// `i / depth` (CCSDS depth 1 to 8, UAT, RS41 depth 2). `erasures` are frame indices. Code words that cannot be decoded are
    /// left as received and counted in the result.
    pub fn decode_interleaved(
        &mut self,
        frame: &mut [u8],
        depth: usize,
        erasures: &[u16],
    ) -> Result<FrameDecoded> {
        let n = self.code.n as usize;
        if depth == 0 || depth > 32 {
            return Param::Depth.range(1, 32, depth);
        }
        if frame.len() != n * depth {
            return Err(FecError::InputLength {
                expected: n * depth,
                got: frame.len(),
            });
        }
        if self.code.m > 8 {
            return Param::M.range(2, 8, self.code.m);
        }
        if let Some(&e) = erasures.iter().find(|&&e| e as usize >= frame.len()) {
            return Err(FecError::BadErasure { at: e as usize });
        }
        let mut res = FrameDecoded::default();
        let mut er = [0u16; 256];
        for cw in 0..depth {
            let mut ne = 0;
            for &e in erasures {
                if e as usize % depth == cw && ne < er.len() {
                    er[ne] = e / depth as u16;
                    ne += 1;
                }
            }
            self.load((0..n).map(|i| frame[i * depth + cw] as u16));
            match self.decode_loaded(&er[..ne]) {
                Ok(count) => {
                    for i in 0..count {
                        let w = self.word_index(self.locs[i] as usize);
                        let f = &mut frame[w * depth + cw];
                        let c = self.conv_of(*f as u16) ^ self.vals[i];
                        *f = self.air_of(c) as u8;
                    }
                    res.corrected += count as u32;
                }
                Err(FecError::Uncorrectable) => {
                    res.failed += 1;
                    res.failed_mask |= 1 << cw;
                }
                Err(e) => return Err(e),
            }
        }
        Ok(res)
    }

    /// The core: the word is loaded (conventional basis, by power). Finds the corrections (positions as powers in `locs`, values
    /// in `vals`) and returns their number, after checking them.
    fn decode_loaded(&mut self, erasures: &[u16]) -> Result<usize> {
        let n = self.code.n as usize;
        let nr = self.code.nroots();
        let nn = self.gf.order();
        if erasures.len() > nr {
            return Err(FecError::Uncorrectable);
        }
        // Erasure positions as powers; check range and duplicates.
        for (i, &e) in erasures.iter().enumerate() {
            if e as usize >= n {
                return Err(FecError::BadErasure { at: e as usize });
            }
            if erasures[..i].contains(&e) {
                return Err(FecError::BadErasure { at: e as usize });
            }
        }
        if self.syndromes() && erasures.is_empty() {
            return Ok(0);
        }
        let gf = &self.gf;
        let prim = self.prim;
        // Erasure locator Γ(x) = Π (1 + X_k x), X_k = γ^p.
        self.lambda.fill(0);
        self.lambda[0] = 1;
        for (k, &e) in erasures.iter().enumerate() {
            let p = self.power(e as usize);
            let x = gf.exp((prim * p) % nn);
            for i in (1..=k + 1).rev() {
                self.lambda[i] ^= gf.mul(x, self.lambda[i - 1]);
            }
        }
        let rho = erasures.len();
        self.b.copy_from_slice(&self.lambda);
        // Berlekamp-Massey from the erasure locator.
        let mut el = rho;
        for r in rho + 1..=nr {
            let mut discr = 0u16;
            for i in 0..r {
                if self.lambda[i] != 0 {
                    discr ^= gf.mul(self.lambda[i], self.synd[r - 1 - i]);
                }
            }
            if discr == 0 {
                // b = x b
                self.b.copy_within(0..nr, 1);
                self.b[0] = 0;
            } else {
                // t = lambda + discr x b
                self.t[0] = self.lambda[0];
                for i in 0..nr {
                    self.t[i + 1] = self.lambda[i + 1] ^ gf.mul(discr, self.b[i]);
                }
                if 2 * el < r + rho {
                    el = r + rho - el;
                    for i in 0..=nr {
                        self.b[i] = gf.div(self.lambda[i], discr).unwrap_or(0);
                    }
                } else {
                    self.b.copy_within(0..nr, 1);
                    self.b[0] = 0;
                }
                self.lambda.copy_from_slice(&self.t);
            }
        }
        let deg = (0..=nr).rev().find(|&i| self.lambda[i] != 0).unwrap_or(0);
        if deg == 0 || deg > nr {
            return Err(FecError::Uncorrectable);
        }
        // Chien search over the sent positions only.
        let found = self.backend.chien(
            &self.tables,
            &self.gf,
            &self.lambda[..=deg],
            prim,
            n,
            &mut self.locs,
        );
        if found != deg {
            return Err(FecError::Uncorrectable);
        }
        let gf = &self.gf;
        // Ω(x) = S(x) Λ(x) mod x^nr.
        for i in 0..nr {
            let mut v = 0u16;
            for j in 0..=i.min(deg) {
                v ^= gf.mul(self.synd[i - j], self.lambda[j]);
            }
            self.omega[i] = v;
        }
        let fcr = self.code.fcr as usize;
        for i in 0..deg {
            let p = self.locs[i] as usize;
            // X^-1 = γ^-p.
            let xinv_log = (nn - (prim * p) % nn) % nn;
            // Ω(X^-1)
            let mut num = 0u16;
            for (j, &o) in self.omega[..nr].iter().enumerate() {
                num ^= gf.mul_exp(o, (xinv_log * j) % nn);
            }
            // Λ'(X^-1): the odd terms, λ_j x^(j-1).
            let mut den = 0u16;
            for j in (1..=deg).step_by(2) {
                den ^= gf.mul_exp(self.lambda[j], (xinv_log * (j - 1)) % nn);
            }
            let Some(q) = gf.div(num, den) else {
                return Err(FecError::Uncorrectable);
            };
            // e = X^(1 - fcr) Ω(X^-1) / Λ'(X^-1).
            let xe = ((prim * p) % nn * ((1 + nn - fcr % nn) % nn)) % nn;
            self.vals[i] = gf.mul_exp(q, xe);
        }
        // Re-check: apply to the scratch copy and recompute the syndromes.
        for i in 0..deg {
            let p = self.locs[i] as usize;
            self.word[p] ^= self.vals[i];
        }
        let ok = self.syndromes();
        if !ok {
            return Err(FecError::Uncorrectable);
        }
        // Erasures whose value was already right are not changes.
        let mut count = 0;
        for i in 0..deg {
            if self.vals[i] != 0 {
                self.locs[count] = self.locs[i];
                self.vals[count] = self.vals[i];
                count += 1;
            }
        }
        Ok(count)
    }
}

fn gcd(a: usize, b: usize) -> usize {
    if b == 0 {
        a
    } else {
        gcd(b, a % b)
    }
}

/// Scalar syndromes: `S_j = Σ_p word[p] root_j^p` by Horner from the highest power.
pub(crate) fn syndromes_scalar(gf: &Gf, word: &[u16], roots: &[u16], synd: &mut [u16]) {
    for (s, &r) in synd.iter_mut().zip(roots) {
        let lr = gf.log(r).unwrap_or(0);
        let mut acc = 0u16;
        for &c in word.iter().rev() {
            acc = gf.mul_exp(acc, lr) ^ c;
        }
        *s = acc;
    }
}

/// Scalar Chien search: the powers `p < n` with `Λ(γ^-p) = 0`, written to `locs`; returns how many (stops at `locs.len()`).
pub(crate) fn chien_scalar(
    gf: &Gf,
    lambda: &[u16],
    prim: usize,
    n: usize,
    locs: &mut [u16],
) -> usize {
    let nn = gf.order();
    let mut found = 0;
    for p in 0..n {
        let xinv = (nn - (prim * p) % nn) % nn;
        let mut v = 0u16;
        for (j, &l) in lambda.iter().enumerate() {
            v ^= gf.mul_exp(l, (xinv * j) % nn);
        }
        if v == 0 {
            if found == locs.len() {
                return found + 1;
            }
            locs[found] = p as u16;
            found += 1;
        }
    }
    found
}
