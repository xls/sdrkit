//! Native samples: how many bytes one sample of each [`SampleType`] takes in a ring, and the conversion of native samples to
//! complex samples. A ring holds multi-byte values in little-endian order.

use sdrkit_core::Complex32;
use sdrkit_graph::stream::SampleType;

/// Bytes of one sample of type `t` (both parts for a complex type).
pub const fn size(t: SampleType) -> usize {
    match t {
        SampleType::I8 | SampleType::U8 => 1,
        SampleType::I16 | SampleType::Cs8 | SampleType::Cu8 => 2,
        SampleType::I32 | SampleType::F32 | SampleType::Cs16 => 4,
        SampleType::Cf32 | SampleType::Cs32 => 8,
    }
}

/// True if a sample of type `t` has an I and a Q part.
pub const fn is_complex(t: SampleType) -> bool {
    matches!(
        t,
        SampleType::Cs8 | SampleType::Cu8 | SampleType::Cs16 | SampleType::Cs32 | SampleType::Cf32
    )
}

/// The value of an unsigned 8-bit part: (b - 127.5) / 128.
fn u8_table() -> &'static [f32; 256] {
    static T: std::sync::OnceLock<[f32; 256]> = std::sync::OnceLock::new();
    T.get_or_init(|| std::array::from_fn(|b| (b as f32 - 127.5) / 128.0))
}

/// Converts the whole samples of type `t` in `bytes` into `out`, and returns how many it wrote: the smaller of the samples in
/// `bytes` and `out.len()`. Integers are scaled so full scale is a magnitude of about 1. A real sample has Q = 0.
pub fn to_iq(t: SampleType, bytes: &[u8], out: &mut [Complex32]) -> usize {
    let n = (bytes.len() / size(t)).min(out.len());
    let chunks = bytes.chunks_exact(size(t)).zip(out[..n].iter_mut());
    let le32 = |c: &[u8]| [c[0], c[1], c[2], c[3]];
    match t {
        SampleType::Cs8 => chunks.for_each(|(c, o)| {
            *o = Complex32::new(f32::from(c[0] as i8) / 128.0, f32::from(c[1] as i8) / 128.0);
        }),
        SampleType::Cu8 => {
            let tab = u8_table();
            chunks.for_each(|(c, o)| *o = Complex32::new(tab[c[0] as usize], tab[c[1] as usize]));
        }
        SampleType::Cs16 => chunks.for_each(|(c, o)| {
            let i = i16::from_le_bytes([c[0], c[1]]);
            let q = i16::from_le_bytes([c[2], c[3]]);
            *o = Complex32::new(f32::from(i) / 32768.0, f32::from(q) / 32768.0);
        }),
        SampleType::Cs32 => chunks.for_each(|(c, o)| {
            let i = i32::from_le_bytes(le32(c));
            let q = i32::from_le_bytes(le32(&c[4..]));
            *o = Complex32::new(i as f32 / 2_147_483_648.0, q as f32 / 2_147_483_648.0);
        }),
        SampleType::Cf32 => chunks.for_each(|(c, o)| {
            *o = Complex32::new(
                f32::from_le_bytes(le32(c)),
                f32::from_le_bytes(le32(&c[4..])),
            );
        }),
        SampleType::I8 => {
            chunks.for_each(|(c, o)| *o = Complex32::new(f32::from(c[0] as i8) / 128.0, 0.0));
        }
        SampleType::U8 => {
            let tab = u8_table();
            chunks.for_each(|(c, o)| *o = Complex32::new(tab[c[0] as usize], 0.0));
        }
        SampleType::I16 => chunks.for_each(|(c, o)| {
            *o = Complex32::new(f32::from(i16::from_le_bytes([c[0], c[1]])) / 32768.0, 0.0);
        }),
        SampleType::I32 => chunks.for_each(|(c, o)| {
            *o = Complex32::new(i32::from_le_bytes(le32(c)) as f32 / 2_147_483_648.0, 0.0);
        }),
        SampleType::F32 => {
            chunks.for_each(|(c, o)| *o = Complex32::new(f32::from_le_bytes(le32(c)), 0.0));
        }
    }
    n
}

/// Appends `iq` to `out` as samples of type `to`: the inverse of the scaling of [`to_iq`], rounded to nearest and
/// saturated. Cu8 is `round(x * 128 + 127.5)`, Cs8 `round(x * 128)`, Cs16 `round(x * 32768)`, Cf32 the floats
/// themselves. Samples that [`to_iq`] made from 8- or 16-bit bytes return to those bytes exactly.
///
/// # Panics
///
/// If `to` is not Cu8, Cs8, Cs16 or Cf32.
pub fn from_iq(to: SampleType, iq: &[Complex32], out: &mut Vec<u8>) {
    let q = |x: f32, scale: f32, offset: f32, lo: f32, hi: f32| {
        (x * scale + offset).round().clamp(lo, hi)
    };
    match to {
        SampleType::Cu8 => {
            for c in iq {
                out.push(q(c.re, 128.0, 127.5, 0.0, 255.0) as u8);
                out.push(q(c.im, 128.0, 127.5, 0.0, 255.0) as u8);
            }
        }
        SampleType::Cs8 => {
            for c in iq {
                out.push(q(c.re, 128.0, 0.0, -128.0, 127.0) as i8 as u8);
                out.push(q(c.im, 128.0, 0.0, -128.0, 127.0) as i8 as u8);
            }
        }
        SampleType::Cs16 => {
            for c in iq {
                let (i, v) = (
                    q(c.re, 32768.0, 0.0, -32768.0, 32767.0) as i16,
                    q(c.im, 32768.0, 0.0, -32768.0, 32767.0) as i16,
                );
                out.extend_from_slice(&i.to_le_bytes());
                out.extend_from_slice(&v.to_le_bytes());
            }
        }
        SampleType::Cf32 => {
            for c in iq {
                out.extend_from_slice(&c.re.to_le_bytes());
                out.extend_from_slice(&c.im.to_le_bytes());
            }
        }
        other => panic!("no conversion from complex samples to {other:?}"),
    }
}

/// Writes `iq` as [`SampleType::Cf32`] bytes into `out`, which must hold `8 * iq.len()` bytes.
///
/// # Panics
///
/// If `out` is shorter.
pub fn iq_to_bytes(iq: &[Complex32], out: &mut [u8]) {
    for (c, o) in iq.iter().zip(out.as_chunks_mut::<8>().0.iter_mut()) {
        o[..4].copy_from_slice(&c.re.to_le_bytes());
        o[4..].copy_from_slice(&c.im.to_le_bytes());
    }
    assert!(
        out.len() >= iq.len() * 8,
        "the byte buffer holds every sample"
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn integers_scale_to_full_scale() {
        let mut out = [Complex32::default(); 2];
        assert_eq!(to_iq(SampleType::Cu8, &[255, 0, 128, 127, 9], &mut out), 2);
        assert_eq!(out[0], Complex32::new(127.5 / 128.0, -127.5 / 128.0));
        assert_eq!(to_iq(SampleType::Cs8, &[0x80, 64], &mut out), 1);
        assert_eq!(out[0], Complex32::new(-1.0, 0.5));
        let b = [0x00, 0x40, 0x00, 0xc0];
        assert_eq!(to_iq(SampleType::Cs16, &b, &mut out), 1);
        assert_eq!(out[0], Complex32::new(0.5, -0.5));
        assert_eq!(to_iq(SampleType::I16, &b, &mut out), 2);
        assert_eq!(out[1], Complex32::new(-0.5, 0.0));
        let b = [0, 0, 0, 0x40, 0, 0, 0, 0xc0, 1];
        assert_eq!(to_iq(SampleType::Cs32, &b, &mut out), 1);
        assert_eq!(out[0], Complex32::new(0.5, -0.5));
    }

    #[test]
    fn integer_samples_return_to_their_bytes() {
        let all8: Vec<u8> = (0..=255u8).collect();
        let all16: Vec<u8> = (0..=u16::MAX).flat_map(u16::to_le_bytes).collect();
        for (t, bytes) in [
            (SampleType::Cu8, &all8),
            (SampleType::Cs8, &all8),
            (SampleType::Cs16, &all16),
        ] {
            let mut iq = vec![Complex32::default(); bytes.len() / size(t)];
            to_iq(t, bytes, &mut iq);
            let mut back = Vec::new();
            from_iq(t, &iq, &mut back);
            assert_eq!(&back, bytes, "{t:?}");
        }
        let mut b = Vec::new();
        from_iq(SampleType::Cu8, &[Complex32::new(4.0, -4.0)], &mut b);
        from_iq(SampleType::Cs8, &[Complex32::new(4.0, -4.0)], &mut b);
        assert_eq!(b, [255, 0, 0x7F, 0x80]);
    }

    #[test]
    fn floats_go_and_come_back() {
        let iq = [Complex32::new(0.25, -1.5), Complex32::new(3.0, 0.0)];
        let mut b = [0u8; 16];
        iq_to_bytes(&iq, &mut b);
        let mut out = [Complex32::default(); 4];
        assert_eq!(to_iq(SampleType::Cf32, &b, &mut out), 2);
        assert_eq!(&out[..2], &iq);
    }
}
