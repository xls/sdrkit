//! The Inmarsat-C frame layer: unique word, row interleaver, the K = 7 code (0x6D, 0x4F), the group scrambler, and the
//! packet framing. A frame is 64 rows of 162 symbols: 2 unique word symbols (the same bit twice), then 160 coded symbols.

use sdrkit_algo_fec::conv::{codes::INMARSAT_K7, Encoder, Ends, Viterbi};
use sdrkit_algo_fec::llr::Quantizer;
use sdrkit_util_bits::seq::INMARSAT_C;

pub const ROWS: usize = 64;
pub const ROW_SYMBOLS: usize = 162;
pub const FRAME_SYMBOLS: usize = ROWS * ROW_SYMBOLS; // 10 368
pub const CODED: usize = ROWS * 160; // 10 240
pub const FRAME_BYTES: usize = 640;
pub const SYMBOL_RATE: f64 = 1200.0;
/// The information bits of a frame (640 bytes, the last one the flush byte).
pub const FRAME_BITS: usize = FRAME_BYTES * 8;

/// The unique word, one bit per transmitted row.
pub const UW: [u8; 64] = {
    let s = b"0000011111101010110011011101101001001110001011110010100011000010";
    let mut u = [0u8; 64];
    let mut i = 0;
    while i < 64 {
        u[i] = s[i] - b'0';
        i += 1;
    }
    u
};

/// The interleaver row carried by transmitted row `j`.
pub const fn row_of(j: usize) -> usize {
    (39 * j) % 64
}

/// The transmitted row that carries interleaver row `i`.
pub const fn inverse_row(i: usize) -> usize {
    (23 * i) % 64
}

/// The scrambler's group mask: group g (4 bytes) is complemented when bit g is set. The bits are the first 160 outputs of the
/// 8-stage register `a(n+8) = a(n) + a(n+2) + a(n+3) + a(n+4)` of the Inmarsat-C SDM (`sdrkit_util_bits::seq::INMARSAT_C`).
pub fn scramble_groups() -> [u8; 160] {
    let mut out = [0u8; 160];
    let mut r = INMARSAT_C;
    r.fill_bits(&mut out);
    out
}

/// Scrambles (or descrambles) 640 bytes in place; the flush byte (the last) is set to zero.
pub fn scramble(bytes: &mut [u8; FRAME_BYTES]) {
    let g = scramble_groups();
    for (k, b) in bytes.iter_mut().enumerate() {
        if g[k / 4] == 1 {
            *b = !*b;
        }
    }
    bytes[FRAME_BYTES - 1] = 0;
}

/// The 10 368 channel symbols (as bits, 0 or 1) of a frame of 639 bytes of packets (the flush byte is added).
pub fn encode_frame(packets: &[u8]) -> Vec<u8> {
    let mut bytes = [0u8; FRAME_BYTES];
    let n = packets.len().min(FRAME_BYTES - 1);
    bytes[..n].copy_from_slice(&packets[..n]);
    scramble(&mut bytes);
    let mut bits = Vec::with_capacity(FRAME_BITS);
    for b in bytes {
        for i in 0..8 {
            bits.push((b >> i) & 1);
        }
    }
    let mut coded = Vec::with_capacity(CODED);
    Encoder::new(INMARSAT_K7).encode(&bits, &mut coded);
    let mut out = vec![0u8; FRAME_SYMBOLS];
    for j in 0..ROWS {
        let u = UW[j];
        out[j * ROW_SYMBOLS] = u;
        out[j * ROW_SYMBOLS + 1] = u;
        let ir = row_of(j);
        for c in 0..160 {
            out[j * ROW_SYMBOLS + 2 + c] = coded[c * 64 + ir];
        }
    }
    out
}

/// The result of decoding a frame.
#[derive(Clone, Debug)]
pub struct FrameOut {
    pub bytes: [u8; FRAME_BYTES],
    /// Channel symbol errors found by re-encoding.
    pub symbol_errors: usize,
}

/// The frame decoder: deinterleaver, the K = 7 Viterbi decoder and the descrambler, with buffers made once.
pub struct FrameDecoder {
    viterbi: Viterbi,
    soft: Vec<f32>,
    llr: Vec<i8>,
    bits: Vec<u8>,
}

impl Default for FrameDecoder {
    fn default() -> Self {
        FrameDecoder {
            viterbi: Viterbi::new(INMARSAT_K7, FRAME_BITS).expect("the Inmarsat-C code is valid"),
            soft: vec![0.0; CODED],
            llr: vec![0; CODED],
            bits: vec![0; FRAME_BITS],
        }
    }
}

impl FrameDecoder {
    /// Decodes 10 368 soft symbols (positive = 1, unique word rows included, polarity already right). The frame starts and ends in
    /// state 0: the last 6 bits of the flush byte are the tail. Returns `None` when `soft` is not one frame or not finite.
    pub fn decode(&mut self, soft: &[f32]) -> Option<FrameOut> {
        if soft.len() != FRAME_SYMBOLS {
            return None;
        }
        for j in 0..ROWS {
            let ir = row_of(j);
            for c in 0..160 {
                // The decoder takes LLRs with positive meaning 0.
                self.soft[c * 64 + ir] = -soft[j * ROW_SYMBOLS + 2 + c];
            }
        }
        Quantizer::default()
            .quantize_auto(&self.soft, &mut self.llr)
            .ok()?;
        let tail = FRAME_BITS - 6;
        let d = self
            .viterbi
            .decode(&self.llr, Ends::Zero, &mut self.bits[..tail])
            .ok()?;
        self.bits[tail..].fill(0);
        let mut bytes = [0u8; FRAME_BYTES];
        for (k, b) in bytes.iter_mut().enumerate() {
            for i in 0..8 {
                *b |= self.bits[k * 8 + i] << i;
            }
        }
        scramble(&mut bytes);
        Some(FrameOut {
            bytes,
            symbol_errors: d.corrected as usize,
        })
    }
}

/// The length of the packet starting with `desc` (and its second byte), or `None` for a long descriptor.
pub fn packet_len(desc: u8, second: u8) -> Option<usize> {
    if desc & 0x80 == 0 {
        Some((desc & 0x0F) as usize + 1)
    } else if desc & 0xC0 == 0x80 {
        Some(second as usize + 2)
    } else {
        None
    }
}

/// Splits a frame's 639 bytes into packets: (descriptor, bytes, check good). A zero descriptor ends them.
pub fn split_packets(bytes: &[u8]) -> Vec<(u8, &[u8], bool)> {
    let mut out = Vec::new();
    let mut p = 0;
    let end = bytes.len().min(FRAME_BYTES - 1);
    while p + 2 < end {
        let d = bytes[p];
        if d == 0 {
            break;
        }
        let Some(len) = packet_len(d, bytes[p + 1]) else {
            break;
        };
        if len < 3 || p + len > end {
            break;
        }
        let pk = &bytes[p..p + len];
        out.push((d, pk, check_ok(pk)));
        p += len;
    }
    out
}

/// The packet check bytes (Inmarsat-C SDM) of `packet`, whose last two bytes are the check (counted as zero): two running sums
/// modulo 2^16, then `c1 = s0 - s1` and `c2 = s1 - 2 s0`, each modulo 256.
pub fn check_bytes(packet: &[u8]) -> (u8, u8) {
    let n = packet.len();
    let (mut s0, mut s1) = (0u32, 0u32);
    for (i, &b) in packet.iter().enumerate() {
        let v = if i + 2 >= n { 0 } else { b as u32 };
        s0 = (s0 + v) & 0xFFFF;
        s1 = (s1 + s0) & 0xFFFF;
    }
    let c1 = (s0.wrapping_sub(s1) & 0xFF) as u8;
    let c2 = (s1.wrapping_sub(2 * s0) & 0xFF) as u8;
    (c1, c2)
}

/// Whether the packet's own check bytes are right.
pub fn check_ok(packet: &[u8]) -> bool {
    let n = packet.len();
    if n < 3 {
        return false;
    }
    let (a, b) = check_bytes(packet);
    packet[n - 2] == a && packet[n - 1] == b
}

/// Sets the check bytes of a packet (its last two).
pub fn seal(packet: &mut [u8]) {
    let n = packet.len();
    if n < 3 {
        return;
    }
    let (a, b) = check_bytes(packet);
    packet[n - 2] = a;
    packet[n - 1] = b;
}

/// The unique word errors of a frame of soft symbols (both symbols of each row against the unique word, in this polarity).
pub fn uw_errors(soft: &[f32], invert: bool) -> usize {
    let mut e = 0;
    for j in 0..ROWS {
        let want = (UW[j] == 1) ^ invert;
        for k in 0..2 {
            if (soft[j * ROW_SYMBOLS + k] > 0.0) != want {
                e += 1;
            }
        }
    }
    e
}

/// The best fit with one polarity flip inside a frame: (start inverted, flip before row k or 64 for none, errors).
pub fn flip_fit(soft: &[f32]) -> (bool, usize, usize) {
    let mut per_row = [[0usize; 2]; ROWS];
    for (j, pr) in per_row.iter_mut().enumerate() {
        for inv in 0..2 {
            let want = (UW[j] == 1) ^ (inv == 1);
            pr[inv] = (0..2)
                .filter(|&k| (soft[j * ROW_SYMBOLS + k] > 0.0) != want)
                .count();
        }
    }
    let plain = |inv: usize| per_row.iter().map(|r| r[inv]).sum::<usize>();
    let (p0, p1) = (plain(0), plain(1));
    let mut best = if p0 <= p1 {
        (false, ROWS, p0)
    } else {
        (true, ROWS, p1)
    };
    let plain_best = best.2;
    for inv in 0..2 {
        // Rows before k in polarity inv, from k on the other.
        let mut before = 0;
        let total_other: usize = per_row.iter().map(|r| r[1 - inv]).sum();
        let mut after = total_other;
        for k in 1..ROWS {
            before += per_row[k - 1][inv];
            after -= per_row[k - 1][1 - inv];
            let e = before + after;
            if e + 8 <= plain_best && e < best.2 {
                best = (inv == 1, k, e);
            }
        }
    }
    best
}

#[cfg(test)]
mod tests {
    use super::*;
    use sdrkit_algo_sim::rng::Rng;

    #[test]
    fn known_answers() {
        assert_eq!(&UW[..16], &[0, 0, 0, 0, 0, 1, 1, 1, 1, 1, 1, 0, 1, 0, 1, 0]);
        assert_eq!((row_of(0), row_of(1), row_of(2)), (0, 39, 14));
        let mut seen = [false; 64];
        for j in 0..64 {
            assert_eq!(inverse_row(row_of(j)), j);
            seen[row_of(j)] = true;
        }
        assert!(seen.iter().all(|&s| s));
    }

    #[test]
    fn clean_frame_round_trip_and_inverted() {
        let mut rng = Rng::new(1);
        let data: Vec<u8> = (0..639).map(|_| rng.next_u32() as u8).collect();
        let sym = encode_frame(&data);
        let soft: Vec<f32> = sym
            .iter()
            .map(|&b| if b == 1 { 1.0 } else { -1.0 })
            .collect();
        let mut dec = FrameDecoder::default();
        let f = dec.decode(&soft).unwrap();
        assert_eq!(&f.bytes[..639], &data[..]);
        assert_eq!(f.symbol_errors, 0);
        assert_eq!(uw_errors(&soft, false), 0);
        let inv: Vec<f32> = soft.iter().map(|x| -x).collect();
        assert_eq!(uw_errors(&inv, false), 128);
        // A flip before row 20 is found with 0 errors.
        let mut fl = soft.clone();
        for x in fl[20 * ROW_SYMBOLS..].iter_mut() {
            *x = -*x;
        }
        assert_eq!(flip_fit(&fl), (false, 20, 0));
        assert_eq!(flip_fit(&soft).1, ROWS);
        assert!(dec.decode(&soft[1..]).is_none());
    }
}
