//! The interface of the analogue video family: the shapes of the result sets, the table rows and the events, the
//! tables, the event types and what each stack declares. The teletext tables and the page event are those of
//! `sdrkit-proto-mpegts`.

use crate::atv::Modulation;
use crate::fpv::ScanPlan;
use crate::mac::frame::Variant;
use crate::results::{
    A2Mode, Aspect, CaSystem, DetectorUsed, FmPolarity, SoundCoding, SoundPlaying, State,
};
use crate::settings::{AtvSetting, FpvSetting, MacSetting};

/// The result fields of the senders of `atv` and `fpv`.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct TvTxFields<'a> {
    /// Whether the sender sends.
    #[field(label = "Sending")]
    pub tx_sending: bool,
    /// The system sent, "PAL B/G 625/50". No value while nothing is sent.
    #[field(label = "System sent")]
    pub tx_system: Option<&'a str>,
    /// The pictures of the host put on the air in the last transmission: one for each field that started with a new one.
    #[field(label = "Frames shown")]
    pub tx_frames: i64,
    /// The fields sent in the last transmission.
    #[field(label = "Fields sent")]
    pub tx_fields: i64,
}

sdrkit_core::choice_enum! {
    /// The layout of the pixels given to `frame`: rows top first, no padding.
    pub enum TvPixels {
        /// Red, green and blue, one byte each.
        Rgb8 = ("rgb8", "RGB, 8 bits each"),
        /// Blue, green, red and alpha, one byte each. Alpha is not used.
        Bgra = ("bgra", "BGRA, 8 bits each"),
        /// One byte of grey.
        Grey8 = ("grey8", "8-bit grey"),
    }
}

/// The arguments of `frame`.
#[derive(Arguments)]
pub struct TvFrameArgs {
    /// The pixels.
    #[arg(
        label = "Pixels",
        help = "The pixel bytes, rows top first with no padding, in the pixel format. None, with width and height 0: the test picture again."
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
    pub pixel_format: TvPixels,
}

/// Starts sending.
pub const TV_START: Command = Command::new("start", "Start sending")
    .help(
        "Sends the television signal continuously from the start of a field: the picture of the command frame, or the test \
         picture, and the host's audio. The transmit settings apply at each start.",
    )
    .transmits(TxContent::File)
    .stopped_by("stop");
/// Stops sending.
pub const TV_STOP: Command = Command::new("stop", "Stop sending")
    .help("Ends the transmission at the end of a field, with a fade of 5 ms.");
/// Sets the picture sent.
pub const TV_FRAME: CommandKind<TvFrameArgs> = CommandKind::new("frame", "Set the picture")
    .help(
        "Sends this picture from the start of the next field until the next one: a still picture, or video when the host \
         gives a picture each field. It is scaled to the picture of the system with its shape kept, black bars above and \
         below or at the sides.",
    );
use crate::systems::Colour;
use sdrkit_core::interface::{
    Arguments, CommandKind, EventKind, Family as Described, Fields, Hertz, Seconds, StackShape,
    Table,
};
use sdrkit_core::publish::field;
use sdrkit_core::publish::keys::{self, Family};
use sdrkit_core::publish::SeriesDef;
use sdrkit_core::settings::Argument;
use sdrkit_core::{Command, SettingSet, TxContent};
use sdrkit_proto_mpegts::publish as teletext;
use sdrkit_util_objects::commands::row_of;
use std::sync::{Arc, OnceLock};

/// The result fields of the `atv` receiver, which its status component writes.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct AtvFields<'a> {
    /// The state of the receiver.
    #[field(label = "State")]
    pub state: State,
    /// The system and its raster, "PAL B/G 625/50". Absent while the receiver knows none.
    #[field(label = "System")]
    pub system: Option<&'a str>,
    /// The colour system in use.
    #[field(label = "Colour system")]
    pub colour_system: Option<Colour>,
    /// The picture has colour now.
    #[field(label = "Colour")]
    pub colour: bool,
    /// The colour killer is on (the burst is too weak).
    #[field(label = "Colour killer")]
    pub colour_killer: bool,
    /// Lines a frame.
    #[field(label = "Lines")]
    pub lines: i64,
    /// Fields a second.
    #[field(label = "Field rate", unit = "Hz")]
    pub field_rate: Hertz,
    /// The line rate.
    #[field(label = "Line rate", unit = "Hz")]
    pub line_rate: Hertz,
    /// The line rate against its nominal value.
    #[field(label = "Line rate error", unit = "ppm")]
    pub line_error_ppm: f64,
    /// The detector in use.
    #[field(label = "Detector")]
    pub detector: DetectorUsed,
    /// The modulation the receiver runs.
    #[field(label = "Modulation")]
    pub modulation: Modulation,
    /// The vision carrier, from the centre of the input.
    #[field(label = "Vision carrier", unit = "Hz")]
    pub vision_carrier: Hertz,
    /// The sound carrier, from the centre of the input. Absent while none is found.
    #[field(label = "Sound carrier", unit = "Hz")]
    pub sound_carrier: Option<Hertz>,
    /// The sound carrier above the vision carrier (negative: below it).
    #[field(label = "Sound spacing", unit = "Hz")]
    pub sound_spacing: Hertz,
    /// The offset of the vision carrier.
    #[field(shared = keys::OFFSET_HZ)]
    pub offset_hz: Hertz,
    /// The level of the vision carrier.
    #[field(shared = keys::LEVEL_DBFS)]
    pub level_dbfs: f64,
    /// The peak vision carrier against the noise in 5 MHz.
    #[field(label = "Carrier to noise", unit = "dB")]
    pub carrier_to_noise_db: f64,
    /// The SNR of the picture.
    #[field(shared = keys::SNR_DB)]
    pub snr_db: f64,
    /// A sound carrier is there.
    #[field(label = "Sound carrier")]
    pub sound_present: bool,
    /// The peak deviation of the FM sound carrier.
    #[field(label = "Deviation", unit = "Hz")]
    pub sound_deviation: Hertz,
    /// The audio level.
    #[field(label = "Audio level", unit = "dBFS")]
    pub audio_level_dbfs: f64,
    /// The sound that plays.
    #[field(label = "Sound")]
    pub sound_system: SoundPlaying,
    /// The share of lines whose sync was found.
    #[field(label = "Sync found", unit = "%")]
    pub sync_found_pct: f64,
    /// Lines since the line loop started.
    #[field(label = "Lines decoded")]
    pub line_count: i64,
    /// Field syncs found.
    #[field(label = "Fields")]
    pub field_count: i64,
    /// Pictures sent.
    #[field(label = "Pictures")]
    pub pictures: i64,
    /// Snow pictures sent.
    #[field(label = "Snow pictures")]
    pub snow_pictures: i64,
    /// The field completed last (1 or 2).
    #[field(label = "Last field")]
    pub last_field: i64,
    /// Fields decoded completely.
    #[field(label = "Fields good")]
    pub fields_ok: i64,
    /// Fields damaged.
    #[field(label = "Fields damaged")]
    pub fields_bad: i64,
    /// The sync depth, a share of the peak carrier.
    #[field(label = "Sync depth", unit = "%")]
    pub sync_depth_pct: f64,
    /// The sync compression measured.
    #[field(label = "Sync compression", unit = "%")]
    pub sync_compression_pct: f64,
    /// The white peak (1 is the nominal white).
    #[field(label = "White peak")]
    pub white_peak: f64,
    /// The colour burst, a share of its nominal level.
    #[field(label = "Burst", unit = "%")]
    pub burst_pct: f64,
    /// The phase error of the burst.
    #[field(label = "Burst phase error", unit = "°")]
    pub burst_phase_error_deg: f64,
    /// The SECAM colour subcarrier level, a share of its nominal level.
    #[field(label = "Chroma level", unit = "%")]
    pub chroma_level_pct: f64,
    /// How SECAM was found.
    #[field(label = "SECAM identification")]
    pub secam_identification: Option<&'a str>,
    /// The polarity of FM video.
    #[field(label = "FM polarity")]
    pub fm_polarity: Option<FmPolarity>,
    /// The NBTV line loop is locked.
    #[field(label = "Line lock")]
    pub nbtv_line_locked: bool,
    /// The NBTV picture is locked.
    #[field(label = "Picture lock")]
    pub nbtv_picture_locked: bool,
    /// The NICAM frame alignment holds. Absent without a NICAM carrier.
    #[field(label = "NICAM")]
    pub nicam_locked: Option<bool>,
    /// The NICAM sound is valid and can play.
    #[field(label = "NICAM sound")]
    pub nicam_valid: Option<bool>,
    /// The NICAM sound mode.
    #[field(label = "NICAM mode")]
    pub nicam_mode: Option<crate::nicam::Mode>,
    /// NICAM sound words whose parity failed, since the lock.
    #[field(label = "NICAM parity errors")]
    pub nicam_parity_errors: Option<i64>,
    /// The share of NICAM sound words whose parity failed.
    #[field(label = "NICAM errors", unit = "%")]
    pub nicam_error_pct: Option<f64>,
    /// The A2 identification found.
    #[field(label = "A2")]
    pub a2_mode: Option<A2Mode>,
}

/// The result fields of the teletext decoder of the `atv` and `fpv` stacks. The counts have no value while no
/// teletext line arrives.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct TeletextFields<'a> {
    /// The header of the last teletext page.
    #[field(label = "Teletext")]
    pub teletext_header: Option<&'a str>,
    /// Teletext pages held.
    #[field(label = "Teletext pages")]
    pub teletext_pages: Option<i64>,
    /// Teletext packets received.
    #[field(label = "Teletext packets")]
    pub teletext_packets: Option<i64>,
    /// Teletext lines whose framing code or address failed.
    #[field(label = "Teletext errors")]
    pub teletext_errors: Option<i64>,
    /// Teletext characters whose parity failed.
    #[field(label = "Teletext parity errors")]
    pub teletext_parity_errors: Option<i64>,
    /// The teletext page shown.
    #[field(label = "Teletext page")]
    pub teletext_page: i64,
    /// The text of the teletext page shown, its rows joined with line feeds.
    #[field(label = "Page text")]
    pub page_text: Option<&'a str>,
}

/// A channel of the setting `channels`. The row key is the label.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct AtvChannelRow<'a> {
    /// The channel label.
    #[field(label = "Channel")]
    pub label: &'a str,
    /// The vision carrier of the channel.
    #[field(label = "Vision carrier", unit = "Hz")]
    pub vision: Hertz,
}

/// A change of the state of the `atv` receiver.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct AtvStateEvent<'a> {
    /// The new state.
    #[field(label = "State")]
    pub state: State,
    /// The system of the receiver.
    #[field(shared = keys::TEXT)]
    pub text: &'a str,
    /// What the event is about: the receiver.
    #[field(shared = keys::FAMILY)]
    pub family: Family,
}

/// The channels of the setting `channels`.
pub static ATV_CHANNELS: Table<AtvChannelRow<'static>> =
    Table::new("channels", "Channels", crate::channels::ATV_CHANNELS_CAP);
/// A change of the state of the `atv` receiver.
pub static ATV_STATE: EventKind<AtvStateEvent<'static>> = EventKind::new("atv.state", "State");

/// The series of the `atv` stack: the line scope, the field-blanking scope and the spectrum.
pub const ATV_SERIES: [SeriesDef; 3] = [
    SeriesDef {
        key: "line",
        label: "Line",
        x_unit: "µs",
        y_unit: "",
    },
    SeriesDef {
        key: "vbi",
        label: "Field blanking line",
        x_unit: "µs",
        y_unit: "",
    },
    SeriesDef {
        key: "spectrum",
        label: "Spectrum",
        x_unit: "Hz",
        y_unit: "dB",
    },
];

/// The arguments of `channel.tune` of the `atv` stack.
#[derive(Arguments)]
pub struct AtvChannelArgs<'a> {
    /// The row key of the channel in `channels`: its label.
    #[arg(shared = ATV_CHANNEL_ROW)]
    pub row: &'a str,
}

/// The row key of a channel of the setting `channels`.
pub const ATV_CHANNEL_ROW: Argument<field::Text> = row_of("channels");
/// Asks the host to tune the radio to a channel of the setting `channels`.
pub const ATV_CHANNEL_TUNE: CommandKind<AtvChannelArgs<'static>> =
    CommandKind::new("channel.tune", "Tune to a channel")
        .help("Asks the host to tune to a channel of the setting channels.");

/// The result fields of the `fpv` receiver, which its display writes.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct FpvFields<'a> {
    /// The state of the receiver.
    #[field(label = "State")]
    pub state: State,
    /// The names of the channels at the tuned frequency. Absent off every channel.
    #[field(label = "Channel")]
    pub channel: Option<&'a str>,
    /// The level of the channel.
    #[field(shared = keys::LEVEL_DBFS)]
    pub level_dbfs: f64,
    /// The level of the channel above the noise floor.
    #[field(label = "Over the noise", unit = "dB")]
    pub over_noise_db: f64,
    /// The noise floor.
    #[field(shared = keys::NOISE_DBFS)]
    pub noise_dbfs: f64,
    /// The offset of the transmitter from the channel.
    #[field(shared = keys::OFFSET_HZ)]
    pub offset_hz: Hertz,
    /// The system and its raster. Absent while the receiver knows none.
    #[field(label = "System")]
    pub system: Option<&'a str>,
    /// The colour system in use.
    #[field(label = "Colour system")]
    pub colour_system: Option<Colour>,
    /// Lines a frame.
    #[field(label = "Lines")]
    pub lines: i64,
    /// Fields a second.
    #[field(label = "Field rate", unit = "Hz")]
    pub field_rate: Hertz,
    /// The share of lines whose sync was found.
    #[field(label = "Sync found", unit = "%")]
    pub sync_found_pct: f64,
    /// Pictures sent.
    #[field(label = "Pictures")]
    pub pictures: i64,
    /// Fields decoded completely.
    #[field(label = "Fields good")]
    pub fields_ok: i64,
    /// Fields damaged.
    #[field(label = "Fields damaged")]
    pub fields_bad: i64,
    /// The polarity of the FM video.
    #[field(label = "Polarity")]
    pub polarity: FmPolarity,
    /// The SNR of the picture.
    #[field(shared = keys::SNR_DB)]
    pub snr_db: f64,
    /// A sound subcarrier is there.
    #[field(label = "Sound carrier")]
    pub sound_present: bool,
    /// The sound subcarrier, from the vision carrier. Absent while none is found.
    #[field(label = "Sound subcarrier", unit = "Hz")]
    pub sound_subcarrier: Option<Hertz>,
    /// The 6.0 MHz subcarrier against the noise beside it.
    #[field(label = "6.0 MHz", unit = "dB")]
    pub subcarrier_60_db: f64,
    /// The 6.5 MHz subcarrier against the noise beside it.
    #[field(label = "6.5 MHz", unit = "dB")]
    pub subcarrier_65_db: f64,
}

/// A channel of the FPV meter. The row key is the channel name ("R5").
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct FpvChannelRow<'a> {
    /// The name of the channel ("R5").
    #[field(label = "Name")]
    pub name: &'a str,
    /// The band letter.
    #[field(label = "Band")]
    pub band: &'a str,
    /// The number of the channel in its band.
    #[field(label = "Number")]
    pub number: i64,
    /// The frequency of the channel.
    #[field(label = "Frequency", unit = "Hz")]
    pub freq: Hertz,
    /// The level of the channel.
    #[field(shared = keys::LEVEL_DBFS)]
    pub level_dbfs: f64,
    /// The level of the channel above the noise floor.
    #[field(label = "Over the noise", unit = "dB")]
    pub over_noise_db: f64,
    /// A transmitter is on the channel.
    #[field(label = "Present")]
    pub present: bool,
    /// The channel is inside the capture.
    #[field(label = "Inside the capture")]
    pub inside: bool,
    /// The stream time at which a transmitter was last heard on the channel.
    #[field(label = "Last heard", unit = "s")]
    pub heard: Option<Seconds>,
}

/// A change of the state of the `fpv` receiver.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct FpvStateEvent<'a> {
    /// The new state.
    #[field(label = "State")]
    pub state: State,
    /// The names of the channels at the tuned frequency.
    #[field(shared = keys::TEXT)]
    pub text: &'a str,
    /// What the event is about: the receiver.
    #[field(shared = keys::FAMILY)]
    pub family: Family,
}

/// The channels of the FPV meter.
pub static FPV_CHANNELS: Table<FpvChannelRow<'static>> = Table::new("channels", "Channels", 64);
/// A change of the state of the `fpv` receiver.
pub static FPV_STATE: EventKind<FpvStateEvent<'static>> = EventKind::new("fpv.state", "State");

/// The series of the `fpv` stack: the spectrum of the capture and the history of the level.
pub const FPV_SERIES: [SeriesDef; 2] = [
    SeriesDef {
        key: "spectrum",
        label: "Spectrum",
        x_unit: "MHz",
        y_unit: "dB",
    },
    SeriesDef {
        key: "history",
        label: "Level history",
        x_unit: "s",
        y_unit: "dB",
    },
];

/// The arguments of `channel.tune` of the `fpv` stack.
#[derive(Arguments)]
pub struct FpvChannelArgs<'a> {
    /// The row key of the channel ("R5").
    #[arg(shared = FPV_CHANNEL_ROW)]
    pub row: &'a str,
}

/// The row key of a channel of the FPV meter.
pub const FPV_CHANNEL_ROW: Argument<field::Text> = row_of("channels");
/// Asks the host to tune to an FPV channel.
pub const FPV_CHANNEL_TUNE: CommandKind<FpvChannelArgs<'static>> =
    CommandKind::new("channel.tune", "Tune to a channel")
        .help("Asks the host to tune to an FPV channel.");

/// The arguments of `scan` of the `fpv` stack.
#[derive(Arguments)]
pub struct FpvScanArgs {
    /// The plan that the scan visits.
    #[arg(label = "Plan", help = "The FPV plan whose channels the scan visits.")]
    pub plan: ScanPlan,
}

/// Asks the host to visit every channel of a plan.
pub const FPV_SCAN: CommandKind<FpvScanArgs> = CommandKind::new("scan", "Scan")
    .help("Asks the host to visit every channel of a plan for 0.7 s each.");

/// The result fields of the MAC receiver.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct MacFields<'a> {
    /// The state of the receiver.
    #[field(label = "State")]
    pub state: State,
    /// The MAC variant. Absent while the receiver searches.
    #[field(label = "System")]
    pub system: Option<Variant>,
    /// The signal-to-noise ratio of the data burst.
    #[field(shared = keys::SNR_DB)]
    pub snr_db: f64,
    /// The SNR of the picture.
    #[field(label = "Video SNR", unit = "dB")]
    pub video_snr_db: f64,
    /// The carrier offset.
    #[field(shared = keys::OFFSET_HZ)]
    pub offset_hz: Hertz,
    /// The level of the signal, dBFS.
    #[field(shared = keys::LEVEL_DBFS)]
    pub level_dbfs: f64,
    /// The FM deviation, Hz a volt of the baseband.
    #[field(label = "Deviation", unit = "Hz/V")]
    pub deviation_hz_per_v: f64,
    /// The spectrum is inverted (and corrected).
    #[field(label = "Spectrum inverted")]
    pub spectrum_inverted: bool,
    /// The line rate.
    #[field(label = "Line rate", unit = "Hz")]
    pub line_rate: Hertz,
    /// The line rate against its nominal value.
    #[field(label = "Line rate error", unit = "ppm")]
    pub line_error_ppm: f64,
    /// The share of lines whose sync word was found.
    #[field(label = "Line sync", unit = "%")]
    pub line_sync_pct: f64,
    /// Frame syncs found.
    #[field(label = "Frame syncs")]
    pub frame_syncs: i64,
    /// Frames decoded.
    #[field(label = "Frames")]
    pub frames: i64,
    /// The bit error rate of the data burst.
    #[field(shared = keys::BER)]
    pub ber: f64,
    /// The channel name. Absent until the service identification names it.
    #[field(label = "Channel")]
    pub channel: Option<&'a str>,
    /// The network name.
    #[field(label = "Network")]
    pub network: Option<&'a str>,
    /// The country.
    #[field(label = "Country")]
    pub country: Option<&'a str>,
    /// The channel identifier.
    #[field(label = "Channel id")]
    pub channel_id: Option<i64>,
    /// The picture's aspect ratio.
    #[field(label = "Aspect")]
    pub aspect: Option<Aspect>,
    /// The picture or sound is scrambled.
    #[field(label = "Scrambled")]
    pub scrambled: bool,
    /// The access conditions, in words.
    #[field(label = "Access")]
    pub access: Option<&'a str>,
    /// The conditional access system, when the channel names one.
    #[field(label = "Access system")]
    pub ca_system: Option<CaSystem>,
    /// Sound words whose protection failed.
    #[field(label = "Sound errors")]
    pub sound_errors: i64,
    /// ECM packets received.
    #[field(label = "ECM packets")]
    pub ecm_packets: i64,
    /// Packet headers decoded.
    #[field(label = "Packets")]
    pub packet_headers: i64,
    /// Packet headers corrected.
    #[field(label = "Packets corrected")]
    pub packets_corrected: i64,
    /// Dummy packets.
    #[field(label = "Dummy packets")]
    pub packets_dummy: i64,
    /// Service identification groups whose CRC passed.
    #[field(label = "SI groups good")]
    pub si_groups_ok: i64,
    /// Service identification groups whose CRC failed.
    #[field(label = "SI groups bad")]
    pub si_groups_bad: i64,
    /// The packet address of the sound.
    #[field(label = "Sound address")]
    pub sound_address: Option<i64>,
    /// The sound is there and plays.
    #[field(label = "Sound carrier")]
    pub sound_present: bool,
    /// Sound packets received.
    #[field(label = "Sound packets")]
    pub sound_packets: i64,
    /// How the sound is coded.
    #[field(label = "Sound coding")]
    pub sound_mode: SoundCoding,
    /// The audio level.
    #[field(label = "Audio level", unit = "dBFS")]
    pub audio_level_dbfs: f64,
    /// Sound blocks whose words failed their parity.
    #[field(label = "Sound blocks bad")]
    pub blocks_bad: i64,
    /// Pictures sent.
    #[field(label = "Pictures")]
    pub pictures: i64,
}

/// A change of the state of the MAC receiver.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct MacStateEvent<'a> {
    /// The new state.
    #[field(label = "State")]
    pub state: State,
    /// The summary of the receiver.
    #[field(shared = keys::TEXT)]
    pub text: &'a str,
    /// What the event is about: the receiver.
    #[field(shared = keys::FAMILY)]
    pub family: Family,
}

/// A change of the state of the MAC receiver.
pub static MAC_STATE: EventKind<MacStateEvent<'static>> = EventKind::new("mac.state", "State");

/// The key of the eye series of the MAC receiver.
pub const EYE_SERIES: &str = "eye";

/// `shape` with the series `series`.
fn with_series(mut shape: StackShape, series: &[SeriesDef]) -> StackShape {
    for s in series {
        shape = shape.series(*s);
    }
    shape
}

/// The interface of the family.
pub fn family() -> &'static Described {
    static FAMILY: OnceLock<Described> = OnceLock::new();
    FAMILY.get_or_init(|| {
        // The channel list is data of no single frequency. Every other result of `atv` is of the present signal.
        let atv = StackShape::new("atv")
            .fields::<AtvFields>()
            .fields::<TeletextFields>()
            .table(&ATV_CHANNELS)
            .table(&teletext::TELETEXT_PAGE_TABLE)
            .table(&teletext::TELETEXT_SUBPAGE_TABLE);
        let atv = with_series(atv, &ATV_SERIES)
            .event(&ATV_STATE)
            .event(teletext::teletext_page())
            .settings(AtvSetting::SETTINGS)
            .command(&ATV_CHANNEL_TUNE)
            .fields::<TvTxFields>()
            .command(&TV_START)
            .command(&TV_STOP)
            .command(&TV_FRAME)
            .command(&teletext::TELETEXT_SELECT)
            .kept(&["channels"]);
        // The channel table of `fpv` is a scan list: the levels that the meter found on each channel.
        let fpv = StackShape::new("fpv")
            .fields::<FpvFields>()
            .fields::<TeletextFields>()
            .table(&FPV_CHANNELS)
            .table(&teletext::TELETEXT_PAGE_TABLE)
            .table(&teletext::TELETEXT_SUBPAGE_TABLE);
        let fpv = with_series(fpv, &FPV_SERIES)
            .event(&FPV_STATE)
            .event(teletext::teletext_page())
            .settings(FpvSetting::SETTINGS)
            .command(&crate::fpv::CLEAR)
            .command(&FPV_CHANNEL_TUNE)
            .command(&FPV_SCAN)
            .command(&teletext::TELETEXT_SELECT)
            .fields::<TvTxFields>()
            .command(&TV_START)
            .command(&TV_STOP)
            .command(&TV_FRAME)
            .kept(&["channels"]);
        // The MAC display holds the measurements and service names of the present signal: nothing is kept over a
        // change of frequency.
        // The key and the label of the aspect ratios are numbers, so the options get the names of their variants.
        Described::new("analogtv", 1, 1)
            .option_name("aspect", "4:3", "normal")
            .option_name("aspect", "16:9", "wide")
            .uses(teletext::interface())
            .stack(atv)
            .stack(
                StackShape::new("mac")
                    .fields::<MacFields>()
                    .series(SeriesDef {
                        key: EYE_SERIES,
                        label: "Eye",
                        x_unit: "",
                        y_unit: "",
                    })
                    .event(&MAC_STATE)
                    .settings(MacSetting::SETTINGS),
            )
            .stack(fpv)
            .added(1, 1, ADDED_1_1)
    })
}

/// What version 1.1 added: the senders of `atv` and `fpv`.
const ADDED_1_1: &[&str] = &[
    "shape.TvTxFields",
    "shape.TvTxFields.tx_sending",
    "shape.TvTxFields.tx_system",
    "shape.TvTxFields.tx_frames",
    "shape.TvTxFields.tx_fields",
    "atv.fields.TvTxFields",
    "atv.setting.tx_system",
    "atv.setting.tx_colour",
    "atv.setting.tx_picture",
    "atv.setting.tx_sound",
    "atv.setting.tx_compression",
    "atv.setting.tx_group_delay",
    "atv.setting.tx_nyquist",
    "atv.setting.tx_setup",
    "atv.setting.tx_secam_id",
    "atv.command.start",
    "atv.command.stop",
    "atv.command.frame",
    "fpv.fields.TvTxFields",
    "fpv.setting.tx_system",
    "fpv.setting.tx_picture",
    "fpv.setting.tx_sound",
    "fpv.setting.tx_subcarrier",
    "fpv.setting.tx_deviation_mhz",
    "fpv.setting.tx_sync_high",
    "fpv.setting.tx_ac_ms",
    "fpv.command.start",
    "fpv.command.stop",
    "fpv.command.frame",
];

/// The description of the stack `id`.
///
/// # Panics
///
/// If the family does not describe the stack.
pub fn shape(id: &str) -> &'static StackShape {
    family()
        .stack_shape(id)
        .unwrap_or_else(|| panic!("the analogtv family does not describe the stack {id}"))
}
