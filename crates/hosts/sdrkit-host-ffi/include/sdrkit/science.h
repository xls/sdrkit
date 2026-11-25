/*
 * sdrkit: the typed interface of the stack family "science", version 2.0. MIT licence.
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

#ifndef SDRKIT_SCIENCE_H
#define SDRKIT_SCIENCE_H

#include "../sdrkit.h"

// The version of the family that this header describes.
#define SDRKIT_SCIENCE_VERSION_MAJOR 2
#define SDRKIT_SCIENCE_VERSION_MINOR 0

// The stack "sonde": its id, and the keys of its event types.
#define SDRKIT_STACK_SONDE "sonde"
#define SDRKIT_EVENT_SONDE_FRAME "sonde.frame"

// The stack "time": its id, and the keys of its event types.
#define SDRKIT_STACK_TIME "time"
#define SDRKIT_EVENT_TIME_MINUTE "time.minute"

// The stack "eczas": its id, and the keys of its event types.
#define SDRKIT_STACK_ECZAS "eczas"
#define SDRKIT_EVENT_ECZAS_TIME "eczas.time"

// The stack "same": its id, and the keys of its event types.
#define SDRKIT_STACK_SAME "same"
#define SDRKIT_EVENT_SAME_ALERT "same.alert"
#define SDRKIT_EVENT_SAME_END "same.end"

// The stack "mscatter": its id, and the keys of its event types.
#define SDRKIT_STACK_MSCATTER "mscatter"
#define SDRKIT_EVENT_MS_PING "ms.ping"

// The stack "ssdv": its id, and the keys of its event types.
#define SDRKIT_STACK_SSDV "ssdv"

// The stack "astro": its id, and the keys of its event types.
#define SDRKIT_STACK_ASTRO "astro"

// The stack "iono": its id, and the keys of its event types.
#define SDRKIT_STACK_IONO "iono"

// The stack "sferics": its id, and the keys of its event types.
#define SDRKIT_STACK_SFERICS "sferics"
#define SDRKIT_EVENT_SFERICS_STORM "sferics.storm"

// The stack "supersid": its id, and the keys of its event types.
#define SDRKIT_STACK_SUPERSID "supersid"
#define SDRKIT_EVENT_SID_EVENT "sid.event"

// The stack "schumann": its id, and the keys of its event types.
#define SDRKIT_STACK_SCHUMANN "schumann"
#define SDRKIT_EVENT_SCIENCE_EVENT "science.event"

// The stack "whistler": its id, and the keys of its event types.
#define SDRKIT_STACK_WHISTLER "whistler"

// The stack "lightning_tdoa": its id, and the keys of its event types.
#define SDRKIT_STACK_LIGHTNING_TDOA "lightning_tdoa"
#define SDRKIT_EVENT_LIGHTNING_STRIKE "lightning.strike"

// The stack "jove": its id, and the keys of its event types.
#define SDRKIT_STACK_JOVE "jove"

// The stack "callisto": its id, and the keys of its event types.
#define SDRKIT_STACK_CALLISTO "callisto"

// The stack "riometer": its id, and the keys of its event types.
#define SDRKIT_STACK_RIOMETER "riometer"
#define SDRKIT_EVENT_RIOMETER_ABSORPTION "riometer.absorption"

// The stack "eme_echo": its id, and the keys of its event types.
#define SDRKIT_STACK_EME_ECHO "eme_echo"

// The stack "beacon_events": its id, and the keys of its event types.
#define SDRKIT_STACK_BEACON_EVENTS "beacon_events"

// The stack "pulsar": its id, and the keys of its event types.
#define SDRKIT_STACK_PULSAR "pulsar"
#define SDRKIT_EVENT_PULSAR_DETECTION "pulsar.detection"

// The stack "radar_detect": its id, and the keys of its event types.
#define SDRKIT_STACK_RADAR_DETECT "radar_detect"
#define SDRKIT_EVENT_RADAR_ROW "radar.row"

// The stack "horus": its id, and the keys of its event types.
#define SDRKIT_STACK_HORUS "horus"
#define SDRKIT_EVENT_HORUS_PACKET "horus.packet"
#define SDRKIT_EVENT_HORUS_CHANNEL_OPENED "horus.channel_opened"
#define SDRKIT_EVENT_HORUS_CHANNEL_CLOSED "horus.channel_closed"

// The options of the setting "station" of the stack "time".
typedef enum sdrkit_time_station {
  // Automatic (from the dial)
  SDRKIT_TIME_STATION_AUTO = 0,
  // DCF77
  SDRKIT_TIME_STATION_DCF77 = 1,
  // MSF
  SDRKIT_TIME_STATION_MSF = 2,
  // WWVB
  SDRKIT_TIME_STATION_WWVB = 3,
  // JJY 40 kHz
  SDRKIT_TIME_STATION_JJY40 = 4,
  // JJY 60 kHz
  SDRKIT_TIME_STATION_JJY60 = 5,
  // BPC
  SDRKIT_TIME_STATION_BPC = 6,
  // WWV / WWVH
  SDRKIT_TIME_STATION_WWV = 7,
  // CHU
  SDRKIT_TIME_STATION_CHU = 8,
  // BPM
  SDRKIT_TIME_STATION_BPM = 9,
  // BPM coded
  SDRKIT_TIME_STATION_BPMC = 10,
  // HLA
  SDRKIT_TIME_STATION_HLA = 11,
  // RWM
  SDRKIT_TIME_STATION_RWM = 12,
} sdrkit_time_station;

// The options of the setting "sixty_khz" of the stack "time".
typedef enum sdrkit_time_sixty_khz {
  // MSF
  SDRKIT_TIME_SIXTY_KHZ_MSF = 0,
  // WWVB
  SDRKIT_TIME_SIXTY_KHZ_WWVB = 1,
  // JJY 60 kHz
  SDRKIT_TIME_SIXTY_KHZ_JJY60 = 2,
} sdrkit_time_sixty_khz;

// The options of the setting "clock_mode" of the stack "time".
typedef enum sdrkit_time_clock_mode {
  // None (a file)
  SDRKIT_TIME_CLOCK_MODE_NONE = 0,
  // Host clock (a radio)
  SDRKIT_TIME_CLOCK_MODE_LIVE = 1,
  // Fixed origin (tests)
  SDRKIT_TIME_CLOCK_MODE_FIXED = 2,
} sdrkit_time_clock_mode;

// The options of the setting "clock_mode" of the stack "eczas".
typedef enum sdrkit_eczas_clock_mode {
  // None (a file)
  SDRKIT_ECZAS_CLOCK_MODE_NONE = 0,
  // Host clock (a radio)
  SDRKIT_ECZAS_CLOCK_MODE_LIVE = 1,
  // Fixed origin (tests)
  SDRKIT_ECZAS_CLOCK_MODE_FIXED = 2,
} sdrkit_eczas_clock_mode;

// The options of the setting "channel" of the stack "same".
typedef enum sdrkit_same_channel {
  // WX1
  SDRKIT_SAME_CHANNEL_WX1 = 0,
  // WX2
  SDRKIT_SAME_CHANNEL_WX2 = 1,
  // WX3
  SDRKIT_SAME_CHANNEL_WX3 = 2,
  // WX4
  SDRKIT_SAME_CHANNEL_WX4 = 3,
  // WX5
  SDRKIT_SAME_CHANNEL_WX5 = 4,
  // WX6
  SDRKIT_SAME_CHANNEL_WX6 = 5,
  // WX7
  SDRKIT_SAME_CHANNEL_WX7 = 6,
} sdrkit_same_channel;

// The options of the setting "country" of the stack "same".
typedef enum sdrkit_same_country {
  // United States
  SDRKIT_SAME_COUNTRY_US = 0,
  // Canada
  SDRKIT_SAME_COUNTRY_CANADA = 1,
} sdrkit_same_country;

// The options of the setting "gen_scene" of the stack "same".
typedef enum sdrkit_same_gen_scene {
  // Two alerts
  SDRKIT_SAME_GEN_SCENE_TWO = 0,
  // Tornado warning
  SDRKIT_SAME_GEN_SCENE_TORNADO = 1,
  // No message
  SDRKIT_SAME_GEN_SCENE_NOISE = 2,
} sdrkit_same_gen_scene;

// The options of the setting "baud" of the stack "ssdv".
typedef enum sdrkit_ssdv_baud {
  // Automatic (all at once)
  SDRKIT_SSDV_BAUD_AUTO = 0,
  // 300 (8N2)
  SDRKIT_SSDV_BAUD_300_8N2 = 1,
  // 600 (8N2)
  SDRKIT_SSDV_BAUD_600_8N2 = 2,
  // 1200 (8N2)
  SDRKIT_SSDV_BAUD_1200_8N2 = 3,
  // 1200
  SDRKIT_SSDV_BAUD_1200 = 4,
  // 2400
  SDRKIT_SSDV_BAUD_2400 = 5,
  // 4800
  SDRKIT_SSDV_BAUD_4800 = 6,
  // 9600
  SDRKIT_SSDV_BAUD_9600 = 7,
  // 19200
  SDRKIT_SSDV_BAUD_19200 = 8,
  // 115200 (Wenet)
  SDRKIT_SSDV_BAUD_115200 = 9,
} sdrkit_ssdv_baud;

// The options of the setting "gen_mode" of the stack "ssdv".
typedef enum sdrkit_ssdv_gen_mode {
  // Automatic (all at once)
  SDRKIT_SSDV_GEN_MODE_AUTO = 0,
  // 300 (8N2)
  SDRKIT_SSDV_GEN_MODE_300_8N2 = 1,
  // 600 (8N2)
  SDRKIT_SSDV_GEN_MODE_600_8N2 = 2,
  // 1200 (8N2)
  SDRKIT_SSDV_GEN_MODE_1200_8N2 = 3,
  // 1200
  SDRKIT_SSDV_GEN_MODE_1200 = 4,
  // 2400
  SDRKIT_SSDV_GEN_MODE_2400 = 5,
  // 4800
  SDRKIT_SSDV_GEN_MODE_4800 = 6,
  // 9600
  SDRKIT_SSDV_GEN_MODE_9600 = 7,
  // 19200
  SDRKIT_SSDV_GEN_MODE_19200 = 8,
  // 115200 (Wenet)
  SDRKIT_SSDV_GEN_MODE_115200 = 9,
} sdrkit_ssdv_gen_mode;

// The options of the setting "fft" of the stack "astro".
typedef enum sdrkit_astro_fft {
  // 1024
  SDRKIT_ASTRO_FFT_1024 = 0,
  // 2048
  SDRKIT_ASTRO_FFT_2048 = 1,
  // 4096
  SDRKIT_ASTRO_FFT_4096 = 2,
  // 8192
  SDRKIT_ASTRO_FFT_8192 = 3,
  // 16384
  SDRKIT_ASTRO_FFT_16384 = 4,
} sdrkit_astro_fft;

// The options of the setting "line" of the stack "astro".
typedef enum sdrkit_astro_line {
  // H I 1420.405751 MHz
  SDRKIT_ASTRO_LINE_HI = 0,
  // OH 1612.231 MHz
  SDRKIT_ASTRO_LINE_OH1612 = 1,
  // OH 1665.402 MHz
  SDRKIT_ASTRO_LINE_OH1665 = 2,
  // OH 1667.359 MHz
  SDRKIT_ASTRO_LINE_OH1667 = 3,
  // OH 1720.530 MHz
  SDRKIT_ASTRO_LINE_OH1720 = 4,
  // Custom
  SDRKIT_ASTRO_LINE_CUSTOM = 5,
} sdrkit_astro_line;

// The options of the setting "band_shape" of the stack "astro".
typedef enum sdrkit_astro_band_shape {
  // The spectrum itself
  SDRKIT_ASTRO_BAND_SHAPE_NONE = 0,
  // Off source
  SDRKIT_ASTRO_BAND_SHAPE_OFF_SOURCE = 1,
  // Frequency switching
  SDRKIT_ASTRO_BAND_SHAPE_SWITCHING = 2,
} sdrkit_astro_band_shape;

// The options of the setting "lsr" of the stack "astro".
typedef enum sdrkit_astro_lsr {
  // Kinematic (IAU)
  SDRKIT_ASTRO_LSR_KINEMATIC = 0,
  // Dynamical (Schönrich)
  SDRKIT_ASTRO_LSR_DYNAMICAL = 1,
} sdrkit_astro_lsr;

// The options of the setting "target" of the stack "astro".
typedef enum sdrkit_astro_target {
  // Galactic l/b
  SDRKIT_ASTRO_TARGET_GALACTIC = 0,
  // Sun
  SDRKIT_ASTRO_TARGET_SUN = 1,
  // Moon
  SDRKIT_ASTRO_TARGET_MOON = 2,
  // Cas A
  SDRKIT_ASTRO_TARGET_CASA = 3,
  // Cyg A
  SDRKIT_ASTRO_TARGET_CYGA = 4,
  // Sgr A*
  SDRKIT_ASTRO_TARGET_SGRA = 5,
  // Custom RA/Dec
  SDRKIT_ASTRO_TARGET_RADEC = 6,
} sdrkit_astro_target;

// The options of the setting "geometry" of the stack "iono".
typedef enum sdrkit_iono_geometry {
  // Automatic
  SDRKIT_IONO_GEOMETRY_AUTO = 0,
  // Vertical
  SDRKIT_IONO_GEOMETRY_VERTICAL = 1,
  // Oblique
  SDRKIT_IONO_GEOMETRY_OBLIQUE = 2,
} sdrkit_iono_geometry;

// The options of the setting "gen_scene" of the stack "sferics".
typedef enum sdrkit_sferics_gen_scene {
  // Storm
  SDRKIT_SFERICS_GEN_SCENE_STORM = 0,
  // Mains train only
  SDRKIT_SFERICS_GEN_SCENE_MAINS = 1,
  // Storm and mains train
  SDRKIT_SFERICS_GEN_SCENE_STORM_MAINS = 2,
  // Quiet
  SDRKIT_SFERICS_GEN_SCENE_QUIET = 3,
} sdrkit_sferics_gen_scene;

// The options of the setting "log_format" of the stack "supersid".
typedef enum sdrkit_supersid_log_format {
  // Off
  SDRKIT_SUPERSID_LOG_FORMAT_OFF = 0,
  // sid_format (one file per transmitter)
  SDRKIT_SUPERSID_LOG_FORMAT_SID_FORMAT = 1,
  // supersid_format (one file, all transmitters)
  SDRKIT_SUPERSID_LOG_FORMAT_SUPERSID_FORMAT = 2,
} sdrkit_supersid_log_format;

// The options of the setting "role" of the stack "lightning_tdoa".
typedef enum sdrkit_lightning_tdoa_role {
  // Local
  SDRKIT_LIGHTNING_TDOA_ROLE_LOCAL = 0,
  // Client
  SDRKIT_LIGHTNING_TDOA_ROLE_CLIENT = 1,
  // Server
  SDRKIT_LIGHTNING_TDOA_ROLE_SERVER = 2,
} sdrkit_lightning_tdoa_role;

// The options of the setting "system_clock" of the stack "lightning_tdoa".
typedef enum sdrkit_lightning_tdoa_system_clock {
  // Not synchronised
  SDRKIT_LIGHTNING_TDOA_SYSTEM_CLOCK_SYSTEM = 0,
  // NTP
  SDRKIT_LIGHTNING_TDOA_SYSTEM_CLOCK_NTP = 1,
  // PTP
  SDRKIT_LIGHTNING_TDOA_SYSTEM_CLOCK_PTP = 2,
} sdrkit_lightning_tdoa_system_clock;

// The options of the setting "fits_bitpix" of the stack "callisto".
typedef enum sdrkit_callisto_fits_bitpix {
  // 16 bits (the manual's example)
  SDRKIT_CALLISTO_FITS_BITPIX_16_BITS = 0,
  // 8 bits
  SDRKIT_CALLISTO_FITS_BITPIX_8_BITS = 1,
} sdrkit_callisto_fits_bitpix;

// The options of the setting "gen_dual_prf" of the stack "radar_detect".
typedef enum sdrkit_radar_detect_gen_dual_prf {
  // The submode's
  SDRKIT_RADAR_DETECT_GEN_DUAL_PRF_AUTO = 0,
  // Single
  SDRKIT_RADAR_DETECT_GEN_DUAL_PRF_SINGLE = 1,
  // Dual
  SDRKIT_RADAR_DETECT_GEN_DUAL_PRF_DUAL = 2,
} sdrkit_radar_detect_gen_dual_prf;

// The options of the setting "gen_content" of the stack "horus".
typedef enum sdrkit_horus_gen_content {
  // Horus Binary v2
  SDRKIT_HORUS_GEN_CONTENT_V2 = 0,
  // Horus Binary v1
  SDRKIT_HORUS_GEN_CONTENT_V1 = 1,
  // RTTY 100 baud 7N2
  SDRKIT_HORUS_GEN_CONTENT_RTTY100 = 2,
  // RTTY 300 baud 8N2
  SDRKIT_HORUS_GEN_CONTENT_RTTY300 = 3,
  // RTTY 50 baud 7N2
  SDRKIT_HORUS_GEN_CONTENT_RTTY50 = 4,
  // Three balloons
  SDRKIT_HORUS_GEN_CONTENT_THREE = 5,
} sdrkit_horus_gen_content;

// The result fields of `sonde`.
typedef struct sdrkit_sonde_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The number of sondes in the table.
  int64_t sondes;
  // Frames (packets, blocks, bursts) that passed their check.
  int64_t frames_ok;
  // Frames that failed their check.
  int64_t frames_bad;
  // Carrier searches so far.
  int64_t searches;
  // Channels in use.
  int64_t channels_used;
  // Channels allowed.
  int64_t channels_allowed;
  // The band searched (the input rate). A frequency in Hz.
  double band;
  // The capture is narrower than 2 MHz: only ±0.45 of the rate is searched.
  bool narrow;
  // True if `selected` holds a value.
  bool has_selected;
  // The row key of the sonde the plots show.
  sdrkit_string selected;
} sdrkit_sonde_fields;

// The options of `type` of `sdrkit_sonde_row`.
typedef enum sdrkit_sonde_row_type {
  // RS41
  SDRKIT_SONDE_ROW_TYPE_RS41 = 0,
  // RS92
  SDRKIT_SONDE_ROW_TYPE_RS92 = 1,
  // DFM
  SDRKIT_SONDE_ROW_TYPE_DFM = 2,
  // M10
  SDRKIT_SONDE_ROW_TYPE_M10 = 3,
  // M20
  SDRKIT_SONDE_ROW_TYPE_M20 = 4,
  // LMS6
  SDRKIT_SONDE_ROW_TYPE_LMS6 = 5,
} sdrkit_sonde_row_type;

// A row of the table `sondes`.
typedef struct sdrkit_sonde_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The family. A value of `sdrkit_sonde_row_type`.
  int32_t type;
  // True if `subtype` holds a value.
  bool has_subtype;
  // The subtype.
  sdrkit_string subtype;
  // The serial.
  sdrkit_string serial;
  // True if `channel` holds a value.
  bool has_channel;
  // The receiving channel.
  int64_t channel;
  // True if `frequency` holds a value.
  bool has_frequency;
  // The absolute frequency. A frequency in Hz.
  double frequency;
  // The offset from the centre. A frequency in Hz.
  double offset;
  // Signal to noise ratio of what is being decoded, dB (in the signal's own bandwidth, or per symbol where the stack says so). Unit: dB.
  double snr_db;
  // True if `frame` holds a value.
  bool has_frame;
  // The frame counter.
  int64_t frame;
  // True if `time` holds a value.
  bool has_time;
  // UTC from the sonde. Seconds since 1970 (UTC).
  double time;
  // True if `position` holds a value.
  bool has_position;
  // The position of the object that the event is about.
  sdrkit_position position;
  // True if `height_m` holds a value.
  bool has_height_m;
  // The height of the object that the event is about, metres above mean sea level. Unit: m.
  double height_m;
  // True if `max_alt_m` holds a value.
  bool has_max_alt_m;
  // The highest altitude. Unit: m.
  double max_alt_m;
  // True if `climb_m_s` holds a value.
  bool has_climb_m_s;
  // The vertical speed. Unit: m/s.
  double climb_m_s;
  // True if `speed_kt` holds a value.
  bool has_speed_kt;
  // The speed over the ground of the object that the event is about, knots. Unit: kt.
  double speed_kt;
  // True if `course_deg` holds a value.
  bool has_course_deg;
  // The course over the ground of the object that the event is about, degrees clockwise from true north. Unit: °.
  double course_deg;
  // True if `sats` holds a value.
  bool has_sats;
  // The satellites used.
  int64_t sats;
  // True if `temp_c` holds a value.
  bool has_temp_c;
  // The temperature. Unit: °C.
  double temp_c;
  // True if `humidity_pct` holds a value.
  bool has_humidity_pct;
  // The relative humidity. Unit: %.
  double humidity_pct;
  // True if `pressure_hpa` holds a value.
  bool has_pressure_hpa;
  // The pressure. Unit: hPa.
  double pressure_hpa;
  // True if `battery_v` holds a value.
  bool has_battery_v;
  // The battery. Unit: V.
  double battery_v;
  // True if `kill` holds a value.
  bool has_kill;
  // The burst or kill timer. A length of time in seconds.
  double kill;
  // True if `cal_held` holds a value.
  bool has_cal_held;
  // The calibration parts held.
  int64_t cal_held;
  // True if `cal_needed` holds a value.
  bool has_cal_needed;
  // The calibration parts needed.
  int64_t cal_needed;
  // Frames (packets, blocks, bursts) that passed their check.
  int64_t frames_ok;
  // Frames that failed their check.
  int64_t frames_bad;
  // Good frames as a share of all frames. Unit: %.
  double ok_pct;
  // True if `landing` holds a value.
  bool has_landing;
  // The predicted landing point.
  sdrkit_position landing;
  // True if `landing_in` holds a value.
  bool has_landing_in;
  // The time to the landing. A length of time in seconds.
  double landing_in;
  // True if `landing_distance_m` holds a value.
  bool has_landing_distance_m;
  // The distance from the station to the landing point. Unit: m.
  double landing_distance_m;
  // True if `landing_bearing_deg` holds a value.
  bool has_landing_bearing_deg;
  // The bearing from the station to the landing point. Unit: °.
  double landing_bearing_deg;
  // True if `heard` holds a value.
  bool has_heard;
  // Times heard.
  int64_t heard;
  // True if `last_heard` holds a value.
  bool has_last_heard;
  // Wall-clock time when last heard, in whole seconds. A host shows the age from it. Seconds since 1970 (UTC).
  double last_heard;
} sdrkit_sonde_row;

// A carrier of the last search. The row key is the place in the list, two digits.
typedef struct sdrkit_sonde_carrier_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The offset from the centre. A frequency in Hz.
  double offset;
  // True if `frequency` holds a value.
  bool has_frequency;
  // The absolute frequency. A frequency in Hz.
  double frequency;
  // The carrier level. Unit: dB.
  double level_db;
  // True if `channel` holds a value.
  bool has_channel;
  // The receiving channel.
  int64_t channel;
} sdrkit_sonde_carrier_row;

// The options of `station` of `sdrkit_time_fields`.
typedef enum sdrkit_time_fields_station {
  // DCF77
  SDRKIT_TIME_FIELDS_STATION_DCF77 = 0,
  // MSF
  SDRKIT_TIME_FIELDS_STATION_MSF = 1,
  // WWVB
  SDRKIT_TIME_FIELDS_STATION_WWVB = 2,
  // JJY 40 kHz
  SDRKIT_TIME_FIELDS_STATION_JJY40 = 3,
  // JJY 60 kHz
  SDRKIT_TIME_FIELDS_STATION_JJY60 = 4,
  // BPC
  SDRKIT_TIME_FIELDS_STATION_BPC = 5,
  // WWV / WWVH
  SDRKIT_TIME_FIELDS_STATION_WWV = 6,
  // CHU
  SDRKIT_TIME_FIELDS_STATION_CHU = 7,
  // BPM
  SDRKIT_TIME_FIELDS_STATION_BPM = 8,
  // BPM coded
  SDRKIT_TIME_FIELDS_STATION_BPMC = 9,
  // HLA
  SDRKIT_TIME_FIELDS_STATION_HLA = 10,
  // RWM
  SDRKIT_TIME_FIELDS_STATION_RWM = 11,
} sdrkit_time_fields_station;

// The options of `path` of `sdrkit_time_fields`.
typedef enum sdrkit_time_fields_path {
  // Ground wave
  SDRKIT_TIME_FIELDS_PATH_GROUND_WAVE = 0,
  // Sky wave estimate
  SDRKIT_TIME_FIELDS_PATH_SKY_WAVE = 1,
} sdrkit_time_fields_path;

// The options of `weekday` of `sdrkit_time_fields`.
typedef enum sdrkit_time_fields_weekday {
  // Monday
  SDRKIT_TIME_FIELDS_WEEKDAY_MONDAY = 0,
  // Tuesday
  SDRKIT_TIME_FIELDS_WEEKDAY_TUESDAY = 1,
  // Wednesday
  SDRKIT_TIME_FIELDS_WEEKDAY_WEDNESDAY = 2,
  // Thursday
  SDRKIT_TIME_FIELDS_WEEKDAY_THURSDAY = 3,
  // Friday
  SDRKIT_TIME_FIELDS_WEEKDAY_FRIDAY = 4,
  // Saturday
  SDRKIT_TIME_FIELDS_WEEKDAY_SATURDAY = 5,
  // Sunday
  SDRKIT_TIME_FIELDS_WEEKDAY_SUNDAY = 6,
} sdrkit_time_fields_weekday;

// The result fields of `time`.
typedef struct sdrkit_time_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The station decoded. A value of `sdrkit_time_fields_station`.
  int32_t station;
  // True if `wwvh` holds a value.
  bool has_wwvh;
  // WWVH (1200 Hz ticks) rather than WWV (1000 Hz ticks).
  bool wwvh;
  // The carrier level. Unit: dBFS.
  double carrier_dbfs;
  // Signal to noise ratio of what is being decoded, dB (in the signal's own bandwidth, or per symbol where the stack says so). Unit: dB.
  double snr_db;
  // The keying depth (LF) or the tick level over the background (HF). Unit: %.
  double depth_pct;
  // Where the second's edge falls, after a whole second of signal time. Unit: ms.
  double tick_ms;
  // The jitter of the edges. Unit: ms.
  double jitter_ms;
  // Minutes decoded.
  int64_t minutes_ok;
  // Minutes refused.
  int64_t minutes_bad;
  // Two minutes in a row agree.
  bool confirmed;
  // True if `distance_m` holds a value.
  bool has_distance_m;
  // The distance from the station. Unit: m.
  double distance_m;
  // True if `delay_ms` holds a value.
  bool has_delay_ms;
  // The path delay. Unit: ms.
  double delay_ms;
  // True if `path` holds a value.
  bool has_path;
  // How the path delay was found. A value of `sdrkit_time_fields_path`.
  int32_t path;
  // The symbols of this minute so far.
  sdrkit_string symbols;
  // True if `utc` holds a value.
  bool has_utc;
  // The received UTC now. Seconds since 1970 (UTC).
  double utc;
  // True if `received` holds a value.
  bool has_received;
  // The last minute as the station sends it.
  sdrkit_string received;
  // True if `dut1_s` holds a value.
  bool has_dut1_s;
  // DUT1 (UT1 − UTC). Unit: s.
  double dut1_s;
  // True if `summer` holds a value.
  bool has_summer;
  // Summer time is in force.
  bool summer;
  // True if `weekday` holds a value.
  bool has_weekday;
  // The day of the week sent, by name. A value of `sdrkit_time_fields_weekday`.
  int32_t weekday;
  // True if `clock_offset_s` holds a value.
  bool has_clock_offset_s;
  // The computer clock minus the received time, now. Unit: s.
  double clock_offset_s;
  // True if `clock_offset_avg_s` holds a value.
  bool has_clock_offset_avg_s;
  // The same, averaged over the last minute. Unit: s.
  double clock_offset_avg_s;
} sdrkit_time_fields;

// The options of `refusal` of `sdrkit_time_minute_row`.
typedef enum sdrkit_time_minute_row_refusal {
  // A data second holds another symbol
  SDRKIT_TIME_MINUTE_ROW_REFUSAL_SYMBOL = 0,
  // A second was not read
  SDRKIT_TIME_MINUTE_ROW_REFUSAL_NOT_READ = 1,
  // A marker is misplaced
  SDRKIT_TIME_MINUTE_ROW_REFUSAL_MARKER = 2,
  // A fixed bit is wrong
  SDRKIT_TIME_MINUTE_ROW_REFUSAL_FIXED_BIT = 3,
  // The fixed bits are wrong
  SDRKIT_TIME_MINUTE_ROW_REFUSAL_FIXED_BITS = 4,
  // Second 59 is keyed
  SDRKIT_TIME_MINUTE_ROW_REFUSAL_KEYED_59 = 5,
  // Second 0 is keyed
  SDRKIT_TIME_MINUTE_ROW_REFUSAL_KEYED_0 = 6,
  // The summer-time bits are equal
  SDRKIT_TIME_MINUTE_ROW_REFUSAL_SUMMER_BITS = 7,
  // Parity error
  SDRKIT_TIME_MINUTE_ROW_REFUSAL_PARITY = 8,
  // A value is out of range
  SDRKIT_TIME_MINUTE_ROW_REFUSAL_VALUE_RANGE = 9,
  // The date is out of range
  SDRKIT_TIME_MINUTE_ROW_REFUSAL_DATE_RANGE = 10,
  // The day of the year is out of range
  SDRKIT_TIME_MINUTE_ROW_REFUSAL_DAY_OF_YEAR = 11,
  // The year is out of range
  SDRKIT_TIME_MINUTE_ROW_REFUSAL_YEAR_RANGE = 12,
  // The weekday is out of range
  SDRKIT_TIME_MINUTE_ROW_REFUSAL_WEEKDAY_RANGE = 13,
  // The weekday does not match the date
  SDRKIT_TIME_MINUTE_ROW_REFUSAL_WEEKDAY_MISMATCH = 14,
  // No minute marker at second 0
  SDRKIT_TIME_MINUTE_ROW_REFUSAL_NO_MINUTE_MARKER = 15,
  // No minute pulse
  SDRKIT_TIME_MINUTE_ROW_REFUSAL_NO_MINUTE_PULSE = 16,
  // The DUT1 sign bits are wrong
  SDRKIT_TIME_MINUTE_ROW_REFUSAL_DUT1_SIGN = 17,
  // DUT1 is doubled on both sides
  SDRKIT_TIME_MINUTE_ROW_REFUSAL_DUT1_DOUBLED = 18,
  // The leap-year bit is wrong
  SDRKIT_TIME_MINUTE_ROW_REFUSAL_LEAP_YEAR_BIT = 19,
  // The frame index is wrong
  SDRKIT_TIME_MINUTE_ROW_REFUSAL_FRAME_INDEX = 20,
  // Decoded from its data bursts
  SDRKIT_TIME_MINUTE_ROW_REFUSAL_FROM_BURSTS = 21,
  // No P0 marker
  SDRKIT_TIME_MINUTE_ROW_REFUSAL_NO_P0 = 22,
  // The two copies differ
  SDRKIT_TIME_MINUTE_ROW_REFUSAL_COPIES_DIFFER = 23,
  // Not an A frame
  SDRKIT_TIME_MINUTE_ROW_REFUSAL_NOT_A_FRAME = 24,
  // No S element
  SDRKIT_TIME_MINUTE_ROW_REFUSAL_NO_S_ELEMENT = 25,
  // No minute frame
  SDRKIT_TIME_MINUTE_ROW_REFUSAL_NO_MINUTE_FRAME = 26,
} sdrkit_time_minute_row_refusal;

// A minute of `time`. The row key is the sequence number of the minute.
typedef struct sdrkit_time_minute_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The signal time of a minute. Unit: s.
  double time_s;
  // True if `host_time` holds a value.
  bool has_host_time;
  // The host time of a minute. Seconds since 1970 (UTC).
  double host_time;
  // The decoded flag of a minute.
  bool ok;
  // True if `text` holds a value.
  bool has_text;
  // The text of a minute.
  sdrkit_string text;
  // True if `refusal` holds a value.
  bool has_refusal;
  // Why a minute was refused. A value of `sdrkit_time_minute_row_refusal`.
  int32_t refusal;
  // True if `second` holds a value.
  bool has_second;
  // The second where the decoder stopped.
  int64_t second;
  // The symbols of this minute so far.
  sdrkit_string symbols;
} sdrkit_time_minute_row;

// The options of `tx_state` of `sdrkit_eczas_fields`.
typedef enum sdrkit_eczas_fields_tx_state {
  // Normal
  SDRKIT_ECZAS_FIELDS_TX_STATE_NORMAL = 0,
  // Planned off for a day
  SDRKIT_ECZAS_FIELDS_TX_STATE_OFF_DAY = 1,
  // Planned off for a week
  SDRKIT_ECZAS_FIELDS_TX_STATE_OFF_WEEK = 2,
  // Planned off for longer than a week
  SDRKIT_ECZAS_FIELDS_TX_STATE_OFF_LONGER = 3,
} sdrkit_eczas_fields_tx_state;

// The result fields of `eczas`.
typedef struct sdrkit_eczas_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // True if `utc` holds a value.
  bool has_utc;
  // The received UTC now. Seconds since 1970 (UTC).
  double utc;
  // True if `tz_hours` holds a value.
  bool has_tz_hours;
  // The hours that the packet adds to UTC for the local time in Poland. Unit: h.
  int64_t tz_hours;
  // True if `leap_announce` holds a value.
  bool has_leap_announce;
  // A leap second is announced.
  bool leap_announce;
  // True if `leap_subtract` holds a value.
  bool has_leap_subtract;
  // The announced leap second is taken away.
  bool leap_subtract;
  // True if `dst_change_soon` holds a value.
  bool has_dst_change_soon;
  // A change of summer time is near.
  bool dst_change_soon;
  // True if `tx_state` holds a value.
  bool has_tx_state;
  // The transmitter state. A value of `sdrkit_eczas_fields_tx_state`.
  int32_t tx_state;
  // True if `last_packet` holds a value.
  bool has_last_packet;
  // The UTC of the last time packet. Seconds since 1970 (UTC).
  double last_packet;
  // Two packets in a row agree.
  bool confirmed;
  // Carrier offset from where it was expected, Hz. A frequency in Hz.
  double offset_hz;
  // Signal to noise ratio of what is being decoded, dB (in the signal's own bandwidth, or per symbol where the stack says so). Unit: dB.
  double snr_db;
  // Level of the signal or channel, dBFS (a mean power of 1 is 0 dBFS). Unit: dBFS.
  double level_dbfs;
  // The quality of the last sync.
  double sync_quality;
  // Syncs found.
  int64_t syncs;
  // Frames cut.
  int64_t frames;
  // Frames (packets, blocks, bursts) that passed their check.
  int64_t frames_ok;
  // Frames that failed their check.
  int64_t frames_bad;
  // Packets of the other slots.
  int64_t other_slots;
  // Syncs not followed by the sync word.
  int64_t false_syncs;
  // True if `distance_m` holds a value.
  bool has_distance_m;
  // The distance from the transmitter. Unit: m.
  double distance_m;
  // True if `delay_ms` holds a value.
  bool has_delay_ms;
  // The path delay. Unit: ms.
  double delay_ms;
  // True if `clock_offset_s` holds a value.
  bool has_clock_offset_s;
  // The computer clock minus the received time, now. Unit: s.
  double clock_offset_s;
  // True if `clock_offset_avg_s` holds a value.
  bool has_clock_offset_avg_s;
  // The same, averaged over the last 10 packets. Unit: s.
  double clock_offset_avg_s;
} sdrkit_eczas_fields;

// The options of `kind` of `sdrkit_eczas_frame_row`.
typedef enum sdrkit_eczas_frame_row_kind {
  // Time packet
  SDRKIT_ECZAS_FRAME_ROW_KIND_TIME = 0,
  // Bad frame
  SDRKIT_ECZAS_FRAME_ROW_KIND_BAD = 1,
  // Other slot
  SDRKIT_ECZAS_FRAME_ROW_KIND_OTHER = 2,
  // Disagrees
  SDRKIT_ECZAS_FRAME_ROW_KIND_DISAGREES = 3,
} sdrkit_eczas_frame_row_kind;

// A frame of `eczas`. The row key is the sequence number of the frame.
typedef struct sdrkit_eczas_frame_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The signal time of a frame. Unit: s.
  double signal_time_s;
  // True if `host_time` holds a value.
  bool has_host_time;
  // The host time of a frame. Seconds since 1970 (UTC).
  double host_time;
  // What a frame is. A value of `sdrkit_eczas_frame_row_kind`.
  int32_t kind;
  // The frame was accepted.
  bool ok;
  // The text of a frame.
  sdrkit_string text;
  // The bytes of a frame.
  sdrkit_bytes bytes;
} sdrkit_eczas_frame_row;

// The options of `channel` of `sdrkit_same_fields`.
typedef enum sdrkit_same_fields_channel {
  // WX1
  SDRKIT_SAME_FIELDS_CHANNEL_WX1 = 0,
  // WX2
  SDRKIT_SAME_FIELDS_CHANNEL_WX2 = 1,
  // WX3
  SDRKIT_SAME_FIELDS_CHANNEL_WX3 = 2,
  // WX4
  SDRKIT_SAME_FIELDS_CHANNEL_WX4 = 3,
  // WX5
  SDRKIT_SAME_FIELDS_CHANNEL_WX5 = 4,
  // WX6
  SDRKIT_SAME_FIELDS_CHANNEL_WX6 = 5,
  // WX7
  SDRKIT_SAME_FIELDS_CHANNEL_WX7 = 6,
} sdrkit_same_fields_channel;

// The options of `country` of `sdrkit_same_fields`.
typedef enum sdrkit_same_fields_country {
  // United States
  SDRKIT_SAME_FIELDS_COUNTRY_US = 0,
  // Canada
  SDRKIT_SAME_FIELDS_COUNTRY_CANADA = 1,
} sdrkit_same_fields_country;

// The result fields of `same`.
typedef struct sdrkit_same_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The channel. A value of `sdrkit_same_fields_channel`.
  int32_t channel;
  // The channel frequency. A frequency in Hz.
  double frequency;
  // Messages decoded.
  int64_t messages_ok;
  // Messages damaged.
  int64_t messages_bad;
  // An attention tone is on the air now.
  bool tone_now;
  // The country of the location names. A value of `sdrkit_same_fields_country`.
  int32_t country;
  // Counties in the loaded table.
  int64_t counties;
  // Signal to noise ratio of what is being decoded, dB (in the signal's own bandwidth, or per symbol where the stack says so). Unit: dB.
  double snr_db;
  // Level of the signal or channel, dBFS (a mean power of 1 is 0 dBFS). Unit: dBFS.
  double level_dbfs;
  // The carrier error. A frequency in Hz.
  double carrier;
} sdrkit_same_fields;

// The options of `kind` of `sdrkit_same_alert_row`.
typedef enum sdrkit_same_alert_row_kind {
  // Alert
  SDRKIT_SAME_ALERT_ROW_KIND_ALERT = 0,
  // End of message
  SDRKIT_SAME_ALERT_ROW_KIND_END = 1,
  // Damaged message
  SDRKIT_SAME_ALERT_ROW_KIND_DAMAGED = 2,
} sdrkit_same_alert_row_kind;

// The options of `event_kind` of `sdrkit_same_alert_row`.
typedef enum sdrkit_same_alert_row_event_kind {
  // Warning
  SDRKIT_SAME_ALERT_ROW_EVENT_KIND_WARNING = 0,
  // Watch
  SDRKIT_SAME_ALERT_ROW_EVENT_KIND_WATCH = 1,
  // Emergency
  SDRKIT_SAME_ALERT_ROW_EVENT_KIND_EMERGENCY = 2,
  // Statement
  SDRKIT_SAME_ALERT_ROW_EVENT_KIND_STATEMENT = 3,
  // Test
  SDRKIT_SAME_ALERT_ROW_EVENT_KIND_TEST = 4,
  // Message
  SDRKIT_SAME_ALERT_ROW_EVENT_KIND_MESSAGE = 5,
} sdrkit_same_alert_row_event_kind;

// The options of `error` of `sdrkit_same_alert_row`.
typedef enum sdrkit_same_alert_row_error {
  // Does not start with ZCZC-
  SDRKIT_SAME_ALERT_ROW_ERROR_NO_START = 0,
  // Bad originator
  SDRKIT_SAME_ALERT_ROW_ERROR_ORIGINATOR = 1,
  // Missing dash after the originator
  SDRKIT_SAME_ALERT_ROW_ERROR_ORIGINATOR_DASH = 2,
  // Bad event code
  SDRKIT_SAME_ALERT_ROW_ERROR_EVENT_CODE = 3,
  // Missing dash after the event code
  SDRKIT_SAME_ALERT_ROW_ERROR_EVENT_DASH = 4,
  // Bad location code
  SDRKIT_SAME_ALERT_ROW_ERROR_LOCATION_CODE = 5,
  // Message ends in the location list
  SDRKIT_SAME_ALERT_ROW_ERROR_ENDS_IN_LOCATIONS = 6,
  // More than 31 locations
  SDRKIT_SAME_ALERT_ROW_ERROR_TOO_MANY_LOCATIONS = 7,
  // Bad separator in the location list
  SDRKIT_SAME_ALERT_ROW_ERROR_LOCATION_SEPARATOR = 8,
  // Bad purge time
  SDRKIT_SAME_ALERT_ROW_ERROR_PURGE_TIME = 9,
  // Purge time minutes over 59
  SDRKIT_SAME_ALERT_ROW_ERROR_PURGE_MINUTES = 10,
  // Purge time zero
  SDRKIT_SAME_ALERT_ROW_ERROR_PURGE_ZERO = 11,
  // Missing dash after the purge time
  SDRKIT_SAME_ALERT_ROW_ERROR_PURGE_DASH = 12,
  // Bad issue time
  SDRKIT_SAME_ALERT_ROW_ERROR_ISSUE_TIME = 13,
  // Issue day out of range
  SDRKIT_SAME_ALERT_ROW_ERROR_ISSUE_DAY = 14,
  // Issue time out of range
  SDRKIT_SAME_ALERT_ROW_ERROR_ISSUE_RANGE = 15,
  // Missing dash after the issue time
  SDRKIT_SAME_ALERT_ROW_ERROR_ISSUE_DASH = 16,
  // Missing dash after the call sign
  SDRKIT_SAME_ALERT_ROW_ERROR_CALLSIGN_DASH = 17,
  // Call sign length
  SDRKIT_SAME_ALERT_ROW_ERROR_CALLSIGN_LENGTH = 18,
  // Bad call sign
  SDRKIT_SAME_ALERT_ROW_ERROR_CALLSIGN = 19,
  // Text after the message
  SDRKIT_SAME_ALERT_ROW_ERROR_TRAILING = 20,
} sdrkit_same_alert_row_error;

// A SAME message. The row key is the count of rows added before it, as six digits.
typedef struct sdrkit_same_alert_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The signal time of a row. Unit: s.
  double signal_time_s;
  // True if `host_time` holds a value.
  bool has_host_time;
  // The host time of a row. Seconds since 1970 (UTC).
  double host_time;
  // What the row is. A value of `sdrkit_same_alert_row_kind`.
  int32_t kind;
  // True if `event` holds a value.
  bool has_event;
  // The event code.
  sdrkit_string event;
  // True if `event_name` holds a value.
  bool has_event_name;
  // The event name.
  sdrkit_string event_name;
  // True if `event_kind` holds a value.
  bool has_event_kind;
  // The kind of event (warning, watch ...). A value of `sdrkit_same_alert_row_event_kind`.
  int32_t event_kind;
  // True if `originator` holds a value.
  bool has_originator;
  // The originator code.
  sdrkit_string originator;
  // True if `originator_name` holds a value.
  bool has_originator_name;
  // The originator's name.
  sdrkit_string originator_name;
  // The number of locations.
  int64_t locations;
  // True if `valid_for` holds a value.
  bool has_valid_for;
  // How long the message is valid. A length of time in seconds.
  double valid_for;
  // True if `issued` holds a value.
  bool has_issued;
  // When the message was issued. Seconds since 1970 (UTC).
  double issued;
  // True if `callsign` holds a value.
  bool has_callsign;
  // The sender's call sign.
  sdrkit_string callsign;
  // The repeats heard.
  int64_t repeats;
  // The repeats identical to the result.
  int64_t agreeing;
  // Bits the vote changed.
  int64_t bits_changed;
  // Signal to noise ratio of what is being decoded, dB (in the signal's own bandwidth, or per symbol where the stack says so). Unit: dB.
  double snr_db;
  // The carrier error. A frequency in Hz.
  double carrier;
  // True if `tone` holds a value.
  bool has_tone;
  // The attention tone that followed. A length of time in seconds.
  double tone;
  // The raw text.
  sdrkit_string raw;
  // True if `error` holds a value.
  bool has_error;
  // Why the message is damaged. A value of `sdrkit_same_alert_row_error`.
  int32_t error;
} sdrkit_same_alert_row;

// A location of a SAME message. The row key is `<alert>/<n>`.
typedef struct sdrkit_same_alert_location_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The alert a location belongs to.
  sdrkit_string alert;
  // A location code (PSSCCC).
  sdrkit_string code;
  // The location as people read it.
  sdrkit_string place;
  // True if `position` holds a value.
  bool has_position;
  // The position of the object that the event is about.
  sdrkit_position position;
} sdrkit_same_alert_location_row;

// An attention tone. The row key is the sequence number of the tone.
typedef struct sdrkit_same_tone_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The signal time of a row. Unit: s.
  double signal_time_s;
  // The length of a tone. A length of time in seconds.
  double duration;
  // The level of a tone. Unit: dB.
  double level_db;
} sdrkit_same_tone_row;

// The result fields of `mscatter`.
typedef struct sdrkit_mscatter_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Pings counted.
  int64_t pings;
  // Pings in the last 60 minutes.
  int64_t rate;
  // Broadband bursts refused.
  int64_t rejected;
  // True if `beacon` holds a value.
  bool has_beacon;
  // The beacon's offset from the centre. A frequency in Hz.
  double beacon;
  // The beacon level over the noise in one bin. Unit: dB.
  double beacon_snr_db;
  // Noise floor of the channel, dBFS. Unit: dBFS.
  double noise_dbfs;
  // The detection level. Unit: dB.
  double threshold_db;
  // True if `ping_now_db` holds a value.
  bool has_ping_now_db;
  // The level of the ping on the air now. Unit: dB.
  double ping_now_db;
} sdrkit_mscatter_fields;

// The options of `kind` of `sdrkit_mscatter_ping_row`.
typedef enum sdrkit_mscatter_ping_row_kind {
  // Underdense
  SDRKIT_MSCATTER_PING_ROW_KIND_UNDERDENSE = 0,
  // Overdense
  SDRKIT_MSCATTER_PING_ROW_KIND_OVERDENSE = 1,
} sdrkit_mscatter_ping_row_kind;

// A meteor ping. The row key is the sequence number of the ping.
typedef struct sdrkit_mscatter_ping_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The number of a ping.
  int64_t id;
  // UTC of a ping. Seconds since 1970 (UTC).
  double utc;
  // The length of a ping. A length of time in seconds.
  double duration;
  // The peak level over the background. Unit: dB.
  double peak_snr_db;
  // The Doppler shift at the peak. A frequency in Hz.
  double doppler;
  // True if `drift_hz_s` holds a value.
  bool has_drift_hz_s;
  // The drift of the Doppler shift. Unit: Hz/s.
  double drift_hz_s;
  // Underdense or overdense. A value of `sdrkit_mscatter_ping_row_kind`.
  int32_t kind;
} sdrkit_mscatter_ping_row;

// A meteor shower. The row key is its name.
typedef struct sdrkit_mscatter_shower_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The name of a shower.
  sdrkit_string name;
  // Days to the peak of a shower (below 0 after it). Unit: d.
  int64_t peak_in_days;
  // The shower is active.
  bool active;
  // The zenithal hourly rate of a shower.
  int64_t zhr;
} sdrkit_mscatter_shower_row;

// The result fields of SSDV.
typedef struct sdrkit_ssdv_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Packets received.
  int64_t packets_ok;
  // Packets refused.
  int64_t packets_bad;
  // Packets the RS code corrected.
  int64_t corrected;
  // Pictures complete.
  int64_t pictures_done;
  // True if `last_baud` holds a value.
  bool has_last_baud;
  // The speed of the last good packet. Unit: Bd.
  int64_t last_baud;
  // True if `last_n82` holds a value.
  bool has_last_n82;
  // The last good packet came as 8N2 bytes.
  bool last_n82;
} sdrkit_ssdv_fields;

// An SSDV packet. The row key is the sequence number of the packet.
typedef struct sdrkit_ssdv_packet_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // Signal time of a packet. Unit: s.
  double time_s;
  // True if `host_time` holds a value.
  bool has_host_time;
  // Host time of a packet. Seconds since 1970 (UTC).
  double host_time;
  // The call sign.
  sdrkit_string call;
  // The image id.
  int64_t image;
  // The packet id.
  int64_t packet;
  // Bytes corrected in a packet.
  int64_t rs;
  // The speed of a packet. Unit: Bd.
  int64_t baud;
  // 8N2 bytes.
  bool n82;
  // Inverted bits.
  bool inverted;
  // A packet without FEC.
  bool no_fec;
  // The last packet of a picture.
  bool eoi;
} sdrkit_ssdv_packet_row;

// An SSDV picture. The row key is the call sign and the image id.
typedef struct sdrkit_ssdv_picture_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The call sign.
  sdrkit_string call;
  // The image id.
  int64_t image;
  // The width of a picture. Unit: px.
  int64_t width;
  // The height of a picture. Unit: px.
  int64_t height;
  // The quality level of a picture.
  int64_t quality;
  // Packets held of a picture.
  int64_t held;
  // Packets missing of a picture.
  int64_t missing;
  // The picture is complete.
  bool complete;
  // The size of the JPEG.
  int64_t bytes;
} sdrkit_ssdv_picture_row;

// The options of `note` of `sdrkit_astro_fields`.
typedef enum sdrkit_astro_fields_note {
  // Frequency switching needs a radio
  SDRKIT_ASTRO_FIELDS_NOTE_SWITCHING_NEEDS_RADIO = 0,
  // The hot power must exceed the cold power
  SDRKIT_ASTRO_FIELDS_NOTE_Y_FACTOR = 1,
  // No clear line to fit
  SDRKIT_ASTRO_FIELDS_NOTE_NO_LINE = 2,
} sdrkit_astro_fields_note;

// The result fields of `astro`.
typedef struct sdrkit_astro_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Finished integrations.
  int64_t finished;
  // The progress of the current integration. Unit: %.
  double progress_pct;
  // The width of a bin. A frequency in Hz.
  double bin;
  // The width of a bin in velocity. Unit: km/s.
  double bin_km_s;
  // The span of the spectrum. A frequency in Hz.
  double span;
  // The span in velocity. Unit: km/s.
  double span_km_s;
  // Bins flagged as RFI in the last spectrum.
  int64_t bins_flagged;
  // True if `peak` holds a value.
  bool has_peak;
  // The frequency of the peak. A frequency in Hz.
  double peak;
  // True if `peak_km_s` holds a value.
  bool has_peak_km_s;
  // The velocity of the peak. Unit: km/s.
  double peak_km_s;
  // True if `excess_pct` holds a value.
  bool has_excess_pct;
  // The excess at the peak. Unit: %.
  double excess_pct;
  // True if `line_snr` holds a value.
  bool has_line_snr;
  // The peak over the residual rms.
  double line_snr;
  // True if `snr_db` holds a value.
  bool has_snr_db;
  // Signal to noise ratio of what is being decoded, dB (in the signal's own bandwidth, or per symbol where the stack says so). Unit: dB.
  double snr_db;
  // The right ascension of the target now. Unit: °.
  double target_ra_deg;
  // The declination of the target now. Unit: °.
  double target_dec_deg;
  // True if `target_az_deg` holds a value.
  bool has_target_az_deg;
  // The azimuth of the target, with a station position. Unit: °.
  double target_az_deg;
  // True if `target_el_deg` holds a value.
  bool has_target_el_deg;
  // The elevation of the target, with a station position. Unit: °.
  double target_el_deg;
  // True if `lsr_rotation_kms` holds a value.
  bool has_lsr_rotation_kms;
  // The observer's velocity towards the target from the Earth's rotation. Unit: km/s.
  double lsr_rotation_kms;
  // True if `lsr_orbit_kms` holds a value.
  bool has_lsr_orbit_kms;
  // The observer's velocity towards the target from the Earth's orbit. Unit: km/s.
  double lsr_orbit_kms;
  // True if `lsr_sun_kms` holds a value.
  bool has_lsr_sun_kms;
  // The observer's velocity towards the target from the Sun's motion. Unit: km/s.
  double lsr_sun_kms;
  // True if `vel_lsr_kms` holds a value.
  bool has_vel_lsr_kms;
  // The velocity of the peak towards the target, corrected to the local standard of rest. Unit: km/s.
  double vel_lsr_kms;
  // True if `trx_k` holds a value.
  bool has_trx_k;
  // The receiver temperature from the Y factor. Unit: K.
  double trx_k;
  // True if `gain_per_k` holds a value.
  bool has_gain_per_k;
  // The gain per kelvin from the Y factor.
  double gain_per_k;
  // True if `note` holds a value.
  bool has_note;
  // A note on the calibration, the switching or the fit. A value of `sdrkit_astro_fields_note`.
  int32_t note;
} sdrkit_astro_fields;

// A bin of the calibrated spectrum of the last finished integration. The row key is the bin, five digits.
typedef struct sdrkit_astro_calibrated_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The frequency of a bin. A frequency in Hz.
  double freq;
  // The topocentric velocity of a bin. Unit: km/s.
  double vel_topo_kms;
  // True if `vel_lsr_kms` holds a value.
  bool has_vel_lsr_kms;
  // The LSR velocity of a bin (none without a station position). Unit: km/s.
  double vel_lsr_kms;
  // The antenna temperature of a bin: the processed ratio times the system temperature. Unit: K.
  double ta_k;
  // The bin was flagged as RFI.
  bool flagged;
} sdrkit_astro_calibrated_row;

// A finished integration of the session. The row key is the sequence number of the integration.
typedef struct sdrkit_astro_session_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The signal time at the end of an integration. Unit: s.
  double signal_time_s;
  // The peak velocity of an integration. Unit: km/s.
  double peak_kms;
  // The excess at the peak. Unit: %.
  double excess_pct;
  // The peak over the residual rms.
  double line_snr;
} sdrkit_astro_session_row;

// A Gaussian of the last line fit. The row key is the place in the fit.
typedef struct sdrkit_astro_fit_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The centre of a fitted Gaussian. Unit: km/s.
  double centre_kms;
  // The amplitude of a fitted Gaussian. Unit: K.
  double amplitude_k;
  // The full width at half maximum of a fitted Gaussian. Unit: km/s.
  double fwhm_kms;
} sdrkit_astro_fit_row;

// A row of the table `lv_rows`.
typedef struct sdrkit_astro_map_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The galactic longitude of a row of the map. Unit: °.
  double longitude;
  // True if `terminal_kms` holds a value.
  bool has_terminal_kms;
  // The terminal velocity at a longitude. Unit: km/s.
  double terminal_kms;
} sdrkit_astro_map_row;

// The result fields of `iono`.
typedef struct sdrkit_iono_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The input rate. A frequency in Hz.
  double rate;
  // The radio frequency of the centre. A frequency in Hz.
  double centre;
  // The signal time (UTC with a radio, else from the first sample). Unit: s.
  double signal_time_s;
  // Sweeps start on whole seconds.
  bool timing_lock;
  // The span the radio sees. A frequency in Hz.
  double span;
  // Sounders heard.
  int64_t sounders_heard;
  // Sweeps being received.
  int64_t active_sweeps;
  // True if `detector_fft` holds a value.
  bool has_detector_fft;
  // The FFT size of the detector.
  int64_t detector_fft;
  // True if `detector_ms` holds a value.
  bool has_detector_ms;
  // The duration of a detector FFT. Unit: ms.
  double detector_ms;
  // True if `detector_bin` holds a value.
  bool has_detector_bin;
  // The bin width of the detector. A frequency in Hz.
  double detector_bin;
  // True if `ffts` holds a value.
  bool has_ffts;
  // FFTs done.
  int64_t ffts;
  // True if `loose_peaks` holds a value.
  bool has_loose_peaks;
  // Chirp peaks not yet tied to a sweep.
  int64_t loose_peaks;
  // True if `level_dbfs` holds a value.
  bool has_level_dbfs;
  // Level of the signal or channel, dBFS (a mean power of 1 is 0 dBFS). Unit: dBFS.
  double level_dbfs;
  // True if `selected` holds a value.
  bool has_selected;
  // The row key of the sounder whose ionogram and history are shown.
  sdrkit_string selected;
} sdrkit_iono_fields;

// The options of `geometry` of `sdrkit_iono_sounder_row`.
typedef enum sdrkit_iono_sounder_row_geometry {
  // Automatic
  SDRKIT_IONO_SOUNDER_ROW_GEOMETRY_AUTO = 0,
  // Vertical
  SDRKIT_IONO_SOUNDER_ROW_GEOMETRY_VERTICAL = 1,
  // Oblique
  SDRKIT_IONO_SOUNDER_ROW_GEOMETRY_OBLIQUE = 2,
} sdrkit_iono_sounder_row_geometry;

// A sounder heard. The row key is the sounder's id.
typedef struct sdrkit_iono_sounder_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The id of a sounder.
  int64_t id;
  // A sweep rate. Unit: kHz/s.
  double rate_khz_s;
  // True if `period` holds a value.
  bool has_period;
  // The repetition period. A length of time in seconds.
  double period;
  // True if `offset_s` holds a value.
  bool has_offset_s;
  // The start offset in the period. Unit: s.
  double offset_s;
  // The start of the last sweep. Unit: s.
  double last_start_s;
  // When the sounder was last heard. Unit: s.
  double last_seen_s;
  // True if `next_s` holds a value.
  bool has_next_s;
  // The start of the next sweep. Unit: s.
  double next_s;
  // Signal to noise ratio of what is being decoded, dB (in the signal's own bandwidth, or per symbol where the stack says so). Unit: dB.
  double snr_db;
  // The lowest radio frequency heard. A frequency in Hz.
  double heard_low;
  // The highest radio frequency heard. A frequency in Hz.
  double heard_high;
  // Sweeps received.
  int64_t sweeps;
  // Expected sweeps not heard.
  int64_t missed;
  // A sweep is being received.
  bool active;
  // True if `known` holds a value.
  bool has_known;
  // The name of the known sounder it matches.
  sdrkit_string known;
  // How well it matches: 0 no match, 1 the rate (and the distance), 2 the rate and the schedule.
  int64_t match_level;
  // The known sounders of the same rate.
  int64_t candidates;
  // True if `geometry` holds a value.
  bool has_geometry;
  // The geometry of the latest scaling. A value of `sdrkit_iono_sounder_row_geometry`.
  int32_t geometry;
  // True if `foe_mhz` holds a value.
  bool has_foe_mhz;
  // The critical frequency of the E layer. Unit: MHz.
  double foe_mhz;
  // True if `fof1_mhz` holds a value.
  bool has_fof1_mhz;
  // The critical frequency of the F1 layer. Unit: MHz.
  double fof1_mhz;
  // True if `fof2_mhz` holds a value.
  bool has_fof2_mhz;
  // The critical frequency of the F2 layer. Unit: MHz.
  double fof2_mhz;
  // True if `hpe_km` holds a value.
  bool has_hpe_km;
  // The virtual height of the E layer. Unit: km.
  double hpe_km;
  // True if `hpf_km` holds a value.
  bool has_hpf_km;
  // The virtual height of the F layer. Unit: km.
  double hpf_km;
  // True if `m3000` holds a value.
  bool has_m3000;
  // The propagation factor M(3000)F2.
  double m3000;
  // True if `muf3000_mhz` holds a value.
  bool has_muf3000_mhz;
  // The MUF over 3000 km. Unit: MHz.
  double muf3000_mhz;
  // True if `path_muf_mhz` holds a value.
  bool has_path_muf_mhz;
  // The MUF of the path from an oblique sounder. Unit: MHz.
  double path_muf_mhz;
  // True if `distance_km` holds a value.
  bool has_distance_km;
  // The distance of an oblique sounder. Unit: km.
  double distance_km;
} sdrkit_iono_sounder_row;

// A sweep heard. The row key is the sequence number of the sweep.
typedef struct sdrkit_iono_timeline_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The start of a sweep. Unit: s.
  double start_s;
  // The id of a sounder.
  int64_t id;
  // A sweep rate. Unit: kHz/s.
  double rate_khz_s;
  // The lowest frequency of the sweep that crossed the span. A frequency in Hz.
  double part_low;
  // The highest frequency of the sweep that crossed the span. A frequency in Hz.
  double part_high;
  // Signal to noise ratio of what is being decoded, dB (in the signal's own bandwidth, or per symbol where the stack says so). Unit: dB.
  double snr_db;
  // The sweep was expected.
  bool predicted;
} sdrkit_iono_timeline_row;

// The options of `state` of `sdrkit_iono_band_row`.
typedef enum sdrkit_iono_band_row_state {
  // Open
  SDRKIT_IONO_BAND_ROW_STATE_OPEN = 0,
  // Closed
  SDRKIT_IONO_BAND_ROW_STATE_CLOSED = 1,
  // Absorbed
  SDRKIT_IONO_BAND_ROW_STATE_ABSORBED = 2,
  // Unknown
  SDRKIT_IONO_BAND_ROW_STATE_UNKNOWN = 3,
} sdrkit_iono_band_row_state;

// A band and its state, from the selected sounder's usable MUF. The row key is the name of the band.
typedef struct sdrkit_iono_band_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // A band.
  sdrkit_string band;
  // The state of a band. A value of `sdrkit_iono_band_row_state`.
  int32_t state;
} sdrkit_iono_band_row;

// The result fields of `sferics`.
typedef struct sdrkit_sferics_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Strikes counted.
  int64_t strikes;
  // Flashes counted.
  int64_t flashes;
  // Impulses of a mains train left out.
  int64_t mains;
  // Impulses too long to be lightning.
  int64_t too_long;
  // Strikes in the last minute. Unit: 1/min.
  double per_min_1;
  // Strikes per minute over 10 minutes. Unit: 1/min.
  double per_min_10;
  // True if `floor_db` holds a value.
  bool has_floor_db;
  // The floor. Unit: dBFS.
  double floor_db;
  // The start level in use. Unit: dB.
  double threshold_db;
  // True if `last_peak_db` holds a value.
  bool has_last_peak_db;
  // The peak of the newest strike. Unit: dB.
  double last_peak_db;
  // True if `last_ago_s` holds a value.
  bool has_last_ago_s;
  // The time since the newest strike. Unit: s.
  double last_ago_s;
  // A mains train is being left out.
  bool mains_active;
  // The storm phase.
  sdrkit_string storm_phase;
  // The change of the median peak. Unit: dB.
  double storm_db_change;
  // The activity word of the 10-minute rate.
  sdrkit_string activity;
  // What no public document confirms.
  sdrkit_string note;
} sdrkit_sferics_fields;

// The result set that a [`DecoderComponent`] adds to the results of its decoder. The description of a stack with a decoder component names it after the result sets of the decoder.
typedef struct sdrkit_science_decoder_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // True if `dropped` holds a value.
  bool has_dropped;
  // Items dropped because the decoder was slow. It holds a value while the decoder runs on a helper thread.
  int64_t dropped;
} sdrkit_science_decoder_fields;

// A strike. The row key is its start, s of signal.
typedef struct sdrkit_sferics_strike_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The start of a strike. Unit: s.
  double time_s;
  // The start of a strike in UTC. Seconds since 1970 (UTC).
  double utc;
  // The peak of a strike over the floor. Unit: dB.
  double peak_db;
  // The width of a strike. Unit: us.
  double width_us;
  // The flash of a strike.
  int64_t flash;
} sdrkit_sferics_strike_row;

// The result fields of `supersid`.
typedef struct sdrkit_supersid_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Transmitters listed.
  int64_t listed_n;
  // Transmitters in span.
  int64_t in_span_n;
  // Transmitters present.
  int64_t present_n;
  // Intervals measured.
  int64_t intervals;
  // True if `bin_hz` holds a value.
  bool has_bin_hz;
  // The width of a bin of the spectrum. A frequency in Hz.
  double bin_hz;
  // Events in total.
  int64_t events_total;
  // True if `last_event_tx` holds a value.
  bool has_last_event_tx;
  // The transmitter of the last event.
  sdrkit_string last_event_tx;
  // True if `last_event_rise_db` holds a value.
  bool has_last_event_rise_db;
  // The rise of the last event. Unit: dB.
  double last_event_rise_db;
  // True if `last_event_age_s` holds a value.
  bool has_last_event_age_s;
  // The age of the last event. A length of time in seconds.
  double last_event_age_s;
  // True if `log_file` holds a value.
  bool has_log_file;
  // The data file written last.
  sdrkit_string log_file;
  // True if `log_error` holds a value.
  bool has_log_error;
  // Why the data file is not written.
  sdrkit_string log_error;
  // The text of a decoded message.
  sdrkit_string text;
  // What no public document confirms.
  sdrkit_string note;
} sdrkit_supersid_fields;

// A monitored transmitter, in list order. The row key is its place in the list, two digits.
typedef struct sdrkit_supersid_transmitter_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // A transmitter's label.
  sdrkit_string label;
  // The frequency a station was heard on. A frequency in Hz.
  double freq_hz;
  // In span.
  bool in_span;
  // Present.
  bool present;
  // True if `level_db` holds a value.
  bool has_level_db;
  // The level over the local noise. Unit: dB.
  double level_db;
  // True if `absolute_dbfs` holds a value.
  bool has_absolute_dbfs;
  // The absolute level. Unit: dBFS.
  double absolute_dbfs;
  // True if `baseline_db` holds a value.
  bool has_baseline_db;
  // The median of the last hour. Unit: dB.
  double baseline_db;
  // True if `rise_db` holds a value.
  bool has_rise_db;
  // The mean of the last 2 minutes less the baseline. Unit: dB.
  double rise_db;
  // An event is on.
  bool event_on;
  // True if `event_peak_db` holds a value.
  bool has_event_peak_db;
  // The peak step of the event that is on. Unit: dB.
  double event_peak_db;
  // True if `sunlit` holds a value.
  bool has_sunlit;
  // The sunlit share of the path.
  double sunlit;
  // The key of the series of minute means.
  sdrkit_string series;
} sdrkit_supersid_transmitter_row;

// A row of the table `sid_events`.
typedef struct sdrkit_supersid_event_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // A transmitter's label.
  sdrkit_string label;
  // The frequency a station was heard on. A frequency in Hz.
  double freq_hz;
  // The UTC of an event's start. Seconds since 1970 (UTC).
  double start_utc;
  // The signal time of an event's start. Unit: s.
  double signal_time_s;
  // The mean of the last 2 minutes less the baseline. Unit: dB.
  double rise_db;
  // The peak step of an event. Unit: dB.
  double peak_db;
  // True if `end_utc` holds a value.
  bool has_end_utc;
  // The UTC of an event's end. Seconds since 1970 (UTC).
  double end_utc;
} sdrkit_supersid_event_row;

// A terminator mark of the UTC day. The row key is its place in time order, three digits.
typedef struct sdrkit_supersid_mark_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // A transmitter's label.
  sdrkit_string label;
  // The point of the path of a mark.
  sdrkit_string point;
  // The crossing of a mark.
  sdrkit_string mark;
  // The UTC of a mark. Seconds since 1970 (UTC).
  double utc;
} sdrkit_supersid_mark_row;

// The result fields of `schumann`.
typedef struct sdrkit_schumann_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The text of a decoded message: the summary of the stack.
  sdrkit_string text;
  // True if `fundamental_hz` holds a value.
  bool has_fundamental_hz;
  // The fundamental, Hz. A frequency in Hz.
  double fundamental_hz;
  // True if `fundamental_db` holds a value.
  bool has_fundamental_db;
  // The height of the fundamental over the baseline. Unit: dB.
  double fundamental_db;
  // True if `fundamental_q` holds a value.
  bool has_fundamental_q;
  // The Q of the fundamental.
  double fundamental_q;
  // Resonances found.
  int64_t peaks_found;
  // Seconds of averaging. A length of time in seconds.
  double averaged_s;
  // True if `level_db` holds a value.
  bool has_level_db;
  // The level of the last group of the transient band. Unit: dB.
  double level_db;
  // Events in total.
  int64_t events_total;
  // What no public document confirms.
  sdrkit_string note;
} sdrkit_schumann_fields;

// A resonance. The row key is its number, 1 to 5.
typedef struct sdrkit_schumann_peak_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The resonance of a row of the peak table: 1 to 5.
  int64_t resonance;
  // The frequency of a resonance, 0 when none was found. A frequency in Hz.
  double frequency_hz;
  // The height of a resonance over the baseline. Unit: dB.
  double height_db;
  // The Q of a resonance.
  double q;
} sdrkit_schumann_peak_row;

// An event of a monitor (`schumann`, `whistler`, `jove`, `callisto`, `eme_echo`, `beacon_events`). The row key is the sequence number of the event.
typedef struct sdrkit_science_event_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The UTC of an event. Seconds since 1970 (UTC).
  double utc;
  // The signal time of an event. Unit: s.
  double signal_time_s;
  // The type of an event.
  sdrkit_string type;
  // The characteristic frequency of an event. A frequency in Hz.
  double frequency_hz;
  // The strength of an event over the background. Unit: dB.
  double peak_db;
  // The duration of an event. A length of time in seconds.
  double duration_s;
  // Stack-specific value 1.
  double aux1;
  // Stack-specific value 2.
  double aux2;
  // The note of an event.
  sdrkit_string event_note;
} sdrkit_science_event_row;

// A trend minute of `schumann`. The row key is the minute of the capture.
typedef struct sdrkit_schumann_trend_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The UTC of the end of the minute. Seconds since 1970 (UTC).
  double utc;
  // The fundamental, Hz. Unit: Hz.
  double fundamental_hz;
  // The height of a resonance over the baseline. Unit: dB.
  double height_db;
} sdrkit_schumann_trend_row;

// The result fields of `whistler`.
typedef struct sdrkit_whistler_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The text of a decoded message: the summary of the stack.
  sdrkit_string text;
  // Whistlers found.
  int64_t whistlers;
  // Tweeks found.
  int64_t tweeks;
  // Events in total.
  int64_t events_total;
  // Sferics counted.
  int64_t sferics;
  // True if `dispersion` holds a value.
  bool has_dispersion;
  // The dispersion of the last whistler. Unit: s√Hz.
  double dispersion;
  // True if `broadband_db` holds a value.
  bool has_broadband_db;
  // The broadband value of the newest row. Unit: dB.
  double broadband_db;
  // What no public document confirms.
  sdrkit_string note;
} sdrkit_whistler_fields;

// A trend minute of `whistler`. The row key is the minute of the capture.
typedef struct sdrkit_whistler_trend_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The UTC of the end of the minute. Seconds since 1970 (UTC).
  double utc;
  // Events of a trend minute.
  double events_per_min;
  // Sferics of a trend minute.
  double sferics_per_min;
} sdrkit_whistler_trend_row;

// The result fields of `lightning_tdoa`.
typedef struct sdrkit_lightning_tdoa_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The role in use.
  sdrkit_string role;
  // Sferics timed.
  int64_t pulses;
  // True if `last_peak_db` holds a value.
  bool has_last_peak_db;
  // The peak of the newest sferic. Unit: dB.
  double last_peak_db;
  // True if `floor_db` holds a value.
  bool has_floor_db;
  // The floor of the sferic timer. Unit: dBFS.
  double floor_db;
  // The best usable time source.
  sdrkit_string time_source;
  // True if `clock_sigma_us` holds a value.
  bool has_clock_sigma_us;
  // Its 1-sigma now. Unit: us.
  double clock_sigma_us;
  // True if `clock_ppm` holds a value.
  bool has_clock_ppm;
  // The rate error of its sample clock. Unit: ppm.
  double clock_ppm;
  // True if `clock_ppm_sigma` holds a value.
  bool has_clock_ppm_sigma;
  // The 1-sigma of the drift. Unit: ppm.
  double clock_ppm_sigma;
  // A detection now would be fine.
  bool fine;
  // True if `sigma_gps_us` holds a value.
  bool has_sigma_gps_us;
  // The 1-sigma of the GPS clock. Unit: us.
  double sigma_gps_us;
  // True if `sigma_gnss_us` holds a value.
  bool has_sigma_gnss_us;
  // The 1-sigma of the GNSS clock. Unit: us.
  double sigma_gnss_us;
  // True if `sigma_cv_us` holds a value.
  bool has_sigma_cv_us;
  // The 1-sigma of the common-view clock. Unit: us.
  double sigma_cv_us;
  // True if `sigma_ptp_us` holds a value.
  bool has_sigma_ptp_us;
  // The 1-sigma of the PTP clock. Unit: us.
  double sigma_ptp_us;
  // True if `sigma_ntp_us` holds a value.
  bool has_sigma_ntp_us;
  // The 1-sigma of the NTP clock. Unit: us.
  double sigma_ntp_us;
  // True if `sigma_system_us` holds a value.
  bool has_sigma_system_us;
  // The 1-sigma of the unsynchronised computer clock. Unit: us.
  double sigma_system_us;
  // The common-view clock stamps the detections.
  bool common_view;
  // True if `timing_locked` holds a value.
  bool has_timing_locked;
  // The timing transmitter is locked.
  bool timing_locked;
  // True if `timing_snr_db` holds a value.
  bool has_timing_snr_db;
  // Its SNR. Unit: dB.
  double timing_snr_db;
  // True if `timing_sigma_us` holds a value.
  bool has_timing_sigma_us;
  // The 1-sigma of its boundary. Unit: us.
  double timing_sigma_us;
  // True if `timing_carrier_hz` holds a value.
  bool has_timing_carrier_hz;
  // Its carrier error. Unit: Hz.
  double timing_carrier_hz;
  // The state of the network.
  sdrkit_string network;
  // Datagrams sent.
  int64_t sent;
  // Datagrams that could not be sent.
  int64_t send_errors;
  // True if `packets_ok` holds a value.
  bool has_packets_ok;
  // Datagrams accepted by the server.
  int64_t packets_ok;
  // True if `refused_mac` holds a value.
  bool has_refused_mac;
  // Refused by the MAC.
  int64_t refused_mac;
  // True if `replays` holds a value.
  bool has_replays;
  // Refused as replays.
  int64_t replays;
  // True if `stale` holds a value.
  bool has_stale;
  // Refused as stale.
  int64_t stale;
  // True if `malformed` holds a value.
  bool has_malformed;
  // Refused as malformed.
  int64_t malformed;
  // True if `rate_limited` holds a value.
  bool has_rate_limited;
  // Refused by the rate limit.
  int64_t rate_limited;
  // True if `groups` holds a value.
  bool has_groups;
  // Groups of detections tried.
  int64_t groups;
  // True if `fixes` holds a value.
  bool has_fixes;
  // Fixes made.
  int64_t fixes;
  // True if `rejected` holds a value.
  bool has_rejected;
  // Fixes refused by the chi-square test.
  int64_t rejected;
  // True if `unmatched` holds a value.
  bool has_unmatched;
  // Groups without a fix.
  int64_t unmatched;
  // True if `coarse_only` holds a value.
  bool has_coarse_only;
  // Groups with too few fine stations.
  int64_t coarse_only;
  // Impulses too long for lightning.
  int64_t too_long;
  // What no public document confirms.
  sdrkit_string note;
} sdrkit_lightning_tdoa_fields;

// A located strike (server). The row key is the sequence number of the strike, eight digits.
typedef struct sdrkit_lightning_tdoa_strike_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The time of a strike, UTC ns. Unit: ns.
  int64_t utc_ns;
  // Latitude. Unit: °.
  double lat;
  // Longitude. Unit: °.
  double lon;
  // The semi-major axis of the 95 % ellipse. Unit: km.
  double major_km;
  // The semi-minor axis. Unit: km.
  double minor_km;
  // The azimuth of the major axis. Unit: °.
  double az_deg;
  // Stations used.
  int64_t stations;
  // Coarse stations that heard it.
  int64_t also_heard;
  // The rms of the residuals. Unit: us.
  double rms_us;
  // Chi-square.
  double chi2;
  // Degrees of freedom.
  int64_t dof;
  // No degree of freedom checks it.
  bool unchecked;
  // A second solution fits as well.
  bool ambiguous;
  // The time is UTC.
  bool utc;
  // True if `time` holds a value.
  bool has_time;
  // The time of a strike, or `None` when the time is not UTC. Seconds since 1970 (UTC).
  double time;
} sdrkit_lightning_tdoa_strike_row;

// A station of the network (server). The row key is the station id, eight hex digits.
typedef struct sdrkit_lightning_tdoa_station_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The station id.
  sdrkit_string id;
  // The station name.
  sdrkit_string name;
  // Latitude. Unit: °.
  double lat;
  // Longitude. Unit: °.
  double lon;
  // The best usable time source.
  sdrkit_string time_source;
  // The 1-sigma of the station's clock. Unit: us.
  double sigma_us;
  // The time since the station's last datagram. Unit: s.
  double age_s;
  // Detections received.
  int64_t detections;
  // Detections used in fixes.
  int64_t used;
  // A detection now would be fine.
  bool fine;
  // The station's clock is UTC.
  bool tied_to_utc;
  // True if `cv_correction_us` holds a value.
  bool has_cv_correction_us;
  // The common-view correction. Unit: us.
  double cv_correction_us;
} sdrkit_lightning_tdoa_station_row;

// The result fields of the burst monitors `jove` and `callisto`.
typedef struct sdrkit_science_burst_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The text of a decoded message: the summary of the stack.
  sdrkit_string text;
  // Bursts found.
  int64_t bursts;
  // Events in total.
  int64_t events_total;
  // True if `level_db` holds a value.
  bool has_level_db;
  // The level of the last row. Unit: dB.
  double level_db;
  // True if `bin_hz` holds a value.
  bool has_bin_hz;
  // The width of a bin. A frequency in Hz.
  double bin_hz;
  // True if `row_s` holds a value.
  bool has_row_s;
  // The time of a row. A length of time in seconds.
  double row_s;
  // A burst is open.
  bool burst_open;
  // Files written.
  int64_t files;
  // The last file written, empty before the first.
  sdrkit_string last_file;
  // What no public document confirms.
  sdrkit_string note;
} sdrkit_science_burst_fields;

// A trend minute of the burst monitors (`jove`, `callisto`). The row key is the minute of the capture.
typedef struct sdrkit_science_burst_trend_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The UTC of the end of the minute. Seconds since 1970 (UTC).
  double utc;
  // The level of a trend minute. Unit: dB.
  double level_change_db;
  // Bursts of a trend minute.
  double bursts_per_min;
} sdrkit_science_burst_trend_row;

// The result fields of `riometer`.
typedef struct sdrkit_riometer_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The text of a decoded message: the summary of the stack.
  sdrkit_string text;
  // True if `power_db` holds a value.
  bool has_power_db;
  // The power of the last minute, dB of full scale. Unit: dBFS.
  double power_db;
  // True if `lst_h` holds a value.
  bool has_lst_h;
  // The local sidereal time. Unit: h.
  double lst_h;
  // True if `curve_db` holds a value.
  bool has_curve_db;
  // The curve at that time. Unit: dBFS.
  double curve_db;
  // True if `absorption_db` holds a value.
  bool has_absorption_db;
  // The absorption. Unit: dB.
  double absorption_db;
  // Bins of the curve with a value.
  int64_t curve_bins;
  // The curve is ready.
  bool curve_ready;
  // The interfered share.
  double interfered_share;
  // An event is open.
  bool event_on;
  // Minutes measured.
  int64_t minutes;
  // True if `qdc` holds a value.
  bool has_qdc;
  // The curve as text, as of the last minute measured or the last change of the curve.
  sdrkit_string qdc;
  // What no public document confirms.
  sdrkit_string note;
} sdrkit_riometer_fields;

// The reply of `export_qdc`.
typedef struct sdrkit_riometer_qdc_reply {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The curve as text.
  sdrkit_string qdc;
} sdrkit_riometer_qdc_reply;

// The result fields of `eme_echo`.
typedef struct sdrkit_eme_echo_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The text of a decoded message: the summary of the stack.
  sdrkit_string text;
  // Own pulses seen.
  int64_t pulses;
  // Echoes found.
  int64_t echoes;
  // True if `carrier_hz` holds a value.
  bool has_carrier_hz;
  // The own carrier. A frequency in Hz.
  double carrier_hz;
  // Transmitting now.
  bool transmitting;
  // True if `noise_db` holds a value.
  bool has_noise_db;
  // The noise per bin. Unit: dBFS.
  double noise_db;
  // True if `last_delay_s` holds a value.
  bool has_last_delay_s;
  // The delay of the last echo. A length of time in seconds.
  double last_delay_s;
  // True if `last_doppler_hz` holds a value.
  bool has_last_doppler_hz;
  // The Doppler shift of the last echo. A frequency in Hz.
  double last_doppler_hz;
  // True if `last_level_db` holds a value.
  bool has_last_level_db;
  // The level of the last echo. Unit: dB.
  double last_level_db;
  // What no public document confirms.
  sdrkit_string note;
} sdrkit_eme_echo_fields;

// A trend minute of `eme_echo`. The row key is the minute of the capture.
typedef struct sdrkit_eme_echo_trend_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The UTC of the end of the minute. Seconds since 1970 (UTC).
  double utc;
  // The largest echo level of a trend minute. Unit: dB.
  double echo_level_db;
  // The mean delay of a trend minute. Unit: s.
  double echo_delay_s;
} sdrkit_eme_echo_trend_row;

// The result fields of `beacon_events`.
typedef struct sdrkit_beacon_events_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The text of a decoded message: the summary of the stack.
  sdrkit_string text;
  // Openings found.
  int64_t openings;
  // True if `noise_db` holds a value.
  bool has_noise_db;
  // The noise per bin. Unit: dBFS.
  double noise_db;
  // An opening is open.
  bool open;
  // True if `width_hz` holds a value.
  bool has_width_hz;
  // The width of the last row. A frequency in Hz.
  double width_hz;
  // True if `z` holds a value.
  bool has_z;
  // The best z of the last row.
  double z;
  // True if `snr_db` holds a value.
  bool has_snr_db;
  // The S/N of the last row. Unit: dB.
  double snr_db;
  // True if `shift_hz` holds a value.
  bool has_shift_hz;
  // The shift of the last row. A frequency in Hz.
  double shift_hz;
  // What no public document confirms.
  sdrkit_string note;
} sdrkit_beacon_events_fields;

// A trend minute of `beacon_events`. The row key is the minute of the capture.
typedef struct sdrkit_beacon_events_trend_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The UTC of the end of the minute. Seconds since 1970 (UTC).
  double utc;
  // The peak S/N of the hit rows of a trend minute. Unit: dB.
  double peak_snr_db;
  // The mean width of the hit rows of a trend minute. Unit: Hz.
  double mean_width_hz;
} sdrkit_beacon_events_trend_row;

// The result fields of `pulsar`.
typedef struct sdrkit_pulsar_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The text of a decoded message: the summary of the stack.
  sdrkit_string text;
  // Whether a detection was made.
  bool ok;
  // True if `snr` holds a value.
  bool has_snr;
  // The best S/N.
  double snr;
  // True if `period_found_s` holds a value.
  bool has_period_found_s;
  // The best period. Unit: s.
  double period_found_s;
  // True if `dm_found` holds a value.
  bool has_dm_found;
  // The best DM. Unit: pc cm⁻³.
  double dm_found;
  // True if `peak_phase` holds a value.
  bool has_peak_phase;
  // The pulse phase.
  double peak_phase;
  // True if `duty` holds a value.
  bool has_duty;
  // The duty cycle.
  double duty;
  // The time observed. A length of time in seconds.
  double observed_s;
  // A search runs.
  bool searching;
  // Searches made.
  int64_t searches;
  // Detections made.
  int64_t detections;
  // Rows held.
  int64_t rows_held;
  // True if `row_ms` holds a value.
  bool has_row_ms;
  // The time of a row. Unit: ms.
  double row_ms;
  // True if `bank_channels` holds a value.
  bool has_bank_channels;
  // Channels of the filter bank.
  int64_t bank_channels;
  // True if `area_m2` holds a value.
  bool has_area_m2;
  // The area needed. Unit: m².
  double area_m2;
  // What no public document confirms.
  sdrkit_string note;
} sdrkit_pulsar_fields;

// A search of `pulsar`. The row key is the sequence number of the search.
typedef struct sdrkit_pulsar_search_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The time observed. A length of time in seconds.
  double observed_s;
  // Whether a detection was made.
  bool ok;
  // True if `snr` holds a value.
  bool has_snr;
  // The best S/N.
  double snr;
  // The best period. Unit: s.
  double period_found_s;
  // The best DM. Unit: pc cm⁻³.
  double dm_found;
  // The pulse phase.
  double peak_phase;
  // The duty cycle.
  double duty;
  // The signal time at the end of the rows searched. Unit: s.
  double time_s;
} sdrkit_pulsar_search_row;

// The result fields of `radar_detect`.
typedef struct sdrkit_radar_detect_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The text of a decoded message: the summary of the stack.
  sdrkit_string text;
  // True if `rotation_s` holds a value.
  bool has_rotation_s;
  // The rotation period. Unit: s.
  double rotation_s;
  // True if `rpm` holds a value.
  bool has_rpm;
  // Rotations per minute. Unit: rpm.
  double rpm;
  // True if `prf_hz` holds a value.
  bool has_prf_hz;
  // The repetition frequency of the fullest bin. A frequency in Hz.
  double prf_hz;
  // True if `prf` holds a value.
  bool has_prf;
  // The repetition text.
  sdrkit_string prf;
  // True if `pulse_width_us` holds a value.
  bool has_pulse_width_us;
  // The median pulse width. Unit: us.
  double pulse_width_us;
  // True if `beamwidth_deg` holds a value.
  bool has_beamwidth_deg;
  // The beamwidth. Unit: °.
  double beamwidth_deg;
  // Crossings found.
  int64_t crossings;
  // Pulses found.
  int64_t pulses;
  // Rows written.
  int64_t rows_total;
  // True if `noise_dbfs` holds a value.
  bool has_noise_dbfs;
  // The noise power. Unit: dBFS.
  double noise_dbfs;
  // True if `sweeps` holds a value.
  bool has_sweeps;
  // Weather: sweeps in this volume.
  int64_t sweeps;
  // True if `volumes` holds a value.
  bool has_volumes;
  // Weather: volume starts seen.
  int64_t volumes;
  // True if `last_volume_s` holds a value.
  bool has_last_volume_s;
  // Weather: the length of the last whole volume. Unit: s.
  double last_volume_s;
  // What no public document confirms.
  sdrkit_string note;
} sdrkit_radar_detect_fields;

// A row of `radar_detect`: a rotation (marine) or a sweep (weather). The row key is the sequence number of the row.
typedef struct sdrkit_radar_detect_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The signal time of a row. Unit: s.
  double signal_time_s;
  // The kind of a row: `rotation` or `sweep`.
  sdrkit_string type;
  // True if `rotation_s` holds a value.
  bool has_rotation_s;
  // The rotation period. Unit: s.
  double rotation_s;
  // True if `rpm` holds a value.
  bool has_rpm;
  // Rotations per minute. Unit: rpm.
  double rpm;
  // The repetition text.
  sdrkit_string prf;
  // True if `pulse_width_us` holds a value.
  bool has_pulse_width_us;
  // The median pulse width. Unit: us.
  double pulse_width_us;
  // True if `beamwidth_deg` holds a value.
  bool has_beamwidth_deg;
  // The beamwidth. Unit: °.
  double beamwidth_deg;
  // The peak of the crossing of a row. Unit: dB.
  double peak_db;
  // True if `volume` holds a value.
  bool has_volume;
  // The volume of a weather row.
  int64_t volume;
  // True if `sweep` holds a value.
  bool has_sweep;
  // The sweep of a weather row in its volume.
  int64_t sweep;
} sdrkit_radar_detect_row;

// The options of `state` of `sdrkit_horus_fields`.
typedef enum sdrkit_horus_fields_state {
  // Waiting
  SDRKIT_HORUS_FIELDS_STATE_WAITING = 0,
  // Listening
  SDRKIT_HORUS_FIELDS_STATE_LISTENING = 1,
  // Decoding
  SDRKIT_HORUS_FIELDS_STATE_DECODING = 2,
} sdrkit_horus_fields_state;

// The present state of the channels of `horus`. The channel bank writes it; a change of frequency clears it.
typedef struct sdrkit_horus_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Waiting for tones, listening to tones, or decoding (a packet within the last 30 s). A value of `sdrkit_horus_fields_state`.
  int32_t state;
  // Channels running, the dial channel included.
  int64_t channels;
  // Unique words found.
  int64_t syncs;
  // Frames that failed their CRC.
  int64_t failed;
  // True if `snr_db` holds a value.
  bool has_snr_db;
  // The best SNR in 3 kHz of a packet of the last 60 s. Unit: dB.
  double snr_db;
} sdrkit_horus_fields;

// What `horus` received: the packet log and the payloads. It stays over a change of frequency.
typedef struct sdrkit_horus_log_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Packets decoded, duplicates left out.
  int64_t packets;
  // Payloads heard within the last 120 s.
  int64_t payloads_active;
  // Lines of the setting `payload_ids` that could not be read.
  int64_t unreadable_ids;
} sdrkit_horus_log_fields;

// The options of `type` of `sdrkit_horus_payload_row`.
typedef enum sdrkit_horus_payload_row_type {
  // Horus Binary v1
  SDRKIT_HORUS_PAYLOAD_ROW_TYPE_V1 = 0,
  // Horus Binary v2
  SDRKIT_HORUS_PAYLOAD_ROW_TYPE_V2 = 1,
  // RTTY
  SDRKIT_HORUS_PAYLOAD_ROW_TYPE_RTTY = 2,
} sdrkit_horus_payload_row_type;

// A row of the table `payloads`. The row key is the payload: its callsign, or "ID n" for an ID that the list does not name.
typedef struct sdrkit_horus_payload_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The callsign, or "ID n".
  sdrkit_string payload;
  // The kind of the last packet. A value of `sdrkit_horus_payload_row_type`.
  int32_t type;
  // True if `payload_id` holds a value.
  bool has_payload_id;
  // The payload ID of a binary packet.
  int64_t payload_id;
  // The sequence number of the last packet.
  int64_t seq;
  // The time of the last packet: its time of day on the receive date, UTC. Seconds since 1970 (UTC).
  double time;
  // True if `position` holds a value.
  bool has_position;
  // The position of the object that the event is about.
  sdrkit_position position;
  // The height of the object that the event is about, metres above mean sea level. Unit: m.
  double height_m;
  // The highest altitude so far. Unit: m.
  double max_alt_m;
  // True if `climb_m_s` holds a value.
  bool has_climb_m_s;
  // The climb rate over the track of the last 30 to 180 s. Unit: m/s.
  double climb_m_s;
  // True if `speed_kmh` holds a value.
  bool has_speed_kmh;
  // The speed of a binary packet. Unit: km/h.
  double speed_kmh;
  // True if `sats` holds a value.
  bool has_sats;
  // The satellites of a binary packet.
  int64_t sats;
  // True if `temp_c` holds a value.
  bool has_temp_c;
  // The temperature of a binary packet. Unit: °C.
  double temp_c;
  // True if `battery_v` holds a value.
  bool has_battery_v;
  // The battery of a binary packet. Unit: V.
  double battery_v;
  // Signal to noise ratio of what is being decoded, dB (in the signal's own bandwidth, or per symbol where the stack says so). Unit: dB.
  double snr_db;
  // Packets of the payload.
  int64_t packets;
  // The middle of the tones of the last packet, from the dial. A frequency in Hz.
  double offset;
  // True if `frequency` holds a value.
  bool has_frequency;
  // The frequency of the middle of the tones. A frequency in Hz.
  double frequency;
  // The symbol rate of the last packet. Unit: Bd.
  double baud;
  // The tone spacing (4FSK) or shift (RTTY) of the last packet. A frequency in Hz.
  double spacing;
  // True if `corrections` holds a value.
  bool has_corrections;
  // The bits that the Golay code corrected in the last binary packet.
  int64_t corrections;
  // True if `rtty_bits` holds a value.
  bool has_rtty_bits;
  // The data bits of a character of the last RTTY packet.
  int64_t rtty_bits;
  // Points of the track.
  int64_t track_points;
  // The last packet as a sentence.
  sdrkit_string sentence;
  // True if `first_heard` holds a value.
  bool has_first_heard;
  // Wall-clock time when first heard, in whole seconds. Seconds since 1970 (UTC).
  double first_heard;
  // True if `last_heard` holds a value.
  bool has_last_heard;
  // Wall-clock time when last heard, in whole seconds. A host shows the age from it. Seconds since 1970 (UTC).
  double last_heard;
} sdrkit_horus_payload_row;

// A custom field of the last v2 packet of a payload, or a field after the altitude of its last RTTY sentence. The row key is the payload, "/" and the place of the field from 1.
typedef struct sdrkit_horus_custom_field_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The payload.
  sdrkit_string payload;
  // The name of the field.
  sdrkit_string name;
  // The value, written by its post-processing.
  sdrkit_string value;
} sdrkit_horus_custom_field_row;

// The options of `type` of `sdrkit_horus_packet_row`.
typedef enum sdrkit_horus_packet_row_type {
  // Horus Binary v1
  SDRKIT_HORUS_PACKET_ROW_TYPE_V1 = 0,
  // Horus Binary v2
  SDRKIT_HORUS_PACKET_ROW_TYPE_V2 = 1,
  // RTTY
  SDRKIT_HORUS_PACKET_ROW_TYPE_RTTY = 2,
} sdrkit_horus_packet_row_type;

// A row of the packet log. The row key is the number of the packet, six digits.
typedef struct sdrkit_horus_packet_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // When the packet was received. Seconds since 1970 (UTC).
  double received;
  // The payload.
  sdrkit_string payload;
  // The kind of the packet. A value of `sdrkit_horus_packet_row_type`.
  int32_t type;
  // Signal to noise ratio of what is being decoded, dB (in the signal's own bandwidth, or per symbol where the stack says so). Unit: dB.
  double snr_db;
  // The middle of the tones, from the dial. A frequency in Hz.
  double offset;
  // The channel that received it (0 is the dial).
  int64_t channel;
  // The packet as a sentence.
  sdrkit_string sentence;
} sdrkit_horus_packet_row;

// The options of `decoding` of `sdrkit_horus_channel_row`.
typedef enum sdrkit_horus_channel_row_decoding {
  // Listening
  SDRKIT_HORUS_CHANNEL_ROW_DECODING_LISTENING = 0,
  // Horus Binary
  SDRKIT_HORUS_CHANNEL_ROW_DECODING_BINARY = 1,
  // RTTY
  SDRKIT_HORUS_CHANNEL_ROW_DECODING_RTTY = 2,
} sdrkit_horus_channel_row_decoding;

// A row of the table `channels`. The row key is the channel number (0 is the dial channel).
typedef struct sdrkit_horus_channel_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The centre of the channel, from the dial. A frequency in Hz.
  double offset;
  // What the channel decodes. A value of `sdrkit_horus_channel_row_decoding`.
  int32_t decoding;
  // True if `tone_1` holds a value.
  bool has_tone_1;
  // The lowest 4FSK tone, from the channel centre. A frequency in Hz.
  double tone_1;
  // True if `tone_2` holds a value.
  bool has_tone_2;
  // The second 4FSK tone. A frequency in Hz.
  double tone_2;
  // True if `tone_3` holds a value.
  bool has_tone_3;
  // The third 4FSK tone. A frequency in Hz.
  double tone_3;
  // True if `tone_4` holds a value.
  bool has_tone_4;
  // The highest 4FSK tone. A frequency in Hz.
  double tone_4;
  // True if `rtty_low` holds a value.
  bool has_rtty_low;
  // The lower RTTY tone, from the channel centre. A frequency in Hz.
  double rtty_low;
  // True if `rtty_high` holds a value.
  bool has_rtty_high;
  // The higher RTTY tone. A frequency in Hz.
  double rtty_high;
  // True if `spacing` holds a value.
  bool has_spacing;
  // The 4FSK tone spacing. A frequency in Hz.
  double spacing;
  // True if `snr_db` holds a value.
  bool has_snr_db;
  // Signal to noise ratio of what is being decoded, dB (in the signal's own bandwidth, or per symbol where the stack says so). Unit: dB.
  double snr_db;
  // Packets decoded.
  int64_t packets;
  // Unique words found.
  int64_t syncs;
  // Frames that failed their CRC.
  int64_t failed;
  // The span search opened the channel.
  bool searched;
} sdrkit_horus_channel_row;

// The options of `family` of `sdrkit_sonde_frame_event`.
typedef enum sdrkit_sonde_frame_event_family {
  // Aircraft
  SDRKIT_SONDE_FRAME_EVENT_FAMILY_AIRCRAFT = 0,
  // Vessel
  SDRKIT_SONDE_FRAME_EVENT_FAMILY_VESSEL = 1,
  // APRS
  SDRKIT_SONDE_FRAME_EVENT_FAMILY_APRS = 2,
  // Pager
  SDRKIT_SONDE_FRAME_EVENT_FAMILY_PAGER = 3,
  // Radiosonde
  SDRKIT_SONDE_FRAME_EVENT_FAMILY_SONDE = 4,
  // Broadcast
  SDRKIT_SONDE_FRAME_EVENT_FAMILY_BROADCAST = 5,
  // Voice
  SDRKIT_SONDE_FRAME_EVENT_FAMILY_VOICE = 6,
  // Message
  SDRKIT_SONDE_FRAME_EVENT_FAMILY_MESSAGE = 7,
  // Sensor
  SDRKIT_SONDE_FRAME_EVENT_FAMILY_SENSOR = 8,
  // Satellite
  SDRKIT_SONDE_FRAME_EVENT_FAMILY_SATELLITE = 9,
  // Receiver
  SDRKIT_SONDE_FRAME_EVENT_FAMILY_RECEIVER = 10,
  // Alert
  SDRKIT_SONDE_FRAME_EVENT_FAMILY_ALERT = 11,
  // Other
  SDRKIT_SONDE_FRAME_EVENT_FAMILY_OTHER = 12,
} sdrkit_sonde_frame_event_family;

// The options of `type` of `sdrkit_sonde_frame_event`.
typedef enum sdrkit_sonde_frame_event_type {
  // RS41
  SDRKIT_SONDE_FRAME_EVENT_TYPE_RS41 = 0,
  // RS92
  SDRKIT_SONDE_FRAME_EVENT_TYPE_RS92 = 1,
  // DFM
  SDRKIT_SONDE_FRAME_EVENT_TYPE_DFM = 2,
  // M10
  SDRKIT_SONDE_FRAME_EVENT_TYPE_M10 = 3,
  // M20
  SDRKIT_SONDE_FRAME_EVENT_TYPE_M20 = 4,
  // LMS6
  SDRKIT_SONDE_FRAME_EVENT_TYPE_LMS6 = 5,
} sdrkit_sonde_frame_event_type;

// A sonde frame, every 5 s for each sonde. `source` is the serial, `text` the sonde for people.
typedef struct sdrkit_sonde_frame_event {
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
  // What an event is about (aircraft, vessel, pager). Outputs and the store filter on it. A value of `sdrkit_sonde_frame_event_family`.
  int32_t family;
  // True if `position` holds a value.
  bool has_position;
  // The position of the object that the event is about.
  sdrkit_position position;
  // True if `height_m` holds a value.
  bool has_height_m;
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
  // The family. A value of `sdrkit_sonde_frame_event_type`.
  int32_t type;
  // The serial.
  sdrkit_string serial;
  // True if `temp_c` holds a value.
  bool has_temp_c;
  // The temperature. Unit: °C.
  double temp_c;
  // True if `humidity_pct` holds a value.
  bool has_humidity_pct;
  // The relative humidity. Unit: %.
  double humidity_pct;
  // True if `climb_m_s` holds a value.
  bool has_climb_m_s;
  // The vertical speed. Unit: m/s.
  double climb_m_s;
  // True if `burst_killer` holds a value.
  bool has_burst_killer;
  // The sonde's burst or kill timer runs.
  bool burst_killer;
} sdrkit_sonde_frame_event;

// The options of `family` of `sdrkit_time_minute_event`.
typedef enum sdrkit_time_minute_event_family {
  // Aircraft
  SDRKIT_TIME_MINUTE_EVENT_FAMILY_AIRCRAFT = 0,
  // Vessel
  SDRKIT_TIME_MINUTE_EVENT_FAMILY_VESSEL = 1,
  // APRS
  SDRKIT_TIME_MINUTE_EVENT_FAMILY_APRS = 2,
  // Pager
  SDRKIT_TIME_MINUTE_EVENT_FAMILY_PAGER = 3,
  // Radiosonde
  SDRKIT_TIME_MINUTE_EVENT_FAMILY_SONDE = 4,
  // Broadcast
  SDRKIT_TIME_MINUTE_EVENT_FAMILY_BROADCAST = 5,
  // Voice
  SDRKIT_TIME_MINUTE_EVENT_FAMILY_VOICE = 6,
  // Message
  SDRKIT_TIME_MINUTE_EVENT_FAMILY_MESSAGE = 7,
  // Sensor
  SDRKIT_TIME_MINUTE_EVENT_FAMILY_SENSOR = 8,
  // Satellite
  SDRKIT_TIME_MINUTE_EVENT_FAMILY_SATELLITE = 9,
  // Receiver
  SDRKIT_TIME_MINUTE_EVENT_FAMILY_RECEIVER = 10,
  // Alert
  SDRKIT_TIME_MINUTE_EVENT_FAMILY_ALERT = 11,
  // Other
  SDRKIT_TIME_MINUTE_EVENT_FAMILY_OTHER = 12,
} sdrkit_time_minute_event_family;

// The options of `station` of `sdrkit_time_minute_event`.
typedef enum sdrkit_time_minute_event_station {
  // DCF77
  SDRKIT_TIME_MINUTE_EVENT_STATION_DCF77 = 0,
  // MSF
  SDRKIT_TIME_MINUTE_EVENT_STATION_MSF = 1,
  // WWVB
  SDRKIT_TIME_MINUTE_EVENT_STATION_WWVB = 2,
  // JJY 40 kHz
  SDRKIT_TIME_MINUTE_EVENT_STATION_JJY40 = 3,
  // JJY 60 kHz
  SDRKIT_TIME_MINUTE_EVENT_STATION_JJY60 = 4,
  // BPC
  SDRKIT_TIME_MINUTE_EVENT_STATION_BPC = 5,
  // WWV / WWVH
  SDRKIT_TIME_MINUTE_EVENT_STATION_WWV = 6,
  // CHU
  SDRKIT_TIME_MINUTE_EVENT_STATION_CHU = 7,
  // BPM
  SDRKIT_TIME_MINUTE_EVENT_STATION_BPM = 8,
  // BPM coded
  SDRKIT_TIME_MINUTE_EVENT_STATION_BPMC = 9,
  // HLA
  SDRKIT_TIME_MINUTE_EVENT_STATION_HLA = 10,
  // RWM
  SDRKIT_TIME_MINUTE_EVENT_STATION_RWM = 11,
} sdrkit_time_minute_event_station;

// A minute decoded: `text` holds the time as the station sends it, `utc` the start of the minute.
typedef struct sdrkit_time_minute_event {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Seconds since the object started, in the time of the signal.
  double time;
  // The frequency of the signal in Hz.
  double freq_hz;
  // The text of a decoded message.
  sdrkit_string text;
  // What an event is about (aircraft, vessel, pager). Outputs and the store filter on it. A value of `sdrkit_time_minute_event_family`.
  int32_t family;
  // The received UTC now. Seconds since 1970 (UTC).
  double utc;
  // The station decoded. A value of `sdrkit_time_minute_event_station`.
  int32_t station;
} sdrkit_time_minute_event;

// The options of `family` of `sdrkit_eczas_time_event`.
typedef enum sdrkit_eczas_time_event_family {
  // Aircraft
  SDRKIT_ECZAS_TIME_EVENT_FAMILY_AIRCRAFT = 0,
  // Vessel
  SDRKIT_ECZAS_TIME_EVENT_FAMILY_VESSEL = 1,
  // APRS
  SDRKIT_ECZAS_TIME_EVENT_FAMILY_APRS = 2,
  // Pager
  SDRKIT_ECZAS_TIME_EVENT_FAMILY_PAGER = 3,
  // Radiosonde
  SDRKIT_ECZAS_TIME_EVENT_FAMILY_SONDE = 4,
  // Broadcast
  SDRKIT_ECZAS_TIME_EVENT_FAMILY_BROADCAST = 5,
  // Voice
  SDRKIT_ECZAS_TIME_EVENT_FAMILY_VOICE = 6,
  // Message
  SDRKIT_ECZAS_TIME_EVENT_FAMILY_MESSAGE = 7,
  // Sensor
  SDRKIT_ECZAS_TIME_EVENT_FAMILY_SENSOR = 8,
  // Satellite
  SDRKIT_ECZAS_TIME_EVENT_FAMILY_SATELLITE = 9,
  // Receiver
  SDRKIT_ECZAS_TIME_EVENT_FAMILY_RECEIVER = 10,
  // Alert
  SDRKIT_ECZAS_TIME_EVENT_FAMILY_ALERT = 11,
  // Other
  SDRKIT_ECZAS_TIME_EVENT_FAMILY_OTHER = 12,
} sdrkit_eczas_time_event_family;

// The options of `tx_state` of `sdrkit_eczas_time_event`.
typedef enum sdrkit_eczas_time_event_tx_state {
  // Normal
  SDRKIT_ECZAS_TIME_EVENT_TX_STATE_NORMAL = 0,
  // Planned off for a day
  SDRKIT_ECZAS_TIME_EVENT_TX_STATE_OFF_DAY = 1,
  // Planned off for a week
  SDRKIT_ECZAS_TIME_EVENT_TX_STATE_OFF_WEEK = 2,
  // Planned off for longer than a week
  SDRKIT_ECZAS_TIME_EVENT_TX_STATE_OFF_LONGER = 3,
} sdrkit_eczas_time_event_tx_state;

// A time packet received: `text` holds the time for people, `utc` the time.
typedef struct sdrkit_eczas_time_event {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Seconds since the object started, in the time of the signal.
  double time;
  // The frequency of the signal in Hz.
  double freq_hz;
  // The text of a frame.
  sdrkit_string text;
  // What an event is about (aircraft, vessel, pager). Outputs and the store filter on it. A value of `sdrkit_eczas_time_event_family`.
  int32_t family;
  // The received UTC now. Seconds since 1970 (UTC).
  double utc;
  // The hours that the packet adds to UTC for the local time in Poland. Unit: h.
  int64_t tz_hours;
  // The transmitter state. A value of `sdrkit_eczas_time_event_tx_state`.
  int32_t tx_state;
} sdrkit_eczas_time_event;

// The options of `family` of `sdrkit_same_alert_event`.
typedef enum sdrkit_same_alert_event_family {
  // Aircraft
  SDRKIT_SAME_ALERT_EVENT_FAMILY_AIRCRAFT = 0,
  // Vessel
  SDRKIT_SAME_ALERT_EVENT_FAMILY_VESSEL = 1,
  // APRS
  SDRKIT_SAME_ALERT_EVENT_FAMILY_APRS = 2,
  // Pager
  SDRKIT_SAME_ALERT_EVENT_FAMILY_PAGER = 3,
  // Radiosonde
  SDRKIT_SAME_ALERT_EVENT_FAMILY_SONDE = 4,
  // Broadcast
  SDRKIT_SAME_ALERT_EVENT_FAMILY_BROADCAST = 5,
  // Voice
  SDRKIT_SAME_ALERT_EVENT_FAMILY_VOICE = 6,
  // Message
  SDRKIT_SAME_ALERT_EVENT_FAMILY_MESSAGE = 7,
  // Sensor
  SDRKIT_SAME_ALERT_EVENT_FAMILY_SENSOR = 8,
  // Satellite
  SDRKIT_SAME_ALERT_EVENT_FAMILY_SATELLITE = 9,
  // Receiver
  SDRKIT_SAME_ALERT_EVENT_FAMILY_RECEIVER = 10,
  // Alert
  SDRKIT_SAME_ALERT_EVENT_FAMILY_ALERT = 11,
  // Other
  SDRKIT_SAME_ALERT_EVENT_FAMILY_OTHER = 12,
} sdrkit_same_alert_event_family;

// The options of `urgency` of `sdrkit_same_alert_event`.
typedef enum sdrkit_same_alert_event_urgency {
  // None
  SDRKIT_SAME_ALERT_EVENT_URGENCY_NONE = 0,
  // Notice
  SDRKIT_SAME_ALERT_EVENT_URGENCY_NOTICE = 1,
  // Urgent
  SDRKIT_SAME_ALERT_EVENT_URGENCY_URGENT = 2,
  // Emergency
  SDRKIT_SAME_ALERT_EVENT_URGENCY_EMERGENCY = 3,
} sdrkit_same_alert_event_urgency;

// The options of `event_kind` of `sdrkit_same_alert_event`.
typedef enum sdrkit_same_alert_event_kind {
  // Warning
  SDRKIT_SAME_ALERT_EVENT_KIND_WARNING = 0,
  // Watch
  SDRKIT_SAME_ALERT_EVENT_KIND_WATCH = 1,
  // Emergency
  SDRKIT_SAME_ALERT_EVENT_KIND_EMERGENCY = 2,
  // Statement
  SDRKIT_SAME_ALERT_EVENT_KIND_STATEMENT = 3,
  // Test
  SDRKIT_SAME_ALERT_EVENT_KIND_TEST = 4,
  // Message
  SDRKIT_SAME_ALERT_EVENT_KIND_MESSAGE = 5,
} sdrkit_same_alert_event_kind;

// A SAME alert. `source` is the sender's call sign, `text` the event and the places, `position` the first place on the map.
typedef struct sdrkit_same_alert_event {
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
  // What an event is about (aircraft, vessel, pager). Outputs and the store filter on it. A value of `sdrkit_same_alert_event_family`.
  int32_t family;
  // How urgent an event is. A stack sets it when the decoded message is urgent. Services alert on it. A value of `sdrkit_same_alert_event_urgency`.
  int32_t urgency;
  // True if `position` holds a value.
  bool has_position;
  // The position of the object that the event is about.
  sdrkit_position position;
  // The event code.
  sdrkit_string event;
  // True if `event_name` holds a value.
  bool has_event_name;
  // The event name.
  sdrkit_string event_name;
  // True if `event_kind` holds a value.
  bool has_event_kind;
  // The kind of event (warning, watch ...). A value of `sdrkit_same_alert_event_kind`.
  int32_t event_kind;
  // The originator code.
  sdrkit_string originator;
  // How long the message is valid. A length of time in seconds.
  double valid_for;
  // The sender's call sign.
  sdrkit_string callsign;
  // The number of locations.
  int64_t locations;
  // The raw text.
  sdrkit_string raw;
} sdrkit_same_alert_event;

// The options of `family` of `sdrkit_same_end_event`.
typedef enum sdrkit_same_end_event_family {
  // Aircraft
  SDRKIT_SAME_END_EVENT_FAMILY_AIRCRAFT = 0,
  // Vessel
  SDRKIT_SAME_END_EVENT_FAMILY_VESSEL = 1,
  // APRS
  SDRKIT_SAME_END_EVENT_FAMILY_APRS = 2,
  // Pager
  SDRKIT_SAME_END_EVENT_FAMILY_PAGER = 3,
  // Radiosonde
  SDRKIT_SAME_END_EVENT_FAMILY_SONDE = 4,
  // Broadcast
  SDRKIT_SAME_END_EVENT_FAMILY_BROADCAST = 5,
  // Voice
  SDRKIT_SAME_END_EVENT_FAMILY_VOICE = 6,
  // Message
  SDRKIT_SAME_END_EVENT_FAMILY_MESSAGE = 7,
  // Sensor
  SDRKIT_SAME_END_EVENT_FAMILY_SENSOR = 8,
  // Satellite
  SDRKIT_SAME_END_EVENT_FAMILY_SATELLITE = 9,
  // Receiver
  SDRKIT_SAME_END_EVENT_FAMILY_RECEIVER = 10,
  // Alert
  SDRKIT_SAME_END_EVENT_FAMILY_ALERT = 11,
  // Other
  SDRKIT_SAME_END_EVENT_FAMILY_OTHER = 12,
} sdrkit_same_end_event_family;

// A SAME end of message.
typedef struct sdrkit_same_end_event {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Seconds since the object started, in the time of the signal.
  double time;
  // The frequency of the signal in Hz.
  double freq_hz;
  // The text of a decoded message.
  sdrkit_string text;
  // What an event is about (aircraft, vessel, pager). Outputs and the store filter on it. A value of `sdrkit_same_end_event_family`.
  int32_t family;
  // The raw text.
  sdrkit_string raw;
} sdrkit_same_end_event;

// The options of `family` of `sdrkit_mscatter_ms_ping_event`.
typedef enum sdrkit_mscatter_ms_ping_event_family {
  // Aircraft
  SDRKIT_MSCATTER_MS_PING_EVENT_FAMILY_AIRCRAFT = 0,
  // Vessel
  SDRKIT_MSCATTER_MS_PING_EVENT_FAMILY_VESSEL = 1,
  // APRS
  SDRKIT_MSCATTER_MS_PING_EVENT_FAMILY_APRS = 2,
  // Pager
  SDRKIT_MSCATTER_MS_PING_EVENT_FAMILY_PAGER = 3,
  // Radiosonde
  SDRKIT_MSCATTER_MS_PING_EVENT_FAMILY_SONDE = 4,
  // Broadcast
  SDRKIT_MSCATTER_MS_PING_EVENT_FAMILY_BROADCAST = 5,
  // Voice
  SDRKIT_MSCATTER_MS_PING_EVENT_FAMILY_VOICE = 6,
  // Message
  SDRKIT_MSCATTER_MS_PING_EVENT_FAMILY_MESSAGE = 7,
  // Sensor
  SDRKIT_MSCATTER_MS_PING_EVENT_FAMILY_SENSOR = 8,
  // Satellite
  SDRKIT_MSCATTER_MS_PING_EVENT_FAMILY_SATELLITE = 9,
  // Receiver
  SDRKIT_MSCATTER_MS_PING_EVENT_FAMILY_RECEIVER = 10,
  // Alert
  SDRKIT_MSCATTER_MS_PING_EVENT_FAMILY_ALERT = 11,
  // Other
  SDRKIT_MSCATTER_MS_PING_EVENT_FAMILY_OTHER = 12,
} sdrkit_mscatter_ms_ping_event_family;

// The options of `kind` of `sdrkit_mscatter_ms_ping_event`.
typedef enum sdrkit_mscatter_ms_ping_event_kind {
  // Underdense
  SDRKIT_MSCATTER_MS_PING_EVENT_KIND_UNDERDENSE = 0,
  // Overdense
  SDRKIT_MSCATTER_MS_PING_EVENT_KIND_OVERDENSE = 1,
} sdrkit_mscatter_ms_ping_event_kind;

// A meteor ping. `text` holds the ping for people.
typedef struct sdrkit_mscatter_ms_ping_event {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Seconds since the object started, in the time of the signal.
  double time;
  // The frequency of the signal in Hz.
  double freq_hz;
  // The text of a decoded message: the ping for people.
  sdrkit_string text;
  // What an event is about (aircraft, vessel, pager). Outputs and the store filter on it. A value of `sdrkit_mscatter_ms_ping_event_family`.
  int32_t family;
  // UTC of a ping. Seconds since 1970 (UTC).
  double utc;
  // The length of a ping. A length of time in seconds.
  double duration;
  // The peak level over the background. Unit: dB.
  double peak_snr_db;
  // The Doppler shift at the peak. A frequency in Hz.
  double doppler;
  // Underdense or overdense. A value of `sdrkit_mscatter_ms_ping_event_kind`.
  int32_t kind;
} sdrkit_mscatter_ms_ping_event;

// The storm phase changed to approaching, overhead or receding.
typedef struct sdrkit_sferics_storm_event {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Seconds since the object started, in the time of the signal.
  double time;
  // The frequency of the signal in Hz.
  double freq_hz;
  // The storm phase.
  sdrkit_string storm_phase;
  // The change of the median peak. Unit: dB.
  double storm_db_change;
  // Strikes per minute in the last 5 minutes. Unit: 1/min.
  double per_minute;
} sdrkit_sferics_storm_event;

// An event `sid.event`.
typedef struct sdrkit_supersid_sid_event {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Seconds since the object started, in the time of the signal.
  double time;
  // The frequency of the signal in Hz.
  double freq_hz;
  // A transmitter's label.
  sdrkit_string label;
  // The UTC of an event's start. Seconds since 1970 (UTC).
  double start_utc;
  // The mean of the last 2 minutes less the baseline. Unit: dB.
  double rise_db;
  // The peak step of an event. Unit: dB.
  double peak_db;
  // The text of a decoded message.
  sdrkit_string text;
} sdrkit_supersid_sid_event;

// An event of a monitor. `text` holds its note.
typedef struct sdrkit_science_event {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Seconds since the object started, in the time of the signal.
  double time;
  // The frequency of the signal in Hz.
  double freq_hz;
  // The text of a decoded message.
  sdrkit_string text;
  // The UTC of an event. Seconds since 1970 (UTC).
  double utc;
  // The type of an event.
  sdrkit_string type;
  // The characteristic frequency of an event. A frequency in Hz.
  double frequency_hz;
  // The strength of an event over the background. Unit: dB.
  double peak_db;
  // The duration of an event. A length of time in seconds.
  double duration_s;
  // Stack-specific value 1.
  double aux1;
  // Stack-specific value 2.
  double aux2;
} sdrkit_science_event;

// A strike located (server).
typedef struct sdrkit_lightning_tdoa_lightning_strike_event {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Seconds since the object started, in the time of the signal.
  double time;
  // The frequency of the signal in Hz.
  double freq_hz;
  // The time of a strike, UTC ns. Unit: ns.
  int64_t utc_ns;
  // Latitude. Unit: °.
  double lat;
  // Longitude. Unit: °.
  double lon;
  // The semi-major axis of the 95 % ellipse. Unit: km.
  double major_km;
  // The semi-minor axis. Unit: km.
  double minor_km;
  // The azimuth of the major axis. Unit: °.
  double az_deg;
  // Stations used.
  int64_t stations;
  // Coarse stations that heard it.
  int64_t also_heard;
  // The rms of the residuals. Unit: us.
  double rms_us;
  // Chi-square.
  double chi2;
  // Degrees of freedom.
  int64_t dof;
  // No degree of freedom checks it.
  bool unchecked;
  // A second solution fits as well.
  bool ambiguous;
  // The time is UTC.
  bool utc;
} sdrkit_lightning_tdoa_lightning_strike_event;

// The start (`phase` `start`, `depth_db` the absorption then) or the end (`end`, the largest absorption, the duration) of an absorption event.
typedef struct sdrkit_riometer_absorption_event {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Seconds since the object started, in the time of the signal.
  double time;
  // The frequency of the signal in Hz.
  double freq_hz;
  // The text of a decoded message.
  sdrkit_string text;
  // Start or end of an event.
  sdrkit_string phase;
  // The depth of an event. Unit: dB.
  double depth_db;
  // The time from the start of an event. A length of time in seconds.
  double duration_s;
} sdrkit_riometer_absorption_event;

// A search that detected the pulsar.
typedef struct sdrkit_pulsar_detection_event {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Seconds since the object started, in the time of the signal.
  double time;
  // The frequency of the signal in Hz.
  double freq_hz;
  // The text of a decoded message: the summary of the detection.
  sdrkit_string text;
  // Whether a detection was made.
  bool ok;
  // The best S/N.
  double snr;
  // The best period. Unit: s.
  double period_found_s;
  // The best DM. Unit: pc cm⁻³.
  double dm_found;
  // The pulse phase.
  double peak_phase;
  // The duty cycle.
  double duty;
  // The time observed. A length of time in seconds.
  double observed_s;
} sdrkit_pulsar_detection_event;

// One row of `radar_detect`. `text` holds a summary.
typedef struct sdrkit_radar_detect_radar_row_event {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Seconds since the object started, in the time of the signal.
  double time;
  // The frequency of the signal in Hz.
  double freq_hz;
  // The text of a decoded message: the summary of the row.
  sdrkit_string text;
  // The kind of a row: `rotation` or `sweep`.
  sdrkit_string type;
  // True if `rotation_s` holds a value.
  bool has_rotation_s;
  // The rotation period. Unit: s.
  double rotation_s;
  // True if `rpm` holds a value.
  bool has_rpm;
  // Rotations per minute. Unit: rpm.
  double rpm;
  // The repetition text.
  sdrkit_string prf;
  // True if `pulse_width_us` holds a value.
  bool has_pulse_width_us;
  // The median pulse width. Unit: us.
  double pulse_width_us;
  // True if `beamwidth_deg` holds a value.
  bool has_beamwidth_deg;
  // The beamwidth. Unit: °.
  double beamwidth_deg;
  // The peak of the crossing of a row. Unit: dB.
  double peak_db;
  // True if `volume` holds a value.
  bool has_volume;
  // The volume of a weather row.
  int64_t volume;
  // True if `sweep` holds a value.
  bool has_sweep;
  // The sweep of a weather row in its volume.
  int64_t sweep;
} sdrkit_radar_detect_radar_row_event;

// The options of `family` of `sdrkit_horus_packet_event`.
typedef enum sdrkit_horus_packet_event_family {
  // Aircraft
  SDRKIT_HORUS_PACKET_EVENT_FAMILY_AIRCRAFT = 0,
  // Vessel
  SDRKIT_HORUS_PACKET_EVENT_FAMILY_VESSEL = 1,
  // APRS
  SDRKIT_HORUS_PACKET_EVENT_FAMILY_APRS = 2,
  // Pager
  SDRKIT_HORUS_PACKET_EVENT_FAMILY_PAGER = 3,
  // Radiosonde
  SDRKIT_HORUS_PACKET_EVENT_FAMILY_SONDE = 4,
  // Broadcast
  SDRKIT_HORUS_PACKET_EVENT_FAMILY_BROADCAST = 5,
  // Voice
  SDRKIT_HORUS_PACKET_EVENT_FAMILY_VOICE = 6,
  // Message
  SDRKIT_HORUS_PACKET_EVENT_FAMILY_MESSAGE = 7,
  // Sensor
  SDRKIT_HORUS_PACKET_EVENT_FAMILY_SENSOR = 8,
  // Satellite
  SDRKIT_HORUS_PACKET_EVENT_FAMILY_SATELLITE = 9,
  // Receiver
  SDRKIT_HORUS_PACKET_EVENT_FAMILY_RECEIVER = 10,
  // Alert
  SDRKIT_HORUS_PACKET_EVENT_FAMILY_ALERT = 11,
  // Other
  SDRKIT_HORUS_PACKET_EVENT_FAMILY_OTHER = 12,
} sdrkit_horus_packet_event_family;

// The options of `type` of `sdrkit_horus_packet_event`.
typedef enum sdrkit_horus_packet_event_type {
  // Horus Binary v1
  SDRKIT_HORUS_PACKET_EVENT_TYPE_V1 = 0,
  // Horus Binary v2
  SDRKIT_HORUS_PACKET_EVENT_TYPE_V2 = 1,
  // RTTY
  SDRKIT_HORUS_PACKET_EVENT_TYPE_RTTY = 2,
} sdrkit_horus_packet_event_type;

// A Horus packet. `source` is the payload, `text` the summary "Horus CALL #seq lat lon alt m".
typedef struct sdrkit_horus_packet_event {
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
  // What an event is about (aircraft, vessel, pager). Outputs and the store filter on it. A value of `sdrkit_horus_packet_event_family`.
  int32_t family;
  // True if `position` holds a value.
  bool has_position;
  // The position of the object that the event is about.
  sdrkit_position position;
  // True if `height_m` holds a value.
  bool has_height_m;
  // The height of the object that the event is about, metres above mean sea level. Unit: m.
  double height_m;
  // The kind of the packet. A value of `sdrkit_horus_packet_event_type`.
  int32_t type;
  // The sequence number.
  int64_t seq;
  // True if `climb_m_s` holds a value.
  bool has_climb_m_s;
  // The climb rate. Unit: m/s.
  double climb_m_s;
  // True if `speed_kmh` holds a value.
  bool has_speed_kmh;
  // The speed of a binary packet. Unit: km/h.
  double speed_kmh;
  // True if `sats` holds a value.
  bool has_sats;
  // The satellites of a binary packet.
  int64_t sats;
  // True if `temp_c` holds a value.
  bool has_temp_c;
  // The temperature of a binary packet. Unit: °C.
  double temp_c;
  // True if `battery_v` holds a value.
  bool has_battery_v;
  // The battery of a binary packet. Unit: V.
  double battery_v;
  // Signal to noise ratio of what is being decoded, dB (in the signal's own bandwidth, or per symbol where the stack says so). Unit: dB.
  double snr_db;
  // The middle of the tones, from the dial. A frequency in Hz.
  double offset;
  // The channel that received it (0 is the dial).
  int64_t channel;
  // The packet as a sentence.
  sdrkit_string sentence;
} sdrkit_horus_packet_event;

// The span search opened a channel on a signal. `text` is "a signal +x.x kHz from the dial".
typedef struct sdrkit_horus_channel_opened_event {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Seconds since the object started, in the time of the signal.
  double time;
  // The frequency of the signal in Hz.
  double freq_hz;
  // The text of a decoded message.
  sdrkit_string text;
  // The channel number.
  int64_t channel;
  // The centre of the channel, from the dial. A frequency in Hz.
  double offset;
} sdrkit_horus_channel_opened_event;

// A channel that the span search opened closed: it decoded nothing for 120 s and its signal was gone for 30 s.
typedef struct sdrkit_horus_channel_closed_event {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Seconds since the object started, in the time of the signal.
  double time;
  // The frequency of the signal in Hz.
  double freq_hz;
  // The text of a decoded message.
  sdrkit_string text;
  // The channel number.
  int64_t channel;
  // The centre of the channel, from the dial. A frequency in Hz.
  double offset;
} sdrkit_horus_channel_closed_event;

// The arguments of `remove` and `select` of `sonde`.
typedef struct sdrkit_sonde_row_args {
  // The size of this struct: sizeof of the type. The caller sets it.
  uint32_t size;
  // The row key of the sonde.
  sdrkit_string row;
} sdrkit_sonde_row_args;

// An arguments struct with its size set and every argument at 0.
#define SDRKIT_SONDE_ROW_ARGS_INIT {sizeof(sdrkit_sonde_row_args), {0}}

// The arguments of `capture` of `astro`.
typedef struct sdrkit_astro_capture_args {
  // The size of this struct: sizeof of the type. The caller sets it.
  uint32_t size;
  // What a capture keeps. The index of an option.
  int32_t what;
} sdrkit_astro_capture_args;

// An arguments struct with its size set and every argument at 0.
#define SDRKIT_ASTRO_CAPTURE_ARGS_INIT {sizeof(sdrkit_astro_capture_args), 0}

// The arguments of `select` of `iono`.
typedef struct sdrkit_iono_select_args {
  // The size of this struct: sizeof of the type. The caller sets it.
  uint32_t size;
  // The row key of the sounder.
  sdrkit_string row;
} sdrkit_iono_select_args;

// An arguments struct with its size set and every argument at 0.
#define SDRKIT_IONO_SELECT_ARGS_INIT {sizeof(sdrkit_iono_select_args), {0}}

// The arguments of `datagram` of `lightning_tdoa`.
typedef struct sdrkit_lightning_tdoa_datagram_args {
  // The size of this struct: sizeof of the type. The caller sets it.
  uint32_t size;
  // The bytes of a datagram.
  sdrkit_bytes bytes;
} sdrkit_lightning_tdoa_datagram_args;

// An arguments struct with its size set and every argument at 0.
#define SDRKIT_LIGHTNING_TDOA_DATAGRAM_ARGS_INIT {sizeof(sdrkit_lightning_tdoa_datagram_args), {0}}

// The arguments of `time_mark` of `lightning_tdoa`.
typedef struct sdrkit_lightning_tdoa_time_mark_args {
  // The size of this struct: sizeof of the type. The caller sets it.
  uint32_t size;
  // The source of a time mark.
  sdrkit_string source;
  // The input sample of a time mark.
  int64_t sample;
  // The UTC of a time mark. Unit: ns.
  int64_t utc_ns;
  // True if `sigma_ns` holds a value.
  bool has_sigma_ns;
  // The 1-sigma of a GNSS mark. Unit: ns.
  double sigma_ns;
  // True if `bias_ns` holds a value.
  bool has_bias_ns;
  // The stream-alignment error of a GNSS mark. Unit: ns.
  double bias_ns;
} sdrkit_lightning_tdoa_time_mark_args;

// An arguments struct with its size set and every argument at 0.
#define SDRKIT_LIGHTNING_TDOA_TIME_MARK_ARGS_INIT {sizeof(sdrkit_lightning_tdoa_time_mark_args), {0}, 0, 0, 0, 0, 0, 0}

// The arguments of `import_qdc`.
typedef struct sdrkit_riometer_qdc_args {
  // The size of this struct: sizeof of the type. The caller sets it.
  uint32_t size;
  // The curve text.
  sdrkit_string text;
} sdrkit_riometer_qdc_args;

// An arguments struct with its size set and every argument at 0.
#define SDRKIT_RIOMETER_QDC_ARGS_INIT {sizeof(sdrkit_riometer_qdc_args), {0}}

// Fills `rows` with the changed rows of the table "sondes" of the stack "sonde" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The column `serial` names a row.
// Place of a row: the column `position`.
// Height: `height_m`. Course: `course_deg`. Speed: `speed_kt`.
static inline int32_t sdrkit_sonde_sondes_rows(SDRKIT_HANDLE handle,
                                              sdrkit_sonde_row *rows,
                                              uint32_t capacity,
                                              sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "science.SondeRow", "sondes", rows, sizeof(sdrkit_sonde_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "carriers" of the stack "sonde" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// Default order: the column `level_db`, highest first.
static inline int32_t sdrkit_sonde_carriers_rows(SDRKIT_HANDLE handle,
                                                sdrkit_sonde_carrier_row *rows,
                                                uint32_t capacity,
                                                sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "science.SondeCarrierRow", "carriers", rows, sizeof(sdrkit_sonde_carrier_row), capacity, info);
}

// Fills `event` with event `n` of the last sdrkit_events_take() on `handle`, an event of the type "sonde.frame"
// (Radiosonde frame). Returns SDRKIT_E_WRONG_TYPE if the event is of another type.
static inline int32_t sdrkit_sonde_frame_event_read(SDRKIT_HANDLE handle, uint32_t n, sdrkit_sonde_frame_event *event) {
  return sdrkit_typed_event(handle, n, "sonde.frame", "science.SondeFrameEvent", event, sizeof(sdrkit_sonde_frame_event));
}

// Runs the command "clear" (Clear) of the stack "sonde" on the session of `handle`. Forgets everything heard so far. Favourites stay.
static inline int32_t sdrkit_sonde_clear(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "clear", NULL, NULL, NULL, NULL, 0);
}

// Runs the command "remove" (Remove) of the stack "sonde" on the session of `handle`. Forgets one object, a favourite too.
static inline int32_t sdrkit_sonde_remove(SDRKIT_HANDLE handle, const sdrkit_sonde_row_args *args) {
  return sdrkit_typed_command(handle, "remove", "science.SondeRowArgs", args, NULL, NULL, 0);
}

// Runs the command "select" (Select) of the stack "sonde" on the session of `handle`. Shows the plots of one sonde.
static inline int32_t sdrkit_sonde_select(SDRKIT_HANDLE handle, const sdrkit_sonde_row_args *args) {
  return sdrkit_typed_command(handle, "select", "science.SondeRowArgs", args, NULL, NULL, 0);
}

// The settings of the test signal generator of the stack "sonde" are the sdrkit_sonde_gen_*() functions
// after the settings of its receiver.

// The setting "channels" (Sondes followed at once) of the stack "sonde". Each channel costs processing time.
static inline int32_t sdrkit_sonde_get_channels(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "channels", value);
}
static inline int32_t sdrkit_sonde_set_channels(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "channels", value);
}

// The setting "landing_window_s" (Landing smoothing) of the stack "sonde". A length of time in seconds.
static inline int32_t sdrkit_sonde_get_landing_window_s(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "landing_window_s", value);
}
static inline int32_t sdrkit_sonde_set_landing_window_s(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "landing_window_s", value);
}

// The setting "landing_layers" (Use the ascent winds) of the stack "sonde".
static inline int32_t sdrkit_sonde_get_landing_layers(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "landing_layers", value);
}
static inline int32_t sdrkit_sonde_set_landing_layers(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "landing_layers", value);
}

// The setting "landing_layer_m" (Layer) of the stack "sonde". Unit: m.
static inline int32_t sdrkit_sonde_get_landing_layer_m(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "landing_layer_m", value);
}
static inline int32_t sdrkit_sonde_set_landing_layer_m(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "landing_layer_m", value);
}

// The setting "landing_ascent_weight" (Ascent wind weight) of the stack "sonde".
static inline int32_t sdrkit_sonde_get_landing_ascent_weight(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "landing_ascent_weight", value);
}
static inline int32_t sdrkit_sonde_set_landing_ascent_weight(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "landing_ascent_weight", value);
}

// The setting "ground_m" (Ground height) of the stack "sonde". Unit: m.
static inline int32_t sdrkit_sonde_get_ground_m(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "ground_m", value);
}
static inline int32_t sdrkit_sonde_set_ground_m(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "ground_m", value);
}

// The setting "gen_count" (Test signal: sondes) of the test signal generator of the stack "sonde".
static inline int32_t sdrkit_sonde_gen_get_count(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "gen_count", value);
}
static inline int32_t sdrkit_sonde_gen_set_count(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "gen_count", value);
}

// The setting "gen_types" (Test signal: types) of the test signal generator of the stack "sonde". The keys of the sonde families, separated by commas (rs41, rs92, dfm, m10, m20, lms6).
static inline int32_t sdrkit_sonde_gen_get_types(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "gen_types", value);
}
static inline int32_t sdrkit_sonde_gen_set_types(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "gen_types", value);
}

// The setting "gen_drift_ms" (Test signal: east drift) of the test signal generator of the stack "sonde". Unit: m/s.
static inline int32_t sdrkit_sonde_gen_get_drift_ms(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_drift_ms", value);
}
static inline int32_t sdrkit_sonde_gen_set_drift_ms(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_drift_ms", value);
}

// The setting "gen_rs41b_khz" (Test signal: second RS41 offset) of the test signal generator of the stack "sonde". Unit: kHz.
static inline int32_t sdrkit_sonde_gen_get_rs41b_khz(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_rs41b_khz", value);
}
static inline int32_t sdrkit_sonde_gen_set_rs41b_khz(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_rs41b_khz", value);
}

// The setting "gen_lband" (Test signal: 1680 MHz (LMS6)) of the test signal generator of the stack "sonde".
static inline int32_t sdrkit_sonde_gen_get_lband(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "gen_lband", value);
}
static inline int32_t sdrkit_sonde_gen_set_lband(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "gen_lband", value);
}

// Fills `rows` with the changed rows of the table "minutes" of the stack "time" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The rows are a log: they arrive over time.
// Time: `host_time`.
static inline int32_t sdrkit_time_minutes_rows(SDRKIT_HANDLE handle,
                                              sdrkit_time_minute_row *rows,
                                              uint32_t capacity,
                                              sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "science.TimeMinuteRow", "minutes", rows, sizeof(sdrkit_time_minute_row), capacity, info);
}

// Fills `event` with event `n` of the last sdrkit_events_take() on `handle`, an event of the type "time.minute"
// (Time signal minute). Returns SDRKIT_E_WRONG_TYPE if the event is of another type.
static inline int32_t sdrkit_time_minute_event_read(SDRKIT_HANDLE handle, uint32_t n, sdrkit_time_minute_event *event) {
  return sdrkit_typed_event(handle, n, "time.minute", "science.TimeMinuteEvent", event, sizeof(sdrkit_time_minute_event));
}

// Runs the command "clear" (Clear) of the stack "time" on the session of `handle`. Forgets everything heard so far. Favourites stay.
static inline int32_t sdrkit_time_clear(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "clear", NULL, NULL, NULL, NULL, 0);
}

// The settings of the test signal generator of the stack "time" are the sdrkit_time_gen_*() functions
// after the settings of its receiver.

// The setting "station" (Station) of the stack "time". A value of `sdrkit_time_station`.
static inline int32_t sdrkit_time_get_station(SDRKIT_HANDLE handle, sdrkit_time_station *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "station", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_time_station)index;
  return code;
}
static inline int32_t sdrkit_time_set_station(SDRKIT_HANDLE handle, sdrkit_time_station value) {
  return sdrkit_typed_setting_set_enum(handle, "station", (int32_t)value);
}

// The setting "sixty_khz" (60 kHz is) of the stack "time". A value of `sdrkit_time_sixty_khz`.
static inline int32_t sdrkit_time_get_sixty_khz(SDRKIT_HANDLE handle, sdrkit_time_sixty_khz *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "sixty_khz", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_time_sixty_khz)index;
  return code;
}
static inline int32_t sdrkit_time_set_sixty_khz(SDRKIT_HANDLE handle, sdrkit_time_sixty_khz value) {
  return sdrkit_typed_setting_set_enum(handle, "sixty_khz", (int32_t)value);
}

// The setting "audio_input" (Demodulated audio recording) of the stack "time".
static inline int32_t sdrkit_time_get_audio_input(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "audio_input", value);
}
static inline int32_t sdrkit_time_set_audio_input(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "audio_input", value);
}

// The setting "audio_tone_hz" (Carrier tone of an LF recording) of the stack "time". A frequency in Hz.
static inline int32_t sdrkit_time_get_audio_tone_hz(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "audio_tone_hz", value);
}
static inline int32_t sdrkit_time_set_audio_tone_hz(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "audio_tone_hz", value);
}

// The setting "clock_slow_pct" (Recording clock slow by) of the stack "time". Unit: %.
static inline int32_t sdrkit_time_get_clock_slow_pct(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "clock_slow_pct", value);
}
static inline int32_t sdrkit_time_set_clock_slow_pct(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "clock_slow_pct", value);
}

// The setting "listen" (Listen) of the stack "time".
static inline int32_t sdrkit_time_get_listen(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "listen", value);
}
static inline int32_t sdrkit_time_set_listen(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "listen", value);
}

// The setting "clock_mode" (Clock to compare) of the stack "time". A value of `sdrkit_time_clock_mode`.
static inline int32_t sdrkit_time_get_clock_mode(SDRKIT_HANDLE handle, sdrkit_time_clock_mode *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "clock_mode", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_time_clock_mode)index;
  return code;
}
static inline int32_t sdrkit_time_set_clock_mode(SDRKIT_HANDLE handle, sdrkit_time_clock_mode value) {
  return sdrkit_typed_setting_set_enum(handle, "clock_mode", (int32_t)value);
}

// The setting "clock_origin" (Sample 0 at (UTC, Unix s)) of the stack "time". Unit: s.
static inline int32_t sdrkit_time_get_clock_origin(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "clock_origin", value);
}
static inline int32_t sdrkit_time_set_clock_origin(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "clock_origin", value);
}

// The setting "year_hint" (Year hint) of the stack "time".
static inline int32_t sdrkit_time_get_year_hint(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "year_hint", value);
}
static inline int32_t sdrkit_time_set_year_hint(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "year_hint", value);
}

// The setting "gen_start_utc" (Test signal: start (UTC, 0 = host clock)) of the test signal generator of the stack "time". Unit: s.
static inline int32_t sdrkit_time_gen_get_start_utc(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_start_utc", value);
}
static inline int32_t sdrkit_time_gen_set_start_utc(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_start_utc", value);
}

// Fills `rows` with the changed rows of the table "frames" of the stack "eczas" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The rows are a log: they arrive over time.
// Time: `host_time`.
static inline int32_t sdrkit_eczas_frames_rows(SDRKIT_HANDLE handle,
                                              sdrkit_eczas_frame_row *rows,
                                              uint32_t capacity,
                                              sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "science.EczasFrameRow", "frames", rows, sizeof(sdrkit_eczas_frame_row), capacity, info);
}

// Fills `event` with event `n` of the last sdrkit_events_take() on `handle`, an event of the type "eczas.time"
// (e-Czas time). Returns SDRKIT_E_WRONG_TYPE if the event is of another type.
static inline int32_t sdrkit_eczas_time_event_read(SDRKIT_HANDLE handle, uint32_t n, sdrkit_eczas_time_event *event) {
  return sdrkit_typed_event(handle, n, "eczas.time", "science.EczasTimeEvent", event, sizeof(sdrkit_eczas_time_event));
}

// Runs the command "clear" (Clear) of the stack "eczas" on the session of `handle`. Forgets everything heard so far. Favourites stay.
static inline int32_t sdrkit_eczas_clear(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "clear", NULL, NULL, NULL, NULL, 0);
}

// The settings of the test signal generator of the stack "eczas" are the sdrkit_eczas_gen_*() functions
// after the settings of its receiver.

// The setting "audio_input" (Audio recording) of the stack "eczas".
static inline int32_t sdrkit_eczas_get_audio_input(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "audio_input", value);
}
static inline int32_t sdrkit_eczas_set_audio_input(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "audio_input", value);
}

// The setting "audio_tone_hz" (Carrier tone in the audio) of the stack "eczas". A frequency in Hz.
static inline int32_t sdrkit_eczas_get_audio_tone_hz(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "audio_tone_hz", value);
}
static inline int32_t sdrkit_eczas_set_audio_tone_hz(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "audio_tone_hz", value);
}

// The setting "listen" (Listen) of the stack "eczas".
static inline int32_t sdrkit_eczas_get_listen(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "listen", value);
}
static inline int32_t sdrkit_eczas_set_listen(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "listen", value);
}

// The setting "clock_mode" (Clock to compare) of the stack "eczas". A value of `sdrkit_eczas_clock_mode`.
static inline int32_t sdrkit_eczas_get_clock_mode(SDRKIT_HANDLE handle, sdrkit_eczas_clock_mode *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "clock_mode", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_eczas_clock_mode)index;
  return code;
}
static inline int32_t sdrkit_eczas_set_clock_mode(SDRKIT_HANDLE handle, sdrkit_eczas_clock_mode value) {
  return sdrkit_typed_setting_set_enum(handle, "clock_mode", (int32_t)value);
}

// The setting "clock_origin" (Sample 0 at (UTC, Unix s)) of the stack "eczas". Unit: s.
static inline int32_t sdrkit_eczas_get_clock_origin(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "clock_origin", value);
}
static inline int32_t sdrkit_eczas_set_clock_origin(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "clock_origin", value);
}

// The setting "gen_start_utc" (Test signal: start (UTC, 0 = host clock)) of the test signal generator of the stack "eczas". Unit: s.
static inline int32_t sdrkit_eczas_gen_get_start_utc(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_start_utc", value);
}
static inline int32_t sdrkit_eczas_gen_set_start_utc(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_start_utc", value);
}

// The setting "gen_carrier_only" (Test signal: carrier only) of the test signal generator of the stack "eczas".
static inline int32_t sdrkit_eczas_gen_get_carrier_only(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "gen_carrier_only", value);
}
static inline int32_t sdrkit_eczas_gen_set_carrier_only(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "gen_carrier_only", value);
}

// Fills `rows` with the changed rows of the table "alerts" of the stack "same" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The rows are the messages of the stack.
// The column `callsign` names a row.
// Sender: `callsign`. Text: `event_name`. Time: `host_time`.
static inline int32_t sdrkit_same_alerts_rows(SDRKIT_HANDLE handle,
                                             sdrkit_same_alert_row *rows,
                                             uint32_t capacity,
                                             sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "science.SameAlertRow", "alerts", rows, sizeof(sdrkit_same_alert_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "alert_locations" of the stack "same" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The column `place` names a row.
// Place of a row: the column `position`.
static inline int32_t sdrkit_same_alert_locations_rows(SDRKIT_HANDLE handle,
                                                      sdrkit_same_alert_location_row *rows,
                                                      uint32_t capacity,
                                                      sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "science.SameAlertLocationRow", "alert_locations", rows, sizeof(sdrkit_same_alert_location_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "tones" of the stack "same" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The rows are a log: they arrive over time.
// Time: `signal_time_s`.
static inline int32_t sdrkit_same_tones_rows(SDRKIT_HANDLE handle,
                                            sdrkit_same_tone_row *rows,
                                            uint32_t capacity,
                                            sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "science.SameToneRow", "tones", rows, sizeof(sdrkit_same_tone_row), capacity, info);
}

// Fills `event` with event `n` of the last sdrkit_events_take() on `handle`, an event of the type "same.alert"
// (SAME alert). Returns SDRKIT_E_WRONG_TYPE if the event is of another type.
static inline int32_t sdrkit_same_alert_event_read(SDRKIT_HANDLE handle, uint32_t n, sdrkit_same_alert_event *event) {
  return sdrkit_typed_event(handle, n, "same.alert", "science.SameAlertEvent", event, sizeof(sdrkit_same_alert_event));
}

// Fills `event` with event `n` of the last sdrkit_events_take() on `handle`, an event of the type "same.end"
// (SAME end of message). Returns SDRKIT_E_WRONG_TYPE if the event is of another type.
static inline int32_t sdrkit_same_end_event_read(SDRKIT_HANDLE handle, uint32_t n, sdrkit_same_end_event *event) {
  return sdrkit_typed_event(handle, n, "same.end", "science.SameEndEvent", event, sizeof(sdrkit_same_end_event));
}

// Runs the command "clear" (Clear) of the stack "same" on the session of `handle`. Forgets everything heard so far. Favourites stay.
static inline int32_t sdrkit_same_clear(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "clear", NULL, NULL, NULL, NULL, 0);
}

// Runs the command "clear_counties" (Clear the county table) of the stack "same" on the session of `handle`. Forgets the loaded county table.
static inline int32_t sdrkit_same_clear_counties(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "clear_counties", NULL, NULL, NULL, NULL, 0);
}

// The settings of the test signal generator of the stack "same" are the sdrkit_same_gen_*() functions
// after the settings of its receiver.

// The setting "channel" (Weather channel) of the stack "same". A value of `sdrkit_same_channel`.
static inline int32_t sdrkit_same_get_channel(SDRKIT_HANDLE handle, sdrkit_same_channel *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "channel", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_same_channel)index;
  return code;
}
static inline int32_t sdrkit_same_set_channel(SDRKIT_HANDLE handle, sdrkit_same_channel value) {
  return sdrkit_typed_setting_set_enum(handle, "channel", (int32_t)value);
}

// The setting "country" (Location names) of the stack "same". A value of `sdrkit_same_country`.
static inline int32_t sdrkit_same_get_country(SDRKIT_HANDLE handle, sdrkit_same_country *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "country", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_same_country)index;
  return code;
}
static inline int32_t sdrkit_same_set_country(SDRKIT_HANDLE handle, sdrkit_same_country value) {
  return sdrkit_typed_setting_set_enum(handle, "country", (int32_t)value);
}

// The setting "county_text" (County table) of the stack "same". The text of a county table: lines SSCCC,Name or SS,CCC,Name or the Census form. The host reads the file.
static inline int32_t sdrkit_same_get_county_text(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "county_text", value);
}
static inline int32_t sdrkit_same_set_county_text(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "county_text", value);
}

// The setting "year" (Year for issue times) of the stack "same".
static inline int32_t sdrkit_same_get_year(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "year", value);
}
static inline int32_t sdrkit_same_set_year(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "year", value);
}

// The setting "listen" (Listen) of the stack "same".
static inline int32_t sdrkit_same_get_listen(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "listen", value);
}
static inline int32_t sdrkit_same_set_listen(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "listen", value);
}

// The setting "gen_scene" (Test signal: scene) of the test signal generator of the stack "same". A value of `sdrkit_same_gen_scene`.
static inline int32_t sdrkit_same_gen_get_scene(SDRKIT_HANDLE handle, sdrkit_same_gen_scene *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "gen_scene", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_same_gen_scene)index;
  return code;
}
static inline int32_t sdrkit_same_gen_set_scene(SDRKIT_HANDLE handle, sdrkit_same_gen_scene value) {
  return sdrkit_typed_setting_set_enum(handle, "gen_scene", (int32_t)value);
}

// The setting "gen_loop" (Test signal: loop) of the test signal generator of the stack "same".
static inline int32_t sdrkit_same_gen_get_loop(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "gen_loop", value);
}
static inline int32_t sdrkit_same_gen_set_loop(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "gen_loop", value);
}

// Fills `rows` with the changed rows of the table "pings" of the stack "mscatter" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The rows are a log: they arrive over time.
// Time: `utc`.
static inline int32_t sdrkit_mscatter_pings_rows(SDRKIT_HANDLE handle,
                                                sdrkit_mscatter_ping_row *rows,
                                                uint32_t capacity,
                                                sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "science.MscatterPingRow", "pings", rows, sizeof(sdrkit_mscatter_ping_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "showers" of the stack "mscatter" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The column `name` names a row.
static inline int32_t sdrkit_mscatter_showers_rows(SDRKIT_HANDLE handle,
                                                  sdrkit_mscatter_shower_row *rows,
                                                  uint32_t capacity,
                                                  sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "science.MscatterShowerRow", "showers", rows, sizeof(sdrkit_mscatter_shower_row), capacity, info);
}

// Fills `event` with event `n` of the last sdrkit_events_take() on `handle`, an event of the type "ms.ping"
// (Meteor ping). Returns SDRKIT_E_WRONG_TYPE if the event is of another type.
static inline int32_t sdrkit_mscatter_ms_ping_event_read(SDRKIT_HANDLE handle, uint32_t n, sdrkit_mscatter_ms_ping_event *event) {
  return sdrkit_typed_event(handle, n, "ms.ping", "science.MscatterPingEvent", event, sizeof(sdrkit_mscatter_ms_ping_event));
}

// Runs the command "clear" (Clear) of the stack "mscatter" on the session of `handle`. Forgets everything heard so far. Favourites stay.
static inline int32_t sdrkit_mscatter_clear(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "clear", NULL, NULL, NULL, NULL, 0);
}

// The settings of the test signal generator of the stack "mscatter" are the sdrkit_mscatter_gen_*() functions
// after the settings of its receiver.

// The setting "threshold_db" (Detection level) of the stack "mscatter". Unit: dB.
static inline int32_t sdrkit_mscatter_get_threshold_db(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "threshold_db", value);
}
static inline int32_t sdrkit_mscatter_set_threshold_db(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "threshold_db", value);
}

// The setting "start_utc" (UTC of sample 0 (files; 0 = host clock)) of the stack "mscatter". Unit: s.
static inline int32_t sdrkit_mscatter_get_start_utc(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "start_utc", value);
}
static inline int32_t sdrkit_mscatter_set_start_utc(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "start_utc", value);
}

// The setting "gen_beacon_hz" (Test signal: beacon offset) of the test signal generator of the stack "mscatter". A frequency in Hz.
static inline int32_t sdrkit_mscatter_gen_get_beacon_hz(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_beacon_hz", value);
}
static inline int32_t sdrkit_mscatter_gen_set_beacon_hz(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_beacon_hz", value);
}

// The setting "gen_mean_gap_s" (Test signal: mean gap between pings) of the test signal generator of the stack "mscatter". A length of time in seconds.
static inline int32_t sdrkit_mscatter_gen_get_mean_gap_s(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_mean_gap_s", value);
}
static inline int32_t sdrkit_mscatter_gen_set_mean_gap_s(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_mean_gap_s", value);
}

// Fills `rows` with the changed rows of the table "packets" of the stack "ssdv" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The rows are a log: they arrive over time.
// The column `call` names a row.
// Time: `host_time`.
static inline int32_t sdrkit_ssdv_packets_rows(SDRKIT_HANDLE handle,
                                              sdrkit_ssdv_packet_row *rows,
                                              uint32_t capacity,
                                              sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "science.SsdvPacketRow", "packets", rows, sizeof(sdrkit_ssdv_packet_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "pictures" of the stack "ssdv" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The column `call` names a row.
static inline int32_t sdrkit_ssdv_pictures_rows(SDRKIT_HANDLE handle,
                                               sdrkit_ssdv_picture_row *rows,
                                               uint32_t capacity,
                                               sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "science.SsdvPictureRow", "pictures", rows, sizeof(sdrkit_ssdv_picture_row), capacity, info);
}

// Runs the command "clear" (Clear) of the stack "ssdv" on the session of `handle`. Forgets everything heard so far. Favourites stay.
static inline int32_t sdrkit_ssdv_clear(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "clear", NULL, NULL, NULL, NULL, 0);
}

// The settings of the test signal generator of the stack "ssdv" are the sdrkit_ssdv_gen_*() functions
// after the settings of its receiver.

// The setting "baud" (Speed) of the stack "ssdv". A value of `sdrkit_ssdv_baud`.
static inline int32_t sdrkit_ssdv_get_baud(SDRKIT_HANDLE handle, sdrkit_ssdv_baud *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "baud", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_ssdv_baud)index;
  return code;
}
static inline int32_t sdrkit_ssdv_set_baud(SDRKIT_HANDLE handle, sdrkit_ssdv_baud value) {
  return sdrkit_typed_setting_set_enum(handle, "baud", (int32_t)value);
}

// The setting "gen_mode" (Test signal: speed) of the test signal generator of the stack "ssdv". A value of `sdrkit_ssdv_gen_mode`.
static inline int32_t sdrkit_ssdv_gen_get_mode(SDRKIT_HANDLE handle, sdrkit_ssdv_gen_mode *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "gen_mode", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_ssdv_gen_mode)index;
  return code;
}
static inline int32_t sdrkit_ssdv_gen_set_mode(SDRKIT_HANDLE handle, sdrkit_ssdv_gen_mode value) {
  return sdrkit_typed_setting_set_enum(handle, "gen_mode", (int32_t)value);
}

// Fills `rows` with the changed rows of the table "calibrated" of the stack "astro" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// Default order: the column `freq`, lowest first.
static inline int32_t sdrkit_astro_calibrated_rows(SDRKIT_HANDLE handle,
                                                  sdrkit_astro_calibrated_row *rows,
                                                  uint32_t capacity,
                                                  sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "science.AstroCalibratedRow", "calibrated", rows, sizeof(sdrkit_astro_calibrated_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "session" of the stack "astro" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The rows are a log: they arrive over time.
// Time: `signal_time_s`.
static inline int32_t sdrkit_astro_session_rows(SDRKIT_HANDLE handle,
                                               sdrkit_astro_session_row *rows,
                                               uint32_t capacity,
                                               sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "science.AstroSessionRow", "session", rows, sizeof(sdrkit_astro_session_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "fit" of the stack "astro" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_astro_fit_rows(SDRKIT_HANDLE handle,
                                           sdrkit_astro_fit_row *rows,
                                           uint32_t capacity,
                                           sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "science.AstroFitRow", "fit", rows, sizeof(sdrkit_astro_fit_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "lv_rows" of the stack "astro" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// Default order: the column `longitude`, lowest first.
static inline int32_t sdrkit_astro_lv_rows(SDRKIT_HANDLE handle,
                                          sdrkit_astro_map_row *rows,
                                          uint32_t capacity,
                                          sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "science.AstroMapRow", "lv_rows", rows, sizeof(sdrkit_astro_map_row), capacity, info);
}

// Runs the command "clear" (Clear) of the stack "astro" on the session of `handle`. Forgets everything heard so far. Favourites stay.
static inline int32_t sdrkit_astro_clear(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "clear", NULL, NULL, NULL, NULL, 0);
}

// Runs the command "capture" (Capture) of the stack "astro" on the session of `handle`. Keeps the next finished integration as the off-source spectrum, or its power as the hot or the cold load.
static inline int32_t sdrkit_astro_capture(SDRKIT_HANDLE handle, const sdrkit_astro_capture_args *args) {
  return sdrkit_typed_command(handle, "capture", "science.AstroCaptureArgs", args, NULL, NULL, 0);
}

// Runs the command "flag_rfi" (Flag RFI) of the stack "astro" on the session of `handle`.
static inline int32_t sdrkit_astro_flag_rfi(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "flag_rfi", NULL, NULL, NULL, NULL, 0);
}

// Runs the command "fit_lines" (Fit lines) of the stack "astro" on the session of `handle`.
static inline int32_t sdrkit_astro_fit_lines(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "fit_lines", NULL, NULL, NULL, NULL, 0);
}

// Runs the command "add_map" (Add to the l-v map) of the stack "astro" on the session of `handle`.
static inline int32_t sdrkit_astro_add_map(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "add_map", NULL, NULL, NULL, NULL, 0);
}

// Runs the command "clear_map" (Clear the map) of the stack "astro" on the session of `handle`.
static inline int32_t sdrkit_astro_clear_map(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "clear_map", NULL, NULL, NULL, NULL, 0);
}

// Runs the command "restart" (Restart the integration) of the stack "astro" on the session of `handle`.
static inline int32_t sdrkit_astro_restart(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "restart", NULL, NULL, NULL, NULL, 0);
}

// The settings of the test signal generator of the stack "astro" are the sdrkit_astro_gen_*() functions
// after the settings of its receiver.

// The setting "fft" (FFT size) of the stack "astro". A value of `sdrkit_astro_fft`.
static inline int32_t sdrkit_astro_get_fft(SDRKIT_HANDLE handle, sdrkit_astro_fft *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "fft", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_astro_fft)index;
  return code;
}
static inline int32_t sdrkit_astro_set_fft(SDRKIT_HANDLE handle, sdrkit_astro_fft value) {
  return sdrkit_typed_setting_set_enum(handle, "fft", (int32_t)value);
}

// The setting "integration_s" (Integration) of the stack "astro". A length of time in seconds.
static inline int32_t sdrkit_astro_get_integration_s(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "integration_s", value);
}
static inline int32_t sdrkit_astro_set_integration_s(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "integration_s", value);
}

// The setting "line" (Rest line) of the stack "astro". A value of `sdrkit_astro_line`.
static inline int32_t sdrkit_astro_get_line(SDRKIT_HANDLE handle, sdrkit_astro_line *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "line", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_astro_line)index;
  return code;
}
static inline int32_t sdrkit_astro_set_line(SDRKIT_HANDLE handle, sdrkit_astro_line value) {
  return sdrkit_typed_setting_set_enum(handle, "line", (int32_t)value);
}

// The setting "rest_hz" (Custom rest frequency) of the stack "astro". A frequency in Hz.
static inline int32_t sdrkit_astro_get_rest_hz(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "rest_hz", value);
}
static inline int32_t sdrkit_astro_set_rest_hz(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "rest_hz", value);
}

// The setting "band_shape" (Band shape from) of the stack "astro". A value of `sdrkit_astro_band_shape`.
static inline int32_t sdrkit_astro_get_band_shape(SDRKIT_HANDLE handle, sdrkit_astro_band_shape *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "band_shape", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_astro_band_shape)index;
  return code;
}
static inline int32_t sdrkit_astro_set_band_shape(SDRKIT_HANDLE handle, sdrkit_astro_band_shape value) {
  return sdrkit_typed_setting_set_enum(handle, "band_shape", (int32_t)value);
}

// The setting "fold" (Fold) of the stack "astro".
static inline int32_t sdrkit_astro_get_fold(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "fold", value);
}
static inline int32_t sdrkit_astro_set_fold(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "fold", value);
}

// The setting "dc_bins" (DC bridge) of the stack "astro". Unit: bins.
static inline int32_t sdrkit_astro_get_dc_bins(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "dc_bins", value);
}
static inline int32_t sdrkit_astro_set_dc_bins(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "dc_bins", value);
}

// The setting "baseline_degree" (Baseline degree) of the stack "astro".
static inline int32_t sdrkit_astro_get_baseline_degree(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "baseline_degree", value);
}
static inline int32_t sdrkit_astro_set_baseline_degree(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "baseline_degree", value);
}

// The setting "window" (Line window) of the stack "astro".
static inline int32_t sdrkit_astro_get_window(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "window", value);
}
static inline int32_t sdrkit_astro_set_window(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "window", value);
}

// The setting "window_lo_kms" (Window from) of the stack "astro". Unit: km/s.
static inline int32_t sdrkit_astro_get_window_lo_kms(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "window_lo_kms", value);
}
static inline int32_t sdrkit_astro_set_window_lo_kms(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "window_lo_kms", value);
}

// The setting "window_hi_kms" (Window to) of the stack "astro". Unit: km/s.
static inline int32_t sdrkit_astro_get_window_hi_kms(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "window_hi_kms", value);
}
static inline int32_t sdrkit_astro_set_window_hi_kms(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "window_hi_kms", value);
}

// The setting "edge" (Edges left out) of the stack "astro".
static inline int32_t sdrkit_astro_get_edge(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "edge", value);
}
static inline int32_t sdrkit_astro_set_edge(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "edge", value);
}

// The setting "smooth_bins" (Smoothing) of the stack "astro". Unit: bins.
static inline int32_t sdrkit_astro_get_smooth_bins(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "smooth_bins", value);
}
static inline int32_t sdrkit_astro_set_smooth_bins(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "smooth_bins", value);
}

// The setting "tsys_k" (System temperature) of the stack "astro". Unit: K.
static inline int32_t sdrkit_astro_get_tsys_k(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "tsys_k", value);
}
static inline int32_t sdrkit_astro_set_tsys_k(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "tsys_k", value);
}

// The setting "lsr" (LSR) of the stack "astro". A value of `sdrkit_astro_lsr`.
static inline int32_t sdrkit_astro_get_lsr(SDRKIT_HANDLE handle, sdrkit_astro_lsr *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "lsr", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_astro_lsr)index;
  return code;
}
static inline int32_t sdrkit_astro_set_lsr(SDRKIT_HANDLE handle, sdrkit_astro_lsr value) {
  return sdrkit_typed_setting_set_enum(handle, "lsr", (int32_t)value);
}

// The setting "target" (Target) of the stack "astro". A value of `sdrkit_astro_target`.
static inline int32_t sdrkit_astro_get_target(SDRKIT_HANDLE handle, sdrkit_astro_target *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "target", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_astro_target)index;
  return code;
}
static inline int32_t sdrkit_astro_set_target(SDRKIT_HANDLE handle, sdrkit_astro_target value) {
  return sdrkit_typed_setting_set_enum(handle, "target", (int32_t)value);
}

// The setting "target_l" (Target l) of the stack "astro". Unit: °.
static inline int32_t sdrkit_astro_get_target_l(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "target_l", value);
}
static inline int32_t sdrkit_astro_set_target_l(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "target_l", value);
}

// The setting "target_b" (Target b) of the stack "astro". Unit: °.
static inline int32_t sdrkit_astro_get_target_b(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "target_b", value);
}
static inline int32_t sdrkit_astro_set_target_b(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "target_b", value);
}

// The setting "target_ra" (Target RA (J2000)) of the stack "astro". Unit: °.
static inline int32_t sdrkit_astro_get_target_ra(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "target_ra", value);
}
static inline int32_t sdrkit_astro_set_target_ra(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "target_ra", value);
}

// The setting "target_dec" (Target Dec (J2000)) of the stack "astro". Unit: °.
static inline int32_t sdrkit_astro_get_target_dec(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "target_dec", value);
}
static inline int32_t sdrkit_astro_set_target_dec(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "target_dec", value);
}

// The setting "gen_off_source" (Test signal: off source (no line)) of the test signal generator of the stack "astro".
static inline int32_t sdrkit_astro_gen_get_off_source(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "gen_off_source", value);
}
static inline int32_t sdrkit_astro_gen_set_off_source(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "gen_off_source", value);
}

// The setting "gen_line_kms" (Test signal: line velocity) of the test signal generator of the stack "astro". Unit: km/s.
static inline int32_t sdrkit_astro_gen_get_line_kms(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_line_kms", value);
}
static inline int32_t sdrkit_astro_gen_set_line_kms(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_line_kms", value);
}

// Fills `rows` with the changed rows of the table "sounders" of the stack "iono" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The column `id` names a row.
static inline int32_t sdrkit_iono_sounders_rows(SDRKIT_HANDLE handle,
                                               sdrkit_iono_sounder_row *rows,
                                               uint32_t capacity,
                                               sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "science.IonoSounderRow", "sounders", rows, sizeof(sdrkit_iono_sounder_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "timeline" of the stack "iono" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The rows are a log: they arrive over time.
// The column `id` names a row.
// Time: `start_s`.
static inline int32_t sdrkit_iono_timeline_rows(SDRKIT_HANDLE handle,
                                               sdrkit_iono_timeline_row *rows,
                                               uint32_t capacity,
                                               sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "science.IonoTimelineRow", "timeline", rows, sizeof(sdrkit_iono_timeline_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "bands" of the stack "iono" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The column `band` names a row.
static inline int32_t sdrkit_iono_bands_rows(SDRKIT_HANDLE handle,
                                            sdrkit_iono_band_row *rows,
                                            uint32_t capacity,
                                            sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "science.IonoBandRow", "bands", rows, sizeof(sdrkit_iono_band_row), capacity, info);
}

// Runs the command "clear" (Clear) of the stack "iono" on the session of `handle`. Forgets everything heard so far. Favourites stay.
static inline int32_t sdrkit_iono_clear(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "clear", NULL, NULL, NULL, NULL, 0);
}

// Runs the command "select" (Show a sounder) of the stack "iono" on the session of `handle`. Shows the ionogram and the history of one sounder.
static inline int32_t sdrkit_iono_select(SDRKIT_HANDLE handle, const sdrkit_iono_select_args *args) {
  return sdrkit_typed_command(handle, "select", "science.IonoSelectArgs", args, NULL, NULL, 0);
}

// Runs the command "select_latest" (Show the latest sounder) of the stack "iono" on the session of `handle`.
static inline int32_t sdrkit_iono_select_latest(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "select_latest", NULL, NULL, NULL, NULL, 0);
}

// The settings of the test signal generator of the stack "iono" are the sdrkit_iono_gen_*() functions
// after the settings of its receiver.

// The setting "locked" (Sweeps start on whole seconds) of the stack "iono".
static inline int32_t sdrkit_iono_get_locked(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "locked", value);
}
static inline int32_t sdrkit_iono_set_locked(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "locked", value);
}

// The setting "geometry" (Geometry) of the stack "iono". A value of `sdrkit_iono_geometry`.
static inline int32_t sdrkit_iono_get_geometry(SDRKIT_HANDLE handle, sdrkit_iono_geometry *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "geometry", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_iono_geometry)index;
  return code;
}
static inline int32_t sdrkit_iono_set_geometry(SDRKIT_HANDLE handle, sdrkit_iono_geometry value) {
  return sdrkit_typed_setting_set_enum(handle, "geometry", (int32_t)value);
}

// The setting "db_top" (Colour scale top) of the stack "iono". Unit: dB.
static inline int32_t sdrkit_iono_get_db_top(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "db_top", value);
}
static inline int32_t sdrkit_iono_set_db_top(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "db_top", value);
}

// The setting "gen_centre_hz" (Test signal: the radio's centre) of the test signal generator of the stack "iono". A frequency in Hz.
static inline int32_t sdrkit_iono_gen_get_centre_hz(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_centre_hz", value);
}
static inline int32_t sdrkit_iono_gen_set_centre_hz(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_centre_hz", value);
}

// The setting "gen_noise_only" (Test signal: no sounders) of the test signal generator of the stack "iono".
static inline int32_t sdrkit_iono_gen_get_noise_only(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "gen_noise_only", value);
}
static inline int32_t sdrkit_iono_gen_set_noise_only(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "gen_noise_only", value);
}

// Fills `rows` with the changed rows of the table "strikes" of the stack "sferics" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The rows are a log: they arrive over time.
// Time: `utc`.
static inline int32_t sdrkit_sferics_strikes_rows(SDRKIT_HANDLE handle,
                                                 sdrkit_sferics_strike_row *rows,
                                                 uint32_t capacity,
                                                 sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "science.SfericsStrikeRow", "strikes", rows, sizeof(sdrkit_sferics_strike_row), capacity, info);
}

// Fills `event` with event `n` of the last sdrkit_events_take() on `handle`, an event of the type "sferics.storm"
// (Storm phase). Returns SDRKIT_E_WRONG_TYPE if the event is of another type.
static inline int32_t sdrkit_sferics_storm_event_read(SDRKIT_HANDLE handle, uint32_t n, sdrkit_sferics_storm_event *event) {
  return sdrkit_typed_event(handle, n, "sferics.storm", "science.SfericsStormEvent", event, sizeof(sdrkit_sferics_storm_event));
}

// Runs the command "clear" (Clear) of the stack "sferics" on the session of `handle`. Forgets everything decoded so far.
static inline int32_t sdrkit_sferics_clear(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "clear", NULL, NULL, NULL, NULL, 0);
}

// Runs the command "reset" (Reset) of the stack "sferics" on the session of `handle`. Empties the tables and counters, and learns the floor and the mains train again.
static inline int32_t sdrkit_sferics_reset(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "reset", NULL, NULL, NULL, NULL, 0);
}

// The settings of the test signal generator of the stack "sferics" are the sdrkit_sferics_gen_*() functions
// after the settings of its receiver.

// The setting "threshold_db" (Threshold over the floor) of the stack "sferics". Unit: dB. An impulse starts when the 25 µs power average rises this far over the floor.
static inline int32_t sdrkit_sferics_get_threshold_db(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "threshold_db", value);
}
static inline int32_t sdrkit_sferics_set_threshold_db(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "threshold_db", value);
}

// The setting "mains_filter" (Leave out mains trains) of the stack "sferics". Impulses on a 10 ms or 1/120 s grid are counted as mains, not as strikes.
static inline int32_t sdrkit_sferics_get_mains_filter(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "mains_filter", value);
}
static inline int32_t sdrkit_sferics_set_mains_filter(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "mains_filter", value);
}

// The setting "merge_ms" (Merge time) of the stack "sferics". Unit: ms. An impulse that starts this soon after the end of the one before joins it.
static inline int32_t sdrkit_sferics_get_merge_ms(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "merge_ms", value);
}
static inline int32_t sdrkit_sferics_set_merge_ms(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "merge_ms", value);
}

// The setting "start_utc" (UTC of sample 0 (files; 0 = host clock)) of the stack "sferics". Unit: s.
static inline int32_t sdrkit_sferics_get_start_utc(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "start_utc", value);
}
static inline int32_t sdrkit_sferics_set_start_utc(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "start_utc", value);
}

// The setting "gen_strike_rate" (Test signal: strokes a second) of the test signal generator of the stack "sferics". Unit: 1/s. Flashes of 1 to 3 strokes come at random gaps of 1.7 / rate seconds on average.
static inline int32_t sdrkit_sferics_gen_get_strike_rate(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_strike_rate", value);
}
static inline int32_t sdrkit_sferics_gen_set_strike_rate(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_strike_rate", value);
}

// The setting "gen_scene" (Test signal: scene) of the test signal generator of the stack "sferics". A value of `sdrkit_sferics_gen_scene`.
static inline int32_t sdrkit_sferics_gen_get_scene(SDRKIT_HANDLE handle, sdrkit_sferics_gen_scene *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "gen_scene", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_sferics_gen_scene)index;
  return code;
}
static inline int32_t sdrkit_sferics_gen_set_scene(SDRKIT_HANDLE handle, sdrkit_sferics_gen_scene value) {
  return sdrkit_typed_setting_set_enum(handle, "gen_scene", (int32_t)value);
}

// The setting "gen_mains_hz" (Test signal: mains frequency) of the test signal generator of the stack "sferics". A frequency in Hz. The train has two impulses a period: 100 a second at 50 Hz, 120 at 60 Hz.
static inline int32_t sdrkit_sferics_gen_get_mains_hz(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_mains_hz", value);
}
static inline int32_t sdrkit_sferics_gen_set_mains_hz(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_mains_hz", value);
}

// Fills `rows` with the changed rows of the table "transmitters" of the stack "supersid" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The column `label` names a row.
static inline int32_t sdrkit_supersid_transmitters_rows(SDRKIT_HANDLE handle,
                                                       sdrkit_supersid_transmitter_row *rows,
                                                       uint32_t capacity,
                                                       sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "science.SupersidTransmitterRow", "transmitters", rows, sizeof(sdrkit_supersid_transmitter_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "sid_events" of the stack "supersid" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The rows are a log: they arrive over time.
// The column `label` names a row.
// Time: `start_utc`.
static inline int32_t sdrkit_supersid_sid_events_rows(SDRKIT_HANDLE handle,
                                                     sdrkit_supersid_event_row *rows,
                                                     uint32_t capacity,
                                                     sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "science.SupersidEventRow", "sid_events", rows, sizeof(sdrkit_supersid_event_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "marks" of the stack "supersid" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The column `label` names a row.
static inline int32_t sdrkit_supersid_marks_rows(SDRKIT_HANDLE handle,
                                                sdrkit_supersid_mark_row *rows,
                                                uint32_t capacity,
                                                sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "science.SupersidMarkRow", "marks", rows, sizeof(sdrkit_supersid_mark_row), capacity, info);
}

// Fills `event` with event `n` of the last sdrkit_events_take() on `handle`, an event of the type "sid.event"
// (Sudden ionospheric disturbance). Returns SDRKIT_E_WRONG_TYPE if the event is of another type.
static inline int32_t sdrkit_supersid_sid_event_read(SDRKIT_HANDLE handle, uint32_t n, sdrkit_supersid_sid_event *event) {
  return sdrkit_typed_event(handle, n, "sid.event", "science.SupersidSidEvent", event, sizeof(sdrkit_supersid_sid_event));
}

// Runs the command "clear" (Clear) of the stack "supersid" on the session of `handle`. Forgets everything decoded so far.
static inline int32_t sdrkit_supersid_clear(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "clear", NULL, NULL, NULL, NULL, 0);
}

// Runs the command "reset" (Reset) of the stack "supersid" on the session of `handle`. Clears, and starts the levels of the hour and the judging again (15 minutes); keeps the minute means.
static inline int32_t sdrkit_supersid_reset(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "reset", NULL, NULL, NULL, NULL, 0);
}

// The settings of the test signal generator of the stack "supersid" are the sdrkit_supersid_gen_*() functions
// after the settings of its receiver.

// The setting "transmitters" (Monitored transmitters) of the stack "supersid". One record per line or between semicolons: label,frequency_hz or label,frequency_hz,lat,lon. At most 32. The host fills it from its transmitter file.
static inline int32_t sdrkit_supersid_get_transmitters(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "transmitters", value);
}
static inline int32_t sdrkit_supersid_set_transmitters(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "transmitters", value);
}

// The setting "step_db" (Step threshold) of the stack "supersid". Unit: dB. The change of the 2-minute mean against 10 minutes earlier that starts an event.
static inline int32_t sdrkit_supersid_get_step_db(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "step_db", value);
}
static inline int32_t sdrkit_supersid_set_step_db(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "step_db", value);
}

// The setting "average_s" (Sample interval) of the stack "supersid". Unit: s. The time over which the spectra are summed for one level; also the log interval of the file.
static inline int32_t sdrkit_supersid_get_average_s(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "average_s", value);
}
static inline int32_t sdrkit_supersid_set_average_s(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "average_s", value);
}

// The setting "start_utc" (UTC of sample 0 (files; 0 = host clock)) of the stack "supersid". Unit: s.
static inline int32_t sdrkit_supersid_get_start_utc(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "start_utc", value);
}
static inline int32_t sdrkit_supersid_set_start_utc(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "start_utc", value);
}

// The setting "log_format" (Data file) of the stack "supersid". A value of `sdrkit_supersid_log_format`. Writes the SuperSID text data file, one per UTC day, into the data file folder.
static inline int32_t sdrkit_supersid_get_log_format(SDRKIT_HANDLE handle, sdrkit_supersid_log_format *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "log_format", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_supersid_log_format)index;
  return code;
}
static inline int32_t sdrkit_supersid_set_log_format(SDRKIT_HANDLE handle, sdrkit_supersid_log_format value) {
  return sdrkit_typed_setting_set_enum(handle, "log_format", (int32_t)value);
}

// The setting "site" (Site name) of the stack "supersid". The Site line of the data file and the start of its name.
static inline int32_t sdrkit_supersid_get_site(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "site", value);
}
static inline int32_t sdrkit_supersid_set_site(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "site", value);
}

// The setting "monitor_id" (Monitor id) of the stack "supersid". The MonitorID line of the data file.
static inline int32_t sdrkit_supersid_get_monitor_id(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "monitor_id", value);
}
static inline int32_t sdrkit_supersid_set_monitor_id(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "monitor_id", value);
}

// The setting "log_folder" (Data file folder) of the stack "supersid". A folder of the host's storage.
static inline int32_t sdrkit_supersid_get_log_folder(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "log_folder", value);
}
static inline int32_t sdrkit_supersid_set_log_folder(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "log_folder", value);
}

// The setting "gen_flare" (Test signal flare) of the test signal generator of the stack "supersid". The first carrier 3 dB stronger from 17 minutes on.
static inline int32_t sdrkit_supersid_gen_get_flare(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "gen_flare", value);
}
static inline int32_t sdrkit_supersid_gen_set_flare(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "gen_flare", value);
}

// Fills `rows` with the changed rows of the table "peaks" of the stack "schumann" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The column `resonance` names a row.
// Default order: the column `resonance`, lowest first.
static inline int32_t sdrkit_schumann_peaks_rows(SDRKIT_HANDLE handle,
                                                sdrkit_schumann_peak_row *rows,
                                                uint32_t capacity,
                                                sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "science.SchumannPeakRow", "peaks", rows, sizeof(sdrkit_schumann_peak_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "events" of the stack "schumann" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The rows are a log: they arrive over time.
// The column `type` names a row.
// Time: `utc`.
static inline int32_t sdrkit_schumann_events_rows(SDRKIT_HANDLE handle,
                                                 sdrkit_science_event_row *rows,
                                                 uint32_t capacity,
                                                 sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "science.ScienceEventRow", "events", rows, sizeof(sdrkit_science_event_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "trend" of the stack "schumann" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_schumann_trend_rows(SDRKIT_HANDLE handle,
                                                sdrkit_schumann_trend_row *rows,
                                                uint32_t capacity,
                                                sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "science.SchumannTrendRow", "trend", rows, sizeof(sdrkit_schumann_trend_row), capacity, info);
}

// Fills `event` with event `n` of the last sdrkit_events_take() on `handle`, an event of the type "science.event"
// (Science event). Returns SDRKIT_E_WRONG_TYPE if the event is of another type.
static inline int32_t sdrkit_science_event_read(SDRKIT_HANDLE handle, uint32_t n, sdrkit_science_event *event) {
  return sdrkit_typed_event(handle, n, "science.event", "science.ScienceEvent", event, sizeof(sdrkit_science_event));
}

// Runs the command "clear" (Clear) of the stack "schumann" on the session of `handle`. Forgets everything decoded so far.
static inline int32_t sdrkit_schumann_clear(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "clear", NULL, NULL, NULL, NULL, 0);
}

// Runs the command "reset" (Reset) of the stack "schumann" on the session of `handle`. Empties the tables and starts the monitor again; the average and the comparison level are learned again.
static inline int32_t sdrkit_schumann_reset(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "reset", NULL, NULL, NULL, NULL, 0);
}

// The settings of the test signal generator of the stack "schumann" are the sdrkit_schumann_gen_*() functions
// after the settings of its receiver.

// The setting "threshold_db" (Transient threshold) of the stack "schumann". Unit: dB. A 0.5 s group this much over the median of the quiet groups is part of a transient: 1 to 20 dB, 0 for the default of 8 dB.
static inline int32_t sdrkit_schumann_get_threshold_db(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "threshold_db", value);
}
static inline int32_t sdrkit_schumann_set_threshold_db(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "threshold_db", value);
}

// The setting "start_utc" (UTC of sample 0 (files; 0 = host clock)) of the stack "schumann". Unit: s.
static inline int32_t sdrkit_schumann_get_start_utc(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "start_utc", value);
}
static inline int32_t sdrkit_schumann_set_start_utc(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "start_utc", value);
}

// The setting "gen_fundamental_hz" (Test signal fundamental) of the test signal generator of the stack "schumann". A frequency in Hz. The first resonance of the test signal; the others follow in proportion.
static inline int32_t sdrkit_schumann_gen_get_fundamental_hz(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_fundamental_hz", value);
}
static inline int32_t sdrkit_schumann_gen_set_fundamental_hz(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_fundamental_hz", value);
}

// The setting "gen_transients" (Test signal transients) of the test signal generator of the stack "schumann". A 12 Hz transient every 60 s from 70 s, Gaussian envelope of 0.25 s: its amplitude is 6 × this level × the rms of the background. 0 for none.
static inline int32_t sdrkit_schumann_gen_get_transients(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_transients", value);
}
static inline int32_t sdrkit_schumann_gen_set_transients(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_transients", value);
}

// Fills `rows` with the changed rows of the table "events" of the stack "whistler" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The rows are a log: they arrive over time.
// The column `type` names a row.
// Time: `utc`.
static inline int32_t sdrkit_whistler_events_rows(SDRKIT_HANDLE handle,
                                                 sdrkit_science_event_row *rows,
                                                 uint32_t capacity,
                                                 sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "science.ScienceEventRow", "events", rows, sizeof(sdrkit_science_event_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "trend" of the stack "whistler" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_whistler_trend_rows(SDRKIT_HANDLE handle,
                                                sdrkit_whistler_trend_row *rows,
                                                uint32_t capacity,
                                                sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "science.WhistlerTrendRow", "trend", rows, sizeof(sdrkit_whistler_trend_row), capacity, info);
}

// Runs the command "clear" (Clear) of the stack "whistler" on the session of `handle`. Forgets everything decoded so far.
static inline int32_t sdrkit_whistler_clear(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "clear", NULL, NULL, NULL, NULL, 0);
}

// Runs the command "reset" (Reset) of the stack "whistler" on the session of `handle`. Empties the tables and starts the detector again; the floor is learned again.
static inline int32_t sdrkit_whistler_reset(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "reset", NULL, NULL, NULL, NULL, 0);
}

// The settings of the test signal generator of the stack "whistler" are the sdrkit_whistler_gen_*() functions
// after the settings of its receiver.

// The setting "threshold_db" (Whistler path threshold) of the stack "whistler". Unit: dB. The least mean level of a whistler path over the floor: 1 to 15 dB, 0 for the default of 6 dB.
static inline int32_t sdrkit_whistler_get_threshold_db(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "threshold_db", value);
}
static inline int32_t sdrkit_whistler_set_threshold_db(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "threshold_db", value);
}

// The setting "start_utc" (UTC of sample 0 (files; 0 = host clock)) of the stack "whistler". Unit: s.
static inline int32_t sdrkit_whistler_get_start_utc(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "start_utc", value);
}
static inline int32_t sdrkit_whistler_set_start_utc(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "start_utc", value);
}

// The setting "gen_dispersion" (Test signal dispersion) of the test signal generator of the stack "whistler". Unit: s√Hz. D of the whistlers of the test signal. 60 is a test value: documents give about 25 to 40 s√Hz for night-time whistlers at low latitude, more by day and at higher latitude.
static inline int32_t sdrkit_whistler_gen_get_dispersion(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_dispersion", value);
}
static inline int32_t sdrkit_whistler_gen_set_dispersion(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_dispersion", value);
}

// The setting "gen_cutoff_hz" (Test signal tweek cut-off) of the test signal generator of the stack "whistler". A frequency in Hz. Documents give 1.6 to 1.9 kHz for the first mode (about 1.8 kHz for a reflection height of 83 km).
static inline int32_t sdrkit_whistler_gen_get_cutoff_hz(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_cutoff_hz", value);
}
static inline int32_t sdrkit_whistler_gen_set_cutoff_hz(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_cutoff_hz", value);
}

// The setting "gen_tone_snr_db" (Test signal whistler SNR) of the test signal generator of the stack "whistler". Unit: dB. The power of a whistler over the power of the noise in 1 kHz.
static inline int32_t sdrkit_whistler_gen_get_tone_snr_db(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_tone_snr_db", value);
}
static inline int32_t sdrkit_whistler_gen_set_tone_snr_db(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_tone_snr_db", value);
}

// Fills `rows` with the changed rows of the table "strikes" of the stack "lightning_tdoa" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The rows are a log: they arrive over time.
// Place of a row: the columns `lat` and `lon`.
// Time: `time`.
static inline int32_t sdrkit_lightning_tdoa_strikes_rows(SDRKIT_HANDLE handle,
                                                        sdrkit_lightning_tdoa_strike_row *rows,
                                                        uint32_t capacity,
                                                        sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "science.LightningTdoaStrikeRow", "strikes", rows, sizeof(sdrkit_lightning_tdoa_strike_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "stations" of the stack "lightning_tdoa" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The column `name` names a row.
// Place of a row: the columns `lat` and `lon`.
static inline int32_t sdrkit_lightning_tdoa_stations_rows(SDRKIT_HANDLE handle,
                                                         sdrkit_lightning_tdoa_station_row *rows,
                                                         uint32_t capacity,
                                                         sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "science.LightningTdoaStationRow", "stations", rows, sizeof(sdrkit_lightning_tdoa_station_row), capacity, info);
}

// Fills `event` with event `n` of the last sdrkit_events_take() on `handle`, an event of the type "lightning.strike"
// (Lightning strike). Returns SDRKIT_E_WRONG_TYPE if the event is of another type.
static inline int32_t sdrkit_lightning_tdoa_lightning_strike_event_read(SDRKIT_HANDLE handle, uint32_t n, sdrkit_lightning_tdoa_lightning_strike_event *event) {
  return sdrkit_typed_event(handle, n, "lightning.strike", "science.LightningStrikeEvent", event, sizeof(sdrkit_lightning_tdoa_lightning_strike_event));
}

// Runs the command "clear" (Clear) of the stack "lightning_tdoa" on the session of `handle`. Forgets everything decoded so far.
static inline int32_t sdrkit_lightning_tdoa_clear(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "clear", NULL, NULL, NULL, NULL, 0);
}

// Runs the command "reset" (Reset) of the stack "lightning_tdoa" on the session of `handle`. Empties the tables and counters and learns the floor of the sferic timer again; the clock discipline is kept.
static inline int32_t sdrkit_lightning_tdoa_reset(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "reset", NULL, NULL, NULL, NULL, 0);
}

// Runs the command "datagram" (Feed a datagram) of the stack "lightning_tdoa" on the session of `handle`. The server takes one datagram as if it came from the network.
static inline int32_t sdrkit_lightning_tdoa_datagram(SDRKIT_HANDLE handle, const sdrkit_lightning_tdoa_datagram_args *args) {
  return sdrkit_typed_command(handle, "datagram", "science.LightningTdoaDatagramArgs", args, NULL, NULL, 0);
}

// Runs the command "time_mark" (Time mark) of the stack "lightning_tdoa" on the session of `handle`. UTC at an input sample, from the radio's GPS time or a GNSS receiver, as the host reads it.
static inline int32_t sdrkit_lightning_tdoa_time_mark(SDRKIT_HANDLE handle, const sdrkit_lightning_tdoa_time_mark_args *args) {
  return sdrkit_typed_command(handle, "time_mark", "science.LightningTdoaTimeMarkArgs", args, NULL, NULL, 0);
}

// The settings of the test signal generator of the stack "lightning_tdoa" are the sdrkit_lightning_tdoa_gen_*() functions
// after the settings of its receiver.

// The setting "role" (Role) of the stack "lightning_tdoa". A value of `sdrkit_lightning_tdoa_role`. Local times sferics and sends nothing. A client sends to the server. The server collects and locates.
static inline int32_t sdrkit_lightning_tdoa_get_role(SDRKIT_HANDLE handle, sdrkit_lightning_tdoa_role *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "role", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_lightning_tdoa_role)index;
  return code;
}
static inline int32_t sdrkit_lightning_tdoa_set_role(SDRKIT_HANDLE handle, sdrkit_lightning_tdoa_role value) {
  return sdrkit_typed_setting_set_enum(handle, "role", (int32_t)value);
}

// The setting "server" (Server address (client)) of the stack "lightning_tdoa". host or host:port.
static inline int32_t sdrkit_lightning_tdoa_get_server(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "server", value);
}
static inline int32_t sdrkit_lightning_tdoa_set_server(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "server", value);
}

// The setting "port" (UDP port) of the stack "lightning_tdoa".
static inline int32_t sdrkit_lightning_tdoa_get_port(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "port", value);
}
static inline int32_t sdrkit_lightning_tdoa_set_port(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "port", value);
}

// The setting "network_key" (Network key (secret)) of the stack "lightning_tdoa". 12 characters or more; shorter keeps the network off. The host keeps it; it is never sent.
static inline int32_t sdrkit_lightning_tdoa_get_network_key(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "network_key", value);
}
static inline int32_t sdrkit_lightning_tdoa_set_network_key(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "network_key", value);
}

// The setting "station_name" (Station name) of the stack "lightning_tdoa". 1 to 24 printable characters. Its hash is the station id.
static inline int32_t sdrkit_lightning_tdoa_get_station_name(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "station_name", value);
}
static inline int32_t sdrkit_lightning_tdoa_set_station_name(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "station_name", value);
}

// The setting "server_is_station" (The server's radio counts as a station) of the stack "lightning_tdoa".
static inline int32_t sdrkit_lightning_tdoa_get_server_is_station(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "server_is_station", value);
}
static inline int32_t sdrkit_lightning_tdoa_set_server_is_station(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "server_is_station", value);
}

// The setting "system_clock" (Computer clock) of the stack "lightning_tdoa". A value of `sdrkit_lightning_tdoa_system_clock`.
static inline int32_t sdrkit_lightning_tdoa_get_system_clock(SDRKIT_HANDLE handle, sdrkit_lightning_tdoa_system_clock *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "system_clock", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_lightning_tdoa_system_clock)index;
  return code;
}
static inline int32_t sdrkit_lightning_tdoa_set_system_clock(SDRKIT_HANDLE handle, sdrkit_lightning_tdoa_system_clock value) {
  return sdrkit_typed_setting_set_enum(handle, "system_clock", (int32_t)value);
}

// The setting "system_sigma_us" (Computer clock error) of the stack "lightning_tdoa". Unit: us.
static inline int32_t sdrkit_lightning_tdoa_get_system_sigma_us(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "system_sigma_us", value);
}
static inline int32_t sdrkit_lightning_tdoa_set_system_sigma_us(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "system_sigma_us", value);
}

// The setting "use_radio_time" (Use the radio's GPS time) of the stack "lightning_tdoa". GPS marks that the host gives (time_mark) are used.
static inline int32_t sdrkit_lightning_tdoa_get_use_radio_time(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "use_radio_time", value);
}
static inline int32_t sdrkit_lightning_tdoa_set_use_radio_time(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "use_radio_time", value);
}

// The setting "timing_id" (Timing transmitter id) of the stack "lightning_tdoa". The same at every station of a network.
static inline int32_t sdrkit_lightning_tdoa_get_timing_id(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "timing_id", value);
}
static inline int32_t sdrkit_lightning_tdoa_set_timing_id(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "timing_id", value);
}

// The setting "timing_label" (Timing transmitter) of the stack "lightning_tdoa".
static inline int32_t sdrkit_lightning_tdoa_get_timing_label(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "timing_label", value);
}
static inline int32_t sdrkit_lightning_tdoa_set_timing_label(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "timing_label", value);
}

// The setting "timing_hz" (Timing transmitter frequency) of the stack "lightning_tdoa". A frequency in Hz. Used when it lies within 0.45 times the rate of the tuned centre.
static inline int32_t sdrkit_lightning_tdoa_get_timing_hz(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "timing_hz", value);
}
static inline int32_t sdrkit_lightning_tdoa_set_timing_hz(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "timing_hz", value);
}

// The setting "timing_baud" (Timing transmitter symbol rate) of the stack "lightning_tdoa". Unit: Bd. No public document gives the rate of each transmitter; 200 Bd is assumed.
static inline int32_t sdrkit_lightning_tdoa_get_timing_baud(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "timing_baud", value);
}
static inline int32_t sdrkit_lightning_tdoa_set_timing_baud(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "timing_baud", value);
}

// The setting "timing_lat" (Timing transmitter latitude) of the stack "lightning_tdoa". Unit: °.
static inline int32_t sdrkit_lightning_tdoa_get_timing_lat(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "timing_lat", value);
}
static inline int32_t sdrkit_lightning_tdoa_set_timing_lat(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "timing_lat", value);
}

// The setting "timing_lon" (Timing transmitter longitude) of the stack "lightning_tdoa". Unit: °.
static inline int32_t sdrkit_lightning_tdoa_get_timing_lon(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "timing_lon", value);
}
static inline int32_t sdrkit_lightning_tdoa_set_timing_lon(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "timing_lon", value);
}

// The setting "threshold_db" (Sferic threshold) of the stack "lightning_tdoa". Unit: dB. A sferic starts when the 10 µs power average rises this far over the floor.
static inline int32_t sdrkit_lightning_tdoa_get_threshold_db(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "threshold_db", value);
}
static inline int32_t sdrkit_lightning_tdoa_set_threshold_db(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "threshold_db", value);
}

// The setting "velocity_factor" (Ground-wave velocity / c) of the stack "lightning_tdoa". An assumption: 0.1 % error moves fixes by about 0.3 km at 400 km.
static inline int32_t sdrkit_lightning_tdoa_get_velocity_factor(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "velocity_factor", value);
}
static inline int32_t sdrkit_lightning_tdoa_set_velocity_factor(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "velocity_factor", value);
}

// The setting "hops" (Ionosphere hops for long paths) of the stack "lightning_tdoa". Paths of 1500 km or more travel by hops between the ground and the reflecting layer.
static inline int32_t sdrkit_lightning_tdoa_get_hops(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "hops", value);
}
static inline int32_t sdrkit_lightning_tdoa_set_hops(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "hops", value);
}

// The setting "hop_height_km" (Reflection height) of the stack "lightning_tdoa". Unit: km.
static inline int32_t sdrkit_lightning_tdoa_get_hop_height_km(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "hop_height_km", value);
}
static inline int32_t sdrkit_lightning_tdoa_set_hop_height_km(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "hop_height_km", value);
}

// The setting "min_stations" (Fine stations for a fix) of the stack "lightning_tdoa". Three-station fixes, which nothing checks, only at 3.
static inline int32_t sdrkit_lightning_tdoa_get_min_stations(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "min_stations", value);
}
static inline int32_t sdrkit_lightning_tdoa_set_min_stations(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "min_stations", value);
}

// The setting "start_utc" (UTC of sample 0 (files; 0 = host clock)) of the stack "lightning_tdoa". Unit: s.
static inline int32_t sdrkit_lightning_tdoa_get_start_utc(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "start_utc", value);
}
static inline int32_t sdrkit_lightning_tdoa_set_start_utc(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "start_utc", value);
}

// The setting "gen_strike_rate" (Test signal: strikes a second) of the test signal generator of the stack "lightning_tdoa". Unit: 1/s. Strikes come at random gaps, at least 2 ms apart, within 3° of latitude and 4° of longitude of 52.5 N 13.4 E.
static inline int32_t sdrkit_lightning_tdoa_gen_get_strike_rate(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_strike_rate", value);
}
static inline int32_t sdrkit_lightning_tdoa_gen_set_strike_rate(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_strike_rate", value);
}

// The setting "gen_timing_tx" (Test signal: timing transmitter) of the test signal generator of the stack "lightning_tdoa". A 200 Bd MSK transmitter at 52.0 N 10.0 E, amplitude 0.05.
static inline int32_t sdrkit_lightning_tdoa_gen_get_timing_tx(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "gen_timing_tx", value);
}
static inline int32_t sdrkit_lightning_tdoa_gen_set_timing_tx(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "gen_timing_tx", value);
}

// The setting "gen_timing_offset_hz" (Test signal: timing transmitter offset) of the test signal generator of the stack "lightning_tdoa". A frequency in Hz. From the tuned centre; at most the rate / 2.5.
static inline int32_t sdrkit_lightning_tdoa_gen_get_timing_offset_hz(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_timing_offset_hz", value);
}
static inline int32_t sdrkit_lightning_tdoa_gen_set_timing_offset_hz(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_timing_offset_hz", value);
}

// The setting "gen_station_lat" (Test signal: station latitude) of the test signal generator of the stack "lightning_tdoa". Unit: °. Where the test signal is heard, when the host gives no station position.
static inline int32_t sdrkit_lightning_tdoa_gen_get_station_lat(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_station_lat", value);
}
static inline int32_t sdrkit_lightning_tdoa_gen_set_station_lat(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_station_lat", value);
}

// The setting "gen_station_lon" (Test signal: station longitude) of the test signal generator of the stack "lightning_tdoa". Unit: °.
static inline int32_t sdrkit_lightning_tdoa_gen_get_station_lon(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_station_lon", value);
}
static inline int32_t sdrkit_lightning_tdoa_gen_set_station_lon(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_station_lon", value);
}

// Fills `rows` with the changed rows of the table "events" of the stack "jove" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The rows are a log: they arrive over time.
// The column `type` names a row.
// Time: `utc`.
static inline int32_t sdrkit_jove_events_rows(SDRKIT_HANDLE handle,
                                             sdrkit_science_event_row *rows,
                                             uint32_t capacity,
                                             sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "science.ScienceEventRow", "events", rows, sizeof(sdrkit_science_event_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "trend" of the stack "jove" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_jove_trend_rows(SDRKIT_HANDLE handle,
                                            sdrkit_science_burst_trend_row *rows,
                                            uint32_t capacity,
                                            sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "science.BurstTrendRow", "trend", rows, sizeof(sdrkit_science_burst_trend_row), capacity, info);
}

// Runs the command "clear" (Clear) of the stack "jove" on the session of `handle`. Forgets everything decoded so far.
static inline int32_t sdrkit_jove_clear(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "clear", NULL, NULL, NULL, NULL, 0);
}

// Runs the command "reset" (Reset) of the stack "jove" on the session of `handle`. Empties the tables and starts the detector again; the background is learned again.
static inline int32_t sdrkit_jove_reset(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "reset", NULL, NULL, NULL, NULL, 0);
}

// The settings of the test signal generator of the stack "jove" are the sdrkit_jove_gen_*() functions
// after the settings of its receiver.

// The setting "threshold_db" (Burst threshold per bin) of the stack "jove". Unit: dB. The least excess of an active bin over its background: 0.5 to 20 dB, 0 for the default of 1 dB.
static inline int32_t sdrkit_jove_get_threshold_db(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "threshold_db", value);
}
static inline int32_t sdrkit_jove_set_threshold_db(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "threshold_db", value);
}

// The setting "start_utc" (UTC of sample 0 (files; 0 = host clock)) of the stack "jove". Unit: s.
static inline int32_t sdrkit_jove_get_start_utc(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "start_utc", value);
}
static inline int32_t sdrkit_jove_set_start_utc(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "start_utc", value);
}

// The setting "gen_level_db" (Test signal burst level) of the test signal generator of the stack "jove". Unit: dB. The power density of a burst over the noise density.
static inline int32_t sdrkit_jove_gen_get_level_db(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_level_db", value);
}
static inline int32_t sdrkit_jove_gen_set_level_db(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_level_db", value);
}

// Fills `rows` with the changed rows of the table "events" of the stack "callisto" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The rows are a log: they arrive over time.
// The column `type` names a row.
// Time: `utc`.
static inline int32_t sdrkit_callisto_events_rows(SDRKIT_HANDLE handle,
                                                 sdrkit_science_event_row *rows,
                                                 uint32_t capacity,
                                                 sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "science.ScienceEventRow", "events", rows, sizeof(sdrkit_science_event_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "trend" of the stack "callisto" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_callisto_trend_rows(SDRKIT_HANDLE handle,
                                                sdrkit_science_burst_trend_row *rows,
                                                uint32_t capacity,
                                                sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "science.BurstTrendRow", "trend", rows, sizeof(sdrkit_science_burst_trend_row), capacity, info);
}

// Runs the command "clear" (Clear) of the stack "callisto" on the session of `handle`. Forgets everything decoded so far.
static inline int32_t sdrkit_callisto_clear(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "clear", NULL, NULL, NULL, NULL, 0);
}

// Runs the command "reset" (Reset) of the stack "callisto" on the session of `handle`. Empties the tables and starts the detector again; the background is learned again.
static inline int32_t sdrkit_callisto_reset(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "reset", NULL, NULL, NULL, NULL, 0);
}

// The settings of the test signal generator of the stack "callisto" are the sdrkit_callisto_gen_*() functions
// after the settings of its receiver.

// The setting "threshold_db" (Burst threshold per bin) of the stack "callisto". Unit: dB. The least excess of an active bin over its background: 0.5 to 20 dB, 0 for the default of 1 dB.
static inline int32_t sdrkit_callisto_get_threshold_db(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "threshold_db", value);
}
static inline int32_t sdrkit_callisto_set_threshold_db(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "threshold_db", value);
}

// The setting "start_utc" (UTC of sample 0 (files; 0 = host clock)) of the stack "callisto". Unit: s.
static inline int32_t sdrkit_callisto_get_start_utc(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "start_utc", value);
}
static inline int32_t sdrkit_callisto_set_start_utc(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "start_utc", value);
}

// The setting "fits" (Write FITS files) of the stack "callisto". Writes e-CALLISTO FITS files of the spectrogram into the file folder of the host's storage.
static inline int32_t sdrkit_callisto_get_fits(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "fits", value);
}
static inline int32_t sdrkit_callisto_set_fits(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "fits", value);
}

// The setting "fits_folder" (FITS folder) of the stack "callisto". A folder of the host's storage.
static inline int32_t sdrkit_callisto_get_fits_folder(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "fits_folder", value);
}
static inline int32_t sdrkit_callisto_set_fits_folder(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "fits_folder", value);
}

// The setting "station_code" (Instrument code) of the stack "callisto". Letters and digits: the start of the file name, INSTRUME and ORIGIN.
static inline int32_t sdrkit_callisto_get_station_code(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "station_code", value);
}
static inline int32_t sdrkit_callisto_set_station_code(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "station_code", value);
}

// The setting "focus_code" (Focus code) of the stack "callisto". The two digits at the end of the file name (the manual lists 00 to 63, default 59).
static inline int32_t sdrkit_callisto_get_focus_code(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "focus_code", value);
}
static inline int32_t sdrkit_callisto_set_focus_code(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "focus_code", value);
}

// The setting "file_s" (File length) of the stack "callisto". Unit: s. Files cover whole multiples of this length in UTC (e-CALLISTO: 900 s).
static inline int32_t sdrkit_callisto_get_file_s(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "file_s", value);
}
static inline int32_t sdrkit_callisto_set_file_s(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "file_s", value);
}

// The setting "fits_bitpix" (Pixel type) of the stack "callisto". A value of `sdrkit_callisto_fits_bitpix`. BITPIX of the image: 16 as the manual's example header (values 0 to 255), or 8.
static inline int32_t sdrkit_callisto_get_fits_bitpix(SDRKIT_HANDLE handle, sdrkit_callisto_fits_bitpix *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "fits_bitpix", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_callisto_fits_bitpix)index;
  return code;
}
static inline int32_t sdrkit_callisto_set_fits_bitpix(SDRKIT_HANDLE handle, sdrkit_callisto_fits_bitpix value) {
  return sdrkit_typed_setting_set_enum(handle, "fits_bitpix", (int32_t)value);
}

// The setting "db_per_digit" (dB per digit) of the stack "callisto". Unit: dB. The power step of one digit. Not given by a public document that was read.
static inline int32_t sdrkit_callisto_get_db_per_digit(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "db_per_digit", value);
}
static inline int32_t sdrkit_callisto_set_db_per_digit(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "db_per_digit", value);
}

// The setting "zero_dbfs" (Level of digit 0) of the stack "callisto". Unit: dBFS. The power of digit 0, dB of full scale.
static inline int32_t sdrkit_callisto_get_zero_dbfs(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "zero_dbfs", value);
}
static inline int32_t sdrkit_callisto_set_zero_dbfs(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "zero_dbfs", value);
}

// The setting "gen_level_db" (Test signal burst level) of the test signal generator of the stack "callisto". Unit: dB. The power density of a burst over the noise density.
static inline int32_t sdrkit_callisto_gen_get_level_db(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_level_db", value);
}
static inline int32_t sdrkit_callisto_gen_set_level_db(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_level_db", value);
}

// The setting "gen_burst_drift_hz_s" (Test signal burst drift) of the test signal generator of the stack "callisto". Unit: Hz/s. The drift of the bursts; 0 for 0.2 × rate over a quarter of the cycle.
static inline int32_t sdrkit_callisto_gen_get_burst_drift_hz_s(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_burst_drift_hz_s", value);
}
static inline int32_t sdrkit_callisto_gen_set_burst_drift_hz_s(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_burst_drift_hz_s", value);
}

// Fills `event` with event `n` of the last sdrkit_events_take() on `handle`, an event of the type "riometer.absorption"
// (Absorption event). Returns SDRKIT_E_WRONG_TYPE if the event is of another type.
static inline int32_t sdrkit_riometer_absorption_event_read(SDRKIT_HANDLE handle, uint32_t n, sdrkit_riometer_absorption_event *event) {
  return sdrkit_typed_event(handle, n, "riometer.absorption", "science.RiometerAbsorptionEvent", event, sizeof(sdrkit_riometer_absorption_event));
}

// Runs the command "clear" (Clear) of the stack "riometer" on the session of `handle`. Forgets everything decoded so far.
static inline int32_t sdrkit_riometer_clear(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "clear", NULL, NULL, NULL, NULL, 0);
}

// Runs the command "reset" (Reset) of the stack "riometer" on the session of `handle`. Starts the power estimate again and empties the series; the quiet-day curve stays.
static inline int32_t sdrkit_riometer_reset(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "reset", NULL, NULL, NULL, NULL, 0);
}

// Runs the command "export_qdc" (Export the quiet-day curve) of the stack "riometer" on the session of `handle`. Replies with the curve as text (qdc1 and 288 values).
static inline int32_t sdrkit_riometer_export_qdc(SDRKIT_HANDLE handle, sdrkit_riometer_qdc_reply *reply) {
  return sdrkit_typed_command(handle, "export_qdc", NULL, NULL, "science.RiometerQdcReply", reply, sizeof(sdrkit_riometer_qdc_reply));
}

// Runs the command "import_qdc" (Import a quiet-day curve) of the stack "riometer" on the session of `handle`. Takes a curve as text; a bad text is refused and changes nothing.
static inline int32_t sdrkit_riometer_import_qdc(SDRKIT_HANDLE handle, const sdrkit_riometer_qdc_args *args) {
  return sdrkit_typed_command(handle, "import_qdc", "science.RiometerQdcArgs", args, NULL, NULL, 0);
}

// The settings of the test signal generator of the stack "riometer" are the sdrkit_riometer_gen_*() functions
// after the settings of its receiver.

// The setting "threshold_db" (Absorption event threshold) of the stack "riometer". Unit: dB. An event lasts while the absorption is at this or more.
static inline int32_t sdrkit_riometer_get_threshold_db(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "threshold_db", value);
}
static inline int32_t sdrkit_riometer_set_threshold_db(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "threshold_db", value);
}

// The setting "qdc" (Quiet-day curve (saved state)) of the stack "riometer". The curve as text: qdc1 and 288 values. Empty keeps the curve the stack holds; a bad text is refused.
static inline int32_t sdrkit_riometer_get_qdc(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "qdc", value);
}
static inline int32_t sdrkit_riometer_set_qdc(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "qdc", value);
}

// The setting "start_utc" (UTC of sample 0 (files; 0 = host clock)) of the stack "riometer". Unit: s.
static inline int32_t sdrkit_riometer_get_start_utc(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "start_utc", value);
}
static inline int32_t sdrkit_riometer_set_start_utc(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "start_utc", value);
}

// The setting "gen_burst_share" (Test signal burst share) of the test signal generator of the stack "riometer". The share of 20 ms chunks of the test signal that are three times stronger.
static inline int32_t sdrkit_riometer_gen_get_burst_share(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_burst_share", value);
}
static inline int32_t sdrkit_riometer_gen_set_burst_share(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_burst_share", value);
}

// Fills `rows` with the changed rows of the table "events" of the stack "eme_echo" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The rows are a log: they arrive over time.
// The column `type` names a row.
// Time: `utc`.
static inline int32_t sdrkit_eme_echo_events_rows(SDRKIT_HANDLE handle,
                                                 sdrkit_science_event_row *rows,
                                                 uint32_t capacity,
                                                 sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "science.ScienceEventRow", "events", rows, sizeof(sdrkit_science_event_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "trend" of the stack "eme_echo" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_eme_echo_trend_rows(SDRKIT_HANDLE handle,
                                                sdrkit_eme_echo_trend_row *rows,
                                                uint32_t capacity,
                                                sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "science.EmeEchoTrendRow", "trend", rows, sizeof(sdrkit_eme_echo_trend_row), capacity, info);
}

// Runs the command "clear" (Clear) of the stack "eme_echo" on the session of `handle`. Forgets everything decoded so far.
static inline int32_t sdrkit_eme_echo_clear(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "clear", NULL, NULL, NULL, NULL, 0);
}

// Runs the command "reset" (Reset) of the stack "eme_echo" on the session of `handle`. Empties the tables and starts the detector again; the noise is learned again.
static inline int32_t sdrkit_eme_echo_reset(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "reset", NULL, NULL, NULL, NULL, 0);
}

// The settings of the test signal generator of the stack "eme_echo" are the sdrkit_eme_echo_gen_*() functions
// after the settings of its receiver.

// The setting "threshold_sigma" (Echo threshold) of the stack "eme_echo". Unit: sigma. The best echo score over the control mean in standard deviations of the control: 2 to 20, 0 for 6.
static inline int32_t sdrkit_eme_echo_get_threshold_sigma(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "threshold_sigma", value);
}
static inline int32_t sdrkit_eme_echo_set_threshold_sigma(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "threshold_sigma", value);
}

// The setting "start_utc" (UTC of sample 0 (files; 0 = host clock)) of the stack "eme_echo". Unit: s.
static inline int32_t sdrkit_eme_echo_get_start_utc(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "start_utc", value);
}
static inline int32_t sdrkit_eme_echo_set_start_utc(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "start_utc", value);
}

// The setting "gen_moon_delay_s" (Test signal echo delay) of the test signal generator of the stack "eme_echo". Unit: s. The delay of the echo after the carrier.
static inline int32_t sdrkit_eme_echo_gen_get_moon_delay_s(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_moon_delay_s", value);
}
static inline int32_t sdrkit_eme_echo_gen_set_moon_delay_s(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_moon_delay_s", value);
}

// The setting "gen_moon_doppler_hz" (Test signal echo Doppler) of the test signal generator of the stack "eme_echo". Unit: Hz. The shift of the echo from the carrier.
static inline int32_t sdrkit_eme_echo_gen_get_moon_doppler_hz(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_moon_doppler_hz", value);
}
static inline int32_t sdrkit_eme_echo_gen_set_moon_doppler_hz(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_moon_doppler_hz", value);
}

// The setting "gen_moon_spread_hz" (Test signal echo spread) of the test signal generator of the stack "eme_echo". Unit: Hz. The Doppler spread of the echo.
static inline int32_t sdrkit_eme_echo_gen_get_moon_spread_hz(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_moon_spread_hz", value);
}
static inline int32_t sdrkit_eme_echo_gen_set_moon_spread_hz(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_moon_spread_hz", value);
}

// The setting "gen_moon_level_db" (Test signal echo level) of the test signal generator of the stack "eme_echo". Unit: dB. The echo over the noise in 12 Hz.
static inline int32_t sdrkit_eme_echo_gen_get_moon_level_db(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_moon_level_db", value);
}
static inline int32_t sdrkit_eme_echo_gen_set_moon_level_db(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_moon_level_db", value);
}

// Fills `rows` with the changed rows of the table "events" of the stack "beacon_events" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The rows are a log: they arrive over time.
// The column `type` names a row.
// Time: `utc`.
static inline int32_t sdrkit_beacon_events_rows(SDRKIT_HANDLE handle,
                                               sdrkit_science_event_row *rows,
                                               uint32_t capacity,
                                               sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "science.ScienceEventRow", "events", rows, sizeof(sdrkit_science_event_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "trend" of the stack "beacon_events" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_beacon_events_trend_rows(SDRKIT_HANDLE handle,
                                                     sdrkit_beacon_events_trend_row *rows,
                                                     uint32_t capacity,
                                                     sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "science.BeaconEventsTrendRow", "trend", rows, sizeof(sdrkit_beacon_events_trend_row), capacity, info);
}

// Runs the command "clear" (Clear) of the stack "beacon_events" on the session of `handle`. Forgets everything decoded so far.
static inline int32_t sdrkit_beacon_events_clear(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "clear", NULL, NULL, NULL, NULL, 0);
}

// Runs the command "reset" (Reset) of the stack "beacon_events" on the session of `handle`. Empties the tables and starts the detector again.
static inline int32_t sdrkit_beacon_events_reset(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "reset", NULL, NULL, NULL, NULL, 0);
}

// The settings of the test signal generator of the stack "beacon_events" are the sdrkit_beacon_events_gen_*() functions
// after the settings of its receiver.

// The setting "threshold_z" (Opening threshold) of the stack "beacon_events". Unit: z. The best scan z of a hit row: 4 to 30, 0 for 12.
static inline int32_t sdrkit_beacon_events_get_threshold_z(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "threshold_z", value);
}
static inline int32_t sdrkit_beacon_events_set_threshold_z(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "threshold_z", value);
}

// The setting "start_utc" (UTC of sample 0 (files; 0 = host clock)) of the stack "beacon_events". Unit: s.
static inline int32_t sdrkit_beacon_events_get_start_utc(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "start_utc", value);
}
static inline int32_t sdrkit_beacon_events_set_start_utc(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "start_utc", value);
}

// The setting "gen_level_db" (Test signal level) of the test signal generator of the stack "beacon_events". Unit: dB. The carrier over the noise in a 2 Hz bin.
static inline int32_t sdrkit_beacon_events_gen_get_level_db(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_level_db", value);
}
static inline int32_t sdrkit_beacon_events_gen_set_level_db(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_level_db", value);
}

// The setting "gen_spread_hz" (Test signal spread) of the test signal generator of the stack "beacon_events". Unit: Hz. The Doppler spread of the carrier while open.
static inline int32_t sdrkit_beacon_events_gen_get_spread_hz(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_spread_hz", value);
}
static inline int32_t sdrkit_beacon_events_gen_set_spread_hz(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_spread_hz", value);
}

// The setting "gen_flutter_db" (Test signal flutter) of the test signal generator of the stack "beacon_events". Unit: dB. The standard deviation of the level of the carrier while open.
static inline int32_t sdrkit_beacon_events_gen_get_flutter_db(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_flutter_db", value);
}
static inline int32_t sdrkit_beacon_events_gen_set_flutter_db(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_flutter_db", value);
}

// Fills `rows` with the changed rows of the table "searches" of the stack "pulsar" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The rows are a log: they arrive over time.
// Time: `time_s`.
static inline int32_t sdrkit_pulsar_searches_rows(SDRKIT_HANDLE handle,
                                                 sdrkit_pulsar_search_row *rows,
                                                 uint32_t capacity,
                                                 sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "science.PulsarSearchRow", "searches", rows, sizeof(sdrkit_pulsar_search_row), capacity, info);
}

// Fills `event` with event `n` of the last sdrkit_events_take() on `handle`, an event of the type "pulsar.detection"
// (Pulsar detection). Returns SDRKIT_E_WRONG_TYPE if the event is of another type.
static inline int32_t sdrkit_pulsar_detection_event_read(SDRKIT_HANDLE handle, uint32_t n, sdrkit_pulsar_detection_event *event) {
  return sdrkit_typed_event(handle, n, "pulsar.detection", "science.PulsarDetectionEvent", event, sizeof(sdrkit_pulsar_detection_event));
}

// Runs the command "clear" (Clear) of the stack "pulsar" on the session of `handle`. Forgets everything decoded so far.
static inline int32_t sdrkit_pulsar_clear(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "clear", NULL, NULL, NULL, NULL, 0);
}

// Runs the command "search_now" (Search now) of the stack "pulsar" on the session of `handle`. Runs a search on the rows held now.
static inline int32_t sdrkit_pulsar_search_now(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "search_now", NULL, NULL, NULL, NULL, 0);
}

// The settings of the test signal generator of the stack "pulsar" are the sdrkit_pulsar_gen_*() functions
// after the settings of its receiver.

// The setting "period_s" (Pulsar period) of the stack "pulsar". Unit: s. The period of the pulsar from your ephemeris. 0.5 s is a test value; the data file data/science/pulsars.csv lists real ones.
static inline int32_t sdrkit_pulsar_get_period_s(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "period_s", value);
}
static inline int32_t sdrkit_pulsar_set_period_s(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "period_s", value);
}

// The setting "dm" (Dispersion measure) of the stack "pulsar". Unit: pc cm⁻³. The dispersion measure of the pulsar. 20 is a test value.
static inline int32_t sdrkit_pulsar_get_dm(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "dm", value);
}
static inline int32_t sdrkit_pulsar_set_dm(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "dm", value);
}

// The setting "channels" (Filter-bank channels) of the stack "pulsar". A power of two from 8 to 1024.
static inline int32_t sdrkit_pulsar_get_channels(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "channels", value);
}
static inline int32_t sdrkit_pulsar_set_channels(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "channels", value);
}

// The setting "sample_ms" (Power sample time) of the stack "pulsar". Unit: ms. The time of a row asked; a row is a whole number of spectra, so the time used differs a little.
static inline int32_t sdrkit_pulsar_get_sample_ms(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "sample_ms", value);
}
static inline int32_t sdrkit_pulsar_set_sample_ms(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "sample_ms", value);
}

// The setting "bins" (Profile bins) of the stack "pulsar".
static inline int32_t sdrkit_pulsar_get_bins(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "bins", value);
}
static inline int32_t sdrkit_pulsar_set_bins(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "bins", value);
}

// The setting "dm_span" (Trial DM span (±)) of the stack "pulsar". Unit: pc cm⁻³.
static inline int32_t sdrkit_pulsar_get_dm_span(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "dm_span", value);
}
static inline int32_t sdrkit_pulsar_set_dm_span(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "dm_span", value);
}

// The setting "dm_steps" (Trial DMs) of the stack "pulsar".
static inline int32_t sdrkit_pulsar_get_dm_steps(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "dm_steps", value);
}
static inline int32_t sdrkit_pulsar_set_dm_steps(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "dm_steps", value);
}

// The setting "period_span" (Trial period span (± relative)) of the stack "pulsar". 2e-4 covers the Earth's orbital Doppler (up to 1e-4 of the period) for a period known to 1e-5.
static inline int32_t sdrkit_pulsar_get_period_span(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "period_span", value);
}
static inline int32_t sdrkit_pulsar_set_period_span(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "period_span", value);
}

// The setting "period_steps" (Trial periods) of the stack "pulsar".
static inline int32_t sdrkit_pulsar_get_period_steps(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "period_steps", value);
}
static inline int32_t sdrkit_pulsar_set_period_steps(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "period_steps", value);
}

// The setting "flux_jy" (Flux of the pulsar) of the stack "pulsar". Unit: Jy. For the area estimate only.
static inline int32_t sdrkit_pulsar_get_flux_jy(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "flux_jy", value);
}
static inline int32_t sdrkit_pulsar_set_flux_jy(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "flux_jy", value);
}

// The setting "tsys_k" (System temperature) of the stack "pulsar". Unit: K. For the area estimate only.
static inline int32_t sdrkit_pulsar_get_tsys_k(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "tsys_k", value);
}
static inline int32_t sdrkit_pulsar_set_tsys_k(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "tsys_k", value);
}

// The setting "hours" (Observing time) of the stack "pulsar". Unit: h. For the area estimate only.
static inline int32_t sdrkit_pulsar_get_hours(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "hours", value);
}
static inline int32_t sdrkit_pulsar_set_hours(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "hours", value);
}

// The setting "gen_snr" (Test signal S/N) of the test signal generator of the stack "pulsar". The power of the tones at the top of a pulse over the power of the noise in the whole band.
static inline int32_t sdrkit_pulsar_gen_get_snr(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_snr", value);
}
static inline int32_t sdrkit_pulsar_gen_set_snr(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_snr", value);
}

// The setting "gen_period_s" (Test signal period) of the test signal generator of the stack "pulsar". Unit: s. The period of the pulses of the test signal. 0.5 s is a test value, not a known pulsar.
static inline int32_t sdrkit_pulsar_gen_get_period_s(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_period_s", value);
}
static inline int32_t sdrkit_pulsar_gen_set_period_s(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_period_s", value);
}

// The setting "gen_dm" (Test signal DM) of the test signal generator of the stack "pulsar". Unit: pc cm⁻³. The dispersion measure of the pulses of the test signal, for a radio centred at 408 MHz. 20 is a test value.
static inline int32_t sdrkit_pulsar_gen_get_dm(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_dm", value);
}
static inline int32_t sdrkit_pulsar_gen_set_dm(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_dm", value);
}

// Fills `rows` with the changed rows of the table "radar_rows" of the stack "radar_detect" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The rows are a log: they arrive over time.
// Time: `signal_time_s`.
static inline int32_t sdrkit_radar_detect_radar_rows(SDRKIT_HANDLE handle,
                                                    sdrkit_radar_detect_row *rows,
                                                    uint32_t capacity,
                                                    sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "science.RadarDetectRow", "radar_rows", rows, sizeof(sdrkit_radar_detect_row), capacity, info);
}

// Fills `event` with event `n` of the last sdrkit_events_take() on `handle`, an event of the type "radar.row"
// (Radar row). Returns SDRKIT_E_WRONG_TYPE if the event is of another type.
static inline int32_t sdrkit_radar_detect_radar_row_event_read(SDRKIT_HANDLE handle, uint32_t n, sdrkit_radar_detect_radar_row_event *event) {
  return sdrkit_typed_event(handle, n, "radar.row", "science.RadarDetectRowEvent", event, sizeof(sdrkit_radar_detect_radar_row_event));
}

// Runs the command "clear" (Clear) of the stack "radar_detect" on the session of `handle`. Forgets everything decoded so far.
static inline int32_t sdrkit_radar_detect_clear(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "clear", NULL, NULL, NULL, NULL, 0);
}

// The settings of the test signal generator of the stack "radar_detect" are the sdrkit_radar_detect_gen_*() functions
// after the settings of its receiver.

// The setting "gen_rpm" (Test signal rotation) of the test signal generator of the stack "radar_detect". Unit: rpm. Rotations per minute of the beam of the test signal.
static inline int32_t sdrkit_radar_detect_gen_get_rpm(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_rpm", value);
}
static inline int32_t sdrkit_radar_detect_gen_set_rpm(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_rpm", value);
}

// The setting "gen_prf_hz" (Test signal PRF) of the test signal generator of the stack "radar_detect". A frequency in Hz. Pulses per second of the test signal; with a staggered train, the higher of the two.
static inline int32_t sdrkit_radar_detect_gen_get_prf_hz(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_prf_hz", value);
}
static inline int32_t sdrkit_radar_detect_gen_set_prf_hz(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_prf_hz", value);
}

// The setting "gen_dual_prf" (Test signal staggered PRF) of the test signal generator of the stack "radar_detect". A value of `sdrkit_radar_detect_gen_dual_prf`. Dual: the intervals alternate 1 / PRF and 1 / (0.75 PRF), as 1200 and 900 Hz.
static inline int32_t sdrkit_radar_detect_gen_get_dual_prf(SDRKIT_HANDLE handle, sdrkit_radar_detect_gen_dual_prf *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "gen_dual_prf", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_radar_detect_gen_dual_prf)index;
  return code;
}
static inline int32_t sdrkit_radar_detect_gen_set_dual_prf(SDRKIT_HANDLE handle, sdrkit_radar_detect_gen_dual_prf value) {
  return sdrkit_typed_setting_set_enum(handle, "gen_dual_prf", (int32_t)value);
}

// Fills `rows` with the changed rows of the table "payloads" of the stack "horus" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The column `payload` names a row.
// Place of a row: the column `position`.
// Height: `height_m`. Speed: `speed_kmh`.
static inline int32_t sdrkit_horus_payloads_rows(SDRKIT_HANDLE handle,
                                                sdrkit_horus_payload_row *rows,
                                                uint32_t capacity,
                                                sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "science.HorusPayloadRow", "payloads", rows, sizeof(sdrkit_horus_payload_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "custom_fields" of the stack "horus" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The column `name` names a row.
static inline int32_t sdrkit_horus_custom_fields_rows(SDRKIT_HANDLE handle,
                                                     sdrkit_horus_custom_field_row *rows,
                                                     uint32_t capacity,
                                                     sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "science.HorusCustomFieldRow", "custom_fields", rows, sizeof(sdrkit_horus_custom_field_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "packets" of the stack "horus" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The rows are a log: they arrive over time.
// The column `payload` names a row.
// Time: `received`.
static inline int32_t sdrkit_horus_packets_rows(SDRKIT_HANDLE handle,
                                               sdrkit_horus_packet_row *rows,
                                               uint32_t capacity,
                                               sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "science.HorusPacketRow", "packets", rows, sizeof(sdrkit_horus_packet_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "channels" of the stack "horus" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_horus_channels_rows(SDRKIT_HANDLE handle,
                                                sdrkit_horus_channel_row *rows,
                                                uint32_t capacity,
                                                sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "science.HorusChannelRow", "channels", rows, sizeof(sdrkit_horus_channel_row), capacity, info);
}

// Fills `event` with event `n` of the last sdrkit_events_take() on `handle`, an event of the type "horus.packet"
// (Horus packet). Returns SDRKIT_E_WRONG_TYPE if the event is of another type.
static inline int32_t sdrkit_horus_packet_event_read(SDRKIT_HANDLE handle, uint32_t n, sdrkit_horus_packet_event *event) {
  return sdrkit_typed_event(handle, n, "horus.packet", "science.HorusPacketEvent", event, sizeof(sdrkit_horus_packet_event));
}

// Fills `event` with event `n` of the last sdrkit_events_take() on `handle`, an event of the type "horus.channel_opened"
// (Channel opened). Returns SDRKIT_E_WRONG_TYPE if the event is of another type.
static inline int32_t sdrkit_horus_channel_opened_event_read(SDRKIT_HANDLE handle, uint32_t n, sdrkit_horus_channel_opened_event *event) {
  return sdrkit_typed_event(handle, n, "horus.channel_opened", "science.HorusChannelOpenedEvent", event, sizeof(sdrkit_horus_channel_opened_event));
}

// Fills `event` with event `n` of the last sdrkit_events_take() on `handle`, an event of the type "horus.channel_closed"
// (Channel closed). Returns SDRKIT_E_WRONG_TYPE if the event is of another type.
static inline int32_t sdrkit_horus_channel_closed_event_read(SDRKIT_HANDLE handle, uint32_t n, sdrkit_horus_channel_closed_event *event) {
  return sdrkit_typed_event(handle, n, "horus.channel_closed", "science.HorusChannelClosedEvent", event, sizeof(sdrkit_horus_channel_closed_event));
}

// Runs the command "clear" (Clear) of the stack "horus" on the session of `handle`. Forgets everything heard so far. Favourites stay.
static inline int32_t sdrkit_horus_clear(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "clear", NULL, NULL, NULL, NULL, 0);
}

// The settings of the test signal generator of the stack "horus" are the sdrkit_horus_gen_*() functions
// after the settings of its receiver.

// The setting "span_search" (Look for other balloons in the span) of the stack "horus". Opens a channel on each narrow signal of the span, at 40 ksps and more.
static inline int32_t sdrkit_horus_get_span_search(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "span_search", value);
}
static inline int32_t sdrkit_horus_set_span_search(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "span_search", value);
}

// The setting "max_channels" (Channels at most) of the stack "horus". The dial channel and the channels of the span search; 1 is the dial only.
static inline int32_t sdrkit_horus_get_max_channels(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "max_channels", value);
}
static inline int32_t sdrkit_horus_set_max_channels(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "max_channels", value);
}

// The setting "payload_ids" (Payload IDs) of the stack "horus". One "id, callsign" a line (id 0 to 65535); # starts a comment; comma, tab, semicolon and spaces separate. An ID that the list does not name is shown as "ID n".
static inline int32_t sdrkit_horus_get_payload_ids(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "payload_ids", value);
}
static inline int32_t sdrkit_horus_set_payload_ids(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "payload_ids", value);
}

// The setting "custom_formats" (Custom fields of v2) of the stack "horus". One format a line: callsign | struct | name:post-processing, ... | sentence names, ... | other payload IDs, ... | comment. Post-processing: none, battery_5v_byte, divide_by_10, divide_by_100. A list with a line that does not read is refused.
static inline int32_t sdrkit_horus_get_custom_formats(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "custom_formats", value);
}
static inline int32_t sdrkit_horus_set_custom_formats(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "custom_formats", value);
}

// The setting "gen_content" (Content) of the test signal generator of the stack "horus". A value of `sdrkit_horus_gen_content`. Binary v2 or v1, RTTY at 100 baud 7N2, 300 baud 8N2 or 50 baud 7N2, or three balloons (v2, v1 12 kHz above, RTTY 18 kHz below).
static inline int32_t sdrkit_horus_gen_get_content(SDRKIT_HANDLE handle, sdrkit_horus_gen_content *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "gen_content", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_horus_gen_content)index;
  return code;
}
static inline int32_t sdrkit_horus_gen_set_content(SDRKIT_HANDLE handle, sdrkit_horus_gen_content value) {
  return sdrkit_typed_setting_set_enum(handle, "gen_content", (int32_t)value);
}

// The setting "gen_spacing_hz" (Tone spacing) of the test signal generator of the stack "horus". Unit: Hz. 0 for 270 Hz (4FSK) and 500 Hz (RTTY).
static inline int32_t sdrkit_horus_gen_get_spacing_hz(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_spacing_hz", value);
}
static inline int32_t sdrkit_horus_gen_set_spacing_hz(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_spacing_hz", value);
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_sonde_fields_read(SDRKIT_HANDLE handle, sdrkit_sonde_fields *fields) {
  return sdrkit_typed_fields(handle, "science.SondeFields", fields, sizeof(sdrkit_sonde_fields));
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_time_fields_read(SDRKIT_HANDLE handle, sdrkit_time_fields *fields) {
  return sdrkit_typed_fields(handle, "science.TimeFields", fields, sizeof(sdrkit_time_fields));
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_eczas_fields_read(SDRKIT_HANDLE handle, sdrkit_eczas_fields *fields) {
  return sdrkit_typed_fields(handle, "science.EczasFields", fields, sizeof(sdrkit_eczas_fields));
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_same_fields_read(SDRKIT_HANDLE handle, sdrkit_same_fields *fields) {
  return sdrkit_typed_fields(handle, "science.SameFields", fields, sizeof(sdrkit_same_fields));
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_mscatter_fields_read(SDRKIT_HANDLE handle, sdrkit_mscatter_fields *fields) {
  return sdrkit_typed_fields(handle, "science.MscatterFields", fields, sizeof(sdrkit_mscatter_fields));
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_ssdv_fields_read(SDRKIT_HANDLE handle, sdrkit_ssdv_fields *fields) {
  return sdrkit_typed_fields(handle, "science.SsdvFields", fields, sizeof(sdrkit_ssdv_fields));
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_astro_fields_read(SDRKIT_HANDLE handle, sdrkit_astro_fields *fields) {
  return sdrkit_typed_fields(handle, "science.AstroFields", fields, sizeof(sdrkit_astro_fields));
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_iono_fields_read(SDRKIT_HANDLE handle, sdrkit_iono_fields *fields) {
  return sdrkit_typed_fields(handle, "science.IonoFields", fields, sizeof(sdrkit_iono_fields));
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_sferics_fields_read(SDRKIT_HANDLE handle, sdrkit_sferics_fields *fields) {
  return sdrkit_typed_fields(handle, "science.SfericsFields", fields, sizeof(sdrkit_sferics_fields));
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_science_decoder_fields_read(SDRKIT_HANDLE handle, sdrkit_science_decoder_fields *fields) {
  return sdrkit_typed_fields(handle, "science.DecoderFields", fields, sizeof(sdrkit_science_decoder_fields));
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_supersid_fields_read(SDRKIT_HANDLE handle, sdrkit_supersid_fields *fields) {
  return sdrkit_typed_fields(handle, "science.SupersidFields", fields, sizeof(sdrkit_supersid_fields));
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_schumann_fields_read(SDRKIT_HANDLE handle, sdrkit_schumann_fields *fields) {
  return sdrkit_typed_fields(handle, "science.SchumannFields", fields, sizeof(sdrkit_schumann_fields));
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_whistler_fields_read(SDRKIT_HANDLE handle, sdrkit_whistler_fields *fields) {
  return sdrkit_typed_fields(handle, "science.WhistlerFields", fields, sizeof(sdrkit_whistler_fields));
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_lightning_tdoa_fields_read(SDRKIT_HANDLE handle, sdrkit_lightning_tdoa_fields *fields) {
  return sdrkit_typed_fields(handle, "science.LightningTdoaFields", fields, sizeof(sdrkit_lightning_tdoa_fields));
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_science_burst_fields_read(SDRKIT_HANDLE handle, sdrkit_science_burst_fields *fields) {
  return sdrkit_typed_fields(handle, "science.BurstFields", fields, sizeof(sdrkit_science_burst_fields));
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_riometer_fields_read(SDRKIT_HANDLE handle, sdrkit_riometer_fields *fields) {
  return sdrkit_typed_fields(handle, "science.RiometerFields", fields, sizeof(sdrkit_riometer_fields));
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_eme_echo_fields_read(SDRKIT_HANDLE handle, sdrkit_eme_echo_fields *fields) {
  return sdrkit_typed_fields(handle, "science.EmeEchoFields", fields, sizeof(sdrkit_eme_echo_fields));
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_beacon_events_fields_read(SDRKIT_HANDLE handle, sdrkit_beacon_events_fields *fields) {
  return sdrkit_typed_fields(handle, "science.BeaconEventsFields", fields, sizeof(sdrkit_beacon_events_fields));
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_pulsar_fields_read(SDRKIT_HANDLE handle, sdrkit_pulsar_fields *fields) {
  return sdrkit_typed_fields(handle, "science.PulsarFields", fields, sizeof(sdrkit_pulsar_fields));
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_radar_detect_fields_read(SDRKIT_HANDLE handle, sdrkit_radar_detect_fields *fields) {
  return sdrkit_typed_fields(handle, "science.RadarDetectFields", fields, sizeof(sdrkit_radar_detect_fields));
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_horus_fields_read(SDRKIT_HANDLE handle, sdrkit_horus_fields *fields) {
  return sdrkit_typed_fields(handle, "science.HorusFields", fields, sizeof(sdrkit_horus_fields));
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_horus_log_fields_read(SDRKIT_HANDLE handle, sdrkit_horus_log_fields *fields) {
  return sdrkit_typed_fields(handle, "science.HorusLogFields", fields, sizeof(sdrkit_horus_log_fields));
}

#endif
