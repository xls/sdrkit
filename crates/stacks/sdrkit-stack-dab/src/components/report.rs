//! The report component (`dab.report`): gathers the status of every component into the stack's results (fields, tables,
//! series, status lock state), the events, and the slides of the service playing on the picture stream.

use super::data::picture_size;
use super::items::*;
use super::services::{DmbItem, JournalineItem, PacketStatus, SpiItem, TiiItem};
use crate::announce::{self, Applies};
use crate::channels::{self, Channel};
use crate::data::dmb::DmbStatus;
use crate::data::tii::Transmitter;
use crate::ensemble::Ensemble;
use crate::follow::{self, Alternative};
use crate::health::{self, Signals};
use crate::ofdm::{self, OfdmStats, SyncState};
use crate::registry;
use crate::results::{
    service_key, store, AudioCodec, Detection, JournalineKind, Playing, ServiceKind,
};
use crate::settings::{ANNOUNCEMENT_TYPES, CHANNELS};
use crate::typed::*;
use sdrkit_core::interface::{Hertz, Seconds, Time};
use sdrkit_core::media::{Codec, Picture};
use sdrkit_core::publish::keys::{Family, Urgency};
use sdrkit_core::publish::{Published, TableDef};
use sdrkit_core::Values;
use sdrkit_core::{
    Choice, ConfigError, Event, LockState, Results, SettingError, SettingId, SettingSet, Value,
};
use sdrkit_graph::{
    values_of, Applied, Component, ComponentInfo, Format, Io, Kept, Kind, Port, RateFrom, Timed,
};
use sdrkit_proto_bcdata::journaline::Kind as JmlKind;
use sdrkit_proto_bcdata::mot::Header;
use sdrkit_proto_bcdata::publish::{
    self as bc, GuideGroupRow, GuideProgrammeRow, GuideServiceRow, LogoRow, Publisher,
};
use sdrkit_proto_bcdata::slideshow::{Categories, PictureFormat, Slide};
use std::fmt::Write as _;
use std::sync::Arc;

sdrkit_core::settings! {
    /// The settings of [`ReportComponent`].
    pub enum ReportSetting {
        /// The channels of service following.
        Channels = CHANNELS,
        /// The announcement types that apply.
        AnnouncementTypes = ANNOUNCEMENT_TYPES,
    }
}

/// When things were first reported (signal seconds, 0 for not yet).
#[derive(Clone, Copy, Debug, Default)]
struct Timing {
    lock_s: f64,
    first_fib_s: f64,
    ensemble_valid_s: f64,
    all_labels_s: f64,
    first_access_unit_s: f64,
    label_s: f64,
    slide_s: f64,
}

fn first(t: &mut f64, now: f64) {
    if *t == 0.0 {
        *t = now.max(1e-6);
    }
}

/// A time of [`Timing`] as a field: no value before it happened.
fn at(t: f64) -> Option<Seconds> {
    (t > 0.0).then_some(Seconds(t))
}

/// The slide shown: its number, its size in pixels and in bytes.
#[derive(Clone, Copy, Debug)]
struct Shown {
    sequence: u64,
    size: (u32, u32),
    bytes: usize,
}

/// The report component.
pub struct ReportComponent {
    p: Published,
    bc: Publisher,
    categories: Categories,
    channels: Vec<Channel>,
    channels_text: String,
    announcement_types: u16,
    ofdm: OfdmStats,
    front: FrontStatus,
    db: Arc<Ensemble>,
    fic: Arc<FicStats>,
    msc: MscStatus,
    aac: AudioStatus,
    mp2: AudioStatus,
    label: Option<LabelItem>,
    utc: (i64, u16),
    timing: Timing,
    slide: Option<Shown>,
    dmb: Option<DmbStatus>,
    spi_objects: Option<i64>,
    journaline_received: Option<i64>,
    tii_analysed: Option<i64>,
    shown_db: Option<Arc<Ensemble>>,
    shown_playing: Option<(u32, u64)>,
    shown_types: u16,
    shown_decoding: Option<u8>,
    services_version: u64,
    last_valid: bool,
    last_active: [u64; 4],
    last_state: SyncState,
    applies: Option<Applies>,
    protection: Option<String>,
    following: Vec<Alternative>,
    events: Vec<Event>,
    summary: String,
    key: String,
    gone: Vec<String>,
    tuned_hz: f64,
}

impl Default for ReportComponent {
    fn default() -> Self {
        ReportComponent {
            p: store(),
            bc: Publisher::new(),
            categories: Categories::default(),
            channels: channels::blocks(),
            channels_text: String::new(),
            announcement_types: ANNOUNCEMENT_TYPES.default as u16,
            ofdm: OfdmStats::default(),
            front: FrontStatus::default(),
            db: Arc::new(Ensemble::default()),
            fic: Arc::new(FicStats::default()),
            msc: MscStatus::default(),
            aac: AudioStatus::default(),
            mp2: AudioStatus::default(),
            label: None,
            utc: (0, 0),
            timing: Timing::default(),
            slide: None,
            dmb: None,
            spi_objects: None,
            journaline_received: None,
            tii_analysed: None,
            shown_db: None,
            shown_playing: None,
            shown_types: 0,
            shown_decoding: None,
            services_version: 0,
            last_valid: false,
            last_active: [0; 4],
            last_state: SyncState::Searching,
            applies: None,
            protection: None,
            following: Vec::new(),
            events: Vec::new(),
            summary: String::new(),
            key: String::new(),
            gone: Vec::new(),
            tuned_hz: 0.0,
        }
    }
}

/// Removes the rows of `t` whose key `keep` refuses. `gone` is scratch space.
fn retain_rows(
    p: &mut Published,
    t: &TableDef,
    gone: &mut Vec<String>,
    mut keep: impl FnMut(&str) -> bool,
) {
    gone.clear();
    let r = p.results();
    if let Some(table) = r.decl().table_id(t.key).and_then(|id| r.table(id)) {
        gone.extend(
            table
                .rows_newest_first()
                .filter(|(k, _)| !keep(k))
                .map(|(k, _)| k.to_string()),
        );
    }
    for k in gone.iter() {
        p.remove_row(t, k);
    }
}

/// The parts of a key `<a>/<b>/...` read as numbers in base `radix`.
fn parts(key: &str, radix: u32) -> impl Iterator<Item = Option<u32>> + '_ {
    key.split('/')
        .map(move |x| u32::from_str_radix(x, radix).ok())
}

/// The protection of sub-channel `id` of `db` as the standard writes it ("EEP 3-A", "UEP 12").
fn protection(db: &Ensemble, id: u8) -> Option<String> {
    db.subchannels.get(&id).map(|s| s.protection_text())
}

impl ReportComponent {
    /// Starts again with the settings and the frequency of now. The guide, the logos and the count of guide objects
    /// stay.
    fn restart(&mut self) {
        let old = std::mem::replace(
            self,
            ReportComponent {
                channels: self.channels.clone(),
                channels_text: self.channels_text.clone(),
                announcement_types: self.announcement_types,
                tuned_hz: self.tuned_hz,
                ..Default::default()
            },
        );
        self.keep_guide(&old.p);
        self.bc = old.bc;
        self.spi_objects = old.spi_objects;
        let data = self.data_fields();
        self.p.put(&data);
        self.p
            .results_mut()
            .set_status(&sdrkit_core::Status::new(LockState::Searching).with_summary("Searching…"));
    }

    /// The counts of the data decoders.
    fn data_fields(&self) -> DataFields {
        DataFields {
            fic_records: self.fic.records as i64,
            spi_objects: self.spi_objects,
            journaline_received: self.journaline_received,
            tii_analysed: self.tii_analysed,
        }
    }

    /// Writes the guide and the logos of `old` again, oldest row first.
    fn keep_guide(&mut self, old: &Published) {
        let r = old.results();
        let p = &mut self.p;
        let rows: Vec<_> = bc::GUIDE_SERVICE_TABLE.rows::<GuideServiceRow>(r).collect();
        for (k, row) in rows.iter().rev() {
            bc::GUIDE_SERVICE_TABLE.put(p, k, row);
        }
        let rows: Vec<_> = bc::GUIDE_PROGRAMME_TABLE
            .rows::<GuideProgrammeRow>(r)
            .collect();
        for (k, row) in rows.iter().rev() {
            bc::GUIDE_PROGRAMME_TABLE.put(p, k, row);
        }
        let rows: Vec<_> = bc::GUIDE_GROUP_TABLE.rows::<GuideGroupRow>(r).collect();
        for (k, row) in rows.iter().rev() {
            bc::GUIDE_GROUP_TABLE.put(p, k, row);
        }
        let rows: Vec<_> = bc::LOGO_TABLE.rows::<LogoRow>(r).collect();
        for (k, row) in rows.iter().rev() {
            bc::LOGO_TABLE.put(p, k, row);
        }
    }

    fn bit(set: &[u64; 4], c: u8) -> bool {
        set[usize::from(c >> 6)] & (1 << (c & 63)) != 0
    }

    fn playing_service(&self) -> Option<&crate::ensemble::Service> {
        self.msc.service.and_then(|sid| self.db.services.get(&sid))
    }

    /// The tables of the ensemble: services, sub-channels, components, user applications, clusters and the transmitter
    /// database.
    fn write_ensemble(&mut self) {
        let db = Arc::clone(&self.db);
        let playing = self.msc.service;
        let now = self
            .label
            .as_ref()
            .map(|l| l.label.text.as_str())
            .unwrap_or("");
        let table = if db.table == 2 { 2 } else { 1 };
        let key = &mut self.key;
        let p = &mut self.p;
        for s in db.services.values() {
            service_key(s.sid, key);
            let comp = s.audio().or_else(|| s.components.first());
            let sc = comp.and_then(|c| c.subch);
            let sub = sc.and_then(|x| db.subchannels.get(&x));
            let plays = playing == Some(s.sid);
            let protection = sub.map(|x| x.protection_text());
            let row = ServiceRow {
                sid: i64::from(s.sid),
                label: (!s.label.is_empty()).then_some(s.label.as_str()),
                short_label: (!s.short_label.is_empty()).then_some(s.short_label.as_str()),
                kind: s.kind(),
                subchannel: sc.map(i64::from),
                bitrate_kbps: sub.map(|x| i64::from(x.bitrate)),
                protection: protection.as_deref(),
                component_type: comp.map(|c| i64::from(c.ty)),
                pty: s.pty.map(i64::from),
                pty_name: s
                    .pty
                    .map(|p| registry::programme_type(p, table))
                    .filter(|n| !n.is_empty()),
                pty_static: !s.pty_dynamic,
                language: s.language.map(i64::from),
                language_name: s.language.map(registry::language).filter(|n| !n.is_empty()),
                scrambled: s.scrambled(),
                announcement_support: i64::from(s.asu),
                now_playing: (plays && !now.is_empty()).then_some(now),
                playing: plays,
            };
            SERVICES.put(p, key, &row);
            for (n, c) in s.components.iter().enumerate() {
                key.clear();
                let _ = write!(key, "{}/{n}", ServiceKey(s.sid));
                let row = ServiceComponentRow {
                    service: key.split('/').next().unwrap_or(""),
                    tmid: i64::from(c.tmid),
                    component_type: i64::from(c.ty),
                    subchannel: c.subch.map(i64::from),
                    packet_address: c.packet_address.map(i64::from),
                    scids: c.scids.map(i64::from),
                    primary: c.primary,
                    scrambled: c.ca,
                    label: c
                        .scids
                        .and_then(|x| s.component_labels.get(&x))
                        .map(String::as_str),
                };
                SERVICE_COMPONENTS.put(p, key, &row);
            }
            for a in &s.user_apps {
                key.clear();
                let _ = write!(key, "{}/{}/{}", ServiceKey(s.sid), a.scids, a.app);
                let row = UserApplicationRow {
                    service: key.split('/').next().unwrap_or(""),
                    scids: i64::from(a.scids),
                    app_type: i64::from(a.app),
                    name: a.name,
                };
                USER_APPLICATIONS.put(p, key, &row);
            }
            for &c in &s.clusters {
                key.clear();
                let _ = write!(key, "{}/{c}", ServiceKey(s.sid));
                let row = ServiceClusterRow {
                    service: key.split('/').next().unwrap_or(""),
                    cluster: i64::from(c),
                };
                SERVICE_CLUSTERS.put(p, key, &row);
            }
        }
        let gone = &mut self.gone;
        retain_rows(p, &SERVICES, gone, |k| {
            u32::from_str_radix(k, 16).is_ok_and(|sid| db.services.contains_key(&sid))
        });
        retain_rows(p, &SERVICE_COMPONENTS, gone, |k| {
            let mut x = parts(k, 16);
            match (
                x.next().flatten(),
                k.rsplit('/').next().and_then(|n| n.parse::<usize>().ok()),
            ) {
                (Some(sid), Some(n)) => db
                    .services
                    .get(&sid)
                    .is_some_and(|s| n < s.components.len()),
                _ => false,
            }
        });
        retain_rows(p, &USER_APPLICATIONS, gone, |k| {
            let mut x = k.split('/');
            let sid = x.next().and_then(|v| u32::from_str_radix(v, 16).ok());
            let scids = x.next().and_then(|v| v.parse::<u8>().ok());
            let app = x.next().and_then(|v| v.parse::<u16>().ok());
            match (sid, scids, app) {
                (Some(sid), Some(scids), Some(app)) => db
                    .services
                    .get(&sid)
                    .is_some_and(|s| s.user_apps.iter().any(|a| a.scids == scids && a.app == app)),
                _ => false,
            }
        });
        retain_rows(p, &SERVICE_CLUSTERS, gone, |k| {
            let mut x = k.split('/');
            let sid = x.next().and_then(|v| u32::from_str_radix(v, 16).ok());
            let c = x.next().and_then(|v| v.parse::<u8>().ok());
            match (sid, c) {
                (Some(sid), Some(c)) => db
                    .services
                    .get(&sid)
                    .is_some_and(|s| s.clusters.contains(&c)),
                _ => false,
            }
        });
        for s in db.subchannels.values() {
            key.clear();
            let _ = write!(key, "{}", s.id);
            let protection = protection(&db, s.id);
            let row = SubchannelRow {
                id: i64::from(s.id),
                start_cu: i64::from(s.start_cu),
                size_cu: i64::from(s.size_cu),
                protection: protection.as_deref(),
                bitrate_kbps: i64::from(s.bitrate),
            };
            SUBCHANNELS.put(p, key, &row);
        }
        retain_rows(p, &SUBCHANNELS, gone, |k| {
            k.parse::<u8>()
                .is_ok_and(|id| db.subchannels.contains_key(&id))
        });
        for t in &db.tii {
            key.clear();
            let _ = write!(key, "{}.{}", t.main, t.sub);
            let row = TiiEntryRow {
                main: i64::from(t.main),
                sub: i64::from(t.sub),
                position: (t.lat, t.lon),
                delay_us: i64::from(t.delay_us),
            };
            TII_DATABASE.put(p, key, &row);
        }
        retain_rows(p, &TII_DATABASE, gone, |k| {
            let mut x = k.split('.').map(|v| v.parse::<u8>().ok());
            let (m, s) = (x.next().flatten(), x.next().flatten());
            db.tii.iter().any(|t| Some(t.main) == m && Some(t.sub) == s)
        });
        for a in db.announcements.values() {
            key.clear();
            let _ = write!(key, "{}", a.cluster);
            let row = AnnouncementRow {
                cluster: i64::from(a.cluster),
                asw: i64::from(a.asw),
                subchannel: i64::from(a.subch),
            };
            ANNOUNCEMENTS.put(p, key, &row);
        }
        retain_rows(p, &ANNOUNCEMENTS, gone, |k| {
            k.parse::<u8>()
                .is_ok_and(|c| db.announcements.contains_key(&c))
        });
    }

    /// The service following and the announcement that applies to the service playing.
    fn write_following(&mut self) {
        let playing = self.msc.decoding;
        self.applies =
            playing.and_then(|p| announce::applicable(&self.db, p, self.announcement_types));
        self.following = self.msc.service.map_or(Vec::new(), |sid| {
            follow::alternatives(&self.db, sid, &self.channels)
        });
        self.protection = self
            .msc
            .decoding
            .and_then(|c| self.db.subchannels.get(&c))
            .map(|s| s.protection_text());
        let p = &mut self.p;
        for a in &self.following {
            self.key.clear();
            let _ = write!(self.key, "{}", a.hz as i64);
            let row = FollowingRow {
                freq: Hertz(a.hz),
                channel: a.channel,
                eid: i64::from(a.eid),
            };
            FOLLOWING.put(p, &self.key, &row);
        }
        let following = &self.following;
        retain_rows(p, &FOLLOWING, &mut self.gone, |k| {
            following
                .iter()
                .any(|a| k.parse::<i64>().is_ok_and(|h| h == a.hz as i64))
        });
    }

    fn write_transmitters(&mut self, tx: &[Transmitter], analysed: u32) {
        self.tii_analysed = Some(i64::from(analysed));
        let p = &mut self.p;
        for t in tx {
            self.key.clear();
            let _ = write!(self.key, "{}.{}", t.main, t.sub);
            let row = TransmitterRow {
                main: i64::from(t.main),
                sub: i64::from(t.sub),
                level_db: f64::from(t.level_db),
                position: t.position,
                delay_us: t.delay_us.map(i64::from),
            };
            TRANSMITTERS.put(p, &self.key, &row);
        }
        retain_rows(p, &TRANSMITTERS, &mut self.gone, |k| {
            let mut x = k.split('.').map(|v| v.parse::<u8>().ok());
            let (m, s) = (x.next().flatten(), x.next().flatten());
            tx.iter().any(|t| Some(t.main) == m && Some(t.sub) == s)
        });
    }

    fn write_journaline(&mut self, store: &sdrkit_proto_bcdata::journaline::Store) {
        self.journaline_received = Some(store.received as i64);
        let p = &mut self.p;
        for o in store.objects.values() {
            self.key.clear();
            let _ = write!(self.key, "{:04X}", o.id);
            let row = JournalineRow {
                id: i64::from(o.id),
                kind: match o.kind {
                    JmlKind::Menu => JournalineKind::Menu,
                    JmlKind::Text => JournalineKind::Text,
                    JmlKind::Title => JournalineKind::Title,
                    JmlKind::List => JournalineKind::List,
                },
                title: &o.title,
                body: (!o.body.is_empty()).then_some(o.body.as_str()),
            };
            JOURNALINE.put(p, &self.key, &row);
            for (n, (target, text)) in o.links.iter().enumerate() {
                self.key.clear();
                let _ = write!(self.key, "{:04X}/{n}", o.id);
                let row = JournalineLinkRow {
                    object: &self.key[..4],
                    target: i64::from(*target),
                    text,
                };
                JOURNALINE_LINKS.put(p, &self.key, &row);
            }
            for (n, text) in o.items.iter().enumerate() {
                self.key.clear();
                let _ = write!(self.key, "{:04X}/{n}", o.id);
                let row = JournalineItemRow {
                    object: &self.key[..4],
                    text,
                };
                JOURNALINE_ITEMS.put(p, &self.key, &row);
            }
        }
        let has = |k: &str| {
            let mut x = k.split('/');
            let id = x.next().and_then(|v| u16::from_str_radix(v, 16).ok());
            let n = x.next().and_then(|v| v.parse::<usize>().ok());
            (id, n)
        };
        retain_rows(p, &JOURNALINE, &mut self.gone, |k| {
            has(k).0.is_some_and(|id| store.objects.contains_key(&id))
        });
        retain_rows(p, &JOURNALINE_LINKS, &mut self.gone, |k| match has(k) {
            (Some(id), Some(n)) => store.objects.get(&id).is_some_and(|o| n < o.links.len()),
            _ => false,
        });
        retain_rows(p, &JOURNALINE_ITEMS, &mut self.gone, |k| match has(k) {
            (Some(id), Some(n)) => store.objects.get(&id).is_some_and(|o| n < o.items.len()),
            _ => false,
        });
    }

    /// The DMB counts of the last report of the DMB decoder.
    fn dmb_fields(&self) -> DmbFields {
        let d = self.dmb.as_ref();
        let size = d.and_then(|d| d.video_size);
        DmbFields {
            dmb_ts_packets: d.map(|d| d.ts_packets as i64),
            dmb_rs_corrected: d.map(|d| d.rs_corrected as i64),
            dmb_rs_failed: d.map(|d| d.rs_failed as i64),
            dmb_video_units: d.map(|d| d.video_units as i64),
            dmb_audio_units: d.map(|d| d.audio_units as i64),
            dmb_video_width: size.map(|s| i64::from(s.0)),
            dmb_video_height: size.map(|s| i64::from(s.1)),
        }
    }

    fn write_fig_types(&mut self) {
        let p = &mut self.p;
        for &(t, e, count, bytes) in &self.fic.per_type {
            self.key.clear();
            let _ = write!(self.key, "{t}/{e}");
            let row = FigTypeRow {
                fig: &self.key,
                count: count as i64,
                bytes: bytes as i64,
            };
            FIG_TYPES.put(p, &self.key, &row);
        }
    }

    /// Clears what belongs to the service that played: the label, the slide and the timing of its audio.
    fn service_changed(&mut self) {
        self.label = None;
        self.bc.clear_label(&mut self.p);
        self.categories.clear();
        self.bc.categories(&mut self.p, &self.categories);
        self.slide = None;
        self.p.put(&bc::SlideFields {
            slide_name: None,
            slide_category: None,
            slide_link: None,
            slide_alert: false,
        });
        self.timing.first_access_unit_s = 0.0;
        self.timing.label_s = 0.0;
        self.timing.slide_s = 0.0;
        self.aac = AudioStatus::default();
        self.mp2 = AudioStatus::default();
    }

    fn slide(&mut self, m: &MotItem, out: &mut Vec<Timed<Picture>>) {
        let Some(codec) = m.codec else {
            return;
        };
        let o = &m.object;
        let name = o.header.content_name.as_str();
        let format = match codec {
            Codec::Jpeg => Some(PictureFormat::Jpeg),
            Codec::Png => Some(PictureFormat::Png),
            _ => None,
        };
        if m.logo {
            if let Some(f) = format {
                self.bc.logo(&mut self.p, name, f, Arc::clone(&o.body));
            }
            return;
        }
        if m.sequence == 0 {
            return;
        }
        first(&mut self.timing.slide_s, m.time);
        let slide = Header::parse(&o.header_bytes)
            .ok()
            .and_then(|h| Slide::from_header(&h));
        let picture = match slide {
            Some(s) => {
                self.categories.add(&s, name);
                self.bc.categories(&mut self.p, &self.categories);
                self.bc.slide(&mut self.p, &s, name, Arc::clone(&o.body))
            }
            None => {
                self.p.put(&bc::SlideFields {
                    slide_name: (!name.is_empty()).then_some(name),
                    slide_category: None,
                    slide_link: None,
                    slide_alert: false,
                });
                Picture {
                    codec,
                    layout: None,
                    name: (!name.is_empty()).then(|| name.to_string()),
                    data: Arc::clone(&o.body),
                }
            }
        };
        self.slide = Some(Shown {
            sequence: m.sequence,
            size: picture_size(codec, &o.body).unwrap_or((0, 0)),
            bytes: o.body.len(),
        });
        out.push(Timed {
            time: m.time,
            value: picture,
        });
        self.key.clear();
        let source = self.msc.service.map(ServiceKey);
        if let Some(s) = source {
            let _ = write!(self.key, "{s}");
        }
        self.events.push(SLIDE_SHOWN.event(
            m.time,
            self.tuned_hz,
            &SlideEvent {
                source: source.map(|_| self.key.as_str()),
                slide_sequence: m.sequence as i64,
                text: (!name.is_empty()).then_some(name),
                family: Family::Broadcast,
            },
        ));
    }

    fn label(&mut self, l: &LabelItem) {
        if !l.label.text.is_empty() {
            first(&mut self.timing.label_s, l.time);
            self.key.clear();
            let source = self.msc.service.map(ServiceKey);
            if let Some(s) = source {
                let _ = write!(self.key, "{s}");
            }
            self.events.push(LABEL_CHANGED.event(
                l.time,
                self.tuned_hz,
                &LabelEvent {
                    source: source.map(|_| self.key.as_str()),
                    text: &l.label.text,
                    family: Family::Broadcast,
                },
            ));
        }
        self.bc.label(&mut self.p, &l.label.text, &l.label.tags);
        self.label = Some(l.clone());
        self.shown_playing = None;
    }

    fn publish(&mut self, time: f64) {
        let o = self.ofdm.clone();
        let locked = o.state == SyncState::Locked;
        if o.lock_time_s > 0.0 {
            self.timing.lock_s = o.lock_time_s;
        }
        if self.fic.fib_ok > 0 {
            first(&mut self.timing.first_fib_s, time);
        }
        let db = Arc::clone(&self.db);
        if db.valid() {
            first(&mut self.timing.ensemble_valid_s, time);
        }
        if db.all_labelled() {
            first(&mut self.timing.all_labels_s, time);
        }
        let selected = self.playing_service().map(|s| (s.sid, s.kind()));
        let kind = selected.map(|s| s.1);
        let selected_label = selected
            .and_then(|s| db.services.get(&s.0))
            .map(|s| s.label.as_str())
            .filter(|l| !l.is_empty());
        let st = if kind == Some(ServiceKind::Dab) {
            self.mp2.clone()
        } else {
            self.aac.clone()
        };
        if st.first_unit_s > 0.0 {
            first(&mut self.timing.first_access_unit_s, st.first_unit_s);
        }
        let playing = if !self.msc.requested {
            Playing::Nothing
        } else if self.msc.scrambled {
            Playing::Scrambled
        } else if self.msc.waiting {
            Playing::Waiting
        } else if st.decoding {
            Playing::Decoding
        } else {
            Playing::NoAudio
        };
        let total_units = st.dabplus.access_units_ok + st.dabplus.access_units_bad;
        let concealed_pct =
            (total_units > 0).then(|| 100.0 * st.dabplus.concealed as f64 / total_units as f64);
        // The status lock state and line.
        let s = &mut self.summary;
        s.clear();
        let text = self
            .label
            .as_ref()
            .map(|l| l.label.text.as_str())
            .unwrap_or("");
        if !text.is_empty() {
            s.push_str(text);
        } else if let (Some((_, k)), Some(l), true) = (selected, selected_label, st.decoding) {
            let kbps = self
                .msc
                .decoding
                .and_then(|c| db.subchannels.get(&c))
                .map_or(0, |c| c.bitrate);
            let _ = write!(
                s,
                "{l} · {} {kbps} kbit/s",
                ServiceKind::CHOICES[usize::from(k.index())].label
            );
        } else if !locked {
            s.push_str("Searching…");
        } else {
            s.push_str(&db.label);
        }
        let lock_state = match o.state {
            SyncState::Locked => LockState::Locked,
            SyncState::Syncing => LockState::Tracking,
            SyncState::Searching => LockState::Searching,
        };
        let level = self.front.level_dbfs;
        let summary = &self.summary;
        self.p.results_mut().update_status(|x| {
            let snr_db = locked.then_some(o.snr_db);
            let same = x.lock_state == lock_state
                && x.snr_db == snr_db
                && x.level_dbfs == Some(level)
                && x.offset_hz == Some(o.cfo_hz)
                && x.summary == *summary;
            if !same {
                x.lock_state = lock_state;
                x.snr_db = snr_db;
                x.level_dbfs = Some(level);
                x.offset_hz = Some(o.cfo_hz);
                x.summary.clear();
                x.summary.push_str(summary);
            }
            !same
        });
        // The health indicators.
        let dp = &st.dabplus;
        let h = health::health(&Signals {
            locked,
            prs_peak: o.prs_peak,
            fib_ok: self.fic.fib_ok,
            fib_ok_last_frame: self.fic.fib_ok_last_frame,
            snr_db: o.snr_db,
            ensemble_valid: db.valid(),
            selected: self.msc.requested,
            mp2: st.codec == Some(AudioCodec::Mp2),
            mp2_frames: (st.mp2_frames, st.mp2_bad),
            superframes: (dp.superframes_ok, dp.superframes_bad),
            logical_frames: self.msc.logical_frames,
            decoding: st.decoding,
            concealed_pct: concealed_pct.unwrap_or(0.0),
        });
        let searching = o.state == SyncState::Searching;
        let p = &mut self.p;
        p.put(&SignalFields {
            snr_db: locked.then(|| (f64::from(o.snr_db) * 10.0).round() / 10.0),
            offset_hz: (!searching).then_some(Hertz(o.cfo_hz)),
            clock_ppm: (!searching).then_some(o.clock_ppm),
            level_dbfs: f64::from(level),
        });
        p.put(&EnsembleFields {
            ensemble_label: (!db.label.is_empty()).then_some(db.label.as_str()),
            ensemble_short_label: (!db.short_label.is_empty()).then_some(db.short_label.as_str()),
            eid: db.eid.map(i64::from),
            ecc: db.country.then_some(i64::from(db.ecc)),
            country: db.eid.map(|e| i64::from(e >> 12)),
            utc: (self.utc.0 > 0).then_some(Time(self.utc.0 as f64)),
            local_time_offset: db.country.then_some(Seconds(f64::from(db.lto) * 1800.0)),
            ensemble_valid: db.valid(),
            service_count: db.services.len() as i64,
        });
        p.put(&SyncFields {
            sync: o.sync,
            prs_peak: (!searching).then_some(f64::from(o.prs_peak)),
            frames: o.frames as i64,
            fib_ok: self.fic.fib_ok as i64,
            fib_bad: self.fic.fib_bad as i64,
            fib_ok_last_frame: i64::from(self.fic.fib_ok_last_frame),
            fic_fill_pct: self.fic.fic_fill_pct,
            quiet: Seconds(o.quiet_ms / 1000.0),
            sync_losses: o.sync_losses as i64,
            relocks: o.relocks as i64,
            erased_cifs: self.msc.erased_cifs as i64,
            impulses_blanked: self.front.impulses_blanked as i64,
            mirrored: o.mirrored,
            signal_time: Seconds(time),
            detection: (!searching).then_some(match o.detection {
                ofdm::Detection::Coherent => Detection::Coherent,
                ofdm::Detection::Differential => Detection::Differential,
            }),
        });
        // The audio of the service playing.
        let configured = st.codec.is_some();
        p.put(&AudioFields {
            audio_codec: st.codec,
            audio_rate: (st.rate > 0).then_some(Hertz(f64::from(st.rate))),
            audio_channels: (st.channels > 0).then_some(i64::from(st.channels)),
            audio_sbr: configured.then_some(st.sbr),
            audio_ps: configured.then_some(st.ps),
            audio_units_per_superframe: (st.units_per_superframe > 0)
                .then_some(st.units_per_superframe as i64),
            audio_logical_frames: self.msc.logical_frames as i64,
            audio_units_ok: dp.access_units_ok as i64,
            audio_units_bad: dp.access_units_bad as i64,
            audio_superframes_ok: dp.superframes_ok as i64,
            audio_superframes_bad: dp.superframes_bad as i64,
            audio_rs_corrected: dp.rs_bytes_corrected as i64,
            audio_rs_uncorrectable: dp.rs_uncorrectable as i64,
            audio_mp2_frames: st.mp2_frames as i64,
            audio_mp2_bad: st.mp2_bad as i64,
            audio_concealed_pct: concealed_pct,
        });
        let slide = self.slide;
        p.put(&PadFields {
            label_charset: self.label.as_ref().map(|l| i64::from(l.label.charset)),
            slide_sequence: slide.map(|s| s.sequence as i64),
            slide_width: slide.map(|s| i64::from(s.size.0)),
            slide_height: slide.map(|s| i64::from(s.size.1)),
            slide_bytes: slide.map(|s| s.bytes as i64),
        });
        let tm = self.timing;
        p.put(&TimingFields {
            timing_lock: at(tm.lock_s),
            timing_first_fib: at(tm.first_fib_s),
            timing_ensemble_valid: at(tm.ensemble_valid_s),
            timing_all_labels: at(tm.all_labels_s),
            timing_first_access_unit: at(tm.first_access_unit_s),
            timing_label: at(tm.label_s),
            timing_slide: at(tm.slide_s),
        });
        let data = self.data_fields();
        let p = &mut self.p;
        p.put(&data);
        p.put(&HealthFields {
            health_sync: h.sync,
            health_prs: h.prs,
            health_fic: h.fic,
            health_ensemble: h.ensemble,
            health_msc: h.msc,
            health_audio: h.audio,
            health_concealed: h.concealed,
            quality: locked.then_some(f64::from(h.quality)),
            quality_text: locked.then_some(h.quality_text),
        });
        // The scopes.
        let r = self.p.results_mut();
        if let (Some(c), Some(id)) = (&o.constellation, r.decl().series_id(CONSTELLATION.key)) {
            r.update_series(id, |s| {
                s.points.clear();
                s.points.extend(c.iter().map(|v| [v.re, v.im]));
                true
            });
        }
        if let (Some(imp), Some(id)) = (&o.impulse, r.decl().series_id(IMPULSE_RESPONSE.key)) {
            r.update_series(id, |s| {
                s.line = true;
                s.points.clear();
                s.points
                    .extend(imp.iter().enumerate().map(|(i, v)| [i as f32, *v]));
                true
            });
        }
        // The tables of the ensemble, and what depends on the service playing.
        let label_count = self.label.as_ref().map_or(0, |l| l.label.count);
        let now_playing = self.msc.service.map(|sid| (sid, label_count));
        let db_changed = !self.shown_db.as_ref().is_some_and(|d| Arc::ptr_eq(d, &db));
        if db_changed || now_playing != self.shown_playing {
            self.shown_db = Some(Arc::clone(&db));
            self.shown_playing = now_playing;
            self.write_ensemble();
        }
        if db_changed
            || self.shown_types != self.announcement_types
            || self.shown_decoding != self.msc.decoding
        {
            self.shown_types = self.announcement_types;
            self.shown_decoding = self.msc.decoding;
            self.write_following();
        }
        // The service playing.
        let sub = self.msc.decoding.and_then(|c| db.subchannels.get(&c));
        self.key.clear();
        if let Some((sid, _)) = selected {
            service_key(sid, &mut self.key);
        }
        self.p.put(&PlayingFields {
            selected_service: selected.map(|_| self.key.as_str()),
            selected_subchannel: self.msc.decoding.map(i64::from),
            selected_label,
            selected_kind: kind,
            selected_bitrate_kbps: sub.map(|s| i64::from(s.bitrate)),
            selected_protection: self.protection.as_deref(),
            playing,
        });
        let ap = self.applies;
        self.p.put(&AnnouncementFields {
            announcement_cluster: ap.map(|a| i64::from(a.cluster)),
            announcement_type: ap.map(|a| i64::from(a.ty)),
            announcement_name: ap.map(|a| registry::announcement_type(a.ty)),
            announcement_subchannel: ap.map(|a| i64::from(a.subch)),
        });
        let dmb = self.dmb_fields();
        self.p.put(&dmb);
        self.write_fig_types();
        // Events.
        let source = |key: &mut String| {
            key.clear();
            if let Some(e) = db.eid {
                let _ = write!(key, "{e:04X}");
            }
        };
        if db.version != self.services_version {
            self.services_version = db.version;
            source(&mut self.key);
            self.events.push(SERVICES_CHANGED.event(
                time,
                self.tuned_hz,
                &ServicesEvent {
                    source: db.eid.map(|_| self.key.as_str()),
                    count: db.services.len() as i64,
                    family: Family::Broadcast,
                },
            ));
        }
        if db.valid() && !self.last_valid {
            source(&mut self.key);
            self.events.push(ENSEMBLE_FOUND.event(
                time,
                self.tuned_hz,
                &EnsembleEvent {
                    source: db.eid.map(|_| self.key.as_str()),
                    text: &db.label,
                    family: Family::Broadcast,
                },
            ));
        }
        self.last_valid = db.valid();
        let mut active = [0u64; 4];
        for &c in db.announcements.keys() {
            active[usize::from(c >> 6)] |= 1 << (c & 63);
        }
        if active != self.last_active {
            source(&mut self.key);
            for a in db.announcements.values() {
                if Self::bit(&self.last_active, a.cluster) {
                    continue;
                }
                let alarm = a.asw & 1 != 0;
                let ty = (0..16u8).find(|b| a.asw & (1 << b) != 0).unwrap_or(0);
                self.events.push(ANNOUNCEMENT_STARTED.event(
                    time,
                    self.tuned_hz,
                    &AnnouncementStartedEvent {
                        source: db.eid.map(|_| self.key.as_str()),
                        cluster: i64::from(a.cluster),
                        asw: i64::from(a.asw),
                        subchannel: i64::from(a.subch),
                        text: registry::announcement_type(ty),
                        urgency: if alarm {
                            Urgency::Urgent
                        } else {
                            Urgency::Notice
                        },
                        family: Family::Broadcast,
                    },
                ));
            }
            for c in 0..=255u8 {
                if Self::bit(&self.last_active, c) && !Self::bit(&active, c) {
                    self.events.push(ANNOUNCEMENT_ENDED.event(
                        time,
                        self.tuned_hz,
                        &AnnouncementEndedEvent {
                            source: db.eid.map(|_| self.key.as_str()),
                            cluster: i64::from(c),
                            family: Family::Broadcast,
                        },
                    ));
                }
            }
            self.last_active = active;
        }
        if self.last_state == SyncState::Locked && !locked {
            self.events.push(SYNC_LOST.event(
                time,
                self.tuned_hz,
                &SyncLostEvent {
                    family: Family::Receiver,
                },
            ));
            self.p.results_mut().log(format!("{time:.2} s: sync lost"));
        }
        self.last_state = o.state;
    }
}

/// A service's row key, for `write!`.
#[derive(Clone, Copy)]
struct ServiceKey(u32);

impl std::fmt::Display for ServiceKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.0 > 0xFFFF {
            write!(f, "{:08X}", self.0)
        } else {
            write!(f, "{:04X}", self.0)
        }
    }
}

impl Component for ReportComponent {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "dab.report",
            vec![
                Port::typed::<OfdmStatusItem>("ofdm"),
                Port::typed::<FrontStatus>("front"),
                Port::typed::<EnsembleItem>("ens"),
                Port::typed::<MscStatus>("msc"),
                Port::typed::<AudioStatus>("aac"),
                Port::typed::<AudioStatus>("mp2"),
                Port::typed::<LabelItem>("label"),
                Port::typed::<MotItem>("mot"),
                Port::typed::<TiiItem>("tii"),
                Port::typed::<SpiItem>("spi"),
                Port::typed::<JournalineItem>("journaline"),
                Port::typed::<DmbItem>("dmb"),
                Port::typed::<PacketStatus>("packet"),
            ],
            vec![
                Port::new("events", Kind::Messages).rate_from(RateFrom::Fixed(0.0)),
                Port::typed::<Picture>("picture").rate_from(RateFrom::Fixed(0.0)),
            ],
        )
        .with_settings(ReportSetting::schema())
    }

    /// Starts again from nothing but the guide and the logos, which belong to no single ensemble.
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        self.restart();
        self.tuned_hz = inputs.first().map_or(0.0, |f| f.tuned_hz);
        Ok(vec![
            Format::new(Kind::Messages, 0.0),
            Format::new(Kind::typed::<Picture>(), 0.0),
        ])
    }

    fn process(&mut self, io: &mut Io) {
        if let Some(f) = io.items::<FrontStatus>(1).last() {
            self.front = f.value;
        }
        for e in io.items::<EnsembleItem>(2) {
            self.db = Arc::clone(&e.value.db);
            self.fic = Arc::clone(&e.value.fic);
            if e.value.utc.0 != 0 {
                self.utc = e.value.utc;
            }
        }
        for k in 0..io.items::<MscStatus>(3).len() {
            let m = io.items::<MscStatus>(3)[k].value.clone();
            if m.service != self.msc.service || m.requested != self.msc.requested {
                self.msc = m;
                self.service_changed();
            } else {
                self.msc = m;
            }
        }
        if let Some(a) = io.items::<AudioStatus>(4).last() {
            self.aac = a.value.clone();
        }
        if let Some(a) = io.items::<AudioStatus>(5).last() {
            self.mp2 = a.value.clone();
        }
        for k in 0..io.items::<LabelItem>(6).len() {
            let l = io.items::<LabelItem>(6)[k].value.clone();
            self.label(&l);
        }
        let mut pictures = std::mem::take(io.items_out::<Picture>(1));
        for k in 0..io.items::<MotItem>(7).len() {
            let m = io.items::<MotItem>(7)[k].value.clone();
            self.slide(&m, &mut pictures);
        }
        *io.items_out::<Picture>(1) = pictures;
        if let Some(t) = io.items::<TiiItem>(8).last().map(|t| t.value.clone()) {
            self.write_transmitters(&t.transmitters, t.analysed);
        }
        if let Some(s) = io
            .items::<SpiItem>(9)
            .last()
            .map(|s| Arc::clone(&s.value.0))
        {
            self.spi_objects = Some(s.objects as i64);
            self.bc.guide(&mut self.p, &s);
        }
        if let Some(j) = io
            .items::<JournalineItem>(10)
            .last()
            .map(|j| Arc::clone(&j.value.0))
        {
            self.write_journaline(&j);
        }
        if let Some(d) = io.items::<DmbItem>(11).last() {
            self.dmb = Some(d.value.0.clone());
        }
        for k in 0..io.items::<OfdmStatusItem>(0).len() {
            let s = &io.items::<OfdmStatusItem>(0)[k];
            let t = s.time;
            self.ofdm = s.value.0.clone();
            self.publish(t);
        }
        io.messages_out(0).append(&mut self.events);
    }

    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        match ReportSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            ReportSetting::Channels => {
                let t = CHANNELS.read(id, v)?;
                self.channels =
                    channels::parse_list(t).ok_or(SettingError::Malformed { id, at: None })?;
                t.clone_into(&mut self.channels_text);
            }
            ReportSetting::AnnouncementTypes => {
                self.announcement_types = ANNOUNCEMENT_TYPES.read(id, v)? as u16
            }
        }
        Ok(Applied::Live)
    }

    fn values(&self) -> Values {
        values_of([
            (
                ReportSetting::Channels,
                Value::Text(self.channels_text.clone()),
            ),
            (
                ReportSetting::AnnouncementTypes,
                Value::I64(i64::from(self.announcement_types)),
            ),
        ])
    }

    /// The guide, the logos and the count of guide objects stay if the stack keeps them; everything else belongs to the
    /// ensemble of the frequency before.
    fn forget(&mut self, kept: &Kept) {
        let guide = [
            bc::GUIDE_SERVICE_TABLE.def(),
            bc::GUIDE_PROGRAMME_TABLE.def(),
            bc::GUIDE_GROUP_TABLE.def(),
            bc::LOGO_TABLE.def(),
        ];
        self.restart();
        for t in guide {
            if !kept.table(t.key) {
                self.p.clear_rows(t);
            }
        }
        if !guide.iter().all(|t| kept.table(t.key)) {
            self.bc = Publisher::new();
        }
        if !kept.field(DataFields::SPI_OBJECTS.key) {
            self.spi_objects = None;
            let data = self.data_fields();
            self.p.put(&data);
        }
    }

    fn results(&self) -> Option<&Results> {
        Some(self.p.results())
    }
}
