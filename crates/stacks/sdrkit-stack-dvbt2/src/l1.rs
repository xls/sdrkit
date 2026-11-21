//! L1 signalling: the L1-pre and L1-post fields, their CRC-32, and their coding (BCH, shortened and punctured 16K LDPC, L1-post
//! scrambling, bit interleaving, demultiplexing and constellations).
//!
//! Standard: ETSI EN 302 755 V1.4.1 clauses 7.2, 7.3 and annex F.

use crate::bicm::point_set;
use crate::error::{Rule, T2Error};
use crate::interleave::l1_post_bits_to_cells;
use crate::params::{CodeRate, FecFrame, Modulation};
use crate::tables;
use sdrkit_algo_fec::bch::{codes, Bch};
use sdrkit_algo_fec::ldpc::{tables::dvb_s2, LdpcCode, LdpcDecoder, MinSum};
use sdrkit_algo_fec::llr::{Llr8, Quantizer};
use sdrkit_algo_fec::shorten::{punctured_positions, shortened_positions};
use sdrkit_algo_modem::{Constellation, Demap};
use sdrkit_core::Complex32;
use sdrkit_util_bits::crc::catalog::CRC32_MPEG2;
use sdrkit_util_bits::interleave::Permutation;
use sdrkit_util_bits::pack::{field, push_u64};
use sdrkit_util_bits::seq::DVB_DISPERSAL;

/// The L1-pre fields (clause 7.2.2, figure 25). 200 bits with the CRC-32.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct L1Pre {
    /// TYPE: the input stream types (table 21).
    pub stream_type: u8,
    /// BWT_EXT: extended carrier mode.
    pub bwt_ext: bool,
    /// S1, as in P1.
    pub s1: u8,
    /// S2, as in P1.
    pub s2: u8,
    /// L1_REPETITION_FLAG.
    pub repetition: bool,
    /// GUARD_INTERVAL (table 22).
    pub guard: u8,
    /// PAPR (table 23).
    pub papr: u8,
    /// L1_MOD (table 24).
    pub l1_mod: u8,
    /// L1_COD (table 25).
    pub l1_cod: u8,
    /// L1_FEC_TYPE (table 26).
    pub l1_fec_type: u8,
    /// L1_POST_SIZE: the L1-post cells.
    pub l1_post_size: u32,
    /// L1_POST_INFO_SIZE: the L1-post information bits without the CRC.
    pub l1_post_info_size: u32,
    /// PILOT_PATTERN (table 27).
    pub pilot_pattern: u8,
    /// TX_ID_AVAILABILITY.
    pub tx_id_availability: u8,
    /// CELL_ID.
    pub cell_id: u16,
    /// NETWORK_ID.
    pub network_id: u16,
    /// T2_SYSTEM_ID.
    pub t2_system_id: u16,
    /// NUM_T2_FRAMES.
    pub num_t2_frames: u8,
    /// NUM_DATA_SYMBOLS.
    pub num_data_symbols: u16,
    /// REGEN_FLAG.
    pub regen_flag: u8,
    /// L1_POST_EXTENSION.
    pub post_extension: bool,
    /// NUM_RF.
    pub num_rf: u8,
    /// CURRENT_RF_IDX.
    pub current_rf_idx: u8,
    /// T2_VERSION (table 28).
    pub t2_version: u8,
    /// L1_POST_SCRAMBLED.
    pub post_scrambled: bool,
    /// T2_BASE_LITE.
    pub t2_base_lite: bool,
    /// RESERVED (4 bits).
    pub reserved: u8,
}

/// The number of L1-pre bits, CRC included.
pub const L1_PRE_BITS: usize = 200;

/// The L1-pre cells in each T2-frame, D_L1pre (clause 8.3.5).
pub const L1_PRE_CELLS: usize = 1840;

impl L1Pre {
    /// The 200 bits, CRC-32 last (annex F).
    pub fn to_bits(&self) -> Vec<u8> {
        let mut b = Vec::with_capacity(L1_PRE_BITS);
        let f: [(u64, u32); 27] = [
            (self.stream_type as u64, 8),
            (self.bwt_ext as u64, 1),
            (self.s1 as u64, 3),
            (self.s2 as u64, 4),
            (self.repetition as u64, 1),
            (self.guard as u64, 3),
            (self.papr as u64, 4),
            (self.l1_mod as u64, 4),
            (self.l1_cod as u64, 2),
            (self.l1_fec_type as u64, 2),
            (self.l1_post_size as u64, 18),
            (self.l1_post_info_size as u64, 18),
            (self.pilot_pattern as u64, 4),
            (self.tx_id_availability as u64, 8),
            (self.cell_id as u64, 16),
            (self.network_id as u64, 16),
            (self.t2_system_id as u64, 16),
            (self.num_t2_frames as u64, 8),
            (self.num_data_symbols as u64, 12),
            (self.regen_flag as u64, 3),
            (self.post_extension as u64, 1),
            (self.num_rf as u64, 3),
            (self.current_rf_idx as u64, 3),
            (self.t2_version as u64, 4),
            (self.post_scrambled as u64, 1),
            (self.t2_base_lite as u64, 1),
            (self.reserved as u64, 4),
        ];
        for (v, n) in f {
            push_u64(&mut b, v, n);
        }
        let crc = CRC32_MPEG2.bits(&b);
        push_u64(&mut b, crc as u64, 32);
        b
    }

    /// Reads the 200 bits. `None` when the CRC-32 is wrong or the slice is short.
    pub fn from_bits(b: &[u8]) -> Option<L1Pre> {
        if b.len() < L1_PRE_BITS || !CRC32_MPEG2.verify_bits(&b[..L1_PRE_BITS]) {
            return None;
        }
        let mut r = Fields { b, at: 0 };
        Some(L1Pre {
            stream_type: r.u(8) as u8,
            bwt_ext: r.u(1) == 1,
            s1: r.u(3) as u8,
            s2: r.u(4) as u8,
            repetition: r.u(1) == 1,
            guard: r.u(3) as u8,
            papr: r.u(4) as u8,
            l1_mod: r.u(4) as u8,
            l1_cod: r.u(2) as u8,
            l1_fec_type: r.u(2) as u8,
            l1_post_size: r.u(18) as u32,
            l1_post_info_size: r.u(18) as u32,
            pilot_pattern: r.u(4) as u8,
            tx_id_availability: r.u(8) as u8,
            cell_id: r.u(16) as u16,
            network_id: r.u(16) as u16,
            t2_system_id: r.u(16) as u16,
            num_t2_frames: r.u(8) as u8,
            num_data_symbols: r.u(12) as u16,
            regen_flag: r.u(3) as u8,
            post_extension: r.u(1) == 1,
            num_rf: r.u(3) as u8,
            current_rf_idx: r.u(3) as u8,
            t2_version: r.u(4) as u8,
            post_scrambled: r.u(1) == 1,
            t2_base_lite: r.u(1) == 1,
            reserved: r.u(4) as u8,
        })
    }

    /// Whether tone reservation is on in the data symbols (PAPR 0010 or 0011, table 23).
    pub fn tone_reservation(&self) -> bool {
        matches!(self.papr, 2 | 3)
    }
}

struct Fields<'a> {
    b: &'a [u8],
    at: usize,
}

impl Fields<'_> {
    fn u(&mut self, n: u32) -> u64 {
        let v = field(self.b, self.at, n).unwrap_or(0);
        self.at += n as usize;
        v
    }
}

/// One PLP of the configurable L1-post loop (clause 7.2.3.1, 89 bits).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PlpConfig {
    /// PLP_ID.
    pub plp_id: u8,
    /// PLP_TYPE (table 30): 0 common, 1 data type 1, 2 data type 2.
    pub plp_type: u8,
    /// PLP_PAYLOAD_TYPE (table 31): 3 is TS.
    pub payload_type: u8,
    /// FF_FLAG.
    pub ff_flag: bool,
    /// FIRST_RF_IDX.
    pub first_rf_idx: u8,
    /// FIRST_FRAME_IDX.
    pub first_frame_idx: u8,
    /// PLP_GROUP_ID.
    pub group_id: u8,
    /// PLP_COD (table 32).
    pub cod: u8,
    /// PLP_MOD (table 33).
    pub modulation: u8,
    /// PLP_ROTATION.
    pub rotation: bool,
    /// PLP_FEC_TYPE (table 34).
    pub fec_type: u8,
    /// PLP_NUM_BLOCKS_MAX.
    pub num_blocks_max: u16,
    /// FRAME_INTERVAL (I_JUMP).
    pub frame_interval: u8,
    /// TIME_IL_LENGTH.
    pub time_il_length: u8,
    /// TIME_IL_TYPE.
    pub time_il_type: bool,
    /// IN_BAND_A_FLAG.
    pub in_band_a: bool,
    /// IN_BAND_B_FLAG.
    pub in_band_b: bool,
    /// RESERVED_1 (11 bits).
    pub reserved_1: u16,
    /// PLP_MODE (table 35).
    pub plp_mode: u8,
    /// STATIC_FLAG.
    pub static_flag: bool,
    /// STATIC_PADDING_FLAG.
    pub static_padding_flag: bool,
}

impl PlpConfig {
    /// P_I, the T2-frames of one Interleaving Frame (clause 6.5.1).
    pub fn p_i(&self) -> usize {
        if self.time_il_type {
            self.time_il_length.max(1) as usize
        } else {
            1
        }
    }

    /// N_TI, the TI-blocks of one Interleaving Frame (0 when time interleaving is off).
    pub fn n_ti(&self) -> usize {
        if self.time_il_type {
            1
        } else {
            self.time_il_length as usize
        }
    }
}

/// The configurable L1-post fields (clause 7.2.3.1).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PostConfig {
    /// SUB_SLICES_PER_FRAME.
    pub sub_slices_per_frame: u16,
    /// AUX_CONFIG_RFU.
    pub aux_config_rfu: u8,
    /// (RF_IDX, FREQUENCY) for each RF channel.
    pub rf: Vec<(u8, u32)>,
    /// (FEF_TYPE, FEF_LENGTH, FEF_INTERVAL), present when the last bit of S2 is 1.
    pub fef: Option<(u8, u32, u8)>,
    /// The PLP loop.
    pub plps: Vec<PlpConfig>,
    /// FEF_LENGTH_MSB.
    pub fef_length_msb: u8,
    /// RESERVED_2 (30 bits).
    pub reserved_2: u32,
    /// (AUX_STREAM_TYPE, AUX_PRIVATE_CONF) for each auxiliary stream.
    pub aux: Vec<(u8, u32)>,
}

/// One PLP of the dynamic L1-post loop (clause 7.2.3.2).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PlpDynamic {
    /// PLP_ID.
    pub plp_id: u8,
    /// PLP_START: the first cell address.
    pub plp_start: u32,
    /// PLP_NUM_BLOCKS.
    pub num_blocks: u16,
    /// RESERVED_2.
    pub reserved_2: u8,
}

/// The dynamic L1-post fields of one T2-frame (clause 7.2.3.2).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PostDynamic {
    /// FRAME_IDX.
    pub frame_idx: u8,
    /// SUB_SLICE_INTERVAL.
    pub sub_slice_interval: u32,
    /// TYPE_2_START.
    pub type_2_start: u32,
    /// L1_CHANGE_COUNTER.
    pub l1_change_counter: u8,
    /// START_RF_IDX.
    pub start_rf_idx: u8,
    /// RESERVED_1.
    pub reserved_1: u8,
    /// The PLP loop.
    pub plps: Vec<PlpDynamic>,
    /// RESERVED_3.
    pub reserved_3: u8,
    /// AUX_PRIVATE_DYN (48 bits) for each auxiliary stream.
    pub aux: Vec<u64>,
}

/// The whole L1-post signalling of one T2-frame (clauses 7.2.3.1 to 7.2.3.5).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct L1Post {
    /// Configurable part.
    pub config: PostConfig,
    /// Dynamic part of this frame.
    pub dynamic: PostDynamic,
    /// Dynamic part of the next frame, when L1_REPETITION_FLAG is 1.
    pub next: Option<PostDynamic>,
    /// The L1-post extension field bits, when L1_POST_EXTENSION is 1.
    pub extension: Vec<u8>,
}

impl L1Post {
    /// The information bits without the CRC (L1_POST_INFO_SIZE of them).
    pub fn info_bits(&self) -> Vec<u8> {
        let c = &self.config;
        let mut b = Vec::new();
        push_u64(&mut b, c.sub_slices_per_frame as u64, 15);
        push_u64(&mut b, c.plps.len() as u64, 8);
        push_u64(&mut b, c.aux.len() as u64, 4);
        push_u64(&mut b, c.aux_config_rfu as u64, 8);
        for &(idx, f) in &c.rf {
            push_u64(&mut b, idx as u64, 3);
            push_u64(&mut b, f as u64, 32);
        }
        if let Some((t, len, iv)) = c.fef {
            push_u64(&mut b, t as u64, 4);
            push_u64(&mut b, len as u64, 22);
            push_u64(&mut b, iv as u64, 8);
        }
        for p in &c.plps {
            let f: [(u64, u32); 21] = [
                (p.plp_id as u64, 8),
                (p.plp_type as u64, 3),
                (p.payload_type as u64, 5),
                (p.ff_flag as u64, 1),
                (p.first_rf_idx as u64, 3),
                (p.first_frame_idx as u64, 8),
                (p.group_id as u64, 8),
                (p.cod as u64, 3),
                (p.modulation as u64, 3),
                (p.rotation as u64, 1),
                (p.fec_type as u64, 2),
                (p.num_blocks_max as u64, 10),
                (p.frame_interval as u64, 8),
                (p.time_il_length as u64, 8),
                (p.time_il_type as u64, 1),
                (p.in_band_a as u64, 1),
                (p.in_band_b as u64, 1),
                (p.reserved_1 as u64, 11),
                (p.plp_mode as u64, 2),
                (p.static_flag as u64, 1),
                (p.static_padding_flag as u64, 1),
            ];
            for (v, n) in f {
                push_u64(&mut b, v, n);
            }
        }
        push_u64(&mut b, c.fef_length_msb as u64, 2);
        push_u64(&mut b, c.reserved_2 as u64, 30);
        for &(t, conf) in &c.aux {
            push_u64(&mut b, t as u64, 4);
            push_u64(&mut b, conf as u64, 28);
        }
        write_dynamic(&mut b, &self.dynamic);
        if let Some(n) = &self.next {
            write_dynamic(&mut b, n);
        }
        b.extend_from_slice(&self.extension);
        b
    }

    /// The information bits followed by the CRC-32 (K_post_ex_pad bits).
    pub fn to_bits(&self) -> Vec<u8> {
        let mut b = self.info_bits();
        let crc = CRC32_MPEG2.bits(&b);
        push_u64(&mut b, crc as u64, 32);
        b
    }

    /// Reads the L1-post from its `info_size + 32` bits, with the layout fixed by L1-pre (NUM_RF, the FEF bit of S2,
    /// L1_REPETITION_FLAG). `None` when the CRC-32 is wrong or the fields do not fit.
    pub fn from_bits(b: &[u8], pre: &L1Pre) -> Option<L1Post> {
        let mut p = L1Post::default();
        p.read(b, pre).then_some(p)
    }

    /// Reads the L1-post into `self` as [`L1Post::from_bits`] does, and keeps the memory of its lists. Returns false when the
    /// CRC-32 is wrong or the fields do not fit. `self` then holds no meaning.
    pub fn read(&mut self, b: &[u8], pre: &L1Pre) -> bool {
        let n = pre.l1_post_info_size as usize;
        if b.len() < n + 32 || !CRC32_MPEG2.verify_bits(&b[..n + 32]) {
            return false;
        }
        let mut r = Fields { b: &b[..n], at: 0 };
        let c = &mut self.config;
        c.sub_slices_per_frame = r.u(15) as u16;
        let num_plp = r.u(8) as usize;
        let num_aux = r.u(4) as usize;
        c.aux_config_rfu = r.u(8) as u8;
        c.rf.clear();
        for _ in 0..pre.num_rf {
            c.rf.push((r.u(3) as u8, r.u(32) as u32));
        }
        c.fef = (pre.s2 & 1 == 1).then(|| (r.u(4) as u8, r.u(22) as u32, r.u(8) as u8));
        c.plps.clear();
        for _ in 0..num_plp {
            c.plps.push(PlpConfig {
                plp_id: r.u(8) as u8,
                plp_type: r.u(3) as u8,
                payload_type: r.u(5) as u8,
                ff_flag: r.u(1) == 1,
                first_rf_idx: r.u(3) as u8,
                first_frame_idx: r.u(8) as u8,
                group_id: r.u(8) as u8,
                cod: r.u(3) as u8,
                modulation: r.u(3) as u8,
                rotation: r.u(1) == 1,
                fec_type: r.u(2) as u8,
                num_blocks_max: r.u(10) as u16,
                frame_interval: r.u(8) as u8,
                time_il_length: r.u(8) as u8,
                time_il_type: r.u(1) == 1,
                in_band_a: r.u(1) == 1,
                in_band_b: r.u(1) == 1,
                reserved_1: r.u(11) as u16,
                plp_mode: r.u(2) as u8,
                static_flag: r.u(1) == 1,
                static_padding_flag: r.u(1) == 1,
            });
        }
        c.fef_length_msb = r.u(2) as u8;
        c.reserved_2 = r.u(30) as u32;
        c.aux.clear();
        for _ in 0..num_aux {
            c.aux.push((r.u(4) as u8, r.u(28) as u32));
        }
        read_dynamic(&mut r, num_plp, num_aux, &mut self.dynamic);
        if pre.repetition {
            let next = self.next.get_or_insert_with(PostDynamic::default);
            read_dynamic(&mut r, num_plp, num_aux, next);
        } else {
            self.next = None;
        }
        if r.at > n {
            return false;
        }
        self.extension.clear();
        self.extension.extend_from_slice(&b[r.at..n]);
        true
    }
}

fn write_dynamic(b: &mut Vec<u8>, d: &PostDynamic) {
    push_u64(b, d.frame_idx as u64, 8);
    push_u64(b, d.sub_slice_interval as u64, 22);
    push_u64(b, d.type_2_start as u64, 22);
    push_u64(b, d.l1_change_counter as u64, 8);
    push_u64(b, d.start_rf_idx as u64, 3);
    push_u64(b, d.reserved_1 as u64, 8);
    for p in &d.plps {
        push_u64(b, p.plp_id as u64, 8);
        push_u64(b, p.plp_start as u64, 22);
        push_u64(b, p.num_blocks as u64, 10);
        push_u64(b, p.reserved_2 as u64, 8);
    }
    push_u64(b, d.reserved_3 as u64, 8);
    for &a in &d.aux {
        push_u64(b, a, 48);
    }
}

fn read_dynamic(r: &mut Fields<'_>, num_plp: usize, num_aux: usize, d: &mut PostDynamic) {
    d.frame_idx = r.u(8) as u8;
    d.sub_slice_interval = r.u(22) as u32;
    d.type_2_start = r.u(22) as u32;
    d.l1_change_counter = r.u(8) as u8;
    d.start_rf_idx = r.u(3) as u8;
    d.reserved_1 = r.u(8) as u8;
    d.plps.clear();
    for _ in 0..num_plp {
        d.plps.push(PlpDynamic {
            plp_id: r.u(8) as u8,
            plp_start: r.u(22) as u32,
            num_blocks: r.u(10) as u16,
            reserved_2: r.u(8) as u8,
        });
    }
    d.reserved_3 = r.u(8) as u8;
    d.aux.clear();
    for _ in 0..num_aux {
        d.aux.push(r.u(48));
    }
}

/// The sizes of the L1-post coding for one configuration (clause 7.3.1.2).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PostSizes {
    /// N_post_FEC_Block.
    pub blocks: usize,
    /// K_sig, the information bits per block (padding included).
    pub k_sig: usize,
    /// K_L1_PADDING.
    pub padding: usize,
    /// N_punc, punctured parity bits per block.
    pub n_punc: usize,
    /// N_post, sent bits per block.
    pub n_post: usize,
    /// N_MOD_Total, the L1-post cells (L1_POST_SIZE).
    pub cells: usize,
}

/// K_bch of the L1-post code (16K, rate 1/2, table 39).
const POST_K_BCH: usize = 7032;

/// The L1-post sizes for `info_size` information bits (L1_POST_INFO_SIZE), the L1-post constellation and N_P2 (clause 7.3.1.2).
/// N_punc_temp is the larger of floor(6/5 (K_bch - K_sig)) and N_L1mult - 1, so that N_punc is never negative. Errors for
/// 256-QAM or an empty L1-post.
pub fn post_sizes(info_size: usize, m: Modulation, n_p2: usize) -> Result<PostSizes, T2Error> {
    if m == Modulation::Qam256 || info_size == 0 {
        return Err(T2Error::Rule(Rule::PostCoding));
    }
    let k_ex_pad = info_size + 32;
    let blocks = k_ex_pad.div_ceil(POST_K_BCH);
    let k_post = k_ex_pad.div_ceil(blocks) * blocks;
    let padding = k_post - k_ex_pad;
    let k_sig = k_post / blocks;
    let eta = m.bits();
    let mult = if n_p2 == 1 { 2 * eta } else { n_p2 * eta };
    let temp = (6 * (POST_K_BCH - k_sig) / 5).max(mult - 1);
    let n_post_temp = k_sig + 168 + 9000 - temp;
    let n_post = n_post_temp.div_ceil(mult) * mult;
    let n_punc = temp - (n_post - n_post_temp);
    Ok(PostSizes {
        blocks,
        k_sig,
        padding,
        n_punc,
        n_post,
        cells: n_post / eta * blocks,
    })
}

/// The shortening and puncturing of one 16K L1 code word (clauses 7.3.2.2, 7.3.2.5 and 7.3.2.6): which BCH information bits
/// carry the K_sig signalling bits, and which LDPC parity bits are sent.
#[derive(Clone, Debug)]
struct ShortPunct {
    /// The K_bch positions of the information bits, in signalling order.
    info: Vec<u32>,
    /// The sent parity indices (0 is the first LDPC parity bit), increasing.
    parity: Vec<u32>,
}

impl ShortPunct {
    /// The positions for K_sig signalling bits in a K_bch information word (groups of 360 in the order `pad`) and N_punc
    /// punctured bits of the `360 q` parity bits (groups in the order `punc`).
    fn new(k_bch: usize, q: usize, k_sig: usize, n_punc: usize, pad: &[u8], punc: &[u8]) -> Result<ShortPunct, T2Error> {
        let mut cut = Vec::new();
        shortened_positions(k_bch, k_sig, 360, pad, &mut cut)?;
        let info = complement(k_bch, &cut);
        punctured_positions(360 * q, q, punc, n_punc, &mut cut)?;
        let parity = complement(360 * q, &cut);
        Ok(ShortPunct { info, parity })
    }
}

/// The positions 0 to `n - 1` that are not in `taken` (increasing).
fn complement(n: usize, taken: &[u32]) -> Vec<u32> {
    (0..n as u32).filter(|p| taken.binary_search(p).is_err()).collect()
}

/// The min-sum rule of the L1 decoder for K_sig signalling bits in cells of `m`: an offset of 1 for BPSK from 1 000 bits
/// and for 16-QAM from 1 500 bits, with scale 16/16 below 4 300 bits and 15/16 above; else 16/16 below 1 500 bits and
/// 15/16 above, with no offset.
///
/// Shortening and puncturing leave few sent bits per check, and a scaled message fades along the chains of punctured
/// parity bits. Measured with `scale::l1_words` (BPSK and 16-QAM, a word lost when LDPC or BCH fails): at K_sig 350, for
/// L1-pre and for 16-QAM at 1 032 an offset costs 1.1 dB to 1.3 dB; at the larger sizes it gains 0.05 dB to 0.46 dB at a
/// frame error rate of 0.1 over the rule without it. The other constellations were not measured with an offset.
fn l1_rule(k_sig: usize, m: Modulation) -> MinSum {
    let (scale_q4, offset) = match (m, k_sig) {
        (Modulation::Bpsk, 1000..4300) | (Modulation::Qam16, 1500..4300) => (16, 1),
        (Modulation::Bpsk | Modulation::Qam16, 4300..) => (15, 1),
        (_, 0..1500) => (16, 0),
        _ => (15, 0),
    };
    MinSum { scale_q4, offset }
}

/// Encoder and decoder of one kind of L1 code word: L1-pre (16K, 1/4, BPSK) or one L1-post block (16K, 1/2).
pub struct L1Codec {
    sp: ShortPunct,
    bch: Bch,
    code: LdpcCode,
    dec: LdpcDecoder,
    k_sig: usize,
    perm: Option<Permutation>,
    m: Modulation,
    points: Vec<Complex32>,
    cons: Constellation,
    scrambled: bool,
    word: Vec<u8>,
    cw: Vec<u8>,
    sent: Vec<u8>,
    llr_f: Vec<f32>,
    llr8: Vec<Llr8>,
    hard: Vec<u8>,
    soft: Vec<f32>,
    sent8: Vec<Llr8>,
    info: Vec<u8>,
    words: Vec<u8>,
}

impl L1Codec {
    /// The L1-pre codec: K_sig 200, N_punc 11 488, 1 840 BPSK cells.
    pub fn pre() -> Result<L1Codec, T2Error> {
        let sp = ShortPunct::new(
            3072,
            36,
            L1_PRE_BITS,
            11_488,
            &tables::L1_PRE_PAD_ORDER,
            &tables::L1_PRE_PUNCTURE_ORDER,
        )?;
        L1Codec::build(
            sp,
            &codes::DVB_S2_SHORT_1_4,
            &dvb_s2::S2_S_1_4,
            L1_PRE_BITS,
            Modulation::Bpsk,
            false,
            None,
        )
    }

    /// The codec of one L1-post block for these sizes, constellation and scrambling.
    pub fn post(s: &PostSizes, m: Modulation, scrambled: bool) -> Result<L1Codec, T2Error> {
        let sp = ShortPunct::new(
            POST_K_BCH,
            25,
            s.k_sig,
            s.n_punc,
            tables::l1_post_pad_order(m),
            tables::l1_post_puncture_order(m),
        )?;
        let perm = match m {
            Modulation::Qam16 | Modulation::Qam64 => Some(l1_post_bits_to_cells(s.n_post, m)?),
            _ => None,
        };
        L1Codec::build(
            sp,
            &codes::DVB_S2_SHORT_1_2,
            &dvb_s2::S2_S_1_2,
            s.k_sig,
            m,
            scrambled,
            perm,
        )
    }

    fn build(
        sp: ShortPunct,
        bch: &sdrkit_algo_fec::bch::BchCode,
        table: &'static sdrkit_algo_fec::ldpc::tables::DvbTable,
        k_sig: usize,
        m: Modulation,
        scrambled: bool,
        perm: Option<Permutation>,
    ) -> Result<L1Codec, T2Error> {
        let bch = Bch::from_code(bch)?;
        let code = LdpcCode::from_dvb(table)?;
        let set = point_set(m);
        let n_sent = k_sig + 168 + sp.parity.len();
        Ok(L1Codec {
            dec: LdpcDecoder::new(&code, l1_rule(k_sig, m)),
            cons: Constellation::new(set.clone())?,
            points: set.points().to_vec(),
            word: vec![0; bch.n()],
            cw: vec![0; code.n()],
            sent: vec![0; n_sent],
            llr_f: vec![0.0; n_sent],
            llr8: vec![0; code.n()],
            hard: vec![0; code.n()],
            soft: vec![0.0; n_sent],
            sent8: vec![0; n_sent],
            info: vec![0; k_sig],
            words: vec![0; n_sent],
            sp,
            bch,
            code,
            k_sig,
            perm,
            m,
            scrambled,
        })
    }

    /// The cells of one code word.
    pub fn cells(&self) -> usize {
        self.sent.len() / self.m.bits()
    }

    /// K_sig.
    pub fn k_sig(&self) -> usize {
        self.k_sig
    }

    /// Encodes K_sig signalling bits into cells (`cells()` of them). Errors on wrong lengths.
    pub fn encode(&mut self, bits: &[u8], out: &mut [Complex32]) -> Result<(), T2Error> {
        if bits.len() != self.k_sig {
            return Err(T2Error::Length {
                expected: self.k_sig,
                got: bits.len(),
            });
        }
        if out.len() != self.cells() {
            return Err(T2Error::Length {
                expected: self.cells(),
                got: out.len(),
            });
        }
        self.info.copy_from_slice(bits);
        if self.scrambled {
            scramble(&mut self.info);
        }
        let info = &self.info;
        let k_bch = self.bch.k();
        self.word.fill(0);
        for (&pos, &b) in self.sp.info.iter().zip(info) {
            self.word[pos as usize] = b;
        }
        self.bch.encode(&mut self.word)?;
        self.code.encode(&self.word, &mut self.cw)?;
        let k_ldpc = self.code.k();
        let mut w = 0;
        for &b in info {
            self.sent[w] = b;
            w += 1;
        }
        for i in k_bch..k_ldpc {
            self.sent[w] = self.cw[i];
            w += 1;
        }
        for &p in &self.sp.parity {
            self.sent[w] = self.cw[k_ldpc + p as usize];
            w += 1;
        }
        let eta = self.m.bits();
        match &self.perm {
            Some(p) => p.apply(&self.sent, &mut self.words)?,
            None => self.words.copy_from_slice(&self.sent),
        }
        for (o, c) in out.iter_mut().zip(self.words.chunks_exact(eta)) {
            let label = c.iter().fold(0usize, |a, &b| (a << 1) | b as usize);
            *o = self.points[label];
        }
        Ok(())
    }

    /// Decodes one code word from its equalised cells and weights into `bits` (K_sig). Returns whether the LDPC parity equations and
    /// the BCH decoder both passed. Errors on wrong lengths.
    pub fn decode(
        &mut self,
        cells: &[Complex32],
        weights: &[f32],
        bits: &mut [u8],
    ) -> Result<bool, T2Error> {
        if bits.len() != self.k_sig {
            return Err(T2Error::Length {
                expected: self.k_sig,
                got: bits.len(),
            });
        }
        if cells.len() != self.cells() || weights.len() != cells.len() {
            let got = if cells.len() != self.cells() {
                cells.len()
            } else {
                weights.len()
            };
            return Err(T2Error::Length {
                expected: self.cells(),
                got,
            });
        }
        let eta = self.m.bits();
        for (i, (&z, &w)) in cells.iter().zip(weights).enumerate() {
            let z = if z.re.is_finite() && z.im.is_finite() {
                z
            } else {
                Complex32::default()
            };
            let w = if w.is_finite() && w >= 0.0 { w } else { 0.0 };
            self.cons.demap_weighted(
                z,
                w,
                Demap::MaxLog,
                &mut self.soft[i * eta..(i + 1) * eta],
            )?;
        }
        match &self.perm {
            Some(p) => p.invert(&self.soft, &mut self.llr_f)?,
            None => self.llr_f.copy_from_slice(&self.soft),
        }
        let q = Quantizer::default();
        let scale = q.scale_for(&self.llr_f);
        q.quantize(&self.llr_f, scale, &mut self.sent8)?;
        let sent8 = &self.sent8;
        // Rebuild the 16 200 LLRs: padded bits are known zeros, punctured bits are erasures.
        self.llr8.fill(0);
        let k_bch = self.bch.k();
        let k_ldpc = self.code.k();
        for i in 0..k_bch {
            self.llr8[i] = 127;
        }
        let mut r = 0;
        for &pos in &self.sp.info {
            self.llr8[pos as usize] = sent8[r];
            r += 1;
        }
        for i in k_bch..k_ldpc {
            self.llr8[i] = sent8[r];
            r += 1;
        }
        for &p in &self.sp.parity {
            self.llr8[k_ldpc + p as usize] = sent8[r];
            r += 1;
        }
        let ldpc_ok = self.dec.decode(&self.llr8, 50, &mut self.hard).is_ok();
        let bch_ok = self.bch.decode(&mut self.hard[..self.bch.n()]).is_ok();
        for (b, &pos) in bits.iter_mut().zip(&self.sp.info) {
            *b = self.hard[pos as usize];
        }
        if self.scrambled {
            scramble(bits);
        }
        Ok(ldpc_ok && bch_ok)
    }
}

/// The L1-post scrambler (clause 7.3.2.1): the BB scrambling sequence from its start, XORed bit by bit.
pub fn scramble(bits: &mut [u8]) {
    let mut r = DVB_DISPERSAL;
    for b in bits.iter_mut() {
        *b ^= r.bit();
    }
}

/// The L1 code parameters as the standard tabulates them, for tests: (K_bch, K_ldpc) of L1-pre and L1-post (table 39).
pub const L1_CODE_PARAMS: [(usize, usize); 2] = [(3072, 3240), (7032, 7200)];

/// The FEC frame and rate of the L1 codes (clause 7.3.1).
pub const L1_CODES: [(FecFrame, CodeRate); 2] = [
    (FecFrame::Short, CodeRate::R1_4),
    (FecFrame::Short, CodeRate::R1_2),
];

/// The min-sum rule of the L1 codes:
/// `cargo test --release -p sdrkit-stack-dvbt2 --lib l1::scale -- --ignored --nocapture`.
#[cfg(test)]
mod scale {
    use super::*;
    use crate::bicm::scale::{compare, header, rules};
    use sdrkit_dev_test::fer::{self, frames, noise, Trial};

    struct Word {
        enc: L1Codec,
        decs: Vec<L1Codec>,
        bits: Vec<u8>,
        out: Vec<u8>,
        cells: Vec<Complex32>,
        weights: Vec<f32>,
        snr_db: f32,
        seed: u64,
    }

    impl Word {
        fn new(make: &dyn Fn() -> L1Codec) -> Word {
            let enc = make();
            let decs = rules()
                .into_iter()
                .map(|r| {
                    let mut c = make();
                    c.dec = LdpcDecoder::new(&c.code, r);
                    c
                })
                .collect();
            Word {
                bits: vec![0; enc.k_sig()],
                out: vec![0; enc.k_sig()],
                cells: vec![Complex32::default(); enc.cells()],
                weights: vec![0.0; enc.cells()],
                enc,
                decs,
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
            sdrkit_algo_sim::rng::Rng::new(seed).fill_bits(&mut self.bits);
            self.enc.encode(&self.bits, &mut self.cells).expect("encode");
            noise(&mut self.cells, &mut self.weights, self.snr_db, !seed);
        }
        fn lost(&mut self, i: usize) -> bool {
            let ok = self.decs[i]
                .decode(&self.cells, &self.weights, &mut self.out)
                .expect("decode");
            !ok || self.out != self.bits
        }
    }

    /// The frame error rate of L1-pre and of L1-post at several sizes, per rule, 1000 frames a point read at 0.1 and
    /// 0.01. A word counts as lost when the decoder reports a failure (the receiver drops it) or its bits are wrong.
    #[test]
    #[ignore = "measurement"]
    fn l1_words() {
        let frames = frames(1000);
        let one_plp = crate::tx::single_plp_post_bits();
        type Make = Box<dyn Fn() -> L1Codec + Sync>;
        let mut cases: Vec<(Make, String, f64)> =
            vec![(Box::new(|| L1Codec::pre().expect("codec")), "L1-pre BPSK".into(), -6.0)];
        for (info, m, first) in [
            (one_plp, Modulation::Bpsk, -6.0),
            (1000, Modulation::Bpsk, -6.0),
            (2000, Modulation::Bpsk, -6.0),
            (3500, Modulation::Bpsk, -6.0),
            (5000, Modulation::Bpsk, -6.0),
            (7000, Modulation::Bpsk, -6.0),
            (one_plp, Modulation::Qam16, 2.0),
            (1000, Modulation::Qam16, 2.0),
            (2000, Modulation::Qam16, 2.0),
            (5000, Modulation::Qam16, 2.0),
            (7000, Modulation::Qam16, 2.0),
        ] {
            let make = move || {
                let s = post_sizes(info, m, 1).expect("sizes");
                L1Codec::post(&s, m, false).expect("codec")
            };
            cases.push((Box::new(make), format!("L1-post {info} bits {m:?}"), first));
        }
        // LDPC_SCALE_ROWS, a comma list, keeps the cases whose name holds one of its items.
        if let Ok(keep) = std::env::var("LDPC_SCALE_ROWS") {
            cases.retain(|(_, name, _)| keep.split(',').any(|item| name.contains(item.trim())));
        }
        header("Es/N0", frames);
        let lines = fer::parallel(&cases, |(make, name, first)| {
            let c = make();
            let used = l1_rule(c.k_sig(), c.m);
            let name = format!("{:<34}", format!("{name} (K_sig {})", make().k_sig()));
            compare(&mut Word::new(make), &name, used, *first, frames, true)
        });
        for l in lines {
            println!("{l}");
        }
    }
}
