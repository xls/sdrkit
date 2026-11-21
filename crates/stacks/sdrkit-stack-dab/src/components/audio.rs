//! The audio components: the DAB+ superframe decoder (`dab.superframe`), the Layer II framer (`dab.mp2_framer`) and the Layer
//! II decoder (`dab.mp2_decoder`). DAB+ audio leaves the graph as AAC access units for the host's decoder. Layer II decodes
//! here and plays through `bcdata.playout`.
//!
//! Standard: TS 102 563, EN 300 401 clause 7.

use super::items::{AudioStatus, MscItem, PadEvent};
use crate::audio::dabplus::{latm, pad_of_au, AuEvent, AudioParams, SuperframeDecoder};
use crate::data::pad::PadItem;
use crate::msc::Content;
use crate::results::AudioCodec;
use crate::settings::{AacPackaging, AAC_PACKAGING};
use sdrkit_algo_dsp::resample::Resampler;
use sdrkit_algo_dsp::Frame;
use sdrkit_codec_mpa::{Decoder, Header};
use sdrkit_core::media::{AudioProps, Codec, MediaFormat, Packaging};
use sdrkit_core::Values;
use sdrkit_core::{Choice, ConfigError, SettingError, SettingId, SettingSet, Value};
use sdrkit_graph::{
    no_values, values_of, Applied, Component, ComponentInfo, Format, Io, Kind, MediaPacket,
    PacketPool, Port, RateFrom, TagKind, Timed,
};
use sdrkit_proto_bcdata::component::Pcm;
use sdrkit_proto_bcdata::mp2::{pad_of_frame, Framer};
use std::sync::Arc;

fn typed<T: sdrkit_graph::Item>() -> Format {
    Format::new(Kind::typed::<T>(), 0.0)
}

fn item_out<T: sdrkit_graph::Item>(name: &str) -> Port {
    Port::typed::<T>(name).rate_from(RateFrom::Fixed(0.0))
}

/// The media format of a DAB+ stream.
pub fn aac_format(p: &AudioParams, latm_packaging: bool) -> MediaFormat {
    let codec = match (p.sbr, p.ps) {
        (false, _) => Codec::Aac,
        (true, false) => Codec::HeAac,
        (true, true) => Codec::HeAacV2,
    };
    let mut f = MediaFormat::new(
        codec,
        if latm_packaging {
            Packaging::Latm
        } else {
            Packaging::Raw
        },
    );
    f.config = p.audio_specific_config().into();
    f.timebase = (1, p.output_rate());
    f.audio = Some(AudioProps {
        sample_rate: p.output_rate(),
        channels: p.output_channels(),
    });
    f
}

/// The codec name of DAB+ audio with these parameters.
fn aac_codec(p: &AudioParams) -> AudioCodec {
    match (p.sbr, p.ps) {
        (false, _) => AudioCodec::AacLc,
        (true, false) => AudioCodec::HeAac,
        (true, true) => AudioCodec::HeAacV2,
    }
}

sdrkit_core::settings! {
    /// The settings of [`SuperframeComponent`].
    pub enum SuperframeSetting {
        /// The packaging of the access units.
        AacPackaging = AAC_PACKAGING,
    }
}

/// The DAB+ superframe decoder component.
#[derive(Default)]
pub struct SuperframeComponent {
    latm: bool,
    selected: Option<u8>,
    dec: SuperframeDecoder,
    format: Option<(AudioParams, Arc<MediaFormat>)>,
    events: Vec<AuEvent>,
    status: AudioStatus,
    pads: PacketPool,
    units: PacketPool,
    latm_unit: Vec<u8>,
}

impl Component for SuperframeComponent {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "dab.superframe",
            vec![Port::typed::<MscItem>("in")],
            vec![
                item_out::<MediaPacket>("aac"),
                item_out::<PadEvent>("pad"),
                item_out::<AudioStatus>("status"),
            ],
        )
        .with_settings(SuperframeSetting::schema())
        .manual_tags()
    }

    fn configure(&mut self, _: &[Format]) -> Result<Vec<Format>, ConfigError> {
        let latm = self.latm;
        *self = SuperframeComponent {
            latm,
            ..Default::default()
        };
        Ok(vec![
            typed::<MediaPacket>(),
            typed::<PadEvent>(),
            typed::<AudioStatus>(),
        ])
    }

    fn process(&mut self, io: &mut Io) {
        let mut any = false;
        for it in io.items::<MscItem>(0) {
            match &it.value {
                MscItem::Start {
                    subch,
                    content,
                    selected,
                    ..
                } => {
                    if *selected {
                        let same = self.selected == Some(*subch) && *content == Content::DabPlus;
                        self.selected = (*content == Content::DabPlus).then_some(*subch);
                        if same {
                            // A reconfiguration of the same service: search the superframes again, keep the counters.
                            self.dec.resync();
                        } else {
                            self.dec = SuperframeDecoder::new();
                            self.format = None;
                            self.status = AudioStatus::default();
                            io.items_out::<PadEvent>(1).push(Timed {
                                time: it.time,
                                value: PadEvent::Reset(*subch),
                            });
                        }
                        any = true;
                    }
                }
                MscItem::Frame(f) => {
                    if Some(f.subch) != self.selected {
                        continue;
                    }
                    self.dec.push(&f.data, f.erased, f.time, &mut self.events);
                    any = true;
                }
            }
            for e in self.events.drain(..) {
                match e {
                    AuEvent::Unit {
                        data,
                        pts,
                        params,
                        time,
                    } => {
                        let fmt = match &self.format {
                            Some((p, f)) if *p == params => Arc::clone(f),
                            _ => {
                                let f = Arc::new(aac_format(&params, self.latm));
                                self.format = Some((params, Arc::clone(&f)));
                                let at = io.output(0).len();
                                io.output(0).tag_here(
                                    at,
                                    TagKind::ConfigChanged {
                                        config: f.config.clone(),
                                    },
                                );
                                f
                            }
                        };
                        if let Some(p) = pad_of_au(data.bytes()) {
                            // The element ends with the F-PAD; the X-PAD before it is in reverse order.
                            let (x, fpad) = p.split_at(p.len() - 2);
                            let item = PadItem {
                                fpad: [fpad[0], fpad[1]],
                                xpad: self.pads.make(x.len(), time, |o| {
                                    o.copy_from_slice(x);
                                    o.reverse();
                                }),
                                exact: true,
                            };
                            io.items_out::<PadEvent>(1).push(Timed {
                                time,
                                value: PadEvent::Pad(item),
                            });
                        }
                        let packet = if self.latm {
                            let v = &mut self.latm_unit;
                            v.clear();
                            latm(&params, data.bytes(), v);
                            self.units.make(v.len(), time, |o| o.copy_from_slice(v))
                        } else {
                            data
                        }
                        .stamped(Some(pts), Some(pts), true);
                        if self.status.first_unit_s == 0.0 {
                            self.status.first_unit_s = time.max(1e-9);
                        }
                        io.items_out::<MediaPacket>(0).push(Timed {
                            time,
                            value: MediaPacket::new(packet, fmt),
                        });
                    }
                    AuEvent::Lost { samples, .. } => {
                        let at = io.output(0).len();
                        io.output(0).tag_here(
                            at,
                            TagKind::Discontinuity {
                                lost: samples as u64,
                            },
                        );
                    }
                }
            }
        }
        if any {
            let s = &mut self.status;
            s.dabplus = self.dec.stats;
            if let Some(p) = self.dec.params {
                s.codec = Some(aac_codec(&p));
                s.rate = p.output_rate();
                s.channels = p.output_channels();
                s.sbr = p.sbr;
                s.ps = p.ps;
                s.surround = p.surround;
                s.units_per_superframe = p.units();
            }
            s.decoding = self.dec.stats.synced && self.dec.stats.access_units_ok > 0;
            let st = s.clone();
            io.items_out::<AudioStatus>(2).push(Timed {
                time: 0.0,
                value: st,
            });
        }
    }

    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        match SuperframeSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            SuperframeSetting::AacPackaging => {
                self.latm = AAC_PACKAGING.read(id, v)? == AacPackaging::Latm;
                self.format = None;
            }
        }
        Ok(Applied::Live)
    }

    fn values(&self) -> Values {
        let p = if self.latm {
            AacPackaging::Latm
        } else {
            AacPackaging::Raw
        };
        values_of([(SuperframeSetting::AacPackaging, Value::Choice(p.index()))])
    }
}

/// The media format of an MPEG Layer II stream.
pub fn mp2_format(h: &Header) -> MediaFormat {
    let mut f = MediaFormat::new(Codec::Mp2, Packaging::Raw);
    f.timebase = (1, h.sample_rate);
    f.audio = Some(AudioProps {
        sample_rate: h.sample_rate,
        channels: h.channels() as u8,
    });
    f
}

/// The Layer II framer component.
#[derive(Default)]
pub struct Mp2FramerComponent {
    selected: Option<u8>,
    framer: Framer,
    format: Option<(Header, Arc<MediaFormat>)>,
    pts: i64,
    status: AudioStatus,
    frames: PacketPool,
    pads: PacketPool,
    xpad: Vec<u8>,
}

impl Component for Mp2FramerComponent {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "dab.mp2_framer",
            vec![Port::typed::<MscItem>("in")],
            vec![
                item_out::<MediaPacket>("mp2"),
                item_out::<PadEvent>("pad"),
                item_out::<AudioStatus>("status"),
            ],
        )
        .manual_tags()
    }

    fn configure(&mut self, _: &[Format]) -> Result<Vec<Format>, ConfigError> {
        *self = Mp2FramerComponent::default();
        Ok(vec![
            typed::<MediaPacket>(),
            typed::<PadEvent>(),
            typed::<AudioStatus>(),
        ])
    }

    fn values(&self) -> Values {
        no_values()
    }

    fn process(&mut self, io: &mut Io) {
        let mut any = false;
        for it in io.items::<MscItem>(0) {
            match &it.value {
                MscItem::Start {
                    subch,
                    content,
                    selected,
                    ..
                } => {
                    if *selected {
                        self.selected = (*content == Content::Mp2).then_some(*subch);
                        self.framer.reset();
                        self.format = None;
                        self.pts = 0;
                        self.status = AudioStatus::default();
                        if self.selected.is_some() {
                            io.items_out::<PadEvent>(1).push(Timed {
                                time: it.time,
                                value: PadEvent::Reset(*subch),
                            });
                        }
                        let at = io.output(0).len();
                        io.output(0)
                            .tag_here(at, TagKind::Discontinuity { lost: 0 });
                        any = true;
                    }
                }
                MscItem::Frame(f) => {
                    if Some(f.subch) != self.selected {
                        continue;
                    }
                    any = true;
                    // The PTS runs on the transmission's time line: a lost frame still advances it (by its share of a frame at
                    // 24 kHz, where a frame spans two logical frames), so a consumer sees the gap.
                    let half = self
                        .format
                        .as_ref()
                        .is_some_and(|(h, _)| h.sample_rate < 32_000);
                    let held = self.framer.holding();
                    let frames = &mut self.frames;
                    let Some(frame) = self
                        .framer
                        .push(&f.data, f.erased)
                        .map(|b| frames.make(b.len(), f.time, |o| o.copy_from_slice(b)))
                    else {
                        if f.erased {
                            self.pts += match (half, held) {
                                (false, _) => 1152,
                                (true, true) => 1152,
                                (true, false) => 576,
                            };
                            let at = io.output(0).len();
                            io.output(0)
                                .tag_here(at, TagKind::Discontinuity { lost: 1152 });
                            self.status.mp2_bad += 1;
                        }
                        continue;
                    };
                    let Ok(h) = Header::parse(frame.bytes()) else {
                        self.pts += if half && !held { 576 } else { 1152 };
                        let at = io.output(0).len();
                        io.output(0)
                            .tag_here(at, TagKind::Discontinuity { lost: 1152 });
                        self.status.mp2_bad += 1;
                        continue;
                    };
                    let fmt = match &self.format {
                        Some((oh, f))
                            if (oh.sample_rate, oh.mode, oh.bitrate_kbps)
                                == (h.sample_rate, h.mode, h.bitrate_kbps) =>
                        {
                            Arc::clone(f)
                        }
                        _ => {
                            let f = Arc::new(mp2_format(&h));
                            self.format = Some((h, Arc::clone(&f)));
                            f
                        }
                    };
                    let x = &mut self.xpad;
                    if let Some(fpad) = pad_of_frame(frame.bytes(), x) {
                        io.items_out::<PadEvent>(1).push(Timed {
                            time: f.time,
                            value: PadEvent::Pad(PadItem {
                                fpad,
                                xpad: self.pads.make(x.len(), f.time, |o| o.copy_from_slice(x)),
                                exact: false,
                            }),
                        });
                    }
                    let packet = frame.stamped(Some(self.pts), Some(self.pts), true);
                    self.pts += 1152;
                    self.status.mp2_frames += 1;
                    self.status.frame_bytes = h.frame_len();
                    self.status.bitrate = h.bitrate_kbps;
                    self.status.rate = h.sample_rate;
                    self.status.channels = h.channels() as u8;
                    if self.status.first_unit_s == 0.0 {
                        self.status.first_unit_s = f.time.max(1e-9);
                    }
                    io.items_out::<MediaPacket>(0).push(Timed {
                        time: f.time,
                        value: MediaPacket::new(packet, fmt),
                    });
                }
            }
        }
        if any {
            let s = &mut self.status;
            s.codec = Some(AudioCodec::Mp2);
            s.decoding = s.mp2_frames > 0 && self.selected.is_some();
            let st = s.clone();
            io.items_out::<AudioStatus>(2).push(Timed {
                time: 0.0,
                value: st,
            });
        }
    }
}

/// The Layer II decoder component: frames in, [`Pcm`] items for `bcdata.playout` out (48 kHz stereo, or the number of frames
/// lost). A `Discontinuity` tag on the input marks frames lost before the next frame.
pub struct Mp2DecoderComponent {
    dec: Decoder,
    pcm: Vec<f32>,
    up: Option<Resampler<Frame<2>>>,
    up_out: Vec<Frame<2>>,
    pool: Vec<Arc<[Frame<2>]>>,
    produced: bool,
    rate: u32,
}

impl Default for Mp2DecoderComponent {
    fn default() -> Self {
        Mp2DecoderComponent {
            dec: Decoder::new(),
            pcm: Vec::with_capacity(2 * 1152),
            up: None,
            up_out: Vec::new(),
            pool: Vec::new(),
            produced: false,
            rate: 0,
        }
    }
}

impl Component for Mp2DecoderComponent {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "dab.mp2_decoder",
            vec![Port::typed::<MediaPacket>("in")],
            vec![item_out::<Pcm>("out")],
        )
        .manual_tags()
    }

    fn configure(&mut self, _: &[Format]) -> Result<Vec<Format>, ConfigError> {
        *self = Mp2DecoderComponent::default();
        Ok(vec![typed::<Pcm>()])
    }

    fn values(&self) -> Values {
        no_values()
    }

    fn process(&mut self, io: &mut Io) {
        let first = io.input(0).first();
        let n = io.items::<MediaPacket>(0).len();
        let mut ti = 0;
        for k in 0..=n {
            loop {
                let tag = io
                    .input(0)
                    .tags()
                    .get(ti)
                    .map(|t| (t.at - first, t.kind.clone()));
                match tag {
                    Some((at, kind)) if at <= k as u64 => {
                        if let TagKind::Discontinuity { lost } = kind {
                            let time = io.items::<MediaPacket>(0).get(k).map_or(0.0, |p| p.time);
                            self.mark_loss(io, lost, time);
                        }
                        ti += 1;
                    }
                    _ => break,
                }
            }
            if k == n {
                break;
            }
            let it = &io.items::<MediaPacket>(0)[k];
            let time = it.time;
            self.pcm.clear();
            match self.dec.decode(it.value.packet.bytes(), &mut self.pcm) {
                Ok(h) => {
                    let frames = Frame::<2>::view(&self.pcm);
                    let sound = if h.sample_rate != 48_000 {
                        if self.rate != h.sample_rate || self.up.is_none() {
                            self.up = Resampler::new(f64::from(h.sample_rate), 48_000.0).ok();
                            self.rate = h.sample_rate;
                        }
                        let Some(r) = self.up.as_mut() else {
                            continue;
                        };
                        self.up_out.clear();
                        r.process(frames, &mut self.up_out);
                        shared(&mut self.pool, &self.up_out)
                    } else {
                        self.rate = h.sample_rate;
                        shared(&mut self.pool, frames)
                    };
                    io.items_out::<Pcm>(0).push(Timed {
                        time,
                        value: Pcm::Sound(sound),
                    });
                    self.produced = true;
                }
                Err(_) => self.mark_loss(io, 1152, time),
            }
        }
    }
}

/// The frames `x` in a buffer of `pool` that no item holds any more, so a warm decoder allocates nothing.
fn shared(pool: &mut Vec<Arc<[Frame<2>]>>, x: &[Frame<2>]) -> Arc<[Frame<2>]> {
    for a in pool.iter_mut() {
        if a.len() == x.len() {
            if let Some(m) = Arc::get_mut(a) {
                m.copy_from_slice(x);
                return Arc::clone(a);
            }
        }
    }
    let a: Arc<[Frame<2>]> = x.into();
    if pool.len() < 16 {
        pool.push(Arc::clone(&a));
    }
    a
}

impl Mp2DecoderComponent {
    fn mark_loss(&mut self, io: &mut Io, frames: u64, time: f64) {
        if frames == 0 {
            self.produced = false;
            return;
        }
        if !self.produced {
            return;
        }
        let f = if self.rate != 0 && self.rate != 48_000 {
            frames * 48_000 / u64::from(self.rate)
        } else {
            frames
        };
        io.items_out::<Pcm>(0).push(Timed {
            time,
            value: Pcm::Lost(f as usize),
        });
    }
}
