//! The layered min-sum decoder with `i8` messages and `i8` posterior LLRs, `z` checks at a time.

use super::{LdpcCode, NO_MASK};
use crate::error::{Decoded, FecError, Result};
use crate::llr::Llr8;
use sdrkit_core::backend::{Backend, Registry};

/// The check-node rule: the minimum magnitude, scaled by `scale_q4 / 16`, less `offset` (normalised and offset min-sum).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MinSum {
    /// Scale in sixteenths (16 = none, 12 = 0.75).
    pub scale_q4: u8,
    /// Subtracted after scaling.
    pub offset: u8,
}

impl MinSum {
    /// Normalised min-sum with the scale `scale_q4 / 16` and no offset.
    pub const fn with_scale(scale_q4: u8) -> Self {
        MinSum { scale_q4, offset: 0 }
    }

    /// The normalised min-sum scale for a code of `k` information bits in `n`: 15/16 for a rate below 0.29, 14/16 for a
    /// rate below 0.47, else 13/16.
    ///
    /// The checks of a low-rate code join few bits, so their minimum is nearer to the true message and is scaled less.
    /// Measured on the DVB-S2 codes of both frame sizes with the default [`crate::Quantizer`] and 50 iterations
    /// (`examples/ldpc_scale.rs`): the scale of the lowest Eb/N0 at which frames decode, per code. The steps lie midway
    /// between measured rates (0.25 and 0.33, 0.44 and 0.5). With these scales a few frames of the rates below 0.4 end
    /// with checks open above that Eb/N0; their information bits are right. A code of another structure, or one that is
    /// shortened or punctured, needs its own measurement.
    pub const fn for_rate(k: usize, n: usize) -> Self {
        let scale_q4 = if 100 * k < 29 * n {
            15
        } else if 100 * k < 47 * n {
            14
        } else {
            13
        };
        MinSum::with_scale(scale_q4)
    }
}

impl Default for MinSum {
    /// Normalised min-sum with a scale of 13/16 (0.8125): on the DVB-S2 normal rate 1/2 code this decodes at 1.0 dB Eb/N0,
    /// where 0.75 and below stall (measured in `tests/ldpc.rs`).
    fn default() -> Self {
        MinSum {
            scale_q4: 13,
            offset: 0,
        }
    }
}

/// A decoder that an [`LdpcBackend`] made for one code and check rule.
pub trait LdpcKernel: Send {
    /// Decodes `llr` (`n` values in code word order, positive means 0) of `code` with at most `max_iter` iterations, as
    /// [`LdpcDecoder::decode`] describes. `code` is the code the kernel was made for. The lengths of `llr` and `out` are
    /// already checked.
    fn decode(
        &mut self,
        code: &LdpcCode,
        llr: &[Llr8],
        max_iter: u32,
        out: &mut [u8],
    ) -> Result<Decoded>;
    /// A copy with its own scratch.
    fn box_clone(&self) -> Box<dyn LdpcKernel>;
    /// Writes the posterior LLRs that the last [`LdpcKernel::decode`] left (`n` values in code word order, in the units of its
    /// input, positive means 0) to `out` and returns true. A kernel that keeps no posteriors returns false and leaves `out`
    /// as it was. The length of `out` is already checked.
    fn posterior(&self, _code: &LdpcCode, _out: &mut [i16]) -> bool {
        false
    }
}

/// A backend of the LDPC family: it makes [`LdpcKernel`]s.
///
/// A minimal backend that uses the plain decoder:
///
/// ```
/// use sdrkit_core::backend::Backend;
/// use sdrkit_algo_fec::ldpc::{check_backend, LdpcBackend, LdpcCode, LdpcKernel, MinSum, BACKENDS};
///
/// struct Delegate;
/// impl Backend for Delegate {
///     fn name(&self) -> &'static str { "delegate" }
/// }
/// impl LdpcBackend for Delegate {
///     fn decoder(&self, code: &LdpcCode, rule: MinSum) -> Option<Box<dyn LdpcKernel>> {
///         BACKENDS.plain().decoder(code, rule)
///     }
/// }
///
/// check_backend(&Delegate);
/// ```
pub trait LdpcBackend: Backend {
    /// A decoder for `code` with the check rule `rule`. `None` when the backend cannot do the code or fails to start.
    fn decoder(&self, code: &LdpcCode, rule: MinSum) -> Option<Box<dyn LdpcKernel>>;
}

/// The LDPC backends of this platform, the default first and the plain backend last.
pub static BACKENDS: Registry<dyn LdpcBackend> = Registry::new(TABLE);

#[cfg(target_arch = "aarch64")]
const TABLE: &[&dyn LdpcBackend] = &[&super::minsum_simd::Neon, &Plain];
#[cfg(not(target_arch = "aarch64"))]
const TABLE: &[&dyn LdpcBackend] = &[&Plain];

/// Makes sure that `backend` agrees with the plain backend: 20 noisy frames of the 5G NR base graph 2 code lifted by 16
/// (rate 1/5 mother code, the first `2 z` bits not sent), at 4 and 8 LLR units of noise for a mean of 16. On every frame that the
/// plain decoder decodes, the backend must decode the same bits. A frame the plain decoder does not decode is not compared.
///
/// # Panics
/// When the backend does not decode a frame the plain decoder decodes, or decodes other bits, or makes no decoder.
pub fn check_backend(backend: &dyn LdpcBackend) {
    let code = LdpcCode::nr(2, 16).expect("a lifting size of table 5.3.2-1");
    let rule = MinSum::default();
    let mut mine = backend
        .decoder(&code, rule)
        .unwrap_or_else(|| panic!("{} makes no decoder", backend.name()));
    let mut plain = Plain
        .decoder(&code, rule)
        .expect("the plain backend does every code");
    let mut seed = 0x2f69_3c7e_51a8_b40du64;
    let mut next = move || {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        seed
    };
    let (n, k) = (code.n(), code.k());
    let mut word = vec![0u8; n];
    let (mut a, mut b) = (vec![0u8; n], vec![0u8; n]);
    for frame in 0..20 {
        let info: Vec<u8> = (0..k).map(|_| (next() & 1) as u8).collect();
        code.encode(&info, &mut word)
            .expect("the code has an encoder");
        let noise = if frame % 2 == 0 { 4 } else { 8 };
        let llr: Vec<Llr8> = word
            .iter()
            .enumerate()
            .map(|(i, &c)| {
                if i < 2 * code.z() {
                    return 0;
                }
                let v = if c == 0 { 16 } else { -16 };
                let u = (next() % (2 * noise + 1)) as i32 - noise as i32;
                (v + u) as Llr8
            })
            .collect();
        let want = plain.decode(&code, &llr, 30, &mut a);
        if want.is_err() {
            continue;
        }
        let got = mine.decode(&code, &llr, 30, &mut b);
        assert!(
            got.is_ok() && a == b,
            "{} differs from the plain decoder on frame {frame}",
            backend.name()
        );
    }
}

/// The plain backend: the layered min-sum decoder below.
struct Plain;

impl Backend for Plain {
    fn name(&self) -> &'static str {
        "plain"
    }
}

impl LdpcBackend for Plain {
    fn decoder(&self, code: &LdpcCode, rule: MinSum) -> Option<Box<dyn LdpcKernel>> {
        Some(Box::new(LayeredMinSum::new(code, rule)))
    }
}

/// A layered min-sum LDPC decoder for one code, made by the first backend of [`BACKENDS`] that does the code. The plain
/// backend keeps every buffer sized at construction. Check messages are `i8`; the posterior LLRs are `i16`, so they never
/// saturate and the message updates stay exact.
pub struct LdpcDecoder {
    code: LdpcCode,
    kernel: Box<dyn LdpcKernel>,
    backend: &'static str,
}

impl Clone for LdpcDecoder {
    fn clone(&self) -> Self {
        LdpcDecoder {
            code: self.code.clone(),
            kernel: self.kernel.box_clone(),
            backend: self.backend,
        }
    }
}

impl std::fmt::Debug for LdpcDecoder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LdpcDecoder")
            .field("code", &self.code.name())
            .field("backend", &self.backend)
            .finish()
    }
}

impl LdpcDecoder {
    /// A decoder for `code` with the check rule `rule`.
    pub fn new(code: &LdpcCode, rule: MinSum) -> Self {
        let (backend, kernel) = BACKENDS
            .build(|b| b.decoder(code, rule).map(|k| (b.name(), k)))
            .unwrap_or_else(|| (Plain.name(), Box::new(LayeredMinSum::new(code, rule))));
        LdpcDecoder {
            code: code.clone(),
            kernel,
            backend,
        }
    }

    /// The code.
    pub fn code(&self) -> &LdpcCode {
        &self.code
    }

    /// The name of the backend that made the decoder.
    pub fn backend(&self) -> &'static str {
        self.backend
    }

    /// Decodes `llr` (`n` values, code word order; punctured or unknown bits 0) with at most `max_iter` iterations, stopping as
    /// soon as every check is satisfied. Writes the `n` hard bits to `out`. `corrected` counts the non-zero input values whose
    /// sign the decoder changed, `iterations` the iterations used, `metric` 0. When the checks are still not all satisfied after
    /// `max_iter` iterations, `out` holds the last hard decisions and the result is `Err(Uncorrectable)`.
    pub fn decode(&mut self, llr: &[Llr8], max_iter: u32, out: &mut [u8]) -> Result<Decoded> {
        let n = self.code.n();
        if llr.len() != n {
            return Err(FecError::InputLength {
                expected: n,
                got: llr.len(),
            });
        }
        if out.len() < n {
            return Err(FecError::OutputTooShort { need: n });
        }
        self.kernel.decode(&self.code, llr, max_iter, out)
    }

    /// The posterior LLRs of the last [`LdpcDecoder::decode`], decoded or not, into `out` (`n` values, code word order, in
    /// the units of the input LLRs, positive means 0). They are the soft output of the decoder: their signs are the hard bits
    /// that `decode` wrote, their magnitudes the confidence. Before the first decode they are 0. An error when `out` is shorter
    /// than `n` or the backend keeps no posteriors.
    pub fn posterior(&self, out: &mut [i16]) -> Result<()> {
        let n = self.code.n();
        if out.len() < n {
            return Err(FecError::OutputTooShort { need: n });
        }
        if self.kernel.posterior(&self.code, &mut out[..n]) {
            Ok(())
        } else {
            crate::error::Param::Kernel.unsupported()
        }
    }
}

/// The plain layered min-sum decoder.
///
/// Each layer reads its `z`-lane edge groups rotated into a scratch buffer, finds the two smallest magnitudes and the sign
/// product lane by lane, and writes back the change of each check message. All inner loops run over `z` contiguous `i8` lanes,
/// so they vectorise (NEON or SSE) without hand-written intrinsics. Two edges of one layer that reach the same bit add their
/// changes, which keeps the decoder correct for codes whose layers touch a bit twice. Input LLRs are `i8` (positive means 0);
/// a target mean magnitude of about 12 to 20 leaves room for the posteriors to grow.
#[derive(Clone, Debug)]
struct LayeredMinSum {
    rule: MinSum,
    l: Vec<i16>,
    r: Vec<i8>,
    q: Vec<i8>,
    min1: Vec<u8>,
    min2: Vec<u8>,
    idx: Vec<u8>,
    sgn: Vec<u8>,
    tmp: Vec<i16>,
}

impl LdpcKernel for LayeredMinSum {
    fn decode(
        &mut self,
        code: &LdpcCode,
        llr: &[Llr8],
        max_iter: u32,
        out: &mut [u8],
    ) -> Result<Decoded> {
        self.run(code, llr, max_iter, out)
    }

    fn box_clone(&self) -> Box<dyn LdpcKernel> {
        Box::new(self.clone())
    }

    fn posterior(&self, code: &LdpcCode, out: &mut [i16]) -> bool {
        for (i, &v) in self.l[..code.n()].iter().enumerate() {
            out[code.pos(i)] = v;
        }
        true
    }
}

impl LayeredMinSum {
    fn new(code: &LdpcCode, rule: MinSum) -> Self {
        let z = code.z();
        let maxdeg = (0..code.layers())
            .map(|i| code.layer(i).len())
            .max()
            .unwrap_or(0);
        LayeredMinSum {
            l: vec![0; code.ncols() * z],
            r: vec![0; code.all_edges().len() * z],
            q: vec![0; maxdeg * z],
            min1: vec![0; z],
            min2: vec![0; z],
            idx: vec![0; z],
            sgn: vec![0; z],
            tmp: vec![0; z],
            rule,
        }
    }

    fn run(
        &mut self,
        code: &LdpcCode,
        llr: &[Llr8],
        max_iter: u32,
        out: &mut [u8],
    ) -> Result<Decoded> {
        let n = code.n();
        for i in 0..n {
            self.l[i] = llr[code.pos(i)].max(-127) as i16;
        }
        self.r.fill(0);
        let mut used = 0;
        let mut ok = self.syndrome_ok(code);
        while !ok && used < max_iter {
            for li in 0..code.layers() {
                self.layer(code, li);
            }
            used += 1;
            ok = self.syndrome_ok(code);
        }
        let mut corrected = 0u32;
        for i in 0..n {
            let p = code.pos(i);
            let b = (self.l[i] < 0) as u8;
            out[p] = b;
            if llr[p] != 0 && ((llr[p] < 0) as u8) != b {
                corrected += 1;
            }
        }
        if ok {
            Ok(Decoded {
                corrected,
                metric: 0,
                iterations: used,
            })
        } else {
            Err(FecError::Uncorrectable)
        }
    }

    fn syndrome_ok(&mut self, code: &LdpcCode) -> bool {
        let z = code.z();
        for li in 0..code.layers() {
            let edges = code.layer(li);
            self.sgn.fill(0);
            for e in edges {
                let src = &self.l[e.col as usize * z..(e.col as usize + 1) * z];
                let s = e.shift as usize;
                let (a, b) = self.sgn.split_at_mut(z - s);
                for (x, &v) in a.iter_mut().zip(&src[s..]) {
                    *x ^= (v < 0) as u8;
                }
                for (x, &v) in b.iter_mut().zip(&src[..s]) {
                    *x ^= (v < 0) as u8;
                }
                if e.mask != NO_MASK {
                    let m = e.mask as usize;
                    self.sgn[m] ^= (src[(m + s) % z] < 0) as u8;
                }
            }
            if self.sgn.iter().any(|&v| v != 0) {
                return false;
            }
        }
        true
    }

    fn layer(&mut self, code: &LdpcCode, li: usize) {
        let z = code.z();
        let start = code.layer_start[li] as usize;
        let edges = &code.edges[start..code.layer_start[li + 1] as usize];
        self.min1.fill(127);
        self.min2.fill(127);
        self.idx.fill(0);
        self.sgn.fill(0);
        for (e, edge) in edges.iter().enumerate() {
            let q = &mut self.q[e * z..(e + 1) * z];
            let src = &self.l[edge.col as usize * z..(edge.col as usize + 1) * z];
            let s = edge.shift as usize;
            let r = &self.r[(start + e) * z..(start + e + 1) * z];
            // q = L - R (rotated), limited to the i8 range for the check rule.
            for ((x, &lv), &rv) in q[..z - s].iter_mut().zip(&src[s..]).zip(r) {
                *x = (lv - rv as i16).clamp(-127, 127) as i8;
            }
            for ((x, &lv), &rv) in q[z - s..].iter_mut().zip(&src[..s]).zip(&r[z - s..]) {
                *x = (lv - rv as i16).clamp(-127, 127) as i8;
            }
            if edge.mask != NO_MASK {
                q[edge.mask as usize] = 127;
            }
            let eb = e as u8;
            for l in 0..z {
                let v = q[l];
                let a = v.unsigned_abs();
                let m1 = self.min1[l];
                let lt = a < m1;
                self.min2[l] = if lt { m1 } else { self.min2[l].min(a) };
                self.min1[l] = if lt { a } else { m1 };
                self.idx[l] = if lt { eb } else { self.idx[l] };
                self.sgn[l] ^= (v as u8) >> 7;
            }
        }
        let (sc, off) = (self.rule.scale_q4 as u16, self.rule.offset);
        for v in self.min1.iter_mut().chain(self.min2.iter_mut()) {
            *v = (((*v as u16 * sc + 8) >> 4).min(127) as u8).saturating_sub(off);
        }
        for (e, edge) in edges.iter().enumerate() {
            let q = &self.q[e * z..(e + 1) * z];
            let dst = &mut self.l[edge.col as usize * z..(edge.col as usize + 1) * z];
            let s = edge.shift as usize;
            self.tmp[..z - s].copy_from_slice(&dst[s..]);
            self.tmp[z - s..].copy_from_slice(&dst[..s]);
            let keep = if edge.mask != NO_MASK {
                self.tmp[edge.mask as usize]
            } else {
                0
            };
            let r = &mut self.r[(start + e) * z..(start + e + 1) * z];
            let eb = e as u8;
            for l in 0..z {
                let mag = if self.idx[l] == eb {
                    self.min2[l]
                } else {
                    self.min1[l]
                } as i16;
                let neg = (self.sgn[l] ^ ((q[l] as u8) >> 7)) & 1;
                let rn = if neg == 1 { -mag } else { mag };
                let old = r[l] as i16;
                r[l] = rn as i8;
                self.tmp[l] = (self.tmp[l] + rn - old).clamp(-30_000, 30_000);
            }
            if edge.mask != NO_MASK {
                let m = edge.mask as usize;
                self.tmp[m] = keep;
                r[m] = 0;
            }
            dst[s..].copy_from_slice(&self.tmp[..z - s]);
            dst[..s].copy_from_slice(&self.tmp[z - s..]);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::MinSum;

    #[test]
    fn the_scale_of_a_rate_steps_at_the_measured_places() {
        let scale = |k, n| MinSum::for_rate(k, n).scale_q4;
        assert_eq!([scale(1, 5), scale(1, 4), scale(28, 100)], [15; 3]);
        assert_eq!([scale(29, 100), scale(1, 3), scale(2, 5), scale(4, 9), scale(46, 100)], [14; 5]);
        assert_eq!([scale(47, 100), scale(1, 2), scale(3, 5), scale(9, 10), scale(1, 1)], [13; 5]);
        assert_eq!(MinSum::for_rate(1, 2), MinSum::default());
        assert_eq!(MinSum::with_scale(13), MinSum::default());
        assert_eq!(MinSum::for_rate(1, 4).offset, 0);
    }
}
