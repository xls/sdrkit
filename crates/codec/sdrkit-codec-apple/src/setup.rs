//! The converter setup of a stream: from the AudioSpecificConfig of the media format, or from the header of each packet (ADTS,
//! MPEG audio, AC-3, E-AC-3). AudioToolbox refuses AAC with 960-sample frames, so such a stream has no setup.
//!
//! Standard: ISO/IEC 14496-3 clause 1.6.2.1 and Table 1.18 (AudioSpecificConfig, sampling frequency index), clause 4.4.1
//! (GASpecificConfig), clause 1.A.2.2 (ADTS header); ISO/IEC 14496-1 clause 7.2.6.5 (ES_Descriptor); ISO/IEC 11172-3 clause
//! 2.4.2.3 (MPEG audio header); ATSC A/52 clauses 5.4.1 and 5.4.2 (AC-3 syncinfo and bsi); ETSI TS 102 366 clauses E.1.2.1
//! and E.1.2.2 (E-AC-3 syncinfo and bsi).

use crate::converter::Input;
use sdrkit_core::codec::CodecError;
use sdrkit_core::media::{Codec, MediaFormat, Packaging};

/// ISO/IEC 14496-3 Table 1.18.
const RATES: [u32; 13] = [
    96_000, 88_200, 64_000, 48_000, 44_100, 32_000, 24_000, 22_050, 16_000, 12_000, 11_025, 8_000,
    7_350,
];

/// The longest AudioSpecificConfig this module writes, in bytes.
const ASC_MAX: usize = 64;

/// How to decode one stream: the converter input and the AudioSpecificConfig to set (empty for MPEG audio, AC-3 and E-AC-3).
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Stream {
    pub input: Input,
    asc: [u8; ASC_MAX],
    asc_len: usize,
}

impl Stream {
    fn plain(input: Input) -> Stream {
        Stream {
            input,
            asc: [0; ASC_MAX],
            asc_len: 0,
        }
    }

    /// The magic cookie for the converter: for AAC an ES_Descriptor that holds the AudioSpecificConfig in its
    /// DecoderSpecificInfo, else empty.
    pub(crate) fn cookie(&self) -> Vec<u8> {
        if self.asc_len == 0 {
            return Vec::new();
        }
        let asc = &self.asc[..self.asc_len];
        let n = self.asc_len as u8;
        let mut v = Vec::with_capacity(self.asc_len + 32);
        // ES_Descriptor: ES_ID 0, no flags.
        v.extend_from_slice(&[0x03, 3 + 2 + 13 + 2 + n + 3, 0, 0, 0]);
        // DecoderConfigDescriptor: object type 0x40 (MPEG-4 audio), stream type 5 (audio), no buffer size or bit rates.
        v.extend_from_slice(&[
            0x04,
            13 + 2 + n,
            0x40,
            0x15,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
        ]);
        // DecoderSpecificInfo.
        v.extend_from_slice(&[0x05, n]);
        v.extend_from_slice(asc);
        // SLConfigDescriptor, predefined 2.
        v.extend_from_slice(&[0x06, 0x01, 0x02]);
        v
    }
}

/// Whether `codec` is one of the AAC family.
pub(crate) fn is_aac(codec: Codec) -> bool {
    matches!(
        codec,
        Codec::Aac | Codec::HeAac | Codec::HeAacV2 | Codec::XheAac
    )
}

/// Reads bits from the most significant bit first.
struct Bits<'a> {
    data: &'a [u8],
    at: usize,
}

impl Bits<'_> {
    fn get(&mut self, n: usize) -> Option<u32> {
        let mut v = 0u32;
        for _ in 0..n {
            let byte = *self.data.get(self.at / 8)?;
            v = (v << 1) | u32::from((byte >> (7 - self.at % 8)) & 1);
            self.at += 1;
        }
        Some(v)
    }
    fn object_type(&mut self) -> Option<u32> {
        match self.get(5)? {
            31 => Some(32 + self.get(6)?),
            t => Some(t),
        }
    }
    fn rate(&mut self) -> Option<u32> {
        match self.get(4)? {
            15 => self.get(24),
            i => RATES.get(i as usize).copied(),
        }
    }
}

/// Writes bits from the most significant bit first into a fixed buffer.
struct Writer {
    data: [u8; ASC_MAX],
    at: usize,
}

impl Writer {
    fn put(&mut self, v: u32, n: usize) {
        for k in (0..n).rev() {
            if (v >> k) & 1 == 1 {
                self.data[self.at / 8] |= 0x80 >> (self.at % 8);
            }
            self.at += 1;
        }
    }
    fn rate(&mut self, rate: u32) {
        match RATES.iter().position(|&r| r == rate) {
            Some(i) => self.put(i as u32, 4),
            None => {
                self.put(15, 4);
                self.put(rate, 24);
            }
        }
    }
}

/// An AAC stream as the converter decodes it.
#[derive(Clone, Copy)]
struct Aac {
    core_rate: u32,
    config: u32,
    channels: u32,
    sbr: bool,
    ps: bool,
}

impl Aac {
    /// The stream with an explicit AudioSpecificConfig of AAC-LC (AOT 5 or 29 around it for SBR and PS) and 1024-sample
    /// frames.
    fn stream(self) -> Stream {
        let mut w = Writer {
            data: [0; ASC_MAX],
            at: 0,
        };
        let rate = if self.sbr {
            2 * self.core_rate
        } else {
            self.core_rate
        };
        if self.sbr {
            w.put(if self.ps { 29 } else { 5 }, 5);
            w.rate(self.core_rate);
            w.put(self.config, 4);
            w.rate(rate);
            w.put(2, 5);
        } else {
            w.put(2, 5);
            w.rate(self.core_rate);
            w.put(self.config, 4);
        }
        // GASpecificConfig: frameLengthFlag 0, dependsOnCoreCoder 0, extensionFlag 0.
        w.put(0, 3);
        let codec = match (self.sbr, self.ps) {
            (false, _) => Codec::Aac,
            (true, false) => Codec::HeAac,
            (true, true) => Codec::HeAacV2,
        };
        let channels = if self.ps { 2 } else { self.channels };
        let frames = if self.sbr { 2048 } else { 1024 };
        Stream {
            input: Input {
                format_id: codec.fourcc().to_u32(),
                rate: f64::from(rate),
                channels,
                frames_per_packet: frames,
            },
            asc: w.data,
            asc_len: w.at.div_ceil(8),
        }
    }
}

/// The stream for an AAC stream with the AudioSpecificConfig `asc`. If `asc` signals no SBR but `format.codec` is HE-AAC,
/// SBR is implicit at twice the core rate. Returns `None` for a configuration it cannot read, an audio object type other
/// than AAC-LC (2) and USAC (42), or 960-sample frames.
fn from_asc(asc: &[u8], format: &MediaFormat) -> Option<Stream> {
    let mut b = Bits { data: asc, at: 0 };
    let mut aot = b.object_type()?;
    let core_rate = b.rate()?;
    let config = b.get(4)?;
    let channels = match config {
        1..=6 => config,
        7 => 8,
        _ => return None,
    };
    let (mut sbr, mut ps) = (false, false);
    if aot == 5 || aot == 29 {
        sbr = true;
        ps = aot == 29;
        b.rate()?;
        aot = b.object_type()?;
    }
    if aot == 42 {
        let a = format.audio.as_ref()?;
        let mut s = Stream::plain(Input {
            format_id: Codec::XheAac.fourcc().to_u32(),
            rate: f64::from(a.sample_rate),
            channels: u32::from(a.channels),
            frames_per_packet: 0,
        });
        s.asc_len = asc.len().min(ASC_MAX);
        s.asc[..s.asc_len].copy_from_slice(&asc[..s.asc_len]);
        return (s.asc_len == asc.len()).then_some(s);
    }
    if aot != 2 {
        return None;
    }
    if b.get(1)? == 1 {
        return None;
    }
    if !sbr && matches!(format.codec, Codec::HeAac | Codec::HeAacV2) {
        sbr = true;
        ps = format.codec == Codec::HeAacV2;
    }
    Some(
        Aac {
            core_rate,
            config,
            channels,
            sbr,
            ps,
        }
        .stream(),
    )
}

/// The stream of bare access units with the AudioSpecificConfig in `format.config`.
pub(crate) fn from_config(format: &MediaFormat) -> Option<Stream> {
    from_asc(&format.config, format)
}

/// The stream for one packet of a stream that carries its configuration in band, and the bytes to decode.
///
/// # Errors
/// [`CodecError::Frame`] if the header is damaged or describes a packet this crate does not decode (ADTS with several raw
/// data blocks).
pub(crate) fn from_frame<'a>(
    format: &MediaFormat,
    frame: &'a [u8],
) -> Result<(Stream, &'a [u8]), CodecError> {
    match (format.codec, format.packaging) {
        (Codec::Aac | Codec::HeAac | Codec::HeAacV2, Packaging::Adts) => adts(format, frame),
        (Codec::Ac3, _) => ac3(frame).map(|i| (Stream::plain(i), frame)),
        (Codec::EAc3, _) => eac3(frame).map(|i| (Stream::plain(i), frame)),
        _ => mpeg(frame).map(|i| (Stream::plain(i), frame)),
    }
}

fn adts<'a>(format: &MediaFormat, f: &'a [u8]) -> Result<(Stream, &'a [u8]), CodecError> {
    if f.len() < 7 || f[0] != 0xFF || f[1] & 0xF0 != 0xF0 {
        return Err(CodecError::Frame);
    }
    let header = if f[1] & 1 == 1 { 7 } else { 9 };
    let profile = f[2] >> 6;
    let index = (f[2] >> 2) & 0x0F;
    let config = ((f[2] & 1) << 2) | (f[3] >> 6);
    let length = (usize::from(f[3] & 3) << 11) | (usize::from(f[4]) << 3) | usize::from(f[5] >> 5);
    if f[6] & 3 != 0 || length < header || length > f.len() {
        return Err(CodecError::Frame);
    }
    let asc = [
        ((profile + 1) << 3) | (index >> 1),
        ((index & 1) << 7) | (config << 3),
    ];
    let stream = from_asc(&asc, format).ok_or(CodecError::Frame)?;
    Ok((stream, &f[header..length]))
}

fn mpeg(f: &[u8]) -> Result<Input, CodecError> {
    let h = sdrkit_codec_mpa::Header::parse(f).map_err(|_| CodecError::Frame)?;
    let codec = match h.layer {
        1 => Codec::Mp1,
        2 => Codec::Mp2,
        _ => Codec::Mp3,
    };
    Ok(Input {
        format_id: codec.fourcc().to_u32(),
        rate: f64::from(h.sample_rate),
        channels: h.channels() as u32,
        frames_per_packet: h.samples() as u32,
    })
}

fn ac3(f: &[u8]) -> Result<Input, CodecError> {
    if f.len() < 8 || f[0] != 0x0B || f[1] != 0x77 {
        return Err(CodecError::Frame);
    }
    let rate = match f[4] >> 6 {
        0 => 48_000,
        1 => 44_100,
        2 => 32_000,
        _ => return Err(CodecError::Frame),
    };
    // bsi starts at byte 5: bsid (5 bits), bsmod (3), acmod (3), then the mix levels that acmod calls for, then lfeon.
    let mut b = Bits { data: f, at: 48 };
    let acmod = b.get(3).ok_or(CodecError::Frame)?;
    let mut skip = 0;
    if acmod & 1 == 1 && acmod != 1 {
        skip += 2;
    }
    if acmod & 4 == 4 {
        skip += 2;
    }
    if acmod == 2 {
        skip += 2;
    }
    b.get(skip).ok_or(CodecError::Frame)?;
    let lfe = b.get(1).ok_or(CodecError::Frame)?;
    let channels = [2, 1, 2, 3, 3, 4, 4, 5][acmod as usize] + lfe;
    Ok(Input {
        format_id: Codec::Ac3.fourcc().to_u32(),
        rate: f64::from(rate),
        channels,
        frames_per_packet: 1536,
    })
}

/// The input for an E-AC-3 access unit `f`, from the header of its first syncframe, which must be independent substream 0
/// (ETSI TS 102 366 E.1.2.2: strmtyp 0 or 2, substreamid 0). The channels are those of that substream; the channels that
/// dependent substreams add are not counted. An access unit holds 6 audio blocks, 1536 samples.
fn eac3(f: &[u8]) -> Result<Input, CodecError> {
    if f.len() < 6 || f[0] != 0x0B || f[1] != 0x77 {
        return Err(CodecError::Frame);
    }
    let mut b = Bits { data: f, at: 16 };
    let mut get = |n| b.get(n).ok_or(CodecError::Frame);
    let strmtyp = get(2)?;
    let substreamid = get(3)?;
    get(11)?; // frmsiz
    let fscod = get(2)?;
    let fscod2 = get(2)?;
    let acmod = get(3)?;
    let lfe = get(1)?;
    let bsid = get(5)?;
    if strmtyp == 1 || strmtyp == 3 || substreamid != 0 || !(11..=16).contains(&bsid) {
        return Err(CodecError::Frame);
    }
    let rate = match (fscod, fscod2) {
        (0, _) => 48_000,
        (1, _) => 44_100,
        (2, _) => 32_000,
        (_, 0) => 24_000,
        (_, 1) => 22_050,
        (_, 2) => 16_000,
        _ => return Err(CodecError::Frame),
    };
    Ok(Input {
        format_id: Codec::EAc3.fourcc().to_u32(),
        rate: f64::from(rate),
        channels: [2, 1, 2, 3, 3, 4, 4, 5][acmod as usize] + lfe,
        frames_per_packet: 1536,
    })
}

/// The downmix gains of the AC-3 frame `f`, centre then surround (ATSC A/52 5.4.2.4, 5.4.2.5, tables 5.9 and 5.10). A
/// reserved code takes the middle gain, as the standard says. `None` for a frame too short to hold them.
pub(crate) fn ac3_mix_levels(f: &[u8]) -> Option<(f32, f32)> {
    let mut b = Bits { data: f, at: 48 };
    let acmod = b.get(3)?;
    let centre = if acmod & 1 == 1 && acmod != 1 {
        [0.707, 0.595, 0.5, 0.595][b.get(2)? as usize]
    } else {
        0.707
    };
    let surround = if acmod & 4 == 4 {
        [0.707, 0.5, 0.0, 0.5][b.get(2)? as usize]
    } else {
        0.707
    };
    Some((centre, surround))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn implicit_sbr_becomes_explicit_and_960_sample_frames_are_refused() {
        let f = MediaFormat::new(Codec::HeAac, Packaging::Raw);
        let s = from_asc(&[0x13, 0x10], &f).unwrap();
        assert_eq!(s.input.format_id, Codec::HeAac.fourcc().to_u32());
        assert_eq!((s.input.rate, s.input.channels), (48_000.0, 2));
        assert_eq!(&s.asc[..s.asc_len], &[0x2B, 0x11, 0x88, 0x00]);
        // A DAB+ configuration: 960-sample frames.
        assert!(from_asc(&[0x2B, 0x11, 0x8A, 0x00], &f).is_none());
        let f = MediaFormat::new(Codec::Aac, Packaging::Raw);
        assert!(from_asc(&[0x11, 0x94], &f).is_none());
        let s = from_asc(&[0x11, 0x90], &f).unwrap();
        assert_eq!(&s.asc[..s.asc_len], &[0x11, 0x90]);
    }
}
