//! Bit conversions in the shapes the protocols use, on top of `sdrkit_util_bits::pack`: hard bits are `u8` 0 or 1, the first sent
//! first, and bytes hold their bits most significant first.

use sdrkit_util_bits::pack::{pack, unpack};
use sdrkit_util_bits::{BitOrder, Pad};

/// Bits to bytes, most significant bit first. A last partial byte is padded with zeros.
pub fn bits_to_bytes(bits: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(bits.len().div_ceil(8));
    pack(bits, BitOrder::Msb, Pad::Zero, &mut out);
    out
}

/// Bytes to bits, most significant bit first.
pub fn bytes_to_bits(bytes: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(bytes.len() * 8);
    unpack(bytes, BitOrder::Msb, &mut out);
    out
}

/// The first `N` bits of `bytes`, most significant bit first. Bits past the end of `bytes` are zero.
pub fn bits_of<const N: usize>(bytes: &[u8]) -> [u8; N] {
    std::array::from_fn(|i| bytes.get(i / 8).map_or(0, |b| (b >> (7 - i % 8)) & 1))
}
