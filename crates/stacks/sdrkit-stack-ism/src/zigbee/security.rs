//! IEEE 802.15.4 MAC frame security (frame version 2006): the auxiliary security header and CCM* with AES-128. The nonce is
//! the extended source address and the frame counter (both most significant octet first) and the security level. Levels 1
//! to 3 authenticate the header and the whole MAC payload; levels 4 to 7 encrypt the private part of the payload (all of a
//! data payload; a beacon after its superframe, GTS and pending-address fields; a command after its identifier). Levels 1
//! and 5 add a 4-octet MIC, 2 and 6 an 8-octet MIC, 3 and 7 a 16-octet MIC; level 4 adds none.
//! Standard: IEEE 802.15.4-2006 clause 7.6 (CCM* and the nonce) and 7.6.2 (the auxiliary security header).

use crate::mac154::{self, Mac};
use sdrkit_algo_crypto::{Aes, AesKey, Ccm, Counter, Key};

/// The MIC length of security level `level`, octets.
pub fn mic_len(level: u8) -> usize {
    [0, 4, 8, 16][usize::from(level & 3)]
}

/// The CCM* nonce of a frame from `src` with frame counter `counter` at security level `level`.
pub fn nonce(src: u64, counter: u32, level: u8) -> [u8; 13] {
    let mut n = [0u8; 13];
    n[..8].copy_from_slice(&src.to_be_bytes());
    n[8..12].copy_from_slice(&counter.to_be_bytes());
    n[12] = level;
    n
}

/// The auxiliary security header: the security control octet (level, key identifier mode), the frame counter (low octet
/// first) and the key identifier (`source`, 0, 4 or 8 octets by the mode, then the key index for modes 1 to 3).
pub fn aux_header(level: u8, key_id_mode: u8, counter: u32, source: &[u8], key_index: u8) -> Vec<u8> {
    let mut v = vec![(level & 7) | (key_id_mode & 3) << 3];
    v.extend(counter.to_le_bytes());
    if key_id_mode != 0 {
        v.extend_from_slice(source);
        v.push(key_index);
    }
    v
}

/// The length of the open part of the MAC payload `pay` of frame type `ftype` at a level that encrypts: the superframe,
/// GTS and pending-address fields of a beacon, the identifier of a MAC command, nothing for other frames.
pub fn open_len(ftype: u8, pay: &[u8]) -> usize {
    match ftype {
        0 => mac154::beacon(pay).payload.min(pay.len()),
        3 => pay.len().min(1),
        _ => 0,
    }
}

/// The 16-octet key of hex text (spaces, ':' and '-' ignored), or `None`.
pub fn key_of(text: &str) -> Option<AesKey> {
    let t: String = text
        .chars()
        .filter(|c| !c.is_whitespace() && *c != ':' && *c != '-')
        .collect();
    let b: [u8; 16] = crate::family::from_hex(&t)?.try_into().ok()?;
    Some(AesKey::Aes128(Key::new(b)))
}

/// The keys of the text `text`: keys of 32 hex digits separated by ';' or ','. `None` when one is not a key.
pub fn keys_of(text: &str) -> Option<Vec<AesKey>> {
    text.split([';', ','])
        .filter(|k| !k.trim().is_empty())
        .map(key_of)
        .collect()
}

/// The counter block A1 of CCM* (flags L − 1 = 1, the nonce, counter 1).
fn first_counter(n: &[u8; 13]) -> [u8; 16] {
    let mut a = [0u8; 16];
    a[0] = 1;
    a[1..14].copy_from_slice(n);
    a[15] = 1;
    a
}

/// The CCM* MIC of `level` over `aad` and `m` (before encryption), encrypting `m` in place for levels 4 to 7.
fn seal(key: &AesKey, n: &[u8; 13], level: u8, aad: &[u8], m: &mut [u8]) -> Option<Vec<u8>> {
    let enc = level & 4 != 0;
    let mut none: [u8; 0] = [];
    let (aad, buf): (Vec<u8>, &mut [u8]) = if enc {
        (aad.to_vec(), m)
    } else {
        ([aad, &*m].concat(), &mut none)
    };
    let tag = match mic_len(level) {
        0 => {
            Aes::new(key)
                .apply_ctr(&first_counter(n), Counter::Low32, buf)
                .ok()?;
            Vec::new()
        }
        4 => Ccm::<4, 13>::new(key).encrypt_in_place(n, &aad, buf).ok()?.to_vec(),
        8 => Ccm::<8, 13>::new(key).encrypt_in_place(n, &aad, buf).ok()?.to_vec(),
        _ => Ccm::<16, 13>::new(key).encrypt_in_place(n, &aad, buf).ok()?.to_vec(),
    };
    Some(tag)
}

/// Checks the MIC `tag` of `level` over `aad` and `c` and returns the plain private part (`c` decrypted for levels 4 to 7).
fn unseal(key: &AesKey, n: &[u8; 13], level: u8, aad: &[u8], c: &[u8], tag: &[u8]) -> Option<Vec<u8>> {
    let enc = level & 4 != 0;
    let mut out = vec![0u8; if enc { c.len() } else { 0 }];
    let aad = if enc { aad.to_vec() } else { [aad, c].concat() };
    let cin: &[u8] = if enc { c } else { &[] };
    match mic_len(level) {
        0 => {
            out.copy_from_slice(c);
            Aes::new(key)
                .apply_ctr(&first_counter(n), Counter::Low32, &mut out)
                .ok()?;
        }
        4 => Ccm::<4, 13>::new(key).decrypt_to(n, &aad, cin, tag.try_into().ok()?, &mut out).ok()?,
        8 => Ccm::<8, 13>::new(key).decrypt_to(n, &aad, cin, tag.try_into().ok()?, &mut out).ok()?,
        _ => Ccm::<16, 13>::new(key).decrypt_to(n, &aad, cin, tag.try_into().ok()?, &mut out).ok()?,
    }
    Some(if enc { out } else { c.to_vec() })
}

/// Secures the frame `hdr` (the MAC header with its security bit set and its auxiliary security header) and `pay` (its MAC
/// payload) from the extended source `src`: the MAC payload, encrypted where the level encrypts, and the MIC.
pub fn protect(key: &AesKey, src: u64, counter: u32, level: u8, ftype: u8, hdr: &[u8], pay: &[u8]) -> Option<Vec<u8>> {
    let n = nonce(src, counter, level);
    let open = if level & 4 != 0 { open_len(ftype, pay) } else { pay.len() };
    let aad = [hdr, &pay[..open]].concat();
    let mut private = pay[open..].to_vec();
    let tag = seal(key, &n, level, &aad, &mut private)?;
    let mut out = pay[..open].to_vec();
    out.extend_from_slice(&private);
    out.extend(tag);
    Some(out)
}

/// The plain MAC payload (without its MIC) of the secured frame `b` (a PSDU without its FCS, parsed as `mac`) from the
/// extended source `src`, and the number of the key in `keys` that opens it; `None` when no key passes the MIC (level 4,
/// which has no MIC, opens with the first key).
pub fn open(b: &[u8], mac: &Mac, src: u64, keys: &[AesKey]) -> Option<(Vec<u8>, usize)> {
    let aux = mac.aux.as_ref()?;
    let counter = aux.counter?;
    let hdr = b.get(..mac.hdr_len)?;
    let pay = b.get(mac.hdr_len..b.len().checked_sub(aux.mic_len)?)?;
    let tag = &b[b.len() - aux.mic_len..];
    let n = nonce(src, counter, aux.level);
    let open = if aux.level & 4 != 0 { open_len(mac.ftype, pay) } else { pay.len() };
    let aad = [hdr, &pay[..open]].concat();
    keys.iter().enumerate().find_map(|(i, k)| {
        let private = unseal(k, &n, aux.level, &aad, &pay[open..], tag)?;
        let mut plain = pay[..open].to_vec();
        plain.extend(private);
        Some((plain, i))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key() -> AesKey {
        key_of("C0C1C2C3C4C5C6C7C8C9CACBCCCDCECF").unwrap()
    }

    /// RFC 3610 packet vector 1: AES-128 key C0..CF, nonce 00 00 00 03 02 01 00 A0..A5, 8 octets of header and 23 of
    /// message, an 8-octet MIC.
    const NONCE_1: [u8; 13] = [0, 0, 0, 3, 2, 1, 0, 0xA0, 0xA1, 0xA2, 0xA3, 0xA4, 0xA5];
    const OUT_1: [u8; 31] = [
        0x58, 0x8C, 0x97, 0x9A, 0x61, 0xC6, 0x63, 0xD2, 0xF0, 0x66, 0xD0, 0xC2, 0xC0, 0xF9, 0x89, 0x80, 0x6D, 0x5F, 0x6B,
        0x61, 0xDA, 0xC3, 0x84, 0x17, 0xE8, 0xD1, 0x2C, 0xFD, 0xF9, 0x26, 0xE0,
    ];

    #[test]
    fn ccm_star_follows_rfc_3610_packet_vector_1() {
        let a: Vec<u8> = (0..8).collect();
        let mut m: Vec<u8> = (8..31).collect();
        // Level 6: encryption and an 8-octet MIC, as the vector.
        let tag = seal(&key(), &NONCE_1, 6, &a, &mut m).unwrap();
        assert_eq!([m.as_slice(), &tag].concat(), OUT_1);
        let plain = unseal(&key(), &NONCE_1, 6, &a, &m, &tag).unwrap();
        assert_eq!(plain, (8..31).collect::<Vec<u8>>());
        // Level 4: the same keystream from counter 1, no MIC.
        let mut m4: Vec<u8> = (8..31).collect();
        assert!(seal(&key(), &NONCE_1, 4, &a, &mut m4).unwrap().is_empty());
        assert_eq!(m4, OUT_1[..23]);
        assert_eq!(unseal(&key(), &NONCE_1, 4, &a, &m4, &[]).unwrap(), (8..31).collect::<Vec<u8>>());
    }

    #[test]
    fn a_wrong_mic_does_not_open() {
        let a: Vec<u8> = (0..8).collect();
        let mut m: Vec<u8> = (8..31).collect();
        let mut tag = seal(&key(), &NONCE_1, 5, &a, &mut m).unwrap();
        tag[0] ^= 1;
        assert!(unseal(&key(), &NONCE_1, 5, &a, &m, &tag).is_none());
        let mut plain: Vec<u8> = (8..31).collect();
        let t2 = seal(&key(), &NONCE_1, 2, &a, &mut plain).unwrap();
        assert_eq!(plain, (8..31).collect::<Vec<u8>>(), "level 2 does not encrypt");
        assert!(unseal(&key(), &NONCE_1, 2, &a, &plain, &t2).is_some());
        assert!(unseal(&key(), &NONCE_1, 1, &a, &plain, &t2[..4]).is_none());
    }

    #[test]
    fn the_nonce_is_the_address_the_counter_and_the_level() {
        assert_eq!(
            nonce(0xACDE_4800_0000_0001, 5, 2),
            [0xAC, 0xDE, 0x48, 0, 0, 0, 0, 1, 0, 0, 0, 5, 2]
        );
        assert_eq!(aux_header(5, 1, 0x0102_0304, &[], 7), [0x0D, 4, 3, 2, 1, 7]);
        assert_eq!(keys_of("00112233445566778899AABBCCDDEEFF; c0c1c2c3c4c5c6c7c8c9cacbcccdcecf").map(|k| k.len()), Some(2));
        assert!(keys_of("0011").is_none());
    }
}
