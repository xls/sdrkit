//! Tracker recognition from the public structure of an advertisement, and the tracker book: how long and how near each
//! tracker stays, with rotating addresses joined, kept on signal time only.

use super::layout::{self, ServiceData, SmartTagAdvert, TagDetail, TagMode};
use sdrkit_proto_wlan_phy::ble::ad::{self, AdValue};
use sdrkit_proto_wlan_phy::ble::{Addr, AdvPdu, PduType};
use sdrkit_util_objects::{fingerprint, LogRing, ObjectTable, Shift, Stamp};
use serde::{Deserialize, Serialize};

sdrkit_core::choice_enum! {
    /// The kinds of tracker that the detector recognizes. The label is the full name.
    #[derive(Serialize, Deserialize)]
    pub enum TrackerKind {
        /// Apple Find My.
        FindMy = ("find_my", "Apple Find My (AirTag or compatible)"),
        /// Tile.
        Tile = ("tile", "Tile"),
        /// Chipolo.
        Chipolo = ("chipolo", "Chipolo"),
        /// Samsung SmartTag.
        SmartTag = ("smart_tag", "Samsung SmartTag"),
        /// Google Find My Device.
        Google = ("google", "Google Find My Device tag"),
    }
}

impl TrackerKind {
    /// Every kind, in order.
    pub const ALL: [TrackerKind; 5] = [
        TrackerKind::FindMy,
        TrackerKind::Tile,
        TrackerKind::Chipolo,
        TrackerKind::SmartTag,
        TrackerKind::Google,
    ];
    /// The short name.
    pub fn short(self) -> &'static str {
        match self {
            TrackerKind::FindMy => "Find My",
            TrackerKind::Tile => "Tile",
            TrackerKind::Chipolo => "Chipolo",
            TrackerKind::SmartTag => "SmartTag",
            TrackerKind::Google => "Google",
        }
    }
    /// The full name.
    pub fn full(self) -> &'static str {
        match self {
            TrackerKind::FindMy => "Apple Find My (AirTag or compatible)",
            TrackerKind::Tile => "Tile",
            TrackerKind::Chipolo => "Chipolo",
            TrackerKind::SmartTag => "Samsung SmartTag",
            TrackerKind::Google => "Google Find My Device tag",
        }
    }
    /// The place of the kind in [`TrackerKind::ALL`], 0 to 4.
    pub fn slot(self) -> usize {
        usize::from(sdrkit_core::Choice::index(self))
    }
}

/// What the detector reads from one advertisement.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Recognised {
    /// The kind.
    pub kind: TrackerKind,
    /// Separated from its owner (Find My).
    pub separated: bool,
    /// The status byte (Find My).
    pub status: Option<u8>,
    /// Unwanted-tracking protection (Google frame type 0x41).
    pub unwanted_protection: bool,
    /// The mode, the SmartTag fields and the service data (Tile, Chipolo, SmartTag).
    pub detail: TagDetail,
}

fn uuid_kind(u: u32) -> Option<TrackerKind> {
    match u16::try_from(u).ok()? {
        layout::TILE_UUID | layout::TILE_SHIPPING_UUID | layout::TILE_OTHER_UUID => {
            Some(TrackerKind::Tile)
        }
        layout::CHIPOLO_UUID => Some(TrackerKind::Chipolo),
        layout::SMARTTAG_UUID | layout::SMARTTAG_UNREGISTERED_UUID => Some(TrackerKind::SmartTag),
        _ => None,
    }
}

fn plain(kind: TrackerKind) -> Recognised {
    Recognised {
        kind,
        separated: false,
        status: None,
        unwanted_protection: false,
        detail: TagDetail::default(),
    }
}

/// Recognises a tracker from advertising data: company identifier, service UUID, frame type and status, and for Tile,
/// Chipolo and SmartTag the mode that the UUID tells and the service data. The first structure that names a kind sets
/// it, later structures of the same kind add their detail, and a Find My frame always sets Find My.
pub fn recognise(data: &[u8]) -> Option<Recognised> {
    let mut found: Option<Recognised> = None;
    for s in ad::structures(data) {
        let this = match s.value() {
            // The offline finding frame: type 0x12, length 25 (0x19), then the status byte.
            AdValue::Manufacturer {
                company: 0x004C,
                data: m,
            } if m.len() >= 27 && m[0] == 0x12 && m[1] == 25 => {
                found = Some(Recognised {
                    kind: TrackerKind::FindMy,
                    separated: true,
                    status: Some(m[2]),
                    ..plain(TrackerKind::FindMy)
                });
                continue;
            }
            // The first UUID of a family names the kind; any UUID of that kind with a mode gives the mode.
            AdValue::Uuids { list, .. } if list.width == 2 => {
                list.values().find_map(uuid_kind).map(|k| Recognised {
                    detail: TagDetail {
                        mode: list
                            .values()
                            .filter(|&u| uuid_kind(u) == Some(k))
                            .find_map(|u| layout::mode_of(u16::try_from(u).ok()?)),
                        ..TagDetail::default()
                    },
                    ..plain(k)
                })
            }
            AdValue::ServiceData16 { uuid, data: d } => match uuid_kind(u32::from(uuid)) {
                Some(k) => Some(Recognised {
                    detail: layout::detail_of_service(uuid, d),
                    ..plain(k)
                }),
                None if uuid == 0xFEAA && matches!(d.first(), Some(0x40 | 0x41)) => {
                    Some(Recognised {
                        unwanted_protection: d[0] == 0x41,
                        ..plain(TrackerKind::Google)
                    })
                }
                None => None,
            },
            _ => None,
        };
        match (&mut found, this) {
            (None, t) => found = t,
            (Some(f), Some(t)) if f.kind == t.kind => f.detail.merge(&t.detail),
            _ => {}
        }
    }
    found
}

/// A tracker sighting (the `ble.tracker_sighting` stream). The address stays in memory only.
#[derive(Clone, Debug, PartialEq)]
pub struct Sighting {
    /// The advertiser address.
    pub addr: Addr,
    /// What was recognised.
    pub what: Recognised,
    /// Level, dBFS.
    pub level_dbfs: f64,
    /// Signal time, s.
    pub time_s: f64,
    /// Wall time, when the source is live.
    pub wall: Option<f64>,
}

impl sdrkit_graph::Item for Sighting {
    const KIND: &'static str = "ble.tracker_sighting";
}

/// Makes a sighting from a parsed PDU: only ADV_IND, ADV_NONCONN_IND, ADV_SCAN_IND and SCAN_RSP with an AdvA and data count.
pub fn sighting(
    p: &AdvPdu<'_>,
    level_dbfs: f64,
    time_s: f64,
    wall: Option<f64>,
) -> Option<Sighting> {
    if !matches!(
        p.header.pdu_type,
        PduType::AdvInd | PduType::AdvNonconnInd | PduType::ScanRsp | PduType::AdvScanInd
    ) {
        return None;
    }
    let data = p.data().filter(|d| !d.is_empty())?;
    Some(Sighting {
        addr: p.adv_a()?,
        what: recognise(data)?,
        level_dbfs,
        time_s,
        wall,
    })
}

/// The book's configuration.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BookConfig {
    /// How long a tracker must stay, s (at least 1).
    pub alert_after_s: f64,
    /// Averaged level counted as near, dBFS.
    pub near_dbfs: f64,
    /// An identifier unheard this long is dropped, s.
    pub forget_s: f64,
    /// Least share of the minutes heard.
    pub min_presence: f64,
    /// Least share of the seconds near.
    pub min_near: f64,
    /// Rotation: the longest silence of the old address before the new one.
    pub link_gap_s: f64,
    /// Rotation: the level difference allowed, dB.
    pub link_db: f64,
}

impl Default for BookConfig {
    fn default() -> Self {
        BookConfig {
            alert_after_s: 600.0,
            near_dbfs: -60.0,
            forget_s: 1800.0,
            min_presence: 0.6,
            min_near: 0.5,
            link_gap_s: 20.0,
            link_db: 8.0,
        }
    }
}

/// Most trackers kept.
pub const RECORD_CAP: usize = 200;
/// Most alerts kept.
pub const ALERT_CAP: usize = 50;
/// The windows of the level history, s.
const WINDOW_S: f64 = 10.0;
/// Windows kept in the level history.
const HISTORY: usize = 24;

/// One tracker. Its row in the book is keyed by the address it uses now.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Tracker {
    /// The id ([`id_of`] the address it uses now).
    pub id: u32,
    /// The last two address bytes ("31:5A").
    pub tail: String,
    /// `"<short kind> ..<tail>"`, made when the tail or the kind changes.
    label: String,
    /// The kind.
    pub kind: Option<TrackerKind>,
    /// Separated from its owner (it stays set).
    pub separated: bool,
    /// The last status byte.
    pub status: Option<u8>,
    /// Unwanted-tracking protection seen.
    pub unwanted_protection: bool,
    /// The last mode that the UUID told (Tile, Chipolo, SmartTag).
    pub mode: Option<TagMode>,
    /// The last decoded SmartTag value.
    pub smarttag: Option<SmartTagAdvert>,
    /// The last rolling identifier in hex (the SmartTag privacy ID); empty when none was heard.
    pub identifier: String,
    /// The last service data value under the family's UUID in hex; empty when none was heard.
    pub service_data: String,
    last_data: Option<ServiceData>,
    /// The last level, dBFS.
    pub level: f64,
    /// The average level, dBFS.
    pub avg: f64,
    /// The peak level, dBFS.
    pub peak: f64,
    /// Sightings.
    pub packets: u64,
    /// First heard, signal time (with every address it used).
    pub first_s: f64,
    /// Last heard, signal time.
    pub last_s: f64,
    /// First heard, wall time.
    pub first_wall: Option<f64>,
    /// Addresses it used.
    pub addresses: u32,
    /// Seconds heard.
    pub secs: u64,
    /// Seconds heard while near.
    pub near_secs: u64,
    last_sec: i64,
    /// Minutes heard.
    pub minutes: u64,
    first_min: i64,
    last_min: i64,
    /// The mean level of each 10 s window, oldest first.
    pub history: Vec<f64>,
    win: i64,
    win_sum: f64,
    win_n: u32,
    /// Marked as the user's own.
    pub mine: bool,
    /// It raised its alert.
    pub alerted: bool,
    /// It follows the radio now.
    pub following: bool,
    /// 2 near, 1 medium, 0 far.
    pub proximity: u8,
    /// +1 closer, -1 farther, 0 steady.
    pub trend: i8,
    /// The share of the seconds heard while near.
    pub near_share: f64,
    /// The share of the minutes heard.
    pub presence: f64,
}

impl Shift for Tracker {
    fn shift_times(&mut self, by_s: f64) {
        self.first_s += by_s;
        self.last_s += by_s;
    }
}

impl Tracker {
    /// Keeps the parts of `d` that it holds; the hex text is made again only when its bytes changed.
    fn take_detail(&mut self, d: &TagDetail) {
        if d.mode.is_some() {
            self.mode = d.mode;
        }
        if let Some(t) = d.smarttag {
            if self.smarttag.is_none_or(|o| o.privacy_id != t.privacy_id) {
                self.identifier.clear();
                sdrkit_core::base64::hex_into(&t.privacy_id, &mut self.identifier);
            }
            self.smarttag = Some(t);
        }
        if let Some(v) = d.service_data {
            if self.last_data != Some(v) {
                self.service_data.clear();
                sdrkit_core::base64::hex_into(v.as_slice(), &mut self.service_data);
                self.last_data = Some(v);
            }
        }
    }
    /// `"<short kind> ..<last two address bytes>"`.
    pub fn label(&self) -> &str {
        &self.label
    }
    /// The state, in order of priority.
    pub fn state(&self) -> TrackerState {
        if self.mine {
            TrackerState::Mine
        } else if self.following {
            TrackerState::Following
        } else if self.separated {
            TrackerState::Separated
        } else {
            TrackerState::Nearby
        }
    }
    /// How long it has stayed, s.
    pub fn stays(&self) -> f64 {
        self.last_s - self.first_s
    }
    /// "near", "medium" or "far".
    pub fn proximity_text(&self) -> &'static str {
        ["far", "medium", "near"][self.proximity.min(2) as usize]
    }
    /// Whether the level rises, falls or stays.
    pub fn trend(&self) -> Trend {
        match self.trend {
            1 => Trend::Closer,
            -1 => Trend::Farther,
            _ => Trend::Steady,
        }
    }
}

sdrkit_core::choice_enum! {
    /// The state of a tracker.
    pub enum TrackerState {
        /// Marked as the user's own.
        Mine = ("mine", "your own"),
        /// It follows the radio.
        Following = ("following", "following you"),
        /// It said that it is separated from its owner.
        Separated = ("separated", "separated from its owner"),
        /// Heard nearby.
        Nearby = ("nearby", "nearby"),
    }
}

sdrkit_core::choice_enum! {
    /// How the level of a tracker moves over the last minute.
    pub enum Trend {
        /// The level rises by 3 dB or more.
        Closer = ("closer", "closer"),
        /// The level falls by 3 dB or more.
        Farther = ("farther", "farther"),
        /// The level stays within 3 dB.
        Steady = ("steady", "steady"),
    }
}

/// An unwanted tracking alert.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Alert {
    /// Wall time, when the source is live.
    pub wall: Option<f64>,
    /// Signal time, s.
    pub time_s: f64,
    /// The kind, if known.
    pub kind: Option<TrackerKind>,
    /// The label of the tracker ("Find My ..31:5A").
    pub label: String,
    /// The tracker said that it is separated from its owner.
    pub separated: bool,
    /// Minutes it stayed.
    pub minutes: f64,
    /// Average level, dBFS.
    pub level_dbfs: f64,
    /// The text.
    pub text: String,
}

impl Shift for Alert {
    fn shift_times(&mut self, by_s: f64) {
        self.time_s += by_s;
    }
}

/// The id of a tracker that uses address `text`: the low 32 bits of its FNV-1a fingerprint, never 0.
pub fn id_of(text: &str) -> u32 {
    match fingerprint(text.as_bytes()) as u32 {
        0 => 1,
        h => h,
    }
}

/// The tracker book.
#[derive(Clone, Debug)]
pub struct Book {
    /// The configuration.
    pub cfg: BookConfig,
    /// The trackers, keyed by the address each uses now.
    pub trackers: ObjectTable<String, Tracker>,
    /// The alerts.
    pub alerts: LogRing<Alert>,
    new_alerts: Vec<Alert>,
    /// Sightings taken.
    pub sightings: u64,
    /// The book's clock (the last housekeeping), s.
    pub now: f64,
    last_wall: Option<f64>,
}

impl Default for Book {
    fn default() -> Self {
        Book::new(BookConfig::default())
    }
}

impl Book {
    /// A book with a configuration.
    pub fn new(cfg: BookConfig) -> Book {
        Book {
            cfg,
            trackers: ObjectTable::new(RECORD_CAP),
            alerts: LogRing::new(ALERT_CAP),
            new_alerts: Vec::new(),
            sightings: 0,
            last_wall: None,
            now: 0.0,
        }
    }

    /// The record that a new address continues, when it is a rotation of a tracker heard before: the same kind, heard again
    /// within the link gap, at a level within the link difference.
    fn continued(&self, s: &Sighting) -> Option<String> {
        let now = s.time_s;
        let cand = self
            .trackers
            .iter()
            .filter(|r| {
                r.record.kind == Some(s.what.kind)
                    && now - r.record.last_s <= self.cfg.link_gap_s
                    && (r.record.avg - s.level_dbfs).abs() <= self.cfg.link_db
            })
            .max_by(|a, b| a.record.last_s.total_cmp(&b.record.last_s))?;
        // An old address heard less than 1 s ago still talks: the newcomer is a second tag.
        (now - cand.record.last_s >= 1.0).then(|| cand.key.clone())
    }

    /// Hears a sighting.
    pub fn hear(&mut self, s: &Sighting) {
        self.sightings += 1;
        self.last_wall = s.wall;
        let now = s.time_s;
        let text = s.addr.text();
        let at = match s.wall {
            Some(w) => Stamp::at(now).with_wall(w),
            None => Stamp::at(now),
        };
        let mut moved = None;
        if !self.trackers.contains(text.as_str()) {
            if let Some(old) = self.continued(s) {
                if let Some(e) = self.trackers.remove(old.as_str()) {
                    let mut r = e.record;
                    r.addresses += 1;
                    moved = Some(r);
                }
            }
        }
        let fresh = || Tracker {
            first_s: now,
            first_wall: s.wall,
            avg: s.level_dbfs,
            peak: s.level_dbfs,
            last_sec: i64::MIN,
            last_min: i64::MIN,
            first_min: (now / 60.0).floor() as i64,
            win: (now / WINDOW_S).floor() as i64,
            ..Default::default()
        };
        let Ok(up) = self
            .trackers
            .upsert(text.as_str(), at, || moved.unwrap_or_else(fresh))
        else {
            return;
        };
        let near = self.cfg.near_dbfs;
        let r = up.row;
        r.id = id_of(&text);
        r.tail = text
            .get(text.len().saturating_sub(5)..)
            .unwrap_or("")
            .to_string();
        if r.kind != Some(s.what.kind) || !r.label.ends_with(r.tail.as_str()) || r.label.is_empty()
        {
            r.label = format!("{} ..{}", s.what.kind.short(), r.tail);
        }
        if r.kind.is_some_and(|k| k != s.what.kind) {
            r.mode = None;
            r.smarttag = None;
            r.identifier.clear();
            r.service_data.clear();
            r.last_data = None;
        }
        r.take_detail(&s.what.detail);
        r.kind = Some(s.what.kind);
        r.separated |= s.what.separated;
        r.unwanted_protection |= s.what.unwanted_protection;
        if s.what.status.is_some() {
            r.status = s.what.status;
        }
        r.level = s.level_dbfs;
        if r.packets > 0 || r.addresses > 0 {
            r.avg += 0.1 * (s.level_dbfs - r.avg);
        }
        if r.addresses == 0 {
            r.addresses = 1;
        }
        r.peak = r.peak.max(s.level_dbfs);
        r.packets += 1;
        r.last_s = now;
        let sec = now.floor() as i64;
        if sec != r.last_sec {
            r.last_sec = sec;
            r.secs += 1;
            if r.avg >= near {
                r.near_secs += 1;
            }
        }
        let min = (now / 60.0).floor() as i64;
        if min != r.last_min {
            r.last_min = min;
            r.minutes += 1;
        }
        let w = (now / WINDOW_S).floor() as i64;
        if w != r.win {
            if r.win_n > 0 {
                let m = r.win_sum / r.win_n as f64;
                crate::report::push_history(&mut r.history, m, HISTORY);
            }
            r.win = w;
            r.win_sum = 0.0;
            r.win_n = 0;
        }
        r.win_sum += s.level_dbfs;
        r.win_n += 1;
    }

    /// Housekeeping at a report: forgets trackers unheard for `forget_s`, then judges every tracker and raises the alerts.
    pub fn housekeeping(&mut self, now: f64) {
        self.now = now;
        self.trackers.expire(now, self.cfg.forget_s);
        let cfg = self.cfg;
        for (_, r) in self.trackers.records_mut() {
            r.proximity = if r.avg >= cfg.near_dbfs {
                2
            } else if r.avg >= cfg.near_dbfs - 6.0 {
                1
            } else {
                0
            };
            r.near_share = if r.secs > 0 {
                r.near_secs as f64 / r.secs as f64
            } else {
                0.0
            };
            r.presence = (r.minutes as f64 / (r.last_min - r.first_min + 1).max(1) as f64).min(1.0);
            r.trend = 0;
            let n = r.history.len();
            if n >= 4 {
                let k = 3.min(n / 2);
                let newest: f64 = r.history[n - k..].iter().sum::<f64>() / k as f64;
                let before: f64 = r.history[n - 2 * k..n - k].iter().sum::<f64>() / k as f64;
                let d = newest - before;
                r.trend = if d >= 3.0 {
                    1
                } else if d <= -3.0 {
                    -1
                } else {
                    0
                };
            }
            r.following = !r.mine
                && now - r.last_s <= 60.0
                && r.stays() >= cfg.alert_after_s
                && r.presence >= cfg.min_presence
                && r.near_share >= cfg.min_near;
            if r.following && !r.alerted {
                r.alerted = true;
                let minutes = r.stays() / 60.0;
                let kind = r.kind.map(|k| k.full()).unwrap_or("unknown");
                let a = Alert {
                    wall: r.first_wall.and(self.last_wall),
                    time_s: now,
                    kind: r.kind,
                    label: r.label.clone(),
                    separated: r.separated,
                    minutes,
                    level_dbfs: r.avg,
                    text: format!(
                        "Unwanted tracking: a {kind} tracker has stayed near you for {minutes:.0} min ({} {})",
                        r.addresses,
                        if r.addresses == 1 { "address" } else { "addresses" }
                    ),
                };
                self.new_alerts.push(a.clone());
                self.alerts.push(a);
            }
        }
    }

    /// The alerts raised since the last call.
    pub fn take_new_alerts(&mut self) -> Vec<Alert> {
        std::mem::take(&mut self.new_alerts)
    }

    /// Marks (or unmarks) the tracker `id` as the user's own. Returns false when no tracker has that id.
    pub fn ignore(&mut self, id: u32, on: bool) -> bool {
        match self.trackers.records_mut().find(|(_, r)| r.id == id) {
            Some((_, r)) => {
                r.mine = on;
                if on {
                    r.following = false;
                }
                true
            }
            None => false,
        }
    }

    /// Forgets every tracker and alert.
    pub fn clear(&mut self) {
        self.trackers.clear_all();
        self.alerts.clear();
        self.new_alerts.clear();
        self.sightings = 0;
    }

    /// Trackers heard in the last 60 s, by kind.
    pub fn now_by_kind(&self) -> [u32; 5] {
        let mut c = [0; 5];
        for r in self.trackers.iter() {
            if let Some(k) = r.record.kind {
                if self.now - r.record.last_s <= 60.0 {
                    c[k.slot()] += 1;
                }
            }
        }
        c
    }

    /// Trackers in the book, by kind.
    pub fn window_by_kind(&self) -> [u32; 5] {
        let mut c = [0; 5];
        for r in self.trackers.iter() {
            if let Some(k) = r.record.kind {
                c[k.slot()] += 1;
            }
        }
        c
    }

    /// Trackers that follow the radio now.
    pub fn following_now(&self) -> usize {
        self.trackers.iter().filter(|r| r.record.following).count()
    }

    /// The ids of the trackers in table order, into `out`: following first, then by how long each stayed, longest first.
    /// `out` is cleared first and keeps its memory.
    pub fn order_into(&self, out: &mut Vec<(bool, f64, u32)>) {
        out.clear();
        out.extend(
            self.trackers
                .iter()
                .map(|r| (r.record.following, r.record.stays(), r.record.id)),
        );
        out.sort_by(|a, b| b.0.cmp(&a.0).then(b.1.total_cmp(&a.1)));
    }

    /// The tracker with id `id`.
    pub fn by_id(&self, id: u32) -> Option<&Tracker> {
        self.trackers.iter().map(|r| r.record).find(|t| t.id == id)
    }
}
