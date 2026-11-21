//! Bit rows: reading, searching and the line decoders.

use sdrkit_proto_ookdev::{BitRows, Error};
use sdrkit_util_bits::line::ManchesterConv;

#[test]
fn rows_bits_search_and_extract() {
    let b = BitRows::parse("0000000101001010 / 1").unwrap();
    assert_eq!(b.num_rows(), 2);
    assert_eq!((b.len(0), b.len(1)), (16, 1));
    assert_eq!(b.search(0, 0, &[0x01, 0x40], 12), Some(0));
    assert_eq!(b.search(0, 1, &[0x01, 0x40], 12), None);
    assert_eq!(b.search(0, 0, &[0xFF], 8), None);
    let mut out = [0u8; 1];
    b.extract(0, 4, 8, &mut out).unwrap();
    assert_eq!(out, [0x14]);
    assert_eq!(b.bits_u64(0, 8, 8), 0x4A);
    // Past the end reads as 0.
    assert_eq!(b.bit(1, 5), 0);
    assert_eq!(b.row(1).byte(9), 0);
    assert_eq!(
        b.extract(0, 0, 9, &mut out),
        Err(Error::OutputTooShort { need: 2 })
    );
}

#[test]
fn invert_keeps_padding_clear_and_reflect_reverses_bytes() {
    let mut b = BitRows::parse("101").unwrap();
    b.invert();
    assert_eq!(b.row(0).bytes, &[0b0100_0000]);
    let mut c = BitRows::from_bytes(&[0x01, 0x80], 16).unwrap();
    c.reflect();
    assert_eq!(c.row(0).bytes, &[0x80, 0x01]);
}

#[test]
fn repeated_rows() {
    let b = BitRows::parse("1010 / 1111 / 1010 / 1010 / 11").unwrap();
    assert_eq!(b.repeated_row(3, 4), Some(0));
    assert_eq!(b.repeated_row(4, 4), None);
    assert_eq!(b.repeated_row(1, 3), Some(0));
}

#[test]
fn capacity_is_counted_not_grown() {
    let mut b = BitRows::new(2, 8);
    for _ in 0..10 {
        b.push(1);
    }
    b.new_row();
    b.new_row();
    assert_eq!(b.len(0), 8);
    assert_eq!(b.num_rows(), 2);
    assert_eq!(b.overflow(), 3);
    let long = "1".repeat(5000);
    assert_eq!(BitRows::parse(&long), Err(Error::Capacity));
}

/// IEEE 802.3 clause 7.3.1.1: Manchester sends the complement of the bit in the first half of the cell and the bit in the second
/// half, so 1 is chips 01 and 0 is chips 10. The decode stops at the first pair without a change.
#[test]
fn manchester_ieee_known_answer() {
    let b = BitRows::parse("01 10 10 01 01 00 01").unwrap();
    let mut out = BitRows::default();
    let n = b.decode_manchester(0, 0, 100, ManchesterConv::Ieee, &mut out);
    assert_eq!(n, 5);
    assert_eq!(out.bits_u64(0, 0, 5), 0b10011);
    // The other convention (G. E. Thomas) reads the same chips as the complement.
    let n = b.decode_manchester(0, 0, 100, ManchesterConv::Thomas, &mut out);
    assert_eq!((n, out.bits_u64(0, 0, 5)), (5, 0b01100));
    // `max` limits the output.
    assert_eq!(
        b.decode_manchester(0, 0, 3, ManchesterConv::Ieee, &mut out),
        3
    );
    // A start position.
    assert_eq!(
        b.decode_manchester(0, 2, 100, ManchesterConv::Ieee, &mut out),
        4
    );
    assert_eq!(out.bits_u64(0, 0, 4), 0b0011);
}

/// ISO/IEC 18000-63 (EPC UHF Gen2), tag-to-interrogator FM0 (biphase space): the level changes at every cell boundary, and a 0
/// also changes in the middle of its cell. From the line at level 1, the bits 1 0 1 1 0 0 are the chips 00 10 11 00 10 10.
#[test]
fn fm0_known_answer() {
    let b = BitRows::parse("1 00 10 11 00 10 10").unwrap();
    let mut out = BitRows::default();
    let n = b.decode_fm0(0, 0, 100, &mut out);
    assert_eq!(n, 6);
    assert_eq!(out.bits_u64(0, 0, 6), 0b101100);
}

#[test]
fn fm0_finds_the_clock_phase_and_stops_at_a_missing_boundary() {
    // Two chips of noise at the start shift the phase by one. The cell after the data has no boundary change.
    let b = BitRows::parse("0 1 00 10 11 00 10 10 00").unwrap();
    let mut out = BitRows::default();
    let n = b.decode_fm0(0, 0, 100, &mut out);
    assert_eq!(n, 6);
    assert_eq!(out.bits_u64(0, 0, 6), 0b101100);
    assert_eq!(b.decode_fm0(0, 0, 4, &mut out), 4);
    // A row of one bit has no cell.
    assert_eq!(
        BitRows::parse("1").unwrap().decode_fm0(0, 0, 9, &mut out),
        0
    );
}

#[test]
fn padded_copy() {
    let b = BitRows::from_bytes(&[0xAB, 0xCD], 12).unwrap();
    let mut buf = [0xFFu8; 4];
    b.row(0).copy_padded(&mut buf);
    assert_eq!(buf, [0xAB, 0xC0, 0, 0]);
    assert_eq!(BitRows::from_bytes(&[0], 9), Err(Error::InputTooShort));
}
