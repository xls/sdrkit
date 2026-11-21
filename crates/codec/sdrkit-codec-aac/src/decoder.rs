//! The AAC decoder: the stream configuration, the raw data block with its syntactic elements, the tool chain of each
//! element, the PCM output and the concealment of lost frames.
//!
//! Standard: ISO/IEC 14496-3 clauses 1.6.2.1 (AudioSpecificConfig), 1.A.2.2 (ADTS header), 4.4.2.1 (raw_data_block) and
//! 4.5.2 (decoding process).

use crate::filterbank::{Filterbank, Overlap};
use crate::ics::{Bands, Channel, IcsInfo};
use crate::sbr::Sbr;
use crate::stereo::{self, MsMask, Noise};
use crate::{Error, Result};
use sdrkit_core::codec::{AudioCodec, CodecError, Pcm};
use sdrkit_core::media::{ChannelLayout, Codec, MediaFormat, Packaging};
use sdrkit_proto_bcdata::aac::{AudioSpecificConfig, AOT_AAC_LC, SAMPLING_FREQUENCIES};
use sdrkit_util_bits::BitReader;

/// The configuration of a stream.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Config {
    /// The sampling frequency index of the core (0 for 96 kHz to 11 for 8 kHz).
    pub index: usize,
    /// The channel configuration (1 to 7).
    pub config: u8,
    /// Samples per channel of a core frame (1024 or 960).
    pub frame: usize,
    /// SBR is present.
    pub sbr: bool,
    /// SBR runs down-sampled: the output rate is the core rate.
    pub sbr_down: bool,
    /// Parametric stereo: a mono core with stereo output.
    pub ps: bool,
}

impl Config {
    /// The configuration of an AudioSpecificConfig. If `asc` signals no SBR, SBR is implicit when `codec` is HE-AAC or
    /// HE-AAC v2, or when `out_rate` is twice the core rate (the DAB+ rule); PS is implicit for HE-AAC v2.
    ///
    /// # Errors
    /// [`Error::Unsupported`] for an object type other than AAC-LC, a rate without band tables, or channel configuration 0.
    pub(crate) fn from_asc(
        asc: &AudioSpecificConfig,
        codec: Codec,
        out_rate: Option<u32>,
    ) -> Result<Config> {
        if asc.object_type != AOT_AAC_LC || !(1..=7).contains(&asc.channels) {
            return Err(Error::Unsupported);
        }
        let index = SAMPLING_FREQUENCIES
            .iter()
            .position(|&f| f == asc.sample_rate)
            .filter(|&i| i < 12)
            .ok_or(Error::Unsupported)?;
        let implicit =
            matches!(codec, Codec::HeAac | Codec::HeAacV2) || out_rate == Some(2 * asc.sample_rate);
        let sbr = asc.sbr_rate.is_some() || implicit;
        let ps = sbr && asc.channels == 1 && (asc.ps || codec == Codec::HeAacV2);
        Ok(Config {
            index,
            config: asc.channels,
            frame: if asc.frame_960 { 960 } else { 1024 },
            sbr,
            sbr_down: asc.sbr_rate == Some(asc.sample_rate),
            ps,
        })
    }

    /// The core sampling rate in Hz.
    pub(crate) fn core_rate(&self) -> u32 {
        SAMPLING_FREQUENCIES[self.index]
    }

    /// Channels of the core.
    fn core_channels(&self) -> usize {
        if self.config == 7 {
            8
        } else {
            usize::from(self.config)
        }
    }

    /// Channels of the output.
    fn out_channels(&self) -> usize {
        if self.ps {
            2
        } else {
            self.core_channels()
        }
    }

    /// Samples per channel of an output frame.
    fn out_frame(&self) -> usize {
        if self.sbr && !self.sbr_down {
            2 * self.frame
        } else {
            self.frame
        }
    }

    /// The output sampling rate in Hz.
    fn out_rate(&self) -> u32 {
        self.core_rate() * (self.out_frame() / self.frame) as u32
    }
}

/// The element ids of a raw data block (clause 4.4.2.1).
mod id {
    pub const SCE: u64 = 0;
    pub const CPE: u64 = 1;
    pub const CCE: u64 = 2;
    pub const LFE: u64 = 3;
    pub const DSE: u64 = 4;
    pub const PCE: u64 = 5;
    pub const FIL: u64 = 6;
}

/// The extension types of a fill element that carry SBR data (clause 4.4.2.7).
const EXT_SBR_DATA: u64 = 13;
const EXT_SBR_DATA_CRC: u64 = 14;

/// One audio element of the raw data block: its first channel and whether it is a channel pair.
#[derive(Clone, Copy, Debug, Default)]
struct Element {
    first: usize,
    pair: bool,
}

/// The decoding state of a stream of one configuration.
struct Core {
    cfg: Config,
    bands: Bands,
    fb: Filterbank,
    pair: [Channel; 2],
    ms: MsMask,
    noise: Noise,
    overlap: Vec<Overlap>,
    /// Core output by channel.
    time: Vec<Vec<f32>>,
    /// Output by channel when SBR runs.
    out: Vec<Vec<f32>>,
    /// The SBR state of each element.
    sbr: Vec<Sbr>,
    elements: Vec<Element>,
    /// The last output frame, interleaved, for concealment.
    last: Vec<f32>,
    erasures: u8,
}

impl Core {
    fn new(cfg: Config) -> Result<Core> {
        let bands = Bands::new(cfg.index, cfg.frame)?;
        let n = cfg.core_channels();
        let out_n = cfg.out_channels();
        let sbr = if cfg.sbr {
            (0..n)
                .map(|_| Sbr::new(cfg.core_rate(), cfg.frame, cfg.sbr_down, cfg.ps))
                .collect::<Result<Vec<_>>>()?
        } else {
            Vec::new()
        };
        Ok(Core {
            bands,
            fb: Filterbank::new(cfg.frame),
            pair: [Channel::new(cfg.frame), Channel::new(cfg.frame)],
            ms: MsMask::default(),
            noise: Noise::default(),
            overlap: (0..n).map(|_| Overlap::new(cfg.frame)).collect(),
            time: (0..n).map(|_| vec![0.0; cfg.frame]).collect(),
            out: if cfg.sbr {
                (0..out_n).map(|_| vec![0.0; cfg.out_frame()]).collect()
            } else {
                Vec::new()
            },
            sbr,
            elements: Vec::with_capacity(n),
            last: vec![0.0; cfg.out_frame() * out_n],
            erasures: 0,
            cfg,
        })
    }

    /// Decodes one raw data block into the core output.
    fn read_block(&mut self, data: &[u8]) -> Result<()> {
        let mut r = BitReader::new(data);
        let n = self.cfg.core_channels();
        self.elements.clear();
        let mut ch = 0;
        loop {
            let kind = r.u(3);
            match kind {
                id::SCE | id::LFE => {
                    r.skip(4);
                    if ch >= n {
                        return Err(Error::Syntax);
                    }
                    let c = &mut self.pair[0];
                    c.read(&mut r, &self.bands, None)?;
                    self.noise.fill(c, &self.bands, None);
                    c.apply_tns(&self.bands);
                    self.fb
                        .synthesize(&c.info, &c.spec, &mut self.overlap[ch], &mut self.time[ch]);
                    self.elements.push(Element {
                        first: ch,
                        pair: false,
                    });
                    ch += 1;
                }
                id::CPE => {
                    r.skip(4);
                    if ch + 2 > n {
                        return Err(Error::Syntax);
                    }
                    let common = r.bit();
                    let mut info = None;
                    self.ms.present = 0;
                    if common {
                        let i = IcsInfo::read(&mut r, &self.bands)?;
                        self.ms.present = r.u(2) as u8;
                        if self.ms.present == 1 {
                            for g in 0..i.n_groups {
                                for b in 0..i.max_sfb {
                                    self.ms.used[g][b] = r.bit();
                                }
                            }
                        }
                        info = Some(i);
                    }
                    let [l, rt] = &mut self.pair;
                    l.read(&mut r, &self.bands, info)?;
                    rt.read(&mut r, &self.bands, info)?;
                    self.noise.fill(l, &self.bands, None);
                    self.noise
                        .fill(rt, &self.bands, common.then_some((&*l, &self.ms)));
                    if common {
                        stereo::apply(l, rt, &self.ms, &self.bands);
                    }
                    for (k, c) in [l, rt].into_iter().enumerate() {
                        c.apply_tns(&self.bands);
                        self.fb.synthesize(
                            &c.info,
                            &c.spec,
                            &mut self.overlap[ch + k],
                            &mut self.time[ch + k],
                        );
                    }
                    self.elements.push(Element {
                        first: ch,
                        pair: true,
                    });
                    ch += 2;
                }
                id::CCE => return Err(Error::Unsupported),
                id::DSE => {
                    r.skip(4);
                    let align = r.bit();
                    let mut count = r.u(8) as usize;
                    if count == 255 {
                        count += r.u(8) as usize;
                    }
                    if align {
                        r.align();
                    }
                    r.skip(8 * count);
                }
                id::PCE => skip_pce(&mut r),
                id::FIL => {
                    let mut count = r.u(4) as usize;
                    if count == 15 {
                        count += r.u(8) as usize;
                        count -= 1;
                    }
                    let end = r.pos() + 8 * count;
                    if count > 0 {
                        let ext = r.peek(4);
                        if (ext == EXT_SBR_DATA || ext == EXT_SBR_DATA_CRC) && self.cfg.sbr {
                            r.skip(4);
                            let e = *self.elements.last().ok_or(Error::Syntax)?;
                            let at = self.elements.len() - 1;
                            self.sbr[at].read(&mut r, end, e.pair, ext == EXT_SBR_DATA_CRC);
                        }
                    }
                    if r.pos() > end {
                        return Err(Error::Syntax);
                    }
                    r.skip(end - r.pos());
                }
                _ => break,
            }
            if r.overrun() {
                return Err(Error::Syntax);
            }
        }
        if ch != n {
            return Err(Error::Syntax);
        }
        if self.cfg.sbr {
            for (e, sbr) in self.elements.iter().zip(self.sbr.iter_mut()) {
                let k = e.first;
                if e.pair {
                    let (a, b) = self.out.split_at_mut(k + 1);
                    sbr.process(&self.time[k..k + 2], &mut a[k], Some(&mut b[0]));
                } else if self.cfg.ps {
                    let (a, b) = self.out.split_at_mut(1);
                    sbr.process(&self.time[k..k + 1], &mut a[0], Some(&mut b[0]));
                } else {
                    sbr.process(&self.time[k..k + 1], &mut self.out[k], None);
                }
            }
        }
        Ok(())
    }

    /// Decodes or conceals one frame and appends its samples to `pcm`.
    fn decode(&mut self, data: &[u8], erasure: bool, pcm: &mut Pcm) -> Result<()> {
        pcm.rate_hz = self.cfg.out_rate();
        pcm.channels = self.cfg.out_channels() as u8;
        pcm.layout = match (self.cfg.out_channels(), self.cfg.config) {
            (6, 6) => ChannelLayout::Surround51,
            (c, _) => ChannelLayout::of_count(c as u16),
        };
        let len = self.last.len();
        pcm.samples.reserve(len);
        if erasure || data.is_empty() {
            // The last frame again with a falling gain, twice; then silence.
            self.erasures = self.erasures.saturating_add(1);
            let (g0, g1) = match self.erasures {
                1 => (1.0, 0.5),
                2 => (0.5, 0.0),
                _ => (0.0, 0.0),
            };
            let step = (g1 - g0) / len as f32;
            for (i, &v) in self.last.iter().enumerate() {
                pcm.samples.push(to_i16(v * (g0 + step * i as f32)));
            }
            return Ok(());
        }
        self.read_block(data)?;
        self.erasures = 0;
        let chans = if self.cfg.sbr { &self.out } else { &self.time };
        let n = chans.len();
        // Configuration 6 decodes C, L R, Ls Rs, LFE (ISO/IEC 14496-3 table 1.19); the output is L C R Ls Rs LFE.
        let swap = usize::from(self.cfg.config == 6);
        for i in 0..self.cfg.out_frame() {
            for (c, ch) in chans.iter().enumerate() {
                let at = if c < 2 { c ^ swap } else { c };
                self.last[i * n + at] = ch[i];
            }
        }
        pcm.samples.extend(self.last.iter().map(|&v| to_i16(v)));
        Ok(())
    }
}

fn to_i16(v: f32) -> i16 {
    v.round().clamp(-32768.0, 32767.0) as i16
}

/// Skips a program_config_element (clause 4.4.1.1).
fn skip_pce(r: &mut BitReader<'_>) {
    r.skip(4 + 2 + 4);
    let front = r.u(4) as usize;
    let side = r.u(4) as usize;
    let back = r.u(4) as usize;
    let lfe = r.u(2) as usize;
    let assoc = r.u(3) as usize;
    let cc = r.u(4) as usize;
    for _ in 0..2 {
        if r.bit() {
            r.skip(4);
        }
    }
    if r.bit() {
        r.skip(3);
    }
    r.skip(5 * (front + side + back) + 4 * (lfe + assoc) + 5 * cc);
    r.align();
    let comment = r.u(8) as usize;
    r.skip(8 * comment);
}

/// Reads an ADTS header (clause 1.A.2.2) and returns the configuration and the raw data block.
fn adts(f: &[u8], codec: Codec) -> Result<(Config, &[u8])> {
    if f.len() < 7 || f[0] != 0xFF || f[1] & 0xF6 != 0xF0 {
        return Err(Error::Syntax);
    }
    let header = if f[1] & 1 == 1 { 7 } else { 9 };
    let length = (usize::from(f[3] & 3) << 11) | (usize::from(f[4]) << 3) | usize::from(f[5] >> 5);
    if f[6] & 3 != 0 || length < header || length > f.len() {
        return Err(Error::Syntax);
    }
    let asc = AudioSpecificConfig {
        object_type: (f[2] >> 6) + 1,
        sample_rate: *SAMPLING_FREQUENCIES
            .get(usize::from((f[2] >> 2) & 0x0F))
            .ok_or(Error::Syntax)?,
        channels: ((f[2] & 1) << 2) | (f[3] >> 6),
        sbr_rate: None,
        ps: false,
        frame_960: false,
    };
    Ok((Config::from_asc(&asc, codec, None)?, &f[header..length]))
}

/// The AAC decoder of one stream, as a codec plug-in.
pub(crate) struct Decoder {
    codec: Codec,
    adts: bool,
    core: Option<Core>,
    /// The format asks for stereo: a 5.1 stream is mixed down.
    stereo: bool,
}

impl Decoder {
    /// A decoder for `format`: bare access units with an AudioSpecificConfig, or ADTS. Returns `None` for another
    /// packaging or a configuration this decoder does not decode.
    pub(crate) fn new(format: &MediaFormat) -> Option<Decoder> {
        match format.packaging {
            Packaging::Raw => {
                let asc = AudioSpecificConfig::parse(&format.config).ok()?;
                let out = format.audio.as_ref().map(|a| a.sample_rate);
                let cfg = Config::from_asc(&asc, format.codec, out).ok()?;
                Some(Decoder {
                    codec: format.codec,
                    adts: false,
                    core: Some(Core::new(cfg).ok()?),
                    stereo: wants_stereo(format),
                })
            }
            Packaging::Adts => Some(Decoder {
                codec: format.codec,
                adts: true,
                core: None,
                stereo: wants_stereo(format),
            }),
            _ => None,
        }
    }
}

/// Whether `format` asks for stereo from a stream of more channels.
fn wants_stereo(format: &MediaFormat) -> bool {
    format.audio.as_ref().is_some_and(|a| a.channels == 2)
}

impl AudioCodec for Decoder {
    fn decode(
        &mut self,
        frame: &[u8],
        erasure: bool,
        pcm: &mut Pcm,
    ) -> std::result::Result<(), CodecError> {
        // Once a configuration is accepted, a frame that fails is a damaged frame, also when its bits read as a tool that
        // this decoder lacks: one bad frame must not end the stream.
        let mut data = frame;
        if self.adts && !erasure {
            let started = self.core.is_some();
            let (cfg, block) = adts(frame, self.codec).map_err(|e| match e {
                Error::Unsupported if !started => CodecError::Unsupported,
                _ => CodecError::Frame,
            })?;
            if self.core.as_ref().is_none_or(|c| c.cfg != cfg) {
                let core = Core::new(cfg).map_err(|_| {
                    if started {
                        CodecError::Frame
                    } else {
                        CodecError::Unsupported
                    }
                })?;
                self.core = Some(core);
            }
            data = block;
        }
        let Some(core) = self.core.as_mut() else {
            return Ok(());
        };
        let at = pcm.samples.len();
        core.decode(data, erasure, pcm)
            .map_err(|_| CodecError::Frame)?;
        if self.stereo {
            // Without a matrix mixdown in a program_config_element, the gains of ITU-R BS.775: -3 dB for the centre and
            // the surround channels.
            pcm.downmix_51(
                at,
                std::f32::consts::FRAC_1_SQRT_2,
                std::f32::consts::FRAC_1_SQRT_2,
            );
        }
        Ok(())
    }
}
