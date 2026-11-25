//! The sample formats a hub carries (Cu8, Cs8, Cs16, Cf32, little-endian as in a driver ring) and the conversions between
//! them.
//!
//! Integer conversions are exact where the target can hold the value: Cu8 and Cs8 differ by the top bit (`b ^ 0x80`), an
//! 8-bit part goes to Cs16 as `v << 8`, and Cs16 goes to 8 bits by round half up with saturation. A conversion to or from
//! Cf32 uses the scaling of [`sdrkit_driver_core::sample::to_iq`] (Cu8 as `(b - 127.5) / 128`, Cs8 as `v / 128`, Cs16 as
//! `v / 32768`) and its inverse with rounding to nearest, so 8- and 16-bit samples come back to their bytes exactly.

use sdrkit_core::Complex32;
use sdrkit_driver_core::sample::{from_iq, size, to_iq};
use sdrkit_graph::stream::SampleType;

/// True if a hub carries samples of type `t`: Cu8, Cs8, Cs16 or Cf32.
pub const fn supported(t: SampleType) -> bool {
    matches!(
        t,
        SampleType::Cu8 | SampleType::Cs8 | SampleType::Cs16 | SampleType::Cf32
    )
}

const FORMATS: [SampleType; 4] = [
    SampleType::Cu8,
    SampleType::Cs8,
    SampleType::Cs16,
    SampleType::Cf32,
];

/// A small number for a [`supported`] type, for an atomic: 0 to 3; 255 for another type.
pub(crate) fn index(t: SampleType) -> u8 {
    FORMATS
        .iter()
        .position(|f| *f == t)
        .map_or(255, |i| i as u8)
}

/// The type of [`index`] `i`.
pub(crate) fn from_index(i: u8) -> Option<SampleType> {
    FORMATS.get(usize::from(i)).copied()
}

/// Bytes of one complex sample of type `t`.
pub const fn bytes_per_sample(t: SampleType) -> usize {
    size(t)
}

/// The Cs8 part for a Cs16 part: `floor((v + 128) / 256)` (round half up), saturated at 127.
pub fn s8_from_s16(v: i16) -> i8 {
    ((i32::from(v) + 128) >> 8).min(127) as i8
}

/// Appends the samples of type `from` in `bytes` to `out` as type `to`. A trailing part of a sample is ignored. `scratch`
/// holds complex samples on the way to or from Cf32. Both types must be [`supported`].
pub fn convert(
    from: SampleType,
    bytes: &[u8],
    to: SampleType,
    out: &mut Vec<u8>,
    scratch: &mut Vec<Complex32>,
) {
    let whole = &bytes[..bytes.len() / size(from) * size(from)];
    use SampleType::{Cf32, Cs16, Cs8, Cu8};
    match (from, to) {
        _ if from == to => out.extend_from_slice(whole),
        (Cu8, Cs8) | (Cs8, Cu8) => out.extend(whole.iter().map(|b| b ^ 0x80)),
        (Cu8 | Cs8, Cs16) => {
            let flip = if from == Cu8 { 0x80 } else { 0 };
            for b in whole {
                out.extend_from_slice(&(i16::from((b ^ flip) as i8) << 8).to_le_bytes());
            }
        }
        (Cs16, Cu8 | Cs8) => {
            let flip = if to == Cu8 { 0x80 } else { 0 };
            for p in whole.as_chunks::<2>().0 {
                out.push(s8_from_s16(i16::from_le_bytes(*p)) as u8 ^ flip);
            }
        }
        (_, Cf32) => {
            scratch.clear();
            scratch.resize(whole.len() / size(from), Complex32::default());
            to_iq(from, whole, scratch);
            from_iq(Cf32, scratch, out);
        }
        (Cf32, _) => {
            scratch.clear();
            scratch.resize(whole.len() / 8, Complex32::default());
            to_iq(Cf32, whole, scratch);
            from_iq(to, scratch, out);
        }
        _ => unreachable!("only supported formats reach a conversion"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use SampleType::{Cf32, Cs16, Cs8, Cu8};

    fn conv(from: SampleType, bytes: &[u8], to: SampleType) -> Vec<u8> {
        let mut out = Vec::new();
        convert(from, bytes, to, &mut out, &mut Vec::new());
        out
    }

    #[test]
    fn eight_bit_formats_go_and_come_back_exactly() {
        let all: Vec<u8> = (0..=255u8).collect();
        for t in [Cu8, Cs8] {
            for via in [Cu8, Cs8, Cs16, Cf32] {
                assert_eq!(conv(via, &conv(t, &all, via), t), all, "{t:?} via {via:?}");
            }
        }
        assert_eq!(conv(Cu8, &[0, 128, 255, 127], Cs8), [0x80, 0, 0x7F, 0xFF]);
        assert_eq!(conv(Cs8, &[0x80, 0x7F], Cs16), [0x00, 0x80, 0x00, 0x7F]);
    }

    #[test]
    fn sixteen_bit_goes_through_floats_exactly() {
        let all: Vec<u8> = (0..=u16::MAX).flat_map(|v| v.to_le_bytes()).collect();
        assert_eq!(conv(Cf32, &conv(Cs16, &all, Cf32), Cs16), all);
    }

    #[test]
    fn sixteen_bit_to_eight_bit_rounds_half_up_and_saturates() {
        assert_eq!(s8_from_s16(0), 0);
        assert_eq!(s8_from_s16(127), 0);
        assert_eq!(s8_from_s16(128), 1);
        assert_eq!(s8_from_s16(-128), 0);
        assert_eq!(s8_from_s16(-129), -1);
        assert_eq!(s8_from_s16(32767), 127);
        assert_eq!(s8_from_s16(-32768), -128);
        let b: Vec<u8> = [256i16, -256, 32767, -32768]
            .iter()
            .flat_map(|v| v.to_le_bytes())
            .collect();
        assert_eq!(conv(Cs16, &b, Cs8), [1, 0xFF, 0x7F, 0x80]);
        assert_eq!(conv(Cs16, &b, Cu8), [0x81, 0x7F, 0xFF, 0x00]);
    }

    #[test]
    fn floats_saturate_and_a_trailing_part_is_ignored() {
        let mut b = Vec::new();
        from_iq(Cf32, &[Complex32::new(4.0, -4.0)], &mut b);
        b.push(9);
        assert_eq!(conv(Cf32, &b, Cu8), [255, 0]);
        assert_eq!(conv(Cf32, &b, Cs8), [0x7F, 0x80]);
        assert_eq!(conv(Cu8, &[1, 2, 3], Cs8), [0x81, 0x82]);
    }
}
