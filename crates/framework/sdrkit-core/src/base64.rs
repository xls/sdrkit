//! Base64 (RFC 4648 sections 4 and 5) and hexadecimal text, without allocation beyond the caller's buffers.
//!
//! ```
//! use sdrkit_core::base64::{self, Alphabet};
//!
//! let mut s = String::new();
//! base64::encode_into(b"foobar", Alphabet::Standard, true, &mut s);
//! assert_eq!(s, "Zm9vYmFy");
//! let mut v = Vec::new();
//! base64::decode_into("Zm9vYmE=", Alphabet::Standard, &mut v).unwrap();
//! assert_eq!(v, b"fooba");
//! ```

/// The text is not valid base64 or hexadecimal.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Invalid;

impl std::fmt::Display for Invalid {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("not valid base64 or hexadecimal text")
    }
}

impl std::error::Error for Invalid {}

type Result<T> = std::result::Result<T, Invalid>;

/// Which 64-character alphabet.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Alphabet {
    /// RFC 4648 section 4: `+` and `/`.
    #[default]
    Standard,
    /// RFC 4648 section 5 ("base64url"): `-` and `_`.
    UrlSafe,
}

const STD: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
const URL: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";

const fn reverse(a: &[u8; 64]) -> [u8; 256] {
    let mut t = [0xFFu8; 256];
    let mut i = 0;
    while i < 64 {
        t[a[i] as usize] = i as u8;
        i += 1;
    }
    t
}

const STD_REV: [u8; 256] = reverse(STD);
const URL_REV: [u8; 256] = reverse(URL);

impl Alphabet {
    fn chars(self) -> &'static [u8; 64] {
        match self {
            Alphabet::Standard => STD,
            Alphabet::UrlSafe => URL,
        }
    }
    fn rev(self) -> &'static [u8; 256] {
        match self {
            Alphabet::Standard => &STD_REV,
            Alphabet::UrlSafe => &URL_REV,
        }
    }
}

/// Appends the base64 text of `data`, with `=` padding when `pad` is set.
pub fn encode_into(data: &[u8], alphabet: Alphabet, pad: bool, out: &mut String) {
    let a = alphabet.chars();
    for ch in data.chunks(3) {
        let v = (ch[0] as u32) << 16
            | (*ch.get(1).unwrap_or(&0) as u32) << 8
            | *ch.get(2).unwrap_or(&0) as u32;
        let n = ch.len() + 1;
        for i in 0..4 {
            if i < n {
                out.push(a[((v >> (18 - 6 * i)) & 63) as usize] as char);
            } else if pad {
                out.push('=');
            }
        }
    }
}

/// Appends the bytes of base64 text to `out`. Padding is optional; whitespace and characters outside the alphabet are errors, as
/// are non-zero bits left over at the end.
pub fn decode_into(text: &str, alphabet: Alphabet, out: &mut Vec<u8>) -> Result<()> {
    let rev = alphabet.rev();
    let body = text.trim_end_matches('=');
    if text.len() - body.len() > 2 || (text.len() != body.len() && !text.len().is_multiple_of(4)) {
        return Err(Invalid);
    }
    let start = out.len();
    let mut acc = 0u32;
    let mut bits = 0;
    for &c in body.as_bytes() {
        let v = rev[c as usize];
        if v == 0xFF {
            out.truncate(start);
            return Err(Invalid);
        }
        acc = acc << 6 | v as u32;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((acc >> bits) as u8);
        }
    }
    if bits >= 6 || acc & ((1 << bits) - 1) != 0 {
        out.truncate(start);
        return Err(Invalid);
    }
    Ok(())
}

/// Appends upper-case hexadecimal text of `data`.
pub fn hex_into(data: &[u8], out: &mut String) {
    const H: &[u8; 16] = b"0123456789ABCDEF";
    for &b in data {
        out.push(H[(b >> 4) as usize] as char);
        out.push(H[(b & 15) as usize] as char);
    }
}

/// Appends the bytes of hexadecimal text (either case, an even number of digits).
pub fn unhex_into(text: &str, out: &mut Vec<u8>) -> Result<()> {
    let t = text.as_bytes();
    if !t.len().is_multiple_of(2) {
        return Err(Invalid);
    }
    let start = out.len();
    for p in t.as_chunks::<2>().0 {
        let d = |c: u8| (c as char).to_digit(16);
        match (d(p[0]), d(p[1])) {
            (Some(a), Some(b)) => out.push((a * 16 + b) as u8),
            _ => {
                out.truncate(start);
                return Err(Invalid);
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rfc4648_test_vectors() {
        // RFC 4648 section 10.
        for (raw, enc) in [
            ("", ""),
            ("f", "Zg=="),
            ("fo", "Zm8="),
            ("foo", "Zm9v"),
            ("foob", "Zm9vYg=="),
            ("fooba", "Zm9vYmE="),
            ("foobar", "Zm9vYmFy"),
        ] {
            let mut s = String::new();
            encode_into(raw.as_bytes(), Alphabet::Standard, true, &mut s);
            assert_eq!(s, enc);
            let mut v = vec![];
            decode_into(enc, Alphabet::Standard, &mut v).unwrap();
            assert_eq!(v, raw.as_bytes());
            let mut v = vec![];
            decode_into(enc.trim_end_matches('='), Alphabet::Standard, &mut v).unwrap();
            assert_eq!(v, raw.as_bytes());
        }
        let mut s = String::new();
        encode_into(&[0xFB, 0xFF], Alphabet::UrlSafe, false, &mut s);
        assert_eq!(s, "-_8");
        let mut v = vec![];
        assert_eq!(
            decode_into("Zm9v!", Alphabet::Standard, &mut v),
            Err(Invalid)
        );
        assert_eq!(
            decode_into("Zh==", Alphabet::Standard, &mut v),
            Err(Invalid)
        );
        assert_eq!(decode_into("Z", Alphabet::Standard, &mut v), Err(Invalid));
        assert!(v.is_empty());
    }

    #[test]
    fn hex() {
        let mut s = String::new();
        hex_into(&[0x00, 0xAB, 0x7F], &mut s);
        assert_eq!(s, "00AB7F");
        let mut v = vec![];
        unhex_into("00ab7F", &mut v).unwrap();
        assert_eq!(v, [0x00, 0xAB, 0x7F]);
        assert_eq!(unhex_into("0", &mut v), Err(Invalid));
        assert_eq!(unhex_into("0g", &mut v), Err(Invalid));
    }
}
