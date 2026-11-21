//! gzip (RFC 1952): members made by the gzip program and the limits of the reader.

use sdrkit_proto_ipcast::gzip::{compress, decompress};
use sdrkit_proto_ipcast::Error;
use sdrkit_proto_ipcast::{Check, Limit};

/// `printf hello | gzip -n`: header (no name, no time, OS 3), DEFLATE, CRC-32 0x3610A686, ISIZE 5.
const HELLO: [u8; 25] = [
    0x1F, 0x8B, 0x08, 0x00, 0, 0, 0, 0, 0x00, 0x03, 0xCB, 0x48, 0xCD, 0xC9, 0xC9, 0x07, 0x00, 0x86,
    0xA6, 0x10, 0x36, 0x05, 0, 0, 0,
];

/// `printf '' | gzip -n`: an empty member (DEFLATE 03 00, CRC-32 0, ISIZE 0).
const EMPTY: [u8; 20] = [
    0x1F, 0x8B, 0x08, 0x00, 0, 0, 0, 0, 0x00, 0x03, 0x03, 0x00, 0, 0, 0, 0, 0, 0, 0, 0,
];

#[test]
fn members_from_the_gzip_program() {
    assert_eq!(decompress(&HELLO, 100).unwrap(), b"hello");
    assert_eq!(decompress(&EMPTY, 100).unwrap(), b"");
}

#[test]
fn bad_members_are_refused() {
    let mut b = HELLO;
    b[17] ^= 1;
    assert_eq!(decompress(&b, 100), Err(Error::Check(Check::GzipCrc32)));
    let mut b = HELLO;
    b[21] = 6;
    assert_eq!(decompress(&b, 100), Err(Error::Check(Check::GzipIsize)));
    assert_eq!(decompress(&HELLO[..10], 100), Err(Error::Short));
    assert_eq!(decompress(&HELLO, 4), Err(Error::TooLarge(Limit::GzipData)));
    // A small member that grows to 1 MB is stopped at the limit.
    let mut z = Vec::new();
    compress(&vec![0u8; 1 << 20], 9, &mut z);
    assert!(z.len() < 5000);
    assert_eq!(
        decompress(&z, 1 << 16),
        Err(Error::TooLarge(Limit::GzipData))
    );
    assert_eq!(decompress(&z, 1 << 20).unwrap().len(), 1 << 20);
}

#[test]
fn optional_header_fields() {
    // FNAME and FCOMMENT (RFC 1952 clause 2.3.1) before the data.
    let mut b = vec![0x1F, 0x8B, 0x08, 0x18, 0, 0, 0, 0, 0, 0x03];
    b.extend_from_slice(b"a.txt\0note\0");
    b.extend_from_slice(&HELLO[10..]);
    assert_eq!(decompress(&b, 100).unwrap(), b"hello");
}
