//! Base-91 numbers of APRS (the APRS 1.0.1 protocol specification, chapter 9, compressed position format): digits are the characters `!`
//! (0) to `|` (90), most significant first.
//!
//! ```
//! use sdrkit_util_text::base91;
//!
//! // APRS 1.0.1 chapter 9 example: latitude 49°30'N is 380926 × (90 − 49.5) = 15 427 503, sent as "5L!!".
//! assert_eq!(base91::decode(b"5L!!"), Ok(15_427_503));
//! let mut b = [0u8; 4];
//! base91::encode(15_427_503, &mut b).unwrap();
//! assert_eq!(&b, b"5L!!");
//! ```

use crate::error::{Error, Result};

/// The value of base-91 digits (at most 9, so the value fits 64 bits).
pub fn decode(digits: &[u8]) -> Result<u64> {
    if digits.len() > 9 {
        return Err(Error::TooLong);
    }
    digits.iter().try_fold(0u64, |v, &c| match c {
        33..=124 => Ok(v * 91 + (c - 33) as u64),
        _ => Err(Error::Invalid),
    })
}

/// Writes `v` as exactly `out.len()` base-91 digits; [`Error::OutOfRange`] when it does not fit.
pub fn encode(mut v: u64, out: &mut [u8]) -> Result<()> {
    for d in out.iter_mut().rev() {
        *d = (v % 91) as u8 + 33;
        v /= 91;
    }
    if v == 0 {
        Ok(())
    } else {
        Err(Error::OutOfRange)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn aprs_chapter_9_examples() {
        // Latitude 49°30'00"N: 380926 × 40.5 = 15427503 = "5L!!".
        assert_eq!(decode(b"5L!!"), Ok(380_926 * 81 / 2));
        // Longitude 72°45'00"W: 190463 × 107.25 = 20427156 (truncated) = "<*e7".
        assert_eq!(decode(b"<*e7"), Ok(20_427_156));
        let mut b = [0u8; 4];
        encode(20_427_156, &mut b).unwrap();
        assert_eq!(&b, b"<*e7");
        assert_eq!(decode(b"}"), Err(Error::Invalid));
        assert_eq!(encode(91, &mut [0u8; 1]), Err(Error::OutOfRange));
        assert_eq!(decode(b"!!!!!!!!!!"), Err(Error::TooLong));
    }
}
