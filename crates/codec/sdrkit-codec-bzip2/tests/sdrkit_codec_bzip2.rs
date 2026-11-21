//! The decoder on streams made by the bzip2 program, the encoder in round trips, joined streams, damaged streams and the
//! output limit.

use sdrkit_codec_bzip2::{compress, decompress, Error};

/// The input of `tests/data/sample.bin.bz2` (made with `bzip2 -9`).
fn sample() -> Vec<u8> {
    (0..40_000usize)
        .map(|i| {
            if (i / 1000) % 3 != 0 {
                ((i * 7 / 13 + i / 500) % 251) as u8
            } else {
                ((i / 300) % 256) as u8
            }
        })
        .collect()
}

/// The input of `tests/data/multi.bin.bz2` (made with `bzip2 -1`: two blocks).
fn multi() -> Vec<u8> {
    let mut x: u64 = 1;
    (0..120_000usize)
        .map(|i| {
            x = (x * 1_103_515_245 + 12_345) & 0x7FFF_FFFF;
            if i % 5000 < 4500 {
                65 + ((x >> 16) % 2) as u8
            } else {
                0x20
            }
        })
        .collect()
}

#[test]
fn streams_of_the_bzip2_program_decode() {
    let one = include_bytes!("data/sample.bin.bz2");
    assert_eq!(decompress(one, 1 << 20).unwrap(), sample());
    let two = include_bytes!("data/multi.bin.bz2");
    assert_eq!(decompress(two, 1 << 20).unwrap(), multi());
}

#[test]
fn round_trips_at_many_lengths() {
    let mut x: u32 = 7;
    for len in [0usize, 1, 2, 3, 4, 5, 8, 255, 256, 257, 300, 1000, 4099, 33_333] {
        for kind in 0..4 {
            let data: Vec<u8> = (0..len)
                .map(|i| {
                    x = x.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
                    match kind {
                        0 => (x >> 24) as u8,
                        1 => b'a',
                        2 => (i / 37 % 3) as u8,
                        _ => (i % 251) as u8 ^ ((x >> 30) as u8),
                    }
                })
                .collect();
            let z = compress(&data, 1);
            let back = decompress(&z, 1 << 24);
            if len == 0 {
                assert_eq!(back.unwrap(), data);
            } else {
                assert_eq!(back.unwrap(), data, "length {len}, kind {kind}");
            }
        }
    }
}

#[test]
fn several_blocks_round_trip() {
    let data: Vec<u8> = (0..250_000u32).map(|i| (i.wrapping_mul(2_654_435_761) >> 27) as u8).collect();
    let z = compress(&data, 1);
    assert_eq!(decompress(&z, 1 << 20).unwrap(), data);
}

#[test]
fn joined_streams_with_junk_between_give_both() {
    let a = b"first stream\n".repeat(50);
    let b: Vec<u8> = (0..3000u32).map(|i| (i % 97) as u8).collect();
    let mut buf = vec![0x55, 0xAA, 0x00];
    buf.extend(compress(&a, 9));
    buf.extend_from_slice(b"junk between");
    buf.extend(compress(&b, 5));
    buf.extend_from_slice(&[0, 1, 2]);
    let mut want = a.clone();
    want.extend_from_slice(&b);
    assert_eq!(decompress(&buf, 1 << 20).unwrap(), want);
}

#[test]
fn damaged_streams_fail_and_never_crash() {
    let data = sample();
    let z = include_bytes!("data/sample.bin.bz2").to_vec();
    assert_eq!(decompress(b"no stream here", 100), Err(Error::NotBzip2));
    assert_eq!(decompress(&z[..z.len() / 2], 1 << 20), Err(Error::Truncated));
    // A flipped bit never panics, and almost always fails: a flip in a code length of a table that no selector names, or in
    // the padding after the end marker, changes nothing.
    let mut ok = 0;
    let mut tried = 0;
    for k in (4..z.len()).step_by(7) {
        for bit in [0u8, 3, 7] {
            let mut d = z.clone();
            d[k] ^= 1 << bit;
            tried += 1;
            if decompress(&d, 1 << 20).is_ok_and(|out| out == data) {
                ok += 1;
            }
        }
    }
    assert!(ok * 20 <= tried, "{ok} of {tried} damaged streams decoded");
    // A block CRC that does not match.
    let mut d = z.clone();
    d[10] ^= 0x01;
    assert!(decompress(&d, 1 << 20).is_err());
}

#[test]
fn the_output_limit_holds() {
    let data = vec![7u8; 10_000];
    let z = compress(&data, 9);
    assert_eq!(decompress(&z, 100), Err(Error::TooLong));
    assert_eq!(decompress(&z, 10_000).unwrap().len(), 10_000);
}
