//! The interface of the drones family: the shapes of the result sets, the table rows and the events, the tables, the
//! event types, the commands and what each stack declares.

use crate::adsl::AdslSetting;
use crate::dji::DjiSetting;
use crate::dji::{Attempt, Class, RadioBand, Reason};
use crate::flarm::FlarmSetting;
use crate::flarm::KeyTime;
use crate::mavlink_sik::SikSetting;
use crate::ogn::OgnSetting;
use crate::ogn::PacketKind;
use crate::rc_links::RcStackSetting;
use crate::remote_id::messages::{MsgType, Transport};
use crate::remote_id::table::Via;
use crate::remote_id::trust::TrustVerdict;
use crate::remote_id::Band;
use crate::remote_id::RidSetting;
use sdrkit_core::interface::{
    Arguments, CommandKind, EventKind, Family as Described, Fields, Hertz, Seconds, StackShape,
    Table, Time,
};
use sdrkit_core::publish::keys::{self, Family, Urgency};
use sdrkit_core::publish::{field, SeriesDef};
use sdrkit_core::settings::Argument;
use sdrkit_core::SettingSet;
use sdrkit_graph::DecoderFields;
use sdrkit_util_objects::commands as objcmd;
use sdrkit_util_objects::publish::columns;
use std::sync::OnceLock;

/// The result fields of Drone Remote ID.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct RemoteIdFields {
    /// The BLE advertising channel in the band (0 for a file without a centre).
    #[field(label = "Bluetooth LE channel")]
    pub ble_channel: Option<i64>,
    /// The offset of the BLE advertising channel from the centre.
    #[field(label = "Bluetooth LE offset", unit = "Hz")]
    pub ble_offset_hz: Option<Hertz>,
    /// The Wi-Fi channel in the band (0 for a file without a centre).
    #[field(label = "Wi-Fi channel")]
    pub wifi_channel: Option<i64>,
    /// The offset of the Wi-Fi channel from the centre.
    #[field(label = "Wi-Fi offset", unit = "Hz")]
    pub wifi_offset_hz: Hertz,
    /// The band of the present centre, when it is one of the bands that the stack offers.
    #[field(label = "Band")]
    pub band: Option<Band>,
    /// Drones in the table.
    #[field(label = "Drones", reading = 1)]
    pub drones: i64,
    /// Drones in the table whose trust verdict is suspicious or spoofed.
    #[field(label = "Flagged drones", reading = 2)]
    pub flagged: i64,
    /// Messages decoded.
    #[field(label = "Messages")]
    pub messages: i64,
    /// Messages refused: members of a Message Pack that are packs or of a reserved type, authentication pages that fail their
    /// checks.
    #[field(label = "Messages refused")]
    pub messages_refused: i64,
    /// Messages with a value outside its range (a Location timestamp above 36000, a latitude or longitude out of range), which
    /// the tables show as absent.
    #[field(label = "Values out of range")]
    pub fields_out_of_range: i64,
    /// Good BLE packets.
    #[field(label = "BLE packets")]
    pub ble_good: i64,
    /// Good BLE packets with Remote ID.
    #[field(label = "BLE packets with Remote ID")]
    pub ble_remote_id: i64,
    /// Good Wi-Fi frames.
    #[field(label = "Wi-Fi frames")]
    pub wifi_good: i64,
    /// Good Wi-Fi frames with Remote ID.
    #[field(label = "Wi-Fi frames with Remote ID")]
    pub wifi_remote_id: i64,
    /// Frames that failed their check.
    #[field(shared = keys::FRAMES_BAD)]
    pub frames_bad: i64,
    /// Basic ID messages (a field: all drones; a column: one drone).
    #[field(label = "Basic ID")]
    pub basic_id_messages: i64,
    /// Location messages.
    #[field(label = "Location")]
    pub location_messages: i64,
    /// Authentication messages.
    #[field(label = "Authentication")]
    pub authentication_messages: i64,
    /// Self-ID messages.
    #[field(label = "Self-ID")]
    pub self_id_messages: i64,
    /// System messages.
    #[field(label = "System")]
    pub system_messages: i64,
    /// Operator ID messages.
    #[field(label = "Operator ID")]
    pub operator_id_messages: i64,
    /// Signal to noise ratio of what is being decoded, dB (in the signal's own bandwidth, or per symbol where the stack says so).
    #[field(shared = keys::SNR_DB)]
    pub snr_db: Option<f64>,
    /// Carrier offset from where it was expected, Hz.
    #[field(shared = keys::OFFSET_HZ)]
    pub offset_hz: Option<Hertz>,
    /// Level of the signal or channel, dBFS (a mean power of 1 is 0 dBFS).
    #[field(shared = keys::LEVEL_DBFS)]
    pub level_dbfs: Option<f64>,
}

/// A drone heard on Remote ID. The row key is the broadcast address. The source is the shown ID: the serial number, else the session ID, else the CAA registration, else the UTM UUID, else the address.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct RemoteIdDroneRow<'a> {
    /// The sender of a decoded message: a registration, an address, a call sign or a code.
    #[field(shared = keys::SOURCE)]
    pub source: &'a str,
    /// The broadcast address.
    #[field(label = "Address")]
    pub address: &'a str,
    /// The trust verdict of the broadcasts heard since the stack started: authenticated, plausible, suspicious or spoofed. No
    /// value before the first packet since the start (a drone of a restored state).
    #[field(label = "Trust verdict")]
    pub verdict: Option<TrustVerdict>,
    /// The active reasons of the trust verdict: their stable names (`teleport`, `counter_conflict`, ...) joined by ", ".
    /// Empty when there is none.
    #[field(label = "Trust reasons")]
    pub reasons: &'a str,
    /// Serial number.
    #[field(label = "Serial number")]
    pub serial: Option<&'a str>,
    /// Session ID.
    #[field(label = "Session ID")]
    pub session_id: Option<&'a str>,
    /// CAA registration.
    #[field(label = "CAA registration")]
    pub caa_registration: Option<&'a str>,
    /// UTM assigned UUID.
    #[field(label = "UTM UUID")]
    pub utm_uuid: Option<&'a str>,
    /// UA type code (ASTM F3411-22a, Basic ID message).
    #[field(label = "UA type")]
    pub ua_type: Option<i64>,
    /// Operational status code (ASTM F3411-22a, Location message).
    #[field(label = "Status")]
    pub status: Option<i64>,
    /// The position of the object that the event is about.
    #[field(shared = keys::POSITION)]
    pub position: Option<(f64, f64)>,
    /// Geodetic altitude.
    #[field(label = "Altitude", unit = "m", role = Altitude)]
    pub alt_geo_m: Option<f64>,
    /// Pressure altitude.
    #[field(label = "Pressure altitude", unit = "m")]
    pub alt_baro_m: Option<f64>,
    /// Height above take-off or ground, as the column `height_type` says.
    #[field(label = "Height", unit = "m")]
    pub height_m: Option<f64>,
    /// What the height is above: 0 take-off, 1 ground.
    #[field(label = "Height above")]
    pub height_type: Option<i64>,
    /// The speed over the ground of the object that the event is about, knots.
    #[field(shared = keys::SPEED_KT)]
    pub speed_kt: Option<f64>,
    /// Vertical speed, up.
    #[field(label = "Vertical speed", unit = "m/s")]
    pub vspeed_m_s: Option<f64>,
    /// The course over the ground of the object that the event is about, degrees clockwise from true north.
    #[field(shared = keys::COURSE_DEG)]
    pub course_deg: Option<f64>,
    /// Horizontal accuracy code.
    #[field(label = "Horizontal accuracy")]
    pub horiz_acc: Option<i64>,
    /// Vertical accuracy code.
    #[field(label = "Vertical accuracy")]
    pub vert_acc: Option<i64>,
    /// Speed accuracy code.
    #[field(label = "Speed accuracy")]
    pub speed_acc: Option<i64>,
    /// Tenths of a second since the hour.
    #[field(label = "Time stamp", unit = "0.1 s")]
    pub timestamp_ds: Option<i64>,
    /// The operator's position.
    #[field(label = "Operator position")]
    pub operator_position: Option<(f64, f64)>,
    /// The operator's altitude.
    #[field(label = "Operator altitude", unit = "m")]
    pub operator_alt_m: Option<f64>,
    /// Operator location type code.
    #[field(label = "Operator location")]
    pub location_type: Option<i64>,
    /// Classification type code.
    #[field(label = "Classification")]
    pub classification: Option<i64>,
    /// EU category code.
    #[field(label = "Category")]
    pub category: Option<i64>,
    /// EU class code (1 to 7 are C0 to C6).
    #[field(label = "Class")]
    pub class: Option<i64>,
    /// System time stamp, seconds since 2019-01-01.
    #[field(label = "System time stamp", unit = "s")]
    pub system_timestamp_s: Option<i64>,
    /// Operator ID.
    #[field(label = "Operator ID")]
    pub operator_id: Option<&'a str>,
    /// Self-ID text.
    #[field(label = "Self-ID")]
    pub self_id: Option<&'a str>,
    /// Authentication type code.
    #[field(label = "Authentication")]
    pub authentication: Option<i64>,
    /// Authentication pages.
    #[field(label = "Pages")]
    pub authentication_pages: Option<i64>,
    /// Authentication data length in bytes, from page 0.
    #[field(label = "Authentication length", unit = "B")]
    pub authentication_length: Option<i64>,
    /// Every page of the present authentication was heard. No signature is verified.
    #[field(label = "Authentication complete")]
    pub authentication_complete: Option<bool>,
    /// Basic ID messages (a field: all drones; a column: one drone).
    #[field(label = "Basic ID")]
    pub basic_id_messages: i64,
    /// Location messages.
    #[field(label = "Location")]
    pub location_messages: i64,
    /// Authentication messages.
    #[field(label = "Authentication")]
    pub authentication_messages: i64,
    /// Self-ID messages.
    #[field(label = "Self-ID")]
    pub self_id_messages: i64,
    /// System messages.
    #[field(label = "System")]
    pub system_messages: i64,
    /// Operator ID messages.
    #[field(label = "Operator ID")]
    pub operator_id_messages: i64,
    /// Trail points.
    #[field(label = "Track points")]
    pub track_points: i64,
    /// How the drone was heard.
    #[field(label = "Via")]
    pub via: Via,
    /// Level of the signal or channel, dBFS (a mean power of 1 is 0 dBFS).
    #[field(shared = keys::LEVEL_DBFS)]
    pub level_dbfs: Option<f64>,
    /// Wall-clock time when last heard, in whole seconds. A host shows the age from it.
    #[field(shared = columns::LAST_HEARD)]
    pub last_heard: Option<Time>,
    /// Times heard.
    #[field(shared = columns::HEARD)]
    pub heard: Option<i64>,
}

/// A line of the message log. The row key is the sequence number of the line.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct RemoteIdMessageRow<'a> {
    /// Signal time of a log line.
    #[field(label = "Time", unit = "s")]
    pub time_s: f64,
    /// Wall time of a log line.
    #[field(label = "Time (UTC)")]
    pub time: Option<Time>,
    /// The carrier of a log line.
    #[field(label = "Via")]
    pub via: Transport,
    /// The radio channel of a log line (BLE 37 to 39, Wi-Fi 1 to 13; none when unknown).
    #[field(label = "Channel")]
    pub radio_channel: Option<i64>,
    /// The broadcast address.
    #[field(label = "Address")]
    pub address: &'a str,
    /// Level of the signal or channel, dBFS (a mean power of 1 is 0 dBFS).
    #[field(shared = keys::LEVEL_DBFS)]
    pub level_dbfs: Option<f64>,
    /// The message type.
    #[field(label = "Type")]
    pub r#type: MsgType,
    /// The summary line of a message.
    #[field(label = "Message")]
    pub message: &'a str,
}

/// A track point of a drone. The row key is the drone key and the signal time of the point.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct DroneTrackRow<'a> {
    /// The drone a track point belongs to.
    #[field(label = "Drone", refers_to = "drones")]
    pub drone: &'a str,
    /// The position of the object that the event is about.
    #[field(shared = keys::POSITION)]
    pub position: (f64, f64),
    /// The signal time of a track point.
    #[field(label = "Time", unit = "s")]
    pub time_s: f64,
}

/// A drone with a position, at most every 5 s for each drone. The urgency is emergency when the drone states the emergency status.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct RemoteIdDroneEvent<'a> {
    /// The sender of a decoded message: a registration, an address, a call sign or a code.
    #[field(shared = keys::SOURCE)]
    pub source: &'a str,
    /// The broadcast address.
    #[field(label = "Address")]
    pub address: &'a str,
    /// The position of the object that the event is about.
    #[field(shared = keys::POSITION)]
    pub position: (f64, f64),
    /// Geodetic altitude.
    #[field(label = "Altitude", unit = "m")]
    pub alt_geo_m: Option<f64>,
    /// The speed over the ground of the object that the event is about, knots.
    #[field(shared = keys::SPEED_KT)]
    pub speed_kt: Option<f64>,
    /// The course over the ground of the object that the event is about, degrees clockwise from true north.
    #[field(shared = keys::COURSE_DEG)]
    pub course_deg: Option<f64>,
    /// The operator's position.
    #[field(label = "Operator position")]
    pub operator_position: Option<(f64, f64)>,
    /// How urgent an event is. A stack sets it when the decoded message is urgent. Services alert on it.
    #[field(shared = keys::URGENCY)]
    pub urgency: Urgency,
    /// What an event is about (aircraft, vessel, pager). Outputs and the store filter on it.
    #[field(shared = keys::FAMILY)]
    pub family: Family,
}

/// A change of the trust verdict of a Remote ID drone. The urgency is notice when the verdict is suspicious or spoofed, else
/// none.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct RemoteIdVerdictEvent<'a> {
    /// The sender of a decoded message: a registration, an address, a call sign or a code.
    #[field(shared = keys::SOURCE)]
    pub source: &'a str,
    /// The broadcast address.
    #[field(label = "Address")]
    pub address: &'a str,
    /// The new trust verdict.
    #[field(label = "Trust verdict")]
    pub verdict: TrustVerdict,
    /// The active reasons of the verdict: their stable names joined by ", ". Empty when there is none.
    #[field(label = "Trust reasons")]
    pub reasons: &'a str,
    /// The position of the object that the event is about.
    #[field(shared = keys::POSITION)]
    pub position: Option<(f64, f64)>,
    /// How urgent an event is. A stack sets it when the decoded message is urgent. Services alert on it.
    #[field(shared = keys::URGENCY)]
    pub urgency: Urgency,
    /// What an event is about (aircraft, vessel, pager). Outputs and the store filter on it.
    #[field(shared = keys::FAMILY)]
    pub family: Family,
}

/// The result fields of DJI drones.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct DjiDroneidFields<'a> {
    /// Bursts measured.
    #[field(label = "Bursts measured")]
    pub bursts_measured: i64,
    /// OFDM-like bursts.
    #[field(label = "OFDM-like bursts")]
    pub bursts_ofdm_like: i64,
    /// DroneID packets decoded.
    #[field(label = "DroneID packets")]
    pub droneid_decoded: i64,
    /// DroneID bursts with the symbols whose CRC failed.
    #[field(label = "DroneID CRC failures")]
    pub droneid_crc_failed: i64,
    /// DroneID attempts.
    #[field(label = "DroneID attempts")]
    pub droneid_tried: i64,
    /// The capture centre.
    #[field(label = "Capture centre", unit = "Hz")]
    pub centre_hz: Option<Hertz>,
    /// The carrier offset of the last DroneID burst from the 0.5 MHz grid.
    #[field(label = "DroneID carrier offset", unit = "Hz")]
    pub burst_cfo_hz: Option<Hertz>,
    /// Drones in the table.
    #[field(label = "Drones")]
    pub drones: i64,
    /// The track with the most likely DJI shape: a row key of the `signals` table.
    #[field(label = "Signal", refers_to = "signals")]
    pub signal: Option<&'a str>,
    /// Noise floor, dBFS.
    #[field(shared = keys::NOISE_DBFS)]
    pub noise_dbfs: Option<f64>,
    /// Level of the signal or channel, dBFS (a mean power of 1 is 0 dBFS).
    #[field(shared = keys::LEVEL_DBFS)]
    pub level_dbfs: Option<f64>,
    /// Signal to noise ratio of what is being decoded, dB (in the signal's own bandwidth, or per symbol where the stack says so).
    #[field(shared = keys::SNR_DB)]
    pub snr_db: Option<f64>,
    /// Carrier offset from where it was expected, Hz.
    #[field(shared = keys::OFFSET_HZ)]
    pub offset_hz: Option<Hertz>,
}

/// A drone link recognised by its shape.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct DjiDroneidSignalRow {
    /// The track number.
    #[field(label = "Track")]
    pub track: i64,
    /// The class of a track.
    #[field(label = "Class")]
    pub class: Class,
    /// The confidence of the class.
    #[field(label = "Confidence", unit = "%")]
    pub confidence_pct: f64,
    /// The radio frequency.
    #[field(label = "Frequency", unit = "Hz")]
    pub freq_hz: Option<Hertz>,
    /// Carrier offset from where it was expected, Hz.
    #[field(shared = keys::OFFSET_HZ)]
    pub offset_hz: Hertz,
    /// The width.
    #[field(label = "Width", unit = "Hz")]
    pub width_hz: Hertz,
    /// The last burst was clipped.
    #[field(label = "Clipped")]
    pub clipped: bool,
    /// The burst length.
    #[field(label = "Burst", unit = "s")]
    pub burst: Seconds,
    /// The burst period.
    #[field(label = "Period", unit = "s")]
    pub period: Option<Seconds>,
    /// The duty cycle.
    #[field(label = "Duty cycle", unit = "%")]
    pub duty_pct: f64,
    /// The share of gaps that are whole periods.
    #[field(label = "Regularity", unit = "%")]
    pub regularity_pct: f64,
    /// How OFDM-like the bursts are.
    #[field(label = "OFDM-like", unit = "%")]
    pub ofdm_pct: f64,
    /// Level of the signal or channel, dBFS (a mean power of 1 is 0 dBFS).
    #[field(shared = keys::LEVEL_DBFS)]
    pub level_dbfs: f64,
    /// Signal to noise ratio of what is being decoded, dB (in the signal's own bandwidth, or per symbol where the stack says so).
    #[field(shared = keys::SNR_DB)]
    pub snr_db: f64,
    /// Bursts seen.
    #[field(label = "Bursts")]
    pub bursts: i64,
    /// The DroneID symbols were seen.
    #[field(label = "DroneID symbols")]
    pub zc_seen: bool,
    /// DroneID packets decoded on the track.
    #[field(label = "Decoded")]
    pub decoded: i64,
    /// DroneID CRC failures on the track.
    #[field(label = "Failed")]
    pub failed: i64,
    /// The band.
    #[field(label = "Band")]
    pub band: Option<RadioBand>,
    /// Why the track has its class.
    #[field(label = "Reason")]
    pub reason: Reason,
    /// The track is within 3 MHz of a frequency reported for DroneID.
    #[field(label = "Near a DroneID frequency")]
    pub near_droneid: bool,
    /// The signal time of the last burst.
    #[field(label = "Last burst", unit = "s")]
    pub last_s: f64,
}

/// A DJI drone heard on DroneID.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct DjiDroneidDroneRow<'a> {
    /// The sender of a decoded message: a registration, an address, a call sign or a code.
    #[field(shared = keys::SOURCE)]
    pub source: &'a str,
    /// The position of the object that the event is about.
    #[field(shared = keys::POSITION)]
    pub position: Option<(f64, f64)>,
    /// The drone's altitude, read as 0.01 m. The unit is not confirmed by a public source: the NDSS 2023 receiver reads feet.
    #[field(label = "Altitude", unit = "m", role = Altitude)]
    pub alt_m: f64,
    /// The drone's height above home, read as 0.01 m. The unit is not confirmed, as for the altitude.
    #[field(label = "Height", unit = "m")]
    pub height_m: f64,
    /// The altitude field as received (a signed 16-bit number), whose unit is not confirmed.
    #[field(label = "Altitude raw")]
    pub alt_raw: i64,
    /// The height field as received (a signed 16-bit number), whose unit is not confirmed.
    #[field(label = "Height raw")]
    pub height_raw: i64,
    /// The speed over the ground of the object that the event is about, knots.
    #[field(shared = keys::SPEED_KT)]
    pub speed_kt: f64,
    /// The course over the ground of the object that the event is about, degrees clockwise from true north.
    #[field(shared = keys::COURSE_DEG)]
    pub course_deg: f64,
    /// The home position.
    #[field(label = "Home")]
    pub home: Option<(f64, f64)>,
    /// The pilot's position.
    #[field(label = "Pilot")]
    pub pilot: Option<(f64, f64)>,
    /// The packet sequence number.
    #[field(label = "Sequence")]
    pub seq: i64,
    /// The state information bits.
    #[field(label = "State information")]
    pub state_info: i64,
    /// The packet version.
    #[field(label = "Version")]
    pub version: i64,
    /// The inner CRC passed.
    #[field(label = "Inner CRC good")]
    pub inner_crc_ok: bool,
    /// Velocity north.
    #[field(label = "Velocity north", unit = "cm/s")]
    pub v_north: i64,
    /// Velocity east.
    #[field(label = "Velocity east", unit = "cm/s")]
    pub v_east: i64,
    /// Velocity up.
    #[field(label = "Velocity up", unit = "cm/s")]
    pub v_up: i64,
    /// The raw yaw.
    #[field(label = "Yaw")]
    pub yaw: i64,
    /// The GPS time of the packet.
    #[field(label = "GPS time")]
    pub gps_time: Time,
    /// The device type code.
    #[field(label = "Device type")]
    pub device_type: i64,
    /// The UUID.
    #[field(label = "UUID")]
    pub uuid: Option<&'a str>,
    /// The radio frequency.
    #[field(label = "Frequency", unit = "Hz")]
    pub freq_hz: Option<Hertz>,
    /// Signal to noise ratio of what is being decoded, dB (in the signal's own bandwidth, or per symbol where the stack says so).
    #[field(shared = keys::SNR_DB)]
    pub snr_db: f64,
    /// The track number.
    #[field(label = "Track")]
    pub track: i64,
    /// Trail points.
    #[field(label = "Track points")]
    pub trail_points: i64,
    /// Wall-clock time when last heard, in whole seconds. A host shows the age from it.
    #[field(shared = columns::LAST_HEARD)]
    pub last_heard: Option<Time>,
    /// Times heard.
    #[field(shared = columns::HEARD)]
    pub heard: Option<i64>,
}

/// A burst in the burst log.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct DjiDroneidBurstRow {
    /// The signal time of a burst.
    #[field(label = "Time", unit = "s")]
    pub time_s: f64,
    /// The wall time of a burst.
    #[field(label = "Time (UTC)")]
    pub time: Option<Time>,
    /// The track number.
    #[field(label = "Track")]
    pub track: i64,
    /// Carrier offset from where it was expected, Hz.
    #[field(shared = keys::OFFSET_HZ)]
    pub offset_hz: Hertz,
    /// The width.
    #[field(label = "Width", unit = "Hz")]
    pub width_hz: Hertz,
    /// The burst length.
    #[field(label = "Burst", unit = "s")]
    pub burst: Seconds,
    /// Level of the signal or channel, dBFS (a mean power of 1 is 0 dBFS).
    #[field(shared = keys::LEVEL_DBFS)]
    pub level_dbfs: f64,
    /// Signal to noise ratio of what is being decoded, dB (in the signal's own bandwidth, or per symbol where the stack says so).
    #[field(shared = keys::SNR_DB)]
    pub snr_db: f64,
    /// How OFDM-like the bursts are.
    #[field(label = "OFDM-like", unit = "%")]
    pub ofdm_pct: f64,
    /// The DroneID outcome of a burst.
    #[field(label = "DroneID")]
    pub droneid: Attempt,
}

/// A DroneID packet.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct DjiDroneidEvent<'a> {
    /// The sender of a decoded message: a registration, an address, a call sign or a code.
    #[field(shared = keys::SOURCE)]
    pub source: &'a str,
    /// The position of the object that the event is about.
    #[field(shared = keys::POSITION)]
    pub position: Option<(f64, f64)>,
    /// The drone's altitude, read as 0.01 m. The unit is not confirmed by a public source: the NDSS 2023 receiver reads feet.
    #[field(label = "Altitude", unit = "m")]
    pub alt_m: f64,
    /// The drone's height above home, read as 0.01 m. The unit is not confirmed, as for the altitude.
    #[field(label = "Height", unit = "m")]
    pub height_m: f64,
    /// The altitude field as received (a signed 16-bit number), whose unit is not confirmed.
    #[field(label = "Altitude raw")]
    pub alt_raw: i64,
    /// The height field as received (a signed 16-bit number), whose unit is not confirmed.
    #[field(label = "Height raw")]
    pub height_raw: i64,
    /// The speed over the ground of the object that the event is about, knots.
    #[field(shared = keys::SPEED_KT)]
    pub speed_kt: f64,
    /// The course over the ground of the object that the event is about, degrees clockwise from true north.
    #[field(shared = keys::COURSE_DEG)]
    pub course_deg: f64,
    /// The home position.
    #[field(label = "Home")]
    pub home: Option<(f64, f64)>,
    /// The pilot's position.
    #[field(label = "Pilot")]
    pub pilot: Option<(f64, f64)>,
    /// What an event is about (aircraft, vessel, pager). Outputs and the store filter on it.
    #[field(shared = keys::FAMILY)]
    pub family: Family,
}

/// The result fields of FLARM.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct FlarmFields {
    /// Old-protocol packets decoded.
    #[field(label = "Packets decoded")]
    pub v6_decoded: i64,
    /// Frames that failed their check.
    #[field(shared = keys::FRAMES_BAD)]
    pub frames_bad: i64,
    /// Syncs taken.
    #[field(label = "Syncs")]
    pub syncs: i64,
    /// Good frames on 868.2 MHz.
    #[field(label = "Frames on 868.2 MHz")]
    pub frames_868_2: i64,
    /// Good frames on 868.4 MHz.
    #[field(label = "Frames on 868.4 MHz")]
    pub frames_868_4: i64,
    /// Newer-protocol packets.
    #[field(label = "Newer-protocol packets")]
    pub newer_packets: i64,
    /// Newer-protocol devices.
    #[field(label = "Newer-protocol devices")]
    pub newer_devices: i64,
    /// Packets of other types.
    #[field(label = "Other packets")]
    pub other_type: i64,
    /// Privacy-flagged aircraft.
    #[field(label = "Privacy-flagged aircraft")]
    pub privacy_aircraft: i64,
    /// Privacy-flagged packets.
    #[field(label = "Privacy-flagged packets")]
    pub privacy_packets: i64,
    /// Aircraft in the table.
    #[field(label = "Aircraft")]
    pub aircraft: i64,
    /// Aircraft with a position.
    #[field(label = "With a position")]
    pub with_position: i64,
    /// The farthest range seen.
    #[field(label = "Farthest", unit = "km")]
    pub farthest_km: f64,
    /// Packets a second, over the last second.
    #[field(label = "Packets per second")]
    pub packets_per_s: f64,
    /// Where the key time comes from.
    #[field(label = "Key time")]
    pub key_time: KeyTime,
    /// The station position is missing: without it the packets give no positions.
    #[field(label = "Needs the station position")]
    pub needs_station: bool,
    /// Signal to noise ratio of what is being decoded, dB (in the signal's own bandwidth, or per symbol where the stack says so).
    #[field(shared = keys::SNR_DB)]
    pub snr_db: Option<f64>,
    /// Noise floor, dBFS.
    #[field(shared = keys::NOISE_DBFS)]
    pub noise_dbfs: Option<f64>,
}

/// An aircraft heard on FLARM.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct FlarmAircraftRow<'a> {
    /// The sender of a decoded message: a registration, an address, a call sign or a code.
    #[field(shared = keys::SOURCE)]
    pub source: &'a str,
    /// Address type code (0 random, 1 ICAO, 2 FLARM, 3 anonymous).
    #[field(label = "Address type")]
    pub address_type: i64,
    /// Aircraft type code (0 to 15).
    #[field(label = "Type")]
    pub r#type: i64,
    /// The position of the object that the event is about.
    #[field(shared = keys::POSITION)]
    pub position: Option<(f64, f64)>,
    /// Altitude.
    #[field(label = "Altitude", unit = "m", role = Altitude)]
    pub alt_m: f64,
    /// The speed over the ground of the object that the event is about, knots.
    #[field(shared = keys::SPEED_KT)]
    pub speed_kt: f64,
    /// The course over the ground of the object that the event is about, degrees clockwise from true north.
    #[field(shared = keys::COURSE_DEG)]
    pub course_deg: f64,
    /// Climb.
    #[field(label = "Climb", unit = "m/s")]
    pub climb_m_s: f64,
    /// Distance from the station.
    #[field(label = "Distance", unit = "km")]
    pub distance_km: Option<f64>,
    /// Bearing from the station.
    #[field(label = "Bearing", unit = "°")]
    pub bearing_deg: Option<f64>,
    /// The radio channel.
    #[field(label = "Channel", unit = "Hz")]
    pub channel_hz: Hertz,
    /// Level over the noise.
    #[field(label = "Level over the noise", unit = "dB")]
    pub level_db: f64,
    /// Airborne.
    #[field(label = "Airborne")]
    pub airborne: bool,
    /// How urgent an event is. A stack sets it when the decoded message is urgent. Services alert on it.
    #[field(shared = keys::URGENCY)]
    pub urgency: Urgency,
    /// Wall-clock time when last heard, in whole seconds. A host shows the age from it.
    #[field(shared = columns::LAST_HEARD)]
    pub last_heard: Option<Time>,
    /// Times heard.
    #[field(shared = columns::HEARD)]
    pub heard: Option<i64>,
}

/// A track point of an aircraft. The row key is the aircraft key and the signal time of the point.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct AircraftTrackRow<'a> {
    /// The aircraft a track point belongs to.
    #[field(label = "Aircraft", refers_to = "aircraft")]
    pub aircraft: &'a str,
    /// The position of the object that the event is about.
    #[field(shared = keys::POSITION)]
    pub position: (f64, f64),
    /// The signal time of a track point.
    #[field(label = "Time", unit = "s")]
    pub time_s: f64,
}

/// An aircraft heard on FLARM.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct FlarmAircraftEvent<'a> {
    /// The sender of a decoded message: a registration, an address, a call sign or a code.
    #[field(shared = keys::SOURCE)]
    pub source: &'a str,
    /// Aircraft type code (0 to 15).
    #[field(label = "Type")]
    pub r#type: i64,
    /// The position of the object that the event is about.
    #[field(shared = keys::POSITION)]
    pub position: (f64, f64),
    /// Altitude.
    #[field(label = "Altitude", unit = "m")]
    pub alt_m: f64,
    /// The speed over the ground of the object that the event is about, knots.
    #[field(shared = keys::SPEED_KT)]
    pub speed_kt: f64,
    /// The course over the ground of the object that the event is about, degrees clockwise from true north.
    #[field(shared = keys::COURSE_DEG)]
    pub course_deg: f64,
    /// Climb.
    #[field(label = "Climb", unit = "m/s")]
    pub climb_m_s: f64,
    /// How urgent an event is. A stack sets it when the decoded message is urgent. Services alert on it.
    #[field(shared = keys::URGENCY)]
    pub urgency: Urgency,
    /// What an event is about (aircraft, vessel, pager). Outputs and the store filter on it.
    #[field(shared = keys::FAMILY)]
    pub family: Family,
}

/// The result fields of ADS-L.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct AdslFields {
    /// Frames (packets, blocks, bursts) that passed their check.
    #[field(shared = keys::FRAMES_OK)]
    pub frames_ok: i64,
    /// Frames that failed their check.
    #[field(shared = keys::FRAMES_BAD)]
    pub frames_bad: i64,
    /// Packets with good parity on the band.
    #[field(label = "Packets on 868.2 MHz")]
    pub packets_868_2: i64,
    /// Packets with good parity on the band.
    #[field(label = "Packets on 868.4 MHz")]
    pub packets_868_4: i64,
    /// Packets with good parity on the band.
    #[field(label = "Packets on O-band LDR")]
    pub packets_ldr: i64,
    /// Packets with good parity on the band.
    #[field(label = "Packets on O-band HDR")]
    pub packets_hdr: i64,
    /// The band is inside the capture and received.
    #[field(label = "868.2 MHz received")]
    pub on_868_2: bool,
    /// The band is inside the capture and received.
    #[field(label = "868.4 MHz received")]
    pub on_868_4: bool,
    /// The band is inside the capture and received.
    #[field(label = "O-band LDR received")]
    pub on_ldr: bool,
    /// The band is inside the capture and received.
    #[field(label = "O-band HDR received")]
    pub on_hdr: bool,
    /// Traffic packets.
    #[field(label = "Traffic packets")]
    pub traffic: i64,
    /// Status packets.
    #[field(label = "Status packets")]
    pub status: i64,
    /// Traffic Uplink packets.
    #[field(label = "Uplink packets")]
    pub uplink: i64,
    /// FIS-B Uplink packets.
    #[field(label = "FIS-B packets")]
    pub fis_b: i64,
    /// Remote Identification packets.
    #[field(label = "Remote ID packets")]
    pub remote_id: i64,
    /// Packets of other payload types.
    #[field(label = "Other packets")]
    pub other: i64,
    /// Packets with a reserved key index: their content is not read.
    #[field(label = "Encrypted packets")]
    pub encrypted: i64,
    /// Packets with the signature flag. The signature is not checked: no register of public keys exists.
    #[field(label = "Signed packets")]
    pub signed: i64,
    /// Packets that needed a repair.
    #[field(label = "Repaired packets")]
    pub corrected: i64,
    /// Syncs taken.
    #[field(label = "Syncs")]
    pub syncs: i64,
    /// Noise floor, dBFS.
    #[field(shared = keys::NOISE_DBFS)]
    pub noise_dbfs: Option<f64>,
    /// Signal to noise ratio of what is being decoded, dB (in the signal's own bandwidth, or per symbol where the stack says so).
    #[field(shared = keys::SNR_DB)]
    pub snr_db: Option<f64>,
    /// Good packets a second, over the last second.
    #[field(label = "Packets per second")]
    pub packets_per_s: f64,
    /// Aircraft in the table.
    #[field(label = "Aircraft")]
    pub aircraft: i64,
    /// Aircraft with a position.
    #[field(label = "With a position")]
    pub with_position: i64,
    /// The farthest range seen.
    #[field(label = "Maximum range", unit = "km")]
    pub max_range_km: f64,
    /// FLARM packets decoded from the same capture.
    #[field(label = "FLARM packets")]
    pub flarm_packets: i64,
}

/// An aircraft heard on ADS-L.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct AdslAircraftRow<'a> {
    /// The sender of a decoded message: a registration, an address, a call sign or a code.
    #[field(shared = keys::SOURCE)]
    pub source: &'a str,
    /// Where the address of an aircraft comes from.
    #[field(label = "Source")]
    pub address_source: &'a str,
    /// The aircraft category.
    #[field(label = "Category")]
    pub category: &'a str,
    /// The flight state.
    #[field(label = "Flight state")]
    pub flight_state: &'a str,
    /// The emergency status.
    #[field(label = "Emergency")]
    pub emergency: &'a str,
    /// The position of the object that the event is about.
    #[field(shared = keys::POSITION)]
    pub position: Option<(f64, f64)>,
    /// The drone's altitude.
    #[field(label = "Altitude", unit = "m", role = Altitude)]
    pub alt_m: Option<f64>,
    /// The speed over the ground of the object that the event is about, knots.
    #[field(shared = keys::SPEED_KT)]
    pub speed_kt: Option<f64>,
    /// The course over the ground of the object that the event is about, degrees clockwise from true north.
    #[field(shared = keys::COURSE_DEG)]
    pub course_deg: Option<f64>,
    /// Climb.
    #[field(label = "Climb", unit = "m/s")]
    pub climb_m_s: Option<f64>,
    /// Level over the noise.
    #[field(label = "Level over the noise", unit = "dB")]
    pub level_db: f64,
    /// Distance from the station.
    #[field(label = "Distance", unit = "km")]
    pub distance_km: Option<f64>,
    /// Bearing from the station.
    #[field(label = "Bearing", unit = "°")]
    pub bearing_deg: Option<f64>,
    /// The radio channel.
    #[field(label = "Channel", unit = "Hz")]
    pub channel_hz: Hertz,
    /// The channel of the last packet.
    #[field(label = "Channel")]
    pub channel: &'a str,
    /// Last heard through a ground station or a relay.
    #[field(label = "Relayed")]
    pub relayed: bool,
    /// The navigation integrity category.
    #[field(label = "NIC")]
    pub nic: i64,
    /// The horizontal position accuracy category.
    #[field(label = "Horizontal accuracy")]
    pub hfom: i64,
    /// The vertical position accuracy category.
    #[field(label = "Vertical accuracy")]
    pub vfom: i64,
    /// The velocity accuracy category.
    #[field(label = "Velocity accuracy")]
    pub velocity_accuracy: i64,
    /// The capabilities of a Status packet.
    #[field(label = "Capabilities")]
    pub capabilities: &'a str,
    /// The content of the event is private (voice, encrypted traffic). Services never read its words or keep its text.
    #[field(shared = keys::PRIVATE)]
    pub private: bool,
    /// How urgent an event is. A stack sets it when the decoded message is urgent. Services alert on it.
    #[field(shared = keys::URGENCY)]
    pub urgency: Urgency,
    /// Wall-clock time when last heard, in whole seconds. A host shows the age from it.
    #[field(shared = columns::LAST_HEARD)]
    pub last_heard: Option<Time>,
    /// Times heard.
    #[field(shared = columns::HEARD)]
    pub heard: Option<i64>,
}

/// An aircraft heard on ADS-L.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct AdslAircraftEvent<'a> {
    /// The sender of a decoded message: a registration, an address, a call sign or a code.
    #[field(shared = keys::SOURCE)]
    pub source: &'a str,
    /// Where the address of an aircraft comes from.
    #[field(label = "Source")]
    pub address_source: &'a str,
    /// The aircraft category.
    #[field(label = "Category")]
    pub category: &'a str,
    /// The position of the object that the event is about.
    #[field(shared = keys::POSITION)]
    pub position: (f64, f64),
    /// The content of the event is private (voice, encrypted traffic). Services never read its words or keep its text.
    #[field(shared = keys::PRIVATE)]
    pub private: bool,
    /// What an event is about (aircraft, vessel, pager). Outputs and the store filter on it.
    #[field(shared = keys::FAMILY)]
    pub family: Family,
}

/// An ADS-L aircraft in an emergency.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct AdslEmergencyEvent<'a> {
    /// The sender of a decoded message: a registration, an address, a call sign or a code.
    #[field(shared = keys::SOURCE)]
    pub source: &'a str,
    /// The text of a decoded message.
    #[field(shared = keys::TEXT)]
    pub text: &'a str,
    /// The position of the object that the event is about.
    #[field(shared = keys::POSITION)]
    pub position: Option<(f64, f64)>,
    /// The content of the event is private (voice, encrypted traffic). Services never read its words or keep its text.
    #[field(shared = keys::PRIVATE)]
    pub private: bool,
    /// How urgent an event is. A stack sets it when the decoded message is urgent. Services alert on it.
    #[field(shared = keys::URGENCY)]
    pub urgency: Urgency,
    /// What an event is about (aircraft, vessel, pager). Outputs and the store filter on it.
    #[field(shared = keys::FAMILY)]
    pub family: Family,
}

/// The result fields of the OGN tracker.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct OgnFields<'a> {
    /// Frames (packets, blocks, bursts) that passed their check.
    #[field(shared = keys::FRAMES_OK)]
    pub frames_ok: i64,
    /// Frames that failed their check.
    #[field(shared = keys::FRAMES_BAD)]
    pub frames_bad: i64,
    /// Sync words found.
    #[field(label = "Syncs")]
    pub syncs: i64,
    /// Packets that needed a repair.
    #[field(label = "Repaired packets")]
    pub corrected: i64,
    /// Packets with an encrypted position.
    #[field(label = "Encrypted packets")]
    pub encrypted: i64,
    /// Trackers in the table.
    #[field(label = "Trackers")]
    pub trackers: i64,
    /// The channels of the plan inside the capture.
    #[field(label = "Channels received")]
    pub channels_on: i64,
    /// The channels of the plan and which are received.
    #[field(label = "Channels")]
    pub channels: &'a str,
    /// Noise floor, dBFS.
    #[field(shared = keys::NOISE_DBFS)]
    pub noise_dbfs: Option<f64>,
    /// Signal to noise ratio of what is being decoded, dB (in the signal's own bandwidth, or per symbol where the stack says so).
    #[field(shared = keys::SNR_DB)]
    pub snr_db: Option<f64>,
    /// What no public document confirms.
    #[field(label = "Note")]
    pub note: &'a str,
}

/// A packet in the packet log.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct OgnPacketRow<'a> {
    /// The signal time at the end of a packet.
    #[field(label = "Time", unit = "s")]
    pub time_s: f64,
    /// What a packet is.
    #[field(label = "Kind")]
    pub kind: PacketKind,
    /// The address and the name of its type.
    #[field(label = "Id")]
    pub id: &'a str,
    /// A packet in one line.
    #[field(label = "Decoded")]
    pub decoded: &'a str,
    /// The position of the object that the event is about.
    #[field(shared = keys::POSITION)]
    pub position: Option<(f64, f64)>,
    /// The radio channel.
    #[field(label = "Channel", unit = "Hz")]
    pub channel_hz: Hertz,
    /// The repair changed a bit of the packet.
    #[field(label = "Repaired")]
    pub repaired: bool,
    /// The 26 bytes of a packet after the repair.
    #[field(label = "Hex")]
    pub hex: &'a str,
}

/// An aircraft heard on OGN.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct OgnAircraftRow<'a> {
    /// The sender of a decoded message: a registration, an address, a call sign or a code.
    #[field(shared = keys::SOURCE)]
    pub source: &'a str,
    /// The name of the address type.
    #[field(label = "Address type")]
    pub address_type: &'a str,
    /// The aircraft type.
    #[field(label = "Aircraft type")]
    pub aircraft_type: Option<&'a str>,
    /// The position of the object that the event is about.
    #[field(shared = keys::POSITION)]
    pub position: Option<(f64, f64)>,
    /// The drone's altitude.
    #[field(label = "Altitude", unit = "m", role = Altitude)]
    pub alt_m: Option<f64>,
    /// The speed over the ground of the object that the event is about, knots.
    #[field(shared = keys::SPEED_KT)]
    pub speed_kt: Option<f64>,
    /// The course over the ground of the object that the event is about, degrees clockwise from true north.
    #[field(shared = keys::COURSE_DEG)]
    pub course_deg: Option<f64>,
    /// Climb.
    #[field(label = "Climb", unit = "m/s")]
    pub climb_m_s: Option<f64>,
    /// The turn rate.
    #[field(label = "Turn rate", unit = "°/s")]
    pub turn_deg_s: Option<f64>,
    /// The count of relays.
    #[field(label = "Relayed")]
    pub relay: i64,
    /// The sender does not want to be shown by other trackers.
    #[field(label = "Stealth")]
    pub stealth: Option<bool>,
    /// The fix quality, 0 to 3.
    #[field(label = "Fix quality")]
    pub fix_quality: Option<i64>,
    /// The second of the minute of the last position.
    #[field(label = "Second")]
    pub second: Option<i64>,
    /// Level over the noise.
    #[field(label = "Level over the noise", unit = "dB")]
    pub level_db: f64,
    /// The radio channel.
    #[field(label = "Channel", unit = "Hz")]
    pub channel_hz: Hertz,
    /// The content of the event is private (voice, encrypted traffic). Services never read its words or keep its text.
    #[field(shared = keys::PRIVATE)]
    pub private: bool,
    /// How urgent an event is. A stack sets it when the decoded message is urgent. Services alert on it.
    #[field(shared = keys::URGENCY)]
    pub urgency: Urgency,
    /// Times heard.
    #[field(shared = columns::HEARD)]
    pub heard: Option<i64>,
    /// Wall-clock time when first heard.
    #[field(shared = columns::FIRST_HEARD)]
    pub first_heard: Option<Time>,
    /// Wall-clock time when last heard, in whole seconds. A host shows the age from it.
    #[field(shared = columns::LAST_HEARD)]
    pub last_heard: Option<Time>,
}

/// An OGN tracker heard.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct OgnTrackerEvent<'a> {
    /// The sender of a decoded message: a registration, an address, a call sign or a code.
    #[field(shared = keys::SOURCE)]
    pub source: &'a str,
    /// The name of the address type.
    #[field(label = "Address type")]
    pub address_type: &'a str,
    /// The aircraft type.
    #[field(label = "Aircraft type")]
    pub aircraft_type: &'a str,
    /// The position of the object that the event is about.
    #[field(shared = keys::POSITION)]
    pub position: (f64, f64),
    /// The content of the event is private (voice, encrypted traffic). Services never read its words or keep its text.
    #[field(shared = keys::PRIVATE)]
    pub private: bool,
    /// What an event is about (aircraft, vessel, pager). Outputs and the store filter on it.
    #[field(shared = keys::FAMILY)]
    pub family: Family,
}

/// An OGN tracker in an emergency.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct OgnEmergencyEvent<'a> {
    /// The sender of a decoded message: a registration, an address, a call sign or a code.
    #[field(shared = keys::SOURCE)]
    pub source: &'a str,
    /// The text of a decoded message.
    #[field(shared = keys::TEXT)]
    pub text: &'a str,
    /// The position of the object that the event is about.
    #[field(shared = keys::POSITION)]
    pub position: Option<(f64, f64)>,
    /// The content of the event is private (voice, encrypted traffic). Services never read its words or keep its text.
    #[field(shared = keys::PRIVATE)]
    pub private: bool,
    /// How urgent an event is. A stack sets it when the decoded message is urgent. Services alert on it.
    #[field(shared = keys::URGENCY)]
    pub urgency: Urgency,
    /// What an event is about (aircraft, vessel, pager). Outputs and the store filter on it.
    #[field(shared = keys::FAMILY)]
    pub family: Family,
}

/// The result fields of the link stacks.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct LinkFields<'a> {
    /// Frames (packets, blocks, bursts) that passed their check.
    #[field(shared = keys::FRAMES_OK)]
    pub frames_ok: i64,
    /// Bursts analysed.
    #[field(label = "Bursts")]
    pub bursts: i64,
    /// Bursts without a frame.
    #[field(label = "Bursts without a frame")]
    pub bursts_without_frame: i64,
    /// 0 nothing, 1 bursts seen, 2 frames decoded lately.
    #[field(label = "State")]
    pub state: i64,
    /// Links in the table.
    #[field(label = "Links")]
    pub links_heard: i64,
    /// The span of the capture that is searched.
    #[field(label = "Searched")]
    pub covered: &'a str,
    /// Signal to noise ratio of what is being decoded, dB (in the signal's own bandwidth, or per symbol where the stack says so).
    #[field(shared = keys::SNR_DB)]
    pub snr_db: Option<f64>,
    /// What no public document confirms.
    #[field(label = "Note")]
    pub note: &'a str,
}

/// A link heard.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct HeardLinkRow<'a> {
    /// The link name.
    #[field(label = "Link")]
    pub link: &'a str,
    /// The transmitter id.
    #[field(label = "Id")]
    pub id: &'a str,
    /// Frames of a link.
    #[field(label = "Frames")]
    pub frames: i64,
    /// Wall-clock time when first heard.
    #[field(shared = columns::FIRST_HEARD)]
    pub first_heard: Option<Time>,
    /// Wall-clock time when last heard, in whole seconds. A host shows the age from it.
    #[field(shared = columns::LAST_HEARD)]
    pub last_heard: Option<Time>,
    /// The type of the last frame of a link.
    #[field(label = "Last type")]
    pub last_type: &'a str,
    /// The fields of the last frame of a link.
    #[field(label = "Last detail")]
    pub last_detail: &'a str,
    /// The position of the object that the event is about.
    #[field(shared = keys::POSITION)]
    pub position: Option<(f64, f64)>,
    /// The altitude of the last position.
    #[field(label = "Altitude", unit = "m", role = Altitude)]
    pub alt_m: Option<f64>,
}

/// A frame in the frame log.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct LinkFrameRow<'a> {
    /// The wall time of a frame.
    #[field(label = "UTC")]
    pub utc: Option<Time>,
    /// The signal time of a frame.
    #[field(label = "Time", unit = "s")]
    pub time_s: f64,
    /// The link name.
    #[field(label = "Link")]
    pub link: &'a str,
    /// The transmitter id.
    #[field(label = "Id")]
    pub id: &'a str,
    /// What a frame is.
    #[field(label = "Type")]
    pub r#type: &'a str,
    /// The fields of a frame.
    #[field(label = "Detail")]
    pub detail: &'a str,
    /// The first bytes of a frame as hex.
    #[field(label = "Bytes")]
    pub hex: &'a str,
    /// The frequency of a frame.
    #[field(label = "Frequency", unit = "Hz")]
    pub freq_hz: f64,
    /// Signal to noise ratio of what is being decoded, dB (in the signal's own bandwidth, or per symbol where the stack says so).
    #[field(shared = keys::SNR_DB)]
    pub snr_db: f64,
    /// The payload is encrypted.
    #[field(label = "Encrypted")]
    pub encrypted: bool,
}

/// A new radio-control link.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct RcLinksLinkEvent<'a> {
    /// The sender of a decoded message: a registration, an address, a call sign or a code.
    #[field(shared = keys::SOURCE)]
    pub source: &'a str,
    /// The link name.
    #[field(label = "Link")]
    pub link: &'a str,
    /// What an event is about (aircraft, vessel, pager). Outputs and the store filter on it.
    #[field(shared = keys::FAMILY)]
    pub family: Family,
}

/// A new MAVLink vehicle.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct MavlinkSikVehicleEvent<'a> {
    /// The sender of a decoded message: a registration, an address, a call sign or a code.
    #[field(shared = keys::SOURCE)]
    pub source: &'a str,
    /// The link name.
    #[field(label = "Link")]
    pub link: &'a str,
    /// The position of the object that the event is about.
    #[field(shared = keys::POSITION)]
    pub position: Option<(f64, f64)>,
    /// What an event is about (aircraft, vessel, pager). Outputs and the store filter on it.
    #[field(shared = keys::FAMILY)]
    pub family: Family,
}

/// The drones of Remote ID.
pub static REMOTE_ID_DRONES: Table<RemoteIdDroneRow<'static>> =
    Table::new("drones", "Drones", crate::remote_id::table::DRONE_CAP)
        .label(&RemoteIdDroneRow::ADDRESS);
/// The message log of Remote ID, newest first.
pub static REMOTE_ID_MESSAGES: Table<RemoteIdMessageRow<'static>> =
    Table::new("messages", "Messages", crate::remote_id::table::LOG_CAP)
        .label(&RemoteIdMessageRow::ADDRESS);
/// The track points of the Remote ID drones.
pub static REMOTE_ID_TRACKS: Table<DroneTrackRow<'static>> = Table::new(
    "drone_tracks",
    "Tracks",
    crate::remote_id::table::DRONE_CAP * crate::remote_id::table::TRAIL_CAP,
);
/// A Remote ID drone with a position.
pub static REMOTE_ID_DRONE: EventKind<RemoteIdDroneEvent<'static>> =
    EventKind::new("remote_id.drone", "Drone");
/// A change of the trust verdict of a Remote ID drone.
pub static REMOTE_ID_VERDICT: EventKind<RemoteIdVerdictEvent<'static>> =
    EventKind::new("remote_id.verdict", "Trust verdict");

/// The drone links recognised by their shape.
pub static DJI_SIGNALS: Table<DjiDroneidSignalRow> =
    Table::new("signals", "Signals", crate::dji::TRACK_CAP).sorted(
        &DjiDroneidSignalRow::SNR_DB,
        sdrkit_core::SortOrder::Descending,
    );
/// The DJI drones.
pub static DJI_DRONES: Table<DjiDroneidDroneRow<'static>> =
    Table::new("drones", "Drones", crate::dji::DRONE_CAP).label(&DjiDroneidDroneRow::SOURCE);
/// The burst log of DJI drones.
pub static DJI_BURSTS: Table<DjiDroneidBurstRow> =
    Table::new("bursts", "Bursts", crate::dji::BURST_LOG);
/// The track points of the DJI drones.
pub static DJI_TRACKS: Table<DroneTrackRow<'static>> = Table::new(
    "drone_tracks",
    "Tracks",
    crate::dji::DRONE_CAP * crate::dji::TRAIL_CAP,
);
/// A DroneID packet.
pub static DJI_DRONEID: EventKind<DjiDroneidEvent<'static>> =
    EventKind::new("dji.droneid", "DroneID packet");

/// The aircraft of FLARM.
pub static FLARM_AIRCRAFT: Table<FlarmAircraftRow<'static>> =
    Table::new("aircraft", "Aircraft", crate::flarm::AIRCRAFT_CAP).label(&FlarmAircraftRow::SOURCE);
/// The track points of the FLARM aircraft.
pub static FLARM_TRACKS: Table<AircraftTrackRow<'static>> = Table::new(
    "aircraft_tracks",
    "Tracks",
    crate::flarm::AIRCRAFT_CAP * crate::flarm::TRAIL_CAP,
);
/// An aircraft heard on FLARM.
pub static FLARM_AIRCRAFT_SEEN: EventKind<FlarmAircraftEvent<'static>> =
    EventKind::new("flarm.aircraft", "Aircraft");

/// The aircraft of ADS-L.
pub static ADSL_AIRCRAFT: Table<AdslAircraftRow<'static>> =
    Table::new("aircraft", "Aircraft", crate::adsl::AIRCRAFT_CAP).label(&AdslAircraftRow::SOURCE);
/// The track points of the ADS-L aircraft.
pub static ADSL_TRACKS: Table<AircraftTrackRow<'static>> = Table::new(
    "aircraft_tracks",
    "Tracks",
    crate::adsl::AIRCRAFT_CAP * crate::adsl::TRAIL_CAP,
);
/// An aircraft heard on ADS-L.
pub static ADSL_AIRCRAFT_SEEN: EventKind<AdslAircraftEvent<'static>> =
    EventKind::new("adsl.aircraft", "ADS-L aircraft");
/// An ADS-L aircraft in an emergency.
pub static ADSL_EMERGENCY: EventKind<AdslEmergencyEvent<'static>> =
    EventKind::new("adsl.emergency", "ADS-L emergency");

/// The packet log of OGN.
pub static OGN_PACKETS: Table<OgnPacketRow<'static>> =
    Table::new("packets", "Packets", crate::ogn::PACKET_CAP).label(&OgnPacketRow::HEX);
/// The aircraft of OGN.
pub static OGN_AIRCRAFT: Table<OgnAircraftRow<'static>> =
    Table::new("aircraft", "Aircraft", crate::ogn::AIRCRAFT_CAP).label(&OgnAircraftRow::SOURCE);
/// The track points of the OGN aircraft.
pub static OGN_TRACKS: Table<AircraftTrackRow<'static>> = Table::new(
    "aircraft_tracks",
    "Tracks",
    crate::ogn::AIRCRAFT_CAP * crate::ogn::TRAIL_CAP,
);
/// An OGN tracker heard.
pub static OGN_TRACKER: EventKind<OgnTrackerEvent<'static>> =
    EventKind::new("ogn.tracker", "OGN tracker");
/// An OGN tracker in an emergency.
pub static OGN_EMERGENCY: EventKind<OgnEmergencyEvent<'static>> =
    EventKind::new("ogn.emergency", "OGN tracker emergency");

/// The links heard.
pub static LINKS: Table<HeardLinkRow<'static>> =
    Table::new("links", "Links", crate::links::LINK_CAP).label(&HeardLinkRow::ID);
/// The frame log.
pub static FRAMES: Table<LinkFrameRow<'static>> =
    Table::new("frames", "Frames", crate::links::FRAME_CAP).label(&LinkFrameRow::HEX);
/// A new radio-control link.
pub static RC_LINKS_LINK: EventKind<RcLinksLinkEvent<'static>> =
    EventKind::new("rc_links.link", "New radio-control link");
/// The PPM frames, newest first, at most one per row interval.
pub static RC_LINKS_PPM_FRAMES: Table<RcLinksPpmFrameRow<'static>> =
    Table::new("ppm_frames", "PPM frames", 500);
/// A good PPM frame.
pub static RC_PPM_FRAME: EventKind<RcPpmFrameEvent<'static>> =
    EventKind::new("rc.ppm_frame", "PPM frame");

sdrkit_core::choice_enum! {
    /// The carrier state of a PPM pulse.
    pub enum PpmPulseState {
        /// The pulse is a notch of the carrier.
        CarrierLow = ("carrier_low", "Carrier low"),
        /// The pulse is a burst of carrier.
        CarrierHigh = ("carrier_high", "Carrier high"),
    }
}

/// The PPM fields of `rc_links` (the submode `ppm72`). The measurements have no value in the other submodes and before a
/// good frame.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct RcLinksPpmFields<'a> {
    /// The channels of the last good frame.
    #[field(label = "PPM channels")]
    pub channels: Option<i64>,
    /// The last good frame: the sync and the channels.
    #[field(label = "PPM frame", unit = "ms")]
    pub frame_ms: Option<f64>,
    /// The frames per second.
    #[field(label = "PPM frame rate", unit = "Hz")]
    pub frame_rate_hz: Option<f64>,
    /// The carrier state of the pulses.
    #[field(label = "PPM pulse state")]
    pub pulse_state: Option<PpmPulseState>,
    /// The channel times of the last good frame in ms, separated by commas.
    #[field(label = "PPM channel times", unit = "ms")]
    pub channel_ms: Option<&'a str>,
    /// The channels in percent of travel, separated by commas: (ms - centre) x 200.
    #[field(label = "PPM channels in travel", unit = "%")]
    pub channel_pct: Option<&'a str>,
    /// The good frames: as many channels as the frame before.
    #[field(label = "PPM frames good")]
    pub ppm_frames_ok: i64,
    /// The bad frames: a channel out of its window, or 1 or more than 12 channels.
    #[field(label = "PPM frames bad")]
    pub ppm_frames_bad: i64,
}

/// A row of the PPM frame table.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct RcLinksPpmFrameRow<'a> {
    /// The signal time of the sync that ended the frame.
    #[field(label = "Time", unit = "s")]
    pub time_s: Seconds,
    /// The channels.
    #[field(label = "Channels")]
    pub channels: i64,
    /// The channel times in ms, separated by commas.
    #[field(label = "Channel times", unit = "ms")]
    pub values_ms: &'a str,
    /// The frame: the sync and the channels.
    #[field(label = "Frame", unit = "ms")]
    pub frame_ms: f64,
    /// The peak over the floor of the envelope, 20 log10.
    #[field(label = "Level", unit = "dB")]
    pub level_db: f64,
}

/// A good PPM frame.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct RcPpmFrameEvent<'a> {
    /// The signal time of the sync that ended the frame.
    #[field(label = "Time", unit = "s")]
    pub time_s: Seconds,
    /// The channels.
    #[field(label = "Channels")]
    pub channels: i64,
    /// The channel times in ms, separated by commas.
    #[field(label = "Channel times", unit = "ms")]
    pub values_ms: &'a str,
    /// The frame: the sync and the channels.
    #[field(label = "Frame", unit = "ms")]
    pub frame_ms: f64,
    /// The peak over the floor of the envelope, 20 log10.
    #[field(label = "Level", unit = "dB")]
    pub level_db: f64,
}

/// A new MAVLink vehicle.
pub static MAVLINK_SIK_VEHICLE: EventKind<MavlinkSikVehicleEvent<'static>> =
    EventKind::new("mavlink_sik.vehicle", "New MAVLink vehicle");

/// The series of the level of DJI drones.
pub const LEVEL_SERIES: &str = "level";

/// The arguments of `remove` of a drone.
#[derive(Arguments)]
pub struct DroneArgs<'a> {
    /// The row key of the drone.
    #[arg(shared = DRONE_ROW)]
    pub row: &'a str,
}
/// The row key of a drone.
pub const DRONE_ROW: Argument<field::Text> = objcmd::row_of("drones");
/// Forgets one drone.
pub const DRONE_REMOVE: CommandKind<DroneArgs<'static>> = objcmd::remove_kind();

/// The arguments of `remove` of an aircraft.
#[derive(Arguments)]
pub struct AircraftArgs<'a> {
    /// The row key of the aircraft.
    #[arg(shared = AIRCRAFT_ROW)]
    pub row: &'a str,
}
/// The row key of an aircraft.
pub const AIRCRAFT_ROW: Argument<field::Text> = objcmd::row_of("aircraft");
/// Forgets one aircraft.
pub const AIRCRAFT_REMOVE: CommandKind<AircraftArgs<'static>> = objcmd::remove_kind();

/// The arguments of `remove` of a link.
#[derive(Arguments)]
pub struct LinkArgs<'a> {
    /// The row key of the link.
    #[arg(shared = LINK_ROW)]
    pub row: &'a str,
}
/// The row key of a link.
pub const LINK_ROW: Argument<field::Text> = objcmd::row_of("links");
/// Forgets one link.
pub const LINK_REMOVE: CommandKind<LinkArgs<'static>> = objcmd::remove_kind();

/// The interface of the family.
pub fn family() -> &'static Described {
    static FAMILY: OnceLock<Described> = OnceLock::new();
    FAMILY.get_or_init(|| {
        // Each stack holds what it heard in one component: the objects heard and the logs are its history, and they
        // stay over a change of frequency, with the counters of the same component.
        Described::new("drones", 1, 2)
            .added(
                1,
                1,
                &[
                    "remote_id.content",
                    "dji_droneid.content",
                    "flarm.content",
                    "adsl.content",
                    "ogn.content",
                    "rc_links.content",
                    "mavlink_sik.content",
                ],
            )
            .added(
                1,
                2,
                &[
                    "shape.RemoteIdDroneRow.verdict",
                    "shape.RemoteIdDroneRow.reasons",
                    "shape.RemoteIdFields.flagged",
                    "shape.RemoteIdFields.messages_refused",
                    "shape.RemoteIdFields.fields_out_of_range",
                    "shape.RemoteIdDroneRow.authentication_length",
                    "shape.RemoteIdDroneRow.authentication_complete",
                    "shape.RemoteIdVerdictEvent",
                    "shape.RemoteIdVerdictEvent.source",
                    "shape.RemoteIdVerdictEvent.address",
                    "shape.RemoteIdVerdictEvent.verdict",
                    "shape.RemoteIdVerdictEvent.reasons",
                    "shape.RemoteIdVerdictEvent.position",
                    "shape.RemoteIdVerdictEvent.urgency",
                    "shape.RemoteIdVerdictEvent.family",
                    "remote_id.event.remote_id.verdict",
                    "remote_id.setting.gen_spoofer",
                    "remote_id.setting.shared_radio",
                    "shape.DjiDroneidDroneRow.alt_raw",
                    "shape.DjiDroneidDroneRow.height_raw",
                    "shape.DjiDroneidEvent.alt_raw",
                    "shape.DjiDroneidEvent.height_raw",
                ],
            )
            .content(
                sdrkit_core::ContentKind::Tracking,
                &[
                    "remote_id",
                    "dji_droneid",
                    "flarm",
                    "adsl",
                    "ogn",
                    "mavlink_sik",
                ],
            )
            .content(sdrkit_core::ContentKind::Networks, &["rc_links"])
            .stack(
                StackShape::new("remote_id")
                    .fields::<RemoteIdFields>()
                    .table(&REMOTE_ID_DRONES)
                    .table(&REMOTE_ID_MESSAGES)
                    .table(&REMOTE_ID_TRACKS)
                    .points("Drones")
                    .event(&REMOTE_ID_DRONE)
                    .event(&REMOTE_ID_VERDICT)
                    .settings(RidSetting::SETTINGS)
                    .command(&objcmd::CLEAR)
                    .command(&DRONE_REMOVE)
                    .kept(&["drones", "messages", "drone_tracks", "RemoteIdFields"]),
            )
            .stack(
                StackShape::new("dji_droneid")
                    .fields::<DjiDroneidFields>()
                    .table(&DJI_SIGNALS)
                    .table(&DJI_DRONES)
                    .table(&DJI_BURSTS)
                    .table(&DJI_TRACKS)
                    .series(SeriesDef {
                        key: LEVEL_SERIES,
                        label: "Level",
                        x_unit: "s",
                        y_unit: "dBFS",
                    })
                    .points("Drones")
                    .event(&DJI_DRONEID)
                    .settings(DjiSetting::SETTINGS)
                    .command(&objcmd::CLEAR)
                    .command(&DRONE_REMOVE)
                    .kept(&[
                        "signals",
                        "drones",
                        "bursts",
                        "drone_tracks",
                        "DjiDroneidFields",
                    ]),
            )
            .stack(
                StackShape::new("flarm")
                    .fields::<FlarmFields>()
                    .table(&FLARM_AIRCRAFT)
                    .table(&FLARM_TRACKS)
                    .points("Aircraft")
                    .event(&FLARM_AIRCRAFT_SEEN)
                    .settings(FlarmSetting::SETTINGS)
                    .command(&objcmd::CLEAR)
                    .command(&AIRCRAFT_REMOVE)
                    .kept(&["aircraft", "aircraft_tracks", "FlarmFields"]),
            )
            .stack(
                StackShape::new("adsl")
                    .fields::<AdslFields>()
                    .table(&ADSL_AIRCRAFT)
                    .table(&ADSL_TRACKS)
                    .points("Aircraft")
                    .event(&ADSL_AIRCRAFT_SEEN)
                    .event(&ADSL_EMERGENCY)
                    .settings(AdslSetting::SETTINGS)
                    .command(&objcmd::CLEAR)
                    .command(&AIRCRAFT_REMOVE)
                    .kept(&["aircraft", "aircraft_tracks", "AdslFields"]),
            )
            .stack(
                StackShape::new("ogn")
                    .fields::<OgnFields>()
                    .table(&OGN_PACKETS)
                    .table(&OGN_AIRCRAFT)
                    .table(&OGN_TRACKS)
                    .points("Aircraft")
                    .event(&OGN_TRACKER)
                    .event(&OGN_EMERGENCY)
                    .settings(OgnSetting::SETTINGS)
                    .command(&objcmd::CLEAR)
                    .command(&AIRCRAFT_REMOVE)
                    .kept(&["packets", "aircraft", "aircraft_tracks", "OgnFields"]),
            )
            .stack(
                links_stack("rc_links")
                    .fields::<RcLinksPpmFields>()
                    .table(&RC_LINKS_PPM_FRAMES)
                    .event(&RC_LINKS_LINK)
                    .event(&RC_PPM_FRAME)
                    .settings(RcStackSetting::SETTINGS)
                    .command(&sdrkit_graph::decoder::CLEAR)
                    .command(&LINK_REMOVE)
                    .kept(&[
                        "links",
                        "frames",
                        "LinkFields",
                        "DecoderFields",
                        "ppm_frames",
                    ]),
            )
            .stack(
                links_stack("mavlink_sik")
                    .points("Vehicles")
                    .event(&MAVLINK_SIK_VEHICLE)
                    .settings(SikSetting::SETTINGS)
                    .command(&sdrkit_graph::decoder::CLEAR)
                    .command(&LINK_REMOVE)
                    .kept(&["links", "frames", "LinkFields", "DecoderFields"]),
            )
    })
}

/// What the two link stacks share: the result sets, the link table and the frame log.
fn links_stack(id: &'static str) -> StackShape {
    StackShape::new(id)
        .fields::<LinkFields>()
        .fields::<DecoderFields>()
        .table(&LINKS)
        .table(&FRAMES)
}

/// The description of the stack `id`.
///
/// # Panics
///
/// If the family does not describe the stack.
pub fn shape(id: &str) -> &'static StackShape {
    family()
        .stack_shape(id)
        .unwrap_or_else(|| panic!("the drones family does not describe the stack {id}"))
}
