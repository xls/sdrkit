//! BBHEADER: EN 302 307-1 clause 5.1.6 (tables 3 and 4), EN 302 755 clause 5.1.7 (figure 4), the CRC-8 of clause 5.1.4.

use sdrkit_proto_ipcast::bbheader::{bb_descramble, BbHeader, Mode, StreamFormat, System};
use sdrkit_proto_ipcast::Check;
use sdrkit_proto_ipcast::Error;
use sdrkit_util_bits::crc::catalog::CRC8_DVB_S2;

/// The CRC-8 encoder of EN 302 307-1 figure 2 as a plain shift register: g(X) = X^8 + X^7 + X^6 + X^4 + X^2 + 1, register zero.
fn crc8_register(data: &[u8]) -> u8 {
    let mut r = 0u8;
    for &byte in data {
        for i in (0..8).rev() {
            let fb = ((r >> 7) ^ (byte >> i)) & 1;
            r <<= 1;
            if fb == 1 {
                r ^= 0xD5;
            }
        }
    }
    r
}

#[test]
fn crc8_check_value_and_register() {
    // The CRC-8/DVB-S2 of "123456789" in the CRC catalogue: 0xBC.
    assert_eq!(CRC8_DVB_S2.checksum(b"123456789"), 0xBC);
    assert_eq!(crc8_register(b"123456789"), 0xBC);
}

#[test]
fn table_4_single_transport_stream() {
    // EN 302 307-1 table 4: TS/GS = 11, SIS, CCM, ISSYI 0, NPD 0, MATYPE-2 0, UPL 188 x 8, SYNC 0x47.
    let h = BbHeader::transport(32_128, 0x1234);
    let mut b = [0u8; 10];
    h.write(&mut b);
    assert_eq!(b[0], 0b1111_0000);
    assert_eq!(b[1], 0);
    assert_eq!(&b[2..4], &1504u16.to_be_bytes());
    assert_eq!(&b[4..6], &32_128u16.to_be_bytes());
    assert_eq!(b[6], 0x47);
    assert_eq!(&b[7..9], &[0x12, 0x34]);
    assert_eq!(b[9], crc8_register(&b[..9]));
    assert_eq!(BbHeader::parse(&b).unwrap(), h);
}

#[test]
fn matype_bits_and_rolloff() {
    // MATYPE-1 bit order (table 3): TS/GS (2), SIS/MIS, CCM/ACM, ISSYI, NPD, RO (2).
    let mut b = [0x79u8, 0x05, 0, 0, 0x01, 0x00, 0, 0, 0, 0];
    b[9] = crc8_register(&b[..9]);
    let h = BbHeader::parse(&b).unwrap();
    assert_eq!(h.format, StreamFormat::GenericContinuous);
    assert!(h.single_stream && h.ccm && h.issyi && !h.npd);
    assert_eq!(h.ext, 1);
    assert_eq!(h.stream_id, 5);
    assert_eq!(h.rolloff(System::DvbS2), Some(0.25));
    assert_eq!(h.rolloff(System::DvbT2), None, "EXT is reserved in DVB-T2");
    assert!(h.carries_gse());
}

#[test]
fn high_efficiency_mode_from_the_crc_byte() {
    // EN 302 755 figure 4(b): CRC-8 XOR MODE = 1, ISSY in the UPL bytes (2 MSB) and the SYNC byte (LSB).
    let mut b = [0xF8u8, 0x00, 0xAB, 0xCD, 0x96, 0xD0, 0xEF, 0x00, 0x00, 0];
    b[9] = crc8_register(&b[..9]) ^ 1;
    let h = BbHeader::parse(&b).unwrap();
    assert_eq!(h.mode, Mode::HighEfficiency);
    assert_eq!(h.issy, Some([0xAB, 0xCD, 0xEF]));
    let mut w = [0u8; 10];
    h.write(&mut w);
    assert_eq!(w, b);
    b[9] ^= 2;
    assert_eq!(BbHeader::parse(&b), Err(Error::Check(Check::BbHeaderCrc8)));
    assert_eq!(BbHeader::parse(&b[..9]), Err(Error::Short));
}

#[test]
fn gse_hem_is_sliced_only_where_the_standard_says() {
    let mut h = BbHeader::transport(800, 0);
    h.format = StreamFormat::Gse;
    assert!(
        h.gse_sliced(System::DvbS2),
        "TS/GS = 10 is GSE-HEM in DVB-S2X"
    );
    assert!(
        !h.gse_sliced(System::DvbT2),
        "DVB-T2 normal mode GSE is aligned to the data field"
    );
    h.mode = Mode::HighEfficiency;
    assert!(h.gse_sliced(System::DvbT2));
}

#[test]
fn bb_scrambler_first_bytes() {
    // EN 302 307-1 clause 5.2.2: 1 + x^14 + x^15 from 100101010000000. The sequence starts 0x03, 0xF6.
    let mut f = [0u8; 4];
    bb_descramble(&mut f);
    assert_eq!(&f[..2], &[0x03, 0xF6]);
    bb_descramble(&mut f);
    assert_eq!(f, [0; 4], "descrambling twice gives the input back");
}
