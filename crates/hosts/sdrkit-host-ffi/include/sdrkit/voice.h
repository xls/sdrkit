/*
 * sdrkit: the typed interface of the stack family "voice", version 1.5. MIT licence.
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

#ifndef SDRKIT_VOICE_H
#define SDRKIT_VOICE_H

#include "../sdrkit.h"
#include "tones.h"

// The version of the family that this header describes.
#define SDRKIT_VOICE_VERSION_MAJOR 1
#define SDRKIT_VOICE_VERSION_MINOR 5

// The stack "hfpacket": its id, and the keys of its event types.
#define SDRKIT_STACK_HFPACKET "hfpacket"
#define SDRKIT_EVENT_HFPACKET_FRAME "hfpacket.frame"
#define SDRKIT_EVENT_HFPACKET_STATION "hfpacket.station"

// The stack "elt121": its id, and the keys of its event types.
#define SDRKIT_STACK_ELT121 "elt121"
#define SDRKIT_EVENT_ELT121_FOUND "elt121.found"
#define SDRKIT_EVENT_ELT121_ENDED "elt121.ended"

// The stack "voice_inversion": its id, and the keys of its event types.
#define SDRKIT_STACK_VOICE_INVERSION "voice_inversion"
#define SDRKIT_EVENT_VOICE_INVERSION_FOUND "voice_inversion.found"
#define SDRKIT_EVENT_VOICE_INVERSION_CLEAR "voice_inversion.clear"

// The stack "voice_channels": its id, and the keys of its event types.
#define SDRKIT_STACK_VOICE_CHANNELS "voice_channels"
#define SDRKIT_EVENT_VOICE_CHANNELS_CALL "voice_channels.call"

// The stack "mdc1200": its id, and the keys of its event types.
#define SDRKIT_STACK_MDC1200 "mdc1200"
#define SDRKIT_EVENT_MDC1200_CALL "mdc1200.call"
#define SDRKIT_EVENT_MDC1200_BURST "mdc1200.burst"

// The stack "wmic": its id, and the keys of its event types.
#define SDRKIT_STACK_WMIC "wmic"
#define SDRKIT_EVENT_WMIC_CANDIDATE "wmic.candidate"

// The stack "fleetsync": its id, and the keys of its event types.
#define SDRKIT_STACK_FLEETSYNC "fleetsync"
#define SDRKIT_EVENT_FLEETSYNC_CALL "fleetsync.call"
#define SDRKIT_EVENT_FLEETSYNC_BURST "fleetsync.burst"

// The options of the setting "sideband" of the stack "hfpacket".
typedef enum sdrkit_hfpacket_sideband {
  // Upper sideband
  SDRKIT_HFPACKET_SIDEBAND_USB = 0,
  // Lower sideband
  SDRKIT_HFPACKET_SIDEBAND_LSB = 1,
} sdrkit_hfpacket_sideband;

// The options of the setting "demod" of the stack "voice_channels".
typedef enum sdrkit_voice_channels_demod {
  // Automatic
  SDRKIT_VOICE_CHANNELS_DEMOD_AUTO = 0,
  // FM
  SDRKIT_VOICE_CHANNELS_DEMOD_FM = 1,
  // AM
  SDRKIT_VOICE_CHANNELS_DEMOD_AM = 2,
  // USB
  SDRKIT_VOICE_CHANNELS_DEMOD_USB = 3,
  // LSB
  SDRKIT_VOICE_CHANNELS_DEMOD_LSB = 4,
} sdrkit_voice_channels_demod;

// The options of the setting "gen_scene" of the stack "voice_channels".
typedef enum sdrkit_voice_channels_gen_scene {
  // Automatic
  SDRKIT_VOICE_CHANNELS_GEN_SCENE_AUTO = 0,
  // Mixed
  SDRKIT_VOICE_CHANNELS_GEN_SCENE_MIXED = 1,
  // AM, FM and SSB
  SDRKIT_VOICE_CHANNELS_GEN_SCENE_AM_FM_SSB = 2,
  // FM with tones
  SDRKIT_VOICE_CHANNELS_GEN_SCENE_FM_TONES = 3,
  // Upper sideband
  SDRKIT_VOICE_CHANNELS_GEN_SCENE_SIDEBAND = 4,
} sdrkit_voice_channels_gen_scene;

// The options of the setting "bit_rate" of the stack "fleetsync".
typedef enum sdrkit_fleetsync_bit_rate {
  // Automatic
  SDRKIT_FLEETSYNC_BIT_RATE_AUTO = 0,
  // 1200 bit/s
  SDRKIT_FLEETSYNC_BIT_RATE_1200_BIT_S = 1,
  // 2400 bit/s
  SDRKIT_FLEETSYNC_BIT_RATE_2400_BIT_S = 2,
} sdrkit_fleetsync_bit_rate;

// The options of the setting "gen_bit_rate" of the stack "fleetsync".
typedef enum sdrkit_fleetsync_gen_bit_rate {
  // 1200 bit/s
  SDRKIT_FLEETSYNC_GEN_BIT_RATE_1200_BIT_S = 0,
  // 2400 bit/s
  SDRKIT_FLEETSYNC_GEN_BIT_RATE_2400_BIT_S = 1,
} sdrkit_fleetsync_gen_bit_rate;

// The options of the setting "gen_frame" of the stack "fleetsync".
typedef enum sdrkit_fleetsync_gen_frame {
  // Both
  SDRKIT_FLEETSYNC_GEN_FRAME_BOTH = 0,
  // FleetSync
  SDRKIT_FLEETSYNC_GEN_FRAME_PLAIN = 1,
  // FleetSync II
  SDRKIT_FLEETSYNC_GEN_FRAME_FS2 = 2,
} sdrkit_fleetsync_gen_frame;

// The options of `state` of `sdrkit_voice_detector_fields`.
typedef enum sdrkit_voice_detector_fields_state {
  // Searching
  SDRKIT_VOICE_DETECTOR_FIELDS_STATE_SEARCHING = 0,
  // Signal found
  SDRKIT_VOICE_DETECTOR_FIELDS_STATE_SIGNAL = 1,
  // Decoded or confirmed
  SDRKIT_VOICE_DETECTOR_FIELDS_STATE_CONFIRMED = 2,
} sdrkit_voice_detector_fields_state;

// The result fields that every single-channel stack shares.
typedef struct sdrkit_voice_detector_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // How far the detector is. A value of `sdrkit_voice_detector_fields_state`.
  int32_t state;
  // True if `level_dbfs` holds a value.
  bool has_level_dbfs;
  // Level of the signal or channel, dBFS (a mean power of 1 is 0 dBFS). Unit: dBFS.
  double level_dbfs;
  // True if `quality_db` holds a value.
  bool has_quality_db;
  // The quality measure of the stack. Unit: dB.
  double quality_db;
  // The signal time. A length of time in seconds.
  double signal_time_s;
  // The rate of the input. A frequency in Hz.
  double input_rate_hz;
  // The count of good results.
  int64_t good;
  // The count of bad results.
  int64_t bad;
} sdrkit_voice_detector_fields;

// The result fields of HF packet.
typedef struct sdrkit_hfpacket_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The tuning correction. A frequency in Hz.
  double tone_offset_hz;
  // Different sources heard.
  int64_t stations;
  // True if `last_frame` holds a value.
  bool has_last_frame;
  // The newest frame in the monitor form.
  sdrkit_string last_frame;
} sdrkit_hfpacket_fields;

// The count of the input guard of a single-channel stack.
typedef struct sdrkit_voice_guard_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Input samples replaced because they were not finite or far beyond full scale.
  int64_t bad_samples;
} sdrkit_voice_guard_fields;

// An event of a single-channel stack. The row key is the number of the event.
typedef struct sdrkit_voice_detector_event_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // True if `time_utc` holds a value.
  bool has_time_utc;
  // The wall-clock time of an event. Seconds since 1970 (UTC).
  double time_utc;
  // The signal time. A length of time in seconds.
  double signal_time_s;
  // The text of a decoded message.
  sdrkit_string text;
} sdrkit_voice_detector_event_row;

// A frame with a good FCS. The row key is the number of the frame.
typedef struct sdrkit_hfpacket_frame_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // True if `time_utc` holds a value.
  bool has_time_utc;
  // When a call started. Seconds since 1970 (UTC).
  double time_utc;
  // The sender of a decoded message: a registration, an address, a call sign or a code.
  sdrkit_string source;
  // The source of a frame.
  sdrkit_string destination;
  // The digipeater path of a frame.
  sdrkit_string path;
  // The frame type.
  sdrkit_string type;
  // True if `pid` holds a value.
  bool has_pid;
  // The protocol identifier.
  int64_t pid;
  // The information field as text.
  sdrkit_string info;
  // The tuning correction. A frequency in Hz.
  double tone_offset_hz;
} sdrkit_hfpacket_frame_row;

// The result fields of the 121.5 MHz ELT detector.
typedef struct sdrkit_elt121_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The periods of the sweeps confirm the signal.
  bool confirmed;
  // True if `sweep_rate_hz` holds a value.
  bool has_sweep_rate_hz;
  // Sweeps a second. Unit: Hz.
  double sweep_rate_hz;
  // True if `sweep_top_hz` holds a value.
  bool has_sweep_top_hz;
  // The first frequency of a sweep. A frequency in Hz.
  double sweep_top_hz;
  // True if `sweep_bottom_hz` holds a value.
  bool has_sweep_bottom_hz;
  // The lowest frequency of a sweep. A frequency in Hz.
  double sweep_bottom_hz;
  // True if `depth_pct` holds a value.
  bool has_depth_pct;
  // The modulation of the tone. Unit: %.
  double depth_pct;
} sdrkit_elt121_fields;

// The result fields of the voice inversion detector.
typedef struct sdrkit_voice_inversion_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Inverted speech is there.
  bool inverted;
  // True if `carrier_hz` holds a value.
  bool has_carrier_hz;
  // The inversion carrier found. A frequency in Hz.
  double carrier_hz;
  // True if `fit_heard` holds a value.
  bool has_fit_heard;
  // The fit of the spectrum as heard.
  double fit_heard;
  // True if `fit_inverted` holds a value.
  bool has_fit_inverted;
  // The best fit of the flipped spectrum.
  double fit_inverted;
} sdrkit_voice_inversion_fields;

// The options of `submode` of `sdrkit_voice_channels_fields`.
typedef enum sdrkit_voice_channels_fields_submode {
  // CB radio
  SDRKIT_VOICE_CHANNELS_FIELDS_SUBMODE_CB = 0,
  // FRS / GMRS
  SDRKIT_VOICE_CHANNELS_FIELDS_SUBMODE_FRS = 1,
  // PMR446 analog
  SDRKIT_VOICE_CHANNELS_FIELDS_SUBMODE_PMR446 = 2,
  // Ham FM repeaters
  SDRKIT_VOICE_CHANNELS_FIELDS_SUBMODE_HAM_REPEATERS = 3,
  // HFGCS / HF voice
  SDRKIT_VOICE_CHANNELS_FIELDS_SUBMODE_HFVOICE = 4,
} sdrkit_voice_channels_fields_submode;

// The options of `scan_state` of `sdrkit_voice_channels_fields`.
typedef enum sdrkit_voice_channels_fields_scan_state {
  // Idle
  SDRKIT_VOICE_CHANNELS_FIELDS_SCAN_STATE_IDLE = 0,
  // Priority channel
  SDRKIT_VOICE_CHANNELS_FIELDS_SCAN_STATE_PRIORITY = 1,
  // Looking at the priority channel
  SDRKIT_VOICE_CHANNELS_FIELDS_SCAN_STATE_PRIORITY_PROBE = 2,
  // Scanning
  SDRKIT_VOICE_CHANNELS_FIELDS_SCAN_STATE_SCANNING = 3,
  // Holding
  SDRKIT_VOICE_CHANNELS_FIELDS_SCAN_STATE_HOLDING = 4,
} sdrkit_voice_channels_fields_scan_state;

// The result fields of the multi-carrier receiver.
typedef struct sdrkit_voice_channels_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // True if `submode` holds a value.
  bool has_submode;
  // The service. A value of `sdrkit_voice_channels_fields_submode`.
  int32_t submode;
  // True if `plan` holds a value.
  bool has_plan;
  // The name of the plan in use. Absent without a plan.
  sdrkit_string plan;
  // The channels of the plan in use.
  int64_t plan_channels;
  // The raster step in use. A frequency in Hz.
  double raster_step_hz;
  // Carriers followed now.
  int64_t carriers;
  // Channels with a carrier now.
  int64_t active_channels;
  // Channels with an open squelch.
  int64_t open_channels;
  // True if `playing` holds a value.
  bool has_playing;
  // The channel that plays.
  sdrkit_string playing;
  // True if `playing_hz` holds a value.
  bool has_playing_hz;
  // The frequency that plays. A frequency in Hz.
  double playing_hz;
  // The scan is on.
  bool scan;
  // What the player does. A value of `sdrkit_voice_channels_fields_scan_state`.
  int32_t scan_state;
  // The scan steps the radio.
  bool single_tuner;
  // True if `tune_hz` holds a value.
  bool has_tune_hz;
  // Where the stepping scan requires the radio. A frequency in Hz.
  double tune_hz;
  // The capture holds more than one channel.
  bool multi_channel;
  // Calls since the start.
  int64_t calls;
  // The input rate. A frequency in Hz.
  double input_rate_hz;
  // The centre of the capture. A frequency in Hz.
  double centre_hz;
  // The signal time. A length of time in seconds.
  double signal_time_s;
  // Reports made.
  int64_t reports;
} sdrkit_voice_channels_fields;

// A call in the call log.
typedef struct sdrkit_voice_call_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The number of a call.
  int64_t serial;
  // True if `time_utc` holds a value.
  bool has_time_utc;
  // When a call started. Seconds since 1970 (UTC).
  double time_utc;
  // The channel key.
  int64_t key_;
  // The number of the plan.
  int64_t number;
  // The short label.
  sdrkit_string label;
  // Where to listen. A frequency in Hz.
  double hz;
  // The signal time a call started. A length of time in seconds.
  double start_s;
  // How long a call lasted. A length of time in seconds.
  double duration_s;
  // The highest level. Unit: dBFS.
  double peak_dbfs;
  // The mean level of a call. Unit: dBFS.
  double mean_dbfs;
  // The demodulator of a call.
  sdrkit_string demod;
  // The tone or code heard.
  sdrkit_string tone;
  // The squelch is open.
  bool open;
} sdrkit_voice_call_row;

// The options of `demod` of `sdrkit_voice_channel_row`.
typedef enum sdrkit_voice_channel_row_demod {
  // FM
  SDRKIT_VOICE_CHANNEL_ROW_DEMOD_FM = 0,
  // AM
  SDRKIT_VOICE_CHANNEL_ROW_DEMOD_AM = 1,
  // USB
  SDRKIT_VOICE_CHANNEL_ROW_DEMOD_USB = 2,
  // LSB
  SDRKIT_VOICE_CHANNEL_ROW_DEMOD_LSB = 3,
} sdrkit_voice_channel_row_demod;

// A channel of the plan or a carrier heard.
typedef struct sdrkit_voice_channel_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The channel key.
  int64_t key_;
  // The number of the plan.
  int64_t number;
  // The short label.
  sdrkit_string label;
  // The use of the channel.
  sdrkit_string name;
  // Where to listen. A frequency in Hz.
  double hz;
  // True if `input_hz` holds a value.
  bool has_input_hz;
  // The repeater input. A frequency in Hz.
  double input_hz;
  // A carrier is there.
  bool active;
  // The squelch is open.
  bool open;
  // The channel plays.
  bool playing;
  // The channel is in the scan list.
  bool in_scan;
  // The schedule says the channel broadcasts now.
  bool on_air;
  // True if `level_dbfs` holds a value.
  bool has_level_dbfs;
  // The channel level. Unit: dBFS.
  double level_dbfs;
  // The demodulator in use. A value of `sdrkit_voice_channel_row_demod`.
  int32_t demod;
  // The tone or code heard.
  sdrkit_string tone;
  // The other reading of a DCS word.
  sdrkit_string tone_alias;
  // The last tone or code heard on the channel.
  sdrkit_string last_tone;
  // Calls on the channel.
  int64_t calls;
  // The time the channel was open in all. A length of time in seconds.
  double total_s;
  // True if `last_heard_s` holds a value.
  bool has_last_heard_s;
  // The time since the channel was last open. A length of time in seconds.
  double last_heard_s;
  // True if `last_heard_utc` holds a value.
  bool has_last_heard_utc;
  // When the channel was last open. Seconds since 1970 (UTC).
  double last_heard_utc;
  // True if `peak_dbfs` holds a value.
  bool has_peak_dbfs;
  // The highest level. Unit: dBFS.
  double peak_dbfs;
} sdrkit_voice_channel_row;

// The result fields of the MDC-1200 burst decoder.
typedef struct sdrkit_mdc1200_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Bursts so far.
  int64_t bursts;
  // Blocks whose check value was wrong.
  int64_t check_failures;
} sdrkit_mdc1200_fields;

// A burst in the burst log.
typedef struct sdrkit_mdc1200_burst_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The number of a call.
  int64_t serial;
  // True if `time_utc` holds a value.
  bool has_time_utc;
  // When a call started. Seconds since 1970 (UTC).
  double time_utc;
  // Where to listen. A frequency in Hz.
  double hz;
  // The short label.
  sdrkit_string label;
  // The unit id, 4 hex digits.
  sdrkit_string unit_id;
  // The operation code, 2 hex digits.
  sdrkit_string op;
  // The argument, 2 hex digits.
  sdrkit_string arg;
  // A double packet.
  bool double_;
  // The four bytes of the second block of a double packet, 8 hex digits.
  sdrkit_string extra;
  // The meaning of the packet.
  sdrkit_string text;
} sdrkit_mdc1200_burst_row;

// The result fields of the wireless microphone finder.
typedef struct sdrkit_wmic_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Candidates now.
  int64_t candidates;
  // True if `plan` holds a value.
  bool has_plan;
  // The name of the band plan.
  sdrkit_string plan;
  // The input rate. A frequency in Hz.
  double input_rate_hz;
  // The centre of the capture. A frequency in Hz.
  double centre_hz;
  // The signal time. A length of time in seconds.
  double signal_time_s;
  // Reports made.
  int64_t reports;
} sdrkit_wmic_fields;

// A carrier that can be a wireless microphone.
typedef struct sdrkit_wmic_candidate_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The carrier id of the finder.
  int64_t id;
  // Where to listen. A frequency in Hz.
  double hz;
  // The occupied width. A frequency in Hz.
  double width_hz;
  // The channel level. Unit: dBFS.
  double level_dbfs;
  // The signal-to-noise ratio. Unit: dB.
  double snr_db;
  // True if `deviation_hz` holds a value.
  bool has_deviation_hz;
  // The peak deviation. A frequency in Hz.
  double deviation_hz;
  // It looks like analogue FM.
  bool fm;
  // Above the threshold now.
  bool active;
  // This candidate plays.
  bool listening;
  // The time since it was first reported. A length of time in seconds.
  double age_s;
  // The band of the plan that holds it.
  sdrkit_string band;
} sdrkit_wmic_candidate_row;

// The result fields of the FleetSync burst decoder.
typedef struct sdrkit_fleetsync_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Bursts so far.
  int64_t bursts;
  // Frames found by their sync word whose check failed.
  int64_t check_failures;
  // Fleet and unit pairs heard.
  int64_t units;
} sdrkit_fleetsync_fields;

// A FleetSync burst: a row of the burst log and the fields of its event.
typedef struct sdrkit_fleetsync_burst_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The number of the burst.
  int64_t serial;
  // True if `time_utc` holds a value.
  bool has_time_utc;
  // When it was decoded. Seconds since 1970 (UTC).
  double time_utc;
  // The frequency of the carrier. A frequency in Hz.
  double hz;
  // The frequency in MHz, 4 decimals.
  sdrkit_string label;
  // The fleet number.
  int64_t fleet;
  // The unit number.
  int64_t unit;
  // The fleet is 100 to 349 and the unit 1000 to 4999, the ranges of the maker's manuals.
  bool in_range;
  // "FleetSync" or "FleetSync II".
  sdrkit_string kind;
  // The signalling rate. Unit: bit/s.
  int64_t bit_rate;
  // Word 1, 8 hex digits.
  sdrkit_string word1;
  // Word 2, 8 hex digits.
  sdrkit_string word2;
  // Word 1 bits 31 to 16 and word 2 bits 27 to 16, not interpreted: 4 and 3 hex digits.
  sdrkit_string raw;
  // "good", or for FleetSync II the bits that the code repaired.
  sdrkit_string check;
  // The burst in words.
  sdrkit_string text;
} sdrkit_fleetsync_burst_row;

// A fleet and unit heard.
typedef struct sdrkit_fleetsync_unit_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The fleet number.
  int64_t fleet;
  // The unit number.
  int64_t unit;
  // The fleet is 100 to 349 and the unit 1000 to 4999, the ranges of the maker's manuals.
  bool in_range;
  // True if `heard` holds a value.
  bool has_heard;
  // Bursts heard.
  int64_t heard;
  // True if `last_heard` holds a value.
  bool has_last_heard;
  // When it was last heard. Seconds since 1970 (UTC).
  double last_heard;
  // The carrier of its last burst. A frequency in Hz.
  double hz;
  // The type of its last burst.
  sdrkit_string kind;
} sdrkit_fleetsync_unit_row;

// A frame with a good FCS.
typedef struct sdrkit_hfpacket_frame_event {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Seconds since the object started, in the time of the signal.
  double time;
  // The frequency of the signal in Hz.
  double freq_hz;
  // The text of a decoded message.
  sdrkit_string text;
  // The sender of a decoded message: a registration, an address, a call sign or a code.
  sdrkit_string source;
  // The source of a frame.
  sdrkit_string destination;
  // The digipeater path of a frame.
  sdrkit_string path;
  // The frame type.
  sdrkit_string type;
  // The information field as text.
  sdrkit_string info;
  // The tuning correction. A frequency in Hz.
  double tone_offset_hz;
} sdrkit_hfpacket_frame_event;

// A station heard for the first time.
typedef struct sdrkit_hfpacket_station_event {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Seconds since the object started, in the time of the signal.
  double time;
  // The frequency of the signal in Hz.
  double freq_hz;
  // The text of the event.
  sdrkit_string text;
  // The station.
  sdrkit_string source;
} sdrkit_hfpacket_station_event;

// An ELT tone found.
typedef struct sdrkit_elt121_found_event {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Seconds since the object started, in the time of the signal.
  double time;
  // The frequency of the signal in Hz.
  double freq_hz;
  // The text of a decoded message.
  sdrkit_string text;
  // Sweeps a second. Unit: Hz.
  double sweep_rate_hz;
  // The first frequency of a sweep. A frequency in Hz.
  double sweep_top_hz;
  // The lowest frequency of a sweep. A frequency in Hz.
  double sweep_bottom_hz;
} sdrkit_elt121_found_event;

// An ELT tone ended.
typedef struct sdrkit_elt121_ended_event {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Seconds since the object started, in the time of the signal.
  double time;
  // The frequency of the signal in Hz.
  double freq_hz;
  // The text of a decoded message.
  sdrkit_string text;
} sdrkit_elt121_ended_event;

// Inverted speech found.
typedef struct sdrkit_voice_inversion_found_event {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Seconds since the object started, in the time of the signal.
  double time;
  // The frequency of the signal in Hz.
  double freq_hz;
  // The text of a decoded message.
  sdrkit_string text;
  // The inversion carrier found. A frequency in Hz.
  double carrier_hz;
} sdrkit_voice_inversion_found_event;

// Speech that is not inverted.
typedef struct sdrkit_voice_inversion_clear_event {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Seconds since the object started, in the time of the signal.
  double time;
  // The frequency of the signal in Hz.
  double freq_hz;
  // The text of a decoded message.
  sdrkit_string text;
} sdrkit_voice_inversion_clear_event;

// A call that ended.
typedef struct sdrkit_voice_channels_call_event {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Seconds since the object started, in the time of the signal.
  double time;
  // The frequency of the signal in Hz.
  double freq_hz;
  // The number of a call.
  int64_t serial;
  // The channel key.
  int64_t key;
  // The number of the plan.
  int64_t number;
  // The short label.
  sdrkit_string label;
  // The signal time a call started. A length of time in seconds.
  double start_s;
  // How long a call lasted. A length of time in seconds.
  double duration_s;
  // The highest level. Unit: dBFS.
  double peak_dbfs;
  // The mean level of a call. Unit: dBFS.
  double mean_dbfs;
  // The demodulator of a call.
  sdrkit_string demod;
  // The tone or code heard.
  sdrkit_string tone;
} sdrkit_voice_channels_call_event;

// A call that ended.
typedef struct sdrkit_mdc1200_call_event {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Seconds since the object started, in the time of the signal.
  double time;
  // The frequency of the signal in Hz.
  double freq_hz;
  // The number of a call.
  int64_t serial;
  // The channel key.
  int64_t key;
  // The number of the plan.
  int64_t number;
  // The short label.
  sdrkit_string label;
  // The signal time a call started. A length of time in seconds.
  double start_s;
  // How long a call lasted. A length of time in seconds.
  double duration_s;
  // The highest level. Unit: dBFS.
  double peak_dbfs;
  // The mean level of a call. Unit: dBFS.
  double mean_dbfs;
  // The demodulator of a call.
  sdrkit_string demod;
  // The tone or code heard.
  sdrkit_string tone;
} sdrkit_mdc1200_call_event;

// A burst that was decoded.
typedef struct sdrkit_mdc1200_burst_event {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Seconds since the object started, in the time of the signal.
  double time;
  // The frequency of the signal in Hz.
  double freq_hz;
  // The number of a call.
  int64_t serial;
  // True if `time_utc` holds a value.
  bool has_time_utc;
  // When a call started. Seconds since 1970 (UTC).
  double time_utc;
  // Where to listen. A frequency in Hz.
  double hz;
  // The short label.
  sdrkit_string label;
  // The unit id, 4 hex digits.
  sdrkit_string unit_id;
  // The operation code, 2 hex digits.
  sdrkit_string op;
  // The argument, 2 hex digits.
  sdrkit_string arg;
  // A double packet.
  bool double_;
  // The four bytes of the second block of a double packet, 8 hex digits.
  sdrkit_string extra;
  // The meaning of the packet.
  sdrkit_string text;
} sdrkit_mdc1200_burst_event;

// A new candidate carrier.
typedef struct sdrkit_wmic_candidate_event {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Seconds since the object started, in the time of the signal.
  double time;
  // The frequency of the signal in Hz.
  double freq_hz;
  // The carrier id of the finder.
  int64_t id;
  // Where to listen. A frequency in Hz.
  double hz;
  // The occupied width. A frequency in Hz.
  double width_hz;
  // The channel level. Unit: dBFS.
  double level_dbfs;
  // The signal-to-noise ratio. Unit: dB.
  double snr_db;
  // True if `deviation_hz` holds a value.
  bool has_deviation_hz;
  // The peak deviation. A frequency in Hz.
  double deviation_hz;
  // It looks like analogue FM.
  bool fm;
  // The band of the plan that holds it.
  sdrkit_string band;
} sdrkit_wmic_candidate_event;

// A call that ended.
typedef struct sdrkit_fleetsync_call_event {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Seconds since the object started, in the time of the signal.
  double time;
  // The frequency of the signal in Hz.
  double freq_hz;
  // The number of a call.
  int64_t serial;
  // The channel key.
  int64_t key;
  // The number of the plan.
  int64_t number;
  // The short label.
  sdrkit_string label;
  // The signal time a call started. A length of time in seconds.
  double start_s;
  // How long a call lasted. A length of time in seconds.
  double duration_s;
  // The highest level. Unit: dBFS.
  double peak_dbfs;
  // The mean level of a call. Unit: dBFS.
  double mean_dbfs;
  // The demodulator of a call.
  sdrkit_string demod;
  // The tone or code heard.
  sdrkit_string tone;
} sdrkit_fleetsync_call_event;

// A FleetSync burst: a row of the burst log and the fields of its event.
typedef struct sdrkit_fleetsync_burst_event {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Seconds since the object started, in the time of the signal.
  double time;
  // The frequency of the signal in Hz.
  double freq_hz;
  // The number of the burst.
  int64_t serial;
  // True if `time_utc` holds a value.
  bool has_time_utc;
  // When it was decoded. Seconds since 1970 (UTC).
  double time_utc;
  // The frequency of the carrier. A frequency in Hz.
  double hz;
  // The frequency in MHz, 4 decimals.
  sdrkit_string label;
  // The fleet number.
  int64_t fleet;
  // The unit number.
  int64_t unit;
  // The fleet is 100 to 349 and the unit 1000 to 4999, the ranges of the maker's manuals.
  bool in_range;
  // "FleetSync" or "FleetSync II".
  sdrkit_string kind;
  // The signalling rate. Unit: bit/s.
  int64_t bit_rate;
  // Word 1, 8 hex digits.
  sdrkit_string word1;
  // Word 2, 8 hex digits.
  sdrkit_string word2;
  // Word 1 bits 31 to 16 and word 2 bits 27 to 16, not interpreted: 4 and 3 hex digits.
  sdrkit_string raw;
  // "good", or for FleetSync II the bits that the code repaired.
  sdrkit_string check;
  // The burst in words.
  sdrkit_string text;
} sdrkit_fleetsync_burst_event;

// The arguments of `listen`.
typedef struct sdrkit_wmic_listen_args {
  // The size of this struct: sizeof of the type. The caller sets it.
  uint32_t size;
  // The id of a candidate, as the `candidates` table shows it.
  int64_t id;
} sdrkit_wmic_listen_args;

// An arguments struct with its size set and every argument at 0.
#define SDRKIT_WMIC_LISTEN_ARGS_INIT {sizeof(sdrkit_wmic_listen_args), 0}

// Fills `rows` with the changed rows of the table "events" of the stack "hfpacket" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The rows are the messages of the stack.
// Text: `text`. Time: `time_utc`.
static inline int32_t sdrkit_hfpacket_events_rows(SDRKIT_HANDLE handle,
                                                 sdrkit_voice_detector_event_row *rows,
                                                 uint32_t capacity,
                                                 sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "voice.DetectorEventRow", "events", rows, sizeof(sdrkit_voice_detector_event_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "frames" of the stack "hfpacket" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The rows are a log: they arrive over time.
// The column `source` names a row.
// Time: `time_utc`.
static inline int32_t sdrkit_hfpacket_frames_rows(SDRKIT_HANDLE handle,
                                                 sdrkit_hfpacket_frame_row *rows,
                                                 uint32_t capacity,
                                                 sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "voice.HfpacketFrameRow", "frames", rows, sizeof(sdrkit_hfpacket_frame_row), capacity, info);
}

// Fills `event` with event `n` of the last sdrkit_events_take() on `handle`, an event of the type "hfpacket.frame"
// (Frame). Returns SDRKIT_E_WRONG_TYPE if the event is of another type.
static inline int32_t sdrkit_hfpacket_frame_event_read(SDRKIT_HANDLE handle, uint32_t n, sdrkit_hfpacket_frame_event *event) {
  return sdrkit_typed_event(handle, n, "hfpacket.frame", "voice.HfpacketFrameEvent", event, sizeof(sdrkit_hfpacket_frame_event));
}

// Fills `event` with event `n` of the last sdrkit_events_take() on `handle`, an event of the type "hfpacket.station"
// (Station). Returns SDRKIT_E_WRONG_TYPE if the event is of another type.
static inline int32_t sdrkit_hfpacket_station_event_read(SDRKIT_HANDLE handle, uint32_t n, sdrkit_hfpacket_station_event *event) {
  return sdrkit_typed_event(handle, n, "hfpacket.station", "voice.HfpacketStationEvent", event, sizeof(sdrkit_hfpacket_station_event));
}

// Runs the command "clear" (Clear) of the stack "hfpacket" on the session of `handle`. Forgets everything decoded so far.
static inline int32_t sdrkit_hfpacket_clear(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "clear", NULL, NULL, NULL, NULL, 0);
}

// The settings of the test signal generator of the stack "hfpacket" are the sdrkit_hfpacket_gen_*() functions
// after the settings of its receiver.

// The setting "sideband" (Sideband) of the stack "hfpacket". A value of `sdrkit_hfpacket_sideband`. Where the dial sits: below the tones (upper sideband) or above them (lower sideband).
static inline int32_t sdrkit_hfpacket_get_sideband(SDRKIT_HANDLE handle, sdrkit_hfpacket_sideband *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "sideband", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_hfpacket_sideband)index;
  return code;
}
static inline int32_t sdrkit_hfpacket_set_sideband(SDRKIT_HANDLE handle, sdrkit_hfpacket_sideband value) {
  return sdrkit_typed_setting_set_enum(handle, "sideband", (int32_t)value);
}

// The setting "gen_frames" (Test signal: frames) of the test signal generator of the stack "hfpacket".
static inline int32_t sdrkit_hfpacket_gen_get_frames(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "gen_frames", value);
}
static inline int32_t sdrkit_hfpacket_gen_set_frames(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "gen_frames", value);
}

// The setting "gen_lead_s" (Test signal: lead) of the test signal generator of the stack "hfpacket". A length of time in seconds.
static inline int32_t sdrkit_hfpacket_gen_get_lead_s(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_lead_s", value);
}
static inline int32_t sdrkit_hfpacket_gen_set_lead_s(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_lead_s", value);
}

// The setting "gen_tail_s" (Test signal: tail) of the test signal generator of the stack "hfpacket". A length of time in seconds.
static inline int32_t sdrkit_hfpacket_gen_get_tail_s(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_tail_s", value);
}
static inline int32_t sdrkit_hfpacket_gen_set_tail_s(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_tail_s", value);
}

// The setting "gen_mark_hz" (Test signal: mark) of the test signal generator of the stack "hfpacket". A frequency in Hz.
static inline int32_t sdrkit_hfpacket_gen_get_mark_hz(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_mark_hz", value);
}
static inline int32_t sdrkit_hfpacket_gen_set_mark_hz(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_mark_hz", value);
}

// The setting "gen_space_hz" (Test signal: space) of the test signal generator of the stack "hfpacket". A frequency in Hz.
static inline int32_t sdrkit_hfpacket_gen_get_space_hz(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_space_hz", value);
}
static inline int32_t sdrkit_hfpacket_gen_set_space_hz(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_space_hz", value);
}

// Fills `rows` with the changed rows of the table "events" of the stack "elt121" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The rows are the messages of the stack.
// Text: `text`. Time: `time_utc`.
static inline int32_t sdrkit_elt121_events_rows(SDRKIT_HANDLE handle,
                                               sdrkit_voice_detector_event_row *rows,
                                               uint32_t capacity,
                                               sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "voice.DetectorEventRow", "events", rows, sizeof(sdrkit_voice_detector_event_row), capacity, info);
}

// Fills `event` with event `n` of the last sdrkit_events_take() on `handle`, an event of the type "elt121.found"
// (ELT tone found). Returns SDRKIT_E_WRONG_TYPE if the event is of another type.
static inline int32_t sdrkit_elt121_found_event_read(SDRKIT_HANDLE handle, uint32_t n, sdrkit_elt121_found_event *event) {
  return sdrkit_typed_event(handle, n, "elt121.found", "voice.Elt121FoundEvent", event, sizeof(sdrkit_elt121_found_event));
}

// Fills `event` with event `n` of the last sdrkit_events_take() on `handle`, an event of the type "elt121.ended"
// (ELT tone ended). Returns SDRKIT_E_WRONG_TYPE if the event is of another type.
static inline int32_t sdrkit_elt121_ended_event_read(SDRKIT_HANDLE handle, uint32_t n, sdrkit_elt121_ended_event *event) {
  return sdrkit_typed_event(handle, n, "elt121.ended", "voice.Elt121EndedEvent", event, sizeof(sdrkit_elt121_ended_event));
}

// Runs the command "clear" (Clear) of the stack "elt121" on the session of `handle`. Forgets everything decoded so far.
static inline int32_t sdrkit_elt121_clear(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "clear", NULL, NULL, NULL, NULL, 0);
}

// The settings of the test signal generator of the stack "elt121" are the sdrkit_elt121_gen_*() functions
// after the settings of its receiver.

// The setting "gen_sweep_hz" (Test signal: sweeps a second) of the test signal generator of the stack "elt121". Unit: Hz.
static inline int32_t sdrkit_elt121_gen_get_sweep_hz(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_sweep_hz", value);
}
static inline int32_t sdrkit_elt121_gen_set_sweep_hz(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_sweep_hz", value);
}

// The setting "gen_duty_pct" (Test signal: duty) of the test signal generator of the stack "elt121". Unit: %.
static inline int32_t sdrkit_elt121_gen_get_duty_pct(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_duty_pct", value);
}
static inline int32_t sdrkit_elt121_gen_set_duty_pct(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_duty_pct", value);
}

// The setting "gen_top_hz" (Test signal: from) of the test signal generator of the stack "elt121". A frequency in Hz. A value below the end frequency makes an upward sweep.
static inline int32_t sdrkit_elt121_gen_get_top_hz(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_top_hz", value);
}
static inline int32_t sdrkit_elt121_gen_set_top_hz(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_top_hz", value);
}

// The setting "gen_bottom_hz" (Test signal: to) of the test signal generator of the stack "elt121". A frequency in Hz.
static inline int32_t sdrkit_elt121_gen_get_bottom_hz(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_bottom_hz", value);
}
static inline int32_t sdrkit_elt121_gen_set_bottom_hz(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_bottom_hz", value);
}

// The setting "gen_depth" (Test signal: depth) of the test signal generator of the stack "elt121".
static inline int32_t sdrkit_elt121_gen_get_depth(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_depth", value);
}
static inline int32_t sdrkit_elt121_gen_set_depth(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_depth", value);
}

// The setting "gen_seconds" (Test signal: length) of the test signal generator of the stack "elt121". A length of time in seconds.
static inline int32_t sdrkit_elt121_gen_get_seconds(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_seconds", value);
}
static inline int32_t sdrkit_elt121_gen_set_seconds(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_seconds", value);
}

// Fills `rows` with the changed rows of the table "events" of the stack "voice_inversion" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The rows are the messages of the stack.
// Text: `text`. Time: `time_utc`.
static inline int32_t sdrkit_voice_inversion_events_rows(SDRKIT_HANDLE handle,
                                                        sdrkit_voice_detector_event_row *rows,
                                                        uint32_t capacity,
                                                        sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "voice.DetectorEventRow", "events", rows, sizeof(sdrkit_voice_detector_event_row), capacity, info);
}

// Fills `event` with event `n` of the last sdrkit_events_take() on `handle`, an event of the type "voice_inversion.found"
// (Inverted speech found). Returns SDRKIT_E_WRONG_TYPE if the event is of another type.
static inline int32_t sdrkit_voice_inversion_found_event_read(SDRKIT_HANDLE handle, uint32_t n, sdrkit_voice_inversion_found_event *event) {
  return sdrkit_typed_event(handle, n, "voice_inversion.found", "voice.VoiceInversionFoundEvent", event, sizeof(sdrkit_voice_inversion_found_event));
}

// Fills `event` with event `n` of the last sdrkit_events_take() on `handle`, an event of the type "voice_inversion.clear"
// (Speech, not inverted). Returns SDRKIT_E_WRONG_TYPE if the event is of another type.
static inline int32_t sdrkit_voice_inversion_clear_event_read(SDRKIT_HANDLE handle, uint32_t n, sdrkit_voice_inversion_clear_event *event) {
  return sdrkit_typed_event(handle, n, "voice_inversion.clear", "voice.VoiceInversionClearEvent", event, sizeof(sdrkit_voice_inversion_clear_event));
}

// Runs the command "clear" (Clear) of the stack "voice_inversion" on the session of `handle`. Forgets everything decoded so far.
static inline int32_t sdrkit_voice_inversion_clear(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "clear", NULL, NULL, NULL, NULL, 0);
}

// The settings of the test signal generator of the stack "voice_inversion" are the sdrkit_voice_inversion_gen_*() functions
// after the settings of its receiver.

// The setting "carrier_hz" (Carrier (Hz, 0 = automatic)) of the stack "voice_inversion". A frequency in Hz. 0 uses the carrier that the detector finds. A carrier is from 2000 to 4300 Hz.
static inline int32_t sdrkit_voice_inversion_get_carrier_hz(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "carrier_hz", value);
}
static inline int32_t sdrkit_voice_inversion_set_carrier_hz(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "carrier_hz", value);
}

// The setting "gen_invert_hz" (Test signal: inversion carrier) of the test signal generator of the stack "voice_inversion". A frequency in Hz. 0 sends clear speech.
static inline int32_t sdrkit_voice_inversion_gen_get_invert_hz(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_invert_hz", value);
}
static inline int32_t sdrkit_voice_inversion_gen_set_invert_hz(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_invert_hz", value);
}

// The setting "gen_seconds" (Test signal: length) of the test signal generator of the stack "voice_inversion". A length of time in seconds.
static inline int32_t sdrkit_voice_inversion_gen_get_seconds(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_seconds", value);
}
static inline int32_t sdrkit_voice_inversion_gen_set_seconds(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_seconds", value);
}

// The setting "gen_deviation_hz" (Test signal: deviation) of the test signal generator of the stack "voice_inversion". A frequency in Hz.
static inline int32_t sdrkit_voice_inversion_gen_get_deviation_hz(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_deviation_hz", value);
}
static inline int32_t sdrkit_voice_inversion_gen_set_deviation_hz(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_deviation_hz", value);
}

// Fills `rows` with the changed rows of the table "calls" of the stack "voice_channels" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The rows are the calls of the stack.
// The column `label` names a row.
// Time: `time_utc`.
static inline int32_t sdrkit_voice_channels_calls_rows(SDRKIT_HANDLE handle,
                                                      sdrkit_voice_call_row *rows,
                                                      uint32_t capacity,
                                                      sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "voice.CallRow", "calls", rows, sizeof(sdrkit_voice_call_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "channels" of the stack "voice_channels" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The rows are the groups of the stack.
// The column `name` names a row.
// Time: `last_heard_utc`.
static inline int32_t sdrkit_voice_channels_rows(SDRKIT_HANDLE handle,
                                                sdrkit_voice_channel_row *rows,
                                                uint32_t capacity,
                                                sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "voice.ChannelRow", "channels", rows, sizeof(sdrkit_voice_channel_row), capacity, info);
}

// Fills `event` with event `n` of the last sdrkit_events_take() on `handle`, an event of the type "voice_channels.call"
// (Call). Returns SDRKIT_E_WRONG_TYPE if the event is of another type.
static inline int32_t sdrkit_voice_channels_call_event_read(SDRKIT_HANDLE handle, uint32_t n, sdrkit_voice_channels_call_event *event) {
  return sdrkit_typed_event(handle, n, "voice_channels.call", "voice.VoiceChannelsCallEvent", event, sizeof(sdrkit_voice_channels_call_event));
}

// Runs the command "clear" (Clear) of the stack "voice_channels" on the session of `handle`. Forgets everything heard so far. Favourites stay.
static inline int32_t sdrkit_voice_channels_clear(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "clear", NULL, NULL, NULL, NULL, 0);
}

// The settings of the test signal generator of the stack "voice_channels" are the sdrkit_voice_channels_gen_*() functions
// after the settings of its receiver.

// The setting "plan" (Channel plan) of the stack "voice_channels". The path of a plan file in the host's storage, in a format that plans.import takes (docs/schema/plans.json). Empty for no plan: every carrier is then labelled by its frequency. A change forgets the carriers and the channel statistics.
static inline int32_t sdrkit_voice_channels_get_plan(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "plan", value);
}
static inline int32_t sdrkit_voice_channels_set_plan(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "plan", value);
}

// The setting "raster_step_hz" (Raster step) of the stack "voice_channels". A frequency in Hz. The spacing of the frequencies a carrier is searched on. 0 takes the narrowest channel of the plan, and without a plan the step that the stack or its submode fixes.
static inline int32_t sdrkit_voice_channels_get_raster_step_hz(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "raster_step_hz", value);
}
static inline int32_t sdrkit_voice_channels_set_raster_step_hz(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "raster_step_hz", value);
}

// The setting "raster_origin_hz" (Raster origin) of the stack "voice_channels". A frequency in Hz. A frequency the raster passes through. 0 takes the first channel of the plan, and without a plan the multiples of the step from 0 Hz.
static inline int32_t sdrkit_voice_channels_get_raster_origin_hz(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "raster_origin_hz", value);
}
static inline int32_t sdrkit_voice_channels_set_raster_origin_hz(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "raster_origin_hz", value);
}

// The setting "channel_width_hz" (Channel width) of the stack "voice_channels". A frequency in Hz. The width of a channel that no plan describes. 0 takes the width that the stack or its submode fixes.
static inline int32_t sdrkit_voice_channels_get_channel_width_hz(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "channel_width_hz", value);
}
static inline int32_t sdrkit_voice_channels_set_channel_width_hz(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "channel_width_hz", value);
}

// The setting "channel" (Channel) of the stack "voice_channels". A channel label of the plan, or auto for the strongest open channel. A label that the plan does not have is as auto.
static inline int32_t sdrkit_voice_channels_get_channel(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "channel", value);
}
static inline int32_t sdrkit_voice_channels_set_channel(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "channel", value);
}

// The setting "demod" (Demodulator) of the stack "voice_channels". A value of `sdrkit_voice_channels_demod`. Automatic uses the demodulator of the plan channel, else the one of the submode: FM, upper sideband, or for CB the one the signal shows.
static inline int32_t sdrkit_voice_channels_get_demod(SDRKIT_HANDLE handle, sdrkit_voice_channels_demod *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "demod", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_voice_channels_demod)index;
  return code;
}
static inline int32_t sdrkit_voice_channels_set_demod(SDRKIT_HANDLE handle, sdrkit_voice_channels_demod value) {
  return sdrkit_typed_setting_set_enum(handle, "demod", (int32_t)value);
}

// The setting "squelch" (Squelch) of the stack "voice_channels". Unit: %. 0 opens at the weakest signal.
static inline int32_t sdrkit_voice_channels_get_squelch(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "squelch", value);
}
static inline int32_t sdrkit_voice_channels_set_squelch(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "squelch", value);
}

// The setting "deemphasis" (De-emphasis) of the stack "voice_channels".
static inline int32_t sdrkit_voice_channels_get_deemphasis(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "deemphasis", value);
}
static inline int32_t sdrkit_voice_channels_set_deemphasis(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "deemphasis", value);
}

// The setting "scan" (Scan) of the stack "voice_channels".
static inline int32_t sdrkit_voice_channels_get_scan(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "scan", value);
}
static inline int32_t sdrkit_voice_channels_set_scan(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "scan", value);
}

// The setting "scan_channels" (Channels) of the stack "voice_channels". Channel labels of the plan. Empty scans every channel of the plan. Without a plan the scan has no channels.
static inline int32_t sdrkit_voice_channels_get_scan_channels(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "scan_channels", value);
}
static inline int32_t sdrkit_voice_channels_set_scan_channels(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "scan_channels", value);
}

// The setting "scan_dwell_s" (Dwell) of the stack "voice_channels". A length of time in seconds.
static inline int32_t sdrkit_voice_channels_get_scan_dwell_s(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "scan_dwell_s", value);
}
static inline int32_t sdrkit_voice_channels_set_scan_dwell_s(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "scan_dwell_s", value);
}

// The setting "scan_hang_s" (Hang) of the stack "voice_channels". A length of time in seconds.
static inline int32_t sdrkit_voice_channels_get_scan_hang_s(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "scan_hang_s", value);
}
static inline int32_t sdrkit_voice_channels_set_scan_hang_s(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "scan_hang_s", value);
}

// The setting "carriers" (Carriers at once) of the stack "voice_channels".
static inline int32_t sdrkit_voice_channels_get_carriers(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "carriers", value);
}
static inline int32_t sdrkit_voice_channels_set_carriers(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "carriers", value);
}

// The setting "show_heard" (Only heard) of the stack "voice_channels". Lists only the channels that are active now or had a call.
static inline int32_t sdrkit_voice_channels_get_show_heard(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "show_heard", value);
}
static inline int32_t sdrkit_voice_channels_set_show_heard(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "show_heard", value);
}

// The setting "time_base" (Time of the first sample) of the stack "voice_channels". Seconds since 1970 (UTC). UNIX time of the first sample of a recording. 0 when not known. Live samples use the computer clock.
static inline int32_t sdrkit_voice_channels_get_time_base(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "time_base", value);
}
static inline int32_t sdrkit_voice_channels_set_time_base(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "time_base", value);
}

// The setting "gen_cnr_db" (Test signal C/N) of the test signal generator of the stack "voice_channels". Unit: dB. The carrier-to-noise ratio of the test signal in 10 kHz.
static inline int32_t sdrkit_voice_channels_gen_get_cnr_db(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_cnr_db", value);
}
static inline int32_t sdrkit_voice_channels_gen_set_cnr_db(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_cnr_db", value);
}

// The setting "gen_scene" (Test signal scene) of the test signal generator of the stack "voice_channels". A value of `sdrkit_voice_channels_gen_scene`. The transmissions of the test signal, at fixed offsets from the centre of the capture. Automatic takes the scene of the submode, and the mixed scene when the generator is given no submode.
static inline int32_t sdrkit_voice_channels_gen_get_scene(SDRKIT_HANDLE handle, sdrkit_voice_channels_gen_scene *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "gen_scene", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_voice_channels_gen_scene)index;
  return code;
}
static inline int32_t sdrkit_voice_channels_gen_set_scene(SDRKIT_HANDLE handle, sdrkit_voice_channels_gen_scene value) {
  return sdrkit_typed_setting_set_enum(handle, "gen_scene", (int32_t)value);
}

// Fills `rows` with the changed rows of the table "calls" of the stack "mdc1200" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The rows are the calls of the stack.
// The column `label` names a row.
// Time: `time_utc`.
static inline int32_t sdrkit_mdc1200_calls_rows(SDRKIT_HANDLE handle,
                                               sdrkit_voice_call_row *rows,
                                               uint32_t capacity,
                                               sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "voice.CallRow", "calls", rows, sizeof(sdrkit_voice_call_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "channels" of the stack "mdc1200" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The rows are the groups of the stack.
// The column `name` names a row.
// Time: `last_heard_utc`.
static inline int32_t sdrkit_mdc1200_channels_rows(SDRKIT_HANDLE handle,
                                                  sdrkit_voice_channel_row *rows,
                                                  uint32_t capacity,
                                                  sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "voice.ChannelRow", "channels", rows, sizeof(sdrkit_voice_channel_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "bursts" of the stack "mdc1200" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The rows are the messages of the stack.
// The column `label` names a row.
// Sender: `unit_id`. Text: `text`. Time: `time_utc`.
static inline int32_t sdrkit_mdc1200_bursts_rows(SDRKIT_HANDLE handle,
                                                sdrkit_mdc1200_burst_row *rows,
                                                uint32_t capacity,
                                                sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "voice.Mdc1200BurstRow", "bursts", rows, sizeof(sdrkit_mdc1200_burst_row), capacity, info);
}

// Fills `event` with event `n` of the last sdrkit_events_take() on `handle`, an event of the type "mdc1200.call"
// (Call). Returns SDRKIT_E_WRONG_TYPE if the event is of another type.
static inline int32_t sdrkit_mdc1200_call_event_read(SDRKIT_HANDLE handle, uint32_t n, sdrkit_mdc1200_call_event *event) {
  return sdrkit_typed_event(handle, n, "mdc1200.call", "voice.Mdc1200CallEvent", event, sizeof(sdrkit_mdc1200_call_event));
}

// Fills `event` with event `n` of the last sdrkit_events_take() on `handle`, an event of the type "mdc1200.burst"
// (Burst). Returns SDRKIT_E_WRONG_TYPE if the event is of another type.
static inline int32_t sdrkit_mdc1200_burst_event_read(SDRKIT_HANDLE handle, uint32_t n, sdrkit_mdc1200_burst_event *event) {
  return sdrkit_typed_event(handle, n, "mdc1200.burst", "voice.Mdc1200BurstEvent", event, sizeof(sdrkit_mdc1200_burst_event));
}

// Runs the command "clear" (Clear) of the stack "mdc1200" on the session of `handle`. Forgets everything heard so far. Favourites stay.
static inline int32_t sdrkit_mdc1200_clear(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "clear", NULL, NULL, NULL, NULL, 0);
}

// The settings of the test signal generator of the stack "mdc1200" are the sdrkit_mdc1200_gen_*() functions
// after the settings of its receiver.

// The setting "raster_step_hz" (Raster step) of the stack "mdc1200". A frequency in Hz. The spacing of the frequencies a carrier is searched on. 0 takes the narrowest channel of the plan, and without a plan the step that the stack or its submode fixes.
static inline int32_t sdrkit_mdc1200_get_raster_step_hz(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "raster_step_hz", value);
}
static inline int32_t sdrkit_mdc1200_set_raster_step_hz(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "raster_step_hz", value);
}

// The setting "raster_origin_hz" (Raster origin) of the stack "mdc1200". A frequency in Hz. A frequency the raster passes through. 0 takes the first channel of the plan, and without a plan the multiples of the step from 0 Hz.
static inline int32_t sdrkit_mdc1200_get_raster_origin_hz(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "raster_origin_hz", value);
}
static inline int32_t sdrkit_mdc1200_set_raster_origin_hz(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "raster_origin_hz", value);
}

// The setting "channel_width_hz" (Channel width) of the stack "mdc1200". A frequency in Hz. The width of a channel that no plan describes. 0 takes the width that the stack or its submode fixes.
static inline int32_t sdrkit_mdc1200_get_channel_width_hz(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "channel_width_hz", value);
}
static inline int32_t sdrkit_mdc1200_set_channel_width_hz(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "channel_width_hz", value);
}

// The setting "squelch" (Squelch) of the stack "mdc1200". Unit: %. 0 opens at the weakest signal.
static inline int32_t sdrkit_mdc1200_get_squelch(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "squelch", value);
}
static inline int32_t sdrkit_mdc1200_set_squelch(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "squelch", value);
}

// The setting "deemphasis" (De-emphasis) of the stack "mdc1200".
static inline int32_t sdrkit_mdc1200_get_deemphasis(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "deemphasis", value);
}
static inline int32_t sdrkit_mdc1200_set_deemphasis(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "deemphasis", value);
}

// The setting "carriers" (Carriers at once) of the stack "mdc1200".
static inline int32_t sdrkit_mdc1200_get_carriers(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "carriers", value);
}
static inline int32_t sdrkit_mdc1200_set_carriers(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "carriers", value);
}

// The setting "time_base" (Time of the first sample) of the stack "mdc1200". Seconds since 1970 (UTC). UNIX time of the first sample of a recording. 0 when not known. Live samples use the computer clock.
static inline int32_t sdrkit_mdc1200_get_time_base(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "time_base", value);
}
static inline int32_t sdrkit_mdc1200_set_time_base(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "time_base", value);
}

// The setting "gen_cnr_db" (Test signal C/N) of the test signal generator of the stack "mdc1200". Unit: dB. The carrier-to-noise ratio of the test signal in 10 kHz.
static inline int32_t sdrkit_mdc1200_gen_get_cnr_db(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_cnr_db", value);
}
static inline int32_t sdrkit_mdc1200_gen_set_cnr_db(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_cnr_db", value);
}

// Fills `rows` with the changed rows of the table "candidates" of the stack "wmic" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// Default order: the column `snr_db`, highest first.
static inline int32_t sdrkit_wmic_candidates_rows(SDRKIT_HANDLE handle,
                                                 sdrkit_wmic_candidate_row *rows,
                                                 uint32_t capacity,
                                                 sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "voice.WmicCandidateRow", "candidates", rows, sizeof(sdrkit_wmic_candidate_row), capacity, info);
}

// Fills `event` with event `n` of the last sdrkit_events_take() on `handle`, an event of the type "wmic.candidate"
// (Candidate). Returns SDRKIT_E_WRONG_TYPE if the event is of another type.
static inline int32_t sdrkit_wmic_candidate_event_read(SDRKIT_HANDLE handle, uint32_t n, sdrkit_wmic_candidate_event *event) {
  return sdrkit_typed_event(handle, n, "wmic.candidate", "voice.WmicCandidateEvent", event, sizeof(sdrkit_wmic_candidate_event));
}

// Runs the command "clear" (Clear) of the stack "wmic" on the session of `handle`. Forgets everything heard so far. Favourites stay.
static inline int32_t sdrkit_wmic_clear(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "clear", NULL, NULL, NULL, NULL, 0);
}

// Runs the command "listen" (Listen) of the stack "wmic" on the session of `handle`. Plays a candidate. The candidate that plays stops.
static inline int32_t sdrkit_wmic_listen(SDRKIT_HANDLE handle, const sdrkit_wmic_listen_args *args) {
  return sdrkit_typed_command(handle, "listen", "voice.ListenArgs", args, NULL, NULL, 0);
}

// The settings of the test signal generator of the stack "wmic" are the sdrkit_wmic_gen_*() functions
// after the settings of its receiver.

// The setting "plan" (Band plan) of the stack "wmic". The path of a plan file in the host's storage, in a format that plans.import takes (docs/schema/plans.json). Its bands name the band of each candidate. Empty for no plan: the band of a candidate is then empty.
static inline int32_t sdrkit_wmic_get_plan(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "plan", value);
}
static inline int32_t sdrkit_wmic_set_plan(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "plan", value);
}

// The setting "raster_step_hz" (Raster step) of the stack "wmic". A frequency in Hz. The spacing of the frequencies a carrier is searched on. 0 takes the narrowest channel of the plan, and without a plan the step that the stack or its submode fixes.
static inline int32_t sdrkit_wmic_get_raster_step_hz(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "raster_step_hz", value);
}
static inline int32_t sdrkit_wmic_set_raster_step_hz(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "raster_step_hz", value);
}

// The setting "raster_origin_hz" (Raster origin) of the stack "wmic". A frequency in Hz. A frequency the raster passes through. 0 takes the first channel of the plan, and without a plan the multiples of the step from 0 Hz.
static inline int32_t sdrkit_wmic_get_raster_origin_hz(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "raster_origin_hz", value);
}
static inline int32_t sdrkit_wmic_set_raster_origin_hz(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "raster_origin_hz", value);
}

// The setting "squelch" (Squelch) of the stack "wmic". Unit: %. 0 opens at the weakest signal.
static inline int32_t sdrkit_wmic_get_squelch(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "squelch", value);
}
static inline int32_t sdrkit_wmic_set_squelch(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "squelch", value);
}

// The setting "carriers" (Carriers at once) of the stack "wmic".
static inline int32_t sdrkit_wmic_get_carriers(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "carriers", value);
}
static inline int32_t sdrkit_wmic_set_carriers(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "carriers", value);
}

// The setting "listen" (Listen) of the stack "wmic". The id of the candidate that plays. 0 plays nothing.
static inline int32_t sdrkit_wmic_get_listen(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "listen", value);
}
static inline int32_t sdrkit_wmic_set_listen(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "listen", value);
}

// The setting "gen_cnr_db" (Test signal C/N) of the test signal generator of the stack "wmic". Unit: dB. The carrier-to-noise ratio of the test signal in 10 kHz.
static inline int32_t sdrkit_wmic_gen_get_cnr_db(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_cnr_db", value);
}
static inline int32_t sdrkit_wmic_gen_set_cnr_db(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_cnr_db", value);
}

// Fills `rows` with the changed rows of the table "calls" of the stack "fleetsync" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The rows are the calls of the stack.
// The column `label` names a row.
// Time: `time_utc`.
static inline int32_t sdrkit_fleetsync_calls_rows(SDRKIT_HANDLE handle,
                                                 sdrkit_voice_call_row *rows,
                                                 uint32_t capacity,
                                                 sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "voice.CallRow", "calls", rows, sizeof(sdrkit_voice_call_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "channels" of the stack "fleetsync" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The rows are the groups of the stack.
// The column `name` names a row.
// Time: `last_heard_utc`.
static inline int32_t sdrkit_fleetsync_channels_rows(SDRKIT_HANDLE handle,
                                                    sdrkit_voice_channel_row *rows,
                                                    uint32_t capacity,
                                                    sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "voice.ChannelRow", "channels", rows, sizeof(sdrkit_voice_channel_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "bursts" of the stack "fleetsync" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The rows are the messages of the stack.
// The column `label` names a row.
// Sender: `unit`. Text: `text`. Time: `time_utc`.
static inline int32_t sdrkit_fleetsync_bursts_rows(SDRKIT_HANDLE handle,
                                                  sdrkit_fleetsync_burst_row *rows,
                                                  uint32_t capacity,
                                                  sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "voice.FleetsyncBurstRow", "bursts", rows, sizeof(sdrkit_fleetsync_burst_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "units" of the stack "fleetsync" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
// The rows are the stations or nodes of the stack.
// Time: `last_heard`.
static inline int32_t sdrkit_fleetsync_units_rows(SDRKIT_HANDLE handle,
                                                 sdrkit_fleetsync_unit_row *rows,
                                                 uint32_t capacity,
                                                 sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "voice.FleetsyncUnitRow", "units", rows, sizeof(sdrkit_fleetsync_unit_row), capacity, info);
}

// Fills `event` with event `n` of the last sdrkit_events_take() on `handle`, an event of the type "fleetsync.call"
// (Call). Returns SDRKIT_E_WRONG_TYPE if the event is of another type.
static inline int32_t sdrkit_fleetsync_call_event_read(SDRKIT_HANDLE handle, uint32_t n, sdrkit_fleetsync_call_event *event) {
  return sdrkit_typed_event(handle, n, "fleetsync.call", "voice.VoiceChannelsCallEvent", event, sizeof(sdrkit_fleetsync_call_event));
}

// Fills `event` with event `n` of the last sdrkit_events_take() on `handle`, an event of the type "fleetsync.burst"
// (Burst). Returns SDRKIT_E_WRONG_TYPE if the event is of another type.
static inline int32_t sdrkit_fleetsync_burst_event_read(SDRKIT_HANDLE handle, uint32_t n, sdrkit_fleetsync_burst_event *event) {
  return sdrkit_typed_event(handle, n, "fleetsync.burst", "voice.FleetsyncBurstRow", event, sizeof(sdrkit_fleetsync_burst_event));
}

// Runs the command "clear" (Clear) of the stack "fleetsync" on the session of `handle`. Forgets everything heard so far. Favourites stay.
static inline int32_t sdrkit_fleetsync_clear(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "clear", NULL, NULL, NULL, NULL, 0);
}

// The settings of the test signal generator of the stack "fleetsync" are the sdrkit_fleetsync_gen_*() functions
// after the settings of its receiver.

// The setting "raster_step_hz" (Raster step) of the stack "fleetsync". A frequency in Hz. The spacing of the frequencies a carrier is searched on. 0 takes the narrowest channel of the plan, and without a plan the step that the stack or its submode fixes.
static inline int32_t sdrkit_fleetsync_get_raster_step_hz(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "raster_step_hz", value);
}
static inline int32_t sdrkit_fleetsync_set_raster_step_hz(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "raster_step_hz", value);
}

// The setting "raster_origin_hz" (Raster origin) of the stack "fleetsync". A frequency in Hz. A frequency the raster passes through. 0 takes the first channel of the plan, and without a plan the multiples of the step from 0 Hz.
static inline int32_t sdrkit_fleetsync_get_raster_origin_hz(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "raster_origin_hz", value);
}
static inline int32_t sdrkit_fleetsync_set_raster_origin_hz(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "raster_origin_hz", value);
}

// The setting "channel_width_hz" (Channel width) of the stack "fleetsync". A frequency in Hz. The width of a channel that no plan describes. 0 takes the width that the stack or its submode fixes.
static inline int32_t sdrkit_fleetsync_get_channel_width_hz(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "channel_width_hz", value);
}
static inline int32_t sdrkit_fleetsync_set_channel_width_hz(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "channel_width_hz", value);
}

// The setting "squelch" (Squelch) of the stack "fleetsync". Unit: %. 0 opens at the weakest signal.
static inline int32_t sdrkit_fleetsync_get_squelch(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "squelch", value);
}
static inline int32_t sdrkit_fleetsync_set_squelch(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "squelch", value);
}

// The setting "deemphasis" (De-emphasis) of the stack "fleetsync".
static inline int32_t sdrkit_fleetsync_get_deemphasis(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "deemphasis", value);
}
static inline int32_t sdrkit_fleetsync_set_deemphasis(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "deemphasis", value);
}

// The setting "carriers" (Carriers at once) of the stack "fleetsync".
static inline int32_t sdrkit_fleetsync_get_carriers(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "carriers", value);
}
static inline int32_t sdrkit_fleetsync_set_carriers(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "carriers", value);
}

// The setting "bit_rate" (Signalling rate) of the stack "fleetsync". A value of `sdrkit_fleetsync_bit_rate`. Automatic reads 1200 and 2400 bit/s at once. The tones at 2400 bit/s (1200 and 2400 Hz) are not confirmed by a public document.
static inline int32_t sdrkit_fleetsync_get_bit_rate(SDRKIT_HANDLE handle, sdrkit_fleetsync_bit_rate *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "bit_rate", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_fleetsync_bit_rate)index;
  return code;
}
static inline int32_t sdrkit_fleetsync_set_bit_rate(SDRKIT_HANDLE handle, sdrkit_fleetsync_bit_rate value) {
  return sdrkit_typed_setting_set_enum(handle, "bit_rate", (int32_t)value);
}

// The setting "time_base" (Time of the first sample) of the stack "fleetsync". Seconds since 1970 (UTC). UNIX time of the first sample of a recording. 0 when not known. Live samples use the computer clock.
static inline int32_t sdrkit_fleetsync_get_time_base(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "time_base", value);
}
static inline int32_t sdrkit_fleetsync_set_time_base(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "time_base", value);
}

// The setting "gen_cnr_db" (Test signal C/N) of the test signal generator of the stack "fleetsync". Unit: dB. The carrier-to-noise ratio of the test signal in 10 kHz.
static inline int32_t sdrkit_fleetsync_gen_get_cnr_db(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gen_cnr_db", value);
}
static inline int32_t sdrkit_fleetsync_gen_set_cnr_db(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gen_cnr_db", value);
}

// The setting "gen_fleet" (Test signal fleet) of the test signal generator of the stack "fleetsync". The maker's manuals allow 100 to 349; the frame carries 99 to 354.
static inline int32_t sdrkit_fleetsync_gen_get_fleet(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "gen_fleet", value);
}
static inline int32_t sdrkit_fleetsync_gen_set_fleet(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "gen_fleet", value);
}

// The setting "gen_unit" (Test signal unit) of the test signal generator of the stack "fleetsync". The unit of the first carrier; the second carrier sends the next unit. The maker's manuals allow 1000 to 4999; the frame carries 999 to 5094.
static inline int32_t sdrkit_fleetsync_gen_get_unit(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "gen_unit", value);
}
static inline int32_t sdrkit_fleetsync_gen_set_unit(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "gen_unit", value);
}

// The setting "gen_bit_rate" (Test signal rate) of the test signal generator of the stack "fleetsync". A value of `sdrkit_fleetsync_gen_bit_rate`.
static inline int32_t sdrkit_fleetsync_gen_get_bit_rate(SDRKIT_HANDLE handle, sdrkit_fleetsync_gen_bit_rate *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "gen_bit_rate", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_fleetsync_gen_bit_rate)index;
  return code;
}
static inline int32_t sdrkit_fleetsync_gen_set_bit_rate(SDRKIT_HANDLE handle, sdrkit_fleetsync_gen_bit_rate value) {
  return sdrkit_typed_setting_set_enum(handle, "gen_bit_rate", (int32_t)value);
}

// The setting "gen_frame" (Test signal frames) of the test signal generator of the stack "fleetsync". A value of `sdrkit_fleetsync_gen_frame`.
static inline int32_t sdrkit_fleetsync_gen_get_frame(SDRKIT_HANDLE handle, sdrkit_fleetsync_gen_frame *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "gen_frame", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_fleetsync_gen_frame)index;
  return code;
}
static inline int32_t sdrkit_fleetsync_gen_set_frame(SDRKIT_HANDLE handle, sdrkit_fleetsync_gen_frame value) {
  return sdrkit_typed_setting_set_enum(handle, "gen_frame", (int32_t)value);
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_voice_detector_fields_read(SDRKIT_HANDLE handle, sdrkit_voice_detector_fields *fields) {
  return sdrkit_typed_fields(handle, "voice.DetectorFields", fields, sizeof(sdrkit_voice_detector_fields));
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_hfpacket_fields_read(SDRKIT_HANDLE handle, sdrkit_hfpacket_fields *fields) {
  return sdrkit_typed_fields(handle, "voice.HfpacketFields", fields, sizeof(sdrkit_hfpacket_fields));
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_elt121_fields_read(SDRKIT_HANDLE handle, sdrkit_elt121_fields *fields) {
  return sdrkit_typed_fields(handle, "voice.Elt121Fields", fields, sizeof(sdrkit_elt121_fields));
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_voice_inversion_fields_read(SDRKIT_HANDLE handle, sdrkit_voice_inversion_fields *fields) {
  return sdrkit_typed_fields(handle, "voice.VoiceInversionFields", fields, sizeof(sdrkit_voice_inversion_fields));
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_voice_channels_fields_read(SDRKIT_HANDLE handle, sdrkit_voice_channels_fields *fields) {
  return sdrkit_typed_fields(handle, "voice.VoiceChannelsFields", fields, sizeof(sdrkit_voice_channels_fields));
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_mdc1200_fields_read(SDRKIT_HANDLE handle, sdrkit_mdc1200_fields *fields) {
  return sdrkit_typed_fields(handle, "voice.Mdc1200Fields", fields, sizeof(sdrkit_mdc1200_fields));
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_wmic_fields_read(SDRKIT_HANDLE handle, sdrkit_wmic_fields *fields) {
  return sdrkit_typed_fields(handle, "voice.WmicFields", fields, sizeof(sdrkit_wmic_fields));
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_fleetsync_fields_read(SDRKIT_HANDLE handle, sdrkit_fleetsync_fields *fields) {
  return sdrkit_typed_fields(handle, "voice.FleetsyncFields", fields, sizeof(sdrkit_fleetsync_fields));
}

// Fills `fields` with the result fields of the component "guard" of the stack "hfpacket" in the snapshot of
// `handle`. A generic consumer reads them under the keys "guard.<key>".
static inline int32_t sdrkit_hfpacket_guard_fields_read(SDRKIT_HANDLE handle, sdrkit_voice_guard_fields *fields) {
  return sdrkit_typed_fields(handle, "voice.GuardFields@guard", fields, sizeof(sdrkit_voice_guard_fields));
}

// Fills `fields` with the result fields of the component "guard" of the stack "elt121" in the snapshot of
// `handle`. A generic consumer reads them under the keys "guard.<key>".
static inline int32_t sdrkit_elt121_guard_fields_read(SDRKIT_HANDLE handle, sdrkit_voice_guard_fields *fields) {
  return sdrkit_typed_fields(handle, "voice.GuardFields@guard", fields, sizeof(sdrkit_voice_guard_fields));
}

// Fills `fields` with the result fields of the component "guard" of the stack "voice_inversion" in the snapshot of
// `handle`. A generic consumer reads them under the keys "guard.<key>".
static inline int32_t sdrkit_voice_inversion_guard_fields_read(SDRKIT_HANDLE handle, sdrkit_voice_guard_fields *fields) {
  return sdrkit_typed_fields(handle, "voice.GuardFields@guard", fields, sizeof(sdrkit_voice_guard_fields));
}

#endif
