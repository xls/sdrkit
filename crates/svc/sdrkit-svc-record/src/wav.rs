//! The WAV header of 16-bit recordings, and the reader of WAV headers.
//!
//! Standard: Microsoft "Multimedia Programming Interface and Data Specifications 1.0", RIFF WAVE format.

use crate::error::FormatError;
use sdrkit_graph::stream::SampleType;

/// The size of the header that [`header`] writes.
pub(crate) const HEADER: usize = 44;
/// The most header bytes [`parse`] reads.
pub(crate) const HEAD_MAX: usize = 4096;

/// The 44-byte header of a 16-bit PCM file of `channels` at `rate` with `data` bytes of samples.
pub(crate) fn header(rate: u32, channels: u16, data: u32) -> [u8; HEADER] {
    let block = u32::from(channels) * 2;
    let mut h = [0u8; HEADER];
    let mut put = |at: usize, b: &[u8]| h[at..at + b.len()].copy_from_slice(b);
    put(0, b"RIFF");
    put(4, &36u32.saturating_add(data).to_le_bytes());
    put(8, b"WAVEfmt ");
    put(16, &16u32.to_le_bytes());
    put(20, &1u16.to_le_bytes());
    put(22, &channels.to_le_bytes());
    put(24, &rate.to_le_bytes());
    put(28, &(rate * block).to_le_bytes());
    put(32, &(block as u16).to_le_bytes());
    put(34, &16u16.to_le_bytes());
    put(36, b"data");
    put(40, &data.to_le_bytes());
    h
}

/// What a WAV header says.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct WavInfo {
    pub rate: u32,
    pub channels: u16,
    pub sample_type: SampleType,
    pub frames: u64,
}

/// Reads the header `head` of a WAV file of `size` bytes. The frame count comes from the data size, limited to the bytes that
/// the file holds.
pub(crate) fn parse(head: &[u8], size: u64) -> Result<WavInfo, FormatError> {
    let u16_at = |at: usize| {
        head.get(at..at + 2)
            .map(|b| u16::from_le_bytes([b[0], b[1]]))
    };
    let u32_at = |at: usize| {
        head.get(at..at + 4)
            .map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    };
    if head.len() < 12 {
        return Err(FormatError::Short);
    }
    if &head[0..4] != b"RIFF" || &head[8..12] != b"WAVE" {
        return Err(FormatError::NotWav);
    }
    let mut at = 12;
    let mut fmt = None;
    loop {
        let id = head.get(at..at + 4).ok_or(FormatError::Short)?;
        let len = u32_at(at + 4).ok_or(FormatError::Short)? as usize;
        let body = at + 8;
        if id == b"fmt " {
            let tag = u16_at(body).ok_or(FormatError::Short)?;
            let channels = u16_at(body + 2).ok_or(FormatError::Short)?;
            let rate = u32_at(body + 4).ok_or(FormatError::Short)?;
            let bits = u16_at(body + 14).ok_or(FormatError::Short)?;
            let sample_type = match (tag, bits) {
                (1, 16) => SampleType::I16,
                (3, 32) => SampleType::F32,
                _ => return Err(FormatError::WavEncoding { tag, bits }),
            };
            if channels == 0 {
                return Err(FormatError::NotWav);
            }
            fmt = Some((rate, channels, sample_type, u64::from(bits / 8)));
        } else if id == b"data" {
            let (rate, channels, sample_type, width) = fmt.ok_or(FormatError::NotWav)?;
            let held = size.saturating_sub(body as u64);
            let bytes = (len as u64).min(held);
            return Ok(WavInfo {
                rate,
                channels,
                sample_type,
                frames: bytes / (width * u64::from(channels)),
            });
        }
        at = body + len + (len & 1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_header_reads_back() {
        let h = header(48_000, 2, 400);
        let w = parse(&h, 44 + 400).unwrap();
        assert_eq!((w.rate, w.channels, w.frames), (48_000, 2, 100));
        assert_eq!(w.sample_type, SampleType::I16);
        let w = parse(&header(8000, 1, 0), 44 + 400).unwrap();
        assert_eq!(w.frames, 0);
        assert_eq!(parse(b"RIFX", 4), Err(FormatError::Short));
    }
}
