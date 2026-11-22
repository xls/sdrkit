//! The interface of the weak-signal family: the shapes of the result sets, the table rows and the events, the tables,
//! the event types, the commands and what each stack declares.
//!
//! The slot stacks (`wsjt` and the twelve single stacks) share the shape of the event type `wsjt.decode`. FSK441,
//! JT6M, ISCAT and PI4 each list the type with a shape of their own.

use crate::components::Js8Speed;
use crate::mode::Mode;
use crate::rx::slot::How;
use crate::slots::TimeSource;
use crate::table::{Js8State, ModeState};
use sdrkit_core::command::Command;
use sdrkit_core::interface::{
    Arguments, CommandKind, EventKind, Family as Described, Fields, Hertz, Seconds, StackShape,
    Table, Time,
};
use sdrkit_core::publish::field;
use sdrkit_core::publish::keys::{self, Family};
use sdrkit_core::publish::SeriesDef;
use sdrkit_core::settings::Argument;
use sdrkit_core::{SortOrder, TxContent};
use sdrkit_util_objects::commands as objcmd;
use sdrkit_util_objects::publish::columns;
use std::sync::OnceLock;

// The display: the status component of every slot stack.

/// What a slot stack can fill, from its modes. The description of the stack, the declaration of its display and the
/// writers of the display all read it, so a stack declares no table, event or field that stays empty.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Gives {
    /// A mode other than JS8: the decode table, the decode event and the count of the decodes.
    pub decodes: bool,
    /// WSPR or FST4W: the table of the WSPR spots.
    pub wspr: bool,
    /// JS8: the frames, the messages, the stations and the waterfall of JS8, with their events and fields.
    pub js8: bool,
    /// A mode that the stack sends: the transmit event.
    pub transmits: bool,
}

impl Gives {
    /// What the slot stack of `mode` (`None`: the combined stack `wsjt`) can fill.
    pub fn of(mode: Option<Mode>) -> Gives {
        let has = |f: fn(Mode) -> bool| mode.map_or_else(|| Mode::ALL.iter().any(|m| f(*m)), f);
        Gives {
            decodes: has(|m| m != Mode::Js8),
            wspr: has(|m| matches!(m, Mode::Wspr | Mode::Fst4w)),
            js8: has(|m| m == Mode::Js8),
            transmits: crate::settings::transmits_too(mode),
        }
    }
}

/// The result fields of a slot stack: the slot clock and the clock estimate from the decodes.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct WsjtFields {
    /// The time source of the slot clock. It has no value before the clock reports.
    #[field(label = "Slot clock")]
    pub time_source: Option<TimeSource>,
    /// The UTC of the newest audio.
    #[field(label = "UTC")]
    pub utc: Time,
    /// The median DT of the recent decodes. It has no value without a recent decode.
    #[field(label = "Clock error", unit = "s")]
    pub clock_error_s: Option<f64>,
    /// The spread of the recent DTs. It has no value without a recent decode.
    #[field(label = "Clock spread", unit = "s")]
    pub clock_spread_s: Option<f64>,
    /// The decodes that the clock estimate uses.
    #[field(label = "Clock decodes")]
    pub clock_decodes: i64,
    /// The clock is more than 1 s off.
    #[field(label = "Clock warning")]
    pub clock_warning: bool,
}

/// The result field of a slot stack with a mode other than JS8: the count of the decodes.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct WsjtDecodeFields {
    /// Decodes since Clear.
    #[field(label = "Decodes")]
    pub decodes_total: i64,
}

/// The result fields of a slot stack with JS8.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct Js8Fields {
    /// JS8 frames since Clear.
    #[field(label = "JS8 frames")]
    pub js8_frames: i64,
    /// JS8 candidates that did not decode since Clear.
    #[field(label = "JS8 candidates failed")]
    pub js8_failed: i64,
    /// What JS8 has heard.
    #[field(label = "JS8")]
    pub js8_state: Js8State,
    /// The sequence number of the newest line of the JS8 waterfall. It has no value before the first line.
    #[field(label = "Waterfall line")]
    pub js8_waterfall_seq: Option<i64>,
}

/// A row of the decode table. The row key is the place of the decode in the log.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct DecodeRow<'a> {
    /// The UTC of the slot.
    #[field(label = "UTC")]
    pub utc: Time,
    /// The SNR in 2500 Hz.
    #[field(shared = keys::SNR_DB)]
    pub snr_db: f64,
    /// The time offset from the slot start.
    #[field(label = "DT", unit = "s")]
    pub dt_s: f64,
    /// The audio frequency.
    #[field(label = "Audio", unit = "Hz")]
    pub audio_hz: Hertz,
    /// The amateur band of the dial, if the dial is in one.
    #[field(label = "Band")]
    pub band: Option<&'a str>,
    /// The dial frequency, if the slot has one.
    #[field(label = "Dial", unit = "Hz")]
    pub dial_hz: Option<Hertz>,
    /// The mode with its sub-mode and period ("JT65B").
    #[field(label = "Mode")]
    pub mode: &'a str,
    /// The decoded message.
    #[field(label = "Message")]
    pub message: &'a str,
    /// The decode was found on the other sideband.
    #[field(label = "Upside down")]
    pub mirrored: bool,
    /// How the decode was made.
    #[field(label = "How")]
    pub how: How,
    /// The decode is from the first part of a slot. The decode of the whole slot replaces it.
    #[field(label = "Early")]
    pub early: bool,
}

/// A row of the WSPR and FST4W spots. The row key is the place of the spot in the log.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct WsprSpotRow<'a> {
    /// The UTC of the slot.
    #[field(label = "UTC")]
    pub utc: Time,
    /// The SNR in 2500 Hz.
    #[field(shared = keys::SNR_DB)]
    pub snr_db: f64,
    /// The time offset from the slot start.
    #[field(label = "DT", unit = "s")]
    pub dt_s: f64,
    /// The audio frequency.
    #[field(label = "Audio", unit = "Hz")]
    pub audio_hz: Hertz,
    /// The callsign of the beacon.
    #[field(label = "Call")]
    pub callsign: &'a str,
    /// The locator of the beacon.
    #[field(shared = keys::LOCATOR)]
    pub locator: &'a str,
    /// The power that the beacon states.
    #[field(label = "Power", unit = "dBm")]
    pub power_dbm: i64,
    /// The decoded message.
    #[field(label = "Message")]
    pub message: &'a str,
    /// The centre of the locator, if the spot holds one.
    #[field(shared = keys::POSITION)]
    pub position: Option<(f64, f64)>,
}

/// A row of the modes table: the counters and the state of one mode heard, or of one JS8 speed. The row key is the
/// mode id, or `js8n`, `js8f` and `js8t`.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct ModeRow<'a> {
    /// The mode with its sub-mode and period.
    #[field(label = "Mode")]
    pub mode: &'a str,
    /// The T/R period.
    #[field(label = "Period", unit = "s")]
    pub period: Seconds,
    /// The slots decoded.
    #[field(label = "Slots")]
    pub slots: i64,
    /// Decodes since Clear.
    #[field(label = "Decodes")]
    pub decodes: i64,
    /// The decodes of the last slot.
    #[field(label = "Last slot")]
    pub last_count: i64,
    /// The UTC of the last slot decoded. It has no value before the first slot.
    #[field(label = "UTC")]
    pub utc: Option<Time>,
    /// The time that the last slot took to decode.
    #[field(label = "Decoding time", unit = "ms")]
    pub decode_ms: f64,
    /// Whether the mode decodes.
    #[field(label = "State")]
    pub state: ModeState,
}

/// A row of the station table: a station heard in any mode. The row key is the callsign.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct StationRow<'a> {
    /// The callsign.
    #[field(label = "Call")]
    pub callsign: &'a str,
    /// The locator, if one was heard.
    #[field(shared = keys::LOCATOR)]
    pub locator: Option<&'a str>,
    /// The mode of the newest decode.
    #[field(label = "Mode")]
    pub mode: Mode,
    /// The SNR of the newest decode.
    #[field(shared = keys::SNR_DB)]
    pub snr_db: f64,
    /// The radio frequency of the newest decode.
    #[field(shared = keys::FREQ_HZ)]
    pub freq_hz: Hertz,
    /// The times heard.
    #[field(shared = columns::HEARD)]
    pub heard: Option<i64>,
    /// The UTC of the last decode.
    #[field(shared = columns::LAST_HEARD)]
    pub last_heard: Option<Time>,
    /// The centre of the locator, if one was heard.
    #[field(shared = keys::POSITION)]
    pub position: Option<(f64, f64)>,
}

/// A row of the JS8 frames. The row key is the place of the frame in the log.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct Js8FrameRow<'a> {
    /// The UTC of the slot.
    #[field(label = "UTC")]
    pub utc: Time,
    /// The speed of the frame.
    #[field(label = "Sub-mode")]
    pub submode: Option<Js8Speed>,
    /// The SNR in 2500 Hz.
    #[field(shared = keys::SNR_DB)]
    pub snr_db: f64,
    /// The time offset from the slot start.
    #[field(label = "DT", unit = "s")]
    pub dt_s: f64,
    /// The audio frequency.
    #[field(label = "Audio", unit = "Hz")]
    pub audio_hz: Hertz,
    /// The text of the frame.
    #[field(label = "Frame")]
    pub frame: &'a str,
}

/// A row of the JS8 messages. The row key is the place of the message in the log.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct Js8MessageRow<'a> {
    /// The UTC of the first frame.
    #[field(label = "UTC")]
    pub utc: Time,
    /// The SNR in 2500 Hz.
    #[field(shared = keys::SNR_DB)]
    pub snr_db: f64,
    /// The audio frequency.
    #[field(label = "Audio", unit = "Hz")]
    pub audio_hz: Hertz,
    /// The message.
    #[field(label = "Message")]
    pub message: &'a str,
}

/// A row of the JS8 stations. The row key is the callsign.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct Js8StationRow<'a> {
    /// The callsign.
    #[field(label = "Call")]
    pub callsign: &'a str,
    /// The locator, if one was heard.
    #[field(shared = keys::LOCATOR)]
    pub locator: Option<&'a str>,
    /// The SNR of the newest frame.
    #[field(shared = keys::SNR_DB)]
    pub snr_db: f64,
    /// The audio frequency of the newest frame.
    #[field(label = "Audio", unit = "Hz")]
    pub audio_hz: Hertz,
    /// The UTC of the newest frame.
    #[field(label = "UTC")]
    pub utc: Time,
    /// The newest text of the station.
    #[field(label = "Last")]
    pub last_text: &'a str,
    /// The UTC of the last frame.
    #[field(shared = columns::LAST_HEARD)]
    pub last_heard: Option<Time>,
    /// The centre of the locator, if one was heard.
    #[field(shared = keys::POSITION)]
    pub position: Option<(f64, f64)>,
}

/// The decodes, newest first. An early decode is replaced in place by the same message of the whole slot.
pub static DECODES: Table<DecodeRow<'static>> =
    Table::new("decodes", "Decodes", crate::table::DECODES_CAP)
        .sorted(&DecodeRow::UTC, SortOrder::Descending);
/// The WSPR and FST4W spots, newest first.
pub static WSPR_SPOTS: Table<WsprSpotRow<'static>> =
    Table::new("wspr_spots", "WSPR spots", crate::table::DECODES_CAP)
        .label(&WsprSpotRow::CALLSIGN)
        .sorted(&WsprSpotRow::UTC, SortOrder::Descending);
/// One row per mode heard (per speed for JS8).
pub static MODES: Table<ModeRow<'static>> = Table::new("modes", "Modes", 32);
/// The stations heard, by callsign, in every mode.
pub static STATIONS: Table<StationRow<'static>> =
    Table::new("stations", "Stations", crate::table::STATIONS_CAP)
        .label(&StationRow::CALLSIGN)
        .sorted(&StationRow::LAST_HEARD, SortOrder::Descending);
/// The JS8 frames, newest first.
pub static JS8_FRAMES: Table<Js8FrameRow<'static>> =
    Table::new("js8_frames", "JS8 frames", crate::table::JS8_FRAMES_CAP)
        .sorted(&Js8FrameRow::UTC, SortOrder::Descending);
/// The JS8 messages, newest first.
pub static JS8_MESSAGES: Table<Js8MessageRow<'static>> = Table::new(
    "js8_messages",
    "JS8 messages",
    crate::table::JS8_MESSAGES_CAP,
)
.sorted(&Js8MessageRow::UTC, SortOrder::Descending);
/// The JS8 stations.
pub static JS8_STATIONS: Table<Js8StationRow<'static>> = Table::new(
    "js8_stations",
    "JS8 stations",
    crate::table::JS8_STATIONS_CAP,
)
.label(&Js8StationRow::CALLSIGN)
.sorted(&Js8StationRow::LAST_HEARD, SortOrder::Descending);

/// The JS8 waterfall: (Hz, dB) of the newest line.
pub const WATERFALL: SeriesDef = SeriesDef {
    key: crate::table::WATERFALL_SERIES,
    label: "JS8 waterfall",
    x_unit: "Hz",
    y_unit: "dB",
};

/// The fields of a decode of any mode but JS8.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct DecodeEvent<'a> {
    /// The mode.
    #[field(label = "Mode")]
    pub mode: Mode,
    /// The decoded message.
    #[field(shared = keys::TEXT)]
    pub text: &'a str,
    /// The SNR in 2500 Hz.
    #[field(shared = keys::SNR_DB)]
    pub snr_db: f64,
    /// The time offset from the slot start.
    #[field(label = "DT", unit = "s")]
    pub dt_s: f64,
    /// The audio frequency.
    #[field(label = "Audio", unit = "Hz")]
    pub audio_hz: Hertz,
    /// The calling station, if the message names one.
    #[field(shared = keys::CALLSIGN)]
    pub callsign: Option<&'a str>,
    /// The locator, if the message holds one.
    #[field(shared = keys::LOCATOR)]
    pub locator: Option<&'a str>,
    /// The message is a CQ.
    #[field(label = "CQ")]
    pub cq: bool,
    /// The decode was found on the other sideband.
    #[field(label = "Upside down")]
    pub mirrored: bool,
    /// How the decode was made.
    #[field(label = "How")]
    pub how: How,
    /// The power that a WSPR or FST4W station states.
    #[field(label = "Power", unit = "dBm")]
    pub power_dbm: Option<i64>,
    /// The calling station, if the message names one.
    #[field(shared = keys::SOURCE)]
    pub source: Option<&'a str>,
    /// The centre of the locator, if the message holds one.
    #[field(shared = keys::POSITION)]
    pub position: Option<(f64, f64)>,
    /// The family of the source: a message.
    #[field(shared = keys::FAMILY)]
    pub family: Family,
}

/// The fields of a station heard, with the standard spot fields.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct SpotEvent<'a> {
    /// The callsign.
    #[field(shared = keys::CALLSIGN)]
    pub callsign: &'a str,
    /// The locator, if one was heard.
    #[field(shared = keys::LOCATOR)]
    pub locator: Option<&'a str>,
    /// The SNR in 2500 Hz.
    #[field(shared = keys::SNR_DB)]
    pub snr_db: f64,
    /// The radio frequency.
    #[field(shared = keys::FREQ_HZ)]
    pub freq_hz: Hertz,
    /// The emission as spot networks name it.
    #[field(shared = keys::SPOT_MODE)]
    pub mode: &'a str,
    /// The callsign.
    #[field(shared = keys::SOURCE)]
    pub source: &'a str,
    /// The family of the source: a message.
    #[field(shared = keys::FAMILY)]
    pub family: Family,
}

/// The fields of a JS8 frame.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct Js8FrameEvent<'a> {
    /// The text of the frame.
    #[field(shared = keys::TEXT)]
    pub text: &'a str,
    /// The twelve characters of the frame.
    #[field(label = "Characters")]
    pub chars: &'a str,
    /// The SNR in 2500 Hz.
    #[field(shared = keys::SNR_DB)]
    pub snr_db: f64,
    /// The speed of the frame.
    #[field(label = "Sub-mode")]
    pub submode: Option<Js8Speed>,
    /// The sending station, if the frame names one.
    #[field(shared = keys::SOURCE)]
    pub source: Option<&'a str>,
    /// The station or group addressed, if the frame names one.
    #[field(shared = keys::TARGET)]
    pub target: Option<&'a str>,
    /// The frame is a CQ: a heartbeat to all with a CQ.
    #[field(label = "CQ")]
    pub cq: bool,
    /// The family of the source: a message.
    #[field(shared = keys::FAMILY)]
    pub family: Family,
}

/// The fields of a JS8 message, sent when its last frame came or when it stopped.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct Js8MessageEvent<'a> {
    /// The message.
    #[field(shared = keys::TEXT)]
    pub text: &'a str,
    /// The last frame of the message came.
    #[field(label = "Complete")]
    pub complete: bool,
    /// The sending station, if it is known.
    #[field(shared = keys::SOURCE)]
    pub source: Option<&'a str>,
    /// The station or group addressed, if it is known.
    #[field(shared = keys::TARGET)]
    pub target: Option<&'a str>,
    /// The message is a CQ.
    #[field(label = "CQ")]
    pub cq: bool,
    /// The family of the source: a message.
    #[field(shared = keys::FAMILY)]
    pub family: Family,
}

/// The fields of a message to send.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct TxEvent<'a> {
    /// The mode.
    #[field(label = "Mode")]
    pub mode: Mode,
    /// The message.
    #[field(shared = keys::TEXT)]
    pub text: &'a str,
    /// The audio frequency.
    #[field(label = "Audio", unit = "Hz")]
    pub audio_hz: Hertz,
    /// The UTC of the start of the transmit slot.
    #[field(label = "Slot")]
    pub slot: Time,
    /// The T/R period.
    #[field(label = "Period", unit = "s")]
    pub period: Seconds,
    /// The family of the source: a message.
    #[field(shared = keys::FAMILY)]
    pub family: Family,
}

/// A decode of any mode but JS8.
pub static DECODE: EventKind<DecodeEvent<'static>> = EventKind::new("wsjt.decode", "Decode");
/// A station heard.
pub static SPOT: EventKind<SpotEvent<'static>> = EventKind::new("wsjt.spot", "Station heard");
/// A JS8 frame.
pub static JS8_FRAME: EventKind<Js8FrameEvent<'static>> = EventKind::new("js8.frame", "JS8 frame");
/// A JS8 message.
pub static JS8_MESSAGE: EventKind<Js8MessageEvent<'static>> =
    EventKind::new("js8.message", "JS8 message");
/// A message to send. It leaves the graph as this event.
pub static TX: EventKind<TxEvent<'static>> = EventKind::new("wsjt.tx", "Message to send");

// The commands.

/// The row key of a station.
pub const STATION_ROW: Argument<field::Text> = objcmd::row_of("stations");

/// The arguments of `remove`.
#[derive(Arguments)]
pub struct StationArgs<'a> {
    /// The row key of the station.
    #[arg(shared = STATION_ROW)]
    pub row: &'a str,
}

/// Forgets one station.
pub const REMOVE: CommandKind<StationArgs<'static>> = objcmd::remove_kind();
/// Starts calling CQ in the next transmit slot. It sends text and lasts until `halt` or the end of the QSO.
pub const CQ: Command = Command::new("cq", "Call CQ")
    .help("Starts calling CQ in the next transmit slot.")
    .transmits(TxContent::Text)
    .stopped_by("halt");
/// Starts the WSPR beacon of the station. It lasts until `halt`.
pub const BEACON: Command = Command::new("beacon", "Start the WSPR beacon")
    .help("Sends the call, locator and power of the station in the beacon slots.")
    .transmits(TxContent::Beacon)
    .stopped_by("halt");
/// Stops transmitting at once.
pub const HALT: Command = Command::new("halt", "Halt").help("Stops transmitting at once.");

/// The arguments of `send`.
#[derive(Arguments)]
pub struct SendArgs<'a> {
    /// The message to send: `HB`, `CQ`, `TO CMD [NUM]` (a command such as `SNR?`), `TO TEXT` or `VIA>DEST TEXT`.
    #[arg(
        label = "Message",
        help = "HB, CQ, TO CMD [NUM], TO TEXT or VIA>DEST TEXT."
    )]
    pub text: &'a str,
}

/// Queues a JS8 message. It sends text, one frame a slot, until the queue is empty or `halt`.
pub const SEND: CommandKind<SendArgs<'static>> = CommandKind::new("send", "Send")
    .help("Queues a JS8 message. My call and locator come from the station.")
    .transmits(TxContent::Text)
    .stopped_by("halt");

// The components that are not the status component.

/// The counts of the decoder of one mode (`dec_<mode>`).
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct SlotDecoderFields {
    /// The slots decoded.
    #[field(label = "Slots decoded")]
    pub slots: i64,
    /// The decodes.
    #[field(label = "Decodes")]
    pub decodes: i64,
    /// The time that the last slot took to decode.
    #[field(label = "Decoding time", unit = "ms")]
    pub decode_ms: f64,
}

/// The result fields of whole-span decoding and the multi-band monitor (`wide`).
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct WideFields {
    /// The dials decoded.
    #[field(label = "Dials decoded")]
    pub channels: i64,
    /// The share of real time that decoding takes.
    #[field(label = "Decoding load", unit = "%")]
    pub load_pct: f64,
    /// Decoding does not keep up.
    #[field(label = "Too slow")]
    pub too_slow: bool,
}

/// A row of the multi-band monitor: one band and mode of the selection. The row key is the band index in two digits
/// and the mode index.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct MultibandRow<'a> {
    /// The band.
    #[field(label = "Band")]
    pub band: &'a str,
    /// The mode.
    #[field(label = "Mode")]
    pub mode: Mode,
    /// The dial of the mode on the band, if it has one.
    #[field(label = "Dial", unit = "Hz")]
    pub dial_hz: Option<Hertz>,
    /// A decoder runs on the dial.
    #[field(label = "Running")]
    pub running: bool,
    /// The decodes of the dial.
    #[field(label = "Decodes")]
    pub decodes: i64,
    /// The capture covers the dial.
    #[field(label = "Covered")]
    pub covered: bool,
    /// The span that the capture needs around its centre to cover the dial. It has no value for a covered dial.
    #[field(label = "Span needed", unit = "Hz")]
    pub span_needed_hz: Option<Hertz>,
}

/// The bands and modes of the multi-band monitor (`wide.multiband`).
pub static MONITOR: Table<MultibandRow<'static>> =
    Table::new("multiband", "Multi-band monitor", 64);

/// The result fields of band hopping (`hop`).
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct HopFields<'a> {
    /// The band of the present hop slot. It has no value when no band is planned or hopping is off.
    #[field(label = "Hop band")]
    pub band: Option<&'a str>,
    /// The UTC of the next band change. It has no value while hopping is off.
    #[field(label = "Next band change")]
    pub next_utc: Option<Time>,
}

/// The name of the decoder component of `mode` in the graph.
pub fn decoder_name(mode: Mode) -> &'static str {
    match mode {
        Mode::Ft8 => "dec_ft8",
        Mode::Ft4 => "dec_ft4",
        Mode::Ft2 => "dec_ft2",
        Mode::Wspr => "dec_wspr",
        Mode::Jt65 => "dec_jt65",
        Mode::Jt9 => "dec_jt9",
        Mode::Jt4 => "dec_jt4",
        Mode::Q65 => "dec_q65",
        Mode::Fst4 => "dec_fst4",
        Mode::Fst4w => "dec_fst4w",
        Mode::Msk144 => "dec_msk144",
        Mode::Js8 => "dec_js8",
    }
}

/// What the display of the slot stack of `mode` (`None`: the combined stack) publishes: its fields, tables, series,
/// map points and events. It is the part of [`slot_stack`] that the display writes.
pub fn display_declaration(mode: Option<Mode>) -> sdrkit_core::ResultsDecl {
    let g = Gives::of(mode);
    let mut d = sdrkit_core::ResultsDecl::new().with_fields(WsjtFields::SHAPE.fields);
    if g.decodes {
        d = d
            .with_fields(WsjtDecodeFields::SHAPE.fields)
            .with_table(&DECODES);
    }
    if g.js8 {
        d = d.with_fields(Js8Fields::SHAPE.fields);
    }
    if g.wspr {
        d = d.with_table(&WSPR_SPOTS);
    }
    d = d.with_table(&MODES).with_table(&STATIONS);
    if g.js8 {
        d = d
            .with_table(&JS8_FRAMES)
            .with_table(&JS8_MESSAGES)
            .with_table(&JS8_STATIONS)
            .series(
                WATERFALL.key,
                WATERFALL.label,
                WATERFALL.x_unit,
                WATERFALL.y_unit,
            );
    }
    d = d.points("Stations");
    if g.decodes {
        d = d.event(&DECODE);
    }
    d = d.event(&SPOT);
    if g.js8 {
        d = d.event(&JS8_FRAME).event(&JS8_MESSAGE);
    }
    if g.transmits {
        d = d.event(&TX);
    }
    d
}

/// The description of the slot stack of `mode` (`None`: the combined stack `wsjt`). A stack has only the tables, the
/// events and the fields that its modes fill ([`Gives`]). The decodes, the spots, the stations, the JS8 logs and
/// their totals stay over a change of frequency.
fn slot_stack(mode: Option<Mode>) -> StackShape {
    use crate::settings::{hop_too, settings_of, wide_too};
    let modes: Vec<Mode> = mode.map_or_else(|| Mode::ALL.to_vec(), |m| vec![m]);
    let g = Gives::of(mode);
    let mut s = StackShape::new(crate::receiver::stack_id(mode)).fields::<WsjtFields>();
    if g.decodes {
        s = s.fields::<WsjtDecodeFields>().table(&DECODES);
    }
    if g.js8 {
        s = s.fields::<Js8Fields>();
    }
    if g.wspr {
        s = s.table(&WSPR_SPOTS);
    }
    s = s.table(&MODES).table(&STATIONS);
    if g.js8 {
        s = s
            .table(&JS8_FRAMES)
            .table(&JS8_MESSAGES)
            .table(&JS8_STATIONS)
            .series(WATERFALL);
    }
    s = s.points("Stations");
    if g.decodes {
        s = s.event(&DECODE);
    }
    s = s.event(&SPOT);
    if g.js8 {
        s = s.event(&JS8_FRAME).event(&JS8_MESSAGE);
    }
    if g.transmits {
        s = s.event(&TX);
    }
    s = s
        .settings(settings_of(mode))
        .command(clear_of(mode))
        .command(&REMOVE);
    if crate::settings::qso_too(mode) {
        s = s.command(&CQ);
    }
    if modes.contains(&Mode::Wspr) {
        s = s.command(&BEACON);
    }
    if modes.contains(&Mode::Js8) {
        s = s.command(&SEND);
    }
    if crate::settings::transmits_too(mode) {
        s = s.command(&HALT);
    }
    for m in modes {
        s = s.component(decoder_name(m), |c| {
            c.fields::<SlotDecoderFields>().dropped()
        });
    }
    if wide_too(mode) {
        s = s.component("wide", |c| c.fields::<WideFields>().table(&MONITOR));
    }
    if hop_too(mode) {
        s = s.component("hop", |c| c.fields::<HopFields>());
    }
    let mut kept = vec!["stations"];
    if g.decodes {
        kept.extend(["decodes", "WsjtDecodeFields.decodes_total"]);
    }
    if g.wspr {
        kept.push("wspr_spots");
    }
    if g.js8 {
        kept.extend([
            "js8_frames",
            "js8_messages",
            "js8_stations",
            "Js8Fields.js8_frames",
            "Js8Fields.js8_failed",
        ]);
    }
    s.kept(&kept)
}

/// The `clear` command of the slot stack of `mode`. The stack takes the command of the first component in run order
/// that has one: the wide component when the stack has it, else a decoder.
fn clear_of(mode: Option<Mode>) -> &'static Command {
    if crate::settings::wide_too(mode) {
        &objcmd::CLEAR
    } else {
        &sdrkit_graph::decoder::CLEAR
    }
}

// The stacks of single transmissions: FSK441, JT6M, ISCAT and PI4.

/// The result fields that each of `fsk441`, `jt6m`, `iscat` and `pi4` shows.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct ScatterFields<'a> {
    /// The decodes since the last clear.
    #[field(label = "Decodes")]
    pub decodes_total: i64,
    /// What no public document confirms.
    #[field(label = "Note")]
    pub note: &'a str,
    /// The input level. These stacks do not measure it: it has no value.
    #[field(shared = keys::LEVEL_DBFS)]
    pub level_dbfs: Option<f64>,
}

/// The row of the decode table and the fields of the decode event of one of these stacks: the columns and fields
/// that all four have, then those of the stack. Each stack has its own event struct for the event type
/// `wsjt.decode`.
macro_rules! scatter_shapes {
    (
        $(#[$row_doc:meta])* $row:ident,
        $(#[$event_doc:meta])* $event:ident,
        { $($(#[$m:meta])* $f:ident: $t:ty,)* }
    ) => {
        $(#[$row_doc])*
        #[derive(Clone, Debug, PartialEq, Fields)]
        pub struct $row<'a> {
            /// The time of the start of the decode.
            #[field(label = "UTC")]
            pub utc: Time,
            /// The signal time of the start of the decode, from the first sample.
            #[field(label = "Signal time", unit = "s")]
            pub time_s: f64,
            /// The SNR.
            #[field(shared = keys::SNR_DB)]
            pub snr_db: f64,
            /// The audio frequency.
            #[field(label = "Audio", unit = "Hz")]
            pub audio_hz: Hertz,
            /// The mode or its variant.
            #[field(label = "Mode")]
            pub mode: &'a str,
            /// The decoded text.
            #[field(label = "Message")]
            pub message: &'a str,
            $($(#[$m])* pub $f: $t,)*
        }

        $(#[$event_doc])*
        #[derive(Clone, Debug, PartialEq, Fields)]
        pub struct $event<'a> {
            /// The mode or its variant.
            #[field(label = "Mode")]
            pub mode: &'a str,
            /// The decoded text.
            #[field(shared = keys::TEXT)]
            pub text: &'a str,
            /// The SNR.
            #[field(shared = keys::SNR_DB)]
            pub snr_db: f64,
            /// The audio frequency.
            #[field(label = "Audio", unit = "Hz")]
            pub audio_hz: Hertz,
            /// The first callsign of the text.
            #[field(shared = keys::CALLSIGN)]
            pub callsign: Option<&'a str>,
            /// The first callsign of the text.
            #[field(shared = keys::SOURCE)]
            pub source: Option<&'a str>,
            /// The family of the source: a message.
            #[field(shared = keys::FAMILY)]
            pub family: Family,
            $($(#[$m])* pub $f: $t,)*
        }
    };
}

scatter_shapes! {
    /// A row of the decode table of `fsk441`. The row key is the number of the decode.
    Fsk441DecodeRow,
    /// The fields of a decode of `fsk441`.
    Fsk441DecodeEvent,
    {
        /// The contrast of the decode. It is no calibrated SNR.
        #[field(label = "Contrast", unit = "dB")]
        contrast_db: f64,
        /// The length of the burst.
        #[field(label = "Duration", unit = "ms")]
        duration_ms: f64,
    }
}

scatter_shapes! {
    /// A row of the ping table of `jt6m`. The row key is the number of the ping.
    Jt6mDecodeRow,
    /// The fields of a ping of `jt6m`.
    Jt6mDecodeEvent,
    {
        /// The length of the ping.
        #[field(label = "Duration", unit = "s")]
        duration_s: f64,
        /// The clear frames of the ping.
        #[field(label = "Clear frames")]
        clear_frames: i64,
    }
}

scatter_shapes! {
    /// A row of the decode table of `iscat`. The row key is the number of the decode.
    IscatDecodeRow,
    /// The fields of a decode of `iscat`.
    IscatDecodeEvent,
    {
        /// The variant of the decode.
        #[field(label = "Variant")]
        variant: &'a str,
        /// The sync ratio r.
        #[field(label = "Sync ratio")]
        sync_ratio: f64,
        /// The whole frames used.
        #[field(label = "Frames")]
        frames: i64,
        /// The characters shown as `?`.
        #[field(label = "Unclear characters")]
        unclear: i64,
    }
}

scatter_shapes! {
    /// A row of the decode table of `pi4`. The row key is the number of the decode.
    Pi4DecodeRow,
    /// The fields of a decode of `pi4`.
    Pi4DecodeEvent,
    {
        /// The start of the decode against the minute.
        #[field(label = "DT", unit = "s")]
        dt_s: f64,
        /// The carrier frequency: tone 0 plus half a spacing.
        #[field(label = "Carrier", unit = "Hz")]
        carrier_hz: f64,
        /// The sync value c.
        #[field(label = "Sync")]
        sync: f64,
        /// The tone spacing factor of the decode.
        #[field(label = "K")]
        k: i64,
    }
}

/// The result fields that `fsk441` adds.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct Fsk441Fields {
    /// The level of the burst finder: the last frame over the noise.
    #[field(label = "Burst level", unit = "dB")]
    pub burst_level_db: f64,
    /// The bursts found.
    #[field(label = "Bursts")]
    pub bursts: i64,
}

/// The result fields that `jt6m` adds. No field has a value before the first ping.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct Jt6mFields<'a> {
    /// The folded message of the newest window that gave one.
    #[field(label = "Folded message")]
    pub folded: Option<&'a str>,
    /// The period of the folded message, in characters.
    #[field(label = "Fold period")]
    pub fold_period: Option<i64>,
    /// The offset of the sync tone of the newest ping.
    #[field(label = "Tuning", unit = "Hz")]
    pub tuning_hz: Option<f64>,
}

/// The result fields that `iscat` adds.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct IscatFields<'a> {
    /// The variant of the newest decode. No value before the first decode.
    #[field(label = "Variant")]
    pub variant: Option<&'a str>,
}

/// The decodes of `fsk441`.
pub static FSK441_DECODES: Table<Fsk441DecodeRow<'static>> =
    Table::new("decodes", "Decodes", crate::scatter::ROWS_CAP)
        .sorted(&Fsk441DecodeRow::UTC, SortOrder::Descending);
/// The pings of `jt6m`.
pub static JT6M_DECODES: Table<Jt6mDecodeRow<'static>> =
    Table::new("decodes", "Pings", crate::scatter::ROWS_CAP)
        .sorted(&Jt6mDecodeRow::UTC, SortOrder::Descending);
/// The decodes of `iscat`.
pub static ISCAT_DECODES: Table<IscatDecodeRow<'static>> =
    Table::new("decodes", "Decodes", crate::scatter::ROWS_CAP)
        .sorted(&IscatDecodeRow::UTC, SortOrder::Descending);
/// The decodes of `pi4`.
pub static PI4_DECODES: Table<Pi4DecodeRow<'static>> =
    Table::new("decodes", "Decodes", crate::scatter::ROWS_CAP)
        .sorted(&Pi4DecodeRow::UTC, SortOrder::Descending);

/// A decode of `fsk441`.
pub static FSK441_DECODE: EventKind<Fsk441DecodeEvent<'static>> =
    EventKind::new("wsjt.decode", "Decode");
/// A ping of `jt6m`.
pub static JT6M_DECODE: EventKind<Jt6mDecodeEvent<'static>> = EventKind::new("wsjt.decode", "Ping");
/// A decode of `iscat`.
pub static ISCAT_DECODE: EventKind<IscatDecodeEvent<'static>> =
    EventKind::new("wsjt.decode", "Decode");
/// A decode of `pi4`.
pub static PI4_DECODE: EventKind<Pi4DecodeEvent<'static>> = EventKind::new("wsjt.decode", "Decode");

/// `shape`, which lists the result sets of one of these stacks, with the count of its decoder, the spot event, the
/// clear command and what is kept. The decodes and their total stay over a change of frequency.
fn scatter_stack(shape: StackShape) -> StackShape {
    shape
        .fields::<sdrkit_graph::DecoderFields>()
        .event(&SPOT)
        .command(&sdrkit_graph::decoder::CLEAR)
        .kept(&["decodes", "ScatterFields.decodes_total"])
}

/// The descriptions of `fsk441`, `jt6m`, `iscat` and `pi4`.
fn scatter_stacks() -> [StackShape; 4] {
    use crate::scatter::{fsk441, iscat, jt6m, pi4};
    let of = |id: &'static str| StackShape::new(id).fields::<ScatterFields<'static>>();
    [
        scatter_stack(
            of("fsk441")
                .fields::<Fsk441Fields>()
                .table(&FSK441_DECODES)
                .event(&FSK441_DECODE)
                .settings(&fsk441::settings()),
        ),
        scatter_stack(
            of("jt6m")
                .fields::<Jt6mFields<'static>>()
                .table(&JT6M_DECODES)
                .event(&JT6M_DECODE)
                .settings(&jt6m::settings()),
        ),
        scatter_stack(
            of("iscat")
                .fields::<IscatFields<'static>>()
                .table(&ISCAT_DECODES)
                .event(&ISCAT_DECODE)
                .settings(&iscat::settings()),
        ),
        scatter_stack(
            of("pi4")
                .table(&PI4_DECODES)
                .event(&PI4_DECODE)
                .settings(&pi4::settings()),
        ),
    ]
}

/// The interface of the family.
pub fn family() -> &'static Described {
    static FAMILY: OnceLock<Described> = OnceLock::new();
    // 2.0: a stack has only the transmit commands and settings of its modes, the WSPR beacon starts with the command
    // `beacon` and no setting, and `cq`, `beacon` and `send` transmit. 3.0: the Q65 stack has its code built in and no
    // setting `code_dir`. 4.0: no stack has the setting `code_dir`, and a mode row has no state `no_table`. 5.0: a slot
    // stack has only the tables, the events and the fields that its modes fill; the count of the decodes and the JS8
    // fields are result sets of their own.
    FAMILY.get_or_init(|| {
        let mut f = Described::new("wsjt", 5, 0)
            .content(
                sdrkit_core::ContentKind::WeakSignal,
                &[
                    "wsjt", "ft8", "ft4", "ft2", "wspr", "jt65", "jt9", "jt4", "q65", "fst4",
                    "fst4w", "msk144", "js8", "fsk441", "jt6m", "iscat", "pi4",
                ],
            )
            .audio(
                sdrkit_core::AudioKind::Monitor,
                &[
                    "wsjt", "ft8", "ft4", "ft2", "wspr", "jt65", "jt9", "jt4", "q65", "fst4",
                    "fst4w", "msk144", "js8", "fsk441", "jt6m", "iscat", "pi4",
                ],
            )
            .text(&[
                "wsjt", "ft8", "ft4", "ft2", "wspr", "jt65", "jt9", "jt4", "q65", "fst4", "fst4w",
                "msk144", "js8", "fsk441", "jt6m", "iscat", "pi4",
            ])
            .shared_source("table", SLOT_STACKS)
            .shared_source("components", SLOT_STACKS)
            .shared_source("runner", SLOT_STACKS)
            .shared_source("receiver", SLOT_STACKS)
            .shared_source("verify", SLOT_STACKS)
            .stack(slot_stack(None));
        for m in Mode::ALL {
            f = f.stack(slot_stack(Some(m)));
        }
        scatter_stacks().into_iter().fold(f, Described::stack)
    })
}

/// The ids of the slot stacks.
const SLOT_STACKS: &[&str] = &[
    "wsjt", "ft8", "ft4", "ft2", "wspr", "jt65", "jt9", "jt4", "q65", "fst4", "fst4w", "msk144",
    "js8",
];

/// The description of the slot stack of `mode` (`None`: the combined stack `wsjt`).
pub fn shape_of(mode: Option<Mode>) -> &'static StackShape {
    shape(crate::receiver::stack_id(mode))
}

/// The description of the stack `id`.
///
/// # Panics
///
/// If the family does not describe the stack.
pub fn shape(id: &str) -> &'static StackShape {
    family()
        .stack_shape(id)
        .unwrap_or_else(|| panic!("the wsjt family does not describe the stack {id}"))
}

/// An empty store of the results of the stack `id`, for its decoder.
pub fn results(id: &str) -> sdrkit_core::publish::Published {
    sdrkit_core::publish::Published::from_decl(std::sync::Arc::new(shape(id).results_decl()))
}
