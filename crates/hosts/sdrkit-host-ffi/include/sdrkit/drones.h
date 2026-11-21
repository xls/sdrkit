/*
 * sdrkit: the typed interface of the stack family "drones", version 1.0. MIT licence.
 *
 * Written from the interface description of the family. Do not edit.
 *
 * Structs. Each struct starts with `size`. A call that fills a struct writes at most the size of the struct that
 * the caller was compiled with, and sets `size` to the number of bytes that it wrote. A struct only grows at its
 * end: a library that is newer than the caller fills the members that the caller knows, and a library that is
 * older leaves the members that it does not know at 0, with `size` below the size of the struct. The caller
 * sets `size` of an arguments struct to its sizeof before the call: use the *_INIT initialiser.
 *
 * Values. A member with a `has_` member before it holds a value only if the `has_` member is true. A string or
 * bytes member that the library writes is a view into a buffer of the handle, valid until the next call on the
 * same handle. A member of an enumeration holds a value of the enum that its comment names.
 *
 * Versions. The minor version rises when the family gets a new stack, table, member, setting, command or event
 * type. The major version rises when one is removed or renamed, or its type or unit changes.
 * sdrkit_family_version() returns the version of the family in the library, or SDRKIT_E_NO_SUCH_KEY if the
 * library was built without the family. sdrkit_object_find() with "stack.<id>" finds out if an engine has a
 * stack.
 *
 * Results. Take a snapshot with sdrkit_snapshot_read() on the handle of a session, then call the *_rows() and
 * *_read() functions on the same handle. Events. Take the events with sdrkit_events_take(), then call the
 * *_event_read() function of the type of each event; it returns SDRKIT_E_WRONG_TYPE for an event of another type.
 * Settings and commands. Call the *_get_*(), *_set_*() and command functions on the handle of a session of the
 * stack; the getters also take the handle of the stack type ("stack.<id>") and return the defaults. One call
 * sets one setting. A function returns SDRKIT_E_WRONG_HANDLE if the object of the handle does not have the
 * table, the fields, the setting or the command with the types of this header.
 */

#ifndef SDRKIT_DRONES_H
#define SDRKIT_DRONES_H

#include "../sdrkit.h"

// The version of the family that this header describes.
#define SDRKIT_DRONES_VERSION_MAJOR 1
#define SDRKIT_DRONES_VERSION_MINOR 0

// The stack "remote_id": its id, and the keys of its event types.
#define SDRKIT_STACK_REMOTE_ID "remote_id"
#define SDRKIT_EVENT_REMOTE_ID_DRONE "remote_id.drone"

// The stack "dji_droneid": its id, and the keys of its event types.
#define SDRKIT_STACK_DJI_DRONEID "dji_droneid"
#define SDRKIT_EVENT_DJI_DRONEID "dji.droneid"

// The stack "flarm": its id, and the keys of its event types.
#define SDRKIT_STACK_FLARM "flarm"
#define SDRKIT_EVENT_FLARM_AIRCRAFT "flarm.aircraft"

// The stack "adsl": its id, and the keys of its event types.
#define SDRKIT_STACK_ADSL "adsl"
#define SDRKIT_EVENT_ADSL_AIRCRAFT "adsl.aircraft"
#define SDRKIT_EVENT_ADSL_EMERGENCY "adsl.emergency"

// The stack "ogn": its id, and the keys of its event types.
#define SDRKIT_STACK_OGN "ogn"
#define SDRKIT_EVENT_OGN_TRACKER "ogn.tracker"
#define SDRKIT_EVENT_OGN_EMERGENCY "ogn.emergency"

// The stack "rc_links": its id, and the keys of its event types.
#define SDRKIT_STACK_RC_LINKS "rc_links"
#define SDRKIT_EVENT_RC_LINKS_LINK "rc_links.link"
#define SDRKIT_EVENT_RC_PPM_FRAME "rc.ppm_frame"

// The stack "mavlink_sik": its id, and the keys of its event types.
#define SDRKIT_STACK_MAVLINK_SIK "mavlink_sik"
#define SDRKIT_EVENT_MAVLINK_SIK_VEHICLE "mavlink_sik.vehicle"

// The options of the setting "band" of the stack "remote_id".
typedef enum sdrkit_remote_id_band {
  // BLE 38 + Wi-Fi 6 (2432 MHz)
  SDRKIT_REMOTE_ID_BAND_BLE_38_WI_FI_6 = 0,
  // Wi-Fi 6 (2437 MHz)
  SDRKIT_REMOTE_ID_BAND_WI_FI_6 = 1,
  // Wi-Fi 1 (2412 MHz)
  SDRKIT_REMOTE_ID_BAND_WI_FI_1 = 2,
  // Wi-Fi 11 (2462 MHz)
  SDRKIT_REMOTE_ID_BAND_WI_FI_11 = 3,
  // BLE 37 + Wi-Fi 1 (2403 MHz)
  SDRKIT_REMOTE_ID_BAND_BLE_37_WI_FI_1 = 4,
  // BLE 38 (2426 MHz)
  SDRKIT_REMOTE_ID_BAND_BLE_38 = 5,
  // BLE 39 (2478 MHz)
  SDRKIT_REMOTE_ID_BAND_BLE_39 = 6,
} sdrkit_remote_id_band;

// The options of the setting "gen_carriers" of the stack "remote_id".
typedef enum sdrkit_remote_id_gen_carriers {
  // both
  SDRKIT_REMOTE_ID_GEN_CARRIERS_BOTH = 0,
  // BLE only
  SDRKIT_REMOTE_ID_GEN_CARRIERS_BLE = 1,
  // Wi-Fi only
  SDRKIT_REMOTE_ID_GEN_CARRIERS_WIFI = 2,
} sdrkit_remote_id_gen_carriers;

// The options of the setting "gen_wifi_phy" of the stack "remote_id".
typedef enum sdrkit_remote_id_gen_wifi_phy {
  // DSSS 1 Mbit/s
  SDRKIT_REMOTE_ID_GEN_WIFI_PHY_DSSS = 0,
  // OFDM 6 Mbit/s
  SDRKIT_REMOTE_ID_GEN_WIFI_PHY_OFDM = 1,
} sdrkit_remote_id_gen_wifi_phy;

// The options of the setting "gen_scenario" of the stack "dji_droneid".
typedef enum sdrkit_dji_droneid_gen_scenario {
  // everything
  SDRKIT_DJI_DRONEID_GEN_SCENARIO_EVERYTHING = 0,
  // DroneID only
  SDRKIT_DJI_DRONEID_GEN_SCENARIO_DRONEID_ONLY = 1,
  // OcuSync-like only
  SDRKIT_DJI_DRONEID_GEN_SCENARIO_OCUSYNC_LIKE_ONLY = 2,
  // Lightbridge-like only
  SDRKIT_DJI_DRONEID_GEN_SCENARIO_LIGHTBRIDGE_LIKE_ONLY = 3,
  // Wi-Fi-like only
  SDRKIT_DJI_DRONEID_GEN_SCENARIO_WI_FI_LIKE_ONLY = 4,
  // DroneID with collisions
  SDRKIT_DJI_DRONEID_GEN_SCENARIO_DRONEID_WITH_COLLISIONS = 5,
} sdrkit_dji_droneid_gen_scenario;

// The options of the setting "submode" of the stack "rc_links".
typedef enum sdrkit_rc_links_submode {
  // All
  SDRKIT_RC_LINKS_SUBMODE_ALL = 0,
  // FrSky
  SDRKIT_RC_LINKS_SUBMODE_FRSKY = 1,
  // FlySky
  SDRKIT_RC_LINKS_SUBMODE_FLYSKY = 2,
  // Futaba S-FHSS
  SDRKIT_RC_LINKS_SUBMODE_SFHSS = 3,
  // Spektrum DSM
  SDRKIT_RC_LINKS_SUBMODE_DSM = 4,
  // RC model PPM (26 to 75 MHz)
  SDRKIT_RC_LINKS_SUBMODE_PPM72 = 5,
} sdrkit_rc_links_submode;

// The options of the setting "flysky_bit_order" of the stack "rc_links".
typedef enum sdrkit_rc_links_flysky_bit_order {
  // Both
  SDRKIT_RC_LINKS_FLYSKY_BIT_ORDER_BOTH = 0,
  // Most significant first
  SDRKIT_RC_LINKS_FLYSKY_BIT_ORDER_MSB = 1,
  // Least significant first
  SDRKIT_RC_LINKS_FLYSKY_BIT_ORDER_LSB = 2,
} sdrkit_rc_links_flysky_bit_order;

// The options of the setting "dsm_chip_polarity" of the stack "rc_links".
typedef enum sdrkit_rc_links_dsm_chip_polarity {
  // 1 chip on the higher frequency
  SDRKIT_RC_LINKS_DSM_CHIP_POLARITY_HIGH = 0,
  // 1 chip on the lower frequency
  SDRKIT_RC_LINKS_DSM_CHIP_POLARITY_LOW = 1,
} sdrkit_rc_links_dsm_chip_polarity;

// The options of the setting "sfhss_crc" of the stack "rc_links".
typedef enum sdrkit_rc_links_sfhss_crc {
  // Either
  SDRKIT_RC_LINKS_SFHSS_CRC_EITHER = 0,
  // CRC-16/IBM-3740
  SDRKIT_RC_LINKS_SFHSS_CRC_CCITT = 1,
  // CRC-16/CMS
  SDRKIT_RC_LINKS_SFHSS_CRC_CC2500 = 2,
} sdrkit_rc_links_sfhss_crc;

// The options of the setting "air_rate" of the stack "mavlink_sik".
typedef enum sdrkit_mavlink_sik_air_rate {
  // Auto
  SDRKIT_MAVLINK_SIK_AIR_RATE_AUTO = 0,
  // 2
  SDRKIT_MAVLINK_SIK_AIR_RATE_2 = 1,
  // 4
  SDRKIT_MAVLINK_SIK_AIR_RATE_4 = 2,
  // 8
  SDRKIT_MAVLINK_SIK_AIR_RATE_8 = 3,
  // 16
  SDRKIT_MAVLINK_SIK_AIR_RATE_16 = 4,
  // 19
  SDRKIT_MAVLINK_SIK_AIR_RATE_19 = 5,
  // 24
  SDRKIT_MAVLINK_SIK_AIR_RATE_24 = 6,
  // 32
  SDRKIT_MAVLINK_SIK_AIR_RATE_32 = 7,
  // 48
  SDRKIT_MAVLINK_SIK_AIR_RATE_48 = 8,
  // 64
  SDRKIT_MAVLINK_SIK_AIR_RATE_64 = 9,
  // 96
  SDRKIT_MAVLINK_SIK_AIR_RATE_96 = 10,
  // 128
  SDRKIT_MAVLINK_SIK_AIR_RATE_128 = 11,
  // 192
  SDRKIT_MAVLINK_SIK_AIR_RATE_192 = 12,
  // 250
  SDRKIT_MAVLINK_SIK_AIR_RATE_250 = 13,
} sdrkit_mavlink_sik_air_rate;

// The options of `band` of `sdrkit_remote_id_fields`.
typedef enum sdrkit_remote_id_fields_band {
  // BLE 38 + Wi-Fi 6 (2432 MHz)
  SDRKIT_REMOTE_ID_FIELDS_BAND_BLE_38_WI_FI_6 = 0,
  // Wi-Fi 6 (2437 MHz)
  SDRKIT_REMOTE_ID_FIELDS_BAND_WI_FI_6 = 1,
  // Wi-Fi 1 (2412 MHz)
  SDRKIT_REMOTE_ID_FIELDS_BAND_WI_FI_1 = 2,
  // Wi-Fi 11 (2462 MHz)
  SDRKIT_REMOTE_ID_FIELDS_BAND_WI_FI_11 = 3,
  // BLE 37 + Wi-Fi 1 (2403 MHz)
  SDRKIT_REMOTE_ID_FIELDS_BAND_BLE_37_WI_FI_1 = 4,
  // BLE 38 (2426 MHz)
  SDRKIT_REMOTE_ID_FIELDS_BAND_BLE_38 = 5,
  // BLE 39 (2478 MHz)
  SDRKIT_REMOTE_ID_FIELDS_BAND_BLE_39 = 6,
} sdrkit_remote_id_fields_band;

// The result fields of Drone Remote ID.
typedef struct sdrkit_remote_id_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // True if `ble_channel` holds a value.
  bool has_ble_channel;
  // The BLE advertising channel in the band (0 for a file without a centre).
  int64_t ble_channel;
  // True if `ble_offset_hz` holds a value.
  bool has_ble_offset_hz;
  // The offset of the BLE advertising channel from the centre. A frequency in Hz.
  double ble_offset_hz;
  // True if `wifi_channel` holds a value.
  bool has_wifi_channel;
  // The Wi-Fi channel in the band (0 for a file without a centre).
  int64_t wifi_channel;
  // The offset of the Wi-Fi channel from the centre. A frequency in Hz.
  double wifi_offset_hz;
  // True if `band` holds a value.
  bool has_band;
  // The band of the present centre, when it is one of the bands that the stack offers. A value of `sdrkit_remote_id_fields_band`.
  int32_t band;
  // Drones in the table.
  int64_t drones;
  // Messages decoded.
  int64_t messages;
  // Good BLE packets.
  int64_t ble_good;
  // Good BLE packets with Remote ID.
  int64_t ble_remote_id;
  // Good Wi-Fi frames.
  int64_t wifi_good;
  // Good Wi-Fi frames with Remote ID.
  int64_t wifi_remote_id;
  // Frames that failed their check.
  int64_t frames_bad;
  // Basic ID messages (a field: all drones; a column: one drone).
  int64_t basic_id_messages;
  // Location messages.
  int64_t location_messages;
  // Authentication messages.
  int64_t authentication_messages;
  // Self-ID messages.
  int64_t self_id_messages;
  // System messages.
  int64_t system_messages;
  // Operator ID messages.
  int64_t operator_id_messages;
  // True if `snr_db` holds a value.
  bool has_snr_db;
  // Signal to noise ratio of what is being decoded, dB (in the signal's own bandwidth, or per symbol where the stack says so). Unit: dB.
  double snr_db;
  // True if `offset_hz` holds a value.
  bool has_offset_hz;
  // Carrier offset from where it was expected, Hz. A frequency in Hz.
  double offset_hz;
  // True if `level_dbfs` holds a value.
  bool has_level_dbfs;
  // Level of the signal or channel, dBFS (a mean power of 1 is 0 dBFS). Unit: dBFS.
  double level_dbfs;
} sdrkit_remote_id_fields;

// The options of `via` of `sdrkit_remote_id_drone_row`.
typedef enum sdrkit_remote_id_drone_row_via {
  // BLE
  SDRKIT_REMOTE_ID_DRONE_ROW_VIA_BLE = 0,
  // Wi-Fi
  SDRKIT_REMOTE_ID_DRONE_ROW_VIA_WIFI = 1,
  // BLE + Wi-Fi
  SDRKIT_REMOTE_ID_DRONE_ROW_VIA_BOTH = 2,
} sdrkit_remote_id_drone_row_via;

// A drone heard on Remote ID. The row key is the broadcast address. The source is the shown ID: the serial number, else the session ID, else the CAA registration, else the UTM UUID, else the address.
typedef struct sdrkit_remote_id_drone_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The sender of a decoded message: a registration, an address, a call sign or a code.
  sdrkit_string source;
  // The broadcast address.
  sdrkit_string address;
  // True if `serial` holds a value.
  bool has_serial;
  // Serial number.
  sdrkit_string serial;
  // True if `session_id` holds a value.
  bool has_session_id;
  // Session ID.
  sdrkit_string session_id;
  // True if `caa_registration` holds a value.
  bool has_caa_registration;
  // CAA registration.
  sdrkit_string caa_registration;
  // True if `utm_uuid` holds a value.
  bool has_utm_uuid;
  // UTM assigned UUID.
  sdrkit_string utm_uuid;
  // True if `ua_type` holds a value.
  bool has_ua_type;
  // UA type code (ASTM F3411-22a, Basic ID message).
  int64_t ua_type;
  // True if `status` holds a value.
  bool has_status;
  // Operational status code (ASTM F3411-22a, Location message).
  int64_t status;
  // True if `position` holds a value.
  bool has_position;
  // The position of the object that the event is about.
  sdrkit_position position;
  // True if `alt_geo_m` holds a value.
  bool has_alt_geo_m;
  // Geodetic altitude. Unit: m.
  double alt_geo_m;
  // True if `alt_baro_m` holds a value.
  bool has_alt_baro_m;
  // Pressure altitude. Unit: m.
  double alt_baro_m;
  // True if `height_m` holds a value.
  bool has_height_m;
  // Height above take-off or ground, as the column `height_type` says. Unit: m.
  double height_m;
  // True if `height_type` holds a value.
  bool has_height_type;
  // What the height is above: 0 take-off, 1 ground.
  int64_t height_type;
  // True if `speed_kt` holds a value.
  bool has_speed_kt;
  // The speed over the ground of the object that the event is about, knots. Unit: kt.
  double speed_kt;
  // True if `vspeed_m_s` holds a value.
  bool has_vspeed_m_s;
  // Vertical speed, up. Unit: m/s.
  double vspeed_m_s;
  // True if `course_deg` holds a value.
  bool has_course_deg;
  // The course over the ground of the object that the event is about, degrees clockwise from true north. Unit: °.
  double course_deg;
  // True if `horiz_acc` holds a value.
  bool has_horiz_acc;
  // Horizontal accuracy code.
  int64_t horiz_acc;
  // True if `vert_acc` holds a value.
  bool has_vert_acc;
  // Vertical accuracy code.
  int64_t vert_acc;
  // True if `speed_acc` holds a value.
  bool has_speed_acc;
  // Speed accuracy code.
  int64_t speed_acc;
  // True if `timestamp_ds` holds a value.
  bool has_timestamp_ds;
  // Tenths of a second since the hour. Unit: 0.1 s.
  int64_t timestamp_ds;
  // True if `operator_position` holds a value.
  bool has_operator_position;
  // The operator's position.
  sdrkit_position operator_position;
  // True if `operator_alt_m` holds a value.
  bool has_operator_alt_m;
  // The operator's altitude. Unit: m.
  double operator_alt_m;
  // True if `location_type` holds a value.
  bool has_location_type;
  // Operator location type code.
  int64_t location_type;
  // True if `classification` holds a value.
  bool has_classification;
  // Classification type code.
  int64_t classification;
  // True if `category` holds a value.
  bool has_category;
  // EU category code.
  int64_t category;
  // True if `class_` holds a value.
  bool has_class_;
  // EU class code (1 to 7 are C0 to C6).
  int64_t class_;
  // True if `system_timestamp_s` holds a value.
  bool has_system_timestamp_s;
  // System time stamp, seconds since 2019-01-01. Unit: s.
  int64_t system_timestamp_s;
  // True if `operator_id` holds a value.
  bool has_operator_id;
  // Operator ID.
  sdrkit_string operator_id;
  // True if `self_id` holds a value.
  bool has_self_id;
  // Self-ID text.
  sdrkit_string self_id;
  // True if `authentication` holds a value.
  bool has_authentication;
  // Authentication type code.
  int64_t authentication;
  // True if `authentication_pages` holds a value.
  bool has_authentication_pages;
  // Authentication pages.
  int64_t authentication_pages;
  // Basic ID messages (a field: all drones; a column: one drone).
  int64_t basic_id_messages;
  // Location messages.
  int64_t location_messages;
  // Authentication messages.
  int64_t authentication_messages;
  // Self-ID messages.
  int64_t self_id_messages;
  // System messages.
  int64_t system_messages;
  // Operator ID messages.
  int64_t operator_id_messages;
  // Trail points.
  int64_t track_points;
  // How the drone was heard. A value of `sdrkit_remote_id_drone_row_via`.
  int32_t via;
  // True if `level_dbfs` holds a value.
  bool has_level_dbfs;
  // Level of the signal or channel, dBFS (a mean power of 1 is 0 dBFS). Unit: dBFS.
  double level_dbfs;
  // True if `last_heard` holds a value.
  bool has_last_heard;
  // Wall-clock time when last heard, in whole seconds. A host shows the age from it. Seconds since 1970 (UTC).
  double last_heard;
  // True if `heard` holds a value.
  bool has_heard;
  // Times heard.
  int64_t heard;
} sdrkit_remote_id_drone_row;

// The options of `via` of `sdrkit_remote_id_message_row`.
typedef enum sdrkit_remote_id_message_row_via {
  // BLE
  SDRKIT_REMOTE_ID_MESSAGE_ROW_VIA_BLE = 0,
  // Wi-Fi
  SDRKIT_REMOTE_ID_MESSAGE_ROW_VIA_WIFI = 1,
} sdrkit_remote_id_message_row_via;

// The options of `type` of `sdrkit_remote_id_message_row`.
typedef enum sdrkit_remote_id_message_row_type {
  // Basic ID
  SDRKIT_REMOTE_ID_MESSAGE_ROW_TYPE_BASIC_ID = 0,
  // Location
  SDRKIT_REMOTE_ID_MESSAGE_ROW_TYPE_LOCATION = 1,
  // Authentication
  SDRKIT_REMOTE_ID_MESSAGE_ROW_TYPE_AUTHENTICATION = 2,
  // Self-ID
  SDRKIT_REMOTE_ID_MESSAGE_ROW_TYPE_SELF_ID = 3,
  // System
  SDRKIT_REMOTE_ID_MESSAGE_ROW_TYPE_SYSTEM = 4,
  // Operator ID
  SDRKIT_REMOTE_ID_MESSAGE_ROW_TYPE_OPERATOR_ID = 5,
} sdrkit_remote_id_message_row_type;

// A line of the message log. The row key is the sequence number of the line.
typedef struct sdrkit_remote_id_message_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // Signal time of a log line. Unit: s.
  double time_s;
  // True if `time` holds a value.
  bool has_time;
  // Wall time of a log line. Seconds since 1970 (UTC).
  double time;
  // The carrier of a log line. A value of `sdrkit_remote_id_message_row_via`.
  int32_t via;
  // True if `radio_channel` holds a value.
  bool has_radio_channel;
  // The radio channel of a log line (BLE 37 to 39, Wi-Fi 1 to 13; none when unknown).
  int64_t radio_channel;
  // The broadcast address.
  sdrkit_string address;
  // True if `level_dbfs` holds a value.
  bool has_level_dbfs;
  // Level of the signal or channel, dBFS (a mean power of 1 is 0 dBFS). Unit: dBFS.
  double level_dbfs;
  // The message type. A value of `sdrkit_remote_id_message_row_type`.
  int32_t type;
  // The summary line of a message.
  sdrkit_string message;
} sdrkit_remote_id_message_row;

// A track point of a drone. The row key is the drone key and the signal time of the point.
typedef struct sdrkit_drones_drone_track_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The drone a track point belongs to.
  sdrkit_string drone;
  // The position of the object that the event is about.
  sdrkit_position position;
  // The signal time of a track point. Unit: s.
  double time_s;
} sdrkit_drones_drone_track_row;

// The result fields of DJI drones.
typedef struct sdrkit_dji_droneid_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Bursts measured.
  int64_t bursts_measured;
  // OFDM-like bursts.
  int64_t bursts_ofdm_like;
  // DroneID packets decoded.
  int64_t droneid_decoded;
  // DroneID bursts with the symbols whose CRC failed.
  int64_t droneid_crc_failed;
  // DroneID attempts.
  int64_t droneid_tried;
  // True if `centre_hz` holds a value.
  bool has_centre_hz;
  // The capture centre. A frequency in Hz.
  double centre_hz;
  // True if `burst_cfo_hz` holds a value.
  bool has_burst_cfo_hz;
  // The carrier offset of the last DroneID burst from the 0.5 MHz grid. A frequency in Hz.
  double burst_cfo_hz;
  // Drones in the table.
  int64_t drones;
  // True if `signal` holds a value.
  bool has_signal;
  // The track with the most likely DJI shape: a row key of the `signals` table.
  sdrkit_string signal;
  // True if `noise_dbfs` holds a value.
  bool has_noise_dbfs;
  // Noise floor, dBFS. Unit: dBFS.
  double noise_dbfs;
  // True if `level_dbfs` holds a value.
  bool has_level_dbfs;
  // Level of the signal or channel, dBFS (a mean power of 1 is 0 dBFS). Unit: dBFS.
  double level_dbfs;
  // True if `snr_db` holds a value.
  bool has_snr_db;
  // Signal to noise ratio of what is being decoded, dB (in the signal's own bandwidth, or per symbol where the stack says so). Unit: dB.
  double snr_db;
  // True if `offset_hz` holds a value.
  bool has_offset_hz;
  // Carrier offset from where it was expected, Hz. A frequency in Hz.
  double offset_hz;
} sdrkit_dji_droneid_fields;

// The options of `class_` of `sdrkit_dji_droneid_signal_row`.
typedef enum sdrkit_dji_droneid_signal_row_class_ {
  // other wideband (not OFDM-like)
  SDRKIT_DJI_DRONEID_SIGNAL_ROW_CLASS__OTHER_WIDEBAND = 0,
  // unknown wideband OFDM
  SDRKIT_DJI_DRONEID_SIGNAL_ROW_CLASS__UNKNOWN_OFDM = 1,
  // DJI OcuSync-like
  SDRKIT_DJI_DRONEID_SIGNAL_ROW_CLASS__OCUSYNC_LIKE = 2,
  // DJI Lightbridge-like
  SDRKIT_DJI_DRONEID_SIGNAL_ROW_CLASS__LIGHTBRIDGE_LIKE = 3,
  // DJI DroneID
  SDRKIT_DJI_DRONEID_SIGNAL_ROW_CLASS__DRONEID = 4,
} sdrkit_dji_droneid_signal_row_class_;

// The options of `band` of `sdrkit_dji_droneid_signal_row`.
typedef enum sdrkit_dji_droneid_signal_row_band {
  // 2.4 GHz
  SDRKIT_DJI_DRONEID_SIGNAL_ROW_BAND_2G4 = 0,
  // 5.8 GHz
  SDRKIT_DJI_DRONEID_SIGNAL_ROW_BAND_5G8 = 1,
  // 5.2 GHz
  SDRKIT_DJI_DRONEID_SIGNAL_ROW_BAND_5G2 = 2,
} sdrkit_dji_droneid_signal_row_band;

// The options of `reason` of `sdrkit_dji_droneid_signal_row`.
typedef enum sdrkit_dji_droneid_signal_row_reason {
  // DroneID packet decoded (the CRC-24 passed): an identification, not a guess
  SDRKIT_DJI_DRONEID_SIGNAL_ROW_REASON_DECODED = 0,
  // the two DroneID Zadoff-Chu symbols were found; the packet did not pass its CRC yet
  SDRKIT_DJI_DRONEID_SIGNAL_ROW_REASON_ZC_ONLY = 1,
  // OFDM bursts every 1 ms, 10 or 20 MHz wide
  SDRKIT_DJI_DRONEID_SIGNAL_ROW_REASON_EVERY_MS = 2,
  // periodic OFDM bursts, 10 or 20 MHz wide, 15 kHz-type spacing
  SDRKIT_DJI_DRONEID_SIGNAL_ROW_REASON_PERIODIC = 3,
  // OFDM bursts with a period outside the OcuSync / Lightbridge range
  SDRKIT_DJI_DRONEID_SIGNAL_ROW_REASON_OTHER_PERIOD = 4,
  // wideband OFDM-like bursts, not regular enough to tell more
  SDRKIT_DJI_DRONEID_SIGNAL_ROW_REASON_IRREGULAR = 5,
  // not enough bursts yet
  SDRKIT_DJI_DRONEID_SIGNAL_ROW_REASON_TOO_FEW = 6,
  // wideband bursts that do not look like 15 / 30 kHz OFDM (Wi-Fi and the like)
  SDRKIT_DJI_DRONEID_SIGNAL_ROW_REASON_NOT_OFDM = 7,
} sdrkit_dji_droneid_signal_row_reason;

// A drone link recognised by its shape.
typedef struct sdrkit_dji_droneid_signal_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The track number.
  int64_t track;
  // The class of a track. A value of `sdrkit_dji_droneid_signal_row_class_`.
  int32_t class_;
  // The confidence of the class. Unit: %.
  double confidence_pct;
  // True if `freq_hz` holds a value.
  bool has_freq_hz;
  // The radio frequency. A frequency in Hz.
  double freq_hz;
  // Carrier offset from where it was expected, Hz. A frequency in Hz.
  double offset_hz;
  // The width. A frequency in Hz.
  double width_hz;
  // The last burst was clipped.
  bool clipped;
  // The burst length. A length of time in seconds.
  double burst;
  // True if `period` holds a value.
  bool has_period;
  // The burst period. A length of time in seconds.
  double period;
  // The duty cycle. Unit: %.
  double duty_pct;
  // The share of gaps that are whole periods. Unit: %.
  double regularity_pct;
  // How OFDM-like the bursts are. Unit: %.
  double ofdm_pct;
  // Level of the signal or channel, dBFS (a mean power of 1 is 0 dBFS). Unit: dBFS.
  double level_dbfs;
  // Signal to noise ratio of what is being decoded, dB (in the signal's own bandwidth, or per symbol where the stack says so). Unit: dB.
  double snr_db;
  // Bursts seen.
  int64_t bursts;
  // The DroneID symbols were seen.
  bool zc_seen;
  // DroneID packets decoded on the track.
  int64_t decoded;
  // DroneID CRC failures on the track.
  int64_t failed;
  // True if `band` holds a value.
  bool has_band;
  // The band. A value of `sdrkit_dji_droneid_signal_row_band`.
  int32_t band;
  // Why the track has its class. A value of `sdrkit_dji_droneid_signal_row_reason`.
  int32_t reason;
  // The track is within 3 MHz of a frequency reported for DroneID.
  bool near_droneid;
  // The signal time of the last burst. Unit: s.
  double last_s;
} sdrkit_dji_droneid_signal_row;

// A DJI drone heard on DroneID.
typedef struct sdrkit_dji_droneid_drone_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The sender of a decoded message: a registration, an address, a call sign or a code.
  sdrkit_string source;
  // True if `position` holds a value.
  bool has_position;
  // The position of the object that the event is about.
  sdrkit_position position;
  // The drone's altitude. Unit: m.
  double alt_m;
  // The drone's height above home. Unit: m.
  double height_m;
  // The speed over the ground of the object that the event is about, knots. Unit: kt.
  double speed_kt;
  // The course over the ground of the object that the event is about, degrees clockwise from true north. Unit: °.
  double course_deg;
  // True if `home` holds a value.
  bool has_home;
  // The home position.
  sdrkit_position home;
  // True if `pilot` holds a value.
  bool has_pilot;
  // The pilot's position.
  sdrkit_position pilot;
  // The packet sequence number.
  int64_t seq;
  // The state information bits.
  int64_t state_info;
  // The packet version.
  int64_t version;
  // The inner CRC passed.
  bool inner_crc_ok;
  // Velocity north. Unit: cm/s.
  int64_t v_north;
  // Velocity east. Unit: cm/s.
  int64_t v_east;
  // Velocity up. Unit: cm/s.
  int64_t v_up;
  // The raw yaw.
  int64_t yaw;
  // The GPS time of the packet. Seconds since 1970 (UTC).
  double gps_time;
  // The device type code.
  int64_t device_type;
  // True if `uuid` holds a value.
  bool has_uuid;
  // The UUID.
  sdrkit_string uuid;
  // True if `freq_hz` holds a value.
  bool has_freq_hz;
  // The radio frequency. A frequency in Hz.
  double freq_hz;
  // Signal to noise ratio of what is being decoded, dB (in the signal's own bandwidth, or per symbol where the stack says so). Unit: dB.
  double snr_db;
  // The track number.
  int64_t track;
  // Trail points.
  int64_t trail_points;
  // True if `last_heard` holds a value.
  bool has_last_heard;
  // Wall-clock time when last heard, in whole seconds. A host shows the age from it. Seconds since 1970 (UTC).
  double last_heard;
  // True if `heard` holds a value.
  bool has_heard;
  // Times heard.
  int64_t heard;
} sdrkit_dji_droneid_drone_row;

// The options of `droneid` of `sdrkit_dji_droneid_burst_row`.
typedef enum sdrkit_dji_droneid_burst_row_droneid {
  // not tried
  SDRKIT_DJI_DRONEID_BURST_ROW_DRONEID_NOT_TRIED = 0,
  // no DroneID
  SDRKIT_DJI_DRONEID_BURST_ROW_DRONEID_NO_DRONEID = 1,
  // CRC failed
  SDRKIT_DJI_DRONEID_BURST_ROW_DRONEID_CRC_FAILED = 2,
  // decoded
  SDRKIT_DJI_DRONEID_BURST_ROW_DRONEID_DECODED = 3,
} sdrkit_dji_droneid_burst_row_droneid;

// A burst in the burst log.
typedef struct sdrkit_dji_droneid_burst_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The signal time of a burst. Unit: s.
  double time_s;
  // True if `time` holds a value.
  bool has_time;
  // The wall time of a burst. Seconds since 1970 (UTC).
  double time;
  // The track number.
  int64_t track;
  // Carrier offset from where it was expected, Hz. A frequency in Hz.
  double offset_hz;
  // The width. A frequency in Hz.
  double width_hz;
  // The burst length. A length of time in seconds.
  double burst;
  // Level of the signal or channel, dBFS (a mean power of 1 is 0 dBFS). Unit: dBFS.
  double level_dbfs;
  // Signal to noise ratio of what is being decoded, dB (in the signal's own bandwidth, or per symbol where the stack says so). Unit: dB.
  double snr_db;
  // How OFDM-like the bursts are. Unit: %.
  double ofdm_pct;
  // The DroneID outcome of a burst. A value of `sdrkit_dji_droneid_burst_row_droneid`.
  int32_t droneid;
} sdrkit_dji_droneid_burst_row;

// The options of `key_time` of `sdrkit_flarm_fields`.
typedef enum sdrkit_flarm_fields_key_time {
  // the computer's clock (keep it right to a few seconds)
  SDRKIT_FLARM_FIELDS_KEY_TIME_CLOCK = 0,
  // the start time plus signal time
  SDRKIT_FLARM_FIELDS_KEY_TIME_START_PLUS_SIGNAL = 1,
} sdrkit_flarm_fields_key_time;

// The result fields of FLARM.
typedef struct sdrkit_flarm_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Old-protocol packets decoded.
  int64_t v6_decoded;
  // Frames that failed their check.
  int64_t frames_bad;
  // Syncs taken.
  int64_t syncs;
  // Good frames on 868.2 MHz.
  int64_t frames_868_2;
  // Good frames on 868.4 MHz.
  int64_t frames_868_4;
  // Newer-protocol packets.
  int64_t newer_packets;
  // Newer-protocol devices.
  int64_t newer_devices;
  // Packets of other types.
  int64_t other_type;
  // Privacy-flagged aircraft.
  int64_t privacy_aircraft;
  // Privacy-flagged packets.
  int64_t privacy_packets;
  // Aircraft in the table.
  int64_t aircraft;
  // Aircraft with a position.
  int64_t with_position;
  // The farthest range seen. Unit: km.
  double farthest_km;
  // Packets a second, over the last second.
  double packets_per_s;
  // Where the key time comes from. A value of `sdrkit_flarm_fields_key_time`.
  int32_t key_time;
  // The station position is missing: without it the packets give no positions.
  bool needs_station;
  // True if `snr_db` holds a value.
  bool has_snr_db;
  // Signal to noise ratio of what is being decoded, dB (in the signal's own bandwidth, or per symbol where the stack says so). Unit: dB.
  double snr_db;
  // True if `noise_dbfs` holds a value.
  bool has_noise_dbfs;
  // Noise floor, dBFS. Unit: dBFS.
  double noise_dbfs;
} sdrkit_flarm_fields;

// The options of `urgency` of `sdrkit_flarm_aircraft_row`.
typedef enum sdrkit_flarm_aircraft_row_urgency {
  // None
  SDRKIT_FLARM_AIRCRAFT_ROW_URGENCY_NONE = 0,
  // Notice
  SDRKIT_FLARM_AIRCRAFT_ROW_URGENCY_NOTICE = 1,
  // Urgent
  SDRKIT_FLARM_AIRCRAFT_ROW_URGENCY_URGENT = 2,
  // Emergency
  SDRKIT_FLARM_AIRCRAFT_ROW_URGENCY_EMERGENCY = 3,
} sdrkit_flarm_aircraft_row_urgency;

// An aircraft heard on FLARM.
typedef struct sdrkit_flarm_aircraft_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The sender of a decoded message: a registration, an address, a call sign or a code.
  sdrkit_string source;
  // Address type code (0 random, 1 ICAO, 2 FLARM, 3 anonymous).
  int64_t address_type;
  // Aircraft type code (0 to 15).
  int64_t type;
  // True if `position` holds a value.
  bool has_position;
  // The position of the object that the event is about.
  sdrkit_position position;
  // Altitude. Unit: m.
  double alt_m;
  // The speed over the ground of the object that the event is about, knots. Unit: kt.
  double speed_kt;
  // The course over the ground of the object that the event is about, degrees clockwise from true north. Unit: °.
  double course_deg;
  // Climb. Unit: m/s.
  double climb_m_s;
  // True if `distance_km` holds a value.
  bool has_distance_km;
  // Distance from the station. Unit: km.
  double distance_km;
  // True if `bearing_deg` holds a value.
  bool has_bearing_deg;
  // Bearing from the station. Unit: °.
  double bearing_deg;
  // The radio channel. A frequency in Hz.
  double channel_hz;
  // Level over the noise. Unit: dB.
  double level_db;
  // Airborne.
  bool airborne;
  // How urgent an event is. A stack sets it when the decoded message is urgent. Services alert on it. A value of `sdrkit_flarm_aircraft_row_urgency`.
  int32_t urgency;
  // True if `last_heard` holds a value.
  bool has_last_heard;
  // Wall-clock time when last heard, in whole seconds. A host shows the age from it. Seconds since 1970 (UTC).
  double last_heard;
  // True if `heard` holds a value.
  bool has_heard;
  // Times heard.
  int64_t heard;
} sdrkit_flarm_aircraft_row;

// A track point of an aircraft. The row key is the aircraft key and the signal time of the point.
typedef struct sdrkit_drones_aircraft_track_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The aircraft a track point belongs to.
  sdrkit_string aircraft;
  // The position of the object that the event is about.
  sdrkit_position position;
  // The signal time of a track point. Unit: s.
  double time_s;
} sdrkit_drones_aircraft_track_row;

// The result fields of ADS-L.
typedef struct sdrkit_adsl_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Frames (packets, blocks, bursts) that passed their check.
  int64_t frames_ok;
  // Frames that failed their check.
  int64_t frames_bad;
  // Packets with good parity on the band.
  int64_t packets_868_2;
  // Packets with good parity on the band.
  int64_t packets_868_4;
  // Packets with good parity on the band.
  int64_t packets_ldr;
  // Packets with good parity on the band.
  int64_t packets_hdr;
  // The band is inside the capture and received.
  bool on_868_2;
  // The band is inside the capture and received.
  bool on_868_4;
  // The band is inside the capture and received.
  bool on_ldr;
  // The band is inside the capture and received.
  bool on_hdr;
  // Traffic packets.
  int64_t traffic;
  // Status packets.
  int64_t status;
  // Traffic Uplink packets.
  int64_t uplink;
  // FIS-B Uplink packets.
  int64_t fis_b;
  // Remote Identification packets.
  int64_t remote_id;
  // Packets of other payload types.
  int64_t other;
  // Packets with a reserved key index: their content is not read.
  int64_t encrypted;
  // Packets with the signature flag. The signature is not checked: no register of public keys exists.
  int64_t signed_;
  // Packets that needed a repair.
  int64_t corrected;
  // Syncs taken.
  int64_t syncs;
  // True if `noise_dbfs` holds a value.
  bool has_noise_dbfs;
  // Noise floor, dBFS. Unit: dBFS.
  double noise_dbfs;
  // True if `snr_db` holds a value.
  bool has_snr_db;
  // Signal to noise ratio of what is being decoded, dB (in the signal's own bandwidth, or per symbol where the stack says so). Unit: dB.
  double snr_db;
  // Good packets a second, over the last second.
  double packets_per_s;
  // Aircraft in the table.
  int64_t aircraft;
  // Aircraft with a position.
  int64_t with_position;
  // The farthest range seen. Unit: km.
  double max_range_km;
  // FLARM packets decoded from the same capture.
  int64_t flarm_packets;
} sdrkit_adsl_fields;

// The options of `urgency` of `sdrkit_adsl_aircraft_row`.
typedef enum sdrkit_adsl_aircraft_row_urgency {
  // None
  SDRKIT_ADSL_AIRCRAFT_ROW_URGENCY_NONE = 0,
  // Notice
  SDRKIT_ADSL_AIRCRAFT_ROW_URGENCY_NOTICE = 1,
  // Urgent
  SDRKIT_ADSL_AIRCRAFT_ROW_URGENCY_URGENT = 2,
  // Emergency
  SDRKIT_ADSL_AIRCRAFT_ROW_URGENCY_EMERGENCY = 3,
} sdrkit_adsl_aircraft_row_urgency;

// An aircraft heard on ADS-L.
typedef struct sdrkit_adsl_aircraft_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The sender of a decoded message: a registration, an address, a call sign or a code.
  sdrkit_string source;
  // Where the address of an aircraft comes from.
  sdrkit_string address_source;
  // The aircraft category.
  sdrkit_string category;
  // The flight state.
  sdrkit_string flight_state;
  // The emergency status.
  sdrkit_string emergency;
  // True if `position` holds a value.
  bool has_position;
  // The position of the object that the event is about.
  sdrkit_position position;
  // True if `alt_m` holds a value.
  bool has_alt_m;
  // The drone's altitude. Unit: m.
  double alt_m;
  // True if `speed_kt` holds a value.
  bool has_speed_kt;
  // The speed over the ground of the object that the event is about, knots. Unit: kt.
  double speed_kt;
  // True if `course_deg` holds a value.
  bool has_course_deg;
  // The course over the ground of the object that the event is about, degrees clockwise from true north. Unit: °.
  double course_deg;
  // True if `climb_m_s` holds a value.
  bool has_climb_m_s;
  // Climb. Unit: m/s.
  double climb_m_s;
  // Level over the noise. Unit: dB.
  double level_db;
  // True if `distance_km` holds a value.
  bool has_distance_km;
  // Distance from the station. Unit: km.
  double distance_km;
  // True if `bearing_deg` holds a value.
  bool has_bearing_deg;
  // Bearing from the station. Unit: °.
  double bearing_deg;
  // The radio channel. A frequency in Hz.
  double channel_hz;
  // The channel of the last packet.
  sdrkit_string channel;
  // Last heard through a ground station or a relay.
  bool relayed;
  // The navigation integrity category.
  int64_t nic;
  // The horizontal position accuracy category.
  int64_t hfom;
  // The vertical position accuracy category.
  int64_t vfom;
  // The velocity accuracy category.
  int64_t velocity_accuracy;
  // The capabilities of a Status packet.
  sdrkit_string capabilities;
  // The content of the event is private (voice, encrypted traffic). Services never read its words or keep its text.
  bool private_;
  // How urgent an event is. A stack sets it when the decoded message is urgent. Services alert on it. A value of `sdrkit_adsl_aircraft_row_urgency`.
  int32_t urgency;
  // True if `last_heard` holds a value.
  bool has_last_heard;
  // Wall-clock time when last heard, in whole seconds. A host shows the age from it. Seconds since 1970 (UTC).
  double last_heard;
  // True if `heard` holds a value.
  bool has_heard;
  // Times heard.
  int64_t heard;
} sdrkit_adsl_aircraft_row;

// The result fields of the OGN tracker.
typedef struct sdrkit_ogn_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Frames (packets, blocks, bursts) that passed their check.
  int64_t frames_ok;
  // Frames that failed their check.
  int64_t frames_bad;
  // Sync words found.
  int64_t syncs;
  // Packets that needed a repair.
  int64_t corrected;
  // Packets with an encrypted position.
  int64_t encrypted;
  // Trackers in the table.
  int64_t trackers;
  // The channels of the plan inside the capture.
  int64_t channels_on;
  // The channels of the plan and which are received.
  sdrkit_string channels;
  // True if `noise_dbfs` holds a value.
  bool has_noise_dbfs;
  // Noise floor, dBFS. Unit: dBFS.
  double noise_dbfs;
  // True if `snr_db` holds a value.
  bool has_snr_db;
  // Signal to noise ratio of what is being decoded, dB (in the signal's own bandwidth, or per symbol where the stack says so). Unit: dB.
  double snr_db;
  // What no public document confirms.
  sdrkit_string note;
} sdrkit_ogn_fields;

// The options of `kind` of `sdrkit_ogn_packet_row`.
typedef enum sdrkit_ogn_packet_row_kind {
  // Position
  SDRKIT_OGN_PACKET_ROW_KIND_POSITION = 0,
  // Status
  SDRKIT_OGN_PACKET_ROW_KIND_STATUS = 1,
  // Encrypted
  SDRKIT_OGN_PACKET_ROW_KIND_ENCRYPTED = 2,
} sdrkit_ogn_packet_row_kind;

// A packet in the packet log.
typedef struct sdrkit_ogn_packet_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The signal time at the end of a packet. Unit: s.
  double time_s;
  // What a packet is. A value of `sdrkit_ogn_packet_row_kind`.
  int32_t kind;
  // The address and the name of its type.
  sdrkit_string id;
  // A packet in one line.
  sdrkit_string decoded;
  // True if `position` holds a value.
  bool has_position;
  // The position of the object that the event is about.
  sdrkit_position position;
  // The radio channel. A frequency in Hz.
  double channel_hz;
  // The repair changed a bit of the packet.
  bool repaired;
  // The 26 bytes of a packet after the repair.
  sdrkit_string hex;
} sdrkit_ogn_packet_row;

// The options of `urgency` of `sdrkit_ogn_aircraft_row`.
typedef enum sdrkit_ogn_aircraft_row_urgency {
  // None
  SDRKIT_OGN_AIRCRAFT_ROW_URGENCY_NONE = 0,
  // Notice
  SDRKIT_OGN_AIRCRAFT_ROW_URGENCY_NOTICE = 1,
  // Urgent
  SDRKIT_OGN_AIRCRAFT_ROW_URGENCY_URGENT = 2,
  // Emergency
  SDRKIT_OGN_AIRCRAFT_ROW_URGENCY_EMERGENCY = 3,
} sdrkit_ogn_aircraft_row_urgency;

// An aircraft heard on OGN.
typedef struct sdrkit_ogn_aircraft_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The sender of a decoded message: a registration, an address, a call sign or a code.
  sdrkit_string source;
  // The name of the address type.
  sdrkit_string address_type;
  // True if `aircraft_type` holds a value.
  bool has_aircraft_type;
  // The aircraft type.
  sdrkit_string aircraft_type;
  // True if `position` holds a value.
  bool has_position;
  // The position of the object that the event is about.
  sdrkit_position position;
  // True if `alt_m` holds a value.
  bool has_alt_m;
  // The drone's altitude. Unit: m.
  double alt_m;
  // True if `speed_kt` holds a value.
  bool has_speed_kt;
  // The speed over the ground of the object that the event is about, knots. Unit: kt.
  double speed_kt;
  // True if `course_deg` holds a value.
  bool has_course_deg;
  // The course over the ground of the object that the event is about, degrees clockwise from true north. Unit: °.
  double course_deg;
  // True if `climb_m_s` holds a value.
  bool has_climb_m_s;
  // Climb. Unit: m/s.
  double climb_m_s;
  // True if `turn_deg_s` holds a value.
  bool has_turn_deg_s;
  // The turn rate. Unit: °/s.
  double turn_deg_s;
  // The count of relays.
  int64_t relay;
  // True if `stealth` holds a value.
  bool has_stealth;
  // The sender does not want to be shown by other trackers.
  bool stealth;
  // True if `fix_quality` holds a value.
  bool has_fix_quality;
  // The fix quality, 0 to 3.
  int64_t fix_quality;
  // True if `second` holds a value.
  bool has_second;
  // The second of the minute of the last position.
  int64_t second;
  // Level over the noise. Unit: dB.
  double level_db;
  // The radio channel. A frequency in Hz.
  double channel_hz;
  // The content of the event is private (voice, encrypted traffic). Services never read its words or keep its text.
  bool private_;
  // How urgent an event is. A stack sets it when the decoded message is urgent. Services alert on it. A value of `sdrkit_ogn_aircraft_row_urgency`.
  int32_t urgency;
  // True if `heard` holds a value.
  bool has_heard;
  // Times heard.
  int64_t heard;
  // True if `first_heard` holds a value.
  bool has_first_heard;
  // Wall-clock time when first heard. Seconds since 1970 (UTC).
  double first_heard;
  // True if `last_heard` holds a value.
  bool has_last_heard;
  // Wall-clock time when last heard, in whole seconds. A host shows the age from it. Seconds since 1970 (UTC).
  double last_heard;
} sdrkit_ogn_aircraft_row;

// The result fields of the link stacks.
typedef struct sdrkit_drones_link_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Frames (packets, blocks, bursts) that passed their check.
  int64_t frames_ok;
  // Bursts analysed.
  int64_t bursts;
  // Bursts without a frame.
  int64_t bursts_without_frame;
  // 0 nothing, 1 bursts seen, 2 frames decoded lately.
  int64_t state;
  // Links in the table.
  int64_t links_heard;
  // The span of the capture that is searched.
  sdrkit_string covered;
  // True if `snr_db` holds a value.
  bool has_snr_db;
  // Signal to noise ratio of what is being decoded, dB (in the signal's own bandwidth, or per symbol where the stack says so). Unit: dB.
  double snr_db;
  // What no public document confirms.
  sdrkit_string note;
} sdrkit_drones_link_fields;

// The result set that a [`DecoderComponent`] adds to the results of its decoder. The description of a stack with a decoder component names it after the result sets of the decoder.
typedef struct sdrkit_drones_decoder_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // True if `dropped` holds a value.
  bool has_dropped;
  // Items dropped because the decoder was slow. It holds a value while the decoder runs on a helper thread.
  int64_t dropped;
} sdrkit_drones_decoder_fields;

// The options of `pulse_state` of `sdrkit_rc_links_ppm_fields`.
typedef enum sdrkit_rc_links_ppm_fields_pulse_state {
  // Carrier low
  SDRKIT_RC_LINKS_PPM_FIELDS_PULSE_STATE_CARRIER_LOW = 0,
  // Carrier high
  SDRKIT_RC_LINKS_PPM_FIELDS_PULSE_STATE_CARRIER_HIGH = 1,
} sdrkit_rc_links_ppm_fields_pulse_state;

// The PPM fields of `rc_links` (the submode `ppm72`). The measurements have no value in the other submodes and before a good frame.
typedef struct sdrkit_rc_links_ppm_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // True if `channels` holds a value.
  bool has_channels;
  // The channels of the last good frame.
  int64_t channels;
  // True if `frame_ms` holds a value.
  bool has_frame_ms;
  // The last good frame: the sync and the channels. Unit: ms.
  double frame_ms;
  // True if `frame_rate_hz` holds a value.
  bool has_frame_rate_hz;
  // The frames per second. Unit: Hz.
  double frame_rate_hz;
  // True if `pulse_state` holds a value.
  bool has_pulse_state;
  // The carrier state of the pulses. A value of `sdrkit_rc_links_ppm_fields_pulse_state`.
  int32_t pulse_state;
  // True if `channel_ms` holds a value.
  bool has_channel_ms;
  // The channel times of the last good frame in ms, separated by commas. Unit: ms.
  sdrkit_string channel_ms;
  // True if `channel_pct` holds a value.
  bool has_channel_pct;
  // The channels in percent of travel, separated by commas: (ms - centre) x 200. Unit: %.
  sdrkit_string channel_pct;
  // The good frames: as many channels as the frame before.
  int64_t ppm_frames_ok;
  // The bad frames: a channel out of its window, or 1 or more than 12 channels.
  int64_t ppm_frames_bad;
} sdrkit_rc_links_ppm_fields;

// A link heard.
typedef struct sdrkit_drones_heard_link_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The link name.
  sdrkit_string link;
  // The transmitter id.
  sdrkit_string id;
  // Frames of a link.
  int64_t frames;
  // True if `first_heard` holds a value.
  bool has_first_heard;
  // Wall-clock time when first heard. Seconds since 1970 (UTC).
  double first_heard;
  // True if `last_heard` holds a value.
  bool has_last_heard;
  // Wall-clock time when last heard, in whole seconds. A host shows the age from it. Seconds since 1970 (UTC).
  double last_heard;
  // The type of the last frame of a link.
  sdrkit_string last_type;
  // The fields of the last frame of a link.
  sdrkit_string last_detail;
  // True if `position` holds a value.
  bool has_position;
  // The position of the object that the event is about.
  sdrkit_position position;
  // True if `alt_m` holds a value.
  bool has_alt_m;
  // The altitude of the last position. Unit: m.
  double alt_m;
} sdrkit_drones_heard_link_row;

// A frame in the frame log.
typedef struct sdrkit_drones_link_frame_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // True if `utc` holds a value.
  bool has_utc;
  // The wall time of a frame. Seconds since 1970 (UTC).
  double utc;
  // The signal time of a frame. Unit: s.
  double time_s;
  // The link name.
  sdrkit_string link;
  // The transmitter id.
  sdrkit_string id;
  // What a frame is.
  sdrkit_string type;
  // The fields of a frame.
  sdrkit_string detail;
  // The first bytes of a frame as hex.
  sdrkit_string hex;
  // The frequency of a frame. Unit: Hz.
  double freq_hz;
  // Signal to noise ratio of what is being decoded, dB (in the signal's own bandwidth, or per symbol where the stack says so). Unit: dB.
  double snr_db;
  // The payload is encrypted.
  bool encrypted;
} sdrkit_drones_link_frame_row;

// A row of the PPM frame table.
typedef struct sdrkit_rc_links_ppm_frame_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The signal time of the sync that ended the frame. A length of time in seconds.
  double time_s;
  // The channels.
  int64_t channels;
  // The channel times in ms, separated by commas. Unit: ms.
  sdrkit_string values_ms;
  // The frame: the sync and the channels. Unit: ms.
  double frame_ms;
  // The peak over the floor of the envelope, 20 log10. Unit: dB.
  double level_db;
} sdrkit_rc_links_ppm_frame_row;

// The options of `urgency` of `sdrkit_remote_id_drone_event`.
typedef enum sdrkit_remote_id_drone_event_urgency {
  // None
  SDRKIT_REMOTE_ID_DRONE_EVENT_URGENCY_NONE = 0,
  // Notice
  SDRKIT_REMOTE_ID_DRONE_EVENT_URGENCY_NOTICE = 1,
  // Urgent
  SDRKIT_REMOTE_ID_DRONE_EVENT_URGENCY_URGENT = 2,
  // Emergency
  SDRKIT_REMOTE_ID_DRONE_EVENT_URGENCY_EMERGENCY = 3,
} sdrkit_remote_id_drone_event_urgency;

// The options of `family` of `sdrkit_remote_id_drone_event`.
typedef enum sdrkit_remote_id_drone_event_family {
  // Aircraft
  SDRKIT_REMOTE_ID_DRONE_EVENT_FAMILY_AIRCRAFT = 0,
  // Vessel
  SDRKIT_REMOTE_ID_DRONE_EVENT_FAMILY_VESSEL = 1,
  // APRS
  SDRKIT_REMOTE_ID_DRONE_EVENT_FAMILY_APRS = 2,
  // Pager
  SDRKIT_REMOTE_ID_DRONE_EVENT_FAMILY_PAGER = 3,
  // Radiosonde
  SDRKIT_REMOTE_ID_DRONE_EVENT_FAMILY_SONDE = 4,
  // Broadcast
  SDRKIT_REMOTE_ID_DRONE_EVENT_FAMILY_BROADCAST = 5,
  // Voice
  SDRKIT_REMOTE_ID_DRONE_EVENT_FAMILY_VOICE = 6,
  // Message
  SDRKIT_REMOTE_ID_DRONE_EVENT_FAMILY_MESSAGE = 7,
  // Sensor
  SDRKIT_REMOTE_ID_DRONE_EVENT_FAMILY_SENSOR = 8,
  // Satellite
  SDRKIT_REMOTE_ID_DRONE_EVENT_FAMILY_SATELLITE = 9,
  // Receiver
  SDRKIT_REMOTE_ID_DRONE_EVENT_FAMILY_RECEIVER = 10,
  // Alert
  SDRKIT_REMOTE_ID_DRONE_EVENT_FAMILY_ALERT = 11,
  // Other
  SDRKIT_REMOTE_ID_DRONE_EVENT_FAMILY_OTHER = 12,
} sdrkit_remote_id_drone_event_family;

// A drone with a position, at most every 5 s for each drone. The urgency is emergency when the drone states the emergency status.
typedef struct sdrkit_remote_id_drone_event {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Seconds since the object started, in the time of the signal.
  double time;
  // The frequency of the signal in Hz.
  double freq_hz;
  // The sender of a decoded message: a registration, an address, a call sign or a code.
  sdrkit_string source;
  // The broadcast address.
  sdrkit_string address;
  // The position of the object that the event is about.
  sdrkit_position position;
  // True if `alt_geo_m` holds a value.
  bool has_alt_geo_m;
  // Geodetic altitude. Unit: m.
  double alt_geo_m;
  // True if `speed_kt` holds a value.
  bool has_speed_kt;
  // The speed over the ground of the object that the event is about, knots. Unit: kt.
  double speed_kt;
  // True if `course_deg` holds a value.
  bool has_course_deg;
  // The course over the ground of the object that the event is about, degrees clockwise from true north. Unit: °.
  double course_deg;
  // True if `operator_position` holds a value.
  bool has_operator_position;
  // The operator's position.
  sdrkit_position operator_position;
  // How urgent an event is. A stack sets it when the decoded message is urgent. Services alert on it. A value of `sdrkit_remote_id_drone_event_urgency`.
  int32_t urgency;
  // What an event is about (aircraft, vessel, pager). Outputs and the store filter on it. A value of `sdrkit_remote_id_drone_event_family`.
  int32_t family;
} sdrkit_remote_id_drone_event;

// The options of `family` of `sdrkit_dji_droneid_event`.
typedef enum sdrkit_dji_droneid_event_family {
  // Aircraft
  SDRKIT_DJI_DRONEID_EVENT_FAMILY_AIRCRAFT = 0,
  // Vessel
  SDRKIT_DJI_DRONEID_EVENT_FAMILY_VESSEL = 1,
  // APRS
  SDRKIT_DJI_DRONEID_EVENT_FAMILY_APRS = 2,
  // Pager
  SDRKIT_DJI_DRONEID_EVENT_FAMILY_PAGER = 3,
  // Radiosonde
  SDRKIT_DJI_DRONEID_EVENT_FAMILY_SONDE = 4,
  // Broadcast
  SDRKIT_DJI_DRONEID_EVENT_FAMILY_BROADCAST = 5,
  // Voice
  SDRKIT_DJI_DRONEID_EVENT_FAMILY_VOICE = 6,
  // Message
  SDRKIT_DJI_DRONEID_EVENT_FAMILY_MESSAGE = 7,
  // Sensor
  SDRKIT_DJI_DRONEID_EVENT_FAMILY_SENSOR = 8,
  // Satellite
  SDRKIT_DJI_DRONEID_EVENT_FAMILY_SATELLITE = 9,
  // Receiver
  SDRKIT_DJI_DRONEID_EVENT_FAMILY_RECEIVER = 10,
  // Alert
  SDRKIT_DJI_DRONEID_EVENT_FAMILY_ALERT = 11,
  // Other
  SDRKIT_DJI_DRONEID_EVENT_FAMILY_OTHER = 12,
} sdrkit_dji_droneid_event_family;

// A DroneID packet.
typedef struct sdrkit_dji_droneid_event {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Seconds since the object started, in the time of the signal.
  double time;
  // The frequency of the signal in Hz.
  double freq_hz;
  // The sender of a decoded message: a registration, an address, a call sign or a code.
  sdrkit_string source;
  // True if `position` holds a value.
  bool has_position;
  // The position of the object that the event is about.
  sdrkit_position position;
  // The drone's altitude. Unit: m.
  double alt_m;
  // The drone's height above home. Unit: m.
  double height_m;
  // The speed over the ground of the object that the event is about, knots. Unit: kt.
  double speed_kt;
  // The course over the ground of the object that the event is about, degrees clockwise from true north. Unit: °.
  double course_deg;
  // True if `home` holds a value.
  bool has_home;
  // The home position.
  sdrkit_position home;
  // True if `pilot` holds a value.
  bool has_pilot;
  // The pilot's position.
  sdrkit_position pilot;
  // What an event is about (aircraft, vessel, pager). Outputs and the store filter on it. A value of `sdrkit_dji_droneid_event_family`.
  int32_t family;
} sdrkit_dji_droneid_event;

// The options of `urgency` of `sdrkit_flarm_aircraft_event`.
typedef enum sdrkit_flarm_aircraft_event_urgency {
  // None
  SDRKIT_FLARM_AIRCRAFT_EVENT_URGENCY_NONE = 0,
  // Notice
  SDRKIT_FLARM_AIRCRAFT_EVENT_URGENCY_NOTICE = 1,
  // Urgent
  SDRKIT_FLARM_AIRCRAFT_EVENT_URGENCY_URGENT = 2,
  // Emergency
  SDRKIT_FLARM_AIRCRAFT_EVENT_URGENCY_EMERGENCY = 3,
} sdrkit_flarm_aircraft_event_urgency;

// The options of `family` of `sdrkit_flarm_aircraft_event`.
typedef enum sdrkit_flarm_aircraft_event_family {
  // Aircraft
  SDRKIT_FLARM_AIRCRAFT_EVENT_FAMILY_AIRCRAFT = 0,
  // Vessel
  SDRKIT_FLARM_AIRCRAFT_EVENT_FAMILY_VESSEL = 1,
  // APRS
  SDRKIT_FLARM_AIRCRAFT_EVENT_FAMILY_APRS = 2,
  // Pager
  SDRKIT_FLARM_AIRCRAFT_EVENT_FAMILY_PAGER = 3,
  // Radiosonde
  SDRKIT_FLARM_AIRCRAFT_EVENT_FAMILY_SONDE = 4,
  // Broadcast
  SDRKIT_FLARM_AIRCRAFT_EVENT_FAMILY_BROADCAST = 5,
  // Voice
  SDRKIT_FLARM_AIRCRAFT_EVENT_FAMILY_VOICE = 6,
  // Message
  SDRKIT_FLARM_AIRCRAFT_EVENT_FAMILY_MESSAGE = 7,
  // Sensor
  SDRKIT_FLARM_AIRCRAFT_EVENT_FAMILY_SENSOR = 8,
  // Satellite
  SDRKIT_FLARM_AIRCRAFT_EVENT_FAMILY_SATELLITE = 9,
  // Receiver
  SDRKIT_FLARM_AIRCRAFT_EVENT_FAMILY_RECEIVER = 10,
  // Alert
  SDRKIT_FLARM_AIRCRAFT_EVENT_FAMILY_ALERT = 11,
  // Other
  SDRKIT_FLARM_AIRCRAFT_EVENT_FAMILY_OTHER = 12,
} sdrkit_flarm_aircraft_event_family;

// An aircraft heard on FLARM.
typedef struct sdrkit_flarm_aircraft_event {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Seconds since the object started, in the time of the signal.
  double time;
  // The frequency of the signal in Hz.
  double freq_hz;
  // The sender of a decoded message: a registration, an address, a call sign or a code.
  sdrkit_string source;
  // Aircraft type code (0 to 15).
  int64_t type;
  // The position of the object that the event is about.
  sdrkit_position position;
  // Altitude. Unit: m.
  double alt_m;
  // The speed over the ground of the object that the event is about, knots. Unit: kt.
  double speed_kt;
  // The course over the ground of the object that the event is about, degrees clockwise from true north. Unit: °.
  double course_deg;
  // Climb. Unit: m/s.
  double climb_m_s;
  // How urgent an event is. A stack sets it when the decoded message is urgent. Services alert on it. A value of `sdrkit_flarm_aircraft_event_urgency`.
  int32_t urgency;
  // What an event is about (aircraft, vessel, pager). Outputs and the store filter on it. A value of `sdrkit_flarm_aircraft_event_family`.
  int32_t family;
} sdrkit_flarm_aircraft_event;

// The options of `family` of `sdrkit_adsl_aircraft_event`.
typedef enum sdrkit_adsl_aircraft_event_family {
  // Aircraft
  SDRKIT_ADSL_AIRCRAFT_EVENT_FAMILY_AIRCRAFT = 0,
  // Vessel
  SDRKIT_ADSL_AIRCRAFT_EVENT_FAMILY_VESSEL = 1,
  // APRS
  SDRKIT_ADSL_AIRCRAFT_EVENT_FAMILY_APRS = 2,
  // Pager
  SDRKIT_ADSL_AIRCRAFT_EVENT_FAMILY_PAGER = 3,
  // Radiosonde
  SDRKIT_ADSL_AIRCRAFT_EVENT_FAMILY_SONDE = 4,
  // Broadcast
  SDRKIT_ADSL_AIRCRAFT_EVENT_FAMILY_BROADCAST = 5,
  // Voice
  SDRKIT_ADSL_AIRCRAFT_EVENT_FAMILY_VOICE = 6,
  // Message
  SDRKIT_ADSL_AIRCRAFT_EVENT_FAMILY_MESSAGE = 7,
  // Sensor
  SDRKIT_ADSL_AIRCRAFT_EVENT_FAMILY_SENSOR = 8,
  // Satellite
  SDRKIT_ADSL_AIRCRAFT_EVENT_FAMILY_SATELLITE = 9,
  // Receiver
  SDRKIT_ADSL_AIRCRAFT_EVENT_FAMILY_RECEIVER = 10,
  // Alert
  SDRKIT_ADSL_AIRCRAFT_EVENT_FAMILY_ALERT = 11,
  // Other
  SDRKIT_ADSL_AIRCRAFT_EVENT_FAMILY_OTHER = 12,
} sdrkit_adsl_aircraft_event_family;

// An aircraft heard on ADS-L.
typedef struct sdrkit_adsl_aircraft_event {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Seconds since the object started, in the time of the signal.
  double time;
  // The frequency of the signal in Hz.
  double freq_hz;
  // The sender of a decoded message: a registration, an address, a call sign or a code.
  sdrkit_string source;
  // Where the address of an aircraft comes from.
  sdrkit_string address_source;
  // The aircraft category.
  sdrkit_string category;
  // The position of the object that the event is about.
  sdrkit_position position;
  // The content of the event is private (voice, encrypted traffic). Services never read its words or keep its text.
  bool private_;
  // What an event is about (aircraft, vessel, pager). Outputs and the store filter on it. A value of `sdrkit_adsl_aircraft_event_family`.
  int32_t family;
} sdrkit_adsl_aircraft_event;

// The options of `urgency` of `sdrkit_adsl_emergency_event`.
typedef enum sdrkit_adsl_emergency_event_urgency {
  // None
  SDRKIT_ADSL_EMERGENCY_EVENT_URGENCY_NONE = 0,
  // Notice
  SDRKIT_ADSL_EMERGENCY_EVENT_URGENCY_NOTICE = 1,
  // Urgent
  SDRKIT_ADSL_EMERGENCY_EVENT_URGENCY_URGENT = 2,
  // Emergency
  SDRKIT_ADSL_EMERGENCY_EVENT_URGENCY_EMERGENCY = 3,
} sdrkit_adsl_emergency_event_urgency;

// The options of `family` of `sdrkit_adsl_emergency_event`.
typedef enum sdrkit_adsl_emergency_event_family {
  // Aircraft
  SDRKIT_ADSL_EMERGENCY_EVENT_FAMILY_AIRCRAFT = 0,
  // Vessel
  SDRKIT_ADSL_EMERGENCY_EVENT_FAMILY_VESSEL = 1,
  // APRS
  SDRKIT_ADSL_EMERGENCY_EVENT_FAMILY_APRS = 2,
  // Pager
  SDRKIT_ADSL_EMERGENCY_EVENT_FAMILY_PAGER = 3,
  // Radiosonde
  SDRKIT_ADSL_EMERGENCY_EVENT_FAMILY_SONDE = 4,
  // Broadcast
  SDRKIT_ADSL_EMERGENCY_EVENT_FAMILY_BROADCAST = 5,
  // Voice
  SDRKIT_ADSL_EMERGENCY_EVENT_FAMILY_VOICE = 6,
  // Message
  SDRKIT_ADSL_EMERGENCY_EVENT_FAMILY_MESSAGE = 7,
  // Sensor
  SDRKIT_ADSL_EMERGENCY_EVENT_FAMILY_SENSOR = 8,
  // Satellite
  SDRKIT_ADSL_EMERGENCY_EVENT_FAMILY_SATELLITE = 9,
  // Receiver
  SDRKIT_ADSL_EMERGENCY_EVENT_FAMILY_RECEIVER = 10,
  // Alert
  SDRKIT_ADSL_EMERGENCY_EVENT_FAMILY_ALERT = 11,
  // Other
  SDRKIT_ADSL_EMERGENCY_EVENT_FAMILY_OTHER = 12,
} sdrkit_adsl_emergency_event_family;

// An ADS-L aircraft in an emergency.
typedef struct sdrkit_adsl_emergency_event {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Seconds since the object started, in the time of the signal.
  double time;
  // The frequency of the signal in Hz.
  double freq_hz;
  // The sender of a decoded message: a registration, an address, a call sign or a code.
  sdrkit_string source;
  // The text of a decoded message.
  sdrkit_string text;
  // True if `position` holds a value.
  bool has_position;
  // The position of the object that the event is about.
  sdrkit_position position;
  // The content of the event is private (voice, encrypted traffic). Services never read its words or keep its text.
  bool private_;
  // How urgent an event is. A stack sets it when the decoded message is urgent. Services alert on it. A value of `sdrkit_adsl_emergency_event_urgency`.
  int32_t urgency;
  // What an event is about (aircraft, vessel, pager). Outputs and the store filter on it. A value of `sdrkit_adsl_emergency_event_family`.
  int32_t family;
} sdrkit_adsl_emergency_event;

// The options of `family` of `sdrkit_ogn_tracker_event`.
typedef enum sdrkit_ogn_tracker_event_family {
  // Aircraft
  SDRKIT_OGN_TRACKER_EVENT_FAMILY_AIRCRAFT = 0,
  // Vessel
  SDRKIT_OGN_TRACKER_EVENT_FAMILY_VESSEL = 1,
  // APRS
  SDRKIT_OGN_TRACKER_EVENT_FAMILY_APRS = 2,
  // Pager
  SDRKIT_OGN_TRACKER_EVENT_FAMILY_PAGER = 3,
  // Radiosonde
  SDRKIT_OGN_TRACKER_EVENT_FAMILY_SONDE = 4,
  // Broadcast
  SDRKIT_OGN_TRACKER_EVENT_FAMILY_BROADCAST = 5,
  // Voice
  SDRKIT_OGN_TRACKER_EVENT_FAMILY_VOICE = 6,
  // Message
  SDRKIT_OGN_TRACKER_EVENT_FAMILY_MESSAGE = 7,
  // Sensor
  SDRKIT_OGN_TRACKER_EVENT_FAMILY_SENSOR = 8,
  // Satellite
  SDRKIT_OGN_TRACKER_EVENT_FAMILY_SATELLITE = 9,
  // Receiver
  SDRKIT_OGN_TRACKER_EVENT_FAMILY_RECEIVER = 10,
  // Alert
  SDRKIT_OGN_TRACKER_EVENT_FAMILY_ALERT = 11,
  // Other
  SDRKIT_OGN_TRACKER_EVENT_FAMILY_OTHER = 12,
} sdrkit_ogn_tracker_event_family;

// An OGN tracker heard.
typedef struct sdrkit_ogn_tracker_event {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Seconds since the object started, in the time of the signal.
  double time;
  // The frequency of the signal in Hz.
  double freq_hz;
  // The sender of a decoded message: a registration, an address, a call sign or a code.
  sdrkit_string source;
  // The name of the address type.
  sdrkit_string address_type;
  // The aircraft type.
  sdrkit_string aircraft_type;
  // The position of the object that the event is about.
  sdrkit_position position;
  // The content of the event is private (voice, encrypted traffic). Services never read its words or keep its text.
  bool private_;
  // What an event is about (aircraft, vessel, pager). Outputs and the store filter on it. A value of `sdrkit_ogn_tracker_event_family`.
  int32_t family;
} sdrkit_ogn_tracker_event;

// The options of `urgency` of `sdrkit_ogn_emergency_event`.
typedef enum sdrkit_ogn_emergency_event_urgency {
  // None
  SDRKIT_OGN_EMERGENCY_EVENT_URGENCY_NONE = 0,
  // Notice
  SDRKIT_OGN_EMERGENCY_EVENT_URGENCY_NOTICE = 1,
  // Urgent
  SDRKIT_OGN_EMERGENCY_EVENT_URGENCY_URGENT = 2,
  // Emergency
  SDRKIT_OGN_EMERGENCY_EVENT_URGENCY_EMERGENCY = 3,
} sdrkit_ogn_emergency_event_urgency;

// The options of `family` of `sdrkit_ogn_emergency_event`.
typedef enum sdrkit_ogn_emergency_event_family {
  // Aircraft
  SDRKIT_OGN_EMERGENCY_EVENT_FAMILY_AIRCRAFT = 0,
  // Vessel
  SDRKIT_OGN_EMERGENCY_EVENT_FAMILY_VESSEL = 1,
  // APRS
  SDRKIT_OGN_EMERGENCY_EVENT_FAMILY_APRS = 2,
  // Pager
  SDRKIT_OGN_EMERGENCY_EVENT_FAMILY_PAGER = 3,
  // Radiosonde
  SDRKIT_OGN_EMERGENCY_EVENT_FAMILY_SONDE = 4,
  // Broadcast
  SDRKIT_OGN_EMERGENCY_EVENT_FAMILY_BROADCAST = 5,
  // Voice
  SDRKIT_OGN_EMERGENCY_EVENT_FAMILY_VOICE = 6,
  // Message
  SDRKIT_OGN_EMERGENCY_EVENT_FAMILY_MESSAGE = 7,
  // Sensor
  SDRKIT_OGN_EMERGENCY_EVENT_FAMILY_SENSOR = 8,
  // Satellite
  SDRKIT_OGN_EMERGENCY_EVENT_FAMILY_SATELLITE = 9,
  // Receiver
  SDRKIT_OGN_EMERGENCY_EVENT_FAMILY_RECEIVER = 10,
  // Alert
  SDRKIT_OGN_EMERGENCY_EVENT_FAMILY_ALERT = 11,
  // Other
  SDRKIT_OGN_EMERGENCY_EVENT_FAMILY_OTHER = 12,
} sdrkit_ogn_emergency_event_family;

// An OGN tracker in an emergency.
typedef struct sdrkit_ogn_emergency_event {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Seconds since the object started, in the time of the signal.
  double time;
  // The frequency of the signal in Hz.
  double freq_hz;
  // The sender of a decoded message: a registration, an address, a call sign or a code.
  sdrkit_string source;
  // The text of a decoded message.
  sdrkit_string text;
  // True if `position` holds a value.
  bool has_position;
  // The position of the object that the event is about.
  sdrkit_position position;
  // The content of the event is private (voice, encrypted traffic). Services never read its words or keep its text.
  bool private_;
  // How urgent an event is. A stack sets it when the decoded message is urgent. Services alert on it. A value of `sdrkit_ogn_emergency_event_urgency`.
  int32_t urgency;
  // What an event is about (aircraft, vessel, pager). Outputs and the store filter on it. A value of `sdrkit_ogn_emergency_event_family`.
  int32_t family;
} sdrkit_ogn_emergency_event;

// The options of `family` of `sdrkit_rc_links_link_event`.
typedef enum sdrkit_rc_links_link_event_family {
  // Aircraft
  SDRKIT_RC_LINKS_LINK_EVENT_FAMILY_AIRCRAFT = 0,
  // Vessel
  SDRKIT_RC_LINKS_LINK_EVENT_FAMILY_VESSEL = 1,
  // APRS
  SDRKIT_RC_LINKS_LINK_EVENT_FAMILY_APRS = 2,
  // Pager
  SDRKIT_RC_LINKS_LINK_EVENT_FAMILY_PAGER = 3,
  // Radiosonde
  SDRKIT_RC_LINKS_LINK_EVENT_FAMILY_SONDE = 4,
  // Broadcast
  SDRKIT_RC_LINKS_LINK_EVENT_FAMILY_BROADCAST = 5,
  // Voice
  SDRKIT_RC_LINKS_LINK_EVENT_FAMILY_VOICE = 6,
  // Message
  SDRKIT_RC_LINKS_LINK_EVENT_FAMILY_MESSAGE = 7,
  // Sensor
  SDRKIT_RC_LINKS_LINK_EVENT_FAMILY_SENSOR = 8,
  // Satellite
  SDRKIT_RC_LINKS_LINK_EVENT_FAMILY_SATELLITE = 9,
  // Receiver
  SDRKIT_RC_LINKS_LINK_EVENT_FAMILY_RECEIVER = 10,
  // Alert
  SDRKIT_RC_LINKS_LINK_EVENT_FAMILY_ALERT = 11,
  // Other
  SDRKIT_RC_LINKS_LINK_EVENT_FAMILY_OTHER = 12,
} sdrkit_rc_links_link_event_family;

// A new radio-control link.
typedef struct sdrkit_rc_links_link_event {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Seconds since the object started, in the time of the signal.
  double time;
  // The frequency of the signal in Hz.
  double freq_hz;
  // The sender of a decoded message: a registration, an address, a call sign or a code.
  sdrkit_string source;
  // The link name.
  sdrkit_string link;
  // What an event is about (aircraft, vessel, pager). Outputs and the store filter on it. A value of `sdrkit_rc_links_link_event_family`.
  int32_t family;
} sdrkit_rc_links_link_event;

// A good PPM frame.
typedef struct sdrkit_rc_links_rc_ppm_frame_event {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Seconds since the object started, in the time of the signal.
  double time;
  // The frequency of the signal in Hz.
  double freq_hz;
  // The signal time of the sync that ended the frame. A length of time in seconds.
  double time_s;
  // The channels.
  int64_t channels;
  // The channel times in ms, separated by commas. Unit: ms.
  sdrkit_string values_ms;
  // The frame: the sync and the channels. Unit: ms.
  double frame_ms;
  // The peak over the floor of the envelope, 20 log10. Unit: dB.
  double level_db;
} sdrkit_rc_links_rc_ppm_frame_event;

// The options of `family` of `sdrkit_mavlink_sik_vehicle_event`.
typedef enum sdrkit_mavlink_sik_vehicle_event_family {
  // Aircraft
  SDRKIT_MAVLINK_SIK_VEHICLE_EVENT_FAMILY_AIRCRAFT = 0,
  // Vessel
  SDRKIT_MAVLINK_SIK_VEHICLE_EVENT_FAMILY_VESSEL = 1,
  // APRS
  SDRKIT_MAVLINK_SIK_VEHICLE_EVENT_FAMILY_APRS = 2,
  // Pager
  SDRKIT_MAVLINK_SIK_VEHICLE_EVENT_FAMILY_PAGER = 3,
  // Radiosonde
  SDRKIT_MAVLINK_SIK_VEHICLE_EVENT_FAMILY_SONDE = 4,
  // Broadcast
  SDRKIT_MAVLINK_SIK_VEHICLE_EVENT_FAMILY_BROADCAST = 5,
  // Voice
  SDRKIT_MAVLINK_SIK_VEHICLE_EVENT_FAMILY_VOICE = 6,
  // Message
  SDRKIT_MAVLINK_SIK_VEHICLE_EVENT_FAMILY_MESSAGE = 7,
  // Sensor
  SDRKIT_MAVLINK_SIK_VEHICLE_EVENT_FAMILY_SENSOR = 8,
  // Satellite
  SDRKIT_MAVLINK_SIK_VEHICLE_EVENT_FAMILY_SATELLITE = 9,
  // Receiver
  SDRKIT_MAVLINK_SIK_VEHICLE_EVENT_FAMILY_RECEIVER = 10,
  // Alert
  SDRKIT_MAVLINK_SIK_VEHICLE_EVENT_FAMILY_ALERT = 11,
  // Other
  SDRKIT_MAVLINK_SIK_VEHICLE_EVENT_FAMILY_OTHER = 12,
} sdrkit_mavlink_sik_vehicle_event_family;

// A new MAVLink vehicle.
typedef struct sdrkit_mavlink_sik_vehicle_event {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Seconds since the object started, in the time of the signal.
  double time;
  // The frequency of the signal in Hz.
  double freq_hz;
  // The sender of a decoded message: a registration, an address, a call sign or a code.
  sdrkit_string source;
  // The link name.
  sdrkit_string link;
  // True if `position` holds a value.
  bool has_position;
  // The position of the object that the event is about.
  sdrkit_position position;
  // What an event is about (aircraft, vessel, pager). Outputs and the store filter on it. A value of `sdrkit_mavlink_sik_vehicle_event_family`.
  int32_t family;
} sdrkit_mavlink_sik_vehicle_event;

// The arguments of `remove` of a drone.
typedef struct sdrkit_drones_drone_args {
  // The size of this struct: sizeof of the type. The caller sets it.
  uint32_t size;
  // The row key of the drone.
  sdrkit_string row;
} sdrkit_drones_drone_args;

// An arguments struct with its size set and every argument at 0.
#define SDRKIT_DRONES_DRONE_ARGS_INIT {sizeof(sdrkit_drones_drone_args), {0}}

// The arguments of `remove` of an aircraft.
typedef struct sdrkit_drones_aircraft_args {
  // The size of this struct: sizeof of the type. The caller sets it.
  uint32_t size;
  // The row key of the aircraft.
  sdrkit_string row;
} sdrkit_drones_aircraft_args;

// An arguments struct with its size set and every argument at 0.
#define SDRKIT_DRONES_AIRCRAFT_ARGS_INIT {sizeof(sdrkit_drones_aircraft_args), {0}}

// The arguments of `remove` of a link.
typedef struct sdrkit_drones_link_args {
  // The size of this struct: sizeof of the type. The caller sets it.
  uint32_t size;
  // The row key of the link.
  sdrkit_string row;
} sdrkit_drones_link_args;

// An arguments struct with its size set and every argument at 0.
#define SDRKIT_DRONES_LINK_ARGS_INIT {sizeof(sdrkit_drones_link_args), {0}}

// Fills `rows` with the changed rows of the table "drones" of the stack "remote_id" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_remote_id_drones_rows(SDRKIT_HANDLE handle,
                                                  sdrkit_remote_id_drone_row *rows,
                                                  uint32_t capacity,
                                                  sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "drones.RemoteIdDroneRow", "drones", rows, sizeof(sdrkit_remote_id_drone_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "messages" of the stack "remote_id" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_remote_id_messages_rows(SDRKIT_HANDLE handle,
                                                    sdrkit_remote_id_message_row *rows,
                                                    uint32_t capacity,
                                                    sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "drones.RemoteIdMessageRow", "messages", rows, sizeof(sdrkit_remote_id_message_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "drone_tracks" of the stack "remote_id" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_remote_id_drone_tracks_rows(SDRKIT_HANDLE handle,
                                                        sdrkit_drones_drone_track_row *rows,
                                                        uint32_t capacity,
                                                        sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "drones.DroneTrackRow", "drone_tracks", rows, sizeof(sdrkit_drones_drone_track_row), capacity, info);
}

// Fills `event` with event `n` of the last sdrkit_events_take() on `handle`, an event of the type "remote_id.drone"
// (Drone). Returns SDRKIT_E_WRONG_TYPE if the event is of another type.
static inline int32_t sdrkit_remote_id_drone_event_read(SDRKIT_HANDLE handle, uint32_t n, sdrkit_remote_id_drone_event *event) {
  return sdrkit_typed_event(handle, n, "remote_id.drone", "drones.RemoteIdDroneEvent", event, sizeof(sdrkit_remote_id_drone_event));
}

// Runs the command "clear" (Clear) of the stack "remote_id" on the session of `handle`. Forgets everything heard so far. Favourites stay.
static inline int32_t sdrkit_remote_id_clear(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "clear", NULL, NULL, NULL, NULL, 0);
}

// Runs the command "remove" (Remove) of the stack "remote_id" on the session of `handle`. Forgets one object, a favourite too.
static inline int32_t sdrkit_remote_id_remove(SDRKIT_HANDLE handle, const sdrkit_drones_drone_args *args) {
  return sdrkit_typed_command(handle, "remove", "drones.DroneArgs", args, NULL, NULL, 0);
}

// The settings of the test signal generator of the stack "remote_id" are the sdrkit_remote_id_gen_*() functions
// after the settings of its receiver.

// The setting "band" (Band) of the stack "remote_id". A value of `sdrkit_remote_id_band`. One 20 MHz band of 2.4 GHz is heard at a time. Drones advertise on all three BLE channels, and most send Wi-Fi beacons on channel 6, so 2432 MHz hears both.
static inline int32_t sdrkit_remote_id_get_band(SDRKIT_HANDLE handle, sdrkit_remote_id_band *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "band", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_remote_id_band)index;
  return code;
}
static inline int32_t sdrkit_remote_id_set_band(SDRKIT_HANDLE handle, sdrkit_remote_id_band value) {
  return sdrkit_typed_setting_set_enum(handle, "band", (int32_t)value);
}

// The setting "ble" (Bluetooth LE) of the stack "remote_id". Run the Bluetooth LE decoder when an advertising channel is in the band.
static inline int32_t sdrkit_remote_id_get_ble(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "ble", value);
}
static inline int32_t sdrkit_remote_id_set_ble(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "ble", value);
}

// The setting "wifi_ofdm" (Wi-Fi OFDM beacons) of the stack "remote_id". Run the OFDM beacon path beside the DSSS one.
static inline int32_t sdrkit_remote_id_get_wifi_ofdm(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "wifi_ofdm", value);
}
static inline int32_t sdrkit_remote_id_set_wifi_ofdm(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "wifi_ofdm", value);
}

// The setting "gen_carriers" (Test signal carriers) of the test signal generator of the stack "remote_id". A value of `sdrkit_remote_id_gen_carriers`.
static inline int32_t sdrkit_remote_id_gen_get_carriers(SDRKIT_HANDLE handle, sdrkit_remote_id_gen_carriers *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "gen_carriers", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_remote_id_gen_carriers)index;
  return code;
}
static inline int32_t sdrkit_remote_id_gen_set_carriers(SDRKIT_HANDLE handle, sdrkit_remote_id_gen_carriers value) {
  return sdrkit_typed_setting_set_enum(handle, "gen_carriers", (int32_t)value);
}

// The setting "gen_centre_mhz" (Test signal radio centre) of the test signal generator of the stack "remote_id". Unit: MHz. 0 places both carriers at 0 Hz (as a file without a frequency).
static inline int32_t sdrkit_remote_id_gen_get_centre_mhz(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_centre_mhz", value);
}
static inline int32_t sdrkit_remote_id_gen_set_centre_mhz(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_centre_mhz", value);
}

// The setting "gen_wifi_phy" (Test signal Wi-Fi PHY) of the test signal generator of the stack "remote_id". A value of `sdrkit_remote_id_gen_wifi_phy`.
static inline int32_t sdrkit_remote_id_gen_get_wifi_phy(SDRKIT_HANDLE handle, sdrkit_remote_id_gen_wifi_phy *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "gen_wifi_phy", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_remote_id_gen_wifi_phy)index;
  return code;
}
static inline int32_t sdrkit_remote_id_gen_set_wifi_phy(SDRKIT_HANDLE handle, sdrkit_remote_id_gen_wifi_phy value) {
  return sdrkit_typed_setting_set_enum(handle, "gen_wifi_phy", (int32_t)value);
}

// Fills `rows` with the changed rows of the table "signals" of the stack "dji_droneid" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_dji_droneid_signals_rows(SDRKIT_HANDLE handle,
                                                     sdrkit_dji_droneid_signal_row *rows,
                                                     uint32_t capacity,
                                                     sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "drones.DjiDroneidSignalRow", "signals", rows, sizeof(sdrkit_dji_droneid_signal_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "drones" of the stack "dji_droneid" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_dji_droneid_drones_rows(SDRKIT_HANDLE handle,
                                                    sdrkit_dji_droneid_drone_row *rows,
                                                    uint32_t capacity,
                                                    sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "drones.DjiDroneidDroneRow", "drones", rows, sizeof(sdrkit_dji_droneid_drone_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "bursts" of the stack "dji_droneid" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_dji_droneid_bursts_rows(SDRKIT_HANDLE handle,
                                                    sdrkit_dji_droneid_burst_row *rows,
                                                    uint32_t capacity,
                                                    sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "drones.DjiDroneidBurstRow", "bursts", rows, sizeof(sdrkit_dji_droneid_burst_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "drone_tracks" of the stack "dji_droneid" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_dji_droneid_drone_tracks_rows(SDRKIT_HANDLE handle,
                                                          sdrkit_drones_drone_track_row *rows,
                                                          uint32_t capacity,
                                                          sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "drones.DroneTrackRow", "drone_tracks", rows, sizeof(sdrkit_drones_drone_track_row), capacity, info);
}

// Fills `event` with event `n` of the last sdrkit_events_take() on `handle`, an event of the type "dji.droneid"
// (DroneID packet). Returns SDRKIT_E_WRONG_TYPE if the event is of another type.
static inline int32_t sdrkit_dji_droneid_event_read(SDRKIT_HANDLE handle, uint32_t n, sdrkit_dji_droneid_event *event) {
  return sdrkit_typed_event(handle, n, "dji.droneid", "drones.DjiDroneidEvent", event, sizeof(sdrkit_dji_droneid_event));
}

// Runs the command "clear" (Clear) of the stack "dji_droneid" on the session of `handle`. Forgets everything heard so far. Favourites stay.
static inline int32_t sdrkit_dji_droneid_clear(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "clear", NULL, NULL, NULL, NULL, 0);
}

// Runs the command "remove" (Remove) of the stack "dji_droneid" on the session of `handle`. Forgets one object, a favourite too.
static inline int32_t sdrkit_dji_droneid_remove(SDRKIT_HANDLE handle, const sdrkit_drones_drone_args *args) {
  return sdrkit_typed_command(handle, "remove", "drones.DroneArgs", args, NULL, NULL, 0);
}

// The settings of the test signal generator of the stack "dji_droneid" are the sdrkit_dji_droneid_gen_*() functions
// after the settings of its receiver.

// The setting "decode_droneid" (Decode DroneID) of the stack "dji_droneid". Try every 0.5 to 1.5 ms burst that is 7 to 11 MHz wide as a DroneID burst: Zadoff-Chu synchronisation, turbo decoding, CRC-24.
static inline int32_t sdrkit_dji_droneid_get_decode_droneid(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "decode_droneid", value);
}
static inline int32_t sdrkit_dji_droneid_set_decode_droneid(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "decode_droneid", value);
}

// The setting "wall_clock" (Stamp with this computer's clock) of the stack "dji_droneid". Stamp rows and bursts of live samples with the computer's clock as well as signal time.
static inline int32_t sdrkit_dji_droneid_get_wall_clock(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "wall_clock", value);
}
static inline int32_t sdrkit_dji_droneid_set_wall_clock(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "wall_clock", value);
}

// The setting "gen_scenario" (Test signal) of the test signal generator of the stack "dji_droneid". A value of `sdrkit_dji_droneid_gen_scenario`.
static inline int32_t sdrkit_dji_droneid_gen_get_scenario(SDRKIT_HANDLE handle, sdrkit_dji_droneid_gen_scenario *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "gen_scenario", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_dji_droneid_gen_scenario)index;
  return code;
}
static inline int32_t sdrkit_dji_droneid_gen_set_scenario(SDRKIT_HANDLE handle, sdrkit_dji_droneid_gen_scenario value) {
  return sdrkit_typed_setting_set_enum(handle, "gen_scenario", (int32_t)value);
}

// The setting "gen_period_s" (DroneID period) of the test signal generator of the stack "dji_droneid". Unit: s.
static inline int32_t sdrkit_dji_droneid_gen_get_period_s(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_period_s", value);
}
static inline int32_t sdrkit_dji_droneid_gen_set_period_s(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_period_s", value);
}

// The setting "gen_eight_symbols" (8-symbol DroneID) of the test signal generator of the stack "dji_droneid".
static inline int32_t sdrkit_dji_droneid_gen_get_eight_symbols(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "gen_eight_symbols", value);
}
static inline int32_t sdrkit_dji_droneid_gen_set_eight_symbols(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "gen_eight_symbols", value);
}

// The setting "gen_wide" (20 MHz OcuSync-like) of the test signal generator of the stack "dji_droneid".
static inline int32_t sdrkit_dji_droneid_gen_get_wide(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "gen_wide", value);
}
static inline int32_t sdrkit_dji_droneid_gen_set_wide(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "gen_wide", value);
}

// Fills `rows` with the changed rows of the table "aircraft" of the stack "flarm" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_flarm_aircraft_rows(SDRKIT_HANDLE handle,
                                                sdrkit_flarm_aircraft_row *rows,
                                                uint32_t capacity,
                                                sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "drones.FlarmAircraftRow", "aircraft", rows, sizeof(sdrkit_flarm_aircraft_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "aircraft_tracks" of the stack "flarm" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_flarm_aircraft_tracks_rows(SDRKIT_HANDLE handle,
                                                       sdrkit_drones_aircraft_track_row *rows,
                                                       uint32_t capacity,
                                                       sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "drones.AircraftTrackRow", "aircraft_tracks", rows, sizeof(sdrkit_drones_aircraft_track_row), capacity, info);
}

// Fills `event` with event `n` of the last sdrkit_events_take() on `handle`, an event of the type "flarm.aircraft"
// (Aircraft). Returns SDRKIT_E_WRONG_TYPE if the event is of another type.
static inline int32_t sdrkit_flarm_aircraft_event_read(SDRKIT_HANDLE handle, uint32_t n, sdrkit_flarm_aircraft_event *event) {
  return sdrkit_typed_event(handle, n, "flarm.aircraft", "drones.FlarmAircraftEvent", event, sizeof(sdrkit_flarm_aircraft_event));
}

// Runs the command "clear" (Clear) of the stack "flarm" on the session of `handle`. Forgets everything heard so far. Favourites stay.
static inline int32_t sdrkit_flarm_clear(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "clear", NULL, NULL, NULL, NULL, 0);
}

// Runs the command "remove" (Remove) of the stack "flarm" on the session of `handle`. Forgets one object, a favourite too.
static inline int32_t sdrkit_flarm_remove(SDRKIT_HANDLE handle, const sdrkit_drones_aircraft_args *args) {
  return sdrkit_typed_command(handle, "remove", "drones.AircraftArgs", args, NULL, NULL, 0);
}

// The settings of the test signal generator of the stack "flarm" are the sdrkit_flarm_gen_*() functions
// after the settings of its receiver.

// The setting "wall_clock" (Key time from this computer's clock) of the stack "flarm". On for a radio: the packets' key time is the computer's clock (keep it right to a few seconds). Off, or for a recording or the test signal: the start time plus signal time.
static inline int32_t sdrkit_flarm_get_wall_clock(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "wall_clock", value);
}
static inline int32_t sdrkit_flarm_set_wall_clock(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "wall_clock", value);
}

// The setting "start_time" (Start time (unix seconds)) of the stack "flarm". Unit: s. The key time of the first sample when the computer's clock is not used. 0: the start of playback.
static inline int32_t sdrkit_flarm_get_start_time(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "start_time", value);
}
static inline int32_t sdrkit_flarm_set_start_time(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "start_time", value);
}

// The setting "expiry_s" (Forget aircraft after) of the stack "flarm". Unit: s. Silence after which an aircraft leaves the table.
static inline int32_t sdrkit_flarm_get_expiry_s(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "expiry_s", value);
}
static inline int32_t sdrkit_flarm_set_expiry_s(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "expiry_s", value);
}

// The setting "gen_centre" (Test signal centre) of the test signal generator of the stack "flarm". The position the simulated aircraft circle.
static inline int32_t sdrkit_flarm_gen_get_centre(SDRKIT_HANDLE handle, sdrkit_position *value) {
  return sdrkit_typed_setting_get_position(handle, "gen_centre", value);
}
static inline int32_t sdrkit_flarm_gen_set_centre(SDRKIT_HANDLE handle, sdrkit_position value) {
  return sdrkit_typed_setting_set_position(handle, "gen_centre", value);
}

// Fills `rows` with the changed rows of the table "aircraft" of the stack "adsl" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_adsl_aircraft_rows(SDRKIT_HANDLE handle,
                                               sdrkit_adsl_aircraft_row *rows,
                                               uint32_t capacity,
                                               sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "drones.AdslAircraftRow", "aircraft", rows, sizeof(sdrkit_adsl_aircraft_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "aircraft_tracks" of the stack "adsl" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_adsl_aircraft_tracks_rows(SDRKIT_HANDLE handle,
                                                      sdrkit_drones_aircraft_track_row *rows,
                                                      uint32_t capacity,
                                                      sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "drones.AircraftTrackRow", "aircraft_tracks", rows, sizeof(sdrkit_drones_aircraft_track_row), capacity, info);
}

// Fills `event` with event `n` of the last sdrkit_events_take() on `handle`, an event of the type "adsl.aircraft"
// (ADS-L aircraft). Returns SDRKIT_E_WRONG_TYPE if the event is of another type.
static inline int32_t sdrkit_adsl_aircraft_event_read(SDRKIT_HANDLE handle, uint32_t n, sdrkit_adsl_aircraft_event *event) {
  return sdrkit_typed_event(handle, n, "adsl.aircraft", "drones.AdslAircraftEvent", event, sizeof(sdrkit_adsl_aircraft_event));
}

// Fills `event` with event `n` of the last sdrkit_events_take() on `handle`, an event of the type "adsl.emergency"
// (ADS-L emergency). Returns SDRKIT_E_WRONG_TYPE if the event is of another type.
static inline int32_t sdrkit_adsl_emergency_event_read(SDRKIT_HANDLE handle, uint32_t n, sdrkit_adsl_emergency_event *event) {
  return sdrkit_typed_event(handle, n, "adsl.emergency", "drones.AdslEmergencyEvent", event, sizeof(sdrkit_adsl_emergency_event));
}

// Runs the command "clear" (Clear) of the stack "adsl" on the session of `handle`. Forgets everything heard so far. Favourites stay.
static inline int32_t sdrkit_adsl_clear(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "clear", NULL, NULL, NULL, NULL, 0);
}

// Runs the command "remove" (Remove) of the stack "adsl" on the session of `handle`. Forgets one object, a favourite too.
static inline int32_t sdrkit_adsl_remove(SDRKIT_HANDLE handle, const sdrkit_drones_aircraft_args *args) {
  return sdrkit_typed_command(handle, "remove", "drones.AircraftArgs", args, NULL, NULL, 0);
}

// The settings of the test signal generator of the stack "adsl" are the sdrkit_adsl_gen_*() functions
// after the settings of its receiver.

// The setting "m1_hz" (M-band 1) of the stack "adsl". A frequency in Hz. The first Manchester channel. The default is the frequency of the test signal.
static inline int32_t sdrkit_adsl_get_m1_hz(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "m1_hz", value);
}
static inline int32_t sdrkit_adsl_set_m1_hz(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "m1_hz", value);
}

// The setting "m2_hz" (M-band 2) of the stack "adsl". A frequency in Hz. The second Manchester channel. The default is the frequency of the test signal.
static inline int32_t sdrkit_adsl_get_m2_hz(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "m2_hz", value);
}
static inline int32_t sdrkit_adsl_set_m2_hz(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "m2_hz", value);
}

// The setting "o_hz" (O-band) of the stack "adsl". A frequency in Hz. The channel of the low and the high data rate. The default is the frequency of the test signal.
static inline int32_t sdrkit_adsl_get_o_hz(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "o_hz", value);
}
static inline int32_t sdrkit_adsl_set_o_hz(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "o_hz", value);
}

// The setting "expiry_s" (Forget aircraft after) of the stack "adsl". Unit: s.
static inline int32_t sdrkit_adsl_get_expiry_s(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "expiry_s", value);
}
static inline int32_t sdrkit_adsl_set_expiry_s(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "expiry_s", value);
}

// The setting "with_flarm" (Decode FLARM from the same capture) of the stack "adsl".
static inline int32_t sdrkit_adsl_get_with_flarm(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "with_flarm", value);
}
static inline int32_t sdrkit_adsl_set_with_flarm(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "with_flarm", value);
}

// The setting "tone_chips" (M-band chip detector by tone energy) of the stack "adsl". Off selects the FM discriminator, for comparisons.
static inline int32_t sdrkit_adsl_get_tone_chips(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "tone_chips", value);
}
static inline int32_t sdrkit_adsl_set_tone_chips(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "tone_chips", value);
}

// The setting "wall_clock" (Key time from this computer's clock) of the stack "adsl". On for a radio: the packets' key time is the computer's clock (keep it right to a few seconds). Off, or for a recording or the test signal: the start time plus signal time.
static inline int32_t sdrkit_adsl_get_wall_clock(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "wall_clock", value);
}
static inline int32_t sdrkit_adsl_set_wall_clock(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "wall_clock", value);
}

// The setting "start_time" (Start time (unix seconds)) of the stack "adsl". Unit: s. The key time of the first sample when the computer's clock is not used. 0: the start of playback.
static inline int32_t sdrkit_adsl_get_start_time(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "start_time", value);
}
static inline int32_t sdrkit_adsl_set_start_time(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "start_time", value);
}

// The setting "gen_centre" (Test signal centre) of the test signal generator of the stack "adsl".
static inline int32_t sdrkit_adsl_gen_get_centre(SDRKIT_HANDLE handle, sdrkit_position *value) {
  return sdrkit_typed_setting_get_position(handle, "gen_centre", value);
}
static inline int32_t sdrkit_adsl_gen_set_centre(SDRKIT_HANDLE handle, sdrkit_position value) {
  return sdrkit_typed_setting_set_position(handle, "gen_centre", value);
}

// The setting "gen_aircraft" (Test signal aircraft) of the test signal generator of the stack "adsl".
static inline int32_t sdrkit_adsl_gen_get_aircraft(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "gen_aircraft", value);
}
static inline int32_t sdrkit_adsl_gen_set_aircraft(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "gen_aircraft", value);
}

// The setting "gen_key_index" (Test signal key index) of the test signal generator of the stack "adsl".
static inline int32_t sdrkit_adsl_gen_get_key_index(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "gen_key_index", value);
}
static inline int32_t sdrkit_adsl_gen_set_key_index(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "gen_key_index", value);
}

// The setting "gen_fec" (Test signal with Reed-Solomon) of the test signal generator of the stack "adsl".
static inline int32_t sdrkit_adsl_gen_get_fec(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "gen_fec", value);
}
static inline int32_t sdrkit_adsl_gen_set_fec(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "gen_fec", value);
}

// The setting "gen_preamble" (Test signal M-band preamble) of the test signal generator of the stack "adsl".
static inline int32_t sdrkit_adsl_gen_get_preamble(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "gen_preamble", value);
}
static inline int32_t sdrkit_adsl_gen_set_preamble(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "gen_preamble", value);
}

// The setting "gen_bad_bit" (Wrong bit in every n-th packet) of the test signal generator of the stack "adsl".
static inline int32_t sdrkit_adsl_gen_get_bad_bit(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "gen_bad_bit", value);
}
static inline int32_t sdrkit_adsl_gen_set_bad_bit(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "gen_bad_bit", value);
}

// Fills `rows` with the changed rows of the table "packets" of the stack "ogn" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_ogn_packets_rows(SDRKIT_HANDLE handle,
                                             sdrkit_ogn_packet_row *rows,
                                             uint32_t capacity,
                                             sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "drones.OgnPacketRow", "packets", rows, sizeof(sdrkit_ogn_packet_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "aircraft" of the stack "ogn" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_ogn_aircraft_rows(SDRKIT_HANDLE handle,
                                              sdrkit_ogn_aircraft_row *rows,
                                              uint32_t capacity,
                                              sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "drones.OgnAircraftRow", "aircraft", rows, sizeof(sdrkit_ogn_aircraft_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "aircraft_tracks" of the stack "ogn" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_ogn_aircraft_tracks_rows(SDRKIT_HANDLE handle,
                                                     sdrkit_drones_aircraft_track_row *rows,
                                                     uint32_t capacity,
                                                     sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "drones.AircraftTrackRow", "aircraft_tracks", rows, sizeof(sdrkit_drones_aircraft_track_row), capacity, info);
}

// Fills `event` with event `n` of the last sdrkit_events_take() on `handle`, an event of the type "ogn.tracker"
// (OGN tracker). Returns SDRKIT_E_WRONG_TYPE if the event is of another type.
static inline int32_t sdrkit_ogn_tracker_event_read(SDRKIT_HANDLE handle, uint32_t n, sdrkit_ogn_tracker_event *event) {
  return sdrkit_typed_event(handle, n, "ogn.tracker", "drones.OgnTrackerEvent", event, sizeof(sdrkit_ogn_tracker_event));
}

// Fills `event` with event `n` of the last sdrkit_events_take() on `handle`, an event of the type "ogn.emergency"
// (OGN tracker emergency). Returns SDRKIT_E_WRONG_TYPE if the event is of another type.
static inline int32_t sdrkit_ogn_emergency_event_read(SDRKIT_HANDLE handle, uint32_t n, sdrkit_ogn_emergency_event *event) {
  return sdrkit_typed_event(handle, n, "ogn.emergency", "drones.OgnEmergencyEvent", event, sizeof(sdrkit_ogn_emergency_event));
}

// Runs the command "clear" (Clear) of the stack "ogn" on the session of `handle`. Forgets everything heard so far. Favourites stay.
static inline int32_t sdrkit_ogn_clear(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "clear", NULL, NULL, NULL, NULL, 0);
}

// Runs the command "remove" (Remove) of the stack "ogn" on the session of `handle`. Forgets one object, a favourite too.
static inline int32_t sdrkit_ogn_remove(SDRKIT_HANDLE handle, const sdrkit_drones_aircraft_args *args) {
  return sdrkit_typed_command(handle, "remove", "drones.AircraftArgs", args, NULL, NULL, 0);
}

// The settings of the test signal generator of the stack "ogn" are the sdrkit_ogn_gen_*() functions
// after the settings of its receiver.

// The setting "channels" (Channels) of the stack "ogn". The count of channels of the plan of your area. The tuned frequency is the middle of the plan. The default is the plan of the test signal.
static inline int32_t sdrkit_ogn_get_channels(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "channels", value);
}
static inline int32_t sdrkit_ogn_set_channels(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "channels", value);
}

// The setting "channel_step_hz" (Channel spacing) of the stack "ogn". A frequency in Hz. The distance between two channels. The default is the plan of the test signal.
static inline int32_t sdrkit_ogn_get_channel_step_hz(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "channel_step_hz", value);
}
static inline int32_t sdrkit_ogn_set_channel_step_hz(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "channel_step_hz", value);
}

// The setting "expiry_s" (Forget trackers after) of the stack "ogn". Unit: s.
static inline int32_t sdrkit_ogn_get_expiry_s(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "expiry_s", value);
}
static inline int32_t sdrkit_ogn_set_expiry_s(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "expiry_s", value);
}

// The setting "gen_centre" (Test signal centre) of the test signal generator of the stack "ogn".
static inline int32_t sdrkit_ogn_gen_get_centre(SDRKIT_HANDLE handle, sdrkit_position *value) {
  return sdrkit_typed_setting_get_position(handle, "gen_centre", value);
}
static inline int32_t sdrkit_ogn_gen_set_centre(SDRKIT_HANDLE handle, sdrkit_position value) {
  return sdrkit_typed_setting_set_position(handle, "gen_centre", value);
}

// The setting "gen_trackers" (Test signal trackers) of the test signal generator of the stack "ogn".
static inline int32_t sdrkit_ogn_gen_get_trackers(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "gen_trackers", value);
}
static inline int32_t sdrkit_ogn_gen_set_trackers(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "gen_trackers", value);
}

// The setting "gen_bad_bits" (Wrong bits in every third packet) of the test signal generator of the stack "ogn".
static inline int32_t sdrkit_ogn_gen_get_bad_bits(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "gen_bad_bits", value);
}
static inline int32_t sdrkit_ogn_gen_set_bad_bits(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "gen_bad_bits", value);
}

// The setting "gen_emergency" (Test signal with an emergency) of the test signal generator of the stack "ogn".
static inline int32_t sdrkit_ogn_gen_get_emergency(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "gen_emergency", value);
}
static inline int32_t sdrkit_ogn_gen_set_emergency(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "gen_emergency", value);
}

// Fills `rows` with the changed rows of the table "links" of the stack "rc_links" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_rc_links_rows(SDRKIT_HANDLE handle,
                                          sdrkit_drones_heard_link_row *rows,
                                          uint32_t capacity,
                                          sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "drones.HeardLinkRow", "links", rows, sizeof(sdrkit_drones_heard_link_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "frames" of the stack "rc_links" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_rc_links_frames_rows(SDRKIT_HANDLE handle,
                                                 sdrkit_drones_link_frame_row *rows,
                                                 uint32_t capacity,
                                                 sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "drones.LinkFrameRow", "frames", rows, sizeof(sdrkit_drones_link_frame_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "ppm_frames" of the stack "rc_links" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_rc_links_ppm_frames_rows(SDRKIT_HANDLE handle,
                                                     sdrkit_rc_links_ppm_frame_row *rows,
                                                     uint32_t capacity,
                                                     sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "drones.RcLinksPpmFrameRow", "ppm_frames", rows, sizeof(sdrkit_rc_links_ppm_frame_row), capacity, info);
}

// Fills `event` with event `n` of the last sdrkit_events_take() on `handle`, an event of the type "rc_links.link"
// (New radio-control link). Returns SDRKIT_E_WRONG_TYPE if the event is of another type.
static inline int32_t sdrkit_rc_links_link_event_read(SDRKIT_HANDLE handle, uint32_t n, sdrkit_rc_links_link_event *event) {
  return sdrkit_typed_event(handle, n, "rc_links.link", "drones.RcLinksLinkEvent", event, sizeof(sdrkit_rc_links_link_event));
}

// Fills `event` with event `n` of the last sdrkit_events_take() on `handle`, an event of the type "rc.ppm_frame"
// (PPM frame). Returns SDRKIT_E_WRONG_TYPE if the event is of another type.
static inline int32_t sdrkit_rc_links_rc_ppm_frame_event_read(SDRKIT_HANDLE handle, uint32_t n, sdrkit_rc_links_rc_ppm_frame_event *event) {
  return sdrkit_typed_event(handle, n, "rc.ppm_frame", "drones.RcPpmFrameEvent", event, sizeof(sdrkit_rc_links_rc_ppm_frame_event));
}

// Runs the command "clear" (Clear) of the stack "rc_links" on the session of `handle`. Forgets everything decoded so far.
static inline int32_t sdrkit_rc_links_clear(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "clear", NULL, NULL, NULL, NULL, 0);
}

// Runs the command "remove" (Remove) of the stack "rc_links" on the session of `handle`. Forgets one object, a favourite too.
static inline int32_t sdrkit_rc_links_remove(SDRKIT_HANDLE handle, const sdrkit_drones_link_args *args) {
  return sdrkit_typed_command(handle, "remove", "drones.LinkArgs", args, NULL, NULL, 0);
}

// The settings of the test signal generator of the stack "rc_links" are the sdrkit_rc_links_gen_*() functions
// after the settings of its receiver.

// The setting "submode" (Link) of the stack "rc_links". A value of `sdrkit_rc_links_submode`. The links searched. All runs each link whose least sample rate the input reaches.
static inline int32_t sdrkit_rc_links_get_submode(SDRKIT_HANDLE handle, sdrkit_rc_links_submode *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "submode", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_rc_links_submode)index;
  return code;
}
static inline int32_t sdrkit_rc_links_set_submode(SDRKIT_HANDLE handle, sdrkit_rc_links_submode value) {
  return sdrkit_typed_setting_set_enum(handle, "submode", (int32_t)value);
}

// The setting "threshold_db" (Burst threshold) of the stack "rc_links". Unit: dB. How far above the noise floor of a frequency bin a burst must rise.
static inline int32_t sdrkit_rc_links_get_threshold_db(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "threshold_db", value);
}
static inline int32_t sdrkit_rc_links_set_threshold_db(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "threshold_db", value);
}

// The setting "flysky_bit_order" (FlySky bit order) of the stack "rc_links". A value of `sdrkit_rc_links_flysky_bit_order`. The bit order of FlySky bytes on the air. No public document gives it; both are tried by default.
static inline int32_t sdrkit_rc_links_get_flysky_bit_order(SDRKIT_HANDLE handle, sdrkit_rc_links_flysky_bit_order *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "flysky_bit_order", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_rc_links_flysky_bit_order)index;
  return code;
}
static inline int32_t sdrkit_rc_links_set_flysky_bit_order(SDRKIT_HANDLE handle, sdrkit_rc_links_flysky_bit_order value) {
  return sdrkit_typed_setting_set_enum(handle, "flysky_bit_order", (int32_t)value);
}

// The setting "dsm_chip_polarity" (DSM chip polarity) of the stack "rc_links". A value of `sdrkit_rc_links_dsm_chip_polarity`. Which frequency a 1 chip of DSM is sent on. No public document gives it.
static inline int32_t sdrkit_rc_links_get_dsm_chip_polarity(SDRKIT_HANDLE handle, sdrkit_rc_links_dsm_chip_polarity *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "dsm_chip_polarity", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_rc_links_dsm_chip_polarity)index;
  return code;
}
static inline int32_t sdrkit_rc_links_set_dsm_chip_polarity(SDRKIT_HANDLE handle, sdrkit_rc_links_dsm_chip_polarity value) {
  return sdrkit_typed_setting_set_enum(handle, "dsm_chip_polarity", (int32_t)value);
}

// The setting "sfhss_crc" (S-FHSS CRC) of the stack "rc_links". A value of `sdrkit_rc_links_sfhss_crc`. The CRC of S-FHSS packets. No public document gives it; both are tried by default and the one that matched is shown.
static inline int32_t sdrkit_rc_links_get_sfhss_crc(SDRKIT_HANDLE handle, sdrkit_rc_links_sfhss_crc *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "sfhss_crc", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_rc_links_sfhss_crc)index;
  return code;
}
static inline int32_t sdrkit_rc_links_set_sfhss_crc(SDRKIT_HANDLE handle, sdrkit_rc_links_sfhss_crc value) {
  return sdrkit_typed_setting_set_enum(handle, "sfhss_crc", (int32_t)value);
}

// The setting "ppm_sync_ms" (Shortest sync gap (PPM)) of the stack "rc_links". Unit: ms.
static inline int32_t sdrkit_rc_links_get_ppm_sync_ms(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "ppm_sync_ms", value);
}
static inline int32_t sdrkit_rc_links_set_ppm_sync_ms(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "ppm_sync_ms", value);
}

// The setting "ppm_channel_min_ms" (Shortest channel (PPM)) of the stack "rc_links". Unit: ms.
static inline int32_t sdrkit_rc_links_get_ppm_channel_min_ms(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "ppm_channel_min_ms", value);
}
static inline int32_t sdrkit_rc_links_set_ppm_channel_min_ms(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "ppm_channel_min_ms", value);
}

// The setting "ppm_channel_max_ms" (Longest channel (PPM)) of the stack "rc_links". Unit: ms.
static inline int32_t sdrkit_rc_links_get_ppm_channel_max_ms(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "ppm_channel_max_ms", value);
}
static inline int32_t sdrkit_rc_links_set_ppm_channel_max_ms(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "ppm_channel_max_ms", value);
}

// The setting "ppm_centre_ms" (Centre of travel (PPM)) of the stack "rc_links". Unit: ms. A channel this long is 0 %; 0.5 ms less is -100 % and 0.5 ms more +100 %.
static inline int32_t sdrkit_rc_links_get_ppm_centre_ms(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "ppm_centre_ms", value);
}
static inline int32_t sdrkit_rc_links_set_ppm_centre_ms(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "ppm_centre_ms", value);
}

// The setting "ppm_row_s" (Row interval (PPM)) of the stack "rc_links". Unit: s.
static inline int32_t sdrkit_rc_links_get_ppm_row_s(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "ppm_row_s", value);
}
static inline int32_t sdrkit_rc_links_set_ppm_row_s(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "ppm_row_s", value);
}

// The setting "gen_channels_ms" (PPM channels) of the test signal generator of the stack "rc_links". Unit: ms. The channel times of the PPM test signal, separated by commas; empty for a steady carrier.
static inline int32_t sdrkit_rc_links_gen_get_channels_ms(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "gen_channels_ms", value);
}
static inline int32_t sdrkit_rc_links_gen_set_channels_ms(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "gen_channels_ms", value);
}

// The setting "gen_frame_ms" (PPM frame) of the test signal generator of the stack "rc_links". Unit: ms.
static inline int32_t sdrkit_rc_links_gen_get_frame_ms(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_frame_ms", value);
}
static inline int32_t sdrkit_rc_links_gen_set_frame_ms(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_frame_ms", value);
}

// The setting "gen_pulse_us" (PPM pulse) of the test signal generator of the stack "rc_links". Unit: us.
static inline int32_t sdrkit_rc_links_gen_get_pulse_us(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_pulse_us", value);
}
static inline int32_t sdrkit_rc_links_gen_set_pulse_us(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_pulse_us", value);
}

// The setting "gen_jitter_us" (PPM timing jitter) of the test signal generator of the stack "rc_links". Unit: us.
static inline int32_t sdrkit_rc_links_gen_get_jitter_us(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_jitter_us", value);
}
static inline int32_t sdrkit_rc_links_gen_set_jitter_us(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_jitter_us", value);
}

// The setting "gen_ppm_clock_ppm" (PPM transmitter clock error) of the test signal generator of the stack "rc_links". Unit: ppm.
static inline int32_t sdrkit_rc_links_gen_get_ppm_clock_ppm(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_ppm_clock_ppm", value);
}
static inline int32_t sdrkit_rc_links_gen_set_ppm_clock_ppm(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_ppm_clock_ppm", value);
}

// The setting "gen_ppm_depth" (PPM keying depth) of the test signal generator of the stack "rc_links".
static inline int32_t sdrkit_rc_links_gen_get_ppm_depth(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_ppm_depth", value);
}
static inline int32_t sdrkit_rc_links_gen_set_ppm_depth(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_ppm_depth", value);
}

// The setting "gen_ppm_invert" (PPM carrier only in the pulse) of the test signal generator of the stack "rc_links". Off: the pulse is a notch in a steady carrier. On: the carrier is on only during the pulse.
static inline int32_t sdrkit_rc_links_gen_get_ppm_invert(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "gen_ppm_invert", value);
}
static inline int32_t sdrkit_rc_links_gen_set_ppm_invert(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "gen_ppm_invert", value);
}

// Fills `rows` with the changed rows of the table "links" of the stack "mavlink_sik" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_mavlink_sik_links_rows(SDRKIT_HANDLE handle,
                                                   sdrkit_drones_heard_link_row *rows,
                                                   uint32_t capacity,
                                                   sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "drones.HeardLinkRow", "links", rows, sizeof(sdrkit_drones_heard_link_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "frames" of the stack "mavlink_sik" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_mavlink_sik_frames_rows(SDRKIT_HANDLE handle,
                                                    sdrkit_drones_link_frame_row *rows,
                                                    uint32_t capacity,
                                                    sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "drones.LinkFrameRow", "frames", rows, sizeof(sdrkit_drones_link_frame_row), capacity, info);
}

// Fills `event` with event `n` of the last sdrkit_events_take() on `handle`, an event of the type "mavlink_sik.vehicle"
// (New MAVLink vehicle). Returns SDRKIT_E_WRONG_TYPE if the event is of another type.
static inline int32_t sdrkit_mavlink_sik_vehicle_event_read(SDRKIT_HANDLE handle, uint32_t n, sdrkit_mavlink_sik_vehicle_event *event) {
  return sdrkit_typed_event(handle, n, "mavlink_sik.vehicle", "drones.MavlinkSikVehicleEvent", event, sizeof(sdrkit_mavlink_sik_vehicle_event));
}

// Runs the command "clear" (Clear) of the stack "mavlink_sik" on the session of `handle`. Forgets everything decoded so far.
static inline int32_t sdrkit_mavlink_sik_clear(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "clear", NULL, NULL, NULL, NULL, 0);
}

// Runs the command "remove" (Remove) of the stack "mavlink_sik" on the session of `handle`. Forgets one object, a favourite too.
static inline int32_t sdrkit_mavlink_sik_remove(SDRKIT_HANDLE handle, const sdrkit_drones_link_args *args) {
  return sdrkit_typed_command(handle, "remove", "drones.LinkArgs", args, NULL, NULL, 0);
}

// The setting "air_rate" (Air speed) of the stack "mavlink_sik". A value of `sdrkit_mavlink_sik_air_rate`. The air rate of the radios, kbit/s. Auto searches every rate that the sample rate allows (at least 2.5 samples a bit).
static inline int32_t sdrkit_mavlink_sik_get_air_rate(SDRKIT_HANDLE handle, sdrkit_mavlink_sik_air_rate *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "air_rate", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_mavlink_sik_air_rate)index;
  return code;
}
static inline int32_t sdrkit_mavlink_sik_set_air_rate(SDRKIT_HANDLE handle, sdrkit_mavlink_sik_air_rate value) {
  return sdrkit_typed_setting_set_enum(handle, "air_rate", (int32_t)value);
}

// The setting "threshold_db" (Burst threshold) of the stack "mavlink_sik". Unit: dB. How far above the noise floor of a frequency bin a burst must rise.
static inline int32_t sdrkit_mavlink_sik_get_threshold_db(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "threshold_db", value);
}
static inline int32_t sdrkit_mavlink_sik_set_threshold_db(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "threshold_db", value);
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_remote_id_fields_read(SDRKIT_HANDLE handle, sdrkit_remote_id_fields *fields) {
  return sdrkit_typed_fields(handle, "drones.RemoteIdFields", fields, sizeof(sdrkit_remote_id_fields));
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_dji_droneid_fields_read(SDRKIT_HANDLE handle, sdrkit_dji_droneid_fields *fields) {
  return sdrkit_typed_fields(handle, "drones.DjiDroneidFields", fields, sizeof(sdrkit_dji_droneid_fields));
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_flarm_fields_read(SDRKIT_HANDLE handle, sdrkit_flarm_fields *fields) {
  return sdrkit_typed_fields(handle, "drones.FlarmFields", fields, sizeof(sdrkit_flarm_fields));
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_adsl_fields_read(SDRKIT_HANDLE handle, sdrkit_adsl_fields *fields) {
  return sdrkit_typed_fields(handle, "drones.AdslFields", fields, sizeof(sdrkit_adsl_fields));
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_ogn_fields_read(SDRKIT_HANDLE handle, sdrkit_ogn_fields *fields) {
  return sdrkit_typed_fields(handle, "drones.OgnFields", fields, sizeof(sdrkit_ogn_fields));
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_drones_link_fields_read(SDRKIT_HANDLE handle, sdrkit_drones_link_fields *fields) {
  return sdrkit_typed_fields(handle, "drones.LinkFields", fields, sizeof(sdrkit_drones_link_fields));
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_drones_decoder_fields_read(SDRKIT_HANDLE handle, sdrkit_drones_decoder_fields *fields) {
  return sdrkit_typed_fields(handle, "drones.DecoderFields", fields, sizeof(sdrkit_drones_decoder_fields));
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_rc_links_ppm_fields_read(SDRKIT_HANDLE handle, sdrkit_rc_links_ppm_fields *fields) {
  return sdrkit_typed_fields(handle, "drones.RcLinksPpmFields", fields, sizeof(sdrkit_rc_links_ppm_fields));
}

#endif
