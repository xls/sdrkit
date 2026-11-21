//! The interface of the STL stack: its own result fields, the table of link changes and their event, beside the
//! reception fields, the RDS tables and the events of the FM components that it runs.

use crate::typed::{
    RdsPart, AF, EON, EON_AF, LABEL_TAGS, MPX, ODA, RDS_SYMBOLS, STATION_NAMED,
    TEXT_DONE, TMC,
};
use sdrkit_core::interface::{EventKind, Fields, Hertz, Seconds, StackShape, Table};
use sdrkit_core::publish::keys;
use sdrkit_core::SettingSet;
use sdrkit_proto_bcdata::publish::LabelFields;

sdrkit_core::choice_enum! {
    /// What a link is.
    #[derive(Default)]
    pub enum LinkKind {
        /// No signal stands out.
        #[default]
        Nothing = ("nothing", "Nothing"),
        /// An FM carrier with the stereo multiplex (pilot locked).
        Composite = ("composite", "Composite (stereo multiplex)"),
        /// An FM carrier without a pilot: programme audio, or a mono multiplex.
        Mono = ("mono", "Mono"),
        /// No FM carrier; a wide, flat signal: a digital link, measured and not decoded.
        DigitalUnknown = ("digital_unknown", "Digital or unknown (not decoded)"),
        /// No FM carrier; a signal that is narrow or not flat.
        Unknown = ("unknown", "Unknown (not FM)"),
    }
}

/// The result fields of the STL link, beside the reception fields of the FM components.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct StlFields {
    /// What the link is. A change holds after two reports of 0.5 s in a row.
    #[field(label = "Link")]
    pub link: LinkKind,
    /// The peak deviation over the last 0.5 s, noise included. No value without a carrier.
    #[field(label = "Peak deviation", unit = "Hz")]
    pub peak_deviation_hz: Option<Hertz>,
    /// The peak deviation against the deviation of full modulation. No value without a carrier.
    #[field(label = "Modulation", unit = "%")]
    pub modulation_pct: Option<f64>,
    /// The pilot level, as deviation. No value while the pilot is not locked.
    #[field(label = "Pilot level", unit = "Hz")]
    pub pilot_hz: Option<Hertz>,
    /// The channel width follows the link (the automatic width).
    #[field(label = "Automatic width")]
    pub width_auto: bool,
    /// The spectrum of the capture reaches far enough past the channel to measure the floor. Without it the digital check
    /// is off.
    #[field(label = "Floor measured")]
    pub floor_measured: bool,
    /// The width of the occupied region. No value while none stands out.
    #[field(label = "Occupied width", unit = "Hz")]
    pub occupied_width_hz: Option<Hertz>,
    /// The mean of the occupied region over the floor.
    #[field(label = "Over the floor", unit = "dB")]
    pub over_floor_db: Option<f64>,
    /// The flatness of the occupied region: its 10th percentile against its mean (0 dB is flat).
    #[field(label = "Flatness", unit = "dB")]
    pub flatness_db: Option<f64>,
    /// The SNR of the SCA subcarrier at 67 kHz. No value while it is absent.
    #[field(label = "SCA 67 kHz", unit = "dB")]
    pub sca67_snr_db: Option<f64>,
    /// The SNR of the SCA subcarrier at 92 kHz. No value while it is absent.
    #[field(label = "SCA 92 kHz", unit = "dB")]
    pub sca92_snr_db: Option<f64>,
    /// The peak of the sound over the last 0.5 s. No value for silence.
    #[field(label = "Sound peak", unit = "dBFS")]
    pub sound_peak_dbfs: Option<f64>,
    /// The RMS of the sound over the last 0.5 s. No value for silence.
    #[field(label = "Sound RMS", unit = "dBFS")]
    pub sound_rms_dbfs: Option<f64>,
}

/// A held change of the link. The row key is the count of changes (`0001`).
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct LinkRow<'a> {
    /// The signal time of the change.
    #[field(label = "Time", unit = "s")]
    pub time_s: Seconds,
    /// What the link became.
    #[field(label = "Link")]
    pub link: LinkKind,
    /// The details: pilot, peak deviation and offset of an FM link, or level and width of a signal that is not FM.
    #[field(shared = keys::TEXT)]
    pub text: &'a str,
    /// The occupied region over the floor. No value without a region.
    #[field(label = "Over the floor", unit = "dB")]
    pub over_floor_db: Option<f64>,
}

/// The held changes of the link.
pub static LINKS: Table<LinkRow<'static>> = Table::new("links", "Link changes", 64);

/// The fields of a change of the link.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct LinkEvent<'a> {
    /// What the link became.
    #[field(label = "Link")]
    pub link: LinkKind,
    /// The details, as in the table of changes.
    #[field(shared = keys::TEXT)]
    pub text: &'a str,
    /// The occupied region over the floor. No value without a region.
    #[field(label = "Over the floor", unit = "dB")]
    pub over_floor_db: Option<f64>,
}

/// The link changed, and the change held for two reports.
pub static LINK_CHANGED: EventKind<LinkEvent<'static>> =
    EventKind::new("stl.link", "STL link changed");

/// The interface of the STL stack, as the family lists it.
pub fn shape() -> StackShape {
    StackShape::new(super::ID)
        .fields::<StlFields>()
        .fields::<RdsPart>()
        .fields::<LabelFields>()
        .table(&LINKS)
        .table(&AF)
        .table(&LABEL_TAGS)
        .table(&ODA)
        .table(&EON)
        .table(&EON_AF)
        .table(&TMC)
        .series(MPX)
        .series(RDS_SYMBOLS)
        .event(&LINK_CHANGED)
        .event(&STATION_NAMED)
        .event(&TEXT_DONE)
        .settings(super::settings::StlSetting::SETTINGS)
        .command(&crate::graph::CLEAR)
}

/// The interface description of the STL stack.
///
/// # Panics
///
/// If the family does not hold the stack.
pub fn stack_shape() -> &'static StackShape {
    crate::typed::family()
        .stack_shape(super::ID)
        .expect("the family holds the STL stack")
}
