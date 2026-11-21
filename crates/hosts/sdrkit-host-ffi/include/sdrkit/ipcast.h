/*
 * sdrkit: the typed interface of the shared interface "ipcast", whose items the stacks of several families show, version 1.0. MIT licence.
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

#ifndef SDRKIT_IPCAST_H
#define SDRKIT_IPCAST_H

#include "../sdrkit.h"

// The version of the family that this header describes.
#define SDRKIT_IPCAST_VERSION_MAJOR 1
#define SDRKIT_IPCAST_VERSION_MINOR 0

// The keys of the event types.

// The result fields of `ipcast.bbframe`: the counts of the mode adaptation.
typedef struct sdrkit_ipcast_bb_frame_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // BBFRAMEs with a good header.
  int64_t frames_ok;
  // BBFRAMEs with a bad header.
  int64_t frames_bad;
  // Transport packets of the chosen stream.
  int64_t ts_packets;
  // User packets with a bad CRC-8.
  int64_t crc_errors;
  // GSE PDUs of every stream.
  int64_t pdus;
  // GSE PDUs that carry no IP.
  int64_t non_ip;
} sdrkit_ipcast_bb_frame_fields;

// The result fields of `ipcast.alp`: the counts of the link layer.
typedef struct sdrkit_ipcast_alp_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // ALP packets read.
  int64_t alp_packets;
  // ALP packets dropped.
  int64_t alp_dropped;
  // ALP packets of IP with compressed headers.
  int64_t compressed_ip;
} sdrkit_ipcast_alp_fields;

// The result fields of `ipcast.ip`: the counts of the IP layer.
typedef struct sdrkit_ipcast_ip_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // IP packets read.
  int64_t ip_packets;
  // IP packets refused.
  int64_t ip_bad;
  // Datagrams put together from fragments.
  int64_t reassembled;
  // Flows seen.
  int64_t flows;
  // Flows whose payload is encrypted.
  int64_t encrypted_flows;
} sdrkit_ipcast_ip_fields;

// The result fields of `ipcast.route`: the counts of the object delivery.
typedef struct sdrkit_ipcast_route_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Objects received whole.
  int64_t objects;
  // Objects dropped.
  int64_t objects_dropped;
} sdrkit_ipcast_route_fields;

// Fills `fields` with the result fields of the component "bbframe" of the stack "ipcast" in the snapshot of
// `handle`. A generic consumer reads them under the keys "bbframe.<key>".
static inline int32_t sdrkit_ipcast_bbframe_bb_frame_fields_read(SDRKIT_HANDLE handle, sdrkit_ipcast_bb_frame_fields *fields) {
  return sdrkit_typed_fields(handle, "ipcast.BbFrameFields@bbframe", fields, sizeof(sdrkit_ipcast_bb_frame_fields));
}

// Fills `fields` with the result fields of the component "alp" of the stack "ipcast" in the snapshot of
// `handle`. A generic consumer reads them under the keys "alp.<key>".
static inline int32_t sdrkit_ipcast_alp_fields_read(SDRKIT_HANDLE handle, sdrkit_ipcast_alp_fields *fields) {
  return sdrkit_typed_fields(handle, "ipcast.AlpFields@alp", fields, sizeof(sdrkit_ipcast_alp_fields));
}

// Fills `fields` with the result fields of the component "ip" of the stack "ipcast" in the snapshot of
// `handle`. A generic consumer reads them under the keys "ip.<key>".
static inline int32_t sdrkit_ipcast_ip_fields_read(SDRKIT_HANDLE handle, sdrkit_ipcast_ip_fields *fields) {
  return sdrkit_typed_fields(handle, "ipcast.IpFields@ip", fields, sizeof(sdrkit_ipcast_ip_fields));
}

// Fills `fields` with the result fields of the component "route" of the stack "ipcast" in the snapshot of
// `handle`. A generic consumer reads them under the keys "route.<key>".
static inline int32_t sdrkit_ipcast_route_fields_read(SDRKIT_HANDLE handle, sdrkit_ipcast_route_fields *fields) {
  return sdrkit_typed_fields(handle, "ipcast.RouteFields@route", fields, sizeof(sdrkit_ipcast_route_fields));
}

#endif
