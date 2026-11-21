//! The ensemble database: what the FIGs say about the ensemble, its sub-channels and services, labels,
//! programme types, announcements, linking and frequencies, plus the FIC records an analyser shows.

use crate::fig::{self, Fig, FigLists, FigRecord, LabelOf, SubChForm};
use crate::registry;
use crate::results::ServiceKind;
use crate::tables::{self, Protection};
use std::collections::{BTreeMap, HashMap};
use std::fmt::Write;

/// A sub-channel (FIG 0/1).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SubChannel {
    /// Sub-channel id.
    pub id: u8,
    /// Start address in CU.
    pub start_cu: u16,
    /// Size in CU (0 = unknown).
    pub size_cu: u16,
    /// The protection; `None` for an undefined short form.
    pub protection: Option<Protection>,
    /// Bit rate in kbit/s (0 = unknown).
    pub bitrate: u16,
    /// FEC scheme (FIG 0/14) for packet mode.
    pub fec: u8,
    /// Language (FIG 0/5).
    pub language: Option<u8>,
}

impl SubChannel {
    /// From a FIG 0/1 entry.
    pub fn from_org(o: &fig::SubChannelOrg) -> Self {
        let (size, protection, bitrate) = match o.form {
            SubChForm::Short(Some(i)) => {
                let u = &tables::UEP[i as usize];
                (u.size_cu, Some(Protection::Uep(i)), u.bitrate)
            }
            SubChForm::Short(None) => (0, None, 0),
            SubChForm::Long {
                option,
                level,
                size,
            } => {
                let br = tables::eep_bitrate(option, level, size as usize).unwrap_or(0) as u16;
                (size, Some(Protection::Eep { option, level }), br)
            }
        };
        SubChannel {
            id: o.id,
            start_cu: o.start,
            size_cu: size,
            protection,
            bitrate,
            fec: 0,
            language: None,
        }
    }
    /// "EEP 3-A", "UEP 12" or "unknown".
    pub fn protection_text(&self) -> String {
        self.protection.map_or("unknown".into(), |p| p.text())
    }
    /// The geometry that decoding depends on.
    pub fn geometry(&self) -> (u16, u16, Option<Protection>, u16) {
        (self.start_cu, self.size_cu, self.protection, self.bitrate)
    }
}

/// A service component (FIG 0/2), with what other FIGs add.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Component {
    /// Transport mechanism.
    pub tmid: u8,
    /// ASCTy (stream audio) or DSCTy (stream data, packet data).
    pub ty: u8,
    /// Sub-channel.
    pub subch: Option<u8>,
    /// SCId (packet mode).
    pub scid: Option<u16>,
    /// Packet address (packet mode, FIG 0/3).
    pub packet_address: Option<u16>,
    /// Data groups (packet mode, FIG 0/3).
    pub data_groups: bool,
    /// Primary.
    pub primary: bool,
    /// Conditional access.
    pub ca: bool,
    /// SCIdS (FIG 0/8), when known.
    pub scids: Option<u8>,
}

/// A user application of a component (FIG 0/13).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UserApp {
    /// SCIdS of the component.
    pub scids: u8,
    /// User application type.
    pub app: u16,
    /// Its name.
    pub name: &'static str,
    /// For an X-PAD application: the X-PAD application type and whether it uses data groups, and the DSCTy.
    pub xpad: Option<(u8, bool, u8)>,
    /// The application data.
    pub data: Vec<u8>,
}

/// A service.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Service {
    /// SId.
    pub sid: u32,
    /// Label.
    pub label: String,
    /// Short label.
    pub short_label: String,
    /// Components.
    pub components: Vec<Component>,
    /// CAId.
    pub caid: u8,
    /// Component labels by SCIdS.
    pub component_labels: BTreeMap<u8, String>,
    /// Programme type.
    pub pty: Option<u8>,
    /// The programme type describes the current programme (S/D = 1).
    pub pty_dynamic: bool,
    /// Language.
    pub language: Option<u8>,
    /// User applications.
    pub user_apps: Vec<UserApp>,
    /// Announcement support flags.
    pub asu: u16,
    /// Announcement clusters.
    pub clusters: Vec<u8>,
    /// Other ensembles carrying this service (FIG 0/24).
    pub other_ensembles: Vec<u16>,
    /// The label came from FIG 2 (FIG 1 labels are then ignored).
    pub ext_label: bool,
}

impl Service {
    /// The playable component: the first stream audio component.
    pub fn audio(&self) -> Option<&Component> {
        self.components.iter().find(|c| c.tmid == 0)
    }
    /// The DMB video component (stream data, DSCTy 24).
    pub fn video(&self) -> Option<&Component> {
        self.components.iter().find(|c| c.tmid == 1 && c.ty == 24)
    }
    /// DAB+, DAB, DMB or data.
    pub fn kind(&self) -> ServiceKind {
        match self.audio() {
            Some(c) if c.ty == 63 => ServiceKind::DabPlus,
            Some(_) => ServiceKind::Dab,
            None if self.video().is_some() => ServiceKind::Dmb,
            None => ServiceKind::Data,
        }
    }
    /// Scrambled: its audio component has the CA flag, or the service has a CAId.
    pub fn scrambled(&self) -> bool {
        self.caid != 0 || self.audio().is_some_and(|c| c.ca)
    }
}

/// An active announcement (FIG 0/19).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Announcement {
    /// Cluster id.
    pub cluster: u8,
    /// ASw flags.
    pub asw: u16,
    /// Sub-channel.
    pub subch: u8,
    /// New flag.
    pub new: bool,
    /// Region.
    pub region: Option<u8>,
    /// The frame it was last heard in.
    pub last_frame: u64,
}

/// A transmitter of the TII database (FIG 0/22).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TiiPlace {
    /// MainId.
    pub main: u8,
    /// SubId (0 for the main identifier's own position).
    pub sub: u8,
    /// Latitude, degrees.
    pub lat: f64,
    /// Longitude, degrees.
    pub lon: f64,
    /// Time delay in µs.
    pub delay_us: u16,
}

/// What the analyser keeps per distinct FIG.
#[derive(Clone, Debug)]
pub struct RecordInfo {
    /// Times seen.
    pub count: u64,
    /// First frame seen.
    pub first_frame: u64,
    /// Last frame seen.
    pub last_frame: u64,
    /// FIB position it came in last.
    pub fib_pos: u8,
}

/// An extended label being collected.
#[derive(Clone, Debug, Default)]
struct ExtLabel {
    toggle: bool,
    /// Up to 8 segments.
    segments: [Option<Vec<u8>>; 8],
    /// The label and the short label assembled from the segments, after the number of flag bytes they were read with.
    assembled: Option<(usize, String, String)>,
}

/// The key of a frequency list in [`Ensemble::frequencies`]: the OE flag, the id in hexadecimal and the R&M, with colons
/// between them.
struct FreqKey {
    len: usize,
    bytes: [u8; 16],
}

impl FreqKey {
    fn new(oe: bool, id: u32, rm: u8) -> Self {
        let mut k = FreqKey {
            len: 0,
            bytes: [0; 16],
        };
        // At most 15 bytes: one digit, 8 hexadecimal digits, 3 digits and two colons.
        let _ = write!(k, "{}:{:X}:{}", oe as u8, id, rm);
        k
    }
    fn as_str(&self) -> &str {
        // Only `write_str` fills the bytes, with whole strings.
        std::str::from_utf8(&self.bytes[..self.len]).unwrap_or_default()
    }
}

impl Write for FreqKey {
    fn write_str(&mut self, s: &str) -> std::fmt::Result {
        let to = self
            .bytes
            .get_mut(self.len..self.len + s.len())
            .ok_or(std::fmt::Error)?;
        to.copy_from_slice(s.as_bytes());
        self.len += s.len();
        Ok(())
    }
}

/// The ensemble database.
#[derive(Clone, Debug, Default)]
pub struct Ensemble {
    /// EId.
    pub eid: Option<u16>,
    /// Label.
    pub label: String,
    /// Short label.
    pub short_label: String,
    /// Ensemble time in seconds since 1970 (0 = not received).
    pub utc_seconds: i64,
    /// Milliseconds of the time.
    pub utc_ms: u16,
    /// Local time offset in half hours.
    pub lto: i8,
    /// Extended country code.
    pub ecc: u8,
    /// The country, the local time offset and the table are received (FIG 0/9).
    pub country: bool,
    /// International table id (1 Europe, 2 North America).
    pub table: u8,
    /// Alarm flag (FIG 0/0).
    pub alarm: bool,
    /// Change flags of FIG 0/0.
    pub change: u8,
    /// Number of services and reconfiguration count (FIG 0/7).
    pub configuration: Option<(u8, u16)>,
    /// Sub-channels.
    pub subchannels: BTreeMap<u8, SubChannel>,
    /// Services by SId.
    pub services: BTreeMap<u32, Service>,
    /// Packet mode components by SCId.
    pub packets: BTreeMap<u16, fig::PacketOrg>,
    /// Linkage sets by LSN.
    pub links: BTreeMap<u16, fig::LinkSet>,
    /// Frequency lists by (OE, id, R&M).
    pub frequencies: BTreeMap<String, fig::FreqList>,
    /// Active announcements by cluster.
    pub announcements: BTreeMap<u8, Announcement>,
    /// The TII database.
    pub tii: Vec<TiiPlace>,
    /// Other-ensemble announcement support (SId, ASu, EIds).
    pub oe_announcements: Vec<(u16, u16, Vec<u16>)>,
    /// Bumped on every change of what the screens list (services, labels, sub-channels).
    pub version: u64,
    /// Bumped on every change of the sub-channel table.
    pub subch_version: u64,
    ext_ensemble_label: bool,
    ext_segments: HashMap<LabelOf, ExtLabel>,
    tii_mains: BTreeMap<u8, (f64, f64)>,
    tii_subs: BTreeMap<u8, Vec<(u8, u16, f64, f64)>>,
    /// Ensemble labels assembled from FIG 2 (for the analyser).
    pub ext_labels: Vec<String>,
    /// The last FIG 1 of each label as sent, so a repeat is not decoded again.
    fig1_labels: HashMap<LabelOf, ([u8; 16], u8, u16)>,
}

/// The FIC analyser: every distinct FIG with counts.
#[derive(Clone, Debug, Default)]
pub struct FicRecords {
    /// Distinct FIGs.
    pub records: HashMap<FigRecord, RecordInfo>,
    /// Per type and extension: (count, bytes with header).
    pub per_type: BTreeMap<(u8, u8), (u64, u64)>,
    /// Bytes used per FIB of the last frame (−1 for a bad CRC).
    pub fib_bytes: [i16; 12],
    /// Good FIBs' bytes used and FIBs counted (for the fill).
    pub used: (u64, u64),
}

impl FicRecords {
    /// Most records kept.
    pub const MAX: usize = 4000;
    /// Frames a record is kept after it was last seen (5 minutes).
    pub const AGE: u64 = 3125;

    /// Records one FIG.
    pub fn add(&mut self, rec: &FigRecord, frame: u64, pos: u8) {
        let e = self.per_type.entry((rec.fig_type, rec.ext)).or_default();
        e.0 += 1;
        e.1 += rec.data().len() as u64 + 1;
        let latest_only = rec.fig_type == 0 && (rec.ext == 0 || rec.ext == 10);
        if latest_only {
            self.records
                .retain(|k, _| !(k.fig_type == 0 && k.ext == rec.ext));
        }
        match self.records.get_mut(rec) {
            Some(i) => {
                i.count += 1;
                i.last_frame = frame;
                i.fib_pos = pos;
            }
            None => {
                if self.records.len() >= Self::MAX {
                    if let Some(old) = self
                        .records
                        .iter()
                        .min_by_key(|(_, i)| i.last_frame)
                        .map(|(k, _)| *k)
                    {
                        self.records.remove(&old);
                    }
                }
                self.records.insert(
                    *rec,
                    RecordInfo {
                        count: 1,
                        first_frame: frame,
                        last_frame: frame,
                        fib_pos: pos,
                    },
                );
            }
        }
    }

    /// Drops records not seen for 5 minutes.
    pub fn age(&mut self, frame: u64) {
        if frame.is_multiple_of(64) {
            self.records
                .retain(|_, i| frame.saturating_sub(i.last_frame) <= Self::AGE);
        }
    }

    /// The share of FIB bytes used in good FIBs, in percent.
    pub fn fill_pct(&self) -> f64 {
        if self.used.1 == 0 {
            0.0
        } else {
            100.0 * self.used.0 as f64 / (30.0 * self.used.1 as f64)
        }
    }
}

/// What applying a FIG changed.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Changes {
    /// The ensemble changed (another EId): everything was cleared.
    pub new_ensemble: bool,
    /// The service list or labels changed.
    pub services: bool,
    /// The sub-channel table changed.
    pub subchannels: bool,
    /// The CIF count of this FIB group (when FIG 0/0 was in it).
    pub cif_count: Option<u16>,
}

impl Ensemble {
    /// Valid: the EId is known and at least one service is listed.
    pub fn valid(&self) -> bool {
        self.eid.is_some() && !self.services.is_empty()
    }

    /// Every service labelled and the ensemble labelled.
    pub fn all_labelled(&self) -> bool {
        self.valid()
            && !self.label.is_empty()
            && self.services.values().all(|s| !s.label.is_empty())
    }

    /// The service that has this sub-channel as its audio component.
    pub fn service_of_subch(&self, subch: u8) -> Option<&Service> {
        self.services
            .values()
            .find(|s| s.audio().and_then(|c| c.subch) == Some(subch))
            .or_else(|| {
                self.services
                    .values()
                    .find(|s| s.components.iter().any(|c| c.subch == Some(subch)))
            })
    }

    fn touch(&mut self) {
        self.version += 1;
    }

    /// Applies one FIG. `lists` is the storage the FIG is decoded into: the caller keeps it from call to call.
    pub fn apply(&mut self, rec: &FigRecord, lists: &mut FigLists, frame: u64, ch: &mut Changes) {
        let Ok(f) = fig::decode(rec, lists) else {
            return;
        };
        let oe = rec.oe();
        let next = rec.cn();
        match f {
            Fig::Ensemble {
                eid,
                change,
                alarm,
                cif_count,
                ..
            } => {
                if let Some(old) = self.eid {
                    if old != eid {
                        *self = Ensemble {
                            version: self.version + 1,
                            subch_version: self.subch_version + 1,
                            ..Default::default()
                        };
                        ch.new_ensemble = true;
                    }
                }
                if self.eid.is_none() {
                    self.eid = Some(eid);
                    self.touch();
                    ch.services = true;
                }
                self.change = change;
                self.alarm = alarm;
                ch.cif_count = Some(cif_count);
            }
            Fig::SubChannels(v) => {
                if next || oe {
                    return;
                }
                for o in v {
                    let mut s = SubChannel::from_org(o);
                    if let Some(old) = self.subchannels.get(&o.id) {
                        s.fec = old.fec;
                        s.language = old.language;
                        if old.geometry() == s.geometry() {
                            continue;
                        }
                    }
                    self.subchannels.insert(o.id, s);
                    self.subch_version += 1;
                    self.touch();
                    ch.subchannels = true;
                }
            }
            Fig::Services(v) => {
                if next || oe {
                    return;
                }
                for o in v {
                    let svc = self.services.entry(o.sid).or_insert_with(|| Service {
                        sid: o.sid,
                        ..Default::default()
                    });
                    let comp = |svc: &Service, c: &fig::ComponentOrg| {
                        let old = svc
                            .components
                            .iter()
                            .find(|x| x.tmid == c.tmid && x.subch == c.subch && x.scid == c.scid);
                        Component {
                            tmid: c.tmid,
                            ty: c.ty,
                            subch: c.subch,
                            scid: c.scid,
                            packet_address: old.and_then(|o| o.packet_address),
                            data_groups: old.is_some_and(|o| o.data_groups),
                            primary: c.primary,
                            ca: c.ca,
                            scids: old.and_then(|o| o.scids).or(if c.primary {
                                Some(0)
                            } else {
                                None
                            }),
                        }
                    };
                    let same = svc.components.len() == o.components.len()
                        && svc
                            .components
                            .iter()
                            .zip(&o.components)
                            .all(|(x, c)| *x == comp(svc, c));
                    if !same || svc.caid != o.caid {
                        let comps: Vec<Component> =
                            o.components.iter().map(|c| comp(svc, c)).collect();
                        svc.components = comps;
                        svc.caid = o.caid;
                        self.version += 1;
                        ch.services = true;
                    }
                }
                self.apply_packets();
            }
            Fig::Packets(v) => {
                if next || oe {
                    return;
                }
                for p in v {
                    self.packets.insert(p.scid, *p);
                }
                self.apply_packets();
            }
            Fig::Languages(v) => {
                if oe {
                    return;
                }
                for &(sc, _scid, lang) in v {
                    if let Some(s) = sc.and_then(|sc| self.subchannels.get_mut(&sc)) {
                        s.language = Some(lang);
                    }
                }
            }
            Fig::Linking(v) => {
                for l in v {
                    if self.links.get(&l.lsn) != Some(l) {
                        self.links.insert(l.lsn, l.clone());
                    }
                }
            }
            Fig::Configuration { services, count } => self.configuration = Some((services, count)),
            Fig::GlobalDefinitions(v) => {
                if next || oe {
                    return;
                }
                for g in v {
                    if let Some(s) = self.services.get_mut(&g.sid) {
                        for c in s.components.iter_mut() {
                            if (g.subch.is_some() && c.subch == g.subch && c.tmid != 3)
                                || (g.scid.is_some() && c.scid == g.scid)
                            {
                                c.scids = Some(g.scids);
                            }
                        }
                    }
                }
            }
            Fig::Country {
                lto, ecc, table, ..
            } => {
                if !oe {
                    self.lto = lto;
                    self.ecc = ecc;
                    self.table = table;
                    self.country = true;
                }
            }
            Fig::DateTime {
                mjd,
                hours,
                minutes,
                seconds,
                ms,
                ..
            } => {
                if mjd > 40587 {
                    let tod =
                        hours as i64 * 3600 + minutes as i64 * 60 + seconds.unwrap_or(0) as i64;
                    self.utc_seconds = (mjd as i64 - 40587) * 86400 + tod;
                    self.utc_ms = ms.unwrap_or(0);
                }
            }
            Fig::UserApps(v) => {
                if next || oe {
                    return;
                }
                for u in v {
                    let Some(s) = self.services.get_mut(&u.sid) else {
                        continue;
                    };
                    let audio = s
                        .components
                        .iter()
                        .any(|c| c.tmid == 0 && c.scids.unwrap_or(0) == u.scids);
                    let xpad = |data: &[u8]| {
                        (audio && data.len() >= 2)
                            .then(|| (data[0] & 31, data[1] & 0x80 == 0, data[1] & 63))
                    };
                    let old = || s.user_apps.iter().filter(|a| a.scids == u.scids);
                    let same = old().count() == u.apps.len()
                        && old().zip(&u.apps).all(|(a, (ty, data))| {
                            a.app == *ty && a.xpad == xpad(data) && a.data[..] == data[..]
                        });
                    if same {
                        // The same applications: they move behind those of the other components, as a new set does.
                        let mut others = 0;
                        for i in 0..s.user_apps.len() {
                            if s.user_apps[i].scids != u.scids {
                                s.user_apps[others..=i].rotate_right(1);
                                others += 1;
                            }
                        }
                        continue;
                    }
                    let apps = u.apps.iter().map(|(ty, data)| UserApp {
                        scids: u.scids,
                        app: *ty,
                        name: registry::user_application(*ty),
                        xpad: xpad(data),
                        data: data.to_vec(),
                    });
                    s.user_apps.retain(|a| a.scids != u.scids);
                    s.user_apps.extend(apps);
                }
            }
            Fig::Fec(v) => {
                for &(sc, fec) in v {
                    if let Some(s) = self.subchannels.get_mut(&sc) {
                        s.fec = fec;
                    }
                }
            }
            Fig::ProgrammeTypes(v) => {
                if oe {
                    return;
                }
                for p in v {
                    if p.secondary {
                        continue;
                    }
                    if let Some(s) = self.services.get_mut(&(p.sid as u32)) {
                        s.pty = Some(p.pty);
                        s.pty_dynamic = p.dynamic;
                        if p.language.is_some() {
                            s.language = p.language;
                        }
                    }
                }
            }
            Fig::AnnouncementSupport(v) => {
                if oe {
                    return;
                }
                for a in v {
                    if let Some(s) = self.services.get_mut(&(a.sid as u32)) {
                        s.asu = a.asu;
                        s.clusters.clone_from(&a.clusters);
                    }
                }
            }
            Fig::AnnouncementSwitching(v) => {
                if oe {
                    return;
                }
                for a in v {
                    if a.asw != 0 {
                        self.announcements.insert(
                            a.cluster,
                            Announcement {
                                cluster: a.cluster,
                                asw: a.asw,
                                subch: a.subch,
                                new: a.new,
                                region: a.region,
                                last_frame: frame,
                            },
                        );
                    } else {
                        self.announcements.remove(&a.cluster);
                    }
                }
            }
            Fig::Frequencies(v) => {
                for l in v {
                    let key = FreqKey::new(oe, l.id, l.rm);
                    match self.frequencies.get_mut(key.as_str()) {
                        Some(old) if old == l => {}
                        Some(old) => *old = l.clone(),
                        None => {
                            self.frequencies.insert(key.as_str().into(), l.clone());
                        }
                    }
                }
            }
            Fig::Tii { mains, subs } => {
                for m in mains {
                    self.tii_mains.insert(m.main, (m.lat, m.lon));
                }
                for s in subs {
                    self.tii_subs.entry(s.main).or_default().clone_from(&s.subs);
                }
                self.place_tii();
            }
            Fig::OeServices(v) => {
                for o in v {
                    if let Some(s) = self.services.get_mut(&o.sid) {
                        s.other_ensembles.clone_from(&o.eids);
                    }
                }
            }
            Fig::OeAnnouncementSupport(v) => {
                if self.oe_announcements != v {
                    self.oe_announcements = v.to_vec();
                }
            }
            Fig::OeAnnouncementSwitching | Fig::Unknown => {}
            Fig::Label {
                of,
                charset,
                text,
                flags,
            } => {
                if oe || self.fig1_labels.get(&of) == Some(&(text, charset, flags)) {
                    return;
                }
                self.fig1_labels.insert(of, (text, charset, flags));
                let label = registry::label_text(&text, charset);
                let short = short_label(&text, charset, flags);
                self.set_label(of, &label, &short, false, ch);
            }
            Fig::ExtLabel {
                of,
                toggle,
                segment,
                rfu,
                data,
            } => {
                let entry = self.ext_segments.entry(of).or_default();
                if entry.toggle != toggle {
                    *entry = ExtLabel {
                        toggle,
                        ..Default::default()
                    };
                }
                let Some(slot) = entry.segments.get_mut(usize::from(segment)) else {
                    return;
                };
                match slot {
                    Some(old) if old[..] == *data => {}
                    Some(old) => {
                        old.clear();
                        old.extend_from_slice(data);
                        entry.assembled = None;
                    }
                    None => {
                        *slot = Some(data.to_vec());
                        entry.assembled = None;
                    }
                }
                let Some(first) = entry.segments[0].as_deref() else {
                    return;
                };
                let Some(&head) = first.first() else {
                    return;
                };
                let utf16 = head & 0x80 != 0;
                let count = usize::from((head >> 4) & 7) + 1;
                if entry.segments[..count].iter().any(Option::is_none) {
                    return;
                }
                let flag_bytes = if rfu && segment == 0 { 0 } else { 2 };
                if entry.assembled.as_ref().is_none_or(|a| a.0 != flag_bytes) {
                    let skip = if first.len() > flag_bytes {
                        1 + flag_bytes
                    } else {
                        1
                    };
                    let flags = if skip == 3 {
                        u16::from_be_bytes([first[1], first[2]])
                    } else {
                        0
                    };
                    let mut bytes = first[skip..].to_vec();
                    for s in entry.segments[1..count].iter().flatten() {
                        bytes.extend_from_slice(s);
                    }
                    let label = registry::label_text(&bytes, if utf16 { 6 } else { 15 });
                    if label.is_empty() {
                        return;
                    }
                    let chars: Vec<char> = label.chars().collect();
                    let short: String = (0..16)
                        .filter(|&i| flags & (0x8000 >> i) != 0)
                        .filter_map(|i| chars.get(i))
                        .take(8)
                        .collect::<String>()
                        .trim_end()
                        .to_string();
                    entry.assembled = Some((flag_bytes, label, short));
                }
                // The assembled label leaves its entry while the database, which holds the entry, is written.
                let Some((read_with, label, short)) = entry.assembled.take() else {
                    return;
                };
                let mut kept = if self.ext_labels.len() >= 32 {
                    self.ext_labels.remove(0)
                } else {
                    String::new()
                };
                kept.clone_from(&label);
                self.ext_labels.push(kept);
                self.set_label(of, &label, &short, true, ch);
                if let Some(entry) = self.ext_segments.get_mut(&of) {
                    entry.assembled = Some((read_with, label, short));
                }
            }
        }
    }

    fn set_label(
        &mut self,
        of: LabelOf,
        label: &str,
        short: &str,
        extended: bool,
        ch: &mut Changes,
    ) {
        match of {
            LabelOf::Ensemble(eid) => {
                if self.eid.is_some_and(|e| e != eid) {
                    return;
                }
                if self.ext_ensemble_label && !extended {
                    return;
                }
                self.ext_ensemble_label |= extended;
                if self.label != label || self.short_label != short {
                    label.clone_into(&mut self.label);
                    short.clone_into(&mut self.short_label);
                    self.touch();
                    ch.services = true;
                }
            }
            LabelOf::Service(sid) | LabelOf::DataService(sid) => {
                let s = self.services.entry(sid).or_insert_with(|| Service {
                    sid,
                    ..Default::default()
                });
                if s.ext_label && !extended {
                    return;
                }
                s.ext_label |= extended;
                if s.label != label || s.short_label != short {
                    label.clone_into(&mut s.label);
                    short.clone_into(&mut s.short_label);
                    self.version += 1;
                    ch.services = true;
                }
            }
            LabelOf::Component { sid, scids } => {
                if let Some(s) = self.services.get_mut(&sid) {
                    match s.component_labels.get_mut(&scids) {
                        Some(old) => label.clone_into(old),
                        None => {
                            s.component_labels.insert(scids, label.into());
                        }
                    }
                }
            }
            _ => {}
        }
    }

    fn apply_packets(&mut self) {
        for s in self.services.values_mut() {
            for c in s.components.iter_mut() {
                if let Some(p) = c.scid.and_then(|id| self.packets.get(&id)) {
                    c.subch = Some(p.subch);
                    c.packet_address = Some(p.address);
                    c.data_groups = p.data_groups;
                    c.ty = p.dscty;
                }
            }
        }
    }

    fn place_tii(&mut self) {
        self.tii.clear();
        for (&main, &(lat, lon)) in &self.tii_mains {
            self.tii.push(TiiPlace {
                main,
                sub: 0,
                lat,
                lon,
                delay_us: 0,
            });
            if let Some(subs) = self.tii_subs.get(&main) {
                for &(sub, td, dlat, dlon) in subs {
                    self.tii.push(TiiPlace {
                        main,
                        sub,
                        lat: lat + dlat,
                        lon: lon + dlon,
                        delay_us: td,
                    });
                }
            }
        }
    }

    /// Ends announcements not heard for 40 frames.
    pub fn age_announcements(&mut self, frame: u64) {
        self.announcements
            .retain(|_, a| frame.saturating_sub(a.last_frame) <= 40);
    }
}

/// The short label: the characters whose flag bit is set (most significant bit the first character), at most 8, trailing spaces
/// removed.
pub fn short_label(text: &[u8; 16], charset: u8, flags: u16) -> String {
    if charset == 6 {
        let full = registry::label_text(text, 6);
        let chars: Vec<char> = full.chars().collect();
        return (0..16)
            .filter(|&i| flags & (0x8000 >> i) != 0)
            .filter_map(|i| chars.get(i))
            .take(8)
            .collect::<String>()
            .trim_end()
            .into();
    }
    if charset == 15 {
        let full = String::from_utf8_lossy(text).to_string();
        let chars: Vec<char> = full.chars().collect();
        let short: String = (0..16)
            .filter(|&i| flags & (0x8000 >> i) != 0)
            .filter_map(|i| chars.get(i))
            .take(8)
            .collect();
        return registry::label_text(short.as_bytes(), 15);
    }
    let picked: Vec<u8> = (0..16)
        .filter(|&i| flags & (0x8000 >> i) != 0)
        .map(|i| text[i])
        .take(8)
        .collect();
    registry::label_text(&picked, charset)
}
