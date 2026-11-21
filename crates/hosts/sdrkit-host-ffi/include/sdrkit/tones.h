/*
 * sdrkit: the typed interface of the shared interface "tones", whose items the stacks of several families show, version 1.0. MIT licence.
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

#ifndef SDRKIT_TONES_H
#define SDRKIT_TONES_H

#include "../sdrkit.h"

// The version of the family that this header describes.
#define SDRKIT_TONES_VERSION_MAJOR 1
#define SDRKIT_TONES_VERSION_MINOR 0

// The keys of the event types.
#define SDRKIT_EVENT_TONES_SUBAUDIO "tones.subaudio"
#define SDRKIT_EVENT_TONES_DTMF "tones.dtmf"
#define SDRKIT_EVENT_TONES_SELCALL "tones.selcall"
#define SDRKIT_EVENT_TONES_SELCAL "tones.selcal"
#define SDRKIT_EVENT_TONES_TWO_TONE "tones.two_tone"

// The options of `family` of `sdrkit_tones_subaudio_event`.
typedef enum sdrkit_tones_subaudio_event_family {
  // Aircraft
  SDRKIT_TONES_SUBAUDIO_EVENT_FAMILY_AIRCRAFT = 0,
  // Vessel
  SDRKIT_TONES_SUBAUDIO_EVENT_FAMILY_VESSEL = 1,
  // APRS
  SDRKIT_TONES_SUBAUDIO_EVENT_FAMILY_APRS = 2,
  // Pager
  SDRKIT_TONES_SUBAUDIO_EVENT_FAMILY_PAGER = 3,
  // Radiosonde
  SDRKIT_TONES_SUBAUDIO_EVENT_FAMILY_SONDE = 4,
  // Broadcast
  SDRKIT_TONES_SUBAUDIO_EVENT_FAMILY_BROADCAST = 5,
  // Voice
  SDRKIT_TONES_SUBAUDIO_EVENT_FAMILY_VOICE = 6,
  // Message
  SDRKIT_TONES_SUBAUDIO_EVENT_FAMILY_MESSAGE = 7,
  // Sensor
  SDRKIT_TONES_SUBAUDIO_EVENT_FAMILY_SENSOR = 8,
  // Satellite
  SDRKIT_TONES_SUBAUDIO_EVENT_FAMILY_SATELLITE = 9,
  // Receiver
  SDRKIT_TONES_SUBAUDIO_EVENT_FAMILY_RECEIVER = 10,
  // Alert
  SDRKIT_TONES_SUBAUDIO_EVENT_FAMILY_ALERT = 11,
  // Other
  SDRKIT_TONES_SUBAUDIO_EVENT_FAMILY_OTHER = 12,
} sdrkit_tones_subaudio_event_family;

// The options of `kind` of `sdrkit_tones_subaudio_event`.
typedef enum sdrkit_tones_subaudio_event_kind {
  // CTCSS
  SDRKIT_TONES_SUBAUDIO_EVENT_KIND_CTCSS = 0,
  // DCS
  SDRKIT_TONES_SUBAUDIO_EVENT_KIND_DCS = 1,
} sdrkit_tones_subaudio_event_kind;

// The options of `event` of `sdrkit_tones_subaudio_event`.
typedef enum sdrkit_tones_subaudio_event_value {
  // Locked
  SDRKIT_TONES_SUBAUDIO_EVENT_LOCKED = 0,
  // Released
  SDRKIT_TONES_SUBAUDIO_EVENT_RELEASED = 1,
  // Turn-off
  SDRKIT_TONES_SUBAUDIO_EVENT_TURN_OFF = 2,
} sdrkit_tones_subaudio_event_value;

// The fields of a `tones.subaudio` event: a change of a CTCSS tone or of a DCS code.
typedef struct sdrkit_tones_subaudio_event {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Seconds since the object started, in the time of the signal.
  double time;
  // The frequency of the signal in Hz.
  double freq_hz;
  // The kind of thing that the event is about. A value of `sdrkit_tones_subaudio_event_family`.
  int32_t family;
  // One line for people.
  sdrkit_string text;
  // The signalling. A value of `sdrkit_tones_subaudio_event_kind`.
  int32_t kind;
  // The change. A value of `sdrkit_tones_subaudio_event_value`.
  int32_t event;
  // True if `tone_hz` holds a value.
  bool has_tone_hz;
  // The CTCSS tone. A frequency in Hz.
  double tone_hz;
  // True if `eia` holds a value.
  bool has_eia;
  // The CTCSS tone is an EIA tone.
  bool eia;
  // True if `motorola` holds a value.
  bool has_motorola;
  // The Motorola code of the CTCSS tone.
  sdrkit_string motorola;
  // True if `measured_hz` holds a value.
  bool has_measured_hz;
  // The measured CTCSS tone. A frequency in Hz.
  double measured_hz;
  // True if `code` holds a value.
  bool has_code;
  // The DCS code with its polarity ("D023N").
  sdrkit_string code;
  // True if `standard` holds a value.
  bool has_standard;
  // The DCS code is a standard code.
  bool standard;
  // True if `alias` holds a value.
  bool has_alias;
  // A standard code that the same word also reads as.
  sdrkit_string alias;
  // The level of the tone, dB relative to a full-scale sine. Unit: dB.
  double level_db;
} sdrkit_tones_subaudio_event;

// The options of `family` of `sdrkit_tones_dtmf_event`.
typedef enum sdrkit_tones_dtmf_event_family {
  // Aircraft
  SDRKIT_TONES_DTMF_EVENT_FAMILY_AIRCRAFT = 0,
  // Vessel
  SDRKIT_TONES_DTMF_EVENT_FAMILY_VESSEL = 1,
  // APRS
  SDRKIT_TONES_DTMF_EVENT_FAMILY_APRS = 2,
  // Pager
  SDRKIT_TONES_DTMF_EVENT_FAMILY_PAGER = 3,
  // Radiosonde
  SDRKIT_TONES_DTMF_EVENT_FAMILY_SONDE = 4,
  // Broadcast
  SDRKIT_TONES_DTMF_EVENT_FAMILY_BROADCAST = 5,
  // Voice
  SDRKIT_TONES_DTMF_EVENT_FAMILY_VOICE = 6,
  // Message
  SDRKIT_TONES_DTMF_EVENT_FAMILY_MESSAGE = 7,
  // Sensor
  SDRKIT_TONES_DTMF_EVENT_FAMILY_SENSOR = 8,
  // Satellite
  SDRKIT_TONES_DTMF_EVENT_FAMILY_SATELLITE = 9,
  // Receiver
  SDRKIT_TONES_DTMF_EVENT_FAMILY_RECEIVER = 10,
  // Alert
  SDRKIT_TONES_DTMF_EVENT_FAMILY_ALERT = 11,
  // Other
  SDRKIT_TONES_DTMF_EVENT_FAMILY_OTHER = 12,
} sdrkit_tones_dtmf_event_family;

// The fields of a `tones.dtmf` event: a DTMF key that went down or up.
typedef struct sdrkit_tones_dtmf_event {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Seconds since the object started, in the time of the signal.
  double time;
  // The frequency of the signal in Hz.
  double freq_hz;
  // The kind of thing that the event is about. A value of `sdrkit_tones_dtmf_event_family`.
  int32_t family;
  // One line for people: the key.
  sdrkit_string text;
  // The DTMF key.
  sdrkit_string key;
  // The key went down (or up).
  bool down;
  // The level of the row tone. Unit: dB.
  double row_db;
  // The level of the column tone. Unit: dB.
  double column_db;
} sdrkit_tones_dtmf_event;

// The fields of a `tones.selcall` event: a selcall call.
typedef struct sdrkit_tones_selcall_event {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Seconds since the object started, in the time of the signal.
  double time;
  // The frequency of the signal in Hz.
  double freq_hz;
  // The digits of the call.
  sdrkit_string source;
  // One line for people: the digits.
  sdrkit_string text;
  // The digits of a call as heard.
  sdrkit_string digits;
  // The digits as sent, with the repeat tones.
  sdrkit_string sent;
  // The tone set the call was read as.
  sdrkit_string set;
  // True if `also_fits` holds a value.
  bool has_also_fits;
  // The other tone sets the call fits, separated by commas.
  sdrkit_string also_fits;
  // The length of the call. A length of time in seconds.
  double length_s;
  // The mean length of a tone. A length of time in seconds.
  double tone_s;
} sdrkit_tones_selcall_event;

// The options of `family` of `sdrkit_tones_selcal_event`.
typedef enum sdrkit_tones_selcal_event_family {
  // Aircraft
  SDRKIT_TONES_SELCAL_EVENT_FAMILY_AIRCRAFT = 0,
  // Vessel
  SDRKIT_TONES_SELCAL_EVENT_FAMILY_VESSEL = 1,
  // APRS
  SDRKIT_TONES_SELCAL_EVENT_FAMILY_APRS = 2,
  // Pager
  SDRKIT_TONES_SELCAL_EVENT_FAMILY_PAGER = 3,
  // Radiosonde
  SDRKIT_TONES_SELCAL_EVENT_FAMILY_SONDE = 4,
  // Broadcast
  SDRKIT_TONES_SELCAL_EVENT_FAMILY_BROADCAST = 5,
  // Voice
  SDRKIT_TONES_SELCAL_EVENT_FAMILY_VOICE = 6,
  // Message
  SDRKIT_TONES_SELCAL_EVENT_FAMILY_MESSAGE = 7,
  // Sensor
  SDRKIT_TONES_SELCAL_EVENT_FAMILY_SENSOR = 8,
  // Satellite
  SDRKIT_TONES_SELCAL_EVENT_FAMILY_SATELLITE = 9,
  // Receiver
  SDRKIT_TONES_SELCAL_EVENT_FAMILY_RECEIVER = 10,
  // Alert
  SDRKIT_TONES_SELCAL_EVENT_FAMILY_ALERT = 11,
  // Other
  SDRKIT_TONES_SELCAL_EVENT_FAMILY_OTHER = 12,
} sdrkit_tones_selcal_event_family;

// The fields of a `tones.selcal` event: a SELCAL call.
typedef struct sdrkit_tones_selcal_event {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Seconds since the object started, in the time of the signal.
  double time;
  // The frequency of the signal in Hz.
  double freq_hz;
  // The code.
  sdrkit_string source;
  // One line for people: the code.
  sdrkit_string text;
  // The kind of thing that the event is about. A value of `sdrkit_tones_selcal_event_family`.
  int32_t family;
  // The SELCAL code ("AB-CD").
  sdrkit_string code;
  // The code uses only the 16 classic tones.
  bool classic;
  // The level of the tones, dB relative to a full-scale sine. Unit: dB.
  double level_db;
  // The shift of the tones from their frequencies (a sideband receiver off tune). A frequency in Hz.
  double shift_hz;
} sdrkit_tones_selcal_event;

// The fields of a `tones.two_tone` event: a two-tone page.
typedef struct sdrkit_tones_two_tone_event {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Seconds since the object started, in the time of the signal.
  double time;
  // The frequency of the signal in Hz.
  double freq_hz;
  // One line for people: the two tones.
  sdrkit_string text;
  // The frequency of tone A. A frequency in Hz.
  double a_hz;
  // The frequency of tone B. A frequency in Hz.
  double b_hz;
  // The length of tone A. A length of time in seconds.
  double a_s;
  // The length of tone B. A length of time in seconds.
  double b_s;
  // Tone B ended (it did not run past the end of the input).
  bool ended;
} sdrkit_tones_two_tone_event;

// Fills `event` with event `n` of the last sdrkit_events_take() on `handle`, an event of the type "tones.subaudio"
// (CTCSS or DCS). Returns SDRKIT_E_WRONG_TYPE if the event is of another type.
static inline int32_t sdrkit_tones_subaudio_event_read(SDRKIT_HANDLE handle, uint32_t n, sdrkit_tones_subaudio_event *event) {
  return sdrkit_typed_event(handle, n, "tones.subaudio", "tones.SubAudioFields", event, sizeof(sdrkit_tones_subaudio_event));
}

// Fills `event` with event `n` of the last sdrkit_events_take() on `handle`, an event of the type "tones.dtmf"
// (DTMF key). Returns SDRKIT_E_WRONG_TYPE if the event is of another type.
static inline int32_t sdrkit_tones_dtmf_event_read(SDRKIT_HANDLE handle, uint32_t n, sdrkit_tones_dtmf_event *event) {
  return sdrkit_typed_event(handle, n, "tones.dtmf", "tones.DtmfFields", event, sizeof(sdrkit_tones_dtmf_event));
}

// Fills `event` with event `n` of the last sdrkit_events_take() on `handle`, an event of the type "tones.selcall"
// (Selcall call). Returns SDRKIT_E_WRONG_TYPE if the event is of another type.
static inline int32_t sdrkit_tones_selcall_event_read(SDRKIT_HANDLE handle, uint32_t n, sdrkit_tones_selcall_event *event) {
  return sdrkit_typed_event(handle, n, "tones.selcall", "tones.SelcallFields", event, sizeof(sdrkit_tones_selcall_event));
}

// Fills `event` with event `n` of the last sdrkit_events_take() on `handle`, an event of the type "tones.selcal"
// (SELCAL call). Returns SDRKIT_E_WRONG_TYPE if the event is of another type.
static inline int32_t sdrkit_tones_selcal_event_read(SDRKIT_HANDLE handle, uint32_t n, sdrkit_tones_selcal_event *event) {
  return sdrkit_typed_event(handle, n, "tones.selcal", "tones.SelcalFields", event, sizeof(sdrkit_tones_selcal_event));
}

// Fills `event` with event `n` of the last sdrkit_events_take() on `handle`, an event of the type "tones.two_tone"
// (Two-tone page). Returns SDRKIT_E_WRONG_TYPE if the event is of another type.
static inline int32_t sdrkit_tones_two_tone_event_read(SDRKIT_HANDLE handle, uint32_t n, sdrkit_tones_two_tone_event *event) {
  return sdrkit_typed_event(handle, n, "tones.two_tone", "tones.TwoToneFields", event, sizeof(sdrkit_tones_two_tone_event));
}

#endif
