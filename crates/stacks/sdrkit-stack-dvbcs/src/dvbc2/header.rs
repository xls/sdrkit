//! The Reed-Muller coded headers: the preamble header (L1_INFO_SIZE and L1_TI_MODE) and the FECFrame header of type 2 data
//! slices. The 16 bits are coded by RM(2,5) (`sdrkit-algo-fec`) into the upper branch `lambda`; the lower branch is the code
//! word delayed cyclically by 2 (u_((i+2) mod 32) = lambda_i) and scrambled with the word w^RM. The robust form sends 32 QPSK cells (`lambda_i` on the real axis,
//! `v_i` on the imaginary axis); the high efficiency form 16 16-QAM cells (the upper branch on the two most significant bits).
//!
//! Standard: ETSI EN 302 769 V1.3.1 clauses 7.2.1 to 7.2.4 and 8.2.

use sdrkit_algo_dsp::constellation::{PointSet, QamMap};
use sdrkit_algo_fec::reedmuller::{rm25_encode, Rm25Decoder};
use sdrkit_algo_modem::{Constellation, Demap};
use sdrkit_core::Complex32;

/// The two branches of a header scrambled with `w_rm`: (lambda, v), code bit `j` at bit `31 - j`.
pub fn branches(bits: u16, w_rm: u32) -> (u32, u32) {
    let lambda = rm25_encode(bits);
    // u_i = lambda_((i - 2) mod 32): a right rotation by 2 in this bit order.
    (lambda, lambda.rotate_right(2) ^ w_rm)
}

/// Code bit `i` (0 to 31) of a word whose bit 0 is the most significant.
fn bit_of(x: u32, i: usize) -> u8 {
    ((x >> (31 - i)) & 1) as u8
}

/// The constellation of a robust (QPSK) or high efficiency (16-QAM) header.
pub fn constellation(robust: bool) -> Constellation {
    let set = PointSet::qam(if robust { 4 } else { 16 }, QamMap::DVB)
        .unwrap_or_else(|_| unreachable!("QPSK and 16-QAM exist"));
    Constellation::new(set).unwrap_or_else(|_| unreachable!("square QAM is a valid constellation"))
}

/// The cells of a header scrambled with `w_rm`: 32 (robust) or 16 (high efficiency).
pub fn cells(bits: u16, robust: bool, cons: &Constellation, w_rm: u32, out: &mut Vec<Complex32>) {
    let (l, v) = branches(bits, w_rm);
    let bit = |x: u32, j: usize| usize::from(bit_of(x, j));
    out.clear();
    let p = cons.points();
    if robust {
        out.extend((0..32).map(|i| p[(bit(l, i) << 1) | bit(v, i)]));
    } else {
        out.extend((0..16).map(|i| {
            p[(bit(l, 2 * i) << 3) | (bit(l, 2 * i + 1) << 2) | (bit(v, 2 * i) << 1) | bit(v, 2 * i + 1)]
        }));
    }
}

/// The cells of a header form.
pub fn header_cells(robust: bool) -> usize {
    if robust {
        32
    } else {
        16
    }
}

/// A decoded header.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Decoded {
    /// The 16 bits.
    pub bits: u16,
    /// Code bits (of 32) whose combined soft value agrees with the decoded code word.
    pub agree: u32,
}

/// Soft decoding of headers. It allocates nothing after [`HeaderDecoder::new`].
#[derive(Clone, Debug)]
pub struct HeaderDecoder {
    rm: Rm25Decoder,
    robust: Constellation,
    efficient: Constellation,
    w_rm: u32,
    llr: [f32; 64],
}

impl HeaderDecoder {
    /// The decoder of headers scrambled with `w_rm`.
    pub fn new(w_rm: u32) -> Self {
        HeaderDecoder {
            w_rm,
            rm: Rm25Decoder::new(),
            robust: constellation(true),
            efficient: constellation(false),
            llr: [0.0; 64],
        }
    }

    /// The soft bits of both branches of the header cells `z` with weights `w` (`|H|^2 / sigma^2`): `upper[j]`, `lower[j]` for
    /// code bit `j`, positive means 0.
    fn branches_soft(&mut self, z: &[Complex32], w: &[f32], robust: bool) -> Option<([f32; 32], [f32; 32])> {
        let n = header_cells(robust);
        if z.len() < n || w.len() < n {
            return None;
        }
        let cons = if robust { &self.robust } else { &self.efficient };
        let bits = cons.bits();
        cons.demap_block_weighted(&z[..n], &w[..n], Demap::MaxLog, &mut self.llr[..n * bits])
            .ok()?;
        let (mut up, mut low) = ([0f32; 32], [0f32; 32]);
        for i in 0..n {
            let c = &self.llr[i * bits..(i + 1) * bits];
            if robust {
                up[i] = c[0];
                low[i] = c[1];
            } else {
                up[2 * i] = c[0];
                up[2 * i + 1] = c[1];
                low[2 * i] = c[2];
                low[2 * i + 1] = c[3];
            }
        }
        Some((up, low))
    }

    /// The candidate measure of a header at the start of `z`: the mean of `sign(lambda_((i - 2) mod 32)) sign(u_i)` over the hard
    /// decisions, where `u_i = v_i xor w^RM_i` (1 for a header without errors, near 0 for other cells).
    pub fn candidate(&mut self, z: &[Complex32], w: &[f32], robust: bool) -> f32 {
        let Some((up, low)) = self.branches_soft(z, w, robust) else {
            return 0.0;
        };
        let mut s = 0i32;
        for i in 0..32 {
            let u = (low[i] < 0.0) as u8 ^ bit_of(self.w_rm, i);
            let l = (up[(i + 30) % 32] < 0.0) as u8;
            s += if u == l { 1 } else { -1 };
        }
        s as f32 / 32.0
    }

    /// Decodes the header at the start of `z`: both branches combined (the lower descrambled and moved back by 2), then the
    /// soft Reed-Muller decoder. `None` when the cells are too few or not finite.
    pub fn decode(&mut self, z: &[Complex32], w: &[f32], robust: bool) -> Option<Decoded> {
        let (up, low) = self.branches_soft(z, w, robust)?;
        let mut soft = [0f32; 32];
        for i in 0..32 {
            let l = if bit_of(self.w_rm, i) == 1 { -low[i] } else { low[i] };
            let j = (i + 30) % 32;
            soft[j] = up[j] + l;
        }
        let (bits, _) = self.rm.decode(&soft).ok()?;
        let cw = rm25_encode(bits);
        let agree = (0..32)
            .filter(|&j| ((cw >> (31 - j)) & 1 == 1) == (soft[j] < 0.0))
            .count() as u32;
        Some(Decoded { bits, agree })
    }
}

/// The fields of a FECFrame header (clause 7.2.1).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FecFrameHeader {
    /// PLP_ID.
    pub plp_id: u8,
    /// PLP_FEC_TYPE: 64K frames.
    pub long: bool,
    /// PLP_MOD (000 a stuffing packet).
    pub plp_mod: u8,
    /// PLP_COD.
    pub plp_cod: u8,
    /// HEADER_COUNTER: two XFECFrames follow.
    pub two: bool,
}

impl FecFrameHeader {
    /// The 16 bits.
    pub fn bits(&self) -> u16 {
        (u16::from(self.plp_id) << 8)
            | (u16::from(self.long) << 7)
            | (u16::from(self.plp_mod & 7) << 4)
            | (u16::from(self.plp_cod & 7) << 1)
            | u16::from(self.two)
    }
    /// The fields of 16 bits.
    pub fn from_bits(b: u16) -> Self {
        FecFrameHeader {
            plp_id: (b >> 8) as u8,
            long: (b >> 7) & 1 == 1,
            plp_mod: ((b >> 4) & 7) as u8,
            plp_cod: ((b >> 1) & 7) as u8,
            two: b & 1 == 1,
        }
    }
}

/// The 16 bits of a preamble header: L1_INFO_SIZE (14 bits) and L1_TI_MODE (2 bits).
pub fn preamble_bits(info_size: u16, ti_mode: u8) -> u16 {
    ((info_size & 0x3FFF) << 2) | u16::from(ti_mode & 3)
}

/// L1_INFO_SIZE and L1_TI_MODE of a preamble header.
pub fn preamble_fields(b: u16) -> (u16, u8) {
    (b >> 2, (b & 3) as u8)
}
