//! `elrs`: the hop channels around the dial with a detector for each rate row and IQ polarity (`elrs.channels`), the frame log
//! and the link table (`elrs.decode`), and the test signal. The receiver is the shared LoRa bank.

use super::{count, show_status};
use crate::components::channels::{Input, Planner};
use crate::gen::{SceneGen, AMP};
use crate::proto::elrs::{self, LinkId, RateRow, ROWS};
use crate::typed::{
    ElrsFields, ElrsFrameRow, ElrsLinkEvent, ElrsLinkRow, ELRS_FRAMES, ELRS_LINK, ELRS_LINKS,
};
use sdrkit_core::command::{CommandError, CommandId, CommandSet, Commands};
use sdrkit_core::interface::{Hertz, Time};
use sdrkit_core::publish::keys::Family;
use sdrkit_core::publish::{field, Published};
use sdrkit_core::{
    Choice, Clock, ConfigError, Event, GenConfig, LockState, Record, Results, Setting,
    SettingError, SettingId, SettingSet, SignalInfo, StateBlob, StateError, Value, Values,
};
use sdrkit_graph::{
    values_of, Applied, Component, ComponentInfo, Format, Io, Item, Kept, Kind, Port, RateFrom,
    RunContext,
};
use sdrkit_proto_lora_phy::bank::{BankChannel, BankPlan};
use sdrkit_proto_lora_phy::channel::in_band;
use sdrkit_proto_lora_phy::coding::Header;
use sdrkit_proto_lora_phy::components::BankState;
use sdrkit_proto_lora_phy::modulate::TxFrame;
use sdrkit_proto_lora_phy::params::{Ldro, LoraParams};
use sdrkit_proto_lora_phy::rx::{DetectorConfig, LoraFrame};
use sdrkit_util_objects::commands::CLEAR;
use sdrkit_util_objects::publish::columns;
use sdrkit_util_objects::shaped::{self, RowOut, Shaped};
use sdrkit_util_objects::{
    Format as StateFormat, LogRing, ObjectTable, Seen, Shift, Stamp, Tables,
};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::fmt::Write as _;

/// The least input rate: one hop channel of the LoRa bank.
pub const MIN_RATE_HZ: f64 = 1.1e6;
/// Frames kept in the log.
pub const FRAME_CAP: usize = 300;
/// Links kept.
pub const LINK_CAP: usize = 100;
/// A frame that is not a sync frame is accepted for this long after a sync frame of its link, seconds.
pub const SYNC_HOLD_S: f64 = 10.0;
/// Two frames of equal bytes on one channel that end within this time are one frame, seconds.
pub const SAME_FRAME_S: f64 = 2e-3;
/// The frames of the test signal on one hop channel before the link moves to the next.
pub const GEN_HOP_FRAMES: u64 = 4;
/// What the stack says about its sources.
pub const NOT_CONFIRMED: &str =
    "experimental: not confirmed on a real signal. Frame layout, CRC start value, rate rows and \
                                 hop plans are not confirmed by a public document.";
/// What the stack says while no binding phrase is set.
pub const NO_PHRASE: &str = "stick values are shown only for a link whose binding phrase is set";

sdrkit_core::choice_enum! {
    /// The rate rows that the receiver searches. The keys are the packet rates.
    pub enum PacketRate {
        /// Every row.
        All = ("all", "All rates"),
        /// SF6, 8 bytes.
        Hz200 = ("200", "200 Hz"),
        /// SF7, 8 bytes.
        Hz100 = ("100", "100 Hz"),
        /// SF8, 8 bytes.
        Hz50 = ("50", "50 Hz"),
        /// SF9, 8 bytes.
        Hz25 = ("25", "25 Hz"),
        /// SF6, 13 bytes.
        Hz100Full = ("100full", "100 Hz Full"),
    }
}

impl PacketRate {
    /// The rows of [`ROWS`] that this choice selects.
    pub fn rows(self) -> std::ops::Range<usize> {
        match self {
            PacketRate::All => 0..ROWS.len(),
            PacketRate::Hz200 => 0..1,
            PacketRate::Hz100 => 1..2,
            PacketRate::Hz50 => 2..3,
            PacketRate::Hz25 => 3..4,
            PacketRate::Hz100Full => 4..5,
        }
    }
}

/// The count of hop channels. The default is the plan of the test signal.
pub const HOP_CHANNELS: Setting<field::I64> = Setting::new("hop_channels", "Hop channels", "", 4)
        .section("Link")
    .range(1.0, 80.0, 1.0)
    .help(
        "The count of channels of the hop plan of your area. The tuned frequency is the middle of the plan. The default \
         is the plan of the test signal.",
    );
/// The distance between two hop channels. The default is the plan of the test signal.
pub const HOP_STEP_HZ: Setting<field::Hz> =
    Setting::new("hop_step_hz", "Hop channel spacing", "Hz", 600e3)
        .section("Link")
        .range(100e3, 2e6, 1.0)
        .help("The distance between two hop channels. The default is the plan of the test signal.");
/// The rate rows searched. The default is the row of the test signal.
pub const PACKET_RATE: Setting<field::Choice<PacketRate>> =
    Setting::choice("packet_rate", "Packet rate", PacketRate::Hz100)
        .section("Link").help(
        "The packet rate of the links. All rates needs five times the processor time. The default is the rate of the \
         test signal.",
    );
/// The binding phrase of the user's own link.
pub const BINDING_PHRASE: Setting<field::Text> =
    Setting::new("binding_phrase", "Binding phrase (your own link)", "", "")
        .section("Link").help(
        "Secret. With it the stick values of your own link are shown. The stack keeps it in memory only: it is not in \
         the saved state, the log or the results. Empty forgets it.",
    );
/// The binding phrase of the link of the test signal.
pub const GEN_PHRASE: Setting<field::Text> = Setting::new(
    "gen_phrase",
    "Test signal binding phrase",
    "",
    "sdrkit test",
)
.section("Test signal")
.advanced();
/// The rate row of the test signal, 1 to 5.
pub const GEN_ROW: Setting<field::I64> = Setting::new("gen_row", "Test signal rate row", "", 2)
    .range(1.0, 5.0, 1.0)
    .section("Test signal")
    .advanced();
/// The test signal has inverted IQ.
pub const GEN_INVERTED: Setting<field::Bool> =
    Setting::new("gen_inverted_iq", "Test signal with inverted IQ", "", false)
        .section("Test signal")
        .advanced();
/// The test signal holds a second link that sends no sync frame.
pub const GEN_SECOND_LINK: Setting<field::Bool> = Setting::new(
    "gen_second_link",
    "Test signal with a second link without sync frames",
    "",
    false,
)
.section("Test signal")
.advanced();

/// The longest binding phrase, characters.
pub const PHRASE_MAX: usize = 63;

sdrkit_core::settings! {
    /// The settings of `elrs.channels`.
    pub enum ElrsPlanSetting {
        /// Hop channels.
        HopChannels = HOP_CHANNELS,
        /// Hop channel spacing.
        HopStepHz = HOP_STEP_HZ,
        /// Packet rate.
        PacketRate = PACKET_RATE,
    }
}

sdrkit_core::settings! {
    /// The settings of `elrs.decode`.
    pub enum ElrsDecodeSetting {
        /// Binding phrase.
        BindingPhrase = BINDING_PHRASE,
    }
}

sdrkit_core::settings! {
    /// The settings of `elrs`.
    pub enum ElrsSetting {
        /// Hop channels.
        HopChannels = HOP_CHANNELS,
        /// Hop channel spacing.
        HopStepHz = HOP_STEP_HZ,
        /// Packet rate.
        PacketRate = PACKET_RATE,
        /// Binding phrase.
        BindingPhrase = BINDING_PHRASE,
        /// Test signal binding phrase.
        GenPhrase = GEN_PHRASE,
        /// Test signal rate row.
        GenRow = GEN_ROW,
        /// Test signal with inverted IQ.
        GenInverted = GEN_INVERTED,
        /// Test signal with a second link.
        GenSecondLink = GEN_SECOND_LINK,
    }
}

/// The frequencies of a plan of `n` channels `step_hz` apart whose middle is `dial_hz`.
pub fn hop_frequencies(dial_hz: f64, n: usize, step_hz: f64) -> Vec<f64> {
    (0..n)
        .map(|i| dial_hz + (i as f64 - (n as f64 - 1.0) / 2.0) * step_hz)
        .collect()
}

/// The tag of the detector of hop channel `channel`, row `row` of [`ROWS`] and the IQ polarity.
fn tag_of(channel: usize, row: usize, inverted: bool) -> u32 {
    (channel as u32) << 8 | (row as u32) << 1 | u32::from(inverted)
}

/// The hop channel of a detector tag.
fn channel_of(tag: u32) -> u16 {
    (tag >> 8) as u16
}

/// The plan of `elrs` as an item (`elrs.plan`).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ElrsPlan {
    /// The hop channels, Hz.
    pub channels: Vec<f64>,
    /// Each hop channel is inside the capture.
    pub inside: Vec<bool>,
    /// The middle of the capture lies on a hop channel.
    pub on_centre: bool,
}

impl Item for ElrsPlan {
    const KIND: &'static str = "elrs.plan";
}

impl ElrsPlan {
    /// The hop channels inside the capture.
    pub fn inside_count(&self) -> usize {
        self.inside.iter().filter(|&&b| b).count()
    }
}

/// The planner of `elrs`: the hop channels around the dial, each with a detector for every selected row and both IQ
/// polarities.
pub struct ElrsPlanner {
    channels: usize,
    step_hz: f64,
    rate: PacketRate,
}

impl Default for ElrsPlanner {
    fn default() -> Self {
        ElrsPlanner {
            channels: HOP_CHANNELS.default as usize,
            step_hz: HOP_STEP_HZ.default,
            rate: PacketRate::Hz100,
        }
    }
}

/// The detector of one rate row.
fn detector(row: &RateRow, inverted: bool, tag: u32) -> DetectorConfig {
    let mut d = DetectorConfig::new(row.sf, elrs::BW_HZ, elrs::SYNC);
    d.preamble = row.preamble;
    d.implicit = Some(Header {
        len: row.bytes,
        cr: row.cr,
        crc: false,
    });
    d.ldro = Ldro::Off;
    d.invert = inverted;
    d.jump_search = false;
    d.tag = tag;
    d
}

impl Planner for ElrsPlanner {
    type Item = ElrsPlan;
    type Setting = ElrsPlanSetting;
    const NAME: &'static str = "elrs.channels";

    fn apply(&mut self, id: SettingId, v: &Value) -> Result<bool, SettingError> {
        match ElrsPlanSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            ElrsPlanSetting::HopChannels => {
                self.channels = usize::try_from(HOP_CHANNELS.read(id, v)?)
                    .unwrap_or(1)
                    .max(1);
            }
            ElrsPlanSetting::HopStepHz => self.step_hz = HOP_STEP_HZ.read(id, v)?,
            ElrsPlanSetting::PacketRate => self.rate = PACKET_RATE.read(id, v)?,
        }
        Ok(false)
    }

    fn values(&self) -> Values {
        values_of([
            (
                ElrsPlanSetting::HopChannels,
                Value::I64(self.channels as i64),
            ),
            (ElrsPlanSetting::HopStepHz, Value::Hz(self.step_hz)),
            (
                ElrsPlanSetting::PacketRate,
                Value::Choice(self.rate.index()),
            ),
        ])
    }

    /// The hop channels around the dial (the centre of the capture when the dial is not known).
    fn plan(&self, input: &Input) -> (BankPlan, ElrsPlan) {
        let dial = input.dial();
        let freqs = hop_frequencies(dial, self.channels, self.step_hz);
        let channels = freqs
            .iter()
            .enumerate()
            .map(|(i, &freq_hz)| BankChannel {
                freq_hz,
                bw: elrs::BW_HZ,
                detectors: self
                    .rate
                    .rows()
                    .flat_map(|r| {
                        [false, true].map(|inv| detector(&ROWS[r], inv, tag_of(i, r, inv)))
                    })
                    .collect(),
            })
            .collect();
        let inside: Vec<bool> = freqs
            .iter()
            .map(|f| in_band(input.rate, f - input.centre_hz, elrs::BW_HZ))
            .collect();
        let on_centre = freqs
            .iter()
            .any(|f| (f - input.centre_hz).abs() < elrs::BW_HZ / 2.0);
        let item = ElrsPlan {
            channels: freqs,
            inside,
            on_centre,
        };
        (
            BankPlan {
                channels,
                fft_from: 3,
            },
            item,
        )
    }
}

sdrkit_core::choice_enum! {
    /// What a frame is.
    #[derive(Serialize, Deserialize)]
    pub enum FrameKind {
        /// A sync frame.
        Sync = ("sync", "SYNC"),
        /// A data frame.
        Data = ("data", "DATA"),
        /// A radio-control frame or link statistics.
        Rc = ("rc", "RC or link stats"),
    }
}

/// One row of the frame log.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FrameRow {
    /// Signal time, seconds.
    pub time: f64,
    /// Wall time, seconds since 1970, when known.
    pub wall: Option<f64>,
    /// The link.
    pub link: String,
    /// What the frame is.
    pub kind: FrameKind,
    /// The frame in one line.
    pub content: String,
    /// The hop channel plus the carrier offset of the frame, Hz.
    pub freq_hz: f64,
    /// The index of the hop channel.
    pub hop_channel: u16,
    /// The spreading factor.
    pub sf: u8,
    /// The frame bytes.
    pub bytes: Vec<u8>,
    /// Inverted IQ.
    pub inverted: bool,
    /// The SNR, dB.
    pub snr_db: f64,
    /// The content is not shown.
    pub encrypted: bool,
}

impl Shift for FrameRow {
    fn shift_times(&mut self, by_s: f64) {
        self.time += by_s;
    }
}

fn hex(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(3 * bytes.len());
    for (i, b) in bytes.iter().enumerate() {
        if i > 0 {
            s.push(' ');
        }
        let _ = write!(s, "{b:02X}");
    }
    s
}

impl Shaped for FrameRow {
    type Row = ElrsFrameRow<'static>;
    fn row(&self, _seen: Option<&Seen>, _now_s: f64, out: RowOut<'_, '_, Self::Row>) {
        out.put(&ElrsFrameRow {
            utc: self.wall.map(Time),
            time_s: self.time,
            link: &self.link,
            kind: self.kind,
            content: &self.content,
            freq_hz: Hertz(self.freq_hz),
            hop_channel: i64::from(self.hop_channel),
            sf: i64::from(self.sf),
            bytes: count(self.bytes.len()),
            inverted: self.inverted,
            snr_db: self.snr_db,
            encrypted: self.encrypted,
            hex: &hex(&self.bytes),
        });
    }
}

/// One link of the table.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LinkRow {
    /// The link, 4 hex digits.
    pub id: String,
    /// What its last frame was.
    pub kind: FrameKind,
    /// Its last frame in one line.
    pub content: String,
    /// Its frames.
    pub frames: u64,
    /// Its sync frames.
    pub sync_frames: u64,
    /// The rate of its last frame.
    pub rate: String,
    /// The hop index of its last sync frame.
    pub hop_index: Option<u8>,
    /// The hop channels on which it was heard, in rising order.
    pub heard_on: Vec<u16>,
    /// The signal time of its last sync frame, seconds.
    pub last_sync: Option<f64>,
    /// The spreading factor and the frame size of its last sync frame: the rate row of the link.
    #[serde(default)]
    pub sync_row: Option<(u8, u8)>,
}

impl LinkRow {
    fn new(link: LinkId) -> Self {
        LinkRow {
            id: link.to_string(),
            kind: FrameKind::Sync,
            content: String::new(),
            frames: 0,
            sync_frames: 0,
            rate: String::new(),
            hop_index: None,
            heard_on: Vec::new(),
            last_sync: None,
            sync_row: None,
        }
    }
}

impl Shift for LinkRow {
    fn shift_times(&mut self, by_s: f64) {
        if let Some(t) = &mut self.last_sync {
            *t += by_s;
        }
    }
}

impl Shaped for LinkRow {
    type Row = ElrsLinkRow<'static>;
    fn row(&self, seen: Option<&Seen>, _now_s: f64, out: RowOut<'_, '_, Self::Row>) {
        let mut on = String::new();
        for (i, c) in self.heard_on.iter().enumerate() {
            let _ = write!(on, "{}{c}", if i > 0 { " " } else { "" });
        }
        out.put(&ElrsLinkRow {
            source: &self.id,
            kind: self.kind,
            content: &self.content,
            frames: count(self.frames),
            sync_frames: count(self.sync_frames),
            rate: &self.rate,
            hop_index: self.hop_index.map(i64::from),
            heard_on: &on,
            first_heard: columns::first_heard(seen).map(Time),
            last_heard: columns::last_heard(seen).map(Time),
        });
    }
}

/// What `elrs.decode` keeps across sessions. It never holds the binding phrase.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Heard {
    /// The frame log, oldest first.
    pub frames: LogRing<FrameRow>,
    /// The links.
    pub links: ObjectTable<LinkId, LinkRow>,
    /// Good frames.
    pub good: u64,
    /// Bad frames.
    pub bad: u64,
}

impl Default for Heard {
    fn default() -> Self {
        Heard {
            frames: LogRing::new(FRAME_CAP),
            links: ObjectTable::new(LINK_CAP),
            good: 0,
            bad: 0,
        }
    }
}

impl Shift for Heard {
    fn shift_times(&mut self, by_s: f64) {
        self.frames.shift_times(by_s);
        self.links.shift_times(by_s);
    }
}

impl Tables for Heard {
    const FORMAT: StateFormat = StateFormat::new("lora.elrs", 1);
    fn clear(&mut self) {
        self.frames.clear();
        self.links.clear();
        self.good = 0;
        self.bad = 0;
    }
    fn adopt(&mut self, saved: Self) {
        self.frames.adopt(saved.frames);
        self.links.adopt(saved.links);
        self.good = saved.good;
        self.bad = saved.bad;
    }
}

sdrkit_core::commands! {
    /// The commands of `elrs.decode`.
    pub enum ElrsCommand {
        /// Forgets the tables and the counters.
        Clear = CLEAR,
    }
}

/// One LoRa frame as the decoder takes it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Air<'a> {
    /// The frame bytes.
    pub bytes: &'a [u8],
    /// The signal time of its start, seconds.
    pub start_s: f64,
    /// The signal time of its end, seconds.
    pub end_s: f64,
    /// The index of its hop channel.
    pub hop_channel: u16,
    /// The hop channel plus the carrier offset, Hz.
    pub freq_hz: f64,
    /// The spreading factor.
    pub sf: u8,
    /// Inverted IQ.
    pub inverted: bool,
    /// The SNR, dB.
    pub snr_db: f64,
}

/// A frame taken a short time ago, to find a second copy of it and the false twin from the other row of its spreading factor.
#[derive(Clone, Copy, Debug)]
struct Recent {
    channel: u16,
    sf: u8,
    inverted: bool,
    start_s: f64,
    end_s: f64,
    bytes: [u8; elrs::LONG],
    len: usize,
    good: bool,
    snr_db: f64,
}

impl Recent {
    fn overlaps(&self, a: &Air<'_>) -> bool {
        self.channel == a.hop_channel
            && self.sf == a.sf
            && self.inverted == a.inverted
            && self.start_s < a.end_s
            && a.start_s < self.end_s
    }
}

/// The `elrs.decode` component.
pub struct ElrsDecode {
    /// What was heard.
    pub heard: Heard,
    p: Published,
    plan: ElrsPlan,
    phrase: Option<(LinkId, [u8; 6])>,
    recent: VecDeque<Recent>,
    now: f64,
    rate: f64,
    level: Option<f64>,
    preambles: u64,
    preambles_seen: u64,
    last_signal: f64,
    last_good: f64,
    last_snr: Option<f64>,
    live: bool,
    clock: Clock,
    events: Vec<Event>,
    text: String,
}

impl Default for ElrsDecode {
    fn default() -> Self {
        ElrsDecode {
            heard: Heard::default(),
            p: crate::typed::results("elrs"),
            plan: ElrsPlan::default(),
            phrase: None,
            recent: VecDeque::with_capacity(RECENT),
            now: 0.0,
            rate: 0.0,
            level: None,
            preambles: 0,
            preambles_seen: 0,
            last_signal: -1e9,
            last_good: -1e9,
            last_snr: None,
            live: false,
            clock: Clock::fixed(0.0),
            events: Vec::new(),
            text: String::new(),
        }
    }
}

const RECENT: usize = 32;

impl ElrsDecode {
    /// Sets the binding phrase of the user's own link. An empty phrase forgets it.
    pub fn set_phrase(&mut self, phrase: &str) {
        self.phrase = (!phrase.is_empty()).then(|| {
            let u = elrs::uid(phrase);
            (LinkId(u16::from_be_bytes([u[4], u[5]])), u)
        });
    }

    /// The link whose frames a frame that is not a sync frame fits, and the number of its CRC start value: the link of the
    /// phrase first, then each link with a sync frame in the last [`SYNC_HOLD_S`].
    fn link_of(&self, a: &Air<'_>) -> Option<(LinkId, u8)> {
        // A link sends every frame on the rate row of its sync frames.
        let row = (a.sf, a.bytes.len() as u8);
        let on_row = |l: &LinkRow| l.sync_row.is_none_or(|r| r == row);
        if let Some((link, _)) = self.phrase {
            if self.heard.links.get(&link).is_none_or(on_row) {
                if let Some(n) = elrs::nonce_of(a.bytes, link) {
                    return Some((link, n));
                }
            }
        }
        self.heard.links.iter_newest_first().find_map(|r| {
            let held = r
                .record
                .last_sync
                .is_some_and(|t| (0.0..=SYNC_HOLD_S).contains(&(a.end_s - t)));
            if !held || !on_row(r.record) {
                return None;
            }
            elrs::nonce_of(a.bytes, *r.key).map(|n| (*r.key, n))
        })
    }

    fn remember(&mut self, a: &Air<'_>, good: bool) {
        let mut bytes = [0u8; elrs::LONG];
        bytes[..a.bytes.len()].copy_from_slice(a.bytes);
        if self.recent.len() == RECENT {
            self.recent.pop_front();
        }
        self.recent.push_back(Recent {
            channel: a.hop_channel,
            sf: a.sf,
            inverted: a.inverted,
            start_s: a.start_s,
            end_s: a.end_s,
            bytes,
            len: a.bytes.len(),
            good,
            snr_db: a.snr_db,
        });
    }

    /// Takes one LoRa frame. A frame of another size than 8 or 13 bytes is ignored. A frame whose CRC fits no link is a bad
    /// frame, unless a good frame of the other row of its spreading factor lies at the same time on its channel.
    pub fn take(&mut self, a: &Air<'_>) {
        if !matches!(a.bytes.len(), elrs::SHORT | elrs::LONG) {
            return;
        }
        self.last_signal = a.end_s;
        // The two polarities or two rows can deliver one frame twice: the copy with the better SNR stays.
        if let Some(r) = self.recent.iter_mut().find(|r| {
            r.good
                && r.channel == a.hop_channel
                && r.bytes[..r.len] == *a.bytes
                && (r.end_s - a.end_s).abs() < SAME_FRAME_S
        }) {
            if a.snr_db > r.snr_db {
                r.snr_db = a.snr_db;
                if let Some(row) = self.heard.frames.newest_mut() {
                    if row.bytes == a.bytes {
                        row.snr_db = a.snr_db;
                        row.inverted = a.inverted;
                    }
                }
            }
            return;
        }
        let sync = elrs::parse_sync(a.bytes);
        // The other row of the spreading factor read a frame that is accepted: its bytes have no meaning.
        if sync.is_none()
            && self
                .recent
                .iter()
                .any(|r| r.good && r.len != a.bytes.len() && r.overlaps(a))
        {
            return;
        }
        let found = match sync {
            Some(s) => Some((s.link, s.nonce)),
            None if elrs::frame_type(a.bytes) == 2 => None,
            None => self.link_of(a),
        };
        let Some((link, n)) = found else {
            if !self.recent.iter().any(|r| r.good && r.overlaps(a)) {
                self.heard.bad += 1;
                self.remember(a, false);
            }
            return;
        };
        // The false twin of this frame came first and was counted.
        for r in self.recent.iter_mut().filter(|r| !r.good && r.overlaps(a)) {
            self.heard.bad = self.heard.bad.saturating_sub(1);
            r.end_s = r.start_s;
        }
        self.remember(a, true);
        self.heard.good += 1;
        self.last_good = a.end_s;
        self.last_snr = Some(a.snr_db);
        let own = self.phrase.is_some_and(|(l, _)| l == link);
        let (kind, content, encrypted) = match sync {
            Some(s) => (
                FrameKind::Sync,
                format!(
                    "hop {}, nonce {}, rate index {}",
                    s.hop, s.nonce, s.rate_index
                ),
                false,
            ),
            None => {
                let kind = if elrs::frame_type(a.bytes) == 1 {
                    FrameKind::Data
                } else {
                    FrameKind::Rc
                };
                match elrs::sticks(a.bytes).filter(|_| own && elrs::frame_type(a.bytes) == 0) {
                    Some((s, armed)) => (
                        kind,
                        format!(
                            "ch1-4 {} {} {} {}, armed {}",
                            s[0],
                            s[1],
                            s[2],
                            s[3],
                            u8::from(armed)
                        ),
                        false,
                    ),
                    None => (kind, format!("{} bytes, nonce {n}", a.bytes.len()), true),
                }
            }
        };
        let stamp = if self.live {
            Stamp::at(a.end_s).with_wall(self.clock.now())
        } else {
            Stamp::at(a.end_s)
        };
        self.heard.frames.push(FrameRow {
            time: a.end_s,
            wall: stamp.wall,
            link: link.to_string(),
            kind,
            content: content.clone(),
            freq_hz: a.freq_hz,
            hop_channel: a.hop_channel,
            sf: a.sf,
            bytes: a.bytes.to_vec(),
            inverted: a.inverted,
            snr_db: a.snr_db,
            encrypted,
        });
        let Ok(up) = self.heard.links.upsert(&link, stamp, || LinkRow::new(link)) else {
            return;
        };
        if up.new {
            self.events.push(ELRS_LINK.event(
                a.end_s,
                a.freq_hz,
                &ElrsLinkEvent {
                    source: &link.to_string(),
                    family: Family::Other,
                },
            ));
        }
        let l = up.row;
        l.kind = kind;
        l.content = content;
        l.frames += 1;
        l.rate = ROWS
            .iter()
            .find(|r| r.sf == a.sf && usize::from(r.bytes) == a.bytes.len())
            .map_or_else(String::new, |r| r.name.to_string());
        if let Some(s) = sync {
            l.sync_frames += 1;
            l.hop_index = Some(s.hop);
            l.last_sync = Some(a.end_s);
            l.sync_row = Some((a.sf, a.bytes.len() as u8));
        }
        if let Err(at) = l.heard_on.binary_search(&a.hop_channel) {
            l.heard_on.insert(at, a.hop_channel);
        }
    }

    fn publish(&mut self) {
        let h = &self.heard;
        let state = if self.now - self.last_good < 8.0 {
            LockState::Locked
        } else if self.now - self.last_signal < 2.0 {
            LockState::Tracking
        } else {
            LockState::Searching
        };
        let (inside, total) = (self.plan.inside_count(), self.plan.channels.len());
        let t = &mut self.text;
        t.clear();
        if h.good > 0 {
            let _ = write!(t, "{} frames", h.good);
        } else {
            t.push_str("listening for ExpressLRS");
        }
        let _ = write!(
            t,
            ", {inside} of {total} hop channels inside the capture; experimental: frame layout, CRC start value, rate rows and \
             hop plans not confirmed by a public document"
        );
        let p = &mut self.p;
        show_status(p, state, self.last_snr, None, self.level, t);
        t.clear();
        t.push_str(NOT_CONFIRMED);
        if self.phrase.is_none() {
            let _ = write!(t, " {NO_PHRASE}.");
        }
        if self.plan.on_centre {
            t.push_str(" The centre of the capture lies on a hop channel.");
        }
        p.put(&ElrsFields {
            frames_ok: count(h.good),
            frames_bad: count(h.bad),
            preambles: count(self.preambles),
            phrase_set: self.phrase.is_some(),
            hop_channels_inside: count(inside),
            hop_channels: count(total),
            snr_db: self.last_snr,
            level_dbfs: self.level,
            links_heard: count(h.links.len()),
            note: t,
        });
        shaped::log_rows(&h.frames, &ELRS_FRAMES, self.now, p);
        shaped::rows(&h.links, &ELRS_LINKS, self.now, p);
    }
}

impl Component for ElrsDecode {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "elrs.decode",
            vec![
                Port::typed::<LoraFrame>("frames"),
                Port::typed::<BankState>("state"),
                Port::typed::<ElrsPlan>("plan"),
            ],
            vec![Port::new("events", Kind::Messages).rate_from(RateFrom::Fixed(0.0))],
        )
        .with_settings(ElrsDecodeSetting::schema())
    }

    fn configure(&mut self, _inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        Ok(vec![Format::new(Kind::Messages, 0.0)])
    }

    fn attach(&mut self, ctx: &RunContext) {
        self.live = ctx.live;
        self.clock = ctx.clock.clone();
    }

    fn process(&mut self, io: &mut Io) {
        if let Some(pl) = io.items::<ElrsPlan>(2).last() {
            if pl.value != self.plan {
                let (k, n) = (pl.value.inside_count(), pl.value.channels.len());
                let line = format!("ExpressLRS: {k} of {n} hop channels inside the capture");
                self.p.results_mut().log(line);
            }
            self.plan = pl.value.clone();
        }
        for s in io.items::<BankState>(1) {
            self.now = s.value.time;
            self.rate = s.value.rate;
            self.level = Some(s.value.level_db);
            // A preamble counts when its sync word follows. Noise gives some: the state follows the frames only.
            let n: u64 = s
                .value
                .detectors
                .iter()
                .map(|d| d.stats.preambles.saturating_sub(d.stats.sync_failed))
                .sum();
            self.preambles += n.saturating_sub(self.preambles_seen);
            self.preambles_seen = n;
        }
        for f in io.items::<LoraFrame>(0) {
            let fr = &f.value;
            let rate = self.rate.max(1.0);
            let end_s = f.time;
            let start_s = end_s - fr.end.saturating_sub(fr.start) as f64 / rate;
            self.take(&Air {
                bytes: &fr.payload,
                start_s,
                end_s,
                hop_channel: channel_of(fr.tag),
                freq_hz: fr.channel_hz + fr.cfo_hz,
                sf: fr.sf,
                inverted: fr.inverted,
                snr_db: fr.snr_db,
            });
        }
        if io.tick() {
            self.publish();
        }
        if !self.events.is_empty() {
            io.messages_out(0).append(&mut self.events);
        }
    }

    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        match ElrsDecodeSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            ElrsDecodeSetting::BindingPhrase => {
                let text = BINDING_PHRASE.read(id, v)?;
                if text.chars().count() > PHRASE_MAX {
                    return Err(SettingError::Malformed {
                        id,
                        at: Some(PHRASE_MAX),
                    });
                }
                self.set_phrase(text);
            }
        }
        Ok(Applied::Live)
    }

    /// The binding phrase is not given back: the value is empty.
    fn values(&self) -> Values {
        values_of([(ElrsDecodeSetting::BindingPhrase, Value::Text(String::new()))])
    }

    fn commands(&self) -> &Commands {
        ElrsCommand::commands()
    }

    fn invoke(&mut self, id: CommandId, _args: &Values) -> Result<Option<Record>, CommandError> {
        match ElrsCommand::from_id(id).ok_or(CommandError::NoSuchId { id })? {
            ElrsCommand::Clear => {
                self.heard.clear();
                self.recent.clear();
                self.preambles = 0;
                self.last_snr = None;
                self.last_good = -1e9;
                self.p.clear_rows(&ELRS_FRAMES);
                self.p.clear_rows(&ELRS_LINKS);
            }
        }
        self.publish();
        Ok(None)
    }

    fn results(&self) -> Option<&Results> {
        Some(self.p.results())
    }

    fn forget(&mut self, kept: &Kept) {
        let h = &mut self.heard;
        if !kept.table(ELRS_FRAMES.key) {
            h.frames.clear();
            self.p.clear_rows(&ELRS_FRAMES);
        }
        if !kept.table(ELRS_LINKS.key) {
            h.links.clear();
            self.p.clear_rows(&ELRS_LINKS);
        }
        if !kept.field("frames_ok") {
            h.good = 0;
        }
        if !kept.field("frames_bad") {
            h.bad = 0;
        }
        if !kept.field("preambles") {
            self.preambles = 0;
        }
        self.recent.clear();
        self.level = None;
        self.last_snr = None;
        self.last_signal = -1e9;
        self.last_good = -1e9;
        self.publish();
    }

    fn save_state(&self) -> Option<StateBlob> {
        self.heard.save_state(self.now).ok()
    }

    fn restore_state(&mut self, state: &StateBlob) -> Result<(), StateError> {
        let now = self.now;
        self.heard.restore_state(state, now)?;
        self.publish();
        Ok(())
    }
}

// ---------------------------------------------------------------------------------------------------------------------------------
// The test signal.

/// The LoRa setting of a sender on rate row `row`.
pub fn tx_params(row: &RateRow, inverted: bool) -> LoraParams {
    let mut p = LoraParams::new(row.sf, elrs::BW_HZ, row.cr, row.preamble, elrs::SYNC);
    p.implicit = true;
    p.implicit_len = row.bytes;
    p.crc = false;
    p.ldro = Ldro::Off;
    p.invert_iq = inverted;
    p
}

/// What the test signal holds.
#[derive(Clone, Debug, PartialEq)]
pub struct Scene {
    /// The hop channels as offsets from the centre of the samples, Hz.
    pub offsets: Vec<f64>,
    /// The binding phrase of the link.
    pub phrase: String,
    /// The row of [`ROWS`], 0 to 4.
    pub row: usize,
    /// Inverted IQ.
    pub inverted: bool,
    /// A second link that sends no sync frame.
    pub second_link: bool,
}

/// The binding phrase of the second link of the test signal.
pub const SECOND_PHRASE: &str = "sdrkit second link";

impl Scene {
    /// The hop channels inside a capture at `rate`: (index in the plan, offset).
    pub fn inside(&self, rate: f64) -> Vec<(usize, f64)> {
        self.offsets
            .iter()
            .copied()
            .enumerate()
            .filter(|(_, o)| in_band(rate, *o, elrs::BW_HZ))
            .collect()
    }

    /// Where frame `k` is in the walk over the hop channels `inside`, `ahead` channels after the link of the scene.
    fn walk(&self, k: u64, inside: &[(usize, f64)], ahead: u64) -> usize {
        let middle = self.offsets.len() / 2;
        let first = inside.iter().position(|(c, _)| *c == middle).unwrap_or(0);
        ((k / GEN_HOP_FRAMES + first as u64 + ahead) % inside.len().max(1) as u64) as usize
    }

    /// Frame `k` of the link: (index of the hop channel in the plan, offset, bytes). The link walks over the hop channels
    /// inside the capture in rising order with [`GEN_HOP_FRAMES`] frames on each, from the middle channel of the plan when
    /// that is inside. The first frame on the middle channel is a sync frame. `None` when no hop channel is inside the
    /// capture.
    pub fn frame(&self, k: u64, rate: f64) -> Option<(usize, f64, Vec<u8>)> {
        let inside = self.inside(rate);
        let middle = self.offsets.len() / 2;
        let at = self.walk(k, &inside, 0);
        let &(channel, offset) = inside.get(at)?;
        let row = &ROWS[self.row.min(ROWS.len() - 1)];
        let len = usize::from(row.bytes);
        let uid = elrs::uid(&self.phrase);
        let link = LinkId(u16::from_be_bytes([uid[4], uid[5]]));
        let n = k as u8;
        let bytes = if channel == middle && k.is_multiple_of(GEN_HOP_FRAMES) {
            elrs::build_sync(&uid, at as u8, n, self.row as u8, len)
        } else {
            let t = k as f64 * row.interval_s;
            let stick =
                |i: usize| (512.0 + 400.0 * (0.4 * t + 1.5 * i as f64).sin()).round() as u16;
            elrs::build_rc(
                link,
                n,
                std::array::from_fn(stick),
                true,
                len,
                [1, 2, 3, 4, 5],
            )
        };
        Some((channel, offset, bytes))
    }

    /// Frame `k` of the second link: a radio-control frame two hop channels after the first link, half a frame time later.
    pub fn second_frame(&self, k: u64, rate: f64) -> Option<(f64, Vec<u8>)> {
        let inside = self.inside(rate);
        let at = self.walk(k, &inside, 2);
        let &(_, offset) = inside.get(at)?;
        let row = &ROWS[self.row.min(ROWS.len() - 1)];
        let link = elrs::link_of(SECOND_PHRASE);
        let bytes = elrs::build_rc(
            link,
            k as u8,
            [300, 400, 500, 600],
            false,
            usize::from(row.bytes),
            [0; 5],
        );
        Some((offset, bytes))
    }
}

/// The test signal of `elrs`: the scene with the middle of the hop plan at the centre of the samples.
pub fn generator(rate: f64, scene: Scene) -> SceneGen {
    let row = ROWS[scene.row.min(ROWS.len() - 1)];
    let p = tx_params(&row, scene.inverted);
    let per_cycle = (1.0 / row.interval_s).round() as u64;
    let span = scene
        .offsets
        .iter()
        .fold(0.0f64, |m, o| m.max(o.abs()))
        .mul_add(2.0, elrs::BW_HZ);
    SceneGen::new(
        rate,
        1.0,
        SignalInfo::new(span),
        Box::new(move |cycle, t0, bursts| {
            for j in 0..per_cycle {
                let k = u64::from(cycle) * per_cycle + j;
                let at = t0 + 0.002 + j as f64 * row.interval_s;
                if let Some((_, offset, bytes)) = scene.frame(k, rate) {
                    if let Ok(f) = TxFrame::new(&p, &bytes) {
                        bursts.push(f.at(at).carrier(offset + 400.0).amplitude(AMP).burst(rate));
                    }
                }
                if !scene.second_link {
                    continue;
                }
                if let Some((offset, bytes)) = scene.second_frame(k, rate) {
                    if let Ok(f) = TxFrame::new(&p, &bytes) {
                        let at = at + row.interval_s / 2.0;
                        bursts.push(f.at(at).carrier(offset - 300.0).amplitude(AMP).burst(rate));
                    }
                }
            }
        }),
    )
}

/// The generator of `elrs` from a configuration whose settings follow [`ElrsSetting`].
pub fn from_config(cfg: &GenConfig) -> SceneGen {
    let s = &cfg.settings;
    let n = usize::try_from(s.get(&HOP_CHANNELS)).unwrap_or(1).max(1);
    generator(
        cfg.rate_hz,
        Scene {
            offsets: hop_frequencies(0.0, n, s.get(&HOP_STEP_HZ)),
            phrase: s.get(&GEN_PHRASE).to_string(),
            row: usize::try_from(s.get(&GEN_ROW) - 1).unwrap_or(1),
            inverted: s.get(&GEN_INVERTED),
            second_link: s.get(&GEN_SECOND_LINK),
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_default_plan_fits_the_capture_of_the_stack() {
        let rate = crate::ELRS.tuning.rate_hz;
        let f = hop_frequencies(0.0, HOP_CHANNELS.default as usize, HOP_STEP_HZ.default);
        assert_eq!(f, [-900e3, -300e3, 300e3, 900e3]);
        assert!(f.iter().all(|o| in_band(rate, *o, elrs::BW_HZ)));
        assert!(crate::ELRS.tuning.min_rate_hz >= 2.2 * elrs::BW_HZ);
    }
}
