//! Media: codec descriptions for compressed audio, video and pictures, raw pixel formats, and the picture and video-frame items.
//!
//! Each codec and pixel format is a [`Codec`] with one row in [`CODECS`]: a stable id, the four-character code that platform
//! decoders use, and a name. Packets, pictures and video frames hold shared bytes, so passing them on never copies them.

use serde::{Deserialize, Serialize};
use std::fmt;
use std::sync::Arc;

/// A four-character code.
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct FourCc(pub [u8; 4]);

impl FourCc {
    /// From four ASCII characters (`FourCc::new("avc1")`); shorter text is padded with spaces.
    pub const fn new(code: &str) -> FourCc {
        let b = code.as_bytes();
        let mut out = [b' '; 4];
        let mut i = 0;
        while i < 4 && i < b.len() {
            out[i] = b[i];
            i += 1;
        }
        FourCc(out)
    }
    /// As text.
    pub fn as_str(&self) -> &str {
        std::str::from_utf8(&self.0).unwrap_or("????")
    }
    /// As the big-endian number platform APIs use (`'avc1'`).
    pub fn to_u32(self) -> u32 {
        u32::from_be_bytes(self.0)
    }
}

impl fmt::Debug for FourCc {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "'{}'", self.as_str())
    }
}

impl fmt::Display for FourCc {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// What a codec or pixel format is for.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MediaType {
    /// Compressed video.
    Video,
    /// Compressed or PCM audio.
    Audio,
    /// A compressed still picture.
    Picture,
    /// Uncompressed pixels (a decoded frame, an SSTV or fax line).
    Pixels,
    /// A container of other streams (a transport stream).
    Data,
}

/// One row of the codec table: a codec or pixel format, its stable id, its four-character code and its name.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CodecInfo {
    /// The codec.
    pub codec: Codec,
    /// The stable id.
    pub id: &'static str,
    /// The per-codec four-character code.
    pub fourcc: FourCc,
    /// What it is for.
    pub media: MediaType,
    /// A name for people.
    pub name: &'static str,
}

macro_rules! codec_table {
    (
        codecs { $($(#[$cm:meta])* $c:ident = ($cid:literal, $cfc:literal, $cmedia:ident, $cname:literal)),+ $(,)? }
        pixels { $($(#[$pm:meta])* $p:ident = ($pid:literal, $pfc:literal, $pname:literal)),+ $(,)? }
    ) => {
        /// A layout of uncompressed pixels.
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
        pub enum PixelFormat {
            $($(#[$pm])* $p),+
        }

        /// A codec, or raw pixels of one [`PixelFormat`]. [`CODECS`] holds one row for each.
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
        pub enum Codec {
            $($(#[$cm])* $c,)+
            /// Uncompressed pixels.
            Pixels(PixelFormat),
        }

        /// Every codec and pixel format that sdrkit defines, with its four-character code.
        pub const CODECS: &[CodecInfo] = &[
            $(CodecInfo { codec: Codec::$c, id: $cid, fourcc: FourCc::new($cfc), media: MediaType::$cmedia, name: $cname },)+
            $(CodecInfo {
                codec: Codec::Pixels(PixelFormat::$p),
                id: $pid,
                fourcc: FourCc::new($pfc),
                media: MediaType::Pixels,
                name: $pname,
            },)+
        ];

        #[allow(clippy::upper_case_acronyms)]
        enum Row {
            $($c),+
        }
        const PLAIN: usize = [$(Row::$c),+].len();

        impl Codec {
            /// Its row of [`CODECS`].
            pub const fn info(self) -> &'static CodecInfo {
                match self {
                    $(Codec::$c => &CODECS[Row::$c as usize],)+
                    Codec::Pixels(p) => &CODECS[PLAIN + p as usize],
                }
            }
        }
    };
}

codec_table! {
    codecs {
        /// H.264 / AVC.
        H264 = ("h264", "avc1", Video, "H.264 / AVC"),
        /// H.265 / HEVC.
        Hevc = ("hevc", "hvc1", Video, "H.265 / HEVC"),
        /// MPEG-2 video.
        Mpeg2Video = ("mpeg2video", "mp2v", Video, "MPEG-2 video"),
        /// MPEG-1 video.
        Mpeg1Video = ("mpeg1video", "mp1v", Video, "MPEG-1 video"),
        /// AVS (GB/T 20090.2).
        Avs = ("avs", "avs1", Video, "AVS"),
        /// AVS2.
        Avs2 = ("avs2", "avs2", Video, "AVS2"),
        /// AVS3.
        Avs3 = ("avs3", "avs3", Video, "AVS3"),
        /// AAC-LC.
        Aac = ("aac", "aac ", Audio, "AAC-LC"),
        /// HE-AAC.
        HeAac = ("he_aac", "aach", Audio, "HE-AAC"),
        /// HE-AAC v2.
        HeAacV2 = ("he_aac_v2", "aacp", Audio, "HE-AAC v2"),
        /// xHE-AAC (USAC).
        XheAac = ("xhe_aac", "usac", Audio, "xHE-AAC (USAC)"),
        /// MPEG-1 Layer I.
        Mp1 = ("mp1", ".mp1", Audio, "MPEG-1 Layer I"),
        /// MPEG-1 Layer II.
        Mp2 = ("mp2", ".mp2", Audio, "MPEG-1 Layer II"),
        /// MPEG-1 Layer III.
        Mp3 = ("mp3", ".mp3", Audio, "MPEG-1 Layer III"),
        /// AC-3.
        Ac3 = ("ac3", "ac-3", Audio, "AC-3"),
        /// E-AC-3.
        EAc3 = ("eac3", "ec-3", Audio, "E-AC-3"),
        /// AC-4.
        Ac4 = ("ac4", "ac-4", Audio, "AC-4"),
        /// DTS.
        Dts = ("dts", "dtsc", Audio, "DTS"),
        /// DRA.
        Dra = ("dra", "dra1", Audio, "DRA"),
        /// Opus.
        Opus = ("opus", "Opus", Audio, "Opus"),
        /// Linear PCM.
        Pcm = ("pcm", "lpcm", Audio, "Linear PCM"),
        /// AMBE. Only a plug-in that the host registers decodes it.
        Ambe = ("ambe", "ambe", Audio, "AMBE"),
        /// AMBE+2. Only a plug-in that the host registers decodes it.
        Ambe2 = ("ambe2", "amb2", Audio, "AMBE+2"),
        /// IMBE. Only a plug-in that the host registers decodes it.
        Imbe = ("imbe", "imbe", Audio, "IMBE"),
        /// TETRA ACELP. Only a plug-in that the host registers decodes it.
        TetraAcelp = ("tetra_acelp", "acel", Audio, "TETRA ACELP"),
        /// TETRAPOL RP-CELP. Only a plug-in that the host registers decodes it.
        TetrapolRpcelp = ("tetrapol_rpcelp", "rpce", Audio, "TETRAPOL RP-CELP"),
        /// Codec 2. Only a plug-in that the host registers decodes it.
        Codec2 = ("codec2", "cod2", Audio, "Codec 2"),
        /// RADE. Only a plug-in that the host registers decodes it.
        Rade = ("rade", "rade", Audio, "RADE"),
        /// JPEG.
        Jpeg = ("jpeg", "jpeg", Picture, "JPEG"),
        /// PNG.
        Png = ("png", "png ", Picture, "PNG"),
        /// JPEG 2000.
        Jpeg2000 = ("jpeg2000", "jp2 ", Picture, "JPEG 2000"),
        /// HEIC.
        Heic = ("heic", "heic", Picture, "HEIC"),
        /// GIF.
        Gif = ("gif", "gif ", Picture, "GIF"),
        /// An MPEG transport stream (ISO/IEC 13818-1).
        MpegTs = ("mp2t", "mp2t", Data, "MPEG transport stream"),
    }
    pixels {
        /// NV12, video range.
        Nv12 = ("nv12", "420v", "NV12, video range"),
        /// NV12, full range.
        Nv12Full = ("nv12_full", "420f", "NV12, full range"),
        /// BGRA, 8 bits each.
        Bgra = ("bgra", "BGRA", "BGRA, 8 bits each"),
        /// I420 (planar 4:2:0).
        I420 = ("i420", "I420", "I420 (planar 4:2:0)"),
        /// YUY2 (packed 4:2:2).
        Yuy2 = ("yuy2", "YUY2", "YUY2 (packed 4:2:2)"),
        /// 8-bit grey.
        Grey8 = ("grey8", "L008", "8-bit grey"),
        /// RGB, 8 bits each, 3 bytes a pixel.
        Rgb8 = ("rgb8", "RGB3", "RGB, 8 bits each"),
    }
}
impl Codec {
    /// The stable id (`"h264"`).
    pub const fn id(self) -> &'static str {
        self.info().id
    }
    /// The per-codec four-character code.
    pub const fn fourcc(self) -> FourCc {
        self.info().fourcc
    }
    /// What it is for.
    pub const fn media(self) -> MediaType {
        self.info().media
    }
    /// A name for people.
    pub const fn name(self) -> &'static str {
        self.info().name
    }
    /// The codec or pixel format with this id. This is the one lookup by text.
    pub fn from_id(id: &str) -> Option<Codec> {
        CODECS.iter().find(|c| c.id == id).map(|c| c.codec)
    }
    /// The codec or pixel format with this four-character code.
    pub fn from_fourcc(code: FourCc) -> Option<Codec> {
        CODECS.iter().find(|c| c.fourcc == code).map(|c| c.codec)
    }
    /// The kind of host decoder for this codec (see [`Output::media`](crate::Output::media)). `None` for a codec that needs no
    /// host decoder (PCM, speech codecs, raw pixels).
    pub const fn stream_kind(self) -> Option<crate::StreamKind> {
        use crate::StreamKind as S;
        Some(match self {
            Codec::H264 => S::H264,
            Codec::Hevc => S::Hevc,
            Codec::Mpeg2Video => S::Mpeg2Video,
            Codec::Aac => S::Aac,
            Codec::HeAac | Codec::HeAacV2 => S::HeAac,
            Codec::XheAac => S::XheAac,
            Codec::Ac3 => S::Ac3,
            Codec::EAc3 => S::EAc3,
            Codec::Mp1 => S::Mp1,
            Codec::Mp2 => S::Mp2,
            Codec::Mp3 => S::Mp3,
            Codec::Jpeg | Codec::Png | Codec::Jpeg2000 | Codec::Heic | Codec::Gif => S::Picture,
            Codec::MpegTs => S::MpegTs,
            Codec::Mpeg1Video
            | Codec::Avs
            | Codec::Avs2
            | Codec::Avs3
            | Codec::Ac4
            | Codec::Dts
            | Codec::Dra
            | Codec::Opus
            | Codec::Pcm
            | Codec::Ambe
            | Codec::Ambe2
            | Codec::Imbe
            | Codec::TetraAcelp
            | Codec::TetrapolRpcelp
            | Codec::Codec2
            | Codec::Rade
            | Codec::Pixels(_) => return None,
        })
    }
}

impl PixelFormat {
    /// Its codec row.
    pub const fn codec(self) -> Codec {
        Codec::Pixels(self)
    }
    /// The rows of each plane after the first, for a frame `h` rows high.
    fn plane_rows(self, h: usize) -> usize {
        match self {
            PixelFormat::Nv12 | PixelFormat::Nv12Full | PixelFormat::I420 => h.div_ceil(2),
            PixelFormat::Bgra | PixelFormat::Yuy2 | PixelFormat::Grey8 | PixelFormat::Rgb8 => h,
        }
    }
}

impl fmt::Display for Codec {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.id())
    }
}

/// A codec serialises as its id.
impl Serialize for Codec {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(self.id())
    }
}

impl<'de> Deserialize<'de> for Codec {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let id = <std::borrow::Cow<'de, str>>::deserialize(d)?;
        Codec::from_id(&id)
            .ok_or_else(|| serde::de::Error::custom(format_args!("unknown codec {id}")))
    }
}

/// A pixel format serialises as its id.
impl Serialize for PixelFormat {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(self.codec().id())
    }
}

impl<'de> Deserialize<'de> for PixelFormat {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        match Codec::deserialize(d)? {
            Codec::Pixels(p) => Ok(p),
            c => Err(serde::de::Error::custom(format_args!(
                "{c} is not a pixel format"
            ))),
        }
    }
}

/// How a codec's packets are framed.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Packaging {
    /// H.264 / HEVC with start codes.
    AnnexB,
    /// H.264 / HEVC with a big-endian length of this many bytes before each NAL unit (avcC / hvcC style).
    LengthPrefixed(u8),
    /// AAC with ADTS headers.
    Adts,
    /// AAC in LATM / LOAS.
    Latm,
    /// Bare access units (the decoder configuration comes separately).
    Raw,
}

/// A colour matrix.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ColourMatrix {
    /// ITU-R BT.601 (standard definition).
    Bt601,
    /// ITU-R BT.709 (high definition).
    #[default]
    Bt709,
    /// ITU-R BT.2020.
    Bt2020,
    /// RGB or grey: no matrix.
    Identity,
}

/// The layout of uncompressed pixels.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PixelLayout {
    /// The pixel format.
    pub pixel: PixelFormat,
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
    /// Bytes per row of each plane, in plane order.
    pub strides: Vec<usize>,
    /// The colour matrix.
    pub matrix: ColourMatrix,
    /// Full range (0..255) rather than video range (16..235).
    pub full_range: bool,
}

impl PixelLayout {
    /// A tightly packed layout of `pixel`, `width` by `height`.
    pub fn packed(pixel: PixelFormat, width: u32, height: u32) -> PixelLayout {
        let w = width as usize;
        let (strides, matrix, full) = match pixel {
            PixelFormat::Nv12 => (vec![w, w.div_ceil(2) * 2], ColourMatrix::Bt709, false),
            PixelFormat::Nv12Full => (vec![w, w.div_ceil(2) * 2], ColourMatrix::Bt709, true),
            PixelFormat::I420 => (
                vec![w, w.div_ceil(2), w.div_ceil(2)],
                ColourMatrix::Bt709,
                false,
            ),
            PixelFormat::Yuy2 => (vec![w.div_ceil(2) * 4], ColourMatrix::Bt709, false),
            PixelFormat::Bgra => (vec![w * 4], ColourMatrix::Identity, true),
            PixelFormat::Grey8 => (vec![w], ColourMatrix::Identity, true),
            PixelFormat::Rgb8 => (vec![w * 3], ColourMatrix::Identity, true),
        };
        PixelLayout {
            pixel,
            width,
            height,
            strides,
            matrix,
            full_range: full,
        }
    }
    /// Bytes of a frame in this layout.
    pub fn frame_bytes(&self) -> usize {
        let h = self.height as usize;
        self.strides
            .iter()
            .enumerate()
            .map(|(i, s)| s * if i == 0 { h } else { self.pixel.plane_rows(h) })
            .sum()
    }
}

/// Video properties.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct VideoProps {
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
    /// Frames per second as a fraction, when known.
    pub frame_rate: Option<(u32, u32)>,
    /// The display aspect ratio, when known (16:9 as `(16, 9)`).
    pub aspect: Option<(u32, u32)>,
    /// Decoded frames: the two fields of a frame are from different times.
    #[serde(default)]
    pub interlaced: bool,
    /// Decoded frames: the top field is the first in time.
    #[serde(default)]
    pub top_field_first: bool,
    /// Decoded frames: the stream lost or damaged pictures, and the frame is the last good one again.
    #[serde(default)]
    pub damaged: bool,
}

/// The meaning and the order of the channels of interleaved audio.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[repr(u32)]
pub enum ChannelLayout {
    /// An order that the library does not know.
    #[default]
    Unknown = 0,
    /// One channel.
    Mono = 1,
    /// Left, right.
    Stereo = 2,
    /// 5.1: left, centre, right, left surround, right surround, low-frequency effects.
    Surround51 = 3,
}

impl ChannelLayout {
    /// The layout of `channels` channels when the source says nothing more: mono for 1, stereo for 2, otherwise unknown.
    pub const fn of_count(channels: u16) -> Self {
        match channels {
            1 => ChannelLayout::Mono,
            2 => ChannelLayout::Stereo,
            _ => ChannelLayout::Unknown,
        }
    }

    /// The number of channels of the layout; `None` for an unknown layout.
    pub const fn channels(self) -> Option<u16> {
        match self {
            ChannelLayout::Unknown => None,
            ChannelLayout::Mono => Some(1),
            ChannelLayout::Stereo => Some(2),
            ChannelLayout::Surround51 => Some(6),
        }
    }
}

/// Audio properties.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct AudioProps {
    /// Sample rate in Hz.
    pub sample_rate: u32,
    /// Number of channels that the decoder delivers. A stream of more channels is mixed down to stereo when this is 2.
    pub channels: u8,
}

/// Everything a decoder needs to know about a compressed stream (or a host needs to show raw pixels).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MediaFormat {
    /// The codec.
    pub codec: Codec,
    /// How packets are framed.
    pub packaging: Packaging,
    /// Decoder configuration (SPS/PPS/VPS, AudioSpecificConfig); a `ConfigChanged` tag marks a change mid-stream.
    #[serde(with = "arc_bytes")]
    pub config: Arc<[u8]>,
    /// Units of pts and dts, as a fraction of a second (`(1, 90000)` for MPEG).
    pub timebase: (u32, u32),
    /// For video.
    pub video: Option<VideoProps>,
    /// For audio.
    pub audio: Option<AudioProps>,
    /// For raw pixels.
    pub pixels: Option<PixelLayout>,
    /// The container's code, when it differs from the codec's (`mp4a`).
    pub container_code: Option<FourCc>,
}

impl MediaFormat {
    /// A stream of `codec` packaged as `packaging`, with a 90 kHz timebase and nothing else known.
    pub fn new(codec: Codec, packaging: Packaging) -> MediaFormat {
        MediaFormat {
            codec,
            packaging,
            config: Arc::from(&[][..]),
            timebase: (1, 90_000),
            video: None,
            audio: None,
            pixels: None,
            container_code: None,
        }
    }
    /// Its per-codec four-character code.
    pub fn fourcc(&self) -> FourCc {
        self.codec.fourcc()
    }
    /// Whether `spec` accepts this stream.
    pub fn matches(&self, spec: &MediaSpec) -> bool {
        spec.codec == self.codec && spec.packaging.is_none_or(|p| p == self.packaging)
    }
}

mod arc_bytes {
    use serde::{Deserialize, Deserializer, Serialize, Serializer};
    use std::sync::Arc;
    pub fn serialize<S: Serializer>(v: &Arc<[u8]>, s: S) -> Result<S::Ok, S::Error> {
        v.as_ref().serialize(s)
    }
    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Arc<[u8]>, D::Error> {
        Ok(Vec::<u8>::deserialize(d)?.into())
    }
}

/// A codec (and optionally one packaging) a port produces or accepts.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MediaSpec {
    /// The codec.
    pub codec: Codec,
    /// The packaging, or any.
    pub packaging: Option<Packaging>,
}

impl MediaSpec {
    /// `codec` in `packaging`.
    pub const fn new(codec: Codec, packaging: Packaging) -> MediaSpec {
        MediaSpec {
            codec,
            packaging: Some(packaging),
        }
    }
    /// `codec` in any packaging.
    pub const fn any(codec: Codec) -> MediaSpec {
        MediaSpec {
            codec,
            packaging: None,
        }
    }
    /// Whether a producer of `self` can feed a consumer accepting `other`.
    pub fn fits(&self, other: &MediaSpec) -> bool {
        self.codec == other.codec
            && match (self.packaging, other.packaging) {
                (Some(a), Some(b)) => a == b,
                _ => true,
            }
    }
}

impl fmt::Display for MediaSpec {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.packaging {
            Some(p) => write!(f, "{} {p:?}", self.codec),
            None => write!(f, "{}", self.codec),
        }
    }
}

/// One packet of compressed media (or of any byte stream). Its bytes are reference-counted, so passing a packet on (or tapping it)
/// never copies them, and many packets may share one allocation (`start`, `len` select the packet's part).
#[derive(Clone, Debug, PartialEq)]
pub struct Packet {
    /// The shared bytes.
    pub data: Arc<[u8]>,
    /// Where the packet starts in `data`.
    pub start: usize,
    /// Its length.
    pub len: usize,
    /// Presentation time stamp, in the stream's timebase ([`MediaFormat::timebase`]).
    pub pts: Option<i64>,
    /// Decoding time stamp, in the same timebase.
    pub dts: Option<i64>,
    /// A random access point (an IDR frame, an audio frame that decodes alone).
    pub keyframe: bool,
    /// A random access point after which nothing refers to data before it: an H.264 or HEVC IDR picture, an HEVC BLA
    /// picture, a decoded frame, an audio frame. False for a recovery point, a CRA picture or the I picture of an open
    /// MPEG-2 group, where a decoder that starts there can show damage until the next clean start.
    pub clean_start: bool,
    /// Seconds in the signal's own time.
    pub time: f64,
}

impl Packet {
    /// A packet of all of `data` at signal time `time`, with no time stamps.
    pub fn new(data: Arc<[u8]>, time: f64) -> Packet {
        Packet {
            start: 0,
            len: data.len(),
            data,
            pts: None,
            dts: None,
            keyframe: false,
            clean_start: false,
            time,
        }
    }
    /// A packet of `data[start..start + len]`.
    ///
    /// # Panics
    /// When the range lies outside `data`.
    pub fn slice(data: &Arc<[u8]>, start: usize, len: usize, time: f64) -> Packet {
        assert!(start + len <= data.len(), "packet outside its bytes");
        Packet {
            data: data.clone(),
            start,
            len,
            pts: None,
            dts: None,
            keyframe: false,
            clean_start: false,
            time,
        }
    }
    /// With time stamps. A keyframe is a clean start; [`Packet::clean_if`] can say otherwise.
    pub fn stamped(mut self, pts: Option<i64>, dts: Option<i64>, keyframe: bool) -> Packet {
        (self.pts, self.dts, self.keyframe, self.clean_start) = (pts, dts, keyframe, keyframe);
        self
    }
    /// The keyframe as a clean start only if `clean`.
    #[must_use]
    pub fn clean_if(mut self, clean: bool) -> Packet {
        self.clean_start = self.keyframe && clean;
        self
    }
    /// The packet's bytes.
    pub fn bytes(&self) -> &[u8] {
        &self.data[self.start..self.start + self.len]
    }
}

/// One picture: a compressed image (JPEG, PNG ...) or raw pixels, with its format. The standard typed item for `"picture"` streams
/// (a DAB slideshow, weather-satellite imagery, data-mode pictures). The bytes are shared, so passing a picture on never copies it.
#[derive(Clone, Debug, PartialEq)]
pub struct Picture {
    /// The codec ([`Codec::Jpeg`], [`Codec::Png`] ...) or, for raw pixels, [`Codec::Pixels`].
    pub codec: Codec,
    /// For raw pixels, their layout; for a compressed picture, its size when known.
    pub layout: Option<PixelLayout>,
    /// A name, when the signal provides one (a slideshow file name) or the component has one (a product, a channel).
    pub name: Option<String>,
    /// The number of the picture among the pictures of its component. Items with the same number are states of one
    /// picture, and each replaces the one before. A component starts a new number for a new picture, and keeps one
    /// number for each product or channel that it builds at the same time. [`Picture::WHOLE`] for a picture that is
    /// sent once, whole.
    pub id: u32,
    /// Whether more of the picture can come.
    pub state: PictureState,
    /// The bytes.
    pub data: Arc<[u8]>,
}

impl Picture {
    /// The number of a picture that is sent once, whole, in the state [`PictureState::Finished`]: a slide, a
    /// received file, a frame of moving pictures. Every such item is a picture of its own for the host.
    pub const WHOLE: u32 = 0;
}

/// Whether more of a picture can come.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PictureState {
    /// The picture can change: more rows, more parts, or better data for the same rows. A later item with the same
    /// number replaces this one.
    #[default]
    Growing,
    /// The picture is complete or its signal ended. A later item with the same number is the same picture drawn
    /// again, after a change of a setting of its view.
    Finished,
}

/// The numbers of the pictures of a component: one for each key (a product, a channel, a file), new for a key that
/// is not open. A component that builds one picture at a time uses the key `()`.
///
/// ```
/// use sdrkit_core::media::PictureIds;
/// let mut ids = PictureIds::new();
/// let (a, b) = (ids.of(&"ch1"), ids.of(&"ch2"));
/// assert_eq!((ids.of(&"ch1"), ids.of(&"ch2")), (a, b));
/// assert_eq!(ids.close(&"ch1"), Some(a));
/// assert_ne!(ids.of(&"ch1"), a);
/// ```
#[derive(Clone, Debug)]
pub struct PictureIds<K> {
    open: Vec<(K, u32)>,
    next: u32,
}

impl<K> Default for PictureIds<K> {
    fn default() -> Self {
        PictureIds {
            open: Vec::new(),
            next: 1,
        }
    }
}

impl<K: PartialEq + Clone> PictureIds<K> {
    /// No picture open. The first number is 1.
    pub fn new() -> Self {
        Self::default()
    }
    /// The number of the open picture of `key`. A key without an open picture gets a new number.
    pub fn of(&mut self, key: &K) -> u32 {
        if let Some((_, id)) = self.open.iter().find(|(k, _)| k == key) {
            return *id;
        }
        let id = self.next;
        self.next = self.next.wrapping_add(1).max(1);
        self.open.push((key.clone(), id));
        id
    }
    /// The number of the open picture of `key`, if it has one. It opens none.
    pub fn open(&self, key: &K) -> Option<u32> {
        self.open.iter().find(|(k, _)| k == key).map(|(_, id)| *id)
    }
    /// Ends the picture of `key` and returns its number: the number for the item that states
    /// [`PictureState::Finished`]. The next picture of the key gets a new number. `None` if the key has no open picture.
    pub fn close(&mut self, key: &K) -> Option<u32> {
        let i = self.open.iter().position(|(k, _)| k == key)?;
        Some(self.open.remove(i).1)
    }
    /// Ends every open picture and returns the keys with their numbers, oldest first.
    pub fn close_all(&mut self) -> Vec<(K, u32)> {
        std::mem::take(&mut self.open)
    }
}

/// One decoded video frame: raw pixels with their layout and presentation time. The standard typed item for `"video.frame"` streams.
/// The planes are shared, so a display can take a frame without a copy.
#[derive(Clone, Debug, PartialEq)]
pub struct VideoFrame {
    /// The presentation time stamp, in the stream's timebase.
    pub pts: Option<i64>,
    /// The layout (pixel format, size, strides, matrix, range).
    pub layout: PixelLayout,
    /// The planes, one after the other, as `layout` describes them.
    pub data: Arc<[u8]>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codec_ids_and_fourccs_map_both_ways() {
        for (i, row) in CODECS.iter().enumerate() {
            assert_eq!(row.codec.info(), row, "{}", row.id);
            assert_eq!(Codec::from_id(row.id), Some(row.codec));
            assert_eq!(Codec::from_fourcc(row.fourcc), Some(row.codec));
            assert!(
                CODECS[i + 1..]
                    .iter()
                    .all(|b| b.fourcc != row.fourcc && b.id != row.id),
                "{} repeats",
                row.id
            );
        }
        assert_eq!(Codec::H264.fourcc().as_str(), "avc1");
        assert_eq!(Codec::Pixels(PixelFormat::Grey8).fourcc().as_str(), "L008");
        assert_eq!(Codec::Pixels(PixelFormat::Grey8).media(), MediaType::Pixels);
        assert!(Codec::from_fourcc(FourCc::new("mp4a")).is_none());
        assert_eq!(FourCc::new("avc1").to_u32(), 0x6176_6331);
        assert_eq!(FourCc::new("png"), FourCc::new("png "));
        assert_eq!(serde_json::to_string(&Codec::HeAac).unwrap(), "\"he_aac\"");
    }

    #[test]
    fn packed_layouts_have_the_right_sizes() {
        let l = PixelLayout::packed(PixelFormat::Nv12, 1920, 1080);
        assert_eq!(l.pixel.codec().fourcc().as_str(), "420v");
        assert_eq!(l.frame_bytes(), 1920 * 1080 * 3 / 2);
        assert_eq!(
            PixelLayout::packed(PixelFormat::Bgra, 10, 10).frame_bytes(),
            400
        );
        assert_eq!(
            PixelLayout::packed(PixelFormat::Grey8, 800, 1).strides,
            vec![800]
        );
        assert_eq!(
            PixelLayout::packed(PixelFormat::Rgb8, 10, 10).frame_bytes(),
            300
        );
    }
}
