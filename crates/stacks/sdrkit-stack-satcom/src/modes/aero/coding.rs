//! Aero P channel coding: frame layouts per rate, the 64-row interleaver, the continuous K = 7 code, the 15-stage scrambler, bytes
//! least significant bit first, and 12-byte signal units with their CRC.
//! Standard: Inmarsat Aeronautical System Definition Manual; ARINC 741.

use sdrkit_algo_fec::conv::codes::INMARSAT_K7;
use sdrkit_algo_fec::conv::{reencode_errors, Encoder, Viterbi};
use sdrkit_algo_fec::llr::Quantizer;
use sdrkit_util_bits::crc::catalog::CRC16_IBM_SDLC;
use sdrkit_util_bits::seq::AERO;
use std::collections::VecDeque;

/// The unique word, sent most significant bit first.
pub const UW: u32 = 0xE15A_E893;

/// A P channel rate's frame layout.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Layout {
    pub rate: u32,
    /// Unique word bits (both arms for 10500).
    pub uw_bits: usize,
    pub header_bits: usize,
    pub unused_bits: usize,
    pub coded_bits: usize,
    /// Interleaver columns per interleaver frame, and the number of them.
    pub cols: usize,
    pub blocks: usize,
    pub frame_s: f64,
}

impl Layout {
    pub fn of(rate: u32) -> Layout {
        match rate {
            600 => Layout {
                rate,
                uw_bits: 32,
                header_bits: 16,
                unused_bits: 0,
                coded_bits: 1152,
                cols: 6,
                blocks: 3,
                frame_s: 2.0,
            },
            1200 => Layout {
                rate,
                uw_bits: 32,
                header_bits: 16,
                unused_bits: 0,
                coded_bits: 1152,
                cols: 9,
                blocks: 2,
                frame_s: 1.0,
            },
            _ => Layout {
                rate: 10500,
                uw_bits: 64,
                header_bits: 16,
                unused_bits: 178,
                coded_bits: 4992,
                cols: 78,
                blocks: 1,
                frame_s: 0.5,
            },
        }
    }
    pub fn frame_bits(&self) -> usize {
        self.uw_bits + self.header_bits + self.unused_bits + self.coded_bits
    }
    pub fn info_bytes(&self) -> usize {
        self.coded_bits / 16
    }
    pub fn sus(&self) -> usize {
        self.info_bytes() / 12
    }
}

/// The interleaver's row permutation: p((27 i) mod 64) = i.
fn perm() -> [usize; 64] {
    let mut p = [0usize; 64];
    for i in 0..64 {
        p[(27 * i) % 64] = i;
    }
    p
}

/// Interleaves one interleaver frame of `64 * n` coded bits: tx[i n + j] = coded[p(i) + 64 j].
pub fn interleave<T: Copy + Default>(coded: &[T], n: usize) -> Vec<T> {
    let p = perm();
    let mut tx = vec![T::default(); 64 * n];
    for i in 0..64 {
        for j in 0..n {
            tx[i * n + j] = coded[p[i] + 64 * j];
        }
    }
    tx
}

/// Deinterleaves one interleaver frame into `out`: out[j 64 + i] = rx[((27 i) mod 64) n + j].
pub fn deinterleave_into<T: Copy>(rx: &[T], n: usize, out: &mut [T]) {
    for i in 0..64 {
        for j in 0..n {
            out[j * 64 + i] = rx[((27 * i) % 64) * n + j];
        }
    }
}

/// The scrambler's output bits for one frame (restarted at every frame): `1 + x + x^15` loaded with 110100101011001
/// (`sdrkit_util_bits::seq::AERO`).
pub fn scrambler(out: &mut [u8]) {
    let mut r = AERO;
    r.fill_bits(out);
}

/// The CRC of a signal unit: CRC-16/IBM-SDLC (the X.25 FCS) over bytes 0 to 9, stored low byte first in 10 and 11.
pub fn su_seal(su: &mut [u8; 12]) {
    let c = CRC16_IBM_SDLC.checksum(&su[..10]);
    su[10] = c as u8;
    su[11] = (c >> 8) as u8;
}

/// Whether a signal unit's CRC is right (an all-zero unit is padding and counts as good).
pub fn su_ok(su: &[u8]) -> bool {
    su.len() == 12
        && (su.iter().all(|&b| b == 0)
            || CRC16_IBM_SDLC.checksum(&su[..10]) == (su[10] as u16 | (su[11] as u16) << 8))
}

/// The 16 header bits: format id 1, superframe marker, two frame counters.
pub fn header(superframe: u8, c1: u8, c2: u8) -> u16 {
    1u16 << 12 | ((superframe as u16) & 15) << 8 | ((c1 as u16) & 15) << 4 | ((c2 as u16) & 15)
}

/// The channel bits (0 or 1) of one frame for `info` bytes. The encoder runs on from frame to frame. Returns `None` when `info`
/// does not have the layout's length.
pub fn encode_frame(l: &Layout, info: &[u8], hdr: u16, enc: &mut Encoder) -> Option<Vec<u8>> {
    if info.len() != l.info_bytes() {
        return None;
    }
    let mut scr = vec![0u8; info.len() * 8];
    scrambler(&mut scr);
    let bits: Vec<u8> = info
        .iter()
        .flat_map(|b| (0..8).map(move |i| (b >> i) & 1))
        .zip(&scr)
        .map(|(b, s)| b ^ s)
        .collect();
    let mut coded = Vec::with_capacity(l.coded_bits);
    enc.encode(&bits, &mut coded);
    let mut out = Vec::with_capacity(l.frame_bits());
    // 10500: the unique word on both arms (bits alternate Q, I).
    for i in (0..32).rev() {
        out.push(((UW >> i) & 1) as u8);
        if l.rate == 10500 {
            out.push(((UW >> i) & 1) as u8);
        }
    }
    for i in (0..16).rev() {
        out.push(((hdr >> i) & 1) as u8);
    }
    out.extend(std::iter::repeat_n(0, l.unused_bits));
    let bl = 64 * l.cols;
    for k in 0..l.blocks {
        out.extend(interleave(&coded[k * bl..(k + 1) * bl], l.cols));
    }
    Some(out)
}

/// The result of decoding a frame's coded part.
#[derive(Clone, Debug, PartialEq)]
pub struct Decoded {
    pub bytes: Vec<u8>,
    /// Channel bit errors (the decided bits re-encoded against the hard channel bits).
    pub channel_errors: usize,
}

/// A frame waiting for its bits.
struct Pending<M> {
    llr: Vec<i8>,
    meta: M,
}

/// The decoder of the continuous code across frames.
///
/// The code runs on from frame to frame, so the last bits of a frame are decided only with the start of the next one: the
/// Viterbi decoder streams with its traceback margin, and a frame comes out when the next frame has been pushed (or at
/// [`FrameDecoder::flush`]). `M` is what the caller keeps with each frame.
pub struct FrameDecoder<M> {
    viterbi: Viterbi,
    pending: VecDeque<Pending<M>>,
    released: Vec<u8>,
    /// The encoder state after the bits given out so far.
    state: u32,
    soft: Vec<f32>,
}

impl<M> Default for FrameDecoder<M> {
    fn default() -> Self {
        FrameDecoder {
            viterbi: Viterbi::new(INMARSAT_K7, 1).expect("the Aero code is valid"),
            pending: VecDeque::new(),
            released: Vec::new(),
            state: 0,
            soft: Vec::new(),
        }
    }
}

impl<M> FrameDecoder<M> {
    /// Starts a new stream: the next frame does not continue the last one. Frames still waiting come out first, decided with a
    /// short window.
    pub fn restart(&mut self, out: &mut Vec<(Decoded, M)>) {
        self.flush(out);
        self.viterbi.reset();
        self.released.clear();
        self.state = 0;
    }

    /// Returns out every waiting frame, its last bits decided from the best state now.
    pub fn flush(&mut self, out: &mut Vec<(Decoded, M)>) {
        if self.pending.is_empty() {
            return;
        }
        self.viterbi.flush(&mut self.released);
        self.release(out);
        self.pending.clear();
    }

    /// Takes the coded soft bits of one frame (positive means 1, interleaved as received) and appends every frame whose bits are
    /// now decided.
    pub fn push(&mut self, l: &Layout, soft: &[f32], meta: M, out: &mut Vec<(Decoded, M)>) {
        if soft.len() != l.coded_bits {
            return;
        }
        let bl = 64 * l.cols;
        self.soft.resize(l.coded_bits, 0.0);
        for k in 0..l.blocks {
            deinterleave_into(
                &soft[k * bl..(k + 1) * bl],
                l.cols,
                &mut self.soft[k * bl..(k + 1) * bl],
            );
        }
        // The decoder takes LLRs with positive meaning 0.
        for v in &mut self.soft {
            *v = -*v;
        }
        let mut llr = vec![0i8; l.coded_bits];
        if Quantizer::default()
            .quantize_auto(&self.soft, &mut llr)
            .is_err()
        {
            return;
        }
        self.viterbi.push(&llr, &mut self.released);
        self.pending.push_back(Pending { llr, meta });
        self.release(out);
    }

    fn release(&mut self, out: &mut Vec<(Decoded, M)>) {
        let mut used = 0;
        while let Some(p) = self.pending.front() {
            let n = p.llr.len() / 2;
            if self.released.len() - used < n {
                break;
            }
            let Some(p) = self.pending.pop_front() else {
                break;
            };
            let bits = &self.released[used..used + n];
            used += n;
            let channel_errors =
                reencode_errors(&INMARSAT_K7, self.state, bits, &p.llr).unwrap_or(0) as usize;
            let mut enc = Encoder::with_state(INMARSAT_K7, self.state);
            for &b in bits {
                enc.push_bit(b);
            }
            self.state = enc.state();
            let mut scr = vec![0u8; n];
            scrambler(&mut scr);
            let bytes = bits
                .chunks(8)
                .zip(scr.chunks(8))
                .map(|(c, s)| {
                    c.iter()
                        .zip(s)
                        .enumerate()
                        .fold(0u8, |v, (i, (&x, &y))| v | (x ^ y) << i)
                })
                .collect();
            out.push((
                Decoded {
                    bytes,
                    channel_errors,
                },
                p.meta,
            ));
        }
        self.released.drain(..used);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn interleaver_round_trip_and_frame_lengths() {
        for r in [600, 1200, 10500] {
            let l = Layout::of(r);
            assert_eq!(l.frame_bits() as f64, r as f64 * l.frame_s, "{r}");
            assert_eq!(l.coded_bits, 64 * l.cols * l.blocks);
            let x: Vec<u32> = (0..64 * l.cols as u32).collect();
            let mut y = vec![0u32; x.len()];
            deinterleave_into(&interleave(&x, l.cols), l.cols, &mut y);
            assert_eq!(y, x);
        }
        assert_eq!(Layout::of(600).sus(), 6);
        assert_eq!(Layout::of(10500).sus(), 26);
        let mut su = [0x40u8, 1, 2, 3, 4, 5, 6, 7, 8, 9, 0, 0];
        su_seal(&mut su);
        assert!(su_ok(&su));
        su[3] ^= 1;
        assert!(!su_ok(&su));
        assert!(su_ok(&[0u8; 12]));
    }
}
