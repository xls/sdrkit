//! LTE rate matching (3GPP TS 36.212 clause 5.1.4): sub-block interleaving of the three code streams, the circular buffer, and
//! bit selection with redundancy versions; and the inverse, which adds received soft values into the streams (soft combining of
//! repeats and of retransmissions).

use crate::error::{FecError, Param, Result};
use crate::llr::Llr8;
use sdrkit_util_bits::interleave::{LTE_SUBBLOCK_CONV, LTE_SUBBLOCK_TURBO};

const NULL: u32 = u32::MAX;

/// Which code the rate matching is for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LteCode {
    /// The turbo code (clause 5.1.4.1): streams of `K + 4`, the parity streams interlaced in the buffer.
    Turbo,
    /// The tail-biting convolutional code (clause 5.1.4.2): streams of `K`, concatenated.
    Convolutional,
}

/// The rate matcher for one stream length and code.
///
/// ```
/// use sdrkit_algo_fec::ratematch::{LteCode, LteRateMatch};
/// // LTE PBCH: 40 information bits, tail-biting code, 1920 bits after rate matching.
/// let rm = LteRateMatch::new(LteCode::Convolutional, 40, 0).unwrap();
/// let coded = [1u8; 120];
/// let mut e = vec![0u8; 1920];
/// rm.rate_match(&coded, 0, rm.buffer_len(), &mut e).unwrap();
/// let llr: Vec<i8> = e.iter().map(|&b| if b == 0 { 4 } else { -4 }).collect();
/// let mut soft = [0i8; 120];
/// rm.dematch(&llr, 0, rm.buffer_len(), &mut soft).unwrap();
/// assert!(soft.iter().all(|&v| v == -64)); // 16 repeats of -4 combined
/// ```
#[derive(Clone, Debug)]
pub struct LteRateMatch {
    code: LteCode,
    d: usize,
    r: usize,
    /// Circular buffer position -> index into the concatenated streams, or NULL.
    map: Vec<u32>,
}

impl LteRateMatch {
    /// The rate matcher for streams of `d` bits each (`K + 4` for turbo, `K` for the convolutional code), with `filler` filler
    /// bits at the start of a turbo code block (they and their first-parity bits are never sent).
    pub fn new(code: LteCode, d: usize, filler: usize) -> Result<Self> {
        Param::StreamLen.check(d, 1, 1 << 16)?;
        if filler >= d {
            return Param::Filler.conflict(Param::StreamLen);
        }
        let r = d.div_ceil(32);
        let kpi = 32 * r;
        let nd = kpi - d;
        let pattern = match code {
            LteCode::Turbo => &LTE_SUBBLOCK_TURBO,
            LteCode::Convolutional => &LTE_SUBBLOCK_CONV,
        };
        // v^(i)_k for the column-permuted streams: position y in the R x 32 matrix (dummies first).
        let sub = |stream: usize, k: usize| -> u32 {
            let y = if code == LteCode::Turbo && stream == 2 {
                (LTE_SUBBLOCK_TURBO[k / r] as usize + 32 * (k % r) + 1) % kpi
            } else {
                (k % r) * 32 + pattern[k / r] as usize
            };
            if y < nd {
                return NULL;
            }
            let idx = y - nd;
            if code == LteCode::Turbo && stream < 2 && idx < filler {
                return NULL;
            }
            (stream * d + idx) as u32
        };
        let mut map = Vec::with_capacity(3 * kpi);
        match code {
            LteCode::Turbo => {
                for k in 0..kpi {
                    map.push(sub(0, k));
                }
                for k in 0..kpi {
                    map.push(sub(1, k));
                    map.push(sub(2, k));
                }
            }
            LteCode::Convolutional => {
                for s in 0..3 {
                    for k in 0..kpi {
                        map.push(sub(s, k));
                    }
                }
            }
        }
        Ok(LteRateMatch { code, d, r, map })
    }

    /// The circular buffer length `K_w = 3 K_Π`.
    pub fn buffer_len(&self) -> usize {
        self.map.len()
    }

    /// Stream length `d`.
    pub fn stream_len(&self) -> usize {
        self.d
    }

    /// The starting position `k0` for redundancy version `rv` and soft buffer size `n_cb` (turbo; 0 for the convolutional code).
    pub fn k0(&self, rv: u8, n_cb: usize) -> usize {
        match self.code {
            LteCode::Turbo => self.r * (2 * n_cb.div_ceil(8 * self.r) * rv as usize + 2),
            LteCode::Convolutional => 0,
        }
    }

    fn check(&self, rv: u8, n_cb: usize, streams: usize) -> Result<()> {
        if streams != 3 * self.d {
            return Err(FecError::InputLength {
                expected: 3 * self.d,
                got: streams,
            });
        }
        Param::Rv.check(rv, 0, 3)?;
        Param::BufferLen.check(n_cb, 1, self.map.len() as i64)?;
        if self.map[..n_cb].iter().all(|&m| m == NULL) {
            return Param::BufferLen.degenerate();
        }
        Ok(())
    }

    /// Selects `e.len()` bits from the three streams (`d0 | d1 | d2`) for redundancy version `rv` with soft buffer `n_cb`.
    pub fn rate_match(&self, streams: &[u8], rv: u8, n_cb: usize, e: &mut [u8]) -> Result<()> {
        self.check(rv, n_cb, streams.len())?;
        let mut j = self.k0(rv, n_cb);
        for o in e.iter_mut() {
            loop {
                let m = self.map[j % n_cb];
                j += 1;
                if m != NULL {
                    *o = streams[m as usize];
                    break;
                }
            }
        }
        Ok(())
    }

    /// Adds the received LLRs `e` (as sent by [`LteRateMatch::rate_match`] with the same `rv` and `n_cb`) into `streams`
    /// (`d0 | d1 | d2`, saturating at ±127), so repeats and retransmissions combine. Zero `streams` before the first call.
    pub fn dematch(&self, e: &[Llr8], rv: u8, n_cb: usize, streams: &mut [Llr8]) -> Result<()> {
        self.check(rv, n_cb, streams.len())?;
        let mut j = self.k0(rv, n_cb);
        for &v in e {
            loop {
                let m = self.map[j % n_cb];
                j += 1;
                if m != NULL {
                    let s = &mut streams[m as usize];
                    *s = crate::llr::add(*s, v.max(-127));
                    break;
                }
            }
        }
        Ok(())
    }
}
