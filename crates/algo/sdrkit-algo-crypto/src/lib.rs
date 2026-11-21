//! Cryptography for keys the user supplies.
//!
//! It wraps audited, permissively licensed implementations: the RustCrypto crates (`aes`, `ctr`, `ccm`, `aes-gcm`, `cmac`, `des`,
//! `sha2`, `sha1`, `hmac`) and the dalek crates (`x25519-dalek`, `ed25519-dalek`). The wrappers fix the key and nonce sizes in
//! the types, work in place on caller buffers, and return every failure as an [`Error`].
//!
//! **Scope.** Decryption with keys the user holds (a Meshtastic channel key, including the published default key; the user's own
//! Meshtastic or MeshCore identity; the user's own LoRaWAN session keys; a satellite operator's key file the user was issued), and
//! encryption and signing for the user's own transmissions. Nothing here searches for, guesses or recovers a key.
//!
//! # Modules
//!
//! | Module | What it holds |
//! |---|---|
//! | [`key`] | [`Key`], fixed-size secret bytes wiped on drop, and [`AesKey`] for the three AES key sizes |
//! | [`cipher`](mod@cipher) | [`Aes`] (FIPS 197) with ECB and CTR (SP 800-38A), and single [`Des`] (FIPS 46-3) with ECB |
//! | [`aead`] | [`Ccm`] (SP 800-38C, RFC 3610) for every tag and nonce size, and [`Gcm`] (SP 800-38D) |
//! | [`mac`] | [`Cmac`] (SP 800-38B, RFC 4493) and [`HmacSha256`] (RFC 2104, RFC 4231), with truncated-tag checks |
//! | [`hash`] | [`sha256`] and the incremental [`Sha256`], [`sha1()`] and [`Sha1`] for protocols that name SHA-1 (FIPS 180-4) |
//! | [`curve`] | [`X25519Secret`] (RFC 7748) and [`Ed25519Key`] (RFC 8032), with the Ed25519 to X25519 conversions |
//!
//! # Conventions
//!
//! - Keys have fixed sizes in their types ([`Key<16>`](Key), [`Key<32>`](Key)). A slice of the wrong length is refused with
//!   [`Error::KeyLength`]. Key bytes are wiped when the value is dropped and are never printed by `Debug`.
//! - Encryption and decryption work in place on the caller's buffer. A keyed object ([`Aes`], [`Ccm`], [`Gcm`], [`Cmac`],
//!   [`HmacSha256`]) expands its key once and can be used for any number of messages.
//! - Bad lengths, a failed tag check, an invalid public key or an invalid signature return an [`Error`].
//! - When an AEAD tag check fails, the buffer is zeroed (see [`aead`]). Tags are compared in constant time.
//!
//! # Example
//!
//! Decrypting a Meshtastic channel packet with the published default key (AES-128-CTR, the counter block built from the
//! packet id and the sender):
//!
//! ```
//! use sdrkit_algo_crypto::{Aes, AesKey, Counter, Key};
//!
//! let key = AesKey::Aes128(Key::new([
//!     0xD4, 0xF1, 0xBB, 0x3A, 0x20, 0x29, 0x07, 0x59, 0xF0, 0xBC, 0xFF, 0xAB, 0xCF, 0x4E, 0x69, 0x01,
//! ]));
//! let aes = Aes::new(&key);
//! let mut counter = [0u8; 16];
//! counter[0..4].copy_from_slice(&0x13B2_D662u32.to_le_bytes()); // packet id
//! counter[8..12].copy_from_slice(&0x0000_0929u32.to_le_bytes()); // sender
//! let mut body = [0x46, 0xF2, 0xEC, 0xD0, 0xB0, 0x2A, 0xC4, 0xCF, 0x2B, 0xE5];
//! aes.apply_ctr(&counter, Counter::Low32, &mut body)?;
//! assert_eq!(&body[4..8], b"test");
//! # Ok::<(), sdrkit_algo_crypto::Error>(())
//! ```

#![warn(missing_docs)]
#![forbid(unsafe_code)]

pub mod aead;
pub mod blake2s;
pub mod cipher;
pub mod curve;
pub mod hash;
pub mod key;
pub mod mac;

pub use aead::{Ccm, Gcm};
pub use cipher::{Aes, Counter, Des};
pub use curve::{Ed25519Key, X25519Secret};
pub use hash::{sha1, sha256, Sha1, Sha256};
pub use key::{AesKey, Key};
pub use mac::{Cmac, HmacSha256};

use std::fmt;

/// Why an operation failed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum Error {
    /// A key had the wrong number of bytes.
    KeyLength {
        /// The number of bytes given.
        got: usize,
    },
    /// The data is not a whole number of cipher blocks (ECB works on whole blocks only).
    NotWholeBlocks {
        /// The cipher's block size in bytes.
        block: usize,
        /// The number of bytes given.
        got: usize,
    },
    /// The message (or the associated data) is longer than the mode allows with these parameters.
    TooLong,
    /// A tag to check was empty or longer than the MAC.
    TagLength,
    /// An output buffer had a different length from the input.
    OutputLength,
    /// The tag did not match: wrong key, wrong nonce, or the message was changed.
    Authentication,
    /// A public key is not a valid curve point.
    PublicKey,
    /// An X25519 exchange provided the all-zero secret: the peer's key is a low-order point (RFC 7748 section 6.1).
    NonContributory,
    /// An Ed25519 signature is not valid for this key and message.
    Signature,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::KeyLength { got } => write!(f, "a key of {got} bytes has the wrong length"),
            Error::NotWholeBlocks { block, got } => {
                write!(
                    f,
                    "{got} bytes are not a whole number of {block}-byte blocks"
                )
            }
            Error::TooLong => f.write_str("the message is too long for this mode"),
            Error::TagLength => f.write_str("the tag is empty or longer than the MAC"),
            Error::OutputLength => f.write_str("the output buffer is not the length of the input"),
            Error::Authentication => {
                f.write_str("the tag does not match (wrong key or changed message)")
            }
            Error::PublicKey => f.write_str("the public key is not a valid curve point"),
            Error::NonContributory => f.write_str("the peer's public key is a low-order point"),
            Error::Signature => f.write_str("the signature is not valid"),
        }
    }
}

impl std::error::Error for Error {}
