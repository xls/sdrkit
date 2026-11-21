//! The choice between the portable arithmetic and the form for 64-bit ARM. Both give the same bits.

use crate::idct;
use crate::mc::{self, Source};

/// See [`mc::predict`].
#[inline]
pub(crate) fn predict(
    plain: bool,
    s: &Source,
    dst: &mut [u8],
    dstride: usize,
    w: usize,
    h: usize,
    average: bool,
) {
    #[cfg(target_arch = "aarch64")]
    if !plain {
        return crate::neon::predict(s, dst, dstride, w, h, average);
    }
    let _ = plain;
    mc::predict(s, dst, dstride, w, h, average);
}

/// See [`idct::put`].
#[inline]
pub(crate) fn put(plain: bool, block: &[i16; 64], dst: &mut [u8], stride: usize) {
    #[cfg(target_arch = "aarch64")]
    if !plain {
        return crate::neon::put(block, dst, stride);
    }
    let _ = plain;
    idct::put(block, dst, stride);
}

/// See [`idct::add`].
#[inline]
pub(crate) fn add(plain: bool, block: &[i16; 64], dst: &mut [u8], stride: usize) {
    #[cfg(target_arch = "aarch64")]
    if !plain {
        return crate::neon::add(block, dst, stride);
    }
    let _ = plain;
    idct::add(block, dst, stride);
}
