//! CRCs of link layers that the CRC catalogue of `sdrkit-util-bits` has no name for, as engines of that catalogue.

use sdrkit_util_bits::crc::{Crc, CrcSpec};

/// The CRC-32 of a Meteosat high-rate DCP message: polynomial `741B8CD7` (hex), register 0, not reflected, no final XOR.
/// "CRC-32/HRDCP" is this crate's name for it; no catalogue name is known.
///
/// Standard: EUMETSAT TD 16, Meteosat Data Collection and Distribution Service, section 3.4.
///
/// ```
/// use sdrkit_proto_link::crc::CRC32_HRDCP;
/// assert_eq!(CRC32_HRDCP.checksum(b"123456789"), 0x085A_3197);
/// ```
pub const CRC32_HRDCP: Crc<u32> = Crc::<u32>::new(CrcSpec::new(
    "CRC-32/HRDCP",
    32,
    0x741B_8CD7,
    0,
    false,
    false,
    0,
    0x085A_3197,
));

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hrdcp_check_values() {
        assert_eq!(CRC32_HRDCP.checksum(b"123456789"), 0x085A_3197);
        assert_eq!(CRC32_HRDCP.checksum(b"CatMouse987654321"), 0x1FC0_DFEC);
    }
}
