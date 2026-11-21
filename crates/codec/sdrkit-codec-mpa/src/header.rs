//! The MPEG audio frame header of Layers I, II and III. DAB carries Layer II frames, and transport streams carry all three layers.
//!
//! Standard: ISO/IEC 11172-3 clause 2.4.2.3, ISO/IEC 13818-3 clause 2.4.2.3 (half sampling rates).
//!
//! ```text
//! syncword (12 bits of 1, the last is the ID bit: 1 MPEG-1, 0 MPEG-2 half rates), layer (2: 11 I, 10 II, 01 III),
//! protection_bit (1: 0 means a CRC follows), bitrate_index (4), sampling_frequency (2), padding_bit (1), private_bit (1),
//! mode (2), mode_extension (2), copyright (1), original/copy (1), emphasis (2)
//! ```
//!
//! ```
//! use sdrkit_codec_mpa::Header;
//!
//! // MPEG-1 Layer III, 128 kbit/s, 44.1 kHz: a frame of 417 bytes.
//! let h = Header::parse(&[0xFF, 0xFB, 0x90, 0x64]).unwrap();
//! assert_eq!((h.layer, h.bitrate_kbps, h.sample_rate, h.frame_len()), (3, 128, 44_100, 417));
//! ```

use crate::{Error, Result};

const BITRATES: [[[u16; 15]; 3]; 2] = [
    // MPEG-2 half sampling rates (ISO/IEC 13818-3 Table B.1): Layer I, II, III.
    [
        [
            0, 32, 48, 56, 64, 80, 96, 112, 128, 144, 160, 176, 192, 224, 256,
        ],
        [0, 8, 16, 24, 32, 40, 48, 56, 64, 80, 96, 112, 128, 144, 160],
        [0, 8, 16, 24, 32, 40, 48, 56, 64, 80, 96, 112, 128, 144, 160],
    ],
    // MPEG-1 (ISO/IEC 11172-3 clause 2.4.2.3): Layer I, II, III.
    [
        [
            0, 32, 64, 96, 128, 160, 192, 224, 256, 288, 320, 352, 384, 416, 448,
        ],
        [
            0, 32, 48, 56, 64, 80, 96, 112, 128, 160, 192, 224, 256, 320, 384,
        ],
        [
            0, 32, 40, 48, 56, 64, 80, 96, 112, 128, 160, 192, 224, 256, 320,
        ],
    ],
];

/// A frame header.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Header {
    /// MPEG-1 (true) or MPEG-2 half sampling rates.
    pub mpeg1: bool,
    /// Layer 1, 2 or 3.
    pub layer: u8,
    /// A 16-bit CRC follows the header.
    pub crc: bool,
    /// Bit rate in kbit/s.
    pub bitrate_kbps: u16,
    /// Sampling rate in Hz.
    pub sample_rate: u32,
    /// Padding.
    pub padding: bool,
    /// The private bit.
    pub private: bool,
    /// Mode: 0 stereo, 1 joint stereo, 2 dual channel, 3 single channel.
    pub mode: u8,
    /// Mode extension.
    pub mode_extension: u8,
    /// Copyright.
    pub copyright: bool,
    /// original/copy: 1 when the bitstream is not a copy.
    pub original_copy: bool,
    /// Emphasis: 0 none, 1 50/15 µs, 3 CCITT J.17.
    pub emphasis: u8,
}

impl Header {
    /// Reads the four header bytes at the start of `b`.
    ///
    /// # Errors
    /// [`Error::Truncated`] for fewer than 4 bytes, [`Error::Sync`] without the syncword, [`Error::Mpeg25`] for the MPEG-2.5
    /// extension, [`Error::Layer`], [`Error::BitRate`] (free format or index 15), [`Error::SampleRate`] and [`Error::Emphasis`]
    /// for reserved codes.
    pub fn parse(b: &[u8]) -> Result<Header> {
        if b.len() < 4 {
            return Err(Error::Truncated);
        }
        if b[0] != 0xFF || b[1] & 0xE0 != 0xE0 {
            return Err(Error::Sync);
        }
        if b[1] & 0x10 == 0 {
            return Err(Error::Mpeg25);
        }
        let mpeg1 = b[1] & 0x08 != 0;
        let layer = match (b[1] >> 1) & 3 {
            3 => 1,
            2 => 2,
            1 => 3,
            _ => return Err(Error::Layer),
        };
        let bi = (b[2] >> 4) as usize;
        let si = ((b[2] >> 2) & 3) as usize;
        if bi == 0 || bi == 15 {
            return Err(Error::BitRate);
        }
        if si == 3 {
            return Err(Error::SampleRate);
        }
        let emphasis = b[3] & 3;
        if emphasis == 2 {
            return Err(Error::Emphasis);
        }
        Ok(Header {
            mpeg1,
            layer,
            crc: b[1] & 1 == 0,
            bitrate_kbps: BITRATES[mpeg1 as usize][layer as usize - 1][bi],
            sample_rate: if mpeg1 {
                [44_100, 48_000, 32_000][si]
            } else {
                [22_050, 24_000, 16_000][si]
            },
            padding: b[2] & 2 != 0,
            private: b[2] & 1 != 0,
            mode: b[3] >> 6,
            mode_extension: (b[3] >> 4) & 3,
            copyright: b[3] & 8 != 0,
            original_copy: b[3] & 4 != 0,
            emphasis,
        })
    }

    /// The four header bytes.
    ///
    /// # Errors
    /// [`Error::Layer`] for a layer other than 1 to 3, [`Error::Field`] for a mode, mode extension or emphasis above 3,
    /// [`Error::BitRate`] and [`Error::SampleRate`] for values the layer and version have no code for.
    pub fn bytes(&self) -> Result<[u8; 4]> {
        if !(1..=3).contains(&self.layer) {
            return Err(Error::Layer);
        }
        if self.mode > 3 || self.mode_extension > 3 || self.emphasis > 3 {
            return Err(Error::Field);
        }
        let bi = BITRATES[self.mpeg1 as usize][self.layer as usize - 1]
            .iter()
            .position(|&r| r == self.bitrate_kbps && r != 0)
            .ok_or(Error::BitRate)?;
        let rates: [u32; 3] = if self.mpeg1 {
            [44_100, 48_000, 32_000]
        } else {
            [22_050, 24_000, 16_000]
        };
        let si = rates
            .iter()
            .position(|&r| r == self.sample_rate)
            .ok_or(Error::SampleRate)?;
        Ok([
            0xFF,
            0xF0 | ((self.mpeg1 as u8) << 3) | ((4 - self.layer) << 1) | (!self.crc as u8),
            ((bi as u8) << 4)
                | ((si as u8) << 2)
                | ((self.padding as u8) << 1)
                | self.private as u8,
            (self.mode << 6)
                | (self.mode_extension << 4)
                | ((self.copyright as u8) << 3)
                | ((self.original_copy as u8) << 2)
                | self.emphasis,
        ])
    }

    /// The number of channels (1 in single channel mode, else 2).
    pub fn channels(&self) -> usize {
        if self.mode == 3 {
            1
        } else {
            2
        }
    }

    /// Samples per channel in one frame: 384 (Layer I), 1152 (Layer II, and Layer III of MPEG-1), 576 (Layer III at half
    /// rates).
    pub fn samples(&self) -> usize {
        match (self.layer, self.mpeg1) {
            (1, _) => 384,
            (3, false) => 576,
            _ => 1152,
        }
    }

    /// The frame length in bytes, header included. It is 0 for a sampling rate of 0.
    pub fn frame_len(&self) -> usize {
        let br = self.bitrate_kbps as usize * 1000;
        let fs = self.sample_rate as usize;
        if fs == 0 {
            return 0;
        }
        match self.layer {
            1 => (12 * br / fs + self.padding as usize) * 4,
            _ => self.samples() / 8 * br / fs + self.padding as usize,
        }
    }
}
