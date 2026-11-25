/*
 * sdrkit: the typed interface of the settings that every scan has, version 3.0. MIT licence.
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

#ifndef SDRKIT_SCAN_H
#define SDRKIT_SCAN_H

#include "../sdrkit.h"

// The version of the family that this header describes.
#define SDRKIT_SCAN_VERSION_MAJOR 3
#define SDRKIT_SCAN_VERSION_MINOR 0

// The options of the setting "stop_on" of the stack "scan".
typedef enum sdrkit_scan_stop_on {
  // Squelch open
  SDRKIT_SCAN_STOP_ON_SQUELCH = 0,
  // SNR
  SDRKIT_SCAN_STOP_ON_SNR = 1,
  // Level
  SDRKIT_SCAN_STOP_ON_LEVEL = 2,
  // Lock
  SDRKIT_SCAN_STOP_ON_LOCK = 3,
  // Squelch or lock
  SDRKIT_SCAN_STOP_ON_SQUELCH_OR_LOCK = 4,
} sdrkit_scan_stop_on;

// The options of the setting "after_find" of the stack "scan".
typedef enum sdrkit_scan_after_find {
  // Stay
  SDRKIT_SCAN_AFTER_FIND_STAY = 0,
  // Resume when quiet
  SDRKIT_SCAN_AFTER_FIND_RESUME = 1,
} sdrkit_scan_after_find;

// The options of `state` of `sdrkit_scan_fields`.
typedef enum sdrkit_scan_fields_state {
  // Idle
  SDRKIT_SCAN_FIELDS_STATE_IDLE = 0,
  // Settle
  SDRKIT_SCAN_FIELDS_STATE_SETTLE = 1,
  // Listen
  SDRKIT_SCAN_FIELDS_STATE_LISTEN = 2,
  // Hold
  SDRKIT_SCAN_FIELDS_STATE_HOLD = 3,
  // Hang
  SDRKIT_SCAN_FIELDS_STATE_HANG = 4,
  // Paused
  SDRKIT_SCAN_FIELDS_STATE_PAUSED = 5,
  // Test
  SDRKIT_SCAN_FIELDS_STATE_TEST = 6,
  // Done
  SDRKIT_SCAN_FIELDS_STATE_DONE = 7,
  // Locked
  SDRKIT_SCAN_FIELDS_STATE_LOCKED = 8,
  // Look
  SDRKIT_SCAN_FIELDS_STATE_LOOK = 9,
} sdrkit_scan_fields_state;

// The options of `kind` of `sdrkit_scan_fields`.
typedef enum sdrkit_scan_fields_kind {
  // None
  SDRKIT_SCAN_FIELDS_KIND_NONE = 0,
  // Activity
  SDRKIT_SCAN_FIELDS_KIND_ACTIVITY = 1,
  // Channels
  SDRKIT_SCAN_FIELDS_KIND_CHANNELS = 2,
  // Span blocks
  SDRKIT_SCAN_FIELDS_KIND_SPAN = 3,
  // Survey
  SDRKIT_SCAN_FIELDS_KIND_SURVEY = 4,
} sdrkit_scan_fields_kind;

// The options of `end` of `sdrkit_scan_fields`.
typedef enum sdrkit_scan_fields_end {
  // Not ended
  SDRKIT_SCAN_FIELDS_END_NONE = 0,
  // Every pass done
  SDRKIT_SCAN_FIELDS_END_PASSES_DONE = 1,
  // Every channel locked out
  SDRKIT_SCAN_FIELDS_END_ALL_LOCKED_OUT = 2,
  // Nothing to scan
  SDRKIT_SCAN_FIELDS_END_NOTHING_TO_SCAN = 3,
  // Stopped
  SDRKIT_SCAN_FIELDS_END_STOPPED = 4,
  // Complete
  SDRKIT_SCAN_FIELDS_END_COMPLETE = 5,
  // Total time over
  SDRKIT_SCAN_FIELDS_END_TIME_OVER = 6,
} sdrkit_scan_fields_end;

// The state and the progress of the scan service.
typedef struct sdrkit_scan_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Where the scan is. A value of `sdrkit_scan_fields_state`.
  int32_t state;
  // The kind of the scan that runs or ran last. A value of `sdrkit_scan_fields_kind`.
  int32_t kind;
  // True if `index` holds a value.
  bool has_index;
  // The place of the channel in the list of the scan.
  int64_t index;
  // True if `count` holds a value.
  bool has_count;
  // The channels of the scan.
  int64_t count;
  // True if `freq_hz` holds a value.
  bool has_freq_hz;
  // The frequency of the channel. A frequency in Hz.
  double freq_hz;
  // True if `name` holds a value.
  bool has_name;
  // The name of the channel.
  sdrkit_string name;
  // True if `stack` holds a value.
  bool has_stack;
  // The stack of the channel.
  sdrkit_string stack;
  // True if `at_priority` holds a value.
  bool has_at_priority;
  // The scan listens to the priority channel.
  bool at_priority;
  // True if `pass` holds a value.
  bool has_pass;
  // The pass over the list.
  int64_t pass;
  // True if `steps` holds a value.
  bool has_steps;
  // The channels that the scan has visited.
  int64_t steps;
  // True if `found` holds a value.
  bool has_found;
  // The channels with a signal so far.
  int64_t found;
  // Why the scan ended. A value of `sdrkit_scan_fields_end`.
  int32_t end;
  // True if `session` holds a value.
  bool has_session;
  // The session that the scan tunes.
  sdrkit_string session;
  // True if `total` holds a value.
  bool has_total;
  // The rows of the scan table.
  int64_t total;
  // True if `time_left_s` holds a value.
  bool has_time_left_s;
  // The time until the total time of a span or survey scan is over. A length of time in seconds.
  double time_left_s;
  // True if `pass_s` holds a value.
  bool has_pass_s;
  // The time that one pass of the scan takes, from its plan. A length of time in seconds.
  double pass_s;
  // True if `passes` holds a value.
  bool has_passes;
  // The passes that fit into the total time, from the plan of the scan.
  double passes;
  // True if `candidates` holds a value.
  bool has_candidates;
  // The candidates of a survey that wait for a visit.
  int64_t candidates;
  // True if `watched` holds a value.
  bool has_watched;
  // The value of the watched result of the stack on the block that is held.
  double watched;
} sdrkit_scan_fields;

// The options of `empty` of `sdrkit_scan_hit_row`.
typedef enum sdrkit_scan_hit_row_empty {
  // No signal
  SDRKIT_SCAN_HIT_ROW_EMPTY_NO_SIGNAL = 0,
  // No lock
  SDRKIT_SCAN_HIT_ROW_EMPTY_NO_LOCK = 1,
} sdrkit_scan_hit_row_empty;

// A row of the table "hits" (Scan hits).
typedef struct sdrkit_scan_hit_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // Frequency. A frequency in Hz.
  double freq_hz;
  // Name.
  sdrkit_string name;
  // Stack.
  sdrkit_string stack;
  // Locked.
  bool locked;
  // True if `snr_db` holds a value.
  bool has_snr_db;
  // SNR. Unit: dB.
  double snr_db;
  // True if `level_dbfs` holds a value.
  bool has_level_dbfs;
  // Level. Unit: dBFS.
  double level_dbfs;
  // Start. Seconds since 1970 (UTC).
  double start;
  // End. Seconds since 1970 (UTC).
  double end_time;
  // True if `priority` holds a value.
  bool has_priority;
  // Priority.
  bool priority;
  // True if `tone` holds a value.
  bool has_tone;
  // Tone.
  sdrkit_string tone;
  // True if `memory` holds a value.
  bool has_memory;
  // Memory.
  sdrkit_string memory;
  // True if `channel` holds a value.
  bool has_channel;
  // Channel.
  sdrkit_string channel;
  // True if `empty` holds a value.
  bool has_empty;
  // Empty because. A value of `sdrkit_scan_hit_row_empty`.
  int32_t empty;
  // Pass.
  int64_t pass;
  // Text.
  sdrkit_string text;
} sdrkit_scan_hit_row;

// The options of `state` of `sdrkit_scan_candidate_row`.
typedef enum sdrkit_scan_candidate_row_state {
  // Waiting
  SDRKIT_SCAN_CANDIDATE_ROW_STATE_WAITING = 0,
  // Visiting
  SDRKIT_SCAN_CANDIDATE_ROW_STATE_VISITING = 1,
  // Found
  SDRKIT_SCAN_CANDIDATE_ROW_STATE_FOUND = 2,
  // Rejected
  SDRKIT_SCAN_CANDIDATE_ROW_STATE_REJECTED = 3,
} sdrkit_scan_candidate_row_state;

// A row of the table "candidates" (Survey candidates).
typedef struct sdrkit_scan_candidate_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // Frequency. A frequency in Hz.
  double freq_hz;
  // Score.
  double score;
  // Speed. Unit: wpm.
  double wpm;
  // SNR. Unit: dB.
  double snr_db;
  // State. A value of `sdrkit_scan_candidate_row_state`.
  int32_t state;
  // Pass.
  int64_t pass;
  // First seen. Seconds since 1970 (UTC).
  double first_seen;
  // True if `skip_until` holds a value.
  bool has_skip_until;
  // Skipped until. Seconds since 1970 (UTC).
  double skip_until;
} sdrkit_scan_candidate_row;

// A row of the table "occupied" (Occupied channels of a survey, 2 to 30 kHz wide; a lone carrier is not listed).
typedef struct sdrkit_scan_occupied_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // Frequency. A frequency in Hz.
  double freq_hz;
  // Width. A frequency in Hz.
  double width_hz;
  // SNR. Unit: dB.
  double snr_db;
  // Pass.
  int64_t pass;
  // Seen.
  int64_t seen;
} sdrkit_scan_occupied_row;

// The options of `kind` of `sdrkit_scan_start_reply`.
typedef enum sdrkit_scan_start_reply_kind {
  // None
  SDRKIT_SCAN_START_REPLY_KIND_NONE = 0,
  // Activity
  SDRKIT_SCAN_START_REPLY_KIND_ACTIVITY = 1,
  // Channels
  SDRKIT_SCAN_START_REPLY_KIND_CHANNELS = 2,
  // Span blocks
  SDRKIT_SCAN_START_REPLY_KIND_SPAN = 3,
  // Survey
  SDRKIT_SCAN_START_REPLY_KIND_SURVEY = 4,
} sdrkit_scan_start_reply_kind;

// The reply of `start`.
typedef struct sdrkit_scan_start_reply {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Channels.
  int64_t channels;
  // Kind. A value of `sdrkit_scan_start_reply_kind`.
  int32_t kind;
  // Dwell. A length of time in seconds.
  double dwell_s;
  // Time of a pass. A length of time in seconds.
  double pass_s;
  // Total time. A length of time in seconds.
  double total_s;
  // Passes.
  double passes;
  // Time of a visit. A length of time in seconds.
  double visit_s;
  // Time of a pass of span blocks. A length of time in seconds.
  double span_pass_s;
} sdrkit_scan_start_reply;

// The options of `kind` of `sdrkit_scan_span_reply`.
typedef enum sdrkit_scan_span_reply_kind {
  // None
  SDRKIT_SCAN_SPAN_REPLY_KIND_NONE = 0,
  // Activity
  SDRKIT_SCAN_SPAN_REPLY_KIND_ACTIVITY = 1,
  // Channels
  SDRKIT_SCAN_SPAN_REPLY_KIND_CHANNELS = 2,
  // Span blocks
  SDRKIT_SCAN_SPAN_REPLY_KIND_SPAN = 3,
  // Survey
  SDRKIT_SCAN_SPAN_REPLY_KIND_SURVEY = 4,
} sdrkit_scan_span_reply_kind;

// The reply of `span`.
typedef struct sdrkit_scan_span_reply {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Channels.
  int64_t channels;
  // Kind. A value of `sdrkit_scan_span_reply_kind`.
  int32_t kind;
  // Dwell. A length of time in seconds.
  double dwell_s;
  // Time of a pass. A length of time in seconds.
  double pass_s;
  // Total time. A length of time in seconds.
  double total_s;
  // Passes.
  double passes;
  // Time of a visit. A length of time in seconds.
  double visit_s;
  // Time of a pass of span blocks. A length of time in seconds.
  double span_pass_s;
} sdrkit_scan_span_reply;

// The options of `kind` of `sdrkit_scan_survey_reply`.
typedef enum sdrkit_scan_survey_reply_kind {
  // None
  SDRKIT_SCAN_SURVEY_REPLY_KIND_NONE = 0,
  // Activity
  SDRKIT_SCAN_SURVEY_REPLY_KIND_ACTIVITY = 1,
  // Channels
  SDRKIT_SCAN_SURVEY_REPLY_KIND_CHANNELS = 2,
  // Span blocks
  SDRKIT_SCAN_SURVEY_REPLY_KIND_SPAN = 3,
  // Survey
  SDRKIT_SCAN_SURVEY_REPLY_KIND_SURVEY = 4,
} sdrkit_scan_survey_reply_kind;

// The reply of `survey`.
typedef struct sdrkit_scan_survey_reply {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Channels.
  int64_t channels;
  // Kind. A value of `sdrkit_scan_survey_reply_kind`.
  int32_t kind;
  // Dwell. A length of time in seconds.
  double dwell_s;
  // Time of a pass. A length of time in seconds.
  double pass_s;
  // Total time. A length of time in seconds.
  double total_s;
  // Passes.
  double passes;
  // Time of a visit. A length of time in seconds.
  double visit_s;
  // Time of a pass of span blocks. A length of time in seconds.
  double span_pass_s;
} sdrkit_scan_survey_reply;

// The arguments of `start`.
typedef struct sdrkit_scan_start_args {
  // The size of this struct: sizeof of the type. The caller sets it.
  uint32_t size;
  // Session.
  sdrkit_string session;
  // Stack.
  sdrkit_string stack;
  // True if `dry` holds a value.
  bool has_dry;
  // Returns the plan of the scan and starts nothing.
  bool dry;
} sdrkit_scan_start_args;

// An arguments struct with its size set and every argument at 0.
#define SDRKIT_SCAN_START_ARGS_INIT {sizeof(sdrkit_scan_start_args), {0}, {0}, 0, 0}

// The arguments of `span`.
typedef struct sdrkit_scan_span_args {
  // The size of this struct: sizeof of the type. The caller sets it.
  uint32_t size;
  // Session.
  sdrkit_string session;
  // Start. A frequency in Hz.
  double start_hz;
  // Stop. A frequency in Hz.
  double stop_hz;
  // The span that the stack watches: the width of a block. The stack gives it as a result. A frequency in Hz.
  double span_hz;
  // The key of the result field of the stack whose value over `above` is a find: a field that holds a number, or a text that starts with one. The stack is `stack`, else the stack of the session. The command refuses a key that the stack does not have, with the list of the keys it has.
  sdrkit_string watch;
  // True if `above` holds a value.
  bool has_above;
  // A find is a watched value over this.
  double above;
  // True if `stack` holds a value.
  bool has_stack;
  // The stack id. Empty for the running stack.
  sdrkit_string stack;
  // True if `dwell_s` holds a value.
  bool has_dwell_s;
  // How long a block or a candidate is held. A length of time in seconds.
  double dwell_s;
  // True if `total_s` holds a value.
  bool has_total_s;
  // Total time. A length of time in seconds.
  double total_s;
  // True if `dry` holds a value.
  bool has_dry;
  // Returns the plan of the scan and starts nothing.
  bool dry;
} sdrkit_scan_span_args;

// An arguments struct with its size set and every argument at 0.
#define SDRKIT_SCAN_SPAN_ARGS_INIT {sizeof(sdrkit_scan_span_args), {0}, 0, 0, 0, {0}, 0, 0, 0, {0}, 0, 0, 0, 0, 0, 0}

// The arguments of `survey`.
typedef struct sdrkit_scan_survey_args {
  // The size of this struct: sizeof of the type. The caller sets it.
  uint32_t size;
  // Session.
  sdrkit_string session;
  // Start. A frequency in Hz.
  double start_hz;
  // Stop. A frequency in Hz.
  double stop_hz;
  // The span that the stack watches: the width of a block. The stack gives it as a result. A frequency in Hz.
  double span_hz;
  // The key of the result field of the stack whose value over `above` is a find: a field that holds a number, or a text that starts with one. The stack is `stack`, else the stack of the session. The command refuses a key that the stack does not have, with the list of the keys it has.
  sdrkit_string watch;
  // True if `above` holds a value.
  bool has_above;
  // A find is a watched value over this.
  double above;
  // True if `stack` holds a value.
  bool has_stack;
  // The stack id. Empty for the running stack.
  sdrkit_string stack;
  // True if `dwell_s` holds a value.
  bool has_dwell_s;
  // How long a block or a candidate is held. A length of time in seconds.
  double dwell_s;
  // True if `total_s` holds a value.
  bool has_total_s;
  // Total time. A length of time in seconds.
  double total_s;
  // True if `rate_hz` holds a value.
  bool has_rate_hz;
  // Widest rate. A frequency in Hz.
  double rate_hz;
  // True if `dry` holds a value.
  bool has_dry;
  // Returns the plan of the scan and starts nothing.
  bool dry;
} sdrkit_scan_survey_args;

// An arguments struct with its size set and every argument at 0.
#define SDRKIT_SCAN_SURVEY_ARGS_INIT {sizeof(sdrkit_scan_survey_args), {0}, 0, 0, 0, {0}, 0, 0, 0, {0}, 0, 0, 0, 0, 0, 0, 0, 0}

// The arguments of `hit.text`.
typedef struct sdrkit_scan_hit_text_args {
  // The size of this struct: sizeof of the type. The caller sets it.
  uint32_t size;
  // Text.
  sdrkit_string text;
  // True if `hit` holds a value.
  bool has_hit;
  // The row key of the hit. Empty for the newest find.
  sdrkit_string hit;
} sdrkit_scan_hit_text_args;

// An arguments struct with its size set and every argument at 0.
#define SDRKIT_SCAN_HIT_TEXT_ARGS_INIT {sizeof(sdrkit_scan_hit_text_args), {0}, 0, {0}}

// Fills `rows` with the changed rows of the table "hits" of the stack "scan" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_scan_hits_rows(SDRKIT_HANDLE handle,
                                           sdrkit_scan_hit_row *rows,
                                           uint32_t capacity,
                                           sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "scan.HitRow", "hits", rows, sizeof(sdrkit_scan_hit_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "candidates" of the stack "scan" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_scan_candidates_rows(SDRKIT_HANDLE handle,
                                                 sdrkit_scan_candidate_row *rows,
                                                 uint32_t capacity,
                                                 sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "scan.CandidateRow", "candidates", rows, sizeof(sdrkit_scan_candidate_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "occupied" of the stack "scan" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_scan_occupied_rows(SDRKIT_HANDLE handle,
                                               sdrkit_scan_occupied_row *rows,
                                               uint32_t capacity,
                                               sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "scan.OccupiedRow", "occupied", rows, sizeof(sdrkit_scan_occupied_row), capacity, info);
}

// Runs the command "start" (Scan the band of the stack) of the stack "scan" on the session of `handle`. Starts a channel scan of the stack, judged by its lock profile: the channels that the host gave for the stack, else one channel for each bandwidth of the range of the stack. If that range has more than 2000 channels, the scan takes the part of it that the session receives now, around its frequency.
static inline int32_t sdrkit_scan_start(SDRKIT_HANDLE handle, const sdrkit_scan_start_args *args, sdrkit_scan_start_reply *reply) {
  return sdrkit_typed_command(handle, "start", "scan.StartArgs", args, "scan.StartReply", reply, sizeof(sdrkit_scan_start_reply));
}

// Runs the command "stop" (Stop scan) of the stack "scan" on the session of `handle`.
static inline int32_t sdrkit_scan_stop(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "stop", NULL, NULL, NULL, NULL, 0);
}

// Runs the command "span" (Scan span blocks) of the stack "scan" on the session of `handle`. Starts a scan of the range from `start_hz` to `stop_hz` in blocks of `span_hz`, each held for `dwell_s` and judged by the result `watch` of the stack: a value over `above` is a find, which holds the scan (the setting after_find). The passes repeat until `total_s` is over.
static inline int32_t sdrkit_scan_span(SDRKIT_HANDLE handle, const sdrkit_scan_span_args *args, sdrkit_scan_span_reply *reply) {
  return sdrkit_typed_command(handle, "span", "scan.SpanArgs", args, "scan.SpanReply", reply, sizeof(sdrkit_scan_span_reply));
}

// Runs the command "survey" (Survey a wide range) of the stack "scan" on the session of `handle`. Starts a survey of the range from `start_hz` to `stop_hz`. Stage one steps the radio in blocks of `rate_hz` and looks for keyed carriers for the time of the setting look each, without the stack; stage two visits each candidate as a span block and lets the result `watch` of the stack decide. Stage one also lists the occupied channels of each block in the table occupied: power 3 dB or more over the local noise across 2 to 30 kHz, of any modulation, with centre, width and level; a lone carrier or spur, a ripple of the passband, a run at the end of a block and the edge of a wider signal are refused, and the rows are not visited. When one pass of span blocks fits into `total_s` (without a total time: when it is not longer than one pass of stage one), the survey runs span blocks; the reply says which. A survey with a `dwell_s` shorter than the look is refused. Without `rate_hz` the survey takes the width that the session receives, which is known as soon as the session has started: before its first samples it is the rate that the session asked of its radio, or the rate of its host feed. A survey can start at once after its session.
static inline int32_t sdrkit_scan_survey(SDRKIT_HANDLE handle, const sdrkit_scan_survey_args *args, sdrkit_scan_survey_reply *reply) {
  return sdrkit_typed_command(handle, "survey", "scan.SurveyArgs", args, "scan.SurveyReply", reply, sizeof(sdrkit_scan_survey_reply));
}

// Runs the command "hit.text" (Attach text to a hit) of the stack "scan" on the session of `handle`. Attaches a text, such as the text that the stack decoded, to a find of the running or last span or survey scan.
static inline int32_t sdrkit_scan_hit_text(SDRKIT_HANDLE handle, const sdrkit_scan_hit_text_args *args) {
  return sdrkit_typed_command(handle, "hit.text", "scan.HitTextArgs", args, NULL, NULL, 0);
}

// The setting "settle" (Settle time) of every scan. A length of time in seconds. After a retune, the time before the channel is judged.
static inline int32_t sdrkit_scan_get_settle(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "settle", value);
}
static inline int32_t sdrkit_scan_set_settle(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "settle", value);
}

// The setting "dwell" (Dwell) of every scan. A length of time in seconds. How long to listen to a quiet channel. The dwell of an entry wins.
static inline int32_t sdrkit_scan_get_dwell(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "dwell", value);
}
static inline int32_t sdrkit_scan_set_dwell(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "dwell", value);
}

// The setting "hang" (Resume delay) of every scan. A length of time in seconds. How long to stay after activity ends.
static inline int32_t sdrkit_scan_get_hang(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "hang", value);
}
static inline int32_t sdrkit_scan_set_hang(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "hang", value);
}

// The setting "max_hold" (Longest hold) of every scan. A length of time in seconds. The longest hold before the scan goes on. 0 holds while the channel is active.
static inline int32_t sdrkit_scan_get_max_hold(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "max_hold", value);
}
static inline int32_t sdrkit_scan_set_max_hold(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "max_hold", value);
}

// The setting "stop_on" (Stop on) of every scan. A value of `sdrkit_scan_stop_on`.
static inline int32_t sdrkit_scan_get_stop_on(SDRKIT_HANDLE handle, sdrkit_scan_stop_on *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "stop_on", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_scan_stop_on)index;
  return code;
}
static inline int32_t sdrkit_scan_set_stop_on(SDRKIT_HANDLE handle, sdrkit_scan_stop_on value) {
  return sdrkit_typed_setting_set_enum(handle, "stop_on", (int32_t)value);
}

// The setting "threshold_db" (Threshold) of every scan. Unit: dB. The SNR in dB, or the level in dBFS for a stop on level.
static inline int32_t sdrkit_scan_get_threshold_db(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "threshold_db", value);
}
static inline int32_t sdrkit_scan_set_threshold_db(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "threshold_db", value);
}

// The setting "priority_interval" (Priority interval) of every scan. A length of time in seconds. How often the priority channel is checked.
static inline int32_t sdrkit_scan_get_priority_interval(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "priority_interval", value);
}
static inline int32_t sdrkit_scan_set_priority_interval(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "priority_interval", value);
}

// The setting "priority_dwell" (Priority dwell) of every scan. A length of time in seconds. How long the priority channel is listened to on a check.
static inline int32_t sdrkit_scan_get_priority_dwell(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "priority_dwell", value);
}
static inline int32_t sdrkit_scan_set_priority_dwell(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "priority_dwell", value);
}

// The setting "passes" (Passes) of every scan. The passes over the channels before the scan ends. 0 scans until it is stopped.
static inline int32_t sdrkit_scan_get_passes(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "passes", value);
}
static inline int32_t sdrkit_scan_set_passes(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "passes", value);
}

// The setting "hit_cap" (Most hits) of every scan.
static inline int32_t sdrkit_scan_get_hit_cap(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "hit_cap", value);
}
static inline int32_t sdrkit_scan_set_hit_cap(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "hit_cap", value);
}

// The setting "keep_empty" (Keep empty channels) of every scan. A channel scan keeps the empty channels in its table, with the reason.
static inline int32_t sdrkit_scan_get_keep_empty(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "keep_empty", value);
}
static inline int32_t sdrkit_scan_set_keep_empty(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "keep_empty", value);
}

// The setting "after_find" (After a find) of every scan. A value of `sdrkit_scan_after_find`. What a span or survey scan does after a find: stay, or go on once the watched result is quiet.
static inline int32_t sdrkit_scan_get_after_find(SDRKIT_HANDLE handle, sdrkit_scan_after_find *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "after_find", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_scan_after_find)index;
  return code;
}
static inline int32_t sdrkit_scan_set_after_find(SDRKIT_HANDLE handle, sdrkit_scan_after_find value) {
  return sdrkit_typed_setting_set_enum(handle, "after_find", (int32_t)value);
}

// The setting "quiet" (Quiet time) of every scan. A length of time in seconds. After a find, how long the watched result must not pass its test before the scan goes on.
static inline int32_t sdrkit_scan_get_quiet(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "quiet", value);
}
static inline int32_t sdrkit_scan_set_quiet(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "quiet", value);
}

// The setting "reject_hold" (Rejection hold) of every scan. A length of time in seconds. How long a survey skips candidates near one on which the stack found nothing.
static inline int32_t sdrkit_scan_get_reject_hold(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "reject_hold", value);
}
static inline int32_t sdrkit_scan_set_reject_hold(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "reject_hold", value);
}

// The setting "look" (Look time) of every scan. A length of time in seconds. How long a survey looks at each wide block for keyed carriers: a longer look finds more slow Morse and more weak stations, and each pass takes longer.
static inline int32_t sdrkit_scan_get_look(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "look", value);
}
static inline int32_t sdrkit_scan_set_look(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "look", value);
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_scan_fields_read(SDRKIT_HANDLE handle, sdrkit_scan_fields *fields) {
  return sdrkit_typed_fields(handle, "scan.ScanFields", fields, sizeof(sdrkit_scan_fields));
}

#endif
