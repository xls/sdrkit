//! The protection of the ATSC 3.0 L1 signalling (A/322 clause 6.5): a shortened BCH code, zero padding by bit groups, the
//! 16200-bit LDPC code, the permutation of the parity groups, repetition, puncturing and the bit demultiplexer.

use super::tables::{PARITY_27, PARITY_36, SHORTEN_18, SHORTEN_9};
use crate::bch::Bch;
use crate::error::{Decoded, FecError, Param, Result};
use crate::ldpc::tables::atsc3::{Atsc3Table, BCH_PARITY_16K, BCH_T};
use crate::ldpc::tables::dvb_bch::S2_SHORT_MINPOLYS;
use crate::ldpc::{LdpcCode, LdpcDecoder, MinSum};
use crate::llr::Llr8;

/// The two parts of the L1 signalling.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum L1Kind {
    /// L1-Basic: always 200 bits.
    Basic,
    /// L1-Detail: one coded block of 200 bits or more.
    Detail,
}

/// The lengths of one coded block (A/322 clauses 6.5.2.7 and 6.5.2.8).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct L1Lengths {
    /// `N_outer`: the information bits and the 168 BCH parity bits.
    pub n_outer: usize,
    /// `N_repeat`: the parity bits sent a second time.
    pub n_repeat: usize,
    /// `N_punc`: the parity bits not sent.
    pub n_punc: usize,
    /// `N_FEC`: the bits of the block without the repeated ones.
    pub n_fec: usize,
}

impl L1Lengths {
    /// The bits sent: `N_FEC + N_repeat`.
    pub fn sent(&self) -> usize {
        self.n_fec + self.n_repeat
    }
}

const N: usize = 16200;
const M_OUTER: usize = BCH_PARITY_16K as usize;

/// The LDPC check rule for a block of mode `mode` of `kind` with `punc` of its `parity` parity bits punctured, as
/// (scale in sixteenths, offset). Where little parity is punctured, an offset of 1 corrects the small magnitudes that a
/// scale leaves unchanged at an LLR gain of 4. Where much is punctured, the punctured bits start at 0 and an offset stalls
/// the decoder; there the rule is a scale alone, and the more is punctured, the less the minimum is scaled. Mode 7 of
/// L1-Detail has its own steps.
///
/// Measured by the `l1_scale` test of the ATSC 3.0 stack (the constellation of each mode, max-log demapping, an LLR gain
/// of 4, 50 iterations): the rule of the lowest Es/N0 at a block error rate of 0.1 among those with no more failures 0.3 dB
/// above it. The steps lie between measured sizes.
fn l1_rule(kind: L1Kind, mode: u8, punc: usize, parity: usize) -> MinSum {
    // The punctured share in ten-thousandths.
    let share = 10_000 * punc / parity;
    let (scale_q4, offset) = match (kind, mode) {
        // The 3/15 code.
        (L1Kind::Basic, _) | (L1Kind::Detail, 1 | 2) => match share {
            0..7000 => (14, 1),
            7000..9300 => (13, 0),
            9300..9800 => (14, 0),
            _ => (15, 0),
        },
        (L1Kind::Detail, 7) => match share {
            0..9200 => (16, 1),
            9200..9666 => (14, 0),
            9666..9675 => (15, 0),
            _ => (16, 0),
        },
        _ => match share {
            0..6300 => (16, 1),
            6300..7000 => (14, 0),
            7000..8200 => (16, 1),
            8200..8850 => (15, 0),
            _ => (16, 0),
        },
    };
    MinSum { scale_q4, offset }
}

/// One protection mode of L1-Basic or L1-Detail (A/322 table 6.17).
pub struct L1Code {
    kind: L1Kind,
    mode: u8,
    bits: usize,
    k_ldpc: usize,
    k_seg: usize,
    /// `A` of table 6.24 as a fraction, and `B`.
    a: (usize, usize),
    b: usize,
    /// `C` of table 6.23 as a fraction and `D`, for the modes with repetition.
    rep: Option<((usize, usize), isize)>,
    shorten: &'static [u8],
    parity: &'static [u8],
    table: Atsc3Table,
    code: LdpcCode,
    dec: LdpcDecoder,
    /// The check rule of `dec`.
    rule: MinSum,
    bch: Option<Bch>,
    /// The padded positions for `pad_for` real bits, and the buffers of the decoder.
    pad: Vec<bool>,
    pad_for: usize,
    sent: Vec<Llr8>,
    soft: Vec<Llr8>,
    hard: Vec<u8>,
    word: Vec<u8>,
}

impl L1Code {
    /// Mode `mode` (1 to 7) of `kind`. An error for another mode.
    pub fn new(kind: L1Kind, mode: u8) -> Result<Self> {
        if !(1..=7).contains(&mode) {
            return Err(FecError::Conflict {
                param: Param::Table,
                with: Param::Rate,
            });
        }
        let m = usize::from(mode) - 1;
        let low = kind == L1Kind::Basic || mode <= 2;
        let (bits, a, b, k_seg) = match (kind, mode) {
            (L1Kind::Basic, _) => (
                [2, 2, 2, 4, 6, 8, 8][m],
                (0, 1),
                [9360, 11460, 12360, 12292, 12350, 12432, 12776][m],
                200,
            ),
            (L1Kind::Detail, 1) => (2, (7, 2), 0, 2352),
            (L1Kind::Detail, 2) => (2, (2, 1), 6036, 3072),
            (L1Kind::Detail, _) => (
                [2, 4, 6, 8, 8][m - 2],
                [(11, 16), (29, 32), (3, 4), (11, 16), (49, 256)][m - 2],
                [4653, 3200, 4284, 4900, 8246][m - 2],
                6312,
            ),
        };
        // Table 6.23: only mode 1 of each repeats parity bits.
        let rep = match (kind, mode) {
            (L1Kind::Basic, 1) => Some(((0, 1), 3672)),
            (L1Kind::Detail, 1) => Some(((61, 16), -508)),
            _ => None,
        };
        let (shorten, parity): (&'static [u8], &'static [u8]) = match (kind, mode) {
            (L1Kind::Basic, _) => (&SHORTEN_9[0], &PARITY_36[0]),
            (L1Kind::Detail, 1 | 2) => (&SHORTEN_9[m + 1], &PARITY_36[m + 1]),
            (L1Kind::Detail, _) => (&SHORTEN_18[m - 2], &PARITY_27[m - 2]),
        };
        let rate = if low { 3 } else { 6 };
        let table = Atsc3Table::find(N, rate).ok_or(FecError::Conflict {
            param: Param::N,
            with: Param::Rate,
        })?;
        let code = LdpcCode::from_atsc3(table)?;
        let rule = MinSum::default();
        let dec = LdpcDecoder::new(&code, rule);
        Ok(L1Code {
            kind,
            mode,
            bits,
            k_ldpc: table.k(),
            k_seg,
            a,
            b,
            rep,
            shorten,
            parity,
            table,
            code,
            dec,
            rule,
            bch: None,
            pad: Vec::new(),
            pad_for: usize::MAX,
            sent: Vec::new(),
            soft: vec![0; N],
            hard: vec![0; N],
            word: Vec::new(),
        })
    }

    /// L1-Basic or L1-Detail.
    pub fn kind(&self) -> L1Kind {
        self.kind
    }

    /// The mode, 1 to 7.
    pub fn mode(&self) -> u8 {
        self.mode
    }

    /// Bits per cell of the constellation of the mode: 2 (QPSK), 4, 6 or 8.
    pub fn bits_per_cell(&self) -> usize {
        self.bits
    }

    /// The code rate of the constellation of the mode in fifteenths (table 6.17: NUC_16_8/15, NUC_64_9/15, NUC_256_9/15,
    /// NUC_256_13/15). QPSK is the same for every rate.
    pub fn constellation_rate(&self) -> u8 {
        match (self.kind, self.mode) {
            (L1Kind::Basic, 4) | (L1Kind::Detail, 4) => 8,
            (_, 7) => 13,
            (_, 5) | (_, 6) => 9,
            _ => 8,
        }
    }

    /// The most information bits of one coded block: 200 for L1-Basic, `K_seg` of table 6.25 for L1-Detail.
    pub fn max_k_sig(&self) -> usize {
        self.k_seg
    }

    /// The lengths of a block of `k_sig` information bits. An error when `k_sig` is not one the mode carries.
    pub fn lengths(&self, k_sig: usize) -> Result<L1Lengths> {
        let ok = match self.kind {
            L1Kind::Basic => k_sig == 200,
            L1Kind::Detail => (200..=self.k_seg).contains(&k_sig),
        };
        if !ok {
            return Err(FecError::InputLength {
                expected: self.k_seg,
                got: k_sig,
            });
        }
        let n_outer = k_sig + M_OUTER;
        let parity = N - self.k_ldpc;
        let n_repeat = match self.rep {
            Some(((c, cd), d)) => (2 * (c * n_outer / cd)) as isize + d,
            None => 0,
        };
        let punc_temp = self.a.0 * (self.k_ldpc - n_outer) / self.a.1 + self.b;
        if n_repeat < 0 || punc_temp > parity {
            return Err(FecError::Conflict {
                param: Param::Puncturing,
                with: Param::K,
            });
        }
        let fec_temp = n_outer + parity - punc_temp;
        let n_fec = fec_temp.div_ceil(self.bits) * self.bits;
        if n_fec - fec_temp > punc_temp {
            return Err(FecError::Conflict {
                param: Param::Puncturing,
                with: Param::K,
            });
        }
        Ok(L1Lengths {
            n_outer,
            n_repeat: n_repeat as usize,
            n_punc: punc_temp - (n_fec - fec_temp),
            n_fec,
        })
    }

    /// Whether information bit `i` of the LDPC code is a padded zero for `n_outer` real bits (clause 6.5.2.4).
    fn padded(&self, n_outer: usize) -> Vec<bool> {
        let mut pad = vec![false; self.k_ldpc];
        let zeros = self.k_ldpc - n_outer;
        let whole = zeros / 360;
        for &g in &self.shorten[..whole] {
            pad[usize::from(g) * 360..][..360].fill(true);
        }
        if whole < self.shorten.len() {
            let g = usize::from(self.shorten[whole]);
            pad[g * 360..][..zeros - 360 * whole].fill(true);
        }
        pad
    }

    /// The place in the LDPC code word `Λ` of bit `j` of the permuted parity part (clause 6.5.2.6).
    fn parity_at(&self, j: usize) -> usize {
        let first = self.k_ldpc / 360;
        let u = usize::from(self.parity[j / 360]) * 360 + j % 360;
        match self.table {
            Atsc3Table::A(_) => u,
            Atsc3Table::B(t) => {
                // u[k + 360 t + s] = c[k + Q s + t].
                let q = t.q as usize;
                let (t, s) = ((u - first * 360) / 360, u % 360);
                self.k_ldpc + q * s + t
            }
        }
    }

    fn bch(&mut self, n_outer: usize) -> Result<&mut Bch> {
        if self.bch.as_ref().is_none_or(|b| b.n() != n_outer) {
            self.bch = Some(Bch::new(
                14,
                S2_SHORT_MINPOLYS[0],
                n_outer,
                usize::from(BCH_T),
            )?);
        }
        Ok(self.bch.as_mut().expect("set above"))
    }

    /// Encodes one block: `sig` holds the `k_sig` scrambled information bits, one per byte. `out` receives the bits of the
    /// cells in the order of the mapper, [`L1Code::bits_per_cell`] bits per cell (the lengths are [`L1Code::lengths`]). An
    /// error when `sig` has a length the mode does not carry.
    pub fn encode(&mut self, sig: &[u8], out: &mut Vec<u8>) -> Result<()> {
        let len = self.lengths(sig.len())?;
        let mut word = vec![0u8; len.n_outer];
        word[..sig.len()].copy_from_slice(sig);
        self.bch(len.n_outer)?.encode(&mut word)?;
        let pad = self.padded(len.n_outer);
        let mut info = vec![0u8; self.k_ldpc];
        let mut bits = word.iter();
        for (i, &p) in info.iter_mut().zip(&pad) {
            if !p {
                *i = *bits.next().expect("n_outer bits are not padded");
            }
        }
        let mut cw = vec![0u8; N];
        match self.table {
            Atsc3Table::A(t) => t.encode(&info, &mut cw)?,
            Atsc3Table::B(_) => self.code.encode(&info, &mut cw)?,
        }
        let parity = N - self.k_ldpc;
        let mut sent = Vec::with_capacity(len.sent());
        sent.extend_from_slice(&word);
        let kept = parity + len.n_repeat - len.n_punc;
        // The repeated bits are the first bits of the permuted parity part, which then follows whole; the end is punctured.
        for i in 0..kept {
            let j = if i < len.n_repeat {
                i % parity
            } else {
                i - len.n_repeat
            };
            sent.push(cw[self.parity_at(j)]);
        }
        debug_assert_eq!(sent.len(), len.sent());
        // Clause 6.5.2.10: written into `bits` columns, read row by row, each row turned by its index.
        let rows = sent.len() / self.bits;
        out.clear();
        out.reserve(sent.len());
        for i in 0..rows {
            for j in 0..self.bits {
                let c = if self.bits > 2 {
                    (i + j) % self.bits
                } else {
                    j
                };
                out.push(sent[c * rows + i]);
            }
        }
        Ok(())
    }

    /// Decodes one block of `k_sig` information bits from the LLRs `llr` of its cells (the order of [`L1Code::encode`],
    /// positive means 0) with at most `max_iter` LDPC iterations. `out` receives the `k_sig` scrambled information bits.
    /// `corrected` is the number of bits the BCH code corrected. An error when a length is wrong or the BCH code cannot correct
    /// the word that the LDPC decoder leaves. The caller checks the CRC of the signalling. After the first block of a size it
    /// allocates nothing.
    pub fn decode(
        &mut self,
        llr: &[Llr8],
        k_sig: usize,
        max_iter: u32,
        out: &mut [u8],
    ) -> Result<Decoded> {
        let len = self.lengths(k_sig)?;
        let rule = l1_rule(self.kind, self.mode, len.n_punc, N - self.k_ldpc);
        self.decode_with(rule, llr, k_sig, max_iter, out)
    }

    /// [`L1Code::decode`] with the LDPC check rule `rule` in place of the one the mode and size choose. A change of rule
    /// allocates.
    pub fn decode_with(
        &mut self,
        rule: MinSum,
        llr: &[Llr8],
        k_sig: usize,
        max_iter: u32,
        out: &mut [u8],
    ) -> Result<Decoded> {
        let len = self.lengths(k_sig)?;
        if rule != self.rule {
            self.dec = LdpcDecoder::new(&self.code, rule);
            self.rule = rule;
        }
        if llr.len() != len.sent() {
            return Err(FecError::InputLength {
                expected: len.sent(),
                got: llr.len(),
            });
        }
        if out.len() < k_sig {
            return Err(FecError::OutputTooShort { need: k_sig });
        }
        let rows = llr.len() / self.bits;
        self.sent.clear();
        self.sent.resize(llr.len(), 0);
        for i in 0..rows {
            for j in 0..self.bits {
                let c = if self.bits > 2 {
                    (i + j) % self.bits
                } else {
                    j
                };
                self.sent[c * rows + i] = llr[i * self.bits + j];
            }
        }
        if self.pad_for != len.n_outer {
            self.pad = self.padded(len.n_outer);
            self.pad_for = len.n_outer;
        }
        let mut at = 0;
        for i in 0..self.k_ldpc {
            self.soft[i] = if self.pad[i] {
                Llr8::MAX
            } else {
                at += 1;
                self.sent[at - 1]
            };
        }
        self.soft[self.k_ldpc..].fill(0);
        let parity = N - self.k_ldpc;
        for i in 0..self.sent.len() - len.n_outer {
            let j = if i < len.n_repeat {
                i % parity
            } else {
                i - len.n_repeat
            };
            let p = self.parity_at(j);
            self.soft[p] = self.soft[p].saturating_add(self.sent[len.n_outer + i]);
        }
        // A block with most of its parity punctured leaves checks open that hold no sent bit: the information bits can be
        // right while the decoder has not closed every check. The BCH code decides.
        let iterations = match self.dec.decode(&self.soft, max_iter, &mut self.hard) {
            Ok(d) => d.iterations,
            Err(FecError::Uncorrectable) => max_iter,
            Err(e) => return Err(e),
        };
        self.word.clear();
        for i in 0..self.k_ldpc {
            if !self.pad[i] {
                self.word.push(self.hard[i]);
            }
        }
        let mut word = std::mem::take(&mut self.word);
        let bch = self.bch(len.n_outer).and_then(|b| b.decode(&mut word));
        out[..k_sig].copy_from_slice(&word[..k_sig]);
        self.word = word;
        let bch = bch?;
        Ok(Decoded {
            corrected: bch.corrected,
            metric: 0,
            iterations,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Rng(u64);
    impl Rng {
        fn next(&mut self) -> u64 {
            self.0 ^= self.0 << 13;
            self.0 ^= self.0 >> 7;
            self.0 ^= self.0 << 17;
            self.0
        }
        fn bit(&mut self) -> u8 {
            (self.next() >> 40) as u8 & 1
        }
    }

    #[test]
    fn the_orders_of_tables_6_20_to_6_22_are_permutations() {
        for row in SHORTEN_9.iter() {
            let mut s = row.to_vec();
            s.sort_unstable();
            assert_eq!(s, (0..9).collect::<Vec<u8>>());
        }
        for row in SHORTEN_18.iter() {
            let mut s = row.to_vec();
            s.sort_unstable();
            assert_eq!(s, (0..18).collect::<Vec<u8>>());
        }
        for row in PARITY_36.iter() {
            let mut s = row.to_vec();
            s.sort_unstable();
            assert_eq!(s, (9..45).collect::<Vec<u8>>());
        }
        for row in PARITY_27.iter() {
            let mut s = row.to_vec();
            s.sort_unstable();
            assert_eq!(s, (18..45).collect::<Vec<u8>>());
        }
    }

    /// A/322 table 6.17: the cells of an L1-Basic block in each mode.
    #[test]
    fn l1_basic_cell_counts_follow_table_6_17() {
        for (mode, cells) in [
            (1, 3820),
            (2, 934),
            (3, 484),
            (4, 259),
            (5, 163),
            (6, 112),
            (7, 69),
        ] {
            let c = L1Code::new(L1Kind::Basic, mode).unwrap();
            let l = c.lengths(200).unwrap();
            assert_eq!(l.n_outer, 368);
            assert_eq!(l.sent() / c.bits_per_cell(), cells, "mode {mode}");
            assert_eq!(l.sent() % c.bits_per_cell(), 0);
            assert!(c.lengths(201).is_err());
        }
        assert!(L1Code::new(L1Kind::Basic, 0).is_err());
        assert!(L1Code::new(L1Kind::Detail, 8).is_err());
    }

    /// Tables 6.18, 6.23 to 6.25: the range of `K_sig` and whole cells at every size.
    #[test]
    fn l1_detail_lengths_are_whole_cells() {
        for (mode, k_seg) in [
            (1, 2352),
            (2, 3072),
            (3, 6312),
            (4, 6312),
            (5, 6312),
            (6, 6312),
            (7, 6312),
        ] {
            let c = L1Code::new(L1Kind::Detail, mode).unwrap();
            assert_eq!(c.max_k_sig(), k_seg);
            assert!(c.lengths(199).is_err());
            assert!(c.lengths(k_seg + 1).is_err());
            for k in (200..=k_seg).step_by(8) {
                let l = c.lengths(k).unwrap();
                assert_eq!(l.sent() % c.bits_per_cell(), 0, "mode {mode} k {k}");
                assert!(l.n_fec >= l.n_outer);
                assert_eq!(l.n_repeat > 0, mode == 1);
            }
        }
        // Mode 2, 200 bits: N_punc = 2 (3240 - 368) + 6036 = 11780, N_FEC = 1548.
        let l = L1Code::new(L1Kind::Detail, 2)
            .unwrap()
            .lengths(200)
            .unwrap();
        assert_eq!((l.n_punc, l.n_fec, l.n_repeat), (11780, 1548, 0));
        // Mode 1, 200 bits: N_repeat = 2 floor(61 x 368 / 16) - 508 = 2298.
        let l = L1Code::new(L1Kind::Detail, 1)
            .unwrap()
            .lengths(200)
            .unwrap();
        assert_eq!(l.n_repeat, 2298);
        assert_eq!(l.n_punc, 7 * (3240 - 368) / 2);
    }

    /// Clause 6.5.2.4: whole groups in the order of table 6.20, then the first bits of the next group.
    #[test]
    fn zero_padding_takes_groups_in_table_order() {
        let c = L1Code::new(L1Kind::Basic, 1).unwrap();
        let pad = c.padded(368);
        // 2872 zeros: 7 whole groups (4, 1, 5, 2, 8, 6, 0) and the first 352 bits of group 7.
        assert_eq!(pad.iter().filter(|&&p| p).count(), 2872);
        for g in [4, 1, 5, 2, 8, 6, 0] {
            assert!(pad[g * 360..][..360].iter().all(|&p| p));
        }
        assert!(pad[7 * 360..][..352].iter().all(|&p| p));
        assert!(pad[7 * 360 + 352..][..8].iter().all(|&p| !p));
        assert!(pad[3 * 360..][..360].iter().all(|&p| !p));
    }

    fn round_trip(kind: L1Kind, mode: u8, k_sig: usize, noise: i32, rng: &mut Rng) {
        let mut c = L1Code::new(kind, mode).unwrap();
        let sig: Vec<u8> = (0..k_sig).map(|_| rng.bit()).collect();
        let mut sent = Vec::new();
        c.encode(&sig, &mut sent).unwrap();
        assert_eq!(sent.len(), c.lengths(k_sig).unwrap().sent());
        let llr: Vec<Llr8> = sent
            .iter()
            .map(|&b| {
                let s = if b == 0 { 16 } else { -16 };
                let n = (rng.next() % (2 * noise as u64 + 1)) as i32 - noise;
                (s + n).clamp(-127, 127) as Llr8
            })
            .collect();
        let mut out = vec![0u8; k_sig];
        c.decode(&llr, k_sig, 50, &mut out)
            .unwrap_or_else(|e| panic!("{kind:?} mode {mode} k {k_sig}: {e:?}"));
        assert_eq!(out, sig, "{kind:?} mode {mode} k {k_sig}");
        assert!(c.decode(&llr[1..], k_sig, 50, &mut out).is_err());
    }

    #[test]
    fn every_mode_decodes_its_own_block_through_noise() {
        let mut rng = Rng(0x2545_F491_4F6C_DD1D);
        for mode in 1..=7 {
            round_trip(L1Kind::Basic, mode, 200, 12, &mut rng);
            let c = L1Code::new(L1Kind::Detail, mode).unwrap();
            let top = c.max_k_sig();
            for k in [200, 904, top] {
                round_trip(L1Kind::Detail, mode, k, 12, &mut rng);
            }
        }
    }

    /// The word sent is systematic: with the row turn undone, the first `k_sig` bits are the information bits.
    #[test]
    fn the_block_begins_with_the_information_bits() {
        let mut rng = Rng(99);
        let mut c = L1Code::new(L1Kind::Basic, 3).unwrap();
        let sig: Vec<u8> = (0..200).map(|_| rng.bit()).collect();
        let mut sent = Vec::new();
        c.encode(&sig, &mut sent).unwrap();
        let rows = sent.len() / 2;
        // QPSK: bit j of the block is in row j mod rows, column j / rows.
        let got: Vec<u8> = (0..200).map(|j| sent[(j % rows) * 2 + j / rows]).collect();
        assert_eq!(got, sig);
        // 16 points: the row turn of clause 6.5.2.10.
        let mut c = L1Code::new(L1Kind::Basic, 4).unwrap();
        c.encode(&sig, &mut sent).unwrap();
        let rows = sent.len() / 4;
        assert_eq!(rows, 259);
        // Row i sends b_i(i mod 4) first; b_i(0) is bit i of the block.
        for i in 0..200 {
            assert_eq!(sent[i * 4 + (4 - i % 4) % 4], sig[i], "row {i}");
        }
    }
}
