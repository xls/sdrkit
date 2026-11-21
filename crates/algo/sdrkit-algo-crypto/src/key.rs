//! Secret key bytes with a fixed size.
//!
//! A [`Key<N>`](Key) holds `N` secret bytes. It is wiped (overwritten with zeros) when dropped and its `Debug` output shows the
//! size only. [`AesKey`] holds one of the three AES key sizes, for protocols that accept more than one (a Meshtastic channel key
//! is 16 or 32 bytes).

use crate::Error;
use std::fmt;
use zeroize::Zeroize;

/// `N` secret bytes, wiped on drop.
///
/// ```
/// use sdrkit_algo_crypto::Key;
/// let k = Key::<16>::from_slice(&[7u8; 16])?;
/// assert_eq!(k.as_bytes()[0], 7);
/// assert!(Key::<16>::from_slice(&[7u8; 15]).is_err());
/// assert_eq!(format!("{k:?}"), "Key<16>(..)");
/// # Ok::<(), sdrkit_algo_crypto::Error>(())
/// ```
#[derive(Clone)]
pub struct Key<const N: usize>([u8; N]);

impl<const N: usize> Key<N> {
    /// Takes the bytes of a key. (The caller's own copy of `bytes` is not wiped by this crate.)
    pub const fn new(bytes: [u8; N]) -> Self {
        Key(bytes)
    }

    /// Copies a key from a slice; [`Error::KeyLength`] unless it is exactly `N` bytes.
    pub fn from_slice(bytes: &[u8]) -> Result<Self, Error> {
        let a: [u8; N] = bytes
            .try_into()
            .map_err(|_| Error::KeyLength { got: bytes.len() })?;
        Ok(Key(a))
    }

    /// The key bytes.
    pub fn as_bytes(&self) -> &[u8; N] {
        &self.0
    }
}

impl<const N: usize> Drop for Key<N> {
    fn drop(&mut self) {
        self.0.zeroize();
    }
}

impl<const N: usize> fmt::Debug for Key<N> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Key<{N}>(..)")
    }
}

/// An AES key of 128, 192 or 256 bits (FIPS 197 section 5).
#[derive(Clone, Debug)]
pub enum AesKey {
    /// AES-128: 16 bytes.
    Aes128(Key<16>),
    /// AES-192: 24 bytes.
    Aes192(Key<24>),
    /// AES-256: 32 bytes.
    Aes256(Key<32>),
}

impl AesKey {
    /// The key size chosen by the slice length (16, 24 or 32 bytes); [`Error::KeyLength`] for any other length.
    ///
    /// ```
    /// use sdrkit_algo_crypto::AesKey;
    /// assert!(matches!(AesKey::from_slice(&[0u8; 32])?, AesKey::Aes256(_)));
    /// assert!(AesKey::from_slice(&[0u8; 20]).is_err());
    /// # Ok::<(), sdrkit_algo_crypto::Error>(())
    /// ```
    pub fn from_slice(bytes: &[u8]) -> Result<Self, Error> {
        match bytes.len() {
            16 => Key::from_slice(bytes).map(AesKey::Aes128),
            24 => Key::from_slice(bytes).map(AesKey::Aes192),
            32 => Key::from_slice(bytes).map(AesKey::Aes256),
            got => Err(Error::KeyLength { got }),
        }
    }

    /// The key length in bytes (16, 24 or 32).
    pub fn len(&self) -> usize {
        self.as_slice().len()
    }

    /// Always `false`: an AES key is never empty. (Present because `len` is.)
    pub fn is_empty(&self) -> bool {
        false
    }

    /// The key bytes.
    pub fn as_slice(&self) -> &[u8] {
        match self {
            AesKey::Aes128(k) => k.as_bytes(),
            AesKey::Aes192(k) => k.as_bytes(),
            AesKey::Aes256(k) => k.as_bytes(),
        }
    }
}

impl From<Key<16>> for AesKey {
    fn from(k: Key<16>) -> Self {
        AesKey::Aes128(k)
    }
}

impl From<Key<24>> for AesKey {
    fn from(k: Key<24>) -> Self {
        AesKey::Aes192(k)
    }
}

impl From<Key<32>> for AesKey {
    fn from(k: Key<32>) -> Self {
        AesKey::Aes256(k)
    }
}
