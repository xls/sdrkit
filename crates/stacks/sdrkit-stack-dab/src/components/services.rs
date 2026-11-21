//! The data service components: packet mode (`dab.packet`), Journaline (`dab.journaline`), SPI (`dab.spi`), transmitter
//! identification (`dab.tii`) and DMB video (`dab.dmb`).

use super::items::{DataGroupItem, EnsembleItem, GroupSource, MotItem, MscItem, NullItem};
use crate::data::dmb::{DmbDecoder, DmbStatus};
use crate::data::packet::{PacketDecoder, PacketStats};
use crate::data::tii::{TiiDecoder, Transmitter};
use crate::ensemble::Ensemble;
use crate::msc::Content;
use sdrkit_core::media::{AudioProps, Codec, MediaFormat, Packaging, Packet, VideoProps};
use sdrkit_core::ConfigError;
use sdrkit_core::Values;
use sdrkit_graph::{
    no_values, Component, ComponentInfo, Format, Io, Item, Kind, MediaPacket, Port, RateFrom, Timed,
};
use sdrkit_proto_bcdata::aac::AudioSpecificConfig;
use sdrkit_proto_bcdata::{datagroup, journaline, spi};
use sdrkit_proto_mpegts::demux::sl::AccessUnit;
use std::collections::BTreeMap;
use std::sync::Arc;

fn typed<T: Item>() -> Format {
    Format::new(Kind::typed::<T>(), 0.0)
}

fn item_out<T: Item>(name: &str) -> Port {
    Port::typed::<T>(name).rate_from(RateFrom::Fixed(0.0))
}

/// Packet mode status.
#[derive(Clone, Debug, Default)]
pub struct PacketStatus {
    /// Per sub-channel counters.
    pub subchannels: Vec<(u8, PacketStats)>,
}
impl Item for PacketStatus {
    const KIND: &'static str = "dab.packet_status";
}

/// The packet mode component: packet-mode logical frames in, data groups out (with the packet-mode FEC when FIG 0/14 says so).
#[derive(Default)]
pub struct PacketComponent {
    decoders: BTreeMap<u8, PacketDecoder>,
    db: Arc<Ensemble>,
    out: Vec<(u16, Arc<[u8]>)>,
}

impl Component for PacketComponent {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "dab.packet",
            vec![
                Port::typed::<MscItem>("in"),
                Port::typed::<EnsembleItem>("ens"),
            ],
            vec![
                item_out::<DataGroupItem>("out"),
                item_out::<PacketStatus>("status"),
            ],
        )
    }
    fn configure(&mut self, _: &[Format]) -> Result<Vec<Format>, ConfigError> {
        *self = PacketComponent::default();
        Ok(vec![typed::<DataGroupItem>(), typed::<PacketStatus>()])
    }
    fn values(&self) -> Values {
        no_values()
    }
    fn process(&mut self, io: &mut Io) {
        for e in io.items::<EnsembleItem>(1) {
            if e.value.changed {
                self.db = e.value.db.clone();
            }
        }
        let mut any = false;
        for it in io.items::<MscItem>(0) {
            match &it.value {
                MscItem::Start { subch, content, .. } => {
                    if *content == Content::Packet {
                        let fec = self.db.subchannels.get(subch).is_some_and(|s| s.fec == 1);
                        self.decoders.insert(*subch, PacketDecoder::new(fec));
                    } else {
                        self.decoders.remove(subch);
                    }
                }
                MscItem::Frame(f) => {
                    let Some(d) = self.decoders.get_mut(&f.subch) else {
                        continue;
                    };
                    if f.erased {
                        continue;
                    }
                    any = true;
                    let out = &mut self.out;
                    d.push(&f.data, &mut |addr, g| out.push((addr, g.into())));
                    for (addr, g) in self.out.drain(..) {
                        io.items_out::<DataGroupItem>(0).push(Timed {
                            time: f.time,
                            value: DataGroupItem {
                                source: GroupSource::Packet(f.subch, addr),
                                data: g,
                                time: f.time,
                            },
                        });
                    }
                }
            }
        }
        if any {
            let s = PacketStatus {
                subchannels: self.decoders.iter().map(|(k, d)| (*k, d.stats())).collect(),
            };
            io.items_out::<PacketStatus>(1).push(Timed {
                time: 0.0,
                value: s,
            });
        }
    }
}

/// The Journaline objects received, for the report.
#[derive(Clone, Debug, Default)]
pub struct JournalineItem(pub Arc<journaline::Store>);
impl Item for JournalineItem {
    const KIND: &'static str = "dab.journaline";
}

/// The Journaline component: data groups of Journaline components in, JML objects out.
#[derive(Default)]
pub struct JournalineComponent {
    db: Arc<Ensemble>,
    store: journaline::Store,
}

impl JournalineComponent {
    /// Whether a data group source carries Journaline, and its extended header length.
    fn journaline(&self, src: GroupSource) -> Option<usize> {
        for s in self.db.services.values() {
            for a in s.user_apps.iter().filter(|a| a.app == 0x44A) {
                let ext = if a.xpad.is_some() {
                    a.data.get(3).copied()
                } else {
                    a.data.get(1).copied()
                }
                .unwrap_or(0) as usize;
                match src {
                    GroupSource::Xpad(t) => {
                        if a.xpad.is_some_and(|x| x.0 == t) {
                            return Some(ext);
                        }
                    }
                    GroupSource::Packet(sc, addr) => {
                        let comp = s.components.iter().find(|c| c.scids == Some(a.scids));
                        if comp
                            .is_some_and(|c| c.subch == Some(sc) && c.packet_address == Some(addr))
                        {
                            return Some(ext);
                        }
                    }
                    GroupSource::Reset => {}
                }
            }
        }
        None
    }
}

impl Component for JournalineComponent {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "dab.journaline",
            vec![
                Port::typed::<DataGroupItem>("xpad"),
                Port::typed::<DataGroupItem>("packet"),
                Port::typed::<EnsembleItem>("ens"),
            ],
            vec![item_out::<JournalineItem>("out")],
        )
    }
    fn configure(&mut self, _: &[Format]) -> Result<Vec<Format>, ConfigError> {
        *self = JournalineComponent::default();
        Ok(vec![typed::<JournalineItem>()])
    }
    fn values(&self) -> Values {
        no_values()
    }
    fn process(&mut self, io: &mut Io) {
        for e in io.items::<EnsembleItem>(2) {
            if e.value.changed {
                self.db = e.value.db.clone();
            }
        }
        let mut changed = false;
        for port in 0..2 {
            for it in io.items::<DataGroupItem>(port) {
                let Some(ext) = self.journaline(it.value.source) else {
                    continue;
                };
                let Ok(g) = datagroup::parse(&it.value.data) else {
                    continue;
                };
                if g.header.group_type != datagroup::GENERAL {
                    continue;
                }
                if let Ok(o) = journaline::decode(g.data, ext) {
                    self.store.add(o);
                    changed = true;
                }
            }
        }
        if changed {
            io.items_out::<JournalineItem>(0).push(Timed {
                time: 0.0,
                value: JournalineItem(Arc::new(self.store.clone())),
            });
        }
    }
}

/// The service and programme information received, for the report.
#[derive(Clone, Debug, Default)]
pub struct SpiItem(pub Arc<spi::Guide>);
impl Item for SpiItem {
    const KIND: &'static str = "dab.spi";
}

/// The SPI component: MOT objects of content type 7 in, services and programmes out.
#[derive(Default)]
pub struct SpiComponent {
    guide: spi::Guide,
}

impl Component for SpiComponent {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "dab.spi",
            vec![
                Port::typed::<MotItem>("xpad"),
                Port::typed::<MotItem>("packet"),
            ],
            vec![item_out::<SpiItem>("out")],
        )
    }
    /// The guide stays: it describes services of several ensembles.
    fn configure(&mut self, _: &[Format]) -> Result<Vec<Format>, ConfigError> {
        Ok(vec![typed::<SpiItem>()])
    }
    fn values(&self) -> Values {
        no_values()
    }
    fn process(&mut self, io: &mut Io) {
        let mut changed = false;
        for port in 0..2 {
            for it in io.items::<MotItem>(port) {
                let o = &it.value.object;
                if o.header.content_type != 7 {
                    continue;
                }
                let body = if o.header.compression == Some(1) {
                    // GZIP: skip its header and inflate the deflate stream.
                    gunzip(&o.body).unwrap_or_default().into()
                } else {
                    Arc::clone(&o.body)
                };
                if let Ok(root) = spi::decode(&body) {
                    self.guide.add(&root);
                    changed = true;
                }
            }
        }
        if changed {
            io.items_out::<SpiItem>(0).push(Timed {
                time: 0.0,
                value: SpiItem(Arc::new(self.guide.clone())),
            });
        }
    }
}

/// Inflates a GZIP member (RFC 1952).
pub fn gunzip(b: &[u8]) -> Option<Vec<u8>> {
    if b.len() < 18 || b[0] != 0x1F || b[1] != 0x8B || b[2] != 8 {
        return None;
    }
    let flags = b[3];
    let mut i = 10;
    if flags & 4 != 0 {
        let x = u16::from_le_bytes([b[i], b[i + 1]]) as usize;
        i += 2 + x;
    }
    for f in [8u8, 16] {
        if flags & f != 0 {
            while *b.get(i)? != 0 {
                i += 1;
            }
            i += 1;
        }
    }
    if flags & 2 != 0 {
        i += 2;
    }
    miniz_oxide::inflate::decompress_to_vec(b.get(i..b.len() - 8)?).ok()
}

/// Transmitters for the report.
#[derive(Clone, Debug, Default)]
pub struct TiiItem {
    /// The transmitters.
    pub transmitters: Vec<Transmitter>,
    /// Null symbols analysed.
    pub analysed: u32,
}
impl Item for TiiItem {
    const KIND: &'static str = "dab.tii";
}

/// The transmitter identification component.
#[derive(Default)]
pub struct TiiComponent {
    dec: TiiDecoder,
    db: Arc<Ensemble>,
}

impl Component for TiiComponent {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "dab.tii",
            vec![
                Port::typed::<NullItem>("in"),
                Port::typed::<EnsembleItem>("ens"),
            ],
            vec![item_out::<TiiItem>("out")],
        )
    }
    fn configure(&mut self, _: &[Format]) -> Result<Vec<Format>, ConfigError> {
        *self = TiiComponent::default();
        Ok(vec![typed::<TiiItem>()])
    }
    fn values(&self) -> Values {
        no_values()
    }
    fn process(&mut self, io: &mut Io) {
        for e in io.items::<EnsembleItem>(1) {
            if e.value.changed {
                self.db = e.value.db.clone();
            }
        }
        for it in io.items::<NullItem>(0) {
            if self.dec.push(&it.value.0) {
                let mut t = self.dec.found.clone();
                for tx in t.iter_mut() {
                    if let Some(p) = self
                        .db
                        .tii
                        .iter()
                        .find(|p| p.main == tx.main && p.sub == tx.sub)
                    {
                        tx.position = Some((p.lat, p.lon));
                        tx.delay_us = Some(p.delay_us);
                    }
                }
                io.items_out::<TiiItem>(0).push(Timed {
                    time: 0.0,
                    value: TiiItem {
                        transmitters: t,
                        analysed: self.dec.analysed,
                    },
                });
            }
        }
    }
}

/// DMB status for the report.
#[derive(Clone, Debug, Default)]
pub struct DmbItem(pub DmbStatus);
impl Item for DmbItem {
    const KIND: &'static str = "dab.dmb";
}

/// The DMB video component: a DMB stream sub-channel in, H.264 and audio access units out as media.
#[derive(Default)]
pub struct DmbComponent {
    decoders: BTreeMap<u8, DmbDecoder>,
    formats: BTreeMap<u16, Arc<MediaFormat>>,
    aus: Vec<AccessUnit>,
}

impl Component for DmbComponent {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "dab.dmb",
            vec![Port::typed::<MscItem>("in")],
            vec![
                item_out::<MediaPacket>("media"),
                item_out::<DmbItem>("status"),
            ],
        )
    }
    fn configure(&mut self, _: &[Format]) -> Result<Vec<Format>, ConfigError> {
        *self = DmbComponent::default();
        Ok(vec![typed::<MediaPacket>(), typed::<DmbItem>()])
    }
    fn values(&self) -> Values {
        no_values()
    }
    fn process(&mut self, io: &mut Io) {
        let mut any = None;
        for it in io.items::<MscItem>(0) {
            match &it.value {
                MscItem::Start { subch, content, .. } => {
                    if *content == Content::Stream(24) {
                        self.decoders.insert(*subch, DmbDecoder::new());
                        self.formats.clear();
                    } else {
                        self.decoders.remove(subch);
                    }
                }
                MscItem::Frame(f) => {
                    let Some(d) = self.decoders.get_mut(&f.subch) else {
                        continue;
                    };
                    d.push(&f.data, &mut self.aus);
                    any = Some(f.subch);
                    for au in self.aus.drain(..) {
                        let Some(es) = d.streams().get(&au.es_id) else {
                            continue;
                        };
                        let fmt = self
                            .formats
                            .entry(au.es_id)
                            .or_insert_with(|| Arc::new(dmb_format(es)))
                            .clone();
                        // An H.264 access point is clean only at an IDR picture.
                        let clean = fmt.codec != Codec::H264
                            || sdrkit_proto_mpegts::codec::h264_nals(&au.data)
                                .iter()
                                .any(|&(t, _, _)| t == 5);
                        let packet = Packet::new(au.data.into(), f.time)
                            .stamped(au.cts_90k, au.cts_90k, au.rap)
                            .clean_if(clean);
                        io.items_out::<MediaPacket>(0).push(Timed {
                            time: f.time,
                            value: MediaPacket::new(packet, fmt),
                        });
                    }
                }
            }
        }
        if let Some(sc) = any {
            if let Some(d) = self.decoders.get(&sc) {
                io.items_out::<DmbItem>(1).push(Timed {
                    time: 0.0,
                    value: DmbItem(d.status.clone()),
                });
            }
        }
    }
}

/// The media format of a DMB elementary stream: H.264 in Annex B form, or MPEG-4 audio with its AudioSpecificConfig.
///
/// Standard: TS 102 428 clause 5.
pub fn dmb_format(es: &crate::data::dmb::EsInfo) -> MediaFormat {
    match (es.oti, es.stream_type) {
        (0x21, _) | (_, 4) => {
            let mut f = MediaFormat::new(Codec::H264, Packaging::AnnexB);
            f.video = Some(VideoProps::default());
            f.config = es.dsi.clone().into();
            f
        }
        _ => {
            let asc = AudioSpecificConfig::parse(&es.dsi).ok();
            let codec = match asc.map(|a| (a.object_type, a.ps)) {
                Some((5, _)) => Codec::HeAac,
                Some((29, _)) | Some((_, true)) => Codec::HeAacV2,
                _ => Codec::Aac,
            };
            let mut f = MediaFormat::new(codec, Packaging::Raw);
            f.config = es.dsi.clone().into();
            f.audio = Some(AudioProps {
                sample_rate: asc.map_or(48_000, |a| a.sample_rate),
                channels: asc.map_or(2, |a| a.channels.max(1)),
            });
            f
        }
    }
}
