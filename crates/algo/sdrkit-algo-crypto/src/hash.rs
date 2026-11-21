//! SHA-256 and SHA-1 hashes. SHA-1 is only for protocols that name it. Do not use SHA-1 where collision resistance matters.
//!
//! Standard: FIPS 180-4 sections 6.1 (SHA-1) and 6.2 (SHA-256).

use ::sha2::Digest;

/// SHA-256 of `data`.
///
/// ```
/// // FIPS 180-4 example "abc" (NIST CSRC example values, SHA256.pdf).
/// let d = sdrkit_algo_crypto::sha256(b"abc");
/// assert_eq!(d[..4], [0xba, 0x78, 0x16, 0xbf]);
/// ```
pub fn sha256(data: &[u8]) -> [u8; 32] {
    ::sha2::Sha256::digest(data).into()
}

/// SHA-256 fed in pieces.
///
/// ```
/// use sdrkit_algo_crypto::Sha256;
/// let mut h = Sha256::new();
/// h.update(b"a");
/// h.update(b"bc");
/// assert_eq!(h.finalize_reset(), sdrkit_algo_crypto::sha256(b"abc"));
/// ```
#[derive(Clone, Default)]
pub struct Sha256(::sha2::Sha256);

impl Sha256 {
    /// An empty hash.
    pub fn new() -> Self {
        Sha256(::sha2::Sha256::new())
    }

    /// Adds `data`.
    pub fn update(&mut self, data: &[u8]) {
        self.0.update(data);
    }

    /// The digest of everything added; the hash is then empty again.
    pub fn finalize_reset(&mut self) -> [u8; 32] {
        self.0.finalize_reset().into()
    }
}

impl std::fmt::Debug for Sha256 {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Sha256(..)")
    }
}

/// SHA-1 of `data`.
///
/// Use it only where a protocol names SHA-1, for example the `Sec-WebSocket-Accept` value of RFC 6455 section 4.2.2.
///
/// ```
/// // FIPS 180-4 example "abc" (NIST CSRC example values, SHA1.pdf).
/// let d = sdrkit_algo_crypto::sha1(b"abc");
/// assert_eq!(d[..4], [0xa9, 0x99, 0x3e, 0x36]);
/// ```
pub fn sha1(data: &[u8]) -> [u8; 20] {
    ::sha1::Sha1::digest(data).into()
}

/// SHA-1 fed in pieces.
///
/// ```
/// use sdrkit_algo_crypto::Sha1;
/// let mut h = Sha1::new();
/// h.update(b"a");
/// h.update(b"bc");
/// assert_eq!(h.finalize_reset(), sdrkit_algo_crypto::sha1(b"abc"));
/// ```
#[derive(Clone, Default)]
pub struct Sha1(::sha1::Sha1);

impl Sha1 {
    /// An empty hash.
    pub fn new() -> Self {
        Sha1(::sha1::Sha1::new())
    }

    /// Adds `data`.
    pub fn update(&mut self, data: &[u8]) {
        self.0.update(data);
    }

    /// The digest of everything added. The hash is then empty again.
    pub fn finalize_reset(&mut self) -> [u8; 20] {
        self.0.finalize_reset().into()
    }
}

impl std::fmt::Debug for Sha1 {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Sha1(..)")
    }
}
