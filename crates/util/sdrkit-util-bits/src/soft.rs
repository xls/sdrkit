//! The [`Soft`] trait: one bit in any of the three representations.

/// One bit as a hard bit (`u8`, 0 or 1), an `i8` LLR or an `f32` LLR.
///
/// LLRs follow the crate convention `L = ln P(b=0) / P(b=1)`: positive means 0. Scramblers, line decoders and interleavers are
/// written once over `T: Soft`: flipping a hard bit XORs it with 1, flipping an LLR negates it and keeps its reliability.
///
/// ```
/// use sdrkit_util_bits::Soft;
/// assert_eq!(1u8.flip(), 0);
/// assert_eq!(90i8.flip(), -90);
/// assert_eq!((-128i8).flip(), 127); // saturates: -128 is never produced
/// assert_eq!((-3.5f32).hard(), 1);
/// assert_eq!(<i8 as Soft>::from_hard(1), -127);
/// ```
pub trait Soft: Copy + PartialEq + Send + Sync + std::fmt::Debug + 'static {
    /// The value that carries no information: 0 for an LLR (an erasure or a punctured bit), 0 for a hard bit.
    const ZERO: Self;
    /// The same bit inverted.
    fn flip(self) -> Self;
    /// The hard decision: 0 or 1.
    fn hard(self) -> u8;
    /// A hard bit (only its lowest bit is read) as this type, at full confidence.
    fn from_hard(bit: u8) -> Self;
    /// The value as an `f32` LLR: a hard bit is ±1 (0 is +1), an `i8` keeps its value.
    fn llr(self) -> f32;
    /// Inverts the bit when the lowest bit of `bit` is 1.
    #[inline]
    fn flip_if(self, bit: u8) -> Self {
        if bit & 1 != 0 {
            self.flip()
        } else {
            self
        }
    }
}

impl Soft for u8 {
    const ZERO: Self = 0;
    #[inline]
    fn flip(self) -> Self {
        (self & 1) ^ 1
    }
    #[inline]
    fn hard(self) -> u8 {
        self & 1
    }
    #[inline]
    fn from_hard(bit: u8) -> Self {
        bit & 1
    }
    #[inline]
    fn flip_if(self, bit: u8) -> Self {
        (self ^ bit) & 1
    }
    #[inline]
    fn llr(self) -> f32 {
        1.0 - 2.0 * (self & 1) as f32
    }
}

impl Soft for i8 {
    const ZERO: Self = 0;
    #[inline]
    fn flip(self) -> Self {
        self.saturating_neg()
    }
    #[inline]
    fn hard(self) -> u8 {
        (self < 0) as u8
    }
    #[inline]
    fn from_hard(bit: u8) -> Self {
        if bit & 1 == 0 {
            127
        } else {
            -127
        }
    }
    #[inline]
    fn llr(self) -> f32 {
        self as f32
    }
}

impl Soft for f32 {
    const ZERO: Self = 0.0;
    #[inline]
    fn flip(self) -> Self {
        -self
    }
    #[inline]
    fn hard(self) -> u8 {
        (self < 0.0) as u8
    }
    #[inline]
    fn from_hard(bit: u8) -> Self {
        if bit & 1 == 0 {
            1.0
        } else {
            -1.0
        }
    }
    #[inline]
    fn llr(self) -> f32 {
        self
    }
}

/// Arithmetic on soft bits (`i8` and `f32` LLRs) that line decoders need: adding two independent observations of a bit, and the
/// LLR of the XOR of two bits (the min-sum rule).
///
/// ```
/// use sdrkit_util_bits::Llr;
/// assert_eq!(100i8.add_llr(100), 127); // saturates
/// assert_eq!(20i8.xor_llr(-50), -20); // different signs: probably 1; as sure as the weaker one
/// assert_eq!((-2.0f32).xor_llr(-3.0), 2.0);
/// ```
pub trait Llr: Soft {
    /// The LLR of a bit seen twice (independent observations): the sum, saturated for `i8`.
    fn add_llr(self, other: Self) -> Self;
    /// The LLR of `a XOR b`: sign product, smaller magnitude.
    fn xor_llr(self, other: Self) -> Self;
}

impl Llr for i8 {
    #[inline]
    fn add_llr(self, other: Self) -> Self {
        (self as i16 + other as i16).clamp(-127, 127) as i8
    }
    #[inline]
    fn xor_llr(self, other: Self) -> Self {
        let m = self.unsigned_abs().min(other.unsigned_abs()).min(127) as i8;
        if (self < 0) != (other < 0) {
            -m
        } else {
            m
        }
    }
}

impl Llr for f32 {
    #[inline]
    fn add_llr(self, other: Self) -> Self {
        self + other
    }
    #[inline]
    fn xor_llr(self, other: Self) -> Self {
        let m = self.abs().min(other.abs());
        if (self < 0.0) != (other < 0.0) {
            -m
        } else {
            m
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hard_bits_flip_and_decide() {
        assert_eq!(0u8.flip(), 1);
        assert_eq!(1u8.flip_if(1), 0);
        assert_eq!(1u8.flip_if(0), 1);
        assert_eq!(3u8.hard(), 1);
    }

    #[test]
    fn llrs_positive_mean_zero() {
        assert_eq!(5i8.hard(), 0);
        assert_eq!((-5i8).hard(), 1);
        assert_eq!(0i8.hard(), 0);
        assert_eq!(i8::from_hard(0), 127);
        assert_eq!((-128i8).flip(), 127);
        assert_eq!(2.0f32.flip_if(1), -2.0);
        assert_eq!(f32::from_hard(0), 1.0);
        // A flip keeps the reliability.
        assert_eq!((-77i8).flip().unsigned_abs(), 77);
    }
}
