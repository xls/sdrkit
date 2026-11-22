//! The interface of the FM stack: the shape of its result fields, of each table row and of its events as a struct, the
//! tables and event kinds, and the description of the family ([`family`]). A component fills a struct to publish; a
//! consumer reads one.

use sdrkit_core::interface::{EventKind, Family, Fields, Hertz, Seconds, StackShape, Table, Time};
use sdrkit_core::publish::{keys, SeriesDef};
use sdrkit_core::{Command, SettingSet, TxContent};
use std::sync::OnceLock;

/// The result fields of the FM receiver, in the order a screen lists them. The display writes the band plan ([`BandPart`]) and the measurements and RDS data ([`RdsPart`]); the broadcast data publisher writes the RadioText and whether its item is running.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct FmFields<'a> {
    /// The station name (PS): all 8 characters with trailing spaces, empty before the first name.
    #[field(label = "Station")]
    pub station: &'a str,
    /// The name being collected (`_` for a segment not yet received).
    #[field(label = "Station so far")]
    pub station_partial: &'a str,
    /// The RadioText.
    #[field(shared = keys::TEXT)]
    pub text: &'a str,
    /// Every RadioText segment has arrived.
    #[field(label = "RadioText complete")]
    pub text_complete: bool,
    /// The programme type's name in the table in force.
    #[field(label = "Programme type")]
    pub programme: Option<&'a str>,
    /// The programme type code.
    #[field(label = "PTY code")]
    pub pty: Option<i64>,
    /// The programme identification, four hexadecimal digits.
    #[field(label = "PI")]
    pub pi: Option<&'a str>,
    /// RBDS call letters.
    #[field(label = "Call letters")]
    pub call_letters: Option<&'a str>,
    /// RBDS meanings apply.
    #[field(label = "RBDS meanings")]
    pub rbds: bool,
    /// Traffic programme.
    #[field(label = "Traffic programme")]
    pub tp: bool,
    /// Traffic announcement.
    #[field(label = "Traffic announcement")]
    pub ta: bool,
    /// Music (true) or speech.
    #[field(label = "Music")]
    pub music: bool,
    /// Programme type name (PTYN).
    #[field(label = "Programme type name")]
    pub ptyn: &'a str,
    /// Extended country code.
    #[field(label = "Extended country code")]
    pub ecc: Option<i64>,
    /// Language code.
    #[field(label = "Language code")]
    pub language: Option<i64>,
    /// Programme item number, `DD HH:MM`.
    #[field(label = "Programme item")]
    pub pin: Option<&'a str>,
    /// The station's clock time.
    #[field(label = "Station clock")]
    pub clock: Option<Time>,
    /// The station's local time offset.
    #[field(label = "Local time offset", unit = "s")]
    pub clock_offset: Option<Seconds>,
    /// Enhanced RadioText.
    #[field(label = "Enhanced RadioText")]
    pub ert: &'a str,
    /// The RadioText+ item is running.
    #[field(shared = sdrkit_proto_bcdata::publish::ITEM_RUNNING)]
    pub item_running: bool,
    /// RDS block sync.
    #[field(label = "RDS sync")]
    pub rds_sync: bool,
    /// RDS usable blocks.
    #[field(label = "RDS usable blocks", unit = "%")]
    pub rds_ok_pct: f64,
    /// Accepted RDS groups.
    #[field(label = "RDS groups")]
    pub groups: i64,
    /// Good RDS blocks.
    #[field(label = "Good blocks")]
    pub blocks_good: i64,
    /// Corrected RDS blocks (voted ones too).
    #[field(label = "Corrected blocks")]
    pub blocks_corrected: i64,
    /// Bad RDS blocks.
    #[field(label = "Bad blocks")]
    pub blocks_bad: i64,
    /// RDS blocks recovered by voting.
    #[field(label = "Voted blocks")]
    pub blocks_voted: i64,
    /// The signal to noise ratio.
    #[field(shared = keys::SNR_DB)]
    pub snr_db: f64,
    /// The level of the channel.
    #[field(shared = keys::LEVEL_DBFS)]
    pub level_dbfs: f64,
    /// The offset of the carrier.
    #[field(shared = keys::OFFSET_HZ)]
    pub offset_hz: Hertz,
    /// A carrier is present.
    #[field(label = "Carrier")]
    pub carrier: bool,
    /// Stereo is reported.
    #[field(label = "Stereo")]
    pub stereo: bool,
    /// The stereo blend while stereo is reported.
    #[field(label = "Stereo blend", unit = "%")]
    pub stereo_pct: f64,
    /// The pilot injection, percent of 75 kHz.
    #[field(label = "Pilot", unit = "%")]
    pub pilot_pct: f64,
    /// The pilot PLL is locked.
    #[field(label = "Pilot locked")]
    pub pilot_locked: bool,
    /// The peak deviation.
    #[field(label = "Peak deviation", unit = "Hz")]
    pub deviation_hz: Hertz,
    /// The multipath ripple.
    #[field(label = "Multipath", unit = "%")]
    pub multipath_pct: f64,
    /// The equaliser is in use for the sound.
    #[field(label = "Equaliser")]
    pub equaliser: bool,
    /// The squelch is closed.
    #[field(label = "Squelched")]
    pub squelched: bool,
    /// The channel filter width in use.
    #[field(label = "Channel width", unit = "Hz")]
    pub channel_width_hz: Hertz,
    /// The upper frequency of the `mpx` series.
    #[field(label = "MPX top", unit = "Hz")]
    pub mpx_top_hz: Hertz,
    /// Signal decoded since the last reset.
    #[field(label = "Signal decoded", unit = "s")]
    pub signal_time: Seconds,
    /// The lowest frequency of the band.
    #[field(label = "Band start", unit = "Hz")]
    pub band_start_hz: Hertz,
    /// The highest frequency of the band.
    #[field(label = "Band end", unit = "Hz")]
    pub band_end_hz: Hertz,
    /// The channel step.
    #[field(label = "Channel step", unit = "Hz")]
    pub band_step_hz: Hertz,
    /// The first channel of the raster.
    #[field(label = "First channel", unit = "Hz")]
    pub band_first_hz: Hertz,
    /// TMC location table number.
    #[field(label = "TMC location table")]
    pub tmc_ltn: Option<i64>,
    /// TMC service identifier.
    #[field(label = "TMC service")]
    pub tmc_sid: Option<i64>,
    /// TMC alternative-frequency indicator.
    #[field(label = "TMC alternative frequencies")]
    pub tmc_afi: bool,
    /// TMC enhanced mode.
    #[field(label = "TMC enhanced mode")]
    pub tmc_enhanced_mode: bool,
    /// TMC geographical scope bits (I, N, R, U).
    #[field(label = "TMC scope bits")]
    pub tmc_scope: i64,
}

/// The part of [`FmFields`] that describes the band plan.
#[derive(Clone, Debug, PartialEq, Fields)]
#[fields(part_of = FmFields<'static>)]
pub struct BandPart {
    /// The lowest frequency of the band.
    pub band_start_hz: Hertz,
    /// The highest frequency of the band.
    pub band_end_hz: Hertz,
    /// The channel step.
    pub band_step_hz: Hertz,
    /// The first channel of the raster.
    pub band_first_hz: Hertz,
}

/// The measurements of the FM receiver and the RDS data: a part of [`FmFields`], and a result set of its own in a stack
/// that runs the FM receiver without the band plan.
#[derive(Clone, Debug, PartialEq, Fields)]
#[fields(part_of = FmFields<'static>)]
pub struct RdsPart<'a> {
    /// The station name (PS): all 8 characters with trailing spaces, empty before the first name.
    pub station: &'a str,
    /// The name being collected (`_` for a segment not yet received).
    pub station_partial: &'a str,
    /// Every RadioText segment has arrived.
    pub text_complete: bool,
    /// The programme type's name in the table in force.
    pub programme: Option<&'a str>,
    /// The programme type code.
    pub pty: Option<i64>,
    /// The programme identification, four hexadecimal digits.
    pub pi: Option<&'a str>,
    /// RBDS call letters.
    pub call_letters: Option<&'a str>,
    /// RBDS meanings apply.
    pub rbds: bool,
    /// Traffic programme.
    pub tp: bool,
    /// Traffic announcement.
    pub ta: bool,
    /// Music (true) or speech.
    pub music: bool,
    /// Programme type name (PTYN).
    pub ptyn: &'a str,
    /// Extended country code.
    pub ecc: Option<i64>,
    /// Language code.
    pub language: Option<i64>,
    /// Programme item number, `DD HH:MM`.
    pub pin: Option<&'a str>,
    /// The station's clock time.
    pub clock: Option<Time>,
    /// The station's local time offset.
    pub clock_offset: Option<Seconds>,
    /// Enhanced RadioText.
    pub ert: &'a str,
    /// RDS block sync.
    pub rds_sync: bool,
    /// RDS usable blocks.
    pub rds_ok_pct: f64,
    /// Accepted RDS groups.
    pub groups: i64,
    /// Good RDS blocks.
    pub blocks_good: i64,
    /// Corrected RDS blocks (voted ones too).
    pub blocks_corrected: i64,
    /// Bad RDS blocks.
    pub blocks_bad: i64,
    /// RDS blocks recovered by voting.
    pub blocks_voted: i64,
    /// The signal to noise ratio.
    pub snr_db: f64,
    /// The level of the channel.
    pub level_dbfs: f64,
    /// The offset of the carrier.
    pub offset_hz: Hertz,
    /// A carrier is present.
    pub carrier: bool,
    /// Stereo is reported.
    pub stereo: bool,
    /// The stereo blend while stereo is reported.
    pub stereo_pct: f64,
    /// The pilot injection, percent of 75 kHz.
    pub pilot_pct: f64,
    /// The pilot PLL is locked.
    pub pilot_locked: bool,
    /// The peak deviation.
    pub deviation_hz: Hertz,
    /// The multipath ripple.
    pub multipath_pct: f64,
    /// The equaliser is in use for the sound.
    pub equaliser: bool,
    /// The squelch is closed.
    pub squelched: bool,
    /// The channel filter width in use.
    pub channel_width_hz: Hertz,
    /// The upper frequency of the `mpx` series.
    pub mpx_top_hz: Hertz,
    /// Signal decoded since the last reset.
    pub signal_time: Seconds,
    /// TMC location table number.
    pub tmc_ltn: Option<i64>,
    /// TMC service identifier.
    pub tmc_sid: Option<i64>,
    /// TMC alternative-frequency indicator.
    pub tmc_afi: bool,
    /// TMC enhanced mode.
    pub tmc_enhanced_mode: bool,
    /// TMC geographical scope bits (I, N, R, U).
    pub tmc_scope: i64,
}

/// A row of the alternative frequencies. The row key is the place in the list (`00`).
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct AfRow {
    /// A frequency.
    #[field(label = "Frequency", unit = "Hz")]
    pub freq_hz: Hertz,
}

/// The alternative frequencies.
pub static AF: Table<AfRow> = Table::new("af", "Alternative frequencies", 32);

/// A row of the tags of the RadioText, which the broadcast data publisher writes. The row key is the class name of the content type (`ITEM.TITLE`), or its code.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct LabelTagRow<'a> {
    /// The content type code (0 to 127).
    #[field(shared = sdrkit_proto_bcdata::publish::CONTENT_CODE)]
    pub code: i64,
    /// What people call the content type ("Title").
    #[field(shared = sdrkit_proto_bcdata::publish::CONTENT_LABEL)]
    pub r#type: Option<&'a str>,
    /// The tagged text.
    #[field(shared = keys::TEXT)]
    pub text: &'a str,
}

/// The tags of the RadioText.
pub static LABEL_TAGS: Table<LabelTagRow<'static>> = Table::new(
    sdrkit_proto_bcdata::publish::LABEL_TAGS.key,
    sdrkit_proto_bcdata::publish::LABEL_TAGS.label,
    sdrkit_proto_bcdata::publish::LABEL_TAGS.cap,
);

/// A row of the open data applications. The row key is the AID in hexadecimal.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct OdaRow<'a> {
    /// An open data application identifier.
    #[field(label = "Application")]
    pub aid: i64,
    /// The group type that carries an application ("11A").
    #[field(label = "Group")]
    pub group: &'a str,
}

/// The open data applications.
pub static ODA: Table<OdaRow<'static>> = Table::new("oda", "Open data applications", 16);

/// A row of the other networks. The row key is the PI in hexadecimal.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct EonRow<'a> {
    /// The programme identification, four hexadecimal digits.
    #[field(label = "PI")]
    pub pi: &'a str,
    /// A name.
    #[field(label = "Name")]
    pub name: &'a str,
    /// Traffic programme.
    #[field(label = "Traffic programme")]
    pub tp: bool,
    /// Traffic announcement.
    #[field(label = "Traffic announcement")]
    pub ta: bool,
    /// The programme type code.
    #[field(label = "PTY code")]
    pub pty: Option<i64>,
    /// Programme item number, `DD HH:MM`.
    #[field(label = "Programme item")]
    pub pin: Option<&'a str>,
}

/// The other networks.
pub static EON: Table<EonRow<'static>> =
    Table::new("eon", "Other networks", 16).label(&EonRow::NAME);

/// A row of the alternative frequencies of the other networks. The row key is `<PI>/<place>` (`4A21/00`).
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct EonAfRow<'a> {
    /// The PI of the other network a row belongs to.
    #[field(label = "Network", refers_to = "eon")]
    pub network: &'a str,
    /// A frequency.
    #[field(label = "Frequency", unit = "Hz")]
    pub freq_hz: Hertz,
}

/// The alternative frequencies of the other networks.
pub static EON_AF: Table<EonAfRow<'static>> =
    Table::new("eon_af", "Other networks' frequencies", 16 * 25);

/// A row of the traffic messages. The row key is the place, oldest first (`00`).
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct TmcRow {
    /// A TMC event code.
    #[field(label = "Event")]
    pub event: i64,
    /// A TMC location code.
    #[field(label = "Location")]
    pub location: i64,
    /// The TMC direction bit.
    #[field(label = "Negative direction")]
    pub negative_direction: bool,
    /// The TMC extent.
    #[field(label = "Extent")]
    pub extent: i64,
    /// The TMC diversion bit.
    #[field(label = "Diversion")]
    pub diversion: bool,
    /// The TMC duration code.
    #[field(label = "Duration code")]
    pub duration: i64,
    /// How often a message came.
    #[field(label = "Repeats")]
    pub repeats: i64,
    /// When a message came first.
    #[field(label = "First")]
    pub first: Time,
    /// When a message came last.
    #[field(label = "Last")]
    pub last: Time,
    /// When a message expires.
    #[field(label = "Expires")]
    pub expires: Time,
}

/// The traffic messages.
pub static TMC: Table<TmcRow> = Table::new("tmc", "Traffic messages", 64);

/// The fields of the two events of the FM receiver.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct StationEvent<'a> {
    /// The PI, four hexadecimal digits.
    #[field(shared = keys::SOURCE)]
    pub source: &'a str,
    /// The station name, or the RadioText.
    #[field(shared = keys::TEXT)]
    pub text: &'a str,
    /// The family of the sender: broadcast.
    #[field(shared = keys::FAMILY)]
    pub family: keys::Family,
}

/// A new station name was published: `source` is the PI, `text` the station name.
pub static STATION_NAMED: EventKind<StationEvent<'static>> =
    EventKind::new("fm.station", "FM station named");
/// The RadioText became complete: `source` is the PI, `text` the RadioText.
pub static TEXT_DONE: EventKind<StationEvent<'static>> =
    EventKind::new("fm.text", "FM RadioText complete");

/// The series of the MPX spectrum.
pub const MPX: SeriesDef = SeriesDef {
    key: "mpx",
    label: "MPX spectrum",
    x_unit: "Hz",
    y_unit: "dB",
};
/// The series of the RDS symbols.
pub const RDS_SYMBOLS: SeriesDef = SeriesDef {
    key: "rds_symbols",
    label: "RDS symbols",
    x_unit: "",
    y_unit: "",
};

/// The result fields of the FM sender. The measurements have no value while the stack does not transmit.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct FmTxFields {
    /// Whether the stack transmits now.
    #[field(label = "Transmitting")]
    pub transmitting: bool,
    /// The transmit frequency: the tuned frequency.
    #[field(label = "Transmit frequency", unit = "Hz")]
    pub tx_freq_hz: Option<Hertz>,
    /// The largest deviation of the multiplex over the last report.
    #[field(label = "Peak deviation sent", unit = "Hz")]
    pub tx_peak_hz: Option<Hertz>,
    /// The power of the multiplex over the last 60 s against that of a sine of ±19 kHz deviation (ITU-R BS.412): 0 dB is
    /// the limit.
    #[field(label = "Multiplex power", unit = "dB")]
    pub tx_mpx_power_db: Option<f64>,
    /// The RDS groups sent in this transmission.
    #[field(label = "RDS groups sent")]
    pub tx_groups: i64,
}

/// Starts the transmission.
pub const START: Command = Command::new("start", "Start sending")
    .help("Sends the FM broadcast signal without end: the audio input in stereo, with the pilot and RDS from the transmit settings, until stop. The setting tx_pi must be set.")
    .transmits(TxContent::File)
    .stopped_by("stop");
/// Ends the transmission.
pub const STOP: Command = Command::new("stop", "Stop sending")
    .help("Ends the transmission at once, with the fall of the carrier.");

/// What version 1.2 added: the sender of the FM stack.
const ADDED_1_2: &[&str] = &[
    "shape.FmTxFields",
    "shape.FmTxFields.transmitting",
    "shape.FmTxFields.tx_freq_hz",
    "shape.FmTxFields.tx_peak_hz",
    "shape.FmTxFields.tx_mpx_power_db",
    "shape.FmTxFields.tx_groups",
    "fm.fields.FmTxFields",
    "fm.setting.tx_pi",
    "fm.setting.tx_ps",
    "fm.setting.tx_rt",
    "fm.setting.tx_pty",
    "fm.setting.tx_tp",
    "fm.setting.tx_ta",
    "fm.setting.tx_music",
    "fm.setting.tx_af",
    "fm.setting.tx_ct",
    "fm.setting.tx_ct_offset",
    "fm.setting.tx_stereo",
    "fm.setting.tx_preemphasis",
    "fm.setting.tx_deviation",
    "fm.setting.tx_pilot",
    "fm.setting.tx_rds_level",
    "fm.setting.tx_limiter",
    "fm.command.start",
    "fm.command.stop",
];

/// The interface of the family: what the FM and the STL stacks declare.
pub fn family() -> &'static Family {
    static FAMILY: OnceLock<Family> = OnceLock::new();
    FAMILY.get_or_init(|| {
        Family::new("fm", 1, 3)
            .added(1, 3, &["fm.content", "stl.content"])
            .content(sdrkit_core::ContentKind::Broadcast, &["fm", "stl"])
            .audio(sdrkit_core::AudioKind::Programme, &["fm", "stl"])
            .stack(
                StackShape::new(crate::graph::ID)
                    .fields::<FmFields>()
                    .fields::<FmTxFields>()
                    .table(&AF)
                    .table(&LABEL_TAGS)
                    .table(&ODA)
                    .table(&EON)
                    .table(&EON_AF)
                    .table(&TMC)
                    .series(MPX)
                    .series(RDS_SYMBOLS)
                    .event(&STATION_NAMED)
                    .event(&TEXT_DONE)
                    .settings(crate::settings::FmSetting::SETTINGS)
                    .command(&crate::graph::CLEAR)
                    .command(&START)
                    .command(&STOP),
            )
            // Version 1.1 added the STL stack.
            .added_in(1, 1, |f| f.stack(crate::stl::typed::shape()))
            .added(1, 2, ADDED_1_2)
    })
}

/// The interface description of the FM stack.
///
/// # Panics
///
/// If the family does not hold the stack.
pub fn shape() -> &'static StackShape {
    family()
        .stack_shape(crate::graph::ID)
        .expect("the family holds the FM stack")
}
