/*
 * sdrkit: the typed interface of the shared interface "graph", whose items the stacks of several families show, version 1.0. MIT licence.
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

#ifndef SDRKIT_GRAPH_H
#define SDRKIT_GRAPH_H

#include "../sdrkit.h"

// The version of the family that this header describes.
#define SDRKIT_GRAPH_VERSION_MAJOR 1
#define SDRKIT_GRAPH_VERSION_MINOR 0

// The keys of the event types.

// The result set of [`DcRemoval`].
typedef struct sdrkit_graph_dc_removal_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // True if `dc_dbfs` holds a value.
  bool has_dc_dbfs;
  // The level that the component removes. No value before the first measurement. Unit: dBFS.
  double dc_dbfs;
} sdrkit_graph_dc_removal_fields;

// The result set of [`IqCorrection`]. No field has a value before the first measurement.
typedef struct sdrkit_graph_iq_correction_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // True if `gain_error_db` holds a value.
  bool has_gain_error_db;
  // The gain error between I and Q. Unit: dB.
  double gain_error_db;
  // True if `phase_error_deg` holds a value.
  bool has_phase_error_deg;
  // The phase error between I and Q. Unit: °.
  double phase_error_deg;
} sdrkit_graph_iq_correction_fields;

// The result set of [`NoiseBlanker`]. No field has a value in the first second.
typedef struct sdrkit_graph_noise_blanker_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // True if `blanked_pct` holds a value.
  bool has_blanked_pct;
  // The share of the last second that was blanked. Unit: %.
  double blanked_pct;
  // True if `blank_events` holds a value.
  bool has_blank_events;
  // The blanks since the start.
  int64_t blank_events;
} sdrkit_graph_noise_blanker_fields;

// The result set of [`PowerMeterSink`]. No field has a value before the first interval ends.
typedef struct sdrkit_graph_power_meter_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // True if `level_dbfs` holds a value.
  bool has_level_dbfs;
  // The mean level. Unit: dBFS.
  double level_dbfs;
  // True if `peak_dbfs` holds a value.
  bool has_peak_dbfs;
  // The peak level of the interval. Unit: dBFS.
  double peak_dbfs;
} sdrkit_graph_power_meter_fields;

// The result set of [`Squelch`]. No field has a value before the first interval ends.
typedef struct sdrkit_graph_squelch_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // True if `open` holds a value.
  bool has_open;
  // The squelch passes the signal.
  bool open;
  // True if `level_db` holds a value.
  bool has_level_db;
  // The level that the squelch compares with its threshold. Unit: dB.
  double level_db;
} sdrkit_graph_squelch_fields;

// The result set of [`AmDemod`].
typedef struct sdrkit_graph_am_demod_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // True if `carrier_offset_hz` holds a value.
  bool has_carrier_offset_hz;
  // The offset of the carrier that the synchronous detector tracks. No value with the envelope detector and before the first measurement. A frequency in Hz.
  double carrier_offset_hz;
} sdrkit_graph_am_demod_fields;

// The result set of [`StackGenerator`].
typedef struct sdrkit_graph_stack_generator_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // True if `ended` holds a value.
  bool has_ended;
  // The finite test signal of the stack has ended. No value before the component is configured.
  bool ended;
} sdrkit_graph_stack_generator_fields;

// The result set of the network sink.
typedef struct sdrkit_graph_net_sink_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // True if `sent_bytes` holds a value.
  bool has_sent_bytes;
  // The bytes sent. No value before the first send that succeeded. Unit: B.
  int64_t sent_bytes;
  // True if `failed` holds a value.
  bool has_failed;
  // The sends that failed. The samples of a failed send are lost. No value before the first failure.
  int64_t failed;
} sdrkit_graph_net_sink_fields;

// Fills `fields` with the result fields of the component "dc_removal" of the stack "graph" in the snapshot of
// `handle`. A generic consumer reads them under the keys "dc_removal.<key>".
static inline int32_t sdrkit_graph_dc_removal_fields_read(SDRKIT_HANDLE handle, sdrkit_graph_dc_removal_fields *fields) {
  return sdrkit_typed_fields(handle, "graph.DcRemovalFields@dc_removal", fields, sizeof(sdrkit_graph_dc_removal_fields));
}

// Fills `fields` with the result fields of the component "iq_correction" of the stack "graph" in the snapshot of
// `handle`. A generic consumer reads them under the keys "iq_correction.<key>".
static inline int32_t sdrkit_graph_iq_correction_fields_read(SDRKIT_HANDLE handle, sdrkit_graph_iq_correction_fields *fields) {
  return sdrkit_typed_fields(handle, "graph.IqCorrectionFields@iq_correction", fields, sizeof(sdrkit_graph_iq_correction_fields));
}

// Fills `fields` with the result fields of the component "noise_blanker" of the stack "graph" in the snapshot of
// `handle`. A generic consumer reads them under the keys "noise_blanker.<key>".
static inline int32_t sdrkit_graph_noise_blanker_fields_read(SDRKIT_HANDLE handle, sdrkit_graph_noise_blanker_fields *fields) {
  return sdrkit_typed_fields(handle, "graph.NoiseBlankerFields@noise_blanker", fields, sizeof(sdrkit_graph_noise_blanker_fields));
}

// Fills `fields` with the result fields of the component "power_meter" of the stack "graph" in the snapshot of
// `handle`. A generic consumer reads them under the keys "power_meter.<key>".
static inline int32_t sdrkit_graph_power_meter_fields_read(SDRKIT_HANDLE handle, sdrkit_graph_power_meter_fields *fields) {
  return sdrkit_typed_fields(handle, "graph.PowerMeterFields@power_meter", fields, sizeof(sdrkit_graph_power_meter_fields));
}

// Fills `fields` with the result fields of the component "squelch" of the stack "graph" in the snapshot of
// `handle`. A generic consumer reads them under the keys "squelch.<key>".
static inline int32_t sdrkit_graph_squelch_fields_read(SDRKIT_HANDLE handle, sdrkit_graph_squelch_fields *fields) {
  return sdrkit_typed_fields(handle, "graph.SquelchFields@squelch", fields, sizeof(sdrkit_graph_squelch_fields));
}

// Fills `fields` with the result fields of the component "am" of the stack "graph" in the snapshot of
// `handle`. A generic consumer reads them under the keys "am.<key>".
static inline int32_t sdrkit_graph_am_demod_fields_read(SDRKIT_HANDLE handle, sdrkit_graph_am_demod_fields *fields) {
  return sdrkit_typed_fields(handle, "graph.AmDemodFields@am", fields, sizeof(sdrkit_graph_am_demod_fields));
}

// Fills `fields` with the result fields of the component "generator" of the stack "graph" in the snapshot of
// `handle`. A generic consumer reads them under the keys "generator.<key>".
static inline int32_t sdrkit_graph_generator_stack_generator_fields_read(SDRKIT_HANDLE handle, sdrkit_graph_stack_generator_fields *fields) {
  return sdrkit_typed_fields(handle, "graph.StackGeneratorFields@generator", fields, sizeof(sdrkit_graph_stack_generator_fields));
}

// Fills `fields` with the result fields of the component "net_sink" of the stack "graph" in the snapshot of
// `handle`. A generic consumer reads them under the keys "net_sink.<key>".
static inline int32_t sdrkit_graph_net_sink_fields_read(SDRKIT_HANDLE handle, sdrkit_graph_net_sink_fields *fields) {
  return sdrkit_typed_fields(handle, "graph.NetSinkFields@net_sink", fields, sizeof(sdrkit_graph_net_sink_fields));
}

#endif
