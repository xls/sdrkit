/*
 * sdrkit: the typed interface of the settings that every record has, version 1.1. MIT licence.
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

#ifndef SDRKIT_RECORD_H
#define SDRKIT_RECORD_H

#include "../sdrkit.h"

// The version of the family that this header describes.
#define SDRKIT_RECORD_VERSION_MAJOR 1
#define SDRKIT_RECORD_VERSION_MINOR 1

// The options of the setting "audio_container" of the stack "record".
typedef enum sdrkit_record_audio_container {
  // WAV
  SDRKIT_RECORD_AUDIO_CONTAINER_WAV = 0,
  // FLAC
  SDRKIT_RECORD_AUDIO_CONTAINER_FLAC = 1,
} sdrkit_record_audio_container;

// The options of the setting "split" of the stack "record".
typedef enum sdrkit_record_split {
  // Whole
  SDRKIT_RECORD_SPLIT_WHOLE = 0,
  // Each transmission
  SDRKIT_RECORD_SPLIT_TRANSMISSION = 1,
} sdrkit_record_split;

// The options of the setting "gate" of the stack "record".
typedef enum sdrkit_record_gate {
  // Squelch
  SDRKIT_RECORD_GATE_SQUELCH = 0,
  // Level
  SDRKIT_RECORD_GATE_LEVEL = 1,
  // Always
  SDRKIT_RECORD_GATE_ALWAYS = 2,
} sdrkit_record_gate;

// The options of the setting "iq_container" of the stack "record".
typedef enum sdrkit_record_iq_container {
  // SigMF
  SDRKIT_RECORD_IQ_CONTAINER_SIGMF = 0,
  // Raw
  SDRKIT_RECORD_IQ_CONTAINER_RAW = 1,
} sdrkit_record_iq_container;

// The options of the setting "iq_sample" of the stack "record".
typedef enum sdrkit_record_iq_sample {
  // 8-bit
  SDRKIT_RECORD_IQ_SAMPLE_CS8 = 0,
  // 16-bit
  SDRKIT_RECORD_IQ_SAMPLE_CS16 = 1,
  // Float
  SDRKIT_RECORD_IQ_SAMPLE_CF32 = 2,
} sdrkit_record_iq_sample;

// The options of `kind` of `sdrkit_record_running_row`.
typedef enum sdrkit_record_running_row_kind {
  // Audio
  SDRKIT_RECORD_RUNNING_ROW_KIND_AUDIO = 0,
  // IQ
  SDRKIT_RECORD_RUNNING_ROW_KIND_IQ = 1,
  // Transport stream
  SDRKIT_RECORD_RUNNING_ROW_KIND_TS = 2,
} sdrkit_record_running_row_kind;

// The options of `state` of `sdrkit_record_running_row`.
typedef enum sdrkit_record_running_row_state {
  // Recording
  SDRKIT_RECORD_RUNNING_ROW_STATE_RECORDING = 0,
  // Waiting
  SDRKIT_RECORD_RUNNING_ROW_STATE_WAITING = 1,
  // Ending
  SDRKIT_RECORD_RUNNING_ROW_STATE_ENDING = 2,
  // Failed
  SDRKIT_RECORD_RUNNING_ROW_STATE_FAILED = 3,
} sdrkit_record_running_row_state;

// The options of `problem` of `sdrkit_record_running_row`.
typedef enum sdrkit_record_running_row_problem {
  // None
  SDRKIT_RECORD_RUNNING_ROW_PROBLEM_NONE = 0,
  // No space
  SDRKIT_RECORD_RUNNING_ROW_PROBLEM_NO_SPACE = 1,
  // Not found
  SDRKIT_RECORD_RUNNING_ROW_PROBLEM_NOT_FOUND = 2,
  // Denied
  SDRKIT_RECORD_RUNNING_ROW_PROBLEM_DENIED = 3,
  // Already exists
  SDRKIT_RECORD_RUNNING_ROW_PROBLEM_ALREADY_EXISTS = 4,
  // Invalid path
  SDRKIT_RECORD_RUNNING_ROW_PROBLEM_INVALID_PATH = 5,
  // Input or output error
  SDRKIT_RECORD_RUNNING_ROW_PROBLEM_IO = 6,
} sdrkit_record_running_row_problem;

// A row of the table "running" (Running recordings).
typedef struct sdrkit_record_running_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // Kind. A value of `sdrkit_record_running_row_kind`.
  int32_t kind;
  // Session.
  sdrkit_string session;
  // State. A value of `sdrkit_record_running_row_state`.
  int32_t state;
  // True if `file` holds a value.
  bool has_file;
  // File.
  sdrkit_string file;
  // Files kept.
  int64_t files;
  // Written.
  int64_t frames;
  // Left out.
  int64_t dropped;
  // Size. Unit: B.
  int64_t bytes;
  // Problem. A value of `sdrkit_record_running_row_problem`.
  int32_t problem;
  // Started. Seconds since 1970 (UTC).
  double started;
} sdrkit_record_running_row;

// The options of `kind` of `sdrkit_record_recording_row`.
typedef enum sdrkit_record_recording_row_kind {
  // Audio
  SDRKIT_RECORD_RECORDING_ROW_KIND_AUDIO = 0,
  // IQ
  SDRKIT_RECORD_RECORDING_ROW_KIND_IQ = 1,
  // Transport stream
  SDRKIT_RECORD_RECORDING_ROW_KIND_TS = 2,
} sdrkit_record_recording_row_kind;

// The options of `container` of `sdrkit_record_recording_row`.
typedef enum sdrkit_record_recording_row_container {
  // WAV
  SDRKIT_RECORD_RECORDING_ROW_CONTAINER_WAV = 0,
  // FLAC
  SDRKIT_RECORD_RECORDING_ROW_CONTAINER_FLAC = 1,
  // SigMF
  SDRKIT_RECORD_RECORDING_ROW_CONTAINER_SIGMF = 2,
  // Raw
  SDRKIT_RECORD_RECORDING_ROW_CONTAINER_RAW = 3,
  // Transport stream
  SDRKIT_RECORD_RECORDING_ROW_CONTAINER_TS = 4,
} sdrkit_record_recording_row_container;

// A row of the table "recordings" (Recordings).
typedef struct sdrkit_record_recording_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // Kind. A value of `sdrkit_record_recording_row_kind`.
  int32_t kind;
  // Container. A value of `sdrkit_record_recording_row_container`.
  int32_t container;
  // Size. Unit: B.
  int64_t bytes;
  // True if `seconds` holds a value.
  bool has_seconds;
  // Length. A length of time in seconds.
  double seconds;
  // True if `rate_hz` holds a value.
  bool has_rate_hz;
  // Rate. A frequency in Hz.
  double rate_hz;
  // True if `centre_hz` holds a value.
  bool has_centre_hz;
  // Centre. A frequency in Hz.
  double centre_hz;
  // Channels.
  int64_t channels;
  // True if `modified` holds a value.
  bool has_modified;
  // Modified. Seconds since 1970 (UTC).
  double modified;
} sdrkit_record_recording_row;

// The reply of `audio.start`.
typedef struct sdrkit_record_audio_start_reply {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Recording.
  sdrkit_string recording;
} sdrkit_record_audio_start_reply;

// The reply of `iq.start`.
typedef struct sdrkit_record_iq_start_reply {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Recording.
  sdrkit_string recording;
} sdrkit_record_iq_start_reply;

// The reply of `ts.start`.
typedef struct sdrkit_record_ts_start_reply {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Recording.
  sdrkit_string recording;
} sdrkit_record_ts_start_reply;

// The arguments of `audio.start`.
typedef struct sdrkit_record_audio_start_args {
  // The size of this struct: sizeof of the type. The caller sets it.
  uint32_t size;
  // Session.
  sdrkit_string session;
  // True if `label` holds a value.
  bool has_label;
  // A word added to the file names.
  sdrkit_string label;
  // True if `seconds` holds a value.
  bool has_seconds;
  // The recording ends after this. 0 records until it is stopped. A length of time in seconds.
  double seconds;
} sdrkit_record_audio_start_args;

// An arguments struct with its size set and every argument at 0.
#define SDRKIT_RECORD_AUDIO_START_ARGS_INIT {sizeof(sdrkit_record_audio_start_args), {0}, 0, {0}, 0, 0}

// The arguments of `audio.stop`.
typedef struct sdrkit_record_audio_stop_args {
  // The size of this struct: sizeof of the type. The caller sets it.
  uint32_t size;
  // The row key, as the table shows it.
  sdrkit_string row;
} sdrkit_record_audio_stop_args;

// An arguments struct with its size set and every argument at 0.
#define SDRKIT_RECORD_AUDIO_STOP_ARGS_INIT {sizeof(sdrkit_record_audio_stop_args), {0}}

// The arguments of `iq.start`.
typedef struct sdrkit_record_iq_start_args {
  // The size of this struct: sizeof of the type. The caller sets it.
  uint32_t size;
  // Session.
  sdrkit_string session;
  // True if `seconds` holds a value.
  bool has_seconds;
  // The recording ends after this. 0 records until it is stopped. A length of time in seconds.
  double seconds;
} sdrkit_record_iq_start_args;

// An arguments struct with its size set and every argument at 0.
#define SDRKIT_RECORD_IQ_START_ARGS_INIT {sizeof(sdrkit_record_iq_start_args), {0}, 0, 0}

// The arguments of `iq.stop`.
typedef struct sdrkit_record_iq_stop_args {
  // The size of this struct: sizeof of the type. The caller sets it.
  uint32_t size;
  // The row key, as the table shows it.
  sdrkit_string row;
} sdrkit_record_iq_stop_args;

// An arguments struct with its size set and every argument at 0.
#define SDRKIT_RECORD_IQ_STOP_ARGS_INIT {sizeof(sdrkit_record_iq_stop_args), {0}}

// The arguments of `ts.start`.
typedef struct sdrkit_record_ts_start_args {
  // The size of this struct: sizeof of the type. The caller sets it.
  uint32_t size;
  // Session.
  sdrkit_string session;
  // True if `services` holds a value.
  bool has_services;
  // Service ids separated by commas, one file each. Empty records the multiplex.
  sdrkit_string services;
} sdrkit_record_ts_start_args;

// An arguments struct with its size set and every argument at 0.
#define SDRKIT_RECORD_TS_START_ARGS_INIT {sizeof(sdrkit_record_ts_start_args), {0}, 0, {0}}

// The arguments of `ts.stop`.
typedef struct sdrkit_record_ts_stop_args {
  // The size of this struct: sizeof of the type. The caller sets it.
  uint32_t size;
  // The row key, as the table shows it.
  sdrkit_string row;
} sdrkit_record_ts_stop_args;

// An arguments struct with its size set and every argument at 0.
#define SDRKIT_RECORD_TS_STOP_ARGS_INIT {sizeof(sdrkit_record_ts_stop_args), {0}}

// The arguments of `delete`.
typedef struct sdrkit_record_delete_args {
  // The size of this struct: sizeof of the type. The caller sets it.
  uint32_t size;
  // The row key, as the table shows it.
  sdrkit_string row;
} sdrkit_record_delete_args;

// An arguments struct with its size set and every argument at 0.
#define SDRKIT_RECORD_DELETE_ARGS_INIT {sizeof(sdrkit_record_delete_args), {0}}

// Fills `rows` with the changed rows of the table "running" of the stack "record" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_record_running_rows(SDRKIT_HANDLE handle,
                                                sdrkit_record_running_row *rows,
                                                uint32_t capacity,
                                                sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "record.RunningRow", "running", rows, sizeof(sdrkit_record_running_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "recordings" of the stack "record" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_record_recordings_rows(SDRKIT_HANDLE handle,
                                                   sdrkit_record_recording_row *rows,
                                                   uint32_t capacity,
                                                   sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "record.RecordingRow", "recordings", rows, sizeof(sdrkit_record_recording_row), capacity, info);
}

// Runs the command "audio.start" (Record audio) of the stack "record" on the session of `handle`. Records the audio of the session with the audio settings.
static inline int32_t sdrkit_record_audio_start(SDRKIT_HANDLE handle, const sdrkit_record_audio_start_args *args, sdrkit_record_audio_start_reply *reply) {
  return sdrkit_typed_command(handle, "audio.start", "record.AudioStartArgs", args, "record.AudioStartReply", reply, sizeof(sdrkit_record_audio_start_reply));
}

// Runs the command "audio.stop" (Stop audio recording) of the stack "record" on the session of `handle`.
static inline int32_t sdrkit_record_audio_stop(SDRKIT_HANDLE handle, const sdrkit_record_audio_stop_args *args) {
  return sdrkit_typed_command(handle, "audio.stop", "record.AudioStopArgs", args, NULL, NULL, 0);
}

// Runs the command "iq.start" (Record IQ) of the stack "record" on the session of `handle`. Records the samples of the session with the IQ settings.
static inline int32_t sdrkit_record_iq_start(SDRKIT_HANDLE handle, const sdrkit_record_iq_start_args *args, sdrkit_record_iq_start_reply *reply) {
  return sdrkit_typed_command(handle, "iq.start", "record.IqStartArgs", args, "record.IqStartReply", reply, sizeof(sdrkit_record_iq_start_reply));
}

// Runs the command "iq.stop" (Stop IQ recording) of the stack "record" on the session of `handle`.
static inline int32_t sdrkit_record_iq_stop(SDRKIT_HANDLE handle, const sdrkit_record_iq_stop_args *args) {
  return sdrkit_typed_command(handle, "iq.stop", "record.IqStopArgs", args, NULL, NULL, 0);
}

// Runs the command "ts.start" (Record stream) of the stack "record" on the session of `handle`. Records the transport stream of the session: the multiplex, or one file per service.
static inline int32_t sdrkit_record_ts_start(SDRKIT_HANDLE handle, const sdrkit_record_ts_start_args *args, sdrkit_record_ts_start_reply *reply) {
  return sdrkit_typed_command(handle, "ts.start", "record.TsStartArgs", args, "record.TsStartReply", reply, sizeof(sdrkit_record_ts_start_reply));
}

// Runs the command "ts.stop" (Stop stream recording) of the stack "record" on the session of `handle`. Stops a stream recording or the time-shift buffer.
static inline int32_t sdrkit_record_ts_stop(SDRKIT_HANDLE handle, const sdrkit_record_ts_stop_args *args) {
  return sdrkit_typed_command(handle, "ts.stop", "record.TsStopArgs", args, NULL, NULL, 0);
}

// Runs the command "delete" (Delete recording) of the stack "record" on the session of `handle`. Deletes a recording file and the metadata beside it.
static inline int32_t sdrkit_record_delete(SDRKIT_HANDLE handle, const sdrkit_record_delete_args *args) {
  return sdrkit_typed_command(handle, "delete", "record.DeleteArgs", args, NULL, NULL, 0);
}

// The setting "folder" (Folder) of every record. The folder of the recordings in the storage of the host, as a relative path.
static inline int32_t sdrkit_record_get_folder(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "folder", value);
}
static inline int32_t sdrkit_record_set_folder(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "folder", value);
}

// The setting "prefix" (Name prefix) of every record.
static inline int32_t sdrkit_record_get_prefix(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "prefix", value);
}
static inline int32_t sdrkit_record_set_prefix(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "prefix", value);
}

// The setting "utc_offset_min" (Local time offset) of every record. Unit: min. The offset of local time from UTC, for file names and schedules.
static inline int32_t sdrkit_record_get_utc_offset_min(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "utc_offset_min", value);
}
static inline int32_t sdrkit_record_set_utc_offset_min(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "utc_offset_min", value);
}

// The setting "reserve_bytes" (Reserve) of every record. Unit: B. Bytes that stay free. A recording that would pass it does not start, and one that runs is cut.
static inline int32_t sdrkit_record_get_reserve_bytes(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "reserve_bytes", value);
}
static inline int32_t sdrkit_record_set_reserve_bytes(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "reserve_bytes", value);
}

// The setting "budget_bytes" (Folder budget) of every record. Unit: B. The most bytes that the recordings hold. The oldest go first. 0 sets no budget.
static inline int32_t sdrkit_record_get_budget_bytes(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "budget_bytes", value);
}
static inline int32_t sdrkit_record_set_budget_bytes(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "budget_bytes", value);
}

// The setting "queue_bytes" (Write queue) of every record. Unit: B. Bytes that can wait for storage. When it is full, an IQ recording waits for storage and keeps every sample; an audio or transport stream recording leaves data out and counts it.
static inline int32_t sdrkit_record_get_queue_bytes(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "queue_bytes", value);
}
static inline int32_t sdrkit_record_set_queue_bytes(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "queue_bytes", value);
}

// The setting "audio_container" (Audio file) of every record. A value of `sdrkit_record_audio_container`.
static inline int32_t sdrkit_record_get_audio_container(SDRKIT_HANDLE handle, sdrkit_record_audio_container *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "audio_container", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_record_audio_container)index;
  return code;
}
static inline int32_t sdrkit_record_set_audio_container(SDRKIT_HANDLE handle, sdrkit_record_audio_container value) {
  return sdrkit_typed_setting_set_enum(handle, "audio_container", (int32_t)value);
}

// The setting "mono" (Mono) of every record. Mixes the channels of the audio to one.
static inline int32_t sdrkit_record_get_mono(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "mono", value);
}
static inline int32_t sdrkit_record_set_mono(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "mono", value);
}

// The setting "split" (Files) of every record. A value of `sdrkit_record_split`.
static inline int32_t sdrkit_record_get_split(SDRKIT_HANDLE handle, sdrkit_record_split *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "split", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_record_split)index;
  return code;
}
static inline int32_t sdrkit_record_set_split(SDRKIT_HANDLE handle, sdrkit_record_split value) {
  return sdrkit_typed_setting_set_enum(handle, "split", (int32_t)value);
}

// The setting "gate" (Transmission gate) of every record. A value of `sdrkit_record_gate`.
static inline int32_t sdrkit_record_get_gate(SDRKIT_HANDLE handle, sdrkit_record_gate *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "gate", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_record_gate)index;
  return code;
}
static inline int32_t sdrkit_record_set_gate(SDRKIT_HANDLE handle, sdrkit_record_gate value) {
  return sdrkit_typed_setting_set_enum(handle, "gate", (int32_t)value);
}

// The setting "open_db" (Open level) of every record. Unit: dBFS.
static inline int32_t sdrkit_record_get_open_db(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "open_db", value);
}
static inline int32_t sdrkit_record_set_open_db(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "open_db", value);
}

// The setting "close_db" (Close level) of every record. Unit: dBFS.
static inline int32_t sdrkit_record_get_close_db(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "close_db", value);
}
static inline int32_t sdrkit_record_set_close_db(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "close_db", value);
}

// The setting "preroll" (Pre-roll) of every record. A length of time in seconds. Audio from before a transmission opens.
static inline int32_t sdrkit_record_get_preroll(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "preroll", value);
}
static inline int32_t sdrkit_record_set_preroll(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "preroll", value);
}

// The setting "hang" (Hang) of every record. A length of time in seconds. How long a file stays open after the gate closes. A reopening goes on in the same file.
static inline int32_t sdrkit_record_get_hang(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "hang", value);
}
static inline int32_t sdrkit_record_set_hang(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "hang", value);
}

// The setting "min_length" (Shortest kept) of every record. A length of time in seconds. A transmission shorter than this, without pre-roll and hang, is deleted.
static inline int32_t sdrkit_record_get_min_length(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "min_length", value);
}
static inline int32_t sdrkit_record_set_min_length(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "min_length", value);
}

// The setting "max_file" (Longest file) of every record. A length of time in seconds. A longer file goes on in a next part. 0 sets no limit.
static inline int32_t sdrkit_record_get_max_file(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "max_file", value);
}
static inline int32_t sdrkit_record_set_max_file(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "max_file", value);
}

// The setting "log" (Log of kept files) of every record. Adds a line for each kept audio file to the CSV file <prefix>.csv in the folder.
static inline int32_t sdrkit_record_get_log(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "log", value);
}
static inline int32_t sdrkit_record_set_log(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "log", value);
}

// The setting "title" (Title tag) of every record.
static inline int32_t sdrkit_record_get_title(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "title", value);
}
static inline int32_t sdrkit_record_set_title(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "title", value);
}

// The setting "comment" (Comment tag) of every record.
static inline int32_t sdrkit_record_get_comment(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "comment", value);
}
static inline int32_t sdrkit_record_set_comment(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "comment", value);
}

// The setting "channel_iq" (Channel IQ) of every record. Writes the channel IQ beside each audio file.
static inline int32_t sdrkit_record_get_channel_iq(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "channel_iq", value);
}
static inline int32_t sdrkit_record_set_channel_iq(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "channel_iq", value);
}

// The setting "iq_container" (IQ file) of every record. A value of `sdrkit_record_iq_container`.
static inline int32_t sdrkit_record_get_iq_container(SDRKIT_HANDLE handle, sdrkit_record_iq_container *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "iq_container", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_record_iq_container)index;
  return code;
}
static inline int32_t sdrkit_record_set_iq_container(SDRKIT_HANDLE handle, sdrkit_record_iq_container value) {
  return sdrkit_typed_setting_set_enum(handle, "iq_container", (int32_t)value);
}

// The setting "iq_sample" (IQ samples) of every record. A value of `sdrkit_record_iq_sample`.
static inline int32_t sdrkit_record_get_iq_sample(SDRKIT_HANDLE handle, sdrkit_record_iq_sample *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "iq_sample", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_record_iq_sample)index;
  return code;
}
static inline int32_t sdrkit_record_set_iq_sample(SDRKIT_HANDLE handle, sdrkit_record_iq_sample value) {
  return sdrkit_typed_setting_set_enum(handle, "iq_sample", (int32_t)value);
}

// The setting "ts_drop_null" (Leave out null packets) of every record.
static inline int32_t sdrkit_record_get_ts_drop_null(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "ts_drop_null", value);
}
static inline int32_t sdrkit_record_set_ts_drop_null(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "ts_drop_null", value);
}

// The setting "ts_max_bytes" (Largest stream file) of every record. Unit: B. A larger file goes on in a next part. 0 sets no limit.
static inline int32_t sdrkit_record_get_ts_max_bytes(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "ts_max_bytes", value);
}
static inline int32_t sdrkit_record_set_ts_max_bytes(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "ts_max_bytes", value);
}

// The setting "ts_sidecar" (Stream sidecar) of every record. Writes a JSON file beside each stream file with its service, part and time.
static inline int32_t sdrkit_record_get_ts_sidecar(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "ts_sidecar", value);
}
static inline int32_t sdrkit_record_set_ts_sidecar(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "ts_sidecar", value);
}

// The setting "rewind" (Rewind buffer) of every record. Keeps the last seconds of the radio's samples in memory, for a save.
static inline int32_t sdrkit_record_get_rewind(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "rewind", value);
}
static inline int32_t sdrkit_record_set_rewind(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "rewind", value);
}

// The setting "rewind_bytes" (Rewind memory) of every record. Unit: B.
static inline int32_t sdrkit_record_get_rewind_bytes(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "rewind_bytes", value);
}
static inline int32_t sdrkit_record_set_rewind_bytes(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "rewind_bytes", value);
}

// The setting "rewind_long_factor" (Centre history factor) of every record. Averages every this many samples into a longer history of the centre of the band.
static inline int32_t sdrkit_record_get_rewind_long_factor(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "rewind_long_factor", value);
}
static inline int32_t sdrkit_record_set_rewind_long_factor(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "rewind_long_factor", value);
}

// The setting "rewind_long_bytes" (Centre history memory) of every record. Unit: B.
static inline int32_t sdrkit_record_get_rewind_long_bytes(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "rewind_long_bytes", value);
}
static inline int32_t sdrkit_record_set_rewind_long_bytes(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "rewind_long_bytes", value);
}

// The setting "timeshift_bytes" (Time-shift size) of every record. Unit: B.
static inline int32_t sdrkit_record_get_timeshift_bytes(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "timeshift_bytes", value);
}
static inline int32_t sdrkit_record_set_timeshift_bytes(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "timeshift_bytes", value);
}

// The setting "timeshift_segment_bytes" (Time-shift segment) of every record. Unit: B.
static inline int32_t sdrkit_record_get_timeshift_segment_bytes(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "timeshift_segment_bytes", value);
}
static inline int32_t sdrkit_record_set_timeshift_segment_bytes(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "timeshift_segment_bytes", value);
}

#endif
