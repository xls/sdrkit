//! Message authentication codes: AES-CMAC and HMAC-SHA256. Both are keyed objects that take the message in pieces.
//! `finalize_reset` returns the MAC and resets the object for the next message under the same key. `verify_truncated_reset`
//! compares the first bytes of the MAC with a received tag in constant time, for protocols that send a truncated MAC.
//!
//! Standard: NIST SP 800-38B and RFC 4493 (CMAC), RFC 2104 with FIPS 180-4 (HMAC-SHA256).

use crate::{AesKey, Error};
use ::aes::cipher::generic_array::GenericArray;
use ::aes::{Aes128, Aes192, Aes256};
use ::hmac::digest::{FixedOutputReset, KeyInit, Update};
use subtle::ConstantTimeEq;
use zeroize::Zeroize;

/// Compares `tag` with the first `tag.len()` bytes of `mac` in constant time.
fn check_truncated(mac: &[u8], tag: &[u8]) -> Result<(), Error> {
    if tag.is_empty() || tag.len() > mac.len() {
        return Err(Error::TagLength);
    }
    if bool::from(mac[..tag.len()].ct_eq(tag)) {
        Ok(())
    } else {
        Err(Error::Authentication)
    }
}

/// AES-CMAC (SP 800-38B), the MAC of LoRaWAN 1.0.x and 1.1 (LoRaWAN 1.0.4 section 4.4 and 6.2.2).
///
/// ```
/// use sdrkit_algo_crypto::{AesKey, Cmac, Key};
/// // RFC 4493 section 4, Example 1 (empty message).
/// let key = AesKey::from_slice(&[
///     0x2b, 0x7e, 0x15, 0x16, 0x28, 0xae, 0xd2, 0xa6, 0xab, 0xf7, 0x15, 0x88, 0x09, 0xcf, 0x4f, 0x3c,
/// ])?;
/// let mut mac = Cmac::new(&key);
/// let t = mac.finalize_reset();
/// assert_eq!(t[..4], [0xbb, 0x1d, 0x69, 0x29]);
/// assert!(mac.verify_truncated_reset(&t[..4]).is_ok());
/// # Ok::<(), sdrkit_algo_crypto::Error>(())
/// ```
#[derive(Clone)]
pub struct Cmac(CmacInner);

#[derive(Clone)]
enum CmacInner {
    K128(::cmac::Cmac<Aes128>),
    K192(::cmac::Cmac<Aes192>),
    K256(::cmac::Cmac<Aes256>),
}

macro_rules! with_cmac {
    ($inner:expr, $c:ident => $body:expr) => {
        match $inner {
            CmacInner::K128($c) => $body,
            CmacInner::K192($c) => $body,
            CmacInner::K256($c) => $body,
        }
    };
}

impl Cmac {
    /// A CMAC keyed with `key`, ready for a message.
    pub fn new(key: &AesKey) -> Self {
        Cmac(match key {
            AesKey::Aes128(k) => {
                CmacInner::K128(KeyInit::new(GenericArray::from_slice(k.as_bytes())))
            }
            AesKey::Aes192(k) => {
                CmacInner::K192(KeyInit::new(GenericArray::from_slice(k.as_bytes())))
            }
            AesKey::Aes256(k) => {
                CmacInner::K256(KeyInit::new(GenericArray::from_slice(k.as_bytes())))
            }
        })
    }

    /// Adds `data` to the message.
    pub fn update(&mut self, data: &[u8]) {
        with_cmac!(&mut self.0, c => Update::update(c, data))
    }

    /// The 16-byte MAC of the message so far; the object is then ready for a new message under the same key.
    pub fn finalize_reset(&mut self) -> [u8; 16] {
        with_cmac!(&mut self.0, c => c.finalize_fixed_reset().into())
    }

    /// Checks a received tag against the first `tag.len()` bytes of the MAC (1 to 16), in constant time, and resets.
    pub fn verify_truncated_reset(&mut self, tag: &[u8]) -> Result<(), Error> {
        check_truncated(&self.finalize_reset(), tag)
    }
}

impl std::fmt::Debug for Cmac {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Cmac(..)")
    }
}

/// AES-CMAC of `data` in one call.
pub fn cmac(key: &AesKey, data: &[u8]) -> [u8; 16] {
    let mut m = Cmac::new(key);
    m.update(data);
    m.finalize_reset()
}

/// HMAC-SHA256 (RFC 2104, FIPS 198-1), as MeshCore uses it (MeshCore keys it with the channel secret padded to 32 bytes and sends
/// the first 2 bytes).
///
/// Any key length is allowed; a key longer than 64 bytes is hashed first (RFC 2104 section 2).
///
/// ```
/// use sdrkit_algo_crypto::HmacSha256;
/// // RFC 4231 Test Case 2.
/// let mut mac = HmacSha256::new(b"Jefe");
/// mac.update(b"what do ya want ");
/// mac.update(b"for nothing?");
/// assert_eq!(mac.finalize_reset()[..4], [0x5b, 0xdc, 0xc1, 0x46]);
/// ```
#[derive(Clone)]
pub struct HmacSha256(::hmac::Hmac<::sha2::Sha256>);

impl HmacSha256 {
    /// An HMAC keyed with `key`, ready for a message.
    pub fn new(key: &[u8]) -> Self {
        // RFC 2104 section 2: a key longer than the 64-byte hash input is replaced by its hash; a shorter one is padded with
        // zeros. The result is a key of exactly 64 bytes, which `Hmac` accepts without a length check.
        let mut padded = [0u8; 64];
        if key.len() > padded.len() {
            padded[..32].copy_from_slice(&crate::sha256(key));
        } else {
            padded[..key.len()].copy_from_slice(key);
        }
        let mac = KeyInit::new(GenericArray::from_slice(&padded));
        padded.zeroize();
        HmacSha256(mac)
    }

    /// Adds `data` to the message.
    pub fn update(&mut self, data: &[u8]) {
        Update::update(&mut self.0, data)
    }

    /// The 32-byte MAC of the message so far; the object is then ready for a new message under the same key.
    pub fn finalize_reset(&mut self) -> [u8; 32] {
        self.0.finalize_fixed_reset().into()
    }

    /// Checks a received tag against the first `tag.len()` bytes of the MAC (1 to 32), in constant time, and resets.
    pub fn verify_truncated_reset(&mut self, tag: &[u8]) -> Result<(), Error> {
        check_truncated(&self.finalize_reset(), tag)
    }
}

impl std::fmt::Debug for HmacSha256 {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("HmacSha256(..)")
    }
}

/// HMAC-SHA256 of `data` in one call.
pub fn hmac_sha256(key: &[u8], data: &[u8]) -> [u8; 32] {
    let mut m = HmacSha256::new(key);
    m.update(data);
    m.finalize_reset()
}
