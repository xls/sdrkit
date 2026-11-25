/*
 * sdrkit: the typed interface of the settings that every share has, version 1.0. MIT licence.
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

#ifndef SDRKIT_SHARE_H
#define SDRKIT_SHARE_H

#include "../sdrkit.h"

// The version of the family that this header describes.
#define SDRKIT_SHARE_VERSION_MAJOR 1
#define SDRKIT_SHARE_VERSION_MINOR 0

// The options of the setting "control" of the stack "share".
typedef enum sdrkit_share_control {
  // Follow the host
  SDRKIT_SHARE_CONTROL_FOLLOW = 0,
  // Remote control
  SDRKIT_SHARE_CONTROL_REMOTE = 1,
  // A slice for each client
  SDRKIT_SHARE_CONTROL_SLICE = 2,
} sdrkit_share_control;

// The options of the setting "format" of the stack "share".
typedef enum sdrkit_share_format {
  // As the radio gives
  SDRKIT_SHARE_FORMAT_NATIVE = 0,
  // 8-bit unsigned
  SDRKIT_SHARE_FORMAT_CU8 = 1,
  // 8-bit signed
  SDRKIT_SHARE_FORMAT_CS8 = 2,
  // 16-bit
  SDRKIT_SHARE_FORMAT_CS16 = 3,
  // 32-bit float
  SDRKIT_SHARE_FORMAT_CF32 = 4,
} sdrkit_share_format;

// The state and the counts of a share.
typedef struct sdrkit_share_fields {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The radio that the share serves.
  sdrkit_string radio;
  // The session of the share on the radio.
  sdrkit_string session;
  // The sample format that the share carries: `cu8`, `cs8`, `cs16` or `cf32`.
  sdrkit_string format;
  // The sample rate of the capture. A frequency in Hz.
  double rate_hz;
  // The centre frequency of the capture. A frequency in Hz.
  double centre_hz;
  // The listeners that run.
  int64_t listeners;
  // The clients that are connected.
  int64_t clients;
  // The blocks of samples that the share queued for its clients.
  int64_t blocks;
  // The samples that the share queued for its clients.
  int64_t samples;
  // The blocks that found no free buffer and reached no client.
  int64_t pool_misses;
  // A session of the host reads the radio: with the control Remote, no client steers.
  bool host_reads;
  // The requests of clients that changed the radio or a slice.
  int64_t requests_done;
  // The requests of clients that the share refused.
  int64_t requests_refused;
  // A client transmits through the share now.
  bool transmitting;
  // The requests to transmit that the share took.
  int64_t tx_taken;
  // The requests to transmit that the share refused.
  int64_t tx_refused;
  // The samples of clients that went to the transmit side of the session.
  int64_t tx_samples;
  // True if `problem` holds a value.
  bool has_problem;
  // Why the share does not serve: the hub or a listener could not be made.
  sdrkit_string problem;
} sdrkit_share_fields;

// The options of `protocol` of `sdrkit_share_listener_row`.
typedef enum sdrkit_share_listener_row_protocol {
  // rtl_tcp
  SDRKIT_SHARE_LISTENER_ROW_PROTOCOL_RTL_TCP = 0,
  // SoapyRemote
  SDRKIT_SHARE_LISTENER_ROW_PROTOCOL_SOAPYREMOTE = 1,
} sdrkit_share_listener_row_protocol;

// A row of the table `listeners`. The row key is the number of the listener, which the command `listen` returns.
typedef struct sdrkit_share_listener_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The protocol. A value of `sdrkit_share_listener_row_protocol`.
  int32_t protocol;
  // The local address.
  sdrkit_string address;
  // The port, as the system gave it for port 0.
  int64_t port;
  // The clients of the listener now.
  int64_t clients;
  // The connections that the listener took.
  int64_t accepted;
  // The connections refused because the address is outside the setting `allow`.
  int64_t refused_allow;
  // The connections refused because `max_clients` clients were connected.
  int64_t refused_full;
  // The bytes sent to the clients of the listener, past and present. Unit: B.
  int64_t bytes;
} sdrkit_share_listener_row;

// The options of `protocol` of `sdrkit_share_client_row`.
typedef enum sdrkit_share_client_row_protocol {
  // rtl_tcp
  SDRKIT_SHARE_CLIENT_ROW_PROTOCOL_RTL_TCP = 0,
  // SoapyRemote
  SDRKIT_SHARE_CLIENT_ROW_PROTOCOL_SOAPYREMOTE = 1,
} sdrkit_share_client_row_protocol;

// A row of the table `clients`. The row key is the number of the client.
typedef struct sdrkit_share_client_row {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The row key.
  sdrkit_string key;
  // The listener that took the client.
  sdrkit_string listener;
  // The protocol. A value of `sdrkit_share_client_row_protocol`.
  int32_t protocol;
  // True if `address` holds a value.
  bool has_address;
  // The address of the client.
  sdrkit_string address;
  // True if `format` holds a value.
  bool has_format;
  // The sample format of its stream. No value while it has no stream.
  sdrkit_string format;
  // When it connected. Seconds since 1970 (UTC).
  double since;
  // The bytes sent to it. Unit: B.
  int64_t bytes;
  // The samples sent to it.
  int64_t samples;
  // The samples dropped for this client because it was slow.
  int64_t dropped_samples;
  // The times that it began to lose samples.
  int64_t drop_events;
  // The breaks in its stream: its own losses, the losses of the radio and retunes.
  int64_t gaps;
  // The blocks that wait for it.
  int64_t queue_fill;
  // The blocks that its queue holds.
  int64_t queue_capacity;
  // The requests it made.
  int64_t requests;
  // The requests that the share refused outright. A request that was answered with the value in force is counted in the field `requests_refused` of the share.
  int64_t refused_requests;
  // True if `slice_centre_hz` holds a value.
  bool has_slice_centre_hz;
  // The centre of its slice, with the control Slice. A frequency in Hz.
  double slice_centre_hz;
  // True if `slice_rate_hz` holds a value.
  bool has_slice_rate_hz;
  // The rate of its slice, with the control Slice. A frequency in Hz.
  double slice_rate_hz;
  // With the control Remote, this client steers the radio while no session of the host reads it.
  bool steers;
  // The command `grant` let this client transmit.
  bool may_transmit;
} sdrkit_share_client_row;

// The reply of `listen`.
typedef struct sdrkit_share_listen_reply {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // The number of the listener: its row key in the table `listeners`.
  int64_t listener;
  // The port that the listener has.
  int64_t port;
} sdrkit_share_listen_reply;

// The fields of the event `share.client_joined`.
typedef struct sdrkit_share_client_joined_event {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Seconds since the object started, in the time of the signal.
  double time;
  // The frequency of the signal in Hz.
  double freq_hz;
  // The number of the client: its row key in the table `clients`.
  int64_t client;
  // The number of the listener that took it.
  int64_t listener;
  // True if `address` holds a value.
  bool has_address;
  // The address of the client.
  sdrkit_string address;
} sdrkit_share_client_joined_event;

// The options of `reason` of `sdrkit_share_client_left_event`.
typedef enum sdrkit_share_client_left_event_reason {
  // Closed
  SDRKIT_SHARE_CLIENT_LEFT_EVENT_REASON_CLOSED = 0,
  // Too slow
  SDRKIT_SHARE_CLIENT_LEFT_EVENT_REASON_TOO_SLOW = 1,
  // Kicked
  SDRKIT_SHARE_CLIENT_LEFT_EVENT_REASON_KICKED = 2,
  // Share stopped
  SDRKIT_SHARE_CLIENT_LEFT_EVENT_REASON_SHUTDOWN = 3,
} sdrkit_share_client_left_event_reason;

// The fields of the event `share.client_left`.
typedef struct sdrkit_share_client_left_event {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Seconds since the object started, in the time of the signal.
  double time;
  // The frequency of the signal in Hz.
  double freq_hz;
  // The number of the client.
  int64_t client;
  // The number of the listener that took it.
  int64_t listener;
  // True if `address` holds a value.
  bool has_address;
  // The address of the client.
  sdrkit_string address;
  // Why it left. A value of `sdrkit_share_client_left_event_reason`.
  int32_t reason;
} sdrkit_share_client_left_event;

// The options of `reason` of `sdrkit_share_refused_event`.
typedef enum sdrkit_share_refused_event_reason {
  // Address not allowed
  SDRKIT_SHARE_REFUSED_EVENT_REASON_NOT_ALLOWED = 0,
  // Too many clients
  SDRKIT_SHARE_REFUSED_EVENT_REASON_FULL = 1,
} sdrkit_share_refused_event_reason;

// The fields of the event `share.refused`.
typedef struct sdrkit_share_refused_event {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Seconds since the object started, in the time of the signal.
  double time;
  // The frequency of the signal in Hz.
  double freq_hz;
  // The number of the listener.
  int64_t listener;
  // True if `address` holds a value.
  bool has_address;
  // The address that connected.
  sdrkit_string address;
  // Why it was refused. A value of `sdrkit_share_refused_event_reason`.
  int32_t reason;
} sdrkit_share_refused_event;

// The options of `protocol` of `sdrkit_share_listen_failed_event`.
typedef enum sdrkit_share_listen_failed_event_protocol {
  // rtl_tcp
  SDRKIT_SHARE_LISTEN_FAILED_EVENT_PROTOCOL_RTL_TCP = 0,
  // SoapyRemote
  SDRKIT_SHARE_LISTEN_FAILED_EVENT_PROTOCOL_SOAPYREMOTE = 1,
} sdrkit_share_listen_failed_event_protocol;

// The fields of the event `share.listen_failed`.
typedef struct sdrkit_share_listen_failed_event {
  // The number of bytes of this struct that the library wrote.
  uint32_t size;
  // Seconds since the object started, in the time of the signal.
  double time;
  // The frequency of the signal in Hz.
  double freq_hz;
  // The protocol. A value of `sdrkit_share_listen_failed_event_protocol`.
  int32_t protocol;
  // The local address.
  sdrkit_string address;
  // The port.
  int64_t port;
  // What the network said.
  sdrkit_string reason;
} sdrkit_share_listen_failed_event;

// The arguments of `listen`.
typedef struct sdrkit_share_listen_args {
  // The size of this struct: sizeof of the type. The caller sets it.
  uint32_t size;
  // The protocol. The index of an option.
  int32_t protocol;
  // The port.
  int64_t port;
  // True if `host` holds a value.
  bool has_host;
  // The local address.
  sdrkit_string host;
} sdrkit_share_listen_args;

// An arguments struct with its size set and every argument at 0.
#define SDRKIT_SHARE_LISTEN_ARGS_INIT {sizeof(sdrkit_share_listen_args), 0, 0, 0, {0}}

// The arguments of `unlisten`.
typedef struct sdrkit_share_unlisten_args {
  // The size of this struct: sizeof of the type. The caller sets it.
  uint32_t size;
  // The number of the listener.
  int64_t listener;
} sdrkit_share_unlisten_args;

// An arguments struct with its size set and every argument at 0.
#define SDRKIT_SHARE_UNLISTEN_ARGS_INIT {sizeof(sdrkit_share_unlisten_args), 0}

// The arguments of `kick`.
typedef struct sdrkit_share_kick_args {
  // The size of this struct: sizeof of the type. The caller sets it.
  uint32_t size;
  // The number of the client.
  int64_t client;
} sdrkit_share_kick_args;

// An arguments struct with its size set and every argument at 0.
#define SDRKIT_SHARE_KICK_ARGS_INIT {sizeof(sdrkit_share_kick_args), 0}

// The arguments of `grant`.
typedef struct sdrkit_share_grant_args {
  // The size of this struct: sizeof of the type. The caller sets it.
  uint32_t size;
  // The number of the client.
  int64_t client;
  // True if `steer` holds a value.
  bool has_steer;
  // The client steers the radio.
  bool steer;
  // True if `transmit` holds a value.
  bool has_transmit;
  // The client may transmit.
  bool transmit;
} sdrkit_share_grant_args;

// An arguments struct with its size set and every argument at 0.
#define SDRKIT_SHARE_GRANT_ARGS_INIT {sizeof(sdrkit_share_grant_args), 0, 0, 0, 0, 0}

// Fills `rows` with the changed rows of the table "listeners" of the stack "share" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_share_listeners_rows(SDRKIT_HANDLE handle,
                                                 sdrkit_share_listener_row *rows,
                                                 uint32_t capacity,
                                                 sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "share.ListenerRow", "listeners", rows, sizeof(sdrkit_share_listener_row), capacity, info);
}

// Fills `rows` with the changed rows of the table "clients" of the stack "share" in the snapshot of `handle`,
// newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.
static inline int32_t sdrkit_share_clients_rows(SDRKIT_HANDLE handle,
                                               sdrkit_share_client_row *rows,
                                               uint32_t capacity,
                                               sdrkit_rows_info *info) {
  return sdrkit_typed_rows(handle, "share.ClientRow", "clients", rows, sizeof(sdrkit_share_client_row), capacity, info);
}

// Fills `event` with event `n` of the last sdrkit_events_take() on `handle`, an event of the type "share.client_joined"
// (Client joined). Returns SDRKIT_E_WRONG_TYPE if the event is of another type.
static inline int32_t sdrkit_share_client_joined_event_read(SDRKIT_HANDLE handle, uint32_t n, sdrkit_share_client_joined_event *event) {
  return sdrkit_typed_event(handle, n, "share.client_joined", "share.ClientJoinedEvent", event, sizeof(sdrkit_share_client_joined_event));
}

// Fills `event` with event `n` of the last sdrkit_events_take() on `handle`, an event of the type "share.client_left"
// (Client left). Returns SDRKIT_E_WRONG_TYPE if the event is of another type.
static inline int32_t sdrkit_share_client_left_event_read(SDRKIT_HANDLE handle, uint32_t n, sdrkit_share_client_left_event *event) {
  return sdrkit_typed_event(handle, n, "share.client_left", "share.ClientLeftEvent", event, sizeof(sdrkit_share_client_left_event));
}

// Fills `event` with event `n` of the last sdrkit_events_take() on `handle`, an event of the type "share.refused"
// (Connection refused). Returns SDRKIT_E_WRONG_TYPE if the event is of another type.
static inline int32_t sdrkit_share_refused_event_read(SDRKIT_HANDLE handle, uint32_t n, sdrkit_share_refused_event *event) {
  return sdrkit_typed_event(handle, n, "share.refused", "share.RefusedEvent", event, sizeof(sdrkit_share_refused_event));
}

// Fills `event` with event `n` of the last sdrkit_events_take() on `handle`, an event of the type "share.listen_failed"
// (Listener failed). Returns SDRKIT_E_WRONG_TYPE if the event is of another type.
static inline int32_t sdrkit_share_listen_failed_event_read(SDRKIT_HANDLE handle, uint32_t n, sdrkit_share_listen_failed_event *event) {
  return sdrkit_typed_event(handle, n, "share.listen_failed", "share.ListenFailedEvent", event, sizeof(sdrkit_share_listen_failed_event));
}

// Runs the command "listen" (Listen) of the stack "share" on the session of `handle`. Starts a listener of one protocol on the share. A share can have several listeners.
static inline int32_t sdrkit_share_listen(SDRKIT_HANDLE handle, const sdrkit_share_listen_args *args, sdrkit_share_listen_reply *reply) {
  return sdrkit_typed_command(handle, "listen", "share.ListenArgs", args, "share.ListenReply", reply, sizeof(sdrkit_share_listen_reply));
}

// Runs the command "unlisten" (Stop listening) of the stack "share" on the session of `handle`. Stops a listener. Its clients stay connected.
static inline int32_t sdrkit_share_unlisten(SDRKIT_HANDLE handle, const sdrkit_share_unlisten_args *args) {
  return sdrkit_typed_command(handle, "unlisten", "share.UnlistenArgs", args, NULL, NULL, 0);
}

// Runs the command "kick" (Disconnect) of the stack "share" on the session of `handle`. Disconnects one client.
static inline int32_t sdrkit_share_kick(SDRKIT_HANDLE handle, const sdrkit_share_kick_args *args) {
  return sdrkit_typed_command(handle, "kick", "share.KickArgs", args, NULL, NULL, 0);
}

// Runs the command "grant" (Grant) of the stack "share" on the session of `handle`. Sets what one client may do beyond listening: steer the radio, transmit. No client may transmit until this command says so.
static inline int32_t sdrkit_share_grant(SDRKIT_HANDLE handle, const sdrkit_share_grant_args *args) {
  return sdrkit_typed_command(handle, "grant", "share.GrantArgs", args, NULL, NULL, 0);
}

// The setting "control" (Control) of every share. A value of `sdrkit_share_control`. Follow: the host alone tunes; a request of a client to tune, to set the rate or the gain is not carried out, and the client is told the value in force. Remote: the client that the command `grant` names, or else the client that connected first, steers the radio while no session of the host reads it. Slice: each client gets its own frequency and rate inside the capture.
static inline int32_t sdrkit_share_get_control(SDRKIT_HANDLE handle, sdrkit_share_control *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "control", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_share_control)index;
  return code;
}
static inline int32_t sdrkit_share_set_control(SDRKIT_HANDLE handle, sdrkit_share_control value) {
  return sdrkit_typed_setting_set_enum(handle, "control", (int32_t)value);
}

// The setting "max_clients" (Clients at most) of every share. The most clients at one time, over every listener. A change disconnects the clients.
static inline int32_t sdrkit_share_get_max_clients(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "max_clients", value);
}
static inline int32_t sdrkit_share_set_max_clients(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "max_clients", value);
}

// The setting "client_queue_ms" (Client queue) of every share. Unit: ms. The time of signal that waits for a client that is slow. What does not fit is dropped for that client only. A change disconnects the clients.
static inline int32_t sdrkit_share_get_client_queue_ms(SDRKIT_HANDLE handle, int64_t *value) {
  return sdrkit_typed_setting_get_i64(handle, "client_queue_ms", value);
}
static inline int32_t sdrkit_share_set_client_queue_ms(SDRKIT_HANDLE handle, int64_t value) {
  return sdrkit_typed_setting_set_i64(handle, "client_queue_ms", value);
}

// The setting "drop_after_s" (Drop a slow client after) of every share. Unit: s. A client that loses samples for this long without a break is disconnected. A change disconnects the clients.
static inline int32_t sdrkit_share_get_drop_after_s(SDRKIT_HANDLE handle, double *value) {
  return sdrkit_typed_setting_get_f64(handle, "drop_after_s", value);
}
static inline int32_t sdrkit_share_set_drop_after_s(SDRKIT_HANDLE handle, double value) {
  return sdrkit_typed_setting_set_f64(handle, "drop_after_s", value);
}

// The setting "allow" (Allowed addresses) of every share. The address ranges that may connect, separated by commas: 192.168.1.0/24, fd00::/8, one address. Empty allows every address. A change disconnects the clients.
static inline int32_t sdrkit_share_get_allow(SDRKIT_HANDLE handle, sdrkit_string *value) {
  return sdrkit_typed_setting_get_string(handle, "allow", value);
}
static inline int32_t sdrkit_share_set_allow(SDRKIT_HANDLE handle, sdrkit_string value) {
  return sdrkit_typed_setting_set_string(handle, "allow", value);
}

// The setting "remote_gain" (Remote gain) of every share. With the control Remote, the client that steers may also set the gain and the automatic gain of the radio.
static inline int32_t sdrkit_share_get_remote_gain(SDRKIT_HANDLE handle, bool *value) {
  return sdrkit_typed_setting_get_bool(handle, "remote_gain", value);
}
static inline int32_t sdrkit_share_set_remote_gain(SDRKIT_HANDLE handle, bool value) {
  return sdrkit_typed_setting_set_bool(handle, "remote_gain", value);
}

// The setting "format" (Sample format) of every share. A value of `sdrkit_share_format`. The format in which the samples wait for the clients. A change disconnects the clients.
static inline int32_t sdrkit_share_get_format(SDRKIT_HANDLE handle, sdrkit_share_format *value) {
  int32_t index = 0;
  int32_t code = sdrkit_typed_setting_get_enum(handle, "format", &index);
  if (code == SDRKIT_OK && value) *value = (sdrkit_share_format)index;
  return code;
}
static inline int32_t sdrkit_share_set_format(SDRKIT_HANDLE handle, sdrkit_share_format value) {
  return sdrkit_typed_setting_set_enum(handle, "format", (int32_t)value);
}

// Fills `fields` with the result fields of the snapshot of `handle`.
static inline int32_t sdrkit_share_fields_read(SDRKIT_HANDLE handle, sdrkit_share_fields *fields) {
  return sdrkit_typed_fields(handle, "share.ShareFields", fields, sizeof(sdrkit_share_fields));
}

#endif
