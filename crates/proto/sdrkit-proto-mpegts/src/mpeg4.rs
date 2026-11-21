//! MPEG-4 systems in a transport stream: object descriptors, ES descriptors and SL packets, as DMB video services carry them.
//! Read and write forms of each structure, an SL packet header reader that allocates nothing, and an access unit assembler.
//!
//! Standard: ISO/IEC 14496-1:2010 clauses 7.2 (object descriptors), 7.3.2 (sync layer) and 8.3.3 (sizes), ISO/IEC 13818-1
//! 2.6.40 to 2.6.44 (IOD, SL and FMC descriptors) and 2.4.4 (ISO_IEC_14496_section), ETSI TS 102 428 (the DMB video service).

use crate::error::{Error, Mpeg4Fault, Mpeg4Field};
use sdrkit_util_bits::{BitReader, BitWriter};

/// Descriptor tags (ISO/IEC 14496-1 7.2.2.1, Table 1).
pub mod tag {
    /// ObjectDescrTag.
    pub const OBJECT_DESCRIPTOR: u8 = 0x01;
    /// InitialObjectDescrTag.
    pub const INITIAL_OBJECT_DESCRIPTOR: u8 = 0x02;
    /// ES_DescrTag.
    pub const ES_DESCRIPTOR: u8 = 0x03;
    /// DecoderConfigDescrTag.
    pub const DECODER_CONFIG: u8 = 0x04;
    /// DecSpecificInfoTag.
    pub const DECODER_SPECIFIC_INFO: u8 = 0x05;
    /// SLConfigDescrTag.
    pub const SL_CONFIG: u8 = 0x06;
    /// LanguageDescrTag.
    pub const LANGUAGE: u8 = 0x43;
}

/// Command tags of the object descriptor stream (ISO/IEC 14496-1 7.2.2.1, Table 2).
pub mod command_tag {
    /// ObjectDescrUpdateTag.
    pub const OD_UPDATE: u8 = 0x01;
    /// ObjectDescrRemoveTag.
    pub const OD_REMOVE: u8 = 0x02;
}

/// Values of streamType (ISO/IEC 14496-1 7.2.6.6.2).
pub mod stream_type {
    /// ObjectDescriptorStream.
    pub const OBJECT_DESCRIPTOR: u8 = 0x01;
    /// ClockReferenceStream.
    pub const CLOCK_REFERENCE: u8 = 0x02;
    /// SceneDescriptionStream (BIFS).
    pub const SCENE_DESCRIPTION: u8 = 0x03;
    /// VisualStream.
    pub const VISUAL: u8 = 0x04;
    /// AudioStream.
    pub const AUDIO: u8 = 0x05;
}

/// Values of objectTypeIndication (ISO/IEC 14496-1 7.2.6.6.2) that DMB and MPEG-4 systems use.
pub mod object_type {
    /// Systems ISO/IEC 14496-1 (scene description and object descriptor streams).
    pub const SYSTEMS: u8 = 0x01;
    /// Systems ISO/IEC 14496-1, version 2.
    pub const SYSTEMS_V2: u8 = 0x02;
    /// Visual ISO/IEC 14496-2.
    pub const VISUAL_14496_2: u8 = 0x20;
    /// Visual ITU-T H.264 | ISO/IEC 14496-10.
    pub const AVC: u8 = 0x21;
    /// Audio ISO/IEC 14496-3: AAC, HE-AAC or BSAC, as the AudioSpecificConfig says.
    pub const AUDIO_14496_3: u8 = 0x40;
    /// No object type specified.
    pub const NONE: u8 = 0xFF;
}

/// The 13818-1 descriptor that holds the initial object descriptor (ISO/IEC 13818-1 2.6.40).
pub const IOD_DESCRIPTOR_TAG: u8 = 0x1D;
/// The 13818-1 descriptor that carries a PID's ES_ID (ISO/IEC 13818-1 2.6.42).
pub const SL_DESCRIPTOR_TAG: u8 = 0x1E;
/// The 13818-1 descriptor that carries the ES_ID of each FlexMux channel (ISO/IEC 13818-1 2.6.44).
pub const FMC_DESCRIPTOR_TAG: u8 = 0x1F;
/// The stream_type of SL packets (or FlexMux) in PES packets (ISO/IEC 13818-1 Table 2-34).
pub const STREAM_TYPE_SL_PES: u8 = 0x12;
/// The stream_type of SL packets (or FlexMux) in 14496 sections (ISO/IEC 13818-1 Table 2-34).
pub const STREAM_TYPE_SL_SECTIONS: u8 = 0x13;
/// The PES stream_id of an SL-packetized stream (ISO/IEC 13818-1 Table 2-22).
pub const SL_PES_STREAM_ID: u8 = 0xFA;
/// The table_id of a 14496 scene description section (ISO/IEC 13818-1 Table 2-31).
pub const TABLE_SCENE_DESCRIPTION: u8 = 0x04;
/// The table_id of a 14496 object descriptor section (ISO/IEC 13818-1 Table 2-31).
pub const TABLE_OBJECT_DESCRIPTOR: u8 = 0x05;

type Result<T> = std::result::Result<T, Error>;

// ---------------------------------------------------------------------------------------------------------------------------------
// Expandable sizes and descriptor loops.

/// Reads a sizeOfInstance field at `b[*at..]` and moves `*at` past it. Each byte carries 7 bits, and its top bit says that
/// another byte follows. The field has one to four bytes (ISO/IEC 14496-1 8.3.3). A non-minimal form (0x80 0x80 0x80 0x05)
/// is allowed.
///
/// # Errors
/// [`Mpeg4Fault::Short`] when the data ends inside the field, [`Mpeg4Fault::Size`] when a fourth byte still says "more".
pub fn read_size(b: &[u8], at: &mut usize) -> Result<usize> {
    let mut n = 0usize;
    for _ in 0..4 {
        let v = *b.get(*at).ok_or(Error::Mpeg4(Mpeg4Fault::Short))?;
        *at += 1;
        n = (n << 7) | (v & 0x7F) as usize;
        if v & 0x80 == 0 {
            return Ok(n);
        }
    }
    Err(Error::Mpeg4(Mpeg4Fault::Size))
}

/// Appends `n` as a sizeOfInstance field in the fewest bytes (ISO/IEC 14496-1 8.3.3).
///
/// # Errors
/// [`Mpeg4Fault::Size`] when `n` needs more than 28 bits.
pub fn write_size(out: &mut Vec<u8>, n: usize) -> Result<()> {
    if n >= 1 << 28 {
        return Err(Error::Mpeg4(Mpeg4Fault::Size));
    }
    let mut k = 1;
    while k < 4 && n >> (7 * k) != 0 {
        k += 1;
    }
    for i in (0..k).rev() {
        let more = if i > 0 { 0x80 } else { 0 };
        out.push(more | ((n >> (7 * i)) & 0x7F) as u8);
    }
    Ok(())
}

/// Appends a descriptor: `tag`, the size of `body`, then `body`.
///
/// # Errors
/// [`Mpeg4Fault::Size`] when `body` is longer than a size field can say.
pub fn write_descriptor(out: &mut Vec<u8>, tag: u8, body: &[u8]) -> Result<()> {
    out.push(tag);
    write_size(out, body.len())?;
    out.extend_from_slice(body);
    Ok(())
}

/// The descriptors one after another in `b`, as `(tag, body)`. It allocates nothing. After an error it returns nothing more.
#[derive(Clone, Debug)]
pub struct Descriptors<'a> {
    rest: &'a [u8],
}

/// The descriptors in `b` (a descriptor loop of ISO/IEC 14496-1, with expandable sizes).
pub fn descriptors(b: &[u8]) -> Descriptors<'_> {
    Descriptors { rest: b }
}

impl<'a> Iterator for Descriptors<'a> {
    type Item = Result<(u8, &'a [u8])>;

    fn next(&mut self) -> Option<Self::Item> {
        let (&tag, _) = self.rest.split_first()?;
        let mut at = 1;
        let r = read_size(self.rest, &mut at).and_then(|n| {
            let body = self
                .rest
                .get(at..at + n)
                .ok_or(Error::Mpeg4(Mpeg4Fault::Short))?;
            Ok((tag, body, at + n))
        });
        match r {
            Ok((tag, body, end)) => {
                self.rest = &self.rest[end..];
                Some(Ok((tag, body)))
            }
            Err(e) => {
                self.rest = &[];
                Some(Err(e))
            }
        }
    }
}

/// The body of the one descriptor at the start of `b`, which must have tag `expected`.
fn expect(b: &[u8], expected: u8) -> Result<&[u8]> {
    let (tag, body) = descriptors(b)
        .next()
        .ok_or(Error::Mpeg4(Mpeg4Fault::Short))??;
    if tag != expected {
        return Err(Error::Mpeg4(Mpeg4Fault::Tag {
            expected,
            found: tag,
        }));
    }
    Ok(body)
}

fn finish(r: &BitReader<'_>) -> Result<()> {
    if r.overrun() {
        Err(Error::Mpeg4(Mpeg4Fault::Short))
    } else {
        Ok(())
    }
}

// ---------------------------------------------------------------------------------------------------------------------------------
// SLConfigDescriptor.

/// The optional duration fields of an SL configuration (durationFlag set).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SlDuration {
    /// timeScale: ticks per second of the two durations.
    pub time_scale: u32,
    /// accessUnitDuration, in ticks of `time_scale`.
    pub au_duration: u16,
    /// compositionUnitDuration, in ticks of `time_scale`.
    pub cu_duration: u16,
}

/// An SL packet header configuration: which fields the SL packet headers of one elementary stream hold
/// (ISO/IEC 14496-1 7.3.2.3, SLConfigDescriptor). [`Default`] is a custom configuration (predefined 0) with no fields.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SlConfig {
    /// predefined: 0 custom, 1 null SL packet header.
    pub predefined: u8,
    /// useAccessUnitStartFlag.
    pub use_au_start: bool,
    /// useAccessUnitEndFlag.
    pub use_au_end: bool,
    /// useRandomAccessPointFlag.
    pub use_rap: bool,
    /// hasRandomAccessUnitsOnlyFlag: every access unit is a random access point.
    pub random_access_units_only: bool,
    /// usePaddingFlag.
    pub use_padding: bool,
    /// useTimeStampsFlag: the headers carry decoding and composition time stamps.
    pub use_timestamps: bool,
    /// useIdleFlag.
    pub use_idle: bool,
    /// timeStampResolution, ticks per second.
    pub ts_resolution: u32,
    /// OCRResolution, ticks per second.
    pub ocr_resolution: u32,
    /// timeStampLength in bits (at most 64).
    pub ts_len: u8,
    /// OCRLength in bits (at most 64).
    pub ocr_len: u8,
    /// AU_Length in bits (at most 32).
    pub au_len: u8,
    /// instantBitrateLength in bits.
    pub instant_bitrate_len: u8,
    /// degradationPriorityLength in bits (at most 15).
    pub degradation_priority_len: u8,
    /// AU_seqNumLength in bits (at most 16).
    pub au_seq_len: u8,
    /// packetSeqNumLength in bits (at most 16).
    pub packet_seq_len: u8,
    /// The duration fields, when durationFlag is set.
    pub duration: Option<SlDuration>,
    /// startDecodingTimeStamp, sent when the headers carry no time stamps.
    pub start_dts: Option<u64>,
    /// startCompositionTimeStamp, sent when the headers carry no time stamps.
    pub start_cts: Option<u64>,
}

impl SlConfig {
    /// The null SL packet header (predefined 1): the headers are empty, and each SL packet is one whole access unit.
    /// The time stamp resolution is 1000 and the time stamp length 32 (ISO/IEC 14496-1 7.3.2.3.2).
    pub fn null_header() -> SlConfig {
        SlConfig {
            predefined: 1,
            ts_resolution: 1000,
            ts_len: 32,
            ..SlConfig::default()
        }
    }

    /// Parses the body of an SLConfigDescriptor (after its tag and size). The start time stamps are read when the headers
    /// carry no time stamps and the body holds them.
    ///
    /// # Errors
    /// [`Mpeg4Fault::Short`] when the body is short, [`Mpeg4Fault::Predefined`] for predefined 2 to 255, and
    /// [`Mpeg4Fault::Field`] when a length is larger than 7.3.2.3.2 allows.
    pub fn parse(body: &[u8]) -> Result<SlConfig> {
        let mut r = BitReader::new(body);
        let predefined = r.u(8) as u8;
        let mut c = match predefined {
            0 => SlConfig::default(),
            1 => SlConfig::null_header(),
            v => return Err(Error::Mpeg4(Mpeg4Fault::Predefined(v))),
        };
        let mut has_duration = false;
        if predefined == 0 {
            c.use_au_start = r.bit();
            c.use_au_end = r.bit();
            c.use_rap = r.bit();
            c.random_access_units_only = r.bit();
            c.use_padding = r.bit();
            c.use_timestamps = r.bit();
            c.use_idle = r.bit();
            has_duration = r.bit();
            c.ts_resolution = r.u(32) as u32;
            c.ocr_resolution = r.u(32) as u32;
            c.ts_len = r.u(8) as u8;
            c.ocr_len = r.u(8) as u8;
            c.au_len = r.u(8) as u8;
            c.instant_bitrate_len = r.u(8) as u8;
            c.degradation_priority_len = r.u(4) as u8;
            c.au_seq_len = r.u(5) as u8;
            c.packet_seq_len = r.u(5) as u8;
            r.skip(2);
            finish(&r)?;
            for (len, max, name) in [
                (c.ts_len, 64, Mpeg4Field::TimeStampLength),
                (c.ocr_len, 64, Mpeg4Field::OcrLength),
                (c.au_len, 32, Mpeg4Field::AuLength),
                (c.au_seq_len, 16, Mpeg4Field::AuSeqNumLength),
                (c.packet_seq_len, 16, Mpeg4Field::PacketSeqNumLength),
            ] {
                if len > max {
                    return Err(Error::Mpeg4(Mpeg4Fault::Field(name)));
                }
            }
        }
        if has_duration {
            c.duration = Some(SlDuration {
                time_scale: r.u(32) as u32,
                au_duration: r.u(16) as u16,
                cu_duration: r.u(16) as u16,
            });
            finish(&r)?;
        }
        if !c.use_timestamps && c.ts_len > 0 && r.left() >= 2 * c.ts_len as usize {
            c.start_dts = Some(r.u(c.ts_len as u32));
            c.start_cts = Some(r.u(c.ts_len as u32));
        }
        Ok(c)
    }

    /// Appends this configuration as a whole SLConfigDescriptor (tag, size, body). The reserved bits are ones.
    ///
    /// # Errors
    /// [`Mpeg4Fault::Predefined`] for a predefined value other than 0 or 1.
    pub fn write(&self, out: &mut Vec<u8>) -> Result<()> {
        if self.predefined > 1 {
            return Err(Error::Mpeg4(Mpeg4Fault::Predefined(self.predefined)));
        }
        let mut body = Vec::new();
        let mut w = BitWriter::new(&mut body);
        w.put(self.predefined as u64, 8);
        if self.predefined == 0 {
            for f in [
                self.use_au_start,
                self.use_au_end,
                self.use_rap,
                self.random_access_units_only,
                self.use_padding,
                self.use_timestamps,
                self.use_idle,
                self.duration.is_some(),
            ] {
                w.put(f as u64, 1);
            }
            w.put(self.ts_resolution as u64, 32);
            w.put(self.ocr_resolution as u64, 32);
            w.put(self.ts_len as u64, 8);
            w.put(self.ocr_len as u64, 8);
            w.put(self.au_len as u64, 8);
            w.put(self.instant_bitrate_len as u64, 8);
            w.put(self.degradation_priority_len as u64, 4);
            w.put(self.au_seq_len as u64, 5);
            w.put(self.packet_seq_len as u64, 5);
            w.put(0b11, 2);
        }
        if let (0, Some(d)) = (self.predefined, self.duration) {
            w.put(d.time_scale as u64, 32);
            w.put(d.au_duration as u64, 16);
            w.put(d.cu_duration as u64, 16);
        }
        if let (false, Some(d), Some(c)) = (self.use_timestamps, self.start_dts, self.start_cts) {
            w.put(d, self.ts_len as u32);
            w.put(c, self.ts_len as u32);
        }
        w.finish();
        write_descriptor(out, tag::SL_CONFIG, &body)
    }

    /// A time stamp of this configuration in 90 kHz units. `None` when the resolution is 0.
    pub fn to_90khz(&self, ticks: u64) -> Option<i64> {
        (self.ts_resolution > 0)
            .then(|| (ticks as u128 * 90_000 / self.ts_resolution as u128) as i64)
    }
}

// ---------------------------------------------------------------------------------------------------------------------------------
// SL packet header.

/// The header of one SL packet (ISO/IEC 14496-1 7.3.2.4). A field that the configuration leaves out is `None` (or `false`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SlHeader {
    /// accessUnitStartFlag: the payload starts an access unit (sent, or inferred as [`SlHeader::parse`] says).
    pub au_start: bool,
    /// accessUnitEndFlag: the payload ends an access unit (sent, or inferred as [`SlHeader::parse`] says).
    pub au_end: bool,
    /// idleFlag: the stream sends no data for a while. The payload is empty.
    pub idle: bool,
    /// paddingFlag.
    pub padding: bool,
    /// paddingBits: 0 with `padding` means the payload is padding only. Otherwise the payload ends with this many stuffing bits.
    pub padding_bits: u8,
    /// packetSequenceNumber.
    pub packet_seq: Option<u32>,
    /// degradationPriority.
    pub degradation_priority: Option<u32>,
    /// objectClockReference.
    pub ocr: Option<u64>,
    /// randomAccessPointFlag.
    pub rap: bool,
    /// AU_sequenceNumber.
    pub au_seq: Option<u32>,
    /// decodingTimeStamp, in ticks of the configuration's resolution.
    pub dts: Option<u64>,
    /// compositionTimeStamp, in ticks of the configuration's resolution.
    pub cts: Option<u64>,
    /// accessUnitLength in bytes.
    pub au_length: Option<u32>,
    /// instantBitrate in bit/s (`None` also when its length is more than 64 bits).
    pub instant_bitrate: Option<u64>,
}

fn opt(r: &mut BitReader<'_>, len: u8) -> Option<u64> {
    (len > 0).then(|| r.u(len as u32))
}

impl SlHeader {
    /// Reads the SL packet header at the start of `packet` with configuration `c`, and returns the header and the payload after
    /// it.
    ///
    /// When the configuration has no start flag, `au_start` is `previous_end`, the end flag of the previous packet (pass
    /// `true` for the first packet). When it has neither flag, every packet is one whole access unit. When it has a start
    /// flag but no end flag, `au_end` is `false`, and the next packet's start flag marks the end of the unit (7.3.2.4.2).
    /// A packet of padding only, or an idle packet, has an empty payload.
    ///
    /// # Errors
    /// [`Mpeg4Fault::Short`] when the packet ends inside the header.
    pub fn parse<'a>(
        packet: &'a [u8],
        c: &SlConfig,
        previous_end: bool,
    ) -> Result<(SlHeader, &'a [u8])> {
        let mut r = BitReader::new(packet);
        let mut h = SlHeader {
            au_start: if c.use_au_end { previous_end } else { true },
            au_end: !c.use_au_start,
            ..SlHeader::default()
        };
        if c.use_au_start {
            h.au_start = r.bit();
        }
        if c.use_au_end {
            h.au_end = r.bit();
        }
        let ocr_flag = c.ocr_len > 0 && r.bit();
        if c.use_idle {
            h.idle = r.bit();
        }
        if c.use_padding {
            h.padding = r.bit();
        }
        if h.padding {
            h.padding_bits = r.u(3) as u8;
        }
        let empty = h.idle || (h.padding && h.padding_bits == 0);
        if !empty {
            h.packet_seq = opt(&mut r, c.packet_seq_len).map(|v| v as u32);
            if c.degradation_priority_len > 0 && r.bit() {
                h.degradation_priority = Some(r.u(c.degradation_priority_len as u32) as u32);
            }
            if ocr_flag {
                h.ocr = Some(r.u(c.ocr_len as u32));
            }
            if h.au_start {
                if c.use_rap {
                    h.rap = r.bit();
                }
                h.au_seq = opt(&mut r, c.au_seq_len).map(|v| v as u32);
                let (dts_flag, cts_flag) = if c.use_timestamps {
                    (r.bit(), r.bit())
                } else {
                    (false, false)
                };
                let ibr_flag = c.instant_bitrate_len > 0 && r.bit();
                if dts_flag {
                    h.dts = Some(r.u(c.ts_len as u32));
                }
                if cts_flag {
                    h.cts = Some(r.u(c.ts_len as u32));
                }
                h.au_length = opt(&mut r, c.au_len).map(|v| v as u32);
                if ibr_flag {
                    let n = c.instant_bitrate_len as usize;
                    if n <= 64 {
                        h.instant_bitrate = Some(r.u(n as u32));
                    } else {
                        r.skip(n);
                    }
                }
            }
            if c.random_access_units_only && h.au_start {
                h.rap = true;
            }
        }
        r.align();
        finish(&r)?;
        let payload = if empty { &[][..] } else { r.rest() };
        Ok((h, payload))
    }

    /// Appends this header for configuration `c`, padded with zero bits to a whole byte. Fields that `c` leaves out are not
    /// written. The time stamps are written when present and `c` uses them.
    ///
    /// # Errors
    /// [`Mpeg4Fault::Field`] when `c` specifies an instant bit rate field longer than 64 bits.
    pub fn write(&self, c: &SlConfig, out: &mut Vec<u8>) -> Result<()> {
        if c.instant_bitrate_len > 64 && self.instant_bitrate.is_some() {
            return Err(Error::Mpeg4(Mpeg4Fault::Field(
                Mpeg4Field::InstantBitrateLength,
            )));
        }
        let mut w = BitWriter::new(out);
        if c.use_au_start {
            w.put(self.au_start as u64, 1);
        }
        if c.use_au_end {
            w.put(self.au_end as u64, 1);
        }
        let ocr_flag = c.ocr_len > 0 && self.ocr.is_some();
        if c.ocr_len > 0 {
            w.put(ocr_flag as u64, 1);
        }
        if c.use_idle {
            w.put(self.idle as u64, 1);
        }
        if c.use_padding {
            w.put(self.padding as u64, 1);
        }
        if c.use_padding && self.padding {
            w.put(self.padding_bits as u64, 3);
        }
        let empty =
            (c.use_idle && self.idle) || (c.use_padding && self.padding && self.padding_bits == 0);
        if !empty {
            if c.packet_seq_len > 0 {
                w.put(self.packet_seq.unwrap_or(0) as u64, c.packet_seq_len as u32);
            }
            if c.degradation_priority_len > 0 {
                w.put(self.degradation_priority.is_some() as u64, 1);
                if let Some(p) = self.degradation_priority {
                    w.put(p as u64, c.degradation_priority_len as u32);
                }
            }
            if let (true, Some(o)) = (ocr_flag, self.ocr) {
                w.put(o, c.ocr_len as u32);
            }
            let start = if c.use_au_start { self.au_start } else { true };
            if start {
                if c.use_rap {
                    w.put(self.rap as u64, 1);
                }
                if c.au_seq_len > 0 {
                    w.put(self.au_seq.unwrap_or(0) as u64, c.au_seq_len as u32);
                }
                let (dts, cts) = if c.use_timestamps {
                    (self.dts, self.cts)
                } else {
                    (None, None)
                };
                if c.use_timestamps {
                    w.put(dts.is_some() as u64, 1);
                    w.put(cts.is_some() as u64, 1);
                }
                let ibr = self.instant_bitrate.filter(|_| c.instant_bitrate_len > 0);
                if c.instant_bitrate_len > 0 {
                    w.put(ibr.is_some() as u64, 1);
                }
                for t in [dts, cts].into_iter().flatten() {
                    w.put(t, c.ts_len as u32);
                }
                if c.au_len > 0 {
                    w.put(self.au_length.unwrap_or(0) as u64, c.au_len as u32);
                }
                if let Some(b) = ibr {
                    w.put(b, c.instant_bitrate_len as u32);
                }
            }
        }
        w.align();
        w.finish();
        Ok(())
    }
}

/// Joins the payloads of SL packets into access units, using the start and end flags. Its buffer is reused, so it allocates
/// nothing once it has grown to the largest access unit.
///
/// ```
/// use sdrkit_proto_mpegts::mpeg4::{SlAssembler, SlConfig, SlHeader};
///
/// // Access unit start and end flags, a random access flag and 33-bit composition time stamps at 90 kHz.
/// let c = SlConfig {
///     use_au_start: true,
///     use_au_end: true,
///     use_rap: true,
///     use_timestamps: true,
///     ts_resolution: 90_000,
///     ts_len: 33,
///     ..Default::default()
/// };
/// let h = SlHeader { au_start: true, au_end: true, rap: true, cts: Some(9000), ..Default::default() };
/// let mut packet = Vec::new();
/// h.write(&c, &mut packet).unwrap();
/// assert_eq!(packet, [0xE8, 0x00, 0x00, 0x8C, 0xA0]);
/// packet.extend_from_slice(b"frame");
///
/// let mut au = SlAssembler::new(1 << 16);
/// let mut got = Vec::new();
/// au.push(&packet, &c, |h, data| got.push((h.cts, data.to_vec()))).unwrap();
/// assert_eq!(got, [(Some(9000), b"frame".to_vec())]);
/// ```
#[derive(Clone, Debug)]
pub struct SlAssembler {
    buf: Vec<u8>,
    first: Option<SlHeader>,
    previous_end: bool,
    max: usize,
    /// Access units dropped: cut short by a new start, larger than the limit, or joined in the middle.
    pub dropped: u64,
}

impl SlAssembler {
    /// An assembler that drops access units longer than `max_bytes`.
    pub fn new(max_bytes: usize) -> SlAssembler {
        SlAssembler {
            buf: Vec::new(),
            first: None,
            previous_end: true,
            max: max_bytes,
            dropped: 0,
        }
    }

    /// Forgets a partial access unit (after a loss or a retune).
    pub fn reset(&mut self) {
        self.buf.clear();
        self.first = None;
        self.previous_end = true;
    }

    /// Adds one SL packet of configuration `c`. Each access unit it completes goes to `out` with the header of its first
    /// packet. When `c` has a start flag but no end flag, a unit completes when the next one starts.
    ///
    /// # Errors
    /// The errors of [`SlHeader::parse`]. The assembler then drops its partial unit.
    pub fn push(
        &mut self,
        packet: &[u8],
        c: &SlConfig,
        mut out: impl FnMut(&SlHeader, &[u8]),
    ) -> Result<()> {
        let (h, payload) = match SlHeader::parse(packet, c, self.previous_end) {
            Ok(v) => v,
            Err(e) => {
                if self.first.take().is_some() {
                    self.dropped += 1;
                }
                self.buf.clear();
                return Err(e);
            }
        };
        if h.idle || (h.padding && h.padding_bits == 0) {
            return Ok(());
        }
        if h.au_start {
            if let Some(f) = self.first.take() {
                if c.use_au_end {
                    self.dropped += 1;
                } else {
                    out(&f, &self.buf);
                }
            }
            self.buf.clear();
            self.first = Some(h);
        } else if self.first.is_none() {
            self.previous_end = h.au_end;
            return Ok(());
        }
        if self.buf.len() + payload.len() > self.max {
            self.dropped += 1;
            self.first = None;
            self.buf.clear();
        } else {
            self.buf.extend_from_slice(payload);
        }
        if h.au_end {
            if let Some(f) = self.first.take() {
                out(&f, &self.buf);
            }
            self.buf.clear();
        }
        self.previous_end = h.au_end;
        Ok(())
    }

    /// Returns the unit held when the configuration has no end flag, at the end of the stream.
    pub fn finish(&mut self, mut out: impl FnMut(&SlHeader, &[u8])) {
        if let Some(f) = self.first.take() {
            out(&f, &self.buf);
        }
        self.buf.clear();
    }
}

// ---------------------------------------------------------------------------------------------------------------------------------
// Decoder configuration, ES descriptor, object descriptors.

/// A DecoderConfigDescriptor: the kind of stream and how to decode it (ISO/IEC 14496-1 7.2.6.6).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DecoderConfig {
    /// objectTypeIndication (see [`object_type`]).
    pub object_type: u8,
    /// streamType (see [`stream_type`]).
    pub stream_type: u8,
    /// upStream.
    pub up_stream: bool,
    /// bufferSizeDB in bytes.
    pub buffer_size_db: u32,
    /// maxBitrate in bit/s.
    pub max_bitrate: u32,
    /// avgBitrate in bit/s (0 for a variable rate).
    pub avg_bitrate: u32,
    /// DecoderSpecificInfo (for H.264 the parameter sets, for audio the AudioSpecificConfig). Empty when absent.
    pub specific_info: Vec<u8>,
}

impl DecoderConfig {
    /// Parses the body of a DecoderConfigDescriptor.
    ///
    /// # Errors
    /// [`Mpeg4Fault::Short`] when it is shorter than its 13 fixed bytes or a descriptor in it runs past the end.
    pub fn parse(body: &[u8]) -> Result<DecoderConfig> {
        let fixed: &[u8; 13] = body
            .get(..13)
            .and_then(|b| b.try_into().ok())
            .ok_or(Error::Mpeg4(Mpeg4Fault::Short))?;
        let be =
            |i: usize| u32::from_be_bytes([fixed[i], fixed[i + 1], fixed[i + 2], fixed[i + 3]]);
        let mut d = DecoderConfig {
            object_type: fixed[0],
            stream_type: fixed[1] >> 2,
            up_stream: fixed[1] & 2 != 0,
            buffer_size_db: u32::from_be_bytes([0, fixed[2], fixed[3], fixed[4]]),
            max_bitrate: be(5),
            avg_bitrate: be(9),
            specific_info: Vec::new(),
        };
        for item in descriptors(&body[13..]) {
            let (t, b) = item?;
            if t == tag::DECODER_SPECIFIC_INFO {
                d.specific_info = b.to_vec();
            }
        }
        Ok(d)
    }

    /// Appends this configuration as a whole DecoderConfigDescriptor.
    ///
    /// # Errors
    /// [`Mpeg4Fault::Field`] when `stream_type` needs more than 6 bits or `buffer_size_db` more than 24 bits.
    pub fn write(&self, out: &mut Vec<u8>) -> Result<()> {
        if self.stream_type > 0x3F {
            return Err(Error::Mpeg4(Mpeg4Fault::Field(Mpeg4Field::StreamType)));
        }
        if self.buffer_size_db > 0xFF_FFFF {
            return Err(Error::Mpeg4(Mpeg4Fault::Field(Mpeg4Field::BufferSizeDb)));
        }
        let mut b = vec![
            self.object_type,
            self.stream_type << 2 | (self.up_stream as u8) << 1 | 1,
        ];
        b.extend_from_slice(&self.buffer_size_db.to_be_bytes()[1..]);
        b.extend_from_slice(&self.max_bitrate.to_be_bytes());
        b.extend_from_slice(&self.avg_bitrate.to_be_bytes());
        if !self.specific_info.is_empty() {
            write_descriptor(&mut b, tag::DECODER_SPECIFIC_INFO, &self.specific_info)?;
        }
        write_descriptor(out, tag::DECODER_CONFIG, &b)
    }
}

/// An ES_Descriptor: one elementary stream of an object (ISO/IEC 14496-1 7.2.6.5).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct EsDescriptor {
    /// ES_ID.
    pub es_id: u16,
    /// dependsOn_ES_ID, when streamDependenceFlag is set.
    pub depends_on: Option<u16>,
    /// URLstring, when URL_Flag is set: the stream is elsewhere.
    pub url: Option<Vec<u8>>,
    /// OCR_ES_Id, when OCRstreamFlag is set.
    pub ocr_es_id: Option<u16>,
    /// streamPriority (0 to 31).
    pub priority: u8,
    /// The decoder configuration.
    pub decoder: DecoderConfig,
    /// The SL packet header configuration.
    pub sl: SlConfig,
    /// The ISO 639-2 language code of a LanguageDescriptor, when present.
    pub language: Option<[u8; 3]>,
}

impl EsDescriptor {
    /// Parses the body of an ES_Descriptor. Descriptors it does not use (IPMP, QoS, registration) are stepped over.
    ///
    /// # Errors
    /// [`Mpeg4Fault::Short`] when it is short, [`Mpeg4Fault::Field`] when the DecoderConfigDescriptor or the SLConfigDescriptor
    /// is missing, and the errors of [`SlConfig::parse`].
    pub fn parse(body: &[u8]) -> Result<EsDescriptor> {
        let short = Error::Mpeg4(Mpeg4Fault::Short);
        let mut e = EsDescriptor {
            es_id: u16::from_be_bytes([*body.first().ok_or(short)?, *body.get(1).ok_or(short)?]),
            ..EsDescriptor::default()
        };
        let flags = *body.get(2).ok_or(short)?;
        e.priority = flags & 0x1F;
        let mut i = 3;
        let u16_at = |i: usize| -> Result<u16> {
            Ok(u16::from_be_bytes([
                *body.get(i).ok_or(short)?,
                *body.get(i + 1).ok_or(short)?,
            ]))
        };
        if flags & 0x80 != 0 {
            e.depends_on = Some(u16_at(i)?);
            i += 2;
        }
        if flags & 0x40 != 0 {
            let n = *body.get(i).ok_or(short)? as usize;
            e.url = Some(body.get(i + 1..i + 1 + n).ok_or(short)?.to_vec());
            i += 1 + n;
        }
        if flags & 0x20 != 0 {
            e.ocr_es_id = Some(u16_at(i)?);
            i += 2;
        }
        let (mut decoder, mut sl) = (None, None);
        for item in descriptors(body.get(i..).ok_or(short)?) {
            let (t, b) = item?;
            match t {
                tag::DECODER_CONFIG => decoder = Some(DecoderConfig::parse(b)?),
                tag::SL_CONFIG => sl = Some(SlConfig::parse(b)?),
                tag::LANGUAGE if b.len() >= 3 => e.language = Some([b[0], b[1], b[2]]),
                _ => {}
            }
        }
        e.decoder = decoder.ok_or(Error::Mpeg4(Mpeg4Fault::Field(
            Mpeg4Field::DecoderConfigDescriptor,
        )))?;
        e.sl = sl.ok_or(Error::Mpeg4(Mpeg4Fault::Field(
            Mpeg4Field::SlConfigDescriptor,
        )))?;
        Ok(e)
    }

    /// Appends this ES descriptor as a whole ES_Descriptor.
    ///
    /// # Errors
    /// [`Mpeg4Fault::Field`] when `priority` is above 31 or the URL is longer than 255 bytes, and the errors of the nested
    /// writers.
    pub fn write(&self, out: &mut Vec<u8>) -> Result<()> {
        if self.priority > 31 {
            return Err(Error::Mpeg4(Mpeg4Fault::Field(Mpeg4Field::StreamPriority)));
        }
        let mut b = self.es_id.to_be_bytes().to_vec();
        b.push(
            (self.depends_on.is_some() as u8) << 7
                | (self.url.is_some() as u8) << 6
                | (self.ocr_es_id.is_some() as u8) << 5
                | self.priority,
        );
        if let Some(d) = self.depends_on {
            b.extend_from_slice(&d.to_be_bytes());
        }
        if let Some(u) = &self.url {
            let n = u8::try_from(u.len()).map_err(|_| Mpeg4Fault::Field(Mpeg4Field::UrlLength))?;
            b.push(n);
            b.extend_from_slice(u);
        }
        if let Some(o) = self.ocr_es_id {
            b.extend_from_slice(&o.to_be_bytes());
        }
        self.decoder.write(&mut b)?;
        self.sl.write(&mut b)?;
        if let Some(l) = self.language {
            write_descriptor(&mut b, tag::LANGUAGE, &l)?;
        }
        write_descriptor(out, tag::ES_DESCRIPTOR, &b)
    }
}

/// The profile and level indications of an initial object descriptor (0xFF: no capability required, 0xFE: not given).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Profiles {
    /// ODProfileLevelIndication.
    pub od: u8,
    /// sceneProfileLevelIndication.
    pub scene: u8,
    /// audioProfileLevelIndication.
    pub audio: u8,
    /// visualProfileLevelIndication.
    pub visual: u8,
    /// graphicsProfileLevelIndication.
    pub graphics: u8,
}

impl Default for Profiles {
    fn default() -> Self {
        Profiles {
            od: 0xFF,
            scene: 0xFF,
            audio: 0xFF,
            visual: 0xFF,
            graphics: 0xFF,
        }
    }
}

/// An ObjectDescriptor or an InitialObjectDescriptor: one media object and its elementary streams (ISO/IEC 14496-1 7.2.6.3
/// and 7.2.6.4).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ObjectDescriptor {
    /// ObjectDescriptorID (10 bits).
    pub id: u16,
    /// An initial object descriptor (tag 0x02), which has the profile indications.
    pub initial: bool,
    /// includeInlineProfileLevelFlag (initial object descriptors only).
    pub include_inline_profiles: bool,
    /// URLstring, when URL_Flag is set: the descriptor is elsewhere, and there are no streams here.
    pub url: Option<Vec<u8>>,
    /// The profile indications (initial object descriptors without a URL only).
    pub profiles: Option<Profiles>,
    /// The ES descriptors.
    pub es: Vec<EsDescriptor>,
}

impl ObjectDescriptor {
    /// Parses a whole object descriptor or initial object descriptor (tag, size, body). Descriptors it does not use (OCI,
    /// IPMP, extensions) are stepped over.
    ///
    /// # Errors
    /// [`Mpeg4Fault::Tag`] for another tag, [`Mpeg4Fault::Short`] when it is short, and the errors of
    /// [`EsDescriptor::parse`].
    pub fn parse(b: &[u8]) -> Result<ObjectDescriptor> {
        let (t, body) = descriptors(b)
            .next()
            .ok_or(Error::Mpeg4(Mpeg4Fault::Short))??;
        Self::parse_body(t, body)
    }

    /// Parses the body of a descriptor with tag `t` ([`tag::OBJECT_DESCRIPTOR`] or [`tag::INITIAL_OBJECT_DESCRIPTOR`]).
    ///
    /// # Errors
    /// As [`ObjectDescriptor::parse`].
    pub fn parse_body(t: u8, body: &[u8]) -> Result<ObjectDescriptor> {
        let initial = match t {
            tag::OBJECT_DESCRIPTOR => false,
            tag::INITIAL_OBJECT_DESCRIPTOR => true,
            found => {
                return Err(Error::Mpeg4(Mpeg4Fault::Tag {
                    expected: tag::OBJECT_DESCRIPTOR,
                    found,
                }))
            }
        };
        let head = u16::from_be_bytes([
            *body.first().ok_or(Error::Mpeg4(Mpeg4Fault::Short))?,
            *body.get(1).ok_or(Error::Mpeg4(Mpeg4Fault::Short))?,
        ]);
        let mut od = ObjectDescriptor {
            id: head >> 6,
            initial,
            include_inline_profiles: initial && head & 0x10 != 0,
            ..ObjectDescriptor::default()
        };
        let mut i = 2;
        if head & 0x20 != 0 {
            let n = *body.get(i).ok_or(Error::Mpeg4(Mpeg4Fault::Short))? as usize;
            od.url = Some(
                body.get(i + 1..i + 1 + n)
                    .ok_or(Error::Mpeg4(Mpeg4Fault::Short))?
                    .to_vec(),
            );
            return Ok(od);
        }
        if initial {
            let p = body.get(i..i + 5).ok_or(Error::Mpeg4(Mpeg4Fault::Short))?;
            od.profiles = Some(Profiles {
                od: p[0],
                scene: p[1],
                audio: p[2],
                visual: p[3],
                graphics: p[4],
            });
            i += 5;
        }
        for item in descriptors(&body[i..]) {
            let (t, b) = item?;
            if t == tag::ES_DESCRIPTOR {
                od.es.push(EsDescriptor::parse(b)?);
            }
        }
        Ok(od)
    }

    /// The ES descriptor with this ES_ID.
    pub fn stream(&self, es_id: u16) -> Option<&EsDescriptor> {
        self.es.iter().find(|e| e.es_id == es_id)
    }

    /// Appends this descriptor whole (tag 0x02 when `initial`, else 0x01). The reserved bits are ones.
    ///
    /// # Errors
    /// [`Mpeg4Fault::Field`] when `id` needs more than 10 bits or the URL is longer than 255 bytes, and the errors of the
    /// nested writers.
    pub fn write(&self, out: &mut Vec<u8>) -> Result<()> {
        if self.id > 0x3FF {
            return Err(Error::Mpeg4(Mpeg4Fault::Field(
                Mpeg4Field::ObjectDescriptorId,
            )));
        }
        let url = self.url.is_some() as u16;
        let head = if self.initial {
            self.id << 6 | url << 5 | (self.include_inline_profiles as u16) << 4 | 0x0F
        } else {
            self.id << 6 | url << 5 | 0x1F
        };
        let mut b = head.to_be_bytes().to_vec();
        if let Some(u) = &self.url {
            let n = u8::try_from(u.len()).map_err(|_| Mpeg4Fault::Field(Mpeg4Field::UrlLength))?;
            b.push(n);
            b.extend_from_slice(u);
        } else {
            if self.initial {
                let p = self.profiles.unwrap_or_default();
                b.extend_from_slice(&[p.od, p.scene, p.audio, p.visual, p.graphics]);
            }
            for e in &self.es {
                e.write(&mut b)?;
            }
        }
        let t = if self.initial {
            tag::INITIAL_OBJECT_DESCRIPTOR
        } else {
            tag::OBJECT_DESCRIPTOR
        };
        write_descriptor(out, t, &b)
    }
}

/// A command of the object descriptor stream (ISO/IEC 14496-1 7.2.5).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum OdCommand {
    /// ObjectDescriptorUpdate: new or changed object descriptors.
    Update(Vec<ObjectDescriptor>),
    /// ObjectDescriptorRemove: the ids of object descriptors that end.
    Remove(Vec<u16>),
    /// Another command, by its tag. Its body is stepped over.
    Other(u8),
}

impl OdCommand {
    /// Appends the commands in `b` (the payload of an access unit of an object descriptor stream) to `out`.
    ///
    /// # Errors
    /// [`Mpeg4Fault::Short`] when a command runs past the end, and the errors of [`ObjectDescriptor::parse_body`]. The
    /// commands before the error stay in `out`.
    pub fn parse_all(b: &[u8], out: &mut Vec<OdCommand>) -> Result<()> {
        for item in descriptors(b) {
            let (t, body) = item?;
            out.push(match t {
                command_tag::OD_UPDATE => {
                    let mut ods = Vec::new();
                    for d in descriptors(body) {
                        let (t, b) = d?;
                        ods.push(ObjectDescriptor::parse_body(t, b)?);
                    }
                    OdCommand::Update(ods)
                }
                command_tag::OD_REMOVE => {
                    let mut r = BitReader::new(body);
                    let ids = (0..body.len() * 8 / 10).map(|_| r.u(10) as u16).collect();
                    OdCommand::Remove(ids)
                }
                other => OdCommand::Other(other),
            });
        }
        Ok(())
    }

    /// Appends this command whole (tag, size, body).
    ///
    /// # Errors
    /// [`Mpeg4Fault::Field`] for [`OdCommand::Other`] or an id above 10 bits, and the errors of [`ObjectDescriptor::write`].
    pub fn write(&self, out: &mut Vec<u8>) -> Result<()> {
        let mut b = Vec::new();
        let t = match self {
            OdCommand::Update(ods) => {
                for od in ods {
                    od.write(&mut b)?;
                }
                command_tag::OD_UPDATE
            }
            OdCommand::Remove(ids) => {
                let mut w = BitWriter::new(&mut b);
                for &id in ids {
                    if id > 0x3FF {
                        return Err(Error::Mpeg4(Mpeg4Fault::Field(
                            Mpeg4Field::ObjectDescriptorId,
                        )));
                    }
                    w.put(id as u64, 10);
                }
                w.finish();
                command_tag::OD_REMOVE
            }
            OdCommand::Other(_) => {
                return Err(Error::Mpeg4(Mpeg4Fault::Field(Mpeg4Field::CommandTag)))
            }
        };
        write_descriptor(out, t, &b)
    }
}

// ---------------------------------------------------------------------------------------------------------------------------------
// The MPEG-2 side: PMT descriptors and 14496 sections.

/// The IOD_descriptor of a PMT: the initial object descriptor of a programme (ISO/IEC 13818-1 2.6.40).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IodDescriptor {
    /// Scope_of_IOD_label: 0x10 unique in the programme, 0x11 unique in the transport stream.
    pub scope: u8,
    /// IOD_label.
    pub label: u8,
    /// The initial object descriptor.
    pub iod: ObjectDescriptor,
}

impl IodDescriptor {
    /// Parses the payload of an IOD_descriptor (after the 13818-1 tag and length bytes).
    ///
    /// # Errors
    /// [`Mpeg4Fault::Short`] when it is short, [`Mpeg4Fault::Tag`] when the object descriptor is not an initial one, and the
    /// errors of [`ObjectDescriptor::parse`].
    pub fn parse(payload: &[u8]) -> Result<IodDescriptor> {
        let (&scope, rest) = payload
            .split_first()
            .ok_or(Error::Mpeg4(Mpeg4Fault::Short))?;
        let (&label, rest) = rest.split_first().ok_or(Error::Mpeg4(Mpeg4Fault::Short))?;
        let iod = ObjectDescriptor::parse_body(
            tag::INITIAL_OBJECT_DESCRIPTOR,
            expect(rest, tag::INITIAL_OBJECT_DESCRIPTOR)?,
        )?;
        Ok(IodDescriptor { scope, label, iod })
    }

    /// Appends the whole 13818-1 descriptor (tag 0x1D, length, payload).
    ///
    /// # Errors
    /// [`Mpeg4Fault::Field`] when `iod` is not initial, [`Mpeg4Fault::Size`] when the payload is longer than 255 bytes, and
    /// the errors of [`ObjectDescriptor::write`].
    pub fn write(&self, out: &mut Vec<u8>) -> Result<()> {
        if !self.iod.initial {
            return Err(Error::Mpeg4(Mpeg4Fault::Field(
                Mpeg4Field::InitialObjectDescriptor,
            )));
        }
        let mut p = vec![self.scope, self.label];
        self.iod.write(&mut p)?;
        let n = u8::try_from(p.len()).map_err(|_| Error::Mpeg4(Mpeg4Fault::Size))?;
        out.push(IOD_DESCRIPTOR_TAG);
        out.push(n);
        out.extend_from_slice(&p);
        Ok(())
    }
}

/// The ES_ID of an SL_descriptor payload (ISO/IEC 13818-1 2.6.42). `None` when it is shorter than 2 bytes.
pub fn sl_descriptor(payload: &[u8]) -> Option<u16> {
    Some(u16::from_be_bytes([*payload.first()?, *payload.get(1)?]))
}

/// The `(ES_ID, FlexMuxChannel)` entries of an FMC_descriptor payload (ISO/IEC 13818-1 2.6.44). A partial entry at the end
/// is ignored.
pub fn fmc_descriptor(payload: &[u8]) -> impl Iterator<Item = (u16, u8)> + '_ {
    payload
        .as_chunks::<3>()
        .0
        .iter()
        .map(|c| (u16::from_be_bytes([c[0], c[1]]), c[2]))
}

/// The SL packet of a 14496 section (table_id 0x04 or 0x05, ISO_IEC_14496_section of ISO/IEC 13818-1 2.4.4): `(table_id, packet)`. The section
/// must be whole, with a good CRC. It allocates nothing.
///
/// # Errors
/// [`Mpeg4Fault::Field`] for another table_id or a section without the long header, [`Mpeg4Fault::Short`] when the section
/// is short, or its length or CRC is wrong.
pub fn section_payload(section: &[u8]) -> Result<(u8, &[u8])> {
    let t = *section.first().ok_or(Error::Mpeg4(Mpeg4Fault::Short))?;
    if t != TABLE_SCENE_DESCRIPTION && t != TABLE_OBJECT_DESCRIPTOR {
        return Err(Error::Mpeg4(Mpeg4Fault::Field(Mpeg4Field::TableId)));
    }
    let h = crate::tables::check(section).map_err(|_| Error::Mpeg4(Mpeg4Fault::Short))?;
    if !h.syntax || h.total_len < 12 {
        return Err(Error::Mpeg4(Mpeg4Fault::Field(
            Mpeg4Field::SectionSyntaxIndicator,
        )));
    }
    Ok((t, &section[8..h.total_len - 4]))
}

/// A 14496 section holding one SL packet, with its CRC (ISO_IEC_14496_section of ISO/IEC 13818-1 2.4.4).
pub fn build_section(table_id: u8, ext: u16, version: u8, sl_packet: &[u8]) -> Vec<u8> {
    crate::tables::build_long(table_id, ext, version, 0, 0, sl_packet)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// ISO/IEC 14496-1 8.3.3: seven bits a byte, the top bit says "more", one to four bytes.
    #[test]
    fn sizes_of_clause_8_3_3() {
        for (n, bytes) in [
            (0usize, &[0x00u8][..]),
            (5, &[0x05]),
            (127, &[0x7F]),
            (128, &[0x81, 0x00]),
            (300, &[0x82, 0x2C]),
            (16_384, &[0x81, 0x80, 0x00]),
            ((1 << 28) - 1, &[0xFF, 0xFF, 0xFF, 0x7F]),
        ] {
            let mut v = Vec::new();
            write_size(&mut v, n).unwrap();
            assert_eq!(v, bytes, "{n}");
            let mut at = 0;
            assert_eq!(read_size(bytes, &mut at), Ok(n));
            assert_eq!(at, bytes.len());
        }
        let mut at = 0;
        assert_eq!(read_size(&[0x80, 0x80, 0x80, 0x05], &mut at), Ok(5));
        assert_eq!(
            read_size(&[0x80, 0x80, 0x80, 0x80, 0x05], &mut 0),
            Err(Error::Mpeg4(Mpeg4Fault::Size))
        );
        assert_eq!(
            read_size(&[0x81], &mut 0),
            Err(Error::Mpeg4(Mpeg4Fault::Short))
        );
        assert_eq!(
            write_size(&mut Vec::new(), 1 << 28),
            Err(Error::Mpeg4(Mpeg4Fault::Size))
        );
    }

    /// An SLConfigDescriptor laid out by hand from 7.3.2.3.1: start, end and random access flags, time stamps, 90 kHz,
    /// 33-bit time stamps, no other fields, reserved bits 0b11.
    const SL_CONFIG: [u8; 18] = [
        0x06,
        0x10,        // tag, size 16
        0x00,        // predefined
        0b1110_0100, // start, end, rap, ra-only 0, padding 0, time stamps, idle 0, duration 0
        0x00,
        0x01,
        0x5F,
        0x90, // timeStampResolution 90000
        0x00,
        0x00,
        0x00,
        0x00, // OCRResolution
        0x21,
        0x00,
        0x00,
        0x00, // timeStampLength 33, OCRLength, AU_Length, instantBitrateLength
        0x00,
        0x03, // degradation 4, AU_seqNum 5, packetSeqNum 5, reserved 2
    ];

    fn timed() -> SlConfig {
        SlConfig {
            use_au_start: true,
            use_au_end: true,
            use_rap: true,
            use_timestamps: true,
            ts_resolution: 90_000,
            ts_len: 33,
            ..SlConfig::default()
        }
    }

    #[test]
    fn sl_config_of_clause_7_3_2_3() {
        let c = SlConfig::parse(&SL_CONFIG[2..]).unwrap();
        assert_eq!(c, timed());
        let mut v = Vec::new();
        timed().write(&mut v).unwrap();
        assert_eq!(v, SL_CONFIG);
        // Predefined 1: the null header. Predefined 2 (MP4 files) and the reserved values are refused.
        assert_eq!(SlConfig::parse(&[1]).unwrap(), SlConfig::null_header());
        assert_eq!(
            SlConfig::parse(&[2]),
            Err(Error::Mpeg4(Mpeg4Fault::Predefined(2)))
        );
        assert_eq!(
            SlConfig::parse(&[0x00, 0xE4]),
            Err(Error::Mpeg4(Mpeg4Fault::Short))
        );
        // timeStampLength 65 is more than 7.3.2.3.2 allows.
        let mut bad = SL_CONFIG;
        bad[12] = 65;
        assert_eq!(
            SlConfig::parse(&bad[2..]),
            Err(Error::Mpeg4(Mpeg4Fault::Field(Mpeg4Field::TimeStampLength)))
        );
        // durationFlag with timeScale 1000, 40 ms units, and start time stamps when the headers carry none.
        let c = SlConfig {
            use_au_start: true,
            ts_resolution: 1000,
            ts_len: 16,
            duration: Some(SlDuration {
                time_scale: 1000,
                au_duration: 40,
                cu_duration: 40,
            }),
            start_dts: Some(0x1234),
            start_cts: Some(0x5678),
            ..SlConfig::default()
        };
        let mut v = Vec::new();
        c.write(&mut v).unwrap();
        assert_eq!(&v[2..4], &[0x00, 0b1000_0001]);
        assert_eq!(
            &v[18..],
            &[0, 0, 0x03, 0xE8, 0, 40, 0, 40, 0x12, 0x34, 0x56, 0x78]
        );
        assert_eq!(SlConfig::parse(&v[2..]).unwrap(), c);
        assert_eq!(c.to_90khz(1000), Some(90_000));
    }

    /// The SL packet header of 7.3.2.4.1 with the configuration above: start 1, end 1, rap 1, DTS flag 0, CTS flag 1, then the
    /// 33-bit CTS 9000 and two zero bits to the byte: 11101 then 9000 in 33 bits then 00 = E8 00 00 8C A0.
    #[test]
    fn sl_header_of_clause_7_3_2_4() {
        let c = timed();
        let packet = [0xE8, 0x00, 0x00, 0x8C, 0xA0, b'a', b'u'];
        let (h, payload) = SlHeader::parse(&packet, &c, true).unwrap();
        assert!(h.au_start && h.au_end && h.rap);
        assert_eq!((h.dts, h.cts), (None, Some(9000)));
        assert_eq!(payload, b"au");
        assert_eq!(c.to_90khz(h.cts.unwrap()), Some(9000));
        let mut v = Vec::new();
        h.write(&c, &mut v).unwrap();
        assert_eq!(v, packet[..5]);
        assert_eq!(
            SlHeader::parse(&packet[..3], &c, true),
            Err(Error::Mpeg4(Mpeg4Fault::Short))
        );
        // A continuation packet (start 0, end 1) has no rap or time stamp fields: 01 then six zero bits.
        let (h, payload) = SlHeader::parse(&[0x40, 7], &c, false).unwrap();
        assert!(!h.au_start && h.au_end && h.cts.is_none());
        assert_eq!(payload, [7]);
    }

    /// The fields of 7.3.2.4.1 in their order: idle, padding with padding bits, sequence numbers, degradation priority, OCR,
    /// both time stamps, AU length and instant bit rate.
    #[test]
    fn every_header_field_in_order() {
        let c = SlConfig {
            use_au_start: true,
            use_au_end: false,
            use_rap: true,
            use_padding: true,
            use_idle: true,
            use_timestamps: true,
            ts_resolution: 1000,
            ts_len: 8,
            ocr_len: 4,
            au_len: 8,
            instant_bitrate_len: 4,
            degradation_priority_len: 3,
            au_seq_len: 2,
            packet_seq_len: 3,
            ..SlConfig::default()
        };
        let h = SlHeader {
            au_start: true,
            au_end: false,
            padding: true,
            padding_bits: 3,
            packet_seq: Some(5),
            degradation_priority: Some(6),
            ocr: Some(9),
            rap: true,
            au_seq: Some(2),
            dts: Some(0xAB),
            cts: Some(0xCD),
            au_length: Some(17),
            instant_bitrate: Some(12),
            ..SlHeader::default()
        };
        // start 1 | OCR flag 1 | idle 0 | padding 1 | padding bits 011 | packet seq 101 | degradation flag 1 | priority 110 |
        // OCR 1001 | rap 1 | AU seq 10 | DTS flag 1 | CTS flag 1 | IBR flag 1 | DTS 10101011 | CTS 11001101 | AU length 00010001
        // | IBR 1100 | then 4 zero bits to the byte.
        let bits = "1101011101111010011101111010101111001101000100011100";
        let expected: Vec<u8> = bits
            .as_bytes()
            .chunks(8)
            .map(|c| c.iter().fold(0u8, |a, &b| a << 1 | (b - b'0')) << (8 - c.len()))
            .collect();
        let mut v = Vec::new();
        h.write(&c, &mut v).unwrap();
        assert_eq!(v, expected);
        let (back, payload) = SlHeader::parse(&v, &c, true).unwrap();
        assert_eq!(back, h);
        assert!(payload.is_empty());
        // An idle packet: start 1, OCR flag 0, idle 1, padding 0, and nothing else.
        let (h, payload) = SlHeader::parse(&[0b1010_0000, 9], &c, true).unwrap();
        assert!(h.idle && payload.is_empty());
        // Padding only (padding 1, padding bits 000).
        let (h, payload) = SlHeader::parse(&[0b0001_0000, 9], &c, true).unwrap();
        assert!(h.padding && h.padding_bits == 0 && payload.is_empty());
    }

    /// 7.3.2.4.2: without either flag, every packet is a whole access unit. With an end flag only, a packet starts a unit
    /// when the one before it ended one.
    #[test]
    fn inferred_start_and_end_flags() {
        let (h, p) = SlHeader::parse(&[1, 2], &SlConfig::null_header(), false).unwrap();
        assert!(h.au_start && h.au_end);
        assert_eq!(p, [1, 2]);
        let end_only = SlConfig {
            use_au_end: true,
            ..SlConfig::default()
        };
        let (h, _) = SlHeader::parse(&[0x80], &end_only, false).unwrap();
        assert!(!h.au_start && h.au_end);
        let (h, _) = SlHeader::parse(&[0x00], &end_only, true).unwrap();
        assert!(h.au_start && !h.au_end);
    }

    #[test]
    fn the_assembler_joins_fragments() {
        let c = timed();
        let mut sl = SlAssembler::new(1000);
        let mut units = Vec::new();
        let mut put = |h: &SlHeader, d: &[u8]| units.push((h.cts, d.to_vec()));
        // Start (no end) with CTS 1, a middle packet, an end packet.
        let start = SlHeader {
            au_start: true,
            rap: true,
            cts: Some(1),
            ..SlHeader::default()
        };
        let mut p = Vec::new();
        start.write(&c, &mut p).unwrap();
        p.extend_from_slice(b"ab");
        sl.push(&p, &c, &mut put).unwrap();
        sl.push(&[0x00, b'c'], &c, &mut put).unwrap();
        sl.push(&[0x40, b'd'], &c, &mut put).unwrap();
        // A continuation with no unit started is dropped silently.
        sl.push(&[0x40, b'x'], &c, &mut put).unwrap();
        // A new start before the end drops the unfinished unit.
        sl.push(&p, &c, &mut put).unwrap();
        sl.push(&p, &c, &mut put).unwrap();
        assert_eq!(units, [(Some(1), b"abcd".to_vec())]);
        assert_eq!(sl.dropped, 1);
        // With a start flag and no end flag, a unit completes when the next starts, or at finish.
        let start_only = SlConfig {
            use_au_start: true,
            ..SlConfig::default()
        };
        let mut sl = SlAssembler::new(1000);
        let mut units = Vec::new();
        for p in [[0x80, 1], [0x00, 2], [0x80, 3]] {
            sl.push(&p, &start_only, |_, d| units.push(d.to_vec()))
                .unwrap();
        }
        sl.finish(|_, d| units.push(d.to_vec()));
        assert_eq!(units, [vec![1, 2], vec![3]]);
    }

    /// An ES_Descriptor and an InitialObjectDescriptor laid out by hand from 7.2.6.4 to 7.2.6.6, as a DMB H.264 stream.
    #[test]
    fn descriptors_of_clause_7_2_6() {
        let mut es = vec![
            0x03, 0x2A, // ES_Descriptor, size 42
            0x01, 0x01, // ES_ID 0x101
            0x00, // no dependence, no URL, no OCR stream, priority 0
            0x04, 0x13, // DecoderConfigDescriptor, size 19
            0x21, // objectTypeIndication H.264
            0x11, // streamType 4, upStream 0, reserved 1
            0x00, 0x10, 0x00, // bufferSizeDB 4096
            0x00, 0x05, 0xDC, 0x00, // maxBitrate 384000
            0x00, 0x05, 0xDC, 0x00, // avgBitrate 384000
            0x05, 0x04, 0x67, 0x42, 0xC0, 0x1E, // DecoderSpecificInfo
        ];
        es.extend_from_slice(&SL_CONFIG);
        assert_eq!(es.len(), 2 + 42);
        let e = EsDescriptor::parse(expect(&es, tag::ES_DESCRIPTOR).unwrap()).unwrap();
        assert_eq!(
            (e.es_id, e.priority, e.depends_on, e.url.clone()),
            (0x101, 0, None, None)
        );
        assert_eq!(e.decoder.object_type, object_type::AVC);
        assert_eq!(e.decoder.stream_type, stream_type::VISUAL);
        assert_eq!(
            (e.decoder.buffer_size_db, e.decoder.max_bitrate),
            (4096, 384_000)
        );
        assert_eq!(e.decoder.specific_info, [0x67, 0x42, 0xC0, 0x1E]);
        assert_eq!(e.sl, timed());
        let mut v = Vec::new();
        e.write(&mut v).unwrap();
        assert_eq!(v, es);

        // The IOD_descriptor of 13818-1 2.6.40: scope 0x10, label 1, then the InitialObjectDescriptor with id 1, no URL,
        // includeInlineProfileLevelFlag 0, reserved 1111 (0x00 0x4F), five profile bytes of 0xFF, and the ES descriptor.
        let mut iod = vec![
            0x02,
            2 + 5 + es.len() as u8,
            0x00,
            0x4F,
            0xFF,
            0xFF,
            0xFF,
            0xFF,
            0xFF,
        ];
        iod.extend_from_slice(&es);
        let mut desc = vec![IOD_DESCRIPTOR_TAG, 2 + iod.len() as u8, 0x10, 0x01];
        desc.extend_from_slice(&iod);
        let d = IodDescriptor::parse(&desc[2..]).unwrap();
        assert_eq!(
            (d.scope, d.label, d.iod.id, d.iod.initial),
            (0x10, 1, 1, true)
        );
        assert_eq!(d.iod.profiles, Some(Profiles::default()));
        assert_eq!(d.iod.stream(0x101), Some(&e));
        let mut v = Vec::new();
        d.write(&mut v).unwrap();
        assert_eq!(v, desc);
        // A URL-only ES_Descriptor and an object descriptor elsewhere (URL_Flag).
        assert_eq!(
            EsDescriptor::parse(&[0, 1, 0x00]),
            Err(Error::Mpeg4(Mpeg4Fault::Field(
                Mpeg4Field::DecoderConfigDescriptor
            )))
        );
        let od = ObjectDescriptor::parse(&[0x01, 0x05, 0x00, 0xBF, 0x02, b'a', b'b']).unwrap();
        assert_eq!(
            (od.id, od.initial, od.url),
            (2, false, Some(b"ab".to_vec()))
        );
    }

    /// ObjectDescriptorUpdate and ObjectDescriptorRemove of 7.2.5: an update with an object descriptor of id 3 (0x00 0xDF:
    /// id 3, URL 0, reserved 11111), and a remove of ids 3 and 1 in 10-bit fields (0000000011 0000000001 then 4 zero bits).
    #[test]
    fn od_commands_of_clause_7_2_5() {
        let mut es = Vec::new();
        EsDescriptor {
            es_id: 0x102,
            decoder: DecoderConfig {
                object_type: object_type::AUDIO_14496_3,
                stream_type: stream_type::AUDIO,
                ..DecoderConfig::default()
            },
            sl: timed(),
            ..EsDescriptor::default()
        }
        .write(&mut es)
        .unwrap();
        let mut od = vec![0x01, 2 + es.len() as u8, 0x00, 0xDF];
        od.extend_from_slice(&es);
        let mut update = vec![0x01, od.len() as u8];
        update.extend_from_slice(&od);
        let remove = [0x02, 0x03, 0x00, 0xC0, 0x10];
        let mut all = update.clone();
        all.extend_from_slice(&remove);
        let mut cmds = Vec::new();
        OdCommand::parse_all(&all, &mut cmds).unwrap();
        assert_eq!(cmds.len(), 2);
        let OdCommand::Update(ods) = &cmds[0] else {
            panic!("an update")
        };
        assert_eq!((ods[0].id, ods[0].es[0].es_id), (3, 0x102));
        assert_eq!(cmds[1], OdCommand::Remove(vec![3, 1]));
        let mut v = Vec::new();
        for c in &cmds {
            c.write(&mut v).unwrap();
        }
        assert_eq!(v, all);
    }

    /// 13818-1 2.6.42 and 2.6.44, and an ISO_IEC_14496_section with its CRC-32/MPEG-2.
    #[test]
    fn pmt_descriptors_and_sections() {
        assert_eq!(sl_descriptor(&[0x01, 0x01]), Some(0x101));
        assert_eq!(sl_descriptor(&[0x01]), None);
        let f: Vec<_> = fmc_descriptor(&[0x01, 0x01, 0, 0x01, 0x02, 1, 9]).collect();
        assert_eq!(f, [(0x101, 0), (0x102, 1)]);
        let s = build_section(TABLE_OBJECT_DESCRIPTOR, 1, 0, &[0x80, 1, 2]);
        assert_eq!(&s[..3], &[0x05, 0xB0, 12]);
        assert_eq!(section_payload(&s), Ok((0x05, &[0x80, 1, 2][..])));
        let mut bad = s.clone();
        bad[9] ^= 1;
        assert_eq!(section_payload(&bad), Err(Error::Mpeg4(Mpeg4Fault::Short)));
        let pat = crate::tables::build_pat(1, 0, &[]);
        assert_eq!(
            section_payload(&pat),
            Err(Error::Mpeg4(Mpeg4Fault::Field(Mpeg4Field::TableId)))
        );
    }
}
