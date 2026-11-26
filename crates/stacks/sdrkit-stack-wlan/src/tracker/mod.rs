//! The BLE tracker detector (`bletracker`): recognises item trackers from their public advertisements and raises an unwanted
//! tracking alert for one that stays with the receiver. Identifiers stay in memory only and are never saved. The stack sends
//! the Find My, Google, Tile, Chipolo and SmartTag frames that the detector reads ([`crate::ble::tx`]) into a transmit sink.

pub mod book;
pub mod layout;

use crate::ble::gen::{tracker_schedule, BleGenParams, BleGenerator};
use crate::ble::{self, AdvChannel, AdvStats, Advert};
use crate::report::{remove_older_rows, wall};
use crate::testgen::{route_to, GEN_SECTION};
use crate::typed::{
    BletrackerAlertEvent, BletrackerAlertRow, BletrackerFields, BletrackerKindRow,
    BletrackerMineArgs, BletrackerTrackerRow, BLETRACKER_ALERT, BLETRACKER_ALERTS,
    BLETRACKER_KINDS, BLETRACKER_MINE, BLETRACKER_TRACKERS,
};
use book::{Alert, Book, BookConfig, TrackerKind};
use sdrkit_core::command::{CommandError, CommandId, CommandSet, Commands};
use sdrkit_core::interface::{Fields as _, Hertz, Seconds, Time};
use sdrkit_core::publish::keys::{Family, Urgency};
use sdrkit_core::publish::{field, Published};
use sdrkit_core::SignalType;
use sdrkit_core::{
    AutoGain, Choice, Clock, ConfigError, Event, GenConfig, Generator, HostRequest, LockProfile,
    LockState, Receiver, Record, Results, Setting, SettingError, SettingId, SettingSet,
    StackConfig, StackDescriptor, Status, TableId, Tuning, Value, Values,
};
use sdrkit_graph::{
    values_of, Applied, Catalog, Component, ComponentInfo, Format, Graph, GraphError, Io, Kept,
    Kind, Port, Registry, RunContext, StackGraph, Timed,
};
use sdrkit_proto_wlan_phy::ble::channel_hz;
use sdrkit_util_objects::commands as objcmd;
use sdrkit_util_objects::shaped::{self, RowOut, Shaped};
use sdrkit_util_objects::Seen;
use std::fmt::Write as _;
use std::sync::Arc;

/// The stack id.
pub const ID: &str = "bletracker";
/// The note on what the detector can and cannot tell.
pub const LIMITS_NOTE: &str = "A tracker that stays near a fixed radio, such as a neighbour's tag, is reported too, and the levels are the radio's own, not calibrated distances. The detector is meant for a radio that travels with you.";
/// The privacy note of the detector.
pub const TRACKER_PRIVACY_NOTE: &str = "Item trackers are recognised from their public advertisements. Nothing is ever connected to, sent to, or read from a tracker's owner. Tracker advertisements go out only on the command send. Identifiers stay in memory for 30 minutes after a tracker was last heard and are never saved. The table shows the last two address bytes, and the rolling identifier and service data that a tracker broadcasts as hex.";

/// The advertising channel to listen on. The host tunes to it.
pub const CHANNEL_SETTING: Setting<field::Choice<AdvChannel>> =
    Setting::choice("channel", "Channel", AdvChannel::C37).help(
        "Trackers advertise on all three channels in turn, so any one of them hears every tracker.",
    );
/// How long a tracker must stay before the alert, minutes.
pub const ALERT_AFTER_MIN: Setting<field::F64> =
    Setting::new("alert_after_min", "Alert after", "min", 10.0)
        .range(1.0, 60.0, 1.0)
        .help("How long a tracker must stay with the radio before the alert.");
/// The averaged level from which a tracker counts as near.
pub const NEAR: Setting<field::F64> = Setting::new("near_dbfs", "Near above", "dBFS", -60.0)
    .range(-100.0, -20.0, 1.0)
    .help("The averaged level from which a tracker counts as near. The radio's level, not a calibrated distance.");
/// The alert time in seconds, for tests. 0 uses [`ALERT_AFTER_MIN`].
pub const ALERT_AFTER_S: Setting<field::Duration> = Setting::new(
    "alert_after_s",
    "Alert after (seconds, for tests)",
    "s",
    0.0,
)
.range(0.0, 7200.0, 0.0)
.help("When above 0, replaces the alert time in minutes.")
.section("Test");

sdrkit_core::settings! {
    /// The settings of [`TrackerBook`].
    pub enum TrackerBookSetting {
        /// The advertising channel.
        Channel = CHANNEL_SETTING,
        /// The alert time in minutes.
        AlertAfterMin = ALERT_AFTER_MIN,
        /// The near level.
        Near = NEAR,
        /// The alert time in seconds.
        AlertAfterS = ALERT_AFTER_S,
    }
}

sdrkit_core::choice_enum! {
    /// The choices of the test-signal setting `gen_schedule`.
    pub enum GenSchedule {
        /// Every tracker and a phone.
        All = ("all", "Every tracker and a phone"),
        /// Only the Find My frame.
        FindMy = ("find_my", "Only the Find My frame"),
    }
}

/// The test-signal setting `gen_schedule`: which packets the test signal sends.
pub const GEN_SCHEDULE: Setting<field::Choice<GenSchedule>> =
    Setting::choice("gen_schedule", "Schedule", GenSchedule::All)
        .section(GEN_SECTION)
        .advanced();

sdrkit_core::settings! {
    /// The settings of the `bletracker` stack: the book's and the test signal's.
    pub enum TrackerSetting {
        /// The advertising channel.
        Channel = CHANNEL_SETTING,
        /// The alert time in minutes.
        AlertAfterMin = ALERT_AFTER_MIN,
        /// The near level.
        Near = NEAR,
        /// The alert time in seconds.
        AlertAfterS = ALERT_AFTER_S,
        /// The level sent.
        TxLevel = ble::tx::TX_LEVEL,
        /// The schedule of the test signal.
        GenSchedule = GEN_SCHEDULE,
    }
}

sdrkit_core::commands! {
    /// The commands of [`TrackerBook`].
    pub enum TrackerBookCommand {
        /// Forget every tracker and alert.
        Clear = objcmd::CLEAR,
        /// Mark a tracker as the user's own.
        Mine = BLETRACKER_MINE.command,
    }
}

impl Shaped for Alert {
    type Row = BletrackerAlertRow<'static>;
    fn row(&self, _seen: Option<&Seen>, _now_s: f64, out: RowOut<'_, '_, Self::Row>) {
        out.put(&BletrackerAlertRow {
            time: self.wall.map(Time),
            signal_s: self.time_s,
            level_dbfs: self.level_dbfs,
            alert: &self.text,
        });
    }
}

/// The tracker book as a component (`wlan.ble_trackers`): advertising packets and the receiver's counters in, the alert
/// events out. Its clock never restarts, so dwell times survive a restart of the radio.
pub struct TrackerBook {
    /// The book.
    pub book: Book,
    published: Published,
    pending: BookConfig,
    alert_after_min: f64,
    alert_after_s: f64,
    stats: AdvStats,
    base: AdvStats,
    sightings_base: u64,
    snr_db: Option<f64>,
    offset_hz: Option<f64>,
    last_good_s: Option<f64>,
    level_dbfs: Option<f64>,
    events: Vec<Event>,
    requests: Vec<HostRequest>,
    status: Status,
    key: String,
    trackers_table: TableId,
    order: Vec<(bool, f64, u32)>,
    channel: AdvChannel,
    clock_base: f64,
    now_s: f64,
    live: bool,
    clock: Clock,
    configured: bool,
}

impl Default for TrackerBook {
    fn default() -> Self {
        let published = Published::new(BletrackerFields::SHAPE.fields).with_tables(&[
            BLETRACKER_TRACKERS.def(),
            BLETRACKER_ALERTS.def(),
            BLETRACKER_KINDS.def(),
        ]);
        let trackers_table = published
            .results()
            .decl()
            .table_id(BLETRACKER_TRACKERS.key)
            .unwrap_or_else(|| unreachable!("the tracker table is declared"));
        let mut t = TrackerBook {
            book: Book::default(),
            published,
            trackers_table,
            order: Vec::new(),
            pending: BookConfig {
                near_dbfs: NEAR.default,
                ..BookConfig::default()
            },
            alert_after_min: ALERT_AFTER_MIN.default,
            alert_after_s: ALERT_AFTER_S.default,
            stats: AdvStats::default(),
            base: AdvStats::default(),
            sightings_base: 0,
            snr_db: None,
            offset_hz: None,
            last_good_s: None,
            level_dbfs: None,
            events: Vec::new(),
            requests: Vec::new(),
            status: Status::default(),
            key: String::new(),
            channel: CHANNEL_SETTING.default,
            clock_base: 0.0,
            now_s: 0.0,
            live: false,
            clock: Clock::fixed(0.0),
            configured: false,
        };
        t.update_alert_time();
        t
    }
}

impl TrackerBook {
    fn update_alert_time(&mut self) {
        self.pending.alert_after_s = if self.alert_after_s > 0.0 {
            self.alert_after_s.max(1.0)
        } else {
            self.alert_after_min * 60.0
        };
    }

    fn take(&mut self, a: &Advert) {
        let t = self.clock_base + a.time_s;
        self.snr_db = Some(match self.snr_db {
            None => a.snr_db,
            Some(s) => s + 0.2 * (a.snr_db - s),
        });
        self.offset_hz = Some(a.offset_hz);
        self.last_good_s = Some(t);
        if let Some(p) = a.parsed() {
            if let Some(s) = book::sighting(&p, a.level_dbfs, t, wall(self.live, &self.clock)) {
                self.book.hear(&s);
            }
        }
    }

    fn alert_event(&self, a: &Alert) -> Event {
        let freq = channel_hz(self.channel.number()).unwrap_or(2402e6);
        let urgency = if a.separated {
            Urgency::Urgent
        } else {
            Urgency::Notice
        };
        BLETRACKER_ALERT.event(
            a.time_s,
            freq,
            &BletrackerAlertEvent {
                source: &a.label,
                kind: a.kind,
                text: &a.text,
                stays: Seconds(a.minutes * 60.0),
                level_dbfs: a.level_dbfs,
                urgency,
                family: Family::Alert,
            },
        )
    }

    /// Housekeeping and publication.
    pub fn publish(&mut self) {
        let now = self.now_s;
        self.book.cfg = self.pending;
        self.book.housekeeping(now);
        for a in self.book.take_new_alerts() {
            self.published.results_mut().log(a.text.clone());
            let e = self.alert_event(&a);
            self.events.push(e);
        }
        let b = &self.book;
        let good = self.stats.packets.saturating_sub(self.base.packets);
        let bad = self
            .stats
            .crc_failures
            .saturating_sub(self.base.crc_failures);
        let following = b.following_now();
        let now_k = b.now_by_kind();
        let win_k = b.window_by_kind();
        let heard_now: u32 = now_k.iter().sum();
        let active = self
            .last_good_s
            .is_some_and(|l| now - l <= ble::table::ACTIVE_S);
        let channel = self.stats.channel;
        let plural = |n: usize| if n == 1 { "tracker" } else { "trackers" };
        let st = &mut self.status;
        st.lock_state = if active {
            LockState::Locked
        } else {
            LockState::Searching
        };
        st.snr_db = self.snr_db.map(|s| s as f32);
        st.level_dbfs = self.level_dbfs.map(|l| l as f32);
        st.offset_hz = self.offset_hz;
        st.summary.clear();
        // Writing to a String cannot fail.
        let _ = if good == 0 {
            if channel != 0 {
                write!(st.summary, "listening on channel {channel}")
            } else {
                write!(st.summary, "listening for advertising")
            }
        } else if following > 0 {
            write!(
                st.summary,
                "{following} {} following you",
                plural(following)
            )
        } else if heard_now == 0 {
            write!(st.summary, "no trackers nearby")
        } else {
            write!(
                st.summary,
                "{heard_now} {} nearby",
                plural(heard_now as usize)
            )
        };
        let sightings = b.sightings.saturating_sub(self.sightings_base);
        let p = &mut self.published;
        p.results_mut().set_status(st);
        p.put(&BletrackerFields {
            following_now: following as i64,
            tracker_packets: sightings as i64,
            trackers_now: i64::from(heard_now),
            trackers: b.trackers.len() as i64,
            alert_after: Seconds(b.cfg.alert_after_s),
            near_dbfs: b.cfg.near_dbfs,
            now_s: now,
            frames_ok: good as i64,
            frames_bad: bad as i64,
            snr_db: self.snr_db,
            offset_hz: self.offset_hz.map(Hertz),
            level_dbfs: self.level_dbfs,
            channel: (channel != 0).then_some(i64::from(channel)),
            limits: LIMITS_NOTE,
            privacy: TRACKER_PRIVACY_NOTE,
        });
        for k in TrackerKind::ALL {
            BLETRACKER_KINDS.put(
                p,
                TrackerKind::CHOICES[k.slot()].key.as_ref(),
                &BletrackerKindRow {
                    kind: k,
                    last_minute: i64::from(now_k[k.slot()]),
                    last_30_min: i64::from(win_k[k.slot()]),
                },
            );
        }
        b.order_into(&mut self.order);
        for t in self.order.iter().rev().filter_map(|o| b.by_id(o.2)) {
            self.key.clear();
            let _ = write!(self.key, "{}", t.id);
            BLETRACKER_TRACKERS.put(
                p,
                &self.key,
                &BletrackerTrackerRow {
                    tracker: t.label(),
                    kind: t.kind,
                    state: t.state(),
                    stays: Seconds(t.stays()),
                    near_pct: 100.0 * t.near_share,
                    level_dbfs: t.avg,
                    peak_dbfs: t.peak,
                    trend: t.trend(),
                    addresses: i64::from(t.addresses),
                    last_s: t.last_s,
                    last_heard: t.last_wall.map(Time),
                    packets: t.packets as i64,
                    separated: t.separated,
                    status: t.status.map(i64::from),
                    following: t.following,
                    mine: t.mine,
                    id: i64::from(t.id),
                    mode: t.mode,
                    tag_state: t.smarttag.and_then(|v| v.state()),
                    tag_state_code: t.smarttag.map(|v| i64::from(v.state_code)),
                    aging_counter: t.smarttag.map(|v| i64::from(v.aging_counter)),
                    region: t.smarttag.map(|v| i64::from(v.region)),
                    e2e: t.smarttag.map(|v| v.e2e),
                    uwb: t.smarttag.map(|v| v.uwb),
                    battery: t.smarttag.map(|v| i64::from(v.battery)),
                    identifier: (!t.identifier.is_empty()).then_some(t.identifier.as_str()),
                    service_data: (!t.service_data.is_empty()).then_some(t.service_data.as_str()),
                },
            );
        }
        remove_older_rows(p, BLETRACKER_TRACKERS.def(), self.order.len());
        shaped::log_rows(&b.alerts, &BLETRACKER_ALERTS, now, p);
    }

    fn mark_mine(&mut self, args: &Values) -> Result<(), CommandError> {
        let a: BletrackerMineArgs = BLETRACKER_MINE.args(args)?;
        let (row, on) = (a.row, a.mine.unwrap_or(true));
        let table = self.trackers_table;
        let no_row = || CommandError::NoSuchRow {
            table,
            key: row.to_owned(),
        };
        let id: u32 = row.parse().map_err(|_| no_row())?;
        if self.book.ignore(id, on) {
            Ok(())
        } else {
            Err(no_row())
        }
    }
}

impl Component for TrackerBook {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "wlan.ble_trackers",
            vec![
                Port::typed::<Advert>("adverts"),
                Port::typed::<AdvStats>("stats"),
            ],
            vec![
                Port::new("events", Kind::Messages),
                Port::typed::<HostRequest>("requests"),
            ],
        )
        .with_settings(TrackerBookSetting::schema())
    }

    fn configure(&mut self, _: &[Format]) -> Result<Vec<Format>, ConfigError> {
        // The book's clock never restarts: the signal time passed is kept as a base.
        if self.configured {
            self.clock_base = self.now_s;
        }
        self.configured = true;
        self.stats = AdvStats::default();
        self.base = AdvStats::default();
        Ok(vec![
            Format::new(Kind::Messages, 0.0),
            Format::new(Kind::typed::<HostRequest>(), 0.0),
        ])
    }

    fn attach(&mut self, ctx: &RunContext) {
        self.live = ctx.wall_clock();
        self.clock = ctx.clock.clone();
    }

    fn process(&mut self, io: &mut Io) {
        for s in io.items::<AdvStats>(1) {
            self.stats = s.value;
            self.now_s = self.now_s.max(self.clock_base + s.value.time_s);
            if s.value.level_dbfs.is_some() {
                self.level_dbfs = s.value.level_dbfs;
            }
        }
        for a in io.items::<Advert>(0) {
            self.take(&a.value);
        }
        if io.tick() {
            self.publish();
        }
        if !self.events.is_empty() {
            io.messages_out(0).append(&mut self.events);
        }
        let now = self.now_s;
        for r in self.requests.drain(..) {
            io.items_out::<HostRequest>(1).push(Timed {
                time: now,
                value: r,
            });
        }
    }

    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        match TrackerBookSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            TrackerBookSetting::Channel => {
                let c = CHANNEL_SETTING.read(id, v)?;
                if c != self.channel {
                    self.channel = c;
                    let hz = channel_hz(c.number()).unwrap_or(2402e6);
                    self.requests
                        .push(HostRequest::retune(hz, "advertising channel chosen"));
                }
            }
            TrackerBookSetting::AlertAfterMin => {
                self.alert_after_min = ALERT_AFTER_MIN.read(id, v)?;
                self.update_alert_time();
            }
            TrackerBookSetting::AlertAfterS => {
                self.alert_after_s = ALERT_AFTER_S.read(id, v)?;
                self.update_alert_time();
            }
            TrackerBookSetting::Near => self.pending.near_dbfs = NEAR.read(id, v)?,
        }
        Ok(Applied::Live)
    }

    fn values(&self) -> Values {
        values_of([
            (
                TrackerBookSetting::Channel,
                Value::Choice(self.channel.index()),
            ),
            (
                TrackerBookSetting::AlertAfterMin,
                Value::F64(self.alert_after_min),
            ),
            (TrackerBookSetting::Near, Value::F64(self.pending.near_dbfs)),
            (
                TrackerBookSetting::AlertAfterS,
                Value::Duration(self.alert_after_s),
            ),
        ])
    }

    fn results(&self) -> Option<&Results> {
        Some(self.published.results())
    }

    fn commands(&self) -> &Commands {
        TrackerBookCommand::commands()
    }

    /// The counters of the receiver and the measurements start again; the book of trackers and the alerts stay if
    /// `kept` holds them.
    fn forget(&mut self, kept: &Kept) {
        self.base = self.stats;
        self.snr_db = None;
        self.offset_hz = None;
        self.last_good_s = None;
        self.level_dbfs = None;
        self.sightings_base = self.book.sightings;
        if !kept.table(BLETRACKER_TRACKERS.key) {
            self.book.clear();
            self.sightings_base = 0;
        }
        if self.configured {
            self.publish();
        }
    }

    fn invoke(&mut self, id: CommandId, args: &Values) -> Result<Option<Record>, CommandError> {
        match TrackerBookCommand::from_id(id).ok_or(CommandError::NoSuchId { id })? {
            TrackerBookCommand::Clear => {
                self.book.clear();
                self.base = self.stats;
                self.sightings_base = 0;
            }
            TrackerBookCommand::Mine => self.mark_mine(args)?,
        }
        if self.configured {
            self.publish();
        }
        Ok(None)
    }
}

/// The `bletracker` graph.
///
/// # Errors
///
/// [`GraphError`] if the graph does not build.
pub fn graph(catalog: &Arc<Catalog>) -> Result<Graph, GraphError> {
    ble::with_sender(ble::front()?, ble::tx::BleSender::tracker())
        .add("book", TrackerBook::default())
        .connect("adv.adverts", "book.adverts")
        .connect("adv.stats", "book.stats")
        .output("book.events")
        .output("book.requests")
        .clock(ble::REPORT_S)
        .status_from("book")
        .catalog(catalog)
        .build()
}

/// The `bletracker` receiver.
///
/// # Errors
///
/// [`GraphError`] if the graph does not build or a value of `cfg` is refused.
pub fn receiver(cfg: &StackConfig) -> Result<StackGraph, GraphError> {
    let b = StackGraph::of(crate::typed::shape(ID), graph(&Catalog::of(cfg)?)?).min_rate(
        ble::MIN_RATE_HZ,
        "The BLE tracker detector needs a sample rate of at least 4 Msps",
    );
    let b = route_to(b, "book", &TrackerSetting::schema())?
        .command("clear", sdrkit_graph::CommandTarget::every("clear"))
        .command("mine", sdrkit_graph::CommandTarget::of("book", "mine"));
    ble::route_sender(b)?.with_settings(&cfg.settings).build()
}

/// Adds the tracker book component to a registry.
pub fn register(r: &mut Registry) {
    r.add("wlan.ble_trackers", || Box::new(TrackerBook::default()));
}

fn make_receiver(cfg: &StackConfig) -> Result<Box<dyn Receiver>, ConfigError> {
    Ok(Box::new(receiver(cfg)?))
}

fn make_generator(cfg: &GenConfig) -> Result<Box<dyn Generator>, ConfigError> {
    let mut packets = tracker_schedule();
    if crate::testgen::gen_setting(cfg, &GEN_SCHEDULE) == Some(GenSchedule::FindMy) {
        packets.truncate(1);
    }
    Ok(Box::new(BleGenerator::new(BleGenParams {
        rate_hz: cfg.rate_hz,
        packets,
        ..Default::default()
    })))
}

/// The `bletracker` stack.
pub static BLETRACKER: StackDescriptor = StackDescriptor {
    id: ID,
    name: "BLE tracker detector",
    group: "Data and IoT",
    blurb: "Finds item trackers (AirTag and Find My accessories, Tile, Chipolo, SmartTag, Google tags) that stay with you.",
    tuning: Tuning {
        kind: Some(sdrkit_core::TuningKind::Channels),
        min_hz: 2400e6,
        max_hz: 2484e6,
        default_hz: 2402e6,
        rate_hz: 4e6,
        min_rate_hz: ble::MIN_RATE_HZ,
        bandwidth_hz: 2e6,
        tune_offset_hz: 1e6,
        carrier_at_centre: false,
        not_circular: false,
        radio_filter_hz: Some(2e6),
        no_offset_tuning: false,
        ..Tuning::DEFAULT
    },
    submodes: &[],
    settings: TrackerSetting::schema,
    make_receiver,
    make_generator: Some(make_generator),
    lock_profile: LockProfile::DEFAULT,
    signal_types: &[SignalType::BluetoothLe],
    auto_gain: AutoGain::Radio,
    verify: Some(&crate::verify::TRACKER),
    benchmarks: None,
};
