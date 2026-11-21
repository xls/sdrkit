//! The DVB-T2 interleavers as permutations: the bit interleaver with the cell word demultiplexer, the cell interleaver, the time
//! interleaver and the frequency interleaver. Each is built once from the formula of the standard and works both ways.
//!
//! Standard: ETSI EN 302 755 V1.4.1 clauses 6.1.3, 6.2.1, 6.4, 6.5.3 and 8.5.

use crate::error::{Rule, T2Error};
use crate::params::{CodeRate, FecFrame, Fft, Modulation};
use crate::tables;
use sdrkit_algo_fec::addrgen::AddressRule;
use sdrkit_algo_fec::bitcells::BitCells;
use sdrkit_util_bits::interleave::Permutation;

/// The bit interleaver and the demultiplexer of one PLP as one permutation: `apply` takes the N_ldpc bits of an LDPC code word
/// and returns the cell words, eta_MOD bits per cell with y_0 first (clauses 6.1.3 and 6.2.1). The receiver uses `invert` on the
/// soft bits of the cells.
///
/// The bit interleaver is parity interleaving and column twist interleaving for 16-, 64- and 256-QAM, parity interleaving only
/// for QPSK with the rates 1/3 and 2/5 (T2-Lite, table I.2), and nothing for other QPSK.
pub fn bits_to_cells(
    frame: FecFrame,
    m: Modulation,
    rate: CodeRate,
) -> Result<Permutation, T2Error> {
    let k = crate::params::ldpc_table(frame, rate)
        .ok_or(Rule::NoCode)?
        .k as usize;
    Ok(BitCells {
        n: frame.n_ldpc(),
        k,
        parity: m != Modulation::Qpsk || matches!(rate, CodeRate::R1_3 | CodeRate::R2_5),
        twist: tables::twist(frame, m).unwrap_or(&[]),
        demux: tables::demux(frame, m, rate),
        bits_per_cell: m.bits(),
    }
    .permutation()?)
}

/// The L1-post bit interleaver and demultiplexer (clauses 7.3.2.7 and 7.3.3.1): a row-column interleaver with N_c 8 (16-QAM) or
/// 12 (64-QAM) over N_post bits, then the demultiplexer of table 13(a). `apply` returns the cell words.
pub fn l1_post_bits_to_cells(n_post: usize, m: Modulation) -> Result<Permutation, T2Error> {
    let eta = m.bits();
    let nc = match m {
        Modulation::Qam16 => 8,
        Modulation::Qam64 => 12,
        _ => 1,
    };
    if !n_post.is_multiple_of(nc.max(eta)) {
        return Err(T2Error::Rule(Rule::PostNotDivisible));
    }
    const NO_TWIST: [u16; 12] = [0; 12];
    Ok(BitCells {
        n: n_post,
        k: 0,
        parity: false,
        twist: if nc > 1 { &NO_TWIST[..nc] } else { &[] },
        demux: tables::demux(FecFrame::Short, m, CodeRate::R1_2),
        bits_per_cell: eta,
    }
    .permutation()?)
}

/// The cell interleaver of one PLP (clause 6.4): L_r(q) = (L_0(q) + P(r)) mod N_cells for FEC block r of a TI-block.
#[derive(Clone, Debug)]
pub struct CellInterleaver {
    l0: Vec<u32>,
    shifts: Vec<u32>,
}

/// The most FEC blocks in a TI-block (PLP_NUM_BLOCKS is 10 bits).
const MAX_FEC_TI: usize = 1024;

impl CellInterleaver {
    /// The interleaver for `n_cells` cells per FEC block (table 17). Errors for a size without a register in clause 6.4.
    pub fn new(n_cells: usize) -> Result<CellInterleaver, T2Error> {
        let nd = usize::BITS as usize - (n_cells - 1).leading_zeros() as usize;
        let taps = tables::cell_taps(nd).ok_or(Rule::NoCellInterleaver)?;
        let l0 = address_sequence(nd, taps, None, n_cells)?;
        let mut shifts = Vec::with_capacity(MAX_FEC_TI);
        let mut k = 0u32;
        while shifts.len() < MAX_FEC_TI {
            let p = k.reverse_bits() >> (32 - nd);
            if (p as usize) < n_cells {
                shifts.push(p);
            }
            k += 1;
        }
        Ok(CellInterleaver { l0, shifts })
    }

    /// N_cells.
    pub fn len(&self) -> usize {
        self.l0.len()
    }

    /// Whether the interleaver is empty (never for a valid size).
    pub fn is_empty(&self) -> bool {
        self.l0.is_empty()
    }

    /// L_0(q).
    pub fn l0(&self) -> &[u32] {
        &self.l0
    }

    /// P(r), the shift for FEC block r of a TI-block (r below 1 024): the bit-reversed count, values of N_cells or more
    /// skipped.
    pub fn shift(&self, r: usize) -> u32 {
        self.shifts.get(r).copied().unwrap_or(0)
    }

    /// Interleaves FEC block `r` of a TI-block: `out[L_r(q)] = input[q]`.
    pub fn interleave<T: Copy>(&self, r: usize, input: &[T], out: &mut [T]) {
        let n = self.l0.len();
        let p = self.shift(r) as usize;
        for (q, &x) in input.iter().enumerate().take(n) {
            out[(self.l0[q] as usize + p) % n] = x;
        }
    }

    /// De-interleaves FEC block `r`: `out[q] = input[L_r(q)]`.
    pub fn deinterleave<T: Copy>(&self, r: usize, input: &[T], out: &mut [T]) {
        let n = self.l0.len();
        let p = self.shift(r) as usize;
        for (q, o) in out.iter_mut().enumerate().take(n) {
            *o = input[(self.l0[q] as usize + p) % n];
        }
    }
}

/// The addresses of the register of `nbits - 1` stages stepped by `taps` (clause 6.4 or 8.5), values of `limit` or more
/// skipped. `perm[j]` is the bit of R that takes bit `nbits - 2 - j` of R' (table 53 lists R' from its most significant bit);
/// `None` keeps the bits in place.
fn address_sequence(nbits: usize, taps: &[u8], perm: Option<&[u8]>, limit: usize) -> Result<Vec<u32>, T2Error> {
    let reg = nbits - 1;
    let taps: Vec<u32> = taps.iter().map(|&t| u32::from(t)).collect();
    let perm: Vec<u32> = (0..reg)
        .map(|j| perm.and_then(|q| q.get(reg - 1 - j)).map_or(j as u32, |&b| u32::from(b)))
        .collect();
    let mut out = Vec::with_capacity(limit);
    AddressRule {
        taps: &taps,
        perm: &perm,
    }
    .addresses(limit, &mut out)?;
    Ok(out)
}

/// The time interleaver of one TI-block (clause 6.5.3): N_r = N_cells / 5 rows, 5 N_FEC columns, written by columns and read by
/// rows. `out[j] = input[c N_r + r]` with `r = j div N_c`, `c = j mod N_c`.
pub fn time_interleave<T: Copy>(n_cells: usize, n_fec: usize, input: &[T], out: &mut [T]) {
    let nr = n_cells / 5;
    let nc = 5 * n_fec;
    for (j, o) in out.iter_mut().enumerate().take(nr * nc) {
        *o = input[(j % nc) * nr + j / nc];
    }
}

/// The time de-interleaver of one TI-block, the inverse of [`time_interleave`].
pub fn time_deinterleave<T: Copy>(n_cells: usize, n_fec: usize, input: &[T], out: &mut [T]) {
    let nr = n_cells / 5;
    let nc = 5 * n_fec;
    for (j, &x) in input.iter().enumerate().take(nr * nc) {
        out[(j % nc) * nr + j / nc] = x;
    }
}

/// The frequency interleaver of one FFT size (clause 8.5): the permutations H0 and H1 (H for 32K) for each number of data cells
/// in use.
#[derive(Clone, Debug)]
pub struct FreqInterleaver {
    fft: Fft,
    perms: Vec<(usize, [Permutation; 2])>,
}

impl FreqInterleaver {
    /// The interleaver of `fft` for the given data cell counts (C_P2, C_data, N_FC). Errors when a count exceeds the FFT size.
    pub fn new(fft: Fft, counts: &[usize]) -> Result<FreqInterleaver, T2Error> {
        let mut perms = Vec::new();
        for &n in counts {
            if perms.iter().any(|(c, _)| *c == n) {
                continue;
            }
            if n > fft.size() {
                return Err(T2Error::Rule(Rule::TooManyCells));
            }
            let (h0, h1) = (Self::h(fft, 0, n)?, Self::h(fft, 1, n)?);
            let mk = |dest: Vec<u32>| Permutation::from_dest(dest);
            let pair = if fft == Fft::K32 {
                // Even symbols: a_H(p) = x_p. Odd symbols: a_p = x_H(p).
                let scatter = mk(h0.clone())?;
                let mut inv = vec![0u32; n];
                for (p, &hp) in h0.iter().enumerate() {
                    inv[hp as usize] = p as u32;
                }
                [scatter, mk(inv)?]
            } else {
                // a_p = x_H(p): input H(p) goes to p.
                let gather = |h: &[u32]| {
                    let mut d = vec![0u32; n];
                    for (p, &hp) in h.iter().enumerate() {
                        d[hp as usize] = p as u32;
                    }
                    d
                };
                [mk(gather(&h0))?, mk(gather(&h1))?]
            };
            perms.push((n, pair));
        }
        Ok(FreqInterleaver { fft, perms })
    }

    /// The FFT size.
    pub fn fft(&self) -> Fft {
        self.fft
    }

    /// The permutation for symbol `l` of the frame with `n` data cells: `apply` interleaves (x to a), `invert` de-interleaves.
    pub fn for_symbol(&self, l: usize, n: usize) -> Option<&Permutation> {
        self.perms
            .iter()
            .find(|(c, _)| *c == n)
            .map(|(_, p)| &p[l % 2])
    }

    /// H(p) for the given permutation (0 is H0, 1 is H1) and data cell count. Errors when `n` exceeds the FFT size.
    pub fn h(fft: Fft, which: usize, n: usize) -> Result<Vec<u32>, T2Error> {
        address_sequence(
            fft.size().trailing_zeros() as usize,
            tables::freq_taps(fft.row()),
            Some(tables::freq_bit_permutation(fft.row(), which)),
            n,
        )
    }
}
