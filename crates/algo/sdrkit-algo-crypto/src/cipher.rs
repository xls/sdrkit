//! Block ciphers: AES with ECB and CTR, and single DES with ECB. On aarch64 the `aes` crate uses the ARMv8 AES instructions only
//! when built with `RUSTFLAGS="--cfg aes_armv8"`. Without it, it uses its constant-time software implementation, which is
//! several times slower.
//!
//! Standard: FIPS 197 (AES), NIST SP 800-38A (ECB, CTR), FIPS 46-3 (DES).

use crate::{AesKey, Error};
use ::aes::cipher::consts::{U16, U8};
use ::aes::cipher::generic_array::GenericArray;
use ::aes::cipher::inout::InOutBuf;
use ::aes::cipher::{
    BlockCipher, BlockDecrypt, BlockEncrypt, BlockSizeUser, InnerIvInit, KeyInit, StreamCipher,
    StreamCipherCoreWrapper,
};
use ::ctr::flavors::{Ctr128BE, Ctr32BE, Ctr64BE, CtrFlavor};
use ::ctr::CtrCore;

/// AES with an expanded key, for any of the three key sizes.
///
/// ```
/// use sdrkit_algo_crypto::{Aes, AesKey, Key};
/// // FIPS 197 Appendix C.1.
/// let aes = Aes::new(&AesKey::Aes128(Key::new(core::array::from_fn(|i| i as u8))));
/// let mut b: [u8; 16] = core::array::from_fn(|i| (i as u8) * 0x11);
/// aes.encrypt_block(&mut b);
/// assert_eq!(b[..4], [0x69, 0xC4, 0xE0, 0xD8]);
/// aes.decrypt_block(&mut b);
/// assert_eq!(b[1], 0x11);
/// ```
#[derive(Clone)]
pub struct Aes(Inner);

#[derive(Clone)]
enum Inner {
    K128(::aes::Aes128),
    K192(::aes::Aes192),
    K256(::aes::Aes256),
}

/// Runs `$body` with `$c` bound to the expanded cipher, whatever its key size.
macro_rules! with_aes {
    ($aes:expr, $c:ident => $body:expr) => {
        match &$aes.0 {
            Inner::K128($c) => $body,
            Inner::K192($c) => $body,
            Inner::K256($c) => $body,
        }
    };
}

/// Which part of a CTR counter block is incremented (big-endian, wrapping within that part) from one block to the next.
///
/// SP 800-38A Appendix B.1 increments the low `m` bits of the counter block; protocols choose `m`. GCM and CCM use the low 32
/// bits; most packet protocols never reach 2^32 blocks, so for them [`Counter::Low32`] and [`Counter::Full128`] give the same
/// keystream. They differ only when the low 32 bits roll over: `Low32` wraps them to zero, `Full128` carries into the rest.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Counter {
    /// The last 4 bytes are the counter (Meshtastic, LoRaWAN `A_i` blocks, GCM, CCM).
    Low32,
    /// The last 8 bytes are the counter.
    Low64,
    /// The whole 16-byte block is one counter (SP 800-38A F.5 examples).
    Full128,
}

impl Aes {
    /// Expands `key`.
    pub fn new(key: &AesKey) -> Self {
        // A `Key<N>` always has the length the cipher requires, so the conversions below cannot fail.
        Aes(match key {
            AesKey::Aes128(k) => {
                Inner::K128(::aes::Aes128::new(GenericArray::from_slice(k.as_bytes())))
            }
            AesKey::Aes192(k) => {
                Inner::K192(::aes::Aes192::new(GenericArray::from_slice(k.as_bytes())))
            }
            AesKey::Aes256(k) => {
                Inner::K256(::aes::Aes256::new(GenericArray::from_slice(k.as_bytes())))
            }
        })
    }

    /// The key length in bytes (16, 24 or 32).
    pub fn key_len(&self) -> usize {
        match self.0 {
            Inner::K128(_) => 16,
            Inner::K192(_) => 24,
            Inner::K256(_) => 32,
        }
    }

    /// Encrypts one 16-byte block in place (the cipher function of FIPS 197 section 5.1).
    pub fn encrypt_block(&self, block: &mut [u8; 16]) {
        with_aes!(self, c => c.encrypt_block(GenericArray::from_mut_slice(block)))
    }

    /// Decrypts one 16-byte block in place (the inverse cipher of FIPS 197 section 5.3).
    pub fn decrypt_block(&self, block: &mut [u8; 16]) {
        with_aes!(self, c => c.decrypt_block(GenericArray::from_mut_slice(block)))
    }

    /// ECB encryption in place (SP 800-38A section 6.1); `data` must be a whole number of 16-byte blocks.
    ///
    /// Padding is the protocol's business: MeshCore pads with zeros, LoRaWAN Join-Accept is always whole blocks.
    pub fn encrypt_ecb(&self, data: &mut [u8]) -> Result<(), Error> {
        let blocks = whole_blocks::<U16>(data)?;
        with_aes!(self, c => c.encrypt_blocks_inout(blocks));
        Ok(())
    }

    /// ECB decryption in place (SP 800-38A section 6.1); `data` must be a whole number of 16-byte blocks.
    pub fn decrypt_ecb(&self, data: &mut [u8]) -> Result<(), Error> {
        let blocks = whole_blocks::<U16>(data)?;
        with_aes!(self, c => c.decrypt_blocks_inout(blocks));
        Ok(())
    }

    /// CTR mode in place (SP 800-38A section 6.5): XORs `data` with the encryptions of `counter`, `counter + 1`, ... where the
    /// increment follows `width`. Encryption and decryption are the same operation. Any length is allowed; a final partial
    /// block uses the start of its keystream block.
    ///
    /// [`Error::TooLong`] when `data` needs more blocks than the counter part can number before it would repeat.
    pub fn apply_ctr(
        &self,
        counter: &[u8; 16],
        width: Counter,
        data: &mut [u8],
    ) -> Result<(), Error> {
        with_aes!(self, c => match width {
            Counter::Low32 => ctr::<_, Ctr32BE>(c, counter, data),
            Counter::Low64 => ctr::<_, Ctr64BE>(c, counter, data),
            Counter::Full128 => ctr::<_, Ctr128BE>(c, counter, data),
        })
    }
}

impl std::fmt::Debug for Aes {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Aes{}(..)", self.key_len() * 8)
    }
}

fn ctr<C, F>(cipher: &C, counter: &[u8; 16], data: &mut [u8]) -> Result<(), Error>
where
    C: BlockEncrypt + BlockCipher + BlockSizeUser<BlockSize = U16>,
    F: CtrFlavor<U16>,
{
    let core = CtrCore::<&C, F>::inner_iv_init(cipher, GenericArray::from_slice(counter));
    StreamCipherCoreWrapper::from_core(core)
        .try_apply_keystream(data)
        .map_err(|_| Error::TooLong)
}

fn whole_blocks<N: ::aes::cipher::ArrayLength<u8>>(
    data: &mut [u8],
) -> Result<InOutBuf<'_, '_, GenericArray<u8, N>>, Error> {
    let got = data.len();
    let (blocks, tail) = InOutBuf::from(data).into_chunks::<N>();
    if tail.is_empty() {
        Ok(blocks)
    } else {
        Err(Error::NotWholeBlocks {
            block: N::USIZE,
            got,
        })
    }
}

/// Single DES (FIPS 46-3) with an 8-byte key, for the GK-2A LRIT/HRIT file layer (KMA key files).
///
/// Single DES is weak; it is here because a standard in use requires it, with the key the user was issued. Parity bits of the
/// key (the low bit of each byte) are ignored, as FIPS 46-3 section 2 says.
///
/// ```
/// use sdrkit_algo_crypto::Des;
/// // The worked example of FIPS 46 (key 133457799BBCDFF1).
/// let des = Des::new(&[0x13, 0x34, 0x57, 0x79, 0x9B, 0xBC, 0xDF, 0xF1]);
/// let mut d = [0x01, 0x23, 0x45, 0x67, 0x89, 0xAB, 0xCD, 0xEF];
/// des.encrypt_ecb(&mut d)?;
/// assert_eq!(d, [0x85, 0xE8, 0x13, 0x54, 0x0F, 0x0A, 0xB4, 0x05]);
/// # Ok::<(), sdrkit_algo_crypto::Error>(())
/// ```
#[derive(Clone)]
pub struct Des(::des::Des);

impl Des {
    /// Expands an 8-byte key.
    pub fn new(key: &[u8; 8]) -> Self {
        Des(::des::Des::new(GenericArray::from_slice(key)))
    }

    /// ECB encryption in place; `data` must be a whole number of 8-byte blocks. (A protocol that leaves a short tail in clear,
    /// as GK-2A does, passes only the whole blocks.)
    pub fn encrypt_ecb(&self, data: &mut [u8]) -> Result<(), Error> {
        self.0.encrypt_blocks_inout(whole_blocks::<U8>(data)?);
        Ok(())
    }

    /// ECB decryption in place; `data` must be a whole number of 8-byte blocks.
    pub fn decrypt_ecb(&self, data: &mut [u8]) -> Result<(), Error> {
        self.0.decrypt_blocks_inout(whole_blocks::<U8>(data)?);
        Ok(())
    }
}

impl std::fmt::Debug for Des {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Des(..)")
    }
}
