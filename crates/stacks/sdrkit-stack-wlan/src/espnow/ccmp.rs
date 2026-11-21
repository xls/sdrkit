//! CCMP of IEEE Std 802.11-2020 clause 12.5.3 for a frame with a 24-byte MAC header: the CCMP header, the nonce, the
//! additional authenticated data, and sealing and opening with AES-128 in CCM with an 8-byte MIC and a 2-byte length field.
//! The nonce flag of a management frame (bit 4) is that of IEEE Std 802.11w, now in 802.11-2020.

use sdrkit_algo_crypto::{AesKey, Ccm, Key};

/// The length of the MAC header that this module handles (no address 4, no QoS control).
pub const HEADER_LEN: usize = 24;
/// The length of the CCMP header.
pub const CCMP_HEADER_LEN: usize = 8;
/// The length of the MIC.
pub const MIC_LEN: usize = 8;
/// The nonce flags of a management frame: bit 4 set, priority 0.
pub const FLAG_MANAGEMENT: u8 = 0x10;

/// AES-128 in CCM with the sizes of CCMP: an 8-byte MIC and a 13-byte nonce.
pub type CcmpCipher = Ccm<MIC_LEN, 13>;

/// A cipher for a 16-byte key.
pub fn cipher(key: &[u8; 16]) -> CcmpCipher {
    Ccm::new(&AesKey::Aes128(Key::new(*key)))
}

/// The 8-byte CCMP header of packet number `pn` (48 bits) and key id `key_id` (0 to 3), ExtIV set.
pub fn ccmp_header(pn: u64, key_id: u8) -> [u8; CCMP_HEADER_LEN] {
    let p = pn.to_le_bytes();
    [p[0], p[1], 0, 0x20 | ((key_id & 3) << 6), p[2], p[3], p[4], p[5]]
}

/// The packet number and the key id of a CCMP header; `None` when ExtIV is clear.
pub fn read_ccmp_header(h: &[u8; CCMP_HEADER_LEN]) -> Option<(u64, u8)> {
    if h[3] & 0x20 == 0 {
        return None;
    }
    let pn = u64::from_le_bytes([h[0], h[1], h[4], h[5], h[6], h[7], 0, 0]);
    Some((pn, h[3] >> 6))
}

/// The 13-byte nonce: `flags`, address 2 of `header`, then the packet number from PN5 to PN0.
pub fn nonce(flags: u8, header: &[u8; HEADER_LEN], pn: u64) -> [u8; 13] {
    let mut n = [0u8; 13];
    n[0] = flags;
    n[1..7].copy_from_slice(&header[10..16]);
    let p = pn.to_be_bytes();
    n[7..13].copy_from_slice(&p[2..8]);
    n
}

/// The 22-byte additional authenticated data of `header`: frame control with retry, power management and more data
/// cleared and Protected set (in a data frame also subtype bits 4 to 6 cleared), addresses 1 to 3, and sequence control
/// with only the fragment number kept.
pub fn aad(header: &[u8; HEADER_LEN]) -> [u8; 22] {
    let mut a = [0u8; 22];
    let ftype = (header[0] >> 2) & 3;
    a[0] = if ftype == 2 {
        header[0] & 0x8F
    } else {
        header[0]
    };
    a[1] = (header[1] & !0x38) | 0x40;
    a[2..20].copy_from_slice(&header[4..22]);
    a[20] = header[22] & 0x0F;
    a
}

/// Seals `body` in place under `cipher` and returns the MIC. `header` must already have Protected set.
///
/// # Errors
///
/// [`sdrkit_algo_crypto::Error`] for a body longer than CCM allows.
pub fn seal(
    cipher: &CcmpCipher,
    flags: u8,
    header: &[u8; HEADER_LEN],
    pn: u64,
    body: &mut [u8],
) -> Result<[u8; MIC_LEN], sdrkit_algo_crypto::Error> {
    cipher.encrypt_in_place(&nonce(flags, header, pn), &aad(header), body)
}

/// Opens `ciphertext` into `out` (same length) under `cipher`; `false` when the MIC does not hold.
pub fn open(
    cipher: &CcmpCipher,
    flags: u8,
    header: &[u8; HEADER_LEN],
    pn: u64,
    ciphertext: &[u8],
    mic: &[u8; MIC_LEN],
    out: &mut [u8],
) -> bool {
    cipher
        .decrypt_to(&nonce(flags, header, pn), &aad(header), ciphertext, mic, out)
        .is_ok()
}

/// The parts of a protected frame.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Protected<'a> {
    /// The MAC header.
    pub header: &'a [u8; HEADER_LEN],
    /// The packet number.
    pub pn: u64,
    /// The key id.
    pub key_id: u8,
    /// The encrypted body, after the CCMP header and before the MIC.
    pub ciphertext: &'a [u8],
    /// The MIC.
    pub mic: &'a [u8; MIC_LEN],
}

/// The parts of a protected frame `mpdu` (FCS removed). `None` for a frame too short for a CCMP header and a MIC, or with
/// ExtIV clear.
pub fn split(mpdu: &[u8]) -> Option<Protected<'_>> {
    if mpdu.len() < HEADER_LEN + CCMP_HEADER_LEN + MIC_LEN {
        return None;
    }
    let header: &[u8; HEADER_LEN] = mpdu[..HEADER_LEN].try_into().ok()?;
    let ch: &[u8; CCMP_HEADER_LEN] = mpdu[HEADER_LEN..HEADER_LEN + CCMP_HEADER_LEN]
        .try_into()
        .ok()?;
    let (pn, key_id) = read_ccmp_header(ch)?;
    let end = mpdu.len() - MIC_LEN;
    Some(Protected {
        header,
        pn,
        key_id,
        ciphertext: &mpdu[HEADER_LEN + CCMP_HEADER_LEN..end],
        mic: mpdu[end..].try_into().ok()?,
    })
}
