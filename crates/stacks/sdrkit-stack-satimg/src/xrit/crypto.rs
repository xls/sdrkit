//! The KMA key files of the GK-2A file layer, and DES in ECB mode on whole 8-byte parts through `sdrkit-algo-crypto`.
//! Standard: FIPS 46-3

use sdrkit_algo_crypto::Des;
use std::collections::BTreeMap;

/// Decrypts the whole 8-byte parts of `data` in place with `key`. A last part shorter than 8 bytes stays as it is.
pub fn des_ecb_decrypt(key: &[u8; 8], data: &mut [u8]) {
    let n = data.len() / 8 * 8;
    let _ = Des::new(key).decrypt_ecb(&mut data[..n]);
}

/// Encrypts the whole 8-byte parts of `data` in place with `key` (for the generator and tests).
pub fn des_ecb_encrypt(key: &[u8; 8], data: &mut [u8]) {
    let n = data.len() / 8 * 8;
    let _ = Des::new(key).encrypt_ecb(&mut data[..n]);
}

/// A line of a key text that is not a key.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NotAKey {
    /// The byte position of the line in the text.
    pub at: usize,
}

/// Reads the keys of a text value: lines "index:hexkey" (the index in hexadecimal, 1 to FFFF, then exactly 16 hexadecimal
/// digits; the separator can be ":", "=", a space or a tab). Empty lines are skipped.
///
/// # Errors
///
/// [`NotAKey`] with the position of the first line that is not a key.
pub fn parse_key_text(text: &str) -> Result<BTreeMap<u16, [u8; 8]>, NotAKey> {
    let mut keys = BTreeMap::new();
    let mut at = 0;
    for raw in text.split_inclusive('\n') {
        let line = raw.trim();
        if !line.is_empty() {
            let one = parse_key_file(line.as_bytes());
            if one.len() != 1 {
                return Err(NotAKey { at });
            }
            keys.extend(one);
        }
        at += raw.len();
    }
    Ok(keys)
}

/// Reads a key file: binary (a 16-bit count, then for each key a 16-bit index and 8 key bytes) or text lines "index:hexkey" (the
/// index in hexadecimal; separators ":", "=", space or tab; exactly 16 hex digits; index 1 to FFFF). Junk returns no keys.
pub fn parse_key_file(bytes: &[u8]) -> BTreeMap<u16, [u8; 8]> {
    let mut keys = BTreeMap::new();
    if bytes.len() >= 2 {
        let n = u16::from_be_bytes([bytes[0], bytes[1]]) as usize;
        if n > 0 && bytes.len() == 2 + n * 10 {
            for k in 0..n {
                let r = &bytes[2 + k * 10..2 + (k + 1) * 10];
                let idx = u16::from_be_bytes([r[0], r[1]]);
                if idx == 0 {
                    continue;
                }
                let mut key = [0u8; 8];
                key.copy_from_slice(&r[2..]);
                keys.insert(idx, key);
            }
            if !keys.is_empty() {
                return keys;
            }
        }
    }
    let Ok(text) = std::str::from_utf8(bytes) else {
        return keys;
    };
    for line in text.lines() {
        let line = line.trim();
        let Some(split) = line.find([':', '=', ' ', '\t']) else {
            continue;
        };
        let (idx, rest) = line.split_at(split);
        let hex = rest[1..].trim_matches(|c: char| c == ':' || c == '=' || c.is_whitespace());
        let Ok(idx) = u16::from_str_radix(idx.trim(), 16) else {
            continue;
        };
        if idx == 0 || hex.len() != 16 || !hex.chars().all(|c| c.is_ascii_hexdigit()) {
            continue;
        }
        let mut key = [0u8; 8];
        for (i, k) in key.iter_mut().enumerate() {
            *k = u8::from_str_radix(&hex[2 * i..2 * i + 2], 16).unwrap_or(0);
        }
        keys.insert(idx, key);
    }
    keys
}

/// The DES key the GK-2A test signal uses for key number 1 (a test value for this receiver's own generator, not a KMA key).
pub const TEST_KEY: [u8; 8] = [0x13, 0x34, 0x57, 0x79, 0x9B, 0xBC, 0xDF, 0xF1];

/// A binary key file holding [`TEST_KEY`] as key 1.
pub fn test_key_file() -> Vec<u8> {
    let mut f = vec![0, 1, 0, 1];
    f.extend_from_slice(&TEST_KEY);
    f
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn des_matches_the_fips_example() {
        let key = [0x13, 0x34, 0x57, 0x79, 0x9B, 0xBC, 0xDF, 0xF1];
        let mut d = [
            0x01, 0x23, 0x45, 0x67, 0x89, 0xAB, 0xCD, 0xEF, 1, 2, 3, 4, 5,
        ];
        des_ecb_encrypt(&key, &mut d);
        assert_eq!(&d[..8], &[0x85, 0xE8, 0x13, 0x54, 0x0F, 0x0A, 0xB4, 0x05]);
        assert_eq!(&d[8..], &[1, 2, 3, 4, 5]);
        des_ecb_decrypt(&key, &mut d);
        assert_eq!(&d[..8], &[0x01, 0x23, 0x45, 0x67, 0x89, 0xAB, 0xCD, 0xEF]);
    }

    #[test]
    fn key_files_read_in_both_forms() {
        let k = parse_key_file(&test_key_file());
        assert_eq!(k.get(&1), Some(&TEST_KEY));
        let t = parse_key_file(b"1:133457799BBCDFF1\n");
        assert_eq!(t.get(&1), Some(&TEST_KEY));
        let t = parse_key_file(b"0001 = 133457799bbcdff1\r\n");
        assert_eq!(t.get(&1), Some(&TEST_KEY));
        assert!(parse_key_file(b"\x00\x07garbage that is not a key file").is_empty());
        assert!(parse_key_file(b"hello world\n1:1234").is_empty());
    }
}
