/*
 * sdrkit: the typed interface of the settings that every session has, version 1.4. MIT licence.
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

#ifndef SDRKIT_SESSION_H
#define SDRKIT_SESSION_H

#include "../sdrkit.h"

// The version of the family that this header describes.
#define SDRKIT_SESSION_VERSION_MAJOR 1
#define SDRKIT_SESSION_VERSION_MINOR 4

// The options of the setting "spectrum_size" of the stack "session".
typedef enum sdrkit_session_spectrum_size {
  // 256
  SDRKIT_SESSION_SPECTRUM_SIZE_256 = 0,
  // 512
  SDRKIT_SESSION_SPECTRUM_SIZE_512 = 1,
  // 1024
  SDRKIT_SESSION_SPECTRUM_SIZE_1024 = 2,
  // 2048
  SDRKIT_SESSION_SPECTRUM_SIZE_2048 = 3,
  // 4096
  SDRKIT_SESSION_SPECTRUM_SIZE_4096 = 4,
  // 8192
  SDRKIT_SESSION_SPECTRUM_SIZE_8192 = 5,
  // 16384
  SDRKIT_SESSION_SPECTRUM_SIZE_16384 = 6,
  // 32768
  SDRKIT_SESSION_SPECTRUM_SIZE_32768 = 7,
  // 65536
  SDRKIT_SESSION_SPECTRUM_SIZE_65536 = 8,
  // 131072
  SDRKIT_SESSION_SPECTRUM_SIZE_131072 = 9,
  // 262144
  SDRKIT_SESSION_SPECTRUM_SIZE_262144 = 10,
} sdrkit_session_spectrum_size;

// The options of the setting "spectrum_window" of the stack "session".
typedef enum sdrkit_session_spectrum_window {
  // Blackman-Harris
  SDRKIT_SESSION_SPECTRUM_WINDOW_BLACKMAN_HARRIS = 0,
  // Hann
  SDRKIT_SESSION_SPECTRUM_WINDOW_HANN = 1,
  // Rectangular
  SDRKIT_SESSION_SPECTRUM_WINDOW_RECTANGULAR = 2,
} sdrkit_session_spectrum_window;

// The options of `state` of `sdrkit_session_fields`.
typedef enum sdrkit_session_fields_state {
  // Waiting
  SDRKIT_SESSION_FIELDS_STATE_WAITING = 0,
  // Running
  SDRKIT_SESSION_FIELDS_STATE_RUNNING = 1,
  // Failed
  SDRKIT_SESSION_FIELDS_STATE_FAILED = 2,
  // Stopped
  SDRKIT_SESSION_FIELDS_STATE_STOPPED = 3,
} sdrkit_session_fields_state;

// The state and the counts of a session.
typedef struct sdrkit_session_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Where the session is. A value of `sdrkit_session_fields_state`.
  int32_t state;
  // The samples that the receiver was fed.
  int64_t samples;
  // The samples that the session lost: those that its queue or a stage could not take, and those missing in gaps that its driver marked. The column `lost` of the table of connections shows the places.
  int64_t dropped;
  // The events of a type that the catalog of the engine does not list. They are dropped.
  int64_t events_unlisted;
  // True if `problem` holds a value.
  bool has_problem;
  // Why the receiver refused its input, or the last setting that it refused.
  sdrkit_string problem;
  // True if `request_problem` holds a value.
  bool has_request_problem;
  // Why the engine refused the last request of the receiver.
  sdrkit_string request_problem;
  // The audio samples lost because the host did not take them in time.
  int64_t audio_lost;
  // The transmit samples lost because the host did not take them in time.
  int64_t transmit_lost;
  // The changes of frequency that the receiver has taken.
  int64_t retunes;
  // True if `tx_sent` holds a value.
  bool has_tx_sent;
  // The bursts that the radio of the session sent.
  int64_t tx_sent;
  // True if `tx_failed` holds a value.
  bool has_tx_failed;
  // The bursts that were refused or did not reach the radio in full.
  int64_t tx_failed;
  // True if `tx_last` holds a value.
  bool has_tx_last;
  // When the last burst ended. Seconds since 1970 (UTC).
  double tx_last;
  // True if `tx_blocked` holds a value.
  bool has_tx_blocked;
  // What stops the session from transmitting now.
  sdrkit_string tx_blocked;
  // True if `snr_db` holds a value.
  bool has_snr_db;
  // The signal to noise ratio that the receiver measures. Unit: dB.
  double snr_db;
  // True if `level_dbfs` holds a value.
  bool has_level_dbfs;
  // The level that the receiver measures. Unit: dBFS.
  double level_dbfs;
  // True if `offset_hz` holds a value.
  bool has_offset_hz;
  // The offset of the signal from the tuned frequency that the receiver measures. A frequency in Hz.
  double offset_hz;
  // True if `share` holds a value.
  bool has_share;
  // The share of one core that the stack takes now, all its threads together.
  double share;
  // True if `cores_needed` holds a value.
  bool has_cores_needed;
  // The cores that the stack needs at its rate, measured over the last second.
  double cores_needed;
  // True if `cores_declared` holds a value.
  bool has_cores_declared;
  // The cores of this device that the recorded cost of the stack comes to.
  double cores_declared;
  // True if `cores_free` holds a value.
  bool has_cores_free;
  // The cores that were free for the session when it started.
  double cores_free;
  // True if `threads` holds a value.
  bool has_threads;
  // The threads that run the stack.
  int64_t threads;
  // True if `stage_behind` holds a value.
  bool has_stage_behind;
  // The times that a stage found the queue after it full and waited.
  int64_t stage_behind;
  // True if `gaps` holds a value.
  bool has_gaps;
  // The places where samples are missing that the stack was told of.
  int64_t gaps;
  // True if `load_problem` holds a value.
  bool has_load_problem;
  // Why the session may lose samples: its stack needs more cores than are free.
  sdrkit_string load_problem;
  // True if `core_units` holds a value.
  bool has_core_units;
  // The calibration units per second that one fast core of this device runs.
  double core_units;
  // The media packets and pictures lost because the host did not take them in time. Decoded video frames are kept for half a second.
  int64_t media_lost;
  // The samples of the audio that the host gave to the session and that the graph did not take in time: the oldest go first.
  int64_t audio_in_lost;
  // True if `audio_in_rate_hz` holds a value.
  bool has_audio_in_rate_hz;
  // The rate of the audio that the graph of the session takes from the host without conversion, such as a microphone. It has a value while the graph has a host audio input and the session can transmit. A frequency in Hz.
  double audio_in_rate_hz;
  // True while a transmission that sends the host audio runs or push-to-talk is pressed: the host opens its microphone and gives the audio only then.
  bool audio_in_wanted;
} sdrkit_session_fields;

// A row of the table "session.components" (Components).
typedef struct sdrkit_session_component_load_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // Section.
  sdrkit_string section;
  // Stage.
  sdrkit_string stage;
  // Thread.
  int64_t thread;
  // Share.
  double share;
  // Share since start.
  double share_total;
  // Calls.
  int64_t calls;
  // Items in.
  int64_t items_in;
  // Items out.
  int64_t items_out;
  // Mean call. Unit: us.
  double mean_us;
  // Worst call. Unit: us.
  double worst_us;
  // Units.
  double units;
} sdrkit_session_component_load_row;

// A row of the table "session.stages" (Stages).
typedef struct sdrkit_session_stage_load_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // Thread.
  int64_t thread;
  // Own thread.
  bool cut;
  // Share.
  double share;
  // Cost.
  double cost;
  // True if `declared` holds a value.
  bool has_declared;
  // Declared cost.
  double declared;
  // Queue fill.
  double queue_fill;
  // Queue units.
  int64_t queue_units;
  // Queue most.
  int64_t queue_units_max;
  // Behind.
  int64_t behind;
  // Units.
  double units;
  // True if `declared_units` holds a value.
  bool has_declared_units;
  // Declared units.
  double declared_units;
} sdrkit_session_stage_load_row;

// A row of the table "session.connections" (Connections).
typedef struct sdrkit_session_connection_load_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // From.
  sdrkit_string from;
  // Output.
  sdrkit_string output;
  // To.
  sdrkit_string to;
  // Input.
  sdrkit_string input;
  // Format.
  sdrkit_string format;
  // Stage.
  sdrkit_string stage;
  // Items. Unit: 1/s.
  double items_per_s;
  // Bytes. Unit: B/s.
  double bytes_per_s;
  // Held. Unit: B.
  int64_t held_bytes;
  // Held mean. Unit: B.
  double held_mean_bytes;
  // Held most. Unit: B.
  int64_t held_max_bytes;
  // True if `capacity_bytes` holds a value.
  bool has_capacity_bytes;
  // Capacity. Unit: B.
  int64_t capacity_bytes;
  // True if `fill` holds a value.
  bool has_fill;
  // Fill.
  double fill;
  // True if `wait_mean_us` holds a value.
  bool has_wait_mean_us;
  // Wait. Unit: us.
  double wait_mean_us;
  // True if `wait_worst_us` holds a value.
  bool has_wait_worst_us;
  // Wait worst. Unit: us.
  double wait_worst_us;
  // Found full.
  int64_t full;
  // Found empty.
  int64_t empty;
  // Stage boundary.
  bool boundary;
  // Cut.
  bool cut;
  // Lost.
  int64_t lost;
} sdrkit_session_connection_load_row;

// The options of `family` of `sdrkit_session_transmit_event`.
typedef enum sdrkit_session_transmit_event_family {
  // Aircraft
  SDRKIT_SESSION_TRANSMIT_EVENT_FAMILY_AIRCRAFT = 0,
  // Vessel
  SDRKIT_SESSION_TRANSMIT_EVENT_FAMILY_VESSEL = 1,
  // APRS
  SDRKIT_SESSION_TRANSMIT_EVENT_FAMILY_APRS = 2,
  // Pager
  SDRKIT_SESSION_TRANSMIT_EVENT_FAMILY_PAGER = 3,
  // Radiosonde
  SDRKIT_SESSION_TRANSMIT_EVENT_FAMILY_SONDE = 4,
  // Broadcast
  SDRKIT_SESSION_TRANSMIT_EVENT_FAMILY_BROADCAST = 5,
  // Voice
  SDRKIT_SESSION_TRANSMIT_EVENT_FAMILY_VOICE = 6,
  // Message
  SDRKIT_SESSION_TRANSMIT_EVENT_FAMILY_MESSAGE = 7,
  // Sensor
  SDRKIT_SESSION_TRANSMIT_EVENT_FAMILY_SENSOR = 8,
  // Satellite
  SDRKIT_SESSION_TRANSMIT_EVENT_FAMILY_SATELLITE = 9,
  // Receiver
  SDRKIT_SESSION_TRANSMIT_EVENT_FAMILY_RECEIVER = 10,
  // Alert
  SDRKIT_SESSION_TRANSMIT_EVENT_FAMILY_ALERT = 11,
  // Other
  SDRKIT_SESSION_TRANSMIT_EVENT_FAMILY_OTHER = 12,
} sdrkit_session_transmit_event_family;

// Burst.
typedef struct sdrkit_session_transmit_event {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Seconds since the object started, in the time of the signal.
  double time;
  // The frequency of the signal in Hz.
  double freq_hz;
  // Sent.
  bool sent;
  // Samples.
  int64_t samples;
  // True if `attenuation` holds a value.
  bool has_attenuation;
  // The attenuation of the transmitter during the burst: the negative of `tx_power_db`. Unit: dB.
  double attenuation;
  // Attempts.
  int64_t attempts;
  // True if `refused` holds a value.
  bool has_refused;
  // Refused because.
  sdrkit_string refused;
  // Family. A value of `sdrkit_session_transmit_event_family`.
  int32_t family;
  // True if `tx_power_db` holds a value.
  bool has_tx_power_db;
  // The radio setting `tx_power` during the burst: the output power relative to the full output of the radio. Unit: dB.
  double tx_power_db;
} sdrkit_session_transmit_event;

// Fills `rows` with the changed rows of the table "session.components" of the stack "session" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_session_components_rows(SDRKIT_HANDLE handle,
                                                    sdrkit_session_component_load_row *rows,
                                                    uint32_t capacity,
                                                    sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "session.ComponentLoadRow", "session.components", rows, sizeof(sdrkit_session_component_load_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "session.stages" of the stack "session" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_session_stages_rows(SDRKIT_HANDLE handle,
                                                sdrkit_session_stage_load_row *rows,
                                                uint32_t capacity,
                                                sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "session.StageLoadRow", "session.stages", rows, sizeof(sdrkit_session_stage_load_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "session.connections" of the stack "session" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_session_connections_rows(SDRKIT_HANDLE handle,
                                                     sdrkit_session_connection_load_row *rows,
                                                     uint32_t capacity,
                                                     sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "session.ConnectionLoadRow", "session.connections", rows, sizeof(sdrkit_session_connection_load_row), capacity, info);
}

// Fills `event` with event `n` of the last sdrkit_events_take() on `handle`, an event of the type "session.transmit"
// (Burst). Returns SDRKIT_E_WRONG_TYPE if the event is of another type.
static inline int32_t sdrkit_session_transmit_event_read(SDRKIT_HANDLE handle, uint32_t n, sdrkit_session_transmit_event *event) {
  return sdrkit_typed_event(handle, n, "session.transmit", "session.TransmitEvent", event, sizeof(sdrkit_session_transmit_event));
}

// The setting "tuned_hz" (Frequency) of every session. A frequency in Hz. The frequency that the session listens to. A radio moves its centre when the frequency is outside its samples. The session stays the same and keeps running. From the change until the receiver has taken the frequency, the session shows no result of its stack. Then it shows what the receiver holds: a stack clears the results that belong to one frequency, and a result that it keeps is shown again. The result session.retunes counts the changes that the receiver has taken.
static inline int32_t sdrkit_session_get_tuned_hz(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "tuned_hz", value);
}
static inline int32_t sdrkit_session_set_tuned_hz(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "tuned_hz", value);
}

// The setting "spectrum" (Spectrum) of every session.
static inline int32_t sdrkit_session_get_spectrum(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "spectrum", value);
}
static inline int32_t sdrkit_session_set_spectrum(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "spectrum", value);
}

// The setting "spectrum_size" (Spectrum size) of every session. A value of `sdrkit_session_spectrum_size`.
static inline int32_t sdrkit_session_get_spectrum_size(SDRKIT_HANDLE handle, sdrkit_session_spectrum_size *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "spectrum_size", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_session_spectrum_size)index;
  return code;
}
static inline int32_t sdrkit_session_set_spectrum_size(SDRKIT_HANDLE handle, sdrkit_session_spectrum_size value) {
  return sdrkit_typed_setting_set_enum(handle, "spectrum_size", (int32_t)value);
}

// The setting "spectrum_window" (Spectrum window) of every session. A value of `sdrkit_session_spectrum_window`.
static inline int32_t sdrkit_session_get_spectrum_window(SDRKIT_HANDLE handle, sdrkit_session_spectrum_window *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "spectrum_window", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_session_spectrum_window)index;
  return code;
}
static inline int32_t sdrkit_session_set_spectrum_window(SDRKIT_HANDLE handle, sdrkit_session_spectrum_window value) {
  return sdrkit_typed_setting_set_enum(handle, "spectrum_window", (int32_t)value);
}

// The setting "spectrum_fps" (Spectrum frames) of every session. Unit: 1/s.
static inline int32_t sdrkit_session_get_spectrum_fps(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "spectrum_fps", value);
}
static inline int32_t sdrkit_session_set_spectrum_fps(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "spectrum_fps", value);
}

// The setting "volume" (Volume) of every session.
static inline int32_t sdrkit_session_get_volume(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "volume", value);
}
static inline int32_t sdrkit_session_set_volume(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "volume", value);
}

// The setting "pan" (Pan) of every session.
static inline int32_t sdrkit_session_get_pan(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "pan", value);
}
static inline int32_t sdrkit_session_set_pan(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "pan", value);
}

// The setting "mute" (Mute) of every session.
static inline int32_t sdrkit_session_get_mute(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "mute", value);
}
static inline int32_t sdrkit_session_set_mute(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "mute", value);
}

// The setting "solo" (Solo) of every session.
static inline int32_t sdrkit_session_get_solo(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "solo", value);
}
static inline int32_t sdrkit_session_set_solo(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "solo", value);
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_session_fields_read(SDRKIT_HANDLE handle, sdrkit_session_fields *fields) {
  return sdrkit_typed_fields(handle, "session.SessionFields", fields, sizeof(sdrkit_session_fields));
}

#endif
