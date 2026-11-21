//! The DTMB bit chain in both directions. Transmitter: transport packets, scrambler, BCH(762, 752), LDPC, mapping (with the
//! Nordstrom-Robinson code for 4QAM-NR) and the time interleaver, giving the 3744 data symbols of each frame. Receiver: the
//! time de-interleaver, demapping, code word alignment, LDPC, BCH and descrambling, giving the packets of the good code words.
//!
//! Standard: GB 20600-2006 clause 4.5. The scrambler is the DVB energy dispersal register; it restarts every `P` packets, `P` the
//! packets per frame.

use super::profile::{Mapping, Profile, BRANCHES, DATA, PACKET_BITS, WORD};
use sdrkit_algo_fec::bch::Bch;
use sdrkit_algo_fec::ldpc::tables::dtmb;
use sdrkit_algo_fec::ldpc::{InfoAt, LdpcCode, LdpcDecoder, LdpcEncoder};
use sdrkit_algo_fec::{nordstrom, FecError, Llr8, Quantizer};
use sdrkit_algo_modem::{Constellation, Demap};
use sdrkit_core::Complex32;
use sdrkit_proto_mpegts::outer::TS;
use sdrkit_proto_mpegts::source::PacketSource;
use sdrkit_util_bits::interleave::DelayLanes;
use sdrkit_util_bits::lfsr::Additive;
use sdrkit_util_bits::{seq, BitOrder};
use std::sync::Arc;

/// The circulant size of the LDPC codes.
const Z: usize = 127;
/// Bits of an LDPC code word, of which the first [`LdpcCode::DTMB_DELETED`] are not sent.
const N: usize = 7493;
/// The bits not sent.
const DELETED: usize = LdpcCode::DTMB_DELETED;
/// A variance at or above this marks an erased symbol.
pub const ERASED: f32 = 1e29;
/// Iterations at most.
pub const MAX_ITER: u32 = 60;
/// Iterations of an alignment attempt.
const ALIGN_ITER: u32 = 20;
/// Failed code words in a row after which only every eighth is decoded.
const FAIL_RUN: u32 = 24;
/// Frames skipped after an alignment attempt that fails.
const ALIGN_PAUSE: u32 = 16;

/// The scrambler's bytes over `packets` packets from its restart.
pub fn scrambler_bytes(packets: usize) -> Vec<u8> {
    let mut s = Additive::new(seq::DVB_DISPERSAL);
    let mut v = vec![0u8; packets * TS];
    s.apply_bytes(&mut v, BitOrder::Msb);
    v
}

/// The LDPC code and BCH code of a rate.
fn codes(p: &Profile) -> Result<(LdpcCode, Bch), FecError> {
    Ok((
        LdpcCode::dtmb(p.rate.rows(), InfoAt::End)?,
        Bch::from_code(&dtmb::BCH)?,
    ))
}

/// The transmitter's bit chain for one profile.
pub struct TxChain {
    profile: Profile,
    enc: LdpcEncoder,
    bch: Bch,
    scr: Vec<u8>,
    source: Box<dyn PacketSource>,
    packet: usize,
    bits: Vec<u8>,
    sym_il: DelayLanes<Complex32>,
    bit_il: DelayLanes<u8>,
    cw: Vec<u8>,
    info: Vec<u8>,
    payload: Vec<u8>,
    block: Vec<u8>,
}

impl TxChain {
    /// The chain of `profile` with its packets from `source`.
    ///
    /// # Errors
    ///
    /// [`FecError`] when a code does not build.
    pub fn new(profile: Profile, source: Box<dyn PacketSource>) -> Result<TxChain, FecError> {
        let (code, bch) = codes(&profile)?;
        let m = profile.interleaver.m() as u32;
        Ok(TxChain {
            enc: LdpcEncoder::new(&code)?,
            bch,
            scr: scrambler_bytes(profile.packets_per_frame()),
            source,
            packet: 0,
            bits: Vec::new(),
            sym_il: DelayLanes::forney(BRANCHES as u32, m, false, Complex32::new(0.0, 0.0)),
            bit_il: DelayLanes::forney(BRANCHES as u32, m, false, 0),
            cw: vec![0; N],
            info: vec![0; profile.rate.k()],
            payload: vec![0; profile.rate.packets() * PACKET_BITS],
            block: vec![0; 762],
            profile,
        })
    }

    /// Appends the 7488 sent bits of the next code word.
    fn word(&mut self) -> Result<(), FecError> {
        let mut pk = [0u8; TS];
        let per = self.profile.packets_per_frame();
        for k in 0..self.profile.rate.packets() {
            self.source.next_packet(&mut pk);
            let s = &self.scr[(self.packet % per) * TS..][..TS];
            for (b, x) in pk.iter_mut().zip(s) {
                *b ^= x;
            }
            self.packet += 1;
            for (i, o) in self.payload[k * PACKET_BITS..][..PACKET_BITS]
                .iter_mut()
                .enumerate()
            {
                *o = pk[i / 8] >> (7 - i % 8) & 1;
            }
        }
        for b in 0..self.profile.rate.bch_blocks() {
            self.block[..752].copy_from_slice(&self.payload[b * 752..][..752]);
            self.bch.encode(&mut self.block)?;
            self.info[b * 762..][..762].copy_from_slice(&self.block);
        }
        self.enc.encode(&self.info, &mut self.cw)?;
        self.bits.extend_from_slice(&self.cw[DELETED..]);
        Ok(())
    }

    /// Writes the 3744 data symbols of the next frame to `out`.
    ///
    /// # Errors
    ///
    /// [`FecError`] when a code word does not encode.
    pub fn frame(&mut self, out: &mut [Complex32]) -> Result<(), FecError> {
        let mapping = self.profile.mapping;
        let need = mapping.stream_bits();
        while self.bits.len() < need {
            self.word()?;
        }
        if mapping == Mapping::Qam4Nr {
            self.bit_il.process(&mut self.bits[..need]);
            let mut w = [0u8; 16];
            for (g, o) in self.bits[..need]
                .as_chunks::<8>()
                .0
                .iter()
                .zip(out.as_chunks_mut::<8>().0)
            {
                let x = g.iter().fold(0u8, |a, &b| (a << 1) | b);
                nordstrom::encode_bits(x, &mut w);
                for (s, pair) in o.iter_mut().zip(w.as_chunks::<2>().0) {
                    *s = super::profile::map_symbol(Mapping::Qam4, pair);
                }
            }
        } else {
            let q = mapping.bits();
            for (s, b) in out.iter_mut().zip(self.bits[..need].chunks_exact(q)) {
                *s = self.sym_il.push(super::profile::map_symbol(mapping, b));
            }
        }
        self.bits.drain(..need);
        Ok(())
    }
}

/// What the frame decoder counted.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Counters {
    /// Code words that decoded with every BCH block and sync byte right.
    pub good: u64,
    /// Code words that did not.
    pub bad: u64,
    /// Code words not tried (only every eighth is tried after a long run of failures).
    pub skipped: u64,
    /// BCH blocks with one bit corrected.
    pub bch_corrected: u64,
    /// Packets delivered.
    pub packets: u64,
    /// Mean LDPC iterations per decoded word, smoothed 0.9 / 0.1.
    pub iterations: f64,
}

/// What became of a code word.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum Fate {
    /// It went through the decoder.
    #[default]
    Decoded,
    /// More than a third of its LLRs were erased: it failed without decoding.
    Erased,
}

/// A code word on its way through the decoder.
#[derive(Clone, Debug, Default)]
struct Word {
    /// Its place in the group.
    g: usize,
    /// The signal time of the frame that completed it.
    time: f64,
    /// The quantised LLRs, the bits not sent first.
    q: Vec<Llr8>,
    fate: Fate,
    /// The LDPC iterations, when every check was met.
    iterations: Option<u32>,
    /// BCH blocks with one bit corrected.
    corrected: u64,
    /// Every BCH block decoded and every packet starts with the sync byte.
    good: bool,
    /// The packets of a good word.
    packets: Vec<[u8; TS]>,
}

impl Word {
    fn new(packets: usize) -> Word {
        Word {
            q: vec![0; N],
            packets: Vec::with_capacity(packets),
            ..Default::default()
        }
    }

    /// Fills the decoder input from the 7488 LLRs `src`; false for a mostly erased word (more than a third exactly 0).
    fn load(&mut self, src: &[f32]) -> bool {
        if src.iter().filter(|&&l| l == 0.0).count() * 3 > WORD {
            return false;
        }
        let quant = Quantizer::default();
        let s = quant.scale_for(src);
        self.q[..DELETED].fill(0);
        let _ = quant.quantize(src, s, &mut self.q[DELETED..]);
        true
    }
}

/// The decoder of one code word: LDPC with the early give-up, BCH and descrambling. It keeps nothing from one word to the
/// next.
struct WordDecoder {
    profile: Profile,
    rows: &'static [&'static [(u16, u16)]],
    dec: LdpcDecoder,
    bch: Bch,
    scr: Arc<[u8]>,
    out: Vec<u8>,
    block: Vec<u8>,
    payload: Vec<u8>,
}

impl WordDecoder {
    fn new(profile: Profile) -> Result<WordDecoder, FecError> {
        let (code, bch) = codes(&profile)?;
        Ok(WordDecoder {
            rows: profile.rate.rows(),
            dec: LdpcDecoder::new(&code, dtmb::RULE),
            bch,
            scr: scrambler_bytes(profile.packets_per_frame()).into(),
            out: vec![0; N],
            block: vec![0; 762],
            payload: vec![0; profile.rate.packets() * PACKET_BITS],
            profile,
        })
    }

    /// Unsatisfied parity checks of the hard decisions in `out`.
    fn unsatisfied(&self) -> usize {
        let mut acc = [0u8; Z];
        let mut n = 0;
        for row in self.rows {
            acc.fill(0);
            for &(c, s) in row.iter() {
                let (base, s) = (c as usize * Z, s as usize);
                let bits = &self.out[base..base + Z];
                for (a, &b) in acc[..Z - s].iter_mut().zip(&bits[s..]) {
                    *a ^= b;
                }
                for (a, &b) in acc[Z - s..].iter_mut().zip(&bits[..s]) {
                    *a ^= b;
                }
            }
            n += acc.iter().filter(|&&a| a != 0).count();
        }
        n
    }

    /// LDPC decoding of `q` with the early give-up: after 4 iterations more than a quarter of the checks unsatisfied, or
    /// after 12 more than a fifth, fails the word. Returns the iterations of a word that meets every check.
    fn ldpc(&mut self, q: &[Llr8]) -> Option<u32> {
        let checks = self.rows.len() * Z;
        for (iters, limit) in [(4, checks / 4), (12, checks / 5), (MAX_ITER, 0)] {
            match self.dec.decode(q, iters, &mut self.out) {
                Ok(d) => return Some(d.iterations),
                Err(_) if iters == MAX_ITER || self.unsatisfied() > limit => return None,
                Err(_) => {}
            }
        }
        None
    }

    /// Decodes a word at the input `q` with a few iterations; true when it meets every check.
    fn meets_checks(&mut self, q: &[Llr8]) -> bool {
        self.dec.decode(q, ALIGN_ITER, &mut self.out).is_ok()
    }

    /// The BCH decoding of a word that met every check: the payload bits into `payload`; false when a block fails or a packet
    /// does not start with the sync byte after descrambling. `corrected` counts the blocks with one bit corrected.
    fn payload_of(&mut self, g: usize, corrected: &mut u64) -> bool {
        let k = self.profile.rate.k();
        let per = self.profile.packets_per_frame();
        let kp = self.profile.rate.packets();
        for b in 0..self.profile.rate.bch_blocks() {
            self.block
                .copy_from_slice(&self.out[N - k + b * 762..][..762]);
            match self.bch.decode(&mut self.block) {
                Ok(d) => *corrected += u64::from(d.corrected > 0),
                Err(_) => return false,
            }
            self.payload[b * 752..][..752].copy_from_slice(&self.block[..752]);
        }
        (0..kp).all(|p| {
            let first = self.payload[p * PACKET_BITS..][..8]
                .iter()
                .fold(0u8, |a, &x| (a << 1) | x);
            first ^ self.scr[((g * kp + p) % per) * TS] == 0x47
        })
    }

    /// Decodes the loaded word `w`: its iterations, its corrections and, when it is good, its packets.
    fn decode(&mut self, w: &mut Word) {
        w.packets.clear();
        w.corrected = 0;
        w.good = false;
        w.iterations = self.ldpc(&w.q);
        if w.iterations.is_none() || !self.payload_of(w.g, &mut w.corrected) {
            return;
        }
        let per = self.profile.packets_per_frame();
        let kp = self.profile.rate.packets();
        for k in 0..kp {
            let mut pk = [0u8; TS];
            for (i, b) in pk.iter_mut().enumerate() {
                let bits = &self.payload[k * PACKET_BITS + 8 * i..][..8];
                *b = bits.iter().fold(0u8, |a, &x| (a << 1) | x);
            }
            let s = &self.scr[((w.g * kp + k) % per) * TS..][..TS];
            for (b, x) in pk.iter_mut().zip(s) {
                *b ^= x;
            }
            w.packets.push(pk);
        }
        w.good = true;
    }
}

/// The receiver's bit chain for one profile: frames of equalised data symbols in, the packets of good code words out, each
/// with the signal time of the frame that completed its code word.
///
/// The de-interleaver starts filled with erased symbols; the output of the first 170 (mode 1) or 510 (mode 2) frames is
/// discarded, after which every frame's output starts on a frame of the sent stream. For 4QAM-NR and 32QAM, whose code words
/// cross frames, the group start is found by decoding a word at the first held frame and one at the next, and again after
/// 24 failed code words in a row: a frame lost without notice moves the group start.
///
/// Every code word is decoded inside [`FrameDecoder::push`], in the order of the words.
pub struct FrameDecoder {
    profile: Profile,
    constellation: Constellation,
    word: WordDecoder,
    sym_deint: DelayLanes<(Complex32, f32)>,
    llr_deint: DelayLanes<f32>,
    frames: u64,
    prime: u64,
    llr: Vec<f32>,
    held_erased: Vec<bool>,
    aligned: bool,
    pause: u32,
    word_index: u64,
    fail_run: u32,
    /// The word in the decoder.
    w: Word,
    cells: Vec<(Complex32, f32)>,
    erased_z: Vec<Complex32>,
    erased_v: Vec<f32>,
    z: Vec<Complex32>,
    wt: Vec<f32>,
    frame_llr: Vec<f32>,
    nr_llr: Vec<f32>,
    /// What was counted.
    pub counters: Counters,
}

impl FrameDecoder {
    /// A decoder for `profile` that decodes in line.
    ///
    /// # Errors
    ///
    /// [`FecError`] when a code does not build.
    pub fn new(profile: Profile) -> Result<FrameDecoder, FecError> {
        let m = profile.interleaver.m() as u32;
        let cm = match profile.mapping {
            Mapping::Qam4Nr => Mapping::Qam4,
            m => m,
        };
        let constellation = Constellation::new(super::profile::point_set(cm))
            .unwrap_or_else(|e| unreachable!("fixed points: {e}"));
        let erased = (Complex32::new(0.0, 0.0), 2.0 * ERASED);
        let per_frame = DATA * profile.mapping.bits();
        Ok(FrameDecoder {
            constellation,
            word: WordDecoder::new(profile)?,
            sym_deint: DelayLanes::forney(BRANCHES as u32, m, true, erased),
            llr_deint: DelayLanes::forney(BRANCHES as u32, m, true, 0.0),
            frames: 0,
            prime: profile.interleaver.delay_frames() as u64,
            llr: Vec::with_capacity(4 * per_frame + 2 * WORD),
            held_erased: Vec::with_capacity(8),
            aligned: profile.mapping.frames_per_group() == 1,
            pause: 0,
            word_index: 0,
            fail_run: 0,
            w: Word::new(profile.rate.packets()),
            cells: vec![erased; DATA],
            erased_z: vec![erased.0; DATA],
            erased_v: vec![erased.1; DATA],
            z: vec![Complex32::new(0.0, 0.0); DATA],
            wt: vec![0.0; DATA],
            frame_llr: vec![0.0; per_frame],
            nr_llr: vec![0.0; DATA],
            counters: Counters::default(),
            profile,
        })
    }

    /// The profile.
    pub fn profile(&self) -> Profile {
        self.profile
    }

    /// Whether the de-interleaver is primed and the code words are aligned.
    pub fn running(&self) -> bool {
        self.frames > self.prime && self.aligned
    }

    /// Takes the 3744 equalised data symbols of the frame at signal time `time` with their complex noise variances (stream
    /// order) and appends the packets of every good code word that is finished to `out`, each with the time of its frame. A
    /// variance of [`ERASED`] or more marks an erased symbol.
    pub fn push(
        &mut self,
        time: f64,
        data: &[Complex32],
        var: &[f32],
        out: &mut Vec<(f64, [u8; TS])>,
    ) {
        let erased = var.iter().all(|&v| v >= ERASED);
        if self.profile.mapping == Mapping::Qam4Nr {
            for (w, &v) in self.wt.iter_mut().zip(var) {
                *w = if v >= ERASED { 0.0 } else { 1.0 / v.max(1e-12) };
            }
            let _ = self.constellation.demap_block_weighted(
                &data[..DATA],
                &self.wt,
                Demap::MaxLog,
                &mut self.frame_llr,
            );
            for (src, dst) in self
                .frame_llr
                .as_chunks::<16>()
                .0
                .iter()
                .zip(self.nr_llr.as_chunks_mut::<8>().0)
            {
                nordstrom::decode_soft(src, dst);
            }
            self.llr_deint.process(&mut self.nr_llr);
        } else {
            for (c, (&d, &v)) in self.cells.iter_mut().zip(data.iter().zip(var)) {
                *c = self.sym_deint.push((d, v));
            }
            for ((z, w), &(d, v)) in self.z.iter_mut().zip(self.wt.iter_mut()).zip(&self.cells) {
                *z = d;
                *w = if v >= ERASED { 0.0 } else { 1.0 / v.max(1e-12) };
            }
            let _ = self.constellation.demap_block_weighted(
                &self.z,
                &self.wt,
                Demap::MaxLog,
                &mut self.frame_llr,
            );
        }
        self.frames += 1;
        if self.frames > self.prime {
            if self.pause > 0 {
                self.pause -= 1;
            } else {
                let frame: &[f32] = if self.profile.mapping == Mapping::Qam4Nr {
                    &self.nr_llr
                } else {
                    &self.frame_llr
                };
                self.llr.extend_from_slice(frame);
                self.held_erased.push(erased);
                self.cut(time, out);
            }
        }
    }

    /// Pushes a frame of erased symbols: the de-interleaver, the priming count and the code word position advance by one frame.
    pub fn erase(&mut self, time: f64, out: &mut Vec<(f64, [u8; TS])>) {
        let (z, v) = (
            std::mem::take(&mut self.erased_z),
            std::mem::take(&mut self.erased_v),
        );
        self.push(time, &z, &v, out);
        self.erased_z = z;
        self.erased_v = v;
    }

    /// Cuts and decodes the code words that the held LLRs complete.
    fn cut(&mut self, time: f64, out: &mut Vec<(f64, [u8; TS])>) {
        let per_frame = self.profile.mapping.stream_bits();
        if self.aligned && self.fail_run >= FAIL_RUN && self.profile.mapping.frames_per_group() > 1
        {
            self.aligned = false;
            self.fail_run = 0;
            let last = self.llr.len().saturating_sub(per_frame);
            self.llr.drain(..last);
            let erased = self.held_erased.pop();
            self.held_erased.clear();
            self.held_erased.extend(erased);
        }
        if !self.aligned {
            if self.llr.len() < per_frame + WORD {
                return;
            }
            let mut found = None;
            if !self.held_erased.iter().any(|&e| e) {
                for off in [0, per_frame] {
                    if self.try_align(off) {
                        found = Some(off);
                        break;
                    }
                }
            }
            match found {
                Some(off) => {
                    self.llr.drain(..off);
                    self.aligned = true;
                    self.word_index = 0;
                }
                None => {
                    self.llr.clear();
                    self.held_erased.clear();
                    self.pause = ALIGN_PAUSE;
                    return;
                }
            }
        }
        let wpg = self.profile.mapping.words_per_group() as u64;
        let mut at = 0;
        while self.llr.len() - at >= WORD {
            let g = (self.word_index % wpg) as usize;
            self.decode_word(at, g, time, out);
            self.word_index += 1;
            at += WORD;
        }
        self.llr.drain(..at);
        let frames_left = self.llr.len().div_ceil(per_frame);
        let n = self.held_erased.len();
        self.held_erased.drain(..n.saturating_sub(frames_left));
    }

    /// Decodes a word at `off` of the held LLRs with a few iterations; true when it meets every check.
    fn try_align(&mut self, off: usize) -> bool {
        self.w.load(&self.llr[off..off + WORD]) && self.word.meets_checks(&self.w.q)
    }

    /// Decodes the code word at `at` of the held LLRs, `g` its place in the group, completed by the frame at `time`.
    fn decode_word(&mut self, at: usize, g: usize, time: f64, out: &mut Vec<(f64, [u8; TS])>) {
        if self.fail_run >= FAIL_RUN && !self.word_index.is_multiple_of(8) {
            self.counters.skipped += 1;
            return;
        }
        let w = &mut self.w;
        (w.g, w.time) = (g, time);
        w.fate = if w.load(&self.llr[at..at + WORD]) {
            self.word.decode(w);
            Fate::Decoded
        } else {
            Fate::Erased
        };
        Self::settle(&mut self.counters, &mut self.fail_run, w, out);
    }

    /// Counts the finished word `w`, the next in order, and appends its packets to `out`.
    fn settle(c: &mut Counters, fail_run: &mut u32, w: &Word, out: &mut Vec<(f64, [u8; TS])>) {
        if w.fate == Fate::Decoded {
            if let Some(it) = w.iterations {
                c.iterations = 0.9 * c.iterations + 0.1 * it as f64;
            }
            c.bch_corrected += w.corrected;
        }
        if w.fate == Fate::Decoded && w.good {
            out.extend(w.packets.iter().map(|p| (w.time, *p)));
            c.good += 1;
            c.packets += w.packets.len() as u64;
            *fail_run = 0;
        } else {
            c.bad += 1;
            *fail_run += 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dtmb::profile::SI_INDICES;
    use sdrkit_algo_sim::rng::Rng;
    use sdrkit_proto_mpegts::source::{Numbered, NumberedCheck, NumberedSource};

    #[test]
    fn scrambler_sequence() {
        let s = scrambler_bytes(1);
        assert_eq!(&s[..8], &[0x03, 0xF6, 0x08, 0x34, 0x30, 0xB8, 0xA3, 0x93]);
        let mut l = seq::DVB_DISPERSAL;
        assert_eq!(l.period(40_000), Some(32_767));
        let mut bits = vec![0u8; 32_767];
        l.fill_bits(&mut bits);
        assert_eq!(bits.iter().filter(|&&b| b == 1).count(), 16_384);
        for lag in [1, 7, 31, 151, 217, 4681] {
            assert!(
                (0..32_767).any(|i| bits[i] != bits[(i + lag) % 32_767]),
                "repeats at {lag}"
            );
        }
        assert_eq!(&bits[..7], &[0, 0, 0, 0, 0, 0, 1]);
    }

    #[test]
    fn time_interleaver_round_trip() {
        for m in [240u32, 720] {
            let mut tx = DelayLanes::forney(52, m, false, 0u32);
            let mut rx = DelayLanes::forney(52, m, true, 0u32);
            let delay = (51 * 52 * m) as usize;
            let n = delay + 20_000;
            let data: Vec<u32> = (1..=n as u32).collect();
            let mut line = data.clone();
            let mut moved = 0;
            let mut at = 0;
            let mut chunk = 1;
            while at < n {
                let e = (at + chunk).min(n);
                tx.process(&mut line[at..e]);
                moved += line[at..e]
                    .iter()
                    .zip(&data[at..e])
                    .filter(|(a, b)| a != b)
                    .count();
                rx.process(&mut line[at..e]);
                at = e;
                chunk = chunk * 3 % 1001 + 1;
            }
            assert_eq!(&line[delay..], &data[..n - delay]);
            assert!(moved > n / 2);
        }
    }

    #[test]
    fn demapper_hard_decisions() {
        use crate::dtmb::profile::{map_symbol, point_set};
        let mut rng = Rng::new(17);
        for m in [
            Mapping::Qam4,
            Mapping::Qam16,
            Mapping::Qam32,
            Mapping::Qam64,
        ] {
            let c = Constellation::new(point_set(m)).unwrap();
            let q = m.bits();
            let (mut errors, mut bits) = (0usize, vec![0u8; q]);
            let mut llr = vec![0f32; q];
            for _ in 0..4000 {
                for b in bits.iter_mut() {
                    *b = rng.bit();
                }
                let z = map_symbol(m, &bits)
                    + Complex32::new(0.05 * rng.normal_f32(), 0.05 * rng.normal_f32());
                c.demap(z, 0.05, Demap::MaxLog, &mut llr).unwrap();
                errors += llr
                    .iter()
                    .zip(&bits)
                    .filter(|(l, &b)| (**l < 0.0) as u8 != b)
                    .count();
            }
            assert!(errors * 100 < 4000 * q, "{m:?}: {errors} bit errors");
        }
    }

    /// Profile `x` through the chain at symbol level with complex noise of variance `nv`: (good, wrong, gaps, bad words).
    fn run(x: usize, frames: usize, nv: f32) -> (NumberedCheck, Counters) {
        let p = Profile::from_index(x).unwrap();
        let pattern = Numbered::TERRESTRIAL;
        let mut tx = TxChain::new(p, Box::new(NumberedSource::new(pattern))).unwrap();
        let mut rx = FrameDecoder::new(p).unwrap();
        let mut rng = Rng::new(x as u64);
        let mut sym = vec![Complex32::new(0.0, 0.0); DATA];
        let var = vec![nv; DATA];
        let mut packets = Vec::new();
        let mut check = NumberedCheck::default();
        for f in 0..p.interleaver.delay_frames() + frames {
            tx.frame(&mut sym).unwrap();
            let s = (nv / 2.0).sqrt();
            for v in sym.iter_mut() {
                *v += Complex32::new(s * rng.normal_f32(), s * rng.normal_f32());
            }
            packets.clear();
            rx.push(f as f64, &sym, &var, &mut packets);
            for (t, pk) in &packets {
                check.push(&pattern, pk, *t);
            }
        }
        (check, rx.counters)
    }

    #[test]
    fn the_decoder_meets_the_middle_points() {
        // 4QAM at rates 0.4, 0.6, 0.8 (indices 5, 7, 9): 400 words each at 2.4, 2.6 and 3.6 dB Eb/N0 over the information
        // bits, through the staged decoding with its give-up rules.
        for (x, ebn0) in [(5, 2.4), (7, 2.6), (9, 3.6)] {
            let p = Profile::from_index(x).unwrap();
            let es = 2.0 * p.rate.k() as f64 / WORD as f64 * 10f64.powf(ebn0 / 10.0);
            let (c, k) = run(x, 400, (1.0 / es) as f32);
            assert_eq!(
                (c.wrong, k.bad),
                (0, 0),
                "index {x} at {ebn0} dB: {c:?} {k:?}"
            );
            assert!(k.good >= 398, "index {x}: {k:?}");
        }
    }

    #[test]
    fn every_profile_decodes_at_symbol_level() {
        for x in SI_INDICES {
            let p = Profile::from_index(x).unwrap();
            let per = p.packets_per_frame() as u64;
            let (c, k) = run(x, 40, 1e-6);
            assert_eq!(
                (c.wrong, c.flagged, c.gaps, k.bad),
                (0, 0, 0, 0),
                "index {x}: {c:?} {k:?}"
            );
            assert!(
                c.good >= 40 * per - 2 * per - 8,
                "index {x}: {} packets",
                c.good
            );
        }
    }
}
