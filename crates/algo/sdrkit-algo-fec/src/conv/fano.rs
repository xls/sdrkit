//! Sequential decoding with the Fano algorithm, for long constraint lengths where a Viterbi trellis is too large: the K = 32
//! rate 1/2 code of WSPR, JT4 and JT9 (`codes::WSPR_K32`).
//!
//! The decoder walks the code tree with the Fano metric `log2(P(c | r) / P(c)) − R` per code bit, and a cycle limit bounds its work.
//!
//! Source: R. Fano, "A heuristic discussion of probabilistic decoding", IEEE Trans. Inf. Theory 9, 1963; R. Johannesson,
//! K. Zigangirov, *Fundamentals of Convolutional Coding*, chapter 6.

use super::ConvCode;
use crate::error::{Decoded, FecError, Param, Result};

/// Tuning of the Fano decoder.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FanoParams {
    /// Metric units per bit of information (the Fano metric is scaled by this and rounded).
    pub scale: f32,
    /// Threshold step, in metric units.
    pub delta: i32,
    /// Most decoder cycles per input bit before giving up.
    pub max_cycles_per_bit: u32,
}

impl Default for FanoParams {
    /// Scale 16, delta 32 (two bits), 10 000 cycles per bit.
    fn default() -> Self {
        FanoParams {
            scale: 16.0,
            delta: 32,
            max_cycles_per_bit: 10_000,
        }
    }
}

#[derive(Clone, Copy, Debug, Default)]
struct Node {
    gamma: i32,
    state: u32,
    tm: [i32; 2],
    bit: [u8; 2],
    i: u8,
    branches: u8,
}

/// A Fano sequential decoder for a terminated code (`K − 1` zero tail bits), sized at construction.
///
/// ```
/// use sdrkit_algo_fec::conv::{codes, Encoder, Fano, FanoParams};
/// let bits: Vec<u8> = (0..50).map(|i| (i * 11 % 7 > 3) as u8).collect();
/// let mut coded = Vec::new();
/// let mut enc = Encoder::new(codes::WSPR_K32);
/// enc.encode(&bits, &mut coded);
/// enc.terminate(&mut coded);
/// let mut llr: Vec<f32> = coded.iter().map(|&b| if b == 0 { 3.0 } else { -3.0 }).collect();
/// for i in [5, 40, 90, 140] {
///     llr[i] = -llr[i];
/// }
/// let mut f = Fano::new(codes::WSPR_K32, 81, FanoParams::default()).unwrap();
/// let mut out = [0u8; 50];
/// f.decode(&llr, &mut out).unwrap();
/// assert_eq!(&out[..], &bits[..]);
/// ```
#[derive(Clone, Debug)]
pub struct Fano {
    code: ConvCode,
    params: FanoParams,
    max_steps: usize,
    nodes: Vec<Node>,
    /// Fano metric of each received value for code bit 0 and 1.
    m0: Vec<i32>,
    m1: Vec<i32>,
}

impl Fano {
    /// A decoder for `code`, for frames of up to `max_steps` steps (information bits plus the `K − 1` tail bits).
    pub fn new(code: ConvCode, max_steps: usize, params: FanoParams) -> Result<Self> {
        if !(params.scale > 0.0 && params.scale.is_finite()) {
            return Param::Scale.degenerate();
        }
        Param::Delta.check(params.delta, 1, i64::MAX)?;
        Param::Cycles.check(params.max_cycles_per_bit, 1, i64::MAX)?;
        let n = code.n();
        Ok(Fano {
            code,
            params,
            max_steps,
            nodes: vec![Node::default(); max_steps + 1],
            m0: vec![0; max_steps * n],
            m1: vec![0; max_steps * n],
        })
    }

    fn node_metrics(&mut self, j: usize, tail: bool) {
        let n = self.code.n();
        let k = self.code.k();
        let st = self.nodes[j].state;
        let mut tm = [0i32; 2];
        for (b, t) in tm.iter_mut().enumerate() {
            let reg = ((b as u32) << (k - 1)) | st;
            let lab = self.code.label(reg);
            let mut s = 0i32;
            for i in 0..n {
                s += if (lab >> i) & 1 == 0 {
                    self.m0[j * n + i]
                } else {
                    self.m1[j * n + i]
                };
            }
            *t = s;
        }
        let nd = &mut self.nodes[j];
        if tail {
            nd.tm = [tm[0], i32::MIN / 2];
            nd.bit = [0, 1];
            nd.branches = 1;
        } else if tm[0] >= tm[1] {
            nd.tm = tm;
            nd.bit = [0, 1];
            nd.branches = 2;
        } else {
            nd.tm = [tm[1], tm[0]];
            nd.bit = [1, 0];
            nd.branches = 2;
        }
        nd.i = 0;
    }

    /// Decodes `llr` (`f32` LLRs, positive means 0, `n` per step, including the tail) into `out` (the information bits, `steps −
    /// (K − 1)` of them). `metric` in the result is the final path metric; `iterations` the cycles used. `Err(Uncorrectable)`
    /// when the cycle limit is reached.
    pub fn decode(&mut self, llr: &[f32], out: &mut [u8]) -> Result<Decoded> {
        let n = self.code.n();
        let k = self.code.k() as usize;
        if !llr.len().is_multiple_of(n) {
            return Err(FecError::InputLength {
                expected: n,
                got: llr.len(),
            });
        }
        let steps = llr.len() / n;
        if steps > self.max_steps {
            return Err(FecError::TooLong {
                max: self.max_steps,
            });
        }
        if steps < k {
            return Err(FecError::InputLength {
                expected: k * n,
                got: llr.len(),
            });
        }
        crate::llr::check_finite(llr)?;
        let info = steps - (k - 1);
        if out.len() < info {
            return Err(FecError::OutputTooShort { need: info });
        }
        // Fano metric per code bit: log2(2 P(c | L)) - R, with P(0 | L) = 1 / (1 + e^-L); R = 1/n.
        let rate = 1.0 / n as f32;
        let sc = self.params.scale;
        for (i, &l) in llr.iter().enumerate() {
            let l = l.clamp(-30.0, 30.0);
            let p0 = 1.0 - (1.0 + (-l).exp()).log2() - rate;
            let p1 = 1.0 - (1.0 + l.exp()).log2() - rate;
            self.m0[i] = (p0 * sc).round() as i32;
            self.m1[i] = (p1 * sc).round() as i32;
        }
        let delta = self.params.delta;
        let max_cycles = self.params.max_cycles_per_bit as u64 * steps as u64;
        self.nodes[0].gamma = 0;
        self.nodes[0].state = 0;
        self.node_metrics(0, info == 0);
        let smask = super::state_mask(k as u32);
        let mut np = 0usize;
        let mut t = 0i32;
        let mut cycles = 0u64;
        loop {
            cycles += 1;
            if cycles > max_cycles {
                return Err(FecError::Uncorrectable);
            }
            let nd = self.nodes[np];
            let ngamma = nd.gamma.saturating_add(nd.tm[nd.i as usize]);
            if ngamma >= t {
                if nd.gamma < t.saturating_add(delta) {
                    // First visit: tighten the threshold as far as the new metric allows.
                    while ngamma >= t.saturating_add(delta) {
                        t += delta;
                    }
                }
                let bit = nd.bit[nd.i as usize] as u32;
                let next = (((bit << (k - 1)) | nd.state) >> 1) & smask;
                np += 1;
                self.nodes[np].gamma = ngamma;
                self.nodes[np].state = next;
                if np == steps {
                    break;
                }
                self.node_metrics(np, np >= info);
                continue;
            }
            // Cannot go forward: look back.
            loop {
                if np == 0 || self.nodes[np - 1].gamma < t {
                    t -= delta;
                    self.nodes[np].i = 0;
                    break;
                }
                np -= 1;
                let nd = &mut self.nodes[np];
                if nd.i == 0 && nd.branches == 2 {
                    nd.i = 1;
                    break;
                }
            }
        }
        for (j, o) in out[..info].iter_mut().enumerate() {
            let nd = &self.nodes[j];
            *o = nd.bit[nd.i as usize];
        }
        // Re-encode to count the received hard decisions that disagree.
        let mut enc = super::Encoder::new(self.code);
        let mut corrected = 0u32;
        for j in 0..steps {
            let b = if j < info { out[j] } else { 0 };
            let lab = enc.push_bit(b);
            for i in 0..n {
                let l = llr[j * n + i];
                if l != 0.0 && ((l < 0.0) as u8) != (lab >> i) & 1 {
                    corrected += 1;
                }
            }
        }
        Ok(Decoded {
            corrected,
            metric: self.nodes[steps].gamma,
            iterations: cycles.min(u32::MAX as u64) as u32,
        })
    }
}
