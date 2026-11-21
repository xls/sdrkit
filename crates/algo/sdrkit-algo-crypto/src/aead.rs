//! Authenticated encryption: AES-CCM and AES-GCM, in place on the caller's buffer with a detached tag. When a tag check fails,
//! the buffer is zeroed. The `decrypt_to` methods leave the input unchanged and write into a second buffer, for a caller that
//! tries one ciphertext with several keys.
//!
//! Standard: NIST SP 800-38C and RFC 3610 (CCM), NIST SP 800-38D (GCM).

use crate::{AesKey, Error};
use ::aes::cipher::generic_array::GenericArray;
use ::aes::cipher::KeyInit;
use ::aes::{Aes128, Aes192, Aes256};
use ::ccm::aead::AeadInPlace;
use ::ccm::consts::{U10, U11, U12, U13, U14, U16, U4, U6, U7, U8, U9};

mod sealed {
    pub trait Sealed {}
}

/// The tag and nonce sizes CCM allows (SP 800-38C section A.1: tag 4, 6, 8, 10, 12, 14 or 16 bytes; nonce 7 to 13 bytes).
///
/// Implemented for [`CcmParams<TAG, NONCE>`](CcmParams) with exactly those sizes; it cannot be implemented outside this crate.
pub trait CcmSize: sealed::Sealed {
    #[doc(hidden)]
    type Tag: ::ccm::aead::generic_array::ArrayLength<u8> + ::ccm::TagSize;
    #[doc(hidden)]
    type Nonce: ::ccm::aead::generic_array::ArrayLength<u8> + ::ccm::NonceSize;
}

/// A pair of CCM sizes in bytes: the tag length `TAG` and the nonce length `NONCE`. Used only as a bound.
pub struct CcmParams<const TAG: usize, const NONCE: usize>;

macro_rules! ccm_sizes {
    ($( ($t:literal, $tt:ty) ),* ; $nonces:tt) => {
        $( ccm_sizes!(@tag $t, $tt, $nonces); )*
    };
    (@tag $t:literal, $tt:ty, [ $( ($n:literal, $nt:ty) ),* ]) => {
        $(
            impl sealed::Sealed for CcmParams<$t, $n> {}
            impl CcmSize for CcmParams<$t, $n> {
                type Tag = $tt;
                type Nonce = $nt;
            }
        )*
    };
}

ccm_sizes!(
    (4, U4), (6, U6), (8, U8), (10, U10), (12, U12), (14, U14), (16, U16);
    [(7, U7), (8, U8), (9, U9), (10, U10), (11, U11), (12, U12), (13, U13)]
);

/// AES-CCM with a `TAG`-byte tag and a `NONCE`-byte nonce (SP 800-38C; RFC 3610 calls them M and 15 − L).
///
/// The message may be at most 2^(8·(15 − `NONCE`)) − 1 bytes: 65,535 with a 13-byte nonce.
///
/// Meshtastic direct messages use `Ccm<8, 13>` with AES-256; Bluetooth LE and IEEE 802.15.4 use 13-byte nonces with 4-, 8- or
/// 16-byte tags.
///
/// ```
/// use sdrkit_algo_crypto::{AesKey, Ccm, Key};
/// // SP 800-38C Appendix C, Example 1.
/// let ccm = Ccm::<4, 7>::new(&AesKey::Aes128(Key::new(core::array::from_fn(|i| 0x40 + i as u8))));
/// let nonce = [0x10, 0x11, 0x12, 0x13, 0x14, 0x15, 0x16];
/// let aad = [0, 1, 2, 3, 4, 5, 6, 7];
/// let mut msg = [0x20, 0x21, 0x22, 0x23];
/// let tag = ccm.encrypt_in_place(&nonce, &aad, &mut msg)?;
/// assert_eq!((msg, tag), ([0x71, 0x62, 0x01, 0x5b], [0x4d, 0xac, 0x25, 0x5d]));
/// ccm.decrypt_in_place(&nonce, &aad, &mut msg, &tag)?;
/// assert_eq!(msg, [0x20, 0x21, 0x22, 0x23]);
/// # Ok::<(), sdrkit_algo_crypto::Error>(())
/// ```
pub struct Ccm<const TAG: usize, const NONCE: usize>
where
    CcmParams<TAG, NONCE>: CcmSize,
{
    inner: CcmInner<CcmParams<TAG, NONCE>>,
}

enum CcmInner<P: CcmSize> {
    K128(::ccm::Ccm<Aes128, P::Tag, P::Nonce>),
    K192(::ccm::Ccm<Aes192, P::Tag, P::Nonce>),
    K256(::ccm::Ccm<Aes256, P::Tag, P::Nonce>),
}

macro_rules! with_ccm {
    ($inner:expr, $c:ident => $body:expr) => {
        match $inner {
            CcmInner::K128($c) => $body,
            CcmInner::K192($c) => $body,
            CcmInner::K256($c) => $body,
        }
    };
}

impl<const TAG: usize, const NONCE: usize> Ccm<TAG, NONCE>
where
    CcmParams<TAG, NONCE>: CcmSize,
{
    /// The longest message these sizes allow, in bytes.
    pub const MAX_LEN: usize = {
        let l = 15 - NONCE;
        if l >= std::mem::size_of::<usize>() {
            usize::MAX
        } else {
            (1usize << (8 * l)) - 1
        }
    };

    /// Expands `key`.
    pub fn new(key: &AesKey) -> Self {
        let inner = match key {
            AesKey::Aes128(k) => CcmInner::K128(::ccm::Ccm::from(Aes128::new(
                GenericArray::from_slice(k.as_bytes()),
            ))),
            AesKey::Aes192(k) => CcmInner::K192(::ccm::Ccm::from(Aes192::new(
                GenericArray::from_slice(k.as_bytes()),
            ))),
            AesKey::Aes256(k) => CcmInner::K256(::ccm::Ccm::from(Aes256::new(
                GenericArray::from_slice(k.as_bytes()),
            ))),
        };
        Ccm { inner }
    }

    /// Encrypts `buf` in place and returns the tag. `aad` is authenticated but not encrypted.
    ///
    /// [`Error::TooLong`] when `buf` is longer than [`Self::MAX_LEN`].
    pub fn encrypt_in_place(
        &self,
        nonce: &[u8; NONCE],
        aad: &[u8],
        buf: &mut [u8],
    ) -> Result<[u8; TAG], Error> {
        if buf.len() > Self::MAX_LEN {
            return Err(Error::TooLong);
        }
        let n = GenericArray::from_slice(nonce);
        let t = with_ccm!(&self.inner, c => c.encrypt_in_place_detached(n, aad, buf))
            .map_err(|_| Error::TooLong)?;
        let mut tag = [0u8; TAG];
        tag.copy_from_slice(&t);
        Ok(tag)
    }

    /// Decrypts `buf` in place and checks `tag`. On [`Error::Authentication`] the buffer is zeroed.
    pub fn decrypt_in_place(
        &self,
        nonce: &[u8; NONCE],
        aad: &[u8],
        buf: &mut [u8],
        tag: &[u8; TAG],
    ) -> Result<(), Error> {
        if buf.len() > Self::MAX_LEN {
            return Err(Error::TooLong);
        }
        let n = GenericArray::from_slice(nonce);
        let t = GenericArray::from_slice(tag);
        with_ccm!(&self.inner, c => c.decrypt_in_place_detached(n, aad, buf, t))
            .map_err(|_| Error::Authentication)
    }

    /// Decrypts `ciphertext` into `out` (which must have the same length) and checks `tag`; `ciphertext` is left as it was, so
    /// it can be tried with another key. On [`Error::Authentication`] `out` is zeroed.
    pub fn decrypt_to(
        &self,
        nonce: &[u8; NONCE],
        aad: &[u8],
        ciphertext: &[u8],
        tag: &[u8; TAG],
        out: &mut [u8],
    ) -> Result<(), Error> {
        if out.len() != ciphertext.len() {
            return Err(Error::OutputLength);
        }
        out.copy_from_slice(ciphertext);
        self.decrypt_in_place(nonce, aad, out, tag)
    }
}

impl<const TAG: usize, const NONCE: usize> Clone for Ccm<TAG, NONCE>
where
    CcmParams<TAG, NONCE>: CcmSize,
{
    fn clone(&self) -> Self {
        let inner = match &self.inner {
            CcmInner::K128(c) => CcmInner::K128(c.clone()),
            CcmInner::K192(c) => CcmInner::K192(c.clone()),
            CcmInner::K256(c) => CcmInner::K256(c.clone()),
        };
        Ccm { inner }
    }
}

impl<const TAG: usize, const NONCE: usize> std::fmt::Debug for Ccm<TAG, NONCE>
where
    CcmParams<TAG, NONCE>: CcmSize,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Ccm<{TAG}, {NONCE}>(..)")
    }
}

/// AES-GCM with a 96-bit nonce and a 128-bit tag (SP 800-38D; the nonce length section 8.2 recommends).
///
/// Used by the Flipper Zero Sub-GHz chat (AES-256-GCM). A nonce must never be used twice with the same key: SP 800-38D
/// section 8 explains that a repeat reveals the authentication key.
///
/// ```
/// use sdrkit_algo_crypto::{AesKey, Gcm, Key};
/// // The GCM specification's Test Case 2 (AES-128, zero key and nonce, one zero block).
/// let gcm = Gcm::new(&AesKey::Aes128(Key::new([0; 16])));
/// let mut msg = [0u8; 16];
/// let tag = gcm.encrypt_in_place(&[0; 12], &[], &mut msg)?;
/// assert_eq!(msg[..4], [0x03, 0x88, 0xda, 0xce]);
/// assert_eq!(tag[..4], [0xab, 0x6e, 0x47, 0xd4]);
/// # Ok::<(), sdrkit_algo_crypto::Error>(())
/// ```
#[derive(Clone)]
pub struct Gcm(GcmInner);

#[derive(Clone)]
enum GcmInner {
    K128(::aes_gcm::AesGcm<Aes128, U12>),
    K192(::aes_gcm::AesGcm<Aes192, U12>),
    K256(::aes_gcm::AesGcm<Aes256, U12>),
}

macro_rules! with_gcm {
    ($inner:expr, $c:ident => $body:expr) => {
        match $inner {
            GcmInner::K128($c) => $body,
            GcmInner::K192($c) => $body,
            GcmInner::K256($c) => $body,
        }
    };
}

impl Gcm {
    /// The longest message, in bytes: 2^39 − 256 bits (SP 800-38D section 5.2.1.1).
    pub const MAX_LEN: u64 = (1 << 36) - 32;

    /// Expands `key`.
    pub fn new(key: &AesKey) -> Self {
        Gcm(match key {
            AesKey::Aes128(k) => {
                GcmInner::K128(Aes128::new(GenericArray::from_slice(k.as_bytes())).into())
            }
            AesKey::Aes192(k) => {
                GcmInner::K192(Aes192::new(GenericArray::from_slice(k.as_bytes())).into())
            }
            AesKey::Aes256(k) => {
                GcmInner::K256(Aes256::new(GenericArray::from_slice(k.as_bytes())).into())
            }
        })
    }

    /// Encrypts `buf` in place and returns the 16-byte tag. `aad` is authenticated but not encrypted.
    pub fn encrypt_in_place(
        &self,
        nonce: &[u8; 12],
        aad: &[u8],
        buf: &mut [u8],
    ) -> Result<[u8; 16], Error> {
        let n = GenericArray::from_slice(nonce);
        let t = with_gcm!(&self.0, c => c.encrypt_in_place_detached(n, aad, buf))
            .map_err(|_| Error::TooLong)?;
        Ok(t.into())
    }

    /// Checks `tag` and decrypts `buf` in place. On [`Error::Authentication`] the buffer is zeroed.
    pub fn decrypt_in_place(
        &self,
        nonce: &[u8; 12],
        aad: &[u8],
        buf: &mut [u8],
        tag: &[u8; 16],
    ) -> Result<(), Error> {
        if buf.len() as u64 > Self::MAX_LEN {
            return Err(Error::TooLong);
        }
        let n = GenericArray::from_slice(nonce);
        let t = GenericArray::from_slice(tag);
        with_gcm!(&self.0, c => c.decrypt_in_place_detached(n, aad, buf, t)).map_err(|_| {
            buf.fill(0);
            Error::Authentication
        })
    }

    /// Decrypts `ciphertext` into `out` (which must have the same length) and checks `tag`; `ciphertext` is left as it was. On
    /// [`Error::Authentication`] `out` is zeroed.
    pub fn decrypt_to(
        &self,
        nonce: &[u8; 12],
        aad: &[u8],
        ciphertext: &[u8],
        tag: &[u8; 16],
        out: &mut [u8],
    ) -> Result<(), Error> {
        if out.len() != ciphertext.len() {
            return Err(Error::OutputLength);
        }
        out.copy_from_slice(ciphertext);
        self.decrypt_in_place(nonce, aad, out, tag)
    }
}

impl std::fmt::Debug for Gcm {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Gcm(..)")
    }
}
