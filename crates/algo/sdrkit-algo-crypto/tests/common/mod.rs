//! Hex helpers for the test vectors (blanks inside the hex are ignored, as the standards print them in groups).

#![allow(dead_code)]

/// Bytes from hex.
pub fn h(s: &str) -> Vec<u8> {
    let d: Vec<u8> = s.bytes().filter(|c| !c.is_ascii_whitespace()).collect();
    assert!(d.len().is_multiple_of(2), "odd hex length in a test vector");
    d.chunks(2)
        .map(|p| u8::from_str_radix(std::str::from_utf8(p).unwrap(), 16).unwrap())
        .collect()
}

/// A fixed-size array from hex.
pub fn a<const N: usize>(s: &str) -> [u8; N] {
    h(s).try_into().expect("test vector length")
}
