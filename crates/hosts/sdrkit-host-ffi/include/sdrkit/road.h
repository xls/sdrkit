/*
 * sdrkit: the typed interface of the stack family "road", version 1.0. MIT licence.
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

#ifndef SDRKIT_ROAD_H
#define SDRKIT_ROAD_H

#include "../sdrkit.h"

// The version of the family that this header describes.
#define SDRKIT_ROAD_VERSION_MAJOR 1
#define SDRKIT_ROAD_VERSION_MINOR 0

// The stack "v2x": its id, and the keys of its event types.
#define SDRKIT_STACK_V2X "v2x"
#define SDRKIT_EVENT_V2X_VEHICLE "v2x.vehicle"

// The options of the setting "gen_rate" of the stack "v2x".
typedef enum sdrkit_v2x_gen_rate {
  // 3 Mbit/s (BPSK 1/2)
  SDRKIT_V2X_GEN_RATE_3_MBIT_S = 0,
  // 6 Mbit/s (QPSK 1/2)
  SDRKIT_V2X_GEN_RATE_6_MBIT_S = 1,
} sdrkit_v2x_gen_rate;

// The result fields of V2X.
typedef struct sdrkit_v2x_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Frames (packets, blocks, bursts) that passed their check.
  int64_t frames_ok;
  // Frames that failed their check.
  int64_t frames_bad;
  // Safety messages (BSM and CAM).
  int64_t safety_messages;
  // DENM frames, counted.
  int64_t denm;
  // Vehicles in the table.
  int64_t vehicles;
  // Short training fields detected.
  int64_t preambles;
  // True if `level_dbfs` holds a value.
  bool has_level_dbfs;
  // Level of the signal or channel, dBFS (a mean power of 1 is 0 dBFS). Unit: dBFS.
  double level_dbfs;
  // True if `snr_db` holds a value.
  bool has_snr_db;
  // Signal to noise ratio of what is being decoded, dB (in the signal's own bandwidth, or per symbol where the stack says so). Unit: dB.
  double snr_db;
  // True if `offset_hz` holds a value.
  bool has_offset_hz;
  // Carrier offset from where it was expected, Hz. A frequency in Hz.
  double offset_hz;
  // The text of a decoded message.
  sdrkit_string text;
  // What no public document confirms.
  sdrkit_string note;
} sdrkit_v2x_fields;

// The result set that a [`DecoderComponent`] adds to the results of its decoder. The description of a stack with a decoder component names it after the result sets of the decoder.
typedef struct sdrkit_v2x_decoder_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // True if `dropped` holds a value.
  bool has_dropped;
  // Items dropped because the decoder was slow. It holds a value while the decoder runs on a helper thread.
  int64_t dropped;
} sdrkit_v2x_decoder_fields;

// The options of `kind` of `sdrkit_v2x_vehicle_row`.
typedef enum sdrkit_v2x_vehicle_row_kind {
  // BSM
  SDRKIT_V2X_VEHICLE_ROW_KIND_BSM = 0,
  // CAM
  SDRKIT_V2X_VEHICLE_ROW_KIND_CAM = 1,
  // DENM
  SDRKIT_V2X_VEHICLE_ROW_KIND_DENM = 2,
  // WSMP
  SDRKIT_V2X_VEHICLE_ROW_KIND_WSMP = 3,
  // GeoNetworking
  SDRKIT_V2X_VEHICLE_ROW_KIND_GEONETWORKING = 4,
} sdrkit_v2x_vehicle_row_kind;

// A vehicle heard. The source is the temporary id.
typedef struct sdrkit_v2x_vehicle_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The kind of a row. A value of `sdrkit_v2x_vehicle_row_kind`.
  int32_t kind;
  // The sender of a decoded message: a registration, an address, a call sign or a code.
  sdrkit_string source;
  // True if `position` holds a value.
  bool has_position;
  // The position of the object that the event is about.
  sdrkit_position position;
  // True if `speed_kt` holds a value.
  bool has_speed_kt;
  // The speed over the ground of the object that the event is about, knots. Unit: kt.
  double speed_kt;
  // True if `course_deg` holds a value.
  bool has_course_deg;
  // The course over the ground of the object that the event is about, degrees clockwise from true north. Unit: °.
  double course_deg;
  // The text of a decoded message.
  sdrkit_string text;
  // Messages from a vehicle.
  int64_t frames;
  // Level of the signal or channel, dBFS (a mean power of 1 is 0 dBFS). Unit: dBFS.
  double level_dbfs;
  // True if `first_heard` holds a value.
  bool has_first_heard;
  // Wall-clock time when first heard. Seconds since 1970 (UTC).
  double first_heard;
  // True if `last_heard` holds a value.
  bool has_last_heard;
  // Wall-clock time when last heard, in whole seconds. A host shows the age from it. Seconds since 1970 (UTC).
  double last_heard;
} sdrkit_v2x_vehicle_row;

// The options of `kind` of `sdrkit_v2x_frame_row`.
typedef enum sdrkit_v2x_frame_row_kind {
  // BSM
  SDRKIT_V2X_FRAME_ROW_KIND_BSM = 0,
  // CAM
  SDRKIT_V2X_FRAME_ROW_KIND_CAM = 1,
  // DENM
  SDRKIT_V2X_FRAME_ROW_KIND_DENM = 2,
  // WSMP
  SDRKIT_V2X_FRAME_ROW_KIND_WSMP = 3,
  // GeoNetworking
  SDRKIT_V2X_FRAME_ROW_KIND_GEONETWORKING = 4,
} sdrkit_v2x_frame_row_kind;

// A frame of the frame log.
typedef struct sdrkit_v2x_frame_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The signal time of a frame. Unit: s.
  double time_s;
  // The kind of a row. A value of `sdrkit_v2x_frame_row_kind`.
  int32_t kind;
  // The sender of a decoded message: a registration, an address, a call sign or a code.
  sdrkit_string source;
  // The text of a decoded message.
  sdrkit_string text;
  // Level of the signal or channel, dBFS (a mean power of 1 is 0 dBFS). Unit: dBFS.
  double level_dbfs;
} sdrkit_v2x_frame_row;

// The options of `kind` of `sdrkit_v2x_vehicle_event`.
typedef enum sdrkit_v2x_vehicle_event_kind {
  // BSM
  SDRKIT_V2X_VEHICLE_EVENT_KIND_BSM = 0,
  // CAM
  SDRKIT_V2X_VEHICLE_EVENT_KIND_CAM = 1,
  // DENM
  SDRKIT_V2X_VEHICLE_EVENT_KIND_DENM = 2,
  // WSMP
  SDRKIT_V2X_VEHICLE_EVENT_KIND_WSMP = 3,
  // GeoNetworking
  SDRKIT_V2X_VEHICLE_EVENT_KIND_GEONETWORKING = 4,
} sdrkit_v2x_vehicle_event_kind;

// The options of `family` of `sdrkit_v2x_vehicle_event`.
typedef enum sdrkit_v2x_vehicle_event_family {
  // Aircraft
  SDRKIT_V2X_VEHICLE_EVENT_FAMILY_AIRCRAFT = 0,
  // Vessel
  SDRKIT_V2X_VEHICLE_EVENT_FAMILY_VESSEL = 1,
  // APRS
  SDRKIT_V2X_VEHICLE_EVENT_FAMILY_APRS = 2,
  // Pager
  SDRKIT_V2X_VEHICLE_EVENT_FAMILY_PAGER = 3,
  // Radiosonde
  SDRKIT_V2X_VEHICLE_EVENT_FAMILY_SONDE = 4,
  // Broadcast
  SDRKIT_V2X_VEHICLE_EVENT_FAMILY_BROADCAST = 5,
  // Voice
  SDRKIT_V2X_VEHICLE_EVENT_FAMILY_VOICE = 6,
  // Message
  SDRKIT_V2X_VEHICLE_EVENT_FAMILY_MESSAGE = 7,
  // Sensor
  SDRKIT_V2X_VEHICLE_EVENT_FAMILY_SENSOR = 8,
  // Satellite
  SDRKIT_V2X_VEHICLE_EVENT_FAMILY_SATELLITE = 9,
  // Receiver
  SDRKIT_V2X_VEHICLE_EVENT_FAMILY_RECEIVER = 10,
  // Alert
  SDRKIT_V2X_VEHICLE_EVENT_FAMILY_ALERT = 11,
  // Other
  SDRKIT_V2X_VEHICLE_EVENT_FAMILY_OTHER = 12,
} sdrkit_v2x_vehicle_event_family;

// A vehicle heard for the first time.
typedef struct sdrkit_v2x_vehicle_event {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Seconds since the object started, in the time of the signal.
  double time;
  // The frequency of the signal in Hz.
  double freq_hz;
  // The kind of a row. A value of `sdrkit_v2x_vehicle_event_kind`.
  int32_t kind;
  // The sender of a decoded message: a registration, an address, a call sign or a code.
  sdrkit_string source;
  // True if `position` holds a value.
  bool has_position;
  // The position of the object that the event is about.
  sdrkit_position position;
  // The text of a decoded message.
  sdrkit_string text;
  // What an event is about (aircraft, vessel, pager). Outputs and the store filter on it. A value of `sdrkit_v2x_vehicle_event_family`.
  int32_t family;
} sdrkit_v2x_vehicle_event;

// The arguments of `remove` of a vehicle.
typedef struct sdrkit_v2x_vehicle_args {
  // The size of this struct: sizeof of the type. The caller sets it.
  uint32_t size;
  // The row key of the vehicle.
  sdrkit_string row;
} sdrkit_v2x_vehicle_args;

// An arguments struct with its size set and every argument at 0.
#define SDRKIT_V2X_VEHICLE_ARGS_INIT {sizeof(sdrkit_v2x_vehicle_args), {0}}

// Fills `rows` with the changed rows of the table "vehicles" of the stack "v2x" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_v2x_vehicles_rows(SDRKIT_HANDLE handle,
                                              sdrkit_v2x_vehicle_row *rows,
                                              uint32_t capacity,
                                              sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "road.V2xVehicleRow", "vehicles", rows, sizeof(sdrkit_v2x_vehicle_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "frames" of the stack "v2x" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_v2x_frames_rows(SDRKIT_HANDLE handle,
                                            sdrkit_v2x_frame_row *rows,
                                            uint32_t capacity,
                                            sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "road.V2xFrameRow", "frames", rows, sizeof(sdrkit_v2x_frame_row), capacity, info);
}

// Fills `event` with event `n` of the last sdrkit_events_take() on `handle`, an event of the type "v2x.vehicle"
// (V2X vehicle). Returns SDRKIT_E_WRONG_TYPE if the event is of another type.
static inline int32_t sdrkit_v2x_vehicle_event_read(SDRKIT_HANDLE handle, uint32_t n, sdrkit_v2x_vehicle_event *event) {
  return sdrkit_typed_event(handle, n, "v2x.vehicle", "road.V2xVehicleEvent", event, sizeof(sdrkit_v2x_vehicle_event));
}

// Runs the command "clear" (Clear) of the stack "v2x" on the session of `handle`. Forgets everything decoded so far.
static inline int32_t sdrkit_v2x_clear(SDRKIT_HANDLE handle) {
  return sdrkit_typed_command(handle, "clear", NULL, NULL, NULL, NULL, 0);
}

// Runs the command "remove" (Remove) of the stack "v2x" on the session of `handle`. Forgets one object, a favourite too.
static inline int32_t sdrkit_v2x_remove(SDRKIT_HANDLE handle, const sdrkit_v2x_vehicle_args *args) {
  return sdrkit_typed_command(handle, "remove", "road.V2xVehicleArgs", args, NULL, NULL, 0);
}

// The settings of the test signal generator of the stack "v2x" are the sdrkit_v2x_gen_*() functions
// after the settings of its receiver.

// The setting "gen_rate" (Test signal data rate) of the test signal generator of the stack "v2x". A value of `sdrkit_v2x_gen_rate`.
static inline int32_t sdrkit_v2x_gen_get_rate(SDRKIT_HANDLE handle, sdrkit_v2x_gen_rate *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "gen_rate", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_v2x_gen_rate)index;
  return code;
}
static inline int32_t sdrkit_v2x_gen_set_rate(SDRKIT_HANDLE handle, sdrkit_v2x_gen_rate value) {
  return sdrkit_typed_setting_set_enum(handle, "gen_rate", (int32_t)value);
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_v2x_fields_read(SDRKIT_HANDLE handle, sdrkit_v2x_fields *fields) {
  return sdrkit_typed_fields(handle, "road.V2xFields", fields, sizeof(sdrkit_v2x_fields));
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_v2x_decoder_fields_read(SDRKIT_HANDLE handle, sdrkit_v2x_decoder_fields *fields) {
  return sdrkit_typed_fields(handle, "road.DecoderFields", fields, sizeof(sdrkit_v2x_decoder_fields));
}

#endif
