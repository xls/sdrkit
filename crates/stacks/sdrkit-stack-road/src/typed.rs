//! The interface of the road family: the shapes of the result sets, the table rows and the events, the tables, the
//! event types, the commands and what each stack declares.

use crate::v2x::gen::V2xSetting;
use crate::v2x::FrameKind;
use sdrkit_core::interface::{
    Arguments, CommandKind, EventKind, Family as Described, Fields, Hertz, StackShape, Table, Time,
};
use sdrkit_core::publish::keys::{self, Family};
use sdrkit_core::publish::field;
use sdrkit_core::settings::Argument;
use sdrkit_core::SettingSet;
use sdrkit_graph::DecoderFields;
use sdrkit_util_objects::commands as objcmd;
use sdrkit_util_objects::publish::columns;
use std::sync::OnceLock;

/// The result fields of V2X.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct V2xFields<'a> {
    /// Frames (packets, blocks, bursts) that passed their check.
    #[field(shared = keys::FRAMES_OK)]
    pub frames_ok: i64,
    /// Frames that failed their check.
    #[field(shared = keys::FRAMES_BAD)]
    pub frames_bad: i64,
    /// Safety messages (BSM and CAM).
    #[field(label = "Safety messages")]
    pub safety_messages: i64,
    /// DENM frames, counted.
    #[field(label = "Hazard warnings")]
    pub denm: i64,
    /// Vehicles in the table.
    #[field(label = "Vehicles")]
    pub vehicles: i64,
    /// Short training fields detected.
    #[field(label = "Preambles")]
    pub preambles: i64,
    /// Level of the signal or channel, dBFS (a mean power of 1 is 0 dBFS).
    #[field(shared = keys::LEVEL_DBFS)]
    pub level_dbfs: Option<f64>,
    /// Signal to noise ratio of what is being decoded, dB (in the signal's own bandwidth, or per symbol where the stack says so).
    #[field(shared = keys::SNR_DB)]
    pub snr_db: Option<f64>,
    /// Carrier offset from where it was expected, Hz.
    #[field(shared = keys::OFFSET_HZ)]
    pub offset_hz: Option<Hertz>,
    /// The text of a decoded message.
    #[field(shared = keys::TEXT)]
    pub text: &'a str,
    /// What no public document confirms.
    #[field(label = "Note")]
    pub note: &'a str,
}

/// A vehicle heard. The source is the temporary id.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct V2xVehicleRow<'a> {
    /// The kind of a row.
    #[field(label = "Kind")]
    pub kind: FrameKind,
    /// The sender of a decoded message: a registration, an address, a call sign or a code.
    #[field(shared = keys::SOURCE)]
    pub source: &'a str,
    /// The position of the object that the event is about.
    #[field(shared = keys::POSITION)]
    pub position: Option<(f64, f64)>,
    /// The speed over the ground of the object that the event is about, knots.
    #[field(shared = keys::SPEED_KT)]
    pub speed_kt: Option<f64>,
    /// The course over the ground of the object that the event is about, degrees clockwise from true north.
    #[field(shared = keys::COURSE_DEG)]
    pub course_deg: Option<f64>,
    /// The text of a decoded message.
    #[field(shared = keys::TEXT)]
    pub text: &'a str,
    /// Messages from a vehicle.
    #[field(label = "Messages")]
    pub frames: i64,
    /// Level of the signal or channel, dBFS (a mean power of 1 is 0 dBFS).
    #[field(shared = keys::LEVEL_DBFS)]
    pub level_dbfs: f64,
    /// Wall-clock time when first heard.
    #[field(shared = columns::FIRST_HEARD)]
    pub first_heard: Option<Time>,
    /// Wall-clock time when last heard, in whole seconds. A host shows the age from it.
    #[field(shared = columns::LAST_HEARD)]
    pub last_heard: Option<Time>,
}

/// A frame of the frame log.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct V2xFrameRow<'a> {
    /// The signal time of a frame.
    #[field(label = "Time", unit = "s")]
    pub time_s: f64,
    /// The kind of a row.
    #[field(label = "Kind")]
    pub kind: FrameKind,
    /// The sender of a decoded message: a registration, an address, a call sign or a code.
    #[field(shared = keys::SOURCE)]
    pub source: &'a str,
    /// The text of a decoded message.
    #[field(shared = keys::TEXT)]
    pub text: &'a str,
    /// Level of the signal or channel, dBFS (a mean power of 1 is 0 dBFS).
    #[field(shared = keys::LEVEL_DBFS)]
    pub level_dbfs: f64,
}

/// A vehicle heard for the first time.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct V2xVehicleEvent<'a> {
    /// The kind of a row.
    #[field(label = "Kind")]
    pub kind: FrameKind,
    /// The sender of a decoded message: a registration, an address, a call sign or a code.
    #[field(shared = keys::SOURCE)]
    pub source: &'a str,
    /// The position of the object that the event is about.
    #[field(shared = keys::POSITION)]
    pub position: Option<(f64, f64)>,
    /// The text of a decoded message.
    #[field(shared = keys::TEXT)]
    pub text: &'a str,
    /// What an event is about (aircraft, vessel, pager). Outputs and the store filter on it.
    #[field(shared = keys::FAMILY)]
    pub family: Family,
}

/// The vehicles, most recently heard first.
pub static V2X_VEHICLES: Table<V2xVehicleRow<'static>> =
    Table::new("vehicles", "Vehicles", crate::v2x::VEHICLE_CAP);
/// The frame log, newest first.
pub static V2X_FRAMES: Table<V2xFrameRow<'static>> =
    Table::new("frames", "Frames", crate::v2x::FRAME_CAP);
/// A vehicle heard for the first time.
pub static V2X_VEHICLE: EventKind<V2xVehicleEvent<'static>> =
    EventKind::new("v2x.vehicle", "V2X vehicle");

/// The arguments of `remove` of a vehicle.
#[derive(Arguments)]
pub struct V2xVehicleArgs<'a> {
    /// The row key of the vehicle.
    #[arg(shared = VEHICLE_ROW)]
    pub row: &'a str,
}
/// The row key of a vehicle.
pub const VEHICLE_ROW: Argument<field::Text> = objcmd::row_of("vehicles");
/// Forgets one vehicle.
pub const V2X_REMOVE: CommandKind<V2xVehicleArgs<'static>> = objcmd::remove_kind();

/// The interface of the family.
pub fn family() -> &'static Described {
    static FAMILY: OnceLock<Described> = OnceLock::new();
    FAMILY.get_or_init(|| {
        Described::new("road", 1, 0).stack(
            StackShape::new("v2x")
                .fields::<V2xFields>()
                .fields::<DecoderFields>()
                .table(&V2X_VEHICLES)
                .table(&V2X_FRAMES)
                .points("Vehicles")
                .event(&V2X_VEHICLE)
                .settings(V2xSetting::SETTINGS)
                .command(&sdrkit_graph::decoder::CLEAR)
                .command(&V2X_REMOVE)
                // The vehicles heard and the frame log are history: the decoder that holds them stays.
                .kept(&["vehicles", "frames", "V2xFields", "DecoderFields"]),
        )
    })
}

/// The description of the stack `id`.
///
/// # Panics
///
/// If the family does not describe the stack.
pub fn shape(id: &str) -> &'static StackShape {
    family()
        .stack_shape(id)
        .unwrap_or_else(|| panic!("the road family does not describe the stack {id}"))
}
