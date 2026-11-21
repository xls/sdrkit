//! The UMTS broadcast channel: the BCH transport channel coding of TS 25.212 (CRC, convolutional code, the two interleavers and
//! the mapping on the P-CCPCH), and the SYSTEM INFORMATION (BCH) message of TS 25.331 with the two blocks read: the master
//! information block (the PLMN) and system information block type 3 (the cell identity).
//!
//! Standard: 3GPP TS 25.212 4.2.1, 4.2.3.1, 4.2.5, 4.2.11, 4.2.13.6; TS 25.211 5.3.3.3; TS 25.331 (Release 1999 ASN.1, unaligned
//! PER).

use sdrkit_algo_fec::conv::{codes::UMTS_K9, Encoder, Ends, Viterbi};
use sdrkit_util_bits::crc::catalog::CRC16_XMODEM;
use sdrkit_util_bits::pack::{field, push_u64};
use std::collections::HashMap;

/// Bits of a BCH transport block (the predefined BCH format of TS 25.331; not confirmed against the text).
pub const BLOCK_BITS: usize = 246;
/// Bits of the block with its CRC.
pub const CRC_BLOCK_BITS: usize = BLOCK_BITS + 16;
/// Channel bits of a P-CCPCH frame.
pub const FRAME_BITS: usize = 270;
/// The inter-column permutation of the second interleaver (TS 25.212 4.2.11).
pub const P2: [usize; 30] = [
    0, 20, 10, 5, 15, 25, 3, 13, 23, 8, 18, 28, 1, 11, 21, 6, 16, 26, 4, 14, 24, 19, 9, 29, 12, 2, 7, 22, 27, 17,
];

/// The 16 CRC bits sent after the data: the remainder `p1 .. p16` (`p1` the coefficient of `D^15`) of gCRC16, sent `p16` first
/// (TS 25.212 4.2.1.2).
pub fn crc_bits(data: &[u8]) -> [u8; 16] {
    let r = CRC16_XMODEM.bits(data);
    let mut out = [0u8; 16];
    for (k, o) in out.iter_mut().enumerate() {
        // Sent bit k is p(16 - k), the coefficient of D^k.
        *o = ((r >> k) & 1) as u8;
    }
    out
}

/// The two frames of channel bits (270 each) of a 246-bit transport block.
pub fn encode(block: &[u8]) -> [[u8; FRAME_BITS]; 2] {
    let mut bits = block[..BLOCK_BITS].to_vec();
    bits.extend_from_slice(&crc_bits(&bits));
    let mut enc = Encoder::new(UMTS_K9);
    let mut coded = Vec::with_capacity(2 * FRAME_BITS);
    enc.encode(&bits, &mut coded);
    enc.terminate(&mut coded);
    // First interleaving (2 columns): frame 1 the even coded bits, frame 2 the odd ones.
    let mut frames = [[0u8; FRAME_BITS]; 2];
    for (f, fr) in frames.iter_mut().enumerate() {
        let col: Vec<u8> = coded.iter().skip(f).step_by(2).copied().collect();
        *fr = interleave2(&col);
    }
    frames
}

/// The second interleaver: 30 columns of 9 rows, written by rows, the columns permuted by [`P2`], read by columns.
fn interleave2(input: &[u8]) -> [u8; FRAME_BITS] {
    let mut out = [0u8; FRAME_BITS];
    for c in 0..30 {
        for r in 0..9 {
            out[9 * c + r] = input[30 * r + P2[c]];
        }
    }
    out
}

/// The inverse of the second interleaver on soft values.
fn deinterleave2(input: &[f32]) -> [f32; FRAME_BITS] {
    let mut out = [0f32; FRAME_BITS];
    for c in 0..30 {
        for r in 0..9 {
            out[30 * r + P2[c]] = input[9 * c + r];
        }
    }
    out
}

/// Decodes the soft channel bits of the two frames of a TTI (positive means 0) into a transport block. `None` when its CRC is
/// wrong.
pub fn decode(frame1: &[f32], frame2: &[f32]) -> Option<Vec<u8>> {
    let a = deinterleave2(frame1);
    let b = deinterleave2(frame2);
    let peak = a.iter().chain(&b).fold(0f32, |m, v| m.max(v.abs())).max(1e-12);
    let q = |v: f32| (v / peak * 100.0).round().clamp(-127.0, 127.0) as i8;
    let mut llr = Vec::with_capacity(2 * FRAME_BITS);
    for i in 0..FRAME_BITS {
        llr.push(q(a[i]));
        llr.push(q(b[i]));
    }
    let mut v = Viterbi::new(UMTS_K9, FRAME_BITS).ok()?;
    let mut out = vec![0u8; CRC_BLOCK_BITS];
    v.decode_only(&llr, Ends::Zero, &mut out).ok()?;
    (crc_bits(&out[..BLOCK_BITS]) == out[BLOCK_BITS..]).then(|| out[..BLOCK_BITS].to_vec())
}

/// The SIB types read.
pub const SIB_MIB: u8 = 0;
/// System information block type 3.
pub const SIB_3: u8 = 3;

/// What the master information block names.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Mib {
    /// MCC.
    pub mcc: u16,
    /// MNC.
    pub mnc: u16,
    /// Digits of the MNC (2 or 3).
    pub mnc_digits: u8,
}

/// The leading fields of the MIB: the PLMN identity of a gsm-MAP or gsm-MAP-and-ANSI-41 PLMN type.
pub fn parse_mib(d: &[u8]) -> Option<Mib> {
    let plmn_type = field(d, 4, 2)?;
    if plmn_type != 0 && plmn_type != 2 {
        return None;
    }
    let digit = |at: usize| field(d, at, 4).filter(|&v| v <= 9).map(|v| v as u16);
    let mcc = 100 * digit(6)? + 10 * digit(10)? + digit(14)?;
    let three = field(d, 18, 1)? == 1;
    let mut mnc = 0;
    for k in 0..if three { 3 } else { 2 } {
        mnc = 10 * mnc + digit(19 + 4 * k)?;
    }
    Some(Mib {
        mcc,
        mnc,
        mnc_digits: if three { 3 } else { 2 },
    })
}

/// The cell identity (28 bits) of SIB 3.
pub fn parse_sib3(d: &[u8]) -> Option<u32> {
    field(d, 2, 28).map(|v| v as u32)
}

/// One SIB or segment of a SYSTEM INFORMATION message.
#[derive(Clone, Debug, PartialEq, Eq)]
enum Part {
    /// A whole SIB.
    Complete(u8, Vec<u8>),
    /// The first segment of a SIB of `count` segments.
    First(u8, usize, Vec<u8>),
    /// Segment `index` (1 ..) of a SIB.
    Subsequent(u8, usize, Vec<u8>),
    /// The last segment, index `index`.
    Last(u8, usize, Vec<u8>),
}

/// A reader of unaligned PER fields over a bit slice.
struct Bits<'a> {
    d: &'a [u8],
    at: usize,
}

impl Bits<'_> {
    fn u(&mut self, n: usize) -> Option<usize> {
        let v = field(self.d, self.at, n as u32)? as usize;
        self.at += n;
        Some(v)
    }
    fn take(&mut self, n: usize) -> Option<Vec<u8>> {
        let s = self.d.get(self.at..self.at + n)?.to_vec();
        self.at += n;
        Some(s)
    }
}

fn first_short(b: &mut Bits) -> Option<Part> {
    let t = b.u(5)? as u8;
    let count = b.u(4)? + 1;
    let len = b.u(8)? + 1;
    Some(Part::First(t, count, b.take(len)?))
}

fn last_short(b: &mut Bits) -> Option<Part> {
    let t = b.u(5)? as u8;
    let index = b.u(4)? + 1;
    let len = b.u(8)? + 1;
    Some(Part::Last(t, index, b.take(len)?))
}

fn complete_list(b: &mut Bits, out: &mut Vec<Part>) -> Option<()> {
    let n = b.u(4)? + 1;
    for _ in 0..n {
        let t = b.u(5)? as u8;
        let len = b.u(8)? + 1;
        out.push(Part::Complete(t, b.take(len)?));
    }
    Some(())
}

/// The SFN of the first frame of the TTI and the parts of a SYSTEM INFORMATION (BCH) message.
fn parse_message(block: &[u8]) -> Option<(u32, Vec<Part>)> {
    let mut b = Bits { d: block, at: 0 };
    let sfn = 2 * b.u(11)? as u32;
    let mut parts = Vec::new();
    match b.u(4)? {
        0 => {}
        1 => {
            let t = b.u(5)? as u8;
            let count = b.u(4)? + 1;
            parts.push(Part::First(t, count, b.take(222)?));
        }
        2 => {
            let t = b.u(5)? as u8;
            let index = b.u(4)? + 1;
            parts.push(Part::Subsequent(t, index, b.take(222)?));
        }
        3 => parts.push(last_short(&mut b)?),
        4 => {
            parts.push(last_short(&mut b)?);
            parts.push(first_short(&mut b)?);
        }
        5 => {
            parts.push(last_short(&mut b)?);
            complete_list(&mut b, &mut parts)?;
        }
        6 => {
            parts.push(last_short(&mut b)?);
            complete_list(&mut b, &mut parts)?;
            parts.push(first_short(&mut b)?);
        }
        7 => complete_list(&mut b, &mut parts)?,
        8 => {
            complete_list(&mut b, &mut parts)?;
            parts.push(first_short(&mut b)?);
        }
        9 => {
            let t = b.u(5)? as u8;
            parts.push(Part::Complete(t, b.take(226)?));
        }
        10 => {
            let t = b.u(5)? as u8;
            let index = b.u(4)? + 1;
            parts.push(Part::Last(t, index, b.take(222)?));
        }
        _ => return None,
    }
    Some((sfn, parts))
}

/// A whole SIB: its type and its data bits.
pub type Sib = (u8, Vec<u8>);

/// The SIBs of a stream of transport blocks: segments joined in order, a SIB complete when its segment count is reached or a last
/// segment arrives; a segment out of order discards the SIB.
#[derive(Clone, Debug, Default)]
pub struct SibAssembly {
    pending: HashMap<u8, (usize, usize, Vec<u8>)>,
}

impl SibAssembly {
    /// Takes one transport block; returns the SFN of its TTI and the SIBs it completed (type, data).
    pub fn push(&mut self, block: &[u8]) -> Option<(u32, Vec<Sib>)> {
        let (sfn, parts) = parse_message(block)?;
        let mut done = Vec::new();
        for p in parts {
            let (t, index, d, last) = match p {
                Part::Complete(t, d) => {
                    done.push((t, d));
                    continue;
                }
                Part::First(t, count, d) => {
                    self.pending.insert(t, (count, 1, d));
                    continue;
                }
                Part::Subsequent(t, i, d) => (t, i, d, false),
                Part::Last(t, i, d) => (t, i, d, true),
            };
            let Some(e) = self.pending.get_mut(&t) else {
                continue;
            };
            if index != e.1 {
                self.pending.remove(&t);
                continue;
            }
            e.1 += 1;
            e.2.extend_from_slice(&d);
            if last || e.1 >= e.0 {
                if let Some((_, _, all)) = self.pending.remove(&t) {
                    done.push((t, all));
                }
            }
        }
        Some((sfn, done))
    }
}

/// A transport block holding one complete SIB of type `sib` with `data`, for the TTI starting at frame `sfn`.
pub fn complete_sib_block(sfn: u32, sib: u8, data: &[u8]) -> Vec<u8> {
    let mut b = Vec::with_capacity(BLOCK_BITS);
    push_u64(&mut b, u64::from((sfn / 2) % 2048), 11);
    push_u64(&mut b, 9, 4);
    push_u64(&mut b, u64::from(sib), 5);
    b.extend_from_slice(&data[..data.len().min(226)]);
    b.resize(BLOCK_BITS, 0);
    b
}

/// The data of a MIB: no extensions, value tag 1, gsm-MAP, the PLMN identity.
pub fn mib_data(m: &Mib) -> Vec<u8> {
    let mut b = Vec::new();
    push_u64(&mut b, 0, 1);
    push_u64(&mut b, 0, 3);
    push_u64(&mut b, 0, 2);
    for d in [m.mcc / 100, m.mcc / 10 % 10, m.mcc % 10] {
        push_u64(&mut b, u64::from(d), 4);
    }
    push_u64(&mut b, u64::from(m.mnc_digits == 3), 1);
    let digits: Vec<u16> = if m.mnc_digits == 3 {
        vec![m.mnc / 100, m.mnc / 10 % 10, m.mnc % 10]
    } else {
        vec![m.mnc / 10 % 10, m.mnc % 10]
    };
    for d in digits {
        push_u64(&mut b, u64::from(d), 4);
    }
    b
}

/// The data of a SIB 3: no extensions, sib4indicator 0, the cell identity.
pub fn sib3_data(cell: u32) -> Vec<u8> {
    let mut b = Vec::new();
    push_u64(&mut b, 0, 2);
    push_u64(&mut b, u64::from(cell & 0x0FFF_FFFF), 28);
    b
}
