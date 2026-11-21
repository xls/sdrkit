//! Parallel concatenated (turbo) codes: one decoder for any pair of identical recursive systematic constituents of 8 or 16
//! states with one or two parities, any interleaver, and any puncturing and repetition given as a transmission map.
//!
//! - [`Rsc`] describes a constituent by its polynomials; [`TurboCode`] adds the interleaver, the block length and how each
//!   constituent ends ([`End`]).
//! - [`TurboDecoder`] runs max-log-MAP with a scaled extrinsic. The caller fills the channel LLRs per (stream, period), either one
//!   at a time ([`TurboDecoder::add`]) or from a transmission map ([`TurboDecoder::load`]): repeated bits add, punctured bits
//!   stay 0.
//! - [`Turbo`] is the LTE / NB-IoT code (3GPP TS 36.212 clause 5.1.3.2) as one configuration: two 8-state encoders with
//!   `[1, g1(D) / g0(D)]`, `g0 = 1 + D^2 + D^3`, `g1 = 1 + D + D^3`, the QPP interleaver of table 5.1.3-3 and the LTE tail layout.
//!
//! Streams of constituent `c` (0 or 1) are numbered `c (1 + P) + j`: `j = 0` is the systematic bit, `j = 1 .. P` the parities.
//! Periods count trellis steps of that constituent: `0 .. n` are data periods, then its tail periods. The named codes of
//! [`crate::turbo_codes`] build their transmission maps on this numbering.

use crate::error::{Decoded, FecError, Param, Result};
use crate::tables_3gpp::LTE_QPP;

mod kernel;
#[cfg(test)]
mod tests;

use kernel::{Branch, Trellis};

/// The 188 turbo interleaver sizes of TS 36.212 table 5.1.3-3, smallest first.
pub fn sizes() -> impl Iterator<Item = usize> {
    LTE_QPP.iter().map(|&(k, _, _)| k as usize)
}

/// The smallest turbo interleaver size of at least `b` bits (the code block size `K+` of clause 5.1.2), if any.
pub fn size_at_least(b: usize) -> Option<usize> {
    sizes().find(|&k| k >= b)
}

const MAX_STATES: usize = 16;

/// A recursive systematic convolutional constituent. Polynomials hold the coefficient of `D^i` in bit `i`; `feedback` has bit
/// 0 set. The register input is `a = u + sum(d_i a(k-i))`, parity `j` is `sum(n_ji a(k-i))` with `a(k-0) = a`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rsc {
    /// Memory `m` (3 or 4: 8 or 16 states).
    pub memory: u32,
    /// The feedback polynomial `d(D)`.
    pub feedback: u32,
    /// The forward polynomials; only the first `parities` are used.
    pub forward: [u32; 2],
    /// Parities per constituent (1 or 2).
    pub parities: usize,
}

/// The LTE constituent: `g0 = 1 + D^2 + D^3`, `g1 = 1 + D + D^3`.
pub const RSC_LTE: Rsc = Rsc {
    memory: 3,
    feedback: 0b1101,
    forward: [0b1011, 0],
    parities: 1,
};

/// The 3GPP2 constituent of C.S0002 and ETSI EN 302 583 clause 5.3.1: `d = 1 + D^2 + D^3`, `n0 = 1 + D + D^3`,
/// `n1 = 1 + D + D^2 + D^3`.
pub const RSC_3GPP2: Rsc = Rsc {
    memory: 3,
    feedback: 0b1101,
    forward: [0b1011, 0b1111],
    parities: 2,
};

/// The 16-state constituent of ETSI TS 102 744-2-1 clause 5.3.8: feedback 23 octal `1 + X^3 + X^4`, forward 35 octal
/// `1 + X + X^2 + X^4`.
pub const RSC_SL: Rsc = Rsc {
    memory: 4,
    feedback: 0b1_1001,
    forward: [0b1_0111, 0],
    parities: 1,
};

impl Rsc {
    /// Number of trellis states.
    pub fn states(&self) -> usize {
        1 << self.memory
    }

    /// Streams per constituent (systematic and parities).
    pub fn streams(&self) -> usize {
        1 + self.parities
    }

    /// One step from state `s` (newest register value in the top bit) with input `u`: (parity bits, bit `j` = parity `j`; next
    /// state).
    #[inline]
    pub fn step(&self, s: usize, u: u8) -> (u8, usize) {
        let m = self.memory;
        // Register value a(k-i) is bit (m - i) of s.
        let mut a = u as u32 & 1;
        for i in 1..=m {
            if self.feedback >> i & 1 == 1 {
                a ^= (s as u32 >> (m - i)) & 1;
            }
        }
        let mut out = 0u8;
        for j in 0..self.parities {
            let g = self.forward[j];
            let mut p = a & g;
            for i in 1..=m {
                if g >> i & 1 == 1 {
                    p ^= (s as u32 >> (m - i)) & 1;
                }
            }
            out |= (p as u8 & 1) << j;
        }
        (out, ((a << (m - 1)) | (s as u32 >> 1)) as usize)
    }

    /// The input that makes the register input 0 in state `s` (one tail or flush bit).
    #[inline]
    pub fn zeroing_input(&self, s: usize) -> u8 {
        let m = self.memory;
        let mut u = 0u32;
        for i in 1..=m {
            if self.feedback >> i & 1 == 1 {
                u ^= (s as u32 >> (m - i)) & 1;
            }
        }
        u as u8
    }

    fn check(&self) -> Result<()> {
        if !(2..=4).contains(&self.memory) {
            return Err(FecError::OutOfRange {
                param: Param::States,
                min: 4,
                max: MAX_STATES as i64,
                got: 1 << self.memory.min(16),
            });
        }
        if !(1..=2).contains(&self.parities) {
            return Err(FecError::OutOfRange {
                param: Param::Generators,
                min: 1,
                max: 2,
                got: self.parities as i64,
            });
        }
        if self.feedback & 1 == 0 || self.feedback >> (self.memory + 1) != 0 {
            return Err(FecError::Degenerate {
                param: Param::Generator,
            });
        }
        Ok(())
    }
}

/// How a constituent ends its block.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum End {
    /// Terminated by `m` tail periods after the data, each with its own systematic and parity bits.
    Tail,
    /// Terminated inside the data: the last `m` data bits are flush bits that bring the register to 0 (constituent 0 only).
    Flush,
    /// Not terminated: the end state is unknown.
    Open,
}

/// A turbo code: two identical constituents, an interleaver, the data length and the ends.
#[derive(Clone, Debug)]
pub struct TurboCode {
    rsc: Rsc,
    k: usize,
    n: usize,
    pi: Vec<u32>,
    ends: [End; 2],
    scale: (i32, i32),
}

impl TurboCode {
    /// A code over `pi.len()` data periods. Step `i` of encoder 1 reads data bit `i`; step `i` of encoder 2 reads data bit
    /// `pi[i]`. With [`End::Flush`] on constituent 0 the data holds `k` information bits and `m` flush bits (`pi.len() = k + m`),
    /// else `pi.len() = k`. Constituent 1 cannot use [`End::Flush`]. The extrinsic scale is 3/4.
    pub fn new(rsc: Rsc, k: usize, pi: Vec<u32>, ends: [End; 2]) -> Result<Self> {
        rsc.check()?;
        let n = pi.len();
        let flush = if ends[0] == End::Flush {
            rsc.memory as usize
        } else {
            0
        };
        if ends[1] == End::Flush {
            return Err(FecError::Unsupported {
                param: Param::Encoder,
            });
        }
        if k == 0 || n != k + flush {
            return Err(FecError::InputLength {
                expected: k + flush,
                got: n,
            });
        }
        let mut seen = vec![false; n];
        for &p in &pi {
            let p = p as usize;
            if p >= n || seen[p] {
                return Err(FecError::Degenerate {
                    param: Param::Permutation,
                });
            }
            seen[p] = true;
        }
        Ok(TurboCode {
            rsc,
            k,
            n,
            pi,
            ends,
            scale: (3, 4),
        })
    }

    /// Sets the extrinsic scale `num / den` (0 < scale <= 1).
    pub fn with_scale(mut self, num: i32, den: i32) -> Self {
        if num > 0 && den >= num {
            self.scale = (num, den);
        }
        self
    }

    /// The constituent.
    pub fn rsc(&self) -> Rsc {
        self.rsc
    }

    /// Information bits per block.
    pub fn k(&self) -> usize {
        self.k
    }

    /// Data periods (information and flush bits).
    pub fn n(&self) -> usize {
        self.n
    }

    /// The interleaver: step `i` of encoder 2 reads data bit `pi[i]`.
    pub fn interleaver(&self) -> &[u32] {
        &self.pi
    }

    /// Number of streams (both constituents).
    pub fn streams(&self) -> usize {
        2 * self.rsc.streams()
    }

    /// Periods held per stream (data and the longest tail).
    pub fn periods(&self) -> usize {
        self.n + self.rsc.memory as usize
    }

    /// Tail periods of constituent `c`.
    pub fn tail(&self, c: usize) -> usize {
        if self.ends[c] == End::Tail {
            self.rsc.memory as usize
        } else {
            0
        }
    }

    /// Encodes `bits` (`k`) into `raw` (`streams() x periods()` bits, stream after stream). Flush bits are computed; slots
    /// that no period uses are 0.
    pub fn encode(&self, bits: &[u8], raw: &mut [u8]) -> Result<()> {
        let per = self.periods();
        if raw.len() != self.streams() * per {
            return Err(FecError::OutputTooShort {
                need: self.streams() * per,
            });
        }
        raw.fill(0);
        self.encode_each(bits, |s, t, b| raw[s * per + t] = b)
    }

    /// Encodes `bits` (`k`), calling `f(stream, period, bit)` for every coded bit of both constituents.
    pub fn encode_each(&self, bits: &[u8], mut f: impl FnMut(usize, usize, u8)) -> Result<()> {
        let n = self.n;
        if bits.len() != self.k {
            return Err(FecError::InputLength {
                expected: self.k,
                got: bits.len(),
            });
        }
        let r = self.rsc;
        let st = r.streams();
        // Constituent 0 over the data, with its flush bits and tail. The flush bits are needed again by encoder 2: they are the
        // zeroing inputs of encoder 1's last m states, kept here.
        let mut flush = [0u8; 4];
        let mut s = 0usize;
        for t in 0..n + self.tail(0) {
            let u = if t < self.k {
                bits[t] & 1
            } else {
                r.zeroing_input(s)
            };
            if t >= self.k && t < n {
                flush[t - self.k] = u;
            }
            f(0, t, u);
            let (p, ns) = r.step(s, u);
            for j in 0..r.parities {
                f(1 + j, t, p >> j & 1);
            }
            s = ns;
        }
        // Constituent 1 over the interleaved data.
        let mut s = 0usize;
        for t in 0..n + self.tail(1) {
            let u = if t < n {
                let i = self.pi[t] as usize;
                if i < self.k {
                    bits[i] & 1
                } else {
                    flush[i - self.k]
                }
            } else {
                r.zeroing_input(s)
            };
            f(st, t, u);
            let (p, ns) = r.step(s, u);
            for j in 0..r.parities {
                f(st + 1 + j, t, p >> j & 1);
            }
            s = ns;
        }
        Ok(())
    }

    /// The bits a transmission map sends: `out[i] = raw[map[i]]`.
    pub fn transmit(&self, map: &[Slot], raw: &[u8], out: &mut [u8]) -> Result<()> {
        if out.len() != map.len() {
            return Err(FecError::OutputTooShort { need: map.len() });
        }
        let per = self.periods();
        for (o, s) in out.iter_mut().zip(map) {
            *o = raw[s.stream as usize * per + s.period as usize];
        }
        Ok(())
    }

    /// Checks that every slot of `map` addresses a stream and period of this code.
    pub fn check_map(&self, map: &[Slot]) -> Result<()> {
        let st = self.rsc.streams();
        for s in map {
            let c = s.stream as usize / st;
            if c > 1 || s.period as usize >= self.n + self.tail(c) {
                return Err(FecError::Degenerate {
                    param: Param::Puncturing,
                });
            }
        }
        Ok(())
    }
}

/// One sent bit of a transmission map: a stream and a period of the code.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Slot {
    /// The stream (`c (1 + P) + j`).
    pub stream: u8,
    /// The period of that stream's constituent.
    pub period: u32,
}

/// A max-log-MAP decoder for one [`TurboCode`]. Buffers are sized at construction; decoding does not allocate.
#[derive(Clone, Debug)]
pub struct TurboDecoder {
    code: TurboCode,
    trellis: Trellis,
    plain: bool,
    chan: Vec<i16>,
    ls: Vec<i16>,
    ls2: Vec<i16>,
    la: Vec<i16>,
    le: Vec<i16>,
    la2: Vec<i16>,
    le2: Vec<i16>,
    alpha: Vec<i16>,
    hard: Vec<u8>,
}

impl TurboDecoder {
    /// A decoder for `code`.
    pub fn new(code: TurboCode) -> Self {
        let trellis = Trellis::new(&code.rsc);
        let per = code.periods();
        TurboDecoder {
            plain: false,
            chan: vec![0; code.streams() * per],
            ls: vec![0; per],
            ls2: vec![0; per],
            la: vec![0; per],
            le: vec![0; per],
            la2: vec![0; per],
            le2: vec![0; per],
            alpha: vec![0; (per + 1) * trellis.ns],
            trellis,
            hard: vec![0; code.k],
            code,
        }
    }

    /// The code.
    pub fn code(&self) -> &TurboCode {
        &self.code
    }

    /// Sets every channel LLR to 0.
    pub fn clear(&mut self) {
        self.chan.fill(0);
    }

    /// Adds `llr` (positive means 0) to the channel value of `stream` at `period`. Out-of-range slots are ignored.
    #[inline]
    pub fn add(&mut self, stream: usize, period: usize, llr: i16) {
        let per = self.code.periods();
        if let Some(v) = self.chan.get_mut(stream * per + period) {
            *v = v.saturating_add(llr);
        }
    }

    /// Clears the channel and adds `llr[i]` at `map[i]` for every `i`.
    pub fn load(&mut self, map: &[Slot], llr: &[i8]) -> Result<()> {
        if map.len() != llr.len() {
            return Err(FecError::InputLength {
                expected: map.len(),
                got: llr.len(),
            });
        }
        self.clear();
        let per = self.code.periods();
        for (s, &l) in map.iter().zip(llr) {
            if let Some(v) = self
                .chan
                .get_mut(s.stream as usize * per + s.period as usize)
            {
                *v = v.saturating_add(l as i16);
            }
        }
        Ok(())
    }

    /// Decodes the loaded channel with at most `iters` iterations, calling `check` on the `k` hard decisions after each one and
    /// stopping as soon as it returns true (a CRC). Writes `k` bits to `out`. `Err(Uncorrectable)` when `check` never passed
    /// (`out` holds the last decisions); with `iters = 0` one iteration runs and its decisions are the answer.
    pub fn decode(
        &mut self,
        iters: u32,
        mut check: impl FnMut(&[u8]) -> bool,
        out: &mut [u8],
    ) -> Result<Decoded> {
        let c = &self.code;
        let (k, n, per) = (c.k, c.n, c.periods());
        if out.len() < k {
            return Err(FecError::OutputTooShort { need: k });
        }
        let st = c.rsc.streams();
        let np = c.rsc.parities;
        // Systematic values: the data bits in natural order (with any copies sent as encoder 2's systematic), then each
        // constituent's tail.
        let chan = &self.chan;
        let s1 = &chan[..per];
        let s2 = &chan[st * per..(st + 1) * per];
        self.ls[..n].copy_from_slice(&s1[..n]);
        for i in 0..n {
            let p = c.pi[i] as usize;
            self.ls[p] = self.ls[p].saturating_add(s2[i]);
        }
        for i in 0..n {
            self.ls2[i] = self.ls[c.pi[i] as usize];
        }
        self.ls[n..].copy_from_slice(&s1[n..]);
        self.ls2[n..].copy_from_slice(&s2[n..]);
        self.la.fill(0);
        self.la2.fill(0);
        let par = |c0: usize, j: usize| &chan[(c0 * st + 1 + j) * per..(c0 * st + 2 + j) * per];
        let len0 = n + c.tail(0);
        let len1 = n + c.tail(1);
        let known0 = c.ends[0] != End::Open;
        let known1 = c.ends[1] != End::Open;
        let mut used = 0;
        let mut passed = false;
        for _ in 0..iters.max(1) {
            used += 1;
            // `la` holds the second constituent's extrinsic of the last iteration in natural order (0 at the start).
            kernel::run(
                &self.trellis,
                self.plain,
                &mut self.alpha,
                Branch {
                    sys: &self.ls,
                    p0: par(0, 0),
                    p1: (np > 1).then(|| par(0, 1)),
                    la: &self.la,
                },
                &mut self.le,
                n,
                len0,
                known0,
                c.scale,
            );
            for i in 0..n {
                self.la2[i] = self.le[c.pi[i] as usize];
            }
            kernel::run(
                &self.trellis,
                self.plain,
                &mut self.alpha,
                Branch {
                    sys: &self.ls2,
                    p0: par(1, 0),
                    p1: (np > 1).then(|| par(1, 1)),
                    la: &self.la2,
                },
                &mut self.le2,
                n,
                len1,
                known1,
                c.scale,
            );
            for i in 0..n {
                self.la[c.pi[i] as usize] = self.le2[i];
            }
            for i in 0..k {
                let l = self.ls[i] as i32 + self.le[i] as i32 + self.la[i] as i32;
                self.hard[i] = (l < 0) as u8;
            }
            if check(&self.hard) {
                passed = true;
                break;
            }
        }
        out[..k].copy_from_slice(&self.hard);
        let corrected = (0..k)
            .filter(|&i| self.ls[i] != 0 && ((self.ls[i] < 0) as u8) != self.hard[i])
            .count() as u32;
        if passed || iters == 0 {
            Ok(Decoded {
                corrected,
                metric: 0,
                iterations: used,
            })
        } else {
            Err(FecError::Uncorrectable)
        }
    }
}

/// An LTE turbo encoder and decoder for one block size `K`, sized at construction.
///
/// Streams are `d0 | d1 | d2`, each `K + 4` long, in the order of clause 5.1.3.2.2 (systematic, first parity, second parity, with
/// the 12 tail bits placed as that clause says).
///
/// ```
/// use sdrkit_algo_fec::turbo::Turbo;
/// let mut t = Turbo::new(40).unwrap();
/// let bits: Vec<u8> = (0..40).map(|i| (i % 3 == 0) as u8).collect();
/// let mut streams = [0u8; 3 * 44];
/// t.encode(&bits, &mut streams).unwrap();
/// let mut llr: Vec<i8> = streams.iter().map(|&b| if b == 0 { 20 } else { -20 }).collect();
/// llr[7] = -llr[7];
/// let mut out = [0u8; 40];
/// // Stop as soon as the decisions match (a real receiver checks a CRC here).
/// let r = t.decode(&llr, 8, |b| b == &bits[..], &mut out).unwrap();
/// assert_eq!(&out[..], &bits[..]);
/// assert_eq!(r.iterations, 1);
/// ```
#[derive(Clone, Debug)]
pub struct Turbo {
    dec: TurboDecoder,
    map: Vec<Slot>,
    pos: Vec<u32>,
    pi16: Vec<u16>,
}

impl Turbo {
    /// A codec for block size `k`, which must be one of the 188 sizes of table 5.1.3-3.
    pub fn new(k: usize) -> Result<Self> {
        let &(_, f1, f2) =
            LTE_QPP
                .iter()
                .find(|&&(kk, _, _)| kk as usize == k)
                .ok_or(FecError::NotAllowed {
                    param: Param::K,
                    got: k as i64,
                })?;
        let (f1, f2) = (f1 as u64, f2 as u64);
        let pi: Vec<u32> = (0..k as u64)
            .map(|i| ((f1 * i + f2 * i * i) % k as u64) as u32)
            .collect();
        let pi16 = pi.iter().map(|&p| p as u16).collect();
        let code = TurboCode::new(RSC_LTE, k, pi, [End::Tail, End::Tail])?;
        let map = lte_map(k);
        let per = code.periods();
        let mut pos = vec![u32::MAX; code.streams() * per];
        for (i, s) in map.iter().enumerate() {
            pos[s.stream as usize * per + s.period as usize] = i as u32;
        }
        Ok(Turbo {
            dec: TurboDecoder::new(code),
            map,
            pos,
            pi16,
        })
    }

    /// The block size `K`.
    pub fn k(&self) -> usize {
        self.dec.code.k
    }

    /// The QPP interleaver: `c'_i = c_{Π(i)}`.
    pub fn interleaver(&self) -> &[u16] {
        &self.pi16
    }

    /// Encodes `bits` (`K`) into `out` (`3 (K + 4)`: `d0`, `d1`, `d2`).
    pub fn encode(&self, bits: &[u8], out: &mut [u8]) -> Result<()> {
        let k = self.k();
        if out.len() != 3 * (k + 4) {
            return Err(FecError::OutputTooShort { need: 3 * (k + 4) });
        }
        let per = self.dec.code.periods();
        self.dec.code.encode_each(bits, |s, t, b| {
            if let Some(&i) = self.pos.get(s * per + t) {
                if let Some(o) = out.get_mut(i as usize) {
                    *o = b;
                }
            }
        })
    }

    /// Decodes `llr` (`3 (K + 4)`, `d0 | d1 | d2`, positive means 0; unsent bits 0) with at most `iters` iterations, calling
    /// `check` on the hard decisions after each one and stopping as soon as it returns true (a CRC). Writes `K` bits to `out`.
    /// `iterations` is the iterations used; `Err(Uncorrectable)` when `check` never passed (`out` holds the last decisions).
    pub fn decode(
        &mut self,
        llr: &[i8],
        iters: u32,
        check: impl FnMut(&[u8]) -> bool,
        out: &mut [u8],
    ) -> Result<Decoded> {
        let k = self.k();
        if llr.len() != 3 * (k + 4) {
            return Err(FecError::InputLength {
                expected: 3 * (k + 4),
                got: llr.len(),
            });
        }
        self.dec.load(&self.map, llr)?;
        self.dec.decode(iters, check, out)
    }
}

/// The LTE layout of `d0 | d1 | d2` (clause 5.1.3.2.2) as a transmission map: streams 0 and 1 are encoder 1's systematic and
/// parity, 2 and 3 encoder 2's.
fn lte_map(k: usize) -> Vec<Slot> {
    let sl = |stream: u8, period: usize| Slot {
        stream,
        period: period as u32,
    };
    let mut m = Vec::with_capacity(3 * (k + 4));
    // d0: x_i, then x_K, z_K+1, x'_K, z'_K+1.
    m.extend((0..k).map(|i| sl(0, i)));
    m.extend([sl(0, k), sl(1, k + 1), sl(2, k), sl(3, k + 1)]);
    // d1: z_i, then z_K, x_K+2, z'_K, x'_K+2.
    m.extend((0..k).map(|i| sl(1, i)));
    m.extend([sl(1, k), sl(0, k + 2), sl(3, k), sl(2, k + 2)]);
    // d2: z'_i, then x_K+1, z_K+2, x'_K+1, z'_K+2.
    m.extend((0..k).map(|i| sl(3, i)));
    m.extend([sl(0, k + 1), sl(1, k + 2), sl(2, k + 1), sl(3, k + 2)]);
    m
}
