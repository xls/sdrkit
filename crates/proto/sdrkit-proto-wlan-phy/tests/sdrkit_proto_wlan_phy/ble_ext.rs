//! The LE Coded sample packet, the channel selection sample data (Bluetooth Core Specification v5.4 Vol 6 Part C clauses 2
//! and 3) and the AuxPtr and SyncInfo layouts (Vol 6 Part B figures 2.19 and 2.21).

use sdrkit_proto_wlan_phy::ble::csa::{self, ChannelMap, Csa1};
use sdrkit_proto_wlan_phy::ble::phy::{coded_block1, pattern_map, FecEncoder};
use sdrkit_proto_wlan_phy::ble::{self, AuxPtr, Coding, SyncInfo};
use sdrkit_util_bits::{pack, BitOrder};

/// Symbols written as in the sample data: digits, spaces ignored.
fn symbols(text: &str) -> Vec<u8> {
    text.bytes()
        .filter(|b| b.is_ascii_digit())
        .map(|b| b - b'0')
        .collect()
}

/// The coded symbols of the reference packet of Part C clause 2.1 (whitening is not part of the sample): FEC block 1 and FEC
/// block 2 of the PDU and CRC as given, TERM2.
fn reference(coding: Coding) -> Vec<u8> {
    let mut out = Vec::new();
    let mut enc = coded_block1(ble::ADV_ACCESS_ADDRESS, coding, &mut out);
    let mut bits = Vec::new();
    pack::unpack(&[0x00, 0x03, 0x42, 0x4C, 0x45, 0x29, 0x0A, 0xCE], BitOrder::Lsb, &mut bits);
    bits.extend_from_slice(&[0, 0, 0]);
    let mut coded = Vec::new();
    enc.encode(&bits, &mut coded);
    pattern_map(&coded, coding, &mut out);
    out
}

const AA: &str = "0011 0011 1100 1100 0011 1100 0011 1100 1100 1100 0011 1100
0011 0011 1100 0011 0011 1100 1100 1100 1100 0011 1100 0011
0011 1100 0011 1100 1100 0011 1100 1100 1100 0011 0011 1100
0011 0011 0011 0011 1100 0011 1100 1100 1100 1100 1100 1100
1100 0011 0011 0011 1100 0011 1100 0011 1100 0011 0011 0011
1100 1100 1100 1100";

#[test]
fn reference_packet_crc_is_part_c_2_1() {
    assert_eq!(
        ble::crc24_bytes(&[0x00, 0x03, 0x42, 0x4C, 0x45], ble::ADV_CRC_INIT),
        [0x29, 0x0A, 0xCE]
    );
}

#[test]
fn coded_reference_packet_is_part_c_2_3_at_s2() {
    let want = symbols(&format!(
        "{AA} 0011 1100 0011 1100 0011 0011 1100 1100 0011 0011
0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 1 1 0 1 0 1 0 0 1 1 0 0 0 0 0 0 0 0 1 1 1 0 1
1 1 1 0 0 1 1 1 0 1 1 1 1 1 1 0 1 0 1 0 0 0 0 1 0 0 0 0 1 0 0 0 1 1 1 1 1 1 1
1 0
0 0 0 1 1 1 0 0 1 0 0 0 0 1 1 1 1 1 1 1 1 0 0 0 0 1 1 1 1 1 0 0 0 0 1 1 0 1 1
0 1 0 0 0 0 0 0 1
0 1 0 0 1 1"
    ));
    let got = reference(Coding::S2);
    assert_eq!(got, want);
    // 510 us with the 80-symbol preamble.
    assert_eq!(got.len() + 80, 510);
}

#[test]
fn coded_reference_packet_is_part_c_2_4_at_s8() {
    let want = symbols(&format!(
        "{AA} 1100 0011 1100 1100 1100 1100 0011 0011 0011 0011
0011 0011 0011 0011 0011 0011 0011 0011 0011 0011 0011 0011
0011 0011 0011 0011 1100 1100 0011 1100 0011 1100 0011 0011
1100 1100 0011 0011 0011 0011 0011 0011 0011 0011 1100 1100
1100 0011 1100 1100 1100 1100 0011 0011 1100 1100 1100 0011
1100 1100 1100 1100 1100 1100 0011 1100 0011 1100 0011 0011
0011 0011 1100 0011 0011 0011 0011 1100 0011 0011 0011 1100
1100 1100 1100 1100 1100 1100 1100 0011
0011 0011 0011 1100 1100 1100 0011 0011 1100 0011 0011 0011
0011 1100 1100 1100 1100 1100 1100 1100 1100 0011 0011 0011
0011 1100 1100 1100 1100 1100 0011 0011 0011 0011 1100 1100
0011 1100 1100 0011 1100 0011 0011 0011 0011 0011 0011 1100
0011 1100 0011 0011 1100 1100"
    ));
    let got = reference(Coding::S8);
    assert_eq!(got, want);
    assert_eq!(got.len() + 80, 912);
    // The encoder of the sample's access address: its state is 4 after the third bit (octal, Part C clause 2.2).
    let mut e = FecEncoder::default();
    let mut o = Vec::new();
    e.encode(&[0, 1, 1], &mut o);
    assert_eq!(o, [0, 0, 1, 1, 0, 1]);
}

#[test]
fn csa2_is_part_c_sample_data() {
    let id = csa::channel_identifier(0x8E89_BED6);
    assert_eq!(id, 0x305F);
    let all = ChannelMap::all();
    for (c, prn, ch) in [(0u16, 56857u16, 25u8), (1, 1685, 20), (2, 38301, 6), (3, 27475, 21)] {
        assert_eq!(csa::prn_e(c, id), prn);
        assert_eq!(csa::csa2_channel(c, id, &all), ch);
    }
    // Channel map [36:0] = 0b11110_00000000_11100000_00000110_00000000.
    let map = ChannelMap::from_bytes([0x00, 0x06, 0xE0, 0x00, 0x1E]).unwrap();
    assert_eq!(map.used(), [9, 10, 21, 22, 23, 33, 34, 35, 36]);
    for (c, prn, ch) in [(6u16, 10975u16, 23u8), (7, 5490, 9), (8, 46970, 34)] {
        assert_eq!(csa::prn_e(c, id), prn);
        assert_eq!(csa::csa2_channel(c, id, &map), ch);
    }
    assert!(ChannelMap::from_bytes([1, 0, 0, 0, 0]).is_none());
}

#[test]
fn csa1_steps_by_the_hop_and_remaps_unused_channels() {
    // Clause 4.5.8.2 worked by hand: hop 7 from 0 gives 7, 14, 21, 28, 35, 5 (42 mod 37), ...
    let all = ChannelMap::all();
    let mut a = Csa1::new(7);
    let got: Vec<u8> = (0..7).map(|_| a.next(&all)).collect();
    assert_eq!(got, [7, 14, 21, 28, 35, 5, 12]);
    for (k, &c) in got.iter().enumerate() {
        assert_eq!(Csa1::channel_of(7, k as u16, &all), c);
    }
    // Only channels 0 to 9 used: 14 is unused and remaps to used[14 mod 10] = 4.
    let ten = ChannelMap::from_bytes([0xFF, 0x03, 0, 0, 0]).unwrap();
    let mut b = Csa1::new(7);
    assert_eq!((b.next(&ten), b.next(&ten)), (7, 4));
}

#[test]
fn aux_ptr_and_sync_info_are_the_figures() {
    // Channel 12, CA 1, 300 us units, offset 0x1ABC, LE Coded: bits 0-5, 6, 7, 8-20, 21-23.
    let a = AuxPtr::read([0xCC, 0xBC, 0x5A]);
    assert_eq!(
        a,
        AuxPtr {
            channel: 12,
            clock_50ppm: true,
            units_300us: true,
            offset: 0x1ABC,
            phy_code: 2,
        }
    );
    assert_eq!(a.bytes(), [0xCC, 0xBC, 0x5A]);
    assert_eq!(a.phy(), Some(ble::Phy::LeCoded));
    assert!((a.offset_s() - 0x1ABC as f64 * 300e-6).abs() < 1e-12);
    let mut b = [0u8; 18];
    b[0] = 0x34; // Offset Base 0x0234, 30 us units, no adjust
    b[1] = 0x02;
    b[2] = 80; // Interval 80 x 1.25 ms
    b[4..9].copy_from_slice(&[0x03, 0x00, 0x00, 0x00, 0xA0]); // channels 0 and 1, SCA 5
    b[9..13].copy_from_slice(&0x5065_17A3u32.to_le_bytes());
    b[13..16].copy_from_slice(&[0x33, 0x22, 0x11]);
    b[16..18].copy_from_slice(&7u16.to_le_bytes());
    let s = SyncInfo::read(&b);
    assert_eq!(s.offset_base, 0x234);
    assert!(!s.units_300us && !s.offset_adjust);
    assert_eq!(s.interval, 80);
    assert_eq!(s.channel_map, [0x03, 0, 0, 0, 0]);
    assert_eq!(s.sca, 5);
    assert_eq!(s.access_address, 0x5065_17A3);
    assert_eq!(s.crc_init, 0x11_2233);
    assert_eq!(s.event_counter, 7);
    assert_eq!(s.bytes(), b);
    assert!((s.interval_s() - 0.1).abs() < 1e-12);
}
