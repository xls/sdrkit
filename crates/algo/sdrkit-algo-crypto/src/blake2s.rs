//! BLAKE2s (RFC 7693): digests of 1 to 32 bytes, with an optional key of up to 32 bytes, in one call or fed in pieces.
//!
//! Standard: RFC 7693 sections 2 and 3 (parameters, mixing function G, compression function F, padding and the parameter
//! block of a sequential hash). Written from the text of the RFC; the tests hold its Appendix B and Appendix E values.

/// The initialisation vector: the first 32 bits of the fractional parts of the square roots of the first eight primes.
const IV: [u32; 8] = [
    0x6A09_E667,
    0xBB67_AE85,
    0x3C6E_F372,
    0xA54F_F53A,
    0x510E_527F,
    0x9B05_688C,
    0x1F83_D9AB,
    0x5BE0_CD19,
];

/// The message word permutations of the ten rounds (RFC 7693 section 2.7).
const SIGMA: [[usize; 16]; 10] = [
    [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15],
    [14, 10, 4, 8, 9, 15, 13, 6, 1, 12, 0, 2, 11, 7, 5, 3],
    [11, 8, 12, 0, 5, 2, 15, 13, 10, 14, 3, 6, 7, 1, 9, 4],
    [7, 9, 3, 1, 13, 12, 11, 14, 2, 6, 5, 10, 4, 0, 15, 8],
    [9, 0, 5, 7, 2, 4, 10, 15, 14, 1, 11, 12, 6, 8, 3, 13],
    [2, 12, 6, 10, 0, 11, 8, 3, 4, 13, 7, 5, 15, 14, 1, 9],
    [12, 5, 1, 15, 14, 13, 4, 10, 0, 7, 6, 3, 9, 2, 8, 11],
    [13, 11, 7, 14, 12, 1, 3, 9, 5, 0, 15, 4, 8, 6, 2, 10],
    [6, 15, 14, 9, 11, 3, 0, 8, 12, 2, 13, 7, 1, 4, 10, 5],
    [10, 2, 8, 4, 7, 6, 1, 5, 15, 11, 9, 14, 3, 12, 13, 0],
];

/// Bytes of one message block.
const BLOCK: usize = 64;

/// Why a [`Blake2s`] could not be made.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Blake2sError {
    /// The digest length is not 1 to 32 bytes.
    DigestLength(usize),
    /// The key is longer than 32 bytes.
    KeyLength(usize),
}

impl std::fmt::Display for Blake2sError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Blake2sError::DigestLength(n) => write!(f, "a BLAKE2s digest of {n} bytes is not 1 to 32 bytes"),
            Blake2sError::KeyLength(n) => write!(f, "a BLAKE2s key of {n} bytes is longer than 32 bytes"),
        }
    }
}

impl std::error::Error for Blake2sError {}

/// BLAKE2s fed in pieces.
///
/// ```
/// use sdrkit_algo_crypto::blake2s::{blake2s_256, Blake2s};
/// let mut h = Blake2s::new_256();
/// h.update(b"a");
/// h.update(b"bc");
/// assert_eq!(h.finalize(), blake2s_256(b"abc").to_vec());
/// ```
#[derive(Clone)]
pub struct Blake2s {
    h: [u32; 8],
    /// Bytes compressed so far.
    t: u64,
    buf: [u8; BLOCK],
    /// Bytes waiting in `buf`; a full buffer waits until more data shows it is not the last block.
    fill: usize,
    out_len: usize,
}

impl std::fmt::Debug for Blake2s {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Blake2s(..)")
    }
}

impl Blake2s {
    /// A hash with a digest of `out_len` bytes (1 to 32) and `key` (0 to 32 bytes; empty for no key).
    ///
    /// # Errors
    ///
    /// [`Blake2sError`] for a digest length or key length out of range.
    pub fn new(out_len: usize, key: &[u8]) -> Result<Blake2s, Blake2sError> {
        if !(1..=32).contains(&out_len) {
            return Err(Blake2sError::DigestLength(out_len));
        }
        if key.len() > 32 {
            return Err(Blake2sError::KeyLength(key.len()));
        }
        let mut h = IV;
        // Parameter block word 0: digest length, key length, fanout 1, depth 1.
        h[0] ^= 0x0101_0000 ^ ((key.len() as u32) << 8) ^ out_len as u32;
        let mut s = Blake2s {
            h,
            t: 0,
            buf: [0; BLOCK],
            fill: 0,
            out_len,
        };
        if !key.is_empty() {
            // The key, padded with zeros to a full block, is the first block.
            s.buf[..key.len()].copy_from_slice(key);
            s.fill = BLOCK;
        }
        Ok(s)
    }

    /// An unkeyed hash with a 32-byte digest (BLAKE2s-256).
    pub fn new_256() -> Blake2s {
        Blake2s::new(32, &[]).expect("a 32-byte digest without a key")
    }

    /// Adds `data`.
    pub fn update(&mut self, mut data: &[u8]) {
        while !data.is_empty() {
            if self.fill == BLOCK {
                self.t = self.t.wrapping_add(BLOCK as u64);
                let b = self.buf;
                self.compress(&b, false);
                self.fill = 0;
            }
            let n = (BLOCK - self.fill).min(data.len());
            self.buf[self.fill..self.fill + n].copy_from_slice(&data[..n]);
            self.fill += n;
            data = &data[n..];
        }
    }

    /// The digest of everything added.
    pub fn finalize(mut self) -> Vec<u8> {
        self.t = self.t.wrapping_add(self.fill as u64);
        let mut b = self.buf;
        b[self.fill..].fill(0);
        self.compress(&b, true);
        let mut out = Vec::with_capacity(32);
        for w in self.h {
            out.extend_from_slice(&w.to_le_bytes());
        }
        out.truncate(self.out_len);
        out
    }

    /// The compression function F of RFC 7693 section 3.2 on one block, with the byte count `self.t`.
    fn compress(&mut self, block: &[u8; BLOCK], last: bool) {
        let mut m = [0u32; 16];
        for (i, w) in m.iter_mut().enumerate() {
            *w = u32::from_le_bytes([block[4 * i], block[4 * i + 1], block[4 * i + 2], block[4 * i + 3]]);
        }
        let mut v = [0u32; 16];
        v[..8].copy_from_slice(&self.h);
        v[8..].copy_from_slice(&IV);
        v[12] ^= self.t as u32;
        v[13] ^= (self.t >> 32) as u32;
        if last {
            v[14] = !v[14];
        }
        for s in &SIGMA {
            g(&mut v, 0, 4, 8, 12, m[s[0]], m[s[1]]);
            g(&mut v, 1, 5, 9, 13, m[s[2]], m[s[3]]);
            g(&mut v, 2, 6, 10, 14, m[s[4]], m[s[5]]);
            g(&mut v, 3, 7, 11, 15, m[s[6]], m[s[7]]);
            g(&mut v, 0, 5, 10, 15, m[s[8]], m[s[9]]);
            g(&mut v, 1, 6, 11, 12, m[s[10]], m[s[11]]);
            g(&mut v, 2, 7, 8, 13, m[s[12]], m[s[13]]);
            g(&mut v, 3, 4, 9, 14, m[s[14]], m[s[15]]);
        }
        for i in 0..8 {
            self.h[i] ^= v[i] ^ v[i + 8];
        }
    }
}

/// The mixing function G of RFC 7693 section 3.1, with the BLAKE2s rotations 16, 12, 8 and 7.
#[inline(always)]
fn g(v: &mut [u32; 16], a: usize, b: usize, c: usize, d: usize, x: u32, y: u32) {
    v[a] = v[a].wrapping_add(v[b]).wrapping_add(x);
    v[d] = (v[d] ^ v[a]).rotate_right(16);
    v[c] = v[c].wrapping_add(v[d]);
    v[b] = (v[b] ^ v[c]).rotate_right(12);
    v[a] = v[a].wrapping_add(v[b]).wrapping_add(y);
    v[d] = (v[d] ^ v[a]).rotate_right(8);
    v[c] = v[c].wrapping_add(v[d]);
    v[b] = (v[b] ^ v[c]).rotate_right(7);
}

/// BLAKE2s-256 of `data`, without a key.
///
/// ```
/// // RFC 7693 Appendix B.
/// let d = sdrkit_algo_crypto::blake2s::blake2s_256(b"abc");
/// assert_eq!(d[..4], [0x50, 0x8c, 0x5e, 0x8c]);
/// ```
pub fn blake2s_256(data: &[u8]) -> [u8; 32] {
    let mut h = Blake2s::new_256();
    h.update(data);
    let mut out = [0u8; 32];
    out.copy_from_slice(&h.finalize());
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hex(b: &[u8]) -> String {
        b.iter().map(|v| format!("{v:02x}")).collect()
    }

    #[test]
    fn rfc_7693_appendix_b_and_short_inputs() {
        assert_eq!(hex(&blake2s_256(b"abc")), "508c5e8c327c14e2e1a72ba34eeb452f37458b209ed63a294d999b4c86675982");
        assert_eq!(hex(&blake2s_256(b"")), "69217a3079908094e11121d042354a7c1f55b6482ca1a51e1b250dfd1ed0eef9");
        assert_eq!(hex(&blake2s_256(b"hello")), "19213bacc58dee6dbde3ceb9a47cbb330b3d86f8cca8997eb00be456f140ca25");
    }

    #[test]
    fn block_edges_and_pieces() {
        // Bytes i mod 251; the last block rule at 64 and 128 bytes.
        let cases = [
            (63, "e57cb79487dd57902432b250733813bd96a84efce59f650fac26e6696aefafc3"),
            (64, "56f34e8b96557e90c1f24b52d0c89d51086acf1b00f634cf1dde9233b8eaaa3e"),
            (65, "1b53ee94aaf34e4b159d48de352c7f0661d0a40edff95a0b1639b4090e974472"),
            (128, "1fa877de67259d19863a2a34bcc6962a2b25fcbf5cbecd7ede8f1fa36688a796"),
            (129, "5bd169e67c82c2c2e98ef7008bdf261f2ddf30b1c00f9e7f275bb3e8a28dc9a2"),
            (1000, "1c067a5e746fb0f6734efac9a8cdb0e11061f0077f255184365c690115392501"),
        ];
        for (n, want) in cases {
            let data: Vec<u8> = (0..n).map(|i| (i % 251) as u8).collect();
            assert_eq!(hex(&blake2s_256(&data)), want, "{n}");
            for piece in [1, 7, 63, 64, 65] {
                let mut h = Blake2s::new_256();
                for c in data.chunks(piece) {
                    h.update(c);
                }
                assert_eq!(hex(&h.finalize()), want, "{n} in pieces of {piece}");
            }
        }
    }

    #[test]
    fn keyed_hash() {
        let mut h = Blake2s::new(32, &(0..32).collect::<Vec<u8>>()).unwrap();
        h.update(&(0..64).collect::<Vec<u8>>());
        assert_eq!(hex(&h.finalize()), "8975b0577fd35566d750b362b0897a26c399136df07bababbde6203ff2954ed4");
        assert_eq!(Blake2s::new(0, &[]).err(), Some(Blake2sError::DigestLength(0)));
        assert_eq!(Blake2s::new(33, &[]).err(), Some(Blake2sError::DigestLength(33)));
        assert_eq!(Blake2s::new(32, &[0; 33]).err(), Some(Blake2sError::KeyLength(33)));
    }

    /// The self-test of RFC 7693 Appendix E: digests of 16, 20, 28 and 32 bytes of inputs of 0 to 1024 bytes, with and
    /// without a key, hashed together.
    #[test]
    fn rfc_7693_appendix_e_self_test() {
        fn sequence(n: usize, seed: u32) -> Vec<u8> {
            let mut a = 0xDEAD_4BADu32.wrapping_mul(seed);
            let mut b = 1u32;
            (0..n)
                .map(|_| {
                    let t = a.wrapping_add(b);
                    a = b;
                    b = t;
                    (t >> 24) as u8
                })
                .collect()
        }
        let mut all = Blake2s::new_256();
        for out_len in [16usize, 20, 28, 32] {
            for in_len in [0usize, 3, 64, 65, 255, 1024] {
                let input = sequence(in_len, in_len as u32);
                let mut h = Blake2s::new(out_len, &[]).unwrap();
                h.update(&input);
                all.update(&h.finalize());
                let mut h = Blake2s::new(out_len, &sequence(out_len, out_len as u32)).unwrap();
                h.update(&input);
                all.update(&h.finalize());
            }
        }
        assert_eq!(hex(&all.finalize()), "6a411f08ce25adcdfb02aba641451cec53c598b24f4fc787fbdc88797f4c1dfe");
    }
}
