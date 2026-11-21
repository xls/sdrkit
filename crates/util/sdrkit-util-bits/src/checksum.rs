//! Checksums and parity: sums, Fletcher, Adler-32, the Internet checksum, odd and even parity, and the small checks of single
//! protocols that are not CRCs.
//!
//! ```
//! use sdrkit_util_bits::checksum::{adler32, fletcher16, internet_checksum, parity};
//! assert_eq!(adler32(b"Wikipedia"), 0x11E6_0398);
//! assert_eq!(fletcher16(b"abcde"), 0xC8F0);
//! assert_eq!(internet_checksum(&[&[0x00, 0x01, 0xF2, 0x03, 0xF4, 0xF5, 0xF6, 0xF7]]), 0x220D);
//! assert_eq!(parity(0b1011), 1);
//! ```

/// The sum of the bytes modulo 256.
#[inline]
pub fn sum8(b: &[u8]) -> u8 {
    b.iter().fold(0u8, |a, &x| a.wrapping_add(x))
}

/// The XOR of the bytes. The NMEA 0183 sentence checksum is this over the characters between `$` (or `!`) and `*`; whole-sentence
/// handling is in `sdrkit-util-geo`.
#[inline]
pub fn xor8(b: &[u8]) -> u8 {
    b.iter().fold(0u8, |a, &x| a ^ x)
}

/// The sum of the bytes modulo 65536 (the MeshCom trailer, sent big-endian).
#[inline]
pub fn sum16(b: &[u8]) -> u16 {
    b.iter().fold(0u16, |a, &x| a.wrapping_add(x as u16))
}

/// The sum of the first `nibbles` nibbles of `b` (high nibble of each byte first), modulo 256 (Oregon Scientific sensors). Nibbles
/// past the end of `b` count as zero.
pub fn nibble_sum(b: &[u8], nibbles: usize) -> u8 {
    (0..nibbles).fold(0u8, |a, i| {
        let byte = b.get(i / 2).copied().unwrap_or(0);
        let n = if i % 2 == 0 { byte >> 4 } else { byte & 0xF };
        a.wrapping_add(n)
    })
}

/// Fletcher's sums with arithmetic modulo 256: returns `(a, b)` where `a` is the sum of the bytes and `b` the sum of the running
/// values of `a`. The Orbcomm packet check is good when both are zero.
pub fn fletcher_mod256(data: &[u8]) -> (u8, u8) {
    let (mut a, mut b) = (0u8, 0u8);
    for &x in data {
        a = a.wrapping_add(x);
        b = b.wrapping_add(a);
    }
    (a, b)
}

/// Fletcher-16 (sums modulo 255): `(b << 8) | a`. J. G. Fletcher, IEEE Trans. Commun. COM-30 (1982). The value for "abcde" is
/// 0xC8F0.
pub fn fletcher16(data: &[u8]) -> u16 {
    let (mut a, mut b) = (0u32, 0u32);
    // 5802 bytes keep both sums below 2^32 before the reduction.
    for chunk in data.chunks(5802) {
        for &x in chunk {
            a += x as u32;
            b += a;
        }
        a %= 255;
        b %= 255;
    }
    ((b << 8) | a) as u16
}

/// The two check octets of ISO 8473-1 annex C for a header of `data.len()` octets whose check octets are at index `at` and
/// `at + 1` (their contents are ignored and taken as zero). Returns them in that order; afterwards both Fletcher sums modulo 255 of
/// the whole header are zero. `None` when `at + 1` is outside `data`.
pub fn iso8473_check(data: &[u8], at: usize) -> Option<[u8; 2]> {
    if at.checked_add(1)? >= data.len() {
        return None;
    }
    let (mut c0, mut c1) = (0u32, 0u32);
    for (i, &x) in data.iter().enumerate() {
        let v = if i == at || i == at + 1 { 0 } else { x as u32 };
        c0 = (c0 + v) % 255;
        c1 = (c1 + c0) % 255;
    }
    // Position of the first check octet counted from 1 (n) in a header of length L.
    let l = data.len() as u32;
    let n = at as u32 + 1;
    let x = ((l - n) * c0 + 255 * 255 - c1) % 255;
    let y = (c1 + 255 * 255 - (l - n + 1) * c0) % 255;
    // ISO 8473: a computed 0 is sent as 255.
    let fix = |v: u32| if v == 0 { 255 } else { v as u8 };
    Some([fix(x), fix(y)])
}

/// The Inmarsat-C packet check: Fletcher-style sums modulo 65536 over the packet with its last two octets (the check) taken as
/// zero, giving check octets `(s0 - s1, s1 - 2 s0)` modulo 256 (Inmarsat-C SDM, packet check). Returns those two octets.
pub fn inmarsat_c_check(packet: &[u8]) -> [u8; 2] {
    let n = packet.len();
    let (mut s0, mut s1) = (0u32, 0u32);
    for (i, &b) in packet.iter().enumerate() {
        let v = if i + 2 >= n { 0 } else { b as u32 };
        s0 = (s0 + v) & 0xFFFF;
        s1 = (s1 + s0) & 0xFFFF;
    }
    [
        (s0.wrapping_sub(s1) & 0xFF) as u8,
        (s1.wrapping_sub(2 * s0) & 0xFF) as u8,
    ]
}

/// Adler-32 (RFC 1950 clause 8.2), streaming. The modulo is deferred over runs of 5552 bytes, the longest that cannot overflow.
///
/// ```
/// use sdrkit_util_bits::checksum::Adler32;
/// let mut a = Adler32::new();
/// a.update(b"Wiki");
/// a.update(b"pedia");
/// assert_eq!(a.finish(), 0x11E6_0398);
/// ```
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Adler32 {
    a: u32,
    b: u32,
}

impl Default for Adler32 {
    fn default() -> Self {
        Self::new()
    }
}

impl Adler32 {
    const MOD: u32 = 65521;

    /// A new checksum (value 1).
    pub fn new() -> Self {
        Adler32 { a: 1, b: 0 }
    }

    /// Adds bytes.
    pub fn update(&mut self, data: &[u8]) {
        for chunk in data.chunks(5552) {
            for &x in chunk {
                self.a += x as u32;
                self.b += self.a;
            }
            self.a %= Self::MOD;
            self.b %= Self::MOD;
        }
    }

    /// The checksum so far.
    pub fn finish(&self) -> u32 {
        (self.b << 16) | self.a
    }
}

/// Adler-32 of `data` in one call.
pub fn adler32(data: &[u8]) -> u32 {
    let mut a = Adler32::new();
    a.update(data);
    a.finish()
}

/// The Internet checksum (RFC 1071): the one's complement of the one's complement sum of 16-bit big-endian words. Several parts are
/// summed as if they were one buffer (each part may have an odd length), so a UDP or TCP pseudo-header is passed as the first
/// part. A buffer that holds its own correct checksum returns 0.
pub fn internet_checksum(parts: &[&[u8]]) -> u16 {
    let mut sum: u64 = 0;
    let mut odd: Option<u8> = None;
    for p in parts {
        let mut s = *p;
        if let Some(hi) = odd.take() {
            if let Some((&lo, rest)) = s.split_first() {
                sum += u16::from_be_bytes([hi, lo]) as u64;
                s = rest;
            } else {
                odd = Some(hi);
                continue;
            }
        }
        let (words, rest) = s.as_chunks::<2>();
        if let [last] = rest {
            odd = Some(*last);
        }
        for w in words {
            sum += u16::from_be_bytes([w[0], w[1]]) as u64;
        }
    }
    if let Some(hi) = odd {
        sum += (hi as u64) << 8;
    }
    while sum >> 16 != 0 {
        sum = (sum & 0xFFFF) + (sum >> 16);
    }
    !(sum as u16)
}

/// The parity of `x`: 1 when it has an odd number of one bits. Also the output of a convolutional code tap set (`parity(reg & g)`).
#[inline]
pub const fn parity(x: u64) -> u8 {
    (x.count_ones() & 1) as u8
}

/// The even parity bit of `x` (the bit that makes the total number of ones even): the same as [`parity`].
#[inline]
pub const fn even_parity_bit(x: u64) -> u8 {
    parity(x)
}

/// The odd parity bit of `x` (the bit that makes the total number of ones odd).
#[inline]
pub const fn odd_parity_bit(x: u64) -> u8 {
    parity(x) ^ 1
}

/// A 7-bit character with odd parity in bit 7 (ACARS ARINC 618, GOES, teletext).
#[inline]
pub const fn with_odd_parity7(c: u8) -> u8 {
    let c = c & 0x7F;
    c | (odd_parity_bit(c as u64) << 7)
}

/// The 7-bit character of a byte with odd parity in bit 7, or `None` when the parity is wrong.
#[inline]
pub const fn odd_parity7(b: u8) -> Option<u8> {
    if parity(b as u64) == 1 {
        Some(b & 0x7F)
    } else {
        None
    }
}

/// Whether every byte of `b` has even parity (a parity bit in each byte, as Acurite sensors send). Checking each byte rather than
/// the whole slice catches two errors in different bytes.
pub fn each_byte_even_parity(b: &[u8]) -> bool {
    b.iter().all(|&x| parity(x as u64) == 0)
}

/// The 5-bit checksum of a DMR full link control message (ETSI TS 102 361-1 clause B.3.11): the sum of the 9 octets modulo 31.
pub fn dmr_lc_checksum(lc: &[u8; 9]) -> u8 {
    (lc.iter().map(|&x| x as u32).sum::<u32>() % 31) as u8
}

/// Whether the check nibble of a FLEX frame information word, block information word or vector word (21 bits, bit 0 the first
/// sent) is right: the five nibbles plus bit 20 sum to 0xF modulo 16.
pub fn flex_check_ok(word21: u32) -> bool {
    let s: u32 = (0..5).map(|k| (word21 >> (4 * k)) & 0xF).sum::<u32>() + ((word21 >> 20) & 1);
    s & 0xF == 0xF
}

/// The FLEX word with bits 0 to 3 set so that [`flex_check_ok`] holds.
pub fn flex_with_check(word21: u32) -> u32 {
    let v = word21 & 0x1F_FFF0;
    let s: u32 = (1..5).map(|k| (v >> (4 * k)) & 0xF).sum::<u32>() + ((v >> 20) & 1);
    v | ((0xF + 16 * 4 - s) & 0xF)
}

/// The 16-bit linear checksum of the Meteomodem M10 and M20 radiosonde frames.
pub fn m10_checksum(data: &[u8]) -> u16 {
    let mut c: u16 = 0;
    for &b in data {
        let b1 = b.rotate_right(1);
        let b2 = b1 ^ (b1 >> 2);
        let lo = (c & 0xFF) as u8;
        let bit = |k: u32| ((c >> k) & 1) as u8;
        let t6 = bit(0) ^ bit(2) ^ bit(4);
        let t7 = bit(1) ^ bit(3) ^ bit(5);
        let t = (lo & 0x3F) | (t6 << 6) | (t7 << 7);
        let s = ((c >> 7) & 0xFF) as u8;
        let s2 = s ^ (s >> 2);
        c = ((lo as u16) << 8) | (b2 ^ t ^ s2) as u16;
    }
    c
}

/// The 5-bit LoRa explicit-header checksum of the header nibbles `n0`, `n1`, `n2` (a fixed parity matrix, Semtech LoRa modem
/// header).
pub fn lora_header_checksum(n0: u8, n1: u8, n2: u8) -> u8 {
    let b = |n: u8, k: u8| (n >> k) & 1;
    let c4 = b(n0, 3) ^ b(n0, 2) ^ b(n0, 1) ^ b(n0, 0);
    let c3 = b(n0, 3) ^ b(n1, 3) ^ b(n1, 2) ^ b(n1, 1) ^ b(n2, 0);
    let c2 = b(n0, 2) ^ b(n1, 3) ^ b(n1, 0) ^ b(n2, 3) ^ b(n2, 1);
    let c1 = b(n0, 1) ^ b(n1, 2) ^ b(n1, 0) ^ b(n2, 2) ^ b(n2, 1) ^ b(n2, 0);
    let c0 = b(n0, 0) ^ b(n1, 1) ^ b(n2, 3) ^ b(n2, 2) ^ b(n2, 1) ^ b(n2, 0);
    (c4 << 4) | (c3 << 3) | (c2 << 2) | (c1 << 1) | c0
}

/// The 8-bit LFSR digest of `data`, as several ISM sensors (Acurite, Ambient Weather F007TH) send it. Bytes are taken from the
/// first and bits from the most significant. Each 1 bit XORs the key into the sum. After each bit the key shifts right one
/// place and takes `gen` when the bit shifted out was 1.
///
/// ```
/// use sdrkit_util_bits::checksum::lfsr_digest8;
/// assert_eq!(lfsr_digest8(b"123456789", 0x98, 0x3E), 0xB5);
/// ```
pub fn lfsr_digest8(data: &[u8], gen: u8, key: u8) -> u8 {
    lfsr_digest16(data, gen as u16, key as u16) as u8
}

/// The reflected 8-bit LFSR digest of `data` (LaCrosse TX141TH, ThermoPro). Bytes are taken from the last and bits from the
/// least significant. Each 1 bit XORs the key into the sum. After each bit the key shifts left one place and takes `gen` when
/// its top bit was 1.
///
/// ```
/// use sdrkit_util_bits::checksum::lfsr_digest8_reflect;
/// assert_eq!(lfsr_digest8_reflect(b"123456789", 0x31, 0xF4), 0x9E);
/// ```
pub fn lfsr_digest8_reflect(data: &[u8], gen: u8, mut key: u8) -> u8 {
    let mut sum = 0u8;
    for &b in data.iter().rev() {
        for i in 0..8 {
            if (b >> i) & 1 != 0 {
                sum ^= key;
            }
            key = (key << 1) ^ if key & 0x80 != 0 { gen } else { 0 };
        }
    }
    sum
}

/// The 16-bit LFSR digest of `data` (Bresser 6-in-1 and 7-in-1): the rule of [`lfsr_digest8`] with a 16-bit key and
/// generator.
///
/// ```
/// use sdrkit_util_bits::checksum::lfsr_digest16;
/// assert_eq!(lfsr_digest16(b"123456789", 0x8810, 0x5412), 0xB746);
/// ```
pub fn lfsr_digest16(data: &[u8], gen: u16, mut key: u16) -> u16 {
    let mut sum = 0u16;
    for &b in data {
        for i in (0..8).rev() {
            if (b >> i) & 1 != 0 {
                sum ^= key;
            }
            key = (key >> 1) ^ if key & 1 != 0 { gen } else { 0 };
        }
    }
    sum
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn published_values() {
        // Fletcher-16 examples (Fletcher 1982, as commonly tabulated).
        assert_eq!(fletcher16(b"abcde"), 0xC8F0);
        assert_eq!(fletcher16(b"abcdef"), 0x2057);
        assert_eq!(fletcher16(b"abcdefgh"), 0x0627);
        // RFC 1950 Adler-32 of "Wikipedia".
        assert_eq!(adler32(b"Wikipedia"), 0x11E6_0398);
        assert_eq!(adler32(b""), 1);
        // RFC 1071 section 3 example: sum DDF2, checksum 220D.
        let d = [0x00u8, 0x01, 0xF2, 0x03, 0xF4, 0xF5, 0xF6, 0xF7];
        assert_eq!(internet_checksum(&[&d]), 0x220D);
        // NMEA 0183: the widely published RMC example ends in *6A.
        let body = b"GPRMC,123519,A,4807.038,N,01131.000,E,022.4,084.4,230394,003.1,W";
        assert_eq!(xor8(body), 0x6A);
        assert_eq!(sum8(b"123456789"), 0xDD);
        assert_eq!(xor8(b"123456789"), 0x31);
    }

    #[test]
    fn internet_checksum_parts_and_odd_lengths() {
        let d = [0x00u8, 0x01, 0xF2, 0x03, 0xF4, 0xF5, 0xF6, 0xF7, 0x12];
        let whole = internet_checksum(&[&d]);
        assert_eq!(internet_checksum(&[&d[..3], &[], &d[3..8], &d[8..]]), whole);
        let mut with = d.to_vec();
        with.push(0); // pad to whole words, then append the checksum
        let c = internet_checksum(&[&with]);
        with.extend_from_slice(&c.to_be_bytes());
        assert_eq!(internet_checksum(&[&with]), 0);
    }

    #[test]
    fn adler_long_input_and_pieces() {
        let data: Vec<u8> = (0..20_000u32).map(|i| (i * 31) as u8).collect();
        let mut a = Adler32::new();
        for c in data.chunks(777) {
            a.update(c);
        }
        // Direct definition with a modulo per byte.
        let (mut s1, mut s2) = (1u32, 0u32);
        for &x in &data {
            s1 = (s1 + x as u32) % 65521;
            s2 = (s2 + s1) % 65521;
        }
        assert_eq!(a.finish(), (s2 << 16) | s1);
        let f = data.iter().fold((0u32, 0u32), |(a, b), &x| {
            let a = (a + x as u32) % 255;
            (a, (b + a) % 255)
        });
        assert_eq!(fletcher16(&data), ((f.1 << 8) | f.0) as u16);
    }

    #[test]
    fn iso8473_check_makes_both_sums_zero() {
        for at in [0usize, 3, 7] {
            let mut h: Vec<u8> = (1..=9u8).map(|x| x.wrapping_mul(37)).collect();
            let c = iso8473_check(&h, at).unwrap();
            h[at] = c[0];
            h[at + 1] = c[1];
            let (mut c0, mut c1) = (0u32, 0u32);
            for &x in &h {
                c0 = (c0 + x as u32) % 255;
                c1 = (c1 + c0) % 255;
            }
            assert_eq!((c0, c1), (0, 0), "at {at}");
        }
        assert!(iso8473_check(&[1, 2], 1).is_none());
    }

    #[test]
    fn inmarsat_c_check_makes_fletcher_sums_zero() {
        let mut p: Vec<u8> = (0..30u8).map(|x| x.wrapping_mul(91)).collect();
        let c = inmarsat_c_check(&p);
        let n = p.len();
        p[n - 2] = c[0];
        p[n - 1] = c[1];
        assert_eq!(fletcher_mod256(&p), (0, 0));
    }

    #[test]
    fn parity_helpers() {
        assert_eq!(with_odd_parity7(0x03), 0x83); // ACARS ETX as sent
        assert_eq!(with_odd_parity7(0x01), 0x01);
        assert_eq!(with_odd_parity7(0x00), 0x80);
        assert_eq!(odd_parity7(0x03), None);
        assert_eq!(odd_parity7(0x97), Some(0x17)); // ACARS ETB with its parity
        assert_eq!(odd_parity7(with_odd_parity7(0x03)), Some(0x03));
        assert!(each_byte_even_parity(&[0x03, 0x81]));
        assert!(!each_byte_even_parity(&[0x01, 0x01]));
        assert_eq!(nibble_sum(&[0x12, 0x34], 3), 6);
        assert_eq!(sum16(&[0xFF; 300]), (300u32 * 255 % 65536) as u16);
    }

    #[test]
    fn small_protocol_checks() {
        assert_eq!(dmr_lc_checksum(&[255; 9]), (9 * 255 % 31) as u8);
        for v in [0u32, 0x1_2340, 0x1F_FFF0, 0x0A_BCD0] {
            assert!(flex_check_ok(flex_with_check(v)));
            assert!(!flex_check_ok(flex_with_check(v) ^ 0x10));
        }
        // LoRa header checksums as published decoders compute them.
        assert_eq!(lora_header_checksum(8, 0, 0), 0x18);
        assert_eq!(lora_header_checksum(0, 0, 1), 0x0B);
        assert_eq!(lora_header_checksum(0, 1, 0), 0x06);
        assert_eq!(m10_checksum(&[]), 0);
        assert_ne!(m10_checksum(&[1]), m10_checksum(&[2]));
    }

    /// The XOR of the keys at the 1 bits of `data` (bytes from the first, bits from the most significant): a second form of
    /// the digest rule, independent of the shift-and-sum loop.
    fn digest_by_keys(data: &[u8], keys: impl Iterator<Item = u16>) -> u16 {
        let bits = data
            .iter()
            .flat_map(|&b| (0..8).rev().map(move |i| (b >> i) & 1));
        bits.zip(keys)
            .filter(|(b, _)| *b == 1)
            .fold(0, |s, (_, k)| s ^ k)
    }

    fn keys_right(gen: u16, key: u16) -> impl Iterator<Item = u16> {
        std::iter::successors(Some(key), move |&k| {
            Some((k >> 1) ^ if k & 1 != 0 { gen } else { 0 })
        })
    }

    #[test]
    fn lfsr_digests_match_their_check_values() {
        // No standard publishes these digests. The six (generator, key, check) triples are the ones the ISM sensors use,
        // each checked over "123456789" against the key-sequence form of the rule and for linearity.
        let s = b"123456789";
        for (gen, key, check) in [(0x98u8, 0x3Eu8, 0xB5u8), (0x98, 0xF1, 0x54)] {
            assert_eq!(lfsr_digest8(s, gen, key), check);
            assert_eq!(
                digest_by_keys(s, keys_right(gen as u16, key as u16)) as u8,
                check
            );
        }
        for (gen, key, check) in [(0x31u8, 0xF4u8, 0x9Eu8), (0x51, 0x04, 0xA2)] {
            assert_eq!(lfsr_digest8_reflect(s, gen, key), check);
            // Bytes from the last and bits from the least significant: the plain rule over the reversed message, with the
            // key moving left.
            let rev: Vec<u8> = s.iter().rev().map(|b| b.reverse_bits()).collect();
            let keys = std::iter::successors(Some(key), move |&k| {
                Some((k << 1) ^ if k & 0x80 != 0 { gen } else { 0 })
            });
            assert_eq!(digest_by_keys(&rev, keys.map(u16::from)) as u8, check);
        }
        for (gen, key, check) in [(0x8810u16, 0x5412u16, 0xB746u16), (0x8810, 0xBA95, 0x0CC1)] {
            assert_eq!(lfsr_digest16(s, gen, key), check);
            assert_eq!(digest_by_keys(s, keys_right(gen, key)), check);
            let (a, b) = (b"12345", b"abcde");
            let x: Vec<u8> = a.iter().zip(b).map(|(p, q)| p ^ q).collect();
            assert_eq!(
                lfsr_digest16(&x, gen, key),
                lfsr_digest16(a, gen, key) ^ lfsr_digest16(b, gen, key)
            );
        }
        assert_eq!(lfsr_digest8(&[], 0x98, 0x3E), 0);
        assert_eq!(lfsr_digest8_reflect(&[], 0x31, 0xF4), 0);
    }
}
