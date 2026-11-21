//! The interface of the HF keyboard family: the shapes of the result sets, the table rows and the events, the tables,
//! the event types, the commands and what each stack declares.

use crate::decoder::{front_settings, RxOptions};
use crate::fsq::Trigger;
use crate::mt63::InterleaveChoice;
use crate::psk::Variant;
use crate::sender::{TX_LEVEL, TX_TUNE_LIMIT};
use sdrkit_core::command::Command;
use sdrkit_core::interface::{
    Arguments, CommandKind, EventKind, Family as Described, Fields, Hertz, Seconds, StackShape,
    Table, Time,
};
use sdrkit_core::publish::keys::{self, Family};
use sdrkit_core::publish::{field, Published, SeriesDef};
use sdrkit_core::settings::{Argument, DeclareSetting};
use sdrkit_core::{Schema, SettingSet};
use sdrkit_graph::DecoderFields;
use sdrkit_util_objects::commands as objcmd;
use sdrkit_util_objects::publish::columns;
use std::sync::{Arc, OnceLock};

// What every stack shares.

/// The levels that the front end measures. A level has no value before the first input.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct FrontFields {
    /// The power of the input.
    #[field(shared = keys::LEVEL_DBFS)]
    pub level_dbfs: Option<f64>,
    /// The power of the 200 to 3800 Hz sideband before the AGC.
    #[field(label = "Audio level", unit = "dBFS")]
    pub audio_dbfs: Option<f64>,
}

/// A row of the stations heard. The row key is the callsign.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct StationRow<'a> {
    /// The callsign.
    #[field(shared = keys::CALLSIGN)]
    pub callsign: &'a str,
    /// Where it was last heard.
    #[field(shared = keys::FREQ_HZ)]
    pub freq_hz: Hertz,
    /// Its SNR when last heard.
    #[field(shared = keys::SNR_DB)]
    pub snr_db: f64,
    /// The times heard.
    #[field(shared = columns::HEARD)]
    pub heard: Option<i64>,
    /// The UTC of the last time heard.
    #[field(shared = columns::LAST_HEARD)]
    pub last_heard: Option<Time>,
}

/// The stations heard: one row for each callsign in the decoded text.
pub static STATIONS: Table<StationRow<'static>> = Table::new("stations", "Stations heard", 100);

/// The fields of a callsign heard for the first time.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct SpotEvent<'a> {
    /// The callsign.
    #[field(shared = keys::CALLSIGN)]
    pub callsign: &'a str,
    /// The callsign.
    #[field(shared = keys::SOURCE)]
    pub source: &'a str,
    /// The SNR.
    #[field(shared = keys::SNR_DB)]
    pub snr_db: f64,
    /// Where it was heard.
    #[field(shared = keys::FREQ_HZ)]
    pub freq_hz: Hertz,
    /// The stack that heard it ("RTTY").
    #[field(shared = keys::SPOT_MODE)]
    pub mode: &'a str,
    /// The family of the source: a message.
    #[field(shared = keys::FAMILY)]
    pub family: Family,
}

/// The fields of a finished line of decoded text.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct LineEvent<'a> {
    /// The callsign after "DE", if the line has one.
    #[field(shared = keys::SOURCE)]
    pub source: Option<&'a str>,
    /// The line.
    #[field(shared = keys::TEXT)]
    pub text: &'a str,
    /// The SNR.
    #[field(shared = keys::SNR_DB)]
    pub snr_db: f64,
    /// The stack that heard it.
    #[field(shared = keys::SPOT_MODE)]
    pub mode: &'a str,
    /// The family of the source: a message.
    #[field(shared = keys::FAMILY)]
    pub family: Family,
}

/// A callsign heard for the first time.
pub static SPOT: EventKind<SpotEvent<'static>> = EventKind::new("hfkbd.spot", "Callsign heard");
/// A finished line of decoded text.
pub static LINE: EventKind<LineEvent<'static>> = EventKind::new("hfkbd.text", "Text line");

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

/// The result fields of the keyboard stacks of many tones: MFSK, Olivia, Contestia, DominoEX and THOR.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct KeyboardFields<'a> {
    /// The variant in use.
    #[field(label = "Variant")]
    pub variant: &'a str,
    /// The audio centre now.
    #[field(label = "Audio centre", unit = "Hz")]
    pub centre_hz: Hertz,
    /// The tuning error found.
    #[field(shared = keys::OFFSET_HZ)]
    pub offset_hz: Hertz,
    /// The decoder's own SNR.
    #[field(shared = keys::SNR_DB)]
    pub snr_db: f64,
    /// The decoded text.
    #[field(shared = keys::TEXT)]
    pub text: &'a str,
    /// Characters decoded.
    #[field(label = "Characters")]
    pub characters: i64,
    /// The newest callsign of the text, if it has one.
    #[field(shared = keys::CALLSIGN)]
    pub callsign: Option<&'a str>,
    /// How clean the decisions are, 0 to 100.
    #[field(label = "Quality", unit = "%")]
    pub quality_pct: f64,
}

/// The picture being received, or else the newest one. Each field of a picture has no value before the first.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct PictureFields<'a> {
    /// The number of the picture.
    #[field(label = "Picture")]
    pub picture_id: Option<i64>,
    /// Its width.
    #[field(label = "Width", unit = "px")]
    pub picture_width: Option<i64>,
    /// Its height.
    #[field(label = "Height", unit = "px")]
    pub picture_height: Option<i64>,
    /// The rows received.
    #[field(label = "Rows")]
    pub picture_rows: Option<i64>,
    /// How much of it is in.
    #[field(label = "Progress", unit = "%")]
    pub picture_progress_pct: Option<f64>,
    /// It is complete.
    #[field(label = "Complete")]
    pub picture_complete: Option<bool>,
    /// It is in colour.
    #[field(label = "Colour")]
    pub picture_colour: Option<bool>,
    /// Its mode.
    #[field(label = "Picture mode")]
    pub picture_mode: Option<&'a str>,
    /// The pictures finished.
    #[field(label = "Pictures")]
    pub pictures: i64,
}

/// The fields of a picture received or cut off.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct PictureEvent<'a> {
    /// The number of the picture.
    #[field(label = "Picture")]
    pub picture_id: i64,
    /// Its width.
    #[field(label = "Width", unit = "px")]
    pub picture_width: i64,
    /// Its height.
    #[field(label = "Height", unit = "px")]
    pub picture_height: i64,
    /// The rows received.
    #[field(label = "Rows")]
    pub picture_rows: i64,
    /// It is complete.
    #[field(label = "Complete")]
    pub picture_complete: bool,
    /// Its mode.
    #[field(label = "Picture mode")]
    pub picture_mode: &'a str,
}

/// An MFSK picture received.
pub static MFSK_PICTURE: EventKind<PictureEvent<'static>> =
    EventKind::new("mfsk.picture", "Picture received");
/// An SSTV picture received or cut off.
pub static SSTV_PICTURE: EventKind<PictureEvent<'static>> =
    EventKind::new("sstv.picture", "Picture received");

// RTTY.

/// The result fields of RTTY.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct RttyFields<'a> {
    /// The decoded text.
    #[field(shared = keys::TEXT)]
    pub text: &'a str,
    /// The newest callsign of the text, if it has one.
    #[field(shared = keys::CALLSIGN)]
    pub callsign: Option<&'a str>,
    /// The mark tone. It has no value while searching.
    #[field(label = "Mark", unit = "Hz")]
    pub mark_hz: Option<Hertz>,
    /// The space tone. It has no value while searching.
    #[field(label = "Space", unit = "Hz")]
    pub space_hz: Option<Hertz>,
    /// The speed in use.
    #[field(shared = keys::SYMBOL_RATE_HZ)]
    pub symbol_rate_hz: Hertz,
    /// The shift in use.
    #[field(label = "Shift", unit = "Hz")]
    pub shift_hz: Hertz,
    /// The mark is the upper tone.
    #[field(label = "Reverse")]
    pub reverse: bool,
    /// Characters decoded.
    #[field(label = "Characters")]
    pub characters: i64,
    /// Characters that the framer dropped.
    #[field(label = "Framing errors")]
    pub framing_errors: i64,
    /// How clean the decisions are, 0 to 100.
    #[field(label = "Quality", unit = "%")]
    pub quality_pct: f64,
    /// Auto is on.
    #[field(label = "Automatic")]
    pub auto: bool,
    /// Auto has applied a guess that still decodes.
    #[field(label = "Automatic lock")]
    pub auto_locked: bool,
    /// The state of Auto: 0 not asked, 1 waiting for audio or running, 2 applied, 3 nothing found.
    #[field(label = "Automatic state")]
    pub auto_state: i64,
}

/// A guess of the last analysis of Auto. The row key is the rank, from 1.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct RttyGuessRow {
    /// The lower tone.
    #[field(label = "Lower tone", unit = "Hz")]
    pub lower_hz: Hertz,
    /// The shift.
    #[field(label = "Shift", unit = "Hz")]
    pub shift_hz: Hertz,
    /// The speed.
    #[field(shared = keys::SYMBOL_RATE_HZ)]
    pub symbol_rate_hz: Hertz,
    /// The mark is the upper tone.
    #[field(label = "Reverse")]
    pub reverse: bool,
    /// The text score less 2.5 for each framing error.
    #[field(label = "Score")]
    pub score: f64,
    /// The characters of the trial.
    #[field(label = "Characters")]
    pub characters: i64,
    /// The framing errors of the trial.
    #[field(label = "Framing errors")]
    pub framing_errors: i64,
}

/// The guesses of the last analysis, best first.
pub static AUTO_GUESSES: Table<RttyGuessRow> = Table::new("auto_guesses", "Automatic guesses", 3);

/// Runs one analysis on up to 20 s of the kept audio once 6 s are kept, and applies its best guess.
pub const AUTO_LOCK: Command = Command::new("auto_lock", "Find the settings").help(
    "Runs one analysis on up to 20 s of the kept audio, once 6 s are kept, and applies its best guess even with Auto off.",
);

// PSK.

/// The result fields of PSK.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct PskFields<'a> {
    /// The modulation decoded.
    #[field(label = "Modulation")]
    pub modulation: Variant,
    /// The noise floor of the window.
    #[field(label = "Noise floor", unit = "dB")]
    pub floor_db: f64,
    /// The id of the signal in focus, if one is.
    #[field(label = "Focus")]
    pub focus: Option<i64>,
    /// Characters decoded.
    #[field(label = "Characters")]
    pub characters: i64,
    /// Characters that the code table does not have.
    #[field(label = "Bad characters")]
    pub errors: i64,
    /// Auto is on.
    #[field(label = "Automatic")]
    pub auto: bool,
    /// Auto has applied a guess that still decodes.
    #[field(label = "Automatic lock")]
    pub auto_locked: bool,
    /// The modulation that Auto found. It has no value before Auto finds one.
    #[field(label = "Automatic modulation")]
    pub auto_modulation: Option<&'a str>,
}

/// A PSK signal heard, or one gone whose text stays. The row key is the signal id. The rows of the gone signals stay
/// over a change of frequency: they are the log of the decoded texts.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct PskSignalRow<'a> {
    /// The signal id.
    #[field(label = "Id")]
    pub id: i64,
    /// The audio frequency.
    #[field(shared = keys::OFFSET_HZ)]
    pub offset_hz: Hertz,
    /// The correction since it was found.
    #[field(label = "AFC", unit = "Hz")]
    pub afc_hz: Hertz,
    /// The SNR.
    #[field(shared = keys::SNR_DB)]
    pub snr_db: f64,
    /// How clean the decisions are, 0 to 100.
    #[field(label = "Quality", unit = "%")]
    pub quality_pct: f64,
    /// Text is coming.
    #[field(label = "Text coming")]
    pub dcd: bool,
    /// The user picked it.
    #[field(label = "Picked")]
    pub manual: bool,
    /// The signal time when it was first heard.
    #[field(label = "First heard", unit = "s")]
    pub first_s: f64,
    /// The signal time when it was last heard.
    #[field(label = "Last heard", unit = "s")]
    pub last_s: f64,
    /// Characters decoded.
    #[field(label = "Characters")]
    pub characters: i64,
    /// The decoded text.
    #[field(shared = keys::TEXT)]
    pub text: &'a str,
    /// It is no longer heard.
    #[field(label = "Gone")]
    pub gone: bool,
}

/// The PSK signals.
pub static PSK_SIGNALS: Table<PskSignalRow<'static>> =
    Table::new("signals", "Signals", crate::psk::SIGNALS_CAP);

/// The window spectrum of PSK.
pub const PSK_SPECTRUM: SeriesDef = SeriesDef {
    key: "spectrum",
    label: "Window spectrum",
    x_unit: "Hz",
    y_unit: "dB",
};

// CW.

sdrkit_core::choice_enum! {
    /// What the audio of the CW skimmer plays.
    #[derive(Default)]
    pub enum Playing {
        /// Silence: `listen` is off, or the rebuilt tone has no signal to key it.
        #[default]
        Nothing = ("nothing", "Nothing"),
        /// The signal listened to, a row of the signals table.
        Signal = ("signal", "Signal"),
        /// The dial through the filter: the real signal while no signal is to be heard.
        Dial = ("dial", "Dial, no signal"),
    }
}

/// The result fields of the CW skimmer. The fields of the sender have no value without a signal in focus.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct CwFields {
    /// The width of the band watched, centred on the dial.
    #[field(label = "Span", unit = "Hz")]
    pub span_hz: Hertz,
    /// The noise floor of the window.
    #[field(label = "Noise floor", unit = "dB")]
    pub floor_db: f64,
    /// The id of the signal in focus, if one is.
    #[field(label = "Focus")]
    pub focus: Option<i64>,
    /// Characters decoded.
    #[field(label = "Characters")]
    pub characters: i64,
    /// Characters that the code table does not have.
    #[field(label = "Bad characters")]
    pub errors: i64,
    /// The dot of the signal in focus.
    #[field(label = "Dot", unit = "ms")]
    pub dot_ms: Option<f64>,
    /// Its dash as dots.
    #[field(label = "Dash ratio")]
    pub dash_ratio: Option<f64>,
    /// Its character gap.
    #[field(label = "Character gap", unit = "dots")]
    pub char_gap: Option<f64>,
    /// Its word gap.
    #[field(label = "Word gap", unit = "dots")]
    pub word_gap: Option<f64>,
    /// How steady its keying is.
    #[field(label = "Steadiness", unit = "%")]
    pub steadiness_pct: Option<f64>,
    /// Its speed.
    #[field(label = "Speed", unit = "WPM")]
    pub wpm: Option<f64>,
    /// The signals in the table that are heard now.
    #[field(label = "Signals")]
    pub signals: i64,
    /// The signal time now, for the age of a signal's last-heard time.
    #[field(label = "Now", unit = "s")]
    pub now_s: f64,
    /// The id of the signal listened to (the row key in the signals table): the focus, else the strongest signal in the table.
    /// Absent while there is none, also while the audio plays the dial.
    #[field(label = "Listened to")]
    pub listened: Option<i64>,
    /// What the audio plays: nothing, the signal listened to, or the dial while there is none.
    #[field(label = "Playing")]
    pub playing: Playing,
    /// The radio frequency that the audio plays: that of the signal listened to, or the dial. Absent while it plays nothing.
    #[field(label = "Frequency played", unit = "Hz")]
    pub listened_hz: Option<Hertz>,
}

/// A CW signal heard, or one gone whose text stays. The row key is the signal id.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct CwSignalRow<'a> {
    /// The signal id.
    #[field(label = "Id")]
    pub id: i64,
    /// The audio frequency.
    #[field(shared = keys::OFFSET_HZ)]
    pub offset_hz: Hertz,
    /// The speed.
    #[field(label = "Speed", unit = "WPM")]
    pub wpm: f64,
    /// The SNR.
    #[field(shared = keys::SNR_DB)]
    pub snr_db: f64,
    /// The key is down.
    #[field(label = "Key down")]
    pub key_down: bool,
    /// The speed is known.
    #[field(label = "Speed known")]
    pub locked: bool,
    /// The sender spaces as Farnsworth.
    #[field(label = "Farnsworth")]
    pub farnsworth: bool,
    /// The user picked it.
    #[field(label = "Picked")]
    pub manual: bool,
    /// The signal time when it was first heard.
    #[field(label = "First heard", unit = "s")]
    pub first_s: f64,
    /// The signal time when it was last heard.
    #[field(label = "Last heard", unit = "s")]
    pub last_s: f64,
    /// Characters decoded.
    #[field(label = "Characters")]
    pub characters: i64,
    /// The decoded text.
    #[field(shared = keys::TEXT)]
    pub text: &'a str,
    /// The confidence of each character, 0 to 9.
    #[field(label = "Confidence")]
    pub confidence: &'a str,
    /// The other readings of unclear characters.
    #[field(label = "Other readings")]
    pub alternatives: &'a str,
    /// It is no longer heard.
    #[field(label = "Gone")]
    pub gone: bool,
    /// Characters that the code table does not have.
    #[field(label = "Bad characters")]
    pub errors: i64,
    /// The radio frequency of the carrier: the dial plus the offset.
    #[field(shared = keys::FREQ_HZ)]
    pub freq_hz: Hertz,
    /// The level of the carrier while the key is down, relative to full scale.
    #[field(shared = keys::LEVEL_DBFS)]
    pub level_dbfs: f64,
}

/// The CW signals.
pub static CW_SIGNALS: Table<CwSignalRow<'static>> =
    Table::new("signals", "Signals", crate::cw::SIGNALS_CAP);

/// The mark lengths of the signal in focus.
pub const CW_MARKS: SeriesDef = SeriesDef {
    key: "marks",
    label: "Mark lengths",
    x_unit: "ln(length/dot)",
    y_unit: "count",
};
/// The gap lengths of the signal in focus.
pub const CW_GAPS: SeriesDef = SeriesDef {
    key: "gaps",
    label: "Gap lengths",
    x_unit: "ln(length/dot)",
    y_unit: "count",
};
/// The averaged spectrum of the span, bins of about 12 Hz (at most 4096 points), each point the greatest bin it covers.
pub const CW_SPECTRUM: SeriesDef = SeriesDef {
    key: "spectrum",
    label: "Spectrum",
    x_unit: "Hz",
    y_unit: "dBFS",
};
/// The key of the signal listened to: its changes in the last 2 s, then its state now.
pub const CW_KEY: SeriesDef = SeriesDef {
    key: "key",
    label: "Key",
    x_unit: "s",
    y_unit: "down",
};

/// The result fields of the decoder of one keyed carrier (`hfkbd.morse`), which other graphs use.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct MorseFields<'a> {
    /// The decoded text.
    #[field(shared = keys::TEXT)]
    pub text: &'a str,
    /// The newest callsign of the text, if it has one.
    #[field(shared = keys::CALLSIGN)]
    pub callsign: Option<&'a str>,
    /// The speed.
    #[field(label = "Speed", unit = "WPM")]
    pub wpm: f64,
    /// The SNR.
    #[field(shared = keys::SNR_DB)]
    pub snr_db: f64,
    /// The speed is known.
    #[field(label = "Speed known")]
    pub locked: bool,
    /// The sender spaces as Farnsworth.
    #[field(label = "Farnsworth")]
    pub farnsworth: bool,
    /// Characters decoded.
    #[field(label = "Characters")]
    pub characters: i64,
    /// Characters that the code table does not have.
    #[field(label = "Bad characters")]
    pub errors: i64,
}

// Hellschreiber.

/// The result fields of Hellschreiber.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct HellFields<'a> {
    /// The variant in use.
    #[field(label = "Variant")]
    pub variant: &'a str,
    /// The audio centre now.
    #[field(label = "Audio centre", unit = "Hz")]
    pub centre_hz: Hertz,
    /// The pixel columns received.
    #[field(label = "Columns")]
    pub columns: i64,
    /// The SNR.
    #[field(shared = keys::SNR_DB)]
    pub snr_db: f64,
}

// MT63.

/// The result fields of MT63.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct Mt63Fields<'a> {
    /// The width of the signal in use.
    #[field(label = "Width", unit = "Hz")]
    pub bandwidth_hz: Hertz,
    /// The interleave in use.
    #[field(label = "Interleave")]
    pub interleave: InterleaveChoice,
    /// The centre of the carriers above the dial.
    #[field(label = "Centre", unit = "Hz")]
    pub centre_hz: Hertz,
    /// How clean the decisions are, 0 to 100.
    #[field(label = "Quality", unit = "%")]
    pub quality_pct: f64,
    /// The noise floor.
    #[field(label = "Noise floor", unit = "dB")]
    pub floor_db: f64,
    /// The times the receiver locked.
    #[field(label = "Locks")]
    pub locks: i64,
    /// The correction since the lock.
    #[field(label = "AFC", unit = "Hz")]
    pub afc_hz: Hertz,
    /// The SNR.
    #[field(shared = keys::SNR_DB)]
    pub snr_db: f64,
    /// Characters decoded.
    #[field(label = "Characters")]
    pub characters: i64,
    /// Characters that the code did not resolve.
    #[field(label = "Bad characters")]
    pub errors: i64,
    /// The decoded text.
    #[field(shared = keys::TEXT)]
    pub text: &'a str,
    /// The newest callsign of the text, if it has one.
    #[field(shared = keys::CALLSIGN)]
    pub callsign: Option<&'a str>,
}

/// The spectrum of the MT63 band.
pub const MT63_SPECTRUM: SeriesDef = SeriesDef {
    key: "spectrum",
    label: "Spectrum",
    x_unit: "Hz",
    y_unit: "dB",
};
/// The level of each MT63 carrier.
pub const MT63_CARRIERS: SeriesDef = SeriesDef {
    key: "carriers",
    label: "Carrier levels",
    x_unit: "carrier",
    y_unit: "dB",
};

// FSQ and IFKP.

/// The result fields of FSQ and IFKP. The tones have no value before a signal is heard.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct FsqFields<'a> {
    /// The speed in use.
    #[field(label = "Speed")]
    pub speed: &'a str,
    /// The width of the tones.
    #[field(label = "Band", unit = "Hz")]
    pub band_hz: Hertz,
    /// The symbol rate.
    #[field(shared = keys::SYMBOL_RATE_HZ)]
    pub symbol_rate_hz: Hertz,
    /// The lowest tone heard.
    #[field(label = "Lowest tone", unit = "Hz")]
    pub lowest_hz: Option<Hertz>,
    /// The highest tone heard.
    #[field(label = "Highest tone", unit = "Hz")]
    pub highest_hz: Option<Hertz>,
    /// How clean the decisions are, 0 to 100.
    #[field(label = "Quality", unit = "%")]
    pub quality_pct: f64,
    /// The SNR.
    #[field(shared = keys::SNR_DB)]
    pub snr_db: f64,
    /// Characters decoded.
    #[field(label = "Characters")]
    pub characters: i64,
    /// Symbols that the receiver refused.
    #[field(label = "Dropped symbols")]
    pub dropped_symbols: i64,
    /// The decoded text.
    #[field(shared = keys::TEXT)]
    pub text: &'a str,
    /// The newest callsign of the text, if it has one.
    #[field(shared = keys::CALLSIGN)]
    pub callsign: Option<&'a str>,
}

/// A row of the FSQ messages. The row key is the place of the message in the log.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct FsqMessageRow<'a> {
    /// The signal time.
    #[field(label = "Time", unit = "s")]
    pub time_s: f64,
    /// The sender, when the line starts with "CALL:".
    #[field(label = "From")]
    pub from: Option<&'a str>,
    /// The two hex digits equal the CRC-8 of the callsign.
    #[field(label = "Check")]
    pub crc_ok: bool,
    /// The addressee.
    #[field(label = "To")]
    pub to: Option<&'a str>,
    /// The trigger.
    #[field(label = "Trigger")]
    pub trigger: Option<Trigger>,
    /// The text after the trigger.
    #[field(shared = keys::TEXT)]
    pub text: &'a str,
    /// Addressed to my call, allcall or cqcqcq.
    #[field(label = "To me")]
    pub to_me: bool,
}

/// The FSQ messages, newest first.
pub static FSQ_MESSAGES: Table<FsqMessageRow<'static>> = Table::new("messages", "Messages", 60);

/// The fields of an FSQ message.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct FsqMessageEvent<'a> {
    /// The sender, when the line starts with "CALL:".
    #[field(shared = keys::SOURCE)]
    pub source: Option<&'a str>,
    /// The addressee.
    #[field(shared = keys::TARGET)]
    pub target: Option<&'a str>,
    /// The text after the trigger.
    #[field(shared = keys::TEXT)]
    pub text: &'a str,
    /// The family of the source: a message.
    #[field(shared = keys::FAMILY)]
    pub family: Family,
    /// The two hex digits equal the CRC-8 of the callsign.
    #[field(label = "Check")]
    pub crc_ok: bool,
    /// The trigger.
    #[field(label = "Trigger")]
    pub trigger: Option<Trigger>,
    /// Addressed to my call, allcall or cqcqcq.
    #[field(label = "To me")]
    pub to_me: bool,
}

/// An FSQ message.
pub static FSQ_MESSAGE: EventKind<FsqMessageEvent<'static>> =
    EventKind::new("fsq.message", "Message");

/// How often each tone position is used.
pub const TONE_USE: SeriesDef = SeriesDef {
    key: "tone_use",
    label: "Tone use",
    x_unit: "position",
    y_unit: "share",
};

// SSTV.

/// The result fields of SSTV.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct SstvFields<'a> {
    /// The mode of the picture being received or the newest one. It has no value before the first picture.
    #[field(label = "Mode")]
    pub mode: Option<&'a str>,
    /// Its VIS code, -1 when it was found from the line period.
    #[field(label = "VIS code")]
    pub vis: Option<i64>,
    /// Its slant: the line period over the nominal one, minus 1.
    #[field(label = "Slant", unit = "ppm")]
    pub slant_ppm: Option<f64>,
    /// The tuning error.
    #[field(shared = keys::OFFSET_HZ)]
    pub offset_hz: Hertz,
}

/// The result fields of the SSTV transmitter.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct SstvTxFields<'a> {
    /// Whether a picture is being sent.
    #[field(label = "Sending")]
    pub tx_sending: bool,
    /// The mode of the picture being sent. No value while nothing is sent.
    #[field(label = "Transmit mode")]
    pub tx_mode: Option<&'a str>,
    /// The line being sent, from 1; 0 during the VIS header. No value while nothing is sent.
    #[field(label = "Line sent")]
    pub tx_line: Option<i64>,
    /// The lines of the picture being sent. No value while nothing is sent.
    #[field(label = "Lines to send")]
    pub tx_lines: Option<i64>,
    /// The time on air of one picture in the transmit mode: the VIS header and the lines.
    #[field(label = "Picture time on air", unit = "s")]
    pub tx_picture_s: Seconds,
    /// The time until the picture is sent again, while repeat waits in the pause. No value otherwise.
    #[field(label = "Next picture in", unit = "s")]
    pub tx_next_s: Option<Seconds>,
}

/// The fields of a change of the SSTV transmission.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct SstvTxEvent<'a> {
    /// The state: started, finished or refused.
    #[field(label = "State")]
    pub state: &'a str,
    /// Why it changed: empty, stopped, the transmitter is off, or why a picture was refused.
    #[field(label = "Reason")]
    pub reason: &'a str,
    /// The mode of the picture.
    #[field(label = "Mode")]
    pub mode: &'a str,
    /// The family of the source: a message.
    #[field(shared = keys::FAMILY)]
    pub family: Family,
}

/// An SSTV transmission.
pub static SSTV_TX: EventKind<SstvTxEvent<'static>> =
    EventKind::new("sstv.tx", "SSTV transmission");

sdrkit_core::choice_enum! {
    /// The layout of the pixels given to `send`: rows top first, no padding.
    pub enum SstvPixels {
        /// Red, green and blue, one byte each.
        Rgb8 = ("rgb8", "RGB, 8 bits each"),
        /// Blue, green, red and alpha, one byte each. Alpha is not used.
        Bgra = ("bgra", "BGRA, 8 bits each"),
        /// One byte of grey.
        Grey8 = ("grey8", "8-bit grey"),
    }
}

/// The arguments of the SSTV `send`.
#[derive(Arguments)]
pub struct SstvSendArgs {
    /// The pixels.
    #[arg(
        label = "Pixels",
        help = "The pixel bytes, rows top first with no padding, in the pixel format."
    )]
    pub pixels: Arc<[u8]>,
    /// The width.
    #[arg(
        label = "Width",
        unit = "px",
        help = "The width of the picture, 1 to 8192 pixels."
    )]
    pub width: i64,
    /// The height.
    #[arg(
        label = "Height",
        unit = "px",
        help = "The height of the picture, 1 to 8192 pixels."
    )]
    pub height: i64,
    /// The pixel format.
    #[arg(label = "Pixel format", help = "The layout of each pixel.")]
    pub pixel_format: SstvPixels,
}

/// Sends a picture.
pub const SSTV_SEND: CommandKind<SstvSendArgs> = CommandKind::new("send", "Send a picture")
    .help(
        "Scales the picture to the transmit mode by the fit setting and sends it: the VIS header, then the lines; again \
         after each pause while repeat is on. Transmit must be on.",
    )
    .transmits(sdrkit_core::TxContent::File)
    .stopped_by("stop");
/// Stops sending.
pub const SSTV_STOP: Command =
    Command::new("stop", "Stop sending").help("Ends the picture at once.");

// SITOR.

/// The result fields of SITOR.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct SitorFields<'a> {
    /// The decoded text.
    #[field(shared = keys::TEXT)]
    pub text: &'a str,
    /// The newest callsign of the text, if it has one.
    #[field(shared = keys::CALLSIGN)]
    pub callsign: Option<&'a str>,
    /// The centre of the tones.
    #[field(shared = keys::OFFSET_HZ)]
    pub offset_hz: Hertz,
    /// The shift. It has no value before a signal is found.
    #[field(label = "Shift", unit = "Hz")]
    pub shift_hz: Option<Hertz>,
    /// The speed. It has no value before a signal is found.
    #[field(shared = keys::SYMBOL_RATE_HZ)]
    pub symbol_rate_hz: Option<Hertz>,
    /// The tones are the other way round.
    #[field(label = "Inverted")]
    pub inverted: bool,
    /// Words of a good code.
    #[field(label = "Good words")]
    pub good_words: i64,
    /// Words of a bad code.
    #[field(label = "Bad words")]
    pub bad_words: i64,
    /// Characters lost in both copies.
    #[field(label = "Mutilated")]
    pub mutilated: i64,
    /// Characters decoded.
    #[field(label = "Characters")]
    pub characters: i64,
    /// The ARQ monitor follows a link.
    #[field(label = "ARQ locked")]
    pub arq_locked: bool,
    /// ARQ blocks read.
    #[field(label = "ARQ blocks")]
    pub arq_blocks: i64,
    /// ARQ blocks that were new.
    #[field(label = "ARQ new blocks")]
    pub arq_new_blocks: i64,
    /// ARQ blocks sent again.
    #[field(label = "ARQ repeats")]
    pub arq_repeats: i64,
    /// ARQ slots that did not read.
    #[field(label = "ARQ bad slots")]
    pub arq_bad: i64,
    /// ARQ control signals read.
    #[field(label = "ARQ control signals")]
    pub arq_control_signals: i64,
    /// Where the control signals lie. It has no value before one is found.
    #[field(label = "ARQ control signal offset", unit = "bits")]
    pub arq_cs_offset_bits: Option<i64>,
    /// ARQ characters decoded.
    #[field(label = "ARQ characters")]
    pub arq_chars: i64,
    /// The ARQ text.
    #[field(label = "ARQ text")]
    pub arq_text: &'a str,
    /// The selective call of the link, if one was read.
    #[field(label = "Selective call")]
    pub selcall: Option<&'a str>,
    /// The changes of direction of the link.
    #[field(label = "ARQ changes of direction")]
    pub arq_changes: i64,
}

/// The fields of a SITOR-A link event.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct SitorArqEvent<'a> {
    /// What happened.
    #[field(shared = keys::TEXT)]
    pub text: &'a str,
}

/// A SITOR-A link event.
pub static SITOR_ARQ: EventKind<SitorArqEvent<'static>> =
    EventKind::new("sitor.arq", "SITOR-A link event");

// ARDOP.

/// The result fields of ARDOP.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct ArdopFields<'a> {
    /// Leaders heard.
    #[field(label = "Leaders")]
    pub leaders: i64,
    /// Headers read.
    #[field(label = "Headers")]
    pub good_headers: i64,
    /// Leaders whose header did not read.
    #[field(label = "Unreadable")]
    pub unreadable_leaders: i64,
    /// Data frames good.
    #[field(label = "Data good")]
    pub data_ok: i64,
    /// Data frames bad.
    #[field(label = "Data bad")]
    pub data_bad: i64,
    /// Reed-Solomon corrections.
    #[field(label = "RS corrections")]
    pub rs_corrections: i64,
    /// Control frames.
    #[field(label = "Control frames")]
    pub control: i64,
    /// Other frames.
    #[field(label = "Other frames")]
    pub other: i64,
    /// Stations heard.
    #[field(label = "Stations")]
    pub stations_heard: i64,
    /// Frames heard.
    #[field(shared = keys::FRAMES_OK)]
    pub frames_ok: i64,
    /// The newest text, if a frame held one.
    #[field(shared = keys::TEXT)]
    pub text: Option<&'a str>,
}

/// A row of the ARDOP frames. The row key is the number of the frame, counted from the start.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct ArdopFrameRow<'a> {
    /// The signal time.
    #[field(label = "Time", unit = "s")]
    pub time_s: f64,
    /// The frame type.
    #[field(label = "Type")]
    pub r#type: i64,
    /// The name of the type.
    #[field(label = "Name")]
    pub name: &'a str,
    /// What the frame is.
    #[field(label = "Kind")]
    pub kind: &'a str,
    /// Its modulation.
    #[field(label = "Modulation")]
    pub modulation: &'a str,
    /// Its speed.
    #[field(label = "Baud", unit = "Bd")]
    pub baud: i64,
    /// The session id.
    #[field(label = "Session")]
    pub session: i64,
    /// The sender, if the frame names one.
    #[field(label = "From")]
    pub from: Option<&'a str>,
    /// The addressee, if the frame names one.
    #[field(label = "To")]
    pub to: Option<&'a str>,
    /// The locator, if the frame holds one.
    #[field(shared = keys::LOCATOR)]
    pub locator: Option<&'a str>,
    /// The text, if the frame holds one.
    #[field(shared = keys::TEXT)]
    pub text: Option<&'a str>,
    /// The quality that the frame reports.
    #[field(label = "Quality")]
    pub quality: Option<i64>,
    /// The SNR, if it was measured.
    #[field(shared = keys::SNR_DB)]
    pub snr_db: Option<f64>,
    /// The length of the leader, if it was measured.
    #[field(label = "Leader", unit = "ms")]
    pub leader_ms: Option<i64>,
    /// The header read.
    #[field(label = "Header good")]
    pub header_ok: bool,
    /// The body was read.
    #[field(label = "Body read")]
    pub body_read: bool,
    /// The body is good.
    #[field(label = "Body good")]
    pub body_ok: bool,
    /// Reed-Solomon corrections.
    #[field(label = "RS corrections")]
    pub rs_corrections: i64,
    /// The offset of the tones.
    #[field(label = "Tone offset", unit = "Hz")]
    pub tone_offset_hz: Hertz,
    /// The SNR of the tones.
    #[field(label = "Tone SNR", unit = "dB")]
    pub tone_snr_db: f64,
}

/// A row of the ARDOP stations. The row key is the callsign.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct ArdopStationRow<'a> {
    /// The callsign.
    #[field(shared = keys::CALLSIGN)]
    pub callsign: &'a str,
    /// The locator, if one was heard.
    #[field(shared = keys::LOCATOR)]
    pub locator: Option<&'a str>,
    /// The times heard.
    #[field(shared = columns::HEARD)]
    pub heard: Option<i64>,
    /// The UTC of the last time heard.
    #[field(shared = columns::LAST_HEARD)]
    pub last_heard: Option<Time>,
    /// The type of its last frame.
    #[field(label = "Last frame")]
    pub last_type: &'a str,
    /// The station it called, if it called one.
    #[field(label = "Called")]
    pub called: Option<&'a str>,
}

/// The ARDOP frames, newest first.
pub static ARDOP_FRAMES: Table<ArdopFrameRow<'static>> = Table::new("frames", "Frames", 300);
/// The ARDOP stations.
pub static ARDOP_STATIONS: Table<ArdopStationRow<'static>> =
    Table::new("stations", "Stations", 100);

/// The level of the ARDOP signal over time.
pub const ARDOP_LEVELS: SeriesDef = SeriesDef {
    key: "levels",
    label: "Level",
    x_unit: "s",
    y_unit: "dB",
};

/// The fields of an ARDOP frame heard.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct ArdopFrameEvent<'a> {
    /// The sender, if the frame names one.
    #[field(shared = keys::SOURCE)]
    pub source: Option<&'a str>,
    /// The addressee, if the frame names one.
    #[field(shared = keys::TARGET)]
    pub target: Option<&'a str>,
    /// The family of the source: a message.
    #[field(shared = keys::FAMILY)]
    pub family: Family,
    /// The name of the type.
    #[field(label = "Name")]
    pub name: &'a str,
    /// The frame type.
    #[field(label = "Type")]
    pub r#type: i64,
}

/// An ARDOP frame heard.
pub static ARDOP_FRAME: EventKind<ArdopFrameEvent<'static>> =
    EventKind::new("ardop.frame", "Frame heard");

sdrkit_core::choice_enum! {
    /// The state of the ARDOP link.
    pub enum ArdopLinkState {
        /// No link.
        Idle = ("idle", "Idle"),
        /// The stack sends connect requests and waits for the acknowledgement.
        Connecting = ("connecting", "Connecting"),
        /// The link is up and the stack holds the turn to send data.
        Sending = ("sending", "Connected, sending"),
        /// The link is up and the other station holds the turn to send data.
        Receiving = ("receiving", "Connected, receiving"),
        /// The stack sent DISC and waits for END.
        Disconnecting = ("disconnecting", "Disconnecting"),
        /// The link ended; the reason says how.
        Closed = ("closed", "Closed"),
        /// The link ended without an answer; the reason says why.
        Failed = ("failed", "Failed"),
    }
}

/// The result fields of the ARDOP sender and of its link.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct ArdopTxFields<'a> {
    /// Whether a burst is on air.
    #[field(label = "Sending")]
    pub tx_sending: bool,
    /// The type of the frame on air. No value while nothing is sent.
    #[field(label = "Frame sent")]
    pub tx_frame: Option<&'a str>,
    /// The bursts waiting to be sent.
    #[field(label = "Bursts waiting")]
    pub tx_waiting: i64,
    /// The state of the link.
    #[field(label = "Link")]
    pub link_state: ArdopLinkState,
    /// The other station of the link. No value while no link was made.
    #[field(label = "Link station")]
    pub link_peer: Option<&'a str>,
    /// The session id of the link. No value while no link was made.
    #[field(label = "Session")]
    pub link_session: Option<i64>,
    /// The data bytes of the stack that the other station acknowledged.
    #[field(label = "Bytes sent")]
    pub link_sent: i64,
    /// The data bytes that the other station sent and the stack received.
    #[field(label = "Bytes received")]
    pub link_received: i64,
    /// The data bytes waiting to be sent on the link.
    #[field(label = "Bytes waiting")]
    pub link_waiting: i64,
    /// The repeats in a row of the frame that waits for its answer.
    #[field(label = "Repeats")]
    pub link_retries: i64,
    /// Why the link state is what it is; empty when it needs no reason.
    #[field(label = "Link reason")]
    pub link_reason: &'a str,
    /// The text received on the link, the newest 4000 bytes. No value before the first byte.
    #[field(label = "Link text")]
    pub link_text: Option<&'a str>,
    /// The station of the newest connect request to the station callsign that the stack did not answer.
    #[field(label = "Request from")]
    pub link_request: Option<&'a str>,
}

/// The fields of a change of the ARDOP link.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct ArdopLinkEvent<'a> {
    /// The link state after the change.
    #[field(label = "Link")]
    pub state: ArdopLinkState,
    /// The other station.
    #[field(shared = keys::SOURCE)]
    pub source: &'a str,
    /// What happened: connected, data received, the other station disconnected, a request not answered, or why the link
    /// failed.
    #[field(label = "Reason")]
    pub reason: &'a str,
    /// The data received, as text; empty when none came.
    #[field(shared = keys::TEXT)]
    pub text: &'a str,
    /// The family of the source: a message.
    #[field(shared = keys::FAMILY)]
    pub family: Family,
}

/// A change of the ARDOP link.
pub static ARDOP_LINK: EventKind<ArdopLinkEvent<'static>> =
    EventKind::new("ardop.link", "ARDOP link");

/// Sends a text: on the link while one is up, else as FEC data frames.
pub const ARDOP_SEND: CommandKind<KeyboardSendArgs<'static>> =
    CommandKind::new("send", "Send a text")
        .help(
            "While a link is up, the text joins the data of the link. Otherwise it is sent once as FEC data frames of the \
             FEC frame type, to no station. Transmit must be on and the station callsign set.",
        )
        .transmits(sdrkit_core::TxContent::Text)
        .stopped_by("stop");

/// The arguments of `ping` and `connect`.
#[derive(Arguments)]
pub struct ArdopCallArgs<'a> {
    /// The called station.
    #[arg(
        label = "Called station",
        help = "1 to 8 characters: letters, digits, - and /. Lower case is sent as upper case."
    )]
    pub target: &'a str,
}

/// Sends the ID frame.
pub const ARDOP_SEND_ID: Command = Command::new("send_id", "Send the ID")
    .help("Sends one ID frame with the station callsign and locator. Transmit must be on and the station callsign set.")
    .transmits(sdrkit_core::TxContent::Beacon)
    .stopped_by("stop");
/// Sends a Ping.
pub const ARDOP_PING: CommandKind<ArdopCallArgs<'static>> = CommandKind::new("ping", "Ping")
    .help(
        "Sends one Ping frame from the station callsign to the called station. Transmit must be on and the station \
         callsign set.",
    )
    .transmits(sdrkit_core::TxContent::Frame)
    .stopped_by("stop");
/// Opens a link as the calling station.
pub const ARDOP_CONNECT: CommandKind<ArdopCallArgs<'static>> =
    CommandKind::new("connect", "Connect")
        .help(
            "Calls the station with ConReq500M, every 9 s until it answers, at most 11 times. On the answer the link is \
             up: the text of `send` goes as 4FSK.500.100 data frames, each sent again until the other station \
             acknowledges it. Transmit must be on and the station callsign set.",
        )
        .transmits(sdrkit_core::TxContent::Frame)
        .stopped_by("stop");
/// Ends the link.
pub const ARDOP_DISCONNECT: Command = Command::new("disconnect", "Disconnect")
    .help("Ends the link once the data waiting is sent: DISC, answered by END.")
    .transmits(sdrkit_core::TxContent::Frame)
    .stopped_by("stop");
/// Stops sending.
pub const ARDOP_STOP: Command = Command::new("stop", "Stop sending").help(
    "Ends the burst on air at once, drops the bursts waiting and closes the link without DISC.",
);

// ROS.

/// The result fields of ROS. The fields of the detection have no value without one.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct RosFields<'a> {
    /// The sub-mode found, if the rate gives one.
    #[field(label = "Sub-mode")]
    pub sub_mode: Option<&'a str>,
    /// What was found.
    #[field(label = "Found")]
    pub name: Option<&'a str>,
    /// Why.
    #[field(label = "Why")]
    pub why: Option<&'a str>,
    /// The symbol rate.
    #[field(shared = keys::SYMBOL_RATE_HZ)]
    pub symbol_rate_hz: Option<Hertz>,
    /// The spacing of the tone grid.
    #[field(label = "Grid spacing", unit = "Hz")]
    pub spacing_hz: Option<Hertz>,
    /// How well the tones fit the grid.
    #[field(label = "Grid fit", unit = "%")]
    pub fit_pct: Option<f64>,
    /// The lowest tone.
    #[field(label = "Lowest tone", unit = "Hz")]
    pub lowest_hz: Option<Hertz>,
    /// The highest tone.
    #[field(label = "Highest tone", unit = "Hz")]
    pub highest_hz: Option<Hertz>,
    /// How pure the tones are.
    #[field(label = "Purity", unit = "%")]
    pub purity_pct: Option<f64>,
    /// The tone positions used.
    #[field(label = "Positions used")]
    pub distinct: Option<i64>,
    /// The score.
    #[field(label = "Score", unit = "%")]
    pub score_pct: Option<f64>,
    /// The signal is identified.
    #[field(label = "Identified")]
    pub detected: bool,
    /// Few symbols were seen.
    #[field(label = "Few symbols")]
    pub weak: bool,
    /// The tuning error.
    #[field(shared = keys::OFFSET_HZ)]
    pub offset_hz: Hertz,
    /// Identifications.
    #[field(label = "Identifications")]
    pub detections: i64,
    /// Signals that did not fit.
    #[field(label = "Did not fit")]
    pub misfits: i64,
}

/// A row of the ROS identifications. The row key is the place of the identification in the log.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct RosDetectionRow<'a> {
    /// The signal time.
    #[field(label = "Time", unit = "s")]
    pub time_s: f64,
    /// The sub-mode, if the rate gives one.
    #[field(label = "Sub-mode")]
    pub sub_mode: Option<&'a str>,
    /// What was found.
    #[field(label = "Found")]
    pub name: &'a str,
    /// The symbol rate.
    #[field(shared = keys::SYMBOL_RATE_HZ)]
    pub symbol_rate_hz: Hertz,
    /// Why.
    #[field(label = "Why")]
    pub why: &'a str,
}

/// The ROS identifications, newest first.
pub static ROS_DETECTIONS: Table<RosDetectionRow<'static>> =
    Table::new("detections", "Identifications", 100);

/// The newest spectrum of the ROS band.
pub const ROS_WATERFALL: SeriesDef = SeriesDef {
    key: "waterfall",
    label: "Newest spectrum",
    x_unit: "Hz",
    y_unit: "dB",
};

/// The fields of a ROS identification.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct RosDetectionEvent<'a> {
    /// The sub-mode, if the rate gives one.
    #[field(label = "Sub-mode")]
    pub sub_mode: Option<&'a str>,
    /// What was found.
    #[field(label = "Found")]
    pub name: &'a str,
    /// The symbol rate.
    #[field(shared = keys::SYMBOL_RATE_HZ)]
    pub symbol_rate_hz: Hertz,
}

/// ROS identified.
pub static ROS_DETECTION: EventKind<RosDetectionEvent<'static>> =
    EventKind::new("ros.detection", "ROS identified");

// Throb.

/// The result fields of Throb. The fields of the reading have no value before the first.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct ThrobFields<'a> {
    /// The decoded text.
    #[field(shared = keys::TEXT)]
    pub text: &'a str,
    /// The newest callsign of the text, if it has one.
    #[field(shared = keys::CALLSIGN)]
    pub callsign: Option<&'a str>,
    /// The variant found.
    #[field(label = "Variant")]
    pub variant: Option<&'a str>,
    /// The audio centre found.
    #[field(label = "Audio centre", unit = "Hz")]
    pub centre_hz: Option<Hertz>,
    /// The score of the reading.
    #[field(label = "Score")]
    pub score: Option<f64>,
    /// The SNR of the reading.
    #[field(shared = keys::SNR_DB)]
    pub snr_db: Option<f64>,
    /// Characters decoded.
    #[field(label = "Characters")]
    pub characters: i64,
    /// A note on the stack.
    #[field(label = "Note")]
    pub note: &'a str,
}

/// The fields of the characters of one Throb reading.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct ThrobTextEvent<'a> {
    /// The characters.
    #[field(shared = keys::TEXT)]
    pub text: &'a str,
    /// The variant.
    #[field(label = "Variant")]
    pub variant: &'a str,
    /// The audio centre.
    #[field(label = "Audio centre", unit = "Hz")]
    pub centre_hz: Hertz,
    /// The family of the source: a message.
    #[field(shared = keys::FAMILY)]
    pub family: Family,
}

/// Throb characters.
pub static THROB_TEXT: EventKind<ThrobTextEvent<'static>> =
    EventKind::new("throb.text", "Throb characters");

// QRSS.

/// The result fields of QRSS. The fields of the reading have no value before the first.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct QrssFields<'a> {
    /// The text of the newest reading.
    #[field(shared = keys::TEXT)]
    pub text: &'a str,
    /// The newest callsign of the text, if it has one.
    #[field(shared = keys::CALLSIGN)]
    pub callsign: Option<&'a str>,
    /// The tone, if one is found.
    #[field(label = "Tone", unit = "Hz")]
    pub centre_hz: Option<Hertz>,
    /// The length of an element, while a tone is found.
    #[field(label = "Element length", unit = "s")]
    pub element_s: Option<f64>,
    /// The SNR of the reading, while a tone is found.
    #[field(shared = keys::SNR_DB)]
    pub snr_db: Option<f64>,
    /// A tone is found.
    #[field(label = "Tone found")]
    pub locked: bool,
    /// The picture rows made.
    #[field(label = "Rows")]
    pub picture_rows: i64,
    /// A note on the stack.
    #[field(label = "Note")]
    pub note: &'a str,
}

/// The fields of a QRSS reading.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct QrssTextEvent<'a> {
    /// The text.
    #[field(shared = keys::TEXT)]
    pub text: &'a str,
    /// The length of an element.
    #[field(label = "Element length", unit = "s")]
    pub element_s: f64,
    /// The tone.
    #[field(label = "Tone", unit = "Hz")]
    pub centre_hz: Hertz,
    /// The family of the source: a message.
    #[field(shared = keys::FAMILY)]
    pub family: Family,
}

/// A QRSS reading.
pub static QRSS_TEXT: EventKind<QrssTextEvent<'static>> = EventKind::new("qrss.text", "QRSS text");

// CP16.

/// The result fields of CP16. The fields of the comb and the clock have no value before they are found.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct Cp16Fields<'a> {
    /// The decoded text.
    #[field(shared = keys::TEXT)]
    pub text: &'a str,
    /// The newest callsign of the text, if it has one.
    #[field(shared = keys::CALLSIGN)]
    pub callsign: Option<&'a str>,
    /// The state of the receiver.
    #[field(label = "State")]
    pub state: i64,
    /// The lowest carrier.
    #[field(label = "Lowest carrier", unit = "Hz")]
    pub base_hz: Option<f64>,
    /// The spacing of the carriers.
    #[field(label = "Spacing", unit = "Hz")]
    pub spacing_hz: Option<f64>,
    /// The time of one row.
    #[field(label = "Row time", unit = "ms")]
    pub row_ms: Option<f64>,
    /// The seconds of one character.
    #[field(label = "Seconds a character", unit = "s")]
    pub char_s: Option<f64>,
    /// The signal is on the lower sideband.
    #[field(label = "Lower sideband")]
    pub mirrored: Option<bool>,
    /// The score of the comb.
    #[field(label = "Comb score", unit = "dB")]
    pub comb_db: Option<f64>,
    /// The score of the timing, 0 to 1.
    #[field(label = "Timing score")]
    pub timing: f64,
    /// The SNR.
    #[field(shared = keys::SNR_DB)]
    pub snr_db: Option<f64>,
    /// The picture rows made.
    #[field(label = "Rows")]
    pub picture_rows: i64,
    /// Characters decoded.
    #[field(label = "Characters")]
    pub characters: i64,
    /// Characters read with a clear margin.
    #[field(label = "Sure characters")]
    pub sure: i64,
    /// Characters read without one.
    #[field(label = "Unclear characters")]
    pub unclear: i64,
    /// The font that the characters are read against. It has no value before a font is loaded.
    #[field(label = "Font")]
    pub font_name: Option<&'a str>,
    /// A note on the stack.
    #[field(label = "Note")]
    pub note: &'a str,
}

/// The fields of a CP16 character read.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct Cp16CharEvent<'a> {
    /// The character.
    #[field(shared = keys::TEXT)]
    pub text: &'a str,
    /// Its score against the font.
    #[field(label = "Score")]
    pub score: f64,
    /// Its margin over the next character.
    #[field(label = "Margin")]
    pub margin: f64,
    /// The margin is clear.
    #[field(label = "Sure")]
    pub sure: bool,
    /// The family of the source: a message.
    #[field(shared = keys::FAMILY)]
    pub family: Family,
}

/// The fields of a change of the CP16 transmission.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct Cp16TxEvent<'a> {
    /// The state of the transmission.
    #[field(label = "State")]
    pub state: &'a str,
    /// Why it changed.
    #[field(label = "Reason")]
    pub reason: &'a str,
    /// The text sent.
    #[field(shared = keys::TEXT)]
    pub text: &'a str,
    /// The family of the source: a message.
    #[field(shared = keys::FAMILY)]
    pub family: Family,
}

/// A CP16 character.
pub static CP16_CHAR: EventKind<Cp16CharEvent<'static>> =
    EventKind::new("cp16.char", "CP16 character");
/// A CP16 transmission.
pub static CP16_TX: EventKind<Cp16TxEvent<'static>> =
    EventKind::new("cp16.tx", "CP16 transmission");

/// The arguments of `send`.
#[derive(Arguments)]
pub struct Cp16SendArgs<'a> {
    /// The text to send.
    #[arg(label = "Text", help = "The text, sent once after 0.5 s of silence.")]
    pub text: &'a str,
}

/// Sends a text.
pub const CP16_SEND: CommandKind<Cp16SendArgs<'static>> =
    CommandKind::new("send", "Send a text").help(
        "Sends the text with the stack's spacing, lowest carrier, seconds a character and sideband. Transmit must be on.",
    )
    .transmits(sdrkit_core::TxContent::Text);
/// Stops sending.
pub const CP16_STOP: Command =
    Command::new("stop", "Stop sending").help("Ends the transmission after the current row.");

// WINMOR.

/// The result fields of WINMOR.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct WinmorFields<'a> {
    /// A note on the stack.
    #[field(label = "Note")]
    pub note: &'a str,
    /// Pilots heard.
    #[field(label = "Pilots")]
    pub pilots: i64,
    /// The tuning error of the newest pilot, if one was heard.
    #[field(label = "Tuning error", unit = "Hz")]
    pub error_hz: Option<f64>,
}

/// A row of the WINMOR pilots. The row key is the place of the pilot in the log.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct WinmorPilotRow<'a> {
    /// The UTC.
    #[field(label = "Time")]
    pub time: Time,
    /// The tuning error.
    #[field(label = "Tuning error", unit = "Hz")]
    pub error_hz: f64,
    /// The pilot symbols heard.
    #[field(label = "Pilot symbols")]
    pub pilot_symbols: i64,
    /// The frame-type tones.
    #[field(label = "Tones")]
    pub tones: &'a str,
}

/// The WINMOR pilots, newest first.
pub static WINMOR_PILOTS: Table<WinmorPilotRow<'static>> = Table::new("pilots", "Pilots", 200);

/// The fields of a WINMOR pilot.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct WinmorPilotEvent<'a> {
    /// What was heard.
    #[field(shared = keys::TEXT)]
    pub text: &'a str,
    /// The tuning error.
    #[field(label = "Tuning error", unit = "Hz")]
    pub error_hz: f64,
    /// The pilot symbols heard.
    #[field(label = "Pilot symbols")]
    pub pilot_symbols: i64,
    /// The frame-type tones.
    #[field(label = "Tones")]
    pub tones: &'a str,
}

/// A WINMOR pilot.
pub static WINMOR_PILOT: EventKind<WinmorPilotEvent<'static>> =
    EventKind::new("winmor.pilot", "Pilot");

// The senders of the keyboard text stacks.

/// The result fields of a keyboard sender.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct KeyboardTxFields {
    /// Whether a burst is on air: text or the tune carrier.
    #[field(label = "Sending")]
    pub tx_sending: bool,
    /// The characters of the burst on air coded into the signal so far.
    #[field(label = "Characters sent")]
    pub tx_sent: i64,
    /// The characters waiting to be sent.
    #[field(label = "Characters waiting")]
    pub tx_waiting: i64,
}

/// The fields of a change of a keyboard transmission.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct KeyboardTxEvent<'a> {
    /// The state: started, queued, finished or refused.
    #[field(label = "State")]
    pub state: &'a str,
    /// Why it changed: empty, stopped, the transmitter is off, the format changed, tune carrier, or why a text was refused.
    #[field(label = "Reason")]
    pub reason: &'a str,
    /// The text: as given to `send` for started, queued and refused; as coded for finished.
    #[field(shared = keys::TEXT)]
    pub text: &'a str,
    /// The family of the source: a message.
    #[field(shared = keys::FAMILY)]
    pub family: Family,
}

/// A keyboard transmission.
pub static KEYBOARD_TX: EventKind<KeyboardTxEvent<'static>> =
    EventKind::new("hfkbd.tx", "Keyboard transmission");

/// The arguments of the keyboard `send`.
#[derive(Arguments)]
pub struct KeyboardSendArgs<'a> {
    /// The text to send.
    #[arg(
        label = "Text",
        help = "The text, sent as typed after the folds of the stack. A character the stack cannot send refuses the whole text."
    )]
    pub text: &'a str,
}

/// Sends a text.
pub const KEYBOARD_SEND: CommandKind<KeyboardSendArgs<'static>> =
    CommandKind::new("send", "Send a text")
        .help(
            "Queues the text. A burst starts when none is on air; text sent during a burst joins it. Transmit must be on.",
        )
        .transmits(sdrkit_core::TxContent::Text)
        .stopped_by("stop");
/// Stops sending.
pub const KEYBOARD_STOP: Command = Command::new("stop", "Stop sending")
    .help("Drops the text waiting and ends the signal with a 10 ms fade.");
/// The tune carrier.
pub const KEYBOARD_TUNE: Command = Command::new("tune", "Tune")
    .help("Sends an unmodulated carrier at the audio centre for the tune time.")
    .transmits(sdrkit_core::TxContent::Carrier)
    .stopped_by("stop");

/// The addressee of an FSQ line.
pub const FSQ_TO: sdrkit_core::Setting<field::Text> = sdrkit_core::Setting::new("to", "To", "", "")
    .help("A callsign, allcall or cqcqcq; empty for a line to no one.");
/// The trigger of an FSQ line.
pub const FSQ_TRIGGER: sdrkit_core::Setting<field::Choice<Trigger>> =
    sdrkit_core::Setting::choice("trigger", "Trigger", Trigger::Text)
        .help("What the line asks of the addressee.");

/// The arguments of the FSQ `send`.
#[derive(Arguments)]
pub struct FsqSendArgs<'a> {
    /// The text to send.
    #[arg(label = "Text", help = "The text of the line, sent as typed.")]
    pub text: &'a str,
    /// The addressee.
    #[arg(shared = FSQ_TO)]
    pub to: Option<&'a str>,
    /// The trigger.
    #[arg(shared = FSQ_TRIGGER)]
    pub trigger: Option<Trigger>,
}

/// Sends an FSQ line.
pub const FSQ_SEND: CommandKind<FsqSendArgs<'static>> = CommandKind::new("send", "Send a line")
    .help(
        "Queues one line from the station callsign with its check, to the addressee with the trigger when one is given. \
         Transmit must be on and the station callsign set.",
    )
    .transmits(sdrkit_core::TxContent::Text)
    .stopped_by("stop");

// The stacks.

/// A stack with the sender's fields, event, settings `own` (those the receiver does not have) and commands; `send` is the
/// stack's own send command.
fn sender(
    s: StackShape,
    own: &[&'static dyn DeclareSetting],
    send: impl Into<sdrkit_core::interface::CommandShape>,
) -> StackShape {
    s.fields::<KeyboardTxFields>()
        .event(&KEYBOARD_TX)
        .settings(own)
        .command(send)
        .command(&KEYBOARD_STOP)
        .command(&KEYBOARD_TUNE)
}

/// The stack `id` with the front end's levels first, the decoder's sets `sets`, the dropped count, and the settings of
/// its decoder, of its front end and of its test signal.
fn stack(
    opt: &RxOptions,
    sets: impl FnOnce(StackShape) -> StackShape,
    own: &[&'static dyn DeclareSetting],
    generator: &[&'static dyn DeclareSetting],
) -> StackShape {
    sets(StackShape::new(opt.id).fields::<FrontFields>())
        .fields::<DecoderFields>()
        .settings(own)
        .settings(front_settings(opt))
        .settings(generator)
        .component("front", |c| {
            c.fields::<FrontFields>()
                .shares(&["level_dbfs", "audio_dbfs"])
        })
}

/// A stack that decodes text: the stations, and the spot and text events.
fn text_stack(s: StackShape) -> StackShape {
    s.table(&STATIONS).event(&SPOT).event(&LINE)
}

/// The stack with the text `text`, the callsign `callsign` and the stations kept over a change of frequency.
fn kept_text(s: StackShape, text: &'static str, callsign: &'static str) -> StackShape {
    s.kept(&[text, callsign, "stations"])
}

/// A keyboard stack of many tones; MFSK has `pictures` too.
fn keyboard(
    opt: &RxOptions,
    own: &[&'static dyn DeclareSetting],
    generator: &[&'static dyn DeclareSetting],
    pictures: bool,
) -> StackShape {
    let s = stack(
        opt,
        |s| {
            let s = text_stack(s.fields::<KeyboardFields>());
            if pictures {
                s.fields::<PictureFields>().event(&MFSK_PICTURE)
            } else {
                s
            }
        },
        own,
        generator,
    )
    .command(&REMOVE)
    .command(&sdrkit_graph::decoder::CLEAR)
    .kept(&["KeyboardFields.characters"]);
    let s = kept_text(s, "KeyboardFields.text", "KeyboardFields.callsign");
    if pictures {
        s.kept(&["PictureFields"])
    } else {
        s
    }
}

/// The interface of the family.
pub fn family() -> &'static Described {
    use crate as k;
    static FAMILY: OnceLock<Described> = OnceLock::new();
    FAMILY.get_or_init(|| {
        let rtty = stack(
            &k::RTTY_RX,
            |s| text_stack(s.fields::<RttyFields>()).table(&AUTO_GUESSES),
            k::rtty::RttySetting::SETTINGS,
            k::rtty::RttyGenSetting::SETTINGS,
        )
        .command(&REMOVE)
        .command(&AUTO_LOCK)
        .command(&sdrkit_graph::decoder::CLEAR);
        let rtty = sender(
            rtty,
            &[&k::rtty::TX_CENTRE_HZ, &TX_LEVEL, &TX_TUNE_LIMIT],
            &KEYBOARD_SEND,
        );
        let psk = stack(
            &k::PSK_RX,
            |s| {
                text_stack(s.fields::<PskFields>())
                    .table(&PSK_SIGNALS)
                    .series(PSK_SPECTRUM)
            },
            k::psk::PskSetting::SETTINGS,
            k::psk::PskGenSetting::SETTINGS,
        )
        .command(&REMOVE)
        .command(&sdrkit_graph::decoder::CLEAR)
        .kept(&[
            "stations",
            "signals",
            "PskFields.characters",
            "PskFields.errors",
        ]);
        let psk = sender(
            psk,
            &[&k::psk::TX_CENTRE_HZ, &TX_LEVEL, &TX_TUNE_LIMIT],
            &KEYBOARD_SEND,
        );
        let cw = stack(
            &k::CW_RX,
            |s| {
                text_stack(s.fields::<CwFields>())
                    .table(&CW_SIGNALS)
                    .series(CW_MARKS)
                    .series(CW_GAPS)
                    .series(CW_SPECTRUM)
                    .series(CW_KEY)
            },
            k::cw::CwSetting::SETTINGS,
            k::cw::CwGenSetting::SETTINGS,
        )
        .command(&REMOVE)
        .command(&sdrkit_graph::decoder::CLEAR)
        .kept(&["stations", "signals", "CwFields.characters"]);
        let hell = stack(
            &k::HELL_RX,
            |s| s.fields::<HellFields>(),
            k::hell::HellSetting::SETTINGS,
            k::hell::HellGenSetting::SETTINGS,
        )
        .command(&sdrkit_graph::decoder::CLEAR);
        let hell = sender(
            hell,
            &[&TX_LEVEL, &TX_TUNE_LIMIT, &k::hell::FONT],
            &KEYBOARD_SEND,
        );
        let mt63 = stack(
            &k::MT63_RX,
            |s| {
                text_stack(s.fields::<Mt63Fields>())
                    .series(MT63_SPECTRUM)
                    .series(MT63_CARRIERS)
            },
            k::mt63::Mt63Setting::SETTINGS,
            k::mt63::Mt63GenSetting::SETTINGS,
        )
        .command(&REMOVE)
        .command(&sdrkit_graph::decoder::CLEAR);
        let mt63 = kept_text(mt63, "Mt63Fields.text", "Mt63Fields.callsign").kept(&[
            "Mt63Fields.characters",
            "Mt63Fields.errors",
            "Mt63Fields.locks",
        ]);
        let mt63 = sender(
            mt63,
            &[&k::mt63::TX_LOW_HZ, &TX_LEVEL, &TX_TUNE_LIMIT],
            &KEYBOARD_SEND,
        );
        let fsq = |opt: &RxOptions, own: &[&'static dyn DeclareSetting], messages: bool| {
            let s = stack(
                opt,
                |s| {
                    let s = text_stack(s.fields::<FsqFields>());
                    let s = if messages { s.table(&FSQ_MESSAGES) } else { s };
                    let s = s.series(TONE_USE);
                    if messages {
                        s.event(&FSQ_MESSAGE)
                    } else {
                        s
                    }
                },
                own,
                k::fsq::FsqGenSetting::SETTINGS,
            )
            .command(&REMOVE)
            .command(&sdrkit_graph::decoder::CLEAR);
            let s = kept_text(s, "FsqFields.text", "FsqFields.callsign")
                .kept(&["FsqFields.characters"]);
            if messages {
                s.kept(&["messages"])
            } else {
                s
            }
        };
        let sstv = stack(
            &k::SSTV_RX,
            |s| {
                s.fields::<SstvFields>()
                    .fields::<PictureFields>()
                    .event(&SSTV_PICTURE)
            },
            k::sstv::SstvSetting::SETTINGS,
            k::sstv::SstvGenSetting::SETTINGS,
        )
        .fields::<SstvTxFields>()
        .event(&SSTV_TX)
        .settings(k::sstv::TX_OWN_SETTINGS)
        .command(&sdrkit_graph::decoder::CLEAR)
        .command(&SSTV_SEND)
        .command(&SSTV_STOP)
        .kept(&["PictureFields"]);
        let sitor = stack(
            &k::SITOR_RX,
            |s| text_stack(s.fields::<SitorFields>()).event(&SITOR_ARQ),
            &[],
            k::sitor::SitorGenSetting::SETTINGS,
        )
        .command(&REMOVE)
        .command(&sdrkit_graph::decoder::CLEAR);
        let sitor = kept_text(sitor, "SitorFields.text", "SitorFields.callsign")
            .kept(&["SitorFields.arq_text"]);
        let ardop = stack(
            &k::ARDOP_RX,
            |s| {
                s.fields::<ArdopFields>()
                    .table(&ARDOP_FRAMES)
                    .table(&ARDOP_STATIONS)
                    .series(ARDOP_LEVELS)
                    .event(&ARDOP_FRAME)
            },
            &[],
            &[],
        )
        .fields::<ArdopTxFields>()
        .event(&KEYBOARD_TX)
        .event(&ARDOP_LINK)
        .settings(k::ardop::tx::OWN_SETTINGS)
        .command(&REMOVE)
        .command(&sdrkit_graph::decoder::CLEAR)
        .command(&ARDOP_SEND)
        .command(&ARDOP_SEND_ID)
        .command(&ARDOP_PING)
        .command(&ARDOP_CONNECT)
        .command(&ARDOP_DISCONNECT)
        .command(&ARDOP_STOP)
        .kept(&["frames", "stations", "ArdopFields"]);
        let ros = stack(
            &k::ROS_RX,
            |s| {
                s.fields::<RosFields>()
                    .table(&ROS_DETECTIONS)
                    .series(ROS_WATERFALL)
                    .event(&ROS_DETECTION)
            },
            &[],
            k::ros::RosGenSetting::SETTINGS,
        )
        .command(&sdrkit_graph::decoder::CLEAR)
        .kept(&["detections", "RosFields.detections", "RosFields.misfits"]);
        let throb = stack(
            &k::THROB_RX,
            |s| text_stack(s.fields::<ThrobFields>()).event(&THROB_TEXT),
            k::throb::ThrobSetting::SETTINGS,
            k::throb::ThrobGenSetting::SETTINGS,
        )
        .command(&REMOVE)
        .command(&sdrkit_graph::decoder::CLEAR);
        let throb = kept_text(throb, "ThrobFields.text", "ThrobFields.callsign")
            .kept(&["ThrobFields.characters"]);
        let qrss = stack(
            &k::QRSS_RX,
            |s| text_stack(s.fields::<QrssFields>()).event(&QRSS_TEXT),
            k::qrss::QrssSetting::SETTINGS,
            k::qrss::QrssGenSetting::SETTINGS,
        )
        .command(&sdrkit_graph::decoder::CLEAR);
        let qrss = kept_text(qrss, "QrssFields.text", "QrssFields.callsign");
        let cp16 = stack(
            &k::cp16::CP16_RX,
            |s| {
                text_stack(s.fields::<Cp16Fields>())
                    .event(&CP16_CHAR)
                    .event(&CP16_TX)
            },
            k::cp16::Cp16Setting::SETTINGS,
            k::cp16::Cp16GenSetting::SETTINGS,
        )
        .command(&sdrkit_graph::decoder::CLEAR)
        .command(&CP16_SEND)
        .command(&CP16_STOP);
        let cp16 = kept_text(cp16, "Cp16Fields.text", "Cp16Fields.callsign").kept(&[
            "Cp16Fields.picture_rows",
            "Cp16Fields.characters",
            "Cp16Fields.sure",
            "Cp16Fields.unclear",
        ]);
        let winmor = stack(
            &k::WINMOR_RX,
            |s| {
                s.fields::<WinmorFields>()
                    .table(&WINMOR_PILOTS)
                    .event(&WINMOR_PILOT)
            },
            &[],
            k::winmor::WinmorGenSetting::SETTINGS,
        )
        .command(&sdrkit_graph::decoder::CLEAR)
        .kept(&["pilots", "WinmorFields.pilots"]);
        let olivia = k::olivia::OliviaSetting::SETTINGS;
        let olivia_gen = k::olivia::OliviaGenSetting::SETTINGS;
        let ifk_gen = k::domino::IfkGenSetting::SETTINGS;
        let mut f = Described::new("hfkbd", 2, 6)
            .added(2, 6, ADDED_2_6)
            .added(2, 5, ADDED_2_5)
            .added(2, 4, ADDED_2_4)
            .added(2, 3, ADDED_2_3)
            .added(
                2,
                2,
                &["shape.CwFields.playing", "shape.CwFields.listened_hz"],
            )
            .added(
                2,
                1,
                &["shape.CwFields.listened", "cw.setting.gen_noise_only"],
            )
            .added(
                2,
                0,
                &[
                    "shape.CwFields.span_hz",
                    "shape.CwFields.signals",
                    "shape.CwFields.now_s",
                    "shape.CwSignalRow.freq_hz",
                    "shape.CwSignalRow.level_dbfs",
                    "cw.series.spectrum",
                    "cw.series.key",
                    "cw.setting.filter",
                    "cw.setting.listen",
                    "cw.setting.forget_s",
                    "cw.setting.span",
                    "cw.setting.gen_signals",
                    "cw.setting.gen_wpm_max",
                    "cw.setting.gen_level_spread_db",
                ],
            );
        for (name, users) in SHARED {
            f = f.shared_source(name, users);
        }
        let rtty = kept_text(rtty, "RttyFields.text", "RttyFields.callsign")
            .kept(&["RttyFields.characters", "RttyFields.framing_errors"]);
        f.stack(rtty)
            .stack(psk)
            .stack(cw)
            .stack(sender(
                keyboard(&k::OLIVIA_RX, olivia, olivia_gen, false),
                &[&TX_LEVEL, &TX_TUNE_LIMIT],
                &KEYBOARD_SEND,
            ))
            .stack(sender(
                keyboard(&k::CONTESTIA_RX, olivia, olivia_gen, false),
                &[&TX_LEVEL, &TX_TUNE_LIMIT],
                &KEYBOARD_SEND,
            ))
            .stack(sender(
                keyboard(
                    &k::MFSK_RX,
                    k::mfsk::MfskSetting::SETTINGS,
                    k::mfsk::MfskGenSetting::SETTINGS,
                    true,
                ),
                &[&TX_LEVEL, &TX_TUNE_LIMIT],
                &KEYBOARD_SEND,
            ))
            .stack(sender(
                keyboard(
                    &k::DOMINOEX_RX,
                    k::domino::DominoSetting::SETTINGS,
                    ifk_gen,
                    false,
                ),
                &[&TX_LEVEL, &TX_TUNE_LIMIT],
                &KEYBOARD_SEND,
            ))
            .stack(sender(
                keyboard(
                    &k::THOR_RX,
                    k::domino::ThorSetting::SETTINGS,
                    ifk_gen,
                    false,
                ),
                &[&TX_LEVEL, &TX_TUNE_LIMIT],
                &KEYBOARD_SEND,
            ))
            .stack(hell)
            .stack(mt63)
            .stack(sender(
                fsq(&k::FSQ_RX, k::fsq::FsqSetting::SETTINGS, true),
                &[&k::fsq::TX_CENTRE_HZ, &TX_LEVEL, &TX_TUNE_LIMIT],
                &FSQ_SEND,
            ))
            .stack(sender(
                fsq(&k::IFKP_RX, k::fsq::IfkpSetting::SETTINGS, false),
                &[&k::fsq::TX_CENTRE_HZ, &TX_LEVEL, &TX_TUNE_LIMIT],
                &KEYBOARD_SEND,
            ))
            .stack(sstv)
            .stack(sitor)
            .stack(ardop)
            .stack(ros)
            .stack(throb)
            .stack(qrss)
            .stack(cp16)
            .stack(winmor)
    })
}

/// What version 2.6 added: the sender and the link of ARDOP, the sideband, FM and repeat of the SSTV sender, the font of the
/// Hell sender.
const ADDED_2_6: &[&str] = &[
    "ardop.fields.ArdopTxFields",
    "shape.ArdopTxFields",
    "shape.ArdopTxFields.tx_sending",
    "shape.ArdopTxFields.tx_frame",
    "shape.ArdopTxFields.tx_waiting",
    "shape.ArdopTxFields.link_state",
    "shape.ArdopTxFields.link_peer",
    "shape.ArdopTxFields.link_session",
    "shape.ArdopTxFields.link_sent",
    "shape.ArdopTxFields.link_received",
    "shape.ArdopTxFields.link_waiting",
    "shape.ArdopTxFields.link_retries",
    "shape.ArdopTxFields.link_reason",
    "shape.ArdopTxFields.link_text",
    "shape.ArdopTxFields.link_request",
    "ardop.event.hfkbd.tx",
    "ardop.event.ardop.link",
    "shape.ArdopLinkEvent",
    "shape.ArdopLinkEvent.state",
    "shape.ArdopLinkEvent.source",
    "shape.ArdopLinkEvent.reason",
    "shape.ArdopLinkEvent.text",
    "shape.ArdopLinkEvent.family",
    "ardop.setting.leader_ms",
    "ardop.setting.fec_type",
    "ardop.setting.auto_answer",
    "ardop.command.send",
    "ardop.command.send_id",
    "ardop.command.ping",
    "ardop.command.connect",
    "ardop.command.disconnect",
    "ardop.command.stop",
    "shape.SstvTxFields.tx_next_s",
    "sstv.setting.repeat",
    "sstv.setting.pause_s",
    "hell.setting.font",
];

/// What version 2.5 added: the senders of MFSK, Olivia, Contestia and MT63.
const ADDED_2_5: &[&str] = &[
    "mfsk.fields.KeyboardTxFields",
    "mfsk.event.hfkbd.tx",
    "mfsk.setting.tx_level",
    "mfsk.setting.tx_tune_limit",
    "mfsk.command.send",
    "mfsk.command.stop",
    "mfsk.command.tune",
    "olivia.fields.KeyboardTxFields",
    "olivia.event.hfkbd.tx",
    "olivia.setting.tx_level",
    "olivia.setting.tx_tune_limit",
    "olivia.command.send",
    "olivia.command.stop",
    "olivia.command.tune",
    "contestia.fields.KeyboardTxFields",
    "contestia.event.hfkbd.tx",
    "contestia.setting.tx_level",
    "contestia.setting.tx_tune_limit",
    "contestia.command.send",
    "contestia.command.stop",
    "contestia.command.tune",
    "mt63.fields.KeyboardTxFields",
    "mt63.event.hfkbd.tx",
    "mt63.setting.tx_low_hz",
    "mt63.setting.tx_level",
    "mt63.setting.tx_tune_limit",
    "mt63.command.send",
    "mt63.command.stop",
    "mt63.command.tune",
];

/// What version 2.4 added: the senders of RTTY, PSK, DominoEX, THOR, Hell, FSQ and IFKP.
const ADDED_2_4: &[&str] = &[
    "rtty.fields.KeyboardTxFields",
    "rtty.event.hfkbd.tx",
    "rtty.setting.tx_centre_hz",
    "rtty.setting.tx_level",
    "rtty.setting.tx_tune_limit",
    "rtty.command.send",
    "rtty.command.stop",
    "rtty.command.tune",
    "psk.fields.KeyboardTxFields",
    "psk.event.hfkbd.tx",
    "psk.setting.tx_centre_hz",
    "psk.setting.tx_level",
    "psk.setting.tx_tune_limit",
    "psk.command.send",
    "psk.command.stop",
    "psk.command.tune",
    "dominoex.fields.KeyboardTxFields",
    "dominoex.event.hfkbd.tx",
    "dominoex.setting.tx_level",
    "dominoex.setting.tx_tune_limit",
    "dominoex.command.send",
    "dominoex.command.stop",
    "dominoex.command.tune",
    "thor.fields.KeyboardTxFields",
    "thor.event.hfkbd.tx",
    "thor.setting.tx_level",
    "thor.setting.tx_tune_limit",
    "thor.command.send",
    "thor.command.stop",
    "thor.command.tune",
    "hell.fields.KeyboardTxFields",
    "hell.event.hfkbd.tx",
    "hell.setting.tx_level",
    "hell.setting.tx_tune_limit",
    "hell.command.send",
    "hell.command.stop",
    "hell.command.tune",
    "fsq.fields.KeyboardTxFields",
    "fsq.event.hfkbd.tx",
    "fsq.setting.tx_centre_hz",
    "fsq.setting.tx_level",
    "fsq.setting.tx_tune_limit",
    "fsq.command.send",
    "fsq.command.stop",
    "fsq.command.tune",
    "ifkp.fields.KeyboardTxFields",
    "ifkp.event.hfkbd.tx",
    "ifkp.setting.tx_centre_hz",
    "ifkp.setting.tx_level",
    "ifkp.setting.tx_tune_limit",
    "ifkp.command.send",
    "ifkp.command.stop",
    "ifkp.command.tune",
    "shape.KeyboardTxFields",
    "shape.KeyboardTxFields.tx_sending",
    "shape.KeyboardTxFields.tx_sent",
    "shape.KeyboardTxFields.tx_waiting",
    "shape.KeyboardTxEvent",
    "shape.KeyboardTxEvent.state",
    "shape.KeyboardTxEvent.reason",
    "shape.KeyboardTxEvent.text",
    "shape.KeyboardTxEvent.family",
];

/// What version 2.3 added: the transmitter of SSTV.
const ADDED_2_3: &[&str] = &[
    "sstv.fields.SstvTxFields",
    "shape.SstvTxFields",
    "shape.SstvTxFields.tx_sending",
    "shape.SstvTxFields.tx_mode",
    "shape.SstvTxFields.tx_line",
    "shape.SstvTxFields.tx_lines",
    "shape.SstvTxFields.tx_picture_s",
    "sstv.event.sstv.tx",
    "shape.SstvTxEvent",
    "shape.SstvTxEvent.state",
    "shape.SstvTxEvent.reason",
    "shape.SstvTxEvent.mode",
    "shape.SstvTxEvent.family",
    "sstv.setting.tx_mode",
    "sstv.setting.fit",
    "sstv.command.send",
    "sstv.command.stop",
];

/// Every stack of the family.
const ALL: &[&str] = &[
    "rtty",
    "psk",
    "cw",
    "olivia",
    "contestia",
    "mfsk",
    "dominoex",
    "thor",
    "hell",
    "mt63",
    "fsq",
    "ifkp",
    "sstv",
    "sitor",
    "ardop",
    "ros",
    "throb",
    "qrss",
    "cp16",
    "winmor",
];

/// The files that several stacks use, with the stacks that use them.
const SHARED: &[(&str, &[&str])] = &[
    ("decoder", ALL),
    ("front", ALL),
    ("kinds", ALL),
    ("util", ALL),
    ("verify", ALL),
    ("fec", ALL),
    ("finder", ALL),
    ("auto", &["rtty", "psk"]),
    ("picture", &["mfsk", "sstv"]),
    ("domino", &["dominoex", "thor"]),
    ("olivia", &["olivia", "contestia"]),
    (
        "sender",
        &[
            "rtty",
            "psk",
            "olivia",
            "contestia",
            "mfsk",
            "dominoex",
            "thor",
            "hell",
            "mt63",
            "fsq",
            "ifkp",
        ],
    ),
];

/// The description of the stack `id`.
///
/// # Panics
///
/// If the family does not describe the stack.
pub fn shape(id: &str) -> &'static StackShape {
    family()
        .stack_shape(id)
        .unwrap_or_else(|| panic!("the hfkbd family does not describe the stack {id}"))
}

/// The settings of the stack `id`, made once into `cell`.
pub fn schema(cell: &'static OnceLock<Arc<Schema>>, id: &str) -> Arc<Schema> {
    cell.get_or_init(|| Arc::new(shape(id).schema())).clone()
}

/// An empty store of the results of the stack `id`, for its decoder.
pub fn results(id: &str) -> Published {
    Published::from_decl(Arc::new(shape(id).results_decl()))
}
