//! The frame checks of the ISM device protocols that no catalogue names: CRCs with device parameters on the shared CRC engine.
//! The LFSR digests of some sensors are in `sdrkit_util_bits::checksum`.

use sdrkit_util_bits::crc::{Crc, CrcSpec};

/// CRC-8 poly 0x31, register 0, not reflected (Fine Offset WH2 and WH24, LaCrosse TX29-IT, TX141W).
pub static CRC8_31: Crc<u8> = Crc::<u8>::new(CrcSpec::new(
    "CRC-8/ISM-31",
    8,
    0x31,
    0x00,
    false,
    false,
    0x00,
    0xA2,
));
/// CRC-8 poly 0x31, register 0x6C (Rubicson).
pub static CRC8_RUBICSON: Crc<u8> = Crc::<u8>::new(CrcSpec::new(
    "CRC-8/RUBICSON",
    8,
    0x31,
    0x6C,
    false,
    false,
    0x00,
    0x86,
));
/// CRC-8 poly 0x07, register 0xF0 (Schrader TPMS).
pub static CRC8_SCHRADER: Crc<u8> = Crc::<u8>::new(CrcSpec::new(
    "CRC-8/SCHRADER",
    8,
    0x07,
    0xF0,
    false,
    false,
    0x00,
    0x52,
));
/// CRC-8 poly 0x07, register 0x80 (Toyota TPMS).
pub static CRC8_TOYOTA: Crc<u8> = Crc::<u8>::new(CrcSpec::new(
    "CRC-8/TOYOTA",
    8,
    0x07,
    0x80,
    false,
    false,
    0x00,
    0xC0,
));
/// CRC-8 poly 0x07, register 0xAA (Hyundai and Kia VDO TPMS).
pub static CRC8_HYUNDAI: Crc<u8> = Crc::<u8>::new(CrcSpec::new(
    "CRC-8/HYUNDAI-VDO",
    8,
    0x07,
    0xAA,
    false,
    false,
    0x00,
    0xFE,
));
/// CRC-8 poly 0x13, register 0 (Pacific PMV-107J TPMS).
pub static CRC8_PMV107J: Crc<u8> = Crc::<u8>::new(CrcSpec::new(
    "CRC-8/PMV-107J",
    8,
    0x13,
    0x00,
    false,
    false,
    0x00,
    0x0B,
));
/// CRC-4 poly 0x3, register 0, on the bytes (Kedsum).
pub static CRC4_KEDSUM: Crc<u8> = Crc::<u8>::new(CrcSpec::new(
    "CRC-4/KEDSUM",
    4,
    0x3,
    0x0,
    false,
    false,
    0x0,
    0xE,
));
/// CRC-16 poly 0x6F63, register 0 (Itron ERT SCM).
pub static CRC16_ERT_SCM: Crc<u16> = Crc::<u16>::new(CrcSpec::new(
    "CRC-16/ERT-SCM",
    16,
    0x6F63,
    0x0000,
    false,
    false,
    0x0000,
    0xBDF4,
));
/// CRC-16 poly 0x1021, register 0xD895 (Itron ERT IDM).
pub static CRC16_ERT_IDM: Crc<u16> = Crc::<u16>::new(CrcSpec::new(
    "CRC-16/ERT-IDM",
    16,
    0x1021,
    0xD895,
    false,
    false,
    0x0000,
    0x2322,
));
/// CRC-16 poly 0x8050, register 0 (Honeywell sensors on channels 2, 4, 9, 10 and 12).
pub static CRC16_HONEYWELL_8050: Crc<u16> = Crc::<u16>::new(CrcSpec::new(
    "CRC-16/HONEYWELL-8050",
    16,
    0x8050,
    0x0000,
    false,
    false,
    0x0000,
    0xDA60,
));

/// Every CRC of this module, for the check-value test.
pub fn all() -> [&'static CrcSpec; 10] {
    [
        CRC8_31.spec(),
        CRC8_RUBICSON.spec(),
        CRC8_SCHRADER.spec(),
        CRC8_TOYOTA.spec(),
        CRC8_HYUNDAI.spec(),
        CRC8_PMV107J.spec(),
        CRC4_KEDSUM.spec(),
        CRC16_ERT_SCM.spec(),
        CRC16_ERT_IDM.spec(),
        CRC16_HONEYWELL_8050.spec(),
    ]
}
