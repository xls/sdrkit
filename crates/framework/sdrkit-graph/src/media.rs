//! Media: codec descriptions for compressed audio, video and pictures, raw pixel formats, packets, and the picture and
//! video-frame items. The types are defined in `sdrkit-core` and re-exported here.
//!
//! A `Bytes` stream of compressed media carries a [`MediaFormat`] in its [`Format`](crate::Format) (`Format::media`); each packet
//! carries its pts, dts and keyframe flag ([`Packet`]). A decoder's input port lists the codecs and packagings it accepts
//! ([`MediaSpec`]). A graph output of compressed media or of [`Picture`] items reaches the host through
//! [`Output::media`](sdrkit_core::Output::media).

pub use sdrkit_core::media::*;

impl crate::stream::Item for Picture {
    const KIND: &'static str = "picture";
}

/// The format a [`Picture`] goes to the host with: its codec, packaged raw, and for raw pixels the pixel layout.
///
/// ```
/// use std::sync::Arc;
/// use sdrkit_graph::media::{picture_format, Codec, Picture, PixelFormat, PixelLayout};
/// let p = Picture {
///     codec: Codec::Pixels(PixelFormat::Grey8),
///     layout: Some(PixelLayout::packed(PixelFormat::Grey8, 4, 2)),
///     name: None,
///     id: 1,
///     state: sdrkit_graph::PictureState::Finished,
///     data: Arc::from(&[0u8; 8][..]),
/// };
/// let f = picture_format(&p);
/// assert_eq!((f.codec, f.pixels.map(|l| l.width)), (Codec::Pixels(PixelFormat::Grey8), Some(4)));
/// ```
pub fn picture_format(p: &Picture) -> MediaFormat {
    let mut f = MediaFormat::new(p.codec, Packaging::Raw);
    if let Codec::Pixels(_) = p.codec {
        f.pixels = p.layout.clone();
    }
    f
}

impl crate::stream::Item for VideoFrame {
    const KIND: &'static str = "video.frame";
}

use std::sync::{Arc, OnceLock};

/// One packet of compressed media with its own format, as a typed item (`media.packet`). Use it for a graph output whose format
/// changes while the graph runs: a DAB+ service change returns a new AudioSpecificConfig, a new service provides new video
/// properties. A `Bytes` output has one format, fixed at configure. Each `MediaPacket` carries its format, so the format can
/// change from one packet to the next.
///
/// A graph output of `MediaPacket` items reaches the host through [`Output::media`](sdrkit_core::Output::media): each packet goes
/// with its own format, both shared and not copied. A packet whose codec the host does not decode ([`Codec::stream_kind`] returns `None`)
/// stays in the graph.
///
/// ```
/// use std::sync::Arc;
/// use sdrkit_graph::media::{MediaFormat, MediaPacket, Packaging, Packet};
/// let aac = Arc::new(MediaFormat::new(sdrkit_graph::media::Codec::HeAac, Packaging::Raw));
/// let p = MediaPacket::new(Packet::new(Arc::from(&[0x21u8, 0x10][..]), 0.12), aac);
/// assert_eq!(p.stream_kind(), Some(sdrkit_core::StreamKind::HeAac));
/// ```
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub struct MediaPacket {
    /// The packet: its bytes, time stamps and keyframe flag.
    pub packet: Packet,
    /// The format of this packet and of the packets after it, until one carries another.
    pub format: Arc<MediaFormat>,
}

impl MediaPacket {
    /// A packet with its format. Share one `Arc` among the packets of one format, so passing them on allocates nothing.
    pub fn new(packet: Packet, format: Arc<MediaFormat>) -> MediaPacket {
        MediaPacket { packet, format }
    }

    /// The host decoder kind of the packet's codec, `None` when the host does not decode it. Raw pixels with video
    /// properties are decoded video.
    pub fn stream_kind(&self) -> Option<sdrkit_core::StreamKind> {
        match self.format.codec {
            sdrkit_core::media::Codec::Pixels(_) if self.format.video.is_some() => {
                Some(sdrkit_core::StreamKind::RawVideo)
            }
            codec => codec.stream_kind(),
        }
    }
}

impl crate::stream::Item for MediaPacket {
    const KIND: &'static str = "media.packet";
}

/// Where a speech stream comes from. A port of [`SpeechPacket`] items can carry several streams, one per speaker: a DMR or
/// P25 Phase 2 slot, a TETRA timeslot. A codec sink keeps one decoder state for each channel.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct SpeechChannel {
    /// The id of the stack (`"dmr"`, `"p25"`).
    pub stack: &'static str,
    /// The carrier frequency, Hz.
    pub carrier_hz: f64,
    /// The slot or logical channel.
    pub slot: u8,
}

/// The bit layout of the payload of a [`SpeechPacket`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SpeechPackaging {
    /// The codec's own codewords, as the codec defines them.
    Codewords,
    /// The raw codec frame, with no channel coding.
    Raw,
    /// The 72-bit D-STAR voice frame.
    Dstar72,
    /// The 144-bit P25 phase 1 IMBE frame.
    P25_144,
    /// The 137-bit TETRA traffic channel frame.
    Tchs137,
    /// The 49 information bits of a YSF voice frame.
    Info49,
    /// The 120-bit TETRAPOL speech frame.
    Tp120,
}

impl SpeechPackaging {
    /// The key that a codec plug-in declares for this layout ([`CodecSet::find`](sdrkit_core::codec::CodecSet::find)).
    pub fn key(self) -> &'static str {
        match self {
            SpeechPackaging::Codewords => "codewords",
            SpeechPackaging::Raw => "raw",
            SpeechPackaging::Dstar72 => "dstar72",
            SpeechPackaging::P25_144 => "p25_144",
            SpeechPackaging::Tchs137 => "tchs137",
            SpeechPackaging::Info49 => "info49",
            SpeechPackaging::Tp120 => "tp120",
        }
    }
}

/// The bit rate of a Codec 2 frame.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Codec2Mode {
    /// 3200 bit/s.
    M3200,
    /// 1600 bit/s.
    M1600,
    /// 1300 bit/s.
    M1300,
}

impl Codec2Mode {
    /// The name of the Codec 2 mode, as Codec 2 names it (`"3200"`).
    pub fn key(self) -> &'static str {
        match self {
            Codec2Mode::M3200 => "3200",
            Codec2Mode::M1600 => "1600",
            Codec2Mode::M1300 => "1300",
        }
    }
}

/// One compressed speech frame for a codec plug-in, as a typed item (`media.speech`).
///
/// A frame carries its payload, the codec (FourCC) and the bit layout of the payload (`packaging`, the frame layout of
/// [`CodecSet::find`](sdrkit_core::codec::CodecSet::find)), the frame duration, the channel, and the call it belongs to. The
/// packet's `pts` is the signal time of the frame's first symbol, in 8 kHz samples. A frame whose own error check failed is
/// [`damaged`](SpeechPacket::damaged): it still goes to the decoder, whose FEC and concealment act on it. A frame position
/// with no frame (a lost burst, a stolen slot, a pause inside a call) is an [`erasure`](SpeechPacket::erasure) with no
/// payload, so the decoder conceals it and the sound keeps its timing.
///
/// ```
/// use std::sync::Arc;
/// use sdrkit_core::FourCc;
/// use sdrkit_graph::media::{Packet, SpeechPackaging, SpeechPacket};
/// let words = Packet::new(Arc::from(&[0u8; 12][..]), 1.0).stamped(Some(8000), None, true);
/// let f = SpeechPacket::frame(FourCc::new("amb2"), SpeechPackaging::Codewords, 20, words).damaged(Some(3)).in_call(7);
/// assert!(f.damaged && !f.erasure);
/// assert_eq!((f.bit_errors, f.call_id, f.samples_8k()), (Some(3), 7, 160));
/// let gap = SpeechPacket::erasure(FourCc::new("amb2"), SpeechPackaging::Codewords, 20, 8160, 1.02);
/// assert!(gap.erasure && gap.payload().is_empty());
/// ```
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub struct SpeechPacket {
    /// The codec's four-character code (`amb2`, `imbe`, `acel`).
    pub fourcc: FourCc,
    /// The bit layout of the payload.
    pub packaging: SpeechPackaging,
    /// The Codec 2 mode, for a Codec 2 frame.
    pub codec_mode: Option<Codec2Mode>,
    /// The frame duration in milliseconds (20, 30 or 40).
    pub duration_ms: u16,
    /// Where the stream comes from.
    pub channel: SpeechChannel,
    /// The payload, with `pts` in 8 kHz samples and the signal time. Empty for an erasure.
    pub packet: Packet,
    /// The frame's own error check failed.
    pub damaged: bool,
    /// No frame was received at this position: the decoder conceals it.
    pub erasure: bool,
    /// The number of bit errors the stack corrected or detected in the frame, if it counts them.
    pub bit_errors: Option<u16>,
    /// The call the frame belongs to (0 when the mode has no calls).
    pub call_id: u64,
}

/// The empty payload of every erasure, made once.
fn no_payload() -> Arc<[u8]> {
    static EMPTY: OnceLock<Arc<[u8]>> = OnceLock::new();
    EMPTY.get_or_init(|| Arc::from(&[][..])).clone()
}

impl SpeechPacket {
    /// A received frame of `fourcc` in `packaging`, `duration_ms` long, with its payload in `packet`.
    pub fn frame(
        fourcc: FourCc,
        packaging: SpeechPackaging,
        duration_ms: u16,
        packet: Packet,
    ) -> SpeechPacket {
        SpeechPacket {
            fourcc,
            packaging,
            codec_mode: None,
            duration_ms,
            channel: SpeechChannel::default(),
            packet,
            damaged: false,
            erasure: false,
            bit_errors: None,
            call_id: 0,
        }
    }

    /// An erasure: the position of a frame of `duration_ms` at `pts` (8 kHz samples) and signal time `time` with no frame. It
    /// has no payload.
    pub fn erasure(
        fourcc: FourCc,
        packaging: SpeechPackaging,
        duration_ms: u16,
        pts: i64,
        time: f64,
    ) -> SpeechPacket {
        let packet = Packet::new(no_payload(), time).stamped(Some(pts), None, true);
        SpeechPacket {
            erasure: true,
            ..SpeechPacket::frame(fourcc, packaging, duration_ms, packet)
        }
    }

    /// The same frame, marked as damaged, with the bit error count if the stack has one.
    pub fn damaged(mut self, bit_errors: Option<u16>) -> SpeechPacket {
        self.damaged = true;
        self.bit_errors = bit_errors;
        self
    }

    /// The same frame, with the bit errors of a frame that passed its check.
    pub fn with_bit_errors(mut self, bit_errors: u16) -> SpeechPacket {
        self.bit_errors = Some(bit_errors);
        self
    }

    /// The same frame, in call `call_id`.
    pub fn in_call(mut self, call_id: u64) -> SpeechPacket {
        self.call_id = call_id;
        self
    }

    /// The same frame, from `channel`.
    pub fn on(mut self, channel: SpeechChannel) -> SpeechPacket {
        self.channel = channel;
        self
    }

    /// The same frame, in codec mode `mode`.
    pub fn with_codec_mode(mut self, mode: Codec2Mode) -> SpeechPacket {
        self.codec_mode = Some(mode);
        self
    }

    /// The payload bytes (empty for an erasure).
    pub fn payload(&self) -> &[u8] {
        if self.erasure {
            &[]
        } else {
            self.packet.bytes()
        }
    }

    /// The 8 kHz samples the frame stands for.
    pub fn samples_8k(&self) -> usize {
        self.duration_ms as usize * 8
    }
}

impl crate::stream::Item for SpeechPacket {
    const KIND: &'static str = "media.speech";
}
