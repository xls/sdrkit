//! The data components: the PAD decoder (`dab.pad`) and the MOT assembler (`dab.mot`).
//!
//! Standard: EN 300 401 clause 7.4, EN 301 234.

use super::items::{DataGroupItem, EnsembleItem, GroupSource, LabelItem, MotItem, PadEvent};
use crate::data::mot::{picture_codec, MotObject};
use crate::data::pad::PadDecoder;
use crate::ensemble::Ensemble;
use sdrkit_core::media::Codec;
use sdrkit_core::ConfigError;
use sdrkit_core::Values;
use sdrkit_graph::{no_values, Component, ComponentInfo, Format, Io, Kind, Port, RateFrom, Timed};
use sdrkit_proto_bcdata::datagroup;
use sdrkit_proto_bcdata::mot::Assembler;
use std::collections::BTreeMap;
use std::sync::Arc;

fn typed<T: sdrkit_graph::Item>() -> Format {
    Format::new(Kind::typed::<T>(), 0.0)
}

fn item_out<T: sdrkit_graph::Item>(name: &str) -> Port {
    Port::typed::<T>(name).rate_from(RateFrom::Fixed(0.0))
}

/// The PAD decoder component: the PAD of DAB+ and of Layer II in, labels and X-PAD data groups out.
#[derive(Default)]
pub struct PadComponent {
    dec: PadDecoder,
    db: Arc<Ensemble>,
    subch: Option<u8>,
}

impl PadComponent {
    fn apps(&mut self) {
        let mut starts = vec![12u8];
        if let Some(s) = self.subch.and_then(|sc| self.db.service_of_subch(sc)) {
            for a in &s.user_apps {
                if let Some((t, dg, _)) = a.xpad {
                    if dg && t >= 4 && !starts.contains(&t) && t != 13 {
                        starts.push(t);
                    }
                }
            }
        }
        self.dec.dg_starts = starts;
    }
}

impl Component for PadComponent {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "dab.pad",
            vec![
                Port::typed::<PadEvent>("aac"),
                Port::typed::<PadEvent>("mp2"),
                Port::typed::<EnsembleItem>("ens"),
            ],
            vec![
                item_out::<LabelItem>("label"),
                item_out::<DataGroupItem>("datagroup"),
            ],
        )
    }

    fn configure(&mut self, _: &[Format]) -> Result<Vec<Format>, ConfigError> {
        *self = PadComponent {
            dec: PadDecoder::new(),
            ..Default::default()
        };
        Ok(vec![typed::<LabelItem>(), typed::<DataGroupItem>()])
    }

    fn values(&self) -> Values {
        no_values()
    }

    fn process(&mut self, io: &mut Io) {
        for e in io.items::<EnsembleItem>(2) {
            if e.value.changed {
                self.db = e.value.db.clone();
                self.apps();
            }
        }
        for port in 0..2 {
            for it in io.items::<PadEvent>(port) {
                match &it.value {
                    PadEvent::Reset(sc) => {
                        self.dec.reset();
                        self.subch = Some(*sc);
                        self.apps();
                        io.items_out::<LabelItem>(0).push(Timed {
                            time: it.time,
                            value: LabelItem {
                                label: self.dec.label.clone(),
                                time: it.time,
                            },
                        });
                        io.items_out::<DataGroupItem>(1).push(Timed {
                            time: it.time,
                            value: DataGroupItem {
                                source: GroupSource::Reset,
                                data: Arc::from(&[][..]),
                                time: it.time,
                            },
                        });
                    }
                    PadEvent::Pad(p) => {
                        self.dec.push(p);
                        if self.dec.label_changed {
                            io.items_out::<LabelItem>(0).push(Timed {
                                time: it.time,
                                value: LabelItem {
                                    label: self.dec.label.clone(),
                                    time: it.time,
                                },
                            });
                        }
                        for g in self.dec.groups.drain(..) {
                            io.items_out::<DataGroupItem>(1).push(Timed {
                                time: it.time,
                                value: DataGroupItem {
                                    source: GroupSource::Xpad(g.app_type),
                                    data: g.data.into(),
                                    time: it.time,
                                },
                            });
                        }
                    }
                }
            }
        }
    }
}

/// Counters of the slideshow.
#[derive(Clone, Copy, Debug, Default)]
pub struct SlideStats {
    /// New pictures of the selected service.
    pub pictures: u64,
    /// Data groups with a bad CRC.
    pub bad_groups: u64,
}

/// The MOT assembler component: data groups in, MOT objects out. A picture of the service playing gets a new slide number.
#[derive(Default)]
pub struct MotComponent {
    assemblers: BTreeMap<(u8, u8, u16), Assembler>,
    sequence: u64,
    /// Counters.
    pub stats: SlideStats,
}

impl Component for MotComponent {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "dab.mot",
            vec![Port::typed::<DataGroupItem>("in")],
            vec![item_out::<MotItem>("object")],
        )
    }

    fn configure(&mut self, _: &[Format]) -> Result<Vec<Format>, ConfigError> {
        let seq = self.sequence;
        *self = MotComponent {
            sequence: seq,
            ..Default::default()
        };
        Ok(vec![typed::<MotItem>()])
    }

    fn values(&self) -> Values {
        no_values()
    }

    fn process(&mut self, io: &mut Io) {
        for it in io.items::<DataGroupItem>(0) {
            let v = &it.value;
            let key = match v.source {
                GroupSource::Reset => {
                    self.assemblers.retain(|k, _| k.0 != 0);
                    continue;
                }
                GroupSource::Xpad(t) => (0, t, 0),
                GroupSource::Packet(sc, addr) => (1, sc, addr),
            };
            let g = match datagroup::parse(&v.data) {
                Ok(g) => g,
                Err(_) => {
                    self.stats.bad_groups += 1;
                    continue;
                }
            };
            let asm = self.assemblers.entry(key).or_default();
            let mut done = None;
            asm.push(&g, &mut |o| done = Some(MotObject::of(&o)));
            let Some(obj) = done else { continue };
            let codec = picture_codec(&obj.header);
            let logo = obj.header.category_title.to_lowercase().contains("logo");
            let mut sequence = 0;
            if codec.is_some() && key.0 == 0 && !logo {
                self.sequence += 1;
                sequence = self.sequence;
                self.stats.pictures += 1;
            }
            io.items_out::<MotItem>(0).push(Timed {
                time: v.time,
                value: MotItem {
                    source: v.source,
                    object: Arc::new(obj),
                    codec,
                    logo,
                    sequence,
                    time: v.time,
                },
            });
        }
    }
}

/// The size of a PNG, JPEG or GIF picture from its header, when it can be read.
pub fn picture_size(codec: Codec, b: &[u8]) -> Option<(u32, u32)> {
    match codec {
        Codec::Png if b.len() >= 24 && &b[1..4] == b"PNG" => Some((
            u32::from_be_bytes([b[16], b[17], b[18], b[19]]),
            u32::from_be_bytes([b[20], b[21], b[22], b[23]]),
        )),
        Codec::Gif if b.len() >= 10 => Some((
            u16::from_le_bytes([b[6], b[7]]) as u32,
            u16::from_le_bytes([b[8], b[9]]) as u32,
        )),
        Codec::Jpeg => {
            let mut i = 2;
            while i + 9 < b.len() {
                if b[i] != 0xFF {
                    return None;
                }
                let m = b[i + 1];
                let len = u16::from_be_bytes([b[i + 2], b[i + 3]]) as usize;
                if (0xC0..=0xCF).contains(&m) && m != 0xC4 && m != 0xC8 && m != 0xCC {
                    return Some((
                        u16::from_be_bytes([b[i + 7], b[i + 8]]) as u32,
                        u16::from_be_bytes([b[i + 5], b[i + 6]]) as u32,
                    ));
                }
                i += 2 + len;
            }
            None
        }
        _ => None,
    }
}
