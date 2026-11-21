/*
 * sdrkit: the typed interface of the stack family "gnss", version 1.0. MIT licence.
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

#ifndef SDRKIT_GNSS_H
#define SDRKIT_GNSS_H

#include "../sdrkit.h"

// The version of the family that this header describes.
#define SDRKIT_GNSS_VERSION_MAJOR 1
#define SDRKIT_GNSS_VERSION_MINOR 0

// The stack "gnss": its id, and the keys of its event types.
#define SDRKIT_STACK_GNSS "gnss"
#define SDRKIT_EVENT_GNSS_FIX "gnss.fix"
#define SDRKIT_EVENT_TEXT_NMEA "text.nmea"

// The stack "sbas": its id, and the keys of its event types.
#define SDRKIT_STACK_SBAS "sbas"

// The stack "navic": its id, and the keys of its event types.
#define SDRKIT_STACK_NAVIC "navic"
#define SDRKIT_EVENT_NAVIC_SUBFRAME "navic.subframe"

// The options of the setting "tune" of the stack "gnss".
typedef enum sdrkit_gnss_tune {
  // L1 / E1 1575.42 MHz
  SDRKIT_GNSS_TUNE_L1 = 0,
  // BeiDou B1I 1561.098 MHz
  SDRKIT_GNSS_TUNE_B1I = 1,
  // GLONASS L1 1602 MHz
  SDRKIT_GNSS_TUNE_GLO = 2,
  // All systems 1582 MHz
  SDRKIT_GNSS_TUNE_ALL = 3,
  // Custom
  SDRKIT_GNSS_TUNE_CUSTOM = 4,
} sdrkit_gnss_tune;

// The options of `search_system` of `sdrkit_gnss_fields`.
typedef enum sdrkit_gnss_fields_search_system {
  // GPS
  SDRKIT_GNSS_FIELDS_SEARCH_SYSTEM_GPS = 0,
  // QZSS
  SDRKIT_GNSS_FIELDS_SEARCH_SYSTEM_QZSS = 1,
  // SBAS
  SDRKIT_GNSS_FIELDS_SEARCH_SYSTEM_SBAS = 2,
  // Galileo
  SDRKIT_GNSS_FIELDS_SEARCH_SYSTEM_GALILEO = 3,
  // BeiDou
  SDRKIT_GNSS_FIELDS_SEARCH_SYSTEM_BEIDOU = 4,
  // GLONASS
  SDRKIT_GNSS_FIELDS_SEARCH_SYSTEM_GLONASS = 5,
} sdrkit_gnss_fields_search_system;

// The options of `frequency_source` of `sdrkit_gnss_fields`.
typedef enum sdrkit_gnss_fields_frequency_source {
  // Measured by the fix
  SDRKIT_GNSS_FIELDS_FREQUENCY_SOURCE_MEASURED = 0,
  // About the search centre
  SDRKIT_GNSS_FIELDS_FREQUENCY_SOURCE_SEARCH_CENTRE = 1,
  // Not known yet
  SDRKIT_GNSS_FIELDS_FREQUENCY_SOURCE_UNKNOWN = 2,
} sdrkit_gnss_fields_frequency_source;

// The result fields of the receiver.
typedef struct sdrkit_gnss_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The centre frequency of the input. A frequency in Hz.
  double centre_hz;
  // The sample rate of the input. A frequency in Hz.
  double input_rate;
  // True if `search_system` holds a value.
  bool has_search_system;
  // The system that the search works on now. A value of `sdrkit_gnss_fields_search_system`.
  int32_t search_system;
  // True if `search_prn` holds a value.
  bool has_search_prn;
  // The PRN that the search works on now.
  int64_t search_prn;
  // True if `search_progress_pct` holds a value.
  bool has_search_progress_pct;
  // The part of the search round done. Unit: %.
  double search_progress_pct;
  // True if `search_round` holds a value.
  bool has_search_round;
  // The number of the search round, from 1.
  int64_t search_round;
  // True if `search_centre` holds a value.
  bool has_search_centre;
  // The centre of the search window (the radio's frequency error at L1). A frequency in Hz.
  double search_centre;
  // True if `search_half_width` holds a value.
  bool has_search_half_width;
  // The half width of the search window. A frequency in Hz.
  double search_half_width;
  // True if `search_integration_ms` holds a value.
  bool has_search_integration_ms;
  // The integration of the search. Unit: ms.
  double search_integration_ms;
  // True if `frequency_error` holds a value.
  bool has_frequency_error;
  // The radio's frequency error at L1, measured by the fix. A frequency in Hz.
  double frequency_error;
  // True if `frequency_error_ppm` holds a value.
  bool has_frequency_error_ppm;
  // The same error in parts per million of 1575.42 MHz. Unit: ppm.
  double frequency_error_ppm;
  // Where the frequency error comes from. A value of `sdrkit_gnss_fields_frequency_source`.
  int32_t frequency_source;
  // True if `first_lock` holds a value.
  bool has_first_lock;
  // Signal time of the first lock. A length of time in seconds.
  double first_lock;
  // Frames (packets, blocks, bursts) that passed their check.
  int64_t frames_ok;
  // Frames that failed their check.
  int64_t frames_bad;
  // SBAS messages with a good CRC.
  int64_t sbas_messages;
  // True if `glonass_tau_gps_s` holds a value.
  bool has_glonass_tau_gps_s;
  // GLONASS time minus GPS time. Unit: s.
  double glonass_tau_gps_s;
  // True if `glonass_tau_c_s` holds a value.
  bool has_glonass_tau_c_s;
  // GLONASS time minus UTC(SU). Unit: s.
  double glonass_tau_c_s;
  // True if `bdt_gps_s` holds a value.
  bool has_bdt_gps_s;
  // BeiDou time minus GPS time, less 14 s. Unit: s.
  double bdt_gps_s;
  // GPS satellites with an almanac entry.
  int64_t almanac_satellites;
  // Whether the ionosphere model comes from the satellites.
  bool iono_broadcast;
  // True if `leap_seconds` holds a value.
  bool has_leap_seconds;
  // GPS time minus UTC: broadcast, else from the station's leap-second table. Unit: s.
  int64_t leap_seconds;
  // Whether the satellites broadcast the leap seconds.
  bool leap_seconds_broadcast;
  // Level of the signal or channel, dBFS (a mean power of 1 is 0 dBFS). Unit: dBFS.
  double level_dbfs;
  // Input samples clipped. Unit: %.
  double clipped_pct;
  // Input DC, in phase (full scale 1).
  double dc_i;
  // Input DC, quadrature (full scale 1).
  double dc_q;
  // Signal time fed. A length of time in seconds.
  double signal;
  // What the receiver waits for while it has no fix, for people, one item a line.
  sdrkit_string progress;
  // True if `position` holds a value.
  bool has_position;
  // The position of the object that the event is about.
  sdrkit_position position;
  // True if `height_m` holds a value.
  bool has_height_m;
  // The height of the object that the event is about, metres above mean sea level. Unit: m.
  double height_m;
  // True if `error_h_m` holds a value.
  bool has_error_h_m;
  // Horizontal error, 1 sigma. Unit: m.
  double error_h_m;
  // True if `error_v_m` holds a value.
  bool has_error_v_m;
  // Vertical error, 1 sigma. Unit: m.
  double error_v_m;
  // True if `hdop` holds a value.
  bool has_hdop;
  // Horizontal dilution of precision.
  double hdop;
  // True if `vdop` holds a value.
  bool has_vdop;
  // Vertical dilution of precision.
  double vdop;
  // True if `pdop` holds a value.
  bool has_pdop;
  // Position dilution of precision.
  double pdop;
  // True if `satellites_used` holds a value.
  bool has_satellites_used;
  // Satellites in the fix.
  int64_t satellites_used;
  // Satellites tracked.
  int64_t satellites_tracked;
  // True if `utc` holds a value.
  bool has_utc;
  // UTC of the fix. Seconds since 1970 (UTC).
  double utc;
  // True if `gps_week` holds a value.
  bool has_gps_week;
  // GPS week of the fix.
  int64_t gps_week;
  // True if `gps_tow_s` holds a value.
  bool has_gps_tow_s;
  // GPS seconds of the week of the fix. Unit: s.
  double gps_tow_s;
  // True if `speed_kt` holds a value.
  bool has_speed_kt;
  // The speed over the ground of the object that the event is about, knots. Unit: kt.
  double speed_kt;
  // True if `course_deg` holds a value.
  bool has_course_deg;
  // The course over the ground of the object that the event is about, degrees clockwise from true north. Unit: °.
  double course_deg;
  // True if `clock_bias_m` holds a value.
  bool has_clock_bias_m;
  // The receiver's clock bias. Unit: m.
  double clock_bias_m;
  // True if `first_fix` holds a value.
  bool has_first_fix;
  // Signal time of the first fix. A length of time in seconds.
  double first_fix;
} sdrkit_gnss_fields;

// The options of `interference` of `sdrkit_gnss_interference_fields`.
typedef enum sdrkit_gnss_interference_fields_interference {
  // Clear
  SDRKIT_GNSS_INTERFERENCE_FIELDS_INTERFERENCE_CLEAR = 0,
  // Raised noise
  SDRKIT_GNSS_INTERFERENCE_FIELDS_INTERFERENCE_RAISED_NOISE = 1,
  // Interference
  SDRKIT_GNSS_INTERFERENCE_FIELDS_INTERFERENCE_INTERFERENCE = 2,
} sdrkit_gnss_interference_fields_interference;

// The result fields of the interference monitor.
typedef struct sdrkit_gnss_interference_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The monitor's verdict. A value of `sdrkit_gnss_interference_fields_interference`.
  int32_t interference;
  // A tone that sweeps through the band.
  bool swept;
  // Pulses.
  bool pulsed;
  // A narrow line that the receiver does not know.
  bool narrowband;
  // A raised noise floor.
  bool wideband;
  // Why the monitor says so, for people.
  sdrkit_string interference_reason;
  // The noise floor, dBFS in 1 Hz. Unit: dBFS/Hz.
  double noise_floor_dbfs;
  // The band power. Unit: dBFS.
  double band_power_dbfs;
  // Spectral flatness, 0 to 1.
  double flatness;
  // Whether spoofing is suspected.
  bool spoofing;
  // True if `spoofing_reason` holds a value.
  bool has_spoofing_reason;
  // Why spoofing is suspected, for people.
  sdrkit_string spoofing_reason;
} sdrkit_gnss_interference_fields;

// The result set that a [`StackGraph`] writes itself. The description of a stack that shows it lists it with [`StackShape::receiver_fields`], at its place among the result sets.
typedef struct sdrkit_gnss_guard_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Input samples replaced because they were not finite or far beyond full scale.
  int64_t bad_samples;
} sdrkit_gnss_guard_fields;

// The options of `system` of `sdrkit_gnss_system_row`.
typedef enum sdrkit_gnss_system_row_system {
  // GPS
  SDRKIT_GNSS_SYSTEM_ROW_SYSTEM_GPS = 0,
  // QZSS
  SDRKIT_GNSS_SYSTEM_ROW_SYSTEM_QZSS = 1,
  // SBAS
  SDRKIT_GNSS_SYSTEM_ROW_SYSTEM_SBAS = 2,
  // Galileo
  SDRKIT_GNSS_SYSTEM_ROW_SYSTEM_GALILEO = 3,
  // BeiDou
  SDRKIT_GNSS_SYSTEM_ROW_SYSTEM_BEIDOU = 4,
  // GLONASS
  SDRKIT_GNSS_SYSTEM_ROW_SYSTEM_GLONASS = 5,
} sdrkit_gnss_system_row_system;

// A satellite system. The row key is the key of the system.
typedef struct sdrkit_gnss_system_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // A satellite system. A value of `sdrkit_gnss_system_row_system`.
  int32_t system;
  // This system is selected for reception.
  bool wanted;
  // The receiver has a band for this system at the present tune.
  bool active;
  // The receiver has searched or tracks this system.
  bool searched;
} sdrkit_gnss_system_row;

// The options of `system` of `sdrkit_gnss_fix_system_row`.
typedef enum sdrkit_gnss_fix_system_row_system {
  // GPS
  SDRKIT_GNSS_FIX_SYSTEM_ROW_SYSTEM_GPS = 0,
  // QZSS
  SDRKIT_GNSS_FIX_SYSTEM_ROW_SYSTEM_QZSS = 1,
  // SBAS
  SDRKIT_GNSS_FIX_SYSTEM_ROW_SYSTEM_SBAS = 2,
  // Galileo
  SDRKIT_GNSS_FIX_SYSTEM_ROW_SYSTEM_GALILEO = 3,
  // BeiDou
  SDRKIT_GNSS_FIX_SYSTEM_ROW_SYSTEM_BEIDOU = 4,
  // GLONASS
  SDRKIT_GNSS_FIX_SYSTEM_ROW_SYSTEM_GLONASS = 5,
} sdrkit_gnss_fix_system_row_system;

// A satellite system in the fix. The row key is the key of the system.
typedef struct sdrkit_gnss_fix_system_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // A satellite system. A value of `sdrkit_gnss_fix_system_row_system`.
  int32_t system;
  // Satellites of a system in the fix.
  int64_t satellites;
  // The receiver clock of a system in the fix. Unit: m.
  double clock_m;
} sdrkit_gnss_fix_system_row;

// The options of `system` of `sdrkit_gnss_satellite_row`.
typedef enum sdrkit_gnss_satellite_row_system {
  // GPS
  SDRKIT_GNSS_SATELLITE_ROW_SYSTEM_GPS = 0,
  // QZSS
  SDRKIT_GNSS_SATELLITE_ROW_SYSTEM_QZSS = 1,
  // SBAS
  SDRKIT_GNSS_SATELLITE_ROW_SYSTEM_SBAS = 2,
  // Galileo
  SDRKIT_GNSS_SATELLITE_ROW_SYSTEM_GALILEO = 3,
  // BeiDou
  SDRKIT_GNSS_SATELLITE_ROW_SYSTEM_BEIDOU = 4,
  // GLONASS
  SDRKIT_GNSS_SATELLITE_ROW_SYSTEM_GLONASS = 5,
} sdrkit_gnss_satellite_row_system;

// The options of `state` of `sdrkit_gnss_satellite_row`.
typedef enum sdrkit_gnss_satellite_row_state {
  // Pull-in
  SDRKIT_GNSS_SATELLITE_ROW_STATE_PULL_IN = 0,
  // Locked
  SDRKIT_GNSS_SATELLITE_ROW_STATE_LOCKED = 1,
  // Bit sync
  SDRKIT_GNSS_SATELLITE_ROW_STATE_BIT_SYNC = 2,
  // Frame sync
  SDRKIT_GNSS_SATELLITE_ROW_STATE_FRAME_SYNC = 3,
  // Ephemeris
  SDRKIT_GNSS_SATELLITE_ROW_STATE_EPHEMERIS = 4,
} sdrkit_gnss_satellite_row_state;

// The options of `ephemeris` of `sdrkit_gnss_satellite_row`.
typedef enum sdrkit_gnss_satellite_row_ephemeris {
  // Held
  SDRKIT_GNSS_SATELLITE_ROW_EPHEMERIS_HELD = 0,
  // Collecting
  SDRKIT_GNSS_SATELLITE_ROW_EPHEMERIS_COLLECTING = 1,
  // Waiting for the frame
  SDRKIT_GNSS_SATELLITE_ROW_EPHEMERIS_WAITING = 2,
  // Not used
  SDRKIT_GNSS_SATELLITE_ROW_EPHEMERIS_NOT_USED = 3,
} sdrkit_gnss_satellite_row_ephemeris;

// The options of `orbit` of `sdrkit_gnss_satellite_row`.
typedef enum sdrkit_gnss_satellite_row_orbit {
  // Broadcast ephemeris
  SDRKIT_GNSS_SATELLITE_ROW_ORBIT_EPHEMERIS = 0,
  // Almanac
  SDRKIT_GNSS_SATELLITE_ROW_ORBIT_ALMANAC = 1,
} sdrkit_gnss_satellite_row_orbit;

// A satellite: what the channels, the navigation data, the sky and the orbits say of it.
typedef struct sdrkit_gnss_satellite_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // True if `system` holds a value.
  bool has_system;
  // A satellite system. A value of `sdrkit_gnss_satellite_row_system`.
  int32_t system;
  // True if `prn` holds a value.
  bool has_prn;
  // A PRN or slot number.
  int64_t prn;
  // True if `glo_channel` holds a value.
  bool has_glo_channel;
  // The frequency channel of a GLONASS satellite.
  int64_t glo_channel;
  // A channel tracks the satellite.
  bool tracked;
  // True if `state` holds a value.
  bool has_state;
  // The state of the satellite's channel. A value of `sdrkit_gnss_satellite_row_state`.
  int32_t state;
  // True if `cn0_dbhz` holds a value.
  bool has_cn0_dbhz;
  // Carrier to noise density. Unit: dB-Hz.
  double cn0_dbhz;
  // True if `doppler` holds a value.
  bool has_doppler;
  // Doppler at the signal's carrier, with the radio's error. A frequency in Hz.
  double doppler;
  // True if `code_chips` holds a value.
  bool has_code_chips;
  // Code phase. Unit: chips.
  double code_chips;
  // True if `azimuth_deg` holds a value.
  bool has_azimuth_deg;
  // Azimuth, degrees from true north. Unit: °.
  double azimuth_deg;
  // True if `elevation_deg` holds a value.
  bool has_elevation_deg;
  // Elevation. Unit: °.
  double elevation_deg;
  // True if `locked_for` holds a value.
  bool has_locked_for;
  // Time since lock was gained, seconds. A length of time in seconds.
  double locked_for;
  // True if `frames_good` holds a value.
  bool has_frames_good;
  // Frames that passed their check.
  int64_t frames_good;
  // True if `frames` holds a value.
  bool has_frames;
  // Frames received.
  int64_t frames;
  // True if `residual_m` holds a value.
  bool has_residual_m;
  // The residual of the satellite in the fix. Unit: m.
  double residual_m;
  // True if `healthy` holds a value.
  bool has_healthy;
  // The satellite says it is healthy.
  bool healthy;
  // The satellite is in the fix.
  bool used;
  // True if `ephemeris` holds a value.
  bool has_ephemeris;
  // What the receiver holds of the ephemeris. A value of `sdrkit_gnss_satellite_row_ephemeris`.
  int32_t ephemeris;
  // True if `parts` holds a value.
  bool has_parts;
  // Ephemeris parts collected.
  int64_t parts;
  // True if `parts_of` holds a value.
  bool has_parts_of;
  // Ephemeris parts needed.
  int64_t parts_of;
  // True if `seconds_left` holds a value.
  bool has_seconds_left;
  // About how long until the ephemeris is complete. A length of time in seconds.
  double seconds_left;
  // True if `iod` holds a value.
  bool has_iod;
  // The issue of data (IODE, IODnav, AODE or the GLONASS tb index).
  int64_t iod;
  // True if `data_age` holds a value.
  bool has_data_age;
  // The age of the data from its epoch (negative: ahead). A length of time in seconds.
  double data_age;
  // True if `week` holds a value.
  bool has_week;
  // The week of the data.
  int64_t week;
  // True if `tow_s` holds a value.
  bool has_tow_s;
  // The time of week of the last unit. Unit: s.
  double tow_s;
  // True if `af0_s` holds a value.
  bool has_af0_s;
  // The satellite clock af0. Unit: s.
  double af0_s;
  // True if `sub_point` holds a value.
  bool has_sub_point;
  // The point below the satellite. Unit: °.
  sdrkit_position sub_point;
  // True if `height_km` holds a value.
  bool has_height_km;
  // The height of the satellite. Unit: km.
  double height_km;
  // True if `orbit` holds a value.
  bool has_orbit;
  // Where the satellite's position comes from. A value of `sdrkit_gnss_satellite_row_orbit`.
  int32_t orbit;
} sdrkit_gnss_satellite_row;

// A line of the interference spectrum.
typedef struct sdrkit_gnss_interference_line_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The offset of a line from the centre. A frequency in Hz.
  double offset;
  // A level above the floor. Unit: dB.
  double level_db;
  // A line that the receiver has identified (a satellite or the radio's own).
  bool known;
} sdrkit_gnss_interference_line_row;

// The options of `level` of `sdrkit_gnss_interference_event_row`.
typedef enum sdrkit_gnss_interference_event_row_level {
  // Info
  SDRKIT_GNSS_INTERFERENCE_EVENT_ROW_LEVEL_INFO = 0,
  // Warning
  SDRKIT_GNSS_INTERFERENCE_EVENT_ROW_LEVEL_WARNING = 1,
  // Alarm
  SDRKIT_GNSS_INTERFERENCE_EVENT_ROW_LEVEL_ALARM = 2,
} sdrkit_gnss_interference_event_row_level;

// An interference event.
typedef struct sdrkit_gnss_interference_event_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // Signal time of an event. A length of time in seconds.
  double at;
  // The level of an event. A value of `sdrkit_gnss_interference_event_row_level`.
  int32_t level;
  // The text of a decoded message.
  sdrkit_string text;
} sdrkit_gnss_interference_event_row;

// The result fields of the SBAS content decoder.
typedef struct sdrkit_sbas_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // A PRN mask and fast corrections are held.
  bool data_valid;
  // The SBAS messages with a good CRC.
  int64_t sbas_messages;
  // The messages missed.
  int64_t messages_missed;
  // True if `help` holds a value.
  bool has_help;
  // Help for people while nothing is heard.
  sdrkit_string help;
  // True if `selected` holds a value.
  bool has_selected;
  // The PRN whose corrections the tables show.
  int64_t selected;
  // True if `geo_iodn` holds a value.
  bool has_geo_iodn;
  // GEO navigation of the selected PRN: the issue of data.
  int64_t geo_iodn;
  // True if `geo_t0_s` holds a value.
  bool has_geo_t0_s;
  // GEO navigation: the time of applicability. Unit: s.
  double geo_t0_s;
  // True if `geo_ura` holds a value.
  bool has_geo_ura;
  // GEO navigation: the accuracy index.
  int64_t geo_ura;
  // True if `geo_radius_m` holds a value.
  bool has_geo_radius_m;
  // GEO navigation: the distance from the centre of the earth. Unit: m.
  double geo_radius_m;
  // True if `geo_longitude_deg` holds a value.
  bool has_geo_longitude_deg;
  // GEO navigation: the longitude. Unit: °.
  double geo_longitude_deg;
  // True if `sbas_week` holds a value.
  bool has_sbas_week;
  // Network time of the selected PRN: the week.
  int64_t sbas_week;
  // True if `sbas_tow_s` holds a value.
  bool has_sbas_tow_s;
  // Network time: the time of week. Unit: s.
  int64_t sbas_tow_s;
  // True if `sbas_leap_s` holds a value.
  bool has_sbas_leap_s;
  // Network time: the leap seconds. Unit: s.
  int64_t sbas_leap_s;
  // True if `sbas_leap_next_s` holds a value.
  bool has_sbas_leap_next_s;
  // Network time: the leap seconds after the next change. Unit: s.
  int64_t sbas_leap_next_s;
  // True if `utc_a0_s` holds a value.
  bool has_utc_a0_s;
  // Network time: the UTC offset A0. Unit: s.
  double utc_a0_s;
  // True if `utc_a1` holds a value.
  bool has_utc_a1;
  // Network time: the UTC drift A1. Unit: s/s.
  double utc_a1;
  // True if `utc_id` holds a value.
  bool has_utc_id;
  // Network time: the identifier of the UTC standard.
  int64_t utc_id;
} sdrkit_sbas_fields;

// The options of `service` of `sdrkit_sbas_satellite_row`.
typedef enum sdrkit_sbas_satellite_row_service {
  // EGNOS
  SDRKIT_SBAS_SATELLITE_ROW_SERVICE_EGNOS = 0,
  // WAAS
  SDRKIT_SBAS_SATELLITE_ROW_SERVICE_WAAS = 1,
  // MSAS
  SDRKIT_SBAS_SATELLITE_ROW_SERVICE_MSAS = 2,
  // GAGAN
  SDRKIT_SBAS_SATELLITE_ROW_SERVICE_GAGAN = 3,
  // SDCM
  SDRKIT_SBAS_SATELLITE_ROW_SERVICE_SDCM = 4,
  // BDSBAS
  SDRKIT_SBAS_SATELLITE_ROW_SERVICE_BDSBAS = 5,
  // KASS
  SDRKIT_SBAS_SATELLITE_ROW_SERVICE_KASS = 6,
} sdrkit_sbas_satellite_row_service;

// The options of `state` of `sdrkit_sbas_satellite_row`.
typedef enum sdrkit_sbas_satellite_row_state {
  // Pull-in
  SDRKIT_SBAS_SATELLITE_ROW_STATE_PULL_IN = 0,
  // Locked
  SDRKIT_SBAS_SATELLITE_ROW_STATE_LOCKED = 1,
  // Bit sync
  SDRKIT_SBAS_SATELLITE_ROW_STATE_BIT_SYNC = 2,
  // Frame sync
  SDRKIT_SBAS_SATELLITE_ROW_STATE_FRAME_SYNC = 3,
  // Ephemeris
  SDRKIT_SBAS_SATELLITE_ROW_STATE_EPHEMERIS = 4,
} sdrkit_sbas_satellite_row_state;

// A row of the SBAS satellite table. The row key is the PRN.
typedef struct sdrkit_sbas_satellite_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The PRN.
  int64_t prn;
  // True if `service` holds a value.
  bool has_service;
  // The service of the PRN. A value of `sdrkit_sbas_satellite_row_service`.
  int32_t service;
  // True if `state` holds a value.
  bool has_state;
  // The state of the channel of the satellite. A value of `sdrkit_sbas_satellite_row_state`.
  int32_t state;
  // The carrier to noise density. Unit: dB-Hz.
  double cn0_dbhz;
  // The Doppler at the carrier, with the error of the radio. A frequency in Hz.
  double doppler;
  // The messages received.
  int64_t messages;
  // The messages missed.
  int64_t messages_missed;
} sdrkit_sbas_satellite_row;

// A message type that an SBAS satellite sent. The row key is the PRN and the type, as `123/9`.
typedef struct sdrkit_sbas_satellite_type_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The SBAS satellite.
  int64_t prn;
  // The message type.
  int64_t message_type;
} sdrkit_sbas_satellite_type_row;

// The count of one message type. The row key is the type.
typedef struct sdrkit_sbas_type_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The message type.
  int64_t message_type;
  // True if `name` holds a value.
  bool has_name;
  // The name of the type.
  sdrkit_string name;
  // The messages of the type.
  int64_t count;
} sdrkit_sbas_type_row;

// A row of the message log. The row key is the number of the message since the start.
typedef struct sdrkit_sbas_message_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The signal time of the message. A length of time in seconds.
  double at;
  // The SBAS satellite.
  int64_t prn;
  // The message type.
  int64_t message_type;
  // The content in words.
  sdrkit_string text;
} sdrkit_sbas_message_row;

// The options of `system` of `sdrkit_sbas_fast_correction_row`.
typedef enum sdrkit_sbas_fast_correction_row_system {
  // GPS
  SDRKIT_SBAS_FAST_CORRECTION_ROW_SYSTEM_GPS = 0,
  // QZSS
  SDRKIT_SBAS_FAST_CORRECTION_ROW_SYSTEM_QZSS = 1,
  // SBAS
  SDRKIT_SBAS_FAST_CORRECTION_ROW_SYSTEM_SBAS = 2,
  // Galileo
  SDRKIT_SBAS_FAST_CORRECTION_ROW_SYSTEM_GALILEO = 3,
  // BeiDou
  SDRKIT_SBAS_FAST_CORRECTION_ROW_SYSTEM_BEIDOU = 4,
  // GLONASS
  SDRKIT_SBAS_FAST_CORRECTION_ROW_SYSTEM_GLONASS = 5,
} sdrkit_sbas_fast_correction_row_system;

// A fast correction of the selected PRN. The row key is the slot.
typedef struct sdrkit_sbas_fast_correction_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The slot of the PRN mask.
  int64_t slot;
  // True if `system` holds a value.
  bool has_system;
  // The system of the corrected satellite. A value of `sdrkit_sbas_fast_correction_row_system`.
  int32_t system;
  // True if `prn` holds a value.
  bool has_prn;
  // The PRN of the corrected satellite.
  int64_t prn;
  // True if `correction_m` holds a value.
  bool has_correction_m;
  // The fast correction. Unit: m.
  double correction_m;
  // The UDRE indicator.
  int64_t udrei;
  // True if `udre_sigma_m` holds a value.
  bool has_udre_sigma_m;
  // The UDRE as one sigma. No value for "not monitored" and "do not use". Unit: m.
  double udre_sigma_m;
  // The issue of data of the fast corrections.
  int64_t iodf;
  // The issue of data of the PRN mask.
  int64_t iodp;
  // The correction belongs to the PRN mask held.
  bool iodp_current;
  // True if `degradation` holds a value.
  bool has_degradation;
  // The degradation factor. Unit: m/s².
  double degradation;
} sdrkit_sbas_fast_correction_row;

// An ionosphere band of the selected PRN. The row key is the band.
typedef struct sdrkit_sbas_ionosphere_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The band.
  int64_t band;
  // The issue of data of the ionosphere.
  int64_t iodi;
  // The grid points.
  int64_t points;
  // The blocks seen.
  int64_t blocks_seen;
  // The blocks needed.
  int64_t blocks_needed;
  // The delays held.
  int64_t values;
  // The points not monitored.
  int64_t not_monitored;
  // The smallest vertical delay. Unit: m.
  double min_m;
  // The mean vertical delay. Unit: m.
  double mean_m;
  // The largest vertical delay. Unit: m.
  double max_m;
  // The worst GIVE indicator.
  int64_t worst_givei;
} sdrkit_sbas_ionosphere_row;

// The result fields of NavIC.
typedef struct sdrkit_navic_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // 1 carrier lock, 2 bit sync, 3 subframes decoded (0 pulling in); for the stack 0 searching, 1 tracking, 2 subframes.
  int64_t state;
  // True if `snr_db` holds a value.
  bool has_snr_db;
  // Signal to noise ratio of what is being decoded, dB (in the signal's own bandwidth, or per symbol where the stack says so). Unit: dB.
  double snr_db;
  // True if `offset_hz` holds a value.
  bool has_offset_hz;
  // Carrier offset from where it was expected, Hz. A frequency in Hz.
  double offset_hz;
  // Frames (packets, blocks, bursts) that passed their check.
  int64_t frames_ok;
  // Frames that failed their check.
  int64_t frames_bad;
  // The PRN being searched.
  int64_t searching_prn;
  // What is not confirmed by a public document.
  sdrkit_string note;
} sdrkit_navic_fields;

// The result set that a [`DecoderComponent`] adds to the results of its decoder. The description of a stack with a decoder component names it after the result sets of the decoder.
typedef struct sdrkit_navic_decoder_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // True if `dropped` holds a value.
  bool has_dropped;
  // Items dropped because the decoder was slow. It holds a value while the decoder runs on a helper thread.
  int64_t dropped;
} sdrkit_navic_decoder_fields;

// A NavIC satellite.
typedef struct sdrkit_navic_satellite_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The PRN.
  int64_t prn;
  // 1 carrier lock, 2 bit sync, 3 subframes decoded (0 pulling in); for the stack 0 searching, 1 tracking, 2 subframes.
  int64_t state;
  // C/N0. Unit: dB-Hz.
  double cn0_dbhz;
  // Doppler. A frequency in Hz.
  double doppler_hz;
  // Subframes good.
  int64_t subframes_ok;
  // Subframes bad.
  int64_t subframes_bad;
  // True if `subframe_id` holds a value.
  bool has_subframe_id;
  // The last subframe ID.
  int64_t subframe_id;
  // True if `towc` holds a value.
  bool has_towc;
  // The time of week count.
  int64_t towc;
  // True if `wn` holds a value.
  bool has_wn;
  // The week number.
  int64_t wn;
  // True if `alert` holds a value.
  bool has_alert;
  // The alert flag.
  bool alert;
} sdrkit_navic_satellite_row;

// The options of `family` of `sdrkit_gnss_fix_event`.
typedef enum sdrkit_gnss_fix_event_family {
  // Aircraft
  SDRKIT_GNSS_FIX_EVENT_FAMILY_AIRCRAFT = 0,
  // Vessel
  SDRKIT_GNSS_FIX_EVENT_FAMILY_VESSEL = 1,
  // APRS
  SDRKIT_GNSS_FIX_EVENT_FAMILY_APRS = 2,
  // Pager
  SDRKIT_GNSS_FIX_EVENT_FAMILY_PAGER = 3,
  // Radiosonde
  SDRKIT_GNSS_FIX_EVENT_FAMILY_SONDE = 4,
  // Broadcast
  SDRKIT_GNSS_FIX_EVENT_FAMILY_BROADCAST = 5,
  // Voice
  SDRKIT_GNSS_FIX_EVENT_FAMILY_VOICE = 6,
  // Message
  SDRKIT_GNSS_FIX_EVENT_FAMILY_MESSAGE = 7,
  // Sensor
  SDRKIT_GNSS_FIX_EVENT_FAMILY_SENSOR = 8,
  // Satellite
  SDRKIT_GNSS_FIX_EVENT_FAMILY_SATELLITE = 9,
  // Receiver
  SDRKIT_GNSS_FIX_EVENT_FAMILY_RECEIVER = 10,
  // Alert
  SDRKIT_GNSS_FIX_EVENT_FAMILY_ALERT = 11,
  // Other
  SDRKIT_GNSS_FIX_EVENT_FAMILY_OTHER = 12,
} sdrkit_gnss_fix_event_family;

// A fix.
typedef struct sdrkit_gnss_fix_event {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Seconds since the object started, in the time of the signal.
  double time;
  // The frequency of the signal in Hz.
  double freq_hz;
  // What an event is about (aircraft, vessel, pager). Outputs and the store filter on it. A value of `sdrkit_gnss_fix_event_family`.
  int32_t family;
  // The position of the object that the event is about.
  sdrkit_position position;
  // The height of the object that the event is about, metres above mean sea level. Unit: m.
  double height_m;
  // True if `speed_kt` holds a value.
  bool has_speed_kt;
  // The speed over the ground of the object that the event is about, knots. Unit: kt.
  double speed_kt;
  // True if `course_deg` holds a value.
  bool has_course_deg;
  // The course over the ground of the object that the event is about, degrees clockwise from true north. Unit: °.
  double course_deg;
  // Satellites in the fix.
  int64_t satellites_used;
} sdrkit_gnss_fix_event;

// The options of `family` of `sdrkit_gnss_text_nmea_event`.
typedef enum sdrkit_gnss_text_nmea_event_family {
  // Aircraft
  SDRKIT_GNSS_TEXT_NMEA_EVENT_FAMILY_AIRCRAFT = 0,
  // Vessel
  SDRKIT_GNSS_TEXT_NMEA_EVENT_FAMILY_VESSEL = 1,
  // APRS
  SDRKIT_GNSS_TEXT_NMEA_EVENT_FAMILY_APRS = 2,
  // Pager
  SDRKIT_GNSS_TEXT_NMEA_EVENT_FAMILY_PAGER = 3,
  // Radiosonde
  SDRKIT_GNSS_TEXT_NMEA_EVENT_FAMILY_SONDE = 4,
  // Broadcast
  SDRKIT_GNSS_TEXT_NMEA_EVENT_FAMILY_BROADCAST = 5,
  // Voice
  SDRKIT_GNSS_TEXT_NMEA_EVENT_FAMILY_VOICE = 6,
  // Message
  SDRKIT_GNSS_TEXT_NMEA_EVENT_FAMILY_MESSAGE = 7,
  // Sensor
  SDRKIT_GNSS_TEXT_NMEA_EVENT_FAMILY_SENSOR = 8,
  // Satellite
  SDRKIT_GNSS_TEXT_NMEA_EVENT_FAMILY_SATELLITE = 9,
  // Receiver
  SDRKIT_GNSS_TEXT_NMEA_EVENT_FAMILY_RECEIVER = 10,
  // Alert
  SDRKIT_GNSS_TEXT_NMEA_EVENT_FAMILY_ALERT = 11,
  // Other
  SDRKIT_GNSS_TEXT_NMEA_EVENT_FAMILY_OTHER = 12,
} sdrkit_gnss_text_nmea_event_family;

// An NMEA 0183 sentence.
typedef struct sdrkit_gnss_text_nmea_event {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Seconds since the object started, in the time of the signal.
  double time;
  // The frequency of the signal in Hz.
  double freq_hz;
  // The family of the event. A value of `sdrkit_gnss_text_nmea_event_family`.
  int32_t family;
  // The sentence.
  sdrkit_string text;
} sdrkit_gnss_text_nmea_event;

// A NavIC subframe.
typedef struct sdrkit_navic_subframe_event {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Seconds since the object started, in the time of the signal.
  double time;
  // The frequency of the signal in Hz.
  double freq_hz;
  // The PRN.
  int64_t prn;
  // The last subframe ID.
  int64_t subframe_id;
  // The time of week count.
  int64_t towc;
  // True if `wn` holds a value.
  bool has_wn;
  // The week number.
  int64_t wn;
  // True if `message_id` holds a value.
  bool has_message_id;
  // The message ID.
  int64_t message_id;
  // True if `prn_id` holds a value.
  bool has_prn_id;
  // The PRN ID.
  int64_t prn_id;
  // The alert flag.
  bool alert;
  // The autonav flag.
  bool autonav;
} sdrkit_navic_subframe_event;

// The arguments of `select` of the `sbas` stack.
typedef struct sdrkit_sbas_select_args {
  // The size of this struct: sizeof of the type. The caller sets it.
  uint32_t size;
  // The row key of the SBAS satellite: its PRN.
  sdrkit_string row;
} sdrkit_sbas_select_args;

// An arguments struct with its size set and every argument at 0.
#define SDRKIT_SBAS_SELECT_ARGS_INIT {sizeof(sdrkit_sbas_select_args), {0}}

// Fills `rows` with the changed rows of the table "systems" of the stack "gnss" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_gnss_systems_rows(SDRKIT_HANDLE handle,
                                              sdrkit_gnss_system_row *rows,
                                              uint32_t capacity,
                                              sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "gnss.GnssSystemRow", "systems", rows, sizeof(sdrkit_gnss_system_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "fix_systems" of the stack "gnss" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_gnss_fix_systems_rows(SDRKIT_HANDLE handle,
                                                  sdrkit_gnss_fix_system_row *rows,
                                                  uint32_t capacity,
                                                  sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "gnss.GnssFixSystemRow", "fix_systems", rows, sizeof(sdrkit_gnss_fix_system_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "satellites" of the stack "gnss" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_gnss_satellites_rows(SDRKIT_HANDLE handle,
                                                 sdrkit_gnss_satellite_row *rows,
                                                 uint32_t capacity,
                                                 sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "gnss.GnssSatelliteRow", "satellites", rows, sizeof(sdrkit_gnss_satellite_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "interference_lines" of the stack "gnss" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_gnss_interference_lines_rows(SDRKIT_HANDLE handle,
                                                         sdrkit_gnss_interference_line_row *rows,
                                                         uint32_t capacity,
                                                         sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "gnss.InterferenceLineRow", "interference_lines", rows, sizeof(sdrkit_gnss_interference_line_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "interference_events" of the stack "gnss" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_gnss_interference_events_rows(SDRKIT_HANDLE handle,
                                                          sdrkit_gnss_interference_event_row *rows,
                                                          uint32_t capacity,
                                                          sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "gnss.InterferenceEventRow", "interference_events", rows, sizeof(sdrkit_gnss_interference_event_row), capacity, info);
}

// Fills `event` with event `n` of the last sdrkit_events_take() on `handle`, an event of the type "gnss.fix"
// (Fix). Returns SDRKIT_E_WRONG_TYPE if the event is of another type.
static inline int32_t sdrkit_gnss_fix_event_read(SDRKIT_HANDLE handle, uint32_t n, sdrkit_gnss_fix_event *event) {
  return sdrkit_typed_event(handle, n, "gnss.fix", "gnss.GnssFixEvent", event, sizeof(sdrkit_gnss_fix_event));
}

// Fills `event` with event `n` of the last sdrkit_events_take() on `handle`, an event of the type "text.nmea"
// (NMEA sentence). Returns SDRKIT_E_WRONG_TYPE if the event is of another type.
static inline int32_t sdrkit_gnss_text_nmea_event_read(SDRKIT_HANDLE handle, uint32_t n, sdrkit_gnss_text_nmea_event *event) {
  return sdrkit_typed_event(handle, n, "text.nmea", "gnss.NmeaEvent", event, sizeof(sdrkit_gnss_text_nmea_event));
}

// Runs the command "reset" (Reset) of the stack "gnss" on the session of `handle`. Drops every channel and search. The navigation data stays.
static inline int32_t sdrkit_gnss_reset(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "reset", NULL, NULL, NULL, NULL, 0);
}

// The settings of the test signal generator of the stack "gnss" are the sdrkit_gnss_gen_*() functions
// after the settings of its receiver.

// The setting "tune" (Tune) of the stack "gnss". A value of `sdrkit_gnss_tune`. Which band the radio is tuned to. A tune that needs another sample rate restarts the radio.
static inline int32_t sdrkit_gnss_get_tune(SDRKIT_HANDLE handle, sdrkit_gnss_tune *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "tune", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_gnss_tune)index;
  return code;
}
static inline int32_t sdrkit_gnss_set_tune(SDRKIT_HANDLE handle, sdrkit_gnss_tune value) {
  return sdrkit_typed_setting_set_enum(handle, "tune", (int32_t)value);
}

// The setting "sys_gps" (GPS) of the stack "gnss". Search and track this system's satellites.
static inline int32_t sdrkit_gnss_get_sys_gps(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "sys_gps", value);
}
static inline int32_t sdrkit_gnss_set_sys_gps(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "sys_gps", value);
}

// The setting "sys_qzss" (QZSS) of the stack "gnss". Search and track this system's satellites.
static inline int32_t sdrkit_gnss_get_sys_qzss(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "sys_qzss", value);
}
static inline int32_t sdrkit_gnss_set_sys_qzss(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "sys_qzss", value);
}

// The setting "sys_sbas" (SBAS) of the stack "gnss". Search and track this system's satellites.
static inline int32_t sdrkit_gnss_get_sys_sbas(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "sys_sbas", value);
}
static inline int32_t sdrkit_gnss_set_sys_sbas(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "sys_sbas", value);
}

// The setting "sys_galileo" (Galileo) of the stack "gnss". Search and track this system's satellites.
static inline int32_t sdrkit_gnss_get_sys_galileo(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "sys_galileo", value);
}
static inline int32_t sdrkit_gnss_set_sys_galileo(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "sys_galileo", value);
}

// The setting "sys_beidou" (BeiDou) of the stack "gnss". Search and track this system's satellites.
static inline int32_t sdrkit_gnss_get_sys_beidou(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "sys_beidou", value);
}
static inline int32_t sdrkit_gnss_set_sys_beidou(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "sys_beidou", value);
}

// The setting "sys_glonass" (GLONASS) of the stack "gnss". Search and track this system's satellites.
static inline int32_t sdrkit_gnss_get_sys_glonass(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "sys_glonass", value);
}
static inline int32_t sdrkit_gnss_set_sys_glonass(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "sys_glonass", value);
}

// The setting "elevation_mask" (Elevation mask) of the stack "gnss". Unit: °. Satellites lower than this are tracked but left out of the position.
static inline int32_t sdrkit_gnss_get_elevation_mask(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "elevation_mask", value);
}
static inline int32_t sdrkit_gnss_set_elevation_mask(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "elevation_mask", value);
}

// The setting "search_half_width" (Search half width) of the stack "gnss". A frequency in Hz. The first frequency window while nothing is found.
static inline int32_t sdrkit_gnss_get_search_half_width(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "search_half_width", value);
}
static inline int32_t sdrkit_gnss_set_search_half_width(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "search_half_width", value);
}

// The setting "frequency_hint" (Radio frequency error) of the stack "gnss". A frequency in Hz. The remembered frequency error of this radio, as seen at 1575.42 MHz.
static inline int32_t sdrkit_gnss_get_frequency_hint(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "frequency_hint", value);
}
static inline int32_t sdrkit_gnss_set_frequency_hint(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "frequency_hint", value);
}

// The setting "frequency_hint_valid" (Use the remembered error) of the stack "gnss".
static inline int32_t sdrkit_gnss_get_frequency_hint_valid(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "frequency_hint_valid", value);
}
static inline int32_t sdrkit_gnss_set_frequency_hint_valid(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "frequency_hint_valid", value);
}

// The setting "acquisition_effort" (Acquisition effort) of the stack "gnss". Unit: FFT/ms. Search work per millisecond of signal.
static inline int32_t sdrkit_gnss_get_acquisition_effort(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "acquisition_effort", value);
}
static inline int32_t sdrkit_gnss_set_acquisition_effort(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "acquisition_effort", value);
}

// The setting "known_week" (Known week) of the stack "gnss". The GPS week used to resolve broadcast weeks. 0 takes it from the computer's clock.
static inline int32_t sdrkit_gnss_get_known_week(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "known_week", value);
}
static inline int32_t sdrkit_gnss_set_known_week(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "known_week", value);
}

// The setting "clock_compare" (Compare the computer clock) of the stack "gnss". On for a live radio: the computer's clock against GPS time.
static inline int32_t sdrkit_gnss_get_clock_compare(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "clock_compare", value);
}
static inline int32_t sdrkit_gnss_set_clock_compare(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "clock_compare", value);
}

// The setting "galileo_codes" (Galileo E1-B code table) of the stack "gnss". A text file with the E1-B memory codes of the Galileo OS SIS ICD, Annex C (one hexadecimal code a line).
static inline int32_t sdrkit_gnss_get_galileo_codes(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "galileo_codes", value);
}
static inline int32_t sdrkit_gnss_set_galileo_codes(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "galileo_codes", value);
}

// The setting "gen_gps" (GPS) of the test signal generator of the stack "gnss".
static inline int32_t sdrkit_gnss_gen_get_gps(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "gen_gps", value);
}
static inline int32_t sdrkit_gnss_gen_set_gps(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "gen_gps", value);
}

// The setting "gen_qzss" (QZSS) of the test signal generator of the stack "gnss".
static inline int32_t sdrkit_gnss_gen_get_qzss(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "gen_qzss", value);
}
static inline int32_t sdrkit_gnss_gen_set_qzss(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "gen_qzss", value);
}

// The setting "gen_sbas" (SBAS) of the test signal generator of the stack "gnss".
static inline int32_t sdrkit_gnss_gen_get_sbas(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "gen_sbas", value);
}
static inline int32_t sdrkit_gnss_gen_set_sbas(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "gen_sbas", value);
}

// The setting "gen_galileo" (Galileo) of the test signal generator of the stack "gnss".
static inline int32_t sdrkit_gnss_gen_get_galileo(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "gen_galileo", value);
}
static inline int32_t sdrkit_gnss_gen_set_galileo(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "gen_galileo", value);
}

// The setting "gen_beidou" (BeiDou) of the test signal generator of the stack "gnss".
static inline int32_t sdrkit_gnss_gen_get_beidou(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "gen_beidou", value);
}
static inline int32_t sdrkit_gnss_gen_set_beidou(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "gen_beidou", value);
}

// The setting "gen_glonass" (GLONASS) of the test signal generator of the stack "gnss".
static inline int32_t sdrkit_gnss_gen_get_glonass(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "gen_glonass", value);
}
static inline int32_t sdrkit_gnss_gen_set_glonass(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "gen_glonass", value);
}

// The setting "gen_sats" (Satellites (0: all)) of the test signal generator of the stack "gnss".
static inline int32_t sdrkit_gnss_gen_get_sats(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "gen_sats", value);
}
static inline int32_t sdrkit_gnss_gen_set_sats(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "gen_sats", value);
}

// The setting "gen_cold" (Cold start) of the test signal generator of the stack "gnss".
static inline int32_t sdrkit_gnss_gen_get_cold(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "gen_cold", value);
}
static inline int32_t sdrkit_gnss_gen_set_cold(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "gen_cold", value);
}

// Fills `rows` with the changed rows of the table "sbas_satellites" of the stack "sbas" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_sbas_satellites_rows(SDRKIT_HANDLE handle,
                                                 sdrkit_sbas_satellite_row *rows,
                                                 uint32_t capacity,
                                                 sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "gnss.SbasSatelliteRow", "sbas_satellites", rows, sizeof(sdrkit_sbas_satellite_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "sbas_satellite_types" of the stack "sbas" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_sbas_satellite_types_rows(SDRKIT_HANDLE handle,
                                                      sdrkit_sbas_satellite_type_row *rows,
                                                      uint32_t capacity,
                                                      sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "gnss.SbasSatelliteTypeRow", "sbas_satellite_types", rows, sizeof(sdrkit_sbas_satellite_type_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "types" of the stack "sbas" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_sbas_types_rows(SDRKIT_HANDLE handle,
                                            sdrkit_sbas_type_row *rows,
                                            uint32_t capacity,
                                            sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "gnss.SbasTypeRow", "types", rows, sizeof(sdrkit_sbas_type_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "messages" of the stack "sbas" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_sbas_messages_rows(SDRKIT_HANDLE handle,
                                               sdrkit_sbas_message_row *rows,
                                               uint32_t capacity,
                                               sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "gnss.SbasMessageRow", "messages", rows, sizeof(sdrkit_sbas_message_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "fast_corrections" of the stack "sbas" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_sbas_fast_corrections_rows(SDRKIT_HANDLE handle,
                                                       sdrkit_sbas_fast_correction_row *rows,
                                                       uint32_t capacity,
                                                       sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "gnss.SbasFastCorrectionRow", "fast_corrections", rows, sizeof(sdrkit_sbas_fast_correction_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "ionosphere" of the stack "sbas" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_sbas_ionosphere_rows(SDRKIT_HANDLE handle,
                                                 sdrkit_sbas_ionosphere_row *rows,
                                                 uint32_t capacity,
                                                 sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "gnss.SbasIonosphereRow", "ionosphere", rows, sizeof(sdrkit_sbas_ionosphere_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "interference_lines" of the stack "sbas" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_sbas_interference_lines_rows(SDRKIT_HANDLE handle,
                                                         sdrkit_gnss_interference_line_row *rows,
                                                         uint32_t capacity,
                                                         sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "gnss.InterferenceLineRow", "interference_lines", rows, sizeof(sdrkit_gnss_interference_line_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "interference_events" of the stack "sbas" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_sbas_interference_events_rows(SDRKIT_HANDLE handle,
                                                          sdrkit_gnss_interference_event_row *rows,
                                                          uint32_t capacity,
                                                          sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "gnss.InterferenceEventRow", "interference_events", rows, sizeof(sdrkit_gnss_interference_event_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "rx.systems" of the stack "sbas" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_sbas_rx_systems_rows(SDRKIT_HANDLE handle,
                                                 sdrkit_gnss_system_row *rows,
                                                 uint32_t capacity,
                                                 sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "gnss.GnssSystemRow", "rx.systems", rows, sizeof(sdrkit_gnss_system_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "rx.fix_systems" of the stack "sbas" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_sbas_rx_fix_systems_rows(SDRKIT_HANDLE handle,
                                                     sdrkit_gnss_fix_system_row *rows,
                                                     uint32_t capacity,
                                                     sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "gnss.GnssFixSystemRow", "rx.fix_systems", rows, sizeof(sdrkit_gnss_fix_system_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "rx.satellites" of the stack "sbas" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_sbas_rx_satellites_rows(SDRKIT_HANDLE handle,
                                                    sdrkit_gnss_satellite_row *rows,
                                                    uint32_t capacity,
                                                    sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "gnss.GnssSatelliteRow", "rx.satellites", rows, sizeof(sdrkit_gnss_satellite_row), capacity, info);
}

// Runs the command "reset" (Reset) of the stack "sbas" on the session of `handle`. Drops every channel and search. The navigation data stays.
static inline int32_t sdrkit_sbas_reset(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "reset", NULL, NULL, NULL, NULL, 0);
}

// Runs the command "select" (Select) of the stack "sbas" on the session of `handle`. Shows the corrections of this SBAS satellite.
static inline int32_t sdrkit_sbas_select(SDRKIT_HANDLE handle, const sdrkit_sbas_select_args *args) {
  return sdrkit_typed_command(handle, "select", "gnss.SbasSelectArgs", args, NULL, NULL, 0);
}

// Runs the command "clear" (Clear) of the stack "sbas" on the session of `handle`. Forgets every message decoded so far.
static inline int32_t sdrkit_sbas_clear(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "clear", NULL, NULL, NULL, NULL, 0);
}

// The setting "search_half_width" (Search half width) of the stack "sbas". A frequency in Hz. The first frequency window while nothing is found.
static inline int32_t sdrkit_sbas_get_search_half_width(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "search_half_width", value);
}
static inline int32_t sdrkit_sbas_set_search_half_width(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "search_half_width", value);
}

// The setting "frequency_hint" (Radio frequency error) of the stack "sbas". A frequency in Hz. The remembered frequency error of this radio, as seen at 1575.42 MHz.
static inline int32_t sdrkit_sbas_get_frequency_hint(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "frequency_hint", value);
}
static inline int32_t sdrkit_sbas_set_frequency_hint(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "frequency_hint", value);
}

// The setting "frequency_hint_valid" (Use the remembered error) of the stack "sbas".
static inline int32_t sdrkit_sbas_get_frequency_hint_valid(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "frequency_hint_valid", value);
}
static inline int32_t sdrkit_sbas_set_frequency_hint_valid(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "frequency_hint_valid", value);
}

// The setting "acquisition_effort" (Acquisition effort) of the stack "sbas". Unit: FFT/ms. Search work per millisecond of signal.
static inline int32_t sdrkit_sbas_get_acquisition_effort(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "acquisition_effort", value);
}
static inline int32_t sdrkit_sbas_set_acquisition_effort(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "acquisition_effort", value);
}

// Fills `rows` with the changed rows of the table "satellites" of the stack "navic" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_navic_satellites_rows(SDRKIT_HANDLE handle,
                                                  sdrkit_navic_satellite_row *rows,
                                                  uint32_t capacity,
                                                  sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "gnss.NavicSatelliteRow", "satellites", rows, sizeof(sdrkit_navic_satellite_row), capacity, info);
}

// Fills `event` with event `n` of the last sdrkit_events_take() on `handle`, an event of the type "navic.subframe"
// (NavIC subframe). Returns SDRKIT_E_WRONG_TYPE if the event is of another type.
static inline int32_t sdrkit_navic_subframe_event_read(SDRKIT_HANDLE handle, uint32_t n, sdrkit_navic_subframe_event *event) {
  return sdrkit_typed_event(handle, n, "navic.subframe", "gnss.NavicSubframeEvent", event, sizeof(sdrkit_navic_subframe_event));
}

// Runs the command "clear" (Clear) of the stack "navic" on the session of `handle`. Forgets everything decoded so far.
static inline int32_t sdrkit_navic_clear(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "clear", NULL, NULL, NULL, NULL, 0);
}

// The settings of the test signal generator of the stack "navic" are the sdrkit_navic_gen_*() functions
// after the settings of its receiver.

// The setting "gen_sats" (Satellites) of the test signal generator of the stack "navic".
static inline int32_t sdrkit_navic_gen_get_sats(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "gen_sats", value);
}
static inline int32_t sdrkit_navic_gen_set_sats(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "gen_sats", value);
}

// The setting "gen_sat_doppler_hz" (Satellite Doppler) of the test signal generator of the stack "navic". Unit: Hz. Satellite i has this Doppler times 1, -0.5, 0.5, 0.75, 1, 1.25, 1.5.
static inline int32_t sdrkit_navic_gen_get_sat_doppler_hz(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_sat_doppler_hz", value);
}
static inline int32_t sdrkit_navic_gen_set_sat_doppler_hz(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_sat_doppler_hz", value);
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_gnss_fields_read(SDRKIT_HANDLE handle, sdrkit_gnss_fields *fields) {
  return sdrkit_typed_fields(handle, "gnss.GnssFields", fields, sizeof(sdrkit_gnss_fields));
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_gnss_interference_fields_read(SDRKIT_HANDLE handle, sdrkit_gnss_interference_fields *fields) {
  return sdrkit_typed_fields(handle, "gnss.InterferenceFields", fields, sizeof(sdrkit_gnss_interference_fields));
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_gnss_guard_fields_read(SDRKIT_HANDLE handle, sdrkit_gnss_guard_fields *fields) {
  return sdrkit_typed_fields(handle, "gnss.GuardFields", fields, sizeof(sdrkit_gnss_guard_fields));
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_sbas_fields_read(SDRKIT_HANDLE handle, sdrkit_sbas_fields *fields) {
  return sdrkit_typed_fields(handle, "gnss.SbasFields", fields, sizeof(sdrkit_sbas_fields));
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_navic_fields_read(SDRKIT_HANDLE handle, sdrkit_navic_fields *fields) {
  return sdrkit_typed_fields(handle, "gnss.NavicFields", fields, sizeof(sdrkit_navic_fields));
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_navic_decoder_fields_read(SDRKIT_HANDLE handle, sdrkit_navic_decoder_fields *fields) {
  return sdrkit_typed_fields(handle, "gnss.DecoderFields", fields, sizeof(sdrkit_navic_decoder_fields));
}

// Fills `fields` with the result fields of the component "rx" of the stack "sbas" in the snapshot of
// `handle`. A generic consumer reads them under the keys "rx.<key>".
static inline int32_t sdrkit_sbas_rx_gnss_fields_read(SDRKIT_HANDLE handle, sdrkit_gnss_fields *fields) {
  return sdrkit_typed_fields(handle, "gnss.GnssFields@rx", fields, sizeof(sdrkit_gnss_fields));
}

#endif
