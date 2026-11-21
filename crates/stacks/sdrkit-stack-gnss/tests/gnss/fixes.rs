//! The leap seconds of the station and the BeiDou word decoder.

use sdrkit_stack_gnss::bds;
use sdrkit_stack_gnss::core::{CoreConfig, GnssCore};
use sdrkit_util_geo::time::LeapSeconds;

/// Before the satellites broadcast GPS - UTC, the receiver takes it from the station's leap-second table: a table with a leap
/// second at 2026-01-01 results in 19 in GPS week 2400 (January 2026).
#[test]
fn leap_seconds_come_from_the_station_table() {
    let mut rx = GnssCore::new(CoreConfig::default());
    assert_eq!(rx.leap_s(302_400.0), Some(18));
    // 2026-01-01 00:00 UTC; TAI - UTC 38, so GPS - UTC 19.
    let table = LeapSeconds::BUILTIN.with_leap(1_767_225_600, 38).unwrap();
    rx.set_leap_table(table);
    assert_eq!(rx.leap_s(302_400.0), Some(19));
}

/// One error in each of six BCH(15,11) code words of a BeiDou subframe is corrected (six is the most a subframe can take).
#[test]
fn beidou_words_correct_one_error_each() {
    let mut info = vec![0u8; 224];
    info[..11].copy_from_slice(&bds::PREAMBLE);
    let sow = 354_486u32;
    sdrkit_util_bits::pack::put_u64(&mut info, 15, 2, 3).unwrap();
    sdrkit_util_bits::pack::put_u64(&mut info, 18, (sow >> 12) as u64, 8).unwrap();
    sdrkit_util_bits::pack::put_u64(&mut info, bds::W1, (sow & 0xFFF) as u64, 12).unwrap();
    for (i, b) in info.iter_mut().enumerate().skip(bds::W1 + 12) {
        *b = ((i * 7) % 3 == 0) as u8;
    }
    let mut tx = bds::encode_subframe(&info);
    // One error in word 1 (bit 20), and one in each of the two code words of words 2, 5 and 9 (bits 4 and 9 of the pair).
    tx[20] ^= 1;
    for w in [0, 3] {
        tx[30 + w * 30 + 4] ^= 1;
        tx[30 + w * 30 + 9] ^= 1;
    }
    tx[30 + 7 * 30 + 12] ^= 1;
    let sf = bds::decode_subframe(&tx, false).expect("a subframe");
    assert_eq!(sf.corrected, 6);
    assert_eq!(sf.info, info);
}
