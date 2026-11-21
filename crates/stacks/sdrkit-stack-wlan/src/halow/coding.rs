//! The S1G data-field coding on the shared code: the MCS table, the constellations, the interleavers of each width (built once),
//! the stream and segment parsers, and the data-field decoder.
//! Standard: IEEE Std 802.11-2020 clause 23.3.9 (S1G data field) and clause 21.3.10 (the VHT coding it reuses).

use sdrkit_algo_dsp::constellation::{PointSet, QamMap};
use sdrkit_algo_fec::conv::{codes, depuncture_frame, puncture, Encoder, Ends, Puncture, Viterbi};
use sdrkit_algo_fec::llr::Quantizer;
use sdrkit_algo_modem::constellation::{Constellation, Demap};
use sdrkit_core::Complex32;
use sdrkit_util_bits::interleave::Permutation;
use sdrkit_util_bits::{pack, seq, BitOrder, Pad};

/// The repetition pattern of MCS 10 and the 1 MHz SIG: the second copy of 12 coded bits is XORed with it (clause 23.3.9.11).
pub const REP: [u8; 12] = [1, 0, 0, 0, 0, 1, 0, 1, 0, 1, 1, 1];
/// The tail bits of the K = 7 code.
pub const TAIL: usize = 6;
/// The longest PSDU a frame can carry here, octets.
pub const MAX_PSDU: usize = 4095;

/// The MCS table (clause 23.5): bits per tone, the puncturing pattern and the coding rate. MCS 10 is MCS 0 sent twice.
pub fn mcs_params(mcs: u8) -> Option<(usize, Puncture<'static>, (usize, usize))> {
    Some(match mcs {
        0 | 10 => (1, codes::NONE_1_2, (1, 2)),
        1 => (2, codes::NONE_1_2, (1, 2)),
        2 => (2, codes::WIFI_3_4, (3, 4)),
        3 => (4, codes::NONE_1_2, (1, 2)),
        4 => (4, codes::WIFI_3_4, (3, 4)),
        5 => (6, codes::WIFI_2_3, (2, 3)),
        6 => (6, codes::WIFI_3_4, (3, 4)),
        7 => (6, codes::WIFI_5_6, (5, 6)),
        8 => (8, codes::WIFI_3_4, (3, 4)),
        9 => (8, codes::WIFI_5_6, (5, 6)),
        _ => return None,
    })
}

/// The constellation of `bpsc` bits per tone with the 802.11 labels: BPSK 0 is -1, QAM by `QamMap::IEEE_802_11`.
pub fn points(bpsc: usize) -> PointSet {
    let made = if bpsc <= 1 {
        PointSet::pam(2)
    } else {
        PointSet::qam(1 << bpsc, QamMap::IEEE_802_11)
    };
    made.unwrap_or_else(|_| unreachable!("the 802.11 constellations are valid"))
}

/// The point of `bits` (first bit most significant in the label).
pub fn map(set: &PointSet, bits: &[u8]) -> Complex32 {
    let label = bits
        .iter()
        .fold(0usize, |a, &b| (a << 1) | (b & 1) as usize);
    set.points()[label]
}

/// The interleavers in use, each built the first time it is needed and kept (clause 21.3.10.8).
#[derive(Clone, Debug, Default)]
pub struct Interleavers {
    built: Vec<((usize, usize, usize), Permutation)>,
}

impl Interleavers {
    /// The interleaver of `n_cbps` coded bits per symbol, `n_bpsc` bits per tone and `n_col` columns.
    pub fn get(&mut self, n_cbps: usize, n_bpsc: usize, n_col: usize) -> &Permutation {
        let key = (n_cbps, n_bpsc, n_col);
        let i = match self.built.iter().position(|(k, _)| *k == key) {
            Some(i) => i,
            None => {
                let p = Permutation::wifi(n_cbps, n_bpsc, n_col)
                    .unwrap_or_else(|_| Permutation::identity(n_cbps));
                self.built.push((key, p));
                self.built.len() - 1
            }
        };
        &self.built[i].1
    }
}

/// The K = 7 encoder of the 802.11 BCC (clause 17.3.5.6), output appended to `out`.
pub fn encode(bits: &[u8], out: &mut Vec<u8>) {
    Encoder::new(codes::WIFI_K7).encode(bits, out);
}

/// Punctures the mother code stream `coded` with `p` into `out`.
pub fn punctured(p: &Puncture<'_>, coded: &[u8], out: &mut Vec<u8>) {
    puncture(p, 0, coded, out);
}

/// The 802.11 scrambler from `seed`, as a source of bits (clause 17.3.5.5).
pub fn scrambler(seed: u8) -> sdrkit_util_bits::lfsr::Lfsr {
    seq::ieee80211(seed)
}

/// A Viterbi decoder of the K = 7 code with its buffers, for frames of up to `max_steps` steps. Soft values are LLRs, positive
/// means 0.
#[derive(Clone, Debug)]
pub struct Bcc {
    viterbi: Viterbi,
    llr: Vec<i8>,
    max_steps: usize,
}

impl Bcc {
    /// A decoder for up to `max_steps` steps (tail included).
    pub fn new(max_steps: usize) -> Bcc {
        Bcc {
            viterbi: Viterbi::new(codes::WIFI_K7, max_steps)
                .unwrap_or_else(|_| unreachable!("WIFI_K7 is a valid code")),
            llr: Vec::with_capacity(2 * max_steps),
            max_steps,
        }
    }

    /// Decodes `2 * steps` LLRs of a frame that ends with the zero tail. `out` gets `steps - 6` bits. Returns false when the input
    /// is too short or too long.
    pub fn decode(&mut self, llr: &[f32], steps: usize, out: &mut Vec<u8>) -> bool {
        if steps <= TAIL || steps > self.max_steps || llr.len() < 2 * steps {
            return false;
        }
        self.llr.clear();
        self.llr.resize(2 * steps, 0);
        if Quantizer::default()
            .quantize_auto(&llr[..2 * steps], &mut self.llr)
            .is_err()
        {
            return false;
        }
        out.clear();
        out.resize(steps - TAIL, 0);
        self.viterbi.decode(&self.llr, Ends::Zero, out).is_ok()
    }
}

/// A demapper of one MCS: max-log LLRs (positive means 0) of an equalised tone with weight `|H|^2 / noise`, and the nearest point.
#[derive(Clone, Debug)]
pub struct Demapper {
    c: Constellation,
    bits: [f32; 8],
}

impl Demapper {
    /// The demapper of `bpsc` bits per tone.
    pub fn new(bpsc: usize) -> Demapper {
        Demapper {
            c: Constellation::new(points(bpsc))
                .unwrap_or_else(|_| unreachable!("the 802.11 constellations are valid")),
            bits: [0.0; 8],
        }
    }
    /// Appends the LLRs of `z`.
    pub fn demap(&mut self, z: Complex32, weight: f32, out: &mut Vec<f32>) {
        let m = self.c.bits();
        let _ = self
            .c
            .demap_weighted(z, weight, Demap::MaxLog, &mut self.bits[..m]);
        out.extend_from_slice(&self.bits[..m]);
    }
    /// The nearest point.
    pub fn nearest(&self, z: Complex32) -> Complex32 {
        self.c.nearest(z)
    }
}

/// The soft decoder of the data field: the LLRs of each symbol in, the PSDU out. It keeps its interleavers and buffers, so once it
/// has decoded the longest frame it allocates nothing more.
#[derive(Clone, Debug)]
pub struct DataDecoder {
    perms: Interleavers,
    bcc: Bcc,
    enc: Vec<Vec<f32>>,
    parts: [Vec<f32>; 2],
    coded: Vec<f32>,
    full: Vec<f32>,
    dec: Vec<Vec<u8>>,
    bits: Vec<u8>,
    psdu: Vec<u8>,
}

impl Default for DataDecoder {
    fn default() -> Self {
        DataDecoder::new()
    }
}

/// Interleaver columns of a width (8 MHz segments for 16 MHz, clause 23.3.9.8).
pub fn n_col(w: u32) -> usize {
    match w {
        1 => 8,
        2 => 13,
        4 => 18,
        _ => 26,
    }
}

/// The data tones of each segment of a width: (first tone index, tones).
pub fn data_segments(w: u32, n_sd: usize) -> [(usize, usize); 2] {
    if w == 16 {
        [(0, 234), (234, 234)]
    } else {
        [(0, n_sd), (n_sd, 0)]
    }
}

impl DataDecoder {
    /// A decoder for PSDUs of up to [`MAX_PSDU`] octets.
    pub fn new() -> DataDecoder {
        DataDecoder {
            perms: Interleavers::default(),
            bcc: Bcc::new(8 + 8 * MAX_PSDU + TAIL),
            enc: Vec::new(),
            parts: [Vec::new(), Vec::new()],
            coded: Vec::new(),
            full: Vec::new(),
            dec: Vec::new(),
            bits: Vec::new(),
            psdu: Vec::new(),
        }
    }

    /// Starts a frame sent with `e` encoders.
    pub fn start(&mut self, e: usize) {
        let e = e.max(1);
        if self.enc.len() < e {
            self.enc.resize(e, Vec::new());
            self.dec.resize(e, Vec::new());
        }
        self.enc.truncate(e);
        for v in self.enc.iter_mut() {
            v.clear();
        }
    }

    /// Takes one symbol's LLRs (positive means 0) on the data tones in ascending order, `bpsc` per tone, of width `w` with `n_sd`
    /// data tones.
    pub fn symbol(&mut self, llr: &[f32], w: u32, n_sd: usize, mcs: u8) {
        let Some((bpsc, _, _)) = mcs_params(mcs) else {
            return;
        };
        let segs = data_segments(w, n_sd);
        for (si, &(off, n)) in segs.iter().enumerate() {
            let part = &mut self.parts[si];
            part.clear();
            if n == 0 {
                continue;
            }
            let ncbps = n * bpsc;
            part.resize(ncbps, 0.0);
            let src = &llr[off * bpsc..(off + n) * bpsc];
            let _ = self.perms.get(ncbps, bpsc, n_col(w)).invert(src, part);
        }
        let s_blk = (bpsc / 2).max(1);
        self.coded.clear();
        if segs[1].1 > 0 {
            // The 16 MHz segment deparser: blocks of s bits from the two segments in turn.
            let mut i = 0;
            while i < self.parts[0].len() {
                for p in &self.parts {
                    self.coded
                        .extend_from_slice(&p[i..(i + s_blk).min(p.len())]);
                }
                i += s_blk;
            }
        } else {
            self.coded.extend_from_slice(&self.parts[0]);
        }
        if mcs == 10 {
            // The repeated copy was XORed with REP: a 1 in REP turns the sign of its LLR.
            let (first, rep) = self.coded.split_at_mut(12);
            for ((c, &r), &p) in first.iter_mut().zip(rep.iter()).zip(REP.iter()) {
                *c += if p == 1 { -r } else { r };
            }
            self.coded.truncate(12);
        }
        // The stream deparser: blocks of s bits to each encoder in turn.
        let e = self.enc.len().max(1);
        let mut i = 0;
        while i < self.coded.len() {
            for k in 0..e {
                for _ in 0..s_blk {
                    if i < self.coded.len() {
                        self.enc[k].push(self.coded[i]);
                        i += 1;
                    }
                }
            }
        }
    }

    /// Decodes a frame of `len` PSDU octets sent at `mcs`. Returns the PSDU, or `None` when the symbols were too few.
    pub fn finish(&mut self, len: usize, mcs: u8) -> Option<&[u8]> {
        let (_, p, _) = mcs_params(mcs)?;
        let e = self.enc.len().max(1);
        let per_enc = (8 + 8 * len) / e + TAIL;
        let mother = 2 * per_enc;
        let need = p.punctured_len(0, mother);
        for k in 0..e {
            if self.enc[k].len() < need {
                return None;
            }
            self.full.clear();
            self.full.resize(mother, 0.0);
            depuncture_frame(&p, 0, &self.enc[k][..need], &mut self.full).ok()?;
            if !self.bcc.decode(&self.full, per_enc, &mut self.dec[k]) {
                return None;
            }
        }
        let total = 8 + 8 * len;
        self.bits.clear();
        for i in 0..total {
            self.bits
                .push(self.dec[i % e].get(i / e).copied().unwrap_or(0));
        }
        let mut s = seq::ieee80211_from_outputs(&self.bits[..7]).ok()?;
        for b in self.bits[7..].iter_mut() {
            *b ^= s.bit();
        }
        self.psdu.clear();
        pack::pack(
            &self.bits[8..total],
            BitOrder::Lsb,
            Pad::Zero,
            &mut self.psdu,
        );
        Some(&self.psdu)
    }
}
