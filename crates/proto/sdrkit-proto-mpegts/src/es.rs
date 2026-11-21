//! Elementary streams for the host's decoders. Each selected PES becomes packets with the stream's format (codec, packaging,
//! decoder configuration, properties), unwrapped PTS and DTS, and the keyframe, damaged, discontinuity and configuration-change
//! flags. A packet holds one video access unit or one audio frame: a PES with several frames gives several packets, and a
//! frame that two PES share is joined. An AAC frame in LOAS/LATM leaves as a bare access unit. An E-AC-3 packet is one access
//! unit: the syncframes of independent substream 0 for 6 audio blocks with the substreams that follow them.
//!
//! Standard: ISO/IEC 13818-1 2.4.3.6, ETSI TS 101 154, ETSI TS 102 366 annex E.

use crate::codec::{self, CodecInfo, EsCodec, EsKind, EsPackaging, EsProps};
use crate::pes::{PesPacket, Unwrapper};
use sdrkit_core::media::{AudioProps, MediaFormat, Packaging, Packet, VideoProps};
use sdrkit_core::StreamKind;
use sdrkit_util_bits::BitReader;
use std::sync::Arc;

/// One packet of an elementary stream for a decoder, with what the transport stream says about it.
#[derive(Clone, Debug)]
pub struct EsPacket {
    /// The PID it came from.
    pub pid: u16,
    /// The bytes with PTS, DTS (unwrapped, 90 kHz) and the keyframe flag.
    pub packet: Packet,
    /// The stream's format, shared.
    pub format: Arc<MediaFormat>,
    /// Some of it was lost or damaged (the video decoder conceals; the audio decoder drops the frame).
    pub damaged: bool,
    /// A discontinuity came before it.
    pub discontinuity: bool,
    /// The decoder configuration changed with this packet.
    pub config_changed: bool,
    /// The system time clock at its arrival (seconds), when the service has a PCR.
    pub stc: Option<f64>,
}

impl EsPacket {
    /// The host decoder that takes it.
    pub fn stream_kind(&self) -> Option<StreamKind> {
        self.format.codec.stream_kind()
    }

    /// Whether it goes to a decoder. A damaged audio frame does not: an audio decoder would read the damage as content
    /// of the stream, while a video decoder conceals it.
    pub fn decodable(&self) -> bool {
        !(self.damaged && self.format.codec.media() == sdrkit_core::media::MediaType::Audio)
    }

    /// The packet with its format, as the graph carries media.
    pub fn media(&self) -> sdrkit_graph::MediaPacket {
        sdrkit_graph::MediaPacket::new(self.packet.clone(), Arc::clone(&self.format))
    }
}

/// A video access unit waiting for the next PES, which may continue it.
#[derive(Clone, Debug)]
struct HeldAu {
    /// The access unit as a part of its PES payload, while nothing was appended (no copy).
    whole: Option<Packet>,
    /// The bytes otherwise.
    bytes: Vec<u8>,
    pts: Option<i64>,
    dts: Option<i64>,
    damaged: bool,
    discontinuity: bool,
    random_access: bool,
    time: f64,
    stc: Option<f64>,
}

impl HeldAu {
    fn data(&self) -> &[u8] {
        self.whole.as_ref().map_or(&self.bytes, Packet::bytes)
    }
    fn append(&mut self, more: &[u8]) {
        if let Some(w) = self.whole.take() {
            self.bytes = Vec::with_capacity(w.len + more.len());
            self.bytes.extend_from_slice(w.bytes());
        }
        self.bytes.extend_from_slice(more);
    }
}

/// The bytes of an audio frame header that describe the stream, without the bits that change from frame to frame (padding,
/// frame length, CRC). `None` for a stream with no such header, or a frame too short to hold it.
fn audio_header_key(codec: EsCodec, packaging: EsPackaging, d: &[u8]) -> Option<u64> {
    let key = |b: &[u8]| b.iter().fold(0u64, |k, &x| k << 8 | u64::from(x));
    match (codec, packaging) {
        // ISO/IEC 11172-3 2.4.1.3: the padding bit is bit 1 of byte 2.
        (EsCodec::Mp2, _) if d.len() >= 4 => Some(key(&[d[0], d[1], d[2] & !0x02, d[3]])),
        // ISO/IEC 13818-7 6.2: the fixed header is the first 28 bits.
        (EsCodec::Aac, EsPackaging::Adts) if d.len() >= 4 => {
            Some(key(&[d[0], d[1], d[2], d[3] & 0xF0]))
        }
        // ETSI TS 102 366 4.4.1 and E.1.2: the four bytes after the sync word and the CRC or the frame size.
        (EsCodec::Ac3 | EsCodec::Eac3, _) if d.len() >= 8 => Some(key(&d[4..8])),
        _ => None,
    }
}

/// Whether a LATM stream with the AudioSpecificConfig `a` and the AAC descriptor profile_and_level `pl` (0 when there is
/// none) has SBR and PS. SBR signalled in `a` stands. Otherwise the descriptor decides (audioProfileLevelIndication of
/// ISO/IEC 14496-3: 0x2C to 0x2F HE-AAC, 0x30 to 0x33 HE-AAC v2), and without it an AAC-LC core of 24 kHz or less has
/// implicit SBR. PS needs no signalling, so a mono core with SBR is taken to carry PS: a stream without PS data decodes
/// to two equal channels.
fn implicit_sbr_ps(a: &codec::Asc, pl: u8) -> (bool, bool) {
    let (sbr, ps) = if a.sbr {
        (true, a.ps)
    } else if (0x30..=0x33).contains(&pl) {
        (true, true)
    } else if (0x2C..=0x2F).contains(&pl) {
        (true, false)
    } else {
        (
            a.object_type == 2 && (1..=24_000).contains(&a.sample_rate),
            false,
        )
    };
    (sbr, ps || (sbr && a.channel_config == 1))
}

/// The part `part` of packet `p`, sharing its bytes. `None` if `part` does not lie in the bytes of `p`.
fn part_of(p: &Packet, part: &[u8]) -> Option<Packet> {
    let all = p.bytes();
    let off = (part.as_ptr() as usize).checked_sub(all.as_ptr() as usize)?;
    (off + part.len() <= all.len())
        .then(|| Packet::slice(&p.data, p.start + off, part.len(), p.time))
}

/// Largest access unit joined from several PES; a longer one is passed on as it is, marked damaged.
const MAX_JOINED_AU: usize = 4 << 20;

/// An E-AC-3 access unit being gathered: syncframes from one of independent substream 0 to the next one that starts a unit.
#[derive(Clone, Debug, Default)]
struct Eac3Unit {
    /// The first syncframe as a part of its PES payload, while it is the only one (no copy).
    whole: Option<Packet>,
    /// The bytes otherwise.
    bytes: Vec<u8>,
    /// Syncframes in the unit.
    frames: usize,
    /// Audio blocks of independent substream 0 in the unit.
    blocks: u32,
    pts: Option<i64>,
    damaged: bool,
    discontinuity: bool,
    time: f64,
    stc: Option<f64>,
}

impl Eac3Unit {
    fn data(&self) -> &[u8] {
        self.whole.as_ref().map_or(&self.bytes, Packet::bytes)
    }
    fn append(&mut self, whole: Option<Packet>, more: &[u8]) {
        if self.frames == 0 && self.bytes.is_empty() && whole.is_some() {
            self.whole = whole;
        } else {
            if let Some(w) = self.whole.take() {
                self.bytes.extend_from_slice(w.bytes());
            }
            self.bytes.extend_from_slice(more);
        }
        self.frames += 1;
    }
}

/// The stream type, the substream id and the audio blocks of the E-AC-3 syncframe `d` (ETSI TS 102 366 E.1.2.2): strmtyp
/// (2 bits), substreamid (3), frmsiz (11), fscod (2), then numblkscod (2), or fscod2 with 6 blocks when fscod is 3.
fn eac3_substream(d: &[u8]) -> (u8, u8, u32) {
    let blocks = if d[4] >> 6 == 3 {
        6
    } else {
        [1, 2, 3, 6][usize::from((d[4] >> 4) & 3)]
    };
    (d[2] >> 6, (d[2] >> 3) & 7, blocks)
}

/// The state of one elementary stream.
#[derive(Clone, Debug)]
pub struct EsState {
    /// The PID.
    pub pid: u16,
    /// What the signalling said.
    pub signalled: CodecInfo,
    /// What the stream is (after checking its first frames).
    pub info: CodecInfo,
    /// Its properties.
    pub props: EsProps,
    /// The decoder configuration (Annex B SPS/PPS/VPS, or the AudioSpecificConfig).
    pub config: Vec<u8>,
    format: Option<Arc<MediaFormat>>,
    pts: Unwrapper,
    dts: Unwrapper,
    last_pts: Option<i64>,
    checked: bool,
    /// A contradiction to log once: the codec signalled, and the codec that the stream carries.
    pub warning: Option<(EsCodec, EsCodec)>,
    sps: Vec<u8>,
    pps: Vec<u8>,
    vps: Vec<u8>,
    config_dirty: bool,
    /// Packets delivered.
    pub packets: u64,
    /// AAC descriptor profile_and_level, for HE-AAC signalling.
    pub aac_profile_level: Option<u8>,
    held: Option<HeldAu>,
    /// The video has reached its first random access point: access units are delivered from there on.
    started: bool,
    /// The presentation time of an open-GOP start (a recovery point or CRA picture rather than an IDR): until the next keyframe,
    /// pictures shown before it (leading pictures) refer to pictures before the start and are dropped.
    leading_before: Option<i64>,
    /// Video access units dropped before the first random access point (a decoder cannot use them).
    pub before_start: u64,
    audio_key: Option<u64>,
    /// The start of an audio frame whose end is in the next PES, with its time.
    audio_part: Vec<u8>,
    audio_part_time: f64,
    /// The presentation time of the next audio frame, 90 kHz, counted on from the last PES that had one.
    audio_pts: Option<i64>,
    /// The LATM stream had a multiplex configuration that [`latm_payload`] reads.
    latm_known: bool,
    /// The E-AC-3 access unit being gathered.
    eac3: Eac3Unit,
    /// Audio bytes skipped to find the next frame header.
    pub audio_skipped: u64,
}

/// What stands at the start of audio bytes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum AudioFrame {
    /// A frame of `len` bytes that lasts `ticks` of the 90 kHz clock (0 if the header does not say).
    Frame { len: usize, ticks: i64 },
    /// Too few bytes for the header.
    Short,
    /// No frame header.
    None,
}

/// The audio frame at the start of `d`, from its header: MPEG audio (ISO/IEC 11172-3 2.4.2.3), ADTS (ISO/IEC 13818-7
/// 6.2), LOAS (ISO/IEC 14496-3 1.7.2), AC-3 and E-AC-3 (ETSI TS 102 366 4.4.1, table 4.13, and E.1.2.2). `core_rate` is
/// the AAC core rate of a LOAS stream, 0 if not known. `None` for a codec whose frames this crate does not delimit.
fn audio_frame(
    codec: EsCodec,
    packaging: EsPackaging,
    core_rate: u32,
    d: &[u8],
) -> Option<AudioFrame> {
    let ticks = |samples: usize, rate: u32| match rate {
        0 => 0,
        r => (samples as i64 * 90_000) / i64::from(r),
    };
    let frame = |len: usize, min: usize, ticks: i64| {
        if len < min {
            AudioFrame::None
        } else {
            AudioFrame::Frame { len, ticks }
        }
    };
    Some(match (codec, packaging) {
        (EsCodec::Mp2, _) => {
            if d.len() < 4 {
                return Some(AudioFrame::Short);
            }
            match sdrkit_codec_mpa::Header::parse(d) {
                Ok(h) => frame(h.frame_len(), 4, ticks(h.samples(), h.sample_rate)),
                Err(_) => AudioFrame::None,
            }
        }
        (EsCodec::Aac, EsPackaging::Adts) => {
            if d.len() < 7 {
                return Some(AudioFrame::Short);
            }
            match codec::parse_adts(d) {
                Some(h) if d[0] == 0xFF && d[1] & 0xF6 == 0xF0 => {
                    let len = (usize::from(d[3] & 3) << 11)
                        | usize::from(d[4]) << 3
                        | usize::from(d[5] >> 5);
                    let rate = [
                        96000, 88200, 64000, 48000, 44100, 32000, 24000, 22050, 16000, 12000,
                        11025, 8000, 7350,
                    ]
                    .get(h.sf_index as usize)
                    .copied()
                    .unwrap_or(0);
                    frame(len, 7, ticks(1024 * (usize::from(d[6] & 3) + 1), rate))
                }
                _ => AudioFrame::None,
            }
        }
        (EsCodec::Aac, EsPackaging::Latm) => {
            if d.len() < 3 {
                return Some(AudioFrame::Short);
            }
            if (u16::from_be_bytes([d[0], d[1]]) >> 5) != 0x2B7 {
                return Some(AudioFrame::None);
            }
            let len = 3 + ((usize::from(d[1] & 0x1F) << 8) | usize::from(d[2]));
            frame(len, 4, ticks(1024, core_rate))
        }
        (EsCodec::Ac3 | EsCodec::Eac3, _) => {
            if d.len() < 6 {
                return Some(AudioFrame::Short);
            }
            if d[0] != 0x0B || d[1] != 0x77 {
                return Some(AudioFrame::None);
            }
            if d[5] >> 3 <= 10 {
                let (fscod, code) = (d[4] >> 6, usize::from(d[4] & 0x3F));
                if fscod == 3 || code >= 38 {
                    return Some(AudioFrame::None);
                }
                let kbps = codec::AC3_RATES[code / 2] as usize;
                let words = match fscod {
                    0 => 2 * kbps,
                    1 => kbps * 96_000 / 44_100 + (code & 1),
                    _ => 3 * kbps,
                };
                frame(
                    2 * words,
                    6,
                    ticks(1536, [48000, 44100, 32000][fscod as usize]),
                )
            } else {
                let words = ((usize::from(d[2] & 7) << 8) | usize::from(d[3])) + 1;
                let fscod = d[4] >> 6;
                let (rate, blocks) = if fscod == 3 {
                    ([24000, 22050, 16000, 0][usize::from((d[4] >> 4) & 3)], 6)
                } else {
                    (
                        [48000, 44100, 32000][fscod as usize],
                        [1, 2, 3, 6][usize::from((d[4] >> 4) & 3)],
                    )
                };
                frame(2 * words, 6, ticks(256 * blocks, rate))
            }
        }
        _ => return None,
    })
}

/// The access unit of the LOAS frame `f` (ISO/IEC 14496-3 1.7.3): the bits of the payload of an AudioMuxElement with one
/// programme, one layer, no sub-frames and frame length type 0. `known` tells that an earlier frame had such a
/// multiplex configuration, and is set by a frame that has one. `None` for another configuration.
fn latm_payload(f: &[u8], known: &mut bool) -> Option<Vec<u8>> {
    let body = f.get(3..)?;
    let mut r = BitReader::new(body);
    if !r.bit() {
        *known = false;
        codec::parse_stream_mux_config(&mut r)?;
        *known = true;
    }
    if !*known {
        return None;
    }
    let mut len = 0usize;
    loop {
        let n = r.u(8) as usize;
        len += n;
        if n != 255 {
            break;
        }
    }
    let start = r.pos();
    (!r.overrun() && len > 0 && start + 8 * len <= 8 * body.len())
        .then(|| extract_bits(body, start, start + 8 * len))
}

impl EsState {
    /// A stream signalled as `info`.
    pub fn new(pid: u16, info: CodecInfo) -> Self {
        EsState {
            pid,
            signalled: info,
            info,
            props: EsProps::default(),
            config: Vec::new(),
            format: None,
            pts: Unwrapper::default(),
            dts: Unwrapper::default(),
            last_pts: None,
            checked: false,
            warning: None,
            sps: Vec::new(),
            pps: Vec::new(),
            vps: Vec::new(),
            config_dirty: false,
            packets: 0,
            aac_profile_level: None,
            held: None,
            started: false,
            leading_before: None,
            before_start: 0,
            audio_key: None,
            audio_part: Vec::new(),
            audio_part_time: 0.0,
            audio_pts: None,
            latm_known: false,
            eac3: Eac3Unit::default(),
            audio_skipped: 0,
        }
    }

    /// The format, once known.
    pub fn format(&self) -> Option<&Arc<MediaFormat>> {
        self.format.as_ref()
    }

    /// Resets the timeline (a new selection or a discontinuity of the clock).
    pub fn reset_time(&mut self) {
        self.pts.reset();
        self.dts.reset();
        self.audio_pts = None;
        // An E-AC-3 unit being gathered would leave with a time of the old timeline.
        let bytes = std::mem::take(&mut self.eac3.bytes);
        self.eac3 = Eac3Unit {
            bytes,
            ..Eac3Unit::default()
        };
    }

    fn check_contents(&mut self, d: &[u8]) {
        if self.checked || d.len() < 4 {
            return;
        }
        self.checked = true;
        if self.info.kind() != EsKind::Audio {
            return;
        }
        let mpa = d[0] == 0xFF && d[1] & 0xE0 == 0xE0 && d[1] & 0x06 != 0;
        let adts = d[0] == 0xFF && d[1] & 0xF6 == 0xF0;
        let loas = (u16::from_be_bytes([d[0], d[1]]) >> 5) == 0x2B7;
        let ac3 = d[0] == 0x0B && d[1] == 0x77;
        let actual = if mpa {
            Some((EsCodec::Mp2, EsPackaging::Elementary))
        } else if adts {
            Some((EsCodec::Aac, EsPackaging::Adts))
        } else if loas {
            Some((EsCodec::Aac, EsPackaging::Latm))
        } else if ac3 {
            let bsid = d.get(5).map_or(0, |b| b >> 3);
            Some(if bsid > 10 {
                (EsCodec::Eac3, EsPackaging::Frames)
            } else {
                (EsCodec::Ac3, EsPackaging::Frames)
            })
        } else {
            None
        };
        if let Some((c, p)) = actual {
            if c != self.info.codec || p != self.info.packaging {
                // AAC signalled with descriptor 0x7C can be LATM. That is not a contradiction.
                if !(self.info.codec == EsCodec::Aac && c == EsCodec::Aac) {
                    self.warning = Some((self.info.codec, c));
                }
                self.info.codec = c;
                self.info.packaging = p;
            }
        }
    }

    fn update_video_config(&mut self, au: &[u8]) -> bool {
        let mut changed = false;
        match self.info.codec {
            EsCodec::H264 => {
                let mut sps = None;
                let mut pps = None;
                for (t, s, e) in codec::h264_nals(au) {
                    match t {
                        7 => sps = Some(&au[s..e]),
                        8 => pps = Some(&au[s..e]),
                        _ => {}
                    }
                }
                if let Some(s) = sps {
                    if s != self.sps.as_slice() {
                        if let Some(p) = codec::parse_h264_sps(s) {
                            codec::h264_props(&p, &mut self.props);
                        }
                        changed |= !self.sps.is_empty();
                        self.sps = s.to_vec();
                        self.config_dirty = true;
                    }
                }
                if let Some(p) = pps {
                    if p != self.pps.as_slice() {
                        changed |= !self.pps.is_empty();
                        self.pps = p.to_vec();
                        self.config_dirty = true;
                    }
                }
            }
            EsCodec::Hevc => {
                let (mut vps, mut sps, mut pps) = (None, None, None);
                for (t, s, e) in codec::hevc_nals(au) {
                    match t {
                        32 => vps = Some(&au[s..e]),
                        33 => sps = Some(&au[s..e]),
                        34 => pps = Some(&au[s..e]),
                        _ => {}
                    }
                }
                for (slot, new) in [(0, vps), (1, sps), (2, pps)] {
                    if let Some(n) = new {
                        let cur = match slot {
                            0 => &mut self.vps,
                            1 => &mut self.sps,
                            _ => &mut self.pps,
                        };
                        if n != cur.as_slice() {
                            changed |= !cur.is_empty();
                            *cur = n.to_vec();
                            self.config_dirty = true;
                            if slot == 1 {
                                if let Some(p) = codec::parse_hevc_sps(n) {
                                    codec::hevc_props(&p, &mut self.props);
                                }
                            }
                        }
                    }
                }
            }
            EsCodec::Mpeg2Video | EsCodec::Mpeg1Video => {
                let before = (
                    self.props.width,
                    self.props.height,
                    self.props.frame_rate,
                    self.props.profile.clone(),
                );
                if codec::mpeg2_props(au, &mut self.props) {
                    let after = (
                        self.props.width,
                        self.props.height,
                        self.props.frame_rate,
                        self.props.profile.clone(),
                    );
                    if after != before {
                        changed |= before.0 != 0;
                        self.config_dirty = true;
                    }
                }
            }
            _ => {}
        }
        if self.config_dirty {
            self.config.clear();
            for n in [&self.vps, &self.sps, &self.pps] {
                if !n.is_empty() {
                    self.config.extend_from_slice(&[0, 0, 0, 1]);
                    self.config.extend_from_slice(n);
                }
            }
        }
        changed
    }

    fn update_audio_config(&mut self, d: &[u8]) -> bool {
        // A frame whose header bytes are those of the frame before describes the stream the same way.
        let key = audio_header_key(self.info.codec, self.info.packaging, d);
        if key.is_some() && key == self.audio_key {
            return false;
        }
        self.audio_key = key;
        let before = (
            self.props.sample_rate,
            self.props.channels,
            self.props.sbr,
            self.props.ps,
        );
        match (self.info.codec, self.info.packaging) {
            (EsCodec::Aac, EsPackaging::Adts) => {
                if let Some(h) = codec::parse_adts(d) {
                    let sr = [
                        96000, 88200, 64000, 48000, 44100, 32000, 24000, 22050, 16000, 12000,
                        11025, 8000, 7350,
                    ][h.sf_index as usize];
                    // Implicit SBR: signalled by the AAC descriptor (profile_and_level 0x2C to 0x33), or a core rate of 24 kHz or less.
                    let pl = self.aac_profile_level.unwrap_or(0);
                    let ps = (0x30..=0x33).contains(&pl);
                    let sbr =
                        ps || (0x2C..=0x2F).contains(&pl) || (h.object_type == 2 && sr <= 24000);
                    let a = codec::Asc {
                        object_type: h.object_type,
                        sample_rate: sr,
                        channel_config: h.channels,
                        sbr,
                        ps,
                        ext_rate: if sbr { sr * 2 } else { 0 },
                    };
                    codec::asc_props(&a, &mut self.props);
                    self.config = codec::build_asc(h.object_type, h.sf_index, h.channels, sbr);
                }
            }
            (EsCodec::Aac, EsPackaging::Latm) => {
                if let Some((mut a, bits)) = codec::parse_latm(d) {
                    let pl = self.aac_profile_level.unwrap_or(0);
                    (a.sbr, a.ps) = implicit_sbr_ps(&a, pl);
                    codec::asc_props(&a, &mut self.props);
                    // The AudioSpecificConfig as the stream sends it.
                    self.config = extract_bits(&d[3..], bits.start, bits.end);
                }
            }
            (EsCodec::Mp2, _) => {
                codec::mpa_props(d, &mut self.props);
            }
            (EsCodec::Ac3 | EsCodec::Eac3, _) => {
                codec::ac3_props(d, &mut self.props);
            }
            _ => {}
        }
        let after = (
            self.props.sample_rate,
            self.props.channels,
            self.props.sbr,
            self.props.ps,
        );
        if after != before {
            self.config_dirty = true;
            return before.0 != 0;
        }
        false
    }

    fn rebuild_format(&mut self) {
        let Some(codec) = self.info.codec.media_codec(&self.props) else {
            self.format = None;
            self.config_dirty = false;
            return;
        };
        let packaging = match self.info.packaging {
            EsPackaging::AnnexB => Packaging::AnnexB,
            EsPackaging::Adts => Packaging::Adts,
            // A LOAS frame leaves as its access unit, with the AudioSpecificConfig in the format.
            _ => Packaging::Raw,
        };
        let mut f = MediaFormat::new(codec, packaging);
        f.config = Arc::from(self.config.as_slice());
        if self.info.kind() == EsKind::Video {
            f.video = Some(VideoProps {
                width: self.props.width,
                height: self.props.height,
                frame_rate: self.props.frame_rate,
                aspect: self.props.dar,
                ..Default::default()
            });
        } else if self.info.kind() == EsKind::Audio {
            f.audio = Some(AudioProps {
                sample_rate: self.props.sample_rate,
                channels: self.props.channels,
            });
        }
        self.format = Some(Arc::new(f));
        self.config_dirty = false;
    }

    /// Whether a keyframe can start the video: H.264 and HEVC with their parameter sets known (SPS and PPS, and the VPS for
    /// HEVC), MPEG video with a sequence header in the access unit. Other codecs start anywhere.
    fn can_start_at(&self, au: &[u8]) -> bool {
        match self.info.codec {
            EsCodec::H264 => !self.sps.is_empty() && !self.pps.is_empty(),
            EsCodec::Hevc => !self.vps.is_empty() && !self.sps.is_empty() && !self.pps.is_empty(),
            EsCodec::Mpeg2Video | EsCodec::Mpeg1Video => {
                au.windows(4).any(|w| w == [0, 0, 1, 0xB3])
            }
            _ => true,
        }
    }

    /// Whether a keyframe starts a closed group of pictures (nothing after it refers to anything before it): an H.264 IDR picture,
    /// an HEVC IDR or BLA picture. MPEG video and an H.264 recovery point or HEVC CRA picture may have leading pictures.
    fn closed_start(&self, au: &[u8]) -> bool {
        match self.info.codec {
            EsCodec::H264 => codec::h264_nals(au).iter().any(|&(t, _, _)| t == 5),
            EsCodec::Hevc => codec::hevc_nals(au)
                .iter()
                .any(|&(t, _, _)| (16..=20).contains(&t)),
            EsCodec::Mpeg2Video | EsCodec::Mpeg1Video => false,
            _ => true,
        }
    }

    /// Whether an H.264 or HEVC access unit lacks its sequence parameter set (so the stored parameter sets must go before it).
    fn needs_parameter_sets(&self, au: &[u8]) -> bool {
        match self.info.codec {
            EsCodec::H264 => !codec::h264_nals(au).iter().any(|&(t, _, _)| t == 7),
            EsCodec::Hevc => !codec::hevc_nals(au).iter().any(|&(t, _, _)| t == 33),
            _ => false,
        }
    }

    /// Where the NAL unit after a leading access unit delimiter starts (0 when the access unit does not begin with one).
    fn after_delimiter(&self, au: &[u8]) -> usize {
        let (nals, aud) = match self.info.codec {
            EsCodec::H264 => (codec::h264_nals(au), 9),
            EsCodec::Hevc => (codec::hevc_nals(au), 35),
            _ => return 0,
        };
        match nals.as_slice() {
            [(t, _, _), (_, next, _), ..] if *t == aud => {
                // Back over the next NAL unit's start code (three or four bytes).
                let mut at = next.saturating_sub(3);
                if at > 0 && au[at - 1] == 0 {
                    at -= 1;
                }
                at
            }
            _ => 0,
        }
    }

    /// One PES of this stream. `out` gets its media packets.
    pub fn push(&mut self, pes: &PesPacket, stc: Option<f64>, out: &mut impl FnMut(EsPacket)) {
        let mut d: &[u8] = pes.payload.bytes();
        if d.is_empty() {
            return;
        }
        let is_video = self.info.kind() == EsKind::Video;
        // An audio stream starts at its first frame: a PES picked up in the middle may begin with the tail of a frame (MPEG audio
        // need not align its frames with its PES packets).
        if !is_video && !self.started && self.info.kind() == EsKind::Audio {
            match audio_sync(d) {
                Some(at) => {
                    self.started = true;
                    d = &d[at..];
                }
                None => {
                    self.before_start += 1;
                    return;
                }
            }
        }
        self.check_contents(d);
        let pts = pes.pts.map(|p| self.pts.unwrap(p));
        let dts = pes.dts.map(|p| self.dts.unwrap(p));
        // Frame rate from PTS intervals when the stream does not say.
        if is_video && self.props.frame_rate.is_none() {
            if let (Some(p), Some(l)) = (pts, self.last_pts) {
                let dt = (p - l).abs();
                if dt > 0 && dt < 9000 {
                    self.props.frame_rate = Some((90_000 / dt as u32, 1));
                    self.config_dirty = true;
                }
            }
        }
        if pts.is_some() {
            self.last_pts = pts;
        }
        if is_video {
            // One access unit per media packet: split at delimiters when a PES holds several, joined when a PES only continues
            // the access unit of the one before. The last access unit of a PES is held until the next PES shows whether it
            // continues it.
            let continues = !starts_access_unit(self.info.codec, d);
            let aus = split_access_units(self.info.codec, d);
            for (k, &(s, e)) in aus.iter().enumerate() {
                let bytes = &d[s..e];
                if k == 0 && continues {
                    if let Some(h) = self.held.as_mut() {
                        if h.data().len() + bytes.len() <= MAX_JOINED_AU {
                            h.append(bytes);
                            h.damaged |= pes.damaged || pes.discontinuity;
                            continue;
                        }
                        h.damaged = true;
                    }
                }
                self.release(out);
                let first = k == 0;
                self.held = Some(HeldAu {
                    whole: part_of(&pes.payload, bytes),
                    bytes: Vec::new(),
                    pts: if first { pts } else { None },
                    dts: if first { dts } else { None },
                    // A continuation with nothing before it lacks its start.
                    damaged: pes.damaged || (first && continues),
                    discontinuity: pes.discontinuity && first,
                    random_access: pes.random_access && first,
                    time: pes.time,
                    stc,
                });
            }
        } else if audio_frame(self.info.codec, self.info.packaging, 0, d).is_some() {
            self.push_audio_frames(pes, d, pts, stc, out);
        } else {
            let changed = self.update_audio_config(d);
            self.emit(
                part_of(&pes.payload, d),
                d,
                (pts, dts),
                (true, true),
                (pes.damaged, pes.discontinuity, changed),
                pes.time,
                stc,
                out,
            );
        }
    }

    /// Delivers the audio frames of the PES payload part `d`, one a packet. A frame that began in the PES before is
    /// completed first. The start of a frame that ends in the next PES is kept. `pts` is the time of the first frame
    /// that begins in `d`; each later frame is one frame length later.
    fn push_audio_frames(
        &mut self,
        pes: &PesPacket,
        d: &[u8],
        pts: Option<i64>,
        stc: Option<f64>,
        out: &mut impl FnMut(EsPacket),
    ) {
        let (codec, packaging) = (self.info.codec, self.info.packaging);
        let frame = |core: u32, d: &[u8]| {
            audio_frame(codec, packaging, core, d).unwrap_or(AudioFrame::None)
        };
        if pes.discontinuity {
            self.release_eac3(out);
            self.audio_part.clear();
            self.audio_pts = None;
        }
        let mut flags = (pes.damaged, pes.discontinuity);
        let mut at = 0;
        if !self.audio_part.is_empty() {
            // The kept start gets bytes of this PES until the header is whole, then the rest of the frame.
            let mut part = std::mem::take(&mut self.audio_part);
            let mut joined = false;
            loop {
                match frame(self.props.core_sample_rate, &part) {
                    AudioFrame::Frame { len, ticks } => {
                        let need = len - part.len().min(len);
                        if need > d.len() - at {
                            part.extend_from_slice(&d[at..]);
                            self.audio_part = part;
                            return;
                        }
                        part.extend_from_slice(&d[at..at + need]);
                        at += need;
                        let time = self.audio_part_time;
                        self.emit_audio_frame(None, &part, ticks, flags, time, stc, out);
                        joined = true;
                        break;
                    }
                    AudioFrame::Short if at < d.len() => {
                        part.push(d[at]);
                        at += 1;
                    }
                    AudioFrame::Short => {
                        self.audio_part = part;
                        return;
                    }
                    AudioFrame::None => break,
                }
            }
            if !joined {
                self.audio_skipped += part.len() as u64;
                at = 0;
            }
            part.clear();
            self.audio_part = part;
        }
        let mut first = true;
        while at < d.len() {
            match frame(self.props.core_sample_rate, &d[at..]) {
                AudioFrame::Frame { len, ticks } if at + len <= d.len() => {
                    if first {
                        self.audio_pts = pts.or(self.audio_pts);
                        first = false;
                    }
                    let whole = part_of(&pes.payload, &d[at..at + len]);
                    self.emit_audio_frame(
                        whole,
                        &d[at..at + len],
                        ticks,
                        flags,
                        pes.time,
                        stc,
                        out,
                    );
                    flags = (pes.damaged, false);
                    at += len;
                }
                AudioFrame::Frame { .. } | AudioFrame::Short => {
                    if first {
                        self.audio_pts = pts.or(self.audio_pts);
                    }
                    self.audio_part.extend_from_slice(&d[at..]);
                    self.audio_part_time = pes.time;
                    return;
                }
                AudioFrame::None => {
                    // Not a header: go on at the next sync pattern.
                    let skip = audio_sync(&d[at + 1..]).map_or(d.len() - at, |n| n + 1);
                    self.audio_skipped += skip as u64;
                    at += skip;
                }
            }
        }
    }

    /// Delivers the audio frame `bytes` at the running presentation time, and moves that time on by `ticks`. A LOAS
    /// frame leaves as its access unit; one whose multiplex configuration this crate does not read is not delivered. An
    /// E-AC-3 syncframe joins its access unit, which leaves when the next one starts.
    #[allow(clippy::too_many_arguments)]
    fn emit_audio_frame(
        &mut self,
        whole: Option<Packet>,
        bytes: &[u8],
        ticks: i64,
        (damaged, discontinuity): (bool, bool),
        time: f64,
        stc: Option<f64>,
        out: &mut impl FnMut(EsPacket),
    ) {
        if self.info.codec == EsCodec::Eac3 {
            self.gather_eac3(
                whole,
                bytes,
                ticks,
                (damaged, discontinuity),
                time,
                stc,
                out,
            );
            return;
        }
        let changed = self.update_audio_config(bytes);
        // The first LOAS frame gives the core rate that its own length in time needs.
        let core = self.props.core_sample_rate;
        let ticks = match ticks {
            0 if self.info.packaging == EsPackaging::Latm && core > 0 => {
                1024 * 90_000 / i64::from(core)
            }
            t => t,
        };
        let pts = self.audio_pts;
        self.audio_pts = pts.filter(|_| ticks > 0).map(|p| p + ticks);
        let flags = (damaged, discontinuity, changed);
        if self.info.packaging == EsPackaging::Latm {
            let mut known = self.latm_known;
            let unit = latm_payload(bytes, &mut known);
            self.latm_known = known;
            match unit {
                Some(u) => self.emit(None, &u, (pts, pts), (true, true), flags, time, stc, out),
                None => self.audio_skipped += bytes.len() as u64,
            }
        } else {
            self.emit(
                whole,
                bytes,
                (pts, pts),
                (true, true),
                flags,
                time,
                stc,
                out,
            );
        }
    }

    /// Adds the E-AC-3 syncframe `bytes` to the access unit being gathered (ETSI TS 102 366 E.1.3.1). Independent substream 0
    /// (strmtyp 0 or 2, substreamid 0) starts a unit once the unit before holds 6 audio blocks; its frames give the time, the
    /// header that describes the stream, and `ticks` to move the running time on. Dependent substreams and other independent
    /// substreams join the unit before them; such a syncframe with no unit before it is skipped.
    #[allow(clippy::too_many_arguments)]
    fn gather_eac3(
        &mut self,
        whole: Option<Packet>,
        bytes: &[u8],
        ticks: i64,
        (damaged, discontinuity): (bool, bool),
        time: f64,
        stc: Option<f64>,
        out: &mut impl FnMut(EsPacket),
    ) {
        let (strmtyp, substream, blocks) = eac3_substream(bytes);
        let starts = strmtyp != 1 && substream == 0;
        if starts && self.eac3.blocks >= 6 {
            self.release_eac3(out);
        }
        let u = &mut self.eac3;
        if u.frames == 0 {
            if !starts {
                self.audio_skipped += bytes.len() as u64;
                return;
            }
            u.pts = self.audio_pts;
            u.time = time;
            u.stc = stc;
        }
        u.damaged |= damaged;
        u.discontinuity |= discontinuity;
        if u.data().len() + bytes.len() > MAX_JOINED_AU {
            u.damaged = true;
            return;
        }
        u.append(whole, bytes);
        if starts {
            u.blocks += blocks;
            self.audio_pts = self.audio_pts.filter(|_| ticks > 0).map(|p| p + ticks);
        }
    }

    /// Delivers the E-AC-3 access unit gathered so far, if any.
    fn release_eac3(&mut self, out: &mut impl FnMut(EsPacket)) {
        if self.eac3.frames == 0 {
            return;
        }
        let mut u = std::mem::take(&mut self.eac3);
        let changed = self.update_audio_config(u.data());
        let flags = (u.damaged, u.discontinuity, changed);
        let whole = u.whole.take();
        let bytes = whole.as_ref().map_or(&u.bytes[..], Packet::bytes);
        self.emit(
            whole.clone(),
            bytes,
            (u.pts, u.pts),
            (true, true),
            flags,
            u.time,
            u.stc,
            out,
        );
        // The buffer is kept for the next unit.
        u.bytes.clear();
        self.eac3 = Eac3Unit {
            bytes: u.bytes,
            ..Eac3Unit::default()
        };
    }

    /// Passes on the access unit held back for joining (end of a file, or before a new selection).
    pub fn flush(&mut self, out: &mut impl FnMut(EsPacket)) {
        self.release(out);
        self.release_eac3(out);
    }

    fn release(&mut self, out: &mut impl FnMut(EsPacket)) {
        let Some(h) = self.held.take() else {
            return;
        };
        let au = h.data();
        let changed = self.update_video_config(au);
        let key = match self.info.codec {
            EsCodec::H264 => codec::h264_keyframe(au),
            EsCodec::Hevc => codec::hevc_keyframe(au),
            EsCodec::Mpeg2Video | EsCodec::Mpeg1Video => codec::mpeg2_keyframe(au),
            _ => h.random_access,
        };
        // A video stream starts at its first random access point: whatever comes before it refers to pictures the decoder never
        // had (a file written from there plays from its first picture, with no missing references).
        let mut whole = h.whole.clone();
        let mut owned = Vec::new();
        if !self.started {
            if !(key && self.can_start_at(au)) {
                self.before_start += 1;
                return;
            }
            self.started = true;
            if !self.closed_start(au) {
                self.leading_before = h.pts;
            }
            if self.needs_parameter_sets(au) {
                // The parameter sets came in an earlier access unit: they go into the first one delivered, after its access
                // unit delimiter (which must stay first).
                let at = self.after_delimiter(au);
                owned.reserve(self.config.len() + au.len());
                owned.extend_from_slice(&au[..at]);
                owned.extend_from_slice(&self.config);
                owned.extend_from_slice(&au[at..]);
                whole = None;
            }
        } else if let Some(start) = self.leading_before {
            if key {
                self.leading_before = None;
            } else if h.pts.is_some_and(|p| p < start) {
                self.before_start += 1;
                return;
            }
        }
        let clean = key && self.closed_start(au);
        let au: &[u8] = if owned.is_empty() { au } else { &owned };
        self.emit(
            whole,
            au,
            (h.pts, h.dts),
            (key, clean),
            (h.damaged, h.discontinuity, changed),
            h.time,
            h.stc,
            out,
        );
    }

    /// Delivers one media packet. `whole` holds `bytes` as a part of the PES payload (shared, not copied).
    #[allow(clippy::too_many_arguments)]
    fn emit(
        &mut self,
        whole: Option<Packet>,
        bytes: &[u8],
        (pts, dts): (Option<i64>, Option<i64>),
        (key, clean): (bool, bool),
        (damaged, discontinuity, changed): (bool, bool, bool),
        time: f64,
        stc: Option<f64>,
        out: &mut impl FnMut(EsPacket),
    ) {
        if self.config_dirty || self.format.is_none() {
            self.rebuild_format();
        }
        let Some(format) = self.format.clone() else {
            return;
        };
        let packet = match whole {
            Some(w) if w.len == bytes.len() => w,
            _ => Packet::new(Arc::from(bytes), time),
        };
        let mut packet = packet.stamped(pts, dts.or(pts), key).clean_if(clean);
        packet.time = time;
        self.packets += 1;
        out(EsPacket {
            pid: self.pid,
            packet,
            format,
            damaged,
            discontinuity,
            config_changed: changed,
            stc,
        });
    }
}

/// Where the first audio frame of `d` starts: an MPEG audio header (sync, a layer, a valid bit rate and sampling rate; ISO/IEC
/// 11172-3 2.4.1.3), an ADTS header (ISO/IEC 13818-7 6.2), a LOAS sync (ISO/IEC 14496-3 1.7.2) or an AC-3 / E-AC-3 sync word
/// (ETSI TS 102 366 4.3.1). None when there is none.
pub fn audio_sync(d: &[u8]) -> Option<usize> {
    d.windows(4).position(|w| {
        let mpa = w[0] == 0xFF
            && w[1] & 0xE0 == 0xE0
            && w[1] & 0x06 != 0
            && w[2] >> 4 != 0x0F
            && (w[2] >> 2) & 3 != 3;
        let adts = w[0] == 0xFF && w[1] & 0xF6 == 0xF0;
        let loas = (u16::from_be_bytes([w[0], w[1]]) >> 5) == 0x2B7;
        let ac3 = w[0] == 0x0B && w[1] == 0x77;
        mpa || adts || loas || ac3
    })
}

/// Whether a video PES payload begins a new access unit. H.264: an access unit delimiter, SPS, PPS or SEI
/// first, or a slice with first_mb_in_slice 0. HEVC: a delimiter, VPS, SPS, PPS or prefix SEI, or a slice segment with
/// first_slice_segment_in_pic_flag. MPEG video: a sequence, GOP or picture header. Bytes before the first start code continue the
/// previous access unit. Other codecs: every PES begins one.
pub fn starts_access_unit(codec: EsCodec, d: &[u8]) -> bool {
    let body = match d {
        [0, 0, 1, rest @ ..] => rest,
        [0, 0, 0, 1, rest @ ..] => rest,
        _ => {
            return !matches!(
                codec,
                EsCodec::H264 | EsCodec::Hevc | EsCodec::Mpeg2Video | EsCodec::Mpeg1Video
            )
        }
    };
    match codec {
        EsCodec::H264 => match body {
            [h, b, ..] => match h & 0x1F {
                6..=9 => true,
                // Slices: first_mb_in_slice is ue(v), 0 when its first bit is 1.
                1 | 5 => b & 0x80 != 0,
                _ => false,
            },
            _ => false,
        },
        EsCodec::Hevc => match body {
            [h, _, b, ..] => match (h >> 1) & 0x3F {
                32..=35 | 39 => true,
                0..=31 => b & 0x80 != 0,
                _ => false,
            },
            _ => false,
        },
        EsCodec::Mpeg2Video | EsCodec::Mpeg1Video => {
            matches!(body.first(), Some(0xB3 | 0xB8 | 0x00))
        }
        _ => true,
    }
}

fn extract_bits(d: &[u8], start: usize, end: usize) -> Vec<u8> {
    let mut r = BitReader::new(d);
    r.skip(start);
    let mut out = Vec::new();
    let mut n = end.saturating_sub(start);
    while n > 0 {
        let k = n.min(8) as u32;
        out.push(((r.u(k) as u32) << (8 - k)) as u8);
        n -= k as usize;
    }
    out
}

/// Access units in a PES payload: split at access unit delimiters (H.264 type 9, HEVC type 35) or, for MPEG video, at a picture start
/// after another picture. A PES with one access unit returns one range.
pub fn split_access_units(codec: EsCodec, d: &[u8]) -> Vec<(usize, usize)> {
    let mut cuts = Vec::new();
    match codec {
        EsCodec::H264 | EsCodec::Hevc => {
            let delim = if codec == EsCodec::H264 { 9 } else { 35 };
            for (s, _) in crate::nal::nal_units(d) {
                let t = if codec == EsCodec::H264 {
                    d[s] & 0x1F
                } else {
                    (d[s] >> 1) & 0x3F
                };
                if t == delim {
                    // The start code begins 3 (or 4) bytes before.
                    let mut c = s - 3;
                    if c > 0 && d[c - 1] == 0 {
                        c -= 1;
                    }
                    cuts.push(c);
                }
            }
        }
        EsCodec::Mpeg2Video | EsCodec::Mpeg1Video => {
            let mut seen_picture = false;
            let mut header_start: Option<usize> = None;
            let mut i = 0;
            while i + 4 <= d.len() {
                if d[i] == 0 && d[i + 1] == 0 && d[i + 2] == 1 {
                    match d[i + 3] {
                        0xB3 | 0xB8 => {
                            if seen_picture && header_start.is_none() {
                                header_start = Some(i);
                            }
                        }
                        0x00 => {
                            if seen_picture {
                                cuts.push(header_start.take().unwrap_or(i));
                            }
                            seen_picture = true;
                            header_start = None;
                        }
                        _ => {}
                    }
                    i += 4;
                } else {
                    i += 1;
                }
            }
        }
        _ => {}
    }
    cuts.retain(|&c| c > 0);
    cuts.dedup();
    let mut out = Vec::with_capacity(cuts.len() + 1);
    let mut s = 0;
    for c in cuts {
        out.push((s, c));
        s = c;
    }
    out.push((s, d.len()));
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::codec::identify;
    use crate::tables::EsDescriptors;

    fn sps_640x360() -> Vec<u8> {
        // A hand-made SPS: Main profile (77), level 3.0, 640x368 with 8 lines cropped, VUI with SAR 1:1 and 25 fps timing.
        let mut w = BitW::default();
        w.u(8, 77);
        w.u(8, 0);
        w.u(8, 30);
        w.ue(0); // id
        w.ue(0); // log2_max_frame_num - 4
        w.ue(0); // poc type 0
        w.ue(0); // log2_max_poc_lsb - 4
        w.ue(1); // max refs
        w.u(1, 0);
        w.ue(39); // 40 MBs
        w.ue(22); // 23 map units
        w.u(1, 1); // frame_mbs_only
        w.u(1, 1); // direct 8x8
        w.u(1, 1); // cropping
        w.ue(0);
        w.ue(0);
        w.ue(0);
        w.ue(4); // bottom 4*2 = 8
        w.u(1, 1); // vui
        w.u(1, 1);
        w.u(8, 1); // SAR 1:1
        w.u(1, 0);
        w.u(1, 0);
        w.u(1, 0);
        w.u(1, 1); // timing
        w.u(32, 1);
        w.u(32, 50);
        w.u(1, 1);
        w.u(1, 1); // stop bit
        let mut v = vec![0x67];
        v.extend(w.bytes());
        v
    }

    #[derive(Default)]
    struct BitW {
        bits: Vec<bool>,
    }
    impl BitW {
        fn u(&mut self, n: u32, v: u64) {
            for i in (0..n).rev() {
                self.bits.push((v >> i) & 1 == 1);
            }
        }
        fn ue(&mut self, v: u32) {
            let x = v as u64 + 1;
            let n = 64 - x.leading_zeros();
            self.u(n - 1, 0);
            self.u(n, x);
        }
        fn bytes(&self) -> Vec<u8> {
            self.bits
                .chunks(8)
                .map(|c| {
                    c.iter()
                        .enumerate()
                        .fold(0u8, |a, (i, &b)| a | ((b as u8) << (7 - i)))
                })
                .collect()
        }
    }

    fn au(sps: &[u8], idr: bool) -> Vec<u8> {
        let mut v = vec![0, 0, 0, 1, 0x09, 0xF0];
        if idr {
            v.extend_from_slice(&[0, 0, 0, 1]);
            v.extend_from_slice(sps);
            v.extend_from_slice(&[0, 0, 0, 1, 0x68, 0xCE, 0x3C, 0x80]);
            v.extend_from_slice(&[0, 0, 1, 0x65, 0x88, 0x84]);
        } else {
            v.extend_from_slice(&[0, 0, 1, 0x41, 0x9A, 0x00]);
        }
        v
    }

    #[test]
    fn configuration_and_keyframes() {
        let sps = sps_640x360();
        let p = codec::parse_h264_sps(&sps).unwrap();
        assert_eq!(
            (p.width, p.height, p.profile_idc, p.level_idc),
            (640, 360, 77, 30)
        );
        assert_eq!(p.frame_rate, Some((25, 1)));
        let mut es = EsState::new(0x200, identify(0x1B, &EsDescriptors::default()));
        let mut got = Vec::new();
        let mut sps2 = sps.clone();
        sps2[3] = 31; // level 3.1 at the third IDR
        for i in 0..9 {
            let idr = i % 3 == 0;
            let s = if i >= 6 { &sps2 } else { &sps };
            let pes = PesPacket {
                pid: 0x200,
                stream_id: 0xE0,
                pts: Some(i * 3600),
                dts: None,
                aligned: true,
                payload: Packet::new(Arc::from(au(s, idr).as_slice()), 0.0),
                damaged: false,
                discontinuity: false,
                random_access: idr,
                time: 0.0,
                info: None,
                aac_profile_level: None,
                pages: None,
                stc: None,
            };
            es.push(&pes, None, &mut |m| got.push(m));
        }
        es.flush(&mut |m| got.push(m));
        assert_eq!(got.len(), 9);
        assert!(got
            .iter()
            .enumerate()
            .all(|(i, m)| m.packet.keyframe == (i % 3 == 0)));
        let changes: Vec<usize> = got
            .iter()
            .enumerate()
            .filter(|m| m.1.config_changed)
            .map(|m| m.0)
            .collect();
        assert_eq!(changes, vec![6]);
        let f = &got[0].format;
        assert_eq!(
            (f.fourcc().as_str(), f.video.as_ref().unwrap().width),
            ("avc1", 640)
        );
        assert_eq!(es.props.aspect, "16:9");
        assert_eq!(es.props.level.as_deref(), Some("3.1"));
    }

    #[test]
    fn only_an_idr_picture_is_a_clean_start() {
        let sps = sps_640x360();
        let mut es = EsState::new(0x200, identify(0x1B, &EsDescriptors::default()));
        // A recovery point SEI (recovery_frame_cnt 0) before a slice that is not an IDR slice.
        let recovery = vec![
            0, 0, 0, 1, 0x09, 0xF0, 0, 0, 1, 0x06, 0x06, 0x01, 0xC4, 0x80, 0, 0, 1, 0x41, 0x9A,
            0x00,
        ];
        let units = [au(&sps, true), au(&sps, false), recovery, au(&sps, false)];
        let mut got = Vec::new();
        for (i, u) in units.iter().enumerate() {
            let pes = PesPacket {
                pid: 0x200,
                stream_id: 0xE0,
                pts: Some(i as i64 * 3600),
                dts: None,
                aligned: true,
                payload: Packet::new(Arc::from(u.as_slice()), 0.0),
                damaged: false,
                discontinuity: false,
                random_access: i % 2 == 0,
                time: 0.0,
                info: None,
                aac_profile_level: None,
                pages: None,
                stc: None,
            };
            es.push(&pes, None, &mut |m| got.push(m));
        }
        es.flush(&mut |m| got.push(m));
        let flags: Vec<(bool, bool)> = got
            .iter()
            .map(|m| (m.packet.keyframe, m.packet.clean_start))
            .collect();
        assert_eq!(
            flags,
            [(true, true), (false, false), (true, false), (false, false)]
        );
    }

    #[test]
    fn an_open_gop_start_drops_its_leading_pictures() {
        let mut es = EsState::new(0x201, identify(0x02, &EsDescriptors::default()));
        let mut got = Vec::new();
        let seq = [
            0u8, 0, 1, 0xB3, 0x2D, 0x02, 0x40, 0x33, 0xFF, 0xFF, 0xE0, 0x18,
        ];
        let pic = |kind: u8| [0u8, 0, 1, 0x00, 0x00, kind, 0xFF, 0xF8, 0, 0, 1, 0x01, 0x12];
        let i_pic: Vec<u8> = [&seq[..], &pic(0x0F)[..]].concat();
        // Decode order I(3) B(1) B(2) P(6) B(4) B(5) I(9): the two B pictures before the I refer to the GOP before it.
        for (bytes, pts) in [
            (i_pic.clone(), 3),
            (pic(0x18).to_vec(), 1),
            (pic(0x18).to_vec(), 2),
            (pic(0x10).to_vec(), 6),
            (pic(0x18).to_vec(), 4),
            (pic(0x18).to_vec(), 5),
            (i_pic, 9),
        ] {
            es.push(&video_pes(&bytes, Some(pts * 3600)), None, &mut |m| {
                got.push(m)
            });
        }
        es.flush(&mut |m| got.push(m));
        let pts: Vec<i64> = got.iter().map(|m| m.packet.pts.unwrap() / 3600).collect();
        assert_eq!(pts, [3, 6, 4, 5, 9]);
        assert_eq!(es.before_start, 2);
    }

    #[test]
    fn audio_starts_at_its_first_frame() {
        let mut es = EsState::new(0x300, identify(0x04, &EsDescriptors::default()));
        let mut got = Vec::new();
        // The tail of a frame, then an MPEG-1 layer II header (48 kHz, 256 kbit/s) and its data: 768 bytes.
        let mut frame = vec![0xFFu8, 0xFC, 0xC4, 0x00, 0x11, 0x22];
        frame.resize(768, 0x33);
        let tail = [0x4D, 0xA6, 0x00, 0x13];
        let mut first = tail.to_vec();
        first.extend_from_slice(&frame);
        es.push(&video_pes(&tail, Some(0)), None, &mut |m| got.push(m));
        es.push(&video_pes(&first, Some(3600)), None, &mut |m| got.push(m));
        es.push(&video_pes(&frame, Some(7200)), None, &mut |m| got.push(m));
        assert_eq!(es.before_start, 1, "a PES with no frame start");
        assert_eq!(got.len(), 2);
        assert_eq!(got[0].packet.bytes(), &frame[..]);
        assert_eq!(got[0].packet.pts, Some(3600));
        assert_eq!(audio_sync(&[0x0B, 0x77, 0, 0]), Some(0));
        assert_eq!(audio_sync(&[0, 0xFF, 0xF1, 0x50, 0x80]), Some(1));
        assert_eq!(
            audio_sync(&[0xFF, 0xFF, 0xFF, 0xFF]),
            None,
            "bit rate index 15 is not a header"
        );
    }

    #[test]
    fn video_starts_at_an_idr_with_its_parameter_sets() {
        let sps = sps_640x360();
        let mut es = EsState::new(0x200, identify(0x1B, &EsDescriptors::default()));
        let mut got = Vec::new();
        // Picked up mid-GOP: two P pictures, then an access unit with the parameter sets but a P picture, then an IDR that
        // carries no parameter sets of its own (they came before).
        let p = au(&sps, false);
        let mut sets_only = vec![0, 0, 0, 1, 0x09, 0xF0, 0, 0, 0, 1];
        sets_only.extend_from_slice(&sps);
        sets_only.extend_from_slice(&[
            0, 0, 0, 1, 0x68, 0xCE, 0x3C, 0x80, 0, 0, 1, 0x41, 0x9A, 0x00,
        ]);
        let bare_idr = vec![0, 0, 0, 1, 0x09, 0xF0, 0, 0, 1, 0x65, 0x88, 0x84];
        for (i, a) in [&p, &p, &sets_only, &bare_idr, &p].into_iter().enumerate() {
            es.push(&video_pes(a, Some(i as i64 * 3600)), None, &mut |m| {
                got.push(m)
            });
        }
        es.flush(&mut |m| got.push(m));
        assert_eq!(es.before_start, 3);
        assert_eq!(got.len(), 2);
        assert!(got[0].packet.keyframe);
        // The first delivered access unit holds the SPS and PPS after its delimiter, before the IDR slice.
        let nals: Vec<u8> = codec::h264_nals(got[0].packet.bytes())
            .iter()
            .map(|n| n.0)
            .collect();
        assert_eq!(nals, [9, 7, 8, 5]);
        assert_eq!(got[1].packet.bytes(), &p[..]);
    }

    fn video_pes(payload: &[u8], pts: Option<i64>) -> PesPacket {
        PesPacket {
            pid: 0x200,
            stream_id: 0xE0,
            pts,
            dts: None,
            aligned: pts.is_some(),
            payload: Packet::new(Arc::from(payload), 0.0),
            damaged: false,
            discontinuity: false,
            random_access: false,
            time: 0.0,
            info: None,
            aac_profile_level: None,
            pages: None,
            stc: None,
        }
    }

    #[test]
    fn access_units_split_across_pes_are_joined() {
        let sps = sps_640x360();
        let aus: Vec<Vec<u8>> = (0..6).map(|i| au(&sps, i % 3 == 0)).collect();
        let mut es = EsState::new(0x200, identify(0x1B, &EsDescriptors::default()));
        let mut got = Vec::new();
        // A stream picked up in the middle of an access unit: the fragment lacks its start.
        es.push(&video_pes(&[0x9A, 0x01, 0x02], None), None, &mut |m| {
            got.push(m)
        });
        // Each access unit in two PES: cut inside the NAL units (the second part has no start code) for the first three, then
        // between NAL units with a slice that does not start the picture (first_mb_in_slice 1).
        for (i, a) in aus.iter().enumerate().take(3) {
            let cut = a.len() - 2;
            es.push(
                &video_pes(&a[..cut], Some(i as i64 * 3600)),
                None,
                &mut |m| got.push(m),
            );
            es.push(&video_pes(&a[cut..], None), None, &mut |m| got.push(m));
        }
        let more_slices = [0u8, 0, 1, 0x41, 0x40, 0x11];
        let mut fourth = aus[3].clone();
        fourth.extend_from_slice(&more_slices);
        es.push(&video_pes(&aus[3], Some(3 * 3600)), None, &mut |m| {
            got.push(m)
        });
        es.push(&video_pes(&more_slices, None), None, &mut |m| got.push(m));
        // Two access units in one PES.
        let mut two = aus[4].clone();
        two.extend_from_slice(&aus[5]);
        es.push(&video_pes(&two, Some(4 * 3600)), None, &mut |m| got.push(m));
        es.flush(&mut |m| got.push(m));

        // The fragment comes before the first keyframe: it is dropped, and the stream starts at the IDR.
        assert_eq!(got.len(), 6, "six access units");
        assert_eq!(es.before_start, 1);
        let want = [&aus[0], &aus[1], &aus[2], &fourth, &aus[4], &aus[5]];
        for (k, w) in want.iter().enumerate() {
            let m = &got[k];
            assert_eq!(m.packet.bytes(), &w[..], "access unit {k}");
            assert!(!m.damaged);
            assert_eq!(m.packet.keyframe, k % 3 == 0, "keyframe of {k}");
        }
        let pts: Vec<Option<i64>> = got.iter().map(|m| m.packet.pts).collect();
        assert_eq!(
            pts,
            vec![
                Some(0),
                Some(3600),
                Some(7200),
                Some(10800),
                Some(14400),
                None
            ]
        );

        // MPEG-2 video: a picture whose slices arrive in a second PES. The stream starts at the I picture with a sequence header.
        let mut es = EsState::new(0x201, identify(0x02, &EsDescriptors::default()));
        let mut got = Vec::new();
        let pic = [0u8, 0, 1, 0x00, 0x00, 0x0F, 0xFF, 0xF8];
        let seq = [
            0u8, 0, 1, 0xB3, 0x2D, 0x02, 0x40, 0x33, 0xFF, 0xFF, 0xE0, 0x18,
        ];
        let slices = [0u8, 0, 1, 0x01, 0x12, 0x34, 0, 0, 1, 0x02, 0x56];
        for i in 0..3 {
            let first: Vec<u8> = if i == 1 {
                [&seq[..], &pic[..]].concat()
            } else {
                pic.to_vec()
            };
            es.push(&video_pes(&first, Some(i * 3600)), None, &mut |m| {
                got.push(m)
            });
            es.push(&video_pes(&slices, None), None, &mut |m| got.push(m));
        }
        es.flush(&mut |m| got.push(m));
        assert_eq!(got.len(), 2, "from the picture with the sequence header");
        assert_eq!(es.before_start, 1);
        assert_eq!(got[0].packet.len, seq.len() + pic.len() + slices.len());
        assert_eq!(got[1].packet.len, pic.len() + slices.len());
        assert!(starts_access_unit(EsCodec::Mpeg2Video, &pic));
        assert!(!starts_access_unit(EsCodec::Mpeg2Video, &slices));
        // HEVC: a delimiter starts one; a dependent slice segment (first_slice_segment_in_pic_flag 0) does not.
        assert!(starts_access_unit(
            EsCodec::Hevc,
            &[0, 0, 1, 35 << 1, 1, 0x50]
        ));
        assert!(starts_access_unit(
            EsCodec::Hevc,
            &[0, 0, 1, 1 << 1, 1, 0x80]
        ));
        assert!(!starts_access_unit(
            EsCodec::Hevc,
            &[0, 0, 1, 1 << 1, 1, 0x40]
        ));
    }

    #[test]
    fn aac_variants() {
        // A frame of 16 bytes, as its header says.
        let adts = |sf: u8| {
            let mut f = vec![0xFF, 0xF1, (1 << 6) | (sf << 2), 0x80, 0x02, 0x1F, 0xFC];
            f.resize(16, 0);
            f
        };
        // ADTS LC 48 kHz.
        let mut es = EsState::new(1, identify(0x0F, &EsDescriptors::default()));
        let pes = |d: Vec<u8>| PesPacket {
            pid: 1,
            stream_id: 0xC0,
            pts: Some(0),
            dts: None,
            aligned: true,
            payload: Packet::new(Arc::from(d.as_slice()), 0.0),
            damaged: false,
            discontinuity: false,
            random_access: false,
            time: 0.0,
            info: None,
            aac_profile_level: None,
            pages: None,
            stc: None,
        };
        let mut last = None;
        es.push(&pes(adts(3)), None, &mut |m| last = Some(m));
        let f = last.take().unwrap().format;
        assert_eq!(
            (
                f.fourcc().as_str(),
                f.audio.as_ref().unwrap().sample_rate,
                f.audio.as_ref().unwrap().channels
            ),
            ("aac ", 48000, 2)
        );
        // ADTS with implicit SBR (24 kHz core).
        let mut es = EsState::new(1, identify(0x0F, &EsDescriptors::default()));
        es.push(&pes(adts(6)), None, &mut |m| last = Some(m));
        let f = last.take().unwrap().format;
        assert_eq!(
            (f.fourcc().as_str(), f.audio.as_ref().unwrap().sample_rate),
            ("aach", 48000)
        );
        // LATM HE-AAC v1, explicit SBR: AOT 5, core 24 kHz (index 6), stereo, ext 48 kHz (index 3), AOT 2.
        let mut cfg = BitW::default();
        cfg.u(1, 0); // useSameStreamMux
        cfg.u(1, 0); // audioMuxVersion
        cfg.u(1, 1);
        cfg.u(6, 0);
        cfg.u(4, 0);
        cfg.u(3, 0);
        cfg.u(5, 5);
        cfg.u(4, 6);
        cfg.u(4, 2);
        cfg.u(4, 3);
        cfg.u(5, 2);
        cfg.u(3, 0);
        cfg.u(8, 0);
        cfg.u(3, 0); // the rest of latmBufferFullness
        cfg.u(1, 0); // otherDataPresent
        cfg.u(1, 0); // crcCheckPresent
        cfg.u(8, 2); // the payload: two bytes
        cfg.u(16, 0x1234);
        let body = cfg.bytes();
        let mut loas = vec![0x56, 0xE0, body.len() as u8];
        loas.extend(body);
        let mut es = EsState::new(1, identify(0x11, &EsDescriptors::default()));
        es.push(&pes(loas), None, &mut |m| last = Some(m));
        let m = last.take().unwrap();
        // The access unit leaves without the LOAS and LATM wrappers, with the AudioSpecificConfig in the format.
        assert_eq!(m.packet.bytes(), [0x12, 0x34]);
        assert_eq!(m.format.packaging, Packaging::Raw);
        assert!(!m.format.config.is_empty());
        let f = m.format;
        assert_eq!(
            (
                f.fourcc().as_str(),
                f.audio.as_ref().unwrap().sample_rate,
                es.props.core_sample_rate
            ),
            ("aach", 48000, 24000)
        );
        // HE-AAC v2 by the AAC descriptor's profile_and_level 0x30.
        let mut es = EsState::new(
            1,
            identify(
                0x06,
                &EsDescriptors {
                    dvb_aac: Some((0x30, None)),
                    ..Default::default()
                },
            ),
        );
        es.aac_profile_level = Some(0x30);
        es.push(&pes(adts(6)), None, &mut |m| last = Some(m));
        let f = last.take().unwrap().format;
        assert_eq!(
            (
                f.fourcc().as_str(),
                f.audio.as_ref().unwrap().sample_rate,
                f.audio.as_ref().unwrap().channels
            ),
            ("aacp", 48000, 2)
        );
    }

    /// Five frames in two PES: the first PES ends in the middle of the third frame. Returns the packets.
    fn in_two_pes(info: CodecInfo, frames: &[Vec<u8>]) -> Vec<EsPacket> {
        let all: Vec<u8> = frames.concat();
        let cut = frames[0].len() + frames[1].len() + frames[2].len() / 2;
        let mut es = EsState::new(0x300, info);
        let mut got = Vec::new();
        es.push(&video_pes(&all[..cut], Some(1000)), None, &mut |m| {
            got.push(m)
        });
        es.push(&video_pes(&all[cut..], Some(50_000)), None, &mut |m| {
            got.push(m)
        });
        // An E-AC-3 access unit leaves when the next one starts, or here.
        es.flush(&mut |m| got.push(m));
        assert_eq!(es.audio_skipped, 0);
        got
    }

    /// A frame of `len` bytes that starts with `head`, filled with `fill`.
    fn frame_of(head: &[u8], len: usize, fill: u8) -> Vec<u8> {
        let mut f = head.to_vec();
        f.resize(len, fill);
        f
    }

    #[test]
    fn every_audio_type_leaves_as_one_frame_a_packet() {
        let none = EsDescriptors::default();
        let ac3 = EsDescriptors {
            ac3: Some(Default::default()),
            ..Default::default()
        };
        let eac3 = EsDescriptors {
            eac3: Some(Default::default()),
            ..Default::default()
        };
        // The stream type and its descriptors, the frame header, the frame length, the 90 kHz ticks of a frame, the codec.
        type Case<'a> = (u8, &'a EsDescriptors, &'a [u8], usize, i64, &'a str);
        let cases: [Case; 4] = [
            // MPEG-1 Layer II, 48 kHz, 192 kbit/s: 1152 samples.
            (0x03, &none, &[0xFF, 0xFD, 0xA4, 0x00], 576, 2160, ".mp2"),
            // ADTS AAC-LC, 48 kHz, a frame of 64 bytes: 1024 samples.
            (
                0x0F,
                &none,
                &[0xFF, 0xF1, 0x4C, 0x80, 0x08, 0x1F, 0xFC],
                64,
                1920,
                "aac ",
            ),
            // AC-3 in a private stream with the AC-3 descriptor: 48 kHz, 64 kbit/s, 1536 samples.
            (
                0x06,
                &ac3,
                &[0x0B, 0x77, 0, 0, 0x08, 0x40, 0x40, 0],
                256,
                2880,
                "ac-3",
            ),
            // E-AC-3 with its descriptor: 128 words, 48 kHz, six blocks.
            (
                0x06,
                &eac3,
                &[0x0B, 0x77, 0x00, 0x7F, 0x34, 0x80, 0, 0],
                256,
                2880,
                "ec-3",
            ),
        ];
        for (stream_type, desc, head, len, ticks, fourcc) in cases {
            // The fill must not look like a frame header.
            let frames: Vec<Vec<u8>> = (0..5)
                .map(|k| frame_of(head, len, 0x11 + k as u8))
                .collect();
            let got = in_two_pes(identify(stream_type, desc), &frames);
            assert_eq!(got.len(), 5, "{fourcc}");
            for (k, m) in got.iter().enumerate() {
                assert_eq!(m.packet.bytes(), &frames[k][..], "{fourcc} frame {k}");
                assert_eq!(m.format.fourcc().as_str(), fourcc);
            }
            // The first two frames count on from the time of the first PES. The third began in the first PES, so it
            // counts on too. The fourth is the first that begins in the second PES: it takes the time of that PES.
            let pts: Vec<Option<i64>> = got.iter().map(|m| m.packet.pts).collect();
            let want = [1000, 1000 + ticks, 1000 + 2 * ticks, 50_000, 50_000 + ticks];
            assert_eq!(pts, want.map(Some), "{fourcc}");
        }
    }

    #[test]
    fn he_aac_in_loas_leaves_as_bare_access_units() {
        // Stream type 0x11. The first frame has the multiplex configuration (AAC-LC, 24 kHz core, stereo); the others say
        // "the same configuration". Each access unit is 3 bytes.
        let loas = |with_config: bool, unit: [u8; 3]| {
            let mut b = BitW::default();
            if with_config {
                b.u(1, 0); // useSameStreamMux
                b.u(1, 0); // audioMuxVersion
                b.u(1, 1); // allStreamsSameTimeFraming
                b.u(6, 0);
                b.u(4, 0);
                b.u(3, 0);
                b.u(5, 2); // AAC-LC
                b.u(4, 6); // 24 kHz
                b.u(4, 2); // stereo
                b.u(3, 0); // GASpecificConfig
                b.u(3, 0); // frameLengthType
                b.u(8, 0xFF); // latmBufferFullness
                b.u(1, 0); // otherDataPresent
                b.u(1, 0); // crcCheckPresent
            } else {
                b.u(1, 1);
            }
            b.u(8, 3);
            for x in unit {
                b.u(8, u64::from(x));
            }
            let body = b.bytes();
            let mut f = vec![0x56, 0xE0 | (body.len() >> 8) as u8, body.len() as u8];
            f.extend(body);
            f
        };
        let units: Vec<[u8; 3]> = (0..5).map(|k| [0x21, 0x10 + k, 0x9C]).collect();
        let frames: Vec<Vec<u8>> = units
            .iter()
            .enumerate()
            .map(|(k, u)| loas(k == 0, *u))
            .collect();
        let es_info = identify(0x11, &EsDescriptors::default());
        assert_eq!(es_info.packaging, EsPackaging::Latm);
        let got = in_two_pes(es_info, &frames);
        assert_eq!(got.len(), 5);
        for (k, m) in got.iter().enumerate() {
            assert_eq!(m.packet.bytes(), &units[k][..], "access unit {k}");
            assert_eq!(m.format.packaging, Packaging::Raw);
            // AAC-LC, index 6, 2 channels: 00010 0110 0010 000.
            assert_eq!(&m.format.config[..], [0x13, 0x10]);
        }
        // The AAC decoder takes this format: bare access units with the configuration.
        let mut set = sdrkit_core::codec::CodecSet::default();
        sdrkit_codec_aac::register(&mut set);
        let entry = set
            .find(got[0].format.fourcc(), "")
            .expect("an AAC decoder");
        assert!((entry.factory)(&got[0].format).is_some());
        assert_eq!(
            crate::components::no_decoder_note(&set, &got[0].format),
            None
        );
        // Implicit SBR at a core rate of 24 kHz: 1024 core samples are 3840 ticks.
        assert_eq!(got[1].packet.pts, Some(1000 + 3840));
        // A frame of another multiplex configuration (two sub-frames) is not passed on as sound.
        let mut b = BitW::default();
        b.u(1, 0);
        b.u(1, 0);
        b.u(1, 1);
        b.u(6, 1);
        b.u(32, 0);
        let body = b.bytes();
        let mut f = vec![0x56, 0xE0, body.len() as u8];
        f.extend(body);
        let mut es = EsState::new(1, identify(0x11, &EsDescriptors::default()));
        let mut n = 0;
        es.push(&video_pes(&f, Some(0)), None, &mut |_| n += 1);
        assert_eq!((n, es.audio_skipped), (0, f.len() as u64));
    }

    #[test]
    fn a_pes_with_many_layer_ii_frames_decodes_to_all_of_its_sound() {
        use sdrkit_core::codec::{CodecSet, Pcm};
        // One second of a tone: 42 frames of 24 ms in one PES, as a radio service with a low bit rate sends them.
        let mut enc = sdrkit_codec_mpa::Encoder::new(128, 48_000, true).unwrap();
        let mut payload = Vec::new();
        let mut frame = Vec::new();
        for k in 0..42 {
            let tone: Vec<f32> = (0..1152)
                .map(|i| ((k * 1152 + i) as f32 * 0.05).sin() * 0.3)
                .collect();
            enc.encode(&tone, &tone, 0, &mut frame).unwrap();
            payload.extend_from_slice(&frame);
        }
        let mut es = EsState::new(0x300, identify(0x03, &EsDescriptors::default()));
        let mut got = Vec::new();
        es.push(&video_pes(&payload, Some(0)), None, &mut |m| got.push(m));
        assert_eq!(got.len(), 42);
        let mut set = CodecSet::default();
        sdrkit_codec_mpa::register(&mut set);
        let entry = set
            .find(got[0].format.fourcc(), "")
            .expect("a Layer II decoder");
        let mut dec = (entry.factory)(&got[0].format).expect("the decoder takes the format");
        // The decoder takes one frame a call: the whole payload in one call gives the sound of its first frame only,
        // 24 ms of the second. This is why a packet holds one frame.
        let mut one = Pcm::default();
        dec.decode(&payload, false, &mut one).unwrap();
        assert_eq!(one.samples.len(), 1152 * 2);
        let mut dec = (entry.factory)(&got[0].format).expect("a new decoder");
        let mut pcm = Pcm::default();
        for m in &got {
            dec.decode(m.packet.bytes(), false, &mut pcm).unwrap();
        }
        // Stereo: 42 frames of 1152 samples a channel, one second of sound.
        assert_eq!(pcm.samples.len(), 42 * 1152 * 2);
        assert_eq!(pcm.rate_hz, 48_000);
        let loud = pcm.samples[20_000..]
            .iter()
            .filter(|s| s.abs() > 3000)
            .count();
        assert!(loud > 10_000, "{loud}");
    }

    #[test]
    fn an_audio_type_without_a_decoder_is_named() {
        use crate::components::no_decoder_note;
        use sdrkit_core::codec::CodecSet;
        use sdrkit_core::media::Codec;
        let mut set = CodecSet::default();
        sdrkit_codec_mpa::register(&mut set);
        let raw = |c| MediaFormat::new(c, Packaging::Raw);
        assert_eq!(no_decoder_note(&set, &raw(Codec::Mp2)), None);
        let note = no_decoder_note(&set, &raw(Codec::EAc3)).expect("a note");
        assert!(
            note.contains("E-AC-3") && note.contains("no decoder"),
            "{note}"
        );
        assert!(no_decoder_note(&set, &raw(Codec::Ac3)).is_some());
        // A decoder that refuses the packaging does not count.
        assert!(no_decoder_note(&set, &MediaFormat::new(Codec::Mp2, Packaging::Adts)).is_some());
        // A host without decoders takes the coded media itself: nothing to say.
        assert_eq!(
            no_decoder_note(&CodecSet::default(), &raw(Codec::EAc3)),
            None
        );
    }

    #[test]
    fn an_aac_pid_carrying_mpeg_audio_is_described_as_mpeg_audio() {
        let mut es = EsState::new(7, identify(0x0F, &EsDescriptors::default()));
        // A frame of 576 bytes (192 kbit/s at 48 kHz).
        let mut mp2 = vec![0xFF, 0xFD, 0xA4, 0x04];
        mp2.resize(576, 0);
        let pes = PesPacket {
            pid: 7,
            stream_id: 0xC0,
            pts: Some(0),
            dts: None,
            aligned: true,
            payload: Packet::new(Arc::from(mp2.as_slice()), 0.0),
            damaged: false,
            discontinuity: false,
            random_access: false,
            time: 0.0,
            info: None,
            aac_profile_level: None,
            pages: None,
            stc: None,
        };
        let mut last = None;
        es.push(&pes, None, &mut |m| last = Some(m));
        assert_eq!(es.info.codec, EsCodec::Mp2);
        assert!(es.warning.is_some());
        assert_eq!(last.unwrap().format.fourcc().as_str(), ".mp2");
    }

    /// An E-AC-3 syncframe of `words` 16-bit words with the given header fields and bsid 16 (ETSI TS 102 366 E.1.2.2).
    fn eac3_frame(
        strmtyp: u8,
        sub: u8,
        numblkscod: u8,
        acmod: u8,
        lfe: u8,
        words: usize,
    ) -> Vec<u8> {
        let frmsiz = words - 1;
        let mut f = vec![
            0x0B,
            0x77,
            strmtyp << 6 | sub << 3 | (frmsiz >> 8) as u8 & 7,
            frmsiz as u8,
            numblkscod << 4 | acmod << 1 | lfe,
            16 << 3,
        ];
        f.resize(2 * words, 0);
        f
    }

    fn eac3_packets(payload: &[u8]) -> (EsState, Vec<EsPacket>) {
        let mut es = EsState::new(0x300, identify(0x87, &EsDescriptors::default()));
        let mut got = Vec::new();
        es.push(&video_pes(payload, Some(9000)), None, &mut |m| got.push(m));
        es.flush(&mut |m| got.push(m));
        (es, got)
    }

    #[test]
    fn an_eac3_access_unit_holds_its_dependent_substream() {
        // A dependent frame with nothing before it, then 3 units of 5.1 (acmod 7, LFE) with a dependent substream of 2/2.
        let (main, dep) = (
            eac3_frame(0, 0, 3, 7, 1, 384),
            eac3_frame(1, 0, 3, 6, 0, 128),
        );
        let mut payload = dep.clone();
        for _ in 0..3 {
            payload.extend_from_slice(&main);
            payload.extend_from_slice(&dep);
        }
        let (es, got) = eac3_packets(&payload);
        assert_eq!(es.audio_skipped, dep.len() as u64);
        assert_eq!(got.len(), 3);
        for (k, m) in got.iter().enumerate() {
            assert_eq!(m.packet.bytes(), [&main[..], &dep[..]].concat(), "unit {k}");
            // 1536 samples at 48 kHz: 2880 ticks a unit, not a syncframe.
            assert_eq!(m.packet.pts, Some(9000 + 2880 * k as i64));
            assert_eq!(m.format.codec, sdrkit_core::media::Codec::EAc3);
            assert_eq!(m.format.packaging, Packaging::Raw);
            assert_eq!(m.format.audio.as_ref().map(|a| a.channels), Some(6));
            assert!(!m.config_changed);
        }
    }

    #[test]
    fn eac3_units_of_short_syncframes_hold_6_blocks() {
        // numblkscod 1: 2 blocks a syncframe, so 3 syncframes a unit.
        let f = eac3_frame(0, 0, 1, 2, 0, 100);
        let (_, got) = eac3_packets(&f.repeat(6));
        assert_eq!(got.len(), 2);
        assert_eq!(got[0].packet.bytes(), f.repeat(3));
        assert_eq!(got[1].packet.pts, Some(9000 + 2880));
    }

    #[test]
    fn an_eac3_change_from_51_to_stereo_is_flagged_once() {
        let (a, b) = (
            eac3_frame(0, 0, 3, 7, 1, 384),
            eac3_frame(0, 0, 3, 2, 0, 192),
        );
        let payload = [a.repeat(2), b.repeat(2)].concat();
        let (_, got) = eac3_packets(&payload);
        let seen: Vec<(u8, bool)> = got
            .iter()
            .map(|m| (m.format.audio.as_ref().unwrap().channels, m.config_changed))
            .collect();
        assert_eq!(seen, [(6, false), (6, false), (2, true), (2, false)]);
    }

    /// The E-AC-3 files of the AudioToolbox decoder tests: 0.4 s of 5.1, then 0.4 s of stereo.
    const EAC3_51: &[u8] =
        include_bytes!("../../../codec/sdrkit-codec-apple/tests/data/eac3_51_48k.ec3");
    const EAC3_20: &[u8] =
        include_bytes!("../../../codec/sdrkit-codec-apple/tests/data/eac3_20_48k.ec3");

    #[test]
    fn eac3_from_a_pes_reaches_the_decoder_or_is_named_as_without_one() {
        use crate::components::no_decoder_note;
        use sdrkit_core::codec::{CodecSet, Pcm};
        let (_, got) = eac3_packets(&[EAC3_51, EAC3_20].concat());
        // 1536-byte units of 5.1 and 384-byte units of stereo.
        let lens: Vec<usize> = got.iter().map(|m| m.packet.len).collect();
        assert!(lens.iter().all(|&n| n == 1536 || n == 384), "{lens:?}");
        let mut set = CodecSet::default();
        sdrkit_codec_mpa::register(&mut set);
        sdrkit_codec_aac::register(&mut set);
        #[cfg(any(target_os = "macos", target_os = "ios"))]
        sdrkit_codec_apple::register(&mut set);
        let note = no_decoder_note(&set, &got[0].format);
        if !cfg!(any(target_os = "macos", target_os = "ios")) {
            assert!(note.is_some_and(|n| n.contains("E-AC-3")));
            return;
        }
        assert_eq!(note, None);
        let mut dec = None;
        let mut channels = Vec::new();
        for m in &got {
            if m.config_changed || dec.is_none() {
                let entry = set.find(m.format.fourcc(), "").unwrap();
                dec = (entry.factory)(&m.format);
            }
            let mut pcm = Pcm::default();
            dec.as_mut()
                .unwrap()
                .decode(m.packet.bytes(), false, &mut pcm)
                .unwrap();
            channels.push(pcm.channels);
        }
        let first_stereo = channels.iter().position(|&c| c == 2).unwrap();
        assert!(first_stereo > 0 && channels[..first_stereo].iter().all(|&c| c == 6));
        assert!(channels[first_stereo..].iter().all(|&c| c == 2));
    }

    /// 1 s of LOAS frames of HE-AAC v1 at 64 kbit/s (mono) and HE-AAC v2 at 56 kbit/s (left 1 kHz, right 3 kHz, both
    /// at 0.5, with 12 kHz at 0.1 of full scale), written by the LATM writer of FFmpeg over the AAC encoder of
    /// AudioToolbox: implicit signalling (AAC-LC, 24 kHz, mono core), a StreamMuxConfig every 20 frames.
    const LATM_HE_MONO: &[u8] = include_bytes!("../tests/data/he_aac_mono_48k.latm");
    const LATM_HE_V2: &[u8] = include_bytes!("../tests/data/he_aac_v2_48k.latm");
    /// ADTS streams of HE-AAC (stereo, 24 kHz core) and HE-AAC v2 (mono core) of the AAC decoder tests: left 1 kHz at 0.5
    /// and 12 kHz at 0.1, right 3 kHz at 0.25 and noise.
    const ADTS_HE: &[u8] =
        include_bytes!("../../../codec/sdrkit-codec-aac/tests/data/he_48k_stereo.aac");
    const ADTS_HE_V2: &[u8] =
        include_bytes!("../../../codec/sdrkit-codec-aac/tests/data/hev2_48k.aac");

    /// The packets of one PES with `payload` from a LATM stream (stream type 0x11) with the AAC descriptor
    /// profile_and_level `pl`.
    fn latm_packets(payload: &[u8], pl: Option<u8>) -> Vec<EsPacket> {
        let mut es = EsState::new(0x300, identify(0x11, &EsDescriptors::default()));
        es.aac_profile_level = pl;
        let mut got = Vec::new();
        es.push(&video_pes(payload, Some(0)), None, &mut |m| got.push(m));
        assert_eq!(es.audio_skipped, 0);
        got
    }

    /// The sound of `packets` through the AAC decoder; every call must give the rate and channels of the format.
    fn decode_aac(packets: &[EsPacket]) -> sdrkit_core::codec::Pcm {
        let mut set = sdrkit_core::codec::CodecSet::default();
        sdrkit_codec_aac::register(&mut set);
        let format = &packets[0].format;
        let mut dec = (set.find(format.fourcc(), "").unwrap().factory)(format).expect("a decoder");
        let (mut all, mut one) = (
            sdrkit_core::codec::Pcm::default(),
            sdrkit_core::codec::Pcm::default(),
        );
        for m in packets {
            one.clear();
            dec.decode(m.packet.bytes(), false, &mut one).unwrap();
            let a = m.format.audio.as_ref().unwrap();
            assert_eq!((one.rate_hz, one.channels), (a.sample_rate, a.channels));
            all.samples.extend_from_slice(&one.samples);
            (all.rate_hz, all.channels) = (one.rate_hz, one.channels);
        }
        all
    }

    /// The amplitude, of full scale 1, of the component at `hz` in channel `ch` of `pcm` after 0.1 s.
    fn tone_level(pcm: &sdrkit_core::codec::Pcm, ch: usize, hz: f64) -> f64 {
        let n = usize::from(pcm.channels);
        let x: Vec<f64> = pcm
            .samples
            .iter()
            .skip(ch)
            .step_by(n)
            .skip(4800)
            .map(|&v| f64::from(v) / 32768.0)
            .collect();
        let (mut re, mut im) = (0.0, 0.0);
        for (i, v) in x.iter().enumerate() {
            let a = std::f64::consts::TAU * hz * i as f64 / f64::from(pcm.rate_hz);
            re += v * a.cos();
            im += v * a.sin();
        }
        2.0 * re.hypot(im) / x.len() as f64
    }

    /// The amplitude of a tone with the power that channel `ch` of `pcm` has between 11 and 13 kHz, over 0.1 s after 0.1 s.
    fn sbr_band_level(pcm: &sdrkit_core::codec::Pcm, ch: usize) -> f64 {
        let n = usize::from(pcm.channels);
        let x: Vec<f64> = pcm
            .samples
            .iter()
            .skip(ch)
            .step_by(n)
            .skip(4800)
            .take(4800)
            .map(|&v| f64::from(v) / 32768.0)
            .collect();
        let bins = 11_000 * x.len() / 48_000..13_000 * x.len() / 48_000;
        let p: f64 = bins
            .map(|k| {
                let (mut re, mut im) = (0.0, 0.0);
                for (i, v) in x.iter().enumerate() {
                    let a = std::f64::consts::TAU * (k * i) as f64 / x.len() as f64;
                    re += v * a.cos();
                    im -= v * a.sin();
                }
                re * re + im * im
            })
            .sum();
        2.0 * p.sqrt() / x.len() as f64
    }

    #[test]
    fn latm_he_aac_with_implicit_signalling_decodes_with_sbr() {
        let got = latm_packets(LATM_HE_MONO, None);
        assert!(got.len() >= 20);
        // A 24 kHz mono core: SBR to 48 kHz, and two channels in case PS data comes.
        let f = &got[0].format;
        assert_eq!(f.codec, sdrkit_core::media::Codec::HeAacV2);
        assert_eq!(&f.config[..], [0x13, 0x08]);
        assert_eq!(
            f.audio.as_ref().map(|a| (a.sample_rate, a.channels)),
            Some((48_000, 2))
        );
        // 1024 samples of the 24 kHz core.
        assert_eq!(got[1].packet.pts, Some(3840));
        let pcm = decode_aac(&got);
        // No PS data: the two channels are the same.
        assert!(pcm
            .samples
            .chunks(2)
            .all(|f| (i32::from(f[0]) - i32::from(f[1])).abs() <= 1));
        // The 12 kHz tone is above the core band: SBR made it.
        let (low, high) = (tone_level(&pcm, 0, 1000.0), sbr_band_level(&pcm, 0));
        assert!(low > 0.3 && high > 0.03, "{low} {high}");
    }

    #[test]
    fn latm_he_aac_v2_with_implicit_signalling_decodes_to_stereo() {
        let got = latm_packets(LATM_HE_V2, None);
        let pcm = decode_aac(&got);
        assert_eq!((pcm.rate_hz, pcm.channels), (48_000, 2));
        let l = [tone_level(&pcm, 0, 1000.0), tone_level(&pcm, 0, 3000.0)];
        let r = [tone_level(&pcm, 1, 1000.0), tone_level(&pcm, 1, 3000.0)];
        assert!(l[0] > 2.0 * l[1] && r[1] > 2.0 * r[0], "{l:?} {r:?}");
        let high = sbr_band_level(&pcm, 0);
        assert!(high > 0.03, "{l:?} {r:?} {high}");
    }

    /// The raw data blocks of an ADTS stream with one block a frame.
    fn adts_blocks(mut d: &[u8]) -> Vec<&[u8]> {
        let mut v = Vec::new();
        while let Some(h) = codec::parse_adts(d) {
            v.push(&d[h.header_len..h.frame_len]);
            d = &d[h.frame_len..];
        }
        v
    }

    /// The signalling of SBR and PS in a StreamMuxConfig.
    #[derive(Clone, Copy, Debug)]
    enum Signal {
        /// audioObjectType 5 or 29 around AAC-LC.
        Explicit,
        /// AAC-LC with the backward-compatible extension 0x2B7 (and 0x548 for PS).
        Compatible,
        /// AAC-LC alone.
        Implicit,
    }

    /// A LOAS stream of the AAC-LC blocks `blocks` with a core of 24 kHz and `channels`, signalled as `signal`, with a
    /// StreamMuxConfig in every 8th frame (ISO/IEC 14496-3 1.7.2 and 1.7.3).
    fn loas(blocks: &[&[u8]], channels: u64, ps: bool, signal: Signal) -> Vec<u8> {
        let mut out = Vec::new();
        for (k, block) in blocks.iter().enumerate() {
            let mut b = BitW::default();
            let config = k % 8 == 0;
            b.u(1, u64::from(!config)); // useSameStreamMux
            if config {
                b.u(1, 0); // audioMuxVersion
                b.u(1, 1); // allStreamsSameTimeFraming
                b.u(6, 0); // numSubFrames
                b.u(4, 0); // numProgram
                b.u(3, 0); // numLayer
                match signal {
                    Signal::Explicit => {
                        b.u(5, if ps { 29 } else { 5 });
                        b.u(4, 6); // 24 kHz core
                        b.u(4, channels);
                        b.u(4, 3); // 48 kHz with SBR
                        b.u(5, 2);
                        b.u(3, 0); // GASpecificConfig
                    }
                    Signal::Compatible => {
                        b.u(5, 2);
                        b.u(4, 6);
                        b.u(4, channels);
                        b.u(3, 0);
                        b.u(11, 0x2B7);
                        b.u(5, 5);
                        b.u(1, 1); // sbrPresentFlag
                        b.u(4, 3);
                        if ps {
                            b.u(11, 0x548);
                            b.u(1, 1); // psPresentFlag
                        }
                    }
                    Signal::Implicit => {
                        b.u(5, 2);
                        b.u(4, 6);
                        b.u(4, channels);
                        b.u(3, 0);
                    }
                }
                b.u(3, 0); // frameLengthType
                b.u(8, 0xFF); // latmBufferFullness
                b.u(1, 0); // otherDataPresent
                b.u(1, 0); // crcCheckPresent
            }
            let mut n = block.len();
            while n >= 255 {
                b.u(8, 255);
                n -= 255;
            }
            b.u(8, n as u64);
            for &x in *block {
                b.u(8, u64::from(x));
            }
            let body = b.bytes();
            out.extend_from_slice(&[0x56, 0xE0 | (body.len() >> 8) as u8, body.len() as u8]);
            out.extend_from_slice(&body);
        }
        out
    }

    #[test]
    fn latm_wrapping_of_he_aac_decodes_as_its_adts_stream_for_every_signalling() {
        use sdrkit_core::media::Codec;
        for (adts, codec, channels, ps) in [
            (ADTS_HE, Codec::HeAac, 2, false),
            (ADTS_HE_V2, Codec::HeAacV2, 1, true),
        ] {
            let blocks = adts_blocks(adts);
            assert!(blocks.len() > 40);
            // The ADTS stream through the AAC decoder, told the codec.
            let mut set = sdrkit_core::codec::CodecSet::default();
            sdrkit_codec_aac::register(&mut set);
            let f = MediaFormat::new(codec, Packaging::Adts);
            let mut dec = (set.find(f.fourcc(), "").unwrap().factory)(&f).unwrap();
            let mut want = sdrkit_core::codec::Pcm::default();
            let mut d = adts;
            while let Some(h) = codec::parse_adts(d) {
                dec.decode(&d[..h.frame_len], false, &mut want).unwrap();
                d = &d[h.frame_len..];
            }
            // The signalling, the descriptor, and whether PS is signalled. PS in a mono core needs no signalling: HE-AAC
            // v2 with audioObjectType 5 decodes the same.
            let cases = [
                (Signal::Explicit, None, ps),
                (Signal::Explicit, None, false),
                (Signal::Compatible, None, ps),
                (Signal::Implicit, Some(if ps { 0x30 } else { 0x2C }), ps),
                (Signal::Implicit, None, ps),
            ];
            for (signal, pl, ps) in cases {
                let got = latm_packets(&loas(&blocks, channels, ps, signal), pl);
                assert_eq!(got.len(), blocks.len(), "{codec:?} {signal:?}");
                let format = &got[0].format;
                assert_eq!(format.codec, codec, "{signal:?} {pl:?}");
                assert_eq!(
                    format.audio.as_ref().map(|a| (a.sample_rate, a.channels)),
                    Some((48_000, 2))
                );
                for (m, block) in got.iter().zip(&blocks) {
                    assert_eq!(m.packet.bytes(), *block);
                }
                let pcm = decode_aac(&got);
                assert!(pcm.samples == want.samples, "{codec:?} {signal:?} {pl:?}");
            }
        }
    }
}
