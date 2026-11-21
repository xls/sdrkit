//! Curve25519: X25519 key agreement, Ed25519 signatures, and the conversion of Ed25519 keys to X25519 keys (the hashed scalar of
//! the seed, and the Montgomery form of the Edwards point). The functions use keys the caller already holds. To make a new key,
//! the host fills 32 bytes from the random source of the operating system.
//!
//! Standard: RFC 7748 (X25519, section 4.1 for the birational map), RFC 8032 (Ed25519).

use crate::{Error, Key};
use subtle::ConstantTimeEq;

/// The X25519 function of RFC 7748 section 5: `scalar` is clamped as that section says, `u` is a little-endian u-coordinate
/// (its top bit ignored).
///
/// ```
/// // RFC 7748 section 5.2, first test vector.
/// fn h(s: &str) -> [u8; 32] { core::array::from_fn(|i| u8::from_str_radix(&s[2 * i..2 * i + 2], 16).unwrap()) }
/// let out = sdrkit_algo_crypto::curve::x25519(
///     &h("a546e36bf0527c9d3b16154b82465edd62144c0ac1fc5a18506a2244ba449ac4"),
///     &h("e6db6867583030db3594c1a424b15f7c726624ec26b3353b10a903a6d0ab1c4c"),
/// );
/// assert_eq!(out, h("c3da55379de9c6908e94ea4df28d084f32eccf03491c71f754b4075577a28552"));
/// ```
pub fn x25519(scalar: &[u8; 32], u: &[u8; 32]) -> [u8; 32] {
    ::x25519_dalek::x25519(*scalar, *u)
}

/// The u-coordinate 9, the base point of RFC 7748 section 4.1.
pub const X25519_BASE_POINT: [u8; 32] = {
    let mut b = [0u8; 32];
    b[0] = 9;
    b
};

/// The user's own X25519 private key.
///
/// ```
/// use sdrkit_algo_crypto::{Key, X25519Secret};
/// let alice = X25519Secret::new(Key::new([1; 32]));
/// let bob = X25519Secret::new(Key::new([2; 32]));
/// let a = alice.diffie_hellman(&bob.public_key())?;
/// let b = bob.diffie_hellman(&alice.public_key())?;
/// assert_eq!(a.as_bytes(), b.as_bytes());
/// # Ok::<(), sdrkit_algo_crypto::Error>(())
/// ```
#[derive(Clone, Debug)]
pub struct X25519Secret(Key<32>);

impl X25519Secret {
    /// Takes the 32 private-key bytes (clamped at use, so clamped and unclamped forms of a key are the same key).
    pub fn new(key: Key<32>) -> Self {
        X25519Secret(key)
    }

    /// The public key: X25519 of the private key and the base point.
    pub fn public_key(&self) -> [u8; 32] {
        x25519(self.0.as_bytes(), &X25519_BASE_POINT)
    }

    /// The shared secret with a peer's public key.
    ///
    /// [`Error::NonContributory`] when the result is all zeros, which happens exactly when the peer's key is a point of small
    /// order (RFC 7748 section 6.1 says to check for it); such a "secret" is known to everyone.
    pub fn diffie_hellman(&self, peer_public: &[u8; 32]) -> Result<Key<32>, Error> {
        let shared = Key::new(x25519(self.0.as_bytes(), peer_public));
        if bool::from(shared.as_bytes().ct_eq(&[0u8; 32])) {
            return Err(Error::NonContributory);
        }
        Ok(shared)
    }
}

/// The user's own Ed25519 key (RFC 8032 section 5.1), made from its 32-byte seed ("private key" in RFC 8032).
///
/// ```
/// use sdrkit_algo_crypto::{curve, Ed25519Key, Key};
/// let me = Ed25519Key::from_seed(&Key::new([7; 32]));
/// let sig = me.sign(b"advert");
/// assert!(curve::ed25519_verify(&me.public_key(), b"advert", &sig).is_ok());
/// assert!(curve::ed25519_verify(&me.public_key(), b"advent", &sig).is_err());
/// ```
#[derive(Clone)]
pub struct Ed25519Key(::ed25519_dalek::SigningKey);

impl Ed25519Key {
    /// The key made from a 32-byte seed.
    pub fn from_seed(seed: &Key<32>) -> Self {
        Ed25519Key(::ed25519_dalek::SigningKey::from_bytes(seed.as_bytes()))
    }

    /// The 32-byte public key (the encoded point A of RFC 8032 section 5.1.5).
    pub fn public_key(&self) -> [u8; 32] {
        self.0.verifying_key().to_bytes()
    }

    /// The 64-byte signature of `msg` (RFC 8032 section 5.1.6; deterministic).
    pub fn sign(&self, msg: &[u8]) -> [u8; 64] {
        use ::ed25519_dalek::Signer;
        self.0.sign(msg).to_bytes()
    }

    /// The X25519 private key of the same identity: the first 32 bytes of SHA-512 of the seed (the scalar of RFC 8032 section
    /// 5.1.5 before clamping, which X25519 applies itself).
    pub fn x25519_secret(&self) -> X25519Secret {
        X25519Secret(Key::new(self.0.to_scalar_bytes()))
    }
}

impl std::fmt::Debug for Ed25519Key {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Ed25519Key(..)")
    }
}

/// Checks an Ed25519 signature (RFC 8032 section 5.1.7) with the strict rules: a non-canonical `S`, or a public key or `R`
/// of small order, is refused. [`Error::PublicKey`] when `public` is not a point; [`Error::Signature`] when the signature fails.
pub fn ed25519_verify(public: &[u8; 32], msg: &[u8], signature: &[u8; 64]) -> Result<(), Error> {
    let vk = ::ed25519_dalek::VerifyingKey::from_bytes(public).map_err(|_| Error::PublicKey)?;
    vk.verify_strict(msg, &::ed25519_dalek::Signature::from_bytes(signature))
        .map_err(|_| Error::Signature)
}

/// The X25519 public key (Montgomery u-coordinate) of an Ed25519 public key; [`Error::PublicKey`] when it is not a point.
pub fn ed25519_public_to_x25519(public: &[u8; 32]) -> Result<[u8; 32], Error> {
    let vk = ::ed25519_dalek::VerifyingKey::from_bytes(public).map_err(|_| Error::PublicKey)?;
    Ok(vk.to_montgomery().to_bytes())
}
