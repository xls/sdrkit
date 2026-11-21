/*
 * sdrkit: the typed interface of the shared interface "broadcast_scan", whose items the stacks of several families show, version 1.0. MIT licence.
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

#ifndef SDRKIT_BROADCAST_SCAN_H
#define SDRKIT_BROADCAST_SCAN_H

#include "../sdrkit.h"

// The version of the family that this header describes.
#define SDRKIT_BROADCAST_SCAN_VERSION_MAJOR 1
#define SDRKIT_BROADCAST_SCAN_VERSION_MINOR 0

// The keys of the event types.

// The progress of a scan. No frequency to go to before the scan is over.
typedef struct sdrkit_broadcast_scan_progress_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The centres visited of all.
  sdrkit_string progress;
  // The radio centre now. A frequency in Hz.
  double centre;
  // The stations found so far.
  int64_t found;
  // True if `tune_to` holds a value.
  bool has_tune_to;
  // Where the radio goes when the scan is over. A frequency in Hz.
  double tune_to;
  // Whether the scan is over.
  bool done;
} sdrkit_broadcast_scan_progress_fields;

// The options of `state` of `sdrkit_broadcast_scan_station_row`.
typedef enum sdrkit_broadcast_scan_station_row_state {
  // Decoded
  SDRKIT_BROADCAST_SCAN_STATION_ROW_STATE_DECODED = 0,
  // Found
  SDRKIT_BROADCAST_SCAN_STATION_ROW_STATE_FOUND = 1,
} sdrkit_broadcast_scan_station_row_state;

// A station that the scan found. The row key is the frequency in whole hertz.
typedef struct sdrkit_broadcast_scan_station_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The frequency of the station. A frequency in Hz.
  double freq_hz;
  // How far the station got. A value of `sdrkit_broadcast_scan_station_row_state`.
  int32_t state;
  // The name of the station.
  sdrkit_string name;
  // True if `snr_db` holds a value.
  bool has_snr_db;
  // The receiver's SNR. Unit: dB.
  double snr_db;
  // True if `level_dbfs` holds a value.
  bool has_level_dbfs;
  // The receiver's level. Unit: dBFS.
  double level_dbfs;
  // How far the signal stood above the noise floor. Unit: dB.
  double over_floor_db;
} sdrkit_broadcast_scan_station_row;

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_broadcast_scan_progress_fields_read(SDRKIT_HANDLE handle, sdrkit_broadcast_scan_progress_fields *fields) {
  return sdrkit_typed_fields(handle, "broadcast_scan.ProgressFields", fields, sizeof(sdrkit_broadcast_scan_progress_fields));
}

#endif
