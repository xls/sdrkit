//! MPEG-4 audio packaging: the AudioSpecificConfig, LATM with LOAS, and ADTS, for a host decoder of DAB+ and DRM audio.
//!
//! Standard: ISO/IEC 14496-3 clauses 1.6.2.1, 1.7.2, 1.7.3 and 1.A.2, ETSI TS 102 563.

use crate::{Error, Result};
use crate::{Malformed, Parameter};
use sdrkit_util_bits::{BitReader, BitWriter};

/// The sampling frequencies by samplingFrequencyIndex (ISO/IEC 14496-3 Table 1.18). Index 15 means an explicit 24-bit rate.
pub const SAMPLING_FREQUENCIES: [u32; 13] = [
    96_000, 88_200, 64_000, 48_000, 44_100, 32_000, 24_000, 22_050, 16_000, 12_000, 11_025, 8_000,
    7_350,
];

/// Object type 2: AAC LC.
pub const AOT_AAC_LC: u8 = 2;
/// Object type 5: SBR.
pub const AOT_SBR: u8 = 5;
/// Object type 29: PS.
pub const AOT_PS: u8 = 29;
/// Object type 42: USAC (xHE-AAC).
pub const AOT_USAC: u8 = 42;

/// An AudioSpecificConfig.
///
/// ```
/// use sdrkit_proto_bcdata::aac::AudioSpecificConfig;
///
/// // AAC-LC, 48 kHz, stereo, 1024-sample frames: the well-known 0x11 0x90.
/// let mut b = Vec::new();
/// AudioSpecificConfig::lc(48_000, 2).write(&mut b).unwrap();
/// assert_eq!(b, [0x11, 0x90]);
/// ```
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AudioSpecificConfig {
    /// The core object type (2 for AAC LC).
    pub object_type: u8,
    /// The core sampling rate in Hz.
    pub sample_rate: u32,
    /// channelConfiguration (1 mono, 2 stereo, 1 with parametric stereo).
    pub channels: u8,
    /// With SBR: the output sampling rate (explicit hierarchical signalling).
    pub sbr_rate: Option<u32>,
    /// Parametric stereo (needs SBR).
    pub ps: bool,
    /// 960-sample frames (frameLengthFlag), as DAB+ and DRM use.
    pub frame_960: bool,
}

impl AudioSpecificConfig {
    /// AAC LC with 1024-sample frames.
    pub const fn lc(sample_rate: u32, channels: u8) -> Self {
        AudioSpecificConfig {
            object_type: AOT_AAC_LC,
            sample_rate,
            channels,
            sbr_rate: None,
            ps: false,
            frame_960: false,
        }
    }

    /// The configuration of a DAB+ superframe (TS 102 563 clause 5.2: dac_rate, sbr_flag, aac_channel_mode, ps_flag). The core
    /// rate is 48 or 32 kHz, halved with SBR. With PS the core is mono.
    pub fn dab_plus(dac_48k: bool, sbr: bool, stereo: bool, ps: bool) -> Result<Self> {
        if ps && !sbr {
            return Err(Error::Invalid(Malformed::PsWithoutSbr));
        }
        let out = if dac_48k { 48_000 } else { 32_000 };
        Ok(AudioSpecificConfig {
            object_type: AOT_AAC_LC,
            sample_rate: if sbr { out / 2 } else { out },
            channels: if stereo && !ps { 2 } else { 1 },
            sbr_rate: sbr.then_some(out),
            ps,
            frame_960: true,
        })
    }

    /// Writes the configuration into a bit writer (not padded). It returns an error for a rate of 0 or above 2^24 − 1, a
    /// channel configuration above 15, or PS without SBR.
    pub fn write_bits(&self, w: &mut BitWriter<'_>) -> Result<()> {
        if self.ps && self.sbr_rate.is_none() {
            return Err(Error::BadParameter(Parameter::PsNeedsSbr));
        }
        if self.channels > 15 || self.object_type == 0 {
            return Err(Error::BadParameter(Parameter::ChannelConfig));
        }
        let aot = |w: &mut BitWriter<'_>, t: u8| {
            if t >= 31 {
                w.put(31, 5);
                w.put((t - 32) as u64, 6);
            } else {
                w.put(t as u64, 5);
            }
        };
        let rate = |w: &mut BitWriter<'_>, r: u32| -> Result<()> {
            match SAMPLING_FREQUENCIES.iter().position(|&f| f == r) {
                Some(i) => w.put(i as u64, 4),
                None if r > 0 && r < 1 << 24 => {
                    w.put(15, 4);
                    w.put(r as u64, 24);
                }
                None => return Err(Error::BadParameter(Parameter::SamplingRate)),
            }
            Ok(())
        };
        match self.sbr_rate {
            Some(out) => {
                aot(w, if self.ps { AOT_PS } else { AOT_SBR });
                rate(w, self.sample_rate)?;
                w.put(self.channels as u64, 4);
                rate(w, out)?;
                aot(w, self.object_type);
            }
            None => {
                aot(w, self.object_type);
                rate(w, self.sample_rate)?;
                w.put(self.channels as u64, 4);
            }
        }
        if matches!(self.object_type, 1..=4 | 6 | 7 | 17 | 19..=23) {
            w.put(self.frame_960 as u64, 1);
            w.put(0, 1);
            w.put(0, 1);
        }
        Ok(())
    }

    /// Appends the configuration as bytes (padded with zero bits), as an MP4 or Matroska decoder configuration holds it.
    pub fn write(&self, out: &mut Vec<u8>) -> Result<()> {
        let mut w = BitWriter::new(out);
        self.write_bits(&mut w)?;
        w.align();
        Ok(())
    }

    /// Reads a configuration with explicit or no SBR signalling and a GASpecificConfig whose extensionFlag is 0.
    pub fn parse(b: &[u8]) -> Result<Self> {
        let mut r = BitReader::new(b);
        let aot = |r: &mut BitReader<'_>| {
            let t = r.u(5) as u8;
            if t == 31 {
                32 + r.u(6) as u8
            } else {
                t
            }
        };
        let rate = |r: &mut BitReader<'_>| -> Result<u32> {
            let i = r.u(4) as usize;
            match i {
                15 => Ok(r.u(24) as u32),
                i => SAMPLING_FREQUENCIES
                    .get(i)
                    .copied()
                    .ok_or(Error::Invalid(Malformed::SamplingFrequencyIndex)),
            }
        };
        let mut t = aot(&mut r);
        let sample_rate = rate(&mut r)?;
        let channels = r.u(4) as u8;
        let (mut sbr_rate, mut ps) = (None, false);
        if t == AOT_SBR || t == AOT_PS {
            ps = t == AOT_PS;
            sbr_rate = Some(rate(&mut r)?);
            t = aot(&mut r);
        }
        let mut frame_960 = false;
        if matches!(t, 1..=4 | 6 | 7 | 17 | 19..=23) {
            frame_960 = r.bit();
            if r.bit() {
                r.u(14);
            }
            if r.bit() {
                return Err(Error::Invalid(Malformed::GaExtensionFlag));
            }
        }
        if r.overrun() {
            return Err(Error::Truncated);
        }
        Ok(AudioSpecificConfig {
            object_type: t,
            sample_rate,
            channels,
            sbr_rate,
            ps,
            frame_960,
        })
    }

    /// The output samples of one access unit (per channel).
    pub fn samples_per_unit(&self) -> u32 {
        let core = if self.frame_960 { 960 } else { 1024 };
        if self.sbr_rate.is_some() {
            2 * core
        } else {
            core
        }
    }
}

/// Appends a LATM AudioMuxElement with muxConfigPresent = 1 (ISO/IEC 14496-3 clause 1.7.3), byte aligned. With
/// `with_config` it carries a StreamMuxConfig (audioMuxVersion 0, one program, one layer, frameLengthType 0). Without it, the element sets
/// useSameStreamMux and the decoder uses the last one.
pub fn write_latm(
    asc: &AudioSpecificConfig,
    with_config: bool,
    au: &[u8],
    out: &mut Vec<u8>,
) -> Result<()> {
    let start = out.len();
    let mut w = BitWriter::new(out);
    w.put(!with_config as u64, 1); // useSameStreamMux
    if with_config {
        w.put(0, 1); // audioMuxVersion
        w.put(1, 1); // allStreamsSameTimeFraming
        w.put(0, 6); // numSubFrames
        w.put(0, 4); // numProgram
        w.put(0, 3); // numLayer
        if let Err(e) = asc.write_bits(&mut w) {
            out.truncate(start);
            return Err(e);
        }
        w.put(0, 3); // frameLengthType
        w.put(0xFF, 8); // latmBufferFullness
        w.put(0, 1); // otherDataPresent
        w.put(0, 1); // crcCheckPresent
    }
    // PayloadLengthInfo: MuxSlotLengthBytes in 255 steps.
    let mut n = au.len();
    while n >= 255 {
        w.put(255, 8);
        n -= 255;
    }
    w.put(n as u64, 8);
    w.bytes(au);
    w.align();
    Ok(())
}

/// Appends one LOAS AudioSyncStream frame (clause 1.7.2) around a LATM AudioMuxElement.
pub fn write_loas(
    asc: &AudioSpecificConfig,
    with_config: bool,
    au: &[u8],
    out: &mut Vec<u8>,
) -> Result<()> {
    let start = out.len();
    out.extend_from_slice(&[0, 0, 0]);
    write_latm(asc, with_config, au, out)?;
    let len = out.len() - start - 3;
    if len > 0x1FFF {
        out.truncate(start);
        return Err(Error::BadParameter(Parameter::LoasFrameLength));
    }
    out[start] = 0x56;
    out[start + 1] = 0xE0 | (len >> 8) as u8;
    out[start + 2] = len as u8;
    Ok(())
}

/// Appends a 7-byte ADTS header (MPEG-4, no CRC) for a raw data block of `payload_len` bytes. SBR is left implicit (the
/// header provides the core). It returns an error for 960-sample frames, which ADTS cannot carry, and for object types other than
/// 1 to 4.
pub fn write_adts_header(
    asc: &AudioSpecificConfig,
    payload_len: usize,
    out: &mut Vec<u8>,
) -> Result<()> {
    if asc.frame_960 {
        return Err(Error::BadParameter(Parameter::AdtsFrame960));
    }
    if !(1..=4).contains(&asc.object_type) || asc.channels > 7 {
        return Err(Error::BadParameter(Parameter::AdtsObjectType));
    }
    let sfi = SAMPLING_FREQUENCIES
        .iter()
        .position(|&f| f == asc.sample_rate)
        .ok_or(Error::BadParameter(Parameter::AdtsRate))?;
    let len = payload_len + 7;
    if len > 0x1FFF {
        return Err(Error::BadParameter(Parameter::AdtsFrameLength));
    }
    let mut w = BitWriter::new(out);
    w.put(0xFFF, 12);
    w.put(0, 1); // ID: MPEG-4
    w.put(0, 2); // layer
    w.put(1, 1); // protection_absent
    w.put((asc.object_type - 1) as u64, 2);
    w.put(sfi as u64, 4);
    w.put(0, 1); // private_bit
    w.put(asc.channels as u64, 3);
    w.put(0, 4); // original_copy, home, copyright_identification_bit, copyright_identification_start
    w.put(len as u64, 13);
    w.put(0x7FF, 11); // adts_buffer_fullness: variable rate
    w.put(0, 2); // number_of_raw_data_blocks_in_frame − 1
    Ok(())
}
