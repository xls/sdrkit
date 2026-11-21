//! Known answers from published sources.

use sdrkit_proto_lora_phy::coding::{self, header_checksum, payload_crc, CODEBOOKS};
use sdrkit_proto_lora_phy::params::{data_symbols, sync_symbols, BANDWIDTHS};
use sdrkit_proto_lora_phy::{fsk, Ldro, LoraParams};

/// The whitening sequence of the published LoRa decoders (Tapparel et al. 2020) starts FF FE FC F8 F0 E1 C2 85.
#[test]
fn whitening_starts_as_published() {
    let mut b = [0u8; 8];
    coding::whiten(&mut b);
    assert_eq!(b, [0xFF, 0xFE, 0xFC, 0xF8, 0xF0, 0xE1, 0xC2, 0x85]);
}

/// The CRC body is CRC-16/XMODEM: its check value over "123456789" is 0x31C3. The last two bytes go in by XOR.
#[test]
fn payload_crc_check_value() {
    let mut d = b"123456789".to_vec();
    d.extend_from_slice(&[0, 0]);
    assert_eq!(payload_crc(&d), 0x31C3);
    d[9] = 0x12;
    d[10] = 0x34;
    assert_eq!(payload_crc(&d), 0x31C3 ^ 0x1234);
}

/// The header checksum matrix of Tapparel et al. 2020 (section on the explicit header), by single bits.
#[test]
fn header_checksums() {
    assert_eq!(header_checksum(0, 0, 0), 0);
    assert_eq!(header_checksum(8, 0, 0), 0x18);
    assert_eq!(header_checksum(0, 0, 1), 0x0B);
    assert_eq!(header_checksum(0, 1, 0), 0x06);
    assert_eq!(header_checksum(1, 0, 0), 0x11);
    assert_eq!(header_checksum(0, 8, 0), 0x0C);
    assert_eq!(header_checksum(0, 0, 8), 0x05);
}

/// The Hamming codewords of the published decoders: at 4/8 nibble 1 is 0x8B (1000 1011), at 4/5 it is 0x11.
#[test]
fn hamming_codewords() {
    assert_eq!(
        CODEBOOKS[3],
        [0, 139, 78, 197, 45, 166, 99, 232, 23, 156, 89, 210, 58, 177, 116, 255]
    );
    assert_eq!(
        CODEBOOKS[0],
        [0, 17, 9, 24, 5, 20, 12, 29, 3, 18, 10, 27, 6, 23, 15, 30]
    );
    // The (8,4) code is an extended Hamming code: minimum distance 4. The (7,4) code has distance 3.
    for (c, d) in [(8usize, 4u32), (7, 3), (6, 2), (5, 2)] {
        let book = &CODEBOOKS[c - 5];
        let min = (0..16)
            .flat_map(|a| (0..16).map(move |b| (a, b)))
            .filter(|(a, b)| a != b)
            .map(|(a, b)| (book[a] ^ book[b]).count_ones())
            .min()
            .unwrap();
        assert_eq!(min, d, "4/{c}");
    }
}

/// The sync symbols: 0x34 (LoRaWAN) maps to 24 and 32, 0x12 to 8 and 16, and Meshtastic's 0x2B to 16 and -40 (2008 at SF11).
#[test]
fn sync_words() {
    assert_eq!(sync_symbols(0x34, 8), [24, 32]);
    assert_eq!(sync_symbols(0x12, 8), [8, 16]);
    assert_eq!(sync_symbols(0x2B, 11), [16, 2008]);
    assert_eq!(sync_symbols(0x2B, 7), [16, 88]);
}

/// Time on air by the formula of the SX1276 data sheet clause 4.1.1.7 and the SX1261/2 data sheet clause 6.1.4. The LoRaWAN
/// uplink with no application payload (13 bytes) at SF7 BW125 takes 46.336 ms. At SF12 BW125 (LDRO on) it takes 1155.072 ms.
/// These are the values of the Semtech LoRa calculator.
#[test]
fn airtime() {
    let mut p = LoraParams::new(7, 125e3, 5, 8, 0x34);
    assert_eq!(p.data_symbols(13, 5, true), 33);
    assert!((p.airtime(13) - 0.046_336).abs() < 1e-9);
    p.sf = 12;
    p.ldro = Ldro::Auto;
    assert!(p.ldro_on());
    assert_eq!(p.data_symbols(13, 5, true), 23);
    assert!((p.airtime(13) - 1.155_072).abs() < 1e-9);
    // Meshtastic LongFast: SF11 BW250 4/5, preamble 16, 50 bytes: 58 data symbols.
    let p = LoraParams::new(11, 250e3, 5, 16, 0x2B);
    assert_eq!(p.data_symbols(50, 5, true), 58);
    assert!((p.airtime(50) - (16.0 + 4.25 + 58.0) * 8.192e-3).abs() < 1e-9);
    // SF5 and SF6 use 20 in place of 28 and add 2 symbols to the header (SX1261/2 clause 6.1.4).
    assert_eq!(data_symbols(5, 5, 10, true, false, false), 8 + 5 * 5);
    let p = LoraParams::new(6, 125e3, 5, 8, 0x12);
    assert_eq!(p.header_symbols(), 14.25);
    assert_eq!(BANDWIDTHS[0], 7_812.5);
    assert_eq!(BANDWIDTHS[9], 500e3);
}

/// The FSK modulation: CRC-16/SEMTECH-FSK has the check value 0x1A33, and PN9 starts FF E1 1D 9A (TI DN509).
#[test]
fn fsk_known_answers() {
    assert_eq!(fsk::crc(b"123456789"), 0x1A33);
    let mut b = [0u8; 4];
    fsk::whiten(&mut b);
    assert_eq!(b, [0xFF, 0xE1, 0x1D, 0x9A]);
    assert!((fsk::airtime(10) - 21.0 * 8.0 / 50e3).abs() < 1e-12);
}
