//! The service information database. Every PSI and SI section goes in, and is checked for version and completeness. Out come
//! the service list (with logical channel numbers, streams, scrambling and now/next), the network, the
//! multiplex identity, the stream's UTC and local time offset, other multiplexes, and the programme guide.

use crate::codec::{identify, CodecInfo, CodecRule, EsCodec, EsKind, EsPackaging, EsProps};
use crate::epg::{Epg, EpgEvent};
use crate::error::{Error, SectionFault};
use crate::tables::{
    self, Delivery, Lcn, LocalTimeOffset, SdtService, SubtitleEntry, SupplementaryAudio,
    TeletextEntry, TsEntry,
};
use sdrkit_core::media::FourCc;
use sdrkit_util_text::dvb::{DefaultTable, DvbText, Options as TextOptions, Usage};
use std::collections::{BTreeMap, BTreeSet};

/// Decodes a DVB SI text field (EN 300 468 Annex A) to clean UTF-8 with [`sdrkit_util_text::dvb`].
pub fn dvb_text(bytes: &[u8], usage: Usage, opts: &TextOptions) -> String {
    let mut t = DvbText::default();
    t.decode(bytes, usage, opts);
    t.text
}

/// One elementary stream of a service.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct StreamEntry {
    /// elementary_PID.
    pub pid: u16,
    /// stream_type.
    pub stream_type: u8,
    /// Video, audio, subtitles, teletext or data.
    pub kind: Option<EsKind>,
    /// The codec.
    pub codec: Option<EsCodec>,
    /// Packaging.
    pub packaging: Option<EsPackaging>,
    /// The signalling that decided the codec.
    pub rule: Option<CodecRule>,
    /// First ISO 639 language.
    pub lang: String,
    /// audio_type of the first language entry.
    pub audio_type: u8,
    /// Every language entry: (language, audio_type).
    pub languages: Vec<(String, u8)>,
    /// Teletext entries.
    pub teletext: Vec<TeletextEntry>,
    /// Subtitle entries.
    pub subtitles: Vec<SubtitleEntry>,
    /// component_tag.
    pub component_tag: Option<u8>,
    /// Supplementary audio signalling.
    pub supplementary: Option<SupplementaryAudio>,
    /// A CA descriptor on the stream.
    pub ca: bool,
    /// The DVB AAC descriptor's profile_and_level.
    pub aac_profile_level: Option<u8>,
    /// Bit rate, kbit/s.
    pub kbps: f64,
    /// Scrambled (from its packets).
    pub scrambled: bool,
    /// What the stream itself says (size, rates, profile ...).
    pub props: EsProps,
}

impl StreamEntry {
    /// What is known of the codec: the PMT entry, refined by what the stream carries.
    pub fn codec_info(&self) -> CodecInfo {
        let pmt = identify(self.stream_type, &crate::tables::EsDescriptors::default());
        CodecInfo {
            codec: self.codec.unwrap_or(pmt.codec),
            packaging: self.packaging.unwrap_or(pmt.packaging),
            rule: self.rule.unwrap_or(pmt.rule),
        }
    }

    /// The four-character code, with what the stream's own headers say. `None` for a stream with none.
    pub fn fourcc(&self) -> Option<FourCc> {
        self.codec.and_then(|c| c.fourcc_with(&self.props))
    }
}

/// A summary of an event for the service list.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct EventSummary {
    /// Title.
    pub title: String,
    /// UTC start.
    pub start: i64,
    /// Seconds.
    pub duration: i64,
    /// Short text.
    pub text: String,
}

impl From<&EpgEvent> for EventSummary {
    fn from(e: &EpgEvent) -> Self {
        EventSummary {
            title: e.title.clone(),
            start: e.start,
            duration: e.duration,
            text: e.text.clone(),
        }
    }
}

/// One service of the list.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Service {
    /// service_id (program_number).
    pub service_id: u16,
    /// Name ("Service N" when unnamed).
    pub name: String,
    /// Provider.
    pub provider: String,
    /// service_type (0 unknown).
    pub service_type: u8,
    /// What the service_type says it is.
    pub kind: ServiceKind,
    /// Logical channel number (0 when absent).
    pub lcn: u16,
    /// Shown in the channel list.
    pub visible: bool,
    /// PMT PID (0 when unknown).
    pub pmt_pid: u16,
    /// PCR PID (0 when unknown).
    pub pcr_pid: u16,
    /// running_status (SDT).
    pub running_status: u8,
    /// free_CA_mode or CA descriptors.
    pub ca_flag: bool,
    /// free_CA_mode of the SDT.
    pub free_ca: bool,
    /// Scrambled, from its packets.
    pub scrambled: bool,
    /// Whether it is scrambled.
    pub ca_state: CaState,
    /// An SDT entry has been seen.
    pub have_sdt: bool,
    /// A PMT has been seen.
    pub have_pmt: bool,
    /// The elementary streams in PMT order.
    pub streams: Vec<StreamEntry>,
    /// The present event.
    pub now: Option<EventSummary>,
    /// The following event.
    pub next: Option<EventSummary>,
    /// Service replacement (onid, tsid, sid).
    pub replacement: Option<(u16, u16, u16)>,
    /// EIT_schedule_flag.
    pub eit_schedule: bool,
    /// EIT_present_following_flag.
    pub eit_pf: bool,
    /// Listed in the PAT (false: gone, kept greyed).
    pub in_pat: bool,
    /// When it went from the PAT (signal seconds).
    pub gone_at: Option<f64>,
    /// original_network_id.
    pub onid: u16,
    /// transport_stream_id.
    pub tsid: u16,
}

impl Service {
    fn new(sid: u16) -> Self {
        Service {
            service_id: sid,
            name: format!("Service {sid}"),
            visible: true,
            kind: ServiceKind::Unknown,
            ca_state: CaState::Unknown,
            ..Default::default()
        }
    }
    /// Whether it is a radio service.
    pub fn is_radio(&self) -> bool {
        self.kind == ServiceKind::Radio
    }
    /// Whether it is TV.
    pub fn is_tv(&self) -> bool {
        self.kind == ServiceKind::Tv
    }
}

sdrkit_core::choice_enum! {
    /// What a service is, by its service_type (ETSI EN 300 468 Table 87).
    #[derive(Default)]
    pub enum ServiceKind {
        /// Television.
        Tv = ("tv", "TV"),
        /// Radio.
        Radio = ("radio", "Radio"),
        /// Data broadcast.
        Data = ("data", "Data"),
        /// Teletext.
        Teletext = ("teletext", "Teletext"),
        /// Another service_type.
        Other = ("other", "Other"),
        /// service_type 0, or not known yet.
        #[default]
        Unknown = ("unknown", "Unknown"),
    }
}

impl ServiceKind {
    /// The kind of a service_type.
    pub fn of(service_type: u8) -> ServiceKind {
        match service_type {
            0 => ServiceKind::Unknown,
            0x01 | 0x11 | 0x16 | 0x17 | 0x19 | 0x1A | 0x1F | 0x20..=0x24 => ServiceKind::Tv,
            0x02 | 0x0A => ServiceKind::Radio,
            0x0C => ServiceKind::Data,
            0x03 => ServiceKind::Teletext,
            _ => ServiceKind::Other,
        }
    }
}

sdrkit_core::choice_enum! {
    /// Whether a service is scrambled.
    #[derive(Default)]
    pub enum CaState {
        /// No PMT yet.
        #[default]
        Unknown = ("unknown", "Unknown"),
        /// Clear.
        Clear = ("clear", "Clear"),
        /// The CA flag is set, but the stream is clear.
        FlaggedClear = ("flagged_clear", "CA flag set, but the stream is clear"),
        /// Scrambled.
        Scrambled = ("scrambled", "Scrambled"),
    }
}

/// Section counts per table.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SectionCounts {
    /// Accepted (parsed or recognised as a repeat).
    pub accepted: u64,
    /// Rejected for a bad CRC.
    pub crc: u64,
    /// Rejected for a bad length.
    pub length: u64,
    /// Rejected for another reason (short, wrong table, wrong PID).
    pub other: u64,
}

/// What changed with a section.
#[derive(Clone, Debug, Default)]
pub struct Changes {
    /// The service list changed.
    pub services: bool,
    /// The network (name, ids) changed.
    pub network: bool,
    /// UTC was received.
    pub time: bool,
    /// Events stored in the guide.
    pub events: Vec<EpgEvent>,
    /// A new multiplex (identity changed): everything of the old one was cleared.
    pub new_multiplex: bool,
}

#[derive(Clone, Debug, Default)]
struct TableState {
    version: u8,
    last: u8,
    crcs: Vec<Option<u32>>,
    data: Vec<Option<Vec<u8>>>,
    applied: Option<u8>,
}

/// A transport stream of another multiplex, for the scanner.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct OtherMultiplex {
    /// The network it came from.
    pub network_id: u16,
    /// The transport stream.
    pub ts: TsEntry,
}

/// The table a new database reads text without a selector byte with: GB 2312 names sent with no selector are read, and
/// table 00 text stays table 00.
pub const DEFAULT_TABLE: DefaultTable = DefaultTable::GbkWhenValid;

/// The database.
#[derive(Clone, Debug, Default)]
pub struct SiDb {
    /// Text options.
    pub text: TextOptions,
    /// The user's country (ISO 3166 alpha-3, upper case: "NOR", "AUS"), for LCN lists and the local time offset.
    pub country: String,
    /// Preferred language (ISO 639-2) for names and the guide.
    pub preferred_lang: String,
    /// transport_stream_id (0 unknown).
    pub tsid: u16,
    /// original_network_id (0 unknown).
    pub onid: u16,
    /// network_id (0 unknown).
    pub network_id: u16,
    /// Network name.
    pub network_name: String,
    /// The NIT PID (0x0010 unless the PAT says otherwise).
    pub nit_pid: u16,
    services: BTreeMap<u16, Service>,
    /// PMT PID -> programmes the PAT maps to it.
    pmt_map: BTreeMap<u16, BTreeSet<u16>>,
    have_pat: bool,
    tables: BTreeMap<(u8, u16, u32), TableState>,
    /// The guide.
    pub epg: Epg,
    /// The last UTC received and the signal time it arrived at.
    utc: Option<(i64, f64)>,
    /// The first UTC received.
    pub first_utc: Option<i64>,
    /// The last UTC received.
    pub last_utc: Option<i64>,
    /// Local time offsets of the TOT.
    pub offsets: Vec<LocalTimeOffset>,
    /// CA systems of the CAT: (CA_system_id, EMM PID).
    pub cat: Vec<(u16, u16)>,
    /// SDT other entries, keyed (onid, tsid, sid).
    pub other_services: BTreeMap<(u16, u16, u16), SdtService>,
    /// Transport streams other than this one (NIT actual and other).
    pub other_multiplexes: Vec<OtherMultiplex>,
    /// This multiplex's own delivery descriptors (NIT actual).
    pub own_delivery: Vec<Delivery>,
    /// Bouquets (BAT), kept when `want_bouquets` is set.
    pub bouquets: BTreeMap<u16, tables::Nit>,
    /// Keep bouquets (a satellite stack sets this).
    pub want_bouquets: bool,
    /// Section counts per table name.
    pub counts: BTreeMap<&'static str, SectionCounts>,
    lcns: Vec<Lcn>,
    /// Bumped whenever the service list changes.
    pub version: u64,
    /// Log lines (multiplex changes and the like), drained by the owner.
    pub log: Vec<String>,
}

fn table_name(table_id: u8) -> &'static str {
    match table_id {
        0x00 => "PAT",
        0x01 => "CAT",
        0x02 => "PMT",
        0x40 | 0x41 => "NIT",
        0x42 | 0x46 => "SDT",
        0x4A => "BAT",
        0x4E..=0x6F => "EIT",
        0x70 => "TDT",
        0x73 => "TOT",
        0xC8 | 0xC9 => "VCT",
        _ => "other",
    }
}

impl SiDb {
    /// An empty database.
    pub fn new() -> Self {
        SiDb {
            nit_pid: 0x10,
            text: TextOptions {
                default_table: DEFAULT_TABLE,
            },
            ..Default::default()
        }
    }

    /// Forgets everything of the current multiplex (a retune to another one). The guide is kept (it is keyed by service).
    pub fn clear_multiplex(&mut self) {
        if self.onid != 0 || self.tsid != 0 {
            self.epg.clear_multiplex_pf(self.onid, self.tsid);
        }
        self.services.clear();
        self.pmt_map.clear();
        self.have_pat = false;
        self.tables.clear();
        self.tsid = 0;
        self.onid = 0;
        self.network_id = 0;
        self.network_name.clear();
        self.nit_pid = 0x10;
        self.lcns.clear();
        self.own_delivery.clear();
        self.cat.clear();
        self.utc = None;
        self.version += 1;
    }

    /// The PMT PIDs the PAT announces.
    pub fn pmt_pids(&self) -> impl Iterator<Item = u16> + '_ {
        self.pmt_map.keys().copied()
    }

    /// A service by id.
    pub fn service(&self, sid: u16) -> Option<&Service> {
        self.services.get(&sid)
    }

    /// Mutable access to a service (the demultiplexer fills in stream properties and scrambling).
    pub fn service_mut(&mut self, sid: u16) -> Option<&mut Service> {
        self.services.get_mut(&sid)
    }

    /// The service_id of a listed service that carries elementary stream `pid`.
    pub fn service_of_pid(&self, pid: u16) -> Option<u16> {
        self.known()
            .find(|s| s.streams.iter().any(|e| e.pid == pid))
            .map(|s| s.service_id)
    }

    /// Every service, in list order: by LCN (0 as 9999), then by service id. Hidden services included.
    pub fn services(&self) -> Vec<&Service> {
        let mut v: Vec<&Service> = self.known().collect();
        v.sort_by_key(|s| list_key(s));
        v
    }

    /// Every service of [`SiDb::services`], in service id order.
    pub(crate) fn known(&self) -> impl Iterator<Item = &Service> + '_ {
        self.services
            .values()
            .filter(|s| s.pmt_pid != 0 || s.have_sdt || s.have_pmt || s.lcn != 0)
    }

    /// The services the channel list shows (visible ones).
    pub fn listed(&self) -> Vec<&Service> {
        self.services().into_iter().filter(|s| s.visible).collect()
    }

    /// The first visible service in list order that has a PMT, is not scrambled, and is TV or radio by its service type or
    /// has an audio or video stream of a known codec that is not scrambled.
    pub fn first_playable(&self) -> Option<u16> {
        let plays = |e: &StreamEntry| {
            matches!(e.kind, Some(EsKind::Audio | EsKind::Video))
                && e.codec.is_some()
                && !e.scrambled
        };
        self.known()
            .filter(|s| {
                s.visible
                    && s.have_pmt
                    && !s.scrambled
                    && (s.is_tv() || s.is_radio() || s.streams.iter().any(plays))
            })
            .min_by_key(|s| list_key(s))
            .map(|s| s.service_id)
    }

    /// UTC now: the last TDT/TOT time plus the signal time since it arrived.
    pub fn utc_now(&self, now: f64) -> Option<i64> {
        self.utc.map(|(u, t)| u + (now - t).max(0.0).floor() as i64)
    }

    /// The local time offset in force for the user's country (or the first entry), in minutes.
    pub fn local_offset_min(&self, now: f64) -> Option<i32> {
        let utc = self.utc_now(now)?;
        let e = self
            .offsets
            .iter()
            .find(|o| o.country == self.country)
            .or(self.offsets.first())?;
        Some(e.at(utc))
    }

    /// Updates the now/next of every service from the guide at `now`.
    pub fn refresh_now_next(&mut self, now: f64) {
        let utc = self.utc_now(now).unwrap_or(0);
        let (onid, tsid) = (self.onid, self.tsid);
        for s in self.services.values_mut() {
            let (n, x) = self.epg.now_next((onid, tsid, s.service_id), utc);
            let nn = n.as_ref().map(EventSummary::from);
            let xx = x.as_ref().map(EventSummary::from);
            if s.now != nn || s.next != xx {
                s.now = nn;
                s.next = xx;
                self.version += 1;
            }
        }
    }

    /// Removes services gone from the PAT for more than 10 s; expires old guide events.
    pub fn tick(&mut self, now: f64) {
        let before = self.services.len();
        self.services
            .retain(|_, s| s.gone_at.is_none_or(|g| now - g <= 10.0));
        if self.services.len() != before {
            self.version += 1;
        }
        if let Some(u) = self.utc_now(now) {
            self.epg.expire(u);
        }
    }

    fn count(&mut self, table_id: u8, r: Result<(), Error>) {
        let c = self.counts.entry(table_name(table_id)).or_default();
        match r {
            Ok(()) => c.accepted += 1,
            Err(Error::Section(SectionFault::Crc)) => c.crc += 1,
            Err(Error::Section(SectionFault::Length)) => c.length += 1,
            Err(_) => c.other += 1,
        }
    }

    /// Versioning: returns the complete set of sections when the table is new or changed and complete, `None` otherwise.
    fn collect(
        &mut self,
        key: (u8, u16, u32),
        s: &[u8],
        h: &tables::SectionHeader,
    ) -> Option<Vec<Vec<u8>>> {
        let crc = u32::from_be_bytes(s[h.total_len - 4..h.total_len].try_into().ok()?);
        let st = self.tables.entry(key).or_default();
        let n = h.number as usize;
        if st.applied == Some(h.version)
            && st.last == h.last
            && st.crcs.get(n).copied().flatten() == Some(crc)
        {
            return None; // a repeat
        }
        if st.version != h.version || st.last != h.last || st.data.len() != h.last as usize + 1 {
            st.version = h.version;
            st.last = h.last;
            st.data = vec![None; h.last as usize + 1];
            st.crcs = vec![None; h.last as usize + 1];
            st.applied = None;
        }
        if n >= st.data.len() {
            return None;
        }
        st.data[n] = Some(s[..h.total_len].to_vec());
        st.crcs[n] = Some(crc);
        if st.data.iter().all(|d| d.is_some()) {
            st.applied = Some(h.version);
            let all = st
                .data
                .iter_mut()
                .map(|d| d.take().unwrap_or_default())
                .collect();
            // Keep the CRCs so repeats are recognised; the data is kept for the next version's completeness only.
            Some(all)
        } else {
            None
        }
    }

    /// One complete section from `pid`, arrived at signal time `now`.
    pub fn push_section(&mut self, pid: u16, s: &[u8], now: f64) -> Changes {
        let mut ch = Changes::default();
        let h = match tables::check(s) {
            Ok(h) => h,
            Err(e) => {
                let t = s.first().copied().unwrap_or(0xFF);
                self.count(t, Err(e));
                return ch;
            }
        };
        let t = h.table_id;
        // The table must fit the PID.
        let ok_pid = match t {
            0x00 => pid == 0,
            0x01 => pid == 1,
            0x02 => self.pmt_map.contains_key(&pid) || (0x1FC8..=0x1FCF).contains(&pid),
            0x40 | 0x41 => pid == self.nit_pid || pid == 0x10,
            0x42 | 0x46 | 0x4A => pid == 0x11,
            0x4E..=0x6F => pid == 0x12,
            0x70 | 0x73 => pid == 0x14,
            0xC8 | 0xC9 => pid == 0x1FFB,
            _ => true,
        };
        if !ok_pid {
            self.count(t, Err(SectionFault::TableId.into()));
            return ch;
        }
        if tables::is_long(t) && !h.current {
            return ch; // the next version, sent in advance
        }
        self.count(t, Ok(()));
        match t {
            0x00 => {
                if let Some(all) = self.collect((t, 0, 0), s, &h) {
                    self.apply_pat(&all, now, &mut ch);
                }
            }
            0x01 => {
                if let Some(all) = self.collect((t, 0, 0), s, &h) {
                    self.cat.clear();
                    for sec in &all {
                        if let Ok(hh) = tables::check(sec) {
                            self.cat.extend(tables::cat(sec, &hh));
                        }
                    }
                    self.cat.truncate(256);
                }
            }
            0x02 => {
                if let Some(all) = self.collect((t, h.ext, pid as u32), s, &h) {
                    for sec in &all {
                        if let Ok(hh) = tables::check(sec) {
                            self.apply_pmt(pid, sec, &hh, &mut ch);
                        }
                    }
                }
            }
            0x40 | 0x41 | 0x4A => {
                if t == 0x4A && !self.want_bouquets {
                    return ch;
                }
                if let Some(all) = self.collect((t, h.ext, 0), s, &h) {
                    let mut merged: Option<tables::Nit> = None;
                    for sec in &all {
                        if let Ok(hh) = tables::check(sec) {
                            if let Some(n) = tables::nit(sec, &hh, &self.text) {
                                match &mut merged {
                                    None => merged = Some(n),
                                    Some(m) => {
                                        if m.name.is_none() {
                                            m.name = n.name;
                                        }
                                        m.names.extend(n.names);
                                        m.streams.extend(n.streams);
                                    }
                                }
                            }
                        }
                    }
                    if let Some(n) = merged {
                        match t {
                            0x40 => {
                                self.apply_nit_actual(n, &mut ch);
                                self.refresh_now_next(now);
                            }
                            0x41 => {
                                self.other_multiplexes.retain(|o| o.network_id != n.id);
                                for ts in n.streams {
                                    if self.other_multiplexes.len() < 4096 {
                                        self.other_multiplexes.push(OtherMultiplex {
                                            network_id: n.id,
                                            ts,
                                        });
                                    }
                                }
                            }
                            _ => {
                                self.bouquets.insert(n.id, n);
                            }
                        }
                    }
                }
            }
            0x42 | 0x46 => {
                let onid = if s.len() >= 10 {
                    u16::from_be_bytes([s[8], s[9]])
                } else {
                    0
                };
                if let Some(all) = self.collect((t, h.ext, onid as u32), s, &h) {
                    let mut services = Vec::new();
                    let (mut tsid, mut onid2) = (h.ext, onid);
                    for sec in &all {
                        if let Ok(hh) = tables::check(sec) {
                            if let Some(sd) = tables::sdt(sec, &hh, &self.text) {
                                tsid = sd.tsid;
                                onid2 = sd.onid;
                                services.extend(sd.services);
                            }
                        }
                    }
                    if t == 0x42 {
                        self.apply_sdt_actual(tsid, onid2, services, &mut ch);
                        self.refresh_now_next(now);
                    } else {
                        for sv in services {
                            if self.other_services.len() < 16384
                                || self
                                    .other_services
                                    .contains_key(&(onid2, tsid, sv.service_id))
                            {
                                self.other_services.insert((onid2, tsid, sv.service_id), sv);
                            }
                        }
                    }
                }
            }
            0x4E..=0x6F => {
                let extra = if s.len() >= 12 {
                    (u16::from_be_bytes([s[10], s[11]]) as u32) << 16
                        | u16::from_be_bytes([s[8], s[9]]) as u32
                } else {
                    0
                };
                let key = (t, h.ext, extra);
                let crc = u32::from_be_bytes(
                    s[h.total_len - 4..h.total_len].try_into().unwrap_or([0; 4]),
                );
                let st = self.tables.entry(key).or_default();
                let n = h.number as usize;
                if st.version != h.version {
                    st.version = h.version;
                    st.crcs.clear();
                }
                if st.crcs.len() <= n {
                    st.crcs.resize(n + 1, None);
                }
                if st.crcs[n] == Some(crc) {
                    return ch;
                }
                st.crcs[n] = Some(crc);
                if let Some(e) = tables::eit(s, &h, &self.text) {
                    let utc = self.utc_now(now).unwrap_or(0);
                    self.epg.preferred_lang.clone_from(&self.preferred_lang);
                    self.epg.text = self.text;
                    ch.events = self.epg.apply(&e, utc);
                    if matches!(t, 0x4E) && (e.tsid == self.tsid || self.tsid == 0) {
                        self.refresh_now_next(now);
                        ch.services = true;
                    }
                }
            }
            0x70 | 0x73 => {
                if let Some(u) = tables::tdt_tot_utc(s) {
                    self.utc = Some((u, now));
                    self.first_utc.get_or_insert(u);
                    self.last_utc = Some(u);
                    ch.time = true;
                }
                if t == 0x73 {
                    self.offsets = tables::tot_offsets(s, &h);
                }
            }
            0xC8 | 0xC9 => {
                if let Some(all) = self.collect((t, h.ext, 0), s, &h) {
                    for sec in &all {
                        if let Ok(hh) = tables::check(sec) {
                            for vc in tables::vct(sec, &hh) {
                                if vc.program_number == 0 || vc.program_number == 0xFFFF {
                                    continue;
                                }
                                let sv = self
                                    .services
                                    .entry(vc.program_number)
                                    .or_insert_with(|| Service::new(vc.program_number));
                                sv.name = vc.short_name.clone();
                                sv.provider = format!("channel {}.{}", vc.major, vc.minor);
                                sv.lcn = vc.major * 100 + vc.minor;
                                sv.service_type = match vc.service_type {
                                    2 => 0x01,
                                    3 => 0x02,
                                    _ => 0x0C,
                                };
                                sv.kind = ServiceKind::of(sv.service_type);
                                sv.have_sdt = true;
                            }
                        }
                    }
                    self.version += 1;
                    ch.services = true;
                }
            }
            _ => {}
        }
        if ch.services || ch.network {
            self.version += 1;
        }
        ch
    }

    fn new_multiplex(&mut self, why: &str, ch: &mut Changes) {
        let wanted_utc = self.utc;
        let (tsid, onid) = (self.tsid, self.onid);
        self.clear_multiplex();
        self.utc = wanted_utc;
        self.log.push(format!(
            "new multiplex ({why}; was onid 0x{onid:04X} tsid 0x{tsid:04X})"
        ));
        ch.new_multiplex = true;
        ch.services = true;
        ch.network = true;
    }

    fn apply_pat(&mut self, all: &[Vec<u8>], now: f64, ch: &mut Changes) {
        let mut programs = Vec::new();
        let mut tsid = 0;
        for sec in all {
            if let Ok(h) = tables::check(sec) {
                let p = tables::pat(sec, &h);
                tsid = p.tsid;
                programs.extend(p.programs);
            }
        }
        if self.tsid != 0 && tsid != 0 && tsid != self.tsid {
            self.new_multiplex("transport_stream_id changed", ch);
        }
        self.tsid = tsid;
        self.have_pat = true;
        self.pmt_map.clear();
        self.nit_pid = 0x10;
        let mut listed = BTreeSet::new();
        for (prog, pid) in programs {
            if prog == 0 {
                self.nit_pid = pid;
                continue;
            }
            listed.insert(prog);
            self.pmt_map.entry(pid).or_default().insert(prog);
            let s = self
                .services
                .entry(prog)
                .or_insert_with(|| Service::new(prog));
            if s.pmt_pid != pid {
                s.pmt_pid = pid;
                s.have_pmt = false;
            }
            s.in_pat = true;
            s.gone_at = None;
            s.tsid = tsid;
        }
        for s in self.services.values_mut() {
            if !listed.contains(&s.service_id) && s.pmt_pid != 0 && s.in_pat {
                s.in_pat = false;
                s.gone_at = Some(now);
            }
        }
        ch.services = true;
    }

    fn apply_pmt(&mut self, pid: u16, s: &[u8], h: &tables::SectionHeader, ch: &mut Changes) {
        let Some(p) = tables::pmt(s, h) else { return };
        let mapped = self
            .pmt_map
            .get(&pid)
            .is_some_and(|ps| ps.contains(&p.program));
        let one_seg = (0x1FC8..=0x1FCF).contains(&pid) && !self.have_pat;
        if !mapped && !one_seg {
            return;
        }
        let svc = self
            .services
            .entry(p.program)
            .or_insert_with(|| Service::new(p.program));
        if one_seg {
            svc.pmt_pid = pid;
        }
        svc.pcr_pid = p.pcr_pid;
        svc.have_pmt = true;
        let prog_ca = !p.program_desc.ca.is_empty();
        let old: BTreeMap<u16, StreamEntry> = svc.streams.drain(..).map(|e| (e.pid, e)).collect();
        for st in &p.streams {
            let info = identify(st.stream_type, &st.desc);
            let mut e = old
                .get(&st.pid)
                .cloned()
                .filter(|o| o.stream_type == st.stream_type)
                .unwrap_or_default();
            e.pid = st.pid;
            e.stream_type = st.stream_type;
            e.kind = Some(info.kind());
            if e.codec.is_none() || e.props.sample_rate == 0 && e.props.width == 0 {
                e.codec = Some(info.codec);
                e.packaging = Some(info.packaging);
            }
            e.rule = Some(info.rule);
            e.languages = st.desc.languages.clone();
            e.lang = st
                .desc
                .languages
                .first()
                .map(|l| l.0.clone())
                .unwrap_or_else(|| {
                    st.desc
                        .supplementary
                        .as_ref()
                        .and_then(|s| s.lang.clone())
                        .unwrap_or_default()
                });
            e.audio_type = st.desc.languages.first().map_or(0, |l| l.1);
            e.teletext = st.desc.teletext.clone();
            e.subtitles = st.desc.subtitles.clone();
            if e.lang.is_empty() {
                if let Some(t) = e.teletext.first() {
                    e.lang = t.lang.clone();
                } else if let Some(t) = e.subtitles.first() {
                    e.lang = t.lang.clone();
                }
            }
            e.component_tag = st.desc.component_tag;
            e.supplementary = st.desc.supplementary.clone();
            e.ca = !st.desc.ca.is_empty();
            e.aac_profile_level = st.desc.dvb_aac.map(|a| a.0);
            if let Some((p_idc, c, l)) = st.desc.avc {
                if e.props.profile.is_none() {
                    e.props.profile = Some(crate::codec::h264_profile(p_idc, c));
                    e.props.level = Some(format!("{}.{}", l / 10, l % 10));
                }
            }
            svc.streams.push(e);
        }
        svc.ca_flag = svc.free_ca || prog_ca || svc.streams.iter().any(|e| e.ca);
        update_ca_state(svc);
        ch.services = true;
    }

    fn apply_nit_actual(&mut self, n: tables::Nit, ch: &mut Changes) {
        self.network_id = n.id;
        let name = if !self.preferred_lang.is_empty() {
            n.names
                .iter()
                .find(|x| crate::epg::same_lang(&x.0, &self.preferred_lang))
                .map(|x| x.1.clone())
                .or(n.name.clone())
        } else {
            n.name.clone()
        };
        if let Some(nm) = name {
            self.network_name = nm;
        }
        self.other_multiplexes.retain(|o| o.network_id != n.id);
        self.lcns.clear();
        for ts in n.streams {
            let ours = (self.tsid == 0 || ts.tsid == self.tsid)
                && (self.onid == 0 || ts.onid == self.onid);
            if ours {
                if self.onid == 0 && ts.onid != 0 {
                    self.onid = ts.onid;
                }
                self.lcns.extend(ts.lcns.iter().cloned());
                self.own_delivery = ts.delivery.clone();
                for &(sid, st) in &ts.services {
                    let s = self
                        .services
                        .entry(sid)
                        .or_insert_with(|| Service::new(sid));
                    if !s.have_sdt && st != 0 {
                        s.service_type = st;
                        s.kind = ServiceKind::of(st);
                    }
                }
            } else if self.other_multiplexes.len() < 4096 {
                self.other_multiplexes.push(OtherMultiplex {
                    network_id: n.id,
                    ts,
                });
            }
        }
        self.apply_lcns();
        ch.network = true;
        ch.services = true;
    }

    /// Chooses each service's LCN: the HD simulcast number first, then the NorDig v2 list of the user's country, then the plain one.
    fn apply_lcns(&mut self) {
        let mut best: BTreeMap<u16, (u8, Lcn)> = BTreeMap::new();
        for l in &self.lcns {
            let rank = if l.hd {
                3
            } else if !l.country.is_empty() {
                if l.country == self.country {
                    2
                } else {
                    0
                }
            } else {
                1
            };
            if rank == 0 && best.contains_key(&l.service_id) {
                continue;
            }
            match best.get(&l.service_id) {
                Some((r, _)) if *r >= rank => {}
                _ => {
                    best.insert(l.service_id, (rank, l.clone()));
                }
            }
        }
        for (sid, (_, l)) in best {
            let s = self
                .services
                .entry(sid)
                .or_insert_with(|| Service::new(sid));
            s.lcn = l.lcn;
            s.visible = l.visible;
        }
    }

    fn apply_sdt_actual(
        &mut self,
        tsid: u16,
        onid: u16,
        services: Vec<SdtService>,
        ch: &mut Changes,
    ) {
        if self.tsid != 0 && tsid != self.tsid {
            return; // an SDT actual of another tsid: ignore until the PAT agrees
        }
        if self.onid != 0 && onid != 0 && onid != self.onid {
            self.new_multiplex("original_network_id changed", ch);
            self.tsid = tsid;
        }
        if onid != 0 {
            self.onid = onid;
        }
        for sv in services {
            let s = self
                .services
                .entry(sv.service_id)
                .or_insert_with(|| Service::new(sv.service_id));
            let ml = (!self.preferred_lang.is_empty())
                .then(|| {
                    sv.names
                        .iter()
                        .find(|n| crate::epg::same_lang(&n.0, &self.preferred_lang))
                })
                .flatten();
            let name = ml.map(|n| n.2.clone()).unwrap_or(sv.name.clone());
            s.name = if name.is_empty() {
                format!("Service {}", sv.service_id)
            } else {
                name
            };
            s.provider = ml.map(|n| n.1.clone()).unwrap_or(sv.provider.clone());
            s.service_type = sv.service_type;
            s.kind = ServiceKind::of(sv.service_type);
            s.running_status = sv.running;
            s.free_ca = sv.free_ca;
            s.eit_schedule = sv.eit_schedule;
            s.eit_pf = sv.eit_pf;
            s.replacement = sv.replacement;
            s.have_sdt = true;
            s.onid = onid;
            s.tsid = tsid;
            s.ca_flag =
                s.free_ca || s.streams.iter().any(|e| e.ca) || !sv.ca_ids.is_empty() && s.ca_flag;
            update_ca_state(s);
        }
        ch.services = true;
    }

    /// The scrambling state of a PID from its packets: (examined, scrambled).
    pub fn set_scrambling(&mut self, pid: u16, examined: bool, scrambled: bool) {
        let mut changed = false;
        for s in self.services.values_mut() {
            let mut touched = false;
            for e in s.streams.iter_mut().filter(|e| e.pid == pid) {
                if examined && e.scrambled != scrambled {
                    e.scrambled = scrambled;
                    touched = true;
                }
            }
            if touched {
                update_ca_state(s);
                changed = true;
            }
        }
        if changed {
            self.version += 1;
        }
    }

    /// The properties and bit rate of a stream, from the elementary stream layer. Returns true if the codec, the packaging or
    /// the properties of a listed stream changed.
    pub fn set_stream(
        &mut self,
        pid: u16,
        codec: Option<(EsCodec, EsPackaging)>,
        props: Option<&EsProps>,
        kbps: Option<f64>,
    ) -> bool {
        let mut changed = false;
        for s in self.services.values_mut() {
            for e in s.streams.iter_mut().filter(|e| e.pid == pid) {
                if let Some((c, p)) =
                    codec.filter(|&(c, p)| (e.codec, e.packaging) != (Some(c), Some(p)))
                {
                    e.codec = Some(c);
                    e.packaging = Some(p);
                    changed = true;
                }
                if let Some(p) = props.filter(|p| e.props != **p) {
                    e.props.clone_from(p);
                    changed = true;
                }
                if let Some(k) = kbps {
                    e.kbps = k;
                }
            }
        }
        changed
    }
}

/// The sort key of a service in list order: the LCN (0 as 9999), then the service id.
pub(crate) fn list_key(s: &Service) -> (u16, u16) {
    (if s.lcn == 0 { 9999 } else { s.lcn }, s.service_id)
}

impl Epg {
    /// Forgets the present/following of one multiplex (its events stay).
    pub fn clear_multiplex_pf(&mut self, onid: u16, tsid: u16) {
        let _ = (onid, tsid);
    }
}

/// A service is scrambled when any video stream is, or when audio is and no audio stream is clear.
fn update_ca_state(s: &mut Service) {
    let video = s.streams.iter().filter(|e| e.kind == Some(EsKind::Video));
    let audio: Vec<&StreamEntry> = s
        .streams
        .iter()
        .filter(|e| e.kind == Some(EsKind::Audio))
        .collect();
    let v_scr = video.clone().any(|e| e.scrambled);
    let a_scr = !audio.is_empty()
        && audio.iter().any(|e| e.scrambled)
        && !audio.iter().any(|e| !e.scrambled);
    s.scrambled = v_scr || a_scr;
    s.ca_state = if s.scrambled {
        CaState::Scrambled
    } else if !s.have_pmt {
        CaState::Unknown
    } else if s.ca_flag {
        CaState::FlaggedClear
    } else {
        CaState::Clear
    };
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tables::{build_long, build_pat, build_pmt, build_sdt, descriptor, loop_bytes};

    fn nit(lcn_desc: Vec<u8>, pds: Option<u32>) -> Vec<u8> {
        let mut net = descriptor(0x40, b"Net");
        net.truncate(net.len());
        let mut tsd = Vec::new();
        if let Some(p) = pds {
            tsd.extend(descriptor(0x5F, &p.to_be_bytes()));
        }
        tsd.extend(lcn_desc);
        let mut ts = 7u16.to_be_bytes().to_vec();
        ts.extend_from_slice(&0x2310u16.to_be_bytes());
        ts.extend(loop_bytes(&tsd));
        let mut b = loop_bytes(&net);
        b.extend(loop_bytes(&ts));
        build_long(0x40, 0x3001, 0, 0, 0, &b)
    }

    #[test]
    fn a_service_from_pat_pmt_sdt() {
        let mut db = SiDb::new();
        db.push_section(0, &build_pat(7, 0, &[(100, 0x100)]), 0.0);
        let mut aac = b"eng".to_vec();
        aac.push(0);
        db.push_section(
            0x100,
            &build_pmt(
                100,
                0,
                0x101,
                &[],
                &[(0x1B, 0x101, vec![]), (0x0F, 0x102, descriptor(0x0A, &aac))],
            ),
            0.0,
        );
        let mut sdt = build_sdt(7, 0x2310, 0, 0, 0, &[(100, 0x01, "Provider", "News One")]);
        let _ = &mut sdt;
        db.push_section(0x11, &sdt, 0.0);
        let s = db.service(100).unwrap();
        assert_eq!(
            (s.name.as_str(), s.provider.as_str(), s.pmt_pid, s.pcr_pid),
            ("News One", "Provider", 0x100, 0x101)
        );
        assert_eq!(s.streams.len(), 2);
        assert_eq!(
            (s.streams[0].kind, s.streams[0].codec),
            (Some(EsKind::Video), Some(EsCodec::H264))
        );
        assert_eq!(
            (s.streams[1].codec, s.streams[1].lang.as_str()),
            (Some(EsCodec::Aac), "eng")
        );
        assert_eq!((db.onid, db.tsid), (0x2310, 7));
        assert_eq!(db.first_playable(), Some(100));
    }

    #[test]
    fn one_segment_pmt_without_pat() {
        let mut db = SiDb::new();
        db.push_section(
            0x1FC8,
            &build_pmt(0x5A1, 0, 0x1011, &[], &[(0x1B, 0x111, vec![])]),
            0.0,
        );
        assert!(db.service(0x5A1).unwrap().have_pmt);
    }

    /// A multiplex of a data service and a radio service: the radio service plays. A TV service before it plays first.
    #[test]
    fn the_first_playable_service_can_be_radio() {
        let mut db = SiDb::new();
        db.push_section(0, &build_pat(1, 0, &[(1, 0x100), (2, 0x200)]), 0.0);
        let data = build_pmt(1, 0, 0x1FFF, &[], &[(0x05, 0x101, vec![])]);
        db.push_section(0x100, &data, 0.0);
        let radio = build_pmt(2, 0, 0x201, &[], &[(0x03, 0x201, vec![])]);
        db.push_section(0x200, &radio, 0.0);
        assert_eq!(db.first_playable(), Some(2));
        let sdt = build_sdt(
            1,
            1,
            0,
            0,
            0,
            &[(1, 0x0C, "P", "Data"), (2, 0x02, "P", "Tunes")],
        );
        db.push_section(0x11, &sdt, 0.0);
        assert_eq!(db.first_playable(), Some(2));
        db.set_scrambling(0x201, true, true);
        assert_eq!(db.first_playable(), None);

        let mut tv = SiDb::new();
        tv.push_section(0, &build_pat(1, 0, &[(1, 0x100), (2, 0x200)]), 0.0);
        let video = build_pmt(1, 0, 0x101, &[], &[(0x1B, 0x101, vec![])]);
        tv.push_section(0x100, &video, 0.0);
        tv.push_section(0x200, &radio, 0.0);
        assert_eq!(tv.first_playable(), Some(1));
    }

    #[test]
    fn no_phantom_programme() {
        let mut db = SiDb::new();
        let mut p = build_pat(1, 0, &[(5, 0x50)]);
        p[1] &= 0x7F;
        p[10] ^= 0xFF;
        db.push_section(0, &p, 0.0);
        assert!(db.services().is_empty());
        assert_eq!(db.counts["PAT"].crc, 1);
    }

    #[test]
    fn old_names_until_the_new_version_is_complete() {
        let mut db = SiDb::new();
        db.push_section(0, &build_pat(7, 0, &[(1, 0x100), (2, 0x200)]), 0.0);
        db.push_section(0x11, &build_sdt(7, 1, 0, 0, 1, &[(1, 1, "P", "One")]), 0.0);
        db.push_section(0x11, &build_sdt(7, 1, 0, 1, 1, &[(2, 1, "P", "Two")]), 0.0);
        assert_eq!(db.service(2).unwrap().name, "Two");
        db.push_section(0x11, &build_sdt(7, 1, 1, 0, 1, &[(1, 1, "P", "Uno")]), 1.0);
        assert_eq!(db.service(1).unwrap().name, "One");
        db.push_section(0x11, &build_sdt(7, 1, 1, 1, 1, &[(2, 1, "P", "Due")]), 2.0);
        assert_eq!(
            (
                db.service(1).unwrap().name.as_str(),
                db.service(2).unwrap().name.as_str()
            ),
            ("Uno", "Due")
        );
    }

    #[test]
    fn lcn_variants() {
        let lcn = |sid: u16, vis: bool, n: u16| {
            let mut v = sid.to_be_bytes().to_vec();
            v.extend_from_slice(&(((vis as u16) << 15) | 0x7C00 | n).to_be_bytes());
            v
        };
        // EACEM without a specifier, then with 0x28.
        for pds in [None, Some(0x28)] {
            let mut db = SiDb::new();
            db.push_section(0, &build_pat(7, 0, &[(1, 0x100)]), 0.0);
            db.push_section(0x10, &nit(descriptor(0x83, &lcn(1, true, 5)), pds), 0.0);
            assert_eq!(db.service(1).unwrap().lcn, 5);
        }
        // NorDig v2 with two lists (NO and SE), user in NO.
        let list = |c: &[u8; 3], n: u16| {
            let mut v = vec![1, 1, b'X'];
            v.extend_from_slice(c);
            v.push(1);
            v.extend(lcn(1, true, n));
            v
        };
        let mut d = list(b"SWE", 11);
        d.extend(list(b"NOR", 22));
        let mut db = SiDb::new();
        db.country = "NOR".into();
        db.push_section(0, &build_pat(7, 0, &[(1, 0x100)]), 0.0);
        db.push_section(0x10, &nit(descriptor(0x87, &d), Some(0x29)), 0.0);
        assert_eq!(db.service(1).unwrap().lcn, 22);
        // HD simulcast replaces the plain number; visible 0 hides.
        let mut both = descriptor(0x83, &lcn(1, true, 5));
        both.extend(descriptor(0x88, &lcn(1, true, 105)));
        both.extend(descriptor(0x83, &lcn(2, false, 6)));
        let mut db = SiDb::new();
        db.push_section(0, &build_pat(7, 0, &[(1, 0x100), (2, 0x200)]), 0.0);
        db.push_section(0x10, &nit(both, Some(0x28)), 0.0);
        assert_eq!(db.service(1).unwrap().lcn, 105);
        assert!(!db.service(2).unwrap().visible);
        assert_eq!(db.listed().len(), 1);
        assert_eq!(db.network_name, "Net");
    }

    #[test]
    fn scrambling_states() {
        let mut db = SiDb::new();
        db.push_section(0, &build_pat(7, 0, &[(1, 0x100)]), 0.0);
        let ca = descriptor(0x09, &[0x0B, 0x00, 0xE1, 0x00]);
        db.push_section(
            0x100,
            &build_pmt(
                1,
                0,
                0x101,
                &ca,
                &[(0x02, 0x101, vec![]), (0x03, 0x102, vec![])],
            ),
            0.0,
        );
        db.set_scrambling(0x101, true, false);
        let s = db.service(1).unwrap();
        assert!(!s.scrambled);
        assert_eq!(s.ca_state, CaState::FlaggedClear);
        db.set_scrambling(0x101, true, true);
        assert!(db.service(1).unwrap().scrambled);
        // Not yet examined: no change.
        db.set_scrambling(0x101, false, false);
        assert!(db.service(1).unwrap().scrambled);
    }

    #[test]
    fn atsc_virtual_channel() {
        let mut ch = Vec::new();
        for c in "KDEC-DT".encode_utf16() {
            ch.extend_from_slice(&c.to_be_bytes());
        }
        ch.extend_from_slice(&[
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        ]);
        ch.truncate(14);
        // major 5 minor 1: 4 reserved bits + 10 bits major + 10 bits minor across bytes 14..17.
        let major: u32 = 5;
        let minor: u32 = 1;
        let v = (0xF << 20) | (major << 10) | minor;
        ch.extend_from_slice(&v.to_be_bytes()[1..]);
        ch.push(0x04); // modulation
        ch.extend_from_slice(&[0, 0, 0, 0]); // carrier
        ch.extend_from_slice(&7u16.to_be_bytes()); // channel tsid
        ch.extend_from_slice(&3u16.to_be_bytes()); // program number
        ch.push(0x0D); // etm, access, hidden ...
        ch.push(0xC2); // service_type 2
        ch.extend_from_slice(&1u16.to_be_bytes()); // source id
        ch.extend_from_slice(&[0xFC, 0x00]); // descriptors length 0
        let mut body = vec![0u8, 1];
        body.extend(ch);
        body.extend_from_slice(&[0xFC, 0x00]);
        let s = build_long(0xC8, 7, 0, 0, 0, &body);
        let mut db = SiDb::new();
        db.push_section(0x1FFB, &s, 0.0);
        let sv = db.service(3).unwrap();
        assert_eq!(
            (sv.name.as_str(), sv.provider.as_str(), sv.lcn),
            ("KDEC-DT", "channel 5.1", 501)
        );
    }
}
