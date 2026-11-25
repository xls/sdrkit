//! The `espnow` device table, the frame log, the frames per PHY rate and per channel, and what the stack publishes (the
//! `wlan.espnow_table` component). The devices and the log are kept in the saved state; the keys are not.

use super::ccmp::{self, CcmpCipher};
use super::frame::{self, Message, Reading};
use crate::report::{hex, mac_text, push_history, wall};
use crate::typed::{
    EspnowChannelRow, EspnowDeviceEvent, EspnowDeviceRow, EspnowFields, EspnowFrameEvent,
    EspnowFrameRow, EspnowRateRow, ESPNOW_BY_CHANNEL, ESPNOW_BY_RATE, ESPNOW_DEVICE,
    ESPNOW_DEVICES, ESPNOW_FRAME, ESPNOW_FRAMES,
};
use crate::wifi::plan::{centre_mhz, hop_list, parse_channels, Band};
use crate::wifi::{DsssCounters, OfdmCounters, PhyKind, WifiMpdu, WifiStats};
use sdrkit_core::command::{CommandError, CommandId, CommandSet, Commands};
use sdrkit_core::interface::{Fields as _, Hertz, Time};
use sdrkit_core::publish::keys::Family;
use sdrkit_core::publish::{field, Published};
use sdrkit_core::{
    Clock, ConfigError, Event, HostRequest, LockState, Record, Results, Setting, SettingError,
    SettingId, SettingSet, StateBlob, StateError, Status, Value, Values,
};
use sdrkit_graph::{
    values_of, Applied, Component, ComponentInfo, Format, Io, Kept, Kind, Port, RunContext, Timed,
};
use sdrkit_util_objects::commands::{self as objcmd, Tables as _};
use sdrkit_util_objects::publish::columns;
use sdrkit_util_objects::shaped::{self, RowOut, Shaped};
use sdrkit_util_objects::{Format as StateFormat, LogRing, ObjectTable, Seen, Shift, Stamp};
use serde::{Deserialize, Serialize};
use std::fmt::Write as _;

/// Most devices kept.
pub const DEVICE_CAP: usize = 300;
/// Most log lines kept.
pub const LOG_CAP: usize = 500;
/// An ESP-NOW frame within this time keeps the lock state on, s.
pub const ACTIVE_S: f64 = 5.0;
/// The longest window of the packet rate, s; a device unheard for longer has rate 0.
pub const RATE_WINDOW_S: f64 = 10.0;
/// New devices that get a line in the text log.
const NEW_DEVICE_LINES: usize = 30;

/// The note of the stack: what is decoded, what is only detected, and what no public document confirms.
pub const NOTE: &str = "Decoded: clear ESP-NOW frames at DSSS 1 and 2 Mbit/s and non-HT OFDM, version 1 and 2 elements, and \
protected frames that a key in Keys opens with CCMP. Only detected: protected frames that no key opens; they show their \
addresses, sequence number and packet number. Not confirmed by a public document: that a key entered here is the CCMP key of \
the frames (the maker's guide names a primary key and local keys per peer and does not state how the frame key follows from \
them), the CCMP header, nonce and additional data as applied to these frames, the action no-ack subtype, and channel 1 as the \
usual channel. Frames with a bad FCS are counted, not listed. Nothing is transmitted.";

/// The channel to listen on: the host tunes to it.
pub const CHANNEL: Setting<field::I64> = Setting::new("channel", "Channel", "", 1)
    .range(1.0, 14.0, 1.0)
    .help("A 2.4 GHz Wi-Fi channel: the host tunes to it.");
/// Hop through the hop list.
pub const HOP: Setting<field::Bool> = Setting::new("hop", "Hop", "", false)
    .help("Asks the host to step through the channels of the hop list.");
/// The time on each channel while hopping.
pub const DWELL_MS: Setting<field::F64> =
    Setting::new("dwell_ms", "Dwell", "ms", 300.0).range(100.0, 5000.0, 10.0);
/// The keys tried on protected frames.
pub const KEYS: Setting<field::Text> = Setting::new("keys", "Keys", "", "").help(
    "16-byte keys, 32 hex digits each, apart by spaces or commas. Each is tried as the CCMP key of a protected frame.",
);

sdrkit_core::settings! {
    /// The settings of [`EspnowTable`].
    pub enum EspnowTableSetting {
        /// The channel.
        Channel = CHANNEL,
        /// Hop through the hop list.
        Hop = HOP,
        /// The dwell while hopping.
        DwellMs = DWELL_MS,
        /// The channels of the hop list.
        Channels24 = crate::wifi::table::CHANNELS_24,
        /// The keys.
        Keys = KEYS,
    }
}

sdrkit_core::commands! {
    /// The commands of [`EspnowTable`].
    pub enum EspnowTableCommand {
        /// Forget the devices, the log and the counters.
        Clear = objcmd::CLEAR,
    }
}

/// One device (one sender address).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Device {
    /// The address, MAC text.
    pub address: String,
    /// The maker ("local" for a locally administered address).
    pub vendor: String,
    /// The address is one of Espressif.
    pub espressif: bool,
    /// ESP-NOW frames.
    pub packets: u64,
    /// Protected frames.
    pub encrypted: u64,
    /// Frames to the broadcast address.
    pub broadcasts: u64,
    /// The start of the rate window, signal s.
    pub window_s: f64,
    /// Frames in the rate window.
    pub window_count: u64,
    /// Level of the last frame, dBFS.
    pub level_dbfs: f64,
    /// The last 24 levels.
    pub history: Vec<f32>,
    /// The channel of the last frame.
    pub channel: u32,
    /// The PHY rate of the last frame, kbit/s.
    pub phy_rate_kbps: u32,
    /// "broadcast" or the last destination.
    pub last_to: String,
    /// The last payload as hex, empty when it was encrypted.
    pub last_payload: String,
    /// The version of the last readable frame.
    pub version: Option<u8>,
}

impl Device {
    /// The packet rate at `now_s`: the frames of the window over its length (1 to 10 s), 0 when the device was last heard
    /// more than 10 s ago.
    pub fn rate(&self, last_s: f64, now_s: f64) -> f64 {
        if now_s - last_s > RATE_WINDOW_S {
            return 0.0;
        }
        self.window_count as f64 / (now_s - self.window_s).clamp(1.0, RATE_WINDOW_S)
    }
}

impl Shift for Device {
    fn shift_times(&mut self, by_s: f64) {
        self.window_s += by_s;
    }
}

impl Shaped for Device {
    type Row = EspnowDeviceRow<'static>;
    fn row(&self, seen: Option<&Seen>, now_s: f64, out: RowOut<'_, '_, Self::Row>) {
        let (first_s, last_s) = seen.map_or((0.0, 0.0), |s| (s.first_s, s.last_s));
        out.put(&EspnowDeviceRow {
            address: &self.address,
            vendor: if self.vendor == "local" {
                "(local)"
            } else {
                &self.vendor
            },
            espressif: self.espressif,
            packets: self.packets as i64,
            encrypted: self.encrypted as i64,
            broadcasts: self.broadcasts as i64,
            rate: self.rate(last_s, now_s),
            level_dbfs: self.level_dbfs,
            channel: i64::from(self.channel),
            phy_rate_kbps: i64::from(self.phy_rate_kbps),
            last_to: &self.last_to,
            last_payload: &self.last_payload,
            version: self.version.map(i64::from),
            first_heard: columns::first_heard(seen).map(Time),
            last_heard: columns::last_heard(seen).map(Time),
            first_s,
            last_s,
        });
    }
}

/// One log line.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct LogLine {
    /// Signal time.
    pub t: f64,
    /// Wall clock, s since 1970, when live.
    pub wall: Option<f64>,
    /// Channel.
    pub ch: u32,
    /// PHY rate, kbit/s.
    pub rate_kbps: u32,
    /// DSSS short preamble.
    pub short: bool,
    /// Sender.
    pub from: String,
    /// "broadcast" or the destination.
    pub to: String,
    /// Sequence number.
    pub seq: u16,
    /// Version, when readable.
    pub version: Option<u8>,
    /// Protected.
    pub encrypted: bool,
    /// Opened with a key.
    pub decrypted: bool,
    /// Level, dBFS.
    pub level_dbfs: f64,
    /// SNR, dB.
    pub snr_db: Option<f64>,
    /// Carrier offset, Hz.
    pub offset_hz: f64,
    /// Payload as hex.
    pub payload: String,
    /// Summary.
    pub info: String,
}

impl Shift for LogLine {
    fn shift_times(&mut self, by_s: f64) {
        self.t += by_s;
    }
}

impl Shaped for LogLine {
    type Row = EspnowFrameRow<'static>;
    fn row(&self, _seen: Option<&Seen>, _now_s: f64, out: RowOut<'_, '_, Self::Row>) {
        out.put(&EspnowFrameRow {
            time: self.wall.map(Time),
            signal_s: self.t,
            channel: i64::from(self.ch),
            phy_rate_kbps: i64::from(self.rate_kbps),
            short_preamble: self.short,
            from: &self.from,
            to: &self.to,
            seq: i64::from(self.seq),
            version: self.version.map(i64::from),
            encrypted: self.encrypted,
            decrypted: self.decrypted,
            level_dbfs: self.level_dbfs,
            snr_db: self.snr_db,
            offset_hz: Some(Hertz(self.offset_hz)),
            payload: &self.payload,
            details: &self.info,
        });
    }
}

/// The devices and the log, kept across sessions.
#[derive(Serialize, Deserialize)]
pub struct Store {
    /// The devices, by address text.
    pub devices: ObjectTable<String, Device>,
    /// The frame log.
    pub log: LogRing<LogLine>,
}

impl Default for Store {
    fn default() -> Self {
        Store {
            devices: ObjectTable::new(DEVICE_CAP),
            log: LogRing::new(LOG_CAP),
        }
    }
}

impl Shift for Store {
    fn shift_times(&mut self, by_s: f64) {
        self.devices.shift_times(by_s);
        self.log.shift_times(by_s);
    }
}

impl objcmd::Tables for Store {
    const FORMAT: StateFormat = StateFormat::new("wlan.espnow_table", 1);
    fn clear(&mut self) {
        self.devices.clear();
        self.log.clear();
    }
    fn adopt(&mut self, saved: Self) {
        self.devices.adopt(saved.devices);
        self.log.adopt(saved.log);
    }
}

/// The PHY rate groups of the `by_rate` table: row key and label.
pub const RATE_GROUPS: [(&str, &str); 5] = [
    ("dsss1", "1 Mbit/s"),
    ("dsss2_long", "2 Mbit/s long"),
    ("dsss2_short", "2 Mbit/s short"),
    ("ofdm6_9", "OFDM 6 and 9"),
    ("ofdm12", "OFDM 12 and above"),
];

/// The index in [`RATE_GROUPS`] of a frame.
fn rate_group(m: &WifiMpdu) -> usize {
    match m.phy {
        PhyKind::Dsss if m.rate_mbps < 1.5 => 0,
        PhyKind::Dsss if m.short_preamble => 2,
        PhyKind::Dsss => 1,
        PhyKind::Ofdm if m.rate_mbps < 10.0 => 3,
        PhyKind::Ofdm | PhyKind::Ht | PhyKind::Vht => 4,
    }
}

/// The counters of the frames taken since the last clear.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Counters {
    /// Management action frames.
    pub action: u64,
    /// ESP-NOW frames.
    pub espnow: u64,
    /// ESP-NOW frames that were protected.
    pub encrypted: u64,
    /// Protected frames that a key opened.
    pub decrypted: u64,
    /// ESP-NOW frames to the broadcast address.
    pub broadcast: u64,
    /// ESP-NOW frames per [`RATE_GROUPS`].
    pub by_rate: [u64; 5],
    /// ESP-NOW frames per channel 1 to 14.
    pub by_channel: [u64; 14],
    /// Level, SNR and carrier offset of the last good frame.
    pub last: Option<(f64, Option<f64>, f64)>,
    /// Signal time of the last ESP-NOW frame.
    pub last_espnow_s: Option<f64>,
}

/// The device table as a component.
pub struct EspnowTable {
    published: Published,
    /// The devices and the log.
    pub store: Store,
    /// The counters.
    pub counters: Counters,
    stats: WifiStats,
    base_dsss: DsssCounters,
    base_ofdm: OfdmCounters,
    channel: u32,
    hop: bool,
    dwell_ms: f64,
    list: (String, Vec<u32>),
    keys_text: String,
    ciphers: Vec<CcmpCipher>,
    scratch: Vec<u8>,
    requests: Vec<HostRequest>,
    events: Vec<Event>,
    new_logged: usize,
    lines: Vec<String>,
    status: Status,
    now_s: f64,
    live: bool,
    clock: Clock,
    configured: bool,
}

impl Default for EspnowTable {
    fn default() -> Self {
        EspnowTable {
            published: Published::new(EspnowFields::SHAPE.fields).with_tables(&[
                ESPNOW_DEVICES.def(),
                ESPNOW_FRAMES.def(),
                ESPNOW_BY_RATE.def(),
                ESPNOW_BY_CHANNEL.def(),
            ]),
            store: Store::default(),
            counters: Counters::default(),
            stats: WifiStats::default(),
            base_dsss: DsssCounters::default(),
            base_ofdm: OfdmCounters::default(),
            channel: u32::try_from(CHANNEL.default).unwrap_or(1),
            hop: HOP.default,
            dwell_ms: DWELL_MS.default,
            list: Default::default(),
            keys_text: String::new(),
            ciphers: Vec::new(),
            scratch: Vec::new(),
            requests: Vec::new(),
            events: Vec::new(),
            new_logged: 0,
            lines: Vec::new(),
            status: Status::default(),
            now_s: 0.0,
            live: false,
            clock: Clock::fixed(0.0),
            configured: false,
        }
    }
}

/// Whether the maker of `addr` is Espressif, from the IEEE registry table of the library.
pub fn is_espressif(addr: &[u8; 6]) -> bool {
    sdrkit_util_ids::oui::vendor_of_address(addr).is_some_and(|v| v.starts_with("Espressif"))
}

impl EspnowTable {
    /// Takes a good MPDU: the counters, the device row, the log and the events.
    pub fn take(&mut self, m: &WifiMpdu) {
        let reading = frame::read(&m.bytes, &self.ciphers, is_espressif, &mut self.scratch);
        let c = &mut self.counters;
        c.last = Some((m.level_dbfs, m.snr_db, m.offset_hz));
        let msg = match reading {
            Reading::Other => return,
            Reading::Action(_) => {
                c.action += 1;
                return;
            }
            Reading::EspNow(msg) => msg,
        };
        c.action += 1;
        c.espnow += 1;
        c.encrypted += u64::from(msg.sealed.is_some());
        c.decrypted += u64::from(msg.decrypted);
        c.broadcast += u64::from(msg.header.broadcast());
        c.by_rate[rate_group(m)] += 1;
        if (1..=14).contains(&m.channel) {
            c.by_channel[m.channel as usize - 1] += 1;
        }
        c.last_espnow_s = Some(m.time_s);
        self.update(&msg, m);
    }

    fn update(&mut self, msg: &Message, m: &WifiMpdu) {
        let now = m.time_s;
        let w = wall(self.live, &self.clock);
        let at = match w {
            Some(w) => Stamp::at(now).with_wall(w),
            None => Stamp::at(now),
        };
        let from = mac_text(&msg.header.from);
        let to = frame::destination(&msg.header);
        let payload = msg.content.as_ref().map(|c| hex(&c.payload)).unwrap_or_default();
        let info = frame::summary(msg);
        let rate_kbps = (m.rate_mbps * 1000.0).round() as u32;
        let freq = centre_mhz(Band::G2, m.channel).map_or(0.0, |f| f * 1e6);
        let level = m.level_dbfs;
        let Ok(up) = self.store.devices.upsert(from.as_str(), at, || {
            let vendor = if sdrkit_util_ids::oui::is_locally_administered(msg.header.from[0]) {
                "local"
            } else {
                sdrkit_util_ids::oui::vendor_of_address(&msg.header.from).unwrap_or("")
            };
            Device {
                address: from.clone(),
                vendor: vendor.into(),
                espressif: is_espressif(&msg.header.from),
                window_s: now,
                ..Default::default()
            }
        }) else {
            return;
        };
        let d = up.row;
        if up.new {
            if self.new_logged < NEW_DEVICE_LINES {
                self.new_logged += 1;
                self.lines.push(format!("ESP-NOW: new device {from}"));
            }
            self.events.push(ESPNOW_DEVICE.event(
                now,
                freq,
                &EspnowDeviceEvent {
                    source: &from,
                    vendor: &d.vendor,
                    family: Family::Other,
                },
            ));
        }
        d.packets += 1;
        d.encrypted += u64::from(msg.sealed.is_some());
        d.broadcasts += u64::from(msg.header.broadcast());
        if now - d.window_s >= RATE_WINDOW_S {
            d.window_s = now;
            d.window_count = 0;
        }
        d.window_count += 1;
        d.level_dbfs = level;
        push_history(&mut d.history, level as f32, 24);
        d.channel = m.channel;
        d.phy_rate_kbps = rate_kbps;
        d.last_to.clone_from(&to);
        d.last_payload.clone_from(&payload);
        if let Some(c) = &msg.content {
            d.version = Some(c.version);
        }
        self.events.push(ESPNOW_FRAME.event(
            now,
            freq,
            &EspnowFrameEvent {
                source: &from,
                to: &to,
                text: &info,
                level_dbfs: level,
                family: Family::Message,
            },
        ));
        self.store.log.push(LogLine {
            t: now,
            wall: w,
            ch: m.channel,
            rate_kbps,
            short: m.short_preamble,
            from,
            to,
            seq: msg.header.seq,
            version: msg.content.as_ref().map(|c| c.version),
            encrypted: msg.sealed.is_some(),
            decrypted: msg.decrypted,
            level_dbfs: level,
            snr_db: m.snr_db,
            offset_hz: m.offset_hz,
            payload,
            info,
        });
    }

    fn clear(&mut self) {
        self.store.clear();
        self.counters = Counters::default();
        self.base_dsss = self.stats.dsss;
        self.base_ofdm = self.stats.ofdm;
        self.published.clear_rows(ESPNOW_BY_RATE.def());
        self.published.clear_rows(ESPNOW_BY_CHANNEL.def());
    }

    /// Publishes the results now.
    pub fn publish(&mut self) {
        let now = self.now_s;
        let st = self.stats;
        let d = st.dsss.since(self.base_dsss);
        let o = st.ofdm.since(self.base_ofdm);
        let c = &self.counters;
        let heard = c.last_espnow_s.is_some_and(|l| now - l <= ACTIVE_S);
        let devices = self.store.devices.len();
        let status = &mut self.status;
        status.lock_state = if heard {
            LockState::Locked
        } else {
            LockState::Searching
        };
        status.snr_db = c.last.and_then(|l| l.1).map(|s| s as f32);
        status.level_dbfs = c.last.map(|l| l.0 as f32);
        status.offset_hz = c.last.map(|l| l.2);
        status.summary.clear();
        // Writing to a String cannot fail.
        let _ = if st.rate_hz > 0.0 && st.rate_hz < super::MIN_RATE_HZ * 0.999_999 {
            write!(
                status.summary,
                "ESP-NOW needs 20 Msps; the input is {} Msps",
                st.rate_hz / 1e6
            )
        } else if devices > 0 {
            write!(
                status.summary,
                "{devices} devices, {} ESP-NOW frames",
                c.espnow
            )
        } else if st.channel != 0 {
            write!(status.summary, "listening on channel {}", st.channel)
        } else {
            write!(status.summary, "listening for ESP-NOW frames")
        };
        let p = &mut self.published;
        {
            let r = p.results_mut();
            r.set_status(status);
            for line in self.lines.drain(..) {
                r.log(line);
            }
        }
        let known = st.channel != 0;
        p.put(&EspnowFields {
            devices: devices as i64,
            frames_ok: (d.frames + o.frames) as i64,
            frames_bad: (d.bad_fcs + o.bad_fcs) as i64,
            action_frames: c.action as i64,
            espnow_frames: c.espnow as i64,
            encrypted: c.encrypted as i64,
            decrypted: c.decrypted as i64,
            broadcast: c.broadcast as i64,
            heard,
            channel: known.then_some(i64::from(st.channel)),
            channel_hz: centre_mhz(Band::G2, st.channel)
                .filter(|_| known)
                .map(|f| Hertz(f * 1e6)),
            channel_offset_hz: known.then_some(Hertz(st.channel_offset_hz)),
            level_dbfs: c.last.map(|l| l.0),
            snr_db: c.last.and_then(|l| l.1),
            offset_hz: c.last.map(|l| Hertz(l.2)),
            input_rate_hz: Hertz(st.rate_hz),
            dsss_sfds: d.sfds as i64,
            dsss_headers_ok: d.headers_ok as i64,
            dsss_headers_bad: d.headers_bad as i64,
            dsss_cck_headers: d.other_rate as i64,
            ofdm_preambles: o.preambles as i64,
            ofdm_signal_ok: o.signal_ok as i64,
            ofdm_signal_bad: o.signal_bad as i64,
            hopping: self.hop,
            note: NOTE,
        });
        for (i, &n) in c.by_rate.iter().enumerate() {
            if n > 0 {
                let (key, label) = RATE_GROUPS[i];
                ESPNOW_BY_RATE.put(
                    p,
                    key,
                    &EspnowRateRow {
                        rate: label,
                        frames: n as i64,
                    },
                );
            }
        }
        const CHANNEL_KEYS: [&str; 14] = [
            "1", "2", "3", "4", "5", "6", "7", "8", "9", "10", "11", "12", "13", "14",
        ];
        for (i, &n) in c.by_channel.iter().enumerate() {
            if n > 0 {
                ESPNOW_BY_CHANNEL.put(
                    p,
                    CHANNEL_KEYS[i],
                    &EspnowChannelRow {
                        channel: i as i64 + 1,
                        frames: n as i64,
                    },
                );
            }
        }
        shaped::rows(&self.store.devices, &ESPNOW_DEVICES, now, p);
        shaped::log_rows(&self.store.log, &ESPNOW_FRAMES, now, p);
    }

    fn request_channel(&mut self) {
        if let Some(mhz) = centre_mhz(Band::G2, self.channel) {
            self.requests
                .push(HostRequest::retune(mhz * 1e6, "ESP-NOW channel chosen"));
        }
    }

    fn request_hop(&mut self) {
        let channels: Vec<f64> = hop_list(Band::G2, &self.list.1, false)
            .into_iter()
            .filter_map(|c| centre_mhz(Band::G2, c))
            .map(|mhz| mhz * 1e6)
            .collect();
        if !channels.is_empty() {
            self.requests.push(HostRequest::Hop {
                channels,
                dwell_s: self.dwell_ms / 1e3,
                stop_on_lock: false,
            });
        }
    }

    fn hop_changed(&mut self) {
        if self.hop {
            self.request_hop();
        }
    }

    fn set_keys(&mut self, id: SettingId, text: &str) -> Result<(), SettingError> {
        let keys = frame::parse_keys(text).ok_or(SettingError::Malformed { id, at: None })?;
        self.ciphers = keys.iter().map(ccmp::cipher).collect();
        text.clone_into(&mut self.keys_text);
        Ok(())
    }
}

impl Component for EspnowTable {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "wlan.espnow_table",
            vec![
                Port::typed::<WifiMpdu>("mpdus"),
                Port::typed::<WifiStats>("stats"),
            ],
            vec![
                Port::new("events", Kind::Messages),
                Port::typed::<HostRequest>("requests"),
            ],
        )
        .with_settings(EspnowTableSetting::schema())
    }

    fn configure(&mut self, _: &[Format]) -> Result<Vec<Format>, ConfigError> {
        // A restart forgets the signal and keeps the tables: their times move back by the old signal time.
        if self.configured {
            self.store.shift_times(-self.now_s);
            if let Some(l) = &mut self.counters.last_espnow_s {
                *l -= self.now_s;
            }
        }
        self.configured = true;
        self.now_s = 0.0;
        self.stats = WifiStats::default();
        self.base_dsss = DsssCounters::default();
        self.base_ofdm = OfdmCounters::default();
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
        for s in io.items::<WifiStats>(1) {
            self.stats = s.value;
            self.now_s = self.now_s.max(s.value.time_s);
        }
        for m in io.items::<WifiMpdu>(0) {
            self.take(&m.value);
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
        if io.tick() {
            self.publish();
        }
    }

    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        match EspnowTableSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            EspnowTableSetting::Channel => {
                let c = u32::try_from(CHANNEL.read(id, v)?)
                    .map_err(|_| SettingError::NotPermitted { id })?;
                if c != self.channel {
                    self.channel = c;
                    self.request_channel();
                }
            }
            EspnowTableSetting::Hop => {
                let on = HOP.read(id, v)?;
                if on != self.hop {
                    self.hop = on;
                    if on {
                        self.request_hop();
                    } else {
                        self.request_channel();
                    }
                }
            }
            EspnowTableSetting::DwellMs => {
                let d = DWELL_MS.read(id, v)?;
                if d != self.dwell_ms {
                    self.dwell_ms = d;
                    self.hop_changed();
                }
            }
            EspnowTableSetting::Channels24 => {
                let text = crate::wifi::table::CHANNELS_24.read(id, v)?;
                let numbers =
                    parse_channels(Band::G2, text).ok_or(SettingError::NotPermitted { id })?;
                text.clone_into(&mut self.list.0);
                if self.list.1 != numbers {
                    self.list.1 = numbers;
                    self.hop_changed();
                }
            }
            EspnowTableSetting::Keys => self.set_keys(id, KEYS.read(id, v)?)?,
        }
        if self.configured {
            self.publish();
        }
        Ok(Applied::Live)
    }

    fn values(&self) -> Values {
        values_of([
            (
                EspnowTableSetting::Channel,
                Value::I64(i64::from(self.channel)),
            ),
            (EspnowTableSetting::Hop, Value::Bool(self.hop)),
            (EspnowTableSetting::DwellMs, Value::F64(self.dwell_ms)),
            (
                EspnowTableSetting::Channels24,
                Value::Text(self.list.0.clone()),
            ),
            (EspnowTableSetting::Keys, Value::Text(self.keys_text.clone())),
        ])
    }

    fn results(&self) -> Option<&Results> {
        Some(self.published.results())
    }

    fn commands(&self) -> &Commands {
        EspnowTableCommand::commands()
    }

    fn invoke(&mut self, id: CommandId, _args: &Values) -> Result<Option<Record>, CommandError> {
        match EspnowTableCommand::from_id(id).ok_or(CommandError::NoSuchId { id })? {
            EspnowTableCommand::Clear => self.clear(),
        }
        if self.configured {
            self.publish();
        }
        Ok(None)
    }

    /// The counters and the measurements start again; the devices and the frame log stay if `kept` holds them.
    fn forget(&mut self, kept: &Kept) {
        let last = self.counters.last_espnow_s;
        self.counters = Counters {
            last_espnow_s: last,
            ..Counters::default()
        };
        self.base_dsss = self.stats.dsss;
        self.base_ofdm = self.stats.ofdm;
        self.published.clear_rows(ESPNOW_BY_RATE.def());
        self.published.clear_rows(ESPNOW_BY_CHANNEL.def());
        if !kept.table(ESPNOW_DEVICES.key) {
            self.store.devices.clear();
        }
        if !kept.table(ESPNOW_FRAMES.key) {
            self.store.log.clear();
        }
        if self.configured {
            self.publish();
        }
    }

    fn save_state(&self) -> Option<StateBlob> {
        self.store.save_state(self.now_s).ok()
    }

    fn restore_state(&mut self, state: &StateBlob) -> Result<(), StateError> {
        self.store.restore_state(state, self.now_s)?;
        self.publish();
        Ok(())
    }
}
