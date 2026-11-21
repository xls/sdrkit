//! The adaptive entropy coder of CCSDS 121.0-B-3 (Lossless Data Compression), also called Rice coding.
//!
//! The coder works on blocks of `J` samples of `n` bits. Before coding, an optional preprocessor (clause 4) predicts each sample
//! from the one before it (the unit-delay predictor) and maps the difference to a non-negative number. The first sample of each
//! sample interval (RSI, a run of blocks in clause 5.1.5) is sent as it is: it is the anchor sample. For each block the encoder picks
//! the shortest of these options (clause 5):
//!
//! - zero block: one or more blocks of zeros, coded as a run (ROS: the run reaches the end of the 64-block segment).
//! - second extension: pairs of samples coded together.
//! - split sample with `k` low bits sent as they are, and the rest in a fundamental sequence (FS, a unary code).
//! - no compression: the samples as they are.
//!
//! The option identifier (ID) has 3, 4 or 5 bits for `n` up to 8, 16 and 32 (Table 5-1). The restricted set for `n` up to 4
//! (clause 5.1.2) has shorter IDs and fewer options. It is a separate setting, because a stream that uses it is not readable
//! with the basic set.
//!
//! The decoder writes into the caller's slice and does not allocate. It returns the bits it used, so a caller with several
//! coded lines in one buffer finds the next line without a search.
//!
//! ```
//! use sdrkit_proto_ccsds::rice::{decode, encode, Params};
//! let p = Params::new(8, 16, 4).unwrap();
//! let line: Vec<u32> = (0..100).map(|i| (100 + (i % 7) * 3) as u32).collect();
//! let mut coded = Vec::new();
//! encode(&line, &p, &mut coded).unwrap();
//! let mut out = vec![0u32; 100];
//! let d = decode(&coded, &p, &mut out).unwrap();
//! assert_eq!(out, line);
//! assert_eq!(d.samples, 100);
//! assert!(coded.len() < 100);
//! ```

use crate::{Error, HeaderField, Param, Result};
use sdrkit_util_bits::{BitReader, BitWriter};

/// The coding parameters.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Params {
    /// Bits per sample `n`, 1 to 32.
    pub bits: u32,
    /// Samples per block `J`: 8, 16, 32 or 64.
    pub block: usize,
    /// Blocks per sample interval (RSI), 1 to 4096.
    pub rsi: usize,
    /// The preprocessor (unit-delay predictor and mapping) is on.
    pub preprocess: bool,
    /// Samples are signed (two's complement in the low `n` bits).
    pub signed: bool,
    /// The restricted set of options for `n` up to 4 (clause 5.1.2).
    pub restricted: bool,
}

impl Params {
    /// The parameters of the VIIRS detector data of the JPSS direct broadcast: 15-bit unsigned samples, blocks of 8, a
    /// sample interval of 128 blocks, preprocessor on. The anchor sample of each interval follows the option identifier in
    /// the bit stream, without byte alignment.
    pub const VIIRS: Params = Params {
        bits: 15,
        block: 8,
        rsi: 128,
        preprocess: true,
        signed: false,
        restricted: false,
    };

    /// Unsigned samples of `bits` bits, blocks of `block` samples, `rsi` blocks per interval, preprocessor on, basic option
    /// set. If a value is outside the ranges of [`Params`], it returns an error.
    pub fn new(bits: u32, block: usize, rsi: usize) -> Result<Self> {
        let p = Params {
            bits,
            block,
            rsi,
            preprocess: true,
            signed: false,
            restricted: false,
        };
        p.check()?;
        Ok(p)
    }

    /// Makes sure that the values are inside the ranges the standard allows.
    pub fn check(&self) -> Result<()> {
        if !(1..=32).contains(&self.bits) {
            return Err(Error::BadParameter(Param::RiceBitsPerSample));
        }
        if !matches!(self.block, 8 | 16 | 32 | 64) {
            return Err(Error::BadParameter(Param::RiceBlockSize));
        }
        if !(1..=4096).contains(&self.rsi) {
            return Err(Error::BadParameter(Param::RiceSampleInterval));
        }
        Ok(())
    }

    /// The length of the option identifier (Table 5-1, and clause 5.1.2 for the restricted set).
    pub fn id_len(&self) -> u32 {
        match self.bits {
            1..=2 if self.restricted => 1,
            3..=4 if self.restricted => 2,
            0..=8 => 3,
            9..=16 => 4,
            _ => 5,
        }
    }

    /// The largest split `k` the identifiers can name: `2^id_len - 3`, or `None` when the set has no split option.
    pub fn k_max(&self) -> Option<u32> {
        let idl = self.id_len();
        (idl > 1).then(|| (1u32 << idl) - 3)
    }

    fn no_compression_id(&self) -> u32 {
        (1 << self.id_len()) - 1
    }

    fn limits(&self) -> (i64, i64) {
        let n = self.bits;
        if self.signed {
            (-(1i64 << (n - 1)), (1i64 << (n - 1)) - 1)
        } else {
            (0, (1i64 << n) - 1)
        }
    }

    /// The sample value of a field of `n` bits (sign extended when signed).
    fn value(&self, raw: u32) -> i64 {
        if self.signed {
            let s = 64 - self.bits;
            ((raw as i64) << s) >> s
        } else {
            raw as i64
        }
    }

    /// The field of `n` bits of a sample value.
    fn raw(&self, v: i64) -> u32 {
        (v as u64 & ((1u64 << self.bits) - 1)) as u32
    }
}

/// The mapping of the preprocessor (CCSDS 121.0-B-3 clause 4.3): the difference between `x` and its prediction `pred`, folded
/// to a non-negative number. `min` and `max` are the sample limits.
pub fn map(x: i64, pred: i64, min: i64, max: i64) -> u64 {
    let d = x - pred;
    let theta = (pred - min).min(max - pred);
    if (0..=theta).contains(&d) {
        2 * d as u64
    } else if (-theta..0).contains(&d) {
        (2 * -d - 1) as u64
    } else {
        (theta + d.abs()) as u64
    }
}

/// The inverse of [`map`].
pub fn unmap(m: u64, pred: i64, min: i64, max: i64) -> i64 {
    let theta = (pred - min).min(max - pred);
    let m = m as i64;
    if m <= 2 * theta {
        if m % 2 == 0 {
            pred + m / 2
        } else {
            pred - (m + 1) / 2
        }
    } else if pred - min <= max - pred {
        // theta is the room below the prediction: the difference is positive.
        pred + (m - theta)
    } else {
        pred - (m - theta)
    }
}

/// What [`decode`] did.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Decoded {
    /// Samples written (fewer than requested when the data ran out).
    pub samples: usize,
    /// Bits of the input used.
    pub bits: usize,
}

impl Decoded {
    /// Bytes of the input used, with the last partial byte.
    pub fn bytes(&self) -> usize {
        self.bits.div_ceil(8)
    }
}

/// Decodes `out.len()` samples from `data`. Samples come out as `n`-bit fields (two's complement when signed). If the data runs
/// out, it returns the samples decoded so far. A last partial block is decoded in full and its padding dropped.
pub fn decode(data: &[u8], p: &Params, out: &mut [u32]) -> Result<Decoded> {
    p.check()?;
    let want = out.len();
    let mut r = BitReader::new(data);
    let j = p.block;
    let blocks = want.div_ceil(j);
    let idl = p.id_len();
    let nocomp = p.no_compression_id();
    let mut written = 0usize; // mapped values written (padding past `want` is dropped)
    let mut good_bits = 0usize;
    let mut good_written = 0usize;
    let put = |out: &mut [u32], w: &mut usize, v: u64| {
        if *w < out.len() {
            out[*w] = v as u32;
        }
        *w += 1;
    };
    let mut b = 0usize;
    'blocks: while b < blocks {
        let in_rsi = b % p.rsi;
        let with_ref = in_rsi == 0 && p.preprocess;
        let id = r.u(idl) as u32;
        if r.overrun() {
            break;
        }
        if id == 0 && idl > 0 {
            // Low entropy: zero block or second extension (clause 5.1.4 and 5.1.5).
            let second = r.bit();
            if with_ref {
                let v = r.u(p.bits);
                put(out, &mut written, v);
            }
            if !second {
                let fs = r.unary() as usize;
                if r.overrun() {
                    break;
                }
                let rsi_start = b - in_rsi;
                let count = match fs {
                    0..=3 => fs + 1,
                    4 => {
                        // The remainder of the segment of 64 blocks (counted from the interval start), or of the interval.
                        let seg_end = rsi_start + (in_rsi / 64 + 1) * 64;
                        seg_end.min(rsi_start + p.rsi).min(blocks) - b
                    }
                    _ => fs,
                };
                if b + count > blocks.max(b + 1) && fs != 4 {
                    return Err(Error::BadField(HeaderField::RiceZeroRun));
                }
                for k in 0..count {
                    let n = if k == 0 && with_ref { j - 1 } else { j };
                    for _ in 0..n {
                        put(out, &mut written, 0);
                    }
                }
                b += count;
                if !r.overrun() {
                    good_bits = r.pos();
                    good_written = written;
                }
                continue;
            }
            let mut i = with_ref as usize;
            while i < j {
                let m = r.unary() as u64;
                if r.overrun() {
                    break 'blocks;
                }
                // m = beta (beta + 1) / 2 + d1, with beta = d0 + d1.
                let mut beta = ((((8 * m + 1) as f64).sqrt() - 1.0) / 2.0) as u64;
                while beta * (beta + 1) / 2 > m {
                    beta -= 1;
                }
                while (beta + 1) * (beta + 2) / 2 <= m {
                    beta += 1;
                }
                let d1 = m - beta * (beta + 1) / 2;
                let d0 = beta - d1;
                if i.is_multiple_of(2) {
                    put(out, &mut written, d0);
                    i += 1;
                }
                put(out, &mut written, d1);
                i += 1;
            }
        } else if id == nocomp {
            for _ in 0..j {
                let v = r.u(p.bits);
                put(out, &mut written, v);
            }
        } else {
            let k = id - 1;
            if with_ref {
                let v = r.u(p.bits);
                put(out, &mut written, v);
            }
            let n = j - with_ref as usize;
            let base = written;
            for _ in 0..n {
                let fs = r.unary() as u64;
                put(out, &mut written, fs << k);
            }
            if k > 0 {
                for w in base..base + n {
                    let low = r.u(k);
                    if w < out.len() {
                        out[w] |= low as u32;
                    }
                }
            }
        }
        if r.overrun() {
            break;
        }
        b += 1;
        good_bits = r.pos();
        good_written = written;
    }
    let samples = good_written.min(want);
    // Undo the preprocessor in place.
    if p.preprocess {
        let (min, max) = p.limits();
        let interval = p.rsi * j;
        let mut prev = 0i64;
        for (k, o) in out[..samples].iter_mut().enumerate() {
            let x = if k % interval == 0 {
                p.value(*o)
            } else {
                unmap(*o as u64, prev, min, max).clamp(min, max)
            };
            *o = p.raw(x);
            prev = x;
        }
    } else {
        let mask = ((1u64 << p.bits) - 1) as u32;
        for o in out[..samples].iter_mut() {
            *o &= mask;
        }
    }
    Ok(Decoded {
        samples,
        bits: good_bits,
    })
}

/// Appends the coded `samples` (`n`-bit fields) to `out`, choosing the shortest option for each block. The last block is padded
/// by repeating the last sample, and the output is padded to a whole byte. For generators and tests (it allocates a little).
pub fn encode(samples: &[u32], p: &Params, out: &mut Vec<u8>) -> Result<()> {
    p.check()?;
    if samples.is_empty() {
        return Ok(());
    }
    let j = p.block;
    let (min, max) = p.limits();
    let blocks = samples.len().div_ceil(j);
    let interval = p.rsi * j;
    let mut mapped: Vec<u64> = Vec::with_capacity(blocks * j);
    let mut prev = 0i64;
    for i in 0..blocks * j {
        let x = p.value(samples[i.min(samples.len() - 1)]);
        let m = if !p.preprocess || i % interval == 0 {
            p.raw(x) as u64
        } else {
            map(x, prev, min, max)
        };
        mapped.push(m);
        prev = x;
    }
    let idl = p.id_len();
    let mut w = BitWriter::new(out);
    let mut b = 0usize;
    while b < blocks {
        let in_rsi = b % p.rsi;
        let with_ref = in_rsi == 0 && p.preprocess;
        let blk = &mapped[b * j..(b + 1) * j];
        let vals = if with_ref { &blk[1..] } else { blk };
        if vals.iter().all(|&v| v == 0) {
            // A run of zero blocks, inside one segment of 64 blocks and one interval.
            let rsi_start = b - in_rsi;
            let seg_end = (rsi_start + (in_rsi / 64 + 1) * 64)
                .min(rsi_start + p.rsi)
                .min(blocks);
            let mut run = 1;
            while b + run < seg_end
                && mapped[(b + run) * j..(b + run + 1) * j]
                    .iter()
                    .all(|&v| v == 0)
            {
                run += 1;
            }
            w.put(0, idl);
            w.put(0, 1);
            if with_ref {
                w.put(blk[0], p.bits);
            }
            let fs = if b + run == seg_end && run >= 5 {
                4
            } else if run <= 4 {
                run - 1
            } else {
                run
            };
            put_fs(&mut w, fs as u64);
            b += run;
            continue;
        }
        // Costs of the options in bits, after the identifier and the anchor sample.
        let raw_cost = vals.len() as u64 * p.bits as u64;
        let mut best = (raw_cost, Choice::Raw);
        if let Some(kmax) = p.k_max() {
            for k in 0..=kmax.min(p.bits) {
                let cost: u64 = vals.iter().map(|&v| (v >> k) + 1 + k as u64).sum();
                if cost < best.0 {
                    best = (cost, Choice::Split(k));
                }
            }
        }
        let se_cost = second_extension(blk, with_ref, |_| {});
        if se_cost.saturating_add(1) < best.0 {
            best = (se_cost + 1, Choice::Second);
        }
        match best.1 {
            Choice::Raw => {
                w.put(p.no_compression_id() as u64, idl);
                for &v in blk {
                    w.put(v, p.bits);
                }
            }
            Choice::Split(k) => {
                w.put(k as u64 + 1, idl);
                if with_ref {
                    w.put(blk[0], p.bits);
                }
                for &v in vals {
                    put_fs(&mut w, v >> k);
                }
                if k > 0 {
                    for &v in vals {
                        w.put(v & ((1 << k) - 1), k);
                    }
                }
            }
            Choice::Second => {
                w.put(0, idl);
                w.put(1, 1);
                if with_ref {
                    w.put(blk[0], p.bits);
                }
                second_extension(blk, with_ref, |g| put_fs(&mut w, g));
            }
        }
        b += 1;
    }
    w.align();
    Ok(())
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Choice {
    Raw,
    Split(u32),
    Second,
}

/// Calls `emit` with the second-extension value of each pair and returns the bits they take. With an anchor sample (clause 5.1.5), the
/// place of the anchor sample counts as 0.
fn second_extension(blk: &[u64], with_ref: bool, mut emit: impl FnMut(u64)) -> u64 {
    let mut bits = 0u64;
    for pair in blk.chunks(2) {
        let a = if with_ref && std::ptr::eq(pair.as_ptr(), blk.as_ptr()) {
            0
        } else {
            pair[0]
        };
        let b = pair.get(1).copied().unwrap_or(0);
        let s = a.saturating_add(b);
        let g = s.saturating_mul(s.saturating_add(1)) / 2 + b;
        bits = bits.saturating_add(g.saturating_add(1));
        emit(g);
    }
    bits
}

fn put_fs(w: &mut BitWriter<'_>, v: u64) {
    let mut v = v;
    while v >= 32 {
        w.put(0, 32);
        v -= 32;
    }
    w.put(1, v as u32 + 1);
}
