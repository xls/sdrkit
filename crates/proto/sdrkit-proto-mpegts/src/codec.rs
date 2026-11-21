//! Codec identification from the PMT, and the stream headers that describe each stream: H.264 and HEVC parameter sets, the
//! MPEG-2 sequence header, ADTS and LATM AudioSpecificConfig, MPEG audio, AC-3 and E-AC-3 sync frames.
//!
//! Standard: ISO/IEC 13818-1 Table 2-34, ETSI EN 300 468 Annex D, ETSI TS 101 154.

use crate::nal::{nal_units, unescape_rbsp};
use crate::tables::EsDescriptors;
use sdrkit_core::media::{Codec, FourCc};
use sdrkit_core::StreamKind;
use sdrkit_util_bits::BitReader;

sdrkit_core::choice_enum! {
    /// What an elementary stream is.
    pub enum EsKind {
        /// Video.
        Video = ("video", "Video"),
        /// Audio.
        Audio = ("audio", "Audio"),
        /// DVB subtitles.
        Subtitles = ("subtitles", "Subtitles"),
        /// Teletext.
        Teletext = ("teletext", "Teletext"),
        /// Anything else.
        Data = ("data", "Data"),
    }
}

sdrkit_core::choice_enum! {
    /// The packaging of a stream, as the service list reports it.
    pub enum EsPackaging {
        /// Start codes.
        AnnexB = ("annexb", "Start codes"),
        /// Elementary video or audio frames.
        Elementary = ("elementary", "Elementary frames"),
        /// ADTS.
        Adts = ("adts", "ADTS"),
        /// LATM / LOAS.
        Latm = ("latm", "LATM"),
        /// Sync frames (AC-3 and similar).
        Frames = ("frames", "Sync frames"),
        /// SMPTE 302M.
        Smpte302m = ("smpte302m", "SMPTE 302M"),
        /// EN 300 472 / EN 300 743 PES.
        Pes = ("pes", "PES"),
        /// Not applicable.
        None = ("none", "None"),
    }
}

sdrkit_core::choice_enum! {
    /// The codec of an elementary stream, as its PMT entry names it.
    pub enum EsCodec {
        /// MPEG-1 video.
        Mpeg1Video = ("mpeg1video", "MPEG-1 video"),
        /// MPEG-2 video.
        Mpeg2Video = ("mpeg2video", "MPEG-2 video"),
        /// H.264.
        H264 = ("h264", "H.264"),
        /// HEVC.
        Hevc = ("hevc", "HEVC"),
        /// AVS.
        Avs = ("avs", "AVS"),
        /// AVS2.
        Avs2 = ("avs2", "AVS2"),
        /// AVS3.
        Avs3 = ("avs3", "AVS3"),
        /// MPEG audio (layer I, II or III).
        Mp2 = ("mp2", "MPEG audio"),
        /// AAC.
        Aac = ("aac", "AAC"),
        /// AC-3.
        Ac3 = ("ac3", "AC-3"),
        /// E-AC-3.
        Eac3 = ("eac3", "E-AC-3"),
        /// AC-4.
        Ac4 = ("ac4", "AC-4"),
        /// DTS.
        Dts = ("dts", "DTS"),
        /// DRA.
        Dra = ("dra", "DRA"),
        /// Linear PCM (SMPTE 302M).
        Lpcm = ("lpcm", "Linear PCM"),
        /// Opus.
        Opus = ("opus", "Opus"),
        /// Teletext.
        Teletext = ("teletext", "Teletext"),
        /// DVB subtitles.
        DvbSubtitle = ("dvbsub", "DVB subtitles"),
        /// Data, or a stream type that is not known.
        Data = ("data", "Data"),
    }
}

impl EsCodec {
    /// What the stream is.
    pub fn kind(self) -> EsKind {
        use EsCodec::*;
        match self {
            Mpeg1Video | Mpeg2Video | H264 | Hevc | Avs | Avs2 | Avs3 => EsKind::Video,
            Mp2 | Aac | Ac3 | Eac3 | Ac4 | Dts | Dra | Lpcm | Opus => EsKind::Audio,
            Teletext => EsKind::Teletext,
            DvbSubtitle => EsKind::Subtitles,
            Data => EsKind::Data,
        }
    }

    /// The four-character code of the stream before its own headers are read. `None` for a stream with none.
    pub fn fourcc(self) -> Option<FourCc> {
        use EsCodec::*;
        let code = match self {
            Mpeg1Video => "mp1v",
            Mpeg2Video => "mp2v",
            H264 => "avc1",
            Hevc => "hvc1",
            Avs => "avs1",
            Avs2 => "avs2",
            Avs3 => "avs3",
            Mp2 => ".mp2",
            Aac => "aac ",
            Ac3 => "ac-3",
            Eac3 => "ec-3",
            Ac4 => "ac-4",
            Dts => "dtsc",
            Dra => "dra1",
            Lpcm => "lpcm",
            Opus => "Opus",
            Teletext | DvbSubtitle | Data => return None,
        };
        Some(FourCc::new(code))
    }

    /// Returns the codec that a host decoder receives for this stream, with the properties from the stream's own headers
    /// (`props`). Returns `None` for a codec that is not passed to the host.
    pub fn media_codec(self, props: &EsProps) -> Option<Codec> {
        Some(match self {
            EsCodec::H264 => Codec::H264,
            EsCodec::Hevc => Codec::Hevc,
            EsCodec::Mpeg2Video | EsCodec::Mpeg1Video => Codec::Mpeg2Video,
            EsCodec::Aac if props.ps => Codec::HeAacV2,
            EsCodec::Aac if props.sbr => Codec::HeAac,
            EsCodec::Aac => Codec::Aac,
            EsCodec::Mp2 if props.layer == 1 => Codec::Mp1,
            EsCodec::Mp2 if props.layer == 3 => Codec::Mp3,
            EsCodec::Mp2 => Codec::Mp2,
            EsCodec::Ac3 => Codec::Ac3,
            EsCodec::Eac3 => Codec::EAc3,
            EsCodec::Lpcm => Codec::Pcm,
            _ => return None,
        })
    }

    /// The host decoder of the stream, with what its own headers say (`props`).
    pub fn stream_kind(self, props: &EsProps) -> Option<StreamKind> {
        match self {
            EsCodec::Lpcm => None,
            _ => self.media_codec(props)?.stream_kind(),
        }
    }

    /// The four-character code of the stream, with what its own headers say (`props`): the code of [`EsCodec::media_codec`]
    /// where there is one, else [`EsCodec::fourcc`].
    pub fn fourcc_with(self, props: &EsProps) -> Option<FourCc> {
        match (self, self.media_codec(props)) {
            (EsCodec::Mpeg1Video, _) | (_, None) => self.fourcc(),
            (_, Some(c)) => Some(c.fourcc()),
        }
    }
}

/// Which rule decided the codec of a stream.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CodecRule {
    /// The stream_type alone.
    StreamType(u8),
    /// A registration descriptor (0x05) with this format_identifier.
    Registration([u8; 4]),
    /// A DVB descriptor with this tag (EN 300 468 Annex D).
    Descriptor(u8),
}

impl std::fmt::Display for CodecRule {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CodecRule::StreamType(t) => write!(f, "type 0x{t:02X}"),
            CodecRule::Registration(r) => write!(f, "registration {}", String::from_utf8_lossy(r)),
            CodecRule::Descriptor(t) => write!(f, "descriptor 0x{t:02X}"),
        }
    }
}

/// What the signalling says a stream is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CodecInfo {
    /// The codec.
    pub codec: EsCodec,
    /// Its packaging.
    pub packaging: EsPackaging,
    /// The rule that decided.
    pub rule: CodecRule,
}

impl CodecInfo {
    /// What the stream is.
    pub fn kind(&self) -> EsKind {
        self.codec.kind()
    }
}

fn ci(codec: EsCodec, packaging: EsPackaging, rule: CodecRule) -> CodecInfo {
    CodecInfo {
        codec,
        packaging,
        rule,
    }
}

/// The codec of a stream from its stream_type and descriptors.
pub fn identify(stream_type: u8, d: &EsDescriptors) -> CodecInfo {
    use EsCodec as C;
    use EsPackaging as P;
    let st = CodecRule::StreamType(stream_type);
    let reg = d.registration;
    let reg_is = |s: &[u8; 4]| reg.as_ref() == Some(s);
    if reg_is(b"HEVC") {
        return ci(C::Hevc, P::AnnexB, CodecRule::Registration(*b"HEVC"));
    }
    if stream_type == 0x24 {
        return ci(C::Hevc, P::AnnexB, st);
    }
    if reg_is(b"DRA1") {
        return ci(C::Dra, P::Frames, CodecRule::Registration(*b"DRA1"));
    }
    match stream_type {
        0x01 => return ci(C::Mpeg1Video, P::Elementary, st),
        0x02 => return ci(C::Mpeg2Video, P::Elementary, st),
        0x1B => return ci(C::H264, P::AnnexB, st),
        0x03 | 0x04 => return ci(C::Mp2, P::Elementary, st),
        0x0F => return ci(C::Aac, P::Adts, st),
        0x11 => return ci(C::Aac, P::Latm, st),
        0x81 => return ci(C::Ac3, P::Frames, st),
        0x87 => return ci(C::Eac3, P::Frames, st),
        0x42 => return ci(C::Avs, P::AnnexB, st),
        0xD2 => return ci(C::Avs2, P::AnnexB, st),
        0xD4 => return ci(C::Avs3, P::AnnexB, st),
        0x05 | 0x0B..=0x0D | 0x15 => return ci(C::Data, P::None, st),
        _ => {}
    }
    if stream_type == 0x06 {
        let desc = CodecRule::Descriptor;
        if d.ac3.is_some() {
            return ci(C::Ac3, P::Frames, desc(0x6A));
        }
        if d.eac3.is_some() {
            return ci(C::Eac3, P::Frames, desc(0x7A));
        }
        if d.ac4 {
            return ci(C::Ac4, P::Frames, desc(0x7F));
        }
        if d.dts {
            return ci(C::Dts, P::Frames, desc(0x7B));
        }
        if d.dvb_aac.is_some() {
            return ci(C::Aac, P::Adts, desc(0x7C));
        }
        if !d.teletext.is_empty() || d.has_teletext {
            return ci(C::Teletext, P::Pes, desc(0x56));
        }
        if d.has_subtitles {
            return ci(C::DvbSubtitle, P::Pes, desc(0x59));
        }
        for (r, c, p) in [
            (b"AC-3", C::Ac3, P::Frames),
            (b"EAC3", C::Eac3, P::Frames),
            (b"BSSD", C::Lpcm, P::Smpte302m),
            (b"Opus", C::Opus, P::Frames),
            (b"KLVA", C::Data, P::None),
            (b"ID3 ", C::Data, P::None),
        ] {
            if reg_is(r) {
                return ci(c, p, CodecRule::Registration(*r));
            }
        }
    }
    ci(C::Data, P::None, st)
}

/// What the stream itself says about it.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct EsProps {
    /// Profile name ("Main", "High", "LC").
    pub profile: Option<String>,
    /// Level ("3.0").
    pub level: Option<String>,
    /// Picture width.
    pub width: u32,
    /// Picture height.
    pub height: u32,
    /// Frames per second as a fraction.
    pub frame_rate: Option<(u32, u32)>,
    /// Sample aspect ratio.
    pub sar: Option<(u32, u32)>,
    /// Display aspect ratio.
    pub dar: Option<(u32, u32)>,
    /// The aspect as text ("16:9", "5:4 (square pixels)").
    pub aspect: String,
    /// Interlaced (false: progressive).
    pub interlaced: Option<bool>,
    /// Colour description: (primaries, transfer, matrix).
    pub colour: Option<(u8, u8, u8)>,
    /// Chroma format (1 4:2:0).
    pub chroma_format: u8,
    /// Bit depth.
    pub bit_depth: u8,
    /// Output sample rate (twice the core rate with SBR).
    pub sample_rate: u32,
    /// Core sample rate (AAC with SBR).
    pub core_sample_rate: u32,
    /// Channels (after PS).
    pub channels: u8,
    /// Channel layout ("stereo", "5.1").
    pub layout: String,
    /// Bit rate from the frame header, kbit/s.
    pub bitrate_kbps: u32,
    /// AAC object type name ("AAC-LC", "HE-AAC", "HE-AAC v2"), or the MPEG audio layer.
    pub object_type: String,
    /// SBR present.
    pub sbr: bool,
    /// Parametric stereo present.
    pub ps: bool,
    /// MPEG audio layer (1 to 3).
    pub layer: u8,
}

fn reduce(a: u32, b: u32) -> (u32, u32) {
    fn gcd(a: u32, b: u32) -> u32 {
        if b == 0 {
            a.max(1)
        } else {
            gcd(b, a % b)
        }
    }
    let g = gcd(a, b);
    (a / g, b / g)
}

fn layout(ch: u8, lfe: bool) -> String {
    match (ch, lfe) {
        (1, false) => "mono".into(),
        (2, false) => "stereo".into(),
        (3, false) => "3.0".into(),
        (4, false) => "4.0".into(),
        (5, false) => "5.0".into(),
        (n, true) => format!("{}.1", n),
        (n, _) => format!("{n} channels"),
    }
}

// ---------------------------------------------------------------------------------------------------------------------------------
// H.264.

/// An H.264 sequence parameter set, as far as a decoder choice and the display need.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct H264Sps {
    /// profile_idc.
    pub profile_idc: u8,
    /// constraint_set flags.
    pub constraints: u8,
    /// level_idc.
    pub level_idc: u8,
    /// seq_parameter_set_id.
    pub id: u32,
    /// chroma_format_idc.
    pub chroma_format: u32,
    /// bit_depth_luma.
    pub bit_depth: u32,
    /// Width after cropping.
    pub width: u32,
    /// Height after cropping.
    pub height: u32,
    /// frame_mbs_only_flag (false: may be interlaced).
    pub frame_mbs_only: bool,
    /// Sample aspect ratio from the VUI.
    pub sar: Option<(u32, u32)>,
    /// Frame rate from the VUI timing: (time_scale, 2 × num_units_in_tick).
    pub frame_rate: Option<(u32, u32)>,
    /// Colour description.
    pub colour: Option<(u8, u8, u8)>,
    /// Full-range video.
    pub full_range: bool,
}

const SAR_TABLE: [(u32, u32); 17] = [
    (0, 0),
    (1, 1),
    (12, 11),
    (10, 11),
    (16, 11),
    (40, 33),
    (24, 11),
    (20, 11),
    (32, 11),
    (80, 33),
    (18, 11),
    (15, 11),
    (64, 33),
    (160, 99),
    (4, 3),
    (3, 2),
    (2, 1),
];

fn scaling_list(r: &mut BitReader, size: usize) {
    let (mut last, mut next) = (8i32, 8i32);
    for _ in 0..size {
        if next != 0 {
            let delta = r.se();
            next = (last + delta + 256) % 256;
        }
        last = if next == 0 { last } else { next };
    }
}

/// Parses an H.264 SPS NAL unit (with its header byte).
pub fn parse_h264_sps(nal: &[u8]) -> Option<H264Sps> {
    if nal.len() < 4 || nal[0] & 0x1F != 7 {
        return None;
    }
    let rbsp = unescape_rbsp(&nal[1..]);
    let mut r = BitReader::new(&rbsp);
    let mut s = H264Sps {
        profile_idc: r.u(8) as u8,
        constraints: r.u(8) as u8,
        level_idc: r.u(8) as u8,
        ..Default::default()
    };
    s.id = r.ue();
    s.chroma_format = 1;
    s.bit_depth = 8;
    if matches!(
        s.profile_idc,
        100 | 110 | 122 | 244 | 44 | 83 | 86 | 118 | 128 | 138 | 139 | 134 | 135
    ) {
        s.chroma_format = r.ue();
        if s.chroma_format == 3 {
            r.bit();
        }
        s.bit_depth = r.ue() + 8;
        let _chroma_depth = r.ue();
        r.bit();
        if r.bit() {
            let n = if s.chroma_format != 3 { 8 } else { 12 };
            for i in 0..n {
                if r.bit() {
                    scaling_list(&mut r, if i < 6 { 16 } else { 64 });
                }
            }
        }
    }
    let _log2_max_frame_num = r.ue();
    let poc_type = r.ue();
    if poc_type == 0 {
        r.ue();
    } else if poc_type == 1 {
        r.bit();
        r.se();
        r.se();
        let n = r.ue();
        for _ in 0..n.min(255) {
            r.se();
        }
    }
    r.ue(); // max_num_ref_frames
    r.bit(); // gaps
    let w_mbs = r.ue() + 1;
    let h_units = r.ue() + 1;
    s.frame_mbs_only = r.bit();
    if !s.frame_mbs_only {
        r.bit();
    }
    r.bit(); // direct_8x8
    let (mut cl, mut cr, mut ct, mut cb) = (0, 0, 0, 0);
    if r.bit() {
        cl = r.ue();
        cr = r.ue();
        ct = r.ue();
        cb = r.ue();
    }
    let (sub_w, sub_h) = match s.chroma_format {
        1 => (2, 2),
        2 => (2, 1),
        _ => (1, 1),
    };
    let crop_x = if s.chroma_format == 0 { 1 } else { sub_w };
    let crop_y = (if s.chroma_format == 0 { 1 } else { sub_h }) * (2 - s.frame_mbs_only as u32);
    s.width = (w_mbs * 16).saturating_sub(crop_x * (cl + cr));
    s.height = (h_units * 16 * (2 - s.frame_mbs_only as u32)).saturating_sub(crop_y * (ct + cb));
    if r.bit() {
        // VUI
        if r.bit() {
            let idc = r.u(8) as u32;
            if idc == 255 {
                let a = r.u(16) as u32;
                let b = r.u(16) as u32;
                s.sar = Some((a, b));
            } else if (idc as usize) < SAR_TABLE.len() && idc > 0 {
                s.sar = Some(SAR_TABLE[idc as usize]);
            }
        }
        if r.bit() {
            r.bit();
        }
        if r.bit() {
            r.u(3);
            s.full_range = r.bit();
            if r.bit() {
                s.colour = Some((r.u(8) as u8, r.u(8) as u8, r.u(8) as u8));
            }
        }
        if r.bit() {
            r.ue();
            r.ue();
        }
        if r.bit() {
            let units = r.u(32) as u32;
            let scale = r.u(32) as u32;
            if units > 0 && scale > 0 {
                s.frame_rate = Some(reduce(scale, 2 * units));
            }
        }
    }
    (!r.overrun() || s.width > 0).then_some(s)
}

/// The H.264 profile name.
pub fn h264_profile(idc: u8, constraints: u8) -> String {
    match idc {
        66 if constraints & 0x40 != 0 => "Constrained Baseline",
        66 => "Baseline",
        77 => "Main",
        88 => "Extended",
        100 => "High",
        110 => "High 10",
        122 => "High 4:2:2",
        244 => "High 4:4:4",
        _ => return format!("profile {idc}"),
    }
    .into()
}

fn dar_text(w: u32, h: u32, sar: Option<(u32, u32)>, square: bool) -> ((u32, u32), String) {
    let (sw, sh) = sar.filter(|s| s.0 > 0 && s.1 > 0).unwrap_or((1, 1));
    let d = reduce(w * sw, h * sh);
    let mut t = format!("{}:{}", d.0, d.1);
    if square && (sw, sh) == (1, 1) {
        t.push_str(" (square pixels)");
    }
    (d, t)
}

/// Fills `p` from an H.264 SPS.
pub fn h264_props(s: &H264Sps, p: &mut EsProps) {
    p.profile = Some(h264_profile(s.profile_idc, s.constraints));
    p.level = Some(format!("{}.{}", s.level_idc / 10, s.level_idc % 10));
    p.width = s.width;
    p.height = s.height;
    p.sar = s.sar;
    let (d, t) = dar_text(s.width, s.height, s.sar, false);
    p.dar = Some(d);
    p.aspect = t;
    if s.frame_rate.is_some() {
        p.frame_rate = s.frame_rate;
    }
    p.interlaced = Some(!s.frame_mbs_only);
    p.colour = s.colour;
    p.chroma_format = s.chroma_format as u8;
    p.bit_depth = s.bit_depth as u8;
}

/// H.264 NAL units of an access unit: (type, start, end).
pub fn h264_nals(au: &[u8]) -> Vec<(u8, usize, usize)> {
    nal_units(au)
        .into_iter()
        .filter(|(s, e)| e > s)
        .map(|(s, e)| (au[s] & 0x1F, s, e))
        .collect()
}

/// Whether an H.264 access unit is a keyframe: an IDR slice, or a recovery point SEI with recovery_frame_cnt 0.
pub fn h264_keyframe(au: &[u8]) -> bool {
    for (t, s, e) in h264_nals(au) {
        if t == 5 {
            return true;
        }
        if t == 6 {
            let rb = unescape_rbsp(&au[s + 1..e]);
            let mut i = 0;
            while i < rb.len() {
                let mut ty = 0usize;
                while i < rb.len() && rb[i] == 0xFF {
                    ty += 255;
                    i += 1;
                }
                if i >= rb.len() {
                    break;
                }
                ty += rb[i] as usize;
                i += 1;
                let mut sz = 0usize;
                while i < rb.len() && rb[i] == 0xFF {
                    sz += 255;
                    i += 1;
                }
                if i >= rb.len() {
                    break;
                }
                sz += rb[i] as usize;
                i += 1;
                if ty == 6 && i < rb.len() {
                    let mut r = BitReader::new(&rb[i..]);
                    if r.ue() == 0 {
                        return true;
                    }
                }
                i += sz;
                if i < rb.len() && rb[i] == 0x80 {
                    break;
                }
            }
        }
    }
    false
}

// ---------------------------------------------------------------------------------------------------------------------------------
// HEVC.

/// An HEVC SPS, as far as the description needs.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct HevcSps {
    /// general_profile_idc.
    pub profile_idc: u8,
    /// general_tier_flag.
    pub tier: u8,
    /// general_level_idc.
    pub level_idc: u8,
    /// chroma_format_idc.
    pub chroma_format: u32,
    /// Width after the conformance window.
    pub width: u32,
    /// Height after the conformance window.
    pub height: u32,
    /// Luma bit depth.
    pub bit_depth: u32,
    /// Sample aspect ratio (VUI).
    pub sar: Option<(u32, u32)>,
    /// Frame rate (VUI timing).
    pub frame_rate: Option<(u32, u32)>,
    /// Colour description.
    pub colour: Option<(u8, u8, u8)>,
}

fn hevc_ptl(r: &mut BitReader, max_sub_layers_minus1: u32) -> (u8, u8, u8) {
    r.u(2);
    let tier = r.u(1) as u8;
    let profile = r.u(5) as u8;
    r.skip(32 + 48);
    let level = r.u(8) as u8;
    let mut sub_profile = [false; 8];
    let mut sub_level = [false; 8];
    for i in 0..max_sub_layers_minus1 as usize {
        sub_profile[i] = r.bit();
        sub_level[i] = r.bit();
    }
    if max_sub_layers_minus1 > 0 {
        for _ in max_sub_layers_minus1..8 {
            r.u(2);
        }
    }
    for i in 0..max_sub_layers_minus1 as usize {
        if sub_profile[i] {
            r.skip(88);
        }
        if sub_level[i] {
            r.skip(8);
        }
    }
    (profile, tier, level)
}

/// One short-term reference picture set of an SPS: the POC deltas before (`s0`, negative, nearest first) and after (`s1`,
/// positive, nearest first) the current picture.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct StRps {
    /// DeltaPocS0.
    pub s0: Vec<i32>,
    /// DeltaPocS1.
    pub s1: Vec<i32>,
}

/// Reads `st_ref_pic_set(idx)` (H.265 7.3.7) and derives its POC deltas (7.4.8), so that a later set predicted from this one reads
/// the right number of flags. `sets` holds the sets read before.
fn st_ref_pic_set(r: &mut BitReader, idx: u32, num: u32, sets: &mut Vec<StRps>) {
    let inter = idx != 0 && r.bit();
    if inter {
        let delta_idx = if idx == num { r.ue() + 1 } else { 1 };
        let sign = r.bit();
        let abs = r.ue() as i64 + 1;
        let delta_rps = if sign { -abs } else { abs };
        let rf = idx
            .checked_sub(delta_idx)
            .and_then(|i| sets.get(i as usize))
            .cloned()
            .unwrap_or_default();
        let n = rf.s0.len() + rf.s1.len();
        // use_delta_flag per entry of the reference set (S0 first, then S1), then for delta_rps itself.
        let mut use_delta = Vec::with_capacity(n + 1);
        for _ in 0..=n {
            let used = r.bit();
            use_delta.push(used || r.bit());
        }
        let (n0, n1) = (rf.s0.len(), rf.s1.len());
        let mut out = StRps::default();
        // Negative deltas, nearest first (7-61).
        for j in (0..n1).rev() {
            let d = rf.s1[j] as i64 + delta_rps;
            if d < 0 && use_delta[n0 + j] {
                out.s0.push(d as i32);
            }
        }
        if delta_rps < 0 && use_delta[n] {
            out.s0.push(delta_rps as i32);
        }
        for (&p, &u) in rf.s0.iter().zip(&use_delta) {
            let d = p as i64 + delta_rps;
            if d < 0 && u {
                out.s0.push(d as i32);
            }
        }
        // Positive deltas, nearest first (7-62).
        for j in (0..n0).rev() {
            let d = rf.s0[j] as i64 + delta_rps;
            if d > 0 && use_delta[j] {
                out.s1.push(d as i32);
            }
        }
        if delta_rps > 0 && use_delta[n] {
            out.s1.push(delta_rps as i32);
        }
        for j in 0..n1 {
            let d = rf.s1[j] as i64 + delta_rps;
            if d > 0 && use_delta[n0 + j] {
                out.s1.push(d as i32);
            }
        }
        out.s0.truncate(16);
        out.s1.truncate(16);
        sets.push(out);
    } else {
        // At most 16 pictures of each kind (sps_max_dec_pic_buffering); more is a damaged SPS.
        let n0 = r.ue().min(16);
        let n1 = r.ue().min(16);
        let mut out = StRps::default();
        let mut poc = 0i32;
        for _ in 0..n0 {
            poc = poc.saturating_sub(r.ue().min(1 << 15) as i32 + 1);
            r.bit();
            out.s0.push(poc);
        }
        poc = 0;
        for _ in 0..n1 {
            poc = poc.saturating_add(r.ue().min(1 << 15) as i32 + 1);
            r.bit();
            out.s1.push(poc);
        }
        sets.push(out);
    }
}

/// Parses an HEVC SPS NAL unit (with its two-byte header).
pub fn parse_hevc_sps(nal: &[u8]) -> Option<HevcSps> {
    if nal.len() < 4 || (nal[0] >> 1) & 0x3F != 33 {
        return None;
    }
    let rbsp = unescape_rbsp(&nal[2..]);
    let mut r = BitReader::new(&rbsp);
    r.u(4);
    let msl = r.u(3) as u32;
    r.bit();
    let (profile_idc, tier, level_idc) = hevc_ptl(&mut r, msl);
    let mut s = HevcSps {
        profile_idc,
        tier,
        level_idc,
        ..Default::default()
    };
    r.ue();
    s.chroma_format = r.ue();
    if s.chroma_format == 3 {
        r.bit();
    }
    let w = r.ue();
    let h = r.ue();
    let (sw, sh) = match s.chroma_format {
        1 => (2, 2),
        2 => (2, 1),
        _ => (1, 1),
    };
    let (mut l, mut rr, mut t, mut b) = (0, 0, 0, 0);
    if r.bit() {
        l = r.ue();
        rr = r.ue();
        t = r.ue();
        b = r.ue();
    }
    s.width = w.saturating_sub(sw * (l + rr));
    s.height = h.saturating_sub(sh * (t + b));
    s.bit_depth = r.ue() + 8;
    r.ue();
    let log2_max_poc = r.ue() + 4;
    let ordering_all = r.bit();
    let start = if ordering_all { 0 } else { msl };
    for _ in start..=msl {
        r.ue();
        r.ue();
        r.ue();
    }
    r.ue();
    r.ue();
    r.ue();
    r.ue();
    r.ue();
    r.ue();
    if r.bit() && r.bit() {
        // sps_scaling_list_data
        for size_id in 0..4 {
            let step = if size_id == 3 { 3 } else { 1 };
            let mut m = 0;
            while m < 6 {
                if !r.bit() {
                    r.ue();
                } else {
                    let coef = (1 << (4 + (size_id << 1))).min(64);
                    if size_id > 1 {
                        r.se();
                    }
                    for _ in 0..coef {
                        r.se();
                    }
                }
                m += step;
            }
        }
    }
    r.bit();
    r.bit();
    if r.bit() {
        r.u(8);
        r.ue();
        r.ue();
        r.bit();
    }
    let n_st = r.ue().min(64);
    let mut sets = Vec::with_capacity(n_st as usize);
    for i in 0..n_st {
        st_ref_pic_set(&mut r, i, n_st, &mut sets);
    }
    if r.bit() {
        let n = r.ue().min(32);
        for _ in 0..n {
            r.skip(log2_max_poc as usize);
            r.bit();
        }
    }
    r.bit();
    r.bit();
    if r.bit() && !r.overrun() {
        if r.bit() {
            let idc = r.u(8) as u32;
            if idc == 255 {
                s.sar = Some((r.u(16) as u32, (r.u(16) as u32)));
            } else if idc > 0 && (idc as usize) < SAR_TABLE.len() {
                s.sar = Some(SAR_TABLE[idc as usize]);
            }
        }
        if r.bit() {
            r.bit();
        }
        if r.bit() {
            r.u(4);
            if r.bit() {
                s.colour = Some((r.u(8) as u8, r.u(8) as u8, r.u(8) as u8));
            }
        }
        if r.bit() {
            r.ue();
            r.ue();
        }
        r.bit();
        r.bit();
        r.bit();
        if r.bit() {
            r.ue();
            r.ue();
            r.ue();
            r.ue();
        }
        if r.bit() {
            let units = r.u(32) as u32;
            let scale = r.u(32) as u32;
            if units > 0 && scale > 0 && !r.overrun() {
                s.frame_rate = Some(reduce(scale, units));
            }
        }
    }
    (s.width > 0).then_some(s)
}

/// Fills `p` from an HEVC SPS.
pub fn hevc_props(s: &HevcSps, p: &mut EsProps) {
    p.profile = Some(
        match s.profile_idc {
            1 => "Main",
            2 => "Main 10",
            3 => "Main Still Picture",
            4 => "Range Extensions",
            _ => "",
        }
        .to_string(),
    )
    .filter(|x| !x.is_empty())
    .or_else(|| Some(format!("profile {}", s.profile_idc)));
    p.level = Some(format!(
        "{}{}",
        s.level_idc as f32 / 30.0,
        if s.tier == 1 { " High tier" } else { "" }
    ));
    p.width = s.width;
    p.height = s.height;
    p.sar = s.sar;
    let (d, t) = dar_text(s.width, s.height, s.sar, false);
    p.dar = Some(d);
    p.aspect = t;
    if s.frame_rate.is_some() {
        p.frame_rate = s.frame_rate;
    }
    p.colour = s.colour;
    p.chroma_format = s.chroma_format as u8;
    p.bit_depth = s.bit_depth as u8;
}

/// HEVC NAL units of an access unit: (type, start, end).
pub fn hevc_nals(au: &[u8]) -> Vec<(u8, usize, usize)> {
    nal_units(au)
        .into_iter()
        .filter(|(s, e)| e > s)
        .map(|(s, e)| ((au[s] >> 1) & 0x3F, s, e))
        .collect()
}

/// Whether an HEVC access unit holds an IRAP picture (BLA, IDR, CRA: types 16 to 23).
pub fn hevc_keyframe(au: &[u8]) -> bool {
    hevc_nals(au).iter().any(|n| (16..=23).contains(&n.0))
}

// ---------------------------------------------------------------------------------------------------------------------------------
// MPEG-1/2 video.

/// Fills `p` from an MPEG-1/2 video sequence header (and sequence extension) found in `es`. Returns whether a header was found.
pub fn mpeg2_props(es: &[u8], p: &mut EsProps) -> bool {
    let mut found = false;
    let mut i = 0;
    while i + 4 <= es.len() {
        if es[i] == 0 && es[i + 1] == 0 && es[i + 2] == 1 {
            let code = es[i + 3];
            let d = &es[i + 4..];
            if code == 0xB3 && d.len() >= 8 {
                let w = (d[0] as u32) << 4 | (d[1] as u32 >> 4);
                let h = (d[1] as u32 & 0x0F) << 8 | d[2] as u32;
                let ar = d[3] >> 4;
                let fr = d[3] & 0x0F;
                p.width = w;
                p.height = h;
                p.frame_rate = match fr {
                    1 => Some((24000, 1001)),
                    2 => Some((24, 1)),
                    3 => Some((25, 1)),
                    4 => Some((30000, 1001)),
                    5 => Some((30, 1)),
                    6 => Some((50, 1)),
                    7 => Some((60000, 1001)),
                    8 => Some((60, 1)),
                    _ => None,
                };
                let (dar, text) = match ar {
                    2 => ((4, 3), "4:3".to_string()),
                    3 => ((16, 9), "16:9".to_string()),
                    4 => ((221, 100), "2.21:1".to_string()),
                    _ => dar_text(w, h, None, true),
                };
                p.dar = Some(dar);
                p.aspect = text;
                p.chroma_format = 1;
                p.bit_depth = 8;
                found = true;
            } else if code == 0xB5 && d.len() >= 6 && d[0] >> 4 == 1 {
                let pl = (d[0] & 0x0F) << 4 | d[1] >> 4;
                let profile = (pl >> 4) & 7;
                let level = pl & 0x0F;
                p.profile = Some(
                    match profile {
                        5 => "Simple",
                        4 => "Main",
                        3 => "SNR Scalable",
                        2 => "Spatially Scalable",
                        1 => "High",
                        _ => "unknown",
                    }
                    .into(),
                );
                p.level = Some(
                    match level {
                        10 => "Low",
                        8 => "Main",
                        6 => "High 1440",
                        4 => "High",
                        _ => "unknown",
                    }
                    .into(),
                );
                p.interlaced = Some(d[1] & 0x08 == 0);
                p.chroma_format = (d[1] >> 1) & 3;
            }
            i += 4;
        } else {
            i += 1;
        }
    }
    found
}

/// Whether an MPEG-1/2 video access unit is a keyframe: an I picture after a sequence header or a GOP header.
pub fn mpeg2_keyframe(es: &[u8]) -> bool {
    let mut header = false;
    let mut i = 0;
    while i + 6 <= es.len() {
        if es[i] == 0 && es[i + 1] == 0 && es[i + 2] == 1 {
            match es[i + 3] {
                0xB3 | 0xB8 => header = true,
                0x00 => {
                    let ty = (es[i + 5] >> 3) & 7;
                    return header && ty == 1;
                }
                _ => {}
            }
            i += 4;
        } else {
            i += 1;
        }
    }
    false
}

// ---------------------------------------------------------------------------------------------------------------------------------
// AAC.

const AAC_RATES: [u32; 13] = [
    96000, 88200, 64000, 48000, 44100, 32000, 24000, 22050, 16000, 12000, 11025, 8000, 7350,
];

/// An AudioSpecificConfig, as far as the description needs.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Asc {
    /// The (core) audio object type.
    pub object_type: u8,
    /// Core sampling rate.
    pub sample_rate: u32,
    /// channelConfiguration.
    pub channel_config: u8,
    /// SBR signalled.
    pub sbr: bool,
    /// PS signalled.
    pub ps: bool,
    /// Output rate with SBR.
    pub ext_rate: u32,
}

fn aot(r: &mut BitReader) -> u8 {
    let a = r.u(5) as u32;
    if a == 31 {
        (32 + (r.u(6) as u32)) as u8
    } else {
        a as u8
    }
}

fn rate(r: &mut BitReader) -> u32 {
    let i = r.u(4) as u32;
    if i == 15 {
        r.u(24) as u32
    } else {
        AAC_RATES.get(i as usize).copied().unwrap_or(0)
    }
}

/// The AudioSpecificConfig up to the end of its GASpecificConfig, without the backward-compatible extension.
fn parse_asc_core(r: &mut BitReader) -> Asc {
    let mut a = Asc {
        object_type: aot(r),
        ..Default::default()
    };
    a.sample_rate = rate(r);
    a.channel_config = r.u(4) as u8;
    if a.object_type == 5 || a.object_type == 29 {
        a.sbr = true;
        a.ps = a.object_type == 29;
        a.ext_rate = rate(r);
        a.object_type = aot(r);
        if a.object_type == 22 {
            r.u(4);
        }
    }
    if matches!(a.object_type, 1..=4 | 6 | 7 | 17 | 19..=23) {
        // GASpecificConfig
        r.bit();
        if r.bit() {
            r.u(14);
        }
        let ext = r.bit();
        if a.channel_config == 0 {
            // program_config_element is not expected in broadcast; give up on the extension.
            return a;
        }
        if matches!(a.object_type, 6 | 20) {
            r.u(3);
        }
        if ext {
            if a.object_type == 22 {
                r.u(16);
            }
            if matches!(a.object_type, 17 | 19..=23) {
                r.u(3);
            }
            r.bit();
        }
    }
    a
}

/// Parses an AudioSpecificConfig (ISO/IEC 14496-3 1.6.2.1), with explicit and backward-compatible SBR / PS signalling. The
/// extension is read only when its sync word follows, so `r` stops at the end of the configuration also when more bits
/// follow, as in a LATM StreamMuxConfig.
pub fn parse_audio_specific_config(r: &mut BitReader) -> Asc {
    let mut a = parse_asc_core(r);
    // Backward-compatible signalling at the end: syncExtensionType 0x2B7, extensionAudioObjectType 5, sbrPresentFlag,
    // then syncExtensionType 0x548 and psPresentFlag.
    if !a.sbr && r.left() >= 16 && r.peek(11) == 0x2B7 {
        r.u(11);
        let ext_aot = aot(r);
        if ext_aot == 5 && r.bit() {
            a.sbr = true;
            a.ext_rate = rate(r);
            if r.left() >= 12 && r.peek(11) == 0x548 {
                r.u(11);
                a.ps = r.bit();
            }
        }
    }
    a
}

/// The StreamMuxConfig of a LATM AudioMuxElement (ISO/IEC 14496-3 1.7.3) at `r`, after useSameStreamMux: audioMuxVersion
/// 0, all streams with the same time framing, one sub-frame, one programme, one layer, and frameLengthType 0. Returns the
/// AudioSpecificConfig and the bits it takes in the data of `r`, and leaves `r` at the first PayloadLengthInfo. `None`
/// for another configuration. A backward-compatible SBR extension is found by its sync word: its first 3 bits (010)
/// cannot be frameLengthType 0.
pub fn parse_stream_mux_config(r: &mut BitReader) -> Option<(Asc, std::ops::Range<usize>)> {
    let version = r.bit();
    let same_time = r.bit();
    let (sub_frames, programs, layers) = (r.u(6), r.u(4), r.u(3));
    if version || !same_time || sub_frames != 0 || programs != 0 || layers != 0 {
        return None;
    }
    let start = r.pos();
    let asc = parse_audio_specific_config(r);
    let end = r.pos();
    if asc.sample_rate == 0 || r.u(3) != 0 {
        return None;
    }
    r.u(8); // latmBufferFullness
    if r.bit() {
        // otherDataLenBits, in bytes with an escape bit.
        let mut bits = 0usize;
        loop {
            let more = r.bit();
            bits = bits << 8 | r.u(8) as usize;
            if !more {
                break;
            }
        }
        r.skip(bits);
    }
    if r.bit() {
        r.u(8); // crcCheckSum
    }
    (!r.overrun()).then_some((asc, start..end))
}

/// Builds an AudioSpecificConfig (2 bytes, or 5 with explicit SBR signalling).
pub fn build_asc(object_type: u8, sf_index: u8, channels: u8, sbr: bool) -> Vec<u8> {
    let v = (object_type as u16) << 11 | (sf_index as u16) << 7 | (channels as u16) << 3;
    let mut out = v.to_be_bytes().to_vec();
    if sbr {
        // Backward-compatible: syncExtensionType 0x2B7, extensionAudioObjectType 5, sbrPresentFlag 1, extension rate index.
        let ext_idx = sf_index.saturating_sub(3);
        let bits: u32 = (0x2B7 << 13) | (5 << 8) | (1 << 7) | ((ext_idx as u32) << 3);
        out.extend_from_slice(&bits.to_be_bytes()[1..]);
    }
    out
}

fn aac_name(sbr: bool, ps: bool, aot: u8) -> String {
    match (sbr, ps) {
        (_, true) => "HE-AAC v2".into(),
        (true, _) => "HE-AAC".into(),
        _ if aot == 2 => "AAC-LC".into(),
        _ if aot == 42 => "xHE-AAC".into(),
        _ => format!("AAC object type {aot}"),
    }
}

/// Fills `p` from an AudioSpecificConfig.
pub fn asc_props(a: &Asc, p: &mut EsProps) {
    p.core_sample_rate = a.sample_rate;
    p.sbr = a.sbr;
    p.ps = a.ps;
    p.sample_rate = if a.sbr {
        if a.ext_rate > 0 {
            a.ext_rate
        } else {
            a.sample_rate * 2
        }
    } else {
        a.sample_rate
    };
    let (ch, lfe) = match a.channel_config {
        6 => (5, true),
        7 => (7, true),
        c => (c, false),
    };
    p.channels = if a.ps { 2 } else { ch + lfe as u8 };
    p.layout = if a.ps {
        "stereo".into()
    } else {
        layout(ch, lfe)
    };
    p.object_type = aac_name(a.sbr, a.ps, a.object_type);
    p.profile = Some(p.object_type.clone());
}

/// An ADTS header.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Adts {
    /// Object type (profile + 1).
    pub object_type: u8,
    /// Sampling frequency index.
    pub sf_index: u8,
    /// Channel configuration.
    pub channels: u8,
    /// Frame length including the header.
    pub frame_len: usize,
    /// Header length (7 or 9).
    pub header_len: usize,
}

/// Parses an ADTS header at the start of `b`.
pub fn parse_adts(b: &[u8]) -> Option<Adts> {
    if b.len() < 7 || b[0] != 0xFF || b[1] & 0xF6 != 0xF0 {
        return None;
    }
    let prot_absent = b[1] & 1 != 0;
    let frame_len = ((b[3] as usize & 3) << 11) | (b[4] as usize) << 3 | (b[5] as usize >> 5);
    let sf = (b[2] >> 2) & 0x0F;
    if sf > 12 || frame_len < 7 {
        return None;
    }
    Some(Adts {
        object_type: (b[2] >> 6) + 1,
        sf_index: sf,
        channels: ((b[2] & 1) << 2) | (b[3] >> 6),
        frame_len,
        header_len: if prot_absent { 7 } else { 9 },
    })
}

/// The AudioSpecificConfig carried in the LOAS frame `b` (ISO/IEC 14496-3 1.7.2), read by [`parse_stream_mux_config`], with
/// its bits in `b[3..]`. `None` for a frame without a StreamMuxConfig (useSameStreamMux) or with one not read here.
pub fn parse_latm(b: &[u8]) -> Option<(Asc, std::ops::Range<usize>)> {
    if b.len() < 4 || (u16::from_be_bytes([b[0], b[1]]) >> 5) != 0x2B7 {
        return None;
    }
    let mut r = BitReader::new(&b[3..]);
    if r.bit() {
        return None;
    }
    parse_stream_mux_config(&mut r)
}

/// The sampling-frequency index of a rate.
pub fn aac_rate_index(rate: u32) -> u8 {
    AAC_RATES.iter().position(|&r| r == rate).unwrap_or(15) as u8
}

// ---------------------------------------------------------------------------------------------------------------------------------
// MPEG audio, AC-3, E-AC-3.

/// Fills `p` from an MPEG audio frame header at the start of `b`, read by [`sdrkit_codec_mpa::Header::parse`]. Returns false
/// when there is no valid header.
pub fn mpa_props(b: &[u8], p: &mut EsProps) -> bool {
    let Ok(h) = sdrkit_codec_mpa::Header::parse(b) else {
        return false;
    };
    p.sample_rate = h.sample_rate;
    p.bitrate_kbps = u32::from(h.bitrate_kbps);
    p.channels = h.channels() as u8;
    p.layout = layout(p.channels, false);
    p.layer = h.layer;
    p.object_type = format!("Layer {}", ["I", "II", "III"][usize::from(h.layer) - 1]);
    true
}

/// The bit rates of AC-3 by half the frame size code, kbit/s (ETSI TS 102 366 table 4.13).
pub(crate) const AC3_RATES: [u32; 19] = [
    32, 40, 48, 56, 64, 80, 96, 112, 128, 160, 192, 224, 256, 320, 384, 448, 512, 576, 640,
];

fn acmod_channels(acmod: u8) -> u8 {
    [2, 1, 2, 3, 3, 4, 4, 5][acmod as usize & 7]
}

/// Fills `p` from an AC-3 or E-AC-3 sync frame at the start of `b`. Returns the bsid (0 when there is no frame).
pub fn ac3_props(b: &[u8], p: &mut EsProps) -> u8 {
    if b.len() < 8 || b[0] != 0x0B || b[1] != 0x77 {
        return 0;
    }
    let bsid = b[5] >> 3;
    if bsid <= 10 {
        let fscod = b[4] >> 6;
        let frmsizecod = (b[4] & 0x3F) as usize;
        if fscod == 3 || frmsizecod >= 38 {
            return 0;
        }
        p.sample_rate = [48000, 44100, 32000][fscod as usize];
        p.bitrate_kbps = AC3_RATES[frmsizecod / 2];
        let mut r = BitReader::new(&b[5..]);
        r.u(5);
        r.u(3);
        let acmod = r.u(3) as u8;
        if acmod & 1 != 0 && acmod != 1 {
            r.u(2);
        }
        if acmod & 4 != 0 {
            r.u(2);
        }
        if acmod == 2 {
            r.u(2);
        }
        let lfe = r.bit();
        p.channels = acmod_channels(acmod) + lfe as u8;
        p.layout = layout(acmod_channels(acmod), lfe);
        p.object_type = "AC-3".into();
    } else if bsid <= 16 {
        let mut r = BitReader::new(&b[2..]);
        let _strmtyp = r.u(2) as u32;
        let _sub = r.u(3) as u32;
        let frmsiz = r.u(11) as u32;
        let fscod = r.u(2) as u32;
        let (rate, blocks) = if fscod == 3 {
            let f2 = r.u(2) as u32;
            ([24000, 22050, 16000, 0][f2 as usize], 6)
        } else {
            let nb = r.u(2) as u32;
            (
                [48000, 44100, 32000][fscod as usize],
                [1, 2, 3, 6][nb as usize],
            )
        };
        let acmod = r.u(3) as u8;
        let lfe = r.bit();
        p.sample_rate = rate;
        p.channels = acmod_channels(acmod) + lfe as u8;
        p.layout = layout(acmod_channels(acmod), lfe);
        let bytes = (frmsiz + 1) * 2;
        if rate > 0 {
            p.bitrate_kbps = (bytes as u64 * 8 * rate as u64 / (blocks as u64 * 256) / 1000) as u32;
        }
        p.object_type = "E-AC-3".into();
    } else {
        return 0;
    }
    bsid
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identification_table() {
        let none = EsDescriptors::default();
        assert_eq!(
            identify(0x1B, &none).codec.fourcc(),
            Some(FourCc::new("avc1"))
        );
        assert_eq!(identify(0x24, &none).codec, EsCodec::Hevc);
        assert_eq!(
            identify(0x03, &none).codec.fourcc(),
            Some(FourCc::new(".mp2"))
        );
        assert_eq!(identify(0x42, &none).codec, EsCodec::Avs);
        assert_eq!(
            identify(0xD2, &none).codec.fourcc(),
            Some(FourCc::new("avs2"))
        );
        let ac3 = EsDescriptors {
            ac3: Some(Default::default()),
            ..Default::default()
        };
        assert_eq!(
            identify(0x06, &ac3).codec.fourcc(),
            Some(FourCc::new("ac-3"))
        );
        assert_eq!(
            identify(0x81, &none).codec.fourcc(),
            Some(FourCc::new("ac-3"))
        );
        let reg = EsDescriptors {
            registration: Some(*b"AC-3"),
            ..Default::default()
        };
        assert_eq!(
            identify(0x06, &reg).codec.fourcc(),
            Some(FourCc::new("ac-3"))
        );
        let e = EsDescriptors {
            eac3: Some(Default::default()),
            ..Default::default()
        };
        assert_eq!(identify(0x06, &e).codec.fourcc(), Some(FourCc::new("ec-3")));
        assert_eq!(
            identify(0x87, &none).codec.fourcc(),
            Some(FourCc::new("ec-3"))
        );
        let dra = EsDescriptors {
            registration: Some(*b"DRA1"),
            ..Default::default()
        };
        assert_eq!(identify(0x06, &dra).codec, EsCodec::Dra);
        assert_eq!(identify(0x99, &none).rule, CodecRule::StreamType(0x99));
    }

    /// MSB-first bit writer for hand-built headers, with emulation prevention added by `nal`.
    #[derive(Default)]
    struct Bw {
        bits: Vec<bool>,
    }
    impl Bw {
        fn u(&mut self, n: u32, v: u64) -> &mut Self {
            for i in (0..n).rev() {
                self.bits.push((v >> i) & 1 == 1);
            }
            self
        }
        fn ue(&mut self, v: u32) -> &mut Self {
            let x = v as u64 + 1;
            let len = 64 - x.leading_zeros();
            self.u(len - 1, 0).u(len, x)
        }
        /// The NAL unit: header bytes, then the RBSP with its stop bit and emulation prevention.
        fn nal(&mut self, header: &[u8]) -> Vec<u8> {
            self.bits.push(true);
            while !self.bits.len().is_multiple_of(8) {
                self.bits.push(false);
            }
            let raw: Vec<u8> = self
                .bits
                .chunks(8)
                .map(|c| c.iter().fold(0u8, |a, &b| (a << 1) | b as u8))
                .collect();
            let mut out = header.to_vec();
            let mut zeros = 0;
            for b in raw {
                if zeros >= 2 && b <= 3 {
                    out.push(3);
                    zeros = 0;
                }
                zeros = if b == 0 { zeros + 1 } else { 0 };
                out.push(b);
            }
            out
        }
    }

    #[test]
    fn hevc_sps_with_inter_predicted_reference_sets() {
        let mut w = Bw::default();
        // VPS id, max_sub_layers_minus1 0, temporal id nesting.
        w.u(4, 0).u(3, 0).u(1, 1);
        // Profile, tier, level: Main, Main tier, level 3.1.
        w.u(2, 0)
            .u(1, 0)
            .u(5, 1)
            .u(32, 0x6000_0000)
            .u(48, 0)
            .u(8, 93);
        // SPS id, 4:2:0, 1280 x 720, no cropping, 8 bit, log2_max_poc_lsb 8, one ordering entry.
        w.ue(0)
            .ue(1)
            .ue(1280)
            .ue(720)
            .u(1, 0)
            .ue(0)
            .ue(0)
            .ue(4)
            .u(1, 1);
        w.ue(4).ue(2).ue(0);
        // Coding block and transform sizes; no scaling list; AMP, SAO; no PCM.
        w.ue(0)
            .ue(3)
            .ue(0)
            .ue(3)
            .ue(0)
            .ue(0)
            .u(1, 0)
            .u(1, 1)
            .u(1, 1)
            .u(1, 0);
        // Four short-term sets.
        w.ue(4);
        // 0: explicit, S0 = [-1, -2].
        w.ue(2).ue(0).ue(0).u(1, 1).ue(0).u(1, 1);
        // 1: from set 0 with delta -1, every entry used: S0 = [-1, -2, -3] (3 entries, 3 flags read).
        w.u(1, 1).u(1, 1).ue(0).u(1, 1).u(1, 1).u(1, 1);
        // 2: from set 1 with delta +1, every flag set: -1 + 1 is the current picture and drops out, leaving S0 = [-1, -2],
        // S1 = [1] (3 entries from 4 flags).
        w.u(1, 1).u(1, 0).ue(0).u(1, 1).u(1, 1).u(1, 1).u(1, 1);
        // 3: from set 2 with delta -1: 3 + 1 flags (one unused, with use_delta 0).
        w.u(1, 1)
            .u(1, 1)
            .ue(0)
            .u(1, 1)
            .u(1, 0)
            .u(1, 0)
            .u(1, 1)
            .u(1, 1);
        // No long-term pictures; temporal MVP, strong intra smoothing; VUI.
        w.u(1, 0).u(1, 1).u(1, 1).u(1, 1);
        // SAR 1:1; no overscan; video signal type with BT.709 colour; no chroma location, neutral, field, frame field, window.
        w.u(1, 1)
            .u(8, 1)
            .u(1, 0)
            .u(1, 1)
            .u(3, 5)
            .u(1, 0)
            .u(1, 1)
            .u(8, 1)
            .u(8, 1)
            .u(8, 1);
        w.u(1, 0).u(1, 0).u(1, 0).u(1, 0).u(1, 0);
        // Timing: 1001 / 60000.
        w.u(1, 1).u(32, 1001).u(32, 60000).u(1, 0);
        let nal = w.nal(&[33 << 1, 1]);
        let s = parse_hevc_sps(&nal).expect("SPS");
        assert_eq!(
            (s.width, s.height, s.profile_idc, s.level_idc),
            (1280, 720, 1, 93)
        );
        assert_eq!(s.sar, Some((1, 1)));
        assert_eq!(s.colour, Some((1, 1, 1)));
        assert_eq!(s.frame_rate, Some((60000, 1001)));

        // The derivation itself.
        let mut w = Bw::default();
        w.ue(2).ue(0).ue(0).u(1, 1).ue(0).u(1, 1);
        w.u(1, 1).u(1, 1).ue(0).u(1, 1).u(1, 1).u(1, 1);
        w.u(1, 1).u(1, 0).ue(0).u(1, 1).u(1, 1).u(1, 1).u(1, 1);
        let bytes = w.nal(&[]);
        let mut r = BitReader::new(&bytes);
        let mut sets = Vec::new();
        for i in 0..3 {
            st_ref_pic_set(&mut r, i, 3, &mut sets);
        }
        assert_eq!(sets[0].s0, vec![-1, -2]);
        assert_eq!(sets[1].s0, vec![-1, -2, -3]);
        assert_eq!(
            (sets[2].s0.clone(), sets[2].s1.clone()),
            (vec![-1, -2], vec![1])
        );
    }

    #[test]
    fn asc_signalling() {
        // AAC-LC 48 kHz stereo.
        let lc = build_asc(2, 3, 2, false);
        let a = parse_audio_specific_config(&mut BitReader::new(&lc));
        assert_eq!(
            (a.object_type, a.sample_rate, a.channel_config, a.sbr),
            (2, 48000, 2, false)
        );
        // Explicit HE-AAC: AOT 5, 24 kHz core, ext 48 kHz, then AOT 2.
        let v: u32 = (5 << 27) | (6 << 23) | (2 << 19) | (3 << 15) | (2 << 10);
        let a = parse_audio_specific_config(&mut BitReader::new(&v.to_be_bytes()));
        assert!(a.sbr && !a.ps);
        let mut p = EsProps::default();
        asc_props(&a, &mut p);
        assert_eq!(
            (p.sample_rate, p.core_sample_rate, p.channels),
            (48000, 24000, 2)
        );
        // HE-AAC v2: AOT 29, mono core.
        let v: u32 = (29 << 27) | (6 << 23) | (1 << 19) | (3 << 15) | (2 << 10);
        let a = parse_audio_specific_config(&mut BitReader::new(&v.to_be_bytes()));
        asc_props(&a, &mut p);
        assert_eq!(
            (p.object_type.as_str(), p.channels, p.sample_rate),
            ("HE-AAC v2", 2, 48000)
        );
        // Backward compatible SBR signalling.
        let bc = build_asc(2, 6, 2, true);
        let a = parse_audio_specific_config(&mut BitReader::new(&bc));
        assert!(a.sbr);
        asc_props(&a, &mut p);
        assert_eq!(p.sample_rate, 48000);
    }
}
