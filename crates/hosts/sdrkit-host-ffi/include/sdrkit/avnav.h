/*
 * sdrkit: the typed interface of the stack family "avnav", version 1.4. MIT licence.
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

#ifndef SDRKIT_AVNAV_H
#define SDRKIT_AVNAV_H

#include "../sdrkit.h"

// The version of the family that this header describes.
#define SDRKIT_AVNAV_VERSION_MAJOR 1
#define SDRKIT_AVNAV_VERSION_MINOR 4

// The stack "ndb": its id, and the keys of its event types.
#define SDRKIT_STACK_NDB "ndb"
#define SDRKIT_EVENT_MORSE_WORD "morse.word"

// The stack "vor": its id, and the keys of its event types.
#define SDRKIT_STACK_VOR "vor"

// The stack "ils": its id, and the keys of its event types.
#define SDRKIT_STACK_ILS "ils"
#define SDRKIT_EVENT_ILS_MARKER "ils.marker"

// The stack "dme": its id, and the keys of its event types.
#define SDRKIT_STACK_DME "dme"

// The stack "loran": its id, and the keys of its event types.
#define SDRKIT_STACK_LORAN "loran"

// The stack "dgps": its id, and the keys of its event types.
#define SDRKIT_STACK_DGPS "dgps"
#define SDRKIT_EVENT_RTCM_FRAME "rtcm.frame"

// The options of the setting "ident_keying" of the stack "ndb".
typedef enum sdrkit_ndb_ident_keying {
  // Automatic
  SDRKIT_NDB_IDENT_KEYING_AUTO = 0,
  // Keyed tone (A2A)
  SDRKIT_NDB_IDENT_KEYING_A = 1,
  // Keyed carrier (A1A)
  SDRKIT_NDB_IDENT_KEYING_B = 2,
} sdrkit_ndb_ident_keying;

// The options of the setting "audio" of the stack "ndb".
typedef enum sdrkit_ndb_audio {
  // AM
  SDRKIT_NDB_AUDIO_AM = 0,
  // Beat note
  SDRKIT_NDB_AUDIO_CW = 1,
} sdrkit_ndb_audio;

// The options of the setting "gen_keying" of the stack "ndb".
typedef enum sdrkit_ndb_gen_keying {
  // Keyed tone
  SDRKIT_NDB_GEN_KEYING_A2A = 0,
  // Keyed carrier
  SDRKIT_NDB_GEN_KEYING_A1A = 1,
} sdrkit_ndb_gen_keying;

// The options of the setting "gen_kind" of the stack "vor".
typedef enum sdrkit_vor_gen_kind {
  // Conventional VOR
  SDRKIT_VOR_GEN_KIND_CONVENTIONAL = 0,
  // Doppler VOR
  SDRKIT_VOR_GEN_KIND_DOPPLER = 1,
} sdrkit_vor_gen_kind;

// The options of the setting "part" of the stack "ils".
typedef enum sdrkit_ils_part {
  // By frequency
  SDRKIT_ILS_PART_AUTO = 0,
  // Localizer
  SDRKIT_ILS_PART_LOCALIZER = 1,
  // Glide path
  SDRKIT_ILS_PART_GLIDESLOPE = 2,
  // Marker beacon
  SDRKIT_ILS_PART_MARKER = 3,
} sdrkit_ils_part;

// The options of the setting "gen_marker" of the stack "ils".
typedef enum sdrkit_ils_gen_marker {
  // Outer
  SDRKIT_ILS_GEN_MARKER_OUTER = 0,
  // Middle
  SDRKIT_ILS_GEN_MARKER_MIDDLE = 1,
  // Inner
  SDRKIT_ILS_GEN_MARKER_INNER = 2,
} sdrkit_ils_gen_marker;

// The options of the setting "gen_marker_keying" of the stack "ils".
typedef enum sdrkit_ils_gen_marker_keying {
  // As the standard
  SDRKIT_ILS_GEN_MARKER_KEYING_STANDARD = 0,
  // Steady tone
  SDRKIT_ILS_GEN_MARKER_KEYING_STEADY = 1,
} sdrkit_ils_gen_marker_keying;

// The options of the setting "mode" of the stack "dme".
typedef enum sdrkit_dme_mode {
  // Automatic
  SDRKIT_DME_MODE_AUTO = 0,
  // X
  SDRKIT_DME_MODE_X = 1,
  // Y
  SDRKIT_DME_MODE_Y = 2,
} sdrkit_dme_mode;

// The options of the setting "gen_mode" of the stack "dme".
typedef enum sdrkit_dme_gen_mode {
  // X
  SDRKIT_DME_GEN_MODE_X = 0,
  // Y
  SDRKIT_DME_GEN_MODE_Y = 1,
} sdrkit_dme_gen_mode;

// The options of the setting "gen_kind" of the stack "dme".
typedef enum sdrkit_dme_gen_kind {
  // Ground beacon
  SDRKIT_DME_GEN_KIND_BEACON = 0,
  // Interrogating aircraft
  SDRKIT_DME_GEN_KIND_AIRCRAFT = 1,
} sdrkit_dme_gen_kind;

// The options of the setting "bitrate" of the stack "dgps".
typedef enum sdrkit_dgps_bitrate {
  // Automatic
  SDRKIT_DGPS_BITRATE_AUTOMATIC = 0,
  // 50 bps
  SDRKIT_DGPS_BITRATE_50_BPS = 1,
  // 100 bps
  SDRKIT_DGPS_BITRATE_100_BPS = 2,
  // 200 bps
  SDRKIT_DGPS_BITRATE_200_BPS = 3,
} sdrkit_dgps_bitrate;

// The options of the setting "gen_bitrate" of the stack "dgps".
typedef enum sdrkit_dgps_gen_bitrate {
  // 50
  SDRKIT_DGPS_GEN_BITRATE_50 = 0,
  // 100
  SDRKIT_DGPS_GEN_BITRATE_100 = 1,
  // 200
  SDRKIT_DGPS_GEN_BITRATE_200 = 2,
} sdrkit_dgps_gen_bitrate;

// The tuned frequency.
typedef struct sdrkit_avnav_tuned_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The tuned frequency. A frequency in Hz.
  double tuned_hz;
} sdrkit_avnav_tuned_fields;

// The identification of the station.
typedef struct sdrkit_avnav_ident_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // True if `ident` holds a value.
  bool has_ident;
  // The confirmed identification. No value before one is confirmed.
  sdrkit_string ident;
  // True if `last_ident` holds a value.
  bool has_last_ident;
  // The last identification heard. No value before a word is heard.
  sdrkit_string last_ident;
  // True if `ident_wpm` holds a value.
  bool has_ident_wpm;
  // The Morse speed of the last identification. No value before a word is heard. Unit: wpm.
  double ident_wpm;
} sdrkit_avnav_ident_fields;

// The carrier of the station.
typedef struct sdrkit_avnav_carrier_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // A carrier stands clear of the noise.
  bool carrier;
  // True if `carrier_hz` holds a value.
  bool has_carrier_hz;
  // The frequency of the carrier. No value before the first measurement. A frequency in Hz.
  double carrier_hz;
  // True if `cn0_dbhz` holds a value.
  bool has_cn0_dbhz;
  // The carrier to noise density. No value before the first measurement. Unit: dB-Hz.
  double cn0_dbhz;
  // True if `offset_hz` holds a value.
  bool has_offset_hz;
  // The offset of the carrier from the tuned frequency. No value before the first measurement. A frequency in Hz.
  double offset_hz;
  // True if `level_dbfs` holds a value.
  bool has_level_dbfs;
  // The level of the carrier. No value before the first measurement. Unit: dBFS.
  double level_dbfs;
} sdrkit_avnav_carrier_fields;

// The identification that the average of the repeats reads. No value while the average reads nothing.
typedef struct sdrkit_ndb_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // True if `ident_period_s` holds a value.
  bool has_ident_period_s;
  // The period of the identification. Unit: s.
  double ident_period_s;
  // True if `ident_period_z` holds a value.
  bool has_ident_period_z;
  // The robust z-score of the autocorrelation at that period.
  double ident_period_z;
  // True if `ident_fold_quality` holds a value.
  bool has_ident_fold_quality;
  // The swing of the average of the repeats over its noise.
  double ident_fold_quality;
  // True if `ident_fold_text` holds a value.
  bool has_ident_fold_text;
  // The word read from the average.
  sdrkit_string ident_fold_text;
} sdrkit_ndb_fields;

// The identification tone that the tone keyer listens to.
typedef struct sdrkit_avnav_keyer_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // True if `id_tone_hz` holds a value.
  bool has_id_tone_hz;
  // The tone: the setting, or the tone found. No value before the first input. A frequency in Hz.
  double id_tone_hz;
  // True if `id_depth_pct` holds a value.
  bool has_id_depth_pct;
  // The modulation depth of the tone. No value before the first input. Unit: %.
  double id_depth_pct;
} sdrkit_avnav_keyer_fields;

// A row of the identifications heard. The row key is the word.
typedef struct sdrkit_avnav_ident_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The word.
  sdrkit_string ident;
  // How often it was heard.
  int64_t heard;
} sdrkit_avnav_ident_row;

// A row of the band scan. The row key is the frequency in whole hertz.
typedef struct sdrkit_ndb_scan_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The frequency of the carrier. A frequency in Hz.
  double freq_hz;
  // The level of the carrier. Unit: dBFS.
  double level_dbfs;
  // The carrier to noise density. Unit: dB-Hz.
  double cn0_dbhz;
} sdrkit_ndb_scan_row;

// The options of `to_from` of `sdrkit_vor_fields`.
typedef enum sdrkit_vor_fields_to_from {
  // TO
  SDRKIT_VOR_FIELDS_TO_FROM_TO = 0,
  // FROM
  SDRKIT_VOR_FIELDS_TO_FROM_FROM = 1,
  // OFF
  SDRKIT_VOR_FIELDS_TO_FROM_OFF = 2,
} sdrkit_vor_fields_to_from;

// The result fields of VOR.
typedef struct sdrkit_vor_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // True if `radial_deg` holds a value.
  bool has_radial_deg;
  // The radial. No value without a valid bearing. Unit: °.
  double radial_deg;
  // True if `bearing_to_deg` holds a value.
  bool has_bearing_to_deg;
  // The bearing to the station: the radial plus 180°. No value without a valid bearing. Unit: °.
  double bearing_to_deg;
  // The selected course. Unit: °.
  double obs_deg;
  // TO, FROM, or OFF (the flag: no valid bearing). A value of `sdrkit_vor_fields_to_from`.
  int32_t to_from;
  // True if `cdi_deg` holds a value.
  bool has_cdi_deg;
  // The course deviation, positive to fly right. No value without a valid bearing. Unit: °.
  double cdi_deg;
  // True if `cdi_dots` holds a value.
  bool has_cdi_dots;
  // The course deviation in dots. No value without a valid bearing. Unit: dots.
  double cdi_dots;
  // True if `am30_pct` holds a value.
  bool has_am30_pct;
  // The depth of the 30 Hz AM. No value before the first measurement. Unit: %.
  double am30_pct;
  // True if `subcarrier_pct` holds a value.
  bool has_subcarrier_pct;
  // The depth of the subcarrier. No value before the first measurement. Unit: %.
  double subcarrier_pct;
  // True if `deviation_hz` holds a value.
  bool has_deviation_hz;
  // The peak deviation of the subcarrier. No value before the first measurement. A frequency in Hz.
  double deviation_hz;
  // True if `steadiness` holds a value.
  bool has_steadiness;
  // How steady the bearing is, 0 (random) to 1. No value before the first measurement.
  double steadiness;
  // Measurements made.
  int64_t fixes;
  // The tuned frequency. A frequency in Hz.
  double tuned_hz;
  // True if `dme_channel` holds a value.
  bool has_dme_channel;
  // The DME channel paired with the frequency ("17X"). No value for a frequency without one.
  sdrkit_string dme_channel;
} sdrkit_vor_fields;

// The options of `part` of `sdrkit_ils_fields`.
typedef enum sdrkit_ils_fields_part {
  // Localizer
  SDRKIT_ILS_FIELDS_PART_LOCALIZER = 0,
  // Glide path
  SDRKIT_ILS_FIELDS_PART_GLIDESLOPE = 1,
  // Marker beacon
  SDRKIT_ILS_FIELDS_PART_MARKER = 2,
} sdrkit_ils_fields_part;

// The options of `command` of `sdrkit_ils_fields`.
typedef enum sdrkit_ils_fields_command {
  // on course
  SDRKIT_ILS_FIELDS_COMMAND_ON_COURSE = 0,
  // on path
  SDRKIT_ILS_FIELDS_COMMAND_ON_PATH = 1,
  // fly left
  SDRKIT_ILS_FIELDS_COMMAND_FLY_LEFT = 2,
  // fly right
  SDRKIT_ILS_FIELDS_COMMAND_FLY_RIGHT = 3,
  // fly up
  SDRKIT_ILS_FIELDS_COMMAND_FLY_UP = 4,
  // fly down
  SDRKIT_ILS_FIELDS_COMMAND_FLY_DOWN = 5,
  // flag
  SDRKIT_ILS_FIELDS_COMMAND_FLAG = 6,
} sdrkit_ils_fields_command;

// The result fields of ILS.
typedef struct sdrkit_ils_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Localizer or glide path. A value of `sdrkit_ils_fields_part`.
  int32_t part;
  // True if `ddm` holds a value.
  bool has_ddm;
  // The difference in depth of modulation (90 Hz less 150 Hz). No value while the flag is out.
  double ddm;
  // True if `sdm_pct` holds a value.
  bool has_sdm_pct;
  // The sum in depth of modulation. No value before the first measurement. Unit: %.
  double sdm_pct;
  // True if `m90_pct` holds a value.
  bool has_m90_pct;
  // The depth of the 90 Hz tone. No value before the first measurement. Unit: %.
  double m90_pct;
  // True if `m150_pct` holds a value.
  bool has_m150_pct;
  // The depth of the 150 Hz tone. No value before the first measurement. Unit: %.
  double m150_pct;
  // True if `needle_ua` holds a value.
  bool has_needle_ua;
  // The needle, positive to fly right or up, limited to 150 µA either way. No value while the flag is out. Unit: µA.
  double needle_ua;
  // True if `needle_dots` holds a value.
  bool has_needle_dots;
  // The needle in dots (75 µA a dot), limited to 2.5 either way. No value while the flag is out. Unit: dots.
  double needle_dots;
  // What the needle says. A value of `sdrkit_ils_fields_command`.
  int32_t command;
  // The flag is out: no carrier, an SDM far from its nominal value, or the marker part, which has no needle.
  bool flag;
  // True if `paired_hz` holds a value.
  bool has_paired_hz;
  // The paired localizer or glide path frequency. No value for a frequency without one. A frequency in Hz.
  double paired_hz;
  // The tuned frequency. A frequency in Hz.
  double tuned_hz;
  // True if `dme_channel` holds a value.
  bool has_dme_channel;
  // The DME channel paired with the localizer ("17X"). No value for a frequency without one.
  sdrkit_string dme_channel;
} sdrkit_ils_fields;

// The options of `marker` of `sdrkit_ils_marker_fields`.
typedef enum sdrkit_ils_marker_fields_marker {
  // None
  SDRKIT_ILS_MARKER_FIELDS_MARKER_NONE = 0,
  // Outer
  SDRKIT_ILS_MARKER_FIELDS_MARKER_OUTER = 1,
  // Middle
  SDRKIT_ILS_MARKER_FIELDS_MARKER_MIDDLE = 2,
  // Inner
  SDRKIT_ILS_MARKER_FIELDS_MARKER_INNER = 3,
} sdrkit_ils_marker_fields_marker;

// The options of `pattern` of `sdrkit_ils_marker_fields`.
typedef enum sdrkit_ils_marker_fields_pattern {
  // Not yet
  SDRKIT_ILS_MARKER_FIELDS_PATTERN_NOT_YET = 0,
  // Dashes
  SDRKIT_ILS_MARKER_FIELDS_PATTERN_DASHES = 1,
  // Dot-dash
  SDRKIT_ILS_MARKER_FIELDS_PATTERN_DOT_DASH = 2,
  // Dots
  SDRKIT_ILS_MARKER_FIELDS_PATTERN_DOTS = 3,
  // No pattern
  SDRKIT_ILS_MARKER_FIELDS_PATTERN_IRREGULAR = 4,
} sdrkit_ils_marker_fields_pattern;

// The marker beacon (the part `marker`): the passage under way, or the last one, and the live indication.
typedef struct sdrkit_ils_marker_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The marker of the passage, named by its tone. None before the first passage of three elements and outside the marker part. A value of `sdrkit_ils_marker_fields_marker`.
  int32_t marker;
  // True if `marker_tone_hz` holds a value.
  bool has_marker_tone_hz;
  // The tone of the passage, measured. No value without a passage. A frequency in Hz.
  double marker_tone_hz;
  // True if `marker_depth_pct` holds a value.
  bool has_marker_depth_pct;
  // The modulation depth of the tone. No value without a passage. Unit: %.
  double marker_depth_pct;
  // True if `keying` holds a value.
  bool has_keying;
  // The keying as text ("2.0 dashes per second (0.30 s long)"). No value without a passage.
  sdrkit_string keying;
  // True if `keying_rate` holds a value.
  bool has_keying_rate;
  // Elements a second, from the first onset to the last. No value without a passage. Unit: /s.
  double keying_rate;
  // True if `pairs_per_min` holds a value.
  bool has_pairs_per_min;
  // Dot-dash pairs a minute. No value without mixed keying. Unit: /min.
  double pairs_per_min;
  // True if `dot_s` holds a value.
  bool has_dot_s;
  // The mean dot length. No value without dots. Unit: s.
  double dot_s;
  // True if `dash_s` holds a value.
  bool has_dash_s;
  // The mean dash length. No value without dashes. Unit: s.
  double dash_s;
  // True if `keying_fits` holds a value.
  bool has_keying_fits;
  // The keying fits the standard (ICAO Annex 10 Volume I, 3.1.7.5). No value without a passage.
  bool keying_fits;
  // True if `passage_s` holds a value.
  bool has_passage_s;
  // The length of the passage. No value without a passage. Unit: s.
  double passage_s;
  // Passages finished, those of fewer than three elements not counted.
  int64_t passages;
  // The tone with the greatest held depth keys the last period.
  bool keyed_now;
  // True if `held_depth_pct` holds a value.
  bool has_held_depth_pct;
  // The held depth of that tone. No value outside the marker part. Unit: %.
  double held_depth_pct;
  // The pattern of the last on-lengths of that tone. A value of `sdrkit_ils_marker_fields_pattern`.
  int32_t pattern;
} sdrkit_ils_marker_fields;

// The options of `kind` of `sdrkit_ils_marker_passage_row`.
typedef enum sdrkit_ils_marker_passage_row_kind {
  // Marker
  SDRKIT_ILS_MARKER_PASSAGE_ROW_KIND_MARKER = 0,
  // Tone only
  SDRKIT_ILS_MARKER_PASSAGE_ROW_KIND_TONE_ONLY = 1,
} sdrkit_ils_marker_passage_row_kind;

// The options of `marker` of `sdrkit_ils_marker_passage_row`.
typedef enum sdrkit_ils_marker_passage_row_marker {
  // None
  SDRKIT_ILS_MARKER_PASSAGE_ROW_MARKER_NONE = 0,
  // Outer
  SDRKIT_ILS_MARKER_PASSAGE_ROW_MARKER_OUTER = 1,
  // Middle
  SDRKIT_ILS_MARKER_PASSAGE_ROW_MARKER_MIDDLE = 2,
  // Inner
  SDRKIT_ILS_MARKER_PASSAGE_ROW_MARKER_INNER = 3,
} sdrkit_ils_marker_passage_row_marker;

// A row of the marker passages. The row key is the number of the passage.
typedef struct sdrkit_ils_marker_passage_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // A marker, or a marker tone whose keying does not fit the standard. A value of `sdrkit_ils_marker_passage_row_kind`.
  int32_t kind;
  // The marker of the tone. A value of `sdrkit_ils_marker_passage_row_marker`.
  int32_t marker;
  // The tone, the keying, the depth and the length ("400 Hz, 2.0 dashes per second (0.30 s long), depth 95 %, 7.8 s").
  sdrkit_string text;
  // True if `level_dbfs` holds a value.
  bool has_level_dbfs;
  // The carrier level at the end of the passage. No value before the first carrier measurement. Unit: dBFS.
  double level_dbfs;
} sdrkit_ils_marker_passage_row;

// The options of `heard` of `sdrkit_dme_fields`.
typedef enum sdrkit_dme_fields_heard {
  // beacon
  SDRKIT_DME_FIELDS_HEARD_BEACON = 0,
  // interrogations
  SDRKIT_DME_FIELDS_HEARD_INTERROGATIONS = 1,
  // none
  SDRKIT_DME_FIELDS_HEARD_NONE = 2,
} sdrkit_dme_fields_heard;

// The options of `channel_mode` of `sdrkit_dme_fields`.
typedef enum sdrkit_dme_fields_channel_mode {
  // X
  SDRKIT_DME_FIELDS_CHANNEL_MODE_X = 0,
  // Y
  SDRKIT_DME_FIELDS_CHANNEL_MODE_Y = 1,
} sdrkit_dme_fields_channel_mode;

// The result fields of DME, over the last second of pairs.
typedef struct sdrkit_dme_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // What is heard. A value of `sdrkit_dme_fields_heard`.
  int32_t heard;
  // True if `channel_mode` holds a value.
  bool has_channel_mode;
  // X or Y, from the pulse spacing. No value with too few pairs. A value of `sdrkit_dme_fields_channel_mode`.
  int32_t channel_mode;
  // True if `channel` holds a value.
  bool has_channel;
  // The channel from the tuned frequency and the mode ("17X"). No value with too few pairs.
  sdrkit_string channel;
  // True if `paired_vhf_hz` holds a value.
  bool has_paired_vhf_hz;
  // The paired VOR or localizer frequency. No value without a channel that has one. A frequency in Hz.
  double paired_vhf_hz;
  // Pairs a second. Unit: /s.
  double pairs_per_s;
  // True if `spacing_us` holds a value.
  bool has_spacing_us;
  // The mean pair spacing. No value with too few pairs. Unit: µs.
  double spacing_us;
  // True if `width_us` holds a value.
  bool has_width_us;
  // The mean pulse width at half amplitude. No value with too few pairs. Unit: µs.
  double width_us;
  // Pairs seen in all.
  int64_t pairs;
  // True if `level_dbfs` holds a value.
  bool has_level_dbfs;
  // The mean level of the pulses. No value with too few pairs. Unit: dBFS.
  double level_dbfs;
} sdrkit_dme_fields;

// The result fields of Loran. A field of the chain has no value before the first measurement.
typedef struct sdrkit_loran_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // True if `gri` holds a value.
  bool has_gri;
  // The GRI in use, in tens of microseconds.
  int64_t gri;
  // True if `chain` holds a value.
  bool has_chain;
  // The chain of that GRI. No value for a GRI that the `chains` setting does not name.
  sdrkit_string chain;
  // The master is found: its correlation SNR is at least 10 dB.
  bool master;
  // True if `master_snr_db` holds a value.
  bool has_master_snr_db;
  // The correlation SNR of the master. Unit: dB.
  double master_snr_db;
  // Measurements made.
  int64_t fixes;
} sdrkit_loran_fields;

// A row of the stations of the chain. The row key is the letter of the station.
typedef struct sdrkit_loran_station_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // `M` for the master, then the letters of the secondaries.
  sdrkit_string station;
  // The time difference from the master. Unit: µs.
  double td_us;
  // The envelope-to-cycle difference relative to the master. Unit: µs.
  double ecd_us;
  // The signal to noise ratio of the correlation. Unit: dB.
  double snr_db;
  // The level relative to the master. Unit: dB.
  double level_db;
} sdrkit_loran_station_row;

// The options of `health` of `sdrkit_dgps_fields`.
typedef enum sdrkit_dgps_fields_health {
  // UDRE scale 1
  SDRKIT_DGPS_FIELDS_HEALTH_SCALE_1 = 0,
  // UDRE scale 0.75
  SDRKIT_DGPS_FIELDS_HEALTH_SCALE_0_75 = 1,
  // UDRE scale 0.5
  SDRKIT_DGPS_FIELDS_HEALTH_SCALE_0_5 = 2,
  // UDRE scale 0.3
  SDRKIT_DGPS_FIELDS_HEALTH_SCALE_0_3 = 3,
  // UDRE scale 0.2
  SDRKIT_DGPS_FIELDS_HEALTH_SCALE_0_2 = 4,
  // UDRE scale 0.1
  SDRKIT_DGPS_FIELDS_HEALTH_SCALE_0_1 = 5,
  // not monitored
  SDRKIT_DGPS_FIELDS_HEALTH_NOT_MONITORED = 6,
  // not working
  SDRKIT_DGPS_FIELDS_HEALTH_NOT_WORKING = 7,
} sdrkit_dgps_fields_health;

// The result fields of DGPS. A field of the station has no value before the first frame.
typedef struct sdrkit_dgps_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // True if `station_id` holds a value.
  bool has_station_id;
  // The base station id.
  int64_t station_id;
  // True if `health` holds a value.
  bool has_health;
  // The health of the station. A value of `sdrkit_dgps_fields_health`.
  int32_t health;
  // True if `zcount` holds a value.
  bool has_zcount;
  // The last modified Z-count: the time into the hour. A length of time in seconds.
  double zcount;
  // True if `symbol_rate_hz` holds a value.
  bool has_symbol_rate_hz;
  // The bit rate decoded. No value before the first report of the link. A frequency in Hz.
  double symbol_rate_hz;
  // The framer of that bit rate holds the frame sync.
  bool locked;
  // Words with good parity.
  int64_t good_words;
  // Words with bad parity.
  int64_t bad_words;
  // Frames decoded.
  int64_t frames_ok;
  // True if `base_station` holds a value.
  bool has_base_station;
  // The position of the base station. No value before a message of type 3.
  sdrkit_position base_station;
  // True if `base_height_m` holds a value.
  bool has_base_height_m;
  // The height of the base station above the ellipsoid. No value before a message of type 3. Unit: m.
  double base_height_m;
  // True if `text` holds a value.
  bool has_text;
  // The last text message. No value before a message of type 16.
  sdrkit_string text;
  // True if `unhealthy` holds a value.
  bool has_unhealthy;
  // The satellites flagged unhealthy, by PRN. No value before a message of type 5.
  sdrkit_string unhealthy;
  // The corrections held.
  int64_t corrections;
} sdrkit_dgps_fields;

// The levels of the channel, from its power meter.
typedef struct sdrkit_dgps_meter_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // True if `level_dbfs` holds a value.
  bool has_level_dbfs;
  // The mean level. No value before the first tenth of a second. Unit: dBFS.
  double level_dbfs;
  // True if `peak_dbfs` holds a value.
  bool has_peak_dbfs;
  // The peak level. No value before the first tenth of a second. Unit: dBFS.
  double peak_dbfs;
} sdrkit_dgps_meter_fields;

// The results of one MSK demodulator.
typedef struct sdrkit_dgps_msk_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // True if `clock_ppm` holds a value.
  bool has_clock_ppm;
  // The error of the symbol clock. Unit: ppm.
  double clock_ppm;
  // True if `offset_hz` holds a value.
  bool has_offset_hz;
  // The carrier offset that the demodulator removes. A frequency in Hz.
  double offset_hz;
} sdrkit_dgps_msk_fields;

// The options of `health` of `sdrkit_dgps_station_row`.
typedef enum sdrkit_dgps_station_row_health {
  // UDRE scale 1
  SDRKIT_DGPS_STATION_ROW_HEALTH_SCALE_1 = 0,
  // UDRE scale 0.75
  SDRKIT_DGPS_STATION_ROW_HEALTH_SCALE_0_75 = 1,
  // UDRE scale 0.5
  SDRKIT_DGPS_STATION_ROW_HEALTH_SCALE_0_5 = 2,
  // UDRE scale 0.3
  SDRKIT_DGPS_STATION_ROW_HEALTH_SCALE_0_3 = 3,
  // UDRE scale 0.2
  SDRKIT_DGPS_STATION_ROW_HEALTH_SCALE_0_2 = 4,
  // UDRE scale 0.1
  SDRKIT_DGPS_STATION_ROW_HEALTH_SCALE_0_1 = 5,
  // not monitored
  SDRKIT_DGPS_STATION_ROW_HEALTH_NOT_MONITORED = 6,
  // not working
  SDRKIT_DGPS_STATION_ROW_HEALTH_NOT_WORKING = 7,
} sdrkit_dgps_station_row_health;

// The row of the station table: the base station of the frames received. The row key is the station id.
typedef struct sdrkit_dgps_station_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The station id.
  int64_t station_id;
  // True if `health` holds a value.
  bool has_health;
  // The health of the station. A value of `sdrkit_dgps_station_row_health`.
  int32_t health;
  // True if `position` holds a value.
  bool has_position;
  // The position of the station. No value before a message of type 3.
  sdrkit_position position;
  // True if `height_m` holds a value.
  bool has_height_m;
  // The height of the station above the ellipsoid. No value before a message of type 3. Unit: m.
  double height_m;
} sdrkit_dgps_station_row;

// A row of the frames by message type. The row key is the type in two digits.
typedef struct sdrkit_dgps_message_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The message type.
  int64_t msg_type;
  // The frames of the type.
  int64_t count;
} sdrkit_dgps_message_row;

// A row of the pseudorange corrections. The row key is the PRN in two digits.
typedef struct sdrkit_dgps_correction_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The satellite.
  int64_t prn;
  // The pseudorange correction. Unit: m.
  double prc_m;
  // The range-rate correction. Unit: m/s.
  double rrc_m_s;
  // The issue of data.
  int64_t iod;
  // The user differential range error.
  int64_t udre;
} sdrkit_dgps_correction_row;

// A row of the beacon almanac. The row key is the station id.
typedef struct sdrkit_dgps_almanac_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The station id.
  int64_t station_id;
  // The frequency of the beacon. A frequency in Hz.
  double freq_hz;
  // The bit rate of the beacon. A frequency in Hz.
  double bit_rate_hz;
  // The position of the beacon.
  sdrkit_position position;
  // The range of the beacon. Unit: km.
  double range_km;
  // The health of the beacon, 0 (healthy) to 3.
  int64_t health;
} sdrkit_dgps_almanac_row;

// The options of `family` of `sdrkit_avnav_morse_word_event`.
typedef enum sdrkit_avnav_morse_word_event_family {
  // Aircraft
  SDRKIT_AVNAV_MORSE_WORD_EVENT_FAMILY_AIRCRAFT = 0,
  // Vessel
  SDRKIT_AVNAV_MORSE_WORD_EVENT_FAMILY_VESSEL = 1,
  // APRS
  SDRKIT_AVNAV_MORSE_WORD_EVENT_FAMILY_APRS = 2,
  // Pager
  SDRKIT_AVNAV_MORSE_WORD_EVENT_FAMILY_PAGER = 3,
  // Radiosonde
  SDRKIT_AVNAV_MORSE_WORD_EVENT_FAMILY_SONDE = 4,
  // Broadcast
  SDRKIT_AVNAV_MORSE_WORD_EVENT_FAMILY_BROADCAST = 5,
  // Voice
  SDRKIT_AVNAV_MORSE_WORD_EVENT_FAMILY_VOICE = 6,
  // Message
  SDRKIT_AVNAV_MORSE_WORD_EVENT_FAMILY_MESSAGE = 7,
  // Sensor
  SDRKIT_AVNAV_MORSE_WORD_EVENT_FAMILY_SENSOR = 8,
  // Satellite
  SDRKIT_AVNAV_MORSE_WORD_EVENT_FAMILY_SATELLITE = 9,
  // Receiver
  SDRKIT_AVNAV_MORSE_WORD_EVENT_FAMILY_RECEIVER = 10,
  // Alert
  SDRKIT_AVNAV_MORSE_WORD_EVENT_FAMILY_ALERT = 11,
  // Other
  SDRKIT_AVNAV_MORSE_WORD_EVENT_FAMILY_OTHER = 12,
} sdrkit_avnav_morse_word_event_family;

// The fields of a decoded identification word: a spot of the station that sends it. The source, the text and the callsign hold the word.
typedef struct sdrkit_avnav_morse_word_event {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Seconds since the object started, in the time of the signal.
  double time;
  // The frequency of the signal in Hz.
  double freq_hz;
  // The word.
  sdrkit_string source;
  // The word.
  sdrkit_string text;
  // The family of the source: other. A value of `sdrkit_avnav_morse_word_event_family`.
  int32_t family;
  // The word.
  sdrkit_string callsign;
  // True if `locator` holds a value.
  bool has_locator;
  // No value: a beacon sends no locator.
  sdrkit_string locator;
  // True if `snr_db` holds a value.
  bool has_snr_db;
  // No value: the decoder does not measure it. Unit: dB.
  double snr_db;
  // True if `freq_hz_` holds a value.
  bool has_freq_hz_;
  // No value: the event carries the frequency. A frequency in Hz.
  double freq_hz_;
  // The emission: "CW".
  sdrkit_string mode;
  // The Morse speed. Unit: wpm.
  double wpm;
} sdrkit_avnav_morse_word_event;

// The options of `family` of `sdrkit_ils_marker_event`.
typedef enum sdrkit_ils_marker_event_family {
  // Aircraft
  SDRKIT_ILS_MARKER_EVENT_FAMILY_AIRCRAFT = 0,
  // Vessel
  SDRKIT_ILS_MARKER_EVENT_FAMILY_VESSEL = 1,
  // APRS
  SDRKIT_ILS_MARKER_EVENT_FAMILY_APRS = 2,
  // Pager
  SDRKIT_ILS_MARKER_EVENT_FAMILY_PAGER = 3,
  // Radiosonde
  SDRKIT_ILS_MARKER_EVENT_FAMILY_SONDE = 4,
  // Broadcast
  SDRKIT_ILS_MARKER_EVENT_FAMILY_BROADCAST = 5,
  // Voice
  SDRKIT_ILS_MARKER_EVENT_FAMILY_VOICE = 6,
  // Message
  SDRKIT_ILS_MARKER_EVENT_FAMILY_MESSAGE = 7,
  // Sensor
  SDRKIT_ILS_MARKER_EVENT_FAMILY_SENSOR = 8,
  // Satellite
  SDRKIT_ILS_MARKER_EVENT_FAMILY_SATELLITE = 9,
  // Receiver
  SDRKIT_ILS_MARKER_EVENT_FAMILY_RECEIVER = 10,
  // Alert
  SDRKIT_ILS_MARKER_EVENT_FAMILY_ALERT = 11,
  // Other
  SDRKIT_ILS_MARKER_EVENT_FAMILY_OTHER = 12,
} sdrkit_ils_marker_event_family;

// The options of `kind` of `sdrkit_ils_marker_event`.
typedef enum sdrkit_ils_marker_event_kind {
  // Marker
  SDRKIT_ILS_MARKER_EVENT_KIND_MARKER = 0,
  // Tone only
  SDRKIT_ILS_MARKER_EVENT_KIND_TONE_ONLY = 1,
} sdrkit_ils_marker_event_kind;

// The options of `marker` of `sdrkit_ils_marker_event`.
typedef enum sdrkit_ils_marker_event_marker {
  // None
  SDRKIT_ILS_MARKER_EVENT_MARKER_NONE = 0,
  // Outer
  SDRKIT_ILS_MARKER_EVENT_MARKER_OUTER = 1,
  // Middle
  SDRKIT_ILS_MARKER_EVENT_MARKER_MIDDLE = 2,
  // Inner
  SDRKIT_ILS_MARKER_EVENT_MARKER_INNER = 3,
} sdrkit_ils_marker_event_marker;

// The fields of a finished marker passage.
typedef struct sdrkit_ils_marker_event {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Seconds since the object started, in the time of the signal.
  double time;
  // The frequency of the signal in Hz.
  double freq_hz;
  // The marker ("Outer marker").
  sdrkit_string source;
  // The tone, the keying, the depth and the length.
  sdrkit_string text;
  // The family of the source: other. A value of `sdrkit_ils_marker_event_family`.
  int32_t family;
  // A marker, or a marker tone whose keying does not fit the standard. A value of `sdrkit_ils_marker_event_kind`.
  int32_t kind;
  // The marker of the tone. A value of `sdrkit_ils_marker_event_marker`.
  int32_t marker;
  // The tone, measured. A frequency in Hz.
  double tone_hz;
  // The modulation depth of the tone. Unit: %.
  double depth_pct;
  // The length of the passage. Unit: s.
  double duration_s;
} sdrkit_ils_marker_event;

// The options of `family` of `sdrkit_dgps_rtcm_frame_event`.
typedef enum sdrkit_dgps_rtcm_frame_event_family {
  // Aircraft
  SDRKIT_DGPS_RTCM_FRAME_EVENT_FAMILY_AIRCRAFT = 0,
  // Vessel
  SDRKIT_DGPS_RTCM_FRAME_EVENT_FAMILY_VESSEL = 1,
  // APRS
  SDRKIT_DGPS_RTCM_FRAME_EVENT_FAMILY_APRS = 2,
  // Pager
  SDRKIT_DGPS_RTCM_FRAME_EVENT_FAMILY_PAGER = 3,
  // Radiosonde
  SDRKIT_DGPS_RTCM_FRAME_EVENT_FAMILY_SONDE = 4,
  // Broadcast
  SDRKIT_DGPS_RTCM_FRAME_EVENT_FAMILY_BROADCAST = 5,
  // Voice
  SDRKIT_DGPS_RTCM_FRAME_EVENT_FAMILY_VOICE = 6,
  // Message
  SDRKIT_DGPS_RTCM_FRAME_EVENT_FAMILY_MESSAGE = 7,
  // Sensor
  SDRKIT_DGPS_RTCM_FRAME_EVENT_FAMILY_SENSOR = 8,
  // Satellite
  SDRKIT_DGPS_RTCM_FRAME_EVENT_FAMILY_SATELLITE = 9,
  // Receiver
  SDRKIT_DGPS_RTCM_FRAME_EVENT_FAMILY_RECEIVER = 10,
  // Alert
  SDRKIT_DGPS_RTCM_FRAME_EVENT_FAMILY_ALERT = 11,
  // Other
  SDRKIT_DGPS_RTCM_FRAME_EVENT_FAMILY_OTHER = 12,
} sdrkit_dgps_rtcm_frame_event_family;

// The options of `health` of `sdrkit_dgps_rtcm_frame_event`.
typedef enum sdrkit_dgps_rtcm_frame_event_health {
  // UDRE scale 1
  SDRKIT_DGPS_RTCM_FRAME_EVENT_HEALTH_SCALE_1 = 0,
  // UDRE scale 0.75
  SDRKIT_DGPS_RTCM_FRAME_EVENT_HEALTH_SCALE_0_75 = 1,
  // UDRE scale 0.5
  SDRKIT_DGPS_RTCM_FRAME_EVENT_HEALTH_SCALE_0_5 = 2,
  // UDRE scale 0.3
  SDRKIT_DGPS_RTCM_FRAME_EVENT_HEALTH_SCALE_0_3 = 3,
  // UDRE scale 0.2
  SDRKIT_DGPS_RTCM_FRAME_EVENT_HEALTH_SCALE_0_2 = 4,
  // UDRE scale 0.1
  SDRKIT_DGPS_RTCM_FRAME_EVENT_HEALTH_SCALE_0_1 = 5,
  // not monitored
  SDRKIT_DGPS_RTCM_FRAME_EVENT_HEALTH_NOT_MONITORED = 6,
  // not working
  SDRKIT_DGPS_RTCM_FRAME_EVENT_HEALTH_NOT_WORKING = 7,
} sdrkit_dgps_rtcm_frame_event_health;

// The fields of an RTCM frame.
typedef struct sdrkit_dgps_rtcm_frame_event {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Seconds since the object started, in the time of the signal.
  double time;
  // The frequency of the signal in Hz.
  double freq_hz;
  // The station id.
  sdrkit_string source;
  // A short text of the message.
  sdrkit_string text;
  // The family of the source: other. A value of `sdrkit_dgps_rtcm_frame_event_family`.
  int32_t family;
  // The message type.
  int64_t msg_type;
  // The station id.
  int64_t station_id;
  // The modified Z-count, seconds into the hour. Unit: s.
  double zcount_s;
  // The sequence number.
  int64_t sequence;
  // The health of the station. A value of `sdrkit_dgps_rtcm_frame_event_health`.
  int32_t health;
} sdrkit_dgps_rtcm_frame_event;

// Fills `rows` with the changed rows of the table "idents" of the stack "ndb" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The column `ident` names a row.
static inline int32_t sdrkit_ndb_idents_rows(SDRKIT_HANDLE handle,
                                            sdrkit_avnav_ident_row *rows,
                                            uint32_t capacity,
                                            sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "avnav.IdentRow", "idents", rows, sizeof(sdrkit_avnav_ident_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "scan" of the stack "ndb" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// Default order: the column `cn0_dbhz`, highest first.
static inline int32_t sdrkit_ndb_scan_rows(SDRKIT_HANDLE handle,
                                          sdrkit_ndb_scan_row *rows,
                                          uint32_t capacity,
                                          sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "avnav.NdbScanRow", "scan", rows, sizeof(sdrkit_ndb_scan_row), capacity, info);
}

// Fills `event` with event `n` of the last sdrkit_events_take() on `handle`, an event of the type "morse.word"
// (Identification word). Returns SDRKIT_E_WRONG_TYPE if the event is of another type.
static inline int32_t sdrkit_avnav_morse_word_event_read(SDRKIT_HANDLE handle, uint32_t n, sdrkit_avnav_morse_word_event *event) {
  return sdrkit_typed_event(handle, n, "morse.word", "avnav.IdentWordEvent", event, sizeof(sdrkit_avnav_morse_word_event));
}

// Runs the command "clear" (Clear) of the stack "ndb" on the session of `handle`. Forgets everything decoded so far.
static inline int32_t sdrkit_ndb_clear(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "clear", NULL, NULL, NULL, NULL, 0);
}

// The settings of the test signal generator of the stack "ndb" are the sdrkit_ndb_gen_*() functions
// after the settings of its receiver.

// The setting "ident_keying" (Identification) of the stack "ndb". A value of `sdrkit_ndb_ident_keying`. A keyed tone, a keyed carrier, or the one that keys.
static inline int32_t sdrkit_ndb_get_ident_keying(SDRKIT_HANDLE handle, sdrkit_ndb_ident_keying *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "ident_keying", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_ndb_ident_keying)index;
  return code;
}
static inline int32_t sdrkit_ndb_set_ident_keying(SDRKIT_HANDLE handle, sdrkit_ndb_ident_keying value) {
  return sdrkit_typed_setting_set_enum(handle, "ident_keying", (int32_t)value);
}

// The setting "tone" (Identification tone) of the stack "ndb". A frequency in Hz. 0 finds it (400 or 1020 Hz).
static inline int32_t sdrkit_ndb_get_tone(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "tone", value);
}
static inline int32_t sdrkit_ndb_set_tone(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "tone", value);
}

// The setting "audio" (Audio) of the stack "ndb". A value of `sdrkit_ndb_audio`.
static inline int32_t sdrkit_ndb_get_audio(SDRKIT_HANDLE handle, sdrkit_ndb_audio *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "audio", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_ndb_audio)index;
  return code;
}
static inline int32_t sdrkit_ndb_set_audio(SDRKIT_HANDLE handle, sdrkit_ndb_audio value) {
  return sdrkit_typed_setting_set_enum(handle, "audio", (int32_t)value);
}

// The setting "pitch" (Beat pitch) of the stack "ndb". A frequency in Hz.
static inline int32_t sdrkit_ndb_get_pitch(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "pitch", value);
}
static inline int32_t sdrkit_ndb_set_pitch(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "pitch", value);
}

// The setting "confirm" (Hearings to confirm) of the stack "ndb". How many times an identification must be heard before it is shown.
static inline int32_t sdrkit_ndb_get_confirm(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "confirm", value);
}
static inline int32_t sdrkit_ndb_set_confirm(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "confirm", value);
}

// The setting "carrier_threshold" (Carrier threshold) of the stack "ndb". Unit: dB-Hz.
static inline int32_t sdrkit_ndb_get_carrier_threshold(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "carrier_threshold", value);
}
static inline int32_t sdrkit_ndb_set_carrier_threshold(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "carrier_threshold", value);
}

// The setting "sync_tones" (Synchronous tone detection) of the stack "ndb". The tone is read from the in-phase part against the followed carrier, not from the envelope.
static inline int32_t sdrkit_ndb_get_sync_tones(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "sync_tones", value);
}
static inline int32_t sdrkit_ndb_set_sync_tones(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "sync_tones", value);
}

// The setting "ident_average" (Average repeated idents) of the stack "ndb". The identification is also read from the average of its repeats over up to three minutes.
static inline int32_t sdrkit_ndb_get_ident_average(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "ident_average", value);
}
static inline int32_t sdrkit_ndb_set_ident_average(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "ident_average", value);
}

// The setting "scan_threshold" (Scan threshold) of the stack "ndb". Unit: dB-Hz. How far above the noise a carrier must stand to be listed.
static inline int32_t sdrkit_ndb_get_scan_threshold(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "scan_threshold", value);
}
static inline int32_t sdrkit_ndb_set_scan_threshold(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "scan_threshold", value);
}

// The setting "scan_interval" (Scan every) of the stack "ndb". A length of time in seconds.
static inline int32_t sdrkit_ndb_get_scan_interval(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "scan_interval", value);
}
static inline int32_t sdrkit_ndb_set_scan_interval(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "scan_interval", value);
}

// The setting "gen_ident" (Test signal: identification) of the test signal generator of the stack "ndb".
static inline int32_t sdrkit_ndb_gen_get_ident(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "gen_ident", value);
}
static inline int32_t sdrkit_ndb_gen_set_ident(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "gen_ident", value);
}

// The setting "gen_keying" (Test signal: keying) of the test signal generator of the stack "ndb". A value of `sdrkit_ndb_gen_keying`.
static inline int32_t sdrkit_ndb_gen_get_keying(SDRKIT_HANDLE handle, sdrkit_ndb_gen_keying *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "gen_keying", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_ndb_gen_keying)index;
  return code;
}
static inline int32_t sdrkit_ndb_gen_set_keying(SDRKIT_HANDLE handle, sdrkit_ndb_gen_keying value) {
  return sdrkit_typed_setting_set_enum(handle, "gen_keying", (int32_t)value);
}

// The setting "gen_tone" (Test signal: tone) of the test signal generator of the stack "ndb". A frequency in Hz.
static inline int32_t sdrkit_ndb_gen_get_tone(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_tone", value);
}
static inline int32_t sdrkit_ndb_gen_set_tone(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_tone", value);
}

// The setting "gen_depth" (Test signal: modulation depth) of the test signal generator of the stack "ndb".
static inline int32_t sdrkit_ndb_gen_get_depth(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_depth", value);
}
static inline int32_t sdrkit_ndb_gen_set_depth(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_depth", value);
}

// The setting "gen_wpm" (Test signal: Morse speed) of the test signal generator of the stack "ndb". Unit: wpm.
static inline int32_t sdrkit_ndb_gen_get_wpm(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_wpm", value);
}
static inline int32_t sdrkit_ndb_gen_set_wpm(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_wpm", value);
}

// The setting "gen_pause" (Test signal: pause) of the test signal generator of the stack "ndb". A length of time in seconds.
static inline int32_t sdrkit_ndb_gen_get_pause(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_pause", value);
}
static inline int32_t sdrkit_ndb_gen_set_pause(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_pause", value);
}

// The setting "gen_others" (Test signal: other beacons) of the test signal generator of the stack "ndb". Beacons beside the tuned one, for the band scan.
static inline int32_t sdrkit_ndb_gen_get_others(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "gen_others", value);
}
static inline int32_t sdrkit_ndb_gen_set_others(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "gen_others", value);
}

// Fills `rows` with the changed rows of the table "idents" of the stack "vor" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The column `ident` names a row.
static inline int32_t sdrkit_vor_idents_rows(SDRKIT_HANDLE handle,
                                            sdrkit_avnav_ident_row *rows,
                                            uint32_t capacity,
                                            sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "avnav.IdentRow", "idents", rows, sizeof(sdrkit_avnav_ident_row), capacity, info);
}

// Runs the command "clear" (Clear) of the stack "vor" on the session of `handle`. Forgets everything decoded so far.
static inline int32_t sdrkit_vor_clear(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "clear", NULL, NULL, NULL, NULL, 0);
}

// The settings of the test signal generator of the stack "vor" are the sdrkit_vor_gen_*() functions
// after the settings of its receiver.

// The setting "obs" (Course (OBS)) of the stack "vor". Unit: °. The selected course: the deviation and the TO/FROM flag are shown for it.
static inline int32_t sdrkit_vor_get_obs(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "obs", value);
}
static inline int32_t sdrkit_vor_set_obs(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "obs", value);
}

// The setting "average" (Averaging) of the stack "vor". A length of time in seconds. How long the bearing is averaged over.
static inline int32_t sdrkit_vor_get_average(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "average", value);
}
static inline int32_t sdrkit_vor_set_average(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "average", value);
}

// The setting "confirm" (Hearings to confirm) of the stack "vor". How many times an identification must be heard before it is shown.
static inline int32_t sdrkit_vor_get_confirm(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "confirm", value);
}
static inline int32_t sdrkit_vor_set_confirm(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "confirm", value);
}

// The setting "carrier_threshold" (Carrier threshold) of the stack "vor". Unit: dB-Hz.
static inline int32_t sdrkit_vor_get_carrier_threshold(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "carrier_threshold", value);
}
static inline int32_t sdrkit_vor_set_carrier_threshold(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "carrier_threshold", value);
}

// The setting "gen_kind" (Test signal: station) of the test signal generator of the stack "vor". A value of `sdrkit_vor_gen_kind`.
static inline int32_t sdrkit_vor_gen_get_kind(SDRKIT_HANDLE handle, sdrkit_vor_gen_kind *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "gen_kind", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_vor_gen_kind)index;
  return code;
}
static inline int32_t sdrkit_vor_gen_set_kind(SDRKIT_HANDLE handle, sdrkit_vor_gen_kind value) {
  return sdrkit_typed_setting_set_enum(handle, "gen_kind", (int32_t)value);
}

// The setting "gen_radial" (Test signal: radial) of the test signal generator of the stack "vor". Unit: °.
static inline int32_t sdrkit_vor_gen_get_radial(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_radial", value);
}
static inline int32_t sdrkit_vor_gen_set_radial(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_radial", value);
}

// The setting "gen_ident" (Test signal: identification) of the test signal generator of the stack "vor".
static inline int32_t sdrkit_vor_gen_get_ident(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "gen_ident", value);
}
static inline int32_t sdrkit_vor_gen_set_ident(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "gen_ident", value);
}

// The setting "gen_wpm" (Test signal: Morse speed) of the test signal generator of the stack "vor". Unit: wpm.
static inline int32_t sdrkit_vor_gen_get_wpm(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_wpm", value);
}
static inline int32_t sdrkit_vor_gen_set_wpm(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_wpm", value);
}

// The setting "gen_pause" (Test signal: pause between idents) of the test signal generator of the stack "vor". A length of time in seconds.
static inline int32_t sdrkit_vor_gen_get_pause(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_pause", value);
}
static inline int32_t sdrkit_vor_gen_set_pause(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_pause", value);
}

// The setting "gen_rotate" (Test signal: radial change) of the test signal generator of the stack "vor". Unit: °/s. The radial moves this fast (an aircraft flying round the station).
static inline int32_t sdrkit_vor_gen_get_rotate(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_rotate", value);
}
static inline int32_t sdrkit_vor_gen_set_rotate(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_rotate", value);
}

// Fills `rows` with the changed rows of the table "idents" of the stack "ils" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The column `ident` names a row.
static inline int32_t sdrkit_ils_idents_rows(SDRKIT_HANDLE handle,
                                            sdrkit_avnav_ident_row *rows,
                                            uint32_t capacity,
                                            sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "avnav.IdentRow", "idents", rows, sizeof(sdrkit_avnav_ident_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "passages" of the stack "ils" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The column `marker` names a row.
static inline int32_t sdrkit_ils_passages_rows(SDRKIT_HANDLE handle,
                                              sdrkit_ils_marker_passage_row *rows,
                                              uint32_t capacity,
                                              sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "avnav.MarkerPassageRow", "passages", rows, sizeof(sdrkit_ils_marker_passage_row), capacity, info);
}

// Fills `event` with event `n` of the last sdrkit_events_take() on `handle`, an event of the type "ils.marker"
// (Marker passage). Returns SDRKIT_E_WRONG_TYPE if the event is of another type.
static inline int32_t sdrkit_ils_marker_event_read(SDRKIT_HANDLE handle, uint32_t n, sdrkit_ils_marker_event *event) {
  return sdrkit_typed_event(handle, n, "ils.marker", "avnav.MarkerEvent", event, sizeof(sdrkit_ils_marker_event));
}

// Runs the command "clear" (Clear) of the stack "ils" on the session of `handle`. Forgets everything decoded so far.
static inline int32_t sdrkit_ils_clear(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "clear", NULL, NULL, NULL, NULL, 0);
}

// The settings of the test signal generator of the stack "ils" are the sdrkit_ils_gen_*() functions
// after the settings of its receiver.

// The setting "part" (Part) of the stack "ils". A value of `sdrkit_ils_part`. Localizer, glide path or marker beacon. By frequency, 74.5 to 75.5 MHz is a marker beacon and above 300 MHz a glide path.
static inline int32_t sdrkit_ils_get_part(SDRKIT_HANDLE handle, sdrkit_ils_part *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "part", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_ils_part)index;
  return code;
}
static inline int32_t sdrkit_ils_set_part(SDRKIT_HANDLE handle, sdrkit_ils_part value) {
  return sdrkit_typed_setting_set_enum(handle, "part", (int32_t)value);
}

// The setting "average" (Averaging) of the stack "ils". A length of time in seconds.
static inline int32_t sdrkit_ils_get_average(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "average", value);
}
static inline int32_t sdrkit_ils_set_average(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "average", value);
}

// The setting "confirm" (Hearings to confirm) of the stack "ils". How many times an identification must be heard before it is shown.
static inline int32_t sdrkit_ils_get_confirm(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "confirm", value);
}
static inline int32_t sdrkit_ils_set_confirm(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "confirm", value);
}

// The setting "carrier_threshold" (Carrier threshold) of the stack "ils". Unit: dB-Hz.
static inline int32_t sdrkit_ils_get_carrier_threshold(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "carrier_threshold", value);
}
static inline int32_t sdrkit_ils_set_carrier_threshold(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "carrier_threshold", value);
}

// The setting "gen_ddm" (Test signal: DDM) of the test signal generator of the stack "ils". Positive: 90 Hz predominates.
static inline int32_t sdrkit_ils_gen_get_ddm(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_ddm", value);
}
static inline int32_t sdrkit_ils_gen_set_ddm(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_ddm", value);
}

// The setting "gen_ident" (Test signal: identification) of the test signal generator of the stack "ils".
static inline int32_t sdrkit_ils_gen_get_ident(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "gen_ident", value);
}
static inline int32_t sdrkit_ils_gen_set_ident(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "gen_ident", value);
}

// The setting "gen_wpm" (Test signal: Morse speed) of the test signal generator of the stack "ils". Unit: wpm.
static inline int32_t sdrkit_ils_gen_get_wpm(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_wpm", value);
}
static inline int32_t sdrkit_ils_gen_set_wpm(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_wpm", value);
}

// The setting "gen_pause" (Test signal: pause between idents) of the test signal generator of the stack "ils". A length of time in seconds.
static inline int32_t sdrkit_ils_gen_get_pause(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_pause", value);
}
static inline int32_t sdrkit_ils_gen_set_pause(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_pause", value);
}

// The setting "gen_marker" (Test signal: marker) of the test signal generator of the stack "ils". A value of `sdrkit_ils_gen_marker`. The marker beacon of the part `marker`.
static inline int32_t sdrkit_ils_gen_get_marker(SDRKIT_HANDLE handle, sdrkit_ils_gen_marker *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "gen_marker", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_ils_gen_marker)index;
  return code;
}
static inline int32_t sdrkit_ils_gen_set_marker(SDRKIT_HANDLE handle, sdrkit_ils_gen_marker value) {
  return sdrkit_typed_setting_set_enum(handle, "gen_marker", (int32_t)value);
}

// The setting "gen_marker_keying" (Test signal: marker keying) of the test signal generator of the stack "ils". A value of `sdrkit_ils_gen_marker_keying`.
static inline int32_t sdrkit_ils_gen_get_marker_keying(SDRKIT_HANDLE handle, sdrkit_ils_gen_marker_keying *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "gen_marker_keying", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_ils_gen_marker_keying)index;
  return code;
}
static inline int32_t sdrkit_ils_gen_set_marker_keying(SDRKIT_HANDLE handle, sdrkit_ils_gen_marker_keying value) {
  return sdrkit_typed_setting_set_enum(handle, "gen_marker_keying", (int32_t)value);
}

// The setting "gen_marker_tone" (Test signal: marker tone) of the test signal generator of the stack "ils". A frequency in Hz. 0 sends the tone of the marker.
static inline int32_t sdrkit_ils_gen_get_marker_tone(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_marker_tone", value);
}
static inline int32_t sdrkit_ils_gen_set_marker_tone(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_marker_tone", value);
}

// The setting "gen_marker_depth" (Test signal: marker depth) of the test signal generator of the stack "ils". Unit: %.
static inline int32_t sdrkit_ils_gen_get_marker_depth(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_marker_depth", value);
}
static inline int32_t sdrkit_ils_gen_set_marker_depth(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_marker_depth", value);
}

// The setting "gen_marker_start" (Test signal: marker keyed from) of the test signal generator of the stack "ils". A length of time in seconds.
static inline int32_t sdrkit_ils_gen_get_marker_start(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_marker_start", value);
}
static inline int32_t sdrkit_ils_gen_set_marker_start(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_marker_start", value);
}

// The setting "gen_marker_stop" (Test signal: marker keyed until) of the test signal generator of the stack "ils". A length of time in seconds. 0 keys without end.
static inline int32_t sdrkit_ils_gen_get_marker_stop(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_marker_stop", value);
}
static inline int32_t sdrkit_ils_gen_set_marker_stop(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_marker_stop", value);
}

// Fills `rows` with the changed rows of the table "idents" of the stack "dme" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The column `ident` names a row.
static inline int32_t sdrkit_dme_idents_rows(SDRKIT_HANDLE handle,
                                            sdrkit_avnav_ident_row *rows,
                                            uint32_t capacity,
                                            sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "avnav.IdentRow", "idents", rows, sizeof(sdrkit_avnav_ident_row), capacity, info);
}

// Runs the command "clear" (Clear) of the stack "dme" on the session of `handle`. Forgets everything decoded so far.
static inline int32_t sdrkit_dme_clear(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "clear", NULL, NULL, NULL, NULL, 0);
}

// The settings of the test signal generator of the stack "dme" are the sdrkit_dme_gen_*() functions
// after the settings of its receiver.

// The setting "mode" (Channel mode) of the stack "dme". A value of `sdrkit_dme_mode`.
static inline int32_t sdrkit_dme_get_mode(SDRKIT_HANDLE handle, sdrkit_dme_mode *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "mode", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_dme_mode)index;
  return code;
}
static inline int32_t sdrkit_dme_set_mode(SDRKIT_HANDLE handle, sdrkit_dme_mode value) {
  return sdrkit_typed_setting_set_enum(handle, "mode", (int32_t)value);
}

// The setting "threshold" (Pulse threshold) of the stack "dme". Unit: dB.
static inline int32_t sdrkit_dme_get_threshold(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "threshold", value);
}
static inline int32_t sdrkit_dme_set_threshold(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "threshold", value);
}

// The setting "confirm" (Hearings to confirm) of the stack "dme". How many times an identification must be heard before it is shown.
static inline int32_t sdrkit_dme_get_confirm(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "confirm", value);
}
static inline int32_t sdrkit_dme_set_confirm(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "confirm", value);
}

// The setting "gen_mode" (Test signal: mode) of the test signal generator of the stack "dme". A value of `sdrkit_dme_gen_mode`.
static inline int32_t sdrkit_dme_gen_get_mode(SDRKIT_HANDLE handle, sdrkit_dme_gen_mode *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "gen_mode", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_dme_gen_mode)index;
  return code;
}
static inline int32_t sdrkit_dme_gen_set_mode(SDRKIT_HANDLE handle, sdrkit_dme_gen_mode value) {
  return sdrkit_typed_setting_set_enum(handle, "gen_mode", (int32_t)value);
}

// The setting "gen_kind" (Test signal: transmitter) of the test signal generator of the stack "dme". A value of `sdrkit_dme_gen_kind`.
static inline int32_t sdrkit_dme_gen_get_kind(SDRKIT_HANDLE handle, sdrkit_dme_gen_kind *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "gen_kind", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_dme_gen_kind)index;
  return code;
}
static inline int32_t sdrkit_dme_gen_set_kind(SDRKIT_HANDLE handle, sdrkit_dme_gen_kind value) {
  return sdrkit_typed_setting_set_enum(handle, "gen_kind", (int32_t)value);
}

// The setting "gen_ppps" (Test signal: pairs a second) of the test signal generator of the stack "dme".
static inline int32_t sdrkit_dme_gen_get_ppps(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_ppps", value);
}
static inline int32_t sdrkit_dme_gen_set_ppps(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_ppps", value);
}

// The setting "gen_ident" (Test signal: identification) of the test signal generator of the stack "dme".
static inline int32_t sdrkit_dme_gen_get_ident(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "gen_ident", value);
}
static inline int32_t sdrkit_dme_gen_set_ident(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "gen_ident", value);
}

// The setting "gen_wpm" (Test signal: Morse speed) of the test signal generator of the stack "dme". Unit: wpm.
static inline int32_t sdrkit_dme_gen_get_wpm(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_wpm", value);
}
static inline int32_t sdrkit_dme_gen_set_wpm(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_wpm", value);
}

// The setting "gen_pause" (Test signal: pause) of the test signal generator of the stack "dme". A length of time in seconds.
static inline int32_t sdrkit_dme_gen_get_pause(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_pause", value);
}
static inline int32_t sdrkit_dme_gen_set_pause(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_pause", value);
}

// Fills `rows` with the changed rows of the table "stations" of the stack "loran" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The column `station` names a row.
static inline int32_t sdrkit_loran_stations_rows(SDRKIT_HANDLE handle,
                                                sdrkit_loran_station_row *rows,
                                                uint32_t capacity,
                                                sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "avnav.LoranStationRow", "stations", rows, sizeof(sdrkit_loran_station_row), capacity, info);
}

// Runs the command "clear" (Clear) of the stack "loran" on the session of `handle`. Forgets everything decoded so far.
static inline int32_t sdrkit_loran_clear(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "clear", NULL, NULL, NULL, NULL, 0);
}

// The settings of the test signal generator of the stack "loran" are the sdrkit_loran_gen_*() functions
// after the settings of its receiver.

// The setting "gri" (GRI) of the stack "loran". The chain's group repetition interval (9960 for 99.6 ms). 0 searches the chains of the setting chains.
static inline int32_t sdrkit_loran_get_gri(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "gri", value);
}
static inline int32_t sdrkit_loran_set_gri(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "gri", value);
}

// The setting "average" (Averaging) of the stack "loran". A length of time in seconds.
static inline int32_t sdrkit_loran_get_average(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "average", value);
}
static inline int32_t sdrkit_loran_set_average(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "average", value);
}

// The setting "threshold" (Secondary threshold) of the stack "loran". Unit: dB.
static inline int32_t sdrkit_loran_get_threshold(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "threshold", value);
}
static inline int32_t sdrkit_loran_set_threshold(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "threshold", value);
}

// The setting "max_secondaries" (Secondaries) of the stack "loran".
static inline int32_t sdrkit_loran_get_max_secondaries(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "max_secondaries", value);
}
static inline int32_t sdrkit_loran_set_max_secondaries(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "max_secondaries", value);
}

// The setting "chains" (Chains) of the stack "loran". One a line (or separated by ;): the GRI in tens of microseconds, a comma, the chain's name. With GRI 0 these GRIs are searched; the chain found takes its name. Fewer rows cost less to search.
static inline int32_t sdrkit_loran_get_chains(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "chains", value);
}
static inline int32_t sdrkit_loran_set_chains(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "chains", value);
}

// The setting "gen_gri" (Test signal: GRI) of the test signal generator of the stack "loran".
static inline int32_t sdrkit_loran_gen_get_gri(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "gen_gri", value);
}
static inline int32_t sdrkit_loran_gen_set_gri(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "gen_gri", value);
}

// The setting "gen_tds" (Test signal: TDs (µs)) of the test signal generator of the stack "loran". The time differences of the secondaries in µs, separated by commas.
static inline int32_t sdrkit_loran_gen_get_tds(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "gen_tds", value);
}
static inline int32_t sdrkit_loran_gen_set_tds(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "gen_tds", value);
}

// The setting "gen_ecd" (Test signal: ECD of the first secondary) of the test signal generator of the stack "loran". Unit: µs.
static inline int32_t sdrkit_loran_gen_get_ecd(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_ecd", value);
}
static inline int32_t sdrkit_loran_gen_set_ecd(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_ecd", value);
}

// Fills `rows` with the changed rows of the table "station" of the stack "dgps" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The column `station_id` names a row.
// Place of a row: the column `position`.
// Height: `height_m`.
static inline int32_t sdrkit_dgps_station_rows(SDRKIT_HANDLE handle,
                                              sdrkit_dgps_station_row *rows,
                                              uint32_t capacity,
                                              sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "avnav.DgpsStationRow", "station", rows, sizeof(sdrkit_dgps_station_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "messages" of the stack "dgps" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The column `msg_type` names a row.
static inline int32_t sdrkit_dgps_messages_rows(SDRKIT_HANDLE handle,
                                               sdrkit_dgps_message_row *rows,
                                               uint32_t capacity,
                                               sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "avnav.DgpsMessageRow", "messages", rows, sizeof(sdrkit_dgps_message_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "corrections" of the stack "dgps" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The column `prn` names a row.
// Default order: the column `prn`, lowest first.
static inline int32_t sdrkit_dgps_corrections_rows(SDRKIT_HANDLE handle,
                                                  sdrkit_dgps_correction_row *rows,
                                                  uint32_t capacity,
                                                  sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "avnav.DgpsCorrectionRow", "corrections", rows, sizeof(sdrkit_dgps_correction_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "almanac" of the stack "dgps" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The column `station_id` names a row.
// Place of a row: the column `position`.
static inline int32_t sdrkit_dgps_almanac_rows(SDRKIT_HANDLE handle,
                                              sdrkit_dgps_almanac_row *rows,
                                              uint32_t capacity,
                                              sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "avnav.DgpsAlmanacRow", "almanac", rows, sizeof(sdrkit_dgps_almanac_row), capacity, info);
}

// Fills `event` with event `n` of the last sdrkit_events_take() on `handle`, an event of the type "rtcm.frame"
// (RTCM frame). Returns SDRKIT_E_WRONG_TYPE if the event is of another type.
static inline int32_t sdrkit_dgps_rtcm_frame_event_read(SDRKIT_HANDLE handle, uint32_t n, sdrkit_dgps_rtcm_frame_event *event) {
  return sdrkit_typed_event(handle, n, "rtcm.frame", "avnav.RtcmFrameEvent", event, sizeof(sdrkit_dgps_rtcm_frame_event));
}

// Runs the command "clear" (Clear) of the stack "dgps" on the session of `handle`. Forgets everything decoded so far.
static inline int32_t sdrkit_dgps_clear(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "clear", NULL, NULL, NULL, NULL, 0);
}

// The settings of the test signal generator of the stack "dgps" are the sdrkit_dgps_gen_*() functions
// after the settings of its receiver.

// The setting "bitrate" (Bit rate) of the stack "dgps". A value of `sdrkit_dgps_bitrate`.
static inline int32_t sdrkit_dgps_get_bitrate(SDRKIT_HANDLE handle, sdrkit_dgps_bitrate *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "bitrate", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_dgps_bitrate)index;
  return code;
}
static inline int32_t sdrkit_dgps_set_bitrate(SDRKIT_HANDLE handle, sdrkit_dgps_bitrate value) {
  return sdrkit_typed_setting_set_enum(handle, "bitrate", (int32_t)value);
}

// The setting "gen_bitrate" (Test signal: bit rate) of the test signal generator of the stack "dgps". A value of `sdrkit_dgps_gen_bitrate`.
static inline int32_t sdrkit_dgps_gen_get_bitrate(SDRKIT_HANDLE handle, sdrkit_dgps_gen_bitrate *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "gen_bitrate", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_dgps_gen_bitrate)index;
  return code;
}
static inline int32_t sdrkit_dgps_gen_set_bitrate(SDRKIT_HANDLE handle, sdrkit_dgps_gen_bitrate value) {
  return sdrkit_typed_setting_set_enum(handle, "gen_bitrate", (int32_t)value);
}

// The setting "gen_station" (Test signal: station id) of the test signal generator of the stack "dgps".
static inline int32_t sdrkit_dgps_gen_get_station(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "gen_station", value);
}
static inline int32_t sdrkit_dgps_gen_set_station(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "gen_station", value);
}

// The setting "gen_lat" (Test signal: latitude) of the test signal generator of the stack "dgps". Unit: °.
static inline int32_t sdrkit_dgps_gen_get_lat(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_lat", value);
}
static inline int32_t sdrkit_dgps_gen_set_lat(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_lat", value);
}

// The setting "gen_lon" (Test signal: longitude) of the test signal generator of the stack "dgps". Unit: °.
static inline int32_t sdrkit_dgps_gen_get_lon(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_lon", value);
}
static inline int32_t sdrkit_dgps_gen_set_lon(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_lon", value);
}

// The setting "gen_text" (Test signal: text) of the test signal generator of the stack "dgps".
static inline int32_t sdrkit_dgps_gen_get_text(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "gen_text", value);
}
static inline int32_t sdrkit_dgps_gen_set_text(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "gen_text", value);
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_avnav_tuned_fields_read(SDRKIT_HANDLE handle, sdrkit_avnav_tuned_fields *fields) {
  return sdrkit_typed_fields(handle, "avnav.TunedFields", fields, sizeof(sdrkit_avnav_tuned_fields));
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_avnav_ident_fields_read(SDRKIT_HANDLE handle, sdrkit_avnav_ident_fields *fields) {
  return sdrkit_typed_fields(handle, "avnav.IdentFields", fields, sizeof(sdrkit_avnav_ident_fields));
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_avnav_carrier_fields_read(SDRKIT_HANDLE handle, sdrkit_avnav_carrier_fields *fields) {
  return sdrkit_typed_fields(handle, "avnav.CarrierFields", fields, sizeof(sdrkit_avnav_carrier_fields));
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_ndb_fields_read(SDRKIT_HANDLE handle, sdrkit_ndb_fields *fields) {
  return sdrkit_typed_fields(handle, "avnav.NdbFields", fields, sizeof(sdrkit_ndb_fields));
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_avnav_keyer_fields_read(SDRKIT_HANDLE handle, sdrkit_avnav_keyer_fields *fields) {
  return sdrkit_typed_fields(handle, "avnav.KeyerFields", fields, sizeof(sdrkit_avnav_keyer_fields));
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_vor_fields_read(SDRKIT_HANDLE handle, sdrkit_vor_fields *fields) {
  return sdrkit_typed_fields(handle, "avnav.VorFields", fields, sizeof(sdrkit_vor_fields));
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_ils_fields_read(SDRKIT_HANDLE handle, sdrkit_ils_fields *fields) {
  return sdrkit_typed_fields(handle, "avnav.IlsFields", fields, sizeof(sdrkit_ils_fields));
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_ils_marker_fields_read(SDRKIT_HANDLE handle, sdrkit_ils_marker_fields *fields) {
  return sdrkit_typed_fields(handle, "avnav.MarkerFields", fields, sizeof(sdrkit_ils_marker_fields));
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_dme_fields_read(SDRKIT_HANDLE handle, sdrkit_dme_fields *fields) {
  return sdrkit_typed_fields(handle, "avnav.DmeFields", fields, sizeof(sdrkit_dme_fields));
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_loran_fields_read(SDRKIT_HANDLE handle, sdrkit_loran_fields *fields) {
  return sdrkit_typed_fields(handle, "avnav.LoranFields", fields, sizeof(sdrkit_loran_fields));
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_dgps_fields_read(SDRKIT_HANDLE handle, sdrkit_dgps_fields *fields) {
  return sdrkit_typed_fields(handle, "avnav.DgpsFields", fields, sizeof(sdrkit_dgps_fields));
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_dgps_meter_fields_read(SDRKIT_HANDLE handle, sdrkit_dgps_meter_fields *fields) {
  return sdrkit_typed_fields(handle, "avnav.DgpsMeterFields", fields, sizeof(sdrkit_dgps_meter_fields));
}

// Fills `fields` with the result fields of the component "keyer" of the stack "ndb" in the snapshot of
// `handle`. A generic consumer reads them under the keys "keyer.<key>".
static inline int32_t sdrkit_ndb_keyer_fields_read(SDRKIT_HANDLE handle, sdrkit_avnav_keyer_fields *fields) {
  return sdrkit_typed_fields(handle, "avnav.KeyerFields@keyer", fields, sizeof(sdrkit_avnav_keyer_fields));
}

// Fills `fields` with the result fields of the component "keyer" of the stack "vor" in the snapshot of
// `handle`. A generic consumer reads them under the keys "keyer.<key>".
static inline int32_t sdrkit_vor_keyer_fields_read(SDRKIT_HANDLE handle, sdrkit_avnav_keyer_fields *fields) {
  return sdrkit_typed_fields(handle, "avnav.KeyerFields@keyer", fields, sizeof(sdrkit_avnav_keyer_fields));
}

// Fills `fields` with the result fields of the component "keyer" of the stack "ils" in the snapshot of
// `handle`. A generic consumer reads them under the keys "keyer.<key>".
static inline int32_t sdrkit_ils_keyer_fields_read(SDRKIT_HANDLE handle, sdrkit_avnav_keyer_fields *fields) {
  return sdrkit_typed_fields(handle, "avnav.KeyerFields@keyer", fields, sizeof(sdrkit_avnav_keyer_fields));
}

// Fills `fields` with the result fields of the component "level" of the stack "dgps" in the snapshot of
// `handle`. A generic consumer reads them under the keys "level.<key>".
static inline int32_t sdrkit_dgps_level_dgps_meter_fields_read(SDRKIT_HANDLE handle, sdrkit_dgps_meter_fields *fields) {
  return sdrkit_typed_fields(handle, "avnav.DgpsMeterFields@level", fields, sizeof(sdrkit_dgps_meter_fields));
}

// Fills `fields` with the result fields of the component "msk50" of the stack "dgps" in the snapshot of
// `handle`. A generic consumer reads them under the keys "msk50.<key>".
static inline int32_t sdrkit_dgps_msk50_dgps_msk_fields_read(SDRKIT_HANDLE handle, sdrkit_dgps_msk_fields *fields) {
  return sdrkit_typed_fields(handle, "avnav.DgpsMskFields@msk50", fields, sizeof(sdrkit_dgps_msk_fields));
}

// Fills `fields` with the result fields of the component "msk100" of the stack "dgps" in the snapshot of
// `handle`. A generic consumer reads them under the keys "msk100.<key>".
static inline int32_t sdrkit_dgps_msk100_dgps_msk_fields_read(SDRKIT_HANDLE handle, sdrkit_dgps_msk_fields *fields) {
  return sdrkit_typed_fields(handle, "avnav.DgpsMskFields@msk100", fields, sizeof(sdrkit_dgps_msk_fields));
}

// Fills `fields` with the result fields of the component "msk200" of the stack "dgps" in the snapshot of
// `handle`. A generic consumer reads them under the keys "msk200.<key>".
static inline int32_t sdrkit_dgps_msk200_dgps_msk_fields_read(SDRKIT_HANDLE handle, sdrkit_dgps_msk_fields *fields) {
  return sdrkit_typed_fields(handle, "avnav.DgpsMskFields@msk200", fields, sizeof(sdrkit_dgps_msk_fields));
}

#endif
