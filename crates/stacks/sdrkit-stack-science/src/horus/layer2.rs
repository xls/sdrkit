//! Horus Binary layer 2: the unique word, Golay (23,12) parity of the payload, the multiplicative bit interleaver and the
//! additive scrambler (Project Horus horusdemodlib wiki, "2 Modem Details"; the constants as section 3.4 of the
//! specification gives them).

use sdrkit_algo_fec::small::{Chase, SmallCode};
use sdrkit_util_bits::interleave::Permutation;
use sdrkit_util_bits::lfsr::Lfsr;

/// The unique word before every frame.
pub const UNIQUE_WORD: [u8; 2] = [0x24, 0x24];
/// The preamble byte.
pub const PREAMBLE: u8 = 0x1B;
/// The payload of Horus Binary v1, bytes.
pub const V1_PAYLOAD: usize = 22;
/// The payload of Horus Binary v2, bytes.
pub const V2_PAYLOAD: usize = 32;
/// The seed of the scrambler.
const SCRAMBLER_SEED: u64 = 0x4A80;

/// Golay code words of a payload of `n` bytes: one for each 12 bits.
pub fn codewords(n: usize) -> usize {
    (8 * n).div_ceil(12)
}

/// The bytes of the frame of a payload of `n` bytes, the unique word included: 45 for v1, 65 for v2.
pub fn frame_len(n: usize) -> usize {
    (16 + 8 * n + 11 * codewords(n)).div_ceil(8)
}

/// The step of the interleaver for a frame body of `nb` bytes: 337 for 43 bytes, 389 for 63. `None` for another size.
pub fn interleaver_step(nb: usize) -> Option<usize> {
    match nb {
        43 => Some(337),
        63 => Some(389),
        _ => None,
    }
}

/// The coder of one payload size.
#[derive(Clone, Debug)]
pub struct Layer2 {
    n: usize,
    golay: SmallCode,
    perm: Permutation,
    scrambler: Vec<u8>,
}

impl Layer2 {
    /// The coder of payloads of `n` bytes (22 or 32). `None` for another size.
    pub fn new(n: usize) -> Option<Layer2> {
        let nb = frame_len(n) - 2;
        let b = interleaver_step(nb)?;
        let perm = Permutation::multiplicative(b, 0, 8 * nb).ok()?;
        // o = bit 1 xor bit 0 of the state, shifted in at bit 14: the recurrence x(i + 15) = x(i) + x(i + 1), whose outputs
        // after its first 15 are the scrambler's.
        let mut lfsr = Lfsr::recurrence(15, 0b11, SCRAMBLER_SEED);
        lfsr.skip(15);
        let mut scrambler = vec![0u8; 8 * nb];
        lfsr.fill_bits(&mut scrambler);
        Some(Layer2 {
            n,
            golay: SmallCode::golay_23_12(),
            perm,
            scrambler,
        })
    }

    /// The payload size, bytes.
    pub fn payload_len(&self) -> usize {
        self.n
    }

    /// The bytes after the unique word.
    pub fn body_len(&self) -> usize {
        frame_len(self.n) - 2
    }

    /// The frame of `payload` (n bytes): the unique word, then the coded body.
    pub fn encode(&self, payload: &[u8]) -> Vec<u8> {
        let mut frame = vec![0u8; frame_len(self.n)];
        frame[..2].copy_from_slice(&UNIQUE_WORD);
        frame[2..2 + self.n].copy_from_slice(&payload[..self.n]);
        let data = msb_bits(&payload[..self.n]);
        let mut at = (2 + self.n) * 8;
        for j in 0..codewords(self.n) {
            let word = self.golay.encode(group(&data, j));
            for k in (0..11).rev() {
                set_msb(&mut frame, at, (word >> k) as u8 & 1);
                at += 1;
            }
        }
        let body = self.whiten(&self.interleave(&frame[2..]));
        frame[2..].copy_from_slice(&body);
        frame
    }

    /// Decodes the body after the unique word (43 or 63 bytes): the payload and the bits that Golay corrected, or `None` when a
    /// code word has more than 3 errors.
    pub fn decode(&self, body: &[u8]) -> Option<(Vec<u8>, usize)> {
        let nb = self.body_len();
        if body.len() < nb {
            return None;
        }
        let plain = self.deinterleave(&self.whiten(&body[..nb]));
        let mut data = msb_bits(&plain[..self.n]);
        let mut fixed = 0;
        for j in 0..codewords(self.n) {
            let mut parity = 0u128;
            for k in 0..11 {
                parity = (parity << 1) | u128::from(get_msb(&plain, 8 * self.n + 11 * j + k));
            }
            let word = (group(&data, j) << 11) | parity;
            let (cw, errs) = self.golay.decode(word)?;
            fixed += errs as usize;
            let info = self.golay.info(cw);
            for k in 0..12 {
                if let Some(b) = data.get_mut(12 * j + k) {
                    *b = (info >> (11 - k)) as u8 & 1;
                }
            }
        }
        let mut payload = vec![0u8; self.n];
        for (i, b) in data.iter().enumerate() {
            payload[i / 8] |= b << (7 - i % 8);
        }
        Some((payload, fixed))
    }

    /// As [`Layer2::decode`], with the reliability of each bit of the body (its |LLR|, the bits numbered least significant
    /// first within each byte): each code word is decoded by a Chase search over its weakest bits.
    pub fn decode_soft(&self, body: &[u8], rel: &[f32]) -> Option<(Vec<u8>, usize)> {
        let nb = self.body_len();
        if body.len() < nb || rel.len() < 8 * nb {
            return None;
        }
        let plain = self.deinterleave(&self.whiten(&body[..nb]));
        let at = |msb_index: usize| {
            let lsb = (msb_index / 8) * 8 + 7 - msb_index % 8;
            self.perm.dest(lsb).map_or(0.0, |d| rel[d])
        };
        let mut data = msb_bits(&plain[..self.n]);
        let mut fixed = 0;
        let mut r = [0f32; 23];
        for j in 0..codewords(self.n) {
            let mut parity = 0u128;
            for k in 0..11 {
                let q = 8 * self.n + 11 * j + k;
                parity = (parity << 1) | u128::from(get_msb(&plain, q));
                r[12 + k] = at(q);
            }
            for (k, rk) in r.iter_mut().take(12).enumerate() {
                let p = 12 * j + k;
                // Bits past the payload are known zeros.
                *rk = if p < 8 * self.n { at(p) } else { f32::MAX };
            }
            let word = (group(&data, j) << 11) | parity;
            let (cw, errs, _) = self.golay.decode_soft(word, &r, Chase::default())?;
            fixed += errs as usize;
            let info = self.golay.info(cw);
            for k in 0..12 {
                if let Some(b) = data.get_mut(12 * j + k) {
                    *b = (info >> (11 - k)) as u8 & 1;
                }
            }
        }
        let mut payload = vec![0u8; self.n];
        for (i, b) in data.iter().enumerate() {
            payload[i / 8] |= b << (7 - i % 8);
        }
        Some((payload, fixed))
    }

    /// The scrambler applied to `bytes`, bits numbered least significant first. It is its own inverse.
    fn whiten(&self, bytes: &[u8]) -> Vec<u8> {
        let mut out = bytes.to_vec();
        for (i, &s) in self.scrambler.iter().enumerate() {
            out[i / 8] ^= s << (i % 8);
        }
        out
    }

    /// Bit `i` (least significant first within each byte) moves to `(b i) mod N`.
    fn interleave(&self, bytes: &[u8]) -> Vec<u8> {
        let bits = lsb_bits(bytes);
        let mut out = vec![0u8; bits.len()];
        let _ = self.perm.apply(&bits, &mut out);
        lsb_bytes(&out)
    }

    /// Output bit `i` takes input bit `(b i) mod N`.
    fn deinterleave(&self, bytes: &[u8]) -> Vec<u8> {
        let bits = lsb_bits(bytes);
        let mut out = vec![0u8; bits.len()];
        let _ = self.perm.invert(&bits, &mut out);
        lsb_bytes(&out)
    }
}

/// The 12 data bits of code word `j`, the first the most significant; bits past the payload are 0.
fn group(data: &[u8], j: usize) -> u128 {
    (0..12).fold(0u128, |a, k| {
        (a << 1) | u128::from(data.get(12 * j + k).copied().unwrap_or(0))
    })
}

/// The bits of `bytes`, each byte most significant bit first.
pub fn msb_bits(bytes: &[u8]) -> Vec<u8> {
    bytes
        .iter()
        .flat_map(|&b| (0..8).rev().map(move |k| (b >> k) & 1))
        .collect()
}

fn lsb_bits(bytes: &[u8]) -> Vec<u8> {
    bytes
        .iter()
        .flat_map(|&b| (0..8).map(move |k| (b >> k) & 1))
        .collect()
}

fn lsb_bytes(bits: &[u8]) -> Vec<u8> {
    bits.chunks(8)
        .map(|c| c.iter().enumerate().fold(0u8, |a, (k, &b)| a | (b << k)))
        .collect()
}

fn get_msb(bytes: &[u8], i: usize) -> u8 {
    (bytes[i / 8] >> (7 - i % 8)) & 1
}

fn set_msb(bytes: &mut [u8], i: usize, v: u8) {
    let m = 0x80 >> (i % 8);
    if v != 0 {
        bytes[i / 8] |= m;
    } else {
        bytes[i / 8] &= !m;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_scrambler_is_the_register_of_the_specification() {
        let l = Layer2::new(V1_PAYLOAD).unwrap();
        let mut state = 0x4A80u32;
        for (i, &s) in l.scrambler.iter().enumerate() {
            let o = ((state >> 1) ^ state) & 1;
            assert_eq!(u32::from(s), o, "bit {i}");
            state = (state >> 1) | (o << 14);
        }
    }
}
