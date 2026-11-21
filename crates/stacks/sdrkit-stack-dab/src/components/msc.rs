//! The sub-channel decoder component (`dab.msc`): decodes the sub-channel of the service selected (and a data sub-channel
//! when requested), follows reconfigurations, and fills gaps with erased CIFs. Until a service is selected with the
//! command `service.select`, it plays the first service with audio.
//!
//! Standard: EN 300 401 clauses 5.3 and 6.

use super::items::{CifSoft, EnsembleItem, MscItem, MscStatus};
use crate::ensemble::Ensemble;
use crate::msc::{CifCounter, Content, Gap, Geometry, SubchannelDecoder};
use crate::settings::{DATA_SUBCHANNEL, EARLY_START};
use sdrkit_core::Values;
use sdrkit_core::{
    CommandError, CommandId, CommandSet, Commands, ConfigError, Record, SettingError, SettingId,
    SettingSet, Value,
};
use sdrkit_graph::{
    values_of, Applied, Component, ComponentInfo, Format, Io, Kind, Port, RateFrom, Timed,
};
use sdrkit_proto_mpegts::publish::{SERVICE_ROW, SERVICE_SELECT_KIND};
use std::collections::VecDeque;
use std::sync::Arc;

sdrkit_core::settings! {
    /// The settings of [`MscComponent`].
    pub enum MscSetting {
        /// The data sub-channel.
        DataSubchannel = DATA_SUBCHANNEL,
        /// The early start of DAB+.
        EarlyStart = EARLY_START,
    }
}

sdrkit_core::commands! {
    /// The commands of [`MscComponent`].
    pub enum MscCommand {
        /// Plays the service whose `services` row key (its SId in hexadecimal) is the argument `row`. An empty row or "0"
        /// plays the first service with audio.
        ServiceSelect = SERVICE_SELECT_KIND.command,
    }
}

/// What the listener requested to play.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum Wanted {
    /// Nothing.
    Nothing,
    /// The first service with audio.
    #[default]
    First,
    /// The service with this SId.
    Service(u32),
}

/// The sub-channel that plays service `sid`: of its audio component, else of its video component, else of its first
/// component in a sub-channel.
pub fn playable_subchannel(db: &Ensemble, sid: u32) -> Option<u8> {
    let s = db.services.get(&sid)?;
    s.audio()
        .or_else(|| s.video())
        .or_else(|| s.components.iter().find(|c| c.subch.is_some()))
        .and_then(|c| c.subch)
}

/// The component.
pub struct MscComponent {
    wanted: Wanted,
    data_subch: i32,
    early_start: bool,
    db: Arc<Ensemble>,
    infos: VecDeque<(u64, i64, Option<u16>)>,
    counter: CifCounter,
    decoders: Vec<(SubchannelDecoder, bool)>,
    skip_frame: Option<u64>,
    status: MscStatus,
    last_status_frame: u64,
    erased_total: u64,
    resync: bool,
}

impl Default for MscComponent {
    fn default() -> Self {
        MscComponent {
            wanted: Wanted::First,
            data_subch: -1,
            early_start: true,
            db: Arc::new(Ensemble::default()),
            infos: VecDeque::new(),
            counter: CifCounter::default(),
            decoders: Vec::new(),
            skip_frame: None,
            status: MscStatus::default(),
            last_status_frame: u64::MAX,
            erased_total: 0,
            resync: false,
        }
    }
}

/// What a sub-channel carries according to the database (and whether its service is scrambled).
pub fn content_of(db: &Ensemble, subch: u8) -> Option<(Content, bool)> {
    let s = db.service_of_subch(subch)?;
    let c = s.components.iter().find(|c| c.subch == Some(subch))?;
    let content = match (c.tmid, c.ty) {
        (0, 63) => Content::DabPlus,
        (0, _) => Content::Mp2,
        (1, d) => Content::Stream(d),
        _ => Content::Packet,
    };
    Some((content, c.ca || (s.caid != 0 && c.tmid == 0)))
}

impl MscComponent {
    /// The SId of the service to play, once the ensemble names it.
    fn service(&self) -> Option<u32> {
        match self.wanted {
            Wanted::Nothing => None,
            Wanted::Service(sid) => Some(sid),
            Wanted::First => self
                .db
                .services
                .values()
                .find(|s| s.audio().is_some() && s.audio().and_then(|c| c.subch).is_some())
                .map(|s| s.sid),
        }
    }

    /// The sub-channel to play, and the data sub-channel (each `None` for none).
    fn wanted(&self) -> [(Option<u8>, bool); 2] {
        let main = self
            .service()
            .and_then(|sid| playable_subchannel(&self.db, sid));
        let data = u8::try_from(self.data_subch)
            .ok()
            .filter(|&d| Some(d) != main);
        [(main, true), (data, false)]
    }

    fn sync_decoders(&mut self, out: &mut Vec<Timed<MscItem>>, time: f64) {
        let wanted = self.wanted();
        self.decoders
            .retain(|(d, _)| wanted.iter().any(|w| w.0 == Some(d.subch)));
        self.status.waiting = self.wanted != Wanted::Nothing && wanted[0].0.is_none();
        self.status.scrambled = false;
        for (subch, selected) in wanted {
            let Some(subch) = subch else {
                continue;
            };
            let Some(sc) = self.db.subchannels.get(&subch) else {
                if selected {
                    self.status.waiting = true;
                }
                continue;
            };
            let Some((content, scrambled)) = content_of(&self.db, subch) else {
                if selected {
                    self.status.waiting = true;
                }
                continue;
            };
            if selected && scrambled {
                self.status.scrambled = true;
                self.decoders.retain(|(d, _)| d.subch != subch);
                continue;
            }
            let Some(protection) = sc.protection else {
                continue;
            };
            let g = Geometry {
                start_cu: sc.start_cu,
                size_cu: sc.size_cu,
                protection,
                bitrate: sc.bitrate,
            };
            if let Some((d, _)) = self.decoders.iter().find(|(d, _)| d.subch == subch) {
                if d.geometry == g && d.content == content {
                    continue;
                }
            }
            let carried = self
                .decoders
                .iter()
                .find(|(d, _)| d.subch == subch)
                .map(|(d, _)| (d.frames, d.erased_cifs));
            self.decoders.retain(|(d, _)| d.subch != subch);
            if let Some(mut d) = SubchannelDecoder::new(subch, content, g, self.early_start) {
                if let Some((f, e)) = carried {
                    d.frames = f;
                    d.erased_cifs = e;
                }
                out.push(Timed {
                    time,
                    value: MscItem::Start {
                        subch,
                        content,
                        bitrate: g.bitrate,
                        selected,
                    },
                });
                self.decoders.push((d, selected));
            }
        }
    }
}

impl Component for MscComponent {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "dab.msc",
            vec![
                Port::typed::<CifSoft>("cif"),
                Port::typed::<EnsembleItem>("ens"),
            ],
            vec![
                Port::typed::<MscItem>("out").rate_from(RateFrom::Fixed(0.0)),
                Port::typed::<MscStatus>("status").rate_from(RateFrom::Fixed(0.0)),
            ],
        )
        .with_settings(MscSetting::schema())
    }

    fn configure(&mut self, _: &[Format]) -> Result<Vec<Format>, ConfigError> {
        let (s, d, e) = (self.wanted, self.data_subch, self.early_start);
        *self = MscComponent {
            wanted: s,
            data_subch: d,
            early_start: e,
            ..Default::default()
        };
        Ok(vec![
            Format::new(Kind::typed::<MscItem>(), 0.0),
            Format::new(Kind::typed::<MscStatus>(), 0.0),
        ])
    }

    fn process(&mut self, io: &mut Io) {
        let meter = io.meter();
        let mut out = std::mem::take(io.items_out::<MscItem>(0));
        for e in io.items::<EnsembleItem>(1) {
            if e.value.new_ensemble {
                self.decoders.clear();
            }
            if e.value.changed || !Arc::ptr_eq(&self.db, &e.value.db) {
                self.db = e.value.db.clone();
                self.sync_decoders(&mut out, e.time);
            }
            self.infos
                .push_back((e.value.frame, e.value.start, e.value.cif_count));
            while self.infos.len() > 16 {
                self.infos.pop_front();
            }
        }
        if std::mem::take(&mut self.resync) {
            let t = io.items::<CifSoft>(0).first().map_or(0.0, |c| c.time);
            self.sync_decoders(&mut out, t);
        }
        let mut status_due = None;
        for c in io.items::<CifSoft>(0) {
            let v = &c.value;
            if v.index == 0 {
                let info = self.infos.iter().find(|i| i.0 == v.frame).copied();
                let count = info.and_then(|i| i.2);
                match self.counter.frame(v.start, count) {
                    Gap::Repeat => {
                        self.skip_frame = Some(v.frame);
                    }
                    Gap::Lost(n) => {
                        self.skip_frame = None;
                        if n > 20 {
                            let decs = std::mem::take(&mut self.decoders);
                            for (mut d, sel) in decs {
                                d.restart();
                                out.push(Timed {
                                    time: c.time,
                                    value: MscItem::Start {
                                        subch: d.subch,
                                        content: d.content,
                                        bitrate: d.geometry.bitrate,
                                        selected: sel,
                                    },
                                });
                                self.decoders.push((d, sel));
                            }
                        } else {
                            self.erased_total += n as u64;
                            for (d, _) in self.decoders.iter_mut() {
                                for _ in 0..n {
                                    if let Some(f) = d.push_cif(None, c.time) {
                                        out.push(Timed {
                                            time: c.time,
                                            value: MscItem::Frame(f),
                                        });
                                    }
                                }
                            }
                        }
                    }
                    Gap::None => self.skip_frame = None,
                }
                status_due = Some(v.frame);
            }
            if self.skip_frame == Some(v.frame) {
                continue;
            }
            let bits = v.bits();
            let _viterbi = meter.section("deinterleave_viterbi");
            for (d, _) in self.decoders.iter_mut() {
                if let Some(f) = d.push_cif(Some(bits), c.time) {
                    out.push(Timed {
                        time: c.time,
                        value: MscItem::Frame(f),
                    });
                }
            }
        }
        *io.items_out::<MscItem>(0) = out;
        if let Some(f) = status_due {
            if f != self.last_status_frame {
                self.last_status_frame = f;
                let service = self.service();
                let s = &mut self.status;
                s.service = service;
                s.requested = self.wanted != Wanted::Nothing;
                s.decoding = self.decoders.iter().find(|d| d.1).map(|d| d.0.subch);
                s.logical_frames = self.decoders.iter().find(|d| d.1).map_or(0, |d| d.0.frames);
                s.erased_cifs = self.erased_total;
                s.data_subchannel = self.decoders.iter().find(|d| !d.1).map(|d| d.0.subch);
                let st = s.clone();
                io.items_out::<MscStatus>(1).push(Timed {
                    time: 0.0,
                    value: st,
                });
            }
        }
    }

    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        match MscSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            MscSetting::DataSubchannel => {
                let n = DATA_SUBCHANNEL.read(id, v)? as i32;
                if n != self.data_subch {
                    self.data_subch = n;
                    self.resync = true;
                }
            }
            MscSetting::EarlyStart => self.early_start = EARLY_START.read(id, v)?,
        }
        Ok(Applied::Live)
    }

    fn values(&self) -> Values {
        values_of([
            (
                MscSetting::DataSubchannel,
                Value::I64(i64::from(self.data_subch)),
            ),
            (MscSetting::EarlyStart, Value::Bool(self.early_start)),
        ])
    }

    fn commands(&self) -> &Commands {
        MscCommand::commands()
    }

    fn invoke(&mut self, id: CommandId, args: &Values) -> Result<Option<Record>, CommandError> {
        match MscCommand::from_id(id).ok_or(CommandError::NoSuchId { id })? {
            MscCommand::ServiceSelect => {
                let row = args.arg(&SERVICE_ROW)?;
                let wanted = if row.is_empty() || row == "0" {
                    Wanted::First
                } else {
                    let sid = u32::from_str_radix(row, 16).ok();
                    match sid {
                        Some(sid) if !self.db.valid() || self.db.services.contains_key(&sid) => {
                            Wanted::Service(sid)
                        }
                        _ => {
                            return Err(CommandError::NoSuchRow {
                                table: crate::results::services_table(),
                                key: row.to_string(),
                            });
                        }
                    }
                };
                if wanted != self.wanted {
                    self.wanted = wanted;
                    self.decoders.retain(|d| !d.1);
                    self.resync = true;
                }
            }
        }
        Ok(None)
    }
}
