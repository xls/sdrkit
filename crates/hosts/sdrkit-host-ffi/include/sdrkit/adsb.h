/*
 * sdrkit: the typed interface of the stack family "adsb", version 1.2. MIT licence.
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

#ifndef SDRKIT_ADSB_H
#define SDRKIT_ADSB_H

#include "../sdrkit.h"

// The version of the family that this header describes.
#define SDRKIT_ADSB_VERSION_MAJOR 1
#define SDRKIT_ADSB_VERSION_MINOR 2

// The stack "adsb": its id, and the keys of its event types.
#define SDRKIT_STACK_ADSB "adsb"
#define SDRKIT_EVENT_ADSB_AIRCRAFT "adsb.aircraft"

// The stack "uat": its id, and the keys of its event types.
#define SDRKIT_STACK_UAT "uat"
#define SDRKIT_EVENT_UAT_MESSAGE "uat.message"

// The stack "modeac": its id, and the keys of its event types.
#define SDRKIT_STACK_MODEAC "modeac"
#define SDRKIT_EVENT_MODEAC_REPLY "modeac.reply"

// The stack "uplink1030": its id, and the keys of its event types.
#define SDRKIT_STACK_UPLINK1030 "uplink1030"
#define SDRKIT_EVENT_UPLINK_INTERROGATION "uplink.interrogation"

// The options of the setting "correction" of the stack "adsb".
typedef enum sdrkit_adsb_correction {
  // Off
  SDRKIT_ADSB_CORRECTION_OFF = 0,
  // 1 bit
  SDRKIT_ADSB_CORRECTION_1BIT = 1,
  // Up to 3 bits
  SDRKIT_ADSB_CORRECTION_3BITS = 2,
} sdrkit_adsb_correction;

// The options of the setting "effort" of the stack "adsb".
typedef enum sdrkit_adsb_effort {
  // Automatic
  SDRKIT_ADSB_EFFORT_AUTO = 0,
  // All passes
  SDRKIT_ADSB_EFFORT_ALL_PASSES = 1,
  // Strong candidates only
  SDRKIT_ADSB_EFFORT_STRONG_CANDIDATES_ONLY = 2,
  // Plain detector
  SDRKIT_ADSB_EFFORT_PLAIN_DETECTOR = 3,
} sdrkit_adsb_effort;

// The options of the setting "asterix_cat" of the stack "adsb".
typedef enum sdrkit_adsb_asterix_cat {
  // 021, 048 and 062
  SDRKIT_ADSB_ASTERIX_CAT_ALL = 0,
  // 021 only
  SDRKIT_ADSB_ASTERIX_CAT_021_ONLY = 1,
  // 048 only
  SDRKIT_ADSB_ASTERIX_CAT_048_ONLY = 2,
  // 062 only
  SDRKIT_ADSB_ASTERIX_CAT_062_ONLY = 3,
} sdrkit_adsb_asterix_cat;

// The result fields of the tracker.
typedef struct sdrkit_adsb_tracker_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Aircraft in the table.
  int64_t aircraft_count;
  // Aircraft with a position.
  int64_t with_position;
  // Good messages per second over the last second. A frequency in Hz.
  double message_rate;
  // True if `level_dbfs` holds a value.
  bool has_level_dbfs;
  // The mean level of the messages. Unit: dBFS.
  double level_dbfs;
  // The noise floor. Unit: dBFS.
  double noise_dbfs;
  // True if `snr_db` holds a value.
  bool has_snr_db;
  // The mean signal to noise ratio of the messages. Unit: dB.
  double snr_db;
  // True if `max_range_m` holds a value.
  bool has_max_range_m;
  // The farthest position counted. Unit: m.
  double max_range_m;
  // Reports made.
  int64_t report;
  // The signal time of the report, seconds from the start of the input. Unit: s.
  double time_s;
  // Messages that passed their check.
  int64_t frames_ok;
  // Messages that failed their check.
  int64_t frames_bad;
  // Messages repaired.
  int64_t repaired;
  // Messages at least 3 dB below full scale.
  int64_t loud;
  // Preamble candidates.
  int64_t candidates;
  // The demodulator effort in use.
  int64_t effort;
  // True if `position_hint` holds a value.
  bool has_position_hint;
  // A station position that fits the aircraft heard, when the one given is far from all of them. Unit: °.
  sdrkit_position position_hint;
  // Positions counted in the range statistics.
  int64_t coverage_positions;
  // True if `coverage_max_alt_ft` holds a value.
  bool has_coverage_max_alt_ft;
  // The highest altitude counted. Unit: ft.
  int64_t coverage_max_alt_ft;
} sdrkit_adsb_tracker_fields;

// The result fields of the feeds out.
typedef struct sdrkit_adsb_feed_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Messages sent to the feeds.
  int64_t feed_messages;
} sdrkit_adsb_feed_fields;

// The result fields of the Mode A/C decoder.
typedef struct sdrkit_modeac_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Replies decoded.
  int64_t modeac_replies;
  // Codes shown.
  int64_t modeac_codes;
} sdrkit_modeac_fields;

// The options of `fix` of `sdrkit_adsb_aircraft_row`.
typedef enum sdrkit_adsb_aircraft_row_fix {
  // Global
  SDRKIT_ADSB_AIRCRAFT_ROW_FIX_GLOBAL = 0,
  // Local
  SDRKIT_ADSB_AIRCRAFT_ROW_FIX_LOCAL = 1,
  // Local from the receiver
  SDRKIT_ADSB_AIRCRAFT_ROW_FIX_LOCAL_FROM_RECEIVER = 2,
  // CPR pair waiting
  SDRKIT_ADSB_AIRCRAFT_ROW_FIX_CPR_PAIR = 3,
  // CPR even waiting
  SDRKIT_ADSB_AIRCRAFT_ROW_FIX_CPR_EVEN = 4,
  // CPR odd waiting
  SDRKIT_ADSB_AIRCRAFT_ROW_FIX_CPR_ODD = 5,
} sdrkit_adsb_aircraft_row_fix;

// The options of `speed_kind` of `sdrkit_adsb_aircraft_row`.
typedef enum sdrkit_adsb_aircraft_row_speed_kind {
  // Ground speed
  SDRKIT_ADSB_AIRCRAFT_ROW_SPEED_KIND_GROUND = 0,
  // Indicated airspeed
  SDRKIT_ADSB_AIRCRAFT_ROW_SPEED_KIND_IAS = 1,
  // True airspeed
  SDRKIT_ADSB_AIRCRAFT_ROW_SPEED_KIND_TAS = 2,
} sdrkit_adsb_aircraft_row_speed_kind;

// A row of the ADS-B aircraft table: the tracker's state for one aircraft. The row key is the address.
typedef struct sdrkit_adsb_aircraft_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The 24-bit address, six hex digits (`~` in front for a non-ICAO address).
  sdrkit_string icao;
  // True if `callsign` holds a value.
  bool has_callsign;
  // The flight identification.
  sdrkit_string callsign;
  // True if `registration` holds a value.
  bool has_registration;
  // The registration derived from the address.
  sdrkit_string registration;
  // True if `country` holds a value.
  bool has_country;
  // The State the address is allocated to.
  sdrkit_string country;
  // True if `category` holds a value.
  bool has_category;
  // The emitter category, set letter and code ("A3").
  sdrkit_string category;
  // True if `squawk` holds a value.
  bool has_squawk;
  // The identity code, four octal digits.
  sdrkit_string squawk;
  // The emergency state (0 none).
  int64_t emergency;
  // True if `emergency_name` holds a value.
  bool has_emergency_name;
  // The emergency state in words.
  sdrkit_string emergency_name;
  // True if `on_ground` holds a value.
  bool has_on_ground;
  // On the ground.
  bool on_ground;
  // The alert bit of the flight status.
  bool alert;
  // The special position identification (ident) bit.
  bool spi;
  // True if `position` holds a value.
  bool has_position;
  // The last position. Unit: °.
  sdrkit_position position;
  // True if `fix` holds a value.
  bool has_fix;
  // How the position was found, or which CPR frames wait for a partner. A value of `sdrkit_adsb_aircraft_row_fix`.
  int32_t fix;
  // True if `position_time` holds a value.
  bool has_position_time;
  // When the position was heard, wall time. Seconds since 1970 (UTC).
  double position_time;
  // True if `alt_ft` holds a value.
  bool has_alt_ft;
  // The barometric altitude. Unit: ft.
  int64_t alt_ft;
  // True if `gnss_alt_ft` holds a value.
  bool has_gnss_alt_ft;
  // The GNSS height. Unit: ft.
  int64_t gnss_alt_ft;
  // True if `speed_kt` holds a value.
  bool has_speed_kt;
  // The speed. Unit: kt.
  double speed_kt;
  // True if `speed_kind` holds a value.
  bool has_speed_kind;
  // What the speed is. A value of `sdrkit_adsb_aircraft_row_speed_kind`.
  int32_t speed_kind;
  // True if `heading_deg` holds a value.
  bool has_heading_deg;
  // The track or heading. Unit: °.
  double heading_deg;
  // True if `heading_is_track` holds a value.
  bool has_heading_is_track;
  // Whether the heading is the track over the ground.
  bool heading_is_track;
  // True if `vertical_rate_fpm` holds a value.
  bool has_vertical_rate_fpm;
  // The vertical rate. Unit: ft/min.
  int64_t vertical_rate_fpm;
  // True if `gnss_minus_baro_ft` holds a value.
  bool has_gnss_minus_baro_ft;
  // GNSS height minus barometric altitude. Unit: ft.
  int64_t gnss_minus_baro_ft;
  // True if `selected_alt_ft` holds a value.
  bool has_selected_alt_ft;
  // The selected altitude. Unit: ft.
  int64_t selected_alt_ft;
  // True if `selected_heading_deg` holds a value.
  bool has_selected_heading_deg;
  // The selected heading. Unit: °.
  double selected_heading_deg;
  // True if `baro_setting_hpa` holds a value.
  bool has_baro_setting_hpa;
  // The barometric pressure setting. Unit: hPa.
  double baro_setting_hpa;
  // True if `adsb_version` holds a value.
  bool has_adsb_version;
  // The ADS-B version number.
  int64_t adsb_version;
  // True if `nacp` holds a value.
  bool has_nacp;
  // The navigation accuracy category for position.
  int64_t nacp;
  // True if `sil` holds a value.
  bool has_sil;
  // The source integrity level.
  int64_t sil;
  // True if `nic` holds a value.
  bool has_nic;
  // The navigation integrity category.
  int64_t nic;
  // True if `ias_kt` holds a value.
  bool has_ias_kt;
  // The indicated airspeed. Unit: kt.
  double ias_kt;
  // True if `mach` holds a value.
  bool has_mach;
  // The Mach number.
  double mach;
  // True if `roll_deg` holds a value.
  bool has_roll_deg;
  // The roll angle. Unit: °.
  double roll_deg;
  // True if `wind_derived_from_deg` holds a value.
  bool has_wind_derived_from_deg;
  // Where the derived wind blows from. Unit: °.
  double wind_derived_from_deg;
  // True if `wind_derived_kt` holds a value.
  bool has_wind_derived_kt;
  // The derived wind speed. Unit: kt.
  double wind_derived_kt;
  // True if `wind_reported_from_deg` holds a value.
  bool has_wind_reported_from_deg;
  // Where the reported wind blows from. Unit: °.
  double wind_reported_from_deg;
  // True if `wind_reported_kt` holds a value.
  bool has_wind_reported_kt;
  // The reported wind speed. Unit: kt.
  double wind_reported_kt;
  // True if `sat_derived_c` holds a value.
  bool has_sat_derived_c;
  // The static air temperature from TAS and Mach. Unit: °C.
  double sat_derived_c;
  // True if `sat_reported_c` holds a value.
  bool has_sat_reported_c;
  // The static air temperature reported. Unit: °C.
  double sat_reported_c;
  // True if `pressure_hpa` holds a value.
  bool has_pressure_hpa;
  // The static pressure reported. Unit: hPa.
  double pressure_hpa;
  // True if `humidity_pct` holds a value.
  bool has_humidity_pct;
  // The humidity reported. Unit: %.
  double humidity_pct;
  // True if `hazards` holds a value.
  bool has_hazards;
  // The hazards reported, with their levels.
  sdrkit_string hazards;
  // True if `advisory` holds a value.
  bool has_advisory;
  // The ACAS resolution advisory in words.
  sdrkit_string advisory;
  // True if `advisory_threat` holds a value.
  bool has_advisory_threat;
  // The address of the threat of the advisory.
  sdrkit_string advisory_threat;
  // True if `advisory_time` holds a value.
  bool has_advisory_time;
  // When the advisory was heard, wall time. Seconds since 1970 (UTC).
  double advisory_time;
  // True if `subnetwork_version` holds a value.
  bool has_subnetwork_version;
  // The Mode S subnetwork version.
  int64_t subnetwork_version;
  // True if `acas_ra_capable` holds a value.
  bool has_acas_ra_capable;
  // ACAS generates resolution advisories.
  bool acas_ra_capable;
  // True if `gicb` holds a value.
  bool has_gicb;
  // The GICB registers the transponder holds.
  sdrkit_string gicb;
  // Comm-B messages were heard.
  bool datalink;
  // Heard through TIS-B.
  bool tisb;
  // True if `level_dbfs` holds a value.
  bool has_level_dbfs;
  // The mean pulse level. Unit: dBFS.
  double level_dbfs;
  // True if `distance_m` holds a value.
  bool has_distance_m;
  // The distance from the station. Unit: m.
  double distance_m;
  // True if `bearing_deg` holds a value.
  bool has_bearing_deg;
  // The bearing from the station. Unit: °.
  double bearing_deg;
  // Where the last update came from: the row key of the sources table ("0" this receiver, else the number of the feed).
  sdrkit_string source;
  // The trail, packed.
  sdrkit_bytes trail;
  // True if `heard` holds a value.
  bool has_heard;
  // Times heard.
  int64_t heard;
  // True if `last_heard` holds a value.
  bool has_last_heard;
  // Wall-clock time when last heard, in whole seconds. Seconds since 1970 (UTC).
  double last_heard;
} sdrkit_adsb_aircraft_row;

// A row of the message monitor: one good message of this receiver.
typedef struct sdrkit_adsb_message_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // Signal time of the preamble. Unit: s.
  double time_s;
  // The frame in hex.
  sdrkit_string hex;
  // The address.
  sdrkit_string icao;
  // The downlink format.
  int64_t df;
  // Bits the repair flipped.
  int64_t fixed_bits;
  // Level. Unit: dBFS.
  double level_dbfs;
  // The description in words.
  sdrkit_string message;
} sdrkit_adsb_message_row;

// The options of `format` of `sdrkit_adsb_source_row`.
typedef enum sdrkit_adsb_source_row_format {
  // Beast
  SDRKIT_ADSB_SOURCE_ROW_FORMAT_BEAST = 0,
  // SBS
  SDRKIT_ADSB_SOURCE_ROW_FORMAT_SBS = 1,
  // ASTERIX
  SDRKIT_ADSB_SOURCE_ROW_FORMAT_ASTERIX = 2,
} sdrkit_adsb_source_row_format;

// A receiver that the table takes messages from. The row key is the number of the source: "0" for this receiver, then the number of each feed in.
typedef struct sdrkit_adsb_source_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The name.
  sdrkit_string name;
  // True if `format` holds a value.
  bool has_format;
  // The format, known after the first bytes. A value of `sdrkit_adsb_source_row_format`.
  int32_t format;
  // Connected now.
  bool connected;
  // Messages taken.
  int64_t messages;
  // Aircraft whose last update came from it.
  int64_t aircraft;
  // True if `error` holds a value.
  bool has_error;
  // The last error.
  sdrkit_string error;
} sdrkit_adsb_source_row;

// A count of messages of one format. The row key is the format number in two digits.
typedef struct sdrkit_adsb_count_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // A downlink or uplink format number.
  int64_t format_number;
  // How many.
  int64_t count;
} sdrkit_adsb_count_row;

// A GPS interference cell of half a degree with at least three aircraft.
typedef struct sdrkit_adsb_gps_cell_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The south-west corner of the cell. Unit: °.
  sdrkit_position corner;
  // Aircraft in the cell.
  int64_t aircraft;
  // The level, 0 to 3.
  int64_t level;
} sdrkit_adsb_gps_cell_row;

// A feed out: a server or an outbound target. The row key is the name.
typedef struct sdrkit_adsb_feed_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The server ("beast", "sbs", "asterix") or the outbound target ("format@host:port").
  sdrkit_string name;
  // True if `port` holds a value.
  bool has_port;
  // The TCP port of a server or a target.
  int64_t port;
  // True if `clients` holds a value.
  bool has_clients;
  // Clients connected to a server.
  int64_t clients;
  // True if `dropped` holds a value.
  bool has_dropped;
  // Messages dropped because a client or target was slow.
  int64_t dropped;
  // True if `connected` holds a value.
  bool has_connected;
  // A target is connected.
  bool connected;
  // True if `error` holds a value.
  bool has_error;
  // Why a server did not start.
  sdrkit_string error;
} sdrkit_adsb_feed_row;

// A Mode A/C code heard at least the set number of times. The row key is the code.
typedef struct sdrkit_modeac_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The 13 code bits in hex.
  sdrkit_string code;
  // The code read as a squawk.
  sdrkit_string squawk;
  // True if `alt_ft` holds a value.
  bool has_alt_ft;
  // The code read as a Mode C altitude. Unit: ft.
  int64_t alt_ft;
  // True if `replies` holds a value.
  bool has_replies;
  // Replies heard.
  int64_t replies;
  // An SPI pulse was heard.
  bool spi;
  // Level of the last reply. Unit: dBFS.
  double level_dbfs;
  // True if `last_heard` holds a value.
  bool has_last_heard;
  // Wall-clock time when last heard, in whole seconds. Seconds since 1970 (UTC).
  double last_heard;
} sdrkit_modeac_row;

// The result fields of the UAT tracker.
typedef struct sdrkit_uat_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Aircraft in the table.
  int64_t aircraft_count;
  // Aircraft with a position.
  int64_t with_position;
  // Good messages per second over the last second. A frequency in Hz.
  double message_rate;
  // FIS-B products held.
  int64_t fisb_products;
  // TIS-B messages among the last 64.
  int64_t tisb_recent;
  // FIS-B files waiting for segments.
  int64_t fisb_pending;
  // Frames that passed their check.
  int64_t frames_ok;
  // True if `snr_db` holds a value.
  bool has_snr_db;
  // The signal to noise ratio. Unit: dB.
  double snr_db;
  // The noise floor. Unit: dBFS.
  double noise_dbfs;
  // Reports made.
  int64_t report;
  // The signal time of the report, seconds from the start of the input. Unit: s.
  double time_s;
} sdrkit_uat_fields;

// A row of the UAT aircraft table. The row key is the address.
typedef struct sdrkit_uat_aircraft_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The 24-bit address, six hex digits.
  sdrkit_string icao;
  // The address qualifier.
  int64_t address_qualifier;
  // True if `callsign` holds a value.
  bool has_callsign;
  // The flight identification.
  sdrkit_string callsign;
  // True if `registration` holds a value.
  bool has_registration;
  // The registration derived from the address.
  sdrkit_string registration;
  // True if `country` holds a value.
  bool has_country;
  // The State the address is allocated to.
  sdrkit_string country;
  // True if `squawk` holds a value.
  bool has_squawk;
  // The identity code, four octal digits.
  sdrkit_string squawk;
  // True if `position` holds a value.
  bool has_position;
  // The last position. Unit: °.
  sdrkit_position position;
  // True if `alt_ft` holds a value.
  bool has_alt_ft;
  // The barometric altitude. Unit: ft.
  int64_t alt_ft;
  // True if `geo_alt_ft` holds a value.
  bool has_geo_alt_ft;
  // The geometric altitude. Unit: ft.
  int64_t geo_alt_ft;
  // True if `speed_kt` holds a value.
  bool has_speed_kt;
  // The speed. Unit: kt.
  double speed_kt;
  // True if `heading_deg` holds a value.
  bool has_heading_deg;
  // The track or heading. Unit: °.
  double heading_deg;
  // True if `vertical_rate_fpm` holds a value.
  bool has_vertical_rate_fpm;
  // The vertical rate. Unit: ft/min.
  int64_t vertical_rate_fpm;
  // On the ground.
  bool on_ground;
  // Heard through TIS-B.
  bool tisb;
  // The navigation integrity category.
  int64_t nic;
  // True if `category` holds a value.
  bool has_category;
  // The UAT emitter category.
  int64_t category;
  // The emergency state (0 none).
  int64_t emergency;
  // The level of the last message. Unit: dBFS.
  double level_dbfs;
  // True if `distance_m` holds a value.
  bool has_distance_m;
  // The distance from the station. Unit: m.
  double distance_m;
  // True if `bearing_deg` holds a value.
  bool has_bearing_deg;
  // The bearing from the station. Unit: °.
  double bearing_deg;
  // True if `heard` holds a value.
  bool has_heard;
  // Times heard.
  int64_t heard;
  // True if `last_heard` holds a value.
  bool has_last_heard;
  // Wall-clock time when last heard, in whole seconds. Seconds since 1970 (UTC).
  double last_heard;
} sdrkit_uat_aircraft_row;

// A FIS-B product held. The row key is the number of the product in six digits.
typedef struct sdrkit_uat_fisb_product_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // Signal time the product was received. Unit: s.
  double time_s;
  // The product id.
  int64_t product_id;
  // The product name.
  sdrkit_string name;
  // The product's own time ("MM-DD HH:MM:SS", the parts sent).
  sdrkit_string stamp;
  // The text.
  sdrkit_string text;
  // TWGO records in the product.
  int64_t records;
  // Grid blocks in the product.
  int64_t blocks;
} sdrkit_uat_fisb_product_row;

// The options of `kind` of `sdrkit_uat_fisb_record_row`.
typedef enum sdrkit_uat_fisb_record_row_kind {
  // Text
  SDRKIT_UAT_FISB_RECORD_ROW_KIND_TEXT = 0,
  // Graphic
  SDRKIT_UAT_FISB_RECORD_ROW_KIND_GRAPHIC = 1,
} sdrkit_uat_fisb_record_row_kind;

// The options of `geometry` of `sdrkit_uat_fisb_record_row`.
typedef enum sdrkit_uat_fisb_record_row_geometry {
  // Polygon
  SDRKIT_UAT_FISB_RECORD_ROW_GEOMETRY_POLYGON = 0,
  // Polyline
  SDRKIT_UAT_FISB_RECORD_ROW_GEOMETRY_POLYLINE = 1,
  // Circle
  SDRKIT_UAT_FISB_RECORD_ROW_GEOMETRY_CIRCLE = 2,
  // Point
  SDRKIT_UAT_FISB_RECORD_ROW_GEOMETRY_POINT = 3,
  // Other
  SDRKIT_UAT_FISB_RECORD_ROW_GEOMETRY_OTHER = 4,
} sdrkit_uat_fisb_record_row_geometry;

// A text or graphic record (TWGO) of a product held: a NOTAM, AIRMET, SIGMET, G-AIRMET, CWA, SUA or TFR.
typedef struct sdrkit_uat_fisb_record_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The product the record belongs to: the row key of the products table.
  sdrkit_string product;
  // Signal time the product was received. Unit: s.
  double time_s;
  // The product id.
  int64_t product_id;
  // The product name.
  sdrkit_string name;
  // The report number of the record.
  int64_t report;
  // What the record is. A value of `sdrkit_uat_fisb_record_row_kind`.
  int32_t kind;
  // True if `active` holds a value.
  bool has_active;
  // The record is active (text records).
  bool active;
  // True if `label` holds a value.
  bool has_label;
  // The label of a graphic record.
  sdrkit_string label;
  // True if `overlay` holds a value.
  bool has_overlay;
  // The overlay record identifier.
  int64_t overlay;
  // True if `start` holds a value.
  bool has_start;
  // The start of the validity ("MM-DD HH:MM", the parts sent).
  sdrkit_string start;
  // True if `end` holds a value.
  bool has_end;
  // The end of the validity.
  sdrkit_string end;
  // True if `geometry` holds a value.
  bool has_geometry;
  // The geometry of a graphic record. A value of `sdrkit_uat_fisb_record_row_geometry`.
  int32_t geometry;
  // True if `position` holds a value.
  bool has_position;
  // The first vertex, the centre or the point. Unit: °.
  sdrkit_position position;
  // True if `vertices` holds a value.
  bool has_vertices;
  // The number of vertices of a path. The vertices are rows of the vertices table.
  int64_t vertices;
  // True if `radius_m` holds a value.
  bool has_radius_m;
  // The radius of a circle. Unit: m.
  double radius_m;
  // True if `alt_bottom_ft` holds a value.
  bool has_alt_bottom_ft;
  // The lowest altitude. Unit: ft.
  int64_t alt_bottom_ft;
  // True if `alt_top_ft` holds a value.
  bool has_alt_top_ft;
  // The highest altitude. Unit: ft.
  int64_t alt_top_ft;
  // True if `text` holds a value.
  bool has_text;
  // The text of a text record.
  sdrkit_string text;
} sdrkit_uat_fisb_record_row;

// A block of a gridded product held (NEXRAD, icing, cloud tops, turbulence, lightning).
typedef struct sdrkit_uat_fisb_block_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The product the block belongs to: the row key of the products table.
  sdrkit_string product;
  // Signal time the product was received. Unit: s.
  double time_s;
  // The product id.
  int64_t product_id;
  // The product name.
  sdrkit_string name;
  // The block number.
  int64_t block;
  // The block is in the southern hemisphere.
  bool south;
  // The scale factor (0 high, 1 medium, 2 low resolution).
  int64_t scale;
  // The north-west corner of the block. Unit: °.
  sdrkit_position corner;
  // The height of the block. Unit: °.
  double lat_size_deg;
  // The width of the block. Unit: °.
  double lon_size_deg;
  // The highest bin value.
  int64_t max_bin;
  // True if `bins` holds a value.
  bool has_bins;
  // The 128 bins, one hex digit each, rows from the north. An empty block has none.
  sdrkit_string bins;
} sdrkit_uat_fisb_block_row;

// A vertex of the path of a graphic record held.
typedef struct sdrkit_uat_fisb_vertex_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The record the vertex belongs to: the row key of the records table.
  sdrkit_string record;
  // The place of the vertex in its path, from 0.
  int64_t index;
  // The position. Unit: °.
  sdrkit_position position;
  // The altitude. Unit: ft.
  int64_t alt_ft;
} sdrkit_uat_fisb_vertex_row;

// The options of `kind` of `sdrkit_uat_log_row`.
typedef enum sdrkit_uat_log_row_kind {
  // ADS-B
  SDRKIT_UAT_LOG_ROW_KIND_ADSB = 0,
  // TIS-B
  SDRKIT_UAT_LOG_ROW_KIND_TISB = 1,
  // FIS-B
  SDRKIT_UAT_LOG_ROW_KIND_FISB = 2,
  // UAT
  SDRKIT_UAT_LOG_ROW_KIND_UAT = 3,
} sdrkit_uat_log_row_kind;

// A line of the UAT message log.
typedef struct sdrkit_uat_log_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // Signal time. Unit: s.
  double time_s;
  // What the message is. A value of `sdrkit_uat_log_row_kind`.
  int32_t kind;
  // True if `address` holds a value.
  bool has_address;
  // The address.
  sdrkit_string address;
  // Level. Unit: dBFS.
  double level_dbfs;
  // What it said.
  sdrkit_string summary;
} sdrkit_uat_log_row;

// The result fields of the interrogation decoder.
typedef struct sdrkit_uplink1030_uplink_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Interrogations of all kinds.
  int64_t interrogations;
  // Mode A interrogations.
  int64_t mode_a;
  // Mode C interrogations.
  int64_t mode_c;
  // Mode S interrogations.
  int64_t mode_s;
  // Mode A/C/S all-calls.
  int64_t all_call_a;
  // Mode C/S all-calls.
  int64_t all_call_c;
  // Mode A/C-only all-calls.
  int64_t ac_only_all_call;
} sdrkit_uplink1030_uplink_fields;

// An aircraft that the interrogations address. The row key is the address.
typedef struct sdrkit_uplink1030_addressed_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The interrogated address.
  sdrkit_string icao;
  // True if `interrogations` holds a value.
  bool has_interrogations;
  // Interrogations.
  int64_t interrogations;
  // Uplink formats with their counts ("UF4x3 UF5x1").
  sdrkit_string formats;
  // The fields of the last interrogation.
  sdrkit_string last;
  // True if `last_heard` holds a value.
  bool has_last_heard;
  // Wall-clock time when last heard, in whole seconds. Seconds since 1970 (UTC).
  double last_heard;
} sdrkit_uplink1030_addressed_row;

// An interrogator heard in all-calls. The row key is its code.
typedef struct sdrkit_uplink1030_interrogator_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The interrogator code of an all-call.
  sdrkit_string code;
  // True if `all_calls` holds a value.
  bool has_all_calls;
  // All-calls.
  int64_t all_calls;
  // True if `last_heard` holds a value.
  bool has_last_heard;
  // Wall-clock time when last heard, in whole seconds. Seconds since 1970 (UTC).
  double last_heard;
} sdrkit_uplink1030_interrogator_row;

// The options of `event` of `sdrkit_adsb_aircraft_event`.
typedef enum sdrkit_adsb_aircraft_event_value {
  // New
  SDRKIT_ADSB_AIRCRAFT_EVENT_NEW = 0,
  // Update
  SDRKIT_ADSB_AIRCRAFT_EVENT_UPDATE = 1,
} sdrkit_adsb_aircraft_event_value;

// The options of `urgency` of `sdrkit_adsb_aircraft_event`.
typedef enum sdrkit_adsb_aircraft_event_urgency {
  // None
  SDRKIT_ADSB_AIRCRAFT_EVENT_URGENCY_NONE = 0,
  // Notice
  SDRKIT_ADSB_AIRCRAFT_EVENT_URGENCY_NOTICE = 1,
  // Urgent
  SDRKIT_ADSB_AIRCRAFT_EVENT_URGENCY_URGENT = 2,
  // Emergency
  SDRKIT_ADSB_AIRCRAFT_EVENT_URGENCY_EMERGENCY = 3,
} sdrkit_adsb_aircraft_event_urgency;

// The options of `family` of `sdrkit_adsb_aircraft_event`.
typedef enum sdrkit_adsb_aircraft_event_family {
  // Aircraft
  SDRKIT_ADSB_AIRCRAFT_EVENT_FAMILY_AIRCRAFT = 0,
  // Vessel
  SDRKIT_ADSB_AIRCRAFT_EVENT_FAMILY_VESSEL = 1,
  // APRS
  SDRKIT_ADSB_AIRCRAFT_EVENT_FAMILY_APRS = 2,
  // Pager
  SDRKIT_ADSB_AIRCRAFT_EVENT_FAMILY_PAGER = 3,
  // Radiosonde
  SDRKIT_ADSB_AIRCRAFT_EVENT_FAMILY_SONDE = 4,
  // Broadcast
  SDRKIT_ADSB_AIRCRAFT_EVENT_FAMILY_BROADCAST = 5,
  // Voice
  SDRKIT_ADSB_AIRCRAFT_EVENT_FAMILY_VOICE = 6,
  // Message
  SDRKIT_ADSB_AIRCRAFT_EVENT_FAMILY_MESSAGE = 7,
  // Sensor
  SDRKIT_ADSB_AIRCRAFT_EVENT_FAMILY_SENSOR = 8,
  // Satellite
  SDRKIT_ADSB_AIRCRAFT_EVENT_FAMILY_SATELLITE = 9,
  // Receiver
  SDRKIT_ADSB_AIRCRAFT_EVENT_FAMILY_RECEIVER = 10,
  // Alert
  SDRKIT_ADSB_AIRCRAFT_EVENT_FAMILY_ALERT = 11,
  // Other
  SDRKIT_ADSB_AIRCRAFT_EVENT_FAMILY_OTHER = 12,
} sdrkit_adsb_aircraft_event_family;

// The fields of the event of an aircraft seen or updated. The address is the source. The urgency is an emergency for squawk 7500, 7600 or 7700 and for an emergency state.
typedef struct sdrkit_adsb_aircraft_event {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Seconds since the object started, in the time of the signal.
  double time;
  // The frequency of the signal in Hz.
  double freq_hz;
  // The first event of an aircraft, or a later one. A value of `sdrkit_adsb_aircraft_event_value`.
  int32_t event;
  // The address.
  sdrkit_string source;
  // True if `callsign` holds a value.
  bool has_callsign;
  // The flight identification.
  sdrkit_string callsign;
  // True if `registration` holds a value.
  bool has_registration;
  // The registration derived from the address.
  sdrkit_string registration;
  // True if `category` holds a value.
  bool has_category;
  // The emitter category, set letter and code ("A3").
  sdrkit_string category;
  // True if `squawk` holds a value.
  bool has_squawk;
  // The identity code, four octal digits.
  sdrkit_string squawk;
  // The emergency state (0 none).
  int64_t emergency;
  // True if `on_ground` holds a value.
  bool has_on_ground;
  // On the ground.
  bool on_ground;
  // True if `altitude_ft` holds a value.
  bool has_altitude_ft;
  // The barometric altitude. Unit: ft.
  int64_t altitude_ft;
  // True if `speed_kt` holds a value.
  bool has_speed_kt;
  // The speed over the ground. Unit: kt.
  double speed_kt;
  // True if `course_deg` holds a value.
  bool has_course_deg;
  // The course over the ground. Unit: °.
  double course_deg;
  // True if `vertical_rate_fpm` holds a value.
  bool has_vertical_rate_fpm;
  // The vertical rate. Unit: ft/min.
  int64_t vertical_rate_fpm;
  // True if `position` holds a value.
  bool has_position;
  // The position.
  sdrkit_position position;
  // True if `distance_km` holds a value.
  bool has_distance_km;
  // The distance from the station. Unit: km.
  double distance_km;
  // Messages heard.
  int64_t messages;
  // How urgent the event is. A value of `sdrkit_adsb_aircraft_event_urgency`.
  int32_t urgency;
  // True if `text` holds a value.
  bool has_text;
  // Why the event is urgent.
  sdrkit_string text;
  // What the event is about. A value of `sdrkit_adsb_aircraft_event_family`.
  int32_t family;
} sdrkit_adsb_aircraft_event;

// The options of `kind` of `sdrkit_uat_message_event`.
typedef enum sdrkit_uat_message_event_kind {
  // ADS-B
  SDRKIT_UAT_MESSAGE_EVENT_KIND_ADSB = 0,
  // TIS-B
  SDRKIT_UAT_MESSAGE_EVENT_KIND_TISB = 1,
  // FIS-B
  SDRKIT_UAT_MESSAGE_EVENT_KIND_FISB = 2,
  // UAT
  SDRKIT_UAT_MESSAGE_EVENT_KIND_UAT = 3,
} sdrkit_uat_message_event_kind;

// The options of `urgency` of `sdrkit_uat_message_event`.
typedef enum sdrkit_uat_message_event_urgency {
  // None
  SDRKIT_UAT_MESSAGE_EVENT_URGENCY_NONE = 0,
  // Notice
  SDRKIT_UAT_MESSAGE_EVENT_URGENCY_NOTICE = 1,
  // Urgent
  SDRKIT_UAT_MESSAGE_EVENT_URGENCY_URGENT = 2,
  // Emergency
  SDRKIT_UAT_MESSAGE_EVENT_URGENCY_EMERGENCY = 3,
} sdrkit_uat_message_event_urgency;

// The options of `family` of `sdrkit_uat_message_event`.
typedef enum sdrkit_uat_message_event_family {
  // Aircraft
  SDRKIT_UAT_MESSAGE_EVENT_FAMILY_AIRCRAFT = 0,
  // Vessel
  SDRKIT_UAT_MESSAGE_EVENT_FAMILY_VESSEL = 1,
  // APRS
  SDRKIT_UAT_MESSAGE_EVENT_FAMILY_APRS = 2,
  // Pager
  SDRKIT_UAT_MESSAGE_EVENT_FAMILY_PAGER = 3,
  // Radiosonde
  SDRKIT_UAT_MESSAGE_EVENT_FAMILY_SONDE = 4,
  // Broadcast
  SDRKIT_UAT_MESSAGE_EVENT_FAMILY_BROADCAST = 5,
  // Voice
  SDRKIT_UAT_MESSAGE_EVENT_FAMILY_VOICE = 6,
  // Message
  SDRKIT_UAT_MESSAGE_EVENT_FAMILY_MESSAGE = 7,
  // Sensor
  SDRKIT_UAT_MESSAGE_EVENT_FAMILY_SENSOR = 8,
  // Satellite
  SDRKIT_UAT_MESSAGE_EVENT_FAMILY_SATELLITE = 9,
  // Receiver
  SDRKIT_UAT_MESSAGE_EVENT_FAMILY_RECEIVER = 10,
  // Alert
  SDRKIT_UAT_MESSAGE_EVENT_FAMILY_ALERT = 11,
  // Other
  SDRKIT_UAT_MESSAGE_EVENT_FAMILY_OTHER = 12,
} sdrkit_uat_message_event_family;

// The fields of the event of a decoded UAT message. The address is the source. The urgency is an emergency for squawk 7500, 7600 or 7700 and for an emergency status.
typedef struct sdrkit_uat_message_event {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Seconds since the object started, in the time of the signal.
  double time;
  // The frequency of the signal in Hz.
  double freq_hz;
  // What the message said.
  sdrkit_string text;
  // What the message is. A value of `sdrkit_uat_message_event_kind`.
  int32_t kind;
  // True if `source` holds a value.
  bool has_source;
  // The address.
  sdrkit_string source;
  // The level. Unit: dBFS.
  double level_dbfs;
  // True if `position` holds a value.
  bool has_position;
  // The position decoded from the message.
  sdrkit_position position;
  // How urgent the event is. A value of `sdrkit_uat_message_event_urgency`.
  int32_t urgency;
  // What the event is about. A value of `sdrkit_uat_message_event_family`.
  int32_t family;
} sdrkit_uat_message_event;

// The options of `family` of `sdrkit_modeac_reply_event`.
typedef enum sdrkit_modeac_reply_event_family {
  // Aircraft
  SDRKIT_MODEAC_REPLY_EVENT_FAMILY_AIRCRAFT = 0,
  // Vessel
  SDRKIT_MODEAC_REPLY_EVENT_FAMILY_VESSEL = 1,
  // APRS
  SDRKIT_MODEAC_REPLY_EVENT_FAMILY_APRS = 2,
  // Pager
  SDRKIT_MODEAC_REPLY_EVENT_FAMILY_PAGER = 3,
  // Radiosonde
  SDRKIT_MODEAC_REPLY_EVENT_FAMILY_SONDE = 4,
  // Broadcast
  SDRKIT_MODEAC_REPLY_EVENT_FAMILY_BROADCAST = 5,
  // Voice
  SDRKIT_MODEAC_REPLY_EVENT_FAMILY_VOICE = 6,
  // Message
  SDRKIT_MODEAC_REPLY_EVENT_FAMILY_MESSAGE = 7,
  // Sensor
  SDRKIT_MODEAC_REPLY_EVENT_FAMILY_SENSOR = 8,
  // Satellite
  SDRKIT_MODEAC_REPLY_EVENT_FAMILY_SATELLITE = 9,
  // Receiver
  SDRKIT_MODEAC_REPLY_EVENT_FAMILY_RECEIVER = 10,
  // Alert
  SDRKIT_MODEAC_REPLY_EVENT_FAMILY_ALERT = 11,
  // Other
  SDRKIT_MODEAC_REPLY_EVENT_FAMILY_OTHER = 12,
} sdrkit_modeac_reply_event_family;

// The fields of the event of a Mode A/C reply.
typedef struct sdrkit_modeac_reply_event {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Seconds since the object started, in the time of the signal.
  double time;
  // The frequency of the signal in Hz.
  double freq_hz;
  // The 13 code bits in hex.
  sdrkit_string code;
  // The code read as a squawk.
  sdrkit_string squawk;
  // True if `altitude_ft` holds a value.
  bool has_altitude_ft;
  // The code read as a Mode C altitude. Unit: ft.
  int64_t altitude_ft;
  // An SPI pulse was heard.
  bool spi;
  // The level. Unit: dBFS.
  double level_dbfs;
  // What the event is about. A value of `sdrkit_modeac_reply_event_family`.
  int32_t family;
} sdrkit_modeac_reply_event;

// The options of `kind` of `sdrkit_uplink1030_uplink_interrogation_event`.
typedef enum sdrkit_uplink1030_uplink_interrogation_event_kind {
  // Mode A
  SDRKIT_UPLINK1030_UPLINK_INTERROGATION_EVENT_KIND_MODE_A = 0,
  // Mode C
  SDRKIT_UPLINK1030_UPLINK_INTERROGATION_EVENT_KIND_MODE_C = 1,
  // Mode A/C/S all-call
  SDRKIT_UPLINK1030_UPLINK_INTERROGATION_EVENT_KIND_ALL_CALL_A = 2,
  // Mode C/S all-call
  SDRKIT_UPLINK1030_UPLINK_INTERROGATION_EVENT_KIND_ALL_CALL_C = 3,
  // Mode A/C-only all-call
  SDRKIT_UPLINK1030_UPLINK_INTERROGATION_EVENT_KIND_AC_ONLY_ALL_CALL = 4,
  // Mode S
  SDRKIT_UPLINK1030_UPLINK_INTERROGATION_EVENT_KIND_MODE_S = 5,
} sdrkit_uplink1030_uplink_interrogation_event_kind;

// The options of `family` of `sdrkit_uplink1030_uplink_interrogation_event`.
typedef enum sdrkit_uplink1030_uplink_interrogation_event_family {
  // Aircraft
  SDRKIT_UPLINK1030_UPLINK_INTERROGATION_EVENT_FAMILY_AIRCRAFT = 0,
  // Vessel
  SDRKIT_UPLINK1030_UPLINK_INTERROGATION_EVENT_FAMILY_VESSEL = 1,
  // APRS
  SDRKIT_UPLINK1030_UPLINK_INTERROGATION_EVENT_FAMILY_APRS = 2,
  // Pager
  SDRKIT_UPLINK1030_UPLINK_INTERROGATION_EVENT_FAMILY_PAGER = 3,
  // Radiosonde
  SDRKIT_UPLINK1030_UPLINK_INTERROGATION_EVENT_FAMILY_SONDE = 4,
  // Broadcast
  SDRKIT_UPLINK1030_UPLINK_INTERROGATION_EVENT_FAMILY_BROADCAST = 5,
  // Voice
  SDRKIT_UPLINK1030_UPLINK_INTERROGATION_EVENT_FAMILY_VOICE = 6,
  // Message
  SDRKIT_UPLINK1030_UPLINK_INTERROGATION_EVENT_FAMILY_MESSAGE = 7,
  // Sensor
  SDRKIT_UPLINK1030_UPLINK_INTERROGATION_EVENT_FAMILY_SENSOR = 8,
  // Satellite
  SDRKIT_UPLINK1030_UPLINK_INTERROGATION_EVENT_FAMILY_SATELLITE = 9,
  // Receiver
  SDRKIT_UPLINK1030_UPLINK_INTERROGATION_EVENT_FAMILY_RECEIVER = 10,
  // Alert
  SDRKIT_UPLINK1030_UPLINK_INTERROGATION_EVENT_FAMILY_ALERT = 11,
  // Other
  SDRKIT_UPLINK1030_UPLINK_INTERROGATION_EVENT_FAMILY_OTHER = 12,
} sdrkit_uplink1030_uplink_interrogation_event_family;

// The fields of the event of an interrogation. The interrogated address is the target once it is confirmed.
typedef struct sdrkit_uplink1030_uplink_interrogation_event {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Seconds since the object started, in the time of the signal.
  double time;
  // The frequency of the signal in Hz.
  double freq_hz;
  // What the interrogation is. A value of `sdrkit_uplink1030_uplink_interrogation_event_kind`.
  int32_t kind;
  // The level. Unit: dBFS.
  double level_dbfs;
  // The P2 (side-lobe suppression) pulse was seen.
  bool p2;
  // True if `uf` holds a value.
  bool has_uf;
  // The uplink format of a Mode S interrogation.
  int64_t uf;
  // True if `hex` holds a value.
  bool has_hex;
  // The Mode S message in hex.
  sdrkit_string hex;
  // True if `target` holds a value.
  bool has_target;
  // The interrogated address, once it is confirmed (see `confirmed`).
  sdrkit_string target;
  // True if `text` holds a value.
  bool has_text;
  // The fields of a Mode S interrogation.
  sdrkit_string text;
  // What the event is about. A value of `sdrkit_uplink1030_uplink_interrogation_event_family`.
  int32_t family;
  // True if `confirmed` holds a value.
  bool has_confirmed;
  // Mode S to one aircraft: the address from the parity is in the table of addressed aircraft, so it is the target. False when it is not yet: a damaged message also gives an address, so the target has no value and the address is only in the message. No value for Mode A/C, all-calls and broadcasts.
  bool confirmed;
} sdrkit_uplink1030_uplink_interrogation_event;

// Fills `rows` with the changed rows of the table "aircraft" of the stack "adsb" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The column `callsign` names a row.
// Place of a row: the column `position`.
// Height: `alt_ft`. Course: `heading_deg`. Speed: `speed_kt`.
static inline int32_t sdrkit_adsb_aircraft_rows(SDRKIT_HANDLE handle,
                                               sdrkit_adsb_aircraft_row *rows,
                                               uint32_t capacity,
                                               sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "adsb.AircraftRow", "aircraft", rows, sizeof(sdrkit_adsb_aircraft_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "messages" of the stack "adsb" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The column `icao` names a row.
static inline int32_t sdrkit_adsb_messages_rows(SDRKIT_HANDLE handle,
                                               sdrkit_adsb_message_row *rows,
                                               uint32_t capacity,
                                               sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "adsb.MessageRow", "messages", rows, sizeof(sdrkit_adsb_message_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "sources" of the stack "adsb" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The column `name` names a row.
static inline int32_t sdrkit_adsb_sources_rows(SDRKIT_HANDLE handle,
                                              sdrkit_adsb_source_row *rows,
                                              uint32_t capacity,
                                              sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "adsb.SourceRow", "sources", rows, sizeof(sdrkit_adsb_source_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "by_df" of the stack "adsb" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_adsb_by_df_rows(SDRKIT_HANDLE handle,
                                            sdrkit_adsb_count_row *rows,
                                            uint32_t capacity,
                                            sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "adsb.CountRow", "by_df", rows, sizeof(sdrkit_adsb_count_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "gps_cells" of the stack "adsb" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// Place of a row: the column `corner`.
static inline int32_t sdrkit_adsb_gps_cells_rows(SDRKIT_HANDLE handle,
                                                sdrkit_adsb_gps_cell_row *rows,
                                                uint32_t capacity,
                                                sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "adsb.GpsCellRow", "gps_cells", rows, sizeof(sdrkit_adsb_gps_cell_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "feeds" of the stack "adsb" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The column `name` names a row.
static inline int32_t sdrkit_adsb_feeds_rows(SDRKIT_HANDLE handle,
                                            sdrkit_adsb_feed_row *rows,
                                            uint32_t capacity,
                                            sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "adsb.FeedRow", "feeds", rows, sizeof(sdrkit_adsb_feed_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "modeac" of the stack "adsb" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_adsb_modeac_rows(SDRKIT_HANDLE handle,
                                             sdrkit_modeac_row *rows,
                                             uint32_t capacity,
                                             sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "adsb.ModeAcRow", "modeac", rows, sizeof(sdrkit_modeac_row), capacity, info);
}

// Fills `event` with event `n` of the last sdrkit_events_take() on `handle`, an event of the type "adsb.aircraft"
// (Aircraft seen or updated). Returns SDRKIT_E_WRONG_TYPE if the event is of another type.
static inline int32_t sdrkit_adsb_aircraft_event_read(SDRKIT_HANDLE handle, uint32_t n, sdrkit_adsb_aircraft_event *event) {
  return sdrkit_typed_event(handle, n, "adsb.aircraft", "adsb.AircraftSeen", event, sizeof(sdrkit_adsb_aircraft_event));
}

// Runs the command "clear" (Clear) of the stack "adsb" on the session of `handle`. Forgets everything heard so far. Favourites stay.
static inline int32_t sdrkit_adsb_clear(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "clear", NULL, NULL, NULL, NULL, 0);
}

// Runs the command "reset_stats" (Reset statistics) of the stack "adsb" on the session of `handle`. Starts the range statistics again.
static inline int32_t sdrkit_adsb_reset_stats(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "reset_stats", NULL, NULL, NULL, NULL, 0);
}

// The settings of the test signal generator of the stack "adsb" are the sdrkit_adsb_gen_*() functions
// after the settings of its receiver.

// The setting "correction" (Error correction) of the stack "adsb". A value of `sdrkit_adsb_correction`. Repairs extended squitters and all-call replies. Up to 3 bits finds more messages but lets more false ones through.
static inline int32_t sdrkit_adsb_get_correction(SDRKIT_HANDLE handle, sdrkit_adsb_correction *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "correction", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_adsb_correction)index;
  return code;
}
static inline int32_t sdrkit_adsb_set_correction(SDRKIT_HANDLE handle, sdrkit_adsb_correction value) {
  return sdrkit_typed_setting_set_enum(handle, "correction", (int32_t)value);
}

// The setting "repair_unknown" (Repair unknown addresses) of the stack "adsb". Also repair frames from aircraft not heard yet. Raises the chance of a false aircraft.
static inline int32_t sdrkit_adsb_get_repair_unknown(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "repair_unknown", value);
}
static inline int32_t sdrkit_adsb_set_repair_unknown(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "repair_unknown", value);
}

// The setting "expiry_s" (Expiry time) of the stack "adsb". Unit: s. How long an aircraft's address stays known after its last trusted message.
static inline int32_t sdrkit_adsb_get_expiry_s(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "expiry_s", value);
}
static inline int32_t sdrkit_adsb_set_expiry_s(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "expiry_s", value);
}

// The setting "effort" (Effort) of the stack "adsb". A value of `sdrkit_adsb_effort`. Extra decoding passes for weak signals. Automatic steps down when the computer is busy.
static inline int32_t sdrkit_adsb_get_effort(SDRKIT_HANDLE handle, sdrkit_adsb_effort *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "effort", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_adsb_effort)index;
  return code;
}
static inline int32_t sdrkit_adsb_set_effort(SDRKIT_HANDLE handle, sdrkit_adsb_effort value) {
  return sdrkit_typed_setting_set_enum(handle, "effort", (int32_t)value);
}

// The setting "modeac" (Mode A/C replies) of the stack "adsb". Also decode Mode A/C replies (squawk and Mode C altitude) on 1090 MHz.
static inline int32_t sdrkit_adsb_get_modeac(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "modeac", value);
}
static inline int32_t sdrkit_adsb_set_modeac(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "modeac", value);
}

// The setting "feeds_in" (Feeds from other receivers) of the stack "adsb". host:port of Beast, SBS or ASTERIX sources, separated by commas.
static inline int32_t sdrkit_adsb_get_feeds_in(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "feeds_in", value);
}
static inline int32_t sdrkit_adsb_set_feeds_in(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "feeds_in", value);
}

// The setting "allow_remote" (Allow hosts outside the local network) of the stack "adsb".
static inline int32_t sdrkit_adsb_get_allow_remote(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "allow_remote", value);
}
static inline int32_t sdrkit_adsb_set_allow_remote(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "allow_remote", value);
}

// The setting "beast_server" (Beast server) of the stack "adsb". Beast binary on TCP port 30005.
static inline int32_t sdrkit_adsb_get_beast_server(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "beast_server", value);
}
static inline int32_t sdrkit_adsb_set_beast_server(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "beast_server", value);
}

// The setting "beast_port" (Beast port) of the stack "adsb". 0 asks the system for a free port.
static inline int32_t sdrkit_adsb_get_beast_port(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "beast_port", value);
}
static inline int32_t sdrkit_adsb_set_beast_port(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "beast_port", value);
}

// The setting "sbs_server" (SBS server) of the stack "adsb". BaseStation text lines on TCP port 30003.
static inline int32_t sdrkit_adsb_get_sbs_server(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "sbs_server", value);
}
static inline int32_t sdrkit_adsb_set_sbs_server(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "sbs_server", value);
}

// The setting "sbs_port" (SBS port) of the stack "adsb". 0 asks the system for a free port.
static inline int32_t sdrkit_adsb_get_sbs_port(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "sbs_port", value);
}
static inline int32_t sdrkit_adsb_set_sbs_port(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "sbs_port", value);
}

// The setting "asterix_server" (ASTERIX server) of the stack "adsb". EUROCONTROL ASTERIX categories 021, 048 and 062 on TCP.
static inline int32_t sdrkit_adsb_get_asterix_server(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "asterix_server", value);
}
static inline int32_t sdrkit_adsb_set_asterix_server(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "asterix_server", value);
}

// The setting "asterix_port" (ASTERIX port) of the stack "adsb".
static inline int32_t sdrkit_adsb_get_asterix_port(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "asterix_port", value);
}
static inline int32_t sdrkit_adsb_set_asterix_port(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "asterix_port", value);
}

// The setting "asterix_cat" (ASTERIX categories) of the stack "adsb". A value of `sdrkit_adsb_asterix_cat`.
static inline int32_t sdrkit_adsb_get_asterix_cat(SDRKIT_HANDLE handle, sdrkit_adsb_asterix_cat *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "asterix_cat", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_adsb_asterix_cat)index;
  return code;
}
static inline int32_t sdrkit_adsb_set_asterix_cat(SDRKIT_HANDLE handle, sdrkit_adsb_asterix_cat value) {
  return sdrkit_typed_setting_set_enum(handle, "asterix_cat", (int32_t)value);
}

// The setting "sac" (ASTERIX SAC) of the stack "adsb".
static inline int32_t sdrkit_adsb_get_sac(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "sac", value);
}
static inline int32_t sdrkit_adsb_set_sac(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "sac", value);
}

// The setting "sic" (ASTERIX SIC) of the stack "adsb".
static inline int32_t sdrkit_adsb_get_sic(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "sic", value);
}
static inline int32_t sdrkit_adsb_set_sic(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "sic", value);
}

// The setting "allow_lan" (Servers accept the local network) of the stack "adsb". Otherwise only programs on this computer can connect.
static inline int32_t sdrkit_adsb_get_allow_lan(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "allow_lan", value);
}
static inline int32_t sdrkit_adsb_set_allow_lan(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "allow_lan", value);
}

// The setting "targets" (Outbound targets) of the stack "adsb". format@host:port, separated by ';': beast, sbs, asterix21, asterix48 or asterix62. Add /udp for UDP and #text for a first line.
static inline int32_t sdrkit_adsb_get_targets(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "targets", value);
}
static inline int32_t sdrkit_adsb_set_targets(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "targets", value);
}

// The setting "drop_s" (Drop time) of the stack "adsb". Unit: s. Aircraft leave the table after this silence.
static inline int32_t sdrkit_adsb_get_drop_s(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "drop_s", value);
}
static inline int32_t sdrkit_adsb_set_drop_s(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "drop_s", value);
}

// The setting "gps_layer" (GPS interference layer) of the stack "adsb".
static inline int32_t sdrkit_adsb_get_gps_layer(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "gps_layer", value);
}
static inline int32_t sdrkit_adsb_set_gps_layer(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "gps_layer", value);
}

// The setting "gen_aircraft" (Aircraft) of the test signal generator of the stack "adsb".
static inline int32_t sdrkit_adsb_gen_get_aircraft(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "gen_aircraft", value);
}
static inline int32_t sdrkit_adsb_gen_set_aircraft(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "gen_aircraft", value);
}

// The setting "gen_replies" (Radar replies) of the test signal generator of the stack "adsb".
static inline int32_t sdrkit_adsb_gen_get_replies(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "gen_replies", value);
}
static inline int32_t sdrkit_adsb_gen_set_replies(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "gen_replies", value);
}

// The setting "gen_emergency" (Emergency demonstration) of the test signal generator of the stack "adsb".
static inline int32_t sdrkit_adsb_gen_get_emergency(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "gen_emergency", value);
}
static inline int32_t sdrkit_adsb_gen_set_emergency(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "gen_emergency", value);
}

// The setting "gen_rate" (Message rate multiplier) of the test signal generator of the stack "adsb".
static inline int32_t sdrkit_adsb_gen_get_rate(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_rate", value);
}
static inline int32_t sdrkit_adsb_gen_set_rate(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_rate", value);
}

// The setting "gen_filter_mhz" (Pulse filter width) of the test signal generator of the stack "adsb". Unit: MHz. The filter that shapes the pulse edges. 0 for automatic (about seven eighths of the sample rate).
static inline int32_t sdrkit_adsb_gen_get_filter_mhz(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_filter_mhz", value);
}
static inline int32_t sdrkit_adsb_gen_set_filter_mhz(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_filter_mhz", value);
}

// The setting "gen_centre_lat" (Airspace centre latitude) of the test signal generator of the stack "adsb". Unit: °.
static inline int32_t sdrkit_adsb_gen_get_centre_lat(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_centre_lat", value);
}
static inline int32_t sdrkit_adsb_gen_set_centre_lat(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_centre_lat", value);
}

// The setting "gen_centre_lon" (Airspace centre longitude) of the test signal generator of the stack "adsb". Unit: °.
static inline int32_t sdrkit_adsb_gen_get_centre_lon(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_centre_lon", value);
}
static inline int32_t sdrkit_adsb_gen_set_centre_lon(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_centre_lon", value);
}

// Fills `rows` with the changed rows of the table "aircraft" of the stack "uat" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The column `callsign` names a row.
// Place of a row: the column `position`.
// Height: `alt_ft`. Course: `heading_deg`. Speed: `speed_kt`.
static inline int32_t sdrkit_uat_aircraft_rows(SDRKIT_HANDLE handle,
                                              sdrkit_uat_aircraft_row *rows,
                                              uint32_t capacity,
                                              sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "adsb.UatAircraftRow", "aircraft", rows, sizeof(sdrkit_uat_aircraft_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "fisb_products" of the stack "uat" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The column `name` names a row.
static inline int32_t sdrkit_uat_fisb_products_rows(SDRKIT_HANDLE handle,
                                                   sdrkit_uat_fisb_product_row *rows,
                                                   uint32_t capacity,
                                                   sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "adsb.FisbProductRow", "fisb_products", rows, sizeof(sdrkit_uat_fisb_product_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "fisb_records" of the stack "uat" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The column `name` names a row.
// Place of a row: the column `position`.
static inline int32_t sdrkit_uat_fisb_records_rows(SDRKIT_HANDLE handle,
                                                  sdrkit_uat_fisb_record_row *rows,
                                                  uint32_t capacity,
                                                  sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "adsb.FisbRecordRow", "fisb_records", rows, sizeof(sdrkit_uat_fisb_record_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "fisb_blocks" of the stack "uat" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The column `name` names a row.
// Place of a row: the column `corner`.
static inline int32_t sdrkit_uat_fisb_blocks_rows(SDRKIT_HANDLE handle,
                                                 sdrkit_uat_fisb_block_row *rows,
                                                 uint32_t capacity,
                                                 sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "adsb.FisbBlockRow", "fisb_blocks", rows, sizeof(sdrkit_uat_fisb_block_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "fisb_vertices" of the stack "uat" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// Place of a row: the column `position`.
// Height: `alt_ft`.
static inline int32_t sdrkit_uat_fisb_vertices_rows(SDRKIT_HANDLE handle,
                                                   sdrkit_uat_fisb_vertex_row *rows,
                                                   uint32_t capacity,
                                                   sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "adsb.FisbVertexRow", "fisb_vertices", rows, sizeof(sdrkit_uat_fisb_vertex_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "log" of the stack "uat" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The column `address` names a row.
static inline int32_t sdrkit_uat_log_rows(SDRKIT_HANDLE handle,
                                         sdrkit_uat_log_row *rows,
                                         uint32_t capacity,
                                         sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "adsb.UatLogRow", "log", rows, sizeof(sdrkit_uat_log_row), capacity, info);
}

// Fills `event` with event `n` of the last sdrkit_events_take() on `handle`, an event of the type "uat.message"
// (UAT message). Returns SDRKIT_E_WRONG_TYPE if the event is of another type.
static inline int32_t sdrkit_uat_message_event_read(SDRKIT_HANDLE handle, uint32_t n, sdrkit_uat_message_event *event) {
  return sdrkit_typed_event(handle, n, "uat.message", "adsb.UatMessage", event, sizeof(sdrkit_uat_message_event));
}

// Runs the command "clear" (Clear) of the stack "uat" on the session of `handle`. Forgets everything heard so far. Favourites stay.
static inline int32_t sdrkit_uat_clear(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "clear", NULL, NULL, NULL, NULL, 0);
}

// The settings of the test signal generator of the stack "uat" are the sdrkit_uat_gen_*() functions
// after the settings of its receiver.

// The setting "expiry_s" (Expiry time) of the stack "uat". Unit: s. How long an aircraft's address stays known after its last trusted message.
static inline int32_t sdrkit_uat_get_expiry_s(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "expiry_s", value);
}
static inline int32_t sdrkit_uat_set_expiry_s(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "expiry_s", value);
}

// The setting "gen_aircraft" (Aircraft) of the test signal generator of the stack "uat".
static inline int32_t sdrkit_uat_gen_get_aircraft(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "gen_aircraft", value);
}
static inline int32_t sdrkit_uat_gen_set_aircraft(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "gen_aircraft", value);
}

// The setting "gen_uplink" (Uplink frames) of the test signal generator of the stack "uat".
static inline int32_t sdrkit_uat_gen_get_uplink(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "gen_uplink", value);
}
static inline int32_t sdrkit_uat_gen_set_uplink(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "gen_uplink", value);
}

// The setting "gen_example" (Example frame only) of the test signal generator of the stack "uat". Sends the example frame of the standard in bursts instead of an airspace.
static inline int32_t sdrkit_uat_gen_get_example(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "gen_example", value);
}
static inline int32_t sdrkit_uat_gen_set_example(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "gen_example", value);
}

// The setting "gen_bursts" (Example bursts) of the test signal generator of the stack "uat".
static inline int32_t sdrkit_uat_gen_get_bursts(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "gen_bursts", value);
}
static inline int32_t sdrkit_uat_gen_set_bursts(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "gen_bursts", value);
}

// Fills `rows` with the changed rows of the table "modeac" of the stack "modeac" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_modeac_rows(SDRKIT_HANDLE handle,
                                        sdrkit_modeac_row *rows,
                                        uint32_t capacity,
                                        sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "adsb.ModeAcRow", "modeac", rows, sizeof(sdrkit_modeac_row), capacity, info);
}

// Fills `event` with event `n` of the last sdrkit_events_take() on `handle`, an event of the type "modeac.reply"
// (Mode A/C reply). Returns SDRKIT_E_WRONG_TYPE if the event is of another type.
static inline int32_t sdrkit_modeac_reply_event_read(SDRKIT_HANDLE handle, uint32_t n, sdrkit_modeac_reply_event *event) {
  return sdrkit_typed_event(handle, n, "modeac.reply", "adsb.ModeAcReplyEvent", event, sizeof(sdrkit_modeac_reply_event));
}

// Runs the command "clear" (Clear) of the stack "modeac" on the session of `handle`. Forgets everything heard so far. Favourites stay.
static inline int32_t sdrkit_modeac_clear(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "clear", NULL, NULL, NULL, NULL, 0);
}

// The settings of the test signal generator of the stack "modeac" are the sdrkit_modeac_gen_*() functions
// after the settings of its receiver.

// The setting "expiry_s" (Expiry time) of the stack "modeac". Unit: s. How long an aircraft's address stays known after its last trusted message.
static inline int32_t sdrkit_modeac_get_expiry_s(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "expiry_s", value);
}
static inline int32_t sdrkit_modeac_set_expiry_s(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "expiry_s", value);
}

// The setting "min_replies" (Replies before a code is shown) of the stack "modeac".
static inline int32_t sdrkit_modeac_get_min_replies(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "min_replies", value);
}
static inline int32_t sdrkit_modeac_set_min_replies(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "min_replies", value);
}

// The setting "gen_targets" (Transponders) of the test signal generator of the stack "modeac".
static inline int32_t sdrkit_modeac_gen_get_targets(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "gen_targets", value);
}
static inline int32_t sdrkit_modeac_gen_set_targets(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "gen_targets", value);
}

// Fills `rows` with the changed rows of the table "addressed" of the stack "uplink1030" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The column `icao` names a row.
static inline int32_t sdrkit_uplink1030_addressed_rows(SDRKIT_HANDLE handle,
                                                      sdrkit_uplink1030_addressed_row *rows,
                                                      uint32_t capacity,
                                                      sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "adsb.AddressedRow", "addressed", rows, sizeof(sdrkit_uplink1030_addressed_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "interrogators" of the stack "uplink1030" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_uplink1030_interrogators_rows(SDRKIT_HANDLE handle,
                                                          sdrkit_uplink1030_interrogator_row *rows,
                                                          uint32_t capacity,
                                                          sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "adsb.InterrogatorRow", "interrogators", rows, sizeof(sdrkit_uplink1030_interrogator_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "by_uf" of the stack "uplink1030" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_uplink1030_by_uf_rows(SDRKIT_HANDLE handle,
                                                  sdrkit_adsb_count_row *rows,
                                                  uint32_t capacity,
                                                  sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "adsb.CountRow", "by_uf", rows, sizeof(sdrkit_adsb_count_row), capacity, info);
}

// Fills `event` with event `n` of the last sdrkit_events_take() on `handle`, an event of the type "uplink.interrogation"
// (Interrogation). Returns SDRKIT_E_WRONG_TYPE if the event is of another type.
static inline int32_t sdrkit_uplink1030_uplink_interrogation_event_read(SDRKIT_HANDLE handle, uint32_t n, sdrkit_uplink1030_uplink_interrogation_event *event) {
  return sdrkit_typed_event(handle, n, "uplink.interrogation", "adsb.InterrogationEvent", event, sizeof(sdrkit_uplink1030_uplink_interrogation_event));
}

// Runs the command "clear" (Clear) of the stack "uplink1030" on the session of `handle`. Forgets everything heard so far. Favourites stay.
static inline int32_t sdrkit_uplink1030_clear(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "clear", NULL, NULL, NULL, NULL, 0);
}

// The setting "expiry_s" (Expiry time) of the stack "uplink1030". Unit: s. How long an aircraft's address stays known after its last trusted message.
static inline int32_t sdrkit_uplink1030_get_expiry_s(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "expiry_s", value);
}
static inline int32_t sdrkit_uplink1030_set_expiry_s(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "expiry_s", value);
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_adsb_tracker_fields_read(SDRKIT_HANDLE handle, sdrkit_adsb_tracker_fields *fields) {
  return sdrkit_typed_fields(handle, "adsb.TrackerFields", fields, sizeof(sdrkit_adsb_tracker_fields));
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_adsb_feed_fields_read(SDRKIT_HANDLE handle, sdrkit_adsb_feed_fields *fields) {
  return sdrkit_typed_fields(handle, "adsb.FeedFields", fields, sizeof(sdrkit_adsb_feed_fields));
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_modeac_fields_read(SDRKIT_HANDLE handle, sdrkit_modeac_fields *fields) {
  return sdrkit_typed_fields(handle, "adsb.ModeAcFields", fields, sizeof(sdrkit_modeac_fields));
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_uat_fields_read(SDRKIT_HANDLE handle, sdrkit_uat_fields *fields) {
  return sdrkit_typed_fields(handle, "adsb.UatFields", fields, sizeof(sdrkit_uat_fields));
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_uplink1030_uplink_fields_read(SDRKIT_HANDLE handle, sdrkit_uplink1030_uplink_fields *fields) {
  return sdrkit_typed_fields(handle, "adsb.UplinkFields", fields, sizeof(sdrkit_uplink1030_uplink_fields));
}

#endif
