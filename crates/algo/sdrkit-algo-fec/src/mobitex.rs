//! The block coding of the Mobitex air interface: the (12,8) code that guards each byte, the 12 by 20 block interleaver and the
//! additive scrambler of the data blocks.
//!
//! A code word is the 8 message bits, then 4 check bits `c3 c2 c1 c0` (`c3` first). The decoder corrects one wrong bit and
//! reports any other syndrome as uncorrectable. A block is 240 bits: 20 code words, written word by word and sent column by
//! column of a 12 by 20 array.
//!
//! Standard: Mobitex Interface Specification (air interface), Ericsson.
//!
//! ```
//! use sdrkit_algo_fec::mobitex;
//! assert_eq!(mobitex::encode(0x01), 0x015);
//! assert_eq!(mobitex::decode(0x015 ^ 0x100), Some((0x01, 1)));
//! ```

/// The masks of the message bits each check bit covers, `c3` first.
const CHECK_MASKS: [u8; 4] = [0xEC, 0xD3, 0xBA, 0x75];

/// Bits in a block.
pub const BLOCK_BITS: usize = 240;
/// Code words in a block.
pub const BLOCK_WORDS: usize = 20;

/// The 4 check bits of a message byte, `c3` in bit 3.
pub const fn checks(m: u8) -> u8 {
    let mut c = 0u8;
    let mut i = 0;
    while i < 4 {
        c = (c << 1) | ((m & CHECK_MASKS[i]).count_ones() as u8 & 1);
        i += 1;
    }
    c
}

/// The 12-bit code word of a message byte: the byte in bits 11 to 4, the check bits in bits 3 to 0.
pub const fn encode(m: u8) -> u16 {
    ((m as u16) << 4) | checks(m) as u16
}

/// The syndrome of a 12-bit word: 0 for a code word.
fn syndrome(w: u16) -> u8 {
    checks((w >> 4) as u8) ^ (w & 0xF) as u8
}

/// The syndrome of a single wrong bit at position `p` (0 is the least significant bit of the word).
fn column(p: u32) -> u8 {
    syndrome(1 << p)
}

/// Decodes a 12-bit word: the message byte and the bits corrected (0 or 1). `None` when the syndrome is not that of a single
/// wrong bit.
pub fn decode(w: u16) -> Option<(u8, u32)> {
    let w = w & 0xFFF;
    let s = syndrome(w);
    if s == 0 {
        return Some(((w >> 4) as u8, 0));
    }
    let p = (0..12).find(|&p| column(p) == s)?;
    Some((((w ^ (1 << p)) >> 4) as u8, 1))
}

/// Decodes a 12-bit word as [`decode`] does, but keeps the message bits as received when the word is uncorrectable. Returns the
/// byte and whether it is trusted.
pub fn decode_or_keep(w: u16) -> (u8, bool) {
    match decode(w) {
        Some((m, _)) => (m, true),
        None => (((w >> 4) & 0xFF) as u8, false),
    }
}

/// The additive scrambler of the data blocks: a 10-stage register started at `1FF` (hex). Each step outputs stage 0, forms
/// the new bit as stage 1 XOR stage 5 and shifts it in at stage 9. It starts on the first bit of block 0 and runs on through
/// every block of a frame.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Scrambler {
    s: u16,
}

impl Default for Scrambler {
    fn default() -> Self {
        Scrambler::new()
    }
}

impl Scrambler {
    /// The scrambler at the start of a frame.
    pub const fn new() -> Self {
        Scrambler { s: 0x1FF }
    }

    /// The next bit of the sequence.
    pub fn next_bit(&mut self) -> u8 {
        let out = (self.s & 1) as u8;
        let new = ((self.s >> 1) ^ (self.s >> 5)) & 1;
        self.s = (self.s >> 1) | (new << 9);
        out
    }

    /// XORs the sequence onto `bits` (one bit per byte).
    pub fn apply(&mut self, bits: &mut [u8]) {
        for b in bits {
            *b ^= self.next_bit();
        }
    }
}

/// The interleaver index: sent bit `x(k)` of a block carries word bit `y(i)` with `k = (i mod 12) × 20 + i / 12`.
pub const fn sent_index(i: usize) -> usize {
    (i % 12) * BLOCK_WORDS + i / 12
}

/// Undoes the interleaver of one block: `sent` holds the 240 received bits, `words` receives the 20 code words in order.
pub fn deinterleave(sent: &[u8; BLOCK_BITS], words: &mut [u16; BLOCK_WORDS]) {
    for (w, out) in words.iter_mut().enumerate() {
        let mut v = 0u16;
        for j in 0..12 {
            v = (v << 1) | u16::from(sent[sent_index(w * 12 + j)] & 1);
        }
        *out = v;
    }
}

/// Codes and interleaves 20 bytes into the 240 bits of a block, as sent.
pub fn encode_block(bytes: &[u8; BLOCK_WORDS], sent: &mut [u8; BLOCK_BITS]) {
    for (w, &m) in bytes.iter().enumerate() {
        let c = encode(m);
        for j in 0..12 {
            sent[sent_index(w * 12 + j)] = ((c >> (11 - j)) & 1) as u8;
        }
    }
}

/// Decodes the 240 received bits of a block (scrambler already removed) into 20 bytes. Returns the words corrected and the
/// words found uncorrectable (their message bits are kept as received).
pub fn decode_block(sent: &[u8; BLOCK_BITS], bytes: &mut [u8; BLOCK_WORDS]) -> (u32, u32) {
    let mut words = [0u16; BLOCK_WORDS];
    deinterleave(sent, &mut words);
    let (mut fixed, mut bad) = (0, 0);
    for (b, &w) in bytes.iter_mut().zip(&words) {
        match decode(w) {
            Some((m, n)) => {
                *b = m;
                fixed += n;
            }
            None => {
                *b = (w >> 4) as u8;
                bad += 1;
            }
        }
    }
    (fixed, bad)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unit_words_and_single_errors() {
        let want = [0x015, 0x026, 0x049, 0x08A, 0x107, 0x20B, 0x40D, 0x80E];
        for (k, &w) in want.iter().enumerate() {
            assert_eq!(encode(1 << k), w);
        }
        for m in 0..=255u8 {
            let c = encode(m);
            assert_eq!(decode(c), Some((m, 0)));
            for p in 0..12 {
                assert_eq!(decode(c ^ (1 << p)), Some((m, 1)), "{m:02X} bit {p}");
            }
        }
        // The control bytes 3F 02 give the check byte C6.
        assert_eq!((checks(0x3F) << 4) | checks(0x02), 0xC6);
        // A syndrome that is no column is refused.
        let cols: Vec<u8> = (0..12).map(column).collect();
        let other = (1..16u8).find(|s| !cols.contains(s)).unwrap();
        let w = (0..0x1000u16).find(|&w| syndrome(w) == other).unwrap();
        assert_eq!(decode(w), None);
        assert!(!decode_or_keep(w).1);
    }

    #[test]
    fn scrambler_starts_as_specified() {
        let mut s = Scrambler::new();
        let bits: String = (0..32).map(|_| char::from(b'0' + s.next_bit())).collect();
        assert_eq!(bits, "11111111100000111101111100010111");
    }

    #[test]
    fn a_block_round_trips_with_one_error_a_word() {
        let bytes: [u8; 20] = core::array::from_fn(|i| (i * 37 + 5) as u8);
        let mut sent = [0u8; BLOCK_BITS];
        encode_block(&bytes, &mut sent);
        for w in 0..BLOCK_WORDS {
            sent[sent_index(w * 12 + w % 12)] ^= 1;
        }
        let mut back = [0u8; 20];
        assert_eq!(decode_block(&sent, &mut back), (20, 0));
        assert_eq!(back, bytes);
    }
}
