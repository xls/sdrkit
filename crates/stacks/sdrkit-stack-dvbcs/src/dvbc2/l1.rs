//! L1 signalling part 2: its fields (table 18), its CRC-32, and its coding: BCH and the shortened and punctured 16K LDPC code
//! of rate 1/2, the 8-column bit interleaver, the 16-QAM demultiplexer and cells, and the time interleaving over the preamble
//! symbols.
//!
//! Standard: ETSI EN 302 769 V1.3.1 clauses 8.3 to 8.5 and annex E.

use super::signal::Guard;
use super::tables::{self, Qam, L1_PAD_ORDER, L1_PUNCTURE_ORDER};
use sdrkit_algo_dsp::constellation::{PointSet, QamMap};
use sdrkit_algo_fec::bch::Bch;
use sdrkit_algo_fec::bitcells::BitCells;
use sdrkit_algo_fec::ldpc::{LdpcCode, LdpcDecoder, MinSum};
use sdrkit_algo_fec::llr::{Llr8, Quantizer};
use sdrkit_algo_fec::shorten::{punctured_positions, shortened_positions};
use sdrkit_algo_fec::twistblock::TwistedBlock;
use sdrkit_algo_modem::{Constellation, Demap};
use sdrkit_core::Complex32;
use sdrkit_util_bits::crc::catalog::CRC32_MPEG2;
use sdrkit_util_bits::interleave::Permutation;
use sdrkit_util_bits::pack::{push_u64, to_u64};

/// One PLP of a data slice.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Plp {
    /// PLP_ID.
    pub id: u8,
    /// PLP_BUNDLED.
    pub bundled: bool,
    /// PLP_TYPE: 0 common, 1 grouped data, 2 normal data.
    pub plp_type: u8,
    /// PLP_PAYLOAD_TYPE: 0 GFPS, 1 GCS, 2 GSE, 3 TS.
    pub payload: u8,
    /// PLP_GROUP_ID (common and grouped data PLPs).
    pub group: u8,
    /// PLP_START (type 1 slices): the first complete XFECFrame of the frame, in cells of the slice.
    pub start: u16,
    /// PLP_FEC_TYPE (type 1 slices): 64K frames.
    pub long: bool,
    /// PLP_MOD (type 1 slices).
    pub plp_mod: u8,
    /// PLP_COD (type 1 slices).
    pub plp_cod: u8,
    /// PSI/SI_REPROCESSING.
    pub psi_reprocessing: bool,
    /// transport_stream_id (when PSI/SI_REPROCESSING is 0).
    pub ts_id: u16,
    /// The network id of the originating delivery system, ON id (when PSI/SI_REPROCESSING is 0).
    pub on_id: u16,
}

/// One data slice.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Slice {
    /// DSLICE_ID.
    pub id: u8,
    /// DSLICE_TUNE_POS, in D_X from START_FREQUENCY.
    pub tune_pos: u16,
    /// DSLICE_OFFSET_LEFT, in D_X from the tuning position.
    pub offset_left: i16,
    /// DSLICE_OFFSET_RIGHT, in D_X from the tuning position.
    pub offset_right: i16,
    /// DSLICE_TI_DEPTH: 0 none, 1 four, 2 eight, 3 sixteen symbols.
    pub ti_depth: u8,
    /// DSLICE_TYPE: type 2.
    pub type2: bool,
    /// FEC_HEADER_TYPE (type 2): the high efficiency (16-QAM) header.
    pub efficient_header: bool,
    /// DSLICE_CONST_CONF.
    pub const_conf: bool,
    /// DSLICE_LEFT_NOTCH.
    pub left_notch: bool,
    /// The PLPs.
    pub plps: Vec<Plp>,
}

impl Slice {
    /// The time interleaving depth in symbols.
    pub fn depth(&self) -> usize {
        match self.ti_depth {
            0 => 1,
            1 => 4,
            2 => 8,
            _ => 16,
        }
    }
    /// The carriers K_DS,min and K_DS,max (inclusive) for START_FREQUENCY `start` and D_X `dx`; `None` for an empty or negative
    /// slice, or one of 3408 carriers or more (clause 9.4.1).
    pub fn carriers(&self, start: usize, dx: usize) -> Option<(usize, usize)> {
        let t = i64::from(self.tune_pos);
        let a = start as i64 + dx as i64 * (t + i64::from(self.offset_left));
        let b = start as i64 + dx as i64 * (t + i64::from(self.offset_right)) - 1;
        (a >= 0 && b >= a && b - a < super::signal::BLOCK as i64).then_some((a as usize, b as usize))
    }
    /// The tuning carrier.
    pub fn tune_carrier(&self, start: usize, dx: usize) -> usize {
        start + dx * usize::from(self.tune_pos)
    }
}

/// One notch.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Notch {
    /// NOTCH_START, in D_X from START_FREQUENCY.
    pub start: u16,
    /// NOTCH_WIDTH, in D_X.
    pub width: u16,
}

impl Notch {
    /// K_N,min and K_N,max (inclusive); `None` for a notch of width 0.
    pub fn carriers(&self, start: usize, dx: usize) -> Option<(usize, usize)> {
        (self.width > 0).then(|| {
            (
                start + dx * usize::from(self.start) + 1,
                start + dx * (usize::from(self.start) + usize::from(self.width)) - 1,
            )
        })
    }
}

/// L1 signalling part 2 (table 18).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct L1Part2 {
    /// NETWORK_ID.
    pub network_id: u16,
    /// C2_SYSTEM_ID.
    pub system_id: u16,
    /// START_FREQUENCY, K_min: an absolute carrier index (24 bits).
    pub start: u32,
    /// C2_BANDWIDTH: the system is C2_BANDWIDTH x D_X + 1 carriers.
    pub bandwidth: u16,
    /// GUARD_INTERVAL.
    pub guard: Guard,
    /// C2_FRAME_LENGTH (448).
    pub frame_length: u16,
    /// L1_PART2_CHANGE_COUNTER.
    pub change_counter: u8,
    /// The data slices.
    pub slices: Vec<Slice>,
    /// The notches.
    pub notches: Vec<Notch>,
    /// RESERVED_TONE.
    pub reserved_tones: bool,
    /// EWS.
    pub ews: bool,
    /// C2_VERSION.
    pub version: u8,
}

/// Reads bits most significant first.
struct Reader<'a> {
    b: &'a [u8],
    at: usize,
}

impl Reader<'_> {
    fn take(&mut self, n: usize) -> Option<u64> {
        let s = self.b.get(self.at..self.at + n)?;
        self.at += n;
        Some(to_u64(s))
    }
    fn signed(&mut self, n: usize) -> Option<i16> {
        let v = self.take(n)? as i64;
        Some((if v >> (n - 1) & 1 == 1 { v - (1 << n) } else { v }) as i16)
    }
}

impl L1Part2 {
    /// K_min.
    pub fn k_min(&self) -> usize {
        self.start as usize
    }
    /// K_max.
    pub fn k_max(&self) -> usize {
        self.start as usize + usize::from(self.bandwidth) * self.guard.d_x()
    }

    /// The fields as bits, without the block padding bit and the CRC.
    pub fn field_bits(&self) -> Vec<u8> {
        let (wide, narrow) = self.guard.field_bits();
        let mut b = Vec::with_capacity(512);
        let p = &mut b;
        push_u64(p, u64::from(self.network_id), 16);
        push_u64(p, u64::from(self.system_id), 16);
        push_u64(p, u64::from(self.start), 24);
        push_u64(p, u64::from(self.bandwidth), 16);
        push_u64(p, u64::from(self.guard.code()), 2);
        push_u64(p, u64::from(self.frame_length), 10);
        push_u64(p, u64::from(self.change_counter), 8);
        push_u64(p, self.slices.len() as u64, 8);
        push_u64(p, self.notches.len() as u64, 4);
        let mask = |v: i16, n: u32| (v as i64 as u64) & ((1u64 << n) - 1);
        for s in &self.slices {
            push_u64(p, u64::from(s.id), 8);
            push_u64(p, u64::from(s.tune_pos), wide);
            push_u64(p, mask(s.offset_left, narrow), narrow);
            push_u64(p, mask(s.offset_right, narrow), narrow);
            push_u64(p, u64::from(s.ti_depth), 2);
            push_u64(p, u64::from(s.type2), 1);
            if s.type2 {
                push_u64(p, u64::from(s.efficient_header), 1);
            }
            push_u64(p, u64::from(s.const_conf), 1);
            push_u64(p, u64::from(s.left_notch), 1);
            push_u64(p, s.plps.len() as u64, 8);
            for q in &s.plps {
                push_u64(p, u64::from(q.id), 8);
                push_u64(p, u64::from(q.bundled), 1);
                push_u64(p, u64::from(q.plp_type), 2);
                push_u64(p, u64::from(q.payload), 5);
                if q.plp_type <= 1 {
                    push_u64(p, u64::from(q.group), 8);
                }
                if !s.type2 {
                    push_u64(p, u64::from(q.start), 14);
                    push_u64(p, u64::from(q.long), 1);
                    push_u64(p, u64::from(q.plp_mod), 3);
                    push_u64(p, u64::from(q.plp_cod), 3);
                }
                push_u64(p, u64::from(q.psi_reprocessing), 1);
                if !q.psi_reprocessing {
                    push_u64(p, u64::from(q.ts_id), 16);
                    push_u64(p, u64::from(q.on_id), 16);
                }
                push_u64(p, 0, 8);
            }
            push_u64(p, 0, 8);
        }
        for n in &self.notches {
            push_u64(p, u64::from(n.start), wide);
            push_u64(p, u64::from(n.width), narrow);
            push_u64(p, 0, 8);
        }
        push_u64(p, u64::from(self.reserved_tones), 1);
        push_u64(p, u64::from(self.ews), 1);
        push_u64(p, u64::from(self.version), 4);
        push_u64(p, 0, 11);
        b
    }

    /// L1_INFO_SIZE: half the length of the fields with the block padding bit.
    pub fn info_size(&self) -> usize {
        self.field_bits().len().div_ceil(2)
    }

    /// The fields, the block padding bit when their length is odd, and the CRC-32: K_L1part2_ex_pad bits.
    pub fn to_bits(&self) -> Vec<u8> {
        let mut b = self.field_bits();
        if b.len() % 2 == 1 {
            b.push(0);
        }
        let crc = CRC32_MPEG2.bits(&b);
        push_u64(&mut b, u64::from(crc), 32);
        b
    }

    /// Reads the fields of `b` (fields, padding and CRC: `2 info_size + 32` bits). `None` when the CRC fails or a field is out
    /// of range.
    pub fn from_bits(b: &[u8]) -> Option<L1Part2> {
        if b.len() < 32 + 104 || !CRC32_MPEG2.verify_bits(b) {
            return None;
        }
        let mut r = Reader { b: &b[..b.len() - 32], at: 0 };
        let network_id = r.take(16)? as u16;
        let system_id = r.take(16)? as u16;
        let start = r.take(24)? as u32;
        let bandwidth = r.take(16)? as u16;
        let guard = Guard::from_code(r.take(2)? as u8)?;
        let (wide, narrow) = guard.field_bits();
        let (wide, narrow) = (wide as usize, narrow as usize);
        let frame_length = r.take(10)? as u16;
        let change_counter = r.take(8)? as u8;
        let n_slices = r.take(8)? as usize;
        let n_notches = r.take(4)? as usize;
        let mut slices = Vec::with_capacity(n_slices);
        for _ in 0..n_slices {
            let mut s = Slice {
                id: r.take(8)? as u8,
                tune_pos: r.take(wide)? as u16,
                offset_left: r.signed(narrow)?,
                offset_right: r.signed(narrow)?,
                ti_depth: r.take(2)? as u8,
                type2: r.take(1)? == 1,
                ..Slice::default()
            };
            if s.type2 {
                s.efficient_header = r.take(1)? == 1;
            }
            s.const_conf = r.take(1)? == 1;
            s.left_notch = r.take(1)? == 1;
            let n_plps = r.take(8)? as usize;
            for _ in 0..n_plps {
                let mut q = Plp {
                    id: r.take(8)? as u8,
                    bundled: r.take(1)? == 1,
                    plp_type: r.take(2)? as u8,
                    payload: r.take(5)? as u8,
                    ..Plp::default()
                };
                if q.plp_type <= 1 {
                    q.group = r.take(8)? as u8;
                }
                if !s.type2 {
                    q.start = r.take(14)? as u16;
                    q.long = r.take(1)? == 1;
                    q.plp_mod = r.take(3)? as u8;
                    q.plp_cod = r.take(3)? as u8;
                }
                q.psi_reprocessing = r.take(1)? == 1;
                if !q.psi_reprocessing {
                    q.ts_id = r.take(16)? as u16;
                    q.on_id = r.take(16)? as u16;
                }
                r.take(8)?;
                s.plps.push(q);
            }
            r.take(8)?;
            slices.push(s);
        }
        let mut notches = Vec::with_capacity(n_notches);
        for _ in 0..n_notches {
            notches.push(Notch {
                start: r.take(wide)? as u16,
                width: r.take(narrow)? as u16,
            });
            r.take(8)?;
        }
        let reserved_tones = r.take(1)? == 1;
        let ews = r.take(1)? == 1;
        let version = r.take(4)? as u8;
        r.take(11)?;
        Some(L1Part2 {
            network_id,
            system_id,
            start,
            bandwidth,
            guard,
            frame_length,
            change_counter,
            slices,
            notches,
            reserved_tones,
            ews,
            version,
        })
    }
}

/// K_bch of the L1 code.
pub const L1_K_BCH: usize = 7032;
/// BCH parity bits of the L1 code.
pub const L1_BCH_PARITY: usize = 168;
/// LDPC parity bits of the L1 code.
pub const L1_LDPC_PARITY: usize = 9000;
/// The most bits of one L1 FEC block before padding.
pub const L1_MAX_BLOCK: usize = 4759;

/// The min-sum rule of the L1 decoder for K_sig signalling bits: scale 15/16 below 1 500 bits, 16/16 less 1 below 2 300
/// bits, else 14/16.
///
/// Shortening and puncturing leave few sent bits per check, and a scaled message fades along the chains of punctured
/// parity bits. Measured with `scale::l1_words` (16-QAM, a word lost when BCH fails): 15/16 is best at K_sig 332, where an
/// offset costs 0.5 dB; 16/16 less 1 gains 0.11 dB over 15/16 at 1 632; at 3 032 and 4 632 an offset gains less than
/// 0.1 dB over 14/16.
fn l1_rule(k_sig: usize) -> MinSum {
    match k_sig {
        0..1500 => MinSum::with_scale(15),
        1500..2300 => MinSum {
            scale_q4: 16,
            offset: 1,
        },
        _ => MinSum::with_scale(14),
    }
}

/// The sizes of an L1 part 2 coding (clause 8.4).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct L1Sizes {
    /// L1_INFO_SIZE.
    pub info_size: usize,
    /// L1_TI_MODE.
    pub ti_mode: u8,
    /// N_FEC_Block.
    pub blocks: usize,
    /// K_sig, signalling bits per block.
    pub k_sig: usize,
    /// N_L1part2, coded bits per block.
    pub n_l1: usize,
    /// N_punc.
    pub n_punc: usize,
    /// The preamble symbols, L_P.
    pub symbols: usize,
}

impl L1Sizes {
    /// The sizes for L1_INFO_SIZE and L1_TI_MODE; `None` for a size the code cannot carry, or more FEC blocks than the time
    /// interleaving depth of modes 10 and 11.
    pub fn new(info_size: usize, ti_mode: u8) -> Option<L1Sizes> {
        let ex_pad = 2 * info_size + 32;
        if info_size == 0 || ex_pad > 32_766 + 32 {
            return None;
        }
        let blocks = ex_pad.div_ceil(L1_MAX_BLOCK);
        let k_sig = ex_pad.div_ceil(blocks);
        let punc_temp = 6 * (L1_K_BCH - k_sig) / 5;
        let temp = k_sig + L1_BCH_PARITY + L1_LDPC_PARITY - punc_temp;
        let (d, symbols) = match ti_mode {
            0 | 1 => (blocks, blocks),
            2 => (4, 4),
            _ => (8, 8),
        };
        if blocks > symbols {
            return None;
        }
        let n_l1 = temp.div_ceil(8 * d) * 8 * d;
        let n_punc = punc_temp.checked_sub(n_l1 - temp)?;
        Some(L1Sizes {
            info_size,
            ti_mode,
            blocks,
            k_sig,
            n_l1,
            n_punc,
            symbols,
        })
    }
    /// 16-QAM cells per FEC block.
    pub fn cells_per_block(&self) -> usize {
        self.n_l1 / 4
    }
    /// L1 cells of the frame.
    pub fn total_cells(&self) -> usize {
        self.blocks * self.cells_per_block()
    }
    /// L1 cells in each preamble symbol.
    pub fn cells_per_symbol(&self) -> usize {
        match self.ti_mode {
            0 => self.cells_per_block(),
            _ => self.total_cells() / self.symbols,
        }
    }
}

/// The L1 code of one size: encoder and decoder. It allocates nothing after [`L1Codec::new`].
pub struct L1Codec {
    sizes: L1Sizes,
    bch: Bch,
    code: LdpcCode,
    dec: LdpcDecoder,
    /// Information positions of the BCH word that carry signalling bits, increasing.
    info: Vec<u32>,
    /// LDPC parity positions that are sent, increasing.
    parity: Vec<u32>,
    perm: Permutation,
    cons: Constellation,
    ti: TwistedBlock,
    word: Vec<u8>,
    cw: Vec<u8>,
    sent: Vec<u8>,
    words: Vec<u8>,
    soft: Vec<f32>,
    soft_sent: Vec<f32>,
    llr: Vec<f32>,
    llr8: Vec<Llr8>,
    hard: Vec<u8>,
    cells: Vec<Complex32>,
    weights: Vec<f32>,
}

/// The 16-QAM of the L1 cells.
fn l1_points() -> PointSet {
    PointSet::qam(16, QamMap::DVB).unwrap_or_else(|_| unreachable!("16-QAM exists"))
}

impl L1Codec {
    /// The codec for `sizes`.
    pub fn new(sizes: L1Sizes) -> Option<L1Codec> {
        let f = tables::fec(false, None);
        let bch = Bch::from_code(f.bch).ok()?;
        let code = LdpcCode::from_dvb(f.ldpc).ok()?;
        let mut short = Vec::new();
        shortened_positions(L1_K_BCH, sizes.k_sig, 360, &L1_PAD_ORDER, &mut short).ok()?;
        let info: Vec<u32> = (0..L1_K_BCH as u32)
            .filter(|p| short.binary_search(p).is_err())
            .collect();
        let mut punct = Vec::new();
        punctured_positions(L1_LDPC_PARITY, 25, &L1_PUNCTURE_ORDER, sizes.n_punc, &mut punct).ok()?;
        let parity: Vec<u32> = (0..L1_LDPC_PARITY as u32)
            .filter(|p| punct.binary_search(p).is_err())
            .collect();
        let perm = BitCells {
            n: sizes.n_l1,
            k: 0,
            parity: false,
            twist: &[0; 8],
            demux: tables::demux(Qam::Q16, false, None),
            bits_per_cell: 4,
        }
        .permutation()
        .ok()?;
        let total = sizes.total_cells();
        let rows = if sizes.ti_mode == 0 { 1 } else { sizes.symbols };
        let ti = TwistedBlock::new(rows, total / rows).ok()?;
        let n = sizes.n_l1;
        Some(L1Codec {
            dec: LdpcDecoder::new(&code, l1_rule(sizes.k_sig)),
            cons: Constellation::new(l1_points()).ok()?,
            word: vec![0; bch.n()],
            cw: vec![0; code.n()],
            sent: vec![0; n],
            words: vec![0; n],
            soft: vec![0.0; n],
            soft_sent: vec![0.0; n],
            llr: vec![0.0; code.n()],
            llr8: vec![0; code.n()],
            hard: vec![0; code.n()],
            cells: vec![Complex32::default(); total],
            weights: vec![0.0; total],
            sizes,
            bch,
            code,
            info,
            parity,
            perm,
            ti,
        })
    }

    /// The sizes.
    pub fn sizes(&self) -> &L1Sizes {
        &self.sizes
    }

    /// Encodes K_L1part2_ex_pad bits (fields, block padding bit, CRC) into the L1 cells of each preamble symbol: `out[r]` for
    /// symbol `r`. `None` on a wrong length.
    pub fn encode(&mut self, bits: &[u8], out: &mut Vec<Vec<Complex32>>) -> Option<()> {
        let s = self.sizes;
        if bits.len() != 2 * s.info_size + 32 {
            return None;
        }
        let points = l1_points();
        let p = points.points();
        let cpb = s.cells_per_block();
        for b in 0..s.blocks {
            self.word.fill(0);
            for (j, &pos) in self.info.iter().enumerate() {
                // L1 padding zeros after the bits.
                self.word[pos as usize] = bits.get(b * s.k_sig + j).copied().unwrap_or(0);
            }
            self.bch.encode(&mut self.word).ok()?;
            self.code.encode(&self.word, &mut self.cw).ok()?;
            let mut w = 0;
            for &pos in &self.info {
                self.sent[w] = self.word[pos as usize];
                w += 1;
            }
            for i in L1_K_BCH..L1_K_BCH + L1_BCH_PARITY {
                self.sent[w] = self.cw[i];
                w += 1;
            }
            let k_ldpc = self.code.k();
            for &q in &self.parity {
                self.sent[w] = self.cw[k_ldpc + q as usize];
                w += 1;
            }
            self.perm.apply(&self.sent, &mut self.words).ok()?;
            for (o, c) in self.cells[b * cpb..(b + 1) * cpb].iter_mut().zip(self.words.as_chunks::<4>().0) {
                *o = p[usize::from(c[0] << 3 | c[1] << 2 | c[2] << 1 | c[3])];
            }
        }
        out.clear();
        let per = s.cells_per_symbol();
        if s.ti_mode == 0 {
            out.extend(self.cells.chunks(per).map(<[Complex32]>::to_vec));
        } else {
            let mut rows = vec![Complex32::default(); self.cells.len()];
            self.ti.interleave(None, &self.cells, &mut rows).ok()?;
            out.extend(rows.chunks(per).map(<[Complex32]>::to_vec));
        }
        Some(())
    }

    /// Decodes the L1 cells of the preamble symbols (`cells` and `weights` in symbol order, `cells_per_symbol` each) into the
    /// K_L1part2_ex_pad bits. Returns whether every block's LDPC and BCH succeeded; the bits are written either way.
    pub fn decode(&mut self, cells: &[Complex32], weights: &[f32], bits: &mut Vec<u8>) -> Option<bool> {
        let s = self.sizes;
        let total = s.total_cells();
        if cells.len() < total || weights.len() < total {
            return None;
        }
        if s.ti_mode == 0 {
            self.cells.copy_from_slice(&cells[..total]);
            self.weights.copy_from_slice(&weights[..total]);
        } else {
            self.ti.deinterleave(None, &cells[..total], &mut self.cells).ok()?;
            self.ti.deinterleave(None, &weights[..total], &mut self.weights).ok()?;
        }
        bits.clear();
        let cpb = s.cells_per_block();
        let quant = Quantizer::default();
        let mut all_ok = true;
        for b in 0..s.blocks {
            let range = b * cpb..(b + 1) * cpb;
            self.cons
                .demap_block_weighted(&self.cells[range.clone()], &self.weights[range], Demap::MaxLog, &mut self.soft)
                .ok()?;
            self.perm.invert(&self.soft, &mut self.soft_sent).ok()?;
            self.llr.fill(0.0);
            let mut r = 0;
            for &pos in &self.info {
                self.llr[pos as usize] = self.soft_sent[r];
                r += 1;
            }
            for i in L1_K_BCH..L1_K_BCH + L1_BCH_PARITY {
                self.llr[i] = self.soft_sent[r];
                r += 1;
            }
            let k_ldpc = self.code.k();
            for &q in &self.parity {
                self.llr[k_ldpc + q as usize] = self.soft_sent[r];
                r += 1;
            }
            let scale = quant.scale_for(&self.soft_sent);
            quant.quantize(&self.llr, scale, &mut self.llr8).ok()?;
            // Shortened bits are known zeros.
            for p in 0..L1_K_BCH as u32 {
                if self.info.binary_search(&p).is_err() {
                    self.llr8[p as usize] = quant.limit;
                }
            }
            // The LDPC result does not decide: the BCH check and the CRC do.
            let _ = self.dec.decode(&self.llr8, 50, &mut self.hard);
            let n = self.bch.n();
            let bch_ok = self.bch.is_codeword(&self.hard[..n]) || self.bch.decode(&mut self.hard[..n]).is_ok();
            all_ok &= bch_ok;
            for &pos in &self.info {
                bits.push(self.hard[pos as usize]);
            }
        }
        bits.truncate(2 * s.info_size + 32);
        Some(all_ok)
    }
}

/// The min-sum rule of the L1 code:
/// `cargo test --release -p sdrkit-stack-dvbcs --lib dvbc2::l1::scale -- --ignored --nocapture`.
#[cfg(test)]
mod scale {
    use super::super::bicm::scale::{compare, header, rules};
    use super::*;
    use sdrkit_algo_sim::rng::Rng;
    use sdrkit_dev_test::fer::{self, frames, noise, Trial};

    /// One L1 word of one size with a decoder per rule.
    struct Word {
        enc: L1Codec,
        decs: Vec<L1Codec>,
        bits: Vec<u8>,
        out: Vec<u8>,
        sym: Vec<Vec<Complex32>>,
        cells: Vec<Complex32>,
        weights: Vec<f32>,
        snr_db: f32,
        seed: u64,
    }

    impl Word {
        fn new(sizes: L1Sizes) -> Word {
            let decs = rules()
                .into_iter()
                .map(|r| {
                    let mut c = L1Codec::new(sizes).expect("codec");
                    c.dec = LdpcDecoder::new(&c.code, r);
                    c
                })
                .collect();
            Word {
                enc: L1Codec::new(sizes).expect("codec"),
                decs,
                bits: vec![0; 2 * sizes.info_size + 32],
                out: Vec::new(),
                sym: Vec::new(),
                cells: Vec::new(),
                weights: vec![0.0; sizes.total_cells()],
                snr_db: 0.0,
                seed: 0,
            }
        }
    }

    impl Trial for Word {
        fn start(&mut self, db: f64, seed: u64) {
            (self.snr_db, self.seed) = (db as f32, seed);
        }
        fn send(&mut self, f: u64) {
            let seed = self.seed << 32 | f;
            Rng::new(seed).fill_bits(&mut self.bits);
            self.enc.encode(&self.bits, &mut self.sym).expect("encode");
            self.cells = self.sym.concat();
            noise(&mut self.cells, &mut self.weights, self.snr_db, !seed);
        }
        /// A word is lost when the BCH check fails or its bits are wrong.
        fn lost(&mut self, i: usize) -> bool {
            let ok = self.decs[i].decode(&self.cells, &self.weights, &mut self.out).expect("decode");
            !ok || self.out != self.bits
        }
    }

    /// The frame error rate of the L1 code at four sizes, per rule, 1000 frames a point read at 0.1 and 0.01.
    #[test]
    #[ignore = "measurement"]
    fn l1_words() {
        let frames = frames(1000);
        header("Es/N0", frames);
        let sizes: Vec<L1Sizes> = [150, 800, 1500, 2300].iter().map(|&i| L1Sizes::new(i, 0).expect("sizes")).collect();
        let lines = fer::parallel(&sizes, |&s| {
            let name = format!("{:<28}", format!("L1 info {} (K_sig {})", s.info_size, s.k_sig));
            compare(&mut Word::new(s), &name, l1_rule(s.k_sig), -4.0, frames, true)
        });
        for l in lines {
            println!("{l}");
        }
    }
}
