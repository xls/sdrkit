/*
 * sdrkit: the typed interface of the settings that every radio has, version 1.0. MIT licence.
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

#ifndef SDRKIT_RADIO_H
#define SDRKIT_RADIO_H

#include "../sdrkit.h"

// The version of the family that this header describes.
#define SDRKIT_RADIO_VERSION_MAJOR 1
#define SDRKIT_RADIO_VERSION_MINOR 0

// The options of `state` of `sdrkit_radio_fields`.
typedef enum sdrkit_radio_fields_state {
  // Idle
  SDRKIT_RADIO_FIELDS_STATE_IDLE = 0,
  // Connecting
  SDRKIT_RADIO_FIELDS_STATE_CONNECTING = 1,
  // Up
  SDRKIT_RADIO_FIELDS_STATE_UP = 2,
  // Reconnecting
  SDRKIT_RADIO_FIELDS_STATE_AWAY = 3,
  // Stopped
  SDRKIT_RADIO_FIELDS_STATE_STOPPED = 4,
} sdrkit_radio_fields_state;

// The link and the counts of a radio.
typedef struct sdrkit_radio_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Where the link to the radio is. A value of `sdrkit_radio_fields_state`.
  int32_t state;
  // True if `rate_hz` holds a value.
  bool has_rate_hz;
  // The sample rate that the radio provides. A frequency in Hz.
  double rate_hz;
  // True if `centre_hz` holds a value.
  bool has_centre_hz;
  // The centre frequency of the samples. A frequency in Hz.
  double centre_hz;
  // The samples that the radio delivered.
  int64_t samples;
  // The samples dropped because the engine did not keep up.
  int64_t dropped;
  // True if `buffer_pct` holds a value.
  bool has_buffer_pct;
  // The samples that wait in the buffer, in percent of its size. Unit: %.
  double buffer_pct;
  // True if `radio_lost` holds a value.
  bool has_radio_lost;
  // The samples that the radio or its link did not deliver.
  int64_t radio_lost;
  // True if `link_down` holds a value.
  bool has_link_down;
  // The samples missed while the link was down.
  int64_t link_down;
  // True if `delivered_pct` holds a value.
  bool has_delivered_pct;
  // The delivered rate in percent of the set rate. Unit: %.
  double delivered_pct;
  // True if `problem` holds a value.
  bool has_problem;
  // The last error of the radio.
  sdrkit_string problem;
} sdrkit_radio_fields;

// The options of `family` of `sdrkit_radio_connected_event`.
typedef enum sdrkit_radio_connected_event_family {
  // Aircraft
  SDRKIT_RADIO_CONNECTED_EVENT_FAMILY_AIRCRAFT = 0,
  // Vessel
  SDRKIT_RADIO_CONNECTED_EVENT_FAMILY_VESSEL = 1,
  // APRS
  SDRKIT_RADIO_CONNECTED_EVENT_FAMILY_APRS = 2,
  // Pager
  SDRKIT_RADIO_CONNECTED_EVENT_FAMILY_PAGER = 3,
  // Radiosonde
  SDRKIT_RADIO_CONNECTED_EVENT_FAMILY_SONDE = 4,
  // Broadcast
  SDRKIT_RADIO_CONNECTED_EVENT_FAMILY_BROADCAST = 5,
  // Voice
  SDRKIT_RADIO_CONNECTED_EVENT_FAMILY_VOICE = 6,
  // Message
  SDRKIT_RADIO_CONNECTED_EVENT_FAMILY_MESSAGE = 7,
  // Sensor
  SDRKIT_RADIO_CONNECTED_EVENT_FAMILY_SENSOR = 8,
  // Satellite
  SDRKIT_RADIO_CONNECTED_EVENT_FAMILY_SATELLITE = 9,
  // Receiver
  SDRKIT_RADIO_CONNECTED_EVENT_FAMILY_RECEIVER = 10,
  // Alert
  SDRKIT_RADIO_CONNECTED_EVENT_FAMILY_ALERT = 11,
  // Other
  SDRKIT_RADIO_CONNECTED_EVENT_FAMILY_OTHER = 12,
} sdrkit_radio_connected_event_family;

// Radio connected.
typedef struct sdrkit_radio_connected_event {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Seconds since the object started, in the time of the signal.
  double time;
  // The frequency of the signal in Hz.
  double freq_hz;
  // True if `source` holds a value.
  bool has_source;
  // Source.
  sdrkit_string source;
  // True if `text` holds a value.
  bool has_text;
  // Text.
  sdrkit_string text;
  // Family. A value of `sdrkit_radio_connected_event_family`.
  int32_t family;
} sdrkit_radio_connected_event;

// The options of `family` of `sdrkit_radio_lost_event`.
typedef enum sdrkit_radio_lost_event_family {
  // Aircraft
  SDRKIT_RADIO_LOST_EVENT_FAMILY_AIRCRAFT = 0,
  // Vessel
  SDRKIT_RADIO_LOST_EVENT_FAMILY_VESSEL = 1,
  // APRS
  SDRKIT_RADIO_LOST_EVENT_FAMILY_APRS = 2,
  // Pager
  SDRKIT_RADIO_LOST_EVENT_FAMILY_PAGER = 3,
  // Radiosonde
  SDRKIT_RADIO_LOST_EVENT_FAMILY_SONDE = 4,
  // Broadcast
  SDRKIT_RADIO_LOST_EVENT_FAMILY_BROADCAST = 5,
  // Voice
  SDRKIT_RADIO_LOST_EVENT_FAMILY_VOICE = 6,
  // Message
  SDRKIT_RADIO_LOST_EVENT_FAMILY_MESSAGE = 7,
  // Sensor
  SDRKIT_RADIO_LOST_EVENT_FAMILY_SENSOR = 8,
  // Satellite
  SDRKIT_RADIO_LOST_EVENT_FAMILY_SATELLITE = 9,
  // Receiver
  SDRKIT_RADIO_LOST_EVENT_FAMILY_RECEIVER = 10,
  // Alert
  SDRKIT_RADIO_LOST_EVENT_FAMILY_ALERT = 11,
  // Other
  SDRKIT_RADIO_LOST_EVENT_FAMILY_OTHER = 12,
} sdrkit_radio_lost_event_family;

// Radio lost.
typedef struct sdrkit_radio_lost_event {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Seconds since the object started, in the time of the signal.
  double time;
  // The frequency of the signal in Hz.
  double freq_hz;
  // True if `source` holds a value.
  bool has_source;
  // Source.
  sdrkit_string source;
  // True if `text` holds a value.
  bool has_text;
  // Text.
  sdrkit_string text;
  // Family. A value of `sdrkit_radio_lost_event_family`.
  int32_t family;
} sdrkit_radio_lost_event;

// The options of `family` of `sdrkit_radio_overflow_event`.
typedef enum sdrkit_radio_overflow_event_family {
  // Aircraft
  SDRKIT_RADIO_OVERFLOW_EVENT_FAMILY_AIRCRAFT = 0,
  // Vessel
  SDRKIT_RADIO_OVERFLOW_EVENT_FAMILY_VESSEL = 1,
  // APRS
  SDRKIT_RADIO_OVERFLOW_EVENT_FAMILY_APRS = 2,
  // Pager
  SDRKIT_RADIO_OVERFLOW_EVENT_FAMILY_PAGER = 3,
  // Radiosonde
  SDRKIT_RADIO_OVERFLOW_EVENT_FAMILY_SONDE = 4,
  // Broadcast
  SDRKIT_RADIO_OVERFLOW_EVENT_FAMILY_BROADCAST = 5,
  // Voice
  SDRKIT_RADIO_OVERFLOW_EVENT_FAMILY_VOICE = 6,
  // Message
  SDRKIT_RADIO_OVERFLOW_EVENT_FAMILY_MESSAGE = 7,
  // Sensor
  SDRKIT_RADIO_OVERFLOW_EVENT_FAMILY_SENSOR = 8,
  // Satellite
  SDRKIT_RADIO_OVERFLOW_EVENT_FAMILY_SATELLITE = 9,
  // Receiver
  SDRKIT_RADIO_OVERFLOW_EVENT_FAMILY_RECEIVER = 10,
  // Alert
  SDRKIT_RADIO_OVERFLOW_EVENT_FAMILY_ALERT = 11,
  // Other
  SDRKIT_RADIO_OVERFLOW_EVENT_FAMILY_OTHER = 12,
} sdrkit_radio_overflow_event_family;

// Samples dropped.
typedef struct sdrkit_radio_overflow_event {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Seconds since the object started, in the time of the signal.
  double time;
  // The frequency of the signal in Hz.
  double freq_hz;
  // True if `source` holds a value.
  bool has_source;
  // Source.
  sdrkit_string source;
  // True if `text` holds a value.
  bool has_text;
  // Text.
  sdrkit_string text;
  // Family. A value of `sdrkit_radio_overflow_event_family`.
  int32_t family;
} sdrkit_radio_overflow_event;

// The options of `family` of `sdrkit_radio_restart_event`.
typedef enum sdrkit_radio_restart_event_family {
  // Aircraft
  SDRKIT_RADIO_RESTART_EVENT_FAMILY_AIRCRAFT = 0,
  // Vessel
  SDRKIT_RADIO_RESTART_EVENT_FAMILY_VESSEL = 1,
  // APRS
  SDRKIT_RADIO_RESTART_EVENT_FAMILY_APRS = 2,
  // Pager
  SDRKIT_RADIO_RESTART_EVENT_FAMILY_PAGER = 3,
  // Radiosonde
  SDRKIT_RADIO_RESTART_EVENT_FAMILY_SONDE = 4,
  // Broadcast
  SDRKIT_RADIO_RESTART_EVENT_FAMILY_BROADCAST = 5,
  // Voice
  SDRKIT_RADIO_RESTART_EVENT_FAMILY_VOICE = 6,
  // Message
  SDRKIT_RADIO_RESTART_EVENT_FAMILY_MESSAGE = 7,
  // Sensor
  SDRKIT_RADIO_RESTART_EVENT_FAMILY_SENSOR = 8,
  // Satellite
  SDRKIT_RADIO_RESTART_EVENT_FAMILY_SATELLITE = 9,
  // Receiver
  SDRKIT_RADIO_RESTART_EVENT_FAMILY_RECEIVER = 10,
  // Alert
  SDRKIT_RADIO_RESTART_EVENT_FAMILY_ALERT = 11,
  // Other
  SDRKIT_RADIO_RESTART_EVENT_FAMILY_OTHER = 12,
} sdrkit_radio_restart_event_family;

// Restart needed.
typedef struct sdrkit_radio_restart_event {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Seconds since the object started, in the time of the signal.
  double time;
  // The frequency of the signal in Hz.
  double freq_hz;
  // True if `source` holds a value.
  bool has_source;
  // Source.
  sdrkit_string source;
  // True if `text` holds a value.
  bool has_text;
  // Text.
  sdrkit_string text;
  // Family. A value of `sdrkit_radio_restart_event_family`.
  int32_t family;
} sdrkit_radio_restart_event;

// Fills `event` with event `n` of the last sdrkit_events_take() on `handle`, an event of the type "radio.connected"
// (Radio connected). Returns SDRKIT_E_WRONG_TYPE if the event is of another type.
static inline int32_t sdrkit_radio_connected_event_read(SDRKIT_HANDLE handle, uint32_t n, sdrkit_radio_connected_event *event) {
  return sdrkit_typed_event(handle, n, "radio.connected", "radio.ConnectedEvent", event, sizeof(sdrkit_radio_connected_event));
}

// Fills `event` with event `n` of the last sdrkit_events_take() on `handle`, an event of the type "radio.lost"
// (Radio lost). Returns SDRKIT_E_WRONG_TYPE if the event is of another type.
static inline int32_t sdrkit_radio_lost_event_read(SDRKIT_HANDLE handle, uint32_t n, sdrkit_radio_lost_event *event) {
  return sdrkit_typed_event(handle, n, "radio.lost", "radio.LostEvent", event, sizeof(sdrkit_radio_lost_event));
}

// Fills `event` with event `n` of the last sdrkit_events_take() on `handle`, an event of the type "radio.overflow"
// (Samples dropped). Returns SDRKIT_E_WRONG_TYPE if the event is of another type.
static inline int32_t sdrkit_radio_overflow_event_read(SDRKIT_HANDLE handle, uint32_t n, sdrkit_radio_overflow_event *event) {
  return sdrkit_typed_event(handle, n, "radio.overflow", "radio.OverflowEvent", event, sizeof(sdrkit_radio_overflow_event));
}

// Fills `event` with event `n` of the last sdrkit_events_take() on `handle`, an event of the type "radio.restart"
// (Restart needed). Returns SDRKIT_E_WRONG_TYPE if the event is of another type.
static inline int32_t sdrkit_radio_restart_event_read(SDRKIT_HANDLE handle, uint32_t n, sdrkit_radio_restart_event *event) {
  return sdrkit_typed_event(handle, n, "radio.restart", "radio.RestartEvent", event, sizeof(sdrkit_radio_restart_event));
}

// Runs the command "reconnect" (Reconnect) of the stack "radio" on the session of `handle`.
static inline int32_t sdrkit_radio_reconnect(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "reconnect", NULL, NULL, NULL, NULL, 0);
}

// The setting "centre_hz" (Centre frequency) of every radio. A frequency in Hz.
static inline int32_t sdrkit_radio_get_centre_hz(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "centre_hz", value);
}
static inline int32_t sdrkit_radio_set_centre_hz(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "centre_hz", value);
}

// The setting "rate" (Sample rate) of every radio. A frequency in Hz. The rate of the samples from the radio. On Automatic the radio takes the least rate at or above what the stack needs; a rate that is set stays, and a stack that needs more is refused. Write 0 to return to Automatic.
static inline int32_t sdrkit_radio_get_rate(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "rate", value);
}
static inline int32_t sdrkit_radio_set_rate(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "rate", value);
}

// The setting "bandwidth" (Bandwidth) of every radio. A frequency in Hz. The width of the analogue filter of the radio. Without a width, the driver chooses one that fits the sample rate.
static inline int32_t sdrkit_radio_get_bandwidth(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "bandwidth", value);
}
static inline int32_t sdrkit_radio_set_bandwidth(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "bandwidth", value);
}

// The setting "gain" (Gain) of every radio. Unit: dB.
static inline int32_t sdrkit_radio_get_gain(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "gain", value);
}
static inline int32_t sdrkit_radio_set_gain(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "gain", value);
}

// The setting "agc" (Automatic gain) of every radio.
static inline int32_t sdrkit_radio_get_agc(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "agc", value);
}
static inline int32_t sdrkit_radio_set_agc(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "agc", value);
}

// The setting "rate_exact" (Exact rate) of every radio. Sends the rate as it is, even outside the rates the driver picks.
static inline int32_t sdrkit_radio_get_rate_exact(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "rate_exact", value);
}
static inline int32_t sdrkit_radio_set_rate_exact(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "rate_exact", value);
}

// The setting "bias_tee" (Bias tee) of every radio.
static inline int32_t sdrkit_radio_get_bias_tee(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "bias_tee", value);
}
static inline int32_t sdrkit_radio_set_bias_tee(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "bias_tee", value);
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_radio_fields_read(SDRKIT_HANDLE handle, sdrkit_radio_fields *fields) {
  return sdrkit_typed_fields(handle, "radio.RadioFields", fields, sizeof(sdrkit_radio_fields));
}

#endif
