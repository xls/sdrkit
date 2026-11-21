//! The codes of D2-MAC/packet, taken from the shared crates: the packet header code, the two codes of line 625, the data group
//! CRC, the energy dispersal sequence and the packet interleave.
//!
//! Standard: ETS 300 250 Part 1 clause 3 and Part 5.

use super::frame::{RDF_BITS, SDF_BITS};
use sdrkit_algo_fec::small::SmallCode;
use sdrkit_util_bits::crc::catalog::CRC16_MAC_SI;
use sdrkit_util_bits::interleave::Permutation;
use sdrkit_util_bits::lfsr::Lfsr;
use sdrkit_util_bits::seq::MAC_PRBS15;
use std::sync::OnceLock;

/// Bits a packet.
pub const PACKET_BITS: usize = 751;
/// Packets a frame in one subframe.
pub const PACKETS: usize = 82;
/// The packet header code: Golay (23,12), generator x^11 + x^10 + x^6 + x^5 + x^4 + x^2 + 1.
pub fn header_code() -> &'static SmallCode {
    static C: OnceLock<SmallCode> = OnceLock::new();
    C.get_or_init(SmallCode::golay_23_12)
}

/// The code of the SDF: BCH (127,113) shortened to (71,57). It corrects two errors.
pub fn sdf_code() -> &'static SmallCode {
    static C: OnceLock<SmallCode> = OnceLock::new();
    C.get_or_init(|| SmallCode::bch_127_113(SDF_BITS).expect("71 is a valid length"))
}

/// The code of each RDF: BCH (127,113) shortened to (94,80). It corrects two errors.
pub fn rdf_code() -> &'static SmallCode {
    static C: OnceLock<SmallCode> = OnceLock::new();
    C.get_or_init(|| SmallCode::bch_127_113(RDF_BITS).expect("94 is a valid length"))
}

/// The packet interleave: data bit `k` of a packet is sent at position `94 k mod 751`.
pub fn packet_interleave() -> &'static Permutation {
    static P: OnceLock<Permutation> = OnceLock::new();
    P.get_or_init(|| Permutation::multiplicative(94, 0, PACKET_BITS).expect("94 is prime to 751"))
}

/// The CRC of a service identification data group (CRC-16/MAC-SI), sent low byte first after the group.
pub fn group_crc(data: &[u8]) -> u16 {
    CRC16_MAC_SI.checksum(data)
}

/// The energy dispersal register x^15 + x + 1, started at all ones.
pub fn dispersal() -> Lfsr {
    MAC_PRBS15
}
