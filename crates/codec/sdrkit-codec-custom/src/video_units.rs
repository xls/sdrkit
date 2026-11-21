//! The synthetic video test stream. It stands in for the video and audio codecs: each unit's bytes are a known function of the
//! stream and the unit number, so a receiver can test every unit. The stream stands in for AV1 and Opus for real content. Those are host or
//! library codecs and are not part of this crate.

use crate::{Error, Param};
use sdrkit_util_bits::xorshift::Xorshift64;

/// Writes the unit bytes of stream byte `stream`, unit number `unit`, into `out` (its length is the unit length).
///
/// The seed is (stream << 40) XOR (unit times 0x9E3779B97F4A7C15 mod 2^64) XOR 0x1234567. Byte i is bits 24 to 31 of step i
/// of the shared xorshift64 generator.
pub fn unit_bytes(stream: u8, unit: u64, out: &mut [u8]) {
    let seed = ((stream as u64) << 40) ^ unit.wrapping_mul(Xorshift64::GOLDEN) ^ 0x123_4567;
    let mut g = Xorshift64::new(seed);
    for b in out.iter_mut() {
        *b = (g.step() >> 24) as u8;
    }
}

/// The parameters of the synthetic stream.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SyntheticParams {
    /// Video rate in bit/s.
    pub video_bps: f64,
    /// Video frame rate in frames per second.
    pub frame_rate: f64,
    /// Audio rate in bit/s.
    pub audio_bps: f64,
    /// Key interval in video units (default 30).
    pub key_interval: u64,
}

impl Default for SyntheticParams {
    fn default() -> Self {
        Self {
            video_bps: 1_200_000.0,
            frame_rate: 30.0,
            audio_bps: 96_000.0,
            key_interval: 30,
        }
    }
}

/// The kind of a synthetic unit.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum UnitKind {
    /// A video unit: packet type 0, stream byte 0.
    Video,
    /// An audio unit: packet type 1, stream byte 1 for the bytes (the packets carry stream 0).
    Audio,
}

impl UnitKind {
    /// The packet type.
    pub const fn packet_type(self) -> u8 {
        match self {
            UnitKind::Video => 0,
            UnitKind::Audio => 1,
        }
    }

    /// The stream byte used for the unit bytes.
    pub const fn stream_byte(self) -> u8 {
        match self {
            UnitKind::Video => 0,
            UnitKind::Audio => 1,
        }
    }
}

/// One synthetic unit, without its bytes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Unit {
    /// Video or audio.
    pub kind: UnitKind,
    /// The unit number within its kind, from 0.
    pub number: u64,
    /// The presentation time in milliseconds.
    pub time_ms: u32,
    /// True for a unit that decodes alone. Every audio unit is a key unit.
    pub key: bool,
    /// The unit length in bytes.
    pub len: usize,
}

impl Unit {
    /// Writes the unit bytes into `out`.
    ///
    /// Errors: [`Error::OutputTooShort`] when `out` is shorter than the unit.
    pub fn fill(&self, out: &mut [u8]) -> Result<(), Error> {
        let o = out
            .get_mut(..self.len)
            .ok_or(Error::OutputTooShort { need: self.len })?;
        unit_bytes(self.kind.stream_byte(), self.number, o);
        Ok(())
    }
}

/// The generator of synthetic units: video and audio units in time order.
#[derive(Clone, Debug)]
pub struct Synthetic {
    p: SyntheticParams,
    tv: f64,
    ta: f64,
    nv: u64,
    na: u64,
}

impl Synthetic {
    /// Makes a generator.
    ///
    /// Errors: [`Error::BadParameter`] when a rate is not positive and finite, or the key interval is 0.
    pub fn new(p: SyntheticParams) -> Result<Self, Error> {
        let ok = |v: f64| v.is_finite() && v > 0.0;
        if !ok(p.video_bps) || !ok(p.frame_rate) || !ok(p.audio_bps) || p.key_interval == 0 {
            return Err(Error::BadParameter {
                param: Param::SyntheticRates,
            });
        }
        Ok(Self {
            p,
            tv: 0.0,
            ta: 0.0,
            nv: 0,
            na: 0,
        })
    }

    /// The media time in seconds: the larger of the video and the audio clock.
    pub fn media_time(&self) -> f64 {
        self.tv.max(self.ta)
    }

    /// Makes the next unit: video when the video clock is not ahead of the audio clock, else audio.
    pub fn next_unit(&mut self) -> Unit {
        if self.tv <= self.ta {
            let i = self.p.key_interval;
            let key = self.nv.is_multiple_of(i);
            let avg = self.p.video_bps / 8.0 / self.p.frame_rate;
            let base = avg * i as f64 / (i + 4) as f64;
            let len = if key {
                (5.0 * base).floor()
            } else {
                base.floor()
            } as usize;
            let u = Unit {
                kind: UnitKind::Video,
                number: self.nv,
                time_ms: (self.tv * 1000.0).floor() as u32,
                key,
                len,
            };
            self.nv += 1;
            self.tv += 1.0 / self.p.frame_rate;
            u
        } else {
            let len = (self.p.audio_bps / 8.0 * 0.02).floor() as usize;
            let u = Unit {
                kind: UnitKind::Audio,
                number: self.na,
                time_ms: (self.ta * 1000.0).floor() as u32,
                key: true,
                len,
            };
            self.na += 1;
            self.ta += 0.02;
            u
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // the two known values.
    #[test]
    fn unit_bytes_check_values() {
        let mut b = [0u8; 16];
        unit_bytes(0, 0, &mut b);
        assert_eq!(
            b,
            [
                0x55, 0xC1, 0x48, 0x41, 0xA7, 0xDC, 0x11, 0xC1, 0x74, 0xCA, 0x26, 0x24, 0x30, 0xA8,
                0x7A, 0xA9
            ]
        );
        unit_bytes(1, 5, &mut b);
        assert_eq!(
            b,
            [
                0xFA, 0x51, 0x54, 0x46, 0xF2, 0x53, 0x2F, 0x92, 0xAD, 0x8C, 0x51, 0x9E, 0xB6, 0x9A,
                0x7E, 0x20
            ]
        );
    }

    #[test]
    fn first_units_and_sizes() {
        let mut g = Synthetic::new(SyntheticParams::default()).unwrap();
        let u = g.next_unit();
        assert_eq!(
            (u.kind, u.key, u.len, u.time_ms),
            (UnitKind::Video, true, 22058, 0)
        );
        let u = g.next_unit();
        assert_eq!((u.kind, u.len), (UnitKind::Audio, 240));
        assert!(Synthetic::new(SyntheticParams {
            key_interval: 0,
            ..Default::default()
        })
        .is_err());
    }
}
