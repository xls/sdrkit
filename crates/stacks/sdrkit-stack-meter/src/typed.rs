//! The interface of the meter family: the shapes of the result sets, the rows of the logs and the events, the tables, the
//! event types and what each stack declares.

use sdrkit_core::interface::{EventKind, Family, Fields, Hertz, Seconds, StackShape, Table};
use sdrkit_core::publish::keys;
use sdrkit_core::SettingSet;
use sdrkit_graph::DecoderFields;
use std::sync::OnceLock;

// ---- am_monitor ----

sdrkit_core::choice_enum! {
    /// A change of the carrier of an AM station.
    pub enum AmCarrierChange {
        /// The carrier was seen for the first time.
        Present = ("present", "Present"),
        /// The carrier went away.
        Lost = ("lost", "Lost"),
        /// The carrier came back.
        Returned = ("returned", "Returned"),
    }
}

/// The result fields of `am_monitor`, measured once a second. The measurements have no value while the carrier test fails.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct AmMonitorFields<'a> {
    /// The headline: the offset, the modulation, the peaks and the audio bandwidth, or "no carrier".
    #[field(shared = keys::TEXT)]
    pub text: &'a str,
    /// The carrier offset: a tuning or station error.
    #[field(label = "Carrier offset", unit = "Hz")]
    pub carrier_offset_hz: Option<f64>,
    /// The carrier level, relative to full scale.
    #[field(label = "Carrier level", unit = "dB")]
    pub carrier_db: Option<f64>,
    /// The modulation depth: the depth of a sine of the same RMS.
    #[field(label = "Modulation (RMS, as a sine)", unit = "%")]
    pub depth_pct: Option<f64>,
    /// The positive peak against the mean carrier.
    #[field(label = "Positive peak", unit = "%")]
    pub peak_pos_pct: Option<f64>,
    /// The negative peak against the mean carrier (at most 100 %).
    #[field(label = "Negative peak", unit = "%")]
    pub peak_neg_pct: Option<f64>,
    /// The asymmetry: (positive - negative) / (positive + negative).
    #[field(label = "Asymmetry", unit = "%")]
    pub asymmetry_pct: Option<f64>,
    /// The mains hum below the carrier; no value when no hum line is seen.
    #[field(label = "Mains hum below the carrier", unit = "dB")]
    pub hum_db: Option<f64>,
    /// The frequency of the mains hum line.
    #[field(label = "Mains hum frequency", unit = "Hz")]
    pub hum_hz: Option<f64>,
    /// The audio bandwidth: 99 % of the audio power lies below it.
    #[field(label = "Audio bandwidth", unit = "kHz")]
    pub bandwidth_khz: Option<f64>,
    /// The share of the last second at zero carrier.
    #[field(label = "Share of the last second at zero carrier", unit = "%")]
    pub over_mod_pct: Option<f64>,
    /// The seconds that were over-modulated.
    #[field(label = "Seconds over-modulated")]
    pub over_mod_s: i64,
    /// The carrier is present (the carrier test and the level rule).
    #[field(label = "Carrier present")]
    pub carrier: bool,
    /// What the monitor measures.
    #[field(label = "Note")]
    pub note: &'a str,
}

/// A row of the log of `am_monitor`. The row key is the sequence number of the row.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct AmMonitorLogRow<'a> {
    /// The signal time of the row.
    #[field(label = "Signal time", unit = "s")]
    pub signal_time_s: f64,
    /// The kind: `Carrier` or `Over-modulation`.
    #[field(label = "Kind")]
    pub kind: &'a str,
    /// What changed: `present`, `lost`, `returned`, or the share of the second at zero carrier.
    #[field(label = "Identity")]
    pub identity: &'a str,
    /// The text of the row.
    #[field(shared = keys::TEXT)]
    pub text: &'a str,
}

/// The carrier of an AM station was seen, was lost or came back.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct AmMonitorCarrierEvent<'a> {
    /// The text of the event.
    #[field(shared = keys::TEXT)]
    pub text: &'a str,
    /// What changed.
    #[field(label = "Change")]
    pub change: AmCarrierChange,
    /// The carrier offset of the second.
    #[field(label = "Carrier offset", unit = "Hz")]
    pub carrier_offset_hz: f64,
    /// The carrier level of the second.
    #[field(label = "Carrier level", unit = "dB")]
    pub carrier_db: f64,
}

/// A second of an AM station was over-modulated.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct AmMonitorOverModulationEvent<'a> {
    /// The text of the event.
    #[field(shared = keys::TEXT)]
    pub text: &'a str,
    /// The share of the second at zero carrier.
    #[field(label = "Share at zero carrier", unit = "%")]
    pub over_mod_pct: f64,
    /// The positive peak of the second.
    #[field(label = "Positive peak", unit = "%")]
    pub peak_pos_pct: f64,
}

/// The log of `am_monitor`, newest first.
pub static AM_MONITOR_LOG: Table<AmMonitorLogRow<'static>> =
    Table::new("log", "Log", crate::am_monitor::ROWS_KEPT).label(&AmMonitorLogRow::IDENTITY);
/// The carrier was seen, was lost or came back.
pub static AM_MONITOR_CARRIER: EventKind<AmMonitorCarrierEvent<'static>> =
    EventKind::new("am_monitor.carrier", "Carrier");
/// A second was over-modulated.
pub static AM_MONITOR_OVER_MODULATION: EventKind<AmMonitorOverModulationEvent<'static>> =
    EventKind::new("am_monitor.over_modulation", "Over-modulation");

// ---- fm_monitor ----

sdrkit_core::choice_enum! {
    /// The pilot of an FM station against the usual range: 8 to 10 % of the deviation limit (ITU-R BS.450).
    pub enum FmPilotRange {
        /// No pilot.
        Absent = ("absent", "Absent"),
        /// In the usual range.
        InRange = ("in_range", "In the usual range"),
        /// Outside the usual range.
        Outside = ("outside", "Outside the usual range"),
    }
}

sdrkit_core::choice_enum! {
    /// A change of the pilot.
    pub enum FmPilotChange {
        /// The pilot is present: at the first measurement or when it comes.
        Present = ("present", "Present"),
        /// No pilot at the first measurement.
        Absent = ("absent", "Absent"),
        /// The pilot went away.
        Lost = ("lost", "Lost"),
    }
}

sdrkit_core::choice_enum! {
    /// A change of RDS.
    pub enum FmRdsChange {
        /// RDS came.
        Present = ("present", "Present"),
        /// RDS went away.
        Lost = ("lost", "Lost"),
    }
}

/// The result fields of `fm_monitor`, measured once a second from the FM-demodulated multiplex (MPX).
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct FmMonitorFields<'a> {
    /// The headline: stereo or mono, the peak, the pilot and RDS levels.
    #[field(shared = keys::TEXT)]
    pub text: &'a str,
    /// The peak deviation of the last second: the largest third-largest excursion of its 5 ms blocks.
    #[field(label = "Peak deviation", unit = "kHz")]
    pub peak_deviation_khz: Option<f64>,
    /// The RMS deviation.
    #[field(label = "RMS deviation", unit = "kHz")]
    pub rms_deviation_khz: Option<f64>,
    /// The MPX power of the last second (0 dBr is a sine of 19 kHz deviation).
    #[field(label = "MPX power, 1 s", unit = "dBr")]
    pub mpx_power_dbr: Option<f64>,
    /// The MPX power over the last 60 s in steps of 1 s, as ITU-R BS.412 integrates it; no value before 60 s.
    #[field(label = "MPX power, 60 s", unit = "dBr")]
    pub mpx_power_60s_dbr: Option<f64>,
    /// The carrier offset.
    #[field(label = "Carrier offset", unit = "kHz")]
    pub carrier_offset_khz: Option<f64>,
    /// The audio (L+R), peak equivalent.
    #[field(label = "Audio (L+R)", unit = "kHz")]
    pub audio_khz: Option<f64>,
    /// The 19 kHz pilot, peak equivalent.
    #[field(label = "Pilot", unit = "kHz")]
    pub pilot_khz: Option<f64>,
    /// The pilot as a share of the deviation limit.
    #[field(label = "Pilot share of the limit", unit = "%")]
    pub pilot_pct: Option<f64>,
    /// The pilot against its usual range.
    #[field(label = "Pilot range")]
    pub pilot: FmPilotRange,
    /// The stereo (L-R) band, peak equivalent.
    #[field(label = "Stereo (L-R) band", unit = "kHz")]
    pub stereo_band_khz: Option<f64>,
    /// Stereo: a pilot and an L-R band over 0.5 kHz.
    #[field(label = "Stereo")]
    pub stereo: bool,
    /// The RDS injection, peak equivalent.
    #[field(label = "RDS injection", unit = "kHz")]
    pub rds_khz: Option<f64>,
    /// RDS is present.
    #[field(label = "RDS")]
    pub rds: bool,
    /// The 67 kHz subcarrier over the floor; no value when absent.
    #[field(label = "67 kHz subcarrier", unit = "dB")]
    pub sca67_db: Option<f64>,
    /// The 92 kHz subcarrier over the floor; no value when absent or below a working rate of 250 ksps.
    #[field(label = "92 kHz subcarrier", unit = "dB")]
    pub sca92_db: Option<f64>,
    /// The seconds whose peak was over the threshold.
    #[field(label = "Seconds over the threshold")]
    pub over_deviation_s: i64,
    /// The input level.
    #[field(label = "Level", unit = "dB")]
    pub level_db: Option<f64>,
    /// What the monitor measures.
    #[field(label = "Note")]
    pub note: &'a str,
}

/// A row of the log of `fm_monitor`. The row key is the sequence number of the row.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct FmMonitorLogRow<'a> {
    /// The signal time of the row.
    #[field(label = "Signal time", unit = "s")]
    pub signal_time_s: f64,
    /// The kind: `Pilot`, `RDS`, `SCA` or `Over-deviation`.
    #[field(label = "Kind")]
    pub kind: &'a str,
    /// What changed, or the peak of an over-deviation.
    #[field(label = "Identity")]
    pub identity: &'a str,
    /// The text of the row.
    #[field(shared = keys::TEXT)]
    pub text: &'a str,
}

/// The pilot of an FM station was present or absent at the first measurement, came or went.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct FmMonitorPilotEvent<'a> {
    /// The text of the event.
    #[field(shared = keys::TEXT)]
    pub text: &'a str,
    /// What changed.
    #[field(label = "Change")]
    pub change: FmPilotChange,
    /// The pilot of the second, peak equivalent.
    #[field(label = "Pilot", unit = "kHz")]
    pub pilot_khz: f64,
}

/// RDS came or went.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct FmMonitorRdsEvent<'a> {
    /// The text of the event.
    #[field(shared = keys::TEXT)]
    pub text: &'a str,
    /// What changed.
    #[field(label = "Change")]
    pub change: FmRdsChange,
    /// The RDS injection of the second, peak equivalent.
    #[field(label = "RDS injection", unit = "kHz")]
    pub rds_khz: f64,
}

/// A subcarrier at 67 or 92 kHz came or went.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct FmMonitorScaEvent<'a> {
    /// The text of the event.
    #[field(shared = keys::TEXT)]
    pub text: &'a str,
    /// The subcarrier, 67 or 92.
    #[field(label = "Subcarrier", unit = "kHz")]
    pub subcarrier_khz: f64,
    /// The subcarrier came (true) or went.
    #[field(label = "On")]
    pub on: bool,
    /// Its level over the floor in that second.
    #[field(label = "Level over the floor", unit = "dB")]
    pub level_db: Option<f64>,
}

/// A second whose peak deviation was over the threshold.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct FmMonitorOverDeviationEvent<'a> {
    /// The text of the event.
    #[field(shared = keys::TEXT)]
    pub text: &'a str,
    /// The peak deviation of the second.
    #[field(label = "Peak deviation", unit = "kHz")]
    pub peak_deviation_khz: f64,
    /// The MPX power of the second.
    #[field(label = "MPX power, 1 s", unit = "dBr")]
    pub mpx_power_dbr: f64,
}

/// The log of `fm_monitor`, newest first.
pub static FM_MONITOR_LOG: Table<FmMonitorLogRow<'static>> =
    Table::new("log", "Log", crate::fm_monitor::ROWS_KEPT).label(&FmMonitorLogRow::IDENTITY);
/// The pilot was present or absent at first, came or went.
pub static FM_MONITOR_PILOT: EventKind<FmMonitorPilotEvent<'static>> =
    EventKind::new("fm_monitor.pilot", "Pilot");
/// RDS came or went.
pub static FM_MONITOR_RDS: EventKind<FmMonitorRdsEvent<'static>> =
    EventKind::new("fm_monitor.rds", "RDS");
/// A subcarrier came or went.
pub static FM_MONITOR_SCA: EventKind<FmMonitorScaEvent<'static>> =
    EventKind::new("fm_monitor.sca", "Subcarrier");
/// A second over the over-deviation threshold.
pub static FM_MONITOR_OVER_DEVIATION: EventKind<FmMonitorOverDeviationEvent<'static>> =
    EventKind::new("fm_monitor.over_deviation", "Over-deviation");

// ---- the spectrum watchers ----

sdrkit_core::choice_enum! {
    /// The state of a spectrum watcher.
    pub enum WatchState {
        /// No energy and no reading in the last 2 s.
        Nothing = ("nothing", "Nothing"),
        /// Energy or a bad reading in the last 2 s, no good reading in the last 8 s.
        Energy = ("energy", "Energy"),
        /// A good reading in the last 8 s.
        Measuring = ("measuring", "Measuring"),
    }
}

/// The status of a spectrum watcher: the headline, the state, the counts of readings and the input level.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct WatchFields<'a> {
    /// The headline.
    #[field(shared = keys::TEXT)]
    pub text: &'a str,
    /// The state.
    #[field(label = "State")]
    pub state: WatchState,
    /// The good readings: what the stack counts as a measurement.
    #[field(label = "Good readings")]
    pub good: i64,
    /// The bad readings: what the stack counts as a fault of the watched band.
    #[field(label = "Bad readings")]
    pub bad: i64,
    /// The input level of the last report, relative to full scale.
    #[field(shared = keys::LEVEL_DBFS)]
    pub level_dbfs: Option<f64>,
    /// What the stack measures.
    #[field(label = "Note")]
    pub note: &'a str,
}

// ---- occupancy ----

/// The result fields of `occupancy`.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct OccupancyFields {
    /// The channels of the grid.
    #[field(label = "Channels in the grid")]
    pub channels: i64,
    /// The width of a channel: whole bins.
    #[field(label = "Channel width", unit = "kHz")]
    pub channel_khz: f64,
    /// The span that the channels cover.
    #[field(label = "Span covered", unit = "MHz")]
    pub span_mhz: f64,
    /// The noise floor of a channel, relative, per bin; no value before the first decision.
    #[field(label = "Noise floor (relative, per bin)", unit = "dB")]
    pub floor_db: Option<f64>,
    /// The channels busy now.
    #[field(label = "Channels busy now")]
    pub busy_now: i64,
    /// The busy share of the channel-time since the end of the warm-up; no value before it.
    #[field(label = "Busy share of channel-time since the start", unit = "%")]
    pub band_occupancy_pct: Option<f64>,
    /// The time measured after the warm-up.
    #[field(label = "Time measured", unit = "s")]
    pub elapsed_s: Seconds,
}

/// A row of the list of busy channels: a channel that has been busy, the busiest first.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct OccupancyChannelRow {
    /// The centre of the channel (an offset from the centre when the tuned frequency is not known).
    #[field(label = "Frequency", unit = "Hz")]
    pub frequency: Hertz,
    /// The busy time over the time measured.
    #[field(label = "Duty cycle", unit = "%")]
    pub duty_pct: f64,
    /// The transmissions that ended.
    #[field(label = "Transmissions")]
    pub transmissions: i64,
    /// The longest transmission.
    #[field(label = "Longest", unit = "s")]
    pub longest_s: Seconds,
    /// The channel is busy now.
    #[field(label = "Busy")]
    pub busy: bool,
}

/// A row of the activity log: one transmission of at least the shortest listed length.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct OccupancyActivityRow {
    /// The end of the transmission, signal time.
    #[field(label = "Time", unit = "s")]
    pub time_s: Seconds,
    /// The centre of the channel.
    #[field(label = "Frequency", unit = "Hz")]
    pub frequency: Hertz,
    /// The length of the transmission.
    #[field(label = "Length", unit = "s")]
    pub length_s: Seconds,
    /// The highest channel power over the floor.
    #[field(label = "Peak over the floor", unit = "dB")]
    pub peak_over_floor_db: f64,
    /// The highest channel power, relative, per bin.
    #[field(label = "Level", unit = "dB")]
    pub level_db: f64,
}

/// A transmission of a channel ended.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct OccupancyActivityEvent {
    /// The end of the transmission, signal time.
    #[field(label = "Time", unit = "s")]
    pub time_s: Seconds,
    /// The centre of the channel.
    #[field(label = "Frequency", unit = "Hz")]
    pub frequency: Hertz,
    /// The length of the transmission.
    #[field(label = "Length", unit = "s")]
    pub length_s: Seconds,
    /// The highest channel power over the floor.
    #[field(label = "Peak over the floor", unit = "dB")]
    pub peak_over_floor_db: f64,
    /// The highest channel power, relative, per bin.
    #[field(label = "Level", unit = "dB")]
    pub level_db: f64,
}

/// The channels that have been busy, the busiest first.
pub static OCCUPANCY_CHANNELS: Table<OccupancyChannelRow> =
    Table::new("channels", "Busy channels", crate::occupancy::ROWS_KEPT);
/// The transmissions, newest first.
pub static OCCUPANCY_ACTIVITY: Table<OccupancyActivityRow> =
    Table::new("activity", "Activity", crate::occupancy::ROWS_KEPT);
/// A transmission ended.
pub static OCCUPANCY_ACTIVITY_EVENT: EventKind<OccupancyActivityEvent> =
    EventKind::new("occupancy.activity", "Activity");

// ---- noise_floor ----

sdrkit_core::choice_enum! {
    /// The kind of a row of the noise floor log.
    pub enum NoiseFloorChange {
        /// A sub-band rose over its baseline and held.
        Rise = ("rise", "Noise rise"),
        /// A raised sub-band fell back.
        Back = ("back", "Noise back"),
        /// Most sub-bands rose at once: a change of the whole band or of the radio's gain.
        WholeBand = ("whole_band", "Whole band"),
    }
}

/// The result fields of `noise_floor`.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct NoiseFloorFields {
    /// The floor of the band: the upper median of the sub-bands now, dB re full scale per bin.
    #[field(label = "Noise floor", unit = "dB")]
    pub floor_db: Option<f64>,
    /// The floor now against the oldest kept value (the baseline window ago, or the start).
    #[field(label = "Change over the baseline window", unit = "dB")]
    pub floor_change_db: Option<f64>,
    /// The sub-bands raised now.
    #[field(label = "Sub-bands raised")]
    pub raised: i64,
    /// The seconds measured.
    #[field(label = "Time measured", unit = "s")]
    pub elapsed_s: Seconds,
}

/// A row of the list of sub-bands.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct NoiseFloorSubBandRow {
    /// The centre of the sub-band.
    #[field(label = "Frequency", unit = "Hz")]
    pub frequency: Hertz,
    /// The floor of the last second, dB re full scale per bin.
    #[field(label = "Now", unit = "dB")]
    pub now_db: Option<f64>,
    /// The baseline: the 20th percentile of the baseline window.
    #[field(label = "Baseline", unit = "dB")]
    pub baseline_db: Option<f64>,
    /// The 10th percentile of the kept history.
    #[field(label = "10th percentile", unit = "dB")]
    pub p10_db: Option<f64>,
    /// The median of the kept history.
    #[field(label = "Median", unit = "dB")]
    pub p50_db: Option<f64>,
    /// The 90th percentile of the kept history.
    #[field(label = "90th percentile", unit = "dB")]
    pub p90_db: Option<f64>,
    /// The minutes of history kept.
    #[field(label = "History", unit = "min")]
    pub history_min: f64,
    /// The sub-band is raised.
    #[field(label = "Raised")]
    pub raised: bool,
}

/// A row of the noise floor log.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct NoiseFloorEventRow {
    /// The signal time of the row.
    #[field(label = "Time", unit = "s")]
    pub time_s: Seconds,
    /// The kind.
    #[field(label = "Kind")]
    pub kind: NoiseFloorChange,
    /// The centre of the sub-band; no value for the whole band.
    #[field(label = "Frequency", unit = "Hz")]
    pub frequency: Option<Hertz>,
    /// The sub-band, from 1; no value for the whole band.
    #[field(label = "Sub-band")]
    pub sub_band: Option<i64>,
    /// The rise over the baseline (the largest, for a row of the fall back); the change of the floor for the whole band.
    #[field(label = "Rise", unit = "dB")]
    pub rise_db: f64,
    /// The baseline of the sub-band.
    #[field(label = "Baseline", unit = "dB")]
    pub baseline_db: Option<f64>,
    /// How long the rise lasted (rows of the fall back).
    #[field(label = "Duration", unit = "s")]
    pub duration_s: Option<Seconds>,
    /// The floor of the sub-band, or of the band, that second.
    #[field(label = "Level", unit = "dB")]
    pub level_db: Option<f64>,
}

/// A row of the noise floor log was written.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct NoiseFloorEvent {
    /// The signal time of the row.
    #[field(label = "Time", unit = "s")]
    pub time_s: Seconds,
    /// The kind.
    #[field(label = "Kind")]
    pub kind: NoiseFloorChange,
    /// The centre of the sub-band; no value for the whole band.
    #[field(label = "Frequency", unit = "Hz")]
    pub frequency: Option<Hertz>,
    /// The sub-band, from 1; no value for the whole band.
    #[field(label = "Sub-band")]
    pub sub_band: Option<i64>,
    /// The rise over the baseline (the largest, for a row of the fall back); the change of the floor for the whole band.
    #[field(label = "Rise", unit = "dB")]
    pub rise_db: f64,
    /// The baseline of the sub-band.
    #[field(label = "Baseline", unit = "dB")]
    pub baseline_db: Option<f64>,
    /// How long the rise lasted (rows of the fall back).
    #[field(label = "Duration", unit = "s")]
    pub duration_s: Option<Seconds>,
    /// The floor of the sub-band, or of the band, that second.
    #[field(label = "Level", unit = "dB")]
    pub level_db: Option<f64>,
}

/// The sub-bands, from the lowest.
pub static NOISE_FLOOR_SUB_BANDS: Table<NoiseFloorSubBandRow> =
    Table::new("sub_bands", "Sub-bands", 64);
/// The rises, the falls back and the changes of the whole band, newest first.
pub static NOISE_FLOOR_EVENTS: Table<NoiseFloorEventRow> = Table::new(
    "noise_events",
    "Noise events",
    crate::noise_floor::ROWS_KEPT,
);
/// A row of the log was written.
pub static NOISE_FLOOR_EVENT: EventKind<NoiseFloorEvent> =
    EventKind::new("noise_floor.event", "Noise event");

// ---- burst_finder ----

sdrkit_core::choice_enum! {
    /// The bandwidth class of a burst.
    pub enum BurstClass {
        /// At most 3.5 bins wide.
        Narrow = ("narrow", "Narrow"),
        /// Wider.
        Wide = ("wide", "Wide"),
    }
}

/// The result fields of `burst_finder`.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct BurstFinderFields {
    /// The bursts found.
    #[field(label = "Bursts")]
    pub bursts: i64,
    /// The time listened.
    #[field(label = "Time listened", unit = "s")]
    pub elapsed_s: Seconds,
    /// Bursts per second over the time listened.
    #[field(label = "Bursts per second", unit = "1/s")]
    pub bursts_per_s: f64,
    /// The sources found repeating now.
    #[field(label = "Repeating sources")]
    pub repeating: i64,
    /// The width of a bin: the frequency resolution.
    #[field(label = "Bin width", unit = "kHz")]
    pub bin_khz: f64,
    /// The frame step: the time resolution.
    #[field(label = "Frame step", unit = "us")]
    pub frame_us: f64,
}

/// A row of the burst log.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct BurstFinderBurstRow {
    /// The start of the burst, signal time.
    #[field(label = "Start", unit = "s")]
    pub time_s: Seconds,
    /// The median centre of the burst.
    #[field(label = "Frequency", unit = "Hz")]
    pub frequency: Hertz,
    /// The length of the burst.
    #[field(label = "Length", unit = "ms")]
    pub length_ms: f64,
    /// The bandwidth class.
    #[field(label = "Class")]
    pub class: BurstClass,
    /// The median width, at least one bin.
    #[field(label = "Width", unit = "kHz")]
    pub width_khz: f64,
    /// The mean peak over the median of the frames.
    #[field(label = "Level over the median", unit = "dB")]
    pub level_db: f64,
}

/// A burst was found.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct BurstFoundEvent {
    /// The start of the burst, signal time.
    #[field(label = "Start", unit = "s")]
    pub time_s: Seconds,
    /// The median centre of the burst.
    #[field(label = "Frequency", unit = "Hz")]
    pub frequency: Hertz,
    /// The length of the burst.
    #[field(label = "Length", unit = "ms")]
    pub length_ms: f64,
    /// The bandwidth class.
    #[field(label = "Class")]
    pub class: BurstClass,
    /// The median width, at least one bin.
    #[field(label = "Width", unit = "kHz")]
    pub width_khz: f64,
    /// The mean peak over the median of the frames.
    #[field(label = "Level over the median", unit = "dB")]
    pub level_db: f64,
}

/// A row of the list of repeating sources.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct BurstFinderRepeatingRow {
    /// The frequency of the source: its first burst.
    #[field(label = "Frequency", unit = "Hz")]
    pub frequency: Hertz,
    /// The median interval between its bursts.
    #[field(label = "Period", unit = "s")]
    pub period_s: Seconds,
    /// Its bursts in the history.
    #[field(label = "Bursts")]
    pub bursts: i64,
    /// The RMS deviation of the agreeing intervals from the period, over the period.
    #[field(label = "Spread", unit = "%")]
    pub spread_pct: f64,
}

/// A source was found repeating.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct BurstRepeatingEvent {
    /// The frequency of the source: its first burst.
    #[field(label = "Frequency", unit = "Hz")]
    pub frequency: Hertz,
    /// The median interval between its bursts.
    #[field(label = "Period", unit = "s")]
    pub period_s: Seconds,
    /// Its bursts in the history.
    #[field(label = "Bursts")]
    pub bursts: i64,
    /// The RMS deviation of the agreeing intervals from the period, over the period.
    #[field(label = "Spread", unit = "%")]
    pub spread_pct: f64,
}

/// The bursts, newest first.
pub static BURST_FINDER_BURSTS: Table<BurstFinderBurstRow> =
    Table::new("bursts", "Bursts", crate::burst_finder::ROWS_KEPT);
/// The sources found repeating now.
pub static BURST_FINDER_REPEATING: Table<BurstFinderRepeatingRow> =
    Table::new("repeating", "Repeating sources", 64);
/// A burst was found.
pub static BURST_FOUND: EventKind<BurstFoundEvent> = EventKind::new("burst.found", "Burst");
/// A source was found repeating.
pub static BURST_REPEATING: EventKind<BurstRepeatingEvent> =
    EventKind::new("burst.repeating", "Repeating source");

// ---- hopper ----

/// The result fields of `hopper`. The readings of the hopper have no value until an evaluation finds one.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct HopperFields {
    /// The hop rate: one over the median start-to-start time.
    #[field(label = "Hop rate", unit = "1/s")]
    pub hop_rate: Option<f64>,
    /// The median start-to-start time.
    #[field(label = "Hop period", unit = "ms")]
    pub hop_period_ms: Option<f64>,
    /// The median dwell.
    #[field(label = "Dwell", unit = "ms")]
    pub dwell_ms: Option<f64>,
    /// The median silence between dwells.
    #[field(label = "Guard", unit = "ms")]
    pub guard_ms: Option<f64>,
    /// The channels found.
    #[field(label = "Channels")]
    pub channels: Option<i64>,
    /// The median distance of neighbouring channels.
    #[field(label = "Channel spacing", unit = "kHz")]
    pub spacing_khz: Option<f64>,
    /// At least 90 % of the channels lie on a grid of the spacing.
    #[field(label = "Regular grid")]
    pub grid: Option<bool>,
    /// The highest minus the lowest channel plus one spacing.
    #[field(label = "Span", unit = "kHz")]
    pub span_khz: Option<f64>,
    /// The lowest channel.
    #[field(label = "Lowest channel", unit = "Hz")]
    pub lowest: Option<Hertz>,
    /// The highest channel.
    #[field(label = "Highest channel", unit = "Hz")]
    pub highest: Option<Hertz>,
    /// The mean dwell level over the frame median.
    #[field(label = "Level over the median", unit = "dB")]
    pub level_db: Option<f64>,
    /// The dwells since the start.
    #[field(label = "Dwells")]
    pub dwells: i64,
    /// The dwells kept for the evaluation.
    #[field(label = "Dwells in the window")]
    pub dwells_window: i64,
    /// The width of a bin.
    #[field(label = "Bin width", unit = "kHz")]
    pub bin_khz: f64,
    /// The frame step.
    #[field(label = "Frame step", unit = "us")]
    pub frame_us: f64,
}

/// A row of the hopper log: an evaluation that found a hopper or a change of it.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct HopperRow {
    /// The signal time of the evaluation.
    #[field(label = "Time", unit = "s")]
    pub time_s: Seconds,
    /// The hop rate.
    #[field(label = "Hop rate", unit = "1/s")]
    pub hop_rate: f64,
    /// The channels.
    #[field(label = "Channels")]
    pub channels: i64,
    /// The channel spacing.
    #[field(label = "Channel spacing", unit = "kHz")]
    pub spacing_khz: f64,
    /// The span.
    #[field(label = "Span", unit = "kHz")]
    pub span_khz: f64,
    /// The lowest channel.
    #[field(label = "Lowest channel", unit = "Hz")]
    pub lowest: Hertz,
    /// The highest channel.
    #[field(label = "Highest channel", unit = "Hz")]
    pub highest: Hertz,
    /// The median dwell.
    #[field(label = "Dwell", unit = "ms")]
    pub dwell_ms: f64,
    /// The median guard.
    #[field(label = "Guard", unit = "ms")]
    pub guard_ms: f64,
    /// The mean dwell level over the frame median.
    #[field(label = "Level over the median", unit = "dB")]
    pub level_db: f64,
}

/// A hopper was found, or its rate or its channel count changed.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct HopperChangedEvent {
    /// The signal time of the evaluation.
    #[field(label = "Time", unit = "s")]
    pub time_s: Seconds,
    /// The hop rate.
    #[field(label = "Hop rate", unit = "1/s")]
    pub hop_rate: f64,
    /// The channels.
    #[field(label = "Channels")]
    pub channels: i64,
    /// The channel spacing.
    #[field(label = "Channel spacing", unit = "kHz")]
    pub spacing_khz: f64,
    /// The span.
    #[field(label = "Span", unit = "kHz")]
    pub span_khz: f64,
    /// The lowest channel.
    #[field(label = "Lowest channel", unit = "Hz")]
    pub lowest: Hertz,
    /// The highest channel.
    #[field(label = "Highest channel", unit = "Hz")]
    pub highest: Hertz,
    /// The median dwell.
    #[field(label = "Dwell", unit = "ms")]
    pub dwell_ms: f64,
    /// The median guard.
    #[field(label = "Guard", unit = "ms")]
    pub guard_ms: f64,
    /// The mean dwell level over the frame median.
    #[field(label = "Level over the median", unit = "dB")]
    pub level_db: f64,
}

/// The hopper readings, newest first.
pub static HOPPER_HOPPERS: Table<HopperRow> =
    Table::new("hoppers", "Hoppers", crate::hopper::ROWS_KEPT);
/// A hopper was found or changed.
pub static HOPPER_CHANGED: EventKind<HopperChangedEvent> =
    EventKind::new("hopper.changed", "Hopper");

// ---- birdie ----

sdrkit_core::choice_enum! {
    /// The class of a persistent carrier.
    pub enum BirdieClass {
        /// Narrowness at least the bare share: an unmodulated carrier.
        Bare = ("bare", "Bare carrier"),
        /// Lower narrowness.
        Modulated = ("modulated", "Modulated or wide"),
    }
}

sdrkit_core::choice_enum! {
    /// The kind of a row of the carrier log.
    pub enum BirdieChange {
        /// A carrier was listed for the first time.
        New = ("new", "New"),
        /// A carrier that was gone is listed again.
        Returned = ("returned", "Returned"),
        /// A listed carrier was not found in 3 evaluations in a row.
        Gone = ("gone", "Gone"),
    }
}

/// The result fields of `birdie`.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct BirdieFields {
    /// The persistent carriers listed now.
    #[field(label = "Persistent carriers")]
    pub carriers: i64,
    /// The snapshots taken.
    #[field(label = "Snapshots")]
    pub snapshots: i64,
    /// The time of a snapshot.
    #[field(label = "Snapshot length", unit = "s")]
    pub snapshot_s: Seconds,
    /// The width of a bin.
    #[field(label = "Bin width", unit = "Hz")]
    pub bin_hz: f64,
}

/// A row of the list of persistent carriers, the strongest first.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct BirdieCarrierRow {
    /// The smoothed frequency.
    #[field(label = "Frequency", unit = "Hz")]
    pub frequency: Hertz,
    /// The mean level over the noise per bin.
    #[field(label = "Level over the noise", unit = "dB")]
    pub level_db: f64,
    /// The standard deviation of the level.
    #[field(label = "Level deviation", unit = "dB")]
    pub level_sd_db: f64,
    /// The class.
    #[field(label = "Class")]
    pub class: BirdieClass,
    /// The least-squares drift of the frequency.
    #[field(label = "Drift", unit = "Hz/min")]
    pub drift_hz_min: f64,
    /// The RMS of the frequency about the drift line.
    #[field(label = "Scatter", unit = "Hz")]
    pub scatter_hz: f64,
    /// The snapshots of the presence window in which it was seen.
    #[field(label = "Present")]
    pub present: i64,
    /// The snapshots of the presence window.
    #[field(label = "Of")]
    pub of: i64,
}

/// A row of the carrier log.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct BirdieEventRow {
    /// The signal time of the row.
    #[field(label = "Time", unit = "s")]
    pub time_s: Seconds,
    /// The kind.
    #[field(label = "Kind")]
    pub kind: BirdieChange,
    /// The frequency.
    #[field(label = "Frequency", unit = "Hz")]
    pub frequency: Hertz,
    /// The mean level over the noise.
    #[field(label = "Level over the noise", unit = "dB")]
    pub level_db: f64,
    /// The class.
    #[field(label = "Class")]
    pub class: BirdieClass,
    /// The drift.
    #[field(label = "Drift", unit = "Hz/min")]
    pub drift_hz_min: f64,
    /// The scatter.
    #[field(label = "Scatter", unit = "Hz")]
    pub scatter_hz: f64,
}

/// A persistent carrier was listed, came back or went.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct BirdieEvent {
    /// The signal time of the row.
    #[field(label = "Time", unit = "s")]
    pub time_s: Seconds,
    /// The kind.
    #[field(label = "Kind")]
    pub kind: BirdieChange,
    /// The frequency.
    #[field(label = "Frequency", unit = "Hz")]
    pub frequency: Hertz,
    /// The mean level over the noise.
    #[field(label = "Level over the noise", unit = "dB")]
    pub level_db: f64,
    /// The class.
    #[field(label = "Class")]
    pub class: BirdieClass,
    /// The drift.
    #[field(label = "Drift", unit = "Hz/min")]
    pub drift_hz_min: f64,
    /// The scatter.
    #[field(label = "Scatter", unit = "Hz")]
    pub scatter_hz: f64,
}

/// The persistent carriers listed now, the strongest first.
pub static BIRDIE_CARRIERS: Table<BirdieCarrierRow> =
    Table::new("carriers", "Persistent carriers", 200).sorted(
        &BirdieCarrierRow::LEVEL_DB,
        sdrkit_core::SortOrder::Descending,
    );
/// The carrier log, newest first.
pub static BIRDIE_CARRIER_EVENTS: Table<BirdieEventRow> =
    Table::new("carrier_events", "Carrier events", crate::birdie::ROWS_KEPT);
/// A carrier was listed, came back or went.
pub static BIRDIE_EVENT: EventKind<BirdieEvent> = EventKind::new("birdie.event", "Carrier event");

// ---- obw ----

/// The result fields of `obw`. The readings have no value while no signal is found.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct ObwFields {
    /// The band that holds the power share, half the rest below and half above it.
    #[field(label = "Occupied bandwidth (99 %)", unit = "kHz")]
    pub obw_khz: Option<f64>,
    /// The lower edge of the occupied band.
    #[field(label = "Lower edge", unit = "Hz")]
    pub obw_low: Option<Hertz>,
    /// The upper edge of the occupied band.
    #[field(label = "Upper edge", unit = "Hz")]
    pub obw_high: Option<Hertz>,
    /// The width at 3 dB under the peak of the smoothed spectrum.
    #[field(label = "Bandwidth 3 dB under the peak", unit = "kHz")]
    pub bw3_khz: Option<f64>,
    /// The width at 6 dB under the peak.
    #[field(label = "Bandwidth 6 dB under the peak", unit = "kHz")]
    pub bw6_khz: Option<f64>,
    /// The width at 20 dB under the peak.
    #[field(label = "Bandwidth 20 dB under the peak", unit = "kHz")]
    pub bw20_khz: Option<f64>,
    /// The width at 26 dB under the peak.
    #[field(label = "Bandwidth 26 dB under the peak", unit = "kHz")]
    pub bw26_khz: Option<f64>,
    /// The middle of the occupied band from the tuned frequency.
    #[field(label = "Centre against the tuned frequency", unit = "Hz")]
    pub centre_offset_hz: Option<f64>,
    /// The power between the edges over the floor, dB re full scale.
    #[field(label = "Signal power", unit = "dB")]
    pub power_db: Option<f64>,
    /// The signal power over the floor in the same bins.
    #[field(label = "Over the noise in the same bandwidth", unit = "dB")]
    pub snr_db: Option<f64>,
    /// The power of the band one adjacent offset below, under the signal; no value outside the spectrum.
    #[field(label = "Adjacent power under the signal, below", unit = "dB")]
    pub adjacent_low_db: Option<f64>,
    /// The same above.
    #[field(label = "Adjacent power under the signal, above", unit = "dB")]
    pub adjacent_high_db: Option<f64>,
    /// The power of the band two adjacent offsets below, under the signal.
    #[field(label = "Alternate power under the signal, below", unit = "dB")]
    pub alternate_low_db: Option<f64>,
    /// The same above.
    #[field(label = "Alternate power under the signal, above", unit = "dB")]
    pub alternate_high_db: Option<f64>,
    /// A signal was found in the last evaluation.
    #[field(label = "Signal found")]
    pub signal: bool,
}

/// A row of the bandwidth log.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct ObwBandwidthRow {
    /// The signal time of the evaluation.
    #[field(label = "Time", unit = "s")]
    pub time_s: Seconds,
    /// The band that holds the power share, half the rest below and half above it.
    #[field(label = "Occupied bandwidth (99 %)", unit = "kHz")]
    pub obw_khz: f64,
    /// The lower edge of the occupied band.
    #[field(label = "Lower edge", unit = "Hz")]
    pub obw_low: Hertz,
    /// The upper edge of the occupied band.
    #[field(label = "Upper edge", unit = "Hz")]
    pub obw_high: Hertz,
    /// The width at 3 dB under the peak of the smoothed spectrum.
    #[field(label = "Bandwidth 3 dB under the peak", unit = "kHz")]
    pub bw3_khz: f64,
    /// The width at 6 dB under the peak.
    #[field(label = "Bandwidth 6 dB under the peak", unit = "kHz")]
    pub bw6_khz: f64,
    /// The width at 20 dB under the peak.
    #[field(label = "Bandwidth 20 dB under the peak", unit = "kHz")]
    pub bw20_khz: f64,
    /// The width at 26 dB under the peak.
    #[field(label = "Bandwidth 26 dB under the peak", unit = "kHz")]
    pub bw26_khz: f64,
    /// The middle of the occupied band from the tuned frequency.
    #[field(label = "Centre against the tuned frequency", unit = "Hz")]
    pub centre_offset_hz: f64,
    /// The power between the edges over the floor, dB re full scale.
    #[field(label = "Signal power", unit = "dB")]
    pub power_db: f64,
    /// The signal power over the floor in the same bins.
    #[field(label = "Over the noise in the same bandwidth", unit = "dB")]
    pub snr_db: f64,
    /// The power of the band one adjacent offset below, under the signal; no value outside the spectrum.
    #[field(label = "Adjacent power under the signal, below", unit = "dB")]
    pub adjacent_low_db: Option<f64>,
    /// The same above.
    #[field(label = "Adjacent power under the signal, above", unit = "dB")]
    pub adjacent_high_db: Option<f64>,
    /// The power of the band two adjacent offsets below, under the signal.
    #[field(label = "Alternate power under the signal, below", unit = "dB")]
    pub alternate_low_db: Option<f64>,
    /// The same above.
    #[field(label = "Alternate power under the signal, above", unit = "dB")]
    pub alternate_high_db: Option<f64>,
}

/// An evaluation was logged.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct ObwReadingEvent {
    /// The signal time of the evaluation.
    #[field(label = "Time", unit = "s")]
    pub time_s: Seconds,
    /// The band that holds the power share, half the rest below and half above it.
    #[field(label = "Occupied bandwidth (99 %)", unit = "kHz")]
    pub obw_khz: f64,
    /// The lower edge of the occupied band.
    #[field(label = "Lower edge", unit = "Hz")]
    pub obw_low: Hertz,
    /// The upper edge of the occupied band.
    #[field(label = "Upper edge", unit = "Hz")]
    pub obw_high: Hertz,
    /// The width at 3 dB under the peak of the smoothed spectrum.
    #[field(label = "Bandwidth 3 dB under the peak", unit = "kHz")]
    pub bw3_khz: f64,
    /// The width at 6 dB under the peak.
    #[field(label = "Bandwidth 6 dB under the peak", unit = "kHz")]
    pub bw6_khz: f64,
    /// The width at 20 dB under the peak.
    #[field(label = "Bandwidth 20 dB under the peak", unit = "kHz")]
    pub bw20_khz: f64,
    /// The width at 26 dB under the peak.
    #[field(label = "Bandwidth 26 dB under the peak", unit = "kHz")]
    pub bw26_khz: f64,
    /// The middle of the occupied band from the tuned frequency.
    #[field(label = "Centre against the tuned frequency", unit = "Hz")]
    pub centre_offset_hz: f64,
    /// The power between the edges over the floor, dB re full scale.
    #[field(label = "Signal power", unit = "dB")]
    pub power_db: f64,
    /// The signal power over the floor in the same bins.
    #[field(label = "Over the noise in the same bandwidth", unit = "dB")]
    pub snr_db: f64,
    /// The power of the band one adjacent offset below, under the signal; no value outside the spectrum.
    #[field(label = "Adjacent power under the signal, below", unit = "dB")]
    pub adjacent_low_db: Option<f64>,
    /// The same above.
    #[field(label = "Adjacent power under the signal, above", unit = "dB")]
    pub adjacent_high_db: Option<f64>,
    /// The power of the band two adjacent offsets below, under the signal.
    #[field(label = "Alternate power under the signal, below", unit = "dB")]
    pub alternate_low_db: Option<f64>,
    /// The same above.
    #[field(label = "Alternate power under the signal, above", unit = "dB")]
    pub alternate_high_db: Option<f64>,
}

/// The logged evaluations, newest first.
pub static OBW_BANDWIDTHS: Table<ObwBandwidthRow> =
    Table::new("bandwidths", "Bandwidths", crate::obw::ROWS_KEPT);
/// An evaluation was logged.
pub static OBW_READING: EventKind<ObwReadingEvent> =
    EventKind::new("obw.reading", "Bandwidth reading");

// ---- ptt_log ----

sdrkit_core::choice_enum! {
    /// The kind of a row of the transmission log.
    pub enum PttKind {
        /// A transmission of at least the kerchunk length.
        Transmission = ("transmission", "Transmission"),
        /// A transmission shorter than the kerchunk length.
        Kerchunk = ("kerchunk", "Kerchunk"),
        /// A transmission longer than the stuck-key length, while it lasts.
        StuckKey = ("stuck_key", "Stuck key"),
    }
}

/// The result fields of `ptt_log`.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct PttLogFields {
    /// The channel is keyed now.
    #[field(label = "Keyed")]
    pub keyed: bool,
    /// The length of the present transmission; no value while idle.
    #[field(label = "Keyed for", unit = "s")]
    pub keyed_s: Option<Seconds>,
    /// The floor of the channel, relative; no value before the first reading.
    #[field(label = "Floor", unit = "dB")]
    pub floor_db: Option<f64>,
    /// The transmissions that ended.
    #[field(label = "Transmissions")]
    pub transmissions: i64,
    /// Those shorter than the kerchunk length.
    #[field(label = "Kerchunks")]
    pub kerchunks: i64,
    /// The keyed share of the time since the end of the warm-up, the present transmission included.
    #[field(label = "Duty cycle", unit = "%")]
    pub duty_pct: Option<f64>,
    /// The keyed share of the last 10 minutes (or of the time measured, when shorter).
    #[field(label = "Duty cycle, 10 min", unit = "%")]
    pub duty_10min_pct: Option<f64>,
    /// The mean length of the transmissions that ended.
    #[field(label = "Mean length", unit = "s")]
    pub mean_s: Option<Seconds>,
    /// The median length.
    #[field(label = "Median length", unit = "s")]
    pub median_s: Option<Seconds>,
    /// The longest.
    #[field(label = "Longest", unit = "s")]
    pub longest_s: Option<Seconds>,
    /// Transmissions per hour of the time measured.
    #[field(label = "Transmissions per hour", unit = "1/h")]
    pub per_hour: Option<f64>,
    /// The time measured since the end of the warm-up.
    #[field(label = "Time measured", unit = "s")]
    pub elapsed_s: Seconds,
}

/// A row of the transmission log.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct PttLogTransmissionRow {
    /// The number of the transmission, from 1.
    #[field(label = "Number")]
    pub number: i64,
    /// The kind.
    #[field(label = "Kind")]
    pub kind: PttKind,
    /// The start, signal time.
    #[field(label = "Start", unit = "s")]
    pub start_s: Seconds,
    /// The length (so far, for a stuck key).
    #[field(label = "Length", unit = "s")]
    pub length_s: Seconds,
    /// The mean level over the floor of its keyed blocks.
    #[field(label = "Over the floor", unit = "dB")]
    pub over_floor_db: f64,
    /// The time from the end of the transmission before; no value for the first.
    #[field(label = "Gap", unit = "s")]
    pub gap_s: Option<Seconds>,
    /// The mean level of its keyed blocks, relative to full scale.
    #[field(label = "Level", unit = "dB")]
    pub level_db: f64,
}

/// A transmission ended, or a stuck key was found.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct PttTransmissionEvent {
    /// The number of the transmission, from 1.
    #[field(label = "Number")]
    pub number: i64,
    /// The kind.
    #[field(label = "Kind")]
    pub kind: PttKind,
    /// The start, signal time.
    #[field(label = "Start", unit = "s")]
    pub start_s: Seconds,
    /// The length (so far, for a stuck key).
    #[field(label = "Length", unit = "s")]
    pub length_s: Seconds,
    /// The mean level over the floor of its keyed blocks.
    #[field(label = "Over the floor", unit = "dB")]
    pub over_floor_db: f64,
    /// The time from the end of the transmission before; no value for the first.
    #[field(label = "Gap", unit = "s")]
    pub gap_s: Option<Seconds>,
    /// The mean level of its keyed blocks, relative to full scale.
    #[field(label = "Level", unit = "dB")]
    pub level_db: f64,
}

/// The transmissions, newest first.
pub static PTT_LOG_TRANSMISSIONS: Table<PttLogTransmissionRow> =
    Table::new("transmissions", "Transmissions", crate::ptt_log::ROWS_KEPT);
/// A transmission ended, or a stuck key was found.
pub static PTT_TRANSMISSION: EventKind<PttTransmissionEvent> =
    EventKind::new("ptt.transmission", "Transmission");

// ---- noise_locator ----

sdrkit_core::choice_enum! {
    /// The class of a repetition rate of impulses.
    pub enum NoiseLocatorClass {
        /// Twice a mains frequency: an arc at each half-cycle.
        TwiceMains = ("twice_mains", "Twice the mains"),
        /// A mains frequency: a fault on one half-cycle.
        Mains = ("mains", "The mains"),
        /// Another steady rate, such as a switching supply or a clock.
        Steady = ("steady", "Other steady rate"),
        /// No repetition.
        NoRate = ("none", "None"),
    }
}

sdrkit_core::choice_enum! {
    /// The kind of a row of the pattern log.
    pub enum NoiseLocatorChange {
        /// A rate was found in two reports in a row.
        Found = ("found", "Found"),
        /// The rate was not found in 3 reports in a row.
        Gone = ("gone", "Gone"),
    }
}

/// The result fields of `noise_locator`.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct NoiseLocatorFields {
    /// The impulses per second of the impulse window.
    #[field(label = "Impulse rate", unit = "1/s")]
    pub impulse_rate: Option<f64>,
    /// The median impulse level over the noise of the analysis band.
    #[field(label = "Impulse level over the noise", unit = "dB")]
    pub impulse_level_db: Option<f64>,
    /// The 75th minus the 25th percentile of the impulse levels.
    #[field(label = "Impulse level spread", unit = "dB")]
    pub impulse_spread_db: Option<f64>,
    /// The repetition rate of the present pattern; no value without one.
    #[field(label = "Repetition rate", unit = "Hz")]
    pub repetition_hz: Option<f64>,
    /// The class of the present pattern.
    #[field(label = "Class")]
    pub class: NoiseLocatorClass,
    /// The mains frequency of the class, when it names one.
    #[field(label = "Mains frequency found", unit = "Hz")]
    pub mains_found_hz: Option<f64>,
    /// The share of the impulses on the rate.
    #[field(label = "Strength", unit = "%")]
    pub strength_pct: Option<f64>,
    /// The impulses since the start.
    #[field(label = "Impulses")]
    pub impulses: i64,
}

/// A row of the pattern log.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct NoiseLocatorPatternRow {
    /// The signal time of the report.
    #[field(label = "Time", unit = "s")]
    pub time_s: Seconds,
    /// Found or gone.
    #[field(label = "Kind")]
    pub kind: NoiseLocatorChange,
    /// The repetition rate.
    #[field(label = "Repetition rate", unit = "Hz")]
    pub repetition_hz: f64,
    /// The class of the rate.
    #[field(label = "Class")]
    pub class: NoiseLocatorClass,
    /// The impulses per second of the window.
    #[field(label = "Impulse rate", unit = "1/s")]
    pub impulse_rate: f64,
    /// The median impulse level over the noise of the analysis band.
    #[field(label = "Level over the noise", unit = "dB")]
    pub level_db: f64,
    /// The share of the impulses on the rate.
    #[field(label = "Strength", unit = "%")]
    pub strength_pct: f64,
}

/// A pattern was found or went.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct NoiseLocatorPatternEvent {
    /// The signal time of the report.
    #[field(label = "Time", unit = "s")]
    pub time_s: Seconds,
    /// Found or gone.
    #[field(label = "Kind")]
    pub kind: NoiseLocatorChange,
    /// The repetition rate.
    #[field(label = "Repetition rate", unit = "Hz")]
    pub repetition_hz: f64,
    /// The class of the rate.
    #[field(label = "Class")]
    pub class: NoiseLocatorClass,
    /// The impulses per second of the window.
    #[field(label = "Impulse rate", unit = "1/s")]
    pub impulse_rate: f64,
    /// The median impulse level over the noise of the analysis band.
    #[field(label = "Level over the noise", unit = "dB")]
    pub level_db: f64,
    /// The share of the impulses on the rate.
    #[field(label = "Strength", unit = "%")]
    pub strength_pct: f64,
}

/// The patterns found and gone, newest first.
pub static NOISE_LOCATOR_PATTERNS: Table<NoiseLocatorPatternRow> =
    Table::new("patterns", "Patterns", crate::noise_locator::ROWS_KEPT);
/// A pattern was found or went.
pub static NOISE_LOCATOR_PATTERN: EventKind<NoiseLocatorPatternEvent> =
    EventKind::new("noise_locator.pattern", "Pattern");

// ---- freq_accuracy ----

sdrkit_core::choice_enum! {
    /// The kind of a row of the `freq_accuracy` log.
    pub enum FreqAccuracyKind {
        /// A frequency reading, the first and then at most one per row interval.
        Frequency = ("frequency", "Frequency"),
        /// The carrier frequency stepped.
        Step = ("step", "Step"),
        /// The carrier was lost.
        Lost = ("lost", "Lost"),
        /// The carrier returned.
        Returned = ("returned", "Returned"),
    }
}

/// The result fields of `freq_accuracy`. The readings of the carrier have no value while the carrier is absent.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct FreqAccuracyFields {
    /// The mean offset of the last 10 readings from the tuned frequency.
    #[field(label = "Carrier offset (mean of the last 10)", unit = "Hz")]
    pub offset_hz: Option<Hertz>,
    /// The sample standard deviation of the last 10 readings.
    #[field(label = "Scatter of the last 10", unit = "Hz")]
    pub scatter_hz: Option<Hertz>,
    /// The offset over the tuned frequency; no value without the tuned frequency.
    #[field(label = "Error", unit = "ppm")]
    pub error_ppm: Option<f64>,
    /// The carrier to noise density of the last window.
    #[field(label = "Carrier to noise density", unit = "dB-Hz")]
    pub cn0_dbhz: Option<f64>,
    /// The least-squares slope of the last 120 readings; from 8 readings.
    #[field(label = "Drift", unit = "Hz/s")]
    pub drift_hz_s: Option<f64>,
    /// The drift over the tuned frequency.
    #[field(label = "Drift", unit = "ppm/h")]
    pub drift_ppm_h: Option<f64>,
    /// The readings kept.
    #[field(label = "Readings")]
    pub readings: i64,
    /// The length of one reading window.
    #[field(label = "Reading window", unit = "s")]
    pub window_s: Option<Seconds>,
    /// The carrier was present in the last window.
    #[field(label = "Carrier present")]
    pub carrier: Option<bool>,
}

/// A row of the Allan deviation table: one averaging time.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct FreqAccuracyAllanRow {
    /// The averaging time.
    #[field(label = "Averaging time", unit = "s")]
    pub tau_s: Seconds,
    /// The fractional Allan deviation; no value without the tuned frequency.
    #[field(label = "Allan deviation", unit = "")]
    pub adev: Option<f64>,
    /// The Allan deviation of the offset.
    #[field(label = "Allan deviation", unit = "Hz")]
    pub adev_hz: Hertz,
    /// The groups it is from.
    #[field(label = "Groups")]
    pub groups: i64,
}

/// A row of the `freq_accuracy` log.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct FreqAccuracyEventRow {
    /// The signal time.
    #[field(label = "Time", unit = "s")]
    pub time_s: Seconds,
    /// The kind.
    #[field(label = "Kind")]
    pub kind: FreqAccuracyKind,
    /// The offset of the reading (for a loss, the last reading).
    #[field(label = "Offset", unit = "Hz")]
    pub offset_hz: Hertz,
    /// The size of a step.
    #[field(label = "Step", unit = "Hz")]
    pub step_hz: Option<Hertz>,
    /// The carrier to noise density of the reading.
    #[field(label = "Carrier to noise density", unit = "dB-Hz")]
    pub cn0_dbhz: Option<f64>,
}

/// A row of the `freq_accuracy` log was written.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct FreqAccuracyEvent {
    /// The signal time.
    #[field(label = "Time", unit = "s")]
    pub time_s: Seconds,
    /// The kind.
    #[field(label = "Kind")]
    pub kind: FreqAccuracyKind,
    /// The offset of the reading (for a loss, the last reading).
    #[field(label = "Offset", unit = "Hz")]
    pub offset_hz: Hertz,
    /// The size of a step.
    #[field(label = "Step", unit = "Hz")]
    pub step_hz: Option<Hertz>,
    /// The carrier to noise density of the reading.
    #[field(label = "Carrier to noise density", unit = "dB-Hz")]
    pub cn0_dbhz: Option<f64>,
}

/// The Allan deviation at the averaging times 1, 2, 4, 8 and 16 reading windows.
pub static FREQ_ACCURACY_ALLAN: Table<FreqAccuracyAllanRow> =
    Table::new("allan", "Allan deviation", 8);
/// The frequency, step and carrier rows, newest first.
pub static FREQ_ACCURACY_EVENTS: Table<FreqAccuracyEventRow> =
    Table::new("events", "Events", crate::txtest::ROWS_KEPT);
/// A row of the log was written.
pub static FREQ_ACCURACY_EVENT: EventKind<FreqAccuracyEvent> =
    EventKind::new("freq_accuracy.event", "Event");

// ---- sinad ----

sdrkit_core::choice_enum! {
    /// The change of the state against the sensitivity mark.
    pub enum SinadCrossing {
        /// No change.
        NoCrossing = ("none", "None"),
        /// The SINAD fell under the mark.
        Fell = ("fell", "Fell under the mark"),
        /// The SINAD rose to the mark.
        Rose = ("rose", "Rose to the mark"),
    }
}

/// The result fields of `sinad`, from the last window. The readings have no value while no test tone is found.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct SinadFields {
    /// SINAD in the audio band, flat weighting.
    #[field(label = "SINAD", unit = "dB")]
    pub sinad_db: Option<f64>,
    /// The SINAD is under the sensitivity mark (12 dB by default).
    #[field(label = "Under the sensitivity mark")]
    pub below_12db: Option<bool>,
    /// The test tone.
    #[field(label = "Test tone", unit = "Hz")]
    pub tone_hz: Option<Hertz>,
    /// The peak deviation of the tone.
    #[field(label = "Peak deviation", unit = "kHz")]
    pub deviation_khz: Option<f64>,
    /// THD of harmonics 2 to 5.
    #[field(label = "THD", unit = "%")]
    pub thd_pct: Option<f64>,
    /// THD and noise.
    #[field(label = "THD + noise", unit = "%")]
    pub thdn_pct: Option<f64>,
    /// The carrier offset: the mean of the discriminator audio.
    #[field(label = "Carrier offset", unit = "Hz")]
    pub carrier_offset_hz: Option<Hertz>,
    /// The windows measured.
    #[field(label = "Windows")]
    pub windows: i64,
}

/// A row of the `sinad` log.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct SinadEventRow {
    /// The signal time.
    #[field(label = "Time", unit = "s")]
    pub time_s: Seconds,
    /// SINAD.
    #[field(label = "SINAD", unit = "dB")]
    pub sinad_db: f64,
    /// The change against the mark.
    #[field(label = "Crossing")]
    pub crossing: SinadCrossing,
    /// The test tone.
    #[field(label = "Test tone", unit = "Hz")]
    pub tone_hz: Hertz,
    /// The peak deviation.
    #[field(label = "Peak deviation", unit = "kHz")]
    pub deviation_khz: f64,
    /// THD.
    #[field(label = "THD", unit = "%")]
    pub thd_pct: f64,
}

/// The SINAD crossed the sensitivity mark.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct SinadCrossingEvent {
    /// The signal time.
    #[field(label = "Time", unit = "s")]
    pub time_s: Seconds,
    /// SINAD.
    #[field(label = "SINAD", unit = "dB")]
    pub sinad_db: f64,
    /// The change against the mark.
    #[field(label = "Crossing")]
    pub crossing: SinadCrossing,
    /// The test tone.
    #[field(label = "Test tone", unit = "Hz")]
    pub tone_hz: Hertz,
    /// The peak deviation.
    #[field(label = "Peak deviation", unit = "kHz")]
    pub deviation_khz: f64,
    /// THD.
    #[field(label = "THD", unit = "%")]
    pub thd_pct: f64,
}

/// The readings, newest first.
pub static SINAD_EVENTS: Table<SinadEventRow> =
    Table::new("events", "Readings", crate::txtest::ROWS_KEPT);
/// The SINAD crossed the mark.
pub static SINAD_CROSSING: EventKind<SinadCrossingEvent> =
    EventKind::new("sinad.crossing", "Crossing");

// ---- nfm_meter ----

sdrkit_core::choice_enum! {
    /// The kind of a row of the `nfm_meter` log.
    pub enum NfmMeterKind {
        /// A CTCSS tone appeared or changed.
        Tone = ("tone", "Tone"),
        /// The CTCSS tone went.
        ToneGone = ("tone_gone", "Tone gone"),
        /// A window over the narrow limit.
        OverNarrow = ("over_narrow", "Over narrow"),
        /// A window over the wide limit.
        OverWide = ("over_wide", "Over wide"),
    }
}

/// The result fields of `nfm_meter`, from the last window. The voice readings have no value without a carrier, the CTCSS
/// readings none without a tone.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct NfmMeterFields {
    /// The 99.9th percentile of the voice band deviation of the window.
    #[field(label = "Voice peak deviation", unit = "kHz")]
    pub voice_peak_khz: Option<f64>,
    /// The largest voice peak so far.
    #[field(label = "Largest voice peak", unit = "kHz")]
    pub voice_peak_max_khz: Option<f64>,
    /// Windows over the narrow channel limit.
    #[field(label = "Windows over the narrow limit")]
    pub over_narrow: i64,
    /// Windows over the wide channel limit.
    #[field(label = "Windows over the wide limit")]
    pub over_wide: i64,
    /// Windows measured.
    #[field(label = "Windows")]
    pub windows: i64,
    /// The CTCSS tone measured.
    #[field(label = "CTCSS tone", unit = "Hz")]
    pub ctcss_hz: Option<Hertz>,
    /// The nearest standard tone.
    #[field(label = "Nearest standard tone", unit = "Hz")]
    pub ctcss_standard_hz: Option<Hertz>,
    /// Measured minus standard.
    #[field(label = "CTCSS error", unit = "Hz")]
    pub ctcss_error_hz: Option<Hertz>,
    /// The peak deviation of the CTCSS tone.
    #[field(label = "CTCSS deviation", unit = "Hz")]
    pub ctcss_deviation_hz: Option<Hertz>,
    /// The CTCSS deviation as a share of the narrow limit.
    #[field(label = "CTCSS of the narrow limit", unit = "%")]
    pub ctcss_share_narrow_pct: Option<f64>,
    /// The CTCSS deviation as a share of the wide limit.
    #[field(label = "CTCSS of the wide limit", unit = "%")]
    pub ctcss_share_wide_pct: Option<f64>,
    /// The carrier offset: the mean of the discriminator audio.
    #[field(label = "Carrier offset", unit = "Hz")]
    pub carrier_offset_hz: Option<Hertz>,
}

/// A row of the `nfm_meter` log.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct NfmMeterEventRow {
    /// The signal time.
    #[field(label = "Time", unit = "s")]
    pub time_s: Seconds,
    /// The kind.
    #[field(label = "Kind")]
    pub kind: NfmMeterKind,
    /// The nearest standard tone.
    #[field(label = "Standard tone", unit = "Hz")]
    pub ctcss_standard_hz: Option<Hertz>,
    /// The CTCSS tone measured.
    #[field(label = "CTCSS tone", unit = "Hz")]
    pub ctcss_hz: Option<Hertz>,
    /// The CTCSS deviation.
    #[field(label = "CTCSS deviation", unit = "Hz")]
    pub ctcss_deviation_hz: Option<Hertz>,
    /// The voice peak of the window.
    #[field(label = "Voice peak", unit = "kHz")]
    pub voice_peak_khz: Option<f64>,
}

/// The tone and over-deviation rows, newest first.
pub static NFM_METER_EVENTS: Table<NfmMeterEventRow> =
    Table::new("events", "Events", crate::txtest::ROWS_KEPT);

// ---- tx_stability ----

/// The result fields of `tx_stability`: the transmission now, and the last analysed transmission.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct TxStabilityFields {
    /// A transmission is on.
    #[field(label = "Transmission on")]
    pub transmitting: bool,
    /// The length of the transmission.
    #[field(label = "Length", unit = "s")]
    pub length_s: Option<Seconds>,
    /// The frequency readings in it.
    #[field(label = "Readings")]
    pub readings: Option<i64>,
    /// The mean frequency from 0.05 to 0.2 s after key-up, from the tuned frequency.
    #[field(label = "Start frequency", unit = "Hz")]
    pub start_hz: Option<Hertz>,
    /// The mean frequency of the last 0.4 s.
    #[field(label = "Final frequency", unit = "Hz")]
    pub final_hz: Option<Hertz>,
    /// Start minus final.
    #[field(label = "Key-up step (start minus final)", unit = "Hz")]
    pub keyup_step_hz: Option<Hertz>,
    /// The time of the last reading outside the settling band, plus 0.05 s.
    #[field(label = "Settling time", unit = "s")]
    pub settle_s: Option<Seconds>,
    /// The larger of 10 Hz and a tenth of the key-up step.
    #[field(label = "Settling band", unit = "Hz")]
    pub settle_band_hz: Option<Hertz>,
    /// The least-squares slope of the readings of the second half.
    #[field(label = "Drift over the second half", unit = "Hz/s")]
    pub drift_hz_s: Option<f64>,
    /// The mean of all readings.
    #[field(label = "Mean offset", unit = "Hz")]
    pub mean_hz: Option<Hertz>,
    /// The mean over the tuned frequency; no value without it.
    #[field(label = "Mean offset", unit = "ppm")]
    pub mean_ppm: Option<f64>,
    /// The RMS residual about the drift line.
    #[field(label = "RMS about the drift line", unit = "Hz")]
    pub scatter_hz: Option<Hertz>,
    /// The transmissions seen, the short ones included.
    #[field(label = "Transmissions seen")]
    pub transmissions: i64,
}

/// A row of the transmission table: one analysed transmission.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct TxStabilityTransmissionRow {
    /// The key-up, signal time.
    #[field(label = "Key-up", unit = "s")]
    pub time_s: Seconds,
    /// The length of the transmission.
    #[field(label = "Length", unit = "s")]
    pub length_s: Seconds,
    /// The frequency readings in it.
    #[field(label = "Readings")]
    pub readings: i64,
    /// The mean frequency from 0.05 to 0.2 s after key-up, from the tuned frequency.
    #[field(label = "Start frequency", unit = "Hz")]
    pub start_hz: Hertz,
    /// The mean frequency of the last 0.4 s.
    #[field(label = "Final frequency", unit = "Hz")]
    pub final_hz: Hertz,
    /// Start minus final.
    #[field(label = "Key-up step (start minus final)", unit = "Hz")]
    pub keyup_step_hz: Hertz,
    /// The time of the last reading outside the settling band, plus 0.05 s.
    #[field(label = "Settling time", unit = "s")]
    pub settle_s: Seconds,
    /// The larger of 10 Hz and a tenth of the key-up step.
    #[field(label = "Settling band", unit = "Hz")]
    pub settle_band_hz: Hertz,
    /// The least-squares slope of the readings of the second half.
    #[field(label = "Drift over the second half", unit = "Hz/s")]
    pub drift_hz_s: Option<f64>,
    /// The mean of all readings.
    #[field(label = "Mean offset", unit = "Hz")]
    pub mean_hz: Hertz,
    /// The mean over the tuned frequency; no value without it.
    #[field(label = "Mean offset", unit = "ppm")]
    pub mean_ppm: Option<f64>,
    /// The RMS residual about the drift line.
    #[field(label = "RMS about the drift line", unit = "Hz")]
    pub scatter_hz: Option<Hertz>,
}

/// A transmission was analysed.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct TxStabilityTransmissionEvent {
    /// The key-up, signal time.
    #[field(label = "Key-up", unit = "s")]
    pub time_s: Seconds,
    /// The length of the transmission.
    #[field(label = "Length", unit = "s")]
    pub length_s: Seconds,
    /// The frequency readings in it.
    #[field(label = "Readings")]
    pub readings: i64,
    /// The mean frequency from 0.05 to 0.2 s after key-up, from the tuned frequency.
    #[field(label = "Start frequency", unit = "Hz")]
    pub start_hz: Hertz,
    /// The mean frequency of the last 0.4 s.
    #[field(label = "Final frequency", unit = "Hz")]
    pub final_hz: Hertz,
    /// Start minus final.
    #[field(label = "Key-up step (start minus final)", unit = "Hz")]
    pub keyup_step_hz: Hertz,
    /// The time of the last reading outside the settling band, plus 0.05 s.
    #[field(label = "Settling time", unit = "s")]
    pub settle_s: Seconds,
    /// The larger of 10 Hz and a tenth of the key-up step.
    #[field(label = "Settling band", unit = "Hz")]
    pub settle_band_hz: Hertz,
    /// The least-squares slope of the readings of the second half.
    #[field(label = "Drift over the second half", unit = "Hz/s")]
    pub drift_hz_s: Option<f64>,
    /// The mean of all readings.
    #[field(label = "Mean offset", unit = "Hz")]
    pub mean_hz: Hertz,
    /// The mean over the tuned frequency; no value without it.
    #[field(label = "Mean offset", unit = "ppm")]
    pub mean_ppm: Option<f64>,
    /// The RMS residual about the drift line.
    #[field(label = "RMS about the drift line", unit = "Hz")]
    pub scatter_hz: Option<Hertz>,
}

/// The analysed transmissions, newest first.
pub static TX_STABILITY_TRANSMISSIONS: Table<TxStabilityTransmissionRow> =
    Table::new("transmissions", "Transmissions", crate::txtest::ROWS_KEPT);
/// A transmission was analysed.
pub static TX_STABILITY_TRANSMISSION: EventKind<TxStabilityTransmissionEvent> =
    EventKind::new("tx_stability.transmission", "Transmission");

// ---- ssb_test ----

sdrkit_core::choice_enum! {
    /// The test that `ssb_test` sees.
    pub enum SsbTestKind {
        /// No clean test tone.
        NoTest = ("none", "None"),
        /// One tone.
        OneTone = ("one_tone", "One tone"),
        /// Two tones.
        TwoTones = ("two_tones", "Two tones"),
    }
}

sdrkit_core::choice_enum! {
    /// A sideband.
    pub enum SsbSideband {
        /// The upper sideband.
        Upper = ("upper", "Upper"),
        /// The lower sideband.
        Lower = ("lower", "Lower"),
    }
}

/// The result fields of `ssb_test`, from the last window. A level under the floor of the measurement is the floor with its
/// flag set.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct SsbTestFields {
    /// The test seen.
    #[field(label = "Test")]
    pub kind: SsbTestKind,
    /// The sideband of the tones.
    #[field(label = "Sideband")]
    pub sideband: Option<SsbSideband>,
    /// The audio frequency of the (lower) tone.
    #[field(label = "Tone 1", unit = "Hz")]
    pub tone1_hz: Option<Hertz>,
    /// The audio frequency of the higher tone of two.
    #[field(label = "Tone 2", unit = "Hz")]
    pub tone2_hz: Option<Hertz>,
    /// The stronger tone over the weaker.
    #[field(label = "Level difference of the tones", unit = "dB")]
    pub tone_level_diff_db: Option<f64>,
    /// The carrier leak under the tone (the weaker tone of two). When no carrier is seen, the floor of the carrier search:
    /// the leak is at least this far under the tone.
    #[field(label = "Carrier leak under the tone", unit = "dB")]
    pub carrier_db: Option<f64>,
    /// No carrier is seen and the value is the floor of the carrier search, a bound.
    #[field(label = "Carrier under the floor")]
    pub carrier_under: Option<bool>,
    /// A carrier was seen; else it is assumed at the tuned frequency for the tones and products.
    #[field(label = "Carrier seen")]
    pub carrier_found: Option<bool>,
    /// The opposite sideband of the (lower) tone.
    #[field(label = "Opposite sideband under the tone", unit = "dB")]
    pub opposite_db: Option<f64>,
    /// The opposite sideband is under the floor.
    #[field(label = "Opposite sideband under the floor")]
    pub opposite_under: Option<bool>,
    /// Harmonic 2 of the audio tone (one tone).
    #[field(label = "Harmonic 2 under the tone", unit = "dB")]
    pub h2_db: Option<f64>,
    /// Harmonic 3 of the audio tone (one tone).
    #[field(label = "Harmonic 3 under the tone", unit = "dB")]
    pub h3_db: Option<f64>,
    /// Harmonic 4 of the audio tone (one tone).
    #[field(label = "Harmonic 4 under the tone", unit = "dB")]
    pub h4_db: Option<f64>,
    /// Harmonic 5 of the audio tone (one tone).
    #[field(label = "Harmonic 5 under the tone", unit = "dB")]
    pub h5_db: Option<f64>,
    /// Harmonic 2 is under the floor.
    #[field(label = "Harmonic 2 under the floor")]
    pub h2_under: Option<bool>,
    /// Harmonic 3 is under the floor.
    #[field(label = "Harmonic 3 under the floor")]
    pub h3_under: Option<bool>,
    /// Harmonic 4 is under the floor.
    #[field(label = "Harmonic 4 under the floor")]
    pub h4_under: Option<bool>,
    /// Harmonic 5 is under the floor.
    #[field(label = "Harmonic 5 under the floor")]
    pub h5_under: Option<bool>,
    /// The stronger product of order 3 under the weaker tone (two tones).
    #[field(label = "IMD3 under one tone", unit = "dB")]
    pub imd3_db: Option<f64>,
    /// The stronger product of order 5 under the weaker tone (two tones).
    #[field(label = "IMD5 under one tone", unit = "dB")]
    pub imd5_db: Option<f64>,
    /// The stronger product of order 7 under the weaker tone (two tones).
    #[field(label = "IMD7 under one tone", unit = "dB")]
    pub imd7_db: Option<f64>,
    /// The products of order 3 are under the floor.
    #[field(label = "IMD3 under the floor")]
    pub imd3_under: Option<bool>,
    /// The products of order 5 are under the floor.
    #[field(label = "IMD5 under the floor")]
    pub imd5_under: Option<bool>,
    /// The products of order 7 are under the floor.
    #[field(label = "IMD7 under the floor")]
    pub imd7_under: Option<bool>,
    /// The product of order 3 under the peak envelope power.
    #[field(label = "IMD3 under PEP", unit = "dB")]
    pub imd3_pep_db: Option<f64>,
    /// The product of order 5 under the peak envelope power.
    #[field(label = "IMD5 under PEP", unit = "dB")]
    pub imd5_pep_db: Option<f64>,
    /// The product of order 7 under the peak envelope power.
    #[field(label = "IMD7 under PEP", unit = "dB")]
    pub imd7_pep_db: Option<f64>,
    /// The floor of this measurement under the tone: the noise of 7 bins.
    #[field(label = "Measurement floor under the tone", unit = "dB")]
    pub floor_db: Option<f64>,
}

/// A row of the `ssb_test` log.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct SsbTestEventRow {
    /// The signal time.
    #[field(label = "Time", unit = "s")]
    pub time_s: Seconds,
    /// The test seen.
    #[field(label = "Test")]
    pub kind: SsbTestKind,
    /// The sideband of the tones.
    #[field(label = "Sideband")]
    pub sideband: Option<SsbSideband>,
    /// The audio frequency of the (lower) tone.
    #[field(label = "Tone 1", unit = "Hz")]
    pub tone1_hz: Option<Hertz>,
    /// The audio frequency of the higher tone of two.
    #[field(label = "Tone 2", unit = "Hz")]
    pub tone2_hz: Option<Hertz>,
    /// The stronger tone over the weaker.
    #[field(label = "Level difference of the tones", unit = "dB")]
    pub tone_level_diff_db: Option<f64>,
    /// The carrier leak under the tone (the weaker tone of two). When no carrier is seen, the floor of the carrier search:
    /// the leak is at least this far under the tone.
    #[field(label = "Carrier leak under the tone", unit = "dB")]
    pub carrier_db: Option<f64>,
    /// No carrier is seen and the value is the floor of the carrier search, a bound.
    #[field(label = "Carrier under the floor")]
    pub carrier_under: Option<bool>,
    /// A carrier was seen; else it is assumed at the tuned frequency for the tones and products.
    #[field(label = "Carrier seen")]
    pub carrier_found: Option<bool>,
    /// The opposite sideband of the (lower) tone.
    #[field(label = "Opposite sideband under the tone", unit = "dB")]
    pub opposite_db: Option<f64>,
    /// The opposite sideband is under the floor.
    #[field(label = "Opposite sideband under the floor")]
    pub opposite_under: Option<bool>,
    /// Harmonic 2 of the audio tone (one tone).
    #[field(label = "Harmonic 2 under the tone", unit = "dB")]
    pub h2_db: Option<f64>,
    /// Harmonic 3 of the audio tone (one tone).
    #[field(label = "Harmonic 3 under the tone", unit = "dB")]
    pub h3_db: Option<f64>,
    /// Harmonic 4 of the audio tone (one tone).
    #[field(label = "Harmonic 4 under the tone", unit = "dB")]
    pub h4_db: Option<f64>,
    /// Harmonic 5 of the audio tone (one tone).
    #[field(label = "Harmonic 5 under the tone", unit = "dB")]
    pub h5_db: Option<f64>,
    /// Harmonic 2 is under the floor.
    #[field(label = "Harmonic 2 under the floor")]
    pub h2_under: Option<bool>,
    /// Harmonic 3 is under the floor.
    #[field(label = "Harmonic 3 under the floor")]
    pub h3_under: Option<bool>,
    /// Harmonic 4 is under the floor.
    #[field(label = "Harmonic 4 under the floor")]
    pub h4_under: Option<bool>,
    /// Harmonic 5 is under the floor.
    #[field(label = "Harmonic 5 under the floor")]
    pub h5_under: Option<bool>,
    /// The stronger product of order 3 under the weaker tone (two tones).
    #[field(label = "IMD3 under one tone", unit = "dB")]
    pub imd3_db: Option<f64>,
    /// The stronger product of order 5 under the weaker tone (two tones).
    #[field(label = "IMD5 under one tone", unit = "dB")]
    pub imd5_db: Option<f64>,
    /// The stronger product of order 7 under the weaker tone (two tones).
    #[field(label = "IMD7 under one tone", unit = "dB")]
    pub imd7_db: Option<f64>,
    /// The products of order 3 are under the floor.
    #[field(label = "IMD3 under the floor")]
    pub imd3_under: Option<bool>,
    /// The products of order 5 are under the floor.
    #[field(label = "IMD5 under the floor")]
    pub imd5_under: Option<bool>,
    /// The products of order 7 are under the floor.
    #[field(label = "IMD7 under the floor")]
    pub imd7_under: Option<bool>,
    /// The product of order 3 under the peak envelope power.
    #[field(label = "IMD3 under PEP", unit = "dB")]
    pub imd3_pep_db: Option<f64>,
    /// The product of order 5 under the peak envelope power.
    #[field(label = "IMD5 under PEP", unit = "dB")]
    pub imd5_pep_db: Option<f64>,
    /// The product of order 7 under the peak envelope power.
    #[field(label = "IMD7 under PEP", unit = "dB")]
    pub imd7_pep_db: Option<f64>,
    /// The floor of this measurement under the tone: the noise of 7 bins.
    #[field(label = "Measurement floor under the tone", unit = "dB")]
    pub floor_db: Option<f64>,
}

/// The test rows, newest first.
pub static SSB_TEST_EVENTS: Table<SsbTestEventRow> =
    Table::new("events", "Tests", crate::txtest::ROWS_KEPT);

// ---- afsk_tuner ----

/// The result fields of `afsk_tuner`: the last analysed burst and the totals.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct AfskTunerFields {
    /// The mark tone of the last burst.
    #[field(label = "Mark tone", unit = "Hz")]
    pub mark_hz: Option<Hertz>,
    /// The space tone of the last burst.
    #[field(label = "Space tone", unit = "Hz")]
    pub space_hz: Option<Hertz>,
    /// The mark tone minus the set mark tone.
    #[field(label = "Mark error", unit = "Hz")]
    pub mark_error_hz: Option<Hertz>,
    /// The space tone minus the set space tone.
    #[field(label = "Space error", unit = "Hz")]
    pub space_error_hz: Option<Hertz>,
    /// The twist of the last burst.
    #[field(label = "Twist, space over mark", unit = "dB")]
    pub twist_db: Option<f64>,
    /// The peak deviation of the last burst.
    #[field(label = "Peak deviation", unit = "kHz")]
    pub peak_deviation_khz: Option<f64>,
    /// The length of the last burst.
    #[field(label = "Burst length", unit = "s")]
    pub length_s: Option<Seconds>,
    /// The gap before the last burst.
    #[field(label = "Time since the burst before", unit = "s")]
    pub gap_s: Option<Seconds>,
    /// The bursts analysed.
    #[field(label = "Bursts analysed")]
    pub bursts: i64,
    /// The least peak deviation over the bursts.
    #[field(label = "Least peak deviation", unit = "kHz")]
    pub deviation_min_khz: Option<f64>,
    /// The largest peak deviation over the bursts.
    #[field(label = "Largest peak deviation", unit = "kHz")]
    pub deviation_max_khz: Option<f64>,
}

/// A row of the burst table.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct AfskTunerBurstRow {
    /// The start of the burst, signal time.
    #[field(label = "Start", unit = "s")]
    pub time_s: Seconds,
    /// The length of the burst.
    #[field(label = "Length", unit = "s")]
    pub length_s: Seconds,
    /// The mark tone.
    #[field(label = "Mark tone", unit = "Hz")]
    pub mark_hz: Hertz,
    /// The space tone.
    #[field(label = "Space tone", unit = "Hz")]
    pub space_hz: Hertz,
    /// The space-band power over the mark-band power.
    #[field(label = "Twist, space over mark", unit = "dB")]
    pub twist_db: f64,
    /// The 99.5th percentile of the band-limited audio.
    #[field(label = "Peak deviation", unit = "kHz")]
    pub peak_deviation_khz: f64,
    /// The time since the end of the burst before; no value for the first.
    #[field(label = "Gap", unit = "s")]
    pub gap_s: Option<Seconds>,
}

/// A burst was analysed.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct AfskTunerBurstEvent {
    /// The start of the burst, signal time.
    #[field(label = "Start", unit = "s")]
    pub time_s: Seconds,
    /// The length of the burst.
    #[field(label = "Length", unit = "s")]
    pub length_s: Seconds,
    /// The mark tone.
    #[field(label = "Mark tone", unit = "Hz")]
    pub mark_hz: Hertz,
    /// The space tone.
    #[field(label = "Space tone", unit = "Hz")]
    pub space_hz: Hertz,
    /// The space-band power over the mark-band power.
    #[field(label = "Twist, space over mark", unit = "dB")]
    pub twist_db: f64,
    /// The 99.5th percentile of the band-limited audio.
    #[field(label = "Peak deviation", unit = "kHz")]
    pub peak_deviation_khz: f64,
    /// The time since the end of the burst before; no value for the first.
    #[field(label = "Gap", unit = "s")]
    pub gap_s: Option<Seconds>,
}

/// The analysed bursts, newest first.
pub static AFSK_TUNER_BURSTS: Table<AfskTunerBurstRow> =
    Table::new("bursts", "Bursts", crate::txtest::ROWS_KEPT);
/// A burst was analysed.
pub static AFSK_TUNER_BURST: EventKind<AfskTunerBurstEvent> =
    EventKind::new("afsk_tuner.burst", "Burst");

// ---- phase_noise ----

sdrkit_core::choice_enum! {
    /// The kind of a row of the `phase_noise` log.
    pub enum PhaseNoiseKind {
        /// A reading of the average.
        Reading = ("reading", "Reading"),
        /// The carrier was lost.
        Lost = ("lost", "Lost"),
    }
}

/// The result fields of `phase_noise`, from the average. They have no value before 4 blocks are averaged.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct PhaseNoiseFields {
    /// Single-sideband phase noise at 10 Hz from the carrier.
    #[field(label = "L(10 Hz)", unit = "dBc/Hz")]
    pub l_10hz: Option<f64>,
    /// Single-sideband phase noise at 100 Hz from the carrier.
    #[field(label = "L(100 Hz)", unit = "dBc/Hz")]
    pub l_100hz: Option<f64>,
    /// Single-sideband phase noise at 1 kHz from the carrier.
    #[field(label = "L(1 kHz)", unit = "dBc/Hz")]
    pub l_1khz: Option<f64>,
    /// Single-sideband phase noise at 10 kHz from the carrier.
    #[field(label = "L(10 kHz)", unit = "dBc/Hz")]
    pub l_10khz: Option<f64>,
    /// Single-sideband phase noise at 100 kHz from the carrier.
    #[field(label = "L(100 kHz)", unit = "dBc/Hz")]
    pub l_100khz: Option<f64>,
    /// The RMS phase jitter of the integration band.
    #[field(label = "Integrated phase jitter", unit = "mrad")]
    pub jitter_mrad: Option<f64>,
    /// The jitter as time at the tuned frequency; no value without it.
    #[field(label = "Integrated jitter as time", unit = "ps")]
    pub jitter_ps: Option<f64>,
    /// The carrier offset from the tuned frequency.
    #[field(label = "Carrier offset", unit = "Hz")]
    pub carrier_offset_hz: Option<Hertz>,
    /// The carrier over the noise in 200 kHz.
    #[field(label = "Carrier over the noise in 200 kHz", unit = "dB")]
    pub cnr_db: Option<f64>,
    /// The blocks in the average.
    #[field(label = "Blocks averaged")]
    pub blocks: i64,
    /// The length of a block.
    #[field(label = "Block length", unit = "s")]
    pub block_s: Option<Seconds>,
    /// The width of a bin.
    #[field(label = "Bin width", unit = "Hz")]
    pub bin_hz: Option<Hertz>,
    /// The equivalent noise bandwidth of the window.
    #[field(label = "Window ENBW", unit = "bins")]
    pub enbw_bins: Option<f64>,
}

/// A row of the spur table.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct PhaseNoiseSpurRow {
    /// The offset of the spur from the carrier, upper side.
    #[field(label = "Offset", unit = "Hz")]
    pub offset_hz: Hertz,
    /// The level of the spur.
    #[field(label = "Level", unit = "dBc")]
    pub level_dbc: f64,
}

/// A row of the `phase_noise` log.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct PhaseNoiseEventRow {
    /// The signal time.
    #[field(label = "Time", unit = "s")]
    pub time_s: Seconds,
    /// The kind.
    #[field(label = "Kind")]
    pub kind: PhaseNoiseKind,
    /// Single-sideband phase noise at 10 Hz from the carrier.
    #[field(label = "L(10 Hz)", unit = "dBc/Hz")]
    pub l_10hz: Option<f64>,
    /// Single-sideband phase noise at 100 Hz from the carrier.
    #[field(label = "L(100 Hz)", unit = "dBc/Hz")]
    pub l_100hz: Option<f64>,
    /// Single-sideband phase noise at 1 kHz from the carrier.
    #[field(label = "L(1 kHz)", unit = "dBc/Hz")]
    pub l_1khz: Option<f64>,
    /// Single-sideband phase noise at 10 kHz from the carrier.
    #[field(label = "L(10 kHz)", unit = "dBc/Hz")]
    pub l_10khz: Option<f64>,
    /// Single-sideband phase noise at 100 kHz from the carrier.
    #[field(label = "L(100 kHz)", unit = "dBc/Hz")]
    pub l_100khz: Option<f64>,
    /// The jitter.
    #[field(label = "Integrated phase jitter", unit = "mrad")]
    pub jitter_mrad: Option<f64>,
    /// The spurs listed.
    #[field(label = "Spurs")]
    pub spurs: i64,
}

/// The spurs, the strongest first.
pub static PHASE_NOISE_SPURS: Table<PhaseNoiseSpurRow> = Table::new("spurs", "Spurs", 6).sorted(
    &PhaseNoiseSpurRow::LEVEL_DBC,
    sdrkit_core::SortOrder::Descending,
);
/// The readings and losses, newest first.
pub static PHASE_NOISE_EVENTS: Table<PhaseNoiseEventRow> =
    Table::new("events", "Events", crate::txtest::ROWS_KEPT);

// ---- cw_keying ----

/// The result fields of `cw_keying`: the evaluation of the last marks and spaces. The evaluation has no value before 10
/// marks and when no mark came in the last 8 s.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct CwKeyingFields {
    /// The speed in words a minute of 50 units.
    #[field(label = "Speed (PARIS)", unit = "wpm")]
    pub wpm: Option<f64>,
    /// The mean dit.
    #[field(label = "Dit", unit = "ms")]
    pub dit_ms: Option<f64>,
    /// The mean dah; no value with fewer than 2 dahs.
    #[field(label = "Dah", unit = "ms")]
    pub dah_ms: Option<f64>,
    /// The mean dah over the mean dit.
    #[field(label = "Dah over dit (ideal 3.00)")]
    pub dah_ratio: Option<f64>,
    /// The mean gap inside a character, in dits.
    #[field(label = "Element gap (ideal 1)", unit = "dits")]
    pub element_gap: Option<f64>,
    /// The mean gap between characters, in dits.
    #[field(label = "Character gap (ideal 3)", unit = "dits")]
    pub char_gap: Option<f64>,
    /// The mean gap between words, in dits.
    #[field(label = "Word gap (ideal 7)", unit = "dits")]
    pub word_gap: Option<f64>,
    /// The character gaps counted.
    #[field(label = "Character gaps")]
    pub char_gaps: Option<i64>,
    /// The word gaps counted.
    #[field(label = "Word gaps")]
    pub word_gaps: Option<i64>,
    /// The dit length over the element gap length.
    #[field(label = "Dit over element gap (ideal 1.00)")]
    pub weight: Option<f64>,
    /// The population standard deviation of the dits over their mean.
    #[field(label = "Coefficient of variation of the dit", unit = "%")]
    pub dit_scatter_pct: Option<f64>,
    /// The mean 10 to 90 % rise of the marks; 1.5 ms is the least the envelope filter shows.
    #[field(label = "10 to 90 % rise", unit = "ms")]
    pub rise_ms: Option<f64>,
    /// The mean 90 to 10 % fall of the marks.
    #[field(label = "90 to 10 % fall", unit = "ms")]
    pub fall_ms: Option<f64>,
    /// The mean frequency of the start of a mark minus that of its second half.
    #[field(label = "Chirp", unit = "Hz")]
    pub chirp_hz: Option<f64>,
    /// The marks the chirp is from.
    #[field(label = "Marks of the chirp")]
    pub chirp_marks: Option<i64>,
    /// The marks in the window.
    #[field(label = "Marks")]
    pub marks: i64,
    /// The spaces in the window.
    #[field(label = "Spaces")]
    pub spaces: i64,
    /// The carrier offset from the tuned frequency.
    #[field(label = "Carrier", unit = "Hz")]
    pub carrier_offset_hz: Option<Hertz>,
}

/// A row of the `cw_keying` log.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct CwKeyingEventRow {
    /// The signal time.
    #[field(label = "Time", unit = "s")]
    pub time_s: Seconds,
    /// The speed.
    #[field(label = "Speed (PARIS)", unit = "wpm")]
    pub wpm: f64,
    /// The mean dit.
    #[field(label = "Dit", unit = "ms")]
    pub dit_ms: f64,
    /// The dah ratio.
    #[field(label = "Dah over dit")]
    pub dah_ratio: Option<f64>,
    /// The element gap.
    #[field(label = "Element gap", unit = "dits")]
    pub element_gap: Option<f64>,
    /// The character gap.
    #[field(label = "Character gap", unit = "dits")]
    pub char_gap: Option<f64>,
    /// The word gap.
    #[field(label = "Word gap", unit = "dits")]
    pub word_gap: Option<f64>,
    /// The weight.
    #[field(label = "Weight")]
    pub weight: Option<f64>,
    /// The dit scatter.
    #[field(label = "Dit scatter", unit = "%")]
    pub dit_scatter_pct: f64,
    /// The rise.
    #[field(label = "Rise", unit = "ms")]
    pub rise_ms: Option<f64>,
    /// The fall.
    #[field(label = "Fall", unit = "ms")]
    pub fall_ms: Option<f64>,
    /// The chirp.
    #[field(label = "Chirp", unit = "Hz")]
    pub chirp_hz: Option<f64>,
}

/// The keying rows, newest first.
pub static CW_KEYING_EVENTS: Table<CwKeyingEventRow> =
    Table::new("events", "Keying", crate::txtest::ROWS_KEPT);

// ---- fsk_analyser ----

sdrkit_core::choice_enum! {
    /// The number of frequency levels.
    pub enum FskLevels {
        /// Two levels.
        Two = ("2", "Two levels"),
        /// Four levels.
        Four = ("4", "Four levels"),
    }
}

/// The result fields of `fsk_analyser`, from the last block. They have no value while no FSK signal is found.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct FskAnalyserFields {
    /// The number of levels.
    #[field(label = "Levels")]
    pub levels: Option<FskLevels>,
    /// Two levels with a modulation index of 0.5.
    #[field(label = "Modulation index 0.5")]
    pub msk_like: Option<bool>,
    /// The symbol rate.
    #[field(label = "Symbol rate", unit = "Bd")]
    pub symbol_rate_bd: Option<f64>,
    /// Two levels: the distance of the tones; four: the mean distance of neighbours.
    #[field(label = "Shift", unit = "Hz")]
    pub shift_hz: Option<Hertz>,
    /// The distance of the outer levels.
    #[field(label = "Outer distance", unit = "Hz")]
    pub outer_hz: Option<Hertz>,
    /// The outer distance over the symbol rate.
    #[field(label = "Modulation index", unit = "")]
    pub mod_index: Option<f64>,
    /// The centre of the levels from the tuned frequency.
    #[field(label = "Centre", unit = "Hz")]
    pub centre_hz: Option<Hertz>,
    /// Four levels: the mean outer spacing over the inner spacing.
    #[field(label = "Spacing ratio", unit = "")]
    pub spacing_ratio: Option<f64>,
    /// Four levels: the spacing ratio is within 0.35 of 1.
    #[field(label = "Equal spacing")]
    pub spacing_equal: Option<bool>,
    /// The level spacing over the spread inside a level.
    #[field(label = "Eye SNR", unit = "dB")]
    pub eye_snr_db: Option<f64>,
}

/// A row of the level table.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct FskAnalyserLevelRow {
    /// The place of the level from the lowest, from 0.
    #[field(label = "Index")]
    pub index: i64,
    /// The frequency of the level from the tuned frequency.
    #[field(label = "Frequency", unit = "Hz")]
    pub frequency_hz: Hertz,
}

/// A row of the `fsk_analyser` log.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct FskAnalyserEventRow {
    /// The signal time.
    #[field(label = "Time", unit = "s")]
    pub time_s: Seconds,
    /// The number of levels.
    #[field(label = "Levels")]
    pub levels: FskLevels,
    /// The symbol rate.
    #[field(label = "Symbol rate", unit = "Bd")]
    pub symbol_rate_bd: f64,
    /// The shift.
    #[field(label = "Shift", unit = "Hz")]
    pub shift_hz: Hertz,
    /// The modulation index.
    #[field(label = "Modulation index", unit = "")]
    pub mod_index: f64,
    /// The centre.
    #[field(label = "Centre", unit = "Hz")]
    pub centre_hz: Hertz,
    /// The eye SNR.
    #[field(label = "Eye SNR", unit = "dB")]
    pub eye_snr_db: f64,
}

/// The frequencies of the levels, from the lowest.
pub static FSK_ANALYSER_LEVEL_FREQS: Table<FskAnalyserLevelRow> =
    Table::new("level_freqs", "Levels", 4);
/// The readings, newest first.
pub static FSK_ANALYSER_EVENTS: Table<FskAnalyserEventRow> =
    Table::new("events", "Readings", crate::txtest::ROWS_KEPT);

// ---- mer ----

sdrkit_core::choice_enum! {
    /// The order of an M-PSK carrier.
    pub enum MerModulation {
        /// Two phases.
        Bpsk = ("bpsk", "BPSK"),
        /// Four phases.
        Qpsk = ("qpsk", "QPSK"),
        /// Eight phases.
        Psk8 = ("psk8", "8-PSK"),
    }
}

/// The result fields of `mer`, from the last block. The readings have no value without a lock.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct MerFields {
    /// A PSK carrier was found and measured in the last block.
    #[field(label = "PSK carrier found")]
    pub locked: Option<bool>,
    /// The order.
    #[field(label = "Order")]
    pub modulation: Option<MerModulation>,
    /// The modulation error ratio after matched filtering, timing and phase recovery.
    #[field(label = "MER", unit = "dB")]
    pub mer_db: Option<f64>,
    /// The RMS error vector, of the RMS amplitude.
    #[field(label = "EVM", unit = "%")]
    pub evm_pct: Option<f64>,
    /// The symbol rate.
    #[field(label = "Symbol rate", unit = "Bd")]
    pub symbol_rate_bd: Option<f64>,
    /// The carrier offset from the tuned frequency.
    #[field(label = "Carrier offset", unit = "Hz")]
    pub carrier_offset_hz: Option<Hertz>,
    /// The roll-off of the best matched filter.
    #[field(label = "Best roll-off", unit = "")]
    pub rolloff: Option<f64>,
    /// The symbols measured in the last block.
    #[field(label = "Symbols")]
    pub symbols: Option<i64>,
}

/// A row of the `mer` log.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct MerEventRow {
    /// The signal time.
    #[field(label = "Time", unit = "s")]
    pub time_s: Seconds,
    /// The order.
    #[field(label = "Order")]
    pub modulation: MerModulation,
    /// MER.
    #[field(label = "MER", unit = "dB")]
    pub mer_db: f64,
    /// EVM.
    #[field(label = "EVM", unit = "%")]
    pub evm_pct: f64,
    /// The symbol rate.
    #[field(label = "Symbol rate", unit = "Bd")]
    pub symbol_rate_bd: f64,
    /// The carrier offset.
    #[field(label = "Carrier offset", unit = "Hz")]
    pub carrier_offset_hz: Hertz,
    /// The mean power of the block, relative to full scale.
    #[field(label = "Level", unit = "dB")]
    pub level_db: f64,
}

/// The readings, newest first.
pub static MER_EVENTS: Table<MerEventRow> =
    Table::new("events", "Readings", crate::txtest::ROWS_KEPT);

// ---- papr ----

sdrkit_core::choice_enum! {
    /// The shape of the power distribution of `papr`.
    pub enum PaprReading {
        /// Crest factor under 2 dB.
        ConstantEnvelope = ("constant_envelope", "Constant envelope"),
        /// The 1 % level 5.5 to 7.5 dB and the 0.1 % level over 7.3 dB.
        GaussianLike = ("gaussian_like", "Gaussian-like"),
        /// The 0.1 % level under 7.0 dB.
        LessPeaky = ("less_peaky", "Less peaky than Gaussian"),
        /// Anything else: bursts, or a carrier with impulses.
        Peakier = ("peakier", "Peakier than Gaussian"),
    }
}

sdrkit_core::choice_enum! {
    /// The signal of the `papr` test generator.
    pub enum PaprGenKind {
        /// Complex Gaussian noise.
        Noise = ("noise", "Noise"),
        /// An FM carrier.
        Fm = ("fm", "FM carrier"),
        /// QPSK with root-raised-cosine pulses.
        Qpsk = ("qpsk", "QPSK"),
        /// Gaussian noise with its magnitude limited.
        ClippedNoise = ("clipped_noise", "Clipped noise"),
        /// Gaussian noise with periodic impulses.
        NoiseWithImpulses = ("noise_with_impulses", "Noise with impulses"),
    }
}

/// The result fields of `papr`, from the last evaluation over the kept samples.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct PaprFields {
    /// The mean power, relative to full scale.
    #[field(label = "Mean power", unit = "dBFS")]
    pub mean_dbfs: Option<f64>,
    /// The largest instantaneous power.
    #[field(label = "Peak", unit = "dBFS")]
    pub peak_dbfs: Option<f64>,
    /// The peak over the mean power.
    #[field(label = "Crest factor", unit = "dB")]
    pub crest_db: Option<f64>,
    /// The level over the mean exceeded by 10 % of the samples.
    #[field(label = "Level at 10 %", unit = "dB")]
    pub ccdf_10_db: Option<f64>,
    /// The level exceeded by 1 %.
    #[field(label = "Level at 1 %", unit = "dB")]
    pub ccdf_1_db: Option<f64>,
    /// The level exceeded by 0.1 %.
    #[field(label = "Level at 0.1 %", unit = "dB")]
    pub ccdf_01_db: Option<f64>,
    /// The level exceeded by 0.01 %; only with a million samples or more.
    #[field(label = "Level at 0.01 %", unit = "dB")]
    pub ccdf_001_db: Option<f64>,
    /// The samples in the statistics.
    #[field(label = "Samples")]
    pub samples: Option<i64>,
    /// The shape against Gaussian noise.
    #[field(label = "Shape")]
    pub reading: Option<PaprReading>,
}

/// A row of the `papr` log.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct PaprEventRow {
    /// The signal time.
    #[field(label = "Time", unit = "s")]
    pub time_s: Seconds,
    /// The level at 10 %.
    #[field(label = "Level at 10 %", unit = "dB")]
    pub ccdf_10_db: f64,
    /// The level at 1 %.
    #[field(label = "Level at 1 %", unit = "dB")]
    pub ccdf_1_db: f64,
    /// The level at 0.1 %.
    #[field(label = "Level at 0.1 %", unit = "dB")]
    pub ccdf_01_db: f64,
    /// The level at 0.01 %.
    #[field(label = "Level at 0.01 %", unit = "dB")]
    pub ccdf_001_db: Option<f64>,
    /// The crest factor.
    #[field(label = "Crest factor", unit = "dB")]
    pub crest_db: f64,
    /// The mean power.
    #[field(label = "Mean power", unit = "dBFS")]
    pub mean_dbfs: f64,
    /// The samples in the statistics.
    #[field(label = "Samples")]
    pub samples: i64,
}

/// The readings, newest first.
pub static PAPR_EVENTS: Table<PaprEventRow> =
    Table::new("events", "Readings", crate::txtest::ROWS_KEPT);

// ---- input_health ----

sdrkit_core::choice_enum! {
    /// The level verdict of `input_health`.
    pub enum InputVerdict {
        /// The RMS level is under the low limit: the radio's own noise is not reached.
        Low = ("low", "Low"),
        /// Within the limits.
        Good = ("good", "Good"),
        /// The RMS level is over the high limit.
        High = ("high", "High"),
        /// More samples near full scale than the overload limit.
        Overload = ("overload", "Overload"),
    }
}

sdrkit_core::choice_enum! {
    /// What a row of the `input_health` log reports.
    pub enum InputEventKind {
        /// The verdict changed.
        Verdict = ("verdict", "Verdict"),
        /// Samples near full scale over the overload limit.
        Clipping = ("clipping", "Clipping"),
    }
}

/// The result fields of `input_health`, from the last block of raw samples. Levels are against a full scale of 1.0 per
/// component.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct InputHealthFields {
    /// The RMS level, DC included.
    #[field(label = "Level", unit = "dBFS")]
    pub rms_dbfs: Option<f64>,
    /// The largest of `|I|` and `|Q|`.
    #[field(label = "Peak", unit = "dBFS")]
    pub peak_dbfs: Option<f64>,
    /// The peak over the RMS level.
    #[field(label = "Crest factor", unit = "dB")]
    pub crest_db: Option<f64>,
    /// The samples with `|I|` or `|Q|` at the clip level or over it.
    #[field(label = "Samples near full scale", unit = "ppm")]
    pub clipping_ppm: Option<f64>,
    /// The mean of I.
    #[field(label = "DC of I", unit = "%")]
    pub dc_i_pct: Option<f64>,
    /// The mean of Q.
    #[field(label = "DC of Q", unit = "%")]
    pub dc_q_pct: Option<f64>,
    /// The length of the mean vector.
    #[field(label = "DC offset", unit = "%")]
    pub dc_pct: Option<f64>,
    /// The power of the mean vector under the RMS level.
    #[field(label = "DC under the level", unit = "dB")]
    pub dc_under_rms_db: Option<f64>,
    /// `10 log10(V_Q / V_I)`.
    #[field(label = "IQ gain mismatch, Q against I", unit = "dB")]
    pub iq_gain_db: Option<f64>,
    /// `asin` of the correlation of I and Q.
    #[field(label = "IQ phase error", unit = "deg")]
    pub iq_phase_deg: Option<f64>,
    /// The image rejection that the mismatch implies, at most 120 dB.
    #[field(label = "Image rejection from the mismatch", unit = "dB")]
    pub image_rejection_db: Option<f64>,
    /// The level verdict.
    #[field(label = "Level verdict")]
    pub verdict: Option<InputVerdict>,
}

/// A row of the `input_health` log.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct InputHealthEventRow {
    /// The signal time.
    #[field(label = "Time", unit = "s")]
    pub time_s: Seconds,
    /// What the row reports.
    #[field(label = "Kind")]
    pub kind: InputEventKind,
    /// The verdict.
    #[field(label = "Verdict")]
    pub verdict: InputVerdict,
    /// The RMS level.
    #[field(label = "Level", unit = "dBFS")]
    pub rms_dbfs: f64,
    /// The peak.
    #[field(label = "Peak", unit = "dBFS")]
    pub peak_dbfs: f64,
    /// The samples near full scale.
    #[field(label = "Samples near full scale", unit = "ppm")]
    pub clipping_ppm: f64,
}

/// The changes of the verdict and the clipping, newest first.
pub static INPUT_HEALTH_EVENTS: Table<InputHealthEventRow> =
    Table::new("events", "Changes", crate::txtest::ROWS_KEPT);

// ---- noise_figure ----

sdrkit_core::choice_enum! {
    /// The state of the noise source after a step of `noise_figure`.
    pub enum NoiseSourceState {
        /// The power rose: the source came on.
        SourceOn = ("source_on", "Source on"),
        /// The power fell: the source went off.
        SourceOff = ("source_off", "Source off"),
    }
}

/// The result fields of `noise_figure`. Levels are relative and uncalibrated.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct NoiseFigureFields {
    /// The level of the present plateau of the power.
    #[field(label = "Power now", unit = "dB")]
    pub power_db: Option<f64>,
    /// The Y-factor: the mean of the kept steps.
    #[field(label = "Y-factor", unit = "dB")]
    pub y_db: Option<f64>,
    /// The Y-factor as a ratio.
    #[field(label = "Y-factor ratio")]
    pub y_lin: Option<f64>,
    /// The sample standard deviation of the kept steps.
    #[field(label = "Spread of the steps", unit = "dB")]
    pub y_spread_db: Option<f64>,
    /// The steps seen.
    #[field(label = "Steps seen")]
    pub steps: i64,
}

/// A row of the noise figure table: one source ENR of the setting.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct NoiseFigureRow {
    /// The excess noise ratio of the source.
    #[field(label = "Source ENR", unit = "dB")]
    pub enr_db: f64,
    /// `ENR - 10 log10(Y - 1)`; no value when Y is 1 or less.
    #[field(label = "Noise figure", unit = "dB")]
    pub nf_db: Option<f64>,
    /// The noise figure is negative: the ENR is lower than the step implies.
    #[field(label = "Negative")]
    pub negative: bool,
}

/// A row of the `noise_figure` log: one step.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct NoiseFigureEventRow {
    /// The signal time.
    #[field(label = "Time", unit = "s")]
    pub time_s: Seconds,
    /// The state of the source after the step.
    #[field(label = "State")]
    pub state: NoiseSourceState,
    /// The plateau before.
    #[field(label = "From", unit = "dB")]
    pub from_db: f64,
    /// The plateau after.
    #[field(label = "To", unit = "dB")]
    pub to_db: f64,
    /// The size of the step.
    #[field(label = "Y-factor", unit = "dB")]
    pub y_db: f64,
    /// The size as a ratio.
    #[field(label = "Y-factor ratio")]
    pub y_lin: f64,
}

/// The noise figure for each source ENR.
pub static NOISE_FIGURE_TABLE: Table<NoiseFigureRow> =
    Table::new("noise_figure", "Noise figure", 32);
/// The steps, newest first.
pub static NOISE_FIGURE_EVENTS: Table<NoiseFigureEventRow> =
    Table::new("events", "Steps", crate::txtest::ROWS_KEPT);

// ---- kurtosis ----

sdrkit_core::choice_enum! {
    /// The kind of a region of `kurtosis`.
    pub enum KurtosisKind {
        /// SK over the upper limit: power that comes and goes.
        Pulsed = ("pulsed", "Pulsed"),
        /// SK under the lower limit: a steady carrier.
        Steady = ("steady", "Steady"),
    }
}

sdrkit_core::choice_enum! {
    /// What a row of the `kurtosis` log reports.
    pub enum KurtosisChange {
        /// A pulsed region held for three evaluations.
        Pulsed = ("pulsed", "Pulsed"),
        /// A steady region held for three evaluations.
        Steady = ("steady", "Steady"),
        /// A reported region is no longer flagged.
        Gone = ("gone", "Gone"),
    }
}

/// The result fields of `kurtosis`, from the last evaluation.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct KurtosisFields {
    /// The share of the used bins whose SK is within the limits.
    #[field(label = "Clean bins", unit = "%")]
    pub clean_pct: Option<f64>,
    /// The expected standard deviation of SK for Gaussian noise.
    #[field(label = "Expected spread of SK")]
    pub sigma: f64,
    /// The flagged regions.
    #[field(label = "Flagged regions")]
    pub regions: Option<i64>,
    /// The time of one evaluation.
    #[field(label = "Evaluation time", unit = "ms")]
    pub block_ms: f64,
}

/// A flagged region of `kurtosis`.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct KurtosisRegionRow {
    /// The low edge.
    #[field(label = "Low edge", unit = "Hz")]
    pub low: Hertz,
    /// The high edge.
    #[field(label = "High edge", unit = "Hz")]
    pub high: Hertz,
    /// The kind.
    #[field(label = "Kind")]
    pub kind: KurtosisKind,
    /// The mean SK of its bins.
    #[field(label = "SK")]
    pub sk: f64,
    /// The share of frames at full power, `1 / (SK + 1)`, for a pulsed region with SK over 1.5.
    #[field(label = "Frames at full power", unit = "%")]
    pub full_power_pct: Option<f64>,
    /// The mean power over the noise per bin.
    #[field(label = "Over the noise", unit = "dB")]
    pub over_noise_db: f64,
}

/// A row of the `kurtosis` log.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct KurtosisEventRow {
    /// The signal time.
    #[field(label = "Time", unit = "s")]
    pub time_s: Seconds,
    /// What happened.
    #[field(label = "Kind")]
    pub kind: KurtosisChange,
    /// The centre of the region.
    #[field(label = "Centre", unit = "Hz")]
    pub centre: Hertz,
    /// The low edge.
    #[field(label = "Low edge", unit = "Hz")]
    pub low: Hertz,
    /// The high edge.
    #[field(label = "High edge", unit = "Hz")]
    pub high: Hertz,
    /// The mean SK.
    #[field(label = "SK")]
    pub sk: f64,
    /// The mean power over the noise.
    #[field(label = "Over the noise", unit = "dB")]
    pub over_noise_db: f64,
}

/// The flagged regions, the largest departure from 1 first.
pub static KURTOSIS_REGIONS: Table<KurtosisRegionRow> = Table::new("regions", "Regions", 8);
/// The changes, newest first.
pub static KURTOSIS_EVENTS: Table<KurtosisEventRow> =
    Table::new("events", "Changes", crate::txtest::ROWS_KEPT);

// ---- glitch ----

sdrkit_core::choice_enum! {
    /// The kind of a stream fault of `glitch`.
    pub enum GlitchKind {
        /// The phase of the watched tone jumped forward: samples missing.
        DroppedSamples = ("dropped_samples", "Dropped samples"),
        /// The phase jumped back: samples repeated.
        RepeatedSamples = ("repeated_samples", "Repeated samples"),
        /// A run of exactly zero samples.
        ZeroRun = ("zero_run", "Zero run"),
        /// A block equal sample for sample to the one before.
        RepeatedBlock = ("repeated_block", "Repeated block"),
        /// The level stepped and stayed.
        LevelStep = ("level_step", "Level step"),
    }
}

/// The result fields of `glitch`.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct GlitchFields {
    /// The steady tone whose phase is watched; no value while none is found.
    #[field(label = "Watched tone", unit = "Hz")]
    pub tone_hz: Option<Hertz>,
    /// Sample drop events.
    #[field(label = "Sample drops")]
    pub sample_drops: i64,
    /// Sample repeat events.
    #[field(label = "Sample repeats")]
    pub sample_repeats: i64,
    /// Zero runs.
    #[field(label = "Zero runs")]
    pub zero_runs: i64,
    /// Repeated blocks.
    #[field(label = "Repeated blocks")]
    pub repeated_blocks: i64,
    /// Level steps.
    #[field(label = "Level steps")]
    pub level_steps: i64,
    /// The time observed since the start or the last retune.
    #[field(label = "Time observed", unit = "s")]
    pub elapsed_s: Seconds,
}

/// A row of the `glitch` log: one stream fault.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct GlitchEventRow {
    /// The signal time of the fault.
    #[field(label = "Time", unit = "s")]
    pub time_s: Seconds,
    /// The kind.
    #[field(label = "Kind")]
    pub kind: GlitchKind,
    /// The samples missing or repeated: the smallest count that fits the phase step.
    #[field(label = "Samples")]
    pub samples: Option<i64>,
    /// The phase step of the tone.
    #[field(label = "Phase step", unit = "rad")]
    pub phase_step_rad: Option<f64>,
    /// The size of a level step.
    #[field(label = "Step", unit = "dB")]
    pub step_db: Option<f64>,
    /// The level before a step.
    #[field(label = "From", unit = "dB")]
    pub from_db: Option<f64>,
    /// The level after a step.
    #[field(label = "To", unit = "dB")]
    pub to_db: Option<f64>,
}

/// A stream fault of `glitch`.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct GlitchEvent {
    /// The signal time of the fault.
    #[field(label = "Time", unit = "s")]
    pub time_s: Seconds,
    /// The kind.
    #[field(label = "Kind")]
    pub kind: GlitchKind,
    /// The samples missing or repeated.
    #[field(label = "Samples")]
    pub samples: Option<i64>,
    /// The phase step of the tone.
    #[field(label = "Phase step", unit = "rad")]
    pub phase_step_rad: Option<f64>,
    /// The size of a level step.
    #[field(label = "Step", unit = "dB")]
    pub step_db: Option<f64>,
    /// The level before a step.
    #[field(label = "From", unit = "dB")]
    pub from_db: Option<f64>,
    /// The level after a step.
    #[field(label = "To", unit = "dB")]
    pub to_db: Option<f64>,
}

/// The stream faults, newest first.
pub static GLITCH_EVENTS: Table<GlitchEventRow> =
    Table::new("events", "Faults", crate::txtest::ROWS_KEPT);
/// A stream fault.
pub static GLITCH_EVENT: EventKind<GlitchEvent> = EventKind::new("glitch.event", "Stream fault");

// ---- intermod ----

sdrkit_core::choice_enum! {
    /// The kind of a product of `intermod`.
    pub enum IntermodKind {
        /// `2 f_a - f_b`, the weaker of a pair at `f_a < f_b` taken twice.
        TwoAMinusB = ("2a-b", "2a - b"),
        /// `2 f_b - f_a`.
        TwoBMinusA = ("2b-a", "2b - a"),
        /// `f_b - f_a`, second order.
        BMinusA = ("b-a", "b - a"),
    }
}

sdrkit_core::choice_enum! {
    /// Whether the level of a peak follows the intermodulation law of its parents.
    pub enum IntermodVerdict {
        /// It moves with its parents as a product must.
        Follows = ("follows", "Follows"),
        /// It does not move with them.
        Independent = ("independent", "Independent"),
        /// Neither clearly.
        Unclear = ("unclear", "Unclear"),
        /// Too few points, or the parents did not change enough.
        CannotTell = ("cannot_tell", "Cannot tell"),
    }
}

/// The result fields of `intermod`, from the last evaluation.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct IntermodFields {
    /// The strong signals used as parents.
    #[field(label = "Strong signals")]
    pub inputs: Option<i64>,
    /// The peaks at product frequencies.
    #[field(label = "Peaks at product frequencies")]
    pub products: Option<i64>,
    /// Those that follow the law.
    #[field(label = "Following the law")]
    pub products_follow: Option<i64>,
    /// Those called independent.
    #[field(label = "Independent")]
    pub products_independent: Option<i64>,
    /// Those unclear or that cannot be told.
    #[field(label = "Unknown")]
    pub products_unknown: Option<i64>,
    /// The snapshots taken.
    #[field(label = "Snapshots")]
    pub snapshots: i64,
}

/// A strong signal of `intermod`.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct IntermodInputRow {
    /// The frequency (an offset from the centre when the tuned frequency is not known).
    #[field(label = "Frequency", unit = "Hz")]
    pub frequency: Hertz,
    /// The mean level over the noise.
    #[field(label = "Level over the noise", unit = "dB")]
    pub level_db: f64,
}

/// A peak at a product frequency.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct IntermodProductRow {
    /// The frequency of the peak.
    #[field(label = "Frequency", unit = "Hz")]
    pub frequency: Hertz,
    /// The kind of product.
    #[field(label = "Kind")]
    pub kind: IntermodKind,
    /// The lower parent.
    #[field(label = "Parent a", unit = "Hz")]
    pub parent_a: Hertz,
    /// The upper parent.
    #[field(label = "Parent b", unit = "Hz")]
    pub parent_b: Hertz,
    /// The mean level under the weaker parent.
    #[field(label = "Under the weaker parent", unit = "dB")]
    pub below_db: f64,
    /// The verdict of the level fit.
    #[field(label = "Verdict")]
    pub verdict: IntermodVerdict,
    /// The slope of its level against the law.
    #[field(label = "Slope")]
    pub slope: Option<f64>,
    /// The correlation of its level with the law.
    #[field(label = "Correlation")]
    pub correlation: Option<f64>,
    /// The snapshots of the fit.
    #[field(label = "Points")]
    pub points: i64,
}

/// A row of the `intermod` log.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct IntermodEventRow {
    /// The signal time.
    #[field(label = "Time", unit = "s")]
    pub time_s: Seconds,
    /// The frequency of the peak.
    #[field(label = "Frequency", unit = "Hz")]
    pub frequency: Hertz,
    /// The kind of product.
    #[field(label = "Kind")]
    pub kind: IntermodKind,
    /// The lower parent.
    #[field(label = "Parent a", unit = "Hz")]
    pub parent_a: Hertz,
    /// The upper parent.
    #[field(label = "Parent b", unit = "Hz")]
    pub parent_b: Hertz,
    /// The mean level under the weaker parent.
    #[field(label = "Under the weaker parent", unit = "dB")]
    pub below_db: f64,
    /// The verdict.
    #[field(label = "Verdict")]
    pub verdict: IntermodVerdict,
    /// The slope.
    #[field(label = "Slope")]
    pub slope: Option<f64>,
    /// The correlation.
    #[field(label = "Correlation")]
    pub correlation: Option<f64>,
    /// The snapshots of the fit.
    #[field(label = "Points")]
    pub points: i64,
}

/// A verdict of a product that held for two evaluations.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct IntermodProductEvent {
    /// The signal time.
    #[field(label = "Time", unit = "s")]
    pub time_s: Seconds,
    /// The frequency of the peak.
    #[field(label = "Frequency", unit = "Hz")]
    pub frequency: Hertz,
    /// The kind of product.
    #[field(label = "Kind")]
    pub kind: IntermodKind,
    /// The lower parent.
    #[field(label = "Parent a", unit = "Hz")]
    pub parent_a: Hertz,
    /// The upper parent.
    #[field(label = "Parent b", unit = "Hz")]
    pub parent_b: Hertz,
    /// The mean level under the weaker parent.
    #[field(label = "Under the weaker parent", unit = "dB")]
    pub below_db: f64,
    /// The verdict.
    #[field(label = "Verdict")]
    pub verdict: IntermodVerdict,
    /// The slope.
    #[field(label = "Slope")]
    pub slope: Option<f64>,
    /// The correlation.
    #[field(label = "Correlation")]
    pub correlation: Option<f64>,
    /// The snapshots of the fit.
    #[field(label = "Points")]
    pub points: i64,
}

/// The strong signals, the strongest first.
pub static INTERMOD_INPUTS: Table<IntermodInputRow> = Table::new("inputs", "Strong signals", 16);
/// The peaks at product frequencies, the least under their parents first.
pub static INTERMOD_PRODUCTS: Table<IntermodProductRow> = Table::new("products", "Products", 200);
/// The verdicts that held, newest first.
pub static INTERMOD_EVENTS: Table<IntermodEventRow> =
    Table::new("events", "Verdicts", crate::txtest::ROWS_KEPT);
/// A verdict of a product held for two evaluations.
pub static INTERMOD_PRODUCT: EventKind<IntermodProductEvent> =
    EventKind::new("intermod.product", "Product");

// ---- spur ----

sdrkit_core::choice_enum! {
    /// The kind of a finding of `spur`.
    pub enum SpurKind {
        /// The mirror of a strong signal at `-f`.
        Image = ("image", "Image"),
        /// The second harmonic, folded into the span.
        SecondHarmonic = ("second_harmonic", "2nd harmonic"),
        /// The third harmonic, folded into the span.
        ThirdHarmonic = ("third_harmonic", "3rd harmonic"),
        /// A steady peak at a multiple of the rate over the grid.
        FixedSpur = ("fixed_spur", "Fixed spur"),
    }
}

sdrkit_core::choice_enum! {
    /// What a row of the `spur` log reports.
    pub enum SpurEventKind {
        /// An image.
        Image = ("image", "Image"),
        /// A second harmonic.
        SecondHarmonic = ("second_harmonic", "2nd harmonic"),
        /// A third harmonic.
        ThirdHarmonic = ("third_harmonic", "3rd harmonic"),
        /// A fixed spur.
        FixedSpur = ("fixed_spur", "Fixed spur"),
        /// The DC spike rose over its threshold.
        Dc = ("dc", "DC spike"),
    }
}

sdrkit_core::choice_enum! {
    /// Whether a finding of `spur` follows its signal.
    pub enum SpurVerdict {
        /// Its level moves with its signal as an image or a harmonic must.
        Follows = ("follows", "Follows"),
        /// It does not move with its signal: a real signal, not a spur.
        Independent = ("independent", "Independent"),
        /// Neither clearly.
        Unclear = ("unclear", "Unclear"),
        /// Too few points, or the signal did not change enough.
        CannotTell = ("cannot_tell", "Cannot tell"),
        /// A steady peak on the spur grid.
        Fixed = ("fixed", "Fixed"),
    }
}

/// The result fields of `spur`, from the last evaluation. The findings hold for this radio at this gain, rate and tuning.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct SpurFields {
    /// The largest bin within 2 bins of 0 Hz over the noise.
    #[field(label = "DC spike over the noise", unit = "dB")]
    pub dc_db: Option<f64>,
    /// The DC spike is over its threshold.
    #[field(label = "DC spike present")]
    pub dc_spike: Option<bool>,
    /// The strong signals.
    #[field(label = "Strong signals")]
    pub signals: Option<i64>,
    /// Images not called independent.
    #[field(label = "Images")]
    pub images: Option<i64>,
    /// Harmonics not called independent.
    #[field(label = "Harmonics")]
    pub harmonics: Option<i64>,
    /// Fixed spurs.
    #[field(label = "Fixed spurs")]
    pub fixed_spurs: Option<i64>,
    /// The snapshots taken.
    #[field(label = "Snapshots")]
    pub snapshots: i64,
}

/// A finding of `spur`.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct SpurFindingRow {
    /// The kind.
    #[field(label = "Kind")]
    pub kind: SpurKind,
    /// The frequency of the peak.
    #[field(label = "Frequency", unit = "Hz")]
    pub frequency: Hertz,
    /// The signal that it belongs to; none for a fixed spur.
    #[field(label = "From signal", unit = "Hz")]
    pub from_signal: Option<Hertz>,
    /// The mean level under the signal; for a fixed spur the mean level over the noise.
    #[field(label = "Under the signal", unit = "dB")]
    pub below_db: f64,
    /// The verdict.
    #[field(label = "Verdict")]
    pub verdict: SpurVerdict,
    /// The slope of its level against the signal's.
    #[field(label = "Slope")]
    pub slope: Option<f64>,
    /// The correlation.
    #[field(label = "Correlation")]
    pub correlation: Option<f64>,
    /// The image rejection (images).
    #[field(label = "Image rejection", unit = "dB")]
    pub image_rejection_db: Option<f64>,
    /// The gain mismatch alone that gives this image.
    #[field(label = "Gain mismatch alone", unit = "dB")]
    pub gain_mismatch_db: Option<f64>,
    /// The phase mismatch alone that gives this image.
    #[field(label = "Phase mismatch alone", unit = "deg")]
    pub phase_mismatch_deg: Option<f64>,
}

/// A row of the `spur` log.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct SpurEventRow {
    /// The signal time.
    #[field(label = "Time", unit = "s")]
    pub time_s: Seconds,
    /// The kind.
    #[field(label = "Kind")]
    pub kind: SpurEventKind,
    /// The frequency of the peak.
    #[field(label = "Frequency", unit = "Hz")]
    pub frequency: Hertz,
    /// The signal that it belongs to.
    #[field(label = "From signal", unit = "Hz")]
    pub from_signal: Option<Hertz>,
    /// The mean level under the signal, or over the noise.
    #[field(label = "Under the signal", unit = "dB")]
    pub below_db: Option<f64>,
    /// The verdict.
    #[field(label = "Verdict")]
    pub verdict: Option<SpurVerdict>,
    /// The slope.
    #[field(label = "Slope")]
    pub slope: Option<f64>,
    /// The correlation.
    #[field(label = "Correlation")]
    pub correlation: Option<f64>,
    /// The image rejection.
    #[field(label = "Image rejection", unit = "dB")]
    pub image_rejection_db: Option<f64>,
    /// The gain mismatch alone.
    #[field(label = "Gain mismatch alone", unit = "dB")]
    pub gain_mismatch_db: Option<f64>,
    /// The phase mismatch alone.
    #[field(label = "Phase mismatch alone", unit = "deg")]
    pub phase_mismatch_deg: Option<f64>,
    /// The DC spike over the noise (kind dc).
    #[field(label = "DC spike over the noise", unit = "dB")]
    pub dc_db: Option<f64>,
}

/// The findings of the last evaluation.
pub static SPUR_FINDINGS: Table<SpurFindingRow> = Table::new("findings", "Findings", 200);
/// The verdicts that held and the DC spike, newest first.
pub static SPUR_EVENTS: Table<SpurEventRow> =
    Table::new("events", "Findings log", crate::txtest::ROWS_KEPT);

// ---- fading ----

sdrkit_core::choice_enum! {
    /// What a row of the `fading` log reports.
    pub enum FadingEventKind {
        /// The carrier was found.
        Found = ("found", "Found"),
        /// The carrier was lost; the statistics start again.
        Lost = ("lost", "Lost"),
        /// A fade deeper or longer than the log limits.
        Fade = ("fade", "Fade"),
        /// A reading of the statistics.
        Reading = ("reading", "Reading"),
    }
}

/// The result fields of `fading`: the statistics of the envelope of one carrier over the window, and the carrier now.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct FadingFields {
    /// The Rician K-factor; 40 dB for no fading, no value for K = 0 (Rayleigh).
    #[field(label = "Rician K-factor", unit = "dB")]
    pub k_db: Option<f64>,
    /// The K-factor as a ratio.
    #[field(label = "Rician K-factor ratio")]
    pub k: Option<f64>,
    /// No steady component: K = 0.
    #[field(label = "Rayleigh")]
    pub rayleigh: Option<bool>,
    /// The level exceeded by 99 % of the time, against the mean power.
    #[field(label = "Level at 1 %", unit = "dB")]
    pub p1_db: Option<f64>,
    /// The level at 10 %.
    #[field(label = "Level at 10 %", unit = "dB")]
    pub p10_db: Option<f64>,
    /// The median level.
    #[field(label = "Level at 50 %", unit = "dB")]
    pub p50_db: Option<f64>,
    /// The level at 90 %.
    #[field(label = "Level at 90 %", unit = "dB")]
    pub p90_db: Option<f64>,
    /// The level at 99 %.
    #[field(label = "Level at 99 %", unit = "dB")]
    pub p99_db: Option<f64>,
    /// The median over the 1 % level.
    #[field(label = "Fade depth", unit = "dB")]
    pub fade_depth_db: Option<f64>,
    /// The scintillation index S4.
    #[field(label = "Scintillation index")]
    pub s4: Option<f64>,
    /// Downward crossings of the fade threshold per second.
    #[field(label = "Fade rate", unit = "1/s")]
    pub fade_rate_hz: Option<f64>,
    /// The average time under the fade threshold.
    #[field(label = "Average fade length", unit = "ms")]
    pub fade_length_ms: Option<f64>,
    /// The RMS Doppler spread.
    #[field(label = "Doppler spread", unit = "Hz")]
    pub doppler_hz: Option<f64>,
    /// The carrier to noise density of the last block.
    #[field(label = "C/N0", unit = "dB-Hz")]
    pub cn0_dbhz: Option<f64>,
    /// The carrier from the tuned frequency.
    #[field(label = "Carrier", unit = "Hz")]
    pub carrier_offset_hz: Option<Hertz>,
    /// The envelope kept.
    #[field(label = "Envelope kept", unit = "s")]
    pub window_s: Option<Seconds>,
}

/// A row of the `fading` log.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct FadingEventRow {
    /// The signal time.
    #[field(label = "Time", unit = "s")]
    pub time_s: Seconds,
    /// What happened.
    #[field(label = "Kind")]
    pub kind: FadingEventKind,
    /// The deepest level of a fade against the mean power.
    #[field(label = "Depth", unit = "dB")]
    pub depth_db: Option<f64>,
    /// The length of a fade.
    #[field(label = "Length", unit = "ms")]
    pub length_ms: Option<f64>,
    /// The K-factor.
    #[field(label = "Rician K-factor", unit = "dB")]
    pub k_db: Option<f64>,
    /// The level at 10 %.
    #[field(label = "Level at 10 %", unit = "dB")]
    pub p10_db: Option<f64>,
    /// The fade rate.
    #[field(label = "Fade rate", unit = "1/s")]
    pub fade_rate_hz: Option<f64>,
    /// The average fade length.
    #[field(label = "Average fade length", unit = "ms")]
    pub fade_length_ms: Option<f64>,
    /// The Doppler spread.
    #[field(label = "Doppler spread", unit = "Hz")]
    pub doppler_hz: Option<f64>,
    /// The carrier to noise density.
    #[field(label = "C/N0", unit = "dB-Hz")]
    pub cn0_dbhz: Option<f64>,
}

/// The carrier changes, the deep and long fades and the readings, newest first.
pub static FADING_EVENTS: Table<FadingEventRow> =
    Table::new("events", "Log", crate::txtest::ROWS_KEPT);

// ---- beacon_monitor ----

sdrkit_core::choice_enum! {
    /// The class of the path of a beacon.
    pub enum BeaconPath {
        /// Fewer than 3 spectra of a decision held the line.
        None = ("none", "None"),
        /// A narrow line of steady level.
        Steady = ("steady", "Steady"),
        /// A narrow line whose level swings.
        Fading = ("fading", "Fading"),
        /// A line smeared over tens of Hz or more.
        AuroraLike = ("aurora_like", "Aurora-like"),
    }
}

/// The result fields of `beacon_monitor`.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct BeaconMonitorFields {
    /// The class of the path.
    #[field(label = "Path")]
    pub class: BeaconPath,
    /// The line over the noise in a bin, last spectrum.
    #[field(label = "Beacon over the noise in a bin", unit = "dB")]
    pub level_db: Option<f64>,
    /// The Doppler spread of the line (a Gaussian full width at half height), last spectrum.
    #[field(label = "Doppler spread of the line", unit = "Hz")]
    pub spread_hz: Option<f64>,
    /// The largest minus the smallest level of the last 12 s, at the last decision.
    #[field(label = "Level swing over the last 12 s", unit = "dB")]
    pub swing_db: Option<f64>,
    /// The line from the tuned frequency, last spectrum.
    #[field(label = "Line from the tuned frequency", unit = "Hz")]
    pub line_offset_hz: Option<Hertz>,
    /// The class changes.
    #[field(label = "Class changes")]
    pub changes: i64,
}

/// A change of class of `beacon_monitor`.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct BeaconMonitorEventRow {
    /// The signal time.
    #[field(label = "Time", unit = "s")]
    pub time_s: Seconds,
    /// The class before.
    #[field(label = "From")]
    pub from_class: BeaconPath,
    /// The class after.
    #[field(label = "To")]
    pub to_class: BeaconPath,
    /// The last level.
    #[field(label = "Level over the noise", unit = "dB")]
    pub level_db: Option<f64>,
    /// The median spread of the decision.
    #[field(label = "Spread", unit = "Hz")]
    pub spread_hz: Option<f64>,
    /// The swing of the decision.
    #[field(label = "Swing", unit = "dB")]
    pub swing_db: Option<f64>,
}

/// A change of class of the path.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct BeaconMonitorChangeEvent {
    /// The signal time.
    #[field(label = "Time", unit = "s")]
    pub time_s: Seconds,
    /// The class before.
    #[field(label = "From")]
    pub from_class: BeaconPath,
    /// The class after.
    #[field(label = "To")]
    pub to_class: BeaconPath,
    /// The last level.
    #[field(label = "Level over the noise", unit = "dB")]
    pub level_db: Option<f64>,
    /// The median spread of the decision.
    #[field(label = "Spread", unit = "Hz")]
    pub spread_hz: Option<f64>,
    /// The swing of the decision.
    #[field(label = "Swing", unit = "dB")]
    pub swing_db: Option<f64>,
}

/// The class changes, newest first.
pub static BEACON_MONITOR_EVENTS: Table<BeaconMonitorEventRow> =
    Table::new("events", "Class changes", crate::txtest::ROWS_KEPT);
/// A change of class.
pub static BEACON_MONITOR_CHANGE: EventKind<BeaconMonitorChangeEvent> =
    EventKind::new("beacon_monitor.change", "Path change");

/// The interface of the family.
pub fn family() -> &'static Family {
    static FAMILY: OnceLock<Family> = OnceLock::new();
    FAMILY.get_or_init(|| {
        let decoder_clear = &sdrkit_graph::decoder::CLEAR;
        let watcher_clear = &sdrkit_graph::decoder::CLEAR;
        Family::new("meter", 1, 1)
            .added(
                1,
                1,
                &[
                    "fm_monitor.content",
                    "am_monitor.content",
                    "occupancy.content",
                    "noise_floor.content",
                    "burst_finder.content",
                    "hopper.content",
                    "birdie.content",
                    "obw.content",
                    "ptt_log.content",
                    "noise_locator.content",
                    "freq_accuracy.content",
                    "sinad.content",
                    "nfm_meter.content",
                    "tx_stability.content",
                    "ssb_test.content",
                    "afsk_tuner.content",
                    "phase_noise.content",
                    "cw_keying.content",
                    "fsk_analyser.content",
                    "mer.content",
                    "papr.content",
                    "input_health.content",
                    "noise_figure.content",
                    "kurtosis.content",
                    "glitch.content",
                    "intermod.content",
                    "spur.content",
                    "fading.content",
                    "beacon_monitor.content",
                ],
            )
            .content(
                sdrkit_core::ContentKind::Measurement,
                &[
                    "fm_monitor",
                    "am_monitor",
                    "occupancy",
                    "noise_floor",
                    "burst_finder",
                    "hopper",
                    "birdie",
                    "obw",
                    "ptt_log",
                    "noise_locator",
                    "freq_accuracy",
                    "sinad",
                    "nfm_meter",
                    "tx_stability",
                    "ssb_test",
                    "afsk_tuner",
                    "phase_noise",
                    "cw_keying",
                    "fsk_analyser",
                    "mer",
                    "papr",
                    "input_health",
                    "noise_figure",
                    "kurtosis",
                    "glitch",
                    "intermod",
                    "spur",
                    "fading",
                    "beacon_monitor",
                ],
            )
            .stack(
                StackShape::new(crate::fm_monitor::ID)
                    .fields::<FmMonitorFields>()
                    .fields::<DecoderFields>()
                    .table(&FM_MONITOR_LOG)
                    .event(&FM_MONITOR_PILOT)
                    .event(&FM_MONITOR_RDS)
                    .event(&FM_MONITOR_SCA)
                    .event(&FM_MONITOR_OVER_DEVIATION)
                    .settings(crate::fm_monitor::FmMonitorSetting::SETTINGS)
                    .command(decoder_clear),
            )
            .stack(
                StackShape::new(crate::am_monitor::ID)
                    .fields::<AmMonitorFields>()
                    .fields::<DecoderFields>()
                    .table(&AM_MONITOR_LOG)
                    .event(&AM_MONITOR_CARRIER)
                    .event(&AM_MONITOR_OVER_MODULATION)
                    .settings(crate::am_monitor::AmMonitorSetting::SETTINGS)
                    .command(decoder_clear),
            )
            .stack(
                StackShape::new(crate::occupancy::ID)
                    .fields::<WatchFields>()
                    .fields::<OccupancyFields>()
                    .fields::<DecoderFields>()
                    .table(&OCCUPANCY_CHANNELS)
                    .table(&OCCUPANCY_ACTIVITY)
                    .event(&OCCUPANCY_ACTIVITY_EVENT)
                    .settings(crate::occupancy::OccupancySetting::SETTINGS)
                    .command(watcher_clear)
                    .command(&crate::occupancy::RESET),
            )
            .stack(
                StackShape::new(crate::noise_floor::ID)
                    .fields::<WatchFields>()
                    .fields::<NoiseFloorFields>()
                    .fields::<DecoderFields>()
                    .table(&NOISE_FLOOR_SUB_BANDS)
                    .table(&NOISE_FLOOR_EVENTS)
                    .event(&NOISE_FLOOR_EVENT)
                    .settings(crate::noise_floor::NoiseFloorSetting::SETTINGS)
                    .command(watcher_clear)
                    .command(&crate::noise_floor::RESET),
            )
            .stack(
                StackShape::new(crate::burst_finder::ID)
                    .fields::<WatchFields>()
                    .fields::<BurstFinderFields>()
                    .fields::<DecoderFields>()
                    .table(&BURST_FINDER_BURSTS)
                    .table(&BURST_FINDER_REPEATING)
                    .event(&BURST_FOUND)
                    .event(&BURST_REPEATING)
                    .settings(crate::burst_finder::BurstFinderSetting::SETTINGS)
                    .command(watcher_clear)
                    .command(&crate::burst_finder::RESET),
            )
            .stack(
                StackShape::new(crate::hopper::ID)
                    .fields::<WatchFields>()
                    .fields::<HopperFields>()
                    .fields::<DecoderFields>()
                    .table(&HOPPER_HOPPERS)
                    .event(&HOPPER_CHANGED)
                    .settings(crate::hopper::HopperSetting::SETTINGS)
                    .command(watcher_clear)
                    .command(&crate::hopper::RESET),
            )
            .stack(
                StackShape::new(crate::birdie::ID)
                    .fields::<WatchFields>()
                    .fields::<BirdieFields>()
                    .fields::<DecoderFields>()
                    .table(&BIRDIE_CARRIERS)
                    .table(&BIRDIE_CARRIER_EVENTS)
                    .event(&BIRDIE_EVENT)
                    .settings(crate::birdie::BirdieSetting::SETTINGS)
                    .command(watcher_clear)
                    .command(&crate::birdie::RESET),
            )
            .stack(
                StackShape::new(crate::obw::ID)
                    .fields::<WatchFields>()
                    .fields::<ObwFields>()
                    .fields::<DecoderFields>()
                    .table(&OBW_BANDWIDTHS)
                    .event(&OBW_READING)
                    .settings(crate::obw::ObwSetting::SETTINGS)
                    .command(watcher_clear)
                    .command(&crate::obw::RESET),
            )
            .stack(
                StackShape::new(crate::ptt_log::ID)
                    .fields::<WatchFields>()
                    .fields::<PttLogFields>()
                    .fields::<DecoderFields>()
                    .table(&PTT_LOG_TRANSMISSIONS)
                    .event(&PTT_TRANSMISSION)
                    .settings(crate::ptt_log::PttLogSetting::SETTINGS)
                    .command(watcher_clear)
                    .command(&crate::ptt_log::RESET),
            )
            .stack(
                StackShape::new(crate::noise_locator::ID)
                    .fields::<WatchFields>()
                    .fields::<NoiseLocatorFields>()
                    .fields::<DecoderFields>()
                    .table(&NOISE_LOCATOR_PATTERNS)
                    .event(&NOISE_LOCATOR_PATTERN)
                    .settings(crate::noise_locator::NoiseLocatorSetting::SETTINGS)
                    .command(watcher_clear)
                    .command(&crate::noise_locator::RESET)
                    .kept(&["patterns"]),
            )
            .stack(
                StackShape::new(crate::freq_accuracy::ID)
                    .fields::<WatchFields>()
                    .fields::<FreqAccuracyFields>()
                    .fields::<DecoderFields>()
                    .table(&FREQ_ACCURACY_ALLAN)
                    .table(&FREQ_ACCURACY_EVENTS)
                    .event(&FREQ_ACCURACY_EVENT)
                    .settings(crate::freq_accuracy::FreqAccuracySetting::SETTINGS)
                    .command(watcher_clear)
                    .command(&crate::txtest::RESET),
            )
            .stack(
                StackShape::new(crate::sinad::ID)
                    .fields::<WatchFields>()
                    .fields::<SinadFields>()
                    .fields::<DecoderFields>()
                    .table(&SINAD_EVENTS)
                    .event(&SINAD_CROSSING)
                    .settings(crate::sinad::SinadSetting::SETTINGS)
                    .command(watcher_clear)
                    .command(&crate::txtest::RESET),
            )
            .stack(
                StackShape::new(crate::nfm_meter::ID)
                    .fields::<WatchFields>()
                    .fields::<NfmMeterFields>()
                    .fields::<DecoderFields>()
                    .table(&NFM_METER_EVENTS)
                    .settings(crate::nfm_meter::NfmMeterSetting::SETTINGS)
                    .command(watcher_clear)
                    .command(&crate::txtest::RESET),
            )
            .stack(
                StackShape::new(crate::tx_stability::ID)
                    .fields::<WatchFields>()
                    .fields::<TxStabilityFields>()
                    .fields::<DecoderFields>()
                    .table(&TX_STABILITY_TRANSMISSIONS)
                    .event(&TX_STABILITY_TRANSMISSION)
                    .settings(crate::tx_stability::TxStabilitySetting::SETTINGS)
                    .command(watcher_clear)
                    .command(&crate::txtest::RESET),
            )
            .stack(
                StackShape::new(crate::ssb_test::ID)
                    .fields::<WatchFields>()
                    .fields::<SsbTestFields>()
                    .fields::<DecoderFields>()
                    .table(&SSB_TEST_EVENTS)
                    .settings(crate::ssb_test::SsbTestSetting::SETTINGS)
                    .command(watcher_clear)
                    .command(&crate::txtest::RESET),
            )
            .stack(
                StackShape::new(crate::afsk_tuner::ID)
                    .fields::<WatchFields>()
                    .fields::<AfskTunerFields>()
                    .fields::<DecoderFields>()
                    .table(&AFSK_TUNER_BURSTS)
                    .event(&AFSK_TUNER_BURST)
                    .settings(crate::afsk_tuner::AfskTunerSetting::SETTINGS)
                    .command(watcher_clear)
                    .command(&crate::txtest::RESET),
            )
            .stack(
                StackShape::new(crate::phase_noise::ID)
                    .fields::<WatchFields>()
                    .fields::<PhaseNoiseFields>()
                    .fields::<DecoderFields>()
                    .table(&PHASE_NOISE_SPURS)
                    .table(&PHASE_NOISE_EVENTS)
                    .settings(crate::phase_noise::PhaseNoiseSetting::SETTINGS)
                    .command(watcher_clear)
                    .command(&crate::txtest::RESET),
            )
            .stack(
                StackShape::new(crate::cw_keying::ID)
                    .fields::<WatchFields>()
                    .fields::<CwKeyingFields>()
                    .fields::<DecoderFields>()
                    .table(&CW_KEYING_EVENTS)
                    .settings(crate::cw_keying::CwKeyingSetting::SETTINGS)
                    .command(watcher_clear)
                    .command(&crate::txtest::RESET),
            )
            .stack(
                StackShape::new(crate::fsk_analyser::ID)
                    .fields::<WatchFields>()
                    .fields::<FskAnalyserFields>()
                    .fields::<DecoderFields>()
                    .table(&FSK_ANALYSER_LEVEL_FREQS)
                    .table(&FSK_ANALYSER_EVENTS)
                    .settings(crate::fsk_analyser::FskAnalyserSetting::SETTINGS)
                    .command(watcher_clear)
                    .command(&crate::txtest::RESET),
            )
            .stack(
                StackShape::new(crate::mer::ID)
                    .fields::<WatchFields>()
                    .fields::<MerFields>()
                    .fields::<DecoderFields>()
                    .table(&MER_EVENTS)
                    .settings(crate::mer::MerSetting::SETTINGS)
                    .command(watcher_clear)
                    .command(&crate::txtest::RESET),
            )
            .stack(
                StackShape::new(crate::papr::ID)
                    .fields::<WatchFields>()
                    .fields::<PaprFields>()
                    .fields::<DecoderFields>()
                    .table(&PAPR_EVENTS)
                    .settings(crate::papr::PaprSetting::SETTINGS)
                    .command(watcher_clear)
                    .command(&crate::txtest::RESET),
            )
            .stack(
                StackShape::new(crate::input_health::ID)
                    .fields::<WatchFields>()
                    .fields::<InputHealthFields>()
                    .fields::<DecoderFields>()
                    .table(&INPUT_HEALTH_EVENTS)
                    .settings(crate::input_health::InputHealthSetting::SETTINGS)
                    .command(watcher_clear)
                    .command(&crate::txtest::RESET),
            )
            .stack(
                StackShape::new(crate::noise_figure::ID)
                    .fields::<WatchFields>()
                    .fields::<NoiseFigureFields>()
                    .fields::<DecoderFields>()
                    .table(&NOISE_FIGURE_TABLE)
                    .table(&NOISE_FIGURE_EVENTS)
                    .settings(crate::noise_figure::NoiseFigureSetting::SETTINGS)
                    .command(watcher_clear)
                    .command(&crate::txtest::RESET),
            )
            .stack(
                StackShape::new(crate::kurtosis::ID)
                    .fields::<WatchFields>()
                    .fields::<KurtosisFields>()
                    .fields::<DecoderFields>()
                    .table(&KURTOSIS_REGIONS)
                    .table(&KURTOSIS_EVENTS)
                    .settings(crate::kurtosis::KurtosisSetting::SETTINGS)
                    .command(watcher_clear)
                    .command(&crate::txtest::RESET),
            )
            .stack(
                StackShape::new(crate::glitch::ID)
                    .fields::<WatchFields>()
                    .fields::<GlitchFields>()
                    .fields::<DecoderFields>()
                    .table(&GLITCH_EVENTS)
                    .event(&GLITCH_EVENT)
                    .settings(crate::glitch::GlitchSetting::SETTINGS)
                    .command(watcher_clear)
                    .command(&crate::txtest::RESET)
                    .kept(&[
                        "events",
                        "GlitchFields.sample_drops",
                        "GlitchFields.sample_repeats",
                        "GlitchFields.zero_runs",
                        "GlitchFields.repeated_blocks",
                        "GlitchFields.level_steps",
                    ]),
            )
            .stack(
                StackShape::new(crate::intermod::ID)
                    .fields::<WatchFields>()
                    .fields::<IntermodFields>()
                    .fields::<DecoderFields>()
                    .table(&INTERMOD_INPUTS)
                    .table(&INTERMOD_PRODUCTS)
                    .table(&INTERMOD_EVENTS)
                    .event(&INTERMOD_PRODUCT)
                    .settings(crate::intermod::IntermodSetting::SETTINGS)
                    .command(watcher_clear)
                    .command(&crate::txtest::RESET),
            )
            .stack(
                StackShape::new(crate::spur::ID)
                    .fields::<WatchFields>()
                    .fields::<SpurFields>()
                    .fields::<DecoderFields>()
                    .table(&SPUR_FINDINGS)
                    .table(&SPUR_EVENTS)
                    .settings(crate::spur::SpurSetting::SETTINGS)
                    .command(watcher_clear)
                    .command(&crate::txtest::RESET),
            )
            .stack(
                StackShape::new(crate::fading::ID)
                    .fields::<WatchFields>()
                    .fields::<FadingFields>()
                    .fields::<DecoderFields>()
                    .table(&FADING_EVENTS)
                    .settings(crate::fading::FadingSetting::SETTINGS)
                    .command(watcher_clear)
                    .command(&crate::txtest::RESET),
            )
            .stack(
                StackShape::new(crate::beacon_monitor::ID)
                    .fields::<WatchFields>()
                    .fields::<BeaconMonitorFields>()
                    .fields::<DecoderFields>()
                    .table(&BEACON_MONITOR_EVENTS)
                    .event(&BEACON_MONITOR_CHANGE)
                    .settings(crate::beacon_monitor::BeaconMonitorSetting::SETTINGS)
                    .command(watcher_clear)
                    .command(&crate::txtest::RESET),
            )
    })
}

/// The description of stack `id` of the family.
///
/// # Panics
///
/// If the family does not describe the stack.
pub fn shape(id: &str) -> &'static StackShape {
    family()
        .stack_shape(id)
        .unwrap_or_else(|| panic!("the meter family does not describe the stack {id}"))
}
